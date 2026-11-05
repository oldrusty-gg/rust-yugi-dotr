//! BMODULE b294 (card 336), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01DE0, data area 0x1ED0000, 0x3A40 bytes) [F]
//!: b350's program 0x10 bytes up (200 tasks of 0x40
//! at D+0x300, alive +0x36, step +0x24 by direct `jal`s; spawn 0x1E02410 =
//! b350's with the colour D+0x3764 and the index D+0x37C2; the node
//! history at 0x1E00080 for node 9 into D+0x3600). The emitter 0x1E00B20:
//! on updates 0x71..0x108 a green smoke ring at the attacker's node 9 (2
//! up) drifting toward the target (0x1E00DB0: a 40th of the gap a step to
//! its age 39, then a 60th with the radius growing 4 x +0xE, x on by the
//! node 9 / 0x3A gap on odd ages); on 0x41 / 0x5F twenty grey chips
//! (0x1E017C0, size 60, swinging 11 sin / cos of a random angle a step) at
//! each of nodes 0x3A / 0x40 / 0x47 / 0x4E (1 down), drawn by 0x1E02530
//! (one textured square turned by the camera's angles, scale 0.1). The
//! steps 0x1E01110 / 0x1E01400 / 0x1E01930 / 0x1E01A10 / 0x1E01CD0 and the
//! entry's object-op case 0x1E01D90 are never spawned (not ported).
//! Returns 1 on update 0x5A, 3 on 0x131, 6 on 0x65..0x12B, 8 once the
//! 320-update timer D+0x3760 has run out.
use crate::b494::camera_angles_saving;
use crate::blib::{quads4, spawn as lib, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 200;
const POOL: u32 = D + 0x300;
const SMOKE: u32 = 0x01E0_0DB0;
const CHIP: u32 = 0x01E0_17C0;
const POS: u32 = D + 0x250;
const TEX: u32 = D + 0x3500;
const TIMER: u32 = D + 0x3760;
const COLOUR: u32 = D + 0x3764;
const OT2: u32 = D + 0x376C;
const W: u32 = D + 0x3794;
const NODES: u32 = D + 0x37A8;
const INDEX: u32 = D + 0x37C2;
const COUNTER: u32 = D + 0x37C6;
const BYTES: u32 = D + 0x37CA;
const SINES: u32 = D + 0x38CC;
const SPR: u32 = 0x7000_0000;

/// The entry 0x1E01DE0 (s0 = 0, s1 = D, s2 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x140);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(OT2, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x140);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3A40);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x37C4, param);
    fx.mem.set32(TIMER, 0x140);
    for o in [0x290u32, 0x294, 0x298] {
        fx.mem.write_u32(D + o, 0);
    }
    for (k, node) in [9, 0x3A, 0x40, 0x47, 0x4E].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    for k in 0xA..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x2E8 + 4 * (k as u32 - 0xA), t);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x2C8, t);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    for k in 0..0x100u32 {
        let v = fx.rand_from(k, 0x01E0_1F30);
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0..8u32 {
        let v = fx.rand_from(k, 0x01E0_1F64);
        fx.mem.set16(D + 0xC0 + 2 * k, v % 4);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    if fx.mem.i16(COUNTER) == 1 {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        fx.node_position(h, slot, sp + 0x90);
        for k in 0..9u32 {
            for c in 0..3 {
                let w = fx.mem.read_u32(sp + 0x90 + 4 * c);
                fx.mem.write_u32(D + 0x3570 + 0x10 * k + 4 * c, w);
            }
        }
    }
    let slot = fx.mem.i16(NODES);
    lib::node_history(fx, D + 0x3600, 0xA, slot);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        // The steps' saves: the entry's s0 = 0, s1 = D.
        let f = fx.mem.read_u32(t + 0x24);
        match f {
            SMOKE => smoke(fx, t),
            CHIP => chip(fx, t),
            _ => debug_assert!(false, "b294: unspawned step {f:#x}"),
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    // 0x1F71A0's frame below the entry's (its saved ra, the entry's s0).
    fx.saved(sp - 0x20, &[(0x10, 0x01E0_2174), (0, 0)]);
    // The entry's s0: the live tasks counted in the draw loop.
    let mut s0 = 0u32;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        s0 += 1;
        let f = fx.mem.read_u32(t + 0x24);
        if f == SMOKE {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x34);
            l2::rect(fx, sp + 0x60, 0x14, sub);
            for k in 0..4 {
                let v = fx.mem.read_u32(sp + 0x60 + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, v);
            }
            fx.mem.set16(D + 0xB8, 7);
            let v = fx.rand_from(s0, 0x01E0_21E8);
            fx.mem.set16(D + 0xBE, v % 4);
            quads4::ring_facing_i(fx, uv::uv_rect4, D, t, 0, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f == CHIP {
            chip_draw(fx, t, sp + 0xA8);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x5A => 1,
        0x131 => 3,
        0x65..=0x12B => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E02410(D, step, 0) (b350's 0x1E01C70 0x10 bytes up): the first free
/// task cleared and set off at D+0x250 (as i16) with the colour D+0x3764
/// (twice), +0x30 = D+0x37C2; `saves` the caller's s0..s4, `ra` the
/// return address.
fn spawn(fx: &mut Fx, step: u32, saves: [u64; 5], ra: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, saves[4]), (0x30, saves[3]), (0x20, saves[2]), (0x10, saves[1]), (0, saves[0])]);
    if let Some(k) = (0..TASKS).find(|&k| fx.mem.i16(POOL + 0x36 + 0x40 * k) == 0) {
        let t = POOL + 0x40 * k;
        fx.bzero(t, 0x40);
        fx.mem.set16(t + 0x36, 1);
        for c in [0x28u32, 0x2C] {
            let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
            l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
            let a = fx.mem.read_u8(COLOUR + 3);
            fx.mem.write_u8(t + c + 3, a);
        }
        l2::to_i16x4(fx, t, POS, 0);
        let a = fx.mem.i16(INDEX);
        fx.mem.set16(t + 0x30, a);
        fx.mem.write_u32(t + 0x20, 0);
        fx.mem.write_u32(t + 0x24, step);
    }
    fx.leave(0x60);
}

/// 0x1E00B20(D, 0) (frame 0x60: ra, the entry's s3 (the runner's, 0
/// [H]) / s2 = the parameter / s1 = D / s0 = 0): the spawn colour (0x80,
/// 0, 0, 0), the update counter D+0x37C6 and the spawns (its own s2 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x60);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x40, 0x01E0_1FA0), (0x30, 0), (0x20, p), (0x10, u64::from(D)), (0, 0)]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let node = |fx: &mut Fx, sp: u32, k: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES + 2 * k);
        fx.node_position(h, s, sp + 0x50);
    };
    let c = fx.mem.i16(COUNTER);
    if (0x71..0x109).contains(&c) {
        node(fx, sp, 0);
        let x = fx.mem.read_u32(sp + 0x50);
        fx.mem.write_u32(POS, x);
        let y = ee::add(2.0, fx.mem.f(sp + 0x54));
        fx.mem.setf(POS + 4, y);
        let z = fx.mem.read_u32(sp + 0x58);
        fx.mem.write_u32(POS + 8, z);
        spawn(fx, SMOKE, [0, u64::from(D), u64::from(D), 0, 0], 0x01E0_0BC8);
    }
    let c = fx.mem.i16(COUNTER);
    if c == 0x41 || c == 0x5F {
        for (k, ra) in [(1u32, 0x01E0_0C50u32), (2, 0x01E0_0CB4), (3, 0x01E0_0D18), (4, 0x01E0_0D7C)] {
            node(fx, sp, k);
            let x = fx.mem.read_u32(sp + 0x50);
            fx.mem.write_u32(POS, x);
            let y = ee::sub(fx.mem.f(sp + 0x54), 1.0);
            fx.mem.setf(POS + 4, y);
            let z = fx.mem.read_u32(sp + 0x58);
            fx.mem.write_u32(POS + 8, z);
            for j in 0..0x14u64 {
                spawn(fx, CHIP, [u64::from(sp + 0x54), u64::from(sp + 0x58), u64::from(D), j, 0], ra);
            }
        }
    }
    fx.leave(0x60);
}

/// rand % m (HI) from a step's rand returning to `ra` (s0 = the task).
fn rand_mod(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v, m).1
}

