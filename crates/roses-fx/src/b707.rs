//! BMODULE b707 (card 76), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03060, data area 0x1ED0000, 0x8AF0 bytes) [F]
//!: card 61's (b041's) layout and program shape
//! (170 tasks of 0x70 bytes at D+0x2A0, alive +0x58, step +0x44 called
//! through `jalr`, spawned by 0x1E03C20 = [`crate::b041::spawn`]) drawn
//! with the second library: rings of quads turned by the task's angles
//! (`billboard::ring_quads_angled` 0x1E00C70, UVs uv_rect_q 0x1E00080),
//! quads (`billboard::quad` 0x1E01220), flat arcs
//! (`billboard::arc_strips_flat` 0x1E00910), window tints
//! (`quads4::tint4` 0x1E004D0) and b041's screen quad (0x1E006A0,
//! [`crate::b041::screen_quad`]). Every update first stores the camera's
//! +0x20 / +0x30 at D+0x870C / D+0x8710.
//!
//! A point D+0x86C0 rides the attacker's node 1 until update 159 (its y /
//! z drawn toward node 0x17's by D+0x8724, which grows by 0.02 to 1 from
//! update 54), then drifts back 6.8 and down 0.2 a update until 183; the
//! spread D+0x8720 grows from update 54 (1.5, 3, 32, 5 a update by
//! phase). Update 1: a violet frame flash (0x1E02D80), the stage light
//! (0x1E02C90) twice, a white / blue arc (0x1E02580) and a dark screen
//! shade (0x1E02810); 93..294: a white ring a update (0x1E01B20) and two
//! dark arcs (0x1E02240); 93..180 every third update a quad orbiting the
//! point inward (0x1E01D30), 216..280 one spiralling out (0x1E01ED0); 185
//! two pale window tints (0x1E02B00); 190 a flickering grey quad (0x1E02070,
//! drawn in a loop of its own before the others). Steps 0x1E02690 (a band
//! 0x1E03DC0) and 0x1E029E0 (an arc) are drawn by the entry but never
//! spawned (not ported). Returns 1 on update 230, 3 on 310, 6 on
//! 241..304, 8 once the 330-update timer D+0x8700 has run out.
use crate::b041::{screen_quad, spawn};
use crate::b076::node_position_saving;
use crate::b494::camera_angles_saving;
use crate::b590::project_residue;
use crate::blib::billboard::{self, uv_rect_q};
use crate::blib::quads4;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0xAA;
const RING: u32 = 0x01E0_1B20;
const ORBIT_IN: u32 = 0x01E0_1D30;
const ORBIT_OUT: u32 = 0x01E0_1ED0;
const GLOW: u32 = 0x01E0_2070;
const ARCS: u32 = 0x01E0_2240;
const ARC: u32 = 0x01E0_2580;
const BAND: u32 = 0x01E0_2690;
const SHADE: u32 = 0x01E0_2810;
const ARC_O: u32 = 0x01E0_29E0;
const TINT: u32 = 0x01E0_2B00;
const LIGHT: u32 = 0x01E0_2C90;
const FRAME: u32 = 0x01E0_2D80;
/// The return address of the entry's `jalr` into a step (0x1E03410 + 8).
const TASK_RA: u64 = 0x01E0_3418;
const POS: u32 = D + 0x250;
const POINT: u32 = D + 0x86C0;
const TEX: u32 = D + 0x86E0;
const TIMER: u32 = D + 0x8700;
const EYE: u32 = D + 0x870C;
const AT30: u32 = D + 0x8710;
/// The screen points' w (0x204D80's out).
const LOOK_W: u32 = D + 0x871C;
const SPREAD: u32 = D + 0x8720;
const PULL: u32 = D + 0x8724;
const NODES: u32 = D + 0x8740;
const TARGET_NODE: u32 = D + 0x8750;
const SPAWN_K: u32 = D + 0x876E;
const FACING: u32 = D + 0x8772;
const COUNTER: u32 = D + 0x8776;
const BYTES: u32 = D + 0x8778;
const BYTES_B: u32 = D + 0x8878;
const TAB: u32 = D + 0x8978;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E03060 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x150);
    fx.mem.write_u32(D + 0x8714, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let e = fx.camera_eye(0);
    fx.mem.write_u32(EYE, e);
    let a = fx.camera_at30(0);
    fx.mem.write_u32(AT30, a);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x150);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x8AF0);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x8774, param);
    fx.mem.set32(TIMER, 0x14A);
    for (k, node) in [1, 0x17, 0x36, 0x15, 0x27].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    for k in 7..0x10u32 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x27C + 4 * (k - 7), t);
    }
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.fix_residue(y, 0x01E0_325C);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    // rand's frames: the entry's s0 = D.
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_328C, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_32E4);
        fx.mem.set8(BYTES + k, v % 4);
    }
    for k in 0xB4..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3344, 0xC);
        fx.mem.set8(BYTES + k, v - 6);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3394, 0xA);
        fx.mem.set8(BYTES_B + k, v - 5);
    }
    0
}

