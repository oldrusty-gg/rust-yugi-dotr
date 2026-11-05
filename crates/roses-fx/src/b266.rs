//! BMODULE b266 (card 142), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01D50) [F]. The entry
//! takes (D, param). b194's task-pool family: 150 tasks of 0x40 at D+0x774
//! (alive +0x36, step +0x24), spawned by 0x1E02520 (b538's spawn: the first
//! free task, set off at D+0x2D0 as i16 with the colour D+0x2EA4 twice,
//! +0x30 = the kind D+0x2EFA), the sines D+0x3004, timer D+0x2EA0 = 0xBE,
//! the OT at D+0x80 and D+0x2EAC. Nodes (the attacker's) 0x18 / 0x29 / 0xD
//! / 0x1D at D+0x2EE8.., textures (4, 0xA..0xF) D+0x368.. and (4, 2)
//! D+0x348; init builds nine fan blades at D+0x380 + 0x40 k (quad_xy(5,
//! 50) then y = 15 / 35 sin(20 k, 20 k + 20) - 10 / + 15, x + 10 k - 45)
//! and 256 jitter bytes D+0x2F02 (rand % 30 - 15).
//!
//! The emitter 0x1E00800 keeps two attacker positions (D+0x320 / D+0x330,
//! taken on update 1) pushed sideways each update by the attacker's facing
//! and spawns only five steps: 0x1E011C0 (rising arcs: 0x5C..0x83 from
//! D+0x320, 0x77..0x9A from D+0x330), 0x1E01060 (fans) and 0x1E01610
//! (b112's drift) in bursts (0x7E..0x88, 0x97..0x9C), 0x1E01480 (the blade
//! fan, re-randomised every update: +0x36 stays 1) and 0x1E018F0 (embers,
//! 12 a update at nodes 0x18 / 0x29 on 0x24..0x28). 0x1E01390 / 0x17B0 /
//! 0x1A60 / 0x1C40 / 0x1D00 and the draws 0x1E02D50 / 0x1E03340 are never
//! spawned (the 0x17B0 spawns sit behind update 0xAB inside 0x7E..0x88)
//! [F]. Draws: arcs 0x1E02640 / fans 0x1E029F0 / drifts 0x1E03130 (b194's
//! 9-value descriptor with uv_rect4 0x1E00080, ALPHA 0x48), the blade fan
//! 0x1E02F10 (the nine blades through D+0x290 = the camera x TRS, far Z),
//! embers billboard::ring_t20. Returns 1 on 0x7C, 4 on 0xB4, 2 on 0xA5, 6
//! on 0x87..0x9F, 8 once the timer has run out.
use crate::blib::billboard;
use crate::blib::uv::uv_rect4;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 0x96;
const POOL: u32 = D + 0x774;
const OT_A: u32 = D + 0x80;
const POS: u32 = D + 0x2D0;
const VP: u32 = D + 0x290;
const AT_A: u32 = D + 0x320;
const AT_B: u32 = D + 0x330;
const TEX: u32 = D + 0x37C;
const BLADES: u32 = D + 0x380;
const TIMER: u32 = D + 0x2EA0;
const COLOUR: u32 = D + 0x2EA4;
const OTW: u32 = D + 0x2EAC;
const MEAN_W: u32 = D + 0x2ED4;
const NODE: u32 = D + 0x2EE8;
const KIND: u32 = D + 0x2EFA;
const PARAM: u32 = D + 0x2EFC;
const COUNTER: u32 = D + 0x2EFE;
const BYTES: u32 = D + 0x2F02;
const TAB: u32 = D + 0x3004;

const FAN: u32 = 0x01E0_1060;
const ARC: u32 = 0x01E0_11C0;
const BLADE: u32 = 0x01E0_1480;
const DRIFT: u32 = 0x01E0_1610;
const EMBER: u32 = 0x01E0_18F0;

pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x130);
    fx.mem.write_u32(OT_A, OT);
    fx.mem.write_u32(OTW, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp) };
    fx.leave(0x130);
    ret
}

fn rmod(fx: &mut Fx, m: i32) -> i32 {
    let v = fx.rand();
    fx.div(v, m).1
}

