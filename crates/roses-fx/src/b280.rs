//! BMODULE b280 (card 334), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02030, data area 0x1ED0000) [F]: b206's layout (a pool of 150 tasks of 0x70 bytes at D+0x2A0,
//! D+0x4Cxx counters) drawn with bands only (853ed233d9de at 0x1E00920,
//! `quads4::ring_band_fy`). On updates 64..320 a widening band a update
//! leaves the attacker's node 0x34 (odd updates) or 0x35 (even; 2 to the
//! facing), swaying with D+0x4C70 (+6 a update), and each update a short
//! band (0.8 more to the facing); both drift sideways from age 3 by their
//! slot. The steps 0x1E01E70 / 0x1E02020 are never spawned and not ported.
//! Returns 1 on update 120, 4 on 360, 2 on 350, 6 on 131..339, 8 once the
//! 382-update timer +0x4C10 has run out.
use crate::blib::quads4;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const WIDE: u32 = 0x01E0_15D0;
const SHORT: u32 = 0x01E0_1AA0;
/// The return address of the entry's `jalr` into a step (0x1E022B0 + 8).
const TASK_RA: u64 = 0x01E0_22B8;
const TIMER: u32 = D + 0x4C10;
const COLOUR: u32 = D + 0x4C14;
const NODES: u32 = D + 0x4C48;
const TARGET_NODE: u32 = D + 0x4C56;
const SWAY: u32 = D + 0x4C70;
const SLOT: u32 = D + 0x4C76;
const FACING: u32 = D + 0x4C7A;
const COUNTER: u32 = D + 0x4C7E;
const BYTES: u32 = D + 0x4C80;
const BYTES2: u32 = D + 0x4D80;

/// The entry 0x1E02030.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    fx.mem.write_u32(D + 0x4C1C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x80);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4FF0);
    l2::sine_table(fx, D + 0x4E80);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x4C7C, param);
    fx.mem.set32(TIMER, 0x17E);
    for (k, node) in [0x25, 0x34, 0x35].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    for k in 0xA..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x288 + 4 * (k as u32 - 0xA), t);
    }
    let t = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, t);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x70);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x74)), 180.0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    // The entry's s0 is D here (rand's frame).
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_2188);
        let (_, h) = fx.div(v, 0x18);
        fx.mem.set8(BYTES + k, h - 0xC);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_21DC);
        fx.mem.set8(BYTES + k, v % 4);
    }
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_222C);
        let (_, h) = fx.div(v, 0x28);
        fx.mem.set8(BYTES2 + k, h - 0x14);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        match f {
            0 => {}
            WIDE => band(fx, t, &WIDE_BAND),
            SHORT => band(fx, t, &SHORT_BAND),
            _ => panic!("b280: unknown task step {f:#x}"),
        }
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == WIDE || f == SHORT {
            for k in 0..4 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(sp + 0x50 + 4 * k, w);
            }
            if f == WIDE {
                let a = (fx.mem.i16(t + 0x5C) + fx.mem.i16(SWAY)) as i16 as i32;
                let s = l2::sin_deg(fx, D + 0xD8, a, 0);
                let x = ee::add(fx.mem.f(sp + 0x50), ee::mul(fx.mem.f(t + 0x30), s));
                fx.mem.setf(sp + 0x50, x);
            }
            quads4::ring_band_fy(fx, D, t, 0x20, sp + 0x50);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x78 => 1,
        0x168 => 4,
        0x15E => 2,
        0x83..=0x153 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// be48a2c22bcb [2 B records; b280 0x1E02530, b708 0x1E022D0, 84 words]:
/// b615's spawn (the first free of the 150 tasks cleared and set off at
/// D+0x250 with the colour D+0x4C14) with the slot D+0x4C76 in both +0x50
/// and +0x52 [F word-compare]. `saves`: its prologue's.
pub(crate) fn spawn52(fx: &mut Fx, step: u32, saves: &[(u32, u64)]) {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
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
        l2::to_i16x4(fx, t, D + 0x250, 0);
        for k in 0..3 {
            let w = fx.mem.read_u32(D + 0x250 + 4 * k);
            fx.mem.write_u32(t + 0x10 + 4 * k, w);
        }
        let s = fx.mem.i16(SLOT);
        fx.mem.set16(t + 0x50, s);
        let s = fx.mem.i16(SLOT);
        fx.mem.set16(t + 0x52, s);
        fx.mem.write_u32(t + 0x40, 0);
        fx.mem.write_u32(t + 0x44, step);
    }
    fx.leave(0x60);
}

