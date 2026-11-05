//! BMODULE b392 (card 574), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0xB4E0
//! bytes) [F], written cold from the project notes
//! (b237's family at 0.51: D spilled to sp + 0xD0, s0 the ordering table,
//! frame 0x480).
//!
//! Init: textures 0 / 1, the target's position D+0x20, node 0x1D (kind 1,
//! slot D+0x30). Every update the camera turn with roll (sp + 0x390), the
//! node into D+0x40. On 0x8C: 256 sparks D+0x50 (cells +8, 1.2 vectors
//! +0x110, 12 wide at the target, even ones grey preset 2 at 0.8 reach, odd
//! (FF, 50, 19)), 32 embers D+0x1160 (falling 8 (0.5 (-0.0109 age^2)), each
//! update writing a trail record: 16 x 0x650 at D+0x1390), the frame flash
//! D+0x7890 (b237's, (FF, 80, 80)). On 0x1F..0x69 puff records (32 x 0x170
//! at D+0x78A0) at the node, spinning by their phases. On 0x5D a cloud
//! D+0xA6A0 (64 sprites at the node, (80, 70, 64)) and a head D+0xAB00 (node
//! to target in 47 updates, dropping a spark record of 16 every update: 8
//! x 0x130 at D+0xAB60).
//! Returns 1 on 0x82, 2 on 0x8C, 8 from 0xBC.
use crate::b182::{alpha, clamp_regs, head, rgba};
use crate::blib::exe::mul_mem;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::far_xy;

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x480;
const VP: u32 = 0x310;
const MS: u32 = 0x350;
const TURN: u32 = 0x390;
const C0: u32 = 0x3D0;
const C1: u32 = 0x3E0;
const Q0: u32 = 0x3F0;
const Q1: u32 = 0x400;
const T: u32 = 0x410;
/// The sprite descriptor (prim 0x26 | 0x800, 5 registers).
const DESC: u32 = 0x420;
const W: u32 = 0x47C;

const SPARKS: u32 = 0x50;
const EMBERS: u32 = 0x1160;
const TRAILS: u32 = 0x1390;
const FLASH: u32 = 0x7890;
const PUFFS: u32 = 0x78A0;
const CLOUD: u32 = 0xA6A0;
const HEAD: u32 = 0xAB00;
const DROPS: u32 = 0xAB60;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xD0, D);
    fx.mem.write_u32(sp + 0xC0, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp) };
    fx.leave(FRAME);
    ret
}

fn vec4(fx: &mut Fx, p: u32, x: [f32; 4]) {
    for (k, c) in x.iter().enumerate() {
        fx.mem.setf(p + 4 * k as u32, *c);
    }
}

fn words(fx: &mut Fx, to: u32, from: u32, n: u32) {
    for c in 0..n {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

/// 0x1A2C70 with its residue frame.
fn fixu(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

fn rnd(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(OT, ra) >> 23
}

fn uv(fx: &mut Fx, a: u32, x: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (x & 0x3FFF) as u16);
}

fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    let d = sp + DESC;
    uv(fx, d + 0x18, u0);
    uv(fx, d + 0x1A, v0);
    uv(fx, d + 0x28, u1);
    uv(fx, d + 0x2A, v1);
}

/// The 32 x 32 atlas cell of byte `b` (4 across) into the sprite's UVs.
fn cell_uvs(fx: &mut Fx, sp: u32, b: u32) {
    let (c, r) = (b & 3, b >> 2);
    uvs(fx, sp, c << 9, r << 9, (32 * c + 32) << 4, (32 * r + 32) << 4);
}

fn sprite_head(fx: &mut Fx, sp: u32) {
    head(fx, sp + DESC, 0x26, 5, 0x4_2421, 0x800);
}

/// The camera's view-projection (x identity).
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + VP);
    fx.identity(sp + MS);
    mul_mem(fx, sp + VP, sp + MS);
}

