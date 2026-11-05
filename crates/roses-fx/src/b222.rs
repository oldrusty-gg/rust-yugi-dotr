//! BMODULE b222 (card 563) and its 0.96 twin b765 (card 612, a [`V`]),
//! hand-ported from the records (linked at 0x1E00000, the entry 0x1E00080
//! in a 0x790 / 0x760 frame, data area a0 = 0x1ED0000, 0xF920 bytes; one
//! attacker post callback 0x1E03760 / 0x1E03790 with its static D at
//! 0x1E03A80 / 0x1E03B00) [F]. The init lays out
//! a 17 x 17 wave grid (D+0x3070, rows 0x110, 600 / 16 apart about the
//! origin, a random phase each at D+0x4280), the shard texel table
//! (D+0x8E10 / D+0x8E90), four shard edges (D+0x8ED0 / D+0x8EE0, radius
//! 25 a third of a turn apart) and the 16 x 33 attacker rings (D+0xB710,
//! radius 40 (20) (16 - k) / 16). Each update, from the attacker's point
//! (D+0x20) and its front point's Z (D+0x44): a white flash (D+0x48);
//! two refracting ring walls at the target (D+0x1B20: 21 rings of 9 drawn
//! twice, coloured then through the frame copied as a texture with screen
//! UVs and a winding test 0x101430); 16-sprite bursts (D+0x1B70), 2 x 32
//! sprite fountains (D+0x4710) and 2 x 20 spinning shards (D+0x8F50) along
//! the walls; a rolling wave sheet (D+0x4704); the stage light; the
//! attacker drawn into its target with the callback (a dark veil and a
//! sprite) and pulled through 16 rings of quads and a fan textured with
//! it (D+0xF910); a frame-textured window sprite; returns 1 / 6 / 2 / 8.
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

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

/// The two records' differences (b222 first).
struct V {
    frame: u32,
    /// b765's offsets from sp + 0x290 up sit 0x30 lower (only the
    /// boundary-visible ones are modelled).
    sh: u32,
    ras: &'static [(u32, u32)],
    cb: u32,
    d_static: u32,
    /// The attacker rings' (D+0xB710) radius.
    ring_r: f32,
    flash: i32,
    flash_len: i32,
    flash_end: i32,
    wall_r: f32,
    /// The wall's rise clamp (age - lo, 0, hi) / div.
    rise: (i32, i32, f32),
    /// b765: the walls drawn apart by -100 clamp(age - 0x122, 0, 0x40) / 64.
    spread: bool,
    wall_s: f32,
    wall_base: i32,
    wall_fade: i32,
    wall_g: u8,
    wall_g2: u8,
    wall_disp: f32,
    wall_end: i32,
    burst: (i32, i32),
    spr_g: u8,
    win2: (i32, i32),
    wave: i32,
    wave_ramp: (f32, f32),
    wave_g: u8,
    wave_end: i32,
    light: i32,
    /// Attacker rings: radius, a's age factor / k step / s step, b's age
    /// factor, the angle's amplitude.
    rr: f32,
    ra_age: i32,
    ra_k: i32,
    ra_s: i32,
    rb_age: i32,
    ramp: f32,
    rclamp: i32,
    rdiv: f32,
    rcol: [u8; 3],
    cb_half: f32,
    win: (i32, i32),
    rets: (i32, i32, i32, i32),
}

impl V {
    fn ra(&self, a: u32) -> u32 {
        self.ras.iter().find(|p| p.0 == a).map_or(a, |p| p.1)
    }
}

const B222: V = V {
    frame: 0x790,
    sh: 0,
    ras: &[],
    cb: 0x01E0_3760,
    d_static: 0x01E0_3A80,
    ring_r: 40.0,
    flash: 0x42,
    flash_len: 0x18,
    flash_end: 0x19,
    wall_r: 12.0,
    rise: (0, 100, 100.0),
    spread: false,
    wall_s: 12.0,
    wall_base: 0x6AAA,
    wall_fade: 0x100,
    wall_g: 0xB2,
    wall_g2: 0x7C,
    wall_disp: f(0x3E99_999A),
    wall_end: 0x101,
    burst: (0xA3, 0x142),
    spr_g: 0xA8,
    win2: (0x43, 0x142),
    wave: 0,
    wave_ramp: (386.0, 354.0),
    wave_g: 0xD2,
    wave_end: 0x183,
    light: 0x182,
    rr: 20.0,
    ra_age: 0xD05,
    ra_k: 0x10000,
    ra_s: 0x20000,
    rb_age: 0x8AE,
    ramp: 14563.0,
    rclamp: 0x20,
    rdiv: 1024.0,
    rcol: [0x4F, 0xC4, 0xFF],
    cb_half: 20.0,
    win: (0x42, 0x172),
    rets: (0x98, 0xA3, 0x152, 0x183),
};

const B765: V = V {
    frame: 0x760,
    sh: 0x30,
    ras: RA612,
    cb: 0x01E0_3790,
    d_static: 0x01E0_3B00,
    ring_r: 20.0,
    flash: 0x82,
    flash_len: 0x30,
    flash_end: 0x31,
    wall_r: 20.0,
    rise: (0x10, 0x80, 128.0),
    spread: true,
    wall_s: 15.0,
    wall_base: 0x4000,
    wall_fade: 0x162,
    wall_g: 0x9E,
    wall_g2: 0x78,
    wall_disp: f(0x3E4C_CCCD),
    wall_end: 0x163,
    burst: (0x103, 0x1AE),
    spr_g: 0x8A,
    win2: (0x83, 0x1E4),
    wave: 0x3B,
    wave_ramp: (425.0, 393.0),
    wave_g: 0xA0,
    wave_end: 0x1AA,
    light: 0x1DF,
    rr: 10.0,
    ra_age: 0x8AE,
    ra_k: 0x20000,
    ra_s: 0x10000,
    rb_age: 0x457,
    ramp: 3640.0,
    rclamp: 0x60,
    rdiv: 9216.0,
    rcol: [0xFF, 0x60, 0xC8],
    cb_half: 10.0,
    win: (0x82, 0x1DF),
    rets: (0xF8, 0x103, 0x1BE, 0x1E5),
};

/// The entry 0x1E00080 of b222 (card 563).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B222)
}

/// The entry 0x1E00080 of b765 (card 612).
pub fn program_b765(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B765)
}

fn run(fx: &mut Fx, param: i32, v: &V) -> i32 {
    let spr = fx.enter(v.frame);
    fx.mem.write_u32(spr + 0xC0, 0);
    let ret = if param >= 0 { init(fx, v) } else { step(fx, v, spr) };
    fx.leave(v.frame);
    ret
}

/// rand() >> 23 from `ra` (the caller's s0 = D).
fn rnd(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(D, ra) >> 23
}

fn vec4(fx: &mut Fx, p: u32, x: f32, y: f32, z: f32) {
    fx.mem.setf(p, x);
    fx.mem.setf(p + 4, y);
    fx.mem.setf(p + 8, z);
    fx.mem.write_u32(p + 0xC, 0x3F80_0000);
}

