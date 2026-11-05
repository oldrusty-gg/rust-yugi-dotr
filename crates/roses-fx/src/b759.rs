//! BMODULE b759 (card 132), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03020) [F]. The entry
//! takes (D, param). A pool of 300 tasks of 0x70 bytes at D+0x2A0 (alive
//! +0x58, step +0x44 called through `jalr`), spawned by 0x1E03880, drawn
//! with the second library: rings of camera-facing quads (98b02c86a827 at
//! 0x1E00D80, `billboard::ring_facing`), rings of quads (7369422df7f8 at
//! 0x1E012F0, `billboard::ring_quads`), 36-strip bands (3fb2be4df9d4 at
//! 0x1E00920, `quads4::ring_band`), a full-window tint (561a6131a635 at
//! 0x1E018A0, `quads4::tint4`) and single quads (add5d81dd3fe at
//! 0x1E01A70, `billboard::quad`), UVs by uv_rect 0x1E00080. Update 1: a
//! cyan tint keyed by the attacker's point moved to x = -10 eye x (290
//! updates); from update 163 every update three smoke quads at the
//! attacker's node 0x3D, from 166 on odd updates three swirling clouds
//! at node 1 that rise and drift toward the target, each bursting into
//! three red rings at update 83 of its life. Returns 1 on update 160, 3
//! on 320, 6 on 171..314, 8 once the 330-update timer D+0x8630 has run
//! out.
//!
//! The steps 0x1E02980 (yellow quad rings) and 0x1E02B80 (a widening
//! band) are never spawned [F: the spawn `jal`s name 0x1E01FE0,
//! 0x1E027E0, 0x1E02C90 and 0x1E02D90]: not ported, nor their draw cases
//! (`ring_quads` / `ring_band`).
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 300;
const POS: u32 = D + 0x250;
const TIMER: u32 = D + 0x8630;
const COLOUR: u32 = D + 0x8634;
/// 0x1FACE0(0)'s pointer (the camera's eye).
const EYE: u32 = D + 0x8644;
const TINT_W: u32 = D + 0x8654;
const INDEX: u32 = D + 0x8696;
const CURSOR: u32 = D + 0x8698;
const YAW: u32 = D + 0x869A;
const COUNTER: u32 = D + 0x869E;
const BYTES_A: u32 = D + 0x86A0;
const BYTES_B: u32 = D + 0x87A0;
const TAB: u32 = D + 0x88A0;

const CLOUD: u32 = 0x01E0_1FE0;
const RING: u32 = 0x01E0_27E0;
const TINT: u32 = 0x01E0_2C90;
const SMOKE: u32 = 0x01E0_2D90;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_33A0;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
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

/// The camera eye's x (through the pointer 0x1FACE0 gave) <= 0.
fn eye_left(fx: &mut Fx) -> bool {
    let p = fx.mem.read_u32(EYE);
    fx.mem.f(p) <= 0.0
}

