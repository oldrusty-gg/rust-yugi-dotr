//! BMODULE b098 (card 108), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01EF0, data area 0x1ED0000) [F]: b585's layout (400 tasks of 0x70 bytes at D+0x2A0, alive +0x58,
//! step +0x44 through `jalr`; spawned by 0x1E02460, b377's spawn with the
//! slot at D+0xB2B0 (+0x50 / +0x52) and the angle D+0xB2A2 (+0x5A)) drawn
//! with the second library (`billboard::ring_facing`,
//! `quads4::sprite_quads`), plus b180's unit flash (0x1E02610: the
//! attacker drawn into the target in a packet layer, the flash tasks
//! 0x1E01C30 stepped). Update 1: a pale yellow flash (0x1E01C30), a frame
//! flash (0x1E01A30) and four red light columns (0x1E01850) riding the
//! attacker's node 0xB6, two growing 25 / 22.5 a update; 116..269 a dust
//! ring a update (three from 251) at the attacker's node 0xB7 (+-5 in x, 3
//! up) drifting toward the target (0x1E014D0, `ring_facing`). Returns 1 on
//! update 160, 4 on 375, 2 on 365, 6 on 170..360, 8 once the 380-update
//! timer D+0xB210 has run out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
const POS: u32 = D + 0x250;
const DUST: u32 = 0x01E0_14D0;
const COLUMN: u32 = 0x01E0_1850;
const FRAME: u32 = 0x01E0_1A30;
/// The flash tasks, run by the unit flash 0x1E02610.
const FLASH: u32 = 0x01E0_1C30;
/// The return address of the entry's `jalr` into a step (0x1E021D0 + 8).
const TASK_RA: u64 = 0x01E0_21D8;
const TEX: u32 = D + 0xB1F0;
const TIMER: u32 = D + 0xB210;
const COLOUR: u32 = D + 0xB214;
const LAYER: u32 = D + 0xB224;
const FLASH_DESC: u32 = D + 0xB228;
const NODES: u32 = D + 0xB244;
const ANGLE: u32 = D + 0xB2A2;
const SLOT: u32 = D + 0xB2B0;
const FACING: u32 = D + 0xB2B4;
const COUNTER: u32 = D + 0xB2B8;
const BYTES: u32 = D + 0xB2BA;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E01EF0 (s0 = the flash count, s1 = 0 / the live count,
/// s2 = D).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x90);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, D + 0xB4BC);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2B6, param);
    fx.mem.set32(TIMER, 0x17C);
    for (k, node) in [0xB7, 0xB6, 0x35, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB254, r);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x80);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x84)), 180.0);
    fx.fix_residue(y, 0x01E0_20BC);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_20EC, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_213C);
        fx.mem.set16(D + 0xB264 + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    // The entry's s0: the flash tasks counted here (the steps' saved s0).
    let mut flashes = 0u32;
    // a3 as the last call left it, for the unit flash's first 0x207BB0
    // (as b585) [run].
    let mut a3 = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        let frame = match f {
            0 => continue,
            FLASH => {
                flashes += 1;
                continue;
            }
            DUST => 0x50,
            COLUMN => 0x30,
            FRAME => 0x90,
            _ => panic!("b098: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(flashes))]);
        match f {
            DUST => dust(fx, t, s, &mut a3),
            COLUMN => column(fx, t, &mut a3),
            _ => {
                frame_flash(fx, t, s);
                a3 = 0;
            }
        }
        fx.leave(frame);
    }
    let last_xyz2 = |fx: &mut Fx, before: usize| match fx.out.prims.get(before..).and_then(|p| p.last()) {
        Some(p) => {
            let d = p.desc;
            let n = u32::from(fx.mem.read_u16(d));
            fx.mem.read_u32(d + 8 + 8 * n)
        }
        None => 0,
    };
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x44) == DUST {
            cell(fx, t, sp + 0x60, 0x14);
            fx.mem.set16(D + 0xC0, 5);
            tex(fx, TEX);
            fx.mem.set16(D + 0xB8, 7);
            copy3(fx, t + 0x10, D + 0x20);
            fx.mem.set16(D + 0xBE, 0);
            billboard::ring_facing(fx, uv::uv_rect, D, t, 0x20, BYTES);
            // ring_facing leaves a3 = 0 [run].
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == COLUMN {
            tex(fx, D + 0x29C);
            l2::rect(fx, sp + 0x70, 0x9A, 0);
            for k in 0..4 {
                let v = fx.mem.read_u32(sp + 0x70 + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, v);
            }
            fx.mem.set16(D + 0xBE, 0);
            let before = fx.out.prims.len();
            quads4::sprite_quads(fx, D, t, 0, t + 0x10);
            quads4::sprite_quads(fx, D, t, 0, t + 0x10);
            a3 = last_xyz2(fx, before);
        }
    }
    if flashes != 0 {
        unit_flash(fx, a3);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xA0 => 1,
        0x177 => 4,
        0x16D => 2,
        0xAA..=0x168 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

fn tex(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

fn copy3(fx: &mut Fx, from: u32, to: u32) {
    for k in 0..3 {
        let v = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, v);
    }
}

/// The atlas cell `id` frame +0x56 (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32, id: i32) {
    let sub = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// 0x1E02460(D, step, 0): the first free task cleared and set off at
/// D+0x250 (i16 +0, float +0x10) with the colour D+0xB214 (twice), slot
/// D+0xB2B0 (+0x50 / +0x52), angle D+0xB2A2 (+0x5A) (b377's spawn).
fn spawn(fx: &mut Fx, step: u32, saves: &[(u32, u64)]) {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
    if let Some(k) = (0..TASKS).find(|&k| fx.mem.i16(D + 0x2F8 + 0x70 * k) == 0) {
        let t = D + 0x2A0 + 0x70 * k;
        fx.bzero(t, 0x70);
        fx.mem.set16(t + 0x58, 1);
        for o in [0x60u32, 0x64] {
            let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
            l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
            let a = fx.mem.read_u8(COLOUR + 3);
            fx.mem.write_u8(t + o + 3, a);
        }
        l2::to_i16x4(fx, t, POS, 0);
        copy3(fx, POS, t + 0x10);
        let s = fx.mem.i16(SLOT);
        fx.mem.set16(t + 0x50, s);
        let s = fx.mem.i16(SLOT);
        fx.mem.set16(t + 0x52, s);
        let a = fx.mem.i16(ANGLE);
        fx.mem.set16(t + 0x5A, a);
        fx.mem.write_u32(t + 0x40, 0);
        fx.mem.write_u32(t + 0x44, step);
    }
    fx.leave(0x60);
}

/// 0x1E01200(D, 0): the update counter and the emitters (frame 0x20: ra,
/// the entry's s0 = 0).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, 0x01E0_2198), (0, 0)]);
    let p = param as i64 as u64;
    // The spawn's saves: s0 = D, the entry's s1 = 0, s2 = D, s3 = the
    // parameter, s4 the runner's (0 [H]).
    let go = |fx: &mut Fx, f: u32, ra: u32| {
        spawn(fx, f, &[(0x50, u64::from(ra) + 8), (0x40, 0), (0x30, p), (0x20, u64::from(D)), (0x10, 0), (0, u64::from(D))]);
    };
    let near = |fx: &mut Fx, ra: u32| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        node_position_saving(fx, h, slot, POS, u64::from(D), Some(0), ra);
        let v = rmod(fx, D, ra + 8, 0xA) - 5;
        let x = ee::add(fx.mem.f(POS), ee::from_i32(v));
        fx.mem.setf(POS, x);
        let y = ee::add(fx.mem.f(POS + 4), 3.0);
        fx.mem.setf(POS + 4, y);
    };
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    if (0x74..0x10E).contains(&c(fx)) {
        for k in 0..4 {
            let w = fx.mem.read_u32(D + 0xB1D0 + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
        near(fx, 0x01E0_1290);
        go(fx, DUST, 0x01E0_12E0);
        if c(fx) >= 0xFB {
            go(fx, DUST, 0x01E0_130C);
            go(fx, DUST, 0x01E0_1320);
        }
        near(fx, 0x01E0_1348);
    }
    if c(fx) == 1 {
        go(fx, FRAME, 0x01E0_13B0);
    }
    if c(fx) == 1 {
        go(fx, FLASH, 0x01E0_13DC);
    }
    if c(fx) == 1 {
        for (a, s, ra) in [(0, 0, 0x01E0_1420u32), (0x5A, 0, 0x01E0_1450), (0x2D, 1, 0x01E0_1484), (0x87, 1, 0x01E0_14B8)] {
            fx.mem.set16(ANGLE, a);
            fx.mem.set16(SLOT, s);
            go(fx, COLUMN, ra);
        }
    }
    fx.leave(0x20);
}

fn fade_in(fx: &mut Fx, t: u32, at: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + at);
    if a < cap {
        fx.mem.write_u8(t + at, a.wrapping_add(up));
    }
}

/// Byte `at` down by `down` while at least `floor`, else the task ends.
fn fade_out(fx: &mut Fx, t: u32, at: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + at);
    if a >= floor {
        fx.mem.write_u8(t + at, a.wrapping_sub(down));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn addf(fx: &mut Fx, at: u32, by: f32) {
    let v = ee::add(fx.mem.f(at), by);
    fx.mem.setf(at, v);
}

/// 0x1E014D0 (frame 0x50 at `sp`, s0 = the task, s1 = D): a dust ring
/// (size 80 growing 0.5, cell rand >> 4 % 8, (0x80, 0x80, 0x20), angles
/// rand % 360 at +0x5A / +0x5C, orientation 0 / 2, radius 0 growing
/// rand % 20 + 10, height rand % 350 + 100, hold rand % 5 + 3): drifting
/// by (rand % 40 - 20) / 60 a update over ages 11..69, sinking 0.05 over
/// 11..39 and along z by a 80th of the attacker's node to the target; in
/// by 0x80 for 90 updates, then out by 11.
fn dust(fx: &mut Fx, t: u32, sp: u32, a3: &mut u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42A0_0000);
        let v = fx.rand_from(t, 0x01E0_1500);
        fx.mem.set16(t + 0x56, (v >> 4) % 8);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x20);
        let v = rmod(fx, t, 0x01E0_1538, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_1554, 0x168);
        fx.mem.set16(t + 0x5C, v);
        let v = fx.rand_from(t, 0x01E0_1570);
        fx.mem.set16(t + 0xA, if v % 2 != 0 { 2 } else { 0 });
        let v = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, v);
        fx.mem.set16(t + 0x50, 0);
        let v = rmod(fx, t, 0x01E0_15AC, 0x15E);
        fx.mem.set16(t + 0x52, v + 0x64);
        let v = rmod(fx, t, 0x01E0_15CC, 0x14);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = rmod(fx, t, 0x01E0_15EC, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_1608, 5);
        fx.mem.set16(t + 6, v + 3);
        for (o, ra) in [(0x30u32, 0x01E0_1628u32), (0x34, 0x01E0_1654), (0x38, 0x01E0_1680)] {
            let v = rmod(fx, t, ra, 0x28) - 0x14;
            fx.mem.setf(t + o, ee::from_i32(v));
        }
        for o in [0x30u32, 0x34, 0x38] {
            let v = ee::div(fx.mem.f(t + o), 60.0);
            fx.mem.setf(t + o, v);
        }
        let h = fx.target_handle();
        unit_position_saving(fx, h, sp + 0x30, u64::from(t), 0x01E0_16E4);
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        node_position_saving(fx, h, slot, sp + 0x40, u64::from(t), Some(u64::from(D)), 0x01E0_1704);
        node_a3(fx, h, a3);
        let v = ee::div(ee::sub(fx.mem.f(sp + 0x38), fx.mem.f(sp + 0x48)), 80.0);
        fx.mem.setf(t + 0x3C, v);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x5A {
        fade_in(fx, t, 0x63, 0x80, 0x80);
    } else {
        fade_out(fx, t, 0x63, 0xD, 0xB);
    }
    let a = fx.mem.i16(t + 0x54);
    if (0xB..0x46).contains(&a) {
        for o in [0u32, 4, 8] {
            let v = fx.mem.f(t + 0x30 + o);
            addf(fx, t + 0x10 + o, v);
        }
    }
    let a = fx.mem.i16(t + 0x54);
    if (0xB..0x28).contains(&a) {
        let v = ee::sub(fx.mem.f(t + 0x14), f32::from_bits(0x3D4C_CCCD));
        fx.mem.setf(t + 0x14, v);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let v = fx.mem.f(t + 0x3C);
        addf(fx, t + 0x18, v);
        addf(fx, t + 0x48, 0.5);
    }
    age(fx, t);
}

