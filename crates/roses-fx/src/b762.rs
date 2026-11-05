//! BMODULE b762 (card 392), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E052E0, data area a0 = 0x1ED0000, 0x7B10 bytes)
//! [F]: b408's family (s6 = D, fp = the OT, s0
//! the parameter then pointers, s1..s5 / s7 counters / pointers, f20 a
//! scale) with its own records. Update 170 a flash and a shake at the
//! attacker's node 0x55; 188 a ribbon (+0x745C) between nodes 0x54 /
//! 0x55, its points followed on 188..533 (cleared on 300); 278 the fade,
//! a long shake at the target, flashes there and at node 0x55, eight bead
//! beams, 32 bolts on four helices node 0x55 -> target and three helix
//! ribbons (+0x7AA4, their 36 points redrawn every update while alive);
//! 280 a long flash at the target and four bolt emitters there; 530
//! eight bead beams at the target. Drops at nodes 0x2B / 0x32 on 2..32
//! and 75..88; odd updates 170..516 a spike at node 0x55, 280..508 one at
//! the target; 188..279 ten sprites from node 0x55; 198..532 two bolts
//! and a flash between nodes 0x54 and 0x55. The bar record +0x7C4 is
//! stepped, never set. Returns 6 on updates 280..529, 2 on 277, 1 on
//! 270, 8 on 564 (no drawing then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `flash_b`
//! 0x1E003A0, `bars_b` 0x1E00E10, `spikes` 0x1E01710, `drops` 0x1E021F0,
//! `fade` 0x1E02B50, `ribbons` 0x1E03020, `bolts_b` 0x1E03890,
//! `segment_spawns_b` 0x1E04320, `beads_at 34 3c` 0x1E04660
//! [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SHAKE: u32 = D + 0x40;
const FLASH: u32 = D + 0xA4;
const BARS: u32 = D + 0x7C4;
const SPIKES: u32 = D + 0x904;
const DROPS: u32 = D + 0x2164;
const FADE: u32 = D + 0x5684;
const BOLTS: u32 = D + 0x568C;
const EMIT: u32 = D + 0x6F8C;
const BEADS: u32 = D + 0x716C;
const RIBBON: u32 = D + 0x745C;
const BYTES: u32 = D + 0x747C;
const HELIX: u32 = D + 0x7AA4;

/// The entry's s0..s5, s7 (64 bits) as the callees' frames save them.
#[derive(Clone, Copy)]
struct R {
    s: [u64; 8],
}

/// The entry 0x1E052E0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x2C0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x2C0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    // 0x199208 memset(D, 0, 0x7B10).
    fx.bzero(D, 0x7B10);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    for k in 0..5 {
        fx.texture(5, k);
    }
    for (o, node, clear) in [(0xCu32, 0x2B, 1), (0x10, 0x32, 0), (0x14, 0x1A, 0), (0x18, 0x21, 0), (0x1C, 0x54, 0), (0x20, 0x55, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + o, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x22E);
    // The entry's s0 = 5 (the texture loop's), s1 / s2 the runner's (0).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_5430, [Some(5), Some(0), Some(0)]);
    0
}

/// rand() >> 4 with the entry's s0 saved in rand's frame.
fn rand4(fx: &mut Fx, r: &R, ra: u32) -> i32 {
    let f = fx.sp - 0x10;
    fx.mem.write_u64(f, r.s[0]);
    fx.mem.write_u64(f + 8, u64::from(ra));
    fx.rand() >> 4
}

/// node_position(self, D[at], out) with the entry's s0 / s1.
fn node_at(fx: &mut Fx, at: u32, out: u32, r: &R, ra: u32) {
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + at));
    node_position_saving(fx, h, s, out, r.s[0], Some(r.s[1]), ra);
}

/// A flash record's fields: kind `k` (+4), unit flag `unit` (+5; +6 the
/// node byte D[`node`] when given), the colour `c`, ALPHA bytes
/// (+0xA, +9, +8) = `ab`, size `size`.
#[allow(clippy::too_many_arguments)]
fn flash_fields(fx: &mut Fx, f: u32, unit: u8, node: Option<u32>, k: u8, ab: [u8; 3], c: u8, size: u32) {
    fx.mem.write_u8(f + 5, unit);
    if let Some(n) = node {
        let v = fx.mem.read_u8(D + n);
        fx.mem.write_u8(f + 6, v);
    }
    for (o, v) in [(4u32, k), (7, 9), (0xA, ab[0]), (9, ab[1]), (8, ab[2]), (0xF, c), (0xC, c), (0xD, c), (0xE, c)] {
        fx.mem.write_u8(f + o, v);
    }
    fx.mem.write_u32(f + 0x10, size);
}

