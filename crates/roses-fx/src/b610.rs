//! BMODULE b610 (card 357), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01520, data area 0x1ED0000) [F] (helper C, 30 Oct
//! 2026): b262's chain program at a smaller scale (8 segments, jitter 80,
//! laid from update 100 to the target), a flickering disc / quad burst at
//! the attacker's node 0x11 and the screen tint from update 62, the frame
//! quad (the frame drawn, tinted) on updates 62..99, the stage light down
//! to 0x20 until update 220. Returns 1 on update 78, 6 on 88..219, 2 on
//! 220, 8 after 240.
//!
//! Library: `disc` (0x1E003B0), `quads_setup` / `quads_draw` (0x1E00910
//! / 0x1E00A70) [`crate::fxlib`], the tint (0x1E00250) and the chain
//! (0x1E01080) of [`crate::b262`], the frame quad (0x1E00120,
//! [`crate::framegs`]).
use crate::b262::{chain, tint};
use crate::bus::Bus;
use crate::exe::Fx;

const D: u32 = crate::bmodule::DATA_AREA;

/// b610 / b438's differences (b438: `crate::b438`).
pub struct Variant {
    /// The quad corner template (rodata).
    pub template: u32,
    /// The burst's / chain's colour (B, G, R) and the chain quads' alpha.
    pub rgb: [u8; 3],
    pub alpha30: u8,
    /// The light goes back up from here; the return codes.
    pub light_up: i32,
    pub ret1: i32,
    pub ret6: i32,
    /// Updates.
    pub length: i32,
    /// The node requests (the burst / chain start; the second unused).
    pub nodes: [i32; 2],
    /// The frame quad's span and its fade start.
    pub frame: (i32, i32, i32),
    /// The burst / chain phases: flicker from, chain from.
    pub flicker: i32,
    pub chain: i32,
    /// The chain starts between the two nodes (0x1036E0) instead of at
    /// the first.
    pub midpoint: bool,
    /// The disc's alpha (+0x5F7) and the burst's (+0x19B) at the start;
    /// `fade_in`: both rise (by 0x10 to 0xFF / by 2 to 0x20) while it
    /// flickers.
    pub disc_alpha: u8,
    pub burst_alpha: u8,
    pub fade_in: bool,
    /// The chain's flags (+4) and hold (+0x34); the disc radius cycles 8 /
    /// 6 / 4 while the chain runs.
    pub chain_flags: u32,
    pub chain_hold: i32,
    pub radius_cycle: bool,
    /// The return addresses of the transform calls (0x1038D0, 0x1039A0 x
    /// 3) [F].
    pub xform_ra: [u32; 4],
}

pub const B610: Variant = Variant {
    template: 0x01E0_1B80,
    rgb: [0x70, 0xA0, 0xD0],
    alpha30: 0x30,
    light_up: 0xDC,
    ret1: 0x4E,
    ret6: 0x58,
    length: 0xF0,
    nodes: [0x11, 0x10],
    frame: (0x3E, 0x64, 0x5E),
    flicker: 0x3E,
    chain: 0x64,
    midpoint: false,
    disc_alpha: 0xFF,
    burst_alpha: 0x20,
    fade_in: false,
    chain_flags: 0x100_0000,
    chain_hold: 6,
    radius_cycle: true,
    xform_ra: [0x01E0_1864, 0x01E0_186C, 0x01E0_1874, 0x01E0_1884],
};

/// The entry 0x1E01520.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B610)
}

pub fn run(fx: &mut Fx, param: i32, v: &Variant) -> i32 {
    let sp = fx.enter(0xA0);
    let ret = if param >= 0 { init(fx, sp, param, v) } else { step(fx, sp, v) };
    fx.leave(0xA0);
    ret
}

fn colour(fx: &mut Fx, at: u32, rgb: [u8; 3], a: u8) {
    fx.mem.write_u8(at + 2, rgb[2]);
    fx.mem.write_u8(at + 1, rgb[1]);
    fx.mem.write_u8(at, rgb[0]);
    fx.mem.write_u8(at + 3, a);
}