/// The entry 0x1E03020 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x140);
    fx.mem.write_u32(D + 0x863C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let e = fx.camera_eye(0);
    fx.mem.write_u32(EYE, e);
    let a = fx.camera_at30(0);
    fx.mem.write_u32(D + 0x8648, a);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x140);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x8A10);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x869C, param);
    fx.mem.set32(TIMER, 0x14A);
    for (h, n, clear, at) in [(0, 1, 1, 0x8668u32), (0, 0x2F, 0, 0x866A), (0, 0x3D, 0, 0x866C), (1, 1, 1, 0x8678)] {
        let h = if h == 0 { fx.self_handle() } else { fx.target_handle() };
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(D + at, r);
    }
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..3u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x8610 + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.fix_residue(y, 0x01E0_31DC);
    fx.mem.set16(YAW, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_320C, 0x1E);
        fx.mem.set8(BYTES_A + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_3264);
        fx.mem.set8(BYTES_A + k, v % 4);
    }
    for k in 0xB4..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_32C4);
        fx.mem.set8(BYTES_A + k, v % 16 - 8);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_331C, 0x28);
        fx.mem.set8(BYTES_B + k, v - 0x14);
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
        if f == 0 {
            continue;
        }
        // The step's prologue: ra, the entry's s1 = 0 (0x60 frame), s0 = D.
        let frame = if f == CLOUD { 0x60 } else { 0x20 };
        let s = fx.enter(frame);
        if f == CLOUD {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
        } else {
            fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]);
        }
        match f {
            CLOUD => cloud(fx, t, s),
            RING => ring(fx, t),
            TINT => tint(fx, t),
            SMOKE => smoke(fx, t),
            _ => panic!("b759: unported task step {f:#x}"),
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
    let n = fx.mem.i32(TIMER);
    if n != 0 {
        fx.mem.set32(TIMER, n - 1);
    }
    let c = fx.mem.i16(COUNTER);
    if c == 0xA0 {
        1
    } else if c == 0x140 {
        3
    } else if (0xAB..0x13B).contains(&c) {
        6
    } else if fx.mem.i32(TIMER) > 0 {
        0
    } else {
        8
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
        CLOUD => {
            let c = fx.mem.i16(t + 0x56);
            if c < 3 {
                fx.mem.set16(D + 0x94, (c << 6) + 1);
                fx.mem.set16(D + 0x96, 1);
                fx.mem.set16(D + 0xC0, 0);
            } else {
                let r = fx.div(c, 3).1;
                fx.mem.set16(D + 0x94, (r << 6) + 1);
                fx.mem.set16(D + 0x96, 0x41);
                let left = eye_left(fx);
                let o = if fx.mem.i16(YAW) != 0 { if left { 6 } else { 5 } } else if left { 5 } else { 6 };
                fx.mem.set16(D + 0xC0, o);
            }
            fx.mem.set16(D + 0x98, 0x3F);
            fx.mem.set16(D + 0x9A, 0x3F);
            let w = fx.mem.read_u32(D + 0x8610);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 1);
            let x = fx.mem.read_u32(t + 0x10);
            fx.mem.write_u32(D + 0x20, x);
            let y = ee::add(fx.mem.f(t + 0x14), fx.mem.f(t + 0x34));
            fx.mem.setf(D + 0x24, y);
            let z = fx.mem.read_u32(t + 0x18);
            fx.mem.write_u32(D + 0x28, z);
            fx.mem.set16(D + 0xBE, 0);
            billboard::ring_facing(fx, uv::uv_rect, D, t, 0x10, BYTES_A);
            fx.mem.set16(D + 0xB8, 0);
        }
        RING => {
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x70, 0x9B, c);
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 6);
            fx.mem.set16(D + 0xC0, 0);
            for k in 0..3u32 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, w);
            }
            billboard::ring_facing(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
            fx.mem.set16(D + 0xB8, 0);
        }
        TINT => {
            let h = fx.self_handle();
            fx.unit_position(h, sp + 0x60);
            // 0x1F71A0's frame below the entry's (its saved ra, s0 = D).
            fx.saved(sp - 0x20, &[(0x10, 0x01E0_36C8), (0, u64::from(D))]);
            let p = fx.mem.read_u32(EYE);
            let x = ee::mul(10.0, ee::neg(fx.mem.f(p)));
            fx.mem.setf(sp + 0x60, x);
            let v = [fx.mem.f(sp + 0x60), fx.mem.f(sp + 0x64), fx.mem.f(sp + 0x68)];
            let mut xy = fx.mem.read_u64(sp + 0xC8);
            if let Some(w) = fx.project(v, None, &mut xy) {
                fx.mem.write_u64(sp + 0xC8, xy);
                fx.mem.setf(TINT_W, w);
            }
            crate::b590::project_residue(fx, sp, sp + 0x60);
            let w = fx.mem.f(TINT_W);
            quads4::tint4(fx, t, 1, w);
        }
        SMOKE => {
            let w = fx.mem.read_u32(D + 0x8618);
            fx.mem.write_u32(D + 0x88, w);
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x90, 0x14, c);
            let o = fx.mem.i16(t + 0x52);
            fx.mem.set16(D + 0xBE, o);
            billboard::quad(fx, uv::uv_rect, D, t, 1, t + 0x10);
        }
        _ => {}
    }
}

