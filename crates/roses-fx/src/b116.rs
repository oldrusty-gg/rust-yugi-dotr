//! BMODULE b116 (card 274), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E042D0, data area 0x1ED0000) [F]: b003's program (170 tasks of 0x40 bytes at D+0x2A0, alive +0x36,
//! step +0x24, each step called by its own `jal`; spawn 0x1E053F0 = b003's)
//! with its own emitter 0x1E01650 and steps. On update 0x14 five points
//! D+0x2DD0 (copied to D+0x2E20) are taken from the attacker's position,
//! pulled a 75th toward z 0 and jittered, and five times 0x5A..0xAD drawn
//! (D+0x4A38, k 15 apart); at time k an orb set at (0, 60, 0) pushed 13.2
//! along y / x by k (D+0x2D30 + 0x10 k; its orbit point D+0x2D80 + 0x10 k
//! 60 lower): an orbiter 0x1E02E90 (not drawn: from update 0x170 it
//! circles D+0x2D80 by the second sine table D+0x4B64, radius D+0x4A20 13.2
//! (0 for k 0) closing by 0.3, lifted by D+0x4A34), a trail 0x1E03190 (a
//! 50-point history D+0x2E70 + 0x460 k of the orbit point drawn by b258's
//! tube in halves, red / yellow on alternate updates) and an orb sprite
//! 0x1E03360 (0x1E009A0: b258's sprite facing the camera through the
//! task's own angles) sliding from its point toward its slot over ages
//! 13..42. Updates 0xCD / 0xE6 a mirrored pair of glyph quads 0x1E036C0
//! (0x1E058A0) at (0, 60, 0); 0x117..0x142 rings (0x1E03D70 at the points,
//! 0x1E03AA0 two at point 0), 0x16F ten rings 0x1E03C60 at the histories'
//! ends, 0x19A..0x1C1 a target ring of quads 0x1E01CE0 (`ring_at_m_uv1`) a
//! update and on odd updates a rising band 0x1E039A0 (`ring_band_t`).
//! Returns 1 on update 0x190, 3 on 0x1C7, 6 on 0x19B..0x1C1, 8 once the
//! 504-update timer D+0x44A0 has run out.
//!
//! Never spawned [F: the emitter's fifteen spawn `jal`s name 0x1E03360 /
//! 0x1E03190 / 0x1E02E90 / 0x1E036C0 / 0x1E01CE0 / 0x1E039A0 / 0x1E03AA0 /
//! 0x1E03D70 / 0x1E03C60 only, and no step spawns]: 0x1E01E90, 0x1E01F70,
//! 0x1E02370, 0x1E02630, 0x1E029F0, 0x1E02C40, 0x1E03870 (and its draw
//! 0x1E05510), 0x1E03E60, 0x1E03FD0, 0x1E04090 (the object op), 0x1E040E0
//! (the flash, so the unit flash 0x1E06850 never runs); not ported.
use crate::blib::{quads4, spawn as bspawn, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 0xAA;
const RING: u32 = 0x01E0_1CE0;
const ORBIT: u32 = 0x01E0_2E90;
const TRAIL: u32 = 0x01E0_3190;
const ORB: u32 = 0x01E0_3360;
const GLYPH: u32 = 0x01E0_36C0;
const BAND: u32 = 0x01E0_39A0;
const WIDE: u32 = 0x01E0_3AA0;
const ENDS: u32 = 0x01E0_3C60;
const POINT: u32 = 0x01E0_3D70;
/// The steps a live task can hold, with the entry's return address after
/// each step's `jal`.
const STEPS: [(u32, u32); 9] = [
    (RING, 0x01E0_4520),
    (ORBIT, 0x01E0_4600),
    (TRAIL, 0x01E0_4620),
    (ORB, 0x01E0_4640),
    (GLYPH, 0x01E0_4660),
    (BAND, 0x01E0_46A0),
    (WIDE, 0x01E0_46C0),
    (ENDS, 0x01E0_46E0),
    (POINT, 0x01E0_4700),
];
const HOME: u32 = D + 0x2D20;
const SLOTS: u32 = D + 0x2D30;
const ORBITS: u32 = D + 0x2D80;
const STARTS: u32 = D + 0x2DD0;
const POINTS: u32 = D + 0x2E20;
const HISTORY: u32 = D + 0x2E70;
const TIMER: u32 = D + 0x44A0;
const COLOUR: u32 = D + 0x44A4;
const RADII: u32 = D + 0x4A20;
const LIFT: u32 = D + 0x4A34;
const TIMES: u32 = D + 0x4A38;
const INDEX: u32 = D + 0x4A5C;
const YAW: u32 = D + 0x4A60;
const COUNTER: u32 = D + 0x4A62;
const BYTES: u32 = D + 0x4A64;
const SINES: u32 = D + 0x4B64;

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

/// The i16 point +0 / +2 / +4 as floats at `out`.
fn point_f(fx: &mut Fx, t: u32, out: u32) {
    for k in 0..3 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(out + 4 * k, v);
    }
}

