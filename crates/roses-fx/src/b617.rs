//! BMODULE b617 (card 200), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E061D0, data area 0x1ED0000, 0xFB0 bytes) [F]
//!: b229's event program (s0 = D, s1 = the OT; a
//! u16 update count at D+0, records armed on fixed updates by 0x101AD0).
//! Handles D+4 / D+8, nodes 0x37 (clear) / 0x36 as words D+0xC / D+0x10,
//! the self's rotation D+0x2C, the light ramp D+0xA0, jitter bytes D+0xEB0.
//!
//! On 0x1F sixteen sprite columns (`columns`, 0x1E003A0, D+0xA4, 0x28
//! apart: a state machine rising (1), a four-cell glow (2), split and
//! sliding apart (3)) about the target, offset by the table at 0x1E06E80;
//! 0x94 the arcs (D+0x834); 0x9F four arches (D+0x6E4) and the frame rays
//! (D+0xB18); 0xC8 the columns split, screen bars (`bars`, 0x1E04AF0,
//! D+0xB70) and two screen flashes (`flash`, 0x1E05310, D+0xE30); 0xD4 the
//! target hidden and laid back in two halves (`split`, 0x1E04200, D+0xB48);
//! fades (D+0xEA8) on 0xC8 / 0xD0 / 0xEC / 0x10C / 0x118; a previous-frame
//! pass 0x64..0x73. Returns 1 on update 0xBE, 2 on 0xCB, 8 on 0x14A (no
//! drawing then) and at once for -2 / -3.
//!
//! Library: `camera_shake` 0x1E00080, `arches_b` 0x1E02010, `arcs_b`
//! 0x1E02870, `frame_rays_b` 0x1E03990, `fade` 0x1E05D00.
use crate::b035::light_ramp;
use crate::b076::{rand_bytes, shake_saving};
use crate::b101::unit_tint;
use crate::b299::in_window;
use crate::blib::exe::{camera_vp_into, countdown, libm_cosf, node_source, set_counter4, tex_rect_a3, tex_rect_strip};
use crate::blib::{self, afterimage, spray};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const COLUMNS: u32 = D + 0xA4;
const ARCHES: u32 = D + 0x6E4;
const ARCS: u32 = D + 0x834;
const RAYS: u32 = D + 0xB18;
const SPLIT: u32 = D + 0xB48;
const BARS: u32 = D + 0xB70;
const FLASH: u32 = D + 0xE30;
const FADE: u32 = D + 0xEA8;
const PI: u32 = 0x4049_0FDB;
const REGS_TEX4: u64 = 0x4214_2142_1421;
const EDGE: f32 = 28672.0;
const FAR_EDGE: f32 = 36864.0;
/// The columns' cells (0x28 bytes), the bars' and flashes' sprite specs,
/// the column offsets.
const CELLS: u32 = 0x01E0_6D80;
const BAR_SPEC: u32 = 0x01E0_6E40;
const FLASH_SPEC: u32 = 0x01E0_6E60;
const OFFSETS: u32 = 0x01E0_6E80;

/// The entry 0x1E061D0 (D, param), frame 0x190.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x190);
    for k in (0..0xC0).step_by(8) {
        let v = fx.mem.read_u64(OFFSETS + k);
        fx.mem.write_u64(sp + 0x80 + k, v);
    }
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0x190);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0xFB0);
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
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x37, 1);
    fx.mem.set32(D + 0xC, r);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x36, 0);
    fx.mem.set32(D + 0x10, r);
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x144);
    rand_bytes(fx, D + 0xEB0, 0x100, 0x01E0_62F0, [Some(D), Some(3), None]);
    0
}

