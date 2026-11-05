//! BMODULE b739 (card 502), hand-ported from the record (linked at
//! 0x1E00000; the entry 0x1E00080 in a 0x5A0 frame with D in s0 and the OT
//! in s1, data area a0 = 0x1ED0000, 0x9900 bytes; the dome callback
//! 0x1E03210 with its static D at 0x1E03B00) [F].
//! b389's family (the same stack layout as b389 / b766: camera sp + 0x2E0,
//! the sprite descriptor sp + 0x430, the mean w sp + 0x598). The init
//! takes the target's position (D+0x30), nodes 0x1D / 0x30 / 0x39
//! (D+0x60..), lays out b440's flat 32 x 17 disc of radius 45 (D+0x2D00),
//! darkens the attacker and builds a flower of 8 rings x 16 petals
//! (D+0x7110, 2 (7 - i) / 7 apart, -70 side (i + 1) / 8 long, each turned
//! by a random z angle; cells D+0x9110). Every update the camera turn with
//! roll (sp + 0x360), the attacker's point D+0x20, the three nodes
//! (D+0x70..) and the window points of the attacker (D+0x40) and the
//! target (D+0x50). On 0xBD a lens at the target for 0x31 updates
//! (D+0x98F0, b424's fan) and three bursts (D+0xB0 + 0xEC0 k, 30 (k - 1)
//! sideways of the target: 32 puffs, 64 sparks in two colours, a 130-wide
//! halo and 128 rays of six sprites, 0x31 updates); from update 0 for
//! 0x41 updates the attacker drawn into its target with the dome (b440's
//! callback with its own settings: ramp 64 / 32, |sin| dropped, radius
//! 45, breathing 0.2, the wobble's ring step 1 / 32 turn); on 0xB0 one
//! flower of three heads at the nodes (D+0x9190, 7 updates) with 96
//! sparks (D+0x91F0, 0x11 updates); on 0x47..0x6D a white flash record
//! (D+0x9890 + 8 (n % 12), 0xD updates) each drawing the attacker
//! (0x1F8270) through b424's fan at its window point. The stage light
//! 0x80 - 104 clamp(0x12D - n) clamp(n) / 4096; returns 1 on 0xB3, 6 on
//! 0xBE..0xEC, 2 on 0xED, 8 once the count reaches 0x12E.
use crate::b182::{alpha, clamp_regs, head, rgba};
use crate::b440::{copy8, copy_z, fix, fixs, frame_mask_packet, rem2, vec4, words};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_xy, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
const PROJ_OUT: u32 = 0x002D_3600;
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
const FAR_Z: u64 = 0x00FF_FFF8_0000_0000;
const ONE: u32 = 0x3F80_0000;

const BURST_IDX: u32 = 0xA0;
const BURST: u32 = 0xB0;
const BURST_LEN: u32 = 0xEC0;
const PASS: u32 = 0x2CF0;
const DISC: u32 = 0x2D00;
const DOME_ROWS: u32 = 0x4F00;
const FLOWER_IDX: u32 = 0x7100;
const PETALS: u32 = 0x7110;
const CELLS: u32 = 0x9110;
const FLOWER: u32 = 0x9190;
const SPARK_IDX: u32 = 0x91E0;
const SPARKS: u32 = 0x91F0;
const FLASH: u32 = 0x9890;
const LENS: u32 = 0x98F0;
const D_STATIC: u32 = 0x01E0_3B00;
const CALLBACK: u32 = 0x01E0_3210;

const LENS_RA: [u32; 12] = [
    0x01E0_0948, 0x01E0_0954, 0x01E0_0998, 0x01E0_09A4, 0x01E0_09E8, 0x01E0_09F4, 0x01E0_0AA8, 0x01E0_0AB4, 0x01E0_0B20, 0x01E0_0B2C, 0x01E0_0B98,
    0x01E0_0BA4,
];
const FLASH_RA: [u32; 12] = [
    0x01E0_2D88, 0x01E0_2D94, 0x01E0_2DD8, 0x01E0_2DE4, 0x01E0_2E28, 0x01E0_2E34, 0x01E0_2EE8, 0x01E0_2EF4, 0x01E0_2F60, 0x01E0_2F6C, 0x01E0_2FDC,
    0x01E0_2FE8,
];
const DOME_RA: [u32; 6] = [0x01E0_345C, 0x01E0_360C, 0x01E0_3700, 0x01E0_3764, 0x01E0_38A4, 0x01E0_3920];

/// The record's working slots (b389's layout): the camera matrix sp +
/// 0x2E0 (model + 0x40, turn + 0x80, corners + 0xC0 / 0xD0, vertices +
/// 0xE0 / 0xF0, the placed point + 0x140), the sprite descriptor sp +
/// 0x430, the fan's triangle descriptor sp + 0x488, the petals' quad
/// descriptor sp + 0x500, the mean w sp + 0x598.
struct Fr {
    sp: u32,
    vp: u32,
    model: u32,
    turn: u32,
    c0: u32,
    c1: u32,
    v0: u32,
    pl: u32,
    d: u32,
    t: u32,
    q: u32,
    mean: u32,
}

impl Fr {
    fn new(sp: u32) -> Fr {
        let vp = sp + 0x2E0;
        Fr { sp, vp, model: vp + 0x40, turn: vp + 0x80, c0: vp + 0xC0, c1: vp + 0xD0, v0: vp + 0xE0, pl: vp + 0x140, d: sp + 0x430, t: sp + 0x488, q: sp + 0x500, mean: sp + 0x598 }
    }
}

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x5A0);
    fx.mem.write_u32(sp + 0xB0, 0);
    let f = Fr::new(sp);
    let ret = if param >= 0 { init(fx, sp) } else { step(fx, &f) };
    fx.leave(0x5A0);
    ret
}

