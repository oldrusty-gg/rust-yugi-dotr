//! BMODULEs b725 / b726 (cards 16 / 17), hand-ported from the records
//! (linked at 0x1E00000, the entry 0x1E00080 in a 0x750 frame with D in s0
//! and the OT in s1, data area a0 = 0x1ED0000, 0xC9F0 bytes; one attacker
//! post callback 0x1E03950 with its static D at 0x1E03C80) [F] (helper D31,
//! 3 Nov 2026). b713 (card 226, b713.rs) at 0.933, written from b713.rs by
//! rule (the project notes: locals from sp + 0x400 0x70 lower, D offsets
//! 0x48 +0x18, 0x50.. +0x70, from the shard texels -0x2190) and its hunks
//! (ed725.py / ed725b.py): a node 0x36 (b726 0x35) whose point D+0x50 (taken
//! at 0x82) is the effect's origin: every update the turn D+0x70 from it to
//! the target (0x100E40, 0x4000 about x) and the reach D+0xB0 = |target -
//! point| (powf / sqrtf); the timeline from 0x82 (flash 0x30 updates, also
//! clearing the pass flag; walls at the turn's origin, ring offsets 12 (b726
//! 17) clamp(n - 0x82, 0, 0x10) / 16 sin(0x5555 row / 20) at 0x16C a step,
//! height D+0xB0 x the falloff of clamp(age, 0, 0x54), fade from 0xD0, end
//! 0xD1, both passes in the turn's camera, a glow at each wall's top);
//! bursts 0xD7..0x151; fountains (16 x 0x250) 0x83..0x151 from the node
//! point; shards 0x83..0x151 (zero vectors, rows + 1 of the ring offsets at
//! the walls' height, turned); the attacker pass from 0x20 (its layer below
//! 0x82, rings 20, swing 8192, alphas 128 k / 17, level / 9216 to 0x152,
//! glow half 80 to 0x153); the window 0x82..0x182; the stage light from
//! 0x182; motes on 0xD7..0x185 on 2.0 vectors; the outcome fade from 0xE7
//! (4 clamp(n - 0xE6, 0, 0x20)); returns 1 / 6 / 2 / 8 on 0xCC / 0xD7..0x161
//! / 0x162 / 0x1BF. b726 ([`B726`]) changes only colours, the node, the ring
//! radius, the fountains' alpha, the motes' amplitude / key and the fade's
//! return address.
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
const FRAME: u32 = 0x750;
const CB: u32 = 0x01E0_3950;
const D_STATIC: u32 = 0x01E0_3C80;

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
    /// b726: the fountains' alpha (16 - age) 3 (b725: (16 - age) 36 >> 4).
    fountain3: bool,
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

pub const B725: V = V {
    node: 0x36,
    ring: 12.0,
    walls: [0xFF, 0x58, 0x41],
    lens: [0x8A, 0x87, 0x80, 0x80],
    tips: [0xFF, 0x76, 0x4B],
    bursts: [0xFF, 0x8A, 0x4B],
    fountains: [0xFF, 0x8A, 0x4B],
    fountain3: false,
    shards: [0xFF, 0x8A, 0x4B],
    pass: [0xFF, 0x20, 0x0C],
    glow: [0xFF, 0x80, 0x0C],
    window: [0xBC, 0x9E, 0x62],
    mote_amp: 96.0,
    mote_keyed: false,
    fade_ra: 0x01E0_3888,
};

pub const B726: V = V {
    node: 0x35,
    ring: 17.0,
    walls: [0x9E, 0x6C, 0xC8],
    lens: [0x87, 0x80, 0x8A, 0x80],
    tips: [0x9E, 0x6C, 0xC8],
    bursts: [0x9E, 0x6C, 0xC8],
    fountains: [0x9E, 0x6C, 0xC8],
    fountain3: true,
    shards: [0xD0, 0x8A, 0xFF],
    pass: [0x50, 0x40, 0xFF],
    glow: [0x94, 0x38, 0xFF],
    window: [0x9E, 0x80, 0xC8],
    mote_amp: 128.0,
    mote_keyed: true,
    fade_ra: 0x01E0_3890,
};

/// The entry 0x1E00080 of b725 (card 16).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B725)
}

/// The entry 0x1E00080 of b726 (card 17).
pub fn program_b726(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B726)
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
    fx.bzero(D, 0xC9F0);
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
    fx.mem.write_u8(D + 0x60, 0);
    fx.mem.write_u8(D + 0x1B90, 0);
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x1BE0 + 0x150 * k, 0);
    }
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x30E0 + 0x250 * k, 0);
    }
    // The shard texels: eight cells of four S and two T.
    for a2 in 0..8i32 {
        let f2 = ee::from_i32(rem2(a2, 4) << 5);
        for a3 in 0..4i32 {
            let s = ee::div(ee::add(f2, ee::div(ee::mul(32.0, ee::from_i32(a3)), 3.0)), 128.0);
            fx.mem.setf(D + 0x55E0 + 0x10 * a2 as u32 + 4 * a3 as u32, s);
        }
        let f2 = ee::from_i32((a2 >> 2) << 5);
        fx.mem.setf(D + 0x5660 + 8 * a2 as u32, ee::div(f2, 64.0));
        fx.mem.setf(D + 0x5664 + 8 * a2 as u32, ee::div(ee::add(32.0, f2), 64.0));
    }
    // The shard edges: top / bottom pairs a third of a turn apart.
    let mut s0 = 0i32;
    for s3 in 0..4i32 {
        let r = D + 0x20 * s3 as u32;
        fx.mult(0x5555_5556, s0);
        let s4 = fx.hilo().1.wrapping_add(((s0 as u32) >> 31) as i32);
        let sn = fx.tsin(s4);
        fx.mem.setf(r + 0x56A0, ee::mul(15.0, sn));
        fx.mem.write_u32(r + 0x56A4, 0x4120_0000);
        let cs = fx.tcos(s4);
        fx.mem.setf(r + 0x56A8, ee::mul(15.0, cs));
        fx.mem.write_u32(r + 0x56AC, 0x3F80_0000);
        for w in 0..4u32 {
            let x = fx.mem.read_u32(r + 0x56A0 + 4 * w);
            fx.mem.write_u32(r + 0x56B0 + 4 * w, x);
        }
        let f1 = ee::div(ee::mul(-30.0, ee::from_i32(s3)), 3.0);
        let y = ee::mul(fx.mem.f(r + 0x56B4), -1.0);
        fx.mem.setf(r + 0x56B4, y);
        let f1 = ee::add(15.0, f1);
        let y = ee::add(fx.mem.f(r + 0x56A4), f1);
        fx.mem.setf(r + 0x56A4, y);
        let y = ee::add(fx.mem.f(r + 0x56B4), f1);
        fx.mem.setf(r + 0x56B4, y);
        s0 = s0.wrapping_add(0x8000);
    }
    for k in 0..0xCu32 {
        fx.mem.write_u8(D + 0x5720 + 0x350 * k, 0);
    }
    // The attacker rings.
    for s4 in 0..0x10i32 {
        let f20 = ee::div(ee::mul(40.0, ee::from_i32(0x10 - s4)), 16.0);
        for s5 in 0..0x21i32 {
            let a = (s5 << 16) >> 5;
            let p = D + 0x210 * s4 as u32 + 0x10 * s5 as u32 + 0x7EE0;
            let sn = fx.tsin(a);
            fx.mem.setf(p, ee::mul(f20, sn));
            let cs = fx.tcos(a);
            fx.mem.setf(p + 4, ee::mul(f20, cs));
            fx.mem.write_u32(p + 8, 0);
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        }
    }
    fx.mem.write_u8(D + 0xC0E0, 0);
    for k in 0..0x18u32 {
        fx.mem.write_u8(D + 0xC0F0 + 0x60 * k, 0);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_add_mode(t, 2, 0, [u64::from(D), u64::from(OT)], 0x01E0_0488);
    0
}