/// rand % m (HI) from the `jal` returning to `ra` (rand's frame: s0).
fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E042D0 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1B0);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x44AC, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x1B0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4CD0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x4A5E, param);
    fx.mem.set32(TIMER, 0x1F8);
    for (target, n, clear, at) in [(false, 0x1F, 1, 0x4A42u32), (false, 0x20, 0, 0x4A44), (true, 1, 1, 0x4A50), (false, 0x47, 0, 0x4A48), (false, 0x4E, 0, 0x4A4A)] {
        let h = if target { fx.target_handle() } else { fx.self_handle() };
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(D + at, r);
    }
    for id in 0xEu32..0x10 {
        let tex = fx.texture(4, id as i32);
        fx.mem.write_u32(D + 0x298 + 4 * (id - 0xE), tex);
    }
    let tex = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x268, tex);
    let tex = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, tex);
    let tex = fx.texture(5, 0);
    fx.mem.write_u32(D + 0x4450, tex);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x100);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0x104)), 180.0);
    fx.fix_residue(v, 0x01E0_445C);
    fx.mem.set16(YAW, ee::fixsfsi(v));
    // rand's frame: s0 = D (the loop counter is s1).
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, 0x01E0_446C);
        fx.mem.write_u8(BYTES + k, ((r & 0xF) - 8) as u8);
    }
    for k in 0..8u32 {
        let r = fx.rand_from(D, 0x01E0_44AC);
        fx.mem.set16(D + 0xC0 + 2 * k, r & 3);
    }
    fx.mem.write_u32(LIFT, 0);
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x24);
        if let Some(&(_, ra)) = STEPS.iter().find(|s| s.0 == f) {
            run_step(fx, f, t, ra);
        }
        debug_assert!(!matches!(f, 0x01E0_1E90 | 0x01E0_1F70 | 0x01E0_2370 | 0x01E0_2630 | 0x01E0_29F0 | 0x01E0_2C40 | 0x01E0_3870 | 0x01E0_3E60 | 0x01E0_3FD0 | 0x01E0_4090 | 0x01E0_40E0));
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x70);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x40 * k;
        if fx.mem.i16(t + 0x36) != 0 {
            draw(fx, sp, t);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x190 => 1,
        0x1C7 => 3,
        0x19B..=0x1C1 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` frame +0x34 (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32, id: i32) {
    let frame = fx.mem.i16(t + 0x34);
    l2::rect(fx, r, id, frame);
    copy4(fx, D + 0x94, r);
}

fn uvs(fx: &mut Fx, v: [i32; 4]) {
    for (k, x) in v.into_iter().enumerate() {
        fx.mem.set16(D + 0x94 + 2 * k as u32, x);
    }
}

/// The entry's draw case for a live task (`sp` the entry's frame).
fn draw(fx: &mut Fx, sp: u32, t: u32) {
    let f = fx.mem.read_u32(t + 0x24);
    let k = fx.mem.i16(t + 0x30);
    match f {
        RING => {
            cell(fx, t, sp + 0x80, 0x14);
            let tex = fx.mem.read_u32(D + 0x260);
            fx.mem.write_u32(D + 0x88, tex);
            fx.mem.set16(D + 0xB8, 3);
            let r = fx.mem.i16(t + 0xA);
            fx.mem.set16(D + 0xC0, r);
            point_f(fx, t, D + 0x20);
            quads4::ring_at_m_uv1(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        ORBIT => {
            let tex = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, tex);
            cell(fx, t, sp + 0xE0, 0x9B);
        }
        TRAIL => {
            let tex = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, tex);
            uvs(fx, [1, 0x10, 0x1F, 1]);
            cell(fx, t, sp + 0xF0, 0x97);
            fx.mem.set16(D + 0x98, 1);
            for (o, v) in [(0x20u32, 0.0f32), (0x24, 60.0), (0x28, 0.0)] {
                fx.mem.setf(D + o, v);
            }
            fx.mem.set16(D + 0xBE, 1);
            crate::b258::tube(fx, t, HISTORY + 0x460 * k as u32, 0, true);
        }
        ORB => {
            let tex = fx.mem.read_u32(D + 0x4450);
            fx.mem.write_u32(D + 0x88, tex);
            uvs(fx, [0, 0, 0x3F, 0x3E]);
            for (o, v) in [(0x70u32, 0.0f32), (0x74, 60.0), (0x78, 0.0)] {
                fx.mem.setf(sp + o, v);
            }
            sprite(fx, t, 0x20, POINTS + 0x10 * k as u32);
        }
        GLYPH => {
            let tex = fx.mem.read_u32(D + 0x4450);
            fx.mem.write_u32(D + 0x88, tex);
            uvs(fx, [0, 0x40, 0xBF, 0xBF]);
            point_f(fx, t, sp + 0x70);
            for (o, v) in [(0x20u32, 100.0f32), (0x24, 0.0), (0x28, 0.0)] {
                fx.mem.setf(D + o, v);
            }
            fx.mem.set16(D + 0xBE, 0);
            pair(fx, t, sp + 0x118, HOME);
        }
        BAND => {
            point_f(fx, t, sp + 0x70);
            let y = ee::add(fx.mem.f(sp + 0x74), 1.5);
            fx.mem.setf(sp + 0x74, y);
            quads4::ring_band_t(fx, D, t, 0x21, sp + 0x70);
        }
        WIDE => {
            copy4(fx, sp + 0x70, POINTS);
            quads4::ring_band_t(fx, D, t, 0x21, sp + 0x70);
        }
        ENDS => {
            let heads = (0..5).map(|j| HISTORY + 0x460 * j);
            let tails = (0..5).map(|j| HISTORY + 0x310 + 0x460 * j);
            for p in heads.chain(tails) {
                copy4(fx, sp + 0x70, p);
                let y = ee::add(fx.mem.f(sp + 0x74), 60.0);
                fx.mem.setf(sp + 0x74, y);
                quads4::ring_band_t(fx, D, t, 0x21, sp + 0x70);
            }
        }
        POINT => {
            if k < 5 {
                copy4(fx, sp + 0x70, POINTS + 0x10 * k as u32);
            }
            quads4::ring_band_t(fx, D, t, 0x21, sp + 0x70);
        }
        _ => {}
    }
}

/// 0x1E01650(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the spawns (frame 0x60: ra, the entry's s2 = 0 / s1 = 0 / s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x30, 0x01E0_44EC), (0x20, 0), (0x10, 0), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    // The spawn's saves: the emitter's s0 / s1, s2 = D, the entry's s3
    // (the parameter), s4 (the runner's: 0) [H].
    let s3 = param as i64 as u64;
    let regs = |s0: u32, s1: u32| [(0x40u32, 0u64), (0x30, s3), (0x20, u64::from(D)), (0x10, u64::from(s1)), (0, u64::from(s0))];
    if fx.mem.i16(COUNTER) == 0xC8 {
        home(fx);
    }
    if fx.mem.i16(COUNTER) == 0x14 {
        for k in 0..5u32 {
            let h = fx.self_handle();
            fx.unit_position(h, STARTS + 0x10 * k);
        }
        for k in 0..5u32 {
            let p = STARTS + 0x10 * k;
            let s0 = D + 0x10 * k;
            let z = fx.mem.f(p + 8);
            let z = ee::sub(z, ee::div(z, 75.0));
            fx.mem.setf(p + 8, z);
            for (o, ra, m, off) in [(0u32, 0x01E0_1724u32, 20, -10), (4, 0x01E0_1758, 20, 0), (8, 0x01E0_1788, 10, -5)] {
                let r = rmod(fx, s0, ra, m);
                let v = ee::add(fx.mem.f(p + o), ee::from_i32(r + off));
                fx.mem.setf(p + o, v);
            }
        }
        for k in 0..5u32 {
            copy4(fx, POINTS + 0x10 * k, STARTS + 0x10 * k);
        }
        for (k, (ra, base)) in [(0x01E0_1804u32, 0x5A), (0x01E0_1824, 0x69), (0x01E0_1844, 0x78), (0x01E0_1864, 0x87), (0x01E0_1884, 0x96)].into_iter().enumerate() {
            let r = rmod(fx, D + 0x50, ra, 20);
            fx.mem.set16(TIMES + 2 * k as u32, r + base);
        }
    }
    home(fx);
    for k in 0..5u32 {
        if fx.mem.i16(COUNTER) != fx.mem.i16(TIMES + 2 * k) {
            continue;
        }
        fx.mem.set16(INDEX, k as i32);
        let s = SLOTS + 0x10 * k;
        copy4(fx, s, HOME);
        let bump = |fx: &mut Fx, at: u32, up: bool| {
            let d = f32::from_bits(0x4153_3333);
            let v = fx.mem.f(at);
            fx.mem.setf(at, if up { ee::add(v, d) } else { ee::sub(v, d) });
        };
        match k {
            1 => bump(fx, s + 4, true),
            2 => bump(fx, s + 4, false),
            3 => bump(fx, s, true),
            4 => bump(fx, s, false),
            _ => {}
        }
        let o = ORBITS + 0x10 * k;
        copy4(fx, o, s);
        let y = ee::sub(fx.mem.f(o + 4), 60.0);
        fx.mem.setf(o + 4, y);
        for (f, ra) in [(ORB, 0x01E0_1A30u32), (TRAIL, 0x01E0_1A44), (ORBIT, 0x01E0_1A58)] {
            spawn(fx, f, &regs(D + 2 * k, k), ra);
        }
    }
    // After the loop: s0 = D + 10, s1 = 5.
    let after = regs(D + 10, 5);
    if fx.mem.i16(COUNTER) == 0xCD {
        fx.mem.set16(INDEX, 0);
        spawn(fx, GLYPH, &after, 0x01E0_1A90);
    }
    if fx.mem.i16(COUNTER) == 0xE6 {
        fx.mem.set16(INDEX, 1);
        spawn(fx, GLYPH, &after, 0x01E0_1ABC);
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x40);
    if fx.mem.i16(COUNTER) >= 0x170 {
        let d = ee::div(ee::sub(fx.mem.f(sp + 0x44), 60.0), 58.0);
        let v = ee::add(fx.mem.f(LIFT), d);
        fx.mem.setf(LIFT, v);
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x40);
    let c = fx.mem.i16(COUNTER);
    if (0x19A..0x1C2).contains(&c) {
        fx.mem.set16(INDEX, 0);
        copy3(fx, D + 0x250, sp + 0x40);
        spawn(fx, RING, &after, 0x01E0_1B68);
        if fx.mem.i16(COUNTER) % 2 == 1 {
            spawn(fx, BAND, &after, 0x01E0_1BA0);
        }
    }
    if fx.mem.i16(COUNTER) == 0x142 {
        fx.mem.set16(INDEX, 0);
        spawn(fx, WIDE, &after, 0x01E0_1BC8);
        fx.mem.set16(INDEX, 1);
        spawn(fx, WIDE, &after, 0x01E0_1BE4);
    }
    if fx.mem.i16(COUNTER) == 0x135 {
        fx.mem.set16(INDEX, 0);
        spawn(fx, POINT, &after, 0x01E0_1C0C);
    }
    if fx.mem.i16(COUNTER) == 0x117 {
        fx.mem.set16(INDEX, 1);
        spawn(fx, POINT, &after, 0x01E0_1C38);
        fx.mem.set16(INDEX, 2);
        spawn(fx, POINT, &after, 0x01E0_1C54);
    }
    if fx.mem.i16(COUNTER) == 0x12A {
        fx.mem.set16(INDEX, 3);
        spawn(fx, POINT, &after, 0x01E0_1C80);
        fx.mem.set16(INDEX, 4);
        spawn(fx, POINT, &after, 0x01E0_1C9C);
    }
    if fx.mem.i16(COUNTER) == 0x16F {
        spawn(fx, ENDS, &after, 0x01E0_1CC0);
    }
    fx.leave(0x60);
}

/// D+0x250 = (0, 60, 0), copied to D+0x2D20 (xyz).
fn home(fx: &mut Fx) {
    fx.mem.write_u32(D + 0x250, 0);
    fx.mem.setf(D + 0x254, 60.0);
    fx.mem.write_u32(D + 0x258, 0);
    copy3(fx, HOME, D + 0x250);
}

/// 0x1E053F0(D, step, 0): b003's spawn: the first free task cleared and
/// set off at D+0x250 (as i16) with the colour D+0x44A4 (twice), +0x30 =
/// D+0x4A5C; `saves` the caller's s4..s0, `ra` the return address.
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

/// A step's frame (0x20: ra, s0 = D; 0x30 / 0x40 also the entry's s1 / s2
/// = 0).
fn frame(fx: &mut Fx, size: u32, ra: u32) {
    let sp = fx.enter(size);
    let ra = u64::from(ra);
    let d = u64::from(D);
    match size {
        0x20 => fx.saved(sp, &[(0x10, ra), (0, d)]),
        0x30 => fx.saved(sp, &[(0x20, ra), (0x10, 0), (0, d)]),
        _ => fx.saved(sp, &[(0x30, ra), (0x20, 0), (0x10, 0), (0, d)]),
    }
}

fn run_step(fx: &mut Fx, f: u32, t: u32, ra: u32) {
    let size = match f {
        RING | BAND | WIDE | POINT => 0x20,
        ORBIT => 0x40,
        _ => 0x30,
    };
    frame(fx, size, ra);
    match f {
        RING => ring(fx, t, RING, [0x20, 0x10, 0x10]),
        ORBIT => orbit(fx, t),
        TRAIL => trail(fx, t),
        ORB => orb(fx, t),
        GLYPH => glyph(fx, t),
        BAND => band(fx, t),
        WIDE => wide(fx, t),
        ENDS => ends(fx, t),
        _ => point(fx, t),
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

fn add_u8(fx: &mut Fx, at: u32, by: u8) {
    let v = fx.mem.read_u8(at);
    fx.mem.write_u8(at, v.wrapping_add(by));
}

/// The byte at `at` down by `down` while at least `floor`, else `end`
/// (the task ends) or 0.
fn fade(fx: &mut Fx, t: u32, at: u32, floor: u8, down: u8, end: bool) {
    let v = fx.mem.read_u8(t + at);
    if v >= floor {
        fx.mem.write_u8(t + at, v - down);
    } else if end {
        fx.mem.set16(t + 0x36, 0);
    } else {
        fx.mem.write_u8(t + at, 0);
    }
}

/// The camera's angles into +0x10 (0x205580; its frame saves the step's s0
/// = the task, s1 and `ra`), then x + 0x4000 when `turn`.
fn face(fx: &mut Fx, t: u32, s1: u32, ra: u32, turn: bool) {
    crate::b494::camera_angles_saving(fx, t + 0x10, u64::from(t), u64::from(s1), ra);
    if turn {
        let x = fx.mem.i32(t + 0x10).wrapping_add(0x4000);
        fx.mem.set32(t + 0x10, x);
    }
}

/// 0x1E01CE0 (D, task) at record address `at` (also b121's 0x1E023B0):
/// a ring of quads (size 60 growing by 8, colour `c` (b116: dark red (0x20,
/// 0x10, 0x10)), cell 0..6, row offset +0xA 0..7 (+0xC the slot), radius
/// 0..19 growing by 16..45, row +8 0..34, life +6 3..7, orientation +0x3A
/// 0..2; rand's returns at fixed offsets from `at`); alpha 0x80 for its
/// life, then down by 8.
pub(crate) fn ring(fx: &mut Fx, t: u32, at: u32, c: [i32; 3]) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x3C);
        let v = rmod(fx, t, at + 0x28, 7);
        fx.mem.set16(t + 0x34, v);
        l2::rgb(fx, t + 0x28, c[0], c[1], c[2]);
        let v = fx.rand_from(t, at + 0x54);
        fx.mem.set16(t + 0xA, v & 7);
        let s = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xC, s);
        for (o, ra, m, add) in [(0x30u32, 0x78u32, 0x14, 0), (0xE, 0x94, 0x1E, 0x10), (8, 0xB4, 0x23, 0), (6, 0xD0, 5, 3), (0x3A, 0xF0, 3, 0)] {
            let v = rmod(fx, t, at + ra, m);
            fx.mem.set16(t + o, v + add);
        }
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0xA {
            fx.mem.write_u8(t + 0x2B, a - 8);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    if age(fx, t) != 0 {
        let s = fx.mem.i16(t + 0x38) + 8;
        fx.mem.set16(t + 0x38, s);
    }
    if age(fx, t) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    grow(fx, t);
}

/// 0x1E02E90 (D, task): orb k's orbiter (size 32 / 13, white): its angle
/// +0x3A 0 / 0 / 180 / 90 / 270 by k, its radius D+0x4A20 + 4k 0 for k 0
/// else 13.2; alpha +0x2B / +0x2F up by 0x40 while its age is under 2.
/// From update 0x170 its orbit point D+0x2D80 + 0x10 k is (sin, cos) x
/// radius (+ D+0x4A34 in y), z drifting 2 by the facing until 0x19A, the
/// radius closing by 0.3 (while above 0), the age counted and the angle up
/// by 8; it faces the camera every update.
fn orbit(fx: &mut Fx, t: u32) {
    let k = fx.mem.i16(t + 0x30);
    let radius = RADII + 4 * k as u32;
    let o = ORBITS + 0x10 * k as u32;
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, if k == 0 { 0x20 } else { 0xD });
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x2C, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x36, 2);
        for (j, a) in [(1, 0), (2, 0xB4), (3, 0x5A), (4, 0x10E)] {
            if k == j {
                fx.mem.set16(t + 0x3A, a);
            }
        }
        fx.mem.write_u32(radius, if k == 0 { 0 } else { 0x4153_3333 });
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if age(fx, t) < 2 {
        add_u8(fx, t + 0x2B, 0x40);
        add_u8(fx, t + 0x2F, 0x40);
    }
    let late = fx.mem.i16(COUNTER) >= 0x170;
    if late {
        for (c, cos) in [(0u32, 0), (4, 1)] {
            let a = fx.mem.i16(t + 0x3A);
            let s = l2::sin_deg(fx, SINES, a, cos);
            let v = ee::mul(fx.mem.f(radius), s);
            fx.mem.setf(o + c, v);
        }
        let y = ee::add(fx.mem.f(o + 4), fx.mem.f(LIFT));
        fx.mem.setf(o + 4, y);
        if fx.mem.i16(COUNTER) < 0x19B {
            let z = fx.mem.f(o + 8);
            let z = if fx.mem.i16(YAW) != 0 { ee::sub(z, 2.0) } else { ee::add(z, 2.0) };
            fx.mem.setf(o + 8, z);
        }
        let r = fx.mem.f(radius);
        if !ee_le(r, 0.0) {
            fx.mem.setf(radius, ee::sub(r, f32::from_bits(0x3E99_999A)));
        }
        grow(fx, t);
    }
    face(fx, t, D, 0x01E0_3150, false);
    if fx.mem.i16(COUNTER) >= 0x170 {
        let a = fx.mem.i16(t + 0x3A) + 8;
        fx.mem.set16(t + 0x3A, a);
    }
}

/// `a <= b` as the EE's c.ole.s (no NaNs in play) [H].
fn ee_le(a: f32, b: f32) -> bool {
    a <= b
}

/// 0x1E03190 (D, task): orb k's trail: 8 points (growing by 2 to 48 from
/// update 0x16F), size 10; its 50-point history D+0x2E70 + 0x460 k filled
/// with the orbit point on its first update and pushed with it every
/// update (0x1E00080, newest first); alpha 0xC0 until update 0x1BF, then
/// down by 5 (the size down by 1 to 0); red (0xF0, 0x40, 0x40) on even
/// ages, yellow (0xF0, 0xF0, 0x40) on odd; the age counted from 0x16F.
fn trail(fx: &mut Fx, t: u32) {
    let k = fx.mem.i16(t + 0x30) as u32;
    let h = HISTORY + 0x460 * k;
    let o = ORBITS + 0x10 * k;
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 6, 8);
        fx.mem.write_u8(t + 0x2B, 0);
        for j in 0..0x31 {
            copy3(fx, h + 0x10 * j, o);
        }
        fx.mem.set16(t + 0x38, 0xA);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(COUNTER) >= 0x16F && fx.mem.i16(t + 6) < 0x30 {
        let n = fx.mem.i16(t + 6) + 2;
        fx.mem.set16(t + 6, n);
    }
    if fx.mem.i16(COUNTER) < 0x1BF {
        fx.mem.write_u8(t + 0x2B, 0xC0);
    } else {
        fade(fx, t, 0x2B, 7, 5, true);
        let s = fx.mem.i16(t + 0x38);
        if s > 0 {
            fx.mem.set16(t + 0x38, s - 1);
        }
    }
    if age(fx, t) % 2 == 0 {
        l2::rgb(fx, t + 0x28, 0xF0, 0x40, 0x40);
    } else {
        l2::rgb(fx, t + 0x28, 0xF0, 0xF0, 0x40);
    }
    bspawn::history(fx, h + 0x310, 0x32, o);
    if fx.mem.i16(COUNTER) >= 0x16F {
        grow(fx, t);
    }
}

/// 0x1E03360 (D, task): orb k's sprite (size 32 / 13, grey 0x80 / 0x60 by
/// its age, or by the update against k while its age is 0): alpha +0x2B /
/// +0x2F up by 10 to 0x80 for 12 updates, out by 7 from update 0x1C8;
/// over ages 13..42 its point D+0x2E20 + 0x10 k moves a 30th of (slot -
/// start) toward the slot; its age runs from update 0x10A (k 0), 0xEC (k
/// 1, 2) or 0x100 (k 3, 4); it faces the camera every update.
fn orb(fx: &mut Fx, t: u32) {
    let k = fx.mem.i16(t + 0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, if k == 0 { 0x20 } else { 0xD });
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x2C, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if age(fx, t) < 0xC {
        for at in [0x2Bu32, 0x2F] {
            let a = fx.mem.read_u8(t + at);
            if a < 0x80 {
                fx.mem.write_u8(t + at, a + 0xA);
            }
        }
    } else if fx.mem.i16(COUNTER) >= 0x1C8 {
        fade(fx, t, 0x2B, 9, 7, true);
    }
    let g = if age(fx, t) % 2 == 1 { 0x80 } else { 0x60 };
    l2::rgb(fx, t + 0x28, g, g, g);
    let a = age(fx, t);
    if (0xD..0x2B).contains(&a) {
        let i = 0x10 * k as u32;
        for c in [0u32, 4, 8] {
            let d = ee::div(ee::sub(fx.mem.f(SLOTS + i + c), fx.mem.f(STARTS + i + c)), 30.0);
            let v = ee::add(fx.mem.f(POINTS + i + c), d);
            fx.mem.setf(POINTS + i + c, v);
        }
    }
    if age(fx, t) == 0 {
        let g = if fx.mem.i16(COUNTER) % 2 == k % 2 { 0x80 } else { 0x60 };
        l2::rgb(fx, t + 0x28, g, g, g);
    }
    let c = fx.mem.i16(COUNTER);
    if c >= 0xC9 {
        let runs = match k {
            0 => c >= 0x10A,
            1 | 2 => c >= 0xEC,
            3 | 4 => c >= 0x100,
            _ => false,
        };
        if runs {
            grow(fx, t);
        }
    }
    face(fx, t, D, 0x01E0_36A0, false);
}

/// 0x1E036C0 (D, task): a glyph pair (size 50, white, quads 100 x 200,
/// +0xE = (+0x3A != 0)); +0x10 0x4000 (+0x18 0 for k 1); alpha +0x2B /
/// +0x2F up by 10 for 12 updates, out by 13 from update 0x147; from age
/// 21 +0x10 down by 0x200 to 0.
fn glyph(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x32);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x2C, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 8, 0x64);
        fx.mem.set16(t + 0xA, 0xC8);
        let e = i32::from(fx.mem.i16(t + 0x3A) != 0);
        fx.mem.set16(t + 0xE, e);
        fx.mem.set16(t + 0x36, 2);
        if fx.mem.i16(t + 0x30) == 1 {
            fx.mem.write_u32(t + 0x18, 0);
        }
        fx.mem.write_u32(t + 0x10, 0x4000);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if age(fx, t) < 0xC {
        add_u8(fx, t + 0x2B, 0xA);
        add_u8(fx, t + 0x2F, 0xA);
    } else if fx.mem.i16(COUNTER) >= 0x147 {
        fade(fx, t, 0x2B, 0xF, 0xD, true);
        fade(fx, t, 0x2F, 0xF, 0xD, false);
    }
    if age(fx, t) >= 0x15 {
        let v = fx.mem.i32(t + 0x10);
        fx.mem.set32(t + 0x10, if v > 0 { v - 0x200 } else { 0 });
    }
    grow(fx, t);
}

/// The rings' first update: size `size`, colours `c28` / `c2C`, a random
/// angle +0x3A (rand % 360 returning to `ra`), +0x2F 0.
fn ring_start(fx: &mut Fx, t: u32, size: i32, c28: [i32; 3], c2c: [i32; 3], ra: u32) {
    fx.mem.set16(t + 0x38, size);
    l2::rgb(fx, t + 0x28, c28[0], c28[1], c28[2]);
    l2::rgb(fx, t + 0x2C, c2c[0], c2c[1], c2c[2]);
    let v = rmod(fx, t, ra, 0x168);
    fx.mem.set16(t + 0x3A, v);
    fx.mem.set16(t + 0x36, 2);
    fx.mem.write_u8(t + 0x2F, 0);
}

const WHITE: [i32; 3] = [0xF0, 0xF0, 0xF0];
const AMBER: [i32; 3] = [0xF0, 0x80, 0x10];

/// 0x1E039A0 (D, task): a rising band at the target (radius 5 growing by
/// 10, +6 = radius - 5, white inside, amber out): alpha up by 0x60 for 2
/// updates, then down by 10; +0x10 0x4000.
fn band(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        ring_start(fx, t, 5, WHITE, AMBER, 0x01E0_39F4);
    }
    if age(fx, t) < 2 {
        add_u8(fx, t + 0x2B, 0x60);
    } else {
        fade(fx, t, 0x2B, 0x11, 0xA, true);
    }
    fx.mem.write_u32(t + 0x10, 0x4000);
    let s = fx.mem.i16(t + 0x38);
    fx.mem.set16(t + 6, s - 5);
    fx.mem.set16(t + 0x38, s + 10);
    grow(fx, t);
}

/// 0x1E03AA0 (D, task): k 0 a wide band (radius 500, white in, amber out,
/// alpha +0x2B), k 1 a narrower one (210, colours swapped, alpha +0x2F up
/// while +0x2B is under 0xC0): up by 0x18 to 0xC0 for 12 updates, then
/// down by 9; facing the camera turned 0x4000.
fn wide(fx: &mut Fx, t: u32) {
    let k = fx.mem.i16(t + 0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        if k == 0 {
            fx.mem.set16(t + 0x38, 0x1F4);
            l2::rgb(fx, t + 0x28, 0xF0, 0xF0, 0xF0);
            l2::rgb(fx, t + 0x2C, 0xF0, 0x80, 0x10);
        } else if k == 1 {
            fx.mem.set16(t + 0x38, 0xD2);
            l2::rgb(fx, t + 0x2C, 0xF0, 0xF0, 0xF0);
            l2::rgb(fx, t + 0x28, 0xF0, 0x80, 0x10);
        }
        let v = rmod(fx, t, 0x01E0_3B40, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if age(fx, t) < 0xC {
        if k == 0 && fx.mem.read_u8(t + 0x2B) < 0xC0 {
            add_u8(fx, t + 0x2B, 0x18);
        }
        if k == 1 && fx.mem.read_u8(t + 0x2B) < 0xC0 {
            add_u8(fx, t + 0x2F, 0x18);
        }
    } else {
        if k == 0 {
            fade(fx, t, 0x2B, 0xB, 9, true);
        }
        if k == 1 {
            fade(fx, t, 0x2F, 0xB, 9, true);
        }
    }
    face(fx, t, 0, 0x01E0_3C30, true);
    grow(fx, t);
}

/// 0x1E03C60 (D, task): the history ends' rings (radius 40, white in,
/// amber out): alpha up by 0x60 for 2 updates, out by 9 from update
/// 0x1B5; facing the camera turned 0x4000.
fn ends(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        ring_start(fx, t, 0x28, WHITE, AMBER, 0x01E0_3CBC);
    }
    if age(fx, t) < 2 {
        add_u8(fx, t + 0x2B, 0x60);
    } else if fx.mem.i16(COUNTER) >= 0x1B5 {
        fade(fx, t, 0x2B, 0xB, 9, true);
    }
    face(fx, t, D, 0x01E0_3D3C, true);
    grow(fx, t);
}

/// 0x1E03D70 (D, task): a point's ring (radius 100, white in, amber out):
/// alpha up by 0x60 for 2 updates, then down by 10; facing the camera
/// turned 0x4000.
fn point(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        ring_start(fx, t, 0x64, WHITE, AMBER, 0x01E0_3DC4);
    }
    if age(fx, t) < 2 {
        add_u8(fx, t + 0x2B, 0x60);
    } else {
        fade(fx, t, 0x2B, 0x11, 0xA, true);
    }
    face(fx, t, 0, 0x01E0_3E30, true);
    grow(fx, t);
}

/// 0x1E009A0(D, task, mode, pos): b258's sprite (one textured FST quad of
/// half size i16 +0x38 at `pos`, scale 0.1, coloured +0x28, UVs D+0x94 in
/// orientation D+0xBE, texture D+0x88; ALPHA by (mode >> 4) & 15, depth
/// mode & 15) facing the camera through the task's own angles: 0x205580
/// into +0x10 (its frame: s0 = the descriptor, s1 = the mode) and +0x10 as
/// the rotation, no turn about z; a 0x1B0 frame (fdiff vs b258's: the
/// frame offsets, the angles and the dropped 0x1099E0) [F].
fn sprite(fx: &mut Fx, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x1B0);
    copy4(fx, sp + 0x50, pos);
    let d = sp + 0x118;
    fx.mem.write_u16(d + 2, 0x24);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, (4u64 << 32) | 0x2424_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let alpha = match (mode >> 4) & 0xF {
        0 => 0x80_0000_0042u64,
        1 => 0x80_0000_0044,
        _ => 0x80_0000_0048,
    };
    fx.desc_alpha(d, alpha);
    crate::c061::rgbaq_at(fx, sp + 0x1A8, t + 0x28, d + 0x10);
    let order = fx.mem.i16(D + 0xBE);
    uv::uv_rect(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xB0 + 4 * k, 0x3DCC_CCCD);
    }
    crate::b494::camera_angles_saving(fx, t + 0x10, u64::from(d), mode as i64 as u64, 0x01E0_0AEC);
    copy3(fx, sp + 0xA0, sp + 0x50);
    l2::xform_vp(fx, sp + 0xD0, sp + 0xB0, t + 0x10, sp + 0xA0);
    let s = ee::from_i32(fx.mem.i16(t + 0x38));
    l2::quad_xy(fx, sp + 0x60, s, s);
    if l2::project(fx, sp + 0x60, SPR, sp + 0xD0, d, sp + 0xC0) {
        let ot = fx.mem.read_u32(D + 0x80);
        match mode & 0xF {
            0 => {
                let w = fx.mem.f(sp + 0xC0);
                fx.fix_residue(w, 0x01E0_0B68);
                let key = ee::fixunssfsi(w) as i32;
                fx.submit(d, ot, key);
            }
            1 => {
                l2::far_z(fx, &[d + 0x50, d + 0x40, d + 0x30, d + 0x20]);
                fx.submit(d, ot, 0);
            }
            _ => fx.submit(d, ot, -1),
        }
    }
    fx.leave(0x1B0);
}

/// 0x1E058A0(D, task, d, pos): a mirrored pair of textured STQ quads (the
/// strip3 header 0x109B50 with FST cleared, all four colours +0x28) of
/// size (+8, +0xA) standing on y 0 at `pos` (scale 0.1, turned by +0x10):
/// UVs D+0x94 / 256 (u reversed for k != 0), the second quad's top
/// mirrored (y negated, x swapped); each shifted by +-D+0x20 in x and
/// +-D+0x24 in y (0x107950); projected (0x205210) and keyed by the mean w
/// on D+0x80 [F].
fn pair(fx: &mut Fx, t: u32, d: u32, pos: u32) {
    use ee::{div, from_i32, neg};
    let sp = fx.enter(0x140);
    copy4(fx, sp + 0x60, pos);
    l2::desc_strip3(fx, d);
    for (local, to) in [(0x120u32, 0x10u32), (0x128, 0x28), (0x130, 0x40), (0x138, 0x58)] {
        crate::c061::rgbaq_at(fx, sp + local, t + 0x28, d + to);
    }
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f & 0xF7FF);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xC0 + 4 * k, 0x3DCC_CCCD);
    }
    copy3(fx, sp + 0xB0, sp + 0x60);
    l2::xform_vp(fx, sp + 0xE0, sp + 0xC0, t + 0x10, sp + 0xB0);
    let kind = fx.mem.i16(t + 0x30);
    for k in 0..2 {
        let (u, v) = (fx.mem.i16(D + 0x94), fx.mem.i16(D + 0x96));
        let (w, h) = (fx.mem.i16(D + 0x98), fx.mem.i16(D + 0x9A));
        let (u0, u1) = if kind == 0 { (u, u + w) } else { (u + w, u) };
        let u0 = div(from_i32(u0), 256.0);
        fx.mem.setf(d + 0x48, u0);
        fx.mem.setf(d + 0x18, u0);
        let u1 = div(from_i32(u1), 256.0);
        fx.mem.setf(d + 0x60, u1);
        fx.mem.setf(d + 0x30, u1);
        let v0 = div(from_i32(v), 256.0);
        fx.mem.setf(d + 0x34, v0);
        fx.mem.setf(d + 0x1C, v0);
        let v1 = div(from_i32(v + h), 256.0);
        fx.mem.setf(d + 0x64, v1);
        fx.mem.setf(d + 0x4C, v1);
        let (qw, qh) = (from_i32(fx.mem.i16(t + 8)), from_i32(fx.mem.i16(t + 0xA)));
        l2::quad_xy(fx, sp + 0x70, qw, qh);
        fx.mem.write_u32(sp + 0xA4, 0);
        fx.mem.write_u32(sp + 0x94, 0);
        if k == 1 {
            let y = ee::mul(fx.mem.f(sp + 0x84), -1.0);
            fx.mem.setf(sp + 0x84, y);
            fx.mem.setf(sp + 0x74, y);
            let (x0, x1, x2, x3) = (fx.mem.read_u32(sp + 0x70), fx.mem.read_u32(sp + 0x80), fx.mem.read_u32(sp + 0x90), fx.mem.read_u32(sp + 0xA0));
            fx.mem.write_u32(sp + 0x70, x1);
            fx.mem.write_u32(sp + 0x80, x0);
            fx.mem.write_u32(sp + 0x90, x3);
            fx.mem.write_u32(sp + 0xA0, x2);
        }
        let (dx, dy) = (fx.mem.f(D + 0x20), fx.mem.f(D + 0x24));
        let (sx, sy) = match (kind == 0, k == 0) {
            (true, true) => (dx, dy),
            (true, false) => (neg(dx), neg(dy)),
            (false, true) => (neg(dx), dy),
            (false, false) => (dx, neg(dy)),
        };
        l2::add_column(fx, sp + 0x70, 0, sx);
        l2::add_column(fx, sp + 0x70, 1, sy);
        let m = fx.read_m4(sp + 0xE0);
        if fx.project_desc(sp + 0x70, SPR, Some(&m), d, sp + 0xD0) {
            let w = fx.mem.f(sp + 0xD0);
            fx.fix_residue(w, 0x01E0_5C88);
            let key = ee::fixunssfsi(w) as i32;
            let ot = fx.mem.read_u32(D + 0x80);
            fx.submit(d, ot, key);
        }
    }
    fx.leave(0x140);
}
