//! BMODULE b379 (card 2), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05770, data area a0 = 0x1ED0000, 0xD280 bytes)
//! [F]: b706's library family, s6 = D, the OT
//! (0x206E40) at sp + 0xB8, a 0x1D0 frame. Init: the handles (+4 / +8),
//! the attacker's rotation (+0x2C), textures (4, 0xF) and (5, 0..3), nodes
//! 0x53 (cleared, +0xC) / 0x3E (+0x10) / 0x48 (+0x14), half the target's
//! radius, at least 24 (+0xD17C, 0x1F6A70), the stage light ramp (+0xA0:
//! 2 / 0x1BC), the random bytes (+0xD180). Every update the target's
//! animated point (0x1F65D0 kind 1) into sp + 0xE0. Update 2: five
//! flashes (+0x388C) at the nodes; 0x43 three long flashes; 0x44 the frame
//! fade window (+0x384C, to n + 0x28) and the fade (+0xA4, kind 0x82);
//! 0x104 fifteen spikes (+0xAC) at the target; 0x11D the window again (to
//! 0x19C); 0x12C eight bands (+0x4174), the fade, the flat window
//! (+0x387C) and two domes (+0x4E3C); 0x16E the fade (kind 2); 0x17E the
//! fade (kind 4), then the outcome boundary 0x1F8100 on 0x101AD0's
//! leftovers (D+0xA4, 8, 0, 0xFF) and, at a nonzero answer, the target
//! tinted (50, 30, 50, 80) (0x1F75F0); 0x1C2 returns 8 (no drawing).
//! Odd 3..0x2F: three spikes at the nodes, each with 14 drops (+0x50BC)
//! thrown from the node; odd 0x43..0xEF three spikes; even 0x4B..0xEE a
//! ribbon (+0xBAFC) along a 20-point arc (0x102F20, +0xBD3C + 0x120 k)
//! from a node to the target; each ribbon at counts 7..9 a spike and four
//! drops at its point, at count 9 an annulus (+0x4734), a spike aimed
//! back and two bolts (+0x294C). Returns 6 on 0x5F..0x178, 2 on 0x179, 1
//! on 0x55, 8 on 0x1C2 and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `fade`
//! 0x1E003A0, `spikes` 0x1E00870, `drops` 0x1E01350, `frame_fade_b`
//! 0x1E01CB0, `window::flat` 0x1E02130, `flash_b` 0x1E022B0,
//! `dash::ribbons` 0x1E02D20, `domes` 0x1E03590, `bands_b` 0x1E03FD0,
//! `annulus::flat` 0x1E04710, `bolts_b` 0x1E04CE0 [`crate::blib`].
use crate::b015::unit_tint_saving;
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving};
use crate::blib::{self, exe::{aim_angles, first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SHAKE: u32 = D + 0x40;
const LIGHT: u32 = D + 0xA0;
const FADE: u32 = D + 0xA4;
const SPIKES: u32 = D + 0xAC;
const BOLTS: u32 = D + 0x294C;
const WINDOW: u32 = D + 0x384C;
const FLAT: u32 = D + 0x387C;
const FLASH: u32 = D + 0x388C;
const BANDS: u32 = D + 0x4174;
const ANNULUS: u32 = D + 0x4734;
const DOMES: u32 = D + 0x4E3C;
const DROPS: u32 = D + 0x50BC;
const RIBBONS: u32 = D + 0xBAFC;
const PATHS: u32 = D + 0xBD3C;
const RADIUS: u32 = D + 0xD17C;
const BYTES: u32 = D + 0xD180;
/// [H] the runner's s1 at the entry.
const RUNNER_S1: u64 = 0;

fn sx(v: i32) -> u64 {
    i64::from(v) as u64
}

/// The entry 0x1E05770 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1D0);
    fx.mem.write_u32(sp + 0xB8, OT);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x1D0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    // 0x199208 memset(D, 0, 0xD280).
    fx.bzero(D, 0xD280);
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
    for (at, node, clear) in [(0xCu32, 0x53, 1), (0x10, 0x3E, 0), (0x14, 0x48, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    let t = fx.mem.i32(D + 8);
    let r = blib::afterimage::unit_radius(fx, t);
    let v = ee::mul(0.5, r);
    fx.mem.setf(RADIUS, v);
    if ee_lt(v, 24.0) {
        fx.mem.write_u32(RADIUS, 0x41C0_0000);
    }
    fx.mem.set16(LIGHT, 2);
    fx.mem.set16(LIGHT + 2, 0x1BC);
    // The entry's s0 = 4 (the texture loop's), s1 / s2 the runner's.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_58CC, [Some(4), None, None]);
    0
}

/// rand() >> 4 with the caller's s0 (64 bits) saved in rand's frame.
fn rand4(fx: &mut Fx, s0: u64, ra: u32) -> i32 {
    let f = fx.sp - 0x10;
    fx.mem.write_u64(f, s0);
    fx.mem.write_u64(f + 8, u64::from(ra));
    fx.rand() >> 4
}

/// (k ((rand >> 4) % m) - half) / 100.
fn jit(fx: &mut Fx, s0: u64, ra: u32, m: i32, k: i32, half: i32) -> f32 {
    let v = rand4(fx, s0, ra);
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(k * r - half), 100.0)
}

/// 0x101940 `aim_angles(v, out)` leaving its second atan2f's 0x1A5020
/// frame: ra 0x1019B4 at the caller's sp - 0x38 (as b051 / b173).
fn aim_saving(fx: &mut Fx, v: u32, out: u32) {
    aim_angles(fx, v, out);
    let sp = fx.sp;
    fx.mem.write_u64(sp - 0x38, 0x0010_19B4);
}

fn set(fx: &mut Fx, r: u32, bytes: &[(u32, u8)], words: &[(u32, u32)]) {
    for &(o, v) in bytes {
        fx.mem.write_u8(r + o, v);
    }
    for &(o, v) in words {
        fx.mem.write_u32(r + o, v);
    }
}

/// The s registers the step's calls save.
struct Regs {
    s0: u64,
    s1: u64,
}

/// The frame fade window (+0x384C) from `n` to `end`, corners +-8.
fn window(fx: &mut Fx, n: i32, end: i32) {
    fx.mem.set16(WINDOW, n);
    fx.mem.set16(WINDOW + 2, end);
    fx.mem.write_u8(WINDOW + 5, 7);
    fx.mem.write_u8(WINDOW + 6, 2);
    fx.mem.write_u32(WINDOW + 8, 0);
    for (o, v) in [(0x14u32, -8), (0xC, -8), (0x18, 8), (0x10, 8), (0x20, -8), (0x1C, -8), (0x28, 8), (0x24, 8)] {
        fx.mem.set32(WINDOW + o, v);
    }
    fx.mem.write_u8(WINDOW + 0x2E, 0x80);
    fx.mem.write_u8(WINDOW + 0x2D, 0x80);
    fx.mem.write_u8(WINDOW + 0x2C, 0x90);
}

/// Update 2 [F 0x1E0596C]: five flashes (two short at the nodes +0xC /
/// +0x10, three long at +0xC / +0x10 / +0x14 delayed 0x20).
fn flashes5(fx: &mut Fx, r: &mut Regs) {
    r.s0 = 0;
    r.s1 = u64::from(D);
    for s3 in 0..5u32 {
        let i = first_free(fx, FLASH, 0x4C, 0x1E);
        if i >= 0 {
            let f = FLASH.wrapping_add((0x4C * i) as u32);
            if s3 < 2 {
                set_counter4(fx, f, 0x10, (r.s0 & 0xFF) as i32, 0xFF);
                fx.mem.write_u8(f + 5, 1);
                let b = fx.mem.read_u8(r.s1 as u32 + 0xC);
                set(fx, f, &[(6, b), (4, 0x81), (7, 9), (0xA, 0), (9, 0), (8, 0x80), (0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0)], &[(0x10, 0x4548_0000), (0x14, 0xC348_0000)]);
            } else {
                set_counter4(fx, f, 0x18, 0x20, 0xFF);
                fx.mem.write_u8(f + 5, 1);
                let b = fx.mem.read_u8(r.s1 as u32 + 4);
                set(fx, f, &[(6, b), (4, 0x80), (7, 9), (0xA, 0x20), (9, 0x20), (8, 0x80), (0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0)], &[(0x10, 0x4516_0000), (0x14, 0xC2C8_0000), (0x18, 0x420C_0000)]);
            }
        }
        r.s0 += 0x10;
        r.s1 += 4;
    }
}

/// Update 0x43 [F 0x1E05AF8]: three long flashes at the nodes (the first
/// free slot unchecked).
fn flashes3(fx: &mut Fx, r: &mut Regs) {
    r.s0 = u64::from(D);
    for _ in 0..3 {
        let i = first_free(fx, FLASH, 0x4C, 0x1E);
        let f = D.wrapping_add((0x4C * i) as u32);
        r.s1 = u64::from(f);
        let f = f.wrapping_add(0x388C);
        set_counter4(fx, f, 0xAD, 0, 0xFF);
        fx.mem.write_u8(f + 5, 1);
        let b = fx.mem.read_u8(r.s0 as u32 + 0xC);
        set(fx, f, &[(6, b), (4, 0x85), (7, 9), (0xA, 0x10), (9, 0x10), (8, 0x80), (0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0)], &[(0x10, 0x4416_0000), (0x14, 0), (0x18, 0)]);
        r.s0 += 4;
    }
}

/// Update 0x104 [F 0x1E05C28]: fifteen spikes at the target, delays 0, 2,
/// .. and lengths 0x7E - delay.
fn spikes15(fx: &mut Fx, sp: u32, r: &mut Regs) {
    r.s0 = 0;
    r.s1 = 0;
    for _ in 0..0xF {
        let i = first_free(fx, SPIKES, 0x68, 0x64);
        if i >= 0 {
            let s = SPIKES.wrapping_add((0x68 * i) as u32);
            let d = r.s0 as i32;
            set_counter4(fx, s, (0x7E - d) & 0xFF, d & 0xFF, 0xFF);
            fx.mem.write_u8(s + 4, 5);
            let v = ee::div(fx.mem.f(RADIUS), 3.0);
            fx.mem.setf(s + 0x14, v);
            fx.mem.write_u32(s + 0x1C, 0x42A0_0000);
            let s1 = r.s1 as i32;
            fx.mem.set8(s + 0xD, 0x76 - s1);
            set(fx, s, &[(5, 0), (0xA, 0), (0xB, 2), (0xC, 3)], &[]);
            fx.mem.set8(s + 8, s1);
            let v = rand4(fx, r.s0, 0x01E0_5CD8);
            fx.mem.set8(s + 9, v);
            fx.mem.write_u32(s + 0x10, BYTES);
            fx.mem.write_u32(s + 0x18, 0x3F80_0000);
            set(fx, s, &[(0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x25, 0x40), (0x24, 0x80), (0x23, 0x20), (0x27, 0xC0)], &[]);
            for (o, ra) in [(0x58u32, 0x01E0_5D28), (0x5C, 0x01E0_5D38), (0x60, 0x01E0_5D48)] {
                let v = rand4(fx, r.s0, ra) & 0x7F;
                fx.mem.set32(s + o, v);
            }
            copy_v4(fx, sp + 0xE0, s + 0x28);
        }
        r.s1 = (r.s1 + 2) & 0xFF;
        r.s0 += 2;
    }
}

/// Update 0x12C [F 0x1E05DFC]: eight bands at the target (two rows of
/// four, delays 0..3 / 8..11), the fade, the flat window and two domes.
fn bands(fx: &mut Fx, sp: u32, r: &mut Regs) {
    let mut s2 = 0u64;
    for s0 in 0..2u64 {
        r.s1 = s2;
        for _ in 0..4 {
            let i = first_free(fx, BANDS, 0x5C, 0x10);
            if i >= 0 {
                let b = BANDS.wrapping_add((0x5C * i) as u32);
                set_counter4(fx, b, 0x30, (r.s1 & 0xFF) as i32, 0xFF);
                set(fx, b, &[(4, 0), (7, 0x24), (0xC, 1), (8, 2), (9, 2), (0xA, 6), (0xB, 0)], &[(0x10, 0x3F80_0000), (0x14, 0), (0x18, 0x4040_0000), (0x1C, 0x3FE6_6666), (0x20, 0x3E80_0000)]);
                set(fx, b, &[(0x26, 0xA0), (0x24, 0x60), (0x25, 0x60), (0x2A, 0xA0), (0x28, 0x60), (0x29, 0x60), (0x27, 0x10), (0x2B, 0x30)], &[]);
                copy_v4(fx, sp + 0xE0, b + 0x2C);
                if s0 != 0 {
                    fx.mem.set32(b + 0x3C, 0x1000);
                    fx.mem.set32(b + 0x40, 0x4000);
                } else {
                    fx.mem.set32(b + 0x3C, 0x2000);
                    fx.mem.set32(b + 0x40, -0x4000);
                }
                if fx.mem.read_u32(D + 0x30) != 0x8000 {
                    let v = fx.mem.i32(b + 0x40);
                    fx.mem.set32(b + 0x40, v.wrapping_neg());
                }
            }
            r.s1 += 1;
        }
        s2 += 8;
    }
    set_counter4(fx, FADE, 8, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    let n = fx.mem.u16(D);
    fx.mem.set16(FLAT, n);
    let n = fx.mem.u16(D);
    fx.mem.set16(FLAT + 2, n + 0x50);
    fx.mem.write_u32(FLAT + 8, 0x3F80_0000);
    set(fx, FLAT, &[(0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)], &[]);
    r.s0 = u64::from(D);
    for s1 in 0..2u32 {
        r.s1 = u64::from(s1);
        let m = DOMES + 0x50 * s1;
        set_counter4(fx, m, 0x34, ((s1 + 0x18) & 0xFF) as i32, 0xFF);
        set(fx, m, &[(0xA, 1), (7, 6), (8, 3), (9, 2)], &[(0xC, 0x4040_0000), (0x10, 0xBC23_D70A), (0x14, 0)]);
        if s1 == 0 {
            fx.mem.set32(m + 0x34, 0x3000);
        }
        fx.mem.write_u32(m + 0x18, 0x4000_0000);
        copy_v4(fx, sp + 0xE0, m + 0x20);
        set(fx, m, &[(0x1C, 0x80), (0x1D, 0x80), (0x1E, 0x80), (0x1F, 0x80)], &[]);
        r.s0 += 0x50;
    }
    r.s1 = 2;
}

/// A node spike (odd updates) [F 0x1E06148 / 0x1E064D8]: +0x14 = `len`.
fn node_spike(fx: &mut Fx, s: u32, node: u32, len: u32, s0: u64, ras: [u32; 4]) {
    set_counter4(fx, s, 0xC, 0, 0xFF);
    fx.mem.write_u8(s + 4, 5);
    fx.mem.write_u32(s + 0x14, len);
    fx.mem.write_u32(s + 0x1C, 0x4220_0000);
    fx.mem.write_u8(s + 8, 0x14);
    fx.mem.write_u8(s + 5, 1);
    let b = fx.mem.read_u8(node);
    set(fx, s, &[(6, b), (0xA, 0), (0xB, 2), (0xC, 7), (0xD, 0)], &[]);
    let v = rand4(fx, s0, ras[0]);
    fx.mem.set8(s + 9, v);
    fx.mem.write_u32(s + 0x10, BYTES);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    set(fx, s, &[(0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0), (0x27, 0x40)], &[]);
    for (o, ra, m) in [(0x58u32, ras[1], 0x7F), (0x5C, ras[2], 0x7F), (0x60, ras[3], 0xFF)] {
        let v = rand4(fx, s0, ra) & m;
        fx.mem.set32(s + o, v);
    }
}

/// Odd updates 3..0x2F [F 0x1E06124]: per node a spike and 14 drops
/// thrown from it (random turn, (0, (200 (r % 80) - 8000) / 100, 0)).
fn sparks(fx: &mut Fx, sp: u32, r: &mut Regs) {
    let s2 = sp + 0x100;
    r.s0 = u64::from(D);
    for _ in 0..3 {
        let i = first_free(fx, SPIKES, 0x68, 0x64);
        if i >= 0 {
            let s = D.wrapping_add((0x68 * i) as u32);
            r.s1 = u64::from(s);
            node_spike(fx, s + 0xAC, r.s0 as u32 + 0xC, 0x4090_0000, r.s0, [0x01E0_61C0, 0x01E0_6208, 0x01E0_6218, 0x01E0_6228]);
        }
        for _ in 0..0xE {
            let j = first_free(fx, DROPS, 0x44, 0x190);
            r.s1 = sx(j);
            if j < 0 {
                continue;
            }
            let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(r.s0 as u32 + 0xC));
            node_position_saving(fx, h, slot, sp + 0xF0, r.s0, Some(r.s1), 0x01E0_626C);
            let v = rand4(fx, r.s0, 0x01E0_6274);
            let k = v % 4;
            let f20 = ee::add(12.0, ee::div(ee::from_i32(200 * k - 400), 100.0));
            fx.fix_residue(f20, 0x01E0_62D4);
            let count = ee::fixsfsi(f20) & 0xFF;
            let d = D.wrapping_add((0x44 * j) as u32);
            r.s1 = u64::from(d);
            let d = d + 0x50BC;
            set_counter4(fx, d, count, 0, 0xFF);
            set(fx, d, &[(4, 0x40), (8, 0), (0xB, 2)], &[]);
            let v = (rand4(fx, r.s0, 0x01E0_6310) + 2) & 3;
            fx.mem.set8(d + 9, v);
            set(fx, d, &[(0xA, 0), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)], &[(0x10, 0x42A0_0000), (0x14, 0xC000_0000), (0x1C, 0), (0x18, 0)]);
            for (o, ra) in [(0x1B0u32, 0x01E0_6354), (0x1B4, 0x01E0_6364), (0x1B8, 0x01E0_6374)] {
                let v = rand4(fx, r.s0, ra) & 0xFFFF;
                fx.mem.set32(sp + o, v);
            }
            let rot = [fx.mem.i32(sp + 0x1B0), fx.mem.i32(sp + 0x1B4), fx.mem.i32(sp + 0x1B8)];
            let m = fx.trs(None, Some(rot), None);
            fx.write_m4(sp + 0x160, &m);
            fx.mem.write_u32(sp + 0x1A8, 0);
            fx.mem.write_u32(sp + 0x1A0, 0);
            let y = jit(fx, r.s0, 0x01E0_63A0, 0x50, 200, 0x1F40);
            fx.mem.setf(sp + 0x1A4, y);
            let m = fx.read_m4(sp + 0x160);
            let v = fx.read_v4(sp + 0x1A0);
            let o = fx.transform(Some(&m), v);
            fx.write_v4(s2, o);
            for c in 0..3u32 {
                let v = ee::add(fx.mem.f(sp + 0xF0 + 4 * c), fx.mem.f(s2 + 4 * c));
                fx.mem.setf(d + 0x20 + 4 * c, v);
            }
            for c in 0..3u32 {
                let v = ee::neg(ee::div(fx.mem.f(s2 + 4 * c), f20));
                fx.mem.setf(d + 0x2C + 4 * c, v);
            }
            for o in [0x40u32, 0x3C, 0x38] {
                fx.mem.write_u32(d + o, 0);
            }
        }
        r.s0 += 4;
    }
}

/// Odd updates 0x43..0xEF [F 0x1E064B8]: a spike at each node.
fn node_spikes(fx: &mut Fx, r: &mut Regs) {
    r.s0 = u64::from(D);
    for _ in 0..3 {
        let i = first_free(fx, SPIKES, 0x68, 0x64);
        if i >= 0 {
            let s = D.wrapping_add((0x68 * i) as u32);
            r.s1 = u64::from(s);
            node_spike(fx, s + 0xAC, r.s0 as u32 + 0xC, 0x40A0_0000, r.s0, [0x01E0_6550, 0x01E0_6598, 0x01E0_65A8, 0x01E0_65B8]);
            for o in [0x50u32, 0x4C, 0x48] {
                fx.mem.write_u32(s + 0xAC + o, 0);
            }
        }
        r.s0 += 4;
    }
}

/// Even updates 0x4C..0xEE [F 0x1E06608]: a ribbon along a 20-point arc
/// from node i % 3 to the target.
fn ribbon(fx: &mut Fx, sp: u32, r: &mut Regs) {
    let i = first_free(fx, RIBBONS, 0x20, 0x12);
    r.s1 = sx(i);
    if i < 0 {
        return;
    }
    let (_, m) = fx.div(i, 3);
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC + 4 * m as u32));
    node_position_saving(fx, h, slot, sp + 0xF0, r.s0, Some(r.s1), 0x01E0_6654);
    fx.mem.set32(sp + 0x130, 0x14);
    let v = ee::add(40.0, jit(fx, r.s0, 0x01E0_6660, 0x23, 100, 0x6D6));
    fx.mem.setf(sp + 0x134, v);
    let v = jit(fx, r.s0, 0x01E0_66B8, 0xB4, 200, 0x4650);
    fx.mem.setf(sp + 0x138, v);
    let v = rand4(fx, r.s0, 0x01E0_670C);
    let k = v % 8;
    let f6 = ee::div(ee::from_i32(100 * k - 400), 100.0);
    let path = PATHS + 0x120 * i as u32;
    r.s0 = u64::from(path);
    fx.mem.setf(sp + 0x13C, f6);
    fx.mem.write_u32(sp + 0x158, path);
    for c in 0..3u32 {
        let w = fx.mem.read_u32(sp + 0xF0 + 4 * c);
        fx.mem.write_u32(sp + 0x140 + 4 * c, w);
    }
    for c in 0..3u32 {
        let w = fx.mem.read_u32(sp + 0xE0 + 4 * c);
        fx.mem.write_u32(sp + 0x14C + 4 * c, w);
    }
    crate::b173::arc_points(fx, sp + 0x130);
    let n = fx.mem.i32(sp + 0x130) + 6;
    let b = RIBBONS + ((i as u32) << 5);
    r.s1 = u64::from((i as u32) << 5);
    set_counter4(fx, b, n & 0xFF, 0, 0xFF);
    let k = fx.mem.read_u8(sp + 0x130);
    set(fx, b, &[(5, 0), (4, 0xB), (8, k), (9, 0), (0xA, 2), (0xB, 0), (0xC, 3)], &[(0x10, path), (0x14, 0x42F0_0000), (0x18, 0x3ECC_CCCD)]);
    set(fx, b, &[(0x1F, 0x80), (0x1C, 0x80), (0x1D, 0x80), (0x1E, 0x80)], &[]);
}

