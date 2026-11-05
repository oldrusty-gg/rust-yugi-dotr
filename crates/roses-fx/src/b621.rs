//! BMODULE b621 (card 49) and its twin b576 (card 287), hand-ported from
//! the records (linked at 0x1E00000, entry 0x1E02BB0, data area 0x1ED0000)
//! [F]: b423's library twice over. Two groups of
//! 256 strips (`blib::spin::strips`) rise round two of the attacker's
//! nodes (+0x41E0 / +0x4910), each node point following a point circling
//! the target (radius 15, turning 0x400 a update) once its follow countdown
//! starts; 60 motes (`blib::spin::motes`) round the first; two groups of
//! 256 sprites (+0x4980 / +0x59E0) rising round the two points; a frame
//! quad (colour +0x6A60) and the stage light down to 0x20. b576 is the
//! same program on earlier updates with other nodes and colours, its node
//! points' x tripled.
//!
//! Library (record addresses): `colours4` 0x1E00080, `frame_quad`
//! 0x1E00120, `motes` 0x1E00250, `sprites_spawn` 0x1E009D0, `sprites`
//! 0x1E00B60, `strips` 0x1E02450.
use crate::blib::spin;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const N: u32 = D + 0x6A68;
const LEFT: u32 = D + 0x6A64;
const STRIPS: u32 = 0x01E0_2450;
const A: u32 = D + 0xE0;
const A_PARTS: u32 = D + 0x160;
const B: u32 = D + 0x2160;
const B_PARTS: u32 = D + 0x21E0;
const X1: u32 = D + 0x41E0;
const X2: u32 = D + 0x4910;
const MOTES: u32 = D + 0x4250;
const SA: u32 = D + 0x4980;
const SB: u32 = D + 0x59E0;
const LIGHT: u32 = D + 0x6A70;
const QUAD: u32 = D + 0x6A60;

/// What differs between the two records.
struct V {
    /// The two node requests (node 1 kept, node 2 cleared) and the
    /// updates.
    nodes: (i32, i32),
    left: i32,
    /// The colour (+4 / +8, +5 / +9, +6 / +0xA) of the strips and sprites.
    colour: [u8; 3],
    /// The motes' cell (w, h).
    mote_cell: (i32, i32),
    /// The node points' x tripled (b576).
    triple: bool,
    /// Updates: the second / first node point follows from; the end; the
    /// frame quad from / fading from; the first / second strips shown
    /// from; the sprites round the second / first point from; returns 1 /
    /// 6 from.
    follow2: i32,
    follow1: i32,
    end: i32,
    quad: i32,
    quad_fade: i32,
    show_a: i32,
    show_b: i32,
    sprites_b: i32,
    sprites_a: i32,
    one: i32,
    six: i32,
    /// The step's return addresses past the first node block (b576's are
    /// 0x30 later).
    shift: u32,
}

const B621: V = V {
    nodes: (0x68, 0x5D),
    left: 0x1DF,
    colour: [0x60, 0x80, 0x30],
    mote_cell: (0x40, 0x80),
    triple: false,
    follow2: 0xD0,
    follow1: 0x113,
    end: 0x1C2,
    quad: 0xBF,
    quad_fade: 0x122,
    show_a: 0x102,
    show_b: 0xBF,
    sprites_b: 0xCF,
    sprites_a: 0x112,
    one: 0x10E,
    six: 0x118,
    shift: 0,
};

/// b576 (card 287) [F, entry 0x1E02BB0].
const B576: V = V {
    nodes: (0x53, 0x3E),
    left: 0x17B,
    colour: [0x80, 0x30, 0x60],
    mote_cell: (0x80, 0x40),
    triple: true,
    follow2: 0x6C,
    follow1: 0x6D,
    end: 0x15E,
    quad: 0x5B,
    quad_fade: 0x8C,
    show_a: 0x5C,
    show_b: 0x5B,
    sprites_b: 0x6B,
    sprites_a: 0x6C,
    one: 0xAA,
    six: 0xB4,
    shift: 0x30,
};

fn angle(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(D, ra) & 0xFFFF
}

