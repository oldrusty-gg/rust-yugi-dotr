//! BMODULE b593 (card 648), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E030B0, data area 0x1ED0000) [F] (helper A12, 31
//! Oct 2026): a pool of 150 tasks of 0x70 bytes at D+0x2A0 (alive +0x58,
//! step +0x44 through `jalr`; spawned by 0x1E03850, b494's spawn: the
//! colour D+0x45D4, the kind D+0x4636 into +0x50 and +0x52) drawn with
//! flat arcs (004542c6809a at 0x1E00ED0, `billboard::arc_strips_flat`),
//! rings of quads (7369422df7f8 at 0x1E00920, `billboard::ring_quads`),
//! sparks (21cb40d76bec at 0x1E01230, `quads4::sparks`) and quads
//! (add5d81dd3fe at 0x1E01630, `billboard::quad`), UVs by uv_rect. Two
//! points: the target's node 1 (+0x4480, 81..119, then drifting to height
//! 50 / z 0 by 140) and the attacker's node 0x6D (+0x4470, 190..254 two
//! higher, then drifting by 281); green (kind 0) at the attacker on
//! 190..286 and blue (kind 1) at the target on 81..286: a flat arc, a
//! ring and a glowing quad a update; the target tinted darker on 91..110
//! (0x1F76A0 / 0x1F75F0), restored on 289; 81..99 three closing rings at
//! the target; 281..295 rings and sparks at the attacker, a white arc on
//! 281, five rings on 291. The unit flash 0x1E039A0 (its flash step
//! 0x1E02E10) is never reached: no flash task is spawned [F]. Returns 1 on
//! update 75, 4 on 335, 2 on 325, 6 on 86..319, 8 once the 350-update
//! timer +0x45D0 has run out.
use crate::b015::unit_tint_saving;
use crate::b076::node_position_saving;
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const ARC: u32 = 0x01E0_1E80;
const RING: u32 = 0x01E0_20C0;
const RING_S: u32 = 0x01E0_2250;
const SPARKS: u32 = 0x01E0_2480;
const GLOW: u32 = 0x01E0_25B0;
const WHITE_ARC: u32 = 0x01E0_28B0;
const RING_G: u32 = 0x01E0_29F0;
const CLOSING: u32 = 0x01E0_2BD0;
/// Never spawned (the unit flash's step).
const FLASH: u32 = 0x01E0_2E10;
/// The return address of the entry's `jalr` into a step (0x1E03398 + 8).
const TASK_RA: u64 = 0x01E0_33A0;
const POS: u32 = D + 0x250;
const AT_SELF: u32 = D + 0x4470;
const AT_TARGET: u32 = D + 0x4480;
const TEX: u32 = D + 0x45B0;
const TIMER: u32 = D + 0x45D0;
const COLOUR: u32 = D + 0x45D4;
const TINT: u32 = D + 0x45DC;
const NODES: u32 = D + 0x4608;
const TARGET_NODE: u32 = D + 0x4618;
const KIND: u32 = D + 0x4636;
const FACING: u32 = D + 0x463A;
const COUNTER: u32 = D + 0x463E;
const BYTES: u32 = D + 0x4640;
const BYTES_B: u32 = D + 0x4740;
const SINES: u32 = D + 0x4840;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E030B0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    fx.mem.write_u32(D + 0x45E0, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xC0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x49B0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x463C, param);
    fx.mem.set32(TIMER, 0x15E);
    for (k, node) in [0x6D, 0x34, 0x35, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.fix_residue(y, 0x01E0_324C);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    // rand's frames: the entry's s0 = D.
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3274, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_32C4);
        fx.mem.set8(BYTES + k, v % 4);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3314, 0x28);
        fx.mem.set8(BYTES_B + k, v - 0x14);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    // The entry's s1: the flash tasks counted here (never any).
    let mut flashes = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 {
            continue;
        }
        if f == FLASH {
            flashes += 1;
            continue;
        }
        // The steps' prologues: the entry's s0 = D (and s1).
        if f == GLOW {
            let s = fx.enter(0x30);
            fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(flashes)), (0, u64::from(D))]);
            glow(fx, t);
            fx.leave(0x30);
            continue;
        }
        let s = fx.enter(0x20);
        fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]);
        match f {
            ARC => arc(fx, t, flashes),
            RING => ring(fx, t),
            RING_S => ring_s(fx, t),
            SPARKS => sparks(fx, t),
            WHITE_ARC => white_arc(fx, t, flashes),
            RING_G => ring_g(fx, t),
            CLOSING => closing(fx, t),
            _ => panic!("b593: unknown task step {f:#x}"),
        }
        fx.leave(0x20);
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        if f(fx) == ARC {
            fx.mem.set16(D + 0xB8, 0x18);
            let kind = fx.mem.i16(t + 0x50);
            let pos = D.wrapping_add((kind << 4) as u32) + 0x4470;
            billboard::arc_strips_flat(fx, D, t, 0x21, pos);
            fx.mem.set16(D + 0xB8, 0);
        }
        for (g, local, rows, hold) in [(RING, 0x70u32, 4, 1), (RING_S, 0x80, 4, 0)] {
            if f(fx) != g {
                continue;
            }
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + local);
            fx.mem.set16(D + 0xB8, rows);
            fx.mem.set16(D + 0xB0, hold);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if f(fx) == SPARKS {
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x1F), (0x9A, 0x7F), (0xBE, 0), (0xB8, 0)] {
                fx.mem.set16(D + o, v);
            }
            for k in 0..3 {
                let v = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, v);
            }
            fx.mem.set16(D + 0xB8, 0x18);
            quads4::sparks(fx, D, t, 0x20, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == GLOW {
            let w = fx.mem.read_u32(TEX + 8);
            fx.mem.write_u32(D + 0x88, w);
            let s = fx.mem.i16(t + 0x56);
            fx.mem.set16(D + 0x94, s << 6);
            fx.mem.set16(D + 0x96, 0);
            fx.mem.set16(D + 0x98, 0x3F);
            fx.mem.set16(D + 0x9A, 0x3F);
            let v = rmod(fx, D, 0x01E0_35F4, 5);
            fx.mem.set16(D + 0xBE, v);
            for k in 0..3 {
                let v = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(sp + 0x60 + 4 * k, v);
            }
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, sp + 0x60);
        }
        if f(fx) == WHITE_ARC {
            fx.mem.set16(D + 0xB8, 0x18);
            billboard::arc_strips_flat(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        for (g, local, rows) in [(RING_G, 0x90u32, 7), (CLOSING, 0xA0, 4)] {
            if f(fx) != g {
                continue;
            }
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + local);
            fx.mem.set16(D + 0xB8, rows);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
    }
    debug_assert_eq!(flashes, 0);
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x4B => 1,
        0x14F => 4,
        0x145 => 2,
        0x56..=0x13F => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell 0x14 frame +0x56 (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32) {
    let sub = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, 0x14, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// 0x1E03850(D, step, 0) (b494's 0x1E01940): the first free task cleared
/// and set off at D+0x250 with the colour D+0x45D4, the kind D+0x4636 at
/// +0x50 and +0x52; `saves` the caller's s0..s4, `ra` the return address.
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
        for o in [0x50u32, 0x52] {
            let s = fx.mem.i16(KIND);
            fx.mem.set16(t + o, s);
        }
        fx.mem.write_u32(t + 0x40, 0);
        fx.mem.write_u32(t + 0x44, step);
    }
    fx.leave(0x60);
}