/// 0x1E03880(D, step, a2): a new task (first free of 300) with the spawn
/// colour D+0x8634 (both colours), at D+0x250 (i16 +0 and float +0x10),
/// index D+0x8696 (+0x50).
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
        let s = fx.mem.i16(INDEX);
        fx.mem.set16(t + 0x50, s);
        fx.mem.write_u32(t + 0x40, 0);
        fx.mem.write_u32(t + 0x44, step);
    }
    fx.leave(0x60);
}

/// 0x1E01D30(D, 0): the update counter and the emitters (frame 0x40: ra,
/// the entry's s2 = param, s1 = 0, s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_3370), (0x20, param as i64 as u64), (0x10, 0), (0, u64::from(D))]);
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    if c(fx) == 1 {
        spawn(fx, TINT);
    }
    let n = c(fx);
    if (0xA6..0x100).contains(&n) && n % 2 == 1 {
        for _ in 0..3 {
            let h = fx.self_handle();
            let slot = fx.mem.i16(D + 0x8668);
            fx.node_position(h, slot, POS);
            let i = fx.mem.i16(CURSOR);
            fx.mem.set16(INDEX, i);
            spawn(fx, CLOUD);
            let i = fx.mem.i16(CURSOR) + 1;
            fx.mem.set16(CURSOR, i);
            if fx.mem.i16(CURSOR) >= 0x78 {
                fx.mem.set16(CURSOR, 0);
            }
        }
    }
    let n = c(fx);
    if (0xA3..0x100).contains(&n) {
        for s0 in 0..3u32 {
            let h = fx.self_handle();
            let slot = fx.mem.i16(D + 0x866C);
            fx.node_position(h, slot, POS);
            for (k, ra) in [0x01E0_1EFCu32, 0x01E0_1F30, 0x01E0_1F64].into_iter().enumerate() {
                let r = rmod(fx, s0, ra, 0xA);
                let a = POS + 4 * k as u32;
                let v = ee::add(fx.mem.f(a), ee::from_i32(r - 5));
                fx.mem.setf(a, v);
            }
            spawn(fx, SMOKE);
        }
    }
    fx.leave(0x40);
}

