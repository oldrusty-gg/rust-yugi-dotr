//! BMODULE b627 (card 81), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E028A0, data area 0x1ED0000) [F]: a task pool of 150 (0x70 bytes at +0x2A0, alive +0x58, step
//! +0x44 as a record address) drawn with the second library: quads
//! (`billboard::quad`, 0x1E00ED0), rings (`billboard::ring_quads`,
//! 0x1E00920) and arc strips (`billboard::arc_strips`, 0x1E01190), UVs by
//! uv_rect 0x1E00080. A point rides the attacker's node 0x35 (+0x4480):
//! on update 1 a flickering flare and a red arc ring start there; until
//! update 330 a grey arc ring and a white ring of sparks start there every
//! update, from 70 also a pink quad; on update 328 a burst: 36 pink quads
//! flying out on a circle, 15 growing white arcs, 20 rings, a wide arc
//! ring and a whole-window frame flash (0x1E02640: a sprite sampling the
//! frame, 0x207EF0). Returns 1 on update 318, 4 on 353, 2 on 343, 6 on
//! 329..337, 8 once the 365-update timer +0x44C0 has run out.
use crate::blib::{billboard, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const FLARE: u32 = 0x01E0_1830;
const SPARKS: u32 = 0x01E0_1A30;
const ARC: u32 = 0x01E0_1C40;
const RED_ARC: u32 = 0x01E0_1DB0;
const PINK: u32 = 0x01E0_1F40;
const BURST: u32 = 0x01E0_2060;
const WIDE: u32 = 0x01E0_2210;
const WHITE_ARC: u32 = 0x01E0_2380;
const RING: u32 = 0x01E0_2480;
const FLASH: u32 = 0x01E0_2640;
const IDLE: u32 = 0x01E0_2890;
/// The return address of the entry's `jalr` into a step (0x1E02B30 + 8).
const TASK_RA: u64 = 0x01E0_2B38;
const TIMER: u32 = D + 0x44C0;
const COLOUR: u32 = D + 0x44C4;
const POINT: u32 = D + 0x4480;
const COUNTER: u32 = D + 0x45A2;
const BYTES: u32 = D + 0x45A4;
/// The degree sine table 0x108910 reads.
const SINES: u32 = D + 0x47A4;

/// The entry 0x1E028A0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    fx.mem.write_u32(D + 0x44CC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xC0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4910);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x45A0, param);
    fx.mem.set32(TIMER, 0x16D);
    for (at, node, clear) in [(0x4544u32, 0x35, 1), (0x4546, 0x34, 0), (0x4548, 0x35, 0), (0x454A, 0x30, 0), (0x454C, 0x2F, 0), (0x454E, 0x2C, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(D + at, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0x4554, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..3u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x44A0 + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.mem.set16(D + 0x459E, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = fx.rand();
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0..0x100u32 {
        let v = fx.rand();
        let (_, h) = fx.div(v, 0x28);
        fx.mem.set8(D + 0x46A4 + k, h - 0x14);
    }
    fx.mem.set16(D + 0x456E, 0x10E);
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
        if f != 0 && f != IDLE {
            let t = slot + 0x2A0;
            match f {
                FLARE => flare(fx, t),
                SPARKS => sparks(fx, t),
                ARC => arc(fx, t),
                RED_ARC => red_arc(fx, t),
                PINK => pink(fx, t),
                BURST => burst(fx, t),
                WIDE => wide(fx, t),
                WHITE_ARC => white_arc(fx, t),
                RING => ring(fx, t),
                FLASH => flash(fx, t),
                _ => panic!("b627: unknown task step {f:#x}"),
            }
        }
    }
    for k in 0..TASKS {
        let s = D + 0x70 * k;
        if fx.mem.i16(s + 0x2F8) == 0 {
            continue;
        }
        let t = s + 0x2A0;
        let p = sp + 0x50;
        if fx.mem.read_u32(t + 0x44) == FLARE {
            texture(fx, 0x44A0);
            cell(fx, t, sp + 0x60);
            let sub = fx.mem.i16(t + 0x56);
            fx.mem.set16(D + 0x94, sub << 6);
            fx.mem.set16(D + 0x96, 0);
            fx.mem.set16(D + 0x98, 0x3F);
            fx.mem.set16(D + 0x9A, 0x3F);
            fx.mem.set16(D + 0xBE, 5);
            copy(fx, POINT, p, 4);
            billboard::quad(fx, uv::uv_rect, D, t, 0x21, p);
        }
        if fx.mem.read_u32(t + 0x44) == SPARKS {
            texture(fx, 0x44A4);
            cell(fx, t, sp + 0x70);
            rings(fx, t, 0x20);
        }
        for f in [ARC, RED_ARC] {
            if fx.mem.read_u32(t + 0x44) == f {
                arcs(fx, t, 0x20, POINT);
            }
        }
        if fx.mem.read_u32(t + 0x44) == PINK {
            texture(fx, 0x44A8);
            cell(fx, t, sp + 0x80);
            fx.mem.set16(D + 0xBE, 5);
            copy(fx, t + 0x10, p, 4);
            billboard::quad(fx, uv::uv_rect, D, t, 0x21, p);
        }
        if fx.mem.read_u32(t + 0x44) == BURST {
            texture(fx, 0x44A8);
            cell(fx, t, sp + 0x90);
            fx.mem.set16(D + 0xBE, 5);
            copy(fx, t + 0x10, p, 3);
            billboard::quad(fx, uv::uv_rect, D, t, 0x10, p);
        }
        if fx.mem.read_u32(t + 0x44) == WIDE {
            arcs(fx, t, 0x20, t + 0x10);
        }
        if fx.mem.read_u32(t + 0x44) == WHITE_ARC {
            arcs(fx, t, 0x21, POINT);
        }
        if fx.mem.read_u32(t + 0x44) == RING {
            texture(fx, 0x44A4);
            cell(fx, t, sp + 0xA0);
            rings(fx, t, 0x21);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x13E => 1,
        0x161 => 4,
        0x157 => 2,
        0x149..=0x151 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

fn copy(fx: &mut Fx, from: u32, to: u32, n: u32) {
    for k in 0..n {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// D+0x88 = the texture word at D + `at`.
fn texture(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(D + at);
    fx.mem.write_u32(D + 0x88, w);
}

/// The atlas cell 0x14 / +0x56 (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32) {
    let sub = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, 0x14, sub);
    copy(fx, r, D + 0x94, 4);
}

/// Four billboards on a ring (`ring_quads`, layout 0).
fn rings(fx: &mut Fx, t: u32, mode: i32) {
    fx.mem.set16(D + 0xB8, 4);
    fx.mem.set16(D + 0xB0, 0);
    billboard::ring_quads(fx, uv::uv_rect, D, t, mode, BYTES);
    fx.mem.set16(D + 0xB8, 0);
    fx.mem.set16(D + 0xB0, 0);
}

/// 24 arc strips at `pos`.
fn arcs(fx: &mut Fx, t: u32, mode: i32, pos: u32) {
    fx.mem.set16(D + 0xB8, 0x18);
    billboard::arc_strips(fx, D, t, mode, pos);
    fx.mem.set16(D + 0xB8, 0);
}

/// 0x1E014F0(D, 0): the update counter, the point and the spawns. Frame
/// 0x60 (the entry's s2 = the parameter, s1 = 0, s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x60);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, 0x01E0_2AF8), (0x20, p), (0x10, 0), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = fx.mem.i16(COUNTER);
    let node = |fx: &mut Fx| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + 0x4544);
        fx.node_position(h, slot, D + 0x4470);
    };
    if c < 0x46 {
        node(fx);
        let y = ee::add(fx.mem.f(D + 0x4474), 3.0);
        fx.mem.setf(D + 0x4474, y);
    } else {
        if c == 0x46 {
            let h = fx.target_handle();
            let slot = fx.mem.i16(D + 0x4554);
            fx.node_position(h, slot, sp + 0x40);
            let d = ee::div(ee::sub(fx.mem.f(sp + 0x48), fx.mem.f(D + 0x4478)), 50.0);
            fx.mem.setf(D + 0x4498, d);
        }
        if fx.mem.i16(COUNTER) < 0x78 {
            let z = ee::add(fx.mem.f(D + 0x4478), fx.mem.f(D + 0x4498));
            fx.mem.setf(D + 0x4478, z);
            if fx.mem.i16(COUNTER) == 0x64 {
                let a = fx.mem.i16(D + 0x456E);
                let s = l2::sin_deg(fx, SINES, a, 0);
                let y = ee::sub(fx.mem.f(D + 0x4474), ee::mul(30.0, s));
                fx.mem.setf(D + 0x4474, y);
            }
            if fx.mem.i16(COUNTER) >= 0x65 {
                let y = ee::add(fx.mem.f(D + 0x4474), fx.mem.f(D + 0x4494));
                fx.mem.setf(D + 0x4474, y);
                let a = fx.mem.i16(D + 0x456E) + 5;
                fx.mem.set16(D + 0x456E, a);
            }
        }
    }
    node(fx);
    copy(fx, D + 0x4470, POINT, 4);
    let saves = |ra: u32, s2: u64, s1: u64| [(0x50u32, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, s2), (0x10, s1), (0, u64::from(D))];
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, RED_ARC, &saves(0x01E0_1678, p, 0));
        spawn(fx, FLARE, &saves(0x01E0_168C, p, 0));
    }
    copy(fx, POINT, D + 0x250, 4);
    if fx.mem.i16(COUNTER) < 0x14B {
        spawn(fx, ARC, &saves(0x01E0_16D0, p, 0));
        spawn(fx, SPARKS, &saves(0x01E0_16E4, p, 0));
        if fx.mem.i16(COUNTER) >= 0x46 {
            spawn(fx, PINK, &saves(0x01E0_1708, p, 0));
        }
    }
    if fx.mem.i16(COUNTER) == 0x148 {
        for k in 0..0x24u32 {
            fx.mem.set16(D + 0x459A, 10 * k as i32);
            spawn(fx, BURST, &saves(0x01E0_173C, u64::from(10 * k), u64::from(k)));
        }
        let s2 = 0x168;
        for k in 0..0xFu32 {
            spawn(fx, WIDE, &saves(0x01E0_1788, s2, u64::from(k)));
        }
        for k in 0..0x14u32 {
            spawn(fx, RING, &saves(0x01E0_17C8, s2, u64::from(k)));
        }
        spawn(fx, WHITE_ARC, &saves(0x01E0_1800, s2, 0x14));
        spawn(fx, FLASH, &saves(0x01E0_1814, s2, 0x14));
    }
    fx.leave(0x60);
}

/// 0x1E02FF0(D, step, 0): the first free task cleared and set off at
/// D+0x250 with the colour +0x44C4 twice, angle +0x459A (+0x50).
/// `saves`: its prologue's.
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
            copy(fx, D + 0x250, t + 0x10, 3);
            let a = fx.mem.i16(D + 0x459A);
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

/// rand % m (HI) from a step's `jal` at `site` (its s0 = the task).
fn rmod(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, site + 8);
    fx.div(v, m).1
}

