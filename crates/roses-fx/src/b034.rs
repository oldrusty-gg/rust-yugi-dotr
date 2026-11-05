//! BMODULE b034 (card 60), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04300, data area 0x1ED0000, 0xAE80 bytes) [F]
//!: b465 / b482 / b771's task family on a pool of
//! 150 tasks of 0x70 bytes at D+0x2A0 (alive +0x58, step +0x44 through
//! `jalr`), spawned by 0x1E05310 (`spawn::task70` on c7a311fb8c14's
//! layout: colour D+0xA904, +0x50 from D+0xAA0A).
//!
//! The emitter 0x1E01030 counts D+0xAA12: the point D+0xA870 at the
//! attacker's node D+0xA95C (y 30); on 1 six hexagon corners D+0xA880 (the
//! angles D+0xA9A8, radius 6 / 12) with the hexagon 0x1E02160 and the ring
//! hexagon 0x1E02650 (both drawn by 0x1E054A0); sparks 0x1E02290 at the
//! attacker 0x1A..0x3B; a glow 0x1E01E70 (lines 0x1E05FF0) on 6, the band
//! ring 0x1E01FA0 on 0x3B; the ring 0x1E02470 / 0x1E03B50 rings (drawn by
//! 0x1E00560) at the node D+0xA95E; motes 0x1E027A0 and spiral ribbons
//! 0x1E02970 (b771's ribbon with three quads a point); at the target from
//! 0x154 bubbles 0x1E03250, flashes 0x1E02F60 / 0x1E03840 / 0x1E03A50; the
//! tint 0x1E03560 (`quads4::tint4`), the frame window 0x1E040D0, the stage
//! light 0x1E03F80 (0x2D) and the object op 0x1E04050 (0xBC). Returns 1 /
//! 3 / 6 on 0x14A / 0x1F9 / 0x155..0x1F3, 8 once the 0x23A-update timer
//! D+0xA900 has run out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::blib::billboard::{self, uv_rect_q};
use crate::blib::{quads4, spawn};
use crate::bus::Bus;
use crate::c061::blend_alpha;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
const POOL: spawn::Pool70 = spawn::Pool70 { base: 0x2A0, n: 150, colour: 0xA904, pos: 0x250, slot: 0xAA0A };
const POS: u32 = D + 0x250;
const POINT: u32 = D + 0xA870;
const CORNERS: u32 = D + 0xA880;
const TIMER: u32 = D + 0xA900;
const OT2: u32 = D + 0xA90C;
const TINT_W: u32 = D + 0xA91C;
const R12: u32 = D + 0xA920;
const R6: u32 = D + 0xA924;
const R2: u32 = D + 0xA928;
const WIDEN: u32 = D + 0xA92C;
const R5: u32 = D + 0xA930;
const SLOT: u32 = D + 0xAA0A;
const NEXT: u32 = D + 0xAA0C;
const FACING: u32 = D + 0xAA0E;
const COUNTER: u32 = D + 0xAA12;
const BYTES: u32 = D + 0xAA14;
const BYTES_B: u32 = D + 0xAB14;
const SINES: u32 = D + 0xAD14;

const GLOW: u32 = 0x01E0_1E70;
const BAND: u32 = 0x01E0_1FA0;
const HEX: u32 = 0x01E0_2160;
const SPARK: u32 = 0x01E0_2290;
const RING: u32 = 0x01E0_2470;
const HEX_RING: u32 = 0x01E0_2650;
const MOTE: u32 = 0x01E0_27A0;
const SPIRAL: u32 = 0x01E0_2970;
const FLASH: u32 = 0x01E0_2F60;
const BUBBLE: u32 = 0x01E0_3250;
const TINT: u32 = 0x01E0_3560;
const BURST: u32 = 0x01E0_3840;
const PILLAR: u32 = 0x01E0_3A50;
const WAVE: u32 = 0x01E0_3B50;
const LIGHT: u32 = 0x01E0_3F80;
const OBJECT: u32 = 0x01E0_4050;
const WINDOW: u32 = 0x01E0_40D0;
/// The return address of the entry's `jalr` into a step.
const TASK_RA: u64 = 0x01E0_49B0;
const SPR: u32 = 0x7000_0000;

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

/// rand % m from a `jal` whose return address is `ra` (rand's frame: s0).
fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn copy3(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn sin(fx: &mut Fx, deg: i32, cos: i32) -> f32 {
    l2::sin_deg(fx, SINES, deg as i16 as i32, cos)
}

/// The hexagon corner `i` (D+0xA880 + 0x10 i).
fn corner(i: i32) -> u32 {
    (CORNERS as i32 + 0x10 * i) as u32
}

/// The entry 0x1E04300 (frame 0x180; s0 = D, s2 = the parameter).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x180);
    fx.mem.write_u32(OT2, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x180);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xAE80);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xAA10, param);
    fx.mem.set32(TIMER, 0x23A);
    for (at, node, clear) in [(0xA95Cu32, 1, 1), (0xA95E, 0x1C, 0), (0xA960, 0xD, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(D + at, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xA96A, r);
    for k in 0xAu32..0x10 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x260 + 4 * k, t);
    }
    let t = fx.texture(4, 3);
    fx.mem.write_u32(D + 0x26C, t);
    for k in 0..3u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0xA8E0 + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xE0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xE4)), 180.0);
    fx.fix_residue(y, 0x01E0_44A4);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_44D4, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 100..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_452C);
        fx.mem.set8(BYTES + k, v & 3);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_4584, 0x12);
        fx.mem.set8(BYTES_B + k, v - 9);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_45D4, 0x1E) - 0xF;
        let a = D + 0xAC14 + k;
        fx.mem.set8(a, v);
        let b = fx.mem.read_u8(a) as i8;
        fx.mem.set8(a, if b <= 0 { -5 } else if b < 5 { 5 } else { -5 });
    }
    for k in 0..0x1Eu32 {
        let v = rmod(fx, D, 0x01E0_4660, 0xF);
        fx.mem.set16(D + 0xA96C + 2 * k, v);
    }
    fx.mem.write_u32(R12, 0x4140_0000);
    fx.mem.write_u32(R2, 0x4000_0000);
    fx.mem.write_u32(WIDEN, 0);
    fx.mem.write_u32(R6, 0x40C0_0000);
    fx.mem.set16(D + 0xAA02, 0);
    fx.mem.write_u32(R5, 0x40A0_0000);
    let halves: [i32; 30] = [
        0x5A, 0x14A, 0xD2, 0x10E, 0x96, 0x1E, // the corner angles
        0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 0, 0, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 3, 3, // the six quads
    ];
    for (k, v) in halves.into_iter().enumerate() {
        fx.mem.set16(D + 0xA9A8 + 2 * k as u32, v);
    }
    // The six spokes: (from, to) corners at D+0xA9E4 + 4 k, start D+0xA934 + 4 k.
    let spokes: [(i32, i32, u32); 6] = [(0, 1, 0), (1, 2, 0x4120_0000), (2, 0, 0x41A0_0000), (3, 4, 0x4120_0000), (4, 5, 0x41A0_0000), (5, 3, 0x41F0_0000)];
    for (k, (a, b, s)) in spokes.into_iter().enumerate() {
        let k = k as u32;
        fx.mem.write_u32(D + 0xA934 + 4 * k, s);
        fx.mem.set16(D + 0xA9E4 + 4 * k, a);
        fx.mem.set16(D + 0xA9E6 + 4 * k, b);
    }
    0
}

/// Stack slots of the draws: the entry's s0 .. s4 at the call.
struct Regs {
    live: u64,
    slot: u64,
    sp: u32,
    k: u64,
}

