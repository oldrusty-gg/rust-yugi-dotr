//! BMODULE b041 (card 61), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E022B0, data area 0x1ED0000, 0x8AF0 bytes) [F]
//!: b259's program shape (here 170 tasks of 0x70
//! bytes at D+0x2A0, alive +0x58, step +0x44 called through `jalr`;
//! spawned by 0x1E02BE0, also card 76's) drawn with the second library:
//! rings of quads (7369422df7f8 at 0x1E00740 with uv_rect_q 0x1E00080),
//! turned quads (`quads4::quad_turned` 0x1E00CF0 / `quad_turned_f`
//! 0x1E00FD0) and a screen quad keyed at the camera's look point
//! (0x1E004D0, [`screen_quad`], also card 76's). Every update first
//! stores the camera's +0x20 / +0x30 (0x1FACE0 / 0x1FAF40) at D+0x870C /
//! D+0x8710. Update 1: the stage light (0x1E01FC0) twice, two dark screen
//! shades (0x1E01DF0, kinds 0 / 1 scrolling their texture row) and a
//! breathing flare at node 1 (0x1E01820, never ending); 0x5D a grey frame
//! flash (0x1E020B0); 0x5D..0xAF at node 1 and 0x96..0x113 at the
//! target's node 1 (each jittered by 15) a reddish ring a update
//! (0x1E01C40); 0x5D..0xAA every third update a spark at node 1 (0x1E01940)
//! and 0x96..0x104 a shrinking rise at the target (0x1E01A70). The step
//! 0x1E01660 (a violet ring at D+0x86C0) is never spawned but drawn by the
//! entry (ported). Returns 1 on update 0x99, 4 on 0x122, 2 on 0x118, 6 on
//! 0xA4..0x112, 8 once the 300-update timer D+0x8700 has run out.
use crate::b076::node_position_saving;
use crate::b590::project_residue;
use crate::blib::billboard::{self, uv_rect_q};
use crate::blib::quads4;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0xAA;
const RING_O: u32 = 0x01E0_1660;
const FLARE: u32 = 0x01E0_1820;
const SPARK: u32 = 0x01E0_1940;
const RISE: u32 = 0x01E0_1A70;
const RING: u32 = 0x01E0_1C40;
const SHADE: u32 = 0x01E0_1DF0;
const LIGHT: u32 = 0x01E0_1FC0;
const FRAME: u32 = 0x01E0_20B0;
/// The return address of the entry's `jalr` into a step (0x1E02558 + 8).
const TASK_RA: u64 = 0x01E0_2560;
const POS: u32 = D + 0x250;
const TEX: u32 = D + 0x86E0;
const TIMER: u32 = D + 0x8700;
const COLOUR: u32 = D + 0x8704;
const EYE: u32 = D + 0x870C;
const AT30: u32 = D + 0x8710;
/// The look point's w (0x204D80's out).
const LOOK_W: u32 = D + 0x871C;
const NODES: u32 = D + 0x8740;
const TARGET_NODE: u32 = D + 0x8750;
const SPAWN_K: u32 = D + 0x876E;
const FACING: u32 = D + 0x8772;
const COUNTER: u32 = D + 0x8776;
const BYTES: u32 = D + 0x8778;
const TAB: u32 = D + 0x8978;
const ALPHA_42: u64 = 0x80_0000_0042;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E022B0 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x160);
    fx.mem.write_u32(D + 0x8714, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let e = fx.camera_eye(0);
    fx.mem.write_u32(EYE, e);
    let a = fx.camera_at30(0);
    fx.mem.write_u32(AT30, a);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x160);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x8AF0);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x8774, param);
    fx.mem.set32(TIMER, 0x12C);
    for (k, node) in [1, 0x17, 0x36, 0x15, 0x27].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xC0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xC4)), 180.0);
    fx.fix_residue(y, 0x01E0_24AC);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    // rand's frames: the entry's s0 = D.
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_24DC, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
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
        // The step's prologue: the entry's s0 = D, s1 = 0.
        let frame = match f {
            0 => continue,
            RING_O | FLARE | RISE | SHADE => 0x30,
            SPARK | RING => 0x20,
            LIGHT => 0x10,
            FRAME => 0x90,
            _ => panic!("b041: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        match frame {
            0x10 => fx.saved(s, &[(0, TASK_RA)]),
            0x20 => fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]),
            _ => fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]),
        }
        match f {
            RING_O => ring_o(fx, t),
            FLARE => flare(fx, t),
            SPARK => spark(fx, t),
            RISE => rise(fx, t),
            RING => ring(fx, t),
            SHADE => shade(fx, t),
            LIGHT => light(fx, t),
            _ => frame_flash(fx, t, s),
        }
        fx.leave(frame);
    }
    let mut alive = 0u32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        alive += 1;
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        if f(fx) == RING_O {
            tex(fx, D + 0x29C);
            cell(fx, t, sp + 0x70);
            fx.mem.set16(D + 0xB8, 5);
            let v = fx.rand_from(D, 0x01E0_2608);
            fx.mem.set16(D + 0xBE, v % 4);
            fx.mem.set16(D + 0xB0, 0);
            for k in 0..4 {
                let w = fx.mem.read_u32(D + 0x86C0 + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            billboard::ring_quads(fx, uv_rect_q, D, t, 0x21, D + 0x8878);
            fx.mem.set16(D + 0xB8, 0);
        }
        // 0x1F6830's frame below the entry's: s0 = D, s1 = the live count.
        for (g, local, ra) in [(FLARE, 0x80u32, 0x01E0_26FCu32), (SPARK, 0x90, 0x01E0_2804), (RISE, 0xA0, 0x01E0_290C)] {
            if f(fx) != g {
                continue;
            }
            tex(fx, TEX + 8);
            cell(fx, t, sp + local);
            fx.mem.set16(D + 0xBE, 0);
            for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x80), (0x9A, 0xFF)] {
                fx.mem.set16(D + o, v);
            }
            let p = sp + 0x60;
            if g == RISE {
                let h = fx.target_handle();
                let slot = fx.mem.i16(TARGET_NODE);
                node_position_saving(fx, h, slot, p, u64::from(D), Some(u64::from(alive)), ra);
                for k in 0..3 {
                    let v = ee::add(fx.mem.f(p + 4 * k), fx.mem.f(t + 0x30 + 4 * k));
                    fx.mem.setf(p + 4 * k, v);
                }
                quads4::quad_turned(fx, uv_rect_q, D, t, 0x21, p);
            } else {
                let h = fx.self_handle();
                let slot = fx.mem.i16(NODES);
                node_position_saving(fx, h, slot, p, u64::from(D), Some(u64::from(alive)), ra);
                let z = fx.mem.f(p + 8);
                let z = if fx.mem.i16(FACING) != 0 { ee::add(z, 5.0) } else { ee::sub(z, 5.0) };
                fx.mem.setf(p + 8, z);
                let y = ee::sub(fx.mem.f(p + 4), 1.0);
                fx.mem.setf(p + 4, y);
                if g == FLARE {
                    quads4::quad_turned_f(fx, uv_rect_q, D, t, 0x20, p);
                } else {
                    quads4::quad_turned(fx, uv_rect_q, D, t, 0x20, p);
                }
            }
        }
        if f(fx) == RING {
            tex(fx, D + 0x29C);
            cell(fx, t, sp + 0xB0);
            fx.mem.set16(D + 0xB8, 5);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv_rect_q, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if f(fx) == SHADE {
            let kind = fx.mem.i16(t + 0x50);
            tex(fx, (TEX as i32 + 4 * kind) as u32);
            let r = fx.mem.i16(t + 8);
            for (o, v) in [(0x94u32, r), (0x96, 0), (0x98, 0x1FC), (0x9A, 0x1FC)] {
                fx.mem.set16(D + o, v);
            }
            let p = sp + 0x60;
            for k in 0..3 {
                let e = fx.mem.read_u32(EYE);
                let a = fx.mem.read_u32(AT30);
                let (e, a) = (fx.mem.f(e + 4 * k), fx.mem.f(a + 4 * k));
                fx.mem.setf(p + 4 * k, ee::add(e, ee::mul(4.0, ee::sub(a, e))));
            }
            fx.mem.set16(D + 0xBE, 0);
            let v = [fx.mem.f(p), fx.mem.f(p + 4), fx.mem.f(p + 8)];
            let mut x = fx.mem.read_u64(sp + 0xE8);
            if let Some(w) = fx.project(v, None, &mut x) {
                fx.mem.write_u64(sp + 0xE8, x);
                fx.mem.setf(LOOK_W, w);
            }
            project_residue(fx, sp, p);
            let w = fx.mem.f(LOOK_W);
            let saves = [u64::from(D), u64::from(alive), u64::from(slot), u64::from(sp + 0xE8), u64::from(k), 0x01E0_2AE8];
            screen_quad(fx, D, t, 0x21, w, saves);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x99 => 1,
        0x122 => 4,
        0x118 => 2,
        0xA4..=0x112 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

fn tex(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

/// The atlas cell 0x9B frame +0x56 (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32) {
    let sub = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, 0x9B, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// 0x1E004D0 (0dd7766f741f; cards 61 / 76) `screen_quad(D, task, mode)`
/// with f12 = `w`: a textured 640 x 480 window quad (strip3 0x109B50, the
/// screen rectangle (0x6C00, 0x7100) .. (0x9400, 0x8F00), z 0xFFFFFF)
/// coloured +0x60, UVs D+0x94 in D+0xBE's orientation (0x108DE0), texture
/// D+0x88 (0x207E50 filter 1); blend (mode >> 4) (0: 0x42); keyed by
/// fix(w) into the frame's OT [F]. `saves`: the caller's s0..s4 and the
/// return address (its f20 [H: 0]).
pub(crate) fn screen_quad(fx: &mut Fx, d0: u32, t: u32, mode: i32, w: f32, saves: [u64; 6]) {
    let sp = fx.enter(0x160);
    fx.saved(sp, &[(0x10, saves[0]), (0x20, saves[1]), (0x30, saves[2]), (0x40, saves[3]), (0x50, saves[4]), (0x60, saves[5])]);
    fx.mem.write_u32(sp, 0);
    let d = sp + 0xB0;
    l2::desc_strip3(fx, d);
    let b = (mode >> 4) as i16 as i64;
    let alpha = if b == 0 { ALPHA_42 } else { ((b << 2) as u64) | 0x80_0000_0040 };
    fx.desc_alpha(d, alpha);
    for (local, to) in [(0x140u32, 0x10u32), (0x148, 0x28), (0x150, 0x40), (0x158, 0x58)] {
        l2::rgbaq(fx, sp + local, t + 0x60);
        let (a, b) = (fx.mem.read_u32(sp + local), fx.mem.read_u32(sp + local + 4));
        fx.mem.write_u32(d + to, a);
        fx.mem.write_u32(d + to + 4, b);
    }
    let order = fx.mem.i16(d0 + 0xBE);
    l2::uv_strip3(fx, d, d0 + 0x94, order);
    let tx = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tx, 1);
    l2::quad_xy(fx, sp + 0x70, 100.0, 100.0);
    let (x0, x1, y0, y1) = ((0x800 - 0x140) << 4, (0x800 + 0x140) << 4, (0x800 - 0xF0) << 4, (0x800 + 0xF0) << 4);
    for (o, x, y) in [(0x20u32, x0, y0), (0x38, x1, y0), (0x50, x0, y1), (0x68, x1, y1)] {
        fx.mem.write_u16(d + o, x as u16);
        fx.mem.write_u16(d + o + 2, y as u16);
        let v = fx.mem.read_u64(d + o);
        fx.mem.write_u64(d + o, (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
        fx.mem.write_u8(d + o + 7, 0);
    }
    let key = ee::fixunssfsi(w) as i32;
    fx.submit(d, OT, key);
    fx.leave(0x160);
}

/// 0x1E02BE0(D, step, 0) (c1969009b680; cards 61 / 76): the first free
/// task cleared and set off at D+0x250 (i16 +0, float +0x10) with the
/// colour D+0x8704 (twice), kind D+0x876E (+0x50 / +0x52); `saves` the
/// caller's s0..s4, `ra` the return address.
pub(crate) fn spawn(fx: &mut Fx, step: u32, saves: [u64; 5], ra: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, saves[4]), (0x30, saves[3]), (0x20, saves[2]), (0x10, saves[1]), (0, saves[0])]);
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
        fx.mem.write_u32(t + 0x40, 0);
        fx.mem.write_u32(t + 0x44, step);
    }
    fx.leave(0x60);
}

