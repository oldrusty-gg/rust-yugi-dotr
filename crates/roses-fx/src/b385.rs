//! BMODULE b385 (card 30), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02A90, data area 0x1ED0000) [F] (helper C9, 31
//! Oct 2026): a flickering disc (+0x3200) between the attacker's nodes
//! 0x4D / 0x53 from update 0x46 (swelling to 15 from 0x76) with a fan
//! (+0x2D20, `fan`) facing node 0x51 on 0x46..0x75; from 0x64 a ribbon
//! (+0x70, `ribbon`) from node 0x51 to 15 above the target, waving; from
//! 0x7F a ring of blades (+0x380, `blade`) and a sphere (+0x1490,
//! `sphere`, swelling to 1.6) at the target, fading from 0xDC. Returns 1
//! on update 0x75, 6 on 0x7F..0xEF, 2 on 0xF0, 8 after 0x104.
//!
//! Library at its own addresses: `colours4` (0x1E00080), the frame quad
//! (0x1E00120), `sphere` setup / draw (0x1E00240 / 0x1E00330), `disc`
//! (0x1E00A00), `ribbon` push / draw (0x1E00F60 / 0x1E010B0), `fan` setup
//! / draw (0x1E01A70 / 0x1E01BD0), `blade` setup / draw (0x1E021E0 /
//! 0x1E02380).
use crate::b142::fstep_up;
use crate::blib::{blade, sphere};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BLADE_SETUP: u32 = 0x01E0_21E0;
const FAN_TEMPLATE: u32 = 0x01E0_3400;
const LEFT: u32 = D + 0x3248;
const N: u32 = D + 0x324C;
const NODES: u32 = D + 0x3254;

/// The entry 0x1E02A90.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x90);
    ret
}

fn bytes(fx: &mut Fx, at: &[(u32, u8)]) {
    for &(o, v) in at {
        fx.mem.write_u8(D + o, v);
    }
}

fn halves(fx: &mut Fx, at: &[(u32, i32)]) {
    for &(o, v) in at {
        fx.mem.set16(D + o, v);
    }
}

fn node(fx: &mut Fx, k: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(NODES + 4 * k);
    fx.node_position(h, s, out);
}

