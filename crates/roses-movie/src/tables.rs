//! MPEG-2 video tables (ISO/IEC 13818-2 Annex B and clause 7) and a
//! two-level variable-length-code lookup built from them at first use.
//! Codes are written as bit strings, exactly as the standard prints them.
use crate::bits::Bits;
use std::sync::OnceLock;

#[derive(Clone, Copy, Default)]
struct Entry {
    value: i32,
    /// Code length for a leaf; 0 for an empty slot or a subtable pointer.
    len: u8,
    /// Index bits of the subtable this slot points to (0 = not a pointer).
    sub_bits: u8,
}

pub struct Vlc {
    first_bits: u32,
    table: Vec<Entry>,
}

impl Vlc {
    /// Build from (code, value) pairs. Panics if two codes overlap (a prefix
    /// of one another), so table typos are caught by the unit tests.
    pub fn build(first_bits: u32, codes: &[(&str, i32)]) -> Self {
        let parsed: Vec<(u32, u32, i32)> = codes
            .iter()
            .map(|(s, v)| {
                let bits: String = s.chars().filter(|c| !c.is_whitespace()).collect();
                assert!(!bits.is_empty() && bits.len() <= 24, "bad code {s}");
                (u32::from_str_radix(&bits, 2).unwrap(), bits.len() as u32, *v)
            })
            .collect();
        let mut table = vec![Entry::default(); 1 << first_bits];
        // Group long codes by their first `first_bits` bits.
        let mut groups: std::collections::BTreeMap<u32, u32> = Default::default();
        for &(code, len, _) in &parsed {
            if len > first_bits {
                let prefix = code >> (len - first_bits);
                let e = groups.entry(prefix).or_insert(0);
                *e = (*e).max(len - first_bits);
            }
        }
        for (&prefix, &sub_bits) in &groups {
            let at = table.len();
            table.resize(at + (1 << sub_bits), Entry::default());
            table[prefix as usize] = Entry {
                value: at as i32,
                len: 0,
                sub_bits: sub_bits as u8,
            };
        }
        for &(code, len, value) in &parsed {
            let (slot_base, span_bits, total_len) = if len <= first_bits {
                (0usize, first_bits - len, len)
            } else {
                let prefix = code >> (len - first_bits);
                let ptr = table[prefix as usize];
                let rest = len - first_bits;
                let sub_code = code & ((1 << rest) - 1);
                let base = ptr.value as usize;
                let span = u32::from(ptr.sub_bits) - rest;
                for k in 0..(1usize << span) {
                    let slot = base + ((sub_code as usize) << span) + k;
                    assert!(table[slot].len == 0, "overlapping VLC code {code:b}/{len}");
                    table[slot] = Entry {
                        value,
                        len: len as u8,
                        sub_bits: 0,
                    };
                }
                continue;
            };
            let _ = total_len;
            for k in 0..(1usize << span_bits) {
                let slot = slot_base + ((code as usize) << span_bits) + k;
                assert!(
                    table[slot].len == 0 && table[slot].sub_bits == 0,
                    "overlapping VLC code {code:b}/{len}"
                );
                table[slot] = Entry {
                    value,
                    len: len as u8,
                    sub_bits: 0,
                };
            }
        }
        Self { first_bits, table }
    }

    /// Decode one code; `None` for a bit pattern that is not in the table.
    #[inline]
    pub fn decode(&self, b: &mut Bits) -> Option<i32> {
        let e = self.table[b.peek(self.first_bits) as usize];
        if e.len != 0 {
            b.skip(u32::from(e.len));
            return Some(e.value);
        }
        if e.sub_bits == 0 {
            return None;
        }
        let both = self.first_bits + u32::from(e.sub_bits);
        let idx = b.peek(both) & ((1 << e.sub_bits) - 1);
        let leaf = self.table[e.value as usize + idx as usize];
        if leaf.len == 0 {
            return None;
        }
        b.skip(u32::from(leaf.len));
        Some(leaf.value)
    }

    /// Sum of 2^-len over all codes (1.0 for a complete prefix code).
    #[cfg(test)]
    pub fn kraft(codes: &[(&str, i32)]) -> f64 {
        codes
            .iter()
            .map(|(s, _)| 0.5f64.powi(s.chars().filter(|c| !c.is_whitespace()).count() as i32))
            .sum()
    }
}

