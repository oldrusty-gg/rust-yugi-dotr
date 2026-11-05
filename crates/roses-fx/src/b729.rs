//! BMODULE b729 (card 506) and its twin b730 (card 621), hand-ported from
//! the records (linked at 0x1E00000, one function 0x1E00080, data area a0
//! = 0x1ED0000, 0x2080 bytes) [F]: a single
//! unscheduled function in a 0x540 frame (s2 = D, s0 the ordering table),
//! the b030 / b014 lens family. Every update the attacker's unit flag
//! +0x20A = 1 (0x1F69C0), the camera turn with roll (sp + 0x270), the
//! attacker's point (0x1F65D0 kind 1, D+0x40; b730 only on 0xDC), node 1
//! (b730 0x2C) into D+0x80, the attacker's screen point D+0x50 (b729 only
//! on 0x6B) and the target's D+0x60 (0x204D80, x / y clamped to 0..512).
//! On 0xBD (0x171) a lens (D+0x1180, 0x4B (0x5F) updates): in a packet
//! layer the frame copied as a texture into eight rings of the
//! eight-triangle fan 0x28ABE0 about the target's screen point (scaled 1
//! + 0.15 (8 - i) / 8), alpha 85 (L - age) / L. On the same update one
//! burst record (D+0xA0, 0x10E0 bytes, 64 (84) updates) at the target: 64
//! random 2.5 vectors with cells, 128 vectors, 64 vectors with cells 5..7;
//! drawn as 64 sprites thrown by the tan falloff (texture 2), 64 more in
//! two alternating blends (texture 0), a 195-wide turned glow (texture 1)
//! and 128 x 6 stretched sparks (texture 1). From 0x7B (0xEC) four flashes
//! (D+0x1190, one every 16th (33rd) update, 17 (34) updates) on the
//! attacker's screen point (texture 3); on 0x6B (0xDC) four quads of the
//! table 0x28AD70 turned 0x457 x age on the attacker's screen point at far
//! Z (texture 4, D+0x11B0, 0x53 (0x96) updates); on 0x19 (0x31) at node 1
//! a 48-wide glow, a 32-segment ring band (the init's three radii) and 128
//! sparks (D+0x11B8, 0x13 updates). Object op 0x1F83B0 on 0xA5 (0x135).
//! The stage light 0x80 - 80 clamp(end - n) clamp(n) / 1024. Returns 1 /
//! 6 / 2 / 8 (b729: 0xB3 / 0xBE..0xFC / 0xFD / 0x128; b730: 0x167 /
//! 0x172..0x1C4 / 0x1C5 / 0x1F0).
use crate::blib::exe::mul_mem;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_z, KEEP, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x540;
/// The sprite descriptor (prim 0x26 | 0x800, 5 registers).
const DESC: u32 = 0x388;
/// The lens' triangle descriptor.
const TRI: u32 = 0x4B0;
/// The quads' descriptor (prim 0x24 | 0x800, 9 registers).
const QUAD: u32 = 0x3E0;
/// The band's descriptor (prim 0x14, 8 registers).
const BAND: u32 = 0x458;
const FAN: u32 = 0x0028_ABE0;
/// The four quads (4 x 4 vec4, exe rodata).
const QUADS: u32 = 0x0028_AD70;
const FAR: u64 = 0xFF_FFF8;
/// The record (one, 0x10E0 bytes).
const REC: u32 = D + 0xA0;

/// What differs between the twins.
struct V {
    /// b730: D+0x40 cleared at the init too.
    init_d40: bool,
    node: i32,
    /// The attacker's point only on this update (else every update).
    point_at: Option<i32>,
    /// Its screen point only on this update (else every update).
    screen_at: Option<i32>,
    lens_at: i32,
    lens_life: i32,
    life: i32,
    col_a: [u8; 3],
    col_even: [u8; 3],
    col_odd: [u8; 3],
    col_glow: [u8; 3],
    col_spark: [u8; 3],
    flash_from: i32,
    flash_p: i32,
    quads_at: i32,
    quads_amp: f32,
    quads_ramp: (f32, f32),
    quads_life: i32,
    quads_uv: u32,
    band_at: i32,
    col_e1: [u8; 3],
    col_e2a: [u8; 3],
    col_e2b: [u8; 3],
    op_at: i32,
    op_mode: i32,
    light_end: i32,
    one: i32,
    six: (i32, i32),
    two: i32,
    end: i32,
    /// Return addresses (see `R_*`).
    ra: [u32; 31],
}

const R_FLAG: usize = 0;
const R_LENS: usize = 1;
const R_RAND1: usize = 13;
const R_RAND3: usize = 14;
const R_GLOW: usize = 15;
const R_SPARK: usize = 16;
const R_KEY: usize = 17;
const R_FLASH: usize = 18;
const R_QUAD: usize = 22;
const R_BAND_RAND: usize = 27;
const R_E1: usize = 28;
const R_PN: usize = 29;

