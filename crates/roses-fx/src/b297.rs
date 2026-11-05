//! BMODULE b297 (card 28), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02650, data area 0x1ED0000) [F] (helper A10, 31
//! Oct 2026): b003's older task pool (200 tasks of 0x40 bytes at +0x2A0,
//! alive +0x36, step +0x24, each step called by its own `jal` with (D, t))
//! drawn as rings at the task (`quads4::ring_at_mt`, 0x1E00920; UVs by
//! uv_rect 0x1E00080). The emitter 0x1E00EA0 (`emit`): on updates
//! 0x15..0x4B a grey smoke ring a update at the attacker's node 8, 2 up
//! (0x1E02010, pushed 10 toward the facing); on 0x51..0x137 two more there
//! (0x1E011F0 light grey, 0x1E014F0 lighter, both drifting toward the
//! target by a 40th of the gap in z); on 0x79..0x13F two rings a update
//! round the target's node 1 (0x1E01B70 grey, 0x1E01DC0 dark:
//! `spawn::ring_rise`). Returns 1 on update 0x5A, 3 on 0x15E, 6 on
//! 0x65..0x158, 8 once the 382-update timer +0x34F0 has run out.
//!
//! The steps 0x1E01110 (e1f99f393722, `spawn::flare_step`), 0x1E017B0,
//! 0x1E022F0 (31f1772573ce), 0x1E023D0 (a camera slot through 0x1F7E20),
//! 0x1E02540 (the stage light) and 0x1E02600 (an object op through
//! 0x1F8380, inlined in the entry) are tested by the entry but never
//! spawned [F: the spawn `jal`s at 0x1E00F3C..0x1E010F4 name 0x1E02010 /
//! 0x1E011F0 / 0x1E014F0 / 0x1E01B70 / 0x1E01DC0 only]; not ported.
use crate::blib::{quads4, spawn, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 200;
/// The steps a task holds (+0x24), as the record's addresses.
const LIGHT: u32 = 0x01E0_11F0;
const LIGHTER: u32 = 0x01E0_14F0;
const GREY: u32 = 0x01E0_1B70;
const DARK: u32 = 0x01E0_1DC0;
const SMOKE: u32 = 0x01E0_2010;
const TIMER: u32 = D + 0x34F0;
const COLOUR: u32 = D + 0x34F4;
const NODES: u32 = D + 0x3514;
const INDEX: u32 = D + 0x352E;
const FACING: u32 = D + 0x3530;
const COUNTER: u32 = D + 0x3534;
const BYTES: u32 = D + 0x3536;
const BYTES2: u32 = D + 0x3636;

/// The entry 0x1E02650.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x100);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x34FC, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x100);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x38B0);
    l2::sine_table(fx, D + 0x3738);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x3532, param);
    fx.mem.set32(TIMER, 0x17E);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xE0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xE4)), 180.0);
    fx.fix_residue(v, 0x01E0_26E8);
    fx.mem.set16(FACING, ee::fixsfsi(v));
    for (k, (target, n, clear)) in [(false, 0xB, 1), (false, 8, 0), (true, 1, 1), (false, 0x47, 0), (false, 0x4E, 0)].into_iter().enumerate() {
        let h = if target { fx.target_handle() } else { fx.self_handle() };
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    for id in 0xAu32..0x10 {
        let tex = fx.texture(4, id as i32);
        fx.mem.write_u32(D + 0x288 + 4 * (id - 0xA), tex);
    }
    let tex = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x268, tex);
    let tex = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, tex);
    for k in 0..4u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x34A0 + 4 * k, tex);
    }
    // rand's frames: s0 = the loop counter.
    for k in 0..0x100u32 {
        let r = fx.rand_from(k, 0x01E0_27E0);
        let v = fx.div(r, 0x1E).1;
        fx.mem.write_u8(BYTES + k, (v - 0xF) as u8);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(k, 0x01E0_2814);
        let v = fx.div(r, 0x1E).1;
        fx.mem.write_u8(BYTES2 + k, (v - 0xF) as u8);
    }
    for k in 0x64..0xB4u32 {
        let r = fx.rand_from(k, 0x01E0_2848);
        let v = fx.div(r, 0xF).1;
        fx.mem.write_u8(BYTES2 + k, v.wrapping_neg() as u8);
    }
    for k in 0xB4..0x100u32 {
        fx.mem.write_u8(BYTES2 + k, 0);
    }
    for k in 0..8u32 {
        let r = fx.rand_from(k, 0x01E0_2898);
        fx.mem.set16(D + 0xC0 + 2 * k, r.wrapping_rem(4));
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        // The steps' saves: the entry's s0 = 0, s1 = D, s2 = D + 0x40 k.
        let f = fx.mem.read_u32(t + 0x24);
        match f {
            LIGHT => light(fx, t, 0x01E0_2928, true),
            LIGHTER => light(fx, t, 0x01E0_2948, false),
            GREY | DARK => {
                let ra = if f == GREY { 0x01E0_2988 } else { 0x01E0_29A8 };
                fx.saved(fx.sp - 0x20, &[(0x10, ra), (0, 0)]);
                spawn::ring_rise(fx, f, t);
            }
            SMOKE => smoke(fx, t, D + 0x40 * k),
            _ => debug_assert!(false, "unspawned step {f:#x}"),
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    // s0 counts the live tasks of the draw loop (rand's saved s0).
    let mut live = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        live += 1;
        let f = fx.mem.read_u32(t + 0x24);
        if f == LIGHT {
            tex(fx, 0x34A8);
            cell(fx, t, sp + 0x60);
            for (o, v) in [(0x94u32, 1), (0x96, 1), (0x98, 0x3F), (0x9A, 0x3F)] {
                fx.mem.set16(D + o, v);
            }
            let rows = fx.mem.i16(t + 0xC);
            ring(fx, t, live, rows, 0x01E0_2B28, 0x20, BYTES);
            tex(fx, 0x260);
            cell(fx, t, sp + 0x70);
            if fx.mem.i16(t + 0x32) >= 4 {
                ring(fx, t, live, 3, 0x01E0_2BB0, 0x20, BYTES);
            }
        }
        if f == LIGHTER {
            tex(fx, 0x260);
            cell(fx, t, sp + 0x80);
            ring(fx, t, live, 4, 0x01E0_2C48, 0x20, BYTES);
        }
        if f == GREY {
            tex(fx, 0x34AC);
            cell(fx, t, sp + 0xA0);
            let rows = fx.mem.i16(t + 0xC);
            ring(fx, t, live, rows, 0x01E0_2D80, 0x10, BYTES);
        }
        if f == DARK {
            tex(fx, 0x34A4);
            let u = (fx.mem.i16(t + 0x34) << 5) + 1;
            for (o, v) in [(0x94u32, u), (0x96, 1), (0x98, 0x1F), (0x9A, 0x3D)] {
                fx.mem.set16(D + o, v);
            }
            let rows = fx.mem.i16(t + 0xC);
            ring(fx, t, live, rows, 0x01E0_2E00, 0x20, BYTES);
        }
        if f == SMOKE {
            tex(fx, 0x34A0);
            cell(fx, t, sp + 0xB0);
            if fx.mem.i16(t + 0x32) >= 4 {
                ring(fx, t, live, 3, 0x01E0_2EA0, 0x10, BYTES2);
            }
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x5A => 1,
        0x15E => 3,
        0x65..=0x158 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// D+0x88 = the texture at D + `o`.
fn tex(fx: &mut Fx, o: u32) {
    let w = fx.mem.read_u32(D + o);
    fx.mem.write_u32(D + 0x88, w);
}

/// The atlas cell 0x14 frame +0x34 (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32) {
    let frame = fx.mem.i16(t + 0x34);
    l2::rect(fx, r, 0x14, frame);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// One ring at the task: D+0xB8 = `rows`, D+0xBE = rand % 4 (rand from
/// the draw loop, s0 = the live count), then D+0xB8 back to 0.
fn ring(fx: &mut Fx, t: u32, live: u32, rows: i32, ra: u32, mode: i32, table: u32) {
    fx.mem.set16(D + 0xB8, rows);
    let r = fx.rand_from(live, ra);
    fx.mem.set16(D + 0xBE, r.wrapping_rem(4));
    quads4::ring_at_mt(fx, uv::uv_rect, D, t, mode, table);
    fx.mem.set16(D + 0xB8, 0);
}

/// 0x1E00EA0(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// +0x3534 and the emitters (frame 0x40: s0 +0, s1 +0x10, ra +0x20).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x20, 0x01E0_28D4), (0x10, u64::from(D)), (0, 0)]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    // The spawn's saves: s0 = D, s1 = the emitter's loop register, the
    // entry's s2 = the parameter (64 bits), s3 / s4 the runner's (0).
    let regs = |s1: u64| [(0x40u32, 0u64), (0x30, 0), (0x20, param as i64 as u64), (0x10, s1), (0, u64::from(D))];
    let c = fx.mem.i16(COUNTER);
    if (0x15..0x4C).contains(&c) {
        site(fx, sp);
        spawn(fx, SMOKE, &regs(0), 0x01E0_0F44);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x51..0x138).contains(&c) {
        site(fx, sp);
        spawn(fx, LIGHT, &regs(0), 0x01E0_0FC0);
        spawn(fx, LIGHTER, &regs(0), 0x01E0_0FE4);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x79..0x140).contains(&c) {
        let h = fx.target_handle();
        let s = fx.mem.i16(NODES + 4);
        fx.node_position(h, s, sp + 0x30);
        for (o, ra, m, off) in [(0u32, 0x01E0_102Cu32, 0x14, 10.0f32), (4, 0x01E0_1060, 0x28, 20.0), (8, 0x01E0_1094, 0x14, 10.0)] {
            let r = fx.rand_from(D, ra);
            let (_, v) = fx.div(r, m);
            let p = ee::sub(ee::add(fx.mem.f(sp + 0x30 + o), ee::from_i32(v)), off);
            fx.mem.setf(D + 0x250 + o, p);
        }
        spawn(fx, GREY, &regs(0), 0x01E0_10DC);
        spawn(fx, DARK, &regs(1), 0x01E0_10FC);
    }
    fx.leave(0x40);
}

/// The attacker's node 8 (slot +0x3516) into D+0x250, y + 2.
fn site(fx: &mut Fx, sp: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i16(NODES + 2);
    fx.node_position(h, s, sp + 0x30);
    let x = fx.mem.read_u32(sp + 0x30);
    fx.mem.write_u32(D + 0x250, x);
    let y = ee::add(2.0, fx.mem.f(sp + 0x34));
    fx.mem.setf(D + 0x254, y);
    let z = fx.mem.read_u32(sp + 0x38);
    fx.mem.write_u32(D + 0x258, z);
}

/// 0x1E03030(D, step, 0): the first free task cleared and set off at
/// D+0x250 (as i16) with the colour +0x34F4 (twice), +0x30 = +0x352E;
/// `saves` the caller's s4..s0, `ra` the return address.
fn spawn(fx: &mut Fx, f: u32, saves: &[(u32, u64)], ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
    fx.saved(sp, &[(0x50, u64::from(ra))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x2D6 + 0x40 * k) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x40 * k;
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
            fx.mem.write_u32(t + 0x24, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// rand % m (HI) from a step's rand returning to `ra` (s0 = the task).
fn rand_mod(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v, m).1
}

fn odd(v: i32) -> bool {
    v.wrapping_rem(2) == 1
}

/// The attacker's nodes 0xB and 8 (slots +0x3514 / +0x3516) into
/// `sp + 0x30` / `sp + 0x40` (`a` / `b`): +0xC = the x gap as an integer.
fn x_gap(fx: &mut Fx, t: u32, a: u32, b: u32, ra: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i16(NODES);
    fx.node_position(h, s, a);
    let h = fx.self_handle();
    let s = fx.mem.i16(NODES + 2);
    fx.node_position(h, s, b);
    let v = ee::sub(fx.mem.f(b), fx.mem.f(a));
    fx.fix_residue(v, ra);
    fx.mem.set16(t + 0xC, ee::fixsfsi(v));
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

/// 0x1E011F0 (`light` true) / 0x1E014F0 (false)(D, t): a smoke ring
/// drifting toward the target (frame 0x60: the entry's s0 = 0, s1 = D):
/// size 0x2D / 0x32, grey 0x50 / 0x70, radius 0 growing by +0xE 5..9, row
/// +8 0..34 / 0..24, life +6 30..54 / 30..39, +0xC (0x1E011F0 first 3..5)
/// the attacker's x gap; fades in by 0x12 below 0x81 / 0x20 below 0x40,
/// out by 9; z closes on the target by a 40th of the gap; the size grows
/// by 2 below 90 then by 2 (0x1E014F0: by 1).
fn light(fx: &mut Fx, t: u32, ra: u32, light: bool) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x20, u64::from(ra)), (0x10, u64::from(D)), (0, 0)]);
    let base = if light { 0x01E0_11F0u32 } else { 0x01E0_14F0 };
    let at = |o: u32| base + o;
    let (size, grey, row, life, up, cap) = if light { (0x2D, 0x50, 0x23, 0x19, 0x12u8, 0x81u8) } else { (0x32, 0x70, 0x19, 0xA, 0x20, 0x40) };
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, size);
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, grey, grey, grey);
        fx.mem.set16(t + 0x30, 0);
        let v = rand_mod(fx, t, at(0x4C), 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rand_mod(fx, t, at(0x68), 5);
        fx.mem.set16(t + 0xE, v + 5);
        let v = rand_mod(fx, t, at(0x88), row);
        fx.mem.set16(t + 8, v);
        let mut life_at = 0xA4;
        if light {
            let v = rand_mod(fx, t, at(0xA4), 3);
            fx.mem.set16(t + 0xC, v + 3);
            life_at = 0xC4;
        }
        let v = rand_mod(fx, t, at(life_at), life);
        fx.mem.set16(t + 6, v + 0x1E);
        x_gap(fx, t, sp + 0x30, sp + 0x40, at(if light { 0x120 } else { 0x100 }));
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < cap {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(up));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x2B, a - 9);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    if light && fx.mem.i16(t + 0x32) != 0 && fx.mem.i16(t + 0x38) < 0x5A {
        let s = fx.mem.i16(t + 0x38) + 2;
        fx.mem.set16(t + 0x38, s);
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
    fx.fix_residue(z, at(if light { 0x210 } else { 0x1CC }));
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
        let s = fx.mem.i16(t + 0x38) + if light { 2 } else { 1 };
        fx.mem.set16(t + 0x38, s);
    }
    angles(fx, t, at(if light { 0x28C } else { 0x248 }));
    fx.leave(0x60);
}

