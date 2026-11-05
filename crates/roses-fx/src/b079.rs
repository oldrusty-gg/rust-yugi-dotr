//! BMODULE b079 (card 437), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04240, data area 0x1ED0000) [F] (helper C7, 31
//! Oct 2026): three motes (transforms +0x9C0 + 0x70 k) gather at the
//! attacker's node 0x14 one by one (updates 0x73 / 0x87 / 0x9B), fly up
//! (0x104490 toward +0x17E0) and on to the target (from 0xA0, one more
//! every 0x14 updates), each carrying a particle cloud (+0xB90 + 0x150 k)
//! and a facing quad (+0xB10, `blib::facing`); a quad trail (+0x70)
//! pushed every third update of 0x6D..0x75, 0x81..0x89 and 0x95..0x9D at
//! node 0x14, drifting along the nodes 0x18 / 0x17, drawn until 0x9F with
//! its cells rising; from 0xB0 a wave (+0x17B0) and 32 sprites (+0xFF0)
//! at the target. Returns 1 on update 0xA6, 6 on 0xB0..0xDD, 2 on 0xDE, 8
//! after 0xFA.
//!
//! Library at its own addresses: `colours4` (0x1E00080), `facing::quad`
//! (0x1E00120), `sprites` (0x1E009B0), `particle_quads` (0x1E022A0),
//! `wave::draw` (0x1E02E30), `quadtrail` push / draw (0x1E03630 /
//! 0x1E03830).
use crate::b186::{bytes, halves};
use crate::blib::{facing, quadtrail, wave};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const TRAIL_DRAW: u32 = 0x01E0_3830;

/// The entry 0x1E04240.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xE0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xE0);
    ret
}

fn node(fx: &mut Fx, at: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(D + at);
    fx.node_position(h, s, out);
}

