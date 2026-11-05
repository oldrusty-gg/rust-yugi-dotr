//! BMODULE b207 (card 376), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03C40, data area 0x1ED0000) [F]: b018's program shape on 150 tasks of 0x40 bytes at D+0x4F4 (alive
//! +0x36, step +0x24, each live step called by its own `jal`), spawned by
//! `tstep::spawn` on colour D+0x3004, parameter D+0x3062 and the point
//! D+0x2D0. A 10-point history D+0x2AF0 of the attacker's node 3 (filled on
//! update 1, pushed by `spawn::node_history`) is kept but only read by dead
//! draws. The emitter 0x1E020D0 counts D+0x3066: on update 1..3 twelve
//! rising flames 0x1E029A0 a update at node 3 (`band9_tex`); on 0xA twelve
//! orbiting glows 0x1E03590 (angles 30 k, `fan18_far0`); on 0x5E..0x81 a
//! white flash (`tstep::white_flash`, `fan18_far0`) a update at node 3; on
//! 0x82 a bright disc 0x1E03780 (`fan18_far0`), twelve growing flames
//! 0x1E02BC0 (`band9_tex`) and twelve smoke bands 0x1E02DE0
//! (`band18_tex`). On update 1 a task with the bare step 0x1E03C30 (never
//! stepped, it only holds its slot). The other steps (0x1E026A0 / 0x28C0 /
//! 0x30D0 / 0x3210 / 0x3380 / 0x3430 / 0x34E0 / 0x3860 / 0x3B20 / 0x3BE0)
//! are tested but never spawned (not ported, nor the draws 0x1E00EF0 /
//! 0x1E04850 / ribbon_tex / ring_t20 / sprite_spec behind them). Returns 1
//! on update 0x78, 4 on 0x8C, 2 on 0x82, 8 once the 180-update timer
//! D+0x3000 has run out. No 0x1F8100.
use crate::blib::{quads4, spawn as bspawn, tstep};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;

const RISE: u32 = 0x01E0_29A0;
const GROW: u32 = 0x01E0_2BC0;
const SMOKE: u32 = 0x01E0_2DE0;
const FLASH: u32 = 0x01E0_3000;
const ORBIT: u32 = 0x01E0_3590;
const DISC: u32 = 0x01E0_3780;
const HOLD: u32 = 0x01E0_3C30;

const POOL: u32 = D + 0x4F4;
const POS: u32 = D + 0x2D0;
const HIST: u32 = D + 0x2AF0;
const KEEP: u32 = D + 0x2AE0;
const TIMER: u32 = D + 0x3000;
const COLOUR: u32 = D + 0x3004;
const OT2: u32 = D + 0x300C;
const NODES: u32 = D + 0x3048;
const PARAM: u32 = D + 0x3062;
const PARAM_COPY: u32 = D + 0x3064;
const COUNTER: u32 = D + 0x3066;
const BYTES: u32 = D + 0x306A;
const SINES: u32 = D + 0x316C;
const TEX: u32 = D + 0x368;
const TEX_F: u32 = D + 0x37C;

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

