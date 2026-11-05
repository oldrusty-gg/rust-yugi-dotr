//! BMODULE b137 (card 166), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E063D0, data area a0 = 0x1ED0000, 0x1BB0 bytes)
//! [F]: s4 = D, s0 = the OT (0x206E40), s1 the
//! parameter then counters, s2 / s3 pointers / counters. Init sets the
//! spark-emitter group (+0xA0, `spawn::emitters` / `sprite::sparks`: its
//! two update lists the record's 0x1E06D88 / 0x1E06D90 at nodes 0xC /
//! 0x54, node 0x54 again over updates 124..136). Update 4 the attacker's
//! streaks (+0xB3C) and eight bead beams (+0xB74, `beam::beads_at`) on
//! node 1, delayed 8..11; 9 a rod (+0xB04, `rod::rods_b`) and an arc
//! (+0xAA4, `arc::arcs_b`, pool +0x12E0) between nodes 0x1B / 0x1C; 124
//! the rod's mode 2 and width 90; 149 a bead emitter (+0x1254,
//! `emit::bead_trails_d`) from node 0x1B feeding the beams; 152 the fade
//! (+0x1AA0) and a shake at the target (sp + 0x60); 153 ten spikes
//! (+0x1420) at node 0x1B turned -0x600 + 0x800 k. Returns 6 on updates
//! 152..180, 2 on 149, 1 on 142, 8 on 260 (no drawing then) and at once
//! for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `sparks`
//! 0x1E003A0, `fan_ring` 0x1E00C70, `emitters` 0x1E00F40, `arcs_b`
//! 0x1E01390, `rods_b` 0x1E024B0, `streaks_b` 0x1E03490, `beads_at`
//! 0x1E04290, `bead_trails_d` 0x1E04F10, `spikes` 0x1E05420, `fade`
//! 0x1E05F00 [`crate::blib`].
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const GROUP: u32 = D + 0xA0;
const SPARKS: u32 = D + 0xE4;
const ARC: u32 = D + 0xAA4;
const ROD: u32 = D + 0xB04;
const STREAKS: u32 = D + 0xB3C;
const BEADS: u32 = D + 0xB74;
const EMIT: u32 = D + 0x1254;
const SPIKES: u32 = D + 0x1420;
const FADE: u32 = D + 0x1AA0;
const BYTES: u32 = D + 0x1AA8;

/// The entry 0x1E063D0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp)
    };
    fx.leave(0x70);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    // 0x199208 memset(D, 0, 0x1BB0).
    fx.bzero(D, 0x1BB0);
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
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0xC, 0), (0x14, 0x54, 0), (0x18, 0x1B, 0), (0x1C, 0x1C, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The emitter group (+0xA0).
    let g = GROUP;
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(g + 0x28, b);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(g + 0x29, b);
    fx.mem.write_u32(g + 8, 0x01E0_6D88);
    fx.mem.write_u32(g + 0xC, 0x01E0_6D90);
    fx.mem.write_u32(g, SPARKS);
    fx.mem.write_u8(g + 0x30, 0x10);
    fx.mem.write_u32(g + 4, D + 0x464);
    fx.mem.write_u8(g + 0x31, 0x28);
    fx.mem.write_u8(g + 0x32, 0);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(g + 0x33, b);
    fx.mem.set16(g + 0x34, 0x7C);
    fx.mem.set16(g + 0x36, 0x88);
    fx.mem.write_u32(g + 0x38, 0x4348_0000);
    fx.mem.write_u32(g + 0x3C, 0x433E_0000);
    fx.mem.write_u32(g + 0x40, 0x4120_0000);
    // The entry's s0 = 3 (the texture loop's), s1 = the parameter, s2 the
    // runner's [H] left.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_6564, [Some(3), Some(param as u32), None]);
    0
}

/// rand() >> 4 with the entry's s0 (the OT) saved in its frame.
fn rand4(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(OT, ra) >> 4
}

