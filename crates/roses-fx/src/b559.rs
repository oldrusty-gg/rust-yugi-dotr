//! BMODULE b559 (card 354), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03D30, data area a0 = 0x1ED0000) [F] (helper B4, 31
//! Oct 2026): b586's library (s0 = D, s1 = the OT). On update 2 two long
//! flashes and two segment spawners (+0x2CC4, `emit::segment_spawns`, into
//! the bolt pool +0x1DC4) on the attacker's nodes 0x2A / 0x31 and a long
//! camera shake (+0x40); on 70 two small flashes there; on 72 a fade
//! (+0x3584), a second shake, eight bead beams (+0x2EA4) on the nodes and
//! four jagged bolts (0x102A50, six segments each) from the nodes to the
//! target; on even updates 2..191 two ribbons a update from the nodes
//! (+0xA4); on 118 a wide flash and two spawners at the target; on
//! 118..254 two flashes and two ribbons a update round the target; the
//! stage light pulsed by 0x1031C0 (+0xA0: down on update 2, up on 264).
//! Returns 1 on update 108, 6 on 118..254, 2 on 255, 8 on 270 (and at
//! once for a parameter of -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `flash`
//! 0x1E00410, `ribbon` 0x1E00FC0, `bolts` 0x1E01B50, `segment_spawns`
//! 0x1E026A0, `beads` 0x1E02A20, `fade` 0x1E037F0 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b586::segment_spawns_saving;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x358C;
const FLASHES: u32 = D + 0x1904;
const BOLTS: u32 = D + 0x1DC4;
const SPAWNERS: u32 = D + 0x2CC4;

/// The entry 0x1E03D30 (a0 = D, a1 = param; s0 = D, s1 = the OT).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1A0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0x1A0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x3690);
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
    for (at, node, clear) in [(0xCu32, 0x2A, 1), (0x10, 0x31, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x108);
    // The entry's s0 = D, s1 = the OT, s2 = 2 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_3E4C, [Some(D), Some(OT), Some(2)]);
    0
}

fn flash_bytes(fx: &mut Fx, r: u32, b4: u8, c: u8, a: u8, b9: u8, b8: u8) {
    fx.mem.write_u8(r + 4, b4);
    fx.mem.write_u8(r + 7, 9);
    for (o, v) in [(0xAu32, a), (9, b9), (8, b8), (0xF, c), (0xC, c), (0xD, c), (0xE, c)] {
        fx.mem.write_u8(r + o, v);
    }
}

/// A segment spawner (+0x2CC4 + 0x3C i) feeding the bolt pool: count /
/// delay / mask, node `node` (None: at `at`), range `r`.
fn spawner(fx: &mut Fx, s: u32, c: [i32; 3], node: Option<u8>, spread: (u8, i32), at: u32) {
    set_counter4(fx, s, c[0], c[1], c[2]);
    match node {
        Some(n) => {
            fx.mem.write_u8(s + 5, 1);
            fx.mem.write_u8(s + 6, n);
            fx.mem.write_u8(s + 4, 2);
        }
        None => {
            fx.mem.write_u8(s + 5, 0);
            fx.mem.write_u8(s + 4, 0);
        }
    }
    fx.mem.write_u32(s + 8, BOLTS);
    for (o, v) in [(0xCu32, 0x28u8), (0xD, spread.0), (0xE, 1), (0xF, 0), (0x10, 5), (0x11, 0xC)] {
        fx.mem.write_u8(s + o, v);
    }
    fx.mem.set32(s + 0x14, spread.1);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x3ECC_CCCD);
    for (o, v) in [(0x20u32, 0x80u8), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20)] {
        fx.mem.write_u8(s + o, v);
    }
    if node.is_none() {
        for c in 0..3u32 {
            let w = fx.mem.read_u32(at + 4 * c);
            fx.mem.write_u32(s + 0x24 + 4 * c, w);
        }
    }
    for o in [0x38u32, 0x34, 0x30] {
        fx.mem.write_u32(s + o, 0);
    }
}

