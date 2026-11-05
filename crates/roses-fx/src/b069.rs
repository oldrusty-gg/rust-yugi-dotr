//! BMODULE b069 (card 550), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01210, data area 0x1ED0000) [F]: b086's older task pool: 150 tasks of 0x40 bytes at D+0x65C
//! (alive +0x36, step +0x24 as a record address). On update 45 two
//! glowing cores (0x1E00EF0, one quad 0x1E01830 at the attacker's node
//! 0x67 / 0x68, refreshed into D+0x20 / +0x30 every update) and four red
//! flames (0x1E010B0, four strip quads 0x1E01A70 about them, turning by 10
//! a update) start; from update 90, while the 381-update timer +0x3950 has
//! 68 or more left, two smoke rings a update (0x1E00BC0, two camera-facing
//! quads of width +0xC / height +0xA: cb18430f13b0 at 0x1E004D0,
//! `quads4::ring_facing_wh`, UVs by uv_rect4 0x1E00080) leave node 0x6A,
//! drifting toward the target in z. Returns 1 on update 170, 3 on 355, 6
//! on 181..344, 8 once the timer has run out.
use crate::blib::{quads4, uv};
use crate::bus::Bus;
use crate::c061::{blend_alpha, desc_uv4, rgbaq_at};
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x24), as the record's addresses.
const SMOKE: u32 = 0x01E0_0BC0;
const CORE: u32 = 0x01E0_0EF0;
const FLAME: u32 = 0x01E0_10B0;
/// The return addresses of the entry's `jal`s into the steps.
const SMOKE_RA: u64 = 0x01E0_1468;
const CORE_RA: u64 = 0x01E0_1488;
const FLAME_RA: u64 = 0x01E0_14A8;
const TIMER: u32 = D + 0x3950;
const COLOUR: u32 = D + 0x3954;
const COUNTER: u32 = D + 0x39B6;
const BYTES: u32 = D + 0x39BA;
/// The scratchpad, where the originals project.
const SPR: u32 = 0x7000_0000;

