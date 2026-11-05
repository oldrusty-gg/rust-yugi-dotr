//! BMODULE b853 (card 682), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05850, data area a0 = 0x1ED0000, 0x5C00 bytes)
//! [F]: b370 / b408's family with s6 = D, s7 =
//! the OT (0x206E40), s0 = the parameter then pointers, s1..s5 counters /
//! pointers, f20..f22 the point step. Update 2 four bands (+0x45E0,
//! `band::bands_b`, turned 0 / 0x2000 / 0x4000 / 0x6000 with their own
//! spins) on node 0x38 and a long shake there; 80 a flash (10 -> 30) and
//! a long flash (90) on node 0xA3, a long shake; 174 and 324 twelve
//! sprites (+0xA4, `drop::drops`) at node 0xA4 thrown 0.8 x towards node
//! 0xA5; 186 the frame rays (+0x3748, `window::frame_rays_b`, 24 counts)
//! at node 0xA3; 198 four jagged bolts (0x102A50 `bolt::jag`, +0x3778) from
//! node 0xA3 to the target, a scrolling tube (+0x5AD0, `segtube::tubes`) on
//! 48 points (+0x5890) node -> target and 48 bands (+0x4750) along them;
//! 250 the fade (+0x4358), a dome (+0x4360, `dome::domes`) 150 before /
//! behind the target by the facing (D+0x30), a flash and a shake at the
//! target lasting to 350; 328 the fade again (0x82). Even updates 2..320 a
//! spike on node 0x38 (+0x15E4), 80..320 one on node 0xA3 (longer from
//! 187), 250..340 one at the target every update; 97..151 six sprites a
//! update from node 0xA3. Returns 6 on updates 250..346, 2 on 347, 1 on
//! 240, 8 on 400 (no drawing then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `flash_b`
//! 0x1E003A0, `spikes` 0x1E00E10, `drops` 0x1E018F0, `frame_rays_b`
//! 0x1E02250, `fade` 0x1E02AC0, `domes` 0x1E02F90, `bolts_b` 0x1E039D0,
//! `segment_spawns_b` 0x1E04460, `tubes` 0x1E047A0, `bands_b` 0x1E05110
//! [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const DROPS: u32 = D + 0xA4;
const SPIKES: u32 = D + 0x15E4;
const FLASH: u32 = D + 0x3664;
const RAYS: u32 = D + 0x3748;
const BOLTS: u32 = D + 0x3778;
const EMIT: u32 = D + 0x4178;
const FADE: u32 = D + 0x4358;
const DOMES: u32 = D + 0x4360;
const BANDS_A: u32 = D + 0x45E0;
const BANDS_B: u32 = D + 0x4750;
const PTS: u32 = D + 0x5890;
const TUBE: u32 = D + 0x5AD0;
const BYTES: u32 = D + 0x5AFC;

/// The entry's saved registers s0..s5 (64 bits) and f20 / f21 as the
/// callees' prologues save them.
#[derive(Clone, Copy)]
struct R {
    s: [u64; 6],
    f: [u32; 2],
}

/// The entry 0x1E05850 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1E0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x1E0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    // 0x199208 memset(D, 0, 0x5C00).
    fx.bzero(D, 0x5C00);
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
    for (o, node, clear) in [(0xCu32, 0x38, 1), (0x10, 0xA3, 0), (0x14, 0xA4, 0), (0x18, 0xA5, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + o, r);
    }
    fx.mem.set16(D + 0xA0, 0x50);
    fx.mem.set16(D + 0xA2, 0x18A);
    // The entry's s0 = 4 (the texture loop's), s1 / s2 the runner's (0).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_5980, [Some(4), Some(0), Some(0)]);
    0
}

/// rand() >> 4 with the entry's s0 saved in rand's frame.
fn rand4(fx: &mut Fx, s0: u64, ra: u32) -> i32 {
    let f = fx.sp - 0x10;
    fx.mem.write_u64(f, s0);
    fx.mem.write_u64(f + 8, u64::from(ra));
    fx.rand() >> 4
}

