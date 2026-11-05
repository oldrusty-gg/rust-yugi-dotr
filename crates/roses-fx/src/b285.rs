//! BMODULE b285 (card 483), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05D60, data area a0 = 0x1ED0000, 0x1060 bytes)
//! [F]: s0 = D, s1 = the OT (0x206E40), s2 the
//! parameter then a record pointer, s3 / s4 counters. Init: the handles
//! (+4 / +8), the attacker's rotation (+0x2C), textures (4, 0xF) and
//! (5, 0), nodes 0x20 (cleared, +0xC) / 0x1E (+0x10) / 0x1F (+0x14), the
//! random byte table (+0xF54). Every update the target's animated point
//! (0x1F65D0 kind 1) into sp + 0x60. Update 0x14 at node 0x1E (sp +
//! 0x70): the ring record (+0xEE8, `iris::rings`, 0x16 counts, eight
//! rings of kinds 0 / 4 / 2 / 2 / 5 / 0 / 4 / 2 at radii 80..300, alphas
//! 0x60 / 0x40, turned +-0x4D00 / +-0x80 by D+0x30 == 0x8000), two
//! flashes (+0x740, `star::flash_b`, 0x18 counts, 200 + 500 k wide), the
//! first spike (+0xBA8, `spike::spikes`, 0x16 counts, flags 5, 3.0 /
//! 40.0, +0x58.. = 0x100) and a shake (+0x40, 0x14, 1.8). Updates 0x28
//! and 0x66: the arc (+0xA0, `arc::arcs_b`, pool +0x100, 0x20 counts)
//! from node 0x20 to node 0x1E. Update 0x2D: a flash at node 0x1F (0x73
//! counts, 550 wide), the rod (+0xA98, `rod::rods_b`, 0x73, node 0x1E ->
//! 0x1F, 250 / 0.65 / 0.3) and a shake (0x6E, 1.5, kind 4 at node 0x1F).
//! Update 0x6E at node 0x20: five spikes (0x20 - 4 counts, flags 0x84,
//! 1.7 / 1.2 / 60, turned +-0x2000 / +-0x1800), three cones (+0xAD0,
//! `cone::cones_c`, 0x10 counts, delays 0 / 3 / 6), the second flash
//! (+0x78C, 0x20, 1000 wide) and eight bead beams (+0x7D8,
//! `beam::beads_at 34 3c`, 0x20 + rand & 3 counts, delays rand & 7, 30 +
//! 4 k long). Returns 2 on update 0x6A, 1 on 0x63, 8 on 0xA0 (no drawing
//! then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080 (0538257fe1fa),
//! `arcs_b` 0x1E003A0, `beads_at` 0x1E014C0, `flash_b` 0x1E02140,
//! `rods_b` 0x1E02BB0, `rings` 0x1E03B90, `spikes` 0x1E04A70, `cones_c`
//! 0x1E05550 [`crate::blib`].
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving};
use crate::blib::{self, exe::set_counter4};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SHAKE: u32 = D + 0x40;
const ARC: u32 = D + 0xA0;
const FLASH: u32 = D + 0x740;
const BEADS: u32 = D + 0x7D8;
const ROD: u32 = D + 0xA98;
const CONES: u32 = D + 0xAD0;
const SPIKES: u32 = D + 0xBA8;
const RINGS: u32 = D + 0xEE8;
const BYTES: u32 = D + 0xF54;

/// The entry 0x1E05D60 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp)
    };
    fx.leave(0x80);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    // 0x199208 memset(D, 0, 0x1060).
    fx.bzero(D, 0x1060);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    fx.texture(5, 0);
    for (at, node, clear) in [(0xCu32, 0x20, 1), (0x10, 0x1E, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The `sw` of +0x14 sits in rand_bytes' delay slot: the node's handle.
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x1F, 0);
    fx.mem.set32(D + 0x14, r);
    // The entry's s0 = D, s1 = 1 (the texture loop's), s2 = the parameter.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_5E50, [Some(D), Some(1), Some(param as u32)]);
    0
}

