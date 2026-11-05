//! BMODULE b133 (card 372), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E042E0, data area a0 = 0x1ED0000, 0x2E30 bytes)
//! [F]: a record of the shared-library family
//! (as b076). The entry (0xC0 frame, s0 = D, s1 = param, s2 = the OT)
//! steps a counter (D+0, u16), reads the target's position (sp + 0xA0)
//! and on fixed updates starts library records: 6 two bars (+0x23DC) and
//! a flash (+0x27EC) at the target's node 1; 0x28 / 0xB6 fifteen spokes
//! (+0x1E3C + 0x60 k, `spokes::lines`) at the attacker's nodes k % 5 aimed
//! away from its point; 0x3C five node trails (+0x1E0 + 0x50 k, local
//! [`trails`], eight slots each of the pool +0x2968) and a camera shake;
//! 0x4A five node flashes; 0xB5 five glows (+0xA0 + 0x40 k, local
//! [`glows`]) flying from the nodes to the target with bead trails
//! (+0x1B80 + 0x8C k) and a shake; 0xCC two flashes and thirteen bars at
//! the target; 0xD0 eight bead beams (+0x370 pool). Every update draws
//! them all; on 0x3C..0x42 and 0xB5 the seven node requests are renewed.
//! Returns 8 on 0xFA (and at once for -2 / -3), 6 on 0xD4..0xE0, 2 on
//! 0xE1, 1 on 0xCA.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `beads`
//! 0x1E01A00, `bead_trails` 0x1E027D0, `bars` 0x1E02D20, `flash`
//! 0x1E03730, `spokes::lines` 0x1E014E0 [`crate::blib`]; local `glows`
//! 0x1E00410, `trails` 0x1E00BF0.
use crate::b076::{copy_v4, rand_bytes, shake_saving};
use crate::blib::exe::{aim_angles, camera_vp_into, countdown, first_free, mul_mem, node_source, set_counter, set_counter4, tex_rect_strip, translation_into};
use crate::blib;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const ASPECT: u32 = 0x3FAA_3D71;
/// The sprite specs (0x18 bytes: texture kind +0, id +4) of `glows` /
/// `trails`.
const GLOW_SPECS: u32 = 0x01E0_5480;
const TRAIL_SPECS: u32 = 0x01E0_54B0;

/// The entry 0x1E042E0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp)
    };
    fx.leave(0xC0);
    ret
}

/// The seven node requests (init and 0x3C..0x42 / 0xB5).
fn requests(fx: &mut Fx) {
    for (at, add, clear) in [(0xCu32, 4, 1), (0x10, 0xD, 0), (0x14, 0x16, 0), (0x18, 0x1F, 0), (0x1C, 0x28, 0)] {
        let node = fx.mem.i32(D + 0x2C) + add;
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    let v = fx.mem.i32(D + 0x2C) + 1;
    fx.mem.set32(D + 0x2C, v);
    let v = fx.mem.i32(D + 0x2C) & 7;
    fx.mem.set32(D + 0x2C, v);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 1, 0);
    fx.mem.set32(D + 0x20, r);
    let t = fx.mem.i32(D + 8);
    fx.request_node(t, 1, 1);
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x2E30);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x30);
    fx.texture(4, 0xF);
    fx.texture(4, 2);
    requests(fx);
    rand_bytes(fx, D + 0x2D28, 0x100, 0x01E0_44A8, [Some(D), Some(param as u32), Some(OT)]);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, sp + 0xA0);
    match fx.mem.u16(D) {
        0xFA => return 8,
        0xD0 => beads_start(fx, sp),
        0xCC => bars_start(fx, sp),
        0xB5 => glows_start(fx, sp),
        0x4A => flashes_start(fx),
        0x3C => trails_start(fx, sp),
        0xB6 | 0x28 => spokes_start(fx, sp),
        6 => first_start(fx),
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_51D4, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    blib::emit::bead_trails(fx, 0x01E0_27D0, D + 0x1B80, 5);
    blib::spokes::lines(fx, 0x01E0_14E0, D + 0x1E3C, OT, 0xF);
    glows(fx, D + 0xA0, OT, 5);
    trails(fx, D + 0x1E0, OT, 5);
    blib::beam::beads(fx, 0x01E0_1A00, D + 0x370, OT, 0x46);
    blib::cross::bars(fx, 0x01E0_2D20, D + 0x23DC, OT, 0xD);
    blib::star::flash(fx, 0x01E0_3730, D + 0x27EC, OT, 5);
    let u = fx.mem.u16(D);
    if u == 0xB5 || (0x3C..=0x42).contains(&u) {
        requests(fx);
    }
    let u = fx.mem.u16(D);
    if (0xD4..0xE1).contains(&u) {
        6
    } else if u == 0xE1 {
        2
    } else if u == 0xCA {
        1
    } else {
        0
    }
}