fn bytes(fx: &mut Fx, at: u32, v: &[u8]) {
    for (k, &b) in v.iter().enumerate() {
        fx.mem.write_u8(at + k as u32, b);
    }
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn u16f(fx: &mut Fx, a: u32) -> f32 {
    ee::from_i32(i32::from(fx.mem.read_u16(a)))
}

fn fade_arm(fx: &mut Fx, count: i32, kind: i32) {
    set_counter4(fx, FADE, count, 0, 0xFF);
    fx.mem.set8(FADE + 4, kind);
}

fn tint_bytes(fx: &mut Fx, sp: u32, v: u8) {
    bytes(fx, sp + 0x188, &[v; 4]);
    let h = fx.mem.i32(D + 8);
    unit_tint(fx, h, sp + 0x188);
}

fn arm_columns(fx: &mut Fx, sp: u32) {
    let rot = fx.mem.read_u32(D + 0x30);
    for k in 0..0x10u32 {
        let p = COLUMNS + 0x28 * k;
        let v = sp + 0x80 + 0xC * k;
        set_counter4(fx, p, 0xFA, (4 * k & 0xFF) as i32, 0xFF);
        fx.mem.write_u8(p + 4, 0);
        bytes(fx, p + 8, &[0, 2, 1, 1, 0]);
        fx.mem.write_u8(p + 0xE, 1);
        let r = fx.rand_from(D, 0x01E0_63EC);
        fx.mem.set8(p + 0xF, ((r >> 4) & 3) + 1);
        fx.mem.write_u32(p + 0x10, 0x4300_0000);
        bytes(fx, p + 0x14, &[0x80; 4]);
        copy4(fx, p + 0x18, sp + 0x70);
        for c in [0u32, 4, 8] {
            let (a, b) = (fx.mem.f(p + 0x18 + c), fx.mem.f(v + c));
            fx.mem.setf(p + 0x18 + c, if rot == 0x8000 { ee::add(a, b) } else { ee::sub(a, b) });
        }
        let x = fx.mem.f(p + 0x18);
        let on = if rot == 0x8000 { !(x < 0.0) } else { x <= 0.0 };
        fx.mem.write_u8(p + 4, u8::from(on));
    }
}

fn arm_arcs(fx: &mut Fx) {
    let p = ARCS;
    set_counter4(fx, p, 0x1C, 0, 0xFF);
    fx.mem.write_u8(p + 4, 1);
    fx.mem.write_u32(p + 8, D + 0x894);
    fx.mem.write_u8(p + 0x30, 0x28);
    fx.mem.write_u8(p + 0x31, 8);
    let a = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(p + 0x32, a);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(p + 0x33, b);
    fx.mem.write_u32(p + 0x38, 0x3F80_0000);
    fx.mem.write_u8(p + 0x34, 0);
    bytes(fx, p + 0x44, &[0x80; 4]);
    bytes(fx, p + 0x48, &[0x10, 0, 0x80]);
    let mut k = 0u32;
    while k < u32::from(fx.mem.read_u8(p + 0x31)) + 1 {
        fx.mem.set16(p + 0x1E + 2 * k, -1);
        fx.mem.set16(p + 0xC + 2 * k, -1);
        k += 1;
    }
}

fn arm_arches(fx: &mut Fx, sp: u32) {
    let (rot, h) = (fx.mem.read_u32(D + 0x30), fx.mem.i32(D + 4));
    for k in 0..4i32 {
        let p = ARCHES + 0x54 * k as u32;
        let kf = ee::from_i32(k);
        set_counter4(fx, p, 0x12, 0, 0xFF);
        bytes(fx, p + 4, &[0, 0]);
        fx.mem.write_u8(p + 7, 9);
        bytes(fx, p + 8, &[0x10, 0, 0x80, 0x20, 0x80, 0x80, 0x80, 0xA0]);
        fx.mem.setf(p + 0x10, ee::sub(13.0, kf));
        fx.mem.write_u32(p + 0x14, 0x4000_0000);
        fx.unit_point(1, h, p + 0x24);
        for c in [0u32, 4, 8] {
            let v = ee::div(ee::sub(fx.mem.f(sp + 0x70 + c), fx.mem.f(p + 0x24 + c)), 32.0);
            fx.mem.setf(p + 0x34 + c, v);
        }
        fx.mem.set32(p + 0x44, -0x4000);
        let z = ee::add(fx.mem.f(p + 0x2C), kf);
        fx.mem.setf(p + 0x2C, z);
        if rot != 0x8000 {
            let v = ee::mul(ee::from_i32(fx.mem.i32(p + 0x44)), -1.0);
            fx.fix_residue(v, 0x01E0_66F4);
            fx.mem.set32(p + 0x44, ee::fixsfsi(v));
            let z = ee::sub(fx.mem.f(p + 0x2C), ee::mul(2.0, kf));
            fx.mem.setf(p + 0x2C, z);
        }
    }
    set_counter4(fx, RAYS, 0xE, 5, 0xFF);
    fx.unit_point(1, h, RAYS + 0xC);
    bytes(fx, RAYS + 0x1C, &[0x80, 0x80, 0x80, 0x20, 0x80, 0x80, 0x80, 0x70]);
    bytes(fx, RAYS + 8, &[2, 1, 0xFF]);
}

/// Update 0xC8: the columns to state 3 (+0xD = k + 4), the bars and
/// flashes at the target's screen point.
fn arm_split(fx: &mut Fx, sp: u32) {
    for k in 0..0x10u32 {
        let p = COLUMNS + 0x28 * k;
        bytes(fx, p + 0xB, &[3, 0, k as u8 + 4]);
    }
    camera_vp_into(fx, sp + 0x140);
    let m = fx.read_m4(sp + 0x140);
    fx.project_at(sp + 0x70, &m, sp + 0x180, sp + 0x18C);
    let (sx, sy) = (u16f(fx, sp + 0x180), u16f(fx, sp + 0x182));
    let mut s3 = 0x3Ci32;
    for k in 0..6u32 {
        let p = BARS + 0x40 * k;
        if k == 0 {
            set_counter4(fx, p, 0x30, 0, 0xFF);
            fx.mem.setf(p + 0x20, sx);
            fx.mem.setf(p + 0x24, sy);
            bytes(fx, p + 8, &[0x80, 0, 0]);
            bytes(fx, p + 0xC, &[0x80, 0x80, 0x80, 0xC0]);
            fx.mem.write_u8(p + 4, 1);
            fx.mem.write_u32(p + 0x10, 0x4616_0000);
            fx.mem.write_u32(p + 0x14, 0x4280_0000);
            fx.mem.write_u32(p + 0x18, 0);
            fx.mem.write_u32(p + 0x1C, 0);
        } else {
            set_counter4(fx, p, 0x10, 0, 0xFF);
            fx.mem.setf(p + 0x20, EDGE);
            fx.mem.setf(p + 0x24, sy);
            fx.mem.write_u32(p + 0x30, 0x4420_0000);
            bytes(fx, p + 8, &[0x10, 0, 0x80]);
            bytes(fx, p + 0xC, &[0x80; 4]);
            fx.mem.setf(p + 0x10, ee::from_i32(s3 << 4));
            fx.mem.write_u32(p + 0x14, 0x4280_0000);
            fx.mem.setf(p + 0x18, ee::mul(20.0, ee::from_i32(k as i32)));
            fx.mem.write_u32(p + 0x1C, 0x40C0_0000);
        }
        s3 += 0x3C;
    }
    let mut s3 = 0x28i32;
    for k in 0..2u32 {
        let p = FLASH + 0x3C * k;
        set_counter4(fx, p, 0x10, 0, 0xFF);
        fx.mem.setf(p + 0x1C, EDGE);
        fx.mem.setf(p + 0x20, sy);
        fx.mem.write_u32(p + 0x2C, 0x4420_0000);
        bytes(fx, p + 8, &[0x10, 0, 0x80]);
        bytes(fx, p + 0xC, &[0x80, 0x80, 0x80, 0xC0]);
        fx.mem.write_u8(p + 4, 1);
        fx.mem.write_u8(p + 7, 9);
        fx.mem.setf(p + 0x10, ee::from_i32(s3 << 4));
        s3 += 0x28;
    }
    fade_arm(fx, 8, 0xA);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = i32::from(fx.mem.read_u16(D)) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, sp + 0x70);
    match i32::from(fx.mem.read_u16(D)) {
        0x14A => return 8,
        0x118 => {
            tint_bytes(fx, sp, 0x80);
            fade_arm(fx, 0x20, 1);
        }
        0x10C => {
            fx.unit_show(t, 1, 0x01E0_6B48);
            fade_arm(fx, 0xC, 4);
        }
        0xEC => fade_arm(fx, 0x20, 0x12),
        0xD4 => {
            tint_bytes(fx, sp, 0);
            let p = SPLIT;
            set_counter4(fx, p, 0x30, 0, 0xFF);
            fx.mem.write_u8(p + 4, 2);
            fx.mem.set32(p + 8, t);
            copy4(fx, p + 0xC, sp + 0x70);
        }
        0xD2 => {
            fx.unit_show(t, 0, 0x01E0_6AAC);
        }
        0xD0 => fade_arm(fx, 8, 9),
        0xC8 => arm_split(fx, sp),
        0x9F => arm_arches(fx, sp),
        0x94 => arm_arcs(fx),
        0x1F => arm_columns(fx, sp),
        _ => {}
    }
    let n = i32::from(fx.mem.read_u16(D));
    if (0x64..0x74).contains(&n) {
        let a = if n >= 0x70 { 0x70 - ((i32::from(fx.mem.read_u8(D)) - 0x6F) << 4) } else { 0x70 };
        fx.mem.set8(sp + 0x18B, a);
        bytes(fx, sp + 0x188, &[0x80; 3]);
        let alpha = fx.alpha_preset(0);
        fx.frame_pass(OT, 0, 1, alpha, sp + 0x188, 0x01E0_6C30);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_6C3C, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = i32::from(fx.mem.read_u16(D));
    light_ramp(fx, D + 0xA0, n, 0x01E0_6C48, [Some(u64::from(D)), Some(u64::from(OT))]);
    columns(fx, COLUMNS, 0x28);
    saves(fx, 0x320, 0x20, 9, 0x01E0_6C58);
    blib::arch::arches_b(fx, 0x01E0_2010, ARCHES, OT, 4);
    // A counted record leaves the k loops' a3 = 0 until a step sets it.
    let due = fx.mem.read_u8(ARCS) != 0 && fx.mem.read_u8(ARCS + 1) == 0;
    let (_, a3) = blib::arc::arcs_b_a3(fx, 0x01E0_2870, ARCS, OT, 1);
    let a3 = if due && a3 == 0x3F { 0 } else { a3 };
    blib::window::frame_rays_b(fx, 0x01E0_3990, RAYS, OT, 1, a3, [0, 0]);
    split(fx, SPLIT, 1);
    saves(fx, 0x260, 0x10, 9, 0x01E0_6C98);
    bars(fx, BARS, 0xB);
    saves(fx, 0x180, 0x10, 9, 0x01E0_6CA8);
    flash(fx, FLASH, 2);
    saves(fx, 0x1E0, 0x20, 9, 0x01E0_6CB8);
    blib::fade::fade(fx, FADE, OT);
    saves(fx, 0xF0, 0x10, 7, 0x01E0_6CC4);
    match fx.mem.read_u16(D) {
        0xCB => 2,
        0xBE => 1,
        _ => 0,
    }
}

/// A callee's `sq` saves left below the entry's sp: s0 = D, s1 = the OT,
/// the rest 0 [H: s2.. hold the event loops' values on those updates],
/// `n` registers from `at`, then ra.
fn saves(fx: &mut Fx, size: u32, at: u32, n: u32, ra: u32) {
    let f = fx.sp - size;
    fx.saved(f, &[(at, u64::from(D)), (at + 0x10, u64::from(OT))]);
    for k in 2..n {
        fx.saved(f, &[(at + 0x10 * k, 0)]);
    }
    fx.saved(f, &[(at + 0x10 * n, u64::from(ra))]);
}

fn fix16(fx: &mut Fx, a: u32, v: f32) {
    fx.mem.write_u16(a, ee::fixunssfsi(v) as u16);
}

/// A UV coordinate in 12.4 (14 bits, the top bits kept).
fn uv14(fx: &mut Fx, a: u32, v: i32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | ((v << 4) as u16 & 0x3FFF));
}

