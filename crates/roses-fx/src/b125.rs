//! BMODULE b125 (card 370), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05250, data area a0 = 0x1ED0000, 0x3E80 bytes) [F]
//!: a record of the shared-library family (as
//! b076 / b133). The entry (0x90 frame, s0 = D, s1 = the OT) steps a
//! counter (D+0, u16), reads the target's position (sp + 0x70) and starts
//! records on fixed updates: 0x26 one bead trail (+0xAF0) from the
//! attacker's node 0x2F; 0x3C thirty rising glints (+0xB7C, local
//! [`glints`]) at node 0x41; 0x8E two rings of 32 swinging plates (+0x10A4,
//! local [`plates`]) at the target; 0xC4 fifty columns (+0x28AC, local
//! [`columns`]) and thirty-six fans (+0x362C, local [`fans`]) at the
//! target. On 0xC4..0xDB the plates spin up, fade and open. A glint on
//! its count 7 starts six flickers (+0x21A4) and a flash (+0x3464). The
//! flicker / flash pools are drawn after those starts. Returns 8 on 0x154
//! (and at once for -2 / -3), 6 on 0xD4..0x128, 2 on 0x129, 1 on 0xCA.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `beads`
//! 0x1E00410, `bead_trails` 0x1E011E0, `flickers` 0x1E02C60, `flash`
//! 0x1E03740 [`crate::blib`]; local `glints` 0x1E01730, `plates`
//! 0x1E01F30, `columns` 0x1E042F0, `fans` 0x1E049B0.
use crate::b076::{node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{alpha_mode, atlas_cell, camera_vp_into, countdown, cvt_u32, first_free, libm_cosf, mul_mem, node_source, set_counter4, tex_rect, tex_rect_strip, translation_into}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const ASPECT: u32 = 0x3FAA_3D71;
const PI: u32 = 0x4049_0FDB;
/// RGBAQ / UV / XYZ2 x 4.
const STRIP: u64 = 0x4214_2142_1421;
const TRAIL: u32 = D + 0xAF0;
const BEADS: u32 = D + 0xA0;
const GLINTS: u32 = D + 0xB7C;
const PLATES: u32 = D + 0x10A4;
const FLICKERS: u32 = D + 0x21A4;
const COLUMNS: u32 = D + 0x28AC;
const FLASHES: u32 = D + 0x3464;
const FANS: u32 = D + 0x362C;
const BYTES: u32 = D + 0x3D7C;

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

/// The entry 0x1E05250 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x90);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    // 0x199208 memset(D, 0, 0x3E80).
    fx.bzero(D, 0x3E80);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    for k in 0..2 {
        fx.texture(5, k);
    }
    for (at, node, clear) in [(0xCu32, 0x2F, 1), (0x10, 0x41, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    rand_bytes(fx, BYTES, 0x100, 0x01E0_5364, [Some(D), Some(OT), Some(2)]);
    0
}

/// (rand >> 4) % m - h, over 100.
fn jitter(fx: &mut Fx, m: i32, h: i32, ra: u32) -> f32 {
    let r = fx.rand_from(D, ra) >> 4;
    let v = fx.div(r, m).1 - h;
    ee::div(ee::from_i32(v), 100.0)
}

fn bytes(fx: &mut Fx, r: u32, list: &[(u32, u8)]) {
    for &(o, v) in list {
        fx.mem.write_u8(r + o, v);
    }
}

/// Update 0x26 [F 0x1E053D4]: the bead trail.
fn trail_start(fx: &mut Fx) {
    let r = TRAIL;
    set_counter4(fx, r, 0x10, 0, 0xFF);
    bytes(fx, r, &[(4, 7), (5, 1)]);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 6, b);
    bytes(fx, r, &[(0x13, 1), (0x14, 3), (0x16, 2)]);
    fx.mem.write_u32(r + 8, BEADS);
    bytes(fx, r, &[(0x10, 0x1E), (0x12, 5), (0x11, 0x1C)]);
    fx.mem.write_u32(r + 0x18, 0x3DCC_CCCD);
    fx.mem.write_u32(r + 0x1C, 0x3DCC_CCCD);
    fx.mem.write_u32(r + 0x20, 0x4120_0000);
    fx.mem.write_u32(r + 0xC, BYTES);
    bytes(fx, r, &[(0x2A, 0x60), (0x29, 0xC0), (0x28, 0x60), (0x2B, 0x80)]);
}

/// Update 0x3C [F 0x1E0547C]: the glints at node 0x41, sp + 0x80.
fn glints_start(fx: &mut Fx, sp: u32) {
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x10));
    node_position_saving(fx, h, s, sp + 0x80, u64::from(D), Some(u64::from(OT)), 0x01E0_5490);
    for s2 in 0..0x1Ei32 {
        let k = first_free(fx, GLINTS, 0x2C, 0x1E);
        if k < 0 {
            continue;
        }
        let c = (fx.rand_from(D, 0x01E0_54C4) & 3) + 0x1C;
        let r = GLINTS + 0x2C * k as u32;
        set_counter4(fx, r, c & 0xFF, (s2 << 2) & 0xFF, 0xFF);
        bytes(fx, r, &[(4, 1), (5, 0), (8, 0), (9, 1), (0xA, (s2 & 7) as u8), (0xB, 2)]);
        let v = jitter(fx, 0x12C, 0x96, 0x01E0_5530);
        fx.mem.setf(r + 0xC, ee::add(60.0, v));
        bytes(fx, r, &[(0x13, 0xC0), (0x10, 0xC0), (0x11, 0xC0), (0x12, 0xC0)]);
        for o in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x80 + 4 * o);
            fx.mem.write_u32(r + 0x14 + 4 * o, w);
        }
        let v = jitter(fx, 0x32, 0x19, 0x01E0_55B4);
        fx.mem.setf(r + 0x20, v);
        let v = jitter(fx, 0x32, 0x19, 0x01E0_5600);
        fx.mem.setf(r + 0x24, v);
        let w = if fx.mem.i32(D + 0x30) == 0x8000 { 0xBF33_3333 } else { 0x3F33_3333 };
        fx.mem.write_u32(r + 0x28, w);
        let v = jitter(fx, 0x1E, 0xF, 0x01E0_5678);
        let w = fx.mem.f(r + 0x28);
        fx.mem.setf(r + 0x28, ee::add(w, v));
    }
}