fn bytes(fx: &mut Fx, r: u32, list: &[(u32, u8)]) {
    for &(o, v) in list {
        fx.mem.write_u8(r + o, v);
    }
}

/// Update 6 [F 0x1E04548]: two bars (+0x23DC + 0x50 k) and a flash
/// (+0x27EC) on the attacker's node 1 (+0x20), mirrored when D+0x34 is
/// 0x8000.
fn first_start(fx: &mut Fx) {
    for k in 0..2i32 {
        let r = D + 0x23DC + 0x50 * k as u32;
        set_counter4(fx, r, 0x20, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0x20);
        fx.mem.write_u8(r + 6, b);
        fx.mem.write_u8(r + 4, 0x80);
        let fk = ee::from_i32(k);
        fx.mem.setf(r + 0x10, ee::sub(1000.0, ee::mul(400.0, fk)));
        fx.mem.write_u32(r + 0x14, 0x41A0_0000);
        bytes(fx, r, &[(0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80), (0xA, 0), (9, 0x40), (8, 0)]);
        fx.mem.setf(r + 0x18, ee::mul(90.0, fk));
        fx.mem.write_u32(r + 0x1C, 0);
        fx.mem.write_u32(r + 0x44, 0x4040_0000);
        fx.mem.write_u32(r + 0x48, 0x4000_0000);
        if fx.mem.i32(D + 0x34) == 0x8000 {
            let v = ee::mul(fx.mem.f(r + 0x48), -1.0);
            fx.mem.setf(r + 0x48, v);
        }
    }
    let g = D + 0x27EC;
    set_counter4(fx, g, 0x20, 0, 0xFF);
    fx.mem.write_u8(g + 5, 1);
    let b = fx.mem.read_u8(D + 0x20);
    fx.mem.write_u8(g + 6, b);
    fx.mem.write_u8(g + 4, 0x80);
    fx.mem.write_u8(g + 7, 9);
    fx.mem.write_u32(g + 0x10, 0x4416_0000);
    fx.mem.write_u32(g + 0x14, 0);
    fx.mem.write_u32(g + 0x18, 0x41F0_0000);
    bytes(fx, g, &[(0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80), (0xA, 0), (9, 0x40), (8, 0)]);
    fx.mem.write_u32(g + 0x40, 0x4040_0000);
    fx.mem.write_u32(g + 0x44, 0x4000_0000);
    if fx.mem.i32(D + 0x34) == 0x8000 {
        let v = ee::mul(fx.mem.f(g + 0x44), -1.0);
        fx.mem.setf(g + 0x44, v);
    }
}

/// Updates 0x28 / 0xB6 [F 0x1E046FC]: fifteen spokes (+0x1E3C + 0x60 k,
/// delays 2 (k / 5)) at the attacker's nodes k % 5, turned along node -
/// the attacker's point (0x1F65D0 kind 1, sp + 0x90; 0x101940).
fn spokes_start(fx: &mut Fx, sp: u32) {
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, sp + 0x90);
    for k in 0..15i32 {
        let r = D + 0x1E3C + 0x60 * k as u32;
        fx.mult(0x6666_6667, k);
        let delay = ((k / 5) << 1) & 0xFF;
        set_counter4(fx, r, 0x10, delay, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        let h = fx.self_handle();
        let (_, slot) = fx.div(k, 5);
        fx.node_position(h, slot, r + 0x20);
        fx.mem.write_u8(r + 8, 0x1E);
        let x = fx.rand_from(D, 0x01E0_47B0) >> 4;
        fx.mem.set8(r + 9, x);
        fx.mem.write_u8(r + 0xA, 3);
        fx.mem.write_u32(r + 0xC, D + 0x2D28);
        fx.mem.write_u32(r + 0x10, 0x3F80_0000);
        fx.mem.write_u32(r + 0x14, 0x3F00_0000);
        bytes(fx, r, &[(0x19, 0x80), (0x1A, 0x80), (0x18, 0x40), (0x1B, 0x80), (0x1E, 0x50), (0x1D, 0x20), (0x1C, 0)]);
        for o in 0..3u32 {
            let v = ee::sub(fx.mem.f(r + 0x20 + 4 * o), fx.mem.f(sp + 0x90 + 4 * o));
            fx.mem.setf(sp + 0xB0 + 4 * o, v);
        }
        aim_angles(fx, sp + 0xB0, r + 0x40);
    }
}

