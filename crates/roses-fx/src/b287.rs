//! BMODULE b287 (card 335), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03170, data area 0x1ED0000, 0xCF30 bytes) [F]
//!: b522's program (b703's 400-task pool of 0x70
//! bytes at D+0x2A0 with the same globals and the same spawn 2b64e6f0541d
//! at 0x1E03870) plus b550's unit flash (0x1E03F90: the flash tasks
//! 0x1E02EB0 counted, not stepped, by the entry and stepped in a packet
//! layer; descriptor D+0xCB28, layer D+0xCB24). Update 1 the stage light
//! (b293's) and a green flash of the target; 0x82 at node 0x16 b703's arc,
//! twenty thrown white flares (b641's), four blue light planes (b544's)
//! and 0x82..0x8C ten b522 rings a update there. The quads 0x1E021A0 and
//! the ribbon task 0x1E028F0 are drawn but never spawned (not ported).
//! Returns 1 on update 0x78, 4 on 0x1D1, 2 on 0xA0, 6 on 0x83..0x9A, 8
//! once the 0xB9-update timer D+0xCB10 has run out.
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
const POS: u32 = D + 0x250;
const RING: u32 = 0x01E0_1FF0;
const THROWN: u32 = 0x01E0_2510;
const PLANE: u32 = 0x01E0_2760;
const ARC: u32 = 0x01E0_2C80;
const LIGHT: u32 = 0x01E0_2DA0;
/// The flash tasks, run by the unit flash 0x1E03F90.
const FLASH: u32 = 0x01E0_2EB0;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_3488;
/// Node 0x16's point kept by the emitter.
const KEPT: u32 = D + 0xCAD0;
const TEX: u32 = D + 0xCAF0;
const TIMER: u32 = D + 0xCB10;
const COLOUR: u32 = D + 0xCB14;
const LAYER: u32 = D + 0xCB24;
const FLASH_DESC: u32 = D + 0xCB28;
const NODES: u32 = D + 0xCB44;
const SPAWN_A: u32 = D + 0xCBA2;
const SPAWN_K: u32 = D + 0xCBB0;
const FACING: u32 = D + 0xCBB4;
const COUNTER: u32 = D + 0xCBB8;
const BYTES: u32 = D + 0xCBBA;
const TAB: u32 = D + 0xCDBC;
const SWAY: u32 = 0x3F19_999A;
const THROW: u32 = 0x4109_999A;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E03170 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    fx.mem.write_u32(D + 0xCB1C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xCF30);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xCBB6, param);
    fx.mem.set32(TIMER, 0xB9);
    for (k, n) in [0x15, 0x16, 0x35, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xCB54, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.fix_residue(y, 0x01E0_3368);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_3398, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_33EC);
        fx.mem.set16(D + 0xCB64 + 2 * k, v % 8);
    }
    0
}