/// The vector from point `k` of path `p` to point k - 1 into sp + 0x120.
fn back(fx: &mut Fx, sp: u32, p: u32) {
    for c in 0..3u32 {
        let v = ee::sub(fx.mem.f(p - 0xC + 4 * c), fx.mem.f(p + 4 * c));
        fx.mem.setf(sp + 0x120 + 4 * c, v);
    }
}

/// The 18 ribbons' followers [F 0x1E0689C..0x1E070F0].
fn followers(fx: &mut Fx, sp: u32, r: &mut Regs) {
    fx.mem.set32(sp + 0xD0, 0);
    fx.mem.write_u32(sp + 0xC0, D);
    r.s0 = u64::from(D);
    while fx.mem.i32(sp + 0xD0) < 0x12 {
        let p = fx.mem.read_u32(sp + 0xC0);
        let s2 = fx.mem.u8(p + 0xBAFC) - 1;
        let s3 = fx.mem.u8(p + 0xBAFE) - s2;
        let s0 = r.s0 as u32;
        // Point s3 of this ribbon's path.
        let pt = s0.wrapping_add((12 * s3) as u32).wrapping_add(0xBD3C);
        if s3 >= 2 && s2 >= 7 {
            let i = first_free(fx, SPIKES, 0x68, 0x64);
            if i >= 0 {
                let rec = D.wrapping_add((0x68 * i) as u32);
                r.s1 = u64::from(rec);
                let s = rec + 0xAC;
                set_counter4(fx, s, 6, 0, 0xFF);
                fx.mem.write_u8(s, 1);
                fx.mem.write_u8(s + 4, 0x44);
                fx.mem.write_u32(s + 0x14, 0x4080_0000);
                fx.mem.write_u32(s + 0x1C, 0x4270_0000);
                set(fx, s, &[(8, 0x10), (5, 0), (0xA, 0), (0xB, 2), (0xC, 7), (0xD, 0)], &[]);
                let v = rand4(fx, r.s0, 0x01E0_6970);
                fx.mem.set8(s + 9, v);
                fx.mem.write_u32(s + 0x10, BYTES);
                fx.mem.write_u32(s + 0x18, 0x3F80_0000);
                set(fx, s, &[(0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x10), (0x27, 0x80)], &[(0x60, 0), (0x5C, 0), (0x58, 0)]);
                for c in 0..3u32 {
                    let w = fx.mem.read_u32(pt + 4 * c);
                    fx.mem.write_u32(s + 0x28 + 4 * c, w);
                }
                back(fx, sp, pt);
                aim_saving(fx, sp + 0x120, s + 0x48);
            }
            for s1 in 0..4u64 {
                r.s1 = s1;
                let j = first_free(fx, DROPS, 0x44, 0x190);
                if j < 0 {
                    continue;
                }
                let delay = rand4(fx, r.s0, 0x01E0_6A94) & 3;
                let d = DROPS + 0x44 * j as u32;
                set_counter4(fx, d, 8, delay, 0xFF);
                set(fx, d, &[(4, 0x40), (8, 0), (0xB, 2)], &[]);
                let v = (rand4(fx, r.s0, 0x01E0_6AD4) + 2) & 3;
                fx.mem.set8(d + 9, v);
                set(fx, d, &[(0xA, 0), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x60)], &[(0x10, 0x42B4_0000), (0x14, 0), (0x1C, 0), (0x18, 0)]);
                for (o, ra) in [(0x2Cu32, 0x01E0_6B18), (0x30, 0x01E0_6B54), (0x34, 0x01E0_6B90)] {
                    let v = jit(fx, r.s0, ra, 0x96, 1, 0x4B);
                    fx.mem.setf(d + o, v);
                }
                for c in 0..3u32 {
                    let w = fx.mem.read_u32(pt + 4 * c);
                    fx.mem.write_u32(d + 0x20 + 4 * c, w);
                }
                for o in [0x40u32, 0x3C, 0x38] {
                    fx.mem.write_u32(d + o, 0);
                }
            }
            r.s1 = 4;
        }
        if s2 == 9 {
            finish(fx, sp, r, pt);
        }
        let p = fx.mem.read_u32(sp + 0xC0);
        fx.mem.write_u32(sp + 0xC0, p + 0x20);
        let k = fx.mem.i32(sp + 0xD0);
        fx.mem.set32(sp + 0xD0, k + 1);
        r.s0 += 0x120;
    }
}