/// The column quad's UVs (descriptor sp + 0x280).
fn col_uv(fx: &mut Fx, sp: u32, u0: i32, u1: i32, v0: i32, v1: i32) {
    for (a, u, v) in [(0x298u32, u0, v0), (0x2B0, u1, v0), (0x2C8, u0, v1), (0x2E0, u1, v1)] {
        uv14(fx, sp + a, u);
        uv14(fx, sp + a + 2, v);
    }
}

fn col_tex(fx: &mut Fx, sp: u32, slot: u32) {
    let rh2 = fx.mem.read_u32(sp + slot);
    fx.desc_texture(sp + 0x280, rh2, 1);
}

/// x of the corners 0 / 2 (`a`) and 1 / 3 (`b`).
fn col_x(fx: &mut Fx, sp: u32, a: f32, b: f32) {
    fix16(fx, sp + 0x2D0, a);
    let v = fx.mem.read_u16(sp + 0x2D0);
    fx.mem.write_u16(sp + 0x2A0, v);
    fix16(fx, sp + 0x2E8, b);
    let v = fx.mem.read_u16(sp + 0x2E8);
    fx.mem.write_u16(sp + 0x2B8, v);
}

/// The y of the pair 0x2EA / 0x2D2 (`top`) or 0x2BA / 0x2A2.
fn col_y(fx: &mut Fx, sp: u32, top: bool, v: f32) {
    let (a, b) = if top { (0x2EAu32, 0x2D2u32) } else { (0x2BA, 0x2A2) };
    fix16(fx, sp + a, v);
    let x = fx.mem.read_u16(sp + a);
    fx.mem.write_u16(sp + b, x);
}

fn col_submit(fx: &mut Fx, sp: u32) {
    let w = fx.mem.f(sp + 0x318);
    let key = ee::fixunssfsi(w) as i32;
    fx.submit(sp + 0x280, OT, key);
}

/// Vertex alphas 0..3.
fn col_alpha(fx: &mut Fx, sp: u32, a: [u8; 4]) {
    for (o, v) in [0x293u32, 0x2AB, 0x2C3, 0x2DB].into_iter().zip(a) {
        fx.mem.write_u8(sp + o, v);
    }
}

/// 0x1E003A0(recs, ot, n), frame 0x320: 0x28-byte records (+8.. three
/// group-5 textures, +0xB the state, +0xC its count, +0xD a delay, +0x10
/// the half size, +0x14 colour, +0x18 the point).
fn columns(fx: &mut Fx, recs: u32, n: i32) {
    let sp = fx.enter(0x320);
    for k in (0..0x28).step_by(8) {
        let v = fx.mem.read_u64(CELLS + k);
        fx.mem.write_u64(sp + 0x250 + k, v);
    }
    fx.mem.write_u32(sp + 0x208, OT);
    fx.mem.set32(sp + 0x204, n);
    for k in 0..n as u32 {
        let p = recs + 0x28 * k;
        fx.mem.set32(sp + 0xC0, k as i32);
        column(fx, sp, p);
    }
    fx.leave(0x320);
}

