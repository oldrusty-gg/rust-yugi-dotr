//! BMODULE b135 (card 309), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E027B0, data area 0x1ED0000) [F] (helper C15, 31
//! Oct 2026): b648's family without the flat sprites. One drifting ribbon
//! (`blib::driftribbon`, +0x70 / +0xC0: 256 slots, width 150 growing 10,
//! x scaled 1.03, fade 8, life 64) fed from update 0xC from the attacker's
//! node 0x42 and drawn from 0xD with a random grey (0x40 + rand % 0x40
//! per channel each update), its alpha up 8 to 0x80 and down from 0xAF;
//! 32 puffs (+0x11A0 / +0x1200, TEXEFF 1, (0x80, 0x10, 0x60), half size 20
//! growing 15) thrown 14 in random directions (0x1043F0) from 0x6F, slowed
//! x 1.03, fading by 4, respawned at age 16, the group's alpha up 1 to
//! 0x40 and down 2 from 0xBE; every update a frame flash
//! (`flash::frame_flash`, +0x1420: 0x80, 0x10, 0x60; alpha up 8 to 0x80,
//! down from 0xBE); the target's point less the attacker's over 80 into
//! +0x90 (0x103660). Returns 1 on update 0x65, 6 on 0x6F..0xBD, 2 on 0xBE,
//! 8 after 0xE6.
//!
//! Library (record addresses): `colours4` 0x1E00080, `flash::frame_flash`
//! 0x1E00120, `Fx::sprites` 0x1E00370, `driftribbon::push` 0x1E01C60,
//! `driftribbon::draw` 0x1E01DD0; the executable's 0x103660
//! [`crate::b079::sub3`].
use crate::b079::sub3;
use crate::blib::{driftribbon, flash};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const N: u32 = D + 0x1414;
const LEFT: u32 = D + 0x1410;
const FLASH: u32 = D + 0x1420;
const RIBBON: u32 = D + 0x70;
const RIB_PTS: u32 = D + 0xC0;
const PUFFS: u32 = D + 0x11A0;
const PARTS: u32 = D + 0x1200;

/// The entry 0x1E027B0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x80);
    ret
}

fn halves(fx: &mut Fx, at: u32, v: &[(u32, i32)]) {
    for &(k, x) in v {
        fx.mem.set16(at + k, x);
    }
}

fn bytes(fx: &mut Fx, at: u32, v: &[(u32, u8)]) {
    for &(k, x) in v {
        fx.mem.write_u8(at + k, x);
    }
}

