//! BMODULE b357 (card 1), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05340, data area 0x1ED0000) [F]: 110 tasks of 0x40 bytes at D+0x2F0 (alive +0x36, step +0x24),
//! spawned by the record's 0x1E06200 (colour D+0x3884 twice, the point
//! D+0x250, +0x30 = D+0x38C6, +0x3A = D+0x38BE); timer D+0x3880 = 0xCD,
//! counter D+0x38CE, nodes 0x26 (clear) / 4 / 0x1A / 0x6A / 0x40 at
//! D+0x38AC.. and the target's node 1 (clear) at D+0x38BA; two rand-byte
//! tables D+0x38D2 / 0x39D2 (the second zeroed from 0xB4).
//!
//! The emitter 0x1E02A30 counts D+0x38CE: update 1 the stage dimmer
//! 0x1E05230; on 0x44 / 0x4D / 0x57 / 0x63 / 0x6F / 0x7A node 0x26's
//! position appended to the history D+0x1EC0 (count D+0x38BC) and three
//! orbs 0x1E04240 / 0x46D0 / 0x4910, a trail 0x1E03F50, a ring 0x1E032B0,
//! three flame bands 0x1E03D30 and a ring 0x1E04B50 started on it; from
//! update 0xB each history point's z pushed 5 towards the target (by the
//! unit's facing, 0x1A2B80 of yaw / 180) while |dz| > 7 (0x1A7520); below
//! update 0xB4 with the timer at 0x15 or more two arcs 0x1E03420 a update
//! (D+0x38C8 cycling 0..9). The orbs follow their history point and end
//! when the target is within 7 in z; 0x1E04240's end spawns a disc
//! 0x1E04FE0, six clouds 0x1E030B0, five sparkles 0x1E03B10, six bands
//! 0x1E04D20, a ring 0x1E04E80, twelve bolts 0x1E03850 (D+0x38CA cycling
//! 0..14) and the light 0x1E050C0. Draws: `cloud::cloud`,
//! `ring_band_task`, `billboard::ring_t`, `band9_tex`, `fan18_far0` and
//! the record's 0x1E00D10 (trail), 0x1E01CD0 (arc), 0x1E02590 (orb),
//! 0x1E02770 (ring) and 0x1E06330 (bolt, twice). Steps 0x1E02EB0 /
//! 0x40E0 / 0x4190 / 0x52F0 are tested but never spawned (not ported).
//! Returns 1 on update 0x3C, 4 on 0xB4, 2 on 0xAA, 6 on 0x47..0xA6, 8 once
//! the timer has run out. No 0x1F8100.
use crate::blib::{afterimage, billboard, cloud, exe as bexe, quads4, spawn as bspawn, spline, uv};
use crate::bus::Bus;
use crate::c061::{desc_uv4, rgbaq_at};
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;

const CLOUD: u32 = 0x01E0_30B0;
const RING: u32 = 0x01E0_32B0;
const ARC: u32 = 0x01E0_3420;
const BOLT: u32 = 0x01E0_3850;
const SPARK: u32 = 0x01E0_3B10;
const FLAME: u32 = 0x01E0_3D30;
const TRAIL: u32 = 0x01E0_3F50;
const ORB: u32 = 0x01E0_4240;
const ORB_B: u32 = 0x01E0_46D0;
const ORB_C: u32 = 0x01E0_4910;
const HALO: u32 = 0x01E0_4B50;
const BAND: u32 = 0x01E0_4D20;
const STAR: u32 = 0x01E0_4E80;
const DISC: u32 = 0x01E0_4FE0;
const LIGHT: u32 = 0x01E0_50C0;
const DIM: u32 = 0x01E0_5230;

/// The run loop's compare chain (each tested again after the last ran).
const RUN: [(u32, u32); 16] = [
    (CLOUD, 0x01E0_55C8),
    (RING, 0x01E0_55E8),
    (ARC, 0x01E0_5608),
    (BOLT, 0x01E0_5628),
    (SPARK, 0x01E0_5648),
    (FLAME, 0x01E0_5668),
    (TRAIL, 0x01E0_5688),
    (ORB, 0x01E0_57F8),
    (ORB_B, 0x01E0_5818),
    (ORB_C, 0x01E0_5838),
    (HALO, 0x01E0_5858),
    (BAND, 0x01E0_5878),
    (STAR, 0x01E0_5898),
    (DISC, 0x01E0_58B8),
    (LIGHT, 0x01E0_58D8),
    (DIM, 0x01E0_58F8),
];

const POOL: u32 = D + 0x2F0;
const POS: u32 = D + 0x250;
const LAMP: u32 = D + 0x1E90;
const HIST: u32 = D + 0x1EC0;
const TRAILS: u32 = D + 0x1F60;
const ARCS: u32 = D + 0x2460;
const BOLTS: u32 = D + 0x30E0;
const TIMER: u32 = D + 0x3880;
const COLOUR: u32 = D + 0x3884;
const OT2: u32 = D + 0x388C;
const NODES: u32 = D + 0x38AC;
const TARGET_NODE: u32 = D + 0x38BA;
const HCOUNT: u32 = D + 0x38BC;
const ANGLE: u32 = D + 0x38BE;
const PARAM: u32 = D + 0x38C6;
const CYC_A: u32 = D + 0x38C8;
const CYC_B: u32 = D + 0x38CA;
const PARAM_COPY: u32 = D + 0x38CC;
const COUNTER: u32 = D + 0x38CE;
const BYTES: u32 = D + 0x38D2;
const BYTES2: u32 = D + 0x39D2;
const SINES: u32 = D + 0x3AD4;

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

/// The entry 0x1E05340 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x240);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(OT2, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x240);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3C40);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(PARAM_COPY, param);
    fx.mem.set32(TIMER, 0xCD);
    for (k, (n, clear)) in [(0x26, 1), (4, 0), (0x1A, 0), (0x6A, 0), (0x40, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    for k in 0xA..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x2D8 + 4 * (k as u32 - 0xA), t);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x2B8, t);
    for k in 0..4 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(D + 0x1E70 + 4 * k as u32, t);
    }
    // rand's frame: s0 = the loop counter.
    for (table, ra) in [(BYTES, 0x01E0_54ACu32), (BYTES2, 0x01E0_54E0)] {
        for k in 0..0x100u32 {
            let v = fx.rand_from(k, ra);
            let r = fx.div(v, 0x1E).1;
            fx.mem.write_u8(table + k, (r - 0xF) as u8);
        }
    }
    for k in 0xB4..0x100u32 {
        fx.mem.write_u8(BYTES2 + k, 0);
    }
    fx.mem.set16(HCOUNT, 0);
    for k in 0..8u32 {
        let v = fx.rand_from(k, 0x01E0_5538);
        fx.mem.set16(D + 0xC0 + 2 * k, v.wrapping_rem(4));
    }
    0
}

/// A callee's ra slot (`sq ra`) at the caller's sp - 0x10 (the upper half
/// 0) [H, as `Fx::saved`]: 0x1F71A0 / 0x1F7370 (0x20 frames) and 0x1F6830
/// (0x30) leave it where the caller's sp - 8 is read back by 0x106960.
fn ra_slot(fx: &mut Fx, ra: u32) {
    let a = fx.sp.wrapping_sub(0x10);
    fx.mem.write_u64(a, u64::from(ra));
    fx.mem.write_u64(a + 8, 0);
}

/// 0x1F71A0(target, out) from return address `ra`.
fn target_position(fx: &mut Fx, out: u32, ra: u32) {
    ra_slot(fx, ra);
    let h = fx.target_handle();
    fx.unit_position(h, out);
}

/// 0x1A7520 sqrtf of `x` called from `ra` (its `sd ra` at sp - 0x10).
fn sqrtf_from(fx: &mut Fx, x: f32, ra: u32) -> f32 {
    let a = fx.sp.wrapping_sub(0x10);
    fx.mem.write_u64(a, u64::from(ra));
    fx.sqrtf(x)
}

/// |target z - z| as the original takes it: sqrtf(dz dz).
fn dz(fx: &mut Fx, tz: u32, z: u32, ra: u32) -> f32 {
    let d = ee::sub(fx.mem.f(tz), fx.mem.f(z));
    sqrtf_from(fx, ee::mul(d, d), ra)
}

