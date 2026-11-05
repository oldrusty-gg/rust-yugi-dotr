//! BMODULE b494 (card 46), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E013C0, data area 0x1ED0000) [F]: a task pool of 150 (0x70 bytes at +0x2A0, alive +0x58, step
//! +0x44 as a record address) drawn with quads (`billboard::quad`,
//! add5d81dd3fe at 0x1E00920, UVs by uv_rect 0x1E00080) and the record's
//! own fans: [`fan_tapered`] (0x1E01A90) and [`fan_raised`] (0x1E02400).
//! Eight random nodes of the attacker (0x1F66F0: its node count) are
//! picked at launch and again every 30th update; from update 25 to 234
//! every 4th update a blue mote rises from each, and every other update
//! two widening white fans (one rising from update 221) start at the
//! attacker (y 31). Returns 1 on update 40, 3 on 240, 6 on 51..234, 8 once
//! the 250-update timer +0x44B0 has run out.
use crate::bus::Bus;
use crate::blib::{billboard, uv};
use crate::c061::{blend_alpha, rgbaq_at, scale_tenth};
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const MOTE: u32 = 0x01E0_0EE0;
const FAN: u32 = 0x01E0_10D0;
const RAISED: u32 = 0x01E0_1230;
const IDLE: u32 = 0x01E0_13B0;
/// The return address of the entry's `jalr` into a step (0x1E016B0 + 8).
const TASK_RA: u64 = 0x01E0_16B8;
const TIMER: u32 = D + 0x44B0;
const COLOUR: u32 = D + 0x44B4;
const NODES: u32 = D + 0x44E4;
const ANGLES: u32 = D + 0x450C;
const FACING: u32 = D + 0x4524;
const COUNTER: u32 = D + 0x4528;
/// The scratchpad, where the originals project.
const SPR: u32 = 0x7000_0000;

/// The entry 0x1E013C0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    fx.mem.write_u32(D + 0x44BC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x80);
    ret
}

/// Eight random nodes of the attacker into +0x44E4 (node count = 0x1F66F0,
/// rand % count, 0 -> 1; the first request clears), rand from return
/// address `ra` with the caller's s0 = D + 2k.
fn pick_nodes(fx: &mut Fx, ra: u32) {
    for k in 0..8u32 {
        let h = fx.self_handle();
        let n = fx.unit_nodes(h);
        let v = fx.rand_from(D + 2 * k, ra);
        let mut node = fx.div(v, n).1 as i16 as i32;
        if node == 0 {
            node = 1;
        }
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k, r);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x48A0);
    l2::sine_table(fx, D + 0x472C);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x4526, param);
    fx.mem.set32(TIMER, 0xFA);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..2u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x4490 + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x70);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x74)), 180.0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = fx.rand();
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(D + 0x452A + k, h - 0xF);
    }
    for k in 0..0x100u32 {
        let v = fx.rand();
        let (_, h) = fx.div(v, 0x28);
        fx.mem.set8(D + 0x462A + k, h - 0x14);
    }
    pick_nodes(fx, 0x01E0_1578);
    for k in 0..8u32 {
        let v = fx.rand_from(k, 0x01E0_162C);
        let (_, h) = fx.div(v >> 4, 0x168);
        fx.mem.set16(ANGLES + 2 * k, h);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(slot + 0x2E4);
        if f == 0 || f == IDLE {
            continue;
        }
        let t = slot + 0x2A0;
        match f {
            MOTE => mote(fx, t),
            FAN => fan(fx, t, false),
            RAISED => fan(fx, t, true),
            _ => panic!("b494: unknown task step {f:#x}"),
        }
    }
    for k in 0..TASKS {
        let s = D + 0x70 * k;
        if fx.mem.i16(s + 0x2F8) == 0 {
            continue;
        }
        let t = s + 0x2A0;
        if fx.mem.read_u32(t + 0x44) == MOTE {
            let w = fx.mem.read_u32(D + 0x4494);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            l2::rect(fx, sp + 0x60, 0x14, sub);
            for k in 0..4 {
                let w = fx.mem.read_u32(sp + 0x60 + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, w);
            }
            fx.mem.set16(D + 0xBE, 0);
            let a = fx.mem.i16(t + 0x5C);
            let s = l2::sin_deg(fx, D + 0xD8, a, 0);
            let x = ee::add(fx.mem.f(t + 0x10), ee::mul(fx.mem.f(t + 0x48), s));
            fx.mem.setf(sp + 0x50, x);
            for o in [4u32, 8] {
                let w = fx.mem.read_u32(t + 0x10 + o);
                fx.mem.write_u32(sp + 0x50 + o, w);
            }
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, sp + 0x50);
        }
        for (f, h) in [(FAN, 0x3F), (RAISED, 0x1F)] {
            if fx.mem.read_u32(t + 0x44) != f {
                continue;
            }
            let w = fx.mem.read_u32(D + 0x4490);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0x94, 0);
            fx.mem.set16(D + 0x96, 0);
            fx.mem.set16(D + 0x98, 0x1F);
            fx.mem.set16(D + 0x9A, h);
            fx.mem.set16(D + 0xB8, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, w);
            }
            fx.mem.set16(D + 0xB8, 0x12);
            fx.mem.set16(D + 0xBE, 0);
            if f == FAN {
                fan_tapered(fx, D, t, 0x20, t + 0x10);
            } else {
                fan_raised(fx, D, t, 0x20, t + 0x10);
            }
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x28 => 1,
        0xF0 => 3,
        0x33..=0xEA => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E00BE0(D, 0): the update counter, the node picks, the motes, the