fn column(fx: &mut Fx, sp: u32, p: u32) {
    if countdown(fx, p) == 0 {
        return;
    }
    let b = fx.mem.read_u8(p + 0xB);
    if b == 0 || b >= 4 {
        fx.mem.write_u8(p, 0);
        return;
    }
    for (o, s) in [(0x104u32, 8u32), (0x108, 9), (0x10C, 0xA)] {
        let id = i32::from(fx.mem.read_u8(p + s));
        let t = fx.texture(5, id);
        fx.mem.write_u32(sp + o, t);
    }
    camera_vp_into(fx, sp + 0x210);
    let d = sp + 0x280;
    fx.desc_init(d, 12, 0x34, REGS_TEX4);
    for (s, o) in [(0x16u32, 0u32), (0x15, 1), (0x14, 2)] {
        let v = fx.mem.read_u8(p + s);
        for b in [0x2D8u32, 0x2C0, 0x2A8, 0x290] {
            fx.mem.write_u8(sp + b + o, v);
        }
    }
    for o in [0x2DCu32, 0x2C4, 0x2AC, 0x294] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    let age = i32::from(fx.mem.read_u8(p + 2)) - i32::from(fx.mem.read_u8(p));
    fx.mem.set32(sp + 0xF0, age);
    let phase = ee::mul(10.0, ee::from_i32(age));
    for k in 0..4i32 {
        let a = ee::add(ee::mul(90.0, ee::from_i32(k)), phase);
        let a = ee::div(ee::mul(f32::from_bits(PI), a), 180.0);
        let s = fx.libm_sinf(a);
        let v = ee::mul(64.0, s);
        fx.mem.set8(sp + 0x31C + k as u32, ee::fixsfsi(v) + 0x40);
    }
    let m = fx.read_m4(sp + 0x210);
    let mut off = !fx.project_at(p + 0x18, &m, sp + 0x310, sp + 0x318);
    if !Fx::on_screen(fx.mem.read_u64(sp + 0x310)) {
        off = true;
    }
    let r = ee::div(fx.mem.f(p + 0x10), ee::div(fx.mem.f(sp + 0x318), 256.0));
    for o in [0x2EFu32, 0x2D7, 0x2BF, 0x2A7] {
        fx.mem.write_u8(sp + o, 0);
    }
    let far = fx.mem.read_u8(p + 4) & 0x80 != 0;
    let mut z = if far { 0xFF_FFFE } else { fx.xyz_z(sp + 0x310) };
    for o in [0x2E8u32, 0x2D0, 0x2B8, 0x2A0] {
        fx.set_xyz_z(sp + o, z);
        z = fx.xyz_z(sp + o);
    }
    if far {
        fx.mem.set32(sp + 0x318, 0);
    }
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    fx.desc_alpha(d, 0x48);
    match b {
        1 => rising(fx, sp, p, r, off),
        3 if fx.mem.read_u8(p + 0xD) == 0 => splitting(fx, sp, p, r, off),
        3 => {
            let v = fx.mem.read_u8(p + 0xD) - 1;
            fx.mem.write_u8(p + 0xD, v);
            glow(fx, sp, p, r, off);
        }
        _ => glow(fx, sp, p, r, off),
    }
    let b = fx.mem.read_u8(p + 0xB);
    if b == 3 || b == 0 {
        let c = 0x18u8.wrapping_sub(fx.mem.read_u8(p + 0xC));
        let e = c.wrapping_shl(1);
        col_alpha(fx, sp, [c, c, e, e]);
    } else {
        col_alpha(fx, sp, [0x10, 0x10, 0x40, 0x40]);
    }
    fx.desc_alpha(d, 0x48);
    let age = ee::from_i32(fx.mem.i32(sp + 0xF0));
    let w1 = ee::mul(f32::from_bits(0x3F99_999A), r);
    let h1 = ee::mul(f32::from_bits(0x4019_999A), r);
    let turn = ee::add(270.0, ee::mul(3.5, age));
    for s in 0..2i32 {
        let e = i32::from(fx.mem.read_u8(p + 2)) - i32::from(fx.mem.read_u8(p));
        let idx = ((e + 4 * s) >> 3) & 7;
        let c = sp + 0x250 + 4 * idx as u32;
        let (u0, u1, v0, v1) = (i32::from(fx.mem.read_u8(c)), i32::from(fx.mem.read_u8(c + 1)), i32::from(fx.mem.read_u8(c + 2)), i32::from(fx.mem.read_u8(c + 3)));
        col_uv(fx, sp, u0, u1, v0, v1);
        col_tex(fx, sp, 0x10C);
        let x = u16f(fx, sp + 0x310);
        if fx.mem.read_u8(p + 4) & 1 != 0 {
            col_x(fx, sp, ee::add(x, w1), ee::sub(x, w1));
        } else {
            col_x(fx, sp, ee::sub(x, w1), ee::add(x, w1));
        }
        let y = u16f(fx, sp + 0x312);
        col_y(fx, sp, true, ee::add(y, h1));
        let len = ee::div(ee::mul(r, ee::from_i32(v1 - v0)), 32.0);
        let a = ee::div(ee::mul(f32::from_bits(PI), turn), 180.0);
        let sn = fx.libm_sinf(a);
        let dy = ee::mul(len, sn);
        let y = u16f(fx, sp + 0x2EA);
        let v = ee::sub(ee::sub(y, ee::mul(f32::from_bits(0x3FCC_CCCD), len)), dy);
        col_y(fx, sp, false, v);
        col_submit(fx, sp);
    }
}

/// State 1: the column rising by the cell's rows for 32 counts.
fn rising(fx: &mut Fx, sp: u32, p: u32, r: f32, off: bool) {
    if fx.mem.read_u8(p + 0xC) == 0x1F {
        let b = fx.mem.read_u8(p + 0xB) + 1;
        fx.mem.write_u8(p + 0xB, b);
    }
    let c = fx.mem.read_u8(p + 0xC).wrapping_add(1);
    fx.mem.write_u8(p + 0xC, c);
    if off {
        return;
    }
    let a = c.wrapping_shl(2);
    col_alpha(fx, sp, [a; 4]);
    let (u0, u1, s3) = (i32::from(fx.mem.read_u8(sp + 0x270)), i32::from(fx.mem.read_u8(sp + 0x271)), i32::from(fx.mem.read_u8(sp + 0x272)));
    let s2 = (s3 + i32::from(c)) << 2;
    col_uv(fx, sp, u0, u1, s3, s2);
    col_tex(fx, sp, 0x104);
    let x = u16f(fx, sp + 0x310);
    if fx.mem.read_u8(p + 4) & 1 != 0 {
        col_x(fx, sp, ee::add(x, r), ee::sub(x, r));
    } else {
        col_x(fx, sp, ee::sub(x, r), ee::add(x, r));
    }
    let y = u16f(fx, sp + 0x312);
    col_y(fx, sp, true, ee::add(y, ee::mul(2.0, r)));
    let d = ee::div(ee::mul(r, ee::from_i32(s2 - s3)), 32.0);
    let y = u16f(fx, sp + 0x2EA);
    col_y(fx, sp, false, ee::sub(y, d));
    col_submit(fx, sp);
}

/// State 3: the cell in two halves sliding apart, fading out after 16.
fn splitting(fx: &mut Fx, sp: u32, p: u32, r: f32, off: bool) {
    if fx.mem.read_u8(p + 0xC) == 0x17 {
        fx.mem.write_u8(p + 0xB, 0);
    }
    let c = fx.mem.read_u8(p + 0xC).wrapping_add(1);
    fx.mem.write_u8(p + 0xC, c);
    if off {
        return;
    }
    let b = |fx: &mut Fx, o: u32| i32::from(fx.mem.read_u8(sp + o));
    let (u0, u1, v0, v1) = (b(fx, 0x274), b(fx, 0x275), b(fx, 0x276), b(fx, 0x277));
    let mid = v0 + ((v1 - v0) >> 1);
    let slide = ee::mul(ee::div(r, 32.0), ee::from_i32(i32::from(c)));
    let (f22, f23) = if c >= 0x11 {
        let a = 0x18u8.wrapping_sub(c).wrapping_shl(4);
        col_alpha(fx, sp, [a; 4]);
        (slide, slide)
    } else {
        col_alpha(fx, sp, [0x80; 4]);
        (0.0, 0.0)
    };
    let flip = fx.mem.read_u8(p + 4) & 1 != 0;
    col_uv(fx, sp, u0, u1, v0, mid);
    col_tex(fx, sp, 0x108);
    let x = u16f(fx, sp + 0x310);
    if flip {
        col_x(fx, sp, ee::sub(ee::add(slide, ee::add(x, r)), f23), ee::add(f23, ee::add(slide, ee::sub(x, r))));
    } else {
        col_x(fx, sp, ee::add(f23, ee::sub(ee::sub(x, r), slide)), ee::sub(ee::sub(ee::add(x, r), slide), f23));
    }
    let len = ee::div(ee::mul(r, ee::from_i32(u1 - u0)), 32.0);
    let y = fx.mem.read_u16(sp + 0x312);
    fx.mem.write_u16(sp + 0x2EA, y);
    fx.mem.write_u16(sp + 0x2D2, y);
    let yf = u16f(fx, sp + 0x2EA);
    col_y(fx, sp, false, ee::sub(ee::sub(yf, len), f22));
    col_submit(fx, sp);
    col_uv(fx, sp, u0, u1, mid, v1);
    col_tex(fx, sp, 0x108);
    let x = u16f(fx, sp + 0x310);
    if flip {
        col_x(fx, sp, ee::sub(ee::sub(ee::add(x, r), slide), f23), ee::add(f23, ee::sub(ee::sub(x, r), slide)));
    } else {
        col_x(fx, sp, ee::add(f23, ee::add(slide, ee::sub(x, r))), ee::sub(ee::add(slide, ee::add(x, r)), f23));
    }
    let y = fx.mem.read_u16(sp + 0x312);
    fx.mem.write_u16(sp + 0x2BA, y);
    fx.mem.write_u16(sp + 0x2A2, y);
    let yf = u16f(fx, sp + 0x2BA);
    col_y(fx, sp, true, ee::add(f22, ee::add(yf, len)));
    col_submit(fx, sp);
}

