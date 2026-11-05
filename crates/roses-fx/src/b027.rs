//! BMODULE b027 (card 625), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E035A0, data area 0x1ED0000) [F]: 210 tasks of 0x40 bytes at D+0x2C0 (alive +0x36, step +0x24),
//! spawned by the record's 0x1E03E30 (colour D+0x38D4 twice, the point
//! D+0x250, +0x30 = D+0x392E); timer D+0x38D0 = 0xD7, counter D+0x3932,
//! nodes 0x12 (clear) / 0x14 / 8 / 0xD / 7 / 0xC at D+0x3916.., two
//! ten-point histories D+0x3770 / D+0x3810 of nodes 7 / 0xC (filled on
//! update 0x48, pushed by `spawn::node_history`), rand bytes D+0x3934.
//!
//! The emitter 0x1E00080 counts D+0x3932 and keeps the attacker's yaw /
//! 180 - 180 at D+0x3914: on 0xB / 0x28 twenty motes 0x1E00900 at node
//! 0x12 (y - 10), on 0x1D / 0x31 twenty more at node 0x14, 0x3F..0x62 three
//! drifting motes 0x1E00B20 an update at node 0x14; on eleven updates
//! 0x77..0xAE a burst at node 8 and on ten 0x7A..0xAB one at node 0xD
//! (five sparks 0x1E00DB0, four flame bands 0x1E01310, ten `ring_t`
//! sparkles 0x1E014A0, a `fan18_far0` disc 0x1E016D0); 0x48 two ribbons
//! 0x1E01600 (over the histories); 0x5E a window flash 0x1E017A0. Steps
//! 0x1E00F90 / 0x10D0 / 0x1220 are tested but never spawned (not ported).
//! Draws: the record's sprites 0x1E03F50 / 0x40F0 (header, then quad) and
//! ribbon 0x1E03030, `band9_tex`, `ring_t`, `fan18_far0`. Returns 1 on
//! update 0x6E, 4 on 0xB9, 6 on 0x79..0xA9, 2 on 0xAF, 8 once the timer
//! has run out. No 0x1F8100.
use crate::blib::{billboard, quads4, spawn as bspawn, spline, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;

const MOTE: u32 = 0x01E0_0900;
const DRIFT: u32 = 0x01E0_0B20;
const SPARK: u32 = 0x01E0_0DB0;
const FLAME: u32 = 0x01E0_1310;
const RING: u32 = 0x01E0_14A0;
const RIBBON: u32 = 0x01E0_1600;
const DISC: u32 = 0x01E0_16D0;
const FLASH: u32 = 0x01E0_17A0;

/// The run loop's compare chain (live steps) with each call's return.
const RUN: [(u32, u32); 8] = [
    (MOTE, 0x01E0_3810),
    (DRIFT, 0x01E0_3830),
    (SPARK, 0x01E0_3850),
    (FLAME, 0x01E0_38D0),
    (RING, 0x01E0_38F0),
    (RIBBON, 0x01E0_3910),
    (DISC, 0x01E0_3930),
    (FLASH, 0x01E0_3950),
];

const POOL: u32 = D + 0x2C0;
const POS: u32 = D + 0x250;
const TEX_F: u32 = D + 0x2BC;
const HIST_A: u32 = D + 0x3770;
const HIST_B: u32 = D + 0x3810;
const TEX5: u32 = D + 0x38B0;
const TIMER: u32 = D + 0x38D0;
const COLOUR: u32 = D + 0x38D4;
const OT2: u32 = D + 0x38DC;
const YAW: u32 = D + 0x3914;
const NODES: u32 = D + 0x3916;
const ANGLE: u32 = D + 0x3928;
const PARAM: u32 = D + 0x392E;
const PARAM_COPY: u32 = D + 0x3930;
const COUNTER: u32 = D + 0x3932;
const BYTES: u32 = D + 0x3934;
const SINES: u32 = D + 0x3A34;

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

/// The entry 0x1E035A0 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1E0);
    fx.mem.write_u32(OT2, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x1E0);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3BA0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(PARAM_COPY, param);
    fx.mem.set32(TIMER, 0xD7);
    for (k, (n, clear)) in [(0x12, 1), (0x14, 0), (8, 0), (0xD, 0), (7, 0), (0xC, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(TEX_F, t);
    let t = fx.texture(5, 0);
    fx.mem.write_u32(TEX5, t);
    // rand's frame: s0 = D.
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_36D0);
        let r = fx.div(v, 0xA).1;
        fx.mem.write_u8(BYTES + k, (r - 5) as u8);
    }
    0
}

/// A callee's ra slot (`sq ra`) at the caller's sp - 0x10 (0x1F6830 /
/// 0x1F71A0 / 0x1F7370), the upper half 0 [H, as `Fx::saved`].
fn ra_slot(fx: &mut Fx, ra: u32) {
    let a = fx.sp.wrapping_sub(0x10);
    fx.mem.write_u64(a, u64::from(ra));
    fx.mem.write_u64(a + 8, 0);
}

