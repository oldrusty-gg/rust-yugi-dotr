//! BMODULE b466 (card 269), hand-ported from the record (linked at
//! 0x1E00000, the entry 0x1E00080 in a 0x7D0 frame with D in s2 and the
//! OT in s0, data area a0 = 0x1ED0000, 0xED80 bytes; one attacker post
//! callback 0x1E037A0 with its static D at 0x1E03B00) [F] (helper A32,
//! 3 Nov 2026). A 0.67 relative of b222 (card 563): its parts with the D
//! offsets moved +0x80 (fountains / shards / attacker rings lower), the
//! locals from sp + 0x410 up 0xC0 higher (0x578.. 0xB8, 0x780.. 0x40), no
//! wave sheet, four nodes 0xE (kind 1) / 0x15 / 0x23 / 0x2A whose pair
//! midpoints (D+0xA0 / 0xB0) feed a new 24 x 0x70 mote pool (D+0xE300).
//! Each update: a white flash (D+0xC0 from 0x82); two refracting ring
//! walls at the target (D+0x1BA0, radius 10 at y 0, rings 15 c / 16 on
//! base 0x4000); 16-sprite bursts (D+0x1BF0, 2.5 table vectors, cells 4..7)
//! and 2 x 32 sprite fountains (D+0x30F0) and 2 x 20 spinning shards
//! (D+0x7930) on 0x83..0x1AD; the stage light (80 / 1024); the attacker
//! drawn into its target with the callback and pulled through 16 rings of
//! quads and a fan (D+0xE2F0, the layer below 0x82) and its glow sprite;
//! a frame-textured window (0x82..0x1DE); the motes; returns 1 / 6 / 2 / 8
//! on 0x78 / 0x83..0x1BD / 0x1BE / 0x1DF.
use crate::b182::{alpha, far, head, rgba, uv};
use crate::b440::{copy8, copy_z, div12, fix, fixs, rem2};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_xy, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
const PROJ_OUT: u32 = 0x002D_3600;
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
/// The random unit-vector table the bursts read (3 x 4 x 32 floats).
const VEC_TABLE: u32 = 0x0028_AE90;
const FRAME: u32 = 0x7D0;
const CB: u32 = 0x01E0_37A0;
const D_STATIC: u32 = 0x01E0_3B00;

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

/// The entry 0x1E00080 of b466 (card 269).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xC0, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp) };
    fx.leave(FRAME);
    ret
}

/// rand() >> 23 from `ra` (the caller's s0 = the OT).
fn rnd(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(OT, ra) >> 23
}

fn vec4(fx: &mut Fx, p: u32, x: f32, y: f32, z: f32) {
    fx.mem.setf(p, x);
    fx.mem.setf(p + 4, y);
    fx.mem.setf(p + 8, z);
    fx.mem.write_u32(p + 0xC, 0x3F80_0000);
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0xED80);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let side = fx.unit_side(h);
    fx.mem.set32(D + 0xC, side);
    for (k, id) in [(0x10u32, 0), (0x14, 1), (0x18, 2), (0x1C, 3)] {
        let r = fx.texture(5, id);
        fx.mem.write_u32(D + k, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x30);
    let h = fx.mem.i32(D + 4);
    let r = crate::blib::afterimage::unit_radius(fx, h);
    fx.mem.setf(D + 0x40, ee::mul(1.5, r));
    for (k, (node, kind)) in [(0xE, 1), (0x15, 0), (0x23, 0), (0x2A, 0)].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let s = fx.request_node(h, node, kind);
        fx.mem.set32(D + 0x48 + 4 * k as u32, s);
    }
    fx.mem.write_u8(D + 0xC0, 0);
    fx.mem.write_u8(D + 0x1BA0, 0);
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x1BF0 + 0x150 * k, 0);
    }
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x30F0 + 0x470 * k, 0);
    }
    // The shard texels: eight cells of four S and two T.
    for a2 in 0..8i32 {
        let f2 = ee::from_i32(rem2(a2, 4) << 5);
        for a3 in 0..4i32 {
            let s = ee::div(ee::add(f2, ee::div(ee::mul(32.0, ee::from_i32(a3)), 3.0)), 128.0);
            fx.mem.setf(D + 0x77F0 + 0x10 * a2 as u32 + 4 * a3 as u32, s);
        }
        let f2 = ee::from_i32((a2 >> 2) << 5);
        fx.mem.setf(D + 0x7870 + 8 * a2 as u32, ee::div(f2, 64.0));
        fx.mem.setf(D + 0x7874 + 8 * a2 as u32, ee::div(ee::add(32.0, f2), 64.0));
    }
    // The shard edges: top / bottom pairs a third of a turn apart.
    let mut s0 = 0i32;
    for s3 in 0..4i32 {
        let r = D + 0x20 * s3 as u32;
        fx.mult(0x5555_5556, s0);
        let s4 = fx.hilo().1.wrapping_add(((s0 as u32) >> 31) as i32);
        let sn = fx.tsin(s4);
        fx.mem.setf(r + 0x78B0, ee::mul(25.0, sn));
        fx.mem.write_u32(r + 0x78B4, 0x4140_0000);
        let cs = fx.tcos(s4);
        fx.mem.setf(r + 0x78B8, ee::mul(25.0, cs));
        fx.mem.write_u32(r + 0x78BC, 0x3F80_0000);
        for w in 0..4u32 {
            let x = fx.mem.read_u32(r + 0x78B0 + 4 * w);
            fx.mem.write_u32(r + 0x78C0 + 4 * w, x);
        }
        let f1 = ee::div(ee::mul(-25.0, ee::from_i32(s3)), 3.0);
        let y = ee::mul(fx.mem.f(r + 0x78C4), -1.0);
        fx.mem.setf(r + 0x78C4, y);
        let f1 = ee::add(12.5, f1);
        let y = ee::add(fx.mem.f(r + 0x78B4), f1);
        fx.mem.setf(r + 0x78B4, y);
        let y = ee::add(fx.mem.f(r + 0x78C4), f1);
        fx.mem.setf(r + 0x78C4, y);
        s0 = s0.wrapping_add(0x8000);
    }
    for k in 0..0xCu32 {
        fx.mem.write_u8(D + 0x7930 + 0x350 * k, 0);
    }
    // The attacker rings.
    for s4 in 0..0x10i32 {
        let f20 = ee::div(ee::mul(20.0, ee::from_i32(0x10 - s4)), 16.0);
        for s5 in 0..0x21i32 {
            let a = (s5 << 16) >> 5;
            let p = D + 0x210 * s4 as u32 + 0x10 * s5 as u32 + 0xA0F0;
            let sn = fx.tsin(a);
            fx.mem.setf(p, ee::mul(f20, sn));
            let cs = fx.tcos(a);
            fx.mem.setf(p + 4, ee::mul(f20, cs));
            fx.mem.write_u32(p + 8, 0);
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        }
    }
    fx.mem.write_u8(D + 0xE2F0, 0);
    for k in 0..0x18u32 {
        fx.mem.write_u8(D + 0xE300 + 0x70 * k, 0);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x550);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x20);
    camera(fx, sp);
    fx.mem.write_u32(sp + 0x610, 0);
    fx.mem.write_u32(sp + 0x614, 0);
    let r = fx.mem.f(D + 0x40);
    fx.mem.setf(sp + 0x618, ee::neg(r));
    fx.mem.write_u32(sp + 0x61C, 0x3F80_0000);
    fx.vu_apply_to(sp + 0x610, sp + 0x550, sp + 0x610);
    fx.vu_add4(sp + 0x610, sp + 0x610, D + 0x20);
    let m = fx.read_m4(sp + 0x4D0);
    fx.project_at(sp + 0x610, &m, sp + 0x7C0, sp + 0x7CC);
    let z = (fx.mem.read_u64(sp + 0x7C0) << 8) >> 40;
    fx.mem.write_u32(D + 0x44, z as u32);
    // The four node points and the two pair midpoints.
    for k in 0..4u32 {
        let h = fx.mem.i32(D + 4);
        let s = fx.mem.i32(D + 0x48 + 4 * k);
        fx.node_position(h, s, D + 0x60 + 0x10 * k);
    }
    for k in 0..2u32 {
        fx.vu_add4(sp + 0x610, D + 0x60 + 0x20 * k, D + 0x70 + 0x20 * k);
        fx.vu_scale4(D + 0xA0 + 0x10 * k, sp + 0x610, 0.5);
    }

    flash(fx, sp);
    walls(fx, sp);
    bursts(fx, sp);
    fountains(fx, sp);
    // a3 at the attacker pass's first TEXFLUSH: the last projection's
    // descriptor (0x205210's untouched argument), else 0x208400's d + 2.
    let a3 = if shards(fx, sp) { sp + 0x730 } else { sp + 0x732 };

    let n = fx.mem.i32(D);
    let s1 = Fx::clamp_i(0x1DE - n, 0, 0x20);
    let c = Fx::clamp_i(n, 0, 0x20);
    let l = (fx.mult(s1, c.wrapping_neg().wrapping_mul(80)) >> 10) + 0x80;
    fx.set_stage_light(l & 0xFF);

    attacker_pass(fx, sp, a3);
    window(fx, sp);
    motes(fx, sp);

    let n = fx.mem.i32(D);
    let mut ret = fx.mem.i32(sp + 0xC0);
    if n == 0x78 {
        ret = 1;
    } else {
        if n >= 0x83 && n < 0x1BE {
            ret = 6;
        }
        if n == 0x1BE {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x1DF {
        ret = 8;
    }
    fx.mem.set32(sp + 0xC0, ret);
    ret
}

/// The camera's view-projection at sp + 0x4D0 times the identity at sp +
/// 0x510.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x4D0);
    fx.identity(sp + 0x510);
    crate::blib::exe::mul_mem(fx, sp + 0x4D0, sp + 0x510);
}