impl Regs {
    /// s0 .. s4 as (offset, value) from `base` (0x10 apart) and ra at
    /// `ra_at`.
    fn save(&self, fx: &mut Fx, sp: u32, base: u32, ra_at: u32, ra: u32) {
        let v = [u64::from(D), self.live, self.slot, u64::from(self.sp + 0x108), self.k];
        for (i, x) in v.into_iter().enumerate() {
            fx.saved(sp + base + 0x10 * i as u32, &[(0, x)]);
        }
        fx.saved(sp, &[(ra_at, u64::from(ra))]);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(slot + 0x2E4);
        if f == 0 {
            continue;
        }
        run_task(fx, f, slot + 0x2A0, slot, k);
    }
    let mut live = 0u64;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        live = (live as i32 + 1) as i16 as i64 as u64;
        let r = Regs { live, slot: u64::from(slot), sp, k: u64::from(k) };
        let kind = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        if kind(fx) == GLOW {
            lines(fx, t, sp + 0xF0, &r, 0x01E0_4A20);
        }
        if kind(fx) == BAND {
            tex(fx, D + 0xA8E8);
            cell(fx, t, sp + 0x70, 0x14);
            fx.mem.set16(D + 0xB8, 1);
            fx.mem.set16(D + 0xBE, 0);
            bands(fx, t, 0x20, POINT, &r, 0x01E0_4AA0);
        }
        if kind(fx) == HEX {
            hexes(fx, t, 0, POINT, &r, 0x01E0_4ACC);
        }
        if kind(fx) == HEX_RING {
            hexes(fx, t, 1, POINT, &r, 0x01E0_4AFC);
        }
        if kind(fx) == SPARK {
            tex(fx, D + 0xA8E4);
            cell(fx, t, sp + 0x80, 0x14);
            fx.mem.set16(D + 0xB8, 5);
            let v = fx.rand_from(D, 0x01E0_4B68);
            fx.mem.set16(D + 0xBE, v & 3);
            fx.mem.set16(D + 0xB0, 0);
            let mode = if counter(fx) < 0xBE { 0x10 } else { 0x12 };
            billboard::ring_quads(fx, uv_rect_q, D, t, mode, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if kind(fx) == RING {
            for (k, o) in [(0u32, 0u32), (4, 2), (8, 4)] {
                let v = ee::from_i32(fx.mem.i16(t + o));
                fx.mem.setf(sp + 0x60 + k, v);
            }
            ring_flat(fx, t, 0x20, POINT, &r, 0x01E0_4C44);
        }
        if kind(fx) == MOTE {
            tex(fx, D + 0x29C);
            cell(fx, t, sp + 0x90, 0x9B);
            fx.mem.set16(D + 0xB8, 0xD);
            fx.mem.set16(D + 0xB0, 1);
            billboard::ring_quads(fx, uv_rect_q, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if kind(fx) == SPIRAL {
            tex(fx, D + 0x29C);
            cell(fx, t, sp + 0xA0, 0x97);
            let h = fx.mem.i16(D + 0x9A) >> 1;
            fx.mem.set16(D + 0x9A, h);
            let v = fx.mem.i16(D + 0x96) + fx.mem.i16(D + 0x9A);
            fx.mem.set16(D + 0x96, v);
            let h = fx.self_handle();
            unit_position_saving(fx, h, sp + 0x60, u64::from(D), 0x01E0_4D50);
            copy3(fx, D + 0x20, POINT);
            fx.mem.set16(D + 0xBE, 1);
            let base = (D as i32 + 0x280 * fx.mem.i16(t + 0x50)) as u32;
            ribbon(fx, t, base + 0x4470, base + 0x45B0, &r);
        }
        if kind(fx) == FLASH {
            tex(fx, D + 0xA8E8);
            fx.mem.set16(D + 0xB8, 1);
            fx.mem.set16(D + 0xBE, 0);
            fx.mem.set16(D + 0xB0, 1);
            match fx.mem.i16(t + 0xE) {
                0 => bands(fx, t, 0x20, t + 0x10, &r, 0x01E0_4E04),
                1 => bands(fx, t, 0x20, t + 0x10, &r, 0x01E0_4E28),
                _ => {}
            }
            fx.mem.set16(D + 0xB0, 0);
        }
        if kind(fx) == BUBBLE {
            tex(fx, D + 0xA8E0);
            cell(fx, t, sp + 0xB0, 0x14);
            if fx.mem.i16(t + 0xA) == 2 {
                tex(fx, D + 0x29C);
                cell(fx, t, sp + 0xC0, 0x9B);
            }
            fx.mem.set16(D + 0xB8, 6);
            let v = fx.rand_from(D, 0x01E0_4EEC);
            fx.mem.set16(D + 0xBE, v & 3);
            for (k, table) in [(0, BYTES_B), (1, BYTES), (2, BYTES)] {
                if fx.mem.i16(t + 0xA) == k {
                    billboard::ring_quads(fx, uv_rect_q, D, t, 0x20, table);
                }
            }
            fx.mem.set16(D + 0xB8, 0);
        }
        if kind(fx) == TINT {
            let h = fx.target_handle();
            unit_position_saving(fx, h, sp + 0x60, u64::from(D), 0x01E0_4FA8);
            let z = fx.mem.f(sp + 0x68);
            let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 30.0) } else { ee::add(z, 30.0) };
            fx.mem.setf(sp + 0x68, z);
            let p = [fx.mem.f(sp + 0x60), fx.mem.f(sp + 0x64), fx.mem.f(sp + 0x68)];
            let mut x = fx.mem.read_u64(sp + 0x108);
            if let Some(w) = fx.project(p, None, &mut x) {
                fx.mem.write_u64(sp + 0x108, x);
                fx.mem.setf(TINT_W, w);
            }
            crate::b590::project_residue(fx, sp, sp + 0x60);
            let mode = match fx.mem.i16(t + 0x50) {
                0 | 2 => Some(0x20),
                1 => Some(1),
                _ => None,
            };
            if let Some(mode) = mode {
                let w = fx.mem.f(TINT_W);
                quads4::tint4(fx, t, mode, w);
            }
        }
        if kind(fx) == BURST {
            tex(fx, D + 0x29C);
            cell(fx, t, sp + 0xD0, 0x9B);
            fx.mem.set16(D + 0xB8, 5);
            fx.mem.set16(D + 0xBE, 0);
            fx.mem.set16(D + 0xB0, 1);
            billboard::ring_quads(fx, uv_rect_q, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if kind(fx) == PILLAR {
            tex(fx, D + 0x29C);
            fx.mem.set16(D + 0xB8, 1);
            fx.mem.set16(D + 0xBE, 0);
            fx.mem.set16(D + 0xB0, 2);
            bands(fx, t, 0x20, t + 0x10, &r, 0x01E0_5134);
            fx.mem.set16(D + 0xB0, 0);
        }
        if kind(fx) == WAVE {
            copy3(fx, sp + 0x60, POINT);
            let n = fx.mem.i16(t + 0x50);
            if n == 2 {
                let y = ee::sub(fx.mem.f(sp + 0x64), 9.0);
                fx.mem.setf(sp + 0x64, y);
            }
            if n == 0xA || n == 0xB {
                copy4(fx, sp + 0x60, t + 0x10);
            }
            ring_flat(fx, t, 0x20, sp + 0x60, &r, 0x01E0_522C);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0x14A => 1,
        0x1F9 => 3,
        0x155..=0x1F3 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

fn tex(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

/// The atlas cell `id` frame +0x56 (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32, id: i32) {
    let sub = fx.mem.i16(t + 0x56);
    cell_sub(fx, r, id, sub);
}

fn cell_sub(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    copy4(fx, D + 0x94, r);
}

/// The emitter's registers (s0, s1, s3, s4; s2 = D) for its callees'
/// frames.
#[derive(Clone, Copy)]
struct Em {
    s0: u64,
    s1: u64,
    s3: u64,
    s4: u64,
}

/// 0x1E05310(D, step, 0) from the emitter (`ra` its return address).
fn spawn_at(fx: &mut Fx, e: Em, f: u32, ra: u32) {
    let saves = [(0x50u32, u64::from(ra)), (0x40, e.s4), (0x30, e.s3), (0x20, u64::from(D)), (0x10, e.s1), (0, e.s0)];
    spawn::task70(fx, D, POOL, f, 0, &saves);
}

fn node_at(fx: &mut Fx, e: Em, target: bool, slot_at: u32, out: u32, ra: u32) {
    let h = if target { fx.target_handle() } else { fx.self_handle() };
    let s = fx.mem.i16(slot_at);
    node_position_saving(fx, h, s, out, e.s0, Some(e.s1), ra);
}

/// x += rand % m - off (rand's frame: the emitter's s0).
fn jitter(fx: &mut Fx, e: Em, at: u32, ra: u32, m: i32, off: i32) {
    let v = rmod(fx, e.s0 as u32, ra, m) - off;
    let x = ee::add(fx.mem.f(at), ee::from_i32(v));
    fx.mem.setf(at, x);
}

/// 0x1E01030(D, 0) (frame 0x60: ra 0x1E0497C +0x50, the entry's s4 .. s0
/// (runner's [H: 0], runner's [H: 0], the parameter, 0, D)): the update
/// counter, the point and the spawns.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x60);
    let d = u64::from(D);
    fx.saved(sp, &[(0x50, 0x01E0_497C), (0x40, 0), (0x30, 0), (0x20, param as i64 as u64), (0x10, 0), (0, d)]);
    let mut e = Em { s0: d, s1: 0, s3: 0, s4: 0 };
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    if counter(fx) == 0x2D {
        spawn_at(fx, e, LIGHT, 0x01E0_1090);
    }
    if counter(fx) == 0xBC {
        spawn_at(fx, e, OBJECT, 0x01E0_10BC);
    }
    if counter(fx) == 0x154 {
        fx.mem.set16(SLOT, 0);
        spawn_at(fx, e, TINT, 0x01E0_10F4);
        fx.mem.set16(SLOT, 1);
        spawn_at(fx, e, TINT, 0x01E0_1118);
        spawn_at(fx, e, WINDOW, 0x01E0_112C);
    }
    if counter(fx) == 0x1F7 {
        fx.mem.set16(SLOT, 2);
        spawn_at(fx, e, TINT, 0x01E0_1168);
    }
    node_at(fx, e, false, D + 0xA95C, POINT, 0x01E0_118C);
    fx.mem.write_u32(POINT + 4, 0x41F0_0000);
    if counter(fx) == 1 {
        for k in 0..6u32 {
            e = Em { s0: d + 2 * u64::from(k), s1: d + 0x10 * u64::from(k), s3: u64::from(k), s4: u64::from(k) };
            node_at(fx, e, false, D + 0xA95C, POS, 0x01E0_11E4);
            let a = fx.mem.i16(D + 0xA9A8 + 2 * k);
            let s = sin(fx, a, 0);
            let r1 = if k >= 3 { fx.mem.f(R12) } else { ee::div(fx.mem.f(R12), 2.0) };
            fx.mem.setf(POS + 8, ee::mul(r1, s));
            let a = fx.mem.i16(D + 0xA9A8 + 2 * k);
            let c = sin(fx, a, 1);
            let r2 = if k >= 3 { fx.mem.f(R12) } else { ee::div(fx.mem.f(R12), 2.0) };
            fx.mem.setf(POS, ee::mul(r2, c));
            fx.mem.write_u32(POS + 4, 0);
            copy4(fx, corner(k as i32), POS);
        }
        e = Em { s0: d + 12, s1: d + 0x60, s3: 6, s4: 6 };
        spawn_at(fx, e, HEX, 0x01E0_1338);
        spawn_at(fx, e, HEX_RING, 0x01E0_134C);
    }
    let h = fx.self_handle();
    unit_position_saving(fx, h, POS, e.s0, 0x01E0_1360);
    if counter(fx) == 6 {
        spawn_at(fx, e, GLOW, 0x01E0_138C);
    }
    if counter(fx) == 0x3B {
        spawn_at(fx, e, BAND, 0x01E0_13B8);
    }
    if counter(fx) >= 0x3C {
        let v = fx.mem.i16(D + 0xAA00);
        if v < 0x168 {
            fx.mem.set16(D + 0xAA00, v + 0xA);
        }
        if fx.mem.i16(D + 0xAA00) >= 0x169 {
            fx.mem.set16(D + 0xAA00, 0x168);
        }
    }
    let c = counter(fx);
    if (0x1A..0x3C).contains(&c) {
        fx.mem.write_u32(POS + 4, 0x41E0_0000);
        if counter(fx).wrapping_rem(3) != 1 {
            spawn_at(fx, e, SPARK, 0x01E0_148C);
        }
    }
    fx.mem.write_u32(POS + 4, 0x41F0_0000);
    if counter(fx) == 0x4C {
        spawn_at(fx, e, RING, 0x01E0_14C0);
        fx.mem.set16(SLOT, 0);
        spawn_at(fx, e, WAVE, 0x01E0_14E0);
    }
    if counter(fx) == 0x6E {
        fx.mem.set16(SLOT, 0);
        spawn_at(fx, e, WAVE, 0x01E0_1518);
        fx.mem.set16(SLOT, 1);
        spawn_at(fx, e, WAVE, 0x01E0_153C);
    }
    if counter(fx) == 0xC8 {
        fx.mem.set16(SLOT, 0);
        spawn_at(fx, e, WAVE, 0x01E0_1574);
        fx.mem.set16(SLOT, 2);
        spawn_at(fx, e, WAVE, 0x01E0_1598);
    }
    node_at(fx, e, false, D + 0xA95E, POS, 0x01E0_15B8);
    let c = counter(fx);
    if (0xC8..0x1D7).contains(&c) {
        fx.mem.set16(SLOT, 0);
        if counter(fx).wrapping_rem(0x1B) == 1 {
            spawn_at(fx, e, WAVE, 0x01E0_1628);
        }
        if counter(fx).wrapping_rem(0xA) == 1 {
            fx.mem.write_u32(POS + 4, 0x41E0_0000);
            spawn_at(fx, e, RING, 0x01E0_1670);
        }
    }
    let c = counter(fx);
    if (0x30..0x1C9).contains(&c) {
        fx.mem.set16(SLOT, 0);
        spawn_at(fx, e, MOTE, 0x01E0_16B4);
        spawn_at(fx, e, MOTE, 0x01E0_16C8);
    }
    if counter(fx) == 0x6E {
        copy4(fx, POS, POINT);
        fx.mem.set16(SLOT, 1);
        for k in 0..0xCu64 {
            e.s0 = k;
            spawn_at(fx, e, BUBBLE, 0x01E0_1734);
        }
        e.s0 = 0xC;
    }
    if counter(fx) == 0x154 {
        fx.mem.set16(SLOT, 0);
        node_at(fx, e, true, D + 0xA96A, POS, 0x01E0_1798);
        for k in 0..7u64 {
            e.s0 = k;
            spawn_at(fx, e, FLASH, 0x01E0_17B4);
        }
        e.s0 = 7;
        fx.mem.set16(SLOT, 0xA);
        spawn_at(fx, e, WAVE, 0x01E0_17F8);
    }
    if counter(fx) == 0x154 {
        node_at(fx, e, true, D + 0xA96A, POS, 0x01E0_1830);
        fx.mem.set16(SLOT, 0);
        for k in 0..0xAu64 {
            e.s0 = k;
            spawn_at(fx, e, BUBBLE, 0x01E0_1858);
        }
        e.s0 = 0xA;
    }
    let c = counter(fx);
    if (0x156..0x1DC).contains(&c) {
        node_at(fx, e, true, D + 0xA96A, POS, 0x01E0_18C4);
        jitter(fx, e, POS, 0x01E0_18CC, 0x28, 0x14);
        jitter(fx, e, POS + 8, 0x01E0_1900, 0x28, 0x14);
        fx.mem.set16(SLOT, 1);
        fx.mem.write_u32(POS + 4, 0x41A0_0000);
        if counter(fx).wrapping_rem(2) == 1 {
            spawn_at(fx, e, FLASH, 0x01E0_1980);
        }
    }
    let c = counter(fx);
    if (0x156..0x1E6).contains(&c) {
        node_at(fx, e, true, D + 0xA96A, POS, 0x01E0_19C4);
        spawn_at(fx, e, BURST, 0x01E0_19D8);
        jitter(fx, e, POS + 4, 0x01E0_19E0, 0x3C, 0x2D);
        jitter(fx, e, POS, 0x01E0_1A14, 0x32, 0x19);
        jitter(fx, e, POS + 8, 0x01E0_1A48, 0x32, 0x19);
        fx.mem.set16(SLOT, 2);
        if counter(fx).wrapping_rem(2) == 0 {
            spawn_at(fx, e, BUBBLE, 0x01E0_1ABC);
        }
    }
    let c = counter(fx);
    if (0x156..0x1DC).contains(&c) {
        node_at(fx, e, true, D + 0xA96A, POS, 0x01E0_1B00);
        fx.mem.set16(SLOT, 2);
        fx.mem.write_u32(POS + 4, 0x41A0_0000);
        spawn_at(fx, e, PILLAR, 0x01E0_1B2C);
    }
    if counter(fx) == 0x168 {
        node_at(fx, e, true, D + 0xA96A, POS, 0x01E0_1B64);
        fx.mem.set16(SLOT, 0xB);
        spawn_at(fx, e, WAVE, 0x01E0_1B88);
    }
    let c = counter(fx);
    if (0x5F..0x12D).contains(&c) && c.wrapping_rem(2) == 1 {
        for ra in [0x01E0_1BF8u32, 0x01E0_1C64] {
            let i = fx.mem.i16(NEXT);
            fx.mem.set16(SLOT, i);
            spawn_at(fx, e, SPIRAL, ra);
            let i = fx.mem.i16(NEXT) + 1;
            fx.mem.set16(NEXT, i);
            if fx.mem.i16(NEXT) >= 0x28 {
                fx.mem.set16(NEXT, 0);
            }
        }
    }
    if counter(fx) >= 0xB4 {
        let v = fx.mem.i16(D + 0xAA06) + 0xA;
        fx.mem.set16(D + 0xAA06, v);
        if fx.mem.i16(D + 0xAA06) >= 0x12D {
            fx.mem.set16(D + 0xAA06, 0x12C);
        }
        let v = ee::add(fx.mem.f(R5), 0.5);
        fx.mem.setf(R5, v);
        if !(v <= 20.0) {
            fx.mem.write_u32(R5, 0x41A0_0000);
        }
    }
    if counter(fx) >= 0x3B {
        let v = fx.mem.i16(D + 0xAA04) + 2;
        fx.mem.set16(D + 0xAA04, v);
        if fx.mem.i16(D + 0xAA04) >= 0x49 {
            fx.mem.set16(D + 0xAA04, 0x48);
        }
    }
    if counter(fx) >= 0x45 {
        let v = fx.mem.i16(D + 0xAA02) + 1;
        fx.mem.set16(D + 0xAA02, v);
        if fx.mem.i16(D + 0xAA02) >= 0x29 {
            fx.mem.set16(D + 0xAA02, 0x28);
            let v = ee::add(fx.mem.f(WIDEN), f32::from_bits(0x3E4C_CCCD));
            fx.mem.setf(WIDEN, v);
            if !(v <= 2.0) {
                fx.mem.write_u32(WIDEN, 0x4000_0000);
            }
        }
    }
    fx.leave(0x60);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn odd(v: i32) -> bool {
    v.wrapping_rem(2) == 1
}

/// Byte `a` + `up` while below `cap`.
fn rise(fx: &mut Fx, t: u32, a: u32, cap: u8, up: u8) {
    let v = fx.mem.read_u8(t + a);
    if v < cap {
        fx.mem.write_u8(t + a, v.wrapping_add(up));
    }
}

/// Byte `a` - `down` from `floor` up, else the task ends.
fn fall(fx: &mut Fx, t: u32, a: u32, floor: u8, down: u8) {
    let v = fx.mem.read_u8(t + a);
    if v >= floor {
        fx.mem.write_u8(t + a, v.wrapping_sub(down));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// Byte `a` - `down` from `floor` up, else 0.
fn fall_to0(fx: &mut Fx, t: u32, a: u32, floor: u8, down: u8) {
    let v = fx.mem.read_u8(t + a);
    if v >= floor {
        fx.mem.write_u8(t + a, v.wrapping_sub(down));
    } else {
        fx.mem.write_u8(t + a, 0);
    }
}

fn first(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x58) == 1
}

fn rgb(fx: &mut Fx, t: u32, c: u32, v: (i32, i32, i32)) {
    l2::rgb(fx, t + c, v.0, v.1, v.2);
}

/// The colours by the age's parity: `odd_c` on odd ages, else `even_c`
/// (each a list of (offset, colour)).
fn flicker(fx: &mut Fx, t: u32, odd_c: &[(u32, (i32, i32, i32))], even_c: &[(u32, (i32, i32, i32))]) {
    let set = if odd(fx.mem.i16(t + 0x54)) { odd_c } else { even_c };
    for &(c, v) in set {
        rgb(fx, t, c, v);
    }
}

fn timer(fx: &mut Fx) -> i32 {
    fx.mem.i32(TIMER)
}

/// A step's frame with its saves: `slots` (offset, register) and ra at
/// `ra_at` (the entry's `jalr`).
fn enter_step(fx: &mut Fx, frame: u32, ra_at: u32, slots: &[(u32, u64)]) -> u32 {
    let sp = fx.enter(frame);
    fx.saved(sp, slots);
    fx.saved(sp, &[(ra_at, TASK_RA)]);
    sp
}

/// The step `f` on the task `t` (the entry's s1 = 0, s2 = `slot`, s3 = `k`
/// at the `jalr`).
fn run_task(fx: &mut Fx, f: u32, t: u32, slot: u32, k: u32) {
    let d = u64::from(D);
    match f {
        GLOW | BAND | WAVE => {
            enter_step(fx, 0x30, 0x20, &[(0x10, 0), (0, d)]);
            match f {
                GLOW => glow(fx, t),
                BAND => band(fx, t),
                _ => wave(fx, t),
            }
            fx.leave(0x30);
        }
        HEX | SPARK | RING | HEX_RING | MOTE | FLASH | BUBBLE | TINT | BURST | PILLAR => {
            enter_step(fx, 0x20, 0x10, &[(0, d)]);
            match f {
                HEX => hex(fx, t),
                SPARK => spark(fx, t),
                RING => ring(fx, t),
                HEX_RING => hex_ring(fx, t),
                MOTE => mote(fx, t),
                FLASH => flash(fx, t),
                BUBBLE => bubble(fx, t),
                TINT => tint(fx, t),
                BURST => burst(fx, t),
                _ => pillar(fx, t),
            }
            fx.leave(0x20);
        }
        SPIRAL => {
            let sp = enter_step(fx, 0x70, 0x50, &[(0x40, u64::from(k)), (0x30, u64::from(slot)), (0x20, 0), (0x10, d)]);
            // swc1 f21 / f20: the runner's (0) [H].
            fx.mem.write_u32(sp + 4, 0);
            fx.mem.write_u32(sp, 0);
            spiral(fx, t, sp);
            fx.leave(0x70);
        }
        LIGHT => {
            enter_step(fx, 0x10, 0, &[]);
            light(fx, t);
            fx.leave(0x10);
        }
        OBJECT => {
            let sp = enter_step(fx, 0x30, 0x10, &[(0, d)]);
            object(fx, t, sp);
            fx.leave(0x30);
        }
        WINDOW => {
            let sp = enter_step(fx, 0x90, 0x20, &[(0x10, 0), (0, d)]);
            window(fx, t, sp);
            fx.leave(0x90);
        }
        _ => panic!("b034: unknown task step {f:#x}"),
    }
}

/// 0x1E01E70 (D, t; frame 0x30): the glow's colour (pale yellow, cell 10),
/// in by 12 below 0x80 for 0x14 updates, out by 9 once the timer is below
/// 0x28; flickering.
fn glow(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        rgb(fx, t, 0x60, (0xFF, 0xFF, 0x80));
        fx.mem.set16(t + 0x56, 0xA);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x14 {
        rise(fx, t, 0x63, 0x80, 0xC);
    } else if timer(fx) < 0x28 {
        fall(fx, t, 0x63, 0xB, 9);
    }
    flicker(fx, t, &[(0x60, (0xFF, 0xFF, 0x80))], &[(0x60, (0x80, 0x80, 0x40))]);
    age(fx, t);
}

/// 0x1E01FA0 (D, t; frame 0x30, s0 = t, s1 = D): the band ring (cell rand
/// % 7, row rand % 3 + 4), both alphas in by 12 below 0x80 for 0x14
/// updates, out by 9 once the timer is below 0x28; facing the camera with
/// no yaw.
fn band(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0);
        let v = rmod(fx, t, 0x01E0_1FCC, 7);
        fx.mem.set16(t + 0x56, v);
        fx.mem.set16(t + 0x58, 2);
        let v = rmod(fx, t, 0x01E0_1FF0, 3);
        fx.mem.set16(t + 8, v + 4);
    }
    if fx.mem.i16(t + 0x54) < 0x14 {
        rise(fx, t, 0x63, 0x80, 0xC);
        rise(fx, t, 0x67, 0x80, 0xC);
    } else if timer(fx) < 0x28 {
        fall(fx, t, 0x63, 0xB, 9);
        fall_to0(fx, t, 0x67, 0xB, 9);
    }
    flicker(
        fx,
        t,
        &[(0x60, (0x40, 0x40, 0x10)), (0x64, (0x40, 0x40, 0x10))],
        &[(0x60, (0x80, 0x80, 0x20)), (0x64, (0x80, 0x80, 0x20))],
    );
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2130);
    fx.mem.write_u32(t + 0x20, 0);
    age(fx, t);
}

const HEX_COLOURS: ([(u32, (i32, i32, i32)); 2], [(u32, (i32, i32, i32)); 2]) =
    ([(0x60, (0x80, 0x80, 0x80)), (0x64, (0x80, 0x20, 0x20))], [(0x60, (0x40, 0x40, 0x40)), (0x64, (0xC0, 0x10, 0x10))]);

/// 0x1E02160 (D, t; frame 0x20): the hexagon, in by 0x10 below 0xA0 for 10
/// updates, out by 13 once the timer is below 0x28; flickering red.
fn hex(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x56, 0);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0xA {
        rise(fx, t, 0x63, 0xA0, 0x10);
    }
    if timer(fx) < 0x28 {
        fall(fx, t, 0x63, 0xF, 0xD);
    }
    flicker(fx, t, &HEX_COLOURS.0, &HEX_COLOURS.1);
    age(fx, t);
}

/// 0x1E02650 (D, t; frame 0x20): the ring hexagon, in by 0x10 below 0xA0
/// while the update is below 0xC8 and the spokes D+0xAA00 are whole, out
/// by 13 once the timer is below 0x28.
fn hex_ring(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x56, 0);
        fx.mem.set16(t + 0x58, 2);
    }
    if counter(fx) < 0xC8 && fx.mem.i16(D + 0xAA00) >= 0x168 {
        rise(fx, t, 0x63, 0xA0, 0x10);
    }
    if timer(fx) < 0x28 {
        fall(fx, t, 0x63, 0xF, 0xD);
    }
    flicker(fx, t, &HEX_COLOURS.0, &HEX_COLOURS.1);
    age(fx, t);
}

/// 0x1E02290 (D, t; frame 0x20): a spark ring (size rand % 30 + 100 growing
/// 6 to age 0x8C, cell rand & 7, radius rand % 50 + 100 growing rand % 20 +
/// 0x1A to age 0x5A, life rand % 14 + 7).
fn spark(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        let v = rmod(fx, t, 0x01E0_22B4, 0x1E);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0x64));
        let v = fx.rand_from(t, 0x01E0_22E0);
        fx.mem.set16(t + 0x56, v & 7);
        rgb(fx, t, 0x60, (0xA0, 0xA0, 0xA0));
        let v = rmod(fx, t, 0x01E0_2314, 0x32);
        fx.mem.set16(t + 0x50, v + 0x64);
        let v = rmod(fx, t, 0x01E0_2334, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_2350, 0x14);
        fx.mem.set16(t + 0xE, v + 0x1A);
        let v = rmod(fx, t, 0x01E0_2370, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_238C, 0xE);
        fx.mem.set16(t + 6, v + 7);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        rise(fx, t, 0x63, 0x80, 0x2A);
    } else {
        fall(fx, t, 0x63, 0xB, 9);
    }
    let a = fx.mem.i16(t + 0x54);
    if a < 0x5A && a != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    if fx.mem.i16(t + 0x54) < 0x8C {
        let s = ee::add(fx.mem.f(t + 0x48), 6.0);
        fx.mem.setf(t + 0x48, s);
    }
    age(fx, t);
}

