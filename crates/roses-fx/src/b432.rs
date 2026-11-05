//! BMODULE b432 (card 44), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03890, data area 0x1ED0000) [F] (helper C18, 1
//! Nov 2026): b747 / b193's task pool (150 x 0x70 at +0x2A0, alive +0x58,
//! step +0x44 through `jalr`; spawned by 0x1E040F0, b193's spawn with the
//! colour +0x7EC4 and the index +0x7F22 into +0x50 / +0x52) on the second
//! library (`uv_rect`, `ring_facing_flat`, `arc_strips`, `quad`, `sparks`)
//! and the record's single-piece tube 0x1E04240
//! (`tubetrail::tube_with(SINGLE)`), with b232's unit flash (0x1E04C30:
//! the attacker drawn into the render target in a packet layer, then every
//! flash task 0x1E03660 stepped). The emitter 0x1E01920 counts +0x7F2A: on
//! update 1 the target's position to +0x7E90, the stage light (0x1E03360,
//! down to 0x60), a frame tint (0x1E03460) and the flash; on 0x60..0x9A
//! at the attacker's node 0x27 two flat rings (0x1E01BB0) and a turned
//! quad (0x1E02FA0), then the spawn point at node 0x3B; on 0x64 a ring
//! following node 0x27 (0x1E02DE0) and a camera light (0x1E031D0, slots 4
//! / 5); on 0x82..0xE6 at node 0x27 a white / red flare (0x1E020D0,
//! `sparks`) and an arc (0x1E022D0); from 0x94 every fourth update (n % 4
//! = 1), 30 times, a trail (0x1E02800: a head at +0x7CB0 + 0x10 k pulled
//! toward the target in 46 updates, swaying by the sines +0x812C, its
//! 30-point history +0x4470 + 0x1E0 k drawn as the tube; arcs 0x1E024D0 /
//! rings 0x1E01F20 at the head on its updates 46..50 and an arc 0x1E026D0
//! on 48) with two quads (0x1E01D80) riding its head. Returns 1 on update
//! 0xBE, 4 on 0x163, 2 on 0x14F, 6 on 0xC9..0x149, 8 once the 379-update
//! timer +0x7EC0 has run out.
use crate::b076::{node_position_saving, request_node_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, spawn::history, tubetrail, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const RING_A: u32 = 0x01E0_1BB0;
const QUADS: u32 = 0x01E0_1D80;
const RING_B: u32 = 0x01E0_1F20;
const FLARE: u32 = 0x01E0_20D0;
const ARC_A: u32 = 0x01E0_22D0;
const ARC_B: u32 = 0x01E0_24D0;
const ARC_C: u32 = 0x01E0_26D0;
const TRAIL: u32 = 0x01E0_2800;
const ARC_D: u32 = 0x01E0_2DE0;
const QUAD: u32 = 0x01E0_2FA0;
const CAMERA: u32 = 0x01E0_31D0;
const LIGHT: u32 = 0x01E0_3360;
const TINT: u32 = 0x01E0_3460;
const FLASH: u32 = 0x01E0_3660;
/// The return address of the entry's `jalr` into a step (0x1E03B40 + 8).
const TASK_RA: u64 = 0x01E0_3B48;
/// The return address of the entry's call into the emitter.
const EMIT_RA: u64 = 0x01E0_3B08;
/// The return address of the unit flash's call into a flash step.
const FLASH_RA: u64 = 0x01E0_4D04;
const SPAWN_AT: u32 = D + 0x250;
const TEX_A: u32 = D + 0x29C;
const CAM: u32 = D + 0x4440;
const TRAILS: u32 = D + 0x4470;
const HEADS: u32 = D + 0x7CB0;
const TARGET: u32 = D + 0x7E90;
const TEX: u32 = D + 0x7EA0;
const TIMER: u32 = D + 0x7EC0;
const COLOUR: u32 = D + 0x7EC4;
const LAYER: u32 = D + 0x7ED4;
const FLASH_DESC: u32 = D + 0x7ED8;
const NODE_3B: u32 = D + 0x7EF4;
const NODE_20: u32 = D + 0x7EF6;
const NODE_27: u32 = D + 0x7EF8;
const TNODE: u32 = D + 0x7F04;
const INDEX: u32 = D + 0x7F22;
const TRAIL_K: u32 = D + 0x7F24;
const FACING: u32 = D + 0x7F26;
const COUNTER: u32 = D + 0x7F2A;
const BYTES: u32 = D + 0x7F2C;
const BYTES2: u32 = D + 0x802C;
const SINES2: u32 = D + 0x812C;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// A trail's history (30 points of 16 bytes) by its index +0x50.
fn trail_of(fx: &mut Fx, t: u32) -> u32 {
    (TRAILS as i32 + 0x1E0 * fx.mem.i16(t + 0x50)) as u32
}

/// A trail's head (+0x7CB0 + 0x10 k) by its index +0x50.
fn head_of(fx: &mut Fx, t: u32) -> u32 {
    (HEADS as i32 + 0x10 * fx.mem.i16(t + 0x50)) as u32
}

/// The entry 0x1E03890 (a0 = D, a1 = param; s0 = D, s1 = s2 = 0, s3 =
/// param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xE0);
    fx.mem.write_u32(D + 0x7ECC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xE0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x82A0);
    l2::sine_table(fx, SINES2);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x7F28, param);
    fx.mem.set32(TIMER, 0x17B);
    for (k, node) in [0x3B, 0x20, 0x27, 0x26, 0x3F, 0x40].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODE_3B + 2 * k as u32, r);
    }
    // The last request's frame (s0 = D, s1 = s2 = 0), as b747's.
    let h = fx.target_handle();
    let r = request_node_saving(fx, h, 1, 1, [Some(u64::from(D)), Some(0), Some(0)], 0x01E0_39A0);
    fx.mem.set16(TNODE, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(TEX_A, t);
    let t = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, t);
    for k in 0..2u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xD0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xD4)), 180.0);
    fx.fix_residue(y, 0x01E0_3A40);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3A68, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3AB4, 0x28);
        fx.mem.set8(BYTES2 + k, v - 0x14);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    // The entry's s1: the flash tasks counted so far (saved by the steps).
    let mut flashes = 0u32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        match f {
            0 => {}
            FLASH => flashes = (flashes + 1) & 0xFFFF,
            RING_A => ring_a(fx, t, flashes),
            QUADS => quads(fx, t, flashes),
            RING_B => ring_b(fx, t),
            FLARE => flare(fx, t),
            ARC_A => arc_ab(fx, t, false),
            ARC_B => arc_ab(fx, t, true),
            ARC_C => arc_c(fx, t, flashes),
            TRAIL => trail(fx, t, flashes, slot, k),
            ARC_D => arc_d(fx, t, flashes),
            QUAD => quad(fx, t, flashes),
            CAMERA => camera(fx, t, flashes),
            LIGHT => light(fx, t),
            TINT => tint(fx, t, flashes),
            _ => panic!("b432: unknown task step {f:#x}"),
        }
    }
    // a3 as the draws' last library call left it, for the unit flash's
    // first TEXFLUSH (b491's lesson).
    let mut a3 = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let at_task = |fx: &mut Fx, y: bool| {
            let x = fx.mem.read_u32(t + 0x10);
            fx.mem.write_u32(D + 0x20, x);
            let y0 = fx.mem.f(t + 0x14);
            if y {
                let v = ee::add(y0, fx.mem.f(t + 0x34));
                fx.mem.setf(D + 0x24, v);
            } else {
                fx.mem.setf(D + 0x24, y0);
            }
            let z = fx.mem.read_u32(t + 0x18);
            fx.mem.write_u32(D + 0x28, z);
        };
        let ring = |fx: &mut Fx, r: u32, rows: i32| {
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, r, 0x14, sub);
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, rows);
            let a = fx.mem.i16(t + 0xA);
            fx.mem.set16(D + 0xC0, a);
            at_task(fx, true);
            quads4::ring_facing_flat(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
            0
        };
        let arcs = |fx: &mut Fx, mode: i32| {
            fx.mem.set16(D + 0xB8, 0x24);
            let before = fx.out.prims.len();
            billboard::arc_strips(fx, D, t, mode, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
            // Depth mode 1 (the far Z) leaves the last XYZ2, mode 0 (the
            // mean w) 0 [run].
            if mode & 0xF == 1 {
                last_xyz2(fx, before)
            } else {
                0
            }
        };
        if fx.mem.read_u32(t + 0x44) == RING_A {
            a3 = ring(fx, sp + 0x70, 3);
        }
        if fx.mem.read_u32(t + 0x44) == QUADS {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x80, 0x14, sub);
            fx.mem.set16(D + 0xBE, 0);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
            a3 = 0;
        }
        if fx.mem.read_u32(t + 0x44) == RING_B {
            a3 = ring(fx, sp + 0x90, 6);
        }
        if fx.mem.read_u32(t + 0x44) == FLARE {
            cell(fx, sp + 0xA0, 0x96, 2);
            let w = fx.mem.read_u32(TEX_A);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 0);
            at_task(fx, false);
            fx.mem.set16(D + 0xB8, 0xC);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sparks(fx, D, t, 0x20, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
            a3 = 0;
        }
        if fx.mem.read_u32(t + 0x44) == ARC_A {
            a3 = arcs(fx, 0x21);
        }
        if fx.mem.read_u32(t + 0x44) == ARC_B {
            a3 = arcs(fx, 0x21);
        }
        if fx.mem.read_u32(t + 0x44) == ARC_C {
            a3 = arcs(fx, 0x21);
        }
        if fx.mem.read_u32(t + 0x44) == TRAIL {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0xB0, 0x14, sub);
            let y = fx.mem.i16(D + 0x96) + 0xF;
            fx.mem.set16(D + 0x96, y);
            fx.mem.set16(D + 0x98, 0x1F);
            fx.mem.set16(D + 0x9A, 1);
            let h = fx.self_handle();
            unit_position_saving(fx, h, sp + 0x60, u64::from(D), 0x01E0_3F20);
            for k in 0..3 {
                fx.mem.write_u32(D + 0x20 + 4 * k, 0);
            }
            fx.mem.set16(D + 0xBE, 2);
            let a = trail_of(fx, t);
            tubetrail::tube_with(fx, &tubetrail::SINGLE, D, t, a, 0);
            a3 = 0;
        }
        if fx.mem.read_u32(t + 0x44) == ARC_D {
            a3 = arcs(fx, 0x20);
        }
        if fx.mem.read_u32(t + 0x44) == QUAD {
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0xC0, 0x14, sub);
            let o = fx.mem.i16(t + 0x52);
            fx.mem.set16(D + 0xBE, o);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
            a3 = 0;
        }
    }
    if flashes != 0 {
        unit_flash(fx, a3);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left.wrapping_sub(1));
    }
    match fx.mem.i16(COUNTER) {
        0xBE => 1,
        0x163 => 4,
        0x14F => 2,
        0xC9..=0x149 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The last XYZ2 of the last primitive drawn since `before` (0 when none),
/// as b491's.
fn last_xyz2(fx: &mut Fx, before: usize) -> u32 {
    match fx.out.prims.get(before..).and_then(|p| p.last()) {
        Some(p) => {
            let d = p.desc;
            let n = u32::from(fx.mem.read_u16(d));
            fx.mem.read_u32(d + 8 + 8 * n)
        }
        None => 0,
    }
}

/// The atlas cell `id` frame `sub` (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// 0x1E040F0(D, step, 0): the first free task cleared and set off at
/// +0x250 with the colour +0x7EC4 twice and the index +0x7F22 (+0x50,
/// +0x52). `saves`: its prologue's (0x60: ra, s4 .. s0).
fn spawn(fx: &mut Fx, f: u32, saves: &[(u32, u64)]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x70 * k + 0x2F8) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for c in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, SPAWN_AT, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(SPAWN_AT + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let a = fx.mem.i16(INDEX);
            fx.mem.set16(t + 0x50, a);
            let a = fx.mem.i16(INDEX);
            fx.mem.set16(t + 0x52, a);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// 0x1E01920(D, 0): the update counter and the spawns. Frame 0x20 (ra,
/// the entry's s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, EMIT_RA), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    // The spawns' saves: s0 = D, the entry's s1 = s2 = 0, s3 = the
    // parameter, s4 the runner's (0 [H, as b747]).
    let p = param as i64 as u64;
    let saves = |ra: u32| [(0x50u32, u64::from(ra)), (0x40, 0), (0x30, p), (0x20, 0), (0x10, 0), (0, u64::from(D))];
    let s1 = Some(0u64);
    let c = fx.mem.i16(COUNTER);
    if (0x60..0x9B).contains(&c) {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODE_27);
        node_position_saving(fx, h, slot, SPAWN_AT, u64::from(D), s1, 0x01E0_1970);
        fx.mem.set16(INDEX, 0);
        spawn(fx, RING_A, &saves(0x01E0_1988));
        spawn(fx, RING_A, &saves(0x01E0_199C));
        spawn(fx, QUAD, &saves(0x01E0_19B0));
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODE_3B);
        node_position_saving(fx, h, slot, SPAWN_AT, u64::from(D), s1, 0x01E0_19C8);
        fx.mem.set16(INDEX, 4);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x82..0xE7).contains(&c) {
        fx.mem.set16(INDEX, 1);
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODE_27);
        node_position_saving(fx, h, slot, SPAWN_AT, u64::from(D), s1, 0x01E0_1A08);
        spawn(fx, FLARE, &saves(0x01E0_1A1C));
        spawn(fx, ARC_A, &saves(0x01E0_1A30));
    }
    if fx.mem.i16(COUNTER) == 1 {
        let h = fx.target_handle();
        unit_position_saving(fx, h, TARGET, u64::from(D), 0x01E0_1A54);
    }
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, LIGHT, &saves(0x01E0_1A78));
    }
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, TINT, &saves(0x01E0_1A9C));
    }
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, FLASH, &saves(0x01E0_1AC0));
    }
    if fx.mem.i16(COUNTER) == 0x64 {
        spawn(fx, ARC_D, &saves(0x01E0_1AE4));
    }
    if fx.mem.i16(COUNTER) == 0x64 {
        spawn(fx, CAMERA, &saves(0x01E0_1B08));
    }
    let c = fx.mem.i16(COUNTER);
    if c >= 0x94 && c.wrapping_rem(4) == 1 && fx.mem.i16(TRAIL_K) < 0x1E {
        let k = fx.mem.i16(TRAIL_K);
        fx.mem.set16(INDEX, k);
        let h = fx.self_handle();
        let i = fx.mem.i16(INDEX);
        let slot = fx.mem.i16(NODE_27);
        node_position_saving(fx, h, slot, (HEADS as i32 + 0x10 * i) as u32, u64::from(D), s1, 0x01E0_1B6C);
        spawn(fx, TRAIL, &saves(0x01E0_1B80));
        spawn(fx, QUADS, &saves(0x01E0_1B94));
        let k = fx.mem.i16(TRAIL_K) + 1;
        fx.mem.set16(TRAIL_K, k);
    }
    fx.leave(0x20);
}