fn rnd(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(D, ra) >> 23
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

// ------------------------------------------------------------ init

fn init(fx: &mut Fx, sp: u32) -> i32 {
    fx.bzero(D, 0x9900);
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
    fx.unit_position(t, D + 0x30);
    for (k, (node, clear)) in [(0x1D, 1), (0x30, 0), (0x39, 0)].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + 0x60 + 4 * k as u32, r);
    }
    fx.mem.write_u32(D + BURST_IDX, 0);
    for k in 0..3u32 {
        fx.mem.write_u8(D + BURST + BURST_LEN * k, 0);
    }
    fx.mem.write_u8(D + PASS, 0);
    // The flat disc: 32 rings of 17 points (0x110 a ring), radius 45.
    for i in 0..0x20i32 {
        let f20 = ee::div(ee::mul(45.0, ee::from_i32(0x20 - i)), 32.0);
        let row = D + 0x110 * i as u32 + DISC;
        for j in 0..0x11i32 {
            let a = (j << 16) >> 4;
            let sn = fx.tsin(a);
            fx.mem.setf(row + 0x10 * j as u32, ee::mul(f20, sn));
            let cs = fx.tcos(a);
            fx.mem.setf(row + 4 + 0x10 * j as u32, ee::mul(f20, cs));
            fx.mem.write_u32(row + 8 + 0x10 * j as u32, 0);
            fx.mem.write_u32(row + 0xC + 0x10 * j as u32, ONE);
        }
    }
    let c = sp + 0x59C;
    for (k, b) in [0u8, 0, 0, 0x80].into_iter().enumerate() {
        fx.mem.write_u8(c + k as u32, b);
    }
    tint(fx, 1, c, 0x01E0_0264);
    fx.mem.write_u32(D + FLOWER_IDX, 0);
    flower_init(fx, sp);
    fx.mem.write_u8(D + FLOWER, 0);
    fx.mem.write_u32(D + SPARK_IDX, 0);
    fx.mem.write_u8(D + SPARKS, 0);
    for k in 0..0xCu32 {
        fx.mem.write_u8(D + FLASH + 8 * k, 0);
    }
    fx.mem.write_u8(D + LENS, 0);
    0
}

/// The flower: ring i's petals (-1..1 wide, -70 side (i + 1) / 8 long)
/// turned by a random z angle and moved 2 (7 - i) / 7 out along the
/// petal's angle [F b739 0x1E00264..0x1E00474].
fn flower_init(fx: &mut Fx, sp: u32) {
    let (m, vv) = (sp + 0x3E0, sp + 0x420);
    for i in 0..8i32 {
        let side = ee::from_i32(fx.mem.i32(D + 0xC));
        let f21 = ee::div(ee::mul(ee::mul(-70.0, side), ee::from_i32(i + 1)), 8.0);
        let f20 = ee::div(ee::mul(2.0, ee::from_i32(7 - i)), 7.0);
        let s7 = (i << 13) >> 3;
        for j in 0..0x10i32 {
            let a = (((j << 16) >> 4).wrapping_add(s7)) & 0xFFFF;
            let sn = fx.tsin(a);
            fx.mem.setf(vv, ee::mul(f20, sn));
            let cs = fx.tcos(a);
            fx.mem.setf(vv + 4, ee::mul(f20, cs));
            fx.mem.write_u32(vv + 8, 0);
            fx.mem.write_u32(vv + 0xC, ONE);
            fx.identity(m);
            let r = rnd(fx, 0x01E0_0338);
            fx.rotate_axis(m, (rem2(r, 256) << 16) >> 8, 2);
            let q = D + 0x400 * i as u32 + 0x40 * j as u32 + PETALS;
            let (n, p, z) = (0xBF80_0000u32, ONE, f21.to_bits());
            vec4(fx, q, [n, 0, 0, ONE]);
            vec4(fx, q + 0x10, [p, 0, 0, ONE]);
            vec4(fx, q + 0x20, [n, 0, z, ONE]);
            vec4(fx, q + 0x30, [p, 0, z, ONE]);
            for k in 0..4u32 {
                fx.vu_apply_to(q + 0x10 * k, m, q + 0x10 * k);
            }
            fx.vu_add4(q + 0x20, q + 0x20, vv);
            fx.vu_add4(q + 0x30, q + 0x30, vv);
            let r = rnd(fx, 0x01E0_0400);
            fx.mem.set8(D + 0x10 * i as u32 + j as u32 + CELLS, rem2(r, 4) + 4);
        }
    }
}

// ------------------------------------------------------------ shared parts

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

fn uvs(fx: &mut Fx, f: &Fr, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, f.d + 0x18, u0);
    uv(fx, f.d + 0x1A, v0);
    uv(fx, f.d + 0x28, u1);
    uv(fx, f.d + 0x2A, v1);
}

/// The 4 x 8 cell `b` of the texture into the sprite's two UVs.
fn cell_uvs(fx: &mut Fx, f: &Fr, b: i32) {
    let (c, r) = (rem2(b, 4), b >> 2);
    let m = |x: i32| (x & 0x3FFF) as u32;
    uvs(fx, f, m(c << 9), m(r << 9), m(((c << 5) + 0x20) << 4), m(((r << 5) + 0x20) << 4));
}

fn camera(fx: &mut Fx, f: &Fr) {
    fx.camera_vp_to(f.vp);
    fx.identity(f.model);
    let id = fx.read_m4(f.model);
    fx.mul_into(Some(f.vp), &id);
}

/// The sprite descriptor rewritten (prim 0x26 | 0x800), its texture D +
/// `tex`, clamp 5 and (with `preset`) its ALPHA.
fn desc_reset(fx: &mut Fx, f: &Fr, tex: u32, preset: Option<i32>) {
    head(fx, f.d, 0x26, 5, 0x4_2421, 0x800);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(f.d, rh, 1);
    fx.desc_clamp(f.d, 5);
    if let Some(p) = preset {
        alpha(fx, f.d, p);
    }
}

