//! BMODULE b585 (card 149), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E021F0, data area 0x1ED0000) [F] (helper A10, 31
//! Oct 2026): b646's layout (400 tasks of 0x70 bytes at D+0x2A0, alive
//! +0x58, step +0x44 called through `jalr`; spawned by 8154ee20301e at
//! 0x1E029F0, `spawn::task400_b4`) drawn with the second library: arcs
//! (e6dce326551d at 0x1E00080, `billboard::arc_strips`), textured arcs
//! (3a2db45c5a9a at 0x1E005B0, `quads4::arc_strips_tex`), sparks
//! (21cb40d76bec at 0x1E009C0, `quads4::sparks`) and full-window tints
//! (561a6131a635 at 0x1E003E0, `quads4::tint4`), plus b180's unit flash
//! (0x1E02B80: the attacker drawn into the target in a packet layer, the
//! flash tasks 0x1E01F10 stepped). Updates 1..6 a flash a update and, on
//! 1, the stage dimmed (0x1E01C20); 173 a lilac / white tint keyed by a
//! point 50 in front of the target (and on 245 another); 205 a green arc
//! at the attacker's node 0x42 and a frame flash (0x1E01D10); 245 a blue
//! arc there; 245..274 textured green arcs (three kinds) at node 0x42;
//! 205..279 on odd updates blue sparks there. Returns 1 on update 237, 4
//! on 310, 2 on 295, 6 on 248..289, 8 once the 315-update timer +0xB210
//! has run out.
use crate::b494::camera_angles_saving;
use crate::b590::project_residue;
use crate::blib::{billboard, quads4, spawn::task400_b4};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
const ARC: u32 = 0x01E0_10E0;
const ARC_T: u32 = 0x01E0_1210;
const SPARKS: u32 = 0x01E0_1720;
const TINT: u32 = 0x01E0_1920;
const ARC_N: u32 = 0x01E0_1AB0;
const LIGHT: u32 = 0x01E0_1C20;
const FRAME: u32 = 0x01E0_1D10;
/// The flash tasks, run by the unit flash 0x1E02B80.
const FLASH: u32 = 0x01E0_1F10;
/// The return address of the entry's `jalr` into a step (0x1E02500 + 8).
const TASK_RA: u64 = 0x01E0_2508;
const POS: u32 = D + 0x250;
const TEX: u32 = D + 0xB1F0;
const TIMER: u32 = D + 0xB210;
const LAYER: u32 = D + 0xB224;
const FLASH_DESC: u32 = D + 0xB228;
const TINT_W: u32 = D + 0xB234;
const NODES: u32 = D + 0xB248;
const KIND: u32 = D + 0xB2B4;
const FACING: u32 = D + 0xB2B8;
const COUNTER: u32 = D + 0xB2BC;
const BYTES: u32 = D + 0xB2BE;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E021F0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x150);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x150);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, D + 0xB4C0);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2BA, param);
    fx.mem.set32(TIMER, 0x13B);
    for (k, node) in [0x42, 0x4A, 0x35, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
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
    fx.fix_residue(y, 0x01E0_23EC);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_241C, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_246C);
        fx.mem.set16(D + 0xB268 + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    // The entry's s0: the flash tasks counted here (the steps' saved s0).
    let mut flashes = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        // The step's prologue: the entry's s0 = the flashes so far, s1 = 0.
        let frame = match f {
            0 => continue,
            FLASH => {
                flashes += 1;
                continue;
            }
            ARC_T | ARC_N => 0x30,
            FRAME => 0x90,
            ARC | SPARKS | TINT | LIGHT => 0x20,
            _ => panic!("b585: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        if frame == 0x20 {
            fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(flashes))]);
        } else {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(flashes))]);
        }
        match f {
            ARC => arc(fx, t),
            ARC_T => arc_t(fx, t),
            SPARKS => sparks(fx, t),
            TINT => tint(fx, t),
            ARC_N => arc_n(fx, t),
            LIGHT => light(fx, t),
            _ => frame_flash(fx, t, s),
        }
        fx.leave(frame);
    }
    // a3 as the draws' last library call left it, for the unit flash's
    // first 0x207BB0 (as b180) [run].
    let mut a3 = 0u32;
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
        if fx.mem.read_u32(t + 0x44) == ARC {
            fx.mem.set16(D + 0xB8, 0x24);
            let before = fx.out.prims.len();
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            a3 = last_xyz2(fx, before);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == ARC_T {
            let kind = |fx: &mut Fx| fx.mem.i16(t + 0x50);
            if kind(fx) == 0 {
                cell(fx, t, sp + 0x80, 0x97);
                tex(fx, D + 0x29C);
            }
            if kind(fx) == 1 {
                cell(fx, t, sp + 0x90, 0x9B);
                tex(fx, D + 0x29C);
            }
            if kind(fx) == 2 {
                tex(fx, TEX);
                fx.mem.set16(D + 0xBE, 0);
                let c = fx.mem.i16(t + 0x56);
                for (o, v) in [(0x94u32, c << 5), (0x96, 0), (0x98, 0x20), (0x9A, 0x40)] {
                    fx.mem.set16(D + o, v);
                }
            }
            fx.mem.set16(D + 0xB8, 0);
            copy3(fx, t + 0x10, D + 0x20);
            fx.mem.set16(D + 0xB8, 0x24);
            if kind(fx) == 0 {
                fx.mem.set16(D + 0xB8, 0x18);
            }
            if kind(fx) == 2 {
                fx.mem.set16(D + 0xB8, 6);
            }
            fx.mem.set16(D + 0xBE, 0);
            let before = fx.out.prims.len();
            quads4::arc_strips_tex(fx, D, t, 0x20, t + 0x10);
            a3 = last_xyz2(fx, before);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == SPARKS {
            l2::rect(fx, sp + 0xA0, 0x96, 2);
            for k in 0..4 {
                let v = fx.mem.read_u32(sp + 0xA0 + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, v);
            }
            tex(fx, D + 0x29C);
            fx.mem.set16(D + 0xB8, 0);
            copy3(fx, t + 0x10, D + 0x20);
            fx.mem.set16(D + 0xB8, 0xC);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sparks(fx, D, t, 0x20, t + 0x10);
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == TINT {
            let h = fx.target_handle();
            fx.unit_position(h, sp + 0x70);
            // 0x1F71A0's frame below the entry's (its saved ra, s0).
            fx.saved(sp - 0x20, &[(0x10, 0x01E0_27C8), (0, u64::from(flashes))]);
            let z = fx.mem.f(sp + 0x78);
            let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 50.0) } else { ee::add(z, 50.0) };
            fx.mem.setf(sp + 0x78, z);
            let p = [fx.mem.f(sp + 0x70), fx.mem.f(sp + 0x74), fx.mem.f(sp + 0x78)];
            let mut x = fx.mem.read_u64(sp + 0xD8);
            if let Some(w) = fx.project(p, None, &mut x) {
                fx.mem.write_u64(sp + 0xD8, x);
                fx.mem.setf(TINT_W, w);
            }
            project_residue(fx, sp, sp + 0x70);
            for (kind, mode) in [(0, 0x20), (1, 0x21), (2, 0x20)] {
                if fx.mem.i16(t + 0x50) == kind {
                    let w = fx.mem.f(TINT_W);
                    quads4::tint4(fx, t, mode, w);
                    // tint4 leaves a3 = the flags 0x207E20 loaded, or with
                    // the far Z (depth 0) the low word of vertex 1's XYZ2 it
                    // loaded at +0x18 (0x1E016F0) [run].
                    if let Some(p) = fx.out.prims.last() {
                        let d = p.desc;
                        a3 = if mode & 0xF == 0 { fx.mem.read_u32(d + 0x18) } else { u32::from(fx.mem.read_u16(d + 2)) };
                    }
                }
            }
        }
        if fx.mem.read_u32(t + 0x44) == ARC_N {
            fx.mem.set16(D + 0xB8, 0x24);
            let before = fx.out.prims.len();
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            a3 = last_xyz2(fx, before);
            fx.mem.set16(D + 0xB8, 0);
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
        0xED => 1,
        0x136 => 4,
        0x127 => 2,
        0xF8..=0x121 => 6,
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

/// 0x1E00DC0(D, 0): the update counter and the emitters (frame 0x20: ra,
/// the entry's s0 = 0).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, 0x01E0_24C8), (0, 0)]);
    let p = param as i64 as u64;
    // The spawn's saves: s0 = D, the entry's s1 = 0, s2 = D, s3 = the
    // parameter, s4 the runner's (0 [H]).
    let spawn = |fx: &mut Fx, f: u32, ra: u32| {
        task400_b4(fx, D, f, 0, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, p), (0x20, u64::from(D)), (0x10, 0), (0, u64::from(D))]);
    };
    let node = |fx: &mut Fx| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        fx.node_position(h, slot, POS);
    };
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    if c(fx) == 1 {
        spawn(fx, LIGHT, 0x01E0_0E10);
    }
    if c(fx) == 0xAD {
        spawn(fx, TINT, 0x01E0_0E3C);
    }
    if c(fx) < 7 {
        fx.mem.set16(KIND, 0);
        spawn(fx, FLASH, 0x01E0_0E74);
    }
    if c(fx) == 0xCD {
        fx.mem.set16(KIND, 0);
        spawn(fx, ARC_N, 0x01E0_0EAC);
        spawn(fx, FRAME, 0x01E0_0EC0);
    }
    if (0xF5..0x119).contains(&c(fx)) {
        node(fx);
        fx.mem.set16(KIND, 2);
        if c(fx) < 0x113 {
            spawn(fx, ARC_T, 0x01E0_0F40);
        }
        let n = c(fx);
        if n < 0x113 && n.wrapping_rem(2) == 0 {
            spawn(fx, ARC_T, 0x01E0_0F88);
        }
        fx.mem.set16(KIND, 1);
        let n = c(fx);
        if n < 0x113 && n.wrapping_rem(2) == 1 {
            spawn(fx, ARC_T, 0x01E0_0FE4);
        }
        if c(fx) == 0xF5 {
            spawn(fx, ARC, 0x01E0_1010);
        }
        if c(fx) == 0xF5 {
            spawn(fx, TINT, 0x01E0_103C);
        }
    }
    if (0xCD..0x119).contains(&c(fx)) {
        node(fx);
        fx.mem.set16(KIND, 2);
        if c(fx).wrapping_rem(2) == 1 {
            spawn(fx, SPARKS, 0x01E0_10D0);
        }
    }
    fx.leave(0x20);
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