// --- Macroblock-level tables -------------------------------------------------

/// Table B-1, macroblock_address_increment (escape handled by the caller).
pub const MB_ADDR_INC: &[(&str, i32)] = &[
    ("1", 1), ("011", 2), ("010", 3), ("0011", 4), ("0010", 5), ("0001 1", 6),
    ("0001 0", 7), ("0000 111", 8), ("0000 110", 9), ("0000 1011", 10),
    ("0000 1010", 11), ("0000 1001", 12), ("0000 1000", 13), ("0000 0111", 14),
    ("0000 0110", 15), ("0000 0101 11", 16), ("0000 0101 10", 17),
    ("0000 0101 01", 18), ("0000 0101 00", 19), ("0000 0100 11", 20),
    ("0000 0100 10", 21), ("0000 0100 011", 22), ("0000 0100 010", 23),
    ("0000 0100 001", 24), ("0000 0100 000", 25), ("0000 0011 111", 26),
    ("0000 0011 110", 27), ("0000 0011 101", 28), ("0000 0011 100", 29),
    ("0000 0011 011", 30), ("0000 0011 010", 31), ("0000 0011 001", 32),
    ("0000 0011 000", 33),
];

pub const MB_QUANT: i32 = 1 << 4;
pub const MB_FWD: i32 = 1 << 3;
pub const MB_BWD: i32 = 1 << 2;
pub const MB_PATTERN: i32 = 1 << 1;
pub const MB_INTRA: i32 = 1;

/// Table B-2, macroblock_type in I pictures.
pub const MB_TYPE_I: &[(&str, i32)] = &[("1", MB_INTRA), ("01", MB_QUANT | MB_INTRA)];
/// Table B-3, P pictures.
pub const MB_TYPE_P: &[(&str, i32)] = &[
    ("1", MB_FWD | MB_PATTERN),
    ("01", MB_PATTERN),
    ("001", MB_FWD),
    ("0001 1", MB_INTRA),
    ("0001 0", MB_QUANT | MB_FWD | MB_PATTERN),
    ("0000 1", MB_QUANT | MB_PATTERN),
    ("0000 01", MB_QUANT | MB_INTRA),
];
/// Table B-4, B pictures.
pub const MB_TYPE_B: &[(&str, i32)] = &[
    ("10", MB_FWD | MB_BWD),
    ("11", MB_FWD | MB_BWD | MB_PATTERN),
    ("010", MB_BWD),
    ("011", MB_BWD | MB_PATTERN),
    ("0010", MB_FWD),
    ("0011", MB_FWD | MB_PATTERN),
    ("0001 1", MB_INTRA),
    ("0001 0", MB_QUANT | MB_FWD | MB_BWD | MB_PATTERN),
    ("0000 11", MB_QUANT | MB_FWD | MB_PATTERN),
    ("0000 10", MB_QUANT | MB_BWD | MB_PATTERN),
    ("0000 01", MB_QUANT | MB_INTRA),
];

/// Table B-9, coded_block_pattern (4:2:0), value = cbp.
pub const CBP: &[(&str, i32)] = &[
    ("111", 60), ("1101", 4), ("1100", 8), ("1011", 16), ("1010", 32),
    ("1001 1", 12), ("1001 0", 48), ("1000 1", 20), ("1000 0", 40),
    ("0111 1", 28), ("0111 0", 44), ("0110 1", 52), ("0110 0", 56),
    ("0101 1", 1), ("0101 0", 61), ("0100 1", 2), ("0100 0", 62),
    ("0011 11", 24), ("0011 10", 36), ("0011 01", 3), ("0011 00", 63),
    ("0010 111", 5), ("0010 110", 9), ("0010 101", 17), ("0010 100", 33),
    ("0010 011", 6), ("0010 010", 10), ("0010 001", 18), ("0010 000", 34),
    ("0001 1111", 7), ("0001 1110", 11), ("0001 1101", 19), ("0001 1100", 35),
    ("0001 1011", 13), ("0001 1010", 49), ("0001 1001", 21), ("0001 1000", 41),
    ("0001 0111", 14), ("0001 0110", 50), ("0001 0101", 22), ("0001 0100", 42),
    ("0001 0011", 15), ("0001 0010", 51), ("0001 0001", 23), ("0001 0000", 43),
    ("0000 1111", 25), ("0000 1110", 37), ("0000 1101", 26), ("0000 1100", 38),
    ("0000 1011", 29), ("0000 1010", 45), ("0000 1001", 53), ("0000 1000", 57),
    ("0000 0111", 30), ("0000 0110", 46), ("0000 0101", 54), ("0000 0100", 58),
    ("0000 0011 1", 31), ("0000 0011 0", 47), ("0000 0010 1", 55),
    ("0000 0010 0", 59), ("0000 0001 1", 27), ("0000 0001 0", 39),
    ("0000 0000 1", 0),
];

