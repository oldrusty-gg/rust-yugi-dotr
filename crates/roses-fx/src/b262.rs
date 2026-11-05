//! BMODULE b262 (card 631), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E013F0, data area 0x1ED0000) [F]: a chain of 14
//! billboards from the attacker's node 0x26 to the target, each a pair of
//! glow discs and a ring of 64 turning quads, laid down one segment per
//! update with a random jitter; a quad burst and a disc at the attacker; a
//! flickering full-screen tint; the stage light lowered to 0x20 while it
//! runs. Returns 1 on update 26, 6 from 36 to 199, 5 on 200, 8 after 220.
//!
//! Library routines from [`crate::fxlib`]: `disc` (b262 0x1E00280),
//! `quads_setup` (0x1E007E0), `quads_draw` (0x1E00940); its own: the screen
//! tint (0x1E00120, shared by 46 records) and the chain (0x1E00F50).
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib::ALPHA_ADD;

const D: u32 = crate::bmodule::DATA_AREA;
/// The quad corner template (rodata 0x1E01A00).
const QUAD_TEMPLATE: u32 = 0x01E0_1A00;

/// Colour bytes (B, G, R, A) at `at`.
fn colour(fx: &mut Fx, at: u32, b: u8, g: u8, r: u8, a: u8) {
    fx.mem.write_u8(at + 2, r);
    fx.mem.write_u8(at + 1, g);
    fx.mem.write_u8(at, b);
    fx.mem.write_u8(at + 3, a);
}

fn rect(fx: &mut Fx, at: u32, x: i32, y: i32, w: i32, h: i32) {
    fx.mem.set16(at, x);
    fx.mem.set16(at + 2, y);
    fx.mem.set16(at + 4, w);
    fx.mem.set16(at + 6, h);
}

