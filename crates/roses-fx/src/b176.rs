//! BMODULE b176 (card 557), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04E50, data area a0 = 0x1ED0000) [F] (helper F12,
//! 3 Nov 2026): b076's library family (s2 = D, s3 = the OT, s0 the
//! parameter). Every update node bursts (+0xA0, motes +0x324 / +0xB24),
//! four motes (+0x324), 24 cone fans (+0x13CC, the local
//! [`cones_tri`]), three cones (+0x12F4), six arc groups (+0xE4), 24 spoke
//! groups (+0x1AEC) and twelve bead groups (+0x23EC, the local [`beads`]).
//! The cone fans are started / changed by [`fans`] (0x1E04A40) on updates
//! 0x3B / 0x41 / 0x43 / 0x49 / 0x51 / 0x56 / 0x82; on 0x57 the arcs, on
//! 0x63 the shake, the spokes and the beads at the target. Nodes
//! re-requested on 0x24 and 0x99. Returns 1 on 0x59, 6 on 0x63..0x7A, 2 on
//! 0x7B, 8 on 0x91 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `arcs` 0x1E015F0, `cones`
//! 0x1E031B0, `spokes::lines` 0x1E03A20 [`crate::blib`]; record-local
//! 6d1384771e58 0x1E02870 ([`cones_tri`]), 5efd9b9d205d 0x1E03F40
//! ([`beads`]), 4c02f6d018d5 0x1E04A40 ([`fans`]).
use crate::b076::{copy_v4, node_position_saving, rand_bytes, request_node_saving, shake_saving};
use crate::blib::{
    self,
    exe::{aim_angles, alpha_mode, atlas_cell, camera_vp_into, countdown, first_free, libm_cosf, mul_mem, node_source, rot_trans_into, set_counter, set_counter4, tex_rect, tex_rect_strip},
};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const PI: u32 = 0x4049_0FDB;
/// RGBAQ / ST / XYZF2 x 4 (a textured strip).
const REGS_TEX4: u64 = 0x4214_2142_1421;
/// 1.33 (0x3FAA3D71) as the code loads it.
const ASPECT: u32 = 0x3FAA_3D71;

/// The entry 0x1E04E50 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x90);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x27F0);
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
    request_nodes(fx, i64::from(param) as u64, &[(0xC, 0x53, 1, 0x01E0_4F38), (0x10, 0x47, 0, 0x01E0_4F50), (0x14, 0x3D, 0, 0x01E0_4F68), (0x18, 0x32, 0, 0x01E0_4F80)]);
    for (k, at) in [0xCu32, 0x10, 0x14, 0x18].into_iter().enumerate() {
        let b = fx.mem.read_u8(D + at);
        fx.mem.write_u8(D + 0xC8 + k as u32, b);
    }
    for (k, spec) in [0x01E0_5BD0u32, 0x01E0_5BD8, 0x01E0_5BE0, 0x01E0_5BE8].into_iter().enumerate() {
        fx.mem.write_u32(D + 0xA8 + 4 * k as u32, spec);
    }
    fx.mem.write_u32(D + 0xA0, D + 0x324);
    fx.mem.write_u8(D + 0xD0, 8);
    fx.mem.write_u32(D + 0xA4, D + 0xB24);
    fx.mem.write_u8(D + 0xD1, 0x32);
    fx.mem.write_u8(D + 0xD2, 0);
    fx.mem.set16(D + 0xD4, 0);
    fx.mem.write_u32(D + 0xD8, 0x4320_0000);
    fx.mem.write_u32(D + 0xDC, 0);
    fx.mem.write_u32(D + 0xE0, 0x40C0_0000);
    // The entry's s0 = the parameter, s1 the runner's 0, s2 = D.
    rand_bytes(fx, D + 0x26EC, 0x100, 0x01E0_5020, [Some(param as u32), Some(0), Some(D)]);
    0
}