/// `p` = eye + `by` (look - eye), projected (0x204D80 through sp + 0xD8
/// into D+0x871C, D+0xBE cleared first when `clear_order`); the w.
fn screen_point(fx: &mut Fx, sp: u32, by: f32, clear_order: bool) -> f32 {
    let p = sp + 0x60;
    for k in 0..3 {
        let e = fx.mem.read_u32(EYE);
        let a = fx.mem.read_u32(AT30);
        let (e, a) = (fx.mem.f(e + 4 * k), fx.mem.f(a + 4 * k));
        fx.mem.setf(p + 4 * k, ee::add(e, ee::mul(by, ee::sub(a, e))));
    }
    if clear_order {
        fx.mem.set16(D + 0xBE, 0);
    }
    let v = [fx.mem.f(p), fx.mem.f(p + 4), fx.mem.f(p + 8)];
    let mut x = fx.mem.read_u64(sp + 0xD8);
    if let Some(w) = fx.project(v, None, &mut x) {
        fx.mem.write_u64(sp + 0xD8, x);
        fx.mem.setf(LOOK_W, w);
    }
    project_residue(fx, sp, p);
    fx.mem.f(LOOK_W)
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        // The steps' prologues: the entry's s0 = D, s1 = 0, s2 = the slot
        // (D + 0x70 k).
        let frame = match f {
            0 => continue,
            RING => 0x40,
            ORBIT_IN | ORBIT_OUT | ARCS | ARC | SHADE => 0x30,
            GLOW | TINT => 0x20,
            LIGHT => 0x10,
            FRAME => 0x90,
            _ => panic!("b707: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        match frame {
            0x10 => fx.saved(s, &[(0, TASK_RA)]),
            0x20 => fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]),
            0x40 => fx.saved(s, &[(0x30, TASK_RA), (0x20, u64::from(D + 0x70 * k)), (0x10, 0), (0, u64::from(D))]),
            _ => fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]),
        }
        match f {
            RING => ring(fx, t),
            ORBIT_IN => orbit(fx, t, false),
            ORBIT_OUT => orbit(fx, t, true),
            GLOW => glow(fx, t),
            ARCS => arcs(fx, t),
            ARC => arc(fx, t),
            SHADE => shade(fx, t),
            TINT => tint(fx, t),
            LIGHT => light(fx, t),
            _ => frame_flash(fx, t, s),
        }
        fx.leave(frame);
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 || fx.mem.read_u32(t + 0x44) != GLOW {
            continue;
        }
        tex(fx, TEX + 8);
        cell(fx, t, sp + 0x70);
        let r = fx.mem.i16(t + 0x56);
        for (o, v) in [(0x94u32, r << 6), (0x96, 0), (0x98, 0x3F), (0x9A, 0x3F)] {
            fx.mem.set16(D + o, v);
        }
        fx.mem.set16(D + 0xBE, 0);
        for k in 0..4 {
            let w = fx.mem.read_u32(POINT + 4 * k);
            fx.mem.write_u32(sp + 0x60 + 4 * k, w);
        }
        billboard::quad(fx, uv_rect_q, D, t, 0x21, sp + 0x60);
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        if f(fx) == RING {
            tex(fx, TEX + 4);
            cell(fx, t, sp + 0x80);
            fx.mem.set16(D + 0xB8, 5);
            let v = fx.rand_from(D, 0x01E0_35C8);
            fx.mem.set16(D + 0xBE, v % 4);
            fx.mem.set16(D + 0xB0, 0);
            for k in 0..4 {
                let w = fx.mem.read_u32(POINT + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            billboard::ring_quads_angled(fx, uv_rect_q, D, t, 0x21, BYTES_B);
            fx.mem.set16(D + 0xB8, 0);
        }
        for (g, local) in [(ORBIT_IN, 0x90u32), (ORBIT_OUT, 0xA0)] {
            if f(fx) == g {
                tex(fx, TEX + 4);
                cell(fx, t, sp + local);
                fx.mem.set16(D + 0xBE, 0);
                for k in 0..3 {
                    let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                    fx.mem.write_u32(sp + 0x60 + 4 * k, w);
                }
                billboard::quad(fx, uv_rect_q, D, t, 0x21, sp + 0x60);
            }
        }
        if f(fx) == ARCS {
            for k in 0..4 {
                let w = fx.mem.read_u32(POINT + 4 * k);
                fx.mem.write_u32(sp + 0x60 + 4 * k, w);
            }
            fx.mem.set16(D + 0xB8, 0x12);
            if fx.mem.i16(t + 0x50) == 0 {
                billboard::arc_strips_flat(fx, D, t, 0x10, sp + 0x60);
            }
            if fx.mem.i16(t + 0x50) == 1 {
                billboard::arc_strips_flat(fx, D, t, 1, sp + 0x60);
            }
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == ARC {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips_flat(fx, D, t, 0x10, POINT);
            fx.mem.set16(D + 0xB8, 0);
        }
        debug_assert_ne!(f(fx), BAND);
        if f(fx) == SHADE {
            let kind = fx.mem.i16(t + 0x50);
            tex(fx, (TEX as i32 + 4 * kind) as u32);
            let r = fx.mem.i16(t + 8);
            for (o, v) in [(0x94u32, r), (0x96, 0), (0x98, 0x200), (0x9A, 0x200)] {
                fx.mem.set16(D + o, v);
            }
            let w = screen_point(fx, sp, 4.0, true);
            let alive = alive_upto(fx, k);
            let saves = [u64::from(D), u64::from(alive), u64::from(D + 0x70 * k), u64::from(sp + 0xD8), u64::from(k), 0x01E0_39B0];
            screen_quad(fx, D, t, 0x21, w, saves);
        }
        debug_assert_ne!(f(fx), ARC_O);
        if f(fx) == TINT {
            let w = screen_point(fx, sp, 5.0, false);
            match fx.mem.i16(t + 0x50) {
                0 => quads4::tint4(fx, t, 1, w),
                1 => quads4::tint4(fx, t, 0x10, w),
                2 => quads4::tint4(fx, t, 0x20, w),
                _ => {}
            }
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xE6 => 1,
        0x136 => 3,
        0xF1..=0x130 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The entry's s1 at the main draw loop's task `k`: the live tasks counted
/// by the glow loop and this loop up to and including `k`.
fn alive_upto(fx: &mut Fx, k: u32) -> u32 {
    let mut live = |j: u32| fx.mem.i16(D + 0x2F8 + 0x70 * j) != 0;
    let all = (0..TASKS).filter(|&j| live(j)).count() as u32;
    all + (0..=k).filter(|&j| live(j)).count() as u32
}

fn tex(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

/// The atlas cell 0x14 frame +0x56 (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32) {
    let sub = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, 0x14, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// 0x1E014E0(D, 0): the update counter, the point, the spread and the
/// spawns (frame 0x30: ra, the entry's s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x10, 0x01E0_33E8), (0, u64::from(D))]);
    // The spawns' saves: s0 = D, the entry's s1 = 0, s2 = the parameter,
    // s3 / s4 the runner's (0 [H]).
    let saves = [u64::from(D), 0, param as i64 as u64, 0, 0];
    let d = u64::from(D);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let n = c(fx);
    if n < 0xA0 {
        let slot = fx.mem.i16(NODES);
        let h = fx.self_handle();
        node_position_saving(fx, h, slot, POINT, d, Some(0), 0x01E0_1544);
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        node_position_saving(fx, h, slot, sp + 0x20, d, Some(0), 0x01E0_1564);
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2);
        node_position_saving(fx, h, slot, POS, d, Some(0), 0x01E0_1584);
        let y = fx.mem.f(POS + 4);
        fx.mem.setf(POINT + 4, y);
        let z = fx.mem.f(POS + 8);
        fx.mem.setf(POINT + 8, z);
        for o in [4u32, 8] {
            let a = fx.mem.f(POINT + o);
            let v = ee::add(a, ee::mul(fx.mem.f(PULL), ee::sub(a, fx.mem.f(sp + 0x20 + o))));
            fx.mem.setf(POINT + o, v);
        }
    } else if n < 0xB8 {
        let z = fx.mem.f(POINT + 8);
        let by = f32::from_bits(0x40D9_999A);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, by) } else { ee::add(z, by) };
        fx.mem.setf(POINT + 8, z);
        let y = ee::sub(fx.mem.f(POINT + 4), f32::from_bits(0x3E4C_CCCD));
        fx.mem.setf(POINT + 4, y);
    } else if n < 0xBC {
        let s = ee::add(fx.mem.f(SPREAD), 32.0);
        fx.mem.setf(SPREAD, s);
    } else if n < 0xE6 {
        let s = ee::add(fx.mem.f(SPREAD), 5.0);
        fx.mem.setf(SPREAD, s);
    }
    if c(fx) == 1 {
        spawn(fx, FRAME, saves, 0x01E0_1754);
    }
    if c(fx) == 1 {
        spawn(fx, LIGHT, saves, 0x01E0_1780);
    }
    if c(fx) == 1 {
        spawn(fx, ARC, saves, 0x01E0_17AC);
    }
    if c(fx) == 1 {
        spawn(fx, LIGHT, saves, 0x01E0_17D8);
        fx.mem.set16(SPAWN_K, 0);
        spawn(fx, SHADE, saves, 0x01E0_17F8);
        fx.mem.set16(SPAWN_K, 1);
    }
    if c(fx) == 0xB9 {
        fx.mem.set16(SPAWN_K, 0);
        spawn(fx, TINT, saves, 0x01E0_1840);
        fx.mem.set16(SPAWN_K, 1);
        spawn(fx, TINT, saves, 0x01E0_1864);
        fx.mem.set16(SPAWN_K, 2);
    }
    let n = c(fx);
    if (0x35..0x12D).contains(&n) {
        fx.mem.set16(SPAWN_K, 0);
        fx.mem.set16(SPAWN_K, 1);
        let n = c(fx);
        if (0x5D..0x127).contains(&n) {
            spawn(fx, RING, saves, 0x01E0_18EC);
        }
        if c(fx) >= 0x5D {
            fx.mem.set16(SPAWN_K, 0);
            if c(fx) < 0x127 {
                spawn(fx, ARCS, saves, 0x01E0_193C);
            }
            fx.mem.set16(SPAWN_K, 1);
            if c(fx) < 0x127 {
                spawn(fx, ARCS, saves, 0x01E0_1978);
            }
            let n = c(fx);
            if n % 3 == 1 && n < 0xB5 {
                spawn(fx, ORBIT_IN, saves, 0x01E0_19C4);
            }
            let n = c(fx);
            if n >= 0xD8 && n % 3 == 1 && n < 0x119 {
                spawn(fx, ORBIT_OUT, saves, 0x01E0_1A1C);
            }
        }
    }
    if c(fx) == 0xBE {
        spawn(fx, GLOW, saves, 0x01E0_1A48);
    }
    let n = c(fx);
    if n >= 0x36 {
        if n < 0x6E {
            let s = ee::add(fx.mem.f(SPREAD), 1.5);
            fx.mem.setf(SPREAD, s);
        } else if n < 0x96 {
            let s = ee::add(fx.mem.f(SPREAD), 3.0);
            fx.mem.setf(SPREAD, s);
        }
        let p = fx.mem.f(PULL);
        if p < 1.0 {
            fx.mem.setf(PULL, ee::add(p, f32::from_bits(0x3CA3_D70A)));
        }
    }
    fx.leave(0x30);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn odd_age(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x54).wrapping_rem(2) == 1
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

/// +0x20 / +0x24 / +0x28 = rand % 0xFFFF each.
fn turn(fx: &mut Fx, t: u32, ras: [u32; 3]) {
    for (o, ra) in [0x20u32, 0x24, 0x28].into_iter().zip(ras) {
        let v = rmod(fx, t, ra, 0xFFFF);
        fx.mem.set32(t + o, v);
    }
}

/// 0x1E01B20 (D, task; frame 0x40): a white ring of quads (size 10, cell
/// row (rand >> 4) % 4, radius 10 + rand % fix(2 x the spread), angle
/// 0..359, +0xE 15..34, row 0..34, life 3..5): in by 0x40 under 0xC0, out
/// by 14; from update 241 widening by +0xE; on 161..209 turned by -0x300
/// about z a update.
fn ring(fx: &mut Fx, t: u32) {
    let f = ee::mul(2.0, fx.mem.f(SPREAD));
    fx.fix_residue(f, 0x01E0_1B58);
    let m = ee::fixsfsi(f) as i16 as i32;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        let v = fx.rand_from(t, 0x01E0_1B78);
        fx.mem.set16(t + 0x56, (v >> 4) % 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        let v = rmod(fx, t, 0x01E0_1BB0, m);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, 0x01E0_1BD4, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_1BF0, 0x14);
        fx.mem.set16(t + 0xE, v + 0xF);
        let v = rmod(fx, t, 0x01E0_1C10, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_1C2C, 3);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, 0xC0, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0x10, 0xE, true);
    }
    if fx.mem.i16(COUNTER) >= 0xF1 && fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    let n = fx.mem.i16(COUNTER);
    if (0xA1..0xD2).contains(&n) {
        let a = fx.mem.i32(t + 0x28).wrapping_sub(0x300);
        fx.mem.set32(t + 0x28, a);
    }
    age(fx, t);
}