/// Node slot `k` (D+0x3916 + 2 k) of the attacker into `out` (0x1F6830
/// from `ra`).
fn node_at(fx: &mut Fx, k: u32, out: u32, ra: u32) {
    let h = fx.self_handle();
    let n = fx.mem.i16(NODES + 2 * k);
    ra_slot(fx, ra);
    fx.node_position(h, n, out);
}

/// 0x1A2B80 of `v` from `ra`.
fn fix(fx: &mut Fx, v: f32, ra: u32) -> i32 {
    fx.fix_residue(v, ra);
    ee::fixsfsi(v)
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    // The entry's s2: the parameter, or the second fill's end pointer.
    let mut s2 = param as i64 as u64;
    for (k, hist, buf, ra) in [(4u32, HIST_A, sp + 0xA0, 0x01E0_3728u32), (5, HIST_B, sp + 0xB0, 0x01E0_3780)] {
        if counter(fx) == 0x48 {
            node_at(fx, k, buf, ra);
            for i in 0..10 {
                for c in 0..3 {
                    let w = fx.mem.read_u32(buf + 4 * c);
                    fx.mem.write_u32(hist + 0x10 * i + 4 * c, w);
                }
            }
            s2 = u64::from(hist + 0xA0);
        }
    }
    let n = fx.mem.i16(NODES + 8);
    bspawn::node_history(fx, HIST_A + 0x90, 0xA, n);
    let n = fx.mem.i16(NODES + 0xA);
    bspawn::node_history(fx, HIST_B + 0x90, 0xA, n);
    emit(fx, s2);
    for k in 0..210u32 {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        for (at, ra) in RUN {
            if fx.mem.read_u32(t + 0x24) == at {
                run_step(fx, t, at, ra);
            }
        }
    }
    ra_slot(fx, 0x01E0_3974);
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x60);
    let z = ee::sub(fx.mem.f(sp + 0x68), 25.0);
    fx.mem.setf(sp + 0x68, z);
    let d = sp + 0x150;
    let mut s1 = 0u32;
    for k in 0..210u32 {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        s1 += 1;
        let r = Regs { s1, s2: D + 0x40 * k, s3: k };
        match fx.mem.read_u32(t + 0x24) {
            MOTE | DRIFT => {
                let ras = if fx.mem.read_u32(t + 0x24) == MOTE { [0x01E0_39C4u32, 0x01E0_39D8] } else { [0x01E0_3A00, 0x01E0_3A14] };
                sprite(fx, t, d, false, ras[0], &r);
                sprite(fx, t, d, true, ras[1], &r);
            }
            SPARK => {
                spark_sprite(fx, t, d, false, 0x01E0_3A80, &r);
                spark_sprite(fx, t, d, true, 0x01E0_3A94, &r);
            }
            FLAME => {
                cell(fx, t, sp + 0x70, 0x9B);
                fx.mem.set16(D + 0xB8, 1);
                quads4::band9_tex(fx, D, t, d);
                fx.mem.set16(D + 0xB8, 0);
            }
            RING => {
                cell(fx, t, sp + 0x80, 0x14);
                fx.mem.set16(D + 0xB8, 2);
                billboard::ring_t(fx, uv::uv_rect, D, t, d, BYTES);
                fx.mem.set16(D + 0xB8, 0);
            }
            RIBBON => {
                cell(fx, t, sp + 0x90, 0x14);
                fx.mem.set16(D + 0xBE, 0);
                for (o, v) in [(0x94u32, 0xF), (0x96, 0x10), (0x98, 0xF), (0x9A, 1)] {
                    fx.mem.set16(D + o, v);
                }
                let p = HIST_A.wrapping_add((0xA0 * fx.mem.i16(t + 0x30)) as u32);
                ribbon_draw(fx, t, p, &r);
            }
            DISC => quads4::fan18_far0(fx, D, t, d),
            _ => {}
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0x6E => 1,
        0xB9 => 4,
        c if (0x79..0xAA).contains(&c) => 6,
        0xAF => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The entry's registers at a draw call (s0 = D).
#[derive(Clone, Copy)]
struct Regs {
    s1: u32,
    s2: u32,
    s3: u32,
}

/// The texture (5, 0) and the atlas cell `id` frame +0x34 (0x106960 into
/// `r`) copied to D+0x88 / D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32, id: i32) {
    let v = fx.mem.read_u32(TEX5);
    fx.mem.write_u32(D + 0x88, v);
    let f = fx.mem.i16(t + 0x34);
    l2::rect(fx, r, id, f);
    for k in 0..4 {
        let w = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, w);
    }
}

/// A step's frame with its prologue saves (the run loop's s0 = D, s1 = 0).
fn run_step(fx: &mut Fx, t: u32, at: u32, ra: u32) {
    let size = match at {
        MOTE | DRIFT | SPARK | FLAME => 0x30,
        FLASH => 0x80,
        _ => 0x20,
    };
    let sp = fx.enter(size);
    let (ra, d) = (u64::from(ra), u64::from(D));
    match size {
        0x30 => fx.saved(sp, &[(0x20, ra), (0x10, 0), (0, d)]),
        _ => fx.saved(sp, &[(0x10, ra), (0, d)]),
    }
    match at {
        MOTE => mote(fx, t),
        DRIFT => drift(fx, t),
        SPARK => spark(fx, t),
        FLAME => flame(fx, t),
        RING => ring(fx, t),
        RIBBON => ribbon(fx, t),
        DISC => disc(fx, t),
        _ => flash(fx, t, sp),
    }
    fx.leave(size);
}

