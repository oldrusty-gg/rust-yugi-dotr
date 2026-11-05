//! BMODULE b489 (card 346), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E00FA0, data area 0x1ED0000) [F] (helper C2, 30
//! Oct 2026): b057's spark column (+0xE0, `blib::trail::sparks`, white,
//! fading in from update 21) rising round the attacker's node 1, each
//! spark fading in as it rises and respawned at the foot above 200; on
//! updates 92..235 the target's added colour (0x1F7790) flickering at
//! random (cleared on update 236), the sparks fading out after 237.
//! Returns 1 on update 81, 6 on 91..235, 2 on 236, 8 after 255.
//!
//! Library: `spawn::scatter` (0x1E00080), `trail::sparks` (0x1E00240)
//! [`crate::blib`].
use crate::b057::spark_place;
use crate::blib;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E00FA0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xA0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x1220);
    for x in [0u32, 0x1110, 0x70, 0x1180] {
        fx.xform_init(D + x);
    }
    let h = fx.self_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set32(D + 0x1210, r);
    fx.mem.set32(D + 0x1200, 0x100);
    fx.mem.set32(D + 0x1204, 1);
    fx.mem.set32(D + 0x1208, param);
    for k in [0x1214u32, 0x120F, 0x120C, 0x120D, 0x120E] {
        fx.mem.write_u8(D + k, 0x80);
    }
    fx.texture5(sp + 0x90, 0);
    // The sparks (+0xE0): 256 at +0x110, width 0.2, length 22.
    let g = D + 0xE0;
    let tex = fx.mem.read_u32(sp + 0x90);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x6000_0003);
    for (k, x) in [(0xAu32, 0x80u8), (9, 0x80), (8, 0x80), (0xB, 0)] {
        fx.mem.write_u8(g + k, x);
    }
    fx.mem.write_u32(g + 0x20, 0x3E4C_CCCD);
    fx.mem.write_u32(g + 0x24, 0x41B0_0000);
    for (k, x) in [(0x10u32, 0), (0x12, 0), (0x14, 0x20), (0x16, 0x20), (0x18, 0), (0x1A, 0), (0x1C, 0x80), (0x1E, 0x40)] {
        fx.mem.set16(g + k, x);
    }
    fx.mem.write_u32(g + 0xC, D + 0x110);
    fx.mem.write_u32(g + 0x28, 0x4120_0000);
    fx.mem.set16(g + 0x2C, 0x100);
    fx.mem.write_u8(g + 0x2E, 8);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 3.0);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x10C) {
        spark_place(fx, k as u32, 0x7D0, [0x01E0_1104, 0x01E0_1120, 0x01E0_114C]);
        let e = fx.mem.read_u32(D + 0xEC).wrapping_add(16 * k as u32);
        fx.mem.write_u8(e + 0xC, 0);
        k += 1;
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x1204);
    let node = |fx: &mut Fx, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + 0x1210);
        fx.node_position(h, s, out);
    };
    let t = fx.target_handle();
    fx.unit_position(t, D + 0x11F0);
    node(fx, D + 0x90);
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0x70);
    node(fx, sp + 0x80);
    fx.look_angles(sp + 0x70, sp + 0x80, D + 0x80);
    node(fx, D + 0x11A0);
    node(fx, D + 0x1130);
    // s0 = D, s1 = the OT, s2 = sp + 0x80, s3 the caller's.
    let regs = [Some(D), Some(OT), Some(sp + 0x80), None];
    fx.xform_world_saving(D, 0x01E0_1284, regs);
    fx.xform_billboard_saving(D + 0x1110, 0x01E0_128C, regs);
    fx.xform_world_saving(D + 0x70, 0x01E0_1294, regs);
    fx.xform_world_saving(D + 0x1180, 0x01E0_129C, regs);
    if (0x5C..0xEC).contains(&n(fx)) {
        let t = fx.target_handle();
        fx.unit_add_colour_from(t, D + 0x120C, [D, OT], 0x01E0_12CC);
        for (at, ra) in [(0x120Eu32, 0x01E0_12D4), (0x120D, 0x01E0_12F0), (0x120C, 0x01E0_130C)] {
            let r = fx.rand_from(D, ra);
            fx.mem.set8(D + at, r % 128);
        }
    }
    if n(fx) == 0xEC {
        let t = fx.target_handle();
        fx.unit_add_colour_from(t, 0, [D, OT], 0x01E0_1348);
    }
    blib::trail::sparks(fx, OT, D + 0xE0, D + 0x70);
    let m = n(fx);
    if m >= 0xED {
        fx.step_down(D + 0xEB, 0, 8);
    } else if (0x15..0xEC).contains(&m) {
        fx.step_up(D + 0xEB, 0x80, 8);
    }
    let mut k = 0;
    while k < fx.mem.i16(D + 0x10C) {
        let p = D + 16 * k as u32;
        if fx.mem.i16(p + 0x11E) >= 0 {
            let y = ee::add(fx.mem.f(p + 0x114), 4.0);
            fx.mem.setf(p + 0x114, y);
            fx.step_up(p + 0x11C, 0x80, 0x10);
            if !ee_le(fx.mem.f(p + 0x114), 200.0) {
                spark_place(fx, k as u32, 0x1E, [0x01E0_1410, 0x01E0_142C, 0x01E0_1458]);
                fx.mem.write_u8(p + 0x11C, 0);
                fx.mem.set16(p + 0x11E, 0);
            }
        }
        k += 1;
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x1204, m);
    let left = fx.mem.i32(D + 0x1200) - 1;
    fx.mem.set32(D + 0x1200, left);
    if m == 0xEC {
        2
    } else if m == 0x51 {
        1
    } else if (0x5B..0xEC).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