/// 0x1F6770 per (slot, node, clear, return address), leaving its frame
/// (the entry's s0 = `s0`, s1 the runner's 0, s2 = D) [run: on 0x24 its s1
/// save's upper half clears the motes' later missing-texture TEX0].
fn request_nodes(fx: &mut Fx, s0: u64, list: &[(u32, i32, i32, u32)]) {
    for &(at, node, clear, ra) in list {
        let h = fx.mem.i32(D + 4);
        let r = request_node_saving(fx, h, node, clear, [Some(s0), Some(0), Some(u64::from(D))], ra);
        fx.mem.set32(D + at, r);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, sp + 0x80);
    let mut s0 = i64::from(param) as u64;
    match fx.mem.u16(D) {
        0x91 => return 8,
        0x82 => {
            for a in 1..7 {
                fans(fx, a, 0, s0, 0x01E0_57D4);
            }
        }
        0x63 => s0 = strike(fx, sp),
        0x57 => s0 = arcs_start(fx),
        0x56 => {
            fans(fx, 1, 2, s0, 0x01E0_5220);
            fans(fx, 3, 2, s0, 0x01E0_5234);
        }
        0x51 => {
            fans(fx, 1, 3, s0, 0x01E0_51F0);
            fans(fx, 3, 3, s0, 0x01E0_5204);
        }
        0x49 => {
            fans(fx, 6, 2, s0, 0x01E0_51C0);
            fans(fx, 4, 2, s0, 0x01E0_51D4);
        }
        0x43 => {
            for (a, b, ra) in [(4, 1, 0x01E0_5154), (4, 3, 0x01E0_5168), (6, 1, 0x01E0_517C), (6, 3, 0x01E0_5190), (3, 1, 0x01E0_51A4)] {
                fans(fx, a, b, s0, ra);
            }
        }
        0x41 => fans(fx, 5, 2, s0, 0x01E0_5138),
        0x3B => {
            for (a, b, ra) in [(1, 1, 0x01E0_50E0), (2, 1, 0x01E0_50F4), (5, 1, 0x01E0_5108), (5, 3, 0x01E0_511C)] {
                fans(fx, a, b, s0, ra);
            }
        }
        _ => {}
    }
    shake_saving(fx, D + 0x40, s0, 0x01E0_5804, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let frame = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA0, frame);
    blib::mote::motes(fx, D + 0x324, OT, 4);
    cones_tri(fx, D + 0x13CC, OT, 0x18);
    blib::cone::cones(fx, 0x01E0_31B0, D + 0x12F4, OT, 3);
    blib::arc::arcs(fx, 0x01E0_15F0, D + 0xE4, OT, 6);
    blib::spokes::lines(fx, 0x01E0_3A20, D + 0x1AEC, OT, 0x18);
    beads(fx, D + 0x23EC, OT, 0xC);
    match fx.mem.u16(D) {
        0x99 => request_nodes(fx, s0, &[(0xC, 0x53, 1, 0x01E0_5974), (0x10, 0x47, 0, 0x01E0_598C), (0x14, 0x3D, 0, 0x01E0_59A4), (0x18, 0x32, 0, 0x01E0_59BC)]),
        0x24 => request_nodes(fx, s0, &[(0xC, 2, 1, 0x01E0_58C4), (0x10, 0xB, 0, 0x01E0_58DC), (0x14, 0x10, 0, 0x01E0_58F4), (0x18, 0xC, 0, 0x01E0_590C), (0x1C, 0xF, 0, 0x01E0_5924), (0x20, 0xD, 0, 0x01E0_593C), (0x24, 0xE, 0, 0x01E0_5954)]),
        _ => {}
    }
    match fx.mem.u16(D) {
        0x63..=0x7A => 6,
        0x7B => 2,
        0x59 => 1,
        _ => 0,
    }
}

/// Update 0x57 [F 0x1E0523C]: six arc groups (+0xE4 + 0x60 k, points
/// +0x4E4) between nodes +0x10 + 4 k and +0xC / +0x14 + 4 k (odd / even
/// k). Returns the entry's s0 after (0x60 x the last k).
fn arcs_start(fx: &mut Fx) -> u64 {
    for k in 0..6u32 {
        let g = D + 0xE4 + 0x60 * k;
        set_counter4(fx, g, 0x1C, 0, 0xFF);
        fx.mem.write_u8(g + 4, 0xE);
        fx.mem.write_u32(g + 8, D + 0x4E4);
        fx.mem.write_u8(g + 0x30, 0x64);
        fx.mem.write_u8(g + 0x31, 6);
        let b = fx.mem.read_u8(D + 0x10 + 4 * k);
        fx.mem.write_u8(g + 0x32, b);
        let other = if k & 1 != 0 { 0xC } else { 0x14 };
        let b = fx.mem.read_u8(D + other + 4 * k);
        fx.mem.write_u8(g + 0x33, b);
        fx.mem.write_u32(g + 0x38, 0x3F99_999A);
        fx.mem.write_u32(g + 0x3C, 0x4080_0000);
        fx.mem.write_u32(g + 0x40, 0x4270_0000);
        for (o, v) in [(0x4Du32, 0x30u8), (0x4E, 0x30), (0x4C, 0x60), (0x4F, 0x40)] {
            fx.mem.write_u8(g + o, v);
        }
        fx.mem.write_u32(g + 0x54, 0x3F4C_CCCD);
        let mut j = 0u32;
        while j < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
            fx.mem.set16(g + 0x1E + 2 * j, -1);
            fx.mem.set16(g + 0xC + 2 * j, -1);
            j += 1;
        }
    }
    0x60 * 5
}

/// rand() with the caller's s0 = `s0` (64 bits) from return address `ra`.
fn rand64(fx: &mut Fx, s0: u64, ra: u32) -> i32 {
    let f = fx.sp - 0x10;
    fx.mem.write_u64(f, s0);
    fx.mem.write_u64(f + 8, u64::from(ra));
    fx.rand()
}

