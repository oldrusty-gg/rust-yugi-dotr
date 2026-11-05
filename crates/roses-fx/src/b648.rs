//! BMODULE b648 (card 203), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E042F0, data area 0x1ED0000) [F] (helper D11, 31
//! Oct 2026): b039's program with a frame flash: two drifting ribbons
//! (`blib::driftribbon`, +0x70 / +0x10C0) fed from update 43 from the
//! attacker's node 4 at x = 3 sin of an angle advancing 0x800 (+0x2B74 /
//! +0x2B78) and drawn from 43 (alpha up by 6 to 0x60, down from 380); 64
//! flat sprites (+0x2260, `flatsprite::sprites`, 8 frames) falling 3 an
//! update round node 0x18 with the bob amplitude +0x22B4 swinging -30..30
//! (bit 0x1F of +0x2B64; the record's 0x1E04F30 `fans`); 32 puffs
//! (+0x2730, `Fx::sprites`, texture 1, half size 20 growing 10) thrown out
//! 14 in random directions (0x1043F0) from update 88, slowing x 1.03 and
//! fading by 4, respawned at age 16; a particle cloud (+0x2A00) on 43..74;
//! every update a frame flash (`blib::flash::frame_flash`, +0x2B50: 0x80,
//! 0x60, 0x10) whose alpha is 0x40 + 64 sin of an angle advancing 0x800
//! from 0xC000 on 43..170, then up by 4 to 0x80, down from 380; the stage
//! light untouched. Returns 1 on update 78, 6 on 88..379, 2 on 380, 8
//! after 396.
//!
//! Library (record addresses): `colours4` 0x1E00080, `flash::frame_flash`
//! 0x1E00120 (3b4ae97aa05c), `Fx::sprites` 0x1E00370, `flatsprite::sprites`
//! 0x1E01C60, `particle_quads` 0x1E02C10, `driftribbon::push` 0x1E037A0,
//! `driftribbon::draw` 0x1E03910; the executable's 0x1F6A60
//! [`crate::b107::target_radius`], 0x103660 [`crate::b079::sub3`].
use crate::b079::sub3;
use crate::b107::target_radius;
use crate::b142::fstep_up;
use crate::b436::fstep_down;
use crate::blib::{driftribbon, flash, flatsprite};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const N: u32 = D + 0x2B5C;
const LEFT: u32 = D + 0x2B58;
const BITS: u32 = D + 0x2B64;
const LIGHT: u32 = D + 0x2B80;
const FLASH: u32 = D + 0x2B50;
const RIB_A: u32 = D + 0x70;
const RIB_B: u32 = D + 0x10C0;
const SPRITES: u32 = D + 0x2260;
const PUFFS: u32 = D + 0x2730;
const CLOUD: u32 = D + 0x2A00;
const END: i32 = 0x17C;

/// The entry 0x1E042F0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x90);
    ret
}

fn halves(fx: &mut Fx, at: u32, v: &[(u32, i32)]) {
    for &(k, x) in v {
        fx.mem.set16(at + k, x);
    }
}

/// A speed (+0 / +8 of the particle at `p`) from the 16-bit angle `a`:
/// `r` cos (0x20ABF0) into +8, `r` sin (0x20ACC0) into +0.
fn fan_dir(fx: &mut Fx, p: u32, a: i32, r: f32) {
    let c = fx.table_trig(a, true);
    fx.mem.setf(p + 8, ee::mul(r, c));
    let s = fx.table_trig(a, false);
    fx.mem.setf(p, ee::mul(r, s));
}

fn rand16(fx: &mut Fx, s0: u32, ra: u32) -> i32 {
    fx.rand_from(s0, ra) & 0xFFFF
}

fn rem(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let r = fx.rand_from(s0, ra);
    fx.div(r, m).1
}