fn init(fx: &mut Fx, v: &V) -> i32 {
    fx.bzero(D, 0xF920);
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
    fx.mem.write_u8(D + 0x48, 0);
    fx.mem.write_u8(D + 0x1B20, 0);
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x1B70 + 0x150 * k, 0);
    }
    // The wave grid and its phases.
    for s6 in 0..0x11i32 {
        let f20 = ee::div(ee::mul(600.0, ee::from_i32(s6)), 16.0);
        for s1 in 0..0x11i32 {
            let p = D + 0x110 * s6 as u32 + 0x10 * s1 as u32;
            let z = ee::sub(ee::div(ee::mul(600.0, ee::from_i32(s1)), 16.0), 300.0);
            fx.mem.setf(p + 0x3070, ee::sub(f20, 300.0));
            fx.mem.write_u32(p + 0x3074, 0);
            fx.mem.setf(p + 0x3078, z);
            fx.mem.write_u32(p + 0x307C, 0x3F80_0000);
            let c = rem2(rnd(fx, 0x01E0_0228), 0x100);
            fx.mem.set32(D + 0x44 * s6 as u32 + 4 * s1 as u32 + 0x4280, (c << 16) >> 8);
        }
    }
    fx.mem.write_u8(D + 0x4704, 0);
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x4710 + 0x470 * k, 0);
    }
    // The shard texels: eight cells of four S and two T.
    for a2 in 0..8i32 {
        let f2 = ee::from_i32(rem2(a2, 4) << 5);
        for a3 in 0..4i32 {
            let s = ee::div(ee::add(f2, ee::div(ee::mul(32.0, ee::from_i32(a3)), 3.0)), 128.0);
            fx.mem.setf(D + 0x8E10 + 0x10 * a2 as u32 + 4 * a3 as u32, s);
        }
        let f2 = ee::from_i32((a2 >> 2) << 5);
        fx.mem.setf(D + 0x8E90 + 8 * a2 as u32, ee::div(f2, 64.0));
        fx.mem.setf(D + 0x8E94 + 8 * a2 as u32, ee::div(ee::add(32.0, f2), 64.0));
    }
    // The shard edges: top / bottom pairs a third of a turn apart.
    let mut s1 = 0i32;
    for s3 in 0..4i32 {
        let r = D + 0x20 * s3 as u32;
        fx.mult(0x5555_5556, s1);
        let s4 = fx.hilo().1.wrapping_add(((s1 as u32) >> 31) as i32);
        let sn = fx.tsin(s4);
        fx.mem.setf(r + 0x8ED0, ee::mul(25.0, sn));
        fx.mem.write_u32(r + 0x8ED4, 0x4140_0000);
        let cs = fx.tcos(s4);
        fx.mem.setf(r + 0x8ED8, ee::mul(25.0, cs));
        fx.mem.write_u32(r + 0x8EDC, 0x3F80_0000);
        for w in 0..4u32 {
            let x = fx.mem.read_u32(r + 0x8ED0 + 4 * w);
            fx.mem.write_u32(r + 0x8EE0 + 4 * w, x);
        }
        let f1 = ee::div(ee::mul(-25.0, ee::from_i32(s3)), 3.0);
        let y = ee::mul(fx.mem.f(r + 0x8EE4), -1.0);
        fx.mem.setf(r + 0x8EE4, y);
        let f1 = ee::add(12.5, f1);
        let y = ee::add(fx.mem.f(r + 0x8ED4), f1);
        fx.mem.setf(r + 0x8ED4, y);
        let y = ee::add(fx.mem.f(r + 0x8EE4), f1);
        fx.mem.setf(r + 0x8EE4, y);
        s1 = s1.wrapping_add(0x8000);
    }
    for k in 0..0xCu32 {
        fx.mem.write_u8(D + 0x8F50 + 0x350 * k, 0);
    }
    // The attacker rings.
    for s4 in 0..0x10i32 {
        let f20 = ee::div(ee::mul(v.ring_r, ee::from_i32(0x10 - s4)), 16.0);
        for s5 in 0..0x21i32 {
            let a = (s5 << 16) >> 5;
            let p = D + 0x210 * s4 as u32 + 0x10 * s5 as u32 + 0xB710;
            let sn = fx.tsin(a);
            fx.mem.setf(p, ee::mul(f20, sn));
            let cs = fx.tcos(a);
            fx.mem.setf(p + 4, ee::mul(f20, cs));
            fx.mem.write_u32(p + 8, 0);
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        }
    }
    fx.mem.write_u8(D + 0xF910, 0);
    0
}

fn step(fx: &mut Fx, v: &V, spr: u32) -> i32 {
    let sp = spr - v.sh;
    fx.camera_turn_roll(sp + 0x490);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x20);
    camera(fx, sp);
    fx.mem.write_u32(sp + 0x550, 0);
    fx.mem.write_u32(sp + 0x554, 0);
    let r = fx.mem.f(D + 0x40);
    fx.mem.setf(sp + 0x558, ee::neg(r));
    fx.mem.write_u32(sp + 0x55C, 0x3F80_0000);
    fx.vu_apply_to(sp + 0x550, sp + 0x490, sp + 0x550);
    fx.vu_add4(sp + 0x550, sp + 0x550, D + 0x20);
    let m = fx.read_m4(sp + 0x410);
    fx.project_at(sp + 0x550, &m, sp + 0x780, sp + 0x78C);
    let z = (fx.mem.read_u64(sp + 0x780) << 8) >> 40;
    fx.mem.write_u32(D + 0x44, z as u32);

    flash(fx, v, sp);
    walls(fx, v, sp);
    bursts(fx, v, sp);
    fountains(fx, v, spr, sp);
    let shard = shards(fx, v, sp);
    // a3 at the attacker pass's first TEXFLUSH: the last projection's
    // descriptor (0x205210's untouched argument), else 0x208400's d + 2.
    let a3 = if wave(fx, v, sp) {
        sp + 0x708
    } else if shard {
        sp + 0x678
    } else {
        sp + 0x67A
    };

    let n = fx.mem.i32(D);
    let s2 = Fx::clamp_i(v.light - n, 0, 0x20);
    let c = Fx::clamp_i(n, 0, 0x20);
    let l = (fx.mult(s2, c.wrapping_neg() << 6) >> 10) + 0x80;
    fx.set_stage_light(l & 0xFF);

    attacker_pass(fx, v, sp, a3);
    window(fx, v, sp);

    let n = fx.mem.i32(D);
    let (r1, r6, r2, r8) = v.rets;
    let mut ret = fx.mem.i32(spr + 0xC0);
    if n == r1 {
        ret = 1;
    } else {
        if n >= r6 && n < r2 {
            ret = 6;
        }
        if n == r2 {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= r8 {
        ret = 8;
    }
    fx.mem.set32(spr + 0xC0, ret);
    ret
}

/// The camera's view-projection at sp + 0x410 times the identity at sp +
/// 0x450.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x410);
    fx.identity(sp + 0x450);
    crate::blib::exe::mul_mem(fx, sp + 0x410, sp + 0x450);
}

/// The camera at the attacker's point under the camera turn (sp + 0x490).
fn camera_at(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x410);
    fx.translate_rows(sp + 0x450, sp + 0x490, D + 0x20);
    crate::blib::exe::mul_mem(fx, sp + 0x410, sp + 0x450);
}

/// The camera at `t` (0x20C0B0 with a translation only).
fn camera_to(fx: &mut Fx, sp: u32, t: u32) {
    fx.camera_vp_to(sp + 0x410);
    let tr = std::array::from_fn(|c| fx.mem.f(t + 4 * c as u32));
    let m = fx.trs(None, None, Some(tr));
    fx.write_m4(sp + 0x450, &m);
    crate::blib::exe::mul_mem(fx, sp + 0x410, sp + 0x450);
}

/// A full-window sprite at `d` (the XYZ2s at +0x18 / +0x20).
fn window_xy(fx: &mut Fx, d: u32) {
    far_xy(fx, d + 0x18, 0x7000, 0x7000);
    far_xy(fx, d + 0x20, 0x9000, 0x9000);
}

/// The white flash (D+0x48) [F 0x1E00668..0x1E007C0].
fn flash(fx: &mut Fx, v: &V, sp: u32) {
    if fx.mem.i32(D) == v.flash {
        fx.mem.write_u8(D + 0x48, 1);
        fx.mem.write_u32(D + 0x4C, 0);
        fx.mem.write_u8(D + 0xF910, 0);
    }
    if fx.mem.read_u8(D + 0x48) != 1 {
        return;
    }
    let d = sp + 0x5D0;
    head(fx, d, 6, 3, 0x441, 0);
    alpha(fx, d, 1);
    let age = fx.mem.i32(D + 0x4C);
    let t = fx.tan_falloff_p(age, v.flash_len, 1);
    let a = fix(fx, ee::mul(200.0, ee::sub(1.0, t)), v.ra(0x01E0_06FC));
    rgba(fx, sp + 0x5E0, [0xFF, 0xF8, 0xE8], a as u8);
    window_xy(fx, d);
    fx.submit(d, OT, 0);
    let age = fx.mem.i32(D + 0x4C) + 1;
    fx.mem.set32(D + 0x4C, age);
    if age >= v.flash_end {
        fx.mem.write_u8(D + 0x48, 0);
    }
}