/// The entry 0x1E03C40 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1A0);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(OT2, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x1A0);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x32E0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(PARAM_COPY, param);
    fx.mem.set32(TIMER, 0xB4);
    for o in [0x310u32, 0x314, 0x318] {
        fx.mem.write_u32(D + o, 0);
    }
    for (k, (n, clear)) in [(3, 1), (4, 0), (0x1A, 0), (0x6A, 0), (0x40, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    for k in 0xA..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(TEX + 4 * (k as u32 - 0xA), t);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x348, t);
    // rand's frame: s0 = D.
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_3D68);
        let r = fx.div(v, 0x1E).1;
        fx.mem.write_u8(BYTES + k, (r - 0xF) as u8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    if counter(fx) == 1 {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODES);
        fx.node_position(h, n, sp + 0xF0);
        for i in 0..9 {
            for k in 0..3 {
                let w = fx.mem.read_u32(sp + 0xF0 + 4 * k);
                fx.mem.write_u32(HIST + 0x10 * i + 4 * k, w);
            }
        }
    }
    let n = fx.mem.i16(NODES);
    bspawn::node_history(fx, HIST + 0x90, 0xA, n);
    for k in 0..150 {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            RISE => run_step(fx, t, 0x30, 0x01E0_3E80, |fx, t, _| flame(fx, t, RISE)),
            GROW => run_step(fx, t, 0x30, 0x01E0_3EA0, |fx, t, _| flame(fx, t, GROW)),
            SMOKE => run_step(fx, t, 0x30, 0x01E0_3EC0, |fx, t, _| smoke(fx, t)),
            FLASH => {
                let f = fx.sp.wrapping_sub(0x20);
                fx.saved(f, &[(0x10, 0x01E0_3EE0), (0, u64::from(D))]);
                tstep::white_flash(fx, t);
            }
            ORBIT => run_step(fx, t, 0x50, 0x01E0_4138, orbit),
            DISC => run_step(fx, t, 0x20, 0x01E0_4158, |fx, t, _| disc(fx, t)),
            _ => {}
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    let d = sp + 0x108;
    for k in 0..150 {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            RISE | GROW => {
                let r = if fx.mem.read_u32(t + 0x24) == RISE { sp + 0x80 } else { sp + 0x90 };
                cell(fx, t, r, 0x9B);
                fx.mem.set16(D + 0xB8, 1);
                quads4::band9_tex(fx, D, t, d);
                fx.mem.set16(D + 0xB8, 0);
            }
            SMOKE => {
                cell(fx, t, sp + 0xA0, 0x97);
                fx.mem.set16(D + 0xB8, 1);
                quads4::band18_tex(fx, D, t, d);
                fx.mem.set16(D + 0xB8, 0);
            }
            FLASH => {
                cell(fx, t, sp + 0xB0, 0x97);
                fx.mem.set16(D + 0xB8, 4);
                fx.mem.set16(D + 0xB8, 0);
                quads4::fan18_far0(fx, D, t, d);
            }
            ORBIT | DISC => quads4::fan18_far0(fx, D, t, d),
            _ => {}
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0x78 => 1,
        0x8C => 4,
        0x82 => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The texture (4, 0xF) and the atlas cell `id` frame +0x34 (0x106960
/// into `r`) copied to D+0x88 / D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32, id: i32) {
    let v = fx.mem.read_u32(TEX_F);
    fx.mem.write_u32(D + 0x88, v);
    let f = fx.mem.i16(t + 0x34);
    l2::rect(fx, r, id, f);
    for k in 0..4 {
        let w = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, w);
    }
}

/// A step's frame and its saves of the entry's s0 = D (and s1 = 0 in the
/// 0x30 / 0x50 frames).
fn run_step(fx: &mut Fx, t: u32, size: u32, ra: u32, f: impl FnOnce(&mut Fx, u32, u32)) {
    let sp = fx.enter(size);
    let (ra, d) = (u64::from(ra), u64::from(D));
    if size == 0x20 {
        fx.saved(sp, &[(0x10, ra), (0, d)]);
    } else {
        fx.saved(sp, &[(0x20, ra), (0x10, 0), (0, d)]);
    }
    f(fx, t, sp);
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

/// Odd ages `odd`, even ones `even` into the colour +0x28.
fn flicker(fx: &mut Fx, t: u32, odd: (i32, i32), even: (i32, i32)) {
    let (r, g) = if age(fx, t).wrapping_rem(2) == 1 { odd } else { even };
    l2::rgb(fx, t + 0x28, r, g, 0);
}

/// The first update of the flames / smoke: size, +0x10 a rand % 0x4000
/// (less `off`), +0x14 rand % 0xFFFF, frame rand % 7, then +0xE / +8
/// (`spread`) rand % 3 + 4 and +6 = `six`; colour (0x80, 0x40, 0) on
/// parameter 0, else (0, 0x80, 0x80). Rands from `at` (the step's entry).
fn start(fx: &mut Fx, t: u32, at: u32, size: i32, off: i32, spread: u32, six: i32) {
    fx.mem.set16(t + 0x38, size);
    // RISE has one more word before its first rand.
    let b = if at == RISE { at + 4 } else { at };
    let v = fx.rand_from(t, b + 0x2C);
    fx.mem.set32(t + 0x10, v.wrapping_rem(0x4000) - off);
    let v = fx.rand_from(t, b + 0x4C);
    let r = fx.div(v, 0xFFFF).1;
    fx.mem.set32(t + 0x14, r);
    let v = fx.rand_from(t, b + 0x68);
    let r = fx.div(v, 7).1;
    fx.mem.set16(t + 0x34, r);
    fx.mem.set16(t + 0x36, 2);
    let v = fx.rand_from(t, b + 0x8C);
    let r = fx.div(v, 3).1;
    fx.mem.set16(t + spread, r + 4);
    fx.mem.set16(t + 6, six);
    if fx.mem.i16(t + 0x30) == 0 {
        l2::rgb(fx, t + 0x28, 0x80, 0x40, 0);
    } else {
        l2::rgb(fx, t + 0x28, 0, 0x80, 0x80);
    }
}

/// 0x1E029A0 (`RISE`: size 0x46 shrinking by +0xE, fading by 0x13 below
/// 0xC) / 0x1E02BC0 (`GROW`: size 0 growing by +0xE, fading by 0x13 from
/// 0x51): alpha up by 0x50 (below 0xA0) on ages 0 / 1; the task ends below
/// size -0xC; out by 0xF once the timer is under 0xA; orange (0x80, 0x40)
/// on odd ages, else (0xB0, 0x80).
fn flame(fx: &mut Fx, t: u32, at: u32) {
    let rise = at == RISE;
    if first(fx, t) {
        start(fx, t, at, if rise { 0x46 } else { 0 }, 0, 0xE, 1);
    }
    if age(fx, t) < 2 {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0xA0 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x50));
        }
    } else {
        let s = fx.mem.i16(t + 0x38);
        if (rise && s < 0xC) || (!rise && s >= 0x51) {
            fall(fx, t, 0x15, 0x13);
        }
        if fx.mem.i16(t + 0x38) < -0xC {
            fx.mem.set16(t + 0x36, 0);
        }
        if fx.mem.i32(TIMER) < 0xA {
            fall(fx, t, 0x11, 0xF);
        }
    }
    flicker(fx, t, (0x80, 0x40), (0xB0, 0x80));
    grow(fx, t);
    let s = fx.mem.i16(t + 0x38);
    let e = fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x38, if rise { s.wrapping_sub(e) } else { s.wrapping_add(e) });
}