/// 0x1E01FE0 (D, task): a swirling cloud (size 5, growing; cell 5 / 3
/// alternating, later 0..2 cycling) from node 1: in by 0x20 to 0x80 until
/// update 305, radius +0x50 growing by 15 to 100..449, then out by 8;
/// drifts by +0x30 / +0x38 on updates 21..29 and toward the target by
/// +0x3C (z), sinking to 10 above it; bursts into three red rings on
/// update 83.
fn cloud(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        let v = fx.rand_from(t, 0x01E0_2010);
        fx.mem.set16(t + 0x56, (v >> 4) % 2);
        let c = if fx.mem.i16(t + 0x56) == 0 { 5 } else { 3 };
        fx.mem.set16(t + 0x56, c);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, 0x01E0_2068, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let left = eye_left(fx);
        let a = if fx.mem.i16(YAW) != 0 { if left { -0x2D } else { 0x2D } } else if left { 0x2D } else { -0x2D };
        fx.mem.set16(t + 0x5C, a);
        let v = fx.rand_from(t, 0x01E0_2108);
        fx.mem.set16(t + 0xA, if v % 2 != 0 { 2 } else { 0 });
        let i = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, i);
        fx.mem.set16(t + 0x50, 0);
        let v = rmod(fx, t, 0x01E0_2144, 0x15E);
        fx.mem.set16(t + 0x52, v + 0x64);
        let v = rmod(fx, t, 0x01E0_2164, 0x14);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = rmod(fx, t, 0x01E0_2184, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_21A0, 5);
        fx.mem.set16(t + 6, v + 3);
        for (o, ra) in [(0x30u32, 0x01E0_21C0u32), (0x34, 0x01E0_21EC), (0x38, 0x01E0_2218)] {
            let v = rmod(fx, t, ra, 0x28);
            fx.mem.setf(t + o, ee::from_i32(v - 0x14));
        }
        for o in [0x30u32, 0x34, 0x38] {
            let v = ee::div(fx.mem.f(t + o), 38.0);
            fx.mem.setf(t + o, v);
        }
        let h = fx.target_handle();
        crate::b076::unit_position_saving(fx, h, sp + 0x30, u64::from(t), 0x01E0_227C);
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + 0x8668);
        crate::b076::node_position_saving(fx, h, slot, sp + 0x40, u64::from(t), Some(u64::from(D)), 0x01E0_229C);
        let dz = ee::div(ee::sub(fx.mem.f(sp + 0x38), fx.mem.f(sp + 0x48)), 80.0);
        fx.mem.setf(t + 0x3C, dz);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(COUNTER) < 0x131 {
        fade_in(fx, t, 0x63, 0x80, 0x20);
        let r = fx.mem.i16(t + 0x50);
        if r < fx.mem.i16(t + 0x52) {
            fx.mem.set16(t + 0x50, r + 0xF);
        }
    } else {
        fade_out(fx, t, 0x63, 0xA, 8, true);
    }
    let a = fx.mem.i16(t + 0x54);
    if (0x15..0x1E).contains(&a) {
        let x = ee::add(fx.mem.f(t + 0x10), fx.mem.f(t + 0x30));
        fx.mem.setf(t + 0x10, x);
        let z = ee::add(fx.mem.f(t + 0x18), fx.mem.f(t + 0x38));
        fx.mem.setf(t + 0x18, z);
        let s = ee::add(fx.mem.f(t + 0x48), f32::from_bits(0x3E99_999A));
        fx.mem.setf(t + 0x48, s);
    }
    if fx.mem.i16(t + 0x54) == 0x53 && fx.mem.i16(COUNTER) < 0x12C {
        let ras: [[u32; 3]; 3] = [[0x01E0_23D4, 0x01E0_2408, 0x01E0_243C], [0x01E0_24A0, 0x01E0_24D4, 0x01E0_2508], [0x01E0_256C, 0x01E0_25A0, 0x01E0_25D4]];
        for r in ras {
            for k in 0..4 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(POS + 4 * k, w);
            }
            for (k, ra) in r.into_iter().enumerate() {
                let v = fx.rand_from(t, ra);
                let a = POS + 4 * k as u32;
                let x = ee::add(fx.mem.f(a), ee::from_i32(v % 16 - 8));
                fx.mem.setf(a, x);
            }
            spawn(fx, RING);
        }
    }
    let c = fx.mem.i16(COUNTER);
    let cyc = |fx: &mut Fx| {
        let v = fx.mem.i16(t + 0x56) + 1;
        fx.mem.set16(t + 0x56, v);
        if fx.mem.i16(t + 0x56) >= 3 {
            fx.mem.set16(t + 0x56, 0);
        }
    };
    if c >= 0xF6 {
        if c == 0xF6 {
            let v = fx.rand_from(t, 0x01E0_2640);
            let r = fx.div(v >> 4, 3).1;
            fx.mem.set16(t + 0x56, r);
            fx.mem.set16(t + 0x5C, 0);
        }
        cyc(fx);
    } else if fx.mem.i16(t + 0x54) < 0x50 {
        let v = if fx.mem.i16(t + 0x56) == 5 { 3 } else { 5 };
        fx.mem.set16(t + 0x56, v);
    } else {
        cyc(fx);
    }
    let h = fx.target_handle();
    crate::b076::unit_position_saving(fx, h, sp + 0x50, u64::from(t), 0x01E0_26EC);
    let a = fx.mem.i16(t + 0x5C);
    let cs = l2::sin_deg(fx, TAB, a, 1);
    fx.mem.setf(t + 0x34, ee::mul(3.0, cs));
    if fx.mem.i16(t + 0x54) < 0x50 {
        let z = ee::add(fx.mem.f(t + 0x18), fx.mem.f(t + 0x3C));
        fx.mem.setf(t + 0x18, z);
        let floor = ee::sub(fx.mem.f(sp + 0x54), 10.0);
        let y = fx.mem.f(t + 0x14);
        if floor < y {
            fx.mem.setf(t + 0x14, ee::sub(y, f32::from_bits(0x3ECC_CCCD)));
        }
        let s = ee::add(fx.mem.f(t + 0x48), 0.5);
        fx.mem.setf(t + 0x48, s);
    } else {
        let y = ee::add(fx.mem.f(t + 0x14), f32::from_bits(0x3ECC_CCCD));
        fx.mem.setf(t + 0x14, y);
    }
    age(fx, t);
}