/// The camera at the attacker's point under the camera turn (sp + 0x550).
fn camera_at(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x4D0);
    fx.translate_rows(sp + 0x510, sp + 0x550, D + 0x20);
    crate::blib::exe::mul_mem(fx, sp + 0x4D0, sp + 0x510);
}

/// The camera at `t` (0x20C0B0 with a translation only).
fn camera_to(fx: &mut Fx, sp: u32, t: u32) {
    fx.camera_vp_to(sp + 0x4D0);
    let tr = std::array::from_fn(|c| fx.mem.f(t + 4 * c as u32));
    let m = fx.trs(None, None, Some(tr));
    fx.write_m4(sp + 0x510, &m);
    crate::blib::exe::mul_mem(fx, sp + 0x4D0, sp + 0x510);
}

/// A full-window sprite at `d` (the XYZ2s at +0x18 / +0x20).
fn window_xy(fx: &mut Fx, d: u32) {
    far_xy(fx, d + 0x18, 0x7000, 0x7000);
    far_xy(fx, d + 0x20, 0x9000, 0x9000);
}

/// The white flash (D+0xC0) [F 0x1E00614..0x1E00764].
fn flash(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x82 {
        fx.mem.write_u8(D + 0xC0, 1);
        fx.mem.write_u32(D + 0xC4, 0);
    }
    if fx.mem.read_u8(D + 0xC0) != 1 {
        return;
    }
    let d = sp + 0x688;
    head(fx, d, 6, 3, 0x441, 0);
    alpha(fx, d, 1);
    let age = fx.mem.i32(D + 0xC4);
    let t = fx.tan_falloff_p(age, 0x40, 1);
    let a = fix(fx, ee::mul(128.0, ee::sub(1.0, t)), 0x01E0_06A0);
    rgba(fx, sp + 0x698, [0xFF, 0xF8, 0xE8], a as u8);
    window_xy(fx, d);
    fx.submit(d, OT, 0);
    let age = fx.mem.i32(D + 0xC4) + 1;
    fx.mem.set32(D + 0xC4, age);
    if age >= 0x41 {
        fx.mem.write_u8(D + 0xC0, 0);
    }
}

/// The nine-point ring (D+0xD0) of radius `r`.
fn ring9(fx: &mut Fx, r: f32) {
    for s4 in 0..9i32 {
        let a = (s4 << 16) >> 3;
        let p = D + 0xD0 + 0x10 * s4 as u32;
        let sn = fx.tsin(a);
        fx.mem.setf(p, ee::mul(r, sn));
        fx.mem.write_u32(p + 4, 0);
        let cs = fx.tcos(a);
        fx.mem.setf(p + 8, ee::mul(r, cs));
        fx.mem.write_u32(p + 0xC, 0x3F80_0000);
    }
}

fn quad_desc(fx: &mut Fx, q: u32) {
    head(fx, q, 0x34, 0xC, 0x4214_2142_1421, 0);
}

