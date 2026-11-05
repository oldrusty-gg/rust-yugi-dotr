//! BMODULE b538 (card 125), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E00EA0, data area 0x1ED0000) [F]: an older task pool: 200 tasks of 0x40 bytes at D+0x300 (alive
//! +0x36, step +0x24, all 0x1E00B60) drawn as rings of seven camera-facing
//! quads (07c172c43e56 at 0x1E004D0, `quads4::ring_facing_i`, UVs by
//! uv_rect4 0x1E00080). From update 88 to 301 one violet puff a update
//! leaves the attacker's node 0x22 (3.5 in z by the facing, 1 down),
//! drifts in x by the gap to node 0x3A and toward the target in z by 1 / 40
//! (1 / 60 after 40 updates) of the gap a update, its cell frame stepping
//! to 5 and its radius / size growing. Returns 1 on update 90, 4 on 370, 2
//! on 355, 6 on 101..349, 8 once the 372-update timer +0x3580 has run out.
use crate::blib::{quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 200;
/// The one step function a task holds (+0x24), as the record's address.
const PUFF: u32 = 0x01E0_0B60;
/// The entry's `jal` return from the emitter (0x1E01058 + 8): its
/// prologue saves sit below the entry's frame (the rings' stale TEX0 /
/// TEX1 when the texture is missing).
const EMIT_RA: u64 = 0x01E0_1060;
const TIMER: u32 = D + 0x3580;
const COUNTER: u32 = D + 0x35E8;
const BYTES: u32 = D + 0x35EC;

/// The entry 0x1E00EA0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x358C, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x80);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x3860);
    l2::sine_table(fx, D + 0x36EC);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x35E6, param);
    fx.mem.set32(TIMER, 0x174);
    for k in 0..3 {
        fx.mem.write_u32(D + 0x290 + 4 * k, 0);
    }
    for (k, (node, clear)) in [(0x22, 1), (0x3A, 0), (0x40, 0), (0x47, 0), (0x4E, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(D + 0x35C8 + 2 * k as u32, r);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x70);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x74)), 180.0);
    fx.mem.set16(D + 0x35E4, ee::fixsfsi(y));
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(D + 0x3500 + 4 * k as u32, t);
    }
    for k in 0..0x100u32 {
        let v = fx.rand();
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0..8u32 {
        let v = fx.rand();
        fx.mem.set16(D + 0xC0 + 2 * k, v % 4);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    for k in 0..TASKS {
        let t = D + 0x40 * k;
        if fx.mem.i16(t + 0x336) != 0 && fx.mem.read_u32(t + 0x324) == PUFF {
            puff(fx, t + 0x300);
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    // 0x1F71A0's frame below the entry's (its saved ra 0x1E010BC and s0):
    // the stale half of the rects' copy (D+0x9C..+0xA3) for the first
    // task drawn.
    fx.saved(sp - 0x20, &[(0x10, 0x01E0_10BC), (0, 0)]);
    for k in 0..TASKS {
        let t = D + 0x40 * k;
        if fx.mem.i16(t + 0x336) == 0 || fx.mem.read_u32(t + 0x324) != PUFF {
            continue;
        }
        let tex = fx.mem.read_u32(D + 0x3500);
        fx.mem.write_u32(D + 0x88, tex);
        let sub = fx.mem.i16(t + 0x334);
        l2::rect(fx, sp + 0x60, 0x14, sub);
        for k in 0..4 {
            let v = fx.mem.read_u32(sp + 0x60 + 4 * k);
            fx.mem.write_u32(D + 0x94 + 4 * k, v);
        }
        fx.mem.set16(D + 0xB8, 7);
        let v = fx.rand();
        fx.mem.set16(D + 0xBE, v % 4);
        quads4::ring_facing_i(fx, uv::uv_rect4, D, t + 0x300, 0, BYTES);
        fx.mem.set16(D + 0xB8, 0);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x5A => 1,
        0x172 => 4,
        0x163 => 2,
        0x65..=0x15D => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E00A50(D, 0): the spawn colour D+0x3584 (0x80, 0, 0, 0), the update
/// counter and the emitter (frame 0x40: ra, the entry's s1 = D / s0 = 0).
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x20, EMIT_RA), (0x10, u64::from(D)), (0, 0)]);
    fx.mem.write_u8(D + 0x3584, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(D + 0x3584 + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if (0x58..0x12E).contains(&fx.mem.i16(COUNTER)) {
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + 0x35C8);
        fx.node_position(h, slot, sp + 0x30);
        let x = fx.mem.read_u32(sp + 0x30);
        fx.mem.write_u32(D + 0x250, x);
        let y = ee::sub(fx.mem.f(sp + 0x34), 1.0);
        fx.mem.setf(D + 0x254, y);
        let z = fx.mem.read_u32(sp + 0x38);
        fx.mem.write_u32(D + 0x258, z);
        let z = fx.mem.f(D + 0x258);
        let z = if fx.mem.i16(D + 0x35E4) != 0 { ee::add(z, 3.5) } else { ee::sub(z, 3.5) };
        fx.mem.setf(D + 0x258, z);
        spawn(fx, PUFF, 0);
    }
    fx.leave(0x40);
}

/// 0x1E01220(D, step, arg): the first free task cleared and set off at
/// D+0x250 (as i16) with the colour D+0x3584 (twice), +0x30 = D+0x35E2; 1
/// when one was free.
fn spawn(fx: &mut Fx, f: u32, arg: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, 0x01E0_0B34), (0x20, u64::MAX), (0x10, 0), (0, u64::from(D))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x40 * k + 0x336) == 0) {
        Some(k) => {
            let t = D + 0x300 + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            for c in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(D + 0x3584), fx.mem.read_u8(D + 0x3585), fx.mem.read_u8(D + 0x3586));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(D + 0x3587);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            let a = fx.mem.i16(D + 0x35E2);
            fx.mem.set16(t + 0x30, a);
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

/// 0x1E00B60(D, task): a violet puff. First step: size +0x38 = 70, violet,
/// a random angle +0x3A, radius growth +0xE 5..9, table row +8 0..24,
/// life +6 45..54, x step +0xC = node 0x3A - node 0x22. Fades in by 0x30
/// (to 0x90) for its life, then out by 9; z moves by (target z - self z)
/// / 40 (/ 60 after 40 updates); on odd ages x += step and the cell frame
/// +0x34 counts to 5; radius += growth, size += 1; three random angles.
fn puff(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x20, 0x01E0_1094), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x46);
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, 0xC0, 0x10, 0xFF);
        fx.mem.set16(t + 0x30, 0);
        let v = rand_mod(fx, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rand_mod(fx, 5);
        fx.mem.set16(t + 0xE, v + 5);
        let v = rand_mod(fx, 0x19);
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, 0xA);
        fx.mem.set16(t + 6, v + 0x2D);
        for (at, out) in [(0x35C8u32, 0x30u32), (0x35CA, 0x40)] {
            let h = fx.self_handle();
            let slot = fx.mem.i16(D + at);
            fx.node_position(h, slot, sp + out);
        }
        let dx = ee::sub(fx.mem.f(sp + 0x40), fx.mem.f(sp + 0x30));
        fx.mem.set16(t + 0xC, ee::fixsfsi(dx));
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < 0x61 {
            fx.mem.write_u8(t + 0x2B, a + 0x30);
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
    let q = if fx.mem.i16(t + 0x32) < 0x28 { 40.0 } else { 60.0 };
    let dz = ee::div(fx.mem.f(sp + 0x38), q);
    fx.mem.setf(sp + 0x38, dz);
    let z = ee::add(ee::from_i32(fx.mem.i16(t + 4)), dz);
    fx.mem.set16(t + 4, ee::fixsfsi(z));
    if fx.mem.i16(t + 0x32) % 2 == 1 {
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
    if fx.mem.i16(t + 0x32) != 0 {
        let s = fx.mem.i16(t + 0x38) + 1;
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
