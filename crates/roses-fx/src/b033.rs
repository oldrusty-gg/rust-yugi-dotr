//! BMODULE b033 (card 59), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05650, data area a0 = 0x1ED0000, 0x1C90 bytes)
//! [F]: b076's "a0 = D" family. Init: the
//! attacker's nodes 0x1E (clear) / 0x10 at D+0xC / D+0x10, 256 random
//! bytes D+0x1B74, the bolt reach D+0x1C88 = 0.6 x the target's radius
//! (at least 14). Every update the target's point (0x1F65D0 kind 1) at sp
//! + 0x50. On 2 twenty flickers (D+0x130C) at the attacker's two nodes;
//! on 0x5A a lens (D+0xA0) and ten flickers sent from the nodes' midpoint
//! toward the attacker's point D+0x1C74 (25 up); on 0x8A two bead beams,
//! two bolts, a flash, the bars and a camera shake there; on 0x9A the bolt
//! spawner (D+0xB00) and two flashes moving with ten flickers, turned by
//! the attacker's yaw on 0xD9 and sent to the target on 0xE2; on 0x122
//! four lenses, four bead beams, two flashes and a shake at the target,
//! and from 0x123 to 0x145 four bolts a update between the target's nodes
//! (0x1E05140) with a flickering target tint (0x1F77A0), cleared on 0x146.
//! Returns 1 / 6 / 2 / 8 on 0x118 / 0x122..0x142 / 0x143 / 0x15E.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `beads`
//! 0x1E00410, `flash` 0x1E02020, `bars` 0x1E02BD0, `flickers` 0x1E035E0
//! [`crate::blib`]; the record's own `bolts` 0x1E011E0 (bolt.rs's routine
//! at 0x3C-byte records, one strip a segment) and its spawner 0x1E01CF0
//! (0x38-byte records) copied here, the lens 0x1E040C0 and the bolt burst
//! 0x1E05140.
use crate::b076::{node_position_saving, rand_bytes, request_node_saving, shake_saving, unit_position_saving};
use crate::blib::{self, beam::length3, exe::{camera_vp_into, countdown, first_free, libm_cosf, node_source, set_counter4, tex_rect_strip_a3}, rel};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BOLTS: u32 = D + 0x1A0;
const SPAWNER: u32 = D + 0xB00;
const FLASH: u32 = D + 0xE80;
const BARS: u32 = D + 0x112C;
const FLICKERS: u32 = D + 0x130C;
const BEADS: u32 = D + 0x1A14;
const BYTES: u32 = D + 0x1B74;
const POINT: u32 = D + 0x1C74;
const TINT_WAIT: u32 = D + 0x1C84;
const REACH: u32 = D + 0x1C88;
const LENSES: u32 = D + 0xA0;
const SHAKE: u32 = D + 0x40;

/// The entry 0x1E05650 (a0 = D, a1 = param; s0 = D, s1 = the OT).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x90);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x1C90);
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
    // The entry's s0 = D, s1 = the OT, s2 = 2 (the texture loop's).
    let s = [Some(u64::from(D)), Some(u64::from(OT)), Some(2)];
    let h = fx.mem.i32(D + 4);
    let r = request_node_saving(fx, h, 0x1E, 1, s, 0x01E0_5730);
    fx.mem.set32(D + 0xC, r);
    let h = fx.mem.i32(D + 4);
    let r = request_node_saving(fx, h, 0x10, 0, s, 0x01E0_5748);
    fx.mem.set32(D + 0x10, r);
    rand_bytes(fx, BYTES, 0x100, 0x01E0_575C, [Some(D), Some(OT), Some(2)]);
    let t = fx.mem.i32(D + 8);
    let r = blib::afterimage::unit_radius(fx, t);
    let r = ee::mul(f32::from_bits(0x3F19_999A), r);
    fx.mem.setf(REACH, r);
    if ee_lt(r, 14.0) {
        fx.mem.write_u32(REACH, 0x4160_0000);
    }
    0
}

/// 0x1F65D0(kind, h, out) with its frame (0x50: the caller's s0..s3 +0 ..
/// +0x30, `ra` +0x40) left below the caller's sp [F exe 0x1F65D0; run: a
/// later missing-texture TEX0 reads s2's upper half].
fn unit_point_saving(fx: &mut Fx, kind: i32, h: i32, out: u32, s: [u64; 4], ra: u32) -> i32 {
    let r = fx.unit_point(kind, h, out);
    let f = fx.sp - 0x50;
    fx.saved(f, &[(0, s[0]), (0x10, s[1]), (0x20, s[2]), (0x30, s[3]), (0x40, u64::from(ra))]);
    r
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

/// A flicker record (0x3C bytes) as the entry starts them.
fn flicker(fx: &mut Fx, r: u32, count: i32, delay: i32, c: i32, alpha: u8) {
    set_counter4(fx, r, count, delay, c);
    for (o, v) in [(5u32, 0u8), (4, 0), (8, 1), (9, 3), (0xA, 2), (0xB, 0), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, alpha)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x10, 0x420C_0000);
    fx.mem.write_u32(r + 0x14, 0x12C);
}

fn flicker_at(k: u32) -> u32 {
    FLICKERS + 0x3C * k
}

/// Update 2 [F 0x1E05844]: the attacker's point D+0x1C74 (25 up), twenty
/// flickers at its nodes D+0xC / D+0x10 by turns.
fn start(fx: &mut Fx, param: i32) {
    let h = fx.mem.i32(D + 4);
    unit_point_saving(fx, 1, h, POINT, [u64::from(D), u64::from(OT), param as i64 as u64, 0], 0x01E0_5858);
    let y = ee::add(fx.mem.f(POINT + 4), 25.0);
    fx.mem.setf(POINT + 4, y);
    for k in 0..0x14u32 {
        let r = flicker_at(k);
        set_counter4(fx, r, 0xFF, (k & 7) as i32, 7);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0xC + 4 * (k & 1));
        fx.mem.write_u8(r + 6, b);
        for (o, v) in [(4u32, 0u8), (8, 1), (9, 3), (0xA, 2), (0xB, 0), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, 0x420C_0000);
        fx.mem.write_u32(r + 0x14, 0x12C);
    }
}

/// Update 0x5A [F 0x1E0591C]: the lens D+0xA0 at the nodes' midpoint
/// D+0xBC moving to D+0x1C74 in 48 updates; ten flickers along.
fn lens_start(fx: &mut Fx, sp: u32) {
    let g = LENSES;
    set_counter4(fx, g, 0x30, 0, 0xFF);
    fx.mem.write_u8(g + 4, 2);
    fx.mem.write_u32(g + 8, 0x3F80_0000);
    fx.mem.write_u32(g + 0xC, 0x42F0_0000);
    fx.mem.write_u32(g + 0x10, 0x3F00_0000);
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0xC);
    node_position_saving(fx, h, n, sp + 0x60, u64::from(D), Some(u64::from(OT)), 0x01E0_5968);
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x10);
    node_position_saving(fx, h, n, sp + 0x70, u64::from(D), Some(u64::from(OT)), 0x01E0_5980);
    for k in 0..3 {
        let a = fx.mem.f(sp + 0x60 + 4 * k);
        let d = ee::div(ee::sub(fx.mem.f(sp + 0x70 + 4 * k), a), 2.0);
        fx.mem.setf(g + 0x1C + 4 * k, ee::add(a, d));
    }
    for k in 0..3 {
        let v = ee::div(ee::sub(fx.mem.f(POINT + 4 * k), fx.mem.f(g + 0x1C + 4 * k)), 48.0);
        fx.mem.setf(g + 0x2C + 4 * k, v);
    }
    for k in 0x14..0x1Eu32 {
        let r = flicker_at(k);
        flicker(fx, r, 0x30, (k & 7) as i32, 7, 0x60);
        copy3(fx, g + 0x1C, r + 0x18);
        copy3(fx, g + 0x2C, r + 0x24);
    }
}