/// The two ring walls at the target (D+0x1BA0) [F 0x1E00768..0x1E01324].
fn walls(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x82 {
        fx.mem.write_u8(D + 0x1BA0, 1);
        fx.mem.write_u32(D + 0x1BA4, 0);
        for s4 in 0..2i32 {
            let r = D + 0x10 * s4 as u32;
            let a = (((s4 << 16) >> 1) + 0x4000) & 0xFFFF;
            let sn = fx.tsin(a);
            fx.mem.setf(r + 0x1BB0, ee::mul(10.0, sn));
            fx.mem.write_u32(r + 0x1BB4, 0);
            let cs = fx.tcos(a);
            fx.mem.setf(r + 0x1BB8, ee::mul(10.0, cs));
            fx.mem.write_u32(r + 0x1BBC, 0x3F80_0000);
            let x = ee::add(fx.mem.f(r + 0x1BB0), fx.mem.f(D + 0x30));
            fx.mem.setf(r + 0x1BB0, x);
            let z = ee::add(fx.mem.f(r + 0x1BB8), fx.mem.f(D + 0x38));
            fx.mem.setf(r + 0x1BB8, z);
        }
    }
    if fx.mem.read_u8(D + 0x1BA0) != 1 {
        return;
    }
    // The rise clamp(age, 0, 0x40) is computed and unused: the walls stand
    // at their spots (0 + 1 (z - 0)).
    for a in 0..2u32 {
        let r = D + 0x10 * a;
        for w in 0..4u32 {
            let x = fx.mem.read_u32(r + 0x1BB0 + 4 * w);
            fx.mem.write_u32(r + 0x1BD0 + 4 * w, x);
        }
        let z = fx.mem.f(r + 0x1BB8);
        fx.mem.setf(r + 0x1BD8, ee::add(0.0, ee::mul(1.0, ee::sub(z, 0.0))));
    }
    let n = fx.mem.i32(D);
    let c = Fx::clamp_i(n, 0, 0x10);
    let f20 = ee::div(ee::mul(15.0, ee::from_i32(c)), 16.0);
    for s7 in 0..2i32 {
        let fp = D + 0x150 * s7 as u32;
        let s4 = 0x6301 * s7;
        for s6 in 0..0x15i32 {
            let s1 = s6 << 15;
            fx.mult(0x6666_6667, s1);
            let q = (fx.hilo().1 >> 3).wrapping_add(((s1 as u32) >> 31) as i32);
            let age = fx.mem.i32(D + 0x1BA4);
            let a = (age.wrapping_mul(0x222).wrapping_add(s4 + q) + 0x4000) & 0xFFFF;
            let p = fp + 0x10 * s6 as u32 + 0x1900;
            let sn = fx.tsin(a);
            fx.mem.setf(p, ee::mul(f20, sn));
            fx.mem.write_u32(p + 4, 0);
            let cs = fx.tcos(a);
            fx.mem.setf(p + 8, ee::mul(f20, cs));
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        }
    }
    let age = fx.mem.i32(D + 0x1BA4);
    let c = Fx::clamp_i(age, 0, 0x10);
    let f24 = ee::mul(130.0, fx.tan_falloff_p(c, 0x10, 1));
    let age = fx.mem.i32(D + 0x1BA4);
    let c = Fx::clamp_i(0x12C - age, 0, 0x20);
    let f22 = ee::div(ee::from_i32(c), 32.0);
    let q = sp + 0x730;
    quad_desc(fx, q);
    alpha(fx, q, 1);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(q, rh, 1);
    fx.desc_clamp(q, 0);
    rgba(fx, sp + 0x740, [0xFF, 0x58, 0x4B], 0x40);
    for o in [0x758u32, 0x770, 0x788] {
        copy8(fx, sp + o, sp + 0x740);
    }
    ring9(fx, ee::mul(10.0, f22));
    // a0..a3 at the first TEXFLUSH: 0x206F30's (desc, ot, key) and the
    // projection's desc after a drawn quad, else 0x205210's (in, out, m,
    // desc) of the rejected one.
    let mut last = [0u32; 4];
    for t in 0..2i32 {
        camera_to(fx, sp, D + 0x10 * t as u32 + 0x1BD0);
        grid(fx, t, f24);
        let age = fx.mem.i32(D + 0x1BA4);
        let f1 = ee::div(ee::mul(2.0, ee::from_i32(rem2(age, 0x10))), 16.0);
        let f21 = ee::sub(ee::div(ee::from_i32(t), 2.0), f1);
        for s7 in 0..0x14i32 {
            let f23 = ee::add(ee::div(ee::mul(5.0, ee::from_i32(s7)), 20.0), f21);
            let f20 = ee::add(ee::div(ee::mul(5.0, ee::from_i32(s7 + 1)), 20.0), f21);
            for s6 in 0..8i32 {
                let f2 = ee::div(ee::mul(2.0, ee::from_i32(s6)), 8.0);
                let f0 = ee::div(ee::mul(2.0, ee::from_i32(s6 + 1)), 8.0);
                for (o, x) in [(0x748u32, f2), (0x74C, f23), (0x760, f0), (0x764, f23), (0x778, f2), (0x77C, f20), (0x790, f0), (0x794, f20)] {
                    fx.mem.setf(sp + o, x);
                }
                let vs = cell(s7, s6, 0x160);
                if fx.project_n(&vs, sp + 0x4D0, q, sp + 0x7CC, 0x01E0_0D50) == 1 {
                    let w = fx.mem.f(sp + 0x7CC);
                    let key = fix(fx, w, 0x01E0_0D64);
                    fx.submit(q, OT, key as i32);
                    last = [q, OT, key, q];
                } else {
                    last = [fx.sp - 0x40, PROJ_OUT, sp + 0x4D0, q];
                }
            }
        }
    }
    quad_desc(fx, q);
    let layer = fx.layer_begin();
    let regs = crate::lens::frame_copy(fx, last, 1, sp + 0x7A8, sp + 0x7B0);
    fx.texflush(regs);
    alpha(fx, q, 0);
    fx.desc_clamp(q, LENS_CLAMP);
    let fl = fx.mem.read_u16(q + 2);
    fx.mem.write_u16(q + 2, fl | 0x800);
    rgba(fx, sp + 0x740, [0x8A, 0x82, 0x80], 0x80);
    for o in [0x758u32, 0x770, 0x788] {
        copy8(fx, sp + o, sp + 0x740);
    }
    ring9(fx, ee::mul(9.5, f22));
    for t in 0..2i32 {
        camera_to(fx, sp, D + 0x10 * t as u32 + 0x1BD0);
        for s7 in 0..0x15i32 {
            let f20 = ee::div(ee::mul(f24, ee::from_i32(s7)), 20.0);
            fx.identity(sp + 0x5D0);
            let ph = phase(fx, t);
            let fp = s7 << 16;
            fx.mult(0x6666_6667, fp);
            let q20 = (fx.hilo().1 >> 3).wrapping_add(((fp as u32) >> 31) as i32);
            let sn = fx.tsin((q20 + ph) & 0xFFFF);
            let rot = fixs(fx, ee::mul(910.0, sn), 0x01E0_1048);
            fx.rotate_axis(sp + 0x5D0, rot, 1);
            let src = D + 0x150 * t as u32 + 0x10 * s7 as u32 + 0x1900;
            for s6 in 0..9i32 {
                let g = D + 0x90 * s7 as u32 + 0x10 * s6 as u32;
                let rp = D + 0xD0 + 0x10 * s6 as u32;
                for w in 0..4u32 {
                    let x = fx.mem.read_u32(rp + 4 * w);
                    fx.mem.write_u32(g + 0x160 + 4 * w, x);
                }
                fx.mem.setf(g + 0x164, f20);
                fx.vu_add4(g + 0x160, g + 0x160, src);
                fx.vu_apply_to(g + 0xD30, sp + 0x5D0, rp);
                let ph = phase(fx, t);
                let sn = fx.tsin((((s6 << 16) >> 3) + ph) & 0xFFFF);
                let f2 = ee::div(f24, 20.0);
                let f0 = ee::mul(ee::mul(f(0x3E4C_CCCD), f2), sn);
                fx.mem.setf(g + 0xD34, ee::add(f20, f0));
                fx.vu_add4(g + 0xD30, g + 0xD30, src);
            }
        }
        for s7 in 0..0x14i32 {
            for s6 in 0..8i32 {
                let vs = cell(s7, s6, 0xD30);
                if fx.project_n(&vs, sp + 0x4D0, q, sp + 0x7CC, 0x01E0_11D0) != 1 {
                    continue;
                }
                for (u, x) in [(0x748u32, 0x750u32), (0x760, 0x768), (0x778, 0x780), (0x790, 0x798)] {
                    fx.uv_from_xy(sp + u, sp + x, 0, 0);
                }
                let vs = cell(s7, s6, 0x160);
                if fx.project_n(&vs, sp + 0x4D0, q, sp + 0x7CC, 0x01E0_124C) == 1 && crate::b222::winding(fx, sp + 0x750, sp + 0x768, sp + 0x780) == 1 {
                    fx.submit(q, 0, 0);
                }
            }
        }
    }
    fx.layer_end(layer, OT, 1);
    let age = fx.mem.i32(D + 0x1BA4) + 1;
    fx.mem.set32(D + 0x1BA4, age);
    if age >= 0x12D {
        fx.mem.write_u8(D + 0x1BA0, 0);
    }
}

