//! BMODULE b051 (card 398), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04F10, data area a0 = 0x1ED0000, 0xB820 bytes)
//! [F]: s3 = D, fp = the OT (0x206E40), s0 the
//! parameter then counters / offsets. The entry first copies the record's
//! 17-word pulse table (0x1E06720) to sp + 0x110. Update 2 reads nodes 1 /
//! 0x18 (+0xB704, y from 0x18); 80 a long flash (+0x771C), a spike (+0xA4)
//! and a long shake at node 0xC; 105 the fade (+0x7714), a flash at node
//! 0xC, eight beads (+0x8E4C, `beam::beads_at`) and the attacker's streaks
//! (+0x8E14, `radiate::streaks_b`, far alpha = table x 5 from update 16);
//! 165 a shake at the target. On updates 20..283 eight sprites (+0xCD4)
//! fountain about nodes 0xB / 0x38 / 0x2F (aimed by 0x101A00); on every
//! fourth update 105..235 a bolt (+0x910C, 0x54 apart, `arch::arches_b`)
//! along 21 points (+0x9AE4, 240 bytes apart, 0x102F20) from node 1 to the
//! target jittered; each bolt walks its points and on its 19th step bursts
//! at its end: three panels (+0x8004, `panel::panels`), a flash, a spike
//! and 25 bouncing sprites. Returns 6 on updates 129..267, 2 on 268, 1 on
//! 119, 8 on 320 (no drawing then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `flash_b`
//! 0x1E003A0, `arches_b` 0x1E00E10, `streaks_b` 0x1E01670, `spikes`
//! 0x1E02470, `drops` 0x1E02F50, `fade` 0x1E038B0, `panels` 0x1E03D80,
//! `beads_at` 0x1E04290 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::b173::arc_points;
use crate::blib::{self, exe::{aim_angles, first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const PULSE: u32 = 0x01E0_6720;
const SPIKES: u32 = D + 0xA4;
const DROPS: u32 = D + 0xCD4;
const FADE: u32 = D + 0x7714;
const FLASH: u32 = D + 0x771C;
const PANELS: u32 = D + 0x8004;
const STREAKS: u32 = D + 0x8E14;
const BEADS: u32 = D + 0x8E4C;
const BOLTS: u32 = D + 0x910C;
const POINTS: u32 = D + 0x9AE4;
const BYTES: u32 = D + 0xB714;

/// The entry 0x1E04F10 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1E0);
    for k in (0..0x40u32).step_by(8) {
        let v = fx.mem.read_u64(PULSE + k);
        fx.mem.write_u64(sp + 0x110 + k, v);
    }
    let v = fx.mem.read_u32(PULSE + 0x40);
    fx.mem.write_u32(sp + 0x150, v);
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
    // 0x199208 memset(D, 0, 0xB820).
    fx.bzero(D, 0xB820);
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
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0x18, 0), (0x14, 0xB, 0), (0x18, 0x38, 0), (0x1C, 0x2F, 0), (0x20, 0xC, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x13A);
    // The entry's s0 = 3 (the texture loop's), s1 / s2 the runner's 0.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_5090, [Some(3), Some(0), Some(0)]);
    0
}

/// 0x101940 `aim_angles(v, out)` leaving its second atan2f's 0x1A5020
/// frame: ra 0x1019B4 at the caller's sp - 0x38 (as b173).
fn aim_saving(fx: &mut Fx, v: u32, out: u32) {
    aim_angles(fx, v, out);
    let sp = fx.sp;
    fx.mem.write_u64(sp - 0x38, 0x0010_19B4);
}

/// rand from return address `ra` with the entry's s0 (64 bits) saved in
/// its frame.
fn rand64(fx: &mut Fx, s0: u64, ra: u32) -> i32 {
    let f = fx.sp - 0x10;
    fx.mem.write_u64(f, s0);
    fx.mem.write_u64(f + 8, u64::from(ra));
    fx.rand()
}

/// ((rand >> 4) % m - half) / 100.
fn jit(fx: &mut Fx, s0: u64, ra: u32, m: i32, half: i32) -> f32 {
    let v = rand64(fx, s0, ra) >> 4;
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(r - half), 100.0)
}

