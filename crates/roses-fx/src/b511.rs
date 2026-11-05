//! BMODULE b511 (card 498), hand-ported from the record (linked at
//! 0x1E00000; the entry 0x1E00080 in a 0x6A0 frame with D in s0 and the
//! OT in s1, data area a0 = 0x1ED0000, 0xB230 bytes; the dome callback
//! 0x1E03A60 with its static D at 0x1E04300) [F].
//! b440's family: the init lays out two flowers of 8 rings x 16 shards
//! (D+0x160, 30 long / 2 wide, cells D+0x2160; D+0x7C20, 80 long / 5 wide
//! tilted by the side, cells D+0x9C20), b440's flat 32 x 17 disc (D+0x3810)
//! and darkens the attacker. Every update the camera turn with roll (sp +
//! 0x460), the attacker's point D+0x30, six nodes 0xF / 0x10 / 0x20 / 0x21
//! / 0x1A / 0x19 (D+0x70..) with two aims (D+0xD0 / 0x110: node 0x10 from
//! 0xF, 0x21 from 0x20) and the target's screen point D+0x40. A lens at
//! the target from 0x157 for 0x31 updates (D+0xB220, b424's fan); on n % 4
//! == 0 of 0x7B..0xF9 a pair of flowers (D+0x21E0, 3 x 0xB0) and spark
//! sets (D+0x2400, 16 x 0x140) at nodes 0x10 / 0x21; from 0 for 0x31
//! updates the attacker drawn into its target with b440's dome callback
//! (b296's ramp); on 0x126 one big flower at node 0x1A (D+0x9CA0) with 64
//! sparks (D+0x9CD0); on 0x157 a burst at the target (D+0xA140, 0x31
//! updates: 64 puffs, 64 sparks in two colours, a 195-wide halo and 128
//! rays of six sprites). The stage light 0x80 - 80 clamp(0x1A7 - n)
//! clamp(n) / 1024; returns 1 / 6 / 2 / 8 on 0x14D / 0x158..0x186 / 0x187
//! / 0x1A8.
//!
//! b723 (card 507, [`program_b723`]) is the same program on [`V`] in a
//! 0x870 frame (D kept at sp + 0x150, the OT in s3; its locals 0x1D0 above
//! b511's), 0xEE70 bytes: nodes 6 / 7 / 3 / 4 / 0x13 / 0x12 (node 0x13 3.1
//! lower), other sizes, colours, timings and alphas, the dome 45 wide, and
//! four more parts after the burst: 32 gather records at node 0x13 on
//! 0x43..0xCF (D+0xB230, 0x130), on 0xFF a beam from node 0x13 to the
//! target (D+0xD840, 0x47 updates) dropping 8 trail records (D+0xD870,
//! 0x60) and 16 spark records (D+0xDB70, 0x130).
use crate::b182::{alpha, head, rgba, uv};
use crate::b440::{copy8, copy_z, fix, rem2, words, Dome, Pulse};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_xy, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
const PROJ_OUT: u32 = 0x002D_3600;
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
const FAR_Z: u64 = 0x00FF_FFF8_0000_0000;
/// The sprite descriptor (prim 0x26 | 0x800, 5 registers).
const DESC: u32 = 0x530;
/// The flowers' quad descriptor (prim 0x34, 12 registers).
const QUAD: u32 = 0x600;
/// The lens' triangle descriptor.
const TRI: u32 = 0x588;

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

/// A flower of 8 rings x 16 shards at D+`at` (cells D+`cells`): ring i
/// `len` (i + 1) / 8 long, the shard's base turned along (w (7 - i) / 7
/// (sin, cos), `len` i / 7 (x -side with `side`)) and by a random z angle;
/// half-width `half`.
struct Flower {
    at: u32,
    cells: u32,
    len: f32,
    width: f32,
    side: bool,
    half: f32,
    ra: [u32; 2],
}

/// The return addresses of one record's fix / rand / tint calls.
struct Ras {
    init_tint: u32,
    lens: [u32; 12],
    spawn: [u32; 2],
    shards: u32,
    flower_far: u32,
    sparks: u32,
    tint: [u32; 3],
    pass_key: u32,
    big_spawn: [u32; 2],
    big_shards: u32,
    big_far: u32,
    big_sparks: u32,
    burst_spawn: [u32; 2],
    halo: u32,
    ray_alpha: u32,
    rays: u32,
    dome: [u32; 6],
}

/// b723's parts after the burst.
struct Extra {
    ras: [u32; 9],
}

/// One record of the family.
pub struct V {
    frame: u32,
    /// The locals' offset above the frame's stack pointer.
    sps: u32,
    bzero: u32,
    nodes: [i32; 6],
    flowers: [Flower; 2],
    disc: f32,
    /// Node 0x13's y moved by this every update (b723).
    node_dy: Option<f32>,
    lens_on: i32,
    /// Flower spawns on n % 4 == 0 (b511) or every update, from..to.
    spawn_mod4: bool,
    spawn: (i32, i32),
    spark_len: f32,
    /// The flower alpha (3 - age) x this / 3, its colour, far size and
    /// colour.
    flower_mul: i32,
    flower_rgb: [u8; 3],
    flower_far: f32,
    flower_far_rgb: [u8; 3],
    spark_size: f32,
    callback: u32,
    d_static: u32,
    dome_radius: f32,
    big_on: i32,
    big_rgb: [u8; 3],
    big_far: f32,
    /// The big flower's far alpha 255 (1 - tan falloff) (b723) or 0.
    big_far_fade: bool,
    big_far_rgb: [u8; 3],
    /// The big sparks' alpha 2 (0x20 - age) (b723) or 0.
    big_sparks_fade: bool,
    burst_on: i32,
    /// The puffs' alpha (0x30 - age) x this / 48.
    puff_mul: i32,
    puff_rgb: [u8; 3],
    spark_rgb: [[u8; 3]; 2],
    halo_scale: f32,
    halo_rgb: [u8; 3],
    ray_rgb: [u8; 3],
    /// The rays' alpha 255 (s + 6) / 12 (0x30 - age) / 48 (b723) or 0.
    ray_fade: bool,
    extra: Option<Extra>,
    light: i32,
    ret: (i32, i32, i32, i32),
    ra: Ras,
}

