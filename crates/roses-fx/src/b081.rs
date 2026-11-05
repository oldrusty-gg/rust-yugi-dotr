//! BMODULE b081 (card 7), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E046F0, data area 0x1ED0000) [F]: b121's family on a pool of 200 tasks of 0x40 bytes at D+0x2F0
//! (alive +0x36, step +0x24, each step called by its own `jal`; spawn
//! 0x1E056D0 = b086's on colour D+0x3CF4, index D+0x3D38, +0x3A from
//! D+0x3D32 and the point D+0x250). The emitter 0x1E01C30 carries a point
//! D+0x3CC0 between the attacker's nodes 0x41 / 0x40 / 0x43 (until update
//! 0xB3; the 0x40 - 0x41 gap scaled by D+0x3D0C = 1.15 growing by 0.03 to
//! 1.4 from 0x47) and then a fortieth of the attacker-to-target gap an
//! update (to 0xDC), and spawns: 0x43 a window flash 0x1E044C0; 0x46 three
//! and one 30-point trails 0x1E036E0 / 0x1E03A70 (`spline::tube`); 0xA a
//! camera light 0x1E04240 at the point; to 0x100 a dome band 0x1E03310 an
//! update (latitude 90 rising by 0..6, size D+0x3D3E); to 0x105 every
//! seventh update a `ring_band_t20` glow 0x1E02B40; to 0x4C on odd updates
//! a closing ring 0x1E02D60 at the point and to 0xC9 a ring 0x1E02FB0 at
//! node 0x41 (`ring_at_m_uv1`); 0xB3..0xDC two rings 0x1E03BF0 a update;
//! 0x39..0x42 eight brown quads 0x1E02740 (sprite quads twice at node
//! 0x41's projected point); 0x40 / 0xB1 / 0xDC the window tints 0x1E02850
//! / 0x1E02970; 6 a sprite pair 0x1E031E0 at node 0x44 / 0x46 by the
//! facing and the eye's side; 0x39, 0x4B..0xAA,
//! 0xAA..0xDC and 0xDC bursts of ring bands 0x1E03490; 0xDC a red glow
//! 0x1E04140; 0xF5..0x113 / 0x78..0xB3 rings 0x1E03DE0 / 0x1E03F90.
//! The stage-light step 0x1E043F0 is never spawned (not ported).
//! Returns 1 on update 0xC8, 4 on 0x118, 2 on 0x113, 6 on 0xD3..0x10D, 8
//! once the 325-update timer D+0x3CF0 has run out.
use crate::blib::{quads4, spawn as bspawn, spline, uv};
use crate::bus::Bus;
use crate::c061::{blend_alpha, rgbaq_at};
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0xC8;
const POOL: u32 = D + 0x2F0;
const SPR: u32 = 0x7000_0000;

const QUADS: u32 = 0x01E0_2740;
const TINT_A: u32 = 0x01E0_2850;
const TINT_B: u32 = 0x01E0_2970;
const GLOW: u32 = 0x01E0_2B40;
const CLOSE: u32 = 0x01E0_2D60;
const RING: u32 = 0x01E0_2FB0;
const SPRITE: u32 = 0x01E0_31E0;
const DOME: u32 = 0x01E0_3310;
const BURST: u32 = 0x01E0_3490;
const TRAILS: u32 = 0x01E0_36E0;
const TRAIL: u32 = 0x01E0_3A70;
const SPARK: u32 = 0x01E0_3BF0;
const RING_B: u32 = 0x01E0_3DE0;
const RING_C: u32 = 0x01E0_3F90;
const RED: u32 = 0x01E0_4140;
const LIGHT: u32 = 0x01E0_4240;
const FLASH: u32 = 0x01E0_44C0;
/// The steps with the entry's return address after each step's `jal`, in
/// the entry's order.
const STEPS: [(u32, u32); 17] = [
    (QUADS, 0x01E0_495C),
    (TINT_A, 0x01E0_4980),
    (TINT_B, 0x01E0_49A0),
    (GLOW, 0x01E0_49C0),
    (CLOSE, 0x01E0_49E0),
    (RING, 0x01E0_4A00),
    (SPRITE, 0x01E0_4A20),
    (DOME, 0x01E0_4A40),
    (BURST, 0x01E0_4A60),
    (TRAILS, 0x01E0_4A80),
    (TRAIL, 0x01E0_4AA0),
    (SPARK, 0x01E0_4AC0),
    (RING_B, 0x01E0_4AE0),
    (RING_C, 0x01E0_4B00),
    (RED, 0x01E0_4B20),
    (LIGHT, 0x01E0_4B40),
    (FLASH, 0x01E0_4B80),
];

const POS: u32 = D + 0x250;
const CAM: u32 = D + 0x34F0;
const HIST3: u32 = D + 0x3540;
const HIST1: u32 = D + 0x3AE0;
const POINT: u32 = D + 0x3CC0;
const TEX5: u32 = D + 0x3CD0;
const TIMER: u32 = D + 0x3CF0;
const COLOUR: u32 = D + 0x3CF4;
const OT2: u32 = D + 0x3CFC;
const W: u32 = D + 0x3D08;
const SCALE: u32 = D + 0x3D0C;
const NODES: u32 = D + 0x3D20;
const ARG: u32 = D + 0x3D32;
const INDEX: u32 = D + 0x3D38;
const TOGGLE: u32 = D + 0x3D3A;
const YAW: u32 = D + 0x3D3C;
const SIZE: u32 = D + 0x3D3E;
const COUNTER: u32 = D + 0x3D42;
const BYTES: u32 = D + 0x3D44;
const BYTES2: u32 = D + 0x3E44;
const SINES: u32 = D + 0x4144;

fn copy3(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// rand % m (HI) from the `jal` returning to `ra` (rand's frame: s0).
fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

fn node(fx: &mut Fx, at: u32) -> i32 {
    fx.mem.i16(NODES + at)
}

/// The entry 0x1E046F0 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x200);
    let eye = fx.camera_eye(0);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(OT2, OT);
    let ret = if param >= 0 { init(fx, sp, param, eye) } else { step(fx, sp, param, eye) };
    fx.leave(0x200);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32, eye: u32) -> i32 {
    fx.bzero(D, 0x42B0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x3D40, param);
    fx.mem.set32(TIMER, 0x145);
    for (k, (n, clear)) in [(0x41, 1), (0x40, 0), (0x43, 0), (0x17, 0), (0x44, 0), (0x46, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    textures(fx, eye);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x160);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0x164)), 180.0);
    fx.fix_residue(v, 0x01E0_4844);
    fx.mem.set16(YAW, ee::fixsfsi(v));
    // rand's frame: s0 = D.
    for k in 0..0x100u32 {
        let r = rmod(fx, D, 0x01E0_4850, 0x1E);
        fx.mem.write_u8(BYTES + k, (r - 0xF) as u8);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, 0x01E0_4884) % 16;
        fx.mem.write_u8(BYTES2 + k, (r - 8) as u8);
    }
    for k in 0xB4u32..0x100 {
        fx.mem.write_u8(BYTES2 + k, 0);
    }
    for k in 0x64u32..0xB4 {
        let r = rmod(fx, D, 0x01E0_48E0, 0xA);
        fx.mem.write_u8(BYTES2 + k, (r - 5) as u8);
    }
    fx.mem.write_u32(SCALE, 0x3F93_3333);
    0
}

/// 0x1E04640(0, D): the effect textures 0..2 at D+0x2B0, 13..15 at
/// D+0x2E4, the attacker's 0..2 at D+0x3CD0.
fn textures(fx: &mut Fx, eye: u32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_4808), (0x20, u64::from(eye)), (0x10, 0), (0, u64::from(D))]);
    for k in 0..3 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x2B0 + 4 * k as u32, t);
    }
    for k in 0xD..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x2E4 + 4 * (k as u32 - 0xD), t);
    }
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX5 + 4 * k as u32, t);
    }
    fx.leave(0x40);
}