/// 0x1E01D30 (`out` false) / 0x1E01ED0 (true) (D, task; frame 0x30): a
/// white quad 10 x 10 (cell row 2..7) on a circle about the point (radius
/// +0x48 from 30 shrinking by 1, or from 0 growing by 1; the angle +0x5C
/// random, up 2 a update): in by 0x60 under 0xC0 for 15 updates, out by 9;
/// turned by the camera.
fn orbit(fx: &mut Fx, t: u32, out: bool) {
    let o = if out { 0x1A0 } else { 0 };
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0xA);
        fx.mem.set16(t + 0xA, 0xA);
        let v = rmod(fx, t, 0x01E0_1D64 + o, 6);
        fx.mem.set16(t + 0x56, v + 2);
        fx.mem.write_u32(t + 0x34, 0x3F00_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = rmod(fx, t, 0x01E0_1DA0 + o, 0x168);
        fx.mem.set16(t + 0x5C, v);
        fx.mem.write_u32(t + 0x48, if out { 0 } else { 0x41F0_0000 });
    }
    if fx.mem.i16(t + 0x54) < 0xF {
        fade_in(fx, t, 0x63, 0xC0, 0x60);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9, true);
    }
    for k in 0..4 {
        let w = fx.mem.read_u32(POINT + 4 * k);
        fx.mem.write_u32(t + 0x10 + 4 * k, w);
    }
    let a = fx.mem.i16(t + 0x5C);
    let c = l2::sin_deg(fx, TAB, a, 1);
    let v = ee::add(fx.mem.f(t + 0x10), ee::mul(fx.mem.f(t + 0x48), c));
    fx.mem.setf(t + 0x10, v);
    let a = fx.mem.i16(t + 0x5C);
    let s = l2::sin_deg(fx, TAB, a, 0);
    let v = ee::add(fx.mem.f(t + 0x14), ee::mul(fx.mem.f(t + 0x48), s));
    fx.mem.setf(t + 0x14, v);
    let a = fx.mem.i16(t + 0x5C) + 2;
    fx.mem.set16(t + 0x5C, a);
    let r = fx.mem.f(t + 0x48);
    fx.mem.setf(t + 0x48, if out { ee::add(r, 1.0) } else { ee::sub(r, 1.0) });
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), if out { 0x01E0_2048 } else { 0x01E0_1EAC });
    age(fx, t);
}