/// The alpha byte `o`'s fade: in by `up.1` below `up.0` while `rising`,
/// else out by `down.1` while at least `down.0`, else the task ends.
fn fade(fx: &mut Fx, t: u32, o: u32, rising: bool, up: (u8, u8), down: (u8, u8)) {
    let a = fx.mem.read_u8(t + o);
    if rising {
        if a < up.0 {
            fx.mem.write_u8(t + o, a.wrapping_add(up.1));
        }
    } else if a >= down.0 {
        fx.mem.write_u8(t + o, a - down.1);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// A 0x20-frame step's prologue (ra, the entry's s0 = D).
fn frame20(fx: &mut Fx) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
}

/// A step's prologue saving ra (+0x20), the entry's s1 (+0x10) and s0 = D.
fn frame_s1(fx: &mut Fx, size: u32, flashes: u32) -> u32 {
    let sp = fx.enter(size);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(flashes)), (0, u64::from(D))]);
    sp
}

/// The flat rings' first update (sites from `f`, `o` the extra offset of
/// 0x1E01F20's): radius +0x50 `rm` + 10 growing by +0xE 10..29, row 0..34,
/// life 3..7, +0xA rand % 8, +0xC the spawn index.
fn ring_rolls(fx: &mut Fx, t: u32, sites: [u32; 7], rm: i32) {
    let v = rmod(fx, t, sites[0], 0x168);
    fx.mem.set16(t + 0x5A, v);
    let v = rmod(fx, t, sites[1], 0x168);
    fx.mem.set16(t + 0x5C, v);
    let v = fx.rand_from(t, sites[2]);
    fx.mem.set16(t + 0xA, v & 7);
    let k = fx.mem.i16(t + 0x50);
    fx.mem.set16(t + 0xC, k);
    let v = rmod(fx, t, sites[3], rm);
    fx.mem.set16(t + 0x50, v + 0xA);
    let v = rmod(fx, t, sites[4], 0x14);
    fx.mem.set16(t + 0xE, v + 0xA);
    let v = rmod(fx, t, sites[5], 0x23);
    fx.mem.set16(t + 8, v);
    let v = rmod(fx, t, sites[6], 5);
    fx.mem.set16(t + 6, v + 3);
}