/// The nine-point ring (D+0x50) of radius `r`.
fn ring9(fx: &mut Fx, r: f32) {
    for s4 in 0..9i32 {
        let a = (s4 << 16) >> 3;
        let p = D + 0x50 + 0x10 * s4 as u32;
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

/// The two ring walls at the target (D+0x1B20) [F 0x1E007C4..0x1E013C4].
fn walls(fx: &mut Fx, v: &V, sp: u32) {
    if fx.mem.i32(D) == v.flash {
        fx.mem.write_u8(D + 0x1B20, 1);
        fx.mem.write_u32(D + 0x1B24, 0);
        for s4 in 0..2i32 {
            let r = D + 0x10 * s4 as u32;
            let a = (((s4 << 16) >> 1) + 0x4000) & 0xFFFF;
            let sn = fx.tsin(a);
            fx.mem.setf(r + 0x1B30, ee::mul(v.wall_r, sn));
            fx.mem.write_u32(r + 0x1B34, 0x4100_0000);
            let cs = fx.tcos(a);
            fx.mem.setf(r + 0x1B38, ee::mul(v.wall_r, cs));
            fx.mem.write_u32(r + 0x1B3C, 0x3F80_0000);
            let x = ee::add(fx.mem.f(r + 0x1B30), fx.mem.f(D + 0x30));
            fx.mem.setf(r + 0x1B30, x);
            let z = ee::add(fx.mem.f(r + 0x1B38), fx.mem.f(D + 0x38));
            fx.mem.setf(r + 0x1B38, z);
        }
    }
    if fx.mem.read_u8(D + 0x1B20) != 1 {
        return;
    }
    let age = fx.mem.i32(D + 0x1B24);
    let c = Fx::clamp_i(age - v.rise.0, 0, v.rise.1);
    let f20 = ee::div(ee::from_i32(c), v.rise.2);
    let off = if v.spread { 0x122 } else { 0 };
    let c2 = Fx::clamp_i(age - off, 0, 0x40);
    let f5 = ee::mul(-100.0, ee::div(ee::from_i32(c2), 64.0));
    for a in 0..2i32 {
        let r = D + 0x10 * a as u32;
        for w in 0..4u32 {
            let x = fx.mem.read_u32(r + 0x1B30 + 4 * w);
            fx.mem.write_u32(r + 0x1B50 + 4 * w, x);
        }
        let side = ee::from_i32(fx.mem.i32(D + 0xC));
        let f2 = ee::mul(70.0, side);
        let z = fx.mem.f(r + 0x1B38);
        fx.mem.setf(r + 0x1B58, ee::add(f2, ee::mul(f20, ee::sub(z, f2))));
        if v.spread {
            let s = rem2(a, 2) * 2 - 1;
            let x = ee::add(fx.mem.f(r + 0x1B50), ee::mul(f5, ee::from_i32(s)));
            fx.mem.setf(r + 0x1B50, x);
        }
    }
    let n = fx.mem.i32(D);
    let c = Fx::clamp_i(n - v.flash, 0, 0x10);
    let f20 = ee::div(ee::mul(v.wall_s, ee::from_i32(c)), 16.0);
    for s7 in 0..2i32 {
        let fp = D + 0x150 * s7 as u32;
        let s4 = 0x6301 * s7;
        for s5 in 0..0x15i32 {
            let s2 = s5 << 15;
            fx.mult(0x6666_6667, s2);
            let q = (fx.hilo().1 >> 3).wrapping_add(((s2 as u32) >> 31) as i32);
            let age = fx.mem.i32(D + 0x1B24);
            let a = (age.wrapping_mul(0x222).wrapping_add(s4 + q) + v.wall_base) & 0xFFFF;
            let p = fp + 0x10 * s5 as u32 + 0x1880;
            let sn = fx.tsin(a);
            fx.mem.setf(p, ee::mul(f20, sn));
            fx.mem.write_u32(p + 4, 0);
            let cs = fx.tcos(a);
            fx.mem.setf(p + 8, ee::mul(f20, cs));
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        }
    }
    let age = fx.mem.i32(D + 0x1B24);
    let c = Fx::clamp_i(age, 0, 0x10);
    let f24 = ee::mul(130.0, fx.tan_falloff_p(c, 0x10, 1));
    let age = fx.mem.i32(D + 0x1B24);
    let c = Fx::clamp_i(v.wall_fade - age, 0, 0x20);
    let f21 = ee::div(ee::from_i32(c), 32.0);
    let q = sp + 0x678;
    quad_desc(fx, q);
    alpha(fx, q, 1);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(q, rh, 1);
    fx.desc_clamp(q, 0);
    rgba(fx, sp + 0x688, [0x80, v.wall_g, 0xFF], 0x40);
    for o in [0x6A0u32, 0x6B8, 0x6D0] {
        copy8(fx, sp + o, sp + 0x688);
    }
    ring9(fx, ee::mul(10.0, f21));
    // a0..a3 at the first TEXFLUSH: 0x206F30's (desc, ot, key) and the
    // projection's desc after a drawn quad, else 0x205210's (in, out, m,
    // desc) of the rejected one.
    let mut last = [0u32; 4];
    for t in 0..2i32 {
        camera_to(fx, sp, D + 0x10 * t as u32 + 0x1B50);
        grid(fx, t, f24);
        let age = fx.mem.i32(D + 0x1B24);
        let f1 = ee::div(ee::mul(2.0, ee::from_i32(rem2(age, 0x10))), 16.0);
        let f20 = ee::sub(ee::div(ee::from_i32(t), 2.0), f1);
        for s7 in 0..0x14i32 {
            let f22 = ee::add(ee::div(ee::mul(5.0, ee::from_i32(s7)), 20.0), f20);
            let f23 = ee::add(ee::div(ee::mul(5.0, ee::from_i32(s7 + 1)), 20.0), f20);
            for s6 in 0..8i32 {
                let f2 = ee::div(ee::mul(2.0, ee::from_i32(s6)), 8.0);
                let f0 = ee::div(ee::mul(2.0, ee::from_i32(s6 + 1)), 8.0);
                for (o, x) in [(0x690u32, f2), (0x694, f22), (0x6A8, f0), (0x6AC, f22), (0x6C0, f2), (0x6C4, f23), (0x6D8, f0), (0x6DC, f23)] {
                    fx.mem.setf(sp + o, x);
                }
                let vs = cell(s7, s6, 0xE0);
                if fx.project_n(&vs, sp + 0x410, q, sp + 0x78C, v.ra(0x01E0_0DF0)) == 1 {
                    let w = fx.mem.f(sp + 0x78C);
                    let key = fix(fx, w, v.ra(0x01E0_0E04));
                    fx.submit(q, OT, key as i32);
                    last = [q, OT, key, q];
                } else {
                    last = [fx.sp - 0x40, PROJ_OUT, sp + 0x410, q];
                }
            }
        }
    }
    quad_desc(fx, q);
    let layer = fx.layer_begin();
    let regs = crate::lens::frame_copy(fx, last, 1, sp + 0x6F0, sp + 0x6F8);
    fx.texflush(regs);
    alpha(fx, q, 0);
    fx.desc_clamp(q, LENS_CLAMP);
    let fl = fx.mem.read_u16(q + 2);
    fx.mem.write_u16(q + 2, fl | 0x800);
    rgba(fx, sp + 0x688, [0x74, v.wall_g2, 0x80], 0x80);
    for o in [0x6A0u32, 0x6B8, 0x6D0] {
        copy8(fx, sp + o, sp + 0x688);
    }
    ring9(fx, ee::mul(9.5, f21));
    for t in 0..2i32 {
        camera_to(fx, sp, D + 0x10 * t as u32 + 0x1B50);
        for s7 in 0..0x15i32 {
            let f20 = ee::div(ee::mul(f24, ee::from_i32(s7)), 20.0);
            fx.identity(sp + 0x510);
            let ph = phase(fx, t);
            let fp = s7 << 16;
            fx.mult(0x6666_6667, fp);
            let q20 = (fx.hilo().1 >> 3).wrapping_add(((fp as u32) >> 31) as i32);
            let sn = fx.tsin((q20 + ph) & 0xFFFF);
            let rot = fixs(fx, ee::mul(910.0, sn), v.ra(0x01E0_10E8));
            fx.rotate_axis(sp + 0x510, rot, 1);
            let src = D + 0x150 * t as u32 + 0x10 * s7 as u32 + 0x1880;
            for s6 in 0..9i32 {
                let g = D + 0x90 * s7 as u32 + 0x10 * s6 as u32;
                let rp = D + 0x50 + 0x10 * s6 as u32;
                for w in 0..4u32 {
                    let x = fx.mem.read_u32(rp + 4 * w);
                    fx.mem.write_u32(g + 0xE0 + 4 * w, x);
                }
                fx.mem.setf(g + 0xE4, f20);
                fx.vu_add4(g + 0xE0, g + 0xE0, src);
                fx.vu_apply_to(g + 0xCB0, sp + 0x510, rp);
                let ph = phase(fx, t);
                let sn = fx.tsin((((s6 << 16) >> 3) + ph) & 0xFFFF);
                let f2 = ee::div(f24, 20.0);
                let f0 = ee::mul(ee::mul(v.wall_disp, f2), sn);
                fx.mem.setf(g + 0xCB4, ee::add(f20, f0));
                fx.vu_add4(g + 0xCB0, g + 0xCB0, src);
            }
        }
        for s7 in 0..0x14i32 {
            for s6 in 0..8i32 {
                let vs = cell(s7, s6, 0xCB0);
                if fx.project_n(&vs, sp + 0x410, q, sp + 0x78C, v.ra(0x01E0_1270)) != 1 {
                    continue;
                }
                for (u, x) in [(0x690u32, 0x698u32), (0x6A8, 0x6B0), (0x6C0, 0x6C8), (0x6D8, 0x6E0)] {
                    fx.uv_from_xy(sp + u, sp + x, 0, 0);
                }
                let vs = cell(s7, s6, 0xE0);
                if fx.project_n(&vs, sp + 0x410, q, sp + 0x78C, v.ra(0x01E0_12EC)) == 1 && winding(fx, sp + 0x698, sp + 0x6B0, sp + 0x6C8) == 1 {
                    fx.submit(q, 0, 0);
                }
            }
        }
    }
    fx.layer_end(layer, OT, 1);
    let age = fx.mem.i32(D + 0x1B24) + 1;
    fx.mem.set32(D + 0x1B24, age);
    if age >= v.wall_end {
        fx.mem.write_u8(D + 0x1B20, 0);
    }
}

/// (rem(age + 5 t, 64) << 16) >> 6: the wall's sway phase.
fn phase(fx: &mut Fx, t: i32) -> i32 {
    let age = fx.mem.i32(D + 0x1B24);
    (rem2(age + 5 * t, 0x40) << 16) >> 6
}

/// The wall grid (D+0xE0, 21 rows of 9) of side `t`: the ring raised by
/// f24 s6 / 20 about the ring offsets.
fn grid(fx: &mut Fx, t: i32, f24: f32) {
    for s6 in 0..0x15i32 {
        let f20 = ee::div(ee::mul(f24, ee::from_i32(s6)), 20.0);
        let src = D + 0x150 * t as u32 + 0x10 * s6 as u32 + 0x1880;
        for s2 in 0..9i32 {
            let g = D + 0x90 * s6 as u32 + 0x10 * s2 as u32 + 0xE0;
            for w in 0..4u32 {
                let x = fx.mem.read_u32(D + 0x50 + 0x10 * s2 as u32 + 4 * w);
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

/// 0x101430(a, b, c) [F]: 1 when the screen triangle of the
/// XYZ2 halfword pairs at `a`, `b`, `c` winds clockwise - the three as
/// (x, y, 0, 1) in its 0x60 frame (+0x30 / +0x40 / +0x50), their normal
/// 0x2057B0(3, pts, sp + 0x20) (the edge cross products 0x20C8B0 summed,
/// divided by 3, normalized 0x20C7E0) with z < 0; else 0.
pub fn winding(fx: &mut Fx, a: u32, b: u32, c: u32) -> i32 {
    let sp = fx.enter(0x60);
    for (k, p) in [a, b, c].into_iter().enumerate() {
        let o = sp + 0x30 + 0x10 * k as u32;
        let x = ee::from_i32(i32::from(fx.mem.read_u16(p)));
        let y = ee::from_i32(i32::from(fx.mem.read_u16(p + 2)));
        fx.mem.setf(o, x);
        fx.mem.setf(o + 4, y);
        fx.mem.write_u32(o + 8, 0);
        fx.mem.write_u32(o + 0xC, 0x3F80_0000);
    }
    normal(fx, 3, sp + 0x30, sp + 0x20);
    let z = fx.mem.f(sp + 0x28);
    fx.leave(0x60);
    i32::from(crate::exe::ee_lt(z, 0.0))
}

/// 0x2057B0(n, pts, out) [F]: out = the normalized mean of the cross
/// products of consecutive edges of the polygon `pts` (n >= 3 vec4s; its
/// 0x80 frame holds the edges at +0x50 / +0x60, their w from 0x293070 /
/// 0x293080, the product at +0x70); 1, or 0 with out zeroed for n < 3.
fn normal(fx: &mut Fx, n: i32, pts: u32, out: u32) -> i32 {
    for k in 0..4u32 {
        fx.mem.write_u32(out + 4 * k, 0);
    }
    if n < 3 {
        return 0;
    }
    let sp = fx.enter(0x80);
    for i in 0..n {
        let (_, r0) = fx.div(i, n);
        let (_, r1) = fx.div(i + 1, n);
        let (_, r2) = fx.div(i + 2, n);
        let (p0, p1, p2) = (pts + 16 * r0 as u32, pts + 16 * r1 as u32, pts + 16 * r2 as u32);
        let w = fx.read_v4(0x0029_3070);
        fx.write_v4(sp + 0x50, w);
        for k in 0..3u32 {
            let x = ee::sub(fx.mem.f(p1 + 4 * k), fx.mem.f(p0 + 4 * k));
            fx.mem.setf(sp + 0x50 + 4 * k, x);
        }
        let w = fx.read_v4(0x0029_3080);
        fx.write_v4(sp + 0x60, w);
        for k in 0..3u32 {
            let x = ee::sub(fx.mem.f(p2 + 4 * k), fx.mem.f(p1 + 4 * k));
            fx.mem.setf(sp + 0x60 + 4 * k, x);
        }
        crate::blib::quadtrail::cross3(fx, sp + 0x70, sp + 0x50, sp + 0x60);
        for k in 0..3u32 {
            let x = ee::add(fx.mem.f(out + 4 * k), fx.mem.f(sp + 0x70 + 4 * k));
            fx.mem.setf(out + 4 * k, x);
        }
    }
    let nf = ee::from_i32(n);
    for k in 0..3u32 {
        let x = ee::div(fx.mem.f(out + 4 * k), nf);
        fx.mem.setf(out + 4 * k, x);
    }
    crate::blib::trail::normalize3(fx, out);
    fx.leave(0x80);
    1
}

fn sprite_desc(fx: &mut Fx, d: u32, preset: i32, tex: u32, clamp: u64) {
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    alpha(fx, d, preset);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, clamp);
}

/// A sprite's corners at sp + 0x4D0 / 0x4E0: (-s, -s) and (s, s) turned by
/// the camera turn.
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    vec4(fx, sp + 0x4D0, ee::neg(s), ee::neg(s), 0.0);
    vec4(fx, sp + 0x4E0, s, s, 0.0);
    fx.mem.write_u32(sp + 0x4D8, 0);
    fx.mem.write_u32(sp + 0x4E8, 0);
    fx.vu_apply_to(sp + 0x4D0, sp + 0x490, sp + 0x4D0);
    fx.vu_apply_to(sp + 0x4E0, sp + 0x490, sp + 0x4E0);
}

/// Cell `c` of a 4 x 2 sprite atlas into the sprite's UVs.
fn atlas(fx: &mut Fx, sp: u32, c: i32) {
    let u = rem2(c, 4);
    let w = c >> 2;
    uv(fx, sp + 0x590, ((u << 9) & 0x3FFF) as u32);
    uv(fx, sp + 0x592, ((w << 9) & 0x3FFF) as u32);
    uv(fx, sp + 0x5A0, ((((u << 5) + 0x20) << 4) & 0x3FFF) as u32);
    uv(fx, sp + 0x5A2, ((((w << 5) + 0x20) << 4) & 0x3FFF) as u32);
}

/// The sprite at `p` (its corners about it at sp + 0x4F0 / 0x500).
fn place(fx: &mut Fx, sp: u32, p: u32) {
    fx.vu_add4(sp + 0x4F0, sp + 0x4D0, p);
    fx.vu_add4(sp + 0x500, sp + 0x4E0, p);
}

/// The bursts (D+0x1B70, 16 x 0x150): 16 sprites flung from the target
/// along random table vectors [F 0x1E013C8..0x1E0192C].
fn bursts(fx: &mut Fx, v: &V, sp: u32) {
    let n = fx.mem.i32(D);
    if n >= v.burst.0 && n < v.burst.1 {
        let r = D + 0x150 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(r + 0x1B70, 1);
        fx.mem.write_u32(r + 0x1B74, 1);
        const RAS: [[u32; 3]; 3] = [
            [0x01E0_144C, 0x01E0_1480, 0x01E0_14AC],
            [0x01E0_14DC, 0x01E0_1510, 0x01E0_153C],
            [0x01E0_156C, 0x01E0_15A0, 0x01E0_15CC],
        ];
        for s4 in 0..0x10u32 {
            let c = rem2(rnd(fx, v.ra(0x01E0_142C)), 8);
            fx.mem.set32(r + 4 * s4 + 0x1B78, c);
            for (k, ras) in RAS.iter().enumerate() {
                let a = rem2(rnd(fx, v.ra(ras[0])), 4);
                let t = VEC_TABLE + 0x180 * a as u32;
                let b = rem2(rnd(fx, v.ra(ras[1])), 0x20);
                let t = t.wrapping_add(12 * b as u32);
                let x = rnd(fx, v.ra(ras[2]));
                let (_, m) = fx.div(x, 3);
                let e = fx.mem.f(t.wrapping_add(4 * m as u32));
                fx.mem.setf(r + 0x10 * s4 + 0x1BC0 + 4 * k as u32, ee::mul(2.0, e));
            }
            fx.mem.write_u32(r + 0x10 * s4 + 0x1BCC, 0x3F80_0000);
        }
    }
    let d = sp + 0x578;
    sprite_desc(fx, d, 1, 0x1C, 5);
    camera(fx, sp);
    for k in 0..0x10u32 {
        let r = D + 0x150 * k;
        if fx.mem.read_u8(r + 0x1B70) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 0x1B74);
        let a = fx.mult(0x10 - age, 0x56) >> 4;
        rgba(fx, sp + 0x588, [0x80, v.spr_g, 0xB2], a as u8);
        let age = ee::from_i32(fx.mem.i32(r + 0x1B74));
        let s = ee::mul(18.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 16.0)));
        corners(fx, sp, s);
        let age = fx.mem.i32(r + 0x1B74);
        let f20 = fx.tan_falloff_p(age, 0x10, 1);
        for s6 in 0..0x10u32 {
            fx.vu_scale4(sp + 0x550, r + 0x10 * s6 + 0x1BC0, f20);
            fx.vu_add4(sp + 0x550, sp + 0x550, D + 0x30);
            place(fx, sp, sp + 0x550);
            let c = fx.mem.i32(r + 4 * s6 + 0x1B78);
            atlas(fx, sp, c);
            if crate::fxlib2::project(fx, sp + 0x4F0, PROJ_OUT, sp + 0x410, d, sp + 0x78C) {
                let w = fx.mem.f(sp + 0x78C);
                let key = fix(fx, w, v.ra(0x01E0_18D8));
                fx.submit(d, OT, key as i32);
            }
        }
        let age = fx.mem.i32(r + 0x1B74) + 1;
        fx.mem.set32(r + 0x1B74, age);
        if age >= 0x11 {
            fx.mem.write_u8(r + 0x1B70, 0);
        }
    }
}

/// The fountains (D+0x4710, 16 x 0x470): 2 x 32 sprites rising from the
/// walls' second ring points [F 0x1E01930..0x1E01DE4].
fn fountains(fx: &mut Fx, v: &V, spr: u32, sp: u32) {
    let n = fx.mem.i32(D);
    if n >= v.win2.0 && n < v.win2.1 {
        let r = D + 0x470 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(r + 0x4710, 1);
        fx.mem.write_u32(r + 0x4714, 1);
        for s7 in 0..2u32 {
            fx.vu_add4(r + 0x10 * s7 + 0x4760, D + 0x150 * s7 + 0x1890, D + 0x10 * s7 + 0x1B50);
            for s6 in 0..0x20u32 {
                let c = rem2(rnd(fx, v.ra(0x01E0_19C8)), 8);
                fx.mem.write_u8(r + 0x20 * s7 + s6 + 0x4718, c as u8);
                let p = r + 0x200 * s7 + 0x10 * s6 + 0x4780;
                fx.random_table_vec(p, 2.0);
                // fabsf's frame: the argument at its sp.
                let y = fx.mem.f(p + 4);
                fx.mem.setf(spr - 0x10, y);
                let y = f32::from_bits(y.to_bits() & 0x7FFF_FFFF);
                fx.mem.setf(p + 4, ee::mul(f(0x3F4C_CCCD), y));
            }
        }
    }
    let d = sp + 0x578;
    sprite_desc(fx, d, 1, 0x1C, 5);
    camera(fx, sp);
    for k in 0..0x10u32 {
        let r = D + 0x470 * k;
        if fx.mem.read_u8(r + 0x4710) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 0x4714);
        let a = ((0x10 - age) * 56) >> 4;
        rgba(fx, sp + 0x588, [0x80, v.spr_g, 0xB2], a as u8);
        let agef = ee::from_i32(fx.mem.i32(r + 0x4714));
        let s = ee::mul(10.0, ee::add(0.5, ee::div(ee::mul(0.5, agef), 16.0)));
        corners(fx, sp, s);
        let age = fx.mem.i32(r + 0x4714);
        let t = fx.tan_falloff_p(age, 0x10, 1);
        let f20 = ee::add(f(0x3E4C_CCCD), ee::mul(f(0x3F4C_CCCD), t));
        for s7 in 0..2u32 {
            for s6 in 0..0x20u32 {
                fx.vu_scale4(sp + 0x550, r + 0x200 * s7 + 0x10 * s6 + 0x4780, f20);
                fx.vu_add4(sp + 0x550, sp + 0x550, r + 0x10 * s7 + 0x4760);
                place(fx, sp, sp + 0x550);
                let c = i32::from(fx.mem.read_u8(r + 0x20 * s7 + s6 + 0x4718));
                atlas(fx, sp, c);
                if crate::fxlib2::project(fx, sp + 0x4F0, PROJ_OUT, sp + 0x410, d, sp + 0x78C) {
                    fx.submit(d, OT, 0);
                }
            }
        }
        let age = fx.mem.i32(r + 0x4714) + 1;
        fx.mem.set32(r + 0x4714, age);
        if age >= 0x11 {
            fx.mem.write_u8(r + 0x4710, 0);
        }
    }
}