/// (rem(age + 5 t, 64) << 16) >> 6: the wall's sway phase.
fn phase(fx: &mut Fx, t: i32) -> i32 {
    let age = fx.mem.i32(D + 0x1BA4);
    (rem2(age + 5 * t, 0x40) << 16) >> 6
}

/// The wall grid (D+0x160, 21 rows of 9) of side `t`: the ring raised by
/// f24 s6 / 20 about the ring offsets.
fn grid(fx: &mut Fx, t: i32, f24: f32) {
    for s6 in 0..0x15i32 {
        let f20 = ee::div(ee::mul(f24, ee::from_i32(s6)), 20.0);
        let src = D + 0x150 * t as u32 + 0x10 * s6 as u32 + 0x1900;
        for s1 in 0..9i32 {
            let g = D + 0x90 * s6 as u32 + 0x10 * s1 as u32 + 0x160;
            for w in 0..4u32 {
                let x = fx.mem.read_u32(D + 0xD0 + 0x10 * s1 as u32 + 4 * w);
                fx.mem.write_u32(g + 4 * w, x);
            }
            fx.mem.setf(g + 4, f20);
            fx.vu_add4(g, g, src);
        }
    }
}

/// The four grid points of cell (row, col) of the grid at D + `base`.
fn cell(row: i32, col: i32, base: u32) -> [u32; 4] {
    let g = |r: i32, c: i32| D + 0x90 * r as u32 + 0x10 * c as u32 + base;
    [g(row, col), g(row, col + 1), g(row + 1, col), g(row + 1, col + 1)]
}

fn sprite_desc(fx: &mut Fx, d: u32, preset: i32, tex: u32, clamp: u64) {
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    alpha(fx, d, preset);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, clamp);
}

/// A sprite's corners at sp + 0x590 / 0x5A0: (-s, -s) and (s, s) turned by
/// the camera turn.
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    vec4(fx, sp + 0x590, ee::neg(s), ee::neg(s), 0.0);
    vec4(fx, sp + 0x5A0, s, s, 0.0);
    fx.mem.write_u32(sp + 0x598, 0);
    fx.mem.write_u32(sp + 0x5A8, 0);
    fx.vu_apply_to(sp + 0x590, sp + 0x550, sp + 0x590);
    fx.vu_apply_to(sp + 0x5A0, sp + 0x550, sp + 0x5A0);
}

/// Cell `c` of a 4 x 2 sprite atlas into the sprite's UVs.
fn atlas(fx: &mut Fx, sp: u32, c: i32) {
    let u = rem2(c, 4);
    let w = c >> 2;
    uv(fx, sp + 0x648, ((u << 9) & 0x3FFF) as u32);
    uv(fx, sp + 0x64A, ((w << 9) & 0x3FFF) as u32);
    uv(fx, sp + 0x658, ((((u << 5) + 0x20) << 4) & 0x3FFF) as u32);
    uv(fx, sp + 0x65A, ((((w << 5) + 0x20) << 4) & 0x3FFF) as u32);
}

/// The sprite at `p` (its corners about it at sp + 0x5B0 / 0x5C0).
fn place(fx: &mut Fx, sp: u32, p: u32) {
    fx.vu_add4(sp + 0x5B0, sp + 0x590, p);
    fx.vu_add4(sp + 0x5C0, sp + 0x5A0, p);
}

/// The bursts (D+0x1BF0, 16 x 0x150): 16 sprites flung from the target
/// along random table vectors [F 0x1E01328..0x1E0186C].
fn bursts(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n >= 0x83 && n < 0x1AE {
        let r = D + 0x150 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(r + 0x1BF0, 1);
        fx.mem.write_u32(r + 0x1BF4, 1);
        const RAS: [[u32; 3]; 3] = [
            [0x01E0_13B0, 0x01E0_13E4, 0x01E0_1410],
            [0x01E0_1440, 0x01E0_1474, 0x01E0_14A0],
            [0x01E0_14D0, 0x01E0_1504, 0x01E0_1530],
        ];
        for s4 in 0..0x10u32 {
            let c = rem2(rnd(fx, 0x01E0_138C), 4) + 4;
            fx.mem.set32(r + 4 * s4 + 0x1BF8, c);
            for (k, ras) in RAS.iter().enumerate() {
                let a = rem2(rnd(fx, ras[0]), 4);
                let t = VEC_TABLE + 0x180 * a as u32;
                let b = rem2(rnd(fx, ras[1]), 0x20);
                let t = t.wrapping_add(12 * b as u32);
                let x = rnd(fx, ras[2]);
                let (_, m) = fx.div(x, 3);
                let e = fx.mem.f(t.wrapping_add(4 * m as u32));
                fx.mem.setf(r + 0x10 * s4 + 0x1C40 + 4 * k as u32, ee::mul(2.5, e));
            }
            fx.mem.write_u32(r + 0x10 * s4 + 0x1C4C, 0x3F80_0000);
        }
    }
    let d = sp + 0x630;
    sprite_desc(fx, d, 1, 0x1C, 5);
    camera(fx, sp);
    for k in 0..0x10u32 {
        let r = D + 0x150 * k;
        if fx.mem.read_u8(r + 0x1BF0) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 0x1BF4);
        let a = ((0x10 - age) << 6) >> 4;
        rgba(fx, sp + 0x640, [0xFF, 0x8A, 0x4B], a as u8);
        let age = ee::from_i32(fx.mem.i32(r + 0x1BF4));
        let s = ee::mul(18.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 16.0)));
        corners(fx, sp, s);
        let age = fx.mem.i32(r + 0x1BF4);
        let f20 = fx.tan_falloff_p(age, 0x10, 1);
        for s6 in 0..0x10u32 {
            fx.vu_scale4(sp + 0x610, r + 0x10 * s6 + 0x1C40, f20);
            fx.vu_add4(sp + 0x610, sp + 0x610, D + 0x30);
            place(fx, sp, sp + 0x610);
            let c = fx.mem.i32(r + 4 * s6 + 0x1BF8);
            atlas(fx, sp, c);
            if crate::fxlib2::project(fx, sp + 0x5B0, PROJ_OUT, sp + 0x4D0, d, sp + 0x7CC) {
                let w = fx.mem.f(sp + 0x7CC);
                let key = fix(fx, w, 0x01E0_1818);
                fx.submit(d, OT, key as i32);
            }
        }
        let age = fx.mem.i32(r + 0x1BF4) + 1;
        fx.mem.set32(r + 0x1BF4, age);
        if age >= 0x11 {
            fx.mem.write_u8(r + 0x1BF0, 0);
        }
    }
}

