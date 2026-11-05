//! BMODULE b291 (card 571), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0xC470
//! bytes) [F]: a single unscheduled function in
//! a 0x570 frame (s1 = D, s0 the ordering table). Every update the camera
//! turn with roll (sp + at(v, 0x350)), the attacker's node 0x1A (D+0x30, y - 0.5)
//! aimed at the target's position (D+0x40: the turn D+0x50 = 0x100E40
//! then 0x4000 about x, the distance D+0x90 by powf / sqrtf). From 0x50
//! for 0x9B updates (D+0x2FA0) a tube: a 21 x 17 grid (D+0x1B0) along the
//! aim, radius 6 (0.5 + 0.5 clamp(age) / 32) x the soft-double (0.2 + 0.8
//! i / 20) (0.8 + 0.2 sin), drawn as a scrolling textured strip mesh
//! (texture 1), then in a packet layer the frame copied as a texture onto
//! a second, wobbling grid (D+0x1800, radius 5.5 ..., turned about y by
//! 5461 sin) through the first's screen points as UVs, culled by 0x101430.
//! Sprite pools (texture 2 / 3): 0x51..0xEA eight 80-sprite spirals
//! (D+0x2FB0, 8 updates), 0x71..0xEA eighteen 48-sprite bursts at the
//! target (D+0x5DB0, 18 updates), twelve 4-sprite flecks (D+0x9830, 12),
//! 0x51..0xEA twelve 4-sprite puffs at the node (D+0x9BF0, 12), 0x15..0x24
//! sixteen-sprite gathers (D+0xA070, 32 updates, drawn in towards the
//! node). Returns 1 on 0x66, 6 on 0x71..0xE9, 2 on 0xEA, 8 from 0xEB.
//!
//! b429 (card 576, data 0xA500 bytes) is a V [F]:
//! node 0x48 (no y offset), base vectors 4.0, the tube from 0x7F for 0xA0
//! updates (growth clamp 0x40 / 64, alpha 100 / 128, radii 9 / 8, soft
//! double 0.4 + 0.6 i / 20, wobble 8192, the lens copy not moved to the
//! centres, strip colour 0x80), windows 0x80 / 0xC0..0x11F, other sprite
//! colours / sizes, and in place of the gathers a node glow and eight blade
//! bursts ([`blades`]); returns 1 / 6 / 2 / 8 on 0xB5 / 0xC0..0x11D /
//! 0x11E / 0x120.
//!
//! b623 (card 604) and b624 (card 605) are Vs too [F]: their
//! frames (0x600 / 0x5A0) move b291's locals (`at`: 0x2D0.. + 0x10 and the
//! descriptors + 0x18 in b623; everything from 0x1C0 + 0x30 in b624), b624's
//! fields from D+0x40 sit 0x10 higher (`dz`) with its attacker point at
//! D+0x40, both keep D in s0 (rand's saved s0); b623 ends with a water sheet
//! ([`water`]), b624 with b429's glow / blades (far Z) and [`motes`].
use crate::blib::exe::mul_mem;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::LENS_CLAMP;

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x570;
/// The sprite descriptor (prim 0x26 | 0x800, 5 registers).
const DESC: u32 = 0x480;
/// The strip descriptor (prim 0x34, 12 registers).
const STRIP: u32 = 0x4D8;

/// A record of the family: b291 (card 571) or its V b429 (card 576) [F]
///.
struct V {
    bzero: u32,
    node: i32,
    /// The node's y offset (b291's - 0.5; b429 none).
    node_dy: Option<f32>,
    /// The burst base vectors' length (D+0x5AB0).
    base_len: f32,
    tube_on: i32,
    /// clamp(life - age, 0, 32) / 32.
    tube_life: i32,
    tube_end: i32,
    /// The age clamp of the tube's growth (and the spirals' spawn).
    agec: i32,
    tube_alpha: f32,
    r_tube: f32,
    r_lens: f32,
    /// The soft-double radius (add + mul i / 20).
    rad_add: f32,
    rad_mul: f32,
    wobble: f32,
    /// The lens grid's copy D+0x1800 moved to the centre points.
    lens_centre: bool,
    strip: [u8; 3],
    spirals: (i32, i32),
    sp_lo: i32,
    sp_amp: f32,
    sp_rgb: [u8; 3],
    /// The bursts' / flecks' window.
    bursts: (i32, i32),
    burst_vec: f32,
    burst_amp: f32,
    burst_rgb: [u8; 3],
    burst_size: f32,
    fleck_vec: f32,
    fleck_size: f32,
    /// tsin((age << 15) / 12) in place of tcos((age << 14) / 12).
    fleck_sin: bool,
    fleck_amp: f32,
    fleck_rgb: [u8; 3],
    puffs: (i32, i32),
    puff_amp: f32,
    puff_rgb: [u8; 3],
    /// The fades' end (clamp(end - n, 0, 32)).
    end: i32,
    /// Returns 1 on .0, 6 on .1..=.2 - 1, 2 on .2, 8 from .3.
    rets: (i32, i32, i32, i32),
    /// What follows the puffs.
    tail: Tail,
    /// Textures 5 / 0..tex into D+0x10.., then the node slot.
    tex: u32,
    slot: u32,
    /// rand's saved s0 (b291's OT, b623's D).
    rs0: u32,
    /// The lens grid's strip colour.
    strip_lens: [u8; 3],
    /// b291's return addresses -> this record's.
    ra: &'static [(u32, u32)],
    /// The frame layout: b291's offsets 0x2D0..0x480 + `lo`, 0x480..0x56C
    /// + `hi`, its mean-w slot 0x56C at `w`.
    lo: u32,
    hi: u32,
    w: u32,
    frame: u32,
    /// The shift of the fields from D+0x40.
    dsh: u32,
    /// b291's frame offsets 0x1C0..0x2D0 + `l2` (b624).
    l2: u32,
    /// The attacker's point (0x1F65D0 kind 1) into D+0x40 (b624).
    self_pt: bool,
    bl: &'static Bl,
}

const V291: V = V {
    bzero: 0xC470,
    node: 0x1A,
    node_dy: Some(-0.5),
    base_len: 3.0,
    tube_on: 0x50,
    tube_life: 0x9B,
    tube_end: 0x9C,
    agec: 0x20,
    tube_alpha: 128.0,
    r_tube: 6.0,
    r_lens: 5.5,
    rad_add: f32::from_bits(0x3E4C_CCCD),
    rad_mul: f32::from_bits(0x3F4C_CCCD),
    wobble: 5461.0,
    lens_centre: true,
    strip: [0x80, 0x94, 0x80],
    spirals: (0x51, 0xEB),
    sp_lo: 0x50,
    sp_amp: 30.0,
    sp_rgb: [0x94, 0xC8, 0xA8],
    bursts: (0x71, 0xEB),
    burst_vec: 0.5,
    burst_amp: 50.0,
    burst_rgb: [0x80, 0xC8, 0x91],
    burst_size: 10.0,
    fleck_vec: 1.5,
    fleck_size: 15.0,
    fleck_sin: false,
    fleck_amp: 50.0,
    fleck_rgb: [0x80, 0xA8, 0x91],
    puffs: (0x51, 0xEB),
    puff_amp: 64.0,
    puff_rgb: [0x80, 0xA8, 0x91],
    end: 0xEB,
    rets: (0x66, 0x71, 0xEA, 0xEC),
    tail: Tail::Gathers,
    tex: 4,
    slot: 0x20,
    rs0: OT,
    strip_lens: [0x80, 0x94, 0x80],
    ra: &[],
    lo: 0,
    hi: 0,
    w: 0x56C,
    frame: FRAME,
    dsh: 0,
    l2: 0,
    self_pt: false,
    bl: &BL429,
};

const V429: V = V {
    bzero: 0xA500,
    node: 0x48,
    node_dy: None,
    base_len: 4.0,
    tube_on: 0x7F,
    tube_life: 0xA0,
    tube_end: 0xA1,
    agec: 0x40,
    tube_alpha: 100.0,
    r_tube: 9.0,
    r_lens: 8.0,
    rad_add: f32::from_bits(0x3ECC_CCCD),
    rad_mul: f32::from_bits(0x3F19_999A),
    wobble: 8192.0,
    lens_centre: false,
    strip: [0x80, 0x80, 0x80],
    spirals: (0x80, 0x11F),
    sp_lo: 0x7F,
    sp_amp: 50.0,
    sp_rgb: [0x80, 0x80, 0x80],
    bursts: (0xC0, 0x11F),
    burst_vec: f32::from_bits(0x3E99_999A),
    burst_amp: 24.0,
    burst_rgb: [0xFF, 0xFF, 0xFF],
    burst_size: 20.0,
    fleck_vec: 2.0,
    fleck_size: 18.0,
    fleck_sin: true,
    fleck_amp: 30.0,
    fleck_rgb: [0xFF, 0xFF, 0xFF],
    puffs: (0x80, 0x11F),
    puff_amp: 96.0,
    puff_rgb: [0x80, 0x80, 0x80],
    end: 0x11F,
    rets: (0xB5, 0xC0, 0x11E, 0x120),
    tail: Tail::Blades,
    tex: 4,
    slot: 0x20,
    rs0: OT,
    strip_lens: [0x80, 0x80, 0x80],
    ra: &[
        (0x04C0, 0x04B0), (0x0934, 0x091C), (0x0948, 0x0930), (0x0A58, 0x0A48), (0x0C88, 0x0C74), (0x0F70, 0x0F40), (0x0FEC, 0x0FBC),
        (0x12C0, 0x1290), (0x12E0, 0x12B0), (0x1368, 0x1338), (0x154C, 0x151C), (0x1750, 0x1718), (0x1860, 0x1828), (0x19CC, 0x199C),
        (0x1BC0, 0x1B88), (0x1C9C, 0x1C64), (0x1E24, 0x1DEC), (0x1FF4, 0x1FB4), (0x211C, 0x20DC), (0x2298, 0x2258), (0x24BC, 0x2474),
    ],
    lo: 0,
    hi: 0,
    w: 0x56C,
    frame: FRAME,
    dsh: 0,
    l2: 0,
    self_pt: false,
    bl: &BL429,
};