/// The shards (D+0x8F50, 12 x 0x350): 2 x 20 spinning three-panel shards
/// rising along the walls' ring offsets [F 0x1E01DE8..0x1E023E4].
fn shards(fx: &mut Fx, v: &V, sp: u32) -> bool {
    let n = fx.mem.i32(D);
    if n >= v.win2.0 && n < v.win2.1 {
        let (_, k) = fx.div(n, 0xC);
        let r = D.wrapping_add(fx.mult(k, 0x350) as u32);
        fx.mem.write_u8(r + 0x8F50, 1);
        fx.mem.write_u32(r + 0x8F54, 0);
        for s7 in 0..2u32 {
            for s6 in 0..0x14u32 {
                let x = rnd(fx, v.ra(0x01E0_1E60));
                let (_, m) = fx.div(x, 3);
                fx.mem.write_u8(r + 0x14 * s7 + s6 + 0x8F58, (m + 5) as u8);
                let c = rem2(rnd(fx, v.ra(0x01E0_1E84)), 0x100);
                fx.mem.set32(r + 0x50 * s7 + 4 * s6 + 0x8F80, (c << 16) >> 8);
                let p = r + 0x140 * s7 + 0x10 * s6 + 0x9020;
                fx.random_table_vec(p, 0.5);
                let y = ee::mul(fx.mem.f(p + 4), 2.0);
                fx.mem.setf(p + 4, y);
            }
        }
    }
    let q = sp + 0x678;
    quad_desc(fx, q);
    alpha(fx, q, 1);
    let rh = fx.mem.read_u32(D + 0x1C);
    fx.desc_texture(q, rh, 1);
    fx.desc_clamp(q, 5);
    let mut any = false;
    for k in 0..0xCu32 {
        let r = D + 0x350 * k;
        if fx.mem.read_u8(r + 0x8F50) == 0 {
            continue;
        }
        any = true;
        let age = fx.mem.i32(r + 0x8F54);
        let a = div12(fx, (0xC - age).wrapping_mul(118));
        rgba(fx, sp + 0x688, [0x80, v.spr_g, 0xB2], a as u8);
        for o in [0x6A0u32, 0x6B8, 0x6D0] {
            copy8(fx, sp + o, sp + 0x688);
        }
        let agef = ee::from_i32(fx.mem.i32(r + 0x8F54));
        let s = ee::add(f(0x3DCC_CCCD), ee::div(ee::mul(f(0x3F66_6666), agef), 12.0));
        vec4(fx, sp + 0x560, s, s, s);
        for t in 0..2u32 {
            for s6 in 0..0x14u32 {
                fx.vu_add4(sp + 0x550, D + 0x150 * t + 0x10 * s6 + 0x1880, D + 0x10 * t + 0x1B50);
                fx.vu_add4(sp + 0x550, sp + 0x550, r + 0x140 * t + 0x10 * s6 + 0x9020);
                let y = ee::add(fx.mem.f(sp + 0x554), ee::div(ee::mul(130.0, ee::from_i32(s6 as i32)), 20.0));
                fx.mem.setf(sp + 0x554, y);
                fx.camera_vp_to(sp + 0x410);
                fx.identity(sp + 0x450);
                fx.translate_rows(sp + 0x450, sp + 0x450, sp + 0x550);
                let age = fx.mem.i32(r + 0x8F54);
                let spin = fx.mult(age, 0x3E7);
                let a = fx.mem.i32(r + 0x50 * t + 4 * s6 + 0x8F80).wrapping_add(spin) & 0xFFFF;
                fx.rotate_axis(sp + 0x450, a, 1);
                fx.scale_rows(sp + 0x450, sp + 0x560);
                crate::blib::exe::mul_mem(fx, sp + 0x410, sp + 0x450);
                let b = u32::from(fx.mem.read_u8(r + 0x14 * t + s6 + 0x8F58));
                for s2 in 0..3u32 {
                    let (s3, s5) = (D + 0x10 * b + 4 * s2, D + 8 * b);
                    for (o, src) in [(0x690u32, s3 + 0x8E10), (0x694, s5 + 0x8E90), (0x6A8, s3 + 0x8E14), (0x6AC, s5 + 0x8E90), (0x6C0, s3 + 0x8E10), (0x6C4, s5 + 0x8E94), (0x6D8, s3 + 0x8E14), (0x6DC, s5 + 0x8E94)] {
                        let x = fx.mem.read_u32(src);
                        fx.mem.write_u32(sp + o, x);
                    }
                    let (e0, e1) = (D + 0x20 * s2, D + 0x20 * (s2 + 1));
                    let vs = [e0 + 0x8ED0, e1 + 0x8ED0, e0 + 0x8EE0, e1 + 0x8EE0];
                    if fx.project_n(&vs, sp + 0x410, q, sp + 0x78C, v.ra(0x01E0_22B4)) == 1 {
                        let w = fx.mem.f(sp + 0x78C);
                        let key = fix(fx, w, v.ra(0x01E0_22C8));
                        fx.submit(q, OT, key as i32);
                    }
                }
            }
        }
        let age = fx.mem.i32(r + 0x8F54) + 1;
        fx.mem.set32(r + 0x8F54, age);
        if age >= 0xD {
            fx.mem.write_u8(r + 0x8F50, 0);
        }
    }
    any
}