/// 0x1E02DE0: a smoke band (size 0 growing by +8, +0x10 rand % 0x4000 -
/// 0x2000, rim (0x80, 8, 8)): alpha up by 0x20 (below 0x40) on ages 0 / 1,
/// then out by 0x13 from size 0x29 and by 0xF once the timer is under 0xA;
/// (0x30, 0x20) on odd ages, else (0x50, 0x40).
fn smoke(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        start(fx, t, SMOKE, 0, 0x2000, 8, 0xA);
        l2::rgb(fx, t + 0x2C, 0x80, 8, 8);
    }
    if age(fx, t) < 2 {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x40 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x20));
        }
    } else {
        if fx.mem.i16(t + 0x38) >= 0x29 {
            fall(fx, t, 0x15, 0x13);
        }
        if fx.mem.i32(TIMER) < 0xA {
            fall(fx, t, 0x11, 0xF);
        }
    }
    flicker(fx, t, (0x30, 0x20), (0x50, 0x40));
    grow(fx, t);
    let s = fx.mem.i16(t + 0x38).wrapping_add(fx.mem.i16(t + 8));
    fx.mem.set16(t + 0x38, s);
}

/// 0x1E03590 (frame `sp`): a glow (size 0x28, (0x80, 0x80, 0x80) / (0xC0,
/// 0xC0, 0), angle +0x3A from the parameter) circling node 3 at radius 3
/// (sine / cosine of +0x3A on x / z), alpha up by 4 for 0x12 updates then
/// 0x40 / 0x50 on odd / even ages; the angle up by 10 an update; ends at
/// age 0xD3.
fn orbit(fx: &mut Fx, t: u32, sp: u32) {
    let deg = fx.mem.i16(t + 0x3A);
    l2::sin_deg(fx, SINES, deg, 0);
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x28);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x2C, 0xC0, 0xC0, 0);
        let p = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0x3A, p);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    let a = age(fx, t);
    if a < 0x12 {
        let v = fx.mem.read_u8(t + 0x2B);
        fx.mem.write_u8(t + 0x2B, v.wrapping_add(4));
    } else {
        let v = if a.wrapping_rem(2) == 1 { 0x40 } else { 0x50 };
        fx.mem.write_u8(t + 0x2B, v);
    }
    let h = fx.self_handle();
    let n = fx.mem.i16(NODES);
    fx.node_position(h, n, sp + 0x30);
    for (k, ra) in [(0u32, 0x01E0_3684u32), (1, 0x01E0_3690), (2, 0x01E0_369C)] {
        let v = fx.mem.f(sp + 0x30 + 4 * k);
        fx.fix_residue(v, ra);
        fx.mem.set16(t + 2 * k, ee::fixsfsi(v));
    }
    for (o, cos, ra) in [(0u32, 0, 0x01E0_36D4u32), (4, 1, 0x01E0_370C)] {
        let deg = fx.mem.i16(t + 0x3A);
        let s = l2::sin_deg(fx, SINES, deg, cos);
        let m = ee::mul(3.0, s);
        let v = ee::add(ee::from_i32(fx.mem.i16(t + o)), m);
        fx.fix_residue(v, ra);
        fx.mem.set16(t + o, ee::fixsfsi(v));
    }
    grow(fx, t);
    if age(fx, t) >= 0xD3 {
        fx.mem.set16(t + 0x36, 0);
    }
    let v = fx.mem.i16(t + 0x3A) + 10;
    fx.mem.set16(t + 0x3A, v);
    let v = fx.mem.i16(t + 0x3A);
    if v >= 0x168 {
        let r = fx.div(v, 0x168).1;
        fx.mem.set16(t + 0x3A, r);
    }
}