fn age(fx: &mut Fx, t: u32) -> i32 {
    fx.mem.i16(t + 0x32)
}

fn grow(fx: &mut Fx, t: u32) {
    let n = age(fx, t) + 1;
    fx.mem.set16(t + 0x32, n);
}

fn first(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x36) == 1
}

/// Alpha +0x2B down by `down` while at least `floor`, else the task ends.
fn fall(fx: &mut Fx, t: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a >= floor {
        fx.mem.write_u8(t + 0x2B, a - down);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// Alpha +0x2B up by `up` while below `below`.
fn rise(fx: &mut Fx, t: u32, below: u8, up: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a < below {
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(up));
    }
}

/// rand % m (HI) with rand's saved s0 the task.
fn rand_rem(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v, m).1
}

/// i16 `o` = fix(i16 `o` + `k` sin / cos (`cos`) of the i16 angle at `a`)
/// (0x108910, 0x1A2B80 from `ra`).
fn wobble(fx: &mut Fx, t: u32, o: u32, a: u32, cos: i32, k: f32, ra: u32) {
    let deg = fx.mem.i16(t + a);
    let s = l2::sin_deg(fx, SINES, deg, cos);
    let m = ee::mul(k, s);
    let v = ee::add(ee::from_i32(fx.mem.i16(t + o)), m);
    let r = fix(fx, v, ra);
    fx.mem.set16(t + o, r);
}

/// The motes' frame +0x34 up to 7 when the age % 3 is 1 (a `div`).
fn third_frame(fx: &mut Fx, t: u32) {
    let a = age(fx, t);
    if fx.div(a, 3).1 == 1 && fx.mem.i16(t + 0x34) < 7 {
        let v = fx.mem.i16(t + 0x34) + 1;
        fx.mem.set16(t + 0x34, v);
    }
}

/// 0x1E00900: a mote ((0x3F, 0x38, 0x3F), size 0 up by 1 on odd ages to
/// 0x12, angle rand % 360, phase +0x30 rand % 0x1E - 0xF): alpha up by 9
/// (below 0x60) for 10 updates, then down by 10 (ends below 0x11); it
/// sways x by 10 sin, y by 4 cos(+0x30), z by 10 cos of its angle.
fn mote(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x3F, 0x38, 0x3F);
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0);
        let r = rand_rem(fx, t, 0x01E0_0944, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x34, 0);
        let r = rand_rem(fx, t, 0x01E0_0964, 0x1E);
        fx.mem.set16(t + 0x30, r - 0xF);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 0xA {
        rise(fx, t, 0x60, 9);
    } else {
        fall(fx, t, 0x11, 0xA);
    }
    grow(fx, t);
    if age(fx, t).wrapping_rem(2) == 1 && fx.mem.i16(t + 0x38) < 0x12 {
        let v = fx.mem.i16(t + 0x38) + 1;
        fx.mem.set16(t + 0x38, v);
    }
    third_frame(fx, t);
    wobble(fx, t, 8, 0x3A, 0, 10.0, 0x01E0_0A8C);
    wobble(fx, t, 0xA, 0x30, 1, 4.0, 0x01E0_0AC4);
    wobble(fx, t, 0xC, 0x3A, 1, 10.0, 0x01E0_0AFC);
}

/// 0x1E00B20: a drifting mote ((0x3F, 0x38, 0x3F), size from 0 up by 1 an
/// update, angle rand % 360 before update 0x82, then the yaw D+0x3914 +
/// rand % 0x78 - 0x3C, +0x30 rand % 0x3C - 0x1E): alpha up by 9 (below
/// 0x60) for 10 updates, then down by 10 (ends below 0x11); x / z by 20
/// sin / cos of its angle, y up by rand % 5 after update 0.
fn drift(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x3F, 0x38, 0x3F);
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0);
        if counter(fx) < 0x82 {
            let r = rand_rem(fx, t, 0x01E0_0B78, 0x168);
            fx.mem.set16(t + 0x3A, r);
        } else {
            let r = rand_rem(fx, t, 0x01E0_0B9C, 0x78);
            let v = fx.mem.i16(YAW) + (r - 0x3C);
            fx.mem.set16(t + 0x3A, v);
        }
        fx.mem.set16(t + 0x34, 0);
        let r = rand_rem(fx, t, 0x01E0_0BC8, 0x3C);
        fx.mem.set16(t + 0x30, r - 0x1E);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 0xA {
        rise(fx, t, 0x60, 9);
    } else {
        fall(fx, t, 0x11, 0xA);
    }
    let v = fx.mem.i16(t + 0x38) + 1;
    fx.mem.set16(t + 0x38, v);
    third_frame(fx, t);
    wobble(fx, t, 8, 0x3A, 0, 20.0, 0x01E0_0CB4);
    let a = age(fx, t);
    if a != 0 {
        let ra = if a < 5 { 0x01E0_0CE0 } else { 0x01E0_0D14 };
        let r = rand_rem(fx, t, ra, 5) as i16 as i32;
        let v = fx.mem.i16(t + 0xA) + r;
        fx.mem.set16(t + 0xA, v);
    }
    wobble(fx, t, 0xC, 0x3A, 1, 20.0, 0x01E0_0D70);
    grow(fx, t);
}

