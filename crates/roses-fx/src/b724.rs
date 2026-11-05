//! BMODULE b724 (card 610), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0xA750
//! bytes) [F]: a single unscheduled function in a
//! 0x580 frame (D at sp + at(v, 0xD0), the OT in s0), b450 / b451's lens-disc
//! family (crates/roses-fx/src/b451.rs) with b451's helpers copied. Every
//! update the attacker's unit flag (0x1F69C0), the camera turn with roll
//! (sp + at(v, 0x280)), the attacker's point (0x1F65D0 kind 1, D+0x20) and node
//! 0x28 (D+0x40 -> D+0x50); on update 0x9A the node point is kept at
//! D+0x60. From update 0 for 0xC9 updates a lens: a 35-wide sprite (alpha
//! 255 ratio_below(age, 32) ramp_above(age, 200, 168), (80, 94, 9E), far
//! Z) at the attacker's point, then in a packet layer (frame copy) a disc
//! of 32 rings x 16 quads (radius 17.5 (32 - ring) / 32 at D+0x80) whose
//! UVs are the screen positions of a warped copy (D+0x2280: radius x (1 +
//! (ring + 1) / 32 x 0.05 cos(0x83F age + ...)), angle + (ring + 1) / 32 x
//! 364 cos(0x457 age + ...)), mesh (8A, 94, 94, 128 f). On 1..0x91 one of
//! 128 sprite records (D+0x4480, 0x40) at the node (one sprite, size 0.1 +
//! 0.9 rand / 256, a random 1.0 vector drawn out by clamp(age, 0, 128) /
//! 128 and raised 40 that), alpha 188 ratio_below(age, 12) ramp_above(age,
//! 128, 115), 5 (0.4 + 0.6 age / 128) wide, keyed by w, 128 updates. On
//! 0x91..0x9A one of 24 bursts (D+0x6480, 0x240) of 32 sprites at the node
//! (0.5 vectors, z = 5 side |z|), 10 (0.5 + 0.5 age / 24) wide, alpha 96
//! cos((age << 14) / 24), 25 updates. On 0x9A a blast (D+0x9A80) for 0x21
//! updates at D+0x60: 128 stretched quads (texture 1) along 1.5 vectors
//! (0x100E40 / 0x100E00 j 512, -2 x the vector, (FF, 78, 72) fading to 0,
//! alpha 100 ramp_above(age, 32, 16), in a matrix scaled 0.5 + 0.5
//! tan-falloff), a 50 (0.5 + 0.5 tan-falloff) glow (EB, DC, B4) and 64
//! sparks 9 (0.2 + 0.8 age / 32) wide (78, AA, C8) on 0.8 vectors scaled
//! by the soft double 0.1 + 0.9 tan-falloff. Returns 1 on 0x90, 2 on 0x9A,
//! 8 once the count reaches 0xBB.
use crate::blib::exe::mul_mem;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_z, KEEP, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
/// The mean-w slot.
const W: u32 = 0x57C;
/// The view-projection matrix / the model matrix / the camera turn.
const VP: u32 = 0x200;
const M: u32 = 0x240;
const CAM: u32 = 0x280;

/// What differs between b724 (card 610) and b727 (card 611).
struct V {
    frame: u32,
    /// Stack offsets (b724's) moved: >= 0x1C0 by `hi`, the loop locals
    /// 0x120..0x13F / 0x140..0x18F / 0x190..0x1BF by `lo`.
    hi: i32,
    lo: [i32; 3],
    bzero: u32,
    node: i32,
    /// The update the node point is kept at D+0x60, the blast starts and
    /// 2 is returned.
    key: i32,
    /// 1 on this update, 8 once the count reaches `end`.
    one: i32,
    end: i32,
    /// Sprite records spawn on 1..`spawn_to`; bursts on `bursts`.
    spawn_to: i32,
    bursts: (i32, i32),
    /// The lens sprite's point raised this far (b727 7.0).
    lift: Option<f32>,
    /// The warp's radius wobble and angle swing.
    wobble: f32,
    swing: f32,
    /// Colours: the lens sprite, the mesh, the records' sprites, the
    /// bursts, the blast quads, the glow, the sparks.
    rgb: [[u8; 3]; 7],
    /// b727: 32 sixteen-sprite records D+0xA750 on 0x96..0xB5.
    pillars: bool,
    /// Return addresses (the 21 calls of b724 in order; see the calls).
    ra: [u32; 21],
}

const B724: V = V {
    frame: 0x580,
    hi: 0,
    lo: [0, 0, 0],
    bzero: 0xA750,
    node: 0x28,
    key: 0x9A,
    one: 0x90,
    end: 0xBB,
    spawn_to: 0x92,
    bursts: (0x91, 0x9B),
    lift: None,
    wobble: f32::from_bits(0x3D4C_CCCD),
    swing: 364.0,
    rgb: [[0x80, 0x94, 0x9E], [0x8A, 0x94, 0x94], [0x80, 0x94, 0x9E], [0x80, 0x94, 0x9E], [0xFF, 0x78, 0x72], [0xEB, 0xDC, 0xB4], [0x78, 0xAA, 0xC8]],
    pillars: false,
    ra: [
        0x01E0_023C, 0x01E0_040C, 0x01E0_064C, 0x01E0_0804, 0x01E0_08F0, 0x01E0_0954, 0x01E0_0A8C, 0x01E0_0B08, 0x01E0_0CE0, 0x01E0_0D00, 0x01E0_0D5C, 0x01E0_0EB0, 0x01E0_1214, 0x01E0_1310, 0x01E0_1498,
        0x01E0_16D4, 0x01E0_17D4, 0x01E0_1834, 0x01E0_193C, 0x01E0_1D78, 0x01E0_2024,
    ],
};

/// b727 (card 611): b724 in a 0x5D0 frame, node 0x15, the key update
/// 0x95, returns on 0x8B / 0x95 / 0xD6, records to 0x8C, bursts on
/// 0x8C..0x96, the lens point 7 up, wobble 0.1 and swing 910, its own
/// colours, plus the pillars.
const B727: V = V {
    frame: 0x5D0,
    hi: 0x50,
    lo: [0xA0, -0x30, -0x20],
    bzero: 0xDD50,
    node: 0x15,
    key: 0x95,
    one: 0x8B,
    end: 0xD6,
    spawn_to: 0x8C,
    bursts: (0x8C, 0x96),
    lift: Some(7.0),
    wobble: f32::from_bits(0x3DCC_CCCD),
    swing: 910.0,
    rgb: [[0x80, 0x8A, 0x9E], [0xA8, 0xA8, 0xB2], [0x80, 0x8A, 0x9E], [0x80, 0x8A, 0x9E], [0xFF, 0xB4, 0x86], [0xEB, 0xDC, 0xC8], [0x78, 0x82, 0xC8]],
    pillars: true,
    ra: [
        0x01E0_0264, 0x01E0_0434, 0x01E0_068C, 0x01E0_0848, 0x01E0_0938, 0x01E0_099C, 0x01E0_0AD4, 0x01E0_0B50, 0x01E0_0D28, 0x01E0_0D48, 0x01E0_0DA4, 0x01E0_0F1C, 0x01E0_1288, 0x01E0_1380, 0x01E0_1508,
        0x01E0_174C, 0x01E0_184C, 0x01E0_18AC, 0x01E0_19B4, 0x01E0_1DE8, 0x01E0_2098,
    ],
};

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B724)
}

