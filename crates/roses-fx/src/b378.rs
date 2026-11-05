//! BMODULE b378 (card 340), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01D00, data area 0x1ED0000, 0x3D70 bytes) [F]
//!: card 72's task pool ([`crate::b267`]) with
//! other steps: 220 tasks of 0x40 bytes at D+0x2D4 (alive +0x36, age +0x32,
//! step +0x24; the entry calls each step by its address), spawned by
//! 0x1E02310 (the first free task cleared, colours D+0x3BA4 at +0x28 /
//! +0x2C, the point D+0x80 as i16 at +0, D+0x3BFA at +0x30). The emitter
//! 0x1E00260 starts two lines (0x1E00970) on 0x3C at nodes 0x24 / 0x2B, puffs
//! (0x1E01530) at those nodes on 0x3C..0x44 and at the lines' last points
//! (0x1E01910, 30 each) on 0x118, the frame window 0x1E01B80 on 0x98, and
//! keeps the nodes' update-0x96 / 0xA0 points D+0x3B50.. Each line walks to
//! node 0x22 / 0x2E, then rides it until 0x100; on 0x96..0xC7 it turns (+0x10
//! / +0x18), on 0xA4..0xF9 it measures its reach D+0x3B80 / 0x3B90, from
//! 0x100 it sinks to the target; every seventh update a sprite (0x1E01690)
//! and on 0x100..0x124 two sweeps (0x1E01230 / 0x1E013B0). Draws (each twice:
//! the header with a3 0, the geometry with 1): rings of the line 0x1E02430,
//! cones 0x1E02CF0 / 0x1E03070 (nine 40-degree quads), discs 0x1E03440 and
//! 0x1E03780. Returns 1 / 4 / 2 on 0x10E / 0x122 / 0x118, 8 at timer out.
//! The steps 0x1E005E0 / 0x1E00800 / 0x1E01A70 / 0x1E01B30 are tested by the
//! entry but never spawned (not ported), so neither are their draws.
use crate::b440::{fix, fixs, rem2};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0xDC;
const LINE: u32 = 0x01E0_0970;
const SWEEP_A: u32 = 0x01E0_1230;
const SWEEP_B: u32 = 0x01E0_13B0;
const PUFF: u32 = 0x01E0_1530;
const SPRITE: u32 = 0x01E0_1690;
const SPREAD: u32 = 0x01E0_1910;
const WINDOW: u32 = 0x01E0_1B80;
const POS: u32 = D + 0x80;
const TURNS: u32 = D + 0xB0;
const POINTS: u32 = D + 0xD0;
const SCALE: u32 = D + 0x110;
const TEX: u32 = D + 0x15C;
const TIMER: u32 = D + 0x3BA0;
const COLOUR: u32 = D + 0x3BA4;
const OT_AT: u32 = D + 0x3BAC;
const MEAN: u32 = D + 0x3BD4;
const NODES: u32 = D + 0x3BE8;
const KIND: u32 = D + 0x3BFA;
const COUNTER: u32 = D + 0x3BFE;
const SINES: u32 = D + 0x3C04;
/// The scratchpad, where the originals project.
const SPR: u32 = 0x7000_0000;

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