const V511: V = V {
    frame: 0x6A0,
    sps: 0,
    bzero: 0xB230,
    nodes: [0xF, 0x10, 0x20, 0x21, 0x1A, 0x19],
    flowers: [
        Flower { at: 0x160, cells: 0x2160, len: -30.0, width: 2.0, side: false, half: 1.0, ra: [0x01E0_0298, 0x01E0_0340] },
        Flower { at: 0x7C20, cells: 0x9C20, len: -80.0, width: 5.0, side: true, half: 2.0, ra: [0x01E0_05C0, 0x01E0_0668] },
    ],
    disc: 35.0,
    node_dy: None,
    lens_on: 0x157,
    spawn_mod4: true,
    spawn: (0x7B, 0xFA),
    spark_len: f(0x3E99_999A),
    flower_mul: 0x56,
    flower_rgb: [0x40, 0x30, 0xFF],
    flower_far: 40.0,
    flower_far_rgb: [0xFF, 0x4C, 0x14],
    spark_size: 5.0,
    callback: 0x01E0_3A60,
    d_static: 0x01E0_4300,
    dome_radius: 35.0,
    big_on: 0x126,
    big_rgb: [0x40, 0xFF, 0x30],
    big_far: 90.0,
    big_far_fade: false,
    big_far_rgb: [0xFF, 0xB0, 0x78],
    big_sparks_fade: false,
    burst_on: 0x157,
    puff_mul: 200,
    puff_rgb: [0x6C, 0xFF, 0x38],
    spark_rgb: [[0x80, 0xFF, 0xFF], [0x5A, 0xFF, 0x1E]],
    halo_scale: 200.0,
    halo_rgb: [0x5E, 0xFF, 0x3A],
    ray_rgb: [0xFF, 0xF0, 0x80],
    ray_fade: false,
    extra: None,
    light: 0x1A7,
    ret: (0x14D, 0x158, 0x187, 0x1A8),
    ra: Ras {
        init_tint: 0x01E0_04BC,
        lens: [
            0x01E0_0AF8, 0x01E0_0B04, 0x01E0_0B48, 0x01E0_0B54, 0x01E0_0B98, 0x01E0_0BA4, 0x01E0_0C58, 0x01E0_0C64, 0x01E0_0CD0, 0x01E0_0CDC, 0x01E0_0D48,
            0x01E0_0D54,
        ],
        spawn: [0x01E0_0ED8, 0x01E0_0FC8],
        shards: 0x01E0_1334,
        flower_far: 0x01E0_149C,
        sparks: 0x01E0_1920,
        tint: [0x01E0_19F8, 0x01E0_1A28, 0x01E0_1C34],
        pass_key: 0x01E0_1BF4,
        big_spawn: [0x01E0_1CC0, 0x01E0_1D58],
        big_shards: 0x01E0_211C,
        big_far: 0x01E0_2290,
        big_sparks: 0x01E0_2754,
        burst_spawn: [0x01E0_29B0, 0x01E0_2AB8],
        halo: 0x01E0_34C0,
        ray_alpha: 0,
        rays: 0x01E0_3890,
        dome: [0x01E0_3CBC, 0x01E0_3E6C, 0x01E0_3F60, 0x01E0_3FC4, 0x01E0_4104, 0x01E0_4180],
    },
};

const V723: V = V {
    frame: 0x870,
    sps: 0x1D0,
    bzero: 0xEE70,
    nodes: [6, 7, 3, 4, 0x13, 0x12],
    flowers: [
        Flower { at: 0x160, cells: 0x2160, len: -120.0, width: 2.0, side: false, half: 3.0, ra: [0x01E0_02F8, 0x01E0_03A0] },
        Flower { at: 0x7C20, cells: 0x9C20, len: -80.0, width: 35.0, side: true, half: 5.0, ra: [0x01E0_062C, 0x01E0_06D8] },
    ],
    disc: 45.0,
    node_dy: Some(f(0xC046_6666)),
    lens_on: 0x145,
    spawn_mod4: false,
    spawn: (0x4B, 0xAA),
    spark_len: 1.0,
    flower_mul: 48,
    flower_rgb: [0x64, 0x80, 0x44],
    flower_far: 80.0,
    flower_far_rgb: [0xFF, 0x40, 0x20],
    spark_size: 10.0,
    callback: 0x01E0_53D0,
    d_static: 0x01E0_5C80,
    dome_radius: 45.0,
    big_on: 0xFF,
    big_rgb: [0x78, 0x50, 0xFF],
    big_far: 220.0,
    big_far_fade: true,
    big_far_rgb: [0x78, 0x50, 0xFF],
    big_sparks_fade: true,
    burst_on: 0x145,
    puff_mul: 255,
    puff_rgb: [0x78, 0x50, 0xFF],
    spark_rgb: [[0x80, 0x80, 0x80], [0x78, 0x50, 0xFF]],
    halo_scale: 255.0,
    halo_rgb: [0x78, 0x50, 0xFF],
    ray_rgb: [0x78, 0x50, 0xFF],
    ray_fade: true,
    extra: Some(Extra { ras: [0x01E0_3D30, 0x01E0_3F28, 0x01E0_40BC, 0x01E0_41F4, 0x01E0_43A0, 0x01E0_48A8, 0x01E0_4920, 0x01E0_49D0, 0x01E0_4DF4] }),
    light: 0x195,
    ret: (0x13B, 0x146, 0x175, 0x196),
    ra: Ras {
        init_tint: 0x01E0_0528,
        lens: [
            0x01E0_0CD4, 0x01E0_0CE4, 0x01E0_0D28, 0x01E0_0D34, 0x01E0_0D78, 0x01E0_0D84, 0x01E0_0E4C, 0x01E0_0E5C, 0x01E0_0EC8, 0x01E0_0ED4, 0x01E0_0F40,
            0x01E0_0F4C,
        ],
        spawn: [0x01E0_10C4, 0x01E0_11B8],
        shards: 0x01E0_1544,
        flower_far: 0x01E0_16BC,
        sparks: 0x01E0_1B28,
        tint: [0x01E0_1C14, 0x01E0_1C4C, 0x01E0_1E5C],
        pass_key: 0x01E0_1E10,
        big_spawn: [0x01E0_1F08, 0x01E0_1FB0],
        big_shards: 0x01E0_23A4,
        big_far: 0x01E0_253C,
        big_sparks: 0x01E0_2A04,
        burst_spawn: [0x01E0_2CB0, 0x01E0_2DD0],
        halo: 0x01E0_37D4,
        ray_alpha: 0x01E0_3B3C,
        rays: 0x01E0_3BF8,
        dome: [0x01E0_562C, 0x01E0_57DC, 0x01E0_58D0, 0x01E0_5934, 0x01E0_5A74, 0x01E0_5AF0],
    },
};

/// The entry 0x1E00080 of b511 (card 498).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &V511)
}

/// b723 (card 507).
pub fn program_b723(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &V723)
}

fn run(fx: &mut Fx, param: i32, v: &V) -> i32 {
    let spr = fx.enter(v.frame);
    fx.mem.write_u32(spr + 0xB0, 0);
    if v.sps != 0 {
        fx.mem.write_u32(spr + 0x150, D);
    }
    let sp = spr + v.sps;
    let ret = if param >= 0 { init(fx, sp, v) } else { step(fx, spr, sp, v) };
    fx.leave(v.frame);
    ret
}

fn rnd(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(D, ra) >> 23
}

fn vec4(fx: &mut Fx, p: u32, v: [f32; 4]) {
    for (k, x) in v.iter().enumerate() {
        fx.mem.setf(p + 4 * k as u32, *x);
    }
}

/// The tint 0x1F7600(attacker, mode, c) from `ra` (s0 = D, s1 = the OT).
fn tint(fx: &mut Fx, mode: i32, c: u32, ra: u32) {
    let h = fx.mem.i32(D + 4);
    fx.unit_tint_mode(h, mode, c, [u64::from(D), u64::from(OT)], ra);
}

/// (x x 0x2AAAAAAB >> 32 >> 3) + sign: x / 48.
fn div48(fx: &mut Fx, x: i32) -> i32 {
    fx.mult(0x2AAA_AAAB, x);
    (fx.hilo().1 >> 3).wrapping_add(((x as u32) >> 31) as i32)
}