/// angles' turn and the fans. Frame 0x70 (the entry's s3 = 0, s2 = 0, s1
/// = D, s0 = the parameter).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x70);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x40, 0x01E0_1678), (0x30, 0), (0x20, 0), (0x10, u64::from(D)), (0, p)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    // The emitter's s2 / s0 at the fans' spawns.
    let (mut s2, mut s0) = (0u64, p);
    let c = fx.mem.i16(COUNTER);
    if (0x19..0xEB).contains(&c) {
        if fx.div(c, 0x1E).1 == 1 {
            pick_nodes(fx, 0x01E0_0C64);
        } else if c % 4 == 0 {
            for k in 0..8u32 {
                let h = fx.self_handle();
                let slot = fx.mem.i16(NODES + 2 * k);
                fx.node_position(h, slot, sp + 0x50);
                for o in 0..4 {
                    let w = fx.mem.read_u32(sp + 0x50 + 4 * o);
                    fx.mem.write_u32(D + 0x250 + 4 * o, w);
                }
                fx.mem.set16(D + 0x4520, k as i32);
                spawn(fx, MOTE, &[(0x50, 0x01E0_0D8C), (0x40, 0), (0x30, 0), (0x20, u64::from(k)), (0x10, u64::from(D)), (0, u64::from(D + 2 * k))]);
            }
            s2 = 8;
            s0 = u64::from(D + 0x10);
        }
    }
    for k in 0..8u32 {
        let a = fx.mem.i16(ANGLES + 2 * k) + 0xA;
        fx.mem.set16(ANGLES + 2 * k, a);
        let a = fx.mem.i16(ANGLES + 2 * k);
        if a >= 0x168 {
            let (_, h) = fx.div(a, 0x168);
            fx.mem.set16(ANGLES + 2 * k, h);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x19..0xEB).contains(&c) {
        let h = fx.self_handle();
        fx.unit_position(h, D + 0x250);
        fx.mem.write_u32(D + 0x254, 0x41F8_0000);
        let saves = |ra: u32| [(0x50u32, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, s2), (0x10, u64::from(D)), (0, s0)];
        if fx.mem.i16(COUNTER) % 2 == 0 {
            spawn(fx, FAN, &saves(0x01E0_0E88));
        }
        if fx.mem.i16(COUNTER) % 2 == 0 {
            spawn(fx, RAISED, &saves(0x01E0_0EBC));
        }
    }
    fx.leave(0x70);
}

