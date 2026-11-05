//! BMODULE b076 (card 453), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04C00, data area a0 = 0x1ED0000) [F] (helper B3, 30
//! Oct 2026): the first record of the "shared-library" family to run off
//! the interpreter. The entry steps a counter (D+0, u16) and, from fixed
//! updates, starts library effects at the target's position: on update 89
//! an arc pair (+0x11E0, `arc::arcs`), on 92 a camera shake (+0x40), a
//! flash (+0xFB4, `star::flash`) and six crossing bars (+0x1000 + 0x50 k,
//! `cross::bars`, the fifth 1000 wide, the others 500 +- 100 at random),
//! on 93 five bead beams (+0x1240 + 0x58 k, `beam::beads`, random bytes
//! +0x1718); node bursts (+0xA0, `burst::node_bursts`) and 32 motes (+0xE4,
//! `mote::motes`) every update. Returns 1 on update 83, 2 on 90, 8 on 180
//! (and at once for a parameter of -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `beads` 0x1E015F0, `arcs`
//! 0x1E023C0, `bars` 0x1E03640, `flash` 0x1E04050 [`crate::blib`].
use crate::blib::{self, exe::set_counter4};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// 0x102330(p, n): n bytes p[k] = rand() >> 4 [F]. Leaves its frame (0x40:
/// the caller's s0 +0, s1 +0x10, s2 +0x20 (`None`: not known, left), `ra`
/// +0x30) and rand's (s0 = p + k, ra 0x102360) below the caller's sp.
pub(crate) fn rand_bytes(fx: &mut Fx, p: u32, n: i32, ra: u32, caller_s: [Option<u32>; 3]) {
    let f = fx.sp - 0x40;
    for (k, v) in caller_s.iter().enumerate() {
        if let Some(v) = v {
            fx.saved(f + 0x10 * k as u32, &[(0, u64::from(*v))]);
        }
    }
    fx.saved(f, &[(0x30, u64::from(ra))]);
    fx.sp = f;
    for k in 0..n.max(0) as u32 {
        let r = fx.rand_from(p + k, 0x0010_2360);
        fx.mem.set8(p + k, r >> 4);
    }
    fx.sp = f + 0x40;
}

/// `camera_shake(s)` called with the caller's s0 = `s0` from return
/// address `ra`, leaving what the motes' missing-texture TEX0 / TEX1 read
/// [run]: its frame (0x20: s0 +0, ra +0x10) and, when the count was due,
/// 0x101BA0's (0x30: s0 = s +0, ra `ra_source` +0x20) and 0x1F7E20's
/// below it (0x20: s0 = s, ra `ra_slots[0]` after clearing the slots,
/// `ra_slots[1]` after setting them) [F].
pub(crate) fn shake_saving(fx: &mut Fx, s: u32, s0: u64, ra: u32, ra_source: u32, ra_slots: [u32; 2]) -> i32 {
    let due = fx.mem.read_u8(s) != 0 && fx.mem.read_u8(s + 1) == 0;
    let r = blib::shake::camera_shake(fx, s);
    let f = fx.sp - 0x20;
    fx.saved(f, &[(0, s0), (0x10, u64::from(ra))]);
    if due {
        let count = fx.mem.read_u8(s);
        let clear = (fx.mem.read_u8(s + 4) & 15 == 8 && count & 1 != 0) || count == 0;
        fx.saved(f - 0x30, &[(0, u64::from(s)), (0x20, u64::from(ra_source))]);
        fx.saved(f - 0x20, &[(0, u64::from(s)), (0x10, u64::from(ra_slots[usize::from(!clear)]))]);
    }
    r
}

/// 0x1F71A0 `unit_position(h, out)` called with the caller's s0 = `s0`
/// from return address `ra`: its frame (0x20: s0 +0, ra +0x10) left below
/// the caller's sp [F; run b096: a later missing-texture TEX1 reads the
/// s0 slot].
pub(crate) fn unit_position_saving(fx: &mut Fx, h: i32, out: u32, s0: u64, ra: u32) -> bool {
    let r = fx.unit_position(h, out);
    let f = fx.sp - 0x20;
    fx.saved(f, &[(0, s0), (0x10, u64::from(ra))]);
    r
}

