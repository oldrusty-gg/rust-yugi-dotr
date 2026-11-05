//! BMODULE b227 (card 119), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01A10, data area 0x1ED0000) [F] (helper C, 30 Oct
//! 2026): a burst of 32 sparks thrown out from a point (+0x11B0, the
//! attacker's node 2, then the target's animated position from update
//! 150), each flying off by 1.03 per update and respawning when faded; a
//! fountain of 128 sprites (+0xE0) round the attacker's node 0x1E, turned
//! to look from node 0x20 toward node 0x1E, rising 8 / 9 / 10 per update
//! and spreading by 1.12 until 180 high. Returns 1 on update 113, 6 on
//! 123..233, 2 on 234, 8 after 256.
//!
//! b514 (card 638) is the same program with other nodes, timings and
//! colours ([`Variant`], `crate::b514`).
//!
//! Library: `sprites` (0x1E00120, [`crate::fxlib`]); the executable's
//! 0x103DB0 / 0x103290 ([`crate::libm`]), 0x1043F0 / 0x1F65D0
//! ([`crate::bexe`]).
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// What b227 and b514 differ in [F: their entries differ only here].
pub struct Variant {
    /// The node requests (slot 0x141C look-from, 0x1420 look-at / the
    /// fountain, 0x1424 the burst before `switch`).
    pub nodes: [i32; 3],
    /// Updates (0x1410).
    pub length: i32,
    /// The burst's flags (+0x11B4) and colour (B, G, R, A).
    pub burst_flags: u32,
    pub burst_colour: [u8; 4],
    /// The fountain's colour (B, G, R, A).
    pub fountain_colour: [u8; 4],
    /// The burst moves to the target (and turns visible) here.
    pub switch: i32,
    /// At `switch` the burst's flag 0x1000 is cleared.
    pub switch_clears: bool,
    /// The burst fades in until `fade_in`, out until `switch`, in until
    /// `fade_out`, then out by 8.
    pub fade_in: i32,
    pub fade_out: i32,
    /// The fountain fades out from here (by 8).
    pub fountain_out: i32,
    /// Return 1 / 6 from / 2.
    pub ret1: i32,
    pub ret6: i32,
    pub ret2: i32,
}

/// b227's.
pub const B227: Variant = Variant {
    nodes: [0x20, 0x1E, 2],
    length: 0x100,
    burst_flags: 0x4808_0003,
    burst_colour: [0x40, 0x80, 0x40, 0x80],
    fountain_colour: [0x40, 0x80, 0x40, 0],
    switch: 0x96,
    switch_clears: false,
    fade_in: 0x70,
    fade_out: 0xEB,
    fountain_out: 0xEA,
    ret1: 0x71,
    ret6: 0x7B,
    ret2: 0xEA,
};

/// The entry 0x1E01A10.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B227)
}

/// The shared entry (b227 / b514 0x1E01A10).
pub fn run(fx: &mut Fx, param: i32, v: &Variant) -> i32 {
    let sp = fx.enter(0x90);
    let ret = if param >= 0 { init(fx, sp, param, v) } else { step(fx, sp, v) };
    fx.leave(0x90);
    ret
}

pub(crate) fn colour(fx: &mut Fx, at: u32, c: [u8; 4]) {
    for k in 0..4u32 {
        fx.mem.write_u8(at + k, c[k as usize]);
    }
}

/// A fountain sprite (`p`) set off: angle `a`, rise `y`, age `age`.
pub(crate) fn fountain_spawn(fx: &mut Fx, p: u32, respawn: bool) {
    let a = fx.rand() & 0xFFFF;
    let r = fx.rand();
    let (_, h) = fx.div(r, 20);
    if respawn {
        fx.mem.set16(p + 0x14E, -h);
    } else {
        fx.mem.setf(p + 0x144, ee::from_i32(h));
    }
    let c = fx.tcos(a);
    let x = ee::mul(fx.mem.f(D + 0x12C), c);
    fx.mem.setf(p + 0x148, x);
    let s = fx.tsin(a);
    let x = ee::mul(fx.mem.f(D + 0x12C), s);
    fx.mem.setf(p + 0x140, x);
    let r = fx.rand();
    let (_, h) = fx.div(r, 20);
    if respawn {
        fx.mem.setf(p + 0x144, ee::from_i32(h));
        fx.mem.write_u8(p + 0x14C, 0);
    } else {
        fx.mem.set16(p + 0x14E, -h);
        fx.mem.write_u8(p + 0x14C, 0);
        fx.mem.write_u8(p + 0x14D, 0);
    }
}

