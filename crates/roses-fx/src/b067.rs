//! BMODULE b067 (card 208), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E033E0, data area 0x1ED0000; compiled without
//! delay-slot scheduling) [F]: the attacker's
//! zoom afterimage (+0x2900, `blib::zoom`) on updates 0xC..0xBF, fading
//! in to 0x40 and out from 0x5C; a swath (+0x70) between the nodes 0x38 /
//! 0x39 (refilled on 0x14 / 0x86 / 0xA7, drawn on 0x52..0x63); a quad
//! trail (+0x1A0) pushed at the nodes 0x12 / 0x18 on 0x26..0x5A; from
//! update 0x5B blades (+0x17F0, `blib::blade`) rising at the attacker, a
//! square cloud (+0x1460) and a wave (+0x1750) at the target. Returns 1
//! on update 0x51, 2 on 0x5C, 8 after 0x6E.
//!
//! Library at its own addresses: `colours4` (0x1E00080), `zoom::sprites`
//! (0x1E00120), `swath` push / draw (0x1E007F0 / 0x1E00940),
//! `pointquad::squares` (0x1E01140), `wave::draw` (0x1E01720),
//! `quadtrail` push / draw (0x1E01F20 / 0x1E02120), `blade` setup / draw
//! (0x1E02B30 / 0x1E02CD0).
use crate::b142::fstep_up;
use crate::b186::{bytes, halves};
use crate::blib::{blade, pointquad, quadtrail, swath, wave, zoom};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const ZOOM: u32 = 0x01E0_0120;
const TRAIL_DRAW: u32 = 0x01E0_2120;
const BLADE_SETUP: u32 = 0x01E0_2B30;

/// The entry 0x1E033E0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x80);
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