/// 0x1F6830 `node_position(h, slot, out)` called with the caller's s0 /
/// s1 from return address `ra`: its frame (0x30: s0 +0, s1 +0x10 (`None`:
/// left), ra +0x20) [F].
pub(crate) fn node_position_saving(fx: &mut Fx, h: i32, slot: i32, out: u32, s0: u64, s1: Option<u64>, ra: u32) -> bool {
    let r = fx.node_position(h, slot, out);
    let f = fx.sp - 0x30;
    fx.saved(f, &[(0, s0), (0x20, u64::from(ra))]);
    if let Some(v) = s1 {
        fx.saved(f + 0x10, &[(0, v)]);
    }
    r
}

/// 0x1F6770(handle, node, clear) with its frame (0x40: the caller's s0 +0,
/// s1 +0x10, s2 +0x20 (`None`: left), `ra` +0x30) left below the caller's
/// sp [run b247: the last pick's ra is a later frame tint's
/// missing-texture TEX1].
pub(crate) fn request_node_saving(fx: &mut Fx, h: i32, node: i32, clear: i32, s: [Option<u64>; 3], ra: u32) -> i32 {
    let r = fx.request_node(h, node, clear);
    let f = fx.sp - 0x40;
    for (k, v) in s.iter().enumerate() {
        if let Some(v) = v {
            fx.saved(f + 0x10 * k as u32, &[(0, *v)]);
        }
    }
    fx.saved(f, &[(0x30, u64::from(ra))]);
    r
}

pub(crate) fn copy_v4(fx: &mut Fx, from: u32, to: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// The entry 0x1E04C00 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x60);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, sp, param)
    } else {
        step(fx, sp)
    };
    fx.leave(0x60);
    ret
}

fn init(fx: &mut Fx, _sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x1820);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    fx.texture(5, 0);
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0x17, 0), (0x14, 0xC, 0), (0x18, 0x37, 0), (0x1C, 0x1B, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA0): spec 0x1E05528 / 0x1E05540, rings at +0xE4
    // / +0x7E4, nodes from +0x14 / +0x18.
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xC8, b);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(D + 0xC9, b);
    fx.mem.write_u32(D + 0xA8, 0x01E0_5528);
    fx.mem.write_u32(D + 0xAC, 0x01E0_5540);
    fx.mem.write_u32(D + 0xA0, D + 0xE4);
    fx.mem.write_u8(D + 0xD0, 0x20);
    fx.mem.write_u32(D + 0xA4, D + 0x7E4);
    fx.mem.write_u8(D + 0xD1, 0x32);
    fx.mem.write_u8(D + 0xD2, 0);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xD3, b);
    fx.mem.set16(D + 0xD4, 0x59);
    fx.mem.set16(D + 0xD6, 0x63);
    fx.mem.write_u32(D + 0xD8, 0x42F0_0000);
    fx.mem.write_u32(D + 0xDC, 0x42F0_0000);
    fx.mem.write_u32(D + 0xE0, 0x40A0_0000);
    // The entry's s0 = the OT, s1 = 1 (the texture loop's counter).
    let _ = param;
    rand_bytes(fx, D + 0x1718, 0x100, 0x01E0_4DC8, [Some(OT), Some(1), None]);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, sp + 0x50);
    match fx.mem.u16(D) {
        0xB4 => return 8,
        0x5D => beads_start(fx, sp),
        0x5C => bars_start(fx, sp),
        0x59 => arcs_start(fx),
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_52B4, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let frame = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA0, frame);
    blib::mote::motes(fx, D + 0xE4, OT, 0x20);
    blib::beam::beads(fx, 0x01E0_15F0, D + 0x1240, OT, 5);
    blib::cross::bars(fx, 0x01E0_3640, D + 0x1000, OT, 6);
    blib::star::flash(fx, 0x01E0_4050, D + 0xFB4, OT, 1);
    blib::arc::arcs(fx, 0x01E0_23C0, D + 0x11E0, OT, 1);
    match fx.mem.u16(D) {
        0x5A => 2,
        0x53 => 1,
        _ => 0,
    }
}