/// 0x103660(a, b) [F]: a -= b (xyz).
pub(crate) fn sub3(fx: &mut Fx, a: u32, b: u32) {
    for k in [0u32, 4, 8] {
        let v = ee::sub(fx.mem.f(a + k), fx.mem.f(b + k));
        fx.mem.setf(a + k, v);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x1920);
    fx.xform_init(D);
    fx.xform_init(D + 0xF80);
    for k in 0..3u32 {
        fx.xform_init(D + 0x9C0 + 0x70 * k);
    }
    fx.xform_init(D + 0x1450);
    for (at, n) in [(0x17FCu32, 0x18), (0x1800, 0x17), (0x1804, 0x14)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(at == 0x17FC));
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x17F0, 0xFA);
    fx.mem.set32(D + 0x17F4, 1);
    fx.mem.set32(D + 0x17F8, param);
    for k in 0..3u32 {
        fx.mem.set32(D + 0x1808 + 4 * k, 0x10);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(k, 0x01E0_4358);
        fx.mem.write_u8(D + 0x1814 + k, (r >> 4) as u8);
    }
    fx.mem.write_u32(D + 0x17E8, 0);
    fx.mem.write_u32(D + 0x17E0, 0);
    fx.mem.write_u32(D + 0x17E4, 0x4248_0000);
    fx.texture4(sp + 0xD0, 7);
    // The facing quad (+0xB10, its quad +0xB70).
    fx.mem.write_u32(D + 0xB48, 0x801);
    halves(fx, &[(0xB10, 0), (0xB12, 0), (0xB14, 0x40), (0xB16, 0x40), (0xB18, 0), (0xB1A, 0), (0xB1C, 0x80), (0xB1E, 0x40)]);
    fx.mem.write_u32(D + 0xB40, 0x3F80_0000);
    bytes(fx, &[(0xB46, 0x80), (0xB45, 0x80), (0xB44, 0x80), (0xB47, 0x80)]);
    halves(fx, &[(0xB58, 0), (0xB5A, 0), (0xB5C, 0), (0xB5E, 1), (0xB60, 0), (0xB62, 0x3E7)]);
    let tex = fx.mem.read_u32(sp + 0xD0);
    for o in [0xB88u32, 0xB84, 0xB80] {
        fx.mem.write_u32(D + o, 0);
    }
    fx.mem.write_u32(D + 0xB78, 0x4020_0000);
    fx.mem.write_u32(D + 0xB70, tex);
    bytes(fx, &[(0xB7E, 0x80), (0xB7D, 0x80), (0xB7C, 0x80), (0xB7F, 0x80)]);
    // The wave (+0x17B0).
    let tex = fx.mem.read_u32(sp + 0xD0);
    fx.mem.write_u32(D + 0x17B0, tex);
    fx.mem.write_u32(D + 0x17BC, D + 0x1814);
    fx.mem.write_u32(D + 0x17B4, 0x1000);
    halves(fx, &[(0x17C0, 0x40), (0x17C2, 0x40), (0x17C4, 0x20), (0x17C6, 0x20)]);
    bytes(fx, &[(0x17BA, 0x80), (0x17B9, 0x80), (0x17B8, 0x80), (0x17BB, 0x80)]);
    fx.mem.write_u32(D + 0x17D0, 0x3FC0_0000);
    fx.mem.write_u32(D + 0x17D4, 0x3D4C_CCCD);
    halves(fx, &[(0x17DA, 0), (0x17D8, 0x100)]);
    fx.mem.write_u8(D + 0x17DC, 7);
    // a1 is still the 1 stored at +0xB5E: texture 1.
    fx.texture4(sp + 0xD0, 1);
    // Three particle clouds (+0xB90 + 0x150 k).
    for k in 0..3u32 {
        let g = 0xB90 + 0x150 * k;
        fx.mem.write_u32(D + g + 4, 0xC000_0001);
        let tex = fx.mem.read_u32(sp + 0xD0);
        fx.mem.write_u32(D + g, tex);
        for o in [0x10u32, 0x14, 0x18, 0x1C] {
            fx.mem.write_u32(D + g + o, 0);
        }
        halves(fx, &[(g + 0x20, 0), (g + 0x22, 0), (g + 0x24, 0x18), (g + 0x26, 0x20), (g + 0x28, 0), (g + 0x2A, 0), (g + 0x2C, 0x80), (g + 0x2E, 0x40)]);
        fx.mem.write_u32(D + g + 0xC, 0x40A0_0000);
        halves(fx, &[(g + 0x130, 0x10), (g + 0x13A, 1), (g + 0x13C, 1), (g + 0x140, 0xA), (g + 0x136, 3), (g + 0x138, 3)]);
        bytes(fx, &[(g + 0xA, 0x80), (g + 9, 0x80), (g + 8, 0x80), (g + 0xB, 0x60)]);
        halves(fx, &[(g + 0x132, -0x6000)]);
        let mut j = 0i32;
        while j < fx.mem.i16(D + g + 0x130) {
            let p = D + g + 0x10 * j as u32;
            for (o, ra) in [(0u32, 0x01E0_454Cu32), (4, 0x01E0_4588)] {
                let base = fx.mem.f(D + g + 0x10 + o);
                let r = fx.rand_from(j as u32, ra);
                let (_, h) = fx.div(r, 0x3C);
                let v = ee::add(base, ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h - 0x1E)));
                fx.mem.setf(p + 0x30 + o, v);
            }
            let z = fx.mem.read_u32(D + g + 0x18);
            fx.mem.write_u32(p + 0x38, z);
            fx.mem.set16(p + 0x3E, 0);
            let r = fx.rand_from(j as u32, 0x01E0_45D0);
            let (_, h) = fx.div(r, 0xA);
            j += 1;
            fx.mem.set16(p + 0x3C, h);
            let c = fx.mem.i16(D + g + 0x140) - 1;
            fx.mem.set16(D + g + 0x13E, c);
        }
    }
    // The sprites (+0xFF0, 32 at +0x1050).
    let tex = fx.mem.read_u32(sp + 0xD0);
    fx.mem.write_u32(D + 0xFF0, tex);
    fx.mem.write_u32(D + 0xFF4, 0x4800_0003);
    bytes(fx, &[(0xFFA, 0x80), (0xFF9, 0x80), (0xFF8, 0x80), (0xFFB, 0x40)]);
    fx.mem.write_u32(D + 0xFFC, 0x41A0_0000);
    for o in [0x1010u32, 0x1014, 0x1018] {
        fx.mem.write_u32(D + o, 0);
    }
    halves(fx, &[(0x1020, 0), (0x1022, 0), (0x1024, 0x18), (0x1026, 0x20), (0x1028, 0), (0x102A, 0), (0x102C, 0x80), (0x102E, 0x40)]);
    fx.mem.write_u32(D + 0x1030, D + 0x1050);
    fx.mem.write_u32(D + 0x1034, D + 0x1250);
    fx.mem.write_u32(D + 0x1038, 0);
    fx.mem.write_u32(D + 0x103C, 0x4170_0000);
    fx.mem.write_u32(D + 0x1040, 0x4120_0000);
    halves(fx, &[(0x104A, 0x20), (0x104C, 8), (0x104E, 0x10)]);
    let mut k = 0i32;
    while k < fx.mem.i16(D + 0x104A) {
        let p = D + 0x10 * k as u32;
        fx.random_dir(sp + 0xC0, 14.0);
        for o in [0u32, 4, 8] {
            let v = fx.mem.read_u32(sp + 0xC0 + o);
            fx.mem.write_u32(p + 0x1050 + o, v);
        }
        let r = fx.rand_from(p, 0x01E0_46EC);
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0x105E, h.wrapping_neg());
        fx.mem.write_u8(p + 0x105C, 0x40);
        let r = fx.rand_from(p, 0x01E0_4714);
        fx.mem.write_u8(p + 0x105D, r.wrapping_rem(8) as u8);
        k += 1;
    }
    fx.texture4(sp + 0xD0, 0xF);
    // The quad trail (+0x70, cells +0xC0).
    let tex = fx.mem.read_u32(sp + 0xD0);
    fx.mem.write_u32(D + 0x70, tex);
    bytes(fx, &[(0x76, 0xB0), (0x75, 0x70), (0x74, 0x40), (0x77, 0x30)]);
    fx.mem.write_u32(D + 0x78, 0x1_8003);
    fx.mem.write_u32(D + 0x7C, D + 0xC0);
    halves(fx, &[(0x80, 0x80), (0x82, 0xC0), (0x84, 0x20), (0x86, 0x20), (0x88, 0x80), (0x8A, 0xC0), (0x8C, 0x80), (0x8E, 0x100)]);
    for (o, v) in [(0x90u32, 0u32), (0x94, 0x3E99_999A), (0x98, 0), (0xA0, 0x4040_0000), (0xA4, 0x3F00_0000), (0xA8, 0x4013_3333)] {
        fx.mem.write_u32(D + o, v);
    }
    halves(fx, &[(0xB2, 0), (0xB4, 0x10), (0xB6, 0x24), (0xB8, 0), (0xBA, 4), (0xBC, 8), (0xBE, 0)]);
    for x in [0x9C0u32, 0xA30, 0xAA0, 0x1450] {
        for o in [8u32, 4, 0] {
            fx.mem.write_u32(D + x + o, 0x3DCC_CCCD);
        }
    }
    0
}