/// b727's entry 0x1E00080.
pub fn program_b727(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B727)
}

/// A stack local: b724's offset moved to the record's.
fn at(v: &V, x: u32) -> u32 {
    let d = match x {
        0x1C0.. => v.hi,
        0x120..=0x13F => v.lo[0],
        0x140..=0x18F => v.lo[1],
        0x190..=0x1BF => v.lo[2],
        _ => 0,
    };
    x.wrapping_add_signed(d)
}

fn run(fx: &mut Fx, param: i32, v: &V) -> i32 {
    let sp = fx.enter(v.frame);
    fx.mem.write_u32(sp + at(v, 0xD0), D);
    fx.mem.write_u32(sp + at(v, 0xB0), 0);
    let ret = if param >= 0 { init(fx, sp, v) } else { step(fx, sp, v) };
    fx.leave(v.frame);
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

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

fn copy16(fx: &mut Fx, to: u32, from: u32) {
    for c in 0..4u32 {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

fn copy8(fx: &mut Fx, to: u32, from: u32) {
    let a = fx.mem.read_u32(from);
    let b = fx.mem.read_u32(from + 4);
    fx.mem.write_u32(to, a);
    fx.mem.write_u32(to + 4, b);
}

/// 0x1A2C70: an unsigned integer from the float (with its residue frame).
fn alpha(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

/// 0x1A7230 fabsf (its argument left at its sp).
fn fabsf(fx: &mut Fx, x: f32) -> f32 {
    fx.mem.write_f32(fx.sp - 0x10, x);
    f32::from_bits(x.to_bits() & 0x7FFF_FFFF)
}

/// A 4 x 8 cell `b` of the texture: (u0, v0, u1, v1).
fn cell(b: u8) -> (u32, u32, u32, u32) {
    let (c, r) = (u32::from(b & 3), u32::from(b >> 2));
    ((c << 9) & 0x3FFF, (r << 9) & 0x3FFF, ((32 * c + 32) << 4) & 0x3FFF, ((32 * r + 32) << 4) & 0x3FFF)
}

/// The cell's UVs into a two-vertex sprite descriptor's UV halfwords.
fn cell_uvs(fx: &mut Fx, a: [u32; 4], b: u8) {
    let (u0, v0, u1, v1) = cell(b);
    uv(fx, a[0], u0);
    uv(fx, a[1], v0);
    uv(fx, a[2], u1);
    uv(fx, a[3], v1);
}

/// The sprite descriptor at sp + at(v, 0x398) (5 registers 0x42421, prim 0x26 |
/// 0x800).
fn sprite_head(fx: &mut Fx, d: u32) {
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
}

/// The colour (c, a, Q = 1.0) at `p`.
fn colour(fx: &mut Fx, p: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(p, c[0]);
    fx.mem.write_u8(p + 1, c[1]);
    fx.mem.write_u8(p + 2, c[2]);
    fx.mem.write_u8(p + 3, a);
    fx.mem.write_u32(p + 4, 0x3F80_0000);
}

fn vec4(fx: &mut Fx, p: u32, x: [u32; 4]) {
    for (k, w) in x.iter().enumerate() {
        fx.mem.write_u32(p + 4 * k as u32, *w);
    }
}

/// A square corner pair (s, s, 0, 1) at `a` and (-s, -s, 0, 1) at `b`.
fn square(fx: &mut Fx, a: u32, b: u32, s: f32) {
    fx.mem.setf(a, s);
    fx.mem.setf(a + 4, s);
    fx.mem.write_u32(a + 8, 0);
    fx.mem.write_u32(a + 0xC, 0x3F80_0000);
    let n = ee::neg(s);
    fx.mem.setf(b, n);
    fx.mem.setf(b + 4, n);
    fx.mem.write_u32(b + 8, 0);
    fx.mem.write_u32(b + 0xC, 0x3F80_0000);
}

/// The plain disc: 32 rings (0x110) of 17 points D+0x80, radius 17.5 (32
/// - ring) / 32, at the table sine / cosine of 0x1000 k.
fn init(fx: &mut Fx, sp: u32, v: &V) -> i32 {
    fx.bzero(D, v.bzero);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    for k in 0..2 {
        let r = fx.texture(5, k);
        fx.mem.write_u32(D + 0x10 + 4 * k as u32, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x30);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, v.node, 1);
    fx.mem.set32(D + 0x40, r);
    fx.mem.write_u8(D + 0x70, 0);
    for s0 in 0..0x20i32 {
        let f20 = ee::div(ee::mul(17.5, ee::from_i32(0x20 - s0)), 32.0);
        for s1 in 0..0x11i32 {
            let a = (s1 << 16) >> 4;
            let o = D + 0x80 + 0x110 * s0 as u32 + 0x10 * s1 as u32;
            let sn = fx.tsin(a);
            fx.mem.setf(o, ee::mul(f20, sn));
            let cs = fx.tcos(a);
            fx.mem.setf(o + 4, ee::mul(f20, cs));
            fx.mem.write_u32(o + 8, 0);
            fx.mem.write_u32(o + 0xC, 0x3F80_0000);
        }
    }
    for k in 0..0x80u32 {
        fx.mem.write_u8(D + 0x4480 + 0x40 * k, 0);
        if !v.pillars {
            fx.mem.write_u32(sp + at(v, 0xD0), D + 0x40 * (k + 1));
        }
    }
    if v.pillars {
        for k in 0..0x20u32 {
            fx.mem.write_u32(sp + at(v, 0xD0), D + 0x1B0 * (k + 1));
            fx.mem.write_u8(D + 0xA750 + 0x1B0 * k, 0);
        }
    }
    0
}

/// The warped disc D+0x2280 [F 0x1E00714..0x1E00888].
fn warp(fx: &mut Fx, sp: u32, v: &V) {
    fx.mem.write_u32(sp + at(v, 0xE0), D);
    fx.mem.write_u32(sp + at(v, 0xF0), 0);
    fx.mem.write_u32(sp + at(v, 0x100), 0);
    for fp in 0..0x20i32 {
        let s3 = fx.mem.read_u32(sp + at(v, 0xE0));
        let f21 = ee::div(ee::mul(17.5, ee::from_i32(0x20 - fp)), 32.0);
        let s7 = fx.mem.i32(sp + at(v, 0xF0)) >> 5;
        let s4 = fx.mem.i32(sp + at(v, 0x100)) >> 5;
        let f20 = ee::div(ee::from_i32(fp + 1), 32.0);
        for k in 0..0x11i32 {
            let (s1, s2) = (k * 0x20000, k * 0x10000);
            let age = fx.mem.i32(D + 0x74);
            let a = age.wrapping_mul(0x83F).wrapping_add(s7 + (s1 >> 4)) & 0xFFFF;
            let c = fx.tcos(a);
            let f0 = ee::mul(v.wobble, c);
            let age = fx.mem.i32(D + 0x74);
            let m = fx.mult(age, 0x457);
            let f22 = ee::add(1.0, ee::mul(f20, f0));
            let c = fx.tcos(s4.wrapping_add(m) & 0xFFFF);
            let f0 = ee::mul(v.swing, c);
            let f12 = ee::add(ee::from_i32(s2 >> 4), ee::mul(f20, f0));
            fx.fix_residue(f12, v.ra[3]);
            let s6 = ee::fixsfsi(f12) & 0xFFFF;
            let sn = fx.tsin(s6);
            let f22 = ee::mul(f21, f22);
            let o = s3 + 0x2280 + 0x10 * k as u32;
            fx.mem.setf(o, ee::mul(f22, sn));
            let cs = fx.tcos(s6);
            fx.mem.setf(o + 4, ee::mul(f22, cs));
            fx.mem.write_u32(o + 8, 0);
            fx.mem.write_u32(o + 0xC, 0x3F80_0000);
        }
        let x = fx.mem.i32(sp + at(v, 0xF0)).wrapping_add(0x20000);
        fx.mem.set32(sp + at(v, 0xF0), x);
        let x = fx.mem.i32(sp + at(v, 0x100)).wrapping_add(0x10000);
        fx.mem.set32(sp + at(v, 0x100), x);
        fx.mem.write_u32(sp + at(v, 0xE0), s3 + 0x110);
    }
}

/// The lens [F 0x1E002D8..0x1E00C5C] (D+0x70 = 1).
fn lens(fx: &mut Fx, sp: u32, v: &V) {
    let d1 = sp + at(v, 0x398);
    sprite_head(fx, d1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d1, a);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d1, rh, 1);
    fx.desc_clamp(d1, 0);
    uv(fx, sp + at(v, 0x3B0), 0);
    uv(fx, sp + at(v, 0x3B2), 0);
    uv(fx, sp + at(v, 0x3C0), 0x200);
    uv(fx, sp + at(v, 0x3C2), 0x200);
    square(fx, sp + at(v, 0x2C0), sp + at(v, 0x2D0), 35.0);
    let age = ee::from_i32(fx.mem.i32(D + 0x74));
    let f20 = Fx::ratio_below(age, 32.0);
    let age = ee::from_i32(fx.mem.i32(D + 0x74));
    let f20 = ee::mul(f20, Fx::ramp_above(age, 200.0, 168.0));
    let a = alpha(fx, ee::mul(255.0, f20), v.ra[1]);
    colour(fx, sp + at(v, 0x3A8), v.rgb[0], a as u8);
    let vp = sp + at(v, VP);
    fx.camera_vp_to(vp);
    copy16(fx, sp + at(v, 0x380), D + 0x20);
    if let Some(l) = v.lift {
        let y = ee::add(fx.mem.f(sp + at(v, 0x384)), l);
        fx.mem.setf(sp + at(v, 0x384), y);
    }
    fx.translate_rows(sp + at(v, M), sp + at(v, CAM), sp + at(v, 0x380));
    mul_mem(fx, vp, sp + at(v, M));
    // a2 / a3 at the first TEXFLUSH: 0x206F30 leaves them (as b451) [H].
    let mut a23 = [vp, d1];
    if crate::fxlib2::project(fx, sp + at(v, 0x2C0), PROJ_OUT, vp, d1, sp + at(v, W)) {
        far_z(fx, sp + at(v, 0x3B8), &[sp + at(v, 0x3C8)]);
        fx.submit(d1, OT, 0);
        a23 = [0, KEEP as u32];
    }
    // The lens descriptors: triangles sp + at(v, 0x3F0) (prim 0x33), quads
    // sp + at(v, 0x468) (prim 0x34), each vertex RGBAQ / UV / XYZ2.
    let (d2, d3) = (sp + at(v, 0x3F0), sp + at(v, 0x468));
    fx.mem.write_u16(d2 + 2, 0x33);
    fx.mem.write_u16(d2, 9);
    fx.mem.write_u64(d2 + 8, 0x4_2142_1421);
    fx.mem.write_u32(d2 + 4, 0);
    fx.mem.write_u16(d3 + 2, 0x34);
    fx.mem.write_u16(d3, 0xC);
    fx.mem.write_u64(d3 + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d3 + 4, 0);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + at(v, 0xCC), layer);
    let (t0, t1) = (sp + at(v, 0x450), sp + at(v, 0x458));
    let regs = crate::lens::frame_copy(fx, [0x2142_1421, d2, a23[0], a23[1]], 1, t0, t1);
    let w = fx.mem.read_u64(t0);
    fx.mem.write_u64(sp + at(v, 0x4E0), w);
    let w = fx.mem.read_u64(t1);
    fx.mem.write_u64(sp + at(v, 0x4E8), w);
    fx.texflush([sp + at(v, 0x4E0), regs[1], regs[2], regs[3]]);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d2, a);
    fx.desc_clamp(d2, LENS_CLAMP);
    let f = fx.mem.read_u16(d2 + 2);
    fx.mem.write_u16(d2 + 2, f | 0x800);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d3, a);
    fx.desc_clamp(d3, LENS_CLAMP);
    let a = alpha(fx, ee::mul(128.0, f20), v.ra[2]);
    let f = fx.mem.read_u16(d3 + 2);
    fx.mem.write_u16(d3 + 2, f | 0x800);
    colour(fx, sp + at(v, 0x400), v.rgb[1], a as u8);
    copy8(fx, sp + at(v, 0x418), sp + at(v, 0x400));
    copy8(fx, sp + at(v, 0x430), sp + at(v, 0x400));
    copy8(fx, sp + at(v, 0x570), sp + at(v, 0x400));
    for o in [0x478u32, 0x490, 0x4A8, 0x4C0] {
        copy8(fx, sp + at(v, o), sp + at(v, 0x570));
    }
    warp(fx, sp, v);
    let c = sp + at(v, 0x380);
    vec4(fx, c, [0, 0, 0, 0x3F80_0000]);
    fx.mem.write_u32(sp + at(v, 0x1E0), 0);
    let w = sp + at(v, W);
    for i in 0..0x20u32 {
        let fp = D + 0x110 * i;
        if i == 0x1F {
            for s1 in 0..0x10u32 {
                let (s2, s4) = (fp + 0x10 * s1, fp + 0x10 * (s1 + 1));
                let r = fx.project_n(&[c, s2 + 0x2280, s4 + 0x2280], vp, d2, w, v.ra[4]);
                if r != 1 {
                    continue;
                }
                for (u, x) in [(0x408u32, 0x410u32), (0x420, 0x428), (0x438, 0x440)] {
                    fx.uv_from_xy(sp + at(v, u), sp + at(v, x), 0, 0);
                }
                let r = fx.project_n(&[c, s2 + 0x80, s4 + 0x80], vp, d2, w, v.ra[5]);
                if r == 1 {
                    far_z(fx, sp + at(v, 0x410), &[sp + at(v, 0x428), sp + at(v, 0x440)]);
                    fx.submit(d2, 0, 0);
                }
            }
        } else {
            let next = D + 0x110 * (i + 1);
            fx.mem.write_u32(sp + at(v, 0x180), next);
            for s6 in 0..0x10u32 {
                let (s2, s5) = (fp + 0x10 * s6, fp + 0x10 * (s6 + 1));
                let (s1, s4) = (next + 0x10 * s6, next + 0x10 * (s6 + 1));
                let r = fx.project_n(&[s2 + 0x2280, s5 + 0x2280, s1 + 0x2280, s4 + 0x2280], vp, d3, w, v.ra[6]);
                if r != 1 {
                    continue;
                }
                for (u, x) in [(0x480u32, 0x488u32), (0x498, 0x4A0), (0x4B0, 0x4B8), (0x4C8, 0x4D0)] {
                    fx.uv_from_xy(sp + at(v, u), sp + at(v, x), 0, 0);
                }
                let r = fx.project_n(&[s2 + 0x80, s5 + 0x80, s1 + 0x80, s4 + 0x80], vp, d3, w, v.ra[7]);
                if r == 1 {
                    far_z(fx, sp + at(v, 0x488), &[sp + at(v, 0x4A0), sp + at(v, 0x4B8), sp + at(v, 0x4D0)]);
                    fx.submit(d3, 0, 0);
                }
            }
        }
        fx.mem.write_u32(sp + at(v, 0x1E0), i + 1);
    }
    let layer = fx.mem.read_u32(sp + at(v, 0xCC));
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0x74) + 1;
    fx.mem.set32(D + 0x74, age);
    if age >= 0xC9 {
        fx.mem.write_u8(D + 0x70, 0);
    }
}