/// node_position(self, D[at], out) with the entry's s0 (the OT) / s1.
fn node_at(fx: &mut Fx, at: u32, out: u32, s1: u64, ra: u32) {
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + at));
    node_position_saving(fx, h, s, out, u64::from(OT), Some(s1), ra);
}

/// Update 4 [F 0x1E065E8]: the attacker's streaks and eight bead beams on
/// node 1.
fn streaks(fx: &mut Fx) {
    let s = STREAKS;
    set_counter4(fx, s, 0x1C, 0, 0xFF);
    for (o, v) in [(4u32, 0u8), (9, 2), (0xA, 0xFF), (8, 8), (0xB, 0x18)] {
        fx.mem.write_u8(s + o, v);
    }
    let h = fx.mem.read_u32(D + 4);
    fx.mem.write_u32(s + 0xC, h);
    for (o, v) in [(0x14u32, 0x80u8), (0x15, 0x80), (0x16, 0x80), (0x17, 0), (0x10, 0x90), (0x11, 0x90), (0x12, 0x90), (0x13, 0x40), (5, 1)] {
        fx.mem.write_u8(s + o, v);
    }
    let v = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(s + 6, v);
    for s1 in 0..8u32 {
        let b = BEADS + 0x58 * s1;
        let delay = ((rand4(fx, 0x01E0_6670) & 3) + 8) & 0xFF;
        set_counter4(fx, b, 0x1E, delay, 0xFF);
        let c = fx.mem.read_u8(b).wrapping_sub(4);
        fx.mem.write_u8(b, c);
        for (o, v) in [(4u32, 5u8), (5, 0), (8, 0x50), (9, s1 as u8), (0xA, 1), (0xC, 2)] {
            fx.mem.write_u8(b + o, v);
        }
        let v = rand4(fx, 0x01E0_66C8) & 3;
        fx.mem.set8(b + 0xB, v);
        fx.mem.write_u32(b + 0x1C, 0x41A0_0000);
        fx.mem.write_u32(b + 0x20, 0);
        fx.mem.write_u32(b + 0x10, BYTES);
        fx.mem.write_u32(b + 0x14, 0x3FC0_0000);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(b + o, 0x80);
        }
        node_at(fx, 0xC, b + 0x28, u64::from(s1), 0x01E0_6714);
    }
}

/// Update 9 [F 0x1E06730]: the rod and the arc between nodes 0x1B / 0x1C.
fn start(fx: &mut Fx) {
    let r = ROD;
    set_counter4(fx, r, 0xD8, 0, 0xFF);
    fx.mem.write_u8(r + 5, 1);
    let v = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(r + 8, v);
    let v = fx.mem.read_u8(D + 0x1C);
    fx.mem.write_u8(r + 9, v);
    fx.mem.write_u8(r + 4, 0x82);
    fx.mem.write_u32(r + 0x10, 0x3DF5_C28F);
    fx.mem.write_u32(r + 0x14, 0x3D4C_CCCD);
    fx.mem.write_u32(r + 0xC, 0x4270_0000);
    for (o, v) in [(0x18u32, 0x80u8), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0xC0), (0x1E, 0xC0), (0x1D, 0x20), (0x1C, 0x10), (0x1F, 0x10)] {
        fx.mem.write_u8(r + o, v);
    }
    let a = ARC;
    set_counter4(fx, a, 0xA4, 0, 0xFF);
    fx.mem.write_u8(a + 4, 1);
    fx.mem.write_u32(a + 8, D + 0x12E0);
    fx.mem.write_u8(a + 0x30, 0x14);
    fx.mem.write_u8(a + 0x31, 8);
    let v = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(a + 0x32, v);
    let v = fx.mem.read_u8(D + 0x1C);
    fx.mem.write_u8(a + 0x33, v);
    fx.mem.write_u32(a + 0x38, 0x3F8C_CCCD);
    fx.mem.write_u8(a + 0x34, 0);
    fx.mem.write_u16(a + 0x36, 0);
    fx.mem.write_u32(a + 0x40, 0x4120_0000);
    for (o, v) in [(0x44u32, 0x80u8), (0x45, 0x80), (0x46, 0x80), (0x47, 0xC0), (0x4A, 0xC0), (0x49, 0x20), (0x48, 0x10)] {
        fx.mem.write_u8(a + o, v);
    }
    let len = fx.mem.u8(a + 0x31) as u32;
    for k in 0..=len {
        fx.mem.set16(a + 0x1E + 2 * k, -1);
        fx.mem.set16(a + 0xC + 2 * k, -1);
    }
}

