//! BMODULE b735 (card 360), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02E00, data area 0x1ED0000) [F]: b590's layout: a pool of 400 tasks of 0x70 bytes at D+0x2A0
//! (alive +0x58, step +0x44 called through `jalr`; 0x1E02DF0 only returns
//! and is skipped), spawned by 0x1E036A0 (= [`crate::b590::spawn`]), drawn
//! with the second library: single quads (add5d81dd3fe at 0x1E00ED0,
//! `billboard::quad`), rings of quads (7369422df7f8 at 0x1E00920,
//! `billboard::ring_quads`), sparks (21cb40d76bec at 0x1E01190,
//! `quads4::sparks`), light columns (792ca1ad9378 at 0x1E01760,
//! `quads4::sprite_quads`) and full-window tints (561a6131a635 at
//! 0x1E01590, `quads4::tint4`). From update 42 to 254 red rings at the
//! attacker's node 0x31; 82..259 nine rings a update there pulled toward
//! node 0x32; update 256 at the target: fourteen sparks, a tint, four
//! columns. Returns 1 on update 246, 4 on 295, 2 on 280, 6 on 257..274, 8
//! once the 300-update timer D+0xB210 has run out.
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
const POS: u32 = D + 0x250;
const QUAD: u32 = 0x01E0_1DE0;
const RING_A: u32 = 0x01E0_21B0;
const RING_B: u32 = 0x01E0_2500;
const SPARKS: u32 = 0x01E0_2850;
const COLUMN: u32 = 0x01E0_2A50;
const TINT: u32 = 0x01E0_2C60;
/// A step that only returns 0 (skipped by the entry).
const IDLE: u32 = 0x01E0_2DF0;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_3110;
const TIMER: u32 = D + 0xB210;
const TINT_W: u32 = D + 0xB234;
const NODE_A: u32 = D + 0xB248;
const NODE_B: u32 = D + 0xB24A;
const SPAWN_A: u32 = D + 0xB2A6;
const SPAWN_K: u32 = D + 0xB2B4;
const PULL: u32 = D + 0xB2B8;
const FACING: u32 = D + 0xB2BA;
const COUNTER: u32 = D + 0xB2BE;
const BYTES: u32 = D + 0xB2C0;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02E00 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x160);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x160);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, D + 0xB4C0);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2BC, param);
    fx.mem.set32(TIMER, 0x12C);
    for (k, node) in [0x31, 0x32, 0x35, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODE_A + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB258, r);
    for k in 0xE..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x298 + 4 * (k as u32 - 0xE), t);
    }
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(D + 0xB1F0 + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xC0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xC4)), 180.0);
    fx.fix_residue(y, 0x01E0_2FF4);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_3024, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_3074);
        fx.mem.set16(D + 0xB268 + 2 * k, v % 8);
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
        // The step's prologue: ra, the entry's s1 = D (0x40 frames), s0 = 0.
        let frame = if matches!(f, QUAD | RING_A | RING_B) { 0x40 } else { 0x20 };
        let s = fx.enter(frame);
        if frame == 0x40 {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
        } else {
            fx.saved(s, &[(0x10, TASK_RA), (0, 0)]);
        }
        match f {
            QUAD => quad(fx, t, s),
            RING_A => ring_a(fx, t, s),
            RING_B => ring_b(fx, t, s),
            SPARKS => sparks(fx, t),
            COLUMN => column(fx, t),
            TINT => tint(fx, t),
            _ => panic!("b735: unknown task step {f:#x}"),
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
        draw(fx, sp, t, s0);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xF6 => 1,
        0x127 => 4,
        0x118 => 2,
        0x101..=0x112 => 6,
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

fn draw(fx: &mut Fx, sp: u32, t: u32, s0: u32) {
    let f = fx.mem.read_u32(t + 0x44);
    match f {
        QUAD => {
            let w = fx.mem.read_u32(D + 0xB1F8);
            fx.mem.write_u32(D + 0x88, w);
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x70, 0x14, c);
            let c = fx.mem.i16(t + 0x56);
            if c < 2 {
                fx.mem.set16(D + 0x94, c << 6);
                fx.mem.set16(D + 0x96, 0);
            } else {
                fx.mem.set16(D + 0x94, (c - 2) << 6);
                fx.mem.set16(D + 0x96, 0x40);
            }
            fx.mem.set16(D + 0x98, 0x40);
            fx.mem.set16(D + 0x9A, 0x40);
            let o = fx.mem.i16(t + 6);
            fx.mem.set16(D + 0xBE, o);
            billboard::quad(fx, uv::uv_rect, D, t, 0x21, t + 0x10);
        }
        RING_A | RING_B => {
            let w = fx.mem.read_u32(D + 0xB1F4);
            fx.mem.write_u32(D + 0x88, w);
            let (local, ra) = if f == RING_A { (0x80, 0x01E0_3298) } else { (0x90, 0x01E0_3358) };
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + local, 0x14, c);
            let c = fx.mem.i16(t + 0x56);
            fx.mem.set16(D + 0x94, c << 5);
            fx.mem.set16(D + 0x96, 0);
            fx.mem.set16(D + 0x98, 0x20);
            fx.mem.set16(D + 0x9A, 0x40);
            fx.mem.set16(D + 0xB8, 4);
            let v = fx.rand_from(s0, ra);
            fx.mem.set16(D + 0xBE, v % 4);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x21, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        SPARKS => {
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0xA0, 0x97, c);
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 0);
            for k in 0..3 {
                let v = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, v);
            }
            fx.mem.set16(D + 0xB8, 0xC);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sparks(fx, D, t, 0x20, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        COLUMN => {
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, sp + 0xB0, 0x9A, 0);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sprite_quads(fx, D, t, 0, t + 0x10);
        }
        TINT => {
            let h = fx.target_handle();
            fx.unit_position(h, sp + 0x60);
            // 0x1F71A0's frame below the entry's (its saved ra, s0).
            fx.saved(sp - 0x20, &[(0x10, 0x01E0_34C0), (0, u64::from(s0))]);
            let z = fx.mem.f(sp + 0x68);
            let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 50.0) } else { ee::add(z, 50.0) };
            fx.mem.setf(sp + 0x68, z);
            let p = [fx.mem.f(sp + 0x60), fx.mem.f(sp + 0x64), fx.mem.f(sp + 0x68)];
            let mut x = fx.mem.read_u64(sp + 0xE8);
            if let Some(w) = fx.project(p, None, &mut x) {
                fx.mem.write_u64(sp + 0xE8, x);
                fx.mem.setf(TINT_W, w);
            }
            crate::b590::project_residue(fx, sp, sp + 0x60);
            for kind in 0..3 {
                if fx.mem.i16(t + 0x50) == kind {
                    let w = fx.mem.f(TINT_W);
                    quads4::tint4(fx, t, 0x21, w);
                }
            }
        }
        _ => {}
    }
}