/// node_position(self, D[at], out) with the entry's s0 / s1.
fn node_at(fx: &mut Fx, at: u32, out: u32, r: &R, ra: u32) {
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + at));
    node_position_saving(fx, h, s, out, r.s[0], Some(r.s[1]), ra);
}

/// A flash record's fields: kind `k` (+4), unit flag `unit` (+5, node
/// D[`node`] at +6), colour 0x80 / `c`, size `size`.
fn flash_fields(fx: &mut Fx, f: u32, unit: u8, node: u32, k: u8, c: u8, size: u32) {
    fx.mem.write_u8(f + 5, unit);
    if unit != 0 {
        let v = fx.mem.read_u8(D + node);
        fx.mem.write_u8(f + 6, v);
    }
    for (o, v) in [(4u32, k), (7, 9), (0xA, 0x40), (9, 0x20), (8, 0x80), (0xF, c), (0xC, c), (0xD, c), (0xE, c)] {
        fx.mem.write_u8(f + o, v);
    }
    fx.mem.write_u32(f + 0x10, size);
}

/// The shake record: count, unit flag / node (`node`: D offset, or none),
/// mode, the three scales, the two amplitudes and +0x90.
fn shake(fx: &mut Fx, count: i32, node: Option<u32>, mode: u8, sc: [u32; 3], amp: u32, v90: u32) {
    set_counter4(fx, D + 0x40, count, 0, 0xFF);
    match node {
        Some(o) => {
            fx.mem.write_u8(D + 0x45, 1);
            let v = fx.mem.read_u8(D + o);
            fx.mem.write_u8(D + 0x46, v);
        }
        None => fx.mem.write_u8(D + 0x45, 0),
    }
    fx.mem.write_u8(D + 0x44, mode);
    for (k, v) in sc.into_iter().enumerate() {
        fx.mem.write_u32(D + 0x54 + 4 * k as u32, v);
    }
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, amp);
    fx.mem.write_u32(D + 0x90, v90);
}

/// Update 2 [F 0x1E05A1C]: four bands on node 0x38, turned 0x2000 k about
/// z with their own spins, and a long shake there.
fn start(fx: &mut Fx, r: &mut R) {
    for s1 in 0..4u32 {
        r.s[1] = u64::from(s1);
        let i = first_free(fx, BANDS_A, 0x5C, 4);
        if i < 0 {
            continue;
        }
        let s0 = D + 0x5C * i as u32;
        r.s[0] = u64::from(s0);
        let g = s0 + 0x45E0;
        set_counter4(fx, g, 0xFF, 0, 0xFF);
        fx.mem.write_u8(g + 5, 1);
        let v = fx.mem.read_u8(D + 0xC);
        fx.mem.write_u8(g + 6, v);
        for (o, v) in [(7u32, 0x24u8), (0xC, 2), (8, 0), (9, 2), (0xA, 0), (0xB, 4)] {
            fx.mem.write_u8(g + o, v);
        }
        fx.mem.write_u32(g + 0x10, 0x3F99_999A);
        fx.mem.write_u32(g + 0x14, 0x3F4C_CCCD);
        for o in [0x28u32, 0x29, 0x2A, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(g + o, 0xC0);
        }
        fx.mem.write_u8(g + 0x27, 0x40);
        fx.mem.write_u8(g + 0x2B, 0x60);
        fx.mem.set32(g + 0x44, 0x2000 * s1 as i32);
        fx.mem.write_u32(g + 0x3C, 0);
        fx.mem.write_u32(g + 0x40, 0);
        match s1 {
            0 => fx.mem.set32(g + 0x54, 0x800),
            1 => fx.mem.set32(g + 0x4C, 0x600),
            2 => fx.mem.set32(g + 0x50, 0xA00),
            _ => fx.mem.set32(g + 0x4C, -0x600),
        }
        fx.mem.write_u32(g + 0x18, 0);
        fx.mem.write_u32(g + 0x1C, 0);
    }
    r.s[1] = 4;
    shake(fx, 0xFF, Some(0xC), 4, [0x3F99_999A, 0x3F99_999A, 0x3F80_0000], 0x41F0_0000, 0x4000_0000);
}

