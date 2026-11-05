//! BMODULE b505 (card 418), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E013F0, data area 0x1ED0000) [F]: an older task pool of 150 tasks of 0x40 bytes at D+0x300 (alive
//! +0x36, step +0x24 as a record address). From update 1 a full-window
//! red tint (an untextured gouraud quad, 0x109DD0) pulses (alpha by 10
//! up to 0xA0 and back, until it is out after update 210); every update
//! up to 199 a ring of three camera-facing quads (2d29e1f9b4be at
//! 0x1E00920, `quads4::ring_at`, UVs by uv_rect 0x1E00080) leaves each of
//! the attacker's nodes 0x11 and 0x15, wobbling up and down, turning by
//! 2 and drifting 4 a update away from the facing. Returns 1 on update
//! 90, 4 on 230, 2 on 220, 6 on 101..214, 8 once the 255-update timer
//! +0x2B80 has run out.
//!
//! The steps 0x1E01160 (a frame tint through 0x1F7E20), 0x1E012E0 (the
//! stage light) and 0x1E013A0 (the object op 0x1F8380) and the step
//! function 0x1E010C0 (inlined in the entry) are never spawned [F: the
//! emitter's two `jal`s name 0x1E010C0 and 0x1E00E00 only]; not ported.
use crate::blib::{quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x24), as the record's addresses.
const RING: u32 = 0x01E0_0E00;
const TINT: u32 = 0x01E0_10C0;
const TIMER: u32 = D + 0x2B80;
const COLOUR: u32 = D + 0x2B84;
const COUNTER: u32 = D + 0x2BCA;
const BYTES: u32 = D + 0x2BCC;