/// Update 0x63 [F 0x1E0537C]: the camera shake (+0x40), 24 spoke groups
/// (+0x1AEC + 0x60 k, 4 x 6) and twelve bead groups (+0x23EC + 0x40 k)
/// each taking 4 particles of +0xB24 round the target (sp + 0x80).
/// Returns the entry's s0 after.
fn strike(fx: &mut Fx, sp: u32) -> u64 {
    let s = D + 0x40;
    set_counter4(fx, s, 0x20, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 8);
    fx.mem.write_u32(s + 0x14, 0x3FC0_0000);
    fx.mem.write_u32(s + 0x18, 0x3FC0_0000);
    fx.mem.write_u32(s + 0x1C, 0x4000_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, sp + 0x80, D + 0x60);
    for i in 0..4i32 {
        for j in 0..6i32 {
            let off = 0x60 * (j + 6 * i) as u32;
            let g = D + 0x1AEC + off;
            set_counter4(fx, g, 0x14, (j << 1) & 0xFF, 0xFF);
            fx.mem.write_u8(g + 5, 0);
            copy_v4(fx, sp + 0x80, g + 0x20);
            fx.mem.write_u8(g + 8, 0x2D);
            let r = rand64(fx, u64::from(off), 0x01E0_547C);
            fx.mem.set8(g + 9, r >> 4);
            fx.mem.write_u8(g + 0xA, 3);
            fx.mem.write_u32(g + 0xC, D + 0x26EC);
            fx.mem.write_u32(g + 0x10, 0x4000_0000);
            fx.mem.write_u32(g + 0x14, 0x3E99_999A);
            for (o, v) in [(0x19u32, 0x80u8), (0x1A, 0x80), (0x18, 0x20), (0x1B, 0xC0), (0x1E, 0x80), (0x1D, 0x20), (0x1C, 0x10)] {
                fx.mem.write_u8(g + o, v);
            }
            let base = (i << 14) + 0x2000;
            let turn = if fx.mem.read_u32(D + 0x30) == 0x8000 { base - 0xA00 * j } else { base + 0xA00 * j };
            fx.mem.set32(g + 0x48, turn);
        }
    }
    let mut s0 = 0u64;
    for i in 0..12i32 {
        let off = 0x40 * i as u32;
        s0 = u64::from(off);
        let g = D + 0x23EC + off;
        set_counter4(fx, g, 1, (i << 1) & 0xFF, 0xFF);
        fx.mem.write_u8(g + 5, 0);
        fx.mem.write_u8(g + 4, 0xC0);
        fx.mem.write_u32(g + 8, D + 0xB24);
        for (o, v) in [(0x1Cu32, 0x32u8), (0x1D, 4), (0x1E, 2), (0x1F, 1), (0x20, 0), (0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x27, 0x80)] {
            fx.mem.write_u8(g + o, v);
        }
        fx.mem.write_u32(g + 0x28, 0x4320_0000);
        fx.mem.write_u32(g + 0x2C, 0);
        let mut k = 0u32;
        while (k as i32) < fx.mem.u8(g + 0x1D) {
            let idx = first_free(fx, D + 0xB24, 0x28, 0x32);
            if idx < 0 {
                fx.mem.set16(g + 0xC + 2 * k, -1);
                k += 1;
                continue;
            }
            fx.mem.set16(g + 0xC + 2 * k, idx);
            let p = D + 0xB24 + 0x28 * idx as u32;
            let r = rand64(fx, s0, 0x01E0_5634);
            set_counter(fx, p, 7, (r >> 4) & 7);
            let r = rand64(fx, s0, 0x01E0_5654);
            fx.mem.set8(p + 3, ((r >> 4) & 3) << 4);
            for (out, src, ra) in [(4u32, 0x80u32, 0x01E0_566Cu32), (0xC, 0x88, 0x01E0_56B8), (8, 0x84, 0x01E0_5704)] {
                let r = rand64(fx, s0, ra) >> 4;
                let (_, m) = fx.div(r, 0x960);
                let d = ee::div(ee::from_i32(m - 0x4B0), 100.0);
                let v = ee::add(fx.mem.f(sp + src), d);
                fx.mem.setf(p + out, v);
            }
            for o in [0x24u32, 0x20, 0x1C, 0x18, 0x14, 0x10] {
                fx.mem.write_u32(p + o, 0);
            }
            k += 1;
        }
    }
    s0
}

