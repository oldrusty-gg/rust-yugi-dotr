//! BMODULE b451 (card 587), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0x7280
//! bytes) [F]: a single unscheduled function in
//! a 0x510 frame (s0 = D), b483's lens family (crates/roses-fx/src/b483.rs)
//! on the attacker. Every update the camera turn with roll (0x2055E0 into
//! sp + 0x230), the attacker's point (0x1F65D0 kind 1, D+0x20) and its
//! node 0x33 (D+0x50). From update 0 for 0xE7 updates a lens: a sprite
//! (50, far Z, alpha 255 x age / 48 x the ramp 230 / 182) at the point,
//! then in a packet layer (TEXFLUSH, the frame copied to TBP 0x2000 by
//! 0x2095A0 / 0x207F50, TEXFLUSH) a disc of 32 rings x 16 quads (the
//! centre ring triangles) on the camera plane whose UVs are the screen
//! positions (0x100C60) of a warped copy of the disc (D+0x2270: radius x
//! (1 + 0.2 sin(0xD05 age + 0x1800 ring + 0x2000 k))) and positions those
//! of the plain disc (D+0x70, radius 25 (32 - ring) / 32), far Z,
//! unlinked; the layer at key 0. On 0x27..0xBD a record (+0x4470, 0x50,
//! 128 round robin) of two sprites a update at the node aimed at a random
//! point round the target (0x101080, 1.0), living 128 updates: the sprite
//! (prim 0x24, four corners 10 (0.8 + 0.2 age / 128) x the record's 0.5 +
//! 0.5 rand / 256 x (0.9 + 0.1 sin / cos (phase + 0x15B3 age))) at the
//! direction x (0.3 + 0.7 age / 128) (z x age / 128), alpha 188 x age /
//! 12; at its end a burst record (+0x6C80, 0x40, 24 round robin) of two
//! sprite pairs at the sprites' last points rising 1.25 age an update for
//! 25 updates (sizes 24 (0.5 + 0.5 age / 24) x 0.5 + 0.5 rand / 256,
//! alphas 128 / 64 cos((age << 14) / 24)). Colours (A8, 80, A8) (the lens
//! mesh (94, 80, 94), the bursts' second sprite (E4, 80, E4)). Returns 1
//! on update 0x9C, 6 on 0xA7..0x155, 2 on 0x156, 8 once the count reaches
//! 0x158. b614 (card 603) in a 0x540 frame: node 0xF, radius 12.5, D+0x24
//! += 2 an update, the lens for 0x12D updates (sprite 25, ramp 300 / 252,
//! colours (80, F8, 9E) / (80, 94, 9E)), the warped disc's rings also x
//! (1 + 0.1 cos(0x8AE age + 5 << 11 ring)) with wobble 0.1, records of
//! three sprites (0x70) on 0x86..0x10D rising 60 sin(32768 age / 128)
//! with scale 0.2 + 0.8 age / 128, sizes 10 (0.4 + 0.6 age / 128),
//! colour (80, F8, B2); 12 bursts (0x50) of three pairs for 13 updates
//! rising 10 age / 12, sizes 24 (0.5 + 0.5 age / 12), alphas 128 / 80
//! cos((age << 14) / 12), colours (80, F8, B2) / (80, F8, 94); returns on
//! 0xFB / 0x106.. / 0x19A / 0x19C. b450 (card 586) in a 0x4D0 frame with D
//! at sp + 0xD0 (locals 0x20 / 0x30 / 0x40 lower, the warp's ring pointer
//! and the record pointer in registers): no node (the records start at
//! the attacker's point D+0x20, raised 2 an update), one texture, the lens
//! flag / age at D+0x40 / D+0x44, the discs at D+0x50 / D+0x2250, the lens
//! for 0xDD updates (ramp 220 / 172, alpha 128, grey), 256 records (0x50)
//! of two sprites on 0x38..0xBD living 0x101 updates (alpha 128 age / 25,
//! sizes 5 (0.8 + 0.2 age / 256), grey), 24 bursts (0x40) of two single
//! sprites (alpha 100 cos((age << 14) / 24), sizes 12 (0.5 + 0.5 age /
//! 24), grey); returns on 0xAD / 0xB8.. / 0x1D6 / 0x1D8.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_z, KEEP, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;