/// The fountains (D+0x30F0, 16 x 0x470): 2 x 32 sprites rising from the
/// walls' second ring points [F 0x1E01870..0x1E01D34].
fn fountains(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n >= 0x83 && n < 0x1AE {
        let r = D + 0x470 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(r + 0x30F0, 1);
        fx.mem.write_u32(r + 0x30F4, 1);
        for s7 in 0..2u32 {
            fx.vu_add4(r + 0x10 * s7 + 0x3140, D + 0x150 * s7 + 0x1910, D + 0x10 * s7 + 0x1BD0);
            for s6 in 0..0x20u32 {
                let c = rem2(rnd(fx, 0x01E0_1908), 8);
                fx.mem.write_u8(r + 0x20 * s7 + s6 + 0x30F8, c as u8);
                let p = r + 0x200 * s7 + 0x10 * s6 + 0x3160;
                fx.random_table_vec(p, 2.0);
                // fabsf's frame: the argument at its sp.
                let y = fx.mem.f(p + 4);
                fx.mem.setf(sp - 0x10, y);
                let y = f32::from_bits(y.to_bits() & 0x7FFF_FFFF);
                fx.mem.setf(p + 4, ee::mul(f(0x3F4C_CCCD), y));
            }
        }
    }
    let d = sp + 0x630;
    sprite_desc(fx, d, 1, 0x1C, 5);
    camera(fx, sp);
    for k in 0..0x10u32 {
        let r = D + 0x470 * k;
        if fx.mem.read_u8(r + 0x30F0) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 0x30F4);
        let a = ((0x10 - age) * 36) >> 4;
        rgba(fx, sp + 0x640, [0xFF, 0x8A, 0x4B], a as u8);
        let agef = ee::from_i32(fx.mem.i32(r + 0x30F4));
        let s = ee::mul(10.0, ee::add(0.5, ee::div(ee::mul(0.5, agef), 16.0)));
        corners(fx, sp, s);
        let age = fx.mem.i32(r + 0x30F4);
        let t = fx.tan_falloff_p(age, 0x10, 1);
        let f20 = ee::add(f(0x3E4C_CCCD), ee::mul(f(0x3F4C_CCCD), t));
        for s7 in 0..2u32 {
            for s6 in 0..0x20u32 {
                fx.vu_scale4(sp + 0x610, r + 0x200 * s7 + 0x10 * s6 + 0x3160, f20);
                fx.vu_add4(sp + 0x610, sp + 0x610, r + 0x10 * s7 + 0x3140);
                place(fx, sp, sp + 0x610);
                let c = i32::from(fx.mem.read_u8(r + 0x20 * s7 + s6 + 0x30F8));
                atlas(fx, sp, c);
                if crate::fxlib2::project(fx, sp + 0x5B0, PROJ_OUT, sp + 0x4D0, d, sp + 0x7CC) {
                    fx.submit(d, OT, 0);
                }
            }
        }
        let age = fx.mem.i32(r + 0x30F4) + 1;
        fx.mem.set32(r + 0x30F4, age);
        if age >= 0x11 {
            fx.mem.write_u8(r + 0x30F0, 0);
        }
    }
}