/// POS += rand % 30 - 15 per axis (rand's s0 = D, from `ras`).
fn jitter(fx: &mut Fx, ras: [u32; 3]) {
    for (k, ra) in ras.into_iter().enumerate() {
        let v = rmod(fx, D, ra, 0x1E);
        let at = POS + 4 * k as u32;
        let p = ee::add(fx.mem.f(at), ee::from_i32(v - 0xF));
        fx.mem.setf(at, p);
    }
}

/// 0x1E012A0(D, 0): the update counter and the emitters (frame 0x20: ra,
/// the entry's s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, 0x01E0_2530), (0, u64::from(D))]);
    // The spawns' saves: s0 = D, the entry's s1 = 0, s2 = the parameter,
    // s3 / s4 the runner's (0 [H]).
    let saves = [u64::from(D), 0, param as i64 as u64, 0, 0];
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    if c(fx) == 1 {
        spawn(fx, LIGHT, saves, 0x01E0_12F0);
    }
    if c(fx) == 1 {
        spawn(fx, LIGHT, saves, 0x01E0_131C);
        fx.mem.set16(SPAWN_K, 0);
        spawn(fx, SHADE, saves, 0x01E0_133C);
        fx.mem.set16(SPAWN_K, 1);
        spawn(fx, SHADE, saves, 0x01E0_1360);
    }
    if c(fx) == 1 {
        spawn(fx, FLARE, saves, 0x01E0_138C);
    }
    let n = c(fx);
    if n >= 0x5D && n % 3 == 1 && n < 0xAB {
        spawn(fx, SPARK, saves, 0x01E0_13E4);
    }
    let n = c(fx);
    if n >= 0x96 && n % 3 == 1 && n < 0x105 {
        spawn(fx, RISE, saves, 0x01E0_143C);
    }
    if c(fx) == 0x5D {
        spawn(fx, FRAME, saves, 0x01E0_1468);
    }
    if (0x5D..0xB0).contains(&c(fx)) {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        node_position_saving(fx, h, slot, POS, u64::from(D), Some(0), 0x01E0_14AC);
        jitter(fx, [0x01E0_14B4, 0x01E0_14E8, 0x01E0_151C]);
        spawn(fx, RING, saves, 0x01E0_1558);
    }
    if (0x96..0x114).contains(&c(fx)) {
        let h = fx.target_handle();
        let slot = fx.mem.i16(TARGET_NODE);
        node_position_saving(fx, h, slot, POS, u64::from(D), Some(0), 0x01E0_159C);
        jitter(fx, [0x01E0_15A4, 0x01E0_15D8, 0x01E0_160C]);
        spawn(fx, RING, saves, 0x01E0_1648);
    }
    fx.leave(0x20);
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