/// D+0x250 = the attacker's node at `at`.
fn place(fx: &mut Fx, at: u32) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(at);
    fx.node_position(h, slot, POS);
}

/// 0x1E01AD0(D, 0): the update counter and the emitters (frame 0x40: ra,
/// the entry's s2 = param, s1 = D, s0 = 0).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_30D0), (0x20, param as i64 as u64), (0x10, u64::from(D)), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    if (0x2A..0xFF).contains(&c(fx)) {
        place(fx, NODE_A);
        crate::b590::spawn(fx, RING_A);
    }
    if c(fx) >= 0x2C {
        let p = fx.mem.i16(PULL);
        if p < 0x32 {
            fx.mem.set16(PULL, p + 1);
        }
    }
    if (0x52..0x104).contains(&c(fx)) {
        place(fx, NODE_A);
        let mut k = 0x12;
        for _ in 1..0xA {
            fx.mem.set16(SPAWN_K, k);
            crate::b590::spawn(fx, RING_B);
            k += 6;
        }
    }
    if c(fx) == 0x100 {
        place(fx, NODE_B);
        let h = fx.target_handle();
        fx.unit_position(h, POS);
        for _ in 1..0xF {
            crate::b590::spawn(fx, SPARKS);
        }
        fx.mem.set16(SPAWN_K, 0);
        crate::b590::spawn(fx, TINT);
        for (a, k) in [(0, 0), (0x5A, 0), (0x2D, 1), (0x87, 1)] {
            fx.mem.set16(SPAWN_A, a);
            fx.mem.set16(SPAWN_K, k);
            crate::b590::spawn(fx, COLUMN);
        }
    }
    fx.leave(0x40);
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