/// The entry 0x1E013F0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xA0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x5520);
    fx.xform_init(D);
    fx.xform_init(D + 0xB0);
    fx.xform_init(D + 0x120);
    for k in 0..16u32 {
        fx.xform_init(D + 0x600 + 0x70 * k);
    }
    let h = fx.self_handle();
    let r = fx.request_node(h, 0x26, 1);
    fx.mem.set32(D + 0x5518, r);
    let h = fx.self_handle();
    let r = fx.request_node(h, 0x21, 0);
    fx.mem.set32(D + 0x551C, r);
    fx.mem.set32(D + 0x5508, 0xDC);
    fx.mem.set32(D + 0x550C, 1);
    fx.mem.set32(D + 0x5510, param);
    fx.mem.set32(D + 0x5514, 0x80);
    colour(fx, D + 0x5500, 0x80, 0x80, 0x80, 0x40);
    colour(fx, D + 0x5504, 0x80, 0x80, 0x80, 0x60);
    // The disc at the attacker (+0x5F0): radius 3, 36 triangles.
    fx.mem.set32(D + 0x5F0, 0);
    colour(fx, D + 0x5F4, 0x70, 0xA0, 0xD0, 0);
    fx.mem.setf(D + 0x5F8, 3.0);
    fx.mem.set16(D + 0x5FC, 0x24);
    fx.mem.set16(D + 0x5FE, 0);
    // The chain's discs: +0x5300 + 16 k (radius 16), +0x5400 + 16 k (radius
    // 5, drawn twice: bit 0x1C).
    for k in 0..16u32 {
        let a = D + 0x5300 + 16 * k;
        fx.mem.set32(a, 0);
        colour(fx, a + 4, 0x70, 0xA0, 0xD0, 0xFF);
        fx.mem.setf(a + 8, 16.0);
        fx.mem.set16(a + 0xC, 0x24);
        fx.mem.set16(a + 0xE, 0);
        let b = D + 0x5400 + 16 * k;
        fx.mem.set32(b, 0x1000_0000);
        colour(fx, b + 4, 0x70, 0xA0, 0xD0, 0xFF);
        fx.mem.setf(b + 8, 5.0);
        fx.mem.set16(b + 0xC, 0x24);
        fx.mem.set16(b + 0xE, 0);
    }
    let tex = fx.texture(4, 15);
    fx.mem.write_u32(sp + 0x90, tex);
    // The burst at the attacker (+0x190) and the chain's quad rings
    // (+0xD00 + 0x460 k): FST cell (142, 0, 34, 64).
    fx.mem.set32(D + 0x194, 1);
    rect(fx, D + 0x1A0, 0x8E, 0, 0x22, 0x40);
    colour(fx, D + 0x198, 0x70, 0xA0, 0xD0, 0);
    let tex = fx.mem.read_u32(sp + 0x90);
    fx.quads_setup(D + 0x190, tex, f32::from_bits(0x3E99_999A), QUAD_TEMPLATE);
    for k in 0..16u32 {
        let g = D + 0x460 * k + 0xD00;
        fx.mem.set32(g + 4, 1);
        rect(fx, g + 0x10, 0x8E, 0, 0x22, 0x40);
        colour(fx, g + 8, 0x80, 0x80, 0x80, 0x30);
        let tex = fx.mem.read_u32(sp + 0x90);
        fx.quads_setup(g, tex, f32::from_bits(0x3F4C_CCCD), QUAD_TEMPLATE);
    }
    // The chain (+0x70): 14 segments, jitter 100, laid one per update.
    let g = D + 0x70;
    let tex = fx.mem.read_u32(sp + 0x90);
    fx.mem.write_u32(g, tex);
    fx.mem.set32(g + 4, 0x100_0000);
    rect(fx, g + 0x10, 0xA, 0, 0x2A, 0x80);
    colour(fx, g + 8, 0x80, 0x80, 0x80, 0x20);
    fx.mem.set16(g + 0xC, 0xE);
    fx.mem.set16(g + 0x30, 0);
    fx.mem.set16(g + 0x32, 1);
    fx.mem.set16(g + 0x34, 0xE);
    fx.mem.set16(g + 0xE, 0x64);
    fx.mem.write_u32(g + 0x20, D + 0x600);
    fx.mem.write_u32(g + 0x24, D + 0xD00);
    fx.mem.write_u32(g + 0x28, D + 0x5300);
    fx.mem.write_u32(g + 0x2C, D + 0x5400);
    let a = fx.mem.read_u8(D + 0xD0B);
    fx.mem.write_u8(g + 0x36, a);
    let a = fx.mem.read_u8(D + 0x5307);
    fx.mem.write_u8(g + 0x37, a);
    let a = fx.mem.read_u8(D + 0x5407);
    fx.mem.write_u8(g + 0x38, a);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let h = fx.self_handle();
    let slot = fx.mem.i32(D + 0x5518);
    fx.node_position(h, slot, D + 0x140);
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0x80);
    fx.xform_world(D);
    fx.xform_billboard(D + 0xB0);
    fx.xform_billboard(D + 0x120);
    for k in 0..16u32 {
        fx.xform_billboard(D + 0x600 + 0x70 * k);
    }
    if fx.mem.i32(D + 0x550C) < 200 {
        fx.step_down(D + 0x5514, 0x20, 4);
    } else {
        fx.step_up(D + 0x5514, 0x80, 4);
    }
    let light = fx.mem.u8(D + 0x5514);
    fx.set_stage_light(light);
    let n = fx.mem.i32(D + 0x550C);
    if (25..200).contains(&n) {
        let v = ((fx.rand() & 0x3F) + 0x40) as u8;
        for at in [0x198u32, 0x5F4, 0x5500, 0x199, 0x5F5, 0x5501, 0x19A, 0x5F6, 0x5502] {
            fx.mem.write_u8(D + at, v);
        }
        tint(fx, D + 0x5500);
        fx.disc(D + 0x5F0, D + 0x120);
        fx.step_up(D + 0x5F7, 0xFF, 0x10);
        fx.quads_draw(D + 0x190, D + 0x120);
        fx.step_up(D + 0x19B, 0x20, 2);
        for k in 0..64u32 {
            for c in 0..3u32 {
                let a = D + 0x1F0 + 16 * k + 4 * c;
                let v = fx.mem.i32(a).wrapping_add(0x80);
                fx.mem.set32(a, v);
            }
        }
        for k in 0..16u32 {
            for (src, dsts) in [(0x19Au32, [0x5406u32, 0x5306, 0xD0A]), (0x199, [0x5405, 0x5305, 0xD09]), (0x198, [0x5404, 0x5304, 0xD08])] {
                let v = fx.mem.read_u8(D + src);
                fx.mem.write_u8(D + dsts[0] + 16 * k, v);
                fx.mem.write_u8(D + dsts[1] + 16 * k, v);
                fx.mem.write_u8(D + dsts[2] + 0x460 * k, v);
            }
        }
        chain(fx, D + 0x70, D + 0x140, sp + 0x80);
    }
    let n = fx.mem.i32(D + 0x550C) + 1;
    fx.mem.set32(D + 0x550C, n);
    let left = fx.mem.i32(D + 0x5508) - 1;
    fx.mem.set32(D + 0x5508, left);
    if n == 200 {
        5
    } else if n == 26 {
        1
    } else if (36..200).contains(&n) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}