fn disc_record(fx: &mut Fx, at: u32, flags: u32, rgb: [u8; 3], radius: u32, alpha: u8) {
    fx.mem.write_u32(at, flags);
    colour(fx, at + 4, rgb, alpha);
    fx.mem.write_u32(at + 8, radius);
    fx.mem.set16(at + 0xC, 0x24);
    fx.mem.set16(at + 0xE, 0);
}

fn init(fx: &mut Fx, sp: u32, param: i32, v: &Variant) -> i32 {
    fx.bzero(D, 0x2DA0);
    fx.xform_init(D);
    fx.xform_init(D + 0xB0);
    fx.xform_init(D + 0x120);
    for k in 0..8u32 {
        fx.xform_init(D + 0x600 + 0x70 * k);
    }
    let h = fx.self_handle();
    let r = fx.request_node(h, v.nodes[0], 1);
    fx.mem.set32(D + 0x2D98, r);
    let h = fx.self_handle();
    let r = fx.request_node(h, v.nodes[1], 0);
    fx.mem.set32(D + 0x2D9C, r);
    fx.mem.set32(D + 0x2D88, v.length);
    fx.mem.set32(D + 0x2D8C, 1);
    fx.mem.set32(D + 0x2D90, param);
    fx.mem.set32(D + 0x2D94, 0x80);
    colour(fx, D + 0x2D80, [0x80; 3], 0x40);
    colour(fx, D + 0x2D84, [0x80; 3], 0x60);
    disc_record(fx, D + 0x5F0, 0x8_0000, v.rgb, 0x4100_0000, v.disc_alpha);
    for k in 0..8u32 {
        disc_record(fx, D + 0x2C80 + 16 * k, 0x8_0000, v.rgb, 0x4180_0000, 0xFF);
        disc_record(fx, D + 0x2D00 + 16 * k, 0x1008_0000, v.rgb, 0x4040_0000, 0xFF);
    }
    fx.texture4(sp + 0x90, 0xF);
    // The burst (+0x190): FST cell (142, 0, 34, 64).
    let g = D + 0x190;
    fx.mem.write_u32(g + 4, 1);
    for (k, x) in [(0x10u32, 0x8E), (0x12, 0), (0x14, 0x22), (0x16, 0x40)] {
        fx.mem.set16(g + k, x);
    }
    colour(fx, g + 8, v.rgb, v.burst_alpha);
    let tex = fx.mem.read_u32(sp + 0x90);
    fx.quads_setup(g, tex, f32::from_bits(0x3F33_3333), v.template);
    for k in 0..8u32 {
        let g = D + 0x980 + 0x460 * k;
        fx.mem.write_u32(g + 4, 0x8_0001);
        for (o, x) in [(0x10u32, 0x8E), (0x12, 0), (0x14, 0x22), (0x16, 0x40)] {
            fx.mem.set16(g + o, x);
        }
        colour(fx, g + 8, [0x80; 3], v.alpha30);
        let tex = fx.mem.read_u32(sp + 0x90);
        fx.quads_setup(g, tex, f32::from_bits(0x3F4C_CCCD), v.template);
    }
    // The chain (+0x70): 8 segments, jitter 80, hold 6.
    let g = D + 0x70;
    let tex = fx.mem.read_u32(sp + 0x90);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, v.chain_flags);
    for (k, x) in [(0x10u32, 0xA), (0x12, 0), (0x14, 0x2A), (0x16, 0x80)] {
        fx.mem.set16(g + k, x);
    }
    colour(fx, g + 8, [0x80; 3], 0x20);
    fx.mem.set16(g + 0xC, 8);
    fx.mem.set16(g + 0x30, 0);
    fx.mem.set16(g + 0x32, 1);
    fx.mem.set16(g + 0x34, v.chain_hold);
    fx.mem.set16(g + 0xE, 0x50);
    fx.mem.write_u32(g + 0x20, D + 0x600);
    fx.mem.write_u32(g + 0x24, D + 0x980);
    fx.mem.write_u32(g + 0x28, D + 0x2C80);
    fx.mem.write_u32(g + 0x2C, D + 0x2D00);
    let a = fx.mem.read_u8(D + 0x98B);
    fx.mem.write_u8(g + 0x36, a);
    let a = fx.mem.read_u8(D + 0x2C87);
    fx.mem.write_u8(g + 0x37, a);
    let a = fx.mem.read_u8(D + 0x2D07);
    fx.mem.write_u8(g + 0x38, a);
    0
}