/// A bead beam (0x58 bytes) at the first free of four [F 0x1E05AF8 /
/// 0x1E06378].
fn bead(fx: &mut Fx, count: i32, delay: i32, kind: u8, len: u8, ra: u32, size: u32, grow: u32, at: u32) -> Option<u32> {
    let i = first_free(fx, BEADS, 0x58, 4);
    if i < 0 {
        return None;
    }
    let b = BEADS + 0x58 * i as u32;
    set_counter4(fx, b, count, delay, 0xFF);
    fx.mem.write_u8(b + 4, kind);
    fx.mem.write_u8(b + 5, 0);
    fx.mem.write_u8(b + 8, len);
    let x = fx.rand_from(D, ra);
    fx.mem.set8(b + 9, x >> 4);
    fx.mem.write_u8(b + 0xA, 1);
    fx.mem.write_u8(b + 0xC, 2);
    fx.mem.write_u32(b + 0x1C, size);
    fx.mem.write_u32(b + 0x20, 0);
    fx.mem.write_u32(b + 0x10, BYTES);
    fx.mem.write_u32(b + 0x14, grow);
    for o in [0x27u32, 0x24, 0x25, 0x26] {
        fx.mem.write_u8(b + o, 0x80);
    }
    copy4(fx, at, b + 0x28);
    Some(0x58 * i as u32)
}

/// Update 0x8A [F 0x1E05AEC]: at D+0x1C74 two bead beams, two bolts (16
/// to either side), the flash, the bars and the camera shake. Returns
/// the entry's s2 / s3 after its loops.
fn burst_start(fx: &mut Fx) -> (u64, u64) {
    let mut s3 = 0u64;
    for k in 0..2 {
        if let Some(o) = bead(fx, 0x10, (k * 4) & 0xFF, 0xD, 0x14, 0x01E0_5B68, 0x4220_0000, 0x3FB3_3333, POINT) {
            s3 = u64::from(o);
        }
    }
    let mut s2 = 2u64;
    for k in 0..2 {
        let i = first_free(fx, BOLTS, 0x3C, 0x28);
        if i < 0 {
            s2 = k as u64;
            break;
        }
        let b = BOLTS + 0x3C * i as u32;
        s3 = u64::from(0x3C * i as u32);
        set_counter4(fx, b, 0x10, 4, 0xFF);
        fx.mem.write_u8(b + 5, 0);
        fx.mem.write_u8(b + 4, 0);
        copy3(fx, POINT, b + 0x24);
        copy3(fx, b + 0x24, b + 0x18);
        let x = fx.mem.f(b + 0x18);
        fx.mem.setf(b + 0x18, if k == 0 { ee::sub(x, 16.0) } else { ee::add(x, 16.0) });
        fx.mem.write_u8(b + 8, 8);
        fx.mem.write_u32(b + 0x10, 0x3F80_0000);
        fx.mem.write_u8(b + 0xB, 0);
        fx.mem.write_u8(b + 0xD, 3);
        fx.mem.write_u8(b + 0xC, 0);
        for o in 0x14..0x18u32 {
            fx.mem.write_u8(b + o, 0x80);
        }
    }
    let g = FLASH;
    set_counter4(fx, g, 0x14, 0, 0xFF);
    fx.mem.write_u8(g + 5, 0);
    fx.mem.write_u8(g + 4, 0x81);
    fx.mem.write_u8(g + 7, 9);
    fx.mem.write_u32(g + 0x10, 0x4416_0000);
    fx.mem.write_u32(g + 0x14, 0);
    for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0xA0), (9, 0x20), (8, 0x80)] {
        fx.mem.write_u8(g + o, v);
    }
    copy4(fx, POINT, g + 0x1C);
    let r = BARS;
    set_counter4(fx, r, 0x12, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 4, 0x80);
    fx.mem.write_u32(r + 0x10, 0x4448_0000);
    fx.mem.write_u32(r + 0x18, 0x42B4_0000);
    fx.mem.write_u32(r + 0x14, 0x4120_0000);
    for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x20), (0xA, 0xA0), (9, 0x20), (8, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x1C, 0);
    copy4(fx, POINT, r + 0x20);
    shake_start(fx, 0x14, 1, [0x4000_0000, 0x3F80_0000, 0x3FC0_0000]);
    copy4(fx, POINT, SHAKE + 0x20);
    (s2, s3)
}

fn shake_start(fx: &mut Fx, count: i32, kind: u8, amp: [u32; 3]) {
    let s = SHAKE;
    set_counter4(fx, s, count, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, kind);
    fx.mem.write_u32(s + 0x14, amp[0]);
    fx.mem.write_u32(s + 0x18, amp[1]);
    fx.mem.write_u32(s + 0x1C, amp[2]);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
}

/// Update 0x9A [F 0x1E05E90]: the bolt spawner at D+0x1C74, two flashes
/// and ten flickers there.
fn spawner_start(fx: &mut Fx) {
    let g = SPAWNER;
    set_counter4(fx, g, 0x96, 0, 1);
    fx.mem.write_u8(g + 5, 0);
    fx.mem.write_u8(g + 4, 0x80);
    fx.mem.write_u32(g + 8, BOLTS);
    for (o, v) in [(0xCu32, 0x28u8), (0xD, 0xB), (0xE, 0), (0xF, 0), (0x10, 5), (0x11, 0xC)] {
        fx.mem.write_u8(g + o, v);
    }
    fx.mem.write_u32(g + 0x14, 0x7D0);
    fx.mem.write_u32(g + 0x18, 0x3F80_0000);
    for o in [0x1Fu32, 0x1C, 0x1D, 0x1E] {
        fx.mem.write_u8(g + o, 0x80);
    }
    copy3(fx, POINT, g + 0x20);
    for k in 0..2u32 {
        let r = FLASH + 0x4C * k;
        set_counter4(fx, r, 0x96, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 1);
        fx.mem.write_u8(r + 7, 9);
        let f = fx.mem.read_u8(r + 4);
        if k != 0 {
            fx.mem.write_u8(r + 4, f | 4);
            fx.mem.write_u32(r + 0x10, 0x4416_0000);
            for (o, v) in [(0xAu32, 0xA0u8), (9, 0x20), (8, 0x80)] {
                fx.mem.write_u8(r + o, v);
            }
        } else {
            fx.mem.write_u8(r + 4, f | 0x8A);
            fx.mem.write_u32(r + 0x10, 0x4396_0000);
            for (o, v) in [(0xAu32, 0x60u8), (9, 0xC0), (8, 0x10)] {
                fx.mem.write_u8(r + o, v);
            }
        }
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(r + o, 0xE0);
        }
        fx.mem.write_u32(r + 0x14, 0);
        copy4(fx, POINT, r + 0x1C);
    }
    for k in 0x14..0x1Eu32 {
        let r = flicker_at(k);
        set_counter4(fx, r, 0x96, (k & 7) as i32, 3);
        for (o, v) in [(5u32, 0u8), (4, 0x80), (8, 1), (9, 3), (0xA, 2), (0xB, 0), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x60)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, 0x420C_0000);
        fx.mem.write_u32(r + 0x14, 0x12C);
        copy3(fx, POINT, r + 0x18);
        for o in [0x2Cu32, 0x28, 0x24] {
            fx.mem.write_u32(r + o, 0);
        }
    }
}