fn step(fx: &mut Fx, sp: u32, param: i32, eye: u32) -> i32 {
    emit(fx, param, eye);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x24);
        if let Some(&(_, ra)) = STEPS.iter().find(|s| s.0 == f) {
            run_step(fx, f, t, ra, k, eye);
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x90);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) != 0 {
            draw(fx, sp, t, eye);
        }
    }
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) != 0 && fx.mem.read_u32(t + 0x24) == BURST {
            for j in 0..3 {
                let v = ee::from_i32(fx.mem.i16(t + 2 * j));
                fx.mem.setf(sp + 0x90 + 4 * j, v);
            }
            let mode = if fx.mem.i16(t + 0xE) == 2 {
                0x21
            } else if counter(fx) < 0xDE {
                0x10
            } else {
                0x11
            };
            quads4::ring_band_t20(fx, D, t, mode, sp + 0x90);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0xC8 => 1,
        0x118 => 4,
        0x113 => 2,
        0xD3..=0x10D => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` frame `sub` (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let w = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, w);
    }
}

fn tex(fx: &mut Fx, at: u32) {
    let v = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, v);
}

/// What 0x204D80 with no matrix leaves below its caller's `sp`: the
/// camera's view-projection at sp - 0x50 and its local vec at sp - 0x10
/// (as b400's).
fn project_residue(fx: &mut Fx, sp: u32, v: u32) {
    fx.camera_vp_to(sp - 0x50);
    let m = fx.camera_vp();
    let p = [fx.mem.f(v), fx.mem.f(v + 4), fx.mem.f(v + 8), 1.0];
    let o = Fx::vu_apply(&m, p);
    let q = ee::div(1.0, o[3]);
    for k in 0..3u32 {
        fx.mem.setf(sp - 0x10 + 4 * k, ee::mul(o[k as usize], q));
    }
    fx.mem.setf(sp - 4, o[3]);
}

/// The attacker's node at `slot` (into sp + 0x90, copied to sp + 0xA0)
/// projected by 0x204D80(sp + 0xA0, 0, sp + 0x188, D+0x3D08).
fn project_node(fx: &mut Fx, sp: u32, slot: u32) {
    let h = fx.self_handle();
    let n = node(fx, slot);
    fx.node_position(h, n, sp + 0x90);
    copy3(fx, sp + 0xA0, sp + 0x90);
    let p = [fx.mem.f(sp + 0xA0), fx.mem.f(sp + 0xA4), fx.mem.f(sp + 0xA8)];
    let mut x = fx.mem.read_u64(sp + 0x188);
    if let Some(v) = fx.project(p, None, &mut x) {
        fx.mem.write_u64(sp + 0x188, x);
        fx.mem.setf(W, v);
    }
    project_residue(fx, sp, sp + 0xA0);
}

/// A ring of quads at the floats `pos` (D+0x20) with `rows` rows (D+0xB8),
/// row offset +0xA (D+0xC0), mode `mode` and offsets `table`.
fn ring_at(fx: &mut Fx, t: u32, rows: i32, mode: i32, table: u32) {
    fx.mem.set16(D + 0xB8, rows);
    let r = fx.mem.i16(t + 0xA);
    fx.mem.set16(D + 0xC0, r);
    quads4::ring_at_m_uv1(fx, uv::uv_rect, D, t, mode, table);
    fx.mem.set16(D + 0xB8, 0);
}

/// D+0x20 = the task's i16 point as floats.
fn task_point(fx: &mut Fx, t: u32) {
    for k in 0..3 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(D + 0x20 + 4 * k, v);
    }
}

/// The trails' header: texture D+0x2EC, cell 0x97 frame +0x34 with v
/// height 1, the origin 0 and D+0xBE 1.
fn trail_header(fx: &mut Fx, sp: u32, t: u32, r: u32) {
    tex(fx, D + 0x2EC);
    for (o, v) in [(0x94u32, 1), (0x96, 0x10), (0x98, 0x1F), (0x9A, 1)] {
        fx.mem.set16(D + o, v);
    }
    let f = fx.mem.i16(t + 0x34);
    cell(fx, sp + r, 0x97, f);
    fx.mem.set16(D + 0x98, 1);
    for o in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(D + o, 0);
    }
    fx.mem.set16(D + 0xBE, 1);
}

/// The entry's draw case for a live task (`sp` the entry's frame).
fn draw(fx: &mut Fx, sp: u32, t: u32, eye: u32) {
    let f = fx.mem.read_u32(t + 0x24);
    match f {
        QUADS => {
            tex(fx, D + 0x2EC);
            cell(fx, sp + 0xB0, 0x9A, 0);
            fx.mem.set16(D + 0xBE, 0);
            project_node(fx, sp, 0);
            copy3(fx, sp + 0x90, POINT);
            let w = fx.mem.read_u32(W);
            fx.mem.write_u32(sp + 0x9C, w);
            sprite_quads40(fx, t, 0x10, sp + 0x90);
            sprite_quads40(fx, t, 0x10, sp + 0x90);
        }
        TINT_A => {
            let w = fx.mem.f(W);
            tint40(fx, t, 0x20, w);
        }
        TINT_B => {
            project_node(fx, sp, 6);
            let w = fx.mem.f(W);
            tint40(fx, t, 1, w);
        }
        GLOW => {
            copy3(fx, sp + 0x90, POINT);
            let mode = (fx.mem.i16(t + 0x30) << 4) + 1;
            quads4::ring_band_t20(fx, D, t, mode, sp + 0x90);
        }
        CLOSE => {
            let f = fx.mem.i16(t + 0x34);
            if fx.mem.i16(t + 0xC) == 0 {
                cell(fx, sp + 0xC0, 0x14, f);
                tex(fx, D + 0x2B0);
                fx.mem.set16(D + 0xB8, 0xA);
            } else {
                cell(fx, sp + 0xD0, 0x9B, f);
                tex(fx, D + 0x2EC);
                fx.mem.set16(D + 0xB8, 5);
            }
            let r = fx.mem.i16(t + 0xA);
            fx.mem.set16(D + 0xC0, r);
            copy3(fx, D + 0x20, POINT);
            quads4::ring_at_m_uv1(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        RING => {
            let f = fx.mem.i16(t + 0x34);
            let first = fx.mem.i16(t + 0xC) == 0;
            if first {
                cell(fx, sp + 0xE0, 0x14, f);
                tex(fx, TEX5 + 4);
            } else {
                cell(fx, sp + 0xF0, 0x14, f);
                tex(fx, TEX5);
            }
            fx.mem.set16(D + 0xB8, 6);
            let r = fx.mem.i16(t + 0xA);
            fx.mem.set16(D + 0xC0, r);
            task_point(fx, t);
            let mode = if first { 0x10 } else { 0x20 };
            quads4::ring_at_m_uv1(fx, uv::uv_rect, D, t, mode, BYTES2);
            fx.mem.set16(D + 0xB8, 0);
        }
        SPRITE => {
            let left = ee_lt(fx.mem.f(eye), 0.0);
            let at = if (fx.mem.i16(YAW) != 0) == left { 8 } else { 0xA };
            let h = fx.self_handle();
            let n = node(fx, at);
            fx.node_position(h, n, sp + 0x90);
            tex(fx, D + 0x2EC);
            cell(fx, sp + 0x100, 0x9A, 0);
            fx.mem.set16(D + 0xBE, 0);
            fx.mem.write_u32(sp + 0x9C, 0);
            sprite_quads40(fx, t, 0, sp + 0x90);
        }
        DOME => dome(fx, t, sp + 0x170, POINT),
        TRAILS => {
            trail_header(fx, sp, t, 0x110);
            for h in [HIST3, HIST3 + 0x1E0, HIST3 + 0x3C0] {
                spline::tube(fx, 0x01E0_5C00, D, t, h, 0);
            }
        }
        TRAIL => {
            trail_header(fx, sp, t, 0x120);
            spline::tube(fx, 0x01E0_5C00, D, t, HIST1, 0);
        }
        SPARK | RING_B | RING_C => {
            let (r, rows) = match f {
                SPARK => (0x130, 5),
                RING_B => (0x140, 0xA),
                _ => (0x150, 8),
            };
            let fr = fx.mem.i16(t + 0x34);
            cell(fx, sp + r, 0x14, fr);
            tex(fx, TEX5);
            fx.mem.set16(D + 0xBE, 0);
            task_point(fx, t);
            ring_at(fx, t, rows, 0x20, BYTES);
        }
        RED => {
            copy3(fx, sp + 0x90, POINT);
            let mode = if counter(fx) < 0xEC { 0x20 } else { 0x21 };
            quads4::ring_band_t20(fx, D, t, mode, sp + 0x90);
        }
        _ => {}
    }
}

/// 0x1E01250(D, task, mode, pos) for 0x40-byte tasks:
/// `quads4::sprite_quads` with the colours +0x2C / +0x28, the i16 size +8
/// / +0xA and the angle +0x3A, in a 0x200 frame (its locals 0x10 below
/// sprite_quads') [F fdiff vs 792ca1ad9378].
fn sprite_quads40(fx: &mut Fx, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x200);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0x60 + 4 * k, w);
    }
    let d = sp + 0x130;
    l2::desc_strip3(fx, d);
    match mode >> 4 {
        0 => {
            rgbaq_at(fx, sp + 0x1C0, t + 0x2C, d + 0x10);
            rgbaq_at(fx, sp + 0x1C8, t + 0x2C, d + 0x28);
            rgbaq_at(fx, sp + 0x1D0, t + 0x2C, d + 0x40);
            rgbaq_at(fx, sp + 0x1D8, t + 0x28, d + 0x58);
        }
        1 => {
            rgbaq_at(fx, sp + 0x1E0, t + 0x2C, d + 0x10);
            rgbaq_at(fx, sp + 0x1E8, t + 0x2C, d + 0x28);
            rgbaq_at(fx, sp + 0x1F0, t + 0x28, d + 0x40);
            rgbaq_at(fx, sp + 0x1F8, t + 0x28, d + 0x58);
        }
        _ => {}
    }
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tx = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tx, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xC0 + 4 * k, 0x3D4C_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0x120);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x60 + 4 * k);
        fx.mem.write_u32(sp + 0xB0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0xE0, sp + 0xC0, sp + 0x120, sp + 0xB0);
    let a = fx.mem.i16(t + 0x3A);
    let depth = fx.mem.f(sp + 0x6C);
    fx.mem.set32(sp + 0x128, a);
    fx.mem.set32(sp + 0x124, 0);
    fx.mem.set32(sp + 0x120, 0);
    for k in 0..4 {
        let w = fx.mem.i16(t + 8);
        let h = fx.mem.i16(t + 0xA);
        l2::quad_ints(fx, sp + 0x70, w, h, k);
        l2::rotate4(fx, sp + 0x70, sp + 0x120, 2, D + 0xD8);
        if l2::project(fx, sp + 0x70, SPR, sp + 0xE0, d, sp + 0xD0) {
            l2::far_z(fx, &[d + 0x68, d + 0x50, d + 0x38, d + 0x20]);
            let key = ee::fixunssfsi(depth) as i32;
            let ot = fx.mem.read_u32(D + 0x80);
            fx.submit(d, ot, key);
        }
    }
    fx.leave(0x200);
}

/// 0x1E015A0(D, task, mode, w): `quads4::tint4` for 0x40-byte tasks: a
/// full-window gouraud quad coloured +0x28 (top) / +0x2C (bottom), blend
/// (mode >> 4) as i16, keyed 0 (no far Z) when mode & 15 is 0, else by `w`.
fn tint40(fx: &mut Fx, t: u32, mode: i32, w: f32) {
    let sp = fx.enter(0xF0);
    let d = sp + 0x60;
    l2::colour_strip4(fx, d, t + 0x28, 1);
    l2::colour_strip4(fx, d, t + 0x2C, 2);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    fx.desc_alpha(d, blend_alpha((mode >> 4) as i16 as i32));
    let key = if mode & 0xF == 0 { 0 } else { ee::fixunssfsi(w) as i32 };
    fx.submit(d, OT, key);
    fx.leave(0xF0);
}

/// 0x1E05800(D, task, desc, pos): a dome band (b148's 0x1E05240 with the
/// descriptor set first: strip3, atlas 0x8C cell +0x34, colour +0x28,
/// texture D+0x2E8): 12 pieces of 30 degrees from +0x3A between latitudes
/// +0x30 and +0x30 + 20 (radius 90, y 90 sin; the sines D+0x4144) at `pos`,
/// scale size +0x38 / 1500, turned by +0x10; each keyed by its mean w into
/// D+0x3CFC.
fn dome(fx: &mut Fx, t: u32, d: u32, pos: u32) {
    let sp = fx.enter(0x190);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0xC0 + 4 * k, w);
    }
    let sub = fx.mem.i16(t + 0x34);
    l2::rect(fx, sp + 0x170, 0x8C, sub);
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x28, 0);
    l2::uv_strip3(fx, d, sp + 0x170, 0);
    let tx = fx.mem.read_u32(D + 0x2E8);
    fx.desc_texture(d, tx, 1);
    copy3(fx, sp + 0x110, sp + 0xC0);
    let s = ee::div(ee::from_i32(fx.mem.i16(t + 0x38)), 1500.0);
    l2::set3(fx, sp + 0x120, s, s, s);
    l2::xform_vp(fx, sp + 0x130, sp + 0x120, t + 0x10, sp + 0x110);
    fx.mem.write_u32(sp + 0xB0, 0);
    let lat = fx.mem.i16(t + 0x30);
    let y = ee::mul(90.0, l2::sin_deg(fx, SINES, lat, 0));
    fx.mem.setf(sp + 0xD4, y);
    fx.mem.setf(sp + 0xE4, y);
    let lat2 = (fx.mem.i16(t + 0x30) + 0x14) as i16 as i32;
    let y = ee::mul(90.0, l2::sin_deg(fx, SINES, lat2, 0));
    fx.mem.setf(sp + 0xF4, y);
    fx.mem.setf(sp + 0x104, y);
    let lat = fx.mem.i16(t + 0x30);
    let f20 = ee::mul(90.0, l2::sin_deg(fx, SINES, lat, 1));
    let lat2 = (fx.mem.i16(t + 0x30) + 0x14) as i16 as i32;
    let f21 = ee::mul(90.0, l2::sin_deg(fx, SINES, lat2, 1));
    let mut a = 0i32;
    for k in 0..0xC {
        let pairs: &[(i32, u32, u32)] = if k == 0 { &[(0, 0xF0, 0xD0), (0x1E, 0x100, 0xE0)] } else { &[(0x1E, 0x100, 0xE0)] };
        if k != 0 {
            for (to, from) in [(0xF0u32, 0x100u32), (0xF8, 0x108), (0xD0, 0xE0), (0xD8, 0xE8)] {
                let v = fx.mem.read_u32(sp + from);
                fx.mem.write_u32(sp + to, v);
            }
        }
        for &(off, outer, inner) in pairs {
            let deg = (fx.mem.i16(t + 0x3A) + off + a) as i16 as i32;
            let c = l2::sin_deg(fx, SINES, deg, 1);
            fx.mem.setf(sp + outer, ee::mul(f21, c));
            let s = l2::sin_deg(fx, SINES, deg, 0);
            fx.mem.setf(sp + outer + 8, ee::mul(f21, s));
            let c = l2::sin_deg(fx, SINES, deg, 1);
            fx.mem.setf(sp + inner, ee::mul(f20, c));
            let s = l2::sin_deg(fx, SINES, deg, 0);
            fx.mem.setf(sp + inner + 8, ee::mul(f20, s));
        }
        if l2::project(fx, sp + 0xD0, SPR, sp + 0x130, d, sp + 0x180) {
            let key = ee::fixunssfsi(fx.mem.f(sp + 0x180)) as i32;
            let ot = fx.mem.read_u32(OT2);
            fx.submit(d, ot, key);
        }
        a = (a + 0x1E) as i16 as i32;
    }
    fx.mem.write_u32(sp + 0xB0, 1);
    fx.leave(0x190);
}

/// 0x1E01C30(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter,
/// the carried point D+0x3CC0 and the spawns (frame 0xA0: ra, s5 / s4 the
/// runner's (0) [H], s3 = the parameter, s2 = the eye pointer, s1 = 0,
/// s0 = D).
fn emit(fx: &mut Fx, param: i32, eye: u32) {
    let sp = fx.enter(0xA0);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x60, 0x01E0_4928), (0x50, 0), (0x40, 0), (0x30, p), (0x20, u64::from(eye)), (0x10, 0), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    // The emitter's s0..s4 at each spawn (saved by the spawn's prologue).
    let mut s: [u64; 5] = [u64::from(D), 0, u64::from(eye), p, 0];
    let sp_ = |f: &mut Fx, step: u32, s: &[u64; 5], ra: u32| {
        spawn(f, step, &[(0x50, u64::from(ra)), (0x40, s[4]), (0x30, s[3]), (0x20, s[2]), (0x10, s[1]), (0, s[0])]);
    };
    if counter(fx) == 0x43 {
        sp_(fx, FLASH, &s, 0x01E0_1C90);
    }
    if counter(fx) >= 0xDC {
        let v = fx.mem.i16(SIZE) + 6;
        fx.mem.set16(SIZE, v);
    }
    if counter(fx) == 0x46 {
        sp_(fx, TRAILS, &s, 0x01E0_1CD0);
    }
    if counter(fx) == 0x46 {
        sp_(fx, TRAIL, &s, 0x01E0_1CF4);
    }
    if counter(fx) == 0xA {
        sp_(fx, LIGHT, &s, 0x01E0_1D18);
    }
    if counter(fx) < 0x100 {
        sp_(fx, DOME, &s, 0x01E0_1D3C);
    }
    let c = counter(fx);
    if c < 0x105 && fx.div(c, 7).1 == 1 {
        let v = fx.mem.i16(TOGGLE);
        fx.mem.set16(INDEX, v);
        sp_(fx, GLOW, &s, 0x01E0_1D88);
        let v = if fx.mem.i16(TOGGLE) != 0 { 0 } else { 1 };
        fx.mem.set16(TOGGLE, v);
    }
    let c = counter(fx);
    if c < 0x4C && c % 2 == 1 {
        let r = fx.rand_from(D, 0x01E0_1DDC) % 2;
        fx.mem.set16(INDEX, r);
        sp_(fx, CLOSE, &s, 0x01E0_1E08);
    }
    if counter(fx) < 0xC9 {
        let h = fx.self_handle();
        let n = node(fx, 0);
        fx.node_position(h, n, sp + 0x70);
        copy3(fx, POS, sp + 0x70);
        let c = counter(fx);
        let i = if fx.div(c, 3).1 == 1 { 1 } else { 0 };
        fx.mem.set16(INDEX, i);
        sp_(fx, RING, &s, 0x01E0_1E8C);
    }
    let c = counter(fx);
    if c < 0xB3 {
        carry(fx, sp);
        s = [u64::from(sp + 0x90), u64::from(sp + 0x94), u64::from(sp + 0x98), u64::from(sp + 0x84), u64::from(sp + 0x88)];
    } else if c < 0xDC {
        let h = fx.self_handle();
        fx.unit_position(h, sp + 0x80);
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x90);
        for k in 0..3 {
            let v = ee::div(ee::sub(fx.mem.f(sp + 0x90 + 4 * k), fx.mem.f(sp + 0x80 + 4 * k)), 40.0);
            fx.mem.setf(sp + 0x90 + 4 * k, v);
        }
        for k in 0..3 {
            let v = ee::add(fx.mem.f(POINT + 4 * k), fx.mem.f(sp + 0x90 + 4 * k));
            fx.mem.setf(POINT + 4 * k, v);
            fx.mem.setf(POS + 4 * k, v);
        }
        s[0] = u64::from(sp + 0x90);
        sp_(fx, SPARK, &s, 0x01E0_21A0);
        sp_(fx, SPARK, &s, 0x01E0_21B4);
    }
    for (c0, pairs) in [(0x39, &[(0x5A, 0x01E0_21E0u32)][..]), (0x3C, &[(0, 0x01E0_2208)][..]), (0x3E, &[(0x2D, 0x01E0_2234), (0x87, 0x01E0_2250)][..]), (0x40, &[(0x19, 0x01E0_227C), (0x41, 0x01E0_2298)][..]), (0x42, &[(0x69, 0x01E0_22C4), (0x9B, 0x01E0_22E0)][..])] {
        if counter(fx) == c0 {
            for &(a, ra) in pairs {
                fx.mem.set16(ARG, a);
                sp_(fx, QUADS, &s, ra);
            }
        }
    }
    fx.mem.set16(INDEX, 0);
    for (c0, i, ras) in [(0x40, 1, [0x01E0_2308u32, 0x01E0_232C]), (0xB1, 2, [0x01E0_2358, 0x01E0_237C]), (0xDC, -1, [0x01E0_23A8, 0x01E0_23CC])] {
        if counter(fx) == c0 {
            sp_(fx, TINT_A, &s, ras[0]);
        }
        if counter(fx) == c0 {
            sp_(fx, TINT_B, &s, ras[1]);
        }
        if i >= 0 {
            fx.mem.set16(INDEX, i);
        }
    }
    if counter(fx) == 6 {
        sp_(fx, SPRITE, &s, 0x01E0_23F0);
    }
    if counter(fx) == 0x39 {
        let x = fx.mem.read_u32(POINT);
        fx.mem.write_u32(POS, x);
        let y = ee::sub(fx.mem.f(POINT + 4), 8.0);
        fx.mem.setf(POS + 4, y);
        let z = fx.mem.read_u32(POINT + 8);
        fx.mem.write_u32(POS + 8, z);
        fx.mem.set16(INDEX, 0);
        for ra in [0x01E0_2458u32, 0x01E0_246C, 0x01E0_2480, 0x01E0_2494, 0x01E0_24A8, 0x01E0_24BC] {
            sp_(fx, BURST, &s, ra);
        }
    }
    let c = counter(fx);
    if (0x4B..0xAB).contains(&c) && fx.div(c, 3).1 == 1 {
        copy3(fx, POS, POINT);
        fx.mem.set16(INDEX, 0);
        sp_(fx, BURST, &s, 0x01E0_2528);
        sp_(fx, BURST, &s, 0x01E0_253C);
    }
    let c = counter(fx);
    if (0xAA..0xDD).contains(&c) {
        copy3(fx, POS, POINT);
        fx.mem.set16(INDEX, 1);
        sp_(fx, BURST, &s, 0x01E0_258C);
    }
    if counter(fx) == 0xDC {
        copy3(fx, POS, POINT);
        fx.mem.set16(INDEX, 2);
        for ra in [0x01E0_25D0u32, 0x01E0_25E4, 0x01E0_25F8, 0x01E0_260C, 0x01E0_2620, 0x01E0_2634, 0x01E0_2648, 0x01E0_265C] {
            sp_(fx, BURST, &s, ra);
        }
    }
    if counter(fx) == 0xDC {
        fx.mem.set16(INDEX, 1);
        sp_(fx, RED, &s, 0x01E0_2688);
    }
    let c = counter(fx);
    if (0xF5..0x114).contains(&c) {
        copy3(fx, POS, POINT);
        sp_(fx, RING_B, &s, 0x01E0_26D0);
    }
    let c = counter(fx);
    if (0x78..0xB4).contains(&c) {
        copy3(fx, POS, POINT);
        sp_(fx, RING_C, &s, 0x01E0_2718);
    }
    fx.leave(0xA0);
}

/// The emitter's carried point before update 0xB3: node 0x40 plus the
/// node 0x41 -> 0x40 gap times D+0x3D0C, then 0.8 of the node 0x40 -> 0x43
/// gap; the dome size D+0x3D3E up by 2 below 30 before update 30 and
/// below 120 from 0x47, and D+0x3D0C up by 0.03 below 1.4 from 0x47.
fn carry(fx: &mut Fx, sp: u32) {
    let h = fx.self_handle();
    let n = node(fx, 2);
    fx.node_position(h, n, POINT);
    let h = fx.self_handle();
    let n = node(fx, 0);
    fx.node_position(h, n, sp + 0x80);
    let h = fx.self_handle();
    let n = node(fx, 2);
    fx.node_position(h, n, sp + 0x90);
    let h = fx.self_handle();
    let n = node(fx, 0);
    fx.node_position(h, n, sp + 0x70);
    for k in 0..3 {
        let d = ee::sub(fx.mem.f(sp + 0x90 + 4 * k), fx.mem.f(sp + 0x80 + 4 * k));
        let v = ee::mul(fx.mem.f(SCALE), d);
        fx.mem.setf(sp + 0x90 + 4 * k, v);
    }
    for k in 0..3 {
        let v = ee::add(fx.mem.f(sp + 0x80 + 4 * k), fx.mem.f(sp + 0x90 + 4 * k));
        fx.mem.setf(POINT + 4 * k, v);
    }
    let h = fx.self_handle();
    let n = node(fx, 2);
    fx.node_position(h, n, sp + 0x80);
    let h = fx.self_handle();
    let n = node(fx, 4);
    fx.node_position(h, n, sp + 0x90);
    let k8 = f32::from_bits(0x3F4C_CCCD);
    for k in 0..3 {
        let d = ee::sub(fx.mem.f(sp + 0x90 + 4 * k), fx.mem.f(sp + 0x80 + 4 * k));
        fx.mem.setf(sp + 0x90 + 4 * k, ee::mul(k8, d));
    }
    for k in 0..3 {
        let v = ee::add(fx.mem.f(POINT + 4 * k), fx.mem.f(sp + 0x90 + 4 * k));
        fx.mem.setf(POINT + 4 * k, v);
    }
    let c = counter(fx);
    if c < 0x1E {
        let v = fx.mem.i16(SIZE);
        if v < 0x1E {
            fx.mem.set16(SIZE, v + 2);
        }
    } else if c >= 0x47 {
        let v = fx.mem.i16(SIZE);
        if v < 0x78 {
            fx.mem.set16(SIZE, v + 2);
        }
    }
    if counter(fx) >= 0x47 {
        let k = fx.mem.f(SCALE);
        if ee_lt(k, f32::from_bits(0x3FB3_3333)) {
            fx.mem.setf(SCALE, ee::add(k, f32::from_bits(0x3CF5_C28F)));
        }
    }
}

/// 0x1E056D0(D, step, 0): the first free task cleared and set off at
/// D+0x250 (as i16) with the colour D+0x3CF4 (twice), +0x30 = D+0x3D38,
/// +0x3A = D+0x3D32; `saves` the spawn's prologue saves.
fn spawn(fx: &mut Fx, f: u32, saves: &[(u32, u64)]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(POOL + 0x36 + 0x40 * k) == 0) {
        Some(k) => {
            let t = POOL + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            for c in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, POS, 0);
            let a = fx.mem.i16(INDEX);
            fx.mem.set16(t + 0x30, a);
            let a = fx.mem.i16(ARG);
            fx.mem.set16(t + 0x3A, a);
            fx.mem.write_u32(t + 0x20, 0);
            fx.mem.write_u32(t + 0x24, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// A step's frame and its saves of the entry's s0 = D, s1 = 0, s2 = the
/// eye pointer, s3 = the slot (D + 0x40 k), s4 = k (s5 / f20 the
/// runner's: 0 [H]).
fn run_step(fx: &mut Fx, f: u32, t: u32, ra: u32, k: u32, eye: u32) {
    let (ra, d) = (u64::from(ra), u64::from(D));
    let size = match f {
        GLOW | TRAIL => 0x30,
        DOME => 0x40,
        TRAILS => 0x90,
        FLASH => 0x80,
        _ => 0x20,
    };
    let sp = fx.enter(size);
    match size {
        0x30 | 0x40 => fx.saved(sp, &[(0x20, ra), (0x10, 0), (0, d)]),
        0x90 => {
            fx.saved(sp, &[(0x70, ra), (0x60, 0), (0x50, u64::from(k)), (0x40, u64::from(D + 0x40 * k)), (0x30, u64::from(eye)), (0x20, 0), (0x10, d)]);
            fx.mem.write_u32(sp, 0);
        }
        _ => fx.saved(sp, &[(0x10, ra), (0, d)]),
    }
    match f {
        QUADS => quads(fx, t),
        TINT_A => tint_a(fx, t),
        TINT_B => tint_b(fx, t),
        GLOW => glow(fx, t),
        CLOSE => close(fx, t),
        RING => ring(fx, t),
        SPRITE => sprite(fx, t),
        DOME => dome_step(fx, sp, t),
        BURST => burst(fx, t),
        TRAILS => trails(fx, sp, t),
        TRAIL => trail(fx, t),
        SPARK => spark(fx, t),
        RING_B => ring_b(fx, t),
        RING_C => ring_c(fx, t),
        RED => red(fx, t),
        LIGHT => light(fx, sp, t),
        _ => flash(fx, sp, t),
    }
    fx.leave(size);
}

fn age(fx: &mut Fx, t: u32) -> i32 {
    fx.mem.i16(t + 0x32)
}

fn grow(fx: &mut Fx, t: u32) {
    let n = age(fx, t) + 1;
    fx.mem.set16(t + 0x32, n);
}

fn first(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x36) == 1
}

fn kill(fx: &mut Fx, t: u32) {
    fx.mem.set16(t + 0x36, 0);
}

/// The alpha byte `at` up by `up` while under 0x80.
fn rise(fx: &mut Fx, t: u32, at: u32, up: u8) {
    let a = fx.mem.read_u8(t + at);
    if a < 0x80 {
        fx.mem.write_u8(t + at, a.wrapping_add(up));
    }
}

/// The alpha byte `at` down by `down` while at least `floor`, else the
/// task's end (`end`) or the byte 0.
fn fall(fx: &mut Fx, t: u32, at: u32, floor: u8, down: u8, end: bool) {
    let a = fx.mem.read_u8(t + at);
    if a >= floor {
        fx.mem.write_u8(t + at, a - down);
    } else if end {
        kill(fx, t);
    } else {
        fx.mem.write_u8(t + at, 0);
    }
}

/// 0x1E02740: a brown quad pair (size 6, white in, (0x40, 0x30, 8) out,
/// 8 x 0x2D0): in by 0x40 for 4 updates, then out by 0xF.
fn quads(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 6);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x2C, 0x40, 0x30, 8);
        fx.mem.set16(t + 8, 8);
        fx.mem.set16(t + 0xA, 0x2D0);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if age(fx, t) < 4 {
        rise(fx, t, 0x2B, 0x40);
        rise(fx, t, 0x2F, 0x40);
    } else {
        fall(fx, t, 0x2B, 0x11, 0xF, true);
        fall(fx, t, 0x2F, 0x11, 0xF, false);
    }
    grow(fx, t);
}

/// 0x1E02850: a window tint ((0xC0, 0xC0, 0x80) both rows) in by 0x15
/// for 8 updates (0x1C for slot 2), then out by 0xF.
fn tint_a(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        l2::rgb(fx, t + 0x28, 0xC0, 0xC0, 0x80);
        l2::rgb(fx, t + 0x2C, 0xC0, 0xC0, 0x80);
        let life = if fx.mem.i16(t + 0x30) == 2 { 0x1C } else { 8 };
        fx.mem.set16(t + 6, life);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < fx.mem.i16(t + 6) {
        rise(fx, t, 0x2B, 0x15);
        rise(fx, t, 0x2F, 0x15);
    } else {
        fall(fx, t, 0x2B, 0x11, 0xF, true);
        fall(fx, t, 0x2F, 0x11, 0xF, false);
    }
    grow(fx, t);
}

/// 0x1E02970: a grey 0xC0 window tint in by 0x15 for 8 / 30 / 35 updates
/// (slot 0 / 1 / 2), then out by 9; cyan (0x20, 0xC0, 0xC0) on ages 7
/// mod 8, blue (0x20, 0x20, 0xC0) on 6 mod 8.
fn tint_b(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        l2::rgb(fx, t + 0x28, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x2C, 0xC0, 0xC0, 0xC0);
        match fx.mem.i16(t + 0x30) {
            0 => fx.mem.set16(t + 6, 8),
            1 => fx.mem.set16(t + 6, 0x1E),
            2 => fx.mem.set16(t + 6, 0x23),
            _ => {}
        }
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < fx.mem.i16(t + 6) {
        rise(fx, t, 0x2B, 0x15);
        rise(fx, t, 0x2F, 0x15);
    } else {
        fall(fx, t, 0x2B, 0xB, 9, true);
        fall(fx, t, 0x2F, 0xB, 9, false);
    }
    match age(fx, t) % 8 {
        7 => {
            l2::rgb(fx, t + 0x28, 0x20, 0xC0, 0xC0);
            l2::rgb(fx, t + 0x2C, 0x20, 0xC0, 0xC0);
        }
        6 => {
            l2::rgb(fx, t + 0x28, 0x20, 0x20, 0xC0);
            l2::rgb(fx, t + 0x2C, 0x20, 0x20, 0xC0);
        }
        _ => {}
    }
    grow(fx, t);
}

/// 0x1E02B40: a `ring_band_t20` glow at the point (radius 10 growing by 4,
/// by 8 from update 0xF8; grey 0x60 in, orange (0x80, 0x30 / 0x60, 0x10)
/// out by the update's parity, outer alpha 8): +0x2B up by 0xC for 18
/// updates, then out by 9; grey 0x30 / 0x80 by parity; facing the camera
/// turned 0x4000 for 10 updates; mode slot 2 (0 on age 10); it ends at
/// age 0xD3.
fn glow(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0xA);
        l2::rgb(fx, t + 0x28, 0x60, 0x60, 0x60);
        if counter(fx) % 2 == 1 {
            l2::rgb(fx, t + 0x2C, 0x80, 0x30, 0x10);
        } else {
            l2::rgb(fx, t + 0x2C, 0x80, 0x60, 0x10);
        }
        let v = rmod(fx, t, 0x01E0_2BDC, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 8);
    }
    if age(fx, t) < 0x12 {
        let a = fx.mem.read_u8(t + 0x2B);
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(0xC));
    } else {
        fall(fx, t, 0x2B, 0xB, 9, true);
    }
    let g = if age(fx, t) % 2 == 1 { 0x30 } else { 0x80 };
    l2::rgb(fx, t + 0x28, g, g, g);
    if age(fx, t) < 0xA {
        crate::b494::camera_angles_saving(fx, t + 0x10, u64::from(t), u64::from(D), 0x01E0_2CB0);
        let x = fx.mem.i32(t + 0x10).wrapping_add(0x4000);
        fx.mem.set32(t + 0x10, x);
    }
    let s = if age(fx, t) == 0xA { 0 } else { 2 };
    fx.mem.set16(t + 0x30, s);
    let by = if counter(fx) < 0xF8 { 4 } else { 8 };
    if age(fx, t) != 0 {
        let v = fx.mem.i16(t + 0x38) + by;
        fx.mem.set16(t + 0x38, v);
    }
    grow(fx, t);
    if age(fx, t) >= 0xD3 {
        kill(fx, t);
    }
}

/// 0x1E02D60: a ring of quads at the point closing in (slot 0: size 2,
/// white, cell 0..6 counting to 7 every third update; slot 1: size 1..4,
/// pink (0xF0, 0x60, 0x60), cell 0..3; row offset 0..7, radius 100..199
/// closing by 7..16, row +8 0..34, life +6 2..4, orientation falling by 10).
fn close(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        if fx.mem.i16(t + 0x30) == 0 {
            fx.mem.set16(t + 0x38, 2);
            let v = rmod(fx, t, 0x01E0_2D94, 7);
            fx.mem.set16(t + 0x34, v);
            l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        } else {
            let v = fx.rand_from(t, 0x01E0_2DC8) % 4;
            fx.mem.set16(t + 0x38, v + 1);
            let v = fx.rand_from(t, 0x01E0_2DE8) % 4;
            fx.mem.set16(t + 0x34, v);
            l2::rgb(fx, t + 0x28, 0xF0, 0x60, 0x60);
        }
        let v = fx.rand_from(t, 0x01E0_2E1C) % 8;
        fx.mem.set16(t + 0xA, v);
        let s = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xC, s);
        for (o, ra, m, add) in [(0x30u32, 0x01E0_2E40u32, 0x64, 0x64), (0xE, 0x01E0_2E60, 0xA, 7), (8, 0x01E0_2E80, 0x23, 0), (6, 0x01E0_2E9C, 3, 2), (0x3A, 0x01E0_2EBC, 0x168, 0)] {
            let v = rmod(fx, t, ra, m);
            fx.mem.set16(t + o, v + add);
        }
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x2B, 0x80);
    }
    if fx.mem.i16(t + 0xC) == 0 {
        let a = age(fx, t);
        if fx.div(a, 3).1 == 1 && fx.mem.i16(t + 0x34) < 7 {
            let f = fx.mem.i16(t + 0x34) + 1;
            fx.mem.set16(t + 0x34, f);
        }
    }
    if age(fx, t) != 0 {
        let r = fx.mem.i16(t + 0x30);
        if r > 0 {
            let e = fx.mem.i16(t + 0xE);
            fx.mem.set16(t + 0x30, r - e);
        } else {
            kill(fx, t);
        }
    }
    if age(fx, t) != 0 {
        let a = fx.mem.i16(t + 0x3A) - 0xA;
        fx.mem.set16(t + 0x3A, a);
    }
    grow(fx, t);
}

/// 0x1E02FB0: a ring of quads at node 0x41 opening out (slot 0: size 12
/// growing by 1, white, radius 0..99 growing by 1..3; slot 1: size 8,
/// grey 0x20, radius 0..24 growing by 2; cell 0..6, row offset 0..7, row
/// +8 0..34, life +6 2..4); alpha 0x80 for its life, then out by 0xF.
fn ring(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        let s0 = fx.mem.i16(t + 0x30) == 0;
        fx.mem.set16(t + 0x38, if s0 { 0xC } else { 8 });
        let v = rmod(fx, t, 0x01E0_2FF4, 7);
        fx.mem.set16(t + 0x34, v);
        let g = if fx.mem.i16(t + 0x30) == 0 { 0x80 } else { 0x20 };
        l2::rgb(fx, t + 0x28, g, g, g);
        let v = fx.rand_from(t, 0x01E0_3050) % 8;
        fx.mem.set16(t + 0xA, v);
        let s = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xC, s);
        let v = if fx.mem.i16(t + 0x30) == 0 { rmod(fx, t, 0x01E0_3084, 0x64) } else { rmod(fx, t, 0x01E0_30A8, 0x19) };
        fx.mem.set16(t + 0x30, v);
        let e = if fx.mem.i16(t + 0xC) == 0 { rmod(fx, t, 0x01E0_30D4, 3) + 1 } else { 2 };
        fx.mem.set16(t + 0xE, e);
        let v = rmod(fx, t, 0x01E0_3104, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_3120, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x3A, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fall(fx, t, 0x2B, 0x11, 0xF, true);
    }
    if age(fx, t) != 0 {
        let v = fx.mem.i16(t + 0x38) + 1;
        fx.mem.set16(t + 0x38, v);
    }
    if age(fx, t) != 0 {
        let v = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, v);
    }
    grow(fx, t);
}

/// 0x1E031E0: a white / red (0x80, 0x10, 0x10) quad pair (size 6, 6 x 20
/// growing by 0x11) in by 0x40 for 4 updates, then out by 9.
fn sprite(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 6);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x2C, 0x80, 0x10, 0x10);
        fx.mem.set16(t + 8, 6);
        fx.mem.set16(t + 0xA, 0x14);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if age(fx, t) < 4 {
        rise(fx, t, 0x2B, 0x40);
        rise(fx, t, 0x2F, 0x40);
    } else {
        fall(fx, t, 0x2B, 0xB, 9, true);
        fall(fx, t, 0x2F, 0xB, 9, false);
    }
    if age(fx, t) != 0 {
        let v = fx.mem.i16(t + 0xA) + 0x11;
        fx.mem.set16(t + 0xA, v);
    }
    grow(fx, t);
}

/// 0x1E03310 (frame `sp`): a dome band ((0x80, 0x40, 0x40), random
/// angles, latitude 90 rising by 0..6 to 270, size D+0x3D3E): in by 0x80
/// for 8 updates, then out by 9.
fn dome_step(fx: &mut Fx, sp: u32, t: u32) {
    l2::rgb(fx, t + 0x28, 0x80, 0x40, 0x40);
    if first(fx, t) {
        let h = fx.self_handle();
        fx.unit_rotation(h, sp + 0x30);
        for (o, ra) in [(0x10u32, 0x01E0_3368u32), (0x14, 0x01E0_3384), (0x18, 0x01E0_33A0)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x30, 0x5A);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0);
        let s = fx.mem.i16(SIZE);
        fx.mem.set16(t + 0x38, s);
    }
    if age(fx, t) < 8 {
        rise(fx, t, 0x2B, 0x80);
    } else {
        fall(fx, t, 0x2B, 0xB, 9, true);
    }
    grow(fx, t);
    if fx.mem.i16(t + 0x30) < 0x10E {
        let r = rmod(fx, t, 0x01E0_3448, 7);
        let v = fx.mem.i16(t + 0x30) + (r as i16 as i32);
        fx.mem.set16(t + 0x30, v);
    } else {
        kill(fx, t);
    }
}

/// 0x1E03490: a ring band burst at its point (slots 0 / 1: blue (0x10,
/// 0x10, 0x80) in, grey 0x30 out, random angles (slot 1 0x4000, 0, 0),
/// inner radius 45 behind; slot 2: cyan (0x10, 0x80, 0x80) in, (0x10,
/// 0x10, 0x10) out, random angles, inner 0): radius 20 growing by 10, +0x2F
/// up by 0xC for 18 (slot 2: 30) updates, then down by 9 (slot 2: 7).
fn burst(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x14);
        if fx.mem.i16(t + 0x30) != 2 {
            l2::rgb(fx, t + 0x28, 0x10, 0x10, 0x80);
            l2::rgb(fx, t + 0x2C, 0x30, 0x30, 0x30);
        } else {
            l2::rgb(fx, t + 0x28, 0x10, 0x80, 0x80);
            l2::rgb(fx, t + 0x2C, 0x10, 0x10, 0x10);
        }
        let s = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xE, s);
        let s = fx.mem.i16(t + 0x30);
        if s == 0 || s == 2 {
            for (o, ra) in [(0x10u32, 0x01E0_3544u32), (0x14, 0x01E0_3560), (0x18, 0x01E0_357C)] {
                let v = rmod(fx, t, ra, 0xFFFF);
                fx.mem.set32(t + o, v);
            }
        } else {
            fx.mem.set32(t + 0x10, 0x4000);
            fx.mem.set32(t + 0x14, 0);
            fx.mem.set32(t + 0x18, 0);
        }
        let v = rmod(fx, t, 0x01E0_35B0, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0);
        fx.mem.set16(t + 6, 0);
        let life = if fx.mem.i16(t + 0x30) == 2 { 0x1E } else { 0x12 };
        fx.mem.set16(t + 8, life);
    }
    if age(fx, t) < fx.mem.i16(t + 8) {
        rise(fx, t, 0x2F, 0xC);
    } else if fx.mem.i16(t + 0x30) == 2 {
        fall(fx, t, 0x2F, 9, 7, true);
    } else {
        fall(fx, t, 0x2F, 0xB, 9, true);
    }
    if fx.mem.i16(t + 0xE) != 2 {
        let v = fx.mem.i16(t + 0x38) - 0x2D;
        fx.mem.set16(t + 6, v);
    }
    if fx.mem.i16(t + 6) < 0 {
        fx.mem.set16(t + 6, 0);
    }
    if age(fx, t) != 0 {
        let v = fx.mem.i16(t + 0x38) + 0xA;
        fx.mem.set16(t + 0x38, v);
    }
    grow(fx, t);
}

/// The three trails' points about the carried point: radius `r` (3
/// growing by 0.6 below 6) at angles a, a + 120, a + 240.
fn trail_point(fx: &mut Fx, sp: u32, a: i32, r: f32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(POINT + 4 * k);
        fx.mem.write_u32(sp + 0x80 + 4 * k, w);
    }
    let c = l2::sin_deg(fx, SINES, a, 1);
    let y = ee::add(fx.mem.f(sp + 0x84), ee::mul(r, c));
    fx.mem.setf(sp + 0x84, y);
    let s = l2::sin_deg(fx, SINES, a, 0);
    let x = ee::add(fx.mem.f(sp + 0x80), ee::mul(r, s));
    fx.mem.setf(sp + 0x80, x);
}

/// 0x1E036E0 (frame `sp`): three 30-point trails D+0x3540 + 0x1E0 j
/// circling the carried point (the angle +0x3A turning by 8; red (0x80,
/// 0x40, 0x40) / (0x80, 0x80, 0x40) by parity, alpha 0xC0 to update 0xF0,
/// then out by 5; 8 points growing by 2 to 0x1C, size 8), pushed until
/// update 0xE6.
fn trails(fx: &mut Fx, sp: u32, t: u32) {
    let step = f32::from_bits(0x3F19_999A);
    if first(fx, t) {
        l2::rgb(fx, t + 0x28, 0x80, 0x40, 0x40);
        fx.mem.set16(t + 6, 8);
        fx.mem.write_u8(t + 0x2B, 0);
        fx.mem.set16(t + 0x34, 0);
        let v = rmod(fx, t, 0x01E0_3748, 0x168);
        let mut a = v as i16 as i32;
        fx.mem.set16(t + 0x3A, v);
        let mut r = 3.0f32;
        for j in 0..3u32 {
            trail_point(fx, sp, a, r);
            let h = HIST3 + 0x1E0 * j;
            for i in 0..0x1D {
                copy3(fx, h + 0x10 * i, sp + 0x80);
            }
            a = (a + 0x78) as i16 as i32;
            if ee_lt(r, 6.0) {
                r = ee::add(r, step);
            }
        }
        fx.mem.set16(t + 0x38, 8);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) != 0 && fx.mem.i16(t + 6) < 0x1C {
        let v = fx.mem.i16(t + 6) + 2;
        fx.mem.set16(t + 6, v);
    }
    if counter(fx) < 0xF0 {
        fx.mem.write_u8(t + 0x2B, 0xC0);
    } else {
        fall(fx, t, 0x2B, 7, 5, true);
    }
    if age(fx, t) % 2 == 1 {
        l2::rgb(fx, t + 0x28, 0x80, 0x40, 0x40);
    } else {
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x40);
    }
    if counter(fx) < 0xE6 {
        let mut a = fx.mem.i16(t + 0x3A);
        let mut r = 3.0f32;
        for j in 0..3u32 {
            trail_point(fx, sp, a, r);
            bspawn::history(fx, D + 0x3710 + 0x1E0 * j, 0x1E, sp + 0x80);
            a = (a + 0x78) as i16 as i32;
            if ee_lt(r, 6.0) {
                r = ee::add(r, step);
            }
        }
    }
    let v = fx.mem.i16(t + 0x3A) + 8;
    fx.mem.set16(t + 0x3A, v);
    grow(fx, t);
}

/// 0x1E03A70: the carried point's 30-point trail D+0x3AE0 (yellow-red
/// (0xF0, 0x40, 0x40) / (0xF0, 0xF0, 0x40) by parity, alpha 0xC0 to update
/// 0xF0, then out by 5 while the size 0x2D shrinks by 1; 8 points growing
/// by 2 to 0x1C), pushed until update 0xE6.
fn trail(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 6, 8);
        fx.mem.write_u8(t + 0x2B, 0);
        for i in 0..0x1D {
            copy3(fx, HIST1 + 0x10 * i, POINT);
        }
        fx.mem.set16(t + 0x38, 0x2D);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) != 0 && fx.mem.i16(t + 6) < 0x1C {
        let v = fx.mem.i16(t + 6) + 2;
        fx.mem.set16(t + 6, v);
    }
    if counter(fx) < 0xF0 {
        fx.mem.write_u8(t + 0x2B, 0xC0);
    } else {
        fall(fx, t, 0x2B, 7, 5, true);
        let s = fx.mem.i16(t + 0x38);
        if s > 0 {
            fx.mem.set16(t + 0x38, s - 1);
        }
    }
    if age(fx, t) % 2 == 0 {
        l2::rgb(fx, t + 0x28, 0xF0, 0x40, 0x40);
    } else {
        l2::rgb(fx, t + 0x28, 0xF0, 0xF0, 0x40);
    }
    if counter(fx) < 0xE6 {
        bspawn::history(fx, D + 0x3CB0, 0x1E, POINT);
    }
    grow(fx, t);
}

/// 0x1E03BF0: a red (0xFF, 0x10, 0x10) spark ring at its point (size 10,
/// cell 0..3 (slot 0 counting to 7), row offset 0..1 then 0..7 an update,
/// radius 40..79 (+0xE 1..2 unused), row +8 0..34, life +6 8..15,
/// orientation +4); alpha 0x80 for its life, then out by 9.
fn spark(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0xA);
        let v = fx.rand_from(t, 0x01E0_3C18) % 4;
        fx.mem.set16(t + 0x34, v);
        l2::rgb(fx, t + 0x28, 0xFF, 0x10, 0x10);
        let v = fx.rand_from(t, 0x01E0_3C4C) % 2;
        fx.mem.set16(t + 0xA, v);
        let s = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xC, s);
        let v = rmod(fx, t, 0x01E0_3C70, 0x28);
        fx.mem.set16(t + 0x30, v + 0x28);
        let v = fx.rand_from(t, 0x01E0_3C90) % 2;
        fx.mem.set16(t + 0xE, v + 1);
        let v = rmod(fx, t, 0x01E0_3CB0, 0x23);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, 0x01E0_3CCC) % 8;
        fx.mem.set16(t + 6, v + 8);
        let v = rmod(fx, t, 0x01E0_3CEC, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fall(fx, t, 0x2B, 0xB, 9, true);
    }
    if fx.mem.i16(t + 0xC) == 0 {
        let a = age(fx, t);
        if fx.div(a, 3).1 == 1 && fx.mem.i16(t + 0x34) < 7 {
            let f = fx.mem.i16(t + 0x34) + 1;
            fx.mem.set16(t + 0x34, f);
        }
    }
    let v = fx.rand_from(t, 0x01E0_3D9C) % 8;
    fx.mem.set16(t + 0xA, v);
    let v = fx.mem.i16(t + 0x3A) + 4;
    fx.mem.set16(t + 0x3A, v);
    grow(fx, t);
}

/// 0x1E03DE0 / 0x1E03F90's set-up: size `size`, cell 0..3, white, row
/// offset 0..1, radius 40 + rand % `m`, +0xE 3..7, row +8 0..34, life +6
/// `life` + 0..3, a random orientation.
fn ring_set(fx: &mut Fx, t: u32, size: i32, m: i32, life: i32, ras: [u32; 7]) {
    fx.mem.set16(t + 0x38, size);
    let v = fx.rand_from(t, ras[0]) % 4;
    fx.mem.set16(t + 0x34, v);
    l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
    let v = fx.rand_from(t, ras[1]) % 2;
    fx.mem.set16(t + 0xA, v);
    let s = fx.mem.i16(t + 0x30);
    fx.mem.set16(t + 0xC, s);
    let v = rmod(fx, t, ras[2], m);
    fx.mem.set16(t + 0x30, v + 0x28);
    let v = rmod(fx, t, ras[3], 5);
    fx.mem.set16(t + 0xE, v + 3);
    let v = rmod(fx, t, ras[4], 0x23);
    fx.mem.set16(t + 8, v);
    let v = fx.rand_from(t, ras[5]) % 4;
    fx.mem.set16(t + 6, v + life);
    let v = rmod(fx, t, ras[6], 0x168);
    fx.mem.set16(t + 0x3A, v);
    fx.mem.set16(t + 0x36, 2);
}

/// Alpha in by 0x20 below 0x80 for the task's life, then out by 9.
fn ring_alpha(fx: &mut Fx, t: u32) {
    if age(fx, t) < fx.mem.i16(t + 6) {
        rise(fx, t, 0x2B, 0x20);
    } else {
        fall(fx, t, 0x2B, 0xB, 9, true);
    }
}

/// 0x1E03DE0: a white ring at the point (size 12, radius 40..239 growing
/// by +0xE, life 3..6).
fn ring_b(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        ring_set(fx, t, 0xC, 0xC8, 3, [0x01E0_3E08, 0x01E0_3E3C, 0x01E0_3E60, 0x01E0_3E80, 0x01E0_3EA0, 0x01E0_3EBC, 0x01E0_3EDC]);
    }
    ring_alpha(fx, t);
    if age(fx, t) != 0 {
        let v = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, v);
    }
    grow(fx, t);
}

/// 0x1E03F90: a white ring closing on the point (size 3, radius 40..439
/// closing by +0xE while positive, life 1..4).
fn ring_c(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        ring_set(fx, t, 3, 0x190, 1, [0x01E0_3FB8, 0x01E0_3FEC, 0x01E0_4010, 0x01E0_4030, 0x01E0_4050, 0x01E0_406C, 0x01E0_408C]);
    }
    ring_alpha(fx, t);
    if age(fx, t) != 0 {
        let r = fx.mem.i16(t + 0x30);
        if r > 0 {
            let v = r - fx.mem.i16(t + 0xE);
            fx.mem.set16(t + 0x30, v);
        }
    }
    grow(fx, t);
}

/// 0x1E04140: a `ring_band_t20` glow at the point (radius 300, grey 0x20
/// in, (0x30, 0x30, 0x10) out) in by 0x20 below 0x80 for 22 updates, then
/// out by 7, facing the camera turned 0x4000.
fn red(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x12C);
        l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x20);
        l2::rgb(fx, t + 0x2C, 0x30, 0x30, 0x10);
        let v = rmod(fx, t, 0x01E0_4194, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if age(fx, t) < 0x16 {
        rise(fx, t, 0x2B, 0x20);
    } else {
        fall(fx, t, 0x2B, 9, 7, true);
    }
    crate::b494::camera_angles_saving(fx, t + 0x10, u64::from(t), 0, 0x01E0_4210);
    let x = fx.mem.i32(t + 0x10).wrapping_add(0x4000);
    fx.mem.set32(t + 0x10, x);
    grow(fx, t);
}

/// 0x1E04240 (frame `sp`): camera slots 4 / 5 (0x1F7E20) from D+0x34F0
/// (128, 4, 2; the carried point; 0, 100, w rising by 0.05 to 1, then
/// flickering (128, 4, 2) / (96, 10, 5) by parity until update 0x11C, then
/// w down by 0.1); cleared when the task ends (w at 0.1, or the timer at
/// 1).
fn light(fx: &mut Fx, sp: u32, t: u32) {
    if first(fx, t) {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x4080_0000), (8, 0x4000_0000), (0xC, 0), (0x20, 0), (0x24, 0x42C8_0000), (0x28, 0)] {
            fx.mem.write_u32(CAM + o, v);
        }
        fx.mem.set16(t + 0x36, 2);
        fx.mem.set16(t + 0x30, 0);
    }
    copy3(fx, CAM + 0x10, POINT);
    let w = fx.mem.f(CAM + 0x28);
    if counter(fx) < 0x11C {
        if ee_lt(w, 1.0) {
            fx.mem.setf(CAM + 0x28, ee::add(w, f32::from_bits(0x3D4C_CCCD)));
        } else if age(fx, t) % 2 == 1 {
            fx.mem.write_u32(CAM + 4, 0x4080_0000);
            fx.mem.write_u32(CAM + 8, 0x4000_0000);
            fx.mem.write_u32(CAM, 0x4300_0000);
        } else {
            fx.mem.write_u32(CAM + 4, 0x4120_0000);
            fx.mem.write_u32(CAM + 8, 0x40A0_0000);
            fx.mem.write_u32(CAM, 0x42C0_0000);
        }
    } else {
        let tenth = f32::from_bits(0x3DCC_CCCD);
        if ee_le(w, tenth) {
            kill(fx, t);
        } else {
            fx.mem.setf(CAM + 0x28, ee::sub(w, tenth));
        }
    }
    if fx.mem.i32(TIMER) == 1 {
        kill(fx, t);
    }
    let (p, ra) = if fx.mem.i16(t + 0x36) == 0 { (0, 0x01E0_43B0) } else { (CAM, 0x01E0_43C4) };
    crate::blib::exe::camera_slots45(fx, p);
    // 0x1F7E20's frame (s0 = the task, ra).
    fx.saved(sp - 0x20, &[(0, u64::from(t)), (0x10, ra)]);
    grow(fx, t);
}

/// 0x1E044C0 (frame `sp`): b202's window sprite (0x6FF8 .. 0x8FF8)
/// sampling the frame (0x207EF0, filter 1) at ST 0..1, white (alpha
/// +0x2B), ALPHA 0x44, key 0; in by 8 below 0x40 until age 0xC8, then out
/// by 9.
fn flash(fx: &mut Fx, sp: u32, t: u32) {
    let d = sp + 0x28;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for k in 0..3 {
        fx.mem.write_u8(sp + 0x38 + k, 0x80);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    fx.mem.write_u8(sp + 0x3B, a);
    fx.mem.write_u32(sp + 0x3C, 0x3F80_0000);
    for (at, v) in [(sp + 0x48, 0x6FF8u16), (sp + 0x58, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    for (o, v) in [(0x40u32, 0u32), (0x44, 0), (0x50, 0x3F80_0000), (0x54, 0x3F80_0000)] {
        fx.mem.write_u32(sp + o, v);
    }
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if age(fx, t) < 0xC8 {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x40 {
            fx.mem.write_u8(t + 0x2B, a + 8);
        }
    } else {
        fall(fx, t, 0x2B, 0xB, 9, true);
    }
    grow(fx, t);
}