const B729: V = V {
    init_d40: false,
    node: 1,
    point_at: None,
    screen_at: Some(0x6B),
    lens_at: 0xBD,
    lens_life: 0x4A,
    life: 0x40,
    col_a: [0x9B, 0x6C, 0x48],
    col_even: [0x80, 0x94, 0xFF],
    col_odd: [0x9B, 0x6C, 0x48],
    col_glow: [0xFF, 0x64, 0x1E],
    col_spark: [0xFF, 0xDC, 0xB4],
    flash_from: 0x7B,
    flash_p: 16,
    quads_at: 0x6B,
    quads_amp: 64.0,
    quads_ramp: (82.0, 66.0),
    quads_life: 0x53,
    quads_uv: 0x20,
    band_at: 0x19,
    col_e1: [0x80, 0x30, 0xFF],
    col_e2a: [0xE4, 0x94, 0xFF],
    col_e2b: [0x1C, 0x12, 0xFF],
    op_at: 0xA5,
    op_mode: 0x17,
    light_end: 0x127,
    one: 0xB3,
    six: (0xBE, 0xFD),
    two: 0xFD,
    end: 0x128,
    ra: [
        0x01E0_02A4, 0x01E0_06C0, 0x01E0_06CC, 0x01E0_0710, 0x01E0_071C, 0x01E0_0760, 0x01E0_076C, 0x01E0_0820, 0x01E0_082C, 0x01E0_0898, 0x01E0_08A4,
        0x01E0_0910, 0x01E0_091C, 0x01E0_0B10, 0x01E0_0BE8, 0x01E0_1564, 0x01E0_18F4, 0x01E0_19A8, 0x01E0_1B5C, 0x01E0_1C08, 0x01E0_1C14, 0x01E0_1C38,
        0x01E0_1DF8, 0x01E0_1F34, 0x01E0_1F40, 0x01E0_1F68, 0x01E0_1F74, 0x01E0_2158, 0x01E0_22DC, 0x01E0_2598, 0x01E0_2698,
    ],
};

const B730: V = V {
    init_d40: true,
    node: 0x2C,
    point_at: Some(0xDC),
    screen_at: None,
    lens_at: 0x171,
    lens_life: 0x5E,
    life: 0x54,
    col_a: [0x6C, 0x48, 0x9B],
    col_even: [0x94, 0xFF, 0x80],
    col_odd: [0x6C, 0x48, 0x9B],
    col_glow: [0x64, 0x1E, 0xFF],
    col_spark: [0xDC, 0xB4, 0xFF],
    flash_from: 0xEC,
    flash_p: 33,
    quads_at: 0xDC,
    quads_amp: 45.0,
    quads_ramp: (149.0, 133.0),
    quads_life: 0x96,
    quads_uv: 0,
    band_at: 0x31,
    col_e1: [0xFF, 0x30, 0x80],
    col_e2a: [0xFF, 0x94, 0xE4],
    col_e2b: [0xFF, 0x12, 0x1C],
    op_at: 0x135,
    op_mode: 0x3B,
    light_end: 0x1EF,
    one: 0x167,
    six: (0x172, 0x1C5),
    two: 0x1C5,
    end: 0x1F0,
    ra: [
        0x01E0_02B4, 0x01E0_06D8, 0x01E0_06E4, 0x01E0_0728, 0x01E0_0734, 0x01E0_0778, 0x01E0_0784, 0x01E0_0838, 0x01E0_0844, 0x01E0_08B0, 0x01E0_08BC,
        0x01E0_0928, 0x01E0_0934, 0x01E0_0B28, 0x01E0_0C00, 0x01E0_15BC, 0x01E0_194C, 0x01E0_1A00, 0x01E0_1BD4, 0x01E0_1C80, 0x01E0_1C8C, 0x01E0_1CD0,
        0x01E0_1E90, 0x01E0_1FCC, 0x01E0_1FD8, 0x01E0_2000, 0x01E0_200C, 0x01E0_21F0, 0x01E0_2374, 0x01E0_2630, 0x01E0_2730,
    ],
};

/// The entry 0x1E00080 of b729 (card 506; a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B729, 0x01E0_2C00)
}

/// The entry of b730 (card 621).
pub fn program_b730(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B730, 0x01E0_2D00)
}

fn run(fx: &mut Fx, param: i32, v: &V, rodata: u32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xB0, 0);
    let w = fx.mem.read_u32(rodata);
    fx.mem.write_u32(sp + 0x538, w);
    let w = fx.mem.read_u32(rodata + 8);
    fx.mem.write_u32(sp + 0x53C, w);
    let ret = if param >= 0 { init(fx, sp, v) } else { step(fx, sp, v) };
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