fn step(fx: &mut Fx, sp: u32, v: &V) -> i32 {
    fx.camera_turn_roll(sp + 0x4D0);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x20);
    camera(fx, sp);
    fx.mem.write_u32(sp + 0x590, 0);
    fx.mem.write_u32(sp + 0x594, 0);
    let r = fx.mem.f(D + 0x40);
    fx.mem.setf(sp + 0x598, ee::neg(r));
    fx.mem.write_u32(sp + 0x59C, 0x3F80_0000);
    fx.vu_apply_to(sp + 0x590, sp + 0x4D0, sp + 0x590);
    fx.vu_add4(sp + 0x590, sp + 0x590, D + 0x20);
    let m = fx.read_m4(sp + 0x450);
    fx.project_at(sp + 0x590, &m, sp + 0x740, sp + 0x748);
    let z = (fx.mem.read_u64(sp + 0x740) << 8) >> 40;
    fx.mem.write_u32(D + 0x44, z as u32);
    // The node 0x36 point (D+0x50) at 0x82; every update the turn D+0x70
    // pointing from it to the target (0x100E40, then 0x4000 about x) and
    // the reach D+0xB0 = |target - point| [F 0x1E0053C..0x1E005CC].
    if fx.mem.i32(D) == 0x82 {
        let h = fx.mem.i32(D + 4);
        let s = fx.mem.i32(D + 0x48);
        fx.node_position(h, s, D + 0x50);
    }
    fx.vu_sub4(sp + 0x590, D + 0x30, D + 0x50);
    fx.point_along(D + 0x70, sp + 0x590);
    fx.rotate_axis(D + 0x70, 0x4000, 0);
    let pv = fx.mem.f(sp + 0x590);
    let x = fx.powf(pv, 2.0);
    let pv = fx.mem.f(sp + 0x594);
    let y = fx.powf(pv, 2.0);
    let f20 = ee::add(x, y);
    let pv = fx.mem.f(sp + 0x598);
    let z = fx.powf(pv, 2.0);
    let l = fx.sqrtf(ee::add(z, f20));
    fx.mem.setf(D + 0xB0, l);

    flash(fx, sp);
    walls(fx, sp, v);
    bursts(fx, sp, v);
    fountains(fx, sp, v);
    // a3 at the attacker pass's first TEXFLUSH: the last projection's
    // descriptor (0x205210's untouched argument), else 0x208400's d + 2.
    let a3 = if shards(fx, sp, v) { sp + 0x6B0 } else { sp + 0x6B2 };

    let n = fx.mem.i32(D);
    let s1 = Fx::clamp_i(0x182 - n, 0, 0x20);
    let c = Fx::clamp_i(n, 0, 0x20);
    let l = (fx.mult(s1, c.wrapping_neg().wrapping_mul(80)) >> 10) + 0x80;
    fx.set_stage_light(l & 0xFF);

    attacker_pass(fx, sp, a3, v);
    window(fx, sp, v);
    let args = motes(fx, sp, v);
    // The fade of a destroyed target (0x1F77A0(D+8, 2, (80, 80, 80, 4
    // clamp(n - 0xD4, 0, 0x20)))) from 0xD5 [F 0x1E03648..0x1E036A4].
    if fx.outcome(args) == 1 && fx.mem.i32(D) >= 0xE7 {
        let n = fx.mem.i32(D);
        let c = Fx::clamp_i(n - 0xE6, 0, 0x20);
        let a = (c << 7) >> 5;
        for k in 0..3u32 {
            fx.mem.write_u8(sp + 0x74C + k, 0x80);
        }
        fx.mem.write_u8(sp + 0x74F, a as u8);
        let t = fx.mem.i32(D + 8);
        fx.unit_add_mode(t, 2, sp + 0x74C, [u64::from(D), u64::from(OT)], v.fade_ra);
    }

    let n = fx.mem.i32(D);
    let mut ret = fx.mem.i32(sp + 0xC0);
    if n == 0xCC {
        ret = 1;
    } else {
        if n >= 0xD7 && n < 0x162 {
            ret = 6;
        }
        if n == 0x162 {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x1BF {
        ret = 8;
    }
    fx.mem.set32(sp + 0xC0, ret);
    ret
}

/// The camera's view-projection at sp + 0x4D0 times the identity at sp +
/// 0x510.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x450);
    fx.identity(sp + 0x490);
    crate::blib::exe::mul_mem(fx, sp + 0x450, sp + 0x490);
}

/// The camera at the attacker's point under the camera turn (sp + 0x550).
fn camera_at(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x450);
    fx.translate_rows(sp + 0x490, sp + 0x4D0, D + 0x20);
    crate::blib::exe::mul_mem(fx, sp + 0x450, sp + 0x490);
}

/// The camera in the turn D+0x70 moved to `t` (0x188420).
fn camera_turn(fx: &mut Fx, sp: u32, t: u32) {
    fx.camera_vp_to(sp + 0x450);
    fx.translate_rows(sp + 0x490, D + 0x70, t);
    crate::blib::exe::mul_mem(fx, sp + 0x450, sp + 0x490);
}

/// A full-window sprite at `d` (the XYZ2s at +0x18 / +0x20).
fn window_xy(fx: &mut Fx, d: u32) {
    far_xy(fx, d + 0x18, 0x7000, 0x7000);
    far_xy(fx, d + 0x20, 0x9000, 0x9000);
}

