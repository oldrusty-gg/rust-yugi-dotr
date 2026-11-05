//! BMODULE b195 (card 559), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01E50, data area 0x1ED0000, 0x41F0 bytes) [F]
//!: b130's task family (s0 = D): 220 tasks of
//! 0x40 at D+0x774 (alive +0x36, step +0x24 by direct `jal`), spawned by
//! 0x1E02650 (b538's spawn at D+0x2D0 with the colour D+0x4024), timer
//! D+0x4020 = 0x92, the OT at D+0x402C / D+0x80, counter D+0x407E, sines
//! D+0x4084 and D+0xD8, the attacker's nodes 0x3E (clear) / 0x3D / 0xD at
//! D+0x4068.., textures (4, 0..2) D+0x340.. and (4, 0xE / 0xF) D+0x378 /
//! D+0x37C; init lays ten quads (5, 50) and nine fans at D+0x380 + 0x40 k.
//!
//! The emitter 0x1E00780: 0x1E..0x31 a red fan 0x1060 a update, below 0x32
//! three orange fans 0xD90; on 0x1D the history D+0x3F70 filled from node
//! 0x3E, kept by 0x1E00260 (10 points); on 0x32 at node 0x3E fifty sparks
//! 0x14A0, thirty rings 0x1730, ten fans 0xC20, the camera 0x1920 (b403's
//! slots 4 / 5, block D+0x3E80) and the disc 0x1840 (`spawn::disc_step`
//! 5225257f1409); node 0x3E's position into D+0x320 on 0x2F and D+0x330 on
//! 0x1E, D+0x338 -= 4 every update. Drawn (twice: mode 0 the header, 1 the
//! primitives, desc sp + 0xB8): 0xC20 / 0x1060 by nine-segment fans
//! 0x1E02A40 / 0x1E02D00 (2 degrees wide; 0x2D00 the rim at y +6), 0xD90
//! by 0x1E02770 (+6 degrees wide, scale (1, 1.5, 1)), 0x14A0 by the
//! camera-facing square 0x1E03200 (b383's 0x1E05090), 0x1730 by the band
//! 0x1E03410 (radius +0x38 / +0x38 - 17), 0x1840 by `quads4::fan18`. From
//! 0x15 D+0x310 / D+0x318 creep up by 0.01 below 0.5, D+0x314 = 0.1.
//! Returns 1 / 4 / 2 on 0x28 / 0x3C / 0x32, 8 when the timer is out.
//!
//! The steps 0x1E00A00 / 0x1390 / 0x1AB0 and the object op 0x1B70 are
//! tested by the entry but never spawned (not ported, nor the draws
//! 0x1E00080 / 0x1E02FE0).
use crate::b076::unit_position_saving;
use crate::b494::camera_angles_saving;
use crate::blib::{quads4, spawn as lspawn, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::Fx;
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 0xDC;
const POOL: u32 = D + 0x774;
const POS: u32 = D + 0x2D0;
const CAM: u32 = D + 0x3E80;
const TIMER: u32 = D + 0x4020;
const COLOUR: u32 = D + 0x4024;
const OTW: u32 = D + 0x402C;
const MEAN: u32 = D + 0x4054;
const NODE: u32 = D + 0x4068;
const COUNTER: u32 = D + 0x407E;
const TAB: u32 = D + 0x4084;
const TEX: u32 = D + 0x37C;

const FAN_C: u32 = 0x01E0_0C20;
const FAN_O: u32 = 0x01E0_0D90;
const FAN_R: u32 = 0x01E0_1060;
const SPARK: u32 = 0x01E0_14A0;
const RING: u32 = 0x01E0_1730;
const DISC: u32 = 0x01E0_1840;
const CAMERA: u32 = 0x01E0_1920;

const D05: u64 = 0x3FE0_0000_0000_0000;

/// The entry 0x1E01E50 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x150);
    fx.mem.write_u32(OTW, crate::exe::OT);
    fx.mem.write_u32(D + 0x80, crate::exe::OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp) };
    fx.leave(0x150);
    ret
}

/// rand() with the caller's s0 = `s0` (64 bits) from `ra`.
fn rand64(fx: &mut Fx, s0: u64, ra: u32) -> i32 {
    let f = fx.sp - 0x10;
    fx.mem.write_u64(f, s0);
    fx.mem.write_u64(f + 8, u64::from(ra));
    fx.rand()
}

fn rmod(fx: &mut Fx, s0: u64, ra: u32, m: i32) -> i32 {
    let v = rand64(fx, s0, ra);
    fx.div(v, m).1
}

/// 0x1A2B80 (fixsfsi) with its frame.
fn fix(fx: &mut Fx, v: f32, ra: u32) -> i32 {
    fx.fix_residue(v, ra);
    ee::fixsfsi(v)
}