fn rand_mod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn sin(fx: &mut Fx, deg: i32, cos: i32) -> f32 {
    l2::sin_deg(fx, SINES, deg as i16 as i32, cos)
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

/// The node `k` (of D+0x3BE8) of the attacker into `out`.
fn node(fx: &mut Fx, k: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i16(NODES + 2 * k);
    fx.node_position(h, s, out);
}

/// The entry 0x1E01D00 (frame 0x110: s0 = D, s1 the param / the loop's
/// index, s2 the drawn count, s3 the slot).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x110);
    fx.mem.write_u32(OT_AT, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp) };
    fx.leave(0x110);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3D70);
    l2::sine_table(fx, SINES);
    fx.mem.set16(D + 0x3BFC, param);
    fx.mem.set32(TIMER, 0x12C);
    for (k, (n, clear)) in [(0x24, 1), (0x2B, 0), (0x22, 0), (0x2E, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    for k in 0xAu32..0x10 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x148 + 4 * (k - 0xA), t);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    for k in 0..TASKS {
        let slot = D + 0x40 * k;
        if fx.mem.i16(slot + 0x30A) == 0 {
            continue;
        }
        let t = slot + 0x2D4;
        // The entry tests each step in turn, reading the slot afresh: a line
        // that ended and spawned into its own slot has that sweep stepped
        // in the same pass.
        let is = |fx: &mut Fx, f: u32| fx.mem.read_u32(slot + 0x2F8) == f;
        if is(fx, LINE) {
            line(fx, t);
        }
        if is(fx, SWEEP_A) {
            sweep(fx, t, [0x20, 0x20, 0]);
        }
        if is(fx, SWEEP_B) {
            sweep(fx, t, [0x40, 0, 0]);
        }
        if is(fx, PUFF) {
            puff(fx, t);
        }
        if is(fx, SPRITE) {
            sprite(fx, t);
        }
        if is(fx, SPREAD) {
            spread(fx, t);
        }
        if is(fx, WINDOW) {
            window(fx, t);
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    let z = ee::sub(fx.mem.f(sp + 0x58), 25.0);
    fx.mem.setf(sp + 0x58, z);
    let d = sp + 0x78;
    for k in 0..TASKS {
        let slot = D + 0x40 * k;
        if fx.mem.i16(slot + 0x30A) == 0 {
            continue;
        }
        let t = slot + 0x2D4;
        let k: Option<fn(&mut Fx, u32, u32, i32)> = match fx.mem.read_u32(slot + 0x2F8) {
            LINE => Some(line_draw),
            SWEEP_A => Some(cone_a),
            SWEEP_B => Some(cone_b),
            PUFF | SPREAD => Some(disc),
            SPRITE => Some(ribbon),
            _ => None,
        };
        if let Some(draw) = k {
            draw(fx, t, d, 0);
            draw(fx, t, d, 1);
        }
    }
    if counter(fx) >= 0x15 {
        if f64::from(fx.mem.f(D + 0x100)) < 0.5 {
            let v = ee::add(fx.mem.f(D + 0x100), f32::from_bits(0x3C23_D70A));
            fx.mem.setf(D + 0x100, v);
        }
        fx.mem.write_u32(D + 0x104, 0x3DCC_CCCD);
        if f64::from(fx.mem.f(D + 0x108)) < 0.5 {
            let v = ee::add(fx.mem.f(D + 0x108), f32::from_bits(0x3C23_D70A));
            fx.mem.setf(D + 0x108, v);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0x10E => 1,
        0x122 => 4,
        0x118 => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E02310(D, step, 0).
fn spawn(fx: &mut Fx, f: u32) {
    if let Some(k) = (0..TASKS).find(|&k| fx.mem.i16(D + 0x40 * k + 0x30A) == 0) {
        let t = D + 0x40 * k + 0x2D4;
        fx.bzero(t, 0x40);
        fx.mem.set16(t + 0x36, 1);
        for o in [0x28u32, 0x2C] {
            let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
            l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
            let a = fx.mem.read_u8(COLOUR + 3);
            fx.mem.write_u8(t + o + 3, a);
        }
        l2::to_i16x4(fx, t, POS, 0);
        let v = fx.mem.i16(KIND);
        fx.mem.set16(t + 0x30, v);
        fx.mem.write_u32(t + 0x20, 0);
        fx.mem.write_u32(t + 0x24, f);
    }
}

/// 0x1E00260(D, 0) (frame 0x60).
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x60);
    for (k, v) in [0x80u8, 0, 0, 0].into_iter().enumerate() {
        fx.mem.write_u8(COLOUR + k as u32, v);
    }
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    if counter(fx) == 0x96 {
        node(fx, 0, D + 0x3B50);
    }
    if counter(fx) == 0xA0 {
        node(fx, 2, D + 0x3B60);
        node(fx, 3, D + 0x3B70);
    }
    if counter(fx) == 0x98 {
        spawn(fx, WINDOW);
    }
    if counter(fx) == 0x3C {
        for s1 in 0..2u32 {
            node(fx, s1, sp + 0x50);
            copy3(fx, POS, sp + 0x50);
            fx.mem.set16(KIND, s1 as i32);
            spawn(fx, LINE);
        }
    }
    if (0x3C..0x45).contains(&counter(fx)) {
        for s3 in 0..2u32 {
            node(fx, s3, sp + 0x50);
            copy3(fx, POS, sp + 0x50);
            for _ in 0..4 {
                spawn(fx, PUFF);
            }
        }
    }
    if counter(fx) == 0x118 {
        for s2 in 0..2u32 {
            copy3(fx, POS, POINTS + 0x10 * s2);
            for _ in 0..0x1E {
                spawn(fx, SPREAD);
            }
        }
    }
    if counter(fx) == 0x3C {
        for o in [0u32, 4, 8] {
            fx.mem.write_u32(SCALE + o, 0x3CF5_C28F);
        }
    }
    if counter(fx) >= 0x3D {
        let x = fx.mem.f(SCALE);
        if crate::exe::ee_lt(x, f32::from_bits(0x3E4C_CCCD)) {
            let up = f32::from_bits(0x3C23_D70A);
            fx.mem.setf(SCALE, ee::add(x, up));
            fx.mem.write_u32(SCALE + 4, 0x3D4C_CCCD);
            let z = ee::add(fx.mem.f(SCALE + 8), up);
            fx.mem.setf(SCALE + 8, z);
        }
    }
    let a = fx.mem.i16(D + 0x3BF6) - 8;
    fx.mem.set16(D + 0x3BF6, a);
    let a = fx.mem.i16(D + 0x3BF6);
    if a < -0x167 {
        let r = fx.div(a, 0x168).1;
        fx.mem.set16(D + 0x3BF6, r);
    }
    fx.leave(0x60);
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

fn add16(fx: &mut Fx, a: u32, d: i32) {
    let v = fx.mem.i16(a).wrapping_add(d);
    fx.mem.set16(a, v);
}

fn add32(fx: &mut Fx, a: u32, d: i32) {
    let v = fx.mem.i32(a).wrapping_add(d);
    fx.mem.set32(a, v);
}

/// The alpha +0x2B up by `up` while below `cap`.
fn fade_in(fx: &mut Fx, t: u32, cap: u8, up: u8) {
    let v = fx.mem.read_u8(t + 0x2B);
    if v < cap {
        fx.mem.write_u8(t + 0x2B, v.wrapping_add(up));
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

/// The task's +0 / +2 / +4 from the floats at `p` through 0x1A2B80.
fn point_from(fx: &mut Fx, t: u32, p: u32, ras: [u32; 3]) {
    for (k, ra) in ras.into_iter().enumerate() {
        let v = fx.mem.f(p + 4 * k as u32);
        let i = fixs(fx, v, ra);
        fx.mem.set16(t + 2 * k as u32, i);
    }
}

/// The task's i16 point as floats at `out`.
fn point_to(fx: &mut Fx, t: u32, out: u32) {
    for k in 0..3u32 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(out + 4 * k, v);
    }
}

/// 0x1A2B80 of yaw / 180 (i16) of the attacker (0: facing one way).
fn facing(fx: &mut Fx, at: u32, ra: u32) -> i32 {
    let h = fx.self_handle();
    fx.unit_rotation(h, at);
    let v = ee::div(ee::from_i32(fx.mem.i32(at + 4)), 180.0);
    fixs(fx, v, ra) as i16 as i32
}

/// One walking stage of a line: node `to` into sp + 0x60, x one step (`down`
/// or up) until within 2 of its x, then x taken from it and state `next`.
fn walk(fx: &mut Fx, t: u32, sp: u32, from: u32, to: u32, down: bool, next: i32, ra: u32) {
    node(fx, from, sp + 0x50);
    node(fx, to, sp + 0x60);
    let x = ee::sub(fx.mem.f(sp + 0x60), fx.mem.f(sp + 0x50));
    fx.mem.setf(sp + 0x50, x);
    let f = ee::sub(fx.mem.f(sp + 0x60), ee::from_i32(fx.mem.i16(t)));
    let r = fx.sqrtf(ee::mul(f, f));
    if crate::exe::ee_le(r, 2.0) {
        let v = fx.mem.f(sp + 0x60);
        let i = fixs(fx, v, ra);
        fx.mem.set16(t, i);
        fx.mem.set16(t + 0x30, next);
    } else {
        add16(fx, t, if down { -1 } else { 1 });
    }
}

/// The reach of a line at the node `node_k` from the stored point `at`
/// into +8 / +0xA / +0xC and the floats `out`.
fn reach(fx: &mut Fx, t: u32, sp: u32, node_k: u32, at: u32, out: u32, ras: [u32; 3]) {
    node(fx, node_k, sp + 0x60);
    let dx = ee::neg(ee::sub(fx.mem.f(at), fx.mem.f(sp + 0x60)));
    let i = fixs(fx, ee::mul(16.0, dx), ras[0]);
    fx.mem.set16(t + 8, i);
    fx.mem.setf(out, ee::from_i32(i as i16 as i32));
    let dy = ee::sub(fx.mem.f(at + 4), fx.mem.f(sp + 0x64));
    let i = fixs(fx, ee::mul(10.0, dy), ras[1]);
    fx.mem.set16(t + 0xA, i);
    fx.mem.setf(out + 4, ee::from_i32(i as i16 as i32));
    let dx = ee::neg(ee::sub(fx.mem.f(at), fx.mem.f(sp + 0x60)));
    let i = fixs(fx, ee::mul(16.0, dx), ras[2]);
    fx.mem.set16(t + 0xC, i);
    fx.mem.setf(out + 8, ee::from_i32(i as i16 as i32));
}

/// On 0x96..0xC7 the line's turn: +0x18 by 0x4EC to -+0x3800 (direction by
/// its kind and the facing), +0x10 by 0x100 to 0xA00.
fn spin(fx: &mut Fx, t: u32, s0: i32) {
    let k = fx.mem.i16(t + 6);
    let (dn, up) = if s0 != 0 { (0, 1) } else { (1, 0) };
    if k == dn {
        let v = fx.mem.i32(t + 0x18);
        if v >= -0x37FF {
            add32(fx, t + 0x18, -0x4EC);
        }
    }
    let k = fx.mem.i16(t + 6);
    if k == up {
        let v = fx.mem.i32(t + 0x18);
        if v < 0x3800 {
            add32(fx, t + 0x18, 0x4EC);
        }
    }
    let v = fx.mem.i32(t + 0x10);
    if v < 0xA00 {
        add32(fx, t + 0x10, 0x100);
    }
}

/// 0x1E00970 (frame 0x80): a line from node 0 / 1 (kind +6).
fn line(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x80);
    let s0 = facing(fx, sp + 0x70, 0x01E0_09C8);
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 4);
        fx.mem.set16(t + 0x34, 0);
        let v = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 6, v);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 0x14 {
        fade_in(fx, t, 0x50, 4);
    }
    if age(fx, t) >= 0x15 {
        match fx.mem.i16(t + 0x30) {
            0 => walk(fx, t, sp, 0, 2, s0 != 0, 0xA, 0x01E0_0AEC),
            1 => walk(fx, t, sp, 1, 3, s0 == 0, 0x64, 0x01E0_0BB8),
            0xA if counter(fx) < 0x100 => {
                node(fx, 2, sp + 0x60);
                point_from(fx, t, sp + 0x60, [0x01E0_0C08, 0x01E0_0C14, 0x01E0_0C20]);
            }
            0x64 if counter(fx) < 0x100 => {
                node(fx, 3, sp + 0x60);
                point_from(fx, t, sp + 0x60, [0x01E0_0C68, 0x01E0_0C74, 0x01E0_0C80]);
            }
            _ => {}
        }
    }
    if (0x96..0xC8).contains(&counter(fx)) {
        spin(fx, t, s0);
    }
    if counter(fx) >= 0x100 {
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x50);
        node(fx, 2, sp + 0x60);
        let z = ee::sub(fx.mem.f(sp + 0x58), fx.mem.f(sp + 0x68));
        fx.mem.setf(sp + 0x58, z);
        let v = ee::add(ee::from_i32(fx.mem.i16(t + 4)), ee::div(z, 20.0));
        let i = fixs(fx, v, 0x01E0_0DF0);
        fx.mem.set16(t + 4, i);
        if counter(fx) == 0x102 {
            add16(fx, t + 2, -8);
        }
    }
    if fx.mem.i32(TIMER) < 0x14 {
        fade_out(fx, t, 0xF, 0xD);
    }
    if (0xA4..0xFA).contains(&counter(fx)) {
        if fx.mem.i16(t + 6) == 0 {
            reach(fx, t, sp, 2, D + 0x3B60, D + 0x3B80, [0x01E0_0EA8, 0x01E0_0EE4, 0x01E0_0F24]);
        }
        if fx.mem.i16(t + 6) == 1 {
            reach(fx, t, sp, 3, D + 0x3B70, D + 0x3B90, [0x01E0_0F90, 0x01E0_0FCC, 0x01E0_100C]);
        }
    }
    l2::rgb(fx, t + 0x28, 0x50, 0x50, 0x10);
    fx.mem.set16(t + 0x38, 0x64);
    add32(fx, t + 0x14, -0x400);
    grow_age(fx, t);
    if counter(fx) >= 0xFB {
        let v = fx.mem.i32(t + 0x10);
        fx.mem.set32(t + 0x10, if v > 0 { v - 0x100 } else { 0 });
    }
    let k = fx.mem.i16(t + 6) as u32;
    point_to(fx, t, POINTS + 0x10 * k);
    copy4(fx, TURNS + 0x10 * k, t + 0x10);
    let n = counter(fx);
    if fx.div(n, 7).1 == 1 && n < 0x118 {
        point_to(fx, t, POS);
        let v = fx.mem.i16(t + 6);
        fx.mem.set16(KIND, v);
        spawn(fx, SPRITE);
    }
    if (0x100..0x125).contains(&counter(fx)) {
        point_to(fx, t, POS);
        spawn(fx, SWEEP_A);
        spawn(fx, SWEEP_B);
    }
    fx.leave(0x80);
}

/// 0x1E01230 / 0x1E013B0 (frame 0x40): a cone of colour `c` at the line's
/// point, its turn from the facing, in by 0x40 for two updates, then out by
/// 5; +0x30 up by 8 to 0xA0; out by 0xD below timer 15.
fn sweep(fx: &mut Fx, t: u32, c: [i32; 3]) {
    let sp = fx.enter(0x40);
    l2::rgb(fx, t + 0x28, c[0], c[1], c[2]);
    if first(fx, t) {
        let ra = if c[1] != 0 { 0x01E0_12A8 } else { 0x01E0_1428 };
        let s = facing(fx, sp + 0x30, ra);
        fx.mem.set32(t + 0x10, if s != 0 { -0x4000 } else { -0xC000 });
        fx.mem.set32(t + 0x14, 0);
        fx.mem.set32(t + 0x18, 0x3800);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x30, 0x5A);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0);
    }
    if age(fx, t) < 2 {
        fade_in(fx, t, 0x80, 0x40);
    } else {
        fade_out(fx, t, 7, 5);
    }
    fx.mem.set16(t + 0x38, 0x32);
    grow_age(fx, t);
    if fx.mem.i16(t + 0x30) < 0xA0 {
        add16(fx, t + 0x30, 8);
    }
    if fx.mem.i32(TIMER) < 0xF {
        fade_out(fx, t, 0xF, 0xD);
    }
    fx.leave(0x40);
}

