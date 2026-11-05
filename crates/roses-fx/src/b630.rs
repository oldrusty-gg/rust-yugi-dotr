//! BMODULE b630 (card 50), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0x25E0
//! bytes) [F]: a single delay-slot-scheduled
//! function (beql / bnel) in a 0x4A0 frame (s0 = D, s1 = the OT). Init:
//! the handles, the attacker's side (0x100D00), textures (5, 0..3) D+0x10..
//! 0x1C, the target's point D+0x30 and two 33-point circle tables (D+0x90
//! + 0x20 k radii 10 / 20, D+0x1370 + 0x20 k radii 70 / 150). Every update
//! the camera turn with roll (sp + 0x1E0), the attacker's point D+0x20,
//! the point D+0x50 eased from it to the target's from update 0xFE over
//! 196 updates (scale D+0x60 = 1 + the ease) and the target's screen
//! point D+0x40 (pixels clamped to 0..512, Z, 1.0). Phases: update 0x208
//! a 0x39-update white full-screen flash (D+0x1D30) that clears the rings
//! / pulse / bursts / records on its eighth update; 0x20 a window sprite
//! (D+0x70, 0x249 updates) 160 in front of the attacker turned by the
//! camera side; update 0 thirty-two random rings of 32 segments (D+0x4B0,
//! 0x3200 updates) and a pulsing window (D+0xD60); updates 1..0x20F one
//! of 24 burst records (D+0xD70, three quads along random vectors for 25
//! updates); 0x185..0x20F one of 18 random-turned big rings (D+0x1790, 19
//! updates); 0x1C8 the fan of eight triangles from the screen point to the
//! nine fan points (frame-textured, 0xC80 updates); 0x208 128 sparkles
//! at the target (D+0x1D40, 64 updates); 0xFE a white flash fading over
//! 32 updates (D+0x25D0). The stage light 0x80 - 128 clamp(n) clamp(0x288
//! - n) / 1024. Returns 1 on 0x17A, 6 on 0x185..0x21F, 3 on 0x220, 8 once
//! the count reaches 0x289.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};
use crate::lens::{FAR_Z, KEEP, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70 / 0x205210's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
/// The fan's nine screen points (exe rodata, 0x10 apart).
const FAN: u32 = 0x0028_ABE0;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x4A0);
    fx.mem.write_u32(sp + 0xB0, 0);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x4A0);
    ret
}

/// 0x1F65D0(kind, h, out) with its frame (0x50: the caller's s0..s3 +0 ..
/// +0x30, `ra` +0x40) left below the caller's sp [F exe 0x1F65D0, as
/// b033's].
fn unit_point_saving(fx: &mut Fx, kind: i32, h: i32, out: u32, s: [u64; 4], ra: u32) -> i32 {
    let r = fx.unit_point(kind, h, out);
    let f = fx.sp - 0x50;
    fx.saved(f, &[(0, s[0]), (0x10, s[1]), (0x20, s[2]), (0x30, s[3]), (0x40, u64::from(ra))]);
    r
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x25E0);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    for k in 0..4u32 {
        let r = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x10 + 4 * k, r);
    }
    let t = fx.mem.i32(D + 8);
    unit_point_saving(fx, 1, t, D + 0x30, [u64::from(D), u64::from(OT), param as i64 as u64, 0], 0x01E0_014C);
    fx.mem.write_u8(D + 0x70, 0);
    circle(fx, D + 0x90, 10.0, 20.0);
    fx.mem.write_u8(D + 0x4B0, 0);
    fx.mem.write_u32(D + 0xD64, 0);
    for k in 0..0x18u32 {
        fx.mem.write_u8(D + 0xD70 + 0x40 * k, 0);
    }
    circle(fx, D + 0x1370, 70.0, 150.0);
    for k in 0..0x12u32 {
        fx.mem.write_u8(D + 0x1790 + 0x50 * k, 0);
    }
    for o in [0x1D30u32, 0x1D38, 0x1D40, 0x25D0] {
        fx.mem.write_u8(D + o, 0);
    }
    0
}