/// What differs between the records.
struct V {
    frame: u32,
    /// Stack offsets: (from, shift) runs; an offset at or above `from`
    /// sits `shift` higher than b451's.
    sp_map: &'static [(u32, i32)],
    /// b451 locals the record has no slot for (b450 keeps the warp's ring
    /// pointer and the sprite record pointer in registers).
    skip: &'static [u32],
    bzero: u32,
    /// Textures (5, 0..this) into D+0x10.
    textures: u32,
    /// The attacker's node requested into D+0x40 and read into D+0x50
    /// every update (b450 none).
    node: Option<i32>,
    /// The lens flag (byte) and its age (b451 D+0x60 / D+0x64, b450
    /// D+0x40 / D+0x44).
    lens_flag: u32,
    lens_age: u32,
    /// Where a new record starts (D+0x50 the node, b450 D+0x20 the point).
    spawn_src: u32,
    /// rand's saved s0: D (s0 = D) or the record pointer + k (b450).
    rand_s0_d: bool,
    /// The discs' outer radius (25).
    radius: f32,
    /// Data offsets: the plain disc, the warped disc, the sprite records
    /// (size, count, sprites each, life), the burst index and records.
    rings: u32,
    warp: u32,
    recs: u32,
    rec_size: u32,
    rec_count: i32,
    rec_sprites: u32,
    rec_life: i32,
    /// A record's phases, position and directions (after the cells +8
    /// and sizes +0xC: b451 +0x14 / +0x20 / +0x30, b614 +0x18 / +0x30 /
    /// +0x40).
    rec_phase: u32,
    rec_pos: u32,
    rec_dir: u32,
    burst_index: u32,
    bursts: u32,
    burst_size: u32,
    burst_count: i32,
    burst_sprites: u32,
    burst_life: i32,
    /// D+0x24 += this every update (b614 / b450: 2.0).
    y_add: Option<f32>,
    lens_len: i32,
    spawn_from: i32,
    spawn_to: i32,
    lens_size: f32,
    lens_lo: f32,
    lens_hi: f32,
    /// The lens sprite's alpha scale (255; b450 128).
    lens_sprite_scale: f32,
    lens_sprite_colour: [u8; 3],
    lens_mesh_colour: [u8; 3],
    /// The warped disc's wobble (0.2 sin(...)).
    wobble: f32,
    /// b614: each ring's radius x (1 + this x cos(0x8AE age + the ring
    /// phase)), the phase (5 << 16 a ring) kept in the raw stack local.
    ring_sway: Option<(u32, f32)>,
    /// The sprites' alpha `sprite_alpha` x age / `sprite_alpha_div`.
    sprite_alpha: f32,
    sprite_alpha_div: f32,
    /// The sprites' size `size_mul` (`size_base` + `size_k` age /
    /// `size_div`) x the record's.
    size_base: f32,
    size_k: f32,
    size_mul: f32,
    size_div: f32,
    motion: Motion,
    sprite_colour: [u8; 3],
    burst_colour: [[u8; 3]; 2],
    /// The burst alpha angle's shift ((age << 14) x 0x2AAAAAAB >> 32 >>
    /// this).
    burst_shift: u32,
    burst_alpha1: f32,
    burst_alpha2: f32,
    /// b450: one sprite a burst entry (the colour written once with the
    /// first alpha, the texture set with the descriptor, no rise).
    burst_single: bool,
    /// The bursts' rise an update: `rise` x age / `rise_div`; the sprite
    /// size `burst_mul` (0.5 + 0.5 age / `burst_div`) x the record's.
    rise: f32,
    rise_div: f32,
    burst_mul: f32,
    burst_div: f32,
    one: i32,
    six_from: i32,
    two: i32,
    end: i32,
    /// Return addresses: the lens sprite alpha, the lens mesh alpha, the
    /// sprite alpha, the sprite key, the burst alphas (two), the burst
    /// keys (two).
    r_alpha: [u32; 8],
    /// rand in the spawn (three), in the burst (two).
    r_rand: [u32; 5],
    /// project_n: the triangles (two), the quads (two).
    r_proj: [u32; 4],
}

/// How a record's sprites move from the node along their direction.
enum Motion {
    /// x, y x (base + k age / 128), z x age / 128 (b451).
    Scale { base: f32, k: f32 },
    /// x, y x (base + k age / 128), z x age / 128, plus `amp` x sin(32768
    /// age / 128) (0x1A2B80 at `ra`) in y (b614).
    Rise { base: f32, k: f32, amp: f32, ra: u32 },
}

const B451: V = V {
    frame: 0x510,
    sp_map: &[(0, 0)],
    skip: &[],
    bzero: 0x7280,
    textures: 2,
    node: Some(0x33),
    lens_flag: 0x60,
    lens_age: 0x64,
    spawn_src: 0x50,
    rand_s0_d: true,
    radius: 25.0,
    rings: 0x70,
    warp: 0x2270,
    recs: 0x4470,
    rec_size: 0x50,
    rec_count: 0x80,
    rec_sprites: 2,
    rec_life: 0x80,
    rec_phase: 0x14,
    rec_pos: 0x20,
    rec_dir: 0x30,
    burst_index: 0x6C70,
    bursts: 0x6C80,
    burst_size: 0x40,
    burst_count: 0x18,
    burst_sprites: 2,
    burst_life: 0x19,
    y_add: None,
    lens_len: 0xE7,
    spawn_from: 0x27,
    spawn_to: 0xBE,
    lens_size: 50.0,
    lens_lo: 230.0,
    lens_hi: 182.0,
    lens_sprite_scale: 255.0,
    lens_sprite_colour: [0xA8, 0x80, 0xA8],
    lens_mesh_colour: [0x94, 0x80, 0x94],
    wobble: f32::from_bits(0x3E4C_CCCD),
    ring_sway: None,
    sprite_alpha: 188.0,
    sprite_alpha_div: 12.0,
    size_base: f32::from_bits(0x3F4C_CCCD),
    size_k: f32::from_bits(0x3E4C_CCCD),
    size_mul: 10.0,
    size_div: 128.0,
    motion: Motion::Scale { base: f32::from_bits(0x3E99_999A), k: f32::from_bits(0x3F33_3333) },
    sprite_colour: [0xA8, 0x80, 0xA8],
    burst_colour: [[0xA8, 0x80, 0xA8], [0xE4, 0x80, 0xE4]],
    burst_shift: 2,
    burst_alpha1: 128.0,
    burst_alpha2: 64.0,
    burst_single: false,
    rise: 30.0,
    rise_div: 24.0,
    burst_mul: 24.0,
    burst_div: 24.0,
    one: 0x9C,
    six_from: 0xA7,
    two: 0x156,
    end: 0x158,
    r_alpha: [0x01E0_03A8, 0x01E0_05C0, 0x01E0_0D90, 0x01E0_1128, 0x01E0_1390, 0x01E0_13D4, 0x01E0_15E4, 0x01E0_169C],
    r_rand: [0x01E0_0BBC, 0x01E0_0BDC, 0x01E0_0C28, 0x01E0_11D0, 0x01E0_11F4],
    r_proj: [0x01E0_07D8, 0x01E0_083C, 0x01E0_096C, 0x01E0_09E8],
};

