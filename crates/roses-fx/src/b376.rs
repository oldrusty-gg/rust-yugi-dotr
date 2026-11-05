//! BMODULE b376 (card 339), hand-ported from the record (linked at
//! 0x1E00000, the entry 0x1E00080 in a 0x760 frame with D in s0 and the OT
//! in s1, data area a0 = 0x1ED0000, 0xDE00 bytes; one attacker post callback
//! 0x1E03E70 with its static D at 0x1E04180) [F].
//! b725 (card 16, b725.rs) at 0.957, written from b725.rs by rule
//! (the project notes: locals from sp + 0x380 0x10 higher, D offsets from
//! 0x60 0x10 higher, code addresses by the 16 -> 339 alignment) and its
//! hunks (the project notes): the node 0x1F point is taken every update
//! into D+0x60 and copied to D+0x50 (the effect's origin) at 0x4E; the
//! timeline about 0x34 earlier (flash / walls / window from 0x4E, wall
//! height falloff 0x20, fade from 0xAA, end 0xAB; bursts 0x6F..0xF7 (alpha
//! (16 - age) 64 >> 4); fountains / shards 0x4F..0xF7 (fountain alpha (16 -
//! age) 26 >> 4); the attacker pass from 0x8, its layer below 0x4E, level to
//! 0xF8, glow to 0xF9; window 0x4E..0x128 with its UVs offset by two rand % 3
//! - 1 (u / v); stage light from 0x128; motes 0x6F..0x10D); a pool of 32 x
//! 0xA0 sprite records (D+0xCA00) spawned on 1..0x45 at the node point with
//! eight 0.8 table vectors and atlas cells rand % 8, drawn after the motes
//! (texture D+0x14, keyed by w), whose last callee leaves the outcome
//! call's arguments; the outcome fade from 0x7F (4 clamp(n - 0x7E, 0,
//! 0x20)); returns 1 / 6 / 2 / 8 on 0x64 / 0x6F..0x107 / 0x108 / 0x147.
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
const FRAME: u32 = 0x760;
const CB: u32 = 0x01E0_3E70;
/// The sprite pool (32 x 0xA0 records: flag, age, eight cells, point,
/// eight vectors).
const POOL: u32 = 0xCA00;
const D_STATIC: u32 = 0x01E0_4180;

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

/// What b726 (card 17) changes in b725 (card 16) [F: the two listings
/// align at 0.998].
pub struct V {
    /// The attacker node (D+0x48).
    node: i32,
    /// The ring offsets' radius at full size.
    ring: f32,
    walls: [u8; 3],
    lens: [u8; 4],
    tips: [u8; 3],
    bursts: [u8; 3],
    fountains: [u8; 3],
    shards: [u8; 3],
    pass: [u8; 3],
    glow: [u8; 3],
    window: [u8; 3],
    mote_amp: f32,
    /// b726: the motes keyed by their mean w (b725: key 0).
    mote_keyed: bool,
    /// The fade's 0x1F77A0 return address.
    fade_ra: u32,
}

pub const B376: V = V {
    node: 0x1F,
    ring: 12.0,
    walls: [0xFF, 0x58, 0x41],
    lens: [0x8A, 0x87, 0x80, 0x80],
    tips: [0xFF, 0x76, 0x4B],
    bursts: [0xFF, 0x8A, 0x4B],
    fountains: [0xFF, 0x8A, 0x4B],
    shards: [0xFF, 0x8A, 0x4B],
    pass: [0xFF, 0x20, 0x0C],
    glow: [0xFF, 0x80, 0x0C],
    window: [0xBC, 0x9E, 0x62],
    mote_amp: 96.0,
    mote_keyed: false,
    fade_ra: 0x01E0_3DA4,
};

/// The entry 0x1E00080 of b376 (card 339).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B376)
}

fn run(fx: &mut Fx, param: i32, v: &V) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xC0, 0);
    let ret = if param >= 0 { init(fx, v) } else { step(fx, sp, v) };
    fx.leave(FRAME);
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
    fx.bzero(D, 0xDE00);
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
    let h = fx.mem.i32(D + 4);
    let s = fx.request_node(h, v.node, 1);
    fx.mem.set32(D + 0x48, s);
    fx.mem.write_u8(D + 0x70, 0);
    fx.mem.write_u8(D + 0x1BA0, 0);
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x1BF0 + 0x150 * k, 0);
    }
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x30F0 + 0x250 * k, 0);
    }
    // The shard texels: eight cells of four S and two T.
    for a2 in 0..8i32 {
        let f2 = ee::from_i32(rem2(a2, 4) << 5);
        for a3 in 0..4i32 {
            let s = ee::div(ee::add(f2, ee::div(ee::mul(32.0, ee::from_i32(a3)), 3.0)), 128.0);
            fx.mem.setf(D + 0x55F0 + 0x10 * a2 as u32 + 4 * a3 as u32, s);
        }
        let f2 = ee::from_i32((a2 >> 2) << 5);
        fx.mem.setf(D + 0x5670 + 8 * a2 as u32, ee::div(f2, 64.0));
        fx.mem.setf(D + 0x5674 + 8 * a2 as u32, ee::div(ee::add(32.0, f2), 64.0));
    }
    // The shard edges: top / bottom pairs a third of a turn apart.
    let mut s0 = 0i32;
    for s3 in 0..4i32 {
        let r = D + 0x20 * s3 as u32;
        fx.mult(0x5555_5556, s0);
        let s4 = fx.hilo().1.wrapping_add(((s0 as u32) >> 31) as i32);
        let sn = fx.tsin(s4);
        fx.mem.setf(r + 0x56B0, ee::mul(15.0, sn));
        fx.mem.write_u32(r + 0x56B4, 0x4120_0000);
        let cs = fx.tcos(s4);
        fx.mem.setf(r + 0x56B8, ee::mul(15.0, cs));
        fx.mem.write_u32(r + 0x56BC, 0x3F80_0000);
        for w in 0..4u32 {
            let x = fx.mem.read_u32(r + 0x56B0 + 4 * w);
            fx.mem.write_u32(r + 0x56C0 + 4 * w, x);
        }
        let f1 = ee::div(ee::mul(-30.0, ee::from_i32(s3)), 3.0);
        let y = ee::mul(fx.mem.f(r + 0x56C4), -1.0);
        fx.mem.setf(r + 0x56C4, y);
        let f1 = ee::add(15.0, f1);
        let y = ee::add(fx.mem.f(r + 0x56B4), f1);
        fx.mem.setf(r + 0x56B4, y);
        let y = ee::add(fx.mem.f(r + 0x56C4), f1);
        fx.mem.setf(r + 0x56C4, y);
        s0 = s0.wrapping_add(0x8000);
    }
    for k in 0..0xCu32 {
        fx.mem.write_u8(D + 0x5730 + 0x350 * k, 0);
    }
    // The attacker rings.
    for s4 in 0..0x10i32 {
        let f20 = ee::div(ee::mul(40.0, ee::from_i32(0x10 - s4)), 16.0);
        for s5 in 0..0x21i32 {
            let a = (s5 << 16) >> 5;
            let p = D + 0x210 * s4 as u32 + 0x10 * s5 as u32 + 0x7EF0;
            let sn = fx.tsin(a);
            fx.mem.setf(p, ee::mul(f20, sn));
            let cs = fx.tcos(a);
            fx.mem.setf(p + 4, ee::mul(f20, cs));
            fx.mem.write_u32(p + 8, 0);
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        }
    }
    fx.mem.write_u8(D + 0xC0F0, 0);
    for k in 0..0x18u32 {
        fx.mem.write_u8(D + 0xC100 + 0x60 * k, 0);
    }
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + POOL + 0xA0 * k, 0);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_add_mode(t, 2, 0, [u64::from(D), u64::from(OT)], 0x01E0_04AC);
    0
}

