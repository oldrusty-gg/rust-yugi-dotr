//! BMODULE b140 (card 395), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E037A0, data area 0x1ED0000, 0x1AF0 bytes) [F]
//!: b049's family (b028's library) with two
//! ribbons: on 6..0x18 a particle cloud (+0x1140, 16 particles, half size
//! 5) spreading by 1.01 at the attacker's node 0x1E; on 0x33..0x4D and
//! 0x68..0x75 two ribbons (`ribbon8::ribbon`, +0x70 between nodes 0x16 /
//! 0x14, +0x190 between 0x2D / 0x2B; histories held at the nodes
//! otherwise); on 0x64..0x6D at node 0x15 a point-set strip (+0x870 /
//! +0x7D0), a quad burst (+0x370) and two rings (+0x8D0 / +0xCD0) growing
//! by 0.5 to 10 and fading once past 1; from 0x6F at node 0x16 (taken on
//! 0x6E) a cloud of 60 squares (+0x1300) spreading by 1.05, a second quad
//! burst (+0x1660) growing to 20 and a dark red full-window tint (+0x1AC0,
//! b262's), all fading. Returns 1 on update 0x65, 2 on 0x6E, 8 once the
//! 160-update timer +0x1AC8 has run out.
//!
//! Library: `ribbon8::strip_colours` 0x1E00080, `tint` 0x1E001F0 (b262's),
//! `pointquad::setup` / `draw` / `squares` (0x1E00350 / 0x1E00690 /
//! 0x1E01270), `ring_setup` / `ring_draw` (0x1E01850 / 0x1E01B10),
//! `particle_quads` 0x1E02000, `quads_setup` / `quads_draw` (0x1E02B90 /
//! 0x1E02CF0), `ribbon8::ribbon` 0x1E03300.
use crate::b028::square_points;
use crate::b436::ring_group;
use crate::b628::halves;
use crate::blib::{pointquad, ribbon8};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// The quad corner template (rodata, 0x1E02B90's lui / addiu).
const QUAD_TEMPLATE: u32 = 0x01E0_4580;
const RIBBON_A: u32 = D + 0x70;
const RIBBON_B: u32 = D + 0x190;
const QUADS_A: u32 = D + 0x370;
const POINTS: u32 = D + 0x7D0;
const LOOK: u32 = D + 0x870;
const RING_A: u32 = D + 0x8D0;
const RING_B: u32 = D + 0xCD0;
const CLOUD: u32 = D + 0x1140;
const SQUARES: u32 = D + 0x1300;
const QUADS_B: u32 = D + 0x1660;
const TINT: u32 = D + 0x1AC0;
const TIMER: u32 = D + 0x1AC8;
const COUNTER: u32 = D + 0x1ACC;
const NODES: u32 = D + 0x1AD4;

/// The entry 0x1E037A0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xC0);
    ret
}

fn node(fx: &mut Fx, k: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(NODES + 4 * k);
    fx.node_position(h, s, out);
}

/// The ribbon `g`'s histories (8 points) at `a` / `b`.
fn hold(fx: &mut Fx, g: u32, a: u32, b: u32) {
    for k in 0..8u32 {
        for c in 0..3u32 {
            let w = fx.mem.read_u32(a + 4 * c);
            fx.mem.write_u32(g + 0x10 + 0x10 * k + 4 * c, w);
            let w = fx.mem.read_u32(b + 4 * c);
            fx.mem.write_u32(g + 0x90 + 0x10 * k + 4 * c, w);
        }
    }
}