/// 0x1E03780: the bright disc (size 0x12C, (0xF0, 0xF0, 0xF0) / (0xF0,
/// 0xF0, 0x10), angle rand % 360): alpha up by 0x60 on ages 0 / 1, then
/// out by 0xA.
fn disc(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x12C);
        l2::rgb(fx, t + 0x28, 0xF0, 0xF0, 0xF0);
        l2::rgb(fx, t + 0x2C, 0xF0, 0xF0, 0x10);
        let v = fx.rand_from(t, 0x01E0_37D4);
        let r = fx.div(v, 0x168).1;
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if age(fx, t) < 2 {
        let a = fx.mem.read_u8(t + 0x2B);
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x60));
    } else {
        fall(fx, t, 0x11, 0xA);
    }
    grow(fx, t);
}

/// `tstep::spawn` (0x1E04730, frame 0x60) with its saves: ra, the runner's
/// s4 / s3 (0) [H], the emitter's s2 / s1 (= D) / s0.
fn spawn(fx: &mut Fx, f: u32, ra: u32, s0: u64, s2: u64) {
    let sp = fx.sp.wrapping_sub(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, s2), (0x10, u64::from(D)), (0, s0)]);
    tstep::spawn(fx, D, f, 0);
}

/// Node slot `k`'s position (into the emitter's sp + 0x40) copied to the
/// spawn point.
fn at_node(fx: &mut Fx, sp: u32, k: u32) {
    let h = fx.self_handle();
    let n = fx.mem.i16(NODES + 2 * k);
    fx.node_position(h, n, sp + 0x40);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x40 + 4 * k);
        fx.mem.write_u32(POS + 4 * k, w);
    }
}

/// The spawn point (x, y - 10, 40 + z) of the unit point at `p`.
fn at_unit(fx: &mut Fx, p: u32) {
    let x = fx.mem.f(p);
    fx.mem.setf(POS, x);
    let y = fx.mem.f(p + 4);
    fx.mem.setf(POS + 4, ee::sub(y, 10.0));
    let z = fx.mem.f(p + 8);
    fx.mem.setf(POS + 8, ee::add(40.0, z));
}