/// 0x1E027E0 (D, task): a red ring (size 5, cell 0..3) of radius 10..59
/// growing by 16..45, life 3..7; 0x80, then out by 8.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        let v = fx.rand_from(t, 0x01E0_2808);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, 0x80, 0x10, 0x10);
        let v = rmod(fx, t, 0x01E0_283C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, 0x01E0_2858);
        fx.mem.set16(t + 0xA, v % 8);
        let i = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, i);
        let v = rmod(fx, t, 0x01E0_287C, 0x32);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, 0x01E0_289C, 0x1E);
        fx.mem.set16(t + 0xE, v + 0x10);
        let v = rmod(fx, t, 0x01E0_28BC, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_28D8, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade_out(fx, t, 0x63, 0xA, 8, true);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E02C90 (D, task): the cyan tint: in by 0x19 to 0x80 (both colours)
/// for 290 updates, then out by 4.
fn tint(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x60, 0x20, 0xF0, 0xF0);
        l2::rgb(fx, t + 0x64, 0x20, 0xF0, 0xF0);
        fx.mem.set16(t + 6, 0x122);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, 0x80, 0x19);
        fade_in(fx, t, 0x67, 0x80, 0x19);
    } else {
        fade_out(fx, t, 0x63, 6, 4, true);
        fade_out(fx, t, 0x67, 6, 4, false);
    }
    age(fx, t);
}

/// 0x1E02D90 (D, task): a dark smoke quad (size 40, cell 2..4) drifting
/// by +0x30.. (random / 25 or / 45 by the index +0x50), facing the camera
/// (0x205580 into +0x20 every update), turned randomly, UV orientation
/// 0..3; in by 0x40 under 0x80 for 4 updates, out by 4.
fn smoke(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x28);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_2DC0);
        let r = fx.div(v >> 4, 3).1;
        fx.mem.set16(t + 0x56, r + 2);
        l2::rgb(fx, t + 0x60, 0x18, 0x18, 0x18);
        fx.mem.set16(t + 0x58, 2);
        for (o, ra) in [(0x34u32, 0x01E0_2DF8u32), (0x30, 0x01E0_2E24), (0x38, 0x01E0_2E50)] {
            let v = rmod(fx, t, ra, 0x64);
            fx.mem.setf(t + o, ee::from_i32(v - 0x32));
        }
        let i = fx.mem.i16(t + 0x50);
        let by = if i == 0 { Some(25.0) } else if i == 1 { Some(45.0) } else { None };
        if let Some(by) = by {
            for o in [0x34u32, 0x30, 0x38] {
                let v = ee::div(fx.mem.f(t + o), by);
                fx.mem.setf(t + o, v);
            }
        }
        let v = rmod(fx, t, 0x01E0_2F04, 0x168);
        fx.mem.set16(t + 0x5C, v);
        crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2F20);
        for (o, ra) in [(4u32, 0x01E0_2F28u32), (2, 0x01E0_2F48), (0, 0x01E0_2F68)] {
            let v = fx.rand_from(t, ra);
            fx.mem.set16(t + o, v % 0x800 - 0x400);
        }
        let v = fx.rand_from(t, 0x01E0_2F88);
        fx.mem.set16(t + 0x52, (v >> 4) % 4);
    }
    if fx.mem.i16(t + 0x54) < 4 {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 6, 4, true);
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2FFC);
    age(fx, t);
}