/// 0x1E01940(D, step, 0): the first free task cleared and set off at
/// D+0x250 with the colour +0x44B4 twice, angle +0x4506 (+0x5A), node
/// index +0x4520 (+0x50). `saves`: its prologue's.
fn spawn(fx: &mut Fx, f: u32, saves: &[(u32, u64)]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x70 * k + 0x2F8) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for c in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(D + 0x250 + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let a = fx.mem.i16(D + 0x4506);
            fx.mem.set16(t + 0x5A, a);
            let a = fx.mem.i16(D + 0x4520);
            fx.mem.set16(t + 0x50, a);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// A step's prologue (frame 0x30: ra, the entry's s1 = D, s0 = the
/// task's slot t - 0x2A0).
fn prologue(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, u64::from(t - 0x2A0))]);
}

/// 0x1E00EE0: a blue mote (frame 0x30) of 12 x 12..41 at its node's
/// angle +0x5C, offset +0x48 growing by 0.1 to 0.8, rising by 0.6,
/// flickering, its cell frame cycling; in by 0x40 for 5 updates, then out
/// by 9.
fn mote(fx: &mut Fx, t: u32) {
    prologue(fx, t);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0xC);
        let v = fx.rand_from(t, 0x01E0_0F10);
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set16(t + 0xA, h + 0xC);
        fx.mem.write_u32(t + 0x48, 0);
        let v = fx.rand_from(t, 0x01E0_0F34);
        fx.mem.set16(t + 0x56, (v >> 4) % 8);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        let n = fx.mem.i16(t + 0x50) as u32;
        let a = fx.mem.i16((ANGLES).wrapping_add(n.wrapping_mul(2)));
        fx.mem.set16(t + 0x5C, a);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 5 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) % 2 == 1 {
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xFF);
    } else {
        l2::rgb(fx, t + 0x60, 0x70, 0x70, 0xC0);
    }
    let c = fx.mem.i16(t + 0x56) + 1;
    fx.mem.set16(t + 0x56, if c >= 8 { 0 } else { c });
    if fx.mem.i16(t + 0x54) != 0 {
        let y = ee::add(fx.mem.f(t + 0x14), f32::from_bits(0x3F19_999A));
        fx.mem.setf(t + 0x14, y);
        let r = fx.mem.f(t + 0x48);
        if r < f32::from_bits(0x3F4C_CCCD) {
            fx.mem.setf(t + 0x48, ee::add(r, f32::from_bits(0x3DCC_CCCD)));
        }
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_10AC);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x30);
}

/// 0x1E010D0 (radii 20 / -38, the inner 60 behind the outer) and
/// 0x1E01230 (`raised`: both radii equal, rising by 40 a update from
/// update 221), frame 0x30: a white fan widening by 950 / 11 a update,
/// turned 180 degrees (0x8000) with the facing; alpha 0xC0 for 15
/// updates, then out by 14; the age held at 15 from update 231.
fn fan(fx: &mut Fx, t: u32, raised: bool) {
    prologue(fx, t);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        fx.mem.setf(t + 0x4C, ee::sub(20.0, 58.0));
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x50, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.write_u32(t + 0x3C, 0x446D_8000);
        let s = ee::div(fx.mem.f(t + 0x3C), 11.0);
        fx.mem.setf(t + 0x3C, s);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let turn = if fx.mem.i16(FACING) != 0 { 0x8000 } else { 0 };
        fx.mem.write_u32(t + 0x24, turn);
    }
    if fx.mem.i16(t + 0x54) < 0xF {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0x10 {
            fx.mem.write_u8(t + 0x63, a - 0xE);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let r = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x3C));
    fx.mem.setf(t + 0x48, r);
    if raised {
        fx.mem.setf(t + 0x4C, r);
        if fx.mem.i16(COUNTER) >= 0xDD {
            let y = ee::add(fx.mem.f(t + 0x34), 40.0);
            fx.mem.setf(t + 0x34, y);
        }
    } else {
        fx.mem.setf(t + 0x4C, ee::sub(r, 60.0));
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    if fx.mem.i16(COUNTER) >= 0xE7 {
        fx.mem.set16(t + 0x54, 0xF);
    }
    fx.leave(0x30);
}