/// 0x1F76A0(h, out) [F]: `out` = the unit's tint (obj+0x12C..+0x12F, 0x80
/// each when it has none), through 0x1F76D0 (its frame: the caller's s0,
/// ra 0x1F76B0) and 0x1F3870; `ra` 0x1F76A0's return address.
fn unit_tint_get(fx: &mut Fx, h: i32, out: u32, s0: u64, ra: u32) {
    for k in 0..4 {
        fx.mem.write_u8(out + k, 0x80);
    }
    let (side, slot) = (h >> 8, h & 0xFF);
    if (0..3).contains(&side) && (0..0xB).contains(&slot) {
        let i = fx.object_index(side, slot);
        if (0..0x54).contains(&i) {
            let o = fx.mem.read_u32(crate::world::OBJECT_TABLE + 4 * i as u32);
            if o != 0 {
                for k in 0..4u32 {
                    let v = fx.mem.read_u8(o + 0x12C + k);
                    fx.mem.write_u8(out + k, v);
                }
            }
        }
    }
    let f = fx.sp - 0x10;
    fx.saved(f, &[(0, u64::from(ra))]);
    fx.saved(f - 0x20, &[(0, s0), (0x10, 0x001F_76B0)]);
}

/// 0x1E018F0(D, 0): the update counter, the two points and the spawns
/// (frame 0x30: ra, the entry's s1 = 0, s0 = D; then s0 = D, s1 a loop
/// count). The spawns' saves: s0 = D, s1, the entry's s2 = 0, s3 = the
/// parameter, s4 the runner's (0 [H]).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_3360), (0x10, 0), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let p = param as i64 as u64;
    let saves = |_: &mut Fx, s1: u64| [u64::from(D), s1, 0, p, 0];
    let go = |fx: &mut Fx, f: u32, s1: u64, ra: u32| {
        let s = saves(fx, s1);
        spawn(fx, f, s, ra);
    };
    let to_pos = |fx: &mut Fx, from: u32| {
        for k in 0..4 {
            let w = fx.mem.read_u32(from + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
    };
    if (0x51..0x78).contains(&c(fx)) {
        let h = fx.target_handle();
        let s = fx.mem.i16(TARGET_NODE);
        node_position_saving(fx, h, s, AT_TARGET, u64::from(D), Some(0), 0x01E0_1940);
        let h = fx.target_handle();
        unit_tint_get(fx, h, TINT, u64::from(D), 0x01E0_1954);
        if (0x5B..0x6E).contains(&c(fx)) {
            let a = fx.mem.read_u8(TINT + 3).wrapping_sub(6);
            fx.mem.write_u8(TINT + 3, a);
        }
        if c(fx) == 0x6F {
            fx.mem.write_u8(TINT + 3, 0);
        }
        let h = fx.target_handle();
        unit_tint_saving(fx, h, TINT, [u64::from(D), 0], 0x01E0_19A4);
    }
    if c(fx) >= 0x78 {
        if c(fx) == 0x78 {
            let v = ee::div(ee::neg(fx.mem.f(AT_TARGET + 8)), 30.0);
            fx.mem.setf(D + 0x4528, v);
            let v = ee::div(ee::sub(50.0, fx.mem.f(AT_TARGET + 4)), 20.0);
            fx.mem.setf(D + 0x4524, v);
        }
        if c(fx) < 0x8D {
            let v = ee::add(fx.mem.f(AT_TARGET + 8), fx.mem.f(D + 0x4528));
            fx.mem.setf(AT_TARGET + 8, v);
            let v = ee::add(fx.mem.f(AT_TARGET + 4), fx.mem.f(D + 0x4524));
            fx.mem.setf(AT_TARGET + 4, v);
        }
    }
    if (0xBE..0xFF).contains(&c(fx)) {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES);
        node_position_saving(fx, h, s, AT_SELF, u64::from(D), Some(0), 0x01E0_1A84);
        let y = ee::add(fx.mem.f(AT_SELF + 4), 2.0);
        fx.mem.setf(AT_SELF + 4, y);
    }
    if c(fx) >= 0xFF {
        if c(fx) == 0xFF {
            let v = ee::div(ee::neg(fx.mem.f(AT_SELF + 8)), 20.0);
            fx.mem.setf(D + 0x4518, v);
            let v = ee::div(ee::neg(ee::sub(50.0, fx.mem.f(AT_SELF + 4))), 50.0);
            fx.mem.setf(D + 0x4514, v);
        }
        if c(fx) < 0x11A {
            let v = ee::add(fx.mem.f(AT_SELF + 8), fx.mem.f(D + 0x4518));
            fx.mem.setf(AT_SELF + 8, v);
            let v = ee::sub(fx.mem.f(AT_SELF + 4), fx.mem.f(D + 0x4514));
            fx.mem.setf(AT_SELF + 4, v);
        }
    }
    if (0xBE..0x11F).contains(&c(fx)) {
        fx.mem.set16(KIND, 0);
        if c(fx) == 0xBE {
            go(fx, GLOW, 0, 0x01E0_1B88);
        }
        go(fx, ARC, 0, 0x01E0_1B9C);
        to_pos(fx, AT_SELF);
        go(fx, RING_S, 0, 0x01E0_1BD0);
    }
    if (0x51..0x11F).contains(&c(fx)) {
        fx.mem.set16(KIND, 1);
        if c(fx) == 0x6F {
            go(fx, GLOW, 0, 0x01E0_1C18);
        }
        go(fx, ARC, 0, 0x01E0_1C2C);
        to_pos(fx, AT_TARGET);
        go(fx, RING_S, 0, 0x01E0_1C60);
    }
    if (0x51..0x64).contains(&c(fx)) {
        fx.mem.set16(KIND, 1);
        to_pos(fx, AT_TARGET);
        for k in 0..3u64 {
            go(fx, CLOSING, k, 0x01E0_1CC0);
        }
    }
    if (0x119..0x128).contains(&c(fx)) {
        to_pos(fx, AT_SELF);
        if c(fx) % 2 == 0 {
            go(fx, SPARKS, 0, 0x01E0_1D58);
        }
        go(fx, RING, 0, 0x01E0_1D6C);
    }
    if c(fx) == 0x119 {
        to_pos(fx, AT_SELF);
        go(fx, WHITE_ARC, 0, 0x01E0_1DB0);
    }
    if c(fx) == 0x121 {
        let h = fx.target_handle();
        unit_tint_get(fx, h, TINT, u64::from(D), 0x01E0_1DD4);
        fx.mem.write_u8(TINT + 3, 0x80);
        let h = fx.target_handle();
        unit_tint_saving(fx, h, TINT, [u64::from(D), 0], 0x01E0_1DEC);
    }
    if c(fx) == 0x123 {
        to_pos(fx, AT_SELF);
        for k in 0..5u64 {
            go(fx, RING_G, k, 0x01E0_1E38);
        }
    }
    fx.leave(0x30);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// Byte +0x63 set to 0x80 for the life +6, then down by 11 while at least
/// 13 (else the task ends).
fn hold(fx: &mut Fx, t: u32, floor: u8, down: u8) {
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= floor {
            fx.mem.write_u8(t + 0x63, a - down);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
}

/// Byte +0x63 up by `up` under `cap` for `life` updates, then down by 11
/// while at least 13 (else the task ends).
fn fade(fx: &mut Fx, t: u32, life: i32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < life {
        if a < cap {
            fx.mem.write_u8(t + 0x63, a + up);
        }
    } else if a >= 0xD {
        fx.mem.write_u8(t + 0x63, a - 0xB);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

fn odd(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x54) % 2 == 1
}

/// 0x1E01E80 (D, task): a flat arc (radius 20..99 growing by 1, spans 15
/// / 15; grey, the outer green (kind 0) / blue (kind 1) flickering) on the
/// kind's point, facing the camera turned a quarter; in by 0x30 under
/// 0x60 for 2 updates, out by 11. `flashes` the entry's s1.
fn arc(fx: &mut Fx, t: u32, flashes: u32) {
    let kind = fx.mem.i16(t + 0x50);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_1EA4, 0x50);
        fx.mem.setf(t + 0x48, ee::add(20.0, ee::from_i32(v)));
        fx.mem.write_u32(t + 0x4C, 0);
        if kind == 0 {
            l2::rgb(fx, t + 0x60, 0x48, 0x48, 0x48);
            l2::rgb(fx, t + 0x64, 0x40, 0x80, 0x40);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0x48, 0x48, 0x48);
            l2::rgb(fx, t + 0x64, 0x40, 0x40, 0x80);
        }
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    fade(fx, t, 2, 0x60, 0x30);
    if fx.mem.i16(t + 0x50) == 0 {
        let g = if odd(fx, t) { 0x80 } else { 0x40 };
        l2::rgb(fx, t + 0x64, 0x20, g, 0x20);
    }
    if fx.mem.i16(t + 0x50) == 1 {
        let b = if odd(fx, t) { 0x80 } else { 0x40 };
        l2::rgb(fx, t + 0x64, 0x20, 0x20, b);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = ee::add(fx.mem.f(t + 0x48), 1.0);
        fx.mem.setf(t + 0x48, r);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(flashes), 0x01E0_2094);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
}

/// 0x1E020C0 (D, task): a grey ring of quads (size 20, cell 5, a random
/// angle, +0xA rand % 8, +0xC the kind, radius 10..59 growing by 16..45,
/// row 0..34, life 3..7); 0x80 during the life, then down by 8.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        fx.mem.set16(t + 0x56, 5);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, 0x01E0_2108, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, 0x01E0_2124);
        fx.mem.set16(t + 0xA, v % 8);
        let k = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, k);
        let v = rmod(fx, t, 0x01E0_2148, 0x32);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, 0x01E0_2168, 0x1E);
        fx.mem.set16(t + 0xE, v + 0x10);
        let v = rmod(fx, t, 0x01E0_2188, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_21A4, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    hold(fx, t, 0xA, 8);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// Random angles +0x20 / +0x24 / +0x28 (rand % 0xFFFF) from `ras`.
fn angles(fx: &mut Fx, t: u32, ras: [u32; 3]) {
    for (o, ra) in [0x20u32, 0x24, 0x28].into_iter().zip(ras) {
        let v = rmod(fx, t, ra, 0xFFFF);
        fx.mem.set32(t + o, v);
    }
}

/// 0x1E02250 (D, task): a small ring of quads (size 5..12 growing by 1,
/// pink (+0x52 0) / blue (1), random angles, radius 10..39 growing by
/// 1..3, row 1..8, life 2..4); 0x80 during the life, then down by 11.
fn ring_s(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = fx.rand_from(t, 0x01E0_2274);
        fx.mem.setf(t + 0x48, ee::from_i32(v % 8 + 5));
        let v = fx.rand_from(t, 0x01E0_22A0);
        fx.mem.set16(t + 0x56, v % 4);
        if fx.mem.i16(t + 0x52) == 0 {
            l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        }
        if fx.mem.i16(t + 0x52) == 1 {
            l2::rgb(fx, t + 0x60, 0x10, 0x10, 0xFF);
        }
        angles(fx, t, [0x01E0_2304, 0x01E0_2320, 0x01E0_233C]);
        let v = rmod(fx, t, 0x01E0_2358, 0x1E);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, 0x01E0_2378, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_2394, 3);
        fx.mem.set16(t + 0xE, v + 1);
        let v = fx.rand_from(t, 0x01E0_23B4);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, t, 0x01E0_23D4, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    hold(fx, t, 0xD, 0xB);
    let r = ee::add(fx.mem.f(t + 0x48), 1.0);
    fx.mem.setf(t + 0x48, r);
    let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, r);
    age(fx, t);
}