const B614: V = V {
    frame: 0x540,
    sp_map: &[(0, 0x10), (0xF0, 0x20), (0x328, 0x30)],
    skip: &[],
    bzero: 0x8040,
    textures: 2,
    node: Some(0xF),
    lens_flag: 0x60,
    lens_age: 0x64,
    spawn_src: 0x50,
    rand_s0_d: true,
    radius: 12.5,
    rings: 0x70,
    warp: 0x2270,
    recs: 0x4470,
    rec_size: 0x70,
    rec_count: 0x80,
    rec_sprites: 3,
    rec_life: 0x80,
    rec_phase: 0x18,
    rec_pos: 0x30,
    rec_dir: 0x40,
    burst_index: 0x7C70,
    bursts: 0x7C80,
    burst_size: 0x50,
    burst_count: 0xC,
    burst_sprites: 3,
    burst_life: 0xD,
    y_add: Some(2.0),
    lens_len: 0x12D,
    spawn_from: 0x86,
    spawn_to: 0x10E,
    lens_size: 25.0,
    lens_lo: 300.0,
    lens_hi: 252.0,
    lens_sprite_scale: 255.0,
    lens_sprite_colour: [0x80, 0xF8, 0x9E],
    lens_mesh_colour: [0x80, 0x94, 0x9E],
    wobble: f32::from_bits(0x3DCC_CCCD),
    ring_sway: Some((0x100, f32::from_bits(0x3DCC_CCCD))),
    sprite_alpha: 188.0,
    sprite_alpha_div: 12.0,
    size_base: f32::from_bits(0x3ECC_CCCD),
    size_k: f32::from_bits(0x3F19_999A),
    size_mul: 10.0,
    size_div: 128.0,
    motion: Motion::Rise { base: f32::from_bits(0x3E4C_CCCD), k: f32::from_bits(0x3F4C_CCCD), amp: 60.0, ra: 0x01E0_0E84 },
    sprite_colour: [0x80, 0xF8, 0xB2],
    burst_colour: [[0x80, 0xF8, 0xB2], [0x80, 0xF8, 0x94]],
    burst_shift: 1,
    burst_alpha1: 128.0,
    burst_alpha2: 80.0,
    burst_single: false,
    rise: 10.0,
    rise_div: 12.0,
    burst_mul: 24.0,
    burst_div: 12.0,
    one: 0xFB,
    six_from: 0x106,
    two: 0x19A,
    end: 0x19C,
    r_alpha: [0x01E0_03C0, 0x01E0_05DC, 0x01E0_0E08, 0x01E0_11CC, 0x01E0_1440, 0x01E0_1484, 0x01E0_16A4, 0x01E0_1760],
    r_rand: [0x01E0_0C34, 0x01E0_0C54, 0x01E0_0CA0, 0x01E0_1280, 0x01E0_12A4],
    r_proj: [0x01E0_0850, 0x01E0_08B4, 0x01E0_09E4, 0x01E0_0A60],
};

const B450: V = V {
    frame: 0x4D0,
    sp_map: &[(0, 0), (0xEC, -0x20), (0x120, -0x30), (0x1B0, -0x40)],
    skip: &[0xF0, 0x110],
    bzero: 0x9A60,
    textures: 1,
    node: None,
    lens_flag: 0x40,
    lens_age: 0x44,
    spawn_src: 0x20,
    rand_s0_d: false,
    radius: 25.0,
    rings: 0x50,
    warp: 0x2250,
    recs: 0x4450,
    rec_size: 0x50,
    rec_count: 0x100,
    rec_sprites: 2,
    rec_life: 0x101,
    rec_phase: 0x14,
    rec_pos: 0x20,
    rec_dir: 0x30,
    burst_index: 0x9450,
    bursts: 0x9460,
    burst_size: 0x40,
    burst_count: 0x18,
    burst_sprites: 2,
    burst_life: 0x19,
    y_add: Some(2.0),
    lens_len: 0xDD,
    spawn_from: 0x38,
    spawn_to: 0xBE,
    lens_size: 50.0,
    lens_lo: 220.0,
    lens_hi: 172.0,
    lens_sprite_scale: 128.0,
    lens_sprite_colour: [0x80, 0x80, 0x80],
    lens_mesh_colour: [0x80, 0x80, 0x80],
    wobble: f32::from_bits(0x3E4C_CCCD),
    ring_sway: None,
    sprite_alpha: 128.0,
    sprite_alpha_div: 25.0,
    size_base: f32::from_bits(0x3F4C_CCCD),
    size_k: f32::from_bits(0x3E4C_CCCD),
    size_mul: 5.0,
    size_div: 256.0,
    motion: Motion::Scale { base: f32::from_bits(0x3E99_999A), k: f32::from_bits(0x3F33_3333) },
    sprite_colour: [0x80, 0x80, 0x80],
    burst_colour: [[0x80, 0x80, 0x80], [0x80, 0x80, 0x80]],
    burst_shift: 2,
    burst_alpha1: 100.0,
    burst_alpha2: 64.0,
    burst_single: true,
    rise: 30.0,
    rise_div: 24.0,
    burst_mul: 12.0,
    burst_div: 24.0,
    one: 0xAD,
    six_from: 0xB8,
    two: 0x1D6,
    end: 0x1D8,
    r_alpha: [0x01E0_03E0, 0x01E0_05F0, 0x01E0_0DC4, 0x01E0_1150, 0x01E0_1424, 0, 0x01E0_1600, 0],
    r_rand: [0x01E0_0BF0, 0x01E0_0C10, 0x01E0_0C5C, 0x01E0_1210, 0x01E0_123C],
    r_proj: [0x01E0_07F8, 0x01E0_085C, 0x01E0_0994, 0x01E0_0A10],
};

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B451)
}

/// b614's entry 0x1E00080.
pub fn program_b614(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B614)
}

/// b450's entry 0x1E00080.
pub fn program_b450(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B450)
}

