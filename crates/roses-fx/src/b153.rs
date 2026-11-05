//! BMODULE b153 (card 115), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0x6A40
//! bytes) [F]: a single unscheduled
//! function in a 0x750 frame (s1 = D, s0 the ordering table), b729's lens
//! / burst family. Every update the camera turn with roll (sp + 0x510),
//! nodes 0x22 / 0x2C (D+0x40 / 0x44) into D+0x50 / 0x60, the turn D+0x70
//! (below 0x55 pointing from node 0 to node 1, else the identity, turned
//! 0x8000 about y when the side is negative), D+0xB0 = node 0 moved to the
//! target by clamp(n - 0x55, 0, 10) / 10 and the target's screen point
//! D+0x30. On 0x5F a lens (D+0x11B0, 48 updates: the frame copied into
//! eight rings of the fan 0x28ABE0 about the target's screen point, alpha
//! 64 (48 - age) / 48) and one 0x10E0 burst record (D+0xD0, 48 updates;
//! b729's four parts with other sizes, alphas and colours). On 0x31 a
//! textured ST quad (D+0x11B8, 0x4E updates) placed by D+0x70 at D+0xB0.
//! From 0x32 to 0x54 a sprite record a update in two pools (D+0x11C0, 28
//! x 0x170: 20 sprites, 28 updates; D+0x3A00, 20 x 0x60: 4 sprites, 20
//! updates), from 0x56 to 0x5E in two more (D+0x4180, 16 x 0x240: 32
//! sprites, 16 updates; D+0x6580, 4 x 0x130: 16 far-Z sprites pushed
//! along z by the side, 4 updates), all at D+0xB0. The stage light 0x80
//! - 80 clamp(0xAF - n) clamp(n) / 1024. Returns 1 / 6 / 2 / 8 on 0x55 /
//! 0x60..0x8E / 0x8F / 0xB0. No 0x1F8100 / 0x1F8280 / 0x1F82B0.
use crate::blib::exe::mul_mem;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_z, KEEP, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70 / 0x205210's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x750;
/// The camera's view-projection, its temporary and the camera turn.
const CAM: u32 = 0x490;
const TMP: u32 = 0x4D0;
const TURN: u32 = 0x510;
/// The sprite's corners, its projected copies and its point.
const CORN: u32 = 0x550;
const PIN: u32 = 0x590;
const POS: u32 = 0x5D0;
/// The sprite descriptor (prim 0x26 | 0x800, 5 registers).
const DESC: u32 = 0x5E0;
/// The lens' triangle descriptor.
const TRI: u32 = 0x638;
/// The quad's descriptor (prim 0x34, 12 registers).
const QUAD: u32 = 0x6B0;
/// The mean w and the screen point's XYZ.
const W: u32 = 0x74C;
const SCR: u32 = 0x740;
const FAN: u32 = 0x0028_ABE0;
const FAR: u64 = 0xFF_FFF8;
/// The burst record (one, 0x10E0 bytes).
const REC: u32 = D + 0xD0;
const LIFE: i32 = 0x30;

/// Return addresses: the lens' twelve 0x1A2B80, the record's two rands,
/// its glow / spark / key 0x1A2C70, the quad's alpha / key, then per pool
/// (A, B, C, D) its rand, alpha and key (D: no key).
const R_LENS: [u32; 12] = [
    0x01E0_05F8, 0x01E0_0604, 0x01E0_0648, 0x01E0_0654, 0x01E0_0698, 0x01E0_06A4, 0x01E0_0758, 0x01E0_0764, 0x01E0_07D0, 0x01E0_07DC, 0x01E0_0848,
    0x01E0_0854,
];
const R_RAND1: u32 = 0x01E0_0A48;
const R_RAND3: u32 = 0x01E0_0B28;
const R_GLOW: u32 = 0x01E0_14AC;
const R_SPARK: u32 = 0x01E0_1814;
const R_KEY: u32 = 0x01E0_18D4;
const R_QUAD: u32 = 0x01E0_1A60;
const R_QUAD_KEY: u32 = 0x01E0_1B60;
const R_A: [u32; 3] = [0x01E0_1C30, 0x01E0_1DCC, 0x01E0_202C];
const R_B: [u32; 3] = [0x01E0_2180, 0x01E0_2318, 0x01E0_2594];
const R_C: [u32; 3] = [0x01E0_26E8, 0x01E0_282C, 0x01E0_2A48];
const R_D: [u32; 2] = [0x01E0_2BA0, 0x01E0_2CDC];

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xB0, 0);
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

