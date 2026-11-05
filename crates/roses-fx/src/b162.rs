//! BMODULE b162 (card 238), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04140, data area a0 = 0x1ED0000) [F] (helper B6, 31
//! Oct 2026): the "a0 = D" family (s0 = D, s2 = the OT). Its own node
//! requests 0x1E03FC0 (eight at the launch, five again on update 63). On
//! update 2 four arc pairs between node pairs (+0x112C); on 64 two more;
//! on 67 at the target four bead beams (+0x12AC), 18 dashes thrown up in
//! two fans (+0x5EC, `dash::dashes`), a flash (+0xA0), four crossing bars
//! (+0xEC) and a camera shake; on 90 24 dashes in four turning fans, 13
//! bars (the first 5000 long), the flash again, eight bead beams and the
//! shake again. Returns 1 on updates 58 and 82, 6 on 65, 7 on 81, 2 on 89,
//! 8 on 130 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `beads`
//! 0x1E00410, `bars` 0x1E011E0, `flash` 0x1E01BF0, `dashes` 0x1E027A0,
//! `arcs` 0x1E02D40 [`crate::blib`].
use crate::b076::{copy_v4, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x218C;
const ARCS: u32 = D + 0x112C;
const BEADS: u32 = D + 0x12AC;
const DASHES: u32 = D + 0x5EC;
const BARS: u32 = D + 0xEC;

/// The entry 0x1E04140 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp)
    };
    fx.leave(0x80);
    ret
}

/// 0x1E03FC0(D, kind) [F]: the attacker's node requests, kind 0 five
/// (0x28, 0x27, 0x30, 0x2F, 1), kind 1 eight (0x35, 0x34, 0x3A, 0x39,
/// 0x3F, 0x3E, 0x44, 0x43), into D+0xC..; the first clears. Leaves its
/// frame (0x20: the caller's s0 = D +0, `ra` +0x10).
fn nodes(fx: &mut Fx, kind: i32, ra: u32) {
    let f = fx.sp - 0x20;
    fx.saved(f, &[(0, u64::from(D)), (0x10, u64::from(ra))]);
    fx.sp = f;
    let list: &[i32] = match kind {
        1 => &[0x35, 0x34, 0x3A, 0x39, 0x3F, 0x3E, 0x44, 0x43],
        0 => &[0x28, 0x27, 0x30, 0x2F, 1],
        _ => &[],
    };
    for (k, &node) in list.iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set32(D + 0xC + 4 * k as u32, r);
    }
    fx.sp = f + 0x20;
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x2290);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    fx.texture(4, 2);
    nodes(fx, 1, 0x01E0_4230);
    // The entry's s0 = D, s1 = the parameter, s2 = the OT.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_4240, [Some(D), Some(param as u32), Some(OT)]);
    0
}

/// An arc pair `g` between nodes +0xC + 8 k / +0x10 + 8 k with count
/// `count`, +0x30 / +0x31 `b30` / `points`, +0x34 / +0x36 `b34` / `h36`.
fn arc_pair(fx: &mut Fx, k: u32, count: i32, b30: u8, points: u8, b34: u8, h36: u16) -> u32 {
    let g = ARCS + 0x60 * k;
    set_counter4(fx, g, count, 0, 0xFF);
    fx.mem.write_u8(g + 4, 1);
    fx.mem.write_u32(g + 8, D + 0x182C);
    fx.mem.write_u8(g + 0x30, b30);
    fx.mem.write_u8(g + 0x31, points);
    let b = fx.mem.read_u8(D + 0xC + 8 * k);
    fx.mem.write_u8(g + 0x32, b);
    let b = fx.mem.read_u8(D + 0x10 + 8 * k);
    fx.mem.write_u8(g + 0x33, b);
    fx.mem.write_u32(g + 0x38, 0x3F80_0000);
    fx.mem.write_u8(g + 0x34, b34);
    fx.mem.write_u16(g + 0x36, h36);
    g
}