/// 0x1E02470 (D, t; frame 0x20): a flat ring (inner +0x48 from 20 growing
/// 10 a update, outer +6 = inner - 45), blue (+0x50 2: teal over dark);
/// +0x67 in by 12 below 0x80 for +8 updates (0x12 / 0x1E), out by 9 (7).
fn ring(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        if fx.mem.i16(t + 0x50) == 2 {
            rgb(fx, t, 0x60, (0x10, 0x80, 0x80));
            rgb(fx, t, 0x64, (0x10, 0x10, 0x10));
        } else {
            rgb(fx, t, 0x60, (0x10, 0x10, 0xF0));
            rgb(fx, t, 0x64, (0xF0, 0xF0, 0xF0));
        }
        let k = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xE, k);
        let v = rmod(fx, t, 0x01E0_2508, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x63, 0);
        fx.mem.set16(t + 6, 0);
        let n = if fx.mem.i16(t + 0x50) == 2 { 0x1E } else { 0x12 };
        fx.mem.set16(t + 8, n);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 8) {
        rise(fx, t, 0x67, 0x80, 0xC);
    } else if fx.mem.i16(t + 0x50) != 2 {
        fall(fx, t, 0x67, 0xB, 9);
    } else {
        fall(fx, t, 0x67, 9, 7);
    }
    if fx.mem.i16(t + 0xE) != 2 {
        let x = ee::sub(fx.mem.f(t + 0x48), 45.0);
        fx.fix_residue(x, 0x01E0_25F8);
        fx.mem.set16(t + 6, ee::fixsfsi(x));
    }
    if fx.mem.i16(t + 6) < 0 {
        fx.mem.set16(t + 6, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = ee::add(fx.mem.f(t + 0x48), 10.0);
        fx.mem.setf(t + 0x48, r);
    }
    age(fx, t);
}