/// The entry 0x1E01210.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x120);
    fx.mem.write_u32(D + 0x395C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x120);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3C30);
    l2::sine_table(fx, D + 0x3ABC);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x39B4, param);
    fx.mem.set32(TIMER, 0x17D);
    for k in 0..3 {
        fx.mem.write_u32(D + 0x310 + 4 * k, 0);
    }
    for (k, node) in [0x67, 0x68, 0x6A, 0x26, 0xF, 0xA, 0x3D].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(D + 0x3998 + 2 * k as u32, r);
    }
    for k in 0xB..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x34C + 4 * (k as u32 - 0xB), t);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x328, t);
    for k in 0..2u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x3930 + 4 * k, t);
    }
    for k in 0x64..0xB4u32 {
        let v = rand_mod(fx, 0xC);
        fx.mem.set8(BYTES + k, v - 4);
    }
    for k in 0..0x64u32 {
        let v = rand_mod(fx, 0x14);
        fx.mem.set8(BYTES + k, v - 0xA);
    }
    for k in 0xB4..0x100u32 {
        let v = rand_mod(fx, 6);
        fx.mem.set8(BYTES + k, v - 3);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = D + 0x40 * k;
        if fx.mem.i16(t + 0x692) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x680) == SMOKE {
            smoke(fx, t + 0x65C);
        }
        if fx.mem.read_u32(t + 0x680) == CORE {
            core(fx, t + 0x65C);
        }
        if fx.mem.read_u32(t + 0x680) == FLAME {
            flame(fx, t + 0x65C);
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    // 0x1F71A0's frame below the entry's (its saved ra and the entry's s0
    // = D).
    fx.saved(sp - 0x20, &[(0x10, 0x01E0_14CC), (0, u64::from(D))]);
    let y = ee::sub(fx.mem.f(sp + 0x58), 25.0);
    fx.mem.setf(sp + 0x58, y);
    for k in 0..TASKS {
        let t = D + 0x40 * k;
        if fx.mem.i16(t + 0x692) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x680) == SMOKE {
            let sub = fx.mem.i16(t + 0x690);
            cell(fx, 0x3934, sp + 0x60, 0x14, sub);
            fx.mem.set16(D + 0xB8, 2);
            let v = fx.rand();
            fx.mem.set16(D + 0xBE, v % 4);
            quads4::ring_facing_wh(fx, uv::uv_rect4, D, t + 0x65C, 0, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x680) == CORE {
            let sub = fx.mem.i16(t + 0x690);
            cell(fx, 0x3930, sp + 0x70, 0x14, sub);
            core_quad(fx, t + 0x65C, sp + 0x90);
        }
        if fx.mem.read_u32(t + 0x680) == FLAME {
            cell(fx, 0x35C, sp + 0x80, 0x9A, 1);
            flame_quads(fx, t + 0x65C, sp + 0x90);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xAA => 1,
        0x163 => 3,
        0xB5..=0x158 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// Texture D+`tex` into D+0x88, the atlas cell `id` / `sub` (0x106960
/// into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, tex: u32, r: u32, id: i32, sub: i32) {
    let w = fx.mem.read_u32(D + tex);
    fx.mem.write_u32(D + 0x88, w);
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// 0x1E00A60(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the emitters (frame 0x30: ra, the entry's s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x10, 0x01E0_1434), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let saves = |ra: u64| [(0x50u32, ra), (0x20, param as i64 as u64), (0x10, 0), (0, u64::from(D))];
    if fx.mem.i16(COUNTER) == 0x2D {
        for (core_flag, ras) in [(0, [0x01E0_0AB0u64, 0x01E0_0AC8, 0x01E0_0AE4]), (1, [0x01E0_0B00, 0x01E0_0B18, 0x01E0_0B34])] {
            fx.mem.set16(D + 0x39B0, core_flag);
            spawn(fx, CORE, 0, &saves(ras[0]));
            fx.mem.set16(D + 0x39AA, 0);
            spawn(fx, FLAME, 0, &saves(ras[1]));
            fx.mem.set16(D + 0x39AA, 0x5A);
            spawn(fx, FLAME, 0, &saves(ras[2]));
        }
    }
    if fx.mem.i16(COUNTER) >= 0x5A && fx.mem.i32(TIMER) >= 0x44 {
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + 0x399C);
        fx.node_position(h, slot, sp + 0x20);
        for k in 0..3 {
            let w = fx.mem.read_u32(sp + 0x20 + 4 * k);
            fx.mem.write_u32(D + 0x2D0 + 4 * k, w);
        }
        fx.mem.set16(D + 0x39B0, 0);
        spawn(fx, SMOKE, 0, &saves(0x01E0_0B9C));
        spawn(fx, SMOKE, 0, &saves(0x01E0_0BB0));
    }
    fx.leave(0x30);
}

/// 0x1E01700(D, step, arg): the first free task cleared and set off at
/// D+0x2D0 (as i16) with the colour D+0x3954 (twice), +0x30 = D+0x39B0,
/// +0x3A = D+0x39AA; 1 when one was free (b086's 0x1E01730 [F
/// word-compare]). `saves`: its prologue's.
fn spawn(fx: &mut Fx, f: u32, arg: u32, saves: &[(u32, u64)]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x40 * k + 0x692) == 0) {
        Some(k) => {
            let t = D + 0x65C + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            for c in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x2D0, 0);
            let a = fx.mem.i16(D + 0x39B0);
            fx.mem.set16(t + 0x30, a);
            let a = fx.mem.i16(D + 0x39AA);
            fx.mem.set16(t + 0x3A, a);
            fx.mem.write_u32(t + 0x20, arg);
            fx.mem.write_u32(t + 0x24, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// rand % m (HI, the remainder).
fn rand_mod(fx: &mut Fx, m: i32) -> i32 {
    let v = fx.rand();
    fx.div(v, m).1
}

/// 0x1E00BC0(D, task): a smoke ring. First step: size +0x38 = 70, grey, a
/// random angle +0x3A, radius growth +0xE 7..11, quad 50 x 10 (+0xC /
/// +0xA), table row +8 0..24, life +6 45..54. Fades in by 0x30 (to 0x90)
/// for its life, then out by 9; z moves by (target z - self z) / 50 (/ 40
/// after 10 updates); every fifth update 1 down; on odd ages the cell
/// frame +0x34 counts to 5; radius += growth; the quad grows by 4 (its
/// height by 8 after 10 updates while below the row +8); three random
/// angles.
fn smoke(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x10, SMOKE_RA), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x46);
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x30, 0);
        let v = rand_mod(fx, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rand_mod(fx, 5);
        fx.mem.set16(t + 0xE, v + 7);
        fx.mem.set16(t + 0xC, 0x32);
        fx.mem.set16(t + 0xA, 0xA);
        let v = rand_mod(fx, 0x19);
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, 0xA);
        fx.mem.set16(t + 6, v + 0x2D);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < 0x61 {
            fx.mem.write_u8(t + 0x2B, a + 0x30);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x2B, a - 9);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x20);
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x30);
    let dz = ee::sub(fx.mem.f(sp + 0x28), fx.mem.f(sp + 0x38));
    fx.mem.setf(sp + 0x28, dz);
    let q = if fx.mem.i16(t + 0x32) < 0xA { 50.0 } else { 40.0 };
    let dz = ee::div(fx.mem.f(sp + 0x28), q);
    fx.mem.setf(sp + 0x28, dz);
    let z = ee::add(ee::from_i32(fx.mem.i16(t + 4)), dz);
    fx.mem.set16(t + 4, ee::fixsfsi(z));
    let age = fx.mem.i16(t + 0x32);
    if fx.div(age, 5).1 == 1 {
        let y = fx.mem.i16(t + 2) - 1;
        fx.mem.set16(t + 2, y);
    }
    if fx.mem.i16(t + 0x32) % 2 == 1 && fx.mem.i16(t + 0x34) < 5 {
        let c = fx.mem.i16(t + 0x34) + 1;
        fx.mem.set16(t + 0x34, c);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let w = fx.mem.i16(t + 0xC) + 4;
        fx.mem.set16(t + 0xC, w);
        let h = fx.mem.i16(t + 0xA) + 4;
        fx.mem.set16(t + 0xA, h);
        if fx.mem.i16(t + 0x32) >= 0xB && fx.mem.i16(t + 0xA) < fx.mem.i16(t + 8) {
            let h = fx.mem.i16(t + 0xA) + 4;
            fx.mem.set16(t + 0xA, h);
        }
    }
    for o in [0x10u32, 0x14, 0x18] {
        let v = rand_mod(fx, 0x168);
        fx.mem.set32(t + o, v);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x50);
}