/// A puff thrown: a random direction of length 14 (0x1043F0 into sp +
/// 0x70) as its speed; 0x1043F0's prologue (0x80: s0 = D +0, `ra` +0x10)
/// left below the stack pointer (the upper half of its ra slot is the flat
/// sprites' missing-texture TEX0's upper word) [run].
fn throw(fx: &mut Fx, sp: u32, p: u32, ra: u32) {
    fx.random_dir(sp + 0x70, 14.0);
    fx.saved(sp - 0x80, &[(0, u64::from(D)), (0x10, u64::from(ra))]);
    for c in [0u32, 4, 8] {
        let v = fx.mem.read_u32(sp + 0x70 + c);
        fx.mem.write_u32(p + 0x2790 + c, v);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2B90);
    for x in [0u32, 0x2110, 0x2180, 0x21F0, 0x26C0, 0x2990] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x2B68u32, 0x18, 1), (0x2B6C, 0x1A, 0), (0x2B70, 4, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(LEFT, 0x18C);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x2B60, param);
    let r = target_radius(fx);
    fx.mem.setf(D + 0x2B7C, r);
    fx.mem.write_u8(LIGHT, 0x80);
    fx.mem.write_u32(BITS, 0);
    fx.mem.write_u32(D + 0x2B74, 0x4000);
    for (o, v) in [(2u32, 0x10u8), (1, 0x60), (0, 0x80), (3, 0)] {
        fx.mem.write_u8(FLASH + o, v);
    }
    fx.mem.write_u32(D + 0x2B54, 0xC000);
    fx.texture4(sp + 0x80, 0xF);
    // The cloud (+0x2A00): 16 quads of size 5, TEXEFF 15.
    let g = CLOUD;
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x0008_0083);
    for k in [0x10u32, 0x14, 0x18, 0x1C] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x28, 0x80), (0x2A, 0xC0), (0x24, 0x20), (0x26, 0x20), (0x2C, 0x80), (0x2E, 0x40)]);
    fx.mem.write_u32(g + 0xC, 0x40A0_0000);
    halves(fx, g, &[(0x130, 0x10), (0x13A, 2), (0x13C, 1), (0x140, 8), (0x136, 0), (0x138, 0)]);
    for (k, v) in [(0xAu32, 0x70u8), (9, 0xA0), (8, 0xA0), (0xB, 0x20)] {
        fx.mem.write_u8(g + k, v);
    }
    for k in 0..0x10i32 {
        let p = g + 16 * k as u32;
        for (c, ra) in [(0u32, 0x01E0_44A8u32), (1, 0x01E0_44DC)] {
            let r = fx.rand_from(D, ra);
            let base = fx.mem.f(g + 0x10 + 4 * c);
            let h = fx.div(r, 6).1;
            fx.mem.setf(p + 0x30 + 4 * c, ee::add(base, ee::from_i32(h - 3)));
        }
        let z = fx.mem.read_u32(g + 0x18);
        fx.mem.write_u32(p + 0x38, z);
        let a = fx.mem.i16(g + 0x13A).wrapping_neg();
        let t = fx.mult(a, k);
        fx.mem.set16(p + 0x3E, t);
        fx.mem.set16(p + 0x3C, 0);
        let n = fx.mem.i16(g + 0x140) - 1;
        fx.mem.set16(g + 0x13E, n);
    }
    // The puffs (+0x2730): 32 sprites of the attacker's texture 1 (0x60,
    // 0x10, 0x80, alpha 0x40), flags 0x48000003, half size 20 growing 10
    // an age, cell (0, 0, 0x18, 0x20) of 0x80 x 0x40, 8 frames every 16
    // updates; thrown 14 in random directions, age -(rand % 12).
    fx.texture4(sp + 0x80, 1);
    let g = PUFFS;
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x4800_0003);
    for (k, v) in [(0xAu32, 0x60u8), (9, 0x10), (8, 0x80), (0xB, 0x40)] {
        fx.mem.write_u8(g + k, v);
    }
    fx.mem.write_u32(g + 0xC, 0x41A0_0000);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x30, 0), (0x32, 0), (0x34, 0x18), (0x36, 0x20), (0x38, 0), (0x3A, 0), (0x3C, 0x80), (0x3E, 0x40)]);
    fx.mem.write_u32(g + 0x40, D + 0x2790);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4170_0000);
    fx.mem.write_u32(g + 0x50, 0x4120_0000);
    halves(fx, g, &[(0x5A, 0x20), (0x5C, 8), (0x5E, 0x10)]);
    let mut k = 0;
    while k < fx.mem.i16(g + 0x5A) {
        let p = D + 16 * k as u32;
        throw(fx, sp, p, 0x01E0_460C);
        let h = rem(fx, D, 0x01E0_4628, 0xC);
        fx.mem.set16(p + 0x279E, h.wrapping_neg());
        fx.mem.write_u8(p + 0x279C, 0x40);
        let f = fx.rand_from(D, 0x01E0_4650).wrapping_rem(8);
        fx.mem.set8(p + 0x279D, f);
        k += 1;
    }
    // The flat sprites (+0x2260): 64 of the attacker's texture 0, flags
    // 0x58000003, half size 1.5 growing 5 an age, bobbing 50 at 0x400,
    // cell (0, 0x30, 0x20, 0x20) of a 0x100 x 0x80 sheet, 8 frames every
    // 100 updates.
    fx.texture5(sp + 0x80, 0);
    let g = SPRITES;
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x5800_0003);
    for k in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(g + k, 0x80);
    }
    fx.mem.write_u32(g + 0xC, 0x3FC0_0000);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x30, 0), (0x32, 0x30), (0x34, 0x20), (0x36, 0x20), (0x38, 0), (0x3A, 0x30), (0x3C, 0x100), (0x3E, 0x100)]);
    fx.mem.write_u32(g + 0x40, D + 0x22C0);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4000_0000);
    fx.mem.write_u32(g + 0x50, 0x40A0_0000);
    fx.mem.write_u32(g + 0x54, 0x4248_0000);
    halves(fx, g, &[(0x58, 0x400), (0x5A, 0x40), (0x5C, 8), (0x5E, 0x64)]);
    let mut k = 0;
    while k < fx.mem.i16(g + 0x5A) {
        let p = D + 16 * k as u32;
        let a = rand16(fx, D, 0x01E0_4750);
        fx.mem.write_u32(p + 0x22C4, 0);
        let r = fx.mem.f(g + 0x4C);
        fan_dir(fx, p + 0x22C0, a, r);
        fx.mem.set16(p + 0x22CE, k.wrapping_neg());
        fx.mem.write_u8(p + 0x22CC, 0);
        let f = fx.rand_from(D, 0x01E0_47A0).wrapping_rem(8);
        fx.mem.set8(p + 0x22CD, f);
        k += 1;
    }
    // The two ribbons (+0x70, +0x10C0): 256 slots, width 150 growing 10
    // an age, x scaled 1.03, cell (0x10, 0, 0x20, 0x30), fade 8, life 64.
    for g in [RIB_A, RIB_B] {
        let tex = fx.mem.read_u32(sp + 0x80);
        fx.mem.write_u32(g, tex);
        fx.mem.write_u32(g + 8, 0x8003);
        for (o, v) in [(6u32, 0x80u8), (5, 0x80), (4, 0x80), (7, 0)] {
            fx.mem.write_u8(g + o, v);
        }
        halves(fx, g, &[(0x10, 0x10), (0x12, 0), (0x14, 0x20), (0x16, 0x30), (0x18, 0x10), (0x1A, 0), (0x1C, 0x20), (0x1E, 0x30)]);
        fx.mem.write_u32(g + 0xC, 0x4316_0000);
        fx.mem.write_u32(g + 0x34, 0x4120_0000);
        fx.mem.write_u32(g + 0x30, 0x3F83_D70A);
        halves(fx, g, &[(0x3A, 8), (0x3C, 0), (0x3E, 0x40), (0x40, 0x100), (0x42, 0), (0x44, 1)]);
    }
    0
}

