//! BMODULE b267 (card 72), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01E60, data area 0x1ED0000, 0x3FA0 bytes) [F]
//!: a pool of 220 tasks of 0x40 bytes at D+0x4D4
//! (alive +0x36, age +0x32, step +0x24; the entry calls each step by its
//! address with `jal`), spawned by 0x1E02520 (the first free task cleared,
//! colours D+0x3DD4 at +0x28 / +0x2C, the point D+0x2D0 as i16 at +0,
//! D+0x3E2A at +0x30, D+0x3E22 at +0x3A). The emitter 0x1E00260 lays five
//! points D+0x3C10 + 0x10 k about the attacker on updates 0x1C / 0x33 /
//! 0x49 / 0x5F / 0x78 (each a pair of 0x1E00E50 quads and a 0x1E01610
//! burst), the pentagram 0x1E01740 (lines between them, 0x1E03A70) on 0x7D,
//! on 0x96 a ring (0x1E00A80) and a glow (0x1E00BF0) at node 0x3F, then
//! fans (0x1E00F90 at the node, 0x1E01260 at its update-0x96 point) on
//! 0x97..0x1EE, sprites at the target (0x1E01500) from 0xFB and the frame
//! window 0x1E01930 on 0x163. Draws (each twice: the header with a3 0, the
//! geometry with 1): ring strips 0x1E028C0, glow quarters 0x1E02650,
//! billboards 0x1E02C60 / 0x1E03820, fans 0x1E02F10 / 0x1E033B0,
//! `quads4::quarters_far` (0x1E00E50). Returns 1 / 3 / 6 / 8 on 0x181 /
//! 0x1F4 / 0x18C..0x1EE / timer out. Steps 0x1E00860 / 0x1E01820 /
//! 0x1E018E0 are tested by the entry but never spawned (not ported).
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0xDC;
const RING: u32 = 0x01E0_0A80;
const GLOW: u32 = 0x01E0_0BF0;
const QUADS: u32 = 0x01E0_0E50;
const FAN: u32 = 0x01E0_0F90;
const FAN_B: u32 = 0x01E0_1260;
const SPRITE: u32 = 0x01E0_1500;
const BURST: u32 = 0x01E0_1610;
const STAR: u32 = 0x01E0_1740;
const WINDOW: u32 = 0x01E0_1930;
const POS: u32 = D + 0x2D0;
const TEX: u32 = D + 0x320;
const POINTS: u32 = D + 0x3C10;
const HOLD: u32 = D + 0x3DC0;
const TEX5: u32 = D + 0x3DA0;
const TIMER: u32 = D + 0x3DD0;
const COLOUR: u32 = D + 0x3DD4;
const OT_AT: u32 = D + 0x3DDC;
const NODES: u32 = D + 0x3E18;
const ANGLE: u32 = D + 0x3E22;
const COUNTER: u32 = D + 0x3E2E;
const SINES: u32 = D + 0x3E34;
/// The scratchpad, where the originals project.
const SPR: u32 = 0x7000_0000;
/// 0.5 as a double.
const D05: u64 = 0x3FE0_0000_0000_0000;

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

fn rand_mod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn copy3(fx: &mut Fx, to: u32, from: u32) {
    for c in [0u32, 4, 8] {
        let w = fx.mem.read_u32(from + c);
        fx.mem.write_u32(to + c, w);
    }
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// The entry 0x1E01E60 (frame 0x130: s0 = D, s1 = the param / the step
/// loop's index, s2 the drawn count, s3 the slot).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x130);
    fx.mem.write_u32(OT_AT, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x130);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3FA0);
    let p = param as i64 as u64;
    l2::sine_table_saving(fx, SINES, u64::from(D), p, 0x01E0_1EB0);
    l2::sine_table_saving(fx, D + 0xD8, u64::from(D), p, 0x01E0_1EB8);
    fx.mem.set16(D + 0x3E2C, param);
    fx.mem.set32(TIMER, 0x200);
    for o in [0x310u32, 0x314, 0x318] {
        fx.mem.write_u32(D + o, 0);
    }
    let s = [Some(u64::from(D)), Some(p), Some(0)];
    for (k, (n, clear, ra)) in [(0x4A, 1, 0x01E0_1EE4u32), (0x3F, 0, 0x01E0_1EFC), (0xD, 0, 0x01E0_1F14)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = crate::b076::request_node_saving(fx, h, n, clear, s, ra);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    textures(fx, p);
    0
}

/// 0x1E01DB0(0, D): textures (4, 0..4) at D+0x320.., (4, 0xE / 0xF) at
/// D+0x358.., (5, 0 / 1) at D+0x3DA0...
fn textures(fx: &mut Fx, param: u64) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_1F24), (0x20, 0), (0x10, param), (0, u64::from(D))]);
    for k in 0..5u32 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(TEX + 4 * k, t);
    }
    for k in 0xEu32..0x10 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x358 + 4 * (k - 0xE), t);
    }
    for k in 0..2u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX5 + 4 * k, t);
    }
    fx.leave(0x40);
}

/// 0x1A2E08's unpack of the double `x` at `p` (b117's).
fn unpack_d(fx: &mut Fx, x: u64, p: u32) {
    fx.mem.write_u32(p + 4, (x >> 63) as u32);
    let e = ((x >> 52) & 0x7FF) as u32;
    let m = x & 0xF_FFFF_FFFF_FFFF;
    if e == 0 {
        fx.mem.write_u32(p, 2);
    } else if e == 0x7FF {
        if m == 0 {
            fx.mem.write_u32(p, 4);
        } else {
            fx.mem.write_u32(p, u32::from(m & (1 << 51) != 0));
            fx.mem.write_u64(p + 0x10, m);
        }
    } else {
        fx.mem.write_u64(p + 0x10, (m << 8) | (1 << 60));
        fx.mem.write_u32(p + 8, e.wrapping_sub(0x3FF));
        fx.mem.write_u32(p, 3);
    }
}