fn young(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6)
}

fn add(fx: &mut Fx, at: u32, by: f32) {
    let v = ee::add(fx.mem.f(at), by);
    fx.mem.setf(at, v);
}

/// +0x10 = node 0x31's point pulled toward node 0x32's by +0x52 / 50 (the
/// step's local sp + 0x30), the frames 0x1F6830 leaves (s0 = the task,
/// s1 = D).
fn pull(fx: &mut Fx, t: u32, sp: u32, ra: [u32; 2]) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODE_A);
    crate::b076::node_position_saving(fx, h, slot, t + 0x10, u64::from(t), Some(u64::from(D)), ra[0]);
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODE_B);
    crate::b076::node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), ra[1]);
    for k in 0..3u32 {
        let v = ee::div(ee::sub(fx.mem.f(sp + 0x30 + 4 * k), fx.mem.f(t + 0x10 + 4 * k)), 50.0);
        fx.mem.setf(sp + 0x30 + 4 * k, v);
    }
    let f = ee::from_i32(fx.mem.i16(t + 0x52));
    for k in 0..3u32 {
        let v = ee::mul(fx.mem.f(sp + 0x30 + 4 * k), f);
        add(fx, t + 0x10 + 4 * k, v);
    }
}

/// 0x1E01DE0 (D, task): a pink quad (size 10 growing to 60, cell 0..3
/// cycling) facing the camera and spinning by its random +0..+4, pulled
/// along the node line for 30..49 updates; in by 0x40 for 32 updates, out
/// by 9.
fn quad(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0xA);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_1E18);
        let h = fx.div(v >> 4, 3).1;
        fx.mem.set16(t + 0x56, h);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        for (o, ra) in [(0x34u32, 0x01E0_1E4Cu32), (0x30, 0x01E0_1E78), (0x38, 0x01E0_1EA4)] {
            let v = rmod(fx, t, ra, 0x64);
            fx.mem.setf(t + o, ee::from_i32(v - 0x32));
        }
        for o in [0x34u32, 0x30, 0x38] {
            let v = ee::div(fx.mem.f(t + o), 60.0);
            fx.mem.setf(t + o, v);
        }
        let v = rmod(fx, t, 0x01E0_1EFC, 0x168);
        fx.mem.set16(t + 0x5C, v);
        crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1F18);
        for (o, ra) in [(4u32, 0x01E0_1F20u32), (2, 0x01E0_1F40), (0, 0x01E0_1F60)] {
            let v = fx.rand_from(t, ra);
            fx.mem.set16(t + o, v % 0x800 - 0x400);
        }
        let v = fx.rand_from(t, 0x01E0_1F80);
        fx.mem.set16(t + 6, v % 8);
        let v = rmod(fx, t, 0x01E0_1F9C, 0x14);
        fx.mem.set16(t + 0xE, v + 0x1E);
        fx.mem.set16(t + 0x52, 0xA);
    }
    if fx.mem.i16(t + 0x54) < 0x20 {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9, true);
    }
    let c = fx.mem.i16(t + 0x56) + 1;
    fx.mem.set16(t + 0x56, c);
    if fx.mem.i16(t + 0x56) >= 4 {
        fx.mem.set16(t + 0x56, 0);
    }
    let s = fx.mem.i16(t + 8);
    if s < 0x3C {
        fx.mem.set16(t + 8, s + 1);
    }
    let s = fx.mem.i16(t + 8);
    fx.mem.set16(t + 0xA, s);
    for (a, d) in [(0x28u32, 4u32), (0x24, 2), (0x20, 0)] {
        let v = fx.mem.i32(t + a).wrapping_add(fx.mem.i16(t + d));
        fx.mem.set32(t + a, v);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 0xE) {
        pull(fx, t, sp, [0x01E0_20B0, 0x01E0_20D0]);
        let n = fx.mem.i16(t + 0x52) + 1;
        fx.mem.set16(t + 0x52, n);
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2180);
    age(fx, t);
}

