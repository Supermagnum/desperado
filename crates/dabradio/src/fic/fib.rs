//! FIB (Fast Information Block) parser.
//!
//! Parses FIG (Fast Information Group) types from FIBs to extract:
//! - Ensemble info (FIG 0/0): ensemble ID, country, change flags
//! - Subchannel config (FIG 0/1): subchannel ID, start address, size, protection
//! - Service-component mapping (FIG 0/2): service ID → component → subchannel
//! - Service labels (FIG 1/0): 16-char labels for services
//! - Ensemble label (FIG 1/0 with OE=0, ext=0): ensemble name
//!
//! Reference: ETSI EN 300 401 §5.2, §6, §8

use crate::charsets;
use serde::Serialize;
use std::collections::HashMap;

/// Accumulated ensemble information from FIBs.
#[derive(Debug, Default, Clone)]
pub struct EnsembleInfo {
    pub ensemble_id: Option<u16>,
    pub ensemble_label: Option<String>,
    pub services: HashMap<u32, ServiceInfo>,
    pub subchannels: HashMap<u8, SubchannelInfo>,
}

/// Audio coding signalled by FIG 0/2 ASCTy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum AudioCoding {
    /// ASCTy 0: legacy DAB MPEG-1/2 Audio Layer II.
    MpegLayer2,
    /// ASCTy 63: DAB+ HE-AAC.
    DabPlus,
    /// A stream-audio component type not yet supported.
    Other(u8),
}

impl AudioCoding {
    fn from_ascty(ascty: u8) -> Self {
        match ascty {
            0 => Self::MpegLayer2,
            63 => Self::DabPlus,
            other => Self::Other(other),
        }
    }
}

/// Information about a single DAB service.
#[derive(Debug, Default, Clone, Serialize)]
pub struct ServiceInfo {
    pub service_id: u32,
    pub label: Option<String>,
    pub subchannel_id: Option<u8>,
    pub is_audio: bool,
    pub audio_coding: Option<AudioCoding>,
    pub bitrate: Option<u16>,
    pub protection: Option<String>,
}

/// Subchannel configuration.
#[derive(Debug, Default, Clone, Serialize)]
pub struct SubchannelInfo {
    pub id: u8,
    pub start_addr: u16,
    pub sub_size: u16, // in CUs
    pub protection_level: u8,
    pub is_eep: bool,
    pub eep_option: u8, // 0 = EEP-A, 1 = EEP-B (only meaningful when is_eep=true)
    /// FIG 0/1 short-form UEP table index; absent for EEP long form.
    pub uep_table_index: Option<u8>,
    pub bitrate: u16,
}

/// JSON output format for ensemble listing.
#[derive(Debug, Serialize)]
pub struct EnsembleOutput {
    pub ensemble_id: Option<u16>,
    pub ensemble_label: Option<String>,
    pub services: Vec<ServiceOutput>,
}

#[derive(Debug, Serialize)]
pub struct ServiceOutput {
    pub service_id: String,
    pub label: Option<String>,
    pub subchannel_id: Option<u8>,
    pub bitrate: Option<u16>,
    pub protection: Option<String>,
}

impl EnsembleInfo {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse a FIB (30 data bytes, CRC already validated).
    pub fn parse_fib(&mut self, fib: &[u8; 32]) {
        let data = &fib[..30]; // 30 data bytes (CRC in bytes 30-31)
        let mut pos = 0;

        while pos < 30 {
            // Each FIG starts with a type byte
            let fig_type = (data[pos] >> 5) & 0x07;
            let fig_length = (data[pos] & 0x1F) as usize;

            if fig_type == 7 || fig_length == 0 {
                break; // End marker or padding
            }

            if pos + 1 + fig_length > 30 {
                break; // Malformed
            }

            let fig_data = &data[pos + 1..pos + 1 + fig_length];

            match fig_type {
                0 => self.parse_fig_type0(fig_data),
                1 => self.parse_fig_type1(fig_data),
                _ => {} // Other FIG types not needed for service listing
            }

            pos += 1 + fig_length;
        }
    }

