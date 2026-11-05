//! BMODULEs b036 (card 23), b464 (card 280), b635 (card 290) and b360
//! (card 572): one program (entry 0x1E044A0, b360 0x1E050B0; data area
//! 0x1ED0000; compiled without delay-slot scheduling), hand-ported from the
//! records [F]; they differ in node ids, colours,
//! timings and a few branches ([`Variant`]); b360 adds a quad trail
//! (+0x2D0, `blib::quadtrail`, 0x1E044A0 / 0x1E046A0) and moves the rest of
//! its data up, and has no stage light. Two swaths (+0x70 / +0x1A0, `blib::swath`)
//! between the attacker's nodes; a frame quad and the object op 0x1F83B0
//! on their stretch of updates; then a full-window tint (+0x1AC0, b262's
//! 0x1E00120), a particle cloud (+0x8A0), a square cloud (+0x12D0,
//! `pointquad::squares`), point quads (+0x840) and a quad group (+0x340)
//! growing at the target; later four waves (+0x18E0 + 0x30 k,
//! `wave::draw`), two rings (+0xA60 / +0xE60) swelling and two more
//! particle clouds (+0x1640 / +0x1790). The stage dims (0x1F7F10).
//!
//! Library at its own addresses: `colours4` (0x1E00080), the tint
//! (0x1E00120), `pointquad` setup / draw / squares (0x1E00280 / 0x1E005C0
//! / 0x1E011A0), `ring_setup` / `ring_draw` (0x1E01780 / 0x1E01A40),
//! `particle_quads` (0x1E01F30), `wave::draw` (0x1E02AC0), the frame quad
//! (0x1E032C0, b26f4b13c68d), `swath` push / draw (0x1E033E0 /
//! 0x1E03530), `quads_setup` / `quads_draw` (0x1E03D30 / 0x1E03E90).
use crate::b186::{bytes, halves};
use crate::blib::{pointquad, quadtrail, swath, wave};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SETUP: u32 = 0x01E0_0280;
const TRAIL_DRAW: u32 = 0x01E0_46A0;

/// What differs between the three records [F].
pub struct Variant {
    /// The entry and its words (to find its call sites).
    entry: u32,
    words: u32,
    /// The data moved up: offsets 0x2D0..0x19C0 by `shift.0`, from 0x19C0
    /// by `shift.1` (b360's quad trail sits at +0x2D0).
    shift: (u32, u32),
    /// The attacker's nodes requested into +0x19AC (clear 1), +0x19B0..
    nodes: &'static [i32],
    /// +0x19A0, the updates left.
    left: i32,
    /// The byte table +0x19C0: rand >> 4 (b464), else rand % 0xFF + 1.
    bytes_shift: bool,
    /// The square cloud's size +0x12DC.
    square_size: u32,
    /// Colours (+2, +1, +0, +3) of the squares, waves, quads, rings,
    /// point quads, the two late clouds and the swaths.
    c_squares: [u8; 4],
    c_waves: [u8; 4],
    wave_size: u32,
    c_quads: [u8; 4],
    c_ring_a: [u8; 4],
    c_ring_b: [u8; 4],
    c_points: [u8; 4],
    cloud_flags: (u32, u32),
    cloud_sizes: (u32, u32),
    c_clouds: ([u8; 4], [u8; 4]),
    c_swath: [u8; 4],
    template: u32,
    /// The swaths' updates (none: every update), their narrow (width 5)
    /// stretch and the update they are filled.
    swath_gate: Option<(i32, i32)>,
    narrow: (i32, i32),
    fill_at: i32,
    /// b464: the four positions are the attacker's node +0x19AC while
    /// before this update (else the target's position every update).
    node_until: Option<i32>,
    /// The stage light: dims before `dim_until`, brightens on `up` (b635:
    /// up from `up.0`, else dims, then set).
    dim_until: i32,
    up: (i32, i32),
    light_after: bool,
    /// b360: no stage light; its quad trail (+0x2D0) pushed at the 7th
    /// node (+0x19C4 shifted) on `trail.0` and the 6th on `trail.1`, drawn
    /// (its cells fading) before update `trail.2`.
    no_light: bool,
    trail: Option<(&'static [i32], &'static [i32], i32)>,
    frame_quad: &'static [(i32, i32)],
    object_op: (i32, i32),
    /// The swaths drawn on these updates; b036 fades both after the draws
    /// (in before `fade_in_until`), the others fade each in after its own.
    swath_draw: (i32, i32),
    fade_in_until: Option<i32>,
    tint_from: i32,
    squares_from: i32,
    waves: (i32, i32),
    clouds_fade: i32,
    two_at: i32,
    one: (i32, i32),
    six: (i32, i32),
}