fn step(fx: &mut Fx, sp: u32, v: &V) -> i32 {
    fx.camera_turn_roll(sp + 0x4E0);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x20);
    camera(fx, sp);
    fx.mem.write_u32(sp + 0x5A0, 0);
    fx.mem.write_u32(sp + 0x5A4, 0);
    let r = fx.mem.f(D + 0x40);
    fx.mem.setf(sp + 0x5A8, ee::neg(r));
    fx.mem.write_u32(sp + 0x5AC, 0x3F80_0000);
    fx.vu_apply_to(sp + 0x5A0, sp + 0x4E0, sp + 0x5A0);
    fx.vu_add4(sp + 0x5A0, sp + 0x5A0, D + 0x20);
    let m = fx.read_m4(sp + 0x460);
    fx.project_at(sp + 0x5A0, &m, sp + 0x750, sp + 0x758);
    let z = (fx.mem.read_u64(sp + 0x750) << 8) >> 40;
    fx.mem.write_u32(D + 0x44, z as u32);
    // The node 0x1F point (D+0x60) every update, copied to D+0x50 at 0x4E;
    // every update the turn D+0x80 pointing from D+0x50 to the target
    // (0x100E40, then 0x4000 about x) and the reach D+0xC0 = |target -
    // D+0x50| [F 0x1E0055C..0x1E00624].
    let h = fx.mem.i32(D + 4);
    let s = fx.mem.i32(D + 0x48);
    fx.node_position(h, s, D + 0x60);
    if fx.mem.i32(D) == 0x4E {
        for w in 0..4u32 {
            let x = fx.mem.read_u32(D + 0x60 + 4 * w);
            fx.mem.write_u32(D + 0x50 + 4 * w, x);
        }
    }
    fx.vu_sub4(sp + 0x5A0, D + 0x30, D + 0x50);
    fx.point_along(D + 0x80, sp + 0x5A0);
    fx.rotate_axis(D + 0x80, 0x4000, 0);
    let pv = fx.mem.f(sp + 0x5A0);
    let x = fx.powf(pv, 2.0);
    let pv = fx.mem.f(sp + 0x5A4);
    let y = fx.powf(pv, 2.0);
    let f20 = ee::add(x, y);
    let pv = fx.mem.f(sp + 0x5A8);
    let z = fx.powf(pv, 2.0);
    let l = fx.sqrtf(ee::add(z, f20));
    fx.mem.setf(D + 0xC0, l);

    flash(fx, sp);
    walls(fx, sp, v);
    bursts(fx, sp, v);
    fountains(fx, sp, v);
    // a3 at the attacker pass's first TEXFLUSH: the last projection's
    // descriptor (0x205210's untouched argument), else 0x208400's d + 2.
    let a3 = if shards(fx, sp, v) { sp + 0x6C0 } else { sp + 0x6C2 };

    let n = fx.mem.i32(D);
    let s1 = Fx::clamp_i(0x128 - n, 0, 0x20);
    let c = Fx::clamp_i(n, 0, 0x20);
    let l = (fx.mult(s1, c.wrapping_neg().wrapping_mul(80)) >> 10) + 0x80;
    fx.set_stage_light(l & 0xFF);

    attacker_pass(fx, sp, a3, v);
    window(fx, sp, v);
    motes(fx, sp, v);
    let args = pool(fx, sp);
    // The fade of a destroyed target (0x1F77A0(D+8, 2, (80, 80, 80, 4
    // clamp(n - 0xD4, 0, 0x20)))) from 0xD5 [F 0x1E03648..0x1E036A4].
    if fx.outcome(args) == 1 && fx.mem.i32(D) >= 0x7F {
        let n = fx.mem.i32(D);
        let c = Fx::clamp_i(n - 0x7E, 0, 0x20);
        let a = (c << 7) >> 5;
        for k in 0..3u32 {
            fx.mem.write_u8(sp + 0x75C + k, 0x80);
        }
        fx.mem.write_u8(sp + 0x75F, a as u8);
        let t = fx.mem.i32(D + 8);
        fx.unit_add_mode(t, 2, sp + 0x75C, [u64::from(D), u64::from(OT)], v.fade_ra);
    }

    let n = fx.mem.i32(D);
    let mut ret = fx.mem.i32(sp + 0xC0);
    if n == 0x64 {
        ret = 1;
    } else {
        if n >= 0x6F && n < 0x108 {
            ret = 6;
        }
        if n == 0x108 {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x147 {
        ret = 8;
    }
    fx.mem.set32(sp + 0xC0, ret);
    ret
}

/// The camera's view-projection at sp + 0x4D0 times the identity at sp +
/// 0x510.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x460);
    fx.identity(sp + 0x4A0);
    crate::blib::exe::mul_mem(fx, sp + 0x460, sp + 0x4A0);
}

/// The camera at the attacker's point under the camera turn (sp + 0x550).
fn camera_at(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x460);
    fx.translate_rows(sp + 0x4A0, sp + 0x4E0, D + 0x20);
    crate::blib::exe::mul_mem(fx, sp + 0x460, sp + 0x4A0);
}

/// The camera in the turn D+0x70 moved to `t` (0x188420).
fn camera_turn(fx: &mut Fx, sp: u32, t: u32) {
    fx.camera_vp_to(sp + 0x460);
    fx.translate_rows(sp + 0x4A0, D + 0x80, t);
    crate::blib::exe::mul_mem(fx, sp + 0x460, sp + 0x4A0);
}

/// A full-window sprite at `d` (the XYZ2s at +0x18 / +0x20).
fn window_xy(fx: &mut Fx, d: u32) {
    far_xy(fx, d + 0x18, 0x7000, 0x7000);
    far_xy(fx, d + 0x20, 0x9000, 0x9000);
}

/// The white flash (D+0x60) from 0x82 [F 0x1E005D0..0x1E0072C].
fn flash(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x4E {
        fx.mem.write_u8(D + 0x70, 1);
        fx.mem.write_u32(D + 0x74, 0);
        fx.mem.write_u8(D + 0xC0F0, 0);
    }
    if fx.mem.read_u8(D + 0x70) != 1 {
        return;
    }
    let d = sp + 0x618;
    head(fx, d, 6, 3, 0x441, 0);
    alpha(fx, d, 1);
    let age = fx.mem.i32(D + 0x74);
    let t = fx.tan_falloff_p(age, 0x30, 1);
    let a = fix(fx, ee::mul(128.0, ee::sub(1.0, t)), 0x01E0_06AC);
    rgba(fx, sp + 0x628, [0xFF, 0xF8, 0xE8], a as u8);
    window_xy(fx, d);
    fx.submit(d, OT, 0);
    let age = fx.mem.i32(D + 0x74) + 1;
    fx.mem.set32(D + 0x74, age);
    if age >= 0x31 {
        fx.mem.write_u8(D + 0x70, 0);
    }
}