/// 0x1E027A0 (D, t; frame 0x20): a mote ring (size rand & 7 + 1, cell rand
/// & 3, orange, random angles, radius from 0 growing (rand & 15) + 3, row
/// rand & 7 + 1, life rand % 3 + 1 at 0x80, then out by 15).
fn mote(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        let v = fx.rand_from(t, 0x01E0_27C4);
        fx.mem.setf(t + 0x48, ee::from_i32((v & 7) + 1));
        let v = fx.rand_from(t, 0x01E0_27F0);
        fx.mem.set16(t + 0x56, v & 3);
        rgb(fx, t, 0x60, (0x80, 0x40, 0));
        for (o, ra) in [(0x20u32, 0x01E0_2824u32), (0x24, 0x01E0_2840), (0x28, 0x01E0_285C)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 0x50, 0);
        let v = rmod(fx, t, 0x01E0_287C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, 0x01E0_2898);
        fx.mem.set16(t + 0xE, (v & 0xF) + 3);
        let v = fx.rand_from(t, 0x01E0_28B8);
        fx.mem.set16(t + 8, (v & 7) + 1);
        let v = rmod(fx, t, 0x01E0_28D8, 3);
        fx.mem.set16(t + 6, v + 1);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fall(fx, t, 0x63, 0x11, 0xF);
    }
    let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, r);
    age(fx, t);
}

/// The radius D+0xA920 (+ D+0xA924 when the stack word sp + 0x60 is 0).
fn spiral_r(fx: &mut Fx, sp: u32) -> f32 {
    if fx.mem.f(sp + 0x60) == 0.0 {
        ee::add(fx.mem.f(R12), fx.mem.f(R6))
    } else {
        fx.mem.f(R12)
    }
}

/// The outer radius: [`spiral_r`]'s + D+0xA930.
fn spiral_r2(fx: &mut Fx, sp: u32) -> f32 {
    if fx.mem.f(sp + 0x60) == 0.0 {
        ee::add(fx.mem.f(R5), ee::add(fx.mem.f(R12), fx.mem.f(R6)))
    } else {
        ee::add(fx.mem.f(R12), fx.mem.f(R5))
    }
}

/// sp + 0x68 = r sin, sp + 0x60 = r cos of `deg`.
fn spiral_point(fx: &mut Fx, sp: u32, r: f32, deg: i32) {
    let s = sin(fx, deg, 0);
    fx.mem.setf(sp + 0x68, ee::mul(r, s));
    let c = sin(fx, deg, 1);
    fx.mem.setf(sp + 0x60, ee::mul(r, c));
}