/// 0x1E02480 (D, task): a dim spark ring (inner radius 200 widening by
/// 20, the outer left 0; spans 15 / 15, turned a quarter, turning +0x24
/// by 0x100); 0x80 for 8 updates, then down by 14.
fn sparks(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4348_0000);
        fx.mem.setf(t + 0x4C, ee::sub(200.0, 58.0));
        l2::rgb(fx, t + 0x60, 0x40, 0x40, 0x40);
        l2::rgb(fx, t + 0x64, 0x40, 0x40, 0x40);
        fx.mem.set16(t + 0x50, 0);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.write_u32(t + 0x34, 0x4348_0000);
        let q = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, q);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.write_u32(t + 0x20, 0x4000);
    }
    if fx.mem.i16(t + 0x54) < 8 {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0x10 {
            fx.mem.write_u8(t + 0x63, a - 0xE);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let r = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::add(100.0, r));
    fx.mem.write_u32(t + 0x4C, 0);
    let a = fx.mem.read_u32(t + 0x24).wrapping_add(0x100);
    fx.mem.write_u32(t + 0x24, a);
    age(fx, t);
}

/// 0x1E025B0 (D, task): a glowing quad of size 45 on the kind's point
/// (pink / blue, flickering green; cell 1 / 0 by the update's parity, a
/// random angle), turned by the camera's angles; both alphas in by 9
/// under 0xC0 until the update 280, then +0x63 down by 14 twice a update.
fn glow(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x2D);
        fx.mem.set16(t + 0xA, 0x2D);
        let v = fx.rand_from(t, 0x01E0_25E4);
        fx.mem.set16(t + 0x56, v % 8);
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
            l2::rgb(fx, t + 0x64, 0xFF, 0x80, 0x80);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xFF);
            l2::rgb(fx, t + 0x64, 0x80, 0x80, 0xFF);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.set16(t + 0x5C, 0);
    }
    if fx.mem.i16(COUNTER) < 0x118 {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0xC0 {
                fx.mem.write_u8(t + o, a + 9);
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
    for kind in [0, 1] {
        if fx.mem.i16(t + 0x50) != kind {
            continue;
        }
        let (r, g, b) = match (kind, odd(fx, t)) {
            (0, true) => (0x20, 0xFF, 0x20),
            (1, true) => (0x20, 0x20, 0xFF),
            _ => (0x10, 0x80, 0x10),
        };
        l2::rgb(fx, t + 0x60, r, g, b);
        l2::rgb(fx, t + 0x64, r, g, b);
    }
    let c = i32::from(odd(fx, t));
    fx.mem.set16(t + 0x56, c);
    let v = rmod(fx, t, 0x01E0_2840, 0x168);
    fx.mem.set16(t + 0x5A, v);
    let kind = fx.mem.i16(t + 0x50);
    let at = D.wrapping_add((kind << 4) as u32) + 0x4470;
    for k in 0..4 {
        let w = fx.mem.read_u32(at + 4 * k);
        fx.mem.write_u32(t + 0x10 + 4 * k, w);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2888);
    age(fx, t);
}

/// 0x1E028B0 (D, task): a white flat arc (radius 300, a grey-blue outer
/// flickering, spans 15 / 15) facing the camera turned a quarter; in by
/// 0x60 under 0xC0 for 14 updates, out by 11. `flashes` the entry's s1.
fn white_arc(fx: &mut Fx, t: u32, flashes: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4396_0000);
        fx.mem.write_u32(t + 0x4C, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x40, 0x80, 0x80);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    fade(fx, t, 0xE, 0xC0, 0x60);
    let v = if odd(fx, t) { 0x80 } else { 0x40 };
    l2::rgb(fx, t + 0x64, 0x20, v, v);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(flashes), 0x01E0_29C4);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
}

