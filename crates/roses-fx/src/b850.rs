//! BMODULE b850 (card 679), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01BF0, data area 0x1ED0000) [F]: b081's family on a pool of 150 tasks of 0x40 bytes at D+0x2F0
//! (alive +0x36, step +0x24, each step called by its own `jal`; spawn
//! 0x1E02280 on colour D+0x2DF4, index D+0x2E34 and the point D+0x250). A
//! 10-point history D+0x28E0 of the attacker's node 3 (filled on update
//! 1, then pushed by `spawn::node_history`). The emitter 0x1E00C20: on
//! update 1 a window flash 0x1E01A60 (b081's, alpha +0x2B in by 0xC below
//! 0x60 until update 0x50); on update 0x8B at node 3 an orange
//! `quads4::fan18_far0` disc 0x1E015B0, twelve `quads4::band9_tex` sliver
//! rings 0x1E00FB0 and twelve `quads4::ring_band_task` bands 0x1E01160;
//! on updates 0x88..0xA9 a white / yellow `fan18_far0` disc 0x1E012F0 at
//! node 3. The steps 0x1E00D90 / 0x1E013C0 / 0x1E01690 / 0x1E01950 and the
//! object op 0x1E01A10 are never spawned (not ported). Returns 1 on update
//! 0x81, 4 on 0xAF, 2 on 0x9B, 6 on 0x8C..0x95, 8 once the 200-update
//! timer D+0x2DF0 has run out.
use crate::blib::{quads4, spawn as bspawn};
use crate::bus::Bus;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0x96;
const POOL: u32 = D + 0x2F0;

const SLIVERS: u32 = 0x01E0_0FB0;
const BAND: u32 = 0x01E0_1160;
const DISC_B: u32 = 0x01E0_12F0;
const DISC_A: u32 = 0x01E0_15B0;
const FLASH: u32 = 0x01E0_1A60;
/// The steps with the entry's return address after each step's `jal`.
const STEPS: [(u32, u32); 5] = [(SLIVERS, 0x01E0_1E30), (BAND, 0x01E0_1E50), (DISC_B, 0x01E0_1E70), (DISC_A, 0x01E0_1EB0), (FLASH, 0x01E0_1F60)];

const POS: u32 = D + 0x250;
const HISTORY: u32 = D + 0x28E0;
const TEX5: u32 = D + 0x2870;
const TIMER: u32 = D + 0x2DF0;
const COLOUR: u32 = D + 0x2DF4;
const NODES: u32 = D + 0x2E1C;
const INDEX: u32 = D + 0x2E34;
const COUNTER: u32 = D + 0x2E38;
const BYTES: u32 = D + 0x2E3C;

/// rand % m (HI) from the `jal` returning to `ra` (rand's frame: s0).
fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

