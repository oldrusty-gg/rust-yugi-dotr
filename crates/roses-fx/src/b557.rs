//! BMODULE b557 (card 642), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03CB0, data area a0 = 0x1ED0000) [F] (helper B9, 31
//! Oct 2026): b101's program shape. The stage dims (0x1F7F10, +0x2864)
//! until update 220; from update 72 a cloud of 128 sprites (+0x160, in
//! transform +0xF0) rising in rings round the attacker's node 0x1C and
//! respawning above 20; sparks (+0xA30) from node 0x3C (+0x9E0) growing by
//! 1.3 and respawning after 13; from 183 a disc (+0xE0) at the target and
//! its afterimage in wavy strips (+0x2820, `blib::afterimage`) with the
//! unit tinted (0x1F75F0); from 187 three rings (+0x1BB0 + 0x400 k)
//! swelling at the target, each turned by the look angles from the
//! attacker. Returns 1 on update 173, 6 on 183..219, 2 on 220, 8 after 240.
//!
//! Library (record addresses): `colours4` 0x1E00080, `strips` 0x1E00120,
//! `scatter` 0x1E00790, `trail::sparks` 0x1E00950, `disc` 0x1E016B0,
//! `ring_setup` / `ring_draw` 0x1E01C10 / 0x1E01ED0, `sprites` 0x1E023C0.
use crate::b101::unit_tint;
use crate::blib::{afterimage, spawn, trail};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E03CB0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xB0);
    ret
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