/// 0x1F65D0 `unit_point` with its frame (0x50: the caller's s0..s3, ra)
/// left below the caller's sp.
fn unit_point_saving(fx: &mut Fx, kind: i32, h: i32, out: u32, s: [u64; 4], ra: u32) -> i32 {
    let r = fx.unit_point(kind, h, out);
    let f = fx.sp - 0x50;
    fx.saved(f, &[(0, s[0]), (0x10, s[1]), (0x20, s[2]), (0x30, s[3]), (0x40, u64::from(ra))]);
    r
}

/// Update 0x8E [F 0x1E056DC]: the plates about the target.
fn plates_start(fx: &mut Fx, sp: u32) {
    for s4 in 0..2i32 {
        for s2 in 0..0x20i32 {
            let k = first_free(fx, PLATES, 0x44, 0x40);
            if k < 0 {
                continue;
            }
            let r = PLATES + 0x44 * k as u32;
            bytes(fx, r, &[(4, 0), (5, 0), (8, 0), (0xA, 1)]);
            fx.mem.set8(r + 0xC, (s2 + 5 * s4) & 7);
            bytes(fx, r, &[(0xE, 2), (9, 1), (0xB, 0), (0xD, 3), (0xF, 2)]);
            let s3 = fx.rand_from(D, 0x01E0_5770) & 0xF;
            let a = if s4 == 0 {
                set_counter4(fx, r, (0xBC - s3) & 0xFF, s3 & 0xFF, 0xFF);
                fx.mem.write_u32(r + 0x10, 0x4190_0000);
                fx.mem.write_u32(r + 0x14, 0x41E0_0000);
                fx.mem.write_u32(r + 0x18, 0x4220_0000);
                fx.mem.write_u32(r + 0x1C, 0x3F80_0000);
                s3
            } else {
                set_counter4(fx, r, (0xB4 - s3) & 0xFF, (s3 + 8) & 0xFF, 0xFF);
                fx.mem.write_u32(r + 0x10, 0x41E0_0000);
                fx.mem.write_u32(r + 0x14, 0x4218_0000);
                fx.mem.write_u32(r + 0x18, 0x42A0_0000);
                fx.mem.write_u32(r + 0x1C, 0x3FE6_6666);
                s3 + 8
            };
            // mula.s / madd.s: the products rounded, then the sum.
            let p = ee::mul(fx.mem.f(r + 0x24), ee::from_i32(a));
            let q = ee::mul(f(0x4134_0000), ee::from_i32(s2));
            fx.mem.setf(r + 0x20, ee::add(p, q));
            fx.mem.write_u32(r + 0x28, 0x40B4_0000);
            fx.mem.write_u32(r + 0x2C, 0x4170_0000);
            bytes(fx, r, &[(0x33, 0xF0), (0x30, 0xF0), (0x31, 0xF0), (0x32, 0xF0), (0x36, 0x10), (0x35, 0x80), (0x34, 0x80), (0x37, 0x20)]);
            let t = fx.mem.i32(D + 8);
            unit_point_saving(fx, 1, t, sp + 0x80, [u64::from(D), u64::from(OT), s2 as u64, s3 as u64], 0x01E0_58F0);
            let w = fx.mem.read_u32(sp + 0x80);
            fx.mem.write_u32(r + 0x38, w);
            let y = fx.mem.f(sp + 0x84);
            fx.mem.setf(r + 0x3C, ee::sub(y, 18.0));
            let w = fx.mem.read_u32(sp + 0x88);
            fx.mem.write_u32(r + 0x40, w);
        }
    }
}

/// Update 0xC4 [F 0x1E0594C]: columns and fans at the target's point.
fn last_start(fx: &mut Fx, sp: u32, param: i32) {
    let t = fx.mem.i32(D + 8);
    // Its frame's s3 slot (0: the caller's s3 [H]) is the first column's
    // missing-texture TEX0 [run], the s0 slot its TEX1.
    unit_point_saving(fx, 1, t, sp + 0x80, [u64::from(D), u64::from(OT), param as i64 as u64, 0], 0x01E0_5960);
    for _ in 0..0x32 {
        let k = first_free(fx, COLUMNS, 0x3C, 0x32);
        if k < 0 {
            continue;
        }
        let d = fx.rand_from(D, 0x01E0_5994) & 0xF;
        let r = COLUMNS + 0x3C * k as u32;
        set_counter4(fx, r, 0x78, d, 0xF);
        bytes(fx, r, &[(4, 0), (5, 0), (8, 1), (9, 3), (0xA, 2)]);
        fx.mem.set32(r + 0xC, 0xBB8);
        fx.mem.set32(r + 0x10, 0xF);
        fx.mem.write_u32(r + 0x18, 0x3DCC_CCCD);
        fx.mem.write_u32(r + 0x1C, 0x40C0_0000);
        bytes(fx, r, &[(0x23, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80)]);
        fx.mem.write_u32(r + 0x14, 0x4270_0000);
        let w = fx.mem.read_u32(sp + 0x80);
        fx.mem.write_u32(r + 0x24, w);
        let y = fx.mem.f(sp + 0x84);
        fx.mem.setf(r + 0x28, ee::sub(y, 19.0));
        let w = fx.mem.read_u32(sp + 0x88);
        fx.mem.write_u32(r + 0x2C, w);
    }
    for k in 0..0x24i32 {
        let r = FANS + 0x34 * k as u32;
        set_counter4(fx, r, 0x72, 0x18, 0xFF);
        bytes(fx, r, &[(4, 0), (5, 0), (8, 0xC), (9, 2)]);
        fx.mem.write_u32(r + 0xC, 0x4190_0000);
        fx.mem.write_u32(r + 0x10, 0x3E38_51EC);
        fx.mem.setf(r + 0x14, ee::mul(10.0, ee::from_i32(k)));
        fx.mem.write_u32(r + 0x18, 0);
        fx.mem.write_u32(r + 0x1C, 0x4120_0000);
        bytes(fx, r, &[(0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x27, 0x40), (0x23, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80)]);
        let w = fx.mem.read_u32(sp + 0x80);
        fx.mem.write_u32(r + 0x28, w);
        let w = fx.mem.read_u32(sp + 0x88);
        fx.mem.write_u32(r + 0x30, w);
        let y = fx.mem.f(sp + 0x84);
        fx.mem.setf(r + 0x2C, ee::sub(y, 18.0));
    }
}

