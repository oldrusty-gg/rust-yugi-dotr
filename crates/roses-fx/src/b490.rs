//! BMODULE b490 (card 283), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02080, data area 0x1ED0000) [F] (helper A11, 31
//! Oct 2026): b361's program with b504's data layout 0x2BC0 lower and
//! b504's spawn (float position): 310 tasks of 0x70 bytes at D+0x2E0
//! (alive +0x58, step +0x44, each step called by its own `jal`; spawned
//! by 0x1E02600) drawn as sprites (92142265a824 at 0x1E02790,
//! `billboard::sprite_spin`, at the float position) and a flaming disc
//! (1f87ac8741ac at 0x1E02980, `quads4::fan18_t`), plus b504's unit flash
//! (0x1E02B80; its mirror tasks 0x1E01260 (grey 0x38) and the window task
//! 0x1E01CF0, b504's code). Updates 35..180 a mirror task a update; 35
//! the window task, the orange disc (radius 630, fading in by 0x40 to
//! 0xC0 for 170 updates) following the attacker's node 1 and the stage
//! dimmed (0x1E01080); 35..220 6..14 grey sprites a update round node 1
//! drifting 10 a update toward the facing. Returns 1 on update 90, 4 on
//! 245, 2 on 230, 6 on 101..224, 8 once the 260-update timer +0x8B00 has
//! run out.
//!
//! The steps 0x1E00E90 (camera slots through 0x1F7E20) and 0x1E01160 (an
//! object op, 7ef2fbb02542) are never spawned [F: the spawn `jal`s name
//! 0x1E01260 / 0x1E01CF0 / 0x1E00D90 / 0x1E01080 / 0x1E00BF0]; not ported.
use crate::b590::project_residue;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 310;
const TASK0: u32 = D + 0x2E0;
const SPRITE: u32 = 0x01E0_0BF0;
const DISC: u32 = 0x01E0_0D90;
const LIGHT: u32 = 0x01E0_1080;
const MIRROR: u32 = 0x01E0_1260;
const WINDOW: u32 = 0x01E0_1CF0;
const POS: u32 = D + 0x250;
const TEX: u32 = D + 0x8AE0;
/// The attacker's node 1 (kept every update by the emitter).
const NODE_P: u32 = D + 0x8AD0;
const LAYER: u32 = D + 0x8B30;
const TIMER: u32 = D + 0x8B00;
const COLOUR: u32 = D + 0x8B04;
const DESC: u32 = D + 0x8B14;
const NODES: u32 = D + 0x8B38;
const KIND: u32 = D + 0x8B50;
const FACING: u32 = D + 0x8B52;
const COUNTER: u32 = D + 0x8B56;
const BYTES: u32 = D + 0x8B58;
const BYTES_B: u32 = D + 0x8C58;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02080 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x130);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x8B0C, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x130);
    ret
}