/// 4c02f6d018d5 0x1E04A40(D, a, b) [F]: the four cone fans 4 (a - 1) +
/// 0..3 (+0x13CC + 0x4C i): b = 0 stops them (count & 0xF), 1 starts them
/// (count 0x9F - 4 k, between nodes +0xC + 4 a and +0x10 / +8 + 4 a by a
/// odd / even), 2 / 3 set their sizes (18 / 10, 80 / 15) and apex alpha;
/// with b = 3 and a = 1 / 3 also cone a - 1 (+0x12F4 + 0x48 (a - 1)) at
/// the fans' first node, aimed from their second.
fn fans(fx: &mut Fx, a: i32, b: i32, s0: u64, ra: u32) {
    let sp = fx.enter(0x80);
    // The saves (the entry's s0, the runner's s1 = 0, s2 = D, s3 = the OT,
    // s4 not known, ra) [run: the ra save's upper half clears the motes'
    // missing-texture TEX0].
    fx.saved(sp, &[(0, s0), (0x10, 0), (0x20, u64::from(D)), (0x30, u64::from(OT)), (0x50, u64::from(ra))]);
    for k in 0..4i32 {
        let r = D + 0x13CC + 0x4C * (k + 4 * (a - 1)) as u32;
        match b {
            0 => {
                let c = fx.mem.read_u8(r) & 0xF;
                fx.mem.write_u8(r, c);
            }
            1 => {
                set_counter4(fx, r, 0x9F, 0, 0xF);
                fx.mem.set8(r, 0x9F - 4 * k);
                fx.mem.write_u8(r + 4, 2);
                fx.mem.write_u8(r + 7, 3);
                fx.mem.write_u8(r + 8, 1);
                fx.mem.write_u8(r + 0xA, 0x20);
                fx.mem.write_u8(r + 9, 7);
                let q = D + 4 * a as u32;
                let v = fx.mem.read_u8(q + 0xC);
                fx.mem.write_u8(r + 0xB, v);
                let v = fx.mem.read_u8(if a & 1 != 0 { q + 0x10 } else { q + 8 });
                fx.mem.write_u8(r + 0xC, v);
                fx.mem.write_u32(r + 0x14, 0x4190_0000);
                fx.mem.write_u32(r + 0x10, 0x4120_0000);
                let v = ee::div(fx.mem.f(r + 0x14), 16.0);
                fx.mem.setf(r + 0x1C, v);
                fx.mem.write_u32(r + 0x20, 0x3F86_6666);
                for (o, v) in [(0x25u32, 0x10u8), (0x26, 0x10), (0x24, 0x80), (0x27, 0), (0x28, 0x80), (0x29, 0x80), (0x2A, 0x80), (0x2B, 0x80)] {
                    fx.mem.write_u8(r + o, v);
                }
            }
            2 | 3 => {
                let (len, rad, alpha) = if b == 2 { (0x4190_0000u32, 0x4120_0000u32, 0x80u8) } else { (0x42A0_0000, 0x4170_0000, 0xC0) };
                fx.mem.write_u32(r + 0x14, len);
                fx.mem.write_u32(r + 0x10, rad);
                fx.mem.write_u8(r + 0x2B, alpha);
                let v = ee::div(fx.mem.f(r + 0x14), 16.0);
                fx.mem.setf(r + 0x1C, v);
            }
            _ => {}
        }
    }
    if b == 3 && (a == 1 || a == 3) {
        let c = D + 0x12F4 + 0x48 * (a - 1) as u32;
        set_counter4(fx, c, 0x18, 0, 0xFF);
        for (o, v) in [(4u32, 0u8), (5, 0), (7, 9), (8, 0), (0xA, 0), (9, 0)] {
            fx.mem.write_u8(c + o, v);
        }
        fx.mem.write_u32(c + 0xC, 0x4080_0000);
        fx.mem.write_u32(c + 0x10, 0x3F00_0000);
        fx.mem.write_u32(c + 0x14, 0x3F66_6666);
        fx.mem.write_u32(c + 0x18, 0);
        fx.mem.write_u32(c + 0x1C, 0x3DCC_CCCD);
        for (o, v) in [(0x20u32, 0x80u8), (0x21, 0x80), (0x22, 0x80), (0x23, 0xC0), (0x25, 0x10), (0x26, 0x10), (0x24, 0x80), (0x27, 0)] {
            fx.mem.write_u8(c + o, v);
        }
        let f = D + 0x13CC + 0x4C * (4 * (a - 1)) as u32;
        let h = fx.mem.i32(D + 4);
        let slot = fx.mem.u8(f + 0xB);
        let s1 = Some(u64::from(D + 0x48 * (a - 1) as u32));
        node_position_saving(fx, h, slot, sp + 0x60, 4, s1, 0x01E0_4DA4);
        copy_v4(fx, sp + 0x60, c + 0x28);
        let h = fx.mem.i32(D + 4);
        let slot = fx.mem.u8(f + 0xC);
        node_position_saving(fx, h, slot, sp + 0x60, 4, s1, 0x01E0_4DE4);
        for k in 0..3u32 {
            let v = ee::sub(fx.mem.f(c + 0x28 + 4 * k), fx.mem.f(sp + 0x60 + 4 * k));
            fx.mem.setf(sp + 0x70 + 4 * k, v);
        }
        aim_angles(fx, sp + 0x70, c + 0x38);
    }
    fx.leave(0x80);
}

