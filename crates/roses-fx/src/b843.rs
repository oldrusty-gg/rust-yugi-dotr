//! BMODULE b843 (card 672), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E069F0, data area a0 = 0x1ED0000) [F] (helper B4, 31
//! Oct 2026): b076's "a0 = D" family. The record's node-request helper
//! 0x1E06740 picks the attacker's nodes (+0xC.. +0x28) in four sets (init,
//! updates 69, 155, 184). Node bursts (+0xA0, `burst::node_bursts`, motes
//! +0xE4) every update; bouncing sprites (`sprite::bounce`, pool of 120 at
//! +0x1930) from the nodes: on update 50 twenty from each of four, on 70
//! twenty from each of two, on 182 thirty from each of two; on 223 an arc
//! pair (+0x1464, `arc::arcs`); on 228 at the target a fade (+0x3D50), a
//! flash (+0x17A4), two ribbons (+0x3C80, turned -+0x4000), two + eight
//! bead beams (+0x3910, `beam::beads`, pool of 10), four bars (+0x17F0)
//! and a camera shake (+0x40). Returns 1 on update 218, 6 on 228..248, 2
//! on 249, 8 on 300 (and at once for a parameter of -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `arcs` 0x1E015F0, `bounce`
//! 0x1E02870, `flash` 0x1E032E0, `ribbon` 0x1E03E90, `fade` 0x1E04A20,
//! `beads` 0x1E04F60, `bars` 0x1E05D30 [`crate::blib`].
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const POOL: u32 = D + 0x1930;
const BYTES: u32 = D + 0x3D58;

/// The entry 0x1E069F0 (a0 = D, a1 = param; s0 = D, s1 = the OT).
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

/// 0x1E06740(D, set) [F]: the attacker's nodes for set 0..3 into +0xC..;
/// the first cleared (clear 1). Leaves its frame (0x20: the caller's s0 =
/// D +0, `ra` +0x10).
fn request_nodes(fx: &mut Fx, set: i32, ra: u32) {
    let nodes: &[i32] = match set {
        0 => &[0x60, 0x68, 0x75, 0x7E],
        1 => &[0x63, 0x6B, 0x78, 0x81, 0x40, 0x4D],
        2 => &[0x63, 0x6B, 0x78, 0x81, 0x5A, 0x70],
        3 => &[0x63, 0x6B, 0x78, 0x81, 0x5D, 0x5C, 0x73, 0x72],
        _ => &[],
    };
    for (k, &node) in nodes.iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set32(D + 0xC + 4 * k as u32, r);
    }
    let f = fx.sp - 0x20;
    fx.saved(f, &[(0, u64::from(D)), (0x10, u64::from(ra))]);
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x3E60);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    for k in 0..3 {
        fx.texture(5, k);
    }
    request_nodes(fx, 0, 0x01E0_6ACC);
    // The node bursts (+0xA0): specs 0x1E07BF0 / 0x1E07BD0 (x2) /
    // 0x1E07BF0, motes +0xE4 / +0x7E4, nodes from +0xC..+0x18.
    for k in 0..4 {
        let b = fx.mem.read_u8(D + 0xC + 4 * k);
        fx.mem.write_u8(D + 0xC8 + k, b);
    }
    fx.mem.write_u32(D + 0xA8, 0x01E0_7BF0);
    fx.mem.write_u32(D + 0xAC, 0x01E0_7BD0);
    fx.mem.write_u32(D + 0xB0, 0x01E0_7BD0);
    fx.mem.write_u32(D + 0xB4, 0x01E0_7BF0);
    fx.mem.write_u32(D + 0xA0, D + 0xE4);
    fx.mem.write_u8(D + 0xD0, 0x20);
    fx.mem.write_u32(D + 0xA4, D + 0x7E4);
    fx.mem.write_u8(D + 0xD1, 0x50);
    fx.mem.write_u8(D + 0xD2, 1);
    fx.mem.set16(D + 0xD4, 0);
    fx.mem.write_u32(D + 0xD8, 0x430C_0000);
    fx.mem.write_u32(D + 0xDC, 0);
    fx.mem.write_u32(D + 0xE0, 0x4020_0000);
    // The entry's s0 = D, s1 = the OT, s2 = 3 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_6B6C, [Some(D), Some(OT), Some(3)]);
    0
}

