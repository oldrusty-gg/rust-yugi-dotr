//! BMODULE b090 (card 233), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05970, data area a0 = 0x1ED0000) [F] (helper B3, 30
//! Oct 2026): b245's family. Node bursts (+0xA4, two specs, motes +0xE8
//! from the attacker's nodes 0x42 / 0x3A / 0x4A / 0x52) every update; on
//! updates 2..18 two sparks a update (bouncing sprites, +0x1D8C) at the
//! nodes 0x42 / 0x3A; on update 65 an arc pair (+0x17E8, nodes 0x25 /
//! 0x23); on 68 at the target a fade (+0x52AC), a flash (+0x1AC8), three
//! ribbons (+0x1C54 + 0x68 k: one wide, two thin turned 0xB000 / 0x3000),
//! 200 bouncing sprites and a camera shake (+0x40); the stage light pulsed
//! by 0x1031C0 (+0xA0: down on update 2, up on 114). Returns 1 on update
//! 58, 2 on 65, 6 on 68..88, 8 on 120 (and at once for a parameter of -2 /
//! -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `arcs` 0x1E015F0, `flash`
//! 0x1E02870, `bars` 0x1E03420 (+0x1B14, never started), `ribbon`
//! 0x1E03E30, `bounce` 0x1E049C0, `fade` 0x1E05430 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, rand_bytes, shake_saving};
use crate::b245::{bounce_spawn, Bounce};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E05970 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0x80);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x53C0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    for k in 0..2 {
        fx.texture(5, k);
    }
    for (at, node, clear) in [(0xCu32, 0x42, 1), (0x10, 0x3A, 0), (0x14, 0x4A, 0), (0x18, 0x52, 0), (0x1C, 0x25, 0), (0x20, 0x23, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA4): specs 0x1E06670 / 0x1E06668 and 0x1E06680
    // / 0x1E06678, motes +0xE8 / +0xB68.
    for k in 0..4 {
        let b = fx.mem.read_u8(D + 0xC + 4 * k);
        fx.mem.write_u8(D + 0xCC + k, b);
    }
    fx.mem.write_u32(D + 0xAC, 0x01E0_6670);
    fx.mem.write_u32(D + 0xB0, 0x01E0_6668);
    fx.mem.write_u32(D + 0xB4, 0x01E0_6680);
    fx.mem.write_u32(D + 0xB8, 0x01E0_6678);
    fx.mem.write_u32(D + 0xA4, D + 0xE8);
    fx.mem.write_u8(D + 0xD4, 0x30);
    fx.mem.write_u32(D + 0xA8, D + 0xB68);
    fx.mem.write_u8(D + 0xD5, 0x50);
    fx.mem.write_u8(D + 0xD6, 0);
    fx.mem.set16(D + 0xD8, 0);
    fx.mem.write_u32(D + 0xDC, 0x4334_0000);
    fx.mem.write_u32(D + 0xE4, 0x4100_0000);
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x72);
    // The entry's s0 = D, s1 = the OT, s2 = 2 (the texture loop's).
    rand_bytes(fx, D + 0x52B4, 0x100, 0x01E0_5B74, [Some(D), Some(OT), Some(2)]);
    0
}

/// Update 65 [F 0x1E05BCC]: the arc pair (+0x17E8), points +0x1848.
fn arcs_start(fx: &mut Fx) {
    let g = D + 0x17E8;
    set_counter4(fx, g, 0x10, 0, 0xFF);
    fx.mem.write_u8(g + 4, 1);
    fx.mem.write_u32(g + 8, D + 0x1848);
    fx.mem.write_u8(g + 0x30, 0x28);
    fx.mem.write_u8(g + 0x31, 8);
    let b = fx.mem.read_u8(D + 0x1C);
    fx.mem.write_u8(g + 0x32, b);
    let b = fx.mem.read_u8(D + 0x20);
    fx.mem.write_u8(g + 0x33, b);
    fx.mem.write_u32(g + 0x38, 0x3F99_999A);
    fx.mem.write_u8(g + 0x34, 0);
    fx.mem.set16(g + 0x36, 0);
    for (o, v) in [(0x47u32, 0x80u8), (0x44, 0x80), (0x45, 0x80), (0x46, 0x80), (0x4A, 0x10), (0x49, 0x10), (0x48, 0xC0)] {
        fx.mem.write_u8(g + o, v);
    }
    let mut a = 0u32;
    while a < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0x1E + 2 * a, -1);
        fx.mem.set16(g + 0xC + 2 * a, -1);
        a += 1;
    }
}

