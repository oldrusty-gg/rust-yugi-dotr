//! BMODULE b467 (card 282), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03420, data area 0x1ED0000) [F] (helper C6, 31
//! Oct 2026): two glowing swaths (+0x70, +0x1A0: `blib::swath`) trailing
//! the attacker's node pairs 0x25 / 0x20 and 0x1D / 0x17 from update 0x19,
//! brightening until update 0x6A, then fading; the quad trail (+0x2D0)
//! pushed at nodes 0x2D (updates 0x42 / 0x4F) and 0x39 (0x37 / 0x4D),
//! drawn until update 0x60 with its cells fading; a ring (+0x1A70) at node
//! 0x14 aimed at node 0xF, drawn on updates 0x5A..0x69 and swelling; from
//! update 0x6A at the target a wave of squares (+0x21D0), a disc (+0x19F0)
//! and a quad burst (+0x1590), fading; the frame quad every update.
//! Returns 1 on update 0x60, 6 on 0x6A..0x81, 2 on 0x82, 8 after 0x96.
//!
//! Library at its own addresses: the frame quad (0x1E00120), `ring_setup`
//! / `ring_draw` (0x1E00240 / 0x1E00500), `swath::push` / `draw`
//! (0x1E009F0 / 0x1E00B40), `disc` (0x1E01340), `wave::draw` (0x1E018A0),
//! `quads_setup` / `quads_draw` (0x1E020A0 / 0x1E02200), `quadtrail::push`
//! / `draw` (0x1E02810 / 0x1E02A10).
use crate::blib::{quadtrail, swath, wave};
use crate::bus::Bus;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// The quad corner template (rodata, 0x1E020A0's lui / addiu).
const QUAD_TEMPLATE: u32 = 0x01E0_3E00;
const TRAIL_DRAW: u32 = 0x01E0_2A10;

/// The entry 0x1E03420.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xD0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xD0);
    ret
}

fn node(fx: &mut Fx, at: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(D + at);
    fx.node_position(h, s, out);
}

fn bytes(fx: &mut Fx, at: &[(u32, u8)]) {
    for &(o, v) in at {
        fx.mem.write_u8(D + o, v);
    }
}

fn halves(fx: &mut Fx, at: &[(u32, i32)]) {
    for &(o, v) in at {
        fx.mem.set16(D + o, v);
    }
}