fn facing_of(fx: &mut Fx, out: u32, ra: u32) -> i32 {
    let h = fx.self_handle();
    fx.unit_rotation(h, out);
    let y = ee::div(ee::from_i32(fx.mem.i32(out + 4)), 180.0);
    fx.fix_residue(y, ra);
    ee::fixsfsi(y)
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x8ED0);
    l2::sine_table(fx, D + 0x8D58);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x8B54, param);
    fx.mem.set32(TIMER, 0x104);
    for k in 0..3u32 {
        let h = fx.self_handle();
        let r = fx.request_node(h, 1, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k, r);
    }
    let t = fx.texture(5, 0);
    fx.mem.write_u32(TEX, t);
    let f = facing_of(fx, sp + 0x80, 0x01E0_21C8);
    fx.mem.set16(FACING, f);
    // rand's frames: the entry's s0 = D.
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_21E0, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x64u32 {
        fx.mem.write_u8(BYTES_B + k, 0);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, D, 0x01E0_2238, 5);
        fx.mem.set8(BYTES_B + k, v);
    }
    let f = facing_of(fx, sp + 0x90, 0x01E0_229C);
    for k in 0xB4..0x100u32 {
        let (ra, sign) = if f != 0 { (0x01E0_22B4, 1) } else { (0x01E0_22E4, -1) };
        let v = fx.rand_from(D, ra);
        fx.mem.set8(BYTES_B + k, sign * (v % 4));
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    // The entry's s1: the flash tasks counted here (the steps' saved s1).
    let mut flashes = 0u32;
    for k in 0..TASKS {
        let t = TASK0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        match f {
            SPRITE => sprite(fx, t, flashes),
            DISC => disc(fx, t, flashes),
            LIGHT => light(fx, t),
            MIRROR | WINDOW => flashes += 1,
            _ => debug_assert!(false, "b490: unported task step {f:#x}"),
        }
    }
    for k in 0..TASKS {
        let t = TASK0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x44) == SPRITE {
            tex(fx, TEX);
            cell(fx, t, sp + 0x70);
            for k in 0..3 {
                let v = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(sp + 0x60 + 4 * k, v);
            }
            billboard::sprite_spin(fx, 0x01E0_2790, uv::uv_rect, D, t, 0, sp + 0x60);
        }
        if fx.mem.read_u32(t + 0x44) == DISC {
            quads4::fan18_t(fx, D, t, sp + 0xA0);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    if flashes != 0 {
        unit_flash(fx, 0);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x5A => 1,
        0xF5 => 4,
        0xE6 => 2,
        0x65..=0xE0 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
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

/// 0x1E02150(D, step, 0): the first free task cleared and set off at
/// D+0x250 (i16 +0, float +0x10) with the colour D+0x8B04 (twice), kind
/// D+0x8B50 (+0x50); `saves` the caller's s0..s4, `ra` the return address.
fn spawn(fx: &mut Fx, step: u32, saves: [u64; 5], ra: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, saves[4]), (0x30, saves[3]), (0x20, saves[2]), (0x10, saves[1]), (0, saves[0])]);
    if let Some(k) = (0..TASKS).find(|&k| fx.mem.i16(TASK0 + 0x58 + 0x70 * k) == 0) {
        let t = TASK0 + 0x70 * k;
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
        let s = fx.mem.i16(KIND);
        fx.mem.set16(t + 0x50, s);
        fx.mem.write_u32(t + 0x40, 0);
        fx.mem.write_u32(t + 0x44, step);
    }
    fx.leave(0x60);
}

/// 0x1E00920(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the emitters (frame 0x50: ra, the entry's s2 = s1 = 0, s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x30, 0x01E0_2334), (0x20, 0), (0x10, 0), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    // The spawns' saves: the emitter's s0 / s1 / s2 (= D) and the entry's
    // s3 = the parameter, s4 the runner's (0 [H]).
    let p = param as i64 as u64;
    let saves = |s0: u64, s1: u64| [s0, s1, u64::from(D), p, 0];
    let node = |fx: &mut Fx, out: u32| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        fx.node_position(h, slot, out);
    };
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    if (0x23..0xB5).contains(&c(fx)) {
        spawn(fx, MIRROR, saves(u64::from(D), 0), 0x01E0_09B8);
    }
    if c(fx) == 0x23 {
        spawn(fx, WINDOW, saves(u64::from(D), 0), 0x01E0_09E4);
    }
    if c(fx) == 0x23 {
        node(fx, sp + 0x40);
        for k in 0..3 {
            let w = fx.mem.read_u32(sp + 0x40 + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
        spawn(fx, DISC, saves(u64::from(D), 0), 0x01E0_0A48);
    }
    if c(fx) == 0x23 {
        spawn(fx, LIGHT, saves(u64::from(D), 0), 0x01E0_0A74);
    }
    if (0x23..0xDD).contains(&c(fx)) {
        node(fx, sp + 0x40);
        let n = rmod(fx, D, 0x01E0_0AC0, 9) + 6;
        let s1 = n.max(0) as u32;
        for i in 0..s1 {
            for (o, ra, m, off) in [(0u32, 0x01E0_0AECu32, 0x3C, 30.0f32), (4, 0x01E0_0B20, 0x3C, 20.0), (8, 0x01E0_0B54, 0x1E, 15.0)] {
                let v = rmod(fx, i, ra, m);
                let p = ee::sub(ee::add(fx.mem.f(sp + 0x40 + o), ee::from_i32(v)), off);
                fx.mem.setf(POS + o, p);
            }
            spawn(fx, SPRITE, saves(u64::from(i), u64::from(s1)), 0x01E0_0B98);
        }
    }
    node(fx, NODE_P);
    fx.leave(0x50);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E00B80 (D, task): a grey sprite (size 50 x 8, +0x50 rand % 180,
/// +0x5A rand % 360) moving 7 a update toward the facing (its i16 z); 0x80
/// for 11 updates, out by 0xF; grey 0x80 / 0xC0 by its age's parity.
/// Frame 0x30 (the entry's s0 = D, s1 = the flashes).
fn sprite(fx: &mut Fx, t: u32, flashes: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_2368), (0x10, u64::from(flashes)), (0, u64::from(D))]);
    let f = SPRITE;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4248_0000);
        fx.mem.set32(t + 0x20, 0x32);
        fx.mem.set32(t + 0x24, 8);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, f + 0x58, 0xB4);
        fx.mem.set16(t + 0x50, v);
        let v = rmod(fx, t, f + 0x74, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0xC {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0x11 {
            fx.mem.write_u8(t + 0x63, a - 0xF);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    fx.mem.set16(t + 0x56, 2);
    let z = fx.mem.f(t + 0x18);
    let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 10.0) } else { ee::add(z, 10.0) };
    fx.mem.setf(t + 0x18, z);
    let g = if fx.mem.i16(t + 0x54).wrapping_rem(2) == 1 { 0x80 } else { 0xC0 };
    l2::rgb(fx, t + 0x60, g, g, g);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E00D00 (D, task): the flaming disc (rim radius 630, orange centre /
/// dark rim, angles 10 / 10) at the attacker's node 1; 0x80 for 150
/// updates, out by 0xA. Frame 0x30 (the entry's s0 = D, s1 = the flashes).
fn disc(fx: &mut Fx, t: u32, flashes: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_2388), (0x10, u64::from(flashes)), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x441D_8000);
        l2::rgb(fx, t + 0x60, 0xC0, 0x70, 0);
        l2::rgb(fx, t + 0x64, 0x20, 0x10, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0x10);
    }
    if fx.mem.i16(t + 0x54) < 0xAA {
        let a = fx.mem.read_u8(t + 0x63);
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0x11 {
            fx.mem.write_u8(t + 0x63, a - 0xA);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    age(fx, t);
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES);
    crate::b076::node_position_saving(fx, h, slot, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_0E78);
    fx.leave(0x30);
}

