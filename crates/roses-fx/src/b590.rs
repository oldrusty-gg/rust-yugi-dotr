//! BMODULE b590 (card 643), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02660, data area 0x1ED0000) [F]: a pool of 400 tasks of 0x70 bytes at D+0x2A0 (alive +0x58, step
//! +0x44 called through `jalr`) drawn with the second library: rings of
//! quads (7369422df7f8 at 0x1E00920, `billboard::ring_quads`), sprite
//! columns (792ca1ad9378 at 0x1E01230, `quads4::sprite_quads`), arcs
//! (e6dce326551d at 0x1E00ED0, `billboard::arc_strips`) and full-window
//! gouraud tints (561a6131a635 at 0x1E015A0, `quads4::tint4`), UVs by
//! uv_rect 0x1E00080. On update 1 a frame flash (0x207EF0) fades in;
//! from 42 to 100 grey rings leave the attacker's nodes 6 / 0xA / 0xE /
//! 0x12; on update 168 at the target: an arc, a blue tint (keyed by a
//! point 50 in front of it), four light columns and twenty rings. Returns 1
//! on update 178, 4 on 205, 2 on 185, 6 on 169..179, 8 once the
//! 210-update timer +0xB210 has run out.
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
/// The step functions a task holds (+0x44), as the record's addresses.
const RING_A: u32 = 0x01E0_1A90;
const RING_B: u32 = 0x01E0_1C40;
const RING_C: u32 = 0x01E0_1DF0;
const COLUMN: u32 = 0x01E0_1FA0;
const TINT: u32 = 0x01E0_21A0;
const ARC: u32 = 0x01E0_2330;
const FLASH: u32 = 0x01E0_2450;
/// A step that only returns 0 (skipped by the entry).
const IDLE: u32 = 0x01E0_2650;
/// The return address of the entry's `jalr` into a step (0x1E02968 + 8).
const TASK_RA: u64 = 0x01E0_2970;
const TEX: u32 = D + 0xB1F0;
const TIMER: u32 = D + 0xB210;
const COLOUR: u32 = D + 0xB214;
/// The tint's w (0x204D80's).
const TINT_W: u32 = D + 0xB234;
const NODES: u32 = D + 0xB248;
const SPAWN_A: u32 = D + 0xB2A6;
const SPAWN_K: u32 = D + 0xB2B4;
const FACING: u32 = D + 0xB2B8;
const COUNTER: u32 = D + 0xB2BC;
const BYTES: u32 = D + 0xB2BE;

/// The entry 0x1E02660.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x150);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x150);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, D + 0xB4C0);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2BA, param);
    fx.mem.set32(TIMER, 0xD2);
    for (k, node) in [6, 0xA, 0xE, 0x12, 0x2C, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB258, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = fx.rand_from(k, 0x01E0_2884);
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_28D4);
        fx.mem.set16(D + 0xB268 + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        match f {
            0 | IDLE => {}
            RING_A | RING_B | RING_C => ring(fx, t, f),
            COLUMN => column(fx, t),
            TINT => tint(fx, t),
            ARC => arc(fx, t),
            FLASH => flash(fx, t),
            _ => panic!("b590: unknown task step {f:#x}"),
        }
    }
    // The entry's s0: the live tasks counted in the draw loop.
    let mut s0 = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        s0 += 1;
        for (f, tex, r, site, layout) in [(RING_A, TEX + 4, 0x70u32, 0x01E0_2A20u32, 0), (RING_B, TEX, 0x80, 0x01E0_2AC0, 0), (RING_C, TEX, 0x90, 0x01E0_2B60, 1)] {
            if fx.mem.read_u32(t + 0x44) != f {
                continue;
            }
            let w = fx.mem.read_u32(tex);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + r, 0x14, sub);
            fx.mem.set16(D + 0xB8, 4);
            let v = fx.rand_from(s0, site);
            fx.mem.set16(D + 0xBE, v % 4);
            fx.mem.set16(D + 0xB0, layout);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x21, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == COLUMN {
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, sp + 0xA0, 0x9A, 0);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sprite_quads(fx, D, t, 0, t + 0x10);
        }
        if fx.mem.read_u32(t + 0x44) == TINT {
            let h = fx.target_handle();
            fx.unit_position(h, sp + 0x60);
            // 0x1F71A0's frame below the entry's (its saved ra, s0).
            fx.saved(sp - 0x20, &[(0x10, 0x01E0_2C38), (0, u64::from(s0))]);
            let z = fx.mem.f(sp + 0x68);
            let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 50.0) } else { ee::add(z, 50.0) };
            fx.mem.setf(sp + 0x68, z);
            let p = [fx.mem.f(sp + 0x60), fx.mem.f(sp + 0x64), fx.mem.f(sp + 0x68)];
            let mut x = fx.mem.read_u64(sp + 0xD8);
            if let Some(w) = fx.project(p, None, &mut x) {
                fx.mem.write_u64(sp + 0xD8, x);
                fx.mem.setf(TINT_W, w);
            }
            project_residue(fx, sp, sp + 0x60);
            for (kind, mode) in [(0, 0x20), (1, 0x21), (2, 0x20)] {
                if fx.mem.i16(t + 0x50) == kind {
                    let w = fx.mem.f(TINT_W);
                    quads4::tint4(fx, t, mode, w);
                }
            }
        }
        if fx.mem.read_u32(t + 0x44) == ARC {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xB2 => 1,
        0xCD => 4,
        0xB9 => 2,
        0xA9..=0xB3 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// What 0x204D80 with no matrix leaves below its caller's `sp` [F
/// 0x204D80..0x204E48]: the camera's view-projection (0x20E2B0) at sp -
/// 0x50 and its local vec at sp - 0x10 (`v` through it, x y z scaled by 1
/// / w, w), stored whether or not the point is accepted. A later missing
/// texture's TEX0 / TEX1 read them [run].
pub(crate) fn project_residue(fx: &mut Fx, sp: u32, v: u32) {
    fx.camera_vp_to(sp - 0x50);
    let m = fx.camera_vp();
    let p = [fx.mem.f(v), fx.mem.f(v + 4), fx.mem.f(v + 8), 1.0];
    let o = Fx::vu_apply(&m, p);
    let q = ee::div(1.0, o[3]);
    for k in 0..3u32 {
        fx.mem.setf(sp - 0x10 + 4 * k, ee::mul(o[k as usize], q));
    }
    fx.mem.setf(sp - 4, o[3]);
}

/// The atlas cell `id` / `sub` (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// rand % m (HI) from a step's `jal` at `site` (rand's frame: s0 = `t`).
fn rmod(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, site + 8);
    fx.div(v, m).1
}

