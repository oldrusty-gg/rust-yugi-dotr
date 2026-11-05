//! BMODULE b712 (card 14), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02040, data area a0 = 0x1ED0000) [F] (helper A20,
//! 1 Nov 2026): s4 = D, s1 = the parameter then the loop counts, s0 = the
//! OT. On 0x42 the frame rays (+0xA8, 0x16 counts) on node +0x10, on 0x124
//! again (0x34 counts) at the target; on 0x70..0x138 two sprites an update
//! at the attacker's point (+0xD8, the local 0x1E00FD0) and an object op
//! 0x1F83B0 (mode 1, alpha by a 16-step table); on 0x8C..0x11C three swept
//! arcs an update (+0x1758, `sweep::sweeps`) between nodes +0xC / +0x10;
//! on 0x124..0x14C two arcs at the target, on 0x124..0x16C seven sprites
//! an update round it. The stage light by 0x1031C0 (+0xA0: down on 0x55,
//! up on 0x18A). Returns 1 on 0x11A, 6 on 0x124..0x154, 3 on 0x155, 8 on
//! 0x190 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `frame_rays_c`
//! 0x1E00440, `sweeps` 0x1E01590 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{atlas_cell, camera_vp_into, countdown, first_free, set_counter4, tex_rect}, rel};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const RAYS: u32 = D + 0xA8;
const SPRITES: u32 = D + 0xD8;
const SWEEPS: u32 = D + 0x1758;
const BYTES: u32 = D + 0x2F18;
/// 0x3FAA3D71: the screen aspect.
const ASPECT: u32 = 0x3FAA_3D71;

/// The entry 0x1E02040 (a0 = D, a1 = param).
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
    fx.bzero(D, 0x3020);
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
    for (o, node, clear) in [(0xCu32, 1, 1), (0x10, 0x58, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + o, r);
    }
    fx.mem.set16(D + 0xA0, 0x55);
    fx.mem.set16(D + 0xA2, 0x18A);
    // The entry's s0 = the OT, s1 = 2 (the texture loop's), s2 = the
    // runner's 0 [H, as b201].
    rand_bytes(fx, BYTES, 0x100, 0x01E0_215C, [Some(OT), Some(2), Some(0)]);
    0
}