/// 0x1E00DB0: a spark (white, size rand % 5 + 3, frame rand & 3, +0x30 /
/// angle rand % 360): alpha 0x80 for 10 updates, then down by 10 (ends
/// below 0x11); after update 0 x / z by 16 sin / cos of its angle, y by 16
/// cos(+0x30).
fn spark(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        let r = rand_rem(fx, t, 0x01E0_0DDC, 5);
        fx.mem.set16(t + 0x38, r + 3);
        let v = fx.rand_from(t, 0x01E0_0DFC);
        fx.mem.set16(t + 0x34, v.wrapping_rem(4));
        fx.rand_from(t, 0x01E0_0E18);
        l2::rgb(fx, t + 0x28, 0xFF, 0xFF, 0xFF);
        let r = rand_rem(fx, t, 0x01E0_0E34, 0x168);
        fx.mem.set16(t + 0x30, r);
        let r = rand_rem(fx, t, 0x01E0_0E50, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 0xA {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fall(fx, t, 0x11, 0xA);
    }
    if counter(fx) != 0 {
        wobble(fx, t, 8, 0x3A, 0, 16.0, 0x01E0_0EEC);
        wobble(fx, t, 0xA, 0x30, 1, 16.0, 0x01E0_0F24);
        wobble(fx, t, 0xC, 0x3A, 1, 16.0, 0x01E0_0F5C);
    }
    grow(fx, t);
}

/// 0x1E01310: a flame band (`band9_tex`: size -10 growing by +8 = rand % 3
/// + 4 after update 0, turn +0x10 rand % 0x4000 - 0x2000, +0x14 rand %
/// 0xFFFF, frame rand % 7, +6 = 0x14, grey 0x80): alpha up by 0x40 (below
/// 0x80) on ages 0 / 1; then from size 0x15 down by 0x13 (ends below
/// 0x15), and by 0xF (ends below 0x11) once the timer is under 0xA.
fn flame(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, -0xA);
        let v = fx.rand_from(t, 0x01E0_1340);
        fx.mem.set32(t + 0x10, v.wrapping_rem(0x4000) - 0x2000);
        let r = rand_rem(fx, t, 0x01E0_1360, 0xFFFF);
        fx.mem.set32(t + 0x14, r);
        let r = rand_rem(fx, t, 0x01E0_137C, 7);
        fx.mem.set16(t + 0x34, r);
        fx.mem.set16(t + 0x36, 2);
        let r = rand_rem(fx, t, 0x01E0_13A0, 3);
        fx.mem.set16(t + 8, r + 4);
        fx.mem.set16(t + 6, 0x14);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
    }
    if age(fx, t) < 2 {
        rise(fx, t, 0x80, 0x40);
    } else {
        if fx.mem.i16(t + 0x38) >= 0x15 {
            fall(fx, t, 0x15, 0x13);
        }
        if fx.mem.i32(TIMER) < 0xA {
            fall(fx, t, 0x11, 0xF);
        }
    }
    if age(fx, t) != 0 {
        let v = fx.mem.i16(t + 0x38).wrapping_add(fx.mem.i16(t + 8));
        fx.mem.set16(t + 0x38, v);
    }
    grow(fx, t);
}

/// 0x1E014A0: a sparkle ring (`ring_t`: size 0x14, frame 1, turn +0x18
/// rand % 0xFFFF, grey 0x80, radius +0x30 from 0 by +0xE = rand % 0x12 +
/// 0x82, angle rand % 360, +8 rand % 10, life +6 rand % 7 + 3): alpha up
/// by 0x40 (below 0x81) for the life, then down by 10 (ends below 0x11).
fn ring(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x14);
        fx.mem.set16(t + 0x34, 1);
        let r = rand_rem(fx, t, 0x01E0_14CC, 0xFFFF);
        fx.mem.set32(t + 0x18, r);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x30, 0);
        let r = rand_rem(fx, t, 0x01E0_14F8, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let r = rand_rem(fx, t, 0x01E0_1514, 0x12);
        fx.mem.set16(t + 0xE, r + 0x82);
        let r = rand_rem(fx, t, 0x01E0_1534, 0xA);
        fx.mem.set16(t + 8, r);
        let r = rand_rem(fx, t, 0x01E0_1550, 7);
        fx.mem.set16(t + 6, r + 3);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < fx.mem.i16(t + 6) {
        rise(fx, t, 0x81, 0x40);
    } else {
        fall(fx, t, 0x11, 0xA);
    }
    if age(fx, t) != 0 {
        let v = fx.mem.i16(t + 0x30).wrapping_add(fx.mem.i16(t + 0xE));
        fx.mem.set16(t + 0x30, v);
    }
    grow(fx, t);
}