/// Update 0xAA [F 0x1E054B4]: the flash slot 0 and a shake at node 0x55.
fn first_flash(fx: &mut Fx) {
    let f = FLASH;
    set_counter4(fx, f, 0x10, 0, 0xFF);
    flash_fields(fx, f, 1, Some(0x20), 0x81, [0x40, 0x20, 0x80], 0xF0, 0x4120_0000);
    fx.mem.write_u32(f + 0x14, 0x41F0_0000);
    fx.mem.write_u32(f + 0x18, 0x4080_0000);
    set_counter4(fx, SHAKE, 0x80, 0, 0xFF);
    fx.mem.write_u8(SHAKE + 5, 1);
    let v = fx.mem.read_u8(D + 0x20);
    fx.mem.write_u8(SHAKE + 6, v);
    fx.mem.write_u8(SHAKE + 4, 4);
    fx.mem.write_u32(D + 0x54, 0x3F80_0000);
    fx.mem.write_u32(D + 0x58, 0x3FE6_6666);
    fx.mem.write_u32(D + 0x5C, 0x3FE6_6666);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x4220_0000);
    fx.mem.write_u32(D + 0x90, 0x4080_0000);
}

/// Update 0xBC [F 0x1E05584]: the ribbon on the points +0x742C.
fn ribbon_start(fx: &mut Fx) {
    let g = RIBBON;
    set_counter4(fx, g, 0xFF, 0, 0xFF);
    for (o, v) in [(5u32, 0u8), (4, 0x81), (8, 1), (9, 0), (0xA, 2), (0xB, 0), (0xC, 3)] {
        fx.mem.write_u8(g + o, v);
    }
    fx.mem.write_u32(g + 0x10, D + 0x742C);
    fx.mem.write_u32(g + 0x14, 0x3F80_0000);
    fx.mem.write_u32(g + 0x18, 0x3E4C_CCCD);
    for (o, v) in [(0x1Cu32, 0x80u8), (0x1D, 0x80), (0x1E, 0x80), (0x1F, 0xC0)] {
        fx.mem.write_u8(g + o, v);
    }
}

/// Eight bead-beam records at `pos` [F 0x1E05824 / 0x1E05CA8]: length
/// `len`, size / drift `size` / `drift`.
fn beads(fx: &mut Fx, r: &mut R, len: u8, size: u32, drift: u32, pos: u32, ra: u32) {
    for s1 in 0..8u32 {
        let s0 = D + 0x58 * s1;
        let b = BEADS + 0x58 * s1;
        r.s[0] = u64::from(s0);
        r.s[1] = u64::from(s1);
        set_counter4(fx, b, 0x20, s1 as i32, 0xFF);
        let c = fx.mem.read_u8(b).wrapping_sub(4);
        fx.mem.write_u8(b, c);
        for (o, v) in [(4u32, 5u8), (5, 0), (8, len), (9, s1 as u8), (0xA, 0), (0xC, 2)] {
            fx.mem.write_u8(b + o, v);
        }
        let v = rand4(fx, r, ra) & 3;
        fx.mem.set8(b + 0xB, v);
        fx.mem.write_u32(b + 0x1C, size);
        fx.mem.write_u32(b + 0x20, drift);
        fx.mem.write_u32(b + 0x10, BYTES);
        fx.mem.write_u32(b + 0x14, 0x4000_0000);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(b + o, 0x80);
        }
        copy_v4(fx, pos, b + 0x28);
    }
    r.s[0] = u64::from(D + 0x58 * 8);
    r.s[1] = 8;
}