pub const B036: Variant = Variant {
    entry: 0x01E0_44A0,
    shift: (0, 0),
    nodes: &[0x6B, 0x60, 0x59, 0x74, 0x70],
    left: 0xDE,
    bytes_shift: false,
    words: 1304,
    square_size: 0x3FC0_0000,
    c_squares: [0x80, 0x70, 0x40, 0x80],
    c_waves: [0x80, 0x20, 0x20, 0x80],
    wave_size: 0x3F19_999A,
    c_quads: [0x40, 0, 0, 0x20],
    c_ring_a: [0x90, 0x50, 0x20, 0x20],
    c_ring_b: [0x90, 0x50, 0x20, 0x30],
    c_points: [0x40, 0, 0, 0x60],
    cloud_flags: (0x8_0005, 0x8_0007),
    cloud_sizes: (0x4100_0000, 0x4120_0000),
    c_clouds: ([0x40, 0, 0, 0x40], [0x80, 0x20, 0x20, 0x40]),
    c_swath: [0xD0, 0xD0, 0x70, 0],
    template: 0x01E0_5A00,
    swath_gate: Some((0x17, 0x99)),
    narrow: (0x3E, 0xC9),
    fill_at: 0x3E,
    node_until: None,
    dim_until: 0x17,
    up: (0xC8, 0xDD),
    light_after: false,
    no_light: false,
    trail: None,
    frame_quad: &[(0x18, 0xDC)],
    object_op: (0x3E, 0xC9),
    swath_draw: (0x3E, 0x99),
    fade_in_until: Some(0x82),
    tint_from: 0x4F,
    squares_from: 0x55,
    waves: (0x99, 0xC9),
    clouds_fade: 0xC1,
    two_at: 0xC8,
    one: (0x45, 0x90),
    six: (0x99, 0xC8),
};

pub const B464: Variant = Variant {
    entry: 0x01E0_44A0,
    shift: (0, 0),
    nodes: &[0xC, 0x15, 0x11, 0x1B, 0x17],
    left: 0xF2,
    bytes_shift: true,
    words: 1304,
    square_size: 0x3F40_0000,
    c_squares: [0x80, 0x70, 0x40, 0x80],
    c_waves: [0x80, 0x20, 0x20, 0x80],
    wave_size: 0x3E99_999A,
    c_quads: [0x40, 0, 0, 0x20],
    c_ring_a: [0x90, 0x50, 0x20, 0x20],
    c_ring_b: [0x90, 0x50, 0x20, 0x30],
    c_points: [0x40, 0, 0, 0x60],
    cloud_flags: (5, 7),
    cloud_sizes: (0x40C0_0000, 0x4100_0000),
    c_clouds: ([0x40, 0, 0, 0x40], [0x80, 0x20, 0x20, 0x40]),
    c_swath: [0x20, 0x20, 0x60, 0],
    template: 0x01E0_5A00,
    swath_gate: Some((0x38, 0xA8)),
    narrow: (0x6E, 0xC9),
    fill_at: 0x6E,
    node_until: Some(0x89),
    dim_until: 0x38,
    up: (0xC8, 0xF1),
    light_after: false,
    no_light: false,
    trail: None,
    frame_quad: &[(0x39, 0x6E), (0xA8, 0xF1)],
    object_op: (0x6E, 0xC9),
    swath_draw: (0x38, 0xA8),
    fade_in_until: None,
    tint_from: 0x89,
    squares_from: 0x8F,
    waves: (0xA8, 0xC9),
    clouds_fade: 0xC1,
    two_at: 0xC8,
    one: (0x7F, 0x9F),
    six: (0xA8, 0xC8),
};

pub const B635: Variant = Variant {
    entry: 0x01E0_44A0,
    shift: (0, 0),
    nodes: &[0x2A, 0x32, 0x2D, 0x40, 0x3B],
    left: 0xEC,
    bytes_shift: false,
    words: 1272,
    square_size: 0x3F40_0000,
    c_squares: [0x80, 0x80, 0x80, 0x80],
    c_waves: [0x80, 0x40, 0x60, 0x80],
    wave_size: 0x3E99_999A,
    c_quads: [0x60, 0x40, 0x80, 0x20],
    c_ring_a: [0x60, 0x40, 0x80, 0x20],
    c_ring_b: [0x60, 0x40, 0x80, 0x30],
    c_points: [0x60, 0x40, 0x80, 0x60],
    cloud_flags: (5, 7),
    cloud_sizes: (0x40C0_0000, 0x4100_0000),
    c_clouds: ([0x60, 0x40, 0x80, 0x40], [0x60, 0x40, 0x80, 0x40]),
    c_swath: [0x20, 0x20, 0x60, 0],
    template: 0x01E0_5980,
    swath_gate: None,
    narrow: (0x46, 0x88),
    fill_at: 0x46,
    node_until: None,
    dim_until: 0,
    up: (0xD9, i32::MAX),
    light_after: true,
    no_light: false,
    trail: None,
    frame_quad: &[(0x46, 0xD8)],
    object_op: (i32::MIN, 0xD8),
    swath_draw: (0x46, i32::MAX),
    fade_in_until: None,
    tint_from: 0x95,
    squares_from: 0x95,
    waves: (0x95, 0xD9),
    clouds_fade: 0xD1,
    two_at: 0xD8,
    one: (0x8B, 0x8C),
    six: (0x95, 0xD8),
};