fn run(fx: &mut Fx, param: i32, v: &V) -> i32 {
    let sp = fx.enter(v.frame);
    fx.mem.write_u32(at(sp, v, 0xB0), 0);
    fx.mem.write_u32(at(sp, v, 0xC8), OT);
    let ret = if param >= 0 { init(fx, v) } else { step(fx, sp, v) };
    fx.leave(v.frame);
    ret
}

/// A stack local: b451's offset mapped to the record's.
fn at(sp: u32, v: &V, off: u32) -> u32 {
    let shift = v.sp_map.iter().filter(|(from, _)| *from <= off).last().map_or(0, |(_, s)| *s);
    (sp + off).wrapping_add_signed(shift)
}

/// A stack local the record may keep in a register instead.
fn set_local(fx: &mut Fx, sp: u32, v: &V, off: u32, val: u32) {
    if !v.skip.contains(&off) {
        fx.mem.write_u32(at(sp, v, off), val);
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

/// A 4 x 8 cell `b` of the texture: (u0, v0, u1, v1).
fn cell(b: u8) -> (u32, u32, u32, u32) {
    let (c, r) = (u32::from(b & 3), u32::from(b >> 2));
    ((c << 9) & 0x3FFF, (r << 9) & 0x3FFF, ((32 * c + 32) << 4) & 0x3FFF, ((32 * r + 32) << 4) & 0x3FFF)
}

/// The sprite descriptor at sp + 0x328 (5 registers 0x42421, prim 0x26 |
/// 0x800) with alpha preset 1.
fn sprite_desc(fx: &mut Fx, sp: u32, v: &V) -> u32 {
    let d = at(sp, v, 0x328);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    d
}

/// The colour (c, a, Q = 1.0) at `p`.
fn colour(fx: &mut Fx, p: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(p, c[0]);
    fx.mem.write_u8(p + 1, c[1]);
    fx.mem.write_u8(p + 2, c[2]);
    fx.mem.write_u8(p + 3, a);
    fx.mem.write_u32(p + 4, 0x3F80_0000);
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

/// The plain disc: 32 rings (0x110) of 17 points, radius (32 - ring) / 32
/// x `radius`, at the table sine / cosine of 0x1000 k.
fn init(fx: &mut Fx, v: &V) -> i32 {
    fx.bzero(D, v.bzero);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    for k in 0..v.textures {
        let r = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x10 + 4 * k, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x30);
    if let Some(node) = v.node {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, 1);
        fx.mem.set32(D + 0x40, r);
    }
    fx.mem.write_u8(D + v.lens_flag, 0);
    for s1 in 0..0x20i32 {
        let f20 = ee::div(ee::mul(v.radius, ee::from_i32(0x20 - s1)), 32.0);
        for s2 in 0..0x11i32 {
            let a = (s2 << 16) >> 4;
            let o = D + v.rings + 0x110 * s1 as u32 + 0x10 * s2 as u32;
            let sn = fx.tsin(a);
            fx.mem.setf(o, ee::mul(f20, sn));
            let cs = fx.tcos(a);
            fx.mem.setf(o + 4, ee::mul(f20, cs));
            fx.mem.write_u32(o + 8, 0);
            fx.mem.write_u32(o + 0xC, 0x3F80_0000);
        }
    }
    for k in 0..v.rec_count as u32 {
        fx.mem.write_u8(D + v.recs + v.rec_size * k, 0);
    }
    fx.mem.write_u32(D + v.burst_index, 0);
    for k in 0..v.burst_count as u32 {
        fx.mem.write_u8(D + v.bursts + v.burst_size * k, 0);
    }
    0
}

/// The warped disc D+0x2270 [F 0x1E0067C]: ring s7's point s5 at the plain
/// radius x (1 + wobble x sin(0xD05 age + 0x1800 s7 + 0x2000 s5)).
fn warp(fx: &mut Fx, sp: u32, v: &V) {
    if let Some((local, _)) = v.ring_sway {
        fx.mem.write_u32(sp + local, 0);
    }
    set_local(fx, sp, v, 0xF0, D);
    for s7 in 0..0x20i32 {
        let sway = match v.ring_sway {
            Some((local, k)) => {
                let acc = fx.mem.i32(sp + local);
                let age = fx.mem.i32(D + v.lens_age);
                let m = fx.mult(age, 0x8AE);
                let a0 = (acc >> 5).wrapping_add(m) & 0xFFFF;
                let c = fx.tcos(a0);
                Some(ee::add(1.0, ee::mul(k, c)))
            }
            None => None,
        };
        let s3 = D + 0x110 * s7 as u32;
        let f21 = ee::div(ee::mul(v.radius, ee::from_i32(0x20 - s7)), 32.0);
        let f21 = match sway {
            Some(f3) => ee::mul(f21, f3),
            None => f21,
        };
        let s4 = (s7 * 0x30000) >> 5;
        for s5 in 0..0x11i32 {
            let age = fx.mem.i32(D + v.lens_age);
            let m = fx.mult(age, 0xD05);
            let a0 = m.wrapping_add(s4.wrapping_add((s5 * 0x20000) >> 4)) & 0xFFFF;
            let sn = fx.tsin(a0);
            let f1 = ee::mul(v.wobble, sn);
            let s6 = (s5 * 0x10000) >> 4;
            let f20 = ee::add(1.0, f1);
            let f20 = ee::mul(f21, f20);
            let o = s3 + v.warp + 0x10 * s5 as u32;
            let sn = fx.tsin(s6);
            fx.mem.setf(o, ee::mul(f20, sn));
            let cs = fx.tcos(s6);
            fx.mem.setf(o + 4, ee::mul(f20, cs));
            fx.mem.write_u32(o + 8, 0);
            fx.mem.write_u32(o + 0xC, 0x3F80_0000);
        }
        if let Some((local, _)) = v.ring_sway {
            let acc = fx.mem.i32(sp + local).wrapping_add(5 << 16);
            fx.mem.set32(sp + local, acc);
        }
        set_local(fx, sp, v, 0xF0, s3 + 0x110);
    }
}

/// The lens [F 0x1E00280..0x1E00B30] (D+0x60 = 1).
fn lens(fx: &mut Fx, sp: u32, v: &V) {
    let d1 = sprite_desc(fx, sp, v);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d1, rh, 1);
    fx.desc_clamp(d1, 0);
    uv(fx, at(sp, v, 0x340), 0);
    uv(fx, at(sp, v, 0x342), 0);
    uv(fx, at(sp, v, 0x350), 0x200);
    uv(fx, at(sp, v, 0x352), 0x200);
    square(fx, at(sp, v, 0x270), at(sp, v, 0x280), v.lens_size);
    let age = ee::from_i32(fx.mem.i32(D + v.lens_age));
    let f20 = Fx::ratio_below(age, 48.0);
    let age = ee::from_i32(fx.mem.i32(D + v.lens_age));
    let f20 = ee::mul(f20, Fx::ramp_above(age, v.lens_lo, v.lens_hi));
    let a = alpha(fx, ee::mul(v.lens_sprite_scale, f20), v.r_alpha[0]);
    colour(fx, at(sp, v, 0x338), v.lens_sprite_colour, a as u8);
    let (vp, m, cam) = (at(sp, v, 0x1B0), at(sp, v, 0x1F0), at(sp, v, 0x230));
    fx.camera_vp_to(vp);
    fx.translate_rows(m, cam, D + 0x20);
    let mm = fx.read_m4(m);
    fx.mul_into(Some(vp), &mm);
    // a2 / a3 at the first TEXFLUSH: 0x206F30 leaves them (as b483) [H:
    // after a rejected projection, 0x204F70's arguments].
    let mut a23 = [vp, d1];
    if crate::fxlib2::project(fx, at(sp, v, 0x270), PROJ_OUT, vp, d1, at(sp, v, 0x50C)) {
        far_z(fx, at(sp, v, 0x348), &[at(sp, v, 0x358)]);
        fx.submit(d1, OT, 0);
        a23 = [0, KEEP as u32];
    }
    // The lens descriptors: triangles sp + 0x3F8 (prim 0x33), quads
    // sp + 0x470 (prim 0x34), each vertex RGBAQ / UV / XYZ2.
    let (d2, d3) = (at(sp, v, 0x3F8), at(sp, v, 0x470));
    fx.mem.write_u16(d2 + 2, 0x33);
    fx.mem.write_u16(d2, 9);
    fx.mem.write_u64(d2 + 8, 0x4_2142_1421);
    fx.mem.write_u32(d2 + 4, 0);
    fx.mem.write_u16(d3 + 2, 0x34);
    fx.mem.write_u16(d3, 0xC);
    fx.mem.write_u64(d3 + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d3 + 4, 0);
    let layer = fx.layer_begin();
    fx.mem.write_u32(at(sp, v, 0xEC), layer);
    let (t0, t1) = (at(sp, v, 0x458), at(sp, v, 0x460));
    let regs = crate::lens::frame_copy(fx, [0x2142_1421, d2, a23[0], a23[1]], 1, t0, t1);
    let w = fx.mem.read_u64(t0);
    fx.mem.write_u64(at(sp, v, 0x4E8), w);
    let w = fx.mem.read_u64(t1);
    fx.mem.write_u64(at(sp, v, 0x4F0), w);
    fx.texflush([at(sp, v, 0x4E8), regs[1], regs[2], regs[3]]);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d2, a);
    fx.desc_clamp(d2, LENS_CLAMP);
    let f = fx.mem.read_u16(d2 + 2);
    fx.mem.write_u16(d2 + 2, f | 0x800);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d3, a);
    fx.desc_clamp(d3, LENS_CLAMP);
    let a = alpha(fx, ee::mul(128.0, f20), v.r_alpha[1]);
    let f = fx.mem.read_u16(d3 + 2);
    fx.mem.write_u16(d3 + 2, f | 0x800);
    colour(fx, at(sp, v, 0x408), v.lens_mesh_colour, a as u8);
    copy8(fx, at(sp, v, 0x420), at(sp, v, 0x408));
    copy8(fx, at(sp, v, 0x438), at(sp, v, 0x408));
    copy8(fx, at(sp, v, 0x500), at(sp, v, 0x408));
    for o in [0x480u32, 0x498, 0x4B0, 0x4C8] {
        copy8(fx, at(sp, v, o), at(sp, v, 0x500));
    }
    warp(fx, sp, v);
    let c = at(sp, v, 0x2F0);
    fx.mem.write_u32(c, 0);
    fx.mem.write_u32(c + 4, 0);
    fx.mem.write_u32(c + 8, 0);
    fx.mem.write_u32(c + 0xC, 0x3F80_0000);
    fx.mem.write_u32(at(sp, v, 0x190), 0);
    let w = at(sp, v, 0x50C);
    for i in 0..0x20u32 {
        let fp = D + 0x110 * i;
        if i == 0x1F {
            for s1 in 0..0x10u32 {
                let (s2, s4) = (fp + 0x10 * s1, fp + 0x10 * (s1 + 1));
                let r = fx.project_n(&[c, s2 + v.warp, s4 + v.warp], vp, d2, w, v.r_proj[0]);
                if r != 1 {
                    continue;
                }
                for (u, x) in [(0x410u32, 0x418u32), (0x428, 0x430), (0x440, 0x448)] {
                    fx.uv_from_xy(at(sp, v, u), at(sp, v, x), 0, 0);
                }
                let r = fx.project_n(&[c, s2 + v.rings, s4 + v.rings], vp, d2, w, v.r_proj[1]);
                if r == 1 {
                    far_z(fx, at(sp, v, 0x418), &[at(sp, v, 0x430), at(sp, v, 0x448)]);
                    fx.submit(d2, 0, 0);
                }
            }
        } else {
            let next = D + 0x110 * (i + 1);
            fx.mem.write_u32(at(sp, v, 0x160), next);
            for s6 in 0..0x10u32 {
                let (s2, s5) = (fp + 0x10 * s6, fp + 0x10 * (s6 + 1));
                let (s1, s4) = (next + 0x10 * s6, next + 0x10 * (s6 + 1));
                let r = fx.project_n(&[s2 + v.warp, s5 + v.warp, s1 + v.warp, s4 + v.warp], vp, d3, w, v.r_proj[2]);
                if r != 1 {
                    continue;
                }
                for (u, x) in [(0x488u32, 0x490u32), (0x4A0, 0x4A8), (0x4B8, 0x4C0), (0x4D0, 0x4D8)] {
                    fx.uv_from_xy(at(sp, v, u), at(sp, v, x), 0, 0);
                }
                let r = fx.project_n(&[s2 + v.rings, s5 + v.rings, s1 + v.rings, s4 + v.rings], vp, d3, w, v.r_proj[3]);
                if r == 1 {
                    far_z(fx, at(sp, v, 0x490), &[at(sp, v, 0x4A8), at(sp, v, 0x4C0), at(sp, v, 0x4D8)]);
                    fx.submit(d3, 0, 0);
                }
            }
        }
        fx.mem.write_u32(at(sp, v, 0x190), i + 1);
    }
    let layer = fx.mem.read_u32(at(sp, v, 0xEC));
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + v.lens_age) + 1;
    fx.mem.set32(D + v.lens_age, age);
    if age >= v.lens_len {
        fx.mem.write_u8(D + v.lens_flag, 0);
    }
}

