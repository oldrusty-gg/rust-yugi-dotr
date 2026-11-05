//! BMODULE b244 (card 327), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02B30, data area a0 = 0x1ED0000) [F] (helper C8,
//! 31 Oct 2026): b103's parts six times over. Six points (+0xB30 + 0x70
//! k) sit at the attacker's node 0x2F until their update +0x4788 (0x1E +
//! 0x19 k), then close on the target (+0x1130 + 0x70 k) by 1 / 30 a step
//! trailing a streamer each (+0x70 + 0x40 k, points from the pointer
//! +0x1F0 + 4 k); from one update before that a disc (+0xDB0 + 0x10 k) and
//! a facing quad (+0xE10 + 0x60 k / +0x1050 + 0x20 k) at the point
//! (billboard +0xB10 + 0x70 k, growing to 5 by 0.2 from +0x47A0), and
//! sparks (+0x1650 + 0x30 k) turned to look at the target, respawning
//! above 30; a frame quad (0x80, 0x80, 0x80, 0x60) on 0x1E..0xB8; from
//! 0xB9 a full-window tint (+0x47C8). Returns 1 on update 0x32, 6 on
//! 0x3C..0xB8, 2 on 0xB9, 8 after 0xCA.
//!
//! Library (record addresses): `colours4` 0x1E00080, `frame_quad_rgba`
//! 0x1E00120, `tint` 0x1E00240, `disc` 0x1E003A0, `facing::quad`
//! 0x1E00900, `scatter` 0x1E01190, `trail::sparks` 0x1E01350,
//! `streamer::push` / `draw` 0x1E020B0 / 0x1E02190.
use crate::b262::tint;
use crate::blib::{facing, spawn, streamer, trail};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const STREAMER: u32 = 0x01E0_2190;
const N: u32 = D + 0x47BC;
const LEFT: u32 = D + 0x47B8;
/// Per point k (4 k): steps to the target, the update it leaves, the
/// update its quad starts to grow.
const STEPS: u32 = D + 0x4770;
const LEAVE: u32 = D + 0x4788;
const GROW: u32 = D + 0x47A0;
const NODE: u32 = D + 0x47C4;

/// The entry 0x1E02B30 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x60);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x60);
    ret
}

fn bytes(fx: &mut Fx, at: &[(u32, u8)]) {
    for &(o, v) in at {
        fx.mem.write_u8(D + o, v);
    }
}