/// (rand >> 4) % m - off, over 100 (rand called from `ra`).
fn spread(fx: &mut Fx, ra: u32, m: i32, off: i32) -> f32 {
    let x = fx.rand_from(D, ra) >> 4;
    let (_, h) = fx.div(x, m);
    ee::div(ee::from_i32(h - off), 100.0)
}

/// How a group of sprites is thrown from the nodes.
struct Throw {
    /// The nodes' first word (+0xC / +0x1C), how many nodes, sprites each.
    nodes: u32,
    count: i32,
    each: i32,
    /// +0x10 / +0x14.
    size: u32,
    vy: u32,
    /// rand's return addresses: delay, node_position, then the spreads.
    ras: [u32; 5],
}

/// One sprite group [F 0x1E06C0C / 0x1E06E6C / 0x1E0709C].
fn throw(fx: &mut Fx, sp: u32, t: &Throw, spreads: impl Fn(&mut Fx, i32, u32)) {
    for k in 0..t.count {
        for _ in 0..t.each {
            let i = first_free(fx, POOL, 0x44, 0x78);
            if i < 0 {
                continue;
            }
            let delay = (fx.rand_from(D, t.ras[0]) >> 4) & 7;
            let b = POOL + 0x44 * i as u32;
            set_counter4(fx, b, 0x20, delay, 0xFF);
            for (o, v) in [(4u32, 0u8), (8, 2), (0xB, 2), (0xA, 0), (9, 5), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)] {
                fx.mem.write_u8(b + o, v);
            }
            fx.mem.write_u32(b + 0x10, t.size);
            fx.mem.write_u32(b + 0x14, t.vy);
            fx.mem.write_u32(b + 0x18, 0xBD4C_CCCD);
            fx.mem.write_u32(b + 0x1C, 0);
            let h = fx.mem.i32(D + 4);
            let n = fx.mem.i32(D + t.nodes + 4 * k as u32);
            node_position_saving(fx, h, n, sp + 0x70, u64::from(D), Some(u64::from(OT)), t.ras[1]);
            for c in 0..3 {
                let w = fx.mem.read_u32(sp + 0x70 + 4 * c);
                fx.mem.write_u32(b + 0x20 + 4 * c, w);
            }
            spreads(fx, k, b);
        }
    }
}

const A: Throw = Throw { nodes: 0xC, count: 4, each: 0x14, size: 0x41A0_0000, vy: 0x3FC0_0000, ras: [0x01E0_6C4C, 0x01E0_6CE8, 0x01E0_6D0C, 0x01E0_6D8C, 0x01E0_6DE8] };
const B: Throw = Throw { nodes: 0x1C, count: 2, each: 0x14, size: 0x41A0_0000, vy: 0x3FC0_0000, ras: [0x01E0_6EAC, 0x01E0_6F48, 0x01E0_6F6C, 0x01E0_6FB8, 0x01E0_7014] };
const C: Throw = Throw { nodes: 0x1C, count: 2, each: 0x1E, size: 0x4270_0000, vy: 0xBECC_CCCD, ras: [0x01E0_70DC, 0x01E0_717C, 0x01E0_71A0, 0x01E0_7220, 0] };

/// Update 223 [F 0x1E072C4]: the arc pair (+0x1464 + 0x60 k, points
/// +0x1524 both), nodes +0x1C / +0x20 and +0x24 / +0x28.
fn arcs_start(fx: &mut Fx) {
    for k in 0..2u32 {
        let g = D + 0x1464 + 0x60 * k;
        set_counter4(fx, g, 0x10, 0, 0xFF);
        fx.mem.write_u8(g + 4, 1);
        fx.mem.write_u32(g + 8, D + 0x1524);
        fx.mem.write_u8(g + 0x30, 0x28);
        fx.mem.write_u8(g + 0x31, 8);
        let b = fx.mem.read_u8(D + 0x1C + 8 * k);
        fx.mem.write_u8(g + 0x32, b);
        let b = fx.mem.read_u8(D + 0x20 + 8 * k);
        fx.mem.write_u8(g + 0x33, b);
        fx.mem.write_u8(g + 0x34, 1);
        fx.mem.set16(g + 0x36, 0x40);
        fx.mem.write_u32(g + 0x38, 0x3F86_6666);
        fx.mem.write_u32(g + 0x40, 0x4120_0000);
        for (o, v) in [(0x4Au32, 0x40u8), (0x49, 0x40), (0x48, 0x40), (0x46, 0xC0), (0x45, 0x10), (0x44, 0x30), (0x47, 0xC0)] {
            fx.mem.write_u8(g + o, v);
        }
        let mut a = 0u32;
        while a < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
            fx.mem.set16(g + 0x1E + 2 * a, -1);
            fx.mem.set16(g + 0xC + 2 * a, -1);
            a += 1;
        }
    }
}