/// The movers' velocities (the spawner +0x2C.., the flashes +0x2C.., the
/// flickers 20..29 +0x24..) set to `v` [F 0x1E060EC / 0x1E0615C /
/// 0x1E06234]; `which` picks the components written.
fn movers(fx: &mut Fx, v: [Option<f32>; 3]) {
    for (c, v) in v.iter().enumerate() {
        if let Some(v) = v {
            let c = 4 * c as u32;
            for a in [FLASH + 0x4C + 0x2C, FLASH + 0x2C, SPAWNER + 0x2C] {
                fx.mem.setf(a + c, *v);
            }
        }
    }
    for k in 0x14..0x1Eu32 {
        for (c, v) in v.iter().enumerate() {
            if let Some(v) = v {
                fx.mem.setf(flicker_at(k) + 0x24 + 4 * c as u32, *v);
            }
        }
    }
}

fn bolts_still(fx: &mut Fx) {
    for k in 0..0x28u32 {
        for o in [0x38u32, 0x34, 0x30] {
            fx.mem.write_u32(BOLTS + 0x3C * k + o, 0);
        }
    }
}

/// Update 0x122 [F 0x1E06234]: the bolt burst's nodes, everything
/// stopped, four lenses, four bead beams, two flashes and a shake at the
/// target (sp + 0x50).
fn strike(fx: &mut Fx, sp: u32, param: i32) {
    bolt_burst(fx, 0, param as i64 as u64);
    bolts_still(fx);
    for a in [0xF00u32, 0xEB4, 0xB34, 0xEFC, 0xEB0, 0xB30, 0xEF8, 0xEAC, 0xB2C] {
        fx.mem.write_u32(D + a, 0);
    }
    for k in 0x14..0x1Eu32 {
        for o in [0x2Cu32, 0x28, 0x24] {
            fx.mem.write_u32(flicker_at(k) + o, 0);
        }
    }
    let at = sp + 0x50;
    for k in 0..4u32 {
        let g = LENSES + 0x40 * k;
        set_counter4(fx, g, 0xC, ((k * 4) & 0xFF) as i32, 0xFF);
        fx.mem.write_u8(g + 4, 0);
        fx.mem.write_u32(g + 8, 0x3F80_0000);
        fx.mem.write_u32(g + 0xC, 0x4270_0000);
        fx.mem.write_u32(g + 0x10, 0x3F00_0000);
        fx.mem.write_u32(g + 0x14, 0x3F4C_CCCD);
        fx.mem.write_u32(g + 0x18, 0x4170_0000);
        copy3(fx, at, g + 0x1C);
    }
    for k in 0..4 {
        bead(fx, 0x18, (k * 2) & 0xFF, 0xF, 0x1E, 0x01E0_63E8, 0x41A0_0000, 0x3FC0_0000, at);
    }
    for k in 0..2u32 {
        let r = FLASH + 0x4C * k;
        set_counter4(fx, r, 0x14, (k & 0xFF) as i32, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0x81);
        fx.mem.write_u8(r + 7, 9);
        let f = fx.mem.read_u8(r + 4);
        if k != 0 {
            fx.mem.write_u8(r + 4, f | 4);
            fx.mem.write_u32(r + 0x10, 0x4461_0000);
        } else {
            fx.mem.write_u8(r + 4, f | 2);
            fx.mem.write_u32(r + 0x10, 0x43C8_0000);
        }
        for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x20), (8, 0xA0)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x14, 0);
        copy4(fx, at, r + 0x1C);
        for o in [0x34u32, 0x30, 0x2C] {
            fx.mem.write_u32(r + o, 0);
        }
    }
    shake_start(fx, 0x18, 8, [0x3FC0_0000, 0x3F80_0000, 0x4000_0000]);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, SHAKE + 0x20, u64::from(D), 0x01E0_65D4);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    // The entry's s0 = D, s1 = the OT, s2 = the parameter, s3 the runner's.
    unit_point_saving(fx, 1, t, sp + 0x50, [u64::from(D), u64::from(OT), param as i64 as u64, 0], 0x01E0_57CC);
    // The entry's s2 / s3 after the update's case (the spawners' prologue
    // saves them): the parameter and the runner's 0 unless a case's loops
    // ran.
    let (s2, s3) = match fx.mem.u16(D) {
        0x15E => return 8,
        0x146 => {
            let t = fx.mem.i32(D + 8);
            fx.unit_add_mode(t, 0, 0, [u64::from(D), u64::from(OT)], 0x01E0_65F0);
            (param as i64 as u64, 0)
        }
        0x122 => {
            strike(fx, sp, param);
            (2, 0x4C)
        }
        0xE2 => {
            bolts_still(fx);
            let mut v = [0.0f32; 3];
            for (c, o) in [0xB20u32, 0xB24, 0xB28].into_iter().enumerate() {
                v[c] = ee::div(ee::sub(fx.mem.f(sp + 0x50 + 4 * c as u32), fx.mem.f(D + o)), 64.0);
            }
            movers(fx, [Some(v[0]), Some(v[1]), Some(v[2])]);
            (0x1E, 0)
        }
        0xD9 => {
            let mut f = f32::from_bits(0x3F4C_CCCD);
            if fx.mem.i32(D + 0x30) != 0x8000 {
                f = ee::mul(f, -1.0);
            }
            movers(fx, [None, None, Some(f)]);
            (0x1E, 0)
        }
        0x9A => {
            spawner_start(fx);
            (0x1E, 0x6CC)
        }
        0x8A => burst_start(fx),
        0x5A => {
            lens_start(fx, sp);
            (0x1E, 0x6CC)
        }
        2 => {
            start(fx, param);
            (0x14, 0x474)
        }
        _ => (param as i64 as u64, 0),
    };
    let n = fx.mem.u16(D);
    if (0x123..0x146).contains(&n) {
        bolt_burst(fx, 1, param as i64 as u64);
        for o in 0x8C..0x90u32 {
            fx.mem.write_u8(sp + o, 0x80);
        }
        let w = fx.mem.i32(TINT_WAIT);
        if w != 0 {
            fx.mem.set32(TINT_WAIT, w - 1);
        } else {
            let x = (fx.rand_from(D, 0x01E0_6668) >> 4) & 3;
            let t = fx.mem.i32(D + 8);
            if x == 0 {
                fx.unit_add_mode(t, 3, sp + 0x8C, [u64::from(D), u64::from(OT)], 0x01E0_668C);
                fx.mem.set32(TINT_WAIT, 1);
            } else {
                fx.unit_add_mode(t, 0, sp + 0x8C, [u64::from(D), u64::from(OT)], 0x01E0_66B0);
            }
        }
    }
    shake_saving(fx, SHAKE, u64::from(D), 0x01E0_66BC, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    blib::flicker::flickers(fx, 0x01E0_35E0, FLICKERS, OT, 0x1E);
    blib::beam::beads(fx, 0x01E0_0410, BEADS, OT, 4);
    // a3 as the draws leave it for the lenses' first TEXFLUSH: beads' spec
    // pointer, then any later writer's.
    let mut a3 = rel(fx, 0x01E0_0410, 0x48, 0x4C);
    if spawners(fx, SPAWNER, 3, [(0x60, 0x01E0_66F8), (0x50, 0), (0x40, s3), (0x30, s2), (0x20, u64::from(OT)), (0x10, u64::from(D))]) {
        a3 = 0xFF;
    }
    if let (_, Some(v)) = blib::star::flash_a3(fx, 0x01E0_2020, FLASH, OT, 9) {
        a3 = v;
    }
    let mut b = None;
    bolts(fx, 0x01E0_11E0, BOLTS, OT, 0x28, &mut b);
    if let Some(v) = b {
        a3 = v;
    }
    // bars: the projection's w pointer sp + 0x22C once a record counts.
    if (0..6u32).any(|k| fx.mem.read_u8(BARS + 0x50 * k) != 0 && fx.mem.read_u8(BARS + 0x50 * k + 1) == 0) {
        a3 = fx.sp - 0x230 + 0x22C;
    }
    blib::cross::bars(fx, 0x01E0_2BD0, BARS, OT, 6);
    lenses(fx, 0x01E0_40C0, LENSES, OT, 4, a3);
    let n = fx.mem.u16(D);
    if (0x122..0x143).contains(&n) {
        6
    } else if n == 0x143 {
        2
    } else if n == 0x118 {
        1
    } else {
        0
    }
}