/// 0x1A2C70 (unsigned) / 0x1A2B80 (signed) with their residue frames.
fn fixu(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}
fn fixs(fx: &mut Fx, f: f32, ra: u32) -> i32 {
    fx.fix_residue(f, ra);
    ee::fixsfsi(f)
}
fn fix_at(fx: &mut Fx, a: u32, ra: u32) -> i32 {
    let f = fx.mem.f(a);
    fixs(fx, f, ra)
}

fn rem2(x: i32, m: i32) -> i32 {
    let r = x & (m - 1);
    if x < 0 && r != 0 {
        r - m
    } else {
        r
    }
}

fn uv(fx: &mut Fx, a: u32, x: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (x & 0x3FFF) as u16);
}

/// The sprite's UVs (desc + 0x18 / 0x1A, 0x28 / 0x2A).
fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, sp + DESC + 0x18, u0);
    uv(fx, sp + DESC + 0x1A, v0);
    uv(fx, sp + DESC + 0x28, u1);
    uv(fx, sp + DESC + 0x2A, v1);
}

/// The 32 x 32 atlas cell of the unsigned byte / halfword `b` (4 across).
fn cell_uvs(fx: &mut Fx, sp: u32, b: u32) {
    let (c, r) = (b & 3, b >> 2);
    uvs(fx, sp, c << 9, r << 9, (32 * c + 32) << 4, (32 * r + 32) << 4);
}

fn rgba(fx: &mut Fx, at: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(at, c[0]);
    fx.mem.write_u8(at + 1, c[1]);
    fx.mem.write_u8(at + 2, c[2]);
    fx.mem.write_u8(at + 3, a);
    fx.mem.write_u32(at + 4, 0x3F80_0000);
}

/// The sprite descriptor head (prim 0x26 | 0x800, 5 registers).
fn head(fx: &mut Fx, sp: u32) {
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
}

fn texture(fx: &mut Fx, d: u32, tex: u32) {
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
}

fn alpha(fx: &mut Fx, d: u32, preset: i32) {
    let a = fx.alpha_preset(preset);
    fx.desc_alpha(d, a);
}

/// The camera's view-projection (x identity).
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + CAM);
    fx.identity(sp + TMP);
    mul_mem(fx, sp + CAM, sp + TMP);
}

/// The corners (-s, -s, 0, 1) / (s, s, 0, 1), turned by the camera.
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    let n = ee::neg(s);
    vec4(fx, sp + CORN, [n, n, 0.0, 1.0]);
    vec4(fx, sp + CORN + 0x10, [s, s, 0.0, 1.0]);
    fx.vu_apply_to(sp + CORN, sp + TURN, sp + CORN);
    fx.vu_apply_to(sp + CORN + 0x10, sp + TURN, sp + CORN + 0x10);
}

/// The sprite at POS + the corners projected (0x204F70); true when drawn.
fn sprite_at(fx: &mut Fx, sp: u32) -> bool {
    fx.vu_add4(sp + PIN, sp + CORN, sp + POS);
    fx.vu_add4(sp + PIN + 0x10, sp + CORN + 0x10, sp + POS);
    crate::fxlib2::project(fx, sp + PIN, PROJ_OUT, sp + CAM, sp + DESC, sp + W)
}

/// The cell's UVs then the sprite projected.
fn sprite_cell(fx: &mut Fx, sp: u32, b: u32) -> bool {
    fx.vu_add4(sp + PIN, sp + CORN, sp + POS);
    fx.vu_add4(sp + PIN + 0x10, sp + CORN + 0x10, sp + POS);
    cell_uvs(fx, sp, b);
    crate::fxlib2::project(fx, sp + PIN, PROJ_OUT, sp + CAM, sp + DESC, sp + W)
}

/// The sprite submitted at the far Z.
fn submit_far(fx: &mut Fx, sp: u32) {
    far_z(fx, sp + DESC + 0x20, &[sp + DESC + 0x30]);
    fx.submit(sp + DESC, OT, 0);
}

/// The sprite submitted at its mean w.
fn submit_w(fx: &mut Fx, sp: u32, ra: u32) {
    let w = fx.mem.f(sp + W);
    let key = fixu(fx, w, ra);
    fx.submit(sp + DESC, OT, key as i32);
}

