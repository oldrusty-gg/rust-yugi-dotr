//! BMODULE b065 (card 158), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02D90, data area 0x1ED0000) [F]: b590's layout: a pool of 400 tasks of 0x70 bytes at D+0x2A0
//! (alive +0x58, step +0x44 called through `jalr`; 0x1E02D80 only returns
//! and is skipped), spawned by 0x1E03470, drawn with the second library:
//! arcs (e6dce326551d at 0x1E00920, `billboard::arc_strips`), textured arcs
//! (3a2db45c5a9a at 0x1E012B0, `quads4::arc_strips_tex`), single quads
//! (add5d81dd3fe at 0x1E00C80, `billboard::quad`) and light columns
//! (792ca1ad9378 at 0x1E00F40, `quads4::sprite_quads`). Update 1: four
//! columns at the attacker, the stage dimmed to 0x30 (0x1E029C0) and the
//! camera slots 4 / 5 (0x1E027A0); updates 17 / 30 / 56 and 24 / 36 / 56:
//! thirteen white motes at nodes 0xF / 0x32; 36..46 green arcs pulled from
//! node 0x2B toward 0x2C; update 53 a frame flash; 57..80 at the target
//! green arcs (textured, three kinds) and on update 57 a band. Returns 1 on
//! update 47, 4 on 120, 2 on 95, 6 on 58..89, 8 once the 130-update timer
//! D+0xB210 has run out.
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
const POS: u32 = D + 0x250;
const ARC: u32 = 0x01E0_1BA0;
const ARC_T: u32 = 0x01E0_1DB0;
const MOTE: u32 = 0x01E0_2240;
const COLUMN: u32 = 0x01E0_2410;
const BAND: u32 = 0x01E0_26B0;
const CAMERA: u32 = 0x01E0_27A0;
const LIGHT: u32 = 0x01E0_29C0;
const FLASH: u32 = 0x01E0_2AD0;
/// A step that only returns 0 (skipped by the entry).
const IDLE: u32 = 0x01E0_2D80;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_30A0;
const TIMER: u32 = D + 0xB210;
const COLOUR: u32 = D + 0xB214;
const NODE_A: u32 = D + 0xB244;
const NODE_B: u32 = D + 0xB246;
const NODE_C: u32 = D + 0xB248;
const NODE_D: u32 = D + 0xB24A;
const SPAWN_A: u32 = D + 0xB2A2;
const ARC_K: u32 = D + 0xB2A8;
const SPAWN_K: u32 = D + 0xB2B2;
const COUNTER: u32 = D + 0xB2BA;
const BYTES: u32 = D + 0xB2BC;
const TAB: u32 = D + 0xB4BC;
const CAM: u32 = D + 0xB1A0;
/// The target's point on update 57.
const AIM: u32 = D + 0xB1D0;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02D90 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xA0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2B8, param);
    fx.mem.set32(TIMER, 0x82);
    for (k, node) in [0xF, 0x32, 0x2B, 0x2C, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODE_A + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB254, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(D + 0xB1F0 + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x90);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x94)), 180.0);
    fx.fix_residue(y, 0x01E0_2F80);
    fx.mem.set16(D + 0xB2B6, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_2FB0, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_3004);
        fx.mem.set16(D + 0xB264 + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 || f == IDLE {
            continue;
        }
        // The step's prologue: ra, the entry's s1 = D, s0 = 0.
        let frame = match f {
            ARC => 0x40,
            ARC_T | MOTE => 0x30,
            COLUMN | CAMERA => 0x50,
            FLASH => 0x90,
            _ => 0x20,
        };
        let s = fx.enter(frame);
        if frame == 0x20 {
            fx.saved(s, &[(0x10, TASK_RA), (0, 0)]);
        } else {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
        }
        match f {
            ARC => arc(fx, t, s),
            ARC_T => arc_t(fx, t),
            MOTE => mote(fx, t),
            COLUMN => column(fx, t, s),
            BAND => band(fx, t),
            CAMERA => camera(fx, t, s),
            LIGHT => light(fx, t),
            FLASH => flash(fx, t, s),
            _ => panic!("b065: unknown task step {f:#x}"),
        }
        fx.leave(frame);
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        draw(fx, sp, t);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x2F => 1,
        0x78 => 4,
        0x5F => 2,
        0x3A..=0x59 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` / `sub` (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

fn draw(fx: &mut Fx, sp: u32, t: u32) {
    let f = fx.mem.read_u32(t + 0x44);
    match f {
        ARC | BAND => {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        ARC_T => {
            let kind = |fx: &mut Fx| fx.mem.i16(t + 0x50);
            if kind(fx) == 0 {
                let c = fx.mem.i16(t + 0x56);
                cell(fx, sp + 0x50, 0x97, c);
                let w = fx.mem.read_u32(D + 0x29C);
                fx.mem.write_u32(D + 0x88, w);
            }
            if kind(fx) == 1 {
                let c = fx.mem.i16(t + 0x56);
                cell(fx, sp + 0x60, 0x9B, c);
                let w = fx.mem.read_u32(D + 0x29C);
                fx.mem.write_u32(D + 0x88, w);
            }
            if kind(fx) == 2 {
                let w = fx.mem.read_u32(D + 0xB1F4);
                fx.mem.write_u32(D + 0x88, w);
                fx.mem.set16(D + 0xBE, 0);
                for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x80), (0x9A, 0x80)] {
                    fx.mem.set16(D + o, v);
                }
            }
            fx.mem.set16(D + 0xB8, 0);
            for k in 0..3 {
                let v = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, v);
            }
            fx.mem.set16(D + 0xB8, 0x24);
            if kind(fx) == 2 {
                fx.mem.set16(D + 0xB8, 6);
            }
            fx.mem.set16(D + 0xBE, 0);
            quads4::arc_strips_tex(fx, D, t, 0x20, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        MOTE => {
            let w = fx.mem.read_u32(D + 0xB1F0);
            fx.mem.write_u32(D + 0x88, w);
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x70, 0x14, c);
            fx.mem.set16(D + 0xBE, 0);
            billboard::quad(fx, uv::uv_rect, D, t, 0x10, t + 0x10);
        }
        COLUMN => {
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, sp + 0x80, 0x9A, 0);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sprite_quads(fx, D, t, 0, t + 0x10);
        }
        _ => {}
    }
}

/// 0x1E03470(D, step, arg): the first free task cleared and set off at
/// D+0x250 (i16 +0, float +0x10) with the colour D+0xB214 (twice), index
/// D+0xB2B2 (+0x50 / +0x52), angle D+0xB2A2 (+0x5A).
fn spawn(fx: &mut Fx, step: u32) {
    fx.enter(0x60);
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
        for k in 0..3 {
            let w = fx.mem.read_u32(POS + 4 * k);
            fx.mem.write_u32(t + 0x10 + 4 * k, w);
        }
        let s = fx.mem.i16(SPAWN_K);
        fx.mem.set16(t + 0x50, s);
        let s = fx.mem.i16(SPAWN_K);
        fx.mem.set16(t + 0x52, s);
        let a = fx.mem.i16(SPAWN_A);
        fx.mem.set16(t + 0x5A, a);
        fx.mem.write_u32(t + 0x40, 0);
        fx.mem.write_u32(t + 0x44, step);
    }
    fx.leave(0x60);
}

/// 0x1E016C0(D, 0): the update counter and the emitters (frame 0x30: ra,
/// the entry's s1 = D, s0 = 0).
fn emit(fx: &mut Fx, param: i32) {
    let _ = param;
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_3060), (0x10, u64::from(D)), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    if (0x24..0x2F).contains(&c(fx)) {
        let k = fx.mem.i16(ARC_K);
        fx.mem.set16(SPAWN_K, k);
        spawn(fx, ARC);
        let k = fx.mem.i16(ARC_K) + 1;
        fx.mem.set16(ARC_K, k);
    }
    if c(fx) == 1 {
        for (a, k) in [(0, 0), (0x5A, 0), (0x2D, 1), (0x87, 1)] {
            fx.mem.set16(SPAWN_A, a);
            fx.mem.set16(SPAWN_K, k);
            spawn(fx, COLUMN);
        }
        spawn(fx, LIGHT);
        spawn(fx, CAMERA);
    }
    if c(fx) == 0x39 {
        let h = fx.target_handle();
        fx.unit_position(h, AIM);
    }
    if (0x39..0x51).contains(&c(fx)) {
        for k in 0..4 {
            let w = fx.mem.read_u32(AIM + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
        let h = fx.target_handle();
        fx.unit_position(h, POS);
        let y = ee::sub(fx.mem.f(POS + 4), 15.0);
        fx.mem.setf(POS + 4, y);
        fx.mem.set16(SPAWN_K, 0);
        if c(fx) % 2 == 1 {
            spawn(fx, ARC_T);
        }
        fx.mem.set16(SPAWN_K, 1);
        if c(fx) % 2 == 1 {
            spawn(fx, ARC_T);
        }
        fx.mem.set16(SPAWN_K, 2);
        if c(fx) % 2 == 0 {
            spawn(fx, ARC_T);
        }
    }
    if c(fx) == 0x39 {
        spawn(fx, BAND);
    }
    if c(fx) == 0x35 {
        spawn(fx, FLASH);
    }
    for (at, ups) in [(NODE_A, [0x11, 0x1E, 0x38]), (NODE_B, [0x18, 0x24, 0x38])] {
        if ups.contains(&c(fx)) {
            let h = fx.self_handle();
            let slot = fx.mem.i16(at);
            fx.node_position(h, slot, POS);
            let y = ee::sub(fx.mem.f(POS + 4), 1.5);
            fx.mem.setf(POS + 4, y);
            for _ in 0..0xD {
                spawn(fx, MOTE);
            }
        }
    }
    fx.leave(0x30);
}

/// Byte `at` up by `up` while under `cap`.
fn fade_in(fx: &mut Fx, t: u32, at: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + at);
    if a < cap {
        fx.mem.write_u8(t + at, a.wrapping_add(up));
    }
}

/// Byte `at` down by `down` while at least `floor`, else the task ends
/// (`end`) or the byte is cleared.
fn fade_out(fx: &mut Fx, t: u32, at: u32, floor: u8, down: u8, end: bool) {
    let a = fx.mem.read_u8(t + at);
    if a >= floor {
        fx.mem.write_u8(t + at, a.wrapping_sub(down));
    } else if end {
        fx.mem.set16(t + 0x58, 0);
    } else {
        fx.mem.write_u8(t + at, 0);
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn add(fx: &mut Fx, at: u32, by: f32) {
    let v = ee::add(fx.mem.f(at), by);
    fx.mem.setf(at, v);
}

/// +0x10 = node 0x2B's point pulled toward node 0x2C's by `by` / `div` (the
/// step's local sp + 0x30), the frames 0x1F6830 leaves (s0 = the task, s1
/// = D).
fn pull(fx: &mut Fx, t: u32, sp: u32, div: f32, by: f32, ra: [u32; 2]) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODE_C);
    crate::b076::node_position_saving(fx, h, slot, t + 0x10, u64::from(t), Some(u64::from(D)), ra[0]);
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODE_D);
    crate::b076::node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), ra[1]);
    for k in 0..3u32 {
        let v = ee::div(ee::sub(fx.mem.f(sp + 0x30 + 4 * k), fx.mem.f(t + 0x10 + 4 * k)), div);
        fx.mem.setf(sp + 0x30 + 4 * k, v);
    }
    for k in 0..3u32 {
        let v = ee::mul(fx.mem.f(sp + 0x30 + 4 * k), by);
        add(fx, t + 0x10 + 4 * k, v);
    }
}

/// 0x1E01BA0 (D, task): a dark green arc (radius 15 + 5 x +0x50) on the
/// line node 0x2B -> 0x2C at +0x50 / 10, facing the camera turned 0x4000;
/// in by 0x30 under 0xC0 for 60 updates, out by 9; ends on update 68.
fn arc(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let k = ee::from_i32(fx.mem.i16(t + 0x50));
        fx.mem.setf(t + 0x4C, ee::add(15.0, ee::mul(5.0, k)));
        l2::rgb(fx, t + 0x60, 0x18, 0x18, 0x18);
        l2::rgb(fx, t + 0x64, 0x20, 0x30, 0x20);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0x3C {
        fade_in(fx, t, 0x67, 0xC0, 0x30);
    } else {
        fade_out(fx, t, 0x67, 0xB, 9, true);
    }
    let by = ee::from_i32(fx.mem.i16(t + 0x50));
    pull(fx, t, sp, 10.0, by, [0x01E0_1CA0, 0x01E0_1CC0]);
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1D60);
    let a = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
    fx.mem.set32(t + 0x20, a);
    age(fx, t);
    if fx.mem.i16(COUNTER) >= 0x44 {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E01DB0 (D, task): a textured arc at the target (radius 140 growing
/// by 35..42.9, the inner +0x4C a share of it), three kinds by +0x50
/// (green on dark / bright by the effect update's parity); 0x80, out by
/// 0xB.
fn arc_t(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x430C_0000);
        fx.mem.setf(t + 0x4C, ee::sub(140.0, 58.0));
        let odd = |fx: &mut Fx| fx.mem.i16(COUNTER) % 2 == 1;
        let colours: [((i32, i32, i32), (i32, i32, i32)); 3] = [((0x80, 0xC0, 0x80), (0x80, 0xFF, 0x80)), ((0x60, 0x80, 0x60), (0x40, 0xFF, 0x40)), ((0x50, 0x30, 0x30), (0x60, 0x28, 0x28))];
        let dark = [8, 8, 8];
        let light = [0x38, 0x38, 0x20];
        for kind in 0..3 {
            if fx.mem.i16(t + 0x50) == kind {
                let (even_c, odd_c) = colours[kind as usize];
                if !odd(fx) {
                    l2::rgb(fx, t + 0x64, even_c.0, even_c.1, even_c.2);
                    let g = dark[kind as usize];
                    l2::rgb(fx, t + 0x60, g, g, g);
                }
                if odd(fx) {
                    l2::rgb(fx, t + 0x64, odd_c.0, odd_c.1, odd_c.2);
                    let g = light[kind as usize];
                    l2::rgb(fx, t + 0x60, g, g, g);
                }
                if kind != 0 {
                    fx.mem.write_u32(t + 0x34, 0x4120_0000);
                }
                fx.mem.set16(t + 6, if kind == 0 { 8 } else { 0xA });
            }
        }
        for (kind, a, b) in [(0, 0xA, 0xA), (1, 0xA, 3), (2, 0x3C, 0x3C)] {
            if fx.mem.i16(t + 0x50) == kind {
                fx.mem.set16(t + 0x5A, a);
                fx.mem.set16(t + 0x5C, b);
            }
        }
        let v = rmod(fx, t, 0x01E0_2090, 0x50);
        fx.mem.setf(t + 0x38, ee::from_i32(v + 0x15E));
        let v = ee::div(fx.mem.f(t + 0x38), 10.0);
        fx.mem.setf(t + 0x38, v);
        fx.mem.write_u32(t + 0x30, 0x43AF_0000);
        let v = ee::div(fx.mem.f(t + 0x30), 10.0);
        fx.mem.setf(t + 0x30, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let v = rmod(fx, t, 0x01E0_20EC, 0xFFFF);
        fx.mem.set32(t + 0x24, v);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x67, 0x80);
    } else {
        fade_out(fx, t, 0x67, 0xD, 0xB, true);
    }
    for (kind, share) in [(0, 0x3ECC_CCCDu32), (1, 0x3F33_3333), (2, 0x3F33_3333)] {
        if fx.mem.i16(t + 0x50) == kind {
            let s = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x38));
            fx.mem.setf(t + 0x48, s);
            fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(share), s));
            if kind != 0 {
                let v = fx.mem.f(t + 0x38);
                add(fx, t + 0x34, v);
            }
        }
    }
    age(fx, t);
}

/// 0x1E02240 (D, task): a white mote (size 130, cell 2..4) thrown 6.6 out
/// at a random angle, then drifting 0.5 a update; in by 0x40 for 10
/// updates, out by 9.
fn mote(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x82);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_2278);
        let r = fx.div(v >> 4, 3).1;
        fx.mem.set16(t + 0x56, r + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = fx.rand_from(t, 0x01E0_22B0);
        let r = fx.div(v >> 4, 0x168).1;
        fx.mem.set16(t + 0x5A, r);
        drift(fx, t, f32::from_bits(0x40D3_3333));
    }
    if fx.mem.i16(t + 0x54) < 0xA {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9, true);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        drift(fx, t, 0.5);
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_23E0);
    age(fx, t);
}

/// z += `by` sin +0x5A, x += `by` cos +0x5A.
fn drift(fx: &mut Fx, t: u32, by: f32) {
    let a = fx.mem.i16(t + 0x5A);
    let s = l2::sin_deg(fx, TAB, a, 0);
    add(fx, t + 0x18, ee::mul(by, s));
    let a = fx.mem.i16(t + 0x5A);
    let c = l2::sin_deg(fx, TAB, a, 1);
    add(fx, t + 0x10, ee::mul(by, c));
}

/// 0x1E02410 (D, task): a light column (size 6; 3 / 8 wide, 70 / 30 / 400
/// high by +0x50, the first two growing by 30 / 10) on the node line
/// 0x2B -> 0x2C at +0x52 (to 25) / 25; in by 0x40 for 4 updates, out by 9.
fn column(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x40, 0x40, 0xFF);
        for (kind, w, h) in [(0, 0x4040_0000u32, 0x428C_0000u32), (1, 0x4040_0000, 0x41F0_0000), (2, 0x4100_0000, 0x43C8_0000)] {
            if fx.mem.i16(t + 0x50) == kind {
                fx.mem.write_u32(t + 0x30, w);
                fx.mem.write_u32(t + 0x34, h);
            }
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.set16(t + 0x52, 0);
    }
    if fx.mem.i16(t + 0x54) < 4 {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9, true);
    }
    for (kind, by) in [(0, 30.0f32), (1, 10.0)] {
        if fx.mem.i16(t + 0x50) == kind && fx.mem.i16(t + 0x54) != 0 {
            add(fx, t + 0x34, by);
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let n = fx.mem.i16(t + 0x52);
        if n < 0x19 {
            fx.mem.set16(t + 0x52, n + 1);
        }
    }
    age(fx, t);
    let a = fx.mem.i16(t + 0x5A) + 6;
    fx.mem.set16(t + 0x5A, a);
    let by = ee::from_i32(fx.mem.i16(t + 0x52));
    pull(fx, t, sp, 25.0, by, [0x01E0_25D8, 0x01E0_25F8]);
}

/// 0x1E026B0 (D, task): a grey / green band (radius 200) facing the camera
/// turned 0x4000; in by 0x30 under 0xC0 for 30 updates, out by 0xF.
fn band(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x4348_0000);
        l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x60, 0x80, 0xFF, 0x80);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0x1E {
        fade_in(fx, t, 0x67, 0xC0, 0x30);
    } else {
        fade_out(fx, t, 0x67, 0x11, 0xF, true);
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2770);
    let a = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
    fx.mem.set32(t + 0x20, a);
    age(fx, t);
}

/// 0x1E027A0 (D, task): camera slots 4 / 5 (0x1F7E20) from D+0xB1A0 (128,
/// 10, 10, 10; node 0x2C's point; 0, 123, w rising by 0.3 to 1 for 60
/// updates, then down by 0.3); cleared when the task ends (w at 0.1, or
/// the timer at 2).
fn camera(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x4120_0000), (8, 0x4120_0000), (0xC, 0x4120_0000), (0x20, 0), (0x24, 0x42F6_0000), (0x28, 0)] {
            fx.mem.write_u32(CAM + o, v);
        }
        for k in 0..3 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * k);
            fx.mem.write_u32(CAM + 0x10 + 4 * k, w);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.set16(t + 0x50, 0);
    }
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODE_D);
    crate::b076::node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), 0x01E0_2880);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x30 + 4 * k);
        fx.mem.write_u32(CAM + 0x10 + 4 * k, w);
    }
    let w = fx.mem.f(CAM + 0x28);
    let step = f32::from_bits(0x3E99_999A);
    if fx.mem.i16(t + 0x54) < 0x3C {
        if w < 1.0 {
            fx.mem.setf(CAM + 0x28, ee::add(w, step));
        }
    } else if w <= f32::from_bits(0x3DCC_CCCD) {
        fx.mem.set16(t + 0x58, 0);
    } else {
        fx.mem.setf(CAM + 0x28, ee::sub(w, step));
    }
    let (p, ra) = if fx.mem.i16(t + 0x58) == 0 { (0, 0x01E0_296C) } else { (CAM, 0x01E0_2980) };
    crate::blib::exe::camera_slots45(fx, p);
    // 0x1F7E20's frame (s0 = the task, ra).
    fx.saved(sp - 0x20, &[(0, u64::from(t)), (0x10, ra)]);
    if fx.mem.i32(TIMER) == 2 {
        fx.mem.set16(t + 0x58, 0);
    }
    age(fx, t);
}

/// 0x1E029C0 (D, task): the stage light (0x1F7F10) down by 10 to 0x30 for
/// 51 updates, back up by 10 to 0x80 once the timer is under 30; 0x80 and
/// the task's end when the timer is at 3.
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x33 {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x31 { 0x30 } else { l - 0xA });
    } else if fx.mem.i32(TIMER) < 0x1E {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x80 { l + 0xA } else { 0x80 });
        if fx.mem.i16(t + 0x50) >= 0x81 {
            fx.mem.set16(t + 0x50, 0x80);
        }
    }
    age(fx, t);
    if fx.mem.i32(TIMER) == 3 {
        fx.set_stage_light(0x80);
        fx.mem.set16(t + 0x58, 0);
    } else {
        let l = fx.mem.read_u8(t + 0x50);
        fx.set_stage_light(i32::from(l));
    }
}

/// 0x1E02AD0 (D, task): a frame flash (b590's 0x1E02450 with grey-green
/// 0x80 / 0x88 (0x90 from update 54), ST from +0x48 = 2.5 growing by 0.6
/// to 4 from update 55): in by 0x16 under 0x58 (then by 0xC under 0x60
/// from update 55) until update 80, then out by 4.
fn flash(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4020_0000);
        l2::rgb(fx, t + 0x60, 0x80, 0x88, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(COUNTER) == 0x36 {
        l2::rgb(fx, t + 0x60, 0x80, 0x90, 0x80);
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
    if fx.mem.i16(COUNTER) >= 0x37 {
        let s = fx.mem.f(t + 0x48);
        if s < 4.0 {
            fx.mem.setf(t + 0x48, ee::add(s, f32::from_bits(0x3F19_999A)));
        }
    }
    if fx.mem.i16(COUNTER) < 0x50 {
        fade_in(fx, t, 0x63, 0x58, 0x16);
        if fx.mem.i16(COUNTER) >= 0x37 {
            fade_in(fx, t, 0x63, 0x60, 0xC);
        }
    } else {
        fade_out(fx, t, 0x63, 6, 4, true);
    }
    age(fx, t);
}
