//! Small task steps and a spawn shared by a few BMODULE records (helper
//! A27, 3 Nov 2026), hand-ported from the records' code [F]. Each step is
//! `step(D, t) -> 0` on a 0x40-byte task `t` (+6 / +8 / +0xE / +0x10..
//! fields per step, +0x28 / +0x2C colours, +0x30 a parameter, +0x32 the
//! age, +0x34 the frame, +0x36 the state (1 new, 2 running, 0 ended),
//! +0x38 the size, +0x3A the angle) in a 0x20 frame (s0 +0, ra +0x10: the
//! caller writes them); rands from s0 = t at the noted return addresses.
//! Divisions by constants (`div` / `mfhi`) go through [`Fx::div`] so LO /
//! HI follow the original's.
use crate::bus::Bus;
use crate::exe::Fx;
use crate::fxlib2::rgb;

/// Alpha +0x2B: `a` + `up` while the age is under `n` (only below
/// `cap`), afterwards down by `down` while at least `floor`, else the task
/// ends (+0x36 = 0).
fn fade(fx: &mut Fx, t: u32, young: bool, up: Option<(u8, u8)>, set: Option<u8>, down: u8, floor: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if young {
        if let Some(v) = set {
            fx.mem.write_u8(t + 0x2B, v);
        } else if let Some((cap, by)) = up {
            if a < cap {
                fx.mem.write_u8(t + 0x2B, a.wrapping_add(by));
            }
        }
    } else if a >= floor {
        fx.mem.write_u8(t + 0x2B, a.wrapping_sub(down));
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// 411a83bd7c2c [2 B records: 216 / 225; card 225 0x1E03990, 52 words]:
/// on its first update colours +0x28 / +0x2C (0x60, 0x60, 0xC0), +6 = 8,
/// alphas 0x80 / 0x40, size 6; alpha 0x80 for 0x78 updates, then down by
/// 9 (ends below 0xB).
pub fn blue_glow(fx: &mut Fx, t: u32) -> i32 {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        rgb(fx, t + 0x28, 0x60, 0x60, 0xC0);
        rgb(fx, t + 0x2C, 0x60, 0x60, 0xC0);
        fx.mem.set16(t + 6, 8);
        fx.mem.write_u8(t + 0x2B, 0x80);
        fx.mem.write_u8(t + 0x2F, 0x40);
        fx.mem.set16(t + 0x38, 6);
        fx.mem.set16(t + 0x36, 2);
    }
    let young = fx.mem.i16(t + 0x32) < 0x78;
    fade(fx, t, young, None, Some(0x80), 9, 0xB);
    age(fx, t);
    fx.leave(0x20);
    0
}

/// dbf1ff684b7d [2 B records: 225 / 376; card 225 0x1E04F70, 72 words]:
/// `spawn(D, a, b) -> 1 / 0`: the first free (+0x36 0) of 150 0x40-byte
/// tasks at D+0x4F4 cleared (bzero) and set alive, colours +0x28 / +0x2C
/// both D+0x3004..7 (alpha D+0x3007), +0 the four floats D+0x2D0 as i16
/// (0x107390), +0x30 = D+0x3062, +0x20 = b, +0x24 = a. A 0x60 frame.
pub fn spawn(fx: &mut Fx, d: u32, a: u32, b: u32) -> i32 {
    fx.enter(0x60);
    let mut t = 0;
    for k in 0..150u32 {
        let p = d + 0x4F4 + 0x40 * k;
        if fx.mem.i16(p + 0x36) == 0 {
            fx.bzero(p, 0x40);
            fx.mem.set16(p + 0x36, 1);
            t = p;
            break;
        }
    }
    let r = if t != 0 {
        for o in [0x28u32, 0x2C] {
            let (r, g, bl) = (fx.mem.read_u8(d + 0x3004), fx.mem.read_u8(d + 0x3005), fx.mem.read_u8(d + 0x3006));
            rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(bl));
            let al = fx.mem.read_u8(d + 0x3007);
            fx.mem.write_u8(t + o + 3, al);
        }
        crate::fxlib2::to_i16x4(fx, t, d + 0x2D0, 0);
        let p = fx.mem.i16(d + 0x3062);
        fx.mem.set16(t + 0x30, p);
        fx.mem.write_u32(t + 0x20, b);
        fx.mem.write_u32(t + 0x24, a);
        1
    } else {
        0
    };
    fx.leave(0x60);
    r
}

/// 4d39a3cb7f05 [3 B records: 376 / 580 / 679; card 376 0x1E03000, 52
/// words]: on its first update size 0x3C, colours (0x80, 0x80, 0x80) /
/// (0xC0, 0xC0, 0), angle +0x3A = +0x30, alpha +0x2F 0; alpha +0x2B up by
/// 0x50 (wrapping) on update 0, then down by 9 (ends below 0xB); a
/// negative size shrinks by 2.
pub fn white_flash(fx: &mut Fx, t: u32) -> i32 {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x3C);
        rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        rgb(fx, t + 0x2C, 0xC0, 0xC0, 0);
        let p = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0x3A, p);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if fx.mem.i16(t + 0x32) <= 0 {
        let a = fx.mem.read_u8(t + 0x2B);
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x50));
    } else {
        fade(fx, t, false, None, None, 9, 0xB);
    }
    age(fx, t);
    let s = fx.mem.i16(t + 0x38);
    if s < 0 {
        fx.mem.set16(t + 0x38, s - 2);
    }
    fx.leave(0x20);
    0
}

