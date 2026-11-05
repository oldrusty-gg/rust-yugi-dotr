//! BMODULE b025 (card 211), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E045D0, data area 0x1ED0000; compiled without
//! delay-slot scheduling) [F]: b067's effect
//! retimed with more parts: the attacker's zoom afterimage (+0x2450,
//! `blib::zoom`) on updates 0xC..0xBF (out from 0xA8); a swath (+0x70)
//! between the nodes 0x3C / 0x3B drawn on 0x15..0x1F, 0x87..0x8E and
//! 0xA8..0xB5; a streamer (+0x1A0, points +0x1E0, `blib::streamer`) pushed
//! at the first node until 0xE; a quad trail (+0x420) pushed at the nodes
//! 0xC / 0x16; from 0x87 blades (+0x1F40) at the attacker (all bright
//! again on 0xA8), a square cloud (+0x16E0) and a wave (+0x19D0) at the
//! target until 0xA7; from 0xA8 a quad group (+0x1A70). Returns 1 on
//! update 0x7E, 6 on 0x88..0xA7, 2 on 0xA8, 8 after 0xFA.
//!
//! Library at its own addresses: `colours4` (0x1E00080), `zoom::sprites`
//! (0x1E00120), `swath` push / draw (0x1E007F0 / 0x1E00940),
//! `pointquad::squares` (0x1E01140), `wave::draw` (0x1E01720),
//! `streamer` push / draw (0x1E01F20 / 0x1E02000), `quadtrail` push /
//! draw (0x1E029A0 / 0x1E02BA0), `quads_setup` / `quads_draw` (0x1E035B0
//! / 0x1E03710), `blade` setup / draw (0x1E03D20 / 0x1E03EC0).
use crate::b067::trail_draw;
use crate::b142::fstep_up;
use crate::b186::{bytes, halves};
use crate::blib::{blade, pointquad, quadtrail, streamer, swath, wave, zoom};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const ZOOM: u32 = 0x01E0_0120;
const STREAMER: u32 = 0x01E0_2000;
const TRAIL_DRAW: u32 = 0x01E0_2BA0;
const BLADE_SETUP: u32 = 0x01E0_3D20;
const QUAD_TEMPLATE: u32 = 0x01E0_5480;

/// The entry 0x1E045D0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x90);
    ret
}

fn node(fx: &mut Fx, at: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(D + at);
    fx.node_position(h, s, out);
}

/// rand % m - sub as a float (rand at `ra`, s0 = D).
fn rand_f(fx: &mut Fx, ra: u32, m: i32, sub: i32) -> f32 {
    let r = fx.rand_from(D, ra);
    let (_, h) = fx.div(r, m);
    ee::from_i32(h - sub)
}