fn step(fx: &mut Fx, sp: u32, v: &Variant) -> i32 {
    if v.midpoint {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + 0x2D98);
        fx.node_position(h, s, sp + 0x60);
        let h = fx.self_handle();
        let s = fx.mem.i32(D + 0x2D9C);
        fx.node_position(h, s, sp + 0x70);
        fx.midpoint(sp + 0x60, sp + 0x70, D + 0x140);
    } else {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + 0x2D98);
        fx.node_position(h, s, D + 0x140);
    }
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0x80);
    // s0 = D, s1 = the OT, s2 = sp + 0x80 (the frame quad's TEX1 slot
    // overlays the last billboard's TRS matrix).
    let regs = [Some(D), Some(crate::exe::OT), Some(sp + 0x80), None];
    fx.xform_world_saving(D, v.xform_ra[0], regs);
    fx.xform_billboard_saving(D + 0xB0, v.xform_ra[1], regs);
    fx.xform_billboard_saving(D + 0x120, v.xform_ra[2], regs);
    for k in 0..8u32 {
        fx.xform_billboard_saving(D + 0x600 + 0x70 * k, v.xform_ra[3], [regs[0], regs[1], regs[2], Some(D + 0x70 * k)]);
    }
    if fx.mem.i32(D + 0x2D8C) < v.light_up {
        fx.step_down(D + 0x2D94, 0x20, 4);
    } else {
        fx.step_up(D + 0x2D94, 0x80, 4);
    }
    let light = fx.mem.u8(D + 0x2D94);
    fx.set_stage_light(light);
    let n = fx.mem.i32(D + 0x2D8C);
    if n >= v.frame.0 && n < v.frame.1 {
        fx.frame_quad(D + 0x2D84);
        if fx.mem.i32(D + 0x2D8C) >= v.frame.2 {
            fx.step_down(D + 0x2D87, 0, 6);
        }
    }
    if fx.mem.i32(D + 0x2D8C) >= v.flicker {
        for c in [2u32, 1, 0] {
            let x = (fx.rand() % 128) as u8;
            fx.mem.write_u8(D + 0x198 + c, x);
            fx.mem.write_u8(D + 0x5F4 + c, x);
            fx.mem.write_u8(D + 0x2D80 + c, x);
        }
        tint(fx, D + 0x2D80);
        fx.disc(D + 0x5F0, D + 0x120);
        if v.fade_in {
            fx.step_up(D + 0x5F7, 0xFF, 0x10);
        }
        fx.quads_draw(D + 0x190, D + 0x120);
        if v.fade_in {
            fx.step_up(D + 0x19B, 0x20, 2);
        }
        for k in 0..64u32 {
            for c in 0..3u32 {
                let a = D + 0x1F0 + 16 * k + 4 * c;
                let x = fx.mem.i32(a).wrapping_add(0x80);
                fx.mem.set32(a, x);
            }
        }
    }
    let n = fx.mem.i32(D + 0x2D8C);
    if n >= v.chain {
        if v.radius_cycle {
            let (_, m) = fx.div(n, 3);
            let r = match m {
                0 => 0x4100_0000,
                1 => 0x40C0_0000,
                _ => 0x4080_0000,
            };
            fx.mem.write_u32(D + 0x5F8, r);
        }
        for k in 0..8u32 {
            for c in [2u32, 1, 0] {
                let x = fx.mem.read_u8(D + 0x198 + c);
                fx.mem.write_u8(D + 0x2D04 + c + 16 * k, x);
                fx.mem.write_u8(D + 0x2C84 + c + 16 * k, x);
                fx.mem.write_u8(D + 0x988 + c + 0x460 * k, x);
            }
        }
        chain(fx, D + 0x70, D + 0x140, sp + 0x80);
    }
    let n = fx.mem.i32(D + 0x2D8C) + 1;
    fx.mem.set32(D + 0x2D8C, n);
    let left = fx.mem.i32(D + 0x2D88) - 1;
    fx.mem.set32(D + 0x2D88, left);
    if n == v.light_up {
        2
    } else if n == v.ret1 {
        1
    } else if n >= v.ret6 && n < v.light_up {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