/// -1 / 0 / 1 / 2 by the update against three marks (the motes not yet
/// at their place: from 2 down to this + 1).
fn stage(n: i32, marks: [i32; 3]) -> i32 {
    if n < marks[0] {
        -1
    } else if n < marks[1] {
        0
    } else if n < marks[2] {
        1
    } else {
        2
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x17F4);
    for out in [0xFA0u32, 0x1470] {
        let t = fx.target_handle();
        fx.unit_point(1, t, D + out);
    }
    let s = stage(n(fx), [0x73, 0x87, 0x9B]);
    let mut k = 2;
    while s < k {
        node(fx, 0x1804, D + 0x9E0 + 0x70 * k as u32);
        k -= 1;
    }
    let m = n(fx);
    let pushed = [(0x6D, 0x76), (0x81, 0x8A), (0x95, 0x9E)].iter().any(|&(a, b)| (a..b).contains(&m));
    if pushed {
        let (_, rem) = fx.div(m, 3);
        if rem == 0 {
            node(fx, 0x17FC, sp + 0x90);
            node(fx, 0x1800, sp + 0xA0);
            node(fx, 0x1804, sp + 0xB0);
            fx.midpoint(sp + 0x90, sp + 0xA0, sp + 0xC0);
            sub3(fx, sp + 0xC0, sp + 0xB0);
            let f = f32::from_bits(0x3E4C_CCCD);
            for (o, s) in [(0x90u32, 0xC0u32), (0x94, 0xC4), (0x98, 0xC8)] {
                let v = ee::mul(f, fx.mem.f(sp + s));
                fx.mem.setf(D + o, v);
            }
            quadtrail::push(fx, D + 0x70, sp + 0xB0);
        }
    }
    for o in [0xC8u32, 0xC4, 0xC0] {
        fx.mem.write_u32(sp + o, 0);
    }
    if n(fx) == 0xA0 {
        for k in 0..3u32 {
            for c in 0..4u32 {
                let v = fx.mem.read_u32(D + 0x17E0 + 4 * c);
                fx.mem.write_u32(D + 0x9E0 + 0x70 * k + 4 * c, v);
            }
            fx.mem.set32(D + 0x1808 + 4 * k, 0x10);
        }
    }
    let m = n(fx);
    // s3 at the transforms: the count (before 0xA0) or D + 4 count.
    let s3;
    if m < 0xA0 {
        let c = if m >= 0x9A {
            3
        } else if m >= 0x86 {
            2
        } else if m >= 0x72 {
            1
        } else {
            0
        };
        for k in 0..c as u32 {
            fx.approach(D + 0x9C0 + 0x70 * k, 1.0, f32::from_bits(0x3DCC_CCCD));
            let t = fx.mem.i32(D + 0x1808 + 4 * k);
            fx.approach_by(D + 0x9E0 + 0x70 * k, D + 0x17E0, t);
            let t = fx.mem.i32(D + 0x1808 + 4 * k) - 1;
            fx.mem.set32(D + 0x1808 + 4 * k, t);
        }
        s3 = c as u32;
    } else {
        let c = if m >= 0xC8 {
            3
        } else if m >= 0xB4 {
            2
        } else {
            1
        };
        for k in 0..c as u32 {
            let t = fx.mem.i32(D + 0x1808 + 4 * k);
            fx.approach_by(D + 0x9E0 + 0x70 * k, D + 0xFA0, t);
            let t = fx.mem.i32(D + 0x1808 + 4 * k) - 1;
            fx.mem.set32(D + 0x1808 + 4 * k, t);
        }
        s3 = D + 4 * c as u32;
    }
    // s0 = sp + 0xC4, s1 = D, s2 = the OT, s3 as above (the loop's D +
    // 0x70 k for the motes' billboards).
    let regs = |s3: u32| [Some(sp + 0xC4), Some(D), Some(OT), Some(s3)];
    fx.xform_world_saving(D, 0x01E0_4B8C, regs(s3));
    fx.xform_billboard_saving(D + 0xF80, 0x01E0_4B94, regs(s3));
    for k in 0..3u32 {
        fx.xform_billboard_saving(D + 0x9C0 + 0x70 * k, 0x01E0_4BA4, regs(D + 0x70 * k));
    }
    fx.xform_billboard_saving(D + 0x1450, 0x01E0_4BC0, regs(D + 0x150));
    if n(fx) < 0xA0 {
        quadtrail::draw(fx, TRAIL_DRAW, OT, D + 0x70, D);
        let mut i = 0;
        while i < fx.mem.i16(D + 0xB8) {
            let mut j = 0;
            while j < fx.mem.i16(D + 0xBA) {
                let w = fx.mem.i16(D + 0xBA);
                let lo = fx.mult(i, w);
                let c = D + 0xC0 + 0x10 * (j + lo) as u32;
                let v = ee::add(fx.mem.f(c + 4), ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(j)));
                fx.mem.setf(c + 4, v);
                fx.step_down(c + 0xC, 0, 3);
                j += 1;
            }
            i += 1;
        }
    }
    let s6 = stage(n(fx), [0xB1, 0xC5, 0xD9]);
    let mut k = 2;
    while s6 < k {
        for o in [0xC8u32, 0xC4, 0xC0] {
            fx.mem.write_u32(sp + o, 0);
        }
        let x = D + 0x9C0 + 0x70 * k as u32;
        fx.particle_quads(x, D + 0xB90 + 0x150 * k as u32, sp + 0xC0);
        facing::quad(fx, OT, D + 0xB10, D + 0xB70, x);
        k -= 1;
    }
    if n(fx) >= 0xB0 {
        if fx.mem.i16(D + 0x17DA) >= 0x15 {
            fx.mem.set16(D + 0x17DA, 0);
            fx.mem.write_u8(D + 0x17BB, 0x80);
        }
        wave::draw(fx, OT, D + 0x17B0, D + 0x1450);
        fx.step_down(D + 0x17BB, 0, 8);
        fx.approach(D + 0x1450, f32::from_bits(0x3E99_999A), f32::from_bits(0x3DCC_CCCD));
    }
    if n(fx) >= 0xB0 {
        fx.sprites(D + 0xFF0, D + 0xF80);
        if n(fx) >= 0xDE {
            fx.step_down(D + 0xFFB, 0, 2);
        } else {
            fx.step_up(D + 0xFFB, 0x40, 1);
        }
        let g = f32::from_bits(0x3F83_D70A);
        let mut k = 0i32;
        while k < fx.mem.i16(D + 0x104A) {
            let p = D + 0x10 * k as u32;
            k += 1;
            if fx.mem.i16(p + 0x105E) >= 0 {
                for o in [0x1050u32, 0x1054, 0x1058] {
                    let v = ee::mul(fx.mem.f(p + o), g);
                    fx.mem.setf(p + o, v);
                }
                fx.step_down(p + 0x105C, 0, 4);
            }
            if fx.mem.i16(p + 0x105E) < 0x10 {
                continue;
            }
            fx.random_dir(sp + 0xC0, 14.0);
            for o in [0u32, 4, 8] {
                let v = fx.mem.read_u32(sp + 0xC0 + o);
                fx.mem.write_u32(p + 0x1050 + o, v);
            }
            fx.mem.set16(p + 0x105E, 0);
            fx.mem.write_u8(p + 0x105C, 0x40);
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x17F4, m);
    let left = fx.mem.i32(D + 0x17F0) - 1;
    fx.mem.set32(D + 0x17F0, left);
    if m == 0xDE {
        2
    } else if m == 0xA6 {
        1
    } else if (0xB0..0xDE).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