/// State 2: four quarter quads, corner alphas from the pulse bytes sp +
/// 0x31C.., their centre corner wobbling by r / 4 along the angle 90 +
/// age x +0xF.
fn glow(fx: &mut Fx, sp: u32, p: u32, r: f32, off: bool) {
    if off {
        return;
    }
    let b = |fx: &mut Fx, o: u32| i32::from(fx.mem.read_u8(sp + o));
    let (s0, s1, s2, s3) = (b(fx, 0x270), b(fx, 0x271), b(fx, 0x272), b(fx, 0x273));
    let (hu, hv) = ((s1 - s0) >> 1, (s3 - s2) >> 1);
    let (age, rate) = (fx.mem.i32(sp + 0xF0), i32::from(fx.mem.read_u8(p + 0xF)));
    let t = fx.mult(age, rate);
    let a = ee::div(ee::mul(f32::from_bits(PI), ee::add(90.0, ee::from_i32(t))), 180.0);
    let sn = fx.libm_sinf(a);
    let q = ee::div(r, 4.0);
    let dx = ee::mul(q, sn);
    let cs = libm_cosf(fx, a);
    let dy = ee::mul(q, cs);
    let (mu, mv) = (s0 + hu, s2 + hv);
    let tall = ee::mul(2.0, r);
    let flip = fx.mem.read_u8(p + 4) & 1 != 0;
    let pulse: [i32; 4] = std::array::from_fn(|k| i32::from(fx.mem.read_u8(sp + 0x31C + k as u32)));
    for quarter in 0..4 {
        let right = quarter & 1 != 0;
        let lower = quarter >= 2;
        let x = fx.mem.read_u16(sp + 0x310);
        let xf = ee::from_i32(i32::from(x));
        if right {
            fx.mem.write_u16(sp + 0x2D0, x);
            fx.mem.write_u16(sp + 0x2A0, x);
            let v = if flip { ee::sub(xf, r) } else { ee::add(xf, r) };
            fix16(fx, sp + 0x2E8, v);
            let h = fx.mem.read_u16(sp + 0x2E8);
            fx.mem.write_u16(sp + 0x2B8, h);
        } else {
            let v = if flip { ee::add(xf, r) } else { ee::sub(xf, r) };
            fix16(fx, sp + 0x2D0, v);
            let h = fx.mem.read_u16(sp + 0x2D0);
            fx.mem.write_u16(sp + 0x2A0, h);
            fx.mem.write_u16(sp + 0x2E8, x);
            fx.mem.write_u16(sp + 0x2B8, x);
        }
        let y = fx.mem.read_u16(sp + 0x312);
        let yf = ee::from_i32(i32::from(y));
        if lower {
            fx.mem.write_u16(sp + 0x2BA, y);
            fx.mem.write_u16(sp + 0x2A2, y);
            col_y(fx, sp, true, ee::add(yf, tall));
        } else {
            col_y(fx, sp, false, ee::sub(yf, tall));
            fx.mem.write_u16(sp + 0x2EA, y);
            fx.mem.write_u16(sp + 0x2D2, y);
        }
        let (cx, cy) = match quarter {
            0 => (0x2E8u32, 0x2EAu32),
            1 => (0x2D0, 0x2D2),
            2 => (0x2B8, 0x2BA),
            _ => (0x2A0, 0x2A2),
        };
        let v = ee::add(u16f(fx, sp + cx), dx);
        fix16(fx, sp + cx, v);
        let v = ee::add(u16f(fx, sp + cy), dy);
        fix16(fx, sp + cy, v);
        let h = |a: i32, b: i32| ((a + b) >> 1) as u8;
        let al = match quarter {
            0 => [pulse[0] as u8, h(pulse[0], pulse[1]), h(pulse[0], pulse[2]), 0x80],
            1 => [h(pulse[0], pulse[1]), pulse[1] as u8, 0x80, h(pulse[0], pulse[3])],
            2 => [h(pulse[0], pulse[2]), 0x80, pulse[2] as u8, h(pulse[2], pulse[3])],
            _ => [0x80, h(pulse[1], pulse[3]), h(pulse[2], pulse[3]), pulse[3] as u8],
        };
        col_alpha(fx, sp, al);
        let (u0, u1) = if right { (mu, s1) } else { (s0, mu) };
        let (v0, v1) = if lower { (mv, s3) } else { (s2, mv) };
        col_uv(fx, sp, u0, u1, v0, v1);
        col_tex(fx, sp, 0x104);
        col_submit(fx, sp);
    }
}

/// 0x1F76A0(h, out): `out` = the unit's tint (obj+0x12C.., 0x80 each when
/// it has none).
fn tint_get(fx: &mut Fx, h: i32, out: u32) {
    bytes(fx, out, &[0x80; 4]);
    let (side, slot) = (h >> 8, h & 0xFF);
    if (0..3).contains(&side) && (0..0xB).contains(&slot) {
        let i = fx.object_index(side, slot);
        if (0..0x54).contains(&i) {
            let o = fx.mem.read_u32(crate::world::OBJECT_TABLE + 4 * i as u32);
            if o != 0 {
                for k in 0..4u32 {
                    let v = fx.mem.read_u8(o + 0x12C + k);
                    fx.mem.write_u8(out + k, v);
                }
            }
        }
    }
}

/// 0x1E04200(recs, ot, n), frame 0x260: the unit of +8 drawn untinted
/// into the offscreen target and laid back, in a packet layer, in two
/// halves split at its point +0xC's screen y and sliding sideways by 32
/// per count; 0x28-byte records.
fn split(fx: &mut Fx, recs: u32, n: i32) {
    let sp = fx.enter(0x260);
    fx.mem.write_u32(sp + 0xE4, n as u32);
    fx.mem.write_u32(sp + 0xE8, OT);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xCC, layer);
    fx.mem.set32(sp + 0xD0, 0);
    for k in 0..n as u32 {
        split_one(fx, sp, recs + 0x28 * k);
    }
    if fx.mem.i32(sp + 0xD0) != 0 {
        let w = fx.mem.f(sp + 0x250);
        let key = ee::fixunssfsi(w) as i32;
        fx.layer_end(layer, OT, key);
    }
    fx.leave(0x260);
}