/// 0x1A2C70 (fixunssfsi) of the float at `at` with its frame.
fn fixu(fx: &mut Fx, at: u32, ra: u32) -> i32 {
    let v = fx.mem.f(at);
    fx.fix_residue(v, ra);
    ee::fixunssfsi(v) as i32
}

/// 0x1A2E08's unpack of the double `x` at `p` (b130's).
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
/// `ra2`) with the frames they leave (b130's `dlt`).
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

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x41F0);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x407C, param);
    fx.mem.set32(TIMER, 0x92);
    for o in [0x310u32, 0x314, 0x318] {
        fx.mem.set32(D + o, 0);
    }
    for (k, (node, clear)) in [(0x3E, 1), (0x3D, 0), (0xD, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(NODE + 2 * k as u32, r);
    }
    for k in 0..3u32 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x340 + 4 * k, t);
    }
    for k in 0..2u32 {
        let t = fx.texture(4, 0xE + k as i32);
        fx.mem.write_u32(D + 0x378 + 4 * k, t);
    }
    for k in 0..10u32 {
        l2::quad_xy(fx, D + 0x380 + 0x40 * k, 5.0, 50.0);
    }
    for k in 0..9i32 {
        let q = D + 0x380 + 0x40 * k as u32;
        let (a, b) = (20 * k, 20 * k + 20);
        let v = ee::mul(15.0, l2::sin_deg(fx, TAB, a as i16 as i32, 0));
        fx.mem.setf(q + 4, v);
        let v = ee::mul(15.0, l2::sin_deg(fx, TAB, b as i16 as i32, 0));
        fx.mem.setf(q + 0x14, v);
        let v = ee::mul(35.0, l2::sin_deg(fx, TAB, a as i16 as i32, 0));
        fx.mem.setf(q + 0x24, v);
        let v = ee::mul(35.0, l2::sin_deg(fx, TAB, b as i16 as i32, 0));
        fx.mem.setf(q + 0x34, v);
        let x = ee::from_i32(-0x2D + 10 * k);
        for o in [0u32, 0x10, 0x20, 0x30] {
            let v = ee::add(fx.mem.f(q + o), x);
            fx.mem.setf(q + o, v);
        }
        for (o, d) in [(0x4u32, -10.0f32), (0x14, -10.0), (0x24, 15.0), (0x34, 15.0)] {
            let v = if d < 0.0 { ee::sub(fx.mem.f(q + o), 10.0) } else { ee::add(fx.mem.f(q + o), d) };
            fx.mem.setf(q + o, v);
        }
        for o in [8u32, 0x18, 0x28, 0x38] {
            fx.mem.set32(q + o, 0);
        }
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let is = |fx: &mut Fx, f: u32| fx.mem.read_u32(t + 0x24) == f;
        if is(fx, FAN_C) {
            fan_c(fx, t);
        }
        if is(fx, FAN_O) {
            fan_o(fx, t);
        }
        if is(fx, FAN_R) {
            fan_r(fx, t);
        }
        if is(fx, SPARK) {
            spark(fx, t);
        }
        if is(fx, RING) {
            ring(fx, t);
        }
        if is(fx, DISC) {
            let k = lspawn::disc("5225257f1409").expect("disc constants");
            lspawn::disc_step_with(fx, k, DISC, t);
        }
        if is(fx, CAMERA) {
            camera(fx, t);
        }
    }
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x70, u64::from(D), 0x01E0_22BC);
    camera_angles_saving(fx, sp + 0x90, u64::from(D), 0, 0x01E0_22C4);
    let z = ee::sub(fx.mem.f(sp + 0x78), 25.0);
    fx.mem.setf(sp + 0x78, z);
    let d = sp + 0xB8;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let is = |fx: &mut Fx, f: u32| fx.mem.read_u32(t + 0x24) == f;
        for mode in 0..2 {
            if is(fx, FAN_C) {
                fan9(fx, t, d, mode, &FAN9_C);
            }
        }
        for mode in 0..2 {
            if is(fx, FAN_O) {
                fan9(fx, t, d, mode, &FAN9_O);
            }
        }
        for mode in 0..2 {
            if is(fx, FAN_R) {
                fan9(fx, t, d, mode, &FAN9_R);
            }
        }
        for mode in 0..2 {
            if is(fx, SPARK) {
                square(fx, t, d, mode);
            }
        }
        for mode in 0..2 {
            if is(fx, RING) {
                band(fx, t, d, mode);
            }
        }
        if is(fx, DISC) {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            let c = fx.mem.i16(t + 0x34);
            l2::rect(fx, sp + 0xA0, 0x9B, c);
            for c in 0..4 {
                let w = fx.mem.read_u32(sp + 0xA0 + 4 * c);
                fx.mem.write_u32(D + 0x94 + 4 * c, w);
            }
            quads4::fan18(fx, D, t, d);
        }
    }
    if fx.mem.i16(COUNTER) >= 0x15 {
        let v = fx.mem.f(D + 0x310);
        if dlt(fx, v, D05, D, 0x01E0_2538, 0x01E0_2548) {
            let v = ee::add(fx.mem.f(D + 0x310), f32::from_bits(0x3C23_D70A));
            fx.mem.setf(D + 0x310, v);
        }
        fx.mem.write_u32(D + 0x314, 0x3DCC_CCCD);
        let v = fx.mem.f(D + 0x318);
        if dlt(fx, v, D05, D, 0x01E0_2580, 0x01E0_2590) {
            let v = ee::add(fx.mem.f(D + 0x318), f32::from_bits(0x3C23_D70A));
            fx.mem.setf(D + 0x318, v);
        }
    }
    let n = fx.mem.i32(TIMER);
    if n != 0 {
        fx.mem.set32(TIMER, n - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x28 => 1,
        0x3C => 4,
        0x32 => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E02650(D, step, arg) (frame 0x60): the first free task cleared and
/// set off at D+0x2D0 (as i16) with the colour D+0x4024 (twice), +0x30 =
/// D+0x407A; 1 when one was free.
fn spawn(fx: &mut Fx, f: u32) -> i32 {
    let sp = fx.enter(0x60);
    let _ = sp;
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
            let a = fx.mem.i16(D + 0x407A);
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

/// 0x1E00780(D, 0) (frame 0x50): the spawn colour (0x80, 0, 0, 0), the
/// update counter and the spawns.
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x50);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    let h = fx.self_handle();
    let n = fx.mem.i16(NODE);
    fx.node_position(h, n, sp + 0x30);
    let c = fx.mem.i16(COUNTER);
    if (0x1E..0x32).contains(&c) {
        spawn(fx, FAN_R);
    }
    if fx.mem.i16(COUNTER) < 0x32 {
        for _ in 0..3 {
            spawn(fx, FAN_O);
        }
    }
    if fx.mem.i16(COUNTER) == 0x1D {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE);
        fx.node_position(h, n, sp + 0x40);
        for k in 0..9u32 {
            for c in 0..3 {
                let w = fx.mem.read_u32(sp + 0x40 + 4 * c);
                fx.mem.write_u32(D + 0x3F70 + 0x10 * k + 4 * c, w);
            }
        }
    }
    let n = fx.mem.i16(NODE);
    lspawn::node_history(fx, D + 0x4000, 0xA, n);
    if fx.mem.i16(COUNTER) == 0x32 {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE);
        fx.node_position(h, n, sp + 0x30);
        for c in 0..3 {
            let w = fx.mem.read_u32(sp + 0x30 + 4 * c);
            fx.mem.write_u32(POS + 4 * c, w);
        }
        for _ in 0..0x32 {
            spawn(fx, SPARK);
        }
        for _ in 0..0x1E {
            spawn(fx, RING);
        }
        for _ in 0..0xA {
            spawn(fx, FAN_C);
        }
        spawn(fx, CAMERA);
        spawn(fx, DISC);
    }
    if fx.mem.i16(COUNTER) == 0x2F {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE);
        fx.node_position(h, n, D + 0x320);
    }
    if fx.mem.i16(COUNTER) == 0x1E {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE);
        fx.node_position(h, n, D + 0x330);
    }
    let v = ee::sub(fx.mem.f(D + 0x338), 4.0);
    fx.mem.setf(D + 0x338, v);
    fx.leave(0x50);
}