/// 0x1E012D0(D, 0): the update counter and the emitter (frame 0x40: ra,
/// the entry's s2 = the parameter, s1 = 0, s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, 0x01E0_2278), (0x20, p), (0x10, 0), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES);
    fx.node_position(h, slot, D + 0x250);
    let z = fx.mem.f(D + 0x258);
    let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 2.0) } else { ee::add(z, 2.0) };
    fx.mem.setf(D + 0x258, z);
    let mut s1 = 0u64;
    let at_slot = |fx: &mut Fx| {
        let c = fx.mem.i16(COUNTER);
        fx.mem.set16(SLOT, c % 2);
        let node = if fx.mem.i16(SLOT) != 0 { NODES + 2 } else { NODES + 4 };
        let h = fx.self_handle();
        let slot = fx.mem.i16(node);
        fx.node_position(h, slot, D + 0x250);
    };
    let c = fx.mem.i16(COUNTER);
    if (0x40..0x141).contains(&c) {
        at_slot(fx);
        spawn52(fx, WIDE, &[(0x50, 0x01E0_140C), (0x40, 0), (0x30, 0), (0x20, u64::from(D)), (0x10, 1), (0, 0)]);
        s1 = 1;
    }
    if fx.mem.i16(COUNTER) != 0 {
        at_slot(fx);
        if fx.mem.i16(FACING) != 0 {
            let z = ee::sub(fx.mem.f(D + 0x258), f32::from_bits(0x3F4C_CCCD));
            fx.mem.setf(D + 0x258, z);
        }
        spawn52(fx, SHORT, &[(0x50, 0x01E0_14EC), (0x40, 0), (0x30, 0), (0x20, u64::from(D)), (0x10, s1), (0, 0)]);
    }
    let h = fx.target_handle();
    let slot = fx.mem.i16(TARGET_NODE);
    fx.node_position(h, slot, D + 0x250);
    let h = fx.target_handle();
    let r = crate::blib::afterimage::unit_radius(fx, h);
    let by = ee::mul(3.0, r);
    let z = fx.mem.f(D + 0x258);
    let z = if fx.mem.i16(FACING) != 0 { ee::add(z, by) } else { ee::sub(z, by) };
    fx.mem.setf(D + 0x258, z);
    let a = fx.mem.i16(SWAY) + 6;
    fx.mem.set16(SWAY, a);
    let a = fx.mem.i16(SWAY);
    if a >= 0x168 {
        let (_, m) = fx.div(a, 0x168);
        fx.mem.set16(SWAY, m);
    }
    fx.leave(0x40);
}

/// The facing side's alpha (+0x67 facing, else +0x63): in by `by` under
/// 0x80 while the age is below `ages`, then out by 9 (the task gone below
/// 0xB).
fn fade(fx: &mut Fx, t: u32, ages: i32, by: u8) {
    let o = if fx.mem.i16(FACING) != 0 { 0x67 } else { 0x63 };
    let a = fx.mem.read_u8(t + o);
    if fx.mem.i16(t + 0x54) < ages {
        if a < 0x80 {
            fx.mem.write_u8(t + o, a.wrapping_add(by));
        }
    } else if a < 0xB {
        fx.mem.set16(t + 0x58, 0);
    } else {
        fx.mem.write_u8(t + o, a - 9);
    }
}

