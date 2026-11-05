//! Radiating copies of the unit: the unit drawn
//! into the render target (0x1F8270) once, then stretched from its screen
//! position out towards the window's edges and corners, in a packet layer.
use super::exe::{alpha_mode, camera_vp_into, countdown, node_source};
use crate::bus::Bus;
use crate::ee;
use crate::exe::Fx;

/// baff7d24198d [15 B records; b173 0x1E03960, 904 words] [F]:
/// `streaks(at, recs, ot, n) -> 1` in a packet layer (0x2078A0; closed and
/// linked into `ot` at key 0 by 0x207910 only when a unit was drawn) steps
/// and draws `n` records (0x38 bytes; a 0x260 frame, the descriptor at sp
/// + 0x1C0: RGBAQ / ST / XYZF2 x 4). Per record: countdown 0x101AF0;
/// position +0x18 += +0x28, then from a node with +5 (0x101BA0); the
/// camera (0x20E2B0 into sp + 0xE0); the eye (0x1FACE0) less the position
/// aimed (0x101A00) turns (0, 5, 0) (-10 with flag 1) into sp + 0x160
/// (0x20C540), less the position again. Both points must project (0x204D80)
/// and the position lie in the window (0x101770); its window offset (s1,
/// s2) = xy - 28672 (0x1A2B80). Once per call: alpha 0x80, Q 1.0, far Z,
/// TEXFLUSH, the unit +0xC drawn into the target (0x1F8270, filter 0),
/// TEXFLUSH. Colours +0x16.. (the unit's pair) / +0x12.. (the far pair);
/// flag 0x80: far Z and key 0, else the first point's z and w; the blend
/// +9 (0x101C80 / 0x207E20), flags | 0x800; UV (s1, s2) at the unit's
/// pair; alphas +0x17 / +0x13 x count / 8 over the last 8. Then
/// min(+0xB, elapsed + 1, count) rings s4 of the directions k set in +0xA
/// (8 bits: left, up-left, up, up-right, right, down-right, down,
/// down-left, the record's jump table): a quad from the unit's pair to
/// the window edge pushed out by 16 / 11.2 x s4 x +8 / 2 (0x1A2C70), UVs to
/// the texture's matching edge (0x2000), no ordering table.
pub fn streaks(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    streaks_a3(fx, at, recs, ot, n).0
}

/// The a3 each direction handler of [`streaks`] leaves (its U / mask
/// constant: 0 / 0 / -0x4000 / 0x2000 / 0x2000 / 0x2000 / -0x4000 / 0)
/// [F b173 0x1E041B0..0x1E0461C].
const DIRECTION_A3: [u32; 8] = [0, 0, 0xFFFF_C000, 0x2000, 0x2000, 0x2000, 0xFFFF_C000, 0];

