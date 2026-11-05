//! BMODULE b620 (card 150), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080 and its spawner 0x1E01ED0, data area
//! a0 = 0x1ED0000, 0xB0F0 bytes) [F]: a single
//! unscheduled function in a 0x450 frame (D in fp, s0 the OT, the return
//! at sp + 0xC0), b290's family. Every update the camera turn with roll
//! (sp + 0x270), the self unit's point D+0x20 (0x1F65D0) and six nodes
//! 0x29 / 0x34 / 0x3E / 0x50 / 0x47 / 0x59 (D+0x60 + 0x10 k). Every tenth
//! update 10..0x118 a ground wave (8 round robin, D+0x6F4 + 8 k): 32 flat
//! far-Z quads between the rings D+0xC0 (17 / 20 round) at the self point,
//! scaled 0.6 + 0.4 tan falloff, blue alpha 255 sin (age 0x400) x (0.8 +
//! 0.2 (n & 1)) clamp(0x122 - n) / 32. From update 0 for 0x123 updates a
//! textured band (D+0x740, 33 x 2 points 16 round, texture D+0x10)
//! turned by four angle sets, alpha 128 |sin| ramps. On updates 10 (k +
//! 1) + 0x63 / 0x68 / 0x6A / 0x65 / 0x67 / 0x6B (below 0xFA / 0xF5 / 0xF7
//! / 0xF2 / 0xF4 / 0xF8) node k throws a beam (0x1E01ED0; 6 x 4 records
//! D+0xBA0): a sprite rising 40 (4 t - 4 t t) along it for 32 updates
//! leaving trails (16 per record, D+0x14A0, 4 scattered sprites 1.5) and on
//! its end a ring (4 per node, D+0xA7F0): a 16-segment two-band mesh
//! (texture D+0x18) of the points D+0xA4A0 (20 / 50 round) turned at
//! random, scaled 0.1 + 0.9 tan falloff, and a sprite 35 at its centre.
//! Returns 1 on 0x79, 6 on 0x84..0x118, 2 on 0x119, 8 from 0x139.
use crate::blib::exe::mul_mem;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70 / 0x205210's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x450;
/// The flat far-Z quad descriptor (8 registers, RGBAQ / XYZ2 x 4).
const FLAT: u32 = 0x308;
/// The textured strip descriptor (prim 0x34, 12 registers).
const QUAD: u32 = 0x360;
/// The sprite descriptor.
const DESC: u32 = 0x3F0;
const WAVES: u32 = D + 0x6F4;
const BAND: u32 = D + 0x740;
const BEAMS: u32 = D + 0xBA0;
const TRAILS: u32 = D + 0x14A0;
const MESH: u32 = D + 0xA4A0;
const RINGS: u32 = D + 0xA7F0;

/// The registers 0x20C0B0 saves (f20..f24, s1..s3) as the function leaves
/// them [F; the runner's values before the first write: 0, H].
#[derive(Default)]
struct Regs {
    f: [f32; 5],
    s1: u64,
    s2: u64,
    s3: u64,
}

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xC0, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp, param) };
    fx.leave(FRAME);
    ret
}

/// The compiler's signed remainder by a power of two.
fn rem2(x: i32, m: i32) -> i32 {
    let r = x & (m - 1);
    if x < 0 && r != 0 {
        r - m
    } else {
        r
    }
}