/// The corners (-s, -s, 0, 1) and (s, s, 0, 1) (turned by the camera turn
/// with `turn`).
fn corners(fx: &mut Fx, f: &Fr, s: f32, turn: bool) {
    let n = ee::neg(s);
    vec4(fx, f.c0, [n.to_bits(), n.to_bits(), 0, ONE]);
    vec4(fx, f.c1, [s.to_bits(), s.to_bits(), 0, ONE]);
    if turn {
        fx.vu_apply_to(f.c0, f.turn, f.c0);
        fx.vu_apply_to(f.c1, f.turn, f.c1);
    }
}

fn colour(fx: &mut Fx, f: &Fr, c: [u8; 3], a: u8) {
    rgba(fx, f.d + 0x10, c, a);
}

/// The placed point (= dir x s + at) and the sprite's vertices around it.
fn place(fx: &mut Fx, f: &Fr, dir: u32, s: f32, at: u32) {
    fx.vu_scale4(f.pl, dir, s);
    fx.vu_add4(f.pl, f.pl, at);
    fx.vu_add4(f.v0, f.c0, f.pl);
    fx.vu_add4(f.v0 + 0x10, f.c1, f.pl);
}

fn project(fx: &mut Fx, f: &Fr, input: u32) -> bool {
    crate::fxlib2::project(fx, input, PROJ_OUT, f.vp, f.d, f.mean)
}

/// Projected from `input` and submitted at the mean w.
fn draw(fx: &mut Fx, f: &Fr, input: u32, ra: u32) {
    if project(fx, f, input) {
        let w = fx.mem.f(f.mean);
        let key = fix(fx, w, ra);
        fx.submit(f.d, OT, key as i32);
    }
}

/// Projected from `input` and submitted at far Z (key 0).
fn draw_far(fx: &mut Fx, f: &Fr, input: u32) {
    if project(fx, f, input) {
        let x = fx.mem.read_u64(f.d + 0x20);
        fx.mem.write_u64(f.d + 0x20, (x & KEEP) | FAR_Z);
        copy_z(fx, f.d + 0x30, f.d + 0x20);
        fx.submit(f.d, OT, 0);
    }
}

/// The window point of `p` (0x204D80 through the camera into sp + 0x590)
/// into `o`: x / y ((x16 >> 4) - 0x700) clamped to 0..512, Z, 1.0.
fn screen_point(fx: &mut Fx, f: &Fr, p: u32, o: u32) {
    camera(fx, f);
    let m = fx.read_m4(f.vp);
    let sp = f.sp;
    fx.project_at(p, &m, sp + 0x590, sp + 0x598);
    let x = (i32::from(fx.mem.read_u16(sp + 0x590)) >> 4) - 0x700;
    fx.mem.setf(o, ee::from_i32(x));
    let y = (i32::from(fx.mem.read_u16(sp + 0x592)) >> 4) - 0x700;
    fx.mem.setf(o + 4, ee::from_i32(y));
    let z = (fx.mem.read_u64(sp + 0x590) << 8) >> 40;
    fx.mem.setf(o + 8, ee::from_i32(z as i32));
    fx.mem.write_u32(o + 0xC, ONE);
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

// ------------------------------------------------------------ step

fn step(fx: &mut Fx, f: &Fr) -> i32 {
    let sp = f.sp;
    fx.camera_turn_roll(f.turn);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x20);
    for k in 0..3u32 {
        let h = fx.mem.i32(D + 4);
        let s = fx.mem.i32(D + 0x60 + 4 * k);
        fx.node_position(h, s, D + 0x70 + 0x10 * k);
    }
    screen_point(fx, f, D + 0x20, D + 0x40);
    screen_point(fx, f, D + 0x30, D + 0x50);
    lens(fx, f);
    if fx.mem.i32(D) == 0xBD {
        burst_spawn(fx);
    }
    bursts(fx, f);
    if fx.mem.i32(D) == 0 {
        fx.mem.write_u8(D + PASS, 1);
        fx.mem.write_u32(D + PASS + 4, 0);
    }
    tint(fx, 0, 0, 0x01E0_1C70);
    if fx.mem.read_u8(D + PASS) == 1 {
        unit_pass(fx, f);
        let t = fx.mem.i32(D + PASS + 4) + 1;
        fx.mem.set32(D + PASS + 4, t);
        if t >= 0x41 {
            fx.mem.write_u8(D + PASS, 0);
            tint(fx, 0, 0, 0x01E0_1E9C);
        }
    }
    if fx.mem.i32(D) == 0xB0 {
        flower_spawn(fx);
    }
    flowers(fx, f);
    flower_sparks(fx, f);
    let n = fx.mem.i32(D);
    if (0x47..0x6E).contains(&n) {
        let k = fx.div(n, 0xC).1 as u32;
        fx.mem.write_u8(D + FLASH + 8 * k, 1);
        fx.mem.write_u32(D + FLASH + 8 * k + 4, 0);
    }
    flashes(fx, f);
    let n = fx.mem.i32(D);
    let c1 = Fx::clamp_i(0x12D - n, 0, 0x40);
    let n = fx.mem.i32(D);
    let c2 = Fx::clamp_i(n, 0, 0x40);
    let l = fx.mult(c1, c2.wrapping_mul(104).wrapping_neg());
    fx.set_stage_light(((l >> 12) + 0x80) & 0xFF);
    let n = fx.mem.i32(D);
    let mut ret = fx.mem.i32(sp + 0xB0);
    if n == 0xB3 {
        ret = 1;
    } else {
        if (0xBE..0xED).contains(&n) {
            ret = 6;
        }
        if n == 0xED {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x12E {
        ret = 8;
    }
    fx.mem.set32(sp + 0xB0, ret);
    ret
}

/// The lens at the target's window point (D+0x98F0 for 0x31 updates) [F
/// b739 0x1E0076C..0x1E00C94].
fn lens(fx: &mut Fx, f: &Fr) {
    if fx.mem.i32(D) == 0xBD {
        fx.mem.write_u8(D + LENS, 1);
        fx.mem.write_u32(D + LENS + 4, 0);
    }
    if fx.mem.read_u8(D + LENS) != 1 {
        return;
    }
    let (sp, t) = (f.sp, f.t);
    head(fx, t, 0x33, 9, 0x4_2142_1421, 0x800);
    alpha(fx, t, 0);
    fx.desc_clamp(t, LENS_CLAMP);
    let a = clamp_regs(fx, t, LENS_CLAMP);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xDC, layer);
    let regs = crate::lens::frame_copy(fx, a, 1, sp + 0x4E8, sp + 0x4F0);
    fx.texflush(regs);
    let age = fx.mem.i32(D + LENS + 4);
    let al = div48(fx, (0x30 - age) << 6);
    rgba(fx, t + 0x10, [0x80; 3], al as u8);
    copy8(fx, t + 0x28, t + 0x10);
    copy8(fx, t + 0x40, t + 0x10);
    for j in 0..8i32 {
        let s = ee::add(1.0, ee::div(ee::mul(f32::from_bits(0x3E19_999A), ee::from_i32(8 - j)), 8.0));
        crate::b424::fan(fx, t, D + 0x50, sp + 0x3C0, s, 0, &LENS_RA);
    }
    let layer = fx.mem.read_u32(sp + 0xDC);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + LENS + 4) + 1;
    fx.mem.set32(D + LENS + 4, age);
    if age >= 0x31 {
        fx.mem.write_u8(D + LENS, 0);
    }
}

