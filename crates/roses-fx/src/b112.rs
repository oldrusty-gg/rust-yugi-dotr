//! BMODULE b112 (card 234), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E026C0, data area 0x1ED0000, 0x3470 bytes) [F]
//!: b403's (card 250) layout and globals word for
//! word (180 tasks of 0x40 at D+0x300, spawn 0x1E038C0 = b403's 0x1D90,
//! timer D+0x31A0 = 145, OT D+0x31AC, nodes D+0x31D8.. (the attacker's
//! 0x15 (clear), 0x4D, 0x16, 0x17, 0x18, 0x1B, 0x1A, 0x19), counter
//! D+0x31F6, bytes D+0x31F8, sines D+0x32F8, textures (4, 7..0xF)), with
//! b403's fan 0x13B0, drift 0x18C0, ring 0x1D20, camera 0x2420 and draws
//! 0x39F0 / 0x4260 [F fdiff: word-equal but for addresses].
//!
//! The emitter 0x1E01000: on 0x5C at node 0x15 fifty drift sprites, fifty
//! rings, the disc c7cfb5b8be8e (`quads4::band18`) and the camera, on 0x5C
//! / 0x61 / 0x67 / 0x6C a fan; 0x17..0x65 two sparks 0x1F70 a update at
//! node 0x4D (jittered, pushed by the facing's cosine times 2..11; drawn by
//! 0x1E04480: twelve quads on a ring of the bytes D+0x31F8, the z gap of
//! the points D+0x30F0 / 0x3110); 2..0x65 a beam 0x21A0 (0x1E00850 a
//! quarter quad of +8 / +0xA turned by +0x3A at four points along each of
//! the attacker's arms 0x16 -> 0x17 -> 0x18 and 0x1B -> 0x1A -> 0x19,
//! +0x30 / 5 of the way; keyed by mean w, at the far Z on most updates) and
//! a band 0x22D0 (`quads4::band18` at the same points). From update 0x15
//! D+0x290 / 0x298 rise by 0.01 while (double) < 0.5 (b089's soft-double
//! compare), D+0x294 = 0.1. Returns 1 on update 0x52, 4 on 0x8C, 2 on
//! 0x7D, 6 on 0x5D..0x77, 8 once the timer has run out.
//!
//! The steps 0x1E01510, 0x17B0, 0x1A60, 0x25B0 and the object op 0x2670
//! are tested by the entry but never spawned [F: the emitter names 0x13B0
//! / 0x18C0 / 0x1D20 / 0x1E90 / 0x1F70 / 0x21A0 / 0x22D0 / 0x2420 only];
//! not ported (nor their draws 0x3C50 / 0x3F90). No 0x1F8100.
use crate::b076::node_position_saving;
use crate::b089::dcmp;
use crate::b148::{facing_of, rmod};
use crate::b403;
use crate::blib::{billboard, quads4, spawn, uv};
use crate::bus::Bus;
use crate::c061::{desc_uv4, rgbaq_at, to_degrees_neg};
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 180;
const POOL: u32 = D + 0x300;
const TIMER: u32 = D + 0x31A0;
const COLOUR: u32 = D + 0x31A4;
const OT2: u32 = D + 0x31AC;
const NODES: u32 = D + 0x31D8;
const KIND: u32 = D + 0x31F2;
const COUNTER: u32 = D + 0x31F6;
const TABLE: u32 = D + 0x31F8;
const SINES: u32 = D + 0x32F8;
const TEX: u32 = D + 0x2FC;
const SPR: u32 = 0x7000_0000;

const FAN: u32 = 0x01E0_13B0;
const DRIFT: u32 = 0x01E0_18C0;
const RING: u32 = 0x01E0_1D20;
const DISC: u32 = 0x01E0_1E90;
const SPARK: u32 = 0x01E0_1F70;
const BEAM: u32 = 0x01E0_21A0;
const BAND: u32 = 0x01E0_22D0;
const CAMERA: u32 = 0x01E0_2420;

/// The entry 0x1E026C0 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x260);
    fx.mem.write_u32(sp + 0xA0, 0);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(OT2, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp) };
    fx.leave(0x260);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3470);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x31F4, param);
    fx.mem.set32(TIMER, 0x91);
    for (k, n) in [0x15, 0x4D, 0x16, 0x17, 0x18, 0x1B, 0x1A, 0x19].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    for k in 7..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x2DC + 4 * (k as u32 - 7), t);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2828, 0x64);
        fx.mem.set8(TABLE + k, v - 0x32);
    }
    0
}