/// 0x1E021B0 (D, task): a red ring (size 5..14 growing by 0.2..0.9 while
/// +0x52 is under D+0xB2B8, cell 0..2 cycling to 7) at node 0x31 pulled
/// toward node 0x32 by +0x52 (10, + 1 to D+0xB2B8) / 50; life 25..27; in by
/// 0x40, out by 0xE; ends on update 260.
fn ring_a(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_21DC, 0xA);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 5));
        let v = rmod(fx, t, 0x01E0_2208, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x30, 0x30);
        let v = rmod(fx, t, 0x01E0_2234, 0xA);
        fx.mem.set16(t + 0x50, v + 5);
        let v = rmod(fx, t, 0x01E0_2254, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_2270, 0x14);
        fx.mem.set16(t + 0xE, v + 0x10);
        let v = rmod(fx, t, 0x01E0_2290, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_22AC, 3);
        fx.mem.set16(t + 6, v + 0x19);
        fx.mem.set16(t + 0x52, 0xA);
        fx.mem.set16(t + 0x58, 2);
    }
    if young(fx, t) {
        fade_in(fx, t, 0x63, 0xC0, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0x10, 0xE, true);
    }
    cycle8(fx, t);
    pull(fx, t, sp, [0x01E0_2368, 0x01E0_2388]);
    let n = fx.mem.i16(t + 0x52);
    if n < fx.mem.i16(PULL) {
        fx.mem.set16(t + 0x52, n + 1);
    }
    let v = fx.rand_from(t, 0x01E0_244C);
    let grow = ee::div(ee::from_i32(v % 8 + 2), 10.0);
    if fx.mem.i16(t + 0x54) != 0 && fx.mem.i16(t + 0x52) < fx.mem.i16(PULL) {
        add(fx, t + 0x48, grow);
    }
    if fx.mem.i16(COUNTER) >= 0x104 {
        fx.mem.set16(t + 0x58, 0);
    }
    age(fx, t);
}

fn cycle8(fx: &mut Fx, t: u32) {
    let c = fx.mem.i16(t + 0x56) + 1;
    fx.mem.set16(t + 0x56, c);
    if fx.mem.i16(t + 0x56) >= 8 {
        fx.mem.set16(t + 0x56, 0);
    }
}

/// 0x1E02500 (D, task): a red ring (size 8..27 growing by (rand % (+0x52
/// / 3) + 2) / 40) set on the node line at +0x52 (the emitter's 18..66) /
/// 50 on its first update; radius 5..14 growing by 2..4 on odd updates;
/// life 2..4.
fn ring_b(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_252C, 0x14);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 8));
        let v = rmod(fx, t, 0x01E0_2558, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x30, 0x30);
        let v = rmod(fx, t, 0x01E0_2584, 0xA);
        fx.mem.set16(t + 0x50, v + 5);
        let v = rmod(fx, t, 0x01E0_25A4, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_25C0, 3);
        fx.mem.set16(t + 0xE, v + 2);
        let v = rmod(fx, t, 0x01E0_25E0, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_25FC, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    if young(fx, t) {
        fade_in(fx, t, 0x63, 0xC0, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0x10, 0xE, true);
    }
    cycle8(fx, t);
    if fx.mem.i16(t + 0x54) == 0 {
        pull(fx, t, sp, [0x01E0_26BC, 0x01E0_26DC]);
    }
    let v = fx.rand_from(t, 0x01E0_277C);
    let n = fx.mem.i16(t + 0x52);
    fx.mult(0x5555_5556, n);
    let third = ((i64::from(0x5555_5556i32) * i64::from(n)) >> 32) as i32 + ((n as u32) >> 31) as i32;
    let r = fx.div(v, third).1;
    let grow = ee::div(ee::from_i32(r + 2), 40.0);
    if fx.mem.i16(t + 0x54) != 0 {
        add(fx, t + 0x48, grow);
    }
    let a = fx.mem.i16(t + 0x54);
    if a != 0 && a % 2 != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E02850 (D, task): a spark burst (size 200 growing by 15..22.9 a
/// update, +0x4C a tenth of it; red on black or pink on grey by (rand >>
/// 4) % 2), random angles; 0x80 for 8 updates, out by 5.
fn sparks(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4348_0000);
        fx.mem.setf(t + 0x4C, ee::sub(200.0, 58.0));
        let v = fx.rand_from(t, 0x01E0_2890);
        fx.mem.set16(t + 0x50, (v >> 4) % 2);
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0xC0, 0x20, 0x20);
            l2::rgb(fx, t + 0x64, 0, 0, 0);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0xFF, 0xA0, 0xA0);
            l2::rgb(fx, t + 0x64, 0x30, 0x30, 0x30);
        }
        fx.mem.set16(t + 0x5A, 0x3C);
        fx.mem.set16(t + 0x5C, 0x3C);
        let v = rmod(fx, t, 0x01E0_2928, 0x50);
        fx.mem.setf(t + 0x34, ee::from_i32(v + 0x96));
        let v = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        for (o, ra) in [(0x20u32, 0x01E0_2970u32), (0x24, 0x01E0_298C), (0x28, 0x01E0_29A8)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
    }
    if fx.mem.i16(t + 0x54) < 8 {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade_out(fx, t, 0x63, 7, 5, true);
    }
    let s = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, s);
    fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(0x3DCC_CCCD), s));
    age(fx, t);
}