/// The corners (a, a, 0, 1) / (b, b, 0, 1) turned by the camera.
fn corners(fx: &mut Fx, sp: u32, a: f32, b: f32) {
    vec4(fx, sp + C0, [a, a, 0.0, 1.0]);
    vec4(fx, sp + C1, [b, b, 0.0, 1.0]);
    fx.vu_apply_to(sp + C0, sp + TURN, sp + C0);
    fx.vu_apply_to(sp + C1, sp + TURN, sp + C1);
}

/// sp + Q0 / Q1 = the corners + sp + T.
fn quad(fx: &mut Fx, sp: u32) {
    fx.vu_add4(sp + Q0, sp + C0, sp + T);
    fx.vu_add4(sp + Q1, sp + C1, sp + T);
}

/// The quad projected and drawn keyed by its mean w.
fn draw(fx: &mut Fx, sp: u32, ra: u32) {
    if crate::fxlib2::project(fx, sp + Q0, PROJ_OUT, sp + VP, sp + DESC, sp + W) {
        let w = fx.mem.f(sp + W);
        let key = fixu(fx, w, ra);
        fx.submit(sp + DESC, OT, key as i32);
    }
}

/// 0.1 + 0.9 x.
fn tenth(x: f32) -> f32 {
    ee::add(f32::from_bits(0x3DCC_CCCD), ee::mul(f32::from_bits(0x3F66_6666), x))
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0xB4E0);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    for k in 0..2u32 {
        let r = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x10 + 4 * k, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x20);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x1D, 1);
    fx.mem.set32(D + 0x30, r);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + TURN);
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x30));
    fx.node_position(h, slot, D + 0x40);
    sparks(fx, sp);
    embers(fx, sp);
    trails(fx, sp);
    flash(fx, sp);
    puffs(fx, sp);
    cloud(fx, sp);
    head_run(fx, sp);
    drops(fx, sp);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x82 {
        ret = 1;
    } else if n == 0x8C {
        ret = 2;
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0xBD {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xC0, ret as u32);
    ret
}

/// The sparks [F 0x1E00278..0x1E00660].
fn sparks(fx: &mut Fx, sp: u32) {
    let g = D + SPARKS;
    if fx.mem.i32(D) == 0x8C {
        fx.mem.write_u8(g, 1);
        fx.mem.write_u32(g + 4, 0);
        for k in 0..0x100u32 {
            let b = rnd(fx, 0x01E0_02AC);
            fx.mem.set8(g + 8 + k, b % 8);
            fx.random_table_vec(D + 0x160 + 0x10 * k, f32::from_bits(0x3F99_999A));
        }
    }
    if fx.mem.read_u8(g) != 1 {
        return;
    }
    let d = sp + DESC;
    sprite_head(fx, sp);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    corners(fx, sp, -6.0, 6.0);
    let age = fx.mem.i32(g + 4);
    let f20 = tenth(fx.tan_falloff_p(age, 0x20, 2));
    let age = ee::from_i32(fx.mem.i32(g + 4));
    let r = Fx::ramp_above(age, 32.0, 16.0);
    let f22 = ee::mul(f32::from_bits(0x3F4C_CCCD), f20);
    let f23 = ee::mul(32.0, r);
    let f24 = ee::mul(96.0, r);
    for k in 0..0x100u32 {
        let f21;
        if k % 2 == 0 {
            alpha(fx, d, 2);
            f21 = f22;
            let al = fixu(fx, f23, 0x01E0_049C);
            rgba(fx, d + 0x10, [0x80, 0x80, 0x80], al as u8);
        } else {
            alpha(fx, d, 1);
            f21 = f20;
            let al = fixu(fx, f24, 0x01E0_04E0);
            rgba(fx, d + 0x10, [0xFF, 0x50, 0x19], al as u8);
        }
        let b = u32::from(fx.mem.read_u8(g + 8 + k));
        cell_uvs(fx, sp, b);
        fx.vu_scale4(sp + T, D + 0x160 + 0x10 * k, f21);
        fx.vu_add4(sp + T, sp + T, D + 0x20);
        quad(fx, sp);
        draw(fx, sp, 0x01E0_0614);
    }
    let age = fx.mem.i32(g + 4) + 1;
    fx.mem.set32(g + 4, age);
    if age >= 0x21 {
        fx.mem.write_u8(g, 0);
    }
}

