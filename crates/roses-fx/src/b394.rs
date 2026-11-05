//! BMODULE b394 (card 394), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02060, data area 0x1ED0000) [F]: b615's family (a pool of 300 tasks of 0x70 bytes at D+0x2A0,
//! alive +0x58, step +0x44 called through `jalr`) on another layout, drawn
//! with rings of quads (7369422df7f8 at 0x1E00E70, `billboard::ring_quads`)
//! and flat facing rings turned about z (2f5a28a746a2 at 0x1E00920,
//! `quads4::ring_facing_flat_z`), UVs by uv_rect 0x1E00080. From update 96
//! to 474 two quad rings a update leave the attacker's nodes 0x13 / 0x25
//! (white on odd ages); a point riding node 0x13 until 155 then drifting
//! to the target's node 1 by 186; a frame flash (0x207EF0) from 152; facing
//! rings from nodes 0x40 / 0x48 on 221..239 and 292..309; three closing
//! rings a update at random offsets on 292..409. Returns 1 on update 290, 4
//! on 540, 2 on 535, 6 on 301..529, 8 once the 570-update timer +0x8670
//! has run out.
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 300;
/// The step functions a task holds (+0x44), as the record's addresses.
const QUADS: u32 = 0x01E0_1870;
const RING: u32 = 0x01E0_1AA0;
const CLOSING: u32 = 0x01E0_1C50;
const FLASH: u32 = 0x01E0_1E40;
/// The return address of the entry's `jalr` into a step (0x1E023A8 + 8).
const TASK_RA: u64 = 0x01E0_23B0;
const TEX: u32 = D + 0x8630;
const TIMER: u32 = D + 0x8670;
const COLOUR: u32 = D + 0x8674;
const NODES: u32 = D + 0x86A4;
const TARGET_NODE: u32 = D + 0x86B4;
const SLOT: u32 = D + 0x86D2;
const FACING: u32 = D + 0x86D6;
const COUNTER: u32 = D + 0x86DA;
const BYTES: u32 = D + 0x86DC;
const BYTES2: u32 = D + 0x87DC;
const SINES: u32 = D + 0x88DC;
/// The drifting point (+0x8610) and its step (+0x8620).
const POINT: u32 = D + 0x8610;