/// (double) `v` < `b` through 0x1A2C30 (from `ra1`) and 0x17BCA0 (from
/// `ra2`) with the frames they leave (b117's `dlt`).
fn dlt(fx: &mut Fx, v: f32, b: u64, s0: u32, ra1: u32, ra2: u32) -> bool {
    let sp = fx.sp;
    fx.fix_residue(v, ra1);
    let f = sp - 0x30;
    let (class, sign, exp, mant) = (fx.mem.read_u32(f), fx.mem.read_u32(f + 4), fx.mem.read_u32(f + 8), fx.mem.read_u32(f + 0xC));
    let g = sp - 0x60;
    fx.mem.write_u32(g, class);
    fx.mem.write_u32(g + 4, sign);
    fx.mem.write_u32(g + 8, exp);
    fx.mem.write_u64(g + 0x10, u64::from(mant) << 30);
    fx.mem.write_u64(g + 0x20, 0x001A_2C60);
    let bits = v.to_bits();
    let a = if (bits >> 23) & 0xFF == 0 { u64::from(bits >> 31) << 63 } else { f64::from(v).to_bits() };
    fx.saved(sp - 0x10, &[(0, u64::from(ra2))]);
    let h = sp - 0x70;
    fx.mem.write_u64(h + 0x40, a);
    fx.mem.write_u64(h + 0x48, b);
    fx.mem.write_u64(h + 0x50, u64::from(s0));
    fx.mem.write_u64(h + 0x58, 0x0017_BCB0);
    unpack_d(fx, a, h);
    unpack_d(fx, b, h + 0x20);
    f64::from_bits(a) < f64::from_bits(b)
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let slot = D + 0x40 * k;
        if fx.mem.i16(slot + 0x50A) == 0 {
            continue;
        }
        let t = slot + 0x4D4;
        let f = fx.mem.read_u32(slot + 0x4F8);
        let e = [u64::from(D), u64::from(k)];
        match f {
            RING => ring(fx, t, e, 0x01E0_1F90),
            GLOW => glow(fx, t, e, 0x01E0_1FB0),
            QUADS => quads(fx, t, e, 0x01E0_1FD0),
            FAN => fan_step(fx, t, e, 0x01E0_1FF0),
            FAN_B => fan_b_step(fx, t, e, 0x01E0_2010),
            SPRITE => sprite(fx, t, e, 0x01E0_2030),
            BURST => burst(fx, t, e, 0x01E0_2050),
            STAR => star(fx, t, e, 0x01E0_2070),
            WINDOW => window(fx, t, e, 0x01E0_2100),
            0x01E0_0860 | 0x01E0_1820 | 0x01E0_18E0 => panic!("b267: step {f:#x} is never spawned"),
            _ => {}
        }
    }
    let h = fx.target_handle();
    crate::b076::unit_position_saving(fx, h, sp + 0x60, u64::from(D), 0x01E0_2124);
    let z = ee::sub(fx.mem.f(sp + 0x68), 25.0);
    fx.mem.setf(sp + 0x68, z);
    let d = sp + 0x98;
    for k in 0..TASKS {
        let slot = D + 0x40 * k;
        if fx.mem.i16(slot + 0x50A) == 0 {
            continue;
        }
        let t = slot + 0x4D4;
        let f = fx.mem.read_u32(slot + 0x4F8);
        match f {
            RING => {
                ring_draw(fx, t, d, 0);
                ring_draw(fx, t, d, 1);
            }
            GLOW => {
                glow_draw(fx, t, d, 0);
                glow_draw(fx, t, d, 1);
            }
            QUADS => {
                let w = fx.mem.read_u32(D + 0x35C);
                fx.mem.write_u32(D + 0x88, w);
                l2::rect(fx, sp + 0x80, 0x9A, 1);
                copy4(fx, D + 0x94, sp + 0x80);
                for (c, o) in [(0u32, 0x60u32), (2, 0x64), (4, 0x68)] {
                    let v = ee::from_i32(fx.mem.i16(t + c));
                    fx.mem.setf(sp + o, v);
                }
                crate::blib::quads4::quarters_far(fx, D, t, d, sp + 0x60);
            }
            FAN => {
                fan_draw(fx, t, d, 0, &FAN_A_KIND);
                fan_draw(fx, t, d, 1, &FAN_A_KIND);
            }
            FAN_B => {
                fan_draw(fx, t, d, 0, &FAN_B_KIND);
                fan_draw(fx, t, d, 1, &FAN_B_KIND);
            }
            SPRITE => {
                billboard(fx, t, d, 0, &SPRITE_KIND);
                billboard(fx, t, d, 1, &SPRITE_KIND);
            }
            BURST => {
                billboard(fx, t, d, 0, &BURST_KIND);
                billboard(fx, t, d, 1, &BURST_KIND);
            }
            STAR => star_draw(fx, t, d),
            _ => {}
        }
    }
    if counter(fx) >= 0x15 {
        let v = fx.mem.f(D + 0x310);
        if dlt(fx, v, D05, D, 0x01E0_2408, 0x01E0_2418) {
            let v = ee::add(fx.mem.f(D + 0x310), f32::from_bits(0x3C23_D70A));
            fx.mem.setf(D + 0x310, v);
        }
        fx.mem.write_u32(D + 0x314, 0x3DCC_CCCD);
        let v = fx.mem.f(D + 0x318);
        if dlt(fx, v, D05, D, 0x01E0_2450, 0x01E0_2460) {
            let v = ee::add(fx.mem.f(D + 0x318), f32::from_bits(0x3C23_D70A));
            fx.mem.setf(D + 0x318, v);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0x181 => 1,
        0x1F4 => 3,
        0x18C..=0x1EE => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E02520(D, step, 0) from the emitter (its s0 = `s0`, s1 = D; the
/// entry's s2 = 0 and the runner's s3 / s4 (0 [H])).
fn spawn(fx: &mut Fx, f: u32, s0: u64, ra: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, 0), (0x10, u64::from(D)), (0, s0)]);
    if let Some(k) = (0..TASKS).find(|&k| fx.mem.i16(D + 0x40 * k + 0x50A) == 0) {
        let t = D + 0x40 * k + 0x4D4;
        fx.bzero(t, 0x40);
        fx.mem.set16(t + 0x36, 1);
        for o in [0x28u32, 0x2C] {
            let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
            l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
            let a = fx.mem.read_u8(COLOUR + 3);
            fx.mem.write_u8(t + o + 3, a);
        }
        l2::to_i16x4(fx, t, POS, 0);
        let v = fx.mem.i16(D + 0x3E2A);
        fx.mem.set16(t + 0x30, v);
        let v = fx.mem.i16(ANGLE);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.write_u32(t + 0x20, 0);
        fx.mem.write_u32(t + 0x24, f);
    }
    fx.leave(0x60);
}