/// Update 2 [F 0x1E03EBC].
fn charge(fx: &mut Fx, sp: u32) {
    for k in 0..2u32 {
        let r = FLASHES + 0x4C * k;
        set_counter4(fx, r, 0xA0, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0xC + 4 * k);
        fx.mem.write_u8(r + 6, b);
        flash_bytes(fx, r, 0x85, 0xC0, 0x40, 0x20, 0x80);
        fx.mem.write_u32(r + 0x10, 0x42C8_0000);
        fx.mem.write_u32(r + 0x14, 0);
    }
    for k in 0..2u32 {
        // No check of the free index: -1 writes the record before the pool.
        let i = first_free(fx, SPAWNERS, 0x3C, 8);
        let s = SPAWNERS.wrapping_add((i.wrapping_mul(0x3C)) as u32);
        let n = fx.mem.read_u8(D + 0xC + 4 * k);
        spawner(fx, s, [0x40, 8, 0], Some(n), (8, 0x44C), sp);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0xFA, 0, 0xFF);
    fx.mem.write_u8(s + 5, 1);
    let v = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(s + 6, v);
    fx.mem.write_u8(s + 4, 4);
    fx.mem.write_u32(s + 0x14, 0x3F99_999A);
    fx.mem.write_u32(s + 0x18, 0x3F99_999A);
    fx.mem.write_u32(s + 0x1C, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x4000_0000);
}

/// Update 70 [F 0x1E040C4]: a small flash on each node.
fn sparks(fx: &mut Fx) {
    for k in 0..2u32 {
        let i = first_free(fx, FLASHES, 0x4C, 0x10);
        if i < 0 {
            continue;
        }
        let r = FLASHES + 0x4C * i as u32;
        set_counter4(fx, r, 0x14, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0xC + 4 * k);
        fx.mem.write_u8(r + 6, b);
        flash_bytes(fx, r, 0x80, 0xB0, 0x40, 0x20, 0x80);
        fx.mem.write_u32(r + 0x10, 0x4040_0000);
        fx.mem.write_u32(r + 0x14, 0x42F0_0000);
        fx.mem.write_u32(r + 0x18, 0x4220_0000);
    }
}