/// The fans' shared header: a strip3 (FST) of colours +0x60 / +0x64,
/// blend `mode >> 4`, UVs from D+0x94 in D+0xBE's orientation, texture
/// D+0x88, the four vertex colours from `colours` (locals, task offsets),
/// and the transform (scale 0.1, the task's angles +0x20, `pos`) at
/// `m`; `at` the position copy.
fn fan_header(fx: &mut Fx, d0: u32, t: u32, mode: i32, d: u32, colours: [(u32, u32, u32); 4], pos_copy: u32, scale: u32, trans: u32, m: u32) {
    l2::colour_strip4(fx, d, t + 0x60, 2);
    l2::colour_strip4(fx, d, t + 0x64, 1);
    let blend = (mode >> 4) as i16 as i32;
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x60, 0);
    fx.desc_alpha(d, blend_alpha(blend));
    let order = fx.mem.i16(d0 + 0xBE);
    l2::uv_strip3(fx, d, d0 + 0x94, order);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    for (local, c, to) in colours {
        rgbaq_at(fx, local, t + c, d + to);
    }
    scale_tenth(fx, scale);
    for k in 0..3 {
        let w = fx.mem.read_u32(pos_copy + 4 * k);
        fx.mem.write_u32(trans + 4 * k, w);
    }
    l2::xform_vp(fx, m, scale, t + 0x20, trans);
}

/// The four vertices of segment `a`..`b` (inner `r0`, outer `r1`) at
/// `v` (inner a, inner b, outer a, outer b; x / z only).
fn segment(fx: &mut Fx, tab: u32, v: u32, a: i32, b: i32, r0: f32, r1: f32) {
    for (angle, at) in [(a, 0u32), (b, 0x10)] {
        for (r, o) in [(r0, 0u32), (r1, 0x20)] {
            let c = l2::sin_deg(fx, tab, angle, 1);
            fx.mem.setf(v + at + o, ee::mul(r, c));
            let s = l2::sin_deg(fx, tab, angle, 0);
            fx.mem.setf(v + at + o + 8, ee::mul(r, s));
        }
    }
}

/// Project the strip and submit it by depth mode `depth` (0 the mean w at
/// `w`, 1 the far Z at key 0, else nothing).
fn fan_submit(fx: &mut Fx, d0: u32, d: u32, depth: i32, v: u32, m: u32, w: u32) {
    if depth <= 1 && l2::project(fx, v, SPR, m, d, w) {
        let ot = fx.mem.read_u32(d0 + 0x80);
        if depth == 0 {
            let key = ee::fixunssfsi(fx.mem.f(w)) as i32;
            fx.submit(d, ot, key);
        } else {
            l2::far_z_desc(fx, d, 1);
            fx.submit(d, ot, 0);
        }
    }
}