/// 0x1E00260(D, 0) (frame 0x70: f20 +0, the entry's s0 = D +0x10, s1 = the
/// param +0x20, ra +0x30).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x70);
    fx.saved(sp + 0x10, &[(0x20, 0x01E0_1F38), (0x10, param as i64 as u64), (0, u64::from(D))]);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x50);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0x54)), 180.0);
    fx.fix_residue(v, 0x01E0_02B0);
    let facing = ee::fixsfsi(v) as i16 as i32;
    let mut s0 = facing as i64 as u64;
    for (k, v) in [0x80u8, 0, 0, 0].into_iter().enumerate() {
        fx.mem.write_u8(COLOUR + k as u32, v);
    }
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    let h = fx.self_handle();
    crate::b076::unit_position_saving(fx, h, sp + 0x40, s0, 0x01E0_02E8);
    let c = counter(fx);
    if let Some(i) = [0x1C, 0x33, 0x49, 0x5F, 0x78].iter().position(|&x| x == c) {
        let xs: [u32; 5] = if facing != 0 {
            [0xC040_0000, 0x4040_0000, 0xC000_0000, 0x4000_0000, 0]
        } else {
            [0x4040_0000, 0xC040_0000, 0x4000_0000, 0xC000_0000, 0]
        };
        fx.mem.write_u32(POS, xs[i]);
        let f20 = [2.0f32, 2.0, -2.0, -2.0, 4.0][i];
        let y = ee::add(fx.mem.f(sp + 0x44), f20);
        fx.mem.setf(POS + 4, y);
        let z = fx.mem.read_u32(sp + 0x48);
        fx.mem.write_u32(POS + 8, z);
        copy4(fx, POINTS + 0x10 * i as u32, POS);
        fx.mem.set16(ANGLE, 0);
        spawn(fx, QUADS, s0, 0x01E0_0598);
        fx.mem.set16(ANGLE, 0x5A);
        spawn(fx, QUADS, s0, 0x01E0_05B4);
        spawn(fx, BURST, s0, 0x01E0_05C8);
    }
    if counter(fx) == 0x7D {
        spawn(fx, STAR, s0, 0x01E0_05EC);
    }
    let h = fx.self_handle();
    let n = fx.mem.i16(NODES + 2);
    crate::b076::node_position_saving(fx, h, n, sp + 0x40, s0, Some(u64::from(D)), 0x01E0_0604);
    if counter(fx) == 0x96 {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODES + 2);
        crate::b076::node_position_saving(fx, h, n, HOLD, s0, Some(u64::from(D)), 0x01E0_062C);
    }
    if counter(fx) == 0x96 {
        let h = fx.self_handle();
        fx.unit_rotation(h, sp + 0x60);
        s0 = 0;
        copy3(fx, POS, sp + 0x40);
        spawn(fx, GLOW, s0, 0x01E0_0680);
        s0 = 1;
    }
    if counter(fx) == 0x96 {
        copy3(fx, POS, sp + 0x40);
        spawn(fx, RING, s0, 0x01E0_06CC);
    }
    fx.camera_at30(0);
    let c = counter(fx);
    if (0x97..0x1EF).contains(&c) {
        copy3(fx, POS, sp + 0x40);
        spawn(fx, FAN, s0, 0x01E0_0720);
    }
    let c = counter(fx);
    if (0x97..0x1EF).contains(&c) {
        copy3(fx, POS, HOLD);
        spawn(fx, FAN_B, s0, 0x01E0_0768);
    }
    let c = counter(fx);
    if (0xFB..0x1EF).contains(&c) {
        let h = fx.target_handle();
        crate::b076::unit_position_saving(fx, h, sp + 0x40, s0, 0x01E0_0798);
        for (o, ra) in [(0u32, 0x01E0_07A0u32), (4, 0x01E0_07D4)] {
            let r = rand_mod(fx, s0 as u32, ra, 0x1E);
            let v = ee::sub(ee::add(fx.mem.f(sp + 0x40 + o), ee::from_i32(r)), 15.0);
            fx.mem.setf(POS + o, v);
        }
        let z = fx.mem.read_u32(sp + 0x48);
        fx.mem.write_u32(POS + 8, z);
        spawn(fx, SPRITE, s0, 0x01E0_0820);
    }
    if counter(fx) == 0x163 {
        spawn(fx, WINDOW, s0, 0x01E0_0844);
    }
    fx.leave(0x70);
}

/// A step's prologue (frame `frame`: the entry's s0 = D, s1 = the index
/// (the first `n`) at +0 / +0x10, ra at `ra_at`).
fn enter_step(fx: &mut Fx, frame: u32, n: u32, ra_at: u32, e: [u64; 2], ra: u32) {
    let sp = fx.enter(frame);
    for k in 0..n {
        fx.saved(sp + 0x10 * k, &[(0, e[k as usize])]);
    }
    fx.saved(sp, &[(ra_at, u64::from(ra))]);
}

fn first(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x36) == 1
}

fn age(fx: &mut Fx, t: u32) -> i32 {
    fx.mem.i16(t + 0x32)
}

fn grow_age(fx: &mut Fx, t: u32) {
    let n = age(fx, t) + 1;
    fx.mem.set16(t + 0x32, n);
}

fn odd_age(fx: &mut Fx, t: u32) -> bool {
    age(fx, t).wrapping_rem(2) == 1
}

fn add16(fx: &mut Fx, a: u32, d: i32) {
    let v = fx.mem.i16(a).wrapping_add(d);
    fx.mem.set16(a, v);
}

/// The byte `a` up by `up` while below `cap`.
fn fade_in(fx: &mut Fx, t: u32, a: u32, cap: u8, up: u8) {
    let v = fx.mem.read_u8(t + a);
    if v < cap {
        fx.mem.write_u8(t + a, v.wrapping_add(up));
    }
}

