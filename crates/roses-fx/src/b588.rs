//! BMODULE b588 (card 669), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02840, data area 0x1ED0000) [F]: a task pool of 320 (0x70 bytes at +0x380, alive +0x58, step
//! +0x44 as a record address) drawn with the second library: quads
//! (`billboard::quad`, 0x1E004D0, UVs by uv_rect4 0x1E00080), arc strips
//! (`billboard::arc_strips`, 0x1E00790) and four-quad sprites
//! (`quads4::sprite_quads`, 0x1E00AF0). From update 40 to 90 two white
//! sparks a update fly out of the attacker's node 1; from 90 to 125 two
//! violet motes a update rise from node 0x11, on update 129 twenty more;
//! from 131 to 170 two blue and one green ember a update drift from node
//! 0x11; on updates 205 and 220 (nodes 0x58 / 0x5F) two light planes, a
//! green arc ring and forty sparks burst out. Returns 1 on update 140, 4
//! on 260, 2 on 240, 6 on 146..234, 8 once the 270-update timer +0xA900
//! has run out.
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 320;
/// The step functions a task holds (+0x44), as the record's addresses.
const MOTE: u32 = 0x01E0_1300;
const GREEN: u32 = 0x01E0_15B0;
const SPARK: u32 = 0x01E0_1920;
const BURST_G: u32 = 0x01E0_1B10;
const BURST_V: u32 = 0x01E0_1D00;
const BLUE: u32 = 0x01E0_1EF0;
const RISE: u32 = 0x01E0_2270;
const ARC: u32 = 0x01E0_2500;
const PLANE: u32 = 0x01E0_25F0;
/// Counted, not run, by the entry's step loop (never spawned) [F].
const IDLE: u32 = 0x01E0_2780;
/// The return address of the entry's `jalr` into a step (0x1E02AD0 + 8).
const TASK_RA: u64 = 0x01E0_2AD8;
const POOL: u32 = D + 0x380;
const TIMER: u32 = D + 0xA900;
const COLOUR: u32 = D + 0xA904;
const FACING: u32 = D + 0xA96A;
const COUNTER: u32 = D + 0xA96C;
const SINES: u32 = D + 0xAB70;
/// The spawn point.
const AT: u32 = D + 0x2D0;