fn split_one(fx: &mut Fx, sp: u32, p: u32) {
    if countdown(fx, p) == 0 {
        return;
    }
    node_source(fx, p, p + 0xC);
    camera_vp_into(fx, sp + 0xF0);
    let eye = fx.camera_eye(0);
    copy4(fx, sp + 0x130, eye);
    let h = fx.mem.i32(p + 8);
    fx.unit_point(1, h, sp + 0x140);
    if fx.mem.read_u8(p + 4) & 2 == 0 {
        for c in [0u32, 4, 8] {
            let v = ee::sub(fx.mem.f(sp + 0x130 + c), fx.mem.f(sp + 0x140 + c));
            fx.mem.setf(sp + 0x150 + c, v);
        }
        spray::aim_rotation(fx, sp + 0x150, sp + 0x170, [u64::from(p), 0], 0x01E0_4340);
        fx.mem.set32(sp + 0x158, 0);
        fx.mem.set32(sp + 0x150, 0);
        let h = fx.mem.i32(p + 8);
        let r = afterimage::unit_radius(fx, h);
        fx.mem.setf(sp + 0x154, r);
        if fx.mem.read_u8(p + 4) & 1 != 0 {
            let v = ee::mul(fx.mem.f(sp + 0x154), -1.0);
            fx.mem.setf(sp + 0x154, v);
        }
        let m = fx.read_m4(sp + 0x170);
        let v = fx.read_v4(sp + 0x150);
        let o = fx.transform(Some(&m), v);
        fx.write_v4(sp + 0x160, o);
    } else {
        for o in [0x168u32, 0x164, 0x160] {
            fx.mem.set32(sp + o, 0);
        }
    }
    for c in [0u32, 4, 8] {
        let v = ee::add(fx.mem.f(sp + 0x160 + c), fx.mem.f(sp + 0x140 + c));
        fx.mem.setf(sp + 0x160 + c, v);
    }
    let m = fx.read_m4(sp + 0xF0);
    if !fx.project_at(sp + 0x160, &m, sp + 0x1B0, sp + 0x250) || !in_window(fx, sp + 0x1B0) {
        return;
    }
    if !fx.project_at(p + 0xC, &m, sp + 0x1B8, sp + 0x254) || !in_window(fx, sp + 0x1B8) {
        return;
    }
    let v = ee::sub(u16f(fx, sp + 0x1BA), EDGE);
    fx.fix_residue(v, 0x01E0_4478);
    let s1 = ee::fixsfsi(v);
    let d = sp + 0x1C0;
    if fx.mem.i32(sp + 0xD0) == 0 {
        fx.desc_init(d, 12, 0x34, REGS_TEX4);
        for o in [0x218u32, 0x200, 0x1E8, 0x1D0] {
            bytes(fx, sp + o, &[0, 0, 0, 0x80]);
            fx.mem.write_u32(sp + o + 4, 0x3F80_0000);
        }
        let mut z = 0xFF_FFFF;
        for o in [0x228u32, 0x210, 0x1F8, 0x1E0] {
            fx.set_xyz_z(sp + o, z);
            z = fx.xyz_z(sp + o);
            fx.mem.write_u8(sp + o + 7, 0);
        }
        let h = fx.mem.i32(p + 8);
        tint_get(fx, h, sp + 0x25C);
        bytes(fx, sp + 0x258, &[0x80; 4]);
        unit_tint(fx, h, sp + 0x258);
        // a0..a3 as 0x1F75F0 leaves them: the tint's last byte twice, the
        // colour, the unit object [run].
        let (side, slot) = (h >> 8, h & 0xFF);
        let i = fx.object_index(side, slot);
        let o = fx.mem.read_u32(crate::world::OBJECT_TABLE + 4 * i as u32);
        fx.texflush([0x80, 0x80, sp + 0x258, o]);
        let (_, r) = fx.unit_draw(h, 0, sp + 0x238, sp + 0x240);
        fx.texflush(r);
        fx.mem.set32(sp + 0xD0, 1);
        unit_tint(fx, h, sp + 0x25C);
    }
    for o in [0x218u32, 0x200, 0x1E8, 0x1D0] {
        bytes(fx, sp + o, &[0x80; 4]);
        fx.mem.write_u32(sp + o + 4, 0x3F80_0000);
    }
    let mut z = fx.xyz_z(sp + 0x1B0);
    for o in [0x228u32, 0x210, 0x1F8, 0x1E0] {
        fx.set_xyz_z(sp + o, z);
        z = fx.xyz_z(sp + o);
    }
    fx.desc_alpha(d, 0x44);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let x = fx.mem.read_u16(sp + 0x1B0);
    fx.mem.write_u16(sp + 0x228, x);
    fx.mem.write_u16(sp + 0x210, x);
    let y = fx.mem.read_u16(sp + 0x1BA);
    fx.mem.write_u16(sp + 0x22A, y);
    fx.mem.write_u16(sp + 0x212, y);
    let age = i32::from(fx.mem.read_u8(p + 2)) - i32::from(fx.mem.read_u8(p));
    let slide = ee::from_i32(age << 5);
    let cut = s1 & 0x3FFF;
    let set = |fx: &mut Fx, a: u32, v: i32| {
        let h = fx.mem.read_u16(a);
        fx.mem.write_u16(a, (h & 0xC000) | v as u16);
    };
    for (a, v) in [(0x1D8u32, 0), (0x1DA, 0), (0x1F0, 0x2000), (0x1F2, 0), (0x208, 0), (0x20A, cut), (0x220, 0x2000), (0x222, cut)] {
        set(fx, sp + a, v);
    }
    let v = ee::sub(EDGE, slide);
    fix16(fx, sp + 0x210, v);
    let h = fx.mem.read_u16(sp + 0x210);
    fx.mem.write_u16(sp + 0x1E0, h);
    let v = ee::sub(FAR_EDGE, slide);
    fix16(fx, sp + 0x228, v);
    let h = fx.mem.read_u16(sp + 0x228);
    fx.mem.write_u16(sp + 0x1F8, h);
    fx.mem.write_u16(sp + 0x1FA, 0x7000);
    fx.mem.write_u16(sp + 0x1E2, 0x7000);
    let y = fx.mem.read_u16(sp + 0x1BA);
    fx.mem.write_u16(sp + 0x22A, y);
    fx.mem.write_u16(sp + 0x212, y);
    let w = fx.mem.f(sp + 0x250);
    fx.submit(d, 0, ee::fixunssfsi(w) as i32);
    for (a, v) in [(0x1D8u32, 0), (0x1DA, cut), (0x1F0, 0x2000), (0x1F2, cut), (0x208, 0), (0x20A, 0x2000), (0x220, 0x2000), (0x222, 0x2000)] {
        set(fx, sp + a, v);
    }
    let v = ee::add(EDGE, slide);
    fix16(fx, sp + 0x210, v);
    let h = fx.mem.read_u16(sp + 0x210);
    fx.mem.write_u16(sp + 0x1E0, h);
    let v = ee::add(FAR_EDGE, slide);
    fix16(fx, sp + 0x228, v);
    let h = fx.mem.read_u16(sp + 0x228);
    fx.mem.write_u16(sp + 0x1F8, h);
    let y = fx.mem.read_u16(sp + 0x1BA);
    fx.mem.write_u16(sp + 0x1FA, y);
    fx.mem.write_u16(sp + 0x1E2, y);
    fx.mem.write_u16(sp + 0x22A, 0x9000);
    fx.mem.write_u16(sp + 0x212, 0x9000);
    let w = fx.mem.f(sp + 0x250);
    fx.submit(d, 0, ee::fixunssfsi(w) as i32);
}

