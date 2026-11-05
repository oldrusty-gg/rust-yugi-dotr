//! BMODULE b163 (card 314), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E032D0, data area 0x1ED0000) [F] (helper C26, 3
//! Nov 2026): b189's family (150 tasks of 0x70 bytes at D+0x2A0, alive
//! +0x58, step +0x44 through `jalr`, 0x1E032C0 an idle step; spawned by
//! 0x1E03CD0 = b272's spawn with the colour D+0x44C4 and the kind
//! D+0x4528; b189's globals) drawn with sparks (`quads4::sparks`
//! 0x1E01320), flat arcs (`billboard::arc_strips_flat` 0x1E01720), rings
//! of quads (`billboard::ring_quads` 0x1E00D70) and a quad
//! (`billboard::quad` 0x1E01A80), UVs by uv_rect. A point (+0x4470) rides
//! the attacker's node 0x48 until update 315, drifting sideways (z) by
//! 0.2 k (k + 1 a update over 157..189, -0.2 when facing); from 316 to the
//! target in 15 steps of 1 / 15.5. 66..335 a red flat arc on it from 145
//! and a green / white spark ring a update (the target's position read
//! then overwritten by the point: odd original kept), 80..335 a purple
//! ring, 148 a violet glowing quad; on 330 ten grey spark rings opening
//! and closing (now 0x200 a update about z) and a white / violet flat arc,
//! on 352 ten white spark rings, ten violet rings of quads and another
//! arc. The draws of 0x1E02130 / 0x1E025A0 / 0x1E02D70 (b189's burst, a
//! `quads4::ring_band_f` and the wide ring) are never spawned (not ported).
//! Returns 1 on update 320, 4 on 385, 2 on 365, 6 on 331..359, 8 once the
//! 390-update timer +0x44C0 has run out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const BURST: u32 = 0x01E0_2130;
const PURPLE: u32 = 0x01E0_2280;
const RED_ARC: u32 = 0x01E0_2400;
const BAND: u32 = 0x01E0_25A0;
const WHITE_ARC: u32 = 0x01E0_26F0;
const GLOW: u32 = 0x01E0_2830;
const SWING: u32 = 0x01E0_29A0;
const VIOLET_RING: u32 = 0x01E0_2BD0;
const WIDE: u32 = 0x01E0_2D70;
const SPARK_A: u32 = 0x01E0_2F80;
const SPARK_B: u32 = 0x01E0_3120;
const IDLE: u32 = 0x01E0_32C0;
/// The return address of the entry's `jalr` into a step (0x1E035C0 + 8).
const TASK_RA: u64 = 0x01E0_35C8;
const POS: u32 = D + 0x250;
const POINT: u32 = D + 0x4470;
const TEX: u32 = D + 0x44A0;
const TIMER: u32 = D + 0x44C0;
const COLOUR: u32 = D + 0x44C4;
const K: u32 = D + 0x44E4;
const NODES: u32 = D + 0x44F8;
const TARGET_NODE: u32 = D + 0x4508;
const DIVISOR: u32 = D + 0x4518;
const KIND: u32 = D + 0x4528;
const FACING: u32 = D + 0x452C;
const COUNTER: u32 = D + 0x4530;
const BYTES: u32 = D + 0x4532;
const BYTES_B: u32 = D + 0x4632;
const SINES: u32 = D + 0x4734;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E032D0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x100);
    fx.mem.write_u32(D + 0x44CC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x100);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x48A0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x452E, param);
    fx.mem.set32(TIMER, 0x186);
    for (k, node) in [0x48, 0x34, 0x43, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    for k in 0xE..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x298 + 4 * (k as u32 - 0xE), t);
    }
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xF0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xF4)), 180.0);
    fx.fix_residue(y, 0x01E0_346C);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    // rand's frames: the entry's s0 = D.
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3494, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_34E4);
        fx.mem.set8(BYTES + k, v % 4);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3534, 0x28);
        fx.mem.set8(BYTES_B + k, v - 0x14);
    }
    fx.mem.set16(DIVISOR, 1);
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    // The entry's s1 counts the idle tasks (never spawned: 0).
    let mut s1 = 0u64;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 {
            continue;
        }
        if f == IDLE {
            s1 += 1;
            continue;
        }
        // The steps' prologues: the entry's s0 = D, s1.
        if f == RED_ARC || f == SWING {
            let s = fx.enter(0x30);
            fx.saved(s, &[(0x20, TASK_RA), (0x10, s1), (0, u64::from(D))]);
            if f == RED_ARC {
                red_arc(fx, t);
            } else {
                swing(fx, t);
            }
            fx.leave(0x30);
            continue;
        }
        let s = fx.enter(0x20);
        fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]);
        match f {
            PURPLE => ring(fx, t, PURPLE, (0x40, 0x20, 0x80), false),
            VIOLET_RING => ring(fx, t, VIOLET_RING, (0x80, 0x10, 0xFF), true),
            WHITE_ARC => white_arc(fx, t, s1),
            GLOW => glow(fx, t, s1),
            SPARK_A => spark(fx, t, false),
            SPARK_B => spark(fx, t, true),
            _ => panic!("b163: unported task step {f:#x}"),
        }
        fx.leave(0x20);
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        assert!(![BURST, BAND, WIDE].contains(&f(fx)), "b163: unported draw");
        if f(fx) == PURPLE {
            cell(fx, t, sp + 0x80, 0x14);
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 4);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if f(fx) == RED_ARC {
            fx.mem.set16(D + 0xB8, 0x18);
            let n = if fx.mem.i16(COUNTER) < 0xE3 { 0x21 } else { 0x20 };
            billboard::arc_strips_flat(fx, D, t, n, POINT);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == WHITE_ARC {
            fx.mem.set16(D + 0xB8, 0x18);
            billboard::arc_strips_flat(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == GLOW {
            let w = fx.mem.read_u32(TEX + 8);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0x90, 0x14);
            fx.mem.set16(D + 0xBE, 5);
            for k in 0..4 {
                let v = fx.mem.read_u32(POINT + 4 * k);
                fx.mem.write_u32(sp + 0x60 + 4 * k, v);
            }
            let n = if fx.mem.i16(COUNTER) < 0xE3 { 0x21 } else { 0x20 };
            billboard::quad(fx, uv::uv_rect, D, t, n, sp + 0x60);
        }
        if f(fx) == SWING {
            cell(fx, t, sp + 0xA0, 0x14);
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 0x18);
            quads4::sparks(fx, D, t, 0x20, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == VIOLET_RING {
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0xB0, 0x9B);
            fx.mem.set16(D + 0xB8, 5);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        for (step, r, ra) in [(SPARK_A, sp + 0xD0, 0x01E0_3A90u32), (SPARK_B, sp + 0xE0, 0x01E0_3B68)] {
            if f(fx) != step {
                continue;
            }
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            for (o, v) in [(0x94u32, 0i32), (0x96, 0), (0x98, 0x1F), (0x9A, 0x7F)] {
                fx.mem.set16(D + o, v);
            }
            fx.mem.set16(D + 0xB8, 0);
            let h = fx.target_handle();
            unit_position_saving(fx, h, sp + 0x60, u64::from(D), ra);
            fx.mem.write_u32(sp + 0x64, 0x4248_0000);
            for k in 0..4 {
                let v = fx.mem.read_u32(POINT + 4 * k);
                fx.mem.write_u32(sp + 0x60 + 4 * k, v);
            }
            cell(fx, t, r, 0x14);
            let v = fx.mem.i16(D + 0x9A) >> 1;
            fx.mem.set16(D + 0x9A, v);
            fx.mem.set16(D + 0xB8, 0x18);
            quads4::sparks(fx, D, t, 0x20, sp + 0x60);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x140 => 1,
        0x181 => 4,
        0x16D => 2,
        0x14B..=0x167 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` frame +0x56 (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32, id: i32) {
    let sub = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// 0x1E03CD0(D, step, 0) (b189's spawn): the first free task cleared and
/// set off at D+0x250; `saves` the caller's s0..s4, `ra` the return
/// address.
fn spawn(fx: &mut Fx, step: u32, saves: [u64; 5], ra: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, saves[4]), (0x30, saves[3]), (0x20, saves[2]), (0x10, saves[1]), (0, saves[0])]);
    if let Some(k) = (0..TASKS).find(|&k| fx.mem.i16(D + 0x2F8 + 0x70 * k) == 0) {
        let t = D + 0x2A0 + 0x70 * k;
        fx.bzero(t, 0x70);
        fx.mem.set16(t + 0x58, 1);
        for o in [0x60u32, 0x64] {
            let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
            l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
            let a = fx.mem.read_u8(COLOUR + 3);
            fx.mem.write_u8(t + o + 3, a);
        }
        l2::to_i16x4(fx, t, POS, 0);
        for k in 0..3 {
            let w = fx.mem.read_u32(POS + 4 * k);
            fx.mem.write_u32(t + 0x10 + 4 * k, w);
        }
        let s = fx.mem.i16(KIND);
        fx.mem.set16(t + 0x50, s);
        fx.mem.write_u32(t + 0x40, 0);
        fx.mem.write_u32(t + 0x44, step);
    }
    fx.leave(0x60);
}

/// 0x1E01D40(D, 0): the update counter, the point and the spawns (frame
/// 0x50: ra, the entry's s1 = 0, s0 = D; then s0 = D, s1 a loop count).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, 0x01E0_3588), (0x10, 0), (0, u64::from(D))]);
    // The spawns' saves: s0 = D, s1, the entry's s2 = 0, s3 = the
    // parameter, s4 the runner's (0 [H]).
    let p = param as i64 as u64;
    let saves = |s1: u64| [u64::from(D), s1, 0, p, 0];
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let to_pos = |fx: &mut Fx| {
        for k in 0..4 {
            let w = fx.mem.read_u32(POINT + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
    };
    if c(fx) < 0x13B {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES);
        node_position_saving(fx, h, s, POINT, u64::from(D), Some(0), 0x01E0_1D88);
        let v = if fx.mem.i16(FACING) != 0 { 0xBE4C_CCCD } else { 0x3E4C_CCCD };
        fx.mem.write_u32(D + 0x4498, v);
        let z = ee::add(fx.mem.f(POINT + 8), ee::mul(fx.mem.f(D + 0x4498), fx.mem.f(K)));
        fx.mem.setf(POINT + 8, z);
        if (0x9D..0xBE).contains(&c(fx)) {
            let k = ee::add(fx.mem.f(K), 1.0);
            fx.mem.setf(K, k);
        }
    } else {
        if c(fx) == 0x13B {
            let h = fx.target_handle();
            let s = fx.mem.i16(TARGET_NODE);
            node_position_saving(fx, h, s, sp + 0x30, u64::from(D), Some(0), 0x01E0_1E24);
            for (o, to) in [(8u32, D + 0x4488), (4, D + 0x4484), (0, D + 0x4480)] {
                let v = ee::div(ee::sub(fx.mem.f(sp + 0x30 + o), fx.mem.f(POINT + o)), 15.5);
                fx.mem.setf(to, v);
            }
        }
        if (0x13C..0x14B).contains(&c(fx)) {
            for (o, by) in [(8u32, D + 0x4488), (4, D + 0x4484), (0, D + 0x4480)] {
                let v = ee::add(fx.mem.f(POINT + o), fx.mem.f(by));
                fx.mem.setf(POINT + o, v);
            }
        }
    }
    let mut s1 = 0u64;
    if c(fx) == 0x14A {
        to_pos(fx);
        fx.mem.set16(KIND, 0);
        for k in 0..0xAu64 {
            spawn(fx, SWING, saves(k), 0x01E0_1F04);
        }
        s1 = 0xA;
        spawn(fx, WHITE_ARC, saves(s1), 0x01E0_1F38);
    }
    if c(fx) == 0x160 {
        to_pos(fx);
        fx.mem.set16(KIND, 1);
        for k in 0..0xAu64 {
            spawn(fx, SPARK_B, saves(k), 0x01E0_1F8C);
        }
        for k in 0..0xAu64 {
            spawn(fx, VIOLET_RING, saves(k), 0x01E0_1FC8);
        }
        s1 = 0xA;
        spawn(fx, WHITE_ARC, saves(s1), 0x01E0_2000);
    }
    if (0x50..0x150).contains(&c(fx)) {
        to_pos(fx);
        spawn(fx, PURPLE, saves(s1), 0x01E0_2050);
    }
    if (0x42..0x150).contains(&c(fx)) {
        to_pos(fx);
        if c(fx) >= 0x91 {
            spawn(fx, RED_ARC, saves(s1), 0x01E0_20B0);
        }
        spawn(fx, SPARK_A, saves(s1), 0x01E0_20C4);
        if c(fx) == 0x94 {
            spawn(fx, GLOW, saves(s1), 0x01E0_20E8);
        }
    }
    if (0x51..0x5F).contains(&c(fx)) {
        let v = fx.mem.i16(DIVISOR) + 7;
        fx.mem.set16(DIVISOR, v);
    }
    fx.leave(0x50);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// Byte +0x63 set to `full` for `life` updates, then down by `down` while
/// at least `floor` (else the task ends).
fn hold(fx: &mut Fx, t: u32, life: i32, full: u8, floor: u8, down: u8) {
    if fx.mem.i16(t + 0x54) < life {
        fx.mem.write_u8(t + 0x63, full);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= floor {
            fx.mem.write_u8(t + 0x63, a - down);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
}

/// Byte +0x63 up by `up` under `cap` for `life` updates, then down by
/// `down` while at least `floor` (else the task ends).
fn fade(fx: &mut Fx, t: u32, life: i32, cap: u8, up: u8, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < life {
        if a < cap {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(up));
        }
    } else if a >= floor {
        fx.mem.write_u8(t + 0x63, a - down);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E02280 (purple (0x40, 0x20, 0x80), size 10, radius 10..59 fixed) /
/// 0x1E02BD0 (`grow`: violet (0x80, 0x10, 0xFF), size 20, radius 10..59
/// growing by 16..45) (D, task) = b189's 0x1E023E0 / 0x1E02D00: a ring of
/// quads: cell 0..3, a random angle, +0xA rand % 8, +0xC the spawn kind,
/// row 0..34, life 3..7; 0x80 during the life, then down by 8.
fn ring(fx: &mut Fx, t: u32, f: u32, rgb: (i32, i32, i32), grow: bool) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, if grow { 0x41A0_0000 } else { 0x4120_0000 });
        let v = fx.rand_from(t, f + 0x28);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, rgb.0, rgb.1, rgb.2);
        let v = rmod(fx, t, f + 0x5C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, f + 0x78);
        fx.mem.set16(t + 0xA, v % 8);
        let k = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, k);
        let v = rmod(fx, t, f + 0x9C, 0x32);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = if grow { rmod(fx, t, f + 0xBC, 0x1E) + 0x10 } else { rmod(fx, t, f + 0xBC, 0xA) + 6 };
        fx.mem.set16(t + 0xE, v);
        let v = rmod(fx, t, f + 0xDC, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0xF8, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    let life = fx.mem.i16(t + 6);
    hold(fx, t, life, 0x80, 0xA, 8);
    if grow && fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E02400 (D, task) (b189's red arc in other colours): a flat arc on
/// the point (radius 10 + rand % D+0x4518 growing by 1, spans 15 / 15,
/// grey inside, the outer (0x40, 0x10, 0x80) / (0x20, 0x80, 0x40) on odd /
/// even updates), facing the camera turned a quarter; in by 0x30 under
/// 0x60 for 2 updates, out by 11.
fn red_arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = fx.rand_from(t, 0x01E0_242C);
        let m = fx.mem.i16(DIVISOR);
        let r = fx.div(v, m).1;
        fx.mem.setf(t + 0x48, ee::add(10.0, ee::from_i32(r)));
        fx.mem.write_u32(t + 0x4C, 0);
        l2::rgb(fx, t + 0x60, 0x68, 0x68, 0x68);
        l2::rgb(fx, t + 0x64, 0x40, 0x10, 0x80);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    fade(fx, t, 2, 0x60, 0x30, 0xD, 0xB);
    if fx.mem.i16(t + 0x54) % 2 == 1 {
        l2::rgb(fx, t + 0x64, 0x40, 0x10, 0x80);
    } else {
        l2::rgb(fx, t + 0x64, 0x20, 0x80, 0x40);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let v = ee::add(fx.mem.f(t + 0x48), 1.0);
        fx.mem.setf(t + 0x48, v);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_256C);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
}

/// 0x1E026F0 (D, task) (b189's white arc with a violet (0x80, 0x20, 0xFF)
/// outer): radius 250 / 300 by the spawn kind, growing by 1, spans 15 /
/// 15, facing the camera turned a quarter; in by 0x60 under 0xC0 for 12
/// updates, out by 11. `s1` the entry's (0x205580 saves it).
fn white_arc(fx: &mut Fx, t: u32, s1: u64) {
    if fx.mem.i16(t + 0x58) == 1 {
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.write_u32(t + 0x48, 0x437A_0000);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.write_u32(t + 0x48, 0x4396_0000);
        }
        fx.mem.write_u32(t + 0x4C, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x80, 0x20, 0xFF);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    fade(fx, t, 0xC, 0xC0, 0x60, 0xD, 0xB);
    if fx.mem.i16(t + 0x54) != 0 {
        let v = ee::add(fx.mem.f(t + 0x48), 1.0);
        fx.mem.setf(t + 0x48, v);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), s1, 0x01E0_27F8);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
}

/// 0x1E02830 (D, task) (b189's glow in violet (0x80, 0x40, 0xFF), held
/// 200 updates): a glowing quad on the point (5 growing by 1 to 30, cell
/// 3 / 5 alternating, turned by the camera's angles); both alphas in by
/// 0x40 under 0xC0, then +0x63 down by 14 twice a update.
fn glow(fx: &mut Fx, t: u32, s1: u64) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 5);
        fx.mem.set16(t + 0xA, 5);
        fx.mem.set16(t + 0x56, 3);
        l2::rgb(fx, t + 0x60, 0x80, 0x40, 0xFF);
        l2::rgb(fx, t + 0x64, 0x80, 0x40, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0xC8 {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0xC0 {
                fx.mem.write_u8(t + o, a + 0x40);
            }
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0x10 {
            fx.mem.write_u8(t + 0x63, a - 0xE);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0x10 {
            fx.mem.write_u8(t + 0x63, a - 0xE);
        }
    }
    let c = if fx.mem.i16(t + 0x56) == 3 { 5 } else { 3 };
    fx.mem.set16(t + 0x56, c);
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [8u32, 0xA] {
            let s = fx.mem.i16(t + o);
            if s < 0x1E {
                fx.mem.set16(t + o, s + 1);
            }
        }
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), s1, 0x01E0_2978);
    age(fx, t);
}

/// 0x1E029A0 (D, task) (b189's swing): a grey spark ring swinging open and
/// shut: radius 600 sin(+0x50) (+0x50 up by 8, by 6 from 90; the outer
/// 400 inside, not below 0), spans 15 / 10, a random cell and angles, the
/// roll +0x28 up 0x200 a update; colours (0x50 x3) / (0x80 x3), alphas
/// 0x20 / 0x80 for 20 updates, then the task ends once the radius is under
/// 80; from 91 degrees +0x63 down by 7.
fn swing(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42C8_0000);
        fx.mem.setf(t + 0x4C, ee::sub(100.0, 58.0));
        l2::rgb(fx, t + 0x60, 0x50, 0x50, 0x50);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x50, 0);
        let v = fx.rand_from(t, 0x01E0_2A10);
        fx.mem.set16(t + 0x56, v % 8);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.write_u32(t + 0x34, 0x437A_0000);
        let q = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, q);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        for (o, ra) in [(0x20u32, 0x01E0_2A64u32), (0x24, 0x01E0_2A80), (0x28, 0x01E0_2A9C)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
    }
    if fx.mem.i16(t + 0x54) < 0x14 {
        fx.mem.write_u8(t + 0x67, 0x80);
        fx.mem.write_u8(t + 0x63, 0x20);
    } else if fx.mem.f(t + 0x48) < 80.0 {
        fx.mem.set16(t + 0x58, 0);
    }
    let a = fx.mem.i16(t + 0x50);
    let s = l2::sin_deg(fx, SINES, a, 0);
    let r = ee::mul(600.0, s);
    let inner = ee::sub(r, 400.0);
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, inner);
    if inner < 0.0 {
        fx.mem.write_u32(t + 0x4C, 0);
    }
    if fx.mem.i16(t + 0x50) >= 0x5B {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 9 {
            fx.mem.write_u8(t + 0x63, a - 7);
        }
    }
    let a = fx.mem.i16(t + 0x50);
    fx.mem.set16(t + 0x50, if a < 0x5A { a + 8 } else { a + 6 });
    let r = fx.mem.read_u32(t + 0x28).wrapping_add(0x200);
    fx.mem.write_u32(t + 0x28, r);
    age(fx, t);
}

/// 0x1E02F80 (`b` false: radius 10, grey (0x80 x3) / green-white outer,
/// spans 20 / 2, in by 0x40 under 0xC0 for 10) / 0x1E03120 (`b`: radius
/// 700, white / grey (0xC0 / 0xF0 x3) outer, spans 20 / 5, in for 7) (D,
/// task): a spark ring at the point (cell 4, random angles) widening by 8
/// a update, its outer colour by the update's parity; then down by 8.
fn spark(fx: &mut Fx, t: u32, b: bool) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, if b { 0x442F_0000 } else { 0x4120_0000 });
        fx.mem.write_u32(t + 0x4C, 0);
        let (c0, c1) = if b { (0xFF, 0x40) } else { (0x80, 0x10) };
        l2::rgb(fx, t + 0x60, c0, c0, c0);
        l2::rgb(fx, t + 0x64, c1, c1, c1);
        fx.mem.set16(t + 0x50, 0);
        fx.mem.set16(t + 0x5A, 0x14);
        fx.mem.set16(t + 0x5C, if b { 5 } else { 2 });
        fx.mem.set16(t + 0x56, 4);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let ras: [u32; 3] = if b { [0x01E0_319C, 0x01E0_31B8, 0x01E0_31D4] } else { [0x01E0_2FF8, 0x01E0_3014, 0x01E0_3030] };
        for (o, ra) in [0x20u32, 0x24, 0x28].into_iter().zip(ras) {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
    }
    let life = if b { 7 } else { 0xA };
    fade(fx, t, life, 0xC0, 0x40, 0xA, 8);
    let odd = fx.mem.i16(t + 0x54) % 2 == 1;
    match (b, odd) {
        (false, true) => l2::rgb(fx, t + 0x64, 0x10, 0x80, 0x10),
        (false, false) => l2::rgb(fx, t + 0x64, 0x20, 0xF0, 0x20),
        (true, true) => l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0xC0),
        (true, false) => l2::rgb(fx, t + 0x64, 0xF0, 0xF0, 0xF0),
    }
    let r = ee::add(fx.mem.f(t + 0x48), 8.0);
    fx.mem.setf(t + 0x48, r);
    age(fx, t);
}