thread_local! {
    /// The a3 the step's last executable call left (rand: its __muldi3's
    /// low word; 0x101C30: the record offset it stopped at; the object op:
    /// its argument) [F 0x101C30, run].
    static A3: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

fn set_a3(v: u32) {
    A3.with(|a| a.set(v));
}

/// 0x101C30 with the a3 it leaves.
fn free_slot(fx: &mut Fx, p: u32, stride: i32, n: i32) -> i32 {
    let i = first_free(fx, p, stride, n);
    set_a3((if i < 0 { n } else { i }).wrapping_mul(stride) as u32);
    i
}

/// rand() >> 4 from return address `ra` (s0 = the OT).
fn r4(fx: &mut Fx, ra: u32) -> i32 {
    let x = fx.rand_from(OT, ra) >> 4;
    set_a3((fx.rand_state() as u32).wrapping_sub(1));
    x
}

/// (rand() >> 4) % m - h, over 100.
fn spread(fx: &mut Fx, ra: u32, m: i32, h: i32) -> f32 {
    let x = r4(fx, ra);
    let (_, r) = fx.div(x, m);
    ee::div(ee::from_i32(r - h), 100.0)
}

/// The rays' bytes (both rays starts).
fn rays(fx: &mut Fx, alpha: u8, delay: u8) {
    for (o, v) in [(0xC4u32, 0x80u8), (0xC5, 0x80), (0xC6, 0x80), (0xC7, alpha), (0xC8, 0x80), (0xC9, 0x80), (0xCA, 0x80), (0xCB, 0x70), (0xB0, 0xC), (0xB1, delay), (0xB2, 0xFF)] {
        fx.mem.write_u8(D + o, v);
    }
}

fn sprite_common(fx: &mut Fx, r: u32) {
    fx.mem.write_u8(r + 4, 1);
    for (o, v) in [(8u32, 0u8), (9, 4), (0xA, 1), (0xB, 2)] {
        fx.mem.write_u8(r + o, v);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let mut s1 = param as i64 as u64;
    set_a3(0);
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(OT), 0x01E0_2184);
    match fx.mem.u16(D) {
        0x190 => return 8,
        0x124 => {
            set_counter4(fx, RAYS, 0x34, 0, 0xFF);
            copy_v4(fx, sp + 0x60, D + 0xB4);
            rays(fx, 0x60, 2);
        }
        0x42 => {
            set_counter4(fx, RAYS, 0x16, 0, 0xFF);
            let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x10));
            node_position_saving(fx, h, slot, D + 0xB4, u64::from(OT), Some(s1), 0x01E0_21EC);
            rays(fx, 0x40, 1);
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x70..0x139).contains(&n) {
        for _ in 0..2 {
            let i = free_slot(fx, SPRITES, 0x20, 0xB4);
            if i < 0 {
                continue;
            }
            let r = SPRITES + 0x20 * i as u32;
            let x = r4(fx, 0x01E0_2328);
            set_counter4(fx, r, ((x & 7) + 0x20) & 0xFF, 0, 0xFF);
            sprite_common(fx, r);
            fx.mem.write_u32(r + 0xC, 0x420C_0000);
            let q = spread(fx, 0x01E0_23B8, 0x28, 0x14);
            fx.mem.setf(r + 0x10, ee::add(f32::from_bits(0x3F99_999A), q));
            let h = fx.mem.i32(D + 4);
            fx.unit_point(1, h, sp + 0x70);
            for (k, (ra, m, h)) in [(0x01E0_242Cu32, 0x16A8, 0xB54), (0x01E0_2480, 0x4B0, 0x258), (0x01E0_24D4, 0x16A8, 0xB54)].into_iter().enumerate() {
                let q = spread(fx, ra, m, h);
                let v = ee::add(fx.mem.f(sp + 0x70 + 4 * k as u32), q);
                fx.mem.setf(r + 0x14 + 4 * k as u32, v);
            }
        }
        s1 = 2;
        for (o, v) in [(0x8Eu32, 0x80u8), (0x8A, 0x80), (0x8D, 0x40), (0x89, 0x40), (0x8C, 0x10), (0x88, 0x10)] {
            fx.mem.write_u8(sp + o, v);
        }
        const ALPHA: [u8; 16] = [0, 0x20, 0x40, 0x60, 0x80, 0xA0, 0xC0, 0xE0, 0xFF, 0xE0, 0xC0, 0xA0, 0x80, 0x60, 0x40, 0x20];
        let a = ALPHA[((n - 0x70) & 0xF) as usize];
        fx.mem.write_u8(sp + 0x8F, a);
        fx.mem.write_u8(sp + 0x8B, a);
        let h = fx.mem.read_u32(D + 4);
        fx.boundary(0x001F_83B0, "object op 0x1F83B0", [h, 1, 1, sp + 0x88]);
        set_a3(sp + 0x88);
    }
    let n = fx.mem.u16(D);
    if (0x8C..0x11D).contains(&n) {
        for s in 0..3 {
            let Some(r) = sweep_slot(fx, 0x01E0_26A8, 3, 0x14) else { continue };
            fx.mem.write_u8(r + 4, 3);
            fx.mem.write_u8(r + 8, 1);
            let x = r4(fx, 0x01E0_272C);
            fx.mem.set8(r + 9, x & 7);
            fx.mem.write_u8(r + 0xA, 2);
            let v = fx.mem.read_u8(D + 0xC);
            fx.mem.write_u8(r + 0xD, v);
            let v = fx.mem.read_u8(D + 0x10);
            fx.mem.write_u8(r + 0xE, v);
            fx.mem.write_u8(r + 0xF, 3);
            fx.mem.write_u32(r + 0x10, 0x3FB3_3333);
            sweep_turn(fx, r, 0x01E0_27FC);
            fx.mem.write_u32(r + 0x20, 0);
            fx.mem.write_u32(r + 0x24, 0);
            if s != 0 {
                let x = r4(fx, 0x01E0_28BC);
                fx.mem.set8(r + 0xB, (x & 3) + 3);
                let x = r4(fx, 0x01E0_28EC);
                fx.mem.set8(r + 0xC, (x & 3) + 3);
                fx.mem.write_u32(r + 0x14, 0x4150_0000);
                fx.mem.write_u32(r + 0x18, 0x4170_0000);
                fx.mem.write_u32(r + 0x28, 0x40C0_0000);
            } else {
                let x = r4(fx, 0x01E0_2984);
                fx.mem.set8(r + 0xB, (x & 3) + 4);
                fx.mem.write_u8(r + 0xC, 1);
                fx.mem.write_u32(r + 0x14, 0x4150_0000);
                fx.mem.write_u32(r + 0x18, 0x4100_0000);
                fx.mem.write_u32(r + 0x28, 0x4100_0000);
            }
        }
        s1 = 3;
    }
    let n = fx.mem.u16(D);
    if (0x124..0x14D).contains(&n) {
        for _ in 0..2 {
            let Some(r) = sweep_slot(fx, 0x01E0_2A9C, 3, 0x18) else { continue };
            fx.mem.write_u8(r + 4, 6);
            fx.mem.write_u8(r + 8, 1);
            let x = r4(fx, 0x01E0_2B20);
            fx.mem.set8(r + 9, x & 7);
            fx.mem.write_u8(r + 0xA, 2);
            fx.mem.write_u8(r + 0xF, 3);
            sweep_turn(fx, r, 0x01E0_2B8C);
            fx.mem.write_u32(r + 0x20, 0);
            fx.mem.write_u32(r + 0x24, 0x41F0_0000);
            let x = r4(fx, 0x01E0_2C48);
            fx.mem.set8(r + 0xB, (x & 3) + 5);
            let x = r4(fx, 0x01E0_2C78);
            fx.mem.set8(r + 0xC, (x & 3) + 3);
            fx.mem.write_u32(r + 0x14, 0x4180_0000);
            fx.mem.write_u32(r + 0x18, 0x4170_0000);
            fx.mem.write_u32(r + 0x28, 0x40A0_0000);
            for (o, f) in [(0x30u32, 0x60u32), (0x34, 0x64), (0x38, 0x68), (0x2C, 0x64)] {
                let v = fx.mem.read_u32(sp + f);
                fx.mem.write_u32(r + o, v);
            }
            fx.mem.write_u32(r + 0x3C, 0x8000);
            fx.mem.write_u32(r + 0x44, 0);
            fx.mem.write_u32(r + 0x40, 0);
        }
        s1 = 2;
    }
    let n = fx.mem.u16(D);
    if (0x124..0x16D).contains(&n) {
        for _ in 0..7 {
            let i = free_slot(fx, SPRITES, 0x20, 0xB4);
            if i < 0 {
                continue;
            }
            let r = SPRITES + 0x20 * i as u32;
            let x = r4(fx, 0x01E0_2E44);
            set_counter4(fx, r, ((x & 7) + 0x16) & 0xFF, 0, 0xFF);
            sprite_common(fx, r);
            let x = r4(fx, 0x01E0_2EC4);
            let (_, m) = fx.div(x, 10);
            let v = ee::div(ee::from_i32(m.wrapping_mul(200) - 1000), 100.0);
            fx.mem.setf(r + 0xC, ee::add(30.0, v));
            let x = r4(fx, 0x01E0_2F34);
            let mut m = x & 1;
            if x < 0 && m != 0 {
                m -= 2;
            }
            let v = ee::div(ee::from_i32(m.wrapping_mul(200) - 200), 100.0);
            fx.mem.setf(r + 0x10, ee::add(f32::from_bits(0x4059_999A), v));
            let half = ee::div(fx.mem.f(sp + 0x64), 2.0);
            fx.fix_residue(half, 0x01E0_2FC8);
            let s3 = ee::fixsfsi(half);
            let w = |fx: &mut Fx, ra: u32| {
                let x = r4(fx, ra);
                let (_, m) = fx.div(x, s3);
                ee::div(ee::from_i32(m.wrapping_mul(200).wrapping_sub(s3.wrapping_mul(200) >> 1)), 100.0)
            };
            let q = w(fx, 0x01E0_2FD4);
            let v = ee::add(fx.mem.f(sp + 0x60), q);
            fx.mem.setf(r + 0x14, v);
            let q = spread(fx, 0x01E0_3050, 0x4B0, 0x258);
            fx.mem.setf(r + 0x18, q);
            let q = w(fx, 0x01E0_309C);
            let v = ee::add(fx.mem.f(sp + 0x68), q);
            fx.mem.setf(r + 0x1C, v);
        }
        s1 = 7;
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_3134, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_3144, [Some(u64::from(OT)), Some(s1)]);
    // The rays' a3: what the last call left, 0 when none ran [H: the shake
    // is never armed here, the runner's / 0x1F71A0's a3 0].
    let a3 = A3.with(|a| a.get());
    blib::window::frame_rays_c(fx, 0x01E0_0440, RAYS, OT, 1, a3);
    sprites(fx, 0x01E0_0FD0, SPRITES, OT, 0xB4);
    blib::sweep::sweeps(fx, 0x01E0_1590, SWEEPS, OT, 0x50);
    let n = fx.mem.u16(D);
    if (0x124..0x155).contains(&n) {
        6
    } else if n == 0x155 {
        3
    } else if n == 0x11A {
        1
    } else {
        0
    }
}