/// 0x1E04AF0(recs, ot, n), frame 0x180: [`crate::blib::cross::bars`] on
/// the screen: 0x40-byte records, the centre the floats +0x20 / +0x24 in
/// 12.4, length +0x10 and width +0x14 as they are, far Z, key 0 [F].
fn bars(fx: &mut Fx, recs: u32, n: i32) {
    let sp = fx.enter(0x180);
    for k in (0..0x18).step_by(8) {
        let v = fx.mem.read_u64(BAR_SPEC + k);
        fx.mem.write_u64(sp + 0xD0 + k, v);
    }
    fx.mem.set32(sp + 0xCC, n);
    let (kind, id) = (fx.mem.i32(sp + 0xD0), i32::from(fx.mem.read_u8(sp + 0xD4)));
    let tex = fx.texture(kind, id);
    fx.mem.write_u32(sp + 0xC0, tex);
    for k in 0..n as u32 {
        fx.mem.set32(sp + 0xB0, k as i32);
        bar(fx, sp, recs + 0x40 * k, tex);
    }
    fx.leave(0x180);
}

fn bar(fx: &mut Fx, sp: u32, p: u32, tex: u32) {
    if countdown(fx, p) == 0 {
        return;
    }
    for k in 0..3u32 {
        let v = ee::add(fx.mem.f(p + 0x20 + 4 * k), fx.mem.f(p + 0x30 + 4 * k));
        fx.mem.setf(p + 0x20 + 4 * k, v);
    }
    let v = ee::add(fx.mem.f(p + 0x18), fx.mem.f(p + 0x1C));
    fx.mem.setf(p + 0x18, v);
    let d = sp + 0xF0;
    fx.desc_init(d, 12, 0x34, REGS_TEX4);
    for o in [0x100u32, 0x101, 0x102] {
        fx.mem.write_u8(sp + o, 0);
    }
    for (s, dsts) in [(0xAu32, &[0x130u32, 0x118][..]), (9, &[0x131, 0x119]), (8, &[0x132, 0x11A]), (0xE, &[0x148]), (0xD, &[0x149]), (0xC, &[0x14A])] {
        let v = fx.mem.read_u8(p + s);
        for &o in dsts {
            fx.mem.write_u8(sp + o, v);
        }
    }
    for o in [0x14Cu32, 0x134, 0x11C, 0x104] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    tex_rect_strip(fx, d, sp + 0xD0, tex);
    let (a, c) = (fx.mem.u8(p + 0xF), fx.mem.u8(p));
    let v = if c < 8 && fx.mem.u8(p + 4) & 0x40 == 0 { fx.mult(a >> 3, c) } else { a };
    for o in [0x14Bu32, 0x133, 0x11B] {
        fx.mem.set8(sp + o, v);
    }
    fx.mem.write_u8(sp + 0x103, 0);
    let x = fx.mem.f(p + 0x20);
    fix16(fx, sp + 0xE8, x);
    let y = fx.mem.f(p + 0x24);
    fix16(fx, sp + 0xEA, y);
    fx.set_xyz_z(sp + 0xE8, 0xFF_FFFE);
    let mut z = fx.xyz_z(sp + 0xE8);
    for o in [0x158u32, 0x140, 0x128, 0x110] {
        fx.set_xyz_z(sp + o, z);
        z = fx.xyz_z(sp + o);
        fx.mem.write_u8(sp + o + 7, 1);
    }
    let len = fx.mem.f(p + 0x10);
    let mut r = len;
    let f = fx.mem.u8(p + 4);
    if f & 0x40 == 0 {
        if f & 1 == 0 {
            let g = (fx.mem.u8(p + 2) - fx.mem.u8(p)) & 0xFF;
            if g < 8 {
                r = ee::mul(len, ee::mul(0.125, ee::from_i32(g)));
            }
        }
        let c = fx.mem.u8(p);
        if c < 8 {
            r = ee::mul(len, ee::mul(0.125, ee::from_i32(c)));
        }
    }
    let width = fx.mem.f(p + 0x14);
    let abs = |v: f32| if v < 0.0 { ee::neg(v) } else { v };
    for s in 0..2 {
        let half = ee::mul(180.0, ee::from_i32(s));
        let ang = ee::div(ee::mul(f32::from_bits(PI), ee::add(fx.mem.f(p + 0x18), half)), 180.0);
        let dx = ee::mul(r, libm_cosf(fx, ang));
        let sn = fx.libm_sinf(ang);
        let dy = ee::mul(r, sn);
        if !(abs(dx) <= abs(dy)) {
            let x = fx.mem.read_u16(sp + 0xE8);
            let xf = ee::from_i32(i32::from(x));
            fix16(fx, sp + 0x140, ee::add(xf, dx));
            fix16(fx, sp + 0x110, ee::add(xf, dx));
            fx.mem.write_u16(sp + 0x158, x);
            fx.mem.write_u16(sp + 0x128, x);
            let y = fx.mem.read_u16(sp + 0xEA);
            let yf = ee::from_i32(i32::from(y));
            let e = ee::add(yf, dy);
            fix16(fx, sp + 0x112, ee::sub(e, width));
            fix16(fx, sp + 0x12A, ee::sub(yf, width));
            fix16(fx, sp + 0x142, e);
            fx.mem.write_u16(sp + 0x15A, y);
            fx.submit(d, OT, 0);
            fix16(fx, sp + 0x112, ee::add(width, ee::add(yf, dy)));
            fix16(fx, sp + 0x12A, ee::add(yf, width));
            fx.submit(d, OT, 0);
        } else {
            let y = fx.mem.read_u16(sp + 0xEA);
            let yf = ee::from_i32(i32::from(y));
            fix16(fx, sp + 0x142, ee::add(yf, dy));
            fix16(fx, sp + 0x112, ee::add(yf, dy));
            fx.mem.write_u16(sp + 0x15A, y);
            fx.mem.write_u16(sp + 0x12A, y);
            let x = fx.mem.read_u16(sp + 0xE8);
            let xf = ee::from_i32(i32::from(x));
            let e = ee::add(xf, dx);
            fix16(fx, sp + 0x110, ee::sub(e, width));
            fix16(fx, sp + 0x128, ee::sub(xf, width));
            fix16(fx, sp + 0x140, e);
            fx.mem.write_u16(sp + 0x158, x);
            fx.submit(d, OT, 0);
            fix16(fx, sp + 0x110, ee::add(width, ee::add(xf, dx)));
            fix16(fx, sp + 0x128, ee::add(xf, width));
            fx.submit(d, OT, 0);
        }
    }
}

/// 0x1E05310(recs, ot, n), frame 0x1E0: [`crate::blib::star::flash`] on
/// the screen: 0x3C-byte records, the centre the floats +0x1C / +0x20 in
/// 12.4, radii +0x10 / +0x18 as they are, no vertical stretch, far Z, key
/// 0, each spike drawn once [F].
fn flash(fx: &mut Fx, recs: u32, n: i32) {
    let sp = fx.enter(0x1E0);
    for k in (0..0x18).step_by(8) {
        let v = fx.mem.read_u64(FLASH_SPEC + k);
        fx.mem.write_u64(sp + 0x100 + k, v);
    }
    fx.mem.set32(sp + 0xFC, n);
    let (kind, id) = (fx.mem.i32(sp + 0x100), i32::from(fx.mem.read_u8(sp + 0x104)));
    let tex = fx.texture(kind, id);
    fx.mem.write_u32(sp + 0xD0, tex);
    for k in 0..n as u32 {
        fx.mem.set32(sp + 0xC0, k as i32);
        flash_one(fx, sp, recs + 0x3C * k);
    }
    fx.leave(0x1E0);
}