fn odd(v: i32) -> bool {
    v.wrapping_rem(2) == 1
}

/// 0x1E00DB0 (D, t) (frame 0x60: ra, the entry's s1 = D, s0 = 0): a green
/// smoke ring (size 70 growing by 1, (0x20, 0x70, 0x20), radius 0 growing
/// by +0xE 5..9, row 0..24, life 45..54, +0xC the x gap of nodes 9 / 0x3A);
/// in by 0x30 below 0x61, out by 9; z closes on the target by a 40th of
/// the gap to its age 39, then by a 60th with the radius up 3 x +0xE more;
/// on odd ages x on by +0xC and the cell frame up to 5; random angles.
fn smoke(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x20, 0x01E0_203C), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x46);
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, 0x20, 0x70, 0x20);
        fx.mem.set16(t + 0x30, 0);
        let v = rand_mod(fx, t, 0x01E0_0DFC, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rand_mod(fx, t, 0x01E0_0E18, 5);
        fx.mem.set16(t + 0xE, v + 5);
        let v = rand_mod(fx, t, 0x01E0_0E38, 0x19);
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, t, 0x01E0_0E54, 0xA);
        fx.mem.set16(t + 6, v + 0x2D);
        let (a, b) = (sp + 0x30, sp + 0x40);
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES);
        fx.node_position(h, s, a);
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES + 2);
        fx.node_position(h, s, b);
        let v = ee::sub(fx.mem.f(b), fx.mem.f(a));
        fx.fix_residue(v, 0x01E0_0EB0);
        fx.mem.set16(t + 0xC, ee::fixsfsi(v));
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < 0x61 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x30));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x2B, a - 9);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x30);
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x40);
    let d = ee::sub(fx.mem.f(sp + 0x38), fx.mem.f(sp + 0x48));
    fx.mem.setf(sp + 0x38, d);
    let early = fx.mem.i16(t + 0x32) < 0x28;
    let d = ee::div(d, if early { 40.0 } else { 60.0 });
    fx.mem.setf(sp + 0x38, d);
    let z = ee::add(ee::from_i32(fx.mem.i16(t + 4)), d);
    fx.fix_residue(z, if early { 0x01E0_0F8C } else { 0x01E0_0FC8 });
    fx.mem.set16(t + 4, ee::fixsfsi(z));
    if !early {
        let e = fx.mem.i16(t + 0xE);
        let r = fx.mem.i16(t + 0x30) + 3 * e;
        fx.mem.set16(t + 0x30, r);
    }
    if odd(fx.mem.i16(t + 0x32)) {
        let x = fx.mem.i16(t) + fx.mem.i16(t + 0xC);
        fx.mem.set16(t, x);
    }
    if odd(fx.mem.i16(t + 0x32)) && fx.mem.i16(t + 0x34) < 5 {
        let f = fx.mem.i16(t + 0x34) + 1;
        fx.mem.set16(t + 0x34, f);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let s = fx.mem.i16(t + 0x38) + 1;
        fx.mem.set16(t + 0x38, s);
    }
    for (o, ra) in [(0x10u32, 0x01E0_1090u32), (0x14, 0x01E0_10AC), (0x18, 0x01E0_10C8)] {
        let v = rand_mod(fx, t, ra, 0x168);
        fx.mem.set32(t + o, v);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x60);
}