/// 0x1E02070 (D, task; frame 0x20): a grey quad 45 x 45 growing by 10 to
/// 104, its cell row toggling 0 / 1, flickering 0x80 / 0x60: in by 0x40
/// under 0xC0 for 90 updates, out by 9; turned by the camera.
fn glow(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x2D);
        fx.mem.set16(t + 0xA, 0x2D);
        let v = fx.rand_from(t, 0x01E0_209C);
        fx.mem.set16(t + 0x56, v % 2);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0x5A {
        fade_in(fx, t, 0x63, 0xC0, 0x40);
        fade_in(fx, t, 0x67, 0xC0, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9, true);
        let a = fx.mem.read_u8(t + 0x67);
        if a >= 0xB {
            fx.mem.write_u8(t + 0x67, a - 9);
        }
    }
    let r = fx.mem.i16(t + 0x56);
    fx.mem.set16(t + 0x56, i32::from(r == 0));
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [8u32, 0xA] {
            let s = fx.mem.i16(t + o);
            if s < 0x5F {
                fx.mem.set16(t + o, s + 0xA);
            }
        }
    }
    let g = if odd_age(fx, t) { 0x80 } else { 0x60 };
    l2::rgb(fx, t + 0x60, g, g, g);
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_221C);
    age(fx, t);
}