/// A step's frame below the entry's sp: the entry's s0 = D / s1 = the
/// slot (the first `n`) and the return address `ra` above them.
fn frame(fx: &mut Fx, low: u32, size: u32, n: u32, t: u32, ra: u32) {
    let base = low - size;
    let vals = [u64::from(D), u64::from(t - 0x300)];
    for i in 0..n {
        fx.saved(base + 0x10 * i, &[(0, vals[i as usize])]);
    }
    fx.saved(base, &[(0x10 * n, u64::from(ra))]);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    let low = fx.sp;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            FAN => {
                b403::fan(fx, FAN, t);
                frame(fx, low, 0x30, 2, t, 0x01E0_2898);
            }
            DRIFT => {
                b403::drift(fx, DRIFT, t);
                frame(fx, low, 0x30, 2, t, 0x01E0_28F8);
            }
            RING => {
                b403::ring(fx, RING, t);
                frame(fx, low, 0x20, 1, t, 0x01E0_2938);
            }
            DISC => {
                let c = spawn::disc("c7cfb5b8be8e").expect("constants");
                spawn::disc_step_with(fx, c, DISC, t);
                frame(fx, low, 0x20, 1, t, 0x01E0_2958);
            }
            SPARK => {
                spark(fx, t);
                frame(fx, low, 0x20, 1, t, 0x01E0_2978);
            }
            BEAM => {
                beam(fx, t);
                frame(fx, low, 0x20, 1, t, 0x01E0_2998);
            }
            BAND => {
                band(fx, t);
                frame(fx, low, 0x20, 1, t, 0x01E0_29B8);
            }
            CAMERA => {
                b403::camera(fx, CAMERA, t);
                frame(fx, low, 0x30, 2, t, 0x01E0_29D8);
            }
            f => {
                debug_assert!(false, "b112: unported task step {f:#x}");
            }
        }
    }
    let d = sp + 0x1C8;
    let pos = sp + 0x150;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        let slot = t - 0x300;
        fx.mem.write_u32(sp + 0x130, k);
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let n = fx.mem.read_u32(sp + 0xA0) + 1;
        fx.mem.write_u32(sp + 0xA0, n);
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x24);
        let set_rect = |fx: &mut Fx, local: u32, id: i32, sub: Option<i32>| {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            let sub = sub.unwrap_or_else(|| fx.mem.i16(t + 0x34));
            l2::rect(fx, local, id, sub);
            for k in 0..4 {
                let v = fx.mem.read_u32(local + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, v);
            }
        };
        if f(fx) == FAN {
            b403::fan_draw(fx, t, d, 0);
            b403::fan_draw(fx, t, d, 1);
        }
        if f(fx) == DRIFT {
            b403::drift_draw(fx, 0x01E0_4260, t, d, 0);
            b403::drift_draw(fx, 0x01E0_4260, t, d, 1);
        }
        if f(fx) == RING {
            set_rect(fx, sp + 0x180, 0x9B, None);
            fx.mem.set16(D + 0xB8, 0xE);
            billboard::ring_t(fx, uv::uv_rect4, D, t, d, TABLE);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == DISC {
            set_rect(fx, sp + 0x190, 0x9B, None);
            for c in 0..3u32 {
                let v = ee::from_i32(fx.mem.i16(t + 2 * c));
                fx.mem.setf(pos + 4 * c, v);
            }
            quads4::band18(fx, D, t, 1, pos);
        }
        if f(fx) == SPARK {
            set_rect(fx, sp + 0x1A0, 0x9E, None);
            fx.mem.write_u32(D + 0xD4, D + 0x30F0);
            spark_draw(fx, t, d, TABLE);
        }
        let c = fx.mem.i16(COUNTER);
        let mid = (0x33..0x4E).contains(&c);
        let early = (0x18..0x1F).contains(&c);
        // (arms' node offsets, the 0x1F6830 return addresses).
        let arms: [(u32, u32, [u32; 2]); 4] = [
            (4, 6, [0x01E0_2DF0, 0x01E0_2E08]),
            (6, 8, [0x01E0_2F10, 0x01E0_2F28]),
            (0xA, 0xC, [0x01E0_3068, 0x01E0_3080]),
            (0xC, 0xE, [0x01E0_3180, 0x01E0_3198]),
        ];
        let arms_band: [[u32; 2]; 4] = [[0x01E0_32D0, 0x01E0_32E8], [0x01E0_33F0, 0x01E0_3408], [0x01E0_3548, 0x01E0_3560], [0x01E0_3660, 0x01E0_3678]];
        let along = |fx: &mut Fx, a: u32, b: u32, ra: [u32; 2]| {
            let h = fx.self_handle();
            let s = fx.mem.i16(NODES + a);
            node_position_saving(fx, h, s, sp + 0x160, u64::from(D), Some(u64::from(slot)), ra[0]);
            let h = fx.self_handle();
            let s = fx.mem.i16(NODES + b);
            node_position_saving(fx, h, s, sp + 0x170, u64::from(D), Some(u64::from(slot)), ra[1]);
            for c in 0..3u32 {
                let g = ee::div(ee::sub(fx.mem.f(sp + 0x170 + 4 * c), fx.mem.f(sp + 0x160 + 4 * c)), 5.0);
                fx.mem.setf(sp + 0x140 + 4 * c, g);
            }
            for c in 0..3u32 {
                let k = ee::from_i32(fx.mem.i16(t + 0x30));
                let v = ee::add(fx.mem.f(sp + 0x160 + 4 * c), ee::mul(fx.mem.f(sp + 0x140 + 4 * c), k));
                fx.mem.setf(pos + 4 * c, v);
            }
        };
        if f(fx) == BEAM {
            set_rect(fx, sp + 0x1B0, 0x9A, Some(1));
            for (i, &(a, b, ra)) in arms.iter().enumerate() {
                fx.mem.write_u32(sp + 0xB0 + 0x10 * i as u32, t);
                along(fx, a, b, ra);
                let far = if i < 2 { !mid } else { !(mid || early) };
                quarters(fx, t, i32::from(far), pos);
            }
        }
        if f(fx) == BAND {
            for (i, &(a, b, _)) in arms.iter().enumerate() {
                fx.mem.write_u32(sp + 0xF0 + 0x10 * i as u32, t);
                along(fx, a, b, arms_band[i]);
                let far = if i < 2 { !mid } else { !(mid || early) };
                quads4::band18(fx, D, t, i32::from(far), pos);
            }
        }
    }
    fx.mem.write_u32(sp + 0x130, TASKS);
    if fx.mem.i16(COUNTER) >= 0x15 {
        let half = 0x3FE0_0000_0000_0000u64;
        let v = fx.mem.f(D + 0x290);
        if dcmp(fx, v, half, D, 0x01E0_3784, 0x01E0_3794, false) {
            let v = ee::add(fx.mem.f(D + 0x290), f32::from_bits(0x3C23_D70A));
            fx.mem.setf(D + 0x290, v);
        }
        fx.mem.write_u32(D + 0x294, 0x3DCC_CCCD);
        let v = fx.mem.f(D + 0x298);
        if dcmp(fx, v, half, D, 0x01E0_37CC, 0x01E0_37DC, false) {
            let v = ee::add(fx.mem.f(D + 0x298), f32::from_bits(0x3C23_D70A));
            fx.mem.setf(D + 0x298, v);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x52 => 1,
        0x8C => 4,
        0x7D => 2,
        0x5D..=0x77 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E01000(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the spawns (frame 0x70: f20 (the runner's 0 [H]), the entry's s0 =
/// D, s1 = param, s2 (the runner's 0 [H]), ra; its s0 the loops' counts,
/// s1 = D, s2 the last spark's push).
fn emit(fx: &mut Fx) {
    let dd = u64::from(D);
    let sp = fx.enter(0x70);
    fx.saved(sp, &[(0x10, dd), (0x20, u64::MAX), (0x30, 0), (0x40, 0x01E0_2864)]);
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let mut s2 = 0u64;
    let spawn = |fx: &mut Fx, step: u32, s0: u64, s2: u64, ra: u32| {
        b403::spawn_regs(fx, step, [s0, dd, s2, 0, 0], ra);
    };
    let at = |fx: &mut Fx| {
        for k in 0..3 {
            let w = fx.mem.read_u32(D + 0x3190 + 4 * k);
            fx.mem.write_u32(D + 0x250 + 4 * k, w);
        }
    };
    if c(fx) == 0x5C {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        node_position_saving(fx, h, slot, D + 0x3190, dd, Some(dd), 0x01E0_1064);
    }
    if c(fx) == 0x5C {
        at(fx);
        for i in 0..0x32u64 {
            spawn(fx, DRIFT, i, s2, 0x01E0_10A4);
        }
    }
    if [0x5C, 0x61, 0x67, 0x6C].contains(&c(fx)) {
        at(fx);
        fx.mem.set16(KIND, 0);
        spawn(fx, FAN, 0, s2, 0x01E0_1120);
    }
    if c(fx) == 0x5C {
        at(fx);
        for i in 0..0x32u64 {
            spawn(fx, RING, i, s2, 0x01E0_1170);
        }
        spawn(fx, DISC, 0x32, s2, 0x01E0_1194);
        spawn(fx, CAMERA, 0x32, s2, 0x01E0_11A8);
    }
    if (0x17..0x66).contains(&c(fx)) {
        for i in 0..2u64 {
            let f = facing_of(fx, sp + 0x60, 0x01E0_1204);
            let cs = l2::sin_deg(fx, SINES, (f - 0xB4) as i16 as i32, 1);
            let r = rmod(fx, i as u32, 0x01E0_1224, 0xA) + 2;
            let p = ee::mul(cs, ee::from_i32(r));
            fx.fix_residue(p, 0x01E0_1250);
            let push = ee::fixsfsi(p);
            s2 = push as i64 as u64;
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + 2);
            node_position_saving(fx, h, slot, sp + 0x50, i, Some(dd), 0x01E0_1268);
            let x = rmod(fx, i as u32, 0x01E0_1270, 0x14);
            let v = ee::sub(ee::add(fx.mem.f(sp + 0x50), ee::from_i32(x)), 10.0);
            fx.mem.setf(D + 0x250, v);
            let y = rmod(fx, i as u32, 0x01E0_12A4, 0xC);
            let v = ee::sub(ee::add(fx.mem.f(sp + 0x54), ee::from_i32(y)), 3.0);
            fx.mem.setf(D + 0x254, v);
            let v = ee::add(fx.mem.f(sp + 0x58), ee::from_i32(push));
            fx.mem.setf(D + 0x258, v);
            spawn(fx, SPARK, i, s2, 0x01E0_12FC);
        }
    }
    if (2..0x66).contains(&c(fx)) {
        let a = rmod(fx, 0, 0x01E0_1338, 0x168);
        fx.mem.set16(D + 0x31EA, a);
        spawn(fx, BEAM, 0, s2, 0x01E0_135C);
        fx.mem.set16(D + 0x31EA, 0x5A);
        spawn(fx, BAND, 0, s2, 0x01E0_1378);
    }
    fx.leave(0x70);
}

/// 0x1E01F70 (D, task): a spark ring task (size 3..11, cell 0..3, one of
/// (0x80, 0x80, 8) / (0x80, 0x40, 8) / (0x60, 0x40, 0x20), +0x3A rand %
/// 360, +0xE 3..10, row +8 rand % 10, +0xA 2..6, +0xC 2..16, life 5..11):
/// alpha up by 0x30 (below 0x61) for 3 updates, then down by 10; radius up
/// by +0xE.
fn spark(fx: &mut Fx, t: u32) {
    let at = SPARK;
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        let v = rmod(fx, t, at + 0x24, 9);
        fx.mem.set16(t + 0x38, v + 3);
        let v = fx.rand_from(t, at + 0x44);
        fx.mem.set16(t + 0x34, v % 4);
        let k = rmod(fx, t, at + 0x60, 3) as i16;
        match k {
            0 => l2::rgb(fx, t + 0x28, 0x80, 0x80, 8),
            1 => l2::rgb(fx, t + 0x28, 0x80, 0x40, 8),
            2 => l2::rgb(fx, t + 0x28, 0x60, 0x40, 0x20),
            _ => {}
        }
        fx.mem.set16(t + 0x30, 0);
        let v = rmod(fx, t, at + 0xF4, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = fx.rand_from(t, at + 0x110);
        fx.mem.set16(t + 0xE, v % 8 + 3);
        let v = rmod(fx, t, at + 0x130, 0xA);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, at + 0x14C, 5);
        fx.mem.set16(t + 0xA, v + 2);
        let v = rmod(fx, t, at + 0x16C, 0xF);
        fx.mem.set16(t + 0xC, v + 2);
        let v = rmod(fx, t, at + 0x18C, 7);
        fx.mem.set16(t + 6, v + 5);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < 3 {
        if a < 0x61 {
            fx.mem.write_u8(t + 0x2B, a + 0x30);
        }
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x2B, a - 0xA);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x30, r);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x20);
}

/// Alpha byte `o` up by 0x40 below 0x40 on the first update, then down by
/// 0xD (`kill`: the task over, else the byte 0, below 0xF).
fn in_out(fx: &mut Fx, t: u32, o: u32, kill: bool) {
    let a = fx.mem.read_u8(t + o);
    if fx.mem.i16(t + 0x32) <= 0 {
        if a < 0x40 {
            fx.mem.write_u8(t + o, a + 0x40);
        }
    } else if a >= 0xF {
        fx.mem.write_u8(t + o, a - 0xD);
    } else if kill {
        fx.mem.set16(t + 0x36, 0);
    } else {
        fx.mem.write_u8(t + o, 0);
    }
}

/// 0x1E021A0 (D, task): the beam's quarter quads (size 6, (0x30, 0x30,
/// 0x30) / (0x50, 0x50, 0), +8 / +0xA = 6 / 100, +0x30 rand % 5 of the arm,
/// rim alpha 0): both alphas in by 0x40 on the first update, then out by
/// 0xD; +0x3A turning by 4.
fn beam(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 6);
        l2::rgb(fx, t + 0x28, 0x30, 0x30, 0x30);
        l2::rgb(fx, t + 0x2C, 0x50, 0x50, 0);
        fx.mem.set16(t + 8, 6);
        fx.mem.set16(t + 0xA, 0x64);
        let v = rmod(fx, t, BEAM + 0x60, 5);
        fx.mem.set16(t + 0x30, v);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    in_out(fx, t, 0x2B, true);
    in_out(fx, t, 0x2F, false);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    let a = fx.mem.i16(t + 0x3A) + 4;
    fx.mem.set16(t + 0x3A, a);
    fx.leave(0x20);
}

/// 0x1E022D0 (D, task): the band (size 30, (8, 8, 8) / (0x50, 0x50, 0),
/// +0x30 rand % 5 of the arm, +0x3A rand % 360): alpha in by 0x40 on the
/// first update, then out by 0x13 (over below 0x15); grey (0x30, 0x30,
/// 0x30) on odd ages, (0x80, 0x80, 0x80) on even.
fn band(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x1E);
        l2::rgb(fx, t + 0x28, 8, 8, 8);
        l2::rgb(fx, t + 0x2C, 0x50, 0x50, 0);
        let v = rmod(fx, t, BAND + 0x54, 5);
        fx.mem.set16(t + 0x30, v);
        let v = rmod(fx, t, BAND + 0x70, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) <= 0 {
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x40));
    } else if a >= 0x15 {
        fx.mem.write_u8(t + 0x2B, a - 0x13);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        l2::rgb(fx, t + 0x28, 0x30, 0x30, 0x30);
    } else {
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x20);
}