/// The embers [F 0x1E00664..0x1E00AF0].
fn embers(fx: &mut Fx, sp: u32) {
    let g = D + EMBERS;
    if fx.mem.i32(D) == 0x8C {
        fx.mem.write_u8(g, 1);
        fx.mem.write_u32(g + 4, 0);
        for k in 0..0x20u32 {
            let b = rnd(fx, 0x01E0_0698);
            fx.mem.set8(g + 8 + k, b % 8);
            fx.random_table_vec(g + 0x30 + 0x10 * k, 3.0);
        }
    }
    if fx.mem.read_u8(g) != 1 {
        return;
    }
    let d = sp + DESC;
    sprite_head(fx, sp);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    alpha(fx, d, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    corners(fx, sp, -3.0, 3.0);
    let age = ee::from_i32(fx.mem.i32(g + 4));
    let f21 = Fx::ramp_above(age, 32.0, 16.0);
    let al = fixu(fx, ee::mul(255.0, f21), 0x01E0_07F4);
    rgba(fx, d + 0x10, [0xFF, 0x40, 0x18], al as u8);
    let age = fx.mem.i32(g + 4);
    let f20 = tenth(fx.tan_falloff_p(age, 0x20, 1));
    let age = ee::from_i32(fx.mem.i32(g + 4));
    let p = ee::mul(f32::from_bits(0xBC32_864D), Fx::powf2(age));
    let f22 = ee::mul(8.0, ee::mul(0.5, p));
    let age = fx.mem.i32(g + 4);
    let r = D + TRAILS + 0x650 * (age % 16) as u32;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 0);
    fx.mem.setf(r + 0x48, f21);
    for i in 0..0x20u32 {
        for j in 0..2u32 {
            let b = rnd(fx, 0x01E0_08F0);
            fx.mem.set8(r + 8 + 2 * i + j, b % 8);
            fx.random_table_vec(r + 0x250 + 0x20 * i + 0x10 * j, f32::from_bits(0x3E4C_CCCD));
        }
    }
    for k in 0..0x20u32 {
        let b = u32::from(fx.mem.read_u8(g + 8 + k));
        cell_uvs(fx, sp, b);
        fx.vu_scale4(sp + T, g + 0x30 + 0x10 * k, f20);
        let y = fx.mem.f(sp + T + 4);
        fx.mem.setf(sp + T + 4, ee::add(y, f22));
        fx.vu_add4(sp + T, sp + T, D + 0x20);
        quad(fx, sp);
        words(fx, r + 0x50 + 0x10 * k, sp + T, 4);
        draw(fx, sp, 0x01E0_0A9C);
    }
    let age = fx.mem.i32(g + 4) + 1;
    fx.mem.set32(g + 4, age);
    if age >= 0x21 {
        fx.mem.write_u8(g, 0);
    }
}

/// The trail records [F 0x1E00AF4..0x1E00EBC].
fn trails(fx: &mut Fx, sp: u32) {
    let d = sp + DESC;
    sprite_head(fx, sp);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    alpha(fx, d, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    for k in 0..0x10u32 {
        let g = D + TRAILS + 0x650 * k;
        if fx.mem.read_u8(g) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 4);
        let f1 = ee::div(ee::mul(0.5, ee::from_i32(0x10 - age)), 16.0);
        let s = ee::mul(3.0, ee::add(0.5, f1));
        corners(fx, sp, ee::neg(s), s);
        let age = ee::from_i32(fx.mem.i32(g + 4));
        let f0 = ee::mul(255.0, Fx::ramp_above(age, 16.0, 8.0));
        let f1 = fx.mem.f(g + 0x48);
        let al = fixu(fx, ee::mul(f0, f1), 0x01E0_0C80);
        rgba(fx, d + 0x10, [0xFF, 0x40, 0x18], al as u8);
        let age = fx.mem.i32(g + 4);
        let f20 = tenth(fx.tan_falloff_p(age, 0x10, 1));
        for i in 0..0x20u32 {
            for j in 0..2u32 {
                let b = u32::from(fx.mem.read_u8(g + 8 + 2 * i + j));
                cell_uvs(fx, sp, b);
                fx.vu_scale4(sp + T, g + 0x250 + 0x20 * i + 0x10 * j, f20);
                fx.vu_add4(sp + T, sp + T, g + 0x50 + 0x10 * i);
                quad(fx, sp);
                draw(fx, sp, 0x01E0_0E08);
            }
        }
        let age = fx.mem.i32(g + 4) + 1;
        fx.mem.set32(g + 4, age);
        if age >= 0x11 {
            fx.mem.write_u8(g, 0);
        }
    }
}

