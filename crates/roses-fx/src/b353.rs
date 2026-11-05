//! BMODULE b353 (card 188), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01E40, data area 0x1ED0000) [F]: b564's two-pool program (150 x 0x40 at D+0x380 by direct `jal`,
//! 150 x 0x70 at D+0x2900 by `jalr`) with its own trail. Update 68 two
//! blue trails (0x1E01010) start at the attacker's node 1: each a ten-point
//! history of a point circling the node (radius 7 / 5, 15 degrees a
//! update, the two in opposite senses, shifted 1.5 along z by the facing)
//! drawn as a gouraud tube (0x1E02660, b701's tube through a node
//! translation, nudged 0.05, keyed by its mean w) until update 78, fading
//! out after. Update 75 at node 0x1D halfway to the target: eight violet
//! `ring_t20` rings (0x1E016A0), fifty violet quads flying off
//! (0x1E01830), a violet arc ring (0x1E01AC0) and a camera light
//! (0x1E01C00, b701's at D+0x6AA0). Returns 1 on update 65, 4 on 115, 2 on
//! 75, 8 once the 120-update timer D+0x6DC0 has run out.
use crate::b076::{node_position_saving, request_node_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, exe::camera_slots45, spawn::history, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
const POOL_A: u32 = D + 0x380;
const POOL_B: u32 = D + 0x2900;
/// Pool A's steps.
const TRAIL: u32 = 0x01E0_1010;
const RING: u32 = 0x01E0_16A0;
const CAMERA: u32 = 0x01E0_1C00;
/// Pool B's steps.
const FLY: u32 = 0x01E0_1830;
const ARC: u32 = 0x01E0_1AC0;
/// A step that is never spawned (counted by the pool B loop).
const IDLE: u32 = 0x01E0_1D80;
/// The return address of the pool B loop's `jalr` (0x1E0226C + 8).
const TASK_RA: u64 = 0x01E0_2274;
/// The trails' histories (ten points, 0xA0 bytes a trail).
const HIST: u32 = D + 0x6AF0;
const CAM: u32 = D + 0x6AA0;
const NODE: u32 = D + 0x6D80;
const TEX5: u32 = D + 0x6DA0;
const TIMER: u32 = D + 0x6DC0;
const COLOUR: u32 = D + 0x6DC4;
const NODES: u32 = D + 0x6E0C;
const KIND: u32 = D + 0x6E26;
const FACING: u32 = D + 0x6E2A;
const COUNTER: u32 = D + 0x6E2C;
const BYTES: u32 = D + 0x6E30;
const SINES: u32 = D + 0x7030;
/// Texture (4, 0xF).
const TEX_F: u32 = D + 0x37C;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E01E40.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x130);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x6DCC, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x130);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x71A0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x6E28, param);
    fx.mem.set32(TIMER, 0x78);
    for o in [0x310u32, 0x314, 0x318] {
        fx.mem.write_u32(D + o, 0);
    }
    let s = [Some(u64::from(D)), Some(0), Some(0)];
    for (k, (node, ra)) in [(1, 0x01E0_1EC8u32), (0x1D, 0x01E0_1EE0), (0x20, 0x01E0_1EF8), (4, 0x01E0_1F10), (5, 0x01E0_1F28), (6, 0x01E0_1F40)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = request_node_saving(fx, h, node, i32::from(k == 0), s, ra);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    textures(fx);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x90);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x94)), 180.0);
    fx.fix_residue(y, 0x01E0_1F8C);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_1F98, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_1FCC, 0x1E);
        fx.mem.set8(BYTES + 0x100 + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, D, 0x01E0_2000, 0x14);
        fx.mem.set8(BYTES + 0x100 + k, v - 0xA);
    }
    0
}