/// The three burst records (D+0xB0 + 0xEC0 k) at the target, 30 (k - 1)
/// along x [F b739 0x1E00C94..0x1E00F2C]: point +0x140, 64 spark cells
/// (+0xB8, halfwords) and 2.0 vectors (+0x150), 128 ray vectors 2.5
/// (+0x550), 32 puff cells (+0xD50, rand % 3 + 5) and 2.0 vectors (+0xD70).
fn burst_spawn(fx: &mut Fx) {
    for k in 0..3u32 {
        fx.mem.write_u32(D + BURST_IDX, k);
        let idx = fx.mem.i32(D + BURST_IDX);
        let r = D + BURST_LEN * idx as u32;
        fx.mem.write_u8(r + BURST, 1);
        fx.mem.write_u32(r + 0xB4, 1);
        fx.mem.setf(r + 0x140, ee::mul(30.0, ee::from_i32(idx - 1)));
        fx.mem.write_u32(r + 0x144, 0);
        fx.mem.write_u32(r + 0x148, 0);
        fx.mem.write_u32(r + 0x14C, ONE);
        fx.vu_add4(r + 0x140, r + 0x140, D + 0x30);
        for j in 0..0x40u32 {
            let c = rnd(fx, 0x01E0_0DD0);
            fx.mem.write_u16(r + 0xB8 + 2 * j, rem2(c, 8) as u16);
            fx.random_table_vec(r + 0x150 + 0x10 * j, 2.0);
        }
        for m in 0..0x80u32 {
            fx.random_table_vec(r + 0x550 + 0x10 * m, 2.5);
        }
        for j in 0..0x20u32 {
            let c = rnd(fx, 0x01E0_0EA8);
            let (_, h) = fx.div(c, 3);
            fx.mem.set8(r + 0xD50 + j, h + 5);
            fx.random_table_vec(r + 0xD70 + 0x10 * j, 2.0);
        }
    }
}

/// `size` x (0.8 + 0.2 age / 48).
fn burst_size(fx: &mut Fx, r: u32, size: f32) -> f32 {
    let age = ee::from_i32(fx.mem.i32(r + 0xB4));
    let s = ee::div(ee::mul(f32::from_bits(0x3E4C_CCCD), age), 48.0);
    ee::mul(size, ee::add(f32::from_bits(0x3F4C_CCCD), s))
}