/// 0x1E00EF0(D, task): a core: size +0x38 = 60, white (+0x2C dark red,
/// alpha 0), a random angle; fades in by 0x40 below 0x80 (out by 0x30 on
/// updates 282..308; after 330 out by 20 until gone); follows the
/// attacker's node 0x67 (+0x30 = 0) or 0x68 into D+0x20 + 16 x +0x30.
fn core(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, CORE_RA), (0x10, 0), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x3C);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x2C, 0x60, 0x10, 0x10);
        fx.mem.set16(t + 0xE, 0);
        let v = rand_mod(fx, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    let c = fx.mem.i16(COUNTER);
    let a = fx.mem.read_u8(t + 0x2B);
    if c < 0x14A {
        if !(0x11A..0x135).contains(&c) {
            if a < 0x80 {
                fx.mem.write_u8(t + 0x2B, a + 0x40);
            }
        } else if a >= 0x31 {
            fx.mem.write_u8(t + 0x2B, a - 0x30);
        } else {
            fx.mem.write_u8(t + 0x2B, 0);
        }
    } else if a >= 0x21 {
        fx.mem.write_u8(t + 0x2B, a - 0x14);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    let at = if fx.mem.i16(t + 0x30) == 0 { 0x3998 } else { 0x399A };
    let h = fx.self_handle();
    let slot = fx.mem.i16(D + at);
    fx.node_position(h, slot, sp + 0x30);
    let p = D + 0x20 + 16 * fx.mem.i16(t + 0x30) as u32;
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x30 + 4 * k);
        fx.mem.write_u32(p + 4 * k, w);
    }
    let n = fx.mem.i16(t + 0xE) + 1;
    fx.mem.set16(t + 0xE, n);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x50);
}

/// 0x1E010B0(D, task): a flame: 13 x 30, reddish (both colours), +0xE =
/// (angle +0x3A != 0); fades in by 0x40 for 7 updates (both alphas), then
/// out by 13; grows 7 a update in height; turns by 10.
fn flame(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, FLAME_RA), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 6);
        l2::rgb(fx, t + 0x28, 0x80, 0x30, 0x30);
        l2::rgb(fx, t + 0x2C, 0x80, 0x30, 0x30);
        fx.mem.set16(t + 8, 0xD);
        fx.mem.set16(t + 0xA, 0x1E);
        let e = i32::from(fx.mem.i16(t + 0x3A) != 0);
        fx.mem.set16(t + 0xE, e);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if fx.mem.i16(t + 0x32) < 7 {
        for o in [0x2Bu32, 0x2F] {
            let a = fx.mem.read_u8(t + o);
            if a < 0x80 {
                fx.mem.write_u8(t + o, a + 0x40);
            }
        }
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0xF {
            fx.mem.write_u8(t + 0x2B, a - 0xD);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
        let b = fx.mem.read_u8(t + 0x2F);
        fx.mem.write_u8(t + 0x2F, if b >= 0xF { b - 0xD } else { 0 });
    }
    let h = fx.mem.i16(t + 0xA) + 7;
    fx.mem.set16(t + 0xA, h);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    let a = fx.mem.i16(t + 0x3A) + 0xA;
    fx.mem.set16(t + 0x3A, a);
    fx.leave(0x20);
}