fn tenth(fx: &mut Fx, ra: u32, m: i32) -> f32 {
    let r = fx.rand_from(D, ra);
    let (_, h) = fx.div(r, m);
    ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h))
}

/// The entry 0x1E02BB0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B621)
}

/// b576's entry 0x1E02BB0.
pub fn program_b576(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B576)
}

fn run(fx: &mut Fx, param: i32, v: &V) -> i32 {
    let sp = fx.enter(0x90);
    let ret = if param < 0 { step(fx, sp, param, v) } else { init(fx, sp, param, v) };
    fx.leave(0x90);
    ret
}

/// A strip group (256, flags 0x8003, lifted 8, growing x 1.08) and its
/// particles (alpha 0x10, height 0.1 x (rand % 1000)).
fn strips_init(fx: &mut Fx, g: u32, parts: u32, tex: u32, v: &V, ra: [u32; 4]) {
    fx.mem.write_u32(g, tex);
    for (o, x) in [(6u32, v.colour[2]), (5, v.colour[1]), (4, v.colour[0]), (7, 0)] {
        fx.mem.write_u8(g + o, x);
    }
    fx.mem.write_u32(g + 8, 0x8003);
    fx.mem.write_u32(g + 0xC, parts);
    fx.mem.write_u32(g + 0x60, 0x4100_0000);
    for (o, x) in [(0x10u32, 0x80i32), (0x12, 0xC0), (0x14, 0x20), (0x16, 0x20), (0x18, 0x80), (0x1A, 0xC0), (0x1C, 0x80), (0x1E, 0x100)] {
        fx.mem.set16(g + o, x);
    }
    fx.mem.write_u32(g + 0x64, 0x3F8A_3D71);
    for (o, x) in [(0x72u32, 0i32), (0x74, 0x18), (0x76, 0x100), (0x78, 0x100), (0x7C, 8), (0x7E, 1)] {
        fx.mem.set16(g + o, x);
    }
    for k in 0..4u32 {
        let t = ee::from_i32((0x1000 * k) as i32);
        let c = fx.cosf(t);
        fx.mem.setf(g + 0x28 + 0x10 * k, c);
        let s = fx.sinf(t);
        fx.mem.setf(g + 0x20 + 0x10 * k, s);
        fx.mem.write_u32(g + 0x24 + 0x10 * k, 0);
    }
    let mut k = 0;
    while k < fx.mem.i16(g + 0x76) {
        let p = parts + 0x20 * k as u32;
        fx.mem.set16(p + 0xE, 0);
        fx.mem.write_u8(p + 0xC, 0x10);
        let r = fx.rand_from(D, ra[0]);
        fx.mem.set8(p + 0xD, r % 8);
        let y = tenth(fx, ra[1], 1000);
        fx.mem.setf(p + 4, y);
        let a = angle(fx, ra[2]);
        fx.mem.set32(p + 0x14, a);
        let s = tenth(fx, ra[3], 0x3C);
        fx.mem.setf(p + 0x10, s);
        k += 1;
    }
}