/// Update 80 [F 0x1E05BE8]: two flashes and a long shake on node 0xA3.
fn flashes(fx: &mut Fx) {
    let f = FLASH;
    set_counter4(fx, f, 0x10, 0, 0xFF);
    flash_fields(fx, f, 1, 0x10, 0x81, 0xF0, 0x4120_0000);
    fx.mem.write_u32(f + 0x14, 0x41F0_0000);
    fx.mem.write_u32(f + 0x18, 0x4080_0000);
    let f = FLASH + 0x4C;
    set_counter4(fx, f, 0xFF, 0, 0xFF);
    flash_fields(fx, f, 1, 0x10, 0x83, 0xE0, 0x42B4_0000);
    fx.mem.write_u32(f + 0x14, 0);
    shake(fx, 0xFF, Some(0x10), 4, [0x3F99_999A, 0x3F99_999A, 0x3F80_0000], 0x4220_0000, 0x4080_0000);
}

/// Updates 174 / 324 [F 0x1E05D24]: twelve sprites at node 0xA4 (sp +
/// 0xC0) thrown 0.8 x towards node 0xA5 (sp + 0xD0) with a jitter.
fn throw(fx: &mut Fx, sp: u32, r: &mut R) {
    node_at(fx, 0x14, sp + 0xC0, r, 0x01E0_5D34);
    r.s[5] = u64::from(sp + 0xD0);
    node_at(fx, 0x18, sp + 0xD0, r, 0x01E0_5D48);
    for s0 in 0..0xCu32 {
        r.s[0] = u64::from(s0);
        let i = first_free(fx, DROPS, 0x44, 0x50);
        if i < 0 {
            continue;
        }
        let s1 = D + 0x44 * i as u32;
        r.s[1] = u64::from(s1);
        let p = s1 + 0xA4;
        set_counter4(fx, p, 0x1C, s0 as i32, 0xFF);
        let v = (rand4(fx, r.s[0], 0x01E0_5D90) & 3) << 4;
        fx.mem.set8(p + 4, v);
        for (o, v) in [(8u32, 1u8), (0xB, 1), (9, 2), (0xA, 5), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80)] {
            fx.mem.write_u8(p + o, v);
        }
        fx.mem.write_u32(p + 0x10, 0x40E0_0000);
        fx.mem.write_u32(p + 0x14, 0x3FF3_3333);
        fx.mem.write_u32(p + 0x1C, 0);
        fx.mem.write_u32(p + 0x18, 0);
        for c in 0..3u32 {
            let v = fx.mem.read_u32(sp + 0xC0 + 4 * c);
            fx.mem.write_u32(p + 0x20 + 4 * c, v);
        }
        r.s[2] = u64::from(sp + 0xC4);
        r.s[3] = u64::from(sp + 0xC8);
        for (c, ra) in [(0u32, 0x01E0_5E10), (4, 0x01E0_5E70), (8, 0x01E0_5ED0)] {
            let m = (rand4(fx, r.s[0], ra) & 7) - 4;
            let j = ee::div(ee::from_i32(m), 100.0);
            let d = ee::sub(fx.mem.f(sp + 0xD0 + c), fx.mem.f(sp + 0xC0 + c));
            let v = ee::add(ee::mul(f32::from_bits(0x3F4C_CCCD), d), j);
            fx.mem.setf(p + 0x2C + c, v);
            if c == 0 {
                r.s[4] = u64::from(p + 0x2C);
            } else if c == 4 {
                r.s[2] = u64::from(p + 0x30);
            }
        }
        for c in 0..3u32 {
            let v = ee::neg(ee::div(fx.mem.f(p + 0x2C + 4 * c), 22.0));
            fx.mem.setf(p + 0x38 + 4 * c, v);
        }
    }
    r.s[0] = 0xC;
}