/// A node of the attacker into D+0x250.
fn at_node(fx: &mut Fx, at: u32) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(at);
    fx.node_position(h, slot, D + 0x250);
}

/// 0x1E01770(D, 0): the update counter and the emitters (frame 0x30: ra,
/// the entry's s1 = D / s0 = 0).
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_2930), (0x10, u64::from(D)), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, FLASH);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x2A..0x65).contains(&c) {
        for k in 0..4 {
            at_node(fx, NODES + 2 * k);
            spawn(fx, RING_B);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x82..0xFB).contains(&c) {
        at_node(fx, NODES + 8);
    }
    if fx.mem.i16(COUNTER) == 0xA8 {
        let h = fx.target_handle();
        fx.unit_position(h, D + 0x250);
        fx.mem.write_u32(D + 0x254, 0x4248_0000);
        spawn(fx, ARC);
        fx.mem.set16(SPAWN_K, 1);
        spawn(fx, TINT);
        for (a, k) in [(0, 0), (0x5A, 0), (0x2D, 1), (0x87, 1)] {
            fx.mem.set16(SPAWN_A, a);
            fx.mem.set16(SPAWN_K, k);
            spawn(fx, COLUMN);
        }
        for _ in 0..0x14 {
            spawn(fx, RING_C);
        }
    }
    fx.leave(0x30);
}