/// The shards (D+0x7930, 12 x 0x350): 2 x 20 spinning three-panel shards
/// rising along the walls' ring offsets [F 0x1E01D38..0x1E0228C].
fn shards(fx: &mut Fx, sp: u32) -> bool {
    let n = fx.mem.i32(D);
    if n >= 0x83 && n < 0x1AE {
        let (_, k) = fx.div(n, 0xC);
        let r = D.wrapping_add(fx.mult(k, 0x350) as u32);
        fx.mem.write_u8(r + 0x7930, 1);
        fx.mem.write_u32(r + 0x7934, 0);
        for s7 in 0..2u32 {
            for s6 in 0..0x14u32 {
                let x = rnd(fx, 0x01E0_1DA0);
                let (_, m) = fx.div(x, 3);
                fx.mem.write_u8(r + 0x14 * s7 + s6 + 0x7938, (m + 5) as u8);
                let c = rem2(rnd(fx, 0x01E0_1DC4), 0x100);
                fx.mem.set32(r + 0x50 * s7 + 4 * s6 + 0x7960, (c << 16) >> 8);
                let p = r + 0x140 * s7 + 0x10 * s6 + 0x7A00;
                fx.random_table_vec(p, 0.5);
                let y = ee::mul(fx.mem.f(p + 4), 2.0);
                fx.mem.setf(p + 4, y);
            }
        }
    }
    let q = sp + 0x730;
    quad_desc(fx, q);
    alpha(fx, q, 1);
    let rh = fx.mem.read_u32(D + 0x1C);
    fx.desc_texture(q, rh, 1);
    fx.desc_clamp(q, 5);
    let mut any = false;
    for k in 0..0xCu32 {
        let r = D + 0x350 * k;
        if fx.mem.read_u8(r + 0x7930) == 0 {
            continue;
        }
        any = true;
        let age = fx.mem.i32(r + 0x7934);
        let a = div12(fx, (0xC - age).wrapping_mul(96));
        rgba(fx, sp + 0x740, [0xFF, 0x8A, 0x4B], a as u8);
        for o in [0x758u32, 0x770, 0x788] {
            copy8(fx, sp + o, sp + 0x740);
        }
        let agef = ee::from_i32(fx.mem.i32(r + 0x7934));
        let s = ee::add(f(0x3DCC_CCCD), ee::div(ee::mul(f(0x3F66_6666), agef), 12.0));
        vec4(fx, sp + 0x620, s, s, s);
        for t in 0..2u32 {
            for s6 in 0..0x14u32 {
                fx.vu_add4(sp + 0x610, D + 0x150 * t + 0x10 * s6 + 0x1900, D + 0x10 * t + 0x1BD0);
                fx.vu_add4(sp + 0x610, sp + 0x610, r + 0x140 * t + 0x10 * s6 + 0x7A00);
                let y = ee::add(fx.mem.f(sp + 0x614), ee::div(ee::mul(130.0, ee::from_i32(s6 as i32)), 20.0));
                fx.mem.setf(sp + 0x614, y);
                fx.camera_vp_to(sp + 0x4D0);
                fx.identity(sp + 0x510);
                fx.translate_rows(sp + 0x510, sp + 0x510, sp + 0x610);
                let age = fx.mem.i32(r + 0x7934);
                let spin = fx.mult(age, 0x3E7);
                let a = fx.mem.i32(r + 0x50 * t + 4 * s6 + 0x7960).wrapping_add(spin) & 0xFFFF;
                fx.rotate_axis(sp + 0x510, a, 1);
                fx.scale_rows(sp + 0x510, sp + 0x620);
                crate::blib::exe::mul_mem(fx, sp + 0x4D0, sp + 0x510);
                let b = u32::from(fx.mem.read_u8(r + 0x14 * t + s6 + 0x7938));
                for s1 in 0..3u32 {
                    let (s3, s5) = (D + 0x10 * b + 4 * s1, D + 8 * b);
                    for (o, src) in [(0x748u32, s3 + 0x77F0), (0x74C, s5 + 0x7870), (0x760, s3 + 0x77F4), (0x764, s5 + 0x7870), (0x778, s3 + 0x77F0), (0x77C, s5 + 0x7874), (0x790, s3 + 0x77F4), (0x794, s5 + 0x7874)] {
                        let x = fx.mem.read_u32(src);
                        fx.mem.write_u32(sp + o, x);
                    }
                    let (e0, e1) = (D + 0x20 * s1, D + 0x20 * (s1 + 1));
                    let vs = [e0 + 0x78B0, e1 + 0x78B0, e0 + 0x78C0, e1 + 0x78C0];
                    if fx.project_n(&vs, sp + 0x4D0, q, sp + 0x7CC, 0x01E0_216C) == 1 {
                        let w = fx.mem.f(sp + 0x7CC);
                        let key = fix(fx, w, 0x01E0_2180);
                        fx.submit(q, OT, key as i32);
                    }
                }
            }
        }
        let age = fx.mem.i32(r + 0x7934) + 1;
        fx.mem.set32(r + 0x7934, age);
        if age >= 0xD {
            fx.mem.write_u8(r + 0x7930, 0);
        }
    }
    any
}

/// 120 k / 17 (HI of 0x78787879 x, >> 3, plus the sign).
fn div17(fx: &mut Fx, x: i32) -> i32 {
    fx.mult(0x7878_7879, x);
    (fx.hilo().1 >> 3).wrapping_add(((x as u32) >> 31) as i32)
}

/// The Z of `z` into the XYZ2 at `p`.
fn set_z(fx: &mut Fx, p: u32, z: u32) {
    let w = fx.mem.read_u64(p);
    fx.mem.write_u64(p, (w & KEEP) | (u64::from(z & 0xFF_FFFF) << 32));
}

/// clamp(age, 0, 0x60) clamp(0xDA - age, 0, 0x60) / 9216.
fn pass_level(fx: &mut Fx) -> f32 {
    let age = fx.mem.i32(D + 0xE2F4);
    let c = ee::from_i32(Fx::clamp_i(age, 0, 0x60));
    let age = fx.mem.i32(D + 0xE2F4);
    let c2 = Fx::clamp_i(0xDA - age, 0, 0x60);
    ee::div(ee::mul(c, ee::from_i32(c2)), 9216.0)
}

const RCOL: [u8; 3] = [0xFF, 0x40, 0x0C];

/// The attacker drawn into its target with the callback and pulled
/// through 16 rings of quads and a fan below update 0x82, then its glow
/// sprite (D+0xE2F0) [F 0x1E022E0..0x1E02F68].
fn attacker_pass(fx: &mut Fx, sp: u32, a3: u32) {
    if fx.mem.i32(D) == 0 {
        fx.mem.write_u8(D + 0xE2F0, 1);
        fx.mem.write_u32(D + 0xE2F4, 0);
    }
    if fx.mem.read_u8(D + 0xE2F0) != 1 {
        return;
    }
    if fx.mem.i32(D) < 0x82 {
        pass_layer(fx, sp, a3);
    }
    let d = sp + 0x630;
    sprite_desc(fx, d, 1, 0x10, 0);
    camera_at(fx, sp);
    let f3 = pass_level(fx);
    let age = fx.mem.i32(D + 0xE2F4);
    let s = ee::add(f(0x3F4C_CCCD), ee::mul(f(0x3E4C_CCCD), ee::from_i32(rem2(age, 2))));
    let a = fix(fx, ee::mul(ee::mul(255.0, f3), s), 0x01E0_2DD4);
    rgba(fx, sp + 0x640, RCOL, a as u8);
    uv(fx, sp + 0x648, 0x600);
    uv(fx, sp + 0x64A, 0);
    uv(fx, sp + 0x658, 0x800);
    uv(fx, sp + 0x65A, 0x200);
    vec4(fx, sp + 0x590, -40.0, -40.0, 0.0);
    fx.mem.write_u32(sp + 0x598, 0);
    vec4(fx, sp + 0x5A0, 40.0, 40.0, 0.0);
    fx.mem.write_u32(sp + 0x5A8, 0);
    if crate::fxlib2::project(fx, sp + 0x590, PROJ_OUT, sp + 0x4D0, d, sp + 0x7CC) {
        far(fx, sp + 0x650, &[sp + 0x660]);
        fx.submit(d, OT, 0);
    }
    let age = fx.mem.i32(D + 0xE2F4) + 1;
    fx.mem.set32(D + 0xE2F4, age);
    if age >= 0xDB {
        fx.mem.write_u8(D + 0xE2F0, 0);
    }
}