/// The nine-point ring (D+0xC0) of radius `r`.
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

/// The two ring walls in the turn D+0x70 (D+0x1B90) [F 0x1E00730..0x1E014D8].
fn walls(fx: &mut Fx, sp: u32, v: &V) {
    if fx.mem.i32(D) == 0x4E {
        fx.mem.write_u8(D + 0x1BA0, 1);
        fx.mem.write_u32(D + 0x1BA4, 0);
        // The spots are the turn's origin: the table sine / cosine are
        // called and their results dropped.
        for s4 in 0..2i32 {
            let r = D + 0x10 * s4 as u32;
            let a = (((s4 << 16) >> 1) + 0x4000) & 0xFFFF;
            fx.tsin(a);
            fx.mem.write_u32(r + 0x1BB0, 0);
            fx.tcos(a);
            fx.mem.write_u32(r + 0x1BB4, 0);
            fx.mem.write_u32(r + 0x1BB8, 0);
            fx.mem.write_u32(r + 0x1BBC, 0x3F80_0000);
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
    // The ring offsets: radius 12 clamp(n - 0x82, 0, 0x10) / 16 times the
    // row's sin(0x5555 row / 20).
    let n = fx.mem.i32(D);
    let c = Fx::clamp_i(n - 0x4E, 0, 0x10);
    let f20 = ee::div(ee::mul(v.ring, ee::from_i32(c)), 16.0);
    for s7 in 0..2i32 {
        let fp = D + 0x150 * s7 as u32;
        let s4 = 0x6301 * s7;
        for s6 in 0..0x15i32 {
            let s2 = 0x5555 * s6;
            fx.mult(0x6666_6667, s2);
            let q2 = (fx.hilo().1 >> 3).wrapping_add(((s2 as u32) >> 31) as i32);
            let f21 = ee::mul(f20, fx.tsin(q2));
            let s1 = s6 << 15;
            fx.mult(0x6666_6667, s1);
            let q = (fx.hilo().1 >> 3).wrapping_add(((s1 as u32) >> 31) as i32);
            let age = fx.mem.i32(D + 0x1BA4);
            let a = (age.wrapping_mul(0x16C).wrapping_add(s4 + q) + 0x4000) & 0xFFFF;
            let p = fp + 0x10 * s6 as u32 + 0x1900;
            let sn = fx.tsin(a);
            fx.mem.setf(p, ee::mul(f21, sn));
            fx.mem.write_u32(p + 4, 0);
            let cs = fx.tcos(a);
            fx.mem.setf(p + 8, ee::mul(f21, cs));
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        }
    }
    // The height: the reach D+0xB0 times the falloff of clamp(age, 0, 0x54).
    let age = fx.mem.i32(D + 0x1BA4);
    let c = Fx::clamp_i(age, 0, 0x20);
    let f24 = ee::mul(fx.mem.f(D + 0xC0), fx.tan_falloff_p(c, 0x20, 1));
    let age = fx.mem.i32(D + 0x1BA4);
    let c = Fx::clamp_i(0xAA - age, 0, 0x20);
    let f22 = ee::div(ee::from_i32(c), 32.0);
    let q = sp + 0x6C0;
    quad_desc(fx, q);
    alpha(fx, q, 1);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(q, rh, 1);
    fx.desc_clamp(q, 0);
    rgba(fx, sp + 0x6D0, v.walls, 0x40);
    for o in [0x6E8u32, 0x700, 0x718] {
        copy8(fx, sp + o, sp + 0x6D0);
    }
    ring9(fx, ee::mul(5.0, f22));
    // a0..a3 at the first TEXFLUSH: 0x206F30's (desc, ot, key) and the
    // projection's desc after a drawn quad, else 0x205210's (in, out, m,
    // desc) of the rejected one.
    let mut last = [0u32; 4];
    for t in 0..2i32 {
        camera_turn(fx, sp, D + 0x50);
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
                for (o, x) in [(0x6D8u32, f2), (0x6DC, f23), (0x6F0, f0), (0x6F4, f23), (0x708, f2), (0x70C, f20), (0x720, f0), (0x724, f20)] {
                    fx.mem.setf(sp + o, x);
                }
                let vs = cell(s7, s6, 0x160);
                if fx.project_n(&vs, sp + 0x460, q, sp + 0x758, 0x01E0_0D50) == 1 {
                    let w = fx.mem.f(sp + 0x758);
                    let key = fix(fx, w, 0x01E0_0D64);
                    fx.submit(q, OT, key as i32);
                    last = [q, OT, key, q];
                } else {
                    last = [fx.sp - 0x40, PROJ_OUT, sp + 0x460, q];
                }
            }
        }
    }
    quad_desc(fx, q);
    let layer = fx.layer_begin();
    let regs = crate::lens::frame_copy(fx, last, 1, sp + 0x738, sp + 0x740);
    fx.texflush(regs);
    alpha(fx, q, 0);
    fx.desc_clamp(q, LENS_CLAMP);
    let fl = fx.mem.read_u16(q + 2);
    fx.mem.write_u16(q + 2, fl | 0x800);
    rgba(fx, sp + 0x6D0, [v.lens[0], v.lens[1], v.lens[2]], v.lens[3]);
    for o in [0x6E8u32, 0x700, 0x718] {
        copy8(fx, sp + o, sp + 0x6D0);
    }
    ring9(fx, ee::mul(4.0, f22));
    for t in 0..2i32 {
        camera_turn(fx, sp, D + 0x50);
        for s7 in 0..0x15i32 {
            let f20 = ee::div(ee::mul(f24, ee::from_i32(s7)), 20.0);
            fx.identity(sp + 0x560);
            let ph = phase(fx, t);
            let fp = s7 << 16;
            fx.mult(0x6666_6667, fp);
            let q20 = (fx.hilo().1 >> 3).wrapping_add(((fp as u32) >> 31) as i32);
            let sn = fx.tsin((q20 + ph) & 0xFFFF);
            let rot = fixs(fx, ee::mul(910.0, sn), 0x01E0_1030);
            fx.rotate_axis(sp + 0x560, rot, 1);
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
                fx.vu_apply_to(g + 0xD30, sp + 0x560, rp);
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
                if fx.project_n(&vs, sp + 0x460, q, sp + 0x758, 0x01E0_11B8) != 1 {
                    continue;
                }
                for (u, x) in [(0x6D8u32, 0x6E0u32), (0x6F0, 0x6F8), (0x708, 0x710), (0x720, 0x728)] {
                    fx.uv_from_xy(sp + u, sp + x, 0, 0);
                }
                let vs = cell(s7, s6, 0x160);
                if fx.project_n(&vs, sp + 0x460, q, sp + 0x758, 0x01E0_1234) == 1 && crate::b222::winding(fx, sp + 0x6E0, sp + 0x6F8, sp + 0x710) == 1 {
                    fx.submit(q, 0, 0);
                }
            }
        }
    }
    fx.layer_end(layer, OT, 1);
    tips(fx, sp, f24, v);
    let age = fx.mem.i32(D + 0x1BA4) + 1;
    fx.mem.set32(D + 0x1BA4, age);
    if age >= 0xAB {
        fx.mem.write_u8(D + 0x1BA0, 0);
    }
}