/// Update 0x3C [F 0x1E04864]: five node trails (+0x1E0 + 0x50 k) of
/// eight pool slots each (+0x2968, 0x10 bytes, count 0x7D at the
/// attacker's node k), then the camera shake (+0x40).
fn trails_start(fx: &mut Fx, sp: u32) {
    for k in 0..5i32 {
        let r = D + 0x1E0 + 0x50 * k as u32;
        set_counter4(fx, r, 0x30, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        fx.mem.set8(r + 6, k);
        fx.mem.write_u32(r + 0x18, 0);
        fx.mem.write_u32(r + 0x2C, 0x3F33_3333);
        fx.mem.write_u32(r + 0x24, 0x42F0_0000);
        fx.mem.write_u32(r + 0x28, 0x3F00_0000);
        bytes(fx, r, &[(0x1E, 0x60), (0x1D, 0x60), (0x1C, 0x20), (0x1F, 0x60), (0x23, 0x60), (0x20, 0x60), (0x21, 0x60), (0x22, 0x60)]);
        fx.mem.write_u32(r + 8, D + 0x2968);
        fx.mem.write_u8(r + 0x14, 8);
        for j in 0..8u32 {
            let i = first_free(fx, D + 0x2968, 0x10, 0x3C);
            if i < 0 {
                break;
            }
            fx.mem.set8(r + 0xC + j, i);
            let s = D + 0x2968 + ((i as u32) << 4);
            set_counter(fx, s, 0x7D, 0);
            let h = fx.self_handle();
            fx.node_position(h, k, sp + 0x90);
            for o in 0..3u32 {
                let w = fx.mem.read_u32(sp + 0x90 + 4 * o);
                fx.mem.write_u32(s + 4 + 4 * o, w);
            }
        }
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x79, 0, 0xF);
    fx.mem.write_u8(s + 5, 1);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(s + 6, b);
    fx.mem.write_u8(s + 4, 4);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x4000_0000);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42F0_0000);
    fx.mem.write_u32(s + 0x50, 0x4080_0000);
}

/// Update 0x4A [F 0x1E04A14]: five flashes (+0x27EC + 0x4C k) on the
/// node requests +0xC + 4 k; the trails' flag 2 set.
fn flashes_start(fx: &mut Fx) {
    for k in 0..5u32 {
        let g = D + 0x27EC + 0x4C * k;
        set_counter4(fx, g, 0x6E, 0, 0xFF);
        fx.mem.write_u8(g + 5, 1);
        let b = fx.mem.read_u8(D + 0xC + 4 * k);
        fx.mem.write_u8(g + 6, b);
        fx.mem.write_u8(g + 4, 0x85);
        fx.mem.write_u8(g + 7, 9);
        fx.mem.write_u32(g + 0x10, 0x4396_0000);
        fx.mem.write_u32(g + 0x14, 0);
        fx.mem.write_u32(g + 0x18, 0x41F0_0000);
        bytes(fx, g, &[(0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0xC0), (9, 0x80), (0xA, 0x80), (8, 0x10)]);
        fx.mem.write_u32(g + 0x40, 0);
        fx.mem.write_u32(g + 0x44, 0);
        let r = D + 0x1E4 + 0x50 * k;
        let v = fx.mem.read_u8(r) | 2;
        fx.mem.write_u8(r, v);
    }
}

