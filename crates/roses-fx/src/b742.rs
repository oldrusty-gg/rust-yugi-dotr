//! BMODULE b742 (card 526), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E029A0, data area 0x1ED0000) [F] (helper C5, 31
//! Oct 2026). A full-window tint every update; at the attacker's node 0xE
//! (rising by 1 a step) a disc (+0x760) and a plate pair (+0x720) fading
//! in until update 20 and out until 125; at node 0x18 falling streaks
//! (+0x2A0, `blib::rain`) brightening on 50..65 and fading after; a
//! particle cloud (+0xE0) from update 25 (re-spawned bigger on 70); from
//! update 70 a ring of 256 turned blades (+0x7E0, `blib::blade`) at node
//! 0x18 aimed at node 0x14 (re-aimed and reset on 88), from 78 a second
//! (+0x18F0) at node 0x23 aimed at node 0x1F; both widen by 1 a step to
//! 10 and each blade fades once 5 old. Returns 1 on update 60, 6 on
//! 70..90, 2 on 91, 8 after 120.
//!
//! Library: `colours4` (0x1E00100), the frame quad (0x1E001A0),
//! `particle_quads` (0x1E002C0), `plate::plates` (0x1E00E50), `disc`
//! (0x1E013E0), `blade::setup` / `draw` (0x1E01940 / 0x1E01AE0),
//! `rain::setup` / `draw` (0x1E021F0 / 0x1E02440, with `colours2`
//! 0x1E00080).
use crate::b142::fstep_up;
use crate::b628::halves;
use crate::blib::{blade, plate, rain};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BLADE_SETUP: u32 = 0x01E0_1940;
const PLATES: u32 = 0x01E0_0E50;
const RAIN_SETUP: u32 = 0x01E0_21F0;
const RAIN_DRAW: u32 = 0x01E0_2440;
const TENTH: f32 = f32::from_bits(0x3DCC_CCCD);

/// The entry 0x1E029A0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x90);
    ret
}

/// A blade ring of b742 (+0x7E0 / +0x18F0, blades at +0xA0): 256 blades,
/// cell (0, 0, 32, 64), colour (0x40, 0x70, 0xB0, 0x80), height 10, width
/// 15, growth 12 / 0, a full turn, width 1, drawn 999 (+3 a step).
fn blades(fx: &mut Fx, g: u32, tex: u32) {
    fx.mem.write_u32(g + 4, 1);
    halves(fx, g, &[(0x10, 0), (0x12, 0), (0x14, 0x20), (0x16, 0x40)]);
    for (k, x) in [(0xAu32, 0xB0u8), (9, 0x70), (8, 0x40), (0xB, 0x80)] {
        fx.mem.write_u8(g + k, x);
    }
    for (k, x) in [(0x80u32, g + 0xA0), (0x84, 0x4120_0000), (0x88, 0x4170_0000), (0x8C, 0x4140_0000), (0x90, 0), (0x94, 0x1_0000), (0xC, 0x3F80_0000)] {
        fx.mem.write_u32(g + k, x);
    }
    halves(fx, g, &[(0x98, 0), (0x9A, 0x100), (0x9C, 0x3E7), (0x9E, 3)]);
    blade::setup(fx, BLADE_SETUP, g, tex);
}