/// 6d1384771e58 0x1E02870 [F]: `cones_tri(recs, ot, n) -> 1` steps and
/// draws `n` cone fans (0x4C bytes; a 0x380 frame, the descriptor at sp +
/// 0x2E8, its spec table 0x1E05B10 copied to sp + 0x250). Per record:
/// countdown 0x101AF0; the texture of spec +8; s = ((+2 - +0) & +3), the
/// height +0x18 = (s + 1) x +0x1C; unless flag 4 the base +0x2C at node +0xB
/// (self) and the angles +0x3C aimed (0x101940) from it to node +0xC (flag
/// 1: away), the base then moved back by +0x20 of that; the camera x
/// (angles +0x3C, base +0x2C); k = min(+7, 9) + 1 points of a quarter turn
/// (libm cosf / sinf) of radius sqrtf(+0x10^2 (1 - (h / +0x14)^2)) at y =
/// -h (sp + 0x160..); per step four triangles (the quadrants, as cones') to
/// the apex (the origin), the last vertex the apex again; colours +0x24.. /
/// +0x28.. (apex), alphas +0x27 / +0x2B x (16 - s) / 16 under 16 (flag 2:
/// halved on odd counts); specs 1..3 STQ and a REGION_REPEAT clamp from +9
/// / +0xA, else 0x101EE0; key 0.
fn cones_tri(fx: &mut Fx, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x380);
    fx.mem.set32(sp + 0xDC, n);
    for k in (0..0x60u32).step_by(8) {
        let v = fx.mem.read_u64(0x01E0_5B10 + k);
        fx.mem.write_u64(sp + 0x250 + k, v);
    }
    let mut g = recs;
    let mut i = 0;
    fx.mem.set32(sp + 0xB0, 0);
    while i < n {
        if i != 0 {
            g += 0x4C;
        }
        cone_tri(fx, sp, g, ot);
        i += 1;
        fx.mem.set32(sp + 0xB0, i);
    }
    fx.leave(0x380);
    1
}