fn uv(fx: &mut Fx, a: u32, x: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (x & 0x3FFF) as u16);
}

/// The sprite's UVs (sp + 0x3A0 / 0x3A2, 0x3B0 / 0x3B2).
fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, sp + 0x3A0, u0);
    uv(fx, sp + 0x3A2, v0);
    uv(fx, sp + 0x3B0, u1);
    uv(fx, sp + 0x3B2, v1);
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

/// The camera's view-projection (x identity) at sp + 0x1F0.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x1F0);
    fx.identity(sp + 0x230);
    mul_mem(fx, sp + 0x1F0, sp + 0x230);
}

/// The camera's view-projection x the camera turn moved to `at` (and
/// scaled by `scale` rows when given) at sp + 0x1F0.
fn placed(fx: &mut Fx, sp: u32, at: u32, scale: Option<[f32; 4]>) {
    fx.camera_vp_to(sp + 0x1F0);
    fx.translate_rows(sp + 0x230, sp + 0x270, at);
    if let Some(s) = scale {
        vec4(fx, sp + 0x350, s);
        fx.scale_rows(sp + 0x230, sp + 0x350);
    }
    mul_mem(fx, sp + 0x1F0, sp + 0x230);
}

/// The corners (-s, -s, 0, 1) / (s, s, 0, 1) at sp + 0x2B0 / 0x2C0, turned
/// by the camera when `turn`.
fn corners(fx: &mut Fx, sp: u32, s: f32, turn: bool) {
    let n = ee::neg(s);
    vec4(fx, sp + 0x2B0, [n, n, 0.0, 1.0]);
    vec4(fx, sp + 0x2C0, [s, s, 0.0, 1.0]);
    if turn {
        fx.vu_apply_to(sp + 0x2B0, sp + 0x270, sp + 0x2B0);
        fx.vu_apply_to(sp + 0x2C0, sp + 0x270, sp + 0x2C0);
    }
}

/// The sprite at sp + 0x350 + the corners (into sp + 0x2F0 / 0x300)
/// projected (0x204F70); true when it projects.
fn sprite_at(fx: &mut Fx, sp: u32) -> bool {
    fx.vu_add4(sp + 0x2F0, sp + 0x2B0, sp + 0x350);
    fx.vu_add4(sp + 0x300, sp + 0x2C0, sp + 0x350);
    crate::fxlib2::project(fx, sp + 0x2F0, PROJ_OUT, sp + 0x1F0, sp + DESC, sp + 0x534)
}

/// The sprite projected from `input` drawn at the far Z.
fn draw_far(fx: &mut Fx, sp: u32, input: u32) {
    if crate::fxlib2::project(fx, input, PROJ_OUT, sp + 0x1F0, sp + DESC, sp + 0x534) {
        far_z(fx, sp + 0x3A8, &[sp + 0x3B8]);
        fx.submit(sp + DESC, OT, 0);
    }
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

fn init(fx: &mut Fx, sp: u32, v: &V) -> i32 {
    fx.bzero(D, 0x2080);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    for k in 0..5u32 {
        let r = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x10 + 4 * k, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x30);
    if v.init_d40 {
        vec4(fx, D + 0x40, [0.0, 0.0, 0.0, 1.0]);
    }
    vec4(fx, D + 0x50, [0.0, 0.0, 0.0, 1.0]);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, v.node, 1);
    fx.mem.set32(D + 0x70, r);
    fx.mem.write_u32(D + 0x90, 0);
    fx.mem.write_u8(REC, 0);
    fx.mem.write_u8(D + 0x1180, 0);
    fx.mem.write_u32(D + 0x1188, 0);
    fx.mem.write_u32(D + 0x118C, 0);
    for k in 0..4u32 {
        fx.mem.write_u8(D + 0x1190 + 8 * k, 0);
    }
    fx.mem.write_u8(D + 0x11B0, 0);
    fx.mem.write_u8(D + 0x11B8, 0);
    // The band's ring: 33 points at radii 16.5 / 26.4 / 27.
    for s1 in 0..0x21u32 {
        let a = (s1 << 11) as i32;
        let x = fx.tsin(a);
        fx.mem.setf(sp + 0x350, x);
        let y = fx.tcos(a);
        fx.mem.setf(sp + 0x354, y);
        fx.mem.write_u32(sp + 0x358, 0);
        fx.mem.write_u32(sp + 0x35C, 0x3F80_0000);
        let p = D + 0x30 * s1;
        fx.vu_scale4(p + 0x11C0, sp + 0x350, 16.5);
        fx.vu_scale4(p + 0x11D0, sp + 0x350, f32::from_bits(0x41D3_3333));
        fx.vu_scale4(p + 0x11E0, sp + 0x350, 27.0);
    }
    0
}