/// Glint `s2` on its count 7 [F 0x1E05CE0]: six flickers and a flash.
fn glint_burst(fx: &mut Fx, g: u32) {
    for s4 in 0..6i32 {
        let k = first_free(fx, FLICKERS, 0x3C, 0x1E);
        if k < 0 {
            continue;
        }
        let d = fx.rand_from(D, 0x01E0_5D3C) & 3;
        let r = FLICKERS + 0x3C * k as u32;
        set_counter4(fx, r, 0x10, d, 7);
        bytes(fx, r, &[(5, 0), (4, 0), (8, 1), (9, 3), (0xA, 2), (0xB, 0), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)]);
        fx.mem.write_u32(r + 0x10, 0x41C8_0000);
        fx.mem.set32(r + 0x14, 0xD2);
        for o in 0..3u32 {
            let w = fx.mem.read_u32(g + 0x14 + 4 * o);
            fx.mem.write_u32(r + 0x18 + 4 * o, w);
        }
        fx.mem.write_u32(r + 0x28, if s4 & 1 != 0 { 0x3E99_999A } else { 0xBE99_999A });
        let v = jitter(fx, 0x1E, 0xF, 0x01E0_5E00);
        let w = fx.mem.f(r + 0x28);
        fx.mem.setf(r + 0x28, ee::add(w, v));
    }
    let k = first_free(fx, FLASHES, 0x4C, 6);
    if k < 0 {
        return;
    }
    let r = FLASHES + 0x4C * k as u32;
    set_counter4(fx, r, 8, 0, 0xFF);
    bytes(fx, r, &[(5, 0), (4, 0x80), (7, 9), (0xA, 0x10), (9, 0x80), (8, 0x80), (0xB, 0x10), (0xF, 0xFF), (0xC, 0xFF), (0xD, 0xFF), (0xE, 0xFF)]);
    fx.mem.write_u32(r + 0x10, 0x42C8_0000);
    fx.mem.write_u32(r + 0x14, 0x41A0_0000);
    fx.mem.write_u32(r + 0x18, 0x4040_0000);
    for o in 0..3u32 {
        let w = fx.mem.read_u32(g + 0x14 + 4 * o);
        fx.mem.write_u32(r + 0x1C + 4 * o, w);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x70, u64::from(D), 0x01E0_538C);
    match fx.mem.u16(D) {
        0x154 => return 8,
        0xC4 => last_start(fx, sp, param),
        0x8E => plates_start(fx, sp),
        0x3C => glints_start(fx, sp),
        0x26 => trail_start(fx),
        _ => {}
    }
    let u = fx.mem.u16(D);
    if (0xC4..0xDC).contains(&u) {
        let a = u - 0xC4;
        for k in 0..0x40u32 {
            let r = PLATES + 0x44 * k;
            let v = if fx.mem.f(r + 0x10) == 18.0 {
                ee::mul(f(0x3E4C_CCCD), ee::from_i32(a))
            } else {
                ee::mul(f(0x3E4C_CCCD), ee::from_i32(-a))
            };
            fx.mem.setf(r + 0x24, v);
            let b = fx.mem.read_u8(r + 0x37);
            fx.mem.write_u8(r + 0x37, b.wrapping_add(3));
            let w = fx.mem.f(r + 0x2C);
            fx.mem.setf(r + 0x2C, ee::add(w, 6.0));
        }
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_5C5C, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    blib::emit::bead_trails(fx, 0x01E0_11E0, TRAIL, 1);
    blib::beam::beads(fx, 0x01E0_0410, BEADS, OT, 0x1E);
    glints(fx, GLINTS, OT, 0x1E);
    plates(fx, PLATES, OT, 0x40);
    columns(fx, COLUMNS, OT, 0x32);
    fans(fx, FANS, OT, 0x24);
    for k in 0..0x1Eu32 {
        let g = GLINTS + 0x2C * k;
        if fx.mem.read_u8(g) == 7 {
            glint_burst(fx, g);
        }
    }
    blib::flicker::flickers(fx, 0x01E0_2C60, FLICKERS, OT, 0x1E);
    blib::star::flash(fx, 0x01E0_3740, FLASHES, OT, 6);
    let u = fx.mem.u16(D);
    if (0xD4..0x129).contains(&u) {
        6
    } else if u == 0x129 {
        2
    } else if u == 0xCA {
        1
    } else {
        0
    }
}

/// 0x1A2C70 with its residue.
fn fixu(fx: &mut Fx, v: f32, ra: u32) -> u32 {
    fx.fix_residue(v, ra);
    ee::fixunssfsi(v)
}

/// 0x1A2C70 of `v` into the u16s at `a`.
fn fix16(fx: &mut Fx, a: &[u32], v: f32, ra: u32) {
    let x = fixu(fx, v, ra) as u16;
    for &p in a {
        fx.mem.write_u16(p, x);
    }
}

/// The u16 at `a` as a float.
fn hf(fx: &mut Fx, a: u32) -> f32 {
    ee::from_i32(i32::from(fx.mem.read_u16(a)))
}

/// The z (bits 32..55) `z` through the XYZ2s `at`, each read back.
fn z_chain(fx: &mut Fx, at: &[u32], z: u64) {
    let mut z = z;
    for &o in at {
        fx.set_xyz_z(o, z);
        z = fx.xyz_z(o);
    }
}

/// The record's 0x1E01730 [F] (3c73bbd101e4, b125 only): `glints(recs, ot,
/// n) -> 1` steps and draws `n` records (0x2C bytes; a 0x1C0 frame, the
/// descriptor at sp + 0x120: RGBAQ / UV / XYZ2 x 4). Per record: countdown;
/// the own-slot texture +8; from count 8 the point +0x14 += +0x20; from a
/// node with +5 (a local copy, sp + 0xF0); projected and on screen; colour
/// +0x10..+0x12; flag 0x80 at Z 0xFFFFFE with w 0, else at its Z; the
/// atlas cell +0xA of layout +9, blend +0xB; half size 0x1018E0(+0xC, w)
/// widening over the last 8 counts and opening over the first 16; alpha
/// +0x13 on odd counts, a quarter on even; with flag 1 a second, 1.15 x
/// wider pass of blend 2.
fn glints(fx: &mut Fx, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x1C0);
    let mut g = recs;
    for i in 0..n {
        if i != 0 {
            g += 0x2C;
        }
        glint(fx, sp, g, ot);
    }
    fx.leave(0x1C0);
    1
}