/// [`streaks`] and the a3 it leaves (helper B10, for a later boundary
/// call that reports it) [F]: per counting record the first / second
/// projection's w pointer (sp + 0x250 / 0x254) when one fails or the
/// point lies outside the window, else the last direction handler's
/// constant ([`DIRECTION_A3`]), or s1 & 0x3FFF (the window offset's UV)
/// with no ring drawn. `None` when no record counted (the caller's a3 is
/// kept). The packet layer's 0x207910 leaves a3.
pub fn streaks_a3(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> (i32, Option<u32>) {
    streaks_with(fx, at, recs, ot, n, &STREAKS)
}

/// Where a compilation of [`streaks`] keeps its frame: the size, the
/// shift of the shared locals (`sp + shift` stands for [`streaks`]' sp),
/// the layer / drawn-flag slots (from that shifted sp) and the offsets of
/// the conversions' `jal`s in the function (aim_rotation's return, then
/// the window offset's two, the 16 direction handlers', the key's).
pub struct StreakLayout {
    pub frame: u32,
    /// Added (wrapping) to the compilation's own sp: 0x60 for a frame 0x60
    /// bigger, -0x20 for one 0x20 smaller.
    pub shift: u32,
    pub layer: u32,
    /// The drawn flag's slot, `None` when it lives in a register (s7).
    pub flag: Option<u32>,
    /// `n`'s slot (sp + 0xD4), `None` when it stays in fp.
    pub n_slot: Option<u32>,
    /// The vertex whose stale XYZ low word the first TEXFLUSH reports as
    /// a1 (the last `ld` into a1).
    pub a1_xyz: u32,
    /// The vertex whose stale XYZ low word that TEXFLUSH reports as a0
    /// (`ld a0`), `None` when a0 is the shifted far Z (`dsll32`: 0).
    pub a0_xyz: Option<u32>,
    /// What the direction handlers (and the UV setter with no ring) leave
    /// in a3.
    pub a3: StreakA3,
    pub jals: [u32; 20],
}

/// The a3 a compilation of [`streaks`] leaves.
pub enum StreakA3 {
    /// [`DIRECTION_A3`]; `s1 & 0x3FFF` with no ring.
    Constant,
    /// Vertex 2's U as written (k = 0, 1, 4, 5) or its V as it was (2, 3,
    /// 6, 7); vertex 3's U as written with no ring.
    Uv,
    /// The pointer to vertex 2's UV (sp + 0x1F0); vertex 3's (sp + 0x220)
    /// with no ring.
    Pointer,
}

/// baff7d24198d's layout (b173 0x1E03960).
pub const STREAKS: StreakLayout = StreakLayout {
    frame: 0x260,
    shift: 0,
    layer: 0xBC,
    flag: Some(0xC0),
    n_slot: Some(0xD4),
    a1_xyz: 0x1F8,
    a0_xyz: Some(0x1E0),
    a3: StreakA3::Constant,
    jals: [0x140, 0x268, 0x2C4, 0x8AC, 0x8D4, 0x94C, 0x970, 0x9F8, 0xA14, 0xA94, 0xAB4, 0xB34, 0xB5C, 0xBD4, 0xBF8, 0xC80, 0xC9C, 0xD1C, 0xD3C, 0xD54],
};

/// b5cb7483461a [20 B records; b103's 0x1E04D40, 896 words] (helper
/// B10) [F]: [`streaks`] compiled with a 0x2C0 frame (the shared locals
/// at the same addresses below the caller's stack: shift 0x60), the layer
/// / drawn flag at its sp + 0xDC / + 0x100 and its own `jal` offsets.
pub const STREAKS_B: StreakLayout = StreakLayout {
    frame: 0x2C0,
    shift: 0x60,
    layer: 0x7C,
    flag: Some(0xA0),
    n_slot: Some(0xD4),
    a1_xyz: 0x228,
    a0_xyz: Some(0x1E0),
    a3: StreakA3::Uv,
    jals: [0x16C, 0x280, 0x2CC, 0x91C, 0x940, 0x9A4, 0x9C4, 0xA40, 0xA54, 0xAC4, 0xADC, 0xB4C, 0xB70, 0xBD4, 0xBF4, 0xC70, 0xC84, 0xCF4, 0xD0C, 0xD2C],
};

/// The b5cb7483461a compilation of [`streaks`] ([`STREAKS_B`]).
pub fn streaks_b(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    streaks_with(fx, at, recs, ot, n, &STREAKS_B).0
}

/// 391d5c6072b1 [4 B records; b199's 0x1E03410, 1012 words]
/// [F]: [`streaks`] compiled with a 0x240 frame (the shared locals 0x20
/// lower in it: the same caller-relative addresses), `n` kept in fp and
/// the drawn flag in s7 (no slots), the layer at the shared sp + 0xD4,
/// the far-Z chain's a0 the shifted constant and its last `ld a1` vertex
/// 1, the direction handlers leaving the vertex-UV pointers in a3, and
/// its own `jal` offsets.
pub const STREAKS_C: StreakLayout = StreakLayout {
    frame: 0x240,
    shift: 0u32.wrapping_sub(0x20),
    layer: 0xD4,
    flag: None,
    n_slot: None,
    a1_xyz: 0x1E0,
    a0_xyz: None,
    a3: StreakA3::Pointer,
    jals: [0x184, 0x25C, 0x2B4, 0x9B8, 0x9E0, 0xA74, 0xA98, 0xB38, 0xB54, 0xBEC, 0xC0C, 0xCA8, 0xCD0, 0xD64, 0xD88, 0xE28, 0xE44, 0xEDC, 0xEFC, 0xF14],
};

/// The 391d5c6072b1 compilation of [`streaks`] ([`STREAKS_C`]).
pub fn streaks_c(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    streaks_with(fx, at, recs, ot, n, &STREAKS_C).0
}

/// [`streaks_c`] and the a3 it leaves (as [`streaks_a3`]).
pub fn streaks_c_a3(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> (i32, Option<u32>) {
    streaks_with(fx, at, recs, ot, n, &STREAKS_C)
}

/// [`streaks`] compiled with layout `l` (and its a3, as [`streaks_a3`]).
pub fn streaks_with(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, l: &StreakLayout) -> (i32, Option<u32>) {
    let mut a3 = None;
    let sp = fx.enter(l.frame).wrapping_add(l.shift);
    fx.mem.write_u32(sp + 0xD8, ot);
    if let Some(o) = l.n_slot {
        fx.mem.set32(sp + o, n);
    }
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + l.layer, layer);
    let mut drawn = false;
    if let Some(o) = l.flag {
        fx.mem.write_u32(sp + o, 0);
    }
    let mut g = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x38;
        }
        if let Some(v) = one(fx, at, sp, g, l, &mut drawn) {
            a3 = Some(v);
        }
        i += 1;
    }
    if drawn {
        let (layer, ot) = (fx.mem.read_u32(sp + l.layer), fx.mem.read_u32(sp + 0xD8));
        fx.layer_end(layer, ot, 0);
    }
    fx.leave(l.frame);
    (1, a3)
}

/// 0x1A2B80 / 0x1A2C70 of `v` from the `jal` at `at + off` (its frame
/// left).
fn fix(fx: &mut Fx, at: u32, off: u32, v: f32) -> i32 {
    fx.fix_residue(v, at + off + 8);
    ee::fixsfsi(v)
}
fn fixu(fx: &mut Fx, at: u32, off: u32, v: f32) -> u16 {
    fx.fix_residue(v, at + off + 8);
    ee::fixunssfsi(v) as u16
}

/// The 14-bit UV fields (u16 at `a`, `a` + 2), their top bits kept.
fn uv(fx: &mut Fx, a: u32, u: i32, v: i32) {
    for (o, x) in [(0u32, u), (2, v)] {
        let h = fx.mem.read_u16(a + o);
        fx.mem.write_u16(a + o, (h & 0xC000) | (x & 0x3FFF) as u16);
    }
}

/// z of the four vertices from `z` down the chain (vertex 4 first).
fn chain_z(fx: &mut Fx, sp: u32, z: u64) {
    fx.set_xyz_z(sp + 0x228, z);
    for (dst, src) in [(0x210u32, 0x228u32), (0x1F8, 0x210), (0x1E0, 0x1F8)] {
        let z = fx.xyz_z(sp + src);
        fx.set_xyz_z(sp + dst, z);
    }
}