fn fix(fx: &mut Fx, v: f32) -> i32 {
    fx.fix_residue(v, 0);
    ee::fixsfsi(v)
}

fn fixu(fx: &mut Fx, at: u32) -> i32 {
    let v = fx.mem.f(at);
    fx.fix_residue(v, 0);
    ee::fixunssfsi(v) as i32
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3170);
    l2::sine_table(fx, TAB);
    fx.mem.set16(PARAM, param);
    fx.mem.set32(TIMER, 0xBE);
    for o in [0x310u32, 0x314, 0x318] {
        fx.mem.set32(D + o, 0);
    }
    for (k, (n, clear)) in [(0x18, 1), (0x29, 0), (0xD, 0), (0x1D, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(NODE + 2 * k as u32, r);
    }
    for k in 0xAu32..0x10 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x368 + 4 * (k - 0xA), t);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x348, t);
    for k in 0..0xAu32 {
        l2::quad_xy(fx, BLADES + 0x40 * k, 5.0, 50.0);
    }
    let (mut a, mut b, mut x) = (0i32, 0x14i32, -0x2Di32);
    for k in 0..9u32 {
        let q = BLADES + 0x40 * k;
        for (o, deg, m) in [(4u32, a, 15.0f32), (0x14, b, 15.0), (0x24, a, 35.0), (0x34, b, 35.0)] {
            let s = l2::sin_deg(fx, TAB, deg as i16 as i32, 0);
            fx.mem.setf(q + o, ee::mul(m, s));
        }
        let fx_ = ee::from_i32(x);
        for o in [0u32, 0x10, 0x20, 0x30] {
            let v = ee::add(fx.mem.f(q + o), fx_);
            fx.mem.setf(q + o, v);
        }
        for (o, d) in [(4u32, -10.0f32), (0x14, -10.0), (0x24, 15.0), (0x34, 15.0)] {
            let v = if d < 0.0 { ee::sub(fx.mem.f(q + o), 10.0) } else { ee::add(fx.mem.f(q + o), 15.0) };
            fx.mem.setf(q + o, v);
        }
        for o in [8u32, 0x18, 0x28, 0x38] {
            fx.mem.set32(q + o, 0);
        }
        a += 0x14;
        b += 0x14;
        x += 0xA;
    }
    for k in 0..0x100u32 {
        let r = rmod(fx, 0x1E);
        fx.mem.write_u8(BYTES + k, (r - 0xF) as u8);
    }
    0
}