/// Update 149 [F 0x1E06894]: the bead emitter from node 0x1B.
fn emitter(fx: &mut Fx) {
    let e = EMIT;
    set_counter4(fx, e, 9, 0, 0xFF);
    fx.mem.write_u8(e + 4, 2);
    fx.mem.write_u8(e + 5, 1);
    let v = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(e + 6, v);
    fx.mem.write_u32(e + 8, BEADS);
    for (o, v) in [(0x10u32, 0x14u8), (0x12, 0x1E), (0x11, 0x20), (0x13, 2), (0x16, 2), (0x14, 3)] {
        fx.mem.write_u8(e + o, v);
    }
    fx.mem.write_u32(e + 0x18, 0x4000_0000);
    fx.mem.write_u32(e + 0x1C, 0x3ECC_CCCD);
    fx.mem.write_u32(e + 0x20, 0x4270_0000);
    fx.mem.write_u32(e + 0xC, BYTES);
    for o in [0x2Bu32, 0x28, 0x29, 0x2A] {
        fx.mem.write_u8(e + o, 0x80);
    }
    node_at(fx, 0x18, e + 0x3C, -0x600i64 as u64, 0x01E0_6934);
    copy_v4(fx, e + 0x3C, e + 0x4C);
}

/// Update 152 [F 0x1E0695C]: the fade and a shake at the target.
fn strike(fx: &mut Fx, sp: u32) {
    set_counter4(fx, FADE, 2, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 4);
    set_counter4(fx, D + 0x40, 0x18, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 0);
    fx.mem.write_u8(D + 0x44, 2);
    fx.mem.write_u32(D + 0x54, 0x4000_0000);
    fx.mem.write_u32(D + 0x58, 0x3FE6_6666);
    fx.mem.write_u32(D + 0x5C, 0x3F80_0000);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x42A0_0000);
    fx.mem.write_u32(D + 0x90, 0x40C0_0000);
    copy_v4(fx, sp + 0x60, D + 0x60);
}