/// 0x1E01A90(D, task, mode, pos): D+0xB8 strips (strip3, FST) on a circle
/// at `pos` (scale 0.1) turned by +0x20, inner radius +0x48, outer +0x4C,
/// strip k from angle k x +0x5A to that + +0x5C degrees (i16 steps); the
/// first six fade in (alpha (a / 6) k .. (a / 6)(k + 1) of +0x63's colour)
/// and the last six fade out the same way, the rest coloured +0x60 inner /
/// +0x64 outer; frame 0x280 (f20 / f21 saved) [F].
pub fn fan_tapered(fx: &mut Fx, d0: u32, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x280);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0xA0 + 4 * k, w);
    }
    let span = fx.mem.i16(t + 0x5C);
    let d = sp + 0x168;
    fan_header(fx, d0, t, mode, d, [(sp + 0x1F8, 0x60, 0x10), (sp + 0x200, 0x64, 0x40), (sp + 0x208, 0x60, 0x28), (sp + 0x210, 0x64, 0x58)], sp + 0xA0, sp + 0x100, sp + 0xF0, sp + 0x110);
    for o in [0xB4u32, 0xC4, 0xD4, 0xE4] {
        fx.mem.set32(sp + o, 0);
    }
    let (inner, outer) = (fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
    let depth = mode & 0xF;
    let tab = d0 + 0xD8;
    let c = sp + 0x27C;
    let copy_colour = |fx: &mut Fx| {
        for k in 0..4 {
            let b = fx.mem.read_u8(t + 0x60 + k);
            fx.mem.write_u8(c + k, b);
        }
    };
    // a / 6 (0x2AAAAAAB high word, + the sign bit) times `n`, through LO / HI.
    let sixth = |fx: &mut Fx, n: i32| -> i32 {
        let a = i32::from(fx.mem.read_u8(t + 0x63));
        fx.mult(0x2AAA_AAAB, a);
        let q = fx.hilo().1 + ((a as u32) >> 31) as i32;
        fx.mult(q, n)
    };
    let mut a = 0i32;
    let mut k = 0i32;
    while k < fx.mem.i16(d0 + 0xB8) {
        let n = fx.mem.i16(d0 + 0xB8);
        if k < 6 {
            copy_colour(fx);
            if k == 0 {
                fx.mem.write_u8(c + 3, 0);
            }
            if k > 0 {
                let v = sixth(fx, k);
                fx.mem.write_u8(c + 3, v as u8);
            }
            rgbaq_at(fx, sp + 0x218, c, d + 0x10);
            rgbaq_at(fx, sp + 0x220, c, d + 0x40);
            copy_colour(fx);
            let v = sixth(fx, k + 1);
            fx.mem.write_u8(c + 3, v as u8);
            rgbaq_at(fx, sp + 0x228, c, d + 0x28);
            rgbaq_at(fx, sp + 0x230, c, d + 0x58);
        } else if k >= n - 6 {
            copy_colour(fx);
            // The original compares k with each of B8 - 6 .. B8 - 1 (B8
            // re-read each time); the matching one sets the alpha [F].
            for (off, times) in [(6, 6), (5, 5), (4, 4), (3, 3), (2, 2), (1, 1)] {
                if k == fx.mem.i16(d0 + 0xB8) - off {
                    let a = i32::from(fx.mem.read_u8(t + 0x63));
                    fx.mult(0x2AAA_AAAB, a);
                    let q = fx.hilo().1 + ((a as u32) >> 31) as i32;
                    fx.mem.write_u8(c + 3, q.wrapping_mul(times) as u8);
                }
            }
            rgbaq_at(fx, sp + 0x238, c, d + 0x10);
            rgbaq_at(fx, sp + 0x240, c, d + 0x40);
            copy_colour(fx);
            for (off, times) in [(6, 5), (5, 4), (4, 3), (3, 2), (2, 1)] {
                if k == fx.mem.i16(d0 + 0xB8) - off {
                    let a = i32::from(fx.mem.read_u8(t + 0x63));
                    fx.mult(0x2AAA_AAAB, a);
                    let q = fx.hilo().1 + ((a as u32) >> 31) as i32;
                    fx.mem.write_u8(c + 3, q.wrapping_mul(times) as u8);
                }
            }
            if k == fx.mem.i16(d0 + 0xB8) - 1 {
                fx.mem.write_u8(c + 3, 0);
            }
            rgbaq_at(fx, sp + 0x248, c, d + 0x28);
            rgbaq_at(fx, sp + 0x250, c, d + 0x58);
        } else {
            rgbaq_at(fx, sp + 0x258, t + 0x60, d + 0x10);
            rgbaq_at(fx, sp + 0x260, t + 0x60, d + 0x28);
            rgbaq_at(fx, sp + 0x268, t + 0x64, d + 0x40);
            rgbaq_at(fx, sp + 0x270, t + 0x64, d + 0x58);
        }
        let end = (span + a) as i16 as i32;
        segment(fx, tab, sp + 0xB0, a, end, inner, outer);
        fan_submit(fx, d0, d, depth, sp + 0xB0, sp + 0x110, sp + 0x150);
        k += 1;
        a = (a + fx.mem.i16(t + 0x5A)) as i16 as i32;
    }
    fx.leave(0x280);
}