/// A sprite group (256, flags 0x58000003, alpha 0x10, size 30) spawned.
fn sprites_init(fx: &mut Fx, g: u32, tex: u32, v: &V, ra: [u32; 2]) {
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x5800_0003);
    for (o, x) in [(0xAu32, v.colour[2]), (9, v.colour[1]), (8, v.colour[0]), (0xB, 0x10)] {
        fx.mem.write_u8(g + o, x);
    }
    fx.mem.write_u32(g + 0xC, 0x41F0_0000);
    for o in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + o, 0);
    }
    for (o, x) in [(0x30u32, 0x80i32), (0x32, 0xC0), (0x34, 0x20), (0x36, 0x20), (0x38, 0x80), (0x3A, 0xC0), (0x3C, 0x80), (0x3E, 0x40)] {
        fx.mem.set16(g + o, x);
    }
    let parts = g + 0x60;
    fx.mem.write_u32(g + 0x40, parts);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4120_0000);
    fx.mem.write_u32(g + 0x50, 0x40A0_0000);
    fx.mem.set16(g + 0x5A, 0x100);
    fx.mem.set16(g + 0x5C, 8);
    fx.mem.set16(g + 0x5E, 1);
    fx.sprites_spawn(g, 30.0);
    let mut k = 0;
    while k < fx.mem.i16(g + 0x5A) {
        let p = parts + 0x10 * k as u32;
        let r = fx.rand_from(D, ra[0]);
        fx.mem.set16(p + 0xE, -(r % 16));
        let b = fx.mem.read_u8(g + 0xB);
        fx.mem.write_u8(p + 0xC, b);
        let r = fx.rand_from(D, ra[1]);
        fx.mem.set8(p + 0xD, r % 8);
        k += 1;
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32, v: &V) -> i32 {
    fx.bzero(D, 0x6A90);
    for x in [0u32, 0x70, 0x41E0, 0x4910] {
        fx.xform_init(D + x);
    }
    let h = fx.self_handle();
    let r = fx.request_node(h, v.nodes.0, 1);
    fx.mem.set32(D + 0x6A84, r);
    let h = fx.self_handle();
    let r = fx.request_node(h, v.nodes.1, 0);
    fx.mem.set32(D + 0x6A88, r);
    fx.mem.set32(LEFT, v.left);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x6A6C, param);
    fx.mem.write_u32(LIGHT, 0x80);
    fx.mem.write_u32(D + 0x6A74, 0x48);
    fx.mem.write_u32(D + 0x6A78, 0x48);
    fx.mem.write_u32(D + 0x6A7C, 0x4000);
    fx.mem.write_u32(D + 0x6A80, 0xC000);
    for (o, x) in [(0u32, 0x80u8), (1, 0x80), (2, 0x80), (3, 0x60)] {
        fx.mem.write_u8(QUAD + o, x);
    }
    fx.texture5(sp + 0x80, 0);
    // The motes: flags 0x40001001, half size 2.0, white, turned by (0,
    // 0x400, 0).
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(MOTES, tex);
    fx.mem.write_u32(MOTES + 4, 0x4000_1001);
    fx.mem.write_u32(MOTES + 0xC, 0x4000_0000);
    for (o, x) in [(0x20u32, 0i32), (0x22, 0), (0x24, v.mote_cell.0), (0x26, v.mote_cell.1)] {
        fx.mem.set16(MOTES + o, x);
    }
    for o in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(MOTES + o, 0x80);
    }
    for (o, x) in [(0x10u32, 0u32), (0x14, 0x400), (0x18, 0)] {
        fx.mem.write_u32(MOTES + o, x);
    }
    for k in 0..0x3Cu32 {
        let p = MOTES + 0x3F0 + 0xC * k;
        let a = angle(fx, 0x01E0_2D00);
        let y = tenth(fx, 0x01E0_2D1C, 1000);
        fx.mem.setf(p + 4, y);
        let c = fx.tcos(a);
        fx.mem.setf(p + 8, ee::mul(4.0, c));
        let s = fx.tsin(a);
        fx.mem.setf(p, ee::mul(4.0, s));
        for (o, ra) in [(0u32, 0x01E0_2D80u32), (4, 0x01E0_2D9C), (8, 0x01E0_2DB8)] {
            let a = angle(fx, ra);
            fx.mem.set32(MOTES + 0x30 + 0x10 * k + o, a);
        }
    }
    fx.texture4(sp + 0x80, 0xF);
    let tex = fx.mem.read_u32(sp + 0x80);
    strips_init(fx, A, A_PARTS, tex, v, [0x01E0_2EE8, 0x01E0_2F04, 0x01E0_2F34, 0x01E0_2F50]);
    let tex = fx.mem.read_u32(sp + 0x80);
    strips_init(fx, B, B_PARTS, tex, v, [0x01E0_3090, 0x01E0_30AC, 0x01E0_30DC, 0x01E0_30F8]);
    let tex = fx.mem.read_u32(sp + 0x80);
    sprites_init(fx, SA, tex, v, [0x01E0_320C, 0x01E0_3234]);
    let tex = fx.mem.read_u32(sp + 0x80);
    sprites_init(fx, SB, tex, v, [0x01E0_3334, 0x01E0_335C]);
    0
}