    /// Parse FIG type 0 (Multiplex Configuration Information).
    fn parse_fig_type0(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        let cn = (data[0] >> 7) & 1; // C/N flag
        let _oe = (data[0] >> 6) & 1; // Other Ensemble
        let _pd = (data[0] >> 5) & 1; // P/D flag (0=16-bit SId, 1=32-bit)
        let extension = data[0] & 0x1F;
        let _ = cn; // suppress unused warning

        let fig_data = &data[1..];

        match extension {
            0 => self.parse_fig0_ext0(fig_data),
            1 => self.parse_fig0_ext1(fig_data),
            2 => self.parse_fig0_ext2(fig_data, (data[0] >> 5) & 1),
            _ => {}
        }
    }

    /// FIG 0/0: Ensemble information.
    /// Contains Ensemble ID (EId), change flags, AL flag, CIF count.
    fn parse_fig0_ext0(&mut self, data: &[u8]) {
        if data.len() < 4 {
            return;
        }
        let eid = ((data[0] as u16) << 8) | data[1] as u16;
        self.ensemble_id = Some(eid);
    }

    /// FIG 0/1: Sub-channel organization (basic subchannel info).
    /// Each entry: SubChId (6 bits), Start Address (10 bits), form flag (1 bit), then details.
    fn parse_fig0_ext1(&mut self, data: &[u8]) {
        let mut pos = 0;
        while pos + 3 <= data.len() {
            let subch_id = (data[pos] >> 2) & 0x3F;
            let start_addr = ((data[pos] as u16 & 0x03) << 8) | data[pos + 1] as u16;
            let form = (data[pos + 2] >> 7) & 1;

            if form == 0 {
                // Short form: table-driven (UEP). Table Switch=1 is
                // reserved for future use and its low six bits must not be
                // interpreted using the current table.
                let table_switch = (data[pos + 2] >> 6) & 1;
                if table_switch != 0 {
                    pos += 3;
                    continue;
                }
                let table_index = data[pos + 2] & 0x3F;
                let (sub_size, bitrate, prot_level) = uep_table(table_index);
                self.subchannels.insert(
                    subch_id,
                    SubchannelInfo {
                        id: subch_id,
                        start_addr,
                        sub_size,
                        protection_level: prot_level,
                        is_eep: false,
                        eep_option: 0,
                        uep_table_index: Some(table_index),
                        bitrate,
                    },
                );
                pos += 3;
            } else {
                // Long form: EEP
                if pos + 4 > data.len() {
                    break;
                }
                let option = (data[pos + 2] >> 4) & 0x07;
                let prot_level = (data[pos + 2] >> 2) & 0x03;
                let sub_size = ((data[pos + 2] as u16 & 0x03) << 8) | data[pos + 3] as u16;
                let bitrate = eep_bitrate(sub_size, option, prot_level);
                self.subchannels.insert(
                    subch_id,
                    SubchannelInfo {
                        id: subch_id,
                        start_addr,
                        sub_size,
                        protection_level: prot_level,
                        is_eep: true,
                        eep_option: option,
                        uep_table_index: None,
                        bitrate,
                    },
                );
                pos += 4;
            }
        }
    }