/// The camera shake (+0x40) [F]: `count` counts, `kind` (4 at node
/// `node`, 1 at `pos`), amplitudes `amp`, `far` / `rate`.
fn shake(fx: &mut Fx, count: i32, kind: u8, amp: [u32; 3], far: u32, pos: Result<u32, u32>) {
    set_counter4(fx, SHAKE, count, 0, 0xFF);
    match pos {
        Ok(_) => fx.mem.write_u8(SHAKE + 5, 0),
        Err(node) => {
            fx.mem.write_u8(SHAKE + 5, 1);
            let b = fx.mem.read_u8(node);
            fx.mem.write_u8(SHAKE + 6, b);
        }
    }
    fx.mem.write_u8(SHAKE + 4, kind);
    for (k, a) in amp.into_iter().enumerate() {
        fx.mem.write_u32(SHAKE + 0x14 + 4 * k as u32, a);
    }
    fx.mem.write_u32(SHAKE + 0x30, 0);
    fx.mem.write_u32(SHAKE + 0x34, far);
    fx.mem.write_u32(SHAKE + 0x50, 0x40C0_0000);
    if let Ok(p) = pos {
        copy_v4(fx, p, SHAKE + 0x20);
    }
}

/// Update 0x14 [F 0x1E05EC8]: the rings, two flashes, a spike and a shake
/// at node +0x10 (sp + 0x70). Returns the entry's (s2, s3).
fn rings(fx: &mut Fx, sp: u32) -> (u64, u64) {
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x10));
    node_position_saving(fx, h, s, sp + 0x70, u64::from(D), Some(u64::from(OT)), 0x01E0_5ED8);
    set_counter4(fx, RINGS, 0x16, 0, 0xFF);
    fx.mem.write_u8(RINGS + 5, 0);
    fx.mem.write_u8(RINGS + 0x10, 8);
    fx.mem.write_u8(RINGS + 0x11, 0x60);
    fx.mem.write_u8(RINGS + 0x12, 0x40);
    fx.mem.write_u32(RINGS + 0x34, 0x41C8_0000);
    fx.mem.write_u32(RINGS + 0x38, 0x4090_0000);
    copy_v4(fx, sp + 0x70, RINGS + 0x3C);
    if fx.mem.read_u32(D + 0x30) == 0x8000 {
        fx.mem.set32(RINGS + 0x4C, -0x4D00);
        fx.mem.set32(RINGS + 0x5C, -0x80);
    } else {
        fx.mem.set32(RINGS + 0x4C, 0x4D00);
        fx.mem.set32(RINGS + 0x5C, 0x80);
    }
    fx.mem.set32(RINGS + 0x60, -0x40);
    for (j, (kind, radius)) in [(0u8, 0x42A0_0000u32), (4, 0x4334_0000), (2, 0x42F0_0000), (2, 0x4396_0000), (5, 0x4348_0000), (0, 0x4302_0000), (4, 0x4316_0000), (2, 0x4270_0000)].into_iter().enumerate() {
        fx.mem.write_u8(RINGS + 8 + j as u32, kind);
        fx.mem.write_u32(RINGS + 0x14 + 4 * j as u32, radius);
    }
    // Two flashes (+0x740, 0x4C apart), 200 + 500 k wide.
    for k in 0..2i32 {
        let f = FLASH + 0x4C * k as u32;
        set_counter4(fx, f, 0x18, 0, 0xFF);
        fx.mem.write_u8(f + 5, 0);
        fx.mem.write_u8(f + 4, 0x81);
        fx.mem.write_u8(f + 7, 9);
        let w = ee::add(200.0, ee::mul(500.0, ee::from_i32(k)));
        fx.mem.setf(f + 0x10, w);
        fx.mem.write_u32(f + 0x14, 0);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xA, 0x80), (9, 0x80), (8, 0), (0xF, 0xC0)] {
            fx.mem.write_u8(f + o, v);
        }
        copy_v4(fx, RINGS + 0x3C, f + 0x1C);
    }
    // The first spike (+0xBA8).
    set_counter4(fx, SPIKES, 0x16, 0, 0xFF);
    fx.mem.write_u8(SPIKES + 4, 5);
    fx.mem.write_u32(SPIKES + 0x14, 0x4040_0000);
    fx.mem.write_u32(SPIKES + 0x1C, 0x4220_0000);
    for (o, v) in [(5u32, 0u8), (0xA, 0), (0xB, 2), (0xC, 3)] {
        fx.mem.write_u8(SPIKES + o, v);
    }
    // The `sb` of +8 sits in rand's delay slot: the constant 0xA0.
    fx.mem.write_u8(SPIKES + 8, 0xA0);
    let v = fx.rand_from(D, 0x01E0_60E0);
    fx.mem.set8(SPIKES + 9, v >> 4);
    fx.mem.write_u32(SPIKES + 0x10, BYTES);
    fx.mem.write_u32(SPIKES + 0x18, 0x3F80_0000);
    for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20), (0x27, 0x80)] {
        fx.mem.write_u8(SPIKES + o, v);
    }
    copy_v4(fx, RINGS + 0x3C, SPIKES + 0x28);
    for o in [0x58u32, 0x5C, 0x60] {
        fx.mem.write_u32(SPIKES + o, 0x100);
    }
    shake(fx, 0x14, 1, [0x3FE6_6666, 0x3FE6_6666, 0x3F80_0000], 0x4220_0000, Ok(RINGS + 0x3C));
    (u64::from(D + 0x98), 2)
}