/// 0x1E02400(D, task, mode, pos): `quads4::sparks` (21cb40d76bec) with
/// the inner vertices at height +0x34 and the outer at +0x30 (sparks
/// keeps 0); frame 0x220 [F].
pub fn fan_raised(fx: &mut Fx, d0: u32, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x220);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0xB0 + 4 * k, w);
    }
    let span = fx.mem.i16(t + 0x5C);
    let d = sp + 0x170;
    fan_header(fx, d0, t, mode, d, [(sp + 0x200, 0x60, 0x10), (sp + 0x208, 0x64, 0x40), (sp + 0x210, 0x60, 0x28), (sp + 0x218, 0x64, 0x58)], sp + 0xB0, sp + 0x110, sp + 0x100, sp + 0x120);
    for (o, h) in [(0xC4u32, 0x34u32), (0xD4, 0x34), (0xE4, 0x30), (0xF4, 0x30)] {
        let w = fx.mem.read_u32(t + h);
        fx.mem.write_u32(sp + o, w);
    }
    let (inner, outer) = (fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
    let depth = mode & 0xF;
    let tab = d0 + 0xD8;
    let mut a = 0i32;
    let mut k = 0i32;
    while k < fx.mem.i16(d0 + 0xB8) {
        let start = (fx.mem.i16(t + 0x5A) + a) as i16 as i32;
        let end = (a + fx.mem.i16(t + 0x5A) + span) as i16 as i32;
        segment(fx, tab, sp + 0xC0, start, end, inner, outer);
        fan_submit(fx, d0, d, depth, sp + 0xC0, sp + 0x120, sp + 0x160);
        k += 1;
        a = (a + fx.mem.i16(t + 0x5A)) as i16 as i32;
    }
    fx.leave(0x220);
}

/// 0x205580(out) called with the caller's s0 / s1 from return address
/// `ra`, leaving the whole residue below the caller's sp [F; run b494: a
/// fan's missing-texture TEX0 reads 0x1A2B80's saved ra]: 0x205580's frame
/// (0x30: s0, s1, ra), 0x1FC1C0's below it (0x30: the camera object, `out`,
/// ra 0x2055AC) and, with a camera, the frames of its two 0x1A2B80 calls
/// (the last, from 0x1FC28C, the one left).
pub(crate) fn camera_angles_saving(fx: &mut Fx, out: u32, s0: u64, s1: u64, ra: u32) {
    let f1 = fx.sp.wrapping_sub(0x30);
    fx.saved(f1, &[(0, s0), (0x10, s1), (0x20, u64::from(ra))]);
    let cam = fx.mem.read_u32(crate::exe::CAMERA_TABLE);
    let f2 = f1.wrapping_sub(0x30);
    fx.saved(f2, &[(0, u64::from(cam)), (0x10, u64::from(out)), (0x20, 0x0020_55AC)]);
    if cam != 0 && out != 0 {
        let pi = f32::from_bits(0x4049_0FDB);
        let save = fx.sp;
        fx.sp = f2;
        let p = ee::div(ee::mul(32768.0, fx.mem.f(cam + 8)), pi);
        fx.fix_residue(p, 0x001F_C24C);
        let y = ee::div(ee::mul(32768.0, fx.mem.f(cam + 4)), pi);
        fx.fix_residue(y, 0x001F_C28C);
        fx.sp = save;
    }
    l2::camera_angles_to(fx, out);
}