/// 0x1E02010(D, t): a grey smoke ring at the attacker (frame 0x70: the
/// entry's s0 = 0, s1 = D, s2 = D + 0x40 k): size 0x2D, grey 0x50, radius
/// 0 growing by +0xE 15..19, row +8 0..34, quads +0xC first 3..5 then the
/// attacker's x gap, life +6 10..24; z 10 toward the facing, y down 5;
/// fades in by 0x12, out by 9.
fn smoke(fx: &mut Fx, t: u32, s2: u32) {
    let sp = fx.enter(0x70);
    fx.saved(sp, &[(0x30, 0x01E0_29C8), (0x20, u64::from(s2)), (0x10, u64::from(D)), (0, 0)]);
    let at = |o: u32| 0x01E0_2010 + o;
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x2D);
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, 0x50, 0x50, 0x50);
        fx.mem.set16(t + 0x30, 0);
        let v = rand_mod(fx, t, at(0x50), 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rand_mod(fx, t, at(0x6C), 5);
        fx.mem.set16(t + 0xE, v + 0xF);
        let v = rand_mod(fx, t, at(0x8C), 0x23);
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, t, at(0xA8), 3);
        fx.mem.set16(t + 0xC, v + 3);
        let v = rand_mod(fx, t, at(0xC8), 0xF);
        fx.mem.set16(t + 6, v + 0xA);
        x_gap(fx, t, sp + 0x40, sp + 0x50, at(0x124));
        let z = fx.mem.i16(t + 4);
        let z = if fx.mem.i16(FACING) != 0 { z - 0xA } else { z + 0xA };
        fx.mem.set16(t + 4, z);
        let y = fx.mem.i16(t + 2) - 5;
        fx.mem.set16(t + 2, y);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < 0x81 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x12));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x2B, a - 9);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    if fx.mem.i16(t + 0x32) != 0 && fx.mem.i16(t + 0x38) < 0x5A {
        let s = fx.mem.i16(t + 0x38) + 2;
        fx.mem.set16(t + 0x38, s);
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
        let s = fx.mem.i16(t + 0x38) + 2;
        fx.mem.set16(t + 0x38, s);
    }
    angles(fx, t, at(0x250));
    fx.leave(0x70);
}