    /// FIG 0/2: Service organization.
    /// Maps service IDs to their components (subchannel references).
    fn parse_fig0_ext2(&mut self, data: &[u8], pd: u8) {
        let mut pos = 0;
        while pos < data.len() {
            let (sid, sid_len) = if pd == 0 {
                // 16-bit service ID (audio)
                if pos + 2 > data.len() {
                    break;
                }
                let sid = ((data[pos] as u32) << 8) | data[pos + 1] as u32;
                (sid, 2)
            } else {
                // 32-bit service ID (data)
                if pos + 4 > data.len() {
                    break;
                }
                let sid = ((data[pos] as u32) << 24)
                    | ((data[pos + 1] as u32) << 16)
                    | ((data[pos + 2] as u32) << 8)
                    | data[pos + 3] as u32;
                (sid, 4)
            };
            pos += sid_len;

            if pos >= data.len() {
                break;
            }

            let _local_flag = (data[pos] >> 7) & 1;
            let num_components = data[pos] & 0x0F;
            pos += 1;

            for _ in 0..num_components {
                if pos + 2 > data.len() {
                    break;
                }
                let tmid = (data[pos] >> 6) & 0x03;
                let is_audio = tmid == 0; // 0 = MSC stream audio
                let ascty = data[pos] & 0x3F;

                // FIG 0/2 component descriptor (ETSI EN 300 401 §6.3.1):
                //   Byte 0: TMId(2) | ASCTy/DSCTy(6)
                //   Byte 1: SubChId(6) | PS(1) | CA(1)
                let subch_id = if tmid == 0 || tmid == 1 {
                    Some((data[pos + 1] >> 2) & 0x3F)
                } else {
                    None
                };

                let service = self.services.entry(sid).or_insert_with(|| ServiceInfo {
                    service_id: sid,
                    ..Default::default()
                });
                let primary = (data[pos + 1] >> 1) & 1 != 0;
                // A service can have audio plus data/secondary components. Never
                // let a later data descriptor clear the selected audio codec;
                // prefer the primary audio component when one is signalled.
                if is_audio && (service.audio_coding.is_none() || primary) {
                    service.is_audio = true;
                    service.audio_coding = Some(AudioCoding::from_ascty(ascty));
                    if let Some(sc_id) = subch_id {
                        service.subchannel_id = Some(sc_id);
                    }
                }

                pos += 2;
            }
        }
    }

    /// Parse FIG type 1 (Labels).
    fn parse_fig_type1(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        let charset = (data[0] >> 4) & 0x0F;
        let _oe = (data[0] >> 3) & 1;
        let extension = data[0] & 0x07;

        let fig_data = &data[1..];

        match extension {
            0 => self.parse_fig1_ext0(fig_data, charset),
            1 => self.parse_fig1_ext1(fig_data, charset),
            _ => {}
        }
    }

    /// FIG 1/0: Ensemble label.
    fn parse_fig1_ext0(&mut self, data: &[u8], charset: u8) {
        if data.len() < 18 {
            return; // 2 bytes EId + 16 chars + ... (may have char flag)
        }
        // Skip 2-byte Ensemble ID
        let label_bytes = &data[2..18];
        self.ensemble_label = Some(decode_label(label_bytes, charset));
    }

    /// FIG 1/1: Service label (16-bit SId).
    fn parse_fig1_ext1(&mut self, data: &[u8], charset: u8) {
        if data.len() < 18 {
            return; // 2 bytes SId + 16 chars
        }
        let sid = ((data[0] as u32) << 8) | data[1] as u32;
        let label_bytes = &data[2..18];
        let label = decode_label(label_bytes, charset);

        let service = self.services.entry(sid).or_insert_with(|| ServiceInfo {
            service_id: sid,
            ..Default::default()
        });
        service.label = Some(label);
    }

    /// Resolve subchannel info (bitrate, protection) into services.
    pub fn resolve_services(&mut self) {
        let subchannels = self.subchannels.clone();
        for service in self.services.values_mut() {
            if let Some(sc_id) = service.subchannel_id
                && let Some(subch) = subchannels.get(&sc_id)
            {
                service.bitrate = Some(subch.bitrate);
                service.protection = Some(if subch.is_eep {
                    let option_letter = if subch.eep_option == 0 { "A" } else { "B" };
                    format!("EEP {}-{}", subch.protection_level + 1, option_letter)
                } else {
                    format!("UEP {}", subch.protection_level)
                });
            }
        }
    }

    /// Convert to JSON output format.
    pub fn to_output(&self) -> EnsembleOutput {
        let mut services: Vec<ServiceOutput> = self
            .services
            .values()
            .map(|s| ServiceOutput {
                service_id: format!("0x{:04X}", s.service_id),
                label: s.label.clone(),
                subchannel_id: s.subchannel_id,
                bitrate: s.bitrate,
                protection: s.protection.clone(),
            })
            .collect();
        services.sort_by(|a, b| a.service_id.cmp(&b.service_id));

        EnsembleOutput {
            ensemble_id: self.ensemble_id,
            ensemble_label: self.ensemble_label.clone(),
            services,
        }
    }