/// A free arc slot (0x101C30 over 0x50 of 0x4C bytes) started with
/// (rand() >> 4) & `mask` + `base` counts.
fn sweep_slot(fx: &mut Fx, ra: u32, mask: i32, base: i32) -> Option<u32> {
    let i = free_slot(fx, SWEEPS, 0x4C, 0x50);
    if i < 0 {
        return None;
    }
    let r = SWEEPS + 0x4C * i as u32;
    let x = r4(fx, ra);
    set_counter4(fx, r, ((x & mask) + base) & 0xFF, 0, 0xFF);
    Some(r)
}

/// +0x1C = 180 + ((rand() >> 4) % 180 x 200 - 18000) / 100.
fn sweep_turn(fx: &mut Fx, r: u32, ra: u32) {
    let x = r4(fx, ra);
    let (_, m) = fx.div(x, 0xB4);
    let v = ee::div(ee::from_i32(m.wrapping_mul(200) - 0x4650), 100.0);
    fx.mem.setf(r + 0x1C, ee::add(180.0, v));
}

/// 8a77c4f3b6f7, the local 0x1E00FD0 (368 words) [F]: `sprites(recs, ot,
/// n) -> 1` steps and draws `n` records (0x20 bytes; a 0x170 frame, the
/// descriptor at sp + 0xF0: RGBAQ then UV / XYZ2 x 4). Per record:
/// countdown 0x101AF0; the texture (5, +8) with +4 1, else the 0x18-byte
/// spec table (record-relative `lui` / `addiu` at +0x94 / +0x98, the id at
/// +0xB0 / +0xB4) entry +8; y +0x18 += +0x10; with +4 1 colour 0x80 (cell
/// +9) in the first half of the count, else 0x60 / 0x10 / 0 (cell +0xA),
/// alpha count x 16 below 8 else 0x80, the atlas cell of layout 0 (blend
/// +0xB) by 0x1020B0 / 0x101D20; else 0xC0 / 0x80 / 0 then 0x30 / 8 / 0,
/// Q 1, alpha count x 31 below 8 else 0xF0, the table entry as the spec.
/// The point +0x14 projected (0x204D80) and on screen (0x101600), half
/// width +0xC / (w / 256) (x ASPECT in y), key 0x1A2C70(w).
fn sprites(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x170);
    let mut g = recs;
    for i in 0..n {
        if i != 0 {
            g += 0x20;
        }
        sprite(fx, at, sp, g, ot);
    }
    fx.leave(0x170);
    1
}