/// Update 0x116 [F 0x1E05618]: the fade, a long shake at the target (sp
/// + 0xB0), a flash at node 0x55 (sp + 0xC0) and a long one on it, eight
/// bead beams there, 32 bolts on four helices node -> target and the
/// three helix ribbons.
fn strike(fx: &mut Fx, sp: u32, r: &mut R) {
    set_counter4(fx, FADE, 0x18, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    set_counter4(fx, SHAKE, 0xFF, 0, 0xFF);
    fx.mem.write_u8(SHAKE + 5, 0);
    fx.mem.write_u8(SHAKE + 4, 8);
    fx.mem.write_u32(D + 0x54, 0x3F80_0000);
    fx.mem.write_u32(D + 0x58, 0x3FB3_3333);
    fx.mem.write_u32(D + 0x5C, 0x4000_0000);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x4320_0000);
    copy_v4(fx, sp + 0xB0, D + 0x60);
    fx.mem.write_u32(D + 0x90, 0x4100_0000);
    fx.mem.write_u32(D + 0x68, 0);
    node_at(fx, 0x20, sp + 0xC0, r, 0x01E0_56B8);
    let i = first_free(fx, FLASH, 0x4C, 0x18);
    if i >= 0 {
        r.s[0] = u64::from(0x4C * i as u32);
        let f = FLASH + 0x4C * i as u32;
        set_counter4(fx, f, 0x14, 0, 0xFF);
        flash_fields(fx, f, 0, None, 0x80, [0x40, 0x20, 0x80], 0xF0, 0x42C8_0000);
        fx.mem.write_u32(f + 0x14, 0x4270_0000);
        fx.mem.write_u32(f + 0x18, 0x4248_0000);
        copy_v4(fx, sp + 0xC0, f + 0x1C);
    }
    let i = first_free(fx, FLASH, 0x4C, 0x18);
    r.s[1] = 0;
    if i >= 0 {
        r.s[0] = u64::from(0x4C * i as u32);
        let f = FLASH + 0x4C * i as u32;
        set_counter4(fx, f, 0xFF, 0xC, 0xFF);
        flash_fields(fx, f, 1, Some(0x20), 0x82, [0x40, 0x20, 0x80], 0xB0, 0x4416_0000);
        fx.mem.write_u32(f + 0x14, 0);
        fx.mem.write_u32(f + 0x18, 0);
    }
    beads(fx, r, 0x32, 0x4270_0000, 0xBF19_999A, sp + 0xC0, 0x01E0_586C);
    // Four helices of eight bolts, node 0x55 (sp + 0xC0) -> the target.
    let p = sp + 0xE0;
    'groups: for s0 in 0..4u32 {
        r.s[0] = u64::from(s0);
        r.s[7] = u64::from(8 * s0);
        fx.mem.set32(p, 8);
        fx.mem.write_u32(p + 4, 0x4120_0000);
        fx.mem.write_u32(p + 0xC, 0x4204_0000);
        fx.mem.write_u32(p + 8, if s0 < 2 { 0x4352_0000 } else { 0x41F0_0000 });
        let s3 = s0 & 1;
        r.s[3] = u64::from(s3);
        if s3 != 0 {
            let v = ee::mul(fx.mem.f(p + 0xC), -1.0);
            fx.mem.setf(p + 0xC, v);
        }
        for (dst, src) in [(0x10u32, 0xC0u32), (0x14, 0xC4), (0x18, 0xC8), (0x1C, 0xB0), (0x20, 0xB4), (0x24, 0xB8)] {
            let w = fx.mem.read_u32(sp + src);
            fx.mem.write_u32(p + dst, w);
        }
        fx.mem.write_u32(p + 0x28, sp + 0x110);
        blib::bolt::helix(fx, p);
        r.s[2] = u64::from(sp + 0x110);
        r.s[5] = 0;
        r.s[1] = r.s[7];
        for s5 in 0..8u32 {
            let i = first_free(fx, BOLTS, 0x40, 0x64);
            if i < 0 {
                continue 'groups;
            }
            let s1 = 8 * s0 + s5;
            let b = BOLTS + (s1 << 6);
            r.s[4] = u64::from(D + (s1 << 6));
            set_counter4(fx, b, 0xFF, s3 as i32, 0xFF);
            fx.mem.write_u8(b + 5, 0);
            fx.mem.write_u8(b + 4, 0);
            let q = sp + 0x110 + 0xC * s5;
            for c in 0..6u32 {
                let w = fx.mem.read_u32(q + 4 * c);
                fx.mem.write_u32(b + 0x1C + 4 * c, w);
            }
            fx.mem.write_u8(b + 8, 6);
            fx.mem.write_u32(b + 0x10, 0x3F80_0000);
            fx.mem.write_u32(b + 0x14, 0x3E99_999A);
            for (o, v) in [(0xBu32, 4u8), (0xD, 3), (0xC, 0), (0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x20)] {
                fx.mem.write_u8(b + o, v);
            }
            r.s[5] = u64::from(s5 + 1);
            r.s[1] = u64::from(s1 + 1);
            r.s[2] = u64::from(q + 0xC);
        }
    }
    r.s[0] = 4;
    r.s[7] = 32;
    for s2 in 0..3u32 {
        let g = HELIX + 0x20 * s2;
        set_counter4(fx, g, 0xFF, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 1), (8, 0x23), (9, 0), (0xA, 2), (0xB, 0), (0xC, 3)] {
            fx.mem.write_u8(g + o, v);
        }
        fx.mem.write_u32(g + 0x10, D + 0x1B0 * s2 + 0x757C);
        fx.mem.write_u32(g + 0x14, 0x42A0_0000);
        fx.mem.write_u32(g + 0x18, 0x3F19_999A);
        for o in [0x1Fu32, 0x1C, 0x1D, 0x1E] {
            fx.mem.write_u8(g + o, 0x80);
        }
    }
    r.s[2] = 3;
    r.s[0] = u64::from(D + 0x60);
    r.s[1] = u64::from(D + 0x510);
}