#[derive(PartialEq)]
enum Tail {
    /// b291's gathers.
    Gathers,
    /// b429's node glow and eight blade bursts.
    Blades,
    /// b623's water sheet.
    Water,
}

/// b291's frame offset `x` as this record's.
fn at(v: &V, x: u32) -> u32 {
    match x {
        0x1C0..=0x2CF => x + v.l2,
        0x2D0..=0x47F => x + v.lo,
        0x480..=0x56B => x + v.hi,
        0x56C => v.w,
        _ => x,
    }
}

const V623: V = V {
    bzero: 0xD150,
    node: 0xE,
    node_dy: None,
    base_len: 4.0,
    tube_on: 0xC8,
    tube_life: 0x96,
    tube_end: 0x97,
    agec: 0x10,
    tube_alpha: 128.0,
    r_tube: 9.0,
    r_lens: 8.5,
    rad_add: f32::from_bits(0x3DCC_CCCD),
    rad_mul: f32::from_bits(0x3F66_6666),
    wobble: 5461.0,
    lens_centre: true,
    strip: [0x80, 0x80, 0x80],
    spirals: (0xC9, 0x15E),
    sp_lo: 0xC8,
    sp_amp: 80.0,
    sp_rgb: [0x6C, 0x94, 0x9B],
    bursts: (0xD9, 0x15E),
    burst_vec: f32::from_bits(0x3E99_999A),
    burst_amp: 60.0,
    burst_rgb: [0x6C, 0x91, 0x9B],
    burst_size: 20.0,
    fleck_vec: 2.0,
    fleck_size: 18.0,
    fleck_sin: true,
    fleck_amp: 60.0,
    fleck_rgb: [0x80, 0xB9, 0xC8],
    puffs: (0xC9, 0x15E),
    puff_amp: 96.0,
    puff_rgb: [0x6C, 0x94, 0x9B],
    end: 0x15E,
    rets: (0xCE, 0xD9, 0x15D, 0x21D),
    tail: Tail::Water,
    tex: 5,
    slot: 0x24,
    rs0: D,
    strip_lens: [0x80, 0x85, 0x8A],
    ra: &[
        (0x04C0, 0x05B8), (0x0934, 0x0A24), (0x0948, 0x0A38), (0x0A58, 0x0B48), (0x0C88, 0x0D78), (0x0F70, 0x1068), (0x0FEC, 0x10E4),
        (0x12C0, 0x13B8), (0x12E0, 0x13D8), (0x1368, 0x1460), (0x154C, 0x1644), (0x1750, 0x1848), (0x1860, 0x1958), (0x19CC, 0x1ACC),
        (0x1BC0, 0x1CC0), (0x1C9C, 0x1D9C), (0x1E24, 0x1F24), (0x1FF4, 0x20F4), (0x211C, 0x221C), (0x2298, 0x2398), (0x24BC, 0x25BC),
    ],
    lo: 0x10,
    hi: 0x18,
    w: 0x5FC,
    frame: 0x600,
    dsh: 0,
    l2: 0,
    self_pt: false,
    bl: &BL429,
};

const V624: V = V {
    bzero: 0xADA0,
    node: 0x1D,
    node_dy: None,
    base_len: 4.0,
    tube_on: 0x7D,
    tube_life: 0x9B,
    tube_end: 0x9C,
    agec: 0x10,
    tube_alpha: 100.0,
    r_tube: 10.0,
    r_lens: 9.0,
    rad_add: f32::from_bits(0x3DCC_CCCD),
    rad_mul: f32::from_bits(0x3F66_6666),
    wobble: 2730.0,
    lens_centre: false,
    strip: [0x58, 0x58, 0xD0],
    spirals: (0x7E, 0x118),
    sp_lo: 0x7D,
    sp_amp: 50.0,
    sp_rgb: [0x58, 0x58, 0xD0],
    bursts: (0x8E, 0x118),
    burst_vec: 0.5,
    burst_amp: 50.0,
    burst_rgb: [0x58, 0x58, 0xD0],
    burst_size: 20.0,
    fleck_vec: 2.0,
    fleck_size: 18.0,
    fleck_sin: true,
    fleck_amp: 30.0,
    fleck_rgb: [0x58, 0x58, 0xD0],
    puffs: (0x7E, 0x118),
    puff_amp: 96.0,
    puff_rgb: [0x58, 0x58, 0xD0],
    end: 0x118,
    rets: (0x83, 0x8E, 0x117, 0x119),
    tail: Tail::Blades,
    tex: 4,
    slot: 0x20,
    rs0: D,
    strip_lens: [0x80, 0x80, 0x8A],
    ra: &[
        (0x04C0, 0x04D0), (0x0934, 0x0944), (0x0948, 0x0958), (0x0A58, 0x0A70), (0x0C88, 0x0CA0), (0x0F70, 0x0F78), (0x0FEC, 0x0FF4),
        (0x12C0, 0x12C8), (0x12E0, 0x12E8), (0x1368, 0x1370), (0x154C, 0x1554), (0x1750, 0x1758), (0x1860, 0x1868), (0x19CC, 0x19D4),
        (0x1BC0, 0x1BC0), (0x1C9C, 0x1C9C), (0x1E24, 0x1E24), (0x1FF4, 0x1FEC), (0x211C, 0x2114), (0x2298, 0x2290), (0x24BC, 0x24AC),
    ],
    lo: 0x30,
    hi: 0x30,
    w: 0x59C,
    frame: 0x5A0,
    dsh: 0x10,
    l2: 0x30,
    self_pt: true,
    bl: &BL624,
};

/// b616 (card 474; helper C29): b429's code with node 0x29 (y - 0.5), the
/// tube from 0x78 for 0xA7 updates (growth clamp 0x10, soft double 0.1 +
/// 0.9 i / 20), windows 0x79 / 0x89..0x11F, every colour (80, 80, F8)
/// (the lens strip (80, 80, 8A)), bursts 0.5 / 50, and [`BL616`]'s glow
/// and blades; returns 1 / 6 / 2 / 8 on 0x7E / 0x89..0x11D / 0x11E /
/// 0x120.
const V616: V = V {
    bzero: 0xA400,
    node: 0x29,
    node_dy: Some(-0.5),
    tube_on: 0x78,
    tube_life: 0xA7,
    tube_end: 0xA8,
    agec: 0x10,
    rad_add: f32::from_bits(0x3DCC_CCCD),
    rad_mul: f32::from_bits(0x3F66_6666),
    strip: [0x80, 0x80, 0xF8],
    spirals: (0x79, 0x11F),
    sp_lo: 0x78,
    sp_rgb: [0x80, 0x80, 0xF8],
    bursts: (0x89, 0x11F),
    burst_vec: 0.5,
    burst_amp: 50.0,
    burst_rgb: [0x80, 0x80, 0xF8],
    fleck_rgb: [0x80, 0x80, 0xF8],
    puffs: (0x79, 0x11F),
    puff_rgb: [0x80, 0x80, 0xF8],
    rets: (0x7E, 0x89, 0x11E, 0x120),
    strip_lens: [0x80, 0x80, 0x8A],
    ra: &[
        (0x04C0, 0x04C8), (0x0934, 0x093C), (0x0948, 0x0950), (0x0A58, 0x0A68), (0x0C88, 0x0C94), (0x0F70, 0x0F70), (0x0FEC, 0x0FEC),
        (0x12C0, 0x12C0), (0x12E0, 0x12E0), (0x1368, 0x1368), (0x154C, 0x154C), (0x1750, 0x1750), (0x1860, 0x1860), (0x19CC, 0x19CC),
        (0x1BC0, 0x1BB8), (0x1C9C, 0x1C94), (0x1E24, 0x1E1C), (0x1FF4, 0x1FE4), (0x211C, 0x210C), (0x2298, 0x2288), (0x24BC, 0x24A4),
    ],
    bl: &BL616,
    ..V429
};

/// b616 (card 474), the same entry.
pub fn program_b616(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &V616)
}

/// b291's return address `a` (0x01E0_xxxx) as this record's.
fn ra(v: &V, a: u32) -> u32 {
    let lo = a - 0x01E0_0000;
    match v.ra.iter().find(|p| p.0 == lo) {
        Some(p) => 0x01E0_0000 + p.1,
        None => {
            debug_assert!(v.ra.is_empty(), "unmapped return address {a:#x}");
            a
        }
    }
}

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &V291)
}

/// b623 (card 604), the same entry.
pub fn program_b623(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &V623)
}

/// b624 (card 605), the same entry.
pub fn program_b624(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &V624)
}

/// b429 (card 576), the same entry.
pub fn program_b429(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &V429)
}

/// The base of the record's fields from 0x40 (b624's are 0x10 higher).
fn dz(v: &V) -> u32 {
    D + v.dsh
}

fn run(fx: &mut Fx, param: i32, v: &V) -> i32 {
    let sp = fx.enter(v.frame);
    fx.mem.write_u32(sp + 0xC0, 0);
    let ret = if param >= 0 { init(fx, v) } else { step(fx, sp, v) };
    fx.leave(v.frame);
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

fn uv(fx: &mut Fx, a: u32, x: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (x & 0x3FFF) as u16);
}

/// The 32 x 32 atlas cell of byte `b` (4 across) into the sprite's UVs.
fn cell_uvs(fx: &mut Fx, v: &V, sp: u32, b: u32) {
    let (c, r) = (b & 3, b >> 2);
    uv(fx, sp + at(v, 0x498), c << 9);
    uv(fx, sp + at(v, 0x49A), r << 9);
    uv(fx, sp + at(v, 0x4A8), (32 * c + 32) << 4);
    uv(fx, sp + at(v, 0x4AA), (32 * r + 32) << 4);
}