/// 0x1E02240 (D, task; frame 0x30): a dark arc at the point: kind 0
/// violet (0x20, 0x10, 0x40) over grey 0x30, from radius 0 growing by a
/// 20th of the spread, for 18 updates; kind 1 dark red (0x40, 0, 0) over
/// grey 0x40, from 500 shrinking by a 10th of it, for 10; the outer
/// radius 0.5 / 0.6 of it; turned at random; the outer in by 12 under
/// 0x80, then out by 9 / 7.
fn arcs(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0x20, 0x10, 0x40);
            l2::rgb(fx, t + 0x64, 0x30, 0x30, 0x30);
        } else {
            l2::rgb(fx, t + 0x60, 0x40, 0, 0);
            l2::rgb(fx, t + 0x64, 0x40, 0x40, 0x40);
        }
        let k = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xE, k);
        if fx.mem.i16(t + 0x50) == 0 {
            turn(fx, t, [0x01E0_22E8, 0x01E0_2304, 0x01E0_2320]);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            turn(fx, t, [0x01E0_2350, 0x01E0_236C, 0x01E0_2388]);
            fx.mem.write_u32(t + 0x48, 0x43FA_0000);
        }
        fx.mem.set16(t + 0x5A, 0x14);
        fx.mem.set16(t + 0x5C, 0x14);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x63, 0);
        fx.mem.set16(t + 6, 0);
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.set16(t + 8, 0x12);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.set16(t + 8, 0xA);
        }
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 8) {
        fade_in(fx, t, 0x67, 0x80, 0xC);
    } else if fx.mem.i16(t + 0x50) == 0 {
        fade_out(fx, t, 0x67, 0xB, 9, true);
    } else {
        fade_out(fx, t, 0x67, 9, 7, true);
    }
    for (kind, ratio, div, grow) in [(0, 0.5f32, 20.0f32, true), (1, f32::from_bits(0x3ECC_CCCD), 10.0, false)] {
        if fx.mem.i16(t + 0x50) != kind {
            continue;
        }
        let r = fx.mem.f(t + 0x48);
        fx.mem.setf(t + 0x4C, ee::sub(r, ee::mul(ratio, r)));
        if fx.mem.i16(t + 6) < 0 {
            fx.mem.write_u32(t + 0x4C, 0);
        }
        if fx.mem.i16(t + 0x54) != 0 {
            let s = ee::div(fx.mem.f(SPREAD), div);
            let r = fx.mem.f(t + 0x48);
            fx.mem.setf(t + 0x48, if grow { ee::add(r, s) } else { ee::sub(r, s) });
        }
    }
    age(fx, t);
}