/// A swath record at `r` with outputs `a` / `b`, both trails filled with
/// nodes `na` / `nb` (8 points).
fn swath_init(fx: &mut Fx, sp: u32, r: u32, a: u32, b: u32, na: u32, nb: u32, pa: u32, pb: u32) {
    let tex = fx.mem.read_u32(sp + 0xC0);
    fx.mem.write_u32(D + r, tex);
    fx.mem.write_u32(D + r + 4, 1);
    halves(fx, &[(r + 0xC, 8), (r + 0x110, 0x8E), (r + 0x112, 0x54), (r + 0x114, 0x20), (r + 0x116, 1)]);
    bytes(fx, &[(r + 0xA, 0xD0), (r + 9, 0xD0), (r + 8, 0x70), (r + 0xB, 0)]);
    fx.mem.write_u32(D + r + 0x120, D + a);
    fx.mem.write_u32(D + r + 0x124, D + b);
    node(fx, na, sp + pa);
    node(fx, nb, sp + pb);
    for k in 0..8u32 {
        let p = D + r + 0x10 + 0x10 * k;
        for c in [0u32, 4, 8] {
            let v = fx.mem.read_u32(sp + pa + c);
            fx.mem.write_u32(p + c, v);
        }
        for c in [0u32, 4, 8] {
            let v = fx.mem.read_u32(sp + pb + c);
            fx.mem.write_u32(p + 0x80 + c, v);
        }
    }
    fx.mem.set16(D + r + 0xE, 0);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2A30);
    for x in [0u32, 0x1520, 0x1A00, 0x1E70] {
        fx.xform_init(D + x);
    }
    for (at, n, clear) in [(0x220Cu32, 0x14, 1), (0x2210, 0xF, 0), (0x2214, 0x1D, 0), (0x2218, 0x17, 0), (0x221C, 0x25, 0), (0x2220, 0x20, 0), (0x2224, 0x39, 0), (0x2228, 0x2D, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x2200, 0x96);
    fx.mem.set32(D + 0x2204, 1);
    fx.mem.set32(D + 0x2208, param);
    for k in 0..0x100u32 {
        let r = fx.rand();
        fx.mem.write_u8(D + 0x292C + k, (r >> 4) as u8);
    }
    // The disc (+0x19F0).
    fx.mem.write_u32(D + 0x19F0, 0x8_0000);
    bytes(fx, &[(0x19F6, 0xD0), (0x19F5, 0xA0), (0x19F4, 0x70), (0x19F7, 0x80)]);
    fx.mem.write_u32(D + 0x19F8, 0x41F0_0000);
    halves(fx, &[(0x19FC, 0x24)]);
    fx.texture4(sp + 0xC0, 0xF);
    halves(fx, &[(0x19FE, 0)]);
    // The ring (+0x1A70).
    fx.mem.write_u32(D + 0x1A74, 0);
    halves(fx, &[(0x1A80, 0x8E), (0x1A82, 0), (0x1A84, 0x20), (0x1A86, 0x40)]);
    bytes(fx, &[(0x1A7A, 0xD0), (0x1A79, 0xD0), (0x1A78, 0x70), (0x1A7B, 0x20)]);
    let tex = fx.mem.read_u32(sp + 0xC0);
    fx.ring_setup(D + 0x1A70, tex, f32::from_bits(0x3DCC_CCCD), 12.0, 0.0, 14.0);
    // The quad burst (+0x1590).
    fx.mem.write_u32(D + 0x1594, 0x8_0001);
    halves(fx, &[(0x15A0, 0x8E), (0x15A2, 0), (0x15A4, 0x22), (0x15A6, 0x40)]);
    bytes(fx, &[(0x159A, 0xD0), (0x1599, 0xA0), (0x1598, 0x70), (0x159B, 0x20)]);
    let tex = fx.mem.read_u32(sp + 0xC0);
    fx.quads_setup(D + 0x1590, tex, 2.0, QUAD_TEMPLATE);
    // The wave (+0x21D0).
    let tex = fx.mem.read_u32(sp + 0xC0);
    fx.mem.write_u32(D + 0x21D0, tex);
    fx.mem.write_u32(D + 0x21DC, D + 0x292C);
    fx.mem.write_u32(D + 0x21D4, 0x2000_0000);
    halves(fx, &[(0x21E0, 0xB0), (0x21E2, 0x40), (0x21E4, 0x20), (0x21E6, 0x20)]);
    bytes(fx, &[(0x21DA, 0x80), (0x21D9, 0x80), (0x21D8, 0x80), (0x21DB, 0xFF)]);
    fx.mem.write_u32(D + 0x21F0, 0x3FCC_CCCD);
    fx.mem.write_u32(D + 0x21F4, 0x3CF5_C28F);
    halves(fx, &[(0x21FA, 0), (0x21F8, 0x100)]);
    fx.mem.write_u8(D + 0x21FC, 7);
    // The swaths.
    swath_init(fx, sp, 0x70, 0x25AC, 0x276C, 0x221C, 0x2220, 0x70, 0x80);
    swath_init(fx, sp, 0x1A0, 0x222C, 0x23EC, 0x2214, 0x2218, 0x90, 0xA0);
    // The quad trail (+0x2D0).
    let tex = fx.mem.read_u32(sp + 0xC0);
    fx.mem.write_u32(D + 0x2D0, tex);
    bytes(fx, &[(0x2D6, 0x80), (0x2D5, 0x80), (0x2D4, 0x80), (0x2D7, 0x30)]);
    fx.mem.write_u32(D + 0x2D8, 0x1_9003);
    fx.mem.write_u32(D + 0x2DC, D + 0x320);
    halves(fx, &[(0x2E0, 0x80), (0x2E2, 0xC0), (0x2E4, 0x20), (0x2E6, 0x20), (0x2E8, 0x80), (0x2EA, 0xC0), (0x2EC, 0x80), (0x2EE, 0x100)]);
    fx.mem.write_u32(D + 0x2F0, 0);
    fx.mem.write_u32(D + 0x2F4, 0x3E99_999A);
    fx.mem.write_u32(D + 0x2F8, 0);
    fx.mem.write_u32(D + 0x300, 0x4040_0000);
    fx.mem.write_u32(D + 0x304, 0x3E4C_CCCD);
    fx.mem.write_u32(D + 0x308, 0x4053_3333);
    halves(fx, &[(0x312, 0), (0x314, 0x18), (0x316, 0x24), (0x318, 0), (0x31A, 8), (0x31C, 8), (0x31E, 0)]);
    for x in [0x1A08u32, 0x1A04, 0x1A00] {
        fx.mem.write_u32(D + x, 0);
    }
    for x in [0x1E78u32, 0x1E74, 0x1E70] {
        fx.mem.write_u32(D + x, 0x3DCC_CCCD);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x2204);
    let pushed = n(fx) < 0x6A;
    if pushed {
        halves(fx, &[(0x1AC, 8), (0x7C, 8)]);
        node(fx, 0x221C, sp + 0x70);
        node(fx, 0x2220, sp + 0x80);
        node(fx, 0x2214, sp + 0x90);
        node(fx, 0x2218, sp + 0xA0);
        swath::push(fx, D + 0x70, sp + 0x70, sp + 0x80);
        swath::push(fx, D + 0x1A0, sp + 0x90, sp + 0xA0);
    }
    node(fx, 0x220C, D + 0x1A20);
    node(fx, 0x220C, sp + 0x50);
    node(fx, 0x2210, sp + 0x60);
    fx.look_angles(sp + 0x60, sp + 0x50, D + 0x1A10);
    if n(fx) == 0x6A {
        let t = fx.target_handle();
        fx.unit_position(t, D + 0x1540);
        let t = fx.target_handle();
        fx.unit_position(t, D + 0x1E90);
    }
    if matches!(n(fx), 0x42 | 0x4F) {
        node(fx, 0x2228, sp + 0xB0);
        quadtrail::push(fx, D + 0x2D0, sp + 0xB0);
    }
    if matches!(n(fx), 0x37 | 0x4D) {
        node(fx, 0x2224, sp + 0xB0);
        quadtrail::push(fx, D + 0x2D0, sp + 0xB0);
    }
    // s0 = D, s1 = the OT, s2 = sp + 0x60, s3 = sp + 0xA0 when the swaths
    // were pushed (else the entry's 0).
    let regs = [Some(D), Some(OT), Some(sp + 0x60), Some(if pushed { sp + 0xA0 } else { 0 })];
    fx.xform_world_saving(D, 0x01E0_3B48, regs);
    fx.xform_billboard_saving(D + 0x1520, 0x01E0_3B50, regs);
    fx.xform_world_saving(D + 0x1A00, 0x01E0_3B58, regs);
    fx.xform_billboard_saving(D + 0x1E70, 0x01E0_3B60, regs);
    fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    if n(fx) >= 0x6A {
        wave::draw(fx, OT, D + 0x21D0, D + 0x1E70);
        fx.step_down(D + 0x21DB, 0, 0x10);
        fx.disc(D + 0x19F0, D + 0x1520);
        fx.step_down(D + 0x19F7, 0, 8);
        fx.quads_draw(D + 0x1590, D + 0x1520);
        fx.step_down(D + 0x159B, 0, 2);
        fx.approach(D + 0x1520, 3.0, f32::from_bits(0x3E4C_CCCD));
        fx.approach(D + 0x1E70, f32::from_bits(0x3E99_999A), f32::from_bits(0x3DCC_CCCD));
    }
    if n(fx) >= 0x19 {
        swath::draw(fx, D + 0x70, D);
        swath::draw(fx, D + 0x1A0, D);
        if n(fx) < 0x6A {
            fx.step_up(D + 0x7B, 0xF0, 0x10);
            fx.step_up(D + 0x1AB, 0xF0, 0x10);
        } else {
            fx.step_down(D + 0x7B, 0, 0x10);
            fx.step_down(D + 0x1AB, 0, 0x10);
        }
    }
    if n(fx) < 0x60 {
        quadtrail::draw(fx, TRAIL_DRAW, OT, D + 0x2D0, D);
        let mut i = 0;
        while i < fx.mem.i16(D + 0x318) {
            let mut j = 0;
            loop {
                let w = fx.mem.i16(D + 0x31A);
                let lo = fx.mult(i, w);
                if j >= w {
                    break;
                }
                fx.step_down(D + 0x32C + 0x10 * (j + lo) as u32, 0, 3);
                j += 1;
            }
            i += 1;
        }
    }
    let m = n(fx);
    if (0x5A..0x6A).contains(&m) {
        fx.ring_draw(D + 0x1A70, D + 0x1A00);
        fx.approach(D + 0x1A00, 2.0, f32::from_bits(0x3E4C_CCCD));
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x2204, m);
    let left = fx.mem.i32(D + 0x2200) - 1;
    fx.mem.set32(D + 0x2200, left);
    if m == 0x82 {
        2
    } else if m == 0x60 {
        1
    } else if (0x6A..0x82).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