/// Update 89 [F 0x1E04E2C]: the arc pair (+0x11E0), nodes from +0x1C /
/// +0x10, points at +0x13F8.
fn arcs_start(fx: &mut Fx) {
    let g = D + 0x11E0;
    set_counter4(fx, g, 0xC, 0, 0xFF);
    fx.mem.write_u8(g + 4, 0xD);
    fx.mem.write_u32(g + 8, D + 0x13F8);
    fx.mem.write_u8(g + 0x30, 0x32);
    fx.mem.write_u8(g + 0x31, 8);
    let b = fx.mem.read_u8(D + 0x1C);
    fx.mem.write_u8(g + 0x32, b);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(g + 0x33, b);
    fx.mem.write_u32(g + 0x38, 0x3F99_999A);
    fx.mem.write_u32(g + 0x3C, 0x4020_0000);
    fx.mem.write_u32(g + 0x40, 0x41A0_0000);
    for (k, v) in [(0x44u32, 0x80u8), (0x45, 0x80), (0x46, 0x80), (0x47, 0xC0), (0x4A, 0xC0), (0x49, 0x40), (0x48, 0x20), (0x4E, 0xC0), (0x4D, 0x40), (0x4C, 0x20), (0x4F, 0x80)] {
        fx.mem.write_u8(g + k, v);
    }
    fx.mem.write_u32(g + 0x54, 0x3F99_999A);
    let mut k = 0u32;
    while k < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0xC + 2 * k, -1);
        fx.mem.set16(g + 0x1E + 2 * k, -1);
        k += 1;
    }
}

/// Update 92 [F 0x1E04F14]: the camera shake (+0x40), the flash (+0xFB4)
/// and six bars (+0x1000 + 0x50 k) at the target (sp + 0x50).
fn bars_start(fx: &mut Fx, sp: u32) {
    let s = D + 0x40;
    set_counter4(fx, s, 0x18, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x3FC0_0000);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x4000_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, sp + 0x50, D + 0x60);
    let g = D + 0xFB4;
    set_counter4(fx, g, 0x18, 0, 0xFF);
    fx.mem.write_u8(g + 5, 0);
    fx.mem.write_u8(g + 4, 0x81);
    fx.mem.write_u8(g + 7, 9);
    fx.mem.write_u32(g + 0x10, 0x43FA_0000);
    fx.mem.write_u32(g + 0x14, 0);
    for (k, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x50), (9, 0x10), (8, 0x80), (0xF, 0xD0)] {
        fx.mem.write_u8(g + k, v);
    }
    for k in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(g + k, 0);
    }
    copy_v4(fx, sp + 0x50, D + 0xFD0);
    for k in 0..6i32 {
        let r = D + 0x1000 + 0x50 * k as u32;
        set_counter4(fx, r, 0x14, (k >> 1) & 0xFF, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0x80);
        if k == 4 {
            fx.mem.write_u32(r + 0x10, 0x447A_0000);
            fx.mem.write_u32(r + 0x14, 0x41A0_0000);
        } else {
            let x = fx.rand_from(OT, 0x01E0_5094) >> 4;
            let (_, h) = fx.div(x, 0x4E20);
            let f = ee::div(ee::from_i32(h - 0x2710), 100.0);
            fx.mem.setf(r + 0x10, ee::add(500.0, f));
            fx.mem.write_u32(r + 0x14, 0x4170_0000);
        }
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0xC0), (0xA, 0x50), (9, 0x10), (8, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.setf(r + 0x18, ee::mul(30.0, ee::from_i32(k)));
        fx.mem.write_u32(r + 0x1C, 0x3F80_0000);
        copy_v4(fx, sp + 0x50, r + 0x20);
    }
}

/// Update 93 [F 0x1E05184]: five bead beams (+0x1240 + 0x58 k) at the
/// target, bytes +0x1718.
fn beads_start(fx: &mut Fx, sp: u32) {
    for k in 0..5i32 {
        let r = D + 0x1240 + 0x58 * k as u32;
        set_counter4(fx, r, 0x14, (k << 1) & 0xFF, 0xFF);
        fx.mem.write_u8(r + 4, 3);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 8, 0x23);
        let x = fx.rand_from(OT, 0x01E0_51E4);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.write_u32(r + 0x1C, 0x4140_0000);
        fx.mem.write_u32(r + 0x20, 0);
        fx.mem.write_u32(r + 0x10, D + 0x1718);
        let f = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(k));
        fx.mem.setf(r + 0x14, ee::add(f32::from_bits(0x3F99_999A), f));
        for (o, v) in [(0x26u32, 0xC0u8), (0x25, 0x40), (0x24, 0x20), (0x27, 0xF0)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, sp + 0x50, r + 0x28);
    }
}