/// An XYZ2 at `p`: x / y halfwords of pixel (x, y), the far Z, a zero top
/// byte.
fn xyz_far(fx: &mut Fx, p: u32, x: i32, y: i32) {
    fx.mem.write_u16(p, (x.wrapping_add(0x700) << 4) as u16);
    fx.mem.write_u16(p + 2, (y.wrapping_add(0x700) << 4) as u16);
    let w = fx.mem.read_u64(p);
    fx.mem.write_u64(p, (w & KEEP) | (FAR << 32));
    fx.mem.write_u8(p + 7, 0);
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x6A40);
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
    fx.unit_position(t, D + 0x20);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x22, 1);
    fx.mem.set32(D + 0x40, r);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x2C, 0);
    fx.mem.set32(D + 0x44, r);
    fx.mem.write_u32(D + 0xC0, 0);
    fx.mem.write_u8(REC, 0);
    fx.mem.write_u8(D + 0x11B0, 0);
    fx.mem.write_u8(D + 0x11B8, 0);
    for k in 0..0x1Cu32 {
        fx.mem.write_u8(D + 0x11C0 + 0x170 * k, 0);
    }
    for k in 0..0x14u32 {
        fx.mem.write_u8(D + 0x3A00 + 0x60 * k, 0);
    }
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x4180 + 0x240 * k, 0);
    }
    for k in 0..4u32 {
        fx.mem.write_u8(D + 0x6580 + 0x130 * k, 0);
    }
    0
}

/// The screen point of `p` into `o` (x / y clamped to 0..512, Z + 8).
fn screen_point(fx: &mut Fx, sp: u32, p: u32, o: u32) {
    camera(fx, sp);
    let m = fx.read_m4(sp + CAM);
    fx.project_at(p, &m, sp + SCR, sp + W);
    let x = (i32::from(fx.mem.read_u16(sp + SCR)) >> 4) - 0x700;
    fx.mem.setf(o, ee::from_i32(x));
    let y = (i32::from(fx.mem.read_u16(sp + SCR + 2)) >> 4) - 0x700;
    fx.mem.setf(o + 4, ee::from_i32(y));
    let z = (fx.mem.read_u64(sp + SCR) << 8) >> 40;
    fx.mem.setf(o + 8, ee::from_i32(z as i32));
    fx.mem.write_u32(o + 0xC, 0x3F80_0000);
    for k in [0u32, 4] {
        if crate::exe::ee_lt(fx.mem.f(o + k), 0.0) {
            fx.mem.write_u32(o + k, 0);
        }
    }
    for k in [0u32, 4] {
        if !crate::exe::ee_le(fx.mem.f(o + k), 512.0) {
            fx.mem.write_u32(o + k, 0x4400_0000);
        }
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + TURN);
    for i in 0..2u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x40 + 4 * i));
        fx.node_position(h, slot, D + 0x50 + 0x10 * i);
    }
    if fx.mem.i32(D) < 0x55 {
        fx.vu_sub4(sp + POS, D + 0x60, D + 0x50);
        fx.point_along(D + 0x70, sp + POS);
    } else {
        fx.identity(D + 0x70);
        if fx.mem.i32(D + 0xC) < 0 {
            fx.rotate_axis(D + 0x70, 0x8000, 1);
        }
    }
    let n = fx.mem.i32(D);
    let f20 = ee::div(ee::from_i32(Fx::clamp_i(n - 0x55, 0, 10)), 10.0);
    fx.vu_sub4(sp + POS, D + 0x20, D + 0x50);
    fx.vu_scale4(sp + POS, sp + POS, f20);
    fx.vu_add4(D + 0xB0, sp + POS, D + 0x50);
    screen_point(fx, sp, D + 0x20, D + 0x30);
    if fx.mem.i32(D) == 0x5F {
        fx.mem.write_u8(D + 0x11B0, 1);
        fx.mem.write_u32(D + 0x11B4, 0);
    }
    if fx.mem.read_u8(D + 0x11B0) == 1 {
        lens(fx, sp);
    }
    if fx.mem.i32(D) == 0x5F {
        spawn(fx);
    }
    record(fx, sp);
    if fx.mem.i32(D) == 0x31 {
        fx.mem.write_u8(D + 0x11B8, 1);
        fx.mem.write_u32(D + 0x11BC, 0);
    }
    if fx.mem.read_u8(D + 0x11B8) == 1 {
        quad(fx, sp);
    }
    pool_a(fx, sp);
    pool_b(fx, sp);
    pool_c(fx, sp);
    pool_d(fx, sp);
    let n = fx.mem.i32(D);
    let a = Fx::clamp_i(0xAF - n, 0, 0x20);
    let n = fx.mem.i32(D);
    let b = Fx::clamp_i(n, 0, 0x20);
    let m = fx.mult(a, (-(b << 2) - b) << 4);
    fx.set_stage_light(((m >> 10) + 0x80) & 0xFF);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x55 {
        ret = 1;
    } else {
        if (0x60..0x8F).contains(&n) {
            ret = 6;
        }
        if n == 0x8F {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0xB0 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xB0, ret as u32);
    ret
}

