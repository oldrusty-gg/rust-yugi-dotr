//! BMODULE b637 (card 667), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02A30, data area 0x1ED0000) [F] (helper C8, 31
//! Oct 2026): b147's program with the data moved up and a cloud of 60
//! spinning quads (0x1E01930, `motes`). Two swaths (+0x70 / +0x1A0,
//! `blib::swath`) between the attacker's nodes 0x61 / 0x5D and 0x30 /
//! 0x2D, drawn throughout (fading in, from update 0xA1 out); two discs at
//! the nodes 0x5F / 0x2F from update 0x2F; from 0x3F a full-window tint
//! (+0x1744), a disc (+0x640) at the target and 16 jagged strands (+0x2D0
//! + 0x30 k, `blib::strand`, ends +0x1620 + 0x10 k) from it, and a wave
//! (+0xEF0) at the target, all flashing back on the updates 0x3F, 0x5E,
//! 0x65, 0x72, 0x84, 0x90; the stage dims until 0xA0. The motes (+0x7C0:
//! positions +0xBB0 + 0xC k rising 2 / 3 a update and respawned at 100,
//! angles +0x7F0 + 0x10 k turning 0x1000) follow the attacker's unit
//! point (+0x750) throughout. Returns 1 on update 0x35, 6 on 0x3F..0x9F, 2
//! on 0xA0, 8 after 0xB2.
//!
//! Library at its own addresses: `colours4` (0x1E00080), the tint
//! (0x1E00120), `swath` push / draw (0x1E00280 / 0x1E003D0), `disc`
//! (0x1E00BD0), `wave::draw` (0x1E01130), `strand::draw` (0x1E020B0).
use crate::b147::{node, swath_init};
use crate::b186::{bytes, halves};
use crate::blib::{strand, swath, wave};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const STRAND: u32 = 0x01E0_20B0;
const N: u32 = D + 0x1724;
const LEFT: u32 = D + 0x1720;
const LIGHT: u32 = D + 0x174C;
const TABLE: u32 = D + 0x174D;
const ENDS: u32 = D + 0x1620;
const MOTES: u32 = D + 0x7C0;