/// Update 153 [F 0x1E069F0]: ten spikes at node 0x1B turned -0x600 +
/// 0x800 k (negated when D[0x30] is 0x8000), delayed k; the entry's s2
/// after it (the last spike's D + 0x68 i, else 0).
fn spikes(fx: &mut Fx) -> u64 {
    let mut s2 = 0u64;
    let mut s1 = -0x600i32;
    for s3 in 0..10i32 {
        let i = first_free(fx, SPIKES, 0x68, 0x10);
        if i >= 0 {
            let q = SPIKES + 0x68 * i as u32;
            s2 = u64::from(D + 0x68 * i as u32);
            set_counter4(fx, q, 0x14, s3 & 0xFF, 0xFF);
            let c = fx.mem.read_u8(q).wrapping_sub(4);
            fx.mem.write_u8(q, c);
            for (o, v) in [(4u32, 4u8), (0x23, 0x40), (0x27, 0x40), (5, 0), (0xA, 1), (0xB, 2), (0xC, 5), (8, 0x32)] {
                fx.mem.write_u8(q + o, v);
            }
            let x = rand4(fx, 0x01E0_6A80);
            fx.mem.set8(q + 9, x);
            fx.mem.write_u32(q + 0x10, BYTES);
            for (o, v) in [(0x24u32, 0x80u8), (0x26, 0x80), (0x20, 0x80), (0x22, 0x80), (0x21, 0x40), (0x25, 0x40)] {
                fx.mem.write_u8(q + o, v);
            }
            node_at(fx, 0x18, q + 0x28, s1 as i64 as u64, 0x01E0_6AC0);
            fx.mem.write_u32(q + 0x1C, 0x4248_0000);
            let w = ee::add(3.0, ee::mul(0.5, ee::from_i32(s3)));
            fx.mem.setf(q + 0x14, w);
            fx.mem.write_u32(q + 0x18, 0x3FB3_3333);
            fx.mem.set32(q + 0x48, s1);
            if fx.mem.read_u32(D + 0x30) == 0x8000 {
                fx.mem.set32(q + 0x48, s1.wrapping_neg());
            }
        }
        s1 += 0x800;
    }
    s2
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(OT), 0x01E0_6584);
    // The entry's s1 / s2 / s3 at the library calls: the dispatch's delay
    // slot sets s1 = -0x600 [F 0x1E0659C]; the runner's registers are 0.
    let (mut s1, mut s2, mut s3) = (-0x600i64 as u64, 0u64, 0u64);
    match fx.mem.u16(D) {
        0x104 => return 8,
        0x99 => {
            s2 = spikes(fx);
            (s1, s3) = (0x4A00, 10);
        }
        0x98 => strike(fx, sp),
        0x95 => emitter(fx),
        0x7C => {
            fx.mem.write_u8(ROD + 4, 2);
            fx.mem.write_u32(ROD + 0xC, 0x42B4_0000);
        }
        9 => start(fx),
        4 => {
            streaks(fx);
            (s1, s2) = (8, u64::from(D + 0x58 * 8));
        }
        _ => {}
    }
    let saves = |ra: u32| [(0u32, u64::from(OT)), (0x10, s1), (0x20, s2), (0x30, s3), (0x40, u64::from(D)), (0x50, 0), (0x60, 0), (0x70, 0), (0x80, 0), (0x90, u64::from(ra))];
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_6B50, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    blib::spawn::emitters(fx, 0x01E0_0F40, GROUP, n);
    // emitters' 0xC0 frame saves the entry's s0..s6 (s4 = D) and ra; s4's
    // slot is the streaks' vertex 1 XYZ low word, their first TEXFLUSH's
    // a0 [run].
    fx.saved(sp - 0xC0, &saves(0x01E0_6B5C)[..7]);
    fx.saved(sp - 0xC0, &[(0x70, 0x01E0_6B5C)]);
    blib::sprite::sparks(fx, SPARKS, OT, 0x10);
    blib::arc::arcs_b(fx, 0x01E0_1390, ARC, OT, 1);
    // [H] f22 = the runner's (0): the entry never sets it.
    blib::rod::rods_b(fx, 0x01E0_24B0, ROD, OT, 1, 0.0);
    // rods_b's 0x1F0 frame saves s0..s7, fp (+0x10..) and ra: s1..s4's
    // slots are the w words of the bead emitter's path samples, copied
    // into the beads [run, sandbox store watch].
    let r = saves(0x01E0_6B8C);
    fx.saved(sp - 0x1F0 + 0x10, &r);
    blib::radiate::streaks_b(fx, 0x01E0_3490, STREAKS, OT, 1);
    // streaks_b keeps two vertex pointers at its sp + 0x110 / + 0x120 (sp
    // + 0x26A / + 0x282) [F 0x1E034F0]: the w words of the bead emitter's
    // last two path samples [run, sandbox store watch].
    let f = sp - 0x2C0;
    fx.mem.write_u32(f + 0x110, f + 0x26A);
    fx.mem.write_u32(f + 0x120, f + 0x282);
    blib::emit::bead_trails_d(fx, 0x01E0_4F10, EMIT, 1);
    blib::beam::beads_at(fx, 0x01E0_4290, 0x34, 0x3C, BEADS, OT, 0x14);
    blib::spike::spikes(fx, 0x01E0_5420, SPIKES, OT, 0x10);
    blib::fade::fade(fx, FADE, OT);
    let n = fx.mem.u16(D);
    if n < 0x98 {
        if n == 0x95 {
            2
        } else if n == 0x8E {
            1
        } else {
            0
        }
    } else if n < 0xB5 {
        6
    } else {
        0
    }
}
