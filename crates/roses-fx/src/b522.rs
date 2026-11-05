//! BMODULE b522 (card 350), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02D40, data area 0x1ED0000, 0xCF30 bytes) [F]
//!: b703's program (b590's 400-task pool of 0x70
//! bytes at D+0x2A0, alive +0x58, step +0x44 through `jalr`, 0x1E02D30
//! the idle step; every global after the pool 0x1900 higher, the node
//! histories D+0xB1D0 / +0xB310) with its globals re-laid (nodes D+0xCB44,
//! kind D+0xCBB0, angle D+0xCBA2, facing D+0xCBB4, counter D+0xCBB8,
//! sines D+0xCDBC), spawned by 0x1E03380 (b703's spawn on those globals)
//! and drawn with the second library: rings of quads (7369422df7f8 at
//! 0x1E009B0), arcs (e6dce326551d at 0x1E00F60), quads (add5d81dd3fe at
//! 0x1E012C0), light columns (792ca1ad9378 at 0x1E019C0) and b293's
//! gouraud ribbon (24e8f5d92559 at 0x1E03530, [`crate::b703::ribbon`]).
//! Update 1 the stage dimmed (0x1E02C20); 0x36 four red columns pulled
//! along node 0x36 -> 0x35; 0x1C..0x5B thirteen motes at nodes 0x27 /
//! 0x2F on eleven updates; 0x5C the ribbon task (nodes 0x36 / 0x35 into
//! two histories to update 0x67, drawn once in pink) and the target's
//! point kept; 0x5F at it a red arc (radius 180) and 0x5F..0x69 ten
//! growing rings a update. Returns 1 on update 0x55, 4 on 0x8C, 2 on
//! 0x73, 6 on 0x60..0x6D, 8 once the 0x96-update timer D+0xCB10 has run
//! out.
use crate::blib::{billboard, quads4, spawn as hist, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
const POS: u32 = D + 0x250;
const RING: u32 = 0x01E0_2150;
const PULLED: u32 = 0x01E0_2300;
const MOTE: u32 = 0x01E0_25A0;
const TRAILS: u32 = 0x01E0_2770;
const ARC: u32 = 0x01E0_2B00;
const LIGHT: u32 = 0x01E0_2C20;
const IDLE: u32 = 0x01E0_2D30;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_3050;
/// The two node histories (20 points of 16 bytes).
const HIST: [u32; 2] = [D + 0xB1D0, D + 0xB310];
/// The target's point kept by the emitter.
const KEPT: u32 = D + 0xCAD0;
const TEX: u32 = D + 0xCAF0;
const TIMER: u32 = D + 0xCB10;
const COLOUR: u32 = D + 0xCB14;
const NODES: u32 = D + 0xCB44;
const SPAWN_A: u32 = D + 0xCBA2;
const SPAWN_K: u32 = D + 0xCBB0;
const FACING: u32 = D + 0xCBB4;
const COUNTER: u32 = D + 0xCBB8;
const BYTES: u32 = D + 0xCBBA;
const TAB: u32 = D + 0xCDBC;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn node(fx: &mut Fx, k: u32) -> i32 {
    fx.mem.i16(NODES + 2 * k)
}

/// The entry 0x1E02D40 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    fx.mem.write_u32(D + 0xCB1C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x90);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xCF30);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xCBB6, param);
    fx.mem.set32(TIMER, 0x96);
    for (k, n) in [0x36, 0x35, 0x27, 0x2F, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xCB54, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x80);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x84)), 180.0);
    fx.fix_residue(y, 0x01E0_2F30);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_2F60, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_2FB4);
        fx.mem.set16(D + 0xCB64 + 2 * k, v % 8);
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
            MOTE => 0x30,
            TRAILS => 0x40,
            PULLED => 0x50,
            _ => 0x20,
        };
        let s = fx.enter(frame);
        if frame == 0x20 {
            fx.saved(s, &[(0x10, TASK_RA), (0, 0)]);
        } else {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
        }
        match f {
            RING => ring(fx, t),
            PULLED => pulled(fx, t, s),
            MOTE => mote(fx, t),
            TRAILS => trails(fx, t, s),
            ARC => arc(fx, t),
            LIGHT => light(fx, t),
            _ => panic!("b522: unknown task step {f:#x}"),
        }
        fx.leave(frame);
    }
    // The entry's s0: the live tasks counted in the draw loop.
    let mut s0 = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        s0 += 1;
        draw(fx, sp, t, s0, k);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x55 => 1,
        0x8C => 4,
        0x73 => 2,
        0x60..=0x6D => 6,
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