fn glint(fx: &mut Fx, sp: u32, g: u32, ot: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    let id = fx.mem.u8(g + 8);
    let tex = fx.texture(5, id);
    if fx.mem.u8(g) >= 8 {
        for k in 0..3u32 {
            let v = ee::add(fx.mem.f(g + 0x14 + 4 * k), fx.mem.f(g + 0x20 + 4 * k));
            fx.mem.setf(g + 0x14 + 4 * k, v);
        }
    }
    for k in 0..3u32 {
        let w = fx.mem.read_u32(g + 0x14 + 4 * k);
        fx.mem.write_u32(sp + 0xF0 + 4 * k, w);
    }
    if fx.mem.read_u8(g + 5) != 0 {
        node_source(fx, g, sp + 0xF0);
    }
    camera_vp_into(fx, sp + 0xB0);
    let m = fx.read_m4(sp + 0xB0);
    if !fx.project_at(sp + 0xF0, &m, sp + 0x1B0, sp + 0x1BC) {
        return;
    }
    if !Fx::on_screen(fx.mem.read_u64(sp + 0x1B0)) {
        return;
    }
    let d = sp + 0x120;
    fx.desc_init(d, 0xC, 0x34, STRIP);
    for (src, o) in [(0x12u32, 0u32), (0x11, 1), (0x10, 2)] {
        let v = fx.mem.read_u8(g + src);
        for a in [0x178u32, 0x160, 0x148, 0x130] {
            fx.mem.write_u8(sp + a + o, v);
        }
    }
    for a in [0x17Cu32, 0x164, 0x14C, 0x134] {
        fx.mem.write_u32(sp + a, 0x3F80_0000);
    }
    for a in [0x18Fu32, 0x177, 0x15F, 0x147] {
        fx.mem.write_u8(sp + a, 0);
    }
    let corners = [sp + 0x188, sp + 0x170, sp + 0x158, sp + 0x140];
    if fx.mem.u8(g + 4) & 0x80 != 0 {
        z_chain(fx, &corners, 0xFF_FFFE);
        fx.mem.write_u32(sp + 0x1BC, 0);
    } else {
        let z = fx.xyz_z(sp + 0x1B0);
        z_chain(fx, &corners, z);
    }
    let b = fx.mem.read_u8(g + 0xB);
    fx.mem.write_u8(sp + 0x105, b);
    let (cell, layout) = (fx.mem.u8(g + 0xA), fx.mem.u8(g + 9));
    atlas_cell(fx, sp + 0x100, cell, layout);
    tex_rect_strip(fx, d, sp + 0x100, tex);
    let (size, w) = (fx.mem.f(g + 0xC), fx.mem.f(sp + 0x1BC));
    let s = fx.view_scale(size, w);
    let (mut f20, mut f21) = (s, s);
    let c = fx.mem.u8(g);
    let e = fx.mem.u8(g + 2) - c;
    if e < 0x10 {
        f21 = ee::mul(ee::div(s, 16.0), ee::from_i32(e));
        f20 = f21;
    }
    if c < 8 {
        let k = ee::from_i32(8 - c);
        f20 = ee::sub(s, ee::mul(ee::div(s, 8.0), k));
        f21 = ee::add(s, ee::mul(s, k));
    }
    let a = fx.mem.read_u8(g + 0x13);
    let a = if c & 1 != 0 { a } else { a >> 2 };
    for o in [0x17Bu32, 0x163, 0x14B, 0x133] {
        fx.mem.write_u8(sp + o, a);
    }
    let x = hf(fx, sp + 0x1B0);
    fix16(fx, &[sp + 0x170, sp + 0x140], ee::sub(x, f20), 0x01E0_1BC0);
    fix16(fx, &[sp + 0x188, sp + 0x158], ee::add(x, f20), 0x01E0_1C14);
    let y = hf(fx, sp + 0x1B2);
    let f22 = ee::mul(f(ASPECT), f21);
    fix16(fx, &[sp + 0x15A, sp + 0x142], ee::sub(y, f22), 0x01E0_1C84);
    fix16(fx, &[sp + 0x18A, sp + 0x172], ee::add(y, f22), 0x01E0_1CD8);
    let w = fx.mem.f(sp + 0x1BC);
    let key = fixu(fx, w, 0x01E0_1CF4) as i32;
    fx.submit(d, ot, key);
    if fx.mem.u8(g + 4) & 1 == 0 {
        return;
    }
    let k = f(0x3F93_3333);
    let hx = ee::mul(k, f20);
    let hy = ee::mul(f(ASPECT), ee::mul(k, f21));
    fx.mem.write_u8(sp + 0x105, 2);
    tex_rect_strip(fx, d, sp + 0x100, tex);
    let x = hf(fx, sp + 0x1B0);
    fix16(fx, &[sp + 0x170, sp + 0x140], ee::sub(x, hx), 0x01E0_1DA8);
    fix16(fx, &[sp + 0x188, sp + 0x158], ee::add(x, hx), 0x01E0_1DFC);
    let y = hf(fx, sp + 0x1B2);
    fix16(fx, &[sp + 0x15A, sp + 0x142], ee::sub(y, hy), 0x01E0_1E54);
    fix16(fx, &[sp + 0x18A, sp + 0x172], ee::add(y, hy), 0x01E0_1EA8);
    let w = fx.mem.f(sp + 0x1BC);
    let key = fixu(fx, w, 0x01E0_1EBC) as i32;
    fx.submit(d, ot, key);
}

/// PI x `deg` / 180.
fn rad(deg: f32) -> f32 {
    ee::div(ee::mul(f(PI), deg), 180.0)
}

/// The record's 0x1E01F30 [F] (708b5a4dccf9, b125 only): `plates(recs, ot,
/// n) -> 1` steps and draws `n` records (0x44 bytes; a 0x310 frame, the
/// descriptor at sp + 0x280). Per record: countdown; textures +8 / +9;
/// the far radius +0x18 closes by +0x1C to the near radius +0x10; the
/// angle +0x20 += +0x24; the centre +0x38 (from a node with +5); in the
/// xz plane about it a plate between radii +0x10 / +0x14, of half width
/// +0x28 degrees, offset by (far - near) along the angle; the four points
/// projected (all or nothing); the plate in colour +0x30.., alpha +0x33
/// (over the first 32 and last 8 counts), keyed by the mean w; then with
/// texture 2, ALPHA 0x48 and cell +0xD shifted 12 right, 8 wide, grey, the
/// four edges as a vertical and a horizontal band of half width
/// 0x1018E0(+0x2C, w), alpha +0x37 (quartered on odd counts with flag 1).
fn plates(fx: &mut Fx, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x310);
    let mut g = recs;
    for i in 0..n {
        if i != 0 {
            g += 0x44;
        }
        plate(fx, sp, g, ot);
    }
    fx.leave(0x310);
    1
}