/// (200 ((rand >> 4) % m) - half) / 100 (the bolts' jitter).
fn spread(fx: &mut Fx, s0: u64, ra: u32, m: i32, half: i32) -> f32 {
    let v = rand64(fx, s0, ra) >> 4;
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(200 * r - half), 100.0)
}

/// The s registers the step's calls save.
struct Regs {
    s0: u64,
    s1: u64,
    s6: u64,
    /// a0's upper 64 bits: 0x101A00 leaves its `move128 a0, s0` (the
    /// entry's a1, whose upper half is still the pulse table's words 6 / 7
    /// from the entry's `lq`); 0x1F6830 clears them [run].
    a0_hi: u64,
}

fn sx(v: i32) -> u64 {
    i64::from(v) as u64
}

/// Update 80 [F 0x1E05134]: a long flash, a spike and a shake at node 0xC.
fn charge(fx: &mut Fx, r: &Regs) {
    set_counter4(fx, FLASH, 0x20, 0, 0xFF);
    fx.mem.write_u8(FLASH + 5, 1);
    let b = fx.mem.read_u8(D + 0x20);
    fx.mem.write_u8(FLASH + 6, b);
    for (o, v) in [(4u32, 0x80u8), (7, 9), (0xA, 0x10), (9, 0x10), (8, 0x80), (0xF, 0xF0), (0xC, 0xF0), (0xD, 0xF0), (0xE, 0xF0)] {
        fx.mem.write_u8(FLASH + o, v);
    }
    fx.mem.write_u32(FLASH + 0x10, 0x44C8_0000);
    fx.mem.write_u32(FLASH + 0x14, 0xC248_0000);
    fx.mem.write_u32(FLASH + 0x18, 0x4120_0000);
    let s = SPIKES;
    set_counter4(fx, s, 0x20, 0, 0xFF);
    fx.mem.write_u8(s + 5, 1);
    let b = fx.mem.read_u8(D + 0x20);
    fx.mem.write_u8(s + 6, b);
    for (o, v) in [(4u32, 0x85u8), (0xA, 1), (0xC, 3)] {
        fx.mem.write_u8(s + o, v);
    }
    fx.mem.write_u32(s + 0x14, 0x4040_0000);
    fx.mem.write_u32(s + 0x1C, 0x41F0_0000);
    fx.mem.write_u8(s + 0x23, 0);
    fx.mem.write_u8(s + 0x27, 0x60);
    fx.mem.write_u8(s + 8, 0x28);
    fx.mem.write_u32(s + 0x58, 0);
    fx.mem.write_u32(s + 0x5C, 0);
    let v = if fx.mem.read_u32(D + 0x30) == 0x8000 { 0x300 } else { -0x300 };
    fx.mem.set32(s + 0x60, v);
    fx.mem.write_u8(s + 0xB, 2);
    let x = rand64(fx, r.s0, 0x01E0_5234);
    fx.mem.set8(s + 9, x >> 4);
    fx.mem.write_u32(s + 0x10, BYTES);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(s + o, 0x80);
    }
    set_counter4(fx, D + 0x40, 0x63, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 1);
    let b = fx.mem.read_u8(D + 0x20);
    fx.mem.write_u8(D + 0x46, b);
    fx.mem.write_u8(D + 0x44, 0);
    fx.mem.write_u32(D + 0x54, 0x4000_0000);
    fx.mem.write_u32(D + 0x58, 0x3FCC_CCCD);
    fx.mem.write_u32(D + 0x5C, 0x3F80_0000);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x42A0_0000);
    fx.mem.write_u32(D + 0x90, 0x4080_0000);
}