/// On 1..0x91 [F 0x1E00C7C]: record n % 128 of one sprite at the node.
fn spawn(fx: &mut Fx, n: i32, v: &V) {
    let rec = D + 0x40 * rem2(n, 0x80) as u32;
    fx.mem.write_u8(rec + 0x4480, 1);
    fx.mem.write_u32(rec + 0x4484, 0);
    copy16(fx, rec + 0x44A0, D + 0x50);
    let r = fx.rand_from(OT, v.ra[8]) >> 23;
    fx.mem.set8(rec + 0x4488, rem2(r, 4));
    let r = fx.rand_from(OT, v.ra[9]) >> 23;
    let f = ee::div(ee::mul(f32::from_bits(0x3F66_6666), ee::from_i32(rem2(r, 256))), 256.0);
    fx.mem.setf(rec + 0x448C, ee::add(f32::from_bits(0x3DCC_CCCD), f));
    let r = fx.rand_from(OT, v.ra[10]) >> 23;
    fx.mem.set32(rec + 0x4490, (rem2(r, 256) << 16) >> 8);
    fx.random_table_vec(rec + 0x44B0, 1.0);
}

/// The sprite records [F 0x1E00DA8..0x1E0127C]: prim 0x24 quads of four
/// corners.
fn sprites(fx: &mut Fx, sp: u32, v: &V) {
    let d4 = sp + at(v, 0x4F8);
    fx.mem.write_u16(d4 + 2, 0x24);
    fx.mem.write_u16(d4, 9);
    fx.mem.write_u64(d4 + 8, 0x4_2424_2421);
    fx.mem.write_u32(d4 + 4, 0);
    let f = fx.mem.read_u16(d4 + 2);
    fx.mem.write_u16(d4 + 2, f | 0x800);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d4, rh, 1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d4, a);
    fx.desc_clamp(d4, 5);
    let (vp, m, cam) = (sp + at(v, VP), sp + at(v, M), sp + at(v, CAM));
    fx.camera_vp_to(vp);
    fx.identity(m);
    mul_mem(fx, vp, m);
    let uvp = [sp + at(v, 0x512), sp + at(v, 0x522), sp + at(v, 0x532), sp + at(v, 0x542)];
    fx.mem.write_u32(sp + at(v, 0x120), uvp[0]);
    fx.mem.write_u32(sp + at(v, 0x130), uvp[1]);
    fx.mem.write_u32(sp + at(v, 0x1D0), 0);
    let (c0, c1, c2, c3) = (sp + at(v, 0x2C0), sp + at(v, 0x2D0), sp + at(v, 0x2E0), sp + at(v, 0x2F0));
    let (pos, w) = (sp + at(v, 0x380), sp + at(v, W));
    for i in 0..0x80u32 {
        let rec = D + 0x40 * i;
        if fx.mem.read_u8(rec + 0x4480) != 0 {
            let age = ee::from_i32(fx.mem.i32(rec + 0x4484));
            let f0 = Fx::ratio_below(age, 12.0);
            let f20 = ee::mul(188.0, f0);
            let age = ee::from_i32(fx.mem.i32(rec + 0x4484));
            let f0 = Fx::ramp_above(age, 128.0, 115.0);
            let a = alpha(fx, ee::mul(f20, f0), v.ra[11]);
            colour(fx, sp + at(v, 0x508), v.rgb[2], a as u8);
            let age = fx.mem.i32(rec + 0x4484);
            let c = Fx::clamp_i(age, 0, 0x80);
            let f22 = ee::div(ee::from_i32(c), 128.0);
            let f20 = ee::mul(40.0, f22);
            fx.mem.write_u32(sp + at(v, 0x110), rec + 0x44A0);
            let age = fx.mem.i32(rec + 0x4484);
            let f3 = ee::div(ee::mul(f32::from_bits(0x3F19_999A), ee::from_i32(age)), 128.0);
            let m = fx.mult(age, 0x15B3);
            let sz = fx.mem.f(rec + 0x448C);
            let fp = fx.mem.i32(rec + 0x4490).wrapping_add(m) & 0xFFFF;
            let f2 = ee::add(f32::from_bits(0x3ECC_CCCD), f3);
            let f1 = ee::mul(5.0, f2);
            let f21 = ee::mul(f1, sz);
            let sn = fx.tsin(fp);
            let f0 = ee::add(f32::from_bits(0x3F66_6666), ee::mul(f32::from_bits(0x3DCC_CCCD), sn));
            let f23 = ee::mul(f21, f0);
            let cs = fx.tcos(fp);
            let f0 = ee::add(f32::from_bits(0x3F66_6666), ee::mul(f32::from_bits(0x3DCC_CCCD), cs));
            let f0 = ee::mul(f21, f0);
            let n23 = ee::neg(f23);
            let n0 = ee::neg(f0);
            for (p, x, y) in [(c0, f23, f23), (c1, n23, f0), (c2, f0, n23), (c3, n0, n0)] {
                fx.mem.setf(p, x);
                fx.mem.setf(p + 4, y);
                fx.mem.write_u32(p + 8, 0);
                fx.mem.write_u32(p + 0xC, 0x3F80_0000);
            }
            for p in [c0, c1, c2, c3] {
                fx.vu_apply_to(p, cam, p);
            }
            let b = fx.mem.read_u8(rec + 0x4488);
            let (u0, v0, u1, v1) = cell(b);
            uv(fx, sp + at(v, 0x510), u0);
            uv(fx, uvp[0], v0);
            uv(fx, sp + at(v, 0x520), u1);
            uv(fx, uvp[1], v0);
            uv(fx, sp + at(v, 0x530), u0);
            uv(fx, uvp[2], v1);
            uv(fx, sp + at(v, 0x540), u1);
            uv(fx, uvp[3], v1);
            fx.vu_scale4(pos, rec + 0x44B0, f22);
            let y = ee::add(fx.mem.f(pos + 4), f20);
            fx.mem.setf(pos + 4, y);
            let p = fx.mem.read_u32(sp + at(v, 0x110));
            fx.vu_add4(pos, pos, p);
            fx.vu_add4(sp + at(v, 0x300), c0, pos);
            fx.vu_add4(sp + at(v, 0x310), c1, pos);
            fx.vu_add4(sp + at(v, 0x320), c2, pos);
            fx.vu_add4(sp + at(v, 0x330), c3, pos);
            if crate::fxlib2::project(fx, sp + at(v, 0x300), PROJ_OUT, vp, d4, w) {
                let wf = fx.mem.f(w);
                let key = alpha(fx, wf, v.ra[12]);
                fx.submit(d4, OT, key as i32);
            }
            let age = fx.mem.i32(rec + 0x4484) + 1;
            fx.mem.set32(rec + 0x4484, age);
            if age >= 0x80 {
                fx.mem.write_u8(rec + 0x4480, 0);
            }
        }
        fx.mem.set32(sp + at(v, 0x1D0), i as i32 + 1);
    }
}