fn one(fx: &mut Fx, at: u32, sp: u32, g: u32, l: &StreakLayout, drawn: &mut bool) -> Option<u32> {
    if countdown(fx, g) == 0 {
        return None;
    }
    for c in 0..3u32 {
        let v = ee::add(fx.mem.f(g + 0x18 + 4 * c), fx.mem.f(g + 0x28 + 4 * c));
        fx.mem.setf(g + 0x18 + 4 * c, v);
    }
    node_source(fx, g, g + 0x18);
    camera_vp_into(fx, sp + 0xE0);
    let eye = fx.camera_eye(0);
    for c in 0..4u32 {
        let v = fx.mem.read_u32(eye + 4 * c);
        fx.mem.write_u32(sp + 0x140 + 4 * c, v);
    }
    for c in 0..3u32 {
        let v = ee::sub(fx.mem.f(sp + 0x140 + 4 * c), fx.mem.f(g + 0x18 + 4 * c));
        fx.mem.setf(sp + 0x150 + 4 * c, v);
    }
    super::spray::aim_rotation(fx, sp + 0x150, sp + 0x170, [u64::from(g), u64::from(sp + 0x154)], at + l.jals[0] + 8);
    fx.mem.write_u32(sp + 0x158, 0);
    fx.mem.write_u32(sp + 0x150, 0);
    fx.mem.write_u32(sp + 0x154, 0x40A0_0000);
    if fx.mem.u8(g + 4) & 1 != 0 {
        let v = ee::mul(fx.mem.f(sp + 0x154), -2.0);
        fx.mem.setf(sp + 0x154, v);
    }
    let m = fx.read_m4(sp + 0x170);
    let v = fx.read_v4(sp + 0x150);
    let r = fx.transform(Some(&m), v);
    fx.write_v4(sp + 0x160, r);
    for c in 0..3u32 {
        let v = ee::add(fx.mem.f(sp + 0x160 + 4 * c), fx.mem.f(g + 0x18 + 4 * c));
        fx.mem.setf(sp + 0x160 + 4 * c, v);
    }
    let m = fx.read_m4(sp + 0xE0);
    if !fx.project_at(sp + 0x160, &m, sp + 0x1B0, sp + 0x250) {
        return Some(sp + 0x250);
    }
    let m = fx.read_m4(sp + 0xE0);
    if !fx.project_at(g + 0x18, &m, sp + 0x1B8, sp + 0x254) {
        return Some(sp + 0x254);
    }
    if !crate::b299::in_window(fx, sp + 0x1B8) {
        return Some(sp + 0x254);
    }
    let x = fx.mem.u16(sp + 0x1B8);
    let s1 = fix(fx, at, l.jals[1], ee::sub(ee::from_i32(x), 28672.0));
    let y = fx.mem.u16(sp + 0x1BA);
    let s2 = fix(fx, at, l.jals[2], ee::sub(ee::from_i32(y), 28672.0));
    let d = sp + 0x1C0;
    if !*drawn {
        fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
        for k in [2u32, 1, 0] {
            for o in [0x218u32, 0x200, 0x1E8, 0x1D0] {
                fx.mem.write_u8(sp + o + k, 0);
            }
        }
        for o in [0x21Bu32, 0x203, 0x1EB, 0x1D3] {
            fx.mem.write_u8(sp + o, 0x80);
        }
        for o in [0x21Cu32, 0x204, 0x1EC, 0x1D4] {
            fx.mem.write_u32(sp + o, 0x3F80_0000);
        }
        let a1 = fx.mem.read_u32(sp + l.a1_xyz);
        let a0 = l.a0_xyz.map_or(0, |o| fx.mem.read_u32(sp + o));
        chain_z(fx, sp, 0xFF_FFFF);
        for o in [0x22Fu32, 0x217, 0x1FF, 0x1E7] {
            fx.mem.write_u8(sp + o, 0);
        }
        // a0 / a1 the low halves of vertex 1's / 2's XYZ before the far Z,
        // a2 = sp - 0x10 (as the calls before leave it), a3 the second
        // projection's w pointer [run].
        fx.texflush([a0, a1, sp.wrapping_sub(l.shift).wrapping_sub(0x10), sp + 0x254]);
        let h = fx.mem.i32(g + 0xC);
        let (_, regs) = fx.unit_draw(h, 0, sp + 0x238, sp + 0x240);
        fx.texflush(regs);
        *drawn = true;
        if let Some(o) = l.flag {
            fx.mem.write_u32(sp + o, 1);
        }
    }
    for (src, dsts) in [(0x16u32, [0x1E8u32, 0x1D0]), (0x12, [0x218, 0x200]), (0x15, [0x1E9, 0x1D1]), (0x11, [0x219, 0x201]), (0x14, [0x1EA, 0x1D2]), (0x10, [0x21A, 0x202])] {
        let v = fx.mem.read_u8(g + src);
        for o in dsts {
            fx.mem.write_u8(sp + o, v);
        }
    }
    let f22 = if fx.mem.u8(g + 4) & 0x80 != 0 {
        chain_z(fx, sp, 0xFF_FFFF);
        0.0
    } else {
        let z = fx.xyz_z(sp + 0x1B0);
        chain_z(fx, sp, z);
        fx.mem.f(sp + 0x250)
    };
    let b = fx.mem.read_u8(g + 9);
    fx.mem.write_u8(sp + 0x125, b);
    alpha_mode(fx, sp + 0x120, sp + 0x25C);
    let alpha = u64::from(fx.mem.read_u8(sp + 0x25C))
        | (u64::from(fx.mem.read_u8(sp + 0x25D)) << 2)
        | (u64::from(fx.mem.read_u8(sp + 0x25E)) << 4)
        | (u64::from(fx.mem.read_u8(sp + 0x25F)) << 6);
    fx.desc_alpha(d, alpha);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    uv(fx, sp + 0x208, s1, s2);
    uv(fx, sp + 0x220, s1, s2);
    let x = fx.mem.read_u16(sp + 0x1B8);
    fx.mem.write_u16(sp + 0x228, x);
    fx.mem.write_u16(sp + 0x210, x);
    let y = fx.mem.read_u16(sp + 0x1BA);
    fx.mem.write_u16(sp + 0x22A, y);
    fx.mem.write_u16(sp + 0x212, y);
    let count = fx.mem.u8(g);
    if count < 8 {
        let a = fx.mem.u8(g + 0x17) >> 3;
        let v = fx.mult(a, count);
        fx.mem.set8(sp + 0x1EB, v);
        fx.mem.set8(sp + 0x1D3, v);
        let v = (fx.mem.u8(g + 0x13) >> 3).wrapping_mul(count);
        fx.mem.set8(sp + 0x21B, v);
        fx.mem.set8(sp + 0x203, v);
    } else {
        let v = fx.mem.read_u8(g + 0x17);
        fx.mem.write_u8(sp + 0x1EB, v);
        fx.mem.write_u8(sp + 0x1D3, v);
        let v = fx.mem.read_u8(g + 0x13);
        fx.mem.write_u8(sp + 0x21B, v);
        fx.mem.write_u8(sp + 0x203, v);
    }
    let mut rings = fx.mem.u8(g + 0xB);
    let e = fx.mem.u8(g + 2) - fx.mem.u8(g) + 1;
    if e < rings {
        rings = e;
    }
    if count < rings {
        rings = count;
    }
    let (ux, uy) = (sp + 0x1B8, sp + 0x1BA);
    // With no ring drawn a3 is the last UV setter's: streaks' `s1 & 0x3FFF`;
    // streaks_b's vertex-3 U as written there, its stale top bits kept
    // [run b715].
    let mut a3 = match l.a3 {
        StreakA3::Constant => (s1 & 0x3FFF) as u32,
        StreakA3::Uv => u32::from(fx.mem.read_u16(sp + 0x220)),
        StreakA3::Pointer => sp + 0x220,
    };
    for s4 in 0..rings {
        let f21 = ee::mul(ee::mul(0.5, ee::from_i32(s4)), ee::from_i32(fx.mem.u8(g + 8)));
        for k in 0..8u32 {
            if fx.mem.u8(g + 0xA) & (1 << k) == 0 {
                continue;
            }
            let v_old = u32::from(fx.mem.read_u16(sp + 0x1DA));
            let f1 = ee::mul(16.0, f21);
            let f20 = ee::mul(f32::from_bits(0x4133_3333), f21);
            let lo = |f: f32| ee::sub(28672.0, f);
            let hi = |f: f32| ee::add(36864.0, f);
            match k {
                0 => {
                    uv(fx, sp + 0x1D8, 0, s2);
                    uv(fx, sp + 0x1F0, 0, 0);
                    let v = fixu(fx, at, l.jals[3], lo(f1));
                    fx.mem.write_u16(sp + 0x1F8, v);
                    fx.mem.write_u16(sp + 0x1E0, v);
                    let y = fx.mem.read_u16(uy);
                    fx.mem.write_u16(sp + 0x1E2, y);
                    let v = fixu(fx, at, l.jals[4], lo(f20));
                    fx.mem.write_u16(sp + 0x1FA, v);
                }
                1 => {
                    uv(fx, sp + 0x1D8, 0, 0);
                    uv(fx, sp + 0x1F0, s1, 0);
                    let v = fixu(fx, at, l.jals[5], lo(f1));
                    fx.mem.write_u16(sp + 0x1E0, v);
                    let x = fx.mem.read_u16(ux);
                    fx.mem.write_u16(sp + 0x1F8, x);
                    let v = fixu(fx, at, l.jals[6], lo(f20));
                    fx.mem.write_u16(sp + 0x1FA, v);
                    fx.mem.write_u16(sp + 0x1E2, v);
                }
                2 => {
                    uv(fx, sp + 0x1D8, s1, 0);
                    uv(fx, sp + 0x1F0, 0x2000, 0);
                    let x = fx.mem.read_u16(ux);
                    fx.mem.write_u16(sp + 0x1E0, x);
                    let v = fixu(fx, at, l.jals[7], hi(f1));
                    fx.mem.write_u16(sp + 0x1F8, v);
                    let v = fixu(fx, at, l.jals[8], lo(f20));
                    fx.mem.write_u16(sp + 0x1FA, v);
                    fx.mem.write_u16(sp + 0x1E2, v);
                }
                3 => {
                    uv(fx, sp + 0x1D8, 0x2000, 0);
                    uv(fx, sp + 0x1F0, 0x2000, s2);
                    let v = fixu(fx, at, l.jals[9], hi(f1));
                    fx.mem.write_u16(sp + 0x1F8, v);
                    fx.mem.write_u16(sp + 0x1E0, v);
                    let v = fixu(fx, at, l.jals[10], lo(f20));
                    fx.mem.write_u16(sp + 0x1E2, v);
                    let y = fx.mem.read_u16(uy);
                    fx.mem.write_u16(sp + 0x1FA, y);
                }
                4 => {
                    uv(fx, sp + 0x1D8, 0x2000, s2);
                    uv(fx, sp + 0x1F0, 0x2000, 0x2000);
                    let v = fixu(fx, at, l.jals[11], hi(f1));
                    fx.mem.write_u16(sp + 0x1F8, v);
                    fx.mem.write_u16(sp + 0x1E0, v);
                    let y = fx.mem.read_u16(uy);
                    fx.mem.write_u16(sp + 0x1E2, y);
                    let v = fixu(fx, at, l.jals[12], hi(f20));
                    fx.mem.write_u16(sp + 0x1FA, v);
                }
                5 => {
                    uv(fx, sp + 0x1D8, 0x2000, 0x2000);
                    uv(fx, sp + 0x1F0, s1, 0x2000);
                    let v = fixu(fx, at, l.jals[13], hi(f1));
                    fx.mem.write_u16(sp + 0x1E0, v);
                    let x = fx.mem.read_u16(ux);
                    fx.mem.write_u16(sp + 0x1F8, x);
                    let v = fixu(fx, at, l.jals[14], hi(f20));
                    fx.mem.write_u16(sp + 0x1FA, v);
                    fx.mem.write_u16(sp + 0x1E2, v);
                }
                6 => {
                    uv(fx, sp + 0x1D8, s1, 0x2000);
                    uv(fx, sp + 0x1F0, 0, 0x2000);
                    let x = fx.mem.read_u16(ux);
                    fx.mem.write_u16(sp + 0x1E0, x);
                    let v = fixu(fx, at, l.jals[15], lo(f1));
                    fx.mem.write_u16(sp + 0x1F8, v);
                    let v = fixu(fx, at, l.jals[16], hi(f20));
                    fx.mem.write_u16(sp + 0x1FA, v);
                    fx.mem.write_u16(sp + 0x1E2, v);
                }
                _ => {
                    uv(fx, sp + 0x1D8, 0, 0x2000);
                    uv(fx, sp + 0x1F0, 0, s2);
                    let v = fixu(fx, at, l.jals[17], lo(f1));
                    fx.mem.write_u16(sp + 0x1F8, v);
                    fx.mem.write_u16(sp + 0x1E0, v);
                    let v = fixu(fx, at, l.jals[18], hi(f20));
                    fx.mem.write_u16(sp + 0x1E2, v);
                    let y = fx.mem.read_u16(uy);
                    fx.mem.write_u16(sp + 0x1FA, y);
                }
            }
            a3 = match l.a3 {
                StreakA3::Constant => DIRECTION_A3[k as usize],
                StreakA3::Uv if matches!(k, 0 | 1 | 4 | 5) => u32::from(fx.mem.read_u16(sp + 0x1D8)),
                StreakA3::Uv => v_old,
                StreakA3::Pointer => sp + 0x1F0,
            };
            fx.fix_residue(f22, at + l.jals[19] + 8);
            let key = ee::fixunssfsi(f22) as i32;
            fx.submit(d, 0, key);
        }
    }
    Some(a3)
}