fn pass_layer(fx: &mut Fx, sp: u32, a3: u32) {
    let t = sp + 0x6B8;
    head(fx, t, 0x33, 9, 0x4_2142_1421, 0);
    let q = sp + 0x730;
    quad_desc(fx, q);
    let layer = fx.layer_begin();
    fx.texflush([0x2142_1421, t, 0x20, a3]);
    fx.mem.write_u32(D_STATIC, D);
    let (_, regs) = fx.unit_draw_post(1, sp + 0x718, sp + 0x720, CB, &mut |fx, p| callback(fx, p));
    copy8(fx, sp + 0x7A8, sp + 0x718);
    copy8(fx, sp + 0x7B0, sp + 0x720);
    fx.texflush([sp + 0x7A8, regs[1], regs[2], regs[3]]);
    camera_at(fx, sp);
    alpha(fx, t, 1);
    fx.desc_clamp(t, LENS_CLAMP);
    let fl = fx.mem.read_u16(t + 2);
    fx.mem.write_u16(t + 2, fl | 0x800);
    alpha(fx, q, 1);
    fx.desc_clamp(q, LENS_CLAMP);
    let fl = fx.mem.read_u16(q + 2);
    fx.mem.write_u16(q + 2, fl | 0x800);
    for k in 0..0x10i32 {
        let rb = D + 0x210 * k as u32;
        let f21 = ee::div(ee::mul(10.0, ee::from_i32(0x10 - k)), 16.0);
        let ak = k.wrapping_mul(0x30000) >> 4;
        let bk = k.wrapping_mul(0x20000) >> 4;
        for s1 in 0..0x21i32 {
            let s4 = (s1 << 16) >> 5;
            let age = fx.mem.i32(D + 0xE2F4);
            let m = fx.mult(age, 0x115C);
            let a = m.wrapping_add(ak + s4) & 0xFFFF;
            let f20 = ee::add(1.0, ee::mul(0.5, fx.tsin(a)));
            let age = fx.mem.i32(D + 0xE2F4);
            let m = fx.mult(age, 0xD05);
            let b = bk.wrapping_add(m) & 0xFFFF;
            let f0 = ee::mul(8192.0, fx.tsin(b));
            let ang = fixs(fx, ee::add(ee::from_i32(s4), f0), 0x01E0_2574) & 0xFFFF;
            let f20 = ee::mul(f21, f20);
            let p = rb + 0x10 * s1 as u32 + 0xC1F0;
            let sn = fx.tsin(ang);
            fx.mem.setf(p, ee::mul(f20, sn));
            let cs = fx.tcos(ang);
            fx.mem.setf(p + 4, ee::mul(f20, cs));
            fx.mem.write_u32(p + 8, 0);
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        }
    }
    for k in 0..3u32 {
        fx.mem.write_u32(sp + 0x610 + 4 * k, 0);
    }
    fx.mem.write_u32(sp + 0x61C, 0x3F80_0000);
    let f20 = pass_level(fx);
    for k in 0..0x10i32 {
        let rb = D + 0x210 * k as u32;
        if k == 0xF {
            let x = div17(fx, 120 * k);
            let a = fix(fx, ee::mul(f20, ee::from_i32(x)), 0x01E0_26F8);
            rgba(fx, sp + 0x6C8, RCOL, a as u8);
            let x = div17(fx, 120 * (k - 1));
            let a = fix(fx, ee::mul(f20, ee::from_i32(x)), 0x01E0_2768);
            rgba(fx, sp + 0x6E0, RCOL, a as u8);
            copy8(fx, sp + 0x6F8, sp + 0x6E0);
            for s3 in 0..0x20u32 {
                let (p0, p1) = (rb + 0x10 * s3, rb + 0x10 * (s3 + 1));
                if fx.project_n(&[sp + 0x610, p0 + 0xC1F0, p1 + 0xC1F0], sp + 0x4D0, t, sp + 0x7CC, 0x01E0_27E4) != 1 {
                    continue;
                }
                for (u, x) in [(0x6D0u32, 0x6D8u32), (0x6E8, 0x6F0), (0x700, 0x708)] {
                    fx.uv_from_xy(sp + u, sp + x, 0, 0);
                }
                if fx.project_n(&[sp + 0x610, p0 + 0xA0F0, p1 + 0xA0F0], sp + 0x4D0, t, sp + 0x7CC, 0x01E0_2850) == 1 {
                    let z = fx.mem.read_u32(D + 0x44);
                    set_z(fx, sp + 0x6D8, z);
                    copy_z(fx, sp + 0x6F0, sp + 0x6D8);
                    copy_z(fx, sp + 0x708, sp + 0x6D8);
                    fx.submit(t, 0, 0);
                }
            }
        } else {
            let x = div17(fx, 120 * k);
            let a = fix(fx, ee::mul(f20, ee::from_i32(x)), 0x01E0_2960);
            rgba(fx, sp + 0x740, RCOL, a as u8);
            copy8(fx, sp + 0x758, sp + 0x740);
            let x = div17(fx, 120 * (k + 1));
            let a = fix(fx, ee::mul(f20, ee::from_i32(x)), 0x01E0_29E0);
            rgba(fx, sp + 0x770, RCOL, a as u8);
            copy8(fx, sp + 0x788, sp + 0x770);
            let rn = rb + 0x210;
            for s4 in 0..0x20u32 {
                let c = |base: u32| [rb + 0x10 * s4 + base, rb + 0x10 * (s4 + 1) + base, rn + 0x10 * s4 + base, rn + 0x10 * (s4 + 1) + base];
                if fx.project_n(&c(0xC1F0), sp + 0x4D0, q, sp + 0x7CC, 0x01E0_2A98) != 1 {
                    continue;
                }
                for (u, x) in [(0x748u32, 0x750u32), (0x760, 0x768), (0x778, 0x780), (0x790, 0x798)] {
                    fx.uv_from_xy(sp + u, sp + x, 0, 0);
                }
                if fx.project_n(&c(0xA0F0), sp + 0x4D0, q, sp + 0x7CC, 0x01E0_2B28) == 1 {
                    let z = fx.mem.read_u32(D + 0x44);
                    set_z(fx, sp + 0x750, z);
                    for o in [0x768u32, 0x780, 0x798] {
                        copy_z(fx, sp + o, sp + 0x750);
                    }
                    fx.submit(q, 0, 0);
                }
            }
        }
    }
    fx.layer_end(layer, OT, 0);
}

/// The frame-textured window sprite while clamp(n - 0x82, 0, 0x10)
/// clamp(0x1DE - n, 0, 0x10) / 256 > 0 [F 0x1E02F6C..0x1E0315C].
fn window(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    let a = ee::from_i32(Fx::clamp_i(n - 0x82, 0, 0x10));
    let n = fx.mem.i32(D);
    let b = ee::from_i32(Fx::clamp_i(0x1DE - n, 0, 0x10));
    let f20 = ee::div(ee::mul(a, b), 256.0);
    if crate::exe::ee_le(f20, 0.0) {
        return;
    }
    let d = sp + 0x630;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    let flags = u32::from(fx.mem.read_u16(d + 2));
    alpha(fx, d, 0);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.desc_clamp(d, 5);
    uv(fx, sp + 0x648, 0x140);
    uv(fx, sp + 0x64A, 0x140);
    uv(fx, sp + 0x658, 0x1EC0);
    uv(fx, sp + 0x65A, 0x1EC0);
    let a = fix(fx, ee::mul(64.0, f20), 0x01E0_30B8);
    rgba(fx, sp + 0x640, [0xBC, 0x8A, 0x62], a as u8);
    far_xy(fx, sp + 0x650, 0x7000, 0x7000);
    far_xy(fx, sp + 0x660, 0x9000, 0x9000);
    fx.submit(d, OT, 0);
}

