//! BMODULE b121 (card 25), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E026D0, data area 0x1ED0000) [F]: b116's family on a pool of 150 tasks of 0x40 bytes at D+0x31C
//! (alive +0x36, step +0x24, each step called by its own `jal`; spawn
//! 0x1E02F60 = b086's on colour D+0x3FF4, index D+0x4036, +0x3A from
//! D+0x4032 (never written: 0) and the point D+0x250 (never written: 0)).
//! The emitter 0x1E017B0: on update 0x3D a glow 0x1E02560 (two
//! `ring_band_t20` bands at the attacker, flickering, until 0xD3); on odd
//! updates 0x3D..0xB4 a ring of quads 0x1E01DF0 at the attacker
//! (`ring_at_m_uv1`, closing in, cell 0x14 or 0x9B); from update 0x89
//! every fifth update one of eight orbs (D+0x4030): its point D+0x28D0 +
//! 0x10 k at the attacker 50 along z by the facing and +-3 jittered, an
//! orb 0x1E019D0 (`billboard::quad_b_twice_40`, sliding to the target's z
//! over 32 updates and shedding two spark rings 0x1E02030 a update into
//! the ring buffer D+0x3350 (100 slots, D+0x402C) until age 30, then eight
//! target rings 0x1E023B0 (b116's ring step) into D+0x3990 (D+0x402E)) and
//! its trail 0x1E021E0 (a 20-point history D+0x2950 + 0x140 k drawn by
//! `spline::tube_flat`). Returns 1 on update 0x96, 4 on 0xF5, 2 on 0xE1,
//! 6 on 0xA1..0xDB, 8 once the 250-update timer D+0x3FF0 has run out.
use crate::blib::{billboard, quads4, spawn as bspawn, spline, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0x96;
const POOL: u32 = D + 0x31C;
const ORB: u32 = 0x01E0_19D0;
const SWARM: u32 = 0x01E0_1DF0;
const SPARK: u32 = 0x01E0_2030;
const TRAIL: u32 = 0x01E0_21E0;
const BURST: u32 = 0x01E0_23B0;
const GLOW: u32 = 0x01E0_2560;
/// The steps with the entry's return address after each step's `jal`.
const STEPS: [(u32, u32); 6] = [(ORB, 0x01E0_28CC), (SWARM, 0x01E0_28F0), (SPARK, 0x01E0_2910), (TRAIL, 0x01E0_2930), (BURST, 0x01E0_2950), (GLOW, 0x01E0_2970)];
const ORBS: u32 = D + 0x28D0;
const HISTORY: u32 = D + 0x2950;
const SPARKS: u32 = D + 0x3350;
const BURSTS: u32 = D + 0x3990;
const TIMER: u32 = D + 0x3FF0;
const COLOUR: u32 = D + 0x3FF4;
const YAW: u32 = D + 0x4028;
const SPARK_NEXT: u32 = D + 0x402C;
const BURST_NEXT: u32 = D + 0x402E;
const ORB_NEXT: u32 = D + 0x4030;
const INDEX: u32 = D + 0x4036;
const COUNTER: u32 = D + 0x403A;
const BYTES: u32 = D + 0x403C;
const BYTES2: u32 = D + 0x413C;

fn copy3(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// rand % m (HI) from the `jal` returning to `ra` (rand's frame: s0).
fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E026D0 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    fx.mem.write_u32(D + 0x3FFC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xC0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x44B0);
    l2::sine_table(fx, D + 0x433C);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x4038, param);
    fx.mem.set32(TIMER, 0xFA);
    let h = fx.self_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0x4018, r);
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0x4026, r);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.fix_residue(v, 0x01E0_2798);
    fx.mem.set16(YAW, ee::fixsfsi(v));
    for id in 0xBu32..0x10 {
        let tex = fx.texture(4, id as i32);
        fx.mem.write_u32(D + 0x28C + 4 * (id - 0xB), tex);
    }
    let tex = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, tex);
    for k in 0..2u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x3FD0 + 4 * k, tex);
    }
    // rand's frame: s0 = D.
    for k in 0..0x100u32 {
        let r = rmod(fx, D, 0x01E0_2808, 0x12);
        fx.mem.write_u8(BYTES + k, (r - 9) as u8);
    }
    for k in 0xB4u32..0x100 {
        fx.mem.write_u8(BYTES + k, 0);
    }
    for k in 0..0x100u32 {
        let r = rmod(fx, D, 0x01E0_285C, 0x12);
        fx.mem.write_u8(BYTES2 + k, (r - 9) as u8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x24);
        if let Some(&(_, ra)) = STEPS.iter().find(|s| s.0 == f) {
            run_step(fx, f, t, ra, k);
        }
    }
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
    match fx.mem.i16(COUNTER) {
        0x96 => 1,
        0xF5 => 4,
        0xE1 => 2,
        0xA1..=0xDB => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` frame +0x34 (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32, id: i32) {
    let frame = fx.mem.i16(t + 0x34);
    l2::rect(fx, r, id, frame);
    for k in 0..4 {
        let w = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, w);
    }
}

fn uvs(fx: &mut Fx, v: [i32; 4]) {
    for (k, x) in v.into_iter().enumerate() {
        fx.mem.set16(D + 0x94 + 2 * k as u32, x);
    }
}

/// The 64-pixel cell +0x34 of the 4 x 2 sheet (0..3 the top row).
fn sheet(fx: &mut Fx, t: u32) {
    let f = fx.mem.i16(t + 0x34);
    let (u, v) = if f < 4 { (f << 6, 0) } else { ((f - 4) << 6, 0x40) };
    fx.mem.set16(D + 0x94, u);
    fx.mem.set16(D + 0x96, v);
    fx.mem.set16(D + 0x98, 0x3F);
    fx.mem.set16(D + 0x9A, 0x3F);
}

/// The attacker's position into `out`, z 5 toward the facing.
fn attacker(fx: &mut Fx, out: u32) {
    let h = fx.self_handle();
    fx.unit_position(h, out);
    let z = fx.mem.f(out + 8);
    let z = if fx.mem.i16(YAW) != 0 { ee::add(z, 5.0) } else { ee::sub(z, 5.0) };
    fx.mem.setf(out + 8, z);
}

/// A ring of quads at `pos` (D+0x20) with `rows` rows (D+0xB8), row offset
/// +0xA (D+0xC0), mode `mode` and offsets `table`.
fn ring_at(fx: &mut Fx, t: u32, rows: i32, pos: u32, mode: i32, table: u32) {
    fx.mem.set16(D + 0xB8, rows);
    let r = fx.mem.i16(t + 0xA);
    fx.mem.set16(D + 0xC0, r);
    copy3(fx, D + 0x20, pos);
    quads4::ring_at_m_uv1(fx, uv::uv_rect, D, t, mode, table);
    fx.mem.set16(D + 0xB8, 0);
}

/// The entry's draw case for a live task (`sp` the entry's frame).
fn draw(fx: &mut Fx, sp: u32, t: u32) {
    let f = fx.mem.read_u32(t + 0x24);
    match f {
        ORB => {
            let tex = fx.mem.read_u32(D + 0x3FD0);
            fx.mem.write_u32(D + 0x88, tex);
            uvs(fx, [0, 0, 0xFF, 0x3F]);
            fx.mem.set16(D + 0xBE, 0);
            let k = fx.mem.i16(t + 0x30) as u32;
            billboard::quad_b_twice_40(fx, uv::uv_rect, D, t, 0x20, ORBS + 0x10 * k);
        }
        SWARM => {
            attacker(fx, sp + 0x50);
            let tex = if fx.mem.i16(t + 0xC) == 0 {
                cell(fx, t, sp + 0x60, 0x14);
                D + 0x260
            } else {
                cell(fx, t, sp + 0x70, 0x9B);
                D + 0x29C
            };
            let tex = fx.mem.read_u32(tex);
            fx.mem.write_u32(D + 0x88, tex);
            ring_at(fx, t, 0xA, sp + 0x50, 0x21, BYTES);
        }
        SPARK => {
            let tex = fx.mem.read_u32(D + 0x3FD4);
            fx.mem.write_u32(D + 0x88, tex);
            sheet(fx, t);
            let j = fx.mem.i16(t + 0xC) as u32;
            ring_at(fx, t, 8, SPARKS + 0x10 * j, 0x20, BYTES2);
        }
        TRAIL => {
            let tex = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, tex);
            uvs(fx, [1, 0x10, 0x1F, 1]);
            cell(fx, t, sp + 0x80, 0x97);
            fx.mem.set16(D + 0x98, 1);
            for (o, v) in [(0x20u32, 0.0f32), (0x24, 60.0), (0x28, 0.0)] {
                fx.mem.setf(D + o, v);
            }
            fx.mem.set16(D + 0xBE, 1);
            let k = fx.mem.i16(t + 0x30) as u32;
            spline::tube_flat(fx, D, t, HISTORY + 0x140 * k, 0);
        }
        BURST => {
            match fx.mem.i16(t + 0x3A) {
                0 => {
                    cell(fx, t, sp + 0x90, 0x14);
                    let tex = fx.mem.read_u32(D + 0x260);
                    fx.mem.write_u32(D + 0x88, tex);
                }
                1 => {
                    cell(fx, t, sp + 0xA0, 0x14);
                    let v = fx.mem.i16(D + 0x96) + 0x80;
                    fx.mem.set16(D + 0x96, v);
                    let tex = fx.mem.read_u32(D + 0x3FD4);
                    fx.mem.write_u32(D + 0x88, tex);
                }
                _ => {
                    let tex = fx.mem.read_u32(D + 0x3FD4);
                    fx.mem.write_u32(D + 0x88, tex);
                    sheet(fx, t);
                }
            }
            let j = fx.mem.i16(t + 0xC) as u32;
            ring_at(fx, t, 7, BURSTS + 0x10 * j, 0x20, BYTES2);
        }
        GLOW => {
            attacker(fx, sp + 0x50);
            quads4::ring_band_t20(fx, D, t, 0x21, sp + 0x50);
            quads4::ring_band_t20(fx, D, t, 0x21, sp + 0x50);
        }
        _ => {}
    }
}

/// 0x1E017B0(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the spawns (frame 0x30: ra, s0 = D, f20 (the runner's: 0) [H]).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_2898), (0x10, u64::from(D))]);
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    // The spawn's saves: s0 = D, the entry's s1 = 0 / s2 = the parameter,
    // s3 / s4 (the runner's: 0) [H].
    let regs = [(0x40u32, 0u64), (0x30, 0), (0x20, param as i64 as u64), (0x10, 0), (0, u64::from(D))];
    if fx.mem.i16(COUNTER) == 0x3D {
        spawn(fx, GLOW, &regs, 0x01E0_1800);
    }
    let c = fx.mem.i16(COUNTER);
    let (_, r5) = fx.div(c, 5);
    if r5 == 1 && fx.mem.i16(ORB_NEXT) < 8 && c >= 0x89 {
        let k = fx.mem.i16(ORB_NEXT);
        fx.mem.set16(INDEX, k);
        let p = ORBS + 0x10 * k as u32;
        let h = fx.self_handle();
        fx.unit_position(h, p);
        let z = fx.mem.f(p + 8);
        let z = if fx.mem.i16(YAW) != 0 { ee::add(z, 50.0) } else { ee::sub(z, 50.0) };
        fx.mem.setf(p + 8, z);
        let r = rmod(fx, D, 0x01E0_18B8, 0x3C);
        let dx = ee::from_i32(r - 0x1E);
        let r = fx.rand_from(D, 0x01E0_18DC);
        let dx = ee::div(dx, 10.0);
        let x = ee::add(fx.mem.f(p), dx);
        fx.mem.setf(p, x);
        let (_, r) = fx.div(r, 0x3C);
        let dy = ee::div(ee::from_i32(r - 0x1E), 10.0);
        let y = ee::add(fx.mem.f(p + 4), dy);
        fx.mem.setf(p + 4, y);
        spawn(fx, ORB, &regs, 0x01E0_1948);
        spawn(fx, TRAIL, &regs, 0x01E0_195C);
        let n = fx.mem.i16(ORB_NEXT) + 1;
        fx.mem.set16(ORB_NEXT, n);
    }
    let c = fx.mem.i16(COUNTER);
    if c >= 0x3D && c % 2 == 1 && c < 0xB5 {
        spawn(fx, SWARM, &regs, 0x01E0_19B8);
    }
    fx.leave(0x30);
}

/// 0x1E02F60(D, step, 0): the first free task cleared and set off at
/// D+0x250 (as i16) with the colour D+0x3FF4 (twice), +0x30 = D+0x4036,
/// +0x3A = D+0x4032; `saves` the caller's s4..s0, `ra` the return address.
fn spawn(fx: &mut Fx, f: u32, saves: &[(u32, u64)], ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
    fx.saved(sp, &[(0x50, u64::from(ra))]);
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
            l2::to_i16x4(fx, t, D + 0x250, 0);
            let a = fx.mem.i16(INDEX);
            fx.mem.set16(t + 0x30, a);
            let a = fx.mem.i16(D + 0x4032);
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

/// A step's frame and its saves of the entry's s0 = D, s1 = 0, s2 = the
/// slot (D + 0x40 k), s3 = k.
fn run_step(fx: &mut Fx, f: u32, t: u32, ra: u32, k: u32) {
    let (ra, d) = (u64::from(ra), u64::from(D));
    let size = match f {
        ORB => 0x70,
        SPARK | BURST => 0x20,
        _ => 0x30,
    };
    let sp = fx.enter(size);
    match size {
        0x70 => fx.saved(sp, &[(0x40, ra), (0x30, u64::from(k)), (0x20, u64::from(D + 0x40 * k)), (0x10, 0), (0, d)]),
        0x20 => fx.saved(sp, &[(0x10, ra), (0, d)]),
        _ => fx.saved(sp, &[(0x20, ra), (0x10, 0), (0, d)]),
    }
    match f {
        ORB => orb(fx, sp, t),
        SWARM => swarm(fx, t),
        SPARK => spark(fx, t),
        TRAIL => trail(fx, t),
        BURST => crate::b116::ring(fx, t, BURST, [0x20, 0x20, 0x20]),
        _ => glow(fx, t),
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

/// 0x1E019D0 (D, task) in its frame `sp`: orb k (quads 127 x 31, white,
/// 0xC0 on odd ages): alpha +0x2B / +0x2F up by 10 to 0x80 over ages 4..31,
/// then +0x2B down by 0x22 twice an update; its point D+0x28D0 + 0x10 k
/// closes a 33rd of the attacker-to-target z gap an update while its age
/// is under 32, +0x14 -+0x4000 by the facing; until age 30 two spark rings
/// a update at its point (the first 3 lower) into D+0x3350, on age 30
/// eight target rings into D+0x3990.
fn orb(fx: &mut Fx, sp: u32, t: u32) {
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x50);
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x60);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 8, 0x7F);
        fx.mem.set16(t + 0xA, 0x1F);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x2C, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if age(fx, t) < 0x20 {
        for at in [0x2Bu32, 0x2F] {
            if age(fx, t) >= 4 {
                let a = fx.mem.read_u8(t + at);
                if a < 0x80 {
                    fx.mem.write_u8(t + at, a + 0xA);
                }
            }
        }
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x24 {
            fx.mem.set16(t + 0x36, 0);
        } else {
            fx.mem.write_u8(t + 0x2B, a - 0x22);
        }
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0x24 {
            fx.mem.write_u8(t + 0x2B, a - 0x22);
        }
    }
    let g = if age(fx, t) % 2 == 1 { 0xC0 } else { 0x80 };
    l2::rgb(fx, t + 0x28, g, g, g);
    grow(fx, t);
    let dz = ee::div(ee::sub(fx.mem.f(sp + 0x68), fx.mem.f(sp + 0x58)), 33.0);
    fx.mem.setf(sp + 0x58, dz);
    let turn = if fx.mem.i16(YAW) != 0 { -0x4000 } else { 0x4000 };
    fx.mem.set32(t + 0x14, turn);
    let k = fx.mem.i16(t + 0x30) as u32;
    let p = ORBS + 0x10 * k;
    if age(fx, t) < 0x20 {
        let z = ee::add(fx.mem.f(p + 8), fx.mem.f(sp + 0x58));
        fx.mem.setf(p + 8, z);
    }
    // The spawn's saves: s0 = the loop counter, s1 = sp + 0x58, s2 = the
    // task, s3 = D, s4 (the runner's: 0) [H].
    let regs = |j: u32| [(0x40u32, 0u64), (0x30, u64::from(D)), (0x20, u64::from(t)), (0x10, u64::from(sp + 0x58)), (0, u64::from(j))];
    if age(fx, t) < 0x1E {
        for j in 0..2u32 {
            let s = SPARKS + 0x10 * fx.mem.i16(SPARK_NEXT) as u32;
            copy3(fx, s, p);
            if j == 0 {
                let z = ee::sub(fx.mem.f(s + 8), 3.0);
                fx.mem.setf(s + 8, z);
            }
            let i = fx.mem.i16(SPARK_NEXT);
            fx.mem.set16(INDEX, i);
            spawn(fx, SPARK, &regs(j), 0x01E0_1CB4);
            next(fx, SPARK_NEXT);
        }
    }
    if age(fx, t) == 0x1E {
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x50);
        for j in 0..8u32 {
            let s = BURSTS + 0x10 * fx.mem.i16(BURST_NEXT) as u32;
            copy3(fx, s, sp + 0x50);
            let i = fx.mem.i16(BURST_NEXT);
            fx.mem.set16(INDEX, i);
            spawn(fx, BURST, &regs(j), 0x01E0_1D84);
            next(fx, BURST_NEXT);
        }
    }
}

/// The ring buffer index at `at` up by 1, back to 0 at 100.
fn next(fx: &mut Fx, at: u32) {
    let n = fx.mem.i16(at) + 1;
    fx.mem.set16(at, if n >= 0x64 { 0 } else { n });
}

/// 0x1E01DF0 (D, task): the attacker's closing ring (size 8 growing by
/// 0..2 to 17, grey 0x20, cell 0..6 (counting to 7 every third update for
/// slot 0), row offset +0xA 0..7 (+0xC the slot), radius 20..119 closing by
/// 1..3, row +8, life +6 2..4, orientation +0x3A falling by 1); alpha 0x80
/// for its life; it ends once its radius is out or from update 0xD3.
fn swarm(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 8);
        let v = rmod(fx, t, 0x01E0_1E20, 7);
        fx.mem.set16(t + 0x34, v);
        l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x20);
        let v = fx.rand_from(t, 0x01E0_1E4C);
        fx.mem.set16(t + 0xA, v & 7);
        let s = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xC, s);
        for (o, ra, m, add) in [(0x30u32, 0x01E0_1E70u32, 0x64, 0x14), (0xE, 0x01E0_1E90, 3, 1), (8, 0x01E0_1EB0, 0x23, 0), (6, 0x01E0_1ECC, 3, 2), (0x3A, 0x01E0_1EEC, 0x168, 0)] {
            let v = rmod(fx, t, ra, m);
            fx.mem.set16(t + o, v + add);
        }
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x2B, 0x80);
    }
    if fx.mem.i16(t + 0xC) == 0 {
        let a = age(fx, t);
        let (_, r) = fx.div(a, 3);
        if r == 1 && fx.mem.i16(t + 0x34) < 7 {
            let f = fx.mem.i16(t + 0x34) + 1;
            fx.mem.set16(t + 0x34, f);
        }
    }
    if age(fx, t) != 0 {
        let r = fx.mem.i16(t + 0x30);
        if r > 0 {
            let e = fx.mem.i16(t + 0xE);
            fx.mem.set16(t + 0x30, r - e);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    if age(fx, t) != 0 {
        let a = fx.mem.i16(t + 0x3A) - 1;
        fx.mem.set16(t + 0x3A, a);
    }
    if fx.mem.i16(t + 0x38) < 0x11 {
        let v = rmod(fx, t, 0x01E0_1FCC, 3);
        let s = fx.mem.i16(t + 0x38) + (v as i16 as i32);
        fx.mem.set16(t + 0x38, s);
    }
    grow(fx, t);
    if fx.mem.i16(COUNTER) >= 0xD3 {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// 0x1E02030 (D, task): a spark ring (size 38 shrinking by 4, grey 0x20,
/// cell 0..6, row offset +0xA 0..7 (+0xC the slot), radius 0..19 (fixed;
/// +0xE 2..11 unused), row +8, life +6 1..2, orientation +0x3A falling by
/// 7); alpha 0x80 for its life, then down by 0x1D.
fn spark(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x26);
        let v = rmod(fx, t, 0x01E0_2058, 7);
        fx.mem.set16(t + 0x34, v);
        l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x20);
        let v = fx.rand_from(t, 0x01E0_2084);
        fx.mem.set16(t + 0xA, v & 7);
        let s = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xC, s);
        for (o, ra, m, add) in [(0x30u32, 0x01E0_20A8u32, 0x14, 0), (0xE, 0x01E0_20C4, 0xA, 2), (8, 0x01E0_20E4, 0x23, 0)] {
            let v = rmod(fx, t, ra, m);
            fx.mem.set16(t + o, v + add);
        }
        let v = fx.rand_from(t, 0x01E0_2100);
        fx.mem.set16(t + 6, (v & 1) + 1);
        let v = rmod(fx, t, 0x01E0_2120, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x1F {
            fx.mem.set16(t + 0x36, 0);
        } else {
            fx.mem.write_u8(t + 0x2B, a - 0x1D);
        }
    }
    if age(fx, t) != 0 && fx.mem.i16(t + 0x38) != 0 {
        let s = fx.mem.i16(t + 0x38) - 4;
        fx.mem.set16(t + 0x38, s);
    }
    if age(fx, t) != 0 {
        let a = fx.mem.i16(t + 0x3A) - 7;
        fx.mem.set16(t + 0x3A, a);
    }
    grow(fx, t);
}

/// 0x1E021E0 (D, task): orb k's trail (5 points, size 35): its 20-point
/// history D+0x2950 + 0x140 k filled with the orb's point on its first
/// update and pushed with it every update (0x1E00080, newest first);
/// alpha 0x80 for 30 updates, then down by 0x31; red (0x60, 0x10, 0x10)
/// on even ages, (0x60, 0x60, 0x10) on odd; +0x14 -+0x4000 by the facing
/// and the orb's point 4 along z.
fn trail(fx: &mut Fx, t: u32) {
    let k = fx.mem.i16(t + 0x30) as u32;
    let h = HISTORY + 0x140 * k;
    let p = ORBS + 0x10 * k;
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 6, 5);
        fx.mem.write_u8(t + 0x2B, 0);
        for j in 0..0x13 {
            copy3(fx, h + 0x10 * j, p);
        }
        fx.mem.set16(t + 0x38, 0x23);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 0x1E {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x29 {
            fx.mem.set16(t + 0x36, 0);
        } else {
            fx.mem.write_u8(t + 0x2B, a.wrapping_sub(0x31));
        }
    }
    if age(fx, t) % 2 == 0 {
        l2::rgb(fx, t + 0x28, 0x60, 0x10, 0x10);
    } else {
        l2::rgb(fx, t + 0x28, 0x60, 0x60, 0x10);
    }
    bspawn::history(fx, h + 0x130, 0x14, p);
    let z = fx.mem.f(p + 8);
    if fx.mem.i16(YAW) != 0 {
        fx.mem.set32(t + 0x14, -0x4000);
        fx.mem.setf(p + 8, ee::sub(z, 4.0));
    } else {
        fx.mem.set32(t + 0x14, 0x4000);
        fx.mem.setf(p + 8, ee::add(z, 4.0));
    }
    grow(fx, t);
}

/// 0x1E02560 (D, task): the attacker's glow (radius 50, grey 0x60 in, red
/// (0x80, 0x10, 0x10) out, outer alpha +0x2F 8): for 18 updates +0x2B up
/// by 3 below 0x20 and +0x2F by 12 below 0x80; grey 0x30 on odd ages, 0x80
/// on even; +0x10 0x4000 before update 100; it ends from update 0xD3.
fn glow(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x32);
        l2::rgb(fx, t + 0x28, 0x60, 0x60, 0x60);
        l2::rgb(fx, t + 0x2C, 0x80, 0x10, 0x10);
        let v = rmod(fx, t, 0x01E0_25BC, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 8);
    }
    if age(fx, t) < 0x12 {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x20 {
            fx.mem.write_u8(t + 0x2B, a + 3);
        }
        let a = fx.mem.read_u8(t + 0x2F);
        if a < 0x80 {
            fx.mem.write_u8(t + 0x2F, a + 0xC);
        }
    }
    let g = if age(fx, t) % 2 == 1 { 0x30 } else { 0x80 };
    l2::rgb(fx, t + 0x28, g, g, g);
    if fx.mem.i16(COUNTER) < 0x64 {
        fx.mem.write_u32(t + 0x10, 0x4000);
    }
    grow(fx, t);
    if fx.mem.i16(COUNTER) >= 0xD3 {
        fx.mem.set16(t + 0x36, 0);
    }
}
