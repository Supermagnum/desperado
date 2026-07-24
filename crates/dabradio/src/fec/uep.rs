//! Unequal Error Protection (UEP) depuncturing for legacy DAB audio.
//!
//! Short-form FIG 0/1 selects one of the standardized UEP profiles. A
//! profile divides each 24 ms logical frame into four regions, each using a
//! different puncturing vector, followed by the six-bit convolutional-code
//! termination punctured with `PI_TAIL`.
//!
//! Reference: ETSI EN 300 401 V2.1.1 (2017-01), clauses 11.1.2 and
//! 11.3.1, especially the independently transcribed protection profiles in
//! Table 15. The standard is publicly available from <https://www.etsi.org/>.

use super::eep::{P_CODES, PI_TAIL};

/// Standards-defined UEP channel-coding profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UepProfile {
    pub bitrate: u16,
    /// UEP protection level (1 strongest, 5 weakest).
    pub level: u8,
    /// Number of 128-coded-bit blocks in the four unequal regions.
    pub l: [usize; 4],
    /// Zero-based indices into `P_CODES`; `None` is valid only for a zero-length region.
    pub pi: [Option<usize>; 4],
}

/// All standardized UEP profiles used by FIG 0/1 short form.
pub const UEP_PROFILES: [UepProfile; 64] = [
    UepProfile {
        bitrate: 32,
        level: 5,
        l: [3, 4, 17, 0],
        pi: [Some(4), Some(2), Some(1), None],
    },
    UepProfile {
        bitrate: 32,
        level: 4,
        l: [3, 3, 18, 0],
        pi: [Some(10), Some(5), Some(4), None],
    },
    UepProfile {
        bitrate: 32,
        level: 3,
        l: [3, 4, 14, 3],
        pi: [Some(14), Some(8), Some(5), Some(7)],
    },
    UepProfile {
        bitrate: 32,
        level: 2,
        l: [3, 4, 14, 3],
        pi: [Some(21), Some(12), Some(7), Some(12)],
    },
    UepProfile {
        bitrate: 32,
        level: 1,
        l: [3, 5, 13, 3],
        pi: [Some(23), Some(16), Some(11), Some(16)],
    },
    UepProfile {
        bitrate: 48,
        level: 5,
        l: [4, 3, 26, 3],
        pi: [Some(4), Some(3), Some(1), Some(2)],
    },
    UepProfile {
        bitrate: 48,
        level: 4,
        l: [3, 4, 26, 3],
        pi: [Some(8), Some(5), Some(3), Some(5)],
    },
    UepProfile {
        bitrate: 48,
        level: 3,
        l: [3, 4, 26, 3],
        pi: [Some(14), Some(9), Some(5), Some(8)],
    },
    UepProfile {
        bitrate: 48,
        level: 2,
        l: [3, 4, 26, 3],
        pi: [Some(23), Some(13), Some(7), Some(14)],
    },
    UepProfile {
        bitrate: 48,
        level: 1,
        l: [3, 5, 25, 3],
        pi: [Some(23), Some(17), Some(12), Some(17)],
    },
    UepProfile {
        bitrate: 56,
        level: 5,
        l: [6, 10, 23, 3],
        pi: [Some(4), Some(3), Some(1), Some(2)],
    },
    UepProfile {
        bitrate: 56,
        level: 4,
        l: [6, 10, 23, 3],
        pi: [Some(8), Some(5), Some(3), Some(4)],
    },
    UepProfile {
        bitrate: 56,
        level: 3,
        l: [6, 12, 21, 3],
        pi: [Some(15), Some(6), Some(5), Some(8)],
    },
    UepProfile {
        bitrate: 56,
        level: 2,
        l: [6, 10, 23, 3],
        pi: [Some(22), Some(12), Some(7), Some(12)],
    },
    UepProfile {
        bitrate: 64,
        level: 5,
        l: [6, 9, 31, 2],
        pi: [Some(4), Some(2), Some(1), Some(2)],
    },
    UepProfile {
        bitrate: 64,
        level: 4,
        l: [6, 9, 33, 0],
        pi: [Some(10), Some(5), Some(4), None],
    },
    UepProfile {
        bitrate: 64,
        level: 3,
        l: [6, 12, 27, 3],
        pi: [Some(15), Some(7), Some(5), Some(8)],
    },
    UepProfile {
        bitrate: 64,
        level: 2,
        l: [6, 10, 29, 3],
        pi: [Some(22), Some(12), Some(7), Some(12)],
    },
    UepProfile {
        bitrate: 64,
        level: 1,
        l: [6, 11, 28, 3],
        pi: [Some(23), Some(17), Some(11), Some(17)],
    },
    UepProfile {
        bitrate: 80,
        level: 5,
        l: [6, 10, 41, 3],
        pi: [Some(5), Some(2), Some(1), Some(2)],
    },
    UepProfile {
        bitrate: 80,
        level: 4,
        l: [6, 10, 41, 3],
        pi: [Some(10), Some(5), Some(4), Some(5)],
    },
    UepProfile {
        bitrate: 80,
        level: 3,
        l: [6, 11, 40, 3],
        pi: [Some(15), Some(7), Some(5), Some(6)],
    },
    UepProfile {
        bitrate: 80,
        level: 2,
        l: [6, 10, 41, 3],
        pi: [Some(22), Some(12), Some(7), Some(12)],
    },
    UepProfile {
        bitrate: 80,
        level: 1,
        l: [6, 10, 41, 3],
        pi: [Some(23), Some(16), Some(11), Some(17)],
    },
    UepProfile {
        bitrate: 96,
        level: 5,
        l: [7, 9, 53, 3],
        pi: [Some(4), Some(3), Some(1), Some(3)],
    },
    UepProfile {
        bitrate: 96,
        level: 4,
        l: [7, 10, 52, 3],
        pi: [Some(8), Some(5), Some(3), Some(5)],
    },
    UepProfile {
        bitrate: 96,
        level: 3,
        l: [6, 12, 51, 3],
        pi: [Some(15), Some(8), Some(5), Some(9)],
    },
    UepProfile {
        bitrate: 96,
        level: 2,
        l: [6, 10, 53, 3],
        pi: [Some(21), Some(11), Some(8), Some(11)],
    },
    UepProfile {
        bitrate: 96,
        level: 1,
        l: [6, 13, 50, 3],
        pi: [Some(23), Some(17), Some(12), Some(18)],
    },
    UepProfile {
        bitrate: 112,
        level: 5,
        l: [14, 17, 50, 3],
        pi: [Some(4), Some(3), Some(1), Some(4)],
    },
    UepProfile {
        bitrate: 112,
        level: 4,
        l: [11, 21, 49, 3],
        pi: [Some(8), Some(5), Some(3), Some(7)],
    },
    UepProfile {
        bitrate: 112,
        level: 3,
        l: [11, 23, 47, 3],
        pi: [Some(15), Some(7), Some(5), Some(8)],
    },
    UepProfile {
        bitrate: 112,
        level: 2,
        l: [11, 21, 49, 3],
        pi: [Some(22), Some(11), Some(8), Some(13)],
    },
    UepProfile {
        bitrate: 128,
        level: 5,
        l: [12, 19, 62, 3],
        pi: [Some(4), Some(2), Some(1), Some(3)],
    },
    UepProfile {
        bitrate: 128,
        level: 4,
        l: [11, 21, 61, 3],
        pi: [Some(10), Some(5), Some(4), Some(6)],
    },
    UepProfile {
        bitrate: 128,
        level: 3,
        l: [11, 22, 60, 3],
        pi: [Some(15), Some(8), Some(5), Some(9)],
    },
    UepProfile {
        bitrate: 128,
        level: 2,
        l: [11, 21, 61, 3],
        pi: [Some(21), Some(11), Some(8), Some(13)],
    },
    UepProfile {
        bitrate: 128,
        level: 1,
        l: [11, 20, 62, 3],
        pi: [Some(23), Some(16), Some(12), Some(18)],
    },
    UepProfile {
        bitrate: 160,
        level: 5,
        l: [11, 19, 87, 3],
        pi: [Some(4), Some(3), Some(1), Some(3)],
    },
    UepProfile {
        bitrate: 160,
        level: 4,
        l: [11, 23, 83, 3],
        pi: [Some(10), Some(5), Some(4), Some(8)],
    },
    UepProfile {
        bitrate: 160,
        level: 3,
        l: [11, 24, 82, 3],
        pi: [Some(15), Some(7), Some(5), Some(10)],
    },
    UepProfile {
        bitrate: 160,
        level: 2,
        l: [11, 21, 85, 3],
        pi: [Some(21), Some(10), Some(8), Some(12)],
    },
    UepProfile {
        bitrate: 160,
        level: 1,
        l: [11, 22, 84, 3],
        pi: [Some(23), Some(17), Some(11), Some(18)],
    },
    UepProfile {
        bitrate: 192,
        level: 5,
        l: [11, 20, 110, 3],
        pi: [Some(5), Some(3), Some(1), Some(4)],
    },
    UepProfile {
        bitrate: 192,
        level: 4,
        l: [11, 22, 108, 3],
        pi: [Some(9), Some(5), Some(3), Some(8)],
    },
    UepProfile {
        bitrate: 192,
        level: 3,
        l: [11, 24, 106, 3],
        pi: [Some(15), Some(9), Some(5), Some(10)],
    },
    UepProfile {
        bitrate: 192,
        level: 2,
        l: [11, 20, 110, 3],
        pi: [Some(21), Some(12), Some(8), Some(12)],
    },
    UepProfile {
        bitrate: 192,
        level: 1,
        l: [11, 21, 109, 3],
        pi: [Some(23), Some(19), Some(12), Some(23)],
    },
    UepProfile {
        bitrate: 224,
        level: 5,
        l: [12, 22, 131, 3],
        pi: [Some(7), Some(5), Some(1), Some(5)],
    },
    UepProfile {
        bitrate: 224,
        level: 4,
        l: [12, 26, 127, 3],
        pi: [Some(11), Some(7), Some(3), Some(10)],
    },
    UepProfile {
        bitrate: 224,
        level: 3,
        l: [11, 20, 134, 3],
        pi: [Some(15), Some(9), Some(6), Some(8)],
    },
    UepProfile {
        bitrate: 224,
        level: 2,
        l: [11, 22, 132, 3],
        pi: [Some(23), Some(15), Some(9), Some(14)],
    },
    UepProfile {
        bitrate: 224,
        level: 1,
        l: [11, 24, 130, 3],
        pi: [Some(23), Some(19), Some(11), Some(19)],
    },
    UepProfile {
        bitrate: 256,
        level: 5,
        l: [11, 24, 154, 3],
        pi: [Some(5), Some(4), Some(1), Some(4)],
    },
    UepProfile {
        bitrate: 256,
        level: 4,
        l: [11, 24, 154, 3],
        pi: [Some(11), Some(8), Some(4), Some(9)],
    },
    UepProfile {
        bitrate: 256,
        level: 3,
        l: [11, 27, 151, 3],
        pi: [Some(15), Some(9), Some(6), Some(9)],
    },
    UepProfile {
        bitrate: 256,
        level: 2,
        l: [11, 22, 156, 3],
        pi: [Some(23), Some(13), Some(9), Some(12)],
    },
    UepProfile {
        bitrate: 256,
        level: 1,
        l: [11, 26, 152, 3],
        pi: [Some(23), Some(18), Some(13), Some(17)],
    },
    UepProfile {
        bitrate: 320,
        level: 5,
        l: [11, 26, 200, 3],
        pi: [Some(7), Some(4), Some(1), Some(5)],
    },
    UepProfile {
        bitrate: 320,
        level: 4,
        l: [11, 25, 201, 3],
        pi: [Some(12), Some(8), Some(4), Some(9)],
    },
    UepProfile {
        bitrate: 320,
        level: 2,
        l: [11, 26, 200, 3],
        pi: [Some(23), Some(16), Some(8), Some(16)],
    },
    UepProfile {
        bitrate: 384,
        level: 5,
        l: [11, 27, 247, 3],
        pi: [Some(7), Some(5), Some(1), Some(6)],
    },
    UepProfile {
        bitrate: 384,
        level: 3,
        l: [11, 24, 250, 3],
        pi: [Some(15), Some(8), Some(6), Some(9)],
    },
    UepProfile {
        bitrate: 384,
        level: 1,
        l: [12, 28, 245, 3],
        pi: [Some(23), Some(19), Some(13), Some(22)],
    },
];