/// The cloud's first `n` particles placed within (+-x / 10, y / 10) with
/// frames -k (rand & 7 ticks in).
fn cloud_place(fx: &mut Fx, g: u32, n: u32, wide: bool) {
    for k in 0..n {
        let p = g + 0x30 + 16 * k;
        let (mx, ox, my, oy) = if wide { (100, -50, 100, -50) } else { (600, -300, 300, 0) };
        let r = fx.rand();
        let (_, h) = fx.div(r, mx);
        fx.mem.setf(p, ee::mul(TENTH, ee::from_i32(h + ox)));
        let r = fx.rand();
        let (_, h) = fx.div(r, my);
        fx.mem.setf(p + 4, ee::mul(TENTH, ee::from_i32(h + oy)));
        let a = -fx.mem.i16(g + 0x13A);
        let f = fx.mult(a, k as i32);
        fx.mem.set16(p + 0xE, f);
        let r = fx.rand();
        fx.mem.set16(p + 0xC, r % 8);
        if !wide {
            let t = fx.mem.i16(g + 0x140) - 1;
            fx.mem.set16(g + 0x13E, t);
        }
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x29B0);
    for x in [0u32, 0x70, 0x230, 0x770, 0x1880, 0x6B0] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x299Cu32, 0x18, 1), (0x29A0, 0x23, 0), (0x29A4, 0x14, 0), (0x29A8, 0x1F, 0), (0x29AC, 0xE, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x2990, 0x78);
    fx.mem.set32(D + 0x2994, 1);
    fx.mem.set32(D + 0x2998, param);
    fx.texture5(sp + 0x80, 0);
    for g in [0x7E0u32, 0x18F0] {
        let tex = fx.mem.read_u32(sp + 0x80);
        blades(fx, D + g, tex);
    }
    fx.texture4(sp + 0x80, 0xF);
    // The particle cloud (+0xE0): 16 particles, half size 20, 8 frames;
    // 12 placed.
    let g = D + 0xE0;
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x8_0007);
    for k in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x20, 0x80), (0x22, 0xC0), (0x24, 0x20), (0x26, 0x20), (0x28, 0x80), (0x2A, 0xC0), (0x2C, 0x80), (0x2E, 0x100)]);
    fx.mem.write_u32(g + 0xC, 0x41A0_0000);
    halves(fx, g, &[(0x130, 0x10), (0x13A, 1), (0x13C, 1), (0x140, 8), (0x136, 0x1E), (0x138, -0x1E)]);
    for (k, x) in [(0xAu32, 0xD0u8), (9, 0x70), (8, 0x30), (0xB, 0x10)] {
        fx.mem.write_u8(g + k, x);
    }
    cloud_place(fx, g, 12, false);
    // The streaks (+0x2A0): 64, speed 10.
    let g = D + 0x2A0;
    fx.mem.write_u32(g, 0x1008_0001);
    for (k, x) in [(6u32, 0xD0u8), (5, 0x70), (4, 0x30), (7, 0)] {
        fx.mem.write_u8(g + k, x);
    }
    rain::setup(fx, RAIN_SETUP, g, 0x40, 10.0);
    // The plates (+0x720) and the disc (+0x760).
    let g = D + 0x720;
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x8_0000);
    for (k, x) in [(0xAu32, 0x70u8), (9, 0xD0), (8, 0xA0), (0xB, 0)] {
        fx.mem.write_u8(g + k, x);
    }
    for (k, x) in [(0xCu32, 0x3F00_0000u32), (0x10, 0x40C0_0000), (0x14, 0x3F00_0000), (0x18, 0x4000_0000)] {
        fx.mem.write_u32(g + k, x);
    }
    halves(fx, g, &[(0x20, 0xB0), (0x22, 0x60), (0x24, 0x20), (0x26, 0x20)]);
    for k in [0x30u32, 0x34, 0x38] {
        fx.mem.write_u32(g + k, 0);
    }
    let g = D + 0x760;
    fx.mem.write_u32(g, 0x8_0000);
    for (k, x) in [(6u32, 0x70u8), (5, 0xD0), (4, 0xA0), (7, 0)] {
        fx.mem.write_u8(g + k, x);
    }
    fx.mem.write_u32(g + 8, 0x4080_0000);
    halves(fx, g, &[(0xC, 0x24), (0xE, 0)]);
    0
}

fn node(fx: &mut Fx, at: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(D + at);
    fx.node_position(h, s, out);
}

/// A ring at node `from` (+`at` / the ring's xform `x`) aimed at node
/// `to` (0x103DB0 into `x` + 0x10).
fn aim(fx: &mut Fx, sp: u32, from: u32, to: u32, x: u32) {
    node(fx, from, x + 0x20);
    node(fx, from, sp + 0x50);
    node(fx, to, sp + 0x60);
    fx.look_angles(sp + 0x50, sp + 0x60, x + 0x10);
}