/// The entry 0x1E013F0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x130);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x2B8C, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x130);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2E40);
    l2::sine_table(fx, D + 0x2CCC);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x2BC8, param);
    fx.mem.set32(TIMER, 0xFF);
    for (k, node) in [0x11, 0x15, 0x1C, 0x30, 0x39].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(D + 0x2BAC + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 0);
    fx.mem.set16(D + 0x2BBA, r);
    for k in 0xB..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x2EC + 4 * (k as u32 - 0xB), t);
    }
    let t = fx.texture(5, 0);
    fx.mem.write_u32(D + 0x2B60, t);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x80);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x84)), 180.0);
    fx.mem.set16(D + 0x2BC6, ee::fixsfsi(y));
    for k in 0..0x100u32 {
        let v = rand_mod(fx, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = D + 0x300 + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x24) == RING {
            ring(fx, t);
        }
        if fx.mem.read_u32(t + 0x24) == TINT {
            tint(fx, t);
        }
        // 0x1E01160 / 0x1E012E0 / 0x1E013A0: never spawned (see above).
        debug_assert!(!matches!(fx.mem.read_u32(t + 0x24), 0x01E0_1160 | 0x01E0_12E0 | 0x01E0_13A0));
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x60);
    // 0x1F71A0's frame below the entry's (its saved ra and the entry's s0
    // = D).
    fx.saved(sp - 0x20, &[(0x10, 0x01E0_173C), (0, u64::from(D))]);
    for k in 0..TASKS {
        let t = D + 0x300 + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x24) == RING {
            let w = fx.mem.read_u32(D + 0x2B60);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x34);
            l2::rect(fx, sp + 0x70, 0x14, sub);
            for k in 0..4 {
                let v = fx.mem.read_u32(sp + 0x70 + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, v);
            }
            for k in 0..3 {
                let v = ee::from_i32(fx.mem.i16(t + 2 * k));
                fx.mem.setf(sp + 0x60 + 4 * k, v);
            }
            fx.mem.set16(D + 0xB8, 3);
            fx.mem.set16(D + 0xBE, 0);
            quads4::ring_at(fx, uv::uv_rect, D, t, BYTES, sp + 0x60);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x24) == TINT {
            let ot = fx.mem.read_u32(D + 0x2B8C);
            let d = sp + 0x98;
            l2::colour_strip4(fx, d, t + 0x28, 0);
            l2::desc_gouraud4(fx, d, 0);
            l2::fullscreen4(fx, d);
            fx.submit(d, ot, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x5A => 1,
        0xE6 => 4,
        0xDC => 2,
        0x65..=0xD6 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E00CE0(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the emitters: on update 1 the tint, on updates 1..199 a ring at
/// each of nodes 0x11 / 0x15 (frame 0x50: ra, the entry's s2 = -1 / s1 =
/// 0 / s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x30, 0x01E0_15B8), (0x20, param as i64 as u64), (0x10, 0), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, TINT, &[(0x50, 0x01E0_0D34), (0x20, param as i64 as u64), (0x10, 0), (0, u64::from(D))]);
    }
    let c = fx.mem.i16(COUNTER);
    if c > 0 && c < 0xC8 {
        for (at, ra) in [(0x2BACu32, 0x01E0_0D98u64), (0x2BAE, 0x01E0_0DDC)] {
            let h = fx.self_handle();
            let slot = fx.mem.i16(D + at);
            fx.node_position(h, slot, sp + 0x40);
            for k in 0..3 {
                let w = fx.mem.read_u32(sp + 0x40 + 4 * k);
                fx.mem.write_u32(D + 0x250 + 4 * k, w);
            }
            spawn(fx, RING, &[(0x50, ra), (0x20, u64::from(sp + 0x48)), (0x10, u64::from(sp + 0x44)), (0, u64::from(D))]);
        }
    }
    fx.leave(0x50);
}

/// 0x1E01920(D, step, 0): the first free task cleared and set off at
/// D+0x250 (as i16) with the colour D+0x2B84 (twice), +0x30 = D+0x2BC4; 1
/// when one was free. `saves`: its prologue's.
fn spawn(fx: &mut Fx, f: u32, saves: &[(u32, u64)]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x336 + 0x40 * k) == 0) {
        Some(k) => {
            let t = D + 0x300 + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            for c in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            let a = fx.mem.i16(D + 0x2BC4);
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

/// rand % m (HI, the remainder).
fn rand_mod(fx: &mut Fx, m: i32) -> i32 {
    let v = fx.rand();
    fx.div(v, m).1
}

/// rand % m (HI) from a step's `jal` at `site` (its frame: s0 = `t`).
fn rand_at(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, site + 8);
    fx.div(v, m).1
}

/// 0x1E00E00(D, task): a ring: size +0x38 7..26, grey, radius +0x30
/// 10..159 growing by +0xE 7..16, a random angle turning by 2 (+0xA = 1)
/// or -2, row +8 0..29, life +6 30..44; fades in by 0x19 below 0x80, then
/// out by 20; the cell frame counts to 6 (not on multiples of 15); up by
/// 1..4 on odd ages, down by 1..4 on even; 4 a update away from the
/// facing.
fn ring(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_15EC), (0x10, 0), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x36) == 1 {
        let v = rand_at(fx, t, 0x01E0_0E24, 0x14);
        fx.mem.set16(t + 0x38, v + 7);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        let v = rand_at(fx, t, 0x01E0_0E54, 0x96);
        fx.mem.set16(t + 0x30, v + 0xA);
        let v = rand_at(fx, t, 0x01E0_0E74, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rand_at(fx, t, 0x01E0_0E90, 0xA);
        fx.mem.set16(t + 0xE, v + 7);
        let v = rand_at(fx, t, 0x01E0_0EB0, 0x1E);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, 0x01E0_0ED4);
        fx.mem.set16(t + 0xA, v % 2);
        let v = rand_at(fx, t, 0x01E0_0EE8, 0xF);
        fx.mem.set16(t + 6, v + 0x1E);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x2B, a + 0x19);
        }
    } else if a >= 0x16 {
        fx.mem.write_u8(t + 0x2B, a - 0x14);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    let age = fx.mem.i16(t + 0x32);
    if fx.div(age, 0xF).1 != 0 && fx.mem.i16(t + 0x34) < 6 {
        let c = fx.mem.i16(t + 0x34) + 1;
        fx.mem.set16(t + 0x34, c);
    }
    let odd = fx.mem.i16(t + 0x32) % 2 == 1;
    // Its frame (s0 = t, ra) lands at the entry's sp - 0x40 / - 0x38: a
    // later quad's stale TEX0 when the texture is missing.
    let v = fx.rand_from(t, if odd { 0x01E0_0FE8 } else { 0x01E0_1020 });
    let dy = (v % 4 + 1) as i16 as i32;
    let y = fx.mem.i16(t + 2);
    fx.mem.set16(t + 2, if odd { y + dy } else { y - dy });
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    let a = fx.mem.i16(t + 0x3A);
    let turn = if fx.mem.i16(t + 0xA) != 0 { 2 } else { -2 };
    fx.mem.set16(t + 0x3A, a + turn);
    let z = fx.mem.i16(t + 4);
    let dz = if fx.mem.i16(D + 0x2BC6) != 0 { -4 } else { 4 };
    fx.mem.set16(t + 4, z + dz);
    fx.leave(0x30);
}

/// The tint (0x1E010C0's body, inlined in the entry 0x1E01608): alpha
/// +0x2B up by 10 to 0xA0 (+0x30 = 0), then down by 10 (+0x30 = 1) until
/// below 12, when +0x30 is 0 again (and after update 210 the task ends);
/// colour (0x5F, 0x3F, 0x3F).
fn tint(fx: &mut Fx, t: u32) {
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x30) == 0 {
        if a < 0xA0 {
            fx.mem.write_u8(t + 0x2B, a + 0xA);
        } else {
            fx.mem.set16(t + 0x30, 1);
        }
    } else if a >= 0xC {
        fx.mem.write_u8(t + 0x2B, a - 0xA);
    } else {
        if fx.mem.i16(COUNTER) >= 0xD3 {
            fx.mem.set16(t + 0x36, 0);
        }
        fx.mem.set16(t + 0x30, 0);
    }
    l2::rgb(fx, t + 0x28, 0x5F, 0x3F, 0x3F);
}