/// Alpha +0x2B up by 0x40 below 0x80 for the first two updates, then
/// (from +0x38 = 0x1A) down by 0xD (the task over under 0xF), and with
/// the timer below 10 down by 0xF (over under 0x11).
fn fan_alpha(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x32) < 2 {
        let a = fx.mem.u8(t + 0x2B);
        if a < 0x80 {
            fx.mem.set8(t + 0x2B, a + 0x40);
        }
        return;
    }
    if fx.mem.i16(t + 0x38) >= 0x1A {
        let a = fx.mem.u8(t + 0x2B);
        if a < 0xF {
            fx.mem.set16(t + 0x36, 0);
        } else {
            fx.mem.set8(t + 0x2B, a - 0xD);
        }
    }
    if fx.mem.i32(TIMER) < 0xA {
        let a = fx.mem.u8(t + 0x2B);
        if a < 0x11 {
            fx.mem.set16(t + 0x36, 0);
        } else {
            fx.mem.set8(t + 0x2B, a - 0xF);
        }
    }
}

/// (rand & 0x3FFF) + `add` (the record's signed rem by 0x4000).
fn rand4000(fx: &mut Fx, s0: u64, ra: u32, add: i32) -> i32 {
    let r = rand64(fx, s0, ra);
    fx.div(r, 0x4000).1 + add
}