/// 0x1A2B80 of `v` from `ra`.
fn fix(fx: &mut Fx, v: f32, ra: u32) -> i32 {
    fx.fix_residue(v, ra);
    ee::fixsfsi(v)
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..110u32 {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        for (at, ra) in RUN {
            if fx.mem.read_u32(t + 0x24) == at {
                run_step(fx, t, k, at, ra);
            }
        }
    }
    target_position(fx, sp + 0x90, 0x01E0_596C);
    let d = sp + 0x1A8;
    // The entry's registers in the draw loop (for the callees' saves and
    // rand's saved s0).
    let mut s0 = D + 0x40 * 110;
    let mut s2 = 110u32;
    let mut s5: Option<u32> = None;
    let mut s7 = 0u32;
    for k in 0..110u32 {
        let t = POOL + 0x40 * k;
        let s4 = 0x8000 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        s7 += 1;
        let r = Regs { s0, s2, s3: D + 0x40 * k, s4, s5, s6: k, s7 };
        match fx.mem.read_u32(t + 0x24) {
            CLOUD => {
                cell(fx, t, D + 0x1E78, sp + 0xC0, 0x14);
                fx.mem.set16(D + 0xB8, 4);
                let v = fx.rand_from(s0, 0x01E0_5A58);
                fx.mem.set16(D + 0xBE, v.wrapping_rem(4));
                cloud::cloud(fx, D, t, d, BYTES);
                fx.mem.set16(D + 0xB8, 0);
            }
            RING => {
                cell(fx, t, D + 0x2EC, sp + 0xD0, 0x97);
                fx.mem.set16(D + 0x94, 0xA0);
                let h = fx.mem.i16(D + 0x9A) >> 1;
                fx.mem.set16(D + 0x9A, h);
                fx.mem.set16(D + 0xB8, 0xA);
                quads4::ring_band_task(fx, 0x01E0_1790, D, t, d);
                fx.mem.set16(D + 0xB8, 0);
            }
            ARC => {
                fx.mem.set16(D + 0xBE, 0);
                cell(fx, t, D + 0x2EC, sp + 0xE0, 0x96);
                let p = ARCS + 0xA0 * fx.mem.i16(t + 0x30) as u32;
                arc_draw(fx, t, p, &r);
            }
            BOLT => {
                fx.mem.write_u32(sp + 0xA8, 0);
                fx.mem.write_u32(sp + 0xA4, 0);
                fx.mem.set16(D + 0xBE, 0);
                cell(fx, t, D + 0x2E8, sp + 0xF0, 0x8D);
                s2 = t;
                s5 = Some(sp + 0xA8);
                for j in 0..2 {
                    fx.mem.write_u32(sp + 0xA8, s4);
                    let p = BOLTS + 0x80 * fx.mem.i16(t + 0x30) as u32;
                    let r = Regs { s0: j, s2, s5, ..r };
                    bolt_draw(fx, t, p, sp + 0xA0, &r);
                }
                s0 = 2;
            }
            SPARK => {
                cell(fx, t, D + 0x2EC, sp + 0x100, 0x9B);
                fx.mem.set16(D + 0xB8, 0xA);
                billboard::ring_t(fx, uv::uv_rect4, D, t, d, BYTES);
                fx.mem.set16(D + 0xB8, 0);
            }
            FLAME => {
                cell(fx, t, D + 0x2EC, sp + 0x110, 0x96);
                fx.mem.set16(D + 0xB8, 2);
                quads4::band9_tex(fx, D, t, d);
                fx.mem.set16(D + 0xB8, 0);
            }
            TRAIL => {
                for (order, r_at, id, ra) in [(1, sp + 0x120, 0x97, 0x01E0_5DA8u32), (0, sp + 0x130, 0x96, 0x01E0_5E10)] {
                    fx.mem.set16(D + 0xBE, order);
                    cell(fx, t, D + 0x2EC, r_at, id);
                    let p = TRAILS + 0xA0 * fx.mem.i16(t + 0x30) as u32;
                    trail_draw(fx, t, p, ra, &r);
                }
            }
            ORB | ORB_B | ORB_C => {
                let at = fx.mem.read_u32(t + 0x24);
                let (r_at, rand_ra, ra) = match at {
                    ORB => (sp + 0x140, 0x01E0_5E68u32, 0x01E0_5E94u32),
                    ORB_B => (sp + 0x150, 0x01E0_5EF8, 0x01E0_5F24),
                    _ => (sp + 0x160, 0x01E0_5F88, 0x01E0_5FB4),
                };
                cell(fx, t, D + 0x1E70, r_at, 0x14);
                if at != ORB {
                    let v = fx.mem.read_u32(D + 0x1E7C);
                    fx.mem.write_u32(D + 0x88, v);
                }
                let v = fx.rand_from(s0, rand_ra);
                fx.mem.set16(D + 0xBE, v.wrapping_rem(4));
                orb_draw(fx, t, d, ra, &r);
            }
            BAND => {
                cell(fx, t, D + 0x2E8, sp + 0x170, 0x8C);
                fx.mem.set16(D + 0xB8, 0);
                quads4::ring_band_task(fx, 0x01E0_1790, D, t, d);
                fx.mem.set16(D + 0xB8, 0);
            }
            STAR | HALO => {
                let (r_at, ra) = if fx.mem.read_u32(t + 0x24) == STAR { (sp + 0x180, 0x01E0_6094u32) } else { (sp + 0x190, 0x01E0_6104) };
                cell(fx, t, D + 0x2B8, r_at, 0x14);
                fx.mem.set16(D + 0xB8, 0);
                star_draw(fx, t, d, ra, &r);
                fx.mem.set16(D + 0xB8, 0);
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
        0x3C => 1,
        0xB4 => 4,
        0xAA => 2,
        c if (0x47..0xA7).contains(&c) => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The entry's registers at a draw call.
#[derive(Clone, Copy)]
struct Regs {
    s0: u32,
    s2: u32,
    s3: u32,
    s4: u32,
    s5: Option<u32>,
    s6: u32,
    s7: u32,
}

/// The texture word at `tex` and the atlas cell `id` frame +0x34 (0x106960
/// into `r`) copied to D+0x88 / D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, tex: u32, r: u32, id: i32) {
    let v = fx.mem.read_u32(tex);
    fx.mem.write_u32(D + 0x88, v);
    let f = fx.mem.i16(t + 0x34);
    l2::rect(fx, r, id, f);
    for k in 0..4 {
        let w = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, w);
    }
}

/// Saves at `sp`: each (offset, value), the unknown ones (the runner's
/// registers) left as they are.
fn saves(fx: &mut Fx, sp: u32, slots: &[(u32, Option<u32>)]) {
    for &(o, v) in slots {
        if let Some(v) = v {
            fx.saved(sp, &[(o, u64::from(v))]);
        }
    }
}

/// A step's frame with its prologue saves (the run loop's s0 = D + 0x40 k,
/// s1 = D, s2 = k).
fn run_step(fx: &mut Fx, t: u32, k: u32, at: u32, ra: u32) {
    let s0 = D + 0x40 * k;
    let size = match at {
        ARC => 0xA0,
        BOLT => 0x60,
        ORB => 0x50,
        ORB_B | ORB_C | HALO => 0x40,
        FLAME | TRAIL | LIGHT => 0x30,
        DIM => 0x10,
        _ => 0x20,
    };
    let sp = fx.enter(size);
    match size {
        0x10 => saves(fx, sp, &[(0, Some(ra))]),
        0x20 => saves(fx, sp, &[(0x10, Some(ra)), (0, Some(s0))]),
        0x30 | 0x40 => saves(fx, sp, &[(0x20, Some(ra)), (0x10, Some(D)), (0, Some(s0))]),
        0x50 => saves(fx, sp, &[(0x30, Some(ra)), (0x20, Some(k)), (0x10, Some(D)), (0, Some(s0))]),
        0x60 => saves(fx, sp, &[(0x50, Some(ra)), (0x20, Some(k)), (0x10, Some(D)), (0, Some(s0))]),
        _ => saves(fx, sp, &[(0x80, Some(ra)), (0x30, Some(k)), (0x20, Some(D)), (0x10, Some(s0))]),
    }
    match at {
        CLOUD => cloud_step(fx, t),
        RING => ring(fx, t),
        ARC => arc(fx, t, sp),
        BOLT => bolt(fx, t, s0),
        SPARK => spark(fx, t),
        FLAME => flame(fx, t),
        TRAIL => trail(fx, t),
        ORB => orb(fx, t, sp, s0),
        ORB_B => orb_bc(fx, t, sp, false),
        ORB_C => orb_bc(fx, t, sp, true),
        HALO => halo(fx, t, sp),
        BAND => band(fx, t),
        STAR => star(fx, t),
        DISC => disc(fx, t),
        LIGHT => light(fx, t),
        _ => dim(fx, t),
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

fn odd(fx: &mut Fx, t: u32) -> bool {
    age(fx, t).wrapping_rem(2) == 1
}

/// The age's remainder by 3 (a `div`).
fn third(fx: &mut Fx, t: u32) -> i32 {
    let a = age(fx, t);
    fx.div(a, 3).1
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

/// Alpha +0x2B to 0x80 when below it.
fn at_least_80(fx: &mut Fx, t: u32) {
    if fx.mem.read_u8(t + 0x2B) < 0x80 {
        fx.mem.write_u8(t + 0x2B, 0x80);
    }
}

fn rand_rem(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn both(fx: &mut Fx, t: u32, r: i32, g: i32, b: i32) {
    l2::rgb(fx, t + 0x28, r, g, b);
    l2::rgb(fx, t + 0x2C, r, g, b);
}

/// The frame +0x34 up to 7, then back to 0.
fn cycle_frame(fx: &mut Fx, t: u32) {
    let f = fx.mem.i16(t + 0x34);
    fx.mem.set16(t + 0x34, if f < 7 { f + 1 } else { 0 });
}

/// History point `p`'s address.
fn hist(p: i32) -> u32 {
    HIST.wrapping_add((0x10 * p) as u32)
}

/// 0x1E030B0: a grey cloud (size rand % 0x50 + 0x1E, radius +0x30 rand %
/// 0xFA + 0x64 growing by +0xE = rand % 5 + 5, angle rand % 360, +8 rand %
/// 10, life +6 rand % 10 + 4): alpha up by 0x40 (below 0x81) for the
/// life, then the task ends; the frame up to 6 on odd ages; the turn
/// three rand % 360 every update.
fn cloud_step(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        let r = rand_rem(fx, t, 0x01E0_30D4, 0x50);
        fx.mem.set16(t + 0x38, r + 0x1E);
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, 0x60, 0x60, 0x60);
        let r = rand_rem(fx, t, 0x01E0_3108, 0xFA);
        fx.mem.set16(t + 0x30, r + 0x64);
        let r = rand_rem(fx, t, 0x01E0_3128, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let r = rand_rem(fx, t, 0x01E0_3144, 5);
        fx.mem.set16(t + 0xE, r + 5);
        let r = rand_rem(fx, t, 0x01E0_3164, 0xA);
        fx.mem.set16(t + 8, r);
        let r = rand_rem(fx, t, 0x01E0_3180, 0xA);
        fx.mem.set16(t + 6, r + 4);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < fx.mem.i16(t + 6) {
        rise(fx, t, 0x81, 0x40);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    if odd(fx, t) && fx.mem.i16(t + 0x34) < 6 {
        let v = fx.mem.i16(t + 0x34) + 1;
        fx.mem.set16(t + 0x34, v);
    }
    if age(fx, t) != 0 {
        let v = fx.mem.i16(t + 0x30).wrapping_add(fx.mem.i16(t + 0xE));
        fx.mem.set16(t + 0x30, v);
    }
    for (k, ra) in [0x01E0_3238u32, 0x01E0_3254, 0x01E0_3270].into_iter().enumerate() {
        let r = rand_rem(fx, t, ra, 0x168);
        fx.mem.set32(t + 0x10 + 4 * k as u32, r);
    }
    grow(fx, t);
}

/// 0x1E032B0: a ring band (size 0 growing by +8 = rand % 3 + 4, turn +0x10
/// = 0x4000, frame 1, +6 = 8, white / rim (0x10, 0x10, 0xF0)): alpha up by
/// 0x80 (below 0x80) on update 0, then from size 0x15 down by 0x13 (ends
/// below 0x15); green (0x10, 0xF0, 0x10) on odd ages, else (8, 0xF0, 8).
fn ring(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0);
        fx.mem.set32(t + 0x10, 0x4000);
        fx.mem.set16(t + 0x34, 1);
        fx.mem.set16(t + 0x36, 2);
        let r = rand_rem(fx, t, 0x01E0_32E8, 3);
        fx.mem.set16(t + 8, r + 4);
        fx.mem.set16(t + 6, 8);
        l2::rgb(fx, t + 0x28, 0xF0, 0xF0, 0xF0);
        l2::rgb(fx, t + 0x2C, 0x10, 0x10, 0xF0);
    }
    if age(fx, t) <= 0 {
        rise(fx, t, 0x80, 0x80);
    } else if fx.mem.i16(t + 0x38) >= 0x15 {
        fall(fx, t, 0x15, 0x13);
    }
    if odd(fx, t) {
        l2::rgb(fx, t + 0x28, 0x10, 0xF0, 0x10);
    } else {
        l2::rgb(fx, t + 0x28, 8, 0xF0, 8);
    }
    if age(fx, t) != 0 {
        let v = fx.mem.i16(t + 0x38).wrapping_add(fx.mem.i16(t + 8));
        fx.mem.set16(t + 0x38, v);
    }
    grow(fx, t);
}

/// 0x1E03420 (frame `sp`): an arc (frame 0, unit scale, life +6 rand % 5 +
/// 5) at the target's node 1 (as i16), size rand % fix(radius) + 5: on
/// its first update its ten points D+0x2460 + 0xA0 +0x30 laid on a
/// Lissajous of radius (unit radius 0x1F6A70) x (rand % 10 + 2) over
/// three angles a / b / c = rand % 360 stepping 0x12 (y = r cos a, z = r
/// sin b, x = r sin c), each jittered by rand % 0x50 - 0x28; alpha up by
/// 0x60 (below 0xC0) on ages 0 / 1, then down by 0x27 (ends below 0x29);
/// (0xC0, 0xC0, 0x40) when the age % 3 is 1, else (0x40, 0x40, 0xC0).
fn arc(fx: &mut Fx, t: u32, sp: u32) {
    use ee::{add, mul};
    if first(fx, t) {
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x36, 2);
        for o in [8u32, 0xA, 0xC] {
            fx.mem.set16(t + o, 1);
        }
        let r = rand_rem(fx, t, 0x01E0_3478, 5);
        fx.mem.set16(t + 6, r + 5);
        let h = fx.target_handle();
        let n = fx.mem.i16(TARGET_NODE);
        ra_slot(fx, 0x01E0_34A8);
        fx.node_position(h, n, sp + 0x90);
        for (k, ra) in [0x01E0_34B0u32, 0x01E0_34BC, 0x01E0_34C8].into_iter().enumerate() {
            let v = fx.mem.f(sp + 0x90 + 4 * k as u32);
            let i = fix(fx, v, ra);
            fx.mem.set16(t + 2 * k as u32, i);
        }
        let mut a = rand_rem(fx, t, 0x01E0_34D0, 0x168) as i16 as i32;
        let mut b = rand_rem(fx, t, 0x01E0_34F0, 0x168) as i16 as i32;
        let mut c = rand_rem(fx, t, 0x01E0_3510, 0x168) as i16 as i32;
        let h = fx.target_handle();
        let rad = afterimage::unit_radius(fx, h);
        let s5 = fx.rand_from(t, 0x01E0_3540);
        let m = fix(fx, rad, 0x01E0_354C) as i16 as i32;
        let q = fx.div(s5, m).1;
        fx.mem.set16(t + 0x38, q + 5);
        let k = rand_rem(fx, t, 0x01E0_3570, 0xA) + 2;
        let f20 = mul(rad, ee::from_i32(k));
        for j in 0..10u32 {
            let p = ARCS.wrapping_add((0xA0 * fx.mem.i16(t + 0x30)) as u32) + 0x10 * j;
            let s = l2::sin_deg(fx, SINES, a, 1);
            fx.mem.setf(p + 4, mul(f20, s));
            let s = l2::sin_deg(fx, SINES, b, 0);
            fx.mem.setf(p + 8, mul(f20, s));
            let s = l2::sin_deg(fx, SINES, c, 0);
            fx.mem.setf(p, mul(f20, s));
            for (o, ra) in [(0u32, 0x01E0_3654u32), (4, 0x01E0_3694), (8, 0x01E0_36D4)] {
                let r = rand_rem(fx, t, ra, 0x50) - 0x28;
                let p = ARCS.wrapping_add((0xA0 * fx.mem.i16(t + 0x30)) as u32) + 0x10 * j;
                let v = add(fx.mem.f(p + o), ee::from_i32(r));
                fx.mem.setf(p + o, v);
            }
            a = (a + 0x12) as i16 as i32;
            b = (b + 0x12) as i16 as i32;
            c = (c + 0x12) as i16 as i32;
        }
    }
    if age(fx, t) < 2 {
        rise(fx, t, 0xC0, 0x60);
    } else {
        fall(fx, t, 0x29, 0x27);
    }
    if third(fx, t) == 1 {
        both(fx, t, 0xC0, 0xC0, 0x40);
    } else {
        both(fx, t, 0x40, 0x40, 0xC0);
    }
    grow(fx, t);
}

/// 0x1E03850 (rand's s0 the run loop's `s0`): a bolt (frame rand % 3,
/// turn +0x18 rand % 0xFFFF, unit scale, life +0xE rand % 6 + 1, +6 = 8
/// points, size rand % 0x14 + 0xA): on its first update its points
/// D+0x30E0 + 0x80 +0x30 rise from 0 by 15 + rand % 0x19 each (x, z 0);
/// alpha up by 0x60 (below 0xC0) for the life, then down by 0x27 (ends
/// below 0x29); grey 0x40 on odd ages, else 0xC0.
fn bolt(fx: &mut Fx, t: u32, s0: u32) {
    use ee::add;
    if first(fx, t) {
        let r = rand_rem(fx, s0, 0x01E0_3888, 3);
        fx.mem.set16(t + 0x34, r);
        fx.mem.set16(t + 0x36, 2);
        let r = rand_rem(fx, s0, 0x01E0_38AC, 0xFFFF);
        fx.mem.set32(t + 0x18, r);
        for o in [8u32, 0xA, 0xC] {
            fx.mem.set16(t + o, 1);
        }
        let r = rand_rem(fx, s0, 0x01E0_38D8, 6);
        fx.mem.set16(t + 0xE, r + 1);
        fx.mem.set16(t + 6, 6);
        let r = rand_rem(fx, s0, 0x01E0_38FC, 0x14);
        fx.mem.set16(t + 0x38, r + 0xA);
        fx.rand_from(s0, 0x01E0_391C);
        fx.rand_from(s0, 0x01E0_3924);
        for j in 0..8u32 {
            let p = BOLTS.wrapping_add((0x80 * fx.mem.i16(t + 0x30)) as u32) + 0x10 * j;
            if j != 0 {
                let r = rand_rem(fx, j, 0x01E0_394C, 0x19);
                let y = add(15.0, add(fx.mem.f(p - 0xC), ee::from_i32(r)));
                fx.mem.setf(p + 4, y);
            } else {
                fx.mem.write_u32(p + 4, 0);
            }
            fx.mem.write_u32(p + 8, 0);
            fx.mem.write_u32(p, 0);
        }
    }
    if age(fx, t) < fx.mem.i16(t + 0xE) {
        rise(fx, t, 0xC0, 0x60);
    } else {
        fall(fx, t, 0x29, 0x27);
    }
    if odd(fx, t) {
        both(fx, t, 0x40, 0x40, 0x40);
    } else {
        both(fx, t, 0xC0, 0xC0, 0xC0);
    }
    grow(fx, t);
}

/// 0x1E03B10: a sparkle ring (`ring_t`: size rand % 8 + 10, frame 0, turn
/// +0x18 rand % 0xFFFF, radius +0x30 rand % 0x96 + 0x32 growing by +0xE =
/// rand % 10 + 0x14, angle rand % 360, +8 rand % 10, life +6 rand % 7 +
/// 3): alpha up by 0x30 (below 0x61) for the life, then down by 0xA (ends
/// below 0x11); colour by the age % 3: 1 (0xC0, 0xC0, 0x40), 2 (0xC0,
/// 0x40, 0x40), else (0x40, 0x40, 0xC0).
fn spark(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        let v = fx.rand_from(t, 0x01E0_3B34);
        fx.mem.set16(t + 0x38, v.wrapping_rem(8) + 0xA);
        fx.mem.set16(t + 0x34, 0);
        let r = rand_rem(fx, t, 0x01E0_3B58, 0xFFFF);
        fx.mem.set32(t + 0x18, r);
        l2::rgb(fx, t + 0x28, 0xC0, 0xC0, 0xC0);
        let r = rand_rem(fx, t, 0x01E0_3B84, 0x96);
        fx.mem.set16(t + 0x30, r + 0x32);
        let r = rand_rem(fx, t, 0x01E0_3BA4, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let r = rand_rem(fx, t, 0x01E0_3BC0, 0xA);
        fx.mem.set16(t + 0xE, r + 0x14);
        let r = rand_rem(fx, t, 0x01E0_3BE0, 0xA);
        fx.mem.set16(t + 8, r);
        let r = rand_rem(fx, t, 0x01E0_3BFC, 7);
        fx.mem.set16(t + 6, r + 3);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < fx.mem.i16(t + 6) {
        rise(fx, t, 0x61, 0x30);
    } else {
        fall(fx, t, 0x11, 0xA);
    }
    match third(fx, t) {
        1 => l2::rgb(fx, t + 0x28, 0xC0, 0xC0, 0x40),
        2 => l2::rgb(fx, t + 0x28, 0xC0, 0x40, 0x40),
        _ => l2::rgb(fx, t + 0x28, 0x40, 0x40, 0xC0),
    }
    if age(fx, t) != 0 {
        let v = fx.mem.i16(t + 0x30).wrapping_add(fx.mem.i16(t + 0xE));
        fx.mem.set16(t + 0x30, v);
    }
    grow(fx, t);
}

/// 0x1E03D30: a flame band (`band9_tex`: size 0 growing by +0xE = rand % 3
/// + 4 after each update, turn +0x10 rand % 0x2000 + 0x3000, +0x14 rand %
/// 0xFFFF, frame 1, +6 rand % 10 + 2; orange on parameter 0, else cyan):
/// alpha up by 0x50 (below 0xA0) on ages 0 / 1; then from size 0x17 down
/// by 0x13 (ends below 0x15), ends from size 0x2B, and down by 0xF (ends
/// below 0x11) once the timer is under 0xA; (0x10, 0x10, 0x80) on odd
/// ages, else (0x40, 0x40, 0x80).
fn flame(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0);
        let v = fx.rand_from(t, 0x01E0_3D5C);
        fx.mem.set32(t + 0x10, v.wrapping_rem(0x2000) + 0x3000);
        let r = rand_rem(fx, t, 0x01E0_3D7C, 0xFFFF);
        fx.mem.set32(t + 0x14, r);
        fx.mem.set16(t + 0x34, 1);
        fx.mem.set16(t + 0x36, 2);
        let r = rand_rem(fx, t, 0x01E0_3DA8, 3);
        fx.mem.set16(t + 0xE, r + 4);
        let r = rand_rem(fx, t, 0x01E0_3DC8, 0xA);
        fx.mem.set16(t + 6, r + 2);
        if fx.mem.i16(t + 0x30) == 0 {
            l2::rgb(fx, t + 0x28, 0x80, 0x40, 0);
        } else {
            l2::rgb(fx, t + 0x28, 0, 0x80, 0x80);
        }
    }
    if age(fx, t) < 2 {
        rise(fx, t, 0xA0, 0x50);
    } else {
        if fx.mem.i16(t + 0x38) >= 0x17 {
            fall(fx, t, 0x15, 0x13);
        }
        if fx.mem.i16(t + 0x38) >= 0x2B {
            fx.mem.set16(t + 0x36, 0);
        }
        if fx.mem.i32(TIMER) < 0xA {
            fall(fx, t, 0x11, 0xF);
        }
    }
    if odd(fx, t) {
        l2::rgb(fx, t + 0x28, 0x10, 0x10, 0x80);
    } else {
        l2::rgb(fx, t + 0x28, 0x40, 0x40, 0x80);
    }
    grow(fx, t);
    let v = fx.mem.i16(t + 0x38).wrapping_add(fx.mem.i16(t + 0xE));
    fx.mem.set16(t + 0x38, v);
}

/// 0x1E03F50: a trail (size 0x1E, +6 = 6 points, (0x80, 0x80, 8)): its
/// ten-point history D+0x1F60 + 0xA0 +0x30 filled with its node history
/// point on the first update, then pushed (`spawn::history`); alpha 0x30
/// for 0x28 updates, then down by 9 (ends below 0xB); (0x80, 0x80, 8) on
/// odd ages, else (0xC0, 0x80, 8).
fn trail(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 8);
        fx.mem.set16(t + 6, 6);
        fx.mem.write_u8(t + 0x2B, 0x80);
        fx.mem.set16(t + 0x38, 0x1E);
        fx.mem.set16(t + 0x36, 2);
        let p = fx.mem.i16(t + 0x30);
        let (src, to) = (hist(p), TRAILS.wrapping_add((0xA0 * p) as u32));
        for i in 0..10u32 {
            for c in 0..3 {
                let w = fx.mem.read_u32(src + 4 * c);
                fx.mem.write_u32(to + 0x10 * i + 4 * c, w);
            }
        }
    }
    if age(fx, t) < 0x28 {
        fx.mem.write_u8(t + 0x2B, 0x30);
    } else {
        fall(fx, t, 0xB, 9);
    }
    grow(fx, t);
    let p = fx.mem.i16(t + 0x30);
    bspawn::history(fx, TRAILS.wrapping_add((0xA0 * p) as u32) + 0x90, 0xA, hist(p));
    if odd(fx, t) {
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 8);
    } else {
        l2::rgb(fx, t + 0x28, 0xC0, 0x80, 8);
    }
}