/// 0x1E017C0 (D, t) (frame 0x30: ra, the entry's s1 = D, s0 = 0): a grey
/// chip (size 60, cell row 0..6, (0x60, 0x60, 0x60)), the drift +8 / +0xC
/// swinging by 11 sin / cos of the random angle +0x3A a step; alpha 0x80
/// for 7 updates, then down by 0xD.
fn chip(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_20A0), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x3C);
        let v = rand_mod(fx, t, 0x01E0_17F0, 7);
        fx.mem.set16(t + 0x34, v);
        l2::rgb(fx, t + 0x28, 0x60, 0x60, 0x60);
        let v = rand_mod(fx, t, 0x01E0_181C, 0x168);
        fx.mem.set16(t + 0x30, v);
        let v = rand_mod(fx, t, 0x01E0_1838, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 7 {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0xF {
            fx.mem.write_u8(t + 0x2B, a - 0xD);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    for (o, cos, ra) in [(8u32, 0, 0x01E0_18C8u32), (0xC, 1, 0x01E0_1900)] {
        let a = fx.mem.i16(t + 0x3A);
        let s = l2::sin_deg(fx, SINES, a, cos);
        let v = ee::add(ee::from_i32(fx.mem.i16(t + o)), ee::mul(11.0, s));
        fx.fix_residue(v, ra);
        fx.mem.set16(t + o, ee::fixsfsi(v));
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x30);
}

/// 0x1E02530(D, t, d, 0): the chip as one textured square (9 values, FST,
/// ALPHA 0x48, flags 0x824) of the i16 size +0x38: cell 0x14 frame +0x34
/// (UVs by uv_rect4), colour +0x28, texture D+0x3500, at the task's i16
/// point plus its i16 drift +8 / +0xA / +0xC over 10, scale 0.1, turned by
/// the camera's angles; keyed by its mean w (D+0x3794) into D+0x376C.
fn chip_draw(fx: &mut Fx, t: u32, d: u32) {
    let sp = fx.enter(0x110);
    let sub = fx.mem.i16(t + 0x34);
    l2::rect(fx, sp + 0xA0, 0x14, sub);
    fx.mem.write_u16(d + 2, 0x24);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, 0x4_2424_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u16(d + 2, 0x824);
    fx.desc_alpha(d, 0x80_0000_0048);
    fx.mem.write_u32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    l2::rgbaq(fx, sp + 0x108, t + 0x28);
    for k in 0..2 {
        let w = fx.mem.read_u32(sp + 0x108 + 4 * k);
        fx.mem.write_u32(d + 0x10 + 4 * k, w);
    }
    uv::uv_rect4(fx, d, sp + 0xA0, 0);
    let tex = fx.mem.read_u32(TEX);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0x90 + 4 * k, 0x3DCC_CCCD);
    }
    camera_angles_saving(fx, sp + 0xF0, u64::from(d), u64::from(t), 0x01E0_2624);
    for k in 0..3 {
        let p = ee::from_i32(fx.mem.i16(t + 2 * k));
        let o = ee::div(ee::from_i32(fx.mem.i16(t + 8 + 2 * k)), 10.0);
        fx.mem.setf(sp + 0x80 + 4 * k, ee::add(p, o));
    }
    l2::xform_vp(fx, sp + 0xB0, sp + 0x90, sp + 0xF0, sp + 0x80);
    let v = ee::from_i32(fx.mem.i16(t + 0x38));
    l2::quad_xy(fx, sp + 0x40, v, v);
    if l2::project(fx, sp + 0x40, SPR, sp + 0xB0, d, W) {
        let w = fx.mem.f(W);
        fx.fix_residue(w, 0x01E0_26F8);
        let key = ee::fixunssfsi(w) as i32;
        let ot = fx.mem.read_u32(OT2);
        fx.submit(d, ot, key);
    }
    fx.leave(0x110);
}