/// 0x1E02A50 (D, task): a light column (size 6 / 18, height 100 / 500 /
/// 1000 by +0x50 growing by 5 / 20 / 60, turning by 6); in by 0x60 under
/// 0xC0 for 4 updates, out by 0xD.
fn column(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0x60, 0x60);
        l2::rgb(fx, t + 0x64, 0xFF, 0x40, 0x40);
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.write_u32(t + 0x30, 0x4190_0000);
            fx.mem.write_u32(t + 0x34, 0x447A_0000);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.write_u32(t + 0x30, 0x4190_0000);
            fx.mem.write_u32(t + 0x34, 0x43FA_0000);
        }
        if fx.mem.i16(t + 0x50) == 2 {
            fx.mem.write_u32(t + 0x30, 0x40C0_0000);
            fx.mem.write_u32(t + 0x34, 0x42C8_0000);
            l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xC0);
            l2::rgb(fx, t + 0x64, 0xFF, 0x40, 0x40);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 4 {
        fade_in(fx, t, 0x63, 0xC0, 0x60);
    } else {
        fade_out(fx, t, 0x63, 0xF, 0xD, true);
    }
    for (kind, by) in [(0, 60.0f32), (1, 20.0), (2, 5.0)] {
        if fx.mem.i16(t + 0x50) == kind {
            if fx.mem.i16(t + 0x54) != 0 {
                add(fx, t + 0x34, by);
            }
            let a = fx.mem.i16(t + 0x5A) + 6;
            fx.mem.set16(t + 0x5A, a);
        }
    }
    age(fx, t);
}

/// 0x1E02C60 (D, task): a full-window tint (pink, blue or grey by +0x50;
/// 8 / 8 / 22 updates) in by 0x40 under 0x80, out by 4.
fn tint(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0x9F, 0x5F, 0x5F);
            l2::rgb(fx, t + 0x64, 0x9F, 0x5F, 0x5F);
            fx.mem.set16(t + 6, 8);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0, 0x30, 0xF0);
            l2::rgb(fx, t + 0x64, 0x30, 0, 0xF0);
            fx.mem.set16(t + 6, 8);
        }
        if fx.mem.i16(t + 0x50) == 2 {
            l2::rgb(fx, t + 0x60, 0x58, 0x58, 0x78);
            l2::rgb(fx, t + 0x64, 0x58, 0x58, 0x78);
            fx.mem.set16(t + 6, 0x16);
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if young(fx, t) {
        fade_in(fx, t, 0x63, 0x80, 0x40);
        fade_in(fx, t, 0x67, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 6, 4, true);
        fade_out(fx, t, 0x67, 6, 4, false);
    }
    age(fx, t);
}