/// 0x1E02580 (D, task; frame 0x30): a white / blue (0x10, 0x10, 0x80) arc
/// from radius 0 out to the spread, the outer in by 0x50 under 0xA0 for
/// 300 updates, then out by 12; turned by the camera and 0x4000 more a
/// update.
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x10, 0x10, 0x80);
        fx.mem.set16(t + 6, 0x12C);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x67, 0xA0, 0x50);
    } else {
        fade_out(fx, t, 0x67, 0xE, 0xC, true);
    }
    let s = fx.mem.read_u32(SPREAD);
    fx.mem.write_u32(t + 0x4C, s);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2658);
    let a = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
    fx.mem.set32(t + 0x20, a);
    age(fx, t);
}

/// 0x1E02810 (D, task; frame 0x30; b041's 0x1E01DF0 word for word): a
/// dark (0x20) screen shade scrolling its texture row +8 (random 0..127)
/// down (kind 0) or up (kind 1) a row a update: in by 4 under 0x60 for 22
/// updates; once the timer is under 30 out by 7.
fn shade(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x60, 0x20, 0x20, 0x20);
        l2::rgb(fx, t + 0x64, 0x20, 0x20, 0x20);
        fx.mem.set16(t + 6, 0x16);
        let v = fx.rand_from(t, 0x01E0_2868);
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