/// The lens [F 0x1E0046C..0x1E00924].
fn lens(fx: &mut Fx, sp: u32) {
    let d = sp + TRI;
    fx.mem.write_u16(d + 2, 0x33);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, 0x4_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    alpha(fx, d, 0);
    fx.desc_clamp(d, LENS_CLAMP);
    // a0..a3 as 0x208400 leaves them.
    let a = [u32::from(fx.mem.read_u16(d + 2)), LENS_CLAMP as u32, d + 0x10, d + 2];
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xDC, layer);
    let regs = crate::lens::frame_copy(fx, a, 1, sp + 0x698, sp + 0x6A0);
    fx.texflush(regs);
    let age = fx.mem.i32(D + 0x11B4);
    let al = ((LIFE - age) << 6) / 48;
    let p = d + 0x10;
    rgba(fx, p, [0x80, 0x80, 0x80], al as u8);
    for o in [0x28u32, 0x40] {
        words(fx, d + o, p, 2);
    }
    let c = D + 0x30;
    let r = &R_LENS;
    for i in 0..8 {
        let f20 = ee::add(1.0, ee::div(ee::mul(f32::from_bits(0x3E19_999A), ee::from_i32(8 - i)), 8.0));
        for k in 0..8u32 {
            let s3 = FAN + 0x10 * k;
            let x = fix_at(fx, c, r[0]);
            let y = fix_at(fx, c + 4, r[1]);
            uv(fx, d + 0x18, (x << 4) as u32);
            uv(fx, d + 0x1A, (y << 4) as u32);
            let x = fix_at(fx, s3, r[2]);
            let y = fix_at(fx, s3 + 4, r[3]);
            uv(fx, d + 0x30, (x << 4) as u32);
            uv(fx, d + 0x32, (y << 4) as u32);
            let x = fix_at(fx, s3 + 0x10, r[4]);
            let y = fix_at(fx, s3 + 0x14, r[5]);
            uv(fx, d + 0x48, (x << 4) as u32);
            uv(fx, d + 0x4A, (y << 4) as u32);
            fx.vu_sub4(sp + PIN, s3, c);
            fx.vu_sub4(sp + PIN + 0x10, s3 + 0x10, c);
            fx.vu_scale4(sp + PIN, sp + PIN, f20);
            fx.vu_scale4(sp + PIN + 0x10, sp + PIN + 0x10, f20);
            fx.vu_add4(sp + PIN, sp + PIN, c);
            fx.vu_add4(sp + PIN + 0x10, sp + PIN + 0x10, c);
            let x = fix_at(fx, c, r[6]);
            let y = fix_at(fx, c + 4, r[7]);
            xyz_far(fx, d + 0x20, x, y);
            let x = fix_at(fx, sp + PIN, r[8]);
            let y = fix_at(fx, sp + PIN + 4, r[9]);
            xyz_far(fx, d + 0x38, x, y);
            let x = fix_at(fx, sp + PIN + 0x10, r[10]);
            let y = fix_at(fx, sp + PIN + 0x14, r[11]);
            xyz_far(fx, d + 0x50, x, y);
            fx.submit(d, 0, 0);
        }
    }
    let layer = fx.mem.read_u32(sp + 0xDC);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0x11B4) + 1;
    fx.mem.set32(D + 0x11B4, age);
    if age >= LIFE + 1 {
        fx.mem.write_u8(D + 0x11B0, 0);
    }
}

/// The burst record's spawn [F 0x1E00934..0x1E00BB4].
fn spawn(fx: &mut Fx) {
    for s2 in 0..1u32 {
        fx.mem.write_u32(D + 0xC0, s2);
        let r = REC + 0x10E0 * fx.mem.read_u32(D + 0xC0);
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 1);
        vec4(fx, r + 0x90, [0.0, 0.0, 0.0, 1.0]);
        fx.vu_add4(r + 0x90, r + 0x90, D + 0x20);
        for j in 0..0x40u32 {
            let b = fx.rand_from(OT, R_RAND1) >> 23;
            fx.mem.set16(r + 8 + 2 * j, rem2(b, 8));
            fx.random_table_vec(r + 0xA0 + 0x10 * j, f32::from_bits(0x3F66_6666));
        }
        for j in 0..0x80u32 {
            fx.random_table_vec(r + 0x4A0 + 0x10 * j, 1.25);
        }
        for j in 0..0x40u32 {
            let b = fx.rand_from(OT, R_RAND3) >> 23;
            let (_, h) = fx.div(b, 3);
            fx.mem.set8(r + 0xCA0 + j, h + 5);
            fx.random_table_vec(r + 0xCE0 + 0x10 * j, f32::from_bits(0x3F66_6666));
        }
    }
}

