//! BMODULE b268 (card 330), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04870, data area a0 = 0x1ED0000, 0x99C0 bytes)
//! [F]: b164's family on its own unscheduled
//! program. s6 = D, s0 = the parameter, a 0x160 frame (the target sp +
//! 0xE0). Update 2 four bolt records (+0x6C30) between node pairs; 0x26
//! a long flash (record 5 of +0x5550) and a ribbon (+0x6000) midway
//! between nodes 0x3F / 0x46 and a shake there; 0x32 four segment
//! spawners (+0x91B0) on nodes 0x3F / 0x46, two slow sprites (+0x518) on
//! them and a long shake; 0xB4 the sprites sent to the target with a
//! spawner each and 60 sprites (+0x5A0) per node; 0xE4 the target untinted,
//! its jitter (+0xEC, `radiate::jitter`), a window sprite (+0xDC), a
//! spawner on the target, the fade and a shake; 0x11A the target tinted
//! back. Updates 0x32..0x3B / 0xB4..0xE3 sixteen sprites (+0x5A0) about
//! the two slow sprites; 0xF4..0x116 twice three bolts and a flash about
//! the target. Returns 6 on 0xE4..0x116, 2 on 0x117, 1 on 0xDA, 8 on
//! 0x136 (no drawing then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `fade`
//! 0x1E00410, `flash` 0x1E00950, `bounce` 0x1E01500, `bolts` 0x1E01F70,
//! `segment_spawns` 0x1E02AC0, `ribbon` 0x1E02E40, `frame_fade` 0x1E039D0
//! (+0xAC, never set), `jitter` 0x1E04040 [`crate::blib`]; the window
//! sprite 0x1E03EB0 is b398's (b186b3c26dc3) at this record's addresses.
use crate::b076::{node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::b035::light_ramp;
use crate::b586::segment_spawns_saving;
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SHAKE: u32 = D + 0x40;
const FADE: u32 = D + 0xA4;
const WINDOW: u32 = D + 0xDC;
const JITTER: u32 = D + 0xEC;
const SLOW: u32 = D + 0x518;
const SPRITES: u32 = D + 0x5A0;
const FLASH: u32 = D + 0x5550;
const RIBBON: u32 = D + 0x6000;
const BOLTS: u32 = D + 0x6C30;
const SPAWNS: u32 = D + 0x91B0;
const BYTES: u32 = D + 0x98B8;

/// The entry 0x1E04870 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x160);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x160);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x99C0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    for k in 0..4 {
        fx.texture(5, k);
    }
    for (at, node, clear) in [(0xCu32, 0x3F, 1), (0x10, 0x3C, 0), (0x14, 0x3B, 0), (0x18, 0x46, 0), (0x1C, 0x43, 0), (0x20, 0x42, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x130);
    // The entry's s0 = the parameter, s1 = 4 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_4A08, [Some(param as u32), Some(4), None]);
    0
}

/// The registers the program's later calls save (s0 / s1).
struct Regs {
    s0: u64,
    s1: Option<u64>,
}

fn rand4(fx: &mut Fx, r: &Regs, ra: u32) -> i32 {
    fx.rand_from(r.s0 as u32, ra) >> 4
}

/// (rand >> 4) % m.
fn rand_rem(fx: &mut Fx, r: &Regs, ra: u32, m: i32) -> i32 {
    let v = rand4(fx, r, ra);
    fx.div(v, m).1
}

fn node_pos(fx: &mut Fx, r: &Regs, slot: i32, out: u32, ra: u32) {
    let h = fx.mem.i32(D + 4);
    node_position_saving(fx, h, slot, out, r.s0, r.s1, ra);
}

fn bytes(fx: &mut Fx, p: u32, at: &[(u32, u8)]) {
    for &(o, v) in at {
        fx.mem.write_u8(p + o, v);
    }
}