/// Table B-10, motion_code magnitude (the sign bit follows non-zero codes).
pub const MOTION_CODE: &[(&str, i32)] = &[
    ("1", 0), ("01", 1), ("001", 2), ("0001", 3), ("0000 11", 4), ("0000 101", 5),
    ("0000 100", 6), ("0000 011", 7), ("0000 0101 1", 8), ("0000 0101 0", 9),
    ("0000 0100 1", 10), ("0000 0100 01", 11), ("0000 0100 00", 12),
    ("0000 0011 11", 13), ("0000 0011 10", 14), ("0000 0011 01", 15),
    ("0000 0011 00", 16),
];

/// Table B-11, dmvector.
pub const DMVECTOR: &[(&str, i32)] = &[("0", 0), ("10", 1), ("11", -1)];

/// Table B-12, dct_dc_size_luminance.
pub const DC_SIZE_LUMA: &[(&str, i32)] = &[
    ("100", 0), ("00", 1), ("01", 2), ("101", 3), ("110", 4), ("1110", 5),
    ("1111 0", 6), ("1111 10", 7), ("1111 110", 8), ("1111 1110", 9),
    ("1111 1111 0", 10), ("1111 1111 1", 11),
];
/// Table B-13, dct_dc_size_chrominance.
pub const DC_SIZE_CHROMA: &[(&str, i32)] = &[
    ("00", 0), ("01", 1), ("10", 2), ("110", 3), ("1110", 4), ("1111 0", 5),
    ("1111 10", 6), ("1111 110", 7), ("1111 1110", 8), ("1111 1111 0", 9),
    ("1111 1111 10", 10), ("1111 1111 11", 11),
];

// --- DCT coefficient tables --------------------------------------------------

pub const DCT_EOB: i32 = -1;
pub const DCT_ESCAPE: i32 = -2;
pub const fn rl(run: i32, level: i32) -> i32 {
    run | level << 8
}