/// 0x1E00F70 (D, task): the stage light (0x1F7F10) down by 10 to 0x30
/// while the timer is at 21 or more, then back up by 10 to 0x80 (the task
/// ending there). Frame 0x10 (ra).
fn light(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x10);
    fx.saved(sp, &[(0, 0x01E0_23C8)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let l = fx.mem.i16(t + 0x50);
    if fx.mem.i32(TIMER) >= 0x15 {
        fx.mem.set16(t + 0x50, if l < 0x31 { 0x30 } else { l - 0xA });
    } else if l < 0x80 {
        fx.mem.set16(t + 0x50, l + 0xA);
    } else {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 0);
    }
    age(fx, t);
    let l = fx.mem.read_u8(t + 0x50);
    fx.set_stage_light(i32::from(l));
    fx.leave(0x10);
}

/// The colour +0x60 as RGBAQ (0x108020 into `local`) into the four vertices
/// of the descriptor `d` (+0x10, +0x28, +0x40, +0x58).
fn colours4(fx: &mut Fx, t: u32, d: u32, local: u32) {
    for k in 0..4u32 {
        l2::rgbaq(fx, local + 8 * k, t + 0x60);
        for o in [0u32, 4] {
            let w = fx.mem.read_u32(local + 8 * k + o);
            fx.mem.write_u32(d + 0x10 + 0x18 * k + o, w);
        }
    }
}

/// XYZ2 at `at`: Z = 0xFFFFFF, its top byte 0.
fn far_z(fx: &mut Fx, at: u32) {
    let v = fx.mem.read_u64(at);
    fx.mem.write_u64(at, (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
    fx.mem.write_u8(at + 7, 0);
}

/// 0x1E019B0(D, t): the window task: the target over the whole window
/// (white) as the 12-value strip at +0x8B14, no ordering table; in by 0x10
/// under 0xC0 until effect update 200, else out by 0x13; ends from update
/// 166. `regs` the unit flash's s0 / s1 (the slot, the flashes so far).
fn window(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, 0x01E0_2D00), (0x10, u64::from(regs[1])), (0, u64::from(regs[0]))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = fx.mem.read_u32(DESC);
    colours4(fx, t, d, sp + 0x30);
    for (o, x, y) in [(0x20u32, 0x6FF8u16, 0x6FF8u16), (0x38, 0x8FF8, 0x6FF8), (0x50, 0x6FF8, 0x8FF8), (0x68, 0x8FF8, 0x8FF8)] {
        fx.mem.write_u16(d + o, x);
        fx.mem.write_u16(d + o + 2, y);
        far_z(fx, d + o);
    }
    for (o, v) in [(0x18u32, 0u32), (0x1C, 0), (0x30, 0x3F80_0000), (0x34, 0), (0x48, 0), (0x4C, 0x3F80_0000), (0x60, 0x3F80_0000), (0x64, 0x3F80_0000)] {
        fx.mem.write_u32(d + o, v);
    }
    fx.submit(d, 0, 0);
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(COUNTER) < 0xC8 {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a + 0x10);
        }
    } else if a >= 0x15 {
        fx.mem.write_u8(t + 0x63, a - 0x13);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(COUNTER) >= 0xB8 {
        fx.mem.set16(t + 0x58, 0);
    }
    age(fx, t);
    fx.leave(0x50);
}