/// The alpha +0x2B down by `down` while at least `floor`, else the task
/// ends.
fn fade_out(fx: &mut Fx, t: u32, floor: u8, down: u8) {
    let v = fx.mem.read_u8(t + 0x2B);
    if v >= floor {
        fx.mem.write_u8(t + 0x2B, v.wrapping_sub(down));
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// 0x1E00A80 (frame 0x30): the ring (radius 0x5A, turned rand % 360),
/// dark grey; in by 8 below 0xA0 for 20 updates, then out by 0xD once the
/// timer is below 20; from update 0x119 spinning by 0x30 to 0x600 and its
/// radius back to 0x5A by 4; its height +0x30 down by 4 to -0x1D.
fn ring(fx: &mut Fx, t: u32, e: [u64; 2], ra: u32) {
    enter_step(fx, 0x30, 2, 0x20, e, ra);
    l2::rgb(fx, t + 0x28, 0x1F, 0x1F, 0x1F);
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x5A);
        let r = rand_mod(fx, t, 0x01E0_0AC8, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x30, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 0x14 {
        fade_in(fx, t, 0x2B, 0xA0, 8);
    } else if fx.mem.i32(TIMER) < 0x14 {
        fade_out(fx, t, 0xF, 0xD);
    }
    if counter(fx) >= 0x119 && fx.mem.i16(t + 0x3A) < 0x600 {
        add16(fx, t + 0x3A, 0x30);
    }
    if counter(fx) >= 0x119 {
        let v = fx.mem.i16(t + 0x38);
        fx.mem.set16(t + 0x38, if v < 0x5A { v + 4 } else { 0x5A });
    }
    if fx.mem.i16(t + 0x30) >= -0x1D {
        add16(fx, t + 0x30, -4);
    }
    grow_age(fx, t);
    add16(fx, t + 0x3A, 1);
    fx.leave(0x30);
}

/// 0x1E00BF0 (frame 0x30): the glow (size 4 growing by 2 to 50), black
/// brightening by 4 to 0x60 (alpha by 7 to 0x90) for 20 updates, then
/// while the timer is above 20 pulsing between 0x30 and 0xC0 by 4; out by
/// 0xD below timer 20; from update 0x119 spinning by 0x30 to 0x600 and
/// growing by 1 to 0x5A.
fn glow(fx: &mut Fx, t: u32, e: [u64; 2], ra: u32) {
    enter_step(fx, 0x30, 2, 0x20, e, ra);
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 4);
        l2::rgb(fx, t + 0x28, 0, 0, 0);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0xE, 1);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 0x14 {
        fade_in(fx, t, 0x2B, 0x90, 7);
        for o in [0x28u32, 0x29, 0x2A] {
            fade_in(fx, t, o, 0x60, 4);
        }
    }
    if fx.mem.i32(TIMER) >= 0x15 {
        let v = fx.mem.i16(t + 0x30);
        if v == 0 {
            if fx.mem.i16(t + 0x38) >= 0x32 {
                fade_in(fx, t, 0x28, 0xC0, 4);
                fade_in(fx, t, 0x29, 0xC0, 4);
                let b = fx.mem.read_u8(t + 0x2A);
                if b < 0xC0 {
                    fx.mem.write_u8(t + 0x2A, b + 4);
                } else {
                    fx.mem.set16(t + 0x30, 1);
                }
            }
        } else if v == 1 {
            for o in [0x28u32, 0x29] {
                let b = fx.mem.read_u8(t + o);
                if b >= 0x31 {
                    fx.mem.write_u8(t + o, b - 4);
                }
            }
            let b = fx.mem.read_u8(t + 0x2A);
            if b >= 0x31 {
                fx.mem.write_u8(t + 0x2A, b - 4);
            } else {
                fx.mem.set16(t + 0x30, 0);
            }
        }
    }
    if fx.mem.i32(TIMER) < 0x14 {
        fade_out(fx, t, 0xF, 0xD);
    }
    if fx.mem.i16(t + 0x38) < 0x32 {
        add16(fx, t + 0x38, 2);
    }
    if counter(fx) >= 0x119 && fx.mem.i16(t + 0x3A) < 0x600 {
        add16(fx, t + 0x3A, 0x30);
    }
    if counter(fx) >= 0x119 && fx.mem.i16(t + 0x38) < 0x5A {
        add16(fx, t + 0x38, 1);
    }
    grow_age(fx, t);
    fx.leave(0x30);
}

/// 0x1E00E50 (frame 0x20): a blue quad pair (half size 2 x 2, rising by 5
/// a update), both alphas in by 0x40 for 2 updates, then out by 0xD.
fn quads(fx: &mut Fx, t: u32, e: [u64; 2], ra: u32) {
    enter_step(fx, 0x20, 1, 0x10, e, ra);
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x2E);
        l2::rgb(fx, t + 0x28, 0x30, 0x30, 0x80);
        l2::rgb(fx, t + 0x2C, 0x30, 0x30, 0x80);
        fx.mem.set16(t + 8, 2);
        fx.mem.set16(t + 0xA, 2);
        let v = if fx.mem.i16(t + 0x3A) == 0 { 0 } else { 1 };
        fx.mem.set16(t + 0xE, v);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if age(fx, t) < 2 {
        for o in [0x2Bu32, 0x2F] {
            let v = fx.mem.read_u8(t + o);
            fx.mem.write_u8(t + o, v.wrapping_add(0x40));
        }
    } else {
        fade_out(fx, t, 0xF, 0xD);
        let v = fx.mem.read_u8(t + 0x2F);
        fx.mem.write_u8(t + 0x2F, if v >= 0xF { v - 0xD } else { 0 });
    }
    add16(fx, t + 0xA, 5);
    grow_age(fx, t);
    fx.leave(0x20);
}

/// The fan steps' common tail: the radius +0x38 up by one on odd ages to
/// `cap`; while +6 is below 10 it counts up on odd ages, then +0x30 grows
/// (by 1 below `slow`, else by 4 with the radius up to +0xE on odd ages,
/// `grey` repainted first) and the angle wraps below -359.
fn fan_tail(fx: &mut Fx, t: u32, slow: i32, grey: Option<i32>) -> bool {
    if odd_age(fx, t) && fx.mem.i16(t + 0x38) < 5 {
        add16(fx, t + 0x38, 1);
    }
    if fx.mem.i16(t + 6) < 0xA {
        if odd_age(fx, t) {
            add16(fx, t + 6, 1);
        }
        return false;
    }
    let v = fx.mem.i16(t + 0x30);
    if v < slow {
        fx.mem.set16(t + 0x30, v + 1);
    } else {
        if let Some(g) = grey {
            l2::rgb(fx, t + 0x28, g, g, g);
        }
        add16(fx, t + 0x30, 4);
        if fx.mem.i16(t + 0x38) < fx.mem.i16(t + 0xE) && odd_age(fx, t) {
            add16(fx, t + 0x38, 1);
        }
    }
    let a = fx.mem.i16(t + 0x3A);
    if a < -0x167 {
        let r = fx.div(a, 0x168).1;
        fx.mem.set16(t + 0x3A, r);
    }
    true
}