fn halves(fx: &mut Fx, at: &[(u32, i32)]) {
    for &(o, v) in at {
        fx.mem.set16(D + o, v);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x47D0);
    fx.xform_init(D);
    for k in 0..6u32 {
        for x in [0xB10u32, 0x1110, 0x13B0] {
            fx.xform_init(D + x + 0x70 * k);
        }
    }
    let h = fx.self_handle();
    let r = fx.request_node(h, 0x2F, 1);
    fx.mem.set32(NODE, r);
    fx.mem.set32(LEFT, 0xCA);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x47C0, param);
    bytes(fx, &[(0x47CA, 0xE0), (0x47C9, 0xB0), (0x47C8, 0x70), (0x47CB, 0xFF)]);
    for k in 0..6u32 {
        fx.mem.set32(STEPS + 4 * k, 0x1E);
        fx.mem.set32(LEAVE + 4 * k, 0x1E + 0x19 * k as i32);
        fx.mem.set32(GROW + 4 * k, 0x3C + 0x19 * k as i32);
    }
    // Discs (+0xDB0 + 0x10 k): radius 4.
    for k in 0..6u32 {
        let g = 0xDB0 + 0x10 * k;
        fx.mem.write_u32(D + g, 0);
        bytes(fx, &[(g + 6, 0xE0), (g + 5, 0xB0), (g + 4, 0x70), (g + 7, 0xFF)]);
        fx.mem.write_u32(D + g + 8, 0x4080_0000);
        halves(fx, &[(g + 0xC, 0x24), (g + 0xE, 0)]);
    }
    // Facing quads (+0xE10 + 0x60 k, quads +0x1050 + 0x20 k), texture 5 / 0.
    fx.texture5(sp + 0x50, 0);
    for k in 0..6u32 {
        let g = 0xE10 + 0x60 * k;
        fx.mem.write_u32(D + g + 0x38, 3);
        halves(fx, &[(g, 0), (g + 2, 0), (g + 4, 0x40), (g + 6, 0x40), (g + 8, 0), (g + 0xA, 0), (g + 0xC, 0x80), (g + 0xE, 0x80)]);
        fx.mem.write_u32(D + g + 0x30, 0x3F80_0000);
        bytes(fx, &[(g + 0x36, 0x80), (g + 0x35, 0x80), (g + 0x34, 0x80), (g + 0x37, 0x80)]);
        halves(fx, &[(g + 0x48, 0), (g + 0x4A, 2), (g + 0x4C, 3), (g + 0x4E, 4), (g + 0x50, 0), (g + 0x52, 0x3E7)]);
        let tex = fx.mem.read_u32(sp + 0x50);
        let q = 0x1050 + 0x20 * k;
        for o in [0x18u32, 0x14, 0x10] {
            fx.mem.write_u32(D + q + o, 0);
        }
        fx.mem.write_u32(D + q + 8, 0x4040_0000);
        fx.mem.write_u32(D + q, tex);
        bytes(fx, &[(q + 0xE, 0x80), (q + 0xD, 0x80), (q + 0xC, 0x80), (q + 0xF, 0x80)]);
    }
    // Streamers (+0x70 + 0x40 k, points +0x210 + 0x180 k), texture 5 / 1.
    fx.texture5(sp + 0x50, 1);
    for k in 0..6u32 {
        fx.mem.write_u32(D + 0x1F0 + 4 * k, D + 0x210 + 0x180 * k);
    }
    for k in 0..6u32 {
        let g = 0x70 + 0x40 * k;
        let tex = fx.mem.read_u32(sp + 0x50);
        fx.mem.write_u32(D + g, tex);
        bytes(fx, &[(g + 6, 0x80), (g + 5, 0x80), (g + 4, 0x80), (g + 7, 0x80)]);
        fx.mem.write_u32(D + g + 8, 0x8003);
        fx.mem.write_u32(D + g + 0xC, 0x41A0_0000);
        halves(fx, &[(g + 0x10, 0x60), (g + 0x12, 0), (g + 0x14, 0x20), (g + 0x16, 0x20), (g + 0x18, 0x60), (g + 0x1A, 0), (g + 0x1C, 0x80), (g + 0x1E, 0x40)]);
        fx.mem.write_u32(D + g + 0x30, 0x3D4C_CCCD);
        halves(fx, &[(g + 0x34, 0), (g + 0x36, 0x10), (g + 0x38, 0x18), (g + 0x3A, 0), (g + 0x3C, 1)]);
    }
    // Spark clouds (+0x1650 + 0x30 k, particles +0x1770 + 0x800 k).
    for k in 0..6u32 {
        fx.mem.write_u32(D + 0x165C + 0x30 * k, D + 0x1770 + 0x800 * k);
    }
    for k in 0..6u32 {
        let g = 0x1650 + 0x30 * k;
        let tex = fx.mem.read_u32(sp + 0x50);
        fx.mem.write_u32(D + g, tex);
        fx.mem.write_u32(D + g + 4, 0x2000_0003);
        bytes(fx, &[(g + 0xA, 0x80), (g + 9, 0x80), (g + 8, 0x80), (g + 0xB, 0x80)]);
        fx.mem.write_u32(D + g + 0x20, 0x3E99_999A);
        fx.mem.write_u32(D + g + 0x24, 0x4120_0000);
        halves(fx, &[(g + 0x10, 0), (g + 0x12, 0), (g + 0x14, 0x20), (g + 0x16, 0x20), (g + 0x18, 0), (g + 0x1A, 0), (g + 0x1C, 0x80), (g + 0x1E, 0x40)]);
        fx.mem.write_u32(D + g + 0x28, 0x40A0_0000);
        halves(fx, &[(g + 0x2C, 0x80)]);
        bytes(fx, &[(g + 0x2E, 8), (g + 0x2F, 0)]);
        spawn::scatter(fx, D + g, 0xC, 4.0);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(N);
    for k in 0..6u32 {
        let t = fx.target_handle();
        fx.unit_position(t, D + 0x1130 + 0x70 * k);
        mover(fx, k);
    }
    // s0 = the OT, s1 = D, s2 = D + 0x70 k, s3 = k (the first call: the
    // loop's leftovers, 6).
    fx.xform_world_saving(D, 0x01E0_2F34, [Some(OT), Some(D), Some(D + 0x2A0), Some(6)]);
    floats_saved(fx, sp);
    for k in 0..6u32 {
        let regs = [Some(OT), Some(D), Some(D + 0x70 * k), Some(k)];
        let x = D + 0x70 * k;
        fx.xform_billboard_saving(x + 0xB10, 0x01E0_2F44, regs);
        fx.xform_billboard_saving(x + 0x1110, 0x01E0_2F4C, regs);
        fx.xform_world_saving(x + 0x13B0, 0x01E0_2F54, regs);
        floats_saved(fx, sp);
    }
    if (0x1E..0xB9).contains(&n(fx)) {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if n(fx) >= 0xB9 {
        tint(fx, D + 0x47C8);
        fx.step_down(D + 0x47CB, 0, 0x10);
    }
    for k in 0..6u32 {
        quad(fx, k);
        sparks(fx, k);
        settle(fx, k);
    }
    let m = n(fx) + 1;
    fx.mem.set32(N, m);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if m == 0xB9 {
        2
    } else if m == 0x32 {
        1
    } else if (0x3C..0xB9).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}

/// What 0x1038D0's 0x20C0B0 leaves at sp - 0xD0.. besides its register
/// saves: its prologue's `swc1 f20..f24` (+0..+0x10), the runner's 0 [run:
/// the facing quad's cell setter 0x104DC0 keeps the top bits of the u16 it
/// overwrites there, later a missing texture's TEX0].
fn floats_saved(fx: &mut Fx, sp: u32) {
    for k in 0..5u32 {
        fx.mem.write_u32(sp - 0xD0 + 4 * k, 0);
    }
}

/// 0x1E03090(D, ot, k): point k at the node until its update +0x4788,
/// then 1 / +0x4770 of the way to the target (counting down) with its
/// streamer pushed; the sparks' transform +0x13B0 there, turned to look
/// from the attacker to the target (0x103DB0).
fn mover(fx: &mut Fx, k: u32) {
    let sp = fx.enter(0x70);
    let p = D + 0xB30 + 0x70 * k;
    if fx.mem.i32(N) < fx.mem.i32(LEAVE + 4 * k) {
        let h = fx.self_handle();
        let s = fx.mem.i32(NODE);
        fx.node_position(h, s, p);
    } else {
        let m = fx.mem.i32(STEPS + 4 * k);
        fx.approach_by(p, D + 0x1130 + 0x70 * k, m);
        if m != 0 {
            fx.mem.set32(STEPS + 4 * k, m - 1);
        }
        let pts = fx.mem.read_u32(D + 0x1F0 + 4 * k);
        streamer::push(fx, D + 0x70 + 0x40 * k, pts, p);
    }
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x60);
    for c in 0..4u32 {
        let v = fx.mem.read_u32(p + 4 * c);
        fx.mem.write_u32(D + 0x13D0 + 0x70 * k + 4 * c, v);
    }
    fx.look_angles(sp + 0x60, D + 0x1130 + 0x70 * k, D + 0x13C0 + 0x70 * k);
    fx.leave(0x70);
}