/// The ends of 0x1E01530 / 0x1E01690 / 0x1E01910: in by 0x50 for two
/// updates, then from size `big` out by 0xD and below timer 10 by 0xF.
fn puff_fade(fx: &mut Fx, t: u32, big: i32) {
    if age(fx, t) < 2 {
        fade_in(fx, t, 0xA0, 0x50);
    } else {
        if fx.mem.i16(t + 0x38) >= big {
            fade_out(fx, t, 0xF, 0xD);
        }
        if fx.mem.i32(TIMER) < 0xA {
            fade_out(fx, t, 0x11, 0xF);
        }
    }
}

/// 0x1E01530 (frame 0x30): an orange disc of radius 4 + 2 age at a node,
/// turned at random, cell rand % 7.
fn puff(fx: &mut Fx, t: u32) {
    fx.enter(0x30);
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 4);
        let r = rand_mod(fx, t, 0x01E0_1560, 0x6000);
        fx.mem.set32(t + 0x18, r - 0x7000);
        let r = rand_mod(fx, t, 0x01E0_1580, 0xFFFF);
        fx.mem.set32(t + 0x14, r);
        let r = rand_mod(fx, t, 0x01E0_159C, 7);
        fx.mem.set16(t + 0x34, r);
        fx.mem.set16(t + 0x36, 2);
    }
    puff_fade(fx, t, 9);
    l2::rgb(fx, t + 0x28, 0xFF, 0x80, 0);
    grow_age(fx, t);
    add16(fx, t + 0x38, 2);
    fx.leave(0x30);
}