/// 0x1E00F90 (frame 0x30): a grey fan at node 0x3F (radius rand % 7 + 2,
/// turned rand % 360, cell rand % 7, limit rand % 50 + 30), in by 0x14
/// below 0xA0 for 8 updates, out by 5 from age 0x2E (and by 0xC below
/// timer 17); turning by -2 (-3 from update 0x118).
fn fan_step(fx: &mut Fx, t: u32, e: [u64; 2], ra: u32) {
    enter_step(fx, 0x30, 2, 0x20, e, ra);
    l2::rgb(fx, t + 0x28, 0x2F, 0x2F, 0x2F);
    if first(fx, t) {
        let r = rand_mod(fx, t, 0x01E0_0FD4, 7);
        fx.mem.set16(t + 0x38, r + 2);
        let r = rand_mod(fx, t, 0x01E0_0FF4, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let r = fx.rand_from(t, 0x01E0_1010);
        fx.mem.set16(t + 6, r & 1);
        let r = rand_mod(fx, t, 0x01E0_102C, 7);
        fx.mem.set16(t + 0x34, r);
        fx.mem.set16(t + 0x30, 0);
        let r = rand_mod(fx, t, 0x01E0_104C, 0x32);
        fx.mem.set16(t + 0xE, r + 0x1E);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 8 {
        fade_in(fx, t, 0x2B, 0xA0, 0x14);
    } else if age(fx, t) >= 0x2E {
        fade_out(fx, t, 7, 5);
    }
    grow_age(fx, t);
    fan_tail(fx, t, 0xD, None);
    let d = if counter(fx) < 0x118 { -2 } else { -3 };
    add16(fx, t + 0x3A, d);
    if fx.mem.i32(TIMER) < 0x11 {
        fade_out(fx, t, 0xE, 0xC);
    }
    fx.leave(0x30);
}

/// 0x1E01260 (frame 0x30): a darker fan at the update-0x96 point (radius
/// 4, the angle D+0x3E26, +0x30 from -20, limit 0x46), in by 0xC below
/// 0x60 for 8 updates, out by 5 from age 0x45 and by 0xC below timer 17;
/// turning by -6 while +0x30 is below 6, then by +6.
fn fan_b_step(fx: &mut Fx, t: u32, e: [u64; 2], ra: u32) {
    enter_step(fx, 0x30, 2, 0x20, e, ra);
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 4);
        l2::rgb(fx, t + 0x28, 0x16, 0x16, 0x16);
        let a = fx.mem.i16(D + 0x3E26);
        fx.mem.set16(t + 0x3A, a);
        let r = fx.rand_from(t, 0x01E0_12AC);
        fx.mem.set16(t + 6, r & 1);
        let r = rand_mod(fx, t, 0x01E0_12C8, 7);
        fx.mem.set16(t + 0x34, r);
        fx.mem.set16(t + 0x30, -0x14);
        fx.mem.set16(t + 0xE, 0x46);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 8 {
        fade_in(fx, t, 0x2B, 0x60, 0xC);
    } else {
        if age(fx, t) >= 0x45 {
            fade_out(fx, t, 7, 5);
        }
        if fx.mem.i32(TIMER) < 0x11 {
            fade_out(fx, t, 0xE, 0xC);
        }
    }
    grow_age(fx, t);
    fan_tail(fx, t, 6, Some(0x30));
    let d = if fx.mem.i16(t + 0x30) < 6 { -6 } else { 6 };
    add16(fx, t + 0x3A, d);
    fx.leave(0x30);
}

/// 0x1E01500 (frame 0x20): a grey sprite at the target (size 100 growing
/// by 10, cell rand % 7 stepping on odd ages to 7), in by 0x18 below 0x60
/// for 4 updates, then out by 0x13.
fn sprite(fx: &mut Fx, t: u32, e: [u64; 2], ra: u32) {
    enter_step(fx, 0x20, 1, 0x10, e, ra);
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x64);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        let r = rand_mod(fx, t, 0x01E0_1540, 7);
        fx.mem.set16(t + 0x34, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 4 {
        fade_in(fx, t, 0x2B, 0x60, 0x18);
    } else {
        fade_out(fx, t, 0x15, 0x13);
    }
    if age(fx, t).wrapping_rem(2) != 0 && fx.mem.i16(t + 0x34) < 7 {
        add16(fx, t + 0x34, 1);
    }
    add16(fx, t + 0x38, 0xA);
    grow_age(fx, t);
    fx.leave(0x20);
}

/// 0x1E01610 (frame 0x30): a burst at a point (size 6, turned rand % 360,
/// cell 2), in by 0x14 below 0xA0 for 8 updates, out by 5 from update
/// 0x97; its count +0x30 up to age 3, then back down to 2.
fn burst(fx: &mut Fx, t: u32, e: [u64; 2], ra: u32) {
    enter_step(fx, 0x30, 2, 0x20, e, ra);
    l2::rgb(fx, t + 0x28, 0x4F, 0x4F, 0x4F);
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 6);
        let r = rand_mod(fx, t, 0x01E0_1658, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x34, 2);
        fx.mem.set16(t + 0x30, 1);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 8 {
        fade_in(fx, t, 0x2B, 0xA0, 0x14);
    } else if counter(fx) >= 0x97 {
        fade_out(fx, t, 7, 5);
    }
    grow_age(fx, t);
    if age(fx, t) < 3 {
        add16(fx, t + 0x30, 1);
    } else if fx.mem.i16(t + 0x30) >= 3 {
        add16(fx, t + 0x30, -1);
    }
    fx.leave(0x30);
}

/// 0x1E01740 (frame 0x30): the pentagram's lines, grey, in by 6 below 0x80
/// for 20 updates, out by 9 from update 0x97.
fn star(fx: &mut Fx, t: u32, e: [u64; 2], ra: u32) {
    enter_step(fx, 0x30, 2, 0x20, e, ra);
    if first(fx, t) {
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x34, 0xA);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 0x14 {
        fade_in(fx, t, 0x2B, 0x80, 6);
    } else if counter(fx) >= 0x97 {
        fade_out(fx, t, 0xB, 9);
    }
    grow_age(fx, t);
    fx.leave(0x30);
}

/// The 0x6FF8 / 0x8FF8 corner XYZ2 at `at` (Z 0xFFFFFF, the top byte 0).
fn corner(fx: &mut Fx, at: u32, v: u16) {
    fx.mem.write_u16(at, v);
    fx.mem.write_u16(at + 2, v);
    let x = fx.mem.read_u64(at);
    fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
    fx.mem.write_u8(at + 7, 0);
}