/// 0x1E015D0 (`wide`) / 0x1E01AA0 (D, task): a band (frame 0x30: ra, the
/// entry's s1 = 0, s0 = D): yellow (0x80, 0x80, 0x10) both sides, the
/// facing side's colour flickering white / grey 0x60 and its alpha in by
/// 0x40 for 35 (7) ages, then out by 9; angles (0x4000, 0, 0); width +0x34
/// by 1.6 to +-50 (by the facing); from age 10 +0x5A by 30. `wide`: sway
/// +0x30 by 0.8 below 6, inner radius by 7.5 for 5 ages, then by +0x38 +
/// 14.5 sin(+0x5A) (+0x38 by 0.2 below 20), +0x5C 0xB4 (slot 0) / 0 then
/// by 20, sideways 0.4 from age 3, 5 to the facing a update; else sway by
/// 0.2 below 10, radius by 3.5, sideways 0.3.
pub(crate) fn band(fx: &mut Fx, t: u32, b: &Band) {
    let wide = b.wide;
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, b.ra), (0x10, 0), (0, u64::from(D))]);
    let facing = |fx: &mut Fx| fx.mem.i16(FACING) != 0;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        let (r, g, bl) = b.colour;
        l2::rgb(fx, t + 0x60, r, g, bl);
        l2::rgb(fx, t + 0x64, r, g, bl);
        fx.mem.set16(t + 0x58, 2);
        if wide {
            let a = if fx.mem.i16(t + 0x50) == 0 { 0xB4 } else { 0 };
            fx.mem.set16(t + 0x5C, a);
            fx.mem.write_u32(t + 0x38, 0);
        }
        fx.mem.write_u8(t + 0x67, 0);
    }
    fade(fx, t, b.ages, b.alpha_in);
    let o = if facing(fx) { 0x60 } else { 0x64 };
    let g = if fx.mem.i16(t + 0x54) % 2 == 1 { 0x80 } else { 0x60 };
    l2::rgb(fx, t + o, g, g, g);
    fx.mem.write_u32(t + 0x20, 0x4000);
    let w = fx.mem.f(t + 0x34);
    let step = f32::from_bits(0x3FCC_CCCD);
    if facing(fx) {
        if w < 50.0 {
            fx.mem.setf(t + 0x34, ee::add(w, step));
        }
    } else if !(w <= -50.0) {
        fx.mem.setf(t + 0x34, ee::sub(w, step));
    }
    let (cap, by) = if wide { (6.0, f32::from_bits(0x3F4C_CCCD)) } else { (10.0, f32::from_bits(0x3E4C_CCCD)) };
    let s = fx.mem.f(t + 0x30);
    if s < cap {
        fx.mem.setf(t + 0x30, ee::add(s, by));
    }
    if fx.mem.i16(t + 0x54) >= 0xA {
        let a = fx.mem.i16(t + 0x5A) + 0x1E;
        fx.mem.set16(t + 0x5A, a);
    }
    if !wide {
        let r = ee::add(fx.mem.f(t + 0x48), 3.5);
        fx.mem.setf(t + 0x48, r);
    } else if fx.mem.i16(t + 0x54) < b.sweep_ages {
        let r = ee::add(fx.mem.f(t + 0x48), 7.5);
        fx.mem.setf(t + 0x48, r);
    } else {
        let a = fx.mem.i16(t + 0x5A);
        let s = l2::sin_deg(fx, D + 0x4E80, a, 0);
        let e = ee::add(fx.mem.f(t + 0x38), ee::mul(b.amplitude, s));
        let r = ee::add(fx.mem.f(t + 0x48), e);
        fx.mem.setf(t + 0x48, r);
        let g = fx.mem.f(t + 0x38);
        if g < 20.0 {
            fx.mem.setf(t + 0x38, ee::add(g, f32::from_bits(0x3E4C_CCCD)));
        }
    }
    let o = ee::sub(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x4C, o);
    if wide {
        let a = fx.mem.i16(t + 0x5C) + 0x14;
        fx.mem.set16(t + 0x5C, a);
    }
    if fx.mem.i16(t + 0x54) >= 3 {
        let by = f32::from_bits(if wide { 0x3ECC_CCCD } else { 0x3E99_999A });
        let plus = (fx.mem.i16(t + 0x50) == 0) == facing(fx);
        let x = fx.mem.f(t + 0x10);
        fx.mem.setf(t + 0x10, if plus { ee::add(x, by) } else { ee::sub(x, by) });
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    if wide && fx.mem.i16(t + 0x54) != 0 {
        let z = fx.mem.f(t + 0x18);
        let z = if facing(fx) { ee::sub(z, 5.0) } else { ee::add(z, 5.0) };
        fx.mem.setf(t + 0x18, z);
    }
    if b.sink {
        let y = ee::sub(fx.mem.f(t + 0x14), 0.5);
        fx.mem.setf(t + 0x14, y);
        if fx.mem.i16(COUNTER) >= 0xCA {
            let n = fx.mem.i16(t + 0x54) + 0x19;
            fx.mem.set16(t + 0x54, n);
        }
    }
    fx.leave(0x30);
}

/// A band step's constants ([`band`]).
pub(crate) struct Band {
    /// 0x1E015D0's kind (else 0x1E01AA0's).
    pub wide: bool,
    /// The entry's `jalr` return address.
    pub ra: u64,
    /// Both sides' first colour.
    pub colour: (i32, i32, i32),
    /// Ages the facing side's alpha rises for, and by how much.
    pub ages: i32,
    pub alpha_in: u8,
    /// `wide`: ages the radius grows by 7.5 for, then the sweep's sine
    /// amplitude.
    pub sweep_ages: i32,
    pub amplitude: f32,
    /// b708: it sinks by 0.5 a update and ages 25 more once the update is
    /// past 201.
    pub sink: bool,
}

const WIDE_BAND: Band = Band { wide: true, ra: TASK_RA, colour: (0x80, 0x80, 0x10), ages: 0x23, alpha_in: 0x40, sweep_ages: 5, amplitude: 14.5, sink: false };
const SHORT_BAND: Band = Band { wide: false, ra: TASK_RA, colour: (0x80, 0x80, 0x10), ages: 7, alpha_in: 0x40, sweep_ages: 5, amplitude: 14.5, sink: false };