/// The motes (D+0xE300, 24 x 0x70): on 0x31..0xD9 one record of 2 x 2
/// sprites drifting up from the two node-pair midpoints along 0.5 table
/// vectors (2 x 2 atlas of 64, 24 updates) [F 0x1E03160..0x1E036D4].
fn motes(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n >= 0x31 && n < 0xDA {
        let (_, k) = fx.div(n, 0x18);
        let r = D + 0x70 * k as u32;
        fx.mem.write_u8(r + 0xE300, 1);
        fx.mem.write_u32(r + 0xE304, 0);
        for j in 0..2u32 {
            for w in 0..4u32 {
                let x = fx.mem.read_u32(D + 0xA0 + 0x10 * j + 4 * w);
                fx.mem.write_u32(r + 0x10 * j + 0xE310 + 4 * w, x);
            }
            for i in 0..2u32 {
                let c = rem2(rnd(fx, 0x01E0_3208), 4);
                fx.mem.write_u8(r + 2 * j + i + 0xE308, c as u8);
                fx.random_table_vec(r + 0x20 * j + 0x10 * i + 0xE330, 0.5);
            }
        }
    }
    let d = sp + 0x630;
    sprite_desc(fx, d, 1, 0x14, 5);
    camera(fx, sp);
    for k in 0..0x18u32 {
        let r = D + 0x70 * k;
        if fx.mem.read_u8(r + 0xE300) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 0xE304);
        let x = age << 15;
        fx.mult(0x2AAA_AAAB, x);
        let a = (fx.hilo().1 >> 2).wrapping_add(((x as u32) >> 31) as i32);
        let sn = fx.tsin(a);
        let a = fix(fx, ee::mul(16.0, sn), 0x01E0_338C);
        rgba(fx, sp + 0x640, [0xFF, 0x64, 0x50], a as u8);
        let agef = ee::from_i32(fx.mem.i32(r + 0xE304));
        let s = ee::mul(8.0, ee::add(0.5, ee::div(ee::mul(0.5, agef), 24.0)));
        corners(fx, sp, s);
        let age = fx.mem.i32(r + 0xE304);
        let t = fx.tan_falloff_p(age, 0x18, 1);
        let f20 = ee::add(f(0x3E4C_CCCD), ee::mul(f(0x3F4C_CCCD), t));
        let agef = ee::from_i32(fx.mem.i32(r + 0xE304));
        let f21 = ee::add(-3.0, ee::div(ee::mul(10.0, agef), 24.0));
        for j in 0..2u32 {
            for i in 0..2u32 {
                fx.vu_scale4(sp + 0x610, r + 0x20 * j + 0x10 * i + 0xE330, f20);
                fx.vu_add4(sp + 0x610, sp + 0x610, r + 0x10 * j + 0xE310);
                let y = ee::add(fx.mem.f(sp + 0x614), f21);
                fx.mem.setf(sp + 0x614, y);
                place(fx, sp, sp + 0x610);
                let c = i32::from(fx.mem.read_u8(r + 2 * j + i + 0xE308));
                let u = rem2(c, 2);
                let w = c >> 1;
                uv(fx, sp + 0x648, ((u << 10) & 0x3FFF) as u32);
                uv(fx, sp + 0x64A, ((w << 10) & 0x3FFF) as u32);
                uv(fx, sp + 0x658, ((((u << 6) + 0x40) << 4) & 0x3FFF) as u32);
                uv(fx, sp + 0x65A, ((((w << 6) + 0x40) << 4) & 0x3FFF) as u32);
                if crate::fxlib2::project(fx, sp + 0x5B0, PROJ_OUT, sp + 0x4D0, d, sp + 0x7CC) {
                    fx.submit(d, OT, 0);
                }
            }
        }
        let age = fx.mem.i32(r + 0xE304) + 1;
        fx.mem.set32(r + 0xE304, age);
        if age >= 0x19 {
            fx.mem.write_u8(r + 0xE300, 0);
        }
    }
}

/// The post callback 0x1E037A0(p) [F]: b222's (FRAME_1 masked, a dark
/// full-window veil, a (FF, FF, FF, 80) sprite of half size 10 with texture
/// D+0x18 at the attacker's point, its V sliding with the update, far Z,
/// FRAME_1 unmasked); the sprite's TEX0 / TEX1 slots (sp + 0x180 / 0x188)
/// hold 0x209800's save of 0x1F0FD0's s1 - the callback's address and 0.
fn callback(fx: &mut Fx, p: u32) -> u32 {
    let sp = fx.enter(0x1A0);
    fx.mem.write_u64(sp + 0x180, u64::from(CB));
    fx.mem.write_u64(sp + 0x188, 0);
    let s0 = crate::b440::frame_mask_packet(fx, p, 0xFF00_0000, [0, 0]);
    fx.camera_turn_roll(sp + 0xB0);
    let t = sp + 0x110;
    head(fx, t, 6, 3, 0x441, 0);
    alpha(fx, t, 0);
    rgba(fx, sp + 0x120, [0, 0, 0], 0x80);
    window_xy(fx, t);
    let mut s1 = fx.submit_at(s0, t, 0, 0);
    let d = sp + 0x140;
    head(fx, d, 0x26, 5, 0x4_2421, 0);
    alpha(fx, d, 1);
    let dd = fx.mem.read_u32(D_STATIC);
    let rh = fx.mem.read_u32(dd + 0x18);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 0);
    fx.camera_vp_to(sp + 0x30);
    fx.translate_rows(sp + 0x70, sp + 0xB0, dd + 0x20);
    crate::blib::exe::mul_mem(fx, sp + 0x30, sp + 0x70);
    rgba(fx, sp + 0x150, [0xFF; 3], 0x80);
    let n = fx.mem.i32(dd);
    let f2 = ee::div(ee::mul(-1.0, ee::from_i32(rem2(n, 0x20))), 32.0);
    fx.mem.write_u32(sp + 0x158, 0);
    fx.mem.setf(sp + 0x15C, f2);
    fx.mem.write_u32(sp + 0x168, 0x40A0_0000);
    fx.mem.setf(sp + 0x16C, ee::add(2.0, f2));
    vec4(fx, sp + 0xF0, -10.0, -10.0, 0.0);
    fx.mem.write_u32(sp + 0xF8, 0);
    vec4(fx, sp + 0x100, 10.0, 10.0, 0.0);
    fx.mem.write_u32(sp + 0x108, 0);
    let mut a23 = [sp + 0x30, d];
    if crate::fxlib2::project(fx, sp + 0xF0, PROJ_OUT, sp + 0x30, d, sp + 0x19C) {
        far(fx, sp + 0x160, &[sp + 0x170]);
        s1 = fx.submit_at(s1, d, 0, 0);
        a23 = [0, 0];
    }
    let r = crate::b440::frame_mask_packet(fx, s1, 0, a23);
    fx.leave(0x1A0);
    r
}