/// The wave sheet (D+0x4704): the 17 x 17 grid rippled 7 sin about the
/// attacker's camera [F 0x1E023E8..0x1E02830].
fn wave(fx: &mut Fx, v: &V, sp: u32) -> bool {
    if fx.mem.i32(D) == v.wave {
        fx.mem.write_u8(D + 0x4704, 1);
        fx.mem.write_u32(D + 0x4708, 0);
    }
    if fx.mem.read_u8(D + 0x4704) != 1 {
        return false;
    }
    let d = sp + 0x708;
    head(fx, d, 0x24, 9, 0x4_2424_2421, 0);
    alpha(fx, d, 0);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 0);
    let age = ee::from_i32(fx.mem.i32(D + 0x4708));
    let f20 = Fx::ratio_below(age, 32.0);
    let age = ee::from_i32(fx.mem.i32(D + 0x4708));
    let f0 = Fx::ramp_above(age, v.wave_ramp.0, v.wave_ramp.1);
    let f1 = ee::mul(f20, f0);
    let f20 = ee::mul(100.0, f1);
    fx.camera_vp_to(sp + 0x410);
    fx.identity(sp + 0x450);
    let y = ee::sub(15.0, ee::mul(2.0, ee::mul(7.0, 0.0)));
    fx.mem.write_u32(sp + 0x550, 0);
    fx.mem.setf(sp + 0x554, y);
    fx.mem.write_u32(sp + 0x558, 0);
    fx.mem.write_u32(sp + 0x55C, 0x3F80_0000);
    fx.translate_rows(sp + 0x450, sp + 0x450, sp + 0x550);
    crate::blib::exe::mul_mem(fx, sp + 0x410, sp + 0x450);
    let a = fix(fx, f20, v.ra(0x01E0_2568)) & 0xFF;
    rgba(fx, sp + 0x718, [0x50, v.wave_g, 0xFF], a as u8);
    for s4 in 0..0x11u32 {
        for s5 in 0..0x11u32 {
            let age = fx.mem.i32(D + 0x4708);
            let m = fx.mult(age, 0x29A);
            let a = fx.mem.i32(D + 0x44 * s4 + 4 * s5 + 0x4280).wrapping_add(m) & 0xFFFF;
            let sn = fx.tsin(a);
            fx.mem.setf(D + 0x110 * s4 + 0x10 * s5 + 0x3074, ee::mul(7.0, sn));
        }
    }
    let age = fx.mem.i32(D + 0x4708);
    let ph = (rem2(age, 0x80) << 16) >> 7;
    let s2 = fx.mult(-1, ph) & 0xFFFF;
    let f22 = ee::mul(f(0x3E4C_CCCD), fx.tsin(s2));
    let f23 = ee::mul(f(0x3E4C_CCCD), fx.tcos(s2));
    for r in 0..0x10i32 {
        let f24 = ee::add(ee::div(ee::from_i32(rem2(r, 2)), 2.0), f23);
        let f25 = ee::add(ee::div(ee::from_i32(rem2(r, 2) + 1), 2.0), f23);
        for c in 0..0x10i32 {
            let s0 = ee::add(ee::div(ee::from_i32(rem2(c, 2)), 2.0), f22);
            let s1 = ee::add(ee::div(ee::from_i32(rem2(c, 2) + 1), 2.0), f22);
            for (o, x) in [(0x720u32, s0), (0x724, f24), (0x730, s1), (0x734, f24), (0x740, s0), (0x744, f25), (0x750, s1), (0x754, f25)] {
                fx.mem.setf(sp + o, x);
            }
            let g = |r: i32, c: i32| D + 0x110 * r as u32 + 0x10 * c as u32 + 0x3070;
            let vs = [g(r, c), g(r, c + 1), g(r + 1, c), g(r + 1, c + 1)];
            if fx.project_n(&vs, sp + 0x410, d, sp + 0x78C, v.ra(0x01E0_279C)) == 1 {
                let w = fx.mem.f(sp + 0x78C);
                let key = fix(fx, w, v.ra(0x01E0_27B0));
                fx.submit(d, OT, key as i32);
            }
        }
    }
    let age = fx.mem.i32(D + 0x4708) + 1;
    fx.mem.set32(D + 0x4708, age);
    if age >= v.wave_end {
        fx.mem.write_u8(D + 0x4704, 0);
    }
    true
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