fn rgba(fx: &mut Fx, at: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(at, c[0]);
    fx.mem.write_u8(at + 1, c[1]);
    fx.mem.write_u8(at + 2, c[2]);
    fx.mem.write_u8(at + 3, a);
    fx.mem.write_u32(at + 4, 0x3F80_0000);
}

/// The sprite descriptor head (prim 0x26 | 0x800, 5 registers); texture
/// `tex` (filter 1), ALPHA preset 1, CLAMP 5 (`alpha_first`: the alpha
/// before the texture).
fn head(fx: &mut Fx, v: &V, sp: u32, tex: u32, preset: i32, alpha_first: bool) {
    let d = sp + at(v, DESC);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let rh = fx.mem.read_u32(D + tex);
    if alpha_first {
        let a = fx.alpha_preset(preset);
        fx.desc_alpha(d, a);
        fx.desc_texture(d, rh, 1);
    } else {
        fx.desc_texture(d, rh, 1);
        let a = fx.alpha_preset(preset);
        fx.desc_alpha(d, a);
    }
    fx.desc_clamp(d, 5);
}

/// The strip descriptor head (prim 0x34, 12 registers).
fn strip_head(fx: &mut Fx, v: &V, sp: u32) {
    let d = sp + at(v, STRIP);
    fx.mem.write_u16(d + 2, 0x34);
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d + 4, 0);
}

/// The camera's view-projection (x identity) at sp + at(v, 0x2D0).
fn camera(fx: &mut Fx, v: &V, sp: u32) {
    fx.camera_vp_to(sp + at(v, 0x2D0));
    fx.identity(sp + at(v, 0x310));
    mul_mem(fx, sp + at(v, 0x2D0), sp + at(v, 0x310));
}

/// The camera's view-projection x the aim D+0x50 at the node at sp + at(v, 0x2D0).
fn aimed(fx: &mut Fx, v: &V, sp: u32) {
    fx.camera_vp_to(sp + at(v, 0x2D0));
    fx.translate_rows(sp + at(v, 0x310), dz(v) + 0x50, D + 0x30);
    mul_mem(fx, sp + at(v, 0x2D0), sp + at(v, 0x310));
}

/// The corners (a, a, 0, 1) / (b, b, 0, 1) at sp + at(v, 0x390) / 0x3A0 turned by
/// the camera.
fn corners(fx: &mut Fx, v: &V, sp: u32, a: f32, b: f32) {
    vec4(fx, sp + at(v, 0x390), [a, a, 0.0, 1.0]);
    vec4(fx, sp + at(v, 0x3A0), [b, b, 0.0, 1.0]);
    fx.vu_apply_to(sp + at(v, 0x390), sp + at(v, 0x350), sp + at(v, 0x390));
    fx.vu_apply_to(sp + at(v, 0x3A0), sp + at(v, 0x350), sp + at(v, 0x3A0));
}

/// The sprite at sp + at(v, 0x450) + the corners (sp + at(v, 0x3D0) / 0x3E0) projected
/// and drawn keyed by its mean w.
fn sprite(fx: &mut Fx, v: &V, sp: u32, ra: u32) {
    fx.vu_add4(sp + at(v, 0x3D0), sp + at(v, 0x390), sp + at(v, 0x450));
    fx.vu_add4(sp + at(v, 0x3E0), sp + at(v, 0x3A0), sp + at(v, 0x450));
    draw(fx, v, sp, ra);
}

fn draw(fx: &mut Fx, v: &V, sp: u32, ra: u32) {
    if crate::fxlib2::project(fx, sp + at(v, 0x3D0), PROJ_OUT, sp + at(v, 0x2D0), sp + at(v, DESC), sp + at(v, 0x56C)) {
        let w = fx.mem.f(sp + at(v, 0x56C));
        let key = fixu(fx, w, ra);
        fx.submit(sp + at(v, DESC), OT, key as i32);
    }
}

fn clamp32(v: i32) -> f32 {
    ee::from_i32(Fx::clamp_i(v, 0, 0x20))
}

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
    for k in 0..v.tex {
        let r = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x10 + 4 * k, r);
    }
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, v.node, 1);
    fx.mem.set32(D + v.slot, r);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, dz(v) + 0x40);
    fx.mem.write_u8(dz(v) + 0x2FA0, 0);
    fx.mem.write_u32(dz(v) + 0x2FA4, 0);
    for k in 0..0x30u32 {
        fx.random_table_vec(dz(v) + 0x5AB0 + 0x10 * k, v.base_len);
    }
    for k in 0..0x12u32 {
        fx.mem.write_u8(dz(v) + 0x5DB0 + 0x340 * k, 0);
    }
    for k in 0..0xCu32 {
        fx.mem.write_u8(dz(v) + 0x9830 + 0x50 * k, 0);
    }
    for k in 0..0xCu32 {
        fx.mem.write_u8(dz(v) + 0x9BF0 + 0x60 * k, 0);
    }
    if v.tail == Tail::Water {
        water_init(fx);
    } else if v.tail == Tail::Blades {
        fx.mem.write_u32(dz(v) + 0xA074, 0);
        for k in 0..8u32 {
            fx.mem.write_u8(dz(v) + 0xA080 + blade_stride(v.bl) * k, 0);
        }
    } else {
        for k in 0..0x20u32 {
            fx.mem.write_u8(dz(v) + 0xA070 + 0x120 * k, 0);
        }
    }
    0
}

/// (float)((double)(0.2 + 0.8 i / 20) x (0.8 + (double)(0.2 sin a))) (the
/// soft-double chain 0x1A2C30 / 0x1A3118 / 0x1A31D8; `k` the extra
/// (double) factor of the spirals).
fn radius(fx: &mut Fx, v: &V, i: i32, age: i32, k: Option<f32>) -> f32 {
    let (_, r) = fx.div(i, 12);
    let a = ((r << 16) / 12).wrapping_sub(0x8AE * age);
    let s = fx.tsin(a & 0xFFFF);
    let d1 = 0.8f64 + f64::from(ee::mul(f32::from_bits(0x3E4C_CCCD), s));
    let f2 = ee::mul(v.rad_mul, ee::from_i32(i));
    let f12 = ee::add(v.rad_add, ee::div(f2, 20.0));
    let p = f64::from(f12) * d1;
    match k {
        Some(k) => (f64::from(k) * p) as f32,
        None => p as f32,
    }
}

fn step(fx: &mut Fx, sp: u32, v: &V) -> i32 {
    fx.camera_turn_roll(sp + at(v, 0x350));
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + v.slot));
    fx.node_position(h, slot, D + 0x30);
    if let Some(dy) = v.node_dy {
        let y = fx.mem.f(D + 0x34);
        fx.mem.setf(D + 0x34, ee::add(y, dy));
    }
    fx.vu_sub4(sp + at(v, 0x450), dz(v) + 0x40, D + 0x30);
    fx.point_along(dz(v) + 0x50, sp + at(v, 0x450));
    fx.rotate_axis(dz(v) + 0x50, 0x4000, 0);
    let x2 = Fx::powf2(fx.mem.f(sp + at(v, 0x450)));
    let y2 = Fx::powf2(fx.mem.f(sp + at(v, 0x454)));
    let f20 = ee::add(x2, y2);
    let z2 = Fx::powf2(fx.mem.f(sp + at(v, 0x458)));
    let d = fx.sqrtf(ee::add(z2, f20));
    fx.mem.setf(dz(v) + 0x90, d);
    if v.self_pt {
        let h = fx.mem.i32(D + 4);
        fx.unit_point(1, h, D + 0x40);
    }
    if fx.mem.i32(D) == v.tube_on {
        fx.mem.write_u8(dz(v) + 0x2FA0, 1);
        fx.mem.write_u32(dz(v) + 0x2FA4, 0);
    }
    let age = fx.mem.i32(dz(v) + 0x2FA4);
    let f20 = ee::div(clamp32(v.tube_life - age), 32.0);
    if fx.mem.read_u8(dz(v) + 0x2FA0) == 1 {
        tube(fx, sp, f20, v);
    }
    let n = fx.mem.i32(D);
    if (v.spirals.0..v.spirals.1).contains(&n) {
        spiral_spawn(fx, sp, n, v);
    }
    spirals(fx, sp, v);
    let n = fx.mem.i32(D);
    if (v.bursts.0..v.bursts.1).contains(&n) {
        let (_, r) = fx.div(n, 0x12);
        let g = dz(v) + 0x340 * r as u32;
        fx.mem.write_u8(g + 0x5DB0, 1);
        fx.mem.write_u32(g + 0x5DB4, 0);
        for j in 0..0x30u32 {
            let b = fx.rand_from(v.rs0, ra(v, 0x01E0_1860)) >> 23;
            fx.mem.set8(g + 0x5DB8 + j, b % 8);
            fx.random_table_vec(sp + at(v, 0x450), v.burst_vec);
            fx.vu_add4(g + 0x5DF0 + 0x10 * j, sp + at(v, 0x450), dz(v) + 0x5AB0 + 0x10 * j);
        }
    }
    bursts(fx, sp, v);
    let n = fx.mem.i32(D);
    if (v.bursts.0..v.bursts.1).contains(&n) {
        let (_, r) = fx.div(n, 0xC);
        let g = dz(v) + 0x50 * r as u32;
        fx.mem.write_u8(g + 0x9830, 1);
        fx.mem.write_u32(g + 0x9834, 0);
        for j in 0..4u32 {
            let b = fx.rand_from(v.rs0, ra(v, 0x01E0_1C9C)) >> 23;
            fx.mem.set8(g + 0x9838 + j, b % 8);
            fx.random_table_vec(g + 0x9840 + 0x10 * j, v.fleck_vec);
        }
    }
    flecks(fx, sp, v);
    let n = fx.mem.i32(D);
    if (v.puffs.0..v.puffs.1).contains(&n) {
        let (_, r) = fx.div(n, 0xC);
        let g = dz(v) + 0x60 * r as u32;
        fx.mem.write_u8(g + 0x9BF0, 1);
        fx.mem.write_u32(g + 0x9BF4, 0);
        words(fx, g + 0x9C00, D + 0x30, 4);
        for j in 0..4u32 {
            let b = fx.rand_from(v.rs0, ra(v, 0x01E0_211C)) >> 23;
            fx.mem.set8(g + 0x9BF8 + j, b % 8);
            fx.random_table_vec(g + 0x9C10 + 0x10 * j, 1.0);
        }
    }
    puffs(fx, sp, v);
    match v.tail {
        Tail::Blades => {
            blades(fx, v, sp);
            return finish(fx, sp, v);
        }
        Tail::Water => {
            water(fx, sp);
            return finish(fx, sp, v);
        }
        Tail::Gathers => {}
    }
    let n = fx.mem.i32(D);
    if (0x15..0x25).contains(&n) {
        let g = dz(v) + 0x120 * (n % 32) as u32;
        fx.mem.write_u8(g + 0xA070, 1);
        fx.mem.write_u32(g + 0xA074, 0);
        for j in 0..0x10u32 {
            let b = fx.rand_from(OT, 0x01E0_25C4) >> 23;
            fx.mem.set8(g + 0xA078 + j, b % 8);
            let q = g + 0xA090 + 0x10 * j;
            fx.random_table_vec(q, 1.0);
            let side = ee::from_i32(fx.mem.i32(D + 0xC));
            let z = fx.mem.f(q + 8);
            fx.mem.setf(q + 8, ee::add(z, ee::mul(-50.0, side)));
            fx.vu_add4(q, q, D + 0x30);
        }
    }
    gathers(fx, v, sp);
    finish(fx, sp, v)
}