/// On 0x91..0x9A [F 0x1E0129C]: burst n % 24 of 32 sprites at the node.
fn burst_spawn(fx: &mut Fx, n: i32, v: &V) {
    let (_, e) = fx.div(n, 0x18);
    let b = D + 0x240 * e as u32;
    fx.mem.write_u8(b + 0x6480, 1);
    fx.mem.write_u32(b + 0x6484, 0);
    copy16(fx, b + 0x64B0, D + 0x50);
    for j in 0..0x20u32 {
        let r = fx.rand_from(OT, v.ra[13]) >> 23;
        fx.mem.set8(b + 0x6488 + j, rem2(r, 8));
        let p = b + 0x64C0 + 0x10 * j;
        fx.random_table_vec(p, 0.5);
        let z = fx.mem.f(p + 8);
        let a = fabsf(fx, z);
        let side = ee::from_i32(fx.mem.i32(D + 0xC));
        fx.mem.setf(p + 8, ee::mul(5.0, ee::mul(side, a)));
    }
}

/// The bursts [F 0x1E01390..0x1E0178C].
fn bursts(fx: &mut Fx, sp: u32, v: &V) {
    let d1 = sp + at(v, 0x398);
    sprite_head(fx, d1);
    fx.mem.write_u32(sp + at(v, 0x190), d1 + 2);
    fx.mem.write_u32(sp + at(v, 0x1A0), d1 + 8);
    fx.mem.write_u32(sp + at(v, 0x1B0), d1 + 4);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d1, a);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d1, rh, 1);
    fx.desc_clamp(d1, 5);
    let (vp, m, cam) = (sp + at(v, VP), sp + at(v, M), sp + at(v, CAM));
    fx.camera_vp_to(vp);
    fx.identity(m);
    mul_mem(fx, vp, m);
    fx.mem.write_u32(sp + at(v, 0x1C0), 0);
    let uvs = [sp + at(v, 0x3B0), sp + at(v, 0x3B2), sp + at(v, 0x3C0), sp + at(v, 0x3C2)];
    let (c0, c1, pos, w) = (sp + at(v, 0x2C0), sp + at(v, 0x2D0), sp + at(v, 0x380), sp + at(v, W));
    for i in 0..0x18u32 {
        let b = D + 0x240 * i;
        fx.mem.write_u32(sp + at(v, 0x170), b);
        if fx.mem.read_u8(b + 0x6480) != 0 {
            let age = fx.mem.i32(b + 0x6484);
            let x = age << 14;
            fx.mult(0x2AAA_AAAB, x);
            let hi = fx.hilo().1;
            let ang = (hi >> 2).wrapping_add(((x as u32) >> 31) as i32);
            let c = fx.tcos(ang);
            let a = alpha(fx, ee::mul(96.0, c), v.ra[14]);
            colour(fx, sp + at(v, 0x3A8), v.rgb[3], a as u8);
            let age = ee::from_i32(fx.mem.i32(b + 0x6484));
            let f0 = ee::div(ee::mul(0.5, age), 24.0);
            let s = ee::mul(10.0, ee::add(0.5, f0));
            square(fx, c0, c1, s);
            fx.vu_apply_to(c0, cam, c0);
            fx.vu_apply_to(c1, cam, c1);
            fx.mem.write_u32(sp + at(v, 0x1F0), 0);
            let f4 = ee::from_i32(fx.mem.i32(b + 0x6484));
            let f2 = ee::div(ee::mul(f32::from_bits(0x3DCC_CCCD), f4), 24.0);
            for o in [0x140u32, 0x150, 0x160] {
                fx.mem.write_u32(sp + at(v, o), b);
            }
            let f20 = ee::add(f32::from_bits(0x3F66_6666), f2);
            let f21 = ee::add(0.5, ee::div(ee::mul(0.5, f4), 24.0));
            for j in 0..0x20u32 {
                let p = b + 0x64C0 + 0x10 * j;
                let x = ee::mul(fx.mem.f(p), f21);
                fx.mem.setf(pos, x);
                let y = ee::mul(fx.mem.f(p + 4), f21);
                fx.mem.setf(pos + 4, y);
                let z = ee::mul(fx.mem.f(p + 8), f20);
                fx.mem.setf(pos + 8, z);
                fx.vu_add4(pos, pos, b + 0x64B0);
                fx.vu_add4(sp + at(v, 0x300), c0, pos);
                fx.vu_add4(sp + at(v, 0x310), c1, pos);
                let by = fx.mem.read_u8(b + 0x6488 + j);
                cell_uvs(fx, uvs, by);
                if crate::fxlib2::project(fx, sp + at(v, 0x300), PROJ_OUT, vp, d1, w) {
                    let wf = fx.mem.f(w);
                    let key = alpha(fx, wf, v.ra[15]);
                    fx.submit(d1, OT, key as i32);
                }
            }
            for (o, d) in [(0x140u32, 0x200u32), (0x150, 0x20), (0x160, 0x10)] {
                fx.mem.write_u32(sp + at(v, o), b + d);
            }
            fx.mem.write_u32(sp + at(v, 0x1F0), 1);
            let age = fx.mem.i32(b + 0x6484) + 1;
            fx.mem.set32(b + 0x6484, age);
            if age >= 0x19 {
                fx.mem.write_u8(b + 0x6480, 0);
            }
        }
        fx.mem.write_u32(sp + at(v, 0x170), b + 0x240);
        fx.mem.write_u32(sp + at(v, 0x1C0), i + 1);
    }
}