/// The frame flash [F 0x1E00EC0..0x1E0110C] (b237's).
fn flash(fx: &mut Fx, sp: u32) {
    let g = D + FLASH;
    if fx.mem.i32(D) == 0x8C {
        fx.mem.write_u8(g, 1);
        fx.mem.write_u32(g + 4, 0);
    }
    if fx.mem.read_u8(g) != 1 {
        return;
    }
    let d = sp + DESC;
    sprite_head(fx, sp);
    alpha(fx, d, 0);
    fx.desc_clamp(d, 5);
    let a = clamp_regs(fx, d, 5);
    let layer = fx.layer_begin();
    let regs = crate::lens::frame_copy(fx, a, 1, sp + 0x460, sp + 0x468);
    fx.texflush(regs);
    let age = fx.mem.i32(g + 4);
    rgba(fx, d + 0x10, [0xFF, 0x80, 0x80], (0x20 - age) as u8);
    far_xy(fx, d + 0x20, 0x7000, 0x7000);
    far_xy(fx, d + 0x30, 0x9000, 0x9000);
    for s in 0..8i32 {
        let v = ((s + 1) * 34) >> 3;
        let lo = ((v << 4) & 0x3FFF) as u32;
        let hi = (((0x200 - v) << 4) & 0x3FFF) as u32;
        uvs(fx, sp, lo, lo, hi, hi);
        fx.submit(d, 0, 0);
    }
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(g + 4) + 1;
    fx.mem.set32(g + 4, age);
    if age >= 0x21 {
        fx.mem.write_u8(g, 0);
    }
}

/// The puff records [F 0x1E0110C..0x1E015E4].
fn puffs(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0x1F..0x6A).contains(&n) {
        let g = D + PUFFS + 0x170 * (n % 32) as u32;
        fx.mem.write_u8(g, 1);
        fx.mem.write_u32(g + 4, 0);
        words(fx, g + 0x60, D + 0x40, 4);
        for k in 0..0x10u32 {
            let b = rnd(fx, 0x01E0_119C);
            fx.mem.set8(g + 8 + k, b % 4);
            let r = rnd(fx, 0x01E0_11BC);
            fx.mem.set32(g + 0x18 + 4 * k, ((r % 256) << 16) >> 8);
            fx.random_table_vec(g + 0x70 + 0x10 * k, f32::from_bits(0x3E99_999A));
        }
    }
    let d = sp + DESC;
    sprite_head(fx, sp);
    alpha(fx, d, 1);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    for k in 0..0x20u32 {
        let g = D + PUFFS + 0x170 * k;
        if fx.mem.read_u8(g) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 4);
        let f1 = ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), ee::from_i32(0x20 - age)), 32.0);
        let s = ee::mul(1.5, ee::add(f32::from_bits(0x3E4C_CCCD), f1));
        corners(fx, sp, ee::neg(s), s);
        let age = fx.mem.i32(g + 4);
        let c = fx.tsin((age << 15) >> 5);
        let al = fixu(fx, ee::mul(255.0, c), 0x01E0_1378);
        rgba(fx, d + 0x10, [0xFF, 0x28, 0x0A], al as u8);
        let age = fx.mem.i32(g + 4);
        let f20 = fx.tan_falloff_p(age, 0x20, 1);
        let f21 = ee::mul(2.0, f20);
        for i in 0..0x10u32 {
            fx.vu_scale4(sp + T, g + 0x70 + 0x10 * i, f20);
            let age = fx.mem.i32(g + 4);
            let up = ee::div(ee::mul(2.0, ee::from_i32(age)), 32.0);
            let y = fx.mem.f(sp + T + 4);
            fx.mem.setf(sp + T + 4, ee::add(y, up));
            let ph = fx.mem.i32(g + 0x18 + 4 * i).wrapping_add((age << 14) >> 5);
            let sg = 2 * (i as i32 % 2) - 1;
            let a = ph.wrapping_mul(sg) & 0xFFFF;
            fx.mem.set32(sp + 0x300, a);
            let s = fx.tsin(a);
            let x = fx.mem.f(sp + T);
            fx.mem.setf(sp + T, ee::add(x, ee::mul(f21, s)));
            let c = fx.tcos(a);
            let z = fx.mem.f(sp + T + 8);
            fx.mem.setf(sp + T + 8, ee::add(z, ee::mul(f21, c)));
            fx.vu_add4(sp + T, sp + T, g + 0x60);
            quad(fx, sp);
            let b = u32::from(fx.mem.read_u8(g + 8 + i));
            cell_uvs(fx, sp, b);
            draw(fx, sp, 0x01E0_1574);
        }
        let age = fx.mem.i32(g + 4) + 1;
        fx.mem.set32(g + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(g, 0);
        }
    }
}