fn finish(fx: &mut Fx, sp: u32, v: &V) -> i32 {
    let (one, six, two, eight) = v.rets;
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == one {
        ret = 1;
    } else {
        if (six..two).contains(&n) {
            ret = 6;
        }
        if n == two {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= eight {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xC0, ret as u32);
    ret
}

/// The ring of 17 points (D+0xA0) at radius `r`.
fn ring(fx: &mut Fx, v: &V, r: f32) {
    for k in 0..0x11u32 {
        let a = (k << 12) as i32;
        let s = fx.tsin(a);
        fx.mem.setf(dz(v) + 0xA0 + 0x10 * k, ee::mul(r, s));
        fx.mem.write_u32(dz(v) + 0xA4 + 0x10 * k, 0);
        let c = fx.tcos(a);
        fx.mem.setf(dz(v) + 0xA8 + 0x10 * k, ee::mul(r, c));
        fx.mem.write_u32(dz(v) + 0xAC + 0x10 * k, 0x3F80_0000);
    }
}

/// The strip's four colours for row `j` (alpha `a` inside, 0 at the ends).
fn row_colours(fx: &mut Fx, v: &V, sp: u32, j: u32, a: u8, c: [u8; 3]) {
    let (top, bottom) = match j {
        0 => (0, a),
        0x13 => (a, 0),
        _ => (a, a),
    };
    rgba(fx, sp + at(v, 0x4E8), c, top);
    words(fx, sp + at(v, 0x500), sp + at(v, 0x4E8), 2);
    rgba(fx, sp + at(v, 0x518), c, bottom);
    words(fx, sp + at(v, 0x530), sp + at(v, 0x518), 2);
}

/// The tube [F 0x1E0032C..0x1E010B8].
fn tube(fx: &mut Fx, sp: u32, f20: f32, v: &V) {
    let age = fx.mem.i32(dz(v) + 0x2FA4);
    // 21 centre points (all 0 x sin / cos).
    for s2 in 0..0x15u32 {
        let a = (1820 * age).wrapping_add(((s2 << 16) / 20) as i32) & 0xFFFF;
        let s = fx.tsin(a);
        let p = dz(v) + 0x2E50 + 0x10 * s2;
        fx.mem.setf(p, ee::mul(0.0, s));
        fx.mem.write_u32(p + 4, 0);
        let c = fx.tcos(a);
        fx.mem.setf(p + 8, ee::mul(0.0, c));
        fx.mem.write_u32(p + 0xC, 0x3F80_0000);
    }
    let age = fx.mem.i32(dz(v) + 0x2FA4);
    let ac = ee::from_i32(v.agec);
    let f3 = ee::from_i32(Fx::clamp_i(age, 0, v.agec));
    let f21 = ee::div(ee::mul(fx.mem.f(dz(v) + 0x90), f3), ac);
    let f22 = ee::add(0.5, ee::div(ee::mul(0.5, f3), ac));
    let d = sp + at(v, STRIP);
    strip_head(fx, v, sp);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 0);
    let f23 = ee::mul(128.0, f20);
    let a_in = fixu(fx, ee::mul(v.tube_alpha, f20), ra(v, 0x01E0_04C0)) as u8;
    fx.mem.write_u8(sp + 0xD0, a_in);
    ring(fx, v, ee::mul(v.r_tube, f22));
    // The textured grid.
    aimed(fx, v, sp);
    grid(fx, f21, None, v);
    let age = fx.mem.i32(dz(v) + 0x2FA4);
    let (_, r) = fx.div(age, 10);
    let f20 = ee::sub(0.0, ee::div(ee::from_i32(r), 10.0));
    let mut regs = [0u32; 4];
    for j in 0..0x14u32 {
        row_colours(fx, v, sp, j, a_in, v.strip);
        let f24 = ee::add(ee::div(ee::mul(5.0, ee::from_i32(j as i32)), 20.0), f20);
        let f25 = ee::add(ee::div(ee::mul(5.0, ee::from_i32(j as i32 + 1)), 20.0), f20);
        let (s4, s5) = (dz(v) + 0x110 * j, dz(v) + 0x110 * (j + 1));
        for s6 in 0..0x10u32 {
            let s0 = ee::div(ee::from_i32(s6 as i32), 16.0);
            let s1 = ee::div(ee::from_i32(s6 as i32 + 1), 16.0);
            for (o, x) in [(0x4F0u32, s0), (0x4F4, f24), (0x508, s1), (0x50C, f24), (0x520, s0), (0x524, f25), (0x538, s1), (0x53C, f25)] {
                fx.mem.setf(sp + at(v, o), x);
            }
            let vs = [s4 + 0x10 * s6 + 0x1B0, s4 + 0x10 * (s6 + 1) + 0x1B0, s5 + 0x10 * s6 + 0x1B0, s5 + 0x10 * (s6 + 1) + 0x1B0];
            if fx.project_n(&vs, sp + at(v, 0x2D0), d, sp + at(v, 0x56C), ra(v, 0x01E0_0934)) == 1 {
                let w = fx.mem.f(sp + at(v, 0x56C));
                let key = fixu(fx, w, ra(v, 0x01E0_0948));
                fx.submit(d, OT, key as i32);
                // 0x206F30 leaves a0..a3 = (desc, ot, key, desc) [run].
                regs = [d, OT, key, d];
            } else {
                // 0x100BA0 rejected: 0x205210's arguments [H].
                regs = [fx.sp - 0x40, PROJ_OUT, sp + at(v, 0x2D0), d];
            }
        }
    }
    // The lens grid.
    strip_head(fx, v, sp);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xEC, layer);
    let regs = crate::lens::frame_copy(fx, regs, 1, sp + at(v, 0x550), sp + at(v, 0x558));
    fx.texflush(regs);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, LENS_CLAMP);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let a_in = fixu(fx, f23, ra(v, 0x01E0_0A58)) as u8;
    fx.mem.write_u8(sp + at(v, 0x2C0), a_in);
    ring(fx, v, ee::mul(v.r_lens, f22));
    aimed(fx, v, sp);
    grid(fx, f21, Some(sp), v);
    for j in 0..0x14u32 {
        row_colours(fx, v, sp, j, a_in, v.strip_lens);
        let (s7, fp) = (dz(v) + 0x110 * j, dz(v) + 0x110 * (j + 1));
        for s6 in 0..0x10u32 {
            let (s3, s5, s2, s4) = (s7 + 0x10 * s6, s7 + 0x10 * (s6 + 1), fp + 0x10 * s6, fp + 0x10 * (s6 + 1));
            if fx.project_n(&[s3 + 0x1800, s5 + 0x1800, s2 + 0x1800, s4 + 0x1800], sp + at(v, 0x2D0), d, sp + at(v, 0x56C), ra(v, 0x01E0_0F70)) != 1 {
                continue;
            }
            for k in 0..4u32 {
                fx.uv_from_xy(sp + at(v, 0x4F0) + 0x18 * k, sp + at(v, 0x4F8) + 0x18 * k, 0, 0);
            }
            if fx.project_n(&[s3 + 0x1B0, s5 + 0x1B0, s2 + 0x1B0, s4 + 0x1B0], sp + at(v, 0x2D0), d, sp + at(v, 0x56C), ra(v, 0x01E0_0FEC)) != 1 {
                continue;
            }
            if front(fx, sp + at(v, 0x4F8), sp + at(v, 0x510), sp + at(v, 0x528)) {
                fx.submit(d, 0, 0);
            }
        }
    }
    let layer = fx.mem.read_u32(sp + 0xEC);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(dz(v) + 0x2FA4) + 1;
    fx.mem.set32(dz(v) + 0x2FA4, age);
    if age >= v.tube_end {
        fx.mem.write_u8(dz(v) + 0x2FA0, 0);
    }
}