/// The white flash (D+0x60) from 0x82 [F 0x1E005D0..0x1E0072C].
fn flash(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x82 {
        fx.mem.write_u8(D + 0x60, 1);
        fx.mem.write_u32(D + 0x64, 0);
        fx.mem.write_u8(D + 0xC0E0, 0);
    }
    if fx.mem.read_u8(D + 0x60) != 1 {
        return;
    }
    let d = sp + 0x608;
    head(fx, d, 6, 3, 0x441, 0);
    alpha(fx, d, 1);
    let age = fx.mem.i32(D + 0x64);
    let t = fx.tan_falloff_p(age, 0x30, 1);
    let a = fix(fx, ee::mul(128.0, ee::sub(1.0, t)), 0x01E0_0668);
    rgba(fx, sp + 0x618, [0xFF, 0xF8, 0xE8], a as u8);
    window_xy(fx, d);
    fx.submit(d, OT, 0);
    let age = fx.mem.i32(D + 0x64) + 1;
    fx.mem.set32(D + 0x64, age);
    if age >= 0x31 {
        fx.mem.write_u8(D + 0x60, 0);
    }
}

/// The nine-point ring (D+0xC0) of radius `r`.
fn ring9(fx: &mut Fx, r: f32) {
    for s4 in 0..9i32 {
        let a = (s4 << 16) >> 3;
        let p = D + 0xC0 + 0x10 * s4 as u32;
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
    if fx.mem.i32(D) == 0x82 {
        fx.mem.write_u8(D + 0x1B90, 1);
        fx.mem.write_u32(D + 0x1B94, 0);
        // The spots are the turn's origin: the table sine / cosine are
        // called and their results dropped.
        for s4 in 0..2i32 {
            let r = D + 0x10 * s4 as u32;
            let a = (((s4 << 16) >> 1) + 0x4000) & 0xFFFF;
            fx.tsin(a);
            fx.mem.write_u32(r + 0x1BA0, 0);
            fx.tcos(a);
            fx.mem.write_u32(r + 0x1BA4, 0);
            fx.mem.write_u32(r + 0x1BA8, 0);
            fx.mem.write_u32(r + 0x1BAC, 0x3F80_0000);
        }
    }
    if fx.mem.read_u8(D + 0x1B90) != 1 {
        return;
    }
    // The rise clamp(age, 0, 0x40) is computed and unused: the walls stand
    // at their spots (0 + 1 (z - 0)).
    for a in 0..2u32 {
        let r = D + 0x10 * a;
        for w in 0..4u32 {
            let x = fx.mem.read_u32(r + 0x1BA0 + 4 * w);
            fx.mem.write_u32(r + 0x1BC0 + 4 * w, x);
        }
        let z = fx.mem.f(r + 0x1BA8);
        fx.mem.setf(r + 0x1BC8, ee::add(0.0, ee::mul(1.0, ee::sub(z, 0.0))));
    }
    // The ring offsets: radius 12 clamp(n - 0x82, 0, 0x10) / 16 times the
    // row's sin(0x5555 row / 20).
    let n = fx.mem.i32(D);
    let c = Fx::clamp_i(n - 0x82, 0, 0x10);
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
            let age = fx.mem.i32(D + 0x1B94);
            let a = (age.wrapping_mul(0x16C).wrapping_add(s4 + q) + 0x4000) & 0xFFFF;
            let p = fp + 0x10 * s6 as u32 + 0x18F0;
            let sn = fx.tsin(a);
            fx.mem.setf(p, ee::mul(f21, sn));
            fx.mem.write_u32(p + 4, 0);
            let cs = fx.tcos(a);
            fx.mem.setf(p + 8, ee::mul(f21, cs));
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        }
    }
    // The height: the reach D+0xB0 times the falloff of clamp(age, 0, 0x54).
    let age = fx.mem.i32(D + 0x1B94);
    let c = Fx::clamp_i(age, 0, 0x54);
    let f24 = ee::mul(fx.mem.f(D + 0xB0), fx.tan_falloff_p(c, 0x54, 1));
    let age = fx.mem.i32(D + 0x1B94);
    let c = Fx::clamp_i(0xD0 - age, 0, 0x20);
    let f22 = ee::div(ee::from_i32(c), 32.0);
    let q = sp + 0x6B0;
    quad_desc(fx, q);
    alpha(fx, q, 1);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(q, rh, 1);
    fx.desc_clamp(q, 0);
    rgba(fx, sp + 0x6C0, v.walls, 0x40);
    for o in [0x6D8u32, 0x6F0, 0x708] {
        copy8(fx, sp + o, sp + 0x6C0);
    }
    ring9(fx, ee::mul(5.0, f22));
    // a0..a3 at the first TEXFLUSH: 0x206F30's (desc, ot, key) and the
    // projection's desc after a drawn quad, else 0x205210's (in, out, m,
    // desc) of the rejected one.
    let mut last = [0u32; 4];
    for t in 0..2i32 {
        camera_turn(fx, sp, D + 0x50);
        grid(fx, t, f24);
        let age = fx.mem.i32(D + 0x1B94);
        let f1 = ee::div(ee::mul(2.0, ee::from_i32(rem2(age, 0x10))), 16.0);
        let f21 = ee::sub(ee::div(ee::from_i32(t), 2.0), f1);
        for s7 in 0..0x14i32 {
            let f23 = ee::add(ee::div(ee::mul(5.0, ee::from_i32(s7)), 20.0), f21);
            let f20 = ee::add(ee::div(ee::mul(5.0, ee::from_i32(s7 + 1)), 20.0), f21);
            for s6 in 0..8i32 {
                let f2 = ee::div(ee::mul(2.0, ee::from_i32(s6)), 8.0);
                let f0 = ee::div(ee::mul(2.0, ee::from_i32(s6 + 1)), 8.0);
                for (o, x) in [(0x6C8u32, f2), (0x6CC, f23), (0x6E0, f0), (0x6E4, f23), (0x6F8, f2), (0x6FC, f20), (0x710, f0), (0x714, f20)] {
                    fx.mem.setf(sp + o, x);
                }
                let vs = cell(s7, s6, 0x150);
                if fx.project_n(&vs, sp + 0x450, q, sp + 0x748, 0x01E0_0D08) == 1 {
                    let w = fx.mem.f(sp + 0x748);
                    let key = fix(fx, w, 0x01E0_0D1C);
                    fx.submit(q, OT, key as i32);
                    last = [q, OT, key, q];
                } else {
                    last = [fx.sp - 0x40, PROJ_OUT, sp + 0x450, q];
                }
            }
        }
    }
    quad_desc(fx, q);
    let layer = fx.layer_begin();
    let regs = crate::lens::frame_copy(fx, last, 1, sp + 0x728, sp + 0x730);
    fx.texflush(regs);
    alpha(fx, q, 0);
    fx.desc_clamp(q, LENS_CLAMP);
    let fl = fx.mem.read_u16(q + 2);
    fx.mem.write_u16(q + 2, fl | 0x800);
    rgba(fx, sp + 0x6C0, [v.lens[0], v.lens[1], v.lens[2]], v.lens[3]);
    for o in [0x6D8u32, 0x6F0, 0x708] {
        copy8(fx, sp + o, sp + 0x6C0);
    }
    ring9(fx, ee::mul(4.0, f22));
    for t in 0..2i32 {
        camera_turn(fx, sp, D + 0x50);
        for s7 in 0..0x15i32 {
            let f20 = ee::div(ee::mul(f24, ee::from_i32(s7)), 20.0);
            fx.identity(sp + 0x550);
            let ph = phase(fx, t);
            let fp = s7 << 16;
            fx.mult(0x6666_6667, fp);
            let q20 = (fx.hilo().1 >> 3).wrapping_add(((fp as u32) >> 31) as i32);
            let sn = fx.tsin((q20 + ph) & 0xFFFF);
            let rot = fixs(fx, ee::mul(910.0, sn), 0x01E0_0FE8);
            fx.rotate_axis(sp + 0x550, rot, 1);
            let src = D + 0x150 * t as u32 + 0x10 * s7 as u32 + 0x18F0;
            for s6 in 0..9i32 {
                let g = D + 0x90 * s7 as u32 + 0x10 * s6 as u32;
                let rp = D + 0xC0 + 0x10 * s6 as u32;
                for w in 0..4u32 {
                    let x = fx.mem.read_u32(rp + 4 * w);
                    fx.mem.write_u32(g + 0x150 + 4 * w, x);
                }
                fx.mem.setf(g + 0x154, f20);
                fx.vu_add4(g + 0x150, g + 0x150, src);
                fx.vu_apply_to(g + 0xD20, sp + 0x550, rp);
                let ph = phase(fx, t);
                let sn = fx.tsin((((s6 << 16) >> 3) + ph) & 0xFFFF);
                let f2 = ee::div(f24, 20.0);
                let f0 = ee::mul(ee::mul(f(0x3E4C_CCCD), f2), sn);
                fx.mem.setf(g + 0xD24, ee::add(f20, f0));
                fx.vu_add4(g + 0xD20, g + 0xD20, src);
            }
        }
        for s7 in 0..0x14i32 {
            for s6 in 0..8i32 {
                let vs = cell(s7, s6, 0xD20);
                if fx.project_n(&vs, sp + 0x450, q, sp + 0x748, 0x01E0_1170) != 1 {
                    continue;
                }
                for (u, x) in [(0x6C8u32, 0x6D0u32), (0x6E0, 0x6E8), (0x6F8, 0x700), (0x710, 0x718)] {
                    fx.uv_from_xy(sp + u, sp + x, 0, 0);
                }
                let vs = cell(s7, s6, 0x150);
                if fx.project_n(&vs, sp + 0x450, q, sp + 0x748, 0x01E0_11EC) == 1 && crate::b222::winding(fx, sp + 0x6D0, sp + 0x6E8, sp + 0x700) == 1 {
                    fx.submit(q, 0, 0);
                }
            }
        }
    }
    fx.layer_end(layer, OT, 1);
    tips(fx, sp, f24, v);
    let age = fx.mem.i32(D + 0x1B94) + 1;
    fx.mem.set32(D + 0x1B94, age);
    if age >= 0xD1 {
        fx.mem.write_u8(D + 0x1B90, 0);
    }
}