/// The swath's 8 points (+0x80 / +0x100) at sp + 0x40 / 0x50.
fn swath_fill(fx: &mut Fx, sp: u32) {
    for k in 0..8u32 {
        for c in 0..3u32 {
            let a = fx.mem.read_u32(sp + 0x40 + 4 * c);
            fx.mem.write_u32(D + 0x80 + 0x10 * k + 4 * c, a);
            let b = fx.mem.read_u32(sp + 0x50 + 4 * c);
            fx.mem.write_u32(D + 0x100 + 0x10 * k + 4 * c, b);
        }
    }
    fx.mem.set16(D + 0x7E, 0);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2DB0);
    fx.xform_init(D);
    fx.xform_init(D + 0x13F0);
    for o in [0x13F8u32, 0x13F4, 0x13F0] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    fx.xform_init(D + 0x1780);
    fx.xform_init(D + 0x2890);
    for (at, n) in [(0x291Cu32, 0x3D), (0x2920, 0x38), (0x2924, 0x39), (0x2928, 0x18), (0x292C, 0x12)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(at == 0x291C));
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x2910, 0x6E);
    fx.mem.set32(D + 0x2914, 1);
    fx.mem.set32(D + 0x2918, param);
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, 0x01E0_352C);
        fx.mem.write_u8(D + 0x2CB0 + k, (r >> 4) as u8);
    }
    // The zoom afterimage (+0x2900) of the attacker.
    fx.mem.write_u32(D + 0x2900, 0);
    bytes(fx, &[(0x2906, 0x10), (0x2905, 0x40), (0x2904, 0x10), (0x2907, 0)]);
    fx.mem.write_u32(D + 0x2908, 0x41A0_0000);
    halves(fx, &[(0x290C, 5)]);
    let h = fx.self_handle();
    halves(fx, &[(0x290E, h)]);
    fx.texture5(sp + 0x70, 0);
    // The blades (+0x17F0, records +0x1890).
    fx.mem.write_u32(D + 0x17F4, 1);
    halves(fx, &[(0x1800, 0), (0x1802, 0), (0x1804, 0x20), (0x1806, 0x40)]);
    bytes(fx, &[(0x17FA, 0xD0), (0x17F9, 0x60), (0x17F8, 0x40), (0x17FB, 0x80)]);
    fx.mem.write_u32(D + 0x1870, D + 0x1890);
    for (o, v) in [(0x1874u32, 0x4270_0000u32), (0x1878, 0x4248_0000), (0x187C, 0x4000_0000), (0x1880, 0), (0x1884, 0x6000), (0x17FC, 0x41A0_0000)] {
        fx.mem.write_u32(D + o, v);
    }
    halves(fx, &[(0x1888, 0), (0x188A, 0x100), (0x188C, 0x3E7), (0x188E, 3)]);
    let tex = fx.mem.read_u32(sp + 0x70);
    blade::setup(fx, BLADE_SETUP, D + 0x17F0, tex);
    fx.texture4(sp + 0x70, 0xF);
    // The swath (+0x70).
    let tex = fx.mem.read_u32(sp + 0x70);
    fx.mem.write_u32(D + 0x70, tex);
    fx.mem.write_u32(D + 0x74, 1);
    halves(fx, &[(0x7C, 8), (0x180, 0x8E), (0x182, 0x54), (0x184, 0x20), (0x186, 1)]);
    bytes(fx, &[(0x7A, 0xD0), (0x79, 0x60), (0x78, 0x40), (0x7B, 0xFF)]);
    fx.mem.write_u32(D + 0x190, D + 0x2930);
    fx.mem.write_u32(D + 0x194, D + 0x2AF0);
    node(fx, 0x2920, sp + 0x40);
    node(fx, 0x2924, sp + 0x50);
    swath_fill(fx, sp);
    // The square cloud (+0x1460, 60 points at +0x1480 + 12 k).
    let tex = fx.mem.read_u32(sp + 0x70);
    fx.mem.write_u32(D + 0x1460, tex);
    fx.mem.write_u32(D + 0x1464, 0x2008_0001);
    halves(fx, &[(0x1470, 0xB0), (0x1472, 0x40), (0x1474, 0x20), (0x1476, 0x20)]);
    bytes(fx, &[(0x146A, 0xD0), (0x1469, 0x60), (0x1468, 0x40), (0x146B, 0xFF)]);
    fx.mem.write_u32(D + 0x146C, 0x3F00_0000);
    for k in 0..0x3Cu32 {
        let p = D + 0xC * k;
        if bit(fx.mem.read_u32(D + 0x1464), 0x1D) {
            let x = rand_f(fx, 0x01E0_37B8, 0x64, 0x32);
            fx.mem.setf(p + 0x1480, x);
            let z = rand_f(fx, 0x01E0_37F8, 0x32, 0);
            fx.mem.setf(p + 0x1488, z);
            let y = rand_f(fx, 0x01E0_3828, 0x64, 0x32);
            fx.mem.setf(p + 0x1484, y);
        } else {
            let x = rand_f(fx, 0x01E0_3864, 0x32, 0x19);
            fx.mem.setf(p + 0x1480, x);
            let y = rand_f(fx, 0x01E0_38A4, 0x19, 0);
            fx.mem.setf(p + 0x1484, y);
            let z = rand_f(fx, 0x01E0_38D4, 0x32, 0x19);
            fx.mem.setf(p + 0x1488, z);
        }
    }
    // The wave (+0x1750).
    let tex = fx.mem.read_u32(sp + 0x70);
    fx.mem.write_u32(D + 0x1750, tex);
    fx.mem.write_u32(D + 0x175C, D + 0x2CB0);
    fx.mem.write_u32(D + 0x1754, 0x2000_0000);
    halves(fx, &[(0x1760, 0xB0), (0x1762, 0x40), (0x1764, 0x20), (0x1766, 0x20)]);
    bytes(fx, &[(0x175A, 0x80), (0x1759, 0x10), (0x1758, 0x10), (0x175B, 0xFF)]);
    fx.mem.write_u32(D + 0x1770, 0x3F80_0000);
    fx.mem.write_u32(D + 0x1774, 0x3CF5_C28F);
    halves(fx, &[(0x177A, 0), (0x1778, 0x100)]);
    fx.mem.write_u8(D + 0x177C, 7);
    // The quad trail (+0x1A0, cells +0x1F0).
    let tex = fx.mem.read_u32(sp + 0x70);
    fx.mem.write_u32(D + 0x1A0, tex);
    bytes(fx, &[(0x1A6, 0x80), (0x1A5, 0x80), (0x1A4, 0x80), (0x1A7, 0x30)]);
    fx.mem.write_u32(D + 0x1A8, 0x1_9003);
    fx.mem.write_u32(D + 0x1AC, D + 0x1F0);
    halves(fx, &[(0x1B0, 0x80), (0x1B2, 0xC0), (0x1B4, 0x20), (0x1B6, 0x20), (0x1B8, 0x80), (0x1BA, 0xC0), (0x1BC, 0x80), (0x1BE, 0x100)]);
    for (o, v) in [(0x1C0u32, 0u32), (0x1C4, 0x3E99_999A), (0x1C8, 0), (0x1D0, 0x4040_0000), (0x1D4, 0x3E4C_CCCD), (0x1D8, 0x4053_3333)] {
        fx.mem.write_u32(D + o, v);
    }
    halves(fx, &[(0x1E2, 0), (0x1E4, 0x18), (0x1E6, 0x24), (0x1E8, 0), (0x1EA, 8), (0x1EC, 8), (0x1EE, 0)]);
    0
}