/// 0x1E01BB0 (D, task; frame 0x40): a grey flat ring (size 4, cell rand %
/// 7, radius 10..29 (not growing)), the angle +0x5C turning 2 a update; in
/// by 0x20 under 0x80 during the life, then out by 8. Its first update
/// reads node 0x27 into the frame (unused).
fn ring_a(fx: &mut Fx, t: u32, flashes: u32) {
    let sp = frame_s1(fx, 0x40, flashes);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4080_0000);
        let v = rmod(fx, t, 0x01E0_1BE0, 7);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        ring_rolls(fx, t, [0x01E0_1C0C, 0x01E0_1C28, 0x01E0_1C44, 0x01E0_1C68, 0x01E0_1C88, 0x01E0_1CA8, 0x01E0_1CC4], 0x14);
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODE_27);
        node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), 0x01E0_1CF4);
        fx.mem.set16(t + 0x58, 2);
    }
    let rising = fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6);
    fade(fx, t, 0x63, rising, (0x80, 0x20), (0xA, 8));
    let a = fx.mem.i16(t + 0x5C) + 2;
    fx.mem.set16(t + 0x5C, a);
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E01D80 (D, task; frame 0x30): two quads (30 x 30, cell 4) riding a
/// trail's head (+0x4470 + 0x1E0 +0x50), turned by the camera's angles;
/// white / grey 0xC0 by the age's parity; in by 0x40 under 0x80 for 50
/// updates, then out by 9.
fn quads(fx: &mut Fx, t: u32, flashes: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(flashes)), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x1E);
        fx.mem.set16(t + 0xA, 0x1E);
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = rmod(fx, t, 0x01E0_1DDC, 0x168);
        fx.mem.set16(t + 0x5C, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1DF8);
    }
    let rising = fx.mem.i16(t + 0x54) < 0x32;
    fade(fx, t, 0x63, rising, (0x80, 0x40), (0xB, 9));
    let g = if fx.mem.i16(t + 0x54).wrapping_rem(2) == 1 { 0xFF } else { 0xC0 };
    l2::rgb(fx, t + 0x60, g, g, g);
    let a = trail_of(fx, t);
    for k in 0..3 {
        let w = fx.mem.read_u32(a + 4 * k);
        fx.mem.write_u32(t + 0x10 + 4 * k, w);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1EF4);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E01F20 (D, task): a white flat ring (size 10, cell 5, radius 10..19
/// growing by 10..29); in by 0x20 under 0x80 during the life, then out by
/// 8.
fn ring_b(fx: &mut Fx, t: u32) {
    frame20(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        fx.mem.set16(t + 0x56, 5);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        ring_rolls(fx, t, [0x01E0_1F68, 0x01E0_1F84, 0x01E0_1FA0, 0x01E0_1FC4, 0x01E0_1FE4, 0x01E0_2004, 0x01E0_2020], 0xA);
        fx.mem.set16(t + 0x58, 2);
    }
    let rising = fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6);
    fade(fx, t, 0x63, rising, (0x80, 0x20), (0xA, 8));
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E020D0 (D, task): a flare (`sparks`: size 80 growing by 20..24.9 a
/// update, the inner 0.2 of it; kind (rand >> 4) & 1: (0xC0, 0x80, 0x80)
/// on black or (0xFF, 0xA0, 0xA0) on grey 0x50; random angles) at 0x80
/// for 8 updates, then out by 5.
fn flare(fx: &mut Fx, t: u32) {
    frame20(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42A0_0000);
        fx.mem.setf(t + 0x4C, ee::sub(80.0, 58.0));
        let v = fx.rand_from(t, 0x01E0_2110);
        fx.mem.set16(t + 0x50, (v >> 4) & 1);
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0xC0, 0x80, 0x80);
            l2::rgb(fx, t + 0x64, 0, 0, 0);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0xFF, 0xA0, 0xA0);
            l2::rgb(fx, t + 0x64, 0x50, 0x50, 0x50);
        }
        fx.mem.set16(t + 0x5A, 0x1E);
        fx.mem.set16(t + 0x5C, 0x1E);
        let v = rmod(fx, t, 0x01E0_21A8, 0x32);
        fx.mem.setf(t + 0x34, ee::from_i32(v + 0xC8));
        let v = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        for (o, site) in [(0x20u32, 0x01E0_21F0u32), (0x24, 0x01E0_220C), (0x28, 0x01E0_2228)] {
            let v = rmod(fx, t, site, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
    }
    if fx.mem.i16(t + 0x54) < 8 {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade(fx, t, 0x63, false, (0, 0), (7, 5));
    }
    let s = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, s);
    fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(0x3E4C_CCCD), s));
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E022D0 (`b` false) / 0x1E024D0 (true) (D, task): an arc
/// (`arc_strips`, angle 10 spanning 10) by the index +0x50: 0 a red-white
/// arc of radius 220 for 14 updates, out by 9; 1 a dark (0x1E022D0) or
/// grey-red (0x1E024D0) arc at random angles growing by 25 (the inner 0.6
/// of it) for 8 updates, out by 15; in by 0x60 under 0xC0 during the life.
fn arc_ab(fx: &mut Fx, t: u32, b: bool) {
    frame20(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.write_u32(t + 0x4C, 0x435C_0000);
            fx.mem.set16(t + 6, 0xE);
            l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
            l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x40);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.set16(t + 6, 8);
            let (outer, inner, sites) = if b {
                ((0x80, 0x20, 0x20), 0x80, [0x01E0_2578u32, 0x01E0_2594, 0x01E0_25B0])
            } else {
                ((0x18, 8, 8), 0x18, [0x01E0_2374, 0x01E0_2390, 0x01E0_23AC])
            };
            l2::rgb(fx, t + 0x64, outer.0, outer.1, outer.2);
            l2::rgb(fx, t + 0x60, inner, inner, inner);
            for (o, site) in [0x20u32, 0x24, 0x28].into_iter().zip(sites) {
                let v = rmod(fx, t, site, 0xFFFF);
                fx.mem.set32(t + o, v);
            }
        }
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade(fx, t, 0x67, true, (0xC0, 0x60), (0, 0));
    } else {
        if fx.mem.i16(t + 0x50) == 0 {
            fade(fx, t, 0x67, false, (0, 0), (0xB, 9));
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fade(fx, t, 0x67, false, (0, 0), (0x11, 0xF));
        }
    }
    if fx.mem.i16(t + 0x50) == 1 {
        let r = ee::add(fx.mem.f(t + 0x48), 25.0);
        fx.mem.setf(t + 0x48, r);
        fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(0x3F19_999A), r));
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E026D0 (D, task): a red-white arc (`arc_strips`, angle 10 spanning
/// 10, radius 70 growing by 1 to 150) turned by the camera's angles (the
/// first + 0x4000); in by 0x40 under 0xC0 for 7 updates, then out by 15.
fn arc_c(fx: &mut Fx, t: u32, flashes: u32) {
    frame20(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x428C_0000);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(t + 0x54) < 7;
    fade(fx, t, 0x67, rising, (0xC0, 0x40), (0x11, 0xF));
    // 0x205580's saves: the step's s0 = the task, s1 the entry's.
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(flashes), 0x01E0_2790);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.f(t + 0x4C);
        if r < 150.0 {
            fx.mem.setf(t + 0x4C, ee::add(r, 1.0));
        }
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02800 (D, task; frame 0x70: ra, the entry's s3 = the slot, s2 = 0,
/// s1 = the flashes, s0 = D, f22..f20 the runner's (0 [H])): a trail. Its
/// head (+0x7CB0 + 0x10 k, k = +0x50) copied to the stack; on the first
/// update the 30-point history (+0x4470 + 0x1E0 k) filled with it, the
/// head's pull toward the target's position +0x7E90 (+-10 each way) over
/// 48, width 20 growing by 1 to 22, sway +0 = ((rand >> 4) % 20) - 10, +0xA
/// rand % 30, size +0x48 10; alpha +0x63 / +0x67 in by 0x2A under 0x80 for
/// 55 updates, then the size down by 1.5 until below 0; below 48 updates
/// the stacked head swayed by +0x4C along the sines of +0x5A and pushed
/// into the history; white / grey 0xC0 by the age's parity until 80; the
/// head pulled until 46; on 46..50 an arc 0x1E024D0 and a ring 0x1E01F20
/// at the history's newest point (the index 1), on 48 an arc 0x1E026D0.
fn trail(fx: &mut Fx, t: u32, flashes: u32, slot: u32, k: u32) {
    use ee::{add, div, from_i32, mul, sub};
    let sp = fx.enter(0x70);
    fx.saved(sp, &[(0x50, TASK_RA), (0x40, u64::from(slot)), (0x30, 0), (0x20, u64::from(flashes)), (0x10, u64::from(D))]);
    for o in [0u32, 4, 8] {
        fx.mem.write_u32(sp + o, 0);
    }
    let p = sp + 0x60;
    let h = head_of(fx, t);
    for c in 0..3 {
        let w = fx.mem.read_u32(h + 4 * c);
        fx.mem.write_u32(p + 4 * c, w);
    }
    // The spawns' saves: s0 = sp + 0x64, s1 = sp + 0x68, s2 = the task, s3
    // = D, the entry's s4 = the slot index.
    let saves = |ra: u32| [(0x50u32, u64::from(ra)), (0x40, u64::from(k)), (0x30, u64::from(D)), (0x20, u64::from(t)), (0x10, u64::from(sp + 0x68)), (0, u64::from(sp + 0x64))];
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        let a = trail_of(fx, t);
        for i in 0..0x1Eu32 {
            for c in 0..3 {
                let w = fx.mem.read_u32(p + 4 * c);
                fx.mem.write_u32(a + 0x10 * i + 4 * c, w);
            }
        }
        let mut f20 = fx.mem.f(TARGET + 4);
        let mut f21 = fx.mem.f(TARGET + 8);
        let mut f22 = fx.mem.f(TARGET);
        let v = rmod(fx, t, 0x01E0_28FC, 0x14);
        f20 = add(f20, from_i32(v - 0xA));
        let v = rmod(fx, t, 0x01E0_2928, 0x14);
        f22 = add(f22, from_i32(v - 0xA));
        let v3 = rmod(fx, t, 0x01E0_2954, 0x14);
        let h = head_of(fx, t);
        let v = div(sub(f22, fx.mem.f(h)), 48.0);
        fx.mem.setf(t + 0x30, v);
        f21 = add(f21, from_i32(v3 - 0xA));
        let h = head_of(fx, t);
        let v = div(sub(f20, fx.mem.f(h + 4)), 48.0);
        fx.mem.setf(t + 0x34, v);
        let h = head_of(fx, t);
        let v = div(sub(f21, fx.mem.f(h + 8)), 48.0);
        fx.mem.setf(t + 0x38, v);
        fx.mem.set16(t + 6, 0x14);
        let v = fx.rand_from(t, 0x01E0_29DC);
        let (_, r) = fx.div(v >> 4, 0x14);
        fx.mem.set16(t, r - 0xA);
        fx.mem.write_u8(t + 0x63, 0);
        fx.mem.set16(t + 8, 0);
        let v = rmod(fx, t, 0x01E0_2A08, 0x1E);
        fx.mem.set16(t + 0xA, v);
        fx.mem.set16(t + 0x56, 4);
        let v = rmod(fx, t, 0x01E0_2A2C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x37 {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0x80 {
                fx.mem.write_u8(t + o, a + 0x2A);
            }
        }
    } else {
        let s = fx.mem.f(t + 0x48);
        if s < 0.0 {
            fx.mem.set16(t + 0x58, 0);
        } else {
            fx.mem.setf(t + 0x48, sub(s, 1.5));
        }
    }
    if fx.mem.i16(t + 0x54) < 0x30 {
        let a = fx.mem.i16(t + 0x5A);
        let s = l2::sin_deg(fx, SINES2, a, 0);
        let v = add(fx.mem.f(p + 4), mul(fx.mem.f(t + 0x4C), s));
        fx.mem.setf(p + 4, v);
        let a = fx.mem.i16(t + 0x5A);
        let c = l2::sin_deg(fx, SINES2, a, 1);
        let v = add(fx.mem.f(p), mul(fx.mem.f(t + 0x4C), c));
        fx.mem.setf(p, v);
        let a = fx.mem.i16(t + 0x5C);
        let s = l2::sin_deg(fx, SINES2, a, 0);
        let w = mul(from_i32(fx.mem.i16(t + 0xA)), s);
        fx.mem.setf(t + 0x4C, w);
        let a = trail_of(fx, t);
        history(fx, a + 0x1D0, 0x1E, p);
        let a = fx.mem.i16(t + 0x5C);
        if a < 0xB4 {
            fx.mem.set16(t + 0x5C, a + 4);
        }
        let a = fx.mem.i16(t + 0x5A) + fx.mem.i16(t);
        fx.mem.set16(t + 0x5A, a);
    }
    if fx.mem.i16(t + 0x54) < 0x50 {
        let g = if fx.mem.i16(t + 0x54).wrapping_rem(2) == 1 { 0xFF } else { 0xC0 };
        l2::rgb(fx, t + 0x60, g, g, g);
    }
    if fx.mem.i16(t + 0x54) != 0 && fx.mem.i16(t + 6) < 0x16 {
        let w = fx.mem.i16(t + 6) + 1;
        fx.mem.set16(t + 6, w);
    }
    let n = fx.mem.i16(t + 0x54);
    if n < 0x2E {
        for (c, o) in [(0u32, 0x30u32), (4, 0x34), (8, 0x38)] {
            let h = head_of(fx, t);
            let v = add(fx.mem.f(h + c), fx.mem.f(t + o));
            fx.mem.setf(h + c, v);
        }
    } else {
        let at_newest = |fx: &mut Fx| {
            let a = trail_of(fx, t);
            for c in 0..3 {
                let w = fx.mem.read_u32(a + 4 * c);
                fx.mem.write_u32(SPAWN_AT + 4 * c, w);
            }
        };
        if n < 0x33 {
            fx.mem.set16(INDEX, 1);
            at_newest(fx);
            spawn(fx, ARC_B, &saves(0x01E0_2D14));
            spawn(fx, RING_B, &saves(0x01E0_2D28));
        }
        if fx.mem.i16(t + 0x54) == 0x30 {
            at_newest(fx);
            spawn(fx, ARC_C, &saves(0x01E0_2DA0));
        }
    }
    age(fx, t);
    fx.leave(0x70);
}

/// 0x1E02DE0 (D, task; frame 0x40): a red-white arc (`arc_strips`, angle
/// 10 spanning 10, radius 50 growing by 3 to 100 from update 30) turned by
/// the camera's angles (the first + 0x4000), following node 0x27 a fifth
/// of the way past node 0x20; in by 0x13 under 0xC0 for 200 updates, then
/// out by 15.
fn arc_d(fx: &mut Fx, t: u32, flashes: u32) {
    use ee::{add, mul, sub};
    let sp = frame_s1(fx, 0x40, flashes);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x4248_0000);
        l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(t + 0x54) < 0xC8;
    fade(fx, t, 0x67, rising, (0xC0, 0x13), (0x11, 0xF));
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2EA8);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    if fx.mem.i16(t + 0x54) >= 0x1E {
        let r = fx.mem.f(t + 0x4C);
        if r < 100.0 {
            fx.mem.setf(t + 0x4C, add(r, 3.0));
        }
    }
    age(fx, t);
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODE_27);
    node_position_saving(fx, h, slot, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_2F18);
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODE_20);
    node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), 0x01E0_2F30);
    let fifth = f32::from_bits(0x3E4C_CCCD);
    for c in 0..3u32 {
        let x = fx.mem.f(t + 0x10 + 4 * c);
        let q = fx.mem.f(sp + 0x30 + 4 * c);
        fx.mem.setf(t + 0x10 + 4 * c, add(x, mul(fifth, sub(x, q))));
    }
    fx.leave(0x40);
}