/// 8661b8724616 [4 B records; b106 0x1E04350, 464 words] [F]:
/// `echoes(recs, ot, n) -> 1` in a packet layer (0x2078A0; closed and
/// linked into `ot` at key 0 by 0x207910 only when a unit was drawn) steps
/// and draws `n` records (0x24 bytes; a 0x260 frame, the descriptor at sp
/// + 0x1C0: RGBAQ / ST / XYZF2 x 4). Per record: countdown 0x101AF0; once
/// per call the descriptor (colours 0x80, Q 1.0, far Z), TEXFLUSH, the
/// unit +0xC drawn into the target (0x1F8270, filter 0), TEXFLUSH, flags |
/// 0x800 and the target's four corner UVs. Then the camera (0x20E2B0 into
/// sp + 0xE0); the unit's point (0x1F65D0 kind 1) aimed at the eye
/// (0x1FACE0, 0x101A00) turns (0, radius 0x1F6A70 (-radius with flag 1),
/// 0) into sp + 0x140 (0x20C540) about the point; it must project
/// (0x204D80) and lie in the window (0x101770). The projected z on every
/// vertex, the blend +9 (0x101C80 / 0x207E20), alphas +0x13; then for the
/// two signs and min(+8 / 2, elapsed + 1, count) steps s the window-sized
/// quad (28672 .. 36864 in 12.4) offset by sign 16 (s + 1) (+0xA, +0xB)
/// (0x1A2C70), keyed by the projection's w, no ordering table. `a3` is
/// the caller's a3 (the first TEXFLUSH reports it).
pub fn echoes(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, a3: u32) -> i32 {
    echoes_with(fx, at, recs, ot, n, a3, &ECHO_A)
}