/// 0x1E02E50(D, step, 0): the first free task cleared and set off at
/// D+0x250 (as i16 and as floats +0x10) with the colour D+0xB214 (+0x60,
/// +0x64), +0x50 = +0x52 = D+0xB2B4, +0x5A = D+0xB2A6; 1 when one was
/// free.
pub(crate) fn spawn(fx: &mut Fx, step: u32) -> i32 {
    fx.enter(0x60);
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
            l2::to_i16x4(fx, t, D + 0x250, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(D + 0x250 + 4 * k);
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

/// The step frame (0x20: ra, the entry's s0 = 0).
fn frame20(fx: &mut Fx) -> u32 {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
    sp
}

/// The fade the rings share: in by 0x40 under 0xC0 while young (+0x54 <
/// +6), then out by 14 (the task ends below 16).
fn fade_c0(fx: &mut Fx, t: u32) {
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
}

/// 0x1E01A90 / 0x1E01C40 (the same body) / 0x1E01DF0 (`f`) (D, task): a
/// grey ring of quads: size 2..4 (0x1E01DF0: 10..15), cell row 0..2,
/// radius +0x50 5..9 growing by +0xE 1..3 (100..199), a random angle
/// (0x1E01DF0's turning by 3), row 0..34, life 3..5 (8..15).
fn ring(fx: &mut Fx, t: u32, f: u32) {
    frame20(fx);
    let big = f == RING_C;
    if fx.mem.i16(t + 0x58) == 1 {
        let (m, a) = if big { (6, 0xA) } else { (3, 2) };
        let v = rmod(fx, t, f + 0x1C, m);
        fx.mem.setf(t + 0x48, ee::from_i32(v + a));
        let v = rmod(fx, t, f + 0x48, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, f + 0x74, 5);
        fx.mem.set16(t + 0x50, v + 5);
        let v = rmod(fx, t, f + 0x94, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let (m, a) = if big { (0x64, 0x64) } else { (3, 1) };
        let v = rmod(fx, t, f + 0xB0, m);
        fx.mem.set16(t + 0xE, v + a);
        let v = rmod(fx, t, f + 0xD0, 0x23);
        fx.mem.set16(t + 8, v);
        let life = if big {
            let v = fx.rand_from(t, f + 0xF4);
            v % 8 + 8
        } else {
            rmod(fx, t, f + 0xEC, 3) + 3
        };
        fx.mem.set16(t + 6, life);
        fx.mem.set16(t + 0x58, 2);
    }
    fade_c0(fx, t);
    if big {
        let a = fx.mem.i16(t + 0x5A) + 3;
        fx.mem.set16(t + 0x5A, a);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E01FA0 (D, task): a light column (+0x50 its kind 0 / 1 / 2: width
/// +0x30 10 / 10 / 8, height +0x34 300 / 200 / 400, the first two growing
/// by 1 / 0.5 and 160 / 80 a update), size 6, red / blue, +0x52 counting to
/// 25; in by 0x40 for 4 updates, then out by 9.
fn column(fx: &mut Fx, t: u32) {
    frame20(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x40, 0x40, 0xFF);
        for (kind, w, h) in [(0, 0x4120_0000u32, 0x4396_0000u32), (1, 0x4120_0000, 0x4348_0000), (2, 0x4100_0000, 0x43C8_0000)] {
            if fx.mem.i16(t + 0x50) == kind {
                fx.mem.write_u32(t + 0x30, w);
                fx.mem.write_u32(t + 0x34, h);
            }
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.set16(t + 0x52, 0);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 4 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x40));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    for (kind, dh, dw) in [(0, 160.0f32, 1.0f32), (1, 80.0, 0.5)] {
        if fx.mem.i16(t + 0x50) == kind {
            if fx.mem.i16(t + 0x54) != 0 {
                let h = ee::add(fx.mem.f(t + 0x34), dh);
                fx.mem.setf(t + 0x34, h);
            }
            if fx.mem.i16(t + 0x54) != 0 {
                let w = ee::add(fx.mem.f(t + 0x30), dw);
                fx.mem.setf(t + 0x30, w);
            }
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let c = fx.mem.i16(t + 0x52);
        if c < 0x19 {
            fx.mem.set16(t + 0x52, c + 1);
        }
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E021A0 (D, task): a tint's colours by its kind +0x50 (0 white, life
/// 15; 1 blue, 13; 2 grey, 22); both alphas in by 0x2A under 0x80 while
/// young, then out by 7 (the task ends when the first is below 9, the
/// second is cleared).
fn tint(fx: &mut Fx, t: u32) {
    frame20(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        for (kind, r, g, b, life) in [(0, 0xF0, 0xF0, 0xF0, 0xF), (1, 0x60, 0x60, 0xC0, 0xD), (2, 0x58, 0x58, 0x78, 0x16)] {
            if fx.mem.i16(t + 0x50) == kind {
                l2::rgb(fx, t + 0x60, r, g, b);
                l2::rgb(fx, t + 0x64, r, g, b);
                fx.mem.set16(t + 6, life);
            }
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0x80 {
                fx.mem.write_u8(t + o, a.wrapping_add(0x2A));
            }
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 9 {
            fx.mem.write_u8(t + 0x63, a - 7);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let a = fx.mem.read_u8(t + 0x67);
        fx.mem.write_u8(t + 0x67, if a >= 9 { a - 7 } else { 0 });
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E02330 (D, task): the arc: outer radius +0x4C 130 growing by 0.4,
/// inner colour (0x80, 0x80, 0xFF), outer 0xC0 grey fading in by 0x30
/// under 0xC0 for 14 updates, then out by 15; angles 10 / 10; turned by
/// the camera's angles (0x205580 into +0x20, its first + 0x4000).
fn arc(fx: &mut Fx, t: u32) {
    frame20(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x4302_0000);
        l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xFF);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let a = fx.mem.read_u8(t + 0x67);
    if fx.mem.i16(t + 0x54) < 0xE {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x67, a.wrapping_add(0x30));
        }
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x67, a - 0xF);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_23F0);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = ee::add(fx.mem.f(t + 0x4C), f32::from_bits(0x3ECC_CCCD));
        fx.mem.setf(t + 0x4C, r);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E02450 (D, task): the frame flash (frame 0x90): a window sprite
/// (0x6FF8 .. 0x8FF8) sampling the frame (0x207EF0) at ST +0x48 / 512 ..
/// (512 - +0x48) / 512 (+0x48 = 2.5), colour (0x80, 0x80, 0x90, alpha
/// +0x63), ALPHA 0x44, key 0; in by 0x16 under 0x58 until update 184, then
/// out by 4 (the task ends below 6).
fn flash(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x90);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4020_0000);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x90);
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
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(COUNTER) < 0xB9 {
        if a < 0x58 {
            fx.mem.write_u8(t + 0x63, a + 0x16);
        }
    } else if a >= 6 {
        fx.mem.write_u8(t + 0x63, a - 4);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x90);
}