/// 0x1E01850 (s0 = the task, s1 = D): a red light column (size 6,
/// (0xFF, 0x80, 0x80) / (0xFF, 0x40, 0x40), width 8, height 200 / 100 by
/// the slot +0x50, growing 25 / 22.5 a update) riding the attacker's node
/// 0xB6; in by 0x40 for 9 updates, then out by 5; +0x52 counting to 25.
fn column(fx: &mut Fx, t: u32, a3: &mut u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0xFF, 0x40, 0x40);
        for (k, h) in [(0, 0x4348_0000u32), (1, 0x42C8_0000), (2, 0x43C8_0000)] {
            if fx.mem.i16(t + 0x50) == k {
                fx.mem.write_u32(t + 0x30, 0x4100_0000);
                fx.mem.write_u32(t + 0x34, h);
            }
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.set16(t + 0x52, 0);
    }
    if fx.mem.i16(t + 0x54) < 9 {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 7, 5);
    }
    if fx.mem.i16(t + 0x50) == 0 && fx.mem.i16(t + 0x54) != 0 {
        addf(fx, t + 0x34, 25.0);
    }
    if fx.mem.i16(t + 0x50) == 1 && fx.mem.i16(t + 0x54) != 0 {
        addf(fx, t + 0x34, 22.5);
    }
    if fx.mem.i16(t + 0x54) != 0 && fx.mem.i16(t + 0x52) < 0x19 {
        let v = fx.mem.i16(t + 0x52) + 1;
        fx.mem.set16(t + 0x52, v);
    }
    age(fx, t);
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES + 2);
    node_position_saving(fx, h, slot, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_1A0C);
    node_a3(fx, h, a3);
}