/// 0x1E00C20 (D, task) (frame 0x30): a fan at the spawn point: +0x38 = 8
/// growing by 2, angles x / y random quarter turns, cell rand % 7; colour
/// (0x40, 0x20, 0).
fn fan_c(fx: &mut Fx, t: u32) {
    fx.enter(0x30);
    let s0 = u64::from(t);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 8);
        let v = rand4000(fx, s0, 0x01E0_0C50, 0x2000);
        fx.mem.set32(t + 0x18, v);
        let v = rand4000(fx, s0, 0x01E0_0C70, -0x2000);
        fx.mem.set32(t + 0x10, v);
        fx.mem.set32(t + 0x14, 0);
        let v = rmod(fx, s0, 0x01E0_0C94, 7);
        fx.mem.set16(t + 0x34, v);
        fx.mem.set16(t + 0x36, 2);
    }
    fan_alpha(fx, t);
    l2::rgb(fx, t + 0x28, 0x40, 0x20, 0);
    let v = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, v);
    let v = fx.mem.i16(t + 0x38) + 2;
    fx.mem.set16(t + 0x38, v);
    fx.leave(0x30);
}

/// 0x1E00D90 (D, task) (frame 0x90): an orange fan riding node 0x3D, +0x30
/// (5..19) twentieths of the way to node 0x3E (until 0x47); +6 2 (4 from
/// 0x32), +0x10 0x4000, +0x14 rand % 0xFFFF; alpha in by 0x40, then out by
/// 5 (the task over under 7, and from 0x33); size 7 (0x14 from 0x32).
fn fan_o(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x90);
    let s0 = u64::from(t);
    l2::rgb(fx, t + 0x28, 0x30, 0x10, 0);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set32(t + 0x10, 0x4000);
        let v = rmod(fx, s0, 0x01E0_0DEC, 0xFFFF);
        fx.mem.set32(t + 0x14, v);
        fx.mem.set16(t + 0x34, 0);
        let w = if fx.mem.i16(COUNTER) < 0x32 { 2 } else { 4 };
        fx.mem.set16(t + 6, w);
        let v = rmod(fx, s0, 0x01E0_0E34, 0xF) + 5;
        fx.mem.set16(t + 0x30, v);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0);
    }
    let h = fx.self_handle();
    let n = fx.mem.i16(NODE);
    fx.node_position(h, n, sp + 0x70);
    let h = fx.self_handle();
    let n = fx.mem.i16(NODE + 2);
    fx.node_position(h, n, sp + 0x80);
    if fx.mem.i16(COUNTER) < 0x47 {
        let x = fx.mem.f(sp + 0x80);
        let v = fix(fx, x, 0x01E0_0EA8);
        fx.mem.set16(t, v);
        let x = fx.mem.f(sp + 0x84);
        let v = fix(fx, x, 0x01E0_0EB8);
        fx.mem.set16(t + 2, v);
        let x = fx.mem.f(sp + 0x88);
        let v = fix(fx, x, 0x01E0_0EC8);
        fx.mem.set16(t + 4, v);
        let f5 = ee::div(ee::sub(fx.mem.f(sp + 0x70), fx.mem.f(sp + 0x80)), 20.0);
        let f21 = ee::sub(fx.mem.f(sp + 0x78), fx.mem.f(sp + 0x88));
        let f20 = ee::sub(fx.mem.f(sp + 0x74), fx.mem.f(sp + 0x84));
        let (f20, f21) = (ee::div(f20, 20.0), ee::div(f21, 20.0));
        for (o, f, ra) in [(0u32, f5, 0x01E0_0F40u32), (2, f20, 0x01E0_0F6C), (4, f21, 0x01E0_0F98)] {
            let n = ee::from_i32(fx.mem.i16(t + 0x30));
            let v = ee::add(ee::from_i32(fx.mem.i16(t + o)), ee::mul(f, n));
            let v = fix(fx, v, ra);
            fx.mem.set16(t + o, v);
        }
    }
    if fx.mem.i16(t + 0x32) < 2 {
        let a = fx.mem.u8(t + 0x2B);
        if a < 0x80 {
            fx.mem.set8(t + 0x2B, a + 0x40);
        }
    } else {
        let a = fx.mem.u8(t + 0x2B);
        if a < 7 {
            fx.mem.set16(t + 0x36, 0);
        } else {
            fx.mem.set8(t + 0x2B, a - 5);
        }
    }
    if fx.mem.i16(COUNTER) >= 0x33 {
        fx.mem.set16(t + 0x36, 0);
    }
    let s = if fx.mem.i16(COUNTER) < 0x32 { 7 } else { 0x14 };
    fx.mem.set16(t + 0x38, s);
    let v = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, v);
    fx.leave(0x90);
}