/// The glow at each wall's top (the last ring offset row over its spot,
/// raised by the height, turned by D+0x70 about D+0x50): a 30-wide sprite
/// (FF, 76, 4B, 128 clamp(0xAA - age, 0, 0x20) / 32) of texture D+0x10
/// [F 0x1E0129C..0x1E014B8].
fn tips(fx: &mut Fx, sp: u32, f23: f32, v: &V) {
    let d = sp + 0x5C0;
    sprite_desc(fx, d, 1, 0x10, 0);
    let age = fx.mem.i32(D + 0x1BA4);
    let c = Fx::clamp_i(0xAA - age, 0, 0x20);
    let a = fix(fx, ee::mul(128.0, ee::div(ee::from_i32(c), 32.0)), 0x01E0_138C);
    rgba(fx, sp + 0x5D0, v.tips, a as u8);
    uv(fx, sp + 0x5D8, 0x600);
    uv(fx, sp + 0x5DA, 0);
    uv(fx, sp + 0x5E8, 0x800);
    uv(fx, sp + 0x5EA, 0x200);
    vec4(fx, sp + 0x520, -30.0, -30.0, 0.0);
    fx.mem.write_u32(sp + 0x528, 0);
    vec4(fx, sp + 0x530, 30.0, 30.0, 0.0);
    fx.mem.write_u32(sp + 0x538, 0);
    for t in 0..2u32 {
        fx.vu_add4(sp + 0x5A0, D + 0x150 * t + 0x1A40, D + 0x10 * t + 0x1BD0);
        let y = ee::add(fx.mem.f(sp + 0x5A4), f23);
        fx.mem.setf(sp + 0x5A4, y);
        fx.vu_apply_to(sp + 0x5A0, D + 0x80, sp + 0x5A0);
        fx.vu_add4(sp + 0x5A0, D + 0x50, sp + 0x5A0);
        fx.camera_vp_to(sp + 0x460);
        fx.identity(sp + 0x4A0);
        fx.translate_rows(sp + 0x4A0, sp + 0x4E0, sp + 0x5A0);
        crate::blib::exe::mul_mem(fx, sp + 0x460, sp + 0x4A0);
        if crate::fxlib2::project(fx, sp + 0x520, PROJ_OUT, sp + 0x460, d, sp + 0x758) {
            let w = fx.mem.f(sp + 0x758);
            let key = fix(fx, w, 0x01E0_14E0);
            fx.submit(d, OT, key as i32);
        }
    }
}

/// (rem(age + 5 t, 64) << 16) >> 6: the wall's sway phase.
fn phase(fx: &mut Fx, t: i32) -> i32 {
    let age = fx.mem.i32(D + 0x1BA4);
    (rem2(age + 5 * t, 0x40) << 16) >> 6
}

/// The wall grid (D+0x150, 21 rows of 9) of side `t`: the ring raised by
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
    vec4(fx, sp + 0x520, ee::neg(s), ee::neg(s), 0.0);
    vec4(fx, sp + 0x530, s, s, 0.0);
    fx.mem.write_u32(sp + 0x528, 0);
    fx.mem.write_u32(sp + 0x538, 0);
    fx.vu_apply_to(sp + 0x520, sp + 0x4E0, sp + 0x520);
    fx.vu_apply_to(sp + 0x530, sp + 0x4E0, sp + 0x530);
}

/// Cell `c` of a 4 x 2 sprite atlas into the sprite's UVs.
fn atlas(fx: &mut Fx, sp: u32, c: i32) {
    let u = rem2(c, 4);
    let w = c >> 2;
    uv(fx, sp + 0x5D8, ((u << 9) & 0x3FFF) as u32);
    uv(fx, sp + 0x5DA, ((w << 9) & 0x3FFF) as u32);
    uv(fx, sp + 0x5E8, ((((u << 5) + 0x20) << 4) & 0x3FFF) as u32);
    uv(fx, sp + 0x5EA, ((((w << 5) + 0x20) << 4) & 0x3FFF) as u32);
}

/// The sprite at `p` (its corners about it at sp + 0x5B0 / 0x5C0).
fn place(fx: &mut Fx, sp: u32, p: u32) {
    fx.vu_add4(sp + 0x540, sp + 0x520, p);
    fx.vu_add4(sp + 0x550, sp + 0x530, p);
}