/// Codes shared by tables B-14 and B-15 (all 12..16-bit codes except the
/// ones listed separately per table, plus escape).
const DCT_COMMON: &[(&str, i32)] = &[
    ("0000 01", DCT_ESCAPE),
    ("0000 0001 1100", rl(3, 3)), ("0000 0001 0010", rl(4, 3)),
    ("0000 0001 1110", rl(6, 2)), ("0000 0001 0101", rl(7, 2)),
    ("0000 0001 0001", rl(8, 2)), ("0000 0001 1111", rl(17, 1)),
    ("0000 0001 1010", rl(18, 1)), ("0000 0001 1001", rl(19, 1)),
    ("0000 0001 0111", rl(20, 1)), ("0000 0001 0110", rl(21, 1)),
    ("0000 0000 1011 0", rl(1, 6)), ("0000 0000 1010 1", rl(1, 7)),
    ("0000 0000 1010 0", rl(2, 5)), ("0000 0000 1001 1", rl(3, 4)),
    ("0000 0000 1001 0", rl(5, 3)), ("0000 0000 1000 1", rl(9, 2)),
    ("0000 0000 1000 0", rl(10, 2)), ("0000 0000 1111 1", rl(22, 1)),
    ("0000 0000 1111 0", rl(23, 1)), ("0000 0000 1110 1", rl(24, 1)),
    ("0000 0000 1110 0", rl(25, 1)), ("0000 0000 1101 1", rl(26, 1)),
    ("0000 0000 0111 11", rl(0, 16)), ("0000 0000 0111 10", rl(0, 17)),
    ("0000 0000 0111 01", rl(0, 18)), ("0000 0000 0111 00", rl(0, 19)),
    ("0000 0000 0110 11", rl(0, 20)), ("0000 0000 0110 10", rl(0, 21)),
    ("0000 0000 0110 01", rl(0, 22)), ("0000 0000 0110 00", rl(0, 23)),
    ("0000 0000 0101 11", rl(0, 24)), ("0000 0000 0101 10", rl(0, 25)),
    ("0000 0000 0101 01", rl(0, 26)), ("0000 0000 0101 00", rl(0, 27)),
    ("0000 0000 0100 11", rl(0, 28)), ("0000 0000 0100 10", rl(0, 29)),
    ("0000 0000 0100 01", rl(0, 30)), ("0000 0000 0100 00", rl(0, 31)),
    ("0000 0000 0011 000", rl(0, 32)), ("0000 0000 0010 111", rl(0, 33)),
    ("0000 0000 0010 110", rl(0, 34)), ("0000 0000 0010 101", rl(0, 35)),
    ("0000 0000 0010 100", rl(0, 36)), ("0000 0000 0010 011", rl(0, 37)),
    ("0000 0000 0010 010", rl(0, 38)), ("0000 0000 0010 001", rl(0, 39)),
    ("0000 0000 0010 000", rl(0, 40)), ("0000 0000 0011 111", rl(1, 8)),
    ("0000 0000 0011 110", rl(1, 9)), ("0000 0000 0011 101", rl(1, 10)),
    ("0000 0000 0011 100", rl(1, 11)), ("0000 0000 0011 011", rl(1, 12)),
    ("0000 0000 0011 010", rl(1, 13)), ("0000 0000 0011 001", rl(1, 14)),
    ("0000 0000 0001 0011", rl(1, 15)), ("0000 0000 0001 0010", rl(1, 16)),
    ("0000 0000 0001 0001", rl(1, 17)), ("0000 0000 0001 0000", rl(1, 18)),
    ("0000 0000 0001 0100", rl(6, 3)), ("0000 0000 0001 1010", rl(11, 2)),
    ("0000 0000 0001 1001", rl(12, 2)), ("0000 0000 0001 1000", rl(13, 2)),
    ("0000 0000 0001 0111", rl(14, 2)), ("0000 0000 0001 0110", rl(15, 2)),
    ("0000 0000 0001 0101", rl(16, 2)), ("0000 0000 0001 1111", rl(27, 1)),
    ("0000 0000 0001 1110", rl(28, 1)), ("0000 0000 0001 1101", rl(29, 1)),
    ("0000 0000 0001 1100", rl(30, 1)), ("0000 0000 0001 1011", rl(31, 1)),
];

/// Table B-14 entries not in DCT_COMMON ("next coefficient" form: (0,1) is
/// '11s'; the first-coefficient '1s' form is handled by the block decoder).
const DCT_B14: &[(&str, i32)] = &[
    ("10", DCT_EOB), ("11", rl(0, 1)), ("011", rl(1, 1)), ("0100", rl(0, 2)),
    ("0101", rl(2, 1)), ("0010 1", rl(0, 3)), ("0011 1", rl(3, 1)),
    ("0011 0", rl(4, 1)), ("0001 10", rl(1, 2)), ("0001 11", rl(5, 1)),
    ("0001 01", rl(6, 1)), ("0001 00", rl(7, 1)), ("0000 110", rl(0, 4)),
    ("0000 100", rl(2, 2)), ("0000 111", rl(8, 1)), ("0000 101", rl(9, 1)),
    ("0010 0110", rl(0, 5)), ("0010 0001", rl(0, 6)), ("0010 0101", rl(1, 3)),
    ("0010 0100", rl(3, 2)), ("0010 0111", rl(10, 1)), ("0010 0011", rl(11, 1)),
    ("0010 0010", rl(12, 1)), ("0010 0000", rl(13, 1)),
    ("0000 0010 10", rl(0, 7)), ("0000 0011 00", rl(1, 4)),
    ("0000 0010 11", rl(2, 3)), ("0000 0011 11", rl(4, 2)),
    ("0000 0010 01", rl(5, 2)), ("0000 0011 10", rl(14, 1)),
    ("0000 0011 01", rl(15, 1)), ("0000 0010 00", rl(16, 1)),
    ("0000 0001 1101", rl(0, 8)), ("0000 0001 1000", rl(0, 9)),
    ("0000 0001 0011", rl(0, 10)), ("0000 0001 0000", rl(0, 11)),
    ("0000 0001 1011", rl(1, 5)), ("0000 0001 0100", rl(2, 4)),
    ("0000 0000 1101 0", rl(0, 12)), ("0000 0000 1100 1", rl(0, 13)),
    ("0000 0000 1100 0", rl(0, 14)), ("0000 0000 1011 1", rl(0, 15)),
];