/// Update 186 [F 0x1E05F7C]: the frame rays at node 0xA3 for 24 counts.
fn rays(fx: &mut Fx, r: &R) {
    set_counter4(fx, RAYS, 0x18, 0, 0xFF);
    node_at(fx, 0x10, RAYS + 0xC, r, 0x01E0_5F9C);
    for (o, v) in [(0x1Cu32, 0x80u8), (0x1D, 0x80), (0x1E, 0x80), (0x1F, 0x70), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x40), (8, 4), (9, 2), (0xA, 0xFF)] {
        fx.mem.write_u8(RAYS + o, v);
    }
}

/// Update 198 [F 0x1E05FE4]: four jagged bolts (six segments each) from
/// node 0xA3 (sp + 0xC0) to the target (sp + 0xB0), the tube on 48 points
/// from the node to the target and 48 bands along them (sp + 0xD0 the
/// node again, f20..f22 the step).
fn bolts(fx: &mut Fx, sp: u32, r: &mut R) {
    node_at(fx, 0x10, sp + 0xC0, r, 0x01E0_5FF4);
    let p = sp + 0xE0;
    let mut s0 = 0u32;
    while s0 < 4 {
        fx.mem.set32(p, 6);
        fx.mem.write_u32(p + 4, 0x3E61_47AE);
        fx.mem.write_u32(p + 0x20, sp + 0x110);
        for (dst, src) in [(0x8u32, 0xC0u32), (0xC, 0xC4), (0x10, 0xC8), (0x14, 0xB0), (0x18, 0xB4), (0x1C, 0xB8)] {
            let w = fx.mem.read_u32(sp + src);
            fx.mem.write_u32(p + dst, w);
        }
        blib::bolt::jag(fx, p);
        r.s = [u64::from(s0), u64::from(s0), 0, u64::from(sp + 0x110), r.s[4], 0];
        let mut broke = false;
        for s5 in 0..6u32 {
            let i = first_free(fx, BOLTS, 0x40, 0x28);
            if i < 0 {
                broke = true;
                break;
            }
            let s4 = D + ((i as u32) << 6);
            r.s[4] = u64::from(s4);
            let b = s4 + 0x3778;
            let count = (0x94 - (8 * s5 as i32 - s0 as i32)) & 0xFF;
            set_counter4(fx, b, count, (s0 + 8 * s5) as i32 & 0xFF, 0xFF);
            fx.mem.write_u8(b + 5, 0);
            fx.mem.write_u8(b + 4, 1);
            for c in 0..6u32 {
                let w = fx.mem.read_u32(sp + 0x110 + 0xC * s5 + 4 * c);
                fx.mem.write_u32(b + 0x1C + 4 * c, w);
            }
            fx.mem.write_u8(b + 8, 0xC);
            fx.mem.write_u32(b + 0x10, 0x3ECC_CCCD);
            fx.mem.write_u32(b + 0x14, 0x3ECC_CCCD);
            for (o, v) in [(0xBu32, 2u8), (0xD, 6), (0xC, 0), (0x18, 0xC0), (0x19, 0xC0), (0x1A, 0xC0), (0x1B, 0x20)] {
                fx.mem.write_u8(b + o, v);
            }
            r.s[1] = u64::from(s0 + 8 * (s5 + 1));
            r.s[2] = u64::from(8 * (s5 + 1));
            r.s[3] = u64::from(sp + 0x110 + 0xC * (s5 + 1));
            r.s[5] = u64::from(s5 + 1);
        }
        let _ = broke;
        s0 += 1;
    }
    let t = TUBE;
    set_counter4(fx, t, 0x8C, 0, 0xFF);
    for (o, v) in [(7u32, 0xCu8), (8, 0x2F), (0xB, 0x28), (9, 3), (0xA, 2)] {
        fx.mem.write_u8(t + o, v);
    }
    fx.mem.write_u32(t + 0xC, PTS);
    fx.mem.write_u32(t + 0x14, 0x3F80_0000);
    fx.mem.write_u32(t + 0x18, 0);
    fx.mem.write_u32(t + 0x1C, 0x3E99_999A);
    fx.mem.write_u32(t + 0x10, 0x3F80_0000);
    fx.mem.write_u32(t + 0x20, 0x3F00_0000);
    for o in [0x28u32, 0x29, 0x2A] {
        fx.mem.write_u8(t + o, 0x80);
    }
    fx.mem.write_u8(t + 0x2B, 0x60);
    fx.mem.write_u32(t + 0x24, 0);
    r.s[0] = 4;
    node_at(fx, 0x10, sp + 0xC0, r, 0x01E0_61F0);
    copy_v4(fx, sp + 0xC0, sp + 0xD0);
    let step: [f32; 3] = std::array::from_fn(|c| {
        let c = 4 * c as u32;
        ee::div(ee::sub(fx.mem.f(sp + 0xB0 + c), fx.mem.f(sp + 0xC0 + c)), 48.0)
    });
    r.f = [step[0].to_bits(), step[1].to_bits()];
    for s2 in 0..0x30u32 {
        let s0 = D + 0x5C * s2;
        r.s[0] = u64::from(s0);
        r.s[1] = u64::from(D + 0xC * s2);
        r.s[2] = u64::from(s2);
        let g = s0 + 0x4750;
        set_counter4(fx, g, 0x1C, s2 as i32, 0xFF);
        for (o, v) in [(5u32, 0u8), (7, 0x18), (0xC, 2), (8, 0), (9, 2), (0xA, 0), (0xB, 4)] {
            fx.mem.write_u8(g + o, v);
        }
        fx.mem.write_u32(g + 0x10, 0x3E99_999A);
        fx.mem.write_u32(g + 0x14, 0x3DCC_CCCD);
        for o in [0x2Bu32, 0x27, 0x28, 0x29, 0x2A, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(g + o, 0x80);
        }
        fx.mem.set32(g + 0x3C, 0x4000);
        fx.mem.write_u32(g + 0x44, 0);
        fx.mem.write_u32(g + 0x40, 0);
        let v = rand4(fx, r.s[0], 0x01E0_62F4);
        let (_, m) = fx.div(v, 0x14);
        let j = ee::div(ee::from_i32(m - 10), 100.0);
        fx.mem.setf(g + 0x18, ee::add(f32::from_bits(0x3E4C_CCCD), j));
        let v = rand4(fx, r.s[0], 0x01E0_633C);
        let fk = ee::from_i32(s2 as i32);
        let d: [f32; 3] = std::array::from_fn(|c| ee::mul(step[c], fk));
        let (_, m) = fx.div(v, 0x14);
        let j = ee::div(ee::from_i32(m - 10), 100.0);
        fx.mem.setf(g + 0x1C, ee::add(f32::from_bits(0x3DCC_CCCD), j));
        for c in 0..3u32 {
            let v = ee::add(fx.mem.f(sp + 0xC0 + 4 * c), d[c as usize]);
            fx.mem.setf(g + 0x2C + 4 * c, v);
            fx.mem.setf(PTS + 0xC * s2 + 4 * c, v);
        }
    }
    r.s[0] = u64::from(D + 0x5C * 0x30);
    r.s[1] = u64::from(D + 0xC * 0x30);
    r.s[2] = 0x30;
}