    /// Check if we have at least one service with a known subchannel.
    pub fn has_services(&self) -> bool {
        !self.services.is_empty() && self.services.values().any(|s| s.subchannel_id.is_some())
    }

    /// Check if the ensemble info looks complete:
    /// - ensemble label is known
    /// - all services that have a subchannel also have a label and resolved bitrate
    pub fn is_complete(&self) -> bool {
        if self.ensemble_label.is_none() {
            return false;
        }
        if self.services.is_empty() {
            return false;
        }
        // Every service with a subchannel must have a label and bitrate
        self.services.values().all(|s| {
            if let Some(sid) = s.subchannel_id {
                s.label.is_some() && self.subchannels.contains_key(&sid)
            } else {
                // Services without subchannel (data services) — just need a label
                s.label.is_some()
            }
        })
    }
}

/// Decode a 16-byte label field to a UTF-8 string.
fn decode_label(bytes: &[u8], charset: u8) -> String {
    let bytes = trim_label_padding(bytes);
    match charset {
        0 => charsets::ebu_latin_to_utf8(bytes).trim_end().to_string(),
        // charset 6 = UTF-8
        6 => String::from_utf8_lossy(bytes).trim_end().to_string(),
        // Others: fallback to lossy UTF-8
        _ => String::from_utf8_lossy(bytes).trim_end().to_string(),
    }
}

fn trim_label_padding(bytes: &[u8]) -> &[u8] {
    let mut end = bytes.len();
    while end > 0 && matches!(bytes[end - 1], 0x00 | 0x20 | 0xFF) {
        end -= 1;
    }
    &bytes[..end]
}

/// UEP (Unequal Error Protection) table lookup.
/// Returns (sub_size in CUs, bitrate in kbps, protection_level).
/// Reference: ETSI EN 300 401 Table 6.
fn uep_table(index: u8) -> (u16, u16, u8) {
    // Complete ETSI EN 300 401 FIG 0/1 short-form table. UEP level 1
    // is strongest and level 5 weakest. Keep all 64 rows: omitting rows
    // shifts every later on-air index and selects the wrong bitrate/FEC profile.
    const TABLE: [(u16, u16, u8); 64] = [
        (16, 32, 5),
        (21, 32, 4),
        (24, 32, 3),
        (29, 32, 2),
        (35, 32, 1),
        (24, 48, 5),
        (29, 48, 4),
        (35, 48, 3),
        (42, 48, 2),
        (52, 48, 1),
        (29, 56, 5),
        (35, 56, 4),
        (42, 56, 3),
        (52, 56, 2),
        (32, 64, 5),
        (42, 64, 4),
        (48, 64, 3),
        (58, 64, 2),
        (70, 64, 1),
        (40, 80, 5),
        (52, 80, 4),
        (58, 80, 3),
        (70, 80, 2),
        (84, 80, 1),
        (48, 96, 5),
        (58, 96, 4),
        (70, 96, 3),
        (84, 96, 2),
        (104, 96, 1),
        (58, 112, 5),
        (70, 112, 4),
        (84, 112, 3),
        (104, 112, 2),
        (64, 128, 5),
        (84, 128, 4),
        (96, 128, 3),
        (116, 128, 2),
        (140, 128, 1),
        (80, 160, 5),
        (104, 160, 4),
        (116, 160, 3),
        (140, 160, 2),
        (168, 160, 1),
        (96, 192, 5),
        (116, 192, 4),
        (140, 192, 3),
        (168, 192, 2),
        (208, 192, 1),
        (116, 224, 5),
        (140, 224, 4),
        (168, 224, 3),
        (208, 224, 2),
        (232, 224, 1),
        (128, 256, 5),
        (168, 256, 4),
        (192, 256, 3),
        (232, 256, 2),
        (280, 256, 1),
        (160, 320, 5),
        (208, 320, 4),
        (280, 320, 2),
        (192, 384, 5),
        (280, 384, 3),
        (416, 384, 1),
    ];
    TABLE[index as usize]
}