fn flower_init(fx: &mut Fx, sp: u32, v: &Flower) {
    let (m, vv) = (sp + 0x4E0, sp + 0x520);
    for i in 0..8i32 {
        let f20 = ee::div(ee::mul(v.len, ee::from_i32(i + 1)), 8.0);
        let mut f22 = ee::div(ee::mul(v.len, ee::from_i32(i)), 7.0);
        if v.side {
            let s = ee::from_i32(fx.mem.i32(D + 0xC));
            f22 = ee::mul(-1.0, ee::mul(f22, s));
        }
        let f21 = ee::div(ee::mul(v.width, ee::from_i32(7 - i)), 7.0);
        let s7 = (i << 13) >> 3;
        for j in 0..0x10i32 {
            let a = (((j << 16) >> 4).wrapping_add(s7)) & 0xFFFF;
            let sn = fx.tsin(a);
            fx.mem.setf(vv, ee::mul(f21, sn));
            let cs = fx.tcos(a);
            fx.mem.setf(vv + 4, ee::mul(f21, cs));
            fx.mem.setf(vv + 8, f22);
            fx.mem.write_u32(vv + 0xC, 0x3F80_0000);
            fx.point_along(m, vv);
            let r = rnd(fx, v.ra[0]);
            fx.rotate_axis(m, (rem2(r, 256) << 16) >> 8, 2);
            let q = D + 0x400 * i as u32 + 0x40 * j as u32 + v.at;
            let (n, p) = (ee::neg(v.half), v.half);
            vec4(fx, q, [n, 0.0, 0.0, 1.0]);
            vec4(fx, q + 0x10, [p, 0.0, 0.0, 1.0]);
            vec4(fx, q + 0x20, [n, 0.0, f20, 1.0]);
            vec4(fx, q + 0x30, [p, 0.0, f20, 1.0]);
            for k in 0..4u32 {
                fx.vu_apply_to(q + 0x10 * k, m, q + 0x10 * k);
            }
            let r = rnd(fx, v.ra[1]);
            fx.mem.set8(D + 0x10 * i as u32 + j as u32 + v.cells, rem2(r, 4) + 4);
        }
    }
}

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
    for k in 0..3u32 {
        let r = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x10 + 4 * k, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x20);
    for (k, &node) in v.nodes.iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set32(D + 0x50 + 4 * k as u32, r);
    }
    fx.mem.write_u32(D + 0x150, 0);
    flower_init(fx, sp, &v.flowers[0]);
    for k in 0..3u32 {
        fx.mem.write_u8(D + 0x21E0 + 0xB0 * k, 0);
    }
    fx.mem.write_u32(D + 0x23F0, 0);
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x2400 + 0x140 * k, 0);
    }
    fx.mem.write_u8(D + 0x3800, 0);
    // The flat disc: 32 rings of 17 points (0x110 a ring).
    for i in 0..0x20i32 {
        let f20 = ee::div(ee::mul(v.disc, ee::from_i32(0x20 - i)), 32.0);
        let row = D + 0x110 * i as u32 + 0x3810;
        for j in 0..0x11i32 {
            let a = (j << 16) >> 4;
            let sn = fx.tsin(a);
            fx.mem.setf(row + 0x10 * j as u32, ee::mul(f20, sn));
            let cs = fx.tcos(a);
            fx.mem.setf(row + 4 + 0x10 * j as u32, ee::mul(f20, cs));
            fx.mem.write_u32(row + 8 + 0x10 * j as u32, 0);
            fx.mem.write_u32(row + 0xC + 0x10 * j as u32, 0x3F80_0000);
        }
    }
    let c = sp + 0x69C;
    for (k, b) in [0u8, 0, 0, 0x80].into_iter().enumerate() {
        fx.mem.write_u8(c + k as u32, b);
    }
    tint(fx, 1, c, v.ra.init_tint);
    fx.mem.write_u32(D + 0x7C10, 0);
    flower_init(fx, sp, &v.flowers[1]);
    fx.mem.write_u8(D + 0x9CA0, 0);
    fx.mem.write_u32(D + 0x9CC0, 0);
    fx.mem.write_u8(D + 0x9CD0, 0);
    fx.mem.write_u32(D + 0xA130, 0);
    fx.mem.write_u8(D + 0xA140, 0);
    fx.mem.write_u8(D + 0xB220, 0);
    if v.extra.is_some() {
        for k in 0..0x20u32 {
            fx.mem.write_u8(D + 0xB230 + 0x130 * k, 0);
        }
        fx.mem.write_u32(D + 0xD830, 0);
        fx.mem.write_u32(D + 0xD834, 0);
        fx.mem.write_u8(D + 0xD840, 0);
        fx.mem.write_u32(D + 0xD848, 0);
        fx.mem.write_u32(D + 0xD84C, 0);
        for k in 0..8u32 {
            fx.mem.write_u8(D + 0xD870 + 0x60 * k, 0);
        }
        for k in 0..0x10u32 {
            fx.mem.write_u8(D + 0xDB70 + 0x130 * k, 0);
        }
    }
    0
}

fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x3E0);
    fx.identity(sp + 0x420);
    crate::blib::exe::mul_mem(fx, sp + 0x3E0, sp + 0x420);
}