/// The swath's 8 points (+0x80 / +0x100) at sp + 0x50 / 0x60.
fn swath_fill(fx: &mut Fx, sp: u32) {
    for k in 0..8u32 {
        for c in 0..3u32 {
            let a = fx.mem.read_u32(sp + 0x50 + 4 * c);
            fx.mem.write_u32(D + 0x80 + 0x10 * k + 4 * c, a);
            let b = fx.mem.read_u32(sp + 0x60 + 4 * c);
            fx.mem.write_u32(D + 0x100 + 0x10 * k + 4 * c, b);
        }
    }
    fx.mem.set16(D + 0x7E, 0);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2900);
    fx.xform_init(D);
    fx.xform_init(D + 0x1670);
    for o in [0x1678u32, 0x1674, 0x1670] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for x in [0x1A00u32, 0x1ED0, 0x23E0] {
        fx.xform_init(D + x);
    }
    for (at, n) in [(0x246Cu32, 0x3D), (0x2470, 0x3C), (0x2474, 0x3B), (0x2478, 0x16), (0x247C, 0xC)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(at == 0x246C));
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x2460, 0xFA);
    fx.mem.set32(D + 0x2464, 1);
    fx.mem.set32(D + 0x2468, param);
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, 0x01E0_472C);
        fx.mem.write_u8(D + 0x2800 + k, (r >> 4) as u8);
    }
    // The zoom afterimage (+0x2450) of the attacker.
    fx.mem.write_u32(D + 0x2450, 0);
    bytes(fx, &[(0x2456, 0x80), (0x2455, 0x10), (0x2454, 0x10), (0x2457, 0)]);
    fx.mem.write_u32(D + 0x2458, 0x41A0_0000);
    halves(fx, &[(0x245C, 5)]);
    let h = fx.self_handle();
    halves(fx, &[(0x245E, h)]);
    fx.texture5(sp + 0x80, 0);
    // The blades (+0x1F40, records +0x1FE0).
    fx.mem.write_u32(D + 0x1F44, 1);
    halves(fx, &[(0x1F50, 0), (0x1F52, 0), (0x1F54, 0x20), (0x1F56, 0x40)]);
    bytes(fx, &[(0x1F4A, 0x20), (0x1F49, 0x10), (0x1F48, 0x40), (0x1F4B, 0x80)]);
    fx.mem.write_u32(D + 0x1FC0, D + 0x1FE0);
    for (o, v) in [(0x1FC4u32, 0x41F0_0000u32), (0x1FC8, 0x41F0_0000), (0x1FCC, 0x4000_0000), (0x1FD0, 0), (0x1FD4, 0x6000), (0x1F4C, 0x41A0_0000)] {
        fx.mem.write_u32(D + o, v);
    }
    halves(fx, &[(0x1FD8, 0), (0x1FDA, 0x40), (0x1FDC, 0x3E7), (0x1FDE, 3)]);
    let tex = fx.mem.read_u32(sp + 0x80);
    blade::setup(fx, BLADE_SETUP, D + 0x1F40, tex);
    fx.texture4(sp + 0x80, 0xF);
    // The swath (+0x70).
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(D + 0x70, tex);
    fx.mem.write_u32(D + 0x74, 1);
    halves(fx, &[(0x7C, 8), (0x180, 0x8E), (0x182, 0x54), (0x184, 0x20), (0x186, 1)]);
    bytes(fx, &[(0x7A, 0x20), (0x79, 0x10), (0x78, 0x40), (0x7B, 0xFF)]);
    fx.mem.write_u32(D + 0x190, D + 0x2480);
    fx.mem.write_u32(D + 0x194, D + 0x2640);
    node(fx, 0x2470, sp + 0x50);
    node(fx, 0x2474, sp + 0x60);
    swath_fill(fx, sp);
    // The square cloud (+0x16E0, 60 points at +0x1700 + 12 k).
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(D + 0x16E0, tex);
    fx.mem.write_u32(D + 0x16E4, 0x2008_0001);
    halves(fx, &[(0x16F0, 0xB0), (0x16F2, 0x40), (0x16F4, 0x20), (0x16F6, 0x20)]);
    bytes(fx, &[(0x16EA, 0x80), (0x16E9, 0x80), (0x16E8, 0x80), (0x16EB, 0xFF)]);
    fx.mem.write_u32(D + 0x16EC, 0x3F00_0000);
    for k in 0..0x3Cu32 {
        let p = D + 0xC * k;
        if bit(fx.mem.read_u32(D + 0x16E4), 0x1D) {
            let x = rand_f(fx, 0x01E0_49A4, 0x64, 0x32);
            fx.mem.setf(p + 0x1700, x);
            let z = rand_f(fx, 0x01E0_49E4, 0x32, 0);
            fx.mem.setf(p + 0x1708, z);
            let y = rand_f(fx, 0x01E0_4A14, 0x64, 0x32);
            fx.mem.setf(p + 0x1704, y);
        } else {
            let x = rand_f(fx, 0x01E0_4A50, 0x32, 0x19);
            fx.mem.setf(p + 0x1700, x);
            let y = rand_f(fx, 0x01E0_4A90, 0x19, 0);
            fx.mem.setf(p + 0x1704, y);
            let z = rand_f(fx, 0x01E0_4AC0, 0x32, 0x19);
            fx.mem.setf(p + 0x1708, z);
        }
    }
    // The wave (+0x19D0).
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(D + 0x19D0, tex);
    fx.mem.write_u32(D + 0x19DC, D + 0x2800);
    fx.mem.write_u32(D + 0x19D4, 0x2000_0000);
    halves(fx, &[(0x19E0, 0xB0), (0x19E2, 0x40), (0x19E4, 0x20), (0x19E6, 0x20)]);
    bytes(fx, &[(0x19DA, 0x80), (0x19D9, 0x10), (0x19D8, 0x10), (0x19DB, 0xFF)]);
    fx.mem.write_u32(D + 0x19F0, 0x3F4C_CCCD);
    fx.mem.write_u32(D + 0x19F4, 0x3CF5_C28F);
    halves(fx, &[(0x19FA, 0), (0x19F8, 0x100)]);
    fx.mem.write_u8(D + 0x19FC, 7);
    // The streamer (+0x1A0, points +0x1E0).
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(D + 0x1A0, tex);
    bytes(fx, &[(0x1A6, 0x80), (0x1A5, 0x10), (0x1A4, 0x10), (0x1A7, 0x30)]);
    for (o, v) in [(0x1A8u32, 0x9003u32), (0x1AC, 0x4040_0000), (0x1C0, 0), (0x1C4, 0x3F00_0000), (0x1C8, 0)] {
        fx.mem.write_u32(D + o, v);
    }
    halves(fx, &[(0x1B0, 0x80), (0x1B2, 0xC0), (0x1B4, 0x20), (0x1B6, 0x20), (0x1B8, 0x80), (0x1BA, 0xC0), (0x1BC, 0x80), (0x1BE, 0x100)]);
    fx.mem.write_u32(D + 0x1D0, 0x3E99_999A);
    halves(fx, &[(0x1D4, 0), (0x1D6, 0x18), (0x1D8, 0x24), (0x1DA, 0), (0x1DC, 8)]);
    // The quad trail (+0x420, cells +0x470).
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(D + 0x420, tex);
    bytes(fx, &[(0x426, 0x80), (0x425, 0x80), (0x424, 0x80), (0x427, 0x30)]);
    fx.mem.write_u32(D + 0x428, 0x1_9003);
    fx.mem.write_u32(D + 0x42C, D + 0x470);
    halves(fx, &[(0x430, 0x80), (0x432, 0xC0), (0x434, 0x20), (0x436, 0x20), (0x438, 0x80), (0x43A, 0xC0), (0x43C, 0x80), (0x43E, 0x100)]);
    for (o, v) in [(0x440u32, 0u32), (0x444, 0x3E99_999A), (0x448, 0), (0x450, 0x4040_0000), (0x454, 0x3E4C_CCCD), (0x458, 0x4053_3333)] {
        fx.mem.write_u32(D + o, v);
    }
    halves(fx, &[(0x462, 0), (0x464, 0x18), (0x466, 0x24), (0x468, 0), (0x46A, 8), (0x46C, 8), (0x46E, 0)]);
    // The quad group (+0x1A70).
    fx.mem.write_u32(D + 0x1A74, 0x8_0001);
    halves(fx, &[(0x1A80, 0x8E), (0x1A82, 0), (0x1A84, 0x22), (0x1A86, 0x40)]);
    bytes(fx, &[(0x1A7A, 0xD0), (0x1A79, 0xA0), (0x1A78, 0x70), (0x1A7B, 0x20)]);
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.quads_setup(D + 0x1A70, tex, f32::from_bits(0x3FCC_CCCD), QUAD_TEMPLATE);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x2464);
    let h = fx.self_handle();
    fx.unit_point(1, h, D + 0x2400);
    for out in [0x1690u32, 0x1A20] {
        let t = fx.target_handle();
        fx.unit_position(t, D + out);
    }
    node(fx, 0x2470, sp + 0x50);
    node(fx, 0x2474, sp + 0x60);
    if matches!(n(fx), 0x14 | 0x86 | 0xA7) {
        swath_fill(fx, sp);
    } else {
        swath::push(fx, D + 0x70, sp + 0x50, sp + 0x60);
    }
    if n(fx) < 0xF {
        streamer::push(fx, D + 0x1A0, D + 0x1E0, sp + 0x50);
    }
    if matches!(n(fx), 0x4F | 0x63 | 0x86 | 0xBD) {
        node(fx, 0x247C, sp + 0x70);
        quadtrail::push(fx, D + 0x420, sp + 0x70);
    }
    if matches!(n(fx), 0x59 | 0x6D | 0x87 | 0xBE) {
        node(fx, 0x2478, sp + 0x70);
        quadtrail::push(fx, D + 0x420, sp + 0x70);
    }
    if n(fx) == 0x87 {
        let h = fx.self_handle();
        fx.unit_point(1, h, D + 0x1EF0);
        fx.mem.write_u32(D + 0x1EF4, 0x41A0_0000);
        if !ee_le(fx.mem.f(D + 0x1EF8), 0.0) {
            fx.mem.write_u32(D + 0x1EE4, 0x8000);
        }
    }
    // s0 = D, s1 = the OT, s2 = sp + 0x60, s3 the runner's (0).
    let regs = [Some(D), Some(OT), Some(sp + 0x60), Some(0)];
    fx.xform_world_saving(D, 0x01E0_4F74, regs);
    fx.xform_billboard_saving(D + 0x1670, 0x01E0_4F80, regs);
    fx.xform_world_saving(D + 0x1A00, 0x01E0_4F8C, regs);
    fx.xform_world_saving(D + 0x1ED0, 0x01E0_4F98, regs);
    fx.xform_billboard_saving(D + 0x23E0, 0x01E0_4FA4, regs);
    if (0xC..0xC0).contains(&n(fx)) {
        zoom::sprites(fx, ZOOM, D + 0x2450, D + 0x2400);
        if n(fx) >= 0xA8 {
            fx.step_down(D + 0x2457, 0, 4);
        } else {
            fx.step_up(D + 0x2457, 0x40, 2);
        }
    }
    let mut s2 = None;
    if n(fx) >= 0x87 {
        if n(fx) == 0xA8 {
            fx.mem.write_u32(D + 0x1F4C, 0x41A0_0000);
            let mut k = 0;
            while k < fx.mem.i16(D + 0x1FDA) {
                let p = fx.mem.read_u32(D + 0x1FC0) + 0x10 * k as u32;
                fx.mem.write_u8(p + 0xC, 0x80);
                fx.mem.set16(p + 0xE, 0x14);
                k += 1;
            }
        }
        blade::draw(fx, OT, D + 0x1F40, D + 0x1ED0);
        fstep_up(fx, D + 0x1F4C, 50.0, 2.0);
        let mut k = 0;
        while k < fx.mem.i16(D + 0x1FDA) {
            let p = fx.mem.read_u32(D + 0x1FC0) + 0x10 * k as u32;
            if fx.mem.i16(p + 0xE) >= 0xB {
                fx.step_down(p + 0xC, 0, 8);
            }
            k += 1;
        }
        s2 = Some(k as u64);
    }
    if n(fx) >= 0xA8 {
        fx.quads_draw(D + 0x1A70, D + 0x1A00);
        // quads_draw's `sq fp` upper half (0) over the blades' w: the
        // trail's and the streamer's keys read it (sp + 0x15C) [run].
        fx.mem.write_u64(sp - 0x1A0 + 0x88, 0);
        fx.step_down(D + 0x1A7B, 0, 8);
    }
    if (0x87..0xA8).contains(&n(fx)) {
        for o in [0x78u32, 0x74, 0x70] {
            fx.mem.write_u32(sp + o, 0x3F83_D70A);
        }
        pointquad::squares(fx, OT, D + 0x16E0, D + 0x1670, sp + 0x70);
        if fx.approach(D + 0x1670, f32::from_bits(0x3E99_999A), f32::from_bits(0x3DCC_CCCD)) == 0 {
            fx.step_down(D + 0x16EB, 0, 0x10);
        }
        wave::draw(fx, OT, D + 0x19D0, D + 0x1670);
        // wave::draw's `sq s2` (the blade loop's count, upper half 0): the
        // trail's and the streamer's keys read its top word (sp + 0x15C).
        if let Some(k) = s2 {
            fx.saved(sp - 0x150, &[(0x30, k)]);
        }
        fx.step_down(D + 0x19DB, 0, 0x10);
    }
    trail_draw(fx, TRAIL_DRAW, 0x420, 0x470);
    streamer::draw(fx, STREAMER, OT, D + 0x1A0, D + 0x1E0, D);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x1DA) {
        fx.step_down(D + 0x1EC + 0x10 * k as u32, 0, 2);
        k += 1;
    }
    if n(fx) >= 0x8F {
        fx.step_down(D + 0x1A7, 0, 8);
    }
    let m = n(fx);
    if (0x15..0x20).contains(&m) || (0x87..0x8F).contains(&m) || (0xA8..0xB6).contains(&m) {
        swath::draw(fx, D + 0x70, D);
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x2464, m);
    let left = fx.mem.i32(D + 0x2460) - 1;
    fx.mem.set32(D + 0x2460, left);
    if m == 0xA8 {
        2
    } else if m == 0x7E {
        1
    } else if (0x88..0xA8).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