fn cone_tri(fx: &mut Fx, sp: u32, g: u32, ot: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    let e = fx.mem.u8(g + 8) as u32;
    let spec = sp + 0x250 + 0x18 * e;
    let (kind, id) = (fx.mem.i32(spec), fx.mem.u8(spec + 4));
    let tex = fx.texture(kind, id);
    let s = ((fx.mem.u8(g + 2) - fx.mem.u8(g)) & 0xFF) & fx.mem.u8(g + 3);
    let h = ee::mul(ee::from_i32(s + 1), fx.mem.f(g + 0x1C));
    fx.mem.setf(g + 0x18, h);
    if fx.mem.u8(g + 4) & 4 == 0 {
        fx.mem.write_u8(g + 5, 1);
        let b = fx.mem.read_u8(g + 0xB);
        fx.mem.write_u8(g + 6, b);
        node_source(fx, g, g + 0x2C);
        let b = fx.mem.read_u8(g + 0xC);
        fx.mem.write_u8(g + 6, b);
        node_source(fx, g, sp + 0x2B0);
        for c in 0..3u32 {
            let v = ee::sub(fx.mem.f(sp + 0x2B0 + 4 * c), fx.mem.f(g + 0x2C + 4 * c));
            fx.mem.setf(sp + 0x2C0 + 4 * c, v);
        }
        if fx.mem.u8(g + 4) & 1 != 0 {
            for c in 0..3u32 {
                let v = ee::mul(fx.mem.f(sp + 0x2C0 + 4 * c), -1.0);
                fx.mem.setf(sp + 0x2C0 + 4 * c, v);
            }
        }
        aim_angles(fx, sp + 0x2C0, g + 0x3C);
        for c in 0..3u32 {
            let back = ee::mul(fx.mem.f(sp + 0x2C0 + 4 * c), fx.mem.f(g + 0x20));
            let v = ee::sub(fx.mem.f(sp + 0x2B0 + 4 * c), back);
            fx.mem.setf(g + 0x2C + 4 * c, v);
        }
    }
    camera_vp_into(fx, sp + 0x120);
    rot_trans_into(fx, sp + 0xE0, g + 0x3C, g + 0x2C);
    mul_mem(fx, sp + 0x120, sp + 0xE0);
    let mut k = fx.mem.u8(g + 7);
    if k >= 10 {
        k = 9;
    }
    fx.mem.set32(sp + 0xC0, k);
    let step = ee::div(90.0, ee::from_i32(k));
    let (r, l, h) = (fx.mem.f(g + 0x10), fx.mem.f(g + 0x14), fx.mem.f(g + 0x18));
    let q = ee::div(ee::mul(h, h), ee::mul(l, l));
    let rad = ee::mul(ee::mul(r, r), ee::sub(1.0, q));
    let rad = fx.sqrtf(rad);
    for j in 0..=k {
        let ang = ee::div(ee::mul(f32::from_bits(PI), ee::mul(step, ee::from_i32(j))), 180.0);
        let p = sp + 0x160 + 16 * j as u32;
        let c = libm_cosf(fx, ang);
        fx.mem.setf(p, ee::mul(rad, c));
        let s = fx.libm_sinf(ang);
        fx.mem.setf(p + 8, ee::mul(rad, s));
        let y = ee::neg(fx.mem.f(g + 0x18));
        fx.mem.setf(p + 4, y);
    }
    let d = sp + 0x2E8;
    fx.desc_init(d, 0xC, 0x34, REGS_TEX4);
    for (src, o) in [(0x26u32, 0u32), (0x25, 1), (0x24, 2)] {
        let v = fx.mem.read_u8(g + src);
        fx.mem.write_u8(sp + 0x310 + o, v);
        fx.mem.write_u8(sp + 0x2F8 + o, v);
        let v = fx.mem.read_u8(g + src + 4);
        fx.mem.write_u8(sp + 0x340 + o, v);
        fx.mem.write_u8(sp + 0x328 + o, v);
    }
    for o in [0x344u32, 0x32C, 0x314, 0x2FC] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    let (base, apex) = if s < 0x10 {
        let a = 0x10 - s;
        let b = fx.mem.u8(g + 0x27) >> 4;
        let base = fx.mult(a, b);
        (base, a.wrapping_mul(fx.mem.u8(g + 0x2B) >> 4))
    } else {
        (fx.mem.u8(g + 0x27), fx.mem.u8(g + 0x2B))
    };
    fx.mem.set8(sp + 0x313, base);
    fx.mem.set8(sp + 0x2FB, base);
    fx.mem.set8(sp + 0x343, apex);
    fx.mem.set8(sp + 0x32B, apex);
    if fx.mem.u8(g + 4) & 2 != 0 && fx.mem.u8(g) & 1 != 0 {
        for o in [0x2FBu32, 0x313, 0x32B, 0x343] {
            let v = fx.mem.read_u8(sp + o) >> 1;
            fx.mem.write_u8(sp + o, v);
        }
    }
    for o in [0x228u32, 0x224, 0x220] {
        fx.mem.write_u32(sp + o, 0);
    }
    for j in 0..k as u32 {
        let p = sp + 0x160 + 16 * j;
        for q in 0..4 {
            match q {
                0 => {
                    copy_v4(fx, p, sp + 0x200);
                    copy_v4(fx, p + 0x10, sp + 0x210);
                }
                1 => {
                    let v = ee::neg(fx.mem.f(p));
                    fx.mem.setf(sp + 0x200, v);
                    let v = ee::neg(fx.mem.f(p + 0x10));
                    fx.mem.setf(sp + 0x210, v);
                }
                2 => {
                    let v = ee::neg(fx.mem.f(p + 8));
                    fx.mem.setf(sp + 0x208, v);
                    let v = ee::neg(fx.mem.f(p + 0x18));
                    fx.mem.setf(sp + 0x218, v);
                }
                _ => {
                    let v = fx.mem.read_u32(p);
                    fx.mem.write_u32(sp + 0x200, v);
                    let v = fx.mem.read_u32(p + 0x10);
                    fx.mem.write_u32(sp + 0x210, v);
                }
            }
            tri(fx, sp, g, ot, spec, tex, d);
        }
    }
}