/// Update 250 [F 0x1E063E0]: the fade, a dome 150 before / behind the
/// target (by D+0x30 = 0x8000), a flash (to update 350) and a shake there.
fn strike(fx: &mut Fx, sp: u32) {
    set_counter4(fx, FADE, 0x12, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    let d = DOMES;
    set_counter4(fx, d, 0x64, 0, 0xFF);
    for (o, v) in [(0xAu32, 0xDu8), (7, 9), (8, 3), (9, 2)] {
        fx.mem.write_u8(d + o, v);
    }
    fx.mem.write_u32(d + 0xC, 0x40A0_0000);
    fx.mem.write_u32(d + 0x10, 0xBD23_D70A);
    fx.mem.write_u32(d + 0x14, 0x4316_0000);
    fx.mem.write_u32(d + 0x18, 0);
    copy_v4(fx, sp + 0xB0, d + 0x20);
    for o in [0x1Cu32, 0x1D, 0x1E, 0x1F] {
        fx.mem.write_u8(d + o, 0x80);
    }
    copy_v4(fx, sp + 0xB0, d + 0x20);
    let far = fx.mem.f(d + 0x14);
    let z = fx.mem.f(d + 0x28);
    if fx.mem.read_u32(D + 0x30) == 0x8000 {
        fx.mem.set32(d + 0x30, -0x4000);
        fx.mem.setf(d + 0x28, ee::sub(z, far));
    } else {
        fx.mem.set32(d + 0x30, 0x4000);
        fx.mem.setf(d + 0x28, ee::add(z, far));
    }
    let n = fx.mem.u16(D);
    let f = FLASH + 0x98;
    set_counter4(fx, f, (0x15E - n) & 0xFF, 0, 0xFF);
    fx.mem.write_u8(f + 5, 0);
    for (o, v) in [(4u32, 0x83u8), (7, 9), (0xA, 0x60), (9, 0x10), (8, 0xC0), (0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0)] {
        fx.mem.write_u8(f + o, v);
    }
    fx.mem.write_u32(f + 0x10, 0x447A_0000);
    fx.mem.write_u32(f + 0x14, 0);
    copy_v4(fx, sp + 0xB0, f + 0x1C);
    let n = fx.mem.u16(D);
    shake(fx, (0x15E - n) & 0xFF, None, 8, [0x3FCC_CCCD, 0x3FCC_CCCD, 0x3FCC_CCCD], 0x4220_0000, 0x40C0_0000);
    copy_v4(fx, sp + 0xB0, D + 0x60);
}

/// A spike record [F 0x1E06644 / 0x1E06780 / 0x1E068E8]: kind `k`, unit
/// flag / node, length `len`, the two sizes, alpha / far alpha, +0xC,
/// `pos`: the target point copied in.
#[allow(clippy::too_many_arguments)]
fn spike(fx: &mut Fx, r: &mut R, k: u8, node: Option<u32>, len: u8, sizes: [u32; 2], alphas: [u8; 2], c: u8, pos: Option<u32>, ra: [u32; 4]) {
    let i = first_free(fx, SPIKES, 0x68, 0x50);
    if i < 0 {
        return;
    }
    let s0 = 0x68 * i as u32;
    r.s[0] = u64::from(s0);
    let q = SPIKES + s0;
    set_counter4(fx, q, 0x10, 0, 0xFF);
    fx.mem.write_u8(q + 4, k);
    if pos.is_none() {
        fx.mem.write_u32(q + 0x14, sizes[0]);
        fx.mem.write_u32(q + 0x1C, sizes[1]);
    }
    fx.mem.write_u8(q + 8, len);
    match node {
        Some(o) => {
            fx.mem.write_u8(q + 5, 1);
            let v = fx.mem.read_u8(D + o);
            fx.mem.write_u8(q + 6, v);
        }
        None => fx.mem.write_u8(q + 5, 0),
    }
    for (o, v) in [(0xAu32, 0u8), (0xB, 2), (0xC, c)] {
        fx.mem.write_u8(q + o, v);
    }
    let x = rand4(fx, r.s[0], ra[0]);
    fx.mem.set8(q + 9, x);
    fx.mem.write_u32(q + 0x10, BYTES);
    fx.mem.write_u32(q + 0x18, 0x3F80_0000);
    for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, alphas[0]), (0x27, alphas[1])] {
        fx.mem.write_u8(q + o, v);
    }
    if let Some(p) = pos {
        fx.mem.write_u32(q + 0x14, sizes[0]);
        fx.mem.write_u32(q + 0x1C, sizes[1]);
        copy_v4(fx, p, q + 0x28);
    }
    for (o, ra) in [(0x58u32, ra[1]), (0x5C, ra[2]), (0x60, ra[3])] {
        let v = rand4(fx, r.s[0], ra) & 0x7F;
        fx.mem.set32(q + o, v);
    }
}