/// What 0x1F6830 leaves in a3: a valid handle's object index runs
/// 0x1F40D0, which leaves a3 = (index < 0x54) (b611's rule) [F].
fn node_a3(fx: &mut Fx, h: i32, a3: &mut u32) {
    let (side, slot) = (h >> 8, h & 0xFF);
    if (0..3).contains(&side) && slot < 0xB {
        let i = fx.object_index(side, slot);
        if i >= 0 {
            *a3 = u32::from(i < 0x54);
        }
    }
}

/// 0x1E01A30 (D, task): b585's frame flash (b590's 0x1E02450 shape) in
/// (0x90, 0x90, 0x80), ST from +0x48 = 2.5: in by 0x16 under 0x58 until
/// effect update 350, then out by 4.
fn frame_flash(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4020_0000);
        l2::rgb(fx, t + 0x60, 0x90, 0x90, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for k in 0..4 {
        let c = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(sp + 0x48 + k, c);
    }
    fx.mem.write_u32(sp + 0x4C, 0x3F80_0000);
    for (at, v) in [(sp + 0x58, 0x6FF8u16), (sp + 0x68, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x50, ee::div(s, 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x54, ee::div(s, 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x60, ee::div(ee::sub(512.0, s), 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x64, ee::div(ee::sub(512.0, s), 512.0));
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if fx.mem.i16(COUNTER) < 0x15E {
        fade_in(fx, t, 0x63, 0x58, 0x16);
    } else {
        fade_out(fx, t, 0x63, 6, 4);
    }
    age(fx, t);
}

/// 0x1E01C30(D, t): b482's flash step (the target over the window, UVs
/// inset by +8 / 10 and +0xA / 10 over 512, into the descriptor at
/// +0xB228, no ordering table) in pale yellow (0xFF, 0xFF, 0x80): alpha up
/// by 9 below 0x80 for 140 updates, then down by 7. `regs` the unit
/// flash's s0 / s1 (the slot index, the live count).
fn flash(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_271C), (0x20, u64::from(regs[1])), (0x10, u64::from(regs[0]))]);
    // swc1 f20 / f21: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 4, 0);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let h = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = fx.mem.read_u32(FLASH_DESC);
    for k in 0..4u32 {
        let b = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(d + 0x10 + k, b);
    }
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    for (at, xy) in [(0x20u32, 0x6FF8u16), (0x30, 0x8FF8)] {
        fx.mem.write_u16(d + at, xy);
        fx.mem.write_u16(d + at + 2, xy);
        let v = fx.mem.read_u64(d + at);
        fx.mem.write_u64(d + at, (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
        fx.mem.write_u8(d + at + 7, 0);
    }
    for (o, v) in [(0x18u32, ee::div(w, 512.0)), (0x1C, ee::div(h, 512.0)), (0x28, ee::div(ee::sub(512.0, w), 512.0)), (0x2C, ee::div(ee::sub(512.0, h), 512.0))] {
        fx.mem.setf(d + o, v);
    }
    fx.submit(d, 0, 0);
    if fx.mem.i16(t + 0x54) < 0x8C {
        fade_in(fx, t, 0x63, 0x80, 9);
    } else {
        fade_out(fx, t, 0x63, 9, 7);
    }
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E02610(D) (b180's 0x1E02DF0, d1cf0abc65df): in a packet layer
/// (0x2078A0) the attacker drawn into the target (0x1F8270 filter 0; ALPHA
/// 0x48), TEXFLUSH before and after; every flash task stepped (0x1E01C30);
/// the layer linked into the ordering table at key 0 when any task was
/// alive.
fn unit_flash(fx: &mut Fx, a3: u32) {
    let sp = fx.enter(0xB0);
    let d = sp + 0x58;
    fx.mem.write_u32(FLASH_DESC, d);
    let layer = fx.layer_begin();
    fx.mem.write_u32(LAYER, layer);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.texflush([5, d, 0x26, a3]);
    fx.desc_alpha(d, 0x80_0000_0048);
    let h = fx.self_handle();
    let (_, regs) = fx.unit_draw(h, 0, sp + 0x98, sp + 0xA0);
    fx.texflush(regs);
    // s1 counts every live task (the `bne`'s delay slot).
    let mut live = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        live += 1;
        if fx.mem.read_u32(t + 0x44) == FLASH {
            flash(fx, t, [k, live]);
        }
    }
    if live != 0 {
        let layer = fx.mem.read_u32(LAYER);
        fx.layer_end(layer, OT, 0);
    }
    fx.leave(0xB0);
}