fn words(fx: &mut Fx, to: u32, from: u32, n: u32) {
    for c in 0..n {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

fn vec4(fx: &mut Fx, p: u32, v: [f32; 4]) {
    for (k, x) in v.iter().enumerate() {
        fx.mem.setf(p + 4 * k as u32, *x);
    }
}

/// 0x1A2C70: an unsigned integer from the float (with its residue frame).
fn fix(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

/// 0x1A7230 fabsf (its argument left at its sp).
fn fabsf(fx: &mut Fx, x: f32) -> f32 {
    fx.mem.write_f32(fx.sp - 0x10, x);
    f32::from_bits(x.to_bits() & 0x7FFF_FFFF)
}

/// rand as rand % 256 << `sh` >> 8 (an angle); rand's frame saving the
/// caller's s0.
fn rand_angle(fx: &mut Fx, s0: u32, ra: u32, sh: u32) -> i32 {
    let r = fx.rand_from(s0, ra) >> 23;
    (rem2(r, 256) << sh) >> 8
}

/// (17 / 20 or 20 / 50) round points: `p` + 0x30 k = (r0 sin, r0 cos, 0,
/// 1), (r1 sin, r1 cos, 0, 1) and their mean, angle k `step`.
fn round(fx: &mut Fx, p: u32, n: u32, step: i32, r0: f32, r1: f32) {
    for k in 0..n {
        let e = p + 0x30 * k;
        let a = (k as i32).wrapping_mul(step);
        let s = fx.tsin(a);
        fx.mem.setf(e, ee::mul(r0, s));
        let c = fx.tcos(a);
        fx.mem.setf(e + 4, ee::mul(r0, c));
        fx.mem.write_u32(e + 8, 0);
        fx.mem.write_u32(e + 0xC, 0x3F80_0000);
        let s = fx.tsin(a);
        fx.mem.setf(e + 0x10, ee::mul(r1, s));
        let c = fx.tcos(a);
        fx.mem.setf(e + 0x14, ee::mul(r1, c));
        fx.mem.write_u32(e + 0x18, 0);
        fx.mem.write_u32(e + 0x1C, 0x3F80_0000);
        fx.vu_add4(e + 0x20, e, e + 0x10);
        fx.vu_scale4(e + 0x20, e + 0x20, 0.5);
    }
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0xB0F0);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    for k in 0..3u32 {
        let r = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x10 + 4 * k, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, D + 0x30);
    for (k, (node, clear)) in [(0x29, 1), (0x34, 0), (0x3E, 0), (0x50, 0), (0x47, 0), (0x59, 0)].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + 0x40 + 4 * k as u32, r);
    }
    round(fx, D + 0xC0, 0x21, 0x800, 17.0, 20.0);
    for k in 0..0x21u32 {
        let e = BAND + 0x20 * k;
        let a = (k << 11) as i32;
        let s = fx.tsin(a);
        fx.mem.setf(e, ee::mul(16.0, s));
        let c = fx.tcos(a);
        fx.mem.setf(e + 4, ee::mul(16.0, c));
        fx.mem.write_u32(e + 8, 0x3F80_0000);
        fx.mem.write_u32(e + 0xC, 0x3F80_0000);
        words(fx, e + 0x10, e, 4);
        let z = ee::mul(fx.mem.f(e + 0x18), -1.0);
        fx.mem.setf(e + 0x18, z);
    }
    round(fx, MESH, 0x11, 0x1000, 20.0, 50.0);
    0
}