/// 0x1E01690 (frame 0x50): a ring on the line of kind +0x30: before 0x64 it
/// takes the line's point every update, then (with the node asked and
/// dropped) keeps its place until 0x100, then for its first four updates
/// again; the line's turn + age, size up by 1 to 0x14.
fn sprite(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x50);
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 4);
        let r = rand_mod(fx, t, 0x01E0_16C0, 7);
        fx.mem.set16(t + 0x34, r);
        fx.mem.set16(t + 0x36, 2);
    }
    puff_fade(fx, t, 9);
    let n = counter(fx);
    let kind = fx.mem.i16(t + 0x30) as u32;
    if n < 0x64 {
        point_from(fx, t, POINTS + 0x10 * kind, [0x01E0_1790, 0x01E0_17A8, 0x01E0_17C0]);
    } else if n < 0x100 {
        if kind == 0 {
            node(fx, 2, sp + 0x40);
        }
        if fx.mem.i16(t + 0x30) == 1 {
            node(fx, 3, sp + 0x40);
        }
    } else if age(fx, t) < 4 {
        point_from(fx, t, POINTS + 0x10 * kind, [0x01E0_184C, 0x01E0_1864, 0x01E0_187C]);
    }
    let kind = fx.mem.i16(t + 0x30) as u32;
    copy4(fx, t + 0x10, TURNS + 0x10 * kind);
    add32(fx, t + 0x14, 0x400);
    l2::rgb(fx, t + 0x28, 0x80, 0x40, 0);
    grow_age(fx, t);
    if fx.mem.i16(t + 0x38) < 0x14 {
        add16(fx, t + 0x38, 1);
    }
    fx.leave(0x50);
}