/// (double) `v` < 0.5 (0x1A2C30 then 0x17BCA0; a denormal converts to 0).
fn below_half(v: f32) -> bool {
    let bits = v.to_bits();
    let a = if (bits >> 23) & 0xFF == 0 { 0.0 } else { f64::from(v) };
    a < 0.5
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            FAN => fan(fx, t),
            ARC => arc(fx, t),
            BLADE => blade(fx, t),
            DRIFT => drift(fx, t),
            EMBER => ember(fx, t),
            _ => {}
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x70);
    let d = sp + 0x98;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let g: fn(&mut Fx, u32, u32, i32) = match fx.mem.read_u32(t + 0x24) {
            FAN => fan_draw,
            ARC => arc_draw,
            BLADE => blade_draw,
            DRIFT => drift_draw,
            EMBER => {
                let w = fx.mem.read_u32(TEX);
                fx.mem.write_u32(D + 0x88, w);
                let c = fx.mem.i16(t + 0x34);
                l2::rect(fx, sp + 0x80, 0x9B, c);
                for k in 0..4 {
                    let w = fx.mem.read_u32(sp + 0x80 + 4 * k);
                    fx.mem.write_u32(D + 0x94 + 4 * k, w);
                }
                billboard::ring_t20(fx, uv_rect4, D, t, d, BYTES);
                continue;
            }
            _ => continue,
        };
        g(fx, t, d, 0);
        g(fx, t, d, 1);
    }
    if fx.mem.i16(COUNTER) >= 0x15 {
        for o in [0x310u32, 0x318] {
            if o == 0x318 {
                fx.mem.write_u32(D + 0x314, 0x3DCC_CCCD);
            }
            let v = fx.mem.f(D + o);
            if below_half(v) {
                let v = ee::add(fx.mem.f(D + o), f32::from_bits(0x3C23_D70A));
                fx.mem.setf(D + o, v);
            }
        }
    }
    let n = fx.mem.i32(TIMER);
    if n != 0 {
        fx.mem.set32(TIMER, n - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x7C => 1,
        0xB4 => 4,
        0xA5 => 2,
        0x87..=0x9F => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E02520(D, step, 0): the first free task from 0 (b538's spawn).
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

/// The attacker's yaw (0x1F7370 into `out`, rotation y / 180 through
/// 0x1A2B80) as an i16.
fn yaw(fx: &mut Fx, out: u32) -> i32 {
    let h = fx.self_handle();
    fx.unit_rotation(h, out);
    let y = ee::div(ee::from_i32(fx.mem.i32(out + 4)), 180.0);
    fix(fx, y) as i16 as i32
}

/// The attacker position `at` pushed by `push` along z by the facing, then
/// the spawn point = (x, y - 10, z +- 40).
fn push(fx: &mut Fx, at: u32, push: f32, yaw: i32) {
    let z = fx.mem.f(at + 8);
    let z = if yaw != 0 { ee::sub(z, push) } else { ee::add(z, push) };
    fx.mem.setf(at + 8, z);
    place(fx, at, yaw);
}

fn place(fx: &mut Fx, at: u32, yaw: i32) {
    let x = fx.mem.read_u32(at);
    fx.mem.write_u32(POS, x);
    let y = ee::sub(fx.mem.f(at + 4), 10.0);
    fx.mem.setf(POS + 4, y);
    let z = fx.mem.f(at + 8);
    let z = if yaw != 0 { ee::add(40.0, z) } else { ee::sub(z, 40.0) };
    fx.mem.setf(POS + 8, z);
}

fn burst(fx: &mut Fx, drifts: u32) {
    fx.mem.set16(KIND, 0);
    for _ in 0..2 {
        spawn(fx, FAN);
    }
    for _ in 0..drifts {
        spawn(fx, DRIFT);
    }
    if fx.mem.i16(COUNTER) == 0xAB {
        for k in 0..4 {
            fx.mem.set16(KIND, k);
            spawn(fx, 0x01E0_17B0);
        }
    }
}

/// 0x1E00800(D, 0) (frame 0xB0): the spawn colour (0x80, 0, 0, 0), the
/// update counter and the spawns.
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0xB0);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    if c == 1 {
        for at in [AT_A, AT_B] {
            let h = fx.self_handle();
            fx.unit_position(h, at);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x5C..0x84).contains(&c) {
        let y = yaw(fx, sp + 0x60);
        push(fx, AT_A, f32::from_bits(0x408C_CCCD), y);
        fx.mem.set16(KIND, 0);
        spawn(fx, ARC);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x7E..0x89).contains(&c) {
        let y = yaw(fx, sp + 0x70);
        push(fx, AT_A, 3.0, y);
        burst(fx, 5);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x77..0x9B).contains(&c) {
        let y = yaw(fx, sp + 0x80);
        push(fx, AT_B, f32::from_bits(0x40B9_999A), y);
        fx.mem.set16(KIND, 1);
        spawn(fx, ARC);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x97..0x9D).contains(&c) {
        let y = yaw(fx, sp + 0x90);
        push(fx, AT_B, 3.0, y);
        burst(fx, 6);
    }
    let y = yaw(fx, sp + 0xA0);
    let c = fx.mem.i16(COUNTER);
    if (0x5A..0x85).contains(&c) {
        place(fx, AT_A, y);
        fx.mem.set16(KIND, 0);
        spawn(fx, BLADE);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x75..0x9C).contains(&c) {
        place(fx, AT_B, y);
        fx.mem.set16(KIND, 1);
        spawn(fx, BLADE);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x24..0x29).contains(&c) {
        for slot in 0..2u32 {
            let h = fx.self_handle();
            let n = fx.mem.i16(NODE + 2 * slot);
            fx.node_position(h, n, sp + 0x50);
            for c in 0..3 {
                let w = fx.mem.read_u32(sp + 0x50 + 4 * c);
                fx.mem.write_u32(POS + 4 * c, w);
            }
            fx.mem.set16(KIND, slot as i32);
            for _ in 0..6 {
                spawn(fx, EMBER);
            }
        }
    }
    fx.leave(0xB0);
}

fn age(fx: &mut Fx, t: u32) -> i32 {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    n
}

fn fade_in(fx: &mut Fx, t: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a < cap {
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(up));
    }
}

/// Alpha +0x2B down by `down` while at least `floor`, else the task ends.
fn fade_out(fx: &mut Fx, t: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a >= floor {
        fx.mem.write_u8(t + 0x2B, a.wrapping_sub(down));
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// i16 `at` += `k` x (sine or cosine of i16 `deg`) through 0x1A2B80.
fn wobble(fx: &mut Fx, t: u32, at: u32, deg: u32, cos: i32, k: f32) {
    let a = fx.mem.i16(t + deg);
    let s = l2::sin_deg(fx, TAB, a, cos);
    let f = ee::add(ee::from_i32(fx.mem.i16(t + at)), ee::mul(k, s));
    let r = fix(fx, f);
    fx.mem.set16(t + at, r);
}

/// 0x1E01060 (D, task): an orange fan (size 8 up by 4, +0x18 (rand &
/// 0x3FFF) - 0x2000, +0x14 rand % 0xFFFF, cell rand % 7).
fn fan(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 8);
        let r = fx.rand();
        fx.mem.set32(t + 0x18, (r & 0x3FFF) - 0x2000);
        let r = rmod(fx, 0xFFFF);
        fx.mem.set32(t + 0x14, r);
        let r = rmod(fx, 7);
        fx.mem.set16(t + 0x34, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 2 {
        fade_in(fx, t, 0xA0, 0x50);
    } else {
        if fx.mem.i16(t + 0x38) >= 0x15 {
            fade_out(fx, t, 0xF, 0xD);
        }
        if fx.mem.i32(TIMER) < 0xA {
            fade_out(fx, t, 0x11, 0xF);
        }
    }
    l2::rgb(fx, t + 0x28, 0x80, 0x40, 0);
    age(fx, t);
    let s = fx.mem.i16(t + 0x38) + 4;
    fx.mem.set16(t + 0x38, s);
}

/// 0x1E011C0 (D, task) (frame 0x40): a dark arc (size 0x32) turned by the
/// facing (+0x10 -0x4000 / -0xC000, +0x18 +-0x1800 by kind), its angle
/// +0x30 from 0x5A up by 8 to 0xA0.
fn arc(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x40);
    l2::rgb(fx, t + 0x28, 0x18, 0x18, 0);
    if fx.mem.i16(t + 0x36) == 1 {
        let y = yaw(fx, sp + 0x30);
        fx.mem.set32(t + 0x10, if y != 0 { -0x4000 } else { -0xC000 });
        fx.mem.set32(t + 0x14, 0);
        let k0 = fx.mem.i16(t + 0x30) == 0;
        let v = if (y != 0) == k0 { 0x1800 } else { -0x1800 };
        fx.mem.set32(t + 0x18, v);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x30, 0x5A);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0);
    }
    if fx.mem.i16(t + 0x32) < 2 {
        fade_in(fx, t, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0xB, 9);
    }
    fx.mem.set16(t + 0x38, 0x32);
    age(fx, t);
    let a = fx.mem.i16(t + 0x30);
    if a < 0xA0 {
        fx.mem.set16(t + 0x30, a + 8);
    }
    if fx.mem.i32(TIMER) < 0xF {
        fade_out(fx, t, 0xF, 0xD);
    }
    fx.leave(0x40);
}