/// On 0x9A [F 0x1E017A4]: the blast record D+0x9A80 (128 cells + 1.5
/// vectors, 64 cells + 0.8 vectors).
fn blast_spawn(fx: &mut Fx, v: &V) {
    fx.mem.write_u8(D + 0x9A80, 1);
    fx.mem.write_u32(D + 0x9A84, 0);
    for j in 0..0x80u32 {
        let r = fx.rand_from(OT, v.ra[16]) >> 23;
        fx.mem.set8(D + 0x9A88 + j, rem2(r, 8));
        fx.random_table_vec(D + 0x9B10 + 0x10 * j, 1.5);
    }
    for j in 0..0x40u32 {
        let r = fx.rand_from(OT, v.ra[17]) >> 23;
        fx.mem.set8(D + 0xA310 + j, rem2(r, 8));
        fx.random_table_vec(D + 0xA350 + 0x10 * j, f32::from_bits(0x3F4C_CCCD));
    }
}

/// 1 - the tan falloff of the blast's age over 32 (0x100D50 p 1).
fn falloff(fx: &mut Fx) -> f32 {
    let age = fx.mem.i32(D + 0x9A84);
    fx.tan_falloff_p(age, 0x20, 1)
}

/// The blast [F 0x1E018A4..0x1E02380].
fn blast(fx: &mut Fx, sp: u32, v: &V) {
    let (vp, m, cam, w) = (sp + at(v, VP), sp + at(v, M), sp + at(v, CAM), sp + at(v, W));
    // 128 stretched quads along the vectors.
    let d3 = sp + at(v, 0x468);
    fx.mem.write_u16(d3 + 2, 0x34);
    fx.mem.write_u16(d3, 0xC);
    fx.mem.write_u64(d3 + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d3 + 4, 0);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d3, a);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d3, rh, 1);
    fx.desc_clamp(d3, 5);
    let age = ee::from_i32(fx.mem.i32(D + 0x9A84));
    let f0 = Fx::ramp_above(age, 32.0, 16.0);
    let a = alpha(fx, ee::mul(100.0, f0), v.ra[18]);
    colour(fx, sp + at(v, 0x478), v.rgb[4], a as u8);
    copy8(fx, sp + at(v, 0x490), sp + at(v, 0x478));
    colour(fx, sp + at(v, 0x4A8), v.rgb[4], 0);
    copy8(fx, sp + at(v, 0x4C0), sp + at(v, 0x4A8));
    fx.camera_vp_to(vp);
    let t = std::array::from_fn(|c| fx.mem.f(D + 0x60 + 4 * c as u32));
    let mm = fx.trs(None, None, Some(t));
    fx.write_m4(m, &mm);
    let f0 = falloff(fx);
    let s = ee::add(0.5, ee::mul(0.5, f0));
    let p = sp + at(v, 0x380);
    fx.mem.setf(p, s);
    fx.mem.setf(p + 4, s);
    fx.mem.setf(p + 8, s);
    fx.mem.write_u32(p + 0xC, 0x3F80_0000);
    fx.scale_rows(m, p);
    mul_mem(fx, vp, m);
    let t = sp + at(v, 0x340);
    for j in 0..0x80u32 {
        let q = D + 0x9B10 + 0x10 * j;
        fx.point_along(t, q);
        fx.rotate_axis(t, ((j << 16) >> 7) as i32, 2);
        vec4(fx, sp + at(v, 0x2C0), [0, 0x3F80_0000, 0, 0x3F80_0000]);
        vec4(fx, sp + at(v, 0x2D0), [0, 0xBF80_0000, 0, 0x3F80_0000]);
        fx.vu_apply_to(sp + at(v, 0x2C0), t, sp + at(v, 0x2C0));
        fx.vu_apply_to(sp + at(v, 0x2D0), t, sp + at(v, 0x2D0));
        fx.vu_add4(sp + at(v, 0x300), sp + at(v, 0x2C0), q);
        fx.vu_add4(sp + at(v, 0x310), sp + at(v, 0x2D0), q);
        fx.vu_scale4(p, q, -2.0);
        fx.vu_add4(sp + at(v, 0x320), sp + at(v, 0x300), p);
        fx.vu_add4(sp + at(v, 0x330), sp + at(v, 0x310), p);
        let b = fx.mem.read_u8(D + 0x9A88 + j);
        let f5 = ee::from_i32(32 * i32::from(b & 3));
        let f2 = ee::from_i32(32 * i32::from(b >> 2));
        let u0 = ee::div(f5, 128.0);
        let u1 = ee::div(ee::add(32.0, f5), 128.0);
        let v1 = ee::div(ee::add(32.0, f2), 64.0);
        let v0 = ee::div(f2, 64.0);
        for (o, x) in [(0x480u32, u0), (0x484, v0), (0x498, u1), (0x49C, v0), (0x4B0, u0), (0x4B4, v1), (0x4C8, u1), (0x4CC, v1)] {
            fx.mem.setf(sp + at(v, o), x);
        }
        let mm = fx.read_m4(vp);
        if fx.project_desc(sp + at(v, 0x300), PROJ_OUT, Some(&mm), d3, w) {
            far_z(fx, sp + at(v, 0x488), &[sp + at(v, 0x4A0), sp + at(v, 0x4B8), sp + at(v, 0x4D0)]);
            fx.submit(d3, OT, 0);
        }
    }
    // The glow at D+0x60.
    let d1 = sp + at(v, 0x398);
    let uvs = [sp + at(v, 0x3B0), sp + at(v, 0x3B2), sp + at(v, 0x3C0), sp + at(v, 0x3C2)];
    sprite_head(fx, d1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d1, a);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d1, rh, 1);
    fx.desc_clamp(d1, 5);
    let age = ee::from_i32(fx.mem.i32(D + 0x9A84));
    let f0 = Fx::ramp_above(age, 32.0, 16.0);
    let a = alpha(fx, ee::mul(48.0, f0), v.ra[19]);
    colour(fx, sp + at(v, 0x3A8), v.rgb[5], a as u8);
    for (p, x) in uvs.iter().zip([0x600u32, 0, 0x800, 0x200]) {
        uv(fx, *p, x);
    }
    fx.camera_vp_to(vp);
    fx.identity(m);
    fx.translate_rows(m, m, D + 0x60);
    mul_mem(fx, vp, m);
    let f0 = falloff(fx);
    let s = ee::mul(50.0, ee::add(0.5, ee::mul(0.5, f0)));
    square(fx, sp + at(v, 0x2C0), sp + at(v, 0x2D0), s);
    fx.vu_apply_to(sp + at(v, 0x2C0), cam, sp + at(v, 0x2C0));
    fx.vu_apply_to(sp + at(v, 0x2D0), cam, sp + at(v, 0x2D0));
    if crate::fxlib2::project(fx, sp + at(v, 0x2C0), PROJ_OUT, vp, d1, w) {
        far_z(fx, sp + at(v, 0x3B8), &[sp + at(v, 0x3C8)]);
        fx.submit(d1, OT, 0);
    }
    // 64 sparks on the 0.8 vectors.
    sprite_head(fx, d1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d1, a);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d1, rh, 1);
    fx.desc_clamp(d1, 5);
    let age = ee::from_i32(fx.mem.i32(D + 0x9A84));
    let f0 = Fx::ramp_above(age, 32.0, 16.0);
    let a = alpha(fx, ee::mul(116.0, f0), v.ra[20]);
    colour(fx, sp + at(v, 0x3A8), v.rgb[6], a as u8);
    let age = ee::from_i32(fx.mem.i32(D + 0x9A84));
    let f2 = ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), age), 32.0);
    let s = ee::mul(9.0, ee::add(f32::from_bits(0x3E4C_CCCD), f2));
    square(fx, sp + at(v, 0x2C0), sp + at(v, 0x2D0), s);
    fx.vu_apply_to(sp + at(v, 0x2C0), cam, sp + at(v, 0x2C0));
    fx.vu_apply_to(sp + at(v, 0x2D0), cam, sp + at(v, 0x2D0));
    fx.camera_vp_to(vp);
    fx.identity(m);
    fx.translate_rows(m, m, D + 0x60);
    mul_mem(fx, vp, m);
    let f0 = falloff(fx);
    // 0x1A2C30 / 0x1A31D8 / 0x1A3118 / 0x1A38F8: (float) ((double) 0.1f +
    // 0.9 x (double) f0).
    let f20 = (f64::from(f32::from_bits(0x3DCC_CCCD)) + 0.9f64 * f64::from(f0)) as f32;
    for j in 0..0x40u32 {
        let b = fx.mem.read_u8(D + 0xA310 + j);
        cell_uvs(fx, uvs, b);
        fx.vu_scale4(p, D + 0xA350 + 0x10 * j, f20);
        fx.vu_add4(sp + at(v, 0x300), sp + at(v, 0x2C0), p);
        fx.vu_add4(sp + at(v, 0x310), sp + at(v, 0x2D0), p);
        if crate::fxlib2::project(fx, sp + at(v, 0x300), PROJ_OUT, vp, d1, w) {
            far_z(fx, sp + at(v, 0x3B8), &[sp + at(v, 0x3C8)]);
            fx.submit(d1, OT, 0);
        }
    }
    let age = fx.mem.i32(D + 0x9A84) + 1;
    fx.mem.set32(D + 0x9A84, age);
    if age >= 0x21 {
        fx.mem.write_u8(D + 0x9A80, 0);
    }
}