/// 0x1E00850 (D, task, far, pos): `quads4::quarters_far` into its own
/// strip3 (frame 0x1E0, the descriptor at sp + 0x130): the quarters of
/// the half extents (+8, +0xA) turned about z by +0x3A at `pos` facing the
/// camera (scale 0.05); `far` 0 keyed by mean w, else at the far Z, key 0.
fn quarters(fx: &mut Fx, t: u32, far: i32, pos: u32) {
    let sp = fx.enter(0x1E0);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0x60 + 4 * k, w);
    }
    let d = sp + 0x130;
    l2::desc_strip3(fx, d);
    for (local, c, to) in [(0x1C0u32, 0x2Cu32, 0x10u32), (0x1C8, 0x2C, 0x28), (0x1D0, 0x2C, 0x40), (0x1D8, 0x28, 0x58)] {
        rgbaq_at(fx, sp + local, t + c, d + to);
    }
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xC0 + 4 * k, 0x3D4C_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0x120);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x60 + 4 * k);
        fx.mem.write_u32(sp + 0xB0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0xE0, sp + 0xC0, sp + 0x120, sp + 0xB0);
    let z = fx.mem.i16(t + 0x3A);
    fx.mem.set32(sp + 0x128, z);
    fx.mem.set32(sp + 0x124, 0);
    fx.mem.set32(sp + 0x120, 0);
    let quad = sp + 0x70;
    for q in 0..4 {
        let (a, b) = (fx.mem.i16(t + 8), fx.mem.i16(t + 0xA));
        l2::quad_ints(fx, quad, a, b, q);
        l2::rotate4(fx, quad, sp + 0x120, 2, D + 0xD8);
        if far == 0 {
            if l2::project(fx, quad, SPR, sp + 0xE0, d, sp + 0xD0) {
                let key = ee::fixunssfsi(fx.mem.f(sp + 0xD0)) as i32;
                let ot = fx.mem.read_u32(D + 0x80);
                fx.submit(d, ot, key);
            }
        } else if far == 1 && l2::project(fx, quad, SPR, sp + 0xE0, d, sp + 0xD0) {
            l2::far_z_desc(fx, d, 0);
            let ot = fx.mem.read_u32(D + 0x80);
            fx.submit(d, ot, 0);
        }
    }
    fx.leave(0x1E0);
}