/// Update 0x118 [F 0x1E05B14]: a long flash at the target and four bolt
/// emitters there.
fn strike2(fx: &mut Fx, sp: u32, r: &mut R) {
    let i = first_free(fx, FLASH, 0x4C, 0x10);
    if i >= 0 {
        r.s[0] = u64::from(0x4C * i as u32);
        let f = FLASH + 0x4C * i as u32;
        set_counter4(fx, f, 0xFD, 0, 0xFF);
        flash_fields(fx, f, 0, Some(0xC), 0x85, [0x40, 0x20, 0x80], 0xC0, 0x44BB_8000);
        fx.mem.write_u32(f + 0x14, 0);
        copy_v4(fx, sp + 0xB0, f + 0x1C);
    }
    for s1 in 0..4u32 {
        r.s[0] = u64::from(D + 0x3C * s1);
        r.s[1] = u64::from(s1);
        let e = EMIT + 0x3C * s1;
        set_counter4(fx, e, 0xEF, s1 as i32, 3);
        fx.mem.write_u8(e + 5, 0);
        fx.mem.write_u8(e + 4, 0);
        fx.mem.write_u32(e + 8, BOLTS);
        for (o, v) in [(0xCu32, 0x64u8), (0xD, 0xC), (0xE, 4), (0xF, 0), (0x10, 5), (0x11, 0x12)] {
            fx.mem.write_u8(e + o, v);
        }
        fx.mem.set32(e + 0x14, 0x1770);
        fx.mem.write_u32(e + 0x18, 0x3F66_6666);
        fx.mem.write_u32(e + 0x1C, 0x3ECC_CCCD);
        for (o, v) in [(0x20u32, 0x80u8), (0x21, 0x80), (0x22, 0x80), (0x23, 0x30)] {
            fx.mem.write_u8(e + o, v);
        }
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0xB0 + 4 * c);
            fx.mem.write_u32(e + 0x24 + 4 * c, w);
        }
        for o in [0x38u32, 0x34, 0x30] {
            fx.mem.write_u32(e + o, 0);
        }
    }
    r.s[0] = u64::from(D + 0x3C * 4);
    r.s[1] = 4;
}

/// ((rand >> 4) % m x 200 - sub) / 100.
fn jit(fx: &mut Fx, r: &R, ra: u32, m: i32, sub: i32) -> f32 {
    let v = rand4(fx, r, ra);
    let (_, q) = fx.div(v, m);
    ee::div(ee::from_i32(200 * q - sub), 100.0)
}

/// ((rand >> 4) % 20 - 10) / 100.
fn spread(fx: &mut Fx, r: &R, ra: u32) -> f32 {
    let v = rand4(fx, r, ra);
    let (_, q) = fx.div(v, 0x14);
    ee::div(ee::from_i32(q - 0xA), 100.0)
}