/// The entry 0x1E02840.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xD0);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0xA90C, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xD0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xACE0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xA968, param);
    fx.mem.set32(TIMER, 0x10E);
    for o in [0x310u32, 0x314, 0x318] {
        fx.mem.write_u32(D + o, 0);
    }
    for (at, node, clear) in [(0xA94Cu32, 0x11, 1), (0xA94E, 0x58, 0), (0xA950, 0x5F, 0), (0xA952, 1, 0), (0xA954, 0x4A, 0), (0xA956, 0x50, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(D + at, r);
    }
    // 0x1E02790(0, D): the textures (frame 0x40: the entry's s2 = 0, s1 =
    // 0, s0 = D and the return address).
    {
        let f = fx.enter(0x40);
        fx.saved(f, &[(0x30, 0x01E0_299C), (0x20, 0), (0x10, 0), (0, u64::from(D))]);
        for k in 0..3u32 {
            let t = fx.texture(4, k as i32);
            fx.mem.write_u32(D + 0x340 + 4 * k, t);
        }
        for k in 0..2u32 {
            let t = fx.texture(4, 0xE + k as i32);
            fx.mem.write_u32(D + 0x378 + 4 * k, t);
        }
        for k in 0..3u32 {
            let t = fx.texture(5, k as i32);
            fx.mem.write_u32(D + 0xA8E0 + 4 * k, t);
        }
        fx.leave(0x40);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xC0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xC4)), 180.0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for table in [D + 0xA970, D + 0xAA70] {
        for k in 0..0x100u32 {
            let v = fx.rand();
            let (_, h) = fx.div(v, 0x1E);
            fx.mem.set8(table + k, h - 0xF);
        }
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand();
        let (_, h) = fx.div(v, 0x14);
        fx.mem.set8(D + 0xAA70 + k, h - 0xA);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = POOL + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 || f == IDLE {
            continue;
        }
        match f {
            MOTE => mote(fx, t),
            GREEN => ember(fx, t, false),
            SPARK => flyer(fx, t, Flyer::Spark),
            BURST_G => flyer(fx, t, Flyer::Green),
            BURST_V => flyer(fx, t, Flyer::Violet),
            BLUE => ember(fx, t, true),
            RISE => rise(fx, t),
            ARC => arc(fx, t),
            PLANE => plane(fx, t),
            _ => panic!("b588: unknown task step {f:#x}"),
        }
    }
    for k in 0..TASKS {
        let t = POOL + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == MOTE {
            texture(fx, 0xA8E4);
            cell(fx, t, sp + 0x60, 0x9B);
            orient(fx, t);
            sub_uv(fx, t);
            billboard::quad(fx, uv::uv_rect4, D, t, 0x20, t + 0x10);
        }
        if fx.mem.read_u32(t + 0x44) == GREEN {
            texture(fx, 0xA8E4);
            orient(fx, t);
            sub_uv(fx, t);
            orient(fx, t);
            billboard::quad(fx, uv::uv_rect4, D, t, 0x20, t + 0x10);
        }
        for (f, tex, local, mode) in [(SPARK, 0xA8E8u32, 0x70u32, 0x10), (BURST_G, 0xA8E8, 0x80, 0x11)] {
            if fx.mem.read_u32(t + 0x44) == f {
                texture(fx, tex);
                cell(fx, t, sp + local, 0x14);
                orient(fx, t);
                billboard::quad(fx, uv::uv_rect4, D, t, mode, t + 0x10);
            }
        }
        if fx.mem.read_u32(t + 0x44) == BURST_V {
            texture(fx, 0xA8E4);
            orient(fx, t);
            sub_uv(fx, t);
            orient(fx, t);
            billboard::quad(fx, uv::uv_rect4, D, t, 0x20, t + 0x10);
        }
        if fx.mem.read_u32(t + 0x44) == BLUE {
            texture(fx, 0xA8E0);
            cell(fx, t, sp + 0x90, 0x14);
            orient(fx, t);
            billboard::quad(fx, uv::uv_rect4, D, t, 0x20, t + 0x10);
        }
        if fx.mem.read_u32(t + 0x44) == RISE {
            texture(fx, 0xA8E4);
            cell(fx, t, sp + 0xA0, 0x9B);
            orient(fx, t);
            sub_uv(fx, t);
            billboard::quad(fx, uv::uv_rect4, D, t, 0x20, t + 0x10);
        }
        if fx.mem.read_u32(t + 0x44) == ARC {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == PLANE {
            texture(fx, 0x37C);
            l2::rect(fx, sp + 0xB0, 0x9A, 0);
            copy4(fx, sp + 0xB0, D + 0x94);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sprite_quads(fx, D, t, 0, t + 0x10);
            quads4::sprite_quads(fx, D, t, 0, t + 0x10);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x8C => 1,
        0x104 => 4,
        0xF0 => 2,
        0x92..=0xEA => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

fn copy4(fx: &mut Fx, from: u32, to: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn texture(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(D + at);
    fx.mem.write_u32(D + 0x88, w);
}

/// The atlas cell `id` / +0x56 (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32, id: i32) {
    let sub = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, id, sub);
    copy4(fx, r, D + 0x94);
}

/// D+0xBE = the task's orientation +0x52.
fn orient(fx: &mut Fx, t: u32) {
    let o = fx.mem.i16(t + 0x52);
    fx.mem.set16(D + 0xBE, o);
}

/// The 64-pixel cell of +0x56 (frames 2.. one row on), size 0x40.
fn sub_uv(fx: &mut Fx, t: u32) {
    let sub = fx.mem.i16(t + 0x56);
    let (u, v) = if sub < 2 { (sub << 6, 0) } else { ((sub - 2) << 6, 0x40) };
    fx.mem.set16(D + 0x94, u);
    fx.mem.set16(D + 0x96, v);
    fx.mem.set16(D + 0x98, 0x40);
    fx.mem.set16(D + 0x9A, 0x40);
}

/// 0x1E00E60(D, 0): the colour, the update counter and the spawns. Frame
/// 0x30 (the entry's s1 = 0, s0 = D); 0x1E030A0's saves: the entry's s4 =
/// 0, s3 = the parameter, s2 = 0, the emitter's s1, s0 = D.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_2A9C), (0x10, 0), (0, u64::from(D))]);
    let p = param as i64 as u64;
    let go = |fx: &mut Fx, f: u32, ra: u32, s1: u64| {
        spawn(fx, f, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, p), (0x20, 0), (0x10, s1), (0, u64::from(D))]);
    };
    let node = |fx: &mut Fx, slot: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i16(D + slot);
        fx.node_position(h, s, AT);
    };
    for (k, v) in [0x80u8, 0, 0, 0].into_iter().enumerate() {
        fx.mem.write_u8(COLOUR + k as u32, v);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    for (at, slot, a, b, ras) in [
        (0xCD, 0xA94Eu32, 0x2D, 0x87, [0x01E0_0F2Cu32, 0x01E0_0F60, 0x01E0_0F74, 0x01E0_0F8C, 0x01E0_0FB4]),
        (0xDC, 0xA950, 0x87, 0x2D, [0x01E0_1030, 0x01E0_1064, 0x01E0_1078, 0x01E0_1090, 0x01E0_10B8]),
    ] {
        if fx.mem.i16(COUNTER) != at {
            continue;
        }
        node(fx, slot);
        fx.mem.set16(D + 0xA95E, a);
        fx.mem.set16(D + 0xA966, 0);
        go(fx, PLANE, ras[0], 0);
        fx.mem.set16(D + 0xA95E, b);
        fx.mem.set16(D + 0xA966, 1);
        go(fx, PLANE, ras[1], 0);
        go(fx, ARC, ras[2], 0);
        for k in 0..0x14u64 {
            go(fx, BURST_G, ras[3], k);
        }
        for k in 0..0x14u64 {
            go(fx, BURST_V, ras[4], k);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x83..0xAB).contains(&c) {
        node(fx, 0xA94C);
        go(fx, BLUE, 0x01E0_1120, 0);
        go(fx, BLUE, 0x01E0_1134, 0);
        go(fx, GREEN, 0x01E0_1148, 0);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x5A..0x7E).contains(&c) {
        node(fx, 0xA94C);
        go(fx, RISE, 0x01E0_11A0, 0);
        go(fx, RISE, 0x01E0_11B4, 0);
    }
    if fx.mem.i16(COUNTER) == 0x81 {
        node(fx, 0xA94C);
        for k in 0..0x14u64 {
            go(fx, MOTE, 0x01E0_1204, k);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x28..0x5B).contains(&c) {
        node(fx, 0xA952);
        let y = ee::sub(fx.mem.f(AT + 4), 7.0);
        fx.mem.setf(AT + 4, y);
        let z = fx.mem.f(AT + 8);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 5.0) } else { ee::add(z, 5.0) };
        fx.mem.setf(AT + 8, z);
        go(fx, SPARK, 0x01E0_12CC, 0);
        go(fx, SPARK, 0x01E0_12E0, 0);
    }
    fx.leave(0x30);
}