/// The record's 0x1E04F30(D, ot) [F] (7f201f928502, b648 only): the bob
/// swing (bit 0x1F), the flat sprites drawn and stepped: each live
/// sprite's height +4 -= 3, its speed x 1.05 while the height is at most
/// 0, else x 0.8; below -200 a respawn (random angle, age -k, height 0,
/// alpha 0x40 from update 43, else 0); the group's alpha fading by 16
/// from 381.
fn fans(fx: &mut Fx) {
    fx.enter(0x50);
    if bit(fx.mem.read_u32(BITS), 0x1F) {
        if fstep_down(fx, D + 0x22B4, -30.0, 1.5) == 0 {
            let w = fx.mem.read_u32(BITS) & 0x7FFF_FFFF;
            fx.mem.write_u32(BITS, w);
        }
    } else if fstep_up(fx, D + 0x22B4, 30.0, 1.5) == 0 {
        let w = fx.mem.read_u32(BITS) | 0x8000_0000;
        fx.mem.write_u32(BITS, w);
    }
    flatsprite::sprites(fx, OT, SPRITES, D + 0x21F0);
    let mut k = 0;
    while k < fx.mem.i16(SPRITES + 0x5A) {
        let p = D + 16 * k as u32;
        let s0 = k as u32;
        k += 1;
        if fx.mem.i16(p + 0x22CE) < 0 {
            continue;
        }
        let d = ee::sub(fx.mem.f(p + 0x22C4), 3.0);
        fx.mem.setf(p + 0x22C4, d);
        let s = if ee_le(d, 0.0) { f32::from_bits(0x3F86_6666) } else { f32::from_bits(0x3F4C_CCCD) };
        for o in [0x22C8u32, 0x22C0] {
            let v = ee::mul(fx.mem.f(p + o), s);
            fx.mem.setf(p + o, v);
        }
        if ee_lt(fx.mem.f(p + 0x22C4), -200.0) {
            let a = rand16(fx, s0, 0x01E0_5098);
            fx.mem.set16(p + 0x22CE, (s0 as i32).wrapping_neg());
            let r = fx.mem.f(SPRITES + 0x4C);
            fan_dir(fx, p + 0x22C0, a, r);
            fx.mem.write_u32(p + 0x22C4, 0);
            let a = if fx.mem.i32(N) >= 0x2B { 0x40 } else { 0 };
            fx.mem.write_u8(p + 0x22CC, a);
        }
    }
    if fx.mem.i32(N) > END {
        fx.step_down(SPRITES + 0xB, 0, 0x10);
    }
    fx.leave(0x50);
}