/// The 21 x 17 grid D+0x1B0 (rows 0x110 apart) along the aim, radius
/// `radius`, row y f21 i / 20; with `lens` (the frame) also the wobbling
/// copy D+0x1800 turned about y.
fn grid(fx: &mut Fx, f21: f32, lens: Option<u32>, v: &V) {
    let age = fx.mem.i32(dz(v) + 0x2FA4);
    for s6 in 0..0x15u32 {
        let i = s6 as i32;
        let f20 = ee::div(ee::mul(f21, ee::from_i32(i)), 20.0);
        let f24 = radius(fx, v, i, age, None);
        let mut s5 = 0;
        if let Some(sp) = lens {
            fx.identity(sp + at(v, 0x410));
            let k = 0;
            let (_, r) = fx.div(age + k, 0x2E);
            let a0 = (r << 16) / 46;
            s5 = (i % 8) << 13;
            let s = fx.tsin(s5.wrapping_add(a0) & 0xFFFF);
            let a = fixs(fx, ee::mul(v.wobble, s), ra(v, 0x01E0_0C88));
            fx.rotate_axis(sp + at(v, 0x410), a, 1);
        }
        let row = dz(v) + 0x110 * s6;
        let centre = dz(v) + 0x2E50 + 0x10 * s6;
        for s2 in 0..0x11u32 {
            let p = row + 0x10 * s2 + 0x1B0;
            fx.vu_scale4(p, dz(v) + 0xA0 + 0x10 * s2, f24);
            fx.mem.setf(p + 4, f20);
            fx.vu_add4(p, p, centre);
            if let Some(sp) = lens {
                let q = row + 0x10 * s2 + 0x1800;
                fx.vu_apply_to(q, sp + at(v, 0x410), p);
                let (_, r) = fx.div(age, 0x30);
                let b = (r << 16) / 48;
                let a = ((s2 as i32 % 16) << 12).wrapping_add(s5.wrapping_add(b));
                let s = fx.tsin(a & 0xFFFF);
                let f1 = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::div(f21, 20.0));
                fx.mem.setf(q + 4, ee::add(f20, ee::mul(f1, s)));
                if v.lens_centre {
                    fx.vu_add4(q, q, centre);
                }
            }
        }
    }
}

/// 0x101430(a, b, c) [F] (b212's): 1 when the triangle of the XYZ2 x / y
/// halfwords at `a` / `b` / `c` faces the camera (normal z < 0).
fn front(fx: &mut Fx, a: u32, b: u32, c: u32) -> bool {
    let sp = fx.enter(0x60);
    for (k, p) in [a, b, c].into_iter().enumerate() {
        let q = sp + 0x30 + 0x10 * k as u32;
        let x = ee::from_i32(i32::from(fx.mem.read_u16(p)));
        fx.mem.setf(q, x);
        let y = ee::from_i32(i32::from(fx.mem.read_u16(p + 2)));
        fx.mem.setf(q + 4, y);
        fx.mem.write_u32(q + 8, 0);
        fx.mem.write_u32(q + 0xC, 0x3F80_0000);
    }
    let n = sp + 0x20;
    let pts = sp + 0x30;
    let fp = fx.enter(0x80);
    for k in 0..4 {
        fx.mem.write_u32(n + 4 * k, 0);
    }
    for i in 0..3u32 {
        let p0 = pts + 0x10 * (i % 3);
        let p1 = pts + 0x10 * ((i + 1) % 3);
        let p2 = pts + 0x10 * ((i + 2) % 3);
        let (e1, e2, x) = (fp + 0x50, fp + 0x60, fp + 0x70);
        words(fx, e1, 0x0029_3070, 4);
        for k in 0..3u32 {
            let v = ee::sub(fx.mem.f(p1 + 4 * k), fx.mem.f(p0 + 4 * k));
            fx.mem.setf(e1 + 4 * k, v);
        }
        words(fx, e2, 0x0029_3080, 4);
        for k in 0..3u32 {
            let v = ee::sub(fx.mem.f(p2 + 4 * k), fx.mem.f(p1 + 4 * k));
            fx.mem.setf(e2 + 4 * k, v);
        }
        crate::blib::quadtrail::cross3(fx, x, e1, e2);
        for k in 0..3u32 {
            let v = ee::add(fx.mem.f(n + 4 * k), fx.mem.f(x + 4 * k));
            fx.mem.setf(n + 4 * k, v);
        }
    }
    for k in 0..3u32 {
        let v = ee::div(fx.mem.f(n + 4 * k), 3.0);
        fx.mem.setf(n + 4 * k, v);
    }
    crate::blib::trail::normalize3(fx, n);
    fx.leave(0x80);
    let r = crate::exe::ee_lt(fx.mem.f(n + 8), 0.0);
    fx.leave(0x60);
    r
}

/// The spirals' spawn [F 0x1E010C8..0x1E01418].
fn spiral_spawn(fx: &mut Fx, sp: u32, n: i32, v: &V) {
    let off = 0x560 * (n % 8) as u32;
    let g = dz(v) + off;
    fx.mem.write_u8(g + 0x2FB0, 1);
    fx.mem.write_u32(g + 0x2FB4, 0);
    fx.vu_sub4(sp + at(v, 0x450), dz(v) + 0x40, D + 0x30);
    let age = fx.mem.i32(dz(v) + 0x2FA4);
    let ac = ee::from_i32(v.agec);
    let f = ee::div(ee::from_i32(Fx::clamp_i(age, 0, v.agec)), ac);
    fx.vu_scale4(sp + at(v, 0x450), sp + at(v, 0x450), f);
    let age = fx.mem.i32(dz(v) + 0x2FA4);
    let f1 = ee::from_i32(Fx::clamp_i(age, 0, v.agec));
    let f21 = ee::mul(v.r_tube, ee::add(0.5, ee::div(ee::mul(0.5, f1), ac)));
    for s7 in 0..0x14u32 {
        let age = fx.mem.i32(dz(v) + 0x2FA4);
        let f20 = radius(fx, v, s7 as i32, age, Some(f21));
        for s5 in 0..4u32 {
            let b = fx.rand_from(v.rs0, ra(v, 0x01E0_12C0)) >> 23;
            fx.mem.set8(g + 0x2FB8 + 4 * s7 + s5, b % 8);
            let r = fx.rand_from(v.rs0, ra(v, 0x01E0_12E0)) >> 23;
            let f2 = ee::div(ee::mul(f32::from_bits(0x3D4C_CCCD), ee::from_i32(r % 256)), 256.0);
            let f0 = ee::div(ee::from_i32(s7 as i32), 20.0);
            fx.vu_scale4(sp + at(v, 0x460), sp + at(v, 0x450), ee::add(f0, f2));
            fx.vu_add4(sp + at(v, 0x460), sp + at(v, 0x460), D + 0x30);
            let r = fx.rand_from(v.rs0, ra(v, 0x01E0_1368)) >> 23;
            let a = (((0x10000 * s5) >> 2) as i32).wrapping_add((r % 256) << 8) & 0xFFFF;
            let s = fx.tsin(a);
            fx.mem.setf(sp + at(v, 0x470), ee::mul(f20, s));
            fx.mem.write_u32(sp + at(v, 0x474), 0);
            let c = fx.tcos(a);
            fx.mem.setf(sp + at(v, 0x478), ee::mul(f20, c));
            fx.mem.write_u32(sp + at(v, 0x47C), 0x3F80_0000);
            let q = g + 0x3010 + 0x40 * s7 + 0x10 * s5;
            fx.vu_apply_to(q, dz(v) + 0x50, sp + at(v, 0x470));
            fx.vu_add4(q, q, sp + at(v, 0x460));
        }
    }
}

/// The spirals [F 0x1E01418..0x1E01800].
fn spirals(fx: &mut Fx, sp: u32, v: &V) {
    head(fx, v, sp, 0x18, 1, false);
    camera(fx, v, sp);
    let n = fx.mem.i32(D);
    let a = clamp32(n - v.sp_lo);
    let n = fx.mem.i32(D);
    let b = clamp32(v.end - n);
    let f20 = ee::div(ee::mul(a, b), 1024.0);
    for k in 0..8u32 {
        let g = dz(v) + 0x560 * k;
        if fx.mem.read_u8(g + 0x2FB0) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 0x2FB4);
        let s = fx.tsin((age << 15) >> 3);
        let al = fixu(fx, ee::mul(f20, ee::mul(v.sp_amp, s)), ra(v, 0x01E0_154C));
        rgba(fx, sp + at(v, 0x490), v.sp_rgb, al as u8);
        for j in 0..0x14u32 {
            let age = fx.mem.i32(g + 0x2FB4);
            let tf = fx.tan_falloff_p(age, 8, 1);
            let f3 = ee::mul(8.0, ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), tf)));
            let s = ee::mul(ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(j as i32)), 20.0)), f3);
            corners(fx, v, sp, s, ee::neg(s));
            for s6 in 0..4u32 {
                let b = u32::from(fx.mem.read_u8(g + 0x2FB8 + 4 * j + s6));
                cell_uvs(fx, v, sp, b);
                words(fx, sp + at(v, 0x450), g + 0x3010 + 0x40 * j + 0x10 * s6, 4);
                sprite(fx, v, sp, ra(v, 0x01E0_1750));
            }
        }
        let age = fx.mem.i32(g + 0x2FB4) + 1;
        fx.mem.set32(g + 0x2FB4, age);
        if age >= 9 {
            fx.mem.write_u8(g + 0x2FB0, 0);
        }
    }
}