/// 0x1E01060 (D, task) (frame 0x90): a red fan at node 0x3E (until 0x33),
/// aimed along node 0x3E -> 0x3D (0x108B90 of the horizontal and full
/// lengths, x 180 by the attacker's yaw / 180); +0x38 rand % 15 growing by
/// 1, cell rand % 7, +6 rand % 15 + 5 (from 0x31 down by 2 to -14); the
/// fan alpha; colour (0x80, 0x40, 0).
fn fan_r(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x90);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x80);
    let yaw = ee::div(ee::from_i32(fx.mem.i32(sp + 0x84)), 180.0);
    let s0 = fix(fx, yaw, 0x01E0_10C4) as i16 as i32;
    let r0 = i64::from(s0) as u64;
    if fx.mem.i16(t + 0x36) == 1 {
        let v = rmod(fx, r0, 0x01E0_10E0, 0xF);
        fx.mem.set16(t + 0x38, v);
        for o in [0x18u32, 0x10, 0x14] {
            fx.mem.set32(t + o, 0);
        }
        let v = rmod(fx, r0, 0x01E0_1108, 7);
        fx.mem.set16(t + 0x34, v);
        fx.mem.set16(t + 0x36, 2);
        let v = rmod(fx, r0, 0x01E0_112C, 0xF) + 5;
        fx.mem.set16(t + 6, v);
    }
    let h = fx.self_handle();
    let n = fx.mem.i16(NODE);
    fx.node_position(h, n, sp + 0x60);
    let h = fx.self_handle();
    let n = fx.mem.i16(NODE + 2);
    fx.node_position(h, n, sp + 0x70);
    if fx.mem.i16(COUNTER) < 0x33 {
        for (k, ra) in [0x01E0_1194u32, 0x01E0_11A0, 0x01E0_11AC].into_iter().enumerate() {
            let x = fx.mem.f(sp + 0x60 + 4 * k as u32);
        let v = fix(fx, x, ra);
            fx.mem.set16(t + 2 * k as u32, v);
        }
    }
    fan_alpha(fx, t);
    if fx.mem.i16(COUNTER) >= 0x31 {
        let w = fx.mem.i16(t + 6);
        if w >= -0xE {
            fx.mem.set16(t + 6, w - 2);
        }
    }
    l2::rgb(fx, t + 0x28, 0x80, 0x40, 0);
    let dx = ee::sub(fx.mem.f(sp + 0x60), fx.mem.f(sp + 0x70));
    let dz = ee::sub(fx.mem.f(sp + 0x68), fx.mem.f(sp + 0x78));
    let (xx, zz) = (ee::mul(dx, dx), ee::mul(dz, dz));
    let dy = ee::sub(fx.mem.f(sp + 0x64), fx.mem.f(sp + 0x74));
    let r = fx.sqrtf(ee::add(xx, zz));
    let a = fix(fx, r, 0x01E0_12B4) as i16 as i32;
    let yy = ee::add(ee::mul(dy, dy), zz);
    let r = fx.sqrtf(ee::add(xx, yy));
    let b = fix(fx, r, 0x01E0_12D4) as i16 as i32;
    let ang = angle(a, b) as i16 as i32;
    if fx.mem.i16(COUNTER) < 0x33 {
        let v = if s0 != 0 { ang.wrapping_mul(180) } else { ang.wrapping_neg().wrapping_mul(180) };
        fx.mem.set32(t + 0x10, v);
    }
    let v = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, v);
    let v = fx.mem.i16(t + 0x38) + 1;
    fx.mem.set16(t + 0x38, v);
    fx.leave(0x90);
}

/// 0x108B90(x, y): the angle of (x, y) in degrees by octant (b194's).
fn angle(x: i32, y: i32) -> i32 {
    if x == 0 && y == 0 {
        return 0;
    }
    let (ax, ay) = (i64::from(x.wrapping_abs()), i64::from(y.wrapping_abs()));
    if ax + ay == 0 {
        return 0;
    }
    let lo = |a: i64, b: i64| (45 * a / b) as i32;
    if y >= 0 {
        if x > 0 {
            if ax == ay {
                0x2D
            } else if ay < ax {
                lo(ay, ax)
            } else {
                90 - lo(ax, ay)
            }
        } else if ax == ay {
            0x87
        } else if ay < ax {
            180 - lo(ay, ax)
        } else {
            lo(ax, ay) + 90
        }
    } else if x > 0 {
        if ax == ay {
            0x13B
        } else if ay < ax {
            360 - lo(ay, ax)
        } else {
            lo(ax, ay) + 270
        }
    } else if ax == ay {
        0xE1
    } else if ay < ax {
        lo(ay, ax) + 180
    } else {
        270 - lo(ax, ay)
    }
}