/// Updates 0x28 and 0x66 [F 0x1E061C0]: the arc (+0xA0, pool +0x100 of
/// 8 + 1) from node +0xC to node +0x10.
fn arc(fx: &mut Fx) {
    set_counter4(fx, ARC, 0x20, 0, 0xFF);
    fx.mem.write_u8(ARC + 4, 1);
    fx.mem.write_u32(ARC + 8, D + 0x100);
    fx.mem.write_u8(ARC + 0x30, 0x64);
    fx.mem.write_u8(ARC + 0x31, 8);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(ARC + 0x32, b);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(ARC + 0x33, b);
    fx.mem.write_u32(ARC + 0x38, 0x3F80_0000);
    fx.mem.write_u8(ARC + 0x34, 0);
    fx.mem.write_u16(ARC + 0x36, 0);
    fx.mem.write_u32(ARC + 0x40, 0x4120_0000);
    for (o, v) in [(0x47u32, 0x80u8), (0x44, 0x80), (0x45, 0x80), (0x46, 0x80), (0x4A, 0x80), (0x49, 0x80), (0x48, 0x10)] {
        fx.mem.write_u8(ARC + o, v);
    }
    let mut t0 = 0;
    while t0 < fx.mem.u8(ARC + 0x31) + 1 {
        fx.mem.set16(ARC + 0x1E + 2 * t0 as u32, -1);
        fx.mem.set16(ARC + 0xC + 2 * t0 as u32, -1);
        t0 += 1;
    }
}

/// Update 0x2D [F 0x1E06280]: the first flash at node +0x14, the rod from
/// node +0x10 to +0x14 and a shake at node +0x14.
fn rod(fx: &mut Fx) {
    set_counter4(fx, FLASH, 0x73, 0, 0xFF);
    fx.mem.write_u8(FLASH + 5, 1);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(FLASH + 6, b);
    fx.mem.write_u8(FLASH + 4, 0x85);
    fx.mem.write_u8(FLASH + 7, 6);
    fx.mem.write_u32(FLASH + 0x10, 0x4409_8000);
    fx.mem.write_u32(FLASH + 0x14, 0);
    for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x80), (8, 0)] {
        fx.mem.write_u8(FLASH + o, v);
    }
    set_counter4(fx, ROD, 0x73, 0, 0xFF);
    fx.mem.write_u8(ROD + 5, 1);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(ROD + 8, b);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(ROD + 9, b);
    fx.mem.write_u8(ROD + 4, 2);
    fx.mem.write_u32(ROD + 0x10, 0x3F26_6666);
    fx.mem.write_u32(ROD + 0x14, 0x3E99_999A);
    fx.mem.write_u32(ROD + 0xC, 0x437A_0000);
    for (o, v) in [(0x1Bu32, 0xC0u8), (0x18, 0xC0), (0x19, 0xC0), (0x1A, 0xC0), (0x1E, 0x80), (0x1D, 0x80), (0x1C, 0), (0x1F, 0)] {
        fx.mem.write_u8(ROD + o, v);
    }
    shake(fx, 0x6E, 4, [0x3FC0_0000, 0x3FC0_0000, 0x3F80_0000], 0x42A0_0000, Err(D + 0x14));
}