/// 0x1E01910 (frame 0x30): a disc at a line's last point, turned at random,
/// radius 4 + 2 age, out from size 0x15.
fn spread(fx: &mut Fx, t: u32) {
    fx.enter(0x30);
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 4);
        let r = fx.rand_from(t, 0x01E0_1940);
        fx.mem.set32(t + 0x18, rem2(r, 0x4000) - 0x2000);
        let r = rand_mod(fx, t, 0x01E0_1960, 0xFFFF);
        fx.mem.set32(t + 0x14, r);
        let r = rand_mod(fx, t, 0x01E0_197C, 7);
        fx.mem.set16(t + 0x34, r);
        fx.mem.set16(t + 0x36, 2);
    }
    puff_fade(fx, t, 0x15);
    l2::rgb(fx, t + 0x28, 0x80, 0x40, 0);
    grow_age(fx, t);
    add16(fx, t + 0x38, 2);
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

/// 0x1E01B80 (frame 0x80): a full-window sprite (ALPHA 0x44) of the frame
/// (0x207EF0), grey, its alpha in by 0xC below 0x60 for 0x5A updates, then
/// out by 0xC.
fn window(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x80);
    let d = sp + 0x28;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for k in 0..3u32 {
        fx.mem.write_u8(sp + 0x38 + k, 0x80);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    fx.mem.write_u8(sp + 0x3B, a);
    fx.mem.write_u32(sp + 0x3C, 0x3F80_0000);
    corner(fx, sp + 0x48, 0x6FF8);
    corner(fx, sp + 0x58, 0x8FF8);
    fx.mem.write_u32(sp + 0x40, 0);
    fx.mem.write_u32(sp + 0x44, 0);
    fx.mem.write_u32(sp + 0x50, 0x3F80_0000);
    fx.mem.write_u32(sp + 0x54, 0x3F80_0000);
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, 0x80_0000_0044);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if age(fx, t) < 0x5A {
        fade_in(fx, t, 0x60, 0xC);
    } else {
        fade_out(fx, t, 0xE, 0xC);
    }
    grow_age(fx, t);
    fx.leave(0x80);
}