/// The screen point of `p` (0x204D80 through sp + 0x3E0 into sp + 0x690)
/// into `o` (b424's, x / y clamped to 0..512).
fn screen_point(fx: &mut Fx, sp: u32, p: u32, o: u32) {
    let m = fx.read_m4(sp + 0x3E0);
    fx.project_at(p, &m, sp + 0x690, sp + 0x698);
    let x = (i32::from(fx.mem.read_u16(sp + 0x690)) >> 4) - 0x700;
    fx.mem.setf(o, ee::from_i32(x));
    let y = (i32::from(fx.mem.read_u16(sp + 0x692)) >> 4) - 0x700;
    fx.mem.setf(o + 4, ee::from_i32(y));
    let z = (fx.mem.read_u64(sp + 0x690) << 8) >> 40;
    fx.mem.setf(o + 8, z as u32 as f32);
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

fn step(fx: &mut Fx, spr: u32, sp: u32, v: &V) -> i32 {
    fx.camera_turn_roll(sp + 0x460);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x30);
    for k in 0..6u32 {
        let h = fx.mem.i32(D + 4);
        let s = fx.mem.i32(D + 0x50 + 4 * k);
        fx.node_position(h, s, D + 0x70 + 0x10 * k);
    }
    for k in 0..2u32 {
        fx.vu_sub4(sp + 0x520, D + 0x70 + 0x10 * (2 * k + 1), D + 0x70 + 0x20 * k);
        fx.point_along(D + 0xD0 + 0x40 * k, sp + 0x520);
    }
    if let Some(dy) = v.node_dy {
        let y = ee::add(fx.mem.f(D + 0xB4), dy);
        fx.mem.setf(D + 0xB4, y);
    }
    camera(fx, sp);
    screen_point(fx, sp, D + 0x20, D + 0x40);

    lens(fx, spr, sp, v);
    let n = fx.mem.i32(D);
    if (!v.spawn_mod4 || rem2(n, 4) == 0) && (v.spawn.0..v.spawn.1).contains(&n) {
        flower_spawn(fx, v);
    }
    flowers(fx, sp, v);
    sparks(fx, sp, v);
    if fx.mem.i32(D) == 0 {
        fx.mem.write_u8(D + 0x3800, 1);
        fx.mem.write_u32(D + 0x3804, 0);
    }
    tint(fx, 0, 0, v.ra.tint[0]);
    if fx.mem.read_u8(D + 0x3800) == 1 {
        unit_pass(fx, sp, v);
        let t = fx.mem.i32(D + 0x3804) + 1;
        fx.mem.set32(D + 0x3804, t);
        if t >= 0x31 {
            fx.mem.write_u8(D + 0x3800, 0);
            tint(fx, 0, 0, v.ra.tint[2]);
        }
    }
    if fx.mem.i32(D) == v.big_on {
        big_spawn(fx, v);
    }
    big_flower(fx, sp, v);
    big_sparks(fx, sp, v);
    if fx.mem.i32(D) == v.burst_on {
        burst_spawn(fx, v);
    }
    burst(fx, spr, sp, v);
    if let Some(e) = &v.extra {
        gather(fx, sp, e);
        beam(fx, sp, e);
        trails(fx, sp, e);
        beam_sparks(fx, sp);
    }

    let n = fx.mem.i32(D);
    let s1 = Fx::clamp_i(v.light - n, 0, 0x20);
    let n = fx.mem.i32(D);
    let c = Fx::clamp_i(n, 0, 0x20);
    let l = (fx.mult(s1, c.wrapping_mul(-80)) >> 10) + 0x80;
    fx.set_stage_light(l & 0xFF);

    let n = fx.mem.i32(D);
    let mut ret = fx.mem.i32(spr + 0xB0);
    if n == v.ret.0 {
        ret = 1;
    } else {
        if (v.ret.1..v.ret.2).contains(&n) {
            ret = 6;
        }
        if n == v.ret.2 {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= v.ret.3 {
        ret = 8;
    }
    fx.mem.set32(spr + 0xB0, ret);
    ret
}

/// The lens at the target's screen point (D+0xB220 for 0x31 updates) [F
/// b511 0x1E00920..0x1E00E40].
fn lens(fx: &mut Fx, spr: u32, sp: u32, v: &V) {
    if fx.mem.i32(D) == v.lens_on {
        fx.mem.write_u8(D + 0xB220, 1);
        fx.mem.write_u32(D + 0xB224, 0);
    }
    if fx.mem.read_u8(D + 0xB220) != 1 {
        return;
    }
    let t = sp + TRI;
    head(fx, t, 0x33, 9, 0x4_2142_1421, 0x800);
    alpha(fx, t, 0);
    fx.desc_clamp(t, LENS_CLAMP);
    let a = crate::b182::clamp_regs(fx, t, LENS_CLAMP);
    let layer = fx.layer_begin();
    fx.mem.write_u32(spr + 0xEC, layer);
    let regs = crate::lens::frame_copy(fx, a, 1, sp + 0x5E8, sp + 0x5F0);
    fx.texflush(regs);
    let age = fx.mem.i32(D + 0xB224);
    let al = div48(fx, (0x30 - age) << 6);
    rgba(fx, sp + 0x598, [0x80; 3], al as u8);
    copy8(fx, sp + 0x5B0, sp + 0x598);
    copy8(fx, sp + 0x5C8, sp + 0x598);
    for j in 0..8i32 {
        let s = ee::add(1.0, ee::div(ee::mul(f(0x3E19_999A), ee::from_i32(8 - j)), 8.0));
        crate::b424::fan(fx, t, D + 0x40, sp + 0x4C0, s, 0, &v.ra.lens);
    }
    let layer = fx.mem.read_u32(spr + 0xEC);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0xB224) + 1;
    fx.mem.set32(D + 0xB224, age);
    if age >= 0x31 {
        fx.mem.write_u8(D + 0xB220, 0);
    }
}

/// A flower pair (D+0x21E0 + 0xB0 k) and a spark pair (D+0x2400 + 0x140
/// m) started at nodes 1 / 3 [F b511 0x1E00E7C..0x1E0106C].
fn flower_spawn(fx: &mut Fx, v: &V) {
    let k = fx.mem.i32(D + 0x150) as u32;
    let fb = D + 0xB0 * k + 0x21E0;
    fx.mem.write_u8(fb, 1);
    fx.mem.write_u32(fb + 4, 0);
    let m = fx.mem.i32(D + 0x23F0) as u32;
    let tb = D + 0x140 * m + 0x2400;
    fx.mem.write_u8(tb, 1);
    fx.mem.write_u32(tb + 4, 0);
    for s in 0..2u32 {
        let r = rnd(fx, v.ra.spawn[0]);
        fx.mem.set32(fb + 8 + 4 * s, (rem2(r, 256) << 16) >> 8);
        words(fx, fb + 0x10 + 0x10 * s, D + 0x70 + 0x10 * (1 + 2 * s));
        for c in 0..8u32 {
            copy8(fx, fb + 0x30 + 0x40 * s + 8 * c, D + 0xD0 + 0x40 * s + 8 * c);
        }
    }
    for s in 0..2u32 {
        words(fx, tb + 0x20 + 0x10 * s, D + 0x70 + 0x10 * (1 + 2 * s));
        for j in 0..8u32 {
            let r = rnd(fx, v.ra.spawn[1]);
            fx.mem.set8(tb + 8 + 8 * s + j, rem2(r, 8));
            fx.random_table_vec(tb + 0x40 + 0x80 * s + 0x10 * j, v.spark_len);
        }
    }
    let k = fx.mem.i32(D + 0x150) + 1;
    fx.mem.set32(D + 0x150, k);
    if k >= 3 {
        fx.mem.write_u32(D + 0x150, 0);
    }
    let m = fx.mem.i32(D + 0x23F0) + 1;
    fx.mem.set32(D + 0x23F0, m);
    if m >= 0x10 {
        fx.mem.write_u32(D + 0x23F0, 0);
    }
}

fn quad_head(fx: &mut Fx, sp: u32) {
    let q = sp + QUAD;
    head(fx, q, 0x34, 0xC, 0x4214_2142_1421, 0);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(q, rh, 1);
    alpha(fx, q, 1);
    fx.desc_clamp(q, 0);
}

/// The sprite descriptor (prim 0x26 | 0x800) with texture D+`tex`, clamp
/// 5 and (with `preset`) its ALPHA.
fn sprite_desc(fx: &mut Fx, sp: u32, tex: u32, preset: Option<i32>) {
    let d = sp + DESC;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    if let Some(p) = preset {
        alpha(fx, d, p);
    }
}

/// The sprite descriptor in b723's later order: ALPHA preset 1, texture
/// D+`tex`, clamp 5.
fn sprite_desc_atc(fx: &mut Fx, sp: u32, tex: u32) {
    let d = sp + DESC;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    alpha(fx, d, 1);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
}

/// The flower colour (sp + 0x610 and its three copies).
fn quad_colour(fx: &mut Fx, sp: u32, c: [u8; 3], a: u8) {
    rgba(fx, sp + 0x610, c, a);
    for o in [0x628u32, 0x640, 0x658] {
        copy8(fx, sp + o, sp + 0x610);
    }
}

/// The 8 x 16 shards of `at` (cells `cells`) through the matrix at sp +
/// 0x3E0, keyed by their mean w.
fn shards(fx: &mut Fx, sp: u32, at: u32, cells: u32, ra: u32) {
    let q = sp + QUAD;
    let mm = fx.read_m4(sp + 0x3E0);
    for i in 0..8u32 {
        for j in 0..0x10u32 {
            let cb = i32::from(fx.mem.read_u8(D + 0x10 * i + j + cells));
            let c = rem2(cb, 4) << 5;
            let r = (cb >> 2) << 5;
            let f4 = ee::from_i32(c);
            let f2 = ee::from_i32(r);
            let f5 = ee::div(ee::add(32.0, f4), 128.0);
            let f4 = ee::div(f4, 128.0);
            let f3 = ee::div(f2, 64.0);
            let f0 = ee::div(ee::add(16.0, f2), 64.0);
            for (o, x) in [(0x618u32, f4), (0x61C, f0), (0x630, f5), (0x634, f0), (0x648, f4), (0x64C, f3), (0x660, f5), (0x664, f3)] {
                fx.mem.setf(sp + o, x);
            }
            let petal = D + 0x400 * i + 0x40 * j + at;
            if fx.project_desc(petal, PROJ_OUT, Some(&mm), q, sp + 0x698) {
                let w = fx.mem.f(sp + 0x698);
                let key = fix(fx, w, ra);
                fx.submit(q, OT, key as i32);
            }
        }
    }
}

/// The corners (-s, -s, 0, 1) / (s, s, 0, 1) at sp + 0x4A0 / 0x4B0,
/// turned by the camera turn with `turn`.
fn corners(fx: &mut Fx, sp: u32, s: f32, turn: bool) {
    let n = ee::neg(s);
    vec4(fx, sp + 0x4A0, [n, n, 0.0, 1.0]);
    vec4(fx, sp + 0x4B0, [s, s, 0.0, 1.0]);
    if turn {
        fx.vu_apply_to(sp + 0x4A0, sp + 0x460, sp + 0x4A0);
        fx.vu_apply_to(sp + 0x4B0, sp + 0x460, sp + 0x4B0);
    }
}

fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, sp + 0x548, u0);
    uv(fx, sp + 0x54A, v0);
    uv(fx, sp + 0x558, u1);
    uv(fx, sp + 0x55A, v1);
}

