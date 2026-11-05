//! BMODULE b282 (card 512), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02B20, data area 0x1ED0000, 0xB630 bytes) [F]
//!: b465's program (card 281) on b750's spawn
//! (0x1E03290: 400 tasks of 0x70 bytes at D+0x2A0, alive +0x58, step +0x44
//! through `jalr`, the kind D+0xB2B0 into +0x50 / +0x52 and the angle
//! D+0xB2A2 into +0x5A) with b180's unit flash (0x1E03750, the flash tasks
//! 0x1E026B0, called every update). Nodes: the attacker's 3 / 4 / 0xD /
//! 0xE / 0x2F / 0x2C (D+0xB244..), the target's 1 (D+0xB254); timer 0xBE.
//!
//! Update 1: the stage light (0x1E025A0: b465's down to 0x30 for 0x97
//! updates, back up from timer 0x1E), the flash (0x1E026B0: grey 0x60 /
//! 0x50 / 0x40 / 0x30 / 0x20 / 0x30 / 0x40 / 0x50 by age % 8, in by 9 for
//! 0x78 updates, out by 7) and four red crosses (0x1E01DB0, angles 0 / 90
//! / 45 / 135, kinds 0 / 0 / 1 / 1, drawn by `quads4::sprite_quads` at
//! the attacker's nodes 0xD and 0xE). On 0x1C / 0x26 / 0x2C thirty blue
//! stars (0x1E01F60, b465's star 1 x 30 rising 3) and a small ring
//! (0x1E01C20) at the attacker's position at node 3's height - 1. On
//! 0x6C..0xA0 a large ring (0x1E01A70, growing) at node 3; on 0x79 ten
//! small rings, an arc (0x1E02480, `billboard::arc_strips`, turned by the
//! camera + 0x4000 and growing 0.4) and thirty larger stars (0x1E021F0, 5
//! x 100 rising 10) there; on 0x86..0xA0 a small ring at node 4, and on
//! 0x90 ten large rings, an arc and thirty larger stars at node 4. The
//! rings by `billboard::ring_quads` (cell 0x14, texture (5, 0)), the stars
//! by `billboard::quad_cross` (cell 0x9B). Returns 1 on 0x6F, 4 on 0xB9,
//! 2 on 0xA0, 6 on 0x7A..0x9A, 8 once the timer D+0xB210 has run out.
use crate::b076::node_position_saving;
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
const POS: u32 = D + 0x250;
const RING: u32 = 0x01E0_1A70;
const RING_S: u32 = 0x01E0_1C20;
const CROSS: u32 = 0x01E0_1DB0;
const STAR: u32 = 0x01E0_1F60;
const STAR_L: u32 = 0x01E0_21F0;
const ARC: u32 = 0x01E0_2480;
const LIGHT: u32 = 0x01E0_25A0;
/// The flash tasks, run by the unit flash 0x1E03750.
const FLASH: u32 = 0x01E0_26B0;
/// The return address of the entry's `jalr` into a step (0x1E02E20 + 8).
const TASK_RA: u64 = 0x01E0_2E28;
const TEX: u32 = D + 0xB1F0;
const TIMER: u32 = D + 0xB210;
const COLOUR: u32 = D + 0xB214;
const LAYER: u32 = D + 0xB224;
const FLASH_DESC: u32 = D + 0xB228;
const NODES: u32 = D + 0xB244;
const ANGLE: u32 = D + 0xB2A2;
const KIND: u32 = D + 0xB2B0;
const FACING: u32 = D + 0xB2B4;
const COUNTER: u32 = D + 0xB2B8;
const BYTES: u32 = D + 0xB2BA;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02B20 (s0 = 0 / the live count, s1 = D, s2 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, D + 0xB4BC);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2B6, param);
    fx.mem.set32(TIMER, 0xBE);
    for (k, node) in [3, 4, 0xD, 0xE, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB254, r);
    for k in 0..2u32 {
        let t = fx.texture(4, 0xE + k as i32);
        fx.mem.write_u32(D + 0x298 + 4 * k, t);
    }
    let t = fx.texture(5, 0);
    fx.mem.write_u32(TEX, t);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.fix_residue(y, 0x01E0_2D0C);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_2D3C, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_2D8C);
        fx.mem.set16(D + 0xB264 + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    // a3 as the last call left it, for the unit flash's first 0x207BB0.
    let mut a3 = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 || f == FLASH {
            continue;
        }
        // The step's prologue: the entry's s0 = 0.
        let s = fx.enter(0x20);
        fx.saved(s, &[(0x10, TASK_RA), (0, 0)]);
        match f {
            RING => ring(fx, t, RING, true, &mut a3),
            RING_S => ring(fx, t, RING_S, false, &mut a3),
            CROSS => cross(fx, t, &mut a3),
            STAR => star(fx, t, STAR, false, &mut a3),
            STAR_L => star(fx, t, STAR_L, true, &mut a3),
            ARC => arc(fx, t, &mut a3),
            LIGHT => light(fx, t, &mut a3),
            _ => panic!("b282: unknown task step {f:#x}"),
        }
        fx.leave(0x20);
    }
    // The entry's s0: the live tasks counted in the draw loop.
    let mut live = 0u32;
    let last_xyz2 = |fx: &mut Fx, before: usize| match fx.out.prims.get(before..).and_then(|p| p.last()) {
        Some(p) => {
            let d = p.desc;
            let n = u32::from(fx.mem.read_u16(d));
            fx.mem.read_u32(d + 8 + 8 * n)
        }
        None => 0,
    };
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        live += 1;
        for (f, local, site) in [(RING, 0x50u32, 0x01E0_2ECCu32), (RING_S, 0x60, 0x01E0_2F70)] {
            if fx.mem.read_u32(t + 0x44) != f {
                continue;
            }
            tex(fx, TEX);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, t, sp + local, 0x14, sub);
            fx.mem.set16(D + 0xB8, 4);
            let v = fx.rand_from(live, site + 8);
            fx.mem.set16(D + 0xBE, v % 4);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x21, BYTES);
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == CROSS {
            tex(fx, D + 0x29C);
            cell(fx, t, sp + 0x70, 0x9A, 0);
            fx.mem.set16(D + 0xBE, 0);
            for (n, ra) in [(2u32, 0x01E0_3024u32), (3, 0x01E0_3058)] {
                let h = fx.self_handle();
                let slot = fx.mem.i16(NODES + 2 * n);
                node_position_saving(fx, h, slot, t + 0x10, u64::from(live), Some(u64::from(D)), ra);
                let before = fx.out.prims.len();
                quads4::sprite_quads(fx, D, t, 0, t + 0x10);
                a3 = last_xyz2(fx, before);
            }
        }
        for (f, local) in [(STAR, 0x80u32), (STAR_L, 0x90)] {
            if fx.mem.read_u32(t + 0x44) != f {
                continue;
            }
            tex(fx, D + 0x29C);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, t, sp + local, 0x9B, sub);
            fx.mem.set16(D + 0xBE, 0);
            fx.mem.set16(D + 0xB0, 1);
            billboard::quad_cross(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
            a3 = 0;
        }
        if fx.mem.read_u32(t + 0x44) == ARC {
            fx.mem.set16(D + 0xB8, 0x24);
            let before = fx.out.prims.len();
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            a3 = last_xyz2(fx, before);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    unit_flash(fx, a3);
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x6F => 1,
        0xB9 => 4,
        0xA0 => 2,
        0x7A..=0x9A => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

fn tex(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

/// The atlas cell `id` frame `sub` (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, _t: u32, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// 0x1E03290(D, step, 0) (b750's spawn): the first free task cleared and
/// set off at D+0x250 (i16 +0, float +0x10) with the colour D+0xB214
/// (twice), kind D+0xB2B0 (+0x50 / +0x52), angle D+0xB2A2 (+0x5A). Its
/// frame saves the emitter's s0 = D / s1, the entry's s2 = the parameter,
/// s3 / s4 the runner's (0 [H]) and `ra`.
fn spawn(fx: &mut Fx, step: u32, s1: u32, param: i32, ra: u32) -> i32 {
    let f = fx.enter(0x60);
    fx.saved(f, &[(0, u64::from(D)), (0x10, u64::from(s1)), (0x20, param as i64 as u64), (0x30, 0), (0x40, 0), (0x50, u64::from(ra))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x2F8 + 0x70 * k) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for o in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + o + 3, a);
            }
            l2::to_i16x4(fx, t, POS, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(POS + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let s = fx.mem.i16(KIND);
            fx.mem.set16(t + 0x50, s);
            let s = fx.mem.i16(KIND);
            fx.mem.set16(t + 0x52, s);
            let a = fx.mem.i16(ANGLE);
            fx.mem.set16(t + 0x5A, a);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, step);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// The attacker's node `k` into `out` (0x1F6830's frame: the emitter's
/// s0 = D / s1).
fn node(fx: &mut Fx, k: u32, out: u32, s1: u32, ra: u32) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES + 2 * k);
    node_position_saving(fx, h, slot, out, u64::from(D), Some(u64::from(s1)), ra);
}

/// 0x1E015A0(D, 0): the update counter and the spawns (frame 0x50: ra,
/// the entry's s1 = D / s0 = 0; then s0 = D, s1 a loop count).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, 0x01E0_2DE8), (0x10, u64::from(D)), (0, 0)]);
    let mut s1 = D;
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    if c(fx) == 1 {
        spawn(fx, LIGHT, s1, param, 0x01E0_15F4);
    }
    if c(fx) == 1 {
        spawn(fx, FLASH, s1, param, 0x01E0_1620);
    }
    if c(fx) == 1 {
        node(fx, 2, POS, s1, 0x01E0_1658);
        for (angle, kind, ra) in [(0, 0, 0x01E0_1684u32), (0x5A, 0, 0x01E0_16B4), (0x2D, 1, 0x01E0_16E8), (0x87, 1, 0x01E0_171C)] {
            fx.mem.set16(ANGLE, angle);
            fx.mem.set16(KIND, kind);
            spawn(fx, CROSS, s1, param, ra);
        }
    }
    if [0x1C, 0x26, 0x2C].contains(&c(fx)) {
        let h = fx.self_handle();
        fx.unit_position(h, POS);
        fx.saved(sp - 0x20, &[(0x10, 0x01E0_1760), (0, u64::from(D))]);
        node(fx, 0, sp + 0x30, s1, 0x01E0_1780);
        let y = ee::sub(fx.mem.f(sp + 0x34), 1.0);
        fx.mem.setf(POS + 4, y);
        fx.mem.set16(KIND, 0);
        s1 = 0;
        while s1 < 0x1E {
            spawn(fx, STAR, s1, param, 0x01E0_17BC);
            s1 += 1;
        }
        spawn(fx, RING_S, s1, param, 0x01E0_17F0);
    }
    if (0x6C..0xA1).contains(&c(fx)) {
        node(fx, 0, POS, s1, 0x01E0_1834);
        spawn(fx, RING, s1, param, 0x01E0_1848);
    }
    for (at, nk, ra_node, small, rings, ra_rings, ra_arc, ra_stars) in [
        (0x79, 0u32, 0x01E0_1880u32, RING_S, 0x01E0_189Cu32, 0, 0x01E0_18D0u32, 0x01E0_18F8u32),
        (0x90, 1, 0x01E0_19B0, RING, 0x01E0_19CC, 0, 0x01E0_1A00, 0x01E0_1A28),
    ] {
        let _ = ra_rings;
        if at == 0x90 && (0x86..0xA1).contains(&c(fx)) {
            node(fx, 1, POS, s1, 0x01E0_1964);
            spawn(fx, RING_S, s1, param, 0x01E0_1978);
        }
        if c(fx) == at {
            node(fx, nk, POS, s1, ra_node);
            s1 = 0;
            while s1 < 0xA {
                spawn(fx, small, s1, param, rings);
                s1 += 1;
            }
            spawn(fx, ARC, s1, param, ra_arc);
            fx.mem.set16(KIND, 0);
            s1 = 0;
            while s1 < 0x1E {
                spawn(fx, STAR_L, s1, param, ra_stars);
                s1 += 1;
            }
        }
    }
    fx.leave(0x50);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn fade_in(fx: &mut Fx, t: u32, at: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + at);
    if a < cap {
        fx.mem.write_u8(t + at, a.wrapping_add(up));
    }
}