/// Update 68 [F 0x1E05C94]: at the target (sp + 0x60).
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x60;
    set_counter4(fx, D + 0x52AC, 8, 0, 0xFF);
    fx.mem.write_u8(D + 0x52B0, 1);
    let f = D + 0x1AC8;
    set_counter4(fx, f, 0x20, 0, 0xFF);
    fx.mem.write_u8(f + 5, 0);
    fx.mem.write_u8(f + 4, 0x81);
    fx.mem.write_u8(f + 7, 9);
    fx.mem.write_u32(f + 0x10, 0x447A_0000);
    fx.mem.write_u32(f + 0x14, 0);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x20), (8, 0xA0), (0xF, 0xD0)] {
        fx.mem.write_u8(f + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(f + o, 0);
    }
    copy_v4(fx, at, f + 0x1C);
    for k in 0..3u32 {
        let r = D + 0x68 * k + 0x1C54;
        fx.mem.write_u8(r + 4, 4);
        if k == 0 {
            set_counter4(fx, r, 0x1C, 0, 0xFF);
            let b = fx.mem.read_u8(r + 4) | 1;
            fx.mem.write_u8(r + 4, b);
            fx.mem.write_u32(r + 0x14, 0x4000_0000);
            fx.mem.write_u32(r + 0x1C, 0x4220_0000);
            fx.mem.write_u8(r + 0x23, 0);
            fx.mem.write_u8(r + 0x27, 0x80);
            fx.mem.write_u8(r + 8, 0xA0);
            fx.mem.write_u32(r + 0x58, 0x40);
            fx.mem.write_u32(r + 0x5C, 0x80);
            fx.mem.write_u32(r + 0x60, 0x100);
        } else {
            set_counter4(fx, r, 0x20, 0, 0xFF);
            let b = fx.mem.read_u8(r + 4) | 2;
            fx.mem.write_u8(r + 4, b);
            fx.mem.write_u32(r + 0x14, 0x3FC0_0000);
            fx.mem.write_u32(r + 0x1C, 0x42F0_0000);
            fx.mem.write_u32(r + 0x18, 0x3ECC_CCCD);
            fx.mem.write_u8(r + 0x23, 0x80);
            fx.mem.write_u8(r + 0x27, 0x40);
            fx.mem.write_u8(r + 8, 0x19);
            fx.mem.write_u32(r + 0x50, if k & 1 != 0 { 0x3000 } else { 0xB000 });
        }
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 0xA, 1);
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xC, 3);
        let x = fx.rand_from(D, 0x01E0_5EB8);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, D + 0x52B4);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, at, r + 0x28);
    }
    let v = Bounce { count: 0x18, b8: 1, base: 100.0, vy: 0xBF80_0000, spread: (0xB4, 0x5A), s0: D, ras: [0x01E0_5F50, 0x01E0_5FBC, 0x01E0_604C, 0x01E0_6098, 0x01E0_60F4] };
    bounce_spawn(fx, D + 0x1D8C, at, &v);
    let s = D + 0x40;
    set_counter4(fx, s, 0x20, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x3FC0_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, D + 0x60);
}

/// (rand >> 4) % 10 - 5, over 100 [F 0x1E062F0].
fn jitter(fx: &mut Fx, ra: u32) -> f32 {
    let x = fx.rand_from(D, ra) >> 4;
    let (_, h) = fx.div(x, 0xA);
    ee::div(ee::from_i32(h - 5), 100.0)
}

/// Updates 2..18 [F 0x1E061D4]: a spark at each of the nodes +0xC / +0x10.
fn sparks(fx: &mut Fx, sp: u32) {
    for k in 0..2u32 {
        let i = first_free(fx, D + 0x1D8C, 0x44, 0xC8);
        if i < 0 {
            continue;
        }
        let b = D + 0x44 * i as u32 + 0x1D8C;
        set_counter4(fx, b, 0x18, 0, 0xFF);
        let x = fx.rand_from(D, 0x01E0_624C);
        fx.mem.set8(b + 4, ((x >> 4) & 3) << 4);
        fx.mem.write_u8(b + 8, 0);
        fx.mem.write_u8(b + 0xA, 0);
        fx.mem.write_u8(b + 0xB, 1);
        let x = fx.rand_from(D, 0x01E0_6278);
        fx.mem.set8(b + 9, (x >> 4) & 3);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x40)] {
            fx.mem.write_u8(b + o, v);
        }
        fx.mem.write_u32(b + 0x10, 0x41F0_0000);
        fx.mem.write_u32(b + 0x14, 0x40C0_0000);
        fx.mem.write_u32(b + 0x1C, 0);
        fx.mem.write_u32(b + 0x18, 0);
        let h = fx.mem.i32(D + 4);
        let slot = fx.mem.i32(D + 0xC + 4 * k);
        fx.node_position(h, slot, sp + 0x70);
        for c in 0..3 {
            let w = fx.mem.read_u32(sp + 0x70 + 4 * c);
            fx.mem.write_u32(b + 0x20 + 4 * c, w);
        }
        for (o, ra) in [(0x2Cu32, 0x01E0_62F8u32), (0x30, 0x01E0_6344), (0x34, 0x01E0_6390)] {
            let v = jitter(fx, ra);
            fx.mem.setf(b + o, v);
        }
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, sp + 0x60);
    match fx.mem.u16(D) {
        0x78 => return 8,
        0x44 => strike(fx, sp),
        0x41 => arcs_start(fx),
        _ => {}
    }
    if (2..0x13).contains(&fx.mem.u16(D)) {
        sparks(fx, sp);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_63F4, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_6404, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::fade::fade(fx, D + 0x52AC, OT);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA4, n);
    blib::mote::motes(fx, D + 0xE8, OT, 0x20);
    blib::arc::arcs(fx, 0x01E0_15F0, D + 0x17E8, OT, 1);
    blib::star::flash(fx, 0x01E0_2870, D + 0x1AC8, OT, 1);
    blib::cross::bars(fx, 0x01E0_3420, D + 0x1B14, OT, 4);
    blib::streak::ribbon(fx, 0x01E0_3E30, D + 0x1C54, OT, 3);
    blib::sprite::bounce(fx, D + 0x1D8C, OT, 0xC8);
    let n = fx.mem.u16(D);
    if (0x44..0x59).contains(&n) {
        6
    } else if n == 0x41 {
        2
    } else if n == 0x3A {
        1
    } else {
        0
    }
}