/// The 4 x 8 cell `b` of the texture into the sprite's UVs.
fn cell_uvs(fx: &mut Fx, sp: u32, b: i32) {
    let (c, r) = (rem2(b, 4), b >> 2);
    uvs(fx, sp, ((c << 9) & 0x3FFF) as u32, ((r << 9) & 0x3FFF) as u32, ((((c << 5) + 0x20) << 4) & 0x3FFF) as u32, ((((r << 5) + 0x20) << 4) & 0x3FFF) as u32);
}

/// The sprite at sp + 0x520 (= dir x f + at); its vertices sp + 0x4C0 /
/// 0x4D0.
fn place(fx: &mut Fx, sp: u32, dir: u32, f: f32, at: u32) {
    fx.vu_scale4(sp + 0x520, dir, f);
    fx.vu_add4(sp + 0x520, sp + 0x520, at);
    fx.vu_add4(sp + 0x4C0, sp + 0x4A0, sp + 0x520);
    fx.vu_add4(sp + 0x4D0, sp + 0x4B0, sp + 0x520);
}

/// The sprite projected and submitted at its mean w.
fn draw(fx: &mut Fx, sp: u32, ra: u32) {
    if crate::fxlib2::project(fx, sp + 0x4C0, PROJ_OUT, sp + 0x3E0, sp + DESC, sp + 0x698) {
        let w = fx.mem.f(sp + 0x698);
        let key = fix(fx, w, ra);
        fx.submit(sp + DESC, OT, key as i32);
    }
}

/// The sprite at the point `at` projected at far Z.
fn draw_far(fx: &mut Fx, sp: u32, at: u32) {
    words(fx, sp + 0x520, at);
    fx.vu_add4(sp + 0x4C0, sp + 0x4A0, sp + 0x520);
    fx.vu_add4(sp + 0x4D0, sp + 0x4B0, sp + 0x520);
    uvs(fx, sp, 0x600, 0, 0x800, 0x200);
    far_sprite(fx, sp);
}

fn far_sprite(fx: &mut Fx, sp: u32) {
    let d = sp + DESC;
    if crate::fxlib2::project(fx, sp + 0x4C0, PROJ_OUT, sp + 0x3E0, d, sp + 0x698) {
        let x = fx.mem.read_u64(sp + 0x550);
        fx.mem.write_u64(sp + 0x550, (x & KEEP) | FAR_Z);
        copy_z(fx, sp + 0x560, sp + 0x550);
        fx.submit(d, OT, 0);
    }
}

/// The flower pairs [F b511 0x1E01070..0x1E0167C].
fn flowers(fx: &mut Fx, sp: u32, v: &V) {
    quad_head(fx, sp);
    for k in 0..3u32 {
        let fb = D + 0xB0 * k + 0x21E0;
        if fx.mem.read_u8(fb) == 0 {
            continue;
        }
        let left = 3 - fx.mem.i32(fb + 4);
        let x = left.wrapping_mul(v.flower_mul);
        fx.mult(0x5555_5556, x);
        let a = fx.hilo().1.wrapping_add(((x as u32) >> 31) as i32);
        quad_colour(fx, sp, v.flower_rgb, a as u8);
        let age = ee::from_i32(fx.mem.i32(fb + 4));
        let s = ee::add(0.5, ee::div(ee::mul(0.5, age), 3.0));
        vec4(fx, sp + 0x520, [s, s, s, 1.0]);
        for s in 0..2u32 {
            fx.camera_vp_to(sp + 0x3E0);
            fx.translate_rows(sp + 0x420, fb + 0x30 + 0x40 * s, D + 0x70 + 0x10 * (1 + 2 * s));
            let ang = fx.mem.i32(fb + 8 + 4 * s);
            fx.rotate_axis(sp + 0x420, ang, 2);
            fx.scale_rows(sp + 0x420, sp + 0x520);
            crate::blib::exe::mul_mem(fx, sp + 0x3E0, sp + 0x420);
            shards(fx, sp, 0x160, 0x2160, v.ra.shards);
        }
        sprite_desc(fx, sp, 0x14, Some(1));
        camera(fx, sp);
        corners(fx, sp, v.flower_far, true);
        let age = fx.mem.i32(fb + 4);
        fx.tan_falloff_p(age, 3, 1);
        let a = fix(fx, 0.0, v.ra.flower_far);
        rgba(fx, sp + 0x540, v.flower_far_rgb, a as u8);
        for s in 0..2u32 {
            draw_far(fx, sp, fb + 0x10 + 0x10 * s);
        }
        let age = fx.mem.i32(fb + 4) + 1;
        fx.mem.set32(fb + 4, age);
        if age >= 4 {
            fx.mem.write_u8(fb, 0);
        }
    }
}

/// The spark pairs [F b511 0x1E01680..0x1E019CC].
fn sparks(fx: &mut Fx, sp: u32, v: &V) {
    sprite_desc(fx, sp, 0x10, Some(0));
    camera(fx, sp);
    for t in 0..0x10u32 {
        let tb = D + 0x140 * t + 0x2400;
        if fx.mem.read_u8(tb) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(tb + 4));
        let s = ee::mul(v.spark_size, ee::add(0.5, ee::div(ee::mul(0.5, age), 16.0)));
        corners(fx, sp, s, true);
        rgba(fx, sp + 0x540, [0xBC; 3], 0);
        let age = fx.mem.i32(tb + 4);
        let f20 = fx.tan_falloff_p(age, 0x10, 1);
        for s in 0..2u32 {
            for j in 0..8u32 {
                place(fx, sp, tb + 0x40 + 0x80 * s + 0x10 * j, f20, tb + 0x20 + 0x10 * s);
                let c = i32::from(fx.mem.read_u8(tb + 8 + 8 * s + j));
                cell_uvs(fx, sp, c);
                draw(fx, sp, v.ra.sparks);
            }
        }
        let age = fx.mem.i32(tb + 4) + 1;
        fx.mem.set32(tb + 4, age);
        if age >= 0x11 {
            fx.mem.write_u8(tb, 0);
        }
    }
}

/// The darkened attacker drawn into its target with the dome, composited
/// by a full-window sprite (b440's unit pass) [F b511 0x1E01A04..0x1E01C30].
fn unit_pass(fx: &mut Fx, sp: u32, v: &V) {
    let c = sp + 0x69C;
    for (k, b) in [0u8, 0, 0, 0x80].into_iter().enumerate() {
        fx.mem.write_u8(c + k as u32, b);
    }
    tint(fx, 1, c, v.ra.tint[1]);
    let d = sp + DESC;
    head(fx, d, 0x26, 5, 0x4_2421, 0);
    let layer = fx.layer_begin();
    // 0x1F7600 leaves a0 = a1 = 0, a2 = the colour, a3 = the object.
    let h = fx.mem.i32(D + 4);
    let i = fx.object_index(h >> 8, h & 0xFF);
    let o = fx.object_at(i);
    fx.texflush([0, 0, c, o]);
    fx.mem.write_u32(v.d_static, D);
    let cfg = Dome {
        d_static: v.d_static,
        cam: 0x30,
        timer: 0x3804,
        dome: 0x5A10,
        disc: 0x3810,
        ratio: None,
        ramp: (48.0, 24.0),
        pulse: Pulse::Dropped,
        alpha: 255.0,
        radius: v.dome_radius,
        wobble: 5461.0,
        ra: v.ra.dome,
    };
    let (_, regs) = fx.unit_draw_post(1, d + 0x40, d + 0x48, v.callback, &mut |fx, p| crate::b440::dome(fx, p, &cfg));
    fx.texflush(regs);
    alpha(fx, d, 0);
    fx.desc_clamp(d, LENS_CLAMP);
    let fl = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, fl | 0x800);
    rgba(fx, sp + 0x540, [0x80; 3], 0x80);
    uvs(fx, sp, 0, 0, 0x2000, 0x2000);
    far_xy(fx, sp + 0x550, 0x6FF8, 0x6FF8);
    far_xy(fx, sp + 0x560, 0x8FF8, 0x8FF8);
    fx.submit(d, 0, 0);
    let w = crate::blib::afterimage::depth_w(fx, D + 0x30, 0.0);
    fx.mem.setf(sp + 0x698, w);
    let key = fix(fx, w, v.ra.pass_key);
    fx.layer_end(layer, OT, key as i32);
}