/// Find the UEP channel-coding profile for a parsed short-form subchannel.
pub fn profile(bitrate: u16, level: u8) -> Option<UepProfile> {
    UEP_PROFILES
        .iter()
        .copied()
        .find(|p| p.bitrate == bitrate && p.level == level)
}

fn punctured_bits_for_region(blocks: usize, pi: usize) -> usize {
    let ones = P_CODES[pi].iter().map(|&x| x as usize).sum::<usize>();
    blocks * 4 * ones
}

/// Number of transmitted soft bits consumed by a UEP profile.
pub fn punctured_size(p: &UepProfile) -> usize {
    p.l.iter()
        .zip(p.pi.iter())
        .map(|(&blocks, &pi)| {
            if blocks == 0 {
                0
            } else {
                punctured_bits_for_region(blocks, pi.expect("non-empty UEP region needs PI"))
            }
        })
        .sum::<usize>()
        + PI_TAIL.iter().map(|&x| x as usize).sum::<usize>()
}

/// Mother-code soft-bit count after depuncturing, including the 24 tail bits.
pub fn depunctured_size(p: &UepProfile) -> usize {
    p.l.iter().sum::<usize>() * 128 + 24
}

/// Depuncture one UEP-protected MSC logical frame.
pub fn depuncture(soft_bits: &[i8], p: &UepProfile) -> Vec<i8> {
    let mut out = Vec::with_capacity(depunctured_size(p));
    let mut read = 0usize;

    for (&blocks, &pi) in p.l.iter().zip(p.pi.iter()) {
        if blocks == 0 {
            continue;
        }
        let pattern = &P_CODES[pi.expect("non-empty UEP region needs PI")];
        for _ in 0..blocks {
            for _ in 0..4 {
                for &keep in pattern {
                    if keep != 0 {
                        out.push(soft_bits.get(read).copied().unwrap_or(0));
                        read += 1;
                    } else {
                        out.push(0);
                    }
                }
            }
        }
    }

    for &keep in &PI_TAIL {
        if keep != 0 {
            out.push(soft_bits.get(read).copied().unwrap_or(0));
            read += 1;
        } else {
            out.push(0);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_profiles_have_consistent_information_and_allocation_sizes() {
        for p in UEP_PROFILES {
            // Four mother-code bits are produced per information bit.
            assert_eq!(p.l.iter().sum::<usize>() * 128, 24 * p.bitrate as usize * 4);
            for (&blocks, &pi) in p.l.iter().zip(p.pi.iter()) {
                assert_eq!(blocks == 0, pi.is_none(), "{p:?}");
            }

            // Every standardized short-form allocation rounds to whole CUs
            // with only the 0/4/8 trailing padding bits allowed by Table 15.
            let transmitted = punctured_size(&p);
            let allocation = transmitted.div_ceil(64) * 64;
            assert!(matches!(allocation - transmitted, 0 | 4 | 8), "{p:?}");
        }
    }

    #[test]
    fn bbc_radio_one_profile_matches_table_index_35() {
        // FIG 0/1 short-form index 35: 96 CUs, 128 kbps, UEP 3.
        let p = profile(128, 3).unwrap();
        // This standardized row leaves four padding bits in its 96-CU allocation.
        assert_eq!(punctured_size(&p), 96 * 64 - 4);
        assert_eq!(depunctured_size(&p), 24 * 128 * 4 + 24);
    }

    #[test]
    fn depuncture_consumes_bbc_profile_and_produces_mother_code_size() {
        let p = profile(128, 3).unwrap();
        let input = vec![42; punctured_size(&p)];
        let output = depuncture(&input, &p);
        assert_eq!(output.len(), depunctured_size(&p));
        assert_eq!(output.iter().filter(|&&x| x != 0).count(), input.len());
    }
}