/// The arcs' camera turn and growth: turned by the camera's angles (its
/// first + 0x4000), the outer radius up by 2.5 below 220 after the first
/// update.
fn turn_grow(fx: &mut Fx, t: u32, ra: u32) {
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), ra);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.f(t + 0x4C);
        if r < 220.0 {
            fx.mem.setf(t + 0x4C, ee::add(r, 2.5));
        }
    }
}

/// 0x1E010E0 (D, task): a blue arc (outer radius 260, inner grey 0xC0,
/// angles 10 / 10); in by 0x40 under 0xC0 for 14 updates, out by 0xF.
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x4382_0000);
        l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xFF);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0xE {
        fade_in(fx, t, 0x67, 0xC0, 0x40);
    } else {
        fade_out(fx, t, 0x67, 0x11, 0xF, true);
    }
    turn_grow(fx, t, 0x01E0_11A0);
    age(fx, t);
}

/// 0x1E01AB0 (D, task): a green arc at the node +0x50 (outer radius 60,
/// inner grey 0xC0, angles 10 / 10); in by 0xC under 0xC0 until effect
/// update 280, then out by 0xF.
fn arc_n(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x4270_0000);
        l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x60, 0x80, 0xFF, 0x80);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(COUNTER) < 0x118 {
        fade_in(fx, t, 0x67, 0xC0, 0xC);
    } else {
        fade_out(fx, t, 0x67, 0x11, 0xF, true);
    }
    turn_grow(fx, t, 0x01E0_1B80);
    age(fx, t);
    let h = fx.self_handle();
    let k = fx.mem.i16(t + 0x50) as u32;
    let slot = fx.mem.i16(NODES + 2 * k);
    crate::b076::node_position_saving(fx, h, slot, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_1C00);
}

