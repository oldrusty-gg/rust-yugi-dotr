//! BMODULE b147 (card 311), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E022B0, data area 0x1ED0000) [F] (helper C7, 31
//! Oct 2026): two swaths (+0x70 / +0x1A0, `blib::swath`) between the
//! attacker's nodes 0x43 / 0x3C and 0x31 / 0x2A, drawn from update 0x6F;
//! the stage dims until 0x7C; a full-window tint (+0x1014) on the even
//! updates 0x66..0x72; two discs at the nodes 0x42 / 0x30 on 0x5A..0x7B;
//! from 0x73 a disc (+0x640) and 16 jagged strands (+0x2D0 + 0x30 k,
//! `blib::strand`) from the target to random points, and a wave (+0x7C0)
//! at the target. Returns 1 on update 0x69, 6 on 0x73..0x7B, 3 on 0x7C, 8
//! after 0xA0.
//!
//! Library at its own addresses: `colours4` (0x1E00080), the tint
//! (0x1E00120), `swath` push / draw (0x1E00280 / 0x1E003D0), `disc`
//! (0x1E00BD0), `wave::draw` (0x1E01130), `strand::draw` (0x1E01930).
use crate::b186::{bytes, halves};
use crate::blib::{strand, swath, wave};
use crate::bus::Bus;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const STRAND: u32 = 0x01E0_1930;

/// The entry 0x1E022B0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xC0);
    ret
}

pub(crate) fn node(fx: &mut Fx, at: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(D + at);
    fx.node_position(h, s, out);
}

/// A swath (`r`, point arrays `pts`) set up between the nodes at `na` /
/// `nb` (copied to sp + `sa` / + 0x10): texture, flags 1, width 8, cell,
/// colour (+2, +1, +0, +3) `c`; all 8 points at the nodes.
pub(crate) fn swath_init(fx: &mut Fx, sp: u32, r: u32, pts: (u32, u32), na: u32, nb: u32, sa: u32, c: [u8; 4]) {
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(D + r, tex);
    fx.mem.write_u32(D + r + 4, 1);
    halves(fx, &[(r + 0xC, 8), (r + 0x110, 0x8E), (r + 0x112, 0x54), (r + 0x114, 0x20), (r + 0x116, 1)]);
    bytes(fx, &[(r + 0xA, c[0]), (r + 9, c[1]), (r + 8, c[2]), (r + 0xB, c[3])]);
    fx.mem.write_u32(D + r + 0x120, D + pts.0);
    fx.mem.write_u32(D + r + 0x124, D + pts.1);
    node(fx, na, sp + sa);
    node(fx, nb, sp + sa + 0x10);
    for k in 0..8u32 {
        for c in 0..3u32 {
            let a = fx.mem.read_u32(sp + sa + 4 * c);
            fx.mem.write_u32(D + r + 0x10 + 0x10 * k + 4 * c, a);
            let b = fx.mem.read_u32(sp + sa + 0x10 + 4 * c);
            fx.mem.write_u32(D + r + 0x90 + 0x10 * k + 4 * c, b);
        }
    }
    fx.mem.set16(D + r + 0xE, 0);
}

/// 16 strands (+0x2D0 + 0x30 k, ends +0xEF0 + 0x10 k): texture, cell,
/// colour, 16 pieces, jitter 0x14, drawn -(k + 1) (staggered starts), a
/// random end of length 20.
pub(crate) fn strands_init(fx: &mut Fx, sp: u32, c: [u8; 4]) {
    for k in 0..0x10u32 {
        let g = 0x2D0 + 0x30 * k;
        let tex = fx.mem.read_u32(sp + 0xB0);
        fx.mem.write_u32(D + g, tex);
        fx.mem.write_u32(D + g + 4, 0);
        halves(fx, &[(g + 0x10, 0x80), (g + 0x12, 0x40), (g + 0x14, 0x20), (g + 0x16, 0x40)]);
        bytes(fx, &[(g + 0xA, c[0]), (g + 9, c[1]), (g + 8, c[2]), (g + 0xB, c[3])]);
        halves(fx, &[(g + 0xC, 0x10), (g + 0x28, -(k as i32 + 1)), (g + 0xE, 0x14)]);
        fx.mem.write_u32(D + g + 0x20, 0);
        fx.mem.write_u32(D + g + 0x24, 0);
        fx.random_dir(D + 0xEF0 + 0x10 * k, 20.0);
    }
}