fn plate(fx: &mut Fx, sp: u32, g: u32, ot: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    let id = fx.mem.u8(g + 8);
    let tex1 = fx.texture(5, id);
    let id = fx.mem.u8(g + 9);
    let tex2 = fx.texture(5, id);
    let (far, near) = (fx.mem.f(g + 0x18), fx.mem.f(g + 0x10));
    if far != near {
        let v = ee::sub(far, fx.mem.f(g + 0x1C));
        fx.mem.setf(g + 0x18, v);
        if ee_lt(v, fx.mem.f(g + 0x10)) {
            let w = fx.mem.read_u32(g + 0x10);
            fx.mem.write_u32(g + 0x18, w);
        }
    }
    let v = ee::add(fx.mem.f(g + 0x20), fx.mem.f(g + 0x24));
    fx.mem.setf(g + 0x20, v);
    for k in 0..3u32 {
        let w = fx.mem.read_u32(g + 0x38 + 4 * k);
        fx.mem.write_u32(sp + 0x250 + 4 * k, w);
    }
    if fx.mem.read_u8(g + 5) != 0 {
        node_source(fx, g, sp + 0x250);
    }
    camera_vp_into(fx, sp + 0x150);
    translation_into(fx, sp + 0x190, sp + 0x250);
    mul_mem(fx, sp + 0x150, sp + 0x190);
    let a = rad(fx.mem.f(g + 0x20));
    let c = libm_cosf(fx, a);
    let f20 = ee::mul(fx.mem.f(g + 0x10), c);
    let s = fx.libm_sinf(a);
    let f21 = ee::mul(fx.mem.f(g + 0x10), s);
    let c = libm_cosf(fx, a);
    let f22 = ee::mul(fx.mem.f(g + 0x18), c);
    let s = fx.libm_sinf(a);
    let f23 = ee::mul(fx.mem.f(g + 0x18), s);
    let (ox, oz) = (ee::sub(f22, f20), ee::sub(f23, f21));
    let a = rad(ee::sub(fx.mem.f(g + 0x20), fx.mem.f(g + 0x28)));
    let b = rad(ee::add(fx.mem.f(g + 0x20), fx.mem.f(g + 0x28)));
    for (p, ang, r) in [(0x210u32, a, 0x14u32), (0x230, a, 0x10), (0x220, b, 0x14), (0x240, b, 0x10)] {
        let c = libm_cosf(fx, ang);
        let v = ee::add(ox, ee::mul(fx.mem.f(g + r), c));
        fx.mem.setf(sp + p, v);
        fx.mem.write_u32(sp + p + 4, 0);
        let s = fx.libm_sinf(ang);
        let v = ee::add(oz, ee::mul(fx.mem.f(g + r), s));
        fx.mem.setf(sp + p + 8, v);
    }
    let d = sp + 0x280;
    fx.desc_init(d, 0xC, 0x34, STRIP);
    for (src, o) in [(0x32u32, 0u32), (0x31, 1), (0x30, 2)] {
        let v = fx.mem.read_u8(g + src);
        for a in [0x2D8u32, 0x2C0, 0x2A8, 0x290] {
            fx.mem.write_u8(sp + a + o, v);
        }
    }
    let alpha = [sp + 0x2DB, sp + 0x2C3, sp + 0x2AB, sp + 0x293];
    let e = fx.mem.u8(g + 2) - fx.mem.u8(g);
    let top = fx.mem.u8(g + 0x33);
    let mut al = top;
    if e < 0x20 {
        al = fx.mult(top >> 5, e);
    }
    let c = fx.mem.u8(g);
    if c < 8 {
        al = fx.mult(top >> 3, c);
    }
    for &a in &alpha {
        fx.mem.set8(a, al);
    }
    let m = fx.read_m4(sp + 0x150);
    for k in 0..4u32 {
        if !fx.project_at(sp + 0x210 + 0x10 * k, &m, sp + 0x1F0 + 8 * k, sp + 0x260 + 4 * k) {
            return;
        }
    }
    for k in 0..4u32 {
        let v = fx.mem.read_u64(sp + 0x1F0 + 8 * k);
        fx.mem.write_u64(sp + 0x2A0 + 0x18 * k, v);
    }
    for o in [0x2EFu32, 0x2D7, 0x2BF, 0x2A7] {
        fx.mem.write_u8(sp + o, 0);
    }
    let b = fx.mem.read_u8(g + 0xE);
    fx.mem.write_u8(sp + 0x1D5, b);
    let (cell, layout) = (fx.mem.u8(g + 0xC), fx.mem.u8(g + 0xA));
    atlas_cell(fx, sp + 0x1D0, cell, layout);
    tex_rect_strip(fx, d, sp + 0x1D0, tex1);
    let (w0, w1, w2, w3) = (fx.mem.f(sp + 0x260), fx.mem.f(sp + 0x264), fx.mem.f(sp + 0x268), fx.mem.f(sp + 0x26C));
    let s = ee::add(w3, ee::add(w2, ee::add(w0, w1)));
    let key = fixu(fx, ee::div(s, 4.0), 0x01E0_2518) as i32;
    fx.submit(d, ot, key);

    fx.desc_init(d, 0xC, 0x34, STRIP);
    for a in [0x2D8u32, 0x2C0, 0x2A8, 0x290] {
        for o in 0..3 {
            fx.mem.write_u8(sp + a + o, 0x80);
        }
        fx.mem.write_u32(sp + a + 4, 0x3F80_0000);
    }
    fx.desc_alpha(d, 0x48);
    let top = fx.mem.u8(g + 0x37);
    let mut al = top;
    if e < 0x20 {
        al = fx.mult(top >> 5, e);
    }
    let c = fx.mem.u8(g);
    if c < 8 {
        al = fx.mult(top >> 3, c);
    }
    let al = if c & 1 != 0 && fx.mem.u8(g + 4) & 1 != 0 { al >> 2 } else { al };
    for &a in &alpha {
        fx.mem.set8(a, al);
    }
    for o in [0x2EFu32, 0x2BF, 0x2D7, 0x2A7] {
        fx.mem.write_u8(sp + o, 1);
    }
    let b = fx.mem.read_u8(g + 0xF);
    fx.mem.write_u8(sp + 0x1D5, b);
    let (cell, layout) = (fx.mem.u8(g + 0xD), fx.mem.u8(g + 0xB));
    atlas_cell(fx, sp + 0x1D0, cell, layout);
    let u = fx.mem.i32(sp + 0x1D8) + 0xC;
    fx.mem.set32(sp + 0x1D8, u);
    fx.mem.set32(sp + 0x1DC, u + 8);
    tex_rect_strip(fx, d, sp + 0x1D0, tex2);
    for (a, b) in [(0u32, 1u32), (1, 3), (3, 2), (2, 0)] {
        let size = fx.mem.f(g + 0x2C);
        let (wa, wb) = (fx.mem.f(sp + 0x260 + 4 * a), fx.mem.f(sp + 0x260 + 4 * b));
        let ha = fx.view_scale(size, wa);
        fx.mem.setf(sp + 0x270 + 4 * a, ha);
        let hb = fx.view_scale(size, wb);
        fx.mem.setf(sp + 0x270 + 4 * b, hb);
        let (pa, pb) = (sp + 0x1F0 + 8 * a, sp + 0x1F0 + 8 * b);
        // The vertical band.
        let (ya, yb) = (hf(fx, pa + 2), hf(fx, pb + 2));
        fix16(fx, &[sp + 0x2D2], ee::add(ya, ha), 0x01E0_2818);
        fix16(fx, &[sp + 0x2EA], ee::add(yb, hb), 0x01E0_2884);
        fix16(fx, &[sp + 0x2A2], ee::sub(ya, ha), 0x01E0_28D0);
        fix16(fx, &[sp + 0x2BA], ee::sub(yb, hb), 0x01E0_291C);
        let x = fx.mem.read_u16(pa);
        fx.mem.write_u16(sp + 0x2D0, x);
        fx.mem.write_u16(sp + 0x2A0, x);
        let x = fx.mem.read_u16(pb);
        fx.mem.write_u16(sp + 0x2E8, x);
        fx.mem.write_u16(sp + 0x2B8, x);
        let v = fixu(fx, ee::add(wa, wb), 0x01E0_2974);
        let key = fixu(fx, ee::div(cvt_u32(v), 2.0), 0x01E0_29D0) as i32;
        fx.submit(d, ot, key);
        // The horizontal band.
        let (xa, xb) = (hf(fx, pa), hf(fx, pb));
        fix16(fx, &[sp + 0x2D0], ee::add(xa, ha), 0x01E0_2A2C);
        fix16(fx, &[sp + 0x2E8], ee::add(xb, hb), 0x01E0_2A7C);
        fix16(fx, &[sp + 0x2A0], ee::sub(xa, ha), 0x01E0_2AC8);
        fix16(fx, &[sp + 0x2B8], ee::sub(xb, hb), 0x01E0_2B14);
        let y = fx.mem.read_u16(pa + 2);
        fx.mem.write_u16(sp + 0x2D2, y);
        fx.mem.write_u16(sp + 0x2A2, y);
        let y = fx.mem.read_u16(pb + 2);
        fx.mem.write_u16(sp + 0x2EA, y);
        fx.mem.write_u16(sp + 0x2BA, y);
        let v = fixu(fx, ee::add(wa, wb), 0x01E0_2B60);
        let key = fixu(fx, ee::div(cvt_u32(v), 2.0), 0x01E0_2BBC) as i32;
        fx.submit(d, ot, key);
    }
}