/// b262 0x1E00120 [65049dfc9548: 46 records]: a full-window additive quad
/// (12.4 (0x7000, 0x7000) .. (0x9000, 0x9000), the far Z) of the colour at
/// `c` (B, G, R, A bytes).
pub(crate) fn tint(fx: &mut Fx, c: u32) {
    let sp = fx.enter(0x70);
    let d = sp + 0x30;
    fx.desc_init(d, 5, 4, 0x4_4441);
    fx.desc_alpha(d, ALPHA_ADD);
    let (b2, b1, b0, b3) = (fx.mem.read_u8(c + 2), fx.mem.read_u8(c + 1), fx.mem.read_u8(c), fx.mem.read_u8(c + 3));
    fx.mem.write_u8(sp + 0x40, b2);
    fx.mem.write_u8(sp + 0x41, b1);
    fx.mem.write_u8(sp + 0x42, b0);
    fx.mem.write_u8(sp + 0x43, b3);
    fx.mem.setf(sp + 0x44, 1.0);
    for (at, x, y) in [(0x48u32, 0x7000u16, 0x7000u16), (0x50, 0x9000, 0x7000), (0x58, 0x7000, 0x9000), (0x60, 0x9000, 0x9000)] {
        fx.mem.write_u16(sp + at, x);
        fx.mem.write_u16(sp + at + 2, y);
        fx.set_xyz_z(sp + at, 0xFF_FFFF);
        fx.mem.write_u8(sp + at + 7, 0);
    }
    fx.submit(d, OT, 0);
    fx.leave(0x70);
}