/// A node point (`x`) at node request `slot` until update `from`, then
/// following the circling point `c` (angle at `ang`, countdown at `cd`).
#[allow(clippy::too_many_arguments)]
fn follow(fx: &mut Fx, v: &V, x: u32, slot: u32, from: i32, c: u32, ang: u32, cd: u32) {
    if fx.mem.i32(N) < from {
        let h = fx.self_handle();
        let s = fx.mem.i32(slot);
        fx.node_position(h, s, x);
        fx.mem.write_u32(x + 4, 0);
        if v.triple {
            let t = ee::mul(fx.mem.f(x), 3.0);
            fx.mem.setf(x, t);
        }
    }
    let t = fx.target_handle();
    fx.unit_point(1, t, c);
    let a = fx.mem.i32(ang);
    let co = fx.tcos(a);
    let z = ee::add(fx.mem.f(c + 8), ee::mul(15.0, co));
    fx.mem.setf(c + 8, z);
    let a = fx.mem.i32(ang);
    let si = fx.tsin(a);
    let xx = ee::add(fx.mem.f(c), ee::mul(15.0, si));
    fx.mem.setf(c, xx);
    fx.mem.write_u32(c + 4, 0);
    if fx.mem.i32(cd) == 0 {
        let a = fx.mem.read_u32(ang).wrapping_add(0x400);
        fx.mem.write_u32(ang, a);
    }
    if fx.mem.i32(N) >= from {
        let n = fx.mem.i32(cd);
        fx.approach_by(x, c, n);
        let n = fx.mem.i32(cd);
        if n != 0 {
            fx.mem.set32(cd, n - 1);
        }
    }
}

/// A strip group's particles rising 2 (odd ones 3) and restarting above
/// 100.
fn rise(fx: &mut Fx, g: u32, parts: u32, ra: [u32; 2]) {
    let mut k = 0;
    while k < fx.mem.i16(g + 0x76) {
        let p = parts + 0x20 * k as u32;
        let y = ee::add(fx.mem.f(p + 4), if k % 2 == 0 { 2.0 } else { 3.0 });
        fx.mem.setf(p + 4, y);
        if !ee_lt(fx.mem.f(p + 4), 100.0) {
            let y = tenth(fx, ra[0], 0x1E);
            fx.mem.setf(p + 4, y);
            let s = tenth(fx, ra[1], 0x1E);
            fx.mem.setf(p + 0x10, s);
        }
        k += 1;
    }
}

/// A sprite at a random angle on the circle of the group's radius +0x4C
/// round the point `x`, at its height.
fn sprite_place(fx: &mut Fx, g: u32, p: u32, x: u32, ra: u32) {
    let a = angle(fx, ra);
    let c = fx.tcos(a);
    let z = ee::add(fx.mem.f(x + 8), ee::mul(fx.mem.f(g + 0x4C), c));
    fx.mem.setf(p + 8, z);
    let s = fx.tsin(a);
    let xx = ee::add(fx.mem.f(x), ee::mul(fx.mem.f(g + 0x4C), s));
    fx.mem.setf(p, xx);
    let y = fx.mem.read_u32(x + 4);
    fx.mem.write_u32(p + 4, y);
}

