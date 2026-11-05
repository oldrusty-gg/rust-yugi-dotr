//! BMODULE b074 (card 650), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E023E0, data area 0x1ED0000) [F] (helper C7, 31
//! Oct 2026): b147's effect (its init, `b147::init_with`) retimed, with a
//! frame quad (0x1E00280, b4807149dc95; colour +0x1018) until update 0x80
//! fading from 0x71; the stage dims until 0xAA; the tint on the even
//! updates 0x86..0xA9; the discs at the nodes from 0x7B / 0x8F, fading
//! from 0x8C / 0xA0; from 0x8B the target's disc, the 16 strands (all
//! bright again on 0x9F) and the wave (fading from 0xAB); the swaths
//! drawn from 0x87 (+0x1A0) / 0x9B (+0x70), fading from 0x8C / 0xA0.
//! Returns 1 on update 0x81, 6 on 0x8B..0xA9, 2 on 0xAA, 8 after 0xBC.
//!
//! Library at its own addresses: `colours4` (0x1E00080), the tint
//! (0x1E00120), the frame quad (0x1E00280), `swath` push / draw
//! (0x1E003B0 / 0x1E00500), `disc` (0x1E00D00), `wave::draw` (0x1E01260),
//! `strand::draw` (0x1E01A60).
use crate::b147::{init_with, node, strands_step, Init};
use crate::blib::{swath, wave};
use crate::bus::Bus;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const STRAND: u32 = 0x01E0_1A60;
const INIT: Init = Init { nodes: [0x3F, 0x3C, 0x31, 0x2E, 0x3E, 0x30], left: 0xBC, light: 0x101C, table: 0x101D, rand_ra: 0x01E0_24F8, frame: true };

/// The entry 0x1E023E0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    let ret = if param >= 0 { init_with(fx, sp, param, &INIT) } else { step(fx, sp) };
    fx.leave(0xC0);
    ret
}

/// Steps the byte at `p` up to 0xFF before update `from`, else down to 0
/// (both by 0x10).
fn fade(fx: &mut Fx, p: u32, n: i32, from: i32) {
    if n >= from {
        fx.step_down(p, 0, 0x10);
    } else {
        fx.step_up(p, 0xFF, 0x10);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(D + 0xFF4);
    node(fx, 0xFFC, sp + 0x60);
    node(fx, 0x1000, sp + 0x70);
    swath::push(fx, D + 0x70, sp + 0x60, sp + 0x70);
    node(fx, 0x1008, sp + 0x80);
    node(fx, 0x100C, sp + 0x90);
    swath::push(fx, D + 0x1A0, sp + 0x80, sp + 0x90);
    for out in [0x5F0u32, 0x770] {
        let t = fx.target_handle();
        fx.unit_position(t, D + out);
    }
    node(fx, 0x1004, D + 0x6F0);
    node(fx, 0x1010, D + 0x670);
    // s0 = D, s1 = the OT, s2 = sp + 0x90, s3 the runner's (0).
    let regs = [Some(D), Some(OT), Some(sp + 0x90), Some(0)];
    fx.xform_world_saving(D, 0x01E0_297C, regs);
    for (x, ra) in [(0x5D0u32, 0x01E0_2984u32), (0x750, 0x01E0_298C), (0x650, 0x01E0_2994), (0x6D0, 0x01E0_299C)] {
        fx.xform_billboard_saving(D + x, ra, regs);
    }
    if n(fx) < 0x81 {
        fx.frame_quad(D + 0x1018);
        if n(fx) >= 0x71 {
            fx.step_down(D + 0x101B, 0, 6);
        }
    }
    if n(fx) < 0xAB {
        fx.step_down(D + 0x101C, 0x20, 4);
    } else {
        fx.step_up(D + 0x101C, 0x80, 4);
    }
    let l = fx.mem.read_u8(D + 0x101C);
    fx.set_stage_light(i32::from(l));
    let m = n(fx);
    if (0x86..0xAA).contains(&m) && m.wrapping_rem(2) == 0 {
        crate::b262::tint(fx, D + 0x1014);
    }
    if n(fx) >= 0x7B {
        fx.disc(D + 0x6C0, D + 0x650);
        let m = n(fx);
        fade(fx, D + 0x6C7, m, 0x8C);
    }
    if n(fx) >= 0x8F {
        fx.disc(D + 0x740, D + 0x6D0);
        let m = n(fx);
        fade(fx, D + 0x747, m, 0xA0);
    }
    if n(fx) >= 0x8B {
        if n(fx) == 0x9F {
            fx.mem.write_u8(D + 0x647, 0xFF);
            for k in 0..0x10u32 {
                fx.mem.write_u8(D + 0x2DB + 0x30 * k, 0x80);
            }
        }
        fx.disc(D + 0x640, D + 0x5D0);
        fx.step_down(D + 0x647, 0, 0x10);
        strands_step(fx, STRAND, sp, D + 0x5D0);
        fx.approach(D + 0x750, f32::from_bits(0x3E99_999A), f32::from_bits(0x3DCC_CCCD));
        wave::draw(fx, OT, D + 0x7C0, D + 0x750);
        if n(fx) >= 0xAB {
            fx.step_down(D + 0x7CB, 0, 0x10);
        }
    }
    if n(fx) >= 0x87 {
        swath::draw(fx, D + 0x1A0, D);
        let m = n(fx);
        fade(fx, D + 0x1AB, m, 0x8C);
    }
    if n(fx) >= 0x9B {
        swath::draw(fx, D + 0x70, D);
        let m = n(fx);
        fade(fx, D + 0x7B, m, 0xA0);
    }
    let m = n(fx) + 1;
    fx.mem.set32(D + 0xFF4, m);
    let left = fx.mem.i32(D + 0xFF0) - 1;
    fx.mem.set32(D + 0xFF0, left);
    if m == 0xAA {
        2
    } else if m == 0x81 {
        1
    } else if (0x8B..0xAA).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