pub const B360: Variant = Variant {
    entry: 0x01E0_50B0,
    shift: (0x1250, 0x1258),
    nodes: &[0x1D, 0x26, 0x1F, 0x2F, 0x28, 0xC, 8],
    left: 0xDE,
    bytes_shift: false,
    words: 1428,
    square_size: 0x3FC0_0000,
    c_squares: [0x80, 0x70, 0x40, 0x80],
    c_waves: [0x80, 0x20, 0x20, 0x80],
    wave_size: 0x3F19_999A,
    c_quads: [0x40, 0, 0, 0x20],
    c_ring_a: [0x90, 0x50, 0x20, 0x20],
    c_ring_b: [0x90, 0x50, 0x20, 0x30],
    c_points: [0x40, 0, 0, 0x60],
    cloud_flags: (0x8_0005, 0x8_0007),
    cloud_sizes: (0x4100_0000, 0x4120_0000),
    c_clouds: ([0x40, 0, 0, 0x40], [0x80, 0x20, 0x20, 0x40]),
    c_swath: [0xD0, 0x70, 0xD0, 0],
    template: 0x01E0_6800,
    swath_gate: Some((0x17, 0x85)),
    narrow: (0x46, 0xC9),
    fill_at: 0x46,
    node_until: None,
    dim_until: 0,
    up: (0, 0),
    light_after: false,
    no_light: true,
    trail: Some((&[0x4B, 0x5F, 0x73, 0x87], &[0x41, 0x55, 0x69, 0x7D], 0x73)),
    frame_quad: &[(0x20, 0xDC)],
    object_op: (0x1F, 0xC9),
    swath_draw: (0x46, 0x85),
    fade_in_until: Some(0x82),
    tint_from: 0x73,
    squares_from: 0x79,
    waves: (0x85, 0xC9),
    clouds_fade: 0xC1,
    two_at: 0xC8,
    one: (0x69, 0x7C),
    six: (0x85, 0xC8),
};

/// b036's entry 0x1E044A0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B036)
}

/// b464's entry 0x1E044A0.
pub fn program_b464(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B464)
}

/// b635's entry 0x1E044A0.
pub fn program_b635(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B635)
}

/// b360's entry 0x1E050B0 (card 572).
pub fn program_b360(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B360)
}

/// The return addresses of the entry's `jal target` sites, in order.
fn sites(fx: &mut Fx, v: &Variant, target: u32) -> Vec<u32> {
    let jal = 0x0C00_0000 | ((target >> 2) & 0x03FF_FFFF);
    (0..v.words).map(|k| v.entry + 4 * k).filter(|&a| fx.mem.read_u32(a) == jal).map(|a| a + 8).collect()
}

fn run(fx: &mut Fx, param: i32, v: &Variant) -> i32 {
    let sp = fx.enter(0xB0);
    let ret = if param >= 0 { init(fx, sp, param, v) } else { step(fx, sp, v) };
    fx.leave(0xB0);
    ret
}

fn hw(fx: &mut Fx, b: u32, at: &[(u32, i32)]) {
    for &(o, v) in at {
        fx.mem.set16(b + o, v);
    }
}

fn bw(fx: &mut Fx, b: u32, at: &[(u32, u8)]) {
    for &(o, v) in at {
        fx.mem.write_u8(b + o, v);
    }
}

fn colour(fx: &mut Fx, at: u32, c: [u8; 4]) {
    bw(fx, at, &[(2, c[0]), (1, c[1]), (0, c[2]), (3, c[3])]);
}

fn node(fx: &mut Fx, at: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(at);
    fx.node_position(h, s, out);
}

/// rand % m - sub as a float (rand at `ra`).
fn rand_f(fx: &mut Fx, ra: u32, m: i32, sub: i32) -> f32 {
    let r = fx.rand_from(D, ra);
    let (_, h) = fx.div(r, m);
    ee::from_i32(h - sub)
}