/// 0x1E01480 (D, task) (frame 0x40): the grey blade fan, its set-up run
/// every update (+0x36 is never set to 2): size (rand & 7) + 3, +0x10 0 /
/// 0x8000 by the facing, +0x18 +-0x2800 by kind, +6 = kind, cell rand % 7.
fn blade(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x40);
    let y = yaw(fx, sp + 0x30);
    l2::rgb(fx, t + 0x28, 0xA0, 0xA0, 0xA0);
    if fx.mem.i16(t + 0x36) == 1 {
        let r = fx.rand();
        fx.mem.set16(t + 0x38, (r & 7) + 3);
        fx.mem.set32(t + 0x10, if y != 0 { 0 } else { 0x8000 });
        let k0 = fx.mem.i16(t + 0x30) == 0;
        let v = if (y != 0) == k0 { -0x2800 } else { 0x2800 };
        fx.mem.set32(t + 0x18, v);
        let k = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 6, k);
        let r = rmod(fx, 7);
        fx.mem.set16(t + 0x34, r);
    }
    if fx.mem.i16(t + 0x32) < 4 {
        fade_in(fx, t, 0x50, 0x14);
    } else {
        fade_out(fx, t, 0xF, 0xD);
    }
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E01610 (D, task): b112's drift in (0x60, 0x40, 8).
fn drift(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0xD);
        fx.mem.set16(t + 0x34, 2);
        l2::rgb(fx, t + 0x28, 0x60, 0x40, 8);
        let r = rmod(fx, 0x168);
        fx.mem.set16(t + 0x30, r);
        let r = rmod(fx, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0xA {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fade_out(fx, t, 0x11, 0xA);
    }
    age(fx, t);
    wobble(fx, t, 8, 0x3A, 0, 9.0);
    wobble(fx, t, 0xA, 0x30, 1, 9.0);
    wobble(fx, t, 0xC, 0x3A, 1, 9.0);
}

