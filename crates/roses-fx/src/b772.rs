//! BMODULE b772 (card 429), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04630, data area a0 = 0x1ED0000) [F] (helper B4, 31
//! Oct 2026): b076's "a0 = D" family (s4 = D, s0 = the OT). On update 2 an
//! arc (+0xEF0, nodes 0x22 / 0x21, held 255 counts); on 152 a bead-trail
//! emitter (+0xE64, `emit::bead_trails`, riding the attacker's node 0x22,
//! beads pool of 40 at +0xA4); on 153 at the target a fade (+0x1CC0), a
//! flash (+0x1CC8), two ribbons (+0x1090) and a camera shake (+0x40); a
//! second flash group (+0x1D14, never started); the stage light pulsed by
//! 0x1031C0 (+0xA0: down on update 2, up on 194). Returns 1 on update 143,
//! 2 on 150, 6 on 153..181, 8 on 200 (and at once for a parameter of -2 /
//! -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `arcs`
//! 0x1E00410, `flash` 0x1E01690, `ribbon` 0x1E02240, `fade` 0x1E02DD0,
//! `beads` 0x1E03310, `bead_trails` 0x1E040E0 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::set_counter4};
use crate::bus::Bus;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x1F74;

/// The entry 0x1E04630 (a0 = D, a1 = param; s4 = D, s0 = the OT, s1 =
/// the parameter).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x70);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x2080);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    for k in 0..3 {
        fx.texture(5, k);
    }
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0x3E, 0), (0x14, 0x45, 0), (0x18, 0x22, 0), (0x1C, 0x21, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xC2);
    // The entry's s0 = the OT, s1 = 3 (the texture loop's), s2: the caller's.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_4794, [Some(OT), Some(3), None]);
    0
}

/// Update 2 [F 0x1E047F8]: the arc (+0xEF0, points +0xF50), nodes +0x18
/// / +0x1C.
fn arcs_start(fx: &mut Fx) {
    let g = D + 0xEF0;
    set_counter4(fx, g, 0xFF, 0, 0xFF);
    fx.mem.write_u8(g + 4, 1);
    fx.mem.write_u32(g + 8, D + 0xF50);
    fx.mem.write_u8(g + 0x30, 0x14);
    fx.mem.write_u8(g + 0x31, 8);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(g + 0x32, b);
    let b = fx.mem.read_u8(D + 0x1C);
    fx.mem.write_u8(g + 0x33, b);
    fx.mem.write_u32(g + 0x38, 0x3F80_0000);
    fx.mem.write_u8(g + 0x34, 0);
    fx.mem.set16(g + 0x36, 0);
    fx.mem.write_u32(g + 0x40, 0x4120_0000);
    for (o, v) in [(0x44u32, 0x80u8), (0x45, 0x80), (0x46, 0x80), (0x47, 0xC0), (0x4A, 0xC0), (0x49, 0xC0), (0x48, 0x20)] {
        fx.mem.write_u8(g + o, v);
    }
    let mut a = 0u32;
    while a < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0x1E + 2 * a, -1);
        fx.mem.set16(g + 0xC + 2 * a, -1);
        a += 1;
    }
}

/// Update 152 [F 0x1E048C4]: the emitter (+0xE64) at node +0x18.
fn emitter_start(fx: &mut Fx, param: i32) {
    let e = D + 0xE64;
    set_counter4(fx, e, 9, 0, 0xFF);
    fx.mem.write_u8(e + 4, 2);
    fx.mem.write_u8(e + 5, 1);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(e + 6, b);
    fx.mem.write_u32(e + 8, D + 0xA4);
    for (o, v) in [(0x10u32, 0x28u8), (0x12, 0xC), (0x11, 0x20), (0x13, 1), (0x16, 2), (0x14, 4)] {
        fx.mem.write_u8(e + o, v);
    }
    fx.mem.write_u32(e + 0x18, 0x4020_0000);
    fx.mem.write_u32(e + 0x1C, 0x3F4C_CCCD);
    fx.mem.write_u32(e + 0x20, 0x42A0_0000);
    fx.mem.write_u32(e + 0xC, BYTES);
    for o in [0x2Bu32, 0x28, 0x29, 0x2A] {
        fx.mem.write_u8(e + o, 0x80);
    }
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x18);
    node_position_saving(fx, h, n, e + 0x3C, u64::from(OT), Some(i64::from(param) as u64), 0x01E0_4970);
    copy_v4(fx, e + 0x3C, e + 0x4C);
}