/// A ribbon's new point: the target's point (0x1F65D0 kind 1) 20 up (or
/// down), less the attacker's node 0x18 (`node`), over 80 into `v`; then
/// from update 43 the node 4 into `p` with x = 3 sin(`angle`), pushed on
/// `g` / `pts`, the angle advanced 0x800.
fn feed(fx: &mut Fx, v: u32, up: bool, node: u32, g: u32, pts: u32, p: u32, angle: u32, ra: u32) {
    let t = fx.target_handle();
    fx.unit_point(1, t, v);
    let y = if up { ee::add(fx.mem.f(v + 4), 20.0) } else { ee::sub(fx.mem.f(v + 4), 20.0) };
    fx.mem.setf(v + 4, y);
    sub3(fx, v, node);
    for c in [0u32, 4, 8] {
        let q = ee::div(fx.mem.f(v + c), 80.0);
        fx.mem.setf(v + c, q);
    }
    if fx.mem.i32(N) < 0x2B {
        return;
    }
    let h = fx.self_handle();
    let s = fx.mem.i32(D + 0x2B70);
    fx.node_position(h, s, p);
    let a = fx.mem.i32(angle);
    let s = fx.table_trig(a, false);
    fx.mem.setf(p, ee::mul(3.0, s));
    driftribbon::push(fx, g, pts, p);
    // The push's prologue (0x60: s0 = D, s1 = the OT, s2 = sp + 0x60, s3 /
    // s4 the runner's 0, ra +0x50).
    let f = fx.sp - 0x60;
    fx.saved(f, &[(0, u64::from(D)), (0x10, u64::from(OT)), (0x20, u64::from(node)), (0x30, 0), (0x40, 0), (0x50, u64::from(ra))]);
    let a = fx.mem.i32(angle).wrapping_add(0x800);
    fx.mem.set32(angle, a);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let node = |fx: &mut Fx, slot: u32, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(D + slot);
        fx.node_position(h, s, out);
    };
    node(fx, 0x2B70, D + 0x29B0);
    node(fx, 0x2B68, D + 0x2210);
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0x50);
    node(fx, 0x2B68, sp + 0x60);
    fx.look_angles(sp + 0x60, sp + 0x50, D + 0x2200);
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x26E0);
    feed(fx, D + 0x90, false, sp + 0x60, RIB_A, D + 0xC0, D + 0x2130, D + 0x2B74, 0x01E0_4A0C);
    feed(fx, D + 0x10E0, true, sp + 0x60, RIB_B, D + 0x1110, D + 0x21A0, D + 0x2B78, 0x01E0_4AD0);
    // s0 = D, s1 = the OT, s2 = sp + 0x60, s3 the runner's (0).
    let regs = [Some(D), Some(OT), Some(sp + 0x60), Some(0)];
    fx.xform_world_saving(D, 0x01E0_4AE8, regs);
    fx.xform_billboard_saving(D + 0x2110, 0x01E0_4AF0, regs);
    fx.xform_billboard_saving(D + 0x2180, 0x01E0_4AF8, regs);
    fx.xform_billboard_saving(D + 0x2990, 0x01E0_4B00, regs);
    fx.xform_world_saving(D + 0x21F0, 0x01E0_4B08, regs);
    fx.xform_world_saving(D + 0x26C0, 0x01E0_4B10, regs);
    // The flash's first TEXFLUSH is reported with a1..a3 as the last call
    // left them: 0x1038D0(D + 0x26C0)'s (its TRS matrix at sp - 0x40, x +
    // 0x10, x + 0x20) below update 43, then the alpha steps' [run].
    let n = fx.mem.i32(N);
    let xf = [sp - 0x40, D + 0x26D0, D + 0x26E0];
    let a123 = if n < 0x2B {
        xf
    } else if n < 0xAB {
        let a = fx.mem.i32(D + 0x2B54).wrapping_add(0x800);
        fx.mem.set32(D + 0x2B54, a);
        let s = fx.table_trig(a & 0xFFFF, false);
        let f = ee::mul(64.0, s);
        let v = ee::fixsfsi(f);
        // 0x1A2B80's frame: its ra at sp - 0x10 is the flash's textured
        // quad's TEX1 residue (0x207FC0 keeps bits 10..18, 21..31).
        fx.fix_residue(f, 0x01E0_4B68);
        fx.mem.write_u8(FLASH + 3, ((v & 0xFF) + 0x40) as u8);
        xf
    } else if n < END {
        // 0x1032C0 / 0x103470 leave a3 = the byte before the step.
        let before = u32::from(fx.mem.read_u8(FLASH + 3));
        fx.step_up(FLASH + 3, 0x80, 4);
        [0x80, 4, before]
    } else {
        let before = u32::from(fx.mem.read_u8(FLASH + 3));
        fx.step_down(FLASH + 3, 0, 4);
        [0, 4, before]
    };
    flash::frame_flash(fx, FLASH, a123);
    for (o, ra) in [(0xAu32, 0x01E0_4BD8u32), (9, 0x01E0_4C00), (8, 0x01E0_4C28)] {
        let v = (fx.rand_from(D, ra).wrapping_rem(0x80) + 0x7F) as u8;
        fx.mem.write_u8(SPRITES + o, v);
        for g in [RIB_B, RIB_A] {
            fx.mem.write_u8(g + o - 4, v);
        }
    }
    if fx.mem.i32(N) >= 0x58 {
        fx.sprites(PUFFS, D + 0x26C0);
        if fx.mem.i32(N) >= END {
            fx.step_down(PUFFS + 0xB, 0, 2);
        } else {
            fx.step_up(PUFFS + 0xB, 0x40, 1);
        }
        let mut k = 0;
        while k < fx.mem.i16(PUFFS + 0x5A) {
            let p = D + 16 * k as u32;
            k += 1;
            if fx.mem.i16(p + 0x279E) >= 0 {
                let s = f32::from_bits(0x3F83_D70A);
                for o in [0x2790u32, 0x2794, 0x2798] {
                    let v = ee::mul(fx.mem.f(p + o), s);
                    fx.mem.setf(p + o, v);
                }
                fx.step_down(p + 0x279C, 0, 4);
            }
            if fx.mem.i16(p + 0x279E) < 0x10 {
                continue;
            }
            throw(fx, sp, p, 0x01E0_4D20);
            fx.mem.set16(p + 0x279E, 0);
            fx.mem.write_u8(p + 0x279C, 0x40);
        }
    }
    let n = fx.mem.i32(N);
    if (0x2B..0x4B).contains(&n) {
        for k in [0x78u32, 0x74, 0x70] {
            fx.mem.write_u32(sp + k, 0x3F81_47AE);
        }
        fx.particle_quads(D + 0x2990, CLOUD, sp + 0x70);
        if fx.mem.i32(N) >= 0x3B {
            fx.step_down(CLOUD + 0xB, 0, 2);
        }
    }
    if fx.mem.i32(N) >= 0x2B {
        for (g, pts) in [(RIB_A, D + 0xC0), (RIB_B, D + 0x1110)] {
            driftribbon::draw(fx, OT, g, pts, D);
            if fx.mem.i32(N) >= END {
                fx.step_down(g + 7, 0, 6);
            } else {
                fx.step_up(g + 7, 0x60, 6);
            }
        }
    }
    fans(fx);
    let n = fx.mem.i32(N) + 1;
    fx.mem.set32(N, n);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if n == END {
        let t = fx.target_handle();
        fx.unit_add_colour(t, 0);
        2
    } else if n == 0x4E {
        1
    } else if (0x58..END).contains(&n) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