/// The bursts [F b739 0x1E00F30..0x1E01C44].
fn bursts(fx: &mut Fx, f: &Fr) {
    for k in 0..3u32 {
        let r = D + BURST_LEN * k;
        if fx.mem.read_u8(r + BURST) == 0 {
            continue;
        }
        let pt = r + 0x140;
        // Puffs.
        desc_reset(fx, f, 0x18, Some(1));
        camera(fx, f);
        let s = burst_size(fx, r, 12.0);
        corners(fx, f, s, true);
        colour(fx, f, [0x50, 0xE4, 0xFF], 0);
        let age = fx.mem.i32(r + 0xB4);
        let f22 = fx.tan_falloff_p(age, 0x30, 1);
        for j in 0..0x20u32 {
            place(fx, f, r + 0xD70 + 0x10 * j, f22, pt);
            let c = i32::from(fx.mem.read_u8(r + 0xD50 + j));
            cell_uvs(fx, f, c);
            draw_far(fx, f, f.v0);
        }
        // Sparks in two colours.
        desc_reset(fx, f, 0x10, None);
        camera(fx, f);
        let s = burst_size(fx, r, 10.0);
        corners(fx, f, s, true);
        for j in 0..0x40u32 {
            let age = fx.mem.i32(r + 0xB4);
            let mut f22 = fx.tan_falloff_p(age, 0x30, 1);
            if j % 2 == 0 {
                f22 = ee::mul(f22, f32::from_bits(0x3F66_6666));
                alpha(fx, f.d, 2);
                colour(fx, f, [0x80, 0xFF, 0xFF], 0);
            } else {
                alpha(fx, f.d, 1);
                colour(fx, f, [0x1E, 0x60, 0xFF], 0);
            }
            place(fx, f, r + 0x150 + 0x10 * j, f22, pt);
            let c = i32::from(fx.mem.read_u16(r + 0xB8 + 2 * j));
            cell_uvs(fx, f, c);
            draw_far(fx, f, f.v0);
        }
        // The halo: the camera at the point scaled by 0.2 + 0.8 falloff.
        desc_reset(fx, f, 0x14, Some(1));
        fx.camera_vp_to(f.vp);
        fx.translate_rows(f.model, f.turn, pt);
        let age = fx.mem.i32(r + 0xB4);
        let tf = fx.tan_falloff_p(age, 0x30, 1);
        let s = ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), tf));
        vec4(fx, f.pl, [s.to_bits(), s.to_bits(), s.to_bits(), ONE]);
        fx.scale_rows(f.model, f.pl);
        let mm = fx.read_m4(f.model);
        fx.mul_into(Some(f.vp), &mm);
        colour(fx, f, [0xFF, 0x5E, 0x3A], 0);
        uvs(fx, f, 0x600, 0, 0x800, 0x200);
        corners(fx, f, 130.0, false);
        draw_far(fx, f, f.c0);
        // The rays: 128 of six sprites, alpha 255 (s + 6) / 6 (0x30 - age) / 48.
        desc_reset(fx, f, 0x14, Some(1));
        uvs(fx, f, 0x600, 0, 0x800, 0x200);
        camera(fx, f);
        corners(fx, f, 1.0, true);
        let f23 = ee::div(ee::from_i32(0x30 - fx.mem.i32(r + 0xB4)), 48.0);
        let age = fx.mem.i32(r + 0xB4);
        let f22 = fx.tan_falloff_p(age, 0x30, 1);
        for m in 0..0x80u32 {
            let dir = r + 0x550 + 0x10 * m;
            for s6 in 0..6i32 {
                let x = (s6 + 6).wrapping_mul(255);
                fx.mult(0x2AAA_AAAB, x);
                let q = (fx.hilo().1 >> 1).wrapping_add(((x as u32) >> 31) as i32);
                let a = fix(fx, ee::mul(f23, ee::from_i32(q)), 0x01E0_1AEC);
                colour(fx, f, [0x80, 0xF0, 0xFF], a as u8);
                let f12 = ee::add(f22, ee::mul(f32::from_bits(0x3C23_D70A), ee::from_i32(s6)));
                place(fx, f, dir, f12, pt);
                draw(fx, f, f.v0, 0x01E0_1BA4);
            }
        }
        let age = fx.mem.i32(r + 0xB4) + 1;
        fx.mem.set32(r + 0xB4, age);
        if age >= 0x31 {
            fx.mem.write_u8(r + BURST, 0);
        }
    }
}

/// The darkened attacker drawn into its target with the dome, composited
/// by a full-window sprite (b389's unit pass) [F b739 0x1E01C80..0x1E01E68].
fn unit_pass(fx: &mut Fx, f: &Fr) {
    let c = f.mean + 4;
    for (k, b) in [0u8, 0, 0, 0x80].into_iter().enumerate() {
        fx.mem.write_u8(c + k as u32, b);
    }
    tint(fx, 1, c, 0x01E0_1CA0);
    let d = f.d;
    head(fx, d, 0x26, 5, 0x4_2421, 0);
    let layer = fx.layer_begin();
    // 0x1F7600 leaves a0 = a1 = 0, a2 = the colour, a3 = the object.
    let h = fx.mem.i32(D + 4);
    let i = fx.object_index(h >> 8, h & 0xFF);
    let o = fx.object_at(i);
    fx.texflush([0, 0, c, o]);
    fx.mem.write_u32(D_STATIC, D);
    let (_, regs) = fx.unit_draw_post(1, d + 0x40, d + 0x48, CALLBACK, &mut dome);
    fx.texflush(regs);
    alpha(fx, d, 0);
    fx.desc_clamp(d, LENS_CLAMP);
    let x = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, x | 0x800);
    rgba(fx, d + 0x10, [0x80; 3], 0x80);
    uvs(fx, f, 0, 0, 0x2000, 0x2000);
    far_xy(fx, d + 0x20, 0x6FF8, 0x6FF8);
    far_xy(fx, d + 0x30, 0x8FF8, 0x8FF8);
    fx.submit(d, 0, 0);
    let w = crate::blib::afterimage::depth_w(fx, D + 0x20, 0.0);
    fx.mem.setf(f.mean, w);
    let key = fix(fx, w, 0x01E0_1E5C);
    fx.layer_end(layer, OT, key as i32);
}

/// On 0xB0 the flower (D+0x9190: three heads at the nodes, each turned by
/// a random z angle) and its sparks (D+0x91F0: 32 cells and 1.0 vectors a
/// node) [F b739 0x1E01EAC..0x1E020C4].
fn flower_spawn(fx: &mut Fx) {
    let k = fx.mem.i32(D + FLOWER_IDX) as u32;
    let fb = D + 0x50 * k + FLOWER;
    fx.mem.write_u8(fb, 1);
    fx.mem.write_u32(fb + 4, 0);
    let m = fx.mem.i32(D + SPARK_IDX) as u32;
    let tb = D + 0x6A0 * m + SPARKS;
    fx.mem.write_u8(tb, 1);
    fx.mem.write_u32(tb + 4, 0);
    for s in 0..3u32 {
        let r = rnd(fx, 0x01E0_1F30);
        fx.mem.set32(fb + 8 + 4 * s, (rem2(r, 256) << 16) >> 8);
        words(fx, fb + 0x20 + 0x10 * s, D + 0x70 + 0x10 * s);
        words(fx, tb + 0x70 + 0x10 * s, D + 0x70 + 0x10 * s);
        for j in 0..0x20u32 {
            let r = rnd(fx, 0x01E0_1FD0);
            fx.mem.set8(tb + 8 + 0x20 * s + j, rem2(r, 8));
            fx.random_table_vec(tb + 0xA0 + 0x200 * s + 0x10 * j, 1.0);
        }
    }
    let k = fx.mem.i32(D + FLOWER_IDX) + 1;
    fx.mem.set32(D + FLOWER_IDX, k);
    if k > 0 {
        fx.mem.write_u32(D + FLOWER_IDX, 0);
    }
    let m = fx.mem.i32(D + SPARK_IDX) + 1;
    fx.mem.set32(D + SPARK_IDX, m);
    if m > 0 {
        fx.mem.write_u32(D + SPARK_IDX, 0);
    }
}