fn bgra(fx: &mut Fx, g: u32, c: [u8; 4]) {
    for (k, x) in [(0xAu32, c[2]), (9, c[1]), (8, c[0]), (0xB, c[3])] {
        fx.mem.write_u8(g + k, x);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x1AF0);
    for x in [0u32, 0x300, 0x10D0, 0x1290, 0x15F0] {
        fx.xform_init(D + x);
    }
    for (k, node) in [0x16, 0x15, 0x14, 0x2D, 0x2C, 0x2B, 0x1E].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set32(NODES + 4 * k as u32, r);
    }
    fx.mem.set32(TIMER, 0xA0);
    fx.mem.set32(COUNTER, 1);
    fx.mem.set32(D + 0x1AD0, param);
    fx.mem.write_u32(D + 0x1AC4, 0);
    for (k, x) in [(0u32, 0u8), (1, 0), (3, 0x80), (2, 0x80)] {
        fx.mem.write_u8(TINT + k, x);
    }
    fx.texture4(sp + 0xB0, 0xF);
    let tex = |fx: &mut Fx| fx.mem.read_u32(sp + 0xB0);
    // The squares (+0x1300): 60 points, half size 0.5.
    let t = tex(fx);
    fx.mem.write_u32(SQUARES, t);
    fx.mem.write_u32(SQUARES + 0xC, 0x3F00_0000);
    square_points(fx, SQUARES);
    fx.mem.write_u32(SQUARES + 4, 0x81);
    halves(fx, SQUARES, &[(0x10, 0xB0), (0x12, 0x40), (0x14, 0x20), (0x16, 0x20)]);
    bgra(fx, SQUARES, [0x70, 0xA0, 0xD0, 0x80]);
    // The quad bursts (+0x370 / +0x1660): size 1.
    fx.mem.write_u32(QUADS_A + 4, 0x8_0001);
    halves(fx, QUADS_A, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x22), (0x16, 0x40)]);
    bgra(fx, QUADS_A, [0x70, 0xA0, 0xD0, 0x20]);
    let t = tex(fx);
    fx.quads_setup(QUADS_A, t, 1.0, QUAD_TEMPLATE);
    fx.mem.write_u32(QUADS_B + 4, 1);
    halves(fx, QUADS_B, &[(0x10, 0xB0), (0x12, 0x60), (0x14, 0x20), (0x16, 0x20)]);
    bgra(fx, QUADS_B, [0x70, 0xA0, 0xD0, 0x20]);
    let t = tex(fx);
    fx.quads_setup(QUADS_B, t, 1.0, QUAD_TEMPLATE);
    // Two rings (+0x8D0: radii 12 / 11.2; +0xCD0: 12 / 12.8).
    ring_group(fx, RING_A, 0x2008_0000, 0x40, [0x70, 0xA0, 0xD0, 0x20]);
    let t = tex(fx);
    fx.ring_setup(RING_A, t, 12.0, f32::from_bits(0x4133_3333), 0.0, 0.0);
    ring_group(fx, RING_B, 0x2008_0000, 0x40, [0x70, 0xA0, 0xD0, 0x30]);
    let t = tex(fx);
    fx.ring_setup(RING_B, t, 12.0, f32::from_bits(0x414C_CCCD), 0.0, 0.0);
    // The point set's look (+0x870): cell (176, 32, 32, 32) of a 256 x 256
    // sheet from (176, 32), looping frame 0.
    fx.mem.write_u32(LOOK + 0x38, 0x8_0001);
    halves(fx, LOOK, &[(0, 0xB0), (2, 0x20), (4, 0x20), (6, 0x20), (8, 0xB0), (0xA, 0x20), (0xC, 0x100), (0xE, 0x100)]);
    fx.mem.write_u32(LOOK + 0x30, 0x3F80_0000);
    for (k, x) in [(0x36u32, 0xD0u8), (0x35, 0xA0), (0x34, 0x70), (0x37, 0x60)] {
        fx.mem.write_u8(LOOK + k, x);
    }
    halves(fx, LOOK, &[(0x48, 0), (0x4A, 0), (0x4C, 0), (0x4E, 1), (0x50, 0), (0x52, 0x3E7)]);
    fx.mem.write_u32(POINTS + 8, 1);
    let t = tex(fx);
    pointquad::setup(fx, 0x01E0_0350, LOOK, POINTS, t, 16.0);
    // The particle cloud (+0x1140): 16 particles, half size 5.
    let t = tex(fx);
    fx.mem.write_u32(CLOUD, t);
    fx.mem.write_u32(CLOUD + 4, 0x8_0081);
    for k in [0x10u32, 0x14, 0x18, 0x1C] {
        fx.mem.write_u32(CLOUD + k, 0);
    }
    halves(fx, CLOUD, &[(0x28, 0xF0), (0x2A, 0), (0x24, 0x10), (0x26, 0x10), (0x2C, 0x100), (0x2E, 0x100)]);
    fx.mem.write_u32(CLOUD + 0xC, 0x40A0_0000);
    halves(fx, CLOUD, &[(0x130, 0x10), (0x13A, 2), (0x13C, 1), (0x140, 8), (0x136, 0), (0x138, 0)]);
    bgra(fx, CLOUD, [0x70, 0xA0, 0xD0, 0x20]);
    for k in 0..16u32 {
        let p = CLOUD + 0x30 + 16 * k;
        for o in [0u32, 4] {
            let r = fx.rand();
            let (_, h) = fx.div(r, 6);
            let v = ee::add(fx.mem.f(CLOUD + 0x10 + o), ee::from_i32(h - 3));
            fx.mem.setf(p + o, v);
        }
        let z = fx.mem.read_u32(CLOUD + 0x18);
        fx.mem.write_u32(p + 8, z);
        let a = -i32::from(fx.mem.i16(CLOUD + 0x13A));
        let f = fx.mult(a, k as i32);
        fx.mem.set16(p + 0xE, f);
        fx.mem.set16(p + 0xC, 0);
        let t = fx.mem.i16(CLOUD + 0x140) - 1;
        fx.mem.set16(CLOUD + 0x13E, t);
    }
    // A group at +0x2B0 (set, never drawn).
    let g = D + 0x2B0;
    let t = tex(fx);
    fx.mem.write_u32(g, t);
    fx.mem.write_u32(g + 4, 0x0600_0001);
    halves(fx, g, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x20), (0x16, 0x40)]);
    bgra(fx, g, [0x80, 0x80, 0x80, 0xFF]);
    fx.mem.write_u32(g + 0xC, 0x40A0_0000);
    // The ribbons: colour (B, G, R, A) (0xA0, 0x80, 0x60, 0x80).
    for r in [RIBBON_A, RIBBON_B] {
        fx.mem.write_u32(r + 4, 0);
        bgra(fx, r, [0xA0, 0x80, 0x60, 0x80]);
    }
    node(fx, 0, sp + 0x70);
    node(fx, 2, sp + 0x80);
    hold(fx, RIBBON_A, sp + 0x70, sp + 0x80);
    node(fx, 3, sp + 0x90);
    node(fx, 5, sp + 0xA0);
    hold(fx, RIBBON_B, sp + 0x90, sp + 0xA0);
    let t = fx.target_handle();
    fx.unit_point(2, t, D + 0x20);
    for x in [0x300u32, 0x1290, 0x15F0] {
        for k in [8u32, 4, 0] {
            fx.mem.write_u32(D + x + k, 0x3DCC_CCCD);
        }
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    node(fx, 0, sp + 0x70);
    node(fx, 2, sp + 0x80);
    node(fx, 3, sp + 0x90);
    node(fx, 5, sp + 0xA0);
    let n = |fx: &mut Fx| fx.mem.i32(COUNTER);
    if n(fx) == 0x6E {
        node(fx, 0, D + 0x12B0);
        node(fx, 0, D + 0x1610);
    }
    node(fx, 1, D + 0x320);
    node(fx, 6, D + 0x10F0);
    // s0 = D, s1 = the OT, s2 = sp + 0x80, s3 = sp + 0xA0 (s4 = sp + 0x90).
    let regs = [Some(D), Some(OT), Some(sp + 0x80), Some(sp + 0xA0)];
    fx.xform_world_saving(D, 0x01E0_405C, regs);
    for (x, ra) in [(0x300u32, 0x01E0_4064), (0x10D0, 0x01E0_406C), (0x1290, 0x01E0_4074)] {
        fx.xform_billboard_saving(D + x, ra, regs);
    }
    fx.xform_world_saving(D + 0x15F0, 0x01E0_407C, regs);
    if (6..0x19).contains(&n(fx)) {
        for k in [8u32, 4, 0] {
            fx.mem.write_u32(sp + 0x60 + k, 0x3F81_47AE);
        }
        fx.particle_quads(D + 0x10D0, CLOUD, sp + 0x60);
        if n(fx) >= 0x14 {
            fx.step_down(CLOUD + 0xB, 0, 0xA);
        }
    }
    let c = n(fx);
    if (0x33..0x4E).contains(&c) || (0x68..0x76).contains(&c) {
        ribbon8::ribbon(fx, RIBBON_A, D, sp + 0x70, sp + 0x80);
        ribbon8::ribbon(fx, RIBBON_B, D, sp + 0x90, sp + 0xA0);
    } else {
        hold(fx, RIBBON_A, sp + 0x70, sp + 0x80);
        hold(fx, RIBBON_B, sp + 0x90, sp + 0xA0);
    }
    if (0x64..0x6E).contains(&n(fx)) {
        pointquad::draw(fx, OT, LOOK, POINTS, D + 0x300);
        fx.quads_draw(QUADS_A, D + 0x300);
        fx.ring_draw(RING_A, D + 0x300);
        fx.ring_draw(RING_B, D + 0x300);
        if !ee_le(fx.mem.f(D + 0x300), 1.0) {
            for (k, s) in [(0x8A7u32, 0x10), (0x37B, 5), (0x8DB, 5), (0xCDB, 8)] {
                fx.step_down(D + k, 0, s);
            }
        }
        fx.approach(D + 0x300, 10.0, 0.5);
        for k in 0..64u32 {
            for c in 0..3u32 {
                let a = D + 0x3D0 + 16 * k + 4 * c;
                let x = fx.mem.i32(a).wrapping_add(0x80);
                fx.mem.set32(a, x);
            }
        }
    }
    if n(fx) >= 0x6F {
        if ee_lt(fx.mem.f(D + 0x1290), 1.0) {
            let v = ee::add(fx.mem.f(D + 0x1298), 0.5);
            for k in [0x1298u32, 0x1294, 0x1290] {
                fx.mem.setf(D + k, v);
            }
        }
        for k in [8u32, 4, 0] {
            fx.mem.write_u32(sp + 0x60 + k, 0x3F86_6666);
        }
        pointquad::squares(fx, OT, SQUARES, D + 0x1290, sp + 0x60);
        fx.step_down(SQUARES + 0xB, 0, 0xA);
        fx.quads_draw(QUADS_B, D + 0x15F0);
        fx.step_down(QUADS_B + 0xB, 0, 2);
        fx.approach(D + 0x15F0, 20.0, 0.5);
        crate::b262::tint(fx, TINT);
        fx.step_down(TINT + 3, 0, 0x10);
    }
    let m = n(fx) + 1;
    fx.mem.set32(COUNTER, m);
    let left = fx.mem.i32(TIMER) - 1;
    fx.mem.set32(TIMER, left);
    if m == 0x6E {
        2
    } else if m == 0x65 {
        1
    } else if left > 0 {
        0
    } else {
        8
    }
}