/// Updates 2..32 / 75..88 [F 0x1E05DB4]: a drop at each of the nodes
/// D[0xC] / D[0x10].
fn drops(fx: &mut Fx, sp: u32, r: &mut R) {
    for s2 in 0..2u32 {
        r.s[2] = u64::from(s2);
        r.s[0] = u64::from(D + 4 * s2);
        let i = first_free(fx, DROPS, 0x44, 0xC8);
        if i < 0 {
            continue;
        }
        let s1 = D + 0x44 * i as u32;
        r.s[1] = u64::from(s1);
        let p = s1 + 0x2164;
        set_counter4(fx, p, 0x18, 0, 0xFF);
        let v = rand4(fx, r, 0x01E0_5DFC) & 3;
        fx.mem.set8(p + 4, v << 4);
        fx.mem.write_u8(p + 8, 2);
        fx.mem.write_u8(p + 0xA, 5);
        fx.mem.write_u8(p + 0xB, 1);
        let v = rand4(fx, r, 0x01E0_5E28) & 3;
        fx.mem.set8(p + 9, v);
        for o in [0xCu32, 0xD, 0xE] {
            fx.mem.write_u8(p + o, 0x80);
        }
        let v = (rand4(fx, r, 0x01E0_5E48) & 0x1F) + 0x60;
        fx.mem.set8(p + 0xF, v);
        fx.mem.write_u32(p + 0x10, 0x4248_0000);
        fx.mem.write_u32(p + 0x14, 0x4120_0000);
        fx.mem.write_u32(p + 0x1C, 0);
        fx.mem.write_u32(p + 0x18, 0);
        node_at(fx, 0xC + 4 * s2, sp + 0xC0, r, 0x01E0_5E80);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0xC0 + 4 * c);
            fx.mem.write_u32(p + 0x20 + 4 * c, w);
        }
        for (o, ra) in [(0x2Cu32, 0x01E0_5E9C), (0x30, 0x01E0_5ED8), (0x34, 0x01E0_5F14)] {
            let v = spread(fx, r, ra);
            fx.mem.setf(p + o, v);
        }
    }
    r.s[2] = 2;
    r.s[0] = u64::from(D + 8);
}

/// Updates 0xBC..0x215 [F 0x1E05F74]: the ribbon's points from node 0x54
/// (+0x7438) to node 0x55 (+0x742C), its direction +0x7444 = 1.2 x the
/// difference; its width +0x7470 up by 2 below 90.
fn ribbon_points(fx: &mut Fx, sp: u32, r: &mut R) {
    node_at(fx, 0x1C, sp + 0xC0, r, 0x01E0_5F84);
    for c in 0..3u32 {
        let w = fx.mem.read_u32(sp + 0xC0 + 4 * c);
        fx.mem.write_u32(D + 0x7438 + 4 * c, w);
    }
    r.s[0] = u64::from(sp + 0xC4);
    r.s[1] = u64::from(sp + 0xC8);
    node_at(fx, 0x20, sp + 0xC0, r, 0x01E0_5FB4);
    for c in 0..3u32 {
        let w = fx.mem.read_u32(sp + 0xC0 + 4 * c);
        fx.mem.write_u32(D + 0x742C + 4 * c, w);
    }
    let k = f32::from_bits(0x3F99_999A);
    for c in 0..3u32 {
        let v = ee::mul(k, ee::sub(fx.mem.f(D + 0x742C + 4 * c), fx.mem.f(D + 0x7438 + 4 * c)));
        fx.mem.setf(D + 0x7444 + 4 * c, v);
    }
    let w = fx.mem.f(D + 0x7470);
    if w < 90.0 {
        fx.mem.setf(D + 0x7470, ee::add(w, 2.0));
    }
}

/// Odd updates 0xAA..0x204 (node 0x55) / 0x118..0x1FC (the target, sp +
/// 0xB0) [F 0x1E06070 / 0x1E06478]: a spike.
fn spike(fx: &mut Fx, sp: u32, r: &mut R, near: bool, ra: [u32; 4]) {
    let i = first_free(fx, SPIKES, 0x68, 0x3C);
    if i < 0 {
        return;
    }
    let s0 = 0x68 * i as u32;
    r.s[0] = u64::from(s0);
    let q = SPIKES + s0;
    if near {
        set_counter4(fx, q, 0x10, 0, 0xFF);
        fx.mem.write_u8(q + 4, 0x85);
        fx.mem.write_u8(q + 5, 1);
        let v = fx.mem.read_u8(D + 0x20);
        fx.mem.write_u8(q + 6, v);
        for (o, v) in [(0xAu32, 0u8), (0xB, 2), (0xC, 3)] {
            fx.mem.write_u8(q + o, v);
        }
        let x = rand4(fx, r, ra[0]);
        fx.mem.set8(q + 9, x);
        fx.mem.write_u32(q + 0x10, BYTES);
        fx.mem.write_u32(q + 0x18, 0x3F80_0000);
        for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0), (0x27, 0x40)] {
            fx.mem.write_u8(q + o, v);
        }
        if fx.mem.u16(D) < 0x118 {
            fx.mem.write_u8(q + 8, 0x14);
            fx.mem.write_u32(q + 0x14, 0x3FE6_6666);
            fx.mem.write_u32(q + 0x1C, 0x420C_0000);
        } else {
            fx.mem.write_u8(q + 8, 0xA);
            fx.mem.write_u32(q + 0x14, 0x4040_0000);
            fx.mem.write_u32(q + 0x1C, 0x41F0_0000);
        }
    } else {
        set_counter4(fx, q, 0x18, 0, 0xFF);
        fx.mem.write_u8(q + 4, 5);
        fx.mem.write_u32(q + 0x14, 0x4060_0000);
        fx.mem.write_u32(q + 0x1C, 0x4248_0000);
        for (o, v) in [(8u32, 0x14u8), (5, 0), (0xA, 0), (0xB, 2), (0xC, 3)] {
            fx.mem.write_u8(q + o, v);
        }
        let x = rand4(fx, r, ra[0]);
        fx.mem.set8(q + 9, x);
        fx.mem.write_u32(q + 0x10, BYTES);
        fx.mem.write_u32(q + 0x18, 0x3F80_0000);
        for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20), (0x27, 0x80)] {
            fx.mem.write_u8(q + o, v);
        }
    }
    for (o, ra) in [(0x58u32, ra[1]), (0x5C, ra[2]), (0x60, ra[3])] {
        let v = rand4(fx, r, ra) & 0x7F;
        fx.mem.set32(q + o, v);
    }
    if !near {
        copy_v4(fx, sp + 0xB0, q + 0x28);
    }
}