/// k (0.8 + 0.2 age / 48).
fn grow(k: f32, age: i32) -> f32 {
    let a = ee::from_i32(age);
    ee::mul(k, ee::add(f32::from_bits(0x3F4C_CCCD), ee::div(ee::mul(f32::from_bits(0x3E4C_CCCD), a), 48.0)))
}

/// The burst record's draws [F 0x1E00BB4..0x1E01974].
fn record(fx: &mut Fx, sp: u32) {
    for i in 0..1u32 {
        let r = REC + 0x10E0 * i;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let pos = r + 0x90;
        // 64 sprites thrown along the third vectors (texture 3).
        head(fx, sp);
        texture(fx, sp + DESC, 0x18);
        fx.desc_clamp(sp + DESC, 5);
        alpha(fx, sp + DESC, 1);
        camera(fx, sp);
        let age = fx.mem.i32(r + 4);
        corners(fx, sp, grow(6.0, age));
        let age = fx.mem.i32(r + 4);
        rgba(fx, sp + DESC + 0x10, [0xFF, 0x6C, 0x38], (((LIFE - age) * 200) / 48) as u8);
        let age = fx.mem.i32(r + 4);
        let f22 = fx.tan_falloff_p(age, LIFE, 1);
        for j in 0..0x40u32 {
            fx.vu_scale4(sp + POS, r + 0xCE0 + 0x10 * j, f22);
            fx.vu_add4(sp + POS, sp + POS, pos);
            let b = u32::from(fx.mem.read_u8(r + 0xCA0 + j));
            if sprite_cell(fx, sp, b) {
                submit_far(fx, sp);
            }
        }
        // 64 sprites along the first vectors, two blends (texture 0).
        head(fx, sp);
        texture(fx, sp + DESC, 0x10);
        fx.desc_clamp(sp + DESC, 5);
        camera(fx, sp);
        let age = fx.mem.i32(r + 4);
        corners(fx, sp, grow(5.0, age));
        let age = fx.mem.i32(r + 4);
        let a_odd = ((LIFE - age) * 255) / 48;
        let a_even = ((LIFE - age) * 128) / 48;
        for j in 0..0x40u32 {
            let age = fx.mem.i32(r + 4);
            let mut f22 = fx.tan_falloff_p(age, LIFE, 1);
            if j % 2 == 0 {
                f22 = ee::mul(f22, f32::from_bits(0x3F4C_CCCD));
                alpha(fx, sp + DESC, 2);
                rgba(fx, sp + DESC + 0x10, [0x80, 0xFF, 0xFF], a_even as u8);
            } else {
                alpha(fx, sp + DESC, 1);
                rgba(fx, sp + DESC + 0x10, [0xFF, 0x60, 0x1E], a_odd as u8);
            }
            fx.vu_scale4(sp + POS, r + 0xA0 + 0x10 * j, f22);
            fx.vu_add4(sp + POS, sp + POS, pos);
            let b = u32::from(fx.mem.read_u16(r + 8 + 2 * j));
            if sprite_cell(fx, sp, b) {
                submit_far(fx, sp);
            }
        }
        // The glow (texture 1).
        head(fx, sp);
        texture(fx, sp + DESC, 0x14);
        fx.desc_clamp(sp + DESC, 5);
        alpha(fx, sp + DESC, 1);
        fx.camera_vp_to(sp + CAM);
        fx.translate_rows(sp + TMP, sp + TURN, pos);
        let age = fx.mem.i32(r + 4);
        let tf = fx.tan_falloff_p(age, LIFE, 1);
        let g = ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), tf));
        vec4(fx, sp + POS, [g, g, g, 1.0]);
        fx.scale_rows(sp + TMP, sp + POS);
        mul_mem(fx, sp + CAM, sp + TMP);
        let age = fx.mem.i32(r + 4);
        let f1 = ee::div(ee::from_i32(LIFE - age), 48.0);
        let al = fixu(fx, ee::mul(200.0, f1), R_GLOW);
        rgba(fx, sp + DESC + 0x10, [0xFF, 0x5E, 0x3A], al as u8);
        uvs(fx, sp, 0x600, 0, 0x800, 0x200);
        vec4(fx, sp + CORN, [-65.0, -65.0, 0.0, 1.0]);
        vec4(fx, sp + CORN + 0x10, [65.0, 65.0, 0.0, 1.0]);
        if crate::fxlib2::project(fx, sp + CORN, PROJ_OUT, sp + CAM, sp + DESC, sp + W) {
            submit_far(fx, sp);
        }
        // 128 x 6 stretched sparks (texture 1).
        head(fx, sp);
        texture(fx, sp + DESC, 0x14);
        fx.desc_clamp(sp + DESC, 5);
        alpha(fx, sp + DESC, 1);
        uvs(fx, sp, 0x600, 0, 0x800, 0x200);
        camera(fx, sp);
        corners(fx, sp, 0.5);
        let age = fx.mem.i32(r + 4);
        let f23 = ee::div(ee::from_i32(LIFE - age), 48.0);
        let age = fx.mem.i32(r + 4);
        let f22 = fx.tan_falloff_p(age, LIFE, 1);
        for j in 0..0x80u32 {
            let p = r + 0x4A0 + 0x10 * j;
            for s1 in 0..6i32 {
                let k = ((s1 + 6) * 255) / 12;
                let al = fixu(fx, ee::mul(f23, ee::from_i32(k)), R_SPARK);
                rgba(fx, sp + DESC + 0x10, [0xFF, 0x8C, 0x80], al as u8);
                let f = ee::add(f22, ee::mul(f32::from_bits(0x3C23_D70A), ee::from_i32(s1)));
                fx.vu_scale4(sp + POS, p, f);
                fx.vu_add4(sp + POS, sp + POS, pos);
                if sprite_at(fx, sp) {
                    submit_w(fx, sp, R_KEY);
                }
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= LIFE + 1 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The textured ST quad (D+0x11B8, 0x4E updates) [F 0x1E019A0..0x1E01B90].
fn quad(fx: &mut Fx, sp: u32) {
    let d = sp + QUAD;
    fx.mem.write_u16(d + 2, 0x34);
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    texture(fx, d, 0x1C);
    fx.desc_clamp(d, 5);
    alpha(fx, d, 0);
    fx.camera_vp_to(sp + CAM);
    fx.translate_rows(sp + TMP, D + 0x70, D + 0xB0);
    mul_mem(fx, sp + CAM, sp + TMP);
    let age = ee::from_i32(fx.mem.i32(D + 0x11BC));
    let f20 = ee::mul(200.0, Fx::ratio_below(age, 32.0));
    let age = ee::from_i32(fx.mem.i32(D + 0x11BC));
    let al = fixu(fx, ee::mul(f20, Fx::ramp_above(age, 78.0, 46.0)), R_QUAD);
    rgba(fx, d + 0x10, [0x80, 0x80, 0x80], al as u8);
    for o in [0x28u32, 0x40, 0x58] {
        words(fx, d + o, d + 0x10, 2);
    }
    vec4(fx, sp + CORN, [0.0, -1.5, 0.0, 1.0]);
    vec4(fx, sp + CORN + 0x10, [0.0, 1.5, 0.0, 1.0]);
    vec4(fx, sp + CORN + 0x20, [0.0, -1.5, 12.0, 1.0]);
    vec4(fx, sp + CORN + 0x30, [0.0, 1.5, 12.0, 1.0]);
    for (o, s, t) in [(0x18u32, 0.0f32, 0.0f32), (0x30, 0.0, 1.0), (0x48, 1.0, 0.0), (0x60, 1.0, 1.0)] {
        fx.mem.setf(d + o, s);
        fx.mem.setf(d + o + 4, t);
    }
    let m = fx.read_m4(sp + CAM);
    if fx.project_desc(sp + CORN, PROJ_OUT, Some(&m), d, sp + W) {
        let w = fx.mem.f(sp + W);
        let key = fixu(fx, w, R_QUAD_KEY);
        fx.submit(d, OT, key as i32);
    }
    let age = fx.mem.i32(D + 0x11BC) + 1;
    fx.mem.set32(D + 0x11BC, age);
    if age >= 0x4F {
        fx.mem.write_u8(D + 0x11B8, 0);
    }
}

/// A sprite pool's spawn: record `k` at D+0xB0 (point at +`pos`), `n`
/// cells rand % `cells` at +8 and directions of length `len` at +`dirs`.
#[allow(clippy::too_many_arguments)]
fn spawn_pool(fx: &mut Fx, base: u32, stride: u32, k: i32, pos: u32, dirs: u32, n: u32, cells: i32, len: f32, ra: u32) {
    let r = base + stride * k as u32;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 0);
    words(fx, r + pos, D + 0xB0, 4);
    for j in 0..n {
        let b = fx.rand_from(OT, ra) >> 23;
        fx.mem.set8(r + 8 + j, rem2(b, cells));
        fx.random_table_vec(r + dirs + 0x10 * j, len);
    }
}

/// The pool's sprite descriptor (texture `tex`, alpha 1, CLAMP 5) and the
/// camera.
fn pool_head(fx: &mut Fx, sp: u32, tex: u32) {
    head(fx, sp);
    fx.desc_clamp(sp + DESC, 5);
    texture(fx, sp + DESC, tex);
    alpha(fx, sp + DESC, 1);
    camera(fx, sp);
}

/// Pool A: 28 x 0x170 (20 sprites, 28 updates) [F 0x1E01B90..0x1E020E8].
fn pool_a(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0x32..0x55).contains(&n) {
        let (_, k) = fx.div(n, 0x1C);
        spawn_pool(fx, D + 0x11C0, 0x170, k, 0x20, 0x30, 0x14, 4, f32::from_bits(0x3DA3_D70A), R_A[0]);
    }
    pool_head(fx, sp, 0x14);
    let n = fx.mem.i32(D);
    let f20 = ee::div(ee::from_i32(Fx::clamp_i(n - 0x31, 0, 0x20)), 32.0);
    for k in 0..0x1Cu32 {
        let r = D + 0x11C0 + 0x170 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 4);
        let c = fx.tcos((age << 14) / 28);
        let al = fixu(fx, ee::mul(f20, ee::mul(128.0, c)), R_A[1]) as u8;
        let age = fx.mem.i32(r + 4);
        let t = ee::div(ee::mul(0.5, ee::from_i32(0x1C - age)), 28.0);
        corners(fx, sp, ee::mul(0.25, ee::add(0.5, t)));
        let age = fx.mem.i32(r + 4);
        let tf = fx.tan_falloff_p(age, 0x1C, 1);
        let f21 = ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), tf));
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let f22 = ee::div(ee::mul(8.0, age), 28.0);
        rgba(fx, sp + DESC + 0x10, [0xFF, 0x5A, 0x2D], al);
        for j in 0..0x14u32 {
            let b = u32::from(fx.mem.read_u8(r + 8 + j));
            cell_uvs(fx, sp, b);
            fx.vu_scale4(sp + POS, r + 0x30 + 0x10 * j, f21);
            fx.vu_add4(sp + POS, sp + POS, r + 0x20);
            let y = fx.mem.f(sp + POS + 4);
            fx.mem.setf(sp + POS + 4, ee::add(y, f22));
            if sprite_at(fx, sp) {
                submit_w(fx, sp, R_A[2]);
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x1D {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// Pool B: 20 x 0x60 (4 sprites, 20 updates) [F 0x1E020E8..0x1E02650].
fn pool_b(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0x32..0x55).contains(&n) {
        let (_, k) = fx.div(n, 0x14);
        spawn_pool(fx, D + 0x3A00, 0x60, k, 0x10, 0x20, 4, 8, f32::from_bits(0x3D4C_CCCD), R_B[0]);
    }
    pool_head(fx, sp, 0x10);
    let n = fx.mem.i32(D);
    let f20 = ee::div(ee::from_i32(Fx::clamp_i(n - 0x31, 0, 0x20)), 32.0);
    for k in 0..0x14u32 {
        let r = D + 0x3A00 + 0x60 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 4);
        let s = fx.tsin((age << 15) / 20);
        let al = fixu(fx, ee::mul(f20, ee::mul(128.0, s)), R_B[1]) as u8;
        let age = fx.mem.i32(r + 4);
        let t = ee::div(ee::mul(f32::from_bits(0x3F33_3333), ee::from_i32(0x14 - age)), 20.0);
        corners(fx, sp, ee::mul(1.5, ee::add(f32::from_bits(0x3E99_999A), t)));
        let age = fx.mem.i32(r + 4);
        let t = ee::div(ee::mul(f32::from_bits(0x3E4C_CCCD), ee::from_i32(0x14 - age)), 20.0);
        let f21 = ee::add(f32::from_bits(0x3F4C_CCCD), t);
        let f22 = ee::add(-1.0, ee::div(ee::mul(6.0, ee::from_i32(age)), 20.0));
        rgba(fx, sp + DESC + 0x10, [0xFF, 0x3C, 0x1E], al);
        for j in 0..4u32 {
            let b = u32::from(fx.mem.read_u8(r + 8 + j));
            cell_uvs(fx, sp, b);
            fx.vu_scale4(sp + POS, r + 0x20 + 0x10 * j, f21);
            fx.vu_add4(sp + POS, sp + POS, r + 0x10);
            let y = fx.mem.f(sp + POS + 4);
            fx.mem.setf(sp + POS + 4, ee::add(y, f22));
            if sprite_at(fx, sp) {
                submit_w(fx, sp, R_B[2]);
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x15 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// Pool C: 16 x 0x240 (32 sprites, 16 updates) [F 0x1E02650..0x1E02B00].
fn pool_c(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0x56..0x5F).contains(&n) {
        spawn_pool(fx, D + 0x4180, 0x240, rem2(n, 0x10), 0x30, 0x40, 0x20, 4, f32::from_bits(0x3ECC_CCCD), R_C[0]);
    }
    pool_head(fx, sp, 0x14);
    for k in 0..0x10u32 {
        let r = D + 0x4180 + 0x240 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 4);
        let c = fx.tcos((age << 14) >> 4);
        let al = fixu(fx, ee::mul(128.0, c), R_C[1]) as u8;
        let age = fx.mem.i32(r + 4);
        let t = ee::div(ee::mul(0.5, ee::from_i32(0x10 - age)), 16.0);
        corners(fx, sp, ee::mul(0.5, ee::add(0.5, t)));
        let age = fx.mem.i32(r + 4);
        let tf = fx.tan_falloff_p(age, 0x10, 1);
        let f20 = ee::add(f32::from_bits(0x3F33_3333), ee::mul(f32::from_bits(0x3E99_999A), tf));
        rgba(fx, sp + DESC + 0x10, [0xFF, 0x5A, 0x2D], al);
        for j in 0..0x20u32 {
            let b = u32::from(fx.mem.read_u8(r + 8 + j));
            cell_uvs(fx, sp, b);
            fx.vu_scale4(sp + POS, r + 0x40 + 0x10 * j, f20);
            fx.vu_add4(sp + POS, sp + POS, r + 0x30);
            if sprite_at(fx, sp) {
                submit_w(fx, sp, R_C[2]);
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x11 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// Pool D: 4 x 0x130 (16 far-Z sprites, 4 updates) [F 0x1E02B00..0x1E03048].
fn pool_d(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0x56..0x5F).contains(&n) {
        spawn_pool(fx, D + 0x6580, 0x130, rem2(n, 4), 0x20, 0x30, 0x10, 8, f32::from_bits(0x3E4C_CCCD), R_D[0]);
    }
    pool_head(fx, sp, 0x10);
    for k in 0..4u32 {
        let r = D + 0x6580 + 0x130 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 4);
        let c = fx.tcos((age << 14) >> 2);
        let al = fixu(fx, ee::mul(128.0, c), R_D[1]) as u8;
        let age = fx.mem.i32(r + 4);
        let t = ee::div(ee::mul(f32::from_bits(0x3F33_3333), ee::from_i32(4 - age)), 4.0);
        corners(fx, sp, ee::mul(1.5, ee::add(f32::from_bits(0x3E99_999A), t)));
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let side = ee::from_i32(fx.mem.i32(D + 0xC));
        let f20 = ee::add(f32::from_bits(0x3DCC_CCCD), ee::div(ee::mul(f32::from_bits(0x3F66_6666), age), 4.0));
        let f21 = ee::mul(ee::div(ee::mul(7.0, age), 4.0), side);
        rgba(fx, sp + DESC + 0x10, [0xFF, 0x3C, 0x1E], al);
        for j in 0..0x10u32 {
            let b = u32::from(fx.mem.read_u8(r + 8 + j));
            cell_uvs(fx, sp, b);
            fx.vu_scale4(sp + POS, r + 0x30 + 0x10 * j, f20);
            fx.vu_add4(sp + POS, sp + POS, D + 0xB0);
            let z = fx.mem.f(sp + POS + 8);
            fx.mem.setf(sp + POS + 8, ee::add(z, f21));
            if sprite_at(fx, sp) {
                submit_far(fx, sp);
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 5 {
            fx.mem.write_u8(r, 0);
        }
    }
}
