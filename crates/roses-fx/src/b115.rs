//! BMODULE b115 (card 404), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02290, data area 0x1ED0000, 0x39A0 bytes) [F]
//!: b003's older pool - 200 tasks of 0x40 bytes at
//! D+0x448 (alive +0x36, age +0x32, step +0x24, each step called by its
//! own `jal`; spawned by 0x1E028F0 = b538's 0x1E01220 at the point D+0x2D0
//! with the colour D+0x36F4 and +0x30 = D+0x372C) - on a 170-update timer
//! D+0x36F0. The emitter 0x1E01450 counts D+0x3730: to 0x6D two red
//! streak rings a update (0x1E016C0) between the attacker's nodes 0x14 /
//! 0x1D, each drawn four times by 0x1E02A10 (13 streaks of quads from the
//! nodes 0x14 / 0x1D / 0x26 / 0x2F, far Z until 0x2E); on 0x2D..0x62 two
//! spinning sprites a update (0x1E01980, `billboard::sprite_square`) at
//! the nodes 0x65 / 0x5B; on 0x6C at node 0x14 eight orange rings
//! (0x1E01F00, `ring_t20`), eight red spinning rings (0x1E01D70, b148's
//! `ring_t20` step), thirty red quad rings (0x1E01BC0, b205's dark ring on
//! `ring_t`), a burning disc (`spawn::disc_step` 5225257f1409,
//! `quads4::fan18`) and b205's camera light (0x1E02070 on D+0x3650).
//! Returns 1 on update 0x62, 4 on 0x87, 2 on 0x78, 6 on 0x6D..0x72, 8 once
//! the timer has run out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::blib::{billboard, exe::camera_slots45, quads4, spawn, uv};
use crate::bus::Bus;
use crate::c061::{desc_uv4, rgbaq_at};
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 200;
const POOL: u32 = D + 0x448;
/// The step functions a task holds (+0x24), as the record's addresses.
const STREAK: u32 = 0x01E0_16C0;
const SPRITE: u32 = 0x01E0_1980;
const DISC: u32 = 0x01E0_1AE0;
const DARK: u32 = 0x01E0_1BC0;
const RING_T: u32 = 0x01E0_1D70;
const ORANGE: u32 = 0x01E0_1F00;
const CAMERA: u32 = 0x01E0_2070;
const POS: u32 = D + 0x2D0;
const CAM: u32 = D + 0x3650;
const POINTS: u32 = D + 0x3680;
const TIMER: u32 = D + 0x36F0;
const COLOUR: u32 = D + 0x36F4;
const OT2: u32 = D + 0x36FC;
const NODES: u32 = D + 0x371C;
const KIND: u32 = D + 0x372C;
const COUNTER: u32 = D + 0x3730;
const TABLE: u32 = D + 0x3734;
const SINES: u32 = D + 0x3834;
const SPR: u32 = 0x7000_0000;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// The entry 0x1E02290 (D, param; s0 = D, s1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x190);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(OT2, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x190);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x39A0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x372E, param);
    fx.mem.set32(TIMER, 0xAA);
    for (k, n) in [0x14, 0x1D, 0x26, 0x2F, 0x65, 0x5B].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0x372A, r);
    // 0x1E021F0(0, D): the textures (frame 0x40: ra, the entry's s2 (the
    // runner's, 0), s1 = param, s0 = D).
    let f = fx.enter(0x40);
    fx.saved(f, &[(0x30, 0x01E0_23B4), (0x20, 0), (0x10, param as i64 as u64), (0, u64::from(D))]);
    for k in 0..3u32 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x310 + 4 * k, t);
    }
    for k in 0xDu32..0x10 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x344 + 4 * (k - 0xD), t);
    }
    let t = fx.texture(5, 0);
    fx.mem.write_u32(D + 0x36D0, t);
    fx.leave(0x40);
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_23C0, 0x1E);
        fx.mem.set8(TABLE + k, v - 0xF);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    let low = fx.sp;
    for k in 0..TASKS {
        let slot = D + 0x40 * k;
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x24);
        // (frame, the return address after the step's jal, saves s1 / s2).
        let (frame, ra, more): (u32, u32, u8) = match f {
            STREAK => {
                streak(fx, t);
                (0x70, 0x01E0_2430, 2)
            }
            SPRITE => {
                sprite(fx, t);
                (0x30, 0x01E0_2450, 1)
            }
            DISC => {
                let c = spawn::disc("5225257f1409").expect("constants");
                spawn::disc_step_with(fx, c, DISC, t);
                (0x20, 0x01E0_2470, 0)
            }
            DARK => {
                dark(fx, t);
                (0x20, 0x01E0_2490, 0)
            }
            RING_T => {
                ring_t(fx, t);
                (0x20, 0x01E0_24B0, 0)
            }
            ORANGE => {
                orange(fx, t);
                (0x20, 0x01E0_24D0, 0)
            }
            CAMERA => {
                camera(fx, t);
                (0x30, 0x01E0_24F0, 1)
            }
            _ => panic!("b115: unknown task step {f:#x}"),
        };
        // The steps' saves: the entry's s0 = D, s1 = the slot, s2 = k.
        let base = low - frame;
        match more {
            0 => fx.saved(base, &[(0x10, u64::from(ra)), (0, u64::from(D))]),
            1 => fx.saved(base, &[(0x20, u64::from(ra)), (0x10, u64::from(slot)), (0, u64::from(D))]),
            _ => fx.saved(base, &[(0x40, u64::from(ra)), (0x30, u64::from(k)), (0x20, u64::from(slot)), (0x10, u64::from(D))]),
        }
    }
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x90, u64::from(D), 0x01E0_2514);
    let d = sp + 0x100;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let set_rect = |fx: &mut Fx, tex: u32, local: u32, id: i32| {
            let w = fx.mem.read_u32(tex);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x34);
            l2::rect(fx, local, id, sub);
            for k in 0..4 {
                let v = fx.mem.read_u32(local + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, v);
            }
        };
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x24);
        if f(fx) == STREAK {
            set_rect(fx, D + 0x34C, sp + 0xA0, 0x9B);
            for j in 0..4u32 {
                if fx.mem.i16(COUNTER) < 0x6C {
                    let h = fx.self_handle();
                    let s = fx.mem.i16(NODES + 2 * j);
                    node_position_saving(fx, h, s, POINTS + 0x10 * j, u64::from(D), Some(u64::from(j)), 0x01E0_25B8);
                }
                streaks(fx, t, POINTS + 0x10 * j, TABLE);
            }
        }
        if f(fx) == SPRITE {
            set_rect(fx, D + 0x36D0, sp + 0xB0, 0x14);
            for k in 0..3 {
                let v = ee::from_i32(fx.mem.i16(t + 2 * k));
                fx.mem.setf(sp + 0x90 + 4 * k, v);
            }
            billboard::sprite_square(fx, 0x01E0_1260, uv::uv_rect, D, t, 0, sp + 0x90);
        }
        if f(fx) == DISC {
            set_rect(fx, D + 0x34C, sp + 0xC0, 0x9B);
            quads4::fan18(fx, D, t, d);
        }
        if f(fx) == DARK {
            set_rect(fx, D + 0x34C, sp + 0xD0, 0x9B);
            billboard::ring_t(fx, uv::uv_rect, D, t, d, TABLE);
        }
        if f(fx) == RING_T {
            set_rect(fx, D + 0x314, sp + 0xE0, 0xA);
            billboard::ring_t20(fx, uv::uv_rect, D, t, d, TABLE);
        }
        if f(fx) == ORANGE {
            set_rect(fx, D + 0x34C, sp + 0xF0, 0x9B);
            billboard::ring_t20(fx, uv::uv_rect, D, t, d, TABLE);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x62 => 1,
        0x87 => 4,
        0x78 => 2,
        0x6D..=0x72 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E028F0(D, step, arg) (frame 0x60: ra, the emitter's s4..s0 = `r`):
/// the first free task cleared and set off at D+0x2D0 (as i16) with the
/// colour D+0x36F4 (twice), +0x30 = D+0x372C; 1 when one was free.
fn spawn(fx: &mut Fx, f: u32, r: [u64; 5], ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, r[4]), (0x30, r[3]), (0x20, r[2]), (0x10, r[1]), (0, r[0])]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(POOL + 0x40 * k + 0x36) == 0) {
        Some(k) => {
            let t = POOL + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            for c in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, POS, 0);
            let a = fx.mem.i16(KIND);
            fx.mem.set16(t + 0x30, a);
            fx.mem.write_u32(t + 0x20, 0);
            fx.mem.write_u32(t + 0x24, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// 0x1E01450(D, 0) (frame 0x60: ra, the entry's s2 (the runner's, 0), s1
/// = param, s0 = D): the colour (0x80, 0, 0, 0), the update counter
/// D+0x3730 and the spawns.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x60);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, 0x01E0_23FC), (0x20, 0), (0x10, p), (0, u64::from(D))]);
    let dd = u64::from(D);
    l2::rgb(fx, COLOUR, 0x80, 0, 0);
    fx.mem.write_u8(COLOUR + 3, 0);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    // The emitter's s1 / s2 (s0 = D, s3 / s4 the runner's, 0).
    let (mut s1, mut s2) = (p, 0u64);
    let copy_pos = |fx: &mut Fx, from: u32| {
        for o in 0..3 {
            let w = fx.mem.read_u32(from + 4 * o);
            fx.mem.write_u32(POS + 4 * o, w);
        }
    };
    if fx.mem.i16(COUNTER) < 0x6E {
        spawn(fx, STREAK, [dd, s1, s2, 0, 0], 0x01E0_14A4);
        spawn(fx, STREAK, [dd, s1, s2, 0, 0], 0x01E0_14B8);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x2D..0x63).contains(&c) {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES + 8);
        node_position_saving(fx, h, s, sp + 0x40, dd, Some(s1), 0x01E0_14EC);
        s1 = u64::from(sp + 0x44);
        s2 = u64::from(sp + 0x48);
        copy_pos(fx, sp + 0x40);
        spawn(fx, SPRITE, [dd, s1, s2, 0, 0], 0x01E0_1520);
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES + 0xA);
        node_position_saving(fx, h, s, sp + 0x40, dd, Some(s1), 0x01E0_1538);
        copy_pos(fx, sp + 0x40);
        spawn(fx, SPRITE, [dd, s1, s2, 0, 0], 0x01E0_1564);
    }
    if fx.mem.i16(COUNTER) == 0x6C {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES);
        node_position_saving(fx, h, s, sp + 0x40, dd, Some(s1), 0x01E0_158C);
        let h = fx.target_handle();
        unit_position_saving(fx, h, sp + 0x50, dd, 0x01E0_15A0);
        copy_pos(fx, sp + 0x40);
        fx.mem.set16(KIND, 1);
        for k in 0..8u64 {
            spawn(fx, ORANGE, [dd, k, s2, 0, 0], 0x01E0_15D8);
        }
        s1 = 8;
    }
    if fx.mem.i16(COUNTER) == 0x6C {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES);
        node_position_saving(fx, h, s, sp + 0x40, dd, Some(s1), 0x01E0_1610);
        copy_pos(fx, sp + 0x40);
        fx.mem.set16(KIND, 1);
        for k in 0..8u64 {
            spawn(fx, RING_T, [dd, k, s2, 0, 0], 0x01E0_1648);
        }
        for k in 0..0x1Eu64 {
            spawn(fx, DARK, [dd, k, s2, 0, 0], 0x01E0_1670);
        }
        spawn(fx, DISC, [dd, 0x1E, s2, 0, 0], 0x01E0_1694);
        spawn(fx, CAMERA, [dd, 0x1E, s2, 0, 0], 0x01E0_16A8);
    }
    fx.leave(0x60);
}