/// The quad trail (`g`, cells `cells`) drawn, its cells fading by 3.
pub(crate) fn trail_draw(fx: &mut Fx, at: u32, g: u32, cells: u32) {
    quadtrail::draw(fx, at, OT, D + g, D);
    let mut i = 0;
    while i < fx.mem.i16(D + g + 0x48) {
        let mut j = 0;
        while j < fx.mem.i16(D + g + 0x4A) {
            let w = fx.mem.i16(D + g + 0x4A);
            let lo = fx.mult(i, w);
            fx.step_down(D + cells + 0xC + 0x10 * (j + lo) as u32, 0, 3);
            j += 1;
        }
        i += 1;
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x2914);
    let h = fx.self_handle();
    fx.unit_point(1, h, D + 0x28B0);
    let t = fx.target_handle();
    fx.unit_position(t, D + 0x1410);
    node(fx, 0x2920, sp + 0x40);
    node(fx, 0x2924, sp + 0x50);
    if matches!(n(fx), 0x14 | 0x86 | 0xA7) {
        swath_fill(fx, sp);
    } else {
        swath::push(fx, D + 0x70, sp + 0x40, sp + 0x50);
    }
    if matches!(n(fx), 0x2E | 0x3E | 0x4B | 0x59) {
        node(fx, 0x292C, sp + 0x60);
        quadtrail::push(fx, D + 0x1A0, sp + 0x60);
    }
    if matches!(n(fx), 0x26 | 0x36 | 0x46 | 0x5A) {
        node(fx, 0x2928, sp + 0x60);
        quadtrail::push(fx, D + 0x1A0, sp + 0x60);
    }
    if n(fx) == 0x5B {
        let h = fx.self_handle();
        fx.unit_point(1, h, D + 0x17A0);
        fx.mem.write_u32(D + 0x17A4, 0x41A0_0000);
        if !ee_le(fx.mem.f(D + 0x17A8), 0.0) {
            fx.mem.write_u32(D + 0x1794, 0x8000);
        }
    }
    // s0 = D, s1 = the OT, s2 = sp + 0x50, s3 the runner's (0).
    let regs = [Some(D), Some(OT), Some(sp + 0x50), Some(0)];
    fx.xform_world_saving(D, 0x01E0_3C78, regs);
    fx.xform_billboard_saving(D + 0x13F0, 0x01E0_3C84, regs);
    fx.xform_world_saving(D + 0x1780, 0x01E0_3C90, regs);
    fx.xform_billboard_saving(D + 0x2890, 0x01E0_3C9C, regs);
    if (0xC..0xC0).contains(&n(fx)) {
        zoom::sprites(fx, ZOOM, D + 0x2900, D + 0x28B0);
        if n(fx) >= 0x5C {
            fx.step_down(D + 0x2907, 0, 4);
        } else {
            fx.step_up(D + 0x2907, 0x40, 2);
        }
    }
    if n(fx) >= 0x5B {
        blade::draw(fx, OT, D + 0x17F0, D + 0x1780);
        fstep_up(fx, D + 0x17FC, 50.0, 2.0);
        let mut k = 0;
        while k < fx.mem.i16(D + 0x188A) {
            let p = fx.mem.read_u32(D + 0x1870) + 0x10 * k as u32;
            fx.step_down(p + 0xC, 0, 8);
            k += 1;
        }
    }
    if n(fx) >= 0x5B {
        for o in [0x68u32, 0x64, 0x60] {
            fx.mem.write_u32(sp + o, 0x3F83_D70A);
        }
        pointquad::squares(fx, OT, D + 0x1460, D + 0x13F0, sp + 0x60);
        if fx.approach(D + 0x13F0, f32::from_bits(0x3E99_999A), f32::from_bits(0x3DCC_CCCD)) == 0 {
            fx.step_down(D + 0x146B, 0, 0x10);
        }
        wave::draw(fx, OT, D + 0x1750, D + 0x13F0);
        // wave::draw's `sq s2` (s2 = the blade loop's count, upper half 0):
        // the trail's key reads its top word as a stale w (sp + 0x15C).
        let k = fx.mem.i16(D + 0x188A).max(0);
        fx.saved(sp - 0x150, &[(0x30, k as u64)]);
        fx.step_down(D + 0x175B, 0, 0x10);
    }
    trail_draw(fx, TRAIL_DRAW, 0x1A0, 0x1F0);
    if (0x52..0x64).contains(&n(fx)) {
        swath::draw(fx, D + 0x70, D);
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x2914, m);
    let left = fx.mem.i32(D + 0x2910) - 1;
    fx.mem.set32(D + 0x2910, left);
    if m == 0x5C {
        2
    } else if m == 0x51 {
        1
    } else if left > 0 {
        0
    } else {
        8
    }
}