/// The glow at each wall's top (the last ring offset row over its spot,
/// raised by the height, turned by D+0x70 about D+0x50): a 30-wide sprite
/// (FF, 76, 4B, 128 clamp(0xD0 - age, 0, 0x20) / 32) of texture D+0x10
/// [F 0x1E0129C..0x1E014B8].
fn tips(fx: &mut Fx, sp: u32, f23: f32, v: &V) {
    let d = sp + 0x5B0;
    sprite_desc(fx, d, 1, 0x10, 0);
    let age = fx.mem.i32(D + 0x1B94);
    let c = Fx::clamp_i(0xD0 - age, 0, 0x20);
    let a = fix(fx, ee::mul(128.0, ee::div(ee::from_i32(c), 32.0)), 0x01E0_1344);
    rgba(fx, sp + 0x5C0, v.tips, a as u8);
    uv(fx, sp + 0x5C8, 0x600);
    uv(fx, sp + 0x5CA, 0);
    uv(fx, sp + 0x5D8, 0x800);
    uv(fx, sp + 0x5DA, 0x200);
    vec4(fx, sp + 0x510, -30.0, -30.0, 0.0);
    fx.mem.write_u32(sp + 0x518, 0);
    vec4(fx, sp + 0x520, 30.0, 30.0, 0.0);
    fx.mem.write_u32(sp + 0x528, 0);
    for t in 0..2u32 {
        fx.vu_add4(sp + 0x590, D + 0x150 * t + 0x1A30, D + 0x10 * t + 0x1BC0);
        let y = ee::add(fx.mem.f(sp + 0x594), f23);
        fx.mem.setf(sp + 0x594, y);
        fx.vu_apply_to(sp + 0x590, D + 0x70, sp + 0x590);
        fx.vu_add4(sp + 0x590, D + 0x50, sp + 0x590);
        fx.camera_vp_to(sp + 0x450);
        fx.identity(sp + 0x490);
        fx.translate_rows(sp + 0x490, sp + 0x4D0, sp + 0x590);
        crate::blib::exe::mul_mem(fx, sp + 0x450, sp + 0x490);
        if crate::fxlib2::project(fx, sp + 0x510, PROJ_OUT, sp + 0x450, d, sp + 0x748) {
            let w = fx.mem.f(sp + 0x748);
            let key = fix(fx, w, 0x01E0_1498);
            fx.submit(d, OT, key as i32);
        }
    }
}

/// (rem(age + 5 t, 64) << 16) >> 6: the wall's sway phase.
fn phase(fx: &mut Fx, t: i32) -> i32 {
    let age = fx.mem.i32(D + 0x1B94);
    (rem2(age + 5 * t, 0x40) << 16) >> 6
}