/// Update 0xB5 [F 0x1E04AF4]: five glows (+0xA0 + 0x40 k) at the node
/// requests +0xC + 4 k flying to the target in 32 updates, each with a
/// bead trail (+0x1B80 + 0x8C k); the fourth also restarts the shake
/// there.
fn glows_start(fx: &mut Fx, sp: u32) {
    for k in 0..5u32 {
        let r = D + 0xA0 + 0x40 * k;
        set_counter4(fx, r, 0x20, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u32(r + 8, 0);
        fx.mem.write_u32(r + 0x14, 0x4396_0000);
        fx.mem.write_u32(r + 0x18, 0);
        bytes(fx, r, &[(0xE, 0x60), (0xD, 0x60), (0xC, 0x20), (0xF, 0x60), (0x13, 0x80), (0x10, 0x80), (0x11, 0x80), (0x12, 0x80)]);
        let h = fx.self_handle();
        let slot = fx.mem.i32(D + 0xC + 4 * k);
        fx.node_position(h, slot, r + 0x20);
        for o in 0..3u32 {
            let v = ee::div(ee::sub(fx.mem.f(sp + 0xA0 + 4 * o), fx.mem.f(r + 0x20 + 4 * o)), 32.0);
            fx.mem.setf(r + 0x30 + 4 * o, v);
        }
        let t = D + 0x1B80 + 0x8C * k;
        set_counter4(fx, t, 0x20, 0, 0xFF);
        fx.mem.write_u8(t + 4, 1);
        fx.mem.write_u8(t + 5, 0);
        fx.mem.write_u32(t + 8, D + 0x370);
        bytes(fx, t, &[(0x10, 0x46), (0x12, 0x10), (0x11, 0xC), (0x13, 0), (0x15, 1)]);
        fx.mem.write_u32(t + 0x18, 0x3FC0_0000);
        fx.mem.write_u32(t + 0x1C, 0x3F99_999A);
        fx.mem.write_u32(t + 0x20, 0x41A0_0000);
        fx.mem.write_u32(t + 0x24, 0);
        fx.mem.write_u32(t + 0xC, D + 0x2D28);
        bytes(fx, t, &[(0x29, 0x80), (0x2A, 0x80), (0x28, 0x40), (0x2B, 0xC0)]);
        copy_v4(fx, r + 0x20, t + 0x4C);
        copy_v4(fx, t + 0x4C, t + 0x3C);
        copy_v4(fx, t + 0x3C, t + 0x2C);
        copy_v4(fx, r + 0x30, t + 0x5C);
        if k == 3 {
            let s = D + 0x40;
            set_counter4(fx, s, 0x30, 0, 7);
            fx.mem.write_u8(s + 5, 0);
            fx.mem.write_u8(s + 4, 2);
            fx.mem.write_u32(s + 0x34, 0x42F0_0000);
            fx.mem.write_u32(s + 0x50, 0x4080_0000);
            copy_v4(fx, r + 0x20, D + 0x60);
            copy_v4(fx, r + 0x30, D + 0x80);
        }
    }
}

/// Update 0xCC [F 0x1E04DA4]: two flashes (+0x27EC + 0x4C k, 600 (1 +
/// k)) and thirteen bars (+0x23DC + 0x50 k: the first 5000 x 60, the
/// others 1000 +- 450 at the random turn +- 200 plus 15 k) at the target.
fn bars_start(fx: &mut Fx, sp: u32) {
    for k in 0..2i32 {
        let g = D + 0x27EC + 0x4C * k as u32;
        set_counter4(fx, g, 0x1C, 0, 0xFF);
        fx.mem.write_u8(g + 5, 0);
        fx.mem.write_u8(g + 4, 0x81);
        fx.mem.write_u8(g + 7, 9);
        fx.mem.setf(g + 0x10, ee::add(600.0, ee::mul(600.0, ee::from_i32(k))));
        fx.mem.write_u32(g + 0x14, 0);
        bytes(fx, g, &[(9, 0x80), (0xA, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (8, 0x40), (0xF, 0x90)]);
        fx.mem.write_u32(g + 0x44, 0);
        fx.mem.write_u32(g + 0x40, 0);
        fx.mem.write_u32(g + 0x3C, 0);
        copy_v4(fx, sp + 0xA0, g + 0x1C);
    }
    let x = fx.rand_from(D, 0x01E0_4E8C) >> 4;
    let (_, h) = fx.div(x, 0x9C40);
    let turn = ee::div(ee::from_i32(h - 0x4E20), 100.0);
    for k in 0..13i32 {
        let r = D + 0x23DC + 0x50 * k as u32;
        set_counter4(fx, r, 0x14, 4, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u32(r + 0x48, 0);
        fx.mem.write_u32(r + 0x44, 0);
        fx.mem.write_u32(r + 0x40, 0);
        copy_v4(fx, sp + 0xA0, r + 0x20);
        if k == 0 {
            fx.mem.write_u8(r + 4, 0x81);
            fx.mem.write_u32(r + 0x10, 0x459C_4000);
            fx.mem.write_u32(r + 0x14, 0x4270_0000);
            bytes(fx, r, &[(0xD, 0x40), (0xE, 0x40), (0xC, 0x80), (9, 0x10), (0xA, 0x10), (8, 0x40), (0xF, 0x60)]);
            fx.mem.write_u32(r + 0x1C, 0);
            fx.mem.write_u32(r + 0x18, 0);
        } else {
            fx.mem.write_u8(r + 4, 0x80);
            let x = fx.rand_from(D, 0x01E0_4FA8) >> 4;
            let (_, h) = fx.div(x, 0x1_5F90);
            let f = ee::div(ee::from_i32(h - 0x8000 - 0x2FC8), 100.0);
            fx.mem.setf(r + 0x10, ee::add(1000.0, f));
            fx.mem.write_u32(r + 0x14, 0x41F0_0000);
            bytes(fx, r, &[(0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (9, 0x40), (0xA, 0x40), (8, 0), (0xF, 0x40)]);
            fx.mem.setf(r + 0x18, ee::add(turn, ee::mul(15.0, ee::from_i32(k))));
            fx.mem.write_u32(r + 0x1C, 0x3F80_0000);
        }
    }
}

/// Update 0xD0 [F 0x1E05074]: eight bead beams in free slots of the pool
/// +0x370 (0x58 bytes, 0x46), counts 20..23 after 0..7, at the target.
fn beads_start(fx: &mut Fx, sp: u32) {
    for k in 0..8i32 {
        let i = first_free(fx, D + 0x370, 0x58, 0x46);
        if i < 0 {
            continue;
        }
        let c = ((fx.rand_from(D, 0x01E0_50A8) >> 4) & 3) + 0x14;
        let delay = (fx.rand_from(D, 0x01E0_50C0) >> 4) & 7;
        let b = D + 0x370 + 0x58 * i as u32;
        set_counter4(fx, b, c & 0xFF, delay, 0xFF);
        fx.mem.write_u8(b + 4, 1);
        fx.mem.write_u8(b + 5, 0);
        fx.mem.write_u8(b + 8, 0x32);
        let x = fx.rand_from(D, 0x01E0_5114) >> 4;
        fx.mem.set8(b + 9, x);
        fx.mem.write_u8(b + 0xA, 0);
        fx.mem.setf(b + 0x1C, ee::add(20.0, ee::mul(4.0, ee::from_i32(k))));
        fx.mem.write_u32(b + 0x20, 0);
        fx.mem.write_u32(b + 0x10, D + 0x2D28);
        fx.mem.write_u32(b + 0x14, 0x4020_0000);
        bytes(fx, b, &[(0x26, 0x80), (0x25, 0x80), (0x24, 0x40), (0x27, 0x80)]);
        copy_v4(fx, sp + 0xA0, b + 0x28);
    }
}

/// 0x1A2C70 of `v` from the `jal` before `ra` (its frame left), as u16.
fn fix16(fx: &mut Fx, v: f32, ra: u32) -> u16 {
    fx.fix_residue(v, ra);
    ee::fixunssfsi(v) as u16
}

/// The projected point's billboard of half size `h` in four quadrant
/// quads (`xyz` the four vertices' XYZ2 slots, `ras` the conversions'
/// return addresses: x / y per quadrant, then the two keys), submitted
/// keyed by `w` (twice when `twice`).
#[allow(clippy::too_many_arguments)]
fn quadrants(fx: &mut Fx, d: u32, p: u32, xyz: [u32; 4], h: f32, w: f32, twice: bool, ot: u32, ras: [u32; 10]) {
    let z = fx.xyz_z(p);
    for a in xyz {
        fx.set_xyz_z(a, z);
    }
    let [v0, v1, v2, v3] = xyz;
    for q in 0..4usize {
        let cx = fx.mem.read_u16(p);
        let x = ee::from_i32(i32::from(cx));
        let vx = if q & 1 == 0 { ee::sub(x, h) } else { ee::add(x, h) };
        let vx = fix16(fx, vx, ras[2 * q]);
        fx.mem.write_u16(v2, vx);
        fx.mem.write_u16(v0, vx);
        if q == 0 {
            fx.mem.write_u16(v3, cx);
            fx.mem.write_u16(v1, cx);
        }
        let cy = fx.mem.read_u16(p + 2);
        let y = ee::from_i32(i32::from(cy));
        let hy = ee::mul(f32::from_bits(ASPECT), h);
        let vy = if q < 2 { ee::sub(y, hy) } else { ee::add(y, hy) };
        let vy = fix16(fx, vy, ras[2 * q + 1]);
        fx.mem.write_u16(v1 + 2, vy);
        fx.mem.write_u16(v0 + 2, vy);
        if q == 0 {
            fx.mem.write_u16(v3 + 2, cy);
            fx.mem.write_u16(v2 + 2, cy);
        }
        fx.fix_residue(w, ras[8]);
        let key = ee::fixunssfsi(w) as i32;
        fx.submit(d, ot, key);
        if twice {
            fx.fix_residue(w, ras[9]);
            let key = ee::fixunssfsi(w) as i32;
            fx.submit(d, ot, key);
        }
    }
}

/// The local 0x1E00410 (b47e65f71261, 504 words) [F]: `glows(recs, ot,
/// n) -> 1` steps and draws `n` records (0x40 bytes; a 0x1F0 frame, the
/// descriptor at sp + 0x158: RGBAQ / ST / XYZ2 x 4). The texture of the
/// first record's spec (+8 into the table 0x1E05480). Per record: countdown
/// 0x101AF0, only specs +8 < 2; position +0x20 += +0x30, size +0x14 +=
/// +0x18, then from a node with +5 (0x101BA0); the camera x the position
/// (0x20E2B0 / 0x20C0B0 / 0x20B370); colours +0xE.. (three corners) /
/// +0x12.. (the centre), UVs from the spec (0x101EE0); alpha count << 4
/// over the last 8 (without flag 0x40), else 0x80; the origin projected
/// (0x204D80, w sp + 0x1EC) on screen (0x101600); half size +0x14 / (w /
/// 256) (x +0x1C with flag 1 on odd counts); flag 0x80: far Z, key 0;
/// four quadrant quads (centre corner shared), twice with flag 2.
fn glows(fx: &mut Fx, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x1F0);
    fx.mem.set32(sp + 0xBC, n);
    let spec = GLOW_SPECS + 0x18 * fx.mem.i32(recs + 8) as u32;
    let (kind, id) = (fx.mem.i32(spec), fx.mem.u8(spec + 4));
    let tex = fx.texture(kind, id);
    fx.mem.write_u32(sp + 0xB0, tex);
    let mut g = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x40;
        }
        glow(fx, sp, g, ot, tex);
        i += 1;
    }
    fx.leave(0x1F0);
    1
}

fn colours(fx: &mut Fx, g: u32, src: [u32; 2], v: [u32; 4]) {
    for k in 0..3u32 {
        let c = fx.mem.read_u8(g + src[0] - k);
        for &a in &v[..3] {
            fx.mem.write_u8(a + k, c);
        }
        let c = fx.mem.read_u8(g + src[1] - k);
        fx.mem.write_u8(v[3] + k, c);
    }
    for a in v {
        fx.mem.write_u32(a + 4, 0x3F80_0000);
    }
}

fn glow(fx: &mut Fx, sp: u32, g: u32, ot: u32, tex: u32) {
    if countdown(fx, g) == 0 || fx.mem.i32(g + 8) >= 2 {
        return;
    }
    for o in [0x20u32, 0x24, 0x28] {
        let v = ee::add(fx.mem.f(g + o), fx.mem.f(g + o + 0x10));
        fx.mem.setf(g + o, v);
    }
    let v = ee::add(fx.mem.f(g + 0x14), fx.mem.f(g + 0x18));
    fx.mem.setf(g + 0x14, v);
    node_source(fx, g, g + 0x20);
    camera_vp_into(fx, sp + 0x100);
    translation_into(fx, sp + 0xC0, g + 0x20);
    mul_mem(fx, sp + 0x100, sp + 0xC0);
    for o in [0x148u32, 0x144, 0x140] {
        fx.mem.write_u32(sp + o, 0);
    }
    let d = sp + 0x158;
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    let rgba = [sp + 0x168, sp + 0x180, sp + 0x198, sp + 0x1B0];
    colours(fx, g, [0xE, 0x12], rgba);
    let spec = GLOW_SPECS + 0x18 * fx.mem.i32(g + 8) as u32;
    tex_rect_strip(fx, d, spec, tex);
    let c = fx.mem.u8(g);
    let a = if c < 8 && fx.mem.u8(g + 4) & 0x40 == 0 { (c << 4) as u8 } else { 0x80 };
    for v in rgba {
        fx.mem.write_u8(v + 3, a);
    }
    let m = fx.read_m4(sp + 0x100);
    if !fx.project_at(sp + 0x140, &m, sp + 0x150, sp + 0x1EC) || !Fx::on_screen(fx.mem.read_u64(sp + 0x150)) {
        return;
    }
    let mut w = fx.mem.f(sp + 0x1EC);
    let mut h = ee::div(fx.mem.f(g + 0x14), ee::div(w, 256.0));
    if fx.mem.u8(g + 4) & 0x80 != 0 {
        fx.set_xyz_z(sp + 0x150, 0xFF_FFFE);
        w = 0.0;
    }
    if fx.mem.u8(g + 4) & 1 != 0 && fx.mem.u8(g) & 1 != 0 {
        h = ee::mul(h, fx.mem.f(g + 0x1C));
    }
    let twice = fx.mem.u8(g + 4) & 2 != 0;
    const RAS: [u32; 10] = [0x01E0_087C, 0x01E0_08E8, 0x01E0_0948, 0x01E0_09AC, 0x01E0_0A04, 0x01E0_0A68, 0x01E0_0AC0, 0x01E0_0B24, 0x01E0_0B3C, 0x01E0_0B6C];
    quadrants(fx, d, sp + 0x150, [sp + 0x178, sp + 0x190, sp + 0x1A8, sp + 0x1C0], h, w, twice, ot, RAS);
}

/// The local 0x1E00BF0 (ee4ab061226e, 572 words) [F]: `trails(recs, ot,
/// n) -> 1` steps and draws `n` records (0x50 bytes; a 0x1D0 frame, the
/// descriptor at sp + 0x138) of +0x14 pool slots each (indices +0xC.., the
/// pool the first record's +8, 0x10-byte slots: count, position +4).
/// The texture of the first record's spec (+0x18 into the table 0x1E054B0).
/// Per record: countdown 0x101AF0, only specs +0x18 < 2; position +0x30 +=
/// +0x40, then from a node with +5; the camera (sp + 0xE0); the record's
/// count cleared. Per slot k: slot 0's count / length copied into the
/// record, the record's into the others; countdown 0x101B50; size +0x24 +
/// +0x28 x elapsed; slot +0x15 (the head) takes the record's position;
/// colours +0x1E.. / +0x22.., UVs (0x101EE0); with the record's count odd
/// and flag 2 skipped; alpha +0x23 (x count / 8 over the last 8 without
/// flag 0x40); projected (sp + 0x130, w sp + 0x1CC) on screen; half size
/// / (w / 256) (x +0x2C with flag 1 on odd counts); flag 0x80 far Z, key
/// 0; four quadrant quads, twice with flag 4. Then the head steps back
/// (wrapping to +0x14 - 1).
fn trails(fx: &mut Fx, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x1D0);
    fx.mem.set32(sp + 0xDC, n);
    let pool = fx.mem.read_u32(recs + 8);
    fx.mem.write_u32(sp + 0xC0, pool);
    let spec = TRAIL_SPECS + 0x18 * fx.mem.i32(recs + 0x18) as u32;
    let (kind, id) = (fx.mem.i32(spec), fx.mem.u8(spec + 4));
    let tex = fx.texture(kind, id);
    fx.mem.write_u32(sp + 0xD0, tex);
    let mut g = recs;
    fx.mem.set32(sp + 0xB0, 0);
    while fx.mem.i32(sp + 0xB0) < fx.mem.i32(sp + 0xDC) {
        if fx.mem.i32(sp + 0xB0) != 0 {
            g += 0x50;
        }
        trail(fx, sp, g, pool, ot, tex);
        let v = fx.mem.i32(sp + 0xB0) + 1;
        fx.mem.set32(sp + 0xB0, v);
    }
    fx.leave(0x1D0);
    1
}