/// Updates 0xBC..0x117 [F 0x1E061D4]: ten sprites from node 0x55, thrown
/// outwards.
fn sprites(fx: &mut Fx, sp: u32, r: &mut R) {
    for s3 in 0..10u32 {
        r.s[3] = u64::from(s3);
        let i = first_free(fx, DROPS, 0x44, 0xC8);
        r.s[0] = i as i64 as u64;
        if i < 0 {
            continue;
        }
        node_at(fx, 0x20, sp + 0xC0, r, 0x01E0_6200);
        let v = rand4(fx, r, 0x01E0_6208);
        let (_, m) = fx.div(v, 6);
        let f20 = ee::add(12.0, ee::div(ee::from_i32(200 * m - 0x258), 100.0));
        fx.fix_residue(f20, 0x01E0_6260);
        let count = ee::fixsfsi(f20) & 0xFF;
        let s0 = D + 0x44 * i as u32;
        r.s[0] = u64::from(s0);
        let p = s0 + 0x2164;
        set_counter4(fx, p, count, 0, 0xFF);
        fx.mem.write_u8(p + 4, 0x41);
        fx.mem.write_u8(p + 8, 0);
        fx.mem.write_u8(p + 0xB, 2);
        let v = rand4(fx, r, 0x01E0_629C) & 7;
        fx.mem.set8(p + 9, v);
        fx.mem.write_u8(p + 0xA, 0);
        for o in [0xCu32, 0xD, 0xE] {
            fx.mem.write_u8(p + o, 0x80);
        }
        fx.mem.write_u8(p + 0xF, 0x60);
        fx.mem.write_u32(p + 0x10, 0x4220_0000);
        fx.mem.write_u32(p + 0x14, 0xC040_0000);
        fx.mem.write_u32(p + 0x1C, 0);
        fx.mem.write_u32(p + 0x18, 0);
        let f = jit(fx, r, 0x01E0_62E0, 0xF, 0x5DC);
        fx.mem.setf(sp + 0xD0, f);
        r.s[1] = u64::from(sp + 0xD0);
        let f = jit(fx, r, 0x01E0_6334, 0xF, 0x5DC);
        fx.mem.setf(sp + 0xD4, f);
        r.s[2] = u64::from(sp + 0xD4);
        let f = jit(fx, r, 0x01E0_6388, 0xF, 0x5DC);
        fx.mem.setf(sp + 0xD8, f);
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
    r.s[3] = 10;
}

/// Updates 0xC6..0x214 [F 0x1E065BC]: a point a random tenth of the way
/// from node 0x55 (sp + 0xD0) to node 0x54 (sp + 0xC0), two short bolts
/// there (jittered +-5) and a flash.
fn sparks(fx: &mut Fx, sp: u32, r: &mut R) {
    let v = rand4(fx, r, 0x01E0_65C4);
    let (_, m) = fx.div(v, 0xA);
    let f20 = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(m));
    node_at(fx, 0x1C, sp + 0xC0, r, 0x01E0_6600);
    r.s[0] = u64::from(sp + 0xD0);
    node_at(fx, 0x20, sp + 0xD0, r, 0x01E0_6614);
    for c in 0..3u32 {
        let a = fx.mem.f(sp + 0xD0 + 4 * c);
        let v = ee::add(a, ee::mul(f20, ee::sub(fx.mem.f(sp + 0xC0 + 4 * c), a)));
        fx.mem.setf(sp + 0xC0 + 4 * c, v);
    }
    r.s[7] = u64::from(sp + 0xC4);
    r.s[5] = u64::from(sp + 0xC8);
    r.s[4] = 0;
    for s4 in 0..2u32 {
        let i = first_free(fx, BOLTS, 0x40, 0x64);
        if i < 0 {
            break;
        }
        let s0 = D + ((i as u32) << 6);
        r.s[0] = u64::from(s0);
        let b = s0 + 0x568C;
        set_counter4(fx, b, 8, s4 as i32, 0xFF);
        fx.mem.write_u8(b + 5, 0);
        fx.mem.write_u8(b + 4, 4);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0xC0 + 4 * c);
            fx.mem.write_u32(b + 0x28 + 4 * c, w);
            fx.mem.write_u32(b + 0x1C + 4 * c, w);
        }
        r.s[1] = u64::from(b + 0x28);
        r.s[2] = u64::from(b + 0x2C);
        r.s[3] = u64::from(b + 0x30);
        for (c, ra) in [(0u32, 0x01E0_66DC), (4, 0x01E0_6734), (8, 0x01E0_678C)] {
            let j = jit(fx, r, ra, 5, 0x1F4);
            let v = ee::add(fx.mem.f(b + 0x28 + c), j);
            fx.mem.setf(b + 0x28 + c, v);
        }
        r.s[4] = u64::from(s4 + 1);
        fx.mem.write_u8(b + 8, 6);
        fx.mem.write_u32(b + 0x10, 0x3F80_0000);
        fx.mem.write_u32(b + 0x14, 0x3E99_999A);
        for (o, v) in [(0xBu32, 4u8), (0xD, 3), (0xC, 0), (0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x40)] {
            fx.mem.write_u8(b + o, v);
        }
    }
    let i = first_free(fx, FLASH, 0x4C, 0x18);
    if i >= 0 {
        r.s[0] = u64::from(0x4C * i as u32);
        let f = FLASH + 0x4C * i as u32;
        set_counter4(fx, f, 9, 0, 0xFF);
        flash_fields(fx, f, 0, None, 5, [0, 0, 0x80], 0xD0, 0x42C8_0000);
        fx.mem.write_u32(f + 0x14, 0);
        copy_v4(fx, sp + 0xC0, f + 0x1C);
    }
}