/// 0x1E01600: a ribbon over history +0x30 ((0x20, 0x20, 0x80) / rim the
/// same, +6 = 6 points, frame 1, size 0x78): alpha 0x60 for 0xAF updates,
/// then down by 9 (ends below 0xB).
fn ribbon(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x80);
        l2::rgb(fx, t + 0x2C, 0x20, 0x20, 0x80);
        fx.mem.set16(t + 6, 6);
        fx.mem.write_u8(t + 0x2B, 0);
        fx.mem.set16(t + 0x34, 1);
        fx.mem.set16(t + 0x38, 0x78);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 0xAF {
        fx.mem.write_u8(t + 0x2B, 0x60);
    } else {
        fall(fx, t, 0xB, 9);
    }
    grow(fx, t);
}

/// 0x1E016D0: the burst disc (`fan18_far0`: size 0x140, (0xC0, 0xC0,
/// 0xC0) / rim (0x80, 0x80, 0x20), angle rand % 360): alpha 0xA0 on
/// update 0, then down by 0x30 (ends below 0x33).
fn disc(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x140);
        l2::rgb(fx, t + 0x28, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x2C, 0x80, 0x80, 0x20);
        let r = rand_rem(fx, t, 0x01E0_1724, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if age(fx, t) <= 0 {
        fx.mem.write_u8(t + 0x2B, 0xA0);
    } else {
        fall(fx, t, 0x33, 0x30);
    }
    grow(fx, t);
}