/// A random -0x400..0x3FF (rand & 0x7FF, less 0x400).
fn spin(fx: &mut Fx, t: u32, ra: u32) -> i32 {
    let v = fx.rand_from(t, ra);
    (v & 0x7FF) - 0x400
}

/// 0x1E02FA0 (D, task): a red-white quad (`quad`, 30 x 30, cell 2..4 by
/// rand >> 4, the orientation +0x52 (rand >> 4) & 3) turned by the
/// camera's angles, drifting by (rand % 100 - 50) / 60 a axis (unused by
/// the draw); +0 / +2 / +4 random -0x400..0x3FF; in by 0x40 under 0x80 for
/// 4 updates, then out by 4.
fn quad(fx: &mut Fx, t: u32, flashes: u32) {
    use ee::{div, from_i32};
    frame20(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x1E);
        fx.mem.set16(t + 0xA, 0x1E);
        let v = fx.rand_from(t, 0x01E0_2FD0);
        let (_, r) = fx.div(v >> 4, 3);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        fx.mem.set16(t + 0x56, r + 2);
        fx.mem.set16(t + 0x58, 2);
        for (o, site) in [(0x34u32, 0x01E0_3008u32), (0x30, 0x01E0_3034), (0x38, 0x01E0_3060)] {
            let v = rmod(fx, t, site, 0x64);
            fx.mem.setf(t + o, from_i32(v - 0x32));
        }
        for o in [0x34u32, 0x30, 0x38] {
            let v = div(fx.mem.f(t + o), 60.0);
            fx.mem.setf(t + o, v);
        }
        let v = rmod(fx, t, 0x01E0_30B8, 0x168);
        fx.mem.set16(t + 0x5C, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(flashes), 0x01E0_30D4);
        for (o, site) in [(4u32, 0x01E0_30DCu32), (2, 0x01E0_30FC), (0, 0x01E0_311C)] {
            let v = spin(fx, t, site);
            fx.mem.set16(t + o, v);
        }
        let v = fx.rand_from(t, 0x01E0_313C);
        fx.mem.set16(t + 0x52, (v >> 4) & 3);
    }
    let rising = fx.mem.i16(t + 0x54) < 4;
    fade(fx, t, 0x63, rising, (0x80, 0x40), (6, 4));
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(flashes), 0x01E0_31B0);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E031D0 (D, task; frame 0x50): camera slots 4 / 5 (0x1F7E20) from
/// +0x4440 (128, 4, 4, 4; node 0x27's point; 0, 123, w rising by 0.1 to 1
/// for 210 updates, then down by 0.3); cleared when the task ends (w at
/// 0.1, or the timer at 1).
fn camera(fx: &mut Fx, t: u32, flashes: u32) {
    let sp = frame_s1(fx, 0x50, flashes);
    if fx.mem.i16(t + 0x58) == 1 {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x4080_0000), (8, 0x4080_0000), (0xC, 0x4080_0000), (0x20, 0), (0x24, 0x42F6_0000), (0x28, 0)] {
            fx.mem.write_u32(CAM + o, v);
        }
        for k in 0..3 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * k);
            fx.mem.write_u32(CAM + 0x10 + 4 * k, w);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.set16(t + 0x50, 0);
    }
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODE_27);
    node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), 0x01E0_3258);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x30 + 4 * k);
        fx.mem.write_u32(CAM + 0x10 + 4 * k, w);
    }
    let w = fx.mem.f(CAM + 0x28);
    let tenth = f32::from_bits(0x3DCC_CCCD);
    if fx.mem.i16(t + 0x54) < 0xD2 {
        if w < 1.0 {
            fx.mem.setf(CAM + 0x28, ee::add(w, tenth));
        }
    } else if w <= tenth {
        fx.mem.set16(t + 0x58, 0);
    } else {
        fx.mem.setf(CAM + 0x28, ee::sub(w, f32::from_bits(0x3E99_999A)));
    }
    let (p, ra) = if fx.mem.i16(t + 0x58) == 0 { (0, 0x01E0_3314) } else { (CAM, 0x01E0_3328) };
    crate::blib::exe::camera_slots45(fx, p);
    // 0x1F7E20's frame (s0 = the task, ra).
    fx.saved(sp - 0x20, &[(0, u64::from(t)), (0x10, ra)]);
    if fx.mem.i32(TIMER) == 1 {
        fx.mem.set16(t + 0x58, 0);
    }
    age(fx, t);
    fx.leave(0x50);
}