fn sprite(fx: &mut Fx, at: u32, sp: u32, g: u32, ot: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    let e = 0x18 * fx.mem.u8(g + 8) as u32;
    let table = rel(fx, at, 0x94, 0x98).wrapping_add(e);
    let t = if fx.mem.u8(g + 4) & 1 != 0 {
        let id = fx.mem.u8(g + 8);
        fx.texture(5, id)
    } else {
        let id = rel(fx, at, 0xB0, 0xB4).wrapping_add(e);
        let id = fx.mem.u8(id);
        let kind = fx.mem.i32(table);
        fx.texture(kind, id)
    };
    let v = ee::add(fx.mem.f(g + 0x18), fx.mem.f(g + 0x10));
    fx.mem.setf(g + 0x18, v);
    for k in 0..3u32 {
        let v = fx.mem.read_u32(g + 0x14 + 4 * k);
        fx.mem.write_u32(sp + 0xD0 + 4 * k, v);
    }
    camera_vp_into(fx, sp + 0x70);
    let d = sp + 0xF0;
    fx.desc_init(d, 9, 0x24, 0x4_2424_2421);
    let count = fx.mem.u8(g);
    let first = (fx.mem.u8(g + 2) >> 1) < count;
    if fx.mem.u8(g + 4) & 1 != 0 {
        let cell = if first {
            for o in [0x102u32, 0x101, 0x100] {
                fx.mem.write_u8(sp + o, 0x80);
            }
            fx.mem.u8(g + 9)
        } else {
            fx.mem.write_u8(sp + 0x100, 0x60);
            fx.mem.write_u8(sp + 0x101, 0x10);
            fx.mem.write_u8(sp + 0x102, 0);
            fx.mem.u8(g + 0xA)
        };
        let a = if fx.mem.u8(g) < 8 { fx.mem.u8(g) << 4 } else { 0x80 };
        fx.mem.set8(sp + 0x103, a);
        let v = fx.mem.read_u8(g + 0xB);
        fx.mem.write_u8(sp + 0xB5, v);
        atlas_cell(fx, sp + 0xB0, cell, 0);
        tex_rect(fx, d, sp + 0xB0, t);
    } else {
        fx.mem.write_u8(sp + 0x102, 0);
        if first {
            fx.mem.write_u8(sp + 0x100, 0xC0);
            fx.mem.write_u8(sp + 0x101, 0x80);
        } else {
            fx.mem.write_u8(sp + 0x100, 0x30);
            fx.mem.write_u8(sp + 0x101, 8);
        }
        fx.mem.write_u32(sp + 0x104, 0x3F80_0000);
        let a = if fx.mem.u8(g) < 8 { fx.mem.u8(g) * 31 } else { 0xF0 };
        fx.mem.set8(sp + 0x103, a);
        tex_rect(fx, d, table, t);
    }
    let m = fx.read_m4(sp + 0x70);
    if !fx.project_at(sp + 0xD0, &m, sp + 0xE8, sp + 0x16C) {
        return;
    }
    let xyz = fx.mem.read_u64(sp + 0xE8);
    if !Fx::on_screen(xyz) {
        return;
    }
    let half = ee::div(fx.mem.f(g + 0xC), ee::div(fx.mem.f(sp + 0x16C), 256.0));
    let z = fx.xyz_z(sp + 0xE8);
    for o in [0x140u32, 0x130, 0x120, 0x110] {
        fx.set_xyz_z(sp + o, z);
    }
    let x = ee::from_i32(fx.mem.u16(sp + 0xE8));
    let y = ee::from_i32(fx.mem.u16(sp + 0xEA));
    let hy = ee::mul(f32::from_bits(ASPECT), half);
    for (v, ra, a, b) in [(ee::sub(x, half), 0x438u32, 0x130u32, 0x110u32), (ee::add(x, half), 0x488, 0x140, 0x120), (ee::sub(y, hy), 0x4EC, 0x122, 0x112), (ee::add(y, hy), 0x550, 0x142, 0x132)] {
        fx.fix_residue(v, at + ra);
        let k = ee::fixunssfsi(v) as u16;
        fx.mem.write_u16(sp + a, k);
        fx.mem.write_u16(sp + b, k);
    }
    let w = fx.mem.f(sp + 0x16C);
    fx.fix_residue(w, at + 0x564);
    let key = ee::fixunssfsi(w) as i32;
    fx.submit(d, ot, key);
}