/// The entry 0x1E02060.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    fx.mem.write_u32(D + 0x867C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x90);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x8A50);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x86D8, param);
    fx.mem.set32(TIMER, 0x23A);
    for (k, node) in [0x13, 0x25, 0x40, 0x48, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x80);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x84)), 180.0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    // The entry's s0 is D here (rand's frame).
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_2280);
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_22DC);
        let (_, h) = fx.div(v, 0x14);
        fx.mem.set8(BYTES + k, h + 4);
    }
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_232C);
        let (_, h) = fx.div(v, 0x14);
        fx.mem.set8(BYTES2 + k, h - 0xA);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        match f {
            0 => {}
            QUADS => quads(fx, t),
            RING => ring(fx, t, RING, 0x20, false),
            CLOSING => ring(fx, t, CLOSING, 0x30, true),
            FLASH => flash(fx, t),
            _ => panic!("b394: unknown task step {f:#x}"),
        }
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x44) == QUADS {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x50, 0x14, sub);
            fx.mem.set16(D + 0xB8, 4);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        for (f, r) in [(RING, 0x60u32), (CLOSING, 0x70)] {
            if fx.mem.read_u32(t + 0x44) != f {
                continue;
            }
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + r, 0x14, sub);
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 1);
            let o = fx.mem.i16(t + 0xA);
            fx.mem.set16(D + 0xC0, o);
            for c in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * c);
                fx.mem.write_u32(D + 0x20 + 4 * c, w);
            }
            quads4::ring_facing_flat_z(fx, uv::uv_rect, D, t, 0x20, BYTES2);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x122 => 1,
        0x21C => 4,
        0x217 => 2,
        0x12D..=0x211 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` / `sub` (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// rand % m (HI) from a step's `jal` at `site` (rand's frame: s0 = `s0`).
fn rmod(fx: &mut Fx, s0: u32, site: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, site + 8);
    fx.div(v, m).1
}

/// 0x1E01420(D, 0): the update counter and the emitter (frame 0x50: ra,
/// the entry's s1 = 0, s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, 0x01E0_2380), (0x10, 0), (0, u64::from(D))]);
    let p = param as i64 as u64;
    // The spawn's prologue saves: the emitter's s0 / s1 = D and the
    // entry's s2 = the parameter (s3 / s4 0).
    let saves = |ra: u32, s0: u64| [(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, p), (0x10, u64::from(D)), (0, s0)];
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = fx.mem.i16(COUNTER);
    if c < 0x9B {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        fx.node_position(h, slot, POINT);
    } else {
        if c == 0x9B {
            let h = fx.target_handle();
            let slot = fx.mem.i16(TARGET_NODE);
            fx.node_position(h, slot, sp + 0x30);
            for (o, s) in [(8u32, 0x18u32), (4, 0x14)] {
                let v = ee::div(ee::sub(fx.mem.f(sp + 0x30 + o), fx.mem.f(POINT + o)), 32.0);
                fx.mem.setf(POINT + s, v);
            }
        }
        if fx.mem.i16(COUNTER) < 0xBA {
            for o in [8u32, 4] {
                let v = ee::add(fx.mem.f(POINT + o), fx.mem.f(POINT + 0x10 + o));
                fx.mem.setf(POINT + o, v);
            }
        }
    }
    if fx.mem.i16(COUNTER) == 0x98 {
        spawn(fx, FLASH, &saves(0x01E0_15A4, u64::from(D)));
    }
    for (lo, hi, step, pairs) in [
        (0x60, 0x1DB, QUADS, [(0u32, 0x01E0_15FCu32), (2, 0x01E0_1654)]),
        (0xDD, 0xF0, RING, [(4, 0x01E0_16AC), (6, 0x01E0_16E0)]),
        (0x124, 0x136, RING, [(4, 0x01E0_1738), (6, 0x01E0_176C)]),
    ] {
        let c = fx.mem.i16(COUNTER);
        if c < lo || c >= hi {
            continue;
        }
        for (node, ra) in pairs {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + node);
            fx.node_position(h, slot, D + 0x250);
            spawn(fx, step, &saves(ra, u64::from(D)));
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x124..0x19A).contains(&c) {
        for k in 0..3u32 {
            let v = rmod(fx, k, 0x01E0_1798, 0x3C);
            fx.mem.setf(D + 0x250, ee::from_i32(v - 0x1E));
            let v = rmod(fx, k, 0x01E0_17C4, 0x28);
            fx.mem.setf(D + 0x254, ee::from_i32(v + 0x14));
            let v = rmod(fx, k, 0x01E0_17F0, 0x14);
            fx.mem.setf(D + 0x258, ee::from_i32(v - 0xA));
            spawn(fx, CLOSING, &saves(0x01E0_182C, u64::from(k)));
        }
    }
    fx.leave(0x50);
}

/// 0x1E026B0(D, step, 0): the first free of the 300 tasks cleared and set
/// off at D+0x250 (as i16 and its xyz at +0x10) with the colour D+0x8674
/// (+0x60, +0x64), +0x50 = D+0x86D2; 1 when one was free. `saves`: its
/// prologue's.
fn spawn(fx: &mut Fx, step: u32, saves: &[(u32, u64)]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
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
            l2::to_i16x4(fx, t, D + 0x250, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(D + 0x250 + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let s = fx.mem.i16(SLOT);
            fx.mem.set16(t + 0x50, s);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, step);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// The step frame (`size`: ra +0x10, the entry's s0 = D +0).
fn frame(fx: &mut Fx, size: u32) {
    let sp = fx.enter(size);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
}

/// 0x1E01870 (D, task): a quad ring (frame 0x20): size +0x48 3, cell row
/// 0..7, white, random angles +0x20..+0x28, radius +0x50 0..14 growing by
/// 1..3 (after the first update), angle +0x5A, row 1..8, life 2..4; in by
/// 0x2A under 0x80, then out by 11; white on odd ages, else grey.
fn quads(fx: &mut Fx, t: u32) {
    frame(fx, 0x20);
    let f = QUADS;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4040_0000);
        let v = fx.rand_from(t, f + 0x28);
        fx.mem.set16(t + 0x56, v % 8);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        for (o, site) in [(0x20u32, 0x54u32), (0x24, 0x70), (0x28, 0x8C)] {
            let v = rmod(fx, t, f + site, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        let v = rmod(fx, t, f + 0xA8, 0xF);
        fx.mem.set16(t + 0x50, v);
        let v = rmod(fx, t, f + 0xC4, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0xE0, 3);
        fx.mem.set16(t + 0xE, v + 1);
        let v = fx.rand_from(t, f + 0x108);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, t, f + 0x120, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        let a = fx.mem.read_u8(t + 0x63);
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x2A);
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xD {
            fx.mem.write_u8(t + 0x63, a - 0xB);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let c = if fx.mem.i16(t + 0x54) % 2 == 1 { 0xFF } else { 0x80 };
    l2::rgb(fx, t + 0x60, c, c, c);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E01AA0 / 0x1E01C50 (D, task) at `f` (frames 0x20 / 0x30): a facing
/// ring: size +0x48 4 growing by 2, row 0, grey, orientation 0..7, the
/// spawn's slot in +0xC, radius from 0 growing by 5..12 (slot 1: 9..20),
/// row 0..34, life 3..7 (closing: 25..29, moving +0x38 a update so it
/// reaches the target's z in 25); alpha 0x80, then out by 8.
fn ring(fx: &mut Fx, t: u32, f: u32, size: u32, closing: bool) {
    frame(fx, size);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4080_0000);
        fx.mem.set16(t + 0x56, 0);
        fx.rand_from(t, f + 0x2C);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = fx.rand_from(t, f + 0x50);
        fx.mem.set16(t + 0xA, v % 8);
        let w = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, w);
        fx.mem.set16(t + 0x50, 0);
        let v = fx.rand_from(t, f + 0x78);
        fx.mem.set16(t + 0xE, v % 8 + 5);
        if fx.mem.i16(t + 0xC) == 1 {
            let v = rmod(fx, t, f + 0xA4, 0xC);
            fx.mem.set16(t + 0xE, v + 9);
        }
        let v = rmod(fx, t, f + 0xC8, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0xE4, 5);
        fx.mem.set16(t + 6, v + if closing { 0x19 } else { 3 });
        if closing {
            let sp = fx.sp;
            let h = fx.target_handle();
            // Its frame (s0 = t, ra) is a ring's missing-texture TEX0.
            crate::b076::unit_position_saving(fx, h, sp + 0x20, u64::from(t), 0x01E0_1D68);
            let v = ee::div(ee::sub(fx.mem.f(sp + 0x28), fx.mem.f(t + 0x18)), 25.0);
            fx.mem.setf(t + 0x38, v);
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xA {
            fx.mem.write_u8(t + 0x63, a - 8);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let g = ee::add(fx.mem.f(t + 0x48), 2.0);
        fx.mem.setf(t + 0x48, g);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
        if closing {
            let z = ee::add(fx.mem.f(t + 0x18), fx.mem.f(t + 0x38));
            fx.mem.setf(t + 0x18, z);
        }
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(size);
}

/// 0x1E01E40 (D, task): the frame flash (frame 0x90): a window sprite
/// (0x6FF8 .. 0x8FF8) sampling the frame (0x207EF0) at ST +0x48 / 512 ..
/// (512 - +0x48) / 512 (+0x48 = 0), colour (0xA0, 0x70, 0xA0, alpha
/// +0x63), ALPHA 0x44, key 0; +0x5A turning by 12; in by 6 under 0x60
/// until update 470, then out by 3 (the task ends below 5).
fn flash(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x90);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        fx.mem.set16(t + 0x5A, 0x5A);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u8(sp + 0x48, 0xA0);
    fx.mem.write_u8(sp + 0x49, 0x70);
    fx.mem.write_u8(sp + 0x4A, 0xA0);
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
    fx.mem.setf(sp + 0x50, ee::div(s, 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x54, ee::div(s, 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x60, ee::div(ee::sub(512.0, s), 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x64, ee::div(ee::sub(512.0, s), 512.0));
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    let a = fx.mem.i16(t + 0x5A) + 0xC;
    fx.mem.set16(t + 0x5A, a);
    let a = fx.mem.i16(t + 0x5A);
    if a >= 0x168 {
        let (_, h) = fx.div(a, 0x168);
        fx.mem.set16(t + 0x5A, h);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(COUNTER) < 0x1D6 {
        if a < 0x60 {
            fx.mem.write_u8(t + 0x63, a + 6);
        }
    } else if a >= 5 {
        fx.mem.write_u8(t + 0x63, a - 3);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x90);
}