/// While the first helix ribbon runs [F 0x1E068E8]: each of the three
/// ribbons' 36 points redrawn as a helix (radius 0.1125 x its count, at
/// most 1.8; phase 90 + 120 k less 30 x the elapsed updates; step 30)
/// from node 0x55 (sp + 0xC0) to the target.
fn helices(fx: &mut Fx, sp: u32, r: &mut R) {
    let a = fx.mem.read_u8(HELIX);
    let e = i32::from(fx.mem.read_u8(HELIX + 2)) - i32::from(a);
    let f20 = ee::mul(30.0, ee::from_i32(e));
    let p = sp + 0xE0;
    for s2 in 0..3u32 {
        r.s[2] = u64::from(s2);
        r.s[0] = u64::from(D + 0x20 * s2);
        r.s[1] = u64::from(D + 0x1B0 * s2);
        node_at(fx, 0x20, sp + 0xC0, r, 0x01E0_6924);
        fx.mem.set32(p, 0x24);
        let c = fx.mem.read_u8(HELIX + 0x20 * s2);
        if c < 0x10 {
            let v = ee::mul(f32::from_bits(0x3DE6_6666), ee::from_i32(i32::from(c)));
            fx.mem.setf(p + 4, v);
        } else {
            fx.mem.write_u32(p + 4, 0x3FE6_6666);
        }
        let ph = ee::sub(ee::add(90.0, ee::mul(120.0, ee::from_i32(s2 as i32))), f20);
        fx.mem.setf(p + 8, ph);
        fx.mem.write_u32(p + 0xC, 0x41F0_0000);
        fx.mem.write_u32(p + 0x28, D + 0x1B0 * s2 + 0x757C);
        for (dst, src) in [(0x10u32, 0xC0u32), (0x14, 0xC4), (0x18, 0xC8), (0x1C, 0xB0), (0x20, 0xB4), (0x24, 0xB8)] {
            let w = fx.mem.read_u32(sp + src);
            fx.mem.write_u32(p + dst, w);
        }
        blib::bolt::helix(fx, p);
    }
    r.s[2] = 3;
    r.s[0] = u64::from(D + 0x60);
    r.s[1] = u64::from(D + 0x510);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    // The entry's s0 = the parameter (64 bits), s1..s7 the runner's 0.
    let mut r = R { s: [param as i64 as u64, 0, 0, 0, 0, 0, 0, 0] };
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0xB0, r.s[0], 0x01E0_5450);
    match fx.mem.u16(D) {
        0x234 => return 8,
        0x212 => beads(fx, &mut r, 0x28, 0x42F0_0000, 0xBF4C_CCCD, sp + 0xB0, 0x01E0_5CF0),
        0x118 => strike2(fx, sp, &mut r),
        0x116 => strike(fx, sp, &mut r),
        0x12C => set_counter4(fx, RIBBON, 0xFF, 0, 0xFF),
        0xBC => ribbon_start(fx),
        0xAA => first_flash(fx),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (2..0x21).contains(&n) || (0x4B..0x59).contains(&n) {
        drops(fx, sp, &mut r);
    }
    let n = fx.mem.u16(D);
    if (0xBC..0x216).contains(&n) {
        ribbon_points(fx, sp, &mut r);
    }
    let n = fx.mem.u16(D);
    if (0xAA..0x205).contains(&n) && n & 1 != 0 {
        spike(fx, sp, &mut r, true, [0x01E0_60E8, 0x01E0_617C, 0x01E0_6190, 0x01E0_61A4]);
    }
    let n = fx.mem.u16(D);
    if (0xBC..0x118).contains(&n) {
        sprites(fx, sp, &mut r);
    }
    let n = fx.mem.u16(D);
    if (0x118..0x1FD).contains(&n) && n & 1 != 0 {
        spike(fx, sp, &mut r, false, [0x01E0_64FC, 0x01E0_6544, 0x01E0_6558, 0x01E0_656C]);
    }
    let n = fx.mem.u16(D);
    if (0xC6..0x215).contains(&n) {
        sparks(fx, sp, &mut r);
    }
    if fx.mem.read_u8(HELIX) != 0 {
        helices(fx, sp, &mut r);
    }
    shake_saving(fx, SHAKE, r.s[0], 0x01E0_6A24, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_6A30, [Some(r.s[0]), Some(r.s[1])]);
    blib::fade::fade(fx, FADE, OT);
    // fade's 0xF0 frame saves the entry's s0..s6 (s6 = D) and ra.
    fx.saved(sp - 0xF0, &[(0x10, r.s[0]), (0x20, r.s[1]), (0x30, r.s[2]), (0x40, r.s[3]), (0x50, r.s[4]), (0x60, r.s[5]), (0x70, u64::from(D)), (0x80, 0x01E0_6A3C)]);
    blib::star::flash_b(fx, 0x01E0_03A0, FLASH, OT, 0x18);
    blib::cross::bars_b(fx, 0x01E0_0E10, BARS, OT, 4);
    blib::spike::spikes(fx, 0x01E0_1710, SPIKES, OT, 0x3C);
    blib::drop::drops(fx, DROPS, OT, 0xC8);
    blib::dash::ribbons(fx, 0x01E0_3020, RIBBON, OT, 1);
    blib::bolt::bolts_b(fx, 0x01E0_3890, BOLTS, OT, 0x64);
    blib::emit::segment_spawns_b(fx, 0x01E0_4320, EMIT, 8);
    // segment_spawns_b's 0x80 frame saves s0..s4 and ra (b408 / b370: the
    // stale TEX0 / TEX1 of the beads' missing-texture strips) [run].
    fx.saved(sp - 0x80, &[(0x10, r.s[0]), (0x20, r.s[1]), (0x30, r.s[2]), (0x40, r.s[3]), (0x50, r.s[4]), (0x60, 0x01E0_6AAC)]);
    blib::beam::beads_at(fx, 0x01E0_4660, 0x34, 0x3C, BEADS, OT, 8);
    blib::dash::ribbons(fx, 0x01E0_3020, HELIX, OT, 3);
    let n = fx.mem.u16(D);
    if (0x118..0x212).contains(&n) {
        6
    } else if n == 0x115 {
        2
    } else if n == 0x10E {
        1
    } else {
        0
    }
}