/// The last XYZ2 of the last primitive drawn since `before` (0 when none).
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

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    // The entry's s0: the flash tasks counted here (the steps' saved s0);
    // its s1 is 0 until the draw loop.
    let mut flashes = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        match f {
            0 => continue,
            FLASH => {
                flashes += 1;
                continue;
            }
            _ => {}
        }
        let frame = if f == THROWN { 0x30 } else { 0x20 };
        let s = fx.enter(frame);
        if frame == 0x20 {
            fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(flashes))]);
        } else {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(flashes))]);
        }
        match f {
            RING => ring(fx, t),
            THROWN => thrown(fx, t),
            PLANE => plane(fx, t),
            ARC => arc(fx, t),
            LIGHT => light(fx, t),
            _ => panic!("b287: unknown task step {f:#x}"),
        }
        fx.leave(frame);
    }
    // a3 as the draws' last library call left it, for the unit flash's
    // first 0x207BB0: 0 after a ring, a quad or planes, the last XYZ2
    // after an arc.
    let mut a3 = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        if f(fx) == RING {
            tex(fx, TEX);
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x60, 0x14, c);
            fx.mem.set16(D + 0xB8, 4);
            let v = fx.rand_from(flashes, 0x01E0_354C);
            fx.mem.set16(D + 0xBE, v % 4);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x21, BYTES);
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == THROWN {
            tex(fx, TEX + 4);
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x80, 0x14, c);
            fx.mem.set16(D + 0xBE, 0);
            billboard::quad(fx, uv::uv_rect, D, t, 0x10, t + 0x10);
            a3 = 0;
        }
        if f(fx) == PLANE {
            tex(fx, D + 0x29C);
            cell(fx, sp + 0x90, 0x9A, 0);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sprite_quads(fx, D, t, 0, t + 0x10);
            quads4::sprite_quads(fx, D, t, 0, t + 0x10);
            a3 = 0;
        }
        if f(fx) == ARC {
            fx.mem.set16(D + 0xB8, 0x24);
            let before = fx.out.prims.len();
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            a3 = last_xyz2(fx, before);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    if flashes != 0 {
        unit_flash(fx, a3);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x78 => 1,
        0x1D1 => 4,
        0xA0 => 2,
        0x83..=0x9A => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` / `sub` (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

fn tex(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

/// 0x1E03870(D, step, 0): b522's spawn (the same function); its frame's
/// saves the emitter's s0 = D / s1, the entry's s2 = D / s3 = param, the
/// runner's s4 (0 [H]) and the return address `ra`.
fn spawn(fx: &mut Fx, step: u32, s1: u32, param: i32, ra: u32) -> i32 {
    let f = fx.enter(0x60);
    fx.saved(f, &[(0, u64::from(D)), (0x10, u64::from(s1)), (0x20, u64::from(D)), (0x30, param as i64 as u64), (0x40, 0), (0x50, u64::from(ra))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x2F8 + 0x70 * k) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for o in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + o + 3, a);
            }
            l2::to_i16x4(fx, t, POS, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(POS + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let s = fx.mem.i16(SPAWN_K);
            fx.mem.set16(t + 0x50, s);
            let s = fx.mem.i16(SPAWN_K);
            fx.mem.set16(t + 0x52, s);
            let a = fx.mem.i16(SPAWN_A);
            fx.mem.set16(t + 0x5A, a);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, step);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

fn kept_to_pos(fx: &mut Fx) {
    for k in 0..4 {
        let w = fx.mem.read_u32(KEPT + 4 * k);
        fx.mem.write_u32(POS + 4 * k, w);
    }
}

/// 0x1E01D30(D, 0): the update counter and the emitters (frame 0x30: ra,
/// the entry's s1 = 0, s0 = 0; then s0 = D, s1 a loop count).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_3448), (0x10, 0), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    if c(fx) == 1 {
        spawn(fx, LIGHT, 0, param, 0x01E0_1D84);
    }
    if c(fx) == 1 {
        spawn(fx, FLASH, 0, param, 0x01E0_1DB0);
    }
    if c(fx) == 0x82 {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2);
        fx.node_position(h, slot, KEPT);
    }
    if c(fx) == 0x82 {
        kept_to_pos(fx);
        spawn(fx, ARC, 0, param, 0x01E0_1E40);
        for k in 0..0x14 {
            spawn(fx, THROWN, k, param, 0x01E0_1E5C);
        }
        let ras = [0x01E0_1EA8, 0x01E0_1EDC, 0x01E0_1F10, 0x01E0_1F44];
        for ((a, kind), ra) in [(0, 0), (0x5A, 1), (0x2D, 2), (0x87, 2)].into_iter().zip(ras) {
            fx.mem.set16(SPAWN_A, a);
            fx.mem.set16(SPAWN_K, kind);
            spawn(fx, PLANE, 0x14, param, ra);
        }
    }
    if (0x82..0x8D).contains(&c(fx)) {
        kept_to_pos(fx);
        for k in 0..0xA {
            spawn(fx, RING, k, param, 0x01E0_1FAC);
        }
    }
    fx.leave(0x30);
}

/// Byte `at` up by `up` while under `cap`.
fn fade_in(fx: &mut Fx, t: u32, at: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + at);
    if a < cap {
        fx.mem.write_u8(t + at, a.wrapping_add(up));
    }
}

/// Byte `at` down by `down` while at least `floor`, else the task ends.
fn fade_out(fx: &mut Fx, t: u32, at: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + at);
    if a >= floor {
        fx.mem.write_u8(t + at, a.wrapping_sub(down));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn add(fx: &mut Fx, at: u32, by: f32) {
    let v = ee::add(fx.mem.f(at), by);
    fx.mem.setf(at, v);
}

/// 0x1E01FF0 (D, task): b522's ring (b703's growing grey ring with size
/// 6..20, life 7..11, word for word).
fn ring(fx: &mut Fx, t: u32) {
    let f = RING;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x24, 0xF);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 6));
        let v = rmod(fx, t, f + 0x50, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, f + 0x7C, 0x28);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, f + 0x9C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0xB8, 0x14);
        fx.mem.set16(t + 0xE, v + 0x10);
        let v = rmod(fx, t, f + 0xD8, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0xF4, 5);
        fx.mem.set16(t + 6, v + 7);
        fx.mem.set16(t + 0x58, 2);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x40));
        }
    } else if a >= 0x10 {
        fx.mem.write_u8(t + 0x63, a - 0xE);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// (rand >> 4) % m (HI) from the `jal` returning to `ra`.
fn rand4_mod(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v >> 4, m).1
}

/// axis `a` (+0x10 / +0x14 / +0x18) += `k` x sin (or cos) of the i16
/// angle at `t + deg` degrees (0x108910 on the table D+0xCDBC).
fn sway(fx: &mut Fx, t: u32, a: u32, deg: u32, cos: i32, k: u32) {
    let d = fx.mem.i16(t + deg);
    let s = l2::sin_deg(fx, TAB, d, cos);
    let v = ee::add(fx.mem.f(t + a), ee::mul(f32::from_bits(k), s));
    fx.mem.setf(t + a, v);
}

/// 0x1E02510 (D, task): b641's thrown flare (0x1E020B0): size 150, cell
/// frame 2..4, white, headings +0x5A / +0x5C random; first thrown 8.6
/// along them, then swaying by 0.6; alpha up by 0x60 below 0xC0 for 26
/// updates, then down by 19; faces the camera.
fn thrown(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x96);
        let s = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, s);
        let v = rand4_mod(fx, t, 0x01E0_2548, 3);
        fx.mem.set16(t + 0x56, v + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = rand4_mod(fx, t, 0x01E0_2580, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rand4_mod(fx, t, 0x01E0_25A0, 0x168);
        fx.mem.set16(t + 0x5C, v);
        sway(fx, t, 0x14, 0x5A, 0, THROW);
        sway(fx, t, 0x10, 0x5A, 1, THROW);
        sway(fx, t, 0x18, 0x5C, 1, THROW);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0x1A {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a + 0x60);
        }
    } else if a >= 0x15 {
        fx.mem.write_u8(t + 0x63, a - 0x13);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        sway(fx, t, 0x14, 0x5A, 0, SWAY);
        sway(fx, t, 0x10, 0x5A, 1, SWAY);
        sway(fx, t, 0x18, 0x5C, 1, SWAY);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2738);
    age(fx, t);
}

/// 0x1E02760 (D, task): b544's light plane (0x1E01DE0) in white / blue
/// (0x10, 0x10, 0x80): kind 0 18 x 800 growing by 80, 1 18 x 500 growing
/// by 60, 2 18 x 400; in by 0x40 for 4 updates, then out by 9.
fn plane(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x10, 0x10, 0x80);
        for (kind, h) in [(0, 0x4448_0000u32), (1, 0x43FA_0000), (2, 0x43C8_0000)] {
            if fx.mem.i16(t + 0x50) == kind {
                fx.mem.write_u32(t + 0x30, 0x4190_0000);
                fx.mem.write_u32(t + 0x34, h);
            }
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 4 {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    for (kind, dh) in [(0, 80.0f32), (1, 60.0)] {
        if fx.mem.i16(t + 0x50) == kind && fx.mem.i16(t + 0x54) != 0 {
            add(fx, t + 0x34, dh);
        }
    }
    age(fx, t);
}

/// 0x1E02C80 (D, task): b703's arc (0x1E03200, word for word): outer
/// radius 150 growing by 0.4 (the entry's s1 = 0 under 0x205580).
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x4316_0000);
        l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xFF);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0xE {
        fade_in(fx, t, 0x67, 0xC0, 0x30);
    } else {
        fade_out(fx, t, 0x67, 0x11, 0xF);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2D40);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    if fx.mem.i16(t + 0x54) != 0 {
        add(fx, t + 0x4C, f32::from_bits(0x3ECC_CCCD));
    }
    age(fx, t);
}

/// 0x1E02DA0 (D, task): b293's stage light (0x1E02C10, word for word).
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x97 {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x31 { 0x30 } else { l - 0xA });
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
}

/// 0x1E02EB0(D, t): b550's flash step (0x1E03ED0) in green (0xC0, 0xFF,
/// 0xC0), not growing: alpha up by 9 below 0x80 for 140 updates, then
/// down by 7 (the task ends under 9). `regs` the unit flash's s0 / s1
/// (the slot index, the live count).
fn flash(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_409C), (0x20, u64::from(regs[1])), (0x10, u64::from(regs[0]))]);
    // swc1 f21 / f20: the runner's (0) [H].
    fx.mem.write_u32(sp + 4, 0);
    fx.mem.write_u32(sp, 0);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let h = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0xC0, 0xFF, 0xC0);
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
    fx.mem.setf(d + 0x18, ee::div(w, 512.0));
    fx.mem.setf(d + 0x1C, ee::div(h, 512.0));
    fx.mem.setf(d + 0x28, ee::div(ee::sub(512.0, w), 512.0));
    fx.mem.setf(d + 0x2C, ee::div(ee::sub(512.0, h), 512.0));
    fx.submit(d, 0, 0);
    if fx.mem.i16(t + 0x54) < 0x8C {
        fade_in(fx, t, 0x63, 0x80, 9);
    } else {
        fade_out(fx, t, 0x63, 9, 7);
    }
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E03F90(D): b550's unit flash (0x1E05CF0) on this record's globals
/// and 400 tasks: in a packet layer (0x2078A0) the attacker drawn into
/// the target (0x1F8270 filter 0; ALPHA 0x48), TEXFLUSH before and after;
/// every flash task stepped (0x1E02EB0); the layer linked into the
/// ordering table at key 0 when any task was alive.
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
    let mut alive = 0u32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        alive += 1;
        if fx.mem.read_u32(slot + 0x2E4) == FLASH {
            flash(fx, slot + 0x2A0, [k, alive]);
        }
    }
    if alive != 0 {
        let layer = fx.mem.read_u32(LAYER);
        fx.layer_end(layer, OT, 0);
    }
    fx.leave(0xB0);
}