/// The strands drawn from sp + 0xA0 (0) through `x`, fading by 8; one
/// whose drawn count wrapped to 0 gets a new random end.
pub(crate) fn strands_step(fx: &mut Fx, at: u32, sp: u32, x: u32) {
    for o in [0xA8u32, 0xA4, 0xA0] {
        fx.mem.write_u32(sp + o, 0);
    }
    for k in 0..0x10u32 {
        let g = D + 0x2D0 + 0x30 * k;
        strand::draw(fx, at, OT, g, x, sp + 0xA0, D + 0xEF0 + 0x10 * k);
        fx.step_down(g + 0xB, 0, 8);
        if fx.mem.i16(g + 0x28) == 0 {
            fx.random_dir(D + 0xEF0 + 0x10 * k, 20.0);
        }
    }
}

/// What b147's init shares with b074's [F]: the attacker's nodes into
/// +0xFFC / +0x1000 / +0x1008 / +0x100C / +0x1004 / +0x1010, the updates
/// left, the stage light byte, the byte table (rand >> 4, rand at
/// `rand_ra`), b074's frame-quad colour +0x1018.
pub(crate) struct Init {
    pub nodes: [i32; 6],
    pub left: i32,
    pub light: u32,
    pub table: u32,
    pub rand_ra: u32,
    pub frame: bool,
}

const B147: Init = Init { nodes: [0x43, 0x3C, 0x31, 0x2A, 0x42, 0x30], left: 0xA0, light: 0x1018, table: 0x1019, rand_ra: 0x01E0_23C8, frame: false };

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    init_with(fx, sp, param, &B147)
}