/// The flat quad's vertices' Z set to 0xFFFFF8 (the far plane).
fn far_z(fx: &mut Fx, sp: u32) {
    fx.set_xyz_z(sp + 0x320, 0xFF_FFF8);
    for o in [0x330u32, 0x340, 0x350] {
        let z = fx.xyz_z(sp + 0x320);
        fx.set_xyz_z(sp + o, z);
    }
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

/// The sprite descriptor (prim 0x26 | 0x800, 5 registers, ALPHA preset
/// 1, texture D+0x14, REGION_CLAMP 5).
fn sprites_begin(fx: &mut Fx, sp: u32) {
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
}

/// The sprite UVs (0, 0) .. (0x800, 0x800) and the corners -+`s` turned
/// by the camera (`turn`).
fn sprite_frame(fx: &mut Fx, sp: u32, s: f32, turn: bool) {
    uv(fx, sp + 0x408, 0);
    uv(fx, sp + 0x40A, 0);
    uv(fx, sp + 0x418, 0x800);
    uv(fx, sp + 0x41A, 0x800);
    let n = ee::neg(s);
    vec4(fx, sp + 0x2B0, [n, n, 0.0, 1.0]);
    vec4(fx, sp + 0x2C0, [s, s, 0.0, 1.0]);
    if turn {
        fx.vu_apply_to(sp + 0x2B0, sp + 0x270, sp + 0x2B0);
        fx.vu_apply_to(sp + 0x2C0, sp + 0x270, sp + 0x2C0);
    }
}

/// The sprite colour (c, a, Q 1) at sp + 0x400.
fn sprite_colour(fx: &mut Fx, sp: u32, c: [u8; 3], a: u8) {
    let p = sp + 0x400;
    fx.mem.write_u8(p, c[0]);
    fx.mem.write_u8(p + 1, c[1]);
    fx.mem.write_u8(p + 2, c[2]);
    fx.mem.write_u8(p + 3, a);
    fx.mem.write_u32(p + 4, 0x3F80_0000);
}

/// The sprite at sp + 0x2F0 (+ the corners) projected (0x204F70) and
/// drawn at the mean w.
fn sprite_draw(fx: &mut Fx, sp: u32, ra: u32) {
    fx.vu_add4(sp + 0x2D0, sp + 0x2B0, sp + 0x2F0);
    fx.vu_add4(sp + 0x2E0, sp + 0x2C0, sp + 0x2F0);
    sprite_submit(fx, sp, sp + 0x2D0, ra);
}

fn sprite_submit(fx: &mut Fx, sp: u32, v: u32, ra: u32) {
    if crate::fxlib2::project(fx, v, PROJ_OUT, sp + 0x1F0, sp + DESC, sp + 0x44C) {
        let w = fx.mem.f(sp + 0x44C);
        let key = fix(fx, w, ra);
        fx.submit(sp + DESC, OT, key as i32);
    }
}

/// The textured strip descriptor at sp + 0x360 (ALPHA preset 1, texture
/// `rh`, REGION_CLAMP 0).
fn quad_desc(fx: &mut Fx, sp: u32, rh: u32) {
    let d = sp + QUAD;
    fx.mem.write_u16(d + 2, 0x34);
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 0);
}