/// How a compile of [`echoes`] differs: its frame (the body's locals stay
/// at the same caller-relative addresses), the `ra`s it leaves (record
/// offsets of 0x101A00's and the five 0x1A2C70 calls'), and the first
/// TEXFLUSH's registers.
struct EchoLayout {
    frame: u32,
    aim: u32,
    fix: [u32; 5],
    /// The registers kept in s registers (b123's compile): the first
    /// TEXFLUSH sees a0 = 0, a1 = vertex 1's low XYZ half, a2 = n, and
    /// 0x101A00 saves s1 = the record index.
    regs_b: bool,
}

const ECHO_A: EchoLayout = EchoLayout { frame: 0x260, aim: 0x348, fix: [0x5E0, 0x60C, 0x63C, 0x664, 0x678], regs_b: false };
const ECHO_B: EchoLayout = EchoLayout { frame: 0x230, aim: 0x3C0, fix: [0x6A0, 0x6D0, 0x700, 0x730, 0x744], regs_b: true };

/// f1079861aa31 [b123 (card 482) 0x1E00E40, 508 words]:
/// [`echoes`] recompiled with the count, the drawn flag and the index in
/// registers: a 0x230 frame (the descriptor and locals at the same
/// caller-relative addresses), its calls' return addresses moved, the
/// first TEXFLUSH reporting (0, vertex 1's low XYZ half, n, a3) and the
/// aim's frame the record index as s1 [F word-compare; fnparity].
pub fn echoes_b(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, a3: u32) -> i32 {
    echoes_with(fx, at, recs, ot, n, a3, &ECHO_B)
}

#[allow(clippy::too_many_arguments)]
fn echoes_with(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, a3: u32, l: &EchoLayout) -> i32 {
    let frame = fx.enter(l.frame);
    let sp = frame + l.frame - 0x260;
    fx.mem.write_u32(sp + 0xD8, ot);
    fx.mem.set32(sp + 0xD4, n);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xBC, layer);
    fx.mem.write_u32(sp + 0xC0, 0);
    let mut g = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x24;
        }
        echo(fx, at, sp, g, a3, l, (i, n));
        i += 1;
    }
    if fx.mem.read_u32(sp + 0xC0) != 0 {
        let (layer, ot) = (fx.mem.read_u32(sp + 0xBC), fx.mem.read_u32(sp + 0xD8));
        fx.layer_end(layer, ot, 0);
    }
    fx.leave(l.frame);
    1
}