/// 0x1E05140(D, spawn) [F]: with `spawn` four bolts (the first free of
/// forty each, else done) between the target's nodes D+0xC.. (positions
/// at sp + 0xA0 + 0x10 k): node 0 -> 1, a jittered one from node 1 (by
/// 200 |reach - |1 - 0||), node 2 -> 3, node 2 -> 4; then new nodes: 1
/// (clear), a random r in 3..count-1, a random s in 3..count-3 and s -+ 2.
fn bolt_burst(fx: &mut Fx, spawn: i32, caller_s2: u64) {
    let sp = fx.enter(0x100);
    // The s1 / s2 the node requests' frames save: the caller's (s1 = the
    // OT) without `spawn`, else the bolt counter and the last bolt's
    // offset 0x3C i.
    let mut s1_left = u64::from(OT);
    let mut s2 = caller_s2;
    if spawn != 0 {
        for s1 in 0..4u32 {
            s1_left = u64::from(s1);
            let i = first_free(fx, BOLTS, 0x3C, 0x28);
            if i < 0 {
                break;
            }
            s2 = u64::from(0x3C * i as u32);
            for k in 0..5u32 {
                let t = fx.mem.i32(D + 8);
                let n = fx.mem.i32(D + 0xC + 4 * k);
                // s0 = the free index, s1 = the bolt, s2 = the node.
                node_position_saving(fx, t, n, sp + 0xA0 + 0x10 * k, i as u64, Some(u64::from(s1)), 0x01E0_51D4);
            }
            let b = BOLTS + 0x3C * i as u32;
            set_counter4(fx, b, 0xC, 0, 0xFF);
            for o in [5u32, 4, 0xB, 0xC] {
                fx.mem.write_u8(b + o, 0);
            }
            for o in 0x14..0x18u32 {
                fx.mem.write_u8(b + o, 0x80);
            }
            for o in [0x38u32, 0x34, 0x30] {
                fx.mem.write_u32(b + o, 0);
            }
            match s1 {
                0 => {
                    fx.mem.write_u8(b + 8, 8);
                    fx.mem.write_u32(b + 0x10, 0x3F4C_CCCD);
                    fx.mem.write_u8(b + 0xD, 3);
                    copy3(fx, sp + 0xA0, b + 0x18);
                    copy3(fx, sp + 0xB0, b + 0x24);
                }
                1 => {
                    fx.mem.write_u8(b + 8, 6);
                    fx.mem.write_u32(b + 0x10, 0x3F99_999A);
                    fx.mem.write_u8(b + 0xD, 4);
                    copy3(fx, sp + 0xB0, b + 0x18);
                    for k in 0..3 {
                        let v = ee::sub(fx.mem.f(sp + 0xB0 + 4 * k), fx.mem.f(sp + 0xA0 + 4 * k));
                        fx.mem.setf(sp + 0xF0 + 4 * k, v);
                    }
                    let len = length3(fx, sp + 0xF0);
                    let d = ee::sub(fx.mem.f(REACH), len);
                    let d = if ee_lt(d, 0.0) {
                        let len = length3(fx, sp + 0xF0);
                        ee::neg(ee::sub(fx.mem.f(REACH), len))
                    } else {
                        let len = length3(fx, sp + 0xF0);
                        ee::sub(fx.mem.f(REACH), len)
                    };
                    let f20 = ee::mul(200.0, d);
                    // rand's s0: the free index, then the first remainder,
                    // then the second rand.
                    let mut s0 = i as u32;
                    let mut s5 = 0;
                    for k in 0..3u32 {
                        let ra = [0x01E0_53AC, 0x01E0_5414, 0x01E0_546C][k as usize];
                        let x = fx.rand_from(s0, ra) >> 4;
                        fx.fix_residue(f20, [0x01E0_53BC, 0x01E0_5424, 0x01E0_547C][k as usize]);
                        let (_, h) = fx.div(x, ee::fixsfsi(f20));
                        if k == 0 {
                            fx.fix_residue(f20, 0x01E0_53D8);
                            s5 = ee::fixsfsi(f20) >> 1;
                            s0 = h as u32;
                        } else {
                            s0 = x as u32;
                        }
                        let j = ee::div(ee::from_i32(h - s5), 100.0);
                        let v = ee::add(fx.mem.f(sp + 0xB0 + 4 * k), j);
                        fx.mem.setf(b + 0x24 + 4 * k, v);
                    }
                }
                _ => {
                    fx.mem.write_u8(b + 8, 8);
                    fx.mem.write_u32(b + 0x10, 0x3F4C_CCCD);
                    fx.mem.write_u8(b + 0xD, 3);
                    copy3(fx, sp + 0xC0, b + 0x18);
                    copy3(fx, sp + 0xB0 + 0x10 * s1, b + 0x24);
                }
            }
            s1_left = u64::from(s1 + 1);
        }
    }
    let t = fx.mem.i32(D + 8);
    let count = fx.unit_nodes(t);
    let sv = |s1: u64| [Some(u64::from(count as u32)), Some(s1), Some(s2)];
    let t = fx.mem.i32(D + 8);
    let r = request_node_saving(fx, t, 1, 1, sv(s1_left), 0x01E0_556C);
    fx.mem.set32(D + 0xC, r);
    let x = fx.rand_from(count as u32, 0x01E0_5578) >> 4;
    let s1 = fx.div(x, count - 3).1 + 3;
    let t = fx.mem.i32(D + 8);
    let r = request_node_saving(fx, t, s1, 0, sv(s1 as u32 as u64), 0x01E0_55A8);
    fx.mem.set32(D + 0x10, r);
    let x = fx.rand_from(count as u32, 0x01E0_55B4) >> 4;
    let s1 = fx.div(x, count - 5).1 + 3;
    for (o, node, ra) in [(0x14u32, s1, 0x01E0_55E4), (0x18, s1 - 2, 0x01E0_55FC), (0x1C, s1 + 2, 0x01E0_5614)] {
        let t = fx.mem.i32(D + 8);
        let r = request_node_saving(fx, t, node, 0, sv(s1 as u32 as u64), ra);
        fx.mem.set32(D + o, r);
    }
    fx.leave(0x100);
}