fn flash_one(fx: &mut Fx, sp: u32, g: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    for k in 0..3u32 {
        let v = ee::add(fx.mem.f(g + 0x1C + 4 * k), fx.mem.f(g + 0x2C + 4 * k));
        fx.mem.setf(g + 0x1C + 4 * k, v);
    }
    let v = ee::add(fx.mem.f(g + 0x10), fx.mem.f(g + 0x14));
    fx.mem.setf(g + 0x10, v);
    let mut spokes = fx.mem.u8(g + 7);
    if spokes >= 10 {
        spokes = 9;
    }
    let step = ee::div(90.0, ee::from_i32(spokes));
    let x = fx.mem.f(g + 0x1C);
    fix16(fx, sp + 0x118, x);
    let y = fx.mem.f(g + 0x20);
    fix16(fx, sp + 0x11A, y);
    fx.set_xyz_z(sp + 0x118, 0xFF_FFFE);
    let (r, r2) = (fx.mem.f(g + 0x10), fx.mem.f(g + 0x18));
    let a = sp + 0x120;
    fx.desc_init(a, 6, 0x13, 0x41_4141);
    for (s, o) in [(0xEu32, 0x130u32), (0xD, 0x131), (0xC, 0x132)] {
        let v = fx.mem.read_u8(g + s);
        fx.mem.write_u8(sp + o, v);
    }
    for (s, o) in [(0xAu32, 0u32), (9, 1), (8, 2)] {
        let v = fx.mem.read_u8(g + s);
        fx.mem.write_u8(sp + 0x150 + o, v);
        fx.mem.write_u8(sp + 0x140 + o, v);
    }
    for o in [0x154u32, 0x144, 0x134] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    let alpha = if fx.mem.u8(g + 4) & 8 != 0 { 0x42 } else { 0x48 };
    fx.desc_alpha(a, alpha);
    let b = sp + 0x168;
    fx.desc_init(b, 9, 0x24, 0x4_2424_2421);
    for (s, o) in [(0xAu32, 0x178u32), (9, 0x179), (8, 0x17A)] {
        let v = fx.mem.read_u8(g + s);
        fx.mem.write_u8(sp + o, v);
    }
    fx.mem.write_u32(sp + 0x17C, 0x3F80_0000);
    let tex = fx.mem.read_u32(sp + 0xD0);
    tex_rect_a3(fx, b, sp + 0x100, tex);
    let (ca, sa) = (sp + 0x133, sp + 0x17B);
    let v = fx.mem.read_u8(g + 0xF);
    fx.mem.write_u8(ca, v);
    fx.mem.write_u8(sa, v >> 1);
    fx.mem.write_u8(sp + 0x153, 0);
    fx.mem.write_u8(sp + 0x143, 0);
    if fx.mem.u8(g + 4) & 0x40 == 0 {
        let count = fx.mem.u8(g);
        let k = if count < 8 { Some(count) } else { Some((fx.mem.u8(g + 2) - count) & 0xFF).filter(|&k| k < 8) };
        if let Some(k) = k {
            let a = fx.mem.u8(g + 0xF) >> 3;
            let v = fx.mult(a, k);
            fx.mem.set8(ca, v);
            let v = (fx.mem.u8(g + 0xF) >> 4).wrapping_mul(k);
            fx.mem.set8(sa, v);
        }
    }
    if fx.mem.u8(g) & 1 != 0 {
        if fx.mem.u8(g + 4) & 2 != 0 {
            let v = fx.mem.u8(ca);
            if v >= 0x11 {
                fx.mem.set8(ca, v - 0x10);
            }
        }
        if fx.mem.u8(g + 4) & 4 != 0 {
            let v = fx.mem.u8(ca);
            if v > 0 {
                fx.mem.set8(ca, v >> 1);
            }
        }
    }
    let c0 = libm_cosf(fx, 0.0);
    let mut x1 = ee::mul(r, c0);
    let s0 = fx.libm_sinf(0.0);
    let mut y1 = ee::mul(r, s0);
    let mut z = fx.xyz_z(sp + 0x118);
    for o in [0x158u32, 0x148, 0x138, 0x1B8, 0x1A8, 0x198, 0x188] {
        fx.set_xyz_z(sp + o, z);
        z = fx.xyz_z(sp + o);
        fx.mem.write_u8(sp + o + 7, 1);
    }
    let abs = |v: f32| if v < 0.0 { ee::neg(v) } else { v };
    let mut s = 1;
    while s < 4 * spokes + 1 {
        let (x0, y0) = (x1, y1);
        let ang = ee::div(ee::mul(f32::from_bits(PI), ee::mul(step, ee::from_i32(s))), 180.0);
        x1 = ee::mul(r, libm_cosf(fx, ang));
        let sn = fx.libm_sinf(ang);
        y1 = ee::mul(r, sn);
        let (cx, cy) = (fx.mem.read_u16(sp + 0x118), fx.mem.read_u16(sp + 0x11A));
        fx.mem.write_u16(sp + 0x138, cx);
        fx.mem.write_u16(sp + 0x13A, cy);
        let (xf, yf) = (ee::from_i32(i32::from(cx)), ee::from_i32(i32::from(cy)));
        fix16(fx, sp + 0x148, ee::add(xf, x0));
        fix16(fx, sp + 0x14A, ee::add(yf, y0));
        fix16(fx, sp + 0x158, ee::add(xf, x1));
        fix16(fx, sp + 0x15A, ee::add(yf, y1));
        fx.submit(a, OT, 0);
        if fx.mem.u8(g + 4) & 1 == 0 {
            if !(abs(ee::sub(x1, x0)) <= abs(ee::sub(y1, y0))) {
                let v = ee::add(xf, x0);
                fix16(fx, sp + 0x1A8, v);
                fix16(fx, sp + 0x188, v);
                let v = ee::add(xf, x1);
                fix16(fx, sp + 0x1B8, v);
                fix16(fx, sp + 0x198, v);
                let t = ee::add(yf, y0);
                fix16(fx, sp + 0x18A, ee::sub(t, r2));
                fix16(fx, sp + 0x1AA, ee::add(r2, t));
                let t = ee::add(yf, y1);
                fix16(fx, sp + 0x19A, ee::sub(t, r2));
                fix16(fx, sp + 0x1BA, ee::add(r2, t));
            } else {
                let v = ee::add(yf, y0);
                fix16(fx, sp + 0x1AA, v);
                fix16(fx, sp + 0x18A, v);
                let v = ee::add(yf, y1);
                fix16(fx, sp + 0x1BA, v);
                fix16(fx, sp + 0x19A, v);
                let t = ee::add(xf, x0);
                fix16(fx, sp + 0x188, ee::add(r2, t));
                fix16(fx, sp + 0x1A8, ee::sub(t, r2));
                let t = ee::add(xf, x1);
                fix16(fx, sp + 0x198, ee::add(r2, t));
                fix16(fx, sp + 0x1B8, ee::sub(t, r2));
            }
            fx.submit(b, OT, 0);
        }
        s += 1;
    }
}
