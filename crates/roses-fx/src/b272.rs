//! BMODULE b272 (card 655), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E013C0, data area 0x1ED0000) [F]: a task pool of 150 sprites (0x70 bytes at +0x2A0, alive flag
//! +0x58, step function +0x44) drawn with the second library's quad
//! (add5d81dd3fe at 0x1E00920, UVs by uv_rect 0x1E00080). From update 36
//! to 180 a sprite rises at the attacker's node 1 each update; from 220 to
//! 350 two sprites a update fall at the target's node 1, each spawning a
//! spark (0x1E011C0) when it has grown. Returns 1 on update 225, 2 on 375,
//! 4 on 385, 6 on 236..369, 8 once the 400-update timer +0x44B0 has run
//! out.
use crate::blib::{billboard, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const RISE: u32 = 0x01E0_0E80;
const FALL: u32 = 0x01E0_0FE0;
const SPARK: u32 = 0x01E0_11C0;
const IDLE: u32 = 0x01E0_13B0;
/// The return address of the entry's `jalr` into a step (0x1E016A8 + 8);
/// the steps' prologue saves (ra, the entry's s1 = 0, s0 = D) are read
/// back as stale TEX0 / TEX1 by the quads when the texture is missing.
const TASK_RA: u64 = 0x01E0_16B0;

/// The entry 0x1E013C0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    fx.mem.write_u32(D + 0x44BC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4890);
    l2::sine_table(fx, D + 0x471C);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x4518, param);
    fx.mem.set32(D + 0x44B0, 0x190);
    for (at, node, clear) in [(0x44E4u32, 1, 1), (0x44E6, 0x34, 0), (0x44E8, 0x35, 0), (0x44EA, 0x30, 0), (0x44EC, 0x2F, 0), (0x44EE, 0x2C, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(D + at, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0x44F4, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(D + 0x4490 + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.mem.set16(D + 0x4516, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = fx.rand();
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(D + 0x451C + k, h - 0xF);
    }
    for k in 100..0xB4u32 {
        let v = fx.rand();
        fx.mem.set8(D + 0x451C + k, v % 4);
    }
    for k in 0..0x100u32 {
        let v = fx.rand();
        let (_, h) = fx.div(v, 0x28);
        fx.mem.set8(D + 0x461C + k, h - 0x14);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    for k in 0..TASKS {
        let t = D + 0x70 * k;
        if fx.mem.i16(t + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x2E4);
        if f != 0 && f != IDLE {
            run_task(fx, f, t + 0x2A0);
        }
    }
    for k in 0..TASKS {
        let t = D + 0x70 * k;
        if fx.mem.i16(t + 0x2F8) == 0 {
            continue;
        }
        for (f, tex, local, order) in [(RISE, 0x4494u32, 0x70u32, None), (FALL, 0x4494, 0x80, Some(0x2F2u32)), (SPARK, 0x4490, 0x90, None)] {
            if fx.mem.read_u32(t + 0x2E4) != f {
                continue;
            }
            draw(fx, sp, t, tex, sp + local, order);
        }
    }
    let left = fx.mem.read_u32(D + 0x44B0);
    if left != 0 {
        fx.mem.write_u32(D + 0x44B0, left - 1);
    }
    match fx.mem.i16(D + 0x451A) {
        0xE1 => 1,
        0x181 => 4,
        0x177 => 2,
        0xEC..=0x171 => 6,
        _ if fx.mem.i32(D + 0x44B0) > 0 => 0,
        _ => 8,
    }
}

/// One sprite of task `t` (D + 0x70 k): the cell 0x14 frame +0x2F6 of the
/// atlas (0x106960) into D+0x94, orientation 5 (or the task's +0x2F2),
/// raised by 4 sin(+0x2FA degrees), blend 2 at the mean w.
fn draw(fx: &mut Fx, sp: u32, t: u32, tex: u32, r: u32, order: Option<u32>) {
    let w = fx.mem.read_u32(D + tex);
    fx.mem.write_u32(D + 0x88, w);
    let sub = fx.mem.i16(t + 0x2F6);
    l2::rect(fx, r, 0x14, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
    let o = match order {
        Some(off) => fx.mem.i16(t + off),
        None => 5,
    };
    fx.mem.set16(D + 0xBE, o);
    let a = fx.mem.i16(t + 0x2FA);
    let s = l2::sin_deg(fx, D + 0x471C, a, 0);
    let x = ee::add(fx.mem.f(t + 0x2B0), ee::mul(4.0, s));
    fx.mem.setf(sp + 0x60, x);
    let y = fx.mem.read_u32(t + 0x2B4);
    fx.mem.write_u32(sp + 0x64, y);
    let z = fx.mem.read_u32(t + 0x2B8);
    fx.mem.write_u32(sp + 0x68, z);
    billboard::quad(fx, uv::uv_rect, D, t + 0x2A0, 0x20, sp + 0x60);
}

fn run_task(fx: &mut Fx, f: u32, t: u32) {
    match f {
        RISE => rise(fx, t),
        FALL => fall(fx, t),
        SPARK => spark(fx, t),
        _ => panic!("b272: unknown task step {f:#x}"),
    }
}

/// D+0x250 += (rand % m - o) on the axis at `a`.
fn jitter(fx: &mut Fx, a: u32, m: i32, o: i32) {
    let v = fx.rand();
    let (_, h) = fx.div(v, m);
    let x = ee::add(fx.mem.f(a), ee::from_i32(h - o));
    fx.mem.setf(a, x);
}

/// 0x1E00BE0(D): the update counter +0x451A and the emitters.
fn emit(fx: &mut Fx) {
    fx.enter(0x20);
    let n = fx.mem.i16(D + 0x451A) + 1;
    fx.mem.set16(D + 0x451A, n);
    let n = fx.mem.i16(D + 0x451A);
    if (0x24..0xB5).contains(&n) {
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + 0x44E4);
        fx.node_position(h, slot, D + 0x250);
        jitter(fx, D + 0x250, 0x1E, 0xF);
        let v = fx.rand();
        let (_, h) = fx.div(v, 0xA);
        let y = ee::sub(fx.mem.f(D + 0x254), ee::from_i32(h));
        fx.mem.setf(D + 0x254, y);
        jitter(fx, D + 0x258, 0x1E, 0xF);
        spawn(fx, RISE, 0);
    }
    let n = fx.mem.i16(D + 0x451A);
    if (0xDC..0x15F).contains(&n) {
        for _ in 0..2 {
            let h = fx.target_handle();
            let slot = fx.mem.i16(D + 0x44F4);
            fx.node_position(h, slot, D + 0x250);
            jitter(fx, D + 0x250, 0x28, 0x14);
            let v = fx.rand();
            let (_, h) = fx.div(v, 0x1E);
            fx.mem.setf(D + 0x254, ee::from_i32(h + 0x46));
            jitter(fx, D + 0x258, 0x28, 0x14);
            spawn(fx, FALL, 0);
        }
    }
    fx.leave(0x20);
}

/// 0x1E019E0(D, step, arg): the first free task cleared and set off at
/// D+0x250 with the colour D+0x44B4 (twice), angle flag D+0x4512; 1 when
/// one was free.
fn spawn(fx: &mut Fx, f: u32, arg: u32) -> i32 {
    fx.enter(0x60);
    let ret = spawn_in(fx, f, arg);
    fx.leave(0x60);
    ret
}

fn spawn_in(fx: &mut Fx, f: u32, arg: u32) -> i32 {
    let Some(k) = (0..TASKS).find(|&k| fx.mem.i16(D + 0x70 * k + 0x2F8) == 0) else { return 0 };
    let t = D + 0x2A0 + 0x70 * k;
    fx.bzero(t, 0x70);
    fx.mem.set16(t + 0x58, 1);
    for c in [0x60u32, 0x64] {
        let (r, g, b) = (fx.mem.read_u8(D + 0x44B4), fx.mem.read_u8(D + 0x44B5), fx.mem.read_u8(D + 0x44B6));
        l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
        let a = fx.mem.read_u8(D + 0x44B7);
        fx.mem.write_u8(t + c + 3, a);
    }
    l2::to_i16x4(fx, t, D + 0x250, 0);
    for k in 0..3 {
        let w = fx.mem.read_u32(D + 0x250 + 4 * k);
        fx.mem.write_u32(t + 0x10 + 4 * k, w);
    }
    let a = fx.mem.i16(D + 0x4512);
    fx.mem.set16(t + 0x50, a);
    fx.mem.write_u32(t + 0x40, arg);
    fx.mem.write_u32(t + 0x44, f);
    1
}

/// A sprite's first step: size 10, cell frame 3, height 0.5, white,
/// a random angle and turn direction.
fn sprite_start(fx: &mut Fx, t: u32) {
    fx.mem.set16(t + 8, 0xA);
    fx.mem.set16(t + 0xA, 0xA);
    fx.mem.set16(t + 0x56, 3);
    fx.mem.write_u32(t + 0x34, 0x3F00_0000);
    l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
    let v = fx.rand();
    let (_, h) = fx.div(v, 0x168);
    fx.mem.set16(t + 0x5A, h);
    let v = fx.rand();
    fx.mem.set16(t + 0x50, (v >> 4) % 2);
    fx.mem.set16(t + 0x58, 2);
}

/// The common tail: y += dy (`rise`) or -= dy (when not the first
/// update), the camera's angles into +0x20, the angle +0x5A turned by 5,
/// the age +0x54 counted. 0x205580's saves (ra = the step's `ra`, s1 =
/// t + 0x20, s0 = t) stay below the step's frame, where the quads'
/// descriptor later holds its TEX0 / TEX1 when the texture is missing.
fn sprite_end(fx: &mut Fx, t: u32, dy: f32, rise: bool, ra: u32) {
    if fx.mem.i16(t + 0x54) != 0 {
        let y = fx.mem.f(t + 0x14);
        let y = if rise { ee::add(y, dy) } else { ee::sub(y, dy) };
        fx.mem.setf(t + 0x14, y);
    }
    let base = fx.sp.wrapping_sub(0x30);
    fx.saved(base, &[(0x20, u64::from(ra)), (0x10, u64::from(t + 0x20)), (0, u64::from(t))]);
    l2::camera_angles_to(fx, t + 0x20);
    let a = fx.mem.i16(t + 0x5A);
    let a = if fx.mem.i16(t + 0x50) == 0 { a + 5 } else { a - 5 };
    fx.mem.set16(t + 0x5A, a);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// The alpha +0x63 up by 0x60 while below 0xC0.
fn fade_in(fx: &mut Fx, t: u32) {
    let a = fx.mem.read_u8(t + 0x63);
    if a < 0xC0 {
        fx.mem.write_u8(t + 0x63, a + 0x60);
    }
}

/// 0x1E00E80: rises by 1 a update, fading in for 20 updates, then out by
/// 9 until gone.
fn rise(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        sprite_start(fx, t);
    }
    if fx.mem.i16(t + 0x54) < 0x14 {
        fade_in(fx, t);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xB {
            fx.mem.write_u8(t + 0x63, a - 9);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    sprite_end(fx, t, 1.0, true, 0x01E0_0F90);
    fx.leave(0x20);
}

/// 0x1E00FE0: falls by 1 a update, fading in for 15 updates, then grows
/// (height +0xA by 15 to 100) and on reaching it spawns a spark at its
/// position and ends; its width +8 shrinks by 2 on every third update.
fn fall(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        sprite_start(fx, t);
    }
    if fx.mem.i16(t + 0x54) < 0xF {
        fade_in(fx, t);
    } else {
        let h = fx.mem.i16(t + 0xA);
        if h < 0x64 {
            fx.mem.set16(t + 0xA, h + 0xF);
        } else {
            for k in 0..4 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x250 + 4 * k, w);
            }
            let a = fx.mem.i16(t + 0x50);
            fx.mem.set16(D + 0x4512, a);
            spawn(fx, SPARK, 0);
            fx.mem.set16(t + 0x58, 0);
        }
        let w = fx.mem.i16(t + 8);
        if w > 0 {
            let n = fx.mem.i16(t + 0x54);
            let (_, r) = fx.div(n, 3);
            if r == 1 {
                fx.mem.set16(t + 8, w - 2);
            }
        }
    }
    sprite_end(fx, t, 1.0, false, 0x01E0_116C);
    fx.leave(0x30);
}

/// 0x1E011C0: a spark: random cell frame / orientation (0..7 signed),
/// growing for 5 updates then shrinking and fading by 9, flickering white
/// / grey, sinking by 0.5.
fn spark(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0);
        fx.mem.set16(t + 0xA, 0);
        let v = fx.rand();
        fx.mem.set16(t + 0x56, (v >> 4) % 8);
        let v = fx.rand();
        fx.mem.set16(t + 0x52, (v >> 4) % 8);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 5 {
        fade_in(fx, t);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xB {
            fx.mem.write_u8(t + 0x63, a - 9);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let w = fx.mem.i16(t + 8);
        if w > 0 {
            fx.mem.set16(t + 8, w - 6);
        }
    }
    if fx.mem.i16(t + 0x54) < 5 {
        let h = fx.mem.i16(t + 0xA);
        if h < 0x1E {
            fx.mem.set16(t + 0xA, h + 6);
        }
        let w = fx.mem.i16(t + 8);
        if w < 0x1E {
            fx.mem.set16(t + 8, w + 3);
        }
    }
    let c = if fx.mem.i16(t + 0x54) % 2 == 1 { 0xFF } else { 0xC0 };
    l2::rgb(fx, t + 0x60, c, c, c);
    sprite_end(fx, t, 0.5, false, 0x01E0_1364);
    fx.leave(0x20);
}