/// 0x1E01210 (D, task): a textured green arc (radius 20 growing by
/// 35..44.9 (a quarter of it for kinds 1 / 2), the inner +0x4C a share of
/// it), three kinds by +0x50 (green on dark / bright by the effect
/// update's parity, kind 0 spinning +0x24 by 0x80 either way); facing away
/// (+0x20 -0x4000 / 0x4000), +0x28 random; 0x80, out by 0xB (b757's
/// 0x1E01FD0 with other steps).
fn arc_t(fx: &mut Fx, t: u32) {
    let f = ARC_T;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        fx.mem.setf(t + 0x4C, ee::sub(20.0, 58.0));
        let parity = |fx: &mut Fx| fx.mem.i16(COUNTER).wrapping_rem(2);
        let colours: [((i32, i32, i32), (i32, i32, i32)); 3] = [((0x80, 0xC0, 0x80), (0x80, 0xFF, 0x80)), ((0x60, 0x80, 0x60), (0x40, 0xFF, 0x40)), ((0x50, 0x50, 0x50), (0x60, 0x60, 0x60))];
        let dark = [8, 8, 0x28];
        let light = [0x38, 0x38, 0x30];
        for kind in 0..3 {
            if fx.mem.i16(t + 0x50) == kind {
                let (even_c, odd_c) = colours[kind as usize];
                if parity(fx) == 0 {
                    l2::rgb(fx, t + 0x64, even_c.0, even_c.1, even_c.2);
                    let g = dark[kind as usize];
                    l2::rgb(fx, t + 0x60, g, g, g);
                }
                if parity(fx) == 1 {
                    l2::rgb(fx, t + 0x64, odd_c.0, odd_c.1, odd_c.2);
                    let g = light[kind as usize];
                    l2::rgb(fx, t + 0x60, g, g, g);
                }
                if kind == 1 {
                    fx.mem.write_u32(t + 0x34, 0x4120_0000);
                }
                if kind == 2 {
                    fx.mem.write_u32(t + 0x34, 0x428C_0000);
                }
                fx.mem.set16(t + 6, if kind == 0 { 8 } else { 0xA });
            }
        }
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.set16(t + 0x5A, 0xF);
            fx.mem.set16(t + 0x5C, 0xF);
            let v = fx.rand_from(t, f + 0x2A0);
            fx.mem.set16(t + 0x52, (v >> 4) % 2);
        }
        for (kind, a, b) in [(1, 0xA, 3), (2, 0x3C, 0x3C)] {
            if fx.mem.i16(t + 0x50) == kind {
                fx.mem.set16(t + 0x5A, a);
                fx.mem.set16(t + 0x5C, b);
            }
        }
        let v = rmod(fx, t, f + 0x300, 0x64);
        fx.mem.setf(t + 0x38, ee::from_i32(v + 0x15E));
        let v = ee::div(fx.mem.f(t + 0x38), 10.0);
        fx.mem.setf(t + 0x38, v);
        fx.mem.write_u32(t + 0x30, 0x43AF_0000);
        let v = ee::div(fx.mem.f(t + 0x30), 10.0);
        fx.mem.setf(t + 0x30, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let a = if fx.mem.i16(FACING) != 0 { -0x4000 } else { 0x4000 };
        fx.mem.set32(t + 0x20, a);
        let v = rmod(fx, t, f + 0x380, 0xFFFF);
        fx.mem.set32(t + 0x28, v);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x67, 0x80);
    } else {
        fade_out(fx, t, 0x67, 0xD, 0xB, true);
    }
    if fx.mem.i16(t + 0x50) == 0 {
        let s = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x38));
        fx.mem.setf(t + 0x48, s);
        fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(0x3E4C_CCCD), s));
        let by = if fx.mem.i16(t + 0x52) == 0 { 0x80 } else { -0x80 };
        let a = fx.mem.i32(t + 0x24).wrapping_add(by);
        fx.mem.set32(t + 0x24, a);
    }
    for (kind, share) in [(1, 0x3F33_3333u32), (2, 0x3E99_999A)] {
        if fx.mem.i16(t + 0x50) == kind {
            let q = ee::div(fx.mem.f(t + 0x38), 4.0);
            let s = ee::add(fx.mem.f(t + 0x48), q);
            fx.mem.setf(t + 0x48, s);
            fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(share), s));
            let v = fx.mem.f(t + 0x38);
            add(fx, t + 0x34, v);
        }
    }
    age(fx, t);
}