/// A spark (`p`) thrown out again: a random direction of length 10.
pub(crate) fn spark(fx: &mut Fx, sp: u32, p: u32) {
    fx.random_dir(sp + 0x50, 10.0);
    for k in 0..3u32 {
        let w = fx.mem.read_u32(sp + 0x50 + 4 * k);
        fx.mem.write_u32(p + 0x1210 + 4 * k, w);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32, v: &Variant) -> i32 {
    fx.bzero(D, 0x1430);
    fx.xform_init(D);
    fx.xform_init(D + 0x70);
    fx.xform_init(D + 0x1140);
    let h = fx.self_handle();
    let r = fx.request_node(h, v.nodes[0], 1);
    fx.mem.set32(D + 0x141C, r);
    let h = fx.self_handle();
    let r = fx.request_node(h, v.nodes[1], 0);
    fx.mem.set32(D + 0x1420, r);
    let h = fx.self_handle();
    let r = fx.request_node(h, v.nodes[2], 0);
    fx.mem.set32(D + 0x1424, r);
    fx.mem.set32(D + 0x1410, v.length);
    fx.mem.set32(D + 0x1414, 1);
    fx.mem.set32(D + 0x1418, param);
    fx.texture5(sp + 0x80, 0);
    // The burst (+0x11B0): 32 sparks, size 20.
    let g = D + 0x11B0;
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, v.burst_flags);
    colour(fx, g + 8, v.burst_colour);
    fx.mem.write_u32(g + 0xC, 0x41A0_0000);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    for (k, x) in [(0x30u32, 0), (0x32, 0), (0x34, 0x20), (0x36, 0x20), (0x38, 0), (0x3A, 0), (0x3C, 0x80), (0x3E, 0x40)] {
        fx.mem.set16(g + k, x);
    }
    fx.mem.write_u32(g + 0x40, D + 0x1210);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4170_0000);
    fx.mem.write_u32(g + 0x50, 0x4120_0000);
    fx.mem.set16(g + 0x5A, 0x20);
    fx.mem.set16(g + 0x5C, 8);
    fx.mem.set16(g + 0x5E, 0x10);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x120A) {
        let p = D + 16 * k as u32;
        spark(fx, sp, p);
        let r = fx.rand();
        let (_, h) = fx.div(r, 12);
        fx.mem.set16(p + 0x121E, -h);
        fx.mem.write_u8(p + 0x121C, 0x80);
        let r = fx.rand();
        fx.mem.write_u8(p + 0x121D, (r % 8) as u8);
        k += 1;
    }
    fx.texture5(sp + 0x80, 1);
    // The fountain (+0xE0): 128 sprites, size 5.
    let g = D + 0xE0;
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0xF800_0003);
    colour(fx, g + 8, v.fountain_colour);
    fx.mem.write_u32(g + 0xC, 0x40A0_0000);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    for (k, x) in [(0x30u32, 0), (0x32, 0), (0x34, 0x40), (0x36, 0x40), (0x38, 0), (0x3A, 0), (0x3C, 0x100), (0x3E, 0x40)] {
        fx.mem.set16(g + k, x);
    }
    fx.mem.write_u32(g + 0x40, D + 0x140);
    fx.mem.write_u32(g + 0x44, D + 0x940);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4040_0000);
    fx.mem.write_u32(g + 0x50, 0x3F00_0000);
    fx.mem.set16(g + 0x5A, 0x80);
    fx.mem.set16(g + 0x5C, 4);
    fx.mem.set16(g + 0x5E, 0);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x13A) {
        fountain_spawn(fx, D + 16 * k as u32, false);
        k += 1;
    }
    0
}