/// 0x1E02970 (D, t; frame 0x70, s3 = D, s2 = t): a spiral ribbon pair
/// about the point (D+0x20 at draw time): the angle +0x5A (rand % 360)
/// turning 12 a update, radius D+0xA920 and D+0xA920 + D+0xA930 30 degrees
/// behind and +0xA / 10 higher (+0xA = D+0xAA06 + rand % 50 + 5); 20-point
/// histories filled at the start, n = +6 points (2, up to 0x12); green
/// flickering, in by 0x15 below 0x80 for 10 updates, then out by 9. The
/// radii read the stale stack word sp + 0x60 (0: + D+0xA924).
fn spiral(fx: &mut Fx, t: u32, sp: u32) {
    let base = (D as i32 + 0x280 * fx.mem.i16(t + 0x50)) as u32;
    let (ha, hb) = (base + 0x4470, base + 0x45B0);
    if first(fx, t) {
        let v = rmod(fx, D, 0x01E0_29AC, 0x32);
        let h = fx.mem.i16(D + 0xAA06) + v + 5;
        fx.mem.set16(t + 0xA, h);
        let f20 = ee::from_i32(h as i16 as i32);
        let v = fx.rand_from(D, 0x01E0_29E4);
        let f20 = ee::div(f20, 10.0);
        fx.mem.set16(t + 8, v & 1);
        rgb(fx, t, 0x64, (0x70, 0x70, 0x70));
        let s0 = sp + 0x64;
        fx.mem.write_u32(s0, 0);
        let v = rmod(fx, s0, 0x01E0_2A28, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let r = spiral_r(fx, sp);
        let a = fx.mem.i16(t + 0x5A);
        spiral_point(fx, sp, r, a);
        for k in 0..0x13u32 {
            copy3(fx, ha + 0x10 * k, sp + 0x60);
        }
        let r = spiral_r2(fx, sp);
        let a = fx.mem.i16(t + 0x5A) - 0x1E;
        spiral_point(fx, sp, r, a);
        fx.mem.setf(sp + 0x64, f20);
        for k in 0..0x13u32 {
            copy3(fx, hb + 0x10 * k, sp + 0x60);
        }
        fx.mem.set16(t + 6, 2);
        fx.mem.write_u8(t + 0x63, 0);
        fx.mem.set16(t + 8, 0);
        fx.mem.set16(t + 0x56, 0);
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0xA {
        rise(fx, t, 0x63, 0x80, 0x15);
    } else {
        fall(fx, t, 0x63, 0xB, 9);
    }
    let f20 = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    let a = fx.mem.i16(t + 0x5A) + 0xC;
    fx.mem.set16(t + 0x5A, a);
    let a = fx.mem.i16(t + 0x5A);
    if a >= 0x168 {
        fx.mem.set16(t + 0x5A, a % 0x168);
    }
    let y = ee::from_i32(fx.mem.i16(t + 8));
    fx.mem.setf(sp + 0x64, y);
    let r = spiral_r(fx, sp);
    let a = fx.mem.i16(t + 0x5A);
    spiral_point(fx, sp, r, a);
    spawn::history(fx, ha + 0x130, 0x14, sp + 0x60);
    let y = ee::add(f20, ee::from_i32(fx.mem.i16(t + 8)));
    fx.mem.setf(sp + 0x64, y);
    let r = spiral_r2(fx, sp);
    let a = fx.mem.i16(t + 0x5A) - 0x1E;
    spiral_point(fx, sp, r, a);
    spawn::history(fx, hb + 0x130, 0x14, sp + 0x60);
    flicker(fx, t, &[(0x60, (0x50, 0x10, 0x20))], &[(0x60, (0x40, 8, 0x10))]);
    if fx.mem.i16(t + 0x54) != 0 {
        let n = fx.mem.i16(t + 6);
        if n < 0x12 {
            fx.mem.set16(t + 6, n + 1);
        }
    }
    age(fx, t);
}

/// 0x1E02F60 (D, t; frame 0x20): a flash band (+0xE = +0x50: kind 0 a
/// widening disc 4 / 0.4, kind 1 a rising ring 20 turning 0x100 a
/// update); in by 0x40 below 0x80 for +6 updates (10 / 0x1A), out by 4.
fn flash(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        let k = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xE, k);
        match fx.mem.i16(t + 0xE) {
            0 => {
                fx.mem.write_u32(t + 0x48, 0x4080_0000);
                fx.mem.write_u32(t + 0x4C, 0x3ECC_CCCD);
            }
            1 => {
                fx.mem.write_u32(t + 0x48, 0x41A0_0000);
                fx.mem.write_u32(t + 0x4C, 0);
            }
            _ => {}
        }
        rgb(fx, t, 0x60, (0x80, 0x80, 0x80));
        rgb(fx, t, 0x64, (0x80, 0x40, 0x10));
        for (o, ra) in [(0x20u32, 0x01E0_2FECu32), (0x24, 0x01E0_3008), (0x28, 0x01E0_3024)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        if fx.mem.i16(t + 0xE) == 0 {
            fx.mem.set16(t + 6, 0xA);
        }
        if fx.mem.i16(t + 0xE) == 1 {
            fx.mem.set16(t + 6, 0x1A);
        }
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        rise(fx, t, 0x63, 0x80, 0x40);
    } else {
        fall(fx, t, 0x63, 6, 4);
    }
    match fx.mem.i16(t + 0xE) {
        0 => {
            let r = ee::add(fx.mem.f(t + 0x48), f32::from_bits(0x401C_CCCD));
            fx.mem.setf(t + 0x48, r);
            if fx.mem.i16(t + 0x54) != 0 {
                let w = fx.mem.f(t + 0x4C);
                if w < 15.0 {
                    fx.mem.setf(t + 0x4C, ee::add(w, 0.25));
                }
            }
        }
        1 => {
            let r = ee::add(fx.mem.f(t + 0x48), f32::from_bits(0x3E19_999A));
            fx.mem.setf(t + 0x48, r);
            fx.mem.setf(t + 0x4C, ee::sub(r, 7.0));
            if fx.mem.i16(t + 0x54) != 0 {
                let y = ee::add(fx.mem.f(t + 0x14), f32::from_bits(0x3F99_999A));
                fx.mem.setf(t + 0x14, y);
            }
            for o in [0x24u32, 0x28] {
                let v = fx.mem.read_u32(t + o).wrapping_add(0x100);
                fx.mem.write_u32(t + o, v);
            }
            flicker(
                fx,
                t,
                &[(0x60, (0x80, 0x80, 0x80)), (0x64, (0x80, 0x40, 0x10))],
                &[(0x60, (0xC0, 0xC0, 0xC0)), (0x64, (0xA0, 0x60, 0x30))],
            );
        }
        _ => {}
    }
    age(fx, t);
}

/// 0x1E03250 (D, t; frame 0x20): a bubble ring (size 15 / 20 / 20 by
/// +0x50, growth rand % 6 + 5 / rand % 10 + 10 / rand % 150 + 150, cell
/// rand & 3, grey (kind 2 dark); +0xA the kind, radius rand % 30, row
/// rand % 35, life rand % 10 + 15 (rand % 3 + 4 when the radius is 2));
/// in by 0x2A below 0x80, then out by 5 (kind 2: 8); kind 2 rises 1 and
/// grows 1.8 a update, the others widen by the growth.
fn bubble(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0x4170_0000);
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        }
        if fx.mem.i16(t + 0x50) == 2 {
            fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        }
        for (k, ra, m, add) in [(0, 0x01E0_32B4u32, 6, 5), (1, 0x01E0_32E8, 0xA, 0xA), (2, 0x01E0_331C, 0x96, 0x96)] {
            if fx.mem.i16(t + 0x50) == k {
                let v = rmod(fx, t, ra, m);
                fx.mem.set16(t + 0xE, v + add);
            }
        }
        let v = fx.rand_from(t, 0x01E0_3340);
        fx.mem.set16(t + 0x56, v & 3);
        rgb(fx, t, 0x60, (0x80, 0x80, 0x80));
        if fx.mem.i16(t + 0x50) == 2 {
            rgb(fx, t, 0x60, (0x10, 0x10, 0x10));
        }
        let k = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xA, k);
        let v = rmod(fx, t, 0x01E0_339C, 0x1E);
        fx.mem.set16(t + 0x50, v);
        let v = rmod(fx, t, 0x01E0_33B8, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_33D4, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_33F0, 0xA);
        fx.mem.set16(t + 6, v + 0xF);
        if fx.mem.i16(t + 0x50) == 2 {
            let v = rmod(fx, t, 0x01E0_3424, 3);
            fx.mem.set16(t + 6, v + 4);
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        rise(fx, t, 0x63, 0x80, 0x2A);
    } else if fx.mem.i16(t + 0xA) != 2 {
        fall(fx, t, 0x63, 7, 5);
    } else {
        fall(fx, t, 0x63, 0xA, 8);
    }
    if fx.mem.i16(t + 0xA) != 2 && fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    if fx.mem.i16(t + 0xA) == 2 {
        let y = ee::add(fx.mem.f(t + 0x14), 1.0);
        fx.mem.setf(t + 0x14, y);
        let s = ee::add(fx.mem.f(t + 0x48), f32::from_bits(0x3FE6_6666));
        fx.mem.setf(t + 0x48, s);
    }
    age(fx, t);
}

/// 0x1E03560 (D, t; frame 0x20): the tint's colours by +0x50 (0: yellow
/// over grey, 70 updates; 1: magenta, 0xA0; 2: white, 0x10) and fades.
fn tint(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        if fx.mem.i16(t + 0x50) == 0 {
            rgb(fx, t, 0x60, (0xD0, 0xD0, 0x80));
            rgb(fx, t, 0x64, (0xC0, 0xC0, 0x80));
        }
        if fx.mem.i16(t + 0x50) == 1 {
            rgb(fx, t, 0x60, (0xFF, 0, 0xFF));
            rgb(fx, t, 0x64, (0xFF, 0, 0xFF));
        }
        if fx.mem.i16(t + 0x50) == 2 {
            rgb(fx, t, 0x60, (0xF0, 0xF0, 0xF0));
            rgb(fx, t, 0x64, (0xF0, 0xF0, 0xF0));
        }
        for (k, n) in [(0, 0x46), (1, 0xA0), (2, 0x10)] {
            if fx.mem.i16(t + 0x50) == k {
                fx.mem.set16(t + 6, n);
            }
        }
        fx.mem.set16(t + 0x58, 2);
    }
    let a = fx.mem.i16(t + 0x54);
    if a < fx.mem.i16(t + 6) {
        match fx.mem.i16(t + 0x50) {
            0 => {
                if a < 0x2D {
                    rise(fx, t, 0x63, 0x80, 0x20);
                    rise(fx, t, 0x67, 0x80, 0x20);
                } else if a < 0x32 {
                    for o in [0x63u32, 0x67] {
                        let v = fx.mem.read_u8(t + o);
                        if v >= 0x61 {
                            fx.mem.write_u8(t + o, v - 0xC);
                        }
                    }
                }
            }
            1 => {
                rise(fx, t, 0x63, 0x60, 0x18);
                rise(fx, t, 0x67, 0x60, 0x18);
            }
            2 => {
                rise(fx, t, 0x63, 0x80, 0x12);
                rise(fx, t, 0x67, 0x80, 0x12);
            }
            _ => {}
        }
    } else if fx.mem.i16(t + 0x50) != 2 {
        fall(fx, t, 0x63, 9, 7);
        fall_to0(fx, t, 0x67, 9, 7);
    } else {
        fall(fx, t, 0x63, 6, 4);
        fall_to0(fx, t, 0x67, 6, 4);
    }
    age(fx, t);
}

/// 0x1E03840 (D, t; frame 0x20): a burst ring (size rand % 40 + 30, cell
/// rand & 3, violet or orange by rand & 1, random angles, radius from 0
/// growing (rand & 15) + 0x23, row rand & 7 + 1, life rand % 3 + 13 at 0x80,
/// then out by 15).
fn burst(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        let v = rmod(fx, t, 0x01E0_3864, 0x28);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0x1E));
        let v = fx.rand_from(t, 0x01E0_3890);
        fx.mem.set16(t + 0x56, v & 3);
        let v = fx.rand_from(t, 0x01E0_38AC);
        if v & 1 == 0 {
            rgb(fx, t, 0x60, (0x40, 0x10, 0x80));
        } else {
            rgb(fx, t, 0x60, (0x80, 0x40, 0x10));
        }
        for (o, ra) in [(0x20u32, 0x01E0_3900u32), (0x24, 0x01E0_391C), (0x28, 0x01E0_3938)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 0x50, 0);
        let v = rmod(fx, t, 0x01E0_3958, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, 0x01E0_3974);
        fx.mem.set16(t + 0xE, (v & 0xF) + 0x23);
        let v = fx.rand_from(t, 0x01E0_3994);
        fx.mem.set16(t + 8, (v & 7) + 1);
        let v = rmod(fx, t, 0x01E0_39B4, 3);
        fx.mem.set16(t + 6, v + 0xD);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fall(fx, t, 0x63, 0x11, 0xF);
    }
    let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, r);
    age(fx, t);
}