/// aad4d81131d3 [6 B records: 56 / 57 / 353 / 430 / 438 / 575; card 56
/// 0x1E02610, 108 words]: on its first update size 0xA, frame rand & 3,
/// +0x10 / +0x14 / +0x18 three rand % 0xFFFF, colour (0x20, 0x10, 8),
/// +0x30 = 0, angle rand % 360, +0xE = (rand & 0xF) + 3, +8 = rand % 10,
/// life +6 = rand % 7 + 5; alpha up by 0x38 (below 0x71) for the life,
/// then down by 0xA (ends below 0x11); +0x30 += +0xE after update 0.
pub fn ember(fx: &mut Fx, at: u32, t: u32) -> i32 {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0xA);
        let v = fx.rand_from(t, at + 0x28);
        fx.mem.set16(t + 0x34, v & 3);
        for (o, ra) in [(0x10u32, 0x44u32), (0x14, 0x60), (0x18, 0x7C)] {
            let v = fx.rand_from(t, at + ra);
            let r = fx.div(v, 0xFFFF).1;
            if o == 0x18 {
                rgb(fx, t + 0x28, 0x20, 0x10, 8);
            }
            fx.mem.set32(t + o, r);
        }
        fx.mem.set16(t + 0x30, 0);
        let v = fx.rand_from(t, at + 0xA8);
        let r = fx.div(v, 360).1;
        fx.mem.set16(t + 0x3A, r);
        let v = fx.rand_from(t, at + 0xC4);
        fx.mem.set16(t + 0xE, (v & 0xF) + 3);
        let v = fx.rand_from(t, at + 0xE4);
        let r = fx.div(v, 10).1;
        fx.mem.set16(t + 8, r);
        let v = fx.rand_from(t, at + 0x100);
        let r = fx.div(v, 7).1;
        fx.mem.set16(t + 6, r + 5);
        fx.mem.set16(t + 0x36, 2);
    }
    let young = fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6);
    fade(fx, t, young, Some((0x71, 0x38)), None, 0xA, 0x11);
    if fx.mem.i16(t + 0x32) != 0 {
        let v = fx.mem.i16(t + 0x30).wrapping_add(fx.mem.i16(t + 0xE));
        fx.mem.set16(t + 0x30, v);
    }
    age(fx, t);
    fx.leave(0x20);
    0
}

/// 6823b7a0f488 [5 B records: 56 / 57 / 229 / 353 / 438; card 56
/// 0x1E027C0, 100 words]: on its first update size 2, frame rand & 3,
/// colour (0x80, 0x40, 0), +0x18 / +0x10 two rand % 0xFFFF, +0x30 = 0,
/// angle rand % 360, +0xE = (rand & 0xF) + 3, +8 = rand % 10, life +6 =
/// rand % 20 + 10; alpha 0x80 for the life, then down by 0xA (ends below
/// 0x11); +0x30 += +0xE every update.
pub fn spark(fx: &mut Fx, at: u32, t: u32) -> i32 {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 2);
        let v = fx.rand_from(t, at + 0x28);
        fx.mem.set16(t + 0x34, v & 3);
        rgb(fx, t + 0x28, 0x80, 0x40, 0);
        for (o, ra) in [(0x18u32, 0x5Cu32), (0x10, 0x78)] {
            let v = fx.rand_from(t, at + ra);
            let r = fx.div(v, 0xFFFF).1;
            fx.mem.set32(t + o, r);
        }
        fx.mem.set16(t + 0x30, 0);
        let v = fx.rand_from(t, at + 0x98);
        let r = fx.div(v, 360).1;
        fx.mem.set16(t + 0x3A, r);
        let v = fx.rand_from(t, at + 0xB4);
        fx.mem.set16(t + 0xE, (v & 0xF) + 3);
        let v = fx.rand_from(t, at + 0xD4);
        let r = fx.div(v, 10).1;
        fx.mem.set16(t + 8, r);
        let v = fx.rand_from(t, at + 0xF0);
        let r = fx.div(v, 20).1;
        fx.mem.set16(t + 6, r + 10);
        fx.mem.set16(t + 0x36, 2);
    }
    let young = fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6);
    fade(fx, t, young, None, Some(0x80), 0xA, 0x11);
    let v = fx.mem.i16(t + 0x30).wrapping_add(fx.mem.i16(t + 0xE));
    fx.mem.set16(t + 0x30, v);
    age(fx, t);
    fx.leave(0x20);
    0
}