/// A particle cloud's particles (`n` at `g` + 0x30 + 0x10 k; count `g` +
/// 0x130): x / y = rand % 0x14 - 10, frame -(+0x13A) k, tick 0, +0x13E =
/// +0x140 - 1.
fn cloud8(fx: &mut Fx, g: u32, ra: [u32; 2]) {
    for k in 0..8i32 {
        let p = g + 0x10 * k as u32;
        let x = rand_f(fx, ra[0], 0x14, 0xA);
        fx.mem.setf(p + 0x30, x);
        let y = rand_f(fx, ra[1], 0x14, 0xA);
        fx.mem.setf(p + 0x34, y);
        let a = fx.mem.i16(g + 0x13A).wrapping_neg();
        let f = fx.mult(a, k);
        fx.mem.set16(p + 0x3E, f);
        fx.mem.set16(p + 0x3C, 0);
        let c = fx.mem.i16(g + 0x140) - 1;
        fx.mem.set16(g + 0x13E, c);
    }
}

/// A swath (`r`) set up between the attacker's nodes at `na` / `nb`
/// (copied into sp + `sa` / `sa` + 0x10): texture, flags 1, width 8,
/// cell, colour, the two point arrays; all 8 points at the nodes.
fn swath_init(fx: &mut Fx, sp: u32, v: &Variant, r: u32, pts: (u32, u32), na: u32, nb: u32, sa: u32) {
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + r, tex);
    fx.mem.write_u32(D + r + 4, 1);
    halves(fx, &[(r + 0xC, 8), (r + 0x110, 0x8E), (r + 0x112, 0x54), (r + 0x114, 0x20), (r + 0x116, 1)]);
    colour(fx, D + r + 8, v.c_swath);
    fx.mem.write_u32(D + r + 0x120, pts.0);
    fx.mem.write_u32(D + r + 0x124, pts.1);
    node(fx, na, sp + sa);
    node(fx, nb, sp + sa + 0x10);
    swath_fill(fx, sp, r, sa);
}