fn arc_ends(fx: &mut Fx, g: u32) {
    let mut a = 0u32;
    while a < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0x1E + 2 * a, -1);
        fx.mem.set16(g + 0xC + 2 * a, -1);
        a += 1;
    }
}

/// Update 2 [F 0x1E042BC]: four arc pairs.
fn arcs4(fx: &mut Fx) {
    for k in 0..4u32 {
        let g = arc_pair(fx, k, 0x36, 0x96, 5, 1, 0x80);
        for (o, v) in [(0x44u32, 0xC0u8), (0x45, 0xC0), (0x46, 0xC0), (0x47, 0x60), (0x4A, 0x10), (0x49, 0x10), (0x48, 0x10)] {
            fx.mem.write_u8(g + o, v);
        }
        arc_ends(fx, g);
    }
}

/// Update 64 [F 0x1E043DC]: two arc pairs.
fn arcs2(fx: &mut Fx) {
    for k in 0..2u32 {
        let g = arc_pair(fx, k, 0x28, 0x46, 8, 0, 0);
        for (o, v) in [(0x47u32, 0xA0u8), (0x44, 0xA0), (0x45, 0xA0), (0x46, 0xA0), (0x4A, 0x80), (0x48, 0), (0x49, 0)] {
            fx.mem.write_u8(g + o, v);
        }
        arc_ends(fx, g);
    }
}

