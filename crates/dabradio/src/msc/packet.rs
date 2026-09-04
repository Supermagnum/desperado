//! DAB packet-mode reassembly (ETSI EN 300 401 §5.3.2 / §5.3.3).
//!
//! MSC logical frames from [`super::MscHandler`] are a concatenation of
//! 24/48/72/96-byte packets. Each packet has a 3-byte header, a useful-data
//! field, padding, and a 2-byte CRC-16 (ITU-T X.25). Packets sharing an
//! address are concatenated using First/Last flags into an MSC data group.
//!
//! Packet header layout matches dab-cmdline `dataProcessor::handlePacket()`:
//! length (2), continuity (2), first/last (2), address (10), command (1),
//! useful length (7).

use crate::constants;
use tracing::debug;

/// Standard packet lengths in bytes (EN 300 401 Table 6).
const PACKET_LENGTHS: [usize; 4] = [24, 48, 72, 96];

/// Statistics for CRC validation. A pass rate indistinguishable from
/// chance (1/65536 per check) means the MSC bits are still wrong.
#[derive(Debug, Default, Clone, Copy)]
pub struct PacketStats {
    pub packets_seen: u64,
    pub crc_ok: u64,
    pub crc_fail: u64,
    pub address_mismatch: u64,
    pub groups_complete: u64,
}

impl PacketStats {
    pub fn crc_pass_rate(&self) -> f64 {
        if self.packets_seen == 0 {
            0.0
        } else {
            self.crc_ok as f64 / self.packets_seen as f64
        }
    }

    /// True when the observed CRC pass rate is indistinguishable from a
    /// random bitstream scanned at known packet boundaries.
    pub fn is_chance_level(&self) -> bool {
        self.packets_seen >= 64 && self.crc_pass_rate() < 0.05
    }
}

/// One reassembled MSC data group (header + optional session header + data
/// + optional CRC), as raw bytes.
#[derive(Debug, Clone)]
pub struct DataGroup {
    pub address: u16,
    pub bytes: Vec<u8>,
}

impl DataGroup {
    /// Strip the MSC data-group header / CRC and return the application
    /// payload. Returns the full buffer if the header is truncated.
    pub fn application_payload(&self) -> &[u8] {
        strip_data_group_header(&self.bytes)
    }
}

/// Assemble packets for a single packet address.
pub struct PacketAssembler {
    address: u16,
    assembling: bool,
    last_continuity: Option<u8>,
    buffer: Vec<u8>,
    pub stats: PacketStats,
}

impl PacketAssembler {
    pub fn new(address: u16) -> Self {
        Self {
            address,
            assembling: false,
            last_continuity: None,
            buffer: Vec::new(),
            stats: PacketStats::default(),
        }
    }

    /// Consume one MSC logical frame (decoded subchannel bytes) and return
    /// any completed data groups.
    pub fn feed_bytes(&mut self, bytes: &[u8]) -> Vec<DataGroup> {
        let mut groups = Vec::new();
        let mut offset = 0;
        while offset + 5 <= bytes.len() {
            let length_id = (bytes[offset] >> 6) & 0x03;
            let packet_len = PACKET_LENGTHS[length_id as usize];
            if offset + packet_len > bytes.len() {
                break;
            }
            let packet = &bytes[offset..offset + packet_len];
            offset += packet_len;
            if let Some(group) = self.handle_packet(packet) {
                groups.push(group);
            }
        }
        groups
    }

    fn handle_packet(&mut self, packet: &[u8]) -> Option<DataGroup> {
        self.stats.packets_seen += 1;
        if !constants::crc16_check(packet) {
            self.stats.crc_fail += 1;
            self.assembling = false;
            self.buffer.clear();
            return None;
        }
        self.stats.crc_ok += 1;

        let continuity = (packet[0] >> 4) & 0x03;
        let first_last = (packet[0] >> 2) & 0x03;
        let address = (((packet[0] as u16) & 0x03) << 8) | packet[1] as u16;
        let useful = (packet[2] & 0x7F) as usize;

        if address != self.address {
            self.stats.address_mismatch += 1;
            return None;
        }
        if useful + 5 > packet.len() {
            self.assembling = false;
            self.buffer.clear();
            return None;
        }
        let payload = &packet[3..3 + useful];

        if let Some(prev) = self.last_continuity
            && continuity != (prev + 1) % 4
            && first_last != 2
            && first_last != 3
        {
            debug!(
                expected = (prev + 1) % 4,
                got = continuity,
                "packet continuity gap"
            );
            self.assembling = false;
            self.buffer.clear();
        }
        self.last_continuity = Some(continuity);

        match first_last {
            2 => {
                // First packet of a series
                self.buffer.clear();
                self.buffer.extend_from_slice(payload);
                self.assembling = true;
                None
            }
            0 => {
                if self.assembling {
                    self.buffer.extend_from_slice(payload);
                }
                None
            }
            1 => {
                if self.assembling {
                    self.buffer.extend_from_slice(payload);
                    self.assembling = false;
                    self.stats.groups_complete += 1;
                    Some(DataGroup {
                        address,
                        bytes: std::mem::take(&mut self.buffer),
                    })
                } else {
                    None
                }
            }
            3 => {
                // Single packet = complete data group
                self.assembling = false;
                self.stats.groups_complete += 1;
                Some(DataGroup {
                    address,
                    bytes: payload.to_vec(),
                })
            }
            _ => None,
        }
    }
}