/// 0x1E017A0 (frame `sp`): b202's window sprite (0x6FF8 .. 0x8FF8)
/// sampling the frame (0x207EF0, filter 1) at ST 0..1, grey 0x80 (alpha
/// +0x2B), ALPHA 0x44, key 0; in by 0xC below 0x30 until age 0x5A, then
/// out by 0xC (ends below 0xE).
fn flash(fx: &mut Fx, t: u32, sp: u32) {
    let d = sp + 0x28;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for k in 0..3 {
        fx.mem.write_u8(sp + 0x38 + k, 0x80);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    fx.mem.write_u8(sp + 0x3B, a);
    fx.mem.write_u32(sp + 0x3C, 0x3F80_0000);
    for (at, v) in [(sp + 0x48, 0x6FF8u16), (sp + 0x58, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    for (o, v) in [(0x40u32, 0u32), (0x44, 0), (0x50, 0x3F80_0000), (0x54, 0x3F80_0000)] {
        fx.mem.write_u32(sp + o, v);
    }
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if age(fx, t) < 0x5A {
        rise(fx, t, 0x30, 0xC);
    } else {
        fall(fx, t, 0xE, 0xC);
    }
    grow(fx, t);
}

/// The record's spawn 0x1E03E30(D, step, 0) (frame 0x60, called from `ra`
/// with s0 = D): the first free of the 210 tasks cleared and set off at
/// D+0x250 (as i16) with the colour D+0x38D4 (twice), +0x30 = D+0x392E.
fn spawn(fx: &mut Fx, f: u32, ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0, u64::from(D))]);
    let ret = match (0..210u32).find(|&k| fx.mem.i16(POOL + 0x36 + 0x40 * k) == 0) {
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
            let p = fx.mem.i16(PARAM);
            fx.mem.set16(t + 0x30, p);
            fx.mem.write_u32(t + 0x20, 0);
            fx.mem.write_u32(t + 0x24, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// rand % 360 into D+0x3928 (rand's s0 = D).
fn angle(fx: &mut Fx, ra: u32) {
    let v = fx.rand_from(D, ra);
    let r = fx.div(v, 0x168).1;
    fx.mem.set16(ANGLE, r);
}

/// The spawn point = the node position at `p`, y less `dy`.
fn at(fx: &mut Fx, p: u32, dy: f32) {
    let x = fx.mem.f(p);
    fx.mem.setf(POS, x);
    let y = fx.mem.f(p + 4);
    fx.mem.setf(POS + 4, if dy == 0.0 { y } else { ee::sub(y, dy) });
    let z = fx.mem.f(p + 8);
    fx.mem.setf(POS + 8, z);
}

/// One burst at the node position `p`: five sparks and four flame bands
/// (angle rand % 360, +0x30 rand % 0x50), ten sparkles (+0x30 0) and a
/// disc; `b` the first rand's return address (the rest follow it).
fn burst(fx: &mut Fx, p: u32, b: u32) {
    for (f, n, o, so) in [(SPARK, 5, 0u32, 0x60u32), (FLAME, 4, 0x7C, 0xD4)] {
        for _ in 0..n {
            angle(fx, b + o);
            let v = fx.rand_from(D, b + o + 0x1C);
            let r = fx.div(v, 0x50).1;
            fx.mem.set16(PARAM, r);
            at(fx, p, 0.0);
            spawn(fx, f, b + so);
        }
    }
    for _ in 0..0xA {
        angle(fx, b + 0xF0);
        fx.mem.set16(PARAM, 0);
        at(fx, p, 0.0);
        spawn(fx, RING, b + 0x130);
    }
    spawn(fx, DISC, b + 0x154);
}

/// 0x1E00080(D, 0) (frame 0x60): the spawn colour (0x80, 0, 0, 0), the
/// update counter, the yaw word and the spawns (`s2` the entry's, saved).
fn emit(fx: &mut Fx, s2: u64) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x40, 0x01E0_37DC), (0x20, s2), (0x10, 0), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    // 0x1E01A10: the attacker's yaw (0x1F7370 into its frame's sp + 0x10).
    let f = fx.enter(0x20);
    fx.saved(f, &[(0, 0x01E0_00C8)]);
    let h = fx.self_handle();
    ra_slot(fx, 0x01E0_1A20);
    fx.unit_rotation(h, f + 0x10);
    let yaw = ee::from_i32(fx.mem.i32(f + 0x14));
    fx.leave(0x20);
    let v = fix(fx, ee::div(yaw, 180.0), 0x01E0_00E8);
    fx.mem.set16(YAW, v);
    let v = fx.mem.i16(YAW) - 0xB4;
    fx.mem.set16(YAW, v);
    let p = sp + 0x50;
    node_at(fx, 0, p, 0x01E0_010C);
    let c = counter(fx);
    if c == 0xB || c == 0x28 {
        for _ in 0..0x14 {
            fx.mem.set16(PARAM, 0);
            fx.mem.write_u32(D + 0x25C, 0);
            angle(fx, 0x01E0_0138);
            at(fx, p, 10.0);
            spawn(fx, MOTE, 0x01E0_0180);
        }
    }
    node_at(fx, 1, p, 0x01E0_01A8);
    let c = counter(fx);
    if c == 0x1D || c == 0x31 {
        for _ in 0..0x14 {
            angle(fx, 0x01E0_01D0);
            at(fx, p, 10.0);
            spawn(fx, MOTE, 0x01E0_0218);
        }
    }
    let c = counter(fx);
    if (0x3F..0x63).contains(&c) {
        for _ in 0..3 {
            angle(fx, 0x01E0_0250);
            at(fx, p, 10.0);
            spawn(fx, DRIFT, 0x01E0_0298);
        }
    }
    if counter(fx) >= 0x3F && fx.mem.i32(TIMER) >= 0x10 {
        for (k, ra, rra) in [(2u32, 0x01E0_02E0u32, 0x01E0_02ECu32), (3, 0x01E0_0340, 0x01E0_034C)] {
            node_at(fx, k, p, ra);
            for _ in 0..2 {
                angle(fx, rra);
                at(fx, p, 0.0);
            }
        }
    }
    if counter(fx) >= 0x44 && fx.mem.i32(TIMER) >= 0x10 {
        for (k, ra, rra, prm) in [(2u32, 0x01E0_03C0u32, 0x01E0_03DCu32, 2), (3, 0x01E0_0430, 0x01E0_044C, 3)] {
            node_at(fx, k, p, ra);
            if counter(fx) >= 0x3F {
                angle(fx, rra);
                at(fx, p, 0.0);
                fx.mem.set16(PARAM, prm);
            }
        }
    }
    if [0x77, 0x7D, 0x83, 0x88, 0x8D, 0x92, 0x97, 0x9C, 0xA2, 0xA8, 0xAE].contains(&counter(fx)) {
        node_at(fx, 2, p, 0x01E0_0528);
        burst(fx, p, 0x01E0_0534);
    }
    node_at(fx, 3, p, 0x01E0_06A0);
    if [0x7A, 0x80, 0x85, 0x8A, 0x8F, 0x95, 0x9A, 0x9F, 0xA5, 0xAB].contains(&counter(fx)) {
        burst(fx, p, 0x01E0_0728);
    }
    if counter(fx) == 0x48 {
        fx.mem.set16(PARAM, 0);
        spawn(fx, RIBBON, 0x01E0_08A4);
        fx.mem.set16(PARAM, 1);
        spawn(fx, RIBBON, 0x01E0_08C0);
    }
    if counter(fx) == 0x5E {
        spawn(fx, FLASH, 0x01E0_08E4);
    }
    fx.leave(0x60);
}

/// 0x204F70 into the scratchpad ([`l2::project`]) with what its 0x1A2C70
/// calls leave below its frame (sp - 0xF0) [F 0x205134..0x205160]: x 16,
/// y 16 and z of each accepted vertex in turn.
fn proj(fx: &mut Fx, input: u32, m: u32, d: u32, mean_w: u32) -> bool {
    let ok = l2::project(fx, input, SPR, m, d, mean_w);
    let n = u32::from(fx.mem.read_u16(d));
    let regs = fx.mem.read_u64(d + 8);
    let count = (0..n).filter(|i| (regs >> (4 * i)) & 15 == 4).count() as u32;
    let lim = f32::from_bits(0x457F_F000);
    let save = fx.sp;
    fx.sp = save.wrapping_sub(0xF0);
    for k in 0..count {
        let v = SPR + 16 * k;
        let (x, y, z) = (fx.mem.f(v), fx.mem.f(v + 4), fx.mem.f(v + 8));
        if crate::exe::ee_lt(z, 1.0) || !crate::exe::ee_le(z, f32::from_bits(0x4B7F_FFFF)) || crate::exe::ee_lt(x, 0.0) || !crate::exe::ee_le(x, lim) || crate::exe::ee_lt(y, 0.0) || !crate::exe::ee_le(y, lim) {
            break;
        }
        fx.fix_residue(ee::mul(16.0, x), 0x0020_513C);
        fx.fix_residue(ee::mul(16.0, y), 0x0020_5154);
        fx.fix_residue(z, 0x0020_5160);
    }
    fx.sp = save;
    ok
}

/// Submit `d` at the mean w `w` (0x1A2C70 from `ra`) into `ot`.
fn submit_at(fx: &mut Fx, d: u32, ot: u32, w: u32, ra: u32) {
    let v = fx.mem.f(w);
    fx.fix_residue(v, ra);
    let key = ee::fixunssfsi(v) as i32;
    fx.submit(d, ot, key);
}

/// The sprites' strip3 header in `d`: atlas cell 0x99 frame +0x34 (into
/// `r`), colour +0x28 on all four, (`alpha`), UVs orientation 0, texture
/// D+0x2BC.
fn sprite_desc(fx: &mut Fx, t: u32, d: u32, r: u32, alpha: bool) {
    let f = fx.mem.i16(t + 0x34);
    l2::rect(fx, r, 0x99, f);
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x28, 0);
    if alpha {
        fx.desc_alpha(d, 0x80_0000_0042);
    }
    l2::uv_strip3(fx, d, r, 0);
    let tex = fx.mem.read_u32(TEX_F);
    fx.desc_texture(d, tex, 1);
}