/// A puff thrown: a random direction of length 14 (0x1043F0 into sp +
/// 0x40) as its speed; 0x1043F0's prologue (0x80: s0 = D +0, `ra` +0x10)
/// left below the stack pointer.
fn throw(fx: &mut Fx, sp: u32, p: u32, ra: u32) {
    fx.random_dir(sp + 0x40, 14.0);
    fx.saved(sp - 0x80, &[(0, u64::from(D)), (0x10, u64::from(ra))]);
    for c in [0u32, 4, 8] {
        let v = fx.mem.read_u32(sp + 0x40 + c);
        fx.mem.write_u32(p + c, v);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x1430);
    for x in [0u32, 0x10C0, 0x1130] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x1424u32, 0x12, 1), (0x1428, 1, 0), (0x142C, 0x42, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(LEFT, 0xE6);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x1418, param);
    fx.mem.set32(D + 0x141C, 0x10);
    bytes(fx, FLASH, &[(2, 0x60), (1, 0x10), (0, 0x80), (3, 0)]);
    // The ribbon (+0x70): the attacker's texture 2, flags 0x8003, grey
    // 0x80, width 150 growing 10 an age, x scaled 1.03, cell (0, 0, 0x20,
    // 0x40), fade 8, life 64, 256 slots.
    fx.texture5(sp + 0x70, 2);
    let tex = fx.mem.read_u32(sp + 0x70);
    let g = RIBBON;
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 8, 0x8003);
    bytes(fx, g, &[(6, 0x80), (5, 0x80), (4, 0x80), (7, 0)]);
    halves(fx, g, &[(0x10, 0), (0x12, 0), (0x14, 0x20), (0x16, 0x40), (0x18, 0), (0x1A, 0), (0x1C, 0x20), (0x1E, 0x40)]);
    fx.mem.write_u32(g + 0xC, 0x4316_0000);
    fx.mem.write_u32(g + 0x34, 0x4120_0000);
    fx.mem.write_u32(g + 0x30, 0x3F83_D70A);
    halves(fx, g, &[(0x3A, 8), (0x3C, 0), (0x3E, 0x40), (0x40, 0x100), (0x42, 0), (0x44, 1)]);
    // The puffs (+0x11A0): 32 sprites of TEXEFF 1 (0x80, 0x10, 0x60, alpha
    // 0x40), flags 0x48000003, half size 20 growing 15 an age, cell (0, 0,
    // 0x18, 0x20) of 0x80 x 0x40, 8 frames every 16 updates; thrown 14 in
    // random directions, age -(rand % 12).
    fx.texture4(sp + 0x70, 1);
    let tex = fx.mem.read_u32(sp + 0x70);
    let g = PUFFS;
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x4800_0003);
    bytes(fx, g, &[(0xA, 0x60), (9, 0x10), (8, 0x80), (0xB, 0x40)]);
    fx.mem.write_u32(g + 0xC, 0x41A0_0000);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x30, 0), (0x32, 0), (0x34, 0x18), (0x36, 0x20), (0x38, 0), (0x3A, 0), (0x3C, 0x80), (0x3E, 0x40)]);
    fx.mem.write_u32(g + 0x40, PARTS);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4170_0000);
    fx.mem.write_u32(g + 0x50, 0x4120_0000);
    halves(fx, g, &[(0x5A, 0x20), (0x5C, 8), (0x5E, 0x10)]);
    let mut k = 0;
    while k < fx.mem.i16(g + 0x5A) {
        let p = PARTS + 16 * k as u32;
        throw(fx, sp, p, 0x01E0_29D8);
        let r = fx.rand_from(D, 0x01E0_29F4);
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0xE, h.wrapping_neg());
        fx.mem.write_u8(p + 0xC, 0x40);
        let f = fx.rand_from(D, 0x01E0_2A1C).wrapping_rem(8);
        fx.mem.set8(p + 0xD, f);
        k += 1;
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let h = fx.self_handle();
    fx.unit_point(1, h, sp + 0x50);
    let t = fx.target_handle();
    fx.unit_point(1, t, sp + 0x60);
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x1150);
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x90);
    sub3(fx, D + 0x90, sp + 0x50);
    for c in [0u32, 4, 8] {
        let q = ee::div(fx.mem.f(D + 0x90 + c), 80.0);
        fx.mem.setf(D + 0x90 + c, q);
    }
    let n = fx.mem.i32(N);
    if n >= 0xC {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + 0x142C);
        fx.node_position(h, s, D + 0x10E0);
        driftribbon::push(fx, RIBBON, RIB_PTS, D + 0x10E0);
        // The push's prologue (0x60: s0 = D, s1 = the OT, s2 = the -1
        // parameter as 64 bits, s3 / s4 the runner's 0, ra +0x50).
        let f = fx.sp - 0x60;
        fx.saved(f, &[(0, u64::from(D)), (0x10, u64::from(OT)), (0x20, u64::MAX), (0x30, 0), (0x40, 0), (0x50, 0x01E0_2B2C)]);
    }
    // 0x1038D0's frames (s0 = D, s1 = the OT, s2 = the -1 parameter): the
    // last one's TRS translation row (the target's point) at sp - 0x10 is
    // the flash's textured quad's TEX1 residue [run].
    let regs = [Some(D), Some(OT), Some(0xFFFF_FFFF), None];
    fx.xform_world_saving(D, 0x01E0_2B38, regs);
    fx.xform_billboard(D + 0x10C0);
    fx.xform_world_saving(D + 0x1130, 0x01E0_2B48, regs);
    // The flash's alpha; its first TEXFLUSH reports the step's (floor /
    // cap, step, the byte before) [run b648].
    let before = u32::from(fx.mem.read_u8(FLASH + 3));
    let a123 = if n >= 0xBE {
        fx.step_down(FLASH + 3, 0, 8);
        [0, 8, before]
    } else {
        fx.step_up(FLASH + 3, 0x80, 8);
        [0x80, 8, before]
    };
    flash::frame_flash(fx, FLASH, a123);
    if n >= 0xD {
        for (o, ra) in [(6u32, 0x01E0_2BA4u32), (5, 0x01E0_2BC4), (4, 0x01E0_2BE4)] {
            let v = (fx.rand_from(D, ra).wrapping_rem(0x40) + 0x40) as u8;
            fx.mem.write_u8(RIBBON + o, v);
        }
        driftribbon::draw(fx, OT, RIBBON, RIB_PTS, D);
        if n >= 0xAF {
            fx.step_down(RIBBON + 7, 0, 8);
        } else {
            fx.step_up(RIBBON + 7, 0x80, 8);
        }
    }
    if n >= 0x6F {
        fx.sprites(PUFFS, D + 0x1130);
        if n >= 0xBE {
            fx.step_down(PUFFS + 0xB, 0, 2);
        } else {
            fx.step_up(PUFFS + 0xB, 0x40, 1);
        }
        let mut k = 0;
        while k < fx.mem.i16(PUFFS + 0x5A) {
            let p = PARTS + 16 * k as u32;
            k += 1;
            if fx.mem.i16(p + 0xE) >= 0 {
                for c in [0u32, 4, 8] {
                    let v = ee::mul(fx.mem.f(p + c), f32::from_bits(0x3F83_D70A));
                    fx.mem.setf(p + c, v);
                }
                fx.step_down(p + 0xC, 0, 4);
            }
            if fx.mem.i16(p + 0xE) < 0x10 {
                continue;
            }
            throw(fx, sp, p, 0x01E0_2D20);
            fx.mem.set16(p + 0xE, 0);
            fx.mem.write_u8(p + 0xC, 0x40);
        }
    }
    let n = n + 1;
    fx.mem.set32(N, n);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if n == 0xBE {
        2
    } else if n == 0x65 {
        1
    } else if (0x6F..0xBE).contains(&n) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