/// Parse an MSC data-group header and return the data-field bytes.
///
/// EN 300 401 §5.3.3.1: extension / CRC / segment / user-access flags in
/// byte 0, continuity+repetition in byte 1, then optional fields, then the
/// data field, then an optional 2-byte CRC when the CRC flag is set.
fn strip_data_group_header(bytes: &[u8]) -> &[u8] {
    if bytes.len() < 2 {
        return bytes;
    }
    let extension = bytes[0] & 0x80 != 0;
    let crc_flag = bytes[0] & 0x40 != 0;
    let segment = bytes[0] & 0x20 != 0;
    let user_access = bytes[0] & 0x10 != 0;
    let mut pos = 2;
    if extension {
        if pos >= bytes.len() {
            return bytes;
        }
        pos += 1;
    }
    if segment {
        pos += 2;
    }
    if user_access {
        if pos >= bytes.len() {
            return bytes;
        }
        let length_indicator = (bytes[pos] & 0x0F) as usize;
        pos += 1 + length_indicator;
    }
    if pos > bytes.len() {
        return bytes;
    }
    let end = if crc_flag && bytes.len() >= pos + 2 {
        bytes.len() - 2
    } else {
        bytes.len()
    };
    if pos > end {
        &bytes[pos.min(bytes.len())..]
    } else {
        &bytes[pos..end]
    }
}

/// Build a well-formed packet for tests (header + payload + padding + CRC).
#[cfg(test)]
pub fn encode_packet(
    length_id: u8,
    continuity: u8,
    first_last: u8,
    address: u16,
    payload: &[u8],
) -> Vec<u8> {
    let packet_len = PACKET_LENGTHS[length_id as usize];
    let mut pkt = vec![0u8; packet_len];
    pkt[0] = (length_id << 6)
        | ((continuity & 0x03) << 4)
        | ((first_last & 0x03) << 2)
        | ((address >> 8) as u8 & 0x03);
    pkt[1] = (address & 0xFF) as u8;
    pkt[2] = payload.len() as u8 & 0x7F;
    pkt[3..3 + payload.len()].copy_from_slice(payload);
    let crc = constants::crc16_ccitt(&pkt[..packet_len - 2]);
    pkt[packet_len - 2] = (crc >> 8) as u8;
    pkt[packet_len - 1] = (crc & 0xFF) as u8;
    pkt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_rejects_corrupt_packet() {
        let mut pkt = encode_packet(0, 0, 3, 852, b"hello");
        let mut asm = PacketAssembler::new(852);
        assert_eq!(asm.feed_bytes(&pkt).len(), 1);
        pkt[3] ^= 0xFF;
        assert!(asm.feed_bytes(&pkt).is_empty());
        assert!(asm.stats.crc_fail >= 1);
    }

    #[test]
    fn single_packet_data_group() {
        let pkt = encode_packet(0, 0, 3, 852, b"TPEG");
        let mut asm = PacketAssembler::new(852);
        let groups = asm.feed_bytes(&pkt);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].bytes, b"TPEG");
    }

    #[test]
    fn multi_packet_reassembly() {
        let first = encode_packet(0, 0, 2, 10, b"AAA");
        let mid = encode_packet(0, 1, 0, 10, b"BBB");
        let last = encode_packet(0, 2, 1, 10, b"CCC");
        let mut asm = PacketAssembler::new(10);
        let mut buf = first;
        buf.extend_from_slice(&mid);
        buf.extend_from_slice(&last);
        let groups = asm.feed_bytes(&buf);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].bytes, b"AAABBBCCC");
    }

    #[test]
    fn wrong_address_is_ignored() {
        let pkt = encode_packet(0, 0, 3, 1, b"x");
        let mut asm = PacketAssembler::new(852);
        assert!(asm.feed_bytes(&pkt).is_empty());
        assert!(asm.stats.address_mismatch >= 1);
        assert!(asm.stats.crc_ok >= 1);
    }

    #[test]
    fn chance_level_detection() {
        let mut stats = PacketStats {
            packets_seen: 100,
            crc_ok: 1,
            crc_fail: 99,
            ..Default::default()
        };
        assert!(stats.is_chance_level());
        stats.crc_ok = 80;
        stats.crc_fail = 20;
        assert!(!stats.is_chance_level());
    }
}