/// The flower [F b739 0x1E020C8..0x1E026FC]: three heads of 8 x 16 petals
/// (alpha 0x56 (6 - age) / 6, (30, 40, FF), 0.5 + age / 12 wide), then a
/// 45-wide far sprite (FF, 4C, 14) at each node, 7 updates.
fn flowers(fx: &mut Fx, f: &Fr) {
    let q = f.q;
    head(fx, q, 0x34, 0xC, 0x4214_2142_1421, 0);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(q, rh, 1);
    alpha(fx, q, 1);
    fx.desc_clamp(q, 0);
    let fb = D + FLOWER;
    if fx.mem.read_u8(fb) == 0 {
        return;
    }
    let left = 6 - fx.mem.i32(fb + 4);
    let x = fx.mult(left, 0x56);
    fx.mult(0x2AAA_AAAB, x);
    let a = fx.hilo().1.wrapping_add(((x as u32) >> 31) as i32);
    rgba(fx, q + 0x10, [0x30, 0x40, 0xFF], a as u8);
    for o in [0x28u32, 0x40, 0x58] {
        copy8(fx, q + o, q + 0x10);
    }
    let age = ee::from_i32(fx.mem.i32(fb + 4));
    let s = ee::add(0.5, ee::div(ee::mul(0.5, age), 6.0));
    vec4(fx, f.pl, [s.to_bits(), s.to_bits(), s.to_bits(), ONE]);
    for s in 0..3u32 {
        fx.camera_vp_to(f.vp);
        fx.identity(f.model);
        fx.translate_rows(f.model, f.model, fb + 0x20 + 0x10 * s);
        fx.rotate_axis(f.model, 0, 0);
        let ang = fx.mem.i32(fb + 8 + 4 * s);
        fx.rotate_axis(f.model, ang, 2);
        fx.scale_rows(f.model, f.pl);
        let mm = fx.read_m4(f.model);
        fx.mul_into(Some(f.vp), &mm);
        petals(fx, f);
    }
    desc_reset(fx, f, 0x14, Some(1));
    camera(fx, f);
    corners(fx, f, 45.0, true);
    let age = fx.mem.i32(fb + 4);
    fx.tan_falloff_p(age, 6, 1);
    let a = fix(fx, 0.0, 0x01E0_24F0);
    colour(fx, f, [0xFF, 0x4C, 0x14], a as u8);
    for s in 0..3u32 {
        words(fx, f.pl, fb + 0x20 + 0x10 * s);
        fx.vu_add4(f.v0, f.c0, f.pl);
        fx.vu_add4(f.v0 + 0x10, f.c1, f.pl);
        uvs(fx, f, 0x600, 0, 0x800, 0x200);
        draw_far(fx, f, f.v0);
    }
    let age = fx.mem.i32(fb + 4) + 1;
    fx.mem.set32(fb + 4, age);
    if age >= 7 {
        fx.mem.write_u8(fb, 0);
    }
}

/// The 8 x 16 petals through the camera matrix, keyed by their mean w.
fn petals(fx: &mut Fx, f: &Fr) {
    let q = f.q;
    let mm = fx.read_m4(f.vp);
    for i in 0..8u32 {
        for j in 0..0x10u32 {
            let cb = i32::from(fx.mem.read_u8(D + 0x10 * i + j + CELLS));
            let c = rem2(cb, 4) << 5;
            let r = (cb >> 2) << 5;
            let f4 = ee::from_i32(c);
            let f2 = ee::from_i32(r);
            let f3 = ee::div(ee::add(32.0, f4), 128.0);
            let f4 = ee::div(f4, 128.0);
            let f5 = ee::div(f2, 64.0);
            let f0 = ee::div(ee::add(16.0, f2), 64.0);
            for (o, x) in [(0x18u32, f4), (0x1C, f0), (0x30, f3), (0x34, f0), (0x48, f4), (0x4C, f5), (0x60, f3), (0x64, f5)] {
                fx.mem.setf(q + o, x);
            }
            let petal = D + 0x400 * i + 0x40 * j + PETALS;
            if fx.project_desc(petal, PROJ_OUT, Some(&mm), q, f.mean) {
                let w = fx.mem.f(f.mean);
                let key = fix(fx, w, 0x01E0_2390);
                fx.submit(q, OT, key as i32);
            }
        }
    }
}

/// The flower's 96 sparks: 10 x (0.5 + age / 32) wide, grey BC, 0x11
/// updates [F b739 0x1E026FC..0x1E02A7C].
fn flower_sparks(fx: &mut Fx, f: &Fr) {
    desc_reset(fx, f, 0x10, Some(0));
    camera(fx, f);
    let tb = D + SPARKS;
    if fx.mem.read_u8(tb) == 0 {
        return;
    }
    let age = ee::from_i32(fx.mem.i32(tb + 4));
    let s = ee::mul(10.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 16.0)));
    corners(fx, f, s, true);
    colour(fx, f, [0xBC; 3], 0);
    let age = fx.mem.i32(tb + 4);
    let f20 = fx.tan_falloff_p(age, 0x10, 1);
    for s in 0..3u32 {
        for j in 0..0x20u32 {
            place(fx, f, tb + 0xA0 + 0x200 * s + 0x10 * j, f20, tb + 0x70 + 0x10 * s);
            let c = i32::from(fx.mem.read_u8(tb + 8 + 0x20 * s + j));
            cell_uvs(fx, f, c);
            draw(fx, f, f.v0, 0x01E0_29AC);
        }
    }
    let age = fx.mem.i32(tb + 4) + 1;
    fx.mem.set32(tb + 4, age);
    if age >= 0x11 {
        fx.mem.write_u8(tb, 0);
    }
}