/// Table B-15 entries not in DCT_COMMON (intra_vlc_format = 1).
const DCT_B15: &[(&str, i32)] = &[
    ("0110", DCT_EOB), ("10", rl(0, 1)), ("010", rl(1, 1)), ("110", rl(0, 2)),
    ("0010 1", rl(2, 1)), ("0111", rl(0, 3)), ("0011 1", rl(3, 1)),
    ("0001 10", rl(4, 1)), ("0011 0", rl(1, 2)), ("0001 11", rl(5, 1)),
    ("0000 110", rl(6, 1)), ("0000 100", rl(7, 1)), ("1110 0", rl(0, 4)),
    ("0000 111", rl(2, 2)), ("0000 101", rl(8, 1)), ("1111 000", rl(9, 1)),
    ("1110 1", rl(0, 5)), ("0001 01", rl(0, 6)), ("1111 001", rl(1, 3)),
    ("0010 0110", rl(3, 2)), ("1111 010", rl(10, 1)), ("0010 0001", rl(11, 1)),
    ("0010 0101", rl(12, 1)), ("0010 0100", rl(13, 1)), ("0001 00", rl(0, 7)),
    ("0010 0111", rl(1, 4)), ("1111 1100", rl(2, 3)), ("1111 1101", rl(4, 2)),
    ("0000 0010 0", rl(5, 2)), ("0000 0010 1", rl(14, 1)),
    ("0000 0011 1", rl(15, 1)), ("0000 0011 01", rl(16, 1)),
    ("1111 011", rl(0, 8)), ("1111 100", rl(0, 9)), ("0010 0011", rl(0, 10)),
    ("0010 0010", rl(0, 11)), ("0010 0000", rl(1, 5)),
    ("0000 0011 00", rl(2, 4)), ("1111 1010", rl(0, 12)),
    ("1111 1011", rl(0, 13)), ("1111 1110", rl(0, 14)), ("1111 1111", rl(0, 15)),
];

pub struct Tables {
    pub mb_addr_inc: Vlc,
    pub mb_type: [Vlc; 3],
    pub cbp: Vlc,
    pub motion_code: Vlc,
    pub dmvector: Vlc,
    pub dc_luma: Vlc,
    pub dc_chroma: Vlc,
    pub dct_b14: Vlc,
    pub dct_b15: Vlc,
}

fn joined(a: &[(&'static str, i32)], b: &[(&'static str, i32)]) -> Vec<(&'static str, i32)> {
    a.iter().chain(b).copied().collect()
}

pub fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| Tables {
        mb_addr_inc: Vlc::build(8, MB_ADDR_INC),
        mb_type: [
            Vlc::build(6, MB_TYPE_I),
            Vlc::build(6, MB_TYPE_P),
            Vlc::build(6, MB_TYPE_B),
        ],
        cbp: Vlc::build(9, CBP),
        motion_code: Vlc::build(11, MOTION_CODE),
        dmvector: Vlc::build(2, DMVECTOR),
        dc_luma: Vlc::build(9, DC_SIZE_LUMA),
        dc_chroma: Vlc::build(10, DC_SIZE_CHROMA),
        dct_b14: Vlc::build(10, &joined(DCT_B14, DCT_COMMON)),
        dct_b15: Vlc::build(10, &joined(DCT_B15, DCT_COMMON)),
    })
}

// --- Scans, matrices, scales -------------------------------------------------

/// Figure 7-2, zigzag scan: scan index -> raster index (v*8+u).
pub const ZIGZAG: [u8; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27,
    20, 13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51,
    58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];