fn copy4(fx: &mut Fx, from: u32, to: u32) {
    for c in 0..4u32 {
        let v = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, v);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x3270);
    for x in [0u32, 0x2A0, 0x2CB0, 0x310, 0x1420, 0x3190] {
        fx.xform_init(D + x);
    }
    for (k, n) in [0x4E, 0x51, 0x4D, 0x53].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set32(NODES + 4 * k as u32, r);
    }
    fx.mem.set32(LEFT, 0x104);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x3250, param);
    fx.mem.set32(D + 0x3240, 0x1A);
    fx.mem.set32(D + 0x3244, 0x8000);
    // The disc (+0x3200): radius 5.
    fx.mem.write_u32(D + 0x3200, 0x1008_0000);
    bytes(fx, &[(0x3206, 0xB0), (0x3205, 0x70), (0x3204, 0x40), (0x3207, 0)]);
    fx.mem.write_u32(D + 0x3208, 0x40A0_0000);
    halves(fx, &[(0x320C, 0x24), (0x320E, 0)]);
    // The ribbon (+0x70, points +0xA0), texture 5 / 0.
    fx.texture5(sp + 0x80, 0);
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(D + 0x70, tex);
    fx.mem.write_u32(D + 0x78, 0x8003);
    bytes(fx, &[(0x76, 0x80), (0x75, 0x80), (0x74, 0x80), (0x77, 0)]);
    halves(fx, &[(0x80, 0), (0x82, 0), (0x84, 0x10), (0x86, 0x20), (0x88, 0), (0x8A, 0), (0x8C, 0x80), (0x8E, 0x40)]);
    fx.mem.write_u32(D + 0x7C, 0x42C8_0000);
    halves(fx, &[(0x92, 0xA), (0x94, 0), (0x96, 0x24), (0x98, 0x20), (0x9A, 0), (0x9C, 1)]);
    // The fan (+0x2D20).
    fx.mem.write_u32(D + 0x2D24, 1);
    halves(fx, &[(0x2D30, 0x40), (0x2D32, 0), (0x2D34, 0x40), (0x2D36, 0x40)]);
    bytes(fx, &[(0x2D2A, 0x80), (0x2D29, 0x80), (0x2D28, 0x80), (0x2D2B, 0x80)]);
    for (o, w) in [(0x2D40u32, 0x3F00_0000u32), (0x2D44, 0x3F80_0000), (0x2D48, 0x40A0_0000), (0x2D4C, 0)] {
        fx.mem.write_u32(D + o, w);
    }
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.fan_setup(D + 0x2D20, tex, 3.0, FAN_TEMPLATE);
    // The blades (+0x380, blades +0x420), texture 5 / 1.
    fx.texture5(sp + 0x80, 1);
    fx.mem.write_u32(D + 0x384, 1);
    halves(fx, &[(0x390, 0), (0x392, 0), (0x394, 0x20), (0x396, 0x40)]);
    bytes(fx, &[(0x38A, 0xB0), (0x389, 0x70), (0x388, 0x40), (0x38B, 0x80)]);
    for (o, w) in [(0x400u32, D + 0x420), (0x404, 0x40A0_0000), (0x408, 0x41F0_0000), (0x40C, 0x3F4C_CCCD), (0x410, 0), (0x414, 0x1_0000), (0x38C, 0)] {
        fx.mem.write_u32(D + o, w);
    }
    halves(fx, &[(0x418, 0), (0x41A, 0x100), (0x41C, 0x3E7), (0x41E, 3)]);
    let tex = fx.mem.read_u32(sp + 0x80);
    blade::setup(fx, BLADE_SETUP, D + 0x380, tex);
    // The sphere (+0x1490), texture 5 / 2: radius 30.
    fx.texture5(sp + 0x80, 2);
    fx.mem.write_u32(D + 0x1494, 3);
    halves(fx, &[(0x14A0, 0), (0x14A2, 0), (0x14A4, 0x80), (0x14A6, 0x80)]);
    bytes(fx, &[(0x149A, 0xB0), (0x1499, 0x70), (0x1498, 0x40), (0x149B, 0x80)]);
    let tex = fx.mem.read_u32(sp + 0x80);
    sphere::setup(fx, D + 0x1490, tex, 30.0);
    for o in [0x1428u32, 0x1424, 0x1420] {
        fx.mem.write_u32(D + o, 0);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(N);
    node(fx, 2, sp + 0x60);
    node(fx, 3, sp + 0x70);
    fx.midpoint(sp + 0x60, sp + 0x70, D + 0x3210);
    if n(fx) < 0x77 {
        copy4(fx, D + 0x3210, sp + 0x60);
        node(fx, 1, sp + 0x70);
        fx.look_angles(sp + 0x60, sp + 0x70, D + 0x2CC0);
        node(fx, 1, D + 0x2CD0);
    }
    node(fx, 1, D + 0x31B0);
    if n(fx) == 0x64 {
        let t = fx.target_handle();
        fx.unit_point(1, t, D + 0x3220);
        fx.mem.write_u32(D + 0x3224, 0x4170_0000);
    }
    if n(fx) < 0x65 {
        node(fx, 1, D + 0x2C0);
    }
    if n(fx) >= 0x64 {
        copy4(fx, D + 0x2C0, sp + 0x50);
        let a = fx.mem.i32(D + 0x3244);
        let s = fx.tsin(a);
        let y = ee::add(fx.mem.f(sp + 0x54), ee::mul(20.0, s));
        fx.mem.setf(sp + 0x54, y);
        fx.ribbon_push(D + 0x70, D + 0xA0, sp + 0x50);
        let m = fx.mem.i32(D + 0x3240);
        fx.approach_by(D + 0x2C0, D + 0x3220, m);
        let m = fx.mem.i32(D + 0x3240);
        if m != 0 {
            fx.mem.set32(D + 0x3240, m - 1);
            let a = fx.mem.i32(D + 0x3244).wrapping_add(0x4EC);
            fx.mem.set32(D + 0x3244, a);
        }
    }
    if n(fx) == 0x7F {
        let t = fx.target_handle();
        fx.unit_point(1, t, D + 0x330);
        fx.mem.write_u32(D + 0x334, 0x4120_0000);
        copy4(fx, D + 0x330, D + 0x1440);
        fx.mem.write_u32(D + 0x1444, 0x4120_0000);
    }
    // s0 = D, s1 = the OT, s2 = sp + 0x70, s3 the runner's 0.
    let regs = [Some(D), Some(OT), Some(sp + 0x70), Some(0)];
    for (x, ra, board) in [(0u32, 0x01E0_2FBCu32, false), (0x2A0, 0x01E0_2FC4, true), (0x2CB0, 0x01E0_2FCC, false), (0x310, 0x01E0_2FD4, false), (0x1420, 0x01E0_2FDC, false), (0x3190, 0x01E0_2FE4, true)] {
        if board {
            fx.xform_billboard_saving(D + x, ra, regs);
        } else {
            fx.xform_world_saving(D + x, ra, regs);
        }
    }
    if n(fx) >= 0x46 {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if n(fx) >= 0x7F {
        if n(fx) == 0xB7 {
            let f = fx.mem.read_u32(D + 0x384) | 0x4000_0000;
            fx.mem.write_u32(D + 0x384, f);
        }
        blade::draw(fx, OT, D + 0x380, D + 0x310);
        fstep_up(fx, D + 0x38C, 35.0, 1.0);
        if n(fx) >= 0xDC {
            if n(fx) == 0xDC {
                let f = fx.mem.read_u32(D + 0x384) & 0xBFFF_FFFF;
                fx.mem.write_u32(D + 0x384, f);
                fx.mem.write_u32(D + 0x40C, 0x40C0_0000);
                let mut j = 0u32;
                while (j as i32) < fx.mem.i16(D + 0x41A) {
                    fx.mem.set16(D + 0x42E + 0x10 * j, 7);
                    j += 1;
                }
            }
            let mut j = 0u32;
            while (j as i32) < fx.mem.i16(D + 0x41A) {
                let p = fx.mem.read_u32(D + 0x400) + 0x10 * j;
                if fx.mem.i16(p + 0xE) >= 0xB {
                    fx.step_down(p + 0xC, 0, 8);
                }
                j += 1;
            }
            fx.step_down(D + 0x149B, 0, 8);
        }
        sphere::draw(fx, OT, D + 0x1490, D + 0x1420);
        fx.approach(D + 0x1420, f32::from_bits(0x3FCC_CCCD), f32::from_bits(0x3CF5_C28F));
        fstep_up(fx, D + 0x1444, 40.0, 1.0);
        for o in [0x1430u32, 0x1434, 0x1438] {
            let w = fx.mem.read_u32(D + o).wrapping_add(0x100);
            fx.mem.write_u32(D + o, w);
        }
    }
    // The disc's colour flickers (rand's frame: the entry's s0 = D).
    for (o, ra, add) in [(0x3206u32, 0x01E0_318Cu32, 0x70), (0x3205, 0x01E0_31AC, 0x30), (0x3204, 0x01E0_31CC, 0)] {
        let r = fx.rand_from(D, ra);
        fx.mem.write_u8(D + o, (r.wrapping_rem(0x40) + add) as u8);
    }
    if n(fx) >= 0x46 {
        fx.disc(D + 0x3200, D + 0x3190);
        if n(fx) >= 0x76 {
            let f = fx.mem.read_u32(D + 0x3200) & 0xEFFF_FFFF;
            fx.mem.write_u32(D + 0x3200, f);
            fx.mem.write_u32(D + 0x3208, 0x4170_0000);
            fx.step_down(D + 0x3207, 0, 8);
        } else {
            fx.step_up(D + 0x3207, 0xFF, 0x10);
        }
    }
    if (0x46..0x76).contains(&n(fx)) {
        let r = fx.rand_from(D, 0x01E0_327C);
        fx.mem.write_u8(D + 0x2D2B, (r.wrapping_rem(8) + 8) as u8);
        fx.fan_draw(D + 0x2D20, D + 0x2CB0);
    }
    if n(fx) >= 0x65 {
        if n(fx) == 0x76 {
            fx.mem.write_u8(D + 0x77, 0x80);
            let mut j = 0u32;
            while (j as i32) < fx.mem.i16(D + 0x9A) {
                fx.mem.write_u8(D + 0xAC + 0x10 * j, 0x80);
                j += 1;
            }
        }
        fx.ribbon_draw(D + 0x70, D + 0xA0, D);
    }
    let m = n(fx) + 1;
    fx.mem.set32(N, m);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if m == 0xF0 {
        2
    } else if m == 0x75 {
        1
    } else if (0x7F..0xF0).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