/// The flash +0xA0 at the target (sp + 0x70), alpha `a`.
fn flash(fx: &mut Fx, sp: u32, a: u8) {
    let r = D + 0xA0;
    set_counter4(fx, r, 0x12, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u8(r + 7, 9);
    fx.mem.write_u32(r + 0x10, 0x442F_0000);
    fx.mem.write_u32(r + 0x14, 0);
    for (o, v) in [(0xAu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (8, 0x20), (9, 0x20), (0xF, a)] {
        fx.mem.write_u8(r + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(r + o, 0);
    }
    copy_v4(fx, sp + 0x70, r + 0x1C);
}

/// A dash's common bytes: count `count`, +4 `b4`, width `w`, colour.
fn dash(fx: &mut Fx, r: u32, count: i32, b4: u8, w: u32) {
    set_counter4(fx, r, count, 0, 0xFF);
    fx.mem.write_u8(r + 4, b4);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u32(r + 8, w);
    for (o, v) in [(0xEu32, 0xC0u8), (0xD, 0x20), (0xC, 0x20), (0xF, 0xF0)] {
        fx.mem.write_u8(r + o, v);
    }
}

/// ((rand >> 4) % m - h) / 100 (rand's s0 = D).
fn jitter(fx: &mut Fx, ra: u32, m: i32, h: i32) -> f32 {
    let x = fx.rand_from(D, ra) >> 4;
    let (_, r) = fx.div(x, m);
    ee::div(ee::from_i32(r - h), 100.0)
}

/// Update 67 [F 0x1E044E4]: beads, dashes, the flash, bars and the shake
/// at the target (sp + 0x70).
fn strike(fx: &mut Fx, sp: u32) {
    for k in 0..4i32 {
        let count = (((fx.rand_from(D, 0x01E0_44F8) >> 4) & 7) + 0x14) & 0xFF;
        let delay = (fx.rand_from(D, 0x01E0_4510) >> 4) & 3;
        let b = BEADS + 0x58 * k as u32;
        set_counter4(fx, b, count, delay, 0xFF);
        fx.mem.setf(b + 0x1C, ee::add(18.0, ee::from_i32((k >> 1) * 3)));
        fx.mem.write_u32(b + 0x14, 0x3FA6_6666);
        fx.mem.write_u32(b + 0x18, 0x3E99_999A);
        for (o, v) in [(0xAu32, 0u8), (8, 0x28), (0x26, 0x80), (0x25, 0x20), (0x24, 0x20), (0x27, 0xF0), (4, 0), (5, 0)] {
            fx.mem.write_u8(b + o, v);
        }
        let x = fx.rand_from(D, 0x01E0_45C4);
        fx.mem.set8(b + 9, x >> 4);
        fx.mem.write_u32(b + 0x10, BYTES);
        copy_v4(fx, sp + 0x70, b + 0x28);
        fx.mem.set32(b + 0x50, if k & 1 != 0 { 0x4000 } else { -0x4000 });
    }
    for f in 0..2u32 {
        for k in 0..9u32 {
            let r = DASHES + 0x50 * (k + 9 * f);
            dash(fx, r, 0x10, 2, 0x41A0_0000);
            let v = ee::add(f32::from_bits(0x3FE6_6666), jitter(fx, 0x01E0_46BC, 0x64, 0x32));
            fx.mem.setf(r + 0x20, v);
            if f != 0 {
                let v = ee::mul(fx.mem.f(r + 0x20), -1.0);
                fx.mem.setf(r + 0x20, v);
            }
            fx.mem.write_u32(r + 0x28, 0);
            fx.mem.write_u32(r + 0x24, 0);
            copy_v4(fx, sp + 0x70, r + 0x10);
            let dy = ee::sub(2.25, ee::mul(0.5, ee::from_i32(k as i32)));
            let y = ee::add(fx.mem.f(r + 0x14), dy);
            fx.mem.setf(r + 0x14, y);
        }
    }
    flash(fx, sp, 0x60);
    for k in 0..4i32 {
        let c = BARS + 0x50 * k as u32;
        set_counter4(fx, c, 0x12, 0, 0xFF);
        fx.mem.write_u8(c + 5, 0);
        fx.mem.write_u8(c + 4, 0x80);
        fx.mem.write_u32(c + 0x10, if k == 2 { 0x4496_0000 } else { 0x4416_0000 });
        fx.mem.write_u32(c + 0x14, 0x4220_0000);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x40), (0xA, 0x80), (9, 0x10), (8, 0x10)] {
            fx.mem.write_u8(c + o, v);
        }
        fx.mem.setf(c + 0x18, ee::mul(45.0, ee::from_i32(k)));
        fx.mem.write_u32(c + 0x1C, 0);
        copy_v4(fx, sp + 0x70, c + 0x20);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x10, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x4080_0000);
    copy_v4(fx, sp + 0x70, D + 0x60);
}

/// Update 90 [F 0x1E049B4]: four turning fans of six dashes, 13 bars, the
/// flash, eight beads and the shake.
fn burst(fx: &mut Fx, sp: u32) {
    for f in 0..4u32 {
        for k in 0..6u32 {
            let r = DASHES + 0x50 * (k + 6 * f);
            dash(fx, r, 0x14, 1, 0x41F0_0000);
            let v = ee::add(2.0, jitter(fx, 0x01E0_4A40, 0x96, 0x4B));
            fx.mem.setf(r + 0x20, v);
            fx.mem.write_u32(r + 0x28, 0);
            fx.mem.write_u32(r + 0x24, 0);
            copy_v4(fx, sp + 0x70, r + 0x10);
            let base = [0x2000, 0x5000, 0xA000, 0xD000][f as usize];
            fx.mem.set32(r + 0x38, 910 * k as i32 + base);
        }
    }
    let spin = jitter(fx, 0x01E0_4BAC, 0x9C40, 0x4E20);
    for k in 0..0xDi32 {
        let c = BARS + 0x50 * k as u32;
        set_counter4(fx, c, 0x14, 4, 0xFF);
        fx.mem.write_u8(c + 5, 0);
        for o in [0x48u32, 0x44, 0x40] {
            fx.mem.write_u32(c + o, 0);
        }
        copy_v4(fx, sp + 0x70, c + 0x20);
        if k == 0 {
            fx.mem.write_u8(c + 4, 0x81);
            fx.mem.write_u32(c + 0x10, 0x459C_4000);
            fx.mem.write_u32(c + 0x14, 0x4248_0000);
            for (o, v) in [(0xDu32, 0x40u8), (0xE, 0x40), (0xC, 0x80), (9, 0x10), (0xA, 0x10), (8, 0x40), (0xF, 0x40)] {
                fx.mem.write_u8(c + o, v);
            }
            fx.mem.write_u32(c + 0x1C, 0);
            fx.mem.write_u32(c + 0x18, 0);
        } else {
            fx.mem.write_u8(c + 4, 0x80);
            let v = ee::add(600.0, jitter(fx, 0x01E0_4CC4, 0x2710, 0x1388));
            fx.mem.setf(c + 0x10, v);
            fx.mem.write_u32(c + 0x14, 0x41F0_0000);
            for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (9, 0x40), (0xA, 0x40), (8, 0), (0xF, 0x60)] {
                fx.mem.write_u8(c + o, v);
            }
            fx.mem.setf(c + 0x18, ee::add(spin, ee::mul(15.0, ee::from_i32(k))));
            fx.mem.write_u32(c + 0x1C, 0x4000_0000);
        }
    }
    flash(fx, sp, 0xC0);
    for k in 0..8i32 {
        let i = first_free(fx, BEADS, 0x58, 0x10);
        if i < 0 {
            continue;
        }
        let count = (((fx.rand_from(D, 0x01E0_4E44) >> 4) & 3) + 0x14) & 0xFF;
        let delay = (fx.rand_from(D, 0x01E0_4E5C) >> 4) & 7;
        let b = BEADS + 0x58 * i as u32;
        set_counter4(fx, b, count, delay, 0xFF);
        fx.mem.write_u8(b + 4, 1);
        fx.mem.write_u8(b + 5, 0);
        fx.mem.write_u8(b + 8, 0x1E);
        let x = fx.rand_from(D, 0x01E0_4EB0);
        fx.mem.set8(b + 9, x >> 4);
        fx.mem.write_u8(b + 0xA, 0);
        fx.mem.setf(b + 0x1C, ee::add(18.0, ee::mul(1.5, ee::from_i32(k))));
        fx.mem.write_u32(b + 0x20, 0);
        fx.mem.write_u32(b + 0x10, BYTES);
        fx.mem.write_u32(b + 0x14, 0x3FC0_0000);
        for (o, v) in [(0x26u32, 0x80u8), (0x25, 0x20), (0x24, 0x20), (0x27, 0xF0)] {
            fx.mem.write_u8(b + o, v);
        }
        copy_v4(fx, sp + 0x70, b + 0x28);
    }
    set_counter4(fx, D + 0x40, 0x10, 0, 0xFF);
    fx.mem.write_u32(D + 0x90, 0x4100_0000);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x70, u64::from(D), 0x01E0_4268);
    match fx.mem.u16(D) {
        0x82 => return 8,
        0x5A => burst(fx, sp),
        0x43 => strike(fx, sp),
        0x40 => arcs2(fx),
        0x3F => nodes(fx, 0, 0x01E0_43D4),
        2 => arcs4(fx),
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_4F94, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    blib::cross::bars(fx, 0x01E0_11E0, BARS, OT, 0x10);
    blib::star::flash(fx, 0x01E0_1BF0, D + 0xA0, OT, 1);
    blib::arc::arcs(fx, 0x01E0_2D40, ARCS, OT, 4);
    blib::beam::beads(fx, 0x01E0_0410, BEADS, OT, 0x10);
    blib::dash::dashes(fx, 0x01E0_27A0, DASHES, OT, 0x24);
    match fx.mem.u16(D) {
        0x59 => 2,
        0x52 => 1,
        0x51 => 7,
        0x41 => 6,
        0x3A => 1,
        _ => 0,
    }
}