/// Compute EEP bitrate from sub-channel size, option, and protection level.
fn eep_bitrate(sub_size: u16, option: u8, protection_level: u8) -> u16 {
    // EEP: bitrate depends on sub_size and code rate
    // Option A: code rates from table
    // Option B: different code rates
    let n = sub_size as u32;
    let bitrate = match option {
        0 => match protection_level {
            // EEP-A
            0 => n * 8 / 12, // 1-A
            1 => n * 8 / 8,  // 2-A
            2 => n * 8 / 6,  // 3-A
            3 => n * 8 / 4,  // 4-A
            _ => 0,
        },
        1 => match protection_level {
            // EEP-B
            0 => n * 32 / 27, // 1-B
            1 => n * 32 / 21, // 2-B
            2 => n * 32 / 18, // 3-B
            3 => n * 32 / 15, // 4-B
            _ => 0,
        },
        _ => 0,
    };
    bitrate as u16
}

#[cfg(test)]
mod tests {
    use super::{AudioCoding, EnsembleInfo, decode_label, uep_table};
    use crate::fec::uep::{profile, punctured_size};

    #[test]
    fn decode_label_trims_nul_padding_before_ebu_conversion() {
        let label = decode_label(b"BORDEAUX 8C\0\0\0\0\0", 0);
        assert_eq!(label, "BORDEAUX 8C");
    }

    #[test]
    fn decode_label_trims_ff_padding_before_ebu_conversion() {
        let label = decode_label(b"NOVA\xFF\xFF\xFF\xFF", 0);
        assert_eq!(label, "NOVA");
    }

    #[test]
    fn decode_label_keeps_internal_reserved_bytes_visible() {
        let label = decode_label(b"A\0B\0\0", 0);
        assert_eq!(label, "A�B");
    }

    #[test]
    fn complete_uep_table_keeps_profiles_and_allocations_aligned() {
        assert_eq!(uep_table(35), (96, 128, 3));
        assert_eq!(uep_table(63), (416, 384, 1));
        for index in 0..64 {
            let (capacity_units, bitrate, level) = uep_table(index);
            let coding = profile(bitrate, level).unwrap();
            let padding = capacity_units as usize * 64 - punctured_size(&coding);
            assert!(matches!(padding, 0 | 4 | 8), "table index {index}");
        }
    }

    #[test]
    fn audio_component_type_selects_codec() {
        assert_eq!(AudioCoding::from_ascty(0), AudioCoding::MpegLayer2);
        assert_eq!(AudioCoding::from_ascty(63), AudioCoding::DabPlus);
        assert_eq!(AudioCoding::from_ascty(7), AudioCoding::Other(7));
    }

    #[test]
    fn short_form_rejects_reserved_table_switch() {
        let mut decoder = EnsembleInfo::new();
        decoder.parse_fig0_ext1(&[0, 0, 0x40 | 35]);
        assert!(decoder.subchannels.is_empty());

        decoder.parse_fig0_ext1(&[0, 0, 35]);
        let subchannel = decoder.subchannels.get(&0).unwrap();
        assert_eq!(subchannel.bitrate, 128);
        assert_eq!(subchannel.protection_level, 3);
    }

    #[test]
    fn data_component_does_not_clear_primary_audio_codec() {
        let mut decoder = EnsembleInfo::new();
        // Service 0x1234, two components: primary ASCTy 0 on subchannel 1,
        // followed by a packet-data component.
        decoder.parse_fig0_ext2(&[0x12, 0x34, 2, 0, (1 << 2) | 2, 3 << 6, 0], 0);
        let service = decoder.services.get(&0x1234).unwrap();
        assert!(service.is_audio);
        assert_eq!(service.subchannel_id, Some(1));
        assert_eq!(service.audio_coding, Some(AudioCoding::MpegLayer2));
    }

    #[test]
    fn primary_audio_component_supersedes_secondary() {
        let mut decoder = EnsembleInfo::new();
        // Secondary MP2 followed by primary DAB+.
        decoder.parse_fig0_ext2(&[0x12, 0x34, 2, 0, 1 << 2, 63, (2 << 2) | 2], 0);
        let service = decoder.services.get(&0x1234).unwrap();
        assert_eq!(service.subchannel_id, Some(2));
        assert_eq!(service.audio_coding, Some(AudioCoding::DabPlus));
    }
}