/// Update 105 [F 0x1E052C4]: the fade, a flash at node 0xC, eight beads
/// and the attacker's streaks.
fn strike(fx: &mut Fx, sp: u32, r: &mut Regs) {
    set_counter4(fx, FADE, 0xC, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x20));
    node_position_saving(fx, h, s, sp + 0xB0, r.s0, Some(r.s1), 0x01E0_52EC);
    r.a0_hi = 0;
    let i = first_free(fx, FLASH, 0x4C, 0x1E);
    if i >= 0 {
        r.s0 = 0x4C * i as u64;
        let a = FLASH + 0x4C * i as u32;
        set_counter4(fx, a, 0x14, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0x81), (7, 9), (0xA, 0x40), (9, 0x20), (8, 0x80), (0xF, 0xF0), (0xC, 0xF0), (0xD, 0xF0), (0xE, 0xF0)] {
            fx.mem.write_u8(a + o, v);
        }
        fx.mem.write_u32(a + 0x10, 0x4348_0000);
        fx.mem.write_u32(a + 0x14, 0x4316_0000);
        copy_v4(fx, sp + 0xB0, a + 0x1C);
    }
    for s1 in 0..8u32 {
        let b = BEADS + 0x58 * s1;
        r.s0 = u64::from(D + 0x58 * s1);
        set_counter4(fx, b, 0x20, s1 as i32, 0xFF);
        let c = fx.mem.read_u8(b).wrapping_sub(4);
        fx.mem.write_u8(b, c);
        for (o, v) in [(4u32, 5u8), (5, 0), (8, 0x3C), (9, s1 as u8), (0xA, 3), (0xC, 2)] {
            fx.mem.write_u8(b + o, v);
        }
        let x = rand64(fx, r.s0, 0x01E0_5440);
        fx.mem.set8(b + 0xB, (x >> 4) & 3);
        fx.mem.write_u32(b + 0x1C, 0x42F0_0000);
        fx.mem.write_u32(b + 0x20, 0x3F19_999A);
        fx.mem.write_u32(b + 0x10, BYTES);
        fx.mem.write_u32(b + 0x14, 0x4080_0000);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(b + o, 0x80);
        }
        copy_v4(fx, sp + 0xB0, b + 0x28);
    }
    r.s0 = u64::from(D + 0x2C0);
    r.s1 = 8;
    let s = STREAKS;
    set_counter4(fx, s, 0xFF, 0, 0xFF);
    for (o, v) in [(4u32, 0x80u8), (9, 2), (0xA, 0xFF), (8, 0x10), (0xB, 6)] {
        fx.mem.write_u8(s + o, v);
    }
    let h = fx.mem.read_u32(D + 4);
    fx.mem.write_u32(s + 0xC, h);
    for (o, v) in [(0x16u32, 0x80u8), (0x15, 0x80), (0x14, 0x80), (0x17, 0x10), (0x12, 0x90), (0x11, 0x90), (0x10, 0xF0), (0x13, 0), (5, 1)] {
        fx.mem.write_u8(s + o, v);
    }
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(s + 6, b);
}