/// The entry 0x1E02A30.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xC0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x1850);
    for x in [0u32, 0x5D0, 0x650, 0x6D0, 0x750] {
        fx.xform_init(D + x);
    }
    for (at, n) in [(0x172Cu32, 0x61), (0x1730, 0x5D), (0x1738, 0x30), (0x173C, 0x2D), (0x1734, 0x5F), (0x1740, 0x2F)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(at == 0x172C));
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(LEFT, 0xB2);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x1728, param);
    fx.mem.write_u8(LIGHT, 0x80);
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, 0x01E0_2B50);
        let (_, m) = fx.div(r, 0xFF);
        fx.mem.write_u8(TABLE + k, (m + 1) as u8);
    }
    bytes(fx, &[(0x1746, 0x90), (0x1745, 0x20), (0x1744, 0x60), (0x1747, 0x80), (0x174A, 0x80), (0x1749, 0x80), (0x1748, 0x80), (0x174B, 0x60)]);
    // Discs (+0x640, +0x6C0, +0x740).
    for (g, a, size) in [(0x640u32, 0xFFu8, 0x41A0_0000u32), (0x6C0, 0, 0x4120_0000), (0x740, 0, 0x4120_0000)] {
        fx.mem.write_u32(D + g, 0x8_0000);
        bytes(fx, &[(g + 6, 0xA0), (g + 5, 0x80), (g + 4, 0x80), (g + 7, a)]);
        fx.mem.write_u32(D + g + 8, size);
        halves(fx, &[(g + 0xC, 0x24), (g + 0xE, 0)]);
    }
    fx.texture5(sp + 0xB0, 0);
    // 16 strands: 16 pieces, jitter 0x14, drawn -(k + 1), random ends.
    for k in 0..0x10u32 {
        let g = 0x2D0 + 0x30 * k;
        let tex = fx.mem.read_u32(sp + 0xB0);
        fx.mem.write_u32(D + g, tex);
        fx.mem.write_u32(D + g + 4, 0);
        halves(fx, &[(g + 0x10, 0x80), (g + 0x12, 0x40), (g + 0x14, 0x20), (g + 0x16, 0x40)]);
        bytes(fx, &[(g + 0xA, 0x80), (g + 9, 0x20), (g + 8, 0x10), (g + 0xB, 0x80)]);
        halves(fx, &[(g + 0xC, 0x10), (g + 0x28, -(k as i32 + 1)), (g + 0xE, 0x14)]);
        fx.mem.write_u32(D + g + 0x20, 0);
        fx.mem.write_u32(D + g + 0x24, 0);
        fx.random_dir(ENDS + 0x10 * k, 20.0);
    }
    fx.texture5(sp + 0xB0, 1);
    // The motes: flags 0x40001001 (integer UVs 0, 0, 64, 128; blend 0x44;
    // all turned by +0x10 = (0, 0x400, 0)), half size 2.0, white.
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(MOTES, tex);
    fx.mem.write_u32(MOTES + 4, 0x4000_1001);
    fx.mem.write_u32(MOTES + 0xC, 0x4000_0000);
    halves(fx, &[(0x7E0, 0), (0x7E2, 0), (0x7E4, 0x40), (0x7E6, 0x80)]);
    bytes(fx, &[(0x7CA, 0x80), (0x7C9, 0x80), (0x7C8, 0x80), (0x7CB, 0x80)]);
    for (o, v) in [(0x10u32, 0u32), (0x14, 0x400), (0x18, 0)] {
        fx.mem.write_u32(MOTES + o, v);
    }
    for k in 0..0x3Cu32 {
        let p = D + 0xBB0 + 0xC * k;
        let a = fx.rand_from(D, 0x01E0_2D20) & 0xFFFF;
        let r = fx.rand_from(D, 0x01E0_2D3C);
        let (_, m) = fx.div(r, 1000);
        fx.mem.setf(p + 4, ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(m)));
        let c = fx.tcos(a);
        fx.mem.setf(p + 8, ee::mul(4.0, c));
        let s = fx.tsin(a);
        fx.mem.setf(p, ee::mul(4.0, s));
        for (o, ra) in [(0u32, 0x01E0_2DA0u32), (4, 0x01E0_2DBC), (8, 0x01E0_2DD8)] {
            let v = fx.rand_from(D, ra) & 0xFFFF;
            fx.mem.set32(MOTES + 0x30 + 0x10 * k + o, v);
        }
    }
    fx.texture4(sp + 0xB0, 0xF);
    swath_init(fx, sp, 0x70, (0xF20, 0x10E0), 0x172C, 0x1730, 0x60, [0x80, 0x28, 0x10, 0]);
    swath_init(fx, sp, 0x1A0, (0x12A0, 0x1460), 0x1738, 0x173C, 0x80, [0x80, 0x28, 0x10, 0]);
    // The wave (+0xEF0).
    let tex = fx.mem.read_u32(sp + 0xB0);
    fx.mem.write_u32(D + 0xEF0, tex);
    fx.mem.write_u32(D + 0xEFC, TABLE);
    fx.mem.write_u32(D + 0xEF4, 0x2000_0000);
    halves(fx, &[(0xF00, 0xB0), (0xF02, 0x40), (0xF04, 0x20), (0xF06, 0x20)]);
    bytes(fx, &[(0xEFA, 0x80), (0xEF9, 0x20), (0xEF8, 0x10), (0xEFB, 0xFF)]);
    fx.mem.write_u32(D + 0xF10, 0x3FB3_3333);
    fx.mem.write_u32(D + 0xF14, 0x3CF5_C28F);
    halves(fx, &[(0xF1A, 0), (0xF18, 0x100)]);
    fx.mem.write_u8(D + 0xF1C, 7);
    0
}