fn echo(fx: &mut Fx, at: u32, sp: u32, g: u32, a3: u32, l: &EchoLayout, (i, n): (i32, i32)) {
    if countdown(fx, g) == 0 {
        return;
    }
    let d = sp + 0x1C0;
    if fx.mem.read_u32(sp + 0xC0) == 0 {
        fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
        for k in [3u32, 2, 1, 0] {
            for o in [0x218u32, 0x200, 0x1E8, 0x1D0] {
                fx.mem.write_u8(sp + o + k, 0x80);
            }
        }
        for o in [0x21Cu32, 0x204, 0x1EC, 0x1D4] {
            fx.mem.write_u32(sp + o, 0x3F80_0000);
        }
        let a2 = fx.mem.read_u32(sp + 0x228);
        let a0 = fx.mem.read_u32(sp + 0x1E0);
        chain_z(fx, sp, 0xFF_FFFF);
        for o in [0x22Fu32, 0x217, 0x1FF, 0x1E7] {
            fx.mem.write_u8(sp + o, 0);
        }
        // a0 / a2 the low halves of vertex 1's / 4's XYZ before the far Z,
        // a1 the z mask's low half [F].
        if l.regs_b {
            fx.texflush([0, a0, n as u32, a3]);
        } else {
            fx.texflush([a0, 0xFFFF_FFFF, a2, a3]);
        }
        let h = fx.mem.i32(g + 0xC);
        let (_, regs) = fx.unit_draw(h, 0, sp + 0x238, sp + 0x240);
        fx.texflush(regs);
        fx.mem.write_u32(sp + 0xC0, 1);
        let f = fx.mem.read_u16(d + 2);
        fx.mem.write_u16(d + 2, f | 0x800);
        uv(fx, sp + 0x1D8, 0, 0);
        uv(fx, sp + 0x1F0, 0x2000, 0);
        uv(fx, sp + 0x208, 0, 0x2000);
        uv(fx, sp + 0x220, 0x2000, 0x2000);
    }
    camera_vp_into(fx, sp + 0xE0);
    let eye = fx.camera_eye(0);
    for c in 0..4u32 {
        let v = fx.mem.read_u32(eye + 4 * c);
        fx.mem.write_u32(sp + 0x150 + 4 * c, v);
    }
    let h = fx.mem.i32(g + 0xC);
    fx.unit_point(1, h, sp + 0x120);
    for c in 0..3u32 {
        let v = ee::sub(fx.mem.f(sp + 0x150 + 4 * c), fx.mem.f(sp + 0x120 + 4 * c));
        fx.mem.setf(sp + 0x130 + 4 * c, v);
    }
    super::spray::aim_rotation(fx, sp + 0x130, sp + 0x160, [u64::from(g), if l.regs_b { i as u64 } else { u64::from(sp + 0x134) }], at + l.aim);
    fx.mem.write_u32(sp + 0x138, 0);
    fx.mem.write_u32(sp + 0x130, 0);
    let h = fx.mem.i32(g + 0xC);
    let r = super::afterimage::unit_radius(fx, h);
    fx.mem.setf(sp + 0x134, r);
    if fx.mem.u8(g + 4) & 1 != 0 {
        let v = ee::mul(fx.mem.f(sp + 0x134), -1.0);
        fx.mem.setf(sp + 0x134, v);
    }
    let m = fx.read_m4(sp + 0x160);
    let v = fx.read_v4(sp + 0x130);
    let r = fx.transform(Some(&m), v);
    fx.write_v4(sp + 0x140, r);
    for c in 0..3u32 {
        let v = ee::add(fx.mem.f(sp + 0x140 + 4 * c), fx.mem.f(sp + 0x120 + 4 * c));
        fx.mem.setf(sp + 0x140 + 4 * c, v);
    }
    let m = fx.read_m4(sp + 0xE0);
    if !fx.project_at(sp + 0x140, &m, sp + 0x250, sp + 0x258) {
        return;
    }
    if !crate::b299::in_window(fx, sp + 0x250) {
        return;
    }
    let z = fx.xyz_z(sp + 0x250);
    chain_z(fx, sp, z);
    for o in [0x22Fu32, 0x217, 0x1FF, 0x1E7] {
        fx.mem.write_u8(sp + o, 0);
    }
    let b = fx.mem.read_u8(g + 9);
    fx.mem.write_u8(sp + 0x1A5, b);
    alpha_mode(fx, sp + 0x1A0, sp + 0x25C);
    let alpha = u64::from(fx.mem.read_u8(sp + 0x25C))
        | (u64::from(fx.mem.read_u8(sp + 0x25D)) << 2)
        | (u64::from(fx.mem.read_u8(sp + 0x25E)) << 4)
        | (u64::from(fx.mem.read_u8(sp + 0x25F)) << 6);
    fx.desc_alpha(d, alpha);
    let a = fx.mem.read_u8(g + 0x13);
    for o in [0x21Bu32, 0x203, 0x1EB, 0x1D3] {
        fx.mem.write_u8(sp + o, a);
    }
    let mut steps = fx.mem.u8(g + 8) >> 1;
    let e = fx.mem.u8(g + 2) - fx.mem.u8(g) + 1;
    if e < steps {
        steps = e;
    }
    let count = fx.mem.u8(g);
    if count < steps {
        steps = count;
    }
    for sign in 0..2 {
        for s in 0..steps {
            let (mut dx, mut dy) = (fx.mem.u8(g + 0xA), fx.mem.u8(g + 0xB));
            if sign == 0 {
                (dx, dy) = (-dx, -dy);
            }
            let ox = fx.mult(dx, s + 1) << 4;
            let oy = dy.wrapping_mul(s + 1) << 4;
            let v = fixu(fx, at, l.fix[0], ee::add(28672.0, ee::from_i32(ox)));
            fx.mem.write_u16(sp + 0x210, v);
            fx.mem.write_u16(sp + 0x1E0, v);
            let v = fixu(fx, at, l.fix[1], ee::add(36864.0, ee::from_i32(ox)));
            fx.mem.write_u16(sp + 0x228, v);
            fx.mem.write_u16(sp + 0x1F8, v);
            let v = fixu(fx, at, l.fix[2], ee::add(28672.0, ee::from_i32(oy)));
            fx.mem.write_u16(sp + 0x1FA, v);
            fx.mem.write_u16(sp + 0x1E2, v);
            let v = fixu(fx, at, l.fix[3], ee::add(36864.0, ee::from_i32(oy)));
            fx.mem.write_u16(sp + 0x22A, v);
            fx.mem.write_u16(sp + 0x212, v);
            let w = fx.mem.f(sp + 0x258);
            fx.fix_residue(w, at + l.fix[4] + 8);
            let key = ee::fixunssfsi(w) as i32;
            fx.submit(d, 0, key);
        }
    }
}