/// The cloud [F 0x1E015E4..0x1E01A70].
fn cloud(fx: &mut Fx, sp: u32) {
    let g = D + CLOUD;
    if fx.mem.i32(D) == 0x5D {
        fx.mem.write_u8(g, 1);
        fx.mem.write_u32(g + 4, 0);
        words(fx, g + 0x50, D + 0x40, 4);
        for k in 0..0x40u32 {
            let b = rnd(fx, 0x01E0_1670);
            fx.mem.set8(g + 8 + k, b % 8);
            fx.random_table_vec(g + 0x60 + 0x10 * k, 0.5);
        }
    }
    if fx.mem.read_u8(g) != 1 {
        return;
    }
    let d = sp + DESC;
    sprite_head(fx, sp);
    fx.desc_clamp(d, 5);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    alpha(fx, d, 0);
    camera(fx, sp);
    let age = fx.mem.i32(g + 4);
    let c = fx.tcos((age << 14) >> 5);
    let al = fixu(fx, ee::mul(64.0, c), 0x01E0_17BC);
    rgba(fx, d + 0x10, [0x80, 0x70, 0x64], al as u8);
    let age = ee::from_i32(fx.mem.i32(g + 4));
    let s = ee::mul(8.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 32.0)));
    corners(fx, sp, ee::neg(s), s);
    let age = fx.mem.i32(g + 4);
    let f20 = fx.tan_falloff_p(age, 0x20, 1);
    for k in 0..0x40u32 {
        let b = u32::from(fx.mem.read_u8(g + 8 + k));
        cell_uvs(fx, sp, b);
        fx.vu_scale4(sp + T, g + 0x60 + 0x10 * k, f20);
        fx.vu_add4(sp + T, sp + T, g + 0x50);
        quad(fx, sp);
        draw(fx, sp, 0x01E0_19C4);
    }
    let age = fx.mem.i32(g + 4) + 1;
    fx.mem.set32(g + 4, age);
    if age >= 0x21 {
        fx.mem.write_u8(g, 0);
    }
}

