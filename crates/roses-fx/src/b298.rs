//! BMODULE b298 (card 186), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E040C0, data area 0x1ED0000; compiled without
//! delay-slot scheduling) [F]: b067's program
//! without the quad trail and with a sprite fountain: the attacker's zoom
//! afterimage (+0xEB0, `blib::zoom`) on updates 0xC..0xBF (out from 0x64);
//! a swath (+0x70) between the nodes 0x52 / 0x51 drawn on 0x53..0x63; 64
//! sprites (+0xF30, `sprites`) thrown up from the attacker's node 0x54
//! (aimed at by +0xED0), bright again on 0x5B and fading from 0x65; from
//! 0x5B blades (+0x5A0), a square cloud (+0x210) and a wave (+0x500) at
//! the target. Returns 1 on update 0x51, 6 on 0x5B..0x63, 2 on 0x64, 8
//! after 0x74.
//!
//! Library at its own addresses: `colours4` (0x1E00080), `zoom::sprites`
//! (0x1E00120), `sprites` (0x1E007F0), `swath` push / draw (0x1E020E0 /
//! 0x1E02230), `pointquad::squares` (0x1E02A30), `wave::draw`
//! (0x1E03010), `blade` setup / draw (0x1E03810 / 0x1E039B0).
use crate::b142::fstep_up;
use crate::b186::{bytes, halves};
use crate::blib::{blade, pointquad, swath, wave, zoom};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const ZOOM: u32 = 0x01E0_0120;
const BLADE_SETUP: u32 = 0x01E0_3810;

/// The entry 0x1E040C0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xF0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xF0);
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