/// Colour (c, a, Q 1) at sp + `at`, copied to sp + `at` + `gap`.
fn pair_colour(fx: &mut Fx, sp: u32, at: u32, gap: u32, c: [u8; 3], a: u8) {
    let p = sp + at;
    fx.mem.write_u8(p, c[0]);
    fx.mem.write_u8(p + 1, c[1]);
    fx.mem.write_u8(p + 2, c[2]);
    fx.mem.write_u8(p + 3, a);
    fx.mem.write_u32(p + 4, 0x3F80_0000);
    words(fx, p + gap, p, 2);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let mut rg = Regs { s1: param as i64 as u64, ..Regs::default() };
    fx.camera_turn_roll(sp + 0x270);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x20);
    for k in 0..6u32 {
        let h = fx.mem.i32(D + 4);
        let s = fx.mem.i32(D + 0x40 + 4 * k);
        fx.node_position(h, s, D + 0x60 + 0x10 * k);
    }
    let n = fx.mem.i32(D);
    let (_, r) = fx.div(n, 10);
    if r == 0 && n > 0 && n < 0x122 {
        let c = fx.mem.read_u32(D + 0x6F0);
        fx.mem.write_u8(WAVES + 8 * c, 1);
        fx.mem.write_u32(WAVES + 8 * c + 4, 0);
        let c = fx.mem.i32(D + 0x6F0) + 1;
        fx.mem.set32(D + 0x6F0, c);
        if c >= 8 {
            fx.mem.write_u32(D + 0x6F0, 0);
        }
    }
    waves(fx, sp, &mut rg);
    if fx.mem.i32(D) == 0 {
        fx.mem.write_u8(D + 0xB60, 1);
        fx.mem.write_u32(D + 0xB64, 0);
    }
    if fx.mem.read_u8(D + 0xB60) == 1 {
        band(fx, sp, &mut rg);
    }
    for (k, (off, below)) in [(0x63, 0xFA), (0x68, 0xF5), (0x6A, 0xF7), (0x65, 0xF2), (0x67, 0xF4), (0x6B, 0xF8)].into_iter().enumerate() {
        let c = fx.mem.i32(D + 0xB68 + 4 * k as u32);
        let n = fx.mem.i32(D);
        if n == 10 * c + off && n < below {
            spawn(fx, k as u32);
        }
    }
    beams(fx, sp);
    trails(fx, sp);
    rings(fx, sp, &mut rg);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x79 {
        ret = 1;
    } else {
        if (0x84..0x119).contains(&n) {
            ret = 6;
        }
        if n == 0x119 {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x13A {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xC0, ret as u32);
    ret
}

/// The ground waves [F 0x1E00608].
fn waves(fx: &mut Fx, sp: u32, rg: &mut Regs) {
    let d = sp + FLAT;
    fx.mem.write_u16(d + 2, 0x14);
    fx.mem.write_u16(d, 8);
    fx.mem.write_u64(d + 8, 0x4141_4141);
    fx.mem.write_u32(d + 4, 0);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let n = fx.mem.i32(D);
    rg.s1 = n as i64 as u64;
    let f3 = ee::div(ee::from_i32(Fx::clamp_i(0x122 - n, 0, 0x20)), 32.0);
    let n2 = fx.mem.i32(D);
    let v = ee::from_i32(rem2(n2, 2));
    let f20 = ee::mul(ee::add(f32::from_bits(0x3F4C_CCCD), ee::mul(f32::from_bits(0x3E4C_CCCD), v)), f3);
    let f21 = ee::div(ee::from_i32(Fx::clamp_i(n, 0, 0x20)), 32.0);
    rg.f[0] = f20;
    rg.f[1] = f21;
    for i in 0..8u32 {
        let e = WAVES + 8 * i;
        if fx.mem.read_u8(e) == 0 {
            continue;
        }
        fx.camera_vp_to(sp + 0x1F0);
        fx.translate_rows(sp + 0x230, sp + 0x270, D + 0x20);
        let age = fx.mem.i32(e + 4);
        let t = fx.tan_falloff_p(age, 0x20, 1);
        let s = ee::mul(ee::add(f32::from_bits(0x3F19_999A), ee::mul(f32::from_bits(0x3ECC_CCCD), t)), f21);
        vec4(fx, sp + 0x2F0, [s, s, s, 1.0]);
        fx.scale_rows(sp + 0x230, sp + 0x2F0);
        mul_mem(fx, sp + 0x1F0, sp + 0x230);
        let age = fx.mem.i32(e + 4);
        let sn = fx.tsin((age << 15) >> 5);
        let f0 = ee::mul(sn, f20);
        pair_colour(fx, sp, 0x318, 0x10, [0, 0, 0x80], 0);
        let a = fix(fx, ee::mul(255.0, f0), 0x01E0_07E4);
        pair_colour(fx, sp, 0x338, 0x10, [0x18, 0x40, 0xFF], a as u8);
        for k in 0..0x20u32 {
            let s1 = D + 0x30 * k;
            let s3 = s1 + 0x30;
            if fx.project_n(&[s1 + 0xC0, s3 + 0xC0, s1 + 0xE0, s3 + 0xE0], sp + 0x1F0, d, sp + 0x44C, 0x01E0_0860) == 1 {
                far_z(fx, sp);
                fx.submit(d, OT, 0);
            }
            if fx.project_n(&[s1 + 0xD0, s3 + 0xD0, s1 + 0xE0, s3 + 0xE0], sp + 0x1F0, d, sp + 0x44C, 0x01E0_0958) == 1 {
                far_z(fx, sp);
                fx.submit(d, OT, 0);
            }
        }
        rg.s1 = u64::from(D + 0x600);
        let age = fx.mem.i32(e + 4) + 1;
        fx.mem.set32(e + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(e, 0);
        }
    }
}

/// The band [F 0x1E00AC0].
fn band(fx: &mut Fx, sp: u32, rg: &mut Regs) {
    let q = sp + QUAD;
    let rh = fx.mem.read_u32(D + 0x10);
    quad_desc(fx, sp, rh);
    let age = fx.mem.i32(D + 0xB64);
    let a = ee::from_i32(Fx::clamp_i(age, 0, 0x20));
    let age = fx.mem.i32(D + 0xB64);
    let b = ee::from_i32(Fx::clamp_i(0x122 - age, 0, 0x20));
    let f20 = ee::div(ee::mul(a, b), 1024.0);
    let age = fx.mem.i32(D + 0xB64);
    let s = ee::div(ee::from_i32(Fx::clamp_i(age, 0, 0x20)), 32.0);
    vec4(fx, sp + 0x2F0, [s, s, s, 1.0]);
    let f20 = ee::mul(128.0, f20);
    rg.f[0] = f20;
    for k in 0..4u32 {
        let (s2, s3) = (0x15B3 * k, 0x1E61 * k);
        let age = fx.mem.i32(D + 0xB64);
        let a = (age.wrapping_mul(0x29A).wrapping_add(s2 as i32)) & 0xFFFF;
        let sn = fx.tsin(a);
        let sn = fabsf(fx, sn);
        let f = ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), sn));
        let al = fix(fx, ee::mul(f20, f), 0x01E0_0C18);
        let p = sp + 0x370;
        fx.mem.write_u8(p, 0xA4);
        fx.mem.write_u8(p + 1, 0xA4);
        fx.mem.write_u8(p + 2, 0x80);
        fx.mem.write_u8(p + 3, al as u8);
        fx.mem.write_u32(p + 4, 0x3F80_0000);
        for o in [0x388u32, 0x3A0, 0x3B8] {
            words(fx, sp + o, p, 2);
        }
        fx.camera_vp_to(sp + 0x1F0);
        rg.s2 = u64::from(s2);
        rg.s3 = u64::from(s3);
        fx.trs_into_saving(sp + 0x230, 0, 0, D + 0x20, rg.f, [u64::from(OT), rg.s1, rg.s2, rg.s3], 0x01E0_0C94);
        let age = fx.mem.i32(D + 0xB64);
        let r = age.wrapping_mul(0x14D);
        fx.rotate_axis(sp + 0x230, r, 2);
        fx.rotate_axis(sp + 0x230, r.wrapping_add(s2 as i32), 0);
        fx.rotate_axis(sp + 0x230, r.wrapping_add(s3 as i32), 1);
        fx.scale_rows(sp + 0x230, sp + 0x2F0);
        mul_mem(fx, sp + 0x1F0, sp + 0x230);
        for j in 0..0x20u32 {
            let s1 = BAND + 0x20 * j;
            let s1n = s1 + 0x20;
            let v = (k + j) as i32;
            let f3 = ee::mul(0.25, ee::from_i32(rem2(v, 4)));
            let f0 = ee::mul(0.25, ee::from_i32(v >> 2));
            let f2 = ee::add(0.25, f3);
            let f0b = ee::add(0.25, f0);
            for (o, x) in [(0x378u32, f3), (0x37C, f0), (0x390, f2), (0x394, f0), (0x3A8, f3), (0x3AC, f0b), (0x3C0, f2), (0x3C4, f0b)] {
                fx.mem.setf(sp + o, x);
            }
            if fx.project_n(&[s1, s1n, s1 + 0x10, s1n + 0x10], sp + 0x1F0, q, sp + 0x44C, 0x01E0_0D9C) == 1 {
                let w = fx.mem.f(sp + 0x44C);
                let key = fix(fx, w, 0x01E0_0DB0);
                fx.submit(q, OT, key as i32);
            }
        }
        rg.s1 = u64::from(D + 0x400);
    }
    let age = fx.mem.i32(D + 0xB64) + 1;
    fx.mem.set32(D + 0xB64, age);
    if age >= 0x123 {
        fx.mem.write_u8(D + 0xB60, 0);
    }
}