/// 0.5 + 0.5 rand / 256 (a signed remainder of rand >> 23).
fn half_plus(fx: &mut Fx, s0: u32, ra: u32) -> f32 {
    let r = fx.rand_from(s0, ra) >> 23;
    let f = ee::from_i32(rem2(r, 256));
    ee::add(0.5, ee::div(ee::mul(0.5, f), 256.0))
}

/// On `spawn_from`..`spawn_to` [F 0x1E00B50]: record n % count of
/// sprites at the node aimed at a random point round the target.
fn spawn(fx: &mut Fx, v: &V, n: i32) {
    let e = rem2(n, v.rec_count);
    let rec = D + v.recs + v.rec_size * e as u32;
    fx.mem.write_u8(rec, 1);
    fx.mem.write_u32(rec + 4, 0);
    copy16(fx, rec + v.rec_pos, D + v.spawn_src);
    for k in 0..v.rec_sprites {
        let s0 = if v.rand_s0_d { D } else { D + v.rec_size * e as u32 + k };
        let r = fx.rand_from(s0, v.r_rand[0]) >> 23;
        fx.mem.set8(rec + 8 + k, rem2(r, 4));
        let f = half_plus(fx, s0, v.r_rand[1]);
        fx.mem.setf(rec + 0xC + 4 * k, f);
        let r = fx.rand_from(s0, v.r_rand[2]) >> 23;
        fx.mem.set32(rec + v.rec_phase + 4 * k, (rem2(r, 256) << 16) >> 8);
        let p = rec + v.rec_dir + 0x10 * k;
        fx.random_table_vec(p, 1.0);
        fx.vu_add4(p, p, D + 0x30);
        fx.vu_sub4(p, p, rec + v.rec_pos);
    }
}