/// ((rand >> 4) % r - (r >> 1)) / 100 (rand called with s0 = `g` from
/// `ra`).
fn jitter(fx: &mut Fx, g: u32, ra: u32, r: i32) -> f32 {
    let x = fx.rand_from(g, ra) >> 4;
    let (_, h) = fx.div(x, r);
    ee::div(ee::from_i32(h - (r >> 1)), 100.0)
}

/// The record's 0x1E01CF0 [F] (emit::segment_spawns' routine at this
/// record's layouts): `spawners(g, _, n) -> 1` steps `n` spawner records
/// (0x38 bytes; a 0x80 frame): countdown; the position +0x20 moves by
/// +0x2C; on counts where count & +3 is set nothing more; else a free bolt
/// (0x3C bytes) in the pool *(+8) (+0xC of them, 0x101C30) counted +0x11
/// takes flags +4 / +5, +9 = +6, +8 / +0xB / +0xC / +0xD = +0xD / +0xE /
/// +0xF / +0x10, the amplitude +0x10 = +0x18, colour +0x14 = +0x1C,
/// velocity +0x30 = +0x2C; with +5 the position from a node (0x101BA0 via
/// sp + 0x70); a jitter j per axis (r = i32 +0x14); flag 2: the bolt's
/// +0x18 = j, else its ends +0x18 = position + j and +0x24 = position - j.
/// Returns whether a bolt was started (0x101AD0 leaves a3 = 0xFF).
fn spawners(fx: &mut Fx, g: u32, n: i32, saves: [(u32, u64); 6]) -> bool {
    let sp = fx.enter(0x80);
    fx.saved(sp, &saves);
    let mut g = g;
    let mut i = 0;
    let mut any = false;
    while i < n {
        if i != 0 {
            g += 0x38;
        }
        any |= spawn_one(fx, sp, g);
        i += 1;
    }
    fx.leave(0x80);
    any
}

fn spawn_one(fx: &mut Fx, sp: u32, g: u32) -> bool {
    if countdown(fx, g) == 0 {
        return false;
    }
    for k in 0..3 {
        let v = ee::add(fx.mem.f(g + 0x20 + 4 * k), fx.mem.f(g + 0x2C + 4 * k));
        fx.mem.setf(g + 0x20 + 4 * k, v);
    }
    if fx.mem.u8(g) & fx.mem.u8(g + 3) != 0 {
        return false;
    }
    let pool = fx.mem.read_u32(g + 8);
    let count = fx.mem.u8(g + 0xC);
    let i = first_free(fx, pool, 0x3C, count);
    if i < 0 {
        return false;
    }
    let b = fx.mem.read_u32(g + 8).wrapping_add((i as u32).wrapping_mul(0x3C));
    let c = fx.mem.u8(g + 0x11);
    set_counter4(fx, b, c, 0, 0xFF);
    for (dst, src) in [(5u32, 5u32), (4, 4), (9, 6), (8, 0xD), (0xB, 0xE), (0xC, 0xF), (0xD, 0x10)] {
        let v = fx.mem.read_u8(g + src);
        fx.mem.write_u8(b + dst, v);
    }
    let w = fx.mem.read_u32(g + 0x18);
    fx.mem.write_u32(b + 0x10, w);
    for k in 0..4u32 {
        let v = fx.mem.read_u8(g + 0x1C + k);
        fx.mem.write_u8(b + 0x14 + k, v);
    }
    copy3(fx, g + 0x2C, b + 0x30);
    if fx.mem.read_u8(g + 5) != 0 {
        node_source(fx, g, sp + 0x70);
        copy3(fx, sp + 0x70, g + 0x20);
    }
    let r = fx.mem.i32(g + 0x14);
    let jx = jitter(fx, g, 0x01E0_1E94, r);
    let r = fx.mem.i32(g + 0x14);
    let jy = jitter(fx, g, 0x01E0_1EE0, r);
    let r = fx.mem.i32(g + 0x14);
    let jz = jitter(fx, g, 0x01E0_1F2C, r);
    let j = [jx, jy, jz];
    if fx.mem.u8(g + 4) & 2 != 0 {
        for k in 0..3u32 {
            fx.mem.setf(b + 0x18 + 4 * k, j[k as usize]);
        }
    } else {
        for k in 0..3u32 {
            let v = ee::add(fx.mem.f(g + 0x20 + 4 * k), j[k as usize]);
            fx.mem.setf(b + 0x18 + 4 * k, v);
        }
        for k in 0..3u32 {
            let v = ee::sub(fx.mem.f(g + 0x20 + 4 * k), j[k as usize]);
            fx.mem.setf(b + 0x24 + 4 * k, v);
        }
    }
    true
}

fn fix16(fx: &mut Fx, a: u32, v: f32) {
    let x = ee::fixunssfsi(v);
    fx.mem.write_u16(a, x as u16);
}

fn swap64(fx: &mut Fx, sp: u32, a: u32, b: u32) {
    let v = fx.mem.read_u64(sp + a);
    fx.mem.write_u64(sp + 0x220, v);
    let v = fx.mem.read_u64(sp + b);
    fx.mem.write_u64(sp + a, v);
    let v = fx.mem.read_u64(sp + 0x220);
    fx.mem.write_u64(sp + b, v);
}

/// The record's 0x1E011E0 [F] (bolt.rs's `bolts` at this record's
/// layout): `bolts(recs, ot, n) -> 1` steps and draws `n` records (0x3C
/// bytes; a 0x330 frame, every local 0x10 below `bolts`': the strip
/// descriptor at sp + 0x298, the specs (`lui` / `addiu` at +0x44 / +0x48)
/// at sp + 0x260). The record: ends +0x18 / +0x24 moving by +0x30, the
/// amplitude +0x10, colour +0x14.. (no width growth); one strip a segment
/// of half width |AB| / +0xD, x 3 on odd counts.
fn bolts(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, a3: &mut Option<u32>) -> i32 {
    let spec = rel(fx, at, 0x44, 0x48);
    let sp = fx.enter(0x330);
    fx.mem.set32(sp + 0xCC, n);
    for q in 0..3u32 {
        for k in (0..16u32).step_by(8) {
            let v = fx.mem.read_u64(((spec + 0x10 * q) & !15) + k);
            fx.mem.write_u64(sp + 0x260 + 0x10 * q + k, v);
        }
    }
    let mut p = recs;
    fx.mem.set32(sp + 0xB0, 0);
    while fx.mem.i32(sp + 0xB0) < fx.mem.i32(sp + 0xCC) {
        if fx.mem.i32(sp + 0xB0) != 0 {
            p += 0x3C;
        }
        bolt(fx, sp, p, ot, a3);
        let v = fx.mem.i32(sp + 0xB0) + 1;
        fx.mem.set32(sp + 0xB0, v);
    }
    fx.leave(0x330);
    1
}