/// 0x1E03A50 (D, t; frame 0x20): a red pillar (+0x48 20 wide, +0x4C 3
/// high), rising 1.2 and turning 0x100 a update; in by 0x40 below 0x80 for
/// 0x1A updates, out by 4.
fn pillar(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        let k = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xE, k);
        fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        fx.mem.write_u32(t + 0x4C, 0x4040_0000);
        rgb(fx, t, 0x60, (0x80, 0x10, 0x10));
        rgb(fx, t, 0x64, (0x80, 0x10, 0x10));
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.set16(t + 6, 0);
    }
    if fx.mem.i16(t + 0x54) < 0x1A {
        rise(fx, t, 0x63, 0x80, 0x40);
    } else {
        fall(fx, t, 0x63, 6, 4);
    }
    let v = fx.mem.read_u32(t + 0x24).wrapping_add(0x100);
    fx.mem.write_u32(t + 0x24, v);
    let y = ee::add(fx.mem.f(t + 0x14), f32::from_bits(0x3F99_999A));
    fx.mem.setf(t + 0x14, y);
    age(fx, t);
}

/// 0x1E03B50 (D, t; frame 0x30, s0 = t, s1 = D): a wave ring by +0x50
/// (0: 200, blue over white; 1: 1000 and 2: 800, red; 0xA: 230 and 0xB:
/// 110 facing the camera, blue), with its own in / out (+8 updates).
fn wave(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        let rows: [(i32, u32, (i32, i32, i32), (i32, i32, i32)); 5] = [
            (0, 0x4348_0000, (0xF0, 0xF0, 0xF0), (0x10, 0x10, 0xF0)),
            (1, 0x447A_0000, (0xC0, 0xC0, 0xC0), (0x80, 0x10, 0x10)),
            (2, 0x4448_0000, (0xC0, 0xC0, 0xC0), (0x80, 0x10, 0x10)),
            (0xA, 0x4366_0000, (0x80, 0x80, 0x80), (0x10, 0x10, 0xF0)),
            (0xB, 0x42DC_0000, (0x80, 0x80, 0x80), (0x10, 0x10, 0xF0)),
        ];
        let k = fx.mem.i16(t + 0x50);
        if let Some(&(_, r, c0, c1)) = rows.iter().find(|r| r.0 == k) {
            fx.mem.write_u32(t + 0x48, r);
            rgb(fx, t, 0x60, c0);
            rgb(fx, t, 0x64, c1);
        }
        let k = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xE, k);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.set16(t + 6, 0);
        let n = match fx.mem.i16(t + 0x50) {
            0 | 2 => Some(0x12),
            1 | 0xA => Some(6),
            0xB => Some(0x64),
            _ => None,
        };
        if let Some(n) = n {
            fx.mem.set16(t + 8, n);
        }
    }
    let k = fx.mem.i16(t + 0x50);
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 8) {
        match k {
            0 => rise(fx, t, 0x67, 0x80, 0xC),
            1 => rise(fx, t, 0x63, 0x80, 0x2A),
            2 => rise(fx, t, 0x63, 0x80, 7),
            0xA => rise(fx, t, 0x63, 0x80, 0x20),
            0xB => rise(fx, t, 0x63, 0x60, 0x18),
            _ => {}
        }
    } else {
        match k {
            0 => fall(fx, t, 0x67, 0xB, 9),
            1 => fall(fx, t, 0x63, 0xB, 9),
            2 => {
                if counter(fx) >= 0x1F5 {
                    fall(fx, t, 0x63, 0xB, 9);
                }
            }
            0xA => fall(fx, t, 0x63, 8, 6),
            0xB => fall(fx, t, 0x63, 6, 4),
            _ => {}
        }
    }
    let k = fx.mem.i16(t + 0x50);
    if k == 0xA || k == 0xB {
        crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_3F48);
        let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
        fx.mem.write_u32(t + 0x20, v);
    }
    age(fx, t);
}

/// 0x1E03F80 (D, t; frame 0x10): the stage light +0x50 from 0x80 down by
/// 10 to 0x30, back up by 10 once the timer is below 0x1F; 0x80 on the
/// timer's 2.
fn light(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let v = fx.mem.i16(t + 0x50);
    if timer(fx) >= 0x1F {
        fx.mem.set16(t + 0x50, if v < 0x31 { 0x30 } else { v - 0xA });
    } else {
        fx.mem.set16(t + 0x50, if v < 0x80 { v + 0xA } else { 0x80 });
    }
    if timer(fx) == 2 {
        fx.set_stage_light(0x80);
    } else {
        let l = fx.mem.read_u8(t + 0x50);
        fx.set_stage_light(i32::from(l));
    }
}