/// 33 points k (0x20 apart): (r0 s, r0 c, 0, 1), (r1 s, r1 c, 0, 1) of the
/// table sine / cosine of k << 11.
fn circle(fx: &mut Fx, at: u32, r0: f32, r1: f32) {
    for k in 0..0x21u32 {
        let a = ((k << 16) as i32) >> 5;
        let s = fx.table_trig(a, false);
        let c = fx.table_trig(a, true);
        let o = at + 0x20 * k;
        fx.mem.setf(o, ee::mul(r0, s));
        fx.mem.setf(o + 4, ee::mul(r0, c));
        fx.mem.write_u32(o + 8, 0);
        fx.mem.write_u32(o + 0xC, 0x3F80_0000);
        fx.mem.setf(o + 0x10, ee::mul(r1, s));
        fx.mem.setf(o + 0x14, ee::mul(r1, c));
        fx.mem.write_u32(o + 0x18, 0);
        fx.mem.write_u32(o + 0x1C, 0x3F80_0000);
    }
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

/// A random rotation angle: ((rand >> 23) % 256 << 16) >> 8.
fn rand_angle(fx: &mut Fx, ra: u32) -> i32 {
    let r = fx.rand_from(D, ra) >> 23;
    (rem2(r, 256) << 16) >> 8
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

fn copy_n(fx: &mut Fx, to: u32, from: u32, words: u32) {
    for c in 0..words {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

/// The far Z into the XYZ2 at `first`, then its Z into the others.
fn far_z(fx: &mut Fx, first: u32, others: &[u32]) {
    let v = fx.mem.read_u64(first);
    fx.mem.write_u64(first, (v & KEEP) | FAR_Z);
    let z = ((fx.mem.read_u64(first) << 8) >> 40) << 32;
    for &o in others {
        let w = fx.mem.read_u64(o);
        fx.mem.write_u64(o, (w & KEEP) | z);
    }
}

/// 0x1A2C70 (fixunssfsi) with its frame.
fn fixu(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

/// 0x1A2B80 (fixsfsi) of the float at `a` with its frame.
fn fix_at(fx: &mut Fx, a: u32, ra: u32) -> i32 {
    let f = fx.mem.f(a);
    fx.fix_residue(f, ra);
    ee::fixsfsi(f)
}

fn set_v4(fx: &mut Fx, a: u32, v: [f32; 3]) {
    fx.mem.setf(a, v[0]);
    fx.mem.setf(a + 4, v[1]);
    fx.mem.setf(a + 8, v[2]);
    fx.mem.write_u32(a + 0xC, 0x3F80_0000);
}

fn rgba(fx: &mut Fx, a: u32, c: [u8; 4]) {
    for (k, b) in c.into_iter().enumerate() {
        fx.mem.write_u8(a + k as u32, b);
    }
    fx.mem.write_u32(a + 4, 0x3F80_0000);
}

/// A descriptor header: count, flags, registers, +4 = 0.
fn desc(fx: &mut Fx, d: u32, n: u16, flags: u16, regs: u64) {
    fx.mem.write_u16(d + 2, flags);
    fx.mem.write_u16(d, n);
    fx.mem.write_u64(d + 8, regs);
    fx.mem.write_u32(d + 4, 0);
}

/// The sprite descriptor at sp + 0x330 (5 registers 0x42421, prim 0x26 |
/// 0x800).
fn sprite_desc(fx: &mut Fx, sp: u32) -> u32 {
    let d = sp + 0x330;
    desc(fx, d, 5, 0x26, 0x4_2421);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    d
}

/// The full-screen flash sprite (sp + 0x300, prim 6, 3 registers 0x441,
/// alpha preset 1, white) at far Z, key 0.
fn flash_sprite(fx: &mut Fx, sp: u32) {
    let d = sp + 0x300;
    desc(fx, d, 3, 6, 0x441);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
}

fn flash_sprite_draw(fx: &mut Fx, sp: u32, a: u32) {
    let d = sp + 0x300;
    fx.mem.write_u8(sp + 0x310, 0xFF);
    fx.mem.write_u8(sp + 0x311, 0xFF);
    fx.mem.write_u8(sp + 0x312, 0xFF);
    fx.mem.write_u8(sp + 0x313, a as u8);
    fx.mem.write_u32(sp + 0x314, 0x3F80_0000);
    for (o, xy) in [(0x318u32, 0x7000u16), (0x320, 0x9000)] {
        fx.mem.write_u16(sp + o, xy);
        fx.mem.write_u16(sp + o + 2, xy);
        let v = fx.mem.read_u64(sp + o);
        fx.mem.write_u64(sp + o, (v & KEEP) | FAR_Z);
        fx.mem.write_u8(sp + o + 7, 0);
    }
    fx.submit(d, OT, 0);
}

/// The view-projection at sp + 0x160 times the matrix at sp + 0x1A0.
fn vp_times(fx: &mut Fx, sp: u32) {
    let m = fx.read_m4(sp + 0x1A0);
    fx.mul_into(Some(sp + 0x160), &m);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.camera_turn_roll(sp + 0x1E0);
    let h = fx.mem.i32(D + 4);
    unit_point_saving(fx, 1, h, D + 0x20, [u64::from(D), u64::from(OT), param as i64 as u64, 0], 0x01E0_02D0);
    ease(fx, sp);
    flash(fx, sp);
    window(fx, sp);
    rings(fx, sp);
    pulse(fx, sp);
    bursts(fx, sp);
    big_rings(fx, sp);
    fan(fx, sp);
    sparkles(fx, sp);
    white(fx, sp);
    let n = fx.mem.i32(D);
    let a = Fx::clamp_i(0x288 - n, 0, 0x20);
    let b = Fx::clamp_i(fx.mem.i32(D), 0, 0x20);
    let v = fx.mult(a, b.wrapping_neg() << 7);
    fx.set_stage_light(((v >> 10) + 0x80) & 0xFF);
    let n = fx.mem.i32(D);
    if n == 0x17A {
        fx.mem.write_u32(sp + 0xB0, 1);
    } else {
        if (0x185..0x220).contains(&n) {
            fx.mem.write_u32(sp + 0xB0, 6);
        }
        if n == 0x220 {
            fx.mem.write_u32(sp + 0xB0, 3);
        }
    }
    let n = fx.mem.i32(D) + 1;
    fx.mem.set32(D, n);
    if n >= 0x289 {
        fx.mem.write_u32(sp + 0xB0, 8);
    }
    fx.mem.i32(sp + 0xB0)
}

/// [F 0x1E002D0..0x1E00490] D+0x50 eased from D+0x20 to D+0x30, scale
/// D+0x60, the target's screen point D+0x40.
fn ease(fx: &mut Fx, sp: u32) {
    let t = Fx::clamp_i(fx.mem.i32(D) - 0xFE, 0, 0xC4);
    let f1 = ee::div(ee::from_i32(t), 196.0);
    let x = ee::mul(32768.0, f1);
    fx.fix_residue(x, 0x01E0_0314);
    let a = ee::fixsfsi(x) & 0xFFFF;
    let c = fx.table_trig(a, true);
    let f20 = ee::mul(0.5, ee::sub(1.0, c));
    fx.vu_sub4(sp + 0x2E0, D + 0x30, D + 0x20);
    fx.vu_scale4(sp + 0x2E0, sp + 0x2E0, f20);
    fx.vu_add4(D + 0x50, sp + 0x2E0, D + 0x20);
    let s = ee::add(1.0, f20);
    fx.mem.setf(D + 0x60, s);
    fx.mem.setf(D + 0x64, s);
    fx.mem.setf(D + 0x68, s);
    fx.mem.write_u32(D + 0x6C, 0x3F80_0000);
    fx.camera_vp_to(sp + 0x160);
    fx.identity(sp + 0x1A0);
    vp_times(fx, sp);
    let m = fx.read_m4(sp + 0x160);
    fx.project_at(D + 0x30, &m, sp + 0x490, sp + 0x49C);
    let x = (i32::from(fx.mem.read_u16(sp + 0x490)) >> 4) - 0x700;
    fx.mem.setf(D + 0x40, ee::from_i32(x));
    let y = (i32::from(fx.mem.read_u16(sp + 0x492)) >> 4) - 0x700;
    fx.mem.setf(D + 0x44, ee::from_i32(y));
    let z = (fx.mem.read_u64(sp + 0x490) << 8) >> 40;
    fx.mem.setf(D + 0x48, ee::from_i32(z as i32));
    fx.mem.write_u32(D + 0x4C, 0x3F80_0000);
    for o in [0x40u32, 0x44] {
        if ee_lt(fx.mem.f(D + o), 0.0) {
            fx.mem.write_u32(D + o, 0);
        }
    }
    for o in [0x40u32, 0x44] {
        if !ee_le(fx.mem.f(D + o), 512.0) {
            fx.mem.write_u32(D + o, 0x4400_0000);
        }
    }
}

/// [F 0x1E00490..0x1E0066C] the flash D+0x1D30 (update 0x208, 0x39
/// updates); on its eighth the rings, pulse, bursts and records end.
fn flash(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x208 {
        fx.mem.write_u8(D + 0x1D30, 1);
        fx.mem.write_u32(D + 0x1D34, 0);
    }
    if fx.mem.read_u8(D + 0x1D30) != 1 {
        return;
    }
    if fx.mem.i32(D + 0x1D34) == 8 {
        fx.mem.write_u8(D + 0x1D38, 0);
        fx.mem.write_u8(D + 0x4B0, 0);
        fx.mem.write_u8(D + 0xD60, 0);
        for k in 0..0x18u32 {
            fx.mem.write_u8(D + 0xD70 + 0x40 * k, 0);
        }
        for k in 0..0x12u32 {
            fx.mem.write_u8(D + 0x1790 + 0x50 * k, 0);
        }
    }
    flash_sprite(fx, sp);
    let n = fx.mem.i32(D + 0x1D34);
    let t = Fx::clamp_i(n - 0x18, 0, 0x20);
    let f0 = fx.tan_falloff_p(t, 0x20, 1);
    let f20 = ee::sub(1.0, f0);
    let n = fx.mem.i32(D + 0x1D34);
    let r = Fx::ratio_below(ee::from_i32(n), 8.0);
    let a = fixu(fx, ee::mul(128.0, ee::mul(r, f20)), 0x01E0_05AC);
    flash_sprite_draw(fx, sp, a);
    let n = fx.mem.i32(D + 0x1D34) + 1;
    fx.mem.set32(D + 0x1D34, n);
    if n >= 0x39 {
        fx.mem.write_u8(D + 0x1D30, 0);
    }
}

/// [F 0x1E0066C..0x1E0099C] the window sprite D+0x70 (update 0x20, 0x249
/// updates) at D+0x80.
fn window(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x20 {
        fx.mem.write_u8(D + 0x70, 1);
        fx.mem.write_u32(D + 0x74, 0);
        let e = fx.camera_eye(0);
        copy_n(fx, sp + 0x2F0, e, 4);
        fx.mem.write_u32(sp + 0x2E0, 0);
        fx.mem.write_u32(sp + 0x2E4, 0);
        fx.mem.write_u32(sp + 0x2E8, 0xC320_0000);
        fx.mem.write_u32(sp + 0x2EC, 0x3F80_0000);
        copy_n(fx, sp + 0x2A0, sp + 0x1E0, 16);
        let side = ee::from_i32(fx.mem.i32(D + 0xC));
        let x = ee::mul(fx.mem.f(sp + 0x2F0), side);
        let a = if ee_lt(x, 0.0) { -0x4000 } else { 0x4000 };
        fx.rotate_axis(sp + 0x2A0, a, 1);
        fx.vu_apply_to(sp + 0x2E0, sp + 0x2A0, sp + 0x2E0);
        fx.vu_add4(D + 0x80, sp + 0x2E0, D + 0x20);
        let y = ee::add(fx.mem.f(D + 0x84), 100.0);
        fx.mem.setf(D + 0x84, y);
    }
    if fx.mem.read_u8(D + 0x70) != 1 {
        return;
    }
    let d = sprite_desc(fx, sp);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 5);
    fx.camera_vp_to(sp + 0x160);
    fx.identity(sp + 0x1A0);
    vp_times(fx, sp);
    let n = fx.mem.i32(D + 0x74);
    let f20 = Fx::ratio_below(ee::from_i32(n), 32.0);
    let n = fx.mem.i32(D + 0x74);
    let f0 = Fx::ramp_above(ee::from_i32(n), 584.0, 552.0);
    let a = fixu(fx, ee::mul(128.0, ee::mul(f20, f0)), 0x01E0_083C);
    fx.mem.write_u8(sp + 0x340, 0x80);
    fx.mem.write_u8(sp + 0x341, 0x80);
    fx.mem.write_u8(sp + 0x342, 0xFF);
    fx.mem.write_u8(sp + 0x343, a as u8);
    set_v4(fx, sp + 0x220, [-52.0, 52.0, 0.0]);
    fx.mem.write_u32(sp + 0x344, 0x3F80_0000);
    set_v4(fx, sp + 0x230, [52.0, -52.0, 0.0]);
    fx.vu_apply_to(sp + 0x220, sp + 0x1E0, sp + 0x220);
    fx.vu_apply_to(sp + 0x230, sp + 0x1E0, sp + 0x230);
    uv(fx, sp + 0x348, 0);
    uv(fx, sp + 0x34A, 0);
    uv(fx, sp + 0x358, 0x800);
    uv(fx, sp + 0x35A, 0x800);
    copy_n(fx, sp + 0x2E0, D + 0x80, 4);
    fx.vu_add4(sp + 0x260, sp + 0x220, sp + 0x2E0);
    fx.vu_add4(sp + 0x270, sp + 0x230, sp + 0x2E0);
    if crate::fxlib2::project(fx, sp + 0x260, PROJ_OUT, sp + 0x160, d, sp + 0x49C) {
        fx.submit(d, OT, 0xFF_FFFF);
    }
    let n = fx.mem.i32(D + 0x74) + 1;
    fx.mem.set32(D + 0x74, n);
    if n >= 0x249 {
        fx.mem.write_u8(D + 0x70, 0);
    }
}

/// [F 0x1E0099C..0x1E00EAC] update 0: 32 random rings (D+0x4B0, 0x3200
/// updates), each 32 segment quads on the D+0x90 table.
fn rings(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0 {
        fx.mem.write_u8(D + 0x4B0, 1);
        fx.mem.write_u32(D + 0x4B4, 0);
        for k in 0..0x20u32 {
            let r = fx.rand_from(D, 0x01E0_09C8) >> 23;
            fx.mem.set8(D + 0x4B8 + k, rem2(r, 8));
            let r = fx.rand_from(D, 0x01E0_09EC) >> 23;
            fx.mem.set32(D + 0x4D8 + 4 * k, rem2(r, 32));
            fx.identity(sp + 0x2A0);
            let a = rand_angle(fx, 0x01E0_0A18);
            fx.rotate_axis(sp + 0x2A0, a, 2);
            let a = rand_angle(fx, 0x01E0_0A4C);
            fx.rotate_axis(sp + 0x2A0, a, 1);
            let a = rand_angle(fx, 0x01E0_0A80);
            fx.rotate_axis(sp + 0x2A0, a, 0);
            copy_n(fx, D + 0x560 + 0x40 * k, sp + 0x2A0, 16);
        }
    }
    if fx.mem.read_u8(D + 0x4B0) != 1 {
        return;
    }
    let d = sp + 0x400;
    desc(fx, d, 0xC, 0x34, 0x4214_2142_1421);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, rh, 1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 0);
    let n = fx.mem.i32(D + 0x4B4);
    let f20 = Fx::ratio_below(ee::from_i32(n), 128.0);
    let n = fx.mem.i32(D + 0x4B4);
    let f0 = Fx::ramp_above(ee::from_i32(n), 12800.0, 12672.0);
    let a = fixu(fx, ee::mul(128.0, ee::mul(f20, f0)), 0x01E0_0BA0);
    fx.mem.write_u8(sp + 0x410, 0xE0);
    fx.mem.write_u8(sp + 0x411, 0xE8);
    fx.mem.write_u8(sp + 0x412, 0xFF);
    fx.mem.write_u8(sp + 0x413, a as u8);
    fx.mem.write_u32(sp + 0x414, 0x3F80_0000);
    copy_n(fx, sp + 0x428, sp + 0x410, 2);
    rgba(fx, sp + 0x440, [0, 0, 0x80, 0]);
    copy_n(fx, sp + 0x458, sp + 0x440, 2);
    for k in 0..0x20u32 {
        fx.camera_vp_to(sp + 0x160);
        fx.identity(sp + 0x1A0);
        fx.scale_rows(sp + 0x1A0, D + 0x60);
        fx.translate_rows(sp + 0x1A0, sp + 0x1A0, D + 0x50);
        copy_n(fx, sp + 0x2A0, D + 0x560 + 0x40 * k, 16);
        let n = fx.mem.i32(D + 0x4B4);
        fx.rotate_axis(sp + 0x2A0, n.wrapping_mul(33), 2);
        let n = fx.mem.i32(D + 0x4B4);
        fx.rotate_axis(sp + 0x2A0, n.wrapping_mul(44), 1);
        let n = fx.mem.i32(D + 0x4B4);
        let a = fx.mult(n, 0x22B);
        fx.rotate_axis(sp + 0x2A0, a, 0);
        let m = fx.read_m4(sp + 0x2A0);
        fx.mul_into(Some(sp + 0x1A0), &m);
        let s = ee::add(f32::from_bits(0x3F4C_CCCD), ee::div(ee::mul(f32::from_bits(0x3E4C_CCCD), ee::from_i32(k as i32 + 1)), 32.0));
        set_v4(fx, sp + 0x2E0, [s, s, s]);
        fx.scale_rows(sp + 0x1A0, sp + 0x2E0);
        vp_times(fx, sp);
        let n = fx.mem.i32(D + 0x4B4);
        let ph = rem2(n.wrapping_add(fx.mem.i32(D + 0x4D8 + 4 * k)), 32);
        let f2 = ee::div(ee::mul(32.0, ee::from_i32(ph)), 32.0);
        let cell = i32::from(fx.mem.read_u8(D + 0x4B8 + k));
        let f21 = ee::div(ee::add(ee::from_i32((cell & 3) << 5), f2), 128.0);
        let f2 = ee::from_i32((cell >> 2) << 5);
        let f22 = ee::div(f2, 64.0);
        let f20 = ee::div(ee::add(32.0, f2), 64.0);
        for j in 0..0x20u32 {
            fx.mem.setf(sp + 0x418, f21);
            fx.mem.setf(sp + 0x41C, f22);
            fx.mem.setf(sp + 0x430, f21);
            fx.mem.setf(sp + 0x434, f22);
            fx.mem.setf(sp + 0x448, f21);
            fx.mem.setf(sp + 0x44C, f20);
            fx.mem.setf(sp + 0x460, f21);
            fx.mem.setf(sp + 0x464, f20);
            let (a, b) = (D + 0x20 * j, D + 0x20 * (j + 1));
            let r = fx.project_n(&[a + 0x90, b + 0x90, a + 0xA0, b + 0xA0], sp + 0x160, d, sp + 0x49C, 0x01E0_0E44);
            if r == 1 {
                let w = fx.mem.f(sp + 0x49C);
                let key = fixu(fx, w, 0x01E0_0E58);
                fx.submit(d, OT, key as i32);
            }
        }
    }
    let n = fx.mem.i32(D + 0x4B4) + 1;
    fx.mem.set32(D + 0x4B4, n);
    if n >= 0x3201 {
        fx.mem.write_u8(D + 0x4B0, 0);
    }
}

/// [F 0x1E00EAC..0x1E0115C] update 0: the pulsing window D+0xD60 (0x3200
/// updates) at D+0x50.
fn pulse(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0 {
        fx.mem.write_u8(D + 0xD60, 1);
        fx.mem.write_u32(D + 0xD64, 0);
    }
    if fx.mem.read_u8(D + 0xD60) != 1 {
        return;
    }
    let d = sprite_desc(fx, sp);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(d, rh, 1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 5);
    fx.camera_vp_to(sp + 0x160);
    fx.identity(sp + 0x1A0);
    fx.scale_rows(sp + 0x1A0, D + 0x60);
    fx.translate_rows(sp + 0x1A0, sp + 0x1A0, D + 0x50);
    vp_times(fx, sp);
    let n = fx.mem.i32(D + 0xD64);
    let f0 = Fx::ratio_below(ee::from_i32(n), 128.0);
    let n = fx.mem.i32(D + 0xD64);
    let p = ee::add(f32::from_bits(0x3F4C_CCCD), ee::mul(f32::from_bits(0x3E4C_CCCD), ee::from_i32(rem2(n, 2))));
    let a = fixu(fx, ee::mul(128.0, ee::mul(p, f0)), 0x01E0_0FD0);
    fx.mem.write_u8(sp + 0x340, 0x80);
    fx.mem.write_u8(sp + 0x341, 0xA4);
    fx.mem.write_u8(sp + 0x342, 0xFF);
    fx.mem.write_u8(sp + 0x343, a as u8);
    set_v4(fx, sp + 0x220, [25.0, 25.0, 0.0]);
    fx.mem.write_u32(sp + 0x344, 0x3F80_0000);
    set_v4(fx, sp + 0x230, [-25.0, -25.0, 0.0]);
    fx.vu_apply_to(sp + 0x260, sp + 0x1E0, sp + 0x220);
    fx.vu_apply_to(sp + 0x270, sp + 0x1E0, sp + 0x230);
    uv(fx, sp + 0x348, 0);
    uv(fx, sp + 0x34A, 0x200);
    uv(fx, sp + 0x358, 0x200);
    uv(fx, sp + 0x35A, 0x400);
    if crate::fxlib2::project(fx, sp + 0x260, PROJ_OUT, sp + 0x160, d, sp + 0x49C) {
        far_z(fx, sp + 0x350, &[sp + 0x360]);
        fx.submit(d, OT, 0);
    }
    let n = fx.mem.i32(D + 0xD64) + 1;
    fx.mem.set32(D + 0xD64, n);
    if n >= 0x3201 {
        fx.mem.write_u8(D + 0xD60, 0);
    }
}

/// The quad descriptor at sp + 0x400 (12 registers, prim 0x34) with its
/// field pointers sp + 0xF0 / 0x100 / 0x110 [F 0x1E011EC].
fn quad_desc(fx: &mut Fx, sp: u32) -> u32 {
    let d = sp + 0x400;
    fx.mem.write_u16(d + 2, 0x34);
    fx.mem.write_u32(sp + 0xF0, d + 2);
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(sp + 0x100, d + 8);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u32(sp + 0x110, d + 4);
    d
}

/// [F 0x1E0115C..0x1E016F8] the bursts D+0xD70 + 0x40 k (one on each
/// update 1..0x20F at n % 24: three cells, three random vectors), each
/// three quads for 25 updates.
fn bursts(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n > 0 && n < 0x210 {
        let (_, e) = fx.div(n, 0x18);
        let rec = D + ((e as u32) << 6);
        fx.mem.write_u8(rec + 0xD70, 1);
        fx.mem.write_u32(rec + 0xD74, 0);
        for k in 0..3u32 {
            let r = fx.rand_from(D, 0x01E0_11A8) >> 23;
            fx.mem.set8(rec + 0xD78 + k, rem2(r, 8));
            fx.random_table_vec(rec + 0xD80 + 0x10 * k, 1.0);
        }
    }
    let d = quad_desc(fx, sp);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    let c = Fx::clamp_i(fx.mem.i32(D), 0, 0x80);
    let f20 = ee::div(ee::from_i32(c), 128.0);
    fx.mem.write_u32(sp + 0x130, 0);
    fx.mem.write_u32(sp + 0xD0, D);
    fx.mem.write_u32(sp + 0xE0, 0);
    for k in 0..0x18u32 {
        let rec = D + 0x40 * k;
        if fx.mem.read_u8(rec + 0xD70) != 0 {
            burst(fx, sp, rec, k, f20);
        }
        fx.mem.write_u32(sp + 0xD0, rec + 0x40);
        fx.mem.write_u32(sp + 0xE0, 0x10000 * (k + 1));
        fx.mem.write_u32(sp + 0x130, k + 1);
    }
}

fn burst(fx: &mut Fx, sp: u32, rec: u32, k: u32, f20: f32) {
    let d = sp + 0x400;
    let age = fx.mem.i32(rec + 0xD74);
    let v = (0x18 - age).wrapping_mul(80);
    fx.mult(0x2AAA_AAAB, v);
    let q = (fx.hilo().1 >> 2) + ((v as u32) >> 31) as i32;
    let a = fixu(fx, ee::mul(f20, ee::from_i32(q)), 0x01E0_1300);
    fx.mem.write_u8(sp + 0x410, 0x80);
    fx.mem.write_u8(sp + 0x411, 0x80);
    fx.mem.write_u8(sp + 0x412, 0xFF);
    fx.mem.write_u8(sp + 0x413, a as u8);
    fx.mem.write_u32(sp + 0x414, 0x3F80_0000);
    for o in [0x428u32, 0x440, 0x458] {
        copy_n(fx, sp + o, sp + 0x410, 2);
    }
    fx.camera_vp_to(sp + 0x160);
    copy_n(fx, sp + 0x2E0, D + 0x50, 4);
    crate::blib::exe::translation_into(fx, sp + 0x1A0, sp + 0x2E0);
    fx.scale_rows(sp + 0x1A0, D + 0x60);
    vp_times(fx, sp);
    let age = fx.mem.i32(rec + 0xD74);
    let f0 = fx.tan_falloff_p(age, 0x18, 1);
    let f21 = ee::mul(-3.5, ee::add(0.5, ee::mul(0.5, f0)));
    let base = (0x10000 * k) as i32;
    fx.mult(0x2AAA_AAAB, base);
    let c0 = (fx.hilo().1 >> 2) + ((base as u32) >> 31) as i32;
    fx.mem.set32(sp + 0xC0, c0);
    let fp = sp + 0x230;
    for j in 0..3u32 {
        let v = rec + 0xD80 + 0x10 * j;
        fx.point_along(sp + 0x2A0, v);
        let s3 = (0x10000 * j) as i32;
        fx.mult(0x5555_5556, s3);
        let t = fx.hilo().1 + ((s3 as u32) >> 31) as i32;
        let c0 = fx.mem.i32(sp + 0xC0);
        fx.rotate_axis(sp + 0x2A0, c0.wrapping_add(t), 2);
        set_v4(fx, sp + 0x220, [0.0, 2.0, 0.0]);
        set_v4(fx, fp, [0.0, -2.0, 0.0]);
        fx.vu_apply_to(sp + 0x220, sp + 0x2A0, sp + 0x220);
        fx.vu_apply_to(fp, sp + 0x2A0, fp);
        fx.vu_add4(sp + 0x260, sp + 0x220, v);
        fx.vu_add4(sp + 0x270, fp, v);
        fx.vu_scale4(sp + 0x2E0, v, f21);
        fx.vu_add4(sp + 0x280, sp + 0x260, sp + 0x2E0);
        fx.vu_add4(sp + 0x290, sp + 0x270, sp + 0x2E0);
        let cell = i32::from(fx.mem.read_u8(rec + 0xD78 + j));
        let f5 = ee::from_i32((cell & 3) << 5);
        let f6 = ee::div(f5, 128.0);
        let f2 = ee::from_i32((cell >> 2) << 5);
        let f4 = ee::div(ee::add(32.0, f5), 128.0);
        let f0 = ee::add(32.0, f2);
        let f2 = ee::div(f2, 64.0);
        let f0 = ee::div(f0, 64.0);
        fx.mem.setf(sp + 0x418, f6);
        fx.mem.setf(sp + 0x41C, f2);
        fx.mem.setf(sp + 0x430, f4);
        fx.mem.setf(sp + 0x434, f2);
        fx.mem.setf(sp + 0x448, f6);
        fx.mem.setf(sp + 0x44C, f0);
        fx.mem.setf(sp + 0x460, f4);
        fx.mem.setf(sp + 0x464, f0);
        let m = fx.read_m4(sp + 0x160);
        if fx.project_desc(sp + 0x260, PROJ_OUT, Some(&m), d, sp + 0x49C) {
            far_z(fx, sp + 0x420, &[sp + 0x438, sp + 0x450, sp + 0x468]);
            fx.submit(d, OT, 0);
        }
    }
    let age = fx.mem.i32(rec + 0xD74) + 1;
    fx.mem.set32(rec + 0xD74, age);
    if age >= 0x19 {
        fx.mem.write_u8(rec + 0xD70, 0);
    }
}

/// [F 0x1E016F8..0x1E01AA8] the big rings D+0x1790 + 0x50 k (one on each
/// update 0x185..0x20F at n % 18: a cell and a random-turned matrix at
/// D+0x50), each 32 quads on the D+0x1370 table for 19 updates.
fn big_rings(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0x185..0x210).contains(&n) {
        let (_, e) = fx.div(n, 0x12);
        let rec = D + 0x50 * e as u32;
        fx.mem.write_u8(rec + 0x1790, 1);
        fx.mem.write_u32(rec + 0x1794, 0);
        let r = fx.rand_from(D, 0x01E0_1748) >> 23;
        fx.mem.set8(rec + 0x1798, rem2(r, 8));
        fx.identity(sp + 0x2A0);
        fx.translate_rows(sp + 0x2A0, sp + 0x2A0, D + 0x50);
        let a = rand_angle(fx, 0x01E0_1788);
        fx.rotate_axis(sp + 0x2A0, a, 2);
        let a = rand_angle(fx, 0x01E0_17BC);
        fx.rotate_axis(sp + 0x2A0, a, 1);
        let a = rand_angle(fx, 0x01E0_17F0);
        fx.rotate_axis(sp + 0x2A0, a, 0);
        copy_n(fx, rec + 0x17A0, sp + 0x2A0, 16);
    }
    let d = sp + 0x400;
    let f = fx.mem.read_u32(sp + 0xF0);
    fx.mem.write_u16(f, 0x34);
    fx.mem.write_u16(d, 0xC);
    let r = fx.mem.read_u32(sp + 0x100);
    fx.mem.write_u64(r, 0x4214_2142_1421);
    let z = fx.mem.read_u32(sp + 0x110);
    fx.mem.write_u32(z, 0);
    let rh = fx.mem.read_u32(D + 0x1C);
    fx.desc_texture(d, rh, 1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 0);
    for k in 0..0x12u32 {
        let rec = D + 0x50 * k;
        if fx.mem.read_u8(rec + 0x1790) == 0 {
            continue;
        }
        let v = fx.mem.i32(rec + 0x1794) << 14;
        fx.mult(0x38E3_8E39, v);
        let q = (fx.hilo().1 >> 2) + ((v as u32) >> 31) as i32;
        let c = fx.table_trig(q, true);
        let a = fixu(fx, ee::mul(200.0, c), 0x01E0_1908);
        fx.mem.write_u8(sp + 0x410, 0xC6);
        fx.mem.write_u8(sp + 0x411, 0xC6);
        fx.mem.write_u8(sp + 0x412, 0xFF);
        fx.mem.write_u8(sp + 0x413, a as u8);
        fx.mem.write_u32(sp + 0x414, 0x3F80_0000);
        copy_n(fx, sp + 0x428, sp + 0x410, 2);
        rgba(fx, sp + 0x440, [0x40, 0x50, 0xFF, 0]);
        copy_n(fx, sp + 0x458, sp + 0x440, 2);
        fx.camera_vp_to(sp + 0x160);
        let s = ee::div(ee::from_i32(fx.mem.i32(rec + 0x1794)), 18.0);
        set_v4(fx, sp + 0x2E0, [s, s, s]);
        copy_n(fx, sp + 0x1A0, rec + 0x17A0, 16);
        fx.scale_rows(sp + 0x1A0, sp + 0x2E0);
        vp_times(fx, sp);
        for j in 0..0x20u32 {
            for (o, v) in [(0x418u32, 0u32), (0x41C, 0), (0x430, 0x3F80_0000), (0x434, 0), (0x448, 0), (0x44C, 0x3F80_0000), (0x460, 0x3F80_0000), (0x464, 0x3F80_0000)] {
                fx.mem.write_u32(sp + o, v);
            }
            let (a, b) = (D + 0x20 * j, D + 0x20 * (j + 1));
            let r = fx.project_n(&[a + 0x1370, b + 0x1370, a + 0x1380, b + 0x1380], sp + 0x160, d, sp + 0x49C, 0x01E0_1A40);
            if r == 1 {
                let w = fx.mem.f(sp + 0x49C);
                let key = fixu(fx, w, 0x01E0_1A54);
                fx.submit(d, OT, key as i32);
            }
        }
        let age = fx.mem.i32(rec + 0x1794) + 1;
        fx.mem.set32(rec + 0x1794, age);
        if age >= 0x13 {
            fx.mem.write_u8(rec + 0x1790, 0);
        }
    }
}

/// [F 0x1E01AA8..0x1E01F50] update 0x1C8: the fan D+0x1D38 (0xC80
/// updates): eight frame-textured triangles from the screen point D+0x40
/// to each pair of the fan points scaled about it by 1.1, texels the
/// unscaled pixels, grey (alpha 128 x age / 64 below 64), key 0.
fn fan(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x1C8 {
        fx.mem.write_u8(D + 0x1D38, 1);
        fx.mem.write_u32(D + 0x1D3C, 0);
    }
    if fx.mem.read_u8(D + 0x1D38) != 1 {
        return;
    }
    let d = sp + 0x388;
    desc(fx, d, 9, 0x33, 0x4_2142_1421);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, LENS_CLAMP);
    fx.frame_desc(d, 1, [d + 0x10, d + 2]);
    let n = fx.mem.i32(D + 0x1D3C);
    let r = Fx::ratio_below(ee::from_i32(n), 64.0);
    let a = fixu(fx, ee::mul(128.0, r), 0x01E0_1B70);
    rgba(fx, sp + 0x398, [0x80, 0x80, 0x80, a as u8]);
    copy_n(fx, sp + 0x3B0, sp + 0x398, 2);
    copy_n(fx, sp + 0x3C8, sp + 0x398, 2);
    fx.mem.write_u32(sp + 0x120, 0);
    let f20 = ee::add(1.0, ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(1)));
    for j in 0..8u32 {
        let (p0, p1) = (FAN + 0x10 * j, FAN + 0x10 * (j + 1));
        let x = fix_at(fx, D + 0x40, 0x01E0_1C20);
        fx.mem.set32(sp + 0x15C, x);
        let y = fix_at(fx, D + 0x44, 0x01E0_1C2C);
        uv(fx, sp + 0x3A0, (x << 4) as u32);
        uv(fx, sp + 0x3A2, (y << 4) as u32);
        let x = fix_at(fx, p0, 0x01E0_1C74);
        fx.mem.set32(sp + 0x158, x);
        let y = fix_at(fx, p0 + 4, 0x01E0_1C80);
        uv(fx, sp + 0x3B8, (x << 4) as u32);
        uv(fx, sp + 0x3BA, (y << 4) as u32);
        let x = fix_at(fx, p1, 0x01E0_1CC8);
        fx.mem.set32(sp + 0x154, x);
        let y = fix_at(fx, p1 + 4, 0x01E0_1CD4);
        uv(fx, sp + 0x3D0, (x << 4) as u32);
        uv(fx, sp + 0x3D2, (y << 4) as u32);
        fx.vu_sub4(sp + 0x260, p0, D + 0x40);
        fx.vu_sub4(sp + 0x270, p1, D + 0x40);
        fx.vu_scale4(sp + 0x260, sp + 0x260, f20);
        fx.vu_scale4(sp + 0x270, sp + 0x270, f20);
        fx.vu_add4(sp + 0x260, sp + 0x260, D + 0x40);
        fx.vu_add4(sp + 0x270, sp + 0x270, D + 0x40);
        for (src, ra, slot, at) in [(D + 0x40, [0x01E0_1D8Cu32, 0x01E0_1D98], 0x150u32, 0x3A8u32), (sp + 0x260, [0x01E0_1E08, 0x01E0_1E14], 0x14C, 0x3C0), (sp + 0x270, [0x01E0_1E84, 0x01E0_1E90], 0x148, 0x3D8)] {
            let x = fix_at(fx, src, ra[0]);
            fx.mem.set32(sp + slot, x);
            let y = fix_at(fx, src + 4, ra[1]);
            crate::lens::far_xy(fx, sp + at, ((x + 0x700) << 4) as u16, ((y + 0x700) << 4) as u16);
        }
        fx.submit(d, OT, 0);
    }
    fx.mem.write_u32(sp + 0x120, 1);
    let n = fx.mem.i32(D + 0x1D3C) + 1;
    fx.mem.set32(D + 0x1D3C, n);
    if n >= 0xC81 {
        fx.mem.write_u8(D + 0x1D38, 0);
    }
}