fn bolt(fx: &mut Fx, sp: u32, p: u32, ot: u32, a3: &mut Option<u32>) {
    if countdown(fx, p) == 0 {
        return;
    }
    let id = fx.mem.u8(p + 0xB);
    let tex = fx.texture(5, id);
    if fx.mem.read_u8(p + 5) != 0 {
        let v = fx.mem.read_u8(p + 9);
        fx.mem.write_u8(p + 6, v);
        node_source(fx, p, sp + 0x230);
        copy3(fx, sp + 0x230, sp + 0x240);
        if fx.mem.u8(p + 4) & 2 != 0 {
            for k in 0..3u32 {
                let v = ee::add(fx.mem.f(sp + 0x240 + 4 * k), fx.mem.f(p + 0x18 + 4 * k));
                fx.mem.setf(sp + 0x24C + 4 * k, v);
            }
        } else {
            let v = fx.mem.read_u8(p + 0xA);
            fx.mem.write_u8(p + 6, v);
            node_source(fx, p, sp + 0x230);
            copy3(fx, sp + 0x230, sp + 0x24C);
        }
    } else {
        for base in [0x18u32, 0x24] {
            for k in 0..3u32 {
                let v = ee::add(fx.mem.f(p + base + 4 * k), fx.mem.f(p + 0x30 + 4 * k));
                fx.mem.setf(p + base + 4 * k, v);
            }
        }
        copy3(fx, p + 0x18, sp + 0x240);
        copy3(fx, p + 0x24, sp + 0x24C);
    }
    camera_vp_into(fx, sp + 0xD0);
    let pts = fx.mem.u8(p + 8);
    fx.mem.set32(sp + 0x110, pts);
    let a = fx.mem.read_u32(p + 0x10);
    fx.mem.write_u32(sp + 0x114, a);
    fx.mem.write_u32(sp + 0x130, sp + 0x140);
    copy3(fx, sp + 0x240, sp + 0x118);
    copy3(fx, sp + 0x24C, sp + 0x124);
    blib::bolt::jag(fx, sp + 0x110);
    let d = sp + 0x298;
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    for (s, o) in [(0x16u32, 0u32), (0x15, 1), (0x14, 2)] {
        let v = fx.mem.read_u8(p + s);
        for b in [0x2F0u32, 0x2D8, 0x2C0, 0x2A8] {
            fx.mem.write_u8(sp + b + o, v);
        }
    }
    for o in [0x2F4u32, 0x2DC, 0x2C4, 0x2AC] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    let v = fx.mem.read_u8(p + 0x17);
    for o in [0x2F3u32, 0x2DB, 0x2C3, 0x2AB] {
        fx.mem.write_u8(sp + o, v);
    }
    let spec = sp + 0x260 + 0x18 * fx.mem.u8(p + 0xC) as u32;
    *a3 = Some(tex_rect_strip_a3(fx, d, spec, tex));
    let total = fx.mem.i32(sp + 0x110);
    let c = fx.mem.u8(p);
    let mut s1 = if c < total { total - c } else { 0 };
    let mut last = total;
    if fx.mem.u8(p + 4) & 1 != 0 {
        let left = fx.mem.u8(p + 2) - c;
        if left < total {
            last = left;
        }
    }
    while s1 < last {
        segment(fx, sp, p, ot, s1, a3);
        s1 += 1;
    }
}

fn segment(fx: &mut Fx, sp: u32, p: u32, ot: u32, s1: i32, a3: &mut Option<u32>) {
    let m = fx.read_m4(sp + 0xD0);
    let a = (sp + 0x140).wrapping_add((s1 as u32).wrapping_mul(12));
    *a3 = Some(sp + 0x328);
    if !fx.project_at(a, &m, sp + 0x210, sp + 0x328) || !Fx::on_screen(fx.mem.read_u64(sp + 0x210)) {
        return;
    }
    *a3 = Some(sp + 0x32C);
    if !fx.project_at(a + 12, &m, sp + 0x218, sp + 0x32C) || !Fx::on_screen(fx.mem.read_u64(sp + 0x218)) {
        return;
    }
    let dx = ee::from_i32((fx.mem.u16(sp + 0x210) - fx.mem.u16(sp + 0x218)).abs());
    let dy = ee::from_i32((fx.mem.u16(sp + 0x212) - fx.mem.u16(sp + 0x21A)).abs());
    let len = fx.sqrtf(ee::add(ee::mul(dx, dx), ee::mul(dy, dy)));
    let mut w = ee::div(len, ee::from_i32(fx.mem.u8(p + 0xD)));
    if fx.mem.u8(p) & 1 != 0 {
        w = ee::mul(w, 3.0);
    }
    let fl = |v: i32| ee::from_i32(v);
    if !ee_le(dx, dy) {
        let y0 = fx.mem.u16(sp + 0x212);
        fix16(fx, sp + 0x2BA, ee::sub(fl(y0), w));
        let y1 = fx.mem.u16(sp + 0x21A);
        fix16(fx, sp + 0x2D2, ee::sub(fl(y1), w));
        fix16(fx, sp + 0x2EA, ee::add(fl(y0), w));
        fix16(fx, sp + 0x302, ee::add(fl(y1), w));
        let x0 = fx.mem.read_u16(sp + 0x210);
        fx.mem.write_u16(sp + 0x2E8, x0);
        fx.mem.write_u16(sp + 0x2B8, x0);
        let x1 = fx.mem.read_u16(sp + 0x218);
        fx.mem.write_u16(sp + 0x300, x1);
        fx.mem.write_u16(sp + 0x2D0, x1);
    } else {
        let x0 = fx.mem.u16(sp + 0x210);
        fix16(fx, sp + 0x2B8, ee::sub(fl(x0), w));
        let x1 = fx.mem.u16(sp + 0x218);
        fix16(fx, sp + 0x2D0, ee::sub(fl(x1), w));
        fix16(fx, sp + 0x2E8, ee::add(fl(x0), w));
        fix16(fx, sp + 0x300, ee::add(fl(x1), w));
        let y0 = fx.mem.read_u16(sp + 0x212);
        fx.mem.write_u16(sp + 0x2EA, y0);
        fx.mem.write_u16(sp + 0x2BA, y0);
        let y1 = fx.mem.read_u16(sp + 0x21A);
        fx.mem.write_u16(sp + 0x302, y1);
        fx.mem.write_u16(sp + 0x2D2, y1);
    }
    let far = fx.mem.u8(p + 4) & 0x80 != 0;
    // The last `ld a3` of the z chain (far: vertex 2's low XYZ word, else
    // vertex 4's), then the last swapped pointer.
    *a3 = Some(fx.mem.read_u32(sp + if far { 0x2D0 } else { 0x300 }));
    let depth = if far {
        fx.set_xyz_z(sp + 0x300, 0xFF_FFFE);
        for (from, to) in [(0x300u32, 0x2E8u32), (0x2E8, 0x2D0), (0x2D0, 0x2B8)] {
            let z = fx.xyz_z(sp + from);
            fx.set_xyz_z(sp + to, z);
        }
        0.0
    } else {
        for (from, to) in [(0x210u32, 0x2D0u32), (0x2D0, 0x2B8), (0x218, 0x300), (0x300, 0x2E8)] {
            let z = fx.xyz_z(sp + from);
            fx.set_xyz_z(sp + to, z);
        }
        let (a, b) = (fx.mem.f(sp + 0x328), fx.mem.f(sp + 0x32C));
        if ee_lt(a, b) {
            a
        } else {
            b
        }
    };
    for o in [0x2D7u32, 0x2BF, 0x307, 0x2EF] {
        fx.mem.write_u8(sp + o, 0);
    }
    let turn = (fx.mem.u8(p) + s1) & 3;
    if turn != 0 {
        *a3 = Some(sp + if turn == 1 { 0x2E8 } else { 0x300 });
    }
    match turn {
        1 => {
            swap64(fx, sp, 0x2B8, 0x300);
            swap64(fx, sp, 0x2D0, 0x2E8);
        }
        2 => {
            swap64(fx, sp, 0x2B8, 0x2D0);
            swap64(fx, sp, 0x2E8, 0x300);
        }
        3 => {
            swap64(fx, sp, 0x2B8, 0x2E8);
            swap64(fx, sp, 0x2D0, 0x300);
        }
        _ => {}
    }
    let key = ee::fixunssfsi(depth) as i32;
    fx.submit(sp + 0x298, ot, key);
}