/// 0x1E01930 (frame 0x90): a full-window sprite (ALPHA 0x44) of the frame
/// (0x207EF0), grey, its alpha in by 4 below 0x30 for 12 updates, by 8 to
/// 0x60 from update 0x18C while the timer is at least 0x12, out by 0xC
/// below timer 17; ended on timer 9.
fn window(fx: &mut Fx, t: u32, e: [u64; 2], ra: u32) {
    enter_step(fx, 0x90, 2, 0x20, e, ra);
    let sp = fx.sp;
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for k in 0..3u32 {
        fx.mem.write_u8(sp + 0x48 + k, 0x80);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    fx.mem.write_u8(sp + 0x4B, a);
    fx.mem.write_u32(sp + 0x4C, 0x3F80_0000);
    corner(fx, sp + 0x58, 0x6FF8);
    corner(fx, sp + 0x68, 0x8FF8);
    fx.mem.write_u32(sp + 0x50, 0);
    fx.mem.write_u32(sp + 0x54, 0);
    fx.mem.write_u32(sp + 0x60, 0x3F80_0000);
    fx.mem.write_u32(sp + 0x64, 0x3F80_0000);
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, 0x80_0000_0044);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if age(fx, t) < 0xC {
        fade_in(fx, t, 0x2B, 0x30, 4);
    }
    if counter(fx) >= 0x18C && fx.mem.i32(TIMER) >= 0x12 {
        fade_in(fx, t, 0x2B, 0x60, 8);
    }
    if fx.mem.i32(TIMER) < 0x11 {
        fade_out(fx, t, 0xE, 0xC);
    }
    grow_age(fx, t);
    if fx.mem.i32(TIMER) == 9 {
        fx.mem.set16(t + 0x36, 0);
    }
    fx.leave(0x90);
}

/// The task's point as the draws take it: i16 xyz + i16 +8 / +0xA / +0xC
/// over 10 (integer division by the magic multiply) into `out`.
fn point_int(fx: &mut Fx, t: u32, out: u32) {
    for (c, o) in [(0u32, 8u32), (2, 0xA), (4, 0xC)] {
        let v = fx.mem.i16(t + o);
        let hi = ((i64::from(v) * 0x6666_6667) >> 32) as i32;
        fx.mult(0x6666_6667, v);
        let q = (hi >> 2) + ((v as u32) >> 31) as i32;
        let p = fx.mem.i16(t + c) + q;
        fx.mem.setf(out + 2 * c, ee::from_i32(p));
    }
}

/// The task's point as 0x1E02C60 / 0x1E02F10 take it: x + (+8) / 10, y +
/// (+0xA) / 10, z + (+0xA) / 100 (the original divides its y step again).
fn point_f(fx: &mut Fx, t: u32, out: u32) {
    let x = ee::add(ee::from_i32(fx.mem.i16(t)), ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0));
    fx.mem.setf(out, x);
    let fy = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    let y = ee::add(ee::from_i32(fx.mem.i16(t + 2)), fy);
    fx.mem.setf(out + 4, y);
    let fz = ee::div(fy, 10.0);
    let z = ee::add(ee::from_i32(fx.mem.i16(t + 4)), fz);
    fx.mem.setf(out + 8, z);
}

fn scale01(fx: &mut Fx, at: u32) {
    for o in [0u32, 4, 8] {
        fx.mem.write_u32(at + o, 0x3DCC_CCCD);
    }
}

fn submit_w(fx: &mut Fx, d: u32, w: u32, ra: u32) {
    let v = fx.mem.f(w);
    fx.fix_residue(v, ra);
    let key = ee::fixunssfsi(v) as i32;
    fx.submit(d, OT, key);
}

/// 0x1E028C0(D, t, d, mode) (frame 0x140): mode 0 heads `d` as an additive
/// 4-vertex gouraud strip coloured +0x28 inside / +0x2C outside; mode 1
/// draws 36 strips 10 degrees wide between the radius +0x38 (y) and +0x38
/// + 20 at height +0x30, facing the camera's yaw only plus +0x18, scale
/// 0.1 at the task's point, key -1.
fn ring_draw(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x140);
    if mode == 0 {
        l2::desc_gouraud4(fx, d, 0);
        l2::fullscreen4(fx, d);
        fx.desc_alpha(d, 0x80_0000_0042);
        l2::colour_strip4(fx, d, t + 0x28, 1);
        l2::colour_strip4(fx, d, t + 0x2C, 2);
        fx.leave(0x140);
        return;
    }
    scale01(fx, sp + 0xD0);
    point_int(fx, t, sp + 0xC0);
    crate::b494::camera_angles_saving(fx, sp + 0x130, u64::from(OT), 1, 0x01E0_2A18);
    fx.mem.write_u32(sp + 0x130, 0);
    fx.mem.write_u32(sp + 0x134, 0);
    let v = fx.mem.read_u32(sp + 0x138).wrapping_add(fx.mem.read_u32(t + 0x18));
    fx.mem.write_u32(sp + 0x138, v);
    l2::xform_vp(fx, sp + 0xF0, sp + 0xD0, sp + 0x130, sp + 0xC0);
    let mut s2 = 0i32;
    for _ in 0..0x24 {
        let r = ee::from_i32(fx.mem.i16(t + 0x38));
        let r2 = ee::from_i32(fx.mem.i16(t + 0x38) + 0x14);
        let s3 = (s2 + 0xA) as i16 as i32;
        fx.mem.write_u32(sp + 0x88, 0);
        let v = l2::sin_deg(fx, SINES, s2, 0);
        fx.mem.setf(sp + 0x80, ee::mul(r, v));
        let v = l2::sin_deg(fx, SINES, s2, 1);
        fx.mem.setf(sp + 0x84, ee::mul(r, v));
        fx.mem.write_u32(sp + 0x98, 0);
        let v = l2::sin_deg(fx, SINES, s3, 0);
        fx.mem.setf(sp + 0x90, ee::mul(r, v));
        let v = l2::sin_deg(fx, SINES, s3, 1);
        fx.mem.setf(sp + 0x94, ee::mul(r, v));
        let h = ee::from_i32(fx.mem.i16(t + 0x30));
        fx.mem.setf(sp + 0xA8, h);
        let v = l2::sin_deg(fx, SINES, s2, 0);
        fx.mem.setf(sp + 0xA0, ee::mul(r2, v));
        let v = l2::sin_deg(fx, SINES, s2, 1);
        fx.mem.setf(sp + 0xA4, ee::mul(r2, v));
        fx.mem.setf(sp + 0xB8, h);
        let v = l2::sin_deg(fx, SINES, s3, 0);
        fx.mem.setf(sp + 0xB0, ee::mul(r2, v));
        let v = l2::sin_deg(fx, SINES, s3, 1);
        fx.mem.setf(sp + 0xB4, ee::mul(r2, v));
        if l2::project(fx, sp + 0x80, SPR, sp + 0xF0, d, sp + 0xE0) {
            fx.submit(d, OT, -1);
        }
        s2 = s3;
    }
    fx.leave(0x140);
}