/// The entry 0x1E01BF0 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x150);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x2DFC, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x150);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x30B0);
    l2::sine_table(fx, D + 0x2F3C);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x2E36, param);
    fx.mem.set32(TIMER, 0xC8);
    for (k, (n, clear)) in [(3, 1), (4, 0), (0x1A, 0), (0x6A, 0), (0x40, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    for k in 0xE..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x2E8 + 4 * (k as u32 - 0xE), t);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x2B8, t);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX5 + 4 * k as u32, t);
    }
    // rand's frame: s0 = D.
    for k in 0..0x100u32 {
        let r = rmod(fx, D, 0x01E0_1D34, 0x1E);
        fx.mem.write_u8(BYTES + k, (r - 0xF) as u8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    if counter(fx) == 1 {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODES);
        fx.node_position(h, n, sp + 0xA0);
        for i in 0..9 {
            for k in 0..3 {
                let w = fx.mem.read_u32(sp + 0xA0 + 4 * k);
                fx.mem.write_u32(HISTORY + 0x10 * i + 4 * k, w);
            }
        }
    }
    let n = fx.mem.i16(NODES);
    bspawn::node_history(fx, HISTORY + 0x90, 0xA, n);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x24);
        if let Some(&(_, ra)) = STEPS.iter().find(|s| s.0 == f) {
            run_step(fx, f, t, ra);
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) != 0 {
            draw(fx, sp, t);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0x81 => 1,
        0xAF => 4,
        0x9B => 2,
        0x8C..=0x95 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The texture at `tex` and the atlas cell `id` frame +0x34 (0x106960
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

/// The entry's draw case for a live task (`sp` the entry's frame, the
/// descriptor at sp + 0xB8).
fn draw(fx: &mut Fx, sp: u32, t: u32) {
    let d = sp + 0xB8;
    match fx.mem.read_u32(t + 0x24) {
        SLIVERS => {
            cell(fx, t, TEX5, sp + 0x70, 0x14);
            fx.mem.set16(D + 0xBE, 0);
            fx.mem.set16(D + 0xB8, 1);
            quads4::band9_tex(fx, D, t, d);
            fx.mem.set16(D + 0xB8, 0);
        }
        BAND => {
            cell(fx, t, D + 0x2EC, sp + 0x80, 0x97);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0x94, 0xA0);
            quads4::ring_band_task(fx, 0x01E0_06E0, D, t, d);
        }
        DISC_B => {
            cell(fx, t, D + 0x2EC, sp + 0x90, 0x97);
            fx.mem.set16(D + 0xB8, 4);
            fx.mem.set16(D + 0xB8, 0);
            quads4::fan18_far0(fx, D, t, d);
        }
        DISC_A => quads4::fan18_far0(fx, D, t, d),
        _ => {}
    }
}

/// 0x1E00C20(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the spawns (frame 0x40: ra, s1 = 0 / s0 = D; the spawn's saves:
/// s4 / s3 the runner's (0) [H], s2 = the parameter, s1 = the emitter's
/// loop counter, s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x20, 0x01E0_1D70), (0x10, 0), (0, u64::from(D))]);
    let p = param as i64 as u64;
    let sp_ = |f: &mut Fx, step: u32, s1: u64, ra: u32| {
        spawn(f, step, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, p), (0x10, s1), (0, u64::from(D))]);
    };
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    let mut s1 = 0u64;
    if counter(fx) == 0x8B {
        at_node(fx, sp);
        sp_(fx, DISC_A, 0, 0x01E0_0CA8);
        for k in 0..0xCu64 {
            sp_(fx, SLIVERS, k, 0x01E0_0CC0);
        }
        for k in 0..0xCu64 {
            sp_(fx, BAND, k, 0x01E0_0CE8);
        }
        s1 = 0xC;
    }
    let c = counter(fx);
    if (0x88..0xAA).contains(&c) {
        at_node(fx, sp);
        sp_(fx, DISC_B, s1, 0x01E0_0D58);
    }
    if counter(fx) == 1 {
        sp_(fx, FLASH, s1, 0x01E0_0D7C);
    }
    fx.leave(0x40);
}

/// The spawn point D+0x250 = the attacker's node 3 (through sp + 0x30).
fn at_node(fx: &mut Fx, sp: u32) {
    let h = fx.self_handle();
    let n = fx.mem.i16(NODES);
    fx.node_position(h, n, sp + 0x30);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x30 + 4 * k);
        fx.mem.write_u32(POS + 4 * k, w);
    }
}

/// 0x1E02280(D, step, 0): the first free task cleared and set off at
/// D+0x250 (as i16) with the colour D+0x2DF4 (twice) and +0x30 = D+0x2E34;
/// `saves` the spawn's prologue saves.
fn spawn(fx: &mut Fx, f: u32, saves: &[(u32, u64)]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(POOL + 0x36 + 0x40 * k) == 0) {
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
            let a = fx.mem.i16(INDEX);
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

/// A step's frame and its saves of the entry's s0 = D, s1 = 0.
fn run_step(fx: &mut Fx, f: u32, t: u32, ra: u32) {
    let (ra, d) = (u64::from(ra), u64::from(D));
    let size = match f {
        SLIVERS => 0x30,
        FLASH => 0x90,
        _ => 0x20,
    };
    let sp = fx.enter(size);
    if size == 0x20 {
        fx.saved(sp, &[(0x10, ra), (0, d)]);
    } else {
        fx.saved(sp, &[(0x20, ra), (0x10, 0), (0, d)]);
    }
    match f {
        SLIVERS => slivers(fx, t),
        BAND => band(fx, t),
        DISC_B => disc_b(fx, t),
        DISC_A => disc_a(fx, t),
        _ => flash(fx, sp, t),
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

fn kill(fx: &mut Fx, t: u32) {
    fx.mem.set16(t + 0x36, 0);
}

/// The alpha byte `at` down by `down` while at least `floor`, else the
/// task's end (`end`) or the byte 0.
fn fall(fx: &mut Fx, t: u32, at: u32, floor: u8, down: u8, end: bool) {
    let a = fx.mem.read_u8(t + at);
    if a >= floor {
        fx.mem.write_u8(t + at, a - down);
    } else if end {
        kill(fx, t);
    } else {
        fx.mem.write_u8(t + at, 0);
    }
}

/// 0x1E00FB0: a white sliver ring (random angles, cell 0..6, radius 0
/// growing by 2..4, +6 1): alpha in by 0x50 below 0xA0 for 2 updates;
/// out by 0x13 from radius 0x47, ended below radius -12, out by 0xF once
/// the timer is under 10.
fn slivers(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0);
        for (o, ra) in [(0x10u32, 0x01E0_0FDCu32), (0x14, 0x01E0_0FF8), (0x18, 0x01E0_1014)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        let v = rmod(fx, t, 0x01E0_1030, 7);
        fx.mem.set16(t + 0x34, v);
        fx.mem.set16(t + 0x36, 2);
        let v = rmod(fx, t, 0x01E0_1054, 3);
        fx.mem.set16(t + 0xE, v + 2);
        fx.mem.set16(t + 6, 1);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
    }
    if age(fx, t) < 2 {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0xA0 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x50));
        }
    } else {
        if fx.mem.i16(t + 0x38) >= 0x47 {
            fall(fx, t, 0x2B, 0x15, 0x13, true);
        }
        if fx.mem.i16(t + 0x38) < -0xC {
            kill(fx, t);
        }
        if fx.mem.i32(TIMER) < 0xA {
            fall(fx, t, 0x2B, 0x11, 0xF, true);
        }
    }
    grow(fx, t);
    let v = fx.mem.i16(t + 0x38) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x38, v);
}