/// The record's 0x1E042F0 [F] (db04207405c4, b125 only): `columns(recs,
/// ot, n) -> 1` steps and draws `n` records (0x3C bytes; a 0x1B0 frame,
/// the descriptor at sp + 0x130: quad of RGBAQ, then UV / XYZ2 x 4). Per
/// record: countdown; the texture +8; the spread +0xC += +0x10; every
/// +3 + 1 counts (elapsed & +3 == 0, or the first count) a new random
/// offset +0x30 / +0x38 of (rand >> 4) % spread - spread / 2 over 100 and
/// the base +0x24 from a node with +5; a vertical line from base y + k
/// +0x18 to base y + k +0x1C (k = elapsed & +3) drawn as a billboard of
/// half widths 0x1018E0(+0x14, w) at its ends; colour +0x20.., alpha +0x23
/// faded over the first / last 8 unless flag 0x40; flag 0x80 at Z 0xFFFFFE
/// and key 0, else keyed by the mean w.
fn columns(fx: &mut Fx, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x1B0);
    let mut g = recs;
    for i in 0..n {
        if i != 0 {
            g += 0x3C;
        }
        column(fx, sp, g, ot);
    }
    fx.leave(0x1B0);
    1
}

fn column(fx: &mut Fx, sp: u32, g: u32, ot: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    let id = fx.mem.u8(g + 8);
    let tex = fx.texture(5, id);
    let v = fx.mem.i32(g + 0xC).wrapping_add(fx.mem.i32(g + 0x10));
    fx.mem.set32(g + 0xC, v);
    let (top, c) = (fx.mem.u8(g + 2), fx.mem.u8(g));
    let s4 = fx.mem.u8(g + 3) & (top - c);
    if s4 == 0 || top - 1 == c {
        if fx.mem.read_u8(g + 5) != 0 {
            node_source(fx, g, sp + 0xF0);
            for k in 0..3u32 {
                let w = fx.mem.read_u32(sp + 0xF0 + 4 * k);
                fx.mem.write_u32(g + 0x24 + 4 * k, w);
            }
        }
        let s = fx.mem.i32(g + 0xC);
        let r = fx.rand_from(g, 0x01E0_43EC) >> 4;
        let v = fx.div(r, s).1 - (s >> 1);
        fx.mem.setf(g + 0x30, ee::div(ee::from_i32(v), 100.0));
        fx.mem.write_u32(g + 0x34, 0);
        let s = fx.mem.i32(g + 0xC);
        let r = fx.rand_from(g, 0x01E0_4438) >> 4;
        let v = fx.div(r, s).1 - (s >> 1);
        fx.mem.setf(g + 0x38, ee::div(ee::from_i32(v), 100.0));
    }
    let x = ee::add(fx.mem.f(g + 0x24), fx.mem.f(g + 0x30));
    fx.mem.setf(sp + 0xE0, x);
    fx.mem.setf(sp + 0xD0, x);
    let z = ee::add(fx.mem.f(g + 0x2C), fx.mem.f(g + 0x38));
    fx.mem.setf(sp + 0xE8, z);
    fx.mem.setf(sp + 0xD8, z);
    let k = ee::from_i32(s4);
    let v = ee::add(fx.mem.f(g + 0x28), ee::mul(k, fx.mem.f(g + 0x18)));
    fx.mem.setf(sp + 0xD4, v);
    let v = ee::add(fx.mem.f(g + 0x28), ee::mul(k, fx.mem.f(g + 0x1C)));
    fx.mem.setf(sp + 0xE4, v);
    camera_vp_into(fx, sp + 0x90);
    let d = sp + 0x130;
    fx.desc_init(d, 9, 0x24, 0x4_2424_2421);
    for (src, dst) in [(0x22u32, 0x140u32), (0x21, 0x141), (0x20, 0x142)] {
        let v = fx.mem.read_u8(g + src);
        fx.mem.write_u8(sp + dst, v);
    }
    fx.mem.write_u32(sp + 0x144, 0x3F80_0000);
    let b = fx.mem.read_u8(g + 0xA);
    fx.mem.write_u8(sp + 0x115, b);
    let cell = fx.mem.u8(g + 9);
    atlas_cell(fx, sp + 0x110, cell, 0);
    tex_rect(fx, d, sp + 0x110, tex);
    let a = fx.mem.read_u8(g + 0x23);
    fx.mem.write_u8(sp + 0x143, a);
    if fx.mem.u8(g + 4) & 0x40 == 0 {
        let c = fx.mem.u8(g);
        if c < 8 {
            let t = fx.mem.u8(g + 0x23) >> 3;
            let v = fx.mult(t, c);
            fx.mem.set8(sp + 0x143, v);
        } else {
            let left = fx.mem.u8(g + 3) - s4;
            if left < 8 {
                let t = fx.mem.u8(g + 0x23) >> 3;
                let v = fx.mult(t, left);
                fx.mem.set8(sp + 0x143, v);
            }
        }
    }
    let m = fx.read_m4(sp + 0x90);
    if !fx.project_at(sp + 0xD0, &m, sp + 0x100, sp + 0x1A8) {
        return;
    }
    if !Fx::on_screen(fx.mem.read_u64(sp + 0x100)) {
        return;
    }
    if !fx.project_at(sp + 0xE0, &m, sp + 0x108, sp + 0x1AC) {
        return;
    }
    if !Fx::on_screen(fx.mem.read_u64(sp + 0x108)) {
        return;
    }
    let size = fx.mem.f(g + 0x14);
    let (w0, w1) = (fx.mem.f(sp + 0x1A8), fx.mem.f(sp + 0x1AC));
    let f21 = fx.view_scale(size, w0);
    let size = fx.mem.f(g + 0x14);
    let f22 = fx.view_scale(size, w1);
    let f20 = if fx.mem.u8(g + 4) & 0x80 != 0 {
        z_chain(fx, &[sp + 0x180, sp + 0x170, sp + 0x160, sp + 0x150], 0xFF_FFFE);
        0.0
    } else {
        let z = fx.xyz_z(sp + 0x108);
        z_chain(fx, &[sp + 0x160, sp + 0x150], z);
        let z = fx.xyz_z(sp + 0x100);
        z_chain(fx, &[sp + 0x180, sp + 0x170], z);
        ee::div(ee::add(w0, w1), 2.0)
    };
    let x = hf(fx, sp + 0x108);
    fix16(fx, &[sp + 0x150], ee::sub(x, f22), 0x01E0_4840);
    fix16(fx, &[sp + 0x160], ee::add(x, f22), 0x01E0_4888);
    let x = hf(fx, sp + 0x100);
    fix16(fx, &[sp + 0x170], ee::sub(x, f21), 0x01E0_48D4);
    fix16(fx, &[sp + 0x180], ee::add(x, f21), 0x01E0_491C);
    let y = fx.mem.read_u16(sp + 0x10A);
    fx.mem.write_u16(sp + 0x162, y);
    fx.mem.write_u16(sp + 0x152, y);
    let y = fx.mem.read_u16(sp + 0x102);
    fx.mem.write_u16(sp + 0x182, y);
    fx.mem.write_u16(sp + 0x172, y);
    let key = fixu(fx, f20, 0x01E0_4944) as i32;
    fx.submit(d, ot, key);
}