/// A sprite group drawn and its sprites rising round `x` from update
/// `from`.
fn sprites_step(fx: &mut Fx, v: &V, g: u32, x: u32, from: i32, ra: [u32; 2]) {
    if fx.mem.i32(N) < from {
        return;
    }
    fx.sprites(g, D);
    if fx.mem.i32(N) >= v.end {
        fx.step_down(g + 0xB, 0, 1);
    }
    let mut k = 0;
    while k < fx.mem.i16(g + 0x5A) {
        let p = g + 0x60 + 0x10 * k as u32;
        if fx.mem.i16(p + 0xE) == 0 {
            sprite_place(fx, g, p, x, ra[0]);
        }
        if fx.mem.i16(p + 0xE) >= 0 {
            let (_, r) = fx.div(k, 3);
            let dy = match r {
                0 => 1.0,
                1 => 2.0,
                _ => 3.0,
            };
            let y = ee::add(fx.mem.f(p + 4), dy);
            fx.mem.setf(p + 4, y);
            if fx.step_down(p + 0xC, 0, 1) == 0 {
                fx.mem.set16(p + 0xE, 0);
                fx.mem.write_u8(p + 0xC, 0x10);
                sprite_place(fx, g, p, x, ra[1]);
            }
        }
        k += 1;
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32, v: &V) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(N);
    let sh = v.shift;
    follow(fx, v, X2 + 0x20, D + 0x6A88, v.follow2, D + 0x6A50, D + 0x6A80, D + 0x6A78);
    follow(fx, v, X1 + 0x20, D + 0x6A84, v.follow1, D + 0x6A40, D + 0x6A7C, D + 0x6A74);
    // s0 = D, s1 = the OT, s2 = the parameter, s3 the runner's (0).
    let regs = [Some(D), Some(OT), Some(param as u32), Some(0)];
    fx.xform_world_saving(D, 0x01E0_354C + sh, regs);
    fx.xform_billboard_saving(D + 0x70, 0x01E0_3554 + sh, regs);
    fx.xform_world_saving(X1, 0x01E0_355C + sh, regs);
    fx.xform_world_saving(X2, 0x01E0_3564 + sh, regs);
    if n(fx) < v.end {
        fx.step_down(LIGHT, 0x20, 4);
    } else {
        fx.step_up(LIGHT, 0x80, 4);
    }
    let light = fx.mem.u8(LIGHT);
    fx.set_stage_light(light);
    let m = n(fx);
    if m >= v.quad && m < v.end {
        fx.frame_quad(QUAD);
        if n(fx) >= v.quad_fade {
            fx.step_down(QUAD + 3, 0x30, 3);
        }
    }
    spin::strips(fx, STRIPS, OT, A, A_PARTS, X1);
    let m = n(fx);
    if m < v.show_a || m >= v.end {
        fx.step_down(A + 7, 0, 1);
    } else {
        fx.step_up(A + 7, 0x10, 1);
    }
    rise(fx, A, A_PARTS, [0x01E0_36C8 + sh, 0x01E0_36F8 + sh]);
    spin::strips(fx, STRIPS, OT, B, B_PARTS, X2);
    let m = n(fx);
    if m < v.show_b || m >= v.end {
        fx.step_down(B + 7, 0, 1);
    } else {
        fx.step_up(B + 7, 0x10, 1);
    }
    rise(fx, B, B_PARTS, [0x01E0_3820 + sh, 0x01E0_3850 + sh]);
    // The motes round the first node, a little wider than tall.
    fx.mem.write_u32(sp + 0x74, 0x3F80_0000);
    fx.mem.write_u32(sp + 0x78, 0x3F8A_3D71);
    fx.mem.write_u32(sp + 0x70, 0x3F8A_3D71);
    spin::motes(fx, OT, MOTES, X1, sp + 0x70);
    if n(fx) >= v.end {
        fx.step_down(MOTES + 0xB, 0, 8);
    }
    for k in 0..0x3Cu32 {
        let p = MOTES + 0x3F0 + 0xC * k;
        let y = ee::add(fx.mem.f(p + 4), if k % 2 == 0 { 2.0 } else { 3.0 });
        fx.mem.setf(p + 4, y);
        if !ee_lt(fx.mem.f(p + 4), 100.0) {
            let a = angle(fx, 0x01E0_3968 + sh);
            fx.mem.write_u32(p + 4, 0);
            let c = fx.tcos(a);
            fx.mem.setf(p + 8, ee::mul(4.0, c));
            let s = fx.tsin(a);
            fx.mem.setf(p, ee::mul(4.0, s));
        }
        for o in [0u32, 4, 8] {
            let a = MOTES + 0x30 + 0x10 * k + o;
            let w = fx.mem.read_u32(a).wrapping_add(0x1000);
            fx.mem.write_u32(a, w);
        }
    }
    sprites_step(fx, v, SB, X2 + 0x20, v.sprites_b, [0x01E0_3A50 + sh, 0x01E0_3B58 + sh]);
    sprites_step(fx, v, SA, X1 + 0x20, v.sprites_a, [0x01E0_3C28 + sh, 0x01E0_3D30 + sh]);
    let m = n(fx) + 1;
    fx.mem.set32(N, m);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if m == v.end {
        2
    } else if m == v.one {
        1
    } else if (v.six..v.end).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