/// ((rand >> 4) & 7) x 200 - 800) / 100.
fn jit(fx: &mut Fx, s0: u64, ra: u32) -> f32 {
    let m = rand4(fx, s0, ra) & 7;
    ee::div(ee::from_i32(200 * m - 0x320), 100.0)
}

/// Updates 97..151 [F 0x1E06A30]: six sprites from node 0xA3, thrown
/// outwards. Returns the entry's f20.
fn sprites(fx: &mut Fx, sp: u32, r: &mut R) {
    for _s3 in 0..6u32 {
        let i = first_free(fx, DROPS, 0x44, 0x50);
        r.s[0] = i as i64 as u64;
        if i < 0 {
            continue;
        }
        node_at(fx, 0x10, sp + 0xC0, r, 0x01E0_6A60);
        let v = rand4(fx, r.s[0], 0x01E0_6A68);
        let (_, m) = fx.div(v, 6);
        let f20 = ee::add(16.0, ee::div(ee::from_i32(200 * m - 0x258), 100.0));
        r.f[0] = f20.to_bits();
        fx.fix_residue(f20, 0x01E0_6AC0);
        let count = ee::fixsfsi(f20) & 0xFF;
        let s0 = D + 0x44 * i as u32;
        r.s[0] = u64::from(s0);
        let p = s0 + 0xA4;
        set_counter4(fx, p, count, 0, 0xFF);
        fx.mem.write_u8(p + 4, 0x40);
        fx.mem.write_u8(p + 8, 0);
        fx.mem.write_u8(p + 0xB, 2);
        fx.mem.write_u8(p + 9, 4);
        fx.mem.write_u8(p + 0xA, 0);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(p + o, 0x80);
        }
        fx.mem.write_u32(p + 0x10, 0x4198_0000);
        fx.mem.write_u32(p + 0x14, 0xBF00_0000);
        fx.mem.write_u32(p + 0x1C, 0);
        fx.mem.write_u32(p + 0x18, 0);
        for (c, ra) in [(0u32, 0x01E0_6B34), (4, 0x01E0_6B90), (8, 0x01E0_6BEC)] {
            let f = jit(fx, r.s[0], ra);
            fx.mem.setf(sp + 0xD0 + c, f);
        }
        r.s[1] = u64::from(sp + 0xD0);
        r.s[2] = u64::from(sp + 0xD4);
        for c in 0..3u32 {
            let v = ee::add(fx.mem.f(sp + 0xC0 + 4 * c), fx.mem.f(sp + 0xD0 + 4 * c));
            fx.mem.setf(p + 0x20 + 4 * c, v);
        }
        for c in 0..3u32 {
            let v = ee::neg(ee::div(fx.mem.f(sp + 0xD0 + 4 * c), f20));
            fx.mem.setf(p + 0x2C + 4 * c, v);
        }
        for o in [0x40u32, 0x3C, 0x38] {
            fx.mem.write_u32(p + o, 0);
        }
    }
    r.s[3] = 6;
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    // The entry's s0 = the parameter (64 bits), s1..s5 the runner's 0,
    // f20..f22 as the entry left them (0 unless set this update).
    let mut r = R { s: [param as i64 as u64, 0, 0, 0, 0, 0], f: [0, 0] };
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0xB0, r.s[0], 0x01E0_59A0);
    match fx.mem.u16(D) {
        0x190 => return 8,
        0x148 => {
            set_counter4(fx, FADE, 0x18, 0, 0xFF);
            fx.mem.write_u8(FADE + 4, 0x82);
        }
        0xFA => strike(fx, sp),
        0xC6 => bolts(fx, sp, &mut r),
        0xBA => rays(fx, &r),
        0x144 | 0xAE => throw(fx, sp, &mut r),
        0x50 => flashes(fx),
        2 => start(fx, &mut r),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (2..0x142).contains(&n) && n & 1 == 0 {
        spike(fx, &mut r, 5, Some(0xC), 0x18, [0x3E99_999A, 0x4100_0000], [0, 0x30], 3, None, [0x01E0_66D8, 0x01E0_6720, 0x01E0_6734, 0x01E0_6748]);
    }
    let n = fx.mem.u16(D);
    if (0x50..0x142).contains(&n) && n & 1 == 0 {
        let (len, sizes) = if n < 0xBB { (8, [0x3F19_999A, 0x4100_0000]) } else { (0x20, [0x3FB3_3333, 0x4190_0000]) };
        spike(fx, &mut r, 5, Some(0x10), len, sizes, [0x10, 0x40], 3, None, [0x01E0_6848, 0x01E0_6894, 0x01E0_68A8, 0x01E0_68BC]);
    }
    let n = fx.mem.u16(D);
    if (0xFA..0x155).contains(&n) {
        spike(fx, &mut r, 0x85, None, 0x19, [0x4100_0000, 0x4270_0000], [0x10, 0x30], 4, Some(sp + 0xB0), [0x01E0_695C, 0x01E0_69DC, 0x01E0_69F0, 0x01E0_6A04]);
    }
    let n = fx.mem.u16(D);
    if (0x61..0x98).contains(&n) {
        sprites(fx, sp, &mut r);
    }
    shake_saving(fx, D + 0x40, r.s[0], 0x01E0_6CC4, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_6CD0, [Some(r.s[0]), Some(r.s[1])]);
    let (_, a3) = blib::star::flash_b_a3(fx, 0x01E0_03A0, FLASH, OT, 3);
    let (_, b3) = blib::spike::spikes_a3(fx, 0x01E0_0E10, SPIKES, OT, 0x50);
    // The rays' first 0x207EF0 reports the a3 the spikes left (a spike
    // record counts on every update they run) [run]; before them the
    // flashes' (0x1031C0's otherwise [H], never reached).
    blib::window::frame_rays_b(fx, 0x01E0_2250, RAYS, OT, 1, b3.or(a3).unwrap_or(0), r.f);
    blib::drop::drops(fx, DROPS, OT, 0x50);
    blib::fade::fade(fx, FADE, OT);
    // fade's 0xF0 frame saves the entry's s0..s6 (s6 = D) and ra.
    fx.saved(sp - 0xF0, &[(0x10, r.s[0]), (0x20, r.s[1]), (0x30, r.s[2]), (0x40, r.s[3]), (0x50, r.s[4]), (0x60, r.s[5]), (0x70, u64::from(D)), (0x80, 0x01E0_6D1C)]);
    blib::emit::segment_spawns_b(fx, 0x01E0_4460, EMIT, 8);
    // segment_spawns_b's 0x80 frame saves s0..s4 and ra.
    fx.saved(sp - 0x80, &[(0x10, r.s[0]), (0x20, r.s[1]), (0x30, r.s[2]), (0x40, r.s[3]), (0x50, r.s[4]), (0x60, 0x01E0_6D2C)]);
    blib::bolt::bolts_b(fx, 0x01E0_39D0, BOLTS, OT, 0x28);
    blib::dome::domes(fx, 0x01E0_2F90, DOMES, OT, 8);
    blib::segtube::tubes(fx, 0x01E0_47A0, TUBE, OT, 1);
    blib::band::bands_b(fx, 0x01E0_5110, BANDS_A, OT, 4);
    blib::band::bands_b(fx, 0x01E0_5110, BANDS_B, OT, 0x30);
    let n = fx.mem.u16(D);
    if (0xFA..0x15B).contains(&n) {
        6
    } else if n == 0x15B {
        2
    } else if n == 0xF0 {
        1
    } else {
        0
    }
}