/// b262 0x1E00F50 (3 records): the chain group `g` (+0xC segments, max 16;
/// +0xE jitter; +0x20 transform records, 0x70 apart; +0x24 quad rings,
/// 0x460 apart; +0x28 / +0x2C discs, 16 apart; phase +0x30, its step +0x32
/// and its hold +0x34; fades +0x36..+0x38): on phase 0 every segment's
/// record placed along `from` -> `to` (with the jitter), then the first
/// phase / step + 1 segments drawn, their quads turned, faded with bit 0x18.
pub(crate) fn chain(fx: &mut Fx, g: u32, from: u32, to: u32) {
    let sp = fx.enter(0xD0);
    let w = fx.mem.i16(g + 0x34);
    let a37 = fx.mem.u8(g + 0x37);
    let a38 = fx.mem.u8(g + 0x38);
    let a36 = fx.mem.u8(g + 0x36);
    let (q, _) = fx.div(a37, w);
    fx.mem.write_u8(sp + 0xB0, q as u8);
    let q1 = if w == 0 { 0 } else { a38 / w };
    fx.mem.write_u8(sp + 0xC0, q1 as u8);
    let (q2, _) = fx.div(a36, w);
    let fade36 = q2 & 0xFF;
    if !(fx.mem.i16(g + 0xC) < 0x11) {
        fx.mem.set16(g + 0xC, 0x10);
    }
    if fx.mem.i16(g + 0x30) == 0 {
        let d = [
            ee::sub(fx.mem.f(to), fx.mem.f(from)),
            ee::sub(fx.mem.f(to + 4), fx.mem.f(from + 4)),
            ee::sub(fx.mem.f(to + 8), fx.mem.f(from + 8)),
        ];
        let mut k = 0;
        while k < fx.mem.i16(g + 0xC) {
            let count = fx.mem.i16(g + 0xC);
            let t = ee::div(ee::from_i32(k + 1), ee::from_i32(count));
            let jitter = fx.mem.i16(g + 0xE);
            let rec = fx.mem.read_u32(g + 0x20).wrapping_add((0x70 * k) as u32);
            for c in 0..3u32 {
                let base = ee::add(fx.mem.f(from + 4 * c), ee::mul(d[c as usize], t));
                let v = if jitter != 0 {
                    let j = fx.mem.i16(g + 0xE);
                    let r = fx.rand();
                    let (_, m) = fx.div(r, j.wrapping_mul(2));
                    ee::add(base, ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(m.wrapping_sub(j))))
                } else {
                    base
                };
                let rec = fx.mem.read_u32(g + 0x20).wrapping_add((0x70 * k) as u32);
                fx.mem.setf(rec + 0x20 + 4 * c, v);
            }
            let _ = rec;
            if k == 0 {
                let r0 = fx.mem.read_u32(g + 0x20);
                fx.xform_billboard(r0);
            }
            if crate::exe::bit(fx.mem.read_u32(g + 4), 0x18) {
                let v = fx.mem.read_u8(g + 0x37);
                let a = fx.mem.read_u32(g + 0x28).wrapping_add((16 * k) as u32);
                fx.mem.write_u8(a + 7, v);
                let v = fx.mem.read_u8(g + 0x38);
                let a = fx.mem.read_u32(g + 0x2C).wrapping_add((16 * k) as u32);
                fx.mem.write_u8(a + 7, v);
                let v = fx.mem.read_u8(g + 0x36);
                let a = fx.mem.read_u32(g + 0x24).wrapping_add((0x460 * k) as u32);
                fx.mem.write_u8(a + 0xB, v);
            }
            k += 1;
        }
    }
    let (p30, p32) = (fx.mem.i16(g + 0x30), fx.mem.i16(g + 0x32));
    let (q, _) = fx.div(p30, p32);
    let mut n = q + 1;
    let count = fx.mem.i16(g + 0xC);
    if count < n {
        n = count;
    }
    for k in 0..n.max(0) {
        let rec = fx.mem.read_u32(g + 0x20).wrapping_add((0x70 * k) as u32);
        let da = fx.mem.read_u32(g + 0x28).wrapping_add((16 * k) as u32);
        fx.disc(da, rec);
        let db = fx.mem.read_u32(g + 0x2C).wrapping_add((16 * k) as u32);
        let rec = fx.mem.read_u32(g + 0x20).wrapping_add((0x70 * k) as u32);
        fx.disc(db, rec);
        let qg = fx.mem.read_u32(g + 0x24).wrapping_add((0x460 * k) as u32);
        let rec = fx.mem.read_u32(g + 0x20).wrapping_add((0x70 * k) as u32);
        fx.quads_draw(qg, rec);
        if crate::exe::bit(fx.mem.read_u32(g + 4), 0x18) {
            let a = fx.mem.read_u32(g + 0x28).wrapping_add((16 * k) as u32);
            let s = fx.mem.u8(sp + 0xB0);
            fx.step_down(a + 7, 0, s);
            let a = fx.mem.read_u32(g + 0x2C).wrapping_add((16 * k) as u32);
            let s = fx.mem.u8(sp + 0xC0);
            fx.step_down(a + 7, 0, s);
            let a = fx.mem.read_u32(g + 0x24).wrapping_add((0x460 * k) as u32);
            fx.step_down(a + 0xB, 0, fade36);
        }
        for j in 0..64u32 {
            for c in 0..3u32 {
                let a = fx.mem.read_u32(g + 0x24).wrapping_add((0x460 * k) as u32) + 16 * j + 0x60 + 4 * c;
                let v = fx.mem.i32(a).wrapping_add(0x80);
                fx.mem.set32(a, v);
            }
        }
    }
    let p = fx.mem.i16(g + 0x30) + 1;
    fx.mem.set16(g + 0x30, p);
    let limit = fx.mem.i16(g + 0x34).wrapping_add(fx.mem.i16(g + 0x32).wrapping_mul(fx.mem.i16(g + 0xC)));
    if limit < fx.mem.i16(g + 0x30) {
        fx.mem.set16(g + 0x30, 0);
    }
    fx.leave(0xD0);
}