fn copy3(fx: &mut Fx, from: u32, to: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn copy4(fx: &mut Fx, from: u32, to: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// Update 2 [F 0x1E04A90]: four bolt records between node pairs
/// (0x3B -> 0x3C, 0x42 -> 0x43 (0x1A), 0x3C -> 0x3F, 0x43 -> 0x46 (0x18,
/// delay 0x14)).
fn opening(fx: &mut Fx, r: &mut Regs) {
    for s1 in 0..4u32 {
        let b = BOLTS + (s1 << 6);
        let (count, delay, a, c, k8) = match s1 {
            0 => (0x1A, 0, 0x14u32, 0x10u32, 5u8),
            1 => (0x1A, 0, 0x20, 0x1C, 5),
            2 => (0x18, 0x14, 0x10, 0xC, 8),
            _ => (0x18, 0x14, 0x1C, 0x18, 8),
        };
        set_counter4(fx, b, count, delay, 0xFF);
        let v = fx.mem.read_u8(D + a);
        fx.mem.write_u8(b + 9, v);
        let v = fx.mem.read_u8(D + c);
        fx.mem.write_u8(b + 0xA, v);
        fx.mem.write_u8(b + 8, k8);
        fx.mem.write_u8(b + 5, 1);
        fx.mem.write_u8(b + 4, 0x81);
        fx.mem.write_u32(b + 0x10, 0x3F99_999A);
        fx.mem.write_u8(b + 0xB, 0);
        fx.mem.write_u8(b + 0xD, 3);
        fx.mem.write_u32(b + 0x14, 0x3E99_999A);
        fx.mem.write_u8(b + 0xC, 0);
        bytes(fx, b, &[(0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x20)]);
        r.s0 = u64::from(s1 << 6);
    }
    r.s1 = Some(4);
}

/// Update 0x26 [F 0x1E04C4C]: flash record 5, a ribbon and a shake midway
/// between nodes 0x3F / 0x46.
fn midway(fx: &mut Fx, sp: u32, r: &mut Regs) {
    let f = FLASH + 5 * 0x4C;
    set_counter4(fx, f, 0x14, 0, 0xFF);
    bytes(fx, f, &[(5, 0), (4, 0x81), (7, 9)]);
    fx.mem.write_u32(f + 0x10, 0x44E1_0000);
    fx.mem.write_u32(f + 0x14, 0);
    bytes(fx, f, &[(0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0xA0), (9, 0x20), (8, 0x80)]);
    let s = fx.mem.i32(D + 0xC);
    node_pos(fx, r, s, sp + 0xC0, 0x01E0_4CC4);
    r.s0 = u64::from(sp + 0xD0);
    let s = fx.mem.i32(D + 0x18);
    node_pos(fx, r, s, sp + 0xD0, 0x01E0_4CDC);
    for k in 0..3u32 {
        let (a, b) = (fx.mem.f(sp + 0xC0 + 4 * k), fx.mem.f(sp + 0xD0 + 4 * k));
        let v = ee::add(a, ee::div(ee::sub(b, a), 2.0));
        fx.mem.setf(f + 0x1C + 4 * k, v);
    }
    let g = RIBBON;
    set_counter4(fx, g, 0x1A, 0, 0xFF);
    let c = fx.mem.read_u8(g).wrapping_sub(8);
    fx.mem.write_u8(g, c);
    fx.mem.write_u8(g + 5, 0);
    copy4(fx, f + 0x1C, g + 0x28);
    bytes(fx, g, &[(4, 0x85), (0xA, 3), (0xC, 3)]);
    fx.mem.write_u32(g + 0x14, 0x4060_0000);
    fx.mem.write_u32(g + 0x1C, 0x4220_0000);
    bytes(fx, g, &[(0x23, 0), (0x27, 0x60), (8, 0x37)]);
    fx.mem.write_u32(g + 0x58, 0);
    fx.mem.write_u32(g + 0x5C, 0);
    let v = if fx.mem.read_u32(D + 0x30) == 0x8000 { 0x300 } else { -0x300 };
    fx.mem.set32(g + 0x60, v);
    fx.mem.write_u8(g + 0xB, 2);
    let x = rand4(fx, r, 0x01E0_4DF0);
    fx.mem.set8(g + 9, x);
    fx.mem.write_u32(g + 0x10, BYTES);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(g + o, 0x80);
    }
    let s = SHAKE;
    set_counter4(fx, s, 0x14, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x4000_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy4(fx, f + 0x1C, s + 0x20);
}

/// A segment spawner (0x101C30 over +0x91B0, the index unchecked) counted
/// `count` (c 0) into the bolt pool: +4 `k4`, +5 `on_node`, +0xD `k`,
/// +0x10 / +0x11, +0x14 the jitter range.
fn spawner(fx: &mut Fx, count: i32, k4: u8, on_node: u8, k: [u8; 3], range: u32) -> u32 {
    let i = first_free(fx, SPAWNS, 0x3C, 0x1E);
    let g = SPAWNS.wrapping_add(i.wrapping_mul(0x3C) as u32);
    set_counter4(fx, g, count, 0, 0);
    fx.mem.write_u8(g + 5, on_node);
    fx.mem.write_u8(g + 4, k4);
    fx.mem.write_u32(g + 8, BOLTS);
    bytes(fx, g, &[(0xC, 0x96), (0xD, k[0]), (0xE, 0), (0xF, 0), (0x10, k[1]), (0x11, k[2])]);
    fx.mem.write_u32(g + 0x14, range);
    fx.mem.write_u32(g + 0x18, 0x3F80_0000);
    fx.mem.write_u32(g + 0x1C, 0x3ECC_CCCD);
    bytes(fx, g, &[(0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20)]);
    g
}

/// Update 0x32 [F 0x1E04E94]: four spawners on nodes 0x3F / 0x46, two
/// slow sprites on them and a long shake.
fn spawners(fx: &mut Fx, r: &mut Regs) {
    for s1 in 0..4u32 {
        let g = spawner(fx, 0x8E, 6, 1, [5, 5, 6], 0xC80);
        r.s0 = u64::from(g - SPAWNS);
        let v = fx.mem.read_u8(D + 0xC + 0xC * (s1 >> 1));
        fx.mem.write_u8(g + 6, v);
        for o in [0x38u32, 0x34, 0x30] {
            fx.mem.write_u32(g + o, 0);
        }
    }
    for s1 in 0..2u32 {
        let g = SLOW + 0x44 * s1;
        r.s0 = u64::from(0x44 * s1);
        set_counter4(fx, g, 0xB2, 0, 0xFF);
        fx.mem.write_u8(g + 5, 1);
        let v = fx.mem.read_u8(D + 0xC + 0xC * s1);
        fx.mem.write_u8(g + 6, v);
        bytes(fx, g, &[(4, 0xC5), (8, 2), (0xB, 2), (9, 0), (0xA, 5), (0xE, 0x80), (0xD, 0x80), (0xC, 0x80), (0xF, 0x60)]);
        fx.mem.write_u32(g + 0x10, 0x43C8_0000);
        for o in [0x14u32, 0x1C, 0x18] {
            fx.mem.write_u32(g + o, 0);
        }
    }
    r.s1 = Some(2);
    let s = SHAKE;
    set_counter4(fx, s, 0x96, 0, 0xFF);
    fx.mem.write_u8(s + 4, 8);
    let v = ee::add(fx.mem.f(s + 0x24), 10.0);
    fx.mem.setf(s + 0x24, v);
}

/// Update 0xB4 [F 0x1E0512C]: per node 0x3F / 0x46 its slow sprite sent
/// to the target over 48 updates with a spawner following it, and 60
/// sprites of +0x5A0 there.
fn launch(fx: &mut Fx, sp: u32, r: &mut Regs) {
    for s1 in 0..2u32 {
        let slot = D + 0xC + 0xC * s1;
        r.s0 = u64::from(slot);
        r.s1 = Some(u64::from(s1));
        let s = fx.mem.i32(slot);
        node_pos(fx, r, s, sp + 0xC0, 0x01E0_5160);
        let g = SLOW + 0x44 * s1;
        fx.mem.write_u8(g + 5, 0);
        copy3(fx, sp + 0xC0, g + 0x20);
        for k in 0..3u32 {
            let v = ee::div(ee::sub(fx.mem.f(sp + 0xE0 + 4 * k), fx.mem.f(sp + 0xC0 + 4 * k)), 48.0);
            fx.mem.setf(g + 0x2C + 4 * k, v);
        }
        let w = spawner(fx, 0x30, 4, 0, [6, 5, 6], 0xBB8);
        let v = fx.mem.read_u8(slot);
        fx.mem.write_u8(w + 6, v);
        copy3(fx, g + 0x20, w + 0x24);
        copy3(fx, g + 0x2C, w + 0x30);
        for s0 in 0..0x3Cu32 {
            r.s0 = u64::from(s0);
            let i = first_free(fx, SPRITES, 0x44, 0x12C);
            if i < 0 {
                continue;
            }
            let delay = rand4(fx, r, 0x01E0_53A0) & 7;
            let q = SPRITES + 0x44 * i as u32;
            set_counter4(fx, q, 0x1C, delay, 0xFF);
            bytes(fx, q, &[(4, 0), (8, 3), (0xB, 2), (0xA, 0)]);
            let v = rand4(fx, r, 0x01E0_53F0) & 7;
            fx.mem.set8(q + 9, v);
            bytes(fx, q, &[(0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)]);
            let m = rand_rem(fx, r, 0x01E0_541C, 0x14);
            let v = ee::add(80.0, ee::div(ee::from_i32(m.wrapping_mul(200) - 0x7D0), 100.0));
            fx.mem.setf(q + 0x10, v);
            fx.mem.write_u32(q + 0x14, 0xBF4C_CCCD);
            fx.mem.write_u32(q + 0x18, 0xBCA3_D70A);
            fx.mem.write_u32(q + 0x1C, 0);
            copy3(fx, g + 0x20, q + 0x20);
            for (k, ra) in [0x01E0_54BCu32, 0x01E0_5508, 0x01E0_5554].iter().enumerate() {
                let m = rand_rem(fx, r, *ra, 0x320);
                let v = ee::div(ee::from_i32(m - 0x190), 100.0);
                fx.mem.setf(q + 0x2C + 4 * k as u32, v);
            }
        }
        r.s0 = 0x3C;
    }
    r.s1 = Some(2);
}

/// Update 0xE4 [F 0x1E055C4]: the target untinted (0x1F75F0 with zeros),
/// its jitter and a window sprite, a spawner on the target, the fade, a
/// shake at the target.
fn strike(fx: &mut Fx, sp: u32, r: &mut Regs) {
    for k in 0..4 {
        fx.mem.write_u8(sp + 0x15C + k, 0);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_tint_mode(t, -1, sp + 0x15C, [r.s0, r.s1.unwrap_or(0)], 0x01E0_55E4);
    let j = JITTER;
    set_counter4(fx, j, 0x36, 0, 0);
    bytes(fx, j, &[(0xC, 1), (0xD, 0xCF), (0xE, 0x10), (0x12, 1), (0x10, 3), (0x11, 0)]);
    let v = fx.mem.read_u32(D + 8);
    fx.mem.write_u32(j + 0x14, v);
    bytes(fx, j, &[(0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x80)]);
    let w = WINDOW;
    fx.mem.set16(w, 0xE4);
    fx.mem.set16(w + 2, 0xF4);
    fx.mem.write_u32(w + 8, 0x4000_0000);
    bytes(fx, w, &[(0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)]);
    let g = spawner(fx, 0x10, 0x84, 0, [8, 4, 7], 0x1770);
    r.s0 = u64::from(g - SPAWNS);
    copy3(fx, sp + 0xE0, g + 0x24);
    for o in [0x38u32, 0x34, 0x30] {
        fx.mem.write_u32(g + o, 0);
    }
    set_counter4(fx, FADE, 8, 8, 0xFF);
    fx.mem.write_u8(FADE + 4, 0x82);
    let s = SHAKE;
    set_counter4(fx, s, 0x36, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 8);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x4000_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    unit_position_saving(fx, t, s + 0x20, r.s0, 0x01E0_5870);
}

/// Updates 0x32..0x3B / 0xB4..0xE3 [F 0x1E058E4]: eight sprites (+0x5A0)
/// about each slow sprite, thrown out along a random turn.
fn spray(fx: &mut Fx, sp: u32, r: &mut Regs) {
    for s0 in 0..2u32 {
        r.s0 = u64::from(s0);
        for s1 in 0..8u64 {
            r.s1 = Some(s1);
            let i = first_free(fx, SPRITES, 0x44, 0x12C);
            if i < 0 {
                continue;
            }
            copy3(fx, SLOW + 0x44 * s0 + 0x20, sp + 0xC0);
            let v = rand4(fx, r, 0x01E0_5954) % 4;
            let f20 = ee::add(16.0, ee::div(ee::from_i32(v * 200 - 0x190), 100.0));
            fx.fix_residue(f20, 0x01E0_59C0);
            let life = ee::fixsfsi(f20) & 0xFF;
            let q = SPRITES + 0x44 * i as u32;
            set_counter4(fx, q, life, 0, 0xFF);
            bytes(fx, q, &[(4, 0x40), (8, 3), (0xB, 2)]);
            let v = (rand4(fx, r, 0x01E0_5A0C) + 2) & 3;
            fx.mem.set8(q + 9, v);
            bytes(fx, q, &[(0xA, 0), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)]);
            fx.mem.write_u32(q + 0x10, 0x4220_0000);
            for o in [0x14u32, 0x1C, 0x18] {
                fx.mem.write_u32(q + o, 0);
            }
            for (k, ra) in [0x01E0_5A54u32, 0x01E0_5A68, 0x01E0_5A7C].iter().enumerate() {
                let v = rand4(fx, r, *ra) & 0xFFFF;
                fx.mem.set32(sp + 0x140 + 4 * k as u32, v);
            }
            let rot = [fx.mem.i32(sp + 0x140), fx.mem.i32(sp + 0x144), fx.mem.i32(sp + 0x148)];
            let m = fx.trs(None, Some(rot), None);
            fx.write_m4(sp + 0xF0, &m);
            fx.mem.write_u32(sp + 0x138, 0);
            fx.mem.write_u32(sp + 0x130, 0);
            let m25 = rand_rem(fx, r, 0x01E0_5AB0, 0x19);
            let v = ee::div(ee::from_i32(m25.wrapping_mul(100) - 0x4E2), 100.0);
            fx.mem.setf(sp + 0x134, v);
            let m = fx.read_m4(sp + 0xF0);
            let v = fx.read_v4(sp + 0x130);
            let o = fx.transform(Some(&m), v);
            fx.write_v4(sp + 0xD0, o);
            copy3(fx, sp + 0xC0, q + 0x20);
            for k in 0..3u32 {
                let v = ee::div(fx.mem.f(sp + 0xD0 + 4 * k), f20);
                fx.mem.setf(q + 0x2C + 4 * k, v);
            }
            for o in [0x40u32, 0x3C, 0x38] {
                fx.mem.write_u32(q + o, 0);
            }
        }
        r.s1 = Some(8);
    }
    r.s0 = 2;
}

/// Updates 0xF4..0x116 [F 0x1E05BAC]: twice, at a point about the target
/// (+-15 / +-15 / +-7.5), three short bolts (delay k) and a flash.
fn crackle(fx: &mut Fx, sp: u32, r: &mut Regs) {
    for s0 in 0..2u32 {
        r.s0 = u64::from(s0);
        for (k, ra, m, half) in [(0u32, 0x01E0_5BC0u32, 0xBB8, 0x5DC), (1, 0x01E0_5C10, 0xBB8, 0x5DC), (2, 0x01E0_5C60, 0x5DC, 0x2EE)] {
            let v = rand_rem(fx, r, ra, m);
            let j = ee::div(ee::from_i32(v - half), 100.0);
            let v = ee::add(fx.mem.f(sp + 0xE0 + 4 * k), j);
            fx.mem.setf(sp + 0xD0 + 4 * k, v);
        }
        let mut s1 = 0u32;
        while s1 < 3 {
            r.s1 = Some(u64::from(s1));
            let i = first_free(fx, BOLTS, 0x40, 0x96);
            if i < 0 {
                break;
            }
            let b = BOLTS + ((i as u32) << 6);
            set_counter4(fx, b, 8, s1 as i32, 0xFF);
            fx.mem.write_u8(b + 5, 0);
            fx.mem.write_u8(b + 4, 0x84);
            for k in 0..3u32 {
                let w = fx.mem.read_u32(sp + 0xD0 + 4 * k);
                fx.mem.write_u32(b + 0x28 + 4 * k, w);
                fx.mem.write_u32(b + 0x1C + 4 * k, w);
            }
            for (k, ra) in [0x01E0_5D40u32, 0x01E0_5DA0, 0x01E0_5E00].iter().enumerate() {
                let v = rand_rem(fx, r, *ra, 0xC);
                let j = ee::div(ee::from_i32(v.wrapping_mul(200) - 0x4B0), 100.0);
                let a = b + 0x28 + 4 * k as u32;
                let v = ee::add(fx.mem.f(a), j);
                fx.mem.setf(a, v);
            }
            fx.mem.write_u8(b + 8, 6);
            fx.mem.write_u32(b + 0x10, 0x3F80_0000);
            fx.mem.write_u32(b + 0x14, 0x3ECC_CCCD);
            bytes(fx, b, &[(0xB, 0), (0xD, 4), (0xC, 0), (0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x20)]);
            s1 += 1;
        }
        r.s1 = Some(u64::from(s1));
        let i = first_free(fx, FLASH, 0x4C, 0x24);
        if i < 0 {
            continue;
        }
        r.s1 = Some(u64::from(0x4C * i as u32));
        let f = FLASH + 0x4C * i as u32;
        set_counter4(fx, f, 0xE, 0, 0xFF);
        bytes(fx, f, &[(5, 0), (4, 0x85), (7, 9), (0xF, 0xD0), (0xC, 0xD0), (0xD, 0xD0), (0xE, 0xD0), (0xA, 0x60), (9, 0), (8, 0x80)]);
        fx.mem.write_u32(f + 0x14, 0);
        fx.mem.write_u32(f + 0x10, 0x437A_0000);
        copy4(fx, sp + 0xD0, f + 0x1C);
    }
    r.s0 = 2;
}

/// 0x1E03EB0 (b186b3c26dc3, b398's `window` at this record's addresses)
/// [F]: `window(p, ot, n) -> 1`, a window sprite while the update lies
/// in [u16 +0, u16 +2]. Returns the a3 it leaves when drawn: 6 (what
/// the ALPHA preset 0x208450 of 3 leaves [run; H: its index 2 x 3]; the
/// runner's 0x206F30 / 0x207910 keep it). `s`: the caller's s0 / s1 (saved).
fn window(fx: &mut Fx, p: u32, ot: u32, n: i32, s: [u64; 2]) -> Option<u32> {
    let sp = fx.enter(0x80);
    // Its saves (sq): the caller's s0 / s1 (upper halves 0) and ra, read
    // as stale vertex words by the jitter's first TEXFLUSH [run].
    fx.saved(sp, &[(0, s[0]), (8, 0), (0x10, s[1]), (0x18, 0), (0x40, 0x01E0_6060)]);
    let layer = fx.layer_begin();
    let s = n & 0xFFFF;
    if s < fx.mem.u16(p) || fx.mem.u16(p + 2) < s {
        fx.leave(0x80);
        return None;
    }
    let d = sp + 0x50;
    fx.mem.write_u16(d + 2, 6);
    fx.mem.write_u16(d, 3);
    fx.mem.write_u64(d + 8, 0x441);
    fx.mem.write_u32(d + 4, 0);
    for (o, src) in [(0x60u32, 0xEu32), (0x61, 0xD), (0x62, 0xC), (0x63, 0xF)] {
        let v = fx.mem.read_u8(p + src);
        fx.mem.write_u8(sp + o, v);
    }
    fx.mem.write_u32(sp + 0x64, 0x3F80_0000);
    for (at, xy, z) in [(0x68u32, 0x6FF8u16, 0xFF_FFFFu64), (0x70, 0x8FF8, 0xFF_FFFE)] {
        fx.mem.write_u16(sp + at, xy);
        fx.mem.write_u16(sp + at + 2, xy);
        fx.set_xyz_z(sp + at, z);
        fx.mem.write_u8(sp + at + 7, 0);
    }
    let alpha = fx.alpha_preset(3);
    fx.desc_alpha(d, alpha);
    let w = fx.mem.f(p + 8);
    fx.fix_residue(w, 0x01E0_3FF4);
    fx.submit(d, 0, ee::fixunssfsi(w) as i32);
    fx.layer_end(layer, ot, 0);
    fx.leave(0x80);
    Some(6)
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let mut r = Regs { s0: i64::from(param) as u64, s1: None };
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0xE0, r.s0, 0x01E0_4A30);
    match fx.mem.u16(D) {
        0x136 => return 8,
        0x11A => {
            for k in 0..4 {
                fx.mem.write_u8(sp + 0x15C + k, 0x80);
            }
            let t = fx.mem.i32(D + 8);
            fx.unit_tint_mode(t, -1, sp + 0x15C, [r.s0, r.s1.unwrap_or(0)], 0x01E0_589C);
        }
        0xE4 => strike(fx, sp, &mut r),
        0xB4 => launch(fx, sp, &mut r),
        0x32 => spawners(fx, &mut r),
        0x26 => midway(fx, sp, &mut r),
        2 => opening(fx, &mut r),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x32..0x3C).contains(&n) || (0xB4..0xE4).contains(&n) {
        spray(fx, sp, &mut r);
    }
    let n = fx.mem.u16(D);
    if (0xF4..0x117).contains(&n) {
        crackle(fx, sp, &mut r);
    }
    shake_saving(fx, SHAKE, r.s0, 0x01E0_5F9C, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_5FAC, [Some(r.s0), r.s1]);
    blib::fade::fade(fx, FADE, OT);
    blib::star::flash(fx, 0x01E0_0950, FLASH, OT, 0x24);
    segment_spawns_saving(fx, 0x01E0_2AC0, SPAWNS, 0x1E, 0x01E0_5FE8);
    let (_, bolt_a3) = blib::bolt::bolts_a3(fx, 0x01E0_1F70, BOLTS, OT, 0x96);
    let (_, slow_a3) = blib::sprite::bounce_a3(fx, SLOW, OT, 2);
    let (_, sprites_a3) = blib::sprite::bounce_a3(fx, SPRITES, OT, 0x12C);
    blib::streak::ribbon(fx, 0x01E0_2E40, RIBBON, OT, 0x1E);
    // [H] a3 = what the sprites / bolts left, else not modelled (0).
    let a3 = sprites_a3.or(slow_a3).or(bolt_a3).unwrap_or(0);
    let n = fx.mem.u16(D);
    blib::window::frame_fade(fx, 0x01E0_39D0, D + 0xAC, OT, n, a3);
    let n = fx.mem.u16(D);
    let a3 = window(fx, WINDOW, OT, n, [r.s0, r.s1.unwrap_or(0)]).unwrap_or(a3);
    blib::radiate::jitter(fx, 0x01E0_4040, JITTER, OT, 1, r.s0 as u32, a3);
    let n = fx.mem.u16(D);
    if (0xE4..0x117).contains(&n) {
        6
    } else if n == 0x117 {
        2
    } else if n == 0xDA {
        1
    } else {
        0
    }
}