fn swath_fill(fx: &mut Fx, sp: u32, r: u32, sa: u32) {
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

fn init(fx: &mut Fx, sp: u32, param: i32, v: &Variant) -> i32 {
    let (b1, b2) = (D + v.shift.0, D + v.shift.1);
    let rs = sites(fx, v, 0x0019_AE88);
    fx.bzero(D, 0x21D0 + v.shift.0);
    for x in [D, b1 + 0x2D0, b1 + 0x1260, b1 + 0x15D0, b1 + 0x9F0] {
        fx.xform_init(x);
    }
    for (k, &n) in v.nodes.iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set32(b1 + 0x19AC + 4 * k as u32, r);
    }
    fx.mem.set32(b1 + 0x19A0, v.left);
    fx.mem.set32(b1 + 0x19A4, 1);
    fx.mem.set32(b1 + 0x19A8, param);
    for o in [0x15C0u32, 0x15C4, 0x15C8] {
        fx.mem.write_u32(b1 + o, 0x3F85_1EB8);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, rs[0]);
        let b = if v.bytes_shift { r >> 4 } else { fx.div(r, 0xFF).1 + 1 };
        fx.mem.write_u8(b2 + 0x19C0 + k, b as u8);
    }
    bw(fx, b2, &[(0x1AC1, 0x80), (0x1AC2, 0x80), (0x1AC0, 0xB0), (0x1AC3, 0xFF), (0x1AC4, 0x80)]);
    fx.texture5(sp + 0xA0, 0);
    // The particle cloud (+0x8A0).
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(b1 + 0x8A0, tex);
    fx.mem.write_u32(b1 + 0x8A4, 0x8_0087);
    for o in [0x8B0u32, 0x8B4, 0x8B8, 0x8BC] {
        fx.mem.write_u32(b1 + o, 0);
    }
    hw(fx, b1, &[(0x8C8, 0), (0x8CA, 0), (0x8C4, 0x20), (0x8C6, 0x20), (0x8CC, 0x80), (0x8CE, 0x40)]);
    fx.mem.write_u32(b1 + 0x8AC, 0x4100_0000);
    hw(fx, b1, &[(0x9D0, 8), (0x9DA, 3), (0x9DC, 1), (0x9E0, 8), (0x9D6, 0xA), (0x9D8, 0xA)]);
    bw(fx, b1, &[(0x8AA, 0x80), (0x8A9, 0x80), (0x8A8, 0x80), (0x8AB, 0x60)]);
    for k in 0..0x10i32 {
        let p = b1 + 0x10 * k as u32;
        let x = ee::add(fx.mem.f(b1 + 0x8B0), rand_f(fx, rs[1], 0xA, 5));
        fx.mem.setf(p + 0x8D0, x);
        let r = fx.rand_from(D, rs[2]);
        let y = ee::add(fx.mem.f(b1 + 0x8B4), ee::from_i32(r.wrapping_rem(8) - 2));
        fx.mem.setf(p + 0x8D4, y);
        let z = fx.mem.read_u32(b1 + 0x8B8);
        fx.mem.write_u32(p + 0x8D8, z);
        let a = fx.mem.i16(b1 + 0x9DA).wrapping_neg();
        let f = fx.mult(a, k);
        fx.mem.set16(p + 0x8DE, f);
        fx.mem.set16(p + 0x8DC, 0);
        let c = fx.mem.i16(b1 + 0x9E0) - 1;
        fx.mem.set16(b1 + 0x9DE, c);
    }
    fx.texture4(sp + 0xA0, 0xF);
    // The square cloud (+0x12D0, 60 points at +0x12F0 + 12 k).
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(b1 + 0x12D0, tex);
    fx.mem.write_u32(b1 + 0x12DC, v.square_size);
    for k in 0..0x3Cu32 {
        let p = b1 + 0xC * k;
        if bit(fx.mem.read_u32(b1 + 0x12D4), 0x1D) {
            let x = rand_f(fx, rs[3], 0x64, 0x32);
            fx.mem.setf(p + 0x12F0, x);
            let z = rand_f(fx, rs[4], 0x32, 0);
            fx.mem.setf(p + 0x12F8, z);
            let y = rand_f(fx, rs[5], 0x64, 0x32);
            fx.mem.setf(p + 0x12F4, y);
        } else {
            let x = rand_f(fx, rs[6], 0x32, 0x19);
            fx.mem.setf(p + 0x12F0, x);
            let y = rand_f(fx, rs[7], 0x19, 0);
            fx.mem.setf(p + 0x12F4, y);
            let z = rand_f(fx, rs[8], 0x32, 0x19);
            fx.mem.setf(p + 0x12F8, z);
        }
    }
    fx.mem.write_u32(b1 + 0x12D4, 1);
    hw(fx, b1, &[(0x12E0, 0xB0), (0x12E2, 0x40), (0x12E4, 0x20), (0x12E6, 0x20)]);
    colour(fx, b1 + 0x12D8, v.c_squares);
    // Four waves (+0x18E0 + 0x30 k).
    for k in 0..4u32 {
        let g = 0x18E0 + 0x30 * k;
        let tex = fx.mem.read_u32(sp + 0xA0);
        fx.mem.write_u32(b1 + g, tex);
        fx.mem.write_u32(b1 + g + 0xC, b2 + 0x19C0);
        fx.mem.write_u32(b1 + g + 4, 0x2000_0000);
        hw(fx, b1, &[(g + 0x10, 0xB0), (g + 0x12, 0x40), (g + 0x14, 0x20), (g + 0x16, 0x20)]);
        colour(fx, b1 + g + 8, v.c_waves);
        fx.mem.write_u32(b1 + g + 0x20, v.wave_size);
        fx.mem.write_u32(b1 + g + 0x24, 0x3D4C_CCCD);
        hw(fx, b1, &[(g + 0x2A, -2 * k as i32), (g + 0x28, 0x100)]);
        fx.mem.write_u8(b1 + g + 0x2C, (k + 7) as u8);
    }
    // The quad group (+0x340).
    fx.mem.write_u32(b1 + 0x344, 0x8_0001);
    hw(fx, b1, &[(0x350, 0x8E), (0x352, 0), (0x354, 0x22), (0x356, 0x40)]);
    colour(fx, b1 + 0x348, v.c_quads);
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.quads_setup(b1 + 0x340, tex, f32::from_bits(0x404C_CCCD), v.template);
    // Two rings (+0xA60, +0xE60).
    for (g, c, outer) in [(0xA60u32, v.c_ring_a, 0x4133_3333u32), (0xE60, v.c_ring_b, 0x414C_CCCD)] {
        fx.mem.write_u32(b1 + g + 4, 0x2000_0000);
        hw(fx, b1, &[(g + 0x10, 0x8E), (g + 0x12, 0), (g + 0x14, 0x20), (g + 0x16, 0x40)]);
        colour(fx, b1 + g + 8, c);
        let tex = fx.mem.read_u32(sp + 0xA0);
        fx.ring_setup(b1 + g, tex, 12.0, f32::from_bits(outer), 0.0, 0.0);
    }
    // The point quads (+0x840, points +0x7A0).
    fx.mem.write_u32(b1 + 0x878, 0x8_0001);
    hw(fx, b1, &[(0x840, 0x80), (0x842, 0), (0x844, 0x20), (0x846, 0x20), (0x848, 0xB0), (0x84A, 0x20), (0x84C, 0x100), (0x84E, 0x100)]);
    fx.mem.write_u32(b1 + 0x870, 0x3F80_0000);
    colour(fx, b1 + 0x874, v.c_points);
    hw(fx, b1, &[(0x888, 0), (0x88A, 0), (0x88C, 0), (0x88E, 1), (0x890, 0), (0x892, 0x3E7)]);
    fx.mem.set32(b1 + 0x7A8, 1);
    let tex = fx.mem.read_u32(sp + 0xA0);
    pointquad::setup(fx, SETUP, b1 + 0x840, b1 + 0x7A0, tex, 40.0);
    if v.trail.is_some() {
        // b360's quad trail (+0x2D0, cells +0x320).
        let tex = fx.mem.read_u32(sp + 0xA0);
        fx.mem.write_u32(D + 0x2D0, tex);
        bytes(fx, &[(0x2D6, 0x80), (0x2D5, 0x80), (0x2D4, 0x80), (0x2D7, 0x40)]);
        fx.mem.write_u32(D + 0x2D8, 0x1_9003);
        fx.mem.write_u32(D + 0x2DC, D + 0x320);
        halves(fx, &[(0x2E0, 0x80), (0x2E2, 0xC0), (0x2E4, 0x20), (0x2E6, 0x20), (0x2E8, 0x80), (0x2EA, 0xC0), (0x2EC, 0x80), (0x2EE, 0x100)]);
        for (o, w) in [(0x2F0u32, 0u32), (0x2F4, 0x3E99_999A), (0x2F8, 0), (0x300, 0x4040_0000), (0x304, 0x3E4C_CCCD), (0x308, 0x4053_3333)] {
            fx.mem.write_u32(D + o, w);
        }
        halves(fx, &[(0x312, 0), (0x314, 0x18), (0x316, 0x24), (0x318, 0), (0x31A, 8), (0x31C, 8), (0x31E, 0)]);
    }
    // Two late particle clouds (+0x1640, +0x1790).
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(b1 + 0x1640, tex);
    fx.mem.write_u32(b1 + 0x1644, v.cloud_flags.0);
    for o in [0x1650u32, 0x1654, 0x1658] {
        fx.mem.write_u32(b1 + o, 0);
    }
    hw(fx, b1, &[(0x1660, 0xF0), (0x1662, 0), (0x1664, 0x10), (0x1666, 0x10), (0x1668, 0xF0), (0x166A, 0), (0x166C, 0x100), (0x166E, 0x100)]);
    fx.mem.write_u32(b1 + 0x164C, v.cloud_sizes.0);
    hw(fx, b1, &[(0x1770, 0x10), (0x177A, 1), (0x177C, 0), (0x1780, 8), (0x1776, 0xA), (0x1778, 0xA)]);
    colour(fx, b1 + 0x1648, v.c_clouds.0);
    cloud8(fx, b1 + 0x1640, [rs[9], rs[10]]);
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(b1 + 0x1790, tex);
    fx.mem.write_u32(b1 + 0x1794, v.cloud_flags.1);
    for o in [0x17A0u32, 0x17A4, 0x17A8] {
        fx.mem.write_u32(b1 + o, 0);
    }
    hw(fx, b1, &[(0x17B0, 0x80), (0x17B2, 0xC0), (0x17B4, 0x20), (0x17B6, 0x20), (0x17B8, 0x80), (0x17BA, 0xC0), (0x17BC, 0x80), (0x17BE, 0x100)]);
    fx.mem.write_u32(b1 + 0x179C, v.cloud_sizes.1);
    hw(fx, b1, &[(0x18C0, 0x10), (0x18CA, 1), (0x18CC, 0), (0x18D0, 8), (0x18C6, 0xA), (0x18C8, 0xA)]);
    colour(fx, b1 + 0x1798, v.c_clouds.1);
    cloud8(fx, b1 + 0x1790, [rs[11], rs[12]]);
    // The swaths (+0x70, +0x1A0).
    swath_init(fx, sp, v, 0x70, (b2 + 0x1E48, b2 + 0x2008), b1 + 0x19B8, b1 + 0x19BC, 0x50);
    swath_init(fx, sp, v, 0x1A0, (b2 + 0x1AC8, b2 + 0x1C88), b1 + 0x19B0, b1 + 0x19B4, 0x70);
    for x in [0x2D0u32, 0x1260, 0x9F0] {
        for o in [8u32, 4, 0] {
            fx.mem.write_u32(b1 + x + o, 0x3DCC_CCCD);
        }
    }
    0
}