/// Update 0x6E [F 0x1E063B8]: the strike at node +0xC (sp + 0x70): five
/// spikes, three cones, the second flash and eight bead beams. Returns the
/// entry's (s2, s3, s4).
fn strike(fx: &mut Fx, sp: u32) -> (u64, u64, u64) {
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
    node_position_saving(fx, h, s, sp + 0x70, u64::from(D), Some(u64::from(OT)), 0x01E0_63C8);
    for s2 in 0..5u32 {
        let q = SPIKES + 0x68 * s2;
        set_counter4(fx, q, 0x20, 0, 0xFF);
        let c = fx.mem.read_u8(q).wrapping_sub(4);
        fx.mem.write_u8(q, c);
        for (o, v) in [(4u32, 0x84u8), (5, 0), (0xA, 0), (0xB, 2), (0xC, 3)] {
            fx.mem.write_u8(q + o, v);
        }
        // The `sb` of +8 sits in rand's delay slot: the constant 0x23.
        fx.mem.write_u8(q + 8, 0x23);
        let v = fx.rand_from(D, 0x01E0_641C);
        fx.mem.set8(q + 9, v >> 4);
        fx.mem.write_u32(q + 0x1C, 0x4270_0000);
        fx.mem.write_u32(q + 0x10, BYTES);
        fx.mem.write_u32(q + 0x14, 0x3FD9_999A);
        fx.mem.write_u32(q + 0x18, 0x3F99_999A);
        for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20), (0x27, 0x80)] {
            fx.mem.write_u8(q + o, v);
        }
        copy_v4(fx, sp + 0x70, q + 0x28);
        fx.mem.write_u32(q + 0x48, 0);
        fx.mem.write_u32(q + 0x50, 0);
        fx.mem.write_u32(q + 0x4C, 0);
        match s2 {
            1 => fx.mem.set32(q + 0x48, 0x2000),
            2 => fx.mem.set32(q + 0x48, -0x2000),
            3 => fx.mem.set32(q + 0x50, 0x1800),
            4 => fx.mem.set32(q + 0x50, -0x1800),
            _ => {}
        }
    }
    // Three cones (+0xAD0, 0x48 apart), delayed 0 / 3 / 6.
    for s4 in 0..3u32 {
        let r = CONES + 0x48 * s4;
        set_counter4(fx, r, 0x10, (3 * s4) as i32 & 0xFF, 0xFF);
        for (o, v) in [(4u32, 0u8), (5, 0), (7, 9), (8, 0)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0xC, 0x4040_0000);
        fx.mem.write_u32(r + 0x10, 0x3E4C_CCCD);
        fx.mem.write_u32(r + 0x14, 0x3F00_0000);
        fx.mem.write_u32(r + 0x18, 0);
        fx.mem.write_u32(r + 0x1C, 0x3E0F_5C29);
        for (o, v) in [(0x23u32, 0x80u8), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x26, 0x80), (0x25, 0x60), (0x24, 0), (0x27, 0)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, sp + 0x70, r + 0x28);
    }
    // The second flash (+0x78C), 1000 wide.
    let f = FLASH + 0x4C;
    set_counter4(fx, f, 0x20, 0, 0xFF);
    fx.mem.write_u8(f + 5, 0);
    fx.mem.write_u8(f + 4, 0x81);
    fx.mem.write_u8(f + 7, 9);
    for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x10), (8, 0x30)] {
        fx.mem.write_u8(f + o, v);
    }
    fx.mem.write_u32(f + 0x14, 0);
    fx.mem.write_u32(f + 0x10, 0x447A_0000);
    copy_v4(fx, sp + 0x70, f + 0x1C);
    // Eight bead beams (+0x7D8, 0x58 apart).
    let mut s4 = 0u64;
    for s3 in 0..8i32 {
        let b = BEADS + 0x58 * s3 as u32;
        let v = fx.rand_from(D, 0x01E0_666C);
        let count = (((v >> 4) & 3) + 0x20) & 0xFF;
        s4 = count as u64;
        let v = fx.rand_from(D, 0x01E0_6680);
        set_counter4(fx, b, count, (v >> 4) & 7, 0xFF);
        fx.mem.write_u8(b + 4, 0xC);
        fx.mem.write_u8(b + 5, 0);
        // The `sb` of +8 sits in rand's delay slot: the constant 0x32.
        fx.mem.write_u8(b + 8, 0x32);
        let v = fx.rand_from(D, 0x01E0_66B0);
        fx.mem.set8(b + 9, v >> 4);
        fx.mem.write_u8(b + 0xA, 0);
        fx.mem.write_u8(b + 0xC, 2);
        fx.mem.write_u8(b + 0xB, 3);
        let w = ee::add(30.0, ee::mul(4.0, ee::from_i32(s3)));
        fx.mem.setf(b + 0x1C, w);
        fx.mem.write_u32(b + 0x20, 0);
        fx.mem.write_u32(b + 0x10, BYTES);
        fx.mem.write_u32(b + 0x14, 0x4000_0000);
        fx.mem.write_u32(b + 0x18, 0x40C0_0000);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(b + o, 0x80);
        }
        copy_v4(fx, sp + 0x70, b + 0x28);
    }
    (u64::from(D + 0x2C0), 8, s4)
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, sp + 0x60);
    // 0x1F65D0's 0x50 frame (s0 = D, s1 = OT, s2 = the parameter, s3 (the
    // runner's 0), ra) [F].
    fx.saved(sp - 0x50, &[(0, u64::from(D)), (0x10, u64::from(OT)), (0x20, u64::MAX), (0x30, 0), (0x40, 0x01E0_5E74)]);
    // The entry's s2 / s3 / s4 after the block (the parameter -1, the
    // runner's 0 / 0, else the last record pointer / counters) [F]: the
    // library prologues save them.
    let (s2, s3, s4): (u64, u64, u64) = match fx.mem.u16(D) {
        0xA0 => return 8,
        0x6E => strike(fx, sp),
        0x2D => {
            rod(fx);
            (u64::MAX, 0, 0)
        }
        0x66 | 0x28 => {
            arc(fx);
            (u64::MAX, 0, 0)
        }
        0x14 => {
            let (s2, s3) = rings(fx, sp);
            (s2, s3, 0)
        }
        _ => (u64::MAX, 0, 0),
    };
    let _ = (s2, s3, s4);
    shake_saving(fx, SHAKE, u64::from(D), 0x01E0_6774, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    blib::arc::arcs_b(fx, 0x01E0_03A0, ARC, OT, 1);
    blib::beam::beads_at(fx, 0x01E0_14C0, 0x34, 0x3C, BEADS, OT, 8);
    blib::star::flash_b(fx, 0x01E0_2140, FLASH, OT, 2);
    // [H] f22 = the runner's (0): the entry never sets it.
    blib::rod::rods_b(fx, 0x01E0_2BB0, ROD, OT, 1, 0.0);
    blib::iris::rings(fx, 0x01E0_3B90, RINGS, OT, 1);
    blib::spike::spikes(fx, 0x01E0_4A70, SPIKES, OT, 8);
    blib::cone::cones_c(fx, 0x01E0_5550, CONES, OT, 3);
    match fx.mem.u16(D) {
        0x6A => 2,
        0x63 => 1,
        _ => 0,
    }
}