/// 0x1E018F0 (D, task): an ember ring (size 2, cell rand & 3, orange,
/// +0x18 rand % 0xFFFF, +0x3A rand % 360, spin +0xE (rand & 0xF) + 3, +8
/// rand % 10, life +6 rand % 3 + 1), its +0x30 turning by +0xE.
fn ember(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 2);
        let r = fx.rand();
        fx.mem.set16(t + 0x34, r & 3);
        l2::rgb(fx, t + 0x28, 0x80, 0x40, 0);
        let r = rmod(fx, 0xFFFF);
        fx.mem.set32(t + 0x18, r);
        fx.mem.set16(t + 0x30, 0);
        let r = rmod(fx, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let r = fx.rand();
        fx.mem.set16(t + 0xE, (r & 0xF) + 3);
        let r = rmod(fx, 0xA);
        fx.mem.set16(t + 8, r);
        let r = rmod(fx, 3);
        fx.mem.set16(t + 6, r + 1);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fade_out(fx, t, 0x11, 0xA);
    }
    let a = fx.mem.i16(t + 0x30).wrapping_add(fx.mem.i16(t + 0xE));
    fx.mem.set16(t + 0x30, a);
    age(fx, t);
}

/// Mode 0 of the arcs / fans / drifts: a 9-value descriptor (RGBAQ + four
/// UV / XYZ2 pairs, flags 0xA24, ALPHA 0x48), colour +0x28, UVs by
/// uv_rect4 of rect `id` cell +0x34 (into `r`), texture D+0x37C.
fn desc9(fx: &mut Fx, t: u32, d: u32, r: u32, q: u32, id: i32) {
    let c = fx.mem.i16(t + 0x34);
    l2::rect(fx, r, id, c);
    fx.mem.set16(d + 2, 0x24);
    fx.mem.set16(d, 9);
    fx.mem.write_u64(d + 8, (4 << 32) | 0x2424_2421);
    fx.mem.set32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    fx.desc_alpha(d, (0x80 << 32) | 0x48);
    fx.mem.set32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    l2::rgbaq(fx, q, t + 0x28);
    for k in 0..2 {
        let w = fx.mem.read_u32(q + 4 * k);
        fx.mem.write_u32(d + 0x10 + 4 * k, w);
    }
    uv_rect4(fx, d, r, 0);
    let w = fx.mem.read_u32(TEX);
    fx.desc_texture(d, w, 1);
}

fn trans(fx: &mut Fx, t: u32, out: u32) {
    for k in 0..3u32 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(out + 4 * k, v);
    }
}

fn submit_mean(fx: &mut Fx, d: u32) {
    let key = fixu(fx, MEAN_W);
    let ot = fx.mem.read_u32(OTW);
    fx.submit(d, ot, key);
}