fn within(n: i32, r: (i32, i32)) -> bool {
    n >= r.0 && n < r.1
}

fn step(fx: &mut Fx, sp: u32, v: &Variant) -> i32 {
    let (b1, b2) = (D + v.shift.0, D + v.shift.1);
    let n = |fx: &mut Fx| fx.mem.i32(b1 + 0x19A4);
    let m = n(fx);
    // s2 / s3: sp + 0x60 / 0x80 once the swath nodes are read, else the
    // parameter (-1, 64-bit) and the runner's 0.
    let gated = v.swath_gate.map_or(true, |g| within(m, g));
    if gated {
        let w = if within(m, v.narrow) { 5 } else { 8 };
        halves(fx, &[(0x1AC, w), (0x7C, w)]);
        node(fx, b1 + 0x19B8, sp + 0x50);
        node(fx, b1 + 0x19BC, sp + 0x60);
        node(fx, b1 + 0x19B0, sp + 0x70);
        node(fx, b1 + 0x19B4, sp + 0x80);
        if n(fx) == v.fill_at {
            swath_fill(fx, sp, 0x70, 0x50);
            swath_fill(fx, sp, 0x1A0, 0x70);
        } else {
            swath::push(fx, D + 0x70, sp + 0x50, sp + 0x60);
            swath::push(fx, D + 0x1A0, sp + 0x70, sp + 0x80);
        }
    }
    for out in [0x15F0u32, 0xA10, 0x2F0, 0x1280] {
        match v.node_until {
            Some(u) => {
                if n(fx) < u {
                    node(fx, b1 + 0x19AC, b1 + out);
                }
            }
            None => {
                let t = fx.target_handle();
                fx.unit_position(t, b1 + out);
            }
        }
    }
    if let Some((a, b, _)) = v.trail {
        for (set, k) in [(a, 6u32), (b, 5)] {
            if set.contains(&n(fx)) {
                node(fx, b1 + 0x19AC + 4 * k, sp + 0x90);
                quadtrail::push(fx, D + 0x2D0, sp + 0x90);
            }
        }
    }
    let world = sites(fx, v, 0x0010_38D0);
    let board = sites(fx, v, 0x0010_39A0);
    let regs = if gated { [Some(D), Some(OT), Some(sp + 0x60), Some(sp + 0x80)] } else { [Some(D), Some(OT), Some(0), Some(0)] };
    fx.xform_world_saving(D, world[0], regs);
    if !gated {
        fx.saved(sp - 0xD0, &[(0x40, u64::MAX)]);
    }
    for (k, x) in [0x2D0u32, 0x1260, 0x15D0, 0x9F0].into_iter().enumerate() {
        fx.xform_billboard_saving(b1 + x, board[k], regs);
        if !gated {
            fx.saved(sp - 0xF0, &[(0x40, u64::MAX)]);
        }
    }
    let m = n(fx);
    if v.no_light {
    } else if v.light_after {
        if m >= v.up.0 {
            fx.step_up(b2 + 0x1AC4, 0x80, 0x10);
        } else {
            fx.step_down(b2 + 0x1AC4, 0x20, 2);
        }
        let l = fx.mem.read_u8(b2 + 0x1AC4);
        fx.set_stage_light(i32::from(l));
    } else {
        let l = fx.mem.read_u8(b2 + 0x1AC4);
        fx.set_stage_light(i32::from(l));
        if n(fx) < v.dim_until {
            fx.step_down(b2 + 0x1AC4, 0x20, 2);
        }
        if within(n(fx), v.up) {
            fx.step_up(b2 + 0x1AC4, 0x80, 0x10);
        }
    }
    let m = n(fx);
    if v.frame_quad.iter().any(|&r| within(m, r)) {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if within(n(fx), v.object_op) {
        let h = fx.self_handle();
        for o in [0xACu32, 0xAD, 0xAE, 0xAB, 0xA8, 0xA9, 0xAA] {
            fx.mem.write_u8(sp + o, 0x80);
        }
        fx.mem.write_u8(sp + 0xAF, 0);
        fx.boundary(0x001F_83B0, "object op 0x1F83B0", [h as u32, 3, 0, sp + 0xA8]);
    }
    if let Some((_, _, until)) = v.trail {
        if n(fx) < until {
            quadtrail::draw(fx, TRAIL_DRAW, OT, D + 0x2D0, D);
            let mut i = 0;
            while i < fx.mem.i16(D + 0x318) {
                let mut j = 0;
                while j < fx.mem.i16(D + 0x31A) {
                    let w = fx.mem.i16(D + 0x31A);
                    let lo = fx.mult(i, w);
                    fx.step_down(D + 0x32C + 0x10 * (j + lo) as u32, 0, 3);
                    j += 1;
                }
                i += 1;
            }
        }
    }
    if within(n(fx), v.swath_draw) {
        match v.fade_in_until {
            Some(u) => {
                swath::draw(fx, D + 0x70, D);
                swath::draw(fx, D + 0x1A0, D);
                if n(fx) < u {
                    fx.step_up(D + 0x7B, 0xF0, 0x10);
                    fx.step_up(D + 0x1AB, 0xF0, 0x10);
                } else {
                    fx.step_down(D + 0x7B, 0, 0x10);
                    fx.step_down(D + 0x1AB, 0, 0x10);
                }
            }
            None => {
                swath::draw(fx, D + 0x70, D);
                fx.step_up(D + 0x7B, 0xF0, 0x10);
                swath::draw(fx, D + 0x1A0, D);
                fx.step_up(D + 0x1AB, 0xF0, 0x10);
            }
        }
    }
    if n(fx) >= v.tint_from {
        crate::b262::tint(fx, b2 + 0x1AC0);
        fx.step_down(b2 + 0x1AC3, 0, 8);
        for o in [0x98u32, 0x94, 0x90] {
            fx.mem.write_u32(sp + o, 0x3F82_8F5C);
        }
        fx.particle_quads(b1 + 0x15D0, b1 + 0x8A0, sp + 0x90);
        fx.step_down(b1 + 0x8AB, 0, 4);
    }
    if n(fx) >= v.squares_from {
        pointquad::squares(fx, OT, b1 + 0x12D0, b1 + 0x1260, b1 + 0x15C0);
        fx.step_down(b1 + 0x12DB, 0, 8);
        pointquad::draw(fx, OT, b1 + 0x840, b1 + 0x7A0, b1 + 0x2D0);
        fx.quads_draw(b1 + 0x340, b1 + 0x2D0);
        if ee_lt(fx.mem.f(b1 + 0x1260), 1.0) {
            let s = ee::add(fx.mem.f(b1 + 0x1268), 0.5);
            for o in [0x1268u32, 0x1264, 0x1260] {
                fx.mem.setf(b1 + o, s);
            }
        }
        if ee_lt(fx.mem.f(b1 + 0x2D0), 1.0) {
            let d = f32::from_bits(0x3ECC_CCCD);
            for o in [0x2D0u32, 0x2D4, 0x2D8] {
                let s = ee::add(fx.mem.f(b1 + o), d);
                fx.mem.setf(b1 + o, s);
            }
        } else {
            let d = f32::from_bits(0x3BA3_D70A);
            for o in [0x2D0u32, 0x2D4, 0x2D8] {
                let s = ee::add(fx.mem.f(b1 + o), d);
                fx.mem.setf(b1 + o, s);
            }
            fx.step_down(b1 + 0x34B, 0, 3);
            fx.step_down(b1 + 0x877, 0, 6);
        }
        for k in 0..0x40u32 {
            for c in 0..3u32 {
                let a = b1 + 0x3A0 + 0x10 * k + 4 * c;
                let w = fx.mem.read_u32(a).wrapping_add(0x80);
                fx.mem.write_u32(a, w);
            }
        }
    }
    if within(n(fx), v.waves) {
        for k in 0..4u32 {
            let g = b1 + 0x18E0 + 0x30 * k;
            if fx.mem.i16(g + 0x2A) >= 0xA {
                fx.mem.set16(g + 0x2A, 0);
                fx.mem.write_u8(g + 0xB, 0x80);
            }
            wave::draw(fx, OT, g, b1 + 0x15D0);
            fx.step_down(g + 0xB, 0, 0x10);
        }
        fx.ring_draw(b1 + 0xA60, b1 + 0x9F0);
        fx.ring_draw(b1 + 0xE60, b1 + 0x9F0);
        fx.step_down(b1 + 0xA6B, 0, 2);
        fx.step_down(b1 + 0xE6B, 0, 3);
        if fx.approach(b1 + 0x9F0, 2.0, f32::from_bits(0x3E4C_CCCD)) == 0 {
            for o in [0x9F8u32, 0x9F4, 0x9F0] {
                fx.mem.write_u32(b1 + o, 0x3DCC_CCCD);
            }
            fx.mem.write_u8(b1 + 0xA6B, 0x20);
            fx.mem.write_u8(b1 + 0xE6B, 0x30);
        }
        for o in [0x98u32, 0x94, 0x90] {
            fx.mem.write_u32(sp + o, 0);
        }
        fx.particle_quads(b1 + 0x15D0, b1 + 0x1640, sp + 0x90);
        fx.particle_quads(b1 + 0x15D0, b1 + 0x1790, sp + 0x90);
        if n(fx) >= v.clouds_fade {
            fx.step_down(b1 + 0x164B, 0, 8);
            fx.step_down(b1 + 0x179B, 0, 8);
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(b1 + 0x19A4, m);
    let left = fx.mem.i32(b1 + 0x19A0) - 1;
    fx.mem.set32(b1 + 0x19A0, left);
    if m == v.two_at {
        2
    } else if within(m, v.one) {
        1
    } else if within(m, v.six) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