/// The orbs' common start: the task's point (as i16, 0x1A2B80 from `ras`)
/// at its history point +0x30.
fn orb_point(fx: &mut Fx, t: u32, ras: [u32; 3]) {
    for (k, ra) in ras.into_iter().enumerate() {
        let p = hist(fx.mem.i16(t + 0x30));
        let v = fx.mem.f(p + 4 * k as u32);
        let i = fix(fx, v, ra);
        fx.mem.set16(t + 2 * k as u32, i);
    }
}

/// The orbs' end: the frame cycling 0..7, the age, the end at age 0xD3 and
/// the angle +0x3A up by 10 (mod 360).
fn orb_tail(fx: &mut Fx, t: u32) {
    cycle_frame(fx, t);
    grow(fx, t);
    if age(fx, t) >= 0xD3 {
        fx.mem.set16(t + 0x36, 0);
    }
    let a = fx.mem.i16(t + 0x3A) + 0xA;
    fx.mem.set16(t + 0x3A, a);
    let a = fx.mem.i16(t + 0x3A);
    if a >= 0x168 {
        let r = fx.div(a, 0x168).1;
        fx.mem.set16(t + 0x3A, r);
    }
}

/// The spawn point = history point 0.
fn at_first(fx: &mut Fx) {
    for c in 0..3 {
        let w = fx.mem.read_u32(HIST + 4 * c);
        fx.mem.write_u32(POS + 4 * c, w);
    }
}