/// The big flower (D+0x9CA0) and its sparks (D+0x9CD0) at node 4 [F b511
/// 0x1E01C44..0x1E01E38].
fn big_spawn(fx: &mut Fx, v: &V) {
    let c = fx.mem.i32(D + 0x7C10) as u32;
    let fb = D + 0x20 * c + 0x9CA0;
    fx.mem.write_u8(fb, 1);
    fx.mem.write_u32(fb + 4, 0);
    let c2 = fx.mem.i32(D + 0x9CC0) as u32;
    let tb = D + 0x460 * c2 + 0x9CD0;
    fx.mem.write_u8(tb, 1);
    fx.mem.write_u32(tb + 4, 0);
    let r = rnd(fx, v.ra.big_spawn[0]);
    fx.mem.set32(fb + 8, (rem2(r, 256) << 16) >> 8);
    words(fx, fb + 0x10, D + 0xB0);
    words(fx, tb + 0x50, D + 0xB0);
    for j in 0..0x40u32 {
        let r = rnd(fx, v.ra.big_spawn[1]);
        fx.mem.set8(tb + 8 + j, rem2(r, 8));
        fx.random_table_vec(tb + 0x60 + 0x10 * j, 1.0);
    }
    let c = fx.mem.i32(D + 0x7C10) + 1;
    fx.mem.set32(D + 0x7C10, c);
    if c > 0 {
        fx.mem.write_u32(D + 0x7C10, 0);
    }
    let c2 = fx.mem.i32(D + 0x9CC0) + 1;
    fx.mem.set32(D + 0x9CC0, c2);
    if c2 > 0 {
        fx.mem.write_u32(D + 0x9CC0, 0);
    }
}

/// The big flower [F b511 0x1E01E3C..0x1E02490].
fn big_flower(fx: &mut Fx, sp: u32, v: &V) {
    quad_head(fx, sp);
    let fb = D + 0x9CA0;
    if fx.mem.read_u8(fb) == 0 {
        return;
    }
    let left = 0xA - fx.mem.i32(fb + 4);
    let x = fx.mult(left, 0x56);
    fx.mult(0x6666_6667, x);
    let a = (fx.hilo().1 >> 2).wrapping_add(((x as u32) >> 31) as i32);
    quad_colour(fx, sp, v.big_rgb, a as u8);
    let age = ee::from_i32(fx.mem.i32(fb + 4));
    let s = ee::add(0.5, ee::div(ee::mul(0.5, age), 10.0));
    vec4(fx, sp + 0x520, [s, s, s, 1.0]);
    fx.camera_vp_to(sp + 0x3E0);
    fx.identity(sp + 0x420);
    fx.translate_rows(sp + 0x420, sp + 0x420, fb + 0x10);
    fx.rotate_axis(sp + 0x420, 0, 0);
    let ang = fx.mem.i32(fb + 8);
    fx.rotate_axis(sp + 0x420, ang, 2);
    fx.scale_rows(sp + 0x420, sp + 0x520);
    crate::blib::exe::mul_mem(fx, sp + 0x3E0, sp + 0x420);
    shards(fx, sp, 0x7C20, 0x9C20, v.ra.big_shards);
    sprite_desc(fx, sp, 0x14, Some(1));
    camera(fx, sp);
    corners(fx, sp, v.big_far, true);
    let age = fx.mem.i32(fb + 4);
    let tf = fx.tan_falloff_p(age, 0xA, 1);
    let a = if v.big_far_fade { fix(fx, ee::mul(255.0, ee::sub(1.0, tf)), v.ra.big_far) } else { fix(fx, 0.0, v.ra.big_far) };
    rgba(fx, sp + 0x540, v.big_far_rgb, a as u8);
    draw_far(fx, sp, fb + 0x10);
    let age = fx.mem.i32(fb + 4) + 1;
    fx.mem.set32(fb + 4, age);
    if age >= 0xB {
        fx.mem.write_u8(fb, 0);
    }
}

/// The big flower's 64 sparks [F b511 0x1E02494..0x1E02818].
fn big_sparks(fx: &mut Fx, sp: u32, v: &V) {
    sprite_desc(fx, sp, 0x10, Some(0));
    camera(fx, sp);
    let tb = D + 0x9CD0;
    if fx.mem.read_u8(tb) == 0 {
        return;
    }
    let age = ee::from_i32(fx.mem.i32(tb + 4));
    let s = ee::mul(10.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 32.0)));
    corners(fx, sp, s, true);
    let a = if v.big_sparks_fade { ((0x20 - fx.mem.i32(tb + 4)) << 6) >> 5 } else { 0 };
    rgba(fx, sp + 0x540, [0xBC; 3], a as u8);
    let age = fx.mem.i32(tb + 4);
    let f20 = fx.tan_falloff_p(age, 0x20, 1);
    for j in 0..0x40u32 {
        place(fx, sp, tb + 0x60 + 0x10 * j, f20, tb + 0x50);
        let c = i32::from(fx.mem.read_u8(tb + 8 + j));
        cell_uvs(fx, sp, c);
        draw(fx, sp, v.ra.big_sparks);
    }
    let age = fx.mem.i32(tb + 4) + 1;
    fx.mem.set32(tb + 4, age);
    if age >= 0x21 {
        fx.mem.write_u8(tb, 0);
    }
}

/// The burst record (D+0xA140, 0x10E0) at the target [F b511
/// 0x1E0281C..0x1E02B5C]: cells +8 (64 halfwords), point +0x90, 64 spark
/// vectors 2.7 +0xA0, 128 ray vectors 3.75 +0x4A0, 64 puff cells +0xCA0
/// and vectors 2.7 +0xCE0.
fn burst_spawn(fx: &mut Fx, v: &V) {
    fx.mem.write_u32(D + 0xA130, 0);
    let c = fx.mem.i32(D + 0xA130) as u32;
    let r = D + 0x10E0 * c + 0xA140;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 1);
    vec4(fx, r + 0x90, [0.0, 0.0, 0.0, 1.0]);
    fx.vu_add4(r + 0x90, r + 0x90, D + 0x20);
    for j in 0..0x40u32 {
        let b = rnd(fx, v.ra.burst_spawn[0]);
        fx.mem.write_u16(r + 8 + 2 * j, rem2(b, 8) as u16);
        fx.random_table_vec(r + 0xA0 + 0x10 * j, f(0x402C_CCCD));
    }
    for j in 0..0x80u32 {
        fx.random_table_vec(r + 0x4A0 + 0x10 * j, 3.75);
    }
    for j in 0..0x40u32 {
        let b = rnd(fx, v.ra.burst_spawn[1]);
        let (_, h) = fx.div(b, 3);
        fx.mem.set8(r + 0xCA0 + j, h + 5);
        fx.random_table_vec(r + 0xCE0 + 0x10 * j, f(0x402C_CCCD));
    }
}