/// The UV halfwords of `at`: u `u`, v `v`, the top bits kept.
fn uv2(fx: &mut Fx, at: u32, u: u32, v: u32) {
    let h = fx.mem.read_u16(at);
    fx.mem.write_u16(at, (h & 0xC000) | (u & 0x3FFF) as u16);
    let h = fx.mem.read_u16(at + 2);
    fx.mem.write_u16(at + 2, (h & 0xC000) | (v & 0x3FFF) as u16);
}

/// The record's 0x1E049B0 [F] (b7fa3ca874e9, b125 only): `fans(recs, ot,
/// n) -> 1` steps and draws `n` records (0x34 bytes; a 0x230 frame, the
/// descriptor at sp + 0x190). Per record: countdown; radius +0xC += +0x10;
/// angle +0x14 += +0x18; the centre +0x28 (from a node with +5); two
/// points on the circle at the angle and + width +0x1C, projected (sp +
/// 0x150 / 0x158, skipped when the first is left of the second), their
/// y held at 0x9000 from below; the frame as a texture (0x207EF0): a
/// quad from the points up to a line a little above the screen's middle
/// row (+8 + (count & 3) pixels above 0x7000), texels the pixels; colours
/// +0x24.. at the bottom row, +0x20.. at the top, faded over the last 8;
/// blend +9; keyed by the mean w.
fn fans(fx: &mut Fx, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x230);
    let mut g = recs;
    for i in 0..n {
        if i != 0 {
            g += 0x34;
        }
        fan(fx, sp, g, ot);
    }
    fx.leave(0x230);
    1
}