/// The wall grid (D+0x150, 21 rows of 9) of side `t`: the ring raised by
/// f24 s6 / 20 about the ring offsets.
fn grid(fx: &mut Fx, t: i32, f24: f32) {
    for s6 in 0..0x15i32 {
        let f20 = ee::div(ee::mul(f24, ee::from_i32(s6)), 20.0);
        let src = D + 0x150 * t as u32 + 0x10 * s6 as u32 + 0x18F0;
        for s1 in 0..9i32 {
            let g = D + 0x90 * s6 as u32 + 0x10 * s1 as u32 + 0x150;
            for w in 0..4u32 {
                let x = fx.mem.read_u32(D + 0xC0 + 0x10 * s1 as u32 + 4 * w);
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
    vec4(fx, sp + 0x510, ee::neg(s), ee::neg(s), 0.0);
    vec4(fx, sp + 0x520, s, s, 0.0);
    fx.mem.write_u32(sp + 0x518, 0);
    fx.mem.write_u32(sp + 0x528, 0);
    fx.vu_apply_to(sp + 0x510, sp + 0x4D0, sp + 0x510);
    fx.vu_apply_to(sp + 0x520, sp + 0x4D0, sp + 0x520);
}

/// Cell `c` of a 4 x 2 sprite atlas into the sprite's UVs.
fn atlas(fx: &mut Fx, sp: u32, c: i32) {
    let u = rem2(c, 4);
    let w = c >> 2;
    uv(fx, sp + 0x5C8, ((u << 9) & 0x3FFF) as u32);
    uv(fx, sp + 0x5CA, ((w << 9) & 0x3FFF) as u32);
    uv(fx, sp + 0x5D8, ((((u << 5) + 0x20) << 4) & 0x3FFF) as u32);
    uv(fx, sp + 0x5DA, ((((w << 5) + 0x20) << 4) & 0x3FFF) as u32);
}

/// The sprite at `p` (its corners about it at sp + 0x5B0 / 0x5C0).
fn place(fx: &mut Fx, sp: u32, p: u32) {
    fx.vu_add4(sp + 0x530, sp + 0x510, p);
    fx.vu_add4(sp + 0x540, sp + 0x520, p);
}

/// The bursts (D+0x1BE0, 16 x 0x150): 16 sprites flung from the target
/// along random table vectors [F 0x1E014DC..0x1E01A24].
fn bursts(fx: &mut Fx, sp: u32, v: &V) {
    let n = fx.mem.i32(D);
    if n >= 0xD7 && n < 0x152 {
        let r = D + 0x150 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(r + 0x1BE0, 1);
        fx.mem.write_u32(r + 0x1BE4, 1);
        const RAS: [[u32; 3]; 3] = [
            [0x01E0_1564, 0x01E0_1598, 0x01E0_15C4],
            [0x01E0_15F4, 0x01E0_1628, 0x01E0_1654],
            [0x01E0_1684, 0x01E0_16B8, 0x01E0_16E4],
        ];
        for s4 in 0..0x10u32 {
            let c = rem2(rnd(fx, 0x01E0_1540), 4) + 4;
            fx.mem.set32(r + 4 * s4 + 0x1BE8, c);
            for (k, ras) in RAS.iter().enumerate() {
                let a = rem2(rnd(fx, ras[0]), 4);
                let t = VEC_TABLE + 0x180 * a as u32;
                let b = rem2(rnd(fx, ras[1]), 0x20);
                let t = t.wrapping_add(12 * b as u32);
                let x = rnd(fx, ras[2]);
                let (_, m) = fx.div(x, 3);
                let e = fx.mem.f(t.wrapping_add(4 * m as u32));
                fx.mem.setf(r + 0x10 * s4 + 0x1C30 + 4 * k as u32, ee::mul(2.5, e));
            }
            fx.mem.write_u32(r + 0x10 * s4 + 0x1C3C, 0x3F80_0000);
        }
    }
    let d = sp + 0x5B0;
    sprite_desc(fx, d, 1, 0x1C, 5);
    camera(fx, sp);
    for k in 0..0x10u32 {
        let r = D + 0x150 * k;
        if fx.mem.read_u8(r + 0x1BE0) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 0x1BE4);
        let a = ((0x10 - age) << 7) >> 4;
        rgba(fx, sp + 0x5C0, v.bursts, a as u8);
        let age = ee::from_i32(fx.mem.i32(r + 0x1BE4));
        let s = ee::mul(18.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 16.0)));
        corners(fx, sp, s);
        let age = fx.mem.i32(r + 0x1BE4);
        let f20 = fx.tan_falloff_p(age, 0x10, 1);
        for s6 in 0..0x10u32 {
            fx.vu_scale4(sp + 0x590, r + 0x10 * s6 + 0x1C30, f20);
            fx.vu_add4(sp + 0x590, sp + 0x590, D + 0x30);
            place(fx, sp, sp + 0x590);
            let c = fx.mem.i32(r + 4 * s6 + 0x1BE8);
            atlas(fx, sp, c);
            if crate::fxlib2::project(fx, sp + 0x530, PROJ_OUT, sp + 0x450, d, sp + 0x748) {
                let w = fx.mem.f(sp + 0x748);
                let key = fix(fx, w, 0x01E0_19D0);
                fx.submit(d, OT, key as i32);
            }
        }
        let age = fx.mem.i32(r + 0x1BE4) + 1;
        fx.mem.set32(r + 0x1BE4, age);
        if age >= 0x11 {
            fx.mem.write_u8(r + 0x1BE0, 0);
        }
    }
}

/// The fountains (D+0x30E0, 16 x 0x250): on 0x83..0x151 two groups of 16
/// sprites from the node point D+0x50 along 1.2 table vectors (cells rand
/// % 8, 17 updates) [F 0x1E01A28..0x1E01E8C].
fn fountains(fx: &mut Fx, sp: u32, v: &V) {
    let n = fx.mem.i32(D);
    if n >= 0x83 && n < 0x152 {
        let r = D + 0x250 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(r + 0x30E0, 1);
        fx.mem.write_u32(r + 0x30E4, 1);
        for g in 0..2u32 {
            for w in 0..4u32 {
                let x = fx.mem.read_u32(D + 0x50 + 4 * w);
                fx.mem.write_u32(r + 0x10 * g + 0x3110 + 4 * w, x);
            }
            for s6 in 0..0x10u32 {
                let c = rem2(rnd(fx, 0x01E0_1AB8), 8);
                fx.mem.write_u8(r + 0x10 * g + s6 + 0x30E8, c as u8);
                fx.random_table_vec(r + 0x100 * g + 0x10 * s6 + 0x3130, f(0x3F99_999A));
            }
        }
    }
    let d = sp + 0x5B0;
    sprite_desc(fx, d, 1, 0x1C, 5);
    camera(fx, sp);
    for k in 0..0x10u32 {
        let r = D + 0x250 * k;
        if fx.mem.read_u8(r + 0x30E0) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 0x30E4);
        let a = if v.fountain3 { (((0x10 - age) * 3) << 4) >> 4 } else { ((0x10 - age) * 36) >> 4 };
        rgba(fx, sp + 0x5C0, v.fountains, a as u8);
        let agef = ee::from_i32(fx.mem.i32(r + 0x30E4));
        let s = ee::mul(10.0, ee::add(0.5, ee::div(ee::mul(0.5, agef), 16.0)));
        corners(fx, sp, s);
        let age = fx.mem.i32(r + 0x30E4);
        let t = fx.tan_falloff_p(age, 0x10, 1);
        let f20 = ee::add(f(0x3E4C_CCCD), ee::mul(f(0x3F4C_CCCD), t));
        for g in 0..2u32 {
            for s6 in 0..0x10u32 {
                fx.vu_scale4(sp + 0x590, r + 0x100 * g + 0x10 * s6 + 0x3130, f20);
                fx.vu_add4(sp + 0x590, sp + 0x590, r + 0x10 * g + 0x3110);
                place(fx, sp, sp + 0x590);
                let c = i32::from(fx.mem.read_u8(r + 0x10 * g + s6 + 0x30E8));
                atlas(fx, sp, c);
                if crate::fxlib2::project(fx, sp + 0x530, PROJ_OUT, sp + 0x450, d, sp + 0x748) {
                    fx.submit(d, OT, 0);
                }
            }
        }
        let age = fx.mem.i32(r + 0x30E4) + 1;
        fx.mem.set32(r + 0x30E4, age);
        if age >= 0x11 {
            fx.mem.write_u8(r + 0x30E0, 0);
        }
    }
}

