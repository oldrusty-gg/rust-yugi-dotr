//! BMODULE b503 (card 349), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02440, data area 0x1ED0000) [F]: an older task pool of 190 tasks of 0x40 bytes at D+0x2A0 (alive
//! +0x36, step +0x24 as a record address) drawn with the second library:
//! rings of five camera-facing quads at a position (2d29e1f9b4be at
//! 0x1E00920, `quads4::ring_at`) and rings of three at D+0x20 with a mode
//! (383a1b86ead2 at 0x1E00CE0, `quads4::ring_at_m`), UVs by uv_rect
//! 0x1E00080. In three bursts (updates 51..89, 111..129, 153..264) grey
//! sparks and dark red smoke leave the attacker's node 0xB; from 131 to
//! 274 the target's node 1 throws grey sparks (odd updates) and red smoke
//! (even updates) about. Returns 1 on update 58, 4 on 300, 2 on 290, 6 on
//! 69..284, 8 once the 308-update timer +0x3270 has run out.
//!
//! The record's program is shared (one compiled family, other constants)
//! with b552 (card 196): [`Family`] holds what differs [F word-compare].
use crate::blib::{quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0xBE;
const TEX: u32 = D + 0x3250;
const TIMER: u32 = D + 0x3270;
const COLOUR: u32 = D + 0x3274;
const NODE: u32 = D + 0x329C;
const TNODE: u32 = D + 0x32AA;
const FACING: u32 = D + 0x32B2;
const COUNTER: u32 = D + 0x32B6;
const BYTES: u32 = D + 0x32B8;

/// What the records of this family differ in (addresses are the record's).
pub(crate) struct Family {
    /// The entry's frame.
    pub frame: u32,
    /// The steps: sparks at the attacker, smoke there, sparks at the
    /// target, smoke there (their addresses, also the tasks' +0x24).
    pub steps: [u32; 4],
    /// The entry's return addresses after each step's `jal`.
    pub step_ra: [u64; 4],
    /// The entry's `jal` rand in the draw loop (the two smoke kinds).
    pub draw_rand: [u32; 2],
    /// The return address of the entry's 0x1F71A0 before the draw loop.
    pub unit_ra: u64,
    /// The init's `jal` rand (the byte table).
    pub init_rand: u32,
    pub timer: i32,
    /// The attacker's node requested at init.
    pub node: i32,
    /// The update from which the fades end (`late`, `late` + 1).
    pub late: i32,
    /// The smoke's colour.
    pub smoke_rgb: [i32; 3],
    /// The emitter's return address into the entry.
    pub emit_ra: u64,
    /// Sparks / smoke at the attacker: active ranges of the counter.
    pub sparks_on: &'static [(i32, i32)],
    pub smoke_on: &'static [(i32, i32)],
    /// The smoke's offset towards the facing.
    pub smoke_dz: f32,
    /// The target's node: range, spark parity (Some(1) odd only).
    pub target_on: (i32, i32),
    pub target_parity: Option<i32>,
    /// The target's smoke: range, parity.
    pub tsmoke_on: (i32, i32),
    pub tsmoke_parity: Option<i32>,
    /// The emitter's `jal` rand sites: sparks (x, y), smoke (y, z facing,
    /// z else), the re-request, the target's smoke (x, y, z).
    pub sites: [u32; 9],
    /// Returns: (update, code) pairs, then 6 on the range.
    pub returns: [(i32, i32); 3],
    pub busy: (i32, i32),
}

const B503: Family = Family {
    frame: 0xB0,
    steps: [0x01E0_1740, 0x01E0_1AB0, 0x01E0_1E00, 0x01E0_2150],
    step_ra: [0x01E0_25B0, 0x01E0_25D0, 0x01E0_25F0, 0x01E0_2610],
    draw_rand: [0x01E0_2768, 0x01E0_28F8],
    unit_ra: 0x01E0_2634,
    init_rand: 0x01E0_2538,
    timer: 0x134,
    node: 0xB,
    late: 0x118,
    smoke_rgb: [0x60, 0x20, 0x20],
    emit_ra: 0x01E0_257C,
    sparks_on: &[(0x33, 0x5A), (0x6F, 0x82), (0x99, 0x109)],
    smoke_on: &[(0x33, 0x5A), (0x6F, 0x82), (0x99, 0x113)],
    smoke_dz: 1.0,
    target_on: (0x83, 0x113),
    target_parity: Some(1),
    tsmoke_on: (0x95, 0x113),
    tsmoke_parity: Some(0),
    sites: [0x01E0_12FC, 0x01E0_1330, 0x01E0_1420, 0x01E0_1464, 0x01E0_14B0, 0x01E0_1570, 0x01E0_1654, 0x01E0_1688, 0x01E0_16BC],
    returns: [(0x3A, 1), (0x12C, 4), (0x122, 2)],
    busy: (0x45, 0x11D),
};

/// The entry 0x1E02440.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, &B503, param, |fx, _, _| {
        for k in 0..2 {
            let t = fx.texture(5, k);
            fx.mem.write_u32(TEX + 4 * k as u32, t);
        }
    })
}