fn step(fx: &mut Fx, sp: u32, v: &V) -> i32 {
    crate::b369::unit_flag(fx, 1, u64::from(OT), v.ra[0]);
    fx.camera_turn_roll(sp + at(v, CAM));
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x20);
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x40));
    fx.node_position(h, slot, D + 0x50);
    if fx.mem.i32(D) == v.key {
        copy16(fx, D + 0x60, D + 0x50);
    }
    if fx.mem.i32(D) == 0 {
        fx.mem.write_u8(D + 0x70, 1);
        fx.mem.write_u32(D + 0x74, 0);
    }
    if fx.mem.read_u8(D + 0x70) == 1 {
        lens(fx, sp, v);
    }
    let n = fx.mem.i32(D);
    if n > 0 && n < v.spawn_to {
        spawn(fx, n, v);
    }
    sprites(fx, sp, v);
    let n = fx.mem.i32(D);
    if (v.bursts.0..v.bursts.1).contains(&n) {
        burst_spawn(fx, n, v);
    }
    bursts(fx, sp, v);
    if fx.mem.i32(D) == v.key {
        blast_spawn(fx, v);
    }
    if fx.mem.read_u8(D + 0x9A80) == 1 {
        blast(fx, sp, v);
    }
    if v.pillars {
        pillars(fx, sp, v);
    }
    let n = fx.mem.i32(D);
    if n == v.one {
        fx.mem.write_u32(sp + at(v, 0xB0), 1);
    } else if n == v.key {
        fx.mem.write_u32(sp + at(v, 0xB0), 2);
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= v.end {
        fx.mem.write_u32(sp + at(v, 0xB0), 8);
    }
    fx.mem.i32(sp + at(v, 0xB0))
}