fn step(fx: &mut Fx, sp: u32, v: &Variant) -> i32 {
    let h = fx.self_handle();
    let s = fx.mem.i32(D + 0x141C);
    fx.node_position(h, s, sp + 0x60);
    let h = fx.self_handle();
    let s = fx.mem.i32(D + 0x1420);
    fx.node_position(h, s, sp + 0x70);
    let h = fx.self_handle();
    let s = fx.mem.i32(D + 0x1420);
    fx.node_position(h, s, D + 0x90);
    fx.look_angles(sp + 0x60, sp + 0x70, D + 0x80);
    if fx.mem.i32(D + 0x1414) >= v.switch {
        let t = fx.target_handle();
        fx.unit_point(1, t, D + 0x1160);
    } else {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + 0x1424);
        fx.node_position(h, s, D + 0x1160);
    }
    // s0 = D, s1 = the OT, s2 = sp + 0x70; s3 the caller's.
    let regs = [Some(D), Some(OT), Some(sp + 0x70), None];
    fx.xform_world_saving(D, 0x01E0_1E08, regs);
    fx.xform_world_saving(D + 0x70, 0x01E0_1E10, regs);
    fx.xform_world_saving(D + 0x1140, 0x01E0_1E18, regs);
    if fx.mem.i32(D + 0x1414) == v.switch {
        fx.mem.write_u8(D + 0x11BB, 0x80);
        if v.switch_clears {
            let f = fx.mem.read_u32(D + 0x11B4) & !0x1000;
            fx.mem.write_u32(D + 0x11B4, f);
        }
    }
    fx.sprites(D + 0x11B0, D + 0x1140);
    let n = fx.mem.i32(D + 0x1414);
    if n < v.fade_in {
        fx.step_up(D + 0x11BB, 0x80, 4);
    } else if n < v.switch {
        fx.step_down(D + 0x11BB, 0, 4);
    } else if n < v.fade_out {
        fx.step_up(D + 0x11BB, 0x80, 4);
    } else {
        fx.step_down(D + 0x11BB, 0, 8);
    }
    let mut k = 0;
    while k < fx.mem.i16(D + 0x120A) {
        let p = D + 16 * k as u32;
        if fx.mem.i16(p + 0x121E) >= 0 {
            let f = f32::from_bits(0x3F83_D70A);
            for c in 0..3u32 {
                let x = ee::mul(fx.mem.f(p + 0x1210 + 4 * c), f);
                fx.mem.setf(p + 0x1210 + 4 * c, x);
            }
            fx.step_down(p + 0x121C, 0, 8);
        }
        if fx.mem.i16(p + 0x121E) >= 0x10 {
            spark(fx, sp, p);
            fx.mem.set16(p + 0x121E, 0);
            fx.mem.write_u8(p + 0x121C, 0x80);
        }
        k += 1;
    }
    fx.sprites(D + 0xE0, D + 0x70);
    if fx.mem.i32(D + 0x1414) >= v.fountain_out {
        fx.step_down(D + 0xEB, 0, 8);
    } else {
        fx.step_up(D + 0xEB, 0x80, 8);
    }
    let top = f32::from_bits(0x4334_0000);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x13A) {
        let p = D + 16 * k as u32;
        if fx.mem.i16(p + 0x14E) >= 0 {
            if ee_lt(fx.mem.f(p + 0x144), top) {
                let (_, m) = fx.div(k, 3);
                fx.mem.set16(p + 0x14E, 0);
                let rise = match m {
                    0 => 8.0,
                    1 => 9.0,
                    _ => 10.0,
                };
                let y = ee::add(fx.mem.f(p + 0x144), rise);
                fx.mem.setf(p + 0x144, y);
                let f = f32::from_bits(0x3F8F_5C29);
                let z = ee::mul(fx.mem.f(p + 0x148), f);
                fx.mem.setf(p + 0x148, z);
                let x = ee::mul(fx.mem.f(p + 0x140), f);
                fx.mem.setf(p + 0x140, x);
            }
            if fx.mem.i32(D + 0x1414) >= v.fade_in {
                fx.step_up(p + 0x14C, 0x80, 0x20);
            }
            if !ee_lt(fx.mem.f(p + 0x144), top) && fx.mem.i16(p + 0x14E) >= 5 {
                fountain_spawn(fx, p, true);
            }
        }
        k += 1;
    }
    let n = fx.mem.i32(D + 0x1414) + 1;
    fx.mem.set32(D + 0x1414, n);
    let left = fx.mem.i32(D + 0x1410) - 1;
    fx.mem.set32(D + 0x1410, left);
    if n == v.ret2 {
        2
    } else if n == v.ret1 {
        1
    } else if n >= v.ret6 && n < v.ret2 {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