/// 0x1E029F0 (D, task): a grey ring of quads (size 15..24 growing by 1,
/// cell 4, random angles, radius 10..39 growing by 5..14, row 1..8, life
/// 10..17); 0x80 during the life, then down by 11.
fn ring_g(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_2A14, 0xA);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0xF));
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        angles(fx, t, [0x01E0_2A58, 0x01E0_2A74, 0x01E0_2A90]);
        let v = rmod(fx, t, 0x01E0_2AAC, 0x1E);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, 0x01E0_2ACC, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_2AE8, 0xA);
        fx.mem.set16(t + 0xE, v + 5);
        let v = fx.rand_from(t, 0x01E0_2B08);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = fx.rand_from(t, 0x01E0_2B28);
        fx.mem.set16(t + 6, v % 8 + 0xA);
        fx.mem.set16(t + 0x58, 2);
    }
    hold(fx, t, 0xD, 0xB);
    let r = ee::add(fx.mem.f(t + 0x48), 1.0);
    fx.mem.setf(t + 0x48, r);
    let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, r);
    age(fx, t);
}

/// 0x1E02BD0 (D, task): a closing ring of quads (size 15..34 growing by
/// 1, pink (+0x52 0) / blue (1), random angles, radius 50..249 closing by
/// 15..34 while positive, row 1..8, life 2..4); 0x80 during the life,
/// then down by 11.
fn closing(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_2BF4, 0x14);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0xF));
        let v = fx.rand_from(t, 0x01E0_2C20);
        fx.mem.set16(t + 0x56, v % 4);
        if fx.mem.i16(t + 0x52) == 0 {
            l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        }
        if fx.mem.i16(t + 0x52) == 1 {
            l2::rgb(fx, t + 0x60, 0x40, 0x40, 0xFF);
        }
        angles(fx, t, [0x01E0_2C84, 0x01E0_2CA0, 0x01E0_2CBC]);
        let v = rmod(fx, t, 0x01E0_2CD8, 0xC8);
        fx.mem.set16(t + 0x50, v + 0x32);
        let v = rmod(fx, t, 0x01E0_2CF8, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_2D14, 0x14);
        fx.mem.set16(t + 0xE, v + 0xF);
        let v = fx.rand_from(t, 0x01E0_2D34);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, t, 0x01E0_2D54, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    hold(fx, t, 0xD, 0xB);
    let r = ee::add(fx.mem.f(t + 0x48), 1.0);
    fx.mem.setf(t + 0x48, r);
    let r = fx.mem.i16(t + 0x50);
    if r > 0 {
        let e = fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r - e);
    }
    age(fx, t);
}