/// b727's pillars [F 0x1E023FC..0x1E02ADC] (offsets in b727's frame): on
/// 0x96..0xB5 record n % 32 (D+0xA750, 0x1B0) of 16 sprites at D+0x60
/// (cells, sizes 0.1 + 0.9 rand / 256, phases, 1.0 vectors), drawn 32
/// updates: alpha 188 ratio_below(age, 3) ramp_above(age, 32, 28), 5 (0.4
/// + 0.6 age / 32) wide, the vector x (0.3 + 0.7 f) raised 10 f (f =
/// clamp(age, 0, 32) / 32), keyed by w.
fn pillars(fx: &mut Fx, sp: u32, v: &V) {
    let n = fx.mem.i32(D);
    if (0x96..0xB5).contains(&n) {
        let rec = D + 0x1B0 * rem2(n, 0x20) as u32;
        fx.mem.write_u8(rec + 0xA750, 1);
        fx.mem.write_u32(rec + 0xA754, 0);
        copy16(fx, rec + 0xA7F0, D + 0x60);
        for k in 0..0x10u32 {
            let r = fx.rand_from(OT, 0x01E0_24A4) >> 23;
            fx.mem.set8(rec + 0xA758 + k, rem2(r, 4));
            let r = fx.rand_from(OT, 0x01E0_24CC) >> 23;
            let f = ee::div(ee::mul(f32::from_bits(0x3F66_6666), ee::from_i32(rem2(r, 256))), 256.0);
            fx.mem.setf(rec + 0xA768 + 4 * k, ee::add(f32::from_bits(0x3DCC_CCCD), f));
            let r = fx.rand_from(OT, 0x01E0_2530) >> 23;
            fx.mem.set32(rec + 0xA7A8 + 4 * k, (rem2(r, 256) << 16) >> 8);
            fx.random_table_vec(rec + 0xA800 + 0x10 * k, 1.0);
        }
    }
    let d4 = sp + 0x548;
    fx.mem.write_u16(d4 + 2, 0x24);
    fx.mem.write_u16(d4, 9);
    fx.mem.write_u64(d4 + 8, 0x4_2424_2421);
    fx.mem.write_u32(d4 + 4, 0);
    let f = fx.mem.read_u16(d4 + 2);
    fx.mem.write_u16(d4 + 2, f | 0x800);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d4, rh, 1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d4, a);
    fx.desc_clamp(d4, 5);
    let (vp, m, cam, w) = (sp + 0x250, sp + 0x290, sp + 0x2D0, sp + 0x5CC);
    fx.camera_vp_to(vp);
    fx.identity(m);
    mul_mem(fx, vp, m);
    let c = [sp + 0x310, sp + 0x320, sp + 0x330, sp + 0x340];
    let pos = sp + 0x3D0;
    for i in 0..0x20u32 {
        fx.mem.write_u32(sp + 0x200, i);
        let rec = D + 0x1B0 * i;
        if fx.mem.read_u8(rec + 0xA750) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(rec + 0xA754));
        let f0 = Fx::ratio_below(age, 3.0);
        let f20 = ee::mul(188.0, f0);
        let age = ee::from_i32(fx.mem.i32(rec + 0xA754));
        let f0 = Fx::ramp_above(age, 32.0, 28.0);
        let a = alpha(fx, ee::mul(f20, f0), 0x01E0_2694);
        colour(fx, sp + 0x558, [0x80, 0x8A, 0x9E], a as u8);
        let age = fx.mem.i32(rec + 0xA754);
        let f3 = ee::div(ee::from_i32(Fx::clamp_i(age, 0, 0x20)), 32.0);
        let f21 = ee::mul(10.0, f3);
        let f20 = ee::add(f32::from_bits(0x3E99_999A), ee::mul(f32::from_bits(0x3F33_3333), f3));
        for k in 0..0x10u32 {
            let age = fx.mem.i32(rec + 0xA754);
            let f2 = ee::mul(f32::from_bits(0x3F19_999A), ee::from_i32(age));
            let mm = fx.mult(age, 0x15B3);
            let sz = fx.mem.f(rec + 0xA768 + 4 * k);
            let f3 = ee::div(f2, 32.0);
            let ph = fx.mem.i32(rec + 0xA7A8 + 4 * k).wrapping_add(mm) & 0xFFFF;
            let f1 = ee::mul(5.0, ee::add(f32::from_bits(0x3ECC_CCCD), f3));
            let f22 = ee::mul(f1, sz);
            let sn = fx.tsin(ph);
            let f23 = ee::mul(f22, ee::add(f32::from_bits(0x3F66_6666), ee::mul(f32::from_bits(0x3DCC_CCCD), sn)));
            let cs = fx.tcos(ph);
            let f0 = ee::mul(f22, ee::add(f32::from_bits(0x3F66_6666), ee::mul(f32::from_bits(0x3DCC_CCCD), cs)));
            let (n23, n0) = (ee::neg(f23), ee::neg(f0));
            for (p, x, y) in [(c[0], f23, f23), (c[1], n23, f0), (c[2], f0, n23), (c[3], n0, n0)] {
                fx.mem.setf(p, x);
                fx.mem.setf(p + 4, y);
                fx.mem.write_u32(p + 8, 0);
                fx.mem.write_u32(p + 0xC, 0x3F80_0000);
            }
            for p in c {
                fx.vu_apply_to(p, cam, p);
            }
            let b = fx.mem.read_u8(rec + 0xA758 + k);
            let (u0, v0, u1, v1) = cell(b);
            for (o, x) in [(0x560u32, u0), (0x562, v0), (0x570, u1), (0x572, v0), (0x580, u0), (0x582, v1), (0x590, u1), (0x592, v1)] {
                uv(fx, sp + o, x);
            }
            fx.vu_scale4(pos, rec + 0xA800 + 0x10 * k, f20);
            let y = ee::add(fx.mem.f(pos + 4), f21);
            fx.mem.setf(pos + 4, y);
            fx.vu_add4(pos, pos, rec + 0xA7F0);
            for (o, p) in [0x350u32, 0x360, 0x370, 0x380].iter().zip(c) {
                fx.vu_add4(sp + o, p, pos);
            }
            if crate::fxlib2::project(fx, sp + 0x350, PROJ_OUT, vp, d4, w) {
                let wf = fx.mem.f(w);
                let key = alpha(fx, wf, 0x01E0_2A54);
                fx.submit(d4, OT, key as i32);
            }
        }
        let age = fx.mem.i32(rec + 0xA754) + 1;
        fx.mem.set32(rec + 0xA754, age);
        if age >= 0x20 {
            fx.mem.write_u8(rec + 0xA750, 0);
        }
    }
    fx.mem.write_u32(sp + 0x200, 0x20);
    let _ = v;
}
