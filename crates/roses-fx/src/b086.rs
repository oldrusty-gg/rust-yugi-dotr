//! BMODULE b086 (card 302), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01290, data area 0x1ED0000) [F]: b538's older task pool: 150 tasks of 0x40 bytes at D+0x65C
//! (alive +0x36, step +0x24) drawn as rings of camera-facing quads
//! (07c172c43e56 at 0x1E004D0, `quads4::ring_facing_i`, UVs by uv_rect4
//! 0x1E00080). From update 45, while the 300-update timer +0x3950 has 71
//! or more left, two grey / red smoke rings (0x1E00BA0, three quads) and
//! two darker ones (0x1E00F70, two quads, far Z) leave the attacker's node
//! 0x41 a update, drifting toward the target in z and sinking toward the
//! attacker's height. Returns 1 on update 90, 3 on 275, 6 on 101..269, 8
//! once the timer has run out.
use crate::blib::{quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x24), as the record's addresses.
const SMOKE: u32 = 0x01E0_0BA0;
const DARK: u32 = 0x01E0_0F70;
/// The entry's `jal` return from the emitter (0x1E014E4 + 8): its
/// prologue saves sit below the entry's frame.
const EMIT_RA: u64 = 0x01E0_14EC;
const TIMER: u32 = D + 0x3950;
const COUNTER: u32 = D + 0x39B6;
const BYTES: u32 = D + 0x39BA;

/// The entry 0x1E01290.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    fx.mem.write_u32(D + 0x395C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x70);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3D30);
    l2::sine_table(fx, D + 0x3BBC);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x39B4, param);
    fx.mem.set32(TIMER, 0x12C);
    for k in 0..3 {
        fx.mem.write_u32(D + 0x310 + 4 * k, 0);
    }
    for (k, node) in [0x41, 0x69, 0x1B, 0x26, 0xF, 0xA, 0x3D].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(D + 0x3998 + 2 * k as u32, r);
    }
    for k in 0xB..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x34C + 4 * (k as u32 - 0xB), t);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x328, t);
    let t = fx.texture(5, 0);
    fx.mem.write_u32(D + 0x3930, t);
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

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = D + 0x40 * k;
        if fx.mem.i16(t + 0x692) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x680) == SMOKE {
            smoke(fx, t + 0x65C, Kind::Smoke);
        }
        if fx.mem.read_u32(t + 0x680) == DARK {
            smoke(fx, t + 0x65C, Kind::Dark);
        }
    }
    for k in 0..TASKS {
        let t = D + 0x40 * k;
        if fx.mem.i16(t + 0x692) == 0 {
            continue;
        }
        for (f, local, count, depth, table) in [(SMOKE, 0x50u32, 3, 0, BYTES), (DARK, 0x60, 2, 1, BYTES + 0x100)] {
            if fx.mem.read_u32(t + 0x680) != f {
                continue;
            }
            let tex = fx.mem.read_u32(D + 0x3930);
            fx.mem.write_u32(D + 0x88, tex);
            let sub = fx.mem.i16(t + 0x690);
            l2::rect(fx, sp + local, 0x14, sub);
            for k in 0..4 {
                let v = fx.mem.read_u32(sp + local + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, v);
            }
            fx.mem.set16(D + 0xB8, count);
            let v = fx.rand();
            fx.mem.set16(D + 0xBE, v % 4);
            quads4::ring_facing_i(fx, uv::uv_rect4, D, t + 0x65C, depth, table);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x5A => 1,
        0x113 => 3,
        0x65..=0x10D => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E00A50(D, 0): the spawn colour D+0x3954 (0x80, 0, 0, 0), the update
/// counter and the emitter: from update 45 while the timer has 71 or more
/// left, at node 0x41 two smoke rings then two dark ones (+0x30 = 0, 1
/// through D+0x39B0) (frame 0x30: ra, the entry's s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x10, EMIT_RA), (0, u64::from(D))]);
    fx.mem.write_u8(D + 0x3954, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(D + 0x3954 + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    for (f, ras) in [(SMOKE, [0x01E0_0AE4u64, 0x01E0_0B00]), (DARK, [0x01E0_0B68, 0x01E0_0B84])] {
        if fx.mem.i16(COUNTER) < 0x2D || fx.mem.i32(TIMER) < 0x47 {
            continue;
        }
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + 0x3998);
        fx.node_position(h, slot, sp + 0x20);
        for k in 0..3 {
            let w = fx.mem.read_u32(sp + 0x20 + 4 * k);
            fx.mem.write_u32(D + 0x2D0 + 4 * k, w);
        }
        for (k, ra) in ras.into_iter().enumerate() {
            fx.mem.set16(D + 0x39B0, k as i32);
            spawn(fx, f, 0, &[(0x50, ra), (0x20, param as i64 as u64), (0x10, 0), (0, u64::from(D))]);
        }
    }
    fx.leave(0x30);
}

/// 0x1E01730(D, step, arg): the first free task cleared and set off at
/// D+0x2D0 (as i16) with the colour D+0x3954 (twice), +0x30 = D+0x39B0,
/// +0x3A = D+0x39AA; 1 when one was free. `saves`: its prologue's.
fn spawn(fx: &mut Fx, f: u32, arg: u32, saves: &[(u32, u64)]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x40 * k + 0x692) == 0) {
        Some(k) => {
            let t = D + 0x65C + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            for c in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(D + 0x3954), fx.mem.read_u8(D + 0x3955), fx.mem.read_u8(D + 0x3956));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(D + 0x3957);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x2D0, 0);
            let a = fx.mem.i16(D + 0x39B0);
            fx.mem.set16(t + 0x30, a);
            let a = fx.mem.i16(D + 0x39AA);
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

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// 0x1E00BA0.
    Smoke,
    /// 0x1E00F70.
    Dark,
}