/// The attacker drawn into its target with the callback and pulled
/// through 16 rings of quads and a fan (D+0xF910) [F 0x1E0287C..0x1E03498].
fn attacker_pass(fx: &mut Fx, v: &V, sp: u32, a3: u32) {
    if fx.mem.i32(D) == 0 {
        fx.mem.write_u8(D + 0xF910, 1);
        fx.mem.write_u32(D + 0xF914, 0);
    }
    if fx.mem.read_u8(D + 0xF910) != 1 {
        return;
    }
    let t = sp + 0x600;
    head(fx, t, 0x33, 9, 0x4_2142_1421, 0);
    let q = sp + 0x678;
    quad_desc(fx, q);
    let layer = fx.layer_begin();
    fx.texflush([0x2142_1421, t, 0x20, a3]);
    fx.mem.write_u32(v.d_static, D);
    let (cb, dst, half) = (v.cb, v.d_static, v.cb_half);
    let (_, regs) = fx.unit_draw_post(1, sp + 0x660, sp + 0x668, cb, &mut |fx, p| callback(fx, p, cb, dst, 0, half));
    copy8(fx, sp + 0x6F0, sp + 0x660);
    copy8(fx, sp + 0x6F8, sp + 0x668);
    fx.texflush([sp + 0x6F0, regs[1], regs[2], regs[3]]);
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
        let f21 = ee::div(ee::mul(v.rr, ee::from_i32(0x10 - k)), 16.0);
        let ak = (k.wrapping_mul(v.ra_k)) >> 4;
        let bk = (k << 17) >> 4;
        for s5 in 0..0x21i32 {
            let age = fx.mem.i32(D + 0xF914);
            let m = fx.mult(age, v.ra_age);
            let a = (m.wrapping_add(ak + (s5.wrapping_mul(v.ra_s) >> 5))) & 0xFFFF;
            let f20 = ee::add(1.0, ee::mul(0.5, fx.tsin(a)));
            let age = fx.mem.i32(D + 0xF914);
            let m = fx.mult(age, v.rb_age);
            let b = bk.wrapping_add(m) & 0xFFFF;
            let f1 = ee::mul(v.ramp, fx.tsin(b));
            let ang = fixs(fx, ee::add(ee::from_i32((s5 << 16) >> 5), f1), v.ra(0x01E0_2B10)) & 0xFFFF;
            let f20 = ee::mul(f21, f20);
            let p = rb + 0x10 * s5 as u32 + 0xD810;
            let sn = fx.tsin(ang);
            fx.mem.setf(p, ee::mul(f20, sn));
            let cs = fx.tcos(ang);
            fx.mem.setf(p + 4, ee::mul(f20, cs));
            fx.mem.write_u32(p + 8, 0);
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        }
    }
    for k in 0..3u32 {
        fx.mem.write_u32(sp + 0x550 + 4 * k, 0);
    }
    fx.mem.write_u32(sp + 0x55C, 0x3F80_0000);
    let age = fx.mem.i32(D + 0xF914);
    let c = ee::from_i32(Fx::clamp_i(age, 0, v.rclamp));
    let age = fx.mem.i32(D + 0xF914);
    let c2 = Fx::clamp_i(0x3200 - age, 0, v.rclamp);
    let f20 = ee::div(ee::mul(c, ee::from_i32(c2)), v.rdiv);
    for k in 0..0x10i32 {
        let rb = D + 0x210 * k as u32;
        if k == 0xF {
            let x = div17(fx, 120 * k);
            let a = fix(fx, ee::mul(f20, ee::from_i32(x)), v.ra(0x01E0_2C98));
            rgba(fx, sp + 0x610, v.rcol, a as u8);
            let x = div17(fx, 120 * (k - 1));
            let a = fix(fx, ee::mul(f20, ee::from_i32(x)), v.ra(0x01E0_2D08));
            rgba(fx, sp + 0x628, v.rcol, a as u8);
            copy8(fx, sp + 0x640, sp + 0x628);
            for s2 in 0..0x20u32 {
                let (p0, p1) = (rb + 0x10 * s2, rb + 0x10 * (s2 + 1));
                if fx.project_n(&[sp + 0x550, p0 + 0xD810, p1 + 0xD810], sp + 0x410, t, sp + 0x78C, v.ra(0x01E0_2D84)) != 1 {
                    continue;
                }
                for (u, x) in [(0x618u32, 0x620u32), (0x630, 0x638), (0x648, 0x650)] {
                    fx.uv_from_xy(sp + u, sp + x, 0, 0);
                }
                if fx.project_n(&[sp + 0x550, p0 + 0xB710, p1 + 0xB710], sp + 0x410, t, sp + 0x78C, v.ra(0x01E0_2DF0)) == 1 {
                    let z = fx.mem.read_u32(D + 0x44);
                    set_z(fx, sp + 0x620, z);
                    copy_z(fx, sp + 0x638, sp + 0x620);
                    copy_z(fx, sp + 0x650, sp + 0x620);
                    fx.submit(t, 0, 0);
                }
            }
        } else {
            let x = div17(fx, 120 * k);
            let a = fix(fx, ee::mul(f20, ee::from_i32(x)), v.ra(0x01E0_2EF8));
            rgba(fx, sp + 0x688, v.rcol, a as u8);
            copy8(fx, sp + 0x6A0, sp + 0x688);
            let x = div17(fx, 120 * (k + 1));
            let a = fix(fx, ee::mul(f20, ee::from_i32(x)), v.ra(0x01E0_2F78));
            rgba(fx, sp + 0x6B8, v.rcol, a as u8);
            copy8(fx, sp + 0x6D0, sp + 0x6B8);
            let rn = rb + 0x210;
            for s6 in 0..0x20u32 {
                let c = |base: u32| [rb + 0x10 * s6 + base, rb + 0x10 * (s6 + 1) + base, rn + 0x10 * s6 + base, rn + 0x10 * (s6 + 1) + base];
                if fx.project_n(&c(0xD810), sp + 0x410, q, sp + 0x78C, v.ra(0x01E0_3028)) != 1 {
                    continue;
                }
                for (u, x) in [(0x690u32, 0x698u32), (0x6A8, 0x6B0), (0x6C0, 0x6C8), (0x6D8, 0x6E0)] {
                    fx.uv_from_xy(sp + u, sp + x, 0, 0);
                }
                if fx.project_n(&c(0xB710), sp + 0x410, q, sp + 0x78C, v.ra(0x01E0_30B4)) == 1 {
                    let z = fx.mem.read_u32(D + 0x44);
                    set_z(fx, sp + 0x698, z);
                    for o in [0x6B0u32, 0x6C8, 0x6E0] {
                        copy_z(fx, sp + o, sp + 0x698);
                    }
                    fx.submit(q, 0, 0);
                }
            }
        }
    }
    fx.layer_end(layer, OT, 0);
    let d = sp + 0x578;
    sprite_desc(fx, d, 1, 0x10, 0);
    camera_at(fx, sp);
    let age = fx.mem.i32(D + 0xF914);
    let s = ee::add(f(0x3F4C_CCCD), ee::mul(f(0x3E4C_CCCD), ee::from_i32(rem2(age, 2))));
    let a = fix(fx, ee::mul(ee::mul(200.0, f20), s), v.ra(0x01E0_32F4));
    rgba(fx, sp + 0x588, v.rcol, a as u8);
    uv(fx, sp + 0x590, 0x600);
    uv(fx, sp + 0x592, 0);
    uv(fx, sp + 0x5A0, 0x800);
    uv(fx, sp + 0x5A2, 0x200);
    vec4(fx, sp + 0x4D0, -40.0, -40.0, 0.0);
    fx.mem.write_u32(sp + 0x4D8, 0);
    vec4(fx, sp + 0x4E0, 40.0, 40.0, 0.0);
    fx.mem.write_u32(sp + 0x4E8, 0);
    if crate::fxlib2::project(fx, sp + 0x4D0, PROJ_OUT, sp + 0x410, d, sp + 0x78C) {
        far(fx, sp + 0x598, &[sp + 0x5A8]);
        fx.submit(d, OT, 0);
    }
    let age = fx.mem.i32(D + 0xF914) + 1;
    fx.mem.set32(D + 0xF914, age);
    if age >= 0x3201 {
        fx.mem.write_u8(D + 0xF910, 0);
    }
}