/// 0x1E014A0 (D, task) (frame 0x40): a spark at node 0x3E thrown along
/// (node - D+0x320.. (the x each time)) / 4 + 5 (rand % 22 - 11), size
/// 0x11 growing by 1, cell 2, turns +0x30 / +0x3A rand % 360; colour (0x60,
/// 0x20, 8), alpha 0x80 for 3 updates then down by 10 (over under 0x11).
fn spark(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x40);
    let s0 = u64::from(t);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x11);
        fx.mem.set16(t + 0x34, 2);
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE);
        fx.node_position(h, n, sp + 0x30);
        for (k, ra) in [0x01E0_14F0u32, 0x01E0_14FC, 0x01E0_1508].into_iter().enumerate() {
            let x = fx.mem.f(sp + 0x30 + 4 * k as u32);
        let v = fix(fx, x, ra);
            fx.mem.set16(t + 2 * k as u32, v);
        }
        for (k, ra) in [0x01E0_1538u32, 0x01E0_1568, 0x01E0_1598].into_iter().enumerate() {
            let v = ee::div(ee::sub(fx.mem.f(sp + 0x30), fx.mem.f(D + 0x320 + 4 * k as u32)), 4.0);
            let v = fix(fx, v, ra);
            fx.mem.set32(t + 0x10 + 4 * k as u32, v);
        }
        for (k, ra) in [0x01E0_15A0u32, 0x01E0_15D0, 0x01E0_1600].into_iter().enumerate() {
            let r = rmod(fx, s0, ra, 0x16) - 0xB;
            let o = t + 0x10 + 4 * k as u32;
            let v = fx.mem.i32(o).wrapping_add(5 * r);
            fx.mem.set32(o, v);
        }
        l2::rgb(fx, t + 0x28, 0x60, 0x20, 8);
        let v = rmod(fx, s0, 0x01E0_1640, 0x168);
        fx.mem.set16(t + 0x30, v);
        let v = rmod(fx, s0, 0x01E0_165C, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 3 {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        let a = fx.mem.u8(t + 0x2B);
        if a < 0x11 {
            fx.mem.set16(t + 0x36, 0);
        } else {
            fx.mem.set8(t + 0x2B, a - 0xA);
        }
    }
    if fx.mem.i16(t + 0x32) != 0 {
        for k in 0..3u32 {
            let v = fx.mem.i16(t + 8 + 2 * k).wrapping_add(fx.mem.i16(t + 0x10 + 4 * k));
            fx.mem.set16(t + 8 + 2 * k, v);
        }
    }
    let v = fx.mem.i16(t + 0x32) + 2;
    fx.mem.set16(t + 0x32, v);
    let v = fx.mem.i16(t + 0x38) + 1;
    fx.mem.set16(t + 0x38, v);
    fx.leave(0x40);
}

/// 0x1E01730 (D, task) (frame 0x20): a ring at the spawn point: radius
/// rand % 30 + 10, cell 6, +0x3A -45, angles x / y random quarter turns;
/// colour (0x30, 0x18, 8), alpha 0x40 (+0x2F 0) for 5 updates then down
/// by 7 (over under 0xB).
fn ring(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    let s0 = u64::from(t);
    if fx.mem.i16(t + 0x36) == 1 {
        let v = rmod(fx, s0, 0x01E0_1754, 0x1E) + 0xA;
        fx.mem.set16(t + 0x38, v);
        fx.mem.set16(t + 0x34, 6);
        fx.mem.set16(t + 0x3A, -0x2D);
        l2::rgb(fx, t + 0x28, 0x30, 0x18, 8);
        let v = rand4000(fx, s0, 0x01E0_1794, 0x2000);
        fx.mem.set32(t + 0x18, v);
        let v = rand4000(fx, s0, 0x01E0_17B4, -0x2000);
        fx.mem.set32(t + 0x10, v);
        fx.mem.set32(t + 0x14, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 5 {
        fx.mem.write_u8(t + 0x2B, 0x40);
        fx.mem.write_u8(t + 0x2F, 0);
    } else {
        let a = fx.mem.u8(t + 0x2B);
        if a < 0xB {
            fx.mem.set16(t + 0x36, 0);
        } else {
            fx.mem.set8(t + 0x2B, a - 7);
        }
    }
    let v = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, v);
    fx.leave(0x20);
}

/// 0x1E01920 (D, task) (b403's 0x1E01460 with the block D+0x3E80: (128,
/// 4, 2, 0), the task's point, (0, 63, 0), weight 0): camera 0's slots 4 /
/// 5 (0x1F7E20); the weight up by 0.2 while below 1 (then +0x30 = 1), then
/// down by 0.08 (the task over at most 0.1); also over on timer 1. Frame
/// 0x30.
fn camera(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x4080_0000), (8, 0x4000_0000), (0xC, 0), (0x20, 0), (0x24, 0x427C_0000), (0x28, 0)] {
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
        fx.mem.setf(CAM + 0x28, ee::sub(w, f32::from_bits(0x3DA3_D70A)));
    }
    let (p, ra) = if fx.mem.i16(t + 0x36) == 0 { (0, CAMERA + 0x144) } else { (CAM, CAMERA + 0x158) };
    crate::blib::exe::camera_slots45(fx, p);
    fx.saved(sp - 0x20, &[(0, u64::from(t)), (0x10, u64::from(ra))]);
    if fx.mem.i32(TIMER) == 1 {
        fx.mem.set16(t + 0x36, 0);
    }
    fx.leave(0x30);
}