/// 0x1E01660 (D, task; never spawned): a violet (0x80, 0x20, 0xFF) ring
/// of quads, size 1..2, cell row 0..3, radius rand % fix(2 x D+0x8720) +
/// 10, a random angle, +0xE 16..35, row 0..34, life 3..5; in by 0x40
/// under 0xC0, out by 14.
fn ring_o(fx: &mut Fx, t: u32) {
    let f = ee::mul(2.0, fx.mem.f(D + 0x8720));
    fx.fix_residue(f, 0x01E0_1690);
    let m = ee::fixsfsi(f) as i16 as i32;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = fx.rand_from(t, 0x01E0_16AC);
        fx.mem.setf(t + 0x48, ee::from_i32(v % 2 + 1));
        let v = fx.rand_from(t, 0x01E0_16D8);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, 0x80, 0x20, 0xFF);
        let v = rmod(fx, t, 0x01E0_170C, m);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, 0x01E0_1730, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_174C, 0x14);
        fx.mem.set16(t + 0xE, v + 0x10);
        let v = rmod(fx, t, 0x01E0_176C, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_1788, 3);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, 0xC0, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0x10, 0xE, true);
    }
    age(fx, t);
}

/// 0x1E01820 (D, task): a grey flare 60 x 120 (+0x48 3.0) breathing: in
/// by 0x10 under 0xC0 for 15 updates; +0x5C up 25 degrees a update, +0x30
/// = 60 + 3 sin, +0x34 = 2 +0x30; never ends nor ages.
fn flare(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x3C);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w << 1);
        fx.mem.set16(t + 0x56, 0);
        fx.mem.write_u32(t + 0x48, 0x4040_0000);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0xF {
        fade_in(fx, t, 0x63, 0xC0, 0x10);
    }
    let a = fx.mem.i16(t + 0x5C) + 0x19;
    fx.mem.set16(t + 0x5C, a);
    let a = fx.mem.i16(t + 0x5C);
    if a >= 0x168 {
        fx.mem.set16(t + 0x5C, a % 0x168);
    }
    let a = fx.mem.i16(t + 0x5C);
    let s = l2::sin_deg(fx, TAB, a, 0);
    let w = ee::add(60.0, ee::mul(fx.mem.f(t + 0x48), s));
    fx.mem.setf(t + 0x30, w);
    fx.mem.setf(t + 0x34, ee::mul(2.0, w));
}