/// 0x1E02B00 (D, task; frame 0x20): a window tint by kind: 0 grey 0xF0,
/// 1 lilac (0xE0, 0xB0, 0xF0) over (0xD0, 0xA0, 0xF0) (32 updates), 2
/// slate (0x58, 0x58, 0x78) (22): in by 0x19 under 0x80, then out by 4
/// (the bottom to 0).
fn tint(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0xF0, 0xF0, 0xF0);
            l2::rgb(fx, t + 0x64, 0xF0, 0xF0, 0xF0);
            fx.mem.set16(t + 6, 0x20);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0xE0, 0xB0, 0xF0);
            l2::rgb(fx, t + 0x64, 0xD0, 0xA0, 0xF0);
            fx.mem.set16(t + 6, 0x20);
        }
        if fx.mem.i16(t + 0x50) == 2 {
            l2::rgb(fx, t + 0x60, 0x58, 0x58, 0x78);
            l2::rgb(fx, t + 0x64, 0x58, 0x58, 0x78);
            fx.mem.set16(t + 6, 0x16);
        }
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

/// 0x1E02C90 (D, task; frame 0x10): the stage light (0x1F7F10) +0x50
/// from 0x80 down by 10 to 0x20 while the timer is at least 31, then up
/// by 10 to 0x80, the task ending there; 0x80 when the timer is 2.
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i32(TIMER) >= 0x1F {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x21 { 0x20 } else { l - 0xA });
    } else {
        let l = fx.mem.i16(t + 0x50);
        if l < 0x80 {
            fx.mem.set16(t + 0x50, l + 0xA);
        } else {
            fx.mem.set16(t + 0x50, 0x80);
            fx.mem.set16(t + 0x58, 0);
        }
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

/// 0x1E02D80 (D, task; frame 0x90): a violet (0x80, 0x80, 0xB0; from
/// update 241 (0x90, 0x70, 0x80)) frame flash (a window sprite with the
/// frame as its texture, ST inset by 10 + +0x48 / 512, +0x48 = 10 sin of
/// +0x5A (up 10 degrees a update) on updates 161..209): in by 10 under
/// 0x50 until update 215, out by 6 to 0x40 from 217; from its update 300
/// out by 9.
fn frame_flash(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xB0);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for k in 0..4 {
        let b = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(sp + 0x48 + k, b);
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
    fx.mem.setf(sp + 0x50, ee::div(ee::add(s, 10.0), 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x54, ee::div(ee::add(s, 10.0), 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x60, ee::div(ee::sub(ee::add(512.0, s), 10.0), 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x64, ee::div(ee::sub(ee::add(512.0, s), 10.0), 512.0));
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    let n = fx.mem.i16(COUNTER);
    if (0xA1..0xD2).contains(&n) {
        let a = fx.mem.i16(t + 0x5A);
        let s = l2::sin_deg(fx, TAB, a, 0);
        fx.mem.setf(t + 0x48, ee::mul(10.0, s));
        let a = fx.mem.i16(t + 0x5A) + 0xA;
        fx.mem.set16(t + 0x5A, a);
    }
    if fx.mem.i16(COUNTER) == 0xF1 {
        l2::rgb(fx, t + 0x60, 0x90, 0x70, 0x80);
    }
    if fx.mem.i16(t + 0x54) < 0x12C {
        if fx.mem.i16(COUNTER) < 0xD8 {
            fade_in(fx, t, 0x63, 0x50, 0xA);
        }
        if fx.mem.i16(COUNTER) >= 0xD9 {
            let a = fx.mem.read_u8(t + 0x63);
            if a >= 0x41 {
                fx.mem.write_u8(t + 0x63, a - 6);
            }
        }
    } else {
        fade_out(fx, t, 0x63, 0xB, 9, true);
    }
    age(fx, t);
}