/// The bursts at the target [F 0x1E018C0..0x1E01C38].
fn bursts(fx: &mut Fx, sp: u32, v: &V) {
    head(fx, v, sp, 0x18, 1, false);
    camera(fx, v, sp);
    let n = fx.mem.i32(D);
    let f20 = ee::div(clamp32(v.end - n), 32.0);
    for k in 0..0x12u32 {
        let g = dz(v) + 0x340 * k;
        if fx.mem.read_u8(g + 0x5DB0) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(g + 0x5DB4));
        let r = Fx::ramp_above(age, 18.0, 12.0);
        let al = fixu(fx, ee::mul(f20, ee::mul(v.burst_amp, r)), ra(v, 0x01E0_19CC));
        rgba(fx, sp + at(v, 0x490), v.burst_rgb, al as u8);
        let age = fx.mem.i32(g + 0x5DB4);
        let tf = fx.tan_falloff_p(age, 0x12, 1);
        let s = ee::mul(v.burst_size, ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), tf)));
        corners(fx, v, sp, s, ee::neg(s));
        let age = ee::from_i32(fx.mem.i32(g + 0x5DB4));
        let f21 = ee::add(f32::from_bits(0x3D4C_CCCD), ee::div(age, 18.0));
        for j in 0..0x30u32 {
            let b = u32::from(fx.mem.read_u8(g + 0x5DB8 + j));
            cell_uvs(fx, v, sp, b);
            fx.vu_scale4(sp + at(v, 0x450), g + 0x5DF0 + 0x10 * j, f21);
            fx.vu_add4(sp + at(v, 0x450), sp + at(v, 0x450), dz(v) + 0x40);
            sprite(fx, v, sp, ra(v, 0x01E0_1BC0));
        }
        let age = fx.mem.i32(g + 0x5DB4) + 1;
        fx.mem.set32(g + 0x5DB4, age);
        if age >= 0x13 {
            fx.mem.write_u8(g + 0x5DB0, 0);
        }
    }
}

/// The flecks at the target [F 0x1E01CE8..0x1E02090].
fn flecks(fx: &mut Fx, sp: u32, v: &V) {
    head(fx, v, sp, 0x1C, 1, false);
    camera(fx, v, sp);
    let n = fx.mem.i32(D);
    let f20 = ee::div(clamp32(v.end - n), 32.0);
    for k in 0..0xCu32 {
        let g = dz(v) + 0x50 * k;
        if fx.mem.read_u8(g + 0x9830) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 0x9834);
        let c = if v.fleck_sin { fx.tsin((age << 15) / 12) } else { fx.tcos((age << 14) / 12) };
        let al = fixu(fx, ee::mul(f20, ee::mul(v.fleck_amp, c)), ra(v, 0x01E0_1E24));
        rgba(fx, sp + at(v, 0x490), v.fleck_rgb, al as u8);
        corners(fx, v, sp, v.fleck_size, ee::neg(v.fleck_size));
        let age = ee::from_i32(fx.mem.i32(g + 0x9834));
        let f22 = ee::add(f32::from_bits(0x3E99_999A), ee::div(ee::mul(f32::from_bits(0x3F33_3333), age), 12.0));
        for j in 0..4u32 {
            let b = u32::from(fx.mem.read_u8(g + 0x9838 + j));
            cell_uvs(fx, v, sp, b);
            fx.vu_scale4(sp + at(v, 0x450), g + 0x9840 + 0x10 * j, f22);
            fx.vu_add4(sp + at(v, 0x450), sp + at(v, 0x450), dz(v) + 0x40);
            sprite(fx, v, sp, ra(v, 0x01E0_1FF4));
        }
        let age = fx.mem.i32(g + 0x9834) + 1;
        fx.mem.set32(g + 0x9834, age);
        if age >= 0xD {
            fx.mem.write_u8(g + 0x9830, 0);
        }
    }
}

/// The puffs at the node [F 0x1E02168..0x1E02558].
fn puffs(fx: &mut Fx, sp: u32, v: &V) {
    head(fx, v, sp, 0x18, 1, false);
    camera(fx, v, sp);
    let n = fx.mem.i32(D);
    let f20 = ee::div(clamp32(v.end - n), 32.0);
    for k in 0..0xCu32 {
        let g = dz(v) + 0x60 * k;
        if fx.mem.read_u8(g + 0x9BF0) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 0x9BF4);
        let c = fx.tcos((age << 14) / 12);
        let al = fixu(fx, ee::mul(f20, ee::mul(v.puff_amp, c)), ra(v, 0x01E0_2298));
        rgba(fx, sp + at(v, 0x490), v.puff_rgb, al as u8);
        let age = ee::from_i32(fx.mem.i32(g + 0x9BF4));
        let s = ee::mul(10.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 12.0)));
        corners(fx, v, sp, s, ee::neg(s));
        let age = ee::from_i32(fx.mem.i32(g + 0x9BF4));
        let f21 = ee::add(f32::from_bits(0x3DCC_CCCD), ee::div(ee::mul(f32::from_bits(0x3F66_6666), age), 12.0));
        for j in 0..4u32 {
            let b = u32::from(fx.mem.read_u8(g + 0x9BF8 + j));
            cell_uvs(fx, v, sp, b);
            fx.vu_scale4(sp + at(v, 0x450), g + 0x9C10 + 0x10 * j, f21);
            fx.vu_add4(sp + at(v, 0x450), sp + at(v, 0x450), g + 0x9C00);
            sprite(fx, v, sp, ra(v, 0x01E0_24BC));
        }
        let age = fx.mem.i32(g + 0x9BF4) + 1;
        fx.mem.set32(g + 0x9BF4, age);
        if age >= 0xD {
            fx.mem.write_u8(g + 0x9BF0, 0);
        }
    }
}

/// The gathers in towards the node [F 0x1E02658..0x1E02A18].
fn gathers(fx: &mut Fx, v: &V, sp: u32) {
    head(fx, v, sp, 0x1C, 0, true);
    camera(fx, v, sp);
    for k in 0..0x20u32 {
        let g = dz(v) + 0x120 * k;
        if fx.mem.read_u8(g + 0xA070) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 0xA074);
        let s = ee::mul(10.0, ee::add(f32::from_bits(0x3ECC_CCCD), ee::div(ee::mul(f32::from_bits(0x3F19_999A), ee::from_i32(0x20 - age)), 32.0)));
        corners(fx, v, sp, ee::neg(s), s);
        let age = ee::from_i32(fx.mem.i32(g + 0xA074));
        let f20 = ee::mul(48.0, Fx::ratio_below(age, 16.0));
        let age = ee::from_i32(fx.mem.i32(g + 0xA074));
        let f0 = Fx::ramp_above(age, 32.0, 28.0);
        let al = fixu(fx, ee::mul(f20, f0), 0x01E0_2810);
        rgba(fx, sp + at(v, 0x490), [0xFF, 0xFF, 0xFF], al as u8);
        let age = fx.mem.i32(g + 0xA074);
        let f20 = fx.tan_falloff_p(0x20 - age, 0x20, 1);
        for j in 0..0x10u32 {
            fx.vu_sub4(sp + at(v, 0x450), g + 0xA090 + 0x10 * j, D + 0x30);
            fx.vu_scale4(sp + at(v, 0x450), sp + at(v, 0x450), f20);
            fx.vu_add4(sp + at(v, 0x450), sp + at(v, 0x450), D + 0x30);
            fx.vu_add4(sp + at(v, 0x3D0), sp + at(v, 0x390), sp + at(v, 0x450));
            fx.vu_add4(sp + at(v, 0x3E0), sp + at(v, 0x3A0), sp + at(v, 0x450));
            let b = u32::from(fx.mem.read_u8(g + 0xA078 + j));
            cell_uvs(fx, v, sp, b);
            draw(fx, v, sp, 0x01E0_297C);
        }
        let age = fx.mem.i32(g + 0xA074) + 1;
        fx.mem.set32(g + 0xA074, age);
        if age >= 0x21 {
            fx.mem.write_u8(g + 0xA070, 0);
        }
    }
}

/// b429's tail [F 0x1E02510..0x1E02D18]: from update 0 for 0x120 updates a
/// glow at the node (D+0xA070, 30 wide, cell 4, colour (62, D8, 37), alpha
/// 128 ratio_below(age, 16) ramp_above(age, 287, 271) (0.8 + 0.2 (age %
/// 2)), far Z, key 0); on 1..0x11E one of eight blade bursts (D+0xA080 +
/// 0x90 (n % 8), 8 updates): 8 cells + 8 vectors 0.5, drawn as strips (1
/// x 2 turned along the vector and by 0x2000 (k + j) about z, stretched
/// -2.5 tan falloff along it, colour (FF, B4, 60), alpha (8 - age) 16
/// clamp(n, 0, 16) clamp(0x11F - n, 0, 16) / 256, keyed by mean w).
/// The tail of b429 / b624 ([`blades`], b624 also [`motes`]).
struct Bl {
    glow_on: i32,
    glow_life: i32,
    glow_ramp: (f32, f32),
    glow_amp: f32,
    glow_rgb: [u8; 3],
    glow_size: f32,
    /// The blades' spawn window (exclusive) and fade.
    win: (i32, i32),
    vec: f32,
    rgb: [u8; 3],
    stretch: f32,
    /// Far Z, key 0 (b624) in place of the mean-w key.
    far: bool,
    motes: bool,
    /// The glow's fixu, the blades' rand / fixu / key fixu, then the
    /// motes' rand / fixu / fixs / sprite fixu.
    ra: [u32; 8],
    /// Sprites a blade record (0x10 + 0x10 n bytes; b616 6, else 8), the
    /// fade's (8 - age) factor (b616 255, else 128), the quads' half
    /// height (b616 2.0), the glow / blade centre's y less `dy` (b616 -0.5).
    n: u32,
    amp: i32,
    half: f32,
    dy: Option<f32>,
}

const BL429: Bl = Bl {
    glow_on: 0,
    glow_life: 0x120,
    glow_ramp: (287.0, 271.0),
    glow_amp: 128.0,
    glow_rgb: [0x62, 0xD8, 0x37],
    glow_size: 15.0,
    win: (0, 0x11F),
    vec: 0.5,
    rgb: [0xFF, 0xB4, 0x60],
    stretch: -2.5,
    far: false,
    motes: false,
    ra: [0x01E0_268C, 0x01E0_28D8, 0x01E0_2A28, 0x01E0_2C7C, 0, 0, 0, 0],
    n: 8,
    amp: 128,
    half: 1.0,
    dy: None,
};