/// One triangle of [`cone_tri`] (vertices sp + 0x200 / 0x210 / the apex
/// sp + 0x220), skipped when one fails to project (0x204D80).
fn tri(fx: &mut Fx, sp: u32, g: u32, ot: u32, spec: u32, tex: u32, d: u32) {
    for (v, xyz, w) in [(0x200u32, 0x308u32, 0x240u32), (0x210, 0x320, 0x244), (0x220, 0x338, 0x248)] {
        let m = fx.read_m4(sp + 0x120);
        if !fx.project_at(sp + v, &m, sp + xyz, sp + w) {
            return;
        }
    }
    let v = fx.mem.read_u64(sp + 0x338);
    fx.mem.write_u64(sp + 0x350, v);
    for o in [0x357u32, 0x33F, 0x327, 0x30F] {
        fx.mem.write_u8(sp + o, 0);
    }
    for k in 0..6u32 {
        let v = fx.mem.read_u32(spec + 4 * k);
        fx.mem.write_u32(sp + 0x2D0 + 4 * k, v);
    }
    let e = fx.mem.u8(g + 8);
    if (1..4).contains(&e) {
        let wrap = u64::from(fx.mem.read_u8(g + 9));
        let (u0, v0, v1) = (fx.mem.i32(sp + 0x2D8), fx.mem.i32(sp + 0x2E0), fx.mem.i32(sp + 0x2E4));
        let clamp = ((wrap << 4) | 0xB) | ((i64::from(u0) << 14) as u64) | ((i64::from(v0) << 24) as u64) | ((i64::from(v1.wrapping_sub(1)) << 34) as u64);
        fx.desc_clamp(d, clamp);
        fx.mem.write_u32(sp + 0x2D8, 0);
        let b = fx.mem.u8(g + 0xA);
        fx.mem.set32(sp + 0x2DC, b);
        let (w0, w1, w2) = (fx.mem.f(sp + 0x240), fx.mem.f(sp + 0x244), fx.mem.f(sp + 0x248));
        fx.mem.setf(sp + 0x2FC, ee::div(1.0, w0));
        fx.mem.setf(sp + 0x314, ee::div(1.0, w1));
        let q2 = ee::div(1.0, w2);
        fx.mem.setf(sp + 0x32C, q2);
        fx.mem.setf(sp + 0x344, q2);
        let t = |fx: &mut Fx, o: u32| ee::div(ee::from_i32(fx.mem.i32(sp + o)), 256.0);
        let s0 = t(fx, 0x2D8);
        fx.mem.setf(sp + 0x300, ee::div(s0, w0));
        let t0 = t(fx, 0x2E0);
        fx.mem.setf(sp + 0x304, ee::div(t0, w0));
        let s1 = t(fx, 0x2DC);
        fx.mem.setf(sp + 0x318, ee::div(s1, w1));
        fx.mem.setf(sp + 0x31C, ee::div(t0, w1));
        fx.mem.setf(sp + 0x330, ee::div(s0, w2));
        let t1 = ee::div(t(fx, 0x2E4), w2);
        fx.mem.setf(sp + 0x334, t1);
        fx.mem.setf(sp + 0x348, ee::div(s1, w2));
        fx.mem.setf(sp + 0x34C, t1);
        alpha_mode(fx, sp + 0x2D0, sp + 0x37C);
        let b = |fx: &mut Fx, k: u32| u64::from(fx.mem.read_u8(sp + 0x37C + k));
        let a = b(fx, 0) | (b(fx, 1) << 2) | (b(fx, 2) << 4) | (b(fx, 3) << 6);
        fx.desc_alpha(d, a);
        fx.desc_texture(d, tex, 1);
    } else {
        tex_rect_strip(fx, d, sp + 0x2D0, tex);
    }
    fx.submit(d, ot, 0);
}

fn fix16(fx: &mut Fx, a: u32, v: f32) {
    let x = ee::fixunssfsi(v) as i32;
    fx.mem.set16(a, x);
}

/// 5efd9b9d205d 0x1E03F40 [F]: `beads(recs, ot, n) -> 1` steps and draws
/// `n` bead groups (0x40 bytes; a 0x1B0 frame, the descriptor at sp +
/// 0x128) and their particles (0x28 bytes of the pool at the first
/// group's +8, by the group's i16 list +0xC, +0x1D long; -1 empty), as
/// `sprite::sparks` but: the texture (5, +0x1F) always; velocity +=
/// +0x1C.., position += velocity, no floor; drawn at the node of +5 / +6
/// (0x101BA0, else the particle) + the group's +0x30..; colour +0x26 /
/// +0x25 / +0x24, alpha +0x27 (x count / 8 under 8 unless flag 0x40); cell
/// 7 - (count >> +0x20) under 4, else (((+3 + 1) - (count & +3)) or the
/// elapsed) >> +0x20 & 7 (flag 1: a random orientation at cell 0); half
/// size +0x28 / (w / 256); far Z with flag 0x80; key 0.
fn beads(fx: &mut Fx, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x1B0);
    fx.mem.set32(sp + 0xBC, n);
    let pool = fx.mem.read_u32(recs + 8);
    let mut g = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x40;
        }
        bead_group(fx, sp, g, pool, ot);
        i += 1;
    }
    fx.leave(0x1B0);
    1
}

fn bead_group(fx: &mut Fx, sp: u32, g: u32, pool: u32, ot: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    let id = fx.mem.u8(g + 0x1F);
    let tex = fx.texture(5, id);
    let v = ee::add(fx.mem.f(g + 0x28), fx.mem.f(g + 0x2C));
    fx.mem.setf(g + 0x28, v);
    let mut k = 0u32;
    while (k as i32) < fx.mem.u8(g + 0x1D) {
        let idx = fx.mem.i16(g + 0xC + 2 * k);
        if idx != -1 {
            let s = pool.wrapping_add(0x28u32.wrapping_mul(idx as u32));
            if fx.mem.u8(s) < 2 {
                fx.mem.set16(g + 0xC + 2 * k, -1);
            } else {
                fx.mem.write_u8(g, 1);
            }
            if countdown(fx, s) != 0 {
                bead(fx, sp, g, s, tex, ot);
            }
        }
        k += 1;
    }
}