const PI: u32 = 0x4049_0FDB;
const ASPECT: u32 = 0x3FAA_3D71;

/// 0x1A2B80 (fixsfsi) from the `jal` at `jal`, its frame left.
fn fix_at(fx: &mut Fx, jal: u32, v: f32) -> i32 {
    fx.fix_residue(v, jal + 8);
    ee::fixsfsi(v)
}

/// 0x1A2C70 (fixunssfsi) from the `jal` at `jal` into the u16 at `a`.
fn fixu_at(fx: &mut Fx, jal: u32, a: u32, v: f32) {
    fx.fix_residue(v, jal + 8);
    let x = ee::fixunssfsi(v);
    fx.mem.write_u16(a, x as u16);
}

/// ST field (14 bits) of the halfword at `a` set to `v`.
fn st14(fx: &mut Fx, a: u32, v: i32) {
    let h = fx.mem.read_u16(a) & 0xC000;
    fx.mem.write_u16(a, h | (v as u16 & 0x3FFF));
}

/// The record's 0x1E040C0 [F]: `lenses(recs, ot, n) -> 1` (0x40-byte
/// records; a 0xA40 frame) in a packet layer (0x2078A0, closed into `ot`
/// at key 0 when any record counted): on the first counting record the
/// frame-copy quad (sp + 0x9A8: white, Q 1, far Z) after a TEXFLUSH, the
/// frame as its texture (0x2095A0 / 0x207F50, TEX0 / TEX1 at sp + 0xA20 /
/// 0xA28) and a TEXFLUSH. Per counting record: radii +8 / +0xC grow by
/// +0x18, the centre +0x1C by +0x2C (or from a node, 0x101BA0), projected
/// into +0x38 (w sp + 0xA3C; in the window, 0x101770); 36 ten-degree
/// points (sp + 0x270 + 0x30 k) on the inner (A) and outer (B) circles and
/// between them at +0x10 (C) and (D) at +0x14 (flag 1: the table sp +
/// 0x960's (+2 - +0) & 7 x 8 bytes, flag 2: & 15 x 4); two screen quads a
/// point (offsets x 16, y x 1.33): ST of C -> A drawn at D -> A, and B ->
/// C at B -> D, skipped when an ST is negative.
fn lenses(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, a3_in: u32) -> i32 {
    let table = rel(fx, at, 0x4C, 0x50);
    let sp = fx.enter(0xA40);
    fx.mem.write_u32(sp + 0x228, ot);
    fx.mem.set32(sp + 0x224, n);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xCC, layer);
    for k in (0..0x40u32).step_by(8) {
        let v = fx.mem.read_u64((table & !15) + k);
        fx.mem.write_u64(sp + 0x960 + k, v);
    }
    fx.mem.write_u32(sp + 0xD0, 0);
    let mut p = recs;
    fx.mem.set32(sp + 0xB0, 0);
    while fx.mem.i32(sp + 0xB0) < fx.mem.i32(sp + 0x224) {
        if fx.mem.i32(sp + 0xB0) != 0 {
            p += 0x40;
        }
        if countdown(fx, p) != 0 {
            if fx.mem.i32(sp + 0xD0) == 0 {
                lens_header(fx, sp, a3_in);
                fx.mem.set32(sp + 0xD0, 1);
            }
            lens(fx, sp, p);
        }
        let v = fx.mem.i32(sp + 0xB0) + 1;
        fx.mem.set32(sp + 0xB0, v);
    }
    if fx.mem.i32(sp + 0xD0) != 0 {
        let layer = fx.mem.read_u32(sp + 0xCC);
        fx.layer_end(layer, ot, 0);
    }
    fx.leave(0xA40);
    1
}

fn lens_header(fx: &mut Fx, sp: u32, a3: u32) {
    let d = sp + 0x9A8;
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    for v in [0xA00u32, 0x9E8, 0x9D0, 0x9B8] {
        for b in 0..4 {
            fx.mem.write_u8(sp + v + b, 0x80);
        }
    }
    for o in [0xA04u32, 0x9EC, 0x9D4, 0x9BC] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    let a2 = fx.mem.read_u32(sp + 0xA10);
    let a0 = fx.mem.read_u32(sp + 0x9C8);
    let v = fx.mem.read_u64(sp + 0xA10);
    let v = (v & crate::lens::KEEP) | (0xFF_FFFFu64 << 32);
    fx.mem.write_u64(sp + 0xA10, v);
    let z = ((fx.mem.read_u64(sp + 0xA10) << 8) >> 40) << 32;
    for o in [0x9F8u32, 0x9E0, 0x9C8] {
        let w = fx.mem.read_u64(sp + o);
        fx.mem.write_u64(sp + o, (w & crate::lens::KEEP) | z);
    }
    for o in [0xA17u32, 0x9FF, 0x9E7, 0x9CF] {
        fx.mem.write_u8(sp + o, 0);
    }
    let regs = crate::lens::frame_copy(fx, [a0, 0xFFFF_FFFF, a2, a3], 0, sp + 0xA20, sp + 0xA28);
    fx.texflush(regs);
}