/// 0x1E04240 (frame `sp`, rand's s0 the loop counters): the leading orb
/// (size 0x50, white, alpha 0x80, +6 = 5) at its history point; once the
/// target is within 7 in z it ends and bursts at history point 0: a disc,
/// six clouds jittered by rand % 0x1E - 0xF, five sparkles, six bands, a
/// star, twelve bolts (+0x30 from D+0x38CA cycling 0..14) and the light.
fn orb(fx: &mut Fx, t: u32, sp: u32, s0: u32) {
    use ee::add;
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x50);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 6, 5);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    fx.mem.write_u8(t + 0x2B, 0x80);
    orb_point(fx, t, [0x01E0_42FC, 0x01E0_4314, 0x01E0_432C]);
    target_position(fx, sp + 0x40, 0x01E0_4340);
    let z = hist(fx.mem.i16(t + 0x30)) + 8;
    let d = dz(fx, sp + 0x48, z, 0x01E0_4360);
    if ee_lt(d, 7.0) {
        fx.mem.set16(t + 0x36, 0);
        let p = hist(fx.mem.i16(t + 0x30));
        for c in 0..3 {
            let w = fx.mem.read_u32(p + 4 * c);
            fx.mem.write_u32(POS + 4 * c, w);
        }
        let s = [Some(s0), Some(t), Some(D)];
        spawn(fx, DISC, 0x01E0_43D0, s);
        for j in 0..6u32 {
            for (c, ra) in [(0u32, 0x01E0_43E0u32), (4, 0x01E0_4414), (8, 0x01E0_4448)] {
                let r = rand_rem(fx, j, ra, 0x1E) - 0xF;
                let v = add(fx.mem.f(HIST + c), ee::from_i32(r));
                fx.mem.setf(POS + c, v);
            }
            spawn(fx, CLOUD, 0x01E0_4484, [Some(j), Some(t), Some(D)]);
        }
        for (f, n, ra) in [(SPARK, 5u32, 0x01E0_44D8u32), (BAND, 6, 0x01E0_4530), (STAR, 1, 0x01E0_4588)] {
            for j in 0..n {
                at_first(fx);
                spawn(fx, f, ra, [Some(j), Some(t), Some(D)]);
            }
        }
        for j in 0..0xCu32 {
            let c = fx.mem.i16(CYC_B);
            fx.mem.set16(PARAM, c);
            at_first(fx);
            spawn(fx, BOLT, 0x01E0_45E4, [Some(j), Some(t), Some(D)]);
            let c = fx.mem.i16(CYC_B) + 1;
            fx.mem.set16(CYC_B, c);
            if fx.mem.i16(CYC_B) >= 0xF {
                fx.mem.set16(CYC_B, 0);
            }
        }
        spawn(fx, LIGHT, 0x01E0_4640, [Some(0xC), Some(t), Some(D)]);
    }
    orb_tail(fx, t);
}