/// A draw's mode-0 header (a 9-register strip3, prim 0x24 | 0x800, ALPHA
/// 0x80_0000_0048, the RGBAQ of +0x28 through the local `tmp`, UVs of
/// sprite `id` cell +0x34 (uv_rect4 order 0) into `rect`, texture
/// D+0x37C).
fn header(fx: &mut Fx, t: u32, d: u32, id: i32, rect: u32, tmp: u32) {
    let c = fx.mem.i16(t + 0x34);
    l2::rect(fx, rect, id, c);
    fx.mem.write_u16(d + 2, 0x24);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, (4u64 << 32) | 0x2424_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    fx.desc_alpha(d, 0x80_0000_0048);
    fx.mem.write_u32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    l2::rgbaq(fx, tmp, t + 0x28);
    let (a, b) = (fx.mem.read_u32(tmp), fx.mem.read_u32(tmp + 4));
    fx.mem.write_u32(d + 0x10, a);
    fx.mem.write_u32(d + 0x14, b);
    uv::uv_rect4(fx, d, rect, 0);
    let w = fx.mem.read_u32(TEX);
    fx.desc_texture(d, w, 1);
}

/// One of the nine-segment fan draws.
struct Fan9 {
    frame: u32,
    /// The locals' base: points +0, translation +0x40, scale +0x50,
    /// matrix +0x60, the rect +0xA0, the colour local +0xB8.
    base: u32,
    sprite: i32,
    /// The width in degrees: `None` the task's i16 +6.
    width: Option<i32>,
    /// The rim's y the task's i16 +6 (else 0).
    rim_y: bool,
    /// The 0x1A2C70 return address.
    fix: u32,
}

/// 0x1E02A40 (fans 0xC20).
const FAN9_C: Fan9 = Fan9 { frame: 0x140, base: 0x80, sprite: 0x9B, width: Some(2), rim_y: false, fix: 0x01E0_2CA4 };
/// 0x1E02770 (fans 0xD90).
const FAN9_O: Fan9 = Fan9 { frame: 0x150, base: 0x90, sprite: 0x97, width: None, rim_y: false, fix: 0x01E0_29E4 };
/// 0x1E02D00 (fans 0x1060).
const FAN9_R: Fan9 = Fan9 { frame: 0x140, base: 0x80, sprite: 0x9B, width: Some(2), rim_y: true, fix: 0x01E0_2F84 };

/// 0x1E02A40 / 0x1E02770 / 0x1E02D00 (D, task, desc, mode): mode 0 the
/// [`header`]; mode 1 nine fans 40 degrees apart (from +0x3A, `width`
/// wide) of radius +0x38 about the task's i16 point (the centre twice),
/// turned by its angles +0x10, scaled (1, 1.5, 1); keyed by the mean w
/// D+0x4054 on D+0x402C.
fn fan9(fx: &mut Fx, t: u32, d: u32, mode: i32, l: &Fan9) {
    use ee::{from_i32, mul};
    let sp = fx.enter(l.frame);
    let b = sp + l.base;
    if mode == 0 {
        header(fx, t, d, l.sprite, b + 0xA0, b + 0xB8);
    } else {
        for k in 0..3u32 {
            let v = from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(b + 0x40 + 4 * k, v);
        }
        for (k, v) in [0x3F80_0000u32, 0x3FC0_0000, 0x3F80_0000].into_iter().enumerate() {
            fx.mem.write_u32(b + 0x50 + 4 * k as u32, v);
        }
        l2::xform_vp(fx, b + 0x60, b + 0x50, t + 0x10, b + 0x40);
        let width = l.width.unwrap_or_else(|| fx.mem.i16(t + 6));
        let rim = if l.rim_y { from_i32(fx.mem.i16(t + 6)) } else { 0.0 };
        fx.mem.set32(b + 4, 0);
        fx.mem.set32(b + 0x14, 0);
        fx.mem.setf(b + 0x24, rim);
        fx.mem.setf(b + 0x34, rim);
        let mut s = 0i32;
        for _ in 0..9 {
            let a = (fx.mem.i16(t + 0x3A) + s) as i16 as i32;
            let e = (s + (fx.mem.i16(t + 0x3A) + width)) as i16 as i32;
            for (deg, v) in [(a, 0x20u32), (e, 0x30)] {
                let c = l2::sin_deg(fx, TAB, deg, 1);
                let r = from_i32(fx.mem.i16(t + 0x38));
                fx.mem.setf(b + v, mul(r, c));
                let sn = l2::sin_deg(fx, TAB, deg, 0);
                let r = from_i32(fx.mem.i16(t + 0x38));
                fx.mem.setf(b + v + 8, mul(r, sn));
                let z = if v == 0x20 { 0 } else { 0x10 };
                fx.mem.set32(b + z, 0);
                fx.mem.set32(b + z + 8, 0);
            }
            if l2::project(fx, b, SPR, b + 0x60, d, MEAN) {
                let key = fixu(fx, MEAN, l.fix);
                let ot = fx.mem.read_u32(OTW);
                fx.submit(d, ot, key);
            }
            s = (s + 0x28) as i16 as i32;
        }
    }
    fx.leave(l.frame);
}