/// Update 228 [F 0x1E073D4]: at the target (sp + 0x60).
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x60;
    set_counter4(fx, D + 0x3D50, 8, 0, 0xFF);
    fx.mem.write_u8(D + 0x3D54, 1);
    let g = D + 0x17A4;
    set_counter4(fx, g, 0x1C, 0, 0xFF);
    fx.mem.write_u8(g + 5, 0);
    fx.mem.write_u8(g + 4, 0x81);
    fx.mem.write_u8(g + 7, 9);
    fx.mem.write_u32(g + 0x10, 0x4396_0000);
    fx.mem.write_u32(g + 0x14, 0x41F0_0000);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x40), (8, 0x20), (0xF, 0xC0)] {
        fx.mem.write_u8(g + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(g + o, 0);
    }
    copy_v4(fx, at, g + 0x1C);
    fx.mem.write_u32(g + 0x18, 0x41A0_0000);
    for k in 0..2u32 {
        let r = D + 0x3C80 + 0x68 * k;
        fx.mem.write_u8(r + 4, 4);
        set_counter4(fx, r, 0x1C, 0, 0xFF);
        fx.mem.write_u32(r + 0x1C, 0x41F0_0000);
        fx.mem.write_u8(r + 0x27, 0x80);
        fx.mem.write_u8(r + 0x23, 0x10);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 0xA, 3);
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xC, 3);
        fx.mem.write_u8(r + 8, 0x50);
        let x = fx.rand_from(D, 0x01E0_751C);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x14, 0x3FE6_6666);
        fx.mem.write_u32(r + 0x18, 0x3E99_999A);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, at, r + 0x28);
        fx.mem.set32(r + 0x50, if k == 0 { -0x4000 } else { 0x4000 });
        let q = D + 0x3910 + 0x58 * k;
        set_counter4(fx, q, 0x1C, 4, 0xFF);
        fx.mem.write_u32(q + 0x1C, 0x42A0_0000);
        fx.mem.write_u32(q + 0x14, 0x3F4C_CCCD);
        fx.mem.write_u32(q + 0x18, 0x3ECC_CCCD);
        fx.mem.write_u8(q + 0xA, 2);
        fx.mem.write_u8(q + 0xC, 2);
        fx.mem.write_u8(q + 8, 0x50);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(q + o, 0xC0);
        }
        fx.mem.write_u8(q + 4, 4);
        fx.mem.write_u8(q + 5, 0);
        let x = fx.rand_from(D, 0x01E0_7638);
        fx.mem.set8(q + 9, x >> 4);
        fx.mem.write_u32(q + 0x10, BYTES);
        copy_v4(fx, at, q + 0x28);
        let w = fx.mem.read_u32(r + 0x50);
        fx.mem.write_u32(q + 0x50, w);
    }
    for k in 0..8 {
        let i = first_free(fx, D + 0x3910, 0x58, 0xA);
        if i < 0 {
            continue;
        }
        let x = fx.rand_from(D, 0x01E0_76C0) >> 4;
        let count = ((x & 3) + 0x1E) & 0xFF;
        let q = D + 0x3910 + 0x58 * i as u32;
        set_counter4(fx, q, count, k & 0xFF, 0xFF);
        fx.mem.write_u8(q + 4, 5);
        fx.mem.write_u8(q + 5, 0);
        fx.mem.write_u8(q + 8, 0x14);
        let x = fx.rand_from(D, 0x01E0_771C);
        fx.mem.set8(q + 9, x >> 4);
        fx.mem.write_u8(q + 0xA, 3);
        fx.mem.write_u8(q + 0xC, 2);
        fx.mem.write_u8(q + 0xB, 3);
        fx.mem.write_u32(q + 0x1C, 0x4270_0000);
        fx.mem.write_u32(q + 0x20, 0xBF80_0000);
        fx.mem.write_u32(q + 0x10, BYTES);
        fx.mem.write_u32(q + 0x14, 0x3F66_6666);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(q + o, 0x80);
        }
        copy_v4(fx, at, q + 0x28);
    }
    for k in 0..4i32 {
        let r = D + 0x17F0 + 0x50 * k as u32;
        set_counter4(fx, r, 0x18, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0x80);
        fx.mem.write_u32(r + 0x10, if k == 2 { 0x44FA_0000 } else { 0x447A_0000 });
        fx.mem.write_u32(r + 0x14, 0x41F0_0000);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x30), (0xA, 0x80), (9, 0x40), (8, 0x20)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.setf(r + 0x18, ee::mul(f32::from_bits(0x4234_0000), ee::from_i32(k)));
        fx.mem.write_u32(r + 0x1C, 0x40C0_0000);
        copy_v4(fx, at, r + 0x20);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x24, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, D + 0x60, u64::from(D), 0x01E0_7910);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(D), 0x01E0_6B94);
    match fx.mem.u16(D) {
        0x12C => return 8,
        0xE4 => strike(fx, sp),
        0xDF => arcs_start(fx),
        0xB8 => request_nodes(fx, 3, 0x01E0_72BC),
        0xB6 => throw(fx, sp, &C, |fx, k, b| {
            let f = spread(fx, C.ras[2], 0x1E, 0xF);
            let base = f32::from_bits(if k > 0 { 0xBE99_999A } else { 0x3E99_999A });
            fx.mem.setf(b + 0x2C, ee::add(base, f));
            let f = spread(fx, C.ras[3], 0x14, 0xA);
            fx.mem.setf(b + 0x30, ee::add(f32::from_bits(0x3ECC_CCCD), f));
            fx.mem.write_u32(b + 0x34, 0xBFE6_6666);
        }),
        0x9B => request_nodes(fx, 2, 0x01E0_7094),
        0x46 => throw(fx, sp, &B, |fx, _, b| {
            let f = spread(fx, B.ras[2], 0x28, 0x14);
            fx.mem.setf(b + 0x2C, f);
            let f = spread(fx, B.ras[3], 0x14, 0xA);
            fx.mem.setf(b + 0x30, ee::add(f32::from_bits(0x3F4C_CCCD), f));
            let f = spread(fx, B.ras[4], 0x28, 0x14);
            fx.mem.setf(b + 0x34, f);
        }),
        0x45 => request_nodes(fx, 1, 0x01E0_6E64),
        0x32 => throw(fx, sp, &A, |fx, k, b| {
            let f = spread(fx, A.ras[2], 0x28, 0x14);
            let base = f32::from_bits(if k < 2 { 0x3E99_999A } else { 0xBE99_999A });
            fx.mem.setf(b + 0x2C, ee::add(base, f));
            let f = spread(fx, A.ras[3], 0x1E, 0xF);
            fx.mem.setf(b + 0x30, ee::add(f32::from_bits(0x3F19_999A), f));
            let f = spread(fx, A.ras[4], 0x32, 0x19);
            fx.mem.setf(b + 0x34, f);
        }),
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_7930, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    blib::fade::fade(fx, D + 0x3D50, OT);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA0, n);
    blib::mote::motes(fx, D + 0xE4, OT, 0x20);
    blib::arc::arcs(fx, 0x01E0_15F0, D + 0x1464, OT, 2);
    blib::star::flash(fx, 0x01E0_32E0, D + 0x17A4, OT, 1);
    blib::sprite::bounce(fx, POOL, OT, 0x78);
    blib::beam::beads(fx, 0x01E0_4F60, D + 0x3910, OT, 0xA);
    blib::streak::ribbon(fx, 0x01E0_3E90, D + 0x3C80, OT, 2);
    blib::cross::bars(fx, 0x01E0_5D30, D + 0x17F0, OT, 4);
    let n = fx.mem.u16(D);
    if (0xE4..0xF9).contains(&n) {
        6
    } else if n == 0xF9 {
        2
    } else if n == 0xDA {
        1
    } else {
        0
    }
}