/// 0x1E016C0 (D, task) (frame 0x70: s0 = the task, s1 = D; f20 / f21 the
/// entry's, 0 [H]): a red streak ring (size 6, cell 2, k = rand % 4 at
/// +0x3A, +0x18 rand % 0xFFFF, growth 7 - k / 2, row 0..9, life 1..3);
/// 0x80 for its life, then out by 10 (by 8 from update 0x6C); the radius
/// +0x30 up by the growth (twice from update 0x6A); the point k tenths of
/// the way from node 0x14 to node 0x1D.
fn streak(fx: &mut Fx, t: u32) {
    use ee::{add, div, from_i32, mul, sub};
    let sp = fx.enter(0x70);
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 4, 0);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 6);
        let v = fx.rand_from(t, 0x01E0_16FC);
        fx.mem.set16(t + 0x3A, v.wrapping_rem(4));
        fx.mem.set16(t + 0x34, 2);
        l2::rgb(fx, t + 0x28, 0x80, 0x20, 0x20);
        let v = rmod(fx, t, 0x01E0_1738, 0xFFFF);
        fx.mem.set32(t + 0x18, v);
        fx.mem.set16(t + 0x30, 0);
        let k = fx.mem.i16(t + 0x3A) >> 1;
        fx.mem.set16(t + 0xE, 7 - k);
        let v = rmod(fx, t, 0x01E0_176C, 0xA);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_1788, 3);
        fx.mem.set16(t + 6, v + 1);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else if fx.mem.i16(COUNTER) < 0x6C {
        if a >= 0x11 {
            fx.mem.write_u8(t + 0x2B, a - 0xA);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    } else if a >= 0xA {
        fx.mem.write_u8(t + 0x2B, a - 8);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    let g = fx.mem.i16(t + 0xE);
    let g = if fx.mem.i16(COUNTER) < 0x6A { g } else { 2 * g };
    let r = fx.mem.i16(t + 0x30) + g;
    fx.mem.set16(t + 0x30, r);
    age(fx, t);
    let h = fx.self_handle();
    let s = fx.mem.i16(NODES);
    node_position_saving(fx, h, s, sp + 0x50, u64::from(t), Some(u64::from(D)), 0x01E0_1878);
    let h = fx.self_handle();
    let s = fx.mem.i16(NODES + 2);
    node_position_saving(fx, h, s, sp + 0x60, u64::from(t), Some(u64::from(sp + 0x60)), 0x01E0_1894);
    let n1 = [fx.mem.f(sp + 0x50), fx.mem.f(sp + 0x54), fx.mem.f(sp + 0x58)];
    let n2 = [fx.mem.f(sp + 0x60), fx.mem.f(sp + 0x64), fx.mem.f(sp + 0x68)];
    let step: [f32; 3] = std::array::from_fn(|i| div(sub(n2[i], n1[i]), 10.0));
    for (i, ra) in [0x01E0_1904u32, 0x01E0_1928, 0x01E0_194C].into_iter().enumerate() {
        let k = from_i32(fx.mem.i16(t + 0x3A));
        let v = add(n1[i], mul(step[i], k));
        fx.fix_residue(v, ra);
        fx.mem.set16(t + 2 * i as u32, ee::fixsfsi(v));
    }
    fx.leave(0x70);
}

/// 0x1E02A10(D, task, point, table) (frame 0x1F0): `billboard::ring_t20`
/// compiled for one point and 13 quads: its own descriptor (sp + 0x158;
/// 9 values, FST, ALPHA 0x48, colour +0x28, UVs D+0x94 in orientation 0,
/// texture D+0x88), quad k of size +0x38 pushed out by the bytes `table`
/// [k + +8 (+0, +0x32, +0x64)] / 20 times +0x30 + k / 2, its first two
/// corners then (+-2.5, 0, 0) (+-5.5 from update 0x6B), all through the
/// camera's angles plus +0x10.. at `point` (scale 0.1); far Z at key 0
/// until update 0x2E, then keyed by the mean w, into D+0x36FC.
fn streaks(fx: &mut Fx, t: u32, point: u32, table: u32) {
    let sp = fx.enter(0x1F0);
    for k in 0..4 {
        let w = fx.mem.read_u32(point + 4 * k);
        fx.mem.write_u32(sp + 0x80 + 4 * k, w);
    }
    let d = sp + 0x158;
    desc_uv4(fx, d);
    fx.desc_alpha(d, 0x80_0000_0048);
    fx.mem.write_u32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    rgbaq_at(fx, sp + 0x1E8, t + 0x28, d + 0x10);
    uv::uv_rect(fx, d, D + 0x94, 0);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xE0 + 4 * k, 0x3DCC_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0x140);
    for k in 0..3 {
        let v = fx.mem.read_u32(sp + 0x140 + 4 * k).wrapping_add(fx.mem.read_u32(t + 0x10 + 4 * k));
        fx.mem.write_u32(sp + 0x140 + 4 * k, v);
    }
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x80 + 4 * k);
        fx.mem.write_u32(sp + 0xD0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x100, sp + 0xE0, sp + 0x140, sp + 0xD0);
    let q = sp + 0x90;
    let mut k = 0i32;
    while k < 0xD {
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, q, s, s);
        for (axis, off) in [(0, 0u32), (1, 0x32), (2, 0x64)] {
            let i = fx.mem.i16(t + 8);
            let b = ee::from_i32(fx.mem.i8(table.wrapping_add(k as u32).wrapping_add(i as u32).wrapping_add(off)));
            let r = ee::from_i32(fx.mem.i16(t + 0x30) + (k >> 1));
            l2::add_column(fx, q, axis, ee::mul(ee::div(b, 20.0), r));
        }
        let e: f32 = if fx.mem.i16(COUNTER) < 0x6B { 2.5 } else { 5.5 };
        for (o, v) in [(4u32, 0u32), (0x14, 0), (0, e.to_bits()), (0x10, ee::neg(e).to_bits()), (8, 0), (0x18, 0)] {
            fx.mem.write_u32(q + o, v);
        }
        if l2::project(fx, q, SPR, sp + 0x100, d, sp + 0xF0) {
            let ot = fx.mem.read_u32(OT2);
            if fx.mem.i16(COUNTER) < 0x2F {
                l2::far_z(fx, &[d + 0x50, d + 0x40, d + 0x30, d + 0x20]);
                fx.submit(d, ot, 0);
            } else {
                let key = ee::fixunssfsi(fx.mem.f(sp + 0xF0)) as i32;
                fx.submit(d, ot, key);
            }
        }
        k = (k + 1) as i16 as i32;
    }
    fx.leave(0x1F0);
}