/// The header of a draw: a strip3 coloured +0x28 on the atlas cell (`id`,
/// +0x34) and the texture D+0x15C.
fn header(fx: &mut Fx, t: u32, d: u32, rect: u32, id: i32) {
    let c = fx.mem.i16(t + 0x34);
    l2::rect(fx, rect, id, c);
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x28, 0);
    l2::uv_strip3(fx, d, rect, 0);
    let tex = fx.mem.read_u32(TEX);
    fx.desc_texture(d, tex, 1);
}

fn submit_w(fx: &mut Fx, d: u32, ra: u32) {
    let w = fx.mem.f(MEAN);
    let key = fix(fx, w, ra);
    let ot = fx.mem.read_u32(OT_AT);
    fx.submit(d, ot, key as i32);
}

/// 0x1E02430(D, t, d, mode) (frame 0x1E0): the line. Mode 0 the header on
/// (0x9C, +0x34); mode 1 first (a line at its end, kind 0xA / 0x64, until
/// 0x100) puts the task at its node, then 35 rings turned 0x800 more about
/// y each, of twelve quads 30 degrees apart (14 sin, 25 + 7 cos, 0 / 15),
/// scale D+0x110, the turn +0x10 + 0x4000 about z, at the task's point plus
/// (+8, +0xA, +0xC) / 10, key 0.
fn line_draw(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x1E0);
    if mode == 0 {
        header(fx, t, d, sp + 0x120, 0x9C);
        fx.leave(0x1E0);
        return;
    }
    let kind = fx.mem.i16(t + 0x30);
    let live = (kind == 0xA || kind == 0x64) && counter(fx) < 0x100;
    if live {
        node(fx, if kind == 0xA { 2 } else { 3 }, sp + 0x110);
        let ras = if kind == 0xA { [0x01E0_2524, 0x01E0_2530, 0x01E0_253C] } else { [0x01E0_2584, 0x01E0_2590, 0x01E0_259C] };
        point_from(fx, t, sp + 0x110, ras);
    }
    for o in [0u32, 4, 8] {
        fx.mem.write_u32(sp + 0x1C0 + o, 0);
    }
    for k in 0..3u32 {
        let off = ee::div(ee::from_i32(fx.mem.i16(t + 8 + 2 * k)), 10.0);
        let base = if live { fx.mem.f(sp + 0x110 + 4 * k) } else { ee::from_i32(fx.mem.i16(t + 2 * k)) };
        fx.mem.setf(sp + 0xF0 + 4 * k, ee::add(base, off));
    }
    copy4(fx, sp + 0x1D0, t + 0x10);
    let v = fx.mem.i32(sp + 0x1D8).wrapping_add(0x4000);
    fx.mem.set32(sp + 0x1D8, v);
    for _ in 0..0x23 {
        l2::xform_vp(fx, sp + 0x130, SCALE, sp + 0x1D0, sp + 0xF0);
        fx.mem.write_u32(sp + 0xB4, 0);
        let r = std::array::from_fn(|k| fx.mem.i32(sp + 0x1C0 + 4 * k as u32));
        let m = fx.trs(None, Some(r), None);
        fx.write_m4(sp + 0x170, &m);
        crate::blib::exe::mul_mem(fx, sp + 0x130, sp + 0x170);
        let v = fx.mem.i32(sp + 0x1C4).wrapping_add(0x800);
        fx.mem.set32(sp + 0x1C4, v);
        let mut s2 = 0i32;
        for _ in 0..0xC {
            let f20 = sin(fx, s2, 0);
            let f21 = sin(fx, s2, 1);
            let s3 = (s2 + 0x1E) as i16 as i32;
            let f22 = sin(fx, s3, 0);
            let f0 = sin(fx, s3, 1);
            let f4 = ee::mul(14.0, f20);
            let f5 = ee::mul(14.0, f22);
            let f2 = ee::add(ee::mul(7.0, f21), 25.0);
            let f0 = ee::add(ee::mul(7.0, f0), 25.0);
            for (o, x, y, z) in [(0xB0u32, f2, f4, 0.0f32), (0xC0, f0, f5, 0.0), (0xD0, f2, f4, 15.0), (0xE0, f0, f5, 15.0)] {
                fx.mem.setf(sp + o, x);
                fx.mem.setf(sp + o + 4, y);
                fx.mem.setf(sp + o + 8, z);
            }
            if l2::project(fx, sp + 0xB0, SPR, sp + 0x130, d, sp + 0x1B0) {
                fx.submit(d, OT, 0);
            }
            s2 = (s2 + 0x1E) as i16 as i32;
        }
    }
    fx.leave(0x1E0);
}