/// The head [F 0x1E01A74..0x1E0202C]: node to target in 47 updates,
/// dropping a record each update.
fn head_run(fx: &mut Fx, sp: u32) {
    let g = D + HEAD;
    if fx.mem.i32(D) == 0x5D {
        fx.mem.write_u8(g, 1);
        fx.mem.write_u32(g + 4, 0);
        words(fx, g + 0x50, D + 0x40, 4);
        words(fx, sp + T, D + 0x20, 4);
        fx.vu_sub4(sp + T, sp + T, g + 0x50);
        let x2 = Fx::powf2(fx.mem.f(sp + T));
        let y2 = Fx::powf2(fx.mem.f(sp + T + 4));
        let f20 = ee::add(x2, y2);
        let z2 = Fx::powf2(fx.mem.f(sp + T + 8));
        let l = fx.sqrtf(ee::add(z2, f20));
        fx.mem.setf(g + 0xC, l);
        fx.point_along(g + 0x10, sp + T);
        let b = rnd(fx, 0x01E0_1B74) % 256;
        fx.rotate_axis(g + 0x10, (b << 16) >> 8, 2);
    }
    let d = sp + DESC;
    sprite_head(fx, sp);
    alpha(fx, d, 1);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    uvs(fx, sp, 0x400, 0, 0x600, 0x200);
    corners(fx, sp, -5.0, 5.0);
    if fx.mem.read_u8(g) == 0 {
        return;
    }
    rgba(fx, d + 0x10, [0xFF, 0x28, 0x0A], 0xFF);
    let f20 = ee::div(ee::from_i32(fx.mem.i32(g + 4)), 47.0);
    let cnt = fx.mem.i32(g + 8);
    fx.mem.set32(g + 8, cnt + 1);
    if cnt + 1 >= 8 {
        fx.mem.write_u32(g + 8, 0);
    }
    let z = ee::mul(f20, fx.mem.f(g + 0xC));
    vec4(fx, sp + T, [0.0, 0.0, z, 1.0]);
    fx.vu_apply_to(sp + T, g + 0x10, sp + T);
    fx.vu_add4(sp + T, sp + T, g + 0x50);
    quad(fx, sp);
    draw(fx, sp, 0x01E0_1EB0);
    let r = D + DROPS + 0x130 * cnt as u32;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 0);
    words(fx, r + 0x20, sp + T, 4);
    for k in 0..0x10u32 {
        let b = rnd(fx, 0x01E0_1F28);
        fx.mem.set8(r + 8 + k, b % 8);
        fx.random_table_vec(r + 0x30 + 0x10 * k, f32::from_bits(0x3E99_999A));
    }
    let age = fx.mem.i32(g + 4) + 1;
    fx.mem.set32(g + 4, age);
    if age >= 0x30 {
        fx.mem.write_u8(g, 0);
    }
}

/// The dropped records [F 0x1E02030..0x1E02410].
fn drops(fx: &mut Fx, sp: u32) {
    let d = sp + DESC;
    sprite_head(fx, sp);
    alpha(fx, d, 1);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    for k in 0..8u32 {
        let g = D + DROPS + 0x130 * k;
        if fx.mem.read_u8(g) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 4);
        let f0 = ee::div(ee::mul(0.5, ee::from_i32(8 - age)), 8.0);
        let s = ee::mul(3.5, ee::add(0.5, f0));
        corners(fx, sp, ee::neg(s), s);
        let age = fx.mem.i32(g + 4);
        rgba(fx, d + 0x10, [0xFF, 0x3C, 0x1E], (((8 - age) << 7) >> 3) as u8);
        let age = fx.mem.i32(g + 4);
        let t = fx.tan_falloff_p(age, 8, 1);
        let f20 = ee::add(f32::from_bits(0x3ECC_CCCD), ee::mul(f32::from_bits(0x3F19_999A), t));
        for i in 0..0x10u32 {
            let b = u32::from(fx.mem.read_u8(g + 8 + i));
            cell_uvs(fx, sp, b);
            fx.vu_scale4(sp + T, g + 0x30 + 0x10 * i, f20);
            fx.vu_add4(sp + T, sp + T, g + 0x20);
            quad(fx, sp);
            draw(fx, sp, 0x01E0_2344);
        }
        let age = fx.mem.i32(g + 4) + 1;
        fx.mem.set32(g + 4, age);
        if age >= 9 {
            fx.mem.write_u8(g, 0);
        }
    }
}