/// Before update 0xA1 up by `step` to 0x80, from it down to `floor`
/// (`late_up`: the other way round).
fn fade(fx: &mut Fx, p: u32, floor: i32, step: i32, late_up: bool) {
    let late = fx.mem.i32(N) >= 0xA1;
    if late == late_up {
        fx.step_up(p, 0x80, step);
    } else {
        fx.step_down(p, floor, step);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(N);
    node(fx, 0x172C, sp + 0x60);
    node(fx, 0x1730, sp + 0x70);
    swath::push(fx, D + 0x70, sp + 0x60, sp + 0x70);
    node(fx, 0x1738, sp + 0x80);
    node(fx, 0x173C, sp + 0x90);
    swath::push(fx, D + 0x1A0, sp + 0x80, sp + 0x90);
    for out in [0x5F0u32, 0xEA0] {
        let t = fx.target_handle();
        fx.unit_position(t, D + out);
    }
    node(fx, 0x1734, D + 0x6F0);
    node(fx, 0x1740, D + 0x670);
    let h = fx.self_handle();
    fx.unit_point(1, h, D + 0x770);
    fx.mem.write_u32(D + 0x774, 0);
    // s0 = D, s1 = the OT, s2 = sp + 0x90, s3 the runner's (0).
    let regs = [Some(D), Some(OT), Some(sp + 0x90), Some(0)];
    fx.xform_world_saving(D, 0x01E0_3150, regs);
    for (x, ra) in [(0x5D0u32, 0x01E0_3158u32), (0xE80, 0x01E0_3160), (0x650, 0x01E0_3168), (0x6D0, 0x01E0_3170)] {
        fx.xform_billboard_saving(D + x, ra, regs);
    }
    fx.xform_world_saving(D + 0x750, 0x01E0_3178, regs);
    if n(fx) >= 0xA1 {
        fx.step_up(LIGHT, 0x80, 4);
    } else {
        fx.step_down(LIGHT, 0x20, 4);
    }
    let l = fx.mem.read_u8(LIGHT);
    fx.set_stage_light(i32::from(l));
    let m = n(fx);
    if m >= 0x3F {
        if matches!(m, 0x3F | 0x5E | 0x65 | 0x72 | 0x84 | 0x90) {
            fx.mem.write_u8(D + 0x1747, 0xFF);
            fx.mem.set16(D + 0xF1A, 0);
            fx.mem.write_u8(D + 0xEFB, 0xFF);
            fx.mem.write_u8(D + 0x647, 0xFF);
            for k in 0..0x10u32 {
                fx.mem.write_u8(D + 0x2DB + 0x30 * k, 0x80);
            }
        }
        crate::b262::tint(fx, D + 0x1744);
        fx.step_down(D + 0x1747, 0, 0x20);
        fx.disc(D + 0x640, D + 0x5D0);
        fx.step_down(D + 0x647, 0, 0x10);
        fx.approach(D + 0xE80, f32::from_bits(0x3E99_999A), f32::from_bits(0x3DCC_CCCD));
        wave::draw(fx, OT, D + 0xEF0, D + 0xE80);
        fx.step_down(D + 0xEFB, 0, 0x10);
        for o in [0xA8u32, 0xA4, 0xA0] {
            fx.mem.write_u32(sp + o, 0);
        }
        for k in 0..0x10u32 {
            let g = D + 0x2D0 + 0x30 * k;
            strand::draw(fx, STRAND, OT, g, D + 0x5D0, sp + 0xA0, ENDS + 0x10 * k);
            fx.step_down(g + 0xB, 0, 8);
            if fx.mem.i16(g + 0x28) == 0 {
                fx.random_dir(ENDS + 0x10 * k, 20.0);
            }
        }
    }
    for (g, x) in [(0x6C0u32, 0x650u32), (0x740, 0x6D0)] {
        if n(fx) >= 0x2F {
            fx.disc(D + g, D + x);
            fade(fx, D + g + 7, 0, 0x10, false);
        }
    }
    swath::draw(fx, D + 0x1A0, D);
    fade(fx, D + 0x1AB, 0, 0x10, false);
    swath::draw(fx, D + 0x70, D);
    fade(fx, D + 0x7B, 0, 0x10, false);
    fx.mem.write_u32(sp + 0xA4, 0x3F80_0000);
    fx.mem.write_u32(sp + 0xA8, 0x3F8A_3D71);
    // The delay slot stores v0 = 1.08 (not 1.0) into x [F 0x1E034A0].
    fx.mem.write_u32(sp + 0xA0, 0x3F8A_3D71);
    motes(fx, OT, MOTES, D + 0x750, sp + 0xA0);
    if n(fx) >= 0xA0 {
        fx.step_down(MOTES + 0xB, 0, 8);
    }
    for k in 0..0x3Cu32 {
        let p = D + 0xBB0 + 0xC * k;
        let y = ee::add(fx.mem.f(p + 4), if k % 2 == 0 { 2.0 } else { 3.0 });
        fx.mem.setf(p + 4, y);
        if !crate::exe::ee_lt(fx.mem.f(p + 4), 100.0) {
            let a = fx.rand_from(D, 0x01E0_3548) & 0xFFFF;
            fx.mem.write_u32(p + 4, 0);
            let c = fx.tcos(a);
            fx.mem.setf(p + 8, ee::mul(4.0, c));
            let s = fx.tsin(a);
            fx.mem.setf(p, ee::mul(4.0, s));
        }
        for o in [0u32, 4, 8] {
            let a = MOTES + 0x30 + 0x10 * k + o;
            let v = fx.mem.read_u32(a).wrapping_add(0x1000);
            fx.mem.write_u32(a, v);
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(N, m);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if m == 0xA0 {
        2
    } else if m == 0x35 {
        1
    } else if (0x3F..0xA0).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}

// The motes (0x1E01930, 0ff5e4222a02) moved to `blib::spin::motes`
//.
pub(crate) use crate::blib::spin::motes;