/// 0x204D80(D+0xB690 (`push`: moved 60 toward the facing), 0, `out`,
/// `w`): the attacker's node 1 projected into the XYZ2 at `out`.
fn project_node(fx: &mut Fx, sp: u32, out: u32, w: u32, push: bool) {
    for k in 0..3 {
        let v = fx.mem.read_u32(NODE_P + 4 * k);
        fx.mem.write_u32(sp + 0x80 + 4 * k, v);
    }
    if push {
        let z = fx.mem.f(sp + 0x88);
        let z = if fx.mem.i16(FACING) != 0 { ee::add(z, 60.0) } else { ee::sub(z, 60.0) };
        fx.mem.setf(sp + 0x88, z);
    }
    let p = [fx.mem.f(sp + 0x80), fx.mem.f(sp + 0x84), fx.mem.f(sp + 0x88)];
    let mut x = fx.mem.read_u64(out);
    if let Some(v) = fx.project(p, None, &mut x) {
        fx.mem.write_u64(out, x);
        fx.mem.setf(w, v);
    }
    project_residue(fx, sp, sp + 0x80);
}

/// A window corner: (0x900 + `v`) x 16 - 8 (`hi`) or (0x700 - `v`) x 16 - 8.
fn edge(v: u16, hi: bool) -> u16 {
    let e = if hi { v.wrapping_add(0x900) } else { 0x700u16.wrapping_sub(v) };
    (e << 4).wrapping_sub(8)
}

/// One vertex's ST from its screen XY: ((0x6FF8 - x) / -16 (at least 0 /
/// at most 512 as `cap`)) / 512.
fn st(fx: &mut Fx, d: u32, o: u32, caps: [bool; 2]) {
    let mut out = [0f32; 2];
    for (k, cap) in caps.into_iter().enumerate() {
        let c = fx.mem.read_u16(d + o + 2 * k as u32) as u32;
        let v = ee::sub(ee::div(ee::from_i32(0x6FF8u32.wrapping_sub(c) as i32), -16.0), 0.0);
        let v = if cap {
            if v <= 512.0 {
                v
            } else {
                512.0
            }
        } else if v < 0.0 {
            0.0
        } else {
            v
        };
        out[k] = ee::div(v, 512.0);
    }
    let s = match o {
        0x20 => 0x18,
        0x38 => 0x30,
        0x50 => 0x48,
        _ => 0x60,
    };
    fx.mem.setf(d + s, out[0]);
    fx.mem.setf(d + s + 4, out[1]);
}