fn fan(fx: &mut Fx, sp: u32, g: u32, ot: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    let v = ee::add(fx.mem.f(g + 0xC), fx.mem.f(g + 0x10));
    fx.mem.setf(g + 0xC, v);
    let v = ee::add(fx.mem.f(g + 0x14), fx.mem.f(g + 0x18));
    fx.mem.setf(g + 0x14, v);
    for k in 0..3u32 {
        let w = fx.mem.read_u32(g + 0x28 + 4 * k);
        fx.mem.write_u32(sp + 0x180 + 4 * k, w);
    }
    if fx.mem.read_u8(g + 5) != 0 {
        node_source(fx, g, sp + 0x180);
    }
    camera_vp_into(fx, sp + 0xB0);
    translation_into(fx, sp + 0xF0, sp + 0x180);
    mul_mem(fx, sp + 0xB0, sp + 0xF0);
    let a = rad(fx.mem.f(g + 0x14));
    let c = libm_cosf(fx, a);
    let v = ee::mul(fx.mem.f(g + 0xC), c);
    fx.mem.setf(sp + 0x160, v);
    fx.mem.write_u32(sp + 0x164, 0);
    let s = fx.libm_sinf(a);
    let v = ee::mul(fx.mem.f(g + 0xC), s);
    fx.mem.setf(sp + 0x168, v);
    let a = rad(ee::add(fx.mem.f(g + 0x14), fx.mem.f(g + 0x1C)));
    let c = libm_cosf(fx, a);
    let v = ee::mul(fx.mem.f(g + 0xC), c);
    fx.mem.setf(sp + 0x170, v);
    fx.mem.write_u32(sp + 0x174, 0);
    let s = fx.libm_sinf(a);
    let v = ee::mul(fx.mem.f(g + 0xC), s);
    fx.mem.setf(sp + 0x178, v);
    let m = fx.read_m4(sp + 0xB0);
    if !fx.project_at(sp + 0x160, &m, sp + 0x150, sp + 0x220) {
        return;
    }
    if !fx.project_at(sp + 0x170, &m, sp + 0x158, sp + 0x224) {
        return;
    }
    if fx.mem.read_u16(sp + 0x150) < fx.mem.read_u16(sp + 0x158) {
        return;
    }
    for o in [0x152u32, 0x15A] {
        let y = hf(fx, sp + o);
        if !ee_lt(y, 36864.0) {
            fx.mem.write_u16(sp + o, 0x9000);
        }
    }
    let d = sp + 0x190;
    fx.desc_init(d, 0xC, 0x34, STRIP);
    // a2 / a3 as 0x204D80 leaves them [run]: its frame pointer, the w.
    fx.frame_desc(d, 1, [sp - 0x10, sp + 0x224]);
    for (a, b, o) in [(0x26u32, 0x22u32, 0u32), (0x25, 0x21, 1), (0x24, 0x20, 2)] {
        let v = fx.mem.read_u8(g + a);
        fx.mem.write_u8(sp + 0x1B8 + o, v);
        fx.mem.write_u8(sp + 0x1A0 + o, v);
        let v = fx.mem.read_u8(g + b);
        fx.mem.write_u8(sp + 0x1E8 + o, v);
        fx.mem.write_u8(sp + 0x1D0 + o, v);
    }
    let c = fx.mem.u8(g);
    let (a0, a1) = if c < 8 {
        let t = fx.mem.u8(g + 0x27) >> 3;
        let a = fx.mult(t, c);
        (a, (fx.mem.u8(g + 0x23) >> 3).wrapping_mul(c))
    } else {
        (fx.mem.u8(g + 0x27), fx.mem.u8(g + 0x23))
    };
    fx.mem.set8(sp + 0x1BB, a0);
    fx.mem.set8(sp + 0x1A3, a0);
    fx.mem.set8(sp + 0x1EB, a1);
    fx.mem.set8(sp + 0x1D3, a1);
    for o in [0x1ECu32, 0x1D4, 0x1BC, 0x1A4] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    for o in [0x1FFu32, 0x1E7, 0x1CF, 0x1B7] {
        fx.mem.write_u8(sp + o, 0);
    }
    let b = fx.mem.read_u8(g + 9);
    fx.mem.write_u8(sp + 0x135, b);
    alpha_mode(fx, sp + 0x130, sp + 0x22C);
    let b = |fx: &mut Fx, k: u32| u64::from(fx.mem.read_u8(sp + 0x22C + k));
    let alpha = b(fx, 0) | (b(fx, 1) << 2) | (b(fx, 2) << 4) | (b(fx, 3) << 6);
    fx.desc_alpha(d, alpha);
    let fl = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, fl | 0x800);
    let f20 = ee::sub(hf(fx, sp + 0x150), 28672.0);
    let f21 = ee::sub(hf(fx, sp + 0x158), 28672.0);
    let u = fixu(fx, f20, 0x01E0_4EB0);
    uv2(fx, sp + 0x1A8, u, 0);
    let u = fixu(fx, f21, 0x01E0_4EEC);
    uv2(fx, sp + 0x1C0, u, 0);
    let y = ee::sub(hf(fx, sp + 0x152), 28672.0);
    let u = fixu(fx, f20, 0x01E0_4F74);
    let v = fixu(fx, y, 0x01E0_4F9C);
    uv2(fx, sp + 0x1D8, u, v);
    let y = ee::sub(hf(fx, sp + 0x15A), 28672.0);
    let u = fixu(fx, f21, 0x01E0_500C);
    let v = fixu(fx, y, 0x01E0_5034);
    uv2(fx, sp + 0x1F0, u, v);
    let row = fx.mem.u8(g + 8) + (fx.mem.u8(g) & 3);
    let z = fx.xyz_z(sp + 0x150);
    z_chain(fx, &[sp + 0x1E0, sp + 0x1B0], z);
    let z = fx.xyz_z(sp + 0x158);
    z_chain(fx, &[sp + 0x1F8, sp + 0x1C8], z);
    let x = fx.mem.read_u16(sp + 0x150);
    fx.mem.write_u16(sp + 0x1E0, x);
    fx.mem.write_u16(sp + 0x1B0, x);
    let x = fx.mem.read_u16(sp + 0x158);
    fx.mem.write_u16(sp + 0x1F8, x);
    fx.mem.write_u16(sp + 0x1C8, x);
    let y = ee::sub(28672.0, ee::from_i32(row << 4));
    fix16(fx, &[sp + 0x1CA, sp + 0x1B2], y, 0x01E0_515C);
    let y = fx.mem.read_u16(sp + 0x152);
    fx.mem.write_u16(sp + 0x1E2, y);
    let y = fx.mem.read_u16(sp + 0x15A);
    fx.mem.write_u16(sp + 0x1FA, y);
    let (w0, w1) = (fx.mem.f(sp + 0x220), fx.mem.f(sp + 0x224));
    let v = fixu(fx, ee::add(w0, w1), 0x01E0_5188);
    let key = fixu(fx, ee::div(cvt_u32(v), 2.0), 0x01E0_51E4) as i32;
    fx.submit(d, ot, key);
}