/// The draw loop's frame saves of the sprites: s0 = D, s1 the live count,
/// s2 = D + 0x40 k, s3 = k.
fn sprite_saves(fx: &mut Fx, sp: u32, ra_at: u32, ra: u32, r: &Regs) {
    fx.saved(sp, &[(ra_at, u64::from(ra)), (0x30, u64::from(r.s3)), (0x20, u64::from(r.s2)), (0x10, u64::from(r.s1)), (0, u64::from(D))]);
}

/// 0x1E03F50 (D, task, desc, draw) (frame 0x110): `draw` 0 builds the
/// header ([`sprite_desc`]); 1 draws a camera-facing square of the i16
/// size +0x38 at the task's point plus its drift +8 / +0xA / +0xC / 10
/// (integer), through 0x109870 into 0x206E40's table.
fn sprite(fx: &mut Fx, t: u32, d: u32, draw: bool, ra: u32, r: &Regs) {
    let sp = fx.enter(0x110);
    sprite_saves(fx, sp, 0x50, ra, r);
    if !draw {
        sprite_desc(fx, t, d, sp + 0xB0, false);
    } else {
        crate::b494::camera_angles_saving(fx, sp + 0x100, u64::from(OT), u64::from(d), 0x01E0_3FEC);
        for k in 0..3 {
            let dv = fx.mem.i16(t + 8 + 2 * k);
            fx.mult(0x6666_6667, dv);
            let hi = fx.hilo().1;
            let q = (hi >> 2).wrapping_add(((dv as u32) >> 31) as i32);
            let v = fx.mem.i16(t + 2 * k).wrapping_add(q);
            fx.mem.setf(sp + 0xA0 + 4 * k, ee::from_i32(v));
        }
        l2::xform_vp(fx, sp + 0xC0, 0, sp + 0x100, sp + 0xA0);
        let v = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x60, v, v);
        l2::project_submit(fx, sp + 0x60, d, sp + 0xC0, OT);
    }
    fx.leave(0x110);
}