/// 0x1E01940 (D, task): a dark spark 30 x 60 turned -30..29 degrees: in
/// by 0x40 under 0x80 for 15 updates, out by 9; growing by 15 a update,
/// its height (int)(1.2 x (double) its width) (soft double).
fn spark(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x1E);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w << 1);
        l2::rgb(fx, t + 0x60, 0x30, 0x30, 0x30);
        fx.mem.set16(t + 0x56, 0);
        let v = rmod(fx, t, 0x01E0_1990, 0x3C);
        fx.mem.set16(t + 0x5A, v - 0x1E);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0xF {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9, true);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let w = fx.mem.i16(t + 8) + 0xF;
        fx.mem.set16(t + 8, w);
        let w = fx.mem.i16(t + 8);
        let h = (f64::from(w) * f64::from_bits(0x3FF3_3333_3333_3333)) as i32;
        fx.mem.set16(t + 0xA, h);
    }
    age(fx, t);
}

/// 0x1E01A70 (D, task): a grey rise 120 x 240 turned -30..29 degrees at
/// (cos, sin) of a random angle x +0x48 (60, shrinking by 3.5 while
/// above 0): in by 0x19 under 0x80 for 15 updates, out by 9.
fn rise(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x78);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w << 1);
        fx.mem.write_u32(t + 0x48, 0x4270_0000);
        let v = rmod(fx, t, 0x01E0_1AB4, 0x168);
        fx.mem.set16(t + 0x5C, v);
        circle(fx, t);
        fx.mem.write_u32(t + 0x38, 0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x56, 0);
        let v = rmod(fx, t, 0x01E0_1B28, 0x3C);
        fx.mem.set16(t + 0x5A, v - 0x1E);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0xF {
        fade_in(fx, t, 0x63, 0x80, 0x19);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9, true);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        circle(fx, t);
        let r = fx.mem.f(t + 0x48);
        if !(r <= 0.0) {
            fx.mem.setf(t + 0x48, ee::sub(r, 3.5));
        }
    }
    age(fx, t);
}