fn lens(fx: &mut Fx, sp: u32, p: u32) {
    camera_vp_into(fx, sp + 0x230);
    for o in [8u32, 0xC] {
        let v = ee::add(fx.mem.f(p + o), fx.mem.f(p + 0x18));
        fx.mem.setf(p + o, v);
    }
    for k in 0..3 {
        let v = ee::add(fx.mem.f(p + 0x1C + 4 * k), fx.mem.f(p + 0x2C + 4 * k));
        fx.mem.setf(p + 0x1C + 4 * k, v);
    }
    node_source(fx, p, p + 0x1C);
    let m = fx.read_m4(sp + 0x230);
    if !fx.project_at(p + 0x1C, &m, p + 0x38, sp + 0xA3C) {
        return;
    }
    if !crate::b299::in_window(fx, p + 0x38) {
        return;
    }
    for s1 in 0..0x24u32 {
        let a = ee::mul(10.0, ee::from_i32(s1 as i32));
        let a = ee::div(ee::mul(f32::from_bits(PI), a), 180.0);
        let s = fx.libm_sinf(a);
        let c = libm_cosf(fx, a);
        let e = sp + 0x270 + 0x30 * s1;
        let (ra, rb) = (fx.mem.f(p + 8), fx.mem.f(p + 0xC));
        fx.mem.setf(e, ee::mul(ra, c));
        let ra = fx.mem.f(p + 8);
        fx.mem.setf(e + 4, ee::mul(ra, s));
        fx.mem.setf(e + 0xC, ee::mul(rb, c));
        let rb = fx.mem.f(p + 0xC);
        fx.mem.setf(e + 0x10, ee::mul(rb, s));
        let lerp = |fx: &mut Fx, t: f32, o: u32| {
            let (a, b) = (fx.mem.f(e + o), fx.mem.f(e + 0xC + o));
            ee::add(a, ee::mul(t, ee::sub(b, a)))
        };
        let t = fx.mem.f(p + 0x10);
        let v = lerp(fx, t, 0);
        fx.mem.setf(e + 0x18, v);
        let t = fx.mem.f(p + 0x10);
        let v = lerp(fx, t, 4);
        fx.mem.setf(e + 0x1C, v);
        let left = fx.mem.u8(p + 2) - fx.mem.u8(p);
        let t = match fx.mem.u8(p + 4) & 3 {
            1 => fx.mem.f(sp + 0x960 + 8 * (left & 7) as u32),
            2 => fx.mem.f(sp + 0x960 + 4 * (left & 0xF) as u32),
            _ => fx.mem.f(p + 0x14),
        };
        let v = lerp(fx, t, 0);
        fx.mem.setf(e + 0x24, v);
        let v = lerp(fx, t, 4);
        fx.mem.setf(e + 0x28, v);
    }
    for o in [0u32, 0xC, 0x18, 0x24] {
        copy3(fx, sp + 0x270 + o, sp + 0x930 + o);
    }
    let f20 = ee::from_i32(fx.mem.u16(p + 0x38));
    let f21 = ee::from_i32(fx.mem.u16(p + 0x3A));
    let d = sp + 0x9A8;
    let fl = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, fl | 0x800);
    let aspect = f32::from_bits(ASPECT);
    for s1 in 0..0x24u32 {
        let e = sp + 0x270 + 0x30 * s1;
        let n = e + 0x30;
        let sx = |fx: &mut Fx, jal: u32, a: u32| {
            let v = ee::add(ee::sub(f20, 28672.0), ee::mul(16.0, fx.mem.f(a)));
            fix_at(fx, jal, v)
        };
        let sy = |fx: &mut Fx, jal: u32, a: u32| {
            let v = ee::add(ee::sub(f21, 28672.0), ee::mul(aspect, ee::mul(16.0, fx.mem.f(a))));
            fix_at(fx, jal, v)
        };
        let px = |fx: &mut Fx, jal: u32, out: u32, a: u32| {
            let v = ee::add(f20, ee::mul(16.0, fx.mem.f(a)));
            fixu_at(fx, jal, sp + out, v);
        };
        let py = |fx: &mut Fx, jal: u32, out: u32, a: u32| {
            let v = ee::add(f21, ee::mul(aspect, ee::mul(16.0, fx.mem.f(a))));
            fixu_at(fx, jal, sp + out, v);
        };
        // ST of C -> A, drawn at D -> A.
        let st = [
            sx(fx, 0x01E0_4698, e + 0x18),
            sx(fx, 0x01E0_46D8, n + 0x18),
            sx(fx, 0x01E0_4718, e),
            sx(fx, 0x01E0_4758, n),
            sy(fx, 0x01E0_47AC, e + 0x1C),
            sy(fx, 0x01E0_4800, n + 0x1C),
            sy(fx, 0x01E0_4854, e + 4),
            sy(fx, 0x01E0_48A8, n + 4),
        ];
        if st.iter().all(|&v| v >= 0) {
            for (k, o) in [0x9C0u32, 0x9D8, 0x9F0, 0xA08].into_iter().enumerate() {
                st14(fx, sp + o, st[k]);
                st14(fx, sp + o + 2, st[4 + k]);
            }
            px(fx, 0x01E0_4A20, 0x9C8, e + 0x24);
            px(fx, 0x01E0_4A54, 0x9E0, n + 0x24);
            px(fx, 0x01E0_4A7C, 0x9F8, e);
            px(fx, 0x01E0_4AA4, 0xA10, n);
            py(fx, 0x01E0_4AEC, 0x9CA, e + 0x28);
            py(fx, 0x01E0_4B34, 0x9E2, n + 0x28);
            py(fx, 0x01E0_4B70, 0x9FA, e + 4);
            py(fx, 0x01E0_4BAC, 0xA12, n + 4);
            fx.submit(d, 0, 0);
        } else {
            continue;
        }
        // ST of B -> C, drawn at B -> D.
        let st = [
            sx(fx, 0x01E0_4C04, e + 0xC),
            sx(fx, 0x01E0_4C44, n + 0xC),
            sx(fx, 0x01E0_4C74, e + 0x18),
            sx(fx, 0x01E0_4CA4, n + 0x18),
            sy(fx, 0x01E0_4CF8, e + 0x10),
            sy(fx, 0x01E0_4D44, n + 0x10),
            sy(fx, 0x01E0_4D88, e + 0x1C),
            sy(fx, 0x01E0_4DCC, n + 0x1C),
        ];
        if !st.iter().all(|&v| v >= 0) {
            continue;
        }
        for (k, o) in [0x9C0u32, 0x9D8, 0x9F0, 0xA08].into_iter().enumerate() {
            st14(fx, sp + o, st[k]);
            st14(fx, sp + o + 2, st[4 + k]);
        }
        px(fx, 0x01E0_4F18, 0x9C8, e + 0xC);
        px(fx, 0x01E0_4F40, 0x9E0, n + 0xC);
        px(fx, 0x01E0_4F68, 0x9F8, e + 0x24);
        px(fx, 0x01E0_4F90, 0xA10, n + 0x24);
        py(fx, 0x01E0_4FCC, 0x9CA, e + 0x10);
        py(fx, 0x01E0_5004, 0x9E2, n + 0x10);
        py(fx, 0x01E0_5040, 0x9FA, e + 0x28);
        py(fx, 0x01E0_507C, 0xA12, n + 0x28);
        fx.submit(d, 0, 0);
    }
}