/// [F 0x1E01F50..0x1E022A8] update 0x208: 128 sparkles at the target
/// (D+0x1D40, 64 updates): cells D+0x1D48 + k, random vectors of 2
/// D+0x1DD0 + 0x10 k spread by 0.7 + 0.3 falloff, sprites of half size 4.
fn sparkles(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x208 {
        fx.mem.write_u8(D + 0x1D40, 1);
        fx.mem.write_u32(D + 0x1D44, 0);
        for k in 0..0x80u32 {
            let r = fx.rand_from(D, 0x01E0_1F7C) >> 23;
            fx.mem.set8(D + 0x1D48 + k, rem2(r, 4));
            fx.random_table_vec(D + 0x1DD0 + 0x10 * k, 2.0);
        }
    }
    if fx.mem.read_u8(D + 0x1D40) != 1 {
        return;
    }
    let d = sprite_desc(fx, sp);
    // a0 = 1 from the flag test's `addiu a0, zero, 1` [F 0x1E01FC4].
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    let n = fx.mem.i32(D + 0x1D44);
    let f0 = Fx::ramp_above(ee::from_i32(n), 64.0, 32.0);
    let a = fixu(fx, ee::mul(255.0, f0), 0x01E0_2058);
    fx.mem.write_u8(sp + 0x340, 0x64);
    fx.mem.write_u8(sp + 0x341, 0x64);
    fx.mem.write_u8(sp + 0x342, 0xFF);
    fx.mem.write_u8(sp + 0x343, a as u8);
    set_v4(fx, sp + 0x220, [4.0, 4.0, 0.0]);
    fx.mem.write_u32(sp + 0x344, 0x3F80_0000);
    let fp = sp + 0x230;
    set_v4(fx, fp, [-4.0, -4.0, 0.0]);
    fx.vu_apply_to(sp + 0x220, sp + 0x1E0, sp + 0x220);
    fx.vu_apply_to(fp, sp + 0x1E0, fp);
    fx.camera_vp_to(sp + 0x160);
    fx.identity(sp + 0x1A0);
    fx.translate_rows(sp + 0x1A0, sp + 0x1A0, D + 0x30);
    vp_times(fx, sp);
    let n = fx.mem.i32(D + 0x1D44);
    let f0 = fx.tan_falloff_p(n, 0x40, 1);
    let p = 0.3f64 * f64::from(f0);
    let f20 = (f64::from(f32::from_bits(0x3F33_3333)) + p) as f32;
    for k in 0..0x80u32 {
        let b = u32::from(fx.mem.read_u8(D + 0x1D48 + k));
        let (c, r) = (b & 3, b >> 2);
        uv(fx, sp + 0x348, c << 9);
        uv(fx, sp + 0x34A, r << 9);
        uv(fx, sp + 0x358, ((c << 5) + 0x20) << 4);
        uv(fx, sp + 0x35A, ((r << 5) + 0x20) << 4);
        fx.vu_scale4(sp + 0x2E0, D + 0x1DD0 + 0x10 * k, f20);
        fx.vu_add4(sp + 0x260, sp + 0x220, sp + 0x2E0);
        fx.vu_add4(sp + 0x270, fp, sp + 0x2E0);
        if crate::fxlib2::project(fx, sp + 0x260, PROJ_OUT, sp + 0x160, d, sp + 0x49C) {
            let w = fx.mem.f(sp + 0x49C);
            let key = fixu(fx, w, 0x01E0_2264);
            fx.submit(d, OT, key as i32);
        }
    }
    let n = fx.mem.i32(D + 0x1D44) + 1;
    fx.mem.set32(D + 0x1D44, n);
    if n >= 0x41 {
        fx.mem.write_u8(D + 0x1D40, 0);
    }
}

/// [F 0x1E022A8..0x1E023FC] update 0xFE: the white flash D+0x25D0
/// fading over 32 updates.
fn white(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0xFE {
        fx.mem.write_u8(D + 0x25D0, 1);
        fx.mem.write_u32(D + 0x25D4, 0);
    }
    if fx.mem.read_u8(D + 0x25D0) != 1 {
        return;
    }
    flash_sprite(fx, sp);
    let n = fx.mem.i32(D + 0x25D4);
    let f1 = ee::div(ee::from_i32(0x20 - n), 32.0);
    let a = fixu(fx, ee::mul(128.0, f1), 0x01E0_233C);
    flash_sprite_draw(fx, sp, a);
    let n = fx.mem.i32(D + 0x25D4) + 1;
    fx.mem.set32(D + 0x25D4, n);
    if n >= 0x21 {
        fx.mem.write_u8(D + 0x25D0, 0);
    }
}