/// 0x1E04050 (D, t; frame 0x30): the object op 0x1F8380(3, 2, the record's
/// words 0x1E06E00 / 0x1E06E08 at sp + 0x28 / + 0x2C, 0) each update; the
/// task ends from age 0x41.
fn object(fx: &mut Fx, t: u32, sp: u32) {
    for (to, from) in [(0x28u32, 0x01E0_6E00u32), (0x2C, 0x01E0_6E08)] {
        let w = fx.mem.read_u32(from);
        fx.mem.write_u32(sp + to, w);
    }
    fx.boundary(0x001F_8380, "object op 0x1F8380", [3, 2, sp + 0x28, sp + 0x2C]);
    age(fx, t);
    if fx.mem.i16(t + 0x54) >= 0x41 {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E040D0 (D, t; frame 0x90): the frame window (a window sprite
/// 0x6FF8..0x8FF8 sampling the frame (0x207EF0) at ST inset by +0x48 + 14
/// over 512, colour (0x70, 0x60, 0x80, +0x63), ALPHA 0x44, key 0); +0x48 =
/// 14 sin +0x5A, +0x5A turning 4 a update; in by 10 below 0x40 to age
/// 0x90, then out by 9.
fn window(fx: &mut Fx, t: u32, sp: u32) {
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u8(sp + 0x48, 0x70);
    fx.mem.write_u8(sp + 0x49, 0x60);
    fx.mem.write_u8(sp + 0x4A, 0x80);
    let a = fx.mem.read_u8(t + 0x63);
    fx.mem.write_u8(sp + 0x4B, a);
    fx.mem.write_u32(sp + 0x4C, 0x3F80_0000);
    for (at, v) in [(sp + 0x58, 0x6FF8u16), (sp + 0x68, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x50, ee::div(ee::add(s, 14.0), 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x54, ee::div(ee::add(s, 14.0), 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x60, ee::div(ee::sub(ee::add(512.0, s), 14.0), 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x64, ee::div(ee::sub(ee::add(512.0, s), 14.0), 512.0));
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    let a = fx.mem.i16(t + 0x5A);
    let s = sin(fx, a, 0);
    fx.mem.setf(t + 0x48, ee::mul(14.0, s));
    let a = fx.mem.i16(t + 0x5A) + 4;
    fx.mem.set16(t + 0x5A, a);
    if fx.mem.i16(t + 0x54) < 0x90 {
        rise(fx, t, 0x63, 0x40, 0xA);
    } else {
        fall(fx, t, 0x63, 0xB, 9);
    }
    age(fx, t);
}

/// Project the strip at `v` through `m` into `d`; on screen keyed by the
/// mean w (0x1A2C70, its residue from `ra`) into the ordering table at
/// `ot_at`.
fn submit_w(fx: &mut Fx, v: u32, m: u32, d: u32, w: u32, ot_at: u32, ra: u32) {
    if l2::project(fx, v, SPR, m, d, w) {
        let w = fx.mem.f(w);
        fx.fix_residue(w, ra);
        let key = ee::fixunssfsi(w) as i32;
        let ot = fx.mem.read_u32(ot_at);
        fx.submit(d, ot, key);
    }
}

/// 0x1E00560(D, t, mode, pos) (frame 0x1D0): a flat ring of 36 gouraud
/// quads 10 degrees apart from +0x5A (inner radius +0x48 coloured +0x64,
/// outer (float) +6 coloured +0x60) turned by the task's angles +0x20 at
/// `pos`, scale D+0x10; blend (mode >> 4) as i16, depth mode & 15 (0 keyed
/// by the mean w, 1 at the far Z key 0, else nothing drawn).
fn ring_flat(fx: &mut Fx, t: u32, mode: i32, pos: u32, r: &Regs, ra: u32) {
    let sp = fx.enter(0x1D0);
    r.save(fx, sp, 0x10, 0x80, ra);
    for k in 0..3u32 {
        fx.mem.write_u32(sp + 4 * k, 0);
    }
    copy4(fx, sp + 0x90, pos);
    let d = sp + 0x140;
    l2::colour_strip4(fx, d, t + 0x60, 2);
    l2::colour_strip4(fx, d, t + 0x64, 1);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    fx.desc_alpha(d, blend_alpha((mode >> 4) as i16 as i32));
    copy3(fx, sp + 0xE0, sp + 0x90);
    l2::xform_vp(fx, sp + 0xF0, D + 0x10, t + 0x20, sp + 0xE0);
    for o in [0xA4u32, 0xB4, 0xC4, 0xD4] {
        fx.mem.set32(sp + o, 0);
    }
    let inner = fx.mem.f(t + 0x48);
    let outer = ee::from_i32(fx.mem.i16(t + 6));
    let depth = mode & 0xF;
    let tab = D + 0xD8;
    let mut s1 = 0i32;
    for k in 0..0x24 {
        if k == 0 {
            let a = (fx.mem.i16(t + 0x5A) + s1) as i16 as i32;
            let c = l2::sin_deg(fx, tab, a, 1);
            let s = l2::sin_deg(fx, tab, a, 0);
            fx.mem.setf(sp + 0xA0, ee::mul(c, inner));
            fx.mem.setf(sp + 0xA8, ee::mul(s, inner));
            fx.mem.setf(sp + 0xC0, ee::mul(c, outer));
            fx.mem.setf(sp + 0xC8, ee::mul(s, outer));
        } else {
            copy4(fx, sp + 0xA0, sp + 0xB0);
            copy4(fx, sp + 0xC0, sp + 0xD0);
        }
        let a = (s1 + (fx.mem.i16(t + 0x5A) + 0xA)) as i16 as i32;
        let c = l2::sin_deg(fx, tab, a, 1);
        let s = l2::sin_deg(fx, tab, a, 0);
        fx.mem.setf(sp + 0xB0, ee::mul(c, inner));
        fx.mem.setf(sp + 0xB8, ee::mul(s, inner));
        fx.mem.setf(sp + 0xD0, ee::mul(c, outer));
        fx.mem.setf(sp + 0xD8, ee::mul(s, outer));
        if depth == 0 {
            submit_w(fx, sp + 0xA0, sp + 0xF0, d, sp + 0x130, D + 0x80, 0x01E0_07E4);
        } else if depth == 1 && l2::project(fx, sp + 0xA0, SPR, sp + 0xF0, d, sp + 0x130) {
            l2::far_z_desc(fx, d, 1);
            let ot = fx.mem.read_u32(D + 0x80);
            fx.submit(d, ot, 0);
        }
        s1 = (s1 + 0xA) as i16 as i32;
    }
    fx.leave(0x1D0);
}

/// 0x1E054A0(D, t, mode, pos) (frame 0x1E0): the hexagon at `pos` (turned
/// 0x8000 unless D+0xAA0E), gouraud +0x64 / +0x60, no blend set: mode 0 the
/// six quads between the corners D+0xA880 (indices D+0xA9B4 + 8 k, pushed
/// out by D+0xA92C), else two rings of 36 quads (radii 12 / 14 and 18 /
/// 20 from D+0xA920 / 0xA924 / 0xA928); keyed by the mean w into D+0xA90C.
fn hexes(fx: &mut Fx, t: u32, mode: i32, pos: u32, r: &Regs, ra: u32) {
    let sp = fx.enter(0x1E0);
    r.save(fx, sp, 0, 0x60, ra);
    copy4(fx, sp + 0x70, pos);
    fx.mem.write_u32(sp + 0x140, 0);
    fx.mem.write_u32(sp + 0x148, 0);
    let y = if fx.mem.i16(FACING) != 0 { 0 } else { 0x8000 };
    fx.mem.write_u32(sp + 0x144, y);
    copy3(fx, sp + 0xC0, sp + 0x70);
    l2::xform_vp(fx, sp + 0xD0, 0, sp + 0x140, sp + 0xC0);
    let d = sp + 0x150;
    l2::colour_strip4(fx, d, t + 0x64, 2);
    l2::colour_strip4(fx, d, t + 0x60, 1);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    if mode == 0 {
        for k in 0..6u32 {
            fx.bzero(sp + 0x80, 0x40);
            let q = D + 0xA9B4 + 8 * k;
            for (to, i) in [(0x80u32, 0u32), (0xA0, 2), (0x90, 4), (0xB0, 6)] {
                let c = fx.mem.i16(q + i);
                copy4(fx, sp + to, corner(c));
            }
            let w = fx.mem.f(WIDEN);
            let nudge = |fx: &mut Fx, o: u32, up: bool| {
                let v = fx.mem.f(sp + o);
                fx.mem.setf(sp + o, if up { ee::add(v, w) } else { ee::sub(v, w) });
            };
            match k {
                0 | 5 => {
                    nudge(fx, 0xA0, false);
                    nudge(fx, 0xB0, false);
                }
                1 => {
                    nudge(fx, 0xA8, true);
                    nudge(fx, 0xB8, true);
                }
                2 | 3 => {
                    nudge(fx, 0xA0, true);
                    nudge(fx, 0xB0, true);
                }
                _ => {
                    nudge(fx, 0xA8, false);
                    nudge(fx, 0xB8, false);
                }
            }
            submit_w(fx, sp + 0x80, sp + 0xD0, d, sp + 0x110, OT2, 0x01E0_57D0);
        }
    } else {
        l2::colour_strip4(fx, d, t + 0x64, 2);
        l2::colour_strip4(fx, d, t + 0x60, 1);
        for o in [0x84u32, 0x94, 0xA4, 0xB4] {
            fx.mem.set32(sp + o, 0);
        }
        let (a, b, c) = (fx.mem.f(R12), fx.mem.f(R6), fx.mem.f(R2));
        fx.mem.setf(sp + 0x130, a);
        fx.mem.setf(sp + 0x134, ee::add(a, c));
        fx.mem.setf(sp + 0x138, ee::add(a, b));
        fx.mem.setf(sp + 0x13C, ee::add(c, ee::add(a, b)));
        let mut s3 = 0i32;
        for _ in 0..0x24 {
            let v = sin(fx, s3, 0);
            fx.mem.setf(sp + 0x120, v);
            let v = sin(fx, s3, 1);
            fx.mem.setf(sp + 0x124, v);
            s3 = (s3 + 0xA) as i16 as i32;
            let v = sin(fx, s3, 0);
            fx.mem.setf(sp + 0x128, v);
            let v = sin(fx, s3, 1);
            fx.mem.setf(sp + 0x12C, v);
            for s2 in 0..2u32 {
                let f1 = fx.mem.f(sp + 0x130 + 8 * s2);
                let f2 = fx.mem.f(sp + 0x134 + 8 * s2);
                for v in 0..2u32 {
                    let (a0, a1) = (sp + 0x120 + 8 * v, sp + 0x80 + 0x10 * v);
                    let (s, c) = (fx.mem.f(a0), fx.mem.f(a0 + 4));
                    fx.mem.setf(a1 + 8, ee::mul(s, f1));
                    fx.mem.setf(a1, ee::mul(c, f1));
                    fx.mem.setf(a1 + 0x28, ee::mul(s, f2));
                    fx.mem.setf(a1 + 0x20, ee::mul(c, f2));
                }
                submit_w(fx, sp + 0x80, sp + 0xD0, d, sp + 0x110, OT2, 0x01E0_59DC);
            }
        }
    }
    fx.leave(0x1E0);
}

/// One band segment: the angles -(+0x5A + `s1` + 90) and 20 further, inner
/// `f20` / outer `f21`, keyed into D+0xA90C.
fn band_seg(fx: &mut Fx, t: u32, sp: u32, d: u32, s1: i32, f20: f32, f21: f32, ra: u32) {
    let a = (-(fx.mem.i16(t + 0x5A) + s1 + 0x5A)) as i16 as i32;
    let c = sin(fx, a, 1);
    let s = sin(fx, a, 0);
    fx.mem.setf(sp + 0xC0, ee::mul(c, f20));
    fx.mem.setf(sp + 0xC8, ee::mul(s, f20));
    fx.mem.setf(sp + 0xA0, ee::mul(c, f21));
    fx.mem.setf(sp + 0xA8, ee::mul(s, f21));
    let a = (-(s1 + (fx.mem.i16(t + 0x5A) + 0x14) + 0x5A)) as i16 as i32;
    let c = sin(fx, a, 1);
    let s = sin(fx, a, 0);
    fx.mem.setf(sp + 0xD0, ee::mul(c, f20));
    fx.mem.setf(sp + 0xD8, ee::mul(s, f20));
    fx.mem.setf(sp + 0xB0, ee::mul(c, f21));
    fx.mem.setf(sp + 0xB8, ee::mul(s, f21));
    submit_w(fx, sp + 0xA0, sp + 0xF0, d, sp + 0x130, OT2, ra);
}

/// 0x1E05A70(D, t, mode, pos) (frame 0x1F0): a textured band of strip3
/// segments turned by the task's angles +0x20 at `pos` (texture D+0x88,
/// colours +0x60 / +0x64): D+0xB0 0 the hexagon's radii 12 / 18, 1 +0x48 /
/// +0x4C, 2 a wall of radius +0x48 and height +0x4C; 18 segments of 20
/// degrees (cells D+0xA96C) or 9 of 40, only below D+0xAA00 degrees.
fn bands(fx: &mut Fx, t: u32, mode: i32, pos: u32, r: &Regs, ra: u32) {
    let sp = fx.enter(0x1F0);
    r.save(fx, sp, 0x10, 0x80, ra);
    for k in 0..3u32 {
        fx.mem.write_u32(sp + 4 * k, 0);
    }
    copy4(fx, sp + 0x90, pos);
    let d = sp + 0x160;
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x60, 1);
    l2::colour_strip3(fx, d, t + 0x64, 2);
    let o = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, o);
    let rh = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, rh, 1);
    fx.desc_alpha(d, blend_alpha(mode >> 4));
    copy3(fx, sp + 0xE0, sp + 0x90);
    l2::xform_vp(fx, sp + 0xF0, 0, t + 0x20, sp + 0xE0);
    for o in [0xA4u32, 0xB4, 0xC4, 0xD4] {
        fx.mem.set32(sp + o, 0);
    }
    let layout = fx.mem.i16(D + 0xB0);
    let (f20, f21) = match layout {
        0 => {
            let a = fx.mem.f(R12);
            (a, ee::add(a, fx.mem.f(R6)))
        }
        1 => (fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C)),
        2 => {
            let a = fx.mem.f(t + 0x48);
            let h = fx.mem.read_u32(t + 0x4C);
            fx.mem.write_u32(sp + 0xA4, h);
            let h = fx.mem.read_u32(t + 0x4C);
            fx.mem.write_u32(sp + 0xB4, h);
            (a, a)
        }
        // The caller's f20 / f21 [H: never met].
        _ => (0.0, 0.0),
    };
    if layout != 2 {
        let mut s1 = 0i32;
        for k in 0..0x12u32 {
            let sub = fx.mem.i16(D + 0xA96C + 2 * k);
            cell_sub(fx, sp + 0x140, 0x14, sub);
            let o = fx.mem.i16(D + 0xBE);
            l2::uv_strip3(fx, d, D + 0x94, o);
            if s1 < fx.mem.i16(D + 0xAA00) {
                band_seg(fx, t, sp, d, s1, f20, f21, 0x01E0_5DA8);
            }
            s1 = (s1 + 0x14) as i16 as i32;
        }
    } else {
        cell_sub(fx, sp + 0x150, 0x9B, 0);
        let o = fx.mem.i16(D + 0xBE);
        l2::uv_strip3(fx, d, D + 0x94, o);
        let mut s1 = 0i32;
        for _ in 0..9 {
            if s1 < fx.mem.i16(D + 0xAA00) {
                band_seg(fx, t, sp, d, s1, f20, f21, 0x01E0_5F68);
            }
            s1 = (s1 + 0x28) as i16 as i32;
        }
    }
    fx.leave(0x1F0);
}

/// 0x109A90(d, c1, c2): a two-vertex gouraud descriptor (4 values, flags
/// 0x11, regs RGBAQ XYZF2 x 2, additive), the colours at +0x10 / +0x20
/// with Q 1.0 and their alphas' top bytes +0x1F / +0x2F cleared (b053's).
fn line_desc(fx: &mut Fx, d: u32, c1: u32, c2: u32) {
    fx.enter(0x40);
    fx.mem.set16(d + 2, 0x11);
    fx.mem.set16(d, 4);
    fx.mem.write_u64(d + 8, 0x4141);
    fx.mem.set32(d + 4, 0);
    fx.desc_alpha(d, crate::fxlib::ALPHA_ADD);
    for k in 0..4 {
        let b = fx.mem.read_u8(c1 + k);
        fx.mem.write_u8(d + 0x10 + k, b);
        let b = fx.mem.read_u8(c2 + k);
        fx.mem.write_u8(d + 0x20 + k, b);
    }
    fx.mem.write_u32(d + 0x24, 0x3F80_0000);
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    fx.mem.write_u8(d + 0x1F, 0);
    fx.mem.write_u8(d + 0x2F, 0);
    fx.leave(0x40);
}

/// 0x1E06CD0(out, p, q) (a leaf, frame 0x30, f12 / f13 = `div` / `f`):
/// out = p + (q - p) / div x f in x y z, its w the frame's stale +0x2C.
fn lerp(fx: &mut Fx, out: u32, p: u32, q: u32, div: f32, f: f32) {
    let s = fx.sp.wrapping_sub(0x30);
    copy4(fx, s, p);
    copy4(fx, s + 0x10, q);
    for k in 0..3u32 {
        let v = ee::div(ee::sub(fx.mem.f(s + 0x10 + 4 * k), fx.mem.f(s + 4 * k)), div);
        fx.mem.setf(s + 0x20 + 4 * k, v);
    }
    for k in 0..3u32 {
        let v = ee::add(fx.mem.f(s + 4 * k), ee::mul(fx.mem.f(s + 0x20 + 4 * k), f));
        fx.mem.setf(s + 0x20 + 4 * k, v);
    }
    copy4(fx, out, s + 0x20);
}

/// 0x1E05FF0(D, t, d, 0) (frame 0x140): the glow's lines at the point
/// (turned 0x8000 unless D+0xAA0E), coloured +0x60 (0x109A90 into the
/// entry's `d`): six spokes between the corners D+0xA9E4 + 4 k growing in
/// tenths from D+0xA934 + 4 k by D+0xAA02, then two circles (radii 12 /
/// 18) of D+0xAA04 segments of 5 degrees from 90; keyed into D+0xA90C.
fn lines(fx: &mut Fx, t: u32, d: u32, r: &Regs, ra: u32) {
    let sp = fx.enter(0x140);
    r.save(fx, sp, 0x10, 0x80, ra);
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 0x120, 0);
    fx.mem.write_u32(sp + 0x128, 0);
    let y = if fx.mem.i16(FACING) != 0 { 0 } else { 0x8000 };
    fx.mem.write_u32(sp + 0x124, y);
    copy3(fx, sp + 0xB0, POINT);
    l2::xform_vp(fx, sp + 0xE0, 0, sp + 0x120, sp + 0xB0);
    line_desc(fx, d, t + 0x60, t + 0x60);
    for k in 0..6u32 {
        let grown = ee::from_i32(fx.mem.i16(D + 0xAA02));
        fx.bzero(sp + 0x90, 0x20);
        let start = fx.mem.f(D + 0xA934 + 4 * k);
        let mut f20 = ee::sub(grown, start);
        if !(f20 <= 10.0) {
            f20 = 10.0;
        }
        let now = ee::from_i32(fx.mem.i16(D + 0xAA02));
        if !(now <= start) {
            let a = fx.mem.i16(D + 0xA9E4 + 4 * k);
            copy4(fx, sp + 0x90, corner(a));
            let b = fx.mem.i16(D + 0xA9E6 + 4 * k);
            copy4(fx, sp + 0xA0, corner(b));
            lerp(fx, sp + 0xC0, sp + 0x90, sp + 0xA0, 10.0, f20);
            copy3(fx, sp + 0xA0, sp + 0xC0);
        }
        submit_w(fx, sp + 0x90, sp + 0xE0, d, sp + 0xD0, OT2, 0x01E0_6200);
    }
    fx.bzero(sp + 0x90, 0x20);
    fx.mem.set16(sp + 0x13C, -0x5A);
    fx.mem.set16(sp + 0x13E, -0x5A);
    let mut n = 0i32;
    while n < fx.mem.i16(D + 0xAA04) {
        for s3 in 0..2u32 {
            let h = sp + 0x13C + 2 * s3;
            let f20 = if s3 != 0 { ee::add(fx.mem.f(R12), fx.mem.f(R6)) } else { fx.mem.f(R12) };
            for s2 in 0..2u32 {
                let p = sp + 0x90 + 0x10 * s2;
                let a = -fx.mem.i16(h);
                let s = sin(fx, a, 0);
                fx.mem.setf(p + 8, ee::mul(f20, s));
                let a = -fx.mem.i16(h);
                let c = sin(fx, a, 1);
                fx.mem.setf(p, ee::mul(f20, c));
                if s2 == 0 {
                    let v = fx.mem.i16(h) + 5;
                    fx.mem.set16(h, v);
                }
            }
            submit_w(fx, sp + 0x90, sp + 0xE0, d, sp + 0xD0, OT2, 0x01E0_635C);
        }
        n = (n + 1) as i16 as i32;
    }
    fx.leave(0x140);
}

/// 0x108020(local, c) and the 8 bytes it builds copied to `to`.
fn rgbaq_to(fx: &mut Fx, local: u32, c: u32, to: u32) {
    l2::rgbaq(fx, local, c);
    for k in 0..2 {
        let w = fx.mem.read_u32(local + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// b191's Lagrange cubic through `p` at `t`.
fn lagrange(p: [[f32; 3]; 4], t: f32) -> [f32; 3] {
    use ee::{add, div, mul, neg, sub};
    let (tm1, tm2, tp1) = (sub(t, 1.0), sub(t, 2.0), add(1.0, t));
    let w0 = div(mul(neg(t), mul(tm1, tm2)), 6.0);
    let w3 = div(mul(tp1, mul(t, tm1)), 6.0);
    let w1 = div(mul(tm2, mul(tp1, tm1)), 2.0);
    let w2 = div(mul(neg(tp1), mul(t, tm2)), 2.0);
    std::array::from_fn(|c| {
        let f6 = add(mul(p[0][c], w0), mul(p[1][c], w1));
        let acc = add(mul(p[2][c], w2), f6);
        add(acc, mul(p[3][c], w3))
    })
}

fn points(fx: &mut Fx, at: [u32; 4]) -> [[f32; 3]; 4] {
    std::array::from_fn(|i| std::array::from_fn(|c| fx.mem.f(at[i] + 4 * c as u32)))
}

/// 5c018f616265 (0x1E06400, this record's only user): b771's ribbon
/// (a907e1567858, b191's strip3 ribbon keyed by the mean w) with three
/// quads a history point, the curve parameter stepping 0.33 (0x3EA8F5C3)
/// instead of two by 0.5 [F word-compare: the only differences].
fn ribbon(fx: &mut Fx, t: u32, a: u32, b: u32, r: &Regs) {
    let sp = fx.enter(0x390);
    let _ = r;
    let n = fx.mem.i16(t + 6);
    copy3(fx, sp + 0x160, D + 0x20);
    l2::xform_vp(fx, sp + 0x230, 0, 0, sp + 0x160);
    let d = sp + 0x2D8;
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x60, 0);
    l2::uv_strip3(fx, d, D + 0x94, 0);
    let rh2 = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, rh2, 1);
    let (ca, cb) = (sp + 0x388, sp + 0x38C);
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(ca + k, v);
        let v = fx.mem.read_u8(t + 0x64 + k);
        fx.mem.write_u8(cb + k, v);
    }
    let fp = 2 * n;
    copy4(fx, sp + 0x2B0, a);
    for (k, o) in [0u32, 0, 0x10, 0x20].into_iter().enumerate() {
        let from = if k < 2 { sp + 0x2B0 } else { a + o };
        copy4(fx, sp + 0x1B0 + 0x10 * k as u32, from);
    }
    copy4(fx, sp + 0x2C0, b);
    for (k, o) in [0u32, 0, 0x10, 0x20].into_iter().enumerate() {
        let from = if k < 2 { sp + 0x2C0 } else { b + o };
        copy4(fx, sp + 0x1F0 + 0x10 * k as u32, from);
    }
    for (to, from) in [(0xD4u32, 0x1C0u32), (0xD8, 0x1D0), (0xDC, 0x1E0), (0xE0, 0x1C4), (0xE4, 0x1B4), (0xE8, 0x1D4), (0xEC, 0x1E4), (0xF0, 0x1C8), (0xF4, 0x1B8), (0xF8, 0x1D8), (0xFC, 0x1E8)] {
        let w = fx.mem.read_u32(sp + from);
        fx.mem.write_u32(sp + to, w);
    }
    fx.mem.set32(sp + 0x100, n - 1);
    let step = f32::from_bits(0x3EA8_F5C3);
    let mut s0 = 0i32;
    while s0 < fx.mem.i32(sp + 0x100) {
        let (pa, pb) = if s0 != 0 {
            let qa = a.wrapping_add((0x10 * s0) as u32);
            let qb = b.wrapping_add((0x10 * s0) as u32);
            ([qa.wrapping_sub(0x10), qa, qa + 0x10, qa + 0x20], [qb.wrapping_sub(0x10), qb, qb + 0x10, qb + 0x20])
        } else {
            let pa = sp + 0x1B0;
            let pb = sp + 0x1F0;
            ([pa, pa + 0x10, pa + 0x20, pa + 0x30], [pb, pb + 0x10, pb + 0x20, pb + 0x30])
        };
        let mut f20 = 0.0f32;
        for _ in 0..3 {
            let v = fx.mem.read_u8(ca + 3);
            fx.mem.write_u8(cb + 3, v);
            rgbaq_to(fx, sp + 0x368, ca, d + 0x10);
            rgbaq_to(fx, sp + 0x370, cb, d + 0x40);
            let al = i32::from(fx.mem.read_u8(t + 0x63));
            let q = fx.div(al, fp).0 as i16 as i32;
            let v = i32::from(fx.mem.read_u8(ca + 3)) - q;
            fx.mem.write_u8(ca + 3, if v > 0 { v as u8 } else { 0 });
            let v = fx.mem.read_u8(ca + 3);
            fx.mem.write_u8(cb + 3, v);
            rgbaq_to(fx, sp + 0x378, ca, d + 0x28);
            rgbaq_to(fx, sp + 0x380, cb, d + 0x58);
            for i in 0..2u32 {
                let pts = points(fx, pa);
                let v = lagrange(pts, f20);
                let out = if s0 != 0 { sp + 0x270 } else { sp + 0x290 };
                for (c, x) in v.into_iter().enumerate() {
                    fx.mem.setf(out + 4 * c as u32, x);
                }
                copy4(fx, sp + 0x170 + 0x10 * i, out);
                let pts = points(fx, pb);
                let v = lagrange(pts, f20);
                let out = if s0 != 0 { sp + 0x280 } else { sp + 0x2A0 };
                for (c, x) in v.into_iter().enumerate() {
                    fx.mem.setf(out + 4 * c as u32, x);
                }
                copy4(fx, sp + 0x190 + 0x10 * i, out);
                if i == 0 {
                    f20 = ee::add(f20, step);
                }
                if f20 > 9.0 {
                    f20 = 1.0;
                }
            }
            for (to, from) in [(0x120u32, 0x170u32), (0x130, 0x180), (0x140, 0x190), (0x150, 0x1A0)] {
                copy3(fx, sp + to, sp + from);
            }
            if l2::project(fx, sp + 0x120, SPR, sp + 0x230, d, sp + 0x110) {
                let w = fx.mem.f(sp + 0x110);
                fx.fix_residue(w, 0x01E0_6C30);
                let key = ee::fixunssfsi(w) as i32;
                let ot = fx.mem.read_u32(D + 0x80);
                fx.submit(d, ot, key);
            }
        }
        s0 += 1;
    }
    fx.leave(0x390);
}