/// +0x30 / +0x34 = +0x48 x (cos, sin) of +0x5C degrees.
fn circle(fx: &mut Fx, t: u32) {
    let a = fx.mem.i16(t + 0x5C);
    let c = l2::sin_deg(fx, TAB, a, 1);
    let v = ee::mul(fx.mem.f(t + 0x48), c);
    fx.mem.setf(t + 0x30, v);
    let a = fx.mem.i16(t + 0x5C);
    let s = l2::sin_deg(fx, TAB, a, 0);
    let v = ee::mul(fx.mem.f(t + 0x48), s);
    fx.mem.setf(t + 0x34, v);
}

/// 0x1E01C40 (D, task): a reddish (0xFF, 0x80, 0x80) ring of quads at
/// random angles: size 10, cell row 3, radius 30..229, angle 0..359, +0xE
/// 1..3, row 1..8, life 1..3; in by 0x40 under 0xC0, out by 19.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        fx.mem.set16(t + 0x56, 3);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        for (o, ra) in [(0x20u32, 0x01E0_1C88u32), (0x24, 0x01E0_1CA4), (0x28, 0x01E0_1CC0)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        let v = rmod(fx, t, 0x01E0_1CDC, 0xC8);
        fx.mem.set16(t + 0x50, v + 0x1E);
        let v = rmod(fx, t, 0x01E0_1CFC, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_1D18, 3);
        fx.mem.set16(t + 0xE, v + 1);
        let v = fx.rand_from(t, 0x01E0_1D38);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, t, 0x01E0_1D58, 3);
        fx.mem.set16(t + 6, v + 1);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, 0xC0, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0x15, 0x13, true);
    }
    age(fx, t);
}