/// The shards (D+0x5720, 12 x 0x350): 2 x 20 spinning three-panel shards
/// rising along the walls' ring offsets in the turn [F 0x1E01E90..0x1E02434].
fn shards(fx: &mut Fx, sp: u32, v: &V) -> bool {
    let n = fx.mem.i32(D);
    if n >= 0x83 && n < 0x152 {
        let (_, k) = fx.div(n, 0xC);
        let r = D.wrapping_add(fx.mult(k, 0x350) as u32);
        fx.mem.write_u8(r + 0x5720, 1);
        fx.mem.write_u32(r + 0x5724, 0);
        for s7 in 0..2u32 {
            for s6 in 0..0x14u32 {
                let x = rnd(fx, 0x01E0_1EF8);
                let (_, m) = fx.div(x, 3);
                fx.mem.write_u8(r + 0x14 * s7 + s6 + 0x5728, (m + 5) as u8);
                let c = rem2(rnd(fx, 0x01E0_1F1C), 0x100);
                fx.mem.set32(r + 0x50 * s7 + 4 * s6 + 0x5750, (c << 16) >> 8);
                let p = r + 0x140 * s7 + 0x10 * s6 + 0x57F0;
                fx.random_table_vec(p, 0.0);
                let y = ee::mul(fx.mem.f(p + 4), 2.0);
                fx.mem.setf(p + 4, y);
            }
        }
    }
    let q = sp + 0x6B0;
    quad_desc(fx, q);
    alpha(fx, q, 1);
    let rh = fx.mem.read_u32(D + 0x1C);
    fx.desc_texture(q, rh, 1);
    fx.desc_clamp(q, 5);
    let mut any = false;
    for k in 0..0xCu32 {
        let r = D + 0x350 * k;
        if fx.mem.read_u8(r + 0x5720) == 0 {
            continue;
        }
        any = true;
        let age = fx.mem.i32(r + 0x5724);
        let a = div12(fx, (0xC - age).wrapping_mul(96));
        rgba(fx, sp + 0x6C0, v.shards, a as u8);
        for o in [0x6D8u32, 0x6F0, 0x708] {
            copy8(fx, sp + o, sp + 0x6C0);
        }
        let agef = ee::from_i32(fx.mem.i32(r + 0x5724));
        let s = ee::add(f(0x3DCC_CCCD), ee::div(ee::mul(f(0x3F66_6666), agef), 12.0));
        vec4(fx, sp + 0x5A0, s, s, s);
        let age = fx.mem.i32(D + 0x1B94);
        let c = Fx::clamp_i(age - 1, 0, 0x54);
        let f20 = ee::mul(fx.mem.f(D + 0xB0), fx.tan_falloff_p(c, 0x54, 1));
        for t in 0..2u32 {
            for s6 in 0..0x14u32 {
                fx.vu_add4(sp + 0x590, D + 0x150 * t + 0x10 * (s6 + 1) + 0x18F0, D + 0x10 * t + 0x1BC0);
                fx.vu_add4(sp + 0x590, sp + 0x590, r + 0x140 * t + 0x10 * s6 + 0x57F0);
                let y = ee::add(fx.mem.f(sp + 0x594), ee::div(ee::mul(f20, ee::from_i32(s6 as i32 + 1)), 20.0));
                fx.mem.setf(sp + 0x594, y);
                fx.vu_apply_to(sp + 0x590, D + 0x70, sp + 0x590);
                fx.vu_add4(sp + 0x590, D + 0x50, sp + 0x590);
                fx.camera_vp_to(sp + 0x450);
                fx.identity(sp + 0x490);
                crate::blib::exe::mul_mem(fx, sp + 0x490, D + 0x70);
                fx.translate_rows(sp + 0x490, sp + 0x490, sp + 0x590);
                let age = fx.mem.i32(r + 0x5724);
                let spin = fx.mult(age, 0x3E7);
                let a = fx.mem.i32(r + 0x50 * t + 4 * s6 + 0x5750).wrapping_add(spin) & 0xFFFF;
                fx.rotate_axis(sp + 0x490, a, 1);
                fx.scale_rows(sp + 0x490, sp + 0x5A0);
                crate::blib::exe::mul_mem(fx, sp + 0x450, sp + 0x490);
                let b = u32::from(fx.mem.read_u8(r + 0x14 * t + s6 + 0x5728));
                for s1 in 0..3u32 {
                    let (s3, s5) = (D + 0x10 * b + 4 * s1, D + 8 * b);
                    for (o, src) in [(0x6C8u32, s3 + 0x55E0), (0x6CC, s5 + 0x5660), (0x6E0, s3 + 0x55E4), (0x6E4, s5 + 0x5660), (0x6F8, s3 + 0x55E0), (0x6FC, s5 + 0x5664), (0x710, s3 + 0x55E4), (0x714, s5 + 0x5664)] {
                        let x = fx.mem.read_u32(src);
                        fx.mem.write_u32(sp + o, x);
                    }
                    let (e0, e1) = (D + 0x20 * s1, D + 0x20 * (s1 + 1));
                    let vs = [e0 + 0x56A0, e1 + 0x56A0, e0 + 0x56B0, e1 + 0x56B0];
                    if fx.project_n(&vs, sp + 0x450, q, sp + 0x748, 0x01E0_231C) == 1 {
                        let w = fx.mem.f(sp + 0x748);
                        let key = fix(fx, w, 0x01E0_2330);
                        fx.submit(q, OT, key as i32);
                    }
                }
            }
        }
        let age = fx.mem.i32(r + 0x5724) + 1;
        fx.mem.set32(r + 0x5724, age);
        if age >= 0xD {
            fx.mem.write_u8(r + 0x5720, 0);
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

/// clamp(age, 0, 0x60) clamp(0x152 - age, 0, 0x60) / 9216.
fn pass_level(fx: &mut Fx) -> f32 {
    let age = fx.mem.i32(D + 0xC0E4);
    let c = ee::from_i32(Fx::clamp_i(age, 0, 0x60));
    let age = fx.mem.i32(D + 0xC0E4);
    let c2 = Fx::clamp_i(0x152 - age, 0, 0x60);
    ee::div(ee::mul(c, ee::from_i32(c2)), 9216.0)
}

/// The attacker drawn into its target with the callback and pulled
/// through 16 rings of quads and a fan below update 0x82, then its glow
/// sprite (D+0xC0E0, from 0x20) [F 0x1E02488..0x1E030F0].
fn attacker_pass(fx: &mut Fx, sp: u32, a3: u32, v: &V) {
    if fx.mem.i32(D) == 0x20 {
        fx.mem.write_u8(D + 0xC0E0, 1);
        fx.mem.write_u32(D + 0xC0E4, 0);
    }
    if fx.mem.read_u8(D + 0xC0E0) != 1 {
        return;
    }
    if fx.mem.i32(D) < 0x82 {
        pass_layer(fx, sp, a3, v);
    }
    let d = sp + 0x5B0;
    sprite_desc(fx, d, 1, 0x10, 0);
    camera_at(fx, sp);
    let f3 = pass_level(fx);
    let age = fx.mem.i32(D + 0xC0E4);
    let s = ee::add(f(0x3F4C_CCCD), ee::mul(f(0x3E4C_CCCD), ee::from_i32(rem2(age, 2))));
    let a = fix(fx, ee::mul(ee::mul(255.0, f3), s), 0x01E0_2F5C);
    rgba(fx, sp + 0x5C0, v.glow, a as u8);
    uv(fx, sp + 0x5C8, 0x600);
    uv(fx, sp + 0x5CA, 0);
    uv(fx, sp + 0x5D8, 0x800);
    uv(fx, sp + 0x5DA, 0x200);
    vec4(fx, sp + 0x510, -80.0, -80.0, 0.0);
    fx.mem.write_u32(sp + 0x518, 0);
    vec4(fx, sp + 0x520, 80.0, 80.0, 0.0);
    fx.mem.write_u32(sp + 0x528, 0);
    if crate::fxlib2::project(fx, sp + 0x510, PROJ_OUT, sp + 0x450, d, sp + 0x748) {
        far(fx, sp + 0x5D0, &[sp + 0x5E0]);
        fx.submit(d, OT, 0);
    }
    let age = fx.mem.i32(D + 0xC0E4) + 1;
    fx.mem.set32(D + 0xC0E4, age);
    if age >= 0x153 {
        fx.mem.write_u8(D + 0xC0E0, 0);
    }
}

fn pass_layer(fx: &mut Fx, sp: u32, a3: u32, v: &V) {
    let t = sp + 0x638;
    head(fx, t, 0x33, 9, 0x4_2142_1421, 0);
    let q = sp + 0x6B0;
    quad_desc(fx, q);
    let layer = fx.layer_begin();
    fx.texflush([0x2142_1421, t, 0x20, a3]);
    fx.mem.write_u32(D_STATIC, D);
    let (_, regs) = fx.unit_draw_post(1, sp + 0x698, sp + 0x6A0, CB, &mut |fx, p| callback(fx, p));
    copy8(fx, sp + 0x728, sp + 0x698);
    copy8(fx, sp + 0x730, sp + 0x6A0);
    fx.texflush([sp + 0x728, regs[1], regs[2], regs[3]]);
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
            let age = fx.mem.i32(D + 0xC0E4);
            let m = fx.mult(age, 0xD05);
            let a = m.wrapping_add(ak + s5) & 0xFFFF;
            let f20 = ee::add(1.0, ee::mul(0.5, fx.tsin(a)));
            let age = fx.mem.i32(D + 0xC0E4);
            let m = fx.mult(age, 0x8AE);
            let b = ak.wrapping_add(m) & 0xFFFF;
            let f0 = ee::mul(8192.0, fx.tsin(b));
            let ang = fixs(fx, ee::add(ee::from_i32(s4), f0), 0x01E0_272C) & 0xFFFF;
            let f20 = ee::mul(f21, f20);
            let p = rb + 0x10 * s1 as u32 + 0x9FE0;
            let sn = fx.tsin(ang);
            fx.mem.setf(p, ee::mul(f20, sn));
            let cs = fx.tcos(ang);
            fx.mem.setf(p + 4, ee::mul(f20, cs));
            fx.mem.write_u32(p + 8, 0);
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        }
    }
    for k in 0..3u32 {
        fx.mem.write_u32(sp + 0x590 + 4 * k, 0);
    }
    fx.mem.write_u32(sp + 0x59C, 0x3F80_0000);
    let f20 = pass_level(fx);
    for k in 0..0x10i32 {
        let rb = D + 0x210 * k as u32;
        if k == 0xF {
            let x = div17(fx, 128 * k);
            let a = fix(fx, ee::mul(f20, ee::from_i32(x)), 0x01E0_28A8);
            rgba(fx, sp + 0x648, v.pass, a as u8);
            let x = div17(fx, 128 * (k - 1));
            let a = fix(fx, ee::mul(f20, ee::from_i32(x)), 0x01E0_2910);
            rgba(fx, sp + 0x660, v.pass, a as u8);
            copy8(fx, sp + 0x678, sp + 0x660);
            for s3 in 0..0x20u32 {
                let (p0, p1) = (rb + 0x10 * s3, rb + 0x10 * (s3 + 1));
                if fx.project_n(&[sp + 0x590, p0 + 0x9FE0, p1 + 0x9FE0], sp + 0x450, t, sp + 0x748, 0x01E0_298C) != 1 {
                    continue;
                }
                for (u, x) in [(0x650u32, 0x658u32), (0x668, 0x670), (0x680, 0x688)] {
                    fx.uv_from_xy(sp + u, sp + x, 0, 0);
                }
                if fx.project_n(&[sp + 0x590, p0 + 0x7EE0, p1 + 0x7EE0], sp + 0x450, t, sp + 0x748, 0x01E0_29F0) == 1 {
                    let z = fx.mem.read_u32(D + 0x44);
                    set_z(fx, sp + 0x658, z);
                    copy_z(fx, sp + 0x670, sp + 0x658);
                    copy_z(fx, sp + 0x688, sp + 0x658);
                    fx.submit(t, 0, 0);
                }
            }
        } else {
            let x = div17(fx, 128 * k);
            let a = fix(fx, ee::mul(f20, ee::from_i32(x)), 0x01E0_2B00);
            rgba(fx, sp + 0x6C0, v.pass, a as u8);
            copy8(fx, sp + 0x6D8, sp + 0x6C0);
            let x = div17(fx, 128 * (k + 1));
            let a = fix(fx, ee::mul(f20, ee::from_i32(x)), 0x01E0_2B78);
            rgba(fx, sp + 0x6F0, v.pass, a as u8);
            copy8(fx, sp + 0x708, sp + 0x6F0);
            let rn = rb + 0x210;
            for s4 in 0..0x20u32 {
                let c = |base: u32| [rb + 0x10 * s4 + base, rb + 0x10 * (s4 + 1) + base, rn + 0x10 * s4 + base, rn + 0x10 * (s4 + 1) + base];
                if fx.project_n(&c(0x9FE0), sp + 0x450, q, sp + 0x748, 0x01E0_2C30) != 1 {
                    continue;
                }
                for (u, x) in [(0x6C8u32, 0x6D0u32), (0x6E0, 0x6E8), (0x6F8, 0x700), (0x710, 0x718)] {
                    fx.uv_from_xy(sp + u, sp + x, 0, 0);
                }
                if fx.project_n(&c(0x7EE0), sp + 0x450, q, sp + 0x748, 0x01E0_2CB0) == 1 {
                    let z = fx.mem.read_u32(D + 0x44);
                    set_z(fx, sp + 0x6D0, z);
                    for o in [0x6E8u32, 0x700, 0x718] {
                        copy_z(fx, sp + o, sp + 0x6D0);
                    }
                    fx.submit(q, 0, 0);
                }
            }
        }
    }
    fx.layer_end(layer, OT, 0);
}

/// The frame-textured window sprite while clamp(n - 0x82, 0, 0x10)
/// clamp(0x182 - n, 0, 0x10) / 256 > 0 [F 0x1E030F4..0x1E032E4].
fn window(fx: &mut Fx, sp: u32, v: &V) {
    let n = fx.mem.i32(D);
    let a = ee::from_i32(Fx::clamp_i(n - 0x82, 0, 0x10));
    let n = fx.mem.i32(D);
    let b = ee::from_i32(Fx::clamp_i(0x182 - n, 0, 0x10));
    let f20 = ee::div(ee::mul(a, b), 256.0);
    if crate::exe::ee_le(f20, 0.0) {
        return;
    }
    let d = sp + 0x5B0;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    let flags = u32::from(fx.mem.read_u16(d + 2));
    alpha(fx, d, 0);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.desc_clamp(d, 5);
    uv(fx, sp + 0x5C8, 0x140);
    uv(fx, sp + 0x5CA, 0x140);
    uv(fx, sp + 0x5D8, 0x1EC0);
    uv(fx, sp + 0x5DA, 0x1EC0);
    let a = fix(fx, ee::mul(64.0, f20), 0x01E0_3240);
    rgba(fx, sp + 0x5C0, v.window, a as u8);
    far_xy(fx, sp + 0x5D0, 0x7000, 0x7000);
    far_xy(fx, sp + 0x5E0, 0x9000, 0x9000);
    fx.submit(d, OT, 0);
}

/// The motes (D+0xC0F0, 24 x 0x60): on 0xD7..0x185 one record of 4
/// sprites flung from the target (D+0x30) along 2.0 table vectors (4 x 2
/// atlas of 32, cells rand % 8, 25 updates) [F 0x1E032E8..0x1E03824].
/// Returns a0..a3 as the loop's last callee leaves them (the outcome call's
/// arguments).
fn motes(fx: &mut Fx, sp: u32, v: &V) -> [u32; 4] {
    let n = fx.mem.i32(D);
    if n >= 0xD7 && n < 0x186 {
        let (_, k) = fx.div(n, 0x18);
        let r = D + 0x60 * k as u32;
        fx.mem.write_u8(r + 0xC0F0, 1);
        fx.mem.write_u32(r + 0xC0F4, 0);
        for w in 0..4u32 {
            let x = fx.mem.read_u32(D + 0x30 + 4 * w);
            fx.mem.write_u32(r + 0xC100 + 4 * w, x);
        }
        for i in 0..4u32 {
            let c = rem2(rnd(fx, 0x01E0_3388), 8);
            fx.mem.write_u8(r + i + 0xC0F8, c as u8);
            fx.random_table_vec(r + 0x10 * i + 0xC110, 2.0);
        }
    }
    let d = sp + 0x5B0;
    sprite_desc(fx, d, 0, 0x14, 5);
    camera(fx, sp);
    // With no record drawn: as the camera leaves them (0x20B370's a0 / a1,
    // 0x20E2B0's a2 = 0 and a3 past the camera's matrix) [run].
    let idx = fx.mem.read_u32(crate::world::CAMERA_STACK_INDEX);
    let cam = crate::world::CAMERA_RECORD.wrapping_add(idx.wrapping_mul(0x1A0)) + 0x40;
    let mut last = [sp + 0x450, sp + 0x490, 0, cam];
    for k in 0..0x18u32 {
        let r = D + 0x60 * k;
        if fx.mem.read_u8(r + 0xC0F0) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 0xC0F4);
        let x = age << 15;
        fx.mult(0x2AAA_AAAB, x);
        let a = (fx.hilo().1 >> 2).wrapping_add(((x as u32) >> 31) as i32);
        let sn = fx.tsin(a);
        let a = fix(fx, ee::mul(v.mote_amp, sn), 0x01E0_34F4);
        rgba(fx, sp + 0x5C0, [0x0C, 0x08, 0x08], a as u8);
        let agef = ee::from_i32(fx.mem.i32(r + 0xC0F4));
        let s = ee::mul(15.0, ee::add(0.5, ee::div(ee::mul(0.5, agef), 24.0)));
        corners(fx, sp, s);
        let agef = ee::from_i32(fx.mem.i32(r + 0xC0F4));
        let f20 = ee::add(f(0x3ECC_CCCD), ee::div(ee::mul(f(0x3F19_999A), agef), 24.0));
        let f21 = ee::add(-15.0, ee::div(ee::mul(40.0, agef), 24.0));
        for i in 0..4u32 {
            fx.vu_scale4(sp + 0x590, r + 0x10 * i + 0xC110, f20);
            fx.vu_add4(sp + 0x590, sp + 0x590, r + 0xC100);
            let y = ee::add(fx.mem.f(sp + 0x594), f21);
            fx.mem.setf(sp + 0x594, y);
            place(fx, sp, sp + 0x590);
            let c = i32::from(fx.mem.read_u8(r + i + 0xC0F8));
            atlas(fx, sp, c);
            if crate::fxlib2::project(fx, sp + 0x530, PROJ_OUT, sp + 0x450, d, sp + 0x748) {
                if v.mote_keyed {
                    let w = fx.mem.f(sp + 0x748);
                    let key = fix(fx, w, 0x01E0_3764);
                    fx.submit(d, OT, key as i32);
                    last = [d, OT, key, 0];
                } else {
                    fx.submit(d, OT, 0);
                    last = [d, OT, 0, 0];
                }
            } else {
                last = [sp + 0x530, PROJ_OUT, sp + 0x450, d];
            }
        }
        let age = fx.mem.i32(r + 0xC0F4) + 1;
        fx.mem.set32(r + 0xC0F4, age);
        if age >= 0x19 {
            fx.mem.write_u8(r + 0xC0F0, 0);
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