/// Updates 20..283 [F 0x1E056F4]: four sprites from node 0xB towards each
/// of nodes 0x38 / 0x2F.
fn fountains(fx: &mut Fx, sp: u32, r: &mut Regs) {
    for (slot, out, ra) in [(0x14u32, 0xB0u32, 0x01E0_5704u32), (0x18, 0xC0, 0x01E0_5714), (0x1C, 0xD0, 0x01E0_5724)] {
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + slot));
        node_position_saving(fx, h, s, sp + out, r.s0, Some(r.s1), ra);
        r.a0_hi = 0;
    }
    for s1 in 0..2u32 {
        r.s1 = u64::from(s1);
        let s2 = sp + 0xB0 + 0x10 * s1;
        for s0 in 0..4u32 {
            r.s0 = u64::from(s0);
            let i = first_free(fx, DROPS, 0x44, 0x190);
            if i < 0 {
                continue;
            }
            let delay = (rand64(fx, r.s0, 0x01E0_5754) >> 4) & 7;
            let q = DROPS + 0x44 * i as u32;
            set_counter4(fx, q, 0x14, delay, 0xFF);
            fx.mem.write_u8(q + 4, 0x41);
            fx.mem.write_u8(q + 8, 1);
            fx.mem.write_u8(q + 0xB, 2);
            let v = rand64(fx, r.s0, 0x01E0_5798);
            fx.mem.set8(q + 9, (v >> 4) & 7);
            fx.mem.write_u8(q + 0xA, 0);
            for o in [0xCu32, 0xD, 0xE] {
                fx.mem.write_u8(q + o, 0x80);
            }
            let v = rand64(fx, r.s0, 0x01E0_57BC);
            fx.mem.set8(q + 0xF, ((v >> 4) & 0x3F) + 0x60);
            fx.mem.write_u32(q + 0x10, 0x4220_0000);
            fx.mem.write_u32(q + 0x14, 0xBECC_CCCD);
            fx.mem.write_u32(q + 0x1C, 0);
            fx.mem.write_u32(q + 0x18, 0);
            for c in 0..3u32 {
                let v = ee::sub(fx.mem.f(s2 + 0x10 + 4 * c), fx.mem.f(sp + 0xB0 + 4 * c));
                fx.mem.setf(sp + 0x160 + 4 * c, v);
            }
            r.s6 = u64::from(sp + 0x164);
            blib::spray::aim_rotation(fx, sp + 0x160, sp + 0x180, [r.s0, r.s1], 0x01E0_582C);
            r.a0_hi = 0x0000_000E_0000_000C;
            fx.mem.write_u32(sp + 0x168, 0);
            fx.mem.write_u32(sp + 0x160, 0);
            let v = (rand64(fx, r.s0, 0x01E0_5838) >> 4) & 0xFF;
            fx.mem.setf(sp + 0x164, ee::div(ee::from_i32(v), 200.0));
            let m = fx.read_m4(sp + 0x180);
            let x = fx.read_v4(sp + 0x160);
            let t = fx.transform(Some(&m), x);
            fx.write_v4(sp + 0x170, t);
            let f = ee::add(fx.mem.f(sp + 0x170), jit(fx, r.s0, 0x01E0_587C, 0x64, 0x32));
            fx.mem.setf(q + 0x2C, f);
            let f = ee::add(fx.mem.f(sp + 0x174), jit(fx, r.s0, 0x01E0_58C0, 0x64, 0x32));
            fx.mem.setf(q + 0x30, f);
            r.s6 = u64::from(q + 0x30);
            let f = if fx.mem.read_u32(D + 0x30) == 0x8000 {
                let z = ee::add(0.5, fx.mem.f(sp + 0x178));
                ee::add(z, jit(fx, r.s0, 0x01E0_5914, 0x3C, 0x1E))
            } else {
                let z = ee::sub(fx.mem.f(sp + 0x178), 0.5);
                ee::add(z, jit(fx, r.s0, 0x01E0_5968, 0x3C, 0x1E))
            };
            fx.mem.setf(q + 0x34, f);
            for c in 0..3u32 {
                let v = ee::add(ee::mul(2.0, fx.mem.f(q + 0x2C + 4 * c)), fx.mem.f(s2 + 0x10 + 4 * c));
                fx.mem.setf(q + 0x20 + 4 * c, v);
            }
            for o in [0x40u32, 0x3C, 0x38] {
                fx.mem.write_u32(q + o, 0);
            }
        }
    }
    r.s0 = 4;
    r.s1 = 2;
}