/// Update 153 [F 0x1E04998]: at the target (sp + 0x60).
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x60;
    set_counter4(fx, D + 0x1CC0, 8, 0, 0xFF);
    fx.mem.write_u8(D + 0x1CC4, 1);
    let g = D + 0x1CC8;
    set_counter4(fx, g, 0x20, 0, 0xFF);
    fx.mem.write_u8(g + 5, 0);
    fx.mem.write_u8(g + 4, 0x81);
    fx.mem.write_u8(g + 7, 9);
    fx.mem.write_u32(g + 0x10, 0x447A_0000);
    fx.mem.write_u32(g + 0x14, 0);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x20), (9, 0x20), (8, 0x80), (0xF, 0xD0)] {
        fx.mem.write_u8(g + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(g + o, 0);
    }
    copy_v4(fx, at, g + 0x1C);
    for k in 0..2u32 {
        let r = D + 0x1090 + 0x68 * k;
        fx.mem.write_u8(r + 4, 5);
        if k != 0 {
            set_counter4(fx, r, 0x20, 0, 0xFF);
            fx.mem.write_u32(r + 0x1C, 0x42A0_0000);
            fx.mem.write_u8(r + 0x23, 0x80);
            fx.mem.write_u8(r + 0x27, 0x40);
            fx.mem.write_u32(r + 0x14, 0x3FC0_0000);
            let b = fx.mem.read_u8(r + 4) | 2;
            fx.mem.write_u8(r + 4, b);
            fx.mem.write_u8(r + 0xA, 1);
        } else {
            set_counter4(fx, r, 0x1E, 0, 0xFF);
            fx.mem.write_u32(r + 0x1C, 0x4248_0000);
            fx.mem.write_u8(r + 0x23, 0x10);
            fx.mem.write_u8(r + 0x27, 0x60);
            fx.mem.write_u32(r + 0x14, 0x4000_0000);
            fx.mem.write_u8(r + 0xA, 0);
            fx.mem.set32(r + 0x58, 0x40);
            fx.mem.set32(r + 0x5C, 0x80);
            fx.mem.set32(r + 0x60, 0x100);
        }
        fx.mem.write_u8(r + 0xC, 3);
        fx.mem.write_u8(r + 8, 0xA0);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 0xB, 2);
        let x = fx.rand_from(OT, 0x01E0_4B60);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, BYTES);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, at, r + 0x28);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x20, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x18, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, D + 0x60);
}

/// `flash` called from the entry from return address `ra`, leaving its
/// frame's saved s7 / fp (0: the runner's) and `ra` (0x270: +0xA0, +0xB0,
/// +0xC0): a following bead trail's curve samples keep these words as
/// their w [run b772; F 0x1E01690].
pub(crate) fn flash_saving(fx: &mut Fx, at: u32, g: u32, n: i32, ra: u32) -> i32 {
    let r = blib::star::flash(fx, at, g, OT, n);
    let f = fx.sp - 0x270;
    fx.saved(f, &[(0xA0, 0), (0xB0, 0), (0xC0, u64::from(ra))]);
    r
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(OT), 0x01E0_47BC);
    // The entry's s1 when 0x1031C0 saves it: the parameter or the ribbon
    // loop's end.
    let mut s1 = i64::from(param) as u64;
    match fx.mem.u16(D) {
        0xC8 => return 8,
        0x99 => {
            strike(fx, sp);
            s1 = 2;
        }
        0x98 => emitter_start(fx, param),
        2 => arcs_start(fx),
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_4C58, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_4C68, [Some(u64::from(OT)), Some(s1)]);
    blib::fade::fade(fx, D + 0x1CC0, OT);
    blib::arc::arcs(fx, 0x01E0_0410, D + 0xEF0, OT, 1);
    blib::streak::ribbon(fx, 0x01E0_2240, D + 0x1090, OT, 0x1E);
    blib::star::flash(fx, 0x01E0_1690, D + 0x1CC8, OT, 1);
    flash_saving(fx, 0x01E0_1690, D + 0x1D14, 8, 0x01E0_4CC8);
    blib::emit::bead_trails(fx, 0x01E0_40E0, D + 0xE64, 1);
    blib::beam::beads(fx, 0x01E0_3310, D + 0xA4, OT, 0x28);
    let n = fx.mem.u16(D);
    if (0x99..0xB6).contains(&n) {
        6
    } else if n == 0x96 {
        2
    } else if n == 0x8F {
        1
    } else {
        0
    }
}