/// The cones' common draw: nine quads 40 degrees apart from the angle
/// +0x3A, the second edge `edge` degrees on, heights 90 sin(+0x30) and 90
/// sin(+0x30 + 40), radii 90 cos(min(+0x30, 0xA0)) and 90 cos of that + 30,
/// through `sp` + 0xF0; each keyed by its mean w (D+0x3BD4).
fn cone(fx: &mut Fx, t: u32, d: u32, sp: u32, edge: i32, ra: u32) {
    let a1 = fx.mem.i16(t + 0x30);
    let s0 = if a1 < 0xA1 { a1 } else { 0xA0 };
    let h0 = ee::mul(90.0, sin(fx, a1, 0));
    fx.mem.setf(sp + 0x94, h0);
    fx.mem.setf(sp + 0xA4, h0);
    let h1 = ee::mul(90.0, sin(fx, fx_add(a1, 0x28), 0));
    fx.mem.setf(sp + 0xB4, h1);
    fx.mem.setf(sp + 0xC4, h1);
    let f20 = ee::mul(90.0, sin(fx, s0, 1));
    let f21 = ee::mul(90.0, sin(fx, fx_add(s0, 0x1E), 1));
    let mut s2 = 0i32;
    for _ in 0..9 {
        let a = fx.mem.i16(t + 0x3A).wrapping_add(s2);
        let f22 = sin(fx, a, 1);
        let f0 = sin(fx, a, 0);
        fx.mem.setf(sp + 0xB0, ee::mul(f22, f21));
        fx.mem.setf(sp + 0xB8, ee::mul(f0, f21));
        fx.mem.setf(sp + 0x90, ee::mul(f22, f20));
        fx.mem.setf(sp + 0x98, ee::mul(f0, f20));
        let a = fx.mem.i16(t + 0x3A).wrapping_add(edge).wrapping_add(s2);
        let f22 = sin(fx, a, 1);
        let f0 = sin(fx, a, 0);
        fx.mem.setf(sp + 0xC0, ee::mul(f22, f21));
        fx.mem.setf(sp + 0xC8, ee::mul(f0, f21));
        fx.mem.setf(sp + 0xA0, ee::mul(f22, f20));
        fx.mem.setf(sp + 0xA8, ee::mul(f0, f20));
        if l2::project(fx, sp + 0x90, SPR, sp + 0xF0, d, MEAN) {
            submit_w(fx, d, ra);
        }
        s2 = (s2 + 0x28) as i16 as i32;
    }
}

fn fx_add(a: i32, b: i32) -> i32 {
    (a + b) as i16 as i32
}

/// (float) ((double) x x c), 0x1A2C30 / 0x1A31D8 / 0x1A38F8.
fn dmul(x: f32, c: u64) -> f32 {
    (f64::from(x) * f64::from_bits(c)) as f32
}

/// 0x1E02CF0 (frame 0x140): header (0x97, +0x34); scale (s / 3, 1.2 s, s)
/// of s = +0x38 / 250, the turn +0x10.
fn cone_a(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x140);
    if mode == 0 {
        header(fx, t, d, sp + 0x130, 0x97);
        fx.leave(0x140);
        return;
    }
    point_to(fx, t, sp + 0xD0);
    let f21 = ee::div(ee::from_i32(fx.mem.i16(t + 0x38)), 250.0);
    let f20 = ee::div(f21, 3.0);
    let f0 = dmul(f21, 0x3FF3_3333_3333_3333);
    l2::set3(fx, sp + 0xE0, f20, f0, f21);
    l2::xform_vp(fx, sp + 0xF0, sp + 0xE0, t + 0x10, sp + 0xD0);
    cone(fx, t, d, sp, 0x28, 0x01E0_2FF4);
    fx.leave(0x140);
}

/// 0x1E03070 (frame 0x140): header (0x9B, +0x34); scale (1.3 s / 3, 2 s,
/// 1.3 s) of s = +0x38 / 250, the second edge 20 degrees on.
fn cone_b(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x140);
    if mode == 0 {
        header(fx, t, d, sp + 0x130, 0x9B);
        fx.leave(0x140);
        return;
    }
    point_to(fx, t, sp + 0xD0);
    let f22 = ee::div(ee::from_i32(fx.mem.i16(t + 0x38)), 250.0);
    let c = 0x3FF4_CCCC_CCCC_CCCD;
    let f21 = dmul(ee::div(f22, 3.0), c);
    let f20 = ee::mul(2.0, f22);
    let f0 = dmul(f22, c);
    l2::set3(fx, sp + 0xE0, f21, f20, f0);
    l2::xform_vp(fx, sp + 0xF0, sp + 0xE0, t + 0x10, sp + 0xD0);
    cone(fx, t, d, sp, 0x14, 0x01E0_33C4);
    fx.leave(0x140);
}