/// 0x1E03360 (D, task): the stage light (0x1F7F10) down by 10 to 0x60 for
/// 51 updates, back up by 10 to 0x80 once the timer is under 30; 0x80 and
/// the task's end when the timer is at 3.
fn light(fx: &mut Fx, t: u32) {
    frame20(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x33 {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x61 { 0x60 } else { l - 0xA });
    } else if fx.mem.i32(TIMER) < 0x1E {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x80 { l + 0xA } else { 0x80 });
        if fx.mem.i16(t + 0x50) >= 0x81 {
            fx.mem.set16(t + 0x50, 0x80);
        }
    }
    age(fx, t);
    if fx.mem.i32(TIMER) == 3 {
        fx.set_stage_light(0x80);
        fx.mem.set16(t + 0x58, 0);
    } else {
        let l = fx.mem.read_u8(t + 0x50);
        fx.set_stage_light(i32::from(l));
    }
    fx.leave(0x20);
}

/// 0x1E03460 (D, task; frame 0x90): a window sprite (0x6FF8 .. 0x8FF8)
/// sampling the frame (0x207EF0, filter 1) at ST inset by +0x48 (2.5) /
/// 512, colour (0x90, 0x80, 0x80, alpha +0x63), ALPHA 0x44, key 0; in by
/// 0x16 below 0x58 until the timer is under 46, then out by 4.
fn tint(fx: &mut Fx, t: u32, flashes: u32) {
    let sp = frame_s1(fx, 0x90, flashes);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4020_0000);
        l2::rgb(fx, t + 0x60, 0x90, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for k in 0..4 {
        let c = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(sp + 0x48 + k, c);
    }
    fx.mem.write_u32(sp + 0x4C, 0x3F80_0000);
    for (at, v) in [(sp + 0x58, 0x6FF8u16), (sp + 0x68, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    for o in [0x50u32, 0x54] {
        let s = fx.mem.f(t + 0x48);
        fx.mem.setf(sp + o, ee::div(s, 512.0));
    }
    for o in [0x60u32, 0x64] {
        let s = fx.mem.f(t + 0x48);
        fx.mem.setf(sp + o, ee::div(ee::sub(512.0, s), 512.0));
    }
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    let rising = fx.mem.i32(TIMER) >= 0x2E;
    fade(fx, t, 0x63, rising, (0x58, 0x16), (6, 4));
    age(fx, t);
    fx.leave(0x90);
}

/// 0x1E03660(D, t): the flash's frame sprite (the attacker over the
/// window, (0xFF, 0x30, 0x80), UVs from +8 / 10 and +0xA / 10 over 512)
/// into the descriptor at +0x7ED8, submitted once with no ordering table;
/// in by 9 under 0x80 for 140 updates, then out by 7. `regs`: the unit
/// flash's s0 (the slot index) and s1 (the live tasks so far).
fn flash(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, FLASH_RA), (0x20, u64::from(regs[1])), (0x10, u64::from(regs[0]))]);
    // swc1 f20 / f21: the runner's (0) [H, as b747].
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 4, 0);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let h = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0x30, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = fx.mem.read_u32(FLASH_DESC);
    for k in 0..4u32 {
        let b = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(d + 0x10 + k, b);
    }
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    for (at, xy) in [(0x20u32, 0x6FF8u16), (0x30, 0x8FF8)] {
        fx.mem.write_u16(d + at, xy);
        fx.mem.write_u16(d + at + 2, xy);
        let v = fx.mem.read_u64(d + at);
        fx.mem.write_u64(d + at, (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
        fx.mem.write_u8(d + at + 7, 0);
    }
    for (o, v) in [(0x18u32, ee::div(w, 512.0)), (0x1C, ee::div(h, 512.0)), (0x28, ee::div(ee::sub(512.0, w), 512.0)), (0x2C, ee::div(ee::sub(512.0, h), 512.0))] {
        fx.mem.setf(d + o, v);
    }
    fx.submit(d, 0, 0);
    let rising = fx.mem.i16(t + 0x54) < 0x8C;
    if rising || fx.mem.i16(t + 0x54) >= 0x29 {
        fade(fx, t, 0x63, rising, (0x80, 9), (9, 7));
    }
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E04C30(D): in a packet layer (0x2078A0) the attacker drawn into the
/// render target (0x1F8270 filter 0, its TEX0 / TEX1 into the sprite
/// descriptor at sp + 0x58; ALPHA 0x48), TEXFLUSH before and after; every
/// flash task stepped (0x1E03660); the layer linked into the ordering
/// table at key 0 when any task lives (b232's 0x1E01ED0 on the attacker).
fn unit_flash(fx: &mut Fx, a3: u32) {
    let sp = fx.enter(0xB0);
    let d = sp + 0x58;
    fx.mem.write_u32(FLASH_DESC, d);
    let layer = fx.layer_begin();
    fx.mem.write_u32(LAYER, layer);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.texflush([5, d, 0x26, a3]);
    fx.desc_alpha(d, 0x80_0000_0048);
    let h = fx.self_handle();
    let (_, regs) = fx.unit_draw(h, 0, sp + 0x98, sp + 0xA0);
    fx.texflush(regs);
    let mut live = 0u32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        live += 1;
        if fx.mem.read_u32(slot + 0x2E4) == FLASH {
            flash(fx, slot + 0x2A0, [k, live]);
        }
    }
    if live != 0 {
        let layer = fx.mem.read_u32(LAYER);
        fx.layer_end(layer, OT, 0);
    }
    fx.leave(0xB0);
}