/// Blades of the ring `g` 5 or more old fade by 16.
fn blades_fade(fx: &mut Fx, g: u32) {
    let mut k = 0;
    while k < fx.mem.i16(g + 0x9A) {
        let p = fx.mem.read_u32(g + 0x80) + 16 * k as u32;
        if fx.mem.i16(p + 0xE) >= 5 {
            fx.step_down(p + 0xC, 0, 0x10);
        }
        k += 1;
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x2994);
    node(fx, 0x29AC, D + 0x6D0);
    let y = ee::add(fx.mem.f(D + 0x6D4), 1.0);
    fx.mem.setf(D + 0x6D4, y);
    node(fx, 0x299C, D + 0x250);
    match n(fx) {
        0x58 => {
            aim(fx, sp, 0x299C, 0x29A4, D + 0x770);
            fx.mem.write_u32(D + 0x7EC, 0x3F80_0000);
            let mut k = 0;
            while k < fx.mem.i16(D + 0x87A) {
                let p = fx.mem.read_u32(D + 0x860) + 16 * k as u32;
                fx.mem.write_u8(p + 0xC, 0x80);
                let p = fx.mem.read_u32(D + 0x860) + 16 * k as u32;
                fx.mem.set16(p + 0xE, 0);
                k += 1;
            }
        }
        0x4E => aim(fx, sp, 0x29A0, 0x29A8, D + 0x1880),
        0x46 => {
            node(fx, 0x299C, D + 0x90);
            fx.mem.write_u8(D + 0xEB, 0x80);
            fx.mem.write_u32(D + 0xEC, 0x4170_0000);
            cloud_place(fx, D + 0xE0, 16, true);
            aim(fx, sp, 0x299C, 0x29A4, D + 0x770);
        }
        0x19 => {
            node(fx, 0x299C, D + 0x90);
            fx.mem.write_u8(D + 0xEB, 0x40);
        }
        _ => {}
    }
    // s0 = D, s1 = the OT, s2 the param (-1, 64 bits), s3 the caller's.
    let regs = [Some(D), Some(OT), None, None];
    fx.xform_world_saving(D, 0x01E0_3108, regs);
    fx.saved(sp - 0xD0, &[(0x40, u64::MAX)]);
    fx.xform_billboard_saving(D + 0x70, 0x01E0_3114, regs);
    fx.saved(sp - 0xF0, &[(0x40, u64::MAX)]);
    for (x, ra) in [(0x230u32, 0x01E0_311C), (0x770, 0x01E0_3124), (0x1880, 0x01E0_312C)] {
        fx.xform_world_saving(D + x, ra, regs);
        fx.saved(sp - 0xD0, &[(0x40, u64::MAX)]);
    }
    fx.xform_billboard_saving(D + 0x6B0, 0x01E0_3134, regs);
    fx.saved(sp - 0xF0, &[(0x40, u64::MAX)]);
    fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    fx.disc(D + 0x760, D + 0x6B0);
    plate::plates(fx, PLATES, OT, D + 0x720, D + 0x6B0);
    let m = n(fx);
    if m < 0x14 {
        fx.step_up(D + 0x767, 0x80, 8);
        fx.step_up(D + 0x72B, 0x80, 8);
    } else if m < 0x7D {
        fx.step_down(D + 0x767, 0, 8);
        fx.step_down(D + 0x72B, 0, 8);
    }
    if n(fx) < 0x7D {
        let a = fx.mem.i32(D + 0x758).wrapping_add(0x200);
        fx.mem.set32(D + 0x758, a);
    } else {
        fx.mem.write_u32(D + 0x758, 0);
    }
    if n(fx) >= 0x46 {
        blade::draw(fx, OT, D + 0x7E0, D + 0x770);
        fstep_up(fx, D + 0x7EC, 10.0, 1.0);
        blades_fade(fx, D + 0x7E0);
    }
    if n(fx) >= 0x4E {
        blade::draw(fx, OT, D + 0x18F0, D + 0x1880);
        fstep_up(fx, D + 0x18FC, 10.0, 1.0);
        blades_fade(fx, D + 0x18F0);
    }
    rain::draw(fx, RAIN_DRAW, OT, D + 0x2A0, D + 0x230, 1.5);
    let m = n(fx);
    if (0x32..0x42).contains(&m) {
        fx.step_up(D + 0x2A7, 0x80, 0x10);
    } else if m >= 0x42 {
        fx.step_down(D + 0x2A7, 0, 0x20);
    }
    if n(fx) >= 0x19 {
        for k in [0x78u32, 0x74, 0x70] {
            fx.mem.write_u32(sp + k, 0);
        }
        fx.particle_quads(D + 0x70, D + 0xE0, sp + 0x70);
        fx.step_down(D + 0xEB, 0, 4);
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x2994, m);
    let left = fx.mem.i32(D + 0x2990) - 1;
    fx.mem.set32(D + 0x2990, left);
    if m == 0x5B {
        2
    } else if m == 0x3C {
        1
    } else if (0x46..0x5B).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