/// 0x1E046D0 (size 0x50, alpha 0x80) / 0x1E04910 (`c`: size 0x78, alpha
/// 0x20 on update 0 and odd ages, else 0x10) (frame `sp`): a trailing orb
/// (frame rand & 7, white then (0xC0, 0xC0, 0) on odd ages / (0x40, 0x40,
/// 0xC0), +6 = 5) at its history point; it ends once the target is within
/// 7 in z.
fn orb_bc(fx: &mut Fx, t: u32, sp: u32, c: bool) {
    let b = if c { 0x01E0_4910 - 0x01E0_46D0 } else { 0 };
    if first(fx, t) {
        fx.mem.set16(t + 0x38, if c { 0x78 } else { 0x50 });
        let v = fx.rand_from(t, 0x01E0_4700 + b);
        fx.mem.set16(t + 0x34, v.wrapping_rem(8));
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 6, 5);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    let a = if !c {
        0x80
    } else if age(fx, t) <= 0 || odd(fx, t) {
        0x20
    } else {
        0x10
    };
    fx.mem.write_u8(t + 0x2B, a);
    orb_point(fx, t, [0x01E0_47A4 + b, 0x01E0_47BC + b, 0x01E0_47D4 + b]);
    if odd(fx, t) {
        l2::rgb(fx, t + 0x28, 0xC0, 0xC0, 0);
    } else {
        l2::rgb(fx, t + 0x28, 0x40, 0x40, 0xC0);
    }
    target_position(fx, sp + 0x30, 0x01E0_4840 + b);
    let z = hist(fx.mem.i16(t + 0x30)) + 8;
    let d = dz(fx, sp + 0x38, z, 0x01E0_4860 + b);
    if ee_lt(d, 7.0) {
        fx.mem.set16(t + 0x36, 0);
    }
    orb_tail(fx, t);
}

/// 0x1E04B50 (frame `sp`): a halo (`star_draw`: size 5, frame cycling
/// 0..7, angle rand % 360, (8, 8, 0x40), radius +6 from 0 by +8 = 2 after
/// update 0, then (0x10, 0x50, 0x10) on age 1 and (0x18, 0x38, 0x80) on
/// age 2): alpha to 0x80 on update 0, then down by 0x12 (ends below
/// 0x14); its z +4 by 2 an update, down when the unit faces back (yaw /
/// 180 through 0x1A2B80 not 0).
fn halo(fx: &mut Fx, t: u32, sp: u32) {
    let h = fx.self_handle();
    ra_slot(fx, 0x01E0_4B74);
    fx.unit_rotation(h, sp + 0x30);
    let f = ee::div(ee::from_i32(fx.mem.i32(sp + 0x34)), 180.0);
    let s0 = fix(fx, f, 0x01E0_4B9C) as i16 as i32;
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 5);
        fx.mem.set16(t + 0x34, 0);
        let v = fx.rand_from(0, 0x01E0_4BC0);
        let f = fx.sp.wrapping_sub(0x10);
        fx.mem.write_u64(f, s0 as i64 as u64);
        let r = fx.div(v, 0x168).1;
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.set16(t + 8, 2);
        fx.mem.set16(t + 6, 0);
        l2::rgb(fx, t + 0x28, 8, 8, 0x40);
    }
    if age(fx, t) <= 0 {
        at_least_80(fx, t);
    } else {
        fall(fx, t, 0x14, 0x12);
    }
    cycle_frame(fx, t);
    match age(fx, t) {
        1 => l2::rgb(fx, t + 0x28, 0x10, 0x50, 0x10),
        2 => l2::rgb(fx, t + 0x28, 0x18, 0x38, 0x80),
        _ => {}
    }
    if age(fx, t) != 0 {
        let v = fx.mem.i16(t + 6).wrapping_add(fx.mem.i16(t + 8));
        fx.mem.set16(t + 6, v);
    }
    if age(fx, t) != 0 {
        let z = fx.mem.i16(t + 4) + if s0 != 0 { 2 } else { -2 };
        fx.mem.set16(t + 4, z);
    }
    grow(fx, t);
}

/// 0x1E04D20: a band (`ring_band_task`: size 6 growing by +8 = rand % 2 +
/// 3 after each update, turn three rand % 0xFFFF, frame 0, +6 = 0x32,
/// grey 0xB0): alpha to 0x80 on update 0, then down by 0xF (ends below
/// 0x11).
fn band(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 6);
        for (o, ra) in [(0x10u32, 0x01E0_4D48u32), (0x18, 0x01E0_4D64), (0x14, 0x01E0_4D80)] {
            let r = rand_rem(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, r);
        }
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x36, 2);
        let v = fx.rand_from(t, 0x01E0_4DA8);
        fx.mem.set16(t + 8, v.wrapping_rem(2) + 3);
        fx.mem.set16(t + 6, 0x32);
        both(fx, t, 0x80, 0x80, 0x80);
    }
    if age(fx, t) <= 0 {
        at_least_80(fx, t);
    } else {
        fall(fx, t, 0x11, 0xF);
    }
    l2::rgb(fx, t + 0x28, 0xB0, 0xB0, 0xB0);
    grow(fx, t);
    let v = fx.mem.i16(t + 0x38).wrapping_add(fx.mem.i16(t + 8));
    fx.mem.set16(t + 0x38, v);
}

/// 0x1E04E80: a star (`star_draw`: size 0xF, frame cycling 0..7, angle
/// rand % 360, (0xF0, 0xF0, 0x10), radius +6 from 0 by +8 = 3 after
/// update 0, then (0x50, 0x50, 0x10) on age 1 and (0x38, 0x38, 0x80) on
/// age 2): alpha to 0x80 for 4 updates, then down by 0x11 (ends below
/// 0x13).
fn star(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0xF);
        fx.mem.set16(t + 0x34, 0);
        let r = rand_rem(fx, t, 0x01E0_4EAC, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.set16(t + 8, 3);
        fx.mem.set16(t + 6, 0);
        l2::rgb(fx, t + 0x28, 0xF0, 0xF0, 0x10);
    }
    if age(fx, t) < 4 {
        at_least_80(fx, t);
    } else {
        fall(fx, t, 0x13, 0x11);
    }
    cycle_frame(fx, t);
    match age(fx, t) {
        1 => l2::rgb(fx, t + 0x28, 0x50, 0x50, 0x10),
        2 => l2::rgb(fx, t + 0x28, 0x38, 0x38, 0x80),
        _ => {}
    }
    if age(fx, t) != 0 {
        let v = fx.mem.i16(t + 6).wrapping_add(fx.mem.i16(t + 8));
        fx.mem.set16(t + 6, v);
    }
    grow(fx, t);
}

/// 0x1E04FE0: the burst disc (`fan18_far0`: size 0x2BC, (0x10, 0x10, 0x80)
/// / rim (0, 0, 0xF0), angle rand % 360): alpha up by 0xC0 (wrapping) for
/// 10 updates, then down by 0xF (ends below 0x11).
fn disc(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x2BC);
        l2::rgb(fx, t + 0x28, 0x10, 0x10, 0x80);
        l2::rgb(fx, t + 0x2C, 0, 0, 0xF0);
        let r = rand_rem(fx, t, 0x01E0_5034, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if age(fx, t) < 0xA {
        let a = fx.mem.read_u8(t + 0x2B);
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(0xC0));
    } else {
        fall(fx, t, 0x11, 0xF);
    }
    grow(fx, t);
}