/// 0x1E01720 (D, task): blue sparks (radius 80 growing by 10..14.9, the
/// inner a fifth of it; kind (rand >> 4) % 2: lilac on black / pale blue on
/// grey), angles 30 / 30, turned at random; 0x80 for 8 updates, out by 5.
fn sparks(fx: &mut Fx, t: u32) {
    let f = SPARKS;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42A0_0000);
        fx.mem.setf(t + 0x4C, ee::sub(80.0, 58.0));
        let v = fx.rand_from(t, f + 0x40);
        fx.mem.set16(t + 0x50, (v >> 4) % 2);
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xC0);
            l2::rgb(fx, t + 0x64, 0, 0, 0);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0xA0, 0xA0, 0xFF);
            l2::rgb(fx, t + 0x64, 0x50, 0x50, 0x50);
        }
        fx.mem.set16(t + 0x5A, 0x1E);
        fx.mem.set16(t + 0x5C, 0x1E);
        let v = rmod(fx, t, f + 0xD8, 0x32);
        fx.mem.setf(t + 0x34, ee::from_i32(v + 0x64));
        let v = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        for (o, ra) in [(0x20u32, 0x120u32), (0x24, 0x13C), (0x28, 0x158)] {
            let v = rmod(fx, t, f + ra, 0xFFFF);
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
    fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(0x3E4C_CCCD), s));
    age(fx, t);
}