fn tex(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

fn draw(fx: &mut Fx, sp: u32, t: u32, s0: u32, k: u32) {
    let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
    if f(fx) == RING {
        tex(fx, TEX);
        let c = fx.mem.i16(t + 0x56);
        cell(fx, sp + 0x50, 0x14, c);
        fx.mem.set16(D + 0xB8, 4);
        let v = fx.rand_from(s0, 0x01E0_30FC);
        fx.mem.set16(D + 0xBE, v % 4);
        fx.mem.set16(D + 0xB0, 0);
        billboard::ring_quads(fx, uv::uv_rect, D, t, 0x21, BYTES);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f(fx) == PULLED {
        tex(fx, D + 0x29C);
        cell(fx, sp + 0x60, 0x9A, 0);
        fx.mem.set16(D + 0xBE, 0);
        quads4::sprite_quads(fx, D, t, 0, t + 0x10);
    }
    if f(fx) == MOTE {
        tex(fx, TEX + 4);
        let c = fx.mem.i16(t + 0x56);
        cell(fx, sp + 0x70, 0x14, c);
        fx.mem.set16(D + 0xBE, 0);
        billboard::quad(fx, uv::uv_rect, D, t, 0x10, t + 0x10);
    }
    if f(fx) == TRAILS {
        let saves = [u64::from(s0), u64::from(D), u64::from(t - 0x2A0), u64::from(k), 0, 0x01E0_3250];
        crate::b703::ribbon(fx, t, HIST[0], HIST[1], saves);
    }
    if f(fx) == ARC {
        fx.mem.set16(D + 0xB8, 0x24);
        billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
        fx.mem.set16(D + 0xB8, 0);
    }
}

/// 0x1E03380(D, step, 0): b703's spawn (0x1E03C70) on this record's
/// globals (colour D+0xCB14, kind D+0xCBB0 into +0x50 / +0x52, angle
/// D+0xCBA2 into +0x5A); its frame's saves the emitter's s0 = D / s1 and
/// the entry's s2 = param, the return address `ra`.
fn spawn(fx: &mut Fx, step: u32, s1: u32, param: i32, ra: u32) -> i32 {
    let f = fx.enter(0x60);
    fx.saved(f, &[(0, u64::from(D)), (0x10, u64::from(s1)), (0x20, param as i64 as u64), (0x30, 0), (0x40, 0), (0x50, u64::from(ra))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x2F8 + 0x70 * k) == 0) {
        Some(k) => {
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
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

fn node_pos(fx: &mut Fx, k: u32, out: u32) {
    let h = fx.self_handle();
    let slot = node(fx, k);
    fx.node_position(h, slot, out);
}

/// 0x1E01D30(D, 0): the update counter and the emitters (frame 0x30: ra,
/// the entry's s1 = D, s0 = 0).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_300C), (0x10, u64::from(D)), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    if c(fx) == 1 {
        spawn(fx, LIGHT, D, param, 0x01E0_1D84);
    }
    if c(fx) == 0x5C {
        spawn(fx, TRAILS, D, param, 0x01E0_1DB0);
    }
    if c(fx) == 0x5C {
        node_pos(fx, 2, KEPT);
    }
    if c(fx) == 0x5C {
        let h = fx.target_handle();
        fx.unit_position(h, KEPT);
    }
    if c(fx) == 0x5F {
        for k in 0..4 {
            let w = fx.mem.read_u32(KEPT + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
        spawn(fx, ARC, D, param, 0x01E0_1E70);
    }
    if (0x5F..0x6A).contains(&c(fx)) {
        for k in 0..0xA {
            spawn(fx, RING, k, param, 0x01E0_1EB0);
        }
    }
    if c(fx) == 0x36 {
        let ras = [0x01E0_1F1C, 0x01E0_1F4C, 0x01E0_1F80, 0x01E0_1FB4];
        for ((a, kind), ra) in [(0, 0), (0x5A, 0), (0x2D, 1), (0x87, 1)].into_iter().zip(ras) {
            fx.mem.set16(SPAWN_A, a);
            fx.mem.set16(SPAWN_K, kind);
            spawn(fx, PULLED, D, param, ra);
        }
    }
    for (hits, k, ra) in [(&[0x22, 0x30, 0x3E, 0x4C, 0x5B][..], 2, 0x01E0_204Cu32), (&[0x1C, 0x29, 0x37, 0x46, 0x54, 0x56][..], 3, 0x01E0_2114)] {
        if hits.contains(&c(fx)) {
            node_pos(fx, k, POS);
            let y = ee::sub(fx.mem.f(POS + 4), 1.5);
            fx.mem.setf(POS + 4, y);
            for j in 0..0xD {
                spawn(fx, MOTE, j, param, ra);
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

fn add(fx: &mut Fx, at: u32, by: f32) {
    let v = ee::add(fx.mem.f(at), by);
    fx.mem.setf(at, v);
}

/// 0x1E02150 (D, task): b703's growing grey ring (0x1E02370) with size
/// 6..20 and life 7..11: radius 10..49 growing by 16..35; in by 0x40
/// under 0xC0, out by 0xE.
fn ring(fx: &mut Fx, t: u32) {
    let f = RING;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x24, 0xF);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 6));
        let v = rmod(fx, t, f + 0x50, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, f + 0x7C, 0x28);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, f + 0x9C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0xB8, 0x14);
        fx.mem.set16(t + 0xE, v + 0x10);
        let v = rmod(fx, t, f + 0xD8, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0xF4, 5);
        fx.mem.set16(t + 6, v + 7);
        fx.mem.set16(t + 0x58, 2);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x40));
        }
    } else if a >= 0x10 {
        fx.mem.write_u8(t + 0x63, a - 0xE);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E025A0 (D, task): b745's white mote (b703's 0x1E02A00) at size 80.
fn mote(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x50);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_25D8);
        let r = fx.div(v >> 4, 3).1;
        fx.mem.set16(t + 0x56, r + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = fx.rand_from(t, 0x01E0_2610);
        let r = fx.div(v >> 4, 0x168).1;
        fx.mem.set16(t + 0x5A, r);
        drift(fx, t, f32::from_bits(0x40D3_3333));
    }
    if fx.mem.i16(t + 0x54) < 0xA {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        drift(fx, t, 0.5);
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2740);
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

/// 0x1E02300 (D, task): b377's light column (b703's 0x1E02BD0) white /
/// red, its nodes 0x36 -> 0x35: +0x10 node 0x36's point pulled toward
/// node 0x35's by +0x52 / 25.
fn pulled(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0x40, 0x40);
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
        fade_out(fx, t, 0x63, 0xB, 9);
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
    let h = fx.self_handle();
    let slot = node(fx, 0);
    crate::b076::node_position_saving(fx, h, slot, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_24C8);
    let h = fx.self_handle();
    let slot = node(fx, 1);
    crate::b076::node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), 0x01E0_24E8);
    for k in 0..3u32 {
        let v = ee::div(ee::sub(fx.mem.f(sp + 0x30 + 4 * k), fx.mem.f(t + 0x10 + 4 * k)), 25.0);
        fx.mem.setf(sp + 0x30 + 4 * k, v);
    }
    for k in 0..3u32 {
        let v = ee::mul(fx.mem.f(sp + 0x30 + 4 * k), by);
        add(fx, t + 0x10 + 4 * k, v);
    }
}