/// The screen point of `p` into `o` (x / y clamped to 0..512, Z + 8).
fn screen_point(fx: &mut Fx, sp: u32, p: u32, o: u32) {
    camera(fx, sp);
    let m = fx.read_m4(sp + 0x1F0);
    fx.project_at(p, &m, sp + 0x528, sp + 0x534);
    let x = (i32::from(fx.mem.read_u16(sp + 0x528)) >> 4) - 0x700;
    fx.mem.setf(o, ee::from_i32(x));
    let y = (i32::from(fx.mem.read_u16(sp + 0x52A)) >> 4) - 0x700;
    fx.mem.setf(o + 4, ee::from_i32(y));
    let z = (fx.mem.read_u64(sp + 0x528) << 8) >> 40;
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

fn step(fx: &mut Fx, sp: u32, v: &V) -> i32 {
    crate::b369::unit_flag(fx, 1, u64::from(OT), v.ra[R_FLAG]);
    fx.camera_turn_roll(sp + 0x270);
    let n = fx.mem.i32(D);
    if v.point_at.map_or(true, |a| a == n) {
        let h = fx.mem.i32(D + 4);
        fx.unit_point(1, h, D + 0x40);
    }
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x70));
    fx.node_position(h, slot, D + 0x80);
    let n = fx.mem.i32(D);
    if v.screen_at.map_or(true, |a| a == n) {
        screen_point(fx, sp, D + 0x40, D + 0x50);
    }
    screen_point(fx, sp, D + 0x30, D + 0x60);
    if fx.mem.i32(D) == v.lens_at {
        fx.mem.write_u8(D + 0x1180, 1);
        fx.mem.write_u32(D + 0x1184, 0);
    }
    if fx.mem.read_u8(D + 0x1180) == 1 {
        lens(fx, sp, v);
    }
    if fx.mem.i32(D) == v.lens_at {
        spawn(fx, v);
    }
    record(fx, sp, v);
    flashes(fx, sp, v);
    if fx.mem.i32(D) == v.quads_at {
        fx.mem.write_u8(D + 0x11B0, 1);
        fx.mem.write_u32(D + 0x11B4, 0);
    }
    if fx.mem.read_u8(D + 0x11B0) == 1 {
        quads(fx, sp, v);
    }
    if fx.mem.i32(D) == v.band_at {
        fx.mem.write_u8(D + 0x11B8, 1);
        fx.mem.write_u32(D + 0x11BC, 0);
        words(fx, D + 0x1870, D + 0x80, 4);
        for j in 0..0x80u32 {
            let r = fx.rand_from(OT, v.ra[R_BAND_RAND]) >> 23;
            fx.mem.set8(D + 0x17F0 + j, r % 4);
            fx.random_table_vec(D + 0x1880 + 0x10 * j, 0.5);
        }
    }
    if fx.mem.read_u8(D + 0x11B8) == 1 {
        band(fx, sp, v);
    }
    let n = fx.mem.i32(D);
    if n == v.op_at {
        let h = fx.mem.read_u32(D + 4);
        fx.boundary(0x001F_83B0, "object op 0x1F83B0", [h, v.op_mode as u32, 0, sp + 0x538]);
    }
    let n = fx.mem.i32(D);
    let a = Fx::clamp_i(v.light_end - n, 0, 0x20);
    let n = fx.mem.i32(D);
    let b = Fx::clamp_i(n, 0, 0x20);
    let m = fx.mult(a, (-(b << 2) - b) << 4);
    fx.set_stage_light(((m >> 10) + 0x80) & 0xFF);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == v.one {
        ret = 1;
    } else {
        if n >= v.six.0 && n < v.six.1 {
            ret = 6;
        }
        if n == v.two {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= v.end {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xB0, ret as u32);
    ret
}

/// The lens [F 0x1E00514..0x1E009EC].
fn lens(fx: &mut Fx, sp: u32, v: &V) {
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
    let regs = crate::lens::frame_copy(fx, a, 1, sp + 0x510, sp + 0x518);
    fx.texflush(regs);
    let age = fx.mem.i32(D + 0x1184);
    let al = (85 * (v.lens_life - age)) / v.lens_life;
    let p = sp + 0x4C0;
    rgba(fx, p, [0x80, 0x80, 0x80], al as u8);
    for o in [0x4D8u32, 0x4F0] {
        words(fx, sp + o, p, 2);
    }
    let c = D + 0x60;
    let r = &v.ra[R_LENS..R_LENS + 12];
    for i in 0..8 {
        let f20 = ee::add(1.0, ee::div(ee::mul(f32::from_bits(0x3E19_999A), ee::from_i32(8 - i)), 8.0));
        for k in 0..8u32 {
            let s3 = FAN + 0x10 * k;
            let x = fix_at(fx, c, r[0]);
            let y = fix_at(fx, c + 4, r[1]);
            uv(fx, sp + 0x4C8, (x << 4) as u32);
            uv(fx, sp + 0x4CA, (y << 4) as u32);
            let x = fix_at(fx, s3, r[2]);
            let y = fix_at(fx, s3 + 4, r[3]);
            uv(fx, sp + 0x4E0, (x << 4) as u32);
            uv(fx, sp + 0x4E2, (y << 4) as u32);
            let x = fix_at(fx, s3 + 0x10, r[4]);
            let y = fix_at(fx, s3 + 0x14, r[5]);
            uv(fx, sp + 0x4F8, (x << 4) as u32);
            uv(fx, sp + 0x4FA, (y << 4) as u32);
            fx.vu_sub4(sp + 0x2F0, s3, c);
            fx.vu_sub4(sp + 0x300, s3 + 0x10, c);
            fx.vu_scale4(sp + 0x2F0, sp + 0x2F0, f20);
            fx.vu_scale4(sp + 0x300, sp + 0x300, f20);
            fx.vu_add4(sp + 0x2F0, sp + 0x2F0, c);
            fx.vu_add4(sp + 0x300, sp + 0x300, c);
            let x = fix_at(fx, c, r[6]);
            let y = fix_at(fx, c + 4, r[7]);
            xyz_far(fx, sp + 0x4D0, x, y);
            let x = fix_at(fx, sp + 0x2F0, r[8]);
            let y = fix_at(fx, sp + 0x2F4, r[9]);
            xyz_far(fx, sp + 0x4E8, x, y);
            let x = fix_at(fx, sp + 0x300, r[10]);
            let y = fix_at(fx, sp + 0x304, r[11]);
            xyz_far(fx, sp + 0x500, x, y);
            fx.submit(d, 0, 0);
        }
    }
    let layer = fx.mem.read_u32(sp + 0xDC);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0x1184) + 1;
    fx.mem.set32(D + 0x1184, age);
    if age >= v.lens_life + 1 {
        fx.mem.write_u8(D + 0x1180, 0);
    }
}