fn trail(fx: &mut Fx, sp: u32, g: u32, pool: u32, ot: u32, tex: u32) {
    if countdown(fx, g) == 0 || fx.mem.i32(g + 0x18) >= 2 {
        return;
    }
    for o in [0x30u32, 0x34, 0x38] {
        let v = ee::add(fx.mem.f(g + o), fx.mem.f(g + o + 0x10));
        fx.mem.setf(g + o, v);
    }
    node_source(fx, g, g + 0x30);
    camera_vp_into(fx, sp + 0xE0);
    fx.mem.write_u8(g, 0);
    let mut k = 0;
    while k < fx.mem.u8(g + 0x14) {
        trail_slot(fx, sp, g, pool, ot, tex, k);
        k += 1;
    }
    let head = fx.mem.u8(g + 0x15);
    if head == 0 {
        let v = fx.mem.u8(g + 0x14) - 1;
        fx.mem.set8(g + 0x15, v);
    } else {
        fx.mem.set8(g + 0x15, head - 1);
    }
}

fn trail_slot(fx: &mut Fx, sp: u32, g: u32, pool: u32, ot: u32, tex: u32, k: i32) {
    let i = fx.mem.u8(g + 0xC + k as u32) as u32;
    let s = pool.wrapping_add(i << 4);
    if k == 0 {
        let v = fx.mem.read_u8(s);
        fx.mem.write_u8(g, v);
        let v = fx.mem.read_u8(s + 2);
        fx.mem.write_u8(g + 2, v);
    } else {
        let v = fx.mem.read_u8(g);
        fx.mem.write_u8(s, v);
        let v = fx.mem.read_u8(g + 2);
        fx.mem.write_u8(s + 2, v);
    }
    if countdown(fx, s) == 0 {
        return;
    }
    let e = fx.mem.u8(s + 2) - fx.mem.u8(s);
    let mut h = ee::add(fx.mem.f(g + 0x24), ee::mul(fx.mem.f(g + 0x28), ee::from_i32(e)));
    if k == fx.mem.u8(g + 0x15) {
        for o in 0..3u32 {
            let v = fx.mem.read_u32(g + 0x30 + 4 * o);
            fx.mem.write_u32(s + 4 + 4 * o, v);
        }
    }
    for o in 0..3u32 {
        let v = fx.mem.read_u32(s + 4 + 4 * o);
        fx.mem.write_u32(sp + 0x120 + 4 * o, v);
    }
    let d = sp + 0x138;
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    let rgba = [sp + 0x148, sp + 0x160, sp + 0x178, sp + 0x190];
    colours(fx, g, [0x1E, 0x22], rgba);
    let spec = TRAIL_SPECS + 0x18 * fx.mem.i32(g + 0x18) as u32;
    tex_rect_strip(fx, d, spec, tex);
    if fx.mem.u8(g) & 1 != 0 && fx.mem.u8(g + 4) & 2 != 0 {
        return;
    }
    let c = fx.mem.u8(s);
    let a = if c < 8 && fx.mem.u8(g + 4) & 0x40 == 0 {
        let v = fx.mem.u8(g + 0x23) >> 3;
        fx.mult(v, c)
    } else {
        fx.mem.u8(g + 0x23)
    };
    for v in [0x193u32, 0x17B, 0x163, 0x14B] {
        fx.mem.set8(sp + v, a);
    }
    let m = fx.read_m4(sp + 0xE0);
    if !fx.project_at(sp + 0x120, &m, sp + 0x130, sp + 0x1CC) || !Fx::on_screen(fx.mem.read_u64(sp + 0x130)) {
        return;
    }
    let mut w = fx.mem.f(sp + 0x1CC);
    h = ee::div(h, ee::div(w, 256.0));
    if fx.mem.u8(g + 4) & 0x80 != 0 {
        fx.set_xyz_z(sp + 0x130, 0xFF_FFFE);
        w = 0.0;
    }
    if fx.mem.u8(g + 4) & 1 != 0 && fx.mem.u8(s) & 1 != 0 {
        h = ee::mul(h, fx.mem.f(g + 0x2C));
    }
    let twice = fx.mem.u8(g + 4) & 4 != 0;
    const RAS: [u32; 10] = [0x01E0_1114, 0x01E0_1180, 0x01E0_11E0, 0x01E0_1244, 0x01E0_129C, 0x01E0_1300, 0x01E0_1358, 0x01E0_13BC, 0x01E0_13D4, 0x01E0_1404];
    quadrants(fx, d, sp + 0x130, [sp + 0x158, sp + 0x170, sp + 0x188, sp + 0x1A0], h, w, twice, ot, RAS);
}