/// 0x205580(t + 0x20) from a step's `jal` at `site` (0x205580's frame
/// 0x30: the step's s1 / s0 = t and the return address).
fn angles(fx: &mut Fx, t: u32, s1: u32, site: u32) {
    let base = fx.sp.wrapping_sub(0x30);
    fx.saved(base, &[(0x20, u64::from(site + 8)), (0x10, u64::from(s1)), (0, u64::from(t))]);
    l2::camera_angles_to(fx, t + 0x20);
}

/// The step's prologue (frame `bytes`: ra, s1 = D when it saves s1, s0 =
/// the entry's 0).
fn prologue(fx: &mut Fx, bytes: u32) {
    let sp = fx.enter(bytes);
    if bytes == 0x20 {
        fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
    } else {
        fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
    }
}

/// Alpha byte `a`: + `up` below `cap`.
fn up(fx: &mut Fx, a: u32, cap: u8, by: u8) {
    let v = fx.mem.read_u8(a);
    if v < cap {
        fx.mem.write_u8(a, v.wrapping_add(by));
    }
}

/// Alpha byte `a`: - `by` from `floor` up, else the task ends (`end`).
fn down(fx: &mut Fx, t: u32, a: u32, floor: u8, by: u8, end: bool) {
    let v = fx.mem.read_u8(a);
    if v >= floor {
        fx.mem.write_u8(a, v - by);
    } else if end {
        fx.mem.set16(t + 0x58, 0);
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E01830: the flare (frame 0x30): grey, size 25 growing by 5 to 50
/// from update 71, flickering white / light grey, fading out from 330.
fn flare(fx: &mut Fx, t: u32) {
    prologue(fx, 0x30);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x19);
        fx.mem.set16(t + 0xA, 0x19);
        fx.mem.set16(t + 0x56, 0);
        fx.mem.write_u32(t + 0x34, 0);
        l2::rgb(fx, t + 0x60, 0x60, 0x60, 0x60);
        l2::rgb(fx, t + 0x64, 0x60, 0x60, 0x60);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let v = rmod(fx, t, 0x01E0_1898, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_18B4, 0xB4);
        fx.mem.set16(t + 0x5C, v - 0x5A);
    }
    let s = fx.mem.i16(t + 0x56);
    fx.mem.set16(t + 0x56, if s != 0 { 0 } else { 1 });
    if fx.mem.i16(COUNTER) < 0x14A {
        up(fx, t + 0x63, 0x80, 0xA);
        up(fx, t + 0x67, 0x80, 0xA);
    } else {
        down(fx, t, t + 0x63, 0x10, 0xE, true);
        down(fx, t, t + 0x63, 0x10, 0xE, false);
    }
    let c = if fx.mem.i16(t + 0x54) % 2 != 0 { 0xAF } else { 0xFF };
    l2::rgb(fx, t + 0x60, c, c, c);
    if fx.mem.i16(COUNTER) >= 0x47 {
        for o in [8u32, 0xA] {
            let w = fx.mem.i16(t + o);
            if w < 0x32 {
                fx.mem.set16(t + o, w + 5);
            }
        }
    }
    angles(fx, t, D, 0x01E0_1A04);
    age(fx, t);
    fx.leave(0x30);
}