/// 0x1E031D0(D, ot, k): from one update before point k leaves, its disc
/// and facing quad; from its update +0x47A0 the billboard grows to 5 by
/// 0.2 while both fade.
fn quad(fx: &mut Fx, k: u32) {
    let sp = fx.enter(0x70);
    let n = fx.mem.i32(N);
    if n >= fx.mem.i32(LEAVE + 4 * k) - 1 {
        let x = D + 0xB10 + 0x70 * k;
        fx.disc(D + 0xDB0 + 0x10 * k, x);
        facing::quad(fx, OT, D + 0xE10 + 0x60 * k, D + 0x1050 + 0x20 * k, x);
        if fx.mem.i32(N) >= fx.mem.i32(GROW + 4 * k) {
            fx.approach(x, 5.0, f32::from_bits(0x3E4C_CCCD));
            fx.step_down(D + 0xDB7 + 0x10 * k, 0, 0x10);
            fx.step_down(D + 0xE47 + 0x60 * k, 0, 8);
        }
    }
    let _ = sp;
    fx.leave(0x70);
}

/// 0x1E032F0(D, ot, k): from point k's leaving until 0xB8 its sparks
/// (fading after +0x47A0; each live particle pushed out by 1.2, respawned
/// in a flat ring at height 4 above 30) and its streamer, whose points
/// fade by 8.
fn sparks(fx: &mut Fx, k: u32) {
    let sp = fx.enter(0xB0);
    // swc1 f20: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    let n = fx.mem.i32(N);
    if n >= fx.mem.i32(LEAVE + 4 * k) && n < 0xB9 {
        let g = D + 0x1650 + 0x30 * k;
        trail::sparks(fx, OT, g, D + 0x13B0 + 0x70 * k);
        if fx.mem.i32(GROW + 4 * k) < fx.mem.i32(N) {
            fx.step_down(g + 0xB, 0, 8);
        }
        let grow = f32::from_bits(0x3F99_999A);
        let mut j = 0u32;
        while (j as i32) < fx.mem.i16(g + 0x2C) {
            let p = fx.mem.read_u32(g + 0xC) + 0x10 * j;
            if fx.mem.i16(p + 0xE) >= 0 {
                for o in [0u32, 4, 8] {
                    let v = ee::mul(fx.mem.f(p + o), grow);
                    fx.mem.setf(p + o, v);
                }
                if !ee_le(fx.mem.f(p + 4), 30.0) {
                    let v = ee::mul(10.0, fx.mem.f(g + 0x28));
                    fx.fix_residue(v, 0x01E0_3444);
                    let spread = ee::fixsfsi(v);
                    // rand's frame: s0 = the particle index.
                    let a = fx.rand_from(j, 0x01E0_344C) & 0xFFFF;
                    let r = fx.rand_from(j, 0x01E0_3468);
                    let (_, h) = fx.div(r, spread);
                    fx.mem.write_u32(p + 4, 0x4080_0000);
                    let rad = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h));
                    let c = fx.tcos(a);
                    fx.mem.setf(p + 8, ee::mul(rad, c));
                    let s = fx.tsin(a);
                    fx.mem.setf(p, ee::mul(rad, s));
                    let r = fx.rand_from(j, 0x01E0_34D0);
                    let (_, h) = fx.div(r, 0xC);
                    fx.mem.set16(p + 0xE, -h);
                }
            }
            j += 1;
        }
        let pts = fx.mem.read_u32(D + 0x1F0 + 4 * k);
        streamer::draw(fx, STREAMER, OT, D + 0x70 + 0x40 * k, pts, D);
        let mut j = 0u32;
        while (j as i32) < fx.mem.i16(D + 0xAA + 0x40 * k) {
            let pts = fx.mem.read_u32(D + 0x1F0 + 4 * k);
            fx.step_down(pts + 0xC + 0x10 * j, 0, 8);
            j += 1;
        }
    }
    fx.leave(0xB0);
}

/// 0x1E035A0(D, ot, k): once point k has arrived (+0x4770 = 0) its
/// billboard grows to 10 by 1 and its disc, quad and streamer fade.
fn settle(fx: &mut Fx, k: u32) {
    let sp = fx.enter(0x30);
    if fx.mem.i32(STEPS + 4 * k) == 0 {
        fx.approach(D + 0xB10 + 0x70 * k, 10.0, 1.0);
        fx.step_down(D + 0xDB7 + 0x10 * k, 0, 0x10);
        fx.step_down(D + 0xE47 + 0x60 * k, 0, 8);
        fx.step_down(D + 0x77 + 0x40 * k, 0, 5);
    }
    let _ = sp;
    fx.leave(0x30);
}
