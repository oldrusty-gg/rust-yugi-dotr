//! BMODULE b548 (card 594), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01830, data area 0x1ED0000) [F]: b038's family: a pool of 400 tasks (0x70 bytes at +0x2A0, alive
//! +0x58, step +0x44) drawn with camera-facing quads (98b02c86a827 at
//! 0x1E00920, `billboard::ring_facing`, UVs by uv_rect 0x1E00080). Every
//! update first stores the camera's +0x20 / +0x30 (0x1FACE0 / 0x1FAF40) at
//! D+0xB204 / D+0xB208. The emitter 0x1E00E90: from update 62 to 250 two
//! violet orbs a update at the attacker's node 0x14 (0x1E01070, rising
//! 120 x (sin +0x5C - sin 45) as +0x5C runs 45..140, then bursting into a
//! ring of three 0x1E01670 quads: always before update 170, one in four
//! later), on updates 23 and 43 two white rings of ten quads (0x1E014C0).
//! Returns 1 on update 130, 3 on 310, 6 on 141..304, 8 once the
//! 325-update timer +0xB1F0 has run out.
use crate::blib::{billboard, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
/// The step functions a task holds (+0x44), as the record's addresses.
const ORB: u32 = 0x01E0_1070;
const RING: u32 = 0x01E0_14C0;
const BURST: u32 = 0x01E0_1670;
/// The entry's `jal` to the emitter / `jalr` returns (0x1E01AA8 + 8,
/// 0x1E01AD8 + 8): prologue saves below the entry's frame (the quads'
/// stale TEX0 / TEX1 when the texture is missing).
const EMIT_RA: u64 = 0x01E0_1AB0;
const TASK_RA: u64 = 0x01E0_1AE0;
/// Data-area fields above 0x10000 (the original's `lui 1` pairs).
const TIMER: u32 = D + 0xB1F0;
const COUNTER: u32 = D + 0xB25E;
const BYTES: u32 = D + 0xB260;
const SINES: u32 = D + 0xB460;

/// The entry 0x1E01830.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    fx.mem.write_u32(D + 0xB1FC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let eye = fx.camera_eye(0);
    fx.mem.write_u32(D + 0xB204, eye);
    let at = fx.camera_at30(0);
    fx.mem.write_u32(D + 0xB208, at);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x90);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB5D0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB25C, param);
    fx.mem.set32(TIMER, 0x145);
    for (at, node, clear) in [(0xB228u32, 0x14, 1), (0xB22A, 4, 0), (0xB22C, 0xD, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(D + at, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB238, r);
    for k in 0xE..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x298 + 4 * (k as u32 - 0xE), t);
    }
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(D + 0xB1D0 + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x80);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x84)), 180.0);
    fx.mem.set16(D + 0xB25A, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = fx.rand();
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 100..0xB4u32 {
        fx.mem.write_u8(BYTES + k, 0);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = D + 0x70 * k;
        if fx.mem.i16(t + 0x2F8) != 0 && fx.mem.read_u32(t + 0x2E4) != 0 {
            match fx.mem.read_u32(t + 0x2E4) {
                ORB => orb(fx, t + 0x2A0, k),
                RING => ring(fx, t + 0x2A0),
                BURST => burst(fx, t + 0x2A0),
                f => panic!("b548: unknown task step {f:#x}"),
            }
        }
    }
    for k in 0..TASKS {
        let t = D + 0x70 * k;
        if fx.mem.i16(t + 0x2F8) == 0 {
            continue;
        }
        for (f, local, count, mode) in [(ORB, 0x50u32, 1, 0x10), (RING, 0x60, 0xA, 0x20), (BURST, 0x70, 3, 0x10)] {
            if fx.mem.read_u32(t + 0x2E4) != f {
                continue;
            }
            let sub = fx.mem.i16(t + 0x2F6);
            l2::rect(fx, sp + local, 0x14, sub);
            for k in 0..4 {
                let v = fx.mem.read_u32(sp + local + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, v);
            }
            fx.mem.set16(D + 0xC0, 5);
            let tex = fx.mem.read_u32(D + 0xB1D0);
            fx.mem.write_u32(D + 0x88, tex);
            fx.mem.set16(D + 0xB8, count);
            for k in 0..3 {
                let v = fx.mem.read_u32(t + 0x2B0 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, v);
            }
            if f == ORB {
                let s = l2::sin_deg(fx, SINES, 0x2D, 0);
                let y = ee::sub(fx.mem.f(D + 0x24), ee::mul(120.0, s));
                fx.mem.setf(D + 0x24, y);
                let a = fx.mem.i16(t + 0x2FC);
                let s = l2::sin_deg(fx, SINES, a, 0);
                let y = ee::add(fx.mem.f(D + 0x24), ee::mul(120.0, s));
                fx.mem.setf(D + 0x24, y);
                fx.mem.set16(t + 0x2F0, 0);
            }
            fx.mem.set16(D + 0xBE, 0);
            billboard::ring_facing(fx, uv::uv_rect, D, t + 0x2A0, mode, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x82 => 1,
        0x136 => 3,
        0x8D..=0x130 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E00E90(D, 0): the update counter (angle flag D+0xB256 cleared) and
/// the two emitters (frame 0x40: ra, s2 = the parameter, s1 = 0, s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, EMIT_RA), (0x20, param as i64 as u64), (0x10, 0), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    fx.mem.set16(D + 0xB256, 0);
    let (mut s0, mut s1) = (u64::from(D), 0u64);
    if (0x3E..0xFB).contains(&fx.mem.i16(COUNTER)) {
        s1 = 2;
        for k in 0..2 {
            let h = fx.self_handle();
            let slot = fx.mem.i16(D + 0xB228);
            fx.node_position(h, slot, D + 0x250);
            let y = ee::sub(fx.mem.f(D + 0x254), 1.0);
            fx.mem.setf(D + 0x254, y);
            let z = fx.mem.f(D + 0x258);
            let z = if fx.mem.i16(D + 0xB25A) != 0 { ee::add(z, 2.0) } else { ee::sub(z, 2.0) };
            fx.mem.setf(D + 0x258, z);
            spawn(fx, ORB, 0, &[(0x50, 0x01E0_0FAC), (0x20, u64::from(D)), (0x10, s1), (0, k)]);
        }
        s0 = 2;
    }
    if matches!(fx.mem.i16(COUNTER), 0x17 | 0x2B) {
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + 0xB228);
        fx.node_position(h, slot, D + 0x250);
        let y = ee::sub(fx.mem.f(D + 0x254), 4.0);
        fx.mem.setf(D + 0x254, y);
        for ra in [0x01E0_103Cu64, 0x01E0_1050] {
            spawn(fx, RING, 0, &[(0x50, ra), (0x20, u64::from(D)), (0x10, s1), (0, s0)]);
        }
    }
    fx.leave(0x40);
}

/// 0x1E01E50(D, step, arg): the first free task cleared and set off at
/// D+0x250 with the colour D+0xB1F4 (twice), angle flag D+0xB256; 1 when
/// one was free. `saves`: its prologue's (the caller's registers).
fn spawn(fx: &mut Fx, f: u32, arg: u32, saves: &[(u32, u64)]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x70 * k + 0x2F8) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for c in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(D + 0xB1F4), fx.mem.read_u8(D + 0xB1F5), fx.mem.read_u8(D + 0xB1F6));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(D + 0xB1F7);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(D + 0x250 + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let a = fx.mem.i16(D + 0xB256);
            fx.mem.set16(t + 0x50, a);
            fx.mem.write_u32(t + 0x40, arg);
            fx.mem.write_u32(t + 0x44, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// rand % m (HI, the remainder).
fn rand_mod(fx: &mut Fx, m: i32) -> i32 {
    let v = fx.rand();
    fx.div(v, m).1
}

/// rand % m (HI) with rand's frame residue (`ra` = the `jal` + 8).
fn rand_mod_from(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v, m).1
}

/// Fade out by 11 (dead under 13).
fn fade_out(fx: &mut Fx, t: u32) {
    let a = fx.mem.read_u8(t + 0x63);
    if a >= 0xD {
        fx.mem.write_u8(t + 0x63, a - 0xB);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

fn move_xz(fx: &mut Fx, t: u32) {
    for o in [0u32, 8] {
        let v = ee::add(fx.mem.f(t + 0x10 + o), fx.mem.f(t + 0x30 + o));
        fx.mem.setf(t + 0x10 + o, v);
    }
}

/// 0x1E01070(D, task): a violet orb. First step: size 70, a random cell
/// frame, violet, +0x5C = 45, a velocity (-20..19) / (60, 70, 70), z speed
/// (target z - node z) / 80. Fades in for 95 updates, then out (only before
/// update 290; from 291 always out); moves in x / z for its updates 6..39
/// and from 86; z += speed up to age 84, size += 0.5; +0x5C counts to 140,
/// where the orb ends, spawning a burst at its place (y = 42: the
/// original's sine-bobbed y is overwritten). `k`: the entry's loop counter
/// (the burst's spawn saves it).
fn orb(fx: &mut Fx, t: u32, k: u32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x428C_0000);
        let v = fx.rand();
        fx.mem.set16(t + 0x56, (v >> 4) % 8);
        l2::rgb(fx, t + 0x60, 0x80, 0x20, 0xFF);
        fx.mem.set16(t + 0x5C, 0x2D);
        fx.mem.set16(t + 0x50, 0);
        fx.mem.set16(t + 0x52, 0);
        let v = rand_mod(fx, 0x23);
        fx.mem.set16(t + 8, v);
        for o in [0x30u32, 0x34, 0x38] {
            let v = rand_mod(fx, 0x28);
            fx.mem.setf(t + o, ee::from_i32(v - 0x14));
        }
        for (o, q) in [(0x30u32, 60.0), (0x34, 70.0), (0x38, 70.0)] {
            let v = ee::div(fx.mem.f(t + o), q);
            fx.mem.setf(t + o, v);
        }
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x30);
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + 0xB228);
        fx.node_position(h, slot, sp + 0x40);
        let dz = ee::div(ee::sub(fx.mem.f(sp + 0x38), fx.mem.f(sp + 0x48)), 80.0);
        fx.mem.setf(t + 0x4C, dz);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(COUNTER) < 0x122 {
        if fx.mem.i16(t + 0x54) < 0x5F {
            let a = fx.mem.read_u8(t + 0x63);
            if a < 0xC0 {
                fx.mem.write_u8(t + 0x63, a.wrapping_add(0xC0));
            }
        } else {
            fade_out(fx, t);
        }
    }
    if (6..0x28).contains(&fx.mem.i16(t + 0x54)) {
        move_xz(fx, t);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        if fx.mem.i16(t + 0x54) < 0x55 {
            let z = ee::add(fx.mem.f(t + 0x18), fx.mem.f(t + 0x4C));
            fx.mem.setf(t + 0x18, z);
        }
        let s = ee::add(fx.mem.f(t + 0x48), 0.5);
        fx.mem.setf(t + 0x48, s);
    }
    if fx.mem.i16(COUNTER) >= 0x123 {
        fade_out(fx, t);
    }
    if fx.mem.i16(t + 0x5C) < 0x8C {
        let a = fx.mem.i16(t + 0x5C) + 1;
        fx.mem.set16(t + 0x5C, a);
    }
    if fx.mem.i16(t + 0x5C) == 0x8C {
        for o in [0u32, 4, 8, 0xC] {
            let w = fx.mem.read_u32(t + 0x10 + o);
            fx.mem.write_u32(D + 0x250 + o, w);
        }
        let s = l2::sin_deg(fx, SINES, 0x2D, 0);
        let y = ee::sub(fx.mem.f(D + 0x254), ee::mul(120.0, s));
        fx.mem.setf(D + 0x254, y);
        let a = fx.mem.i16(t + 0x5C);
        let s = l2::sin_deg(fx, SINES, a, 0);
        let y = ee::add(fx.mem.f(D + 0x254), ee::mul(120.0, s));
        fx.mem.setf(D + 0x254, y);
        fx.mem.write_u32(D + 0x254, 0x4228_0000);
        let n = fx.mem.i16(COUNTER);
        // The spawn's saves: ra, the entry's s3 (loop counter) / s2 (the
        // task's slot), this step's s1 = D / s0 = the task.
        let saves = |ra: u64| [(0x50, ra), (0x30, u64::from(k)), (0x20, u64::from(t - 0x2A0)), (0x10, u64::from(D)), (0, u64::from(t))];
        if n < 0xAA {
            spawn(fx, BURST, 0, &saves(0x01E0_1414));
        } else if n < 0x122 {
            let v = fx.rand();
            if (v >> 4) % 4 == 0 {
                spawn(fx, BURST, 0, &saves(0x01E0_1464));
            }
        }
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) >= 0x56 {
        move_xz(fx, t);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x50);
}

/// 0x1E014C0(D, task): a white ring (drawn as ten quads). First step: size
/// 100, a random cell frame (rand % 4), white, a random angle, orientation
/// rand % 8, radius +0x50 = 50..99 growing by +0xE (8..20) a update, life
/// +6 (3..7) updates at alpha 0xC0, then fading by 11; size += 1 a
/// update. Its rand frames stay below its own.
fn ring(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42C8_0000);
        let v = fx.rand_from(t, 0x01E0_14E8);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        let v = rand_mod_from(fx, t, 0x01E0_151C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, 0x01E0_1538);
        fx.mem.set16(t + 0xA, v % 8);
        let a = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, a);
        let v = rand_mod_from(fx, t, 0x01E0_155C, 0x32);
        fx.mem.set16(t + 0x50, v + 0x32);
        let v = rand_mod_from(fx, t, 0x01E0_157C, 0xD);
        fx.mem.set16(t + 0xE, v + 8);
        let v = rand_mod_from(fx, t, 0x01E0_159C, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rand_mod_from(fx, t, 0x01E0_15B8, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        fade_out(fx, t);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    let s = ee::add(fx.mem.f(t + 0x48), 1.0);
    fx.mem.setf(t + 0x48, s);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E01670(D, task): an orb's burst (three quads). First step: size 140,
/// a random cell frame (rand % 4), violet, a random angle, orientation
/// rand % 8, radius 30..59 growing by 15..32 a update, life 8..17 updates
/// fading in by 0x60, then out by 11; size += 2 a update.
fn burst(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x430C_0000);
        let v = fx.rand_from(t, 0x01E0_1698);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, 0x80, 0x10, 0xFF);
        let v = rand_mod_from(fx, t, 0x01E0_16CC, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, 0x01E0_16E8);
        fx.mem.set16(t + 0xA, v % 8);
        let a = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, a);
        let v = rand_mod_from(fx, t, 0x01E0_170C, 0x1E);
        fx.mem.set16(t + 0x50, v + 0x1E);
        let v = rand_mod_from(fx, t, 0x01E0_172C, 0x12);
        fx.mem.set16(t + 0xE, v + 0xF);
        let v = rand_mod_from(fx, t, 0x01E0_174C, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rand_mod_from(fx, t, 0x01E0_1768, 0xA);
        fx.mem.set16(t + 6, v + 8);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        let a = fx.mem.read_u8(t + 0x63);
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x60));
        }
    } else {
        fade_out(fx, t);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    let s = ee::add(fx.mem.f(t + 0x48), 2.0);
    fx.mem.setf(t + 0x48, s);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}