/// The family's entry; `textures` loads the record's textures.
pub(crate) fn run(fx: &mut Fx, f: &Family, param: i32, textures: impl Fn(&mut Fx, &Family, u32)) -> i32 {
    let sp = fx.enter(f.frame);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x327C, OT);
    let ret = if param >= 0 { init(fx, f, sp, param, textures) } else { step(fx, f, sp) };
    fx.leave(f.frame);
    ret
}

fn init(fx: &mut Fx, f: &Family, sp: u32, param: i32, textures: impl Fn(&mut Fx, &Family, u32)) -> i32 {
    fx.bzero(D, 0x3530);
    l2::sine_table(fx, D + 0x33B8);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x32B4, param);
    fx.mem.set32(TIMER, f.timer);
    let h = fx.self_handle();
    let r = fx.request_node(h, f.node, 1);
    fx.mem.set16(NODE, r);
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TNODE, r);
    textures(fx, f, sp);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..0x100u32 {
        let h = rmod(fx, D, f.init_rand, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    0
}

fn step(fx: &mut Fx, f: &Family, sp: u32) -> i32 {
    emit(fx, f);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        for (i, (&s, &ra)) in f.steps.iter().zip(&f.step_ra).enumerate() {
            if fx.mem.read_u32(t + 0x24) == s {
                match i {
                    0 => spark(fx, f, t, s, ra, true),
                    1 => smoke(fx, f, t, s, ra, true),
                    2 => spark(fx, f, t, s, ra, false),
                    _ => smoke(fx, f, t, s, ra, false),
                }
            }
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    // 0x1F71A0's frame below the entry's (its saved ra, the entry's s0).
    fx.saved(sp - 0x20, &[(0x10, f.unit_ra), (0, u64::from(D))]);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        for (i, &s) in f.steps.iter().enumerate() {
            if fx.mem.read_u32(t + 0x24) != s {
                continue;
            }
            let tex = TEX + 4 * (i as u32 % 2);
            let r = sp + 0x60 + 0x10 * i as u32;
            let w = fx.mem.read_u32(tex);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x34);
            l2::rect(fx, r, 0x14, sub);
            for k in 0..4 {
                let v = fx.mem.read_u32(r + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, v);
            }
            if i % 2 == 0 {
                for k in 0..3 {
                    let v = ee::from_i32(fx.mem.i16(t + 2 * k));
                    fx.mem.setf(sp + 0x50 + 4 * k, v);
                }
                fx.mem.set16(D + 0xB8, 5);
                fx.mem.set16(D + 0xBE, 0);
                quads4::ring_at(fx, uv::uv_rect, D, t, BYTES, sp + 0x50);
                fx.mem.set16(D + 0xB8, 0);
            } else if fx.mem.i16(t + 0x32) >= 4 {
                fx.mem.set16(D + 0xB8, 3);
                let v = fx.rand_from(D, f.draw_rand[i / 2] + 8);
                fx.mem.set16(D + 0xBE, v % 4);
                for k in 0..3 {
                    let v = ee::from_i32(fx.mem.i16(t + 2 * k));
                    fx.mem.setf(D + 0x20 + 4 * k, v);
                }
                quads4::ring_at_m(fx, uv::uv_rect, D, t, 0x20, BYTES);
                fx.mem.set16(D + 0xB8, 0);
            }
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    let c = fx.mem.i16(COUNTER);
    if let Some(&(_, r)) = f.returns.iter().find(|r| r.0 == c) {
        return r;
    }
    if (f.busy.0..f.busy.1).contains(&c) {
        6
    } else if fx.mem.i32(TIMER) > 0 {
        0
    } else {
        8
    }
}

/// rand % m (HI) from the `jal` at `site` (rand's frame: `s0`, ra).
fn rmod(fx: &mut Fx, s0: u32, site: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, site + 8);
    fx.div(v, m).1
}

/// In one of the ranges and not on an update of 10 k + 1 (the `div` only
/// when in range).
fn burst(fx: &mut Fx, c: i32, on: &[(i32, i32)]) -> bool {
    on.iter().any(|r| (r.0..r.1).contains(&c)) && fx.div(c, 0xA).1 != 1
}

/// 0x1E01240(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the emitters (frame 0x40).
fn emit(fx: &mut Fx, f: &Family) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x20, f.emit_ra), (0x10, 0), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    // The emitter's s0 (rand's frame): the entry's D, the spark counter
    // after the first burst ran.
    let mut s0 = D;
    let c = fx.mem.i16(COUNTER);
    // Two sparks about the attacker's node.
    if burst(fx, c, f.sparks_on) {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODE);
        fx.node_position(h, slot, sp + 0x30);
        for k in 0..2 {
            s0 = k;
            let r = rmod(fx, s0, f.sites[0], 0xA);
            let x = ee::sub(ee::add(fx.mem.f(sp + 0x30), ee::from_i32(r)), 5.0);
            fx.mem.setf(D + 0x250, x);
            let r = rmod(fx, s0, f.sites[1], 0xA);
            let y = ee::sub(ee::add(fx.mem.f(sp + 0x34), ee::from_i32(r)), 5.0);
            fx.mem.setf(D + 0x254, y);
            let z = fx.mem.read_u32(sp + 0x38);
            fx.mem.write_u32(D + 0x258, z);
            spawn(fx, f.steps[0]);
        }
        s0 = 2;
    }
    let c = fx.mem.i16(COUNTER);
    // Smoke there, `smoke_dz` towards the facing.
    if burst(fx, c, f.smoke_on) {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODE);
        fx.node_position(h, slot, sp + 0x30);
        let x = fx.mem.read_u32(sp + 0x30);
        fx.mem.write_u32(D + 0x250, x);
        let r = rmod(fx, s0, f.sites[2], 0xA);
        let y = ee::sub(ee::add(fx.mem.f(sp + 0x34), ee::from_i32(r)), 5.0);
        fx.mem.setf(D + 0x254, y);
        let z0 = fx.mem.f(sp + 0x38);
        let facing = fx.mem.i16(FACING) != 0;
        let z = if facing { ee::add(f.smoke_dz, z0) } else { ee::sub(z0, f.smoke_dz) };
        let r = rmod(fx, s0, if facing { f.sites[3] } else { f.sites[4] }, 0xA);
        let z = ee::sub(ee::add(z, ee::from_i32(r)), 5.0);
        fx.mem.setf(D + 0x258, z);
        spawn(fx, f.steps[1]);
    }
    let c = fx.mem.i16(COUNTER);
    if (f.target_on.0..f.target_on.1).contains(&c) {
        if fx.div(c, 0xA).1 == 1 {
            // The nodes re-requested: the attacker's 0x11, the target's 1.
            let h = fx.self_handle();
            let r = fx.request_node(h, 0x11, 1);
            fx.mem.set16(NODE, r);
            let h = fx.target_handle();
            fx.unit_nodes(h);
            fx.rand_from(s0, f.sites[5] + 8);
            let h = fx.target_handle();
            let r = fx.request_node(h, 1, 1);
            fx.mem.set16(TNODE, r);
        } else {
            let h = fx.target_handle();
            let slot = fx.mem.i16(TNODE);
            fx.node_position(h, slot, sp + 0x30);
            for k in 0..3 {
                let w = fx.mem.read_u32(sp + 0x30 + 4 * k);
                fx.mem.write_u32(D + 0x250 + 4 * k, w);
            }
            if f.target_parity.map_or(true, |p| fx.mem.i16(COUNTER) % 2 == p) {
                spawn(fx, f.steps[2]);
            }
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (f.tsmoke_on.0..f.tsmoke_on.1).contains(&c) && fx.div(c, 0xA).1 != 1 {
        let h = fx.target_handle();
        let slot = fx.mem.i16(TNODE);
        fx.node_position(h, slot, sp + 0x30);
        for k in 0..3 {
            let r = rmod(fx, s0, f.sites[6 + k as usize], 0x1E);
            let v = ee::sub(ee::add(fx.mem.f(sp + 0x30 + 4 * k), ee::from_i32(r)), 15.0);
            fx.mem.setf(D + 0x250 + 4 * k, v);
        }
        if f.tsmoke_parity.map_or(true, |p| fx.mem.i16(COUNTER) % 2 == p) {
            spawn(fx, f.steps[3]);
        }
    }
    fx.leave(0x40);
}

/// 0x1E02A20 (b552 0x1E02CD0) (D, step, 0): the first free task cleared
/// and set off at D+0x250 (as i16) with the colour D+0x3274 (twice), +0x30
/// = D+0x32B0; 1 when one was free.
fn spawn(fx: &mut Fx, step: u32) -> i32 {
    fx.enter(0x60);
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
            let a = fx.mem.i16(D + 0x32B0);
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

/// The fade both kinds share: below the life +6 up by `up` under `cap`
/// (before update `late`), after it down by `down` (the task ends at or
/// below `floor`); after update `late` down by 9 (ends below 11).
fn fade(fx: &mut Fx, f: &Family, t: u32, cap: u8, up: u8, floor: u8, down: u8) {
    let early = fx.mem.i16(COUNTER) < f.late;
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        let a = fx.mem.read_u8(t + 0x2B);
        if early && a < cap {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(up));
        }
    } else if early {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= floor {
            fx.mem.write_u8(t + 0x2B, a - down);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    if fx.mem.i16(COUNTER) > f.late {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0xB {
            fx.mem.write_u8(t + 0x2B, a - 9);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
}

/// 0x1E01740 (`drift` true) / 0x1E01E00 (D, task) at `s`: a spark ring
/// (frame 0x30): size 3..17, grey / light grey / white, radius 10..159
/// growing by 7..16 (0x1E01E00: 14..27), angle turning by 2 either way,
/// row 0..29, life 30..44; fades in by 0x19, out by 20; a random cell
/// frame 0..7; up by 1..3 on odd ages, down by 1..4 on even; 0x1E01740
/// also drifts 6 a update away from the facing.
fn spark(fx: &mut Fx, f: &Family, t: u32, s: u32, ra: u64, drift: bool) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, ra), (0x10, 0), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x36) == 1 {
        let v = rmod(fx, t, s + 0x24, 0xF);
        fx.mem.set16(t + 0x38, v + 3);
        let c = rmod(fx, t, s + 0x44, 3) as i16;
        let g = match c {
            0 => 0x80,
            1 => 0xB0,
            _ => 0xFF,
        };
        l2::rgb(fx, t + 0x28, g, g, g);
        let v = rmod(fx, t, s + 0xC8, 0x96);
        fx.mem.set16(t + 0x30, v + 0xA);
        let v = rmod(fx, t, s + 0xE8, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let (m, a) = if drift { (0xA, 7) } else { (0xE, 0xE) };
        let v = rmod(fx, t, s + 0x104, m);
        fx.mem.set16(t + 0xE, v + a);
        let v = rmod(fx, t, s + 0x124, 0x1E);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, s + 0x148);
        fx.mem.set16(t + 0xA, v % 2);
        let v = rmod(fx, t, s + 0x15C, 0xF);
        fx.mem.set16(t + 6, v + 0x1E);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    fade(fx, f, t, 0x80, 0x19, 0x16, 0x14);
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    let v = fx.rand_from(t, s + 0x250);
    fx.mem.set16(t + 0x34, v % 8);
    let y = fx.mem.i16(t + 2);
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let v = rmod(fx, t, s + 0x28C, 3);
        fx.mem.set16(t + 2, y + (v + 1) as i16 as i32);
    } else {
        let v = fx.rand_from(t, s + 0x2CC);
        fx.mem.set16(t + 2, y - (v % 4 + 1) as i16 as i32);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    let a = fx.mem.i16(t + 0x3A);
    let turn = if fx.mem.i16(t + 0xA) != 0 { 2 } else { -2 };
    fx.mem.set16(t + 0x3A, a + turn);
    if drift {
        let z = fx.mem.i16(t + 4);
        let dz = if fx.mem.i16(FACING) != 0 { -6 } else { 6 };
        fx.mem.set16(t + 4, z + dz);
    }
    fx.leave(0x30);
}

/// 0x1E01AB0 (`first`: frame 0x60, size 20, growth 4) / 0x1E02150 (frame
/// 0x30, 12, 2) (D, task) at `s`: smoke (the family's colour): size
/// `size`..+11 growing by `grow` (under 60) and 2 on odd ages, the radius
/// +0x30 kept in +0xA and growing from 0 by 2..6, row 0..34, life 1..6
/// (0x1E02150: 40..64); fades in by 0x40, out by 7; the cell frame
/// counting to 5 on odd ages, random angles +0x10 / +0x14 / +0x18, +0x3A
/// turning by 1. The first also reads the target's node and the
/// attacker's position and drifts 1 a update away from the facing.
fn smoke(fx: &mut Fx, f: &Family, t: u32, s: u32, ra: u64, first: bool) {
    let (frame, size, grow) = if first { (0x60, 0x14, 4) } else { (0x30, 0xC, 2) };
    let late = if first { 0x284 } else { 0x224 };
    let sp = fx.enter(frame);
    fx.saved(sp, &[(0x20, ra), (0x10, 0), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x36) == 1 {
        let v = rmod(fx, t, s + 0x24, 0xC);
        fx.mem.set16(t + 0x38, v + size);
        fx.mem.set16(t + 0x34, 0);
        let [r, g, b] = f.smoke_rgb;
        l2::rgb(fx, t + 0x28, r, g, b);
        let r = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, r);
        fx.mem.set16(t + 0x30, 0);
        let v = rmod(fx, t, s + 0x60, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rmod(fx, t, s + 0x7C, 5);
        fx.mem.set16(t + 0xE, v + 2);
        let v = rmod(fx, t, s + 0x9C, 0x23);
        fx.mem.set16(t + 8, v);
        let v = if first { rmod(fx, t, s + 0xB8, 6) + 1 } else { rmod(fx, t, s + 0xB8, 0x19) + 0x28 };
        fx.mem.set16(t + 6, v);
        fx.mem.set16(t + 0x36, 2);
    }
    fade(fx, f, t, 0x81, 0x40, 9, 7);
    if fx.mem.i16(t + 0x32) != 0 {
        let s = fx.mem.i16(t + 0x38);
        if s < 0x3C {
            fx.mem.set16(t + 0x38, s + grow);
        }
    }
    if first {
        let h = fx.target_handle();
        let slot = fx.mem.i16(TNODE);
        fx.node_position(h, slot, sp + 0x30);
        let h = fx.self_handle();
        fx.unit_position(h, sp + 0x40);
        if fx.mem.i16(t + 0x32) != 0 {
            let z = fx.mem.i16(t + 4);
            let dz = if fx.mem.i16(FACING) != 0 { -1 } else { 1 };
            fx.mem.set16(t + 4, z + dz);
        }
    }
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let c = fx.mem.i16(t + 0x34);
        if c < 5 {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
    let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x30, r);
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let s = fx.mem.i16(t + 0x38);
        fx.mem.set16(t + 0x38, s + 2);
    }
    for (o, k) in [(0x10u32, 0u32), (0x14, 0x1C)] {
        let v = rmod(fx, t, s + late + k, 0x168);
        fx.mem.set32(t + o, v);
    }
    let a = fx.mem.i16(t + 0x3A);
    let turn = if fx.mem.i16(t + 0xE) % 2 == 1 { 1 } else { -1 };
    fx.mem.set16(t + 0x3A, a + turn);
    let v = rmod(fx, t, s + late + 0x7C, 0x168);
    fx.mem.set32(t + 0x18, v);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(frame);
}