/// 0x1E02650(D, t, d, mode) (frame 0x130): mode 0 heads `d` as a strip3
/// coloured +0x28 with the UV rectangle (1, 1, 0x7E, 0x3E) in order 10 on
/// texture D+0x3DA4; mode 1 draws +0xE rings of two quarters (0x109640
/// modes 0 / 1, half size +0x38, mirrored below) facing the camera's yaw
/// plus +0x18, scale 0.1 at the task's point, each at the far Z keyed by
/// its mean w.
fn glow_draw(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x130);
    if mode == 0 {
        for (o, v) in [(0u32, 1), (2, 1), (4, 0x7E), (6, 0x3E)] {
            fx.mem.set16(sp + 0xC0 + o, v);
        }
        l2::desc_strip3(fx, d);
        l2::colour_strip3(fx, d, t + 0x28, 0);
        l2::uv_strip3(fx, d, sp + 0xC0, 0xA);
        let tex = fx.mem.read_u32(TEX5 + 4);
        fx.desc_texture(d, tex, 1);
        fx.leave(0x130);
        return;
    }
    crate::b494::camera_angles_saving(fx, sp + 0x120, u64::from(OT), u64::from(D), 0x01E0_26F4);
    scale01(fx, sp + 0xB0);
    point_int(fx, t, sp + 0xA0);
    fx.mem.write_u32(sp + 0x120, 0);
    fx.mem.write_u32(sp + 0x124, 0);
    let v = fx.mem.read_u32(sp + 0x128).wrapping_add(fx.mem.read_u32(t + 0x18));
    fx.mem.write_u32(sp + 0x128, v);
    l2::xform_vp(fx, sp + 0xE0, sp + 0xB0, sp + 0x120, sp + 0xA0);
    let mut j = 0i32;
    while j < fx.mem.i16(t + 0xE) {
        for k in 0..2 {
            let a = fx.mem.i16(t + 0x38);
            l2::quad_ints(fx, sp + 0x60, a, a, k);
            let v = ee::neg(fx.mem.f(sp + 0x64));
            fx.mem.setf(sp + 0x84, v);
            let v = ee::neg(fx.mem.f(sp + 0x74));
            fx.mem.setf(sp + 0x94, v);
            if l2::project(fx, sp + 0x60, SPR, sp + 0xE0, d, sp + 0xD0) {
                l2::far_z_desc(fx, d, 0);
                submit_w(fx, d, sp + 0xD0, 0x01E0_283C);
            }
        }
        j = (j + 1) as i16 as i32;
    }
    fx.leave(0x130);
}

/// A billboard kind: the header's atlas sprites (default, for a burst),
/// textures (default, burst) and its draw site.
struct Board {
    sprite: i32,
    burst_sprite: i32,
    tex: u32,
    burst_tex: u32,
    cam_ra: u32,
    fix_ra: u32,
    int_point: bool,
}

const SPRITE_KIND: Board = Board { sprite: 0x14, burst_sprite: 0x50, tex: 0x320, burst_tex: 0x340, cam_ra: 0x01E0_3910, fix_ra: 0x01E0_3A34, int_point: true };
const BURST_KIND: Board = Board { sprite: 0x9B, burst_sprite: 0x14, tex: 0x35C, burst_tex: 0x3DA0, cam_ra: 0x01E0_2D50, fix_ra: 0x01E0_2E8C, int_point: false };

/// 0x1E02C60 / 0x1E03820(D, t, d, mode) (frame 0x130): mode 0 heads `d` as
/// a strip3 coloured +0x28 on the atlas cell (sprite, +0x34) and its
/// texture (a burst's own); mode 1 draws a camera-facing quad of half size
/// +0x38 (scale 0.1) at the task's point: a burst (and every 0x1E03820
/// quad) at the far Z (a burst's) keyed by its mean w, 0x1E02C60's others
/// +0x30 times through 0x109870.
fn billboard(fx: &mut Fx, t: u32, d: u32, mode: i32, k: &Board) {
    let sp = fx.enter(0x130);
    let is_burst = fx.mem.read_u32(t + 0x24) == BURST;
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0xC0, k.sprite, c);
        if is_burst {
            let c = fx.mem.i16(t + 0x34);
            l2::rect(fx, sp + 0xC0, k.burst_sprite, c);
        }
        l2::desc_strip3(fx, d);
        l2::colour_strip3(fx, d, t + 0x28, 0);
        l2::uv_strip3(fx, d, sp + 0xC0, 0);
        let tex = fx.mem.read_u32(D + if is_burst { k.burst_tex } else { k.tex });
        fx.desc_texture(d, tex, 1);
        fx.leave(0x130);
        return;
    }
    crate::b494::camera_angles_saving(fx, sp + 0x120, u64::from(OT), u64::from(d), k.cam_ra);
    if k.int_point {
        point_int(fx, t, sp + 0xA0);
        scale01(fx, sp + 0xB0);
    } else {
        point_f(fx, t, sp + 0xA0);
        scale01(fx, sp + 0xB0);
    }
    l2::xform_vp(fx, sp + 0xD0, sp + 0xB0, sp + 0x120, sp + 0xA0);
    let s = ee::from_i32(fx.mem.i16(t + 0x38));
    l2::quad_xy(fx, sp + 0x60, s, s);
    if k.int_point || is_burst {
        if l2::project(fx, sp + 0x60, SPR, sp + 0xD0, d, sp + 0x110) {
            if fx.mem.read_u32(t + 0x24) == BURST {
                l2::far_z_desc(fx, d, 0);
            }
            submit_w(fx, d, sp + 0x110, k.fix_ra);
        }
    } else {
        let mut j = 0i32;
        while j < fx.mem.i16(t + 0x30) {
            l2::project_submit(fx, sp + 0x60, d, sp + 0xD0, OT);
            j = (j + 1) as i16 as i32;
        }
    }
    fx.leave(0x130);
}

/// A fan kind: atlas sprite, texture, ALPHA 0x42 on mode 1, its pieces and
/// their spacing, the second edge's angle, the inner height, the outer
/// edge's angles and the draw sites.
struct FanKind {
    sprite: i32,
    tex: u32,
    alpha: bool,
    pieces: i32,
    gap: i32,
    span: i32,
    outer: Option<i32>,
    outer_a: [i32; 2],
    fix_ra: u32,
    cam_ra: u32,
}