/// Every fourth update 105..235 [F 0x1E05A48]: a bolt from node 1 to the
/// target (sp + 0xA0) jittered.
fn bolt(fx: &mut Fx, sp: u32, r: &mut Regs) {
    let i = first_free(fx, BOLTS, 0x54, 0x1E);
    r.s0 = sx(i);
    if i < 0 {
        return;
    }
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
    node_position_saving(fx, h, s, sp + 0xB0, r.s0, Some(r.s1), 0x01E0_5A78);
    // ... and the record's `move128 a0, s2` (upper half 0) [F 0x1E05C8C].
    r.a0_hi = 0;
    fx.mem.set32(sp + 0xE0, 0x14);
    let f = ee::add(40.0, spread(fx, r.s0, 0x01E0_5A84, 0x19, 0x9C4));
    fx.mem.setf(sp + 0xE4, f);
    let f = spread(fx, r.s0, 0x01E0_5ADC, 0xB4, 0x4650);
    fx.mem.setf(sp + 0xE8, f);
    let s1 = 240 * i as u32;
    r.s1 = u64::from(s1);
    fx.mem.write_u32(sp + 0x108, POINTS + s1);
    for c in 0..3u32 {
        let w = fx.mem.read_u32(sp + 0xB0 + 4 * c);
        fx.mem.write_u32(sp + 0xF0 + 4 * c, w);
    }
    for (c, ra, m, half) in [(0u32, 0x01E0_5B54u32, 0xF, 0x5DC), (1, 0x01E0_5BAC, 0xF, 0x5DC), (2, 0x01E0_5C04, 0xA, 0x3E8)] {
        let f = ee::add(fx.mem.f(sp + 0xA0 + 4 * c), spread(fx, r.s0, ra, m, half));
        fx.mem.setf(sp + 0xFC + 4 * c, f);
    }
    arc_points(fx, sp + 0xE0);
    let s0 = 0x54 * i as u32;
    r.s0 = u64::from(s0);
    let b = BOLTS + s0;
    set_counter4(fx, b, 0x14, 0, 0xFF);
    for (o, v) in [(4u32, 0x41u8), (5, 0), (7, 6), (0xE, 0x20), (0xD, 0x20), (0xC, 0xC0), (0xF, 0x20), (0xB, 0xC0), (8, 0xC0), (9, 0xC0), (0xA, 0xC0)] {
        fx.mem.write_u8(b + o, v);
    }
    fx.mem.write_u32(b + 0x10, 0x4150_0000);
    fx.mem.write_u32(b + 0x14, 0x4000_0000);
    fx.mem.write_u32(b + 0x18, 0);
    let v = rand64(fx, r.s0, 0x01E0_5D54) >> 4;
    fx.mem.set32(b + 0x1C, v & 0x7FFF);
    fx.mem.set32(b + 0x20, 0x300);
    let p = POINTS + s1;
    for c in 0..3u32 {
        let w = fx.mem.read_u32(p + 4 * c);
        fx.mem.write_u32(b + 0x24 + 4 * c, w);
    }
    for o in [0x34u32, 0x38, 0x3C] {
        fx.mem.write_u32(b + o, 0);
    }
    for c in 0..3u32 {
        let v = ee::sub(fx.mem.f(p + 0xC + 4 * c), fx.mem.f(b + 0x24 + 4 * c));
        fx.mem.setf(sp + 0x1C0 + 4 * c, v);
    }
    aim_saving(fx, sp + 0x1C0, b + 0x44);
}