/// A ribbon's count 9 [F 0x1E06C1C]: an annulus at its point s3, a spike
/// from the target aimed back along the path, two bolts there.
fn finish(fx: &mut Fx, sp: u32, r: &mut Regs, pt: u32) {
    let i = first_free(fx, ANNULUS, 0x3C, 0x1E);
    if i >= 0 {
        let a = D.wrapping_add((0x3C * i) as u32);
        r.s1 = u64::from(a);
        let a = a + 0x4734;
        set_counter4(fx, a, 0x18, 0, 0xFF);
        set(fx, a, &[(5, 0), (7, 9)], &[(0x10, 0x3E99_999A), (0x14, 0)]);
        set(fx, a, &[(0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0xC0), (9, 0), (0xA, 0), (8, 0x80), (0xB, 0)], &[]);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(pt + 4 * c);
            fx.mem.write_u32(a + 0x20 + 4 * c, w);
        }
        back(fx, sp, pt);
        aim_saving(fx, sp + 0x120, a + 0x2C);
        fx.mem.write_u32(a + 0x18, 0x3F4C_CCCD);
        fx.mem.write_u32(a + 0x1C, 0x3E99_999A);
    }
    let i = first_free(fx, SPIKES, 0x68, 0x64);
    if i >= 0 {
        let rec = D.wrapping_add((0x68 * i) as u32);
        r.s1 = u64::from(rec);
        let s = rec + 0xAC;
        set_counter4(fx, s, 0x12, 0, 0xFF);
        fx.mem.write_u8(s + 4, 4);
        fx.mem.write_u32(s + 0x14, 0x40A0_0000);
        fx.mem.write_u32(s + 0x1C, 0x4234_0000);
        set(fx, s, &[(8, 0xA), (5, 0), (0xA, 0), (0xB, 2), (0xC, 6), (0xD, 0)], &[]);
        let v = rand4(fx, r.s0, 0x01E0_6DCC);
        fx.mem.set8(s + 9, v);
        fx.mem.write_u32(s + 0x10, BYTES);
        fx.mem.write_u32(s + 0x18, 0x3F00_0000);
        set(fx, s, &[(0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0), (0x27, 0x60)], &[(0x5C, 0), (0x58, 0), (0x60, 0)]);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0xE0 + 4 * c);
            fx.mem.write_u32(s + 0x28 + 4 * c, w);
        }
        back(fx, sp, pt);
        aim_saving(fx, sp + 0x120, s + 0x48);
    }
    for c in 0..3u32 {
        let w = fx.mem.read_u32(pt + 4 * c);
        fx.mem.write_u32(sp + 0x100 + 4 * c, w);
    }
    for s5 in 0..2u32 {
        let i = first_free(fx, BOLTS, 0x40, 0x3C);
        if i < 0 {
            return;
        }
        let rec = D + 0x40 * i as u32;
        r.s1 = u64::from(rec);
        let b = rec + 0x294C;
        set_counter4(fx, b, 0xA, (s5 & 0xFF) as i32, 0xFF);
        fx.mem.write_u8(b + 5, 0);
        fx.mem.write_u8(b + 4, 0x84);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x100 + 4 * c);
            fx.mem.write_u32(b + 0x28 + 4 * c, w);
            fx.mem.write_u32(b + 0x1C + 4 * c, w);
        }
        for (c, ra) in [(0u32, 0x01E0_6F78), (4, 0x01E0_6FD0), (8, 0x01E0_7028)] {
            let v = rand4(fx, r.s0, ra);
            let (_, m) = fx.div(v, 0x1E);
            let p = fx.mem.f(b + 0x28 + c);
            let v = ee::add(p, ee::div(ee::from_i32(200 * m - 0xBB8), 100.0));
            fx.mem.setf(b + 0x28 + c, v);
        }
        set(fx, b, &[(8, 6), (0xB, 1), (0xD, 4), (0xC, 0), (0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x20)], &[(0x10, 0x3F80_0000), (0x14, 0x3ECC_CCCD)]);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, sp + 0xE0);
    let mut r = Regs { s0: sx(param), s1: RUNNER_S1 };
    match fx.mem.u16(D) {
        0x1C2 => return 8,
        0x17E => {
            set_counter4(fx, FADE, 8, 0, 0xFF);
            fx.mem.write_u8(FADE + 4, 4);
            // a0..a3 as 0x101AD0 leaves them.
            if fx.outcome([FADE, 8, 0, 0xFF]) != 0 {
                set(fx, sp, &[(0x1CE, 0x50), (0x1CD, 0x30), (0x1CC, 0x50), (0x1CF, 0x80)], &[]);
                let t = fx.mem.i32(D + 8);
                unit_tint_saving(fx, t, sp + 0x1CC, [r.s0, r.s1], 0x01E0_60E8);
            }
        }
        0x16E => {
            set_counter4(fx, FADE, 0x10, 0, 0xFF);
            fx.mem.write_u8(FADE + 4, 2);
        }
        n => {
            r.s0 = 0;
            match n {
                0x12C => bands(fx, sp, &mut r),
                0x11D => window(fx, 0x11D, 0x19C),
                0x104 => spikes15(fx, sp, &mut r),
                0x44 => {
                    window(fx, 0x44, 0x44 + 0x28);
                    set_counter4(fx, FADE, 8, 8, 0xFF);
                    fx.mem.write_u8(FADE + 4, 0x82);
                }
                0x43 => flashes3(fx, &mut r),
                2 => flashes5(fx, &mut r),
                _ => {}
            }
        }
    }
    let n = fx.mem.u16(D);
    if n & 1 != 0 && n >= 2 && n < 0x30 {
        sparks(fx, sp, &mut r);
    }
    let n = fx.mem.u16(D);
    if n & 1 != 0 && n >= 0x43 && n < 0xF0 {
        node_spikes(fx, &mut r);
    }
    let n = fx.mem.u16(D);
    if n >= 0x4B && n < 0xF0 && n & 1 == 0 {
        ribbon(fx, sp, &mut r);
    }
    followers(fx, sp, &mut r);
    shake_saving(fx, SHAKE, r.s0, 0x01E0_70F8, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, LIGHT, n, 0x01E0_7104, [Some(r.s0), Some(r.s1)]);
    let ot = fx.mem.read_u32(sp + 0xB8);
    // A drawn fade leaves a3 = its descriptor's flags halfword 0x14 (0x207E20's
    // leftover; 0x206F30 keeps it) [F 0x1E00658..0x1E00838].
    let fade_a3 = (blib::fade::fade(fx, FADE, ot) == 1).then_some(0x14);
    let (_, flash_a3) = blib::star::flash_b_a3(fx, 0x01E0_22B0, FLASH, ot, 0x10);
    let (_, drop_a3) = blib::drop::drops_a3(fx, DROPS, ot, 0x190);
    let (_, dome_a3) = blib::dome::domes_a3(fx, 0x01E0_3590, DOMES, ot, 8);
    blib::dash::ribbons(fx, 0x01E0_2D20, RIBBONS, ot, 0x12);
    let (_, spike_a3) = blib::spike::spikes_a3(fx, 0x01E0_0870, SPIKES, ot, 0x64);
    let (_, band_a3) = blib::band::bands_b_a3(fx, 0x01E0_3FD0, BANDS, ot, 0x10);
    blib::annulus::flat(fx, 0x01E0_4710, ANNULUS, ot, 0x1E);
    let (_, bolt_a3) = blib::bolt::bolts_b_a3(fx, 0x01E0_4CE0, BOLTS, ot, 0x3C);
    // The a3 the last drawing call left ([H] ribbons / annuli not
    // modelled; nothing drawn: 0).
    let a3 = bolt_a3.or(band_a3).or(spike_a3).or(dome_a3).or(drop_a3).or(flash_a3).or(fade_a3).unwrap_or(0);
    let n = fx.mem.u16(D);
    blib::window::frame_fade_b(fx, 0x01E0_1CB0, WINDOW, ot, n, a3);
    let n = fx.mem.u16(D);
    blib::window::flat(fx, 0x01E0_2130, FLAT, ot, n);
    // Its frame (0x80: s0..s3 +0..+0x30, ra +0x40) stays below the entry's
    // sp; a later update's frame_fade_b reads the s3 slot's zero upper half
    // as vertex 3's stale z [run]. The lower halves (the entry's s0..s3)
    // are not modelled [H].
    for o in [0u32, 0x10, 0x20, 0x30] {
        fx.mem.write_u64(sp - 0x80 + o + 8, 0);
    }
    fx.mem.write_u64(sp - 0x40, 0x01E0_71B4);
    let n = fx.mem.u16(D);
    if (0x5F..0x179).contains(&n) {
        6
    } else if n == 0x179 {
        2
    } else if n == 0x55 {
        1
    } else {
        0
    }
}