/// 92429ed597fc [cards 330 0x1E04040 / 540 0x1E042A0, 524 words] (helper
/// C27) [F]: `jitter(recs, ot, n) -> 1` in a packet layer (0x2078A0;
/// closed and linked into `ot` at key 0 by 0x207910 only when a unit was
/// drawn) steps and draws `n` records (0x42C bytes; a 0x1F0 frame, the
/// descriptor at sp + 0x150: RGBAQ / ST / XYZF2 x 4, echoes' 0x70 lower).
/// Per record: countdown 0x101AF0; the hold +0xF, when out, flips the
/// phase +8 and reloads as (rand >> 4 & (+0x11 leaving phase 1, else
/// +0x10)) + 1, then counts down. Once per call echoes' descriptor (colours
/// 0x80, Q 1.0, far Z), TEXFLUSH, the unit +0x14 drawn into the target
/// (0x1F8270, filter 0), TEXFLUSH, flags | 0x800 and the corner UVs. The
/// camera (0x20E2B0 into sp + 0xD0), the eye copied to sp + 0x120 (unread),
/// the unit's point (0x1F65D0 kind 1) must project (0x204D80; no window
/// test); its z on every vertex, the blend +0xC (0x101C80 / 0x207E20),
/// colour (+0x1A, +0x19, +0x18, +0x1B). Then strips s = 0, +0x12, .. <
/// 0x200: the row's offset +0x2C + 2 s, when 0, becomes (rem(rand >> 4,
/// +0xE) 200 - (+0xE 200 >> 1)) / 100 (0x1A2B80) with chance +0xD / 256
/// (rand >> 4 & 0xFF < +0xD), else is cleared; UVs u = 16 s .. 16 (s +
/// +0x12), v 0 .. 0x2000; x the same + 28672, y 0x7000 .. 0x9000 in phase
/// 0, else 28672 + 16 offset .. + 16 (offset + 0x200) (0x1A2C70), keyed by
/// the projection's w, no ordering table. `s0` / `a3` are the caller's
/// (rand's frames save s0; the first TEXFLUSH reports a2 = n / a3 until a
/// rand runs, then __muldi3's low product word).
#[allow(clippy::too_many_arguments)]
pub fn jitter(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, s0: u32, a3: u32) -> i32 {
    let sp = fx.enter(0x1F0);
    fx.mem.write_u32(sp + 0xC8, ot);
    fx.mem.set32(sp + 0xC4, n);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xAC, layer);
    fx.mem.write_u32(sp + 0xB0, 0);
    let mut st = Jit { s0, a2a3: None };
    let mut g = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x42C;
        }
        jitter_one(fx, at, sp, g, (n, a3), &mut st);
        i += 1;
    }
    if fx.mem.read_u32(sp + 0xB0) != 0 {
        let (layer, ot) = (fx.mem.read_u32(sp + 0xAC), fx.mem.read_u32(sp + 0xC8));
        fx.layer_end(layer, ot, 0);
    }
    fx.leave(0x1F0);
    1
}

/// [`jitter`]'s registers across records: s0 (what rand saves) and the
/// a2 / a3 the last rand left.
struct Jit {
    s0: u32,
    a2a3: Option<u32>,
}

fn jit_rand(fx: &mut Fx, st: &mut Jit, ra: u32) -> i32 {
    let r = fx.rand_from(st.s0, ra);
    st.a2a3 = Some((fx.rand_state() as u32).wrapping_sub(1));
    r
}