/// 0x1E04480 (D, task, desc, table): twelve textured squares (9 values,
/// FST, ALPHA 0x48) of the i16 size +0x38 facing the camera's yaw (-y /
/// 180 degrees) at the task's i16 point (scale 0.1), each pushed out by
/// the bytes `table` [k + +8] / 20 and [k + +8 + 0x64] / 40 times +0x30 + k
/// / 2 in x / y, in z by (u8 [k + +8 + 0xB4] / 20, at least 1) times the
/// negated z gap of the points at [D+0xD4] (+0x28 less +8) / 20 times it,
/// and up by +0xC cos +0x30; each keyed by its mean w into D+0x80.
fn spark_draw(fx: &mut Fx, t: u32, d: u32, table: u32) {
    use ee::{div, mul};
    let sp = fx.enter(0x170);
    let g = fx.mem.read_u32(D + 0xD4);
    desc_uv4(fx, d);
    fx.desc_alpha(d, 0x80_0000_0048);
    fx.mem.write_u32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    rgbaq_at(fx, sp + 0x168, t + 0x28, d + 0x10);
    uv::uv_rect4(fx, d, D + 0x94, 0);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xF0 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..3 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(sp + 0xE0 + 4 * k, v);
    }
    l2::xform_vp(fx, sp + 0x110, sp + 0xF0, 0, sp + 0xE0);
    crate::b494::camera_angles_saving(fx, sp + 0x150, u64::from(d), u64::from(t), 0x01E0_45D0);
    to_degrees_neg(fx, sp + 0x154);
    let q = sp + 0xA0;
    for k in 0..0xCi32 {
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, q, s, s);
        l2::rotate4(fx, q, sp + 0x150, 0, D + 0xD8);
        let row = table.wrapping_add(k as u32);
        let r = |fx: &mut Fx| ee::from_i32(fx.mem.i16(t + 0x30) + (k >> 1));
        let i = fx.mem.i16(t + 8) as u32;
        let b = ee::from_i32(fx.mem.i8(row.wrapping_add(i)));
        let f = mul(div(b, 20.0), r(fx));
        l2::add_column(fx, q, 0, f);
        let i = fx.mem.i16(t + 8) as u32;
        let b = ee::from_i32(fx.mem.i8(row.wrapping_add(i).wrapping_add(0x64)));
        let f = mul(div(b, 40.0), r(fx));
        l2::add_column(fx, q, 1, f);
        let i = fx.mem.i16(t + 8) as u32;
        let u = (fx.mem.read_u8(row.wrapping_add(i).wrapping_add(0xB4)) / 20).max(1);
        let gap = ee::neg(ee::sub(fx.mem.f(g + 0x28), fx.mem.f(g + 8)));
        let f = div(mul(ee::from_i32(i32::from(u)), gap), 20.0);
        let f = mul(f, r(fx));
        l2::add_column(fx, q, 2, f);
        let a = fx.mem.i16(t + 0x30);
        let c = l2::sin_deg(fx, D + 0xD8, a, 1);
        let f = mul(ee::from_i32(fx.mem.i16(t + 0xC)), c);
        l2::add_column(fx, q, 1, f);
        if l2::project(fx, q, SPR, sp + 0x110, d, sp + 0x100) {
            let key = ee::fixunssfsi(fx.mem.f(sp + 0x100)) as i32;
            let ot = fx.mem.read_u32(D + 0x80);
            fx.submit(d, ot, key);
        }
    }
    fx.leave(0x170);
}