/// 0x1E01ED0(D, k) [F] (this record's own): node k's next beam record
/// (D+0xBA0 + 0x180 k + 0x60 c, c = D+0xB80[k] round robin of 4): on, age
/// 0, from the node D+0x60 + 0x10 k, aimed (0x100E40) along 2 x a random
/// table direction (0x101080) + the target's point - the node, its length
/// +0xC (powf / sqrtf), turned about z by a random angle; D+0xB68[k] + 1.
fn spawn(fx: &mut Fx, k: u32) {
    let sp = fx.enter(0x90);
    let c = fx.mem.read_u32(D + 0xB80 + 4 * k);
    let r = BEAMS + 0x180 * k + 0x60 * c;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 0);
    words(fx, r + 0x10, D + 0x60 + 0x10 * k, 4);
    let v = sp + 0x80;
    fx.random_table_vec(v, 2.0);
    fx.vu_add4(v, v, D + 0x30);
    fx.vu_sub4(v, v, D + 0x60 + 0x10 * k);
    let x = Fx::powf2(fx.mem.f(v));
    let y = Fx::powf2(fx.mem.f(v + 4));
    let f20 = ee::add(x, y);
    let z = Fx::powf2(fx.mem.f(v + 8));
    let l = fx.sqrtf(ee::add(z, f20));
    fx.mem.setf(r + 0xC, l);
    fx.point_along(r + 0x20, v);
    let a = rand_angle(fx, D, 0x01E0_1FF4, 16);
    fx.rotate_axis(r + 0x20, a, 2);
    let n = fx.mem.i32(D + 0xB68 + 4 * k) + 1;
    fx.mem.set32(D + 0xB68 + 4 * k, n);
    let c = fx.mem.i32(D + 0xB80 + 4 * k) + 1;
    fx.mem.set32(D + 0xB80 + 4 * k, c);
    if c >= 4 {
        fx.mem.write_u32(D + 0xB80 + 4 * k, 0);
    }
    fx.leave(0x90);
}