/// 0x1E03440 (frame 0x140): header (0x9B, +0x34); nine flat quads 40
/// degrees apart, 1 (4 for a 0x1E01910 task) degree wide between the
/// radii +0x38 - 7 and +0x38, scale (1, 1.5, 1), the turn +0x10.
fn disc(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x140);
    if mode == 0 {
        header(fx, t, d, sp + 0x130, 0x9B);
        fx.leave(0x140);
        return;
    }
    point_to(fx, t, sp + 0xD0);
    l2::set3(fx, sp + 0xE0, 1.0, 1.5, 1.0);
    l2::xform_vp(fx, sp + 0xF0, sp + 0xE0, t + 0x10, sp + 0xD0);
    for o in [0x94u32, 0xA4, 0xB4, 0xC4] {
        fx.mem.write_u32(sp + o, 0);
    }
    let s7 = if fx.mem.read_u32(t + 0x24) == SPREAD { 4 } else { 1 };
    let mut s2 = 0i32;
    for _ in 0..9 {
        let a = fx.mem.i16(t + 0x3A).wrapping_add(s2);
        let b = fx.mem.i16(t + 0x3A).wrapping_add(s7).wrapping_add(s2);
        for (o, x, r) in [(0xB0u32, a, 0), (0x90, a, -7), (0xC0, b, 0), (0xA0, b, -7)] {
            let f0 = sin(fx, x, 1);
            let f1 = ee::from_i32(fx.mem.i16(t + 0x38) + r);
            fx.mem.setf(sp + o, ee::mul(f1, f0));
            let f0 = sin(fx, x, 0);
            let f1 = ee::from_i32(fx.mem.i16(t + 0x38) + r);
            fx.mem.setf(sp + o + 8, ee::mul(f1, f0));
        }
        if l2::project(fx, sp + 0x90, SPR, sp + 0xF0, d, MEAN) {
            submit_w(fx, d, 0x01E0_3724);
        }
        s2 = (s2 + 0x28) as i16 as i32;
    }
    fx.leave(0x140);
}

/// The point of 0x1E03780 on a line's node: the node plus its reach / 10.
fn node_reach(fx: &mut Fx, sp: u32, k: u32, reach: u32) {
    node(fx, k, sp + 0x130);
    for c in [0u32, 4, 8] {
        let v = ee::add(fx.mem.f(sp + 0x130 + c), ee::div(fx.mem.f(reach + c), 10.0));
        fx.mem.setf(sp + 0xC0 + c, v);
    }
}

/// 0x1E03780 (frame 0x150): header (0x97, +0x34); eighteen flat quads 20
/// degrees wide between the radii +6 and +0x38 at the task's point (on
/// 0x65..0xFF the line's node plus its reach), the turn +0x10 + 0x4000 about
/// z.
fn ribbon(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x150);
    if mode == 0 {
        header(fx, t, d, sp + 0x110, 0x97);
        fx.leave(0x150);
        return;
    }
    point_to(fx, t, sp + 0xC0);
    if (0x65..0x100).contains(&counter(fx)) {
        if fx.mem.i16(t + 0x30) == 0 {
            node_reach(fx, sp, 2, D + 0x3B80);
        }
        if fx.mem.i16(t + 0x30) == 1 {
            node_reach(fx, sp, 3, D + 0x3B90);
        }
    }
    copy4(fx, sp + 0x140, t + 0x10);
    let v = fx.mem.i32(sp + 0x148).wrapping_add(0x4000);
    fx.mem.set32(sp + 0x148, v);
    l2::xform_vp(fx, sp + 0xD0, 0, sp + 0x140, sp + 0xC0);
    for o in [0x84u32, 0x94, 0xA4, 0xB4] {
        fx.mem.write_u32(sp + o, 0);
    }
    let mut s4 = 0i32;
    for _ in 0..0x12 {
        let a = fx.mem.i16(t + 0x3A).wrapping_add(s4);
        let b = fx.mem.i16(t + 0x3A).wrapping_add(0x14).wrapping_add(s4);
        for (o, x, w) in [(0xA0u32, a, 0x38u32), (0x80, a, 6), (0xB0, b, 0x38), (0x90, b, 6)] {
            let f1 = ee::from_i32(fx.mem.i16(t + w));
            let v = sin(fx, x, 1);
            fx.mem.setf(sp + o, ee::mul(f1, v));
            let f1 = ee::from_i32(fx.mem.i16(t + w));
            let v = sin(fx, x, 0);
            fx.mem.setf(sp + o + 8, ee::mul(f1, v));
        }
        if l2::project(fx, sp + 0x80, SPR, sp + 0xD0, d, MEAN) {
            submit_w(fx, d, 0x01E0_3B2C);
        }
        s4 = (s4 + 0x14) as i16 as i32;
    }
    fx.leave(0x150);
}