/// A sprite placed on a ring (the init loop / the respawn): angle rand &
/// 0xFFFF, radius 0.1 (rand % 60 - 30); x = r sin, z = r cos.
fn ring_place(fx: &mut Fx, p: u32, ras: [u32; 2]) {
    let a = fx.rand_from(D, ras[0]) & 0xFFFF;
    let r = fx.rand_from(D, ras[1]);
    let (_, h) = fx.div(r, 0x3C);
    let rad = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h - 0x1E));
    let c = fx.tcos(a);
    fx.mem.setf(p + 0x1C8, ee::mul(rad, c));
    let s = fx.tsin(a);
    fx.mem.setf(p + 0x1C0, ee::mul(rad, s));
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2870);
    for x in [0u32, 0x70, 0x9C0, 0x1A60, 0x1AD0, 0x1B40, 0x27B0, 0xF0] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x2854u32, 0x1B, 1), (0x2858, 0x1C, 0), (0x285C, 0x3C, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(D + 0x2848, 0xF0);
    fx.mem.set32(D + 0x284C, 1);
    fx.mem.set32(D + 0x2850, param);
    let rad = crate::b107::target_radius(fx);
    fx.mem.setf(D + 0x2868, rad);
    fx.mem.set32(D + 0x2864, 0x80);
    // The disc (+0xE0): radius 15.
    fx.mem.write_u32(D + 0xE0, 0x8_0000);
    bytes(fx, &[(0xE6, 0x40), (0xE5, 0x60), (0xE4, 0x80), (0xE7, 0xFF)]);
    fx.mem.write_u32(D + 0xE8, 0x4170_0000);
    halves(fx, &[(0xEC, 0x24), (0xEE, 0)]);
    // The afterimage (+0x2820).
    fx.mem.write_u32(D + 0x2820, 0x4_1000);
    bytes(fx, &[(0x2826, 0x40), (0x2825, 0x60), (0x2824, 0x80), (0x2827, 0x80)]);
    fx.mem.write_u32(D + 0x2830, 0);
    fx.mem.write_u32(D + 0x282C, 0x2000);
    halves(fx, &[(0x2828, 0)]);
    fx.mem.write_u32(D + 0x2834, 0x4120_0000);
    let t = fx.target_handle();
    halves(fx, &[(0x282A, t)]);
    bytes(fx, &[(0x2862, 0x80), (0x2861, 0x80), (0x2860, 0x80), (0x2863, 0x80)]);
    // The sparks (+0xA30).
    fx.texture5(sp + 0xA0, 0);
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0xA30, tex);
    fx.mem.write_u32(D + 0xA34, 3);
    bytes(fx, &[(0xA3A, 0x80), (0xA39, 0x80), (0xA38, 0x80), (0xA3B, 0)]);
    fx.mem.write_u32(D + 0xA50, 0x3F33_3333);
    fx.mem.write_u32(D + 0xA54, 0x4080_0000);
    halves(fx, &[(0xA40, 0x20), (0xA42, 0x20), (0xA44, 0x20), (0xA46, 0x20), (0xA48, 0x20), (0xA4A, 0x20), (0xA4C, 0x80), (0xA4E, 0x40)]);
    fx.mem.write_u32(D + 0xA3C, D + 0xA60);
    fx.mem.write_u32(D + 0xA58, 0x4120_0000);
    halves(fx, &[(0xA5C, 0x100)]);
    bytes(fx, &[(0xA5E, 1), (0xA5F, 0)]);
    spawn::scatter(fx, D + 0xA30, 0xC, 3.0);
    // The sprites (+0x160): 128 particles at +0x1C0.
    fx.texture5(sp + 0xA0, 1);
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x160, tex);
    fx.mem.write_u32(D + 0x164, 0x7000_0003);
    bytes(fx, &[(0x16A, 0x80), (0x169, 0x80), (0x168, 0x80), (0x16B, 0x80)]);
    fx.mem.write_u32(D + 0x16C, 0x3F80_0000);
    fx.mem.write_u32(D + 0x180, 0);
    fx.mem.write_u32(D + 0x184, 0x400);
    fx.mem.write_u32(D + 0x188, 0);
    halves(fx, &[(0x190, 0), (0x192, 0x30), (0x194, 0x20), (0x196, 0x20), (0x198, 0), (0x19A, 0x30), (0x19C, 0x100), (0x19E, 0x100)]);
    fx.mem.write_u32(D + 0x1A0, D + 0x1C0);
    fx.mem.write_u32(D + 0x1A4, 0);
    fx.mem.write_u32(D + 0x1A8, 0);
    fx.mem.write_u32(D + 0x1AC, 0x4000_0000);
    halves(fx, &[(0x1BA, 0x80), (0x1BC, 8), (0x1BE, 0x64)]);
    let mut k = 0u32;
    while (k as i32) < fx.mem.i16(D + 0x1BA) {
        let p = D + 0x10 * k;
        fx.mem.set16(p + 0x1CE, 1);
        fx.mem.write_u8(p + 0x1CC, 0);
        let r = fx.rand_from(D, 0x01E0_3F98);
        fx.mem.set8(p + 0x1CD, r % 8);
        ring_place(fx, p, [0x01E0_3FB4, 0x01E0_3FD0]);
        let r = fx.rand_from(D, 0x01E0_4020);
        let (_, h) = fx.div(r, 0xC8);
        fx.mem.setf(p + 0x1C4, ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h)));
        k += 1;
    }
    // Three rings (+0x1BB0 + 0x400 k).
    fx.texture4(sp + 0xA0, 0xF);
    for k in 0..3u32 {
        let g = 0x1BB0 + 0x400 * k;
        fx.mem.write_u32(D + g + 4, 0);
        halves(fx, &[(g + 0x10, 0x8E), (g + 0x12, 0), (g + 0x14, 0x20), (g + 0x16, 0x80)]);
        bytes(fx, &[(g + 0xA, 0x40), (g + 9, 0x60), (g + 8, 0x80), (g + 0xB, 0x80)]);
        let tex = fx.mem.read_u32(sp + 0xA0);
        fx.ring_setup(D + g, tex, f32::from_bits(0x413C_CCCD), f32::from_bits(0x4143_3333), 0.0, 0.0);
    }
    for (x, v) in [(0x1A60u32, 0u32), (0x1AD0, 0x3F00_0000), (0x1B40, 0x3F80_0000)] {
        for o in [8u32, 4, 0] {
            fx.mem.write_u32(D + x + o, v);
        }
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0x284C);
    for (slot, out) in [(0x2854u32, sp + 0x80), (0x2858, sp + 0x90), (0x2858, D + 0x110)] {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + slot);
        fx.node_position(h, s, out);
    }
    let t = fx.target_handle();
    fx.unit_position(t, D + 0x90);
    if n(fx) < 0xB9 {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + 0x285C);
        fx.node_position(h, s, D + 0x9E0);
    }
    for k in 0..3u32 {
        let t = fx.target_handle();
        fx.unit_position(t, D + 0x1A80 + 0x70 * k);
        let h = fx.self_handle();
        fx.unit_point(1, h, sp + 0x80);
        let t = fx.target_handle();
        fx.unit_point(1, t, sp + 0x90);
        fx.look_angles(sp + 0x80, sp + 0x90, D + 0x1A70 + 0x70 * k);
    }
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x27D0);
    // The transforms' frames (s0 = D, s1 = the OT, s2 / s3 the loops'):
    // the sparks' missing-texture TEX0 / TEX1 read the last TRS matrix.
    let regs = |s2: u32, s3: u32| [Some(D), Some(OT), Some(s2), Some(s3)];
    fx.xform_world_saving(D, 0x01E0_4238, regs(D + 0x150, sp + 0x90));
    fx.xform_billboard_saving(D + 0x70, 0x01E0_4240, regs(D + 0x150, sp + 0x90));
    fx.xform_world_saving(D + 0x9C0, 0x01E0_4248, regs(D + 0x150, sp + 0x90));
    for k in 0..3u32 {
        fx.xform_world_saving(D + 0x1A60 + 0x70 * k, 0x01E0_4258, regs(D + 0x70 * k, k));
    }
    fx.xform_world_saving(D + 0x27B0, 0x01E0_4274, regs(D + 0x150, 3));
    fx.xform_world_saving(D + 0xF0, 0x01E0_427C, regs(D + 0x150, 3));
    if n(fx) < 0xDC {
        fx.step_down(D + 0x2864, 0x20, 2);
    } else {
        fx.step_up(D + 0x2864, 0x80, 4);
    }
    let l = fx.mem.read_u8(D + 0x2864);
    fx.set_stage_light(i32::from(l));
    if n(fx) >= 0x48 {
        fx.sprites(D + 0x160, D + 0xF0);
        if n(fx) >= 0x91 {
            fx.step_down(D + 0x16B, 0, 8);
        }
        let mut k = 0u32;
        while (k as i32) < fx.mem.i16(D + 0x1BA) {
            let p = D + 0x10 * k;
            k += 1;
            if fx.mem.i16(p + 0x1CE) < 0 {
                continue;
            }
            let y = ee::add(fx.mem.f(p + 0x1C4), 1.0);
            fx.mem.setf(p + 0x1C4, y);
            if fx.mem.read_u8(p + 0x1CC) != 0 {
                if ee_le(fx.mem.f(p + 0x1C4), 10.0) {
                    fx.step_up(p + 0x1CC, 0x80, 8);
                } else {
                    fx.step_down(p + 0x1CC, 0, 8);
                }
            }
            if ee_lt(fx.mem.f(p + 0x1C4), 20.0) {
                let g = f32::from_bits(0x3F86_6666);
                for o in [0x1C8u32, 0x1C0] {
                    let v = ee::mul(fx.mem.f(p + o), g);
                    fx.mem.setf(p + o, v);
                }
            }
            if !ee_lt(fx.mem.f(p + 0x1C4), 20.0) {
                ring_place(fx, p, [0x01E0_43F8, 0x01E0_4414]);
                let r = fx.rand_from(D, 0x01E0_4464);
                let (_, h) = fx.div(r, 0x3C);
                fx.mem.setf(p + 0x1C4, ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h - 0x1E)));
                fx.mem.write_u8(p + 0x1CC, 1);
                let r = fx.rand_from(D, 0x01E0_44A4);
                fx.mem.set8(p + 0x1CD, r % 8);
            }
        }
    }
    if n(fx) >= 0xB7 {
        fx.disc(D + 0xE0, D + 0x70);
        fx.step_down(D + 0xE7, 0, 0x10);
    }
    if n(fx) == 0xB7 {
        fx.mem.write_u8(D + 0xA3B, 0x80);
    }
    trail::sparks(fx, OT, D + 0xA30, D + 0x9C0);
    fx.step_down(D + 0xA3B, 0, 8);
    let grow = f32::from_bits(0x3FA6_6666);
    let mut k = 0u32;
    while (k as i32) < fx.mem.i16(D + 0xA5C) {
        let p = D + 0x10 * k;
        k += 1;
        if fx.mem.i16(p + 0xA6E) < 0 {
            continue;
        }
        for o in [0xA60u32, 0xA64, 0xA68] {
            let v = ee::mul(fx.mem.f(p + o), grow);
            fx.mem.setf(p + o, v);
        }
        if fx.mem.i16(p + 0xA6E) >= 0xD {
            fx.random_dir(sp + 0x70, 3.0);
            for (o, s) in [(0xA64u32, 0x74u32), (0xA68, 0x78), (0xA60, 0x70)] {
                let v = fx.mem.read_u32(sp + s);
                fx.mem.write_u32(p + o, v);
            }
            let r = fx.rand_from(D, 0x01E0_45C8);
            let (_, h) = fx.div(r, 0xC);
            fx.mem.set16(p + 0xA6E, -h);
            fx.mem.write_u8(p + 0xA6C, 0x80);
        }
    }
    if n(fx) >= 0xBB {
        let tenth = f32::from_bits(0x3DCC_CCCD);
        for k in 0..3u32 {
            let (g, x) = (D + 0x1BB0 + 0x400 * k, D + 0x1A60 + 0x70 * k);
            fx.ring_draw(g, x);
            fx.step_down(g + 0xB, 0, 8);
            if fx.approach(x, 2.0, tenth) == 0 {
                for o in [8u32, 4, 0] {
                    fx.mem.setf(x + o, tenth);
                }
                for (o, ra) in [(0x10u32, 0x01E0_4680), (0x14, 0x01E0_469C), (0x18, 0x01E0_46B8)] {
                    let r = fx.rand_from(D, ra);
                    fx.mem.set32(x + o, r & 0xFFFF);
                }
                if n(fx) < 0xDC {
                    fx.mem.write_u8(g + 0xB, 0x80);
                }
            }
        }
    }
    if n(fx) >= 0xB7 {
        let a = fx.mem.read_u8(D + 0x2827);
        fx.mem.write_u8(D + 0x2863, 0x80u8.wrapping_sub(a));
        afterimage::strips(fx, D + 0x2820, D + 0x27D0);
        let h = fx.mem.i16(D + 0x282A);
        unit_tint(fx, h, D + 0x2860);
        if n(fx) >= 0xDD {
            fx.step_down(D + 0x2827, 0, 8);
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0x284C, m);
    let left = fx.mem.i32(D + 0x2848) - 1;
    fx.mem.set32(D + 0x2848, left);
    if m == 0xDC {
        2
    } else if m == 0xAD {
        1
    } else if (0xB7..0xDC).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