/// The beams' sprites, their trails and rings [F 0x1E00F54].
fn beams(fx: &mut Fx, sp: u32) {
    sprites_begin(fx, sp);
    fx.camera_vp_to(sp + 0x1F0);
    fx.identity(sp + 0x230);
    mul_mem(fx, sp + 0x1F0, sp + 0x230);
    sprite_frame(fx, sp, 5.0, true);
    for j in 0..6u32 {
        for i in 0..4u32 {
            let g = BEAMS + 0x180 * j + 0x60 * i;
            if fx.mem.read_u8(g) == 0 {
                continue;
            }
            sprite_colour(fx, sp, [0xFF, 0xC4, 0xB4], 0xFF);
            let t = ee::div(ee::from_i32(fx.mem.i32(g + 4)), 32.0);
            let y = ee::add(ee::mul(4.0, t), ee::mul(ee::mul(-4.0, t), t));
            fx.mem.write_u32(sp + 0x2F0, 0);
            fx.mem.setf(sp + 0x2F4, ee::mul(40.0, y));
            let z = ee::mul(t, fx.mem.f(g + 0xC));
            fx.mem.setf(sp + 0x2F8, z);
            fx.mem.write_u32(sp + 0x2FC, 0x3F80_0000);
            fx.vu_apply_to(sp + 0x2F0, g + 0x20, sp + 0x2F0);
            fx.vu_add4(sp + 0x2F0, sp + 0x2F0, g + 0x10);
            sprite_draw(fx, sp, 0x01E0_11F0);
            let c = fx.mem.read_u8(g + 8) as u32;
            let t = TRAILS + 0x1800 * j + 0x600 * i + 0x60 * c;
            fx.mem.write_u8(t, 1);
            fx.mem.write_u32(t + 4, 0);
            words(fx, t + 0x10, sp + 0x2F0, 4);
            for s in 0..4u32 {
                fx.random_table_vec(t + 0x20 + 0x10 * s, f32::from_bits(0x3E99_999A));
            }
            let c = fx.mem.read_u8(g + 8).wrapping_add(1);
            fx.mem.write_u8(g + 8, c);
            if fx.mem.read_u8(g + 8) >= 0x10 {
                fx.mem.write_u8(g + 8, 0);
            }
            if fx.mem.i32(g + 4) == 0x20 {
                let rc = fx.mem.read_u32(D + 0xA7D0 + 4 * j);
                let b = RINGS + 0x180 * j + 0x60 * rc;
                fx.mem.write_u8(b, 1);
                fx.mem.write_u32(b + 4, 0);
                words(fx, b + 0x10, sp + 0x2F0, 4);
                let m = b + 0x20;
                fx.identity(m);
                let a = rand_angle(fx, OT, 0x01E0_1328, 15);
                fx.rotate_axis(m, a, 0);
                let a = rand_angle(fx, OT, 0x01E0_135C, 15);
                fx.rotate_axis(m, a, 1);
                let a = rand_angle(fx, OT, 0x01E0_1390, 16);
                fx.rotate_axis(m, a, 2);
                let c = fx.mem.i32(D + 0xA7D0 + 4 * j) + 1;
                fx.mem.set32(D + 0xA7D0 + 4 * j, c);
                if c >= 4 {
                    fx.mem.write_u32(D + 0xA7D0 + 4 * j, 0);
                }
            }
            let age = fx.mem.i32(g + 4) + 1;
            fx.mem.set32(g + 4, age);
            if age >= 0x21 {
                fx.mem.write_u8(g, 0);
            }
        }
    }
}