/// The records' sprites [F 0x1E00C98]: prim 0x24 quads of four corners.
fn sprites(fx: &mut Fx, sp: u32, v: &V) {
    let d4 = at(sp, v, 0x380);
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
    let (vp, m, cam) = (at(sp, v, 0x1B0), at(sp, v, 0x1F0), at(sp, v, 0x230));
    fx.camera_vp_to(vp);
    fx.identity(m);
    let id = fx.read_m4(m);
    fx.mul_into(Some(vp), &id);
    let uvp = [at(sp, v, 0x39A), at(sp, v, 0x3AA), at(sp, v, 0x3BA), at(sp, v, 0x3CA)];
    for (k, p) in uvp.iter().enumerate() {
        fx.mem.write_u32(at(sp, v, 0x120 + 0x10 * k as u32), *p);
    }
    fx.mem.write_u32(at(sp, v, 0x180), 0);
    set_local(fx, sp, v, 0x110, D);
    let (c0, c1, c2, c3) = (at(sp, v, 0x270), at(sp, v, 0x280), at(sp, v, 0x290), at(sp, v, 0x2A0));
    let (pos, w) = (at(sp, v, 0x2F0), at(sp, v, 0x50C));
    for i in 0..v.rec_count {
        let rec = D + v.recs + v.rec_size * i as u32;
        if fx.mem.read_u8(rec) != 0 {
            let age = fx.mem.i32(rec + 4);
            let f0 = Fx::ratio_below(ee::from_i32(age), v.sprite_alpha_div);
            let a = alpha(fx, ee::mul(v.sprite_alpha, f0), v.r_alpha[2]);
            colour(fx, at(sp, v, 0x390), v.sprite_colour, a as u8);
            let age = fx.mem.i32(rec + 4);
            let c = Fx::clamp_i(age, 0, 0x80);
            let f21 = ee::div(ee::from_i32(c), 128.0);
            // (f20, f21, rise): the x / y scale, the z scale, the rise.
            let (f20, f21, rise) = match v.motion {
                Motion::Scale { base, k } => {
                    fx.mem.write_u32(at(sp, v, 0x100), rec + v.rec_pos);
                    (ee::add(base, ee::mul(k, f21)), f21, 0.0)
                }
                Motion::Rise { base, k, amp, ra } => {
                    let f1 = ee::mul(k, f21);
                    let x = ee::mul(32768.0, f21);
                    let scale = ee::add(base, f1);
                    fx.fix_residue(x, ra);
                    let a = ee::fixsfsi(x) & 0xFFFF;
                    let sn = fx.tsin(a);
                    let rise = ee::mul(amp, sn);
                    fx.mem.write_u32(at(sp, v, 0x100), rec + v.rec_pos);
                    (scale, f21, rise)
                }
            };
            for k in 0..v.rec_sprites {
                let age = fx.mem.i32(rec + 4);
                let f3 = ee::div(ee::mul(v.size_k, ee::from_i32(age)), v.size_div);
                let m = fx.mult(age, 0x15B3);
                let fp = fx.mem.i32(rec + v.rec_phase + 4 * k).wrapping_add(m) & 0xFFFF;
                let f2 = ee::add(v.size_base, f3);
                let f1 = ee::mul(v.size_mul, f2);
                let sz = fx.mem.f(rec + 0xC + 4 * k);
                let f22 = ee::mul(f1, sz);
                let sn = fx.tsin(fp);
                let f0 = ee::add(f32::from_bits(0x3F66_6666), ee::mul(f32::from_bits(0x3DCC_CCCD), sn));
                let f23 = ee::mul(f22, f0);
                let cs = fx.tcos(fp);
                let f0 = ee::add(f32::from_bits(0x3F66_6666), ee::mul(f32::from_bits(0x3DCC_CCCD), cs));
                let f0 = ee::mul(f22, f0);
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
                let b = fx.mem.read_u8(rec + 8 + k);
                let (u0, v0, u1, v1) = cell(b);
                uv(fx, at(sp, v, 0x398), u0);
                uv(fx, uvp[0], v0);
                uv(fx, at(sp, v, 0x3A8), u1);
                uv(fx, uvp[1], v0);
                uv(fx, at(sp, v, 0x3B8), u0);
                uv(fx, uvp[2], v1);
                uv(fx, at(sp, v, 0x3C8), u1);
                uv(fx, uvp[3], v1);
                let dir = rec + v.rec_dir + 0x10 * k;
                let (dx, dy, dz) = (fx.mem.f(dir), fx.mem.f(dir + 4), fx.mem.f(dir + 8));
                fx.mem.setf(pos, ee::mul(dx, f20));
                let y = ee::mul(dy, f20);
                let y = match v.motion {
                    Motion::Scale { .. } => y,
                    Motion::Rise { .. } => ee::add(rise, y),
                };
                fx.mem.setf(pos + 4, y);
                fx.mem.setf(pos + 8, ee::mul(dz, f21));
                let p = fx.mem.read_u32(at(sp, v, 0x100));
                fx.vu_add4(pos, pos, p);
                fx.vu_add4(at(sp, v, 0x2B0), c0, pos);
                fx.vu_add4(at(sp, v, 0x2C0), c1, pos);
                fx.vu_add4(at(sp, v, 0x2D0), c2, pos);
                fx.vu_add4(at(sp, v, 0x2E0), c3, pos);
                if crate::fxlib2::project(fx, at(sp, v, 0x2B0), PROJ_OUT, vp, d4, w) {
                    let wf = fx.mem.f(w);
                    let key = alpha(fx, wf, v.r_alpha[3]);
                    fx.submit(d4, OT, key as i32);
                }
                copy16(fx, at(sp, v, 0x300) + 0x10 * k, pos);
            }
            let age = fx.mem.i32(rec + 4) + 1;
            fx.mem.set32(rec + 4, age);
            if age >= v.rec_life {
                fx.mem.write_u8(rec, 0);
                let idx = fx.mem.i32(D + v.burst_index);
                let b = D + v.bursts + v.burst_size * idx as u32;
                fx.mem.write_u8(b, 1);
                fx.mem.write_u32(b + 4, 0);
                for k in 0..v.burst_sprites {
                    let s0 = if v.rand_s0_d { D } else { D + v.burst_size * idx as u32 + k };
                    let r = fx.rand_from(s0, v.r_rand[3]) >> 23;
                    fx.mem.set8(b + 8 + k, rem2(r, 4) + 4);
                    let f = half_plus(fx, s0, v.r_rand[4]);
                    fx.mem.setf(b + 0xC + 4 * k, f);
                    copy16(fx, b + 0x20 + 0x10 * k, at(sp, v, 0x300) + 0x10 * k);
                }
                let idx = fx.mem.i32(D + v.burst_index) + 1;
                fx.mem.set32(D + v.burst_index, idx);
                if idx >= v.burst_count {
                    fx.mem.write_u32(D + v.burst_index, 0);
                }
            }
        }
        set_local(fx, sp, v, 0x110, D + v.rec_size * (i + 1) as u32);
        fx.mem.set32(at(sp, v, 0x180), i + 1);
    }
}

