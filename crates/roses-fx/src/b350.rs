//! BMODULE b350 (card 121), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01660, data area 0x1ED0000) [F]: b566's layout (200 tasks of 0x40 bytes at D+0x2F0, alive +0x36,
//! step +0x24, each step called by its own `jal` with (D, t); spawn
//! 0x1E01C70 = b284's with the index D+0x379E; the node history
//! a72bfb6bb70a at 0x1E00080 for node 0x13) with b297's smoke steps, drawn
//! by `quads4::ring_facing_i` (0x1E005A0; UVs by uv_rect4 0x1E00150). The
//! emitter 0x1E00B20: on updates 45..199 a smoke ring and a dark one at
//! the attacker's node 0x13 (2 up) drifting toward the target (0x1E00C60 /
//! 0x1E00F20, b297's 0x1E014F0 with other sizes, greys and fades); on
//! 1..19 a grey ring at node 0xB (0x1E011E0). Returns 1 on update 70, 3 on
//! 250, 6 on 81..244, 8 once the 260-update timer D+0x3750 has run out.
//!
//! The steps 0x1E013E0 (a camera slot), 0x1E01550 (the stage light) and
//! the entry's case 0x1E01610 (an object op through 0x1F8380) are never
//! spawned [F: the emitter's `jal`s name 0x1E00C60 / 0x1E00F20 / 0x1E011E0
//! only]; not ported.
use crate::blib::{quads4, spawn as lib, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 200;
/// The step functions a task holds (+0x24), as the record's addresses.
const SMOKE: u32 = 0x01E0_0C60;
const DARK: u32 = 0x01E0_0F20;
const GREY: u32 = 0x01E0_11E0;
const TEX: u32 = D + 0x34F0;
const TIMER: u32 = D + 0x3750;
const COLOUR: u32 = D + 0x3754;
const NODES: u32 = D + 0x3784;
const INDEX: u32 = D + 0x379E;
const COUNTER: u32 = D + 0x37A2;
const BYTES: u32 = D + 0x37A6;
const BYTES_B: u32 = D + 0x38A6;

/// The entry 0x1E01660 (s0 = 0, s1 = D, s2 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x375C, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3B20);
    l2::sine_table(fx, D + 0x39A8);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x37A0, param);
    fx.mem.set32(TIMER, 0x104);
    for (k, node) in [0x13, 0xB, 0x40, 0x47, 0x4E].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    for k in 0xA..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x2D8 + 4 * (k as u32 - 0xA), t);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x2B8, t);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    for k in 0..0x100u32 {
        let v = fx.rand_from(k, 0x01E0_17A4);
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0..0x100u32 {
        let v = fx.rand_from(k, 0x01E0_17D8);
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES_B + k, h - 0xF);
    }
    for k in 0x64..0xB4u32 {
        fx.mem.write_u8(BYTES_B + k, 0);
    }
    for k in 0..8u32 {
        let v = fx.rand_from(k, 0x01E0_182C);
        fx.mem.set16(D + 0xC0 + 2 * k, v % 4);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    if fx.mem.i16(COUNTER) == 1 {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        fx.node_position(h, slot, sp + 0x90);
        for k in 0..9u32 {
            for c in 0..3 {
                let w = fx.mem.read_u32(sp + 0x90 + 4 * c);
                fx.mem.write_u32(D + 0x3560 + 0x10 * k + 4 * c, w);
            }
        }
    }
    let slot = fx.mem.i16(NODES);
    lib::node_history(fx, D + 0x35F0, 0xA, slot);
    for k in 0..TASKS {
        let t = D + 0x2F0 + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        // The steps' saves: the entry's s0 = 0, s1 = D.
        let f = fx.mem.read_u32(t + 0x24);
        match f {
            SMOKE => smoke(fx, t, &SMOKE_A, 0x01E0_1904),
            DARK => smoke(fx, t, &SMOKE_DARK, 0x01E0_1928),
            GREY => grey(fx, t),
            _ => debug_assert!(false, "b350: unspawned step {f:#x}"),
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    // 0x1F71A0's frame below the entry's (its saved ra, the entry's s0).
    fx.saved(sp - 0x20, &[(0x10, 0x01E0_19FC), (0, 0)]);
    // The entry's s0: the live tasks counted in the draw loop.
    let mut s0 = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2F0 + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        s0 += 1;
        let f = fx.mem.read_u32(t + 0x24);
        for (step, tex, r, rows, ra, mode, table) in [(SMOKE, 0u32, 0x60u32, 7, 0x01E0_1A70u32, 0, BYTES), (DARK, 4, 0x70, 4, 0x01E0_1B00, 0, BYTES), (GREY, 0, 0x80, 6, 0x01E0_1B90, 2, BYTES_B)] {
            if f != step {
                continue;
            }
            let w = fx.mem.read_u32(TEX + tex);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x34);
            l2::rect(fx, sp + r, 0x14, sub);
            for k in 0..4 {
                let v = fx.mem.read_u32(sp + r + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, v);
            }
            fx.mem.set16(D + 0xB8, rows);
            let v = fx.rand_from(s0, ra);
            fx.mem.set16(D + 0xBE, v % 4);
            quads4::ring_facing_i(fx, uv::uv_rect4, D, t, mode, table);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x46 => 1,
        0xFA => 3,
        0x51..=0xF4 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E01C70(D, step, 0): the first free task cleared and set off at
/// D+0x250 (as i16) with the colour D+0x3754 (twice), +0x30 = D+0x379E
/// (b284's spawn); `saves` the caller's s0..s4, `ra` the return address.
fn spawn(fx: &mut Fx, step: u32, saves: &[(u32, u64)], ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
    fx.saved(sp, &[(0x50, u64::from(ra))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x326 + 0x40 * k) == 0) {
        Some(k) => {
            let t = D + 0x2F0 + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            for c in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            let a = fx.mem.i16(INDEX);
            fx.mem.set16(t + 0x30, a);
            fx.mem.write_u32(t + 0x20, 0);
            fx.mem.write_u32(t + 0x24, step);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// 0x1E00B20(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// D+0x37A2 and the emitters (frame 0x40: ra, the entry's s1 = D, s0 = 0).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x20, 0x01E0_1868), (0x10, u64::from(D)), (0, 0)]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    // The spawn's saves: s0 = D, s1 = the emitter's loop register (else
    // the entry's D), the entry's s2 = the parameter, s3 / s4 the
    // runner's (0 [H]).
    let regs = |s1: u64| [(0x40u32, 0u64), (0x30, 0), (0x20, param as i64 as u64), (0x10, s1), (0, u64::from(D))];
    let node = |fx: &mut Fx, sp: u32, k: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES + 2 * k);
        fx.node_position(h, s, sp + 0x30);
    };
    let c = fx.mem.i16(COUNTER);
    if (0x2D..0xC8).contains(&c) {
        node(fx, sp, 0);
        let x = fx.mem.read_u32(sp + 0x30);
        fx.mem.write_u32(D + 0x250, x);
        let y = ee::add(2.0, fx.mem.f(sp + 0x34));
        fx.mem.setf(D + 0x254, y);
        let z = fx.mem.read_u32(sp + 0x38);
        fx.mem.write_u32(D + 0x258, z);
        spawn(fx, SMOKE, &regs(0), 0x01E0_0BC0);
        spawn(fx, DARK, &regs(0), 0x01E0_0BE4);
    }
    if fx.mem.i16(COUNTER) < 0x14 {
        node(fx, sp, 1);
        for o in [0u32, 4, 8] {
            let w = fx.mem.read_u32(sp + 0x30 + o);
            fx.mem.write_u32(D + 0x250 + o, w);
        }
        spawn(fx, GREY, &regs(u64::from(D)), 0x01E0_0C44);
    }
    fx.leave(0x40);
}

/// rand % m (HI) from a step's rand returning to `ra` (s0 = the task).
fn rand_mod(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v, m).1
}

fn odd(v: i32) -> bool {
    v.wrapping_rem(2) == 1
}

/// The three angles +0x10 / +0x14 / +0x18 random, the age +0x32 up.
fn angles(fx: &mut Fx, t: u32, at: u32) {
    for (o, ra) in [(0x10u32, at), (0x14, at + 0x1C), (0x18, at + 0x38)] {
        let v = rand_mod(fx, t, ra, 0x168);
        fx.mem.set32(t + o, v);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// A b297 0x1E014F0 smoke ring's constants.
struct SmokeCfg {
    at: u32,
    size: i32,
    grey: i32,
    cap: u8,
    up: u8,
    grow: i32,
}

const SMOKE_A: SmokeCfg = SmokeCfg { at: SMOKE, size: 0x1E, grey: 0x70, cap: 0x61, up: 0x30, grow: 2 };
const SMOKE_DARK: SmokeCfg = SmokeCfg { at: DARK, size: 0x32, grey: 0x20, cap: 0x40, up: 0x20, grow: 1 };

/// 0x1E00C60 / 0x1E00F20 (D, t): b297's 0x1E014F0 smoke ring drifting
/// toward the target (frame 0x60: the entry's s0 = 0, s1 = D): size
/// `size`, grey `grey`, radius 0 growing by +0xE 5..9, row +8 0..24, life
/// +6 30..39, +0xC the attacker's x gap (nodes 0x13 / 0xB); fades in by
/// `up` below `cap`, out by 9; z closes on the target by a 40th of the gap;
/// the size grows by `grow`.
fn smoke(fx: &mut Fx, t: u32, c: &SmokeCfg, ra: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x20, u64::from(ra)), (0x10, u64::from(D)), (0, 0)]);
    let at = |o: u32| c.at + o;
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, c.size);
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, c.grey, c.grey, c.grey);
        fx.mem.set16(t + 0x30, 0);
        let v = rand_mod(fx, t, at(0x4C), 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rand_mod(fx, t, at(0x68), 5);
        fx.mem.set16(t + 0xE, v + 5);
        let v = rand_mod(fx, t, at(0x88), 0x19);
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, t, at(0xA4), 0xA);
        fx.mem.set16(t + 6, v + 0x1E);
        // The attacker's nodes 0x13 and 0xB: +0xC = the x gap.
        let (a, b) = (sp + 0x30, sp + 0x40);
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES);
        fx.node_position(h, s, a);
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES + 2);
        fx.node_position(h, s, b);
        let v = ee::sub(fx.mem.f(b), fx.mem.f(a));
        fx.fix_residue(v, at(0x100));
        fx.mem.set16(t + 0xC, ee::fixsfsi(v));
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < c.cap {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(c.up));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x2B, a - 9);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x30);
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x40);
    let d = ee::sub(fx.mem.f(sp + 0x38), fx.mem.f(sp + 0x48));
    fx.mem.setf(sp + 0x38, d);
    let d = ee::div(d, 40.0);
    fx.mem.setf(sp + 0x38, d);
    let z = ee::add(ee::from_i32(fx.mem.i16(t + 4)), d);
    fx.fix_residue(z, at(0x1CC));
    fx.mem.set16(t + 4, ee::fixsfsi(z));
    if odd(fx.mem.i16(t + 0x32)) && fx.mem.i16(t + 0x34) < 5 {
        let f = fx.mem.i16(t + 0x34) + 1;
        fx.mem.set16(t + 0x34, f);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let s = fx.mem.i16(t + 0x38) + c.grow;
        fx.mem.set16(t + 0x38, s);
    }
    angles(fx, t, at(0x248));
    fx.leave(0x60);
}