/// The white flash records (D+0x9890, 12 x 8): the first live one opens a
/// layer with the attacker drawn (0x1F8270); each draws b424's fan at the
/// attacker's window point, (64, 30, FF) at alpha f 128 sin((age + 1) / 13
/// half turn), f = clamp(n - 0x46, 0, 16) clamp(0x6E - n, 0, 16) / 256,
/// 1 + 0.4 age / 12 - 0.2 + 1 / 60 wide, 0xD updates [F b739
/// 0x1E02A7C..0x1E030FC].
fn flashes(fx: &mut Fx, f: &Fr) {
    let (sp, t) = (f.sp, f.t);
    head(fx, t, 0x33, 9, 0x4_2142_1421, 0x800);
    alpha(fx, t, 1);
    fx.desc_clamp(t, LENS_CLAMP);
    let n = fx.mem.i32(D);
    let c1 = Fx::clamp_i(n - 0x46, 0, 0x10);
    let n = fx.mem.i32(D);
    let c2 = Fx::clamp_i(0x6E - n, 0, 0x10);
    let f20 = ee::div(ee::mul(ee::from_i32(c1), ee::from_i32(c2)), 256.0);
    let mut layer = None;
    for k in 0..0xCu32 {
        let r = D + FLASH + 8 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        if layer.is_none() {
            let l = fx.layer_begin();
            fx.mem.write_u32(sp + 0xDC, l);
            layer = Some(l);
            // 0x101050 leaves a0 = its answer, a1 = 0, a2 = 0x10; a3 = t + 2
            // from 0x208400.
            fx.texflush([c2 as u32, 0, 0x10, t + 2]);
            let h = fx.mem.i32(D + 4);
            let (_, regs) = fx.unit_draw(h, 1, sp + 0x4E8, sp + 0x4F0);
            fx.texflush(regs);
        }
        for (o, ra) in [(0x10u32, 0x01E0_2C60), (0x28, 0x01E0_2CDC)] {
            let x = (fx.mem.i32(r + 4) + 1) << 15;
            fx.mult(0x4EC4_EC4F, x);
            let q = (fx.hilo().1 >> 2).wrapping_add(((x as u32) >> 31) as i32);
            let sn = fx.tsin(q);
            let a = fix(fx, ee::mul(f20, ee::mul(128.0, sn)), ra);
            rgba(fx, t + o, [0x64, 0x30, 0xFF], a as u8);
        }
        copy8(fx, t + 0x40, t + 0x28);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let g = ee::div(ee::mul(f32::from_bits(0x3ECC_CCCD), age), 12.0);
        let g = ee::add(1.0, ee::add(f32::from_bits(0xBE4C_CCCD), g));
        let f21 = ee::add(f32::from_bits(0x3C88_8889), g);
        crate::b424::fan(fx, t, D + 0x40, sp + 0x3C0, f21, 0, &FLASH_RA);
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0xD {
            fx.mem.write_u8(r, 0);
        }
    }
    if layer.is_some() {
        let l = fx.mem.read_u32(sp + 0xDC);
        fx.layer_end(l, OT, 0);
    }
}

// ------------------------------------------------------------ the dome