/// The trails [F 0x1E01470].
fn trails(fx: &mut Fx, sp: u32) {
    sprites_begin(fx, sp);
    fx.camera_vp_to(sp + 0x1F0);
    fx.identity(sp + 0x230);
    mul_mem(fx, sp + 0x1F0, sp + 0x230);
    sprite_frame(fx, sp, 1.5, true);
    for j in 0..6u32 {
        for i in 0..4u32 {
            for k in 0..0x10u32 {
                let t = TRAILS + 0x1800 * j + 0x600 * i + 0x60 * k;
                if fx.mem.read_u8(t) == 0 {
                    continue;
                }
                let age = fx.mem.i32(t + 4);
                let a = ((0x10 - age) * 0xFF) >> 4;
                sprite_colour(fx, sp, [0xFF, 0x8C, 0x82], a as u8);
                let age = fx.mem.i32(t + 4);
                let tf = fx.tan_falloff_p(age, 0x10, 1);
                let f20 = ee::add(f32::from_bits(0x3ECC_CCCD), ee::mul(f32::from_bits(0x3F19_999A), tf));
                for s in 0..4u32 {
                    fx.vu_scale4(sp + 0x2F0, t + 0x20 + 0x10 * s, f20);
                    fx.vu_add4(sp + 0x2F0, sp + 0x2F0, t + 0x10);
                    sprite_draw(fx, sp, 0x01E0_16E8);
                }
                let age = fx.mem.i32(t + 4) + 1;
                fx.mem.set32(t + 4, age);
                if age >= 0x11 {
                    fx.mem.write_u8(t, 0);
                }
            }
        }
    }
}