/// 0x1E011E0 (D, t) (frame 0x20: ra, the entry's s0 = 0): a grey ring
/// (size 30 growing by 1, (0x70, 0x70, 0x70), radius 0 growing by 5..9,
/// row 0..24, life 4..13); in by 0x30 below 0x61, out by 9; the cell frame
/// counting to 5 on odd ages; random angles.
fn grey(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, 0x01E0_1948), (0, 0)]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x1E);
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, 0x70, 0x70, 0x70);
        fx.mem.set16(t + 0x30, 0);
        let v = rand_mod(fx, t, 0x01E0_1224, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rand_mod(fx, t, 0x01E0_1240, 5);
        fx.mem.set16(t + 0xE, v + 5);
        let v = rand_mod(fx, t, 0x01E0_1260, 0x19);
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, t, 0x01E0_127C, 0xA);
        fx.mem.set16(t + 6, v + 4);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < 0x61 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x30));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x2B, a - 9);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    if odd(fx.mem.i16(t + 0x32)) && fx.mem.i16(t + 0x34) < 5 {
        let f = fx.mem.i16(t + 0x34) + 1;
        fx.mem.set16(t + 0x34, f);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let s = fx.mem.i16(t + 0x38) + 1;
        fx.mem.set16(t + 0x38, s);
    }
    angles(fx, t, 0x01E0_1368);
    fx.leave(0x20);
}