/// The burst [F b511 0x1E02B60..0x1E03948].
fn burst(fx: &mut Fx, spr: u32, sp: u32, v: &V) {
    let r = D + 0xA140;
    if fx.mem.read_u8(r) == 0 {
        return;
    }
    let pt = r + 0x90;
    // Puffs.
    sprite_desc(fx, sp, 0x18, Some(1));
    camera(fx, sp);
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let s = ee::mul(18.0, ee::add(f(0x3F4C_CCCD), ee::div(ee::mul(f(0x3E4C_CCCD), age), 48.0)));
    corners(fx, sp, s, true);
    let left = 0x30 - fx.mem.i32(r + 4);
    let a = div48(fx, left.wrapping_mul(v.puff_mul));
    rgba(fx, sp + 0x540, v.puff_rgb, a as u8);
    let age = fx.mem.i32(r + 4);
    let f22 = fx.tan_falloff_p(age, 0x30, 1);
    for j in 0..0x40u32 {
        fx.vu_scale4(sp + 0x520, r + 0xCE0 + 0x10 * j, f22);
        fx.vu_add4(sp + 0x520, sp + 0x520, pt);
        fx.vu_add4(sp + 0x4C0, sp + 0x4A0, sp + 0x520);
        fx.vu_add4(sp + 0x4D0, sp + 0x4B0, sp + 0x520);
        let c = i32::from(fx.mem.read_u8(r + 0xCA0 + j));
        cell_uvs(fx, sp, c);
        far_sprite(fx, sp);
    }
    // Sparks in two colours.
    sprite_desc(fx, sp, 0x10, None);
    camera(fx, sp);
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let s = ee::mul(15.0, ee::add(f(0x3F4C_CCCD), ee::div(ee::mul(f(0x3E4C_CCCD), age), 48.0)));
    corners(fx, sp, s, true);
    let left = 0x30 - fx.mem.i32(r + 4);
    let a1 = div48(fx, (left << 8) - left);
    fx.mem.write_u8(sp + 0x3C0, a1 as u8);
    let a2 = div48(fx, left << 7);
    fx.mem.write_u8(spr + 0xD0, a2 as u8);
    for j in 0..0x40u32 {
        let age = fx.mem.i32(r + 4);
        let mut f22 = fx.tan_falloff_p(age, 0x30, 1);
        if j % 2 == 0 {
            f22 = ee::mul(f22, f(0x3F4C_CCCD));
            alpha(fx, sp + DESC, 2);
            rgba(fx, sp + 0x540, v.spark_rgb[0], a2 as u8);
        } else {
            alpha(fx, sp + DESC, 1);
            rgba(fx, sp + 0x540, v.spark_rgb[1], a1 as u8);
        }
        fx.vu_scale4(sp + 0x520, r + 0xA0 + 0x10 * j, f22);
        fx.vu_add4(sp + 0x520, sp + 0x520, pt);
        fx.vu_add4(sp + 0x4C0, sp + 0x4A0, sp + 0x520);
        fx.vu_add4(sp + 0x4D0, sp + 0x4B0, sp + 0x520);
        let c = i32::from(fx.mem.read_u16(r + 8 + 2 * j));
        cell_uvs(fx, sp, c);
        far_sprite(fx, sp);
    }
    // The halo, scaled by the spread at the point.
    sprite_desc(fx, sp, 0x14, Some(1));
    fx.camera_vp_to(sp + 0x3E0);
    fx.translate_rows(sp + 0x420, sp + 0x460, pt);
    let age = fx.mem.i32(r + 4);
    let tf = fx.tan_falloff_p(age, 0x30, 1);
    let s = ee::add(f(0x3E4C_CCCD), ee::mul(f(0x3F4C_CCCD), tf));
    vec4(fx, sp + 0x520, [s, s, s, 1.0]);
    fx.scale_rows(sp + 0x420, sp + 0x520);
    crate::blib::exe::mul_mem(fx, sp + 0x3E0, sp + 0x420);
    let left = ee::from_i32(0x30 - fx.mem.i32(r + 4));
    let a = fix(fx, ee::mul(v.halo_scale, ee::div(left, 48.0)), v.ra.halo);
    rgba(fx, sp + 0x540, v.halo_rgb, a as u8);
    uvs(fx, sp, 0x600, 0, 0x800, 0x200);
    vec4(fx, sp + 0x4A0, [-195.0, -195.0, 0.0, 1.0]);
    vec4(fx, sp + 0x4B0, [195.0, 195.0, 0.0, 1.0]);
    let d = sp + DESC;
    if crate::fxlib2::project(fx, sp + 0x4A0, PROJ_OUT, sp + 0x3E0, d, sp + 0x698) {
        let x = fx.mem.read_u64(sp + 0x550);
        fx.mem.write_u64(sp + 0x550, (x & KEEP) | FAR_Z);
        copy_z(fx, sp + 0x560, sp + 0x550);
        fx.submit(d, OT, 0);
    }
    // The rays.
    sprite_desc(fx, sp, 0x14, Some(1));
    uvs(fx, sp, 0x600, 0, 0x800, 0x200);
    camera(fx, sp);
    corners(fx, sp, 1.0, true);
    let f23 = ee::div(ee::from_i32(0x30 - fx.mem.i32(r + 4)), 48.0);
    let age = fx.mem.i32(r + 4);
    let f22 = fx.tan_falloff_p(age, 0x30, 1);
    for m in 0..0x80u32 {
        let dir = r + 0x4A0 + 0x10 * m;
        for s6 in 0..6i32 {
            let a = if v.ray_fade {
                let x = (s6 + 6) * 255;
                fx.mult(0x2AAA_AAAB, x);
                let q = (fx.hilo().1 >> 1).wrapping_add(((x as u32) >> 31) as i32);
                fix(fx, ee::mul(f23, ee::from_i32(q)), v.ra.ray_alpha)
            } else {
                0
            };
            rgba(fx, sp + 0x540, v.ray_rgb, a as u8);
            let f12 = ee::add(f22, ee::mul(f(0x3BE5_6042), ee::from_i32(s6)));
            place(fx, sp, dir, f12, pt);
            draw(fx, sp, v.ra.rays);
        }
    }
    let age = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, age);
    if age >= 0x31 {
        fx.mem.write_u8(r, 0);
    }
}

// ------------------------------------------------- b723's later parts

