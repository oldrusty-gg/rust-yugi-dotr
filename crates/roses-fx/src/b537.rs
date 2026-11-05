//! BMODULE b537 (card 270), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01BA0, data area 0x1ED0000) [F]: b086's older task pool: 150 tasks of 0x40 bytes at D+0x31C
//! (alive +0x36, step +0x24 as a record address) drawn as rings of
//! camera-facing quads: 07c172c43e56 at 0x1E00920
//! (`quads4::ring_facing_i`) and its mode-taking twin 1b8a350ccf03 at
//! 0x1E00EA0 (`quads4::ring_facing_m`), UVs by uv_rect4 0x1E00080. On
//! updates 135..147 two grey smoke rings a update (0x1E01820) leave the
//! attacker's node 0x1D (1 up, 3 toward the facing), drifting toward the
//! target in z by 1 / 160 of the gap a update for 20 updates, then holding
//! while their radius grows (+0xE 4..6 a update to their 100th); their
//! cell frame cycles 0..6. Returns 1 on update 250, 3 on 320, 6 on
//! 261..314, 8 once the 335-update timer +0x2FD0 has run out.
use crate::blib::{quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task may hold (+0x24), as the record's addresses:
/// two idle stubs (drawn, never stepped) and two smoke steps.
const STILL: u32 = 0x01E0_14A0;
const STILL_FAR: u32 = 0x01E0_14B0;
const PUFF: u32 = 0x01E0_14C0;
const SMOKE: u32 = 0x01E0_1820;
const TIMER: u32 = D + 0x2FD0;
const COLOUR: u32 = D + 0x2FD4;
const COUNTER: u32 = D + 0x301A;
const FACING: u32 = D + 0x3014;
const BYTES: u32 = D + 0x301C;

/// The entry 0x1E01BA0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    fx.mem.write_u32(D + 0x2FDC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x3390);
    l2::sine_table(fx, D + 0x321C);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x3018, param);
    fx.mem.set32(TIMER, 0x14F);
    for (k, node) in [0x1D, 1, 0x1B, 0x26, 0xF, 0xA, 0x3D].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(D + 0x2FFC + 2 * k as u32, r);
    }
    for k in 0xB..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x28C + 4 * (k as u32 - 0xB), t);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x268, t);
    for k in 0..2u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x2FB0 + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0x64..0xB4u32 {
        let v = rand_mod(fx, 0x14);
        fx.mem.set8(BYTES + k, v - 0xA);
    }
    for k in 0..0x64u32 {
        let v = rand_mod(fx, 0x50);
        fx.mem.set8(BYTES + k, v - 0x28);
    }
    for k in 0xB4..0x100u32 {
        let v = fx.rand();
        fx.mem.set8(BYTES + k, v % 2 + 1);
    }
    for k in 0..0x100u32 {
        let v = rand_mod(fx, 0x14);
        fx.mem.set8(BYTES + k, v - 0xA);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    for k in 0..TASKS {
        let t = D + 0x40 * k;
        if fx.mem.i16(t + 0x352) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x340) == PUFF {
            smoke(fx, t + 0x31C, false);
        }
        if fx.mem.read_u32(t + 0x340) == SMOKE {
            smoke(fx, t + 0x31C, true);
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    // 0x1F71A0's frame below the entry's (its saved ra and the entry's s0
    // = D).
    fx.saved(sp - 0x20, &[(0x10, 0x01E0_1EAC), (0, u64::from(D))]);
    let y = ee::sub(fx.mem.f(sp + 0x58), 25.0);
    fx.mem.setf(sp + 0x58, y);
    for k in 0..TASKS {
        let t = D + 0x40 * k;
        if fx.mem.i16(t + 0x352) == 0 {
            continue;
        }
        for (f, local, count, mode, table) in [(STILL, 0x60u32, 4, None, BYTES), (STILL_FAR, 0x70, 2, Some(1), BYTES + 0x100), (PUFF, 0x80, 6, Some(1), BYTES), (SMOKE, 0x90, 3, None, BYTES)] {
            if fx.mem.read_u32(t + 0x340) != f {
                continue;
            }
            let tex = fx.mem.read_u32(D + 0x2FB4);
            fx.mem.write_u32(D + 0x88, tex);
            let sub = fx.mem.i16(t + 0x350);
            l2::rect(fx, sp + local, 0x14, sub);
            for k in 0..4 {
                let v = fx.mem.read_u32(sp + local + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, v);
            }
            fx.mem.set16(D + 0xB8, count);
            let v = fx.rand();
            fx.mem.set16(D + 0xBE, v % 4);
            match (f, mode) {
                (SMOKE, _) => quads4::ring_facing_m(fx, uv::uv_rect4, D, t + 0x31C, 0x20, table),
                (_, m) => quads4::ring_facing_i(fx, uv::uv_rect4, D, t + 0x31C, m.unwrap_or(0), table),
            }
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xFA => 1,
        0x140 => 3,
        0x105..=0x13A => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E013B0(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the emitter: on updates 135..147 two smoke rings (+0x30 = 0, 1) at
/// node 0x1D, 1 up (frame 0x40: ra, the entry's s0 = D).
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x10, 0x01E0_1E34), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if (0x87..0x94).contains(&fx.mem.i16(COUNTER)) {
        for (at, out) in [(0x2FFCu32, 0x20u32), (0x2FFE, 0x30)] {
            let h = fx.self_handle();
            let slot = fx.mem.i16(D + at);
            fx.node_position(h, slot, sp + out);
        }
        let x = fx.mem.read_u32(sp + 0x20);
        fx.mem.write_u32(D + 0x250, x);
        let y = ee::add(1.0, fx.mem.f(sp + 0x24));
        fx.mem.setf(D + 0x254, y);
        let z = fx.mem.read_u32(sp + 0x28);
        fx.mem.write_u32(D + 0x258, z);
        for (k, ra) in [0x01E0_1468u64, 0x01E0_1484].into_iter().enumerate() {
            fx.mem.set16(D + 0x3012, k as i32);
            spawn(fx, SMOKE, 0, &[(0x50, ra), (0x20, u64::MAX), (0x10, 0), (0, u64::from(D))]);
        }
    }
    fx.leave(0x40);
}

/// 0x1E021C0(D, step, arg): the first free task cleared and set off at
/// D+0x250 (as i16) with the colour D+0x2FD4 (twice), +0x30 = D+0x3012,
/// +0x3A = D+0x300E; 1 when one was free (b086's 0x1E01730 with other
/// offsets [F]). `saves`: its prologue's.
fn spawn(fx: &mut Fx, f: u32, arg: u32, saves: &[(u32, u64)]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x40 * k + 0x352) == 0) {
        Some(k) => {
            let t = D + 0x31C + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            for c in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            let a = fx.mem.i16(D + 0x3012);
            fx.mem.set16(t + 0x30, a);
            let a = fx.mem.i16(D + 0x300E);
            fx.mem.set16(t + 0x3A, a);
            fx.mem.write_u32(t + 0x20, arg);
            fx.mem.write_u32(t + 0x24, f);
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

/// 0x1E014C0 (`turning` false) / 0x1E01820 (D, task): a smoke ring. First
/// step: size +0x38 = 4, cell frame +0x34 = rand % 8, +0xA = the spawn's
/// +0x30, grey 0x50 / 0xA0, (0x1E014C0: a random angle +0x3A), radius
/// growth +0xE 4..6, table row +8 0..24, life +6 90..179 (+0xA = 0) or
/// 0..9; 0x1E01820 moves 3 toward the facing. Fades in by 0x10 below 0x30
/// for its life, then out by 4. For 20 updates z moves by (target z -
/// self z) / 160 and the size grows by 2; then (with +0xA = 0) after
/// update 179 z moves by 1 / 160 (1 / 136 from 230) and the size grows by
/// 2. On odd ages the cell frame cycles 0..6; the radius grows on ages
/// 15..99; 0x1E01820 turns by 6 after age 20.
fn smoke(fx: &mut Fx, t: u32, turning: bool) {
    let sp = fx.enter(0x60);
    let ra = if turning { 0x01E0_1E88 } else { 0x01E0_1E68 };
    fx.saved(sp, &[(0x20, ra), (0x10, 0), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 4);
        let v = fx.rand();
        fx.mem.set16(t + 0x34, v % 8);
        let a = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, a);
        let c = if turning { 0xA0 } else { 0x50 };
        l2::rgb(fx, t + 0x28, c, c, c);
        fx.mem.set16(t + 0x30, 0);
        if !turning {
            let v = rand_mod(fx, 0x168);
            fx.mem.set16(t + 0x3A, v);
        }
        let v = rand_mod(fx, 3);
        fx.mem.set16(t + 0xE, v + 4);
        let v = rand_mod(fx, 0x19);
        fx.mem.set16(t + 8, v);
        let life = if fx.mem.i16(t + 0xA) == 0 { rand_mod(fx, 0x5A) + 0x5A } else { rand_mod(fx, 0xA) };
        fx.mem.set16(t + 6, life);
        if turning {
            let z = fx.mem.i16(t + 4);
            let z = if fx.mem.i16(FACING) != 0 { z + 3 } else { z - 3 };
            fx.mem.set16(t + 4, z);
        }
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < 0x30 {
            fx.mem.write_u8(t + 0x2B, a + 0x10);
        }
    } else if a >= 6 {
        fx.mem.write_u8(t + 0x2B, a - 4);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x30);
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x40);
    let dz = ee::sub(fx.mem.f(sp + 0x38), fx.mem.f(sp + 0x48));
    fx.mem.setf(sp + 0x38, dz);
    let drift = |fx: &mut Fx, q: f32| {
        let dz = ee::div(fx.mem.f(sp + 0x38), q);
        fx.mem.setf(sp + 0x38, dz);
        let z = ee::add(ee::from_i32(fx.mem.i16(t + 4)), dz);
        fx.mem.set16(t + 4, ee::fixsfsi(z));
    };
    if fx.mem.i16(t + 0x32) < 0x14 {
        drift(fx, 160.0);
    } else if fx.mem.i16(t + 0xA) == 0 {
        let c = fx.mem.i16(COUNTER);
        if c >= 0xB3 {
            drift(fx, if c < 0xE6 { 160.0 } else { 136.0 });
        }
        let s = fx.mem.i16(t + 0x38) + 2;
        fx.mem.set16(t + 0x38, s);
    }
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let c = fx.mem.i16(t + 0x34);
        fx.mem.set16(t + 0x34, if c < 6 { c + 1 } else { 0 });
    }
    let age = fx.mem.i16(t + 0x32);
    if (0xF..0x64).contains(&age) {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    let age = fx.mem.i16(t + 0x32);
    if age < 0x14 && age != 0 {
        let s = fx.mem.i16(t + 0x38) + 2;
        fx.mem.set16(t + 0x38, s);
    }
    if turning && fx.mem.i16(t + 0x32) >= 0x15 {
        let a = fx.mem.i16(t + 0x3A) + 6;
        fx.mem.set16(t + 0x3A, a);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x60);
}