/// Byte `at` down by `down` while at least `floor`, else the task ends.
fn fade_out(fx: &mut Fx, t: u32, at: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + at);
    if a >= floor {
        fx.mem.write_u8(t + at, a.wrapping_sub(down));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E01A70 (`large`) / 0x1E01C20 (D, task): a grey ring of quads: size
/// rand % 50 + 50 (C0) / rand % 20 + 10 (80), cell row 0..2, radius rand %
/// 40 + 10, a random angle, growth rand % 20 + 20 / + 16, row 0..34, life
/// 7..11; in by 0x40 under 0xC0, out by 14; the large one's radius grows
/// by its growth from age 1.
fn ring(fx: &mut Fx, t: u32, f: u32, large: bool, a3: &mut u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let (s, grey, g) = if large { ((0x32, 0x32), 0xC0, 0x14) } else { ((0x14, 0xA), 0x80, 0x10) };
        let v = rmod(fx, t, f + 0x24, s.0);
        fx.mem.setf(t + 0x48, ee::from_i32(v + s.1));
        let v = rmod(fx, t, f + 0x50, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, grey, grey, grey);
        *a3 = grey as u32;
        let v = rmod(fx, t, f + 0x7C, 0x28);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, f + 0x9C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0xB8, 0x14);
        fx.mem.set16(t + 0xE, v + g);
        let v = rmod(fx, t, f + 0xD8, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0xF4, 5);
        fx.mem.set16(t + 6, v + 7);
        fx.mem.set16(t + 0x58, 2);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a >= 0x10 {
        fx.mem.write_u8(t + 0x63, a - 0xE);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if large && fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E01DB0 (D, task): a red cross (FF, 60, 60) / (FF, 40, 40), size
/// +0x48 6, by kind +0x50 (3, 50) / (3, 20) / (8, 400) at +0x30 / +0x34;
/// in by 0x40 under 0x80 for 4 updates, out by 9; from age 1 kind 0 grows
/// 15 / kind 1 5 a update in +0x34, +0x52 counts to 25.
fn cross(fx: &mut Fx, t: u32, a3: &mut u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0x60, 0x60);
        l2::rgb(fx, t + 0x64, 0xFF, 0x40, 0x40);
        *a3 = 0x40;
        for (kind, w, h) in [(0, 3.0f32, 50.0f32), (1, 3.0, 20.0), (2, 8.0, 400.0)] {
            if fx.mem.i16(t + 0x50) == kind {
                fx.mem.setf(t + 0x30, w);
                fx.mem.setf(t + 0x34, h);
            }
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.set16(t + 0x52, 0);
    }
    if fx.mem.i16(t + 0x54) < 4 {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    for (kind, k) in [(0, 15.0f32), (1, 5.0)] {
        if fx.mem.i16(t + 0x50) == kind && fx.mem.i16(t + 0x54) != 0 {
            let v = ee::add(fx.mem.f(t + 0x34), k);
            fx.mem.setf(t + 0x34, v);
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let n = fx.mem.i16(t + 0x52);
        if n < 0x19 {
            fx.mem.set16(t + 0x52, n + 1);
        }
    }
    age(fx, t);
}

/// 0x1E01F60 / 0x1E021F0 (`large`) (D, task): b465's star in blue (0x40,
/// 0x40, 0xFF): width 1 / 5, height 30 / 100 growing by 10 / 15 and
/// rising 3 / 10 a update (+0xC through 0x1A2B80), cell 2, kind +0x52
/// 0..3, random angles +0x20..+0x28, life 10; in by 0x40 under 0x80, out
/// by 9.
fn star(fx: &mut Fx, t: u32, f: u32, large: bool, a3: &mut u32) {
    // b282's larger star has one more instruction before its first rand.
    let r = |o: u32| f + o + if large { 4 } else { 0 };
    if fx.mem.i16(t + 0x58) == 1 {
        let (w, h) = if large { (5, 0x64) } else { (1, 0x1E) };
        fx.mem.set16(t + 8, w);
        fx.mem.set16(t + 0xA, h);
        fx.mem.set16(t + 0xC, 0);
        fx.mem.set16(t + 0x56, 2);
        l2::rgb(fx, t + 0x60, 0x40, 0x40, 0xFF);
        *a3 = 0xFF;
        fx.mem.set16(t + 0x58, 2);
        for (o, ra) in [(0x34u32, r(0x54)), (0x30, r(0x80)), (0x38, r(0xAC))] {
            let v = rmod(fx, t, ra, 0x64);
            fx.mem.setf(t + o, ee::from_i32(v - 0x32));
        }
        for o in [0x34u32, 0x30, 0x38] {
            let q = ee::div(fx.mem.f(t + o), 20.0);
            fx.mem.setf(t + o, q);
        }
        for (o, ra) in [(4u32, r(0x104)), (2, r(0x124)), (0, r(0x144))] {
            let v = fx.rand_from(t, ra);
            fx.mem.set16(t + o, v % 0x800 - 0x400);
        }
        let v = fx.rand_from(t, r(0x164));
        fx.mem.set16(t + 0x52, (v >> 4) % 4);
        for (o, ra) in [(0x20u32, r(0x184)), (0x24, r(0x1A0)), (0x28, r(0x1BC))] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 6, 0xA);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let (dh, dy, ra) = if large { (0xF, 10.0f32, r(0x264)) } else { (0xA, 3.0, r(0x264)) };
        let h = fx.mem.i16(t + 0xA) + dh;
        fx.mem.set16(t + 0xA, h);
        let y = ee::add(ee::from_i32(fx.mem.i16(t + 0xC)), dy);
        fx.fix_residue(y, ra);
        fx.mem.set16(t + 0xC, ee::fixsfsi(y));
    }
    age(fx, t);
}

/// 0x1E02480 (D, task): an arc (inner (0x80, 0x80, 0xFF), outer (0xC0,
/// 0xC0, 0xC0)) of radius +0x4C 200 growing 0.4 a update, 10 x 10,
/// turned by the camera (0x205580 into +0x20) + 0x4000; its outer alpha
/// up by 0x30 under 0xC0 for 10 updates, then down by 15.
fn arc(fx: &mut Fx, t: u32, a3: &mut u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x4348_0000);
        l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xFF);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0xA {
        fade_in(fx, t, 0x67, 0xC0, 0x30);
    } else {
        fade_out(fx, t, 0x67, 0x11, 0xF);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2540);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = ee::add(fx.mem.f(t + 0x4C), f32::from_bits(0x3ECC_CCCD));
        fx.mem.setf(t + 0x4C, r);
    }
    age(fx, t);
    *a3 = 0;
}