/// 0x1E00BA0 / 0x1E00F70 (D, task): a smoke ring. First step: size +0x38
/// 30 / 40, grey (flag +0x30 = 0) or red, a random angle +0x3A, radius
/// growth +0xE 5..9, table row +8 0..24, life +6 45..54 / 2..5, at node
/// 0x41 (3 lower). Fades in by 0x2A / 0x20 for its life, then out by 9; z
/// moves by (target z - self z) / 40 (smoke after 40 updates: / 50, radius
/// += 5 x growth, 2 down; dark: / 63) and the ring sinks by 1 toward 15
/// under the attacker's height; on odd ages the cell frame +0x34 counts to
/// 5 (smoke: x += +0xC); radius += growth, size += 3 / 1; three random
/// angles.
fn smoke(fx: &mut Fx, t: u32, kind: Kind) {
    let sp = fx.enter(0x60);
    let ra = if kind == Kind::Smoke { 0x01E0_1520 } else { 0x01E0_1540 };
    fx.saved(sp, &[(0x20, ra), (0x10, 0), (0, u64::from(D))]);
    let dark = kind == Kind::Dark;
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, if dark { 0x28 } else { 0x1E });
        fx.mem.set16(t + 0x34, 0);
        let (grey, red) = if dark { (0x30, 0x40) } else { (0x40, 0x50) };
        if fx.mem.i16(t + 0x30) == 0 {
            l2::rgb(fx, t + 0x28, grey, grey, grey);
        } else {
            l2::rgb(fx, t + 0x28, red, 0x10, 0x10);
        }
        fx.mem.set16(t + 0x30, 0);
        let v = rand_mod(fx, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rand_mod(fx, 5);
        fx.mem.set16(t + 0xE, v + 5);
        let v = rand_mod(fx, 0x19);
        fx.mem.set16(t + 8, v);
        let life = if dark {
            let v = fx.rand();
            v % 4 + 2
        } else {
            rand_mod(fx, 0xA) + 0x2D
        };
        fx.mem.set16(t + 6, life);
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + 0x3998);
        fx.node_position(h, slot, sp + 0x30);
        let x = ee::fixsfsi(fx.mem.f(sp + 0x30));
        fx.mem.set16(t, x);
        let y = ee::fixsfsi(ee::add(3.0, fx.mem.f(sp + 0x34)));
        fx.mem.set16(t + 2, y);
        let z = ee::fixsfsi(fx.mem.f(sp + 0x38));
        fx.mem.set16(t + 4, z);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        let (below, by) = if dark { (0x41, 0x20) } else { (0x80, 0x2A) };
        if a < below {
            fx.mem.write_u8(t + 0x2B, a + by);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x2B, a - 9);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x30);
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x40);
    let dz = ee::sub(fx.mem.f(sp + 0x38), fx.mem.f(sp + 0x48));
    fx.mem.setf(sp + 0x38, dz);
    let early = !dark && fx.mem.i16(t + 0x32) < 0x28;
    let q = if dark { 63.0 } else if early { 40.0 } else { 50.0 };
    let dz = ee::div(fx.mem.f(sp + 0x38), q);
    fx.mem.setf(sp + 0x38, dz);
    let z = ee::add(ee::from_i32(fx.mem.i16(t + 4)), dz);
    fx.mem.set16(t + 4, ee::fixsfsi(z));
    if dark || early {
        let floor = ee::sub(fx.mem.f(sp + 0x44), 15.0);
        let y = fx.mem.i16(t + 2);
        if !(ee::from_i32(y) <= floor) {
            fx.mem.set16(t + 2, y - 1);
        }
    } else {
        let r = fx.mem.i16(t + 0x30) + 5 * fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
        let y = fx.mem.i16(t + 2) + 2;
        fx.mem.set16(t + 2, y);
    }
    if !dark && fx.mem.i16(t + 0x32) % 2 == 1 {
        let x = fx.mem.i16(t) + fx.mem.i16(t + 0xC);
        fx.mem.set16(t, x);
    }
    if fx.mem.i16(t + 0x32) % 2 == 1 && fx.mem.i16(t + 0x34) < 5 {
        let c = fx.mem.i16(t + 0x34) + 1;
        fx.mem.set16(t + 0x34, c);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    if dark {
        let s = fx.mem.i16(t + 0x38) + 1;
        fx.mem.set16(t + 0x38, s);
    } else if fx.mem.i16(t + 0x32) != 0 {
        let s = fx.mem.i16(t + 0x38) + 3;
        fx.mem.set16(t + 0x38, s);
    }
    for o in [0x10u32, 0x14, 0x18] {
        let v = rand_mod(fx, 0x168);
        fx.mem.set32(t + o, v);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x60);
}