/// 0x1E020D0(D, 0) (frame 0x50: ra +0x30, s2 +0x20 = the parameter, s1
/// +0x10 = 0, s0 = D): the spawn colour (0x80, 0, 0, 0), the update
/// counter, the spawn points and the spawns.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    let p = param as i64 as u64;
    let d = u64::from(D);
    fx.saved(sp, &[(0x30, 0x01E0_3DA4), (0x20, p), (0x10, 0), (0, d)]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    // The emitter's s0 (loop counter, else the entry's D).
    let mut s0 = d;
    if counter(fx) == 0xA {
        for k in 0..12u64 {
            fx.mem.set16(PARAM, 30 * k as i32);
            spawn(fx, ORBIT, 0x01E0_2148, k, 30 * k);
        }
        s0 = 12;
    }
    if counter(fx) < 4 {
        at_node(fx, sp, 0);
        for k in 0..12u64 {
            spawn(fx, RISE, 0x01E0_21B8, k, p);
        }
        s0 = 12;
    }
    if counter(fx) == 0x82 {
        at_node(fx, sp, 0);
        spawn(fx, DISC, 0x01E0_221C, s0, p);
        for k in 0..12u64 {
            spawn(fx, GROW, 0x01E0_2234, k, p);
        }
        for k in 0..12u64 {
            spawn(fx, SMOKE, 0x01E0_225C, k, p);
        }
        s0 = 12;
    }
    let c = counter(fx);
    if (0x5E..0x82).contains(&c) {
        at_node(fx, sp, 0);
        spawn(fx, FLASH, 0x01E0_22D0, s0, p);
    }
    if counter(fx) == 0x33 {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODES + 2);
        fx.node_position(h, n, KEEP);
    }
    let c = counter(fx);
    if (0x34..0x41).contains(&c) {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODES + 2);
        fx.node_position(h, n, sp + 0x40);
        for k in 0..3 {
            let w = fx.mem.read_u32(KEEP + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
    }
    if counter(fx) == 1 {
        let h = fx.self_handle();
        fx.unit_position(h, D + 0x320);
        let h = fx.self_handle();
        fx.unit_position(h, D + 0x330);
    }
    let c = counter(fx);
    if (0x70..0xC3).contains(&c) {
        let z = fx.mem.f(D + 0x328);
        fx.mem.setf(D + 0x328, ee::sub(z, f32::from_bits(0x4019_999A)));
        at_unit(fx, D + 0x320);
        fx.mem.set16(PARAM, 0);
    }
    if counter(fx) == 0x50 {
        at_node(fx, sp, 2);
        fx.mem.set16(PARAM, 0);
        at_node(fx, sp, 3);
        fx.mem.set16(PARAM, 0);
    }
    let c = counter(fx);
    if (0xAB..0xE1).contains(&c) {
        let z = fx.mem.f(D + 0x338);
        fx.mem.setf(D + 0x338, ee::sub(z, f32::from_bits(0x4073_3333)));
        at_unit(fx, D + 0x330);
        fx.mem.set16(PARAM, 1);
    }
    let c = counter(fx);
    if (0x70..0xBF).contains(&c) {
        at_unit(fx, D + 0x320);
        fx.mem.set16(PARAM, 0);
    }
    let c = counter(fx);
    if (0xA9..0xDD).contains(&c) {
        at_unit(fx, D + 0x330);
        fx.mem.set16(PARAM, 1);
    }
    if counter(fx) == 1 {
        spawn(fx, HOLD, 0x01E0_25A8, s0, p);
    }
    let c = counter(fx);
    if (0x30..=0x33).contains(&c) {
        at_node(fx, sp, 0);
        fx.mem.set16(PARAM, 0);
    }
    let c = counter(fx);
    if (0x33..=0x36).contains(&c) {
        at_node(fx, sp, 1);
        fx.mem.set16(PARAM, 1);
    }
    fx.leave(0x50);
}