const BL624: Bl = Bl {
    glow_on: 0x5A,
    glow_life: 0x24,
    glow_ramp: (35.0, 19.0),
    glow_amp: 255.0,
    glow_rgb: [0x9B, 0x55, 0xFF],
    glow_size: 40.0,
    win: (0x5A, 0x7D),
    vec: 1.0,
    rgb: [0x80, 0xE6, 0xFF],
    stretch: -2.0,
    far: true,
    motes: true,
    ra: [0x01E0_26C0, 0x01E0_2910, 0x01E0_2A68, 0, 0x01E0_2E9C, 0x01E0_2FE0, 0x01E0_3034, 0x01E0_3270],
    n: 8,
    amp: 128,
    half: 1.0,
    dy: None,
};

/// b616's glow / blades: the glow on 0x12 for 0x35 (ramp 52 / 36, 255,
/// (37, 55, FF), 40 wide), six-sprite blades on 0x13..0x45 (1.5 vectors,
/// (80, B4, FF) at 255 (8 - age) / 8, 2.0 high, far Z), both 0.5 above
/// the node.
const BL616: Bl = Bl {
    glow_on: 0x12,
    glow_life: 0x35,
    glow_ramp: (52.0, 36.0),
    glow_amp: 255.0,
    glow_rgb: [0x37, 0x55, 0xFF],
    glow_size: 40.0,
    win: (0x12, 0x46),
    vec: 1.5,
    rgb: [0x80, 0xB4, 0xFF],
    stretch: -2.5,
    far: true,
    motes: false,
    ra: [0x01E0_26B8, 0x01E0_2920, 0x01E0_2A78, 0, 0, 0, 0, 0],
    n: 6,
    amp: 255,
    half: 2.0,
    dy: Some(-0.5),
};

/// A blade record's bytes.
fn blade_stride(bl: &Bl) -> u32 {
    0x10 + 0x10 * bl.n
}

/// D+0x30 into `to` (y less `bl.dy`).
fn centre(fx: &mut Fx, bl: &Bl, to: u32) {
    words(fx, to, D + 0x30, 4);
    if let Some(dy) = bl.dy {
        let y = fx.mem.f(to + 4);
        fx.mem.setf(to + 4, ee::sub(y, dy));
    }
}

fn blades(fx: &mut Fx, v: &V, sp: u32) {
    let bl = v.bl;
    if fx.mem.i32(D) == bl.glow_on {
        fx.mem.write_u8(dz(v) + 0xA070, 1);
        fx.mem.write_u32(dz(v) + 0xA074, 0);
    }
    if fx.mem.read_u8(dz(v) + 0xA070) == 1 {
        head(fx, v, sp, 0x10, 1, false);
        camera(fx, v, sp);
        let age = ee::from_i32(fx.mem.i32(dz(v) + 0xA074));
        let f20 = Fx::ratio_below(age, 16.0);
        let age = ee::from_i32(fx.mem.i32(dz(v) + 0xA074));
        let f0 = Fx::ramp_above(age, bl.glow_ramp.0, bl.glow_ramp.1);
        let f3 = ee::mul(f20, f0);
        let r = fx.mem.i32(dz(v) + 0xA074) % 2;
        let f0 = ee::add(f32::from_bits(0x3F4C_CCCD), ee::mul(f32::from_bits(0x3E4C_CCCD), ee::from_i32(r)));
        let al = fixu(fx, ee::mul(bl.glow_amp, ee::mul(f0, f3)), bl.ra[0]);
        rgba(fx, sp + at(v, 0x490), bl.glow_rgb, al as u8);
        corners(fx, v, sp, bl.glow_size, ee::neg(bl.glow_size));
        cell_uvs(fx, v, sp, 4);
        centre(fx, bl, sp + at(v, 0x450));
        fx.vu_add4(sp + at(v, 0x3D0), sp + at(v, 0x390), sp + at(v, 0x450));
        fx.vu_add4(sp + at(v, 0x3E0), sp + at(v, 0x3A0), sp + at(v, 0x450));
        if crate::fxlib2::project(fx, sp + at(v, 0x3D0), PROJ_OUT, sp + at(v, 0x2D0), sp + at(v, DESC), sp + at(v, 0x56C)) {
            crate::lens::far_z(fx, sp + at(v, 0x4A0), &[sp + at(v, 0x4B0)]);
            fx.submit(sp + at(v, DESC), OT, 0);
        }
        let age = fx.mem.i32(dz(v) + 0xA074) + 1;
        fx.mem.set32(dz(v) + 0xA074, age);
        if age >= bl.glow_life {
            fx.mem.write_u8(dz(v) + 0xA070, 0);
        }
    }
    let n = fx.mem.i32(D);
    if n > bl.win.0 && n < bl.win.1 {
        let g = dz(v) + blade_stride(bl) * (n % 8) as u32;
        fx.mem.write_u8(g + 0xA080, 1);
        fx.mem.write_u32(g + 0xA084, 0);
        for j in 0..bl.n {
            let b = fx.rand_from(v.rs0, bl.ra[1]) >> 23;
            fx.mem.set8(g + 0xA088 + j, b % 8);
            fx.random_table_vec(g + 0xA090 + 0x10 * j, bl.vec);
        }
    }
    let d = sp + at(v, STRIP);
    strip_head(fx, v, sp);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    let n = fx.mem.i32(D);
    let a = ee::from_i32(Fx::clamp_i(n - bl.win.0, 0, 0x10));
    let n = fx.mem.i32(D);
    let b = ee::from_i32(Fx::clamp_i(bl.win.1 - n, 0, 0x10));
    let f20 = ee::div(ee::mul(a, b), 256.0);
    for k in 0..8u32 {
        let g = dz(v) + blade_stride(bl) * k;
        if fx.mem.read_u8(g + 0xA080) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 0xA084);
        let x = (8 - age).wrapping_mul(bl.amp) >> 3;
        let al = fixu(fx, ee::mul(f20, ee::from_i32(x)), bl.ra[2]);
        rgba(fx, sp + at(v, 0x4E8), bl.rgb, al as u8);
        words(fx, sp + at(v, 0x500), sp + at(v, 0x4E8), 2);
        words(fx, sp + at(v, 0x518), sp + at(v, 0x4E8), 2);
        words(fx, sp + at(v, 0x530), sp + at(v, 0x4E8), 2);
        fx.camera_vp_to(sp + at(v, 0x2D0));
        let t = if bl.dy.is_some() {
            centre(fx, bl, sp + at(v, 0x450));
            std::array::from_fn(|c| fx.mem.f(sp + at(v, 0x450) + 4 * c as u32))
        } else {
            std::array::from_fn(|c| fx.mem.f(D + 0x30 + 4 * c as u32))
        };
        let m = fx.trs(None, None, Some(t));
        fx.write_m4(sp + at(v, 0x310), &m);
        mul_mem(fx, sp + at(v, 0x2D0), sp + at(v, 0x310));
        let age = fx.mem.i32(g + 0xA084);
        let f21 = ee::mul(bl.stretch, fx.tan_falloff_p(age, 8, 1));
        for j in 0..bl.n {
            let p = g + 0xA090 + 0x10 * j;
            fx.point_along(sp + at(v, 0x410), p);
            let a = (((k << 16) as i32) >> 3).wrapping_add(((j << 16) as i32) / bl.n as i32);
            fx.rotate_axis(sp + at(v, 0x410), a, 2);
            vec4(fx, sp + at(v, 0x390), [0.0, bl.half, 0.0, 1.0]);
            vec4(fx, sp + at(v, 0x3A0), [0.0, ee::neg(bl.half), 0.0, 1.0]);
            fx.vu_apply_to(sp + at(v, 0x390), sp + at(v, 0x410), sp + at(v, 0x390));
            fx.vu_apply_to(sp + at(v, 0x3A0), sp + at(v, 0x410), sp + at(v, 0x3A0));
            fx.vu_add4(sp + at(v, 0x3D0), sp + at(v, 0x390), p);
            fx.vu_add4(sp + at(v, 0x3E0), sp + at(v, 0x3A0), p);
            fx.vu_scale4(sp + at(v, 0x450), p, f21);
            fx.vu_add4(sp + at(v, 0x3F0), sp + at(v, 0x3D0), sp + at(v, 0x450));
            fx.vu_add4(sp + at(v, 0x400), sp + at(v, 0x3E0), sp + at(v, 0x450));
            let b = u32::from(fx.mem.read_u8(g + 0xA088 + j));
            let (c, r) = (ee::from_i32(32 * (b & 3) as i32), ee::from_i32(32 * (b >> 2) as i32));
            let u0 = ee::div(c, 128.0);
            let u1 = ee::div(ee::add(32.0, c), 128.0);
            let v0 = ee::div(r, 64.0);
            let v1 = ee::div(ee::add(32.0, r), 64.0);
            for (o, x) in [(0x4F0u32, u0), (0x4F4, v0), (0x508, u1), (0x50C, v0), (0x520, u0), (0x524, v1), (0x538, u1), (0x53C, v1)] {
                fx.mem.setf(sp + at(v, o), x);
            }
            let mm = fx.read_m4(sp + at(v, 0x2D0));
            if fx.project_desc(sp + at(v, 0x3D0), PROJ_OUT, Some(&mm), d, sp + at(v, 0x56C)) {
                if bl.far {
                    crate::lens::far_z(fx, sp + at(v, 0x4F8), &[sp + at(v, 0x510), sp + at(v, 0x528), sp + at(v, 0x540)]);
                    fx.submit(d, OT, 0);
                } else {
                    let w = fx.mem.f(sp + at(v, 0x56C));
                    let key = fixu(fx, w, bl.ra[3]);
                    fx.submit(d, OT, key as i32);
                }
            }
        }
        let age = fx.mem.i32(g + 0xA084) + 1;
        fx.mem.set32(g + 0xA084, age);
        if age >= 9 {
            fx.mem.write_u8(g + 0xA080, 0);
        }
    }
    if bl.motes {
        motes(fx, v, sp);
    }
}