/// The burst record's spawn [F 0x1E009EC..0x1E00C74].
fn spawn(fx: &mut Fx, v: &V) {
    for s1 in 0..1u32 {
        fx.mem.write_u32(D + 0x90, s1);
        let r = REC + 0x10E0 * fx.mem.read_u32(D + 0x90);
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 1);
        vec4(fx, r + 0x90, [0.0, 0.0, 0.0, 1.0]);
        fx.vu_add4(r + 0x90, r + 0x90, D + 0x30);
        for j in 0..0x40u32 {
            let b = fx.rand_from(OT, v.ra[R_RAND1]) >> 23;
            fx.mem.set16(r + 8 + 2 * j, b % 8);
            fx.random_table_vec(r + 0xA0 + 0x10 * j, 2.5);
        }
        for j in 0..0x80u32 {
            fx.random_table_vec(r + 0x4A0 + 0x10 * j, 2.5);
        }
        for j in 0..0x40u32 {
            let b = fx.rand_from(OT, v.ra[R_RAND3]) >> 23;
            let (_, h) = fx.div(b, 3);
            fx.mem.set8(r + 0xCA0 + j, h + 5);
            fx.random_table_vec(r + 0xCE0 + 0x10 * j, 2.5);
        }
    }
}

/// The burst record's draws [F 0x1E00C74..0x1E01A44].
fn record(fx: &mut Fx, sp: u32, v: &V) {
    for i in 0..1u32 {
        let r = REC + 0x10E0 * i;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let life = v.life;
        let lf = ee::from_i32(life);
        let pos = r + 0x90;
        // 64 sprites thrown along the third vectors (texture 2).
        head(fx, sp);
        texture(fx, sp + DESC, 0x18);
        fx.desc_clamp(sp + DESC, 5);
        alpha(fx, sp + DESC, 1);
        camera(fx, sp);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let s = ee::mul(15.0, ee::add(f32::from_bits(0x3F4C_CCCD), ee::div(ee::mul(f32::from_bits(0x3E4C_CCCD), age), lf)));
        corners(fx, sp, s, true);
        let age = fx.mem.i32(r + 4);
        rgba(fx, sp + 0x398, v.col_a, (((life - age) * 255) / life) as u8);
        let age = fx.mem.i32(r + 4);
        let f22 = fx.tan_falloff_p(age, life, 1);
        for j in 0..0x40u32 {
            fx.vu_scale4(sp + 0x350, r + 0xCE0 + 0x10 * j, f22);
            fx.vu_add4(sp + 0x350, sp + 0x350, pos);
            let b = u32::from(fx.mem.read_u8(r + 0xCA0 + j));
            if sprite_cell(fx, sp, b) {
                far_z(fx, sp + 0x3A8, &[sp + 0x3B8]);
                fx.submit(sp + DESC, OT, 0);
            }
        }
        // 64 sprites along the first vectors, two blends (texture 0).
        head(fx, sp);
        texture(fx, sp + DESC, 0x10);
        fx.desc_clamp(sp + DESC, 5);
        camera(fx, sp);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let s = ee::mul(15.0, ee::add(f32::from_bits(0x3F4C_CCCD), ee::div(ee::mul(f32::from_bits(0x3E4C_CCCD), age), lf)));
        corners(fx, sp, s, true);
        let age = fx.mem.i32(r + 4);
        let a_odd = ((life - age) * 255) / life;
        let a_even = ((life - age) * 100) / life;
        for j in 0..0x40u32 {
            let age = fx.mem.i32(r + 4);
            let mut f22 = fx.tan_falloff_p(age, life, 1);
            if j % 2 == 0 {
                f22 = ee::mul(f22, f32::from_bits(0x3F4C_CCCD));
                alpha(fx, sp + DESC, 2);
                rgba(fx, sp + 0x398, v.col_even, a_even as u8);
            } else {
                alpha(fx, sp + DESC, 1);
                rgba(fx, sp + 0x398, v.col_odd, a_odd as u8);
            }
            fx.vu_scale4(sp + 0x350, r + 0xA0 + 0x10 * j, f22);
            fx.vu_add4(sp + 0x350, sp + 0x350, pos);
            let b = u32::from(fx.mem.read_u16(r + 8 + 2 * j));
            if sprite_cell(fx, sp, b) {
                far_z(fx, sp + 0x3A8, &[sp + 0x3B8]);
                fx.submit(sp + DESC, OT, 0);
            }
        }
        // The glow (texture 1).
        head(fx, sp);
        texture(fx, sp + DESC, 0x14);
        fx.desc_clamp(sp + DESC, 5);
        alpha(fx, sp + DESC, 1);
        fx.camera_vp_to(sp + 0x1F0);
        fx.translate_rows(sp + 0x230, sp + 0x270, pos);
        let age = fx.mem.i32(r + 4);
        let tf = fx.tan_falloff_p(age, life, 1);
        let g = ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), tf));
        vec4(fx, sp + 0x350, [g, g, g, 1.0]);
        fx.scale_rows(sp + 0x230, sp + 0x350);
        mul_mem(fx, sp + 0x1F0, sp + 0x230);
        let age = fx.mem.i32(r + 4);
        let f1 = ee::div(ee::from_i32(life - age), lf);
        let al = fixu(fx, ee::mul(228.0, f1), v.ra[R_GLOW]);
        rgba(fx, sp + 0x398, v.col_glow, al as u8);
        uvs(fx, sp, 0x600, 0, 0x800, 0x200);
        vec4(fx, sp + 0x2B0, [-195.0, -195.0, 0.0, 1.0]);
        vec4(fx, sp + 0x2C0, [195.0, 195.0, 0.0, 1.0]);
        draw_far(fx, sp, sp + 0x2B0);
        // 128 x 6 stretched sparks (texture 1).
        head(fx, sp);
        texture(fx, sp + DESC, 0x14);
        fx.desc_clamp(sp + DESC, 5);
        alpha(fx, sp + DESC, 1);
        uvs(fx, sp, 0x600, 0, 0x800, 0x200);
        camera(fx, sp);
        corners(fx, sp, 1.0, true);
        let age = fx.mem.i32(r + 4);
        let f23 = ee::div(ee::from_i32(life - age), lf);
        let age = fx.mem.i32(r + 4);
        let f22 = fx.tan_falloff_p(age, life, 1);
        for j in 0..0x80u32 {
            let p = r + 0x4A0 + 0x10 * j;
            for s1 in 0..6i32 {
                let k = ((s1 + 6) * 255) / 12;
                let al = fixu(fx, ee::mul(f23, ee::from_i32(k)), v.ra[R_SPARK]);
                rgba(fx, sp + 0x398, v.col_spark, al as u8);
                let f = ee::add(f22, ee::mul(f32::from_bits(0x3BE5_6042), ee::from_i32(s1)));
                fx.vu_scale4(sp + 0x350, p, f);
                fx.vu_add4(sp + 0x350, sp + 0x350, pos);
                if sprite_at(fx, sp) {
                    let w = fx.mem.f(sp + 0x534);
                    let key = fixu(fx, w, v.ra[R_KEY]);
                    fx.submit(sp + DESC, OT, key as i32);
                }
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= life + 1 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The cell's UVs then the sprite projected.
fn sprite_cell(fx: &mut Fx, sp: u32, b: u32) -> bool {
    fx.vu_add4(sp + 0x2F0, sp + 0x2B0, sp + 0x350);
    fx.vu_add4(sp + 0x300, sp + 0x2C0, sp + 0x350);
    cell_uvs(fx, sp, b);
    crate::fxlib2::project(fx, sp + 0x2F0, PROJ_OUT, sp + 0x1F0, sp + DESC, sp + 0x534)
}

/// The four flashes on the attacker's screen point [F 0x1E01A44..0x1E01D24].
fn flashes(fx: &mut Fx, sp: u32, v: &V) {
    if fx.mem.i32(D) >= v.flash_from {
        let c = fx.mem.i32(D + 0x118C);
        if c % v.flash_p == 0 {
            let k = fx.mem.i32(D + 0x1188);
            if k < 4 {
                let g = D + 8 * k as u32;
                fx.mem.write_u8(g + 0x1190, 1);
                fx.mem.write_u32(g + 0x1194, 0);
                fx.mem.set32(D + 0x1188, k + 1);
            }
        }
        let c = fx.mem.i32(D + 0x118C);
        fx.mem.set32(D + 0x118C, c + 1);
    }
    head(fx, sp);
    texture(fx, sp + DESC, 0x1C);
    fx.desc_clamp(sp + DESC, 5);
    alpha(fx, sp + DESC, 1);
    let r = &v.ra[R_FLASH..R_FLASH + 4];
    for k in 0..4u32 {
        let g = D + 8 * k + 0x1190;
        if fx.mem.read_u8(g) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 4);
        let c = fx.tcos((age << 14) / v.flash_p);
        let al = fixu(fx, ee::mul(128.0, c), r[0]);
        rgba(fx, sp + 0x398, [0x80, 0x80, 0x80], al as u8);
        let u = 0xC0 - 0x40 * k;
        uvs(fx, sp, u << 4, 0, (u + 0x40) << 4, 0x400);
        let sx = fix_at(fx, D + 0x50, r[1]);
        let sy = fix_at(fx, D + 0x54, r[2]);
        let age = fx.mem.i32(g + 4);
        let c = fx.tcos((age << 14) / v.flash_p);
        let h = fixs(fx, ee::mul(70.0, c), r[3]);
        xyz_far(fx, sp + 0x3A8, sx - h, sy - h);
        xyz_far(fx, sp + 0x3B8, h + sx, h + sy);
        fx.submit(sp + DESC, OT, 0);
        let age = fx.mem.i32(g + 4) + 1;
        fx.mem.set32(g + 4, age);
        if age >= v.flash_p + 1 {
            fx.mem.write_u8(g, 0);
        }
    }
}

/// The four turned quads on the attacker's screen point [F
/// 0x1E01D40..0x1E0210C].
fn quads(fx: &mut Fx, sp: u32, v: &V) {
    let d = sp + QUAD;
    fx.mem.write_u16(d + 2, 0x24);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, 0x4_2424_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    texture(fx, d, 0x20);
    fx.desc_clamp(d, 5);
    alpha(fx, d, 0);
    let age = ee::from_i32(fx.mem.i32(D + 0x11B4));
    let f0 = Fx::ratio_below(age, 16.0);
    let f20 = ee::mul(v.quads_amp, f0);
    let age = ee::from_i32(fx.mem.i32(D + 0x11B4));
    let f0 = Fx::ramp_above(age, v.quads_ramp.0, v.quads_ramp.1);
    let r = &v.ra[R_QUAD..R_QUAD + 5];
    let al = fixu(fx, ee::mul(f20, f0), r[0]);
    rgba(fx, sp + 0x3F0, [0x80, 0x80, 0x80], al as u8);
    let (lo, hi) = (v.quads_uv, 0x400);
    for (o, x) in [(0x3F8u32, lo), (0x3FA, lo), (0x408, hi), (0x40A, lo), (0x418, lo), (0x41A, hi), (0x428, hi), (0x42A, hi)] {
        uv(fx, sp + o, x);
    }
    fx.identity(sp + 0x310);
    let age = fx.mem.i32(D + 0x11B4);
    let a = fx.mult(age, 0x457);
    fx.rotate_axis(sp + 0x310, a, 2);
    for q in 0..4u32 {
        for c in 0..4u32 {
            let s1 = sp + 0x2B0 + 0x10 * c;
            fx.vu_scale4(s1, QUADS + 0x40 * q + 0x10 * c, 100.0);
            fx.vu_apply_to(s1, sp + 0x310, s1);
            let y = fx.mem.f(s1 + 4);
            let y = fixs(fx, ee::mul(f32::from_bits(0x3FAA_AAA8), y), r[1]);
            fx.mem.set32(sp + 0x370 + 4 * c, y);
            let x = fix_at(fx, s1, r[2]);
            fx.mem.set32(sp + 0x360 + 4 * c, x);
        }
        let sx = fix_at(fx, D + 0x50, r[3]);
        let sy = fix_at(fx, D + 0x54, r[4]);
        for c in 0..4u32 {
            let x = fx.mem.i32(sp + 0x360 + 4 * c).wrapping_add(sx);
            let y = fx.mem.i32(sp + 0x370 + 4 * c).wrapping_add(sy);
            xyz_far(fx, sp + 0x400 + 0x10 * c, x, y);
        }
        fx.submit(d, OT, 0);
    }
    let age = fx.mem.i32(D + 0x11B4) + 1;
    fx.mem.set32(D + 0x11B4, age);
    if age >= v.quads_life {
        fx.mem.write_u8(D + 0x11B0, 0);
    }
}

/// The glow, ring band and sparks at node 1's point [F 0x1E02198..0x1E02AC0].
fn band(fx: &mut Fx, sp: u32, v: &V) {
    let at = D + 0x1870;
    // The 48-wide glow.
    head(fx, sp);
    texture(fx, sp + DESC, 0x14);
    alpha(fx, sp + DESC, 1);
    fx.desc_clamp(sp + DESC, 5);
    uvs(fx, sp, 0x600, 0, 0x800, 0x200);
    placed(fx, sp, at, None);
    let age = fx.mem.i32(D + 0x11BC);
    let tf = fx.tan_falloff_p(age, 0x12, 1);
    let al = fixu(fx, ee::mul(255.0, ee::sub(1.0, tf)), v.ra[R_E1]);
    rgba(fx, sp + 0x398, v.col_e1, al as u8);
    vec4(fx, sp + 0x2B0, [-48.0, -48.0, 0.0, 1.0]);
    vec4(fx, sp + 0x2C0, [48.0, 48.0, 0.0, 1.0]);
    draw_far(fx, sp, sp + 0x2B0);
    // The ring band.
    let d = sp + BAND;
    fx.mem.write_u16(d + 2, 0x14);
    fx.mem.write_u16(d, 8);
    fx.mem.write_u64(d + 8, 0x4141_4141);
    fx.mem.write_u32(d + 4, 0);
    alpha(fx, d, 1);
    fx.camera_vp_to(sp + 0x1F0);
    fx.translate_rows(sp + 0x230, sp + 0x270, at);
    let age = fx.mem.i32(D + 0x11BC);
    let tf = fx.tan_falloff_p(age, 0x12, 1);
    let f20 = ee::add(f32::from_bits(0x3DCC_CCCD), ee::mul(f32::from_bits(0x3F66_6666), tf));
    vec4(fx, sp + 0x350, [f20, f20, 0.0, 1.0]);
    fx.scale_rows(sp + 0x230, sp + 0x350);
    mul_mem(fx, sp + 0x1F0, sp + 0x230);
    let age = fx.mem.i32(D + 0x11BC);
    let a = ((0x12 - age) * 255) / 18;
    rgba(fx, sp + 0x468, v.col_e2a, a as u8);
    words(fx, sp + 0x478, sp + 0x468, 2);
    rgba(fx, sp + 0x488, v.col_e2b, 0);
    words(fx, sp + 0x498, sp + 0x488, 2);
    for s6 in 0..0x20u32 {
        let s1 = D + 0x30 * s6;
        let s4 = D + 0x30 * (s6 + 1);
        for (k, outer) in [(0usize, 0x11C0u32), (1, 0x11E0)] {
            let r = fx.project_n(&[s1 + 0x11D0, s4 + 0x11D0, s1 + outer, s4 + outer], sp + 0x1F0, d, sp + 0x534, v.ra[R_PN + k]);
            if r == 1 {
                far_z(fx, sp + 0x470, &[sp + 0x480, sp + 0x490, sp + 0x4A0]);
                fx.submit(d, OT, 0);
            }
        }
    }
    // 128 sparks.
    head(fx, sp);
    texture(fx, sp + DESC, 0x14);
    alpha(fx, sp + DESC, 1);
    fx.desc_clamp(sp + DESC, 5);
    camera(fx, sp);
    let age = fx.mem.i32(D + 0x11BC);
    let a = ((0x12 - age) * 255) / 18;
    rgba(fx, sp + 0x398, v.col_e1, a as u8);
    corners(fx, sp, f32::from_bits(0x3F4C_CCCD), true);
    for j in 0..0x80u32 {
        let b = u32::from(fx.mem.read_u8(D + 0x17F0 + j));
        cell_uvs(fx, sp, b);
        fx.vu_scale4(sp + 0x350, D + 0x1880 + 0x10 * j, f20);
        fx.vu_add4(sp + 0x350, sp + 0x350, at);
        if sprite_at(fx, sp) {
            far_z(fx, sp + 0x3A8, &[sp + 0x3B8]);
            fx.submit(sp + DESC, OT, 0);
        }
    }
    let age = fx.mem.i32(D + 0x11BC) + 1;
    fx.mem.set32(D + 0x11BC, age);
    if age >= 0x13 {
        fx.mem.write_u8(D + 0x11B8, 0);
    }
}