/// A bolt's end [F 0x1E05F4C]: three panels, a flash, a spike and 25
/// bouncing sprites at its last point (sp + 0xB0).
fn burst(fx: &mut Fx, sp: u32, s7: u32, r: &mut Regs) {
    let b = BOLTS + 0x54 * s7;
    let p = POINTS + 240 * s7;
    fx.mem.write_u8(b, 0);
    for c in 0..3u32 {
        let w = fx.mem.read_u32(p + 0xE4 + 4 * c);
        fx.mem.write_u32(sp + 0xB0 + 4 * c, w);
    }
    r.s0 = u64::from(sp + 0xB4);
    r.s1 = u64::from(sp + 0xB8);
    for s2 in 0..3i32 {
        let i = first_free(fx, PANELS, 0x3C, 0x3C);
        if i < 0 {
            continue;
        }
        let q = PANELS + 0x3C * i as u32;
        set_counter4(fx, q, 0xE, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0), (8, 2), (0xA, 0), (9, 6), (0xB, 2), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)] {
            fx.mem.write_u8(q + o, v);
        }
        copy_v4(fx, sp + 0xB0, q + 0x2C);
        let dy = ee::add(-1.5, ee::mul(1.5, ee::from_i32(s2)));
        let y = ee::add(fx.mem.f(q + 0x30), dy);
        fx.mem.setf(q + 0x30, y);
        fx.mem.write_u32(q + 0x10, 0x3FC0_0000);
        fx.mem.write_u32(q + 0x14, 0x3F80_0000);
        fx.mem.write_u32(q + 0x18, if s2 == 1 { 0x40A0_0000 } else { 0x4080_0000 });
        let t = fx.mem.read_u32(b + 0x1C);
        fx.mem.write_u32(q + 0x24, t);
        r.s6 = u64::from(q + 0x24);
        if fx.mem.read_u32(D + 0x30) != 0x8000 {
            let f = ee::mul(ee::from_i32(fx.mem.i32(q + 0x24)), -1.0);
            fx.fix_residue(f, 0x01E0_6154);
            fx.mem.set32(q + 0x24, ee::fixsfsi(f));
        }
    }
    let i = first_free(fx, FLASH, 0x4C, 0x1E);
    if i >= 0 {
        let a = FLASH + 0x4C * i as u32;
        set_counter4(fx, a, 0x12, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0x80), (7, 9)] {
            fx.mem.write_u8(a + o, v);
        }
        fx.mem.write_u32(a + 0x10, 0x4348_0000);
        fx.mem.write_u32(a + 0x14, 0x41C8_0000);
        for (o, v) in [(0xCu32, 0xF0u8), (0xD, 0xF0), (0xE, 0xF0), (0xA, 0x80), (9, 0x20), (8, 0x60), (0xF, 0xC0)] {
            fx.mem.write_u8(a + o, v);
        }
        for o in [0x44u32, 0x40, 0x3C] {
            fx.mem.write_u32(a + o, 0);
        }
        copy_v4(fx, sp + 0xB0, a + 0x1C);
        fx.mem.write_u32(a + 0x18, 0x4170_0000);
    }
    let i = first_free(fx, SPIKES, 0x68, 0x1E);
    if i >= 0 {
        let q = SPIKES + 0x68 * i as u32;
        set_counter4(fx, q, 0x10, 0, 0xFF);
        fx.mem.write_u8(q + 4, 5);
        fx.mem.write_u32(q + 0x14, 0x40A0_0000);
        fx.mem.write_u32(q + 0x1C, 0x4220_0000);
        for (o, v) in [(5u32, 0u8), (0xA, 0), (0xB, 2), (0xC, 3), (8, 0x28)] {
            fx.mem.write_u8(q + o, v);
        }
        let x = rand64(fx, r.s0, 0x01E0_62B8);
        fx.mem.set8(q + 9, x >> 4);
        fx.mem.write_u32(q + 0x10, BYTES);
        fx.mem.write_u32(q + 0x18, 0x3F80_0000);
        for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0), (0x27, 0x80)] {
            fx.mem.write_u8(q + o, v);
        }
        copy_v4(fx, sp + 0xB0, q + 0x28);
        fx.mem.write_u32(q + 0x58, 0x40);
        fx.mem.write_u32(q + 0x5C, 0x80);
        fx.mem.write_u32(q + 0x60, 0x100);
    }
    for _ in 0..0x19 {
        let i = first_free(fx, DROPS, 0x44, 0x190);
        r.s6 = sx(i);
        if i < 0 {
            continue;
        }
        let delay = (rand64(fx, r.s0, 0x01E0_635C) >> 4) & 7;
        let q = DROPS + 0x44 * i as u32;
        r.s6 = u64::from(D + 0x44 * i as u32);
        set_counter4(fx, q, 0x20, delay, 0xFF);
        for (o, v) in [(4u32, 0u8), (8, 0), (0xB, 2), (0xA, 0), (9, 3), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)] {
            fx.mem.write_u8(q + o, v);
        }
        let f = ee::add(90.0, spread(fx, r.s0, 0x01E0_63B8, 0x1E, 0xBB8));
        fx.mem.setf(q + 0x10, f);
        fx.mem.write_u32(q + 0x14, 0xBECC_CCCD);
        fx.mem.write_u32(q + 0x18, 0xBD75_C28F);
        fx.mem.write_u32(q + 0x1C, 0);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0xB0 + 4 * c);
            fx.mem.write_u32(q + 0x20 + 4 * c, w);
        }
        let f = jit(fx, r.s0, 0x01E0_6448, 0xD2, 0x69);
        fx.mem.setf(q + 0x2C, f);
        let f = jit(fx, r.s0, 0x01E0_6484, 0xD2, 0x69);
        fx.mem.setf(q + 0x30, ee::add(f32::from_bits(0x3F4C_CCCD), f));
        let f = jit(fx, r.s0, 0x01E0_64CC, 0xD2, 0x69);
        fx.mem.setf(q + 0x34, f);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let mut r = Regs { s0: sx(param), s1: 0, s6: 0, a0_hi: 0 };
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0xA0, r.s0, 0x01E0_50B0);
    match fx.mem.u16(D) {
        0x140 => return 8,
        0xA5 => {
            set_counter4(fx, D + 0x40, 0x6A, 0, 0xFF);
            fx.mem.write_u8(D + 0x45, 0);
            fx.mem.write_u8(D + 0x44, 8);
            fx.mem.write_u32(D + 0x54, 0x3FE6_6666);
            fx.mem.write_u32(D + 0x58, 0x3F80_0000);
            fx.mem.write_u32(D + 0x5C, 0x3F99_999A);
            fx.mem.write_u32(D + 0x70, 0);
            fx.mem.write_u32(D + 0x74, 0x42A0_0000);
            fx.mem.write_u32(D + 0x90, 0x4080_0000);
            copy_v4(fx, sp + 0xA0, D + 0x60);
        }
        0x69 => strike(fx, sp, &mut r),
        0x50 => charge(fx, &r),
        2 => {
            let h = fx.mem.i32(D + 4);
            let s = fx.mem.i32(D + 0xC);
            node_position_saving(fx, h, s, D + 0xB704, r.s0, Some(r.s1), 0x01E0_510C);
            let h = fx.mem.i32(D + 4);
            let s = fx.mem.i32(D + 0x10);
            node_position_saving(fx, h, s, sp + 0xB0, r.s0, Some(r.s1), 0x01E0_511C);
            let y = fx.mem.read_u32(sp + 0xB4);
            fx.mem.write_u32(D + 0xB708, y);
        }
        _ => {}
    }
    if fx.mem.u8(STREAKS) > 0 {
        let k = ((fx.mem.u16(D) - 0x10) & 0xF) as u32;
        let v = fx.mem.read_u8(sp + 0x110 + 4 * k);
        fx.mem.write_u8(STREAKS + 0x13, v.wrapping_mul(5));
    }
    let n = fx.mem.u16(D);
    if (0x14..0x11C).contains(&n) {
        fountains(fx, sp, &mut r);
    }
    let n = fx.mem.u16(D);
    if (0x69..0xEC).contains(&n) && n & 3 == 0 {
        bolt(fx, sp, &mut r);
    }
    for s7 in 0..0x1Eu32 {
        let b = BOLTS + 0x54 * s7;
        let c = fx.mem.u8(b);
        if c <= 0 {
            continue;
        }
        let k = fx.mem.u8(b + 2) - c;
        let p = POINTS + 240 * s7 + 12 * k as u32;
        for c in 0..3u32 {
            let w = fx.mem.read_u32(p + 4 * c);
            fx.mem.write_u32(b + 0x24 + 4 * c, w);
        }
        if k != 0x13 {
            for c in 0..3u32 {
                let v = ee::sub(fx.mem.f(p + 0xC + 4 * c), fx.mem.f(b + 0x24 + 4 * c));
                fx.mem.setf(sp + 0x1D0 + 4 * c, v);
            }
            aim_saving(fx, sp + 0x1D0, b + 0x44);
            continue;
        }
        burst(fx, sp, s7, &mut r);
    }
    let due = fx.mem.read_u8(D + 0x40) != 0 && fx.mem.read_u8(D + 0x41) == 0;
    shake_saving(fx, D + 0x40, r.s0, 0x01E0_652C, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    if due {
        // camera_shake's s0 (`move128` of a0) saved by 0x101BA0 / 0x1F7E20
        // with a0's upper half: the arches' first two Q words [run].
        fx.mem.write_u64(sp - 0x48, r.a0_hi);
        fx.mem.write_u64(sp - 0x38, r.a0_hi);
    }
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_6538, [Some(r.s0), Some(r.s1)]);
    blib::fade::fade(fx, FADE, OT);
    // fade's 0xF0 frame saves the entry's s6 at +0x70 and ra at +0x80.
    fx.saved(sp - 0xF0, &[(0x70, r.s6), (0x80, 0x01E0_6544)]);
    blib::star::flash_b(fx, 0x01E0_03A0, FLASH, OT, 0x1E);
    blib::spike::spikes(fx, 0x01E0_2470, SPIKES, OT, 0x1E);
    blib::drop::drops(fx, DROPS, OT, 0x190);
    blib::arch::arches_b(fx, 0x01E0_0E10, BOLTS, OT, 0x1E);
    blib::panel::panels(fx, 0x01E0_3D80, PANELS, OT, 0x3C);
    blib::radiate::streaks_b(fx, 0x01E0_1670, STREAKS, OT, 1);
    blib::beam::beads_at(fx, 0x01E0_4290, 0x34, 0x3C, BEADS, OT, 8);
    let n = fx.mem.u16(D);
    if (0x81..0x10C).contains(&n) {
        6
    } else if n == 0x10C {
        2
    } else if n == 0x77 {
        1
    } else {
        0
    }
}