/// Figure 7-3, alternate scan.
pub const ALTERNATE: [u8; 64] = [
    0, 8, 16, 24, 1, 9, 2, 10, 17, 25, 32, 40, 48, 56, 57, 49, 41, 33, 26, 18, 3, 11, 4, 12, 19,
    27, 34, 42, 50, 58, 35, 43, 51, 59, 20, 28, 5, 13, 6, 14, 21, 29, 36, 44, 52, 60, 37, 45,
    53, 61, 22, 30, 7, 15, 23, 31, 38, 46, 54, 62, 39, 47, 55, 63,
];

/// Default intra quantiser matrix, raster order (clause 6.3.11).
pub const DEFAULT_INTRA: [u8; 64] = [
    8, 16, 19, 22, 26, 27, 29, 34, 16, 16, 22, 24, 27, 29, 34, 37, 19, 22, 26, 27, 29, 34, 34,
    38, 22, 22, 26, 27, 29, 34, 37, 40, 22, 26, 27, 29, 32, 35, 40, 48, 26, 27, 29, 32, 35, 40,
    48, 58, 26, 27, 29, 34, 38, 46, 56, 69, 27, 29, 35, 38, 46, 56, 69, 83,
];

/// Table 7-6, quantiser_scale for q_scale_type = 1.
pub const NON_LINEAR_QSCALE: [u8; 32] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 12, 14, 16, 18, 20, 22, 24, 28, 32, 36, 40, 44, 48, 52, 56,
    64, 72, 80, 88, 96, 104, 112,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_tables_build_without_overlap() {
        let _ = tables();
    }

    #[test]
    fn complete_prefix_codes() {
        // Tables that use the whole code space, or all of it except the
        // escape / start-code-emulating patterns the standard reserves.
        assert_eq!(Vlc::kraft(DC_SIZE_LUMA), 1.0);
        assert_eq!(Vlc::kraft(DC_SIZE_CHROMA), 1.0);
        assert_eq!(Vlc::kraft(DMVECTOR), 1.0);
        assert_eq!(Vlc::kraft(MB_TYPE_I), 0.75);
        // CBP: all 64 values once; only '0000 0000 0' is unused.
        assert_eq!(Vlc::kraft(CBP), 1.0 - 1.0 / 512.0);
        let mut seen = [false; 64];
        for (_, v) in CBP {
            assert!(!seen[*v as usize]);
            seen[*v as usize] = true;
        }
        // B-14 covers everything but '0000 0000 0000' (start code prefix).
        let b14 = joined(DCT_B14, DCT_COMMON);
        assert_eq!(Vlc::kraft(&b14), 1.0 - 1.0 / 4096.0);
        // Address increment: with the escape '0000 0001 000' and MPEG-1
        // stuffing '0000 0001 111', only '0000 0000' (start codes),
        // '0000 0010' and '0000 0001 001'..'110' are unused.
        let mut mba = MB_ADDR_INC.to_vec();
        mba.push(("0000 0001 000", 0));
        mba.push(("0000 0001 111", 0));
        assert_eq!(Vlc::kraft(&mba), 1.0 - 2.0 / 256.0 - 6.0 / 2048.0);
    }

    #[test]
    fn dct_tables_assign_each_run_level_once() {
        for table in [joined(DCT_B14, DCT_COMMON), joined(DCT_B15, DCT_COMMON)] {
            let mut seen = std::collections::BTreeSet::new();
            for (_, v) in &table {
                assert!(seen.insert(*v), "duplicate value {v}");
            }
            // 111 run/level pairs + EOB + escape.
            assert_eq!(table.len(), 113, "table size");
        }
    }

    #[test]
    fn scans_are_permutations() {
        for scan in [ZIGZAG, ALTERNATE] {
            let mut s = scan.to_vec();
            s.sort();
            assert_eq!(s, (0..64).collect::<Vec<u8>>());
        }
    }

    #[test]
    fn decodes_long_and_short_codes() {
        let t = tables();
        // '0000 0000 0001 1011' + '1' (B-14, run 31 level 1) then '10' (EOB)
        let data = [0b0000_0000, 0b0001_1011, 0b1100_0000];
        let mut b = Bits::new(&data);
        assert_eq!(t.dct_b14.decode(&mut b), Some(rl(31, 1)));
        assert_eq!(b.read(1), 1);
        assert_eq!(t.dct_b14.decode(&mut b), Some(DCT_EOB));
        let zero = [0u8; 4];
        assert_eq!(t.dct_b14.decode(&mut Bits::new(&zero)), None);
    }
}