/// 0x1E050C0: the camera light D+0x1E90 (camera 0's slots 4 / 5,
/// 0x1F7E20) at the task's point: (128, 3, 3, 0) / (0, 140, level), the
/// level D+0x1EB8 up by 0.5 to 1, then down by 0.3 (ending at 0.3 or
/// less, the slots cleared); it ends with the timer at 1 too.
fn light(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x4040_0000), (8, 0x4040_0000), (0xC, 0), (0x20, 0), (0x24, 0x430C_0000), (0x28, 0)] {
            fx.mem.write_u32(LAMP + o, v);
        }
        for k in 0..3 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(LAMP + 0x10 + 4 * k, v);
        }
        fx.mem.set16(t + 0x36, 2);
        fx.mem.set16(t + 0x30, 0);
    }
    let lv = fx.mem.f(LAMP + 0x28);
    if fx.mem.i16(t + 0x30) == 0 {
        if ee_lt(lv, 1.0) {
            fx.mem.setf(LAMP + 0x28, ee::add(lv, 0.5));
        } else {
            fx.mem.set16(t + 0x30, 1);
        }
    } else {
        let step = f32::from_bits(0x3E99_999A);
        if ee_le(lv, step) {
            fx.mem.set16(t + 0x36, 0);
        } else {
            fx.mem.setf(LAMP + 0x28, ee::sub(lv, step));
        }
    }
    let p = if fx.mem.i16(t + 0x36) == 0 { 0 } else { LAMP };
    bexe::camera_slots45(fx, p);
    if fx.mem.i32(TIMER) == 1 {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// 0x1E05230: the stage light (0x1F7F10) from 0x80 down by 10 to 0x30
/// while the timer is 0x1F or more, then back up by 10 to 0x80.
fn dim(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x30, 0x80);
        fx.mem.set16(t + 0x36, 2);
    }
    let l = fx.mem.i16(t + 0x30);
    if fx.mem.i32(TIMER) >= 0x1F {
        fx.mem.set16(t + 0x30, if l < 0x31 { 0x30 } else { l - 0xA });
    } else {
        fx.mem.set16(t + 0x30, if l < 0x80 { l + 0xA } else { 0x80 });
    }
    let v = fx.mem.read_u8(t + 0x30);
    fx.set_stage_light(i32::from(v));
}

/// The record's spawn 0x1E06200(D, step, 0) (frame 0x60, called from `ra`
/// with the caller's s0..s2 = `s`, s3 = D): the first free of the 110
/// tasks cleared and set off at D+0x250 (as i16) with the colour D+0x3884
/// (twice), +0x30 = D+0x38C6, +0x3A = D+0x38BE.
fn spawn(fx: &mut Fx, f: u32, ra: u32, s: [Option<u32>; 3]) -> i32 {
    let sp = fx.enter(0x60);
    saves(fx, sp, &[(0x50, Some(ra)), (0x20, s[2]), (0x10, s[1]), (0, s[0])]);
    let ret = match (0..110u32).find(|&k| fx.mem.i16(POOL + 0x36 + 0x40 * k) == 0) {
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
            let a = fx.mem.i16(ANGLE);
            fx.mem.set16(t + 0x3A, a);
            fx.mem.write_u32(t + 0x20, 0);
            fx.mem.write_u32(t + 0x24, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// 0x1E02A30(D, 0) (frame 0x70): the spawn colour (0x80, 0, 0, 0), the
/// update counter and the spawns; the history points pushed in z towards
/// the target, and the spawn point left at the last far one.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x70);
    let d = Some(D);
    saves(fx, sp, &[(0x40, Some(0x01E0_5574)), (0x10, d)]);
    fx.saved(sp, &[(0, param as i64 as u64)]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    if counter(fx) == 1 {
        spawn(fx, DIM, 0x01E0_2A88, [None, d, None]);
        let f = fx.sp - 0x60;
        fx.saved(f, &[(0, param as i64 as u64)]);
    }
    let mut s1 = d;
    if [0x44, 0x4D, 0x57, 0x63, 0x6F, 0x7A].contains(&counter(fx)) {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODES);
        ra_slot(fx, 0x01E0_2AEC);
        fx.node_position(h, n, sp + 0x50);
        let c = fx.mem.i16(HCOUNT);
        fx.mem.set16(PARAM, c);
        fx.mem.set16(HCOUNT, c + 1);
        let p = hist(fx.mem.i16(PARAM));
        for k in 0..4 {
            let w = fx.mem.read_u32(sp + 0x50 + 4 * k);
            fx.mem.write_u32(p + 4 * k, w);
        }
        fx.mem.set16(ANGLE, 0);
        let s = [Some(0), Some(0), None];
        spawn(fx, ORB, 0x01E0_2B60, s);
        spawn(fx, ORB_B, 0x01E0_2B74, s);
        spawn(fx, ORB_C, 0x01E0_2B88, s);
        let s = [Some(1), Some(0x1E), None];
        spawn(fx, TRAIL, 0x01E0_2BB0, s);
        let p = hist(fx.mem.i16(PARAM));
        for c in 0..3 {
            let w = fx.mem.read_u32(p + 4 * c);
            fx.mem.write_u32(POS + 4 * c, w);
        }
        spawn(fx, RING, 0x01E0_2C00, s);
        for ra in [0x01E0_2C14u32, 0x01E0_2C28, 0x01E0_2C3C] {
            spawn(fx, FLAME, ra, s);
        }
        spawn(fx, HALO, 0x01E0_2C54, [Some(0), Some(0x1E), None]);
        s1 = Some(0x1E);
    }
    let h = fx.self_handle();
    ra_slot(fx, 0x01E0_2C74);
    fx.unit_rotation(h, sp + 0x60);
    let f = ee::div(ee::from_i32(fx.mem.i32(sp + 0x64)), 180.0);
    let back = fix(fx, f, 0x01E0_2C9C) != 0;
    let mut s2 = None;
    if counter(fx) >= 0xB {
        target_position(fx, sp + 0x50, 0x01E0_2CC0);
        let mut i = 0;
        while i < fx.mem.i16(HCOUNT) {
            let z = hist(i) + 8;
            let dd = dz(fx, sp + 0x58, z, 0x01E0_2CE0);
            if !ee_le(dd, 7.0) {
                let v = fx.mem.f(z);
                fx.mem.setf(z, if back { ee::sub(v, 5.0) } else { ee::add(v, 5.0) });
            }
            i += 1;
        }
        s2 = Some(i as u32);
    }
    if counter(fx) < 0xB4 && fx.mem.i32(TIMER) >= 0x15 {
        for j in 0..2u32 {
            let c = fx.mem.i16(CYC_A);
            fx.mem.set16(PARAM, c);
            spawn(fx, ARC, 0x01E0_2D90, [Some(j), s1, s2]);
            let c = fx.mem.i16(CYC_A) + 1;
            fx.mem.set16(CYC_A, c);
            if fx.mem.i16(CYC_A) >= 0xA {
                fx.mem.set16(CYC_A, 0);
            }
        }
    }
    let mut a = 0;
    while a < fx.mem.i16(HCOUNT) {
        fx.mem.set16(PARAM, a);
        a += 1;
    }
    if counter(fx) >= 0x44 {
        target_position(fx, sp + 0x50, 0x01E0_2E10);
        let mut i = 0;
        while i < fx.mem.i16(HCOUNT) {
            let p = hist(i);
            let dd = dz(fx, sp + 0x58, p + 8, 0x01E0_2E30);
            if !ee_le(dd, 7.0) {
                for c in 0..3 {
                    let w = fx.mem.read_u32(p + 4 * c);
                    fx.mem.write_u32(POS + 4 * c, w);
                }
                let z = ee::add(fx.mem.f(p + 8), 2.5);
                fx.mem.setf(POS + 8, z);
            }
            i += 1;
        }
    }
    fx.leave(0x70);
}

/// 0x204F70 into the scratchpad ([`l2::project`]) with what its 0x1A2C70
/// calls leave below its frame (sp - 0xF0) [F 0x205134..0x205160]: x 16,
/// y 16 and z of each accepted vertex in turn (the projected values are
/// still in the scratchpad), the last one's z frame on top. Card 1's bolt
/// reads that back through its unwritten seventh point.
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
        if ee_lt(z, 1.0) || !ee_le(z, f32::from_bits(0x4B7F_FFFF)) || ee_lt(x, 0.0) || !ee_le(x, lim) || ee_lt(y, 0.0) || !ee_le(y, lim) {
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

/// The textured-quad header (9 values, FST, ALPHA 0x48, vertex alphas 0)
/// of `d`, coloured +0x28 (RGBAQ through `local`).
fn quad_desc(fx: &mut Fx, t: u32, d: u32, local: u32) {
    desc_uv4(fx, d);
    fx.desc_alpha(d, 0x80_0000_0048);
    fx.mem.write_u32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    rgbaq_at(fx, local, t + 0x28, d + 0x10);
}

/// The camera's negated yaw in degrees at `a` (the 0xB60B60B7 `mult`).
fn yaw_degrees_neg(fx: &mut Fx, a: u32) {
    let v = fx.mem.i32(a);
    fx.mult(0xB60B_60B7u32 as i32, v);
    let hi = fx.hilo().1;
    let q = (hi.wrapping_add(v) >> 7).wrapping_add(((v as u32) >> 31) as i32);
    fx.mem.set32(a, q.wrapping_neg());
}

/// 0x1E02590 (D, task, desc, 0) (frame 0x110, called from `ra`): an orb
/// sprite: a camera-facing square of the i16 size +0x38 (UVs D+0x94 in
/// D+0xBE's order, texture D+0x88) at the task's point plus its drift +8 /
/// +0xA / +0xC over 10 (scale 0.1), keyed by its mean w into D+0x80.
fn orb_draw(fx: &mut Fx, t: u32, d: u32, ra: u32, r: &Regs) {
    let sp = fx.enter(0x110);
    saves(fx, sp, &[(0x30, Some(ra)), (0x20, Some(r.s2)), (0x10, Some(D)), (0, Some(r.s0))]);
    quad_desc(fx, t, d, sp + 0x108);
    let order = fx.mem.i16(D + 0xBE);
    uv::uv_rect4(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0x90 + 4 * k, 0x3DCC_CCCD);
    }
    crate::b494::camera_angles_saving(fx, sp + 0xF0, u64::from(d), u64::from(t), 0x01E0_266C);
    for k in 0..3 {
        let p = ee::from_i32(fx.mem.i16(t + 2 * k));
        let o = ee::div(ee::from_i32(fx.mem.i16(t + 8 + 2 * k)), 10.0);
        fx.mem.setf(sp + 0x80 + 4 * k, ee::add(p, o));
    }
    l2::xform_vp(fx, sp + 0xB0, sp + 0x90, sp + 0xF0, sp + 0x80);
    let v = ee::from_i32(fx.mem.i16(t + 0x38));
    l2::quad_xy(fx, sp + 0x40, v, v);
    if proj(fx, sp + 0x40, sp + 0xB0, d, sp + 0xA0) {
        let ot = fx.mem.read_u32(D + 0x80);
        submit_at(fx, d, ot, sp + 0xA0, 0x01E0_2740);
    }
    fx.leave(0x110);
}

/// 0x1E02770 (D, task, desc, 0) (frame 0x140, called from `ra`): twelve
/// camera-facing squares of the i16 size +0x38 (UVs D+0x94 orientation 0,
/// texture D+0x88) turned by the camera's negated yaw, pushed out at
/// radius i16 +6 at angles +0x3A + 30 k (x = r cos, y = r sin), in the
/// task's TRS (point, turn +0x10); each submitted D+0xB8 + 1 times at its
/// mean w into D+0x80.
fn star_draw(fx: &mut Fx, t: u32, d: u32, ra: u32, r: &Regs) {
    let sp = fx.enter(0x140);
    saves(
        fx,
        sp,
        &[(0x70, Some(ra)), (0x60, Some(r.s6)), (0x50, r.s5), (0x40, Some(r.s4)), (0x30, Some(r.s3)), (0x20, Some(r.s2)), (0x10, Some(D)), (0, Some(r.s0))],
    );
    quad_desc(fx, t, d, sp + 0x138);
    uv::uv_rect4(fx, d, D + 0x94, 0);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(sp + 0xC0 + 4 * k, v);
    }
    crate::b494::camera_angles_saving(fx, sp + 0x120, u64::from(r.s0), u64::from(D), 0x01E0_288C);
    yaw_degrees_neg(fx, sp + 0x124);
    l2::xform_vp(fx, sp + 0xD0, 0, t + 0x10, sp + 0xC0);
    let mut a = 0i32;
    for _ in 0..0xC {
        let v = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x80, v, v);
        l2::rotate4(fx, sp + 0x80, sp + 0x120, 0, D + 0xD8);
        let deg = (fx.mem.i16(t + 0x3A) + a) as i16 as i32;
        let c = l2::sin_deg(fx, D + 0xD8, deg, 1);
        let rr = ee::from_i32(fx.mem.i16(t + 6));
        fx.mem.setf(sp + 0xC0, ee::mul(rr, c));
        let s = l2::sin_deg(fx, D + 0xD8, deg, 0);
        let rr = ee::from_i32(fx.mem.i16(t + 6));
        fx.mem.setf(sp + 0xC4, ee::mul(rr, s));
        let x = fx.mem.f(sp + 0xC0);
        l2::add_column(fx, sp + 0x80, 0, x);
        let y = fx.mem.f(sp + 0xC4);
        l2::add_column(fx, sp + 0x80, 1, y);
        if proj(fx, sp + 0x80, sp + 0xD0, d, sp + 0x110) {
            let mut n = 0;
            while n < fx.mem.i16(D + 0xB8) + 1 {
                let ot = fx.mem.read_u32(D + 0x80);
                submit_at(fx, d, ot, sp + 0x110, 0x01E0_29C4);
                n += 1;
            }
        }
        a = (a + 0x1E) as i16 as i32;
    }
    fx.leave(0x140);
}