/// The frame-textured window sprite while clamp(n - lo, 0, 0x20) clamp(hi
/// - n, 0, 0x20) / 1024 > 0 [F 0x1E0349C..0x1E03694].
fn window(fx: &mut Fx, v: &V, sp: u32) {
    let n = fx.mem.i32(D);
    let a = ee::from_i32(Fx::clamp_i(n - v.win.0, 0, 0x20));
    let n = fx.mem.i32(D);
    let b = ee::from_i32(Fx::clamp_i(v.win.1 - n, 0, 0x20));
    let f20 = ee::div(ee::mul(a, b), 1024.0);
    if crate::exe::ee_le(f20, 0.0) {
        return;
    }
    let d = sp + 0x578;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    let flags = u32::from(fx.mem.read_u16(d + 2));
    alpha(fx, d, 0);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.desc_clamp(d, 5);
    uv(fx, sp + 0x590, 0x140);
    uv(fx, sp + 0x592, 0x140);
    uv(fx, sp + 0x5A0, 0x1EC0);
    uv(fx, sp + 0x5A2, 0x1EC0);
    let a = fix(fx, ee::mul(64.0, f20), v.ra(0x01E0_35F8));
    rgba(fx, sp + 0x588, [0x80; 3], a as u8);
    far_xy(fx, sp + 0x598, 0x7000, 0x7000);
    far_xy(fx, sp + 0x5A8, 0x9000, 0x9000);
    fx.submit(d, OT, 0);
}

/// The post callback (b222 0x1E03760 / b765 0x1E03790)(p) [F]: run by
/// 0x1F0FD0 after the attacker's render into its target. FRAME_1 masked
/// (0x2096C0 0xFF000000), a dark full-window veil (0, 0, 0, 80) at ALPHA
/// preset 0, a (FF, FF, FF, 80) sprite (texture D+0x18, half size 20 /
/// 10) at the attacker's point under the camera turn, its V sliding with
/// the update (the UV registers' upper words carry floats), far Z,
/// buffered; FRAME_1 unmasked. The sprite's TEX0 / TEX1 slots (sp + 0x180
/// / 0x188) are not written when the texture is missing: they hold
/// 0x209800's save of 0x1F0FD0's s1 - the callback's address `cb` and
/// `hi`; `dst` the static D.
fn callback(fx: &mut Fx, p: u32, cb: u32, dst: u32, hi: u64, half: f32) -> u32 {
    let sp = fx.enter(0x1A0);
    fx.mem.write_u64(sp + 0x180, u64::from(cb));
    fx.mem.write_u64(sp + 0x188, hi);
    let s0 = frame_mask(fx, p, 0xFF00_0000, [0, 0]);
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
    let dd = fx.mem.read_u32(dst);
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
    vec4(fx, sp + 0xF0, ee::neg(half), ee::neg(half), 0.0);
    fx.mem.write_u32(sp + 0xF8, 0);
    vec4(fx, sp + 0x100, half, half, 0.0);
    fx.mem.write_u32(sp + 0x108, 0);
    let mut a23 = [sp + 0x30, d];
    if crate::fxlib2::project(fx, sp + 0xF0, PROJ_OUT, sp + 0x30, d, sp + 0x19C) {
        far(fx, sp + 0x160, &[sp + 0x170]);
        s1 = fx.submit_at(s1, d, 0, 0);
        a23 = [0, 0];
    }
    let r = frame_mask(fx, s1, 0, a23);
    fx.leave(0x1A0);
    r
}

fn frame_mask(fx: &mut Fx, p: u32, mask: u32, a23: [u32; 2]) -> u32 {
    crate::b440::frame_mask_packet(fx, p, mask, a23)
}