fn jitter_one(fx: &mut Fx, at: u32, sp: u32, g: u32, (n, a3): (i32, u32), st: &mut Jit) {
    if countdown(fx, g) == 0 {
        return;
    }
    if fx.mem.read_u8(g + 0xF) == 0 {
        let v = if fx.mem.read_u32(g + 8) != 0 {
            fx.mem.write_u32(g + 8, 0);
            let r = jit_rand(fx, st, at + 0x98);
            ((r >> 4) as u8 & fx.mem.read_u8(g + 0x11)).wrapping_add(1)
        } else {
            fx.mem.write_u32(g + 8, 1);
            let r = jit_rand(fx, st, at + 0xC4);
            ((r >> 4) as u8 & fx.mem.read_u8(g + 0x10)).wrapping_add(1)
        };
        fx.mem.write_u8(g + 0xF, v);
    }
    let v = fx.mem.read_u8(g + 0xF).wrapping_sub(1);
    fx.mem.write_u8(g + 0xF, v);
    let d = sp + 0x150;
    if fx.mem.read_u32(sp + 0xB0) == 0 {
        st.s0 = d + 2;
        fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
        for k in [3u32, 2, 1, 0] {
            for o in [0x1A8u32, 0x190, 0x178, 0x160] {
                fx.mem.write_u8(sp + o + k, 0x80);
            }
        }
        for o in [0x1ACu32, 0x194, 0x17C, 0x164] {
            fx.mem.write_u32(sp + o, 0x3F80_0000);
        }
        let a1 = fx.mem.read_u32(sp + 0x188);
        let a0 = fx.mem.read_u32(sp + 0x170);
        jit_chain_z(fx, sp, 0xFF_FFFF);
        let (a2, a3) = match st.a2a3 {
            Some(v) => (v, v),
            None => (n as u32, a3),
        };
        fx.texflush([a0, a1, a2, a3]);
        let h = fx.mem.i32(g + 0x14);
        let (_, regs) = fx.unit_draw(h, 0, sp + 0x1C8, sp + 0x1D0);
        fx.texflush(regs);
        fx.mem.write_u32(sp + 0xB0, 1);
        let f = fx.mem.read_u16(d + 2);
        fx.mem.write_u16(d + 2, f | 0x800);
        uv(fx, sp + 0x168, 0, 0);
        uv(fx, sp + 0x180, 0x2000, 0);
        uv(fx, sp + 0x198, 0, 0x2000);
        uv(fx, sp + 0x1B0, 0x2000, 0x2000);
    }
    camera_vp_into(fx, sp + 0xD0);
    let eye = fx.camera_eye(0);
    for c in 0..4u32 {
        let v = fx.mem.read_u32(eye + 4 * c);
        fx.mem.write_u32(sp + 0x120 + 4 * c, v);
    }
    let h = fx.mem.i32(g + 0x14);
    fx.unit_point(1, h, sp + 0x110);
    let m = fx.read_m4(sp + 0xD0);
    if !fx.project_at(sp + 0x110, &m, sp + 0x1E0, sp + 0x1EC) {
        return;
    }
    st.s0 = sp + 0x170;
    let z = fx.xyz_z(sp + 0x1E0);
    jit_chain_z(fx, sp, z);
    let b = fx.mem.read_u8(g + 0xC);
    fx.mem.write_u8(sp + 0x135, b);
    alpha_mode(fx, sp + 0x130, sp + 0x1E8);
    let alpha = u64::from(fx.mem.read_u8(sp + 0x1E8))
        | (u64::from(fx.mem.read_u8(sp + 0x1E9)) << 2)
        | (u64::from(fx.mem.read_u8(sp + 0x1EA)) << 4)
        | (u64::from(fx.mem.read_u8(sp + 0x1EB)) << 6);
    fx.desc_alpha(d, alpha);
    for (src, k) in [(0x1Bu32, 3u32), (0x1A, 0), (0x19, 1), (0x18, 2)] {
        let v = fx.mem.read_u8(g + src);
        for o in [0x1A8u32, 0x190, 0x178, 0x160] {
            fx.mem.write_u8(sp + o + k, v);
        }
    }
    let mut s = 0i32;
    while s < 0x200 {
        let row = g.wrapping_add(0x2C).wrapping_add((2 * s) as u32);
        if fx.mem.read_u16(row) as i16 == 0 {
            let r = jit_rand(fx, st, at + 0x514);
            if ((r >> 4) & 0xFF) < fx.mem.u8(g + 0xD) {
                let m = fx.mem.u8(g + 0xE);
                let r = jit_rand(fx, st, at + 0x538);
                let rem = fx.div(r >> 4, m).1;
                let v = rem.wrapping_mul(200).wrapping_sub(m.wrapping_mul(200) >> 1);
                let x = fix(fx, at, 0x5A4, ee::div(ee::from_i32(v), 100.0));
                fx.mem.write_u16(row, x as u16);
            }
        } else {
            fx.mem.write_u16(row, 0);
        }
        let step = fx.mem.u8(g + 0x12);
        let (u0, u1) = (s << 4, s.wrapping_add(step) << 4);
        uv(fx, sp + 0x168, u0, 0);
        uv(fx, sp + 0x180, u1, 0);
        uv(fx, sp + 0x198, u0, 0x2000);
        uv(fx, sp + 0x1B0, u1, 0x2000);
        if fx.mem.read_u32(g + 8) == 0 {
            for o in [0x18Au32, 0x172] {
                fx.mem.write_u16(sp + o, 0x7000);
            }
            for o in [0x1BAu32, 0x1A2] {
                fx.mem.write_u16(sp + o, 0x9000);
            }
        } else {
            let off = i32::from(fx.mem.read_u16(row) as i16);
            let y = fixu(fx, at, 0x6D4, ee::add(28672.0, ee::from_i32(off << 4)));
            fx.mem.write_u16(sp + 0x18A, y);
            fx.mem.write_u16(sp + 0x172, y);
            let off = i32::from(fx.mem.read_u16(row) as i16);
            let y = fixu(fx, at, 0x70C, ee::add(28672.0, ee::from_i32((off + 0x200) << 4)));
            fx.mem.write_u16(sp + 0x1BA, y);
            fx.mem.write_u16(sp + 0x1A2, y);
        }
        let x = fixu(fx, at, 0x738, ee::add(28672.0, ee::from_i32(u0)));
        fx.mem.write_u16(sp + 0x1A0, x);
        fx.mem.write_u16(sp + 0x170, x);
        let step = fx.mem.u8(g + 0x12);
        let x = fixu(fx, at, 0x770, ee::add(28672.0, ee::from_i32(s.wrapping_add(step) << 4)));
        fx.mem.write_u16(sp + 0x1B8, x);
        fx.mem.write_u16(sp + 0x188, x);
        let w = fx.mem.f(sp + 0x1EC);
        fx.fix_residue(w, at + 0x784 + 8);
        let key = ee::fixunssfsi(w) as i32;
        fx.submit(d, 0, key);
        let step = fx.mem.u8(g + 0x12);
        s = s.wrapping_add(step);
    }
}

/// z of [`jitter`]'s four vertices from `z` down the chain (vertex 4
/// first), their F bytes cleared.
fn jit_chain_z(fx: &mut Fx, sp: u32, z: u64) {
    fx.set_xyz_z(sp + 0x1B8, z);
    for (dst, src) in [(0x1A0u32, 0x1B8u32), (0x188, 0x1A0), (0x170, 0x188)] {
        let z = fx.xyz_z(sp + src);
        fx.set_xyz_z(sp + dst, z);
    }
    for o in [0x1BFu32, 0x1A7, 0x18F, 0x177] {
        fx.mem.write_u8(sp + o, 0);
    }
}