/// 0x1E01140(D, t): a mirror task: the target in the four quadrants about
/// the attacker's node 1 (60 toward the facing; the strip's corners pushed
/// out to the window edges by the sizes +8 / +0xA, which grow by 20..34 a
/// update), dark grey 0x30 at 0x80, then out by 0x13; ends from effect
/// update 166. `regs` the unit flash's s0..s3 (the index, the flashes so
/// far, the slot, D).
fn mirror(fx: &mut Fx, t: u32, regs: [u32; 4]) {
    let sp = fx.enter(0xF0);
    fx.saved(sp, &[(0x60, 0x01E0_2CB4), (0x50, 0), (0x40, u64::from(TASKS)), (0x30, u64::from(regs[3])), (0x20, u64::from(regs[2])), (0x10, u64::from(regs[1])), (0, u64::from(regs[0]))]);
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x60, 0x38, 0x38, 0x38);
        fx.mem.write_u8(t + 0x63, 0x80);
        fx.mem.set16(t + 8, 0);
        fx.mem.set16(t + 0xA, 0);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = fx.mem.read_u32(DESC);
    colours4(fx, t, d, sp + 0xD0);
    project_node(fx, sp, d + 0x20, D + 0x8B1C, true);
    let w8 = |fx: &mut Fx| fx.mem.read_u16(t + 8);
    let wa = |fx: &mut Fx| fx.mem.read_u16(t + 0xA);
    for q in 0..4u32 {
        if q == 0 {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + 4);
            fx.node_position(h, slot, sp + 0xC0);
        }
        let out = d + 0x20 + 0x18 * q;
        project_node(fx, sp, out, sp + 0x70, false);
        let (x8, ya) = (w8(fx), wa(fx));
        let set = |fx: &mut Fx, o: u32, v: u16| fx.mem.write_u16(d + o, v);
        let get = |fx: &mut Fx, o: u32| fx.mem.read_u16(d + o);
        match q {
            0 => {
                set(fx, 0x38, edge(x8, true));
                let v = get(fx, 0x22);
                set(fx, 0x3A, v);
                let v = get(fx, 0x20);
                set(fx, 0x50, v);
                set(fx, 0x52, edge(ya, true));
                set(fx, 0x68, edge(x8, true));
                set(fx, 0x6A, edge(ya, true));
            }
            1 => {
                set(fx, 0x20, edge(x8, false));
                let v = get(fx, 0x3A);
                set(fx, 0x22, v);
                set(fx, 0x50, edge(x8, false));
                set(fx, 0x52, edge(ya, true));
                let v = get(fx, 0x38);
                set(fx, 0x68, v);
                set(fx, 0x6A, edge(ya, true));
            }
            2 => {
                let v = get(fx, 0x50);
                set(fx, 0x20, v);
                set(fx, 0x22, edge(ya, false));
                set(fx, 0x38, edge(x8, true));
                set(fx, 0x3A, edge(ya, false));
                set(fx, 0x68, edge(x8, true));
                let v = get(fx, 0x52);
                set(fx, 0x6A, v);
            }
            _ => {
                set(fx, 0x20, edge(x8, false));
                set(fx, 0x22, edge(ya, false));
                let v = get(fx, 0x68);
                set(fx, 0x38, v);
                set(fx, 0x3A, edge(ya, false));
                set(fx, 0x50, edge(x8, false));
                let v = get(fx, 0x6A);
                set(fx, 0x52, v);
            }
        }
        for o in [0x20u32, 0x38, 0x50, 0x68] {
            far_z(fx, d + o);
        }
        st(fx, d, 0x20, [false, false]);
        st(fx, d, 0x38, [true, false]);
        st(fx, d, 0x50, [false, true]);
        st(fx, d, 0x68, [true, true]);
        fx.submit(d, 0, 0);
    }
    for (o, ra) in [(8u32, 0x01E0_1C14u32), (0xA, 0x01E0_1C44)] {
        let v = fx.rand_from(4, ra);
        let r = fx.div(v >> 4, 0xF).1;
        let s = fx.mem.i16(t + o) + (r + 0x14) as i16 as i32;
        fx.mem.set16(t + o, s);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0x15 {
            fx.mem.write_u8(t + 0x63, a - 0x13);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    if fx.mem.i16(COUNTER) >= 0xB8 {
        fx.mem.set16(t + 0x58, 0);
    }
    age(fx, t);
    fx.leave(0xF0);
}

/// 0x1E02670(D): in a packet layer (0x2078A0) the attacker drawn into the
/// target (0x1F8270 filter 0, its TEX0 / TEX1 into the 12-value strip
/// descriptor at sp + 0xB0 (+0x78 / +0x80); ALPHA 0x48), TEXFLUSH before
/// and after; the mirror tasks then the window task stepped; the layer
/// linked into the ordering table at key 0 when any ran.
fn unit_flash(fx: &mut Fx, a3: u32) {
    let sp = fx.enter(0x140);
    fx.mem.write_u32(D + 0x8B34, sp + 0x58);
    let d = sp + 0xB0;
    fx.mem.write_u32(DESC, d);
    let layer = fx.layer_begin();
    fx.mem.write_u32(LAYER, layer);
    fx.mem.write_u16(d + 2, 0x34);
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    fx.texflush([d, 0xC, 0x34, a3]);
    fx.desc_alpha(d, 0x80_0000_0048);
    let h = fx.self_handle();
    let (_, regs) = fx.unit_draw(h, 0, d + 0x78, d + 0x80);
    fx.texflush(regs);
    let mut ran = 0u32;
    for k in 0..TASKS {
        let t = TASK0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) != 0 && fx.mem.read_u32(t + 0x44) == MIRROR {
            ran += 1;
            mirror(fx, t, [k, ran, D + 0x70 * k, D]);
        }
    }
    for k in 0..TASKS {
        let t = TASK0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) != 0 && fx.mem.read_u32(t + 0x44) == WINDOW {
            ran += 1;
            window(fx, t, [D + 0x70 * k, ran]);
        }
    }
    if ran != 0 {
        let layer = fx.mem.read_u32(LAYER);
        fx.layer_end(layer, OT, 0);
    }
    fx.leave(0x140);
}