/// RGBAQ of the colour at `c` (0x108020 into `local`) to `to`.
fn rgbaq_to(fx: &mut Fx, local: u32, c: u32, to: u32) {
    l2::rgbaq(fx, local, c);
    for k in 0..2 {
        let w = fx.mem.read_u32(local + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn copy3(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// The strip3 header of the record's ribbons in `d` (0x109B50, UVs D+0x94
/// in D+0xBE's order, texture D+0x88 filter 1).
fn strip_desc(fx: &mut Fx, d: u32) {
    l2::desc_strip3(fx, d);
}

fn strip_tex(fx: &mut Fx, d: u32) {
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
}

/// 0x1E00D10 (D, task, pts) (frame 0x250, called from `ra`): a crossed
/// textured ribbon along the vec4 history `pts` (i16 +6 = n points): per
/// segment four curve pieces (basis 0, t and t + 1/4; the first segment
/// on stack copies p0 p0 p1 p2), the near pair coloured +0x28 as it
/// stands and the far pair with the alpha less +0x2B / 3n (clamped at 0);
/// each piece drawn twice: pushed by w = +0x38 / 10 in y (+, -), then in
/// x (+, -) with z by w / 2; keyed by the mean w into 0x206E40's table.
fn trail_draw(fx: &mut Fx, t: u32, pts: u32, ra: u32, r: &Regs) {
    use ee::{add, sub};
    let sp = fx.enter(0x250);
    saves(
        fx,
        sp,
        &[
            (0xA0, Some(ra)),
            (0x80, Some(r.s7)),
            (0x70, Some(r.s6)),
            (0x60, r.s5),
            (0x50, Some(r.s4)),
            (0x40, Some(r.s3)),
            (0x30, Some(r.s2)),
            (0x20, Some(D)),
            (0x10, Some(r.s0)),
        ],
    );
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 0x38)), 10.0);
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x198;
    l2::fullscreen4(fx, d);
    strip_desc(fx, d);
    strip_tex(fx, d);
    let c = sp + 0x24C;
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(c + k, v);
    }
    let n3 = 3 * n;
    copy4(fx, sp + 0xE0, pts);
    copy4(fx, sp + 0xF0, pts);
    copy4(fx, sp + 0x100, pts + 0x10);
    copy4(fx, sp + 0x110, pts + 0x20);
    fx.mem.set32(sp + 0xB0, n - 1);
    let mut s0 = 0i32;
    while s0 < fx.mem.i32(sp + 0xB0) {
        let (q, end) = if s0 != 0 { (pts.wrapping_add((0x10 * (s0 - 1)) as u32), 0x170u32) } else { (sp + 0xE0, 0x180) };
        let mut f20 = 0.0f32;
        for _ in 0..4 {
            spline::spline(fx, sp + 0xC0, q, f20, 0, [0.0; 4]);
            f20 = add(f20, 0.25);
            spline::spline(fx, sp + end, q, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0xD0, sp + end);
            rgbaq_to(fx, sp + 0x228, c, d + 0x10);
            rgbaq_to(fx, sp + 0x230, c, d + 0x28);
            let al = i32::from(fx.mem.read_u8(t + 0x2B));
            let dq = fx.div(al, n3).0 as i16 as i32;
            let v = i32::from(fx.mem.read_u8(c + 3)) - dq;
            fx.mem.write_u8(c + 3, if v > 0 { v as u8 } else { 0 });
            rgbaq_to(fx, sp + 0x238, c, d + 0x40);
            rgbaq_to(fx, sp + 0x240, c, d + 0x58);
            for s1 in 0..2 {
                for (to, from) in [(0x120u32, 0xC0u32), (0x130, 0xC0), (0x140, 0xD0), (0x150, 0xD0)] {
                    copy3(fx, sp + to, sp + from);
                }
                let adj = |fx: &mut Fx, a: u32, by: f32, up: bool| {
                    let x = fx.mem.f(sp + a);
                    fx.mem.setf(sp + a, if up { add(x, by) } else { sub(x, by) });
                };
                if s1 == 0 {
                    adj(fx, 0x124, w, true);
                    adj(fx, 0x144, w, true);
                    adj(fx, 0x134, w, false);
                    adj(fx, 0x154, w, false);
                } else {
                    let h = ee::div(w, 2.0);
                    adj(fx, 0x120, w, true);
                    adj(fx, 0x140, w, true);
                    adj(fx, 0x130, w, false);
                    adj(fx, 0x150, w, false);
                    adj(fx, 0x128, h, true);
                    adj(fx, 0x148, h, true);
                    adj(fx, 0x138, h, false);
                    adj(fx, 0x158, h, false);
                }
                if proj(fx, sp + 0x120, 0, d, sp + 0x160) {
                    submit_at(fx, d, OT, sp + 0x160, 0x01E0_1180);
                }
            }
        }
        s0 += 1;
    }
    fx.leave(0x250);
}

/// The ribbons' per-segment colours: vertices 0 / 1 the colours at `ca` /
/// `cb` (`ca` for both without `rim`) as they stand, then each alpha less the task's +0x2B / +0x2F over
/// `m` (wrapping; the second a `div1`), vertices 2 / 3 after; RGBAQ
/// locals from `local`.
fn segment_colours(fx: &mut Fx, t: u32, d: u32, ca: u32, local: u32, m: i32, rim: bool) {
    let cb = ca + 4;
    let odd = if rim { cb } else { ca };
    rgbaq_to(fx, local, ca, d + 0x10);
    rgbaq_to(fx, local + 8, odd, d + 0x28);
    let a = i32::from(fx.mem.read_u8(t + 0x2B));
    let q = fx.div(a, m).0;
    let v = fx.mem.read_u8(ca + 3).wrapping_sub(q as u8);
    fx.mem.write_u8(ca + 3, v);
    let a = i32::from(fx.mem.read_u8(t + 0x2F));
    let q = if m != 0 { a.wrapping_div(m) } else { 0 };
    let v = fx.mem.read_u8(cb + 3).wrapping_sub(q as u8);
    fx.mem.write_u8(cb + 3, v);
    rgbaq_to(fx, local + 0x10, ca, d + 0x40);
    rgbaq_to(fx, local + 0x18, odd, d + 0x58);
}