/// 0x1E01D90(0, D): the effect textures 0..2 at D+0x340, 0xB..0xF at
/// D+0x36C, the attacker's 0..1 at D+0x6DA0.
fn textures(fx: &mut Fx) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_1F50), (0x20, 0), (0x10, 0), (0, u64::from(D))]);
    for k in 0..3 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x340 + 4 * k as u32, t);
    }
    for k in 0xB..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x340 + 4 * k as u32, t);
    }
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX5 + 4 * k as u32, t);
    }
    fx.leave(0x40);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    for k in 0..TASKS {
        let t = POOL_A + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x24) == TRAIL {
            let s = fx.enter(0x50);
            fx.saved(s, &[(0x30, 0x01E0_2070), (0x20, 0), (0x10, 0), (0, u64::from(D))]);
            trail(fx, s, t);
            fx.leave(0x50);
        }
        if fx.mem.read_u32(t + 0x24) == RING {
            let s = fx.enter(0x20);
            fx.saved(s, &[(0x10, 0x01E0_2090), (0, u64::from(D))]);
            ring(fx, t);
            fx.leave(0x20);
        }
        if fx.mem.read_u32(t + 0x24) == CAMERA {
            let s = fx.enter(0x30);
            fx.saved(s, &[(0x20, 0x01E0_20B0), (0x10, 0), (0, u64::from(D))]);
            camera(fx, t);
            fx.leave(0x30);
        }
    }
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x60, u64::from(D), 0x01E0_20D4);
    for k in 0..TASKS {
        let t = POOL_A + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x24) == TRAIL {
            if fx.mem.i16(t + 0x30) == 0 {
                fx.mem.write_u32(D + 4, 0);
            }
            if fx.mem.i16(t + 0x30) == 1 {
                fx.mem.write_u32(D + 4, 0x3F80_0000);
            }
            if fx.mem.i16(COUNTER) < 0x4C {
                let h = fx.self_handle();
                let slot = fx.mem.i16(NODES);
                node_position_saving(fx, h, slot, NODE, u64::from(D), Some(0), 0x01E0_2150);
            }
            for k in 0..4 {
                let w = fx.mem.read_u32(NODE + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, w);
            }
            fx.mem.write_u32(D, 0x4020_0000);
            let z = if fx.mem.i16(FACING) != 0 { 0xC060_0000 } else { 0x4060_0000 };
            fx.mem.write_u32(D + 8, z);
            let idx = fx.mem.i16(t + 0x30);
            let p = (HIST as i32 + idx * 0xA0) as u32;
            tube(fx, t, p, HIST);
        }
        if fx.mem.read_u32(t + 0x24) == RING {
            let w = fx.mem.read_u32(TEX_F);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x34);
            cell(fx, sp + 0x70, sub);
            billboard::ring_t20(fx, uv::uv_rect4, D, t, sp + 0xA0, BYTES);
        }
    }
    for k in 0..TASKS {
        let t = POOL_B + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 || f == IDLE {
            continue;
        }
        match f {
            FLY => {
                let s = fx.enter(0x20);
                fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]);
                fly(fx, t);
                fx.leave(0x20);
            }
            ARC => {
                let s = fx.enter(0x30);
                fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
                arc(fx, t);
                fx.leave(0x30);
            }
            _ => panic!("b353: unknown task step {f:#x}"),
        }
    }
    for k in 0..TASKS {
        let t = POOL_B + 0x70 * k;
        // The original reads the cell frame of pool A's task k (+0x34).
        let ta = POOL_A + 0x40 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x44) == FLY {
            let w = fx.mem.read_u32(TEX_F);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(ta + 0x34);
            cell(fx, sp + 0x80, sub);
            billboard::quad(fx, uv::uv_rect4, D, t, 0x20, t + 0x10);
        }
        if fx.mem.read_u32(t + 0x44) == ARC {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x41 => 1,
        0x73 => 4,
        0x4B => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell 0x9B frame `sub` (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, sub: i32) {
    l2::rect(fx, r, 0x9B, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// 0x1E00EA0(D, 0) (frame 0x50: the entry's s0 = D / s1 = 0): the spawn
/// colour (0x80, 0, 0, 0), the update counter and the emitters.
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, 0x01E0_203C), (0x10, 0), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if fx.mem.i16(COUNTER) == 0x44 {
        fx.mem.set16(KIND, 0);
        spawn_a(fx, TRAIL, 0, 0x01E0_0EF4);
        fx.mem.set16(KIND, 1);
        spawn_a(fx, TRAIL, 0, 0x01E0_0F10);
    }
    if fx.mem.i16(COUNTER) == 0x4B {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2);
        node_position_saving(fx, h, slot, sp + 0x30, u64::from(D), Some(0), 0x01E0_0F38);
        let h = fx.target_handle();
        unit_position_saving(fx, h, sp + 0x40, u64::from(D), 0x01E0_0F4C);
        let x = fx.mem.read_u32(sp + 0x30);
        fx.mem.write_u32(D + 0x2D0, x);
        let y = fx.mem.read_u32(sp + 0x34);
        fx.mem.write_u32(D + 0x2D4, y);
        let z = fx.mem.f(sp + 0x38);
        let z = ee::add(z, ee::div(ee::sub(fx.mem.f(sp + 0x48), z), 2.0));
        fx.mem.setf(D + 0x2D8, z);
        fx.mem.set16(KIND, 1);
        for k in 0..8 {
            spawn_a(fx, RING, k, 0x01E0_0F9C);
        }
        for k in 0..0x32 {
            spawn_b(fx, FLY, k, 0x01E0_0FC4);
        }
        spawn_b(fx, ARC, 0, 0x01E0_0FE8);
        spawn_a(fx, CAMERA, 0, 0x01E0_0FFC);
    }
    fx.leave(0x50);
}