pub(crate) fn init_with(fx: &mut Fx, sp: u32, param: i32, v: &Init) -> i32 {
    fx.bzero(D, 0x1120);
    for x in [0u32, 0x5D0, 0x650, 0x6D0] {
        fx.xform_init(D + x);
    }
    for (at, n) in [0xFFCu32, 0x1000, 0x1008, 0x100C, 0x1004, 0x1010].into_iter().zip(v.nodes) {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(at == 0xFFC));
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0xFF0, v.left);
    fx.mem.set32(D + 0xFF4, 1);
    fx.mem.set32(D + 0xFF8, param);
    fx.mem.write_u8(D + v.light, 0x80);
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, v.rand_ra);
        fx.mem.write_u8(D + v.table + k, (r >> 4) as u8);
    }
    bytes(fx, &[(0x1016, 0x60), (0x1015, 0x20), (0x1014, 0x90), (0x1017, 0x80)]);
    if v.frame {
        bytes(fx, &[(0x101A, 0x80), (0x1019, 0x80), (0x1018, 0x80), (0x101B, 0x60)]);
    }
    // Discs (+0x640, +0x6C0, +0x740).
    for (g, a, size) in [(0x640u32, 0xFFu8, 0x41A0_0000u32), (0x6C0, 0, 0x4120_0000), (0x740, 0, 0x4120_0000)] {
        fx.mem.write_u32(D + g, 0x8_0000);
        bytes(fx, &[(g + 6, 0xA0), (g + 5, 0x80), (g + 4, 0x80), (g + 7, a)]);
        fx.mem.write_u32(D + g + 8, size);
        halves(fx, &[(g + 0xC, 0x24), (g + 0xE, 0)]);
    }
    fx.texture5(sp + 0xB0, 0);
    strands_init(fx, sp, [0x80; 4]);
    fx.texture4(sp + 0xB0, 0xF);
    swath_init(fx, sp, 0x70, (0x7F0, 0x9B0), 0xFFC, 0x1000, 0x60, [0x80, 0x10, 0x10, 0]);
    swath_init(fx, sp, 0x1A0, (0xB70, 0xD30), 0x1008, 0x100C, 0x80, [0x80, 0x10, 0x10, 0]);
    // The wave (+0x7C0).
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(D + 0x7C0, tex);
    fx.mem.write_u32(D + 0x7CC, D + v.table);
    fx.mem.write_u32(D + 0x7C4, 0x2000_0000);
    halves(fx, &[(0x7D0, 0xB0), (0x7D2, 0x40), (0x7D4, 0x20), (0x7D6, 0x20)]);
    bytes(fx, &[(0x7CA, 0x80), (0x7C9, 0x10), (0x7C8, 0x10), (0x7CB, 0xFF)]);
    fx.mem.write_u32(D + 0x7E0, 0x3F4C_CCCD);
    fx.mem.write_u32(D + 0x7E4, 0x3CF5_C28F);
    halves(fx, &[(0x7EA, 0), (0x7E8, 0x100)]);
    fx.mem.write_u8(D + 0x7EC, 7);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0xFF4);
    node(fx, 0xFFC, sp + 0x60);
    node(fx, 0x1000, sp + 0x70);
    swath::push(fx, D + 0x70, sp + 0x60, sp + 0x70);
    node(fx, 0x1008, sp + 0x80);
    node(fx, 0x100C, sp + 0x90);
    swath::push(fx, D + 0x1A0, sp + 0x80, sp + 0x90);
    for out in [0x5F0u32, 0x770] {
        let t = fx.target_handle();
        fx.unit_position(t, D + out);
    }
    node(fx, 0x1004, D + 0x6F0);
    node(fx, 0x1010, D + 0x670);
    // s0 = D, s1 = the OT, s2 = sp + 0x90, s3 the runner's (0).
    let regs = [Some(D), Some(OT), Some(sp + 0x90), Some(0)];
    fx.xform_world_saving(D, 0x01E0_283C, regs);
    for (x, ra) in [(0x5D0u32, 0x01E0_2844u32), (0x750, 0x01E0_284C), (0x650, 0x01E0_2854), (0x6D0, 0x01E0_285C)] {
        fx.xform_billboard_saving(D + x, ra, regs);
    }
    if n(fx) < 0x7C {
        fx.step_down(D + 0x1018, 0x20, 4);
    } else {
        fx.step_up(D + 0x1018, 0x80, 4);
    }
    let l = fx.mem.read_u8(D + 0x1018);
    fx.set_stage_light(i32::from(l));
    let m = n(fx);
    if (0x66..0x73).contains(&m) && m.wrapping_rem(2) == 0 {
        crate::b262::tint(fx, D + 0x1014);
    }
    if (0x5A..0x7C).contains(&n(fx)) {
        fx.disc(D + 0x6C0, D + 0x650);
        fx.step_up(D + 0x6C7, 0xFF, 0x10);
        fx.disc(D + 0x740, D + 0x6D0);
        fx.step_up(D + 0x747, 0xFF, 0x10);
    }
    if n(fx) >= 0x73 {
        fx.disc(D + 0x640, D + 0x5D0);
        fx.step_down(D + 0x647, 0, 0x10);
        strands_step(fx, STRAND, sp, D + 0x5D0);
        fx.approach(D + 0x750, f32::from_bits(0x3E99_999A), f32::from_bits(0x3DCC_CCCD));
        wave::draw(fx, OT, D + 0x7C0, D + 0x750);
        fx.step_down(D + 0x7CB, 0, 0x10);
    }
    if n(fx) >= 0x6F {
        swath::draw(fx, D + 0x70, D);
        fx.step_up(D + 0x7B, 0xFF, 0x10);
        swath::draw(fx, D + 0x1A0, D);
        fx.step_up(D + 0x1AB, 0xFF, 0x10);
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0xFF4, m);
    let left = fx.mem.i32(D + 0xFF0) - 1;
    fx.mem.set32(D + 0xFF0, left);
    if m == 0x7C {
        3
    } else if m == 0x69 {
        1
    } else if (0x73..0x7C).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