/// 0x1E01160: a `ring_band_task` band (grey-blue (0x30, 0x30, 0xA0) both
/// edges, random angles, radius 10 growing by 2..3, +6 15): both alphas
/// 0x80 on its first update, then out by 7.
fn band(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0xA);
        for (o, ra) in [(0x10u32, 0x01E0_1188u32), (0x18, 0x01E0_11A4), (0x14, 0x01E0_11C0)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x36, 2);
        let v = fx.rand_from(t, 0x01E0_11E8) % 2;
        fx.mem.set16(t + 8, v + 2);
        fx.mem.set16(t + 6, 0xF);
        l2::rgb(fx, t + 0x28, 0x30, 0x30, 0xA0);
        l2::rgb(fx, t + 0x2C, 0x30, 0x30, 0xA0);
    }
    if age(fx, t) <= 0 {
        for at in [0x2Bu32, 0x2F] {
            if fx.mem.read_u8(t + at) < 0x80 {
                fx.mem.write_u8(t + at, 0x80);
            }
        }
    } else {
        fall(fx, t, 0x2B, 9, 7, true);
        fall(fx, t, 0x2F, 9, 7, false);
    }
    if age(fx, t) != 0 {
        let v = fx.mem.i16(t + 0x38) + fx.mem.i16(t + 8);
        fx.mem.set16(t + 0x38, v);
    }
    grow(fx, t);
}

/// 0x1E012F0: a disc (white centre, yellow (0xC0, 0xC0, 0) rim, radius 60
/// fixed (it steps by -2 only once negative), angle the slot +0x30): +0x2B
/// up by 0x50 on its first update, then out by 9.
fn disc_b(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x3C);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x2C, 0xC0, 0xC0, 0);
        let s = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0x3A, s);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if age(fx, t) <= 0 {
        let a = fx.mem.read_u8(t + 0x2B);
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x50));
    } else {
        fall(fx, t, 0x2B, 0xB, 9, true);
    }
    grow(fx, t);
    // bgez skips the step: the radius moves only once negative.
    let r = fx.mem.i16(t + 0x38);
    if r < 0 {
        fx.mem.set16(t + 0x38, r - 2);
    }
}

/// 0x1E015B0: a disc (light (0xF0, 0xF0, 0xF0) centre, (0x40, 0x40, 0xF0)
/// rim, radius 300, a random angle): +0x2B up by 0x60 for 2 updates, then
/// out by 0xF.
fn disc_a(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x12C);
        l2::rgb(fx, t + 0x28, 0xF0, 0xF0, 0xF0);
        l2::rgb(fx, t + 0x2C, 0x40, 0x40, 0xF0);
        let v = rmod(fx, t, 0x01E0_1604, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if age(fx, t) < 2 {
        let a = fx.mem.read_u8(t + 0x2B);
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x60));
    } else {
        fall(fx, t, 0x2B, 0x11, 0xF, true);
    }
    grow(fx, t);
}

/// 0x1E01A60 (frame `sp`): b081's window flash with its descriptor at sp
/// + 0x38 (a 0x90 frame); in by 0xC below 0x60 until update 0x50, then
/// out by 9.
fn flash(fx: &mut Fx, sp: u32, t: u32) {
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for k in 0..3 {
        fx.mem.write_u8(d + 0x10 + k, 0x80);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    fx.mem.write_u8(d + 0x13, a);
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    for (at, v) in [(d + 0x20, 0x6FF8u16), (d + 0x30, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    for (o, v) in [(0x18u32, 0u32), (0x1C, 0), (0x28, 0x3F80_0000), (0x2C, 0x3F80_0000)] {
        fx.mem.write_u32(d + o, v);
    }
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if counter(fx) < 0x50 {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x60 {
            fx.mem.write_u8(t + 0x2B, a + 0xC);
        }
    } else {
        fall(fx, t, 0x2B, 0xB, 9, true);
    }
    grow(fx, t);
}