/// The spin and ring launch 0x1E01A30 / 0x1E02480 share: angles +0x20..
/// rand % 0xFFFF, radius +0x50 5..49, +0x5A, size +0xE, row +8 0..19,
/// life +6 (+4 rand) from `life`. `sites`: the eight rands' `jal`s.
fn spin_launch(fx: &mut Fx, t: u32, sites: [u32; 8], e: (i32, i32), life: i32) {
    for (o, site) in [0x20u32, 0x24, 0x28].into_iter().zip(&sites[..3]) {
        let v = rmod(fx, t, *site, 0xFFFF);
        fx.mem.set32(t + o, v);
    }
    let v = rmod(fx, t, sites[3], 0x2D);
    fx.mem.set16(t + 0x50, v + 5);
    let v = rmod(fx, t, sites[4], 0x168);
    fx.mem.set16(t + 0x5A, v);
    let v = rmod(fx, t, sites[5], e.0);
    fx.mem.set16(t + 0xE, v + e.1);
    let v = rmod(fx, t, sites[6], 0x14);
    fx.mem.set16(t + 8, v);
    let v = fx.rand_from(t, sites[7] + 8);
    fx.mem.set16(t + 6, v % 4 + life);
    fx.mem.set16(t + 0x58, 2);
}

/// 0x1E01A30: a white ring of sparks (frame 0x20): size 2..4 (4..6 from
/// update 100), full alpha 0x80 for its life, then out by 15.
fn sparks(fx: &mut Fx, t: u32) {
    prologue(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        let (site, base) = if fx.mem.i16(COUNTER) < 0x64 { (0x01E0_1A5C, 2) } else { (0x01E0_1A90, 4) };
        let v = rmod(fx, t, site, 3);
        fx.mem.setf(t + 0x48, ee::from_i32(v + base));
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        spin_launch(fx, t, [0x01E0_1ADC, 0x01E0_1AF8, 0x01E0_1B14, 0x01E0_1B30, 0x01E0_1B50, 0x01E0_1B6C, 0x01E0_1B8C, 0x01E0_1BA8], (0x3C, 0x28), 5);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        down(fx, t, t + 0x63, 0x11, 0xF, true);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E01C40: a grey arc ring (frame 0x20): inner radius 0, outer 1..20
/// (2..41 from update 70), height 2, spinning by 0x4000 a update.
fn arc(fx: &mut Fx, t: u32) {
    prologue(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        let (site, m, base) = if fx.mem.i16(COUNTER) < 0x46 { (0x01E0_1C70, 0x14, 1.0) } else { (0x01E0_1CA4, 0x28, 2.0) };
        let v = rmod(fx, t, site, m);
        fx.mem.setf(t + 0x4C, ee::add(base, ee::from_i32(v)));
        fx.mem.write_u32(t + 0x34, 0x4000_0000);
        fx.mem.set16(t + 6, 3);
        l2::rgb(fx, t + 0x64, 0x40, 0x40, 0x40);
        l2::rgb(fx, t + 0x60, 0x40, 0x10, 0x10);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        up(fx, t + 0x67, 0xC0, 0x60);
    } else {
        down(fx, t, t + 0x67, 0xD, 0xB, true);
    }
    angles(fx, t, 0, 0x01E0_1D78);
    spin(fx, t);
    age(fx, t);
    fx.leave(0x20);
}

fn spin(fx: &mut Fx, t: u32) {
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
}

/// 0x1E01DB0: the red arc ring (frame 0x30): inner radius 40 growing by
/// 3 to 75 from update 71, the outer colour flickering.
fn red_arc(fx: &mut Fx, t: u32) {
    prologue(fx, 0x30);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4220_0000);
        l2::rgb(fx, t + 0x60, 0xCF, 0xCF, 0xCF);
        l2::rgb(fx, t + 0x64, 0x80, 8, 8);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let r = if fx.mem.i16(t + 0x54) % 2 == 1 { 0xFF } else { 0xC0 };
    l2::rgb(fx, t + 0x64, r, 8, 8);
    if fx.mem.i16(COUNTER) < 0x14A {
        up(fx, t + 0x63, 0xC0, 0x40);
    } else {
        down(fx, t, t + 0x63, 0x10, 0xE, true);
    }
    if fx.mem.i16(COUNTER) >= 0x47 {
        let r = fx.mem.f(t + 0x48);
        if r < 75.0 {
            fx.mem.setf(t + 0x48, ee::add(r, 3.0));
        }
    }
    angles(fx, t, D, 0x01E0_1F04);
    spin(fx, t);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E01F40: a pink quad (frame 0x20) of size 50, a random cell frame
/// 2..6, in by 0x40 for 3 updates, then out by 13.
fn pink(fx: &mut Fx, t: u32) {
    prologue(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x32);
        fx.mem.set16(t + 0xA, 0x32);
        let v = rmod(fx, t, 0x01E0_1F64, 5);
        fx.mem.set16(t + 0x56, v + 2);
        fx.mem.write_u32(t + 0x34, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 3 {
        up(fx, t + 0x63, 0x80, 0x40);
        up(fx, t + 0x67, 0x80, 0x40);
    } else {
        down(fx, t, t + 0x63, 0xF, 0xD, true);
        down(fx, t, t + 0x63, 0xF, 0xD, false);
    }
    angles(fx, t, 0, 0x01E0_2038);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02060: a burst quad (frame 0x30) of size 100 growing by 15, flying
/// out along its angle +0x50 at 1.5 a update.
fn burst(fx: &mut Fx, t: u32) {
    prologue(fx, 0x30);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x64);
        fx.mem.set16(t + 0xA, 0x64);
        let v = fx.rand_from(t, 0x01E0_2094);
        fx.mem.set16(t + 0x56, v % 8);
        let a = fx.mem.i16(t + 0x50);
        let c = l2::sin_deg(fx, SINES, a, 1);
        fx.mem.setf(t + 0x30, ee::mul(1.5, c));
        let a = fx.mem.i16(t + 0x50);
        let s = l2::sin_deg(fx, SINES, a, 0);
        fx.mem.setf(t + 0x34, ee::mul(1.5, s));
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0xFF, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.set16(t + 0x5C, 0);
    }
    if fx.mem.i16(t + 0x54) < 0xC {
        up(fx, t + 0x63, 0xC0, 0x40);
        up(fx, t + 0x67, 0xC0, 0x40);
    } else {
        down(fx, t, t + 0x63, 0x10, 0xE, true);
        down(fx, t, t + 0x63, 0x10, 0xE, false);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [8u32, 0xA] {
            let w = fx.mem.i16(t + o) + 0xF;
            fx.mem.set16(t + o, w);
        }
    }
    let x = ee::add(fx.mem.f(t + 0x10), fx.mem.f(t + 0x30));
    fx.mem.setf(t + 0x10, x);
    let y = ee::add(fx.mem.f(t + 0x14), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x14, y);
    angles(fx, t, D, 0x01E0_21E4);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02210: a wide white arc ring (frame 0x20): radii 20 / 220 growing
/// by 6 / 10, a random turn.
fn wide(fx: &mut Fx, t: u32) {
    prologue(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        fx.mem.write_u32(t + 0x4C, 0x435C_0000);
        fx.mem.write_u32(t + 0x34, 0x4000_0000);
        fx.mem.set16(t + 6, 5);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0x80, 0x80);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 1);
        for (o, site) in [(0x20u32, 0x01E0_2280u32), (0x24, 0x01E0_229C), (0x28, 0x01E0_22B8)] {
            let v = rmod(fx, t, site, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        up(fx, t + 0x67, 0xC0, 0x40);
    } else {
        down(fx, t, t + 0x67, 0x10, 0xE, true);
    }
    let r = ee::add(fx.mem.f(t + 0x48), 6.0);
    fx.mem.setf(t + 0x48, r);
    let r = ee::add(fx.mem.f(t + 0x4C), 10.0);
    fx.mem.setf(t + 0x4C, r);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02380: the white arc ring of the burst (frame 0x20): outer radius
/// 250, spinning.
fn white_arc(fx: &mut Fx, t: u32) {
    prologue(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        fx.mem.write_u32(t + 0x4C, 0x437A_0000);
        fx.mem.write_u32(t + 0x34, 0x4000_0000);
        fx.mem.set16(t + 6, 0xA);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        up(fx, t + 0x67, 0xC0, 0x60);
    } else {
        down(fx, t, t + 0x67, 0xD, 0xB, true);
    }
    angles(fx, t, 0, 0x01E0_244C);
    spin(fx, t);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02480: a ring of the burst (frame 0x20): quad size 10, its radius
/// +0x50 growing by +0xE (15..44), alpha 0xC0 for its life, then out by 9.
fn ring(fx: &mut Fx, t: u32) {
    prologue(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        spin_launch(fx, t, [0x01E0_24C0, 0x01E0_24DC, 0x01E0_24F8, 0x01E0_2514, 0x01E0_2534, 0x01E0_2550, 0x01E0_2570, 0x01E0_258C], (0x1E, 0xF), 7);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        down(fx, t, t + 0x63, 0xB, 9, true);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02640: the frame flash (frame 0x80): a window sprite (0x6FF8 ..
/// 0x8FF8) sampling the frame (0x207EF0) at ST +0x48 / 512 .. (512 -
/// +0x48) / 512, colour (0x80, 0x7F, 0x7F, alpha +0x63), ALPHA 0x44, key
/// 0; in by 0x13 below 0x60 for 10 updates while the zoom grows by 0.3 to
/// 100, then out by 6 while it shrinks to 0.
fn flash(fx: &mut Fx, t: u32) {
    flash_from(fx, t, TASK_RA);
}

/// [`flash`] called from return address `ra` with the caller's s0 = D
/// (also b144's 0x1E034D0, the same body).
pub(crate) fn flash_from(fx: &mut Fx, t: u32, ra: u64) {
    let sp = fx.enter(0x80);
    fx.saved(sp, &[(0x10, ra), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x3F80_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x28;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u8(sp + 0x38, 0x80);
    fx.mem.write_u8(sp + 0x39, 0x7F);
    fx.mem.write_u8(sp + 0x3A, 0x7F);
    let a = fx.mem.read_u8(t + 0x63);
    fx.mem.write_u8(sp + 0x3B, a);
    fx.mem.write_u32(sp + 0x3C, 0x3F80_0000);
    for (at, v) in [(sp + 0x48, 0x6FF8u16), (sp + 0x58, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x40, ee::div(s, 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x44, ee::div(s, 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x50, ee::div(ee::sub(512.0, s), 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x54, ee::div(ee::sub(512.0, s), 512.0));
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if fx.mem.i16(t + 0x54) < 0xA {
        up(fx, t + 0x63, 0x60, 0x13);
        let z = fx.mem.f(t + 0x48);
        if z < 100.0 {
            fx.mem.setf(t + 0x48, ee::add(z, f32::from_bits(0x3E99_999A)));
        }
    } else {
        down(fx, t, t + 0x63, 8, 6, true);
        let z = fx.mem.f(t + 0x48);
        if !(z <= 0.0) {
            fx.mem.setf(t + 0x48, ee::sub(z, f32::from_bits(0x3E99_999A)));
        } else {
            fx.mem.write_u32(t + 0x48, 0);
        }
    }
    age(fx, t);
    fx.leave(0x80);
}