/// The strip3 of the arc / bolt draws in `d`: header, RGBAQs +0x28 / +0x2C
/// / +0x28 / +0x2C through `local`, UVs and texture; the colours copied to
/// `c` / `c` + 4.
fn arc_desc(fx: &mut Fx, t: u32, d: u32, local: u32, c: u32) {
    strip_desc(fx, d);
    for (k, (src, to)) in [(0x28u32, 0x10u32), (0x2C, 0x28), (0x28, 0x40), (0x2C, 0x58)].into_iter().enumerate() {
        rgbaq_to(fx, local + 8 * k as u32, t + src, d + to);
    }
    strip_tex(fx, d);
    for k in 0..8 {
        let v = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(c + k, v);
    }
}

/// 0x1E01CD0 (D, task, pts) (frame 0x2A0): the arc: a chain of camera-
/// facing bands through its i16 +6 points `pts` (each segment p(i) ..
/// p(i + 1) widened by +-size along the camera's yawed x), in the TRS of
/// the task's point and its scale +8 / +0xA / +0xC over 10; colours +0x28
/// / +0x2C with the alphas fading by +0x2B / +0x2F over n - 1 a segment;
/// keyed by the mean w into 0x206E40's table.
fn arc_draw(fx: &mut Fx, t: u32, pts: u32, r: &Regs) {
    let sp = fx.enter(0x2A0);
    saves(
        fx,
        sp,
        &[
            (0x90, Some(0x01E0_5B8C)),
            (0x70, Some(r.s7)),
            (0x60, Some(r.s6)),
            (0x50, r.s5),
            (0x40, Some(r.s4)),
            (0x30, Some(r.s3)),
            (0x20, Some(r.s2)),
            (0x10, Some(D)),
            (0, Some(r.s0)),
        ],
    );
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x1C8;
    let c = sp + 0x298;
    arc_desc(fx, t, d, sp + 0x258, c);
    for k in 0..3 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(sp + 0xD0 + 4 * k, v);
        let v = ee::from_i32(fx.mem.i16(t + 8 + 2 * k));
        fx.mem.setf(sp + 0xE0 + 4 * k, v);
    }
    for k in 0..3 {
        let v = ee::div(fx.mem.f(sp + 0xE0 + 4 * k), 10.0);
        fx.mem.setf(sp + 0xE0 + 4 * k, v);
    }
    crate::b494::camera_angles_saving(fx, sp + 0x130, n as i64 as u64, u64::from(d), 0x01E0_1EDC);
    yaw_degrees_neg(fx, sp + 0x134);
    l2::xform_vp(fx, sp + 0xF0, sp + 0xE0, 0, sp + 0xD0);
    fx.mem.set32(sp + 0xB0, n - 1);
    fx.mem.set32(sp + 0xA0, 0);
    let mut p = pts;
    while fx.mem.i32(sp + 0xA0) < fx.mem.i32(sp + 0xB0) {
        let m = fx.mem.i32(sp + 0xB0);
        segment_colours(fx, t, d, c, sp + 0x278, m, true);
        let v = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x180, v, 0.0);
        l2::rotate4(fx, sp + 0x180, sp + 0x130, 0, D + 0xD8);
        for (to, from) in [(0x140u32, p), (0x150, p), (0x160, p + 0x10), (0x170, p + 0x10)] {
            copy3(fx, sp + to, from);
        }
        for k in 0..4u32 {
            add3(fx, sp + 0x140 + 0x10 * k, sp + 0x180 + 0x10 * k);
        }
        if proj(fx, sp + 0x140, sp + 0xF0, d, sp + 0xC0) {
            submit_at(fx, d, OT, sp + 0xC0, 0x01E0_2104);
        }
        p += 0x10;
        let i = fx.mem.i32(sp + 0xA0) + 1;
        fx.mem.set32(sp + 0xA0, i);
    }
    fx.leave(0x2A0);
}

/// 0x107430(a, b): a.xyz += b.xyz.
fn add3(fx: &mut Fx, a: u32, b: u32) {
    for k in 0..3 {
        let v = ee::add(fx.mem.f(a + 4 * k), fx.mem.f(b + 4 * k));
        fx.mem.setf(a + 4 * k, v);
    }
}

/// 0x1E06330 (D, task, pts, extra) (frame 0x3E0): a bolt: its i16 +6
/// points `pts` jittered by rand % 0x14 - 0xA in x and z (a stack copy),
/// a ribbon through them (basis 0, t and t + 1/2; the first segment p0
/// p0 p1 p2 with p0 unjittered) of flat bands +-w in x, w = +0x38 / 10
/// falling by 0.1 a band, in the TRS of the task's point and the camera's
/// angles plus the turn +0x10 / +0x14 / +0x18 (+ the word `extra` + 8);
/// colours as the arc's; keyed by the mean w into 0x206E40's table.
fn bolt_draw(fx: &mut Fx, t: u32, pts: u32, extra: u32, r: &Regs) {
    use ee::add;
    let sp = fx.enter(0x3E0);
    saves(
        fx,
        sp,
        &[
            (0xA0, Some(0x01E0_5C24)),
            (0x80, Some(r.s7)),
            (0x70, Some(r.s6)),
            (0x60, r.s5),
            (0x50, Some(r.s4)),
            (0x40, Some(r.s3)),
            (0x30, Some(r.s2)),
            (0x20, Some(D)),
            (0x10, Some(r.s0)),
        ],
    );
    copy4(fx, sp + 0xC0, extra);
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x308;
    let c = sp + 0x3D8;
    arc_desc(fx, t, d, sp + 0x398, c);
    for k in 0..3 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(sp + 0xE0 + 4 * k, v);
    }
    crate::b494::camera_angles_saving(fx, sp + 0x130, u64::from(pts), u64::from(d), 0x01E0_64F0);
    let j = sp + 0x140;
    for i in 0..n.max(0) as u32 {
        copy3(fx, j + 0x10 * i, pts + 0x10 * i);
    }
    for i in 0..n.max(0) as u32 {
        for (o, ra) in [(0u32, 0x01E0_654Cu32), (8, 0x01E0_6580)] {
            let v = fx.rand_from(pts, ra);
            let q = fx.div(v, 0x14).1 - 0xA;
            let x = add(fx.mem.f(j + 0x10 * i + o), ee::from_i32(q));
            fx.mem.setf(j + 0x10 * i + o, x);
        }
    }
    copy4(fx, sp + 0x2A0, pts);
    copy4(fx, sp + 0x2B0, pts);
    copy4(fx, sp + 0x2C0, sp + 0x150);
    copy4(fx, sp + 0x2D0, sp + 0x160);
    let mut f21 = ee::div(ee::from_i32(fx.mem.i16(t + 0x38)), 10.0);
    for (a, o) in [(0x130u32, 0x10u32), (0x134, 0x14)] {
        let v = fx.mem.i32(sp + a).wrapping_add(fx.mem.i32(t + o));
        fx.mem.set32(sp + a, v);
    }
    let v = fx.mem.i32(sp + 0x138).wrapping_add(fx.mem.i32(t + 0x18).wrapping_add(fx.mem.i32(sp + 0xC8)));
    fx.mem.set32(sp + 0x138, v);
    l2::xform_vp(fx, sp + 0xF0, 0, sp + 0x130, sp + 0xE0);
    fx.mem.set32(sp + 0xB0, n - 1);
    let mut s6 = 0i32;
    while s6 < fx.mem.i32(sp + 0xB0) {
        let m = fx.mem.i32(sp + 0xB0);
        segment_colours(fx, t, d, c, sp + 0x3B8, m, false);
        let mut f20 = 0.0f32;
        let q = (sp + 0x140).wrapping_add((0x10 * (s6 - 1)) as u32);
        for _ in 0..2 {
            l2::quad_xy(fx, sp + 0x220, f21, 0.0);
            f21 = ee::sub(f21, f32::from_bits(0x3DCC_CCCD));
            let (src, end) = if s6 != 0 { (q, 0x2E0u32) } else { (sp + 0x2A0, 0x2F0) };
            spline::spline(fx, sp + 0x260, src, f20, 0, [0.0; 4]);
            f20 = add(f20, 0.5);
            spline::spline(fx, sp + end, src, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0x270, sp + end);
            for (to, from) in [(0x1E0u32, 0x260u32), (0x1F0, 0x260), (0x200, 0x270), (0x210, 0x270)] {
                copy3(fx, sp + to, sp + from);
            }
            for k in 0..4u32 {
                add3(fx, sp + 0x1E0 + 0x10 * k, sp + 0x220 + 0x10 * k);
            }
            if proj(fx, sp + 0x1E0, sp + 0xF0, d, sp + 0xD0) {
                submit_at(fx, d, OT, sp + 0xD0, 0x01E0_6940);
            }
        }
        s6 += 1;
    }
    fx.leave(0x3E0);
}