/// The bursts (D+0x1BE0, 16 x 0x150): 16 sprites flung from the target
/// along random table vectors [F 0x1E014DC..0x1E01A24].
fn bursts(fx: &mut Fx, sp: u32, v: &V) {
    let n = fx.mem.i32(D);
    if n >= 0x6F && n < 0xF8 {
        let r = D + 0x150 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(r + 0x1BF0, 1);
        fx.mem.write_u32(r + 0x1BF4, 1);
        const RAS: [[u32; 3]; 3] = [
            [0x01E0_15AC, 0x01E0_15E0, 0x01E0_160C],
            [0x01E0_163C, 0x01E0_1670, 0x01E0_169C],
            [0x01E0_16CC, 0x01E0_1700, 0x01E0_172C],
        ];
        for s4 in 0..0x10u32 {
            let c = rem2(rnd(fx, 0x01E0_1588), 4) + 4;
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
    let d = sp + 0x5C0;
    sprite_desc(fx, d, 1, 0x1C, 5);
    camera(fx, sp);
    for k in 0..0x10u32 {
        let r = D + 0x150 * k;
        if fx.mem.read_u8(r + 0x1BF0) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 0x1BF4);
        let a = ((0x10 - age) << 6) >> 4;
        rgba(fx, sp + 0x5D0, v.bursts, a as u8);
        let age = ee::from_i32(fx.mem.i32(r + 0x1BF4));
        let s = ee::mul(18.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 16.0)));
        corners(fx, sp, s);
        let age = fx.mem.i32(r + 0x1BF4);
        let f20 = fx.tan_falloff_p(age, 0x10, 1);
        for s6 in 0..0x10u32 {
            fx.vu_scale4(sp + 0x5A0, r + 0x10 * s6 + 0x1C40, f20);
            fx.vu_add4(sp + 0x5A0, sp + 0x5A0, D + 0x30);
            place(fx, sp, sp + 0x5A0);
            let c = fx.mem.i32(r + 4 * s6 + 0x1BF8);
            atlas(fx, sp, c);
            if crate::fxlib2::project(fx, sp + 0x540, PROJ_OUT, sp + 0x460, d, sp + 0x758) {
                let w = fx.mem.f(sp + 0x758);
                let key = fix(fx, w, 0x01E0_1A18);
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

/// The fountains (D+0x30E0, 16 x 0x250): on 0x83..0x151 two groups of 16
/// sprites from the node point D+0x50 along 1.2 table vectors (cells rand
/// % 8, 17 updates) [F 0x1E01A28..0x1E01E8C].
fn fountains(fx: &mut Fx, sp: u32, v: &V) {
    let n = fx.mem.i32(D);
    if n >= 0x4F && n < 0xF8 {
        let r = D + 0x250 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(r + 0x30F0, 1);
        fx.mem.write_u32(r + 0x30F4, 1);
        for g in 0..2u32 {
            for w in 0..4u32 {
                let x = fx.mem.read_u32(D + 0x50 + 4 * w);
                fx.mem.write_u32(r + 0x10 * g + 0x3120 + 4 * w, x);
            }
            for s6 in 0..0x10u32 {
                let c = rem2(rnd(fx, 0x01E0_1B00), 8);
                fx.mem.write_u8(r + 0x10 * g + s6 + 0x30F8, c as u8);
                fx.random_table_vec(r + 0x100 * g + 0x10 * s6 + 0x3140, f(0x3F99_999A));
            }
        }
    }
    let d = sp + 0x5C0;
    sprite_desc(fx, d, 1, 0x1C, 5);
    camera(fx, sp);
    for k in 0..0x10u32 {
        let r = D + 0x250 * k;
        if fx.mem.read_u8(r + 0x30F0) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 0x30F4);
        let a = ((0x10 - age) * 26) >> 4;
        rgba(fx, sp + 0x5D0, v.fountains, a as u8);
        let agef = ee::from_i32(fx.mem.i32(r + 0x30F4));
        let s = ee::mul(10.0, ee::add(0.5, ee::div(ee::mul(0.5, agef), 16.0)));
        corners(fx, sp, s);
        let age = fx.mem.i32(r + 0x30F4);
        let t = fx.tan_falloff_p(age, 0x10, 1);
        let f20 = ee::add(f(0x3E4C_CCCD), ee::mul(f(0x3F4C_CCCD), t));
        for g in 0..2u32 {
            for s6 in 0..0x10u32 {
                fx.vu_scale4(sp + 0x5A0, r + 0x100 * g + 0x10 * s6 + 0x3140, f20);
                fx.vu_add4(sp + 0x5A0, sp + 0x5A0, r + 0x10 * g + 0x3120);
                place(fx, sp, sp + 0x5A0);
                let c = i32::from(fx.mem.read_u8(r + 0x10 * g + s6 + 0x30F8));
                atlas(fx, sp, c);
                if crate::fxlib2::project(fx, sp + 0x540, PROJ_OUT, sp + 0x460, d, sp + 0x758) {
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

/// The shards (D+0x5720, 12 x 0x350): 2 x 20 spinning three-panel shards
/// rising along the walls' ring offsets in the turn [F 0x1E01E90..0x1E02434].
fn shards(fx: &mut Fx, sp: u32, v: &V) -> bool {
    let n = fx.mem.i32(D);
    if n >= 0x4F && n < 0xF8 {
        let (_, k) = fx.div(n, 0xC);
        let r = D.wrapping_add(fx.mult(k, 0x350) as u32);
        fx.mem.write_u8(r + 0x5730, 1);
        fx.mem.write_u32(r + 0x5734, 0);
        for s7 in 0..2u32 {
            for s6 in 0..0x14u32 {
                let x = rnd(fx, 0x01E0_1F48);
                let (_, m) = fx.div(x, 3);
                fx.mem.write_u8(r + 0x14 * s7 + s6 + 0x5738, (m + 5) as u8);
                let c = rem2(rnd(fx, 0x01E0_1F6C), 0x100);
                fx.mem.set32(r + 0x50 * s7 + 4 * s6 + 0x5760, (c << 16) >> 8);
                let p = r + 0x140 * s7 + 0x10 * s6 + 0x5800;
                fx.random_table_vec(p, 0.0);
                let y = ee::mul(fx.mem.f(p + 4), 2.0);
                fx.mem.setf(p + 4, y);
            }
        }
    }
    let q = sp + 0x6C0;
    quad_desc(fx, q);
    alpha(fx, q, 1);
    let rh = fx.mem.read_u32(D + 0x1C);
    fx.desc_texture(q, rh, 1);
    fx.desc_clamp(q, 5);
    let mut any = false;
    for k in 0..0xCu32 {
        let r = D + 0x350 * k;
        if fx.mem.read_u8(r + 0x5730) == 0 {
            continue;
        }
        any = true;
        let age = fx.mem.i32(r + 0x5734);
        let a = div12(fx, (0xC - age).wrapping_mul(96));
        rgba(fx, sp + 0x6D0, v.shards, a as u8);
        for o in [0x6E8u32, 0x700, 0x718] {
            copy8(fx, sp + o, sp + 0x6D0);
        }
        let agef = ee::from_i32(fx.mem.i32(r + 0x5734));
        let s = ee::add(f(0x3DCC_CCCD), ee::div(ee::mul(f(0x3F66_6666), agef), 12.0));
        vec4(fx, sp + 0x5B0, s, s, s);
        let age = fx.mem.i32(D + 0x1BA4);
        let c = Fx::clamp_i(age - 1, 0, 0x20);
        let f20 = ee::mul(fx.mem.f(D + 0xC0), fx.tan_falloff_p(c, 0x20, 1));
        for t in 0..2u32 {
            for s6 in 0..0x14u32 {
                fx.vu_add4(sp + 0x5A0, D + 0x150 * t + 0x10 * (s6 + 1) + 0x1900, D + 0x10 * t + 0x1BD0);
                fx.vu_add4(sp + 0x5A0, sp + 0x5A0, r + 0x140 * t + 0x10 * s6 + 0x5800);
                let y = ee::add(fx.mem.f(sp + 0x5A4), ee::div(ee::mul(f20, ee::from_i32(s6 as i32 + 1)), 20.0));
                fx.mem.setf(sp + 0x5A4, y);
                fx.vu_apply_to(sp + 0x5A0, D + 0x80, sp + 0x5A0);
                fx.vu_add4(sp + 0x5A0, D + 0x50, sp + 0x5A0);
                fx.camera_vp_to(sp + 0x460);
                fx.identity(sp + 0x4A0);
                crate::blib::exe::mul_mem(fx, sp + 0x4A0, D + 0x80);
                fx.translate_rows(sp + 0x4A0, sp + 0x4A0, sp + 0x5A0);
                let age = fx.mem.i32(r + 0x5734);
                let spin = fx.mult(age, 0x3E7);
                let a = fx.mem.i32(r + 0x50 * t + 4 * s6 + 0x5760).wrapping_add(spin) & 0xFFFF;
                fx.rotate_axis(sp + 0x4A0, a, 1);
                fx.scale_rows(sp + 0x4A0, sp + 0x5B0);
                crate::blib::exe::mul_mem(fx, sp + 0x460, sp + 0x4A0);
                let b = u32::from(fx.mem.read_u8(r + 0x14 * t + s6 + 0x5738));
                for s1 in 0..3u32 {
                    let (s3, s5) = (D + 0x10 * b + 4 * s1, D + 8 * b);
                    for (o, src) in [(0x6D8u32, s3 + 0x55F0), (0x6DC, s5 + 0x5670), (0x6F0, s3 + 0x55F4), (0x6F4, s5 + 0x5670), (0x708, s3 + 0x55F0), (0x70C, s5 + 0x5674), (0x720, s3 + 0x55F4), (0x724, s5 + 0x5674)] {
                        let x = fx.mem.read_u32(src);
                        fx.mem.write_u32(sp + o, x);
                    }
                    let (e0, e1) = (D + 0x20 * s1, D + 0x20 * (s1 + 1));
                    let vs = [e0 + 0x56B0, e1 + 0x56B0, e0 + 0x56C0, e1 + 0x56C0];
                    if fx.project_n(&vs, sp + 0x460, q, sp + 0x758, 0x01E0_236C) == 1 {
                        let w = fx.mem.f(sp + 0x758);
                        let key = fix(fx, w, 0x01E0_2380);
                        fx.submit(q, OT, key as i32);
                    }
                }
            }
        }
        let age = fx.mem.i32(r + 0x5734) + 1;
        fx.mem.set32(r + 0x5734, age);
        if age >= 0xD {
            fx.mem.write_u8(r + 0x5730, 0);
        }
    }
    any
}

/// x / 17 (HI of 0x78787879 x, >> 3, plus the sign).
fn div17(fx: &mut Fx, x: i32) -> i32 {
    fx.mult(0x7878_7879, x);
    (fx.hilo().1 >> 3).wrapping_add(((x as u32) >> 31) as i32)
}

/// The Z of `z` into the XYZ2 at `p`.
fn set_z(fx: &mut Fx, p: u32, z: u32) {
    let w = fx.mem.read_u64(p);
    fx.mem.write_u64(p, (w & KEEP) | (u64::from(z & 0xFF_FFFF) << 32));
}

/// clamp(age, 0, 0x60) clamp(0xF8 - age, 0, 0x60) / 9216.
fn pass_level(fx: &mut Fx) -> f32 {
    let age = fx.mem.i32(D + 0xC0F4);
    let c = ee::from_i32(Fx::clamp_i(age, 0, 0x60));
    let age = fx.mem.i32(D + 0xC0F4);
    let c2 = Fx::clamp_i(0xF8 - age, 0, 0x60);
    ee::div(ee::mul(c, ee::from_i32(c2)), 9216.0)
}

/// The attacker drawn into its target with the callback and pulled
/// through 16 rings of quads and a fan below update 0x82, then its glow
/// sprite (D+0xC0E0, from 0x20) [F 0x1E02488..0x1E030F0].
fn attacker_pass(fx: &mut Fx, sp: u32, a3: u32, v: &V) {
    if fx.mem.i32(D) == 0x8 {
        fx.mem.write_u8(D + 0xC0F0, 1);
        fx.mem.write_u32(D + 0xC0F4, 0);
    }
    if fx.mem.read_u8(D + 0xC0F0) != 1 {
        return;
    }
    if fx.mem.i32(D) < 0x4E {
        pass_layer(fx, sp, a3, v);
    }
    let d = sp + 0x5C0;
    sprite_desc(fx, d, 1, 0x10, 0);
    camera_at(fx, sp);
    let f3 = pass_level(fx);
    let age = fx.mem.i32(D + 0xC0F4);
    let s = ee::add(f(0x3F4C_CCCD), ee::mul(f(0x3E4C_CCCD), ee::from_i32(rem2(age, 2))));
    let a = fix(fx, ee::mul(ee::mul(255.0, f3), s), 0x01E0_2FAC);
    rgba(fx, sp + 0x5D0, v.glow, a as u8);
    uv(fx, sp + 0x5D8, 0x600);
    uv(fx, sp + 0x5DA, 0);
    uv(fx, sp + 0x5E8, 0x800);
    uv(fx, sp + 0x5EA, 0x200);
    vec4(fx, sp + 0x520, -80.0, -80.0, 0.0);
    fx.mem.write_u32(sp + 0x528, 0);
    vec4(fx, sp + 0x530, 80.0, 80.0, 0.0);
    fx.mem.write_u32(sp + 0x538, 0);
    if crate::fxlib2::project(fx, sp + 0x520, PROJ_OUT, sp + 0x460, d, sp + 0x758) {
        far(fx, sp + 0x5E0, &[sp + 0x5F0]);
        fx.submit(d, OT, 0);
    }
    let age = fx.mem.i32(D + 0xC0F4) + 1;
    fx.mem.set32(D + 0xC0F4, age);
    if age >= 0xF9 {
        fx.mem.write_u8(D + 0xC0F0, 0);
    }
}

fn pass_layer(fx: &mut Fx, sp: u32, a3: u32, v: &V) {
    let t = sp + 0x648;
    head(fx, t, 0x33, 9, 0x4_2142_1421, 0);
    let q = sp + 0x6C0;
    quad_desc(fx, q);
    let layer = fx.layer_begin();
    fx.texflush([0x2142_1421, t, 0x20, a3]);
    fx.mem.write_u32(D_STATIC, D);
    let (_, regs) = fx.unit_draw_post(1, sp + 0x6A8, sp + 0x6B0, CB, &mut |fx, p| callback(fx, p));
    copy8(fx, sp + 0x738, sp + 0x6A8);
    copy8(fx, sp + 0x740, sp + 0x6B0);
    fx.texflush([sp + 0x738, regs[1], regs[2], regs[3]]);
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
        let f21 = ee::div(ee::mul(20.0, ee::from_i32(0x10 - k)), 16.0);
        let ak = k.wrapping_mul(0x30000) >> 4;
        for s1 in 0..0x21i32 {
            let s4 = (s1 << 16) >> 5;
            let s5 = s1.wrapping_mul(0x20000) >> 5;
            let age = fx.mem.i32(D + 0xC0F4);
            let m = fx.mult(age, 0xD05);
            let a = m.wrapping_add(ak + s5) & 0xFFFF;
            let f20 = ee::add(1.0, ee::mul(0.5, fx.tsin(a)));
            let age = fx.mem.i32(D + 0xC0F4);
            let m = fx.mult(age, 0x8AE);
            let b = ak.wrapping_add(m) & 0xFFFF;
            let f0 = ee::mul(8192.0, fx.tsin(b));
            let ang = fixs(fx, ee::add(ee::from_i32(s4), f0), 0x01E0_277C) & 0xFFFF;
            let f20 = ee::mul(f21, f20);
            let p = rb + 0x10 * s1 as u32 + 0x9FF0;
            let sn = fx.tsin(ang);
            fx.mem.setf(p, ee::mul(f20, sn));
            let cs = fx.tcos(ang);
            fx.mem.setf(p + 4, ee::mul(f20, cs));
            fx.mem.write_u32(p + 8, 0);
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        }
    }
    for k in 0..3u32 {
        fx.mem.write_u32(sp + 0x5A0 + 4 * k, 0);
    }
    fx.mem.write_u32(sp + 0x5AC, 0x3F80_0000);
    let f20 = pass_level(fx);
    for k in 0..0x10i32 {
        let rb = D + 0x210 * k as u32;
        if k == 0xF {
            let x = div17(fx, 128 * k);
            let a = fix(fx, ee::mul(f20, ee::from_i32(x)), 0x01E0_28F8);
            rgba(fx, sp + 0x658, v.pass, a as u8);
            let x = div17(fx, 128 * (k - 1));
            let a = fix(fx, ee::mul(f20, ee::from_i32(x)), 0x01E0_2960);
            rgba(fx, sp + 0x670, v.pass, a as u8);
            copy8(fx, sp + 0x688, sp + 0x670);
            for s3 in 0..0x20u32 {
                let (p0, p1) = (rb + 0x10 * s3, rb + 0x10 * (s3 + 1));
                if fx.project_n(&[sp + 0x5A0, p0 + 0x9FF0, p1 + 0x9FF0], sp + 0x460, t, sp + 0x758, 0x01E0_29DC) != 1 {
                    continue;
                }
                for (u, x) in [(0x660u32, 0x668u32), (0x678, 0x680), (0x690, 0x698)] {
                    fx.uv_from_xy(sp + u, sp + x, 0, 0);
                }
                if fx.project_n(&[sp + 0x5A0, p0 + 0x7EF0, p1 + 0x7EF0], sp + 0x460, t, sp + 0x758, 0x01E0_2A40) == 1 {
                    let z = fx.mem.read_u32(D + 0x44);
                    set_z(fx, sp + 0x668, z);
                    copy_z(fx, sp + 0x680, sp + 0x668);
                    copy_z(fx, sp + 0x698, sp + 0x668);
                    fx.submit(t, 0, 0);
                }
            }
        } else {
            let x = div17(fx, 128 * k);
            let a = fix(fx, ee::mul(f20, ee::from_i32(x)), 0x01E0_2B50);
            rgba(fx, sp + 0x6D0, v.pass, a as u8);
            copy8(fx, sp + 0x6E8, sp + 0x6D0);
            let x = div17(fx, 128 * (k + 1));
            let a = fix(fx, ee::mul(f20, ee::from_i32(x)), 0x01E0_2BC8);
            rgba(fx, sp + 0x700, v.pass, a as u8);
            copy8(fx, sp + 0x718, sp + 0x700);
            let rn = rb + 0x210;
            for s4 in 0..0x20u32 {
                let c = |base: u32| [rb + 0x10 * s4 + base, rb + 0x10 * (s4 + 1) + base, rn + 0x10 * s4 + base, rn + 0x10 * (s4 + 1) + base];
                if fx.project_n(&c(0x9FF0), sp + 0x460, q, sp + 0x758, 0x01E0_2C80) != 1 {
                    continue;
                }
                for (u, x) in [(0x6D8u32, 0x6E0u32), (0x6F0, 0x6F8), (0x708, 0x710), (0x720, 0x728)] {
                    fx.uv_from_xy(sp + u, sp + x, 0, 0);
                }
                if fx.project_n(&c(0x7EF0), sp + 0x460, q, sp + 0x758, 0x01E0_2D00) == 1 {
                    let z = fx.mem.read_u32(D + 0x44);
                    set_z(fx, sp + 0x6E0, z);
                    for o in [0x6F8u32, 0x710, 0x728] {
                        copy_z(fx, sp + o, sp + 0x6E0);
                    }
                    fx.submit(q, 0, 0);
                }
            }
        }
    }
    fx.layer_end(layer, OT, 0);
}

/// The frame-textured window sprite while clamp(n - 0x82, 0, 0x10)
/// clamp(0x128 - n, 0, 0x10) / 256 > 0 [F 0x1E030F4..0x1E032E4].
fn window(fx: &mut Fx, sp: u32, v: &V) {
    let n = fx.mem.i32(D);
    let a = ee::from_i32(Fx::clamp_i(n - 0x4E, 0, 0x10));
    let n = fx.mem.i32(D);
    let b = ee::from_i32(Fx::clamp_i(0x128 - n, 0, 0x10));
    let f20 = ee::div(ee::mul(a, b), 256.0);
    if crate::exe::ee_le(f20, 0.0) {
        return;
    }
    let d = sp + 0x5C0;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    let flags = u32::from(fx.mem.read_u16(d + 2));
    alpha(fx, d, 0);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.desc_clamp(d, 5);
    let a = rnd(fx, 0x01E0_322C) % 3 - 1;
    let b = rnd(fx, 0x01E0_324C) % 3 - 1;
    uv(fx, sp + 0x5D8, (((a + 0x14) << 4) & 0x3FFF) as u32);
    uv(fx, sp + 0x5DA, (((b + 0x14) << 4) & 0x3FFF) as u32);
    uv(fx, sp + 0x5E8, (((a + 0x1EC) << 4) & 0x3FFF) as u32);
    uv(fx, sp + 0x5EA, (((b + 0x1EC) << 4) & 0x3FFF) as u32);
    let a = fix(fx, ee::mul(64.0, f20), 0x01E0_32F8);
    rgba(fx, sp + 0x5D0, v.window, a as u8);
    far_xy(fx, sp + 0x5E0, 0x7000, 0x7000);
    far_xy(fx, sp + 0x5F0, 0x9000, 0x9000);
    fx.submit(d, OT, 0);
}

/// The motes (D+0xC0F0, 24 x 0x60): on 0xD7..0x185 one record of 4
/// sprites flung from the target (D+0x30) along 2.0 table vectors (4 x 2
/// atlas of 32, cells rand % 8, 25 updates) [F 0x1E032E8..0x1E03824].
/// Returns a0..a3 as the loop's last callee leaves them (the outcome call's
/// arguments).
fn motes(fx: &mut Fx, sp: u32, v: &V) -> [u32; 4] {
    let n = fx.mem.i32(D);
    if n >= 0x6F && n < 0x10E {
        let (_, k) = fx.div(n, 0x18);
        let r = D + 0x60 * k as u32;
        fx.mem.write_u8(r + 0xC100, 1);
        fx.mem.write_u32(r + 0xC104, 0);
        for w in 0..4u32 {
            let x = fx.mem.read_u32(D + 0x30 + 4 * w);
            fx.mem.write_u32(r + 0xC110 + 4 * w, x);
        }
        for i in 0..4u32 {
            let c = rem2(rnd(fx, 0x01E0_3440), 8);
            fx.mem.write_u8(r + i + 0xC108, c as u8);
            fx.random_table_vec(r + 0x10 * i + 0xC120, 2.0);
        }
    }
    let d = sp + 0x5C0;
    sprite_desc(fx, d, 0, 0x14, 5);
    camera(fx, sp);
    // With no record drawn: as the camera leaves them (0x20B370's a0 / a1,
    // 0x20E2B0's a2 = 0 and a3 past the camera's matrix) [run].
    let idx = fx.mem.read_u32(crate::world::CAMERA_STACK_INDEX);
    let cam = crate::world::CAMERA_RECORD.wrapping_add(idx.wrapping_mul(0x1A0)) + 0x40;
    let mut last = [sp + 0x460, sp + 0x4A0, 0, cam];
    for k in 0..0x18u32 {
        let r = D + 0x60 * k;
        if fx.mem.read_u8(r + 0xC100) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 0xC104);
        let x = age << 15;
        fx.mult(0x2AAA_AAAB, x);
        let a = (fx.hilo().1 >> 2).wrapping_add(((x as u32) >> 31) as i32);
        let sn = fx.tsin(a);
        let a = fix(fx, ee::mul(v.mote_amp, sn), 0x01E0_35AC);
        rgba(fx, sp + 0x5D0, [0x0C, 0x08, 0x08], a as u8);
        let agef = ee::from_i32(fx.mem.i32(r + 0xC104));
        let s = ee::mul(15.0, ee::add(0.5, ee::div(ee::mul(0.5, agef), 24.0)));
        corners(fx, sp, s);
        let agef = ee::from_i32(fx.mem.i32(r + 0xC104));
        let f20 = ee::add(f(0x3ECC_CCCD), ee::div(ee::mul(f(0x3F19_999A), agef), 24.0));
        let f21 = ee::add(-15.0, ee::div(ee::mul(40.0, agef), 24.0));
        for i in 0..4u32 {
            fx.vu_scale4(sp + 0x5A0, r + 0x10 * i + 0xC120, f20);
            fx.vu_add4(sp + 0x5A0, sp + 0x5A0, r + 0xC110);
            let y = ee::add(fx.mem.f(sp + 0x5A4), f21);
            fx.mem.setf(sp + 0x5A4, y);
            place(fx, sp, sp + 0x5A0);
            let c = i32::from(fx.mem.read_u8(r + i + 0xC108));
            atlas(fx, sp, c);
            if crate::fxlib2::project(fx, sp + 0x540, PROJ_OUT, sp + 0x460, d, sp + 0x758) {
                if v.mote_keyed {
                    let w = fx.mem.f(sp + 0x758);
                    let key = fix(fx, w, 0x01E0_381C);
                    fx.submit(d, OT, key as i32);
                    last = [d, OT, key, 0];
                } else {
                    fx.submit(d, OT, 0);
                    last = [d, OT, 0, 0];
                }
            } else {
                last = [sp + 0x540, PROJ_OUT, sp + 0x460, d];
            }
        }
        let age = fx.mem.i32(r + 0xC104) + 1;
        fx.mem.set32(r + 0xC104, age);
        if age >= 0x19 {
            fx.mem.write_u8(r + 0xC100, 0);
        }
    }
    last
}

/// The sprite pool (D+0xCA00, 32 x 0xA0) [F 0x1E038E0..0x1E03D40]: on
/// 1..0x45 record n % 32 at the node point D+0x60 with eight 0.8 table
/// vectors and cells rand % 8; each update every live record's eight
/// sprites (9B, 91, 91, 100 cos(age 512)) of half size 5 (0.3 + 0.7
/// falloff(age, 0x20)) at the point + (0.1 + 0.9 age / 32) vector, 4 x 2
/// atlas, keyed by w; 33 updates. Returns a0..a3 as the loop's last callee
/// leaves them (the outcome call's arguments): the camera's with nothing
/// drawn, 0x206F30's (desc, OT, key, 0) after a drawn sprite, else the
/// projection's (in, PROJ_OUT, m, desc).
fn pool(fx: &mut Fx, sp: u32) -> [u32; 4] {
    let n = fx.mem.i32(D);
    if n > 0 && n < 0x46 {
        let r = D + POOL + 0xA0 * (n & 0x1F) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for w in 0..4u32 {
            let x = fx.mem.read_u32(D + 0x60 + 4 * w);
            fx.mem.write_u32(r + 0x10 + 4 * w, x);
        }
        for i in 0..8u32 {
            let c = rem2(rnd(fx, 0x01E0_3970), 8);
            fx.mem.write_u8(r + 8 + i, c as u8);
            fx.random_table_vec(r + 0x20 + 0x10 * i, f(0x3F4C_CCCD));
        }
    }
    let d = sp + 0x5C0;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, rh, 1);
    alpha(fx, d, 0);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    let idx = fx.mem.read_u32(crate::world::CAMERA_STACK_INDEX);
    let cam = crate::world::CAMERA_RECORD.wrapping_add(idx.wrapping_mul(0x1A0)) + 0x40;
    let mut last = [sp + 0x460, sp + 0x4A0, 0, cam];
    for k in 0..0x20u32 {
        let r = D + POOL + 0xA0 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 4);
        let cs = fx.tcos((age << 14) >> 5);
        let a = fix(fx, ee::mul(100.0, cs), 0x01E0_3A90);
        rgba(fx, sp + 0x5D0, [0x9B, 0x91, 0x91], a as u8);
        let age = fx.mem.i32(r + 4);
        let t = fx.tan_falloff_p(age, 0x20, 1);
        let s = ee::mul(5.0, ee::add(f(0x3E99_999A), ee::mul(f(0x3F33_3333), t)));
        vec4(fx, sp + 0x520, s, s, 0.0);
        vec4(fx, sp + 0x530, ee::neg(s), ee::neg(s), 0.0);
        fx.vu_apply_to(sp + 0x520, sp + 0x4E0, sp + 0x520);
        fx.vu_apply_to(sp + 0x530, sp + 0x4E0, sp + 0x530);
        let agef = ee::from_i32(fx.mem.i32(r + 4));
        let f20 = ee::add(f(0x3DCC_CCCD), ee::div(ee::mul(f(0x3F66_6666), agef), 32.0));
        for i in 0..8u32 {
            let c = i32::from(fx.mem.read_u8(r + 8 + i));
            atlas(fx, sp, c);
            fx.vu_scale4(sp + 0x5A0, r + 0x20 + 0x10 * i, f20);
            fx.vu_add4(sp + 0x5A0, sp + 0x5A0, r + 0x10);
            place(fx, sp, sp + 0x5A0);
            if crate::fxlib2::project(fx, sp + 0x540, PROJ_OUT, sp + 0x460, d, sp + 0x758) {
                let w = fx.mem.f(sp + 0x758);
                let key = fix(fx, w, 0x01E0_3CBC);
                fx.submit(d, OT, key as i32);
                last = [d, OT, key, 0];
            } else {
                last = [sp + 0x540, PROJ_OUT, sp + 0x460, d];
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(r, 0);
        }
    }
    last
}

/// The post callback 0x1E03950(p) [F]: b222's (FRAME_1 masked, a dark
/// full-window veil, a (FF, FF, FF, 80) sprite of half size 20 with texture
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
    vec4(fx, sp + 0xF0, -20.0, -20.0, 0.0);
    fx.mem.write_u32(sp + 0xF8, 0);
    vec4(fx, sp + 0x100, 20.0, 20.0, 0.0);
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