/// 0x1E040F0 (D, task, desc, draw) (frame 0x120): as [`sprite`] with ALPHA
/// 0x42 in the header; the square at the point plus the drift over 10
/// (floats, scale 0.1), keyed by its mean w into D+0x38DC.
fn spark_sprite(fx: &mut Fx, t: u32, d: u32, draw: bool, ra: u32, r: &Regs) {
    let sp = fx.enter(0x120);
    fx.saved(sp, &[(0x40, u64::from(ra)), (0x30, u64::from(r.s3)), (0x20, u64::from(r.s2)), (0x10, u64::from(r.s1)), (0, u64::from(D))]);
    if !draw {
        sprite_desc(fx, t, d, sp + 0xB0, true);
    } else {
        crate::b494::camera_angles_saving(fx, sp + 0x110, u64::from(d), u64::from(t), 0x01E0_41A4);
        for k in 0..3 {
            let p = ee::from_i32(fx.mem.i16(t + 2 * k));
            let o = ee::div(ee::from_i32(fx.mem.i16(t + 8 + 2 * k)), 10.0);
            fx.mem.setf(sp + 0x90 + 4 * k, ee::add(p, o));
        }
        for k in 0..3 {
            fx.mem.write_u32(sp + 0xA0 + 4 * k, 0x3DCC_CCCD);
        }
        l2::xform_vp(fx, sp + 0xC0, sp + 0xA0, sp + 0x110, sp + 0x90);
        let v = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x50, v, v);
        if proj(fx, sp + 0x50, sp + 0xC0, d, sp + 0x100) {
            let ot = fx.mem.read_u32(OT2);
            submit_at(fx, d, ot, sp + 0x100, 0x01E0_4288);
        }
    }
    fx.leave(0x120);
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// RGBAQ of the colour at `c` (0x108020 into `local`) to `to`.
fn rgbaq_to(fx: &mut Fx, local: u32, c: u32, to: u32) {
    l2::rgbaq(fx, local, c);
    for k in 0..2 {
        let w = fx.mem.read_u32(local + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// 0x1E03030 (D, task, pts) (frame 0x260): a textured ribbon along the
/// vec4 history `pts` (i16 +6 = n points): per segment four curve pieces
/// (basis 0, t and t + 1/4; the first segment on stack copies p0 p0 p1
/// p2), the near pair (0x108070 which 1) coloured +0x28 as it stands and
/// the far pair (which 2) with the alpha less +0x2B / 3n (clamped at 0);
/// each piece drawn twice with its second and fourth vertices pushed by w
/// = +0x38 / 10 in y (+, then -); keyed by the mean w into 0x206E40's
/// table.
fn ribbon_draw(fx: &mut Fx, t: u32, pts: u32, r: &Regs) {
    use ee::{add, sub};
    let sp = fx.enter(0x260);
    let ra = 0x01E0_3D48u64;
    fx.saved(sp, &[(0xA0, ra), (0x40, u64::from(r.s3)), (0x30, u64::from(r.s2)), (0x20, u64::from(r.s1)), (0x10, u64::from(D))]);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 0x38)), 10.0);
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x1A8;
    l2::desc_strip3(fx, d);
    for (k, (src, to)) in [(0x28u32, 0x10u32), (0x2C, 0x28), (0x28, 0x40), (0x2C, 0x58)].into_iter().enumerate() {
        rgbaq_to(fx, sp + 0x238 + 8 * k as u32, t + src, d + to);
    }
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    let c = sp + 0x25C;
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(c + k, v);
    }
    copy4(fx, sp + 0xE0, pts);
    copy4(fx, sp + 0xF0, pts);
    copy4(fx, sp + 0x100, pts + 0x10);
    copy4(fx, sp + 0x110, pts + 0x20);
    crate::b494::camera_angles_saving(fx, sp + 0x170, n as i64 as u64, u64::from(D), 0x01E0_3214);
    let n3 = 3 * n;
    fx.mem.set32(sp + 0xB0, n - 1);
    let v = fx.mem.i32(sp + 0x174);
    fx.mult(0xB60B_60B7u32 as i32, v);
    let hi = fx.hilo().1;
    let q = (hi.wrapping_add(v) >> 7).wrapping_add(((v as u32) >> 31) as i32);
    fx.mem.set32(sp + 0x174, q.wrapping_neg());
    let mut s0 = 0i32;
    while s0 < fx.mem.i32(sp + 0xB0) {
        let (q, end) = if s0 != 0 { (pts.wrapping_add((0x10 * (s0 - 1)) as u32), 0x180u32) } else { (sp + 0xE0, 0x190) };
        let mut f20 = 0.0f32;
        for _ in 0..4 {
            spline::spline(fx, sp + 0xC0, q, f20, 0, [0.0; 4]);
            f20 = add(f20, 0.25);
            spline::spline(fx, sp + end, q, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0xD0, sp + end);
            l2::colour_strip3(fx, d, c, 1);
            let al = i32::from(fx.mem.read_u8(t + 0x2B));
            let dq = fx.div(al, n3).0 as i16 as i32;
            let v = i32::from(fx.mem.read_u8(c + 3)) - dq;
            fx.mem.write_u8(c + 3, if v > 0 { v as u8 } else { 0 });
            l2::colour_strip3(fx, d, c, 2);
            for s1 in 0..2 {
                for (to, from) in [(0x120u32, 0xC0u32), (0x130, 0xC0), (0x140, 0xD0), (0x150, 0xD0)] {
                    for k in 0..3 {
                        let x = fx.mem.read_u32(sp + from + 4 * k);
                        fx.mem.write_u32(sp + to + 4 * k, x);
                    }
                }
                for a in [0x134u32, 0x154] {
                    let x = fx.mem.f(sp + a);
                    fx.mem.setf(sp + a, if s1 == 0 { add(x, w) } else { sub(x, w) });
                }
                if proj(fx, sp + 0x120, 0, d, sp + 0x160) {
                    submit_at(fx, d, OT, sp + 0x160, 0x01E0_3518);
                }
            }
        }
        s0 += 1;
    }
    fx.leave(0x260);
}