/// 0x1E02770 (D, task): b293's ribbon task (0x1E02780) in pink (0xFF,
/// 0xC0, 0xC0) on nodes 0x36 / 0x35: in by 0x3C under 0xF0 to update
/// 0x66, then out by 0x1D (the task ends under 0x1F); to update 0x67 each
/// history takes its node's point. Its fill and step also run nodes 2 / 3
/// into histories 2 / 3 (D+0xB450 / +0xB590), never drawn.
fn trails(fx: &mut Fx, t: u32, sp: u32) {
    l2::rgb(fx, t + 0x60, 0xFF, 0xC0, 0xC0);
    let hist4 = [HIST[0], HIST[1], D + 0xB450, D + 0xB590];
    let ras_fill = [0x01E0_27D8u32, 0x01E0_2844, 0x01E0_28B4, 0x01E0_2924];
    let ras_step = [0x01E0_2A08u32, 0x01E0_2A40, 0x01E0_2A78, 0x01E0_2AB0];
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        fx.mem.set16(t + 0xE, 0x1E);
        for k in 0..4u32 {
            let h = fx.self_handle();
            let slot = node(fx, k);
            crate::b076::node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), ras_fill[k as usize]);
            for e in 0..0x13u32 {
                l2::copy3(fx, hist4[k as usize] + 0x10 * e, sp + 0x30);
            }
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(COUNTER) < 0x67 {
        fade_in(fx, t, 0x63, 0xF0, 0x3C);
    } else {
        fade_out(fx, t, 0x63, 0x1F, 0x1D);
    }
    if fx.mem.i16(COUNTER) < 0x68 {
        for k in 0..4u32 {
            let h = fx.self_handle();
            let slot = node(fx, k);
            crate::b076::node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), ras_step[k as usize]);
            hist::history(fx, hist4[k as usize] + 0x130, 0x14, sp + 0x30);
        }
    }
    fx.mem.set16(t + 6, 0xA);
    age(fx, t);
}

/// 0x1E02B00 (D, task): b703's arc (0x1E03200) in red with outer radius
/// 180 growing by 0.4.
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x4334_0000);
        l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0xE {
        fade_in(fx, t, 0x67, 0xC0, 0x30);
    } else {
        fade_out(fx, t, 0x67, 0x11, 0xF);
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2BC0);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    if fx.mem.i16(t + 0x54) != 0 {
        add(fx, t + 0x4C, f32::from_bits(0x3ECC_CCCD));
    }
    age(fx, t);
}

/// 0x1E02C20 (D, task): b703's stage light (0x1E03320): down by 10 to
/// 0x30 for 51 updates, back up once the timer is under 20.
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x33 {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x31 { 0x30 } else { l - 0xA });
    } else if fx.mem.i32(TIMER) < 0x14 {
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