/// 0x1E02400(D, step, 0) (b538's spawn for 150 tasks at D+0x380): the
/// first free task cleared and set off at D+0x2D0 (as i16) with the colour
/// D+0x6DC4 (twice), +0x30 = D+0x6E26 (saves: the emitter's s0 = D / s1,
/// the entry's s2 = 0, ra).
fn spawn_a(fx: &mut Fx, f: u32, s1: u64, ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x20, 0), (0x10, s1), (0, u64::from(D))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(POOL_A + 0x40 * k + 0x36) == 0) {
        Some(k) => {
            let t = POOL_A + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            colour(fx, t, [0x28, 0x2C]);
            l2::to_i16x4(fx, t, D + 0x2D0, 0);
            let a = fx.mem.i16(KIND);
            fx.mem.set16(t + 0x30, a);
            fx.mem.write_u32(t + 0x20, 0);
            fx.mem.write_u32(t + 0x24, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// 0x1E02520(D, step, 0) (b272's spawn for 150 tasks at D+0x2900).
fn spawn_b(fx: &mut Fx, f: u32, s1: u64, ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x20, 0), (0x10, s1), (0, u64::from(D))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(POOL_B + 0x70 * k + 0x58) == 0) {
        Some(k) => {
            let t = POOL_B + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            colour(fx, t, [0x60, 0x64]);
            l2::to_i16x4(fx, t, D + 0x2D0, 0);
            for c in 0..3 {
                let w = fx.mem.read_u32(D + 0x2D0 + 4 * c);
                fx.mem.write_u32(t + 0x10 + 4 * c, w);
            }
            let a = fx.mem.i16(KIND);
            fx.mem.set16(t + 0x50, a);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

fn colour(fx: &mut Fx, t: u32, at: [u32; 2]) {
    for c in at {
        let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
        l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
        let a = fx.mem.read_u8(COLOUR + 3);
        fx.mem.write_u8(t + c + 3, a);
    }
}

/// The trail's circling point (0, 7 sin a, 5 cos a) into `v`.
fn circle(fx: &mut Fx, t: u32, v: u32) {
    let a = fx.mem.i16(t + 0x3A);
    let s = l2::sin_deg(fx, SINES, a, 0);
    fx.mem.setf(v + 4, ee::mul(7.0, s));
    fx.mem.write_u32(v, 0);
    let a = fx.mem.i16(t + 0x3A);
    let c = l2::sin_deg(fx, SINES, a, 1);
    fx.mem.setf(v + 8, ee::mul(5.0, c));
}

/// 0x1E01010 (D, task), frame 0x50 at `sp`: a blue (0x20, 0x20, 0xFF)
/// trail. First step: size 5, +0xE 30 (7 for trail 1), angle -90 (trail
/// 0) / 90 (trail 1), all ten history points (D+0x6AF0 + 0xA0 k) at the
/// circling point. In by 0x16 under 0x71 before update 79, then out by
/// 0x1D (ending under 0x1F); before update 79 the point turns 15 degrees
/// (trail 0 down / trail 1 up when facing, the other way when not) and is
/// pushed into the history; the length +6 is 10.
fn trail(fx: &mut Fx, sp: u32, t: u32) {
    l2::rgb(fx, t + 0x28, 0x20, 0x20, 0xFF);
    let v = sp + 0x40;
    let facing = fx.mem.i16(FACING) != 0;
    let shift = |fx: &mut Fx| {
        let z = fx.mem.f(v + 8);
        let z = if facing { ee::add(z, 1.5) } else { ee::sub(z, 1.5) };
        fx.mem.setf(v + 8, z);
    };
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 5);
        fx.mem.set16(t + 0xE, 0x1E);
        for (idx, deg) in [(0, -0x5A), (1, 0x5A)] {
            if fx.mem.i16(t + 0x30) != idx {
                continue;
            }
            fx.mem.set16(t + 0x3A, deg);
            circle(fx, t, v);
            shift(fx);
            let base = (HIST as i32 + fx.mem.i16(t + 0x30) * 0xA0) as u32;
            for k in 0..9 {
                for c in 0..3 {
                    let w = fx.mem.read_u32(v + 4 * c);
                    fx.mem.write_u32(base + 0x10 * k + 4 * c, w);
                }
            }
        }
        if fx.mem.i16(t + 0x30) == 1 {
            fx.mem.set16(t + 0xE, 7);
        }
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(COUNTER) < 0x4F {
        if a < 0x71 {
            fx.mem.write_u8(t + 0x2B, a + 0x16);
        }
    } else if a >= 0x1F {
        fx.mem.write_u8(t + 0x2B, a - 0x1D);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    if fx.mem.i16(COUNTER) < 0x4F {
        for idx in [0, 1] {
            if fx.mem.i16(t + 0x30) != idx {
                continue;
            }
            circle(fx, t, v);
            // trail 0 turns down when facing, trail 1 up (and the other
            // way round when not).
            let down = (idx == 0) == facing;
            let a = fx.mem.i16(t + 0x3A) + if down { -0xF } else { 0xF };
            fx.mem.set16(t + 0x3A, a);
            shift(fx);
            let p = (HIST as i32 + fx.mem.i16(t + 0x30) * 0xA0 + 0x90) as u32;
            history(fx, p, 0xA, v);
        }
    }
    fx.mem.set16(t + 6, 0xA);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// 0x1E016A0 (D, task): a violet (0x80, 0x40, 0xFF) ring of size 6 (cell
/// 0..3, random angles +0x18 / +0x10, a random angle +0x3A), radius
/// growing by 3..18 from the start, row 0..9, life 10..29: 0x80 during the
/// life, out by 10.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 6);
        let v = fx.rand_from(t, 0x01E0_16C8);
        fx.mem.set16(t + 0x34, v % 4);
        l2::rgb(fx, t + 0x28, 0x80, 0x40, 0xFF);
        let v = rmod(fx, t, 0x01E0_16FC, 0xFFFF);
        fx.mem.set32(t + 0x18, v);
        let v = rmod(fx, t, 0x01E0_1718, 0xFFFF);
        fx.mem.set32(t + 0x10, v);
        fx.mem.set16(t + 0x30, 0);
        let v = rmod(fx, t, 0x01E0_1738, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = fx.rand_from(t, 0x01E0_1754);
        fx.mem.set16(t + 0xE, v % 0x10 + 3);
        let v = rmod(fx, t, 0x01E0_1774, 0xA);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_1790, 0x14);
        fx.mem.set16(t + 6, v + 0xA);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0x11 {
            fx.mem.write_u8(t + 0x2B, a - 0xA);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x30, r);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// 0x1E01C00 (D, task): b701's camera light at D+0x6AA0.
fn camera(fx: &mut Fx, t: u32) {
    let sp = fx.sp;
    if fx.mem.i16(t + 0x36) == 1 {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x4080_0000), (8, 0x4000_0000), (0xC, 0), (0x20, 0), (0x24, 0x42F0_0000), (0x28, 0)] {
            fx.mem.write_u32(CAM + o, v);
        }
        for k in 0..3 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(CAM + 0x10 + 4 * k, v);
        }
        fx.mem.set16(t + 0x36, 2);
        fx.mem.set16(t + 0x30, 0);
    }
    let w = fx.mem.f(CAM + 0x28);
    if fx.mem.i16(t + 0x30) == 0 {
        if w < 1.0 {
            fx.mem.setf(CAM + 0x28, ee::add(w, f32::from_bits(0x3E4C_CCCD)));
        } else {
            fx.mem.set16(t + 0x30, 1);
        }
    } else if w <= f32::from_bits(0x3DCC_CCCD) {
        fx.mem.set16(t + 0x36, 0);
    } else {
        fx.mem.setf(CAM + 0x28, ee::sub(w, f32::from_bits(0x3DCC_CCCD)));
    }
    let (p, ra) = if fx.mem.i16(t + 0x36) == 0 { (0, 0x01E0_1D34) } else { (CAM, 0x01E0_1D48) };
    camera_slots45(fx, p);
    // 0x1F7E20's frame (s0 = the task, ra).
    fx.saved(sp - 0x20, &[(0, u64::from(t)), (0x10, ra)]);
    if fx.mem.i32(TIMER) == 1 {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// 0x1E01830 (D, task): a small violet quad (size 4..11, a random cell
/// row) flying (-50..49) / 60 a update, turned by the camera's angles; in
/// by 0x40 under 0x80 for 20 updates, out by 9.
fn fly(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = fx.rand_from(t, 0x01E0_1854);
        fx.mem.set16(t + 8, v % 8 + 4);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_187C);
        fx.mem.set16(t + 0x56, (v >> 4) % 3);
        l2::rgb(fx, t + 0x60, 0x80, 0x40, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        for (o, ra) in [(0x34u32, 0x01E0_18B0u32), (0x30, 0x01E0_18DC), (0x38, 0x01E0_1908)] {
            let v = rmod(fx, t, ra, 0x64);
            fx.mem.setf(t + o, ee::from_i32(v - 0x32));
        }
        for o in [0x34u32, 0x30, 0x38] {
            let q = ee::div(fx.mem.f(t + o), 60.0);
            fx.mem.setf(t + o, q);
        }
        let v = rmod(fx, t, 0x01E0_1960, 0x168);
        fx.mem.set16(t + 0x5C, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_197C);
        for (o, ra) in [(4u32, 0x01E0_1984u32), (2, 0x01E0_19A4), (0, 0x01E0_19C4)] {
            let v = fx.rand_from(t, ra);
            fx.mem.set16(t + o, v % 0x800 - 0x400);
        }
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0x14 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [0u32, 4, 8] {
            let v = ee::add(fx.mem.f(t + 0x10 + o), fx.mem.f(t + 0x30 + o));
            fx.mem.setf(t + 0x10 + o, v);
        }
    }
    for (o, by) in [(0x28u32, 4u32), (0x24, 2), (0x20, 0)] {
        let v = fx.mem.read_u32(t + o).wrapping_add(fx.mem.i16(t + by) as u32);
        fx.mem.write_u32(t + o, v);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_1AA0);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E01AC0 (D, task): a violet arc ring (outer white, radius 180 growing
/// by 0.4), its outer alpha in by 0x60 under 0xC0 for 8 updates, then out
/// by 15, spinning; ended on update 195.
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x4334_0000);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x60, 0x80, 0x40, 0xFF);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let a = fx.mem.read_u8(t + 0x67);
    if fx.mem.i16(t + 0x54) < 8 {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x67, a + 0x60);
        }
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x67, a - 0xF);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1B88);
    let s = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, s);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = ee::add(fx.mem.f(t + 0x4C), f32::from_bits(0x3ECC_CCCD));
        fx.mem.setf(t + 0x4C, r);
    }
    if fx.mem.i16(COUNTER) == 0xC3 {
        fx.mem.set16(t + 0x58, 0);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E02660(D, task, p, q) (b701's tube through the translation D+0x20):
/// for each segment k (k + 1 < n = i16 +6) of the points `p`, 18 rings of
/// 4 untextured gouraud strips (0x109BC0 additive, 0x109DD0) round the
/// segment at angles -90 + 10 j, radius D+0 (x) / D+8 (z), the strips'
/// first and third vertices nudged 0.05 in +z / -z / +x / -x; the colour
/// +0x28 at the first two vertices, its alpha then less alpha / n at the
/// last two; projected through TRS(translate D+0x20) x the camera
/// (0x1097D0, 0x204F70) and keyed by the mean w (0x1A2C70) [F].
fn tube(fx: &mut Fx, t: u32, p: u32, q: u32) {
    let sp = fx.enter(0x260);
    let ot = OT;
    let d = sp + 0x1C8;
    let n = fx.mem.i16(t + 6);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    for k in 0..3 {
        let w = fx.mem.read_u32(D + 0x20 + 4 * k);
        fx.mem.write_u32(sp + 0x1B0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x170, 0, 0, sp + 0x1B0);
    for k in 0..4 {
        let b = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(sp + 0x25C + k, b);
    }
    let a = sp + 0x150;
    for k in 0..4 {
        let w = fx.mem.read_u32(p + 4 * k);
        fx.mem.write_u32(a + 4 * k, w);
        let w = fx.mem.read_u32(q + 4 * k);
        fx.mem.write_u32(sp + 0x160 + 4 * k, w);
    }
    for c in 0..3 {
        let v = ee::sub(fx.mem.f(a + 4 * c), fx.mem.f(sp + 0x160 + 4 * c));
        fx.mem.setf(a + 4 * c, v);
    }
    let w = fx.mem.read_u32(D);
    fx.mem.write_u32(a, w);
    let y = ee::neg(fx.mem.f(a + 4));
    fx.mem.setf(a + 4, y);
    let w = fx.mem.read_u32(D + 8);
    fx.mem.write_u32(a + 8, w);
    let f23 = fx.mem.f(a + 8);
    let mut seg = p;
    let mut i = 0;
    while i < n - 1 {
        l2::colour_strip4(fx, d, sp + 0x25C, 1);
        let alpha = i32::from(fx.mem.read_u8(t + 0x2B));
        let qd = fx.div(alpha, n).0 & 0xFF;
        let v = fx.mem.read_u8(sp + 0x25F).wrapping_sub(qd as u8);
        fx.mem.write_u8(sp + 0x25F, v);
        l2::colour_strip4(fx, d, sp + 0x25C, 2);
        let mut deg = -0x5Ai32;
        for _ in 0..0x12 {
            let s = l2::sin_deg(fx, D + 0xD8, deg as i16 as i32, 0);
            let f21 = ee::mul(fx.mem.f(a), s);
            let c = l2::sin_deg(fx, D + 0xD8, deg as i16 as i32, 1);
            let f22 = ee::mul(f23, c);
            for s0 in 0..4 {
                let m = sp + 0x110;
                for (row, from) in [(0u32, seg), (0x10, seg), (0x20, seg + 0x10), (0x30, seg + 0x10)] {
                    for c in 0..3 {
                        let w = fx.mem.read_u32(from + 4 * c);
                        fx.mem.write_u32(m + row + 4 * c, w);
                    }
                }
                l2::add_column(fx, m, 0, f21);
                l2::add_column(fx, m, 2, f22);
                let nudge = f32::from_bits(0x3D4C_CCCD);
                let (o, up) = match s0 {
                    0 => (8u32, true),
                    1 => (8, false),
                    2 => (0, true),
                    _ => (0, false),
                };
                for row in [0u32, 0x20] {
                    let at = m + row + o;
                    let v = fx.mem.f(at);
                    fx.mem.setf(at, if up { ee::add(v, nudge) } else { ee::sub(v, nudge) });
                }
                if l2::project(fx, m, SPR, sp + 0x170, d, sp + 0x100) {
                    let w = fx.mem.f(sp + 0x100);
                    fx.fix_residue(w, 0x01E0_2A20);
                    let key = ee::fixunssfsi(w) as i32;
                    fx.submit(d, ot, key);
                }
            }
            deg += 0xA;
        }
        seg += 0x10;
        i += 1;
    }
    fx.leave(0x260);
}

/// The scratchpad (0x204F70's `out`).
const SPR: u32 = 0x7000_0000;