const FAN_A_KIND: FanKind = FanKind { sprite: 0x1E, tex: 0x32C, alpha: true, pieces: 3, gap: 0x78, span: 0xA, outer: None, outer_a: [4, 0xE], fix_ra: 0x01E0_2F84, cam_ra: 0x01E0_3010 };
const FAN_B_KIND: FanKind = FanKind { sprite: 0x14, tex: 0x320, alpha: false, pieces: 6, gap: 0x3C, span: 0x22, outer: Some(0x19), outer_a: [0, 0x22], fix_ra: 0x01E0_3424, cam_ra: 0x01E0_3498 };

/// 0x1E02F10 / 0x1E033B0(D, t, d, mode) (frame 0x150): mode 0 heads `d` as
/// a strip3 coloured +0x28 on the atlas cell (sprite, +0x34) and texture;
/// mode 1 draws `pieces` quads `gap` degrees apart from the angle +0x3A,
/// inner edge at radius +0x38 / height +0x30, outer at +0x38 + 1 / +0x30 +
/// (+6 or 0x19), facing the camera turned -+0x4000 about x by the unit's
/// facing, at the task's point; each at the far Z (kind 1), key 0.
fn fan_draw(fx: &mut Fx, t: u32, d: u32, mode: i32, k: &FanKind) {
    let sp = fx.enter(0x150);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x140);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0x144)), 180.0);
    fx.fix_residue(v, k.fix_ra);
    let facing = ee::fixsfsi(v) as i16 as i32;
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0xD0, k.sprite, c);
        l2::desc_strip3(fx, d);
        l2::colour_strip3(fx, d, t + 0x28, 0);
        l2::uv_strip3(fx, d, sp + 0xD0, 0);
        let tex = fx.mem.read_u32(D + k.tex);
        fx.desc_texture(d, tex, 1);
        fx.leave(0x150);
        return;
    }
    if k.alpha {
        fx.desc_alpha(d, 0x80_0000_0042);
    }
    crate::b494::camera_angles_saving(fx, sp + 0x130, u64::from(d), u64::from(t), k.cam_ra);
    fx.mem.write_u32(sp + 0x134, 0);
    fx.mem.set32(sp + 0x130, if facing != 0 { -0x4000 } else { 0x4000 });
    point_f(fx, t, sp + 0xC0);
    l2::xform_vp(fx, sp + 0xE0, 0, sp + 0x130, sp + 0xC0);
    let mut s5 = 0i32;
    for _ in 0..k.pieces {
        let a = |fx: &mut Fx, e: i32| (fx.mem.i16(t + 0x3A) + e + s5) as i16 as i32;
        let r = ee::from_i32(fx.mem.i16(t + 0x38));
        let r1 = ee::from_i32(fx.mem.i16(t + 0x38) + 1);
        let h = ee::from_i32(fx.mem.i16(t + 0x30));
        let h2 = match k.outer {
            None => ee::from_i32(fx.mem.i16(t + 0x30) + fx.mem.i16(t + 6)),
            Some(o) => ee::from_i32(fx.mem.i16(t + 0x30) + o),
        };
        let pts = [(0x80u32, 0, r, h), (0x90, k.span, r, h), (0xA0, k.outer_a[0], r1, h2), (0xB0, k.outer_a[1], r1, h2)];
        for (o, e, rr, hh) in pts {
            fx.mem.setf(sp + o + 4, hh);
            let ang = a(fx, e);
            let v = l2::sin_deg(fx, SINES, ang, 0);
            fx.mem.setf(sp + o, ee::mul(rr, v));
            let ang = a(fx, e);
            let v = l2::sin_deg(fx, SINES, ang, 1);
            fx.mem.setf(sp + o + 8, ee::mul(rr, v));
        }
        if l2::project(fx, sp + 0x80, SPR, sp + 0xE0, d, sp + 0x120) {
            l2::far_z_desc(fx, d, 1);
            fx.submit(d, OT, 0);
        }
        s5 = (s5 + k.gap) as i16 as i32;
    }
    fx.leave(0x150);
}

/// 0x109A90(d, c1, c2): a two-vertex gouraud descriptor (4 values, flags
/// 0x11, regs RGBAQ XYZF2 x 2, additive), the colours at +0x10 / +0x20
/// with Q 1.0 and their alphas' top bytes +0x1F / +0x2F cleared (b053's).
fn line_desc(fx: &mut Fx, d: u32, c1: u32, c2: u32) {
    fx.enter(0x40);
    fx.mem.set16(d + 2, 0x11);
    fx.mem.set16(d, 4);
    fx.mem.write_u64(d + 8, 0x4141);
    fx.mem.set32(d + 4, 0);
    fx.desc_alpha(d, crate::fxlib::ALPHA_ADD);
    for k in 0..4 {
        let b = fx.mem.read_u8(c1 + k);
        fx.mem.write_u8(d + 0x10 + k, b);
        let b = fx.mem.read_u8(c2 + k);
        fx.mem.write_u8(d + 0x20 + k, b);
    }
    fx.mem.write_u32(d + 0x24, 0x3F80_0000);
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    fx.mem.write_u8(d + 0x1F, 0);
    fx.mem.write_u8(d + 0x2F, 0);
    fx.leave(0x40);
}

/// 0x1E03A70(D, t, d, 0) (frame 0x60): the pentagram, five lines between
/// the points D+0x3C10 + 0x10 k (4-1, 4-0, 1-3, 0-2, 3-2), coloured +0x28,
/// each through the view-projection at the far Z (0x109930), key 0.
fn star_draw(fx: &mut Fx, t: u32, d: u32) {
    let sp = fx.enter(0x60);
    line_desc(fx, d, t + 0x28, t + 0x28);
    for (k, (a, b)) in [(4u32, 1u32), (4, 0), (1, 3), (0, 2), (3, 2)].into_iter().enumerate() {
        fx.bzero(sp + 0x40, 0x20);
        copy4(fx, sp + 0x40, POINTS + 0x10 * a);
        copy4(fx, sp + 0x50, POINTS + 0x10 * b);
        // 0x109930(in, desc, 0, ot): its mean w at its own sp + 0x30.
        let f = fx.sp - 0x40;
        fx.saved(f, &[(0, k as u64), (0x10, u64::from(d)), (0x20, 0x01E0_3C74)]);
        if l2::project(fx, sp + 0x40, SPR, 0, d, f + 0x30) {
            for o in [0x18u32, 0x28] {
                let v = fx.mem.read_u64(d + o);
                fx.mem.write_u64(d + o, (v & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFF << 32));
            }
            let ot = fx.mem.read_u32(OT_AT);
            fx.submit(d, ot, 0);
        }
    }
    fx.leave(0x60);
}