/// A fountain sprite (+0xF90 + 0x10 k) thrown: speed 0.1 (rand % (10
/// +0xF7C)) along a random direction (rand & 0xFFFF), y 0.1 (20 - rand %
/// 0x28), life -(rand % 0xC), alpha 0x80; the rands / 0x1A2B80 at `ra`.
fn throw(fx: &mut Fx, p: u32, ra: [u32; 5]) {
    let v = ee::mul(10.0, fx.mem.f(D + 0xF7C));
    fx.fix_residue(v, ra[0]);
    let s2 = ee::fixsfsi(v);
    let a = fx.rand_from(D, ra[1]) & 0xFFFF;
    let r = fx.rand_from(D, ra[2]);
    let (_, h) = fx.div(r, s2);
    let f20 = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h));
    let r = fx.rand_from(D, ra[3]);
    let (_, h) = fx.div(r, 0x28);
    fx.mem.setf(p + 0xF94, ee::mul(f32::from_bits(0x3DCC_CCCD), ee::neg(ee::from_i32(h - 0x14))));
    let c = fx.tcos(a);
    fx.mem.setf(p + 0xF98, ee::mul(f20, c));
    let s = fx.tsin(a);
    fx.mem.setf(p + 0xF90, ee::mul(f20, s));
    let r = fx.rand_from(D, ra[4]);
    let (_, h) = fx.div(r, 0xC);
    fx.mem.set16(p + 0xF9E, h.wrapping_neg());
    fx.mem.write_u8(p + 0xF9C, 0x80);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x1830);
    fx.xform_init(D);
    fx.xform_init(D + 0x1A0);
    for o in [0x1A8u32, 0x1A4, 0x1A0] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for x in [0x530u32, 0xE40, 0xEC0] {
        fx.xform_init(D + x);
    }
    for (at, n) in [(0x139Cu32, 0x54), (0x13A0, 0x52), (0x13A4, 0x51), (0x13A8, 0x18), (0x13AC, 0x12)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(at == 0x139C));
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x1390, 0x74);
    fx.mem.set32(D + 0x1394, 1);
    fx.mem.set32(D + 0x1398, param);
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, 0x01E0_4234);
        let (_, h) = fx.div(r, 0xFF);
        fx.mem.write_u8(D + 0x1730 + k, (h + 1) as u8);
    }
    // The zoom afterimage (+0xEB0) of the attacker.
    fx.mem.write_u32(D + 0xEB0, 0);
    bytes(fx, &[(0xEB6, 0x80), (0xEB5, 0x60), (0xEB4, 0x40), (0xEB7, 0)]);
    fx.mem.write_u32(D + 0xEB8, 0x41A0_0000);
    halves(fx, &[(0xEBC, 5)]);
    let h = fx.self_handle();
    halves(fx, &[(0xEBE, h)]);
    fx.texture5(sp + 0xE0, 0);
    // The blades (+0x5A0, records +0x640).
    fx.mem.write_u32(D + 0x5A4, 1);
    halves(fx, &[(0x5B0, 0), (0x5B2, 0), (0x5B4, 0x20), (0x5B6, 0x40)]);
    bytes(fx, &[(0x5AA, 0xD0), (0x5A9, 0x60), (0x5A8, 0x40), (0x5AB, 0x80)]);
    fx.mem.write_u32(D + 0x620, D + 0x640);
    for (o, v) in [(0x624u32, 0x4270_0000u32), (0x628, 0x4248_0000), (0x62C, 0x4000_0000), (0x630, 0), (0x634, 0x6000), (0x5AC, 0x41A0_0000)] {
        fx.mem.write_u32(D + o, v);
    }
    halves(fx, &[(0x638, 0), (0x63A, 0x80), (0x63C, 0x3E7), (0x63E, 3)]);
    let tex = fx.mem.read_u32(sp + 0xE0);
    blade::setup(fx, BLADE_SETUP, D + 0x5A0, tex);
    fx.texture5(sp + 0xE0, 1);
    // The sprite fountain (+0xF30, 64 sprites at +0xF90).
    let tex = fx.mem.read_u32(sp + 0xE0);
    fx.mem.write_u32(D + 0xF30, tex);
    fx.mem.write_u32(D + 0xF34, 0x7008_0403);
    bytes(fx, &[(0xF3A, 0x80), (0xF39, 0x80), (0xF38, 0x80), (0xF3B, 0)]);
    fx.mem.write_u32(D + 0xF3C, 0x40A0_0000);
    for o in [0xF50u32, 0xF54, 0xF58] {
        fx.mem.write_u32(D + o, 0);
    }
    halves(fx, &[(0xF60, 0), (0xF62, 0x30), (0xF64, 0x20), (0xF66, 0x20), (0xF68, 0), (0xF6A, 0x30), (0xF6C, 0x100), (0xF6E, 0x80)]);
    fx.mem.write_u32(D + 0xF70, D + 0xF90);
    fx.mem.write_u32(D + 0xF74, 0);
    fx.mem.write_u32(D + 0xF78, 0);
    fx.mem.write_u32(D + 0xF7C, 0x40A0_0000);
    halves(fx, &[(0xF8A, 0x40), (0xF8C, 0xE), (0xF8E, 0x64)]);
    let mut k = 0i32;
    while k < fx.mem.i16(D + 0xF8A) {
        let p = D + 0x10 * k as u32;
        throw(fx, p, [0x01E0_4410, 0x01E0_441C, 0x01E0_443C, 0x01E0_4474, 0x01E0_44F4]);
        let r = fx.rand_from(D, 0x01E0_4524);
        let (_, h) = fx.div(r, 0xE);
        fx.mem.write_u8(p + 0xF9D, h as u8);
        k += 1;
    }
    fx.texture4(sp + 0xE0, 0xF);
    // The swath (+0x70).
    let tex = fx.mem.read_u32(sp + 0xE0);
    fx.mem.write_u32(D + 0x70, tex);
    fx.mem.write_u32(D + 0x74, 1);
    halves(fx, &[(0x7C, 8), (0x180, 0x8E), (0x182, 0x54), (0x184, 0x20), (0x186, 1)]);
    bytes(fx, &[(0x7A, 0xD0), (0x79, 0x60), (0x78, 0x40), (0x7B, 0xFF)]);
    fx.mem.write_u32(D + 0x190, D + 0x13B0);
    fx.mem.write_u32(D + 0x194, D + 0x1570);
    node(fx, 0x13A0, sp + 0xB0);
    node(fx, 0x13A4, sp + 0xC0);
    for k in 0..8u32 {
        for c in 0..3u32 {
            let a = fx.mem.read_u32(sp + 0xB0 + 4 * c);
            fx.mem.write_u32(D + 0x80 + 0x10 * k + 4 * c, a);
            let b = fx.mem.read_u32(sp + 0xC0 + 4 * c);
            fx.mem.write_u32(D + 0x100 + 0x10 * k + 4 * c, b);
        }
    }
    fx.mem.set16(D + 0x7E, 0);
    // The square cloud (+0x210, 60 points at +0x230 + 12 k).
    let tex = fx.mem.read_u32(sp + 0xE0);
    fx.mem.write_u32(D + 0x210, tex);
    fx.mem.write_u32(D + 0x214, 0x2008_0001);
    halves(fx, &[(0x220, 0xB0), (0x222, 0x40), (0x224, 0x20), (0x226, 0x20)]);
    bytes(fx, &[(0x21A, 0xD0), (0x219, 0x60), (0x218, 0x40), (0x21B, 0xFF)]);
    fx.mem.write_u32(D + 0x21C, 0x3F00_0000);
    for k in 0..0x3Cu32 {
        let p = D + 0xC * k;
        if bit(fx.mem.read_u32(D + 0x214), 0x1D) {
            let x = rand_f(fx, 0x01E0_46E0, 0x64, 0x32);
            fx.mem.setf(p + 0x230, x);
            let z = rand_f(fx, 0x01E0_4720, 0x32, 0);
            fx.mem.setf(p + 0x238, z);
            let y = rand_f(fx, 0x01E0_4750, 0x64, 0x32);
            fx.mem.setf(p + 0x234, y);
        } else {
            let x = rand_f(fx, 0x01E0_478C, 0x32, 0x19);
            fx.mem.setf(p + 0x230, x);
            let y = rand_f(fx, 0x01E0_47CC, 0x19, 0);
            fx.mem.setf(p + 0x234, y);
            let z = rand_f(fx, 0x01E0_47FC, 0x32, 0x19);
            fx.mem.setf(p + 0x238, z);
        }
    }
    // The wave (+0x500).
    let tex = fx.mem.read_u32(sp + 0xE0);
    fx.mem.write_u32(D + 0x500, tex);
    fx.mem.write_u32(D + 0x50C, D + 0x1730);
    fx.mem.write_u32(D + 0x504, 0x2000_0000);
    halves(fx, &[(0x510, 0xB0), (0x512, 0x40), (0x514, 0x20), (0x516, 0x20)]);
    bytes(fx, &[(0x50A, 0x80), (0x509, 0x10), (0x508, 0x10), (0x50B, 0xFF)]);
    fx.mem.write_u32(D + 0x520, 0x3F80_0000);
    fx.mem.write_u32(D + 0x524, 0x3CF5_C28F);
    halves(fx, &[(0x52A, 0), (0x528, 0x100)]);
    fx.mem.write_u8(D + 0x52C, 7);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x1394);
    let h = fx.self_handle();
    fx.unit_point(1, h, D + 0xE60);
    let t = fx.target_handle();
    fx.unit_position(t, D + 0x1C0);
    node(fx, 0x13A0, sp + 0xB0);
    node(fx, 0x13A4, sp + 0xC0);
    swath::push(fx, D + 0x70, sp + 0xB0, sp + 0xC0);
    node(fx, 0x139C, D + 0xEE0);
    fx.look_angles(sp + 0xC0, D + 0xEE0, D + 0xED0);
    if n(fx) == 0x5B {
        let h = fx.self_handle();
        fx.unit_point(1, h, D + 0x550);
        fx.mem.write_u32(D + 0x554, 0x41A0_0000);
        if !ee_le(fx.mem.f(D + 0x558), 0.0) {
            fx.mem.write_u32(D + 0x544, 0x8000);
        }
    }
    // s0 = D, s1 = sp + 0xC0, s2 = the OT, s3 the runner's (0).
    let regs = [Some(D), Some(sp + 0xC0), Some(OT), Some(0)];
    fx.xform_world_saving(D, 0x01E0_49D8, regs);
    fx.xform_billboard_saving(D + 0x1A0, 0x01E0_49E4, regs);
    fx.xform_world_saving(D + 0x530, 0x01E0_49F0, regs);
    fx.xform_billboard_saving(D + 0xE40, 0x01E0_49FC, regs);
    fx.xform_world_saving(D + 0xEC0, 0x01E0_4A08, regs);
    if (0xC..0xC0).contains(&n(fx)) {
        zoom::sprites(fx, ZOOM, D + 0xEB0, D + 0xE60);
        if n(fx) >= 0x64 {
            fx.step_down(D + 0xEB7, 0, 4);
        } else {
            fx.step_up(D + 0xEB7, 0x40, 2);
        }
    }
    if n(fx) >= 0x5B {
        blade::draw(fx, OT, D + 0x5A0, D + 0x530);
        fstep_up(fx, D + 0x5AC, 50.0, 2.0);
        let mut k = 0;
        while k < fx.mem.i16(D + 0x63A) {
            let p = fx.mem.read_u32(D + 0x620) + 0x10 * k as u32;
            fx.step_down(p + 0xC, 0, 8);
            k += 1;
        }
    }
    if n(fx) >= 0x5B {
        for o in [0xD8u32, 0xD4, 0xD0] {
            fx.mem.write_u32(sp + o, 0x3F83_D70A);
        }
        pointquad::squares(fx, OT, D + 0x210, D + 0x1A0, sp + 0xD0);
        if fx.approach(D + 0x1A0, f32::from_bits(0x3E99_999A), f32::from_bits(0x3DCC_CCCD)) == 0 {
            fx.step_down(D + 0x21B, 0, 0x10);
        }
        wave::draw(fx, OT, D + 0x500, D + 0x1A0);
        // wave::draw's `sq s7` / `sq fp` / `sq ra` (ra 0x1E04B88; upper
        // halves 0): the missing-texture sprites' TEX0 top word and the
        // XYZ F bytes read them [run].
        let w = sp - 0x150;
        fx.mem.write_u64(w + 0x88, 0);
        fx.mem.write_u64(w + 0x98, 0);
        fx.saved(w, &[(0xA0, 0x01E0_4B88)]);
        fx.step_down(D + 0x50B, 0, 0x10);
    }
    if (0x53..0x64).contains(&n(fx)) {
        swath::draw(fx, D + 0x70, D);
    }
    if n(fx) == 0x5B {
        fx.mem.write_u8(D + 0xF3B, 0x80);
        let mut k = 0;
        while k < fx.mem.i16(D + 0xF8A) {
            fx.mem.write_u8(D + 0xF9C + 0x10 * k as u32, 0x80);
            k += 1;
        }
    }
    fx.sprites(D + 0xF30, D + 0xEC0);
    if n(fx) >= 0x65 {
        fx.step_down(D + 0xF3B, 0, 0x10);
    }
    let mut k = 0i32;
    while k < fx.mem.i16(D + 0xF8A) {
        let p = D + 0x10 * k as u32;
        k += 1;
        if fx.mem.i16(p + 0xF9E) < 0 {
            continue;
        }
        let y = ee::add(fx.mem.f(p + 0xF94), 2.0);
        fx.mem.setf(p + 0xF94, y);
        let g = f32::from_bits(0x3FB3_3333);
        for o in [0xF90u32, 0xF98] {
            let v = ee::mul(fx.mem.f(p + o), g);
            fx.mem.setf(p + o, v);
        }
        fx.step_down(p + 0xF9C, 0, 8);
        if !ee_le(fx.mem.f(p + 0xF94), 20.0) {
            throw(fx, p, [0x01E0_4D00, 0x01E0_4D0C, 0x01E0_4D2C, 0x01E0_4D64, 0x01E0_4DD4]);
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x1394, m);
    let left = fx.mem.i32(D + 0x1390) - 1;
    fx.mem.set32(D + 0x1390, left);
    if m == 0x64 {
        2
    } else if m == 0x51 {
        1
    } else if (0x5B..0x64).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