/// 0x1E01920 (D, task): a tint's colours by its kind +0x50 (0 white, 1
/// lilac, life 15; 2 grey, 22); both alphas in by 0x19 under 0x80 while
/// young, then out by 4 (the task ends when the first is below 6, the
/// second is cleared).
fn tint(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let kinds: [([i32; 3], [i32; 3], i32); 3] = [([0xF0, 0xF0, 0xF0], [0xF0, 0xF0, 0xF0], 0xF), ([0xE0, 0xB0, 0xF0], [0xD0, 0xA0, 0xF0], 0xF), ([0x58, 0x58, 0x78], [0x58, 0x58, 0x78], 0x16)];
        for (kind, (a, b, life)) in kinds.into_iter().enumerate() {
            if fx.mem.i16(t + 0x50) == kind as i32 {
                l2::rgb(fx, t + 0x60, a[0], a[1], a[2]);
                l2::rgb(fx, t + 0x64, b[0], b[1], b[2]);
                fx.mem.set16(t + 6, life);
            }
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

/// 0x1E01C20 (D, task): the stage light (0x1F7F10) down by 10 to 0x30 for
/// 151 updates, back up by 10 to 0x80 once the timer is under 30; 0x80 and
/// the task's end when the timer is at 3 (b757's 0x1E02BC0).
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x97 {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x31 { 0x30 } else { l - 0xA });
    } else if fx.mem.i32(TIMER) < 0x1E {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x80 { l + 0xA } else { 0x80 });
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

/// 0x1E01D10 (D, task): a frame flash (b590's 0x1E02450 shape: grey 0x80
/// / 0x80 / 0x90, ST from +0x48 = 2.5): in by 0x16 under 0x58 until effect
/// update 280, then out by 4.
fn frame_flash(fx: &mut Fx, t: u32, sp: u32) {
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
    if fx.mem.i16(COUNTER) < 0x118 {
        fade_in(fx, t, 0x63, 0x58, 0x16);
    } else {
        fade_out(fx, t, 0x63, 6, 4, true);
    }
    age(fx, t);
}

/// 0x1E01F10(D, t): the flash's frame sprite (the target over the window,
/// dark grey 0x30, UVs inset by +8 / 10 and +0xA / 10 over 512) into the
/// descriptor at +0xB228, no ordering table; alpha up by 9 below 0x80 for
/// 156 updates, then down by 7, the sizes growing by 30 from its update
/// 157. `regs` the unit flash's s0 / s1 (the slot index, the live count).
fn flash(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_2C8C), (0x20, u64::from(regs[1])), (0x10, u64::from(regs[0]))]);
    // swc1 f20 / f21: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 4, 0);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let h = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0x30, 0x30, 0x30);
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
    if fx.mem.i16(t + 0x54) >= 0x9D {
        for o in [8u32, 0xA] {
            let s = fx.mem.i16(t + o) + 0x1E;
            fx.mem.set16(t + o, s);
        }
    }
    if fx.mem.i16(t + 0x54) < 0x9C {
        fade_in(fx, t, 0x63, 0x80, 9);
    } else {
        fade_out(fx, t, 0x63, 9, 7, true);
    }
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E02B80(D) (b180's 0x1E02DF0, d1cf0abc65df): in a packet layer
/// (0x2078A0) the attacker drawn into the target (0x1F8270 filter 0; ALPHA
/// 0x48), TEXFLUSH before and after; every flash task stepped (0x1E01F10);
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