/// The burst alpha: `scale` x cos(((age << 14) x 0x2AAAAAAB >> 32) >>
/// shift).
fn burst_alpha(fx: &mut Fx, v: &V, age: i32, scale: f32, ra: u32) -> u32 {
    let x = age << 14;
    fx.mult(0x2AAA_AAAB, x);
    let hi = fx.hilo().1;
    let a = (hi >> v.burst_shift).wrapping_add(((x as u32) >> 31) as i32);
    let c = fx.tcos(a);
    alpha(fx, ee::mul(scale, c), ra)
}

/// The bursts [F 0x1E012C0]: sprite pairs at the sprites' last points,
/// the second `rise` x age / `burst_div` higher.
fn bursts(fx: &mut Fx, sp: u32, v: &V) {
    let d1 = if v.burst_single {
        // The texture with the descriptor, then alpha preset 1.
        let d = at(sp, v, 0x328);
        fx.mem.write_u16(d + 2, 0x26);
        fx.mem.write_u16(d, 5);
        fx.mem.write_u64(d + 8, 0x4_2421);
        fx.mem.write_u32(d + 4, 0);
        let f = fx.mem.read_u16(d + 2);
        fx.mem.write_u16(d + 2, f | 0x800);
        let rh = fx.mem.read_u32(D + 0x10);
        fx.desc_texture(d, rh, 1);
        let a = fx.alpha_preset(1);
        fx.desc_alpha(d, a);
        d
    } else {
        sprite_desc(fx, sp, v)
    };
    fx.desc_clamp(d1, 0);
    let (vp, m, cam) = (at(sp, v, 0x1B0), at(sp, v, 0x1F0), at(sp, v, 0x230));
    fx.camera_vp_to(vp);
    fx.identity(m);
    let id = fx.read_m4(m);
    fx.mul_into(Some(vp), &id);
    fx.mem.write_u32(at(sp, v, 0x170), 0);
    let (c0, c1, pos, w) = (at(sp, v, 0x270), at(sp, v, 0x280), at(sp, v, 0x2F0), at(sp, v, 0x50C));
    let (va, vb) = (at(sp, v, 0x2B0), at(sp, v, 0x2C0));
    for i in 0..v.burst_count {
        let b = D + v.bursts + v.burst_size * i as u32;
        if fx.mem.read_u8(b) != 0 {
            let age = fx.mem.i32(b + 4);
            let a1 = burst_alpha(fx, v, age, v.burst_alpha1, v.r_alpha[4]);
            let mut f20 = 0.0;
            if v.burst_single {
                colour(fx, at(sp, v, 0x338), v.burst_colour[0], a1 as u8);
            } else {
                fx.mem.write_u8(at(sp, v, 0x1A0), a1 as u8);
                let age = fx.mem.i32(b + 4);
                let a2 = burst_alpha(fx, v, age, v.burst_alpha2, v.r_alpha[5]);
                fx.mem.write_u8(at(sp, v, 0xD0), a2 as u8);
                let age = ee::from_i32(fx.mem.i32(b + 4));
                f20 = ee::div(ee::mul(v.rise, age), v.rise_div);
            }
            for k in 0..v.burst_sprites {
                let age = ee::from_i32(fx.mem.i32(b + 4));
                let f1 = ee::div(ee::mul(0.5, age), v.burst_div);
                let f1 = ee::mul(v.burst_mul, ee::add(0.5, f1));
                let sz = fx.mem.f(b + 0xC + 4 * k);
                let f0 = ee::mul(f1, sz);
                square(fx, c0, c1, f0);
                fx.vu_apply_to(c0, cam, c0);
                fx.vu_apply_to(c1, cam, c1);
                let by = fx.mem.read_u8(b + 8 + k);
                let (u0, v0, u1, v1) = cell(by);
                uv(fx, at(sp, v, 0x340), u0);
                uv(fx, at(sp, v, 0x342), v0);
                uv(fx, at(sp, v, 0x350), u1);
                uv(fx, at(sp, v, 0x352), v1);
                copy16(fx, pos, b + 0x20 + 0x10 * k);
                fx.vu_add4(va, c0, pos);
                fx.vu_add4(vb, c1, pos);
                if !v.burst_single {
                    let rh = fx.mem.read_u32(D + 0x10);
                    fx.desc_texture(d1, rh, 1);
                    let a = fx.mem.read_u8(at(sp, v, 0x1A0));
                    colour(fx, at(sp, v, 0x338), v.burst_colour[0], a);
                }
                if crate::fxlib2::project(fx, va, PROJ_OUT, vp, d1, w) {
                    let wf = fx.mem.f(w);
                    let key = alpha(fx, wf, v.r_alpha[6]);
                    fx.submit(d1, OT, key as i32);
                }
                if v.burst_single {
                    continue;
                }
                let y = ee::add(fx.mem.f(pos + 4), f20);
                fx.mem.setf(pos + 4, y);
                fx.vu_add4(va, c0, pos);
                fx.vu_add4(vb, c1, pos);
                let rh = fx.mem.read_u32(D + 0x14);
                fx.desc_texture(d1, rh, 1);
                let a = fx.mem.read_u8(at(sp, v, 0xD0));
                colour(fx, at(sp, v, 0x338), v.burst_colour[1], a);
                if crate::fxlib2::project(fx, va, PROJ_OUT, vp, d1, w) {
                    let wf = fx.mem.f(w);
                    let key = alpha(fx, wf, v.r_alpha[7]);
                    fx.submit(d1, OT, key as i32);
                }
            }
            let age = fx.mem.i32(b + 4) + 1;
            fx.mem.set32(b + 4, age);
            if age >= v.burst_life {
                fx.mem.write_u8(b, 0);
            }
        }
        fx.mem.set32(at(sp, v, 0x170), i + 1);
    }
}

fn step(fx: &mut Fx, sp: u32, v: &V) -> i32 {
    fx.camera_turn_roll(at(sp, v, 0x230));
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x20);
    if v.node.is_some() {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x40));
        fx.node_position(h, slot, D + 0x50);
    }
    if let Some(a) = v.y_add {
        let y = ee::add(fx.mem.f(D + 0x24), a);
        fx.mem.setf(D + 0x24, y);
    }
    if fx.mem.i32(D) == 0 {
        fx.mem.write_u8(D + v.lens_flag, 1);
        fx.mem.write_u32(D + v.lens_age, 0);
    }
    if fx.mem.read_u8(D + v.lens_flag) == 1 {
        lens(fx, sp, v);
    }
    let n = fx.mem.i32(D);
    if n >= v.spawn_from && n < v.spawn_to {
        spawn(fx, v, n);
    }
    sprites(fx, sp, v);
    bursts(fx, sp, v);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == v.one {
        ret = 1;
    } else if n >= v.six_from && n < v.two {
        ret = 6;
    } else if n == v.two {
        ret = 2;
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= v.end {
        ret = 8;
    }
    fx.mem.write_u32(at(sp, v, 0xB0), ret as u32);
    ret
}