/// The dome callback 0x1E03210(p) [F]: b440::dome with this record's
/// settings (static D 0x1E03B00, the camera at D+0x20, the pass timer
/// D+0x2CF4, dome rows D+0x4F00 on the disc D+0x2D00, ramp above 64 / 32,
/// |sin| taken and dropped, alpha 255, radius 45, wobble 5461) and two
/// constants of its own: the breathing 0.2 (b440's 0.3) and the wobble's
/// ring step (i << 16) >> 5 (b440's (i << 14) >> 5).
fn dome(fx: &mut Fx, p: u32) -> u32 {
    let sp = fx.enter(0x320);
    let d = fx.mem.read_u32(D_STATIC);
    fx.mem.write_u32(sp + 0xB0, d);
    let s0 = frame_mask_packet(fx, p, 0xFF00_0000, [0, 0]);
    fx.camera_turn_roll(sp + 0x1B0);
    fx.camera_vp_to(sp + 0x130);
    words(fx, sp + 0x1F0, d + 0x20);
    fx.translate_rows(sp + 0x170, sp + 0x1B0, sp + 0x1F0);
    let m = fx.read_m4(sp + 0x170);
    fx.mul_into(Some(sp + 0x130), &m);
    let (ta, qb) = (sp + 0x208, sp + 0x280);
    fx.mem.write_u16(ta + 2, 0x33);
    fx.mem.write_u16(ta, 9);
    fx.mem.write_u64(ta + 8, 0x4_2142_1421);
    fx.mem.write_u32(ta + 4, 0);
    fx.mem.write_u16(qb + 2, 0x34);
    fx.mem.write_u16(qb, 0xC);
    fx.mem.write_u64(qb + 8, 0x4214_2142_1421);
    fx.mem.write_u32(qb + 4, 0);
    // 0x188420 leaves a3 = the turn's first word.
    let a3 = fx.mem.read_u32(sp + 0x1B0);
    fx.boundary(0x0020_EA20, "packet writer 0x20EA20", [s0, 0x2142_1421, ta, a3]);
    let p1 = fx.texflush_packet(s0);
    let p2 = fx.frame_texture(p1, -1, 1, sp + 0x268, sp + 0x270);
    let t = fx.mem.read_u64(sp + 0x268);
    fx.mem.write_u64(sp + 0x2F8, t);
    let t = fx.mem.read_u64(sp + 0x270);
    fx.mem.write_u64(sp + 0x300, t);
    let a1 = fx.mem.read_u32(sp + 0x268);
    fx.boundary(0x0020_EA20, "packet writer 0x20EA20", [p2, a1, sp + 0x2F8, 0]);
    let mut s5 = fx.texflush_packet(p2);
    for dd in [ta, qb] {
        let a = fx.alpha_preset(0);
        fx.desc_alpha(dd, a);
        fx.desc_clamp(dd, 0x0000_0800_0080_000A);
        let fl = fx.mem.read_u16(dd + 2);
        fx.mem.write_u16(dd + 2, fl | 0x800);
    }
    let t = fx.mem.i32(d + PASS + 4);
    let f20 = Fx::ramp_above(ee::from_i32(t), 64.0, 32.0);
    let a = ((rem2(t, 0x40) << 16) >> 6) & 0xFFFF;
    let s = fx.tsin(a);
    fx.mem.write_f32(fx.sp - 0x10, s);
    let f1 = ee::mul(f20, 1.0);
    for k in 0..3 {
        fx.mem.write_u8(sp + 0x218 + k, 0xFF);
    }
    let alpha = fix(fx, ee::mul(255.0, f1), DOME_RA[0]);
    fx.mem.write_u8(sp + 0x21B, alpha as u8);
    fx.mem.write_u32(sp + 0x21C, ONE);
    for to in [0x230, 0x248, 0x310, 0x290, 0x2A8, 0x2C0, 0x2D8] {
        copy8(fx, sp + to, sp + 0x218);
    }
    // The dome rebuilt: 32 rings of 17 points.
    let fp = fx.mult(t, 0xD3);
    let t111 = t.wrapping_mul(0x6F);
    for i in 0..0x20i32 {
        let f21 = ee::div(ee::mul(45.0, ee::from_i32(0x20 - i)), 32.0);
        let f20 = ee::div(ee::from_i32(i + 1), 32.0);
        let s7 = (i << 17) >> 5;
        let s4 = (((i << 16) >> 5).wrapping_add(t111)) & 0xFFFF;
        let row = d + 0x110 * i as u32 + DOME_ROWS;
        for j in 0..0x11i32 {
            let a = (s7 + ((j << 17) >> 4)).wrapping_add(fp) & 0xFFFF;
            let c = fx.tcos(a);
            let f22 = ee::add(1.0, ee::mul(f20, ee::mul(f32::from_bits(0x3E4C_CCCD), c)));
            let c2 = fx.tcos(s4);
            let f12 = ee::add(ee::from_i32((j << 16) >> 4), ee::mul(f20, ee::mul(5461.0, c2)));
            let s6 = fixs(fx, f12, DOME_RA[1]) & 0xFFFF;
            let sn = fx.tsin(s6);
            let f22 = ee::mul(f21, f22);
            fx.mem.setf(row + 0x10 * j as u32, ee::mul(f22, sn));
            let cs = fx.tcos(s6);
            fx.mem.setf(row + 4 + 0x10 * j as u32, ee::mul(f22, cs));
            fx.mem.write_u32(row + 8 + 0x10 * j as u32, 0);
            fx.mem.write_u32(row + 0xC + 0x10 * j as u32, ONE);
        }
    }
    vec4(fx, sp + 0x1F0, [0, 0, 0, ONE]);
    let mm = sp + 0x130;
    let (dm, dc) = (DOME_ROWS, DISC);
    for k in 0..0x20u32 {
        let fpr = d + 0x110 * k;
        if k == 0x1F {
            for s0 in 0..0x10u32 {
                let (s1, s3) = (fpr + 0x10 * s0, fpr + 0x10 * (s0 + 1));
                if fx.project_n(&[sp + 0x1F0, s1 + dm, s3 + dm], mm, ta, sp + 0x31C, DOME_RA[2]) != 1 {
                    continue;
                }
                for (u, x) in [(0x220, 0x228), (0x238, 0x240), (0x250, 0x258)] {
                    fx.uv_from_xy(sp + u, sp + x, 0, 0);
                }
                if fx.project_n(&[sp + 0x1F0, s1 + dc, s3 + dc], mm, ta, sp + 0x31C, DOME_RA[3]) != 1 {
                    continue;
                }
                let x = fx.mem.read_u64(sp + 0x228);
                fx.mem.write_u64(sp + 0x228, (x & KEEP) | FAR_Z);
                copy_z(fx, sp + 0x240, sp + 0x228);
                copy_z(fx, sp + 0x258, sp + 0x228);
                s5 = fx.submit_at(s5, ta, 0, 0);
            }
        } else {
            let nrow = d + 0x110 * (k + 1);
            for s6 in 0..0x10u32 {
                let (s1, s4) = (fpr + 0x10 * s6, fpr + 0x10 * (s6 + 1));
                let (s0, s3) = (nrow + 0x10 * s6, nrow + 0x10 * (s6 + 1));
                if fx.project_n(&[s1 + dm, s4 + dm, s0 + dm, s3 + dm], mm, qb, sp + 0x31C, DOME_RA[4]) != 1 {
                    continue;
                }
                for (u, x) in [(0x298, 0x2A0), (0x2B0, 0x2B8), (0x2C8, 0x2D0), (0x2E0, 0x2E8)] {
                    fx.uv_from_xy(sp + u, sp + x, 0, 0);
                }
                if fx.project_n(&[s1 + dc, s4 + dc, s0 + dc, s3 + dc], mm, qb, sp + 0x31C, DOME_RA[5]) != 1 {
                    continue;
                }
                let x = fx.mem.read_u64(sp + 0x2A0);
                fx.mem.write_u64(sp + 0x2A0, (x & KEEP) | FAR_Z);
                for to in [0x2B8, 0x2D0, 0x2E8] {
                    copy_z(fx, sp + to, sp + 0x2A0);
                }
                s5 = fx.submit_at(s5, qb, 0, 0);
            }
        }
    }
    let r = frame_mask_packet(fx, s5, 0, [0, 0]);
    fx.leave(0x320);
    r
}