/// The gather records (D+0xB230 + 0x130 k, 32): on 0x43..0xCF one at node
/// 4 with 16 cells (+8) and 3.0 table vectors about the node (+0x30), drawn
/// drawn in twice (3-wide sprites of texture D+0x14, 10-wide of D+0x10)
/// for 0x21 updates [F b723 0x1E03CB4..0x1E04420].
fn gather(fx: &mut Fx, sp: u32, e: &Extra) {
    let n = fx.mem.i32(D);
    if (0x43..0xD0).contains(&n) {
        let r = D + 0x130 * rem2(n, 0x20) as u32 + 0xB230;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for j in 0..0x10u32 {
            let b = rnd(fx, e.ras[0]);
            fx.mem.set8(r + 8 + j, rem2(b, 4));
            let p = r + 0x30 + 0x10 * j;
            fx.random_table_vec(p, 3.0);
            fx.vu_add4(p, p, D + 0xB0);
        }
    }
    let d = sp + DESC;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    for k in 0..0x20u32 {
        let r = D + 0x130 * k + 0xB230;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        for (tex, size, amp, ra, key_ra, pull, cell) in [(0x14u32, 3.0f32, 128.0f32, e.ras[1], e.ras[2], 1.0f32, 0i32), (0x10, 10.0, 64.0, e.ras[3], e.ras[4], f(0xBF33_3333), 4)] {
            let rh = fx.mem.read_u32(D + tex);
            fx.desc_texture(d, rh, 1);
            alpha(fx, d, 1);
            let left = ee::from_i32(0x20 - fx.mem.i32(r + 4));
            let s = ee::mul(size, ee::add(f(0x3E4C_CCCD), ee::div(ee::mul(f(0x3F4C_CCCD), left), 32.0)));
            corners(fx, sp, s, true);
            let age = fx.mem.i32(r + 4);
            let sn = fx.tsin((age << 14) >> 5);
            let a = fix(fx, ee::mul(amp, sn), ra);
            rgba(fx, sp + 0x540, [0x78, 0x50, 0xFF], a as u8);
            let left = ee::from_i32(0x20 - fx.mem.i32(r + 4));
            let f20 = if cell == 0 { ee::div(left, 32.0) } else { ee::div(ee::mul(pull, left), 32.0) };
            for j in 0..0x10u32 {
                fx.vu_sub4(sp + 0x520, r + 0x30 + 0x10 * j, D + 0xB0);
                fx.vu_scale4(sp + 0x520, sp + 0x520, f20);
                fx.vu_add4(sp + 0x520, sp + 0x520, D + 0xB0);
                fx.vu_add4(sp + 0x4C0, sp + 0x4A0, sp + 0x520);
                fx.vu_add4(sp + 0x4D0, sp + 0x4B0, sp + 0x520);
                let c = i32::from(fx.mem.read_u8(r + 8 + j)) + cell;
                cell_uvs(fx, sp, c);
                draw(fx, sp, key_ra);
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// On 0xFF the beam record (D+0xD840): from node 4 (+0x10) towards the
/// target with node 4's x (+0x20 the way); drawn for 0x47 updates as one
/// 20-wide sprite age / 70 along it, each update dropping a trail record
/// (D+0xD870 + 0x60 (+8 round robin)) and a spark record (D+0xDB70 + 0x130
/// (+0xC round robin)) there [F b723 0x1E04424..0x1E04AE0].
fn beam(fx: &mut Fx, sp: u32, e: &Extra) {
    if fx.mem.i32(D) == 0xFF {
        let c = fx.mem.i32(D + 0xD834) as u32;
        let r = D + 0x30 * c + 0xD840;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        words(fx, r + 0x10, D + 0xB0);
        fx.random_table_vec(sp + 0x520, 0.0);
        fx.vu_add4(sp + 0x520, sp + 0x520, D + 0x20);
        let x = fx.mem.read_u32(r + 0x10);
        fx.mem.write_u32(sp + 0x520, x);
        fx.vu_sub4(r + 0x20, sp + 0x520, r + 0x10);
        let a = fx.mem.i32(D + 0xD830) + 1;
        fx.mem.set32(D + 0xD830, a);
        let c = fx.mem.i32(D + 0xD834) + 1;
        fx.mem.set32(D + 0xD834, c);
        if c > 0 {
            fx.mem.write_u32(D + 0xD834, 0);
        }
    }
    sprite_desc_atc(fx, sp, 0x14);
    camera(fx, sp);
    uvs(fx, sp, 0x400, 0, 0x600, 0x200);
    corners(fx, sp, 20.0, true);
    let r = D + 0xD840;
    if fx.mem.read_u8(r) == 0 {
        return;
    }
    rgba(fx, sp + 0x540, [0x78, 0x50, 0xFF], 0xFF);
    let f20 = ee::div(ee::from_i32(fx.mem.i32(r + 4)), 70.0);
    let ta = fx.mem.i32(r + 8);
    fx.mem.set32(r + 8, ta + 1);
    if ta + 1 >= 8 {
        fx.mem.write_u32(r + 8, 0);
    }
    let sa = fx.mem.i32(r + 0xC);
    fx.mem.set32(r + 0xC, sa + 1);
    if sa + 1 >= 0x10 {
        fx.mem.write_u32(r + 0xC, 0);
    }
    place(fx, sp, r + 0x20, f20, r + 0x10);
    draw(fx, sp, e.ras[5]);
    let t = D + 0x60 * ta as u32 + 0xD870;
    fx.mem.write_u8(t, 1);
    fx.mem.write_u32(t + 4, 0);
    words(fx, t + 0x10, sp + 0x520);
    for j in 0..4u32 {
        let b = rnd(fx, e.ras[6]);
        fx.mem.set8(t + 8 + j, rem2(b, 8));
        fx.random_table_vec(t + 0x20 + 0x10 * j, 0.5);
    }
    let s = D + 0x130 * sa as u32 + 0xDB70;
    fx.mem.write_u8(s, 1);
    fx.mem.write_u32(s + 4, 0);
    words(fx, s + 0x20, sp + 0x520);
    for j in 0..0x10u32 {
        let b = rnd(fx, e.ras[7]);
        let (_, h) = fx.div(b, 3);
        fx.mem.set8(s + 8 + j, h);
        fx.random_table_vec(s + 0x30 + 0x10 * j, 1.0);
    }
    let age = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, age);
    if age >= 0x47 {
        fx.mem.write_u8(r, 0);
    }
}

/// The trail records (D+0xD870, 8 x 0x60): four sprites of texture D+0x10
/// along 0.5 table vectors for 9 updates [F b723 0x1E04AE4..0x1E04EC0].
fn trails(fx: &mut Fx, sp: u32, e: &Extra) {
    sprite_desc_atc(fx, sp, 0x10);
    camera(fx, sp);
    for k in 0..8u32 {
        let t = D + 0x60 * k + 0xD870;
        if fx.mem.read_u8(t) == 0 {
            continue;
        }
        let left = ee::from_i32(8 - fx.mem.i32(t + 4));
        let s = ee::mul(10.5, ee::add(0.5, ee::div(ee::mul(0.5, left), 8.0)));
        corners(fx, sp, s, true);
        let a = ((8 - fx.mem.i32(t + 4)) << 7) >> 3;
        rgba(fx, sp + 0x540, [0x78, 0x50, 0xFF], a as u8);
        let age = fx.mem.i32(t + 4);
        let tf = fx.tan_falloff_p(age, 8, 1);
        let f20 = ee::add(f(0x3ECC_CCCD), ee::mul(f(0x3F19_999A), tf));
        for j in 0..4u32 {
            let c = i32::from(fx.mem.read_u8(t + 8 + j));
            cell_uvs(fx, sp, c);
            place(fx, sp, t + 0x20 + 0x10 * j, f20, t + 0x10);
            draw(fx, sp, e.ras[8]);
        }
        let age = fx.mem.i32(t + 4) + 1;
        fx.mem.set32(t + 4, age);
        if age >= 9 {
            fx.mem.write_u8(t, 0);
        }
    }
}

/// The beam's spark records (D+0xDB70, 16 x 0x130): 16 sprites of texture
/// D+0x14 along 1.0 table vectors for 0x11 updates [F b723
/// 0x1E04EC4..0x1E052A8].
fn beam_sparks(fx: &mut Fx, sp: u32) {
    sprite_desc_atc(fx, sp, 0x14);
    camera(fx, sp);
    for k in 0..0x10u32 {
        let s = D + 0x130 * k + 0xDB70;
        if fx.mem.read_u8(s) == 0 {
            continue;
        }
        let left = ee::from_i32(0x10 - fx.mem.i32(s + 4));
        let sz = ee::mul(5.0, ee::add(0.5, ee::div(ee::mul(0.5, left), 16.0)));
        corners(fx, sp, sz, true);
        let l = 0x10 - fx.mem.i32(s + 4);
        let a = ((l << 8) - l) >> 4;
        rgba(fx, sp + 0x540, [0x78, 0x50, 0xFF], a as u8);
        let age = fx.mem.i32(s + 4);
        let tf = fx.tan_falloff_p(age, 0x10, 1);
        let f20 = ee::add(f(0x3E4C_CCCD), ee::mul(f(0x3F4C_CCCD), tf));
        for j in 0..0x10u32 {
            let c = i32::from(fx.mem.read_u8(s + 8 + j));
            cell_uvs(fx, sp, c);
            place(fx, sp, s + 0x30 + 0x10 * j, f20, s + 0x20);
            draw(fx, sp, 0x01E0_51DC);
        }
        let age = fx.mem.i32(s + 4) + 1;
        fx.mem.set32(s + 4, age);
        if age >= 0x11 {
            fx.mem.write_u8(s, 0);
        }
    }
}