/// Update 72 [F 0x1E041AC]: the fade, the shake at the target, eight bead
/// beams on the nodes and four jagged bolts node -> target.
fn bolts_start(fx: &mut Fx, sp: u32) {
    set_counter4(fx, D + 0x3584, 0x10, 0, 0xFF);
    fx.mem.write_u8(D + 0x3588, 1);
    let s = D + 0x40;
    set_counter4(fx, s, 0xC0, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 8);
    fx.mem.write_u32(s + 0x14, 0x3F80_0000);
    fx.mem.write_u32(s + 0x18, 0x3FB3_3333);
    fx.mem.write_u32(s + 0x1C, 0x4000_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4320_0000);
    copy_v4(fx, sp + 0x60, D + 0x60);
    fx.mem.write_u32(s + 0x50, 0x4100_0000);
    fx.mem.write_u32(D + 0x68, 0);
    for k in 0..8i32 {
        let r = D + 0x2EA4 + 0x58 * k as u32;
        set_counter4(fx, r, 0x1C, k & 0xFF, 0xFF);
        let c = fx.mem.read_u8(r).wrapping_sub(4);
        fx.mem.write_u8(r, c);
        fx.mem.write_u8(r + 4, 5);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 8, 0x1E);
        fx.mem.set8(r + 9, k);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.write_u8(r + 0xC, 2);
        let x = fx.rand_from(D, 0x01E0_42C0) >> 4;
        fx.mem.set8(r + 0xB, x & 3);
        fx.mem.write_u32(r + 0x1C, 0x4248_0000);
        fx.mem.write_u32(r + 0x20, 0xBF19_999A);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x14, 0x3F80_0000);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(r + o, 0x80);
        }
        let h = fx.mem.i32(D + 4);
        let n = fx.mem.i32(D + 0xC + 4 * (k as u32 & 1));
        node_position_saving(fx, h, n, r + 0x28, u64::from(D), Some(u64::from(OT)), 0x01E0_4328);
    }
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0xC);
    node_position_saving(fx, h, n, sp + 0x80, u64::from(D), Some(u64::from(OT)), 0x01E0_4350);
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x10);
    node_position_saving(fx, h, n, sp + 0x90, u64::from(D), Some(u64::from(OT)), 0x01E0_4364);
    let p = sp + 0xA0;
    for j in 0..4u32 {
        fx.mem.set32(p, 6);
        fx.mem.write_u32(p + 4, 0x3ECC_CCCD);
        fx.mem.write_u32(p + 0x20, sp + 0xD0);
        let from = sp + 0x80 + 0x10 * (j & 1);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(from + 4 * c);
            fx.mem.write_u32(p + 8 + 4 * c, w);
        }
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x60 + 4 * c);
            fx.mem.write_u32(p + 0x14 + 4 * c, w);
        }
        blib::bolt::jag(fx, p);
        for s in 0..6u32 {
            let i = first_free(fx, BOLTS, 0x40, 0x3C);
            if i < 0 {
                continue;
            }
            let b = BOLTS + ((i as u32) << 6);
            set_counter4(fx, b, 0x8C, ((9 * s + j) & 0xFF) as i32, 0xFF);
            fx.mem.write_u8(b + 5, 0);
            fx.mem.write_u8(b + 4, 1);
            for c in 0..6u32 {
                let w = fx.mem.read_u32(sp + 0xD0 + 12 * s + 4 * c);
                fx.mem.write_u32(b + 0x1C + 4 * c, w);
            }
            fx.mem.write_u8(b + 8, 9);
            fx.mem.write_u32(b + 0x10, 0x3F33_3333);
            fx.mem.write_u32(b + 0x14, 0x3E4C_CCCD);
            for (o, v) in [(0xBu32, 1u8), (0xD, 3), (0xC, 0), (0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x20)] {
                fx.mem.write_u8(b + o, v);
            }
        }
    }
}

/// Update 118 [F 0x1E044FC]: a wide flash (+0x199C) and two spawners at
/// the target (sp + 0x60).
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x60;
    let r = D + 0x199C;
    set_counter4(fx, r, 0x84, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    flash_bytes(fx, r, 0x83, 0x80, 0x40, 0x20, 0x80);
    fx.mem.write_u32(r + 0x10, 0x447A_0000);
    fx.mem.write_u32(r + 0x14, 0);
    copy_v4(fx, at, r + 0x1C);
    for k in 0..2u32 {
        spawner(fx, SPAWNERS + 0x3C * k, [0x80, 0, 3], None, (0xC, 0x1770), at);
    }
}

/// Even updates 2..191 [F 0x1E0468C]: a ribbon from each node.
fn ribbons(fx: &mut Fx) {
    for k in 0..2u32 {
        let i = first_free(fx, D + 0xA4, 0x68, 0x3C);
        if i < 0 {
            continue;
        }
        let r = D + 0xA4 + 0x68 * i as u32;
        set_counter4(fx, r, 0x10, 0, 0xFF);
        fx.mem.write_u8(r + 4, 5);
        if fx.mem.u16(D) < 0x48 {
            fx.mem.write_u32(r + 0x14, 0x3F19_999A);
            fx.mem.write_u32(r + 0x1C, 0x4170_0000);
            fx.mem.write_u8(r + 8, 8);
        } else {
            fx.mem.write_u32(r + 0x14, 0x4000_0000);
            fx.mem.write_u32(r + 0x1C, 0x41F0_0000);
            fx.mem.write_u8(r + 8, 0x12);
        }
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0xC + 4 * k);
        fx.mem.write_u8(r + 6, b);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xC, 3);
        let x = fx.rand_from(D, 0x01E0_4788);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u8(r + 0x23, 0x10);
        fx.mem.write_u8(r + 0x27, 0x40);
        for (o, ra) in [(0x58u32, 0x01E0_47D8u32), (0x5C, 0x01E0_47F0), (0x60, 0x01E0_4808)] {
            let x = fx.rand_from(D, ra) >> 4;
            fx.mem.set32(r + o, x & 0x7F);
        }
    }
}