fn bead(fx: &mut Fx, sp: u32, g: u32, s: u32, tex: u32, ot: u32) {
    for c in 0..3u32 {
        let v = ee::add(fx.mem.f(s + 0x10 + 4 * c), fx.mem.f(s + 0x1C + 4 * c));
        fx.mem.setf(s + 0x10 + 4 * c, v);
    }
    for c in 0..3u32 {
        let v = ee::add(fx.mem.f(s + 4 + 4 * c), fx.mem.f(s + 0x10 + 4 * c));
        fx.mem.setf(s + 4 + 4 * c, v);
    }
    for c in 0..3u32 {
        let v = fx.mem.read_u32(s + 4 + 4 * c);
        fx.mem.write_u32(sp + 0x100 + 4 * c, v);
    }
    node_source(fx, g, sp + 0x100);
    for c in 0..3u32 {
        let v = ee::add(fx.mem.f(sp + 0x100 + 4 * c), fx.mem.f(g + 0x30 + 4 * c));
        fx.mem.setf(sp + 0x100 + 4 * c, v);
    }
    camera_vp_into(fx, sp + 0xC0);
    let d = sp + 0x128;
    fx.desc_init(d, 9, 0x24, 0x4_2424_2421);
    for (o, src) in [(0x138u32, 0x26u32), (0x139, 0x25), (0x13A, 0x24)] {
        let v = fx.mem.read_u8(g + src);
        fx.mem.write_u8(sp + o, v);
    }
    fx.mem.write_u32(sp + 0x13C, 0x3F80_0000);
    let count = fx.mem.u8(s);
    let mask = fx.mem.u8(g + 3);
    let sh = (fx.mem.u8(g + 0x20) & 31) as u32;
    let v = if mask != 0xFF { (mask + 1) - (count & mask) } else { fx.mem.u8(s + 2) - count };
    let mut cell = (v >> sh) & 7;
    let c = count >> sh;
    if c < 4 {
        cell = 7 - c;
    }
    if fx.mem.u8(g + 4) & 1 != 0 && cell == 0 {
        let r = fx.rand_from(g, 0x01E0_41E0);
        let o = ((r >> 4) & 3) << 4;
        let b = (fx.mem.u8(s + 3) & 0xC7) | o;
        fx.mem.set8(s + 3, b);
    }
    let b = fx.mem.read_u8(g + 0x1E);
    fx.mem.write_u8(sp + 0x115, b);
    atlas_cell(fx, sp + 0x110, cell, 0);
    tex_rect(fx, d, sp + 0x110, tex);
    let v = fx.mem.read_u8(g + 0x27);
    fx.mem.write_u8(sp + 0x13B, v);
    if fx.mem.u8(g + 4) & 0x40 == 0 {
        let c = fx.mem.u8(s);
        if c < 8 {
            let a = fx.mem.u8(g + 0x27) >> 3;
            let v = fx.mult(a, c);
            fx.mem.set8(sp + 0x13B, v);
        }
    }
    let m = fx.read_m4(sp + 0xC0);
    if !fx.project_at(sp + 0x100, &m, sp + 0x1A0, sp + 0x1AC) {
        return;
    }
    if !Fx::on_screen(fx.mem.read_u64(sp + 0x1A0)) {
        return;
    }
    let half = ee::div(fx.mem.f(g + 0x28), ee::div(fx.mem.f(sp + 0x1AC), 256.0));
    let mut z = if fx.mem.u8(g + 4) & 0x80 != 0 { 0xFF_FFFE } else { fx.xyz_z(sp + 0x1A0) };
    for o in [0x178u32, 0x168, 0x158, 0x148] {
        fx.set_xyz_z(sp + o, z);
        z = fx.xyz_z(sp + o);
    }
    // (x minus, x plus, y minus, y plus) corner pairs by orientation, the
    // vertices at sp + 0x148 / 0x158 / 0x168 / 0x178.
    let (xm, xp, ym, yp): ([u32; 2], [u32; 2], [u32; 2], [u32; 2]) = match fx.mem.u8(s + 3) & 0x30 {
        0x10 => ([0x158, 0x148], [0x178, 0x168], [0x17A, 0x15A], [0x16A, 0x14A]),
        0x20 => ([0x178, 0x158], [0x168, 0x148], [0x17A, 0x16A], [0x15A, 0x14A]),
        0x30 => ([0x178, 0x168], [0x158, 0x148], [0x16A, 0x14A], [0x17A, 0x15A]),
        _ => ([0x168, 0x148], [0x178, 0x158], [0x15A, 0x14A], [0x17A, 0x16A]),
    };
    let x = ee::from_i32(fx.mem.u16(sp + 0x1A0));
    for o in xm {
        fix16(fx, sp + o, ee::sub(x, half));
    }
    for o in xp {
        fix16(fx, sp + o, ee::add(x, half));
    }
    let y = ee::from_i32(fx.mem.u16(sp + 0x1A2));
    let hy = ee::mul(f32::from_bits(ASPECT), half);
    for o in ym {
        fix16(fx, sp + o, ee::sub(y, hy));
    }
    for o in yp {
        fix16(fx, sp + o, ee::add(y, hy));
    }
    fx.submit(d, ot, 0);
}