/// The node point D+0x20 + 16 x i16 `t`+0x30 into `out` (xyz).
fn node_point(fx: &mut Fx, t: u32, out: u32) {
    let p = D + 0x20 + 16 * fx.mem.i16(t + 0x30) as u32;
    for k in 0..3 {
        let w = fx.mem.read_u32(p + 4 * k);
        fx.mem.write_u32(out + 4 * k, w);
    }
}

/// 0x1E01830(D, task, d, 0): one textured quad (9 values, FST, blend 2)
/// of size i16 +0x38 facing the camera at the node point +0x30 (scale
/// 0.1) into the caller's descriptor `d`, coloured +0x28, UVs from D+0x94
/// (uv_rect4 mode 0), texture D+0x88; the far Z at key 0 [F].
fn core_quad(fx: &mut Fx, t: u32, d: u32) {
    let sp = fx.enter(0x110);
    desc_uv4(fx, d);
    fx.desc_alpha(d, blend_alpha(2));
    fx.mem.set32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    rgbaq_at(fx, sp + 0x108, t + 0x28, d + 0x10);
    uv::uv_rect4(fx, d, D + 0x94, 0);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0x90 + 4 * k, 0x3DCC_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0xF0);
    node_point(fx, t, sp + 0x80);
    l2::xform_vp(fx, sp + 0xB0, sp + 0x90, sp + 0xF0, sp + 0x80);
    let s = ee::from_i32(fx.mem.i16(t + 0x38));
    l2::quad_xy(fx, sp + 0x40, s, s);
    if l2::project(fx, sp + 0x40, SPR, sp + 0xB0, d, sp + 0xA0) {
        l2::far_z(fx, &[d + 0x50, d + 0x40, d + 0x30, d + 0x20]);
        let ot = fx.mem.read_u32(D + 0x80);
        fx.submit(d, ot, 0);
    }
    fx.leave(0x110);
}

/// 0x1E01A70(D, task, d, 0): four textured quads (strip3, FST, additive;
/// 0x109640's layouts) of size i16 +8 / +0xA turned about z by i16 +0x3A,
/// facing the camera at the node point +0x30 (scale 0.05) into the
/// caller's descriptor `d`; coloured +0x2C x3 / +0x28, UVs from D+0x94 in
/// D+0xBE's orientation, texture D+0x88; each at its far Z, key 0 [F].
fn flame_quads(fx: &mut Fx, t: u32, d: u32) {
    let sp = fx.enter(0x130);
    l2::desc_strip3(fx, d);
    rgbaq_at(fx, sp + 0x110, t + 0x2C, d + 0x10);
    rgbaq_at(fx, sp + 0x118, t + 0x2C, d + 0x28);
    rgbaq_at(fx, sp + 0x120, t + 0x2C, d + 0x40);
    rgbaq_at(fx, sp + 0x128, t + 0x28, d + 0x58);
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xA0 + 4 * k, 0x3D4C_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0x100);
    node_point(fx, t, sp + 0x90);
    l2::xform_vp(fx, sp + 0xC0, sp + 0xA0, sp + 0x100, sp + 0x90);
    let a = fx.mem.i16(t + 0x3A);
    fx.mem.set32(sp + 0x108, a);
    fx.mem.set32(sp + 0x104, 0);
    fx.mem.set32(sp + 0x100, 0);
    for k in 0..4 {
        let (w, h) = (fx.mem.i16(t + 8), fx.mem.i16(t + 0xA));
        l2::quad_ints(fx, sp + 0x50, w, h, k);
        l2::rotate4(fx, sp + 0x50, sp + 0x100, 2, D + 0xD8);
        if l2::project(fx, sp + 0x50, SPR, sp + 0xC0, d, sp + 0xB0) {
            l2::far_z(fx, &[d + 0x68, d + 0x50, d + 0x38, d + 0x20]);
            let ot = fx.mem.read_u32(D + 0x80);
            fx.submit(d, ot, 0);
        }
    }
    fx.leave(0x130);
}