/// 0x1E01DF0 (D, task): a dark (0x20) screen shade scrolling its texture
/// row +8 (random 0..127) down (kind 0) or up (kind 1) a row a update: in
/// by 4 under 0x60 for 22 updates; once the timer is under 30 out by 7.
fn shade(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x60, 0x20, 0x20, 0x20);
        l2::rgb(fx, t + 0x64, 0x20, 0x20, 0x20);
        fx.mem.set16(t + 6, 0x16);
        let v = fx.rand_from(t, 0x01E0_1E48);
        fx.mem.set16(t + 8, v % 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, 0x60, 4);
        fade_in(fx, t, 0x67, 0x60, 4);
    } else if fx.mem.i32(TIMER) < 0x1E {
        fade_out(fx, t, 0x63, 9, 7, true);
        fade_out(fx, t, 0x67, 9, 7, false);
    }
    if fx.mem.i16(t + 0x50) == 0 {
        let r = fx.mem.i16(t + 8) + 1;
        fx.mem.set16(t + 8, r);
        let r = fx.mem.i16(t + 8);
        if r >= 0x80 {
            fx.mem.set16(t + 8, r % 0x80);
        }
    }
    if fx.mem.i16(t + 0x50) == 1 {
        let r = fx.mem.i16(t + 8) - 1;
        fx.mem.set16(t + 8, r);
        let r = fx.mem.i16(t + 8);
        if r <= 0 {
            fx.mem.set16(t + 8, 0x80 - r % 0x80);
        }
    }
    age(fx, t);
}

/// 0x1E01FC0 (D, task; frame 0x10): the stage light (0x1F7F10) +0x50
/// from 0x80 down by 10 to 0x30 while the timer is at least 31, then up
/// by 10 to 0x80; 0x80 when the timer is 2. It never ages nor ends.
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i32(TIMER) >= 0x1F {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x51 { 0x30 } else { l - 0xA });
    } else {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x80 { l + 0xA } else { 0x80 });
        if fx.mem.i16(t + 0x50) >= 0x81 {
            fx.mem.set16(t + 0x50, 0x80);
        }
    }
    if fx.mem.i32(TIMER) == 2 {
        fx.set_stage_light(0x80);
    } else {
        let l = fx.mem.read_u8(t + 0x50);
        fx.set_stage_light(i32::from(l));
    }
}

/// 0x1E020B0 (D, task; frame 0x90): a grey (0x90, 0x78, 0x78) frame flash
/// (a window sprite with the frame as its texture, ST from +0x48 = 0): in
/// by 6 under 0x50 until update 0x11A, then out by 9.
fn frame_flash(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u8(sp + 0x48, 0x90);
    fx.mem.write_u8(sp + 0x49, 0x78);
    fx.mem.write_u8(sp + 0x4A, 0x78);
    let a = fx.mem.read_u8(t + 0x63);
    fx.mem.write_u8(sp + 0x4B, a);
    fx.mem.write_u32(sp + 0x4C, 0x3F80_0000);
    for (at, v) in [(sp + 0x58, 0x6FF8u16), (sp + 0x68, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x50, ee::div(ee::add(s, 0.0), 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x54, ee::div(ee::add(s, 0.0), 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x60, ee::div(ee::sub(ee::add(512.0, s), 0.0), 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x64, ee::div(ee::sub(ee::add(512.0, s), 0.0), 512.0));
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if fx.mem.i16(COUNTER) < 0x11B {
        fade_in(fx, t, 0x63, 0x50, 6);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9, true);
    }
    age(fx, t);
}