/// Updates 118..254 [F 0x1E04850]: two flashes and two ribbons a update
/// round the target (+-14 in x, y).
fn crackle(fx: &mut Fx, sp: u32) {
    let p = sp + 0x70;
    for _ in 0..2 {
        for (c, ra) in [(0u32, 0x01E0_4860u32), (4, 0x01E0_48AC)] {
            let x = fx.rand_from(D, ra) >> 4;
            let (_, h) = fx.div(x, 0xAF0);
            let f = ee::div(ee::from_i32(h - 0x578), 100.0);
            let v = ee::add(fx.mem.f(sp + 0x60 + c), f);
            fx.mem.setf(p + c, v);
        }
        let z = fx.mem.read_u32(sp + 0x68);
        fx.mem.write_u32(p + 8, z);
        let i = first_free(fx, FLASHES, 0x4C, 0x10);
        if i >= 0 {
            let r = FLASHES + 0x4C * i as u32;
            set_counter4(fx, r, 0xC, 0, 0xFF);
            fx.mem.write_u8(r + 5, 0);
            flash_bytes(fx, r, 1, 0xC0, 0x60, 0x10, 0x40);
            fx.mem.write_u32(r + 0x14, 0);
            fx.mem.write_u32(r + 0x10, 0x4396_0000);
            copy_v4(fx, p, r + 0x1C);
        }
        let i = first_free(fx, D + 0xA4, 0x68, 0x3C);
        if i < 0 {
            continue;
        }
        let r = D + 0xA4 + 0x68 * i as u32;
        fx.mem.write_u8(r + 4, 5);
        set_counter4(fx, r, 8, 0, 0xFF);
        fx.mem.write_u32(r + 0x14, 0x40A0_0000);
        fx.mem.write_u32(r + 0x1C, 0x4220_0000);
        fx.mem.write_u8(r + 0x27, 0xC0);
        fx.mem.write_u8(r + 0x23, 0);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xC, 3);
        fx.mem.write_u8(r + 8, 0x28);
        let x = fx.rand_from(D, 0x01E0_4A60);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, p, r + 0x28);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(D), 0x01E0_3E74);
    match fx.mem.u16(D) {
        0x10E => return 8,
        0x76 => strike(fx, sp),
        0x48 => bolts_start(fx, sp),
        0x46 => sparks(fx),
        2 => charge(fx, sp),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (2..0xC0).contains(&n) && n & 1 == 0 {
        ribbons(fx);
    }
    let n = fx.mem.u16(D);
    if (0x76..0xFF).contains(&n) {
        crackle(fx, sp);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_4AE4, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_4AF4, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::star::flash(fx, 0x01E0_0410, FLASHES, OT, 0x10);
    blib::streak::ribbon(fx, 0x01E0_0FC0, D + 0xA4, OT, 0x3C);
    segment_spawns_saving(fx, 0x01E0_26A0, SPAWNERS, 8, 0x01E0_4B30);
    blib::bolt::bolts(fx, 0x01E0_1B50, BOLTS, OT, 0x3C);
    blib::fade::fade(fx, D + 0x3584, OT);
    blib::beam::beads(fx, 0x01E0_2A20, D + 0x2EA4, OT, 0x14);
    let n = fx.mem.u16(D);
    if (0x76..0xFF).contains(&n) {
        6
    } else if n == 0xFF {
        2
    } else if n == 0x6C {
        1
    } else {
        0
    }
}