/// 0x1E02640 (D, task, desc, mode) (frame 0x150): an arc of nine quads, 40
/// degrees each from +0x3A (cosines x 3 cos(s) / 3 cos(s + 30), sines x 40
/// cos(s) / 30 cos(s + 30), s = min(+0x30, 0x87)) at heights 45 sin(+0x30)
/// / 45 sin(+0x30 + 35), turned by +0x10 at the task's point.
fn arc_draw(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    use ee::mul;
    let sp = fx.enter(0x150);
    if mode == 0 {
        desc9(fx, t, d, sp + 0x130, sp + 0x148, 0x97);
        fx.leave(0x150);
        return;
    }
    trans(fx, t, sp + 0xE0);
    l2::xform_vp(fx, sp + 0xF0, 0, t + 0x10, sp + 0xE0);
    let a = fx.mem.i16(t + 0x30);
    let s0 = a.min(0x87);
    let v = mul(45.0, l2::sin_deg(fx, TAB, a, 0));
    fx.mem.setf(sp + 0xA4, v);
    fx.mem.setf(sp + 0xB4, v);
    let b = (fx.mem.i16(t + 0x30) + 0x23) as i16 as i32;
    let v = mul(45.0, l2::sin_deg(fx, TAB, b, 0));
    fx.mem.setf(sp + 0xC4, v);
    fx.mem.setf(sp + 0xD4, v);
    let f20 = mul(40.0, l2::sin_deg(fx, TAB, s0, 1));
    let s2 = (s0 + 0x1E) as i16 as i32;
    let f21 = mul(30.0, l2::sin_deg(fx, TAB, s2, 1));
    let f22 = mul(3.0, l2::sin_deg(fx, TAB, s0, 1));
    let f23 = mul(3.0, l2::sin_deg(fx, TAB, s2, 1));
    let mut s = 0i32;
    for _ in 0..9 {
        for (off, base) in [(0i32, 0u32), (0x28, 0x10)] {
            let a = (fx.mem.i16(t + 0x3A) + off + s) as i16 as i32;
            let c = l2::sin_deg(fx, TAB, a, 1);
            let n = l2::sin_deg(fx, TAB, a, 0);
            fx.mem.setf(sp + 0xC0 + base, mul(c, f22));
            fx.mem.setf(sp + 0xC8 + base, mul(n, f21));
            fx.mem.setf(sp + 0xA0 + base, mul(c, f23));
            fx.mem.setf(sp + 0xA8 + base, mul(n, f20));
        }
        if l2::project(fx, sp + 0xA0, SPR, sp + 0xF0, d, MEAN_W) {
            submit_mean(fx, d);
        }
        s = (s + 0x28) as i16 as i32;
    }
    fx.leave(0x150);
}

/// 0x1E029F0 (D, task, desc, mode) (frame 0x140): a fan of nine 40-degree
/// quads from +0x3A between radius +0x38 - 17 and +0x38, turned by +0x10,
/// scaled (1, 1.5, 1) at the task's point.
fn fan_draw(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    use ee::mul;
    let sp = fx.enter(0x140);
    if mode == 0 {
        desc9(fx, t, d, sp + 0x120, sp + 0x138, 0x9B);
        fx.leave(0x140);
        return;
    }
    trans(fx, t, sp + 0xC0);
    for (o, v) in [(0xD0u32, 0x3F80_0000u32), (0xD4, 0x3FC0_0000), (0xD8, 0x3F80_0000)] {
        fx.mem.write_u32(sp + o, v);
    }
    l2::xform_vp(fx, sp + 0xE0, sp + 0xD0, t + 0x10, sp + 0xC0);
    for o in [0x84u32, 0x94, 0xA4, 0xB4] {
        fx.mem.write_u32(sp + o, 0);
    }
    let mut s = 0i32;
    for _ in 0..9 {
        for (off, base) in [(0i32, 0u32), (0x28, 0x10)] {
            let a = (fx.mem.i16(t + 0x3A) + off + s) as i16 as i32;
            let r = |fx: &mut Fx, k: i32| ee::from_i32(fx.mem.i16(t + 0x38) + k);
            let c = l2::sin_deg(fx, TAB, a, 1);
            let v = mul(r(fx, 0), c);
            fx.mem.setf(sp + 0xA0 + base, v);
            let n = l2::sin_deg(fx, TAB, a, 0);
            let v = mul(r(fx, 0), n);
            fx.mem.setf(sp + 0xA8 + base, v);
            let c = l2::sin_deg(fx, TAB, a, 1);
            let v = mul(r(fx, -0x11), c);
            fx.mem.setf(sp + 0x80 + base, v);
            let n = l2::sin_deg(fx, TAB, a, 0);
            let v = mul(r(fx, -0x11), n);
            fx.mem.setf(sp + 0x88 + base, v);
        }
        if l2::project(fx, sp + 0x80, SPR, sp + 0xE0, d, MEAN_W) {
            submit_mean(fx, d);
        }
        s = (s + 0x28) as i16 as i32;
    }
    fx.leave(0x140);
}