/// 0x1E025A0 (D, task): b465's stage light (0x1F7F10) down by 10 to 0x30
/// for 0x97 updates, back up by 10 to 0x80 once the timer is under 30;
/// 0x80 and the task's end when the timer is at 3.
fn light(fx: &mut Fx, t: u32, a3: &mut u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x97 {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x31 { 0x30 } else { l - 0xA });
    } else if fx.mem.i32(TIMER) < 0x1E {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x80 { l + 0xA } else { 0x80 });
        if fx.mem.i16(t + 0x50) >= 0x81 {
            fx.mem.set16(t + 0x50, 0x80);
        }
    }
    age(fx, t);
    if fx.mem.i32(TIMER) == 3 {
        fx.set_stage_light(0x80);
        fx.mem.set16(t + 0x58, 0);
    } else {
        let l = fx.mem.read_u8(t + 0x50);
        fx.set_stage_light(i32::from(l));
    }
    *a3 = 0;
}

/// 0x1E026B0(D, t): b180's flash step in greys by age % 8 (0x60, 0x50,
/// 0x40, 0x30, 0x20, 0x30, 0x40, 0x50; 0x60 at its start): alpha up by 9
/// below 0x80 for 0x78 updates, then down by 7. `regs` the unit flash's
/// s0 / s1 (the slot index, the live count).
fn flash(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_385C), (0x20, u64::from(regs[1])), (0x10, u64::from(regs[0]))]);
    // swc1 f20 / f21: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 4, 0);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let h = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0x60, 0x60, 0x60);
        fx.mem.set16(t + 0x58, 2);
    }
    let n = fx.mem.i16(t + 0x54);
    let r = n & 7;
    let r = if n < 0 && r != 0 { r - 8 } else { r };
    const GREYS: [i32; 8] = [0x60, 0x50, 0x40, 0x30, 0x20, 0x30, 0x40, 0x50];
    if (0..8).contains(&r) {
        let g = GREYS[r as usize];
        l2::rgb(fx, t + 0x60, g, g, g);
    }
    let d = fx.mem.read_u32(FLASH_DESC);
    for k in 0..4u32 {
        let b = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(d + 0x10 + k, b);
    }
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    for (at, xy) in [(0x20u32, 0x6FF8u16), (0x30, 0x8FF8)] {
        fx.mem.write_u16(d + at, xy);
        fx.mem.write_u16(d + at + 2, xy);
        let v = fx.mem.read_u64(d + at);
        fx.mem.write_u64(d + at, (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
        fx.mem.write_u8(d + at + 7, 0);
    }
    for (o, v) in [(0x18u32, ee::div(w, 512.0)), (0x1C, ee::div(h, 512.0)), (0x28, ee::div(ee::sub(512.0, w), 512.0)), (0x2C, ee::div(ee::sub(512.0, h), 512.0))] {
        fx.mem.setf(d + o, v);
    }
    fx.submit(d, 0, 0);
    if fx.mem.i16(t + 0x54) < 0x78 {
        fade_in(fx, t, 0x63, 0x80, 9);
    } else {
        fade_out(fx, t, 0x63, 9, 7);
    }
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E03750(D) (b180's 0x1E02DF0 word for word but the step): in a packet
/// layer (0x2078A0) the attacker drawn into the target (0x1F8270 filter
/// 0; ALPHA 0x48), TEXFLUSH before and after; every flash task stepped
/// (0x1E026B0); the layer linked into the ordering table at key 0 when
/// any task was alive.
fn unit_flash(fx: &mut Fx, a3: u32) {
    let sp = fx.enter(0xB0);
    let d = sp + 0x58;
    fx.mem.write_u32(FLASH_DESC, d);
    let layer = fx.layer_begin();
    fx.mem.write_u32(LAYER, layer);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.texflush([5, d, 0x26, a3]);
    fx.desc_alpha(d, 0x80_0000_0048);
    let h = fx.self_handle();
    let (_, regs) = fx.unit_draw(h, 0, sp + 0x98, sp + 0xA0);
    fx.texflush(regs);
    let mut live = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        live += 1;
        if fx.mem.read_u32(t + 0x44) == FLASH {
            flash(fx, t, [k, live]);
        }
    }
    if live != 0 {
        let layer = fx.mem.read_u32(LAYER);
        fx.layer_end(layer, OT, 0);
    }
    fx.leave(0xB0);
}