/// 0x1E01980 (D, task) (frame 0x30: s0 = the task, s1 = D): a spinning
/// sprite (size 60, cell 5, grey, +0x30 / +0x3A random angles); 0x80 for
/// 7 updates, then out by 15; +8 / +0xC moved by 16 sin / cos(+0x3A).
fn sprite(fx: &mut Fx, t: u32) {
    fx.enter(0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x3C);
        fx.mem.set16(t + 0x34, 5);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, 0x01E0_19D0, 0x168);
        fx.mem.set16(t + 0x30, v);
        let v = rmod(fx, t, 0x01E0_19EC, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 7 {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0x11 {
            fx.mem.write_u8(t + 0x2B, a - 0xF);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    for (o, cos, ra) in [(8u32, 0, 0x01E0_1A7Cu32), (0xC, 1, 0x01E0_1AB4)] {
        let a = fx.mem.i16(t + 0x3A);
        let s = l2::sin_deg(fx, SINES, a, cos);
        let v = ee::add(ee::from_i32(fx.mem.i16(t + o)), ee::mul(16.0, s));
        fx.fix_residue(v, ra);
        fx.mem.set16(t + o, ee::fixsfsi(v));
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E01BC0 (D, task): b205's dark ring in red (size 10, cell rand % 4,
/// three random angles, (0x80, 0x10, 0x10), growth 3..18, row 0..9, life
/// 5..11; in by 0x38 under 0x71, out by 10).
fn dark(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0xA);
        let v = fx.rand_from(t, 0x01E0_1BE8);
        fx.mem.set16(t + 0x34, v % 4);
        for (o, ra) in [(0x10u32, 0x01E0_1C04u32), (0x14, 0x01E0_1C20), (0x18, 0x01E0_1C3C)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        l2::rgb(fx, t + 0x28, 0x80, 0x10, 0x10);
        fx.mem.set16(t + 0x30, 0);
        let v = rmod(fx, t, 0x01E0_1C68, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = fx.rand_from(t, 0x01E0_1C84);
        fx.mem.set16(t + 0xE, v % 0x10 + 3);
        let v = rmod(fx, t, 0x01E0_1CA4, 0xA);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_1CC0, 7);
        fx.mem.set16(t + 6, v + 5);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < 0x71 {
            fx.mem.write_u8(t + 0x2B, a + 0x38);
        }
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x2B, a - 0xA);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E01D70 (D, task): b148's spinning ring step (0x1E037A0) in red (size
/// 2, cell rand % 4, (0x80, 0x20, 0x20), two random angles, growth 3..18,
/// row 0..9, life 5..14; 0x80 for its life, then out by 15).
fn ring_t(fx: &mut Fx, t: u32) {
    let at = RING_T;
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 2);
        let v = fx.rand_from(t, at + 0x28);
        fx.mem.set16(t + 0x34, v % 4);
        l2::rgb(fx, t + 0x28, 0x80, 0x20, 0x20);
        let v = rmod(fx, t, at + 0x5C, 0xFFFF);
        fx.mem.set32(t + 0x18, v);
        let v = rmod(fx, t, at + 0x78, 0xFFFF);
        fx.mem.set32(t + 0x10, v);
        fx.mem.set16(t + 0x30, 0);
        let v = rmod(fx, t, at + 0x98, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = fx.rand_from(t, at + 0xB4);
        fx.mem.set16(t + 0xE, v % 0x10 + 3);
        let v = rmod(fx, t, at + 0xD4, 0xA);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, at + 0xF0, 0xA);
        fx.mem.set16(t + 6, v + 5);
        fx.mem.set16(t + 0x36, 2);
    }
    crate::b148::fade(fx, t, Some(0x80));
    let v = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x30, v);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E01F00 (D, task) (b046's never-spawned 0x1E02890): an orange ring
/// (size 2, cell rand % 4, (0x80, 0x40, 0), +0x18 random, angle +0x3A,
/// growth 3..18, row 0..9, life 1..3); 0x80 for its life, then out by 10;
/// the radius +0x30 up by the growth every update.
fn orange(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 2);
        let v = fx.rand_from(t, 0x01E0_1F28);
        fx.mem.set16(t + 0x34, v % 4);
        l2::rgb(fx, t + 0x28, 0x80, 0x40, 0);
        let v = rmod(fx, t, 0x01E0_1F5C, 0xFFFF);
        fx.mem.set32(t + 0x18, v);
        fx.mem.set16(t + 0x30, 0);
        let v = rmod(fx, t, 0x01E0_1F7C, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = fx.rand_from(t, 0x01E0_1F98);
        fx.mem.set16(t + 0xE, v % 0x10 + 3);
        let v = rmod(fx, t, 0x01E0_1FB8, 0xA);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_1FD4, 3);
        fx.mem.set16(t + 6, v + 1);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0x11 {
            fx.mem.write_u8(t + 0x2B, a - 0xA);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x30, r);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02070 (D, task) (frame 0x30: s0 = the task, s1 = D): b205's camera
/// light on D+0x3650 (b564's): its weight +0x28 up by 0.2 to 1, then down
/// by 0.1 (the task over at 0.1); camera slots 4 / 5 from D+0x3650 (0 once
/// over); over when the timer reads 1.
fn camera(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x4080_0000), (8, 0x4000_0000), (0xC, 0), (0x20, 0), (0x24, 0x42F0_0000), (0x28, 0)] {
            fx.mem.write_u32(CAM + o, v);
        }
        for k in 0..3 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(CAM + 0x10 + 4 * k, v);
        }
        fx.mem.set16(t + 0x36, 2);
        fx.mem.set16(t + 0x30, 0);
    }
    let w = fx.mem.f(CAM + 0x28);
    if fx.mem.i16(t + 0x30) == 0 {
        if w < 1.0 {
            fx.mem.setf(CAM + 0x28, ee::add(w, f32::from_bits(0x3E4C_CCCD)));
        } else {
            fx.mem.set16(t + 0x30, 1);
        }
    } else if w <= f32::from_bits(0x3DCC_CCCD) {
        fx.mem.set16(t + 0x36, 0);
    } else {
        fx.mem.setf(CAM + 0x28, ee::sub(w, f32::from_bits(0x3DCC_CCCD)));
    }
    let (p, ra) = if fx.mem.i16(t + 0x36) == 0 { (0, 0x01E0_21A4) } else { (CAM, 0x01E0_21B8) };
    camera_slots45(fx, p);
    // 0x1F7E20's frame (s0 = the task, ra).
    fx.saved(sp - 0x20, &[(0, u64::from(t)), (0x10, ra)]);
    if fx.mem.i32(TIMER) == 1 {
        fx.mem.set16(t + 0x36, 0);
    }
    fx.leave(0x30);
}