/// 0x1E030A0(D, step, 0): the first free task cleared and set off at
/// D+0x2D0 with the colour +0xA904 twice, kind +0xA966 (+0x50), angle
/// +0xA95E (+0x5A). `saves`: its prologue's.
fn spawn(fx: &mut Fx, f: u32, saves: &[(u32, u64)]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(POOL + 0x70 * k + 0x58) == 0) {
        Some(k) => {
            let t = POOL + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for c in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, AT, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(AT + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let a = fx.mem.i16(D + 0xA966);
            fx.mem.set16(t + 0x50, a);
            let a = fx.mem.i16(D + 0xA95E);
            fx.mem.set16(t + 0x5A, a);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// A step's prologue: frame 0x20 (ra, the entry's s0 = D) or 0x30 (ra,
/// the entry's s1 = 0, s0 = D).
fn prologue(fx: &mut Fx, bytes: u32) {
    let sp = fx.enter(bytes);
    if bytes == 0x20 {
        fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
    } else {
        fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
    }
}

/// rand from a step's `jal` at `site` (its s0 = the task).
fn r(fx: &mut Fx, t: u32, site: u32) -> i32 {
    fx.rand_from(t, site + 8)
}

/// rand % m (HI).
fn rmod(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = r(fx, t, site);
    fx.div(v, m).1
}

/// (rand >> 4) % m (HI).
fn rmod4(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = r(fx, t, site);
    fx.div(v >> 4, m).1
}

/// 0x205580(t + 0x20) from a step's `jal` at `site` (its frame 0x30: the
/// step's s1 and s0 = t).
fn angles(fx: &mut Fx, t: u32, s1: u32, site: u32) {
    let base = fx.sp.wrapping_sub(0x30);
    fx.saved(base, &[(0x20, u64::from(site + 8)), (0x10, u64::from(s1)), (0, u64::from(t))]);
    l2::camera_angles_to(fx, t + 0x20);
}

/// The drift +0x30 / +0x34 (/ `d`) and +0x38 (/ 60 when `z`) as rand % 100
/// - 50, the spin +0x5C and the camera's angles, the spins +4 / +2 / +0
/// (rand % 0x800 - 0x400). `sites`: the rands' and 0x205580's `jal`s.
fn launch(fx: &mut Fx, t: u32, s1: u32, sites: &[u32], d: f32, z: bool) {
    let mut s = sites.iter().copied();
    let v = rmod(fx, t, s.next().unwrap(), 0x64);
    fx.mem.setf(t + 0x34, ee::from_i32(v - 0x32));
    let v = rmod(fx, t, s.next().unwrap(), 0x64);
    fx.mem.setf(t + 0x30, ee::from_i32(v - 0x32));
    if z {
        let v = rmod(fx, t, s.next().unwrap(), 0x64);
        fx.mem.setf(t + 0x38, ee::from_i32(v - 0x32));
    }
    for o in [0x34u32, 0x30] {
        let q = ee::div(fx.mem.f(t + o), d);
        fx.mem.setf(t + o, q);
    }
    if z {
        let q = ee::div(fx.mem.f(t + 0x38), d);
        fx.mem.setf(t + 0x38, q);
    }
    let v = rmod(fx, t, s.next().unwrap(), 0x168);
    fx.mem.set16(t + 0x5C, v);
    angles(fx, t, s1, s.next().unwrap());
    for o in [4u32, 2, 0] {
        let v = r(fx, t, s.next().unwrap());
        fx.mem.set16(t + o, v % 0x800 - 0x400);
    }
}

fn spins(fx: &mut Fx, t: u32) {
    for (a, s) in [(0x28u32, 4u32), (0x24, 2), (0x20, 0)] {
        let v = fx.mem.read_u32(t + a).wrapping_add(fx.mem.i16(t + s) as u32);
        fx.mem.write_u32(t + a, v);
    }
}

/// Alpha +0x63 (or `a`): + `up` below `cap` until age `until`, then -
/// `down` from `floor` up, else the task ends.
fn fade(fx: &mut Fx, t: u32, a: u32, until: i32, cap: u8, up: u8, floor: u8, down: u8) {
    let v = fx.mem.read_u8(t + a);
    if fx.mem.i16(t + 0x54) < until {
        if v < cap {
            fx.mem.write_u8(t + a, v + up);
        }
    } else if v >= floor {
        fx.mem.write_u8(t + a, v - down);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E01300: a violet mote (frame 0x20) of size 50 growing by 1,
/// drifting +-50 / 60 each way, spinning.
fn mote(fx: &mut Fx, t: u32) {
    prologue(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x32);
        fx.mem.set16(t + 0xA, 0x32);
        let v = rmod4(fx, t, 0x01E0_1328, 3);
        fx.mem.set16(t + 0x56, v + 2);
        l2::rgb(fx, t + 0x60, 0x80, 0x40, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        launch(fx, t, 0, &[0x01E0_1360, 0x01E0_138C, 0x01E0_13B8, 0x01E0_1410, 0x01E0_142C, 0x01E0_1434, 0x01E0_1454, 0x01E0_1474], 60.0, true);
        let v = r(fx, t, 0x01E0_1494);
        fx.mem.set16(t + 0x52, (v >> 4) % 4);
    }
    fade(fx, t, 0x63, 8, 0x80, 0x40, 6, 4);
    if fx.mem.i16(t + 0x54) != 0 {
        for (p, v) in [(0x10u32, 0x30u32), (0x14, 0x34), (0x18, 0x38)] {
            let x = ee::add(fx.mem.f(t + p), fx.mem.f(t + v));
            fx.mem.setf(t + p, x);
        }
        let w = fx.mem.i16(t + 8) + 1;
        fx.mem.set16(t + 8, w);
        fx.mem.set16(t + 0xA, w);
    }
    spins(fx, t);
    angles(fx, t, 0, 0x01E0_1584);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E015B0 (green, size 50..89, grows to 130, 3 a update toward the
/// facing, fades after 40) / 0x1E01EF0 (blue, size 50..79, grows to 150,
/// 2.7 a update, fades after 50), frame 0x30: an ember drifting +-50 /
/// 400, falling by 0.09, spinning; random turn +0x20.. (rand % 0xFFFF).
fn ember(fx: &mut Fx, t: u32, blue: bool) {
    prologue(fx, 0x30);
    let sites: [u32; 13] = if blue {
        [0x01E0_1F14, 0x01E0_1F40, 0x01E0_1F74, 0x01E0_1FA0, 0x01E0_1FEC, 0x01E0_2008, 0x01E0_2010, 0x01E0_2030, 0x01E0_2050, 0x01E0_2070, 0x01E0_208C, 0x01E0_20A8, 0x01E0_20C4]
    } else {
        [0x01E0_15D4, 0x01E0_1600, 0x01E0_1634, 0x01E0_1660, 0x01E0_16AC, 0x01E0_16C8, 0x01E0_16D0, 0x01E0_16F0, 0x01E0_1710, 0x01E0_1730, 0x01E0_174C, 0x01E0_1768, 0x01E0_1784]
    };
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, sites[0], if blue { 0x1E } else { 0x28 });
        fx.mem.set16(t + 8, v + 0x32);
        let h = fx.mem.i16(t + 8) + 4;
        fx.mem.set16(t + 0xA, h);
        let v = rmod4(fx, t, sites[1], 3);
        fx.mem.set16(t + 0x56, v);
        if blue {
            l2::rgb(fx, t + 0x60, 0x10, 0x10, 0xFF);
        } else {
            l2::rgb(fx, t + 0x60, 0x40, 0xFF, 0x40);
        }
        fx.mem.set16(t + 0x58, 2);
        launch(fx, t, D, &sites[2..9], 400.0, false);
        for (o, site) in [(0x20u32, sites[9]), (0x24, sites[10]), (0x28, sites[11])] {
            let v = rmod(fx, t, site, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        let v = r(fx, t, sites[12]);
        fx.mem.set16(t + 0x52, v % 4);
    }
    fade(fx, t, 0x63, if blue { 0x32 } else { 0x28 }, 0x80, 0x40, 0xB, 9);
    let age_now = fx.mem.i16(t + 0x54);
    if fx.div(age_now, 3).1 == 1 {
        let w = fx.mem.i16(t + 8);
        if w < if blue { 0x96 } else { 0x82 } {
            fx.mem.set16(t + 8, w + 5);
        }
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let step = if blue { f32::from_bits(0x402C_CCCD) } else { 3.0 };
        let z = fx.mem.f(t + 0x18);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, step) } else { ee::add(z, step) };
        fx.mem.setf(t + 0x18, z);
        let x = ee::add(fx.mem.f(t + 0x10), fx.mem.f(t + 0x30));
        fx.mem.setf(t + 0x10, x);
        let dy = ee::sub(fx.mem.f(t + 0x34), f32::from_bits(0x3DB8_51EC));
        let y = ee::add(fx.mem.f(t + 0x14), dy);
        fx.mem.setf(t + 0x14, y);
    }
    angles(fx, t, D, if blue { 0x01E0_2208 } else { 0x01E0_18C0 });
    spins(fx, t);
    age(fx, t);
    fx.leave(0x30);
}

#[derive(Clone, Copy, PartialEq)]
enum Flyer {
    /// 0x1E01920: white, size 80, thrown 8.6 along its angle, then 0.6 a
    /// update in x / z.
    Spark,
    /// 0x1E01B10: green, size 130, 3.6 then 1.6 a update in x / y.
    Green,
    /// 0x1E01D00: violet, size 130, 3.6 then 1.8 a update in x / y.
    Violet,
}

/// The burst sparks (frame 0x30): a random cell frame 2..4, an angle
/// +0x5A ((rand >> 4) % 360), in by 0x60 for 6 updates, then out by 19.
fn flyer(fx: &mut Fx, t: u32, kind: Flyer) {
    prologue(fx, 0x30);
    let (sites, throw, speed, angle_ra): ([u32; 3], u32, u32, u32) = match kind {
        Flyer::Spark => ([0x01E0_1950, 0x01E0_1988, 0x01E0_1A00], 0x4109_999A, 0x3F19_999A, 0x01E0_1AE0),
        Flyer::Green => ([0x01E0_1B40, 0x01E0_1B78, 0x01E0_1BF0], 0x4066_6666, 0x3FCC_CCCD, 0x01E0_1CD0),
        Flyer::Violet => ([0x01E0_1D30, 0x01E0_1D68, 0x01E0_1DE0], 0x4066_6666, 0x3FE6_6666, 0x01E0_1EC0),
    };
    let (throw, speed) = (f32::from_bits(throw), f32::from_bits(speed));
    if fx.mem.i16(t + 0x58) == 1 {
        let size = if kind == Flyer::Spark { 0x50 } else { 0x82 };
        fx.mem.set16(t + 8, size);
        fx.mem.set16(t + 0xA, size);
        let v = rmod4(fx, t, sites[0], 3);
        fx.mem.set16(t + 0x56, v + 2);
        match kind {
            Flyer::Spark => l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF),
            Flyer::Green => l2::rgb(fx, t + 0x60, 0x40, 0xFF, 0x40),
            Flyer::Violet => l2::rgb(fx, t + 0x60, 0x80, 0x10, 0xFF),
        }
        fx.mem.set16(t + 0x58, 2);
        let v = rmod4(fx, t, sites[1], 0x168);
        fx.mem.set16(t + 0x5A, v);
        let a = fx.mem.i16(t + 0x5A);
        let s = l2::sin_deg(fx, SINES, a, 0);
        let z = ee::add(fx.mem.f(t + 0x18), ee::mul(throw, s));
        fx.mem.setf(t + 0x18, z);
        let a = fx.mem.i16(t + 0x5A);
        let c = l2::sin_deg(fx, SINES, a, 1);
        let x = ee::add(fx.mem.f(t + 0x10), ee::mul(throw, c));
        fx.mem.setf(t + 0x10, x);
        let v = r(fx, t, sites[2]);
        fx.mem.set16(t + 0x52, (v >> 4) % 4);
    }
    fade(fx, t, 0x63, 6, 0xC0, 0x60, 0x15, 0x13);
    if fx.mem.i16(t + 0x54) != 0 {
        let o = if kind == Flyer::Spark { 0x18 } else { 0x14 };
        let a = fx.mem.i16(t + 0x5A);
        let s = l2::sin_deg(fx, SINES, a, 0);
        let v = ee::add(fx.mem.f(t + o), ee::mul(speed, s));
        fx.mem.setf(t + o, v);
        let a = fx.mem.i16(t + 0x5A);
        let c = l2::sin_deg(fx, SINES, a, 1);
        let x = ee::add(fx.mem.f(t + 0x10), ee::mul(speed, c));
        fx.mem.setf(t + 0x10, x);
    }
    angles(fx, t, D, angle_ra);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02270: a rising violet mote (frame 0x20) of 50 x 25, drifting +-50
/// / 60 each way and up by 1 a update, spinning.
fn rise(fx: &mut Fx, t: u32) {
    prologue(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x32);
        let h = fx.mem.i16(t + 8) >> 1;
        fx.mem.set16(t + 0xA, h);
        let v = rmod4(fx, t, 0x01E0_229C, 3);
        fx.mem.set16(t + 0x56, v + 2);
        l2::rgb(fx, t + 0x60, 0x80, 0x40, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        launch(fx, t, 0, &[0x01E0_22D4, 0x01E0_2300, 0x01E0_232C, 0x01E0_2384, 0x01E0_23A0, 0x01E0_23A8, 0x01E0_23C8, 0x01E0_23E8], 60.0, true);
        let v = r(fx, t, 0x01E0_2408);
        fx.mem.set16(t + 0x52, (v >> 4) % 4);
    }
    fade(fx, t, 0x63, 8, 0x80, 0x40, 6, 4);
    if fx.mem.i16(t + 0x54) != 0 {
        let y = ee::sub(fx.mem.f(t + 0x14), 1.0);
        fx.mem.setf(t + 0x14, y);
    }
    spins(fx, t);
    angles(fx, t, 0, 0x01E0_24CC);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02500: the green arc ring (frame 0x20): outer radius 140, its
/// outer alpha in by 0x60 for 14 updates, then out by 9, spinning.
fn arc(fx: &mut Fx, t: u32) {
    prologue(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x430C_0000);
        l2::rgb(fx, t + 0x64, 0xC0, 0xFF, 0xC0);
        l2::rgb(fx, t + 0x60, 0x40, 0xFF, 0x40);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    fade(fx, t, 0x67, 0xE, 0xC0, 0x60, 0xB, 9);
    angles(fx, t, 0, 0x01E0_25B8);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E025F0: a light plane (frame 0x20): kind +0x50 0: 18 x 1000 growing
/// by 40, 1: 18 x 250 growing by 20, 2: 18 x 400; in by 0x40 for 4
/// updates, then out by 9.
fn plane(fx: &mut Fx, t: u32) {
    prologue(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0x80, 0xFF, 0x80);
        l2::rgb(fx, t + 0x64, 0x10, 0xFF, 0x10);
        for (kind, h) in [(0, 0x447A_0000u32), (1, 0x437A_0000), (2, 0x43C8_0000)] {
            if fx.mem.i16(t + 0x50) == kind {
                fx.mem.write_u32(t + 0x30, 0x4190_0000);
                fx.mem.write_u32(t + 0x34, h);
            }
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    fade(fx, t, 0x63, 4, 0x80, 0x40, 0xB, 9);
    for (kind, dh) in [(0, 40.0f32), (1, 20.0)] {
        if fx.mem.i16(t + 0x50) == kind && fx.mem.i16(t + 0x54) != 0 {
            let h = ee::add(fx.mem.f(t + 0x34), dh);
            fx.mem.setf(t + 0x34, h);
        }
    }
    age(fx, t);
    fx.leave(0x20);
}