/// b623's water sheet init [F 0x1E00214..0x1E00328]: a 25 x 25 grid of
/// points D+0xA070 (rows 0x190 apart, 600 / 24 spacing about the origin,
/// y 0) with a random phase each (D+0xC780, ((rand >> 23) % 256 << 16) >>
/// 8), the flag D+0xD144 off.
fn water_init(fx: &mut Fx) {
    for i in 0..0x19u32 {
        let f20 = ee::div(ee::mul(600.0, ee::from_i32(i as i32)), 24.0);
        for j in 0..0x19u32 {
            let z = ee::sub(ee::div(ee::mul(600.0, ee::from_i32(j as i32)), 24.0), 300.0);
            let p = D + 0xA070 + 0x190 * i + 0x10 * j;
            fx.mem.setf(p, ee::sub(f20, 300.0));
            fx.mem.write_u32(p + 4, 0);
            fx.mem.setf(p + 8, z);
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
            let r = fx.rand_from(D, 0x01E0_02C8) >> 23;
            fx.mem.set32(D + 0xC780 + 0x64 * i + 4 * j, ((r % 256) << 16) >> 8);
        }
    }
    fx.mem.write_u8(D + 0xD144, 0);
}

/// b623's water sheet [F 0x1E02658..0x1E02B38]: from update 0 for 0x21D
/// updates (D+0xD144 / age D+0xD148) the grid's heights 5 sin(phase +
/// 0x33333 p / 2 + 0x29A age) drawn twice as textured quads (texture 4,
/// prim 0x24, 9 registers, alpha preset 0, CLAMP 0) at y 20 - 10 p, colour
/// (62, 80, 80) with alpha 70 / 128 ratio_below(age, 160) ramp_above(age,
/// 540, 490), the 2 x 2 ST tiles scrolled 0.1 (sin, cos)(+-((age + 32 p)
/// % 128) << 9), keyed by mean w.
fn water(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0 {
        fx.mem.write_u8(D + 0xD144, 1);
        fx.mem.write_u32(D + 0xD148, 0);
    }
    if fx.mem.read_u8(D + 0xD144) != 1 {
        return;
    }
    let d = sp + 0x580;
    fx.mem.write_u16(d + 2, 0x24);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, 0x4_2424_2421);
    fx.mem.write_u32(d + 4, 0);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x20);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 0);
    let age = ee::from_i32(fx.mem.i32(D + 0xD148));
    let f20 = Fx::ratio_below(age, 160.0);
    let age = ee::from_i32(fx.mem.i32(D + 0xD148));
    let f1 = ee::mul(f20, Fx::ramp_above(age, 540.0, 490.0));
    let f20 = ee::mul(70.0, f1);
    let f21 = ee::mul(128.0, f1);
    fx.mem.write_u32(sp + 0x2D0, 0);
    fx.mem.write_u32(sp + 0x1C0, 0);
    fx.mem.write_u32(sp + 0x1D0, 0);
    for p in 0..2i32 {
        fx.mem.set32(sp + 0x2D0, p);
        fx.camera_vp_to(sp + 0x2E0);
        fx.identity(sp + 0x320);
        let f2 = ee::mul(5.0, ee::from_i32(p));
        vec4(fx, sp + 0x460, [0.0, ee::sub(20.0, ee::mul(2.0, f2)), 0.0, 1.0]);
        fx.translate_rows(sp + 0x320, sp + 0x320, sp + 0x460);
        mul_mem(fx, sp + 0x2E0, sp + 0x320);
        let al = if p == 0 { fixu(fx, f20, 0x01E0_27FC) } else { fixu(fx, f21, 0x01E0_2810) };
        rgba(fx, sp + 0x590, [0x62, 0x80, 0x80], al as u8);
        let s4 = fx.mem.i32(sp + 0x1C0) >> 1;
        for i in 0..0x19u32 {
            for j in 0..0x19u32 {
                let ph = fx.mem.i32(D + 0xC780 + 0x64 * i + 4 * j);
                let age = fx.mem.i32(D + 0xD148);
                let a = ph.wrapping_add(s4).wrapping_add(age.wrapping_mul(0x29A));
                let y = ee::mul(5.0, fx.tsin(a & 0xFFFF));
                fx.mem.setf(D + 0xA074 + 0x190 * i + 0x10 * j, y);
            }
        }
        let t = fx.mem.i32(D + 0xD148).wrapping_add(fx.mem.i32(sp + 0x1D0));
        let a0 = ((t % 128) << 16) >> 7;
        let sg = 2 * (p % 2) - 1;
        let s2 = sg.wrapping_mul(a0) & 0xFFFF;
        let f22 = ee::mul(f32::from_bits(0x3DCC_CCCD), fx.tsin(s2));
        let f23 = ee::mul(f32::from_bits(0x3DCC_CCCD), fx.tcos(s2));
        for i in 0..0x18u32 {
            let r = (i % 2) as i32;
            let f24 = ee::add(ee::div(ee::from_i32(r), 2.0), f23);
            let f25 = ee::add(ee::div(ee::from_i32(r + 1), 2.0), f23);
            let (s4, s5) = (D + 0xA070 + 0x190 * i, D + 0xA070 + 0x190 * (i + 1));
            for j in 0..0x18u32 {
                let c = (j % 2) as i32;
                let u0 = ee::add(ee::div(ee::from_i32(c), 2.0), f22);
                let u1 = ee::add(ee::div(ee::from_i32(c + 1), 2.0), f22);
                for (o, x) in [(0x598u32, u0), (0x59C, f24), (0x5A8, u1), (0x5AC, f24), (0x5B8, u0), (0x5BC, f25), (0x5C8, u1), (0x5CC, f25)] {
                    fx.mem.setf(sp + o, x);
                }
                let vs = [s4 + 0x10 * j, s4 + 0x10 * (j + 1), s5 + 0x10 * j, s5 + 0x10 * (j + 1)];
                if fx.project_n(&vs, sp + 0x2E0, d, sp + 0x5FC, 0x01E0_2A74) == 1 {
                    let w = fx.mem.f(sp + 0x5FC);
                    let key = fixu(fx, w, 0x01E0_2A88);
                    fx.submit(d, OT, key as i32);
                }
            }
        }
        let x = fx.mem.i32(sp + 0x1C0).wrapping_add(0x3_3333);
        fx.mem.set32(sp + 0x1C0, x);
        let x = fx.mem.i32(sp + 0x1D0) + 0x20;
        fx.mem.set32(sp + 0x1D0, x);
    }
    fx.mem.write_u32(sp + 0x2D0, 2);
    let age = fx.mem.i32(D + 0xD148) + 1;
    fx.mem.set32(D + 0xD148, age);
    if age >= 0x21D {
        fx.mem.write_u8(D + 0xD144, 0);
    }
}

/// b624's motes [F 0x1E02E58..0x1E032D0]: from update 0 for 0x75 updates
/// (D+0xA510, age +4, angle +8) 128 sprites (cells D+0xA51C, vectors 0.5
/// with y x 2 at D+0xA5A0) turned about y by an angle advancing 6666
/// ratio_below(age, 96) a update, at the attacker's point D+0x40, drawn in
/// towards the node by 1 - ramp_above(age, 116, 96), sized 4 (0.2 + 0.8
/// ramp), colour (58, 58, D0), alpha 100 ratio_below(age, 96).
fn motes(fx: &mut Fx, v: &V, sp: u32) {
    let ra = v.bl.ra;
    let m = dz(v) + 0xA500;
    if fx.mem.i32(D) == 0 {
        fx.mem.write_u8(m, 1);
        fx.mem.write_u32(m + 4, 0);
        fx.mem.write_u32(m + 8, 0);
        for k in 0..0x80u32 {
            let b = fx.rand_from(v.rs0, ra[4]) >> 23;
            fx.mem.set8(m + 0xC + k, b % 8);
            let q = m + 0x90 + 0x10 * k;
            fx.random_table_vec(q, 0.5);
            let y = fx.mem.f(q + 4);
            fx.mem.setf(q + 4, ee::mul(y, 2.0));
        }
    }
    if fx.mem.read_u8(m) != 1 {
        return;
    }
    head(fx, v, sp, 0x18, 1, false);
    camera(fx, v, sp);
    let age = ee::from_i32(fx.mem.i32(m + 4));
    let f20 = Fx::ratio_below(age, 96.0);
    let al = fixu(fx, ee::mul(100.0, f20), ra[5]);
    rgba(fx, sp + at(v, 0x490), [0x58, 0x58, 0xD0], al as u8);
    fx.identity(sp + at(v, 0x410));
    let an = ee::add(ee::from_i32(fx.mem.i32(m + 8)), ee::mul(6666.0, f20));
    let an = fixs(fx, an, ra[6]);
    fx.mem.set32(m + 8, an);
    let an = fx.mem.i32(m + 8);
    fx.rotate_axis(sp + at(v, 0x410), an, 1);
    let age = ee::from_i32(fx.mem.i32(m + 4));
    let r = Fx::ramp_above(age, 116.0, 96.0);
    let s = ee::mul(4.0, ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), r)));
    corners(fx, v, sp, s, ee::neg(s));
    let age = ee::from_i32(fx.mem.i32(m + 4));
    let f20 = ee::sub(1.0, Fx::ramp_above(age, 116.0, 96.0));
    let (t, u) = (sp + at(v, 0x450), sp + at(v, 0x460));
    for k in 0..0x80u32 {
        let b = u32::from(fx.mem.read_u8(m + 0xC + k));
        cell_uvs(fx, v, sp, b);
        fx.vu_apply_to(t, sp + at(v, 0x410), m + 0x90 + 0x10 * k);
        fx.vu_add4(t, t, dz(v) + 0x30);
        fx.vu_sub4(u, D + 0x30, t);
        fx.vu_scale4(u, u, f20);
        fx.vu_add4(t, t, u);
        sprite(fx, v, sp, ra[7]);
    }
    let age = fx.mem.i32(m + 4) + 1;
    fx.mem.set32(m + 4, age);
    if age >= 0x75 {
        fx.mem.write_u8(m, 0);
    }
}