const RA612: &[(u32, u32)] = &[
    (0x01E0_0890, 0x01E0_0894), (0x01E0_08C4, 0x01E0_08C8), (0x01E0_0938, 0x01E0_0994), (0x01E0_09C4, 0x01E0_0A24), (0x01E0_09D8, 0x01E0_0A38), (0x01E0_0A24, 0x01E0_0A84),
    (0x01E0_0A34, 0x01E0_0A94), (0x01E0_0A58, 0x01E0_0AB8), (0x01E0_0ABC, 0x01E0_0B1C), (0x01E0_0AC8, 0x01E0_0B28), (0x01E0_0AD8, 0x01E0_0B38), (0x01E0_0AE4, 0x01E0_0B44),
    (0x01E0_0B8C, 0x01E0_0BEC), (0x01E0_0BA0, 0x01E0_0C00), (0x01E0_0BDC, 0x01E0_0C3C), (0x01E0_0BF4, 0x01E0_0C54), (0x01E0_0C00, 0x01E0_0C60), (0x01E0_0C74, 0x01E0_0CD4),
    (0x01E0_0DF0, 0x01E0_0E50), (0x01E0_0E04, 0x01E0_0E64), (0x01E0_0E14, 0x01E0_0E74), (0x01E0_0EB0, 0x01E0_0F10), (0x01E0_0EB8, 0x01E0_0F18), (0x01E0_0EC0, 0x01E0_0F20),
    (0x01E0_0ED4, 0x01E0_0F34), (0x01E0_0EDC, 0x01E0_0F3C), (0x01E0_0EE4, 0x01E0_0F44), (0x01E0_0EF0, 0x01E0_0F50), (0x01E0_0F0C, 0x01E0_0F6C), (0x01E0_0FC4, 0x01E0_1024),
    (0x01E0_0FD8, 0x01E0_1038), (0x01E0_1018, 0x01E0_1078), (0x01E0_1030, 0x01E0_1090), (0x01E0_103C, 0x01E0_109C), (0x01E0_1080, 0x01E0_10E0), (0x01E0_10D4, 0x01E0_1134),
    (0x01E0_10E8, 0x01E0_1148), (0x01E0_10F8, 0x01E0_1158), (0x01E0_1144, 0x01E0_11A4), (0x01E0_1154, 0x01E0_11B4), (0x01E0_118C, 0x01E0_11EC), (0x01E0_11C8, 0x01E0_1228),
    (0x01E0_1270, 0x01E0_12D0), (0x01E0_1290, 0x01E0_12F0), (0x01E0_12A4, 0x01E0_1304), (0x01E0_12B8, 0x01E0_1318), (0x01E0_12CC, 0x01E0_132C), (0x01E0_12EC, 0x01E0_134C),
    (0x01E0_1308, 0x01E0_1368), (0x01E0_1324, 0x01E0_1384), (0x01E0_13A8, 0x01E0_1408), (0x01E0_142C, 0x01E0_148C), (0x01E0_144C, 0x01E0_14AC), (0x01E0_1480, 0x01E0_14E0),
    (0x01E0_14AC, 0x01E0_150C), (0x01E0_14DC, 0x01E0_153C), (0x01E0_1510, 0x01E0_1570), (0x01E0_153C, 0x01E0_159C), (0x01E0_156C, 0x01E0_15CC), (0x01E0_15A0, 0x01E0_1600),
    (0x01E0_15CC, 0x01E0_162C), (0x01E0_1668, 0x01E0_16C8), (0x01E0_1674, 0x01E0_16D4), (0x01E0_1684, 0x01E0_16E4), (0x01E0_1690, 0x01E0_16F0), (0x01E0_1698, 0x01E0_16F8),
    (0x01E0_16A0, 0x01E0_1700), (0x01E0_16AC, 0x01E0_170C), (0x01E0_1788, 0x01E0_17E8), (0x01E0_1798, 0x01E0_17F8), (0x01E0_17A8, 0x01E0_1808), (0x01E0_17C8, 0x01E0_1828),
    (0x01E0_17D8, 0x01E0_1838), (0x01E0_17E8, 0x01E0_1848), (0x01E0_17F8, 0x01E0_1858), (0x01E0_18C4, 0x01E0_1924), (0x01E0_18D8, 0x01E0_1938), (0x01E0_18E8, 0x01E0_1948),
    (0x01E0_19B4, 0x01E0_1A14), (0x01E0_19C8, 0x01E0_1A28), (0x01E0_19F4, 0x01E0_1A54), (0x01E0_19FC, 0x01E0_1A5C), (0x01E0_1AAC, 0x01E0_1B0C), (0x01E0_1AB8, 0x01E0_1B18),
    (0x01E0_1AC8, 0x01E0_1B28), (0x01E0_1AD4, 0x01E0_1B34), (0x01E0_1ADC, 0x01E0_1B3C), (0x01E0_1AE4, 0x01E0_1B44), (0x01E0_1AF0, 0x01E0_1B50), (0x01E0_1BC0, 0x01E0_1C20),
    (0x01E0_1BD0, 0x01E0_1C30), (0x01E0_1BE4, 0x01E0_1C44), (0x01E0_1C40, 0x01E0_1CA0), (0x01E0_1C50, 0x01E0_1CB0), (0x01E0_1C60, 0x01E0_1CC0), (0x01E0_1C70, 0x01E0_1CD0),
    (0x01E0_1D3C, 0x01E0_1D9C), (0x01E0_1D58, 0x01E0_1DB8), (0x01E0_1E60, 0x01E0_1EC0), (0x01E0_1E84, 0x01E0_1EE4), (0x01E0_1EC4, 0x01E0_1F24), (0x01E0_1F70, 0x01E0_1FD0),
    (0x01E0_1F7C, 0x01E0_1FDC), (0x01E0_1F8C, 0x01E0_1FEC), (0x01E0_1F98, 0x01E0_1FF8), (0x01E0_210C, 0x01E0_216C), (0x01E0_2124, 0x01E0_2184), (0x01E0_2168, 0x01E0_21C8),
    (0x01E0_2170, 0x01E0_21D0), (0x01E0_2180, 0x01E0_21E0), (0x01E0_21B8, 0x01E0_2218), (0x01E0_21C4, 0x01E0_2224), (0x01E0_21D0, 0x01E0_2230), (0x01E0_22B4, 0x01E0_2314),
    (0x01E0_22C8, 0x01E0_2328), (0x01E0_22D8, 0x01E0_2338), (0x01E0_2448, 0x01E0_24AC), (0x01E0_2454, 0x01E0_24B8), (0x01E0_2464, 0x01E0_24C8), (0x01E0_2470, 0x01E0_24D4),
    (0x01E0_2484, 0x01E0_24E8), (0x01E0_24A4, 0x01E0_2510), (0x01E0_24DC, 0x01E0_2548), (0x01E0_24E4, 0x01E0_2550), (0x01E0_2548, 0x01E0_25B4), (0x01E0_2554, 0x01E0_25C0),
    (0x01E0_2568, 0x01E0_25D4), (0x01E0_257C, 0x01E0_25E8), (0x01E0_25E0, 0x01E0_2650), (0x01E0_2670, 0x01E0_26E0), (0x01E0_2688, 0x01E0_26F8), (0x01E0_279C, 0x01E0_280C),
    (0x01E0_27B0, 0x01E0_2820), (0x01E0_27C0, 0x01E0_2830), (0x01E0_284C, 0x01E0_28BC), (0x01E0_2860, 0x01E0_28D0), (0x01E0_287C, 0x01E0_28EC), (0x01E0_2928, 0x01E0_2998),
    (0x01E0_2930, 0x01E0_29A0), (0x01E0_295C, 0x01E0_29CC), (0x01E0_2978, 0x01E0_29E8), (0x01E0_2980, 0x01E0_29F0), (0x01E0_2990, 0x01E0_2A00), (0x01E0_299C, 0x01E0_2A0C),
    (0x01E0_29A4, 0x01E0_2A14), (0x01E0_29B0, 0x01E0_2A20), (0x01E0_29CC, 0x01E0_2A3C), (0x01E0_29E0, 0x01E0_2A50), (0x01E0_29EC, 0x01E0_2A5C), (0x01E0_2A08, 0x01E0_2A78),
    (0x01E0_2AA8, 0x01E0_2B00), (0x01E0_2AE4, 0x01E0_2B3C), (0x01E0_2B10, 0x01E0_2B5C), (0x01E0_2B1C, 0x01E0_2B68), (0x01E0_2B38, 0x01E0_2B84), (0x01E0_2BF8, 0x01E0_2C20),
    (0x01E0_2C20, 0x01E0_2C48), (0x01E0_2C98, 0x01E0_2CC0), (0x01E0_2D08, 0x01E0_2D30), (0x01E0_2D84, 0x01E0_2DAC), (0x01E0_2DA4, 0x01E0_2DCC), (0x01E0_2DB8, 0x01E0_2DE0),
    (0x01E0_2DCC, 0x01E0_2DF4), (0x01E0_2DF0, 0x01E0_2E18), (0x01E0_2EA4, 0x01E0_2ECC), (0x01E0_2EF8, 0x01E0_2F20), (0x01E0_2F78, 0x01E0_2FA0), (0x01E0_3028, 0x01E0_3050),
    (0x01E0_3048, 0x01E0_3070), (0x01E0_305C, 0x01E0_3084), (0x01E0_3070, 0x01E0_3098), (0x01E0_3084, 0x01E0_30AC), (0x01E0_30B4, 0x01E0_30DC), (0x01E0_3198, 0x01E0_31C0),
    (0x01E0_31F4, 0x01E0_321C), (0x01E0_3240, 0x01E0_3268), (0x01E0_324C, 0x01E0_3274), (0x01E0_325C, 0x01E0_3284), (0x01E0_3268, 0x01E0_3290), (0x01E0_3270, 0x01E0_3298),
    (0x01E0_3280, 0x01E0_32A8), (0x01E0_328C, 0x01E0_32B4), (0x01E0_32F4, 0x01E0_331C), (0x01E0_33D8, 0x01E0_3400), (0x01E0_345C, 0x01E0_3484), (0x01E0_34B0, 0x01E0_34D8),
    (0x01E0_34D0, 0x01E0_34F8), (0x01E0_3558, 0x01E0_3580), (0x01E0_3564, 0x01E0_358C), (0x01E0_3570, 0x01E0_3598), (0x01E0_357C, 0x01E0_35A4), (0x01E0_35F8, 0x01E0_3620),
    (0x01E0_3698, 0x01E0_36C0), (0x01E0_3780, 0x01E0_37B0), (0x01E0_378C, 0x01E0_37BC), (0x01E0_37B4, 0x01E0_37E4), (0x01E0_37C0, 0x01E0_37F0), (0x01E0_3864, 0x01E0_3894),
    (0x01E0_3894, 0x01E0_38C4), (0x01E0_38A0, 0x01E0_38D0), (0x01E0_38B0, 0x01E0_38E0), (0x01E0_38BC, 0x01E0_38EC), (0x01E0_38C4, 0x01E0_38F4), (0x01E0_38D4, 0x01E0_3904),
    (0x01E0_38E0, 0x01E0_3910), (0x01E0_39AC, 0x01E0_39DC), (0x01E0_3A34, 0x01E0_3A64), (0x01E0_3A44, 0x01E0_3A74),
];