/// 0x1E03200 (D, task, desc, mode) (b383's 0x1E05090): mode 0 the
/// [`header`] (sprite 0x9B); mode 1 a camera-facing square of the i16 size
/// +0x38 at the task's point plus its drift +8 / +0xA / +0xC over 10
/// (scale 0.1), keyed by the mean w D+0x4054 on D+0x402C.
fn square(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x120);
    if mode == 0 {
        header(fx, t, d, 0x9B, sp + 0xB0, sp + 0x118);
    } else {
        for k in 0..3 {
            fx.mem.write_u32(sp + 0xA0 + 4 * k, 0x3DCC_CCCD);
        }
        camera_angles_saving(fx, sp + 0x100, u64::from(d), u64::from(t), 0x01E0_330C);
        for k in 0..3 {
            let p = ee::from_i32(fx.mem.i16(t + 2 * k));
            let o = ee::div(ee::from_i32(fx.mem.i16(t + 8 + 2 * k)), 10.0);
            fx.mem.setf(sp + 0x90 + 4 * k, ee::add(p, o));
        }
        l2::xform_vp(fx, sp + 0xC0, sp + 0xA0, sp + 0x100, sp + 0x90);
        let v = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x50, v, v);
        if l2::project(fx, sp + 0x50, SPR, sp + 0xC0, d, MEAN) {
            let key = fixu(fx, MEAN, 0x01E0_33E0);
            let ot = fx.mem.read_u32(OTW);
            fx.submit(d, ot, key);
        }
    }
    fx.leave(0x120);
}

/// 0x1E03410 (D, task, desc, mode) (frame 0x140): mode 0 the [`header`]
/// (sprite 0x97); mode 1 nine quads 40 degrees apart (from +0x3A, 4 wide)
/// between radius +0x38 - 17 and +0x38 about the task's point, turned by
/// its angles +0x10, scaled (1, 1.5, 1); keyed by the mean w.
fn band(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    use ee::{from_i32, mul};
    let sp = fx.enter(0x140);
    if mode == 0 {
        header(fx, t, d, 0x97, sp + 0x120, sp + 0x138);
    } else {
        for k in 0..3u32 {
            let v = from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(sp + 0xC0 + 4 * k, v);
        }
        for (k, v) in [0x3F80_0000u32, 0x3FC0_0000, 0x3F80_0000].into_iter().enumerate() {
            fx.mem.write_u32(sp + 0xD0 + 4 * k as u32, v);
        }
        l2::xform_vp(fx, sp + 0xE0, sp + 0xD0, t + 0x10, sp + 0xC0);
        for o in [0x84u32, 0x94, 0xA4, 0xB4] {
            fx.mem.set32(sp + o, 0);
        }
        let mut s = 0i32;
        for _ in 0..9 {
            let a = (fx.mem.i16(t + 0x3A) + s) as i16 as i32;
            let e = (s + (fx.mem.i16(t + 0x3A) + 4)) as i16 as i32;
            for (deg, outer, inner) in [(a, 0xA0u32, 0x80u32), (e, 0xB0, 0x90)] {
                let c = l2::sin_deg(fx, TAB, deg, 1);
                let r = from_i32(fx.mem.i16(t + 0x38));
                fx.mem.setf(sp + outer, mul(r, c));
                let sn = l2::sin_deg(fx, TAB, deg, 0);
                let r = from_i32(fx.mem.i16(t + 0x38));
                fx.mem.setf(sp + outer + 8, mul(r, sn));
                let c = l2::sin_deg(fx, TAB, deg, 1);
                let r = from_i32(fx.mem.i16(t + 0x38) - 0x11);
                fx.mem.setf(sp + inner, mul(r, c));
                let sn = l2::sin_deg(fx, TAB, deg, 0);
                let r = from_i32(fx.mem.i16(t + 0x38) - 0x11);
                fx.mem.setf(sp + inner + 8, mul(r, sn));
            }
            if l2::project(fx, sp + 0x80, SPR, sp + 0xE0, d, MEAN) {
                let key = fixu(fx, MEAN, 0x01E0_371C);
                let ot = fx.mem.read_u32(OTW);
                fx.submit(d, ot, key);
            }
            s = (s + 0x28) as i16 as i32;
        }
    }
    fx.leave(0x140);
}