/// The rings: the mesh and the centre sprite [F 0x1E01788].
fn rings(fx: &mut Fx, sp: u32, rg: &mut Regs) {
    let q = sp + QUAD;
    let rh = fx.mem.read_u32(D + 0x18);
    quad_desc(fx, sp, rh);
    sprites_begin(fx, sp);
    sprite_frame(fx, sp, 35.0, false);
    for j in 0..6u32 {
        for i in 0..4u32 {
            let b = RINGS + 0x180 * j + 0x60 * i;
            if fx.mem.read_u8(b) == 0 {
                continue;
            }
            let age = ee::from_i32(fx.mem.i32(b + 4));
            let f0 = Fx::ramp_above(age, 32.0, 16.0);
            let p = sp + 0x370;
            fx.mem.write_u8(p, 0x60);
            fx.mem.write_u8(p + 1, 0x26);
            fx.mem.write_u8(p + 2, 0x1C);
            fx.mem.write_u8(p + 3, 0);
            fx.mem.write_u32(p + 4, 0x3F80_0000);
            words(fx, sp + 0x388, p, 2);
            let a = fix(fx, ee::mul(196.0, f0), 0x01E0_19A4);
            pair_colour(fx, sp, 0x3A0, 0x18, [0xFF, 0x62, 0x4E], a as u8);
            fx.camera_vp_to(sp + 0x1F0);
            rg.s2 = u64::from(b - 0xA7F0);
            fx.trs_into_saving(sp + 0x230, 0, 0, b + 0x10, rg.f, [u64::from(OT), rg.s1, rg.s2, rg.s3], 0x01E0_1A00);
            mul_mem(fx, sp + 0x230, b + 0x20);
            let age = fx.mem.i32(b + 4);
            let tf = fx.tan_falloff_p(age, 0x20, 1);
            let s = ee::add(f32::from_bits(0x3DCC_CCCD), ee::mul(f32::from_bits(0x3F66_6666), tf));
            vec4(fx, sp + 0x2F0, [s, s, 0.0, 1.0]);
            fx.scale_rows(sp + 0x230, sp + 0x2F0);
            mul_mem(fx, sp + 0x1F0, sp + 0x230);
            let age = fx.mem.i32(b + 4);
            let f20 = ee::div(ee::mul(-1.0, ee::from_i32(rem2(age, 0x40))), 64.0);
            let f21 = ee::add(0.5, f20);
            let f22 = ee::add(1.0, f20);
            for k in 0..0x10u32 {
                let e0 = MESH + 0x30 * k;
                let e1 = e0 + 0x30;
                let f23 = ee::div(ee::mul(6.0, ee::from_i32(k as i32)), 16.0);
                let f24 = ee::div(ee::mul(6.0, ee::from_i32(k as i32 + 1)), 16.0);
                for (o, v) in [(0x378u32, f23), (0x37C, f20), (0x390, f24), (0x394, f20), (0x3A8, f23), (0x3AC, f21), (0x3C0, f24), (0x3C4, f21)] {
                    fx.mem.setf(sp + o, v);
                }
                if fx.project_n(&[e0, e1, e0 + 0x20, e1 + 0x20], sp + 0x1F0, q, sp + 0x44C, 0x01E0_1BA4) == 1 {
                    let w = fx.mem.f(sp + 0x44C);
                    let key = fix(fx, w, 0x01E0_1BB8);
                    fx.submit(q, OT, key as i32);
                }
                for (o, v) in [(0x378u32, f23), (0x37C, f22), (0x390, f24), (0x394, f22), (0x3A8, f23), (0x3AC, f21), (0x3C0, f24), (0x3C4, f21)] {
                    fx.mem.setf(sp + o, v);
                }
                if fx.project_n(&[e0 + 0x10, e1 + 0x10, e0 + 0x20, e1 + 0x20], sp + 0x1F0, q, sp + 0x44C, 0x01E0_1C2C) == 1 {
                    let w = fx.mem.f(sp + 0x44C);
                    let key = fix(fx, w, 0x01E0_1C40);
                    fx.submit(q, OT, key as i32);
                }
                rg.f = [f20, f21, f22, f23, f24];
            }
            rg.s1 = u64::from(D + 0x300);
            rg.s3 = u64::from(MESH + 0x30 * 0x10 + 0x20);
            let age = ee::from_i32(fx.mem.i32(b + 4));
            let f0 = Fx::ramp_above(age, 32.0, 16.0);
            let a = fix(fx, ee::mul(255.0, f0), 0x01E0_1C94);
            sprite_colour(fx, sp, [0xFF, 0x80, 0x5A], a as u8);
            fx.camera_vp_to(sp + 0x1F0);
            fx.translate_rows(sp + 0x230, sp + 0x270, b + 0x10);
            let age = fx.mem.i32(b + 4);
            let tf = fx.tan_falloff_p(age, 0x20, 1);
            let s = ee::add(f32::from_bits(0x3DCC_CCCD), ee::mul(f32::from_bits(0x3F66_6666), tf));
            vec4(fx, sp + 0x2F0, [s, s, s, 1.0]);
            fx.scale_rows(sp + 0x230, sp + 0x2F0);
            mul_mem(fx, sp + 0x1F0, sp + 0x230);
            sprite_submit(fx, sp, sp + 0x2B0, 0x01E0_1D74);
            let age = fx.mem.i32(b + 4) + 1;
            fx.mem.set32(b + 4, age);
            if age >= 0x21 {
                fx.mem.write_u8(b, 0);
            }
        }
    }
}