/// 0x1E02F10 (D, task, desc, mode) (frame 0xF0): mode 0 a strip3 (rect
/// 0x97 cell 0, colours +0x28 / +0x2C, texture D+0x37C); mode 1 the nine
/// init blades D+0x380.. through D+0x290 = the camera x TRS((0.55, 1, 1),
/// (+0x10 - 0x4000, 0, +0x18), the task's point), at the far Z.
fn blade_draw(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0xF0);
    if mode == 0 {
        l2::rect(fx, sp + 0x60, 0x97, 0);
        l2::desc_strip3(fx, d);
        for (k, (o, c)) in [(0x10u32, 0x28u32), (0x28, 0x28), (0x40, 0x2C), (0x58, 0x2C)].into_iter().enumerate() {
            let q = sp + 0xD0 + 8 * k as u32;
            l2::rgbaq(fx, q, t + c);
            for j in 0..2 {
                let w = fx.mem.read_u32(q + 4 * j);
                fx.mem.write_u32(d + o + 4 * j, w);
            }
        }
        l2::uv_strip3(fx, d, sp + 0x60, 0);
        let w = fx.mem.read_u32(TEX);
        fx.desc_texture(d, w, 1);
        fx.leave(0xF0);
        return;
    }
    fx.camera_vp_to(VP);
    let p = [0u32, 2, 4].map(|o| ee::from_i32(fx.mem.i16(t + o)));
    l2::set3(fx, sp + 0x50, p[0], p[1], p[2]);
    let rot = [fx.mem.i32(t + 0x10).wrapping_sub(0x4000), 0, fx.mem.i32(t + 0x18)];
    let m = fx.trs(Some([f32::from_bits(0x3F0C_CCCD), 1.0, 1.0]), Some(rot), Some(p));
    fx.write_m4(sp + 0x90, &m);
    fx.mul_into(Some(VP), &m);
    for k in 0..9u32 {
        if l2::project(fx, BLADES + 0x40 * k, SPR, VP, d, MEAN_W) {
            l2::far_z_desc(fx, d, 0);
            submit_mean(fx, d);
        }
    }
    fx.leave(0xF0);
}

/// 0x1E03130 (D, task, desc, mode) (frame 0x120): b403's drift_draw on the
/// 9-value descriptor: a camera-facing square of half size +0x38 scaled
/// 0.1 at the task's point + offsets / 10.
fn drift_draw(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x120);
    if mode == 0 {
        desc9(fx, t, d, sp + 0xB0, sp + 0x118, 0x9B);
        fx.leave(0x120);
        return;
    }
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xA0 + 4 * k, 0x3DCC_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0x100);
    for k in 0..3u32 {
        let o = ee::div(ee::from_i32(fx.mem.i16(t + 8 + 2 * k)), 10.0);
        let p = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(sp + 0x90 + 4 * k, ee::add(p, o));
    }
    l2::xform_vp(fx, sp + 0xC0, sp + 0xA0, sp + 0x100, sp + 0x90);
    let s = ee::from_i32(fx.mem.i16(t + 0x38));
    l2::quad_xy(fx, sp + 0x50, s, s);
    if l2::project(fx, sp + 0x50, SPR, sp + 0xC0, d, MEAN_W) {
        submit_mean(fx, d);
    }
    fx.leave(0x120);
}
