//! BMODULE b544 (card 126), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02B00, data area 0x1ED0000) [F]. b588's family: a pool of 320 tasks of 0x70 bytes at D+0x380
//! (alive +0x58, step +0x44 called through `jalr`; spawned by 0x1E031C0,
//! b588's spawn word for word), drawn with the second library (quads
//! `billboard::quad` with uv_rect4, `billboard::arc_strips`,
//! `quads4::sprite_quads`), the record's own gouraud ribbons between two
//! point histories (0x1E03360, spline::spline) and its own light column
//! (0x1E038B0). Update 1: a frame flash; 1..74: red and dark sparks at the
//! attacker's nodes 0xF / 0x1B; 85: a trail at nodes 0x2C / 0x2D (eight
//! 20-point histories D+0x8FD0, three ribbons); 89 at node 0x2D: an arc,
//! forty rising columns and four light planes. Returns 1 on update 81, 4
//! on 145, 2 on 125, 6 on 92..119, 8 once the 150-update timer +0xA900 has
//! run out.
use crate::b076::{node_position_saving, request_node_saving};
use crate::b494::camera_angles_saving;
use crate::blib::exe::{camera_vp_into, mul_mem, rot_trans_into};
use crate::blib::{billboard, quads4, spawn as lspawn, spline, uv};
use crate::bus::Bus;
use crate::c061::rgbaq_at;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 320;
const POOL: u32 = D + 0x380;
const AT: u32 = D + 0x2D0;
const VP: u32 = D + 0x290;
const HIST: u32 = D + 0x8FD0;
const TIMER: u32 = D + 0xA900;
const COLOUR: u32 = D + 0xA904;
const OTW: u32 = D + 0xA90C;
const MEAN: u32 = D + 0xA93C;
const NODES: u32 = D + 0xA94C;
const ANGLE: u32 = D + 0xA95E;
const KIND: u32 = D + 0xA966;
const FACING: u32 = D + 0xA96A;
const COUNTER: u32 = D + 0xA96C;
const SINES: u32 = D + 0xAB70;

const SPARK: u32 = 0x01E0_16F0;
const DARK: u32 = 0x01E0_1950;
const ARC: u32 = 0x01E0_1BB0;
const BEAM: u32 = 0x01E0_1CA0;
const PLANE: u32 = 0x01E0_1DE0;
const TRAIL: u32 = 0x01E0_1F70;
const FLASH: u32 = 0x01E0_2870;
/// Counted, not run, by the entry's step loop (never spawned) [F].
const IDLE: u32 = 0x01E0_2A40;
/// The return address of the entry's `jalr` into a step.
const TASK_RA: u64 = 0x01E0_2DA0;

pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x150);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(OTW, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x150);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let p = param as i64 as u64;
    fx.bzero(D, 0xACE0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xA968, param);
    fx.mem.set32(TIMER, 0x96);
    for o in [0x310u32, 0x314, 0x318] {
        fx.mem.write_u32(D + o, 0);
    }
    for (k, (node, clear, ra)) in [(0x2C, 1, 0x01E0_2BACu32), (0x2D, 0, 0x01E0_2BCC), (0xF, 0, 0x01E0_2BEC), (0x1B, 0, 0x01E0_2C0C), (0x4A, 0, 0x01E0_2C2C), (0x50, 0, 0x01E0_2C4C)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = request_node_saving(fx, h, node, clear, [Some(0), Some(p), Some(u64::from(D))], ra);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    // 0x1E02A50(0, D): the textures (frame 0x40: the entry's s2 = D, s1 =
    // the parameter, s0 = 0 and the return address).
    {
        let f = fx.enter(0x40);
        fx.saved(f, &[(0x30, 0x01E0_2C64), (0x20, u64::from(D)), (0x10, p), (0, 0)]);
        for k in 0..3u32 {
            let t = fx.texture(4, k as i32);
            fx.mem.write_u32(D + 0x340 + 4 * k, t);
        }
        for k in 0..2u32 {
            let t = fx.texture(4, 0xE + k as i32);
            fx.mem.write_u32(D + 0x378 + 4 * k, t);
        }
        for k in 0..2u32 {
            let t = fx.texture(5, k as i32);
            fx.mem.write_u32(D + 0xA8E0 + 4 * k, t);
        }
        fx.leave(0x40);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.fix_residue(y, 0x01E0_2CA0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for (table, ra) in [(D + 0xA970, 0x01E0_2CB4u32), (D + 0xAA70, 0x01E0_2CEC)] {
        for k in 0..0x100u32 {
            let v = fx.rand_from(k, ra);
            let (_, h) = fx.div(v, 0x1E);
            fx.mem.set8(table + k, h - 0xF);
        }
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(k, 0x01E0_2D24);
        let (_, h) = fx.div(v, 0x14);
        fx.mem.set8(D + 0xAA70 + k, h - 0xA);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    let mut idle = 0i32;
    for k in 0..TASKS {
        let t = POOL + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 {
            continue;
        }
        if f == IDLE {
            idle = (idle + 1) as i16 as i32;
            continue;
        }
        // The entry's s0 = the idle count, s1 = D + 0x70 k, s2 = D, s3 = 0.
        let (s0, s1) = (idle as i64 as u64, u64::from(D + 0x70 * k));
        let frame = match f {
            TRAIL => 0x60,
            FLASH => 0x90,
            _ => 0x20,
        };
        if f == FLASH {
            flash(fx, t, s0, s1);
            continue;
        }
        let s = fx.enter(frame);
        if frame == 0x20 {
            fx.saved(s, &[(0x10, TASK_RA), (0, s0)]);
        } else {
            fx.saved(s, &[(0x40, TASK_RA), (0x30, 0), (0x20, u64::from(D)), (0x10, s1), (0, s0)]);
        }
        match f {
            SPARK => spark(fx, t, s1, false),
            DARK => spark(fx, t, s1, true),
            ARC => arc(fx, t, s1),
            BEAM => beam(fx, t),
            PLANE => plane(fx, t),
            TRAIL => trail(fx, t, s),
            _ => panic!("b544: unknown task step {f:#x}"),
        }
        fx.leave(frame);
    }
    let mut count = 0u32;
    for k in 0..TASKS {
        let t = POOL + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        count += 1;
        let f = fx.mem.read_u32(t + 0x44);
        if f == SPARK {
            texture(fx, 0xA8E0);
            cell(fx, t, sp + 0x80, 0x9B);
            orient(fx, t);
            let sub = fx.mem.i16(t + 0x56);
            let (u, v) = if sub < 2 { (sub << 6, 0) } else { ((sub - 2) << 6, 0x40) };
            fx.mem.set16(D + 0x94, u);
            fx.mem.set16(D + 0x96, v);
            fx.mem.set16(D + 0x98, 0x40);
            fx.mem.set16(D + 0x9A, 0x40);
            billboard::quad(fx, uv::uv_rect4, D, t, 0x20, t + 0x10);
        }
        if fx.mem.read_u32(t + 0x44) == DARK {
            texture(fx, 0xA8E4);
            cell(fx, t, sp + 0x90, 0x14);
            orient(fx, t);
            billboard::quad(fx, uv::uv_rect4, D, t, 0x20, t + 0x10);
        }
        if fx.mem.read_u32(t + 0x44) == ARC {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == BEAM {
            let saves = |fx: &mut Fx, ra: u32| {
                let f = fx.sp - 0x100;
                fx.saved(f, &[(0x30, u64::from(ra)), (0x20, u64::from(D)), (0x10, u64::from(k)), (0, u64::from(D + 0x70 * k))]);
            };
            saves(fx, 0x01E0_2F70);
            column(fx, t, sp + 0xC0, 0);
            saves(fx, 0x01E0_2F84);
            column(fx, t, sp + 0xC0, 1);
        }
        if fx.mem.read_u32(t + 0x44) == PLANE {
            texture(fx, 0x37C);
            l2::rect(fx, sp + 0xA0, 0x9A, 0);
            copy4(fx, sp + 0xA0, D + 0x94);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sprite_quads(fx, D, t, 0, t + 0x10);
            quads4::sprite_quads(fx, D, t, 0, t + 0x10);
        }
        if fx.mem.read_u32(t + 0x44) == TRAIL {
            // The ribbon's saves: s0 = D + 0x70 k, s1 = k, s2 = D, s3 = the
            // count, s4..s6 the loop's registers, s7 / fp / f20 the
            // runner's (0 [H]).
            let base = [u64::from(D + 0x70 * k), u64::from(k), u64::from(D), u64::from(count)];
            let kind = fx.mem.i16(t + 0x50);
            if kind == 0 {
                for j in 0..3u32 {
                    let s = [base[0], base[1], base[2], base[3], u64::from(j), u64::from(D + 0x140 * j), u64::from(t), 0x01E0_3068];
                    ribbon(fx, t, HIST + 0x140 * j, HIST + 0x140 * (j + 4), s);
                }
            }
            if fx.mem.i16(t + 0x50) == 1 {
                for j in 10..13u32 {
                    let s = [base[0], base[1], base[2], base[3], u64::from(D + 0xC80 + 0x140 * (j - 10)), u64::from(t), u64::from(j), 0x01E0_30C8];
                    ribbon(fx, t, HIST + 0x140 * j, HIST + 0x140 * (j + 4), s);
                }
            }
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x51 => 1,
        0x91 => 4,
        0x7D => 2,
        0x5C..=0x77 => 6,
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

/// rand % m (HI) from a step's `jal` at `site` (its s0 = the task).
fn rmod(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, site + 8);
    fx.div(v, m).1
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// Alpha `a`: + `up` below `cap` until age `until`, then - `down` from
/// `floor` up, else the task ends.
fn fade(fx: &mut Fx, t: u32, a: u32, until: i32, cap: u8, up: u8, floor: u8, down: u8) {
    let v = fx.mem.read_u8(t + a);
    if fx.mem.i16(t + 0x54) < until {
        if v < cap {
            fx.mem.write_u8(t + a, v.wrapping_add(up));
        }
    } else if v >= floor {
        fx.mem.write_u8(t + a, v - down);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E01C00 / 0x1E01950 (D, task): a spark quad (b533's): red (size 100
/// x 50) or dark (200 x 100, cell rows two on); cell row (rand >> 4) % 3,
/// drift +0x30..+0x38 (-50..49) / 60 (unused), angle +0x5C random, the
/// camera's angles each update (0x205580 into +0x20; the spins +0..+4
/// added first are lost), orientation +0x52 (rand >> 4) % 4; in by 0x40
/// under 0x80 for 12 (8) updates, then out by 9 (4).
fn spark(fx: &mut Fx, t: u32, s1: u64, dark: bool) {
    let f = if dark { DARK } else { SPARK };
    // The `jal`s after the first sit one word later in the dark one.
    let at = |o: u32| if dark && o > 0x2C { f + o + 4 } else { f + o };
    if fx.mem.i16(t + 0x58) == 1 {
        let (size, extra, rgb) = if dark { (0xC8, 2, [0x10, 0x10, 0x10]) } else { (0x64, 0, [0xFF, 0x40, 0x40]) };
        fx.mem.set16(t + 8, size);
        let w = fx.mem.i16(t + 8) >> 1;
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, at(0x2C) + 8);
        let (_, h) = fx.div(v >> 4, 3);
        fx.mem.set16(t + 0x56, h + extra);
        l2::rgb(fx, t + 0x60, rgb[0], rgb[1], rgb[2]);
        fx.mem.set16(t + 0x58, 2);
        for (o, site) in [(0x34u32, 0x60u32), (0x30, 0x8C), (0x38, 0xB8)] {
            let v = rmod(fx, t, at(site), 0x64);
            fx.mem.setf(t + o, ee::from_i32(v - 0x32));
        }
        for o in [0x34u32, 0x30, 0x38] {
            let q = ee::div(fx.mem.f(t + o), 60.0);
            fx.mem.setf(t + o, q);
        }
        let v = rmod(fx, t, at(0x110), 0x168);
        fx.mem.set16(t + 0x5C, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), s1, at(0x134));
        for (o, site) in [(4u32, 0x134u32), (2, 0x154), (0, 0x174)] {
            let v = fx.rand_from(t, at(site) + 8);
            fx.mem.set16(t + o, v % 0x800 - 0x400);
        }
        let v = fx.rand_from(t, at(0x194) + 8);
        fx.mem.set16(t + 0x52, (v >> 4) % 4);
    }
    if dark {
        fade(fx, t, 0x63, 8, 0x80, 0x40, 6, 4);
    } else {
        fade(fx, t, 0x63, 0xC, 0x80, 0x40, 0xB, 9);
    }
    for (o, s) in [(0x28u32, 4u32), (0x24, 2), (0x20, 0)] {
        let v = fx.mem.read_u32(t + o).wrapping_add(fx.mem.i16(t + s) as u32);
        fx.mem.write_u32(t + o, v);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), s1, at(0x23C));
    age(fx, t);
}

/// 0x1E01BB0 (D, task): the arc: outer radius 220, inner red (0xFF, 0x40,
/// 0x40), outer white fading in by 0x60 under 0xC0 for 14 updates, then
/// out by 9; angles 10 / 10; turned by the camera's angles (its first +
/// 0x4000).
fn arc(fx: &mut Fx, t: u32, s1: u64) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x435C_0000);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x40);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    fade(fx, t, 0x67, 0xE, 0xC0, 0x60, 0xB, 9);
    camera_angles_saving(fx, t + 0x20, u64::from(t), s1, 0x01E0_1C70);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    age(fx, t);
}

/// 0x1E01CA0 (D, task): a rising light column: height +6 up by 4 to 72,
/// +2 up by 2 (the task ends at 181); white; on its first update +2 5,
/// random angles +0x20..+0x28 (rand % 0xFFFF), alpha 0xC0; from +2 = 60
/// both alphas down by 12 (the task ends when the first is under 14).
fn beam(fx: &mut Fx, t: u32) {
    let h = fx.mem.i16(t + 6);
    if h < 0x46 {
        fx.mem.set16(t + 6, h + 4);
    }
    let y = fx.mem.i16(t + 2) + 2;
    fx.mem.set16(t + 2, y);
    l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
    if fx.mem.i16(t + 2) >= 0xB5 {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 2, 5);
        fx.mem.set16(t, 0);
        fx.mem.set16(t + 4, 0);
        fx.mem.set16(t + 6, 0);
        for (o, site) in [(0x20u32, 0x01E0_1D18u32), (0x24, 0x01E0_1D34), (0x28, 0x01E0_1D50)] {
            let v = rmod(fx, t, site, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.write_u8(t + 0x63, 0xC0);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 2) >= 0x3C {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xE {
            fx.mem.write_u8(t + 0x63, a - 0xC);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let a = fx.mem.read_u8(t + 0x67);
        if a >= 0xE {
            fx.mem.write_u8(t + 0x67, a - 0xC);
        }
    }
}

/// 0x1E01DE0 (D, task): a light plane: kind +0x50 0: 18 x 800 growing by
/// 80, 1: 18 x 500 growing by 60, 2: 18 x 400; white / (0x80, 0x10,
/// 0x10); in by 0x40 for 4 updates, then out by 9 (b588's plane).
fn plane(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x80, 0x10, 0x10);
        for (kind, h) in [(0, 0x4448_0000u32), (1, 0x43FA_0000), (2, 0x43C8_0000)] {
            if fx.mem.i16(t + 0x50) == kind {
                fx.mem.write_u32(t + 0x30, 0x4190_0000);
                fx.mem.write_u32(t + 0x34, h);
            }
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    fade(fx, t, 0x63, 4, 0x80, 0x40, 0xB, 9);
    for (kind, dh) in [(0, 80.0f32), (1, 60.0)] {
        if fx.mem.i16(t + 0x50) == kind && fx.mem.i16(t + 0x54) != 0 {
            let h = ee::add(fx.mem.f(t + 0x34), dh);
            fx.mem.setf(t + 0x34, h);
        }
    }
    age(fx, t);
}

/// The trail's histories: kind 0 nodes D+0xA94C / D+0xA94E into
/// histories 0..3 / 4..7, kind 1 nodes D+0xA950 / D+0xA952 into 10..13 /
/// 14..17; each node's four 1.4 apart in y; the `jal`s of the node
/// fetches (first fill, history).
const TRAIL_NODES: [[(u32, u32, u32, u32); 2]; 2] = [
    [(0xA94C, 0, 0x01E0_1FEC, 0x01E0_2568), (0xA94E, 4, 0x01E0_212C, 0x01E0_261C)],
    [(0xA950, 10, 0x01E0_227C, 0x01E0_26F4), (0xA952, 14, 0x01E0_23BC, 0x01E0_27A8)],
];

/// 0x1E01F70 (D, task): the red trail (0xC0, 0x20, 0x20; frame 0x60):
/// on its first update +0x48 5, +0xE 30 and its histories filled (19
/// points each) with its nodes' points; alpha 0xFF until update 97, then
/// down by 9 (the task ends under 11); until update 98 the histories take
/// the nodes' points; +6 13 (the ribbons' point count).
fn trail(fx: &mut Fx, t: u32, sp: u32) {
    let y_down = |fx: &mut Fx| {
        let y = ee::sub(fx.mem.f(sp + 0x54), f32::from_bits(0x3FB3_3333));
        fx.mem.setf(sp + 0x54, y);
    };
    l2::rgb(fx, t + 0x60, 0xC0, 0x20, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        let kind = fx.mem.i16(t + 0x50);
        if (0..2).contains(&kind) {
            fx.mem.set16(t + 0xE, 0x1E);
            for (slot, j0, ra, _) in TRAIL_NODES[kind as usize] {
                let h = fx.self_handle();
                let n = fx.mem.i16(D + slot);
                node_position_saving(fx, h, n, sp + 0x50, u64::from(t), Some(u64::from(D)), ra);
                for j in j0..j0 + 4 {
                    if j != j0 {
                        y_down(fx);
                    }
                    let a = HIST + 0x140 * j;
                    for e in 0..0x13u32 {
                        for c in 0..3u32 {
                            let w = fx.mem.read_u32(sp + 0x50 + 4 * c);
                            fx.mem.write_u32(a + 0x10 * e + 4 * c, w);
                        }
                    }
                }
            }
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(COUNTER) < 0x61 {
        fx.mem.write_u8(t + 0x63, 0xFF);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xB {
            fx.mem.write_u8(t + 0x63, a - 9);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    for kind in 0..2i32 {
        if fx.mem.i16(t + 0x50) == kind && fx.mem.i16(COUNTER) < 0x62 {
            for (slot, j0, _, ra) in TRAIL_NODES[kind as usize] {
                let h = fx.self_handle();
                let n = fx.mem.i16(D + slot);
                node_position_saving(fx, h, n, sp + 0x50, u64::from(t), Some(u64::from(D)), ra);
                for j in j0..j0 + 4 {
                    if j != j0 {
                        y_down(fx);
                    }
                    lspawn::history(fx, HIST + 0x140 * j + 0x130, 0x14, sp + 0x50);
                }
            }
        }
    }
    fx.mem.set16(t + 6, 0xD);
    age(fx, t);
}

/// 0x1E02870 (D, task): the frame flash (frame 0x90; its descriptor at sp
/// + 0x38 = the library frame quad's, [`Fx::frame_quad_rgba`] from this
/// caller): the frame over the window in (0x80, 0x80, 0x80, alpha +0x63),
/// ALPHA 0x44; in by 0x14 under 0x50 while the timer is at least 21,
/// then out by 9 (the task ends below 11).
fn flash(fx: &mut Fx, t: u32, s0: u64, s1: u64) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let c = [fx.mem.read_u8(t + 0x60), fx.mem.read_u8(t + 0x61), fx.mem.read_u8(t + 0x62), fx.mem.read_u8(t + 0x63)];
    fx.frame_quad_rgba(c);
    let sp = fx.enter(0x90);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, s1), (0, s0)]);
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i32(TIMER) >= 0x15 {
        if a < 0x50 {
            fx.mem.write_u8(t + 0x63, a + 0x14);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    age(fx, t);
    fx.leave(0x90);
}

/// 0x1E031C0(D, step, 0): the first free task cleared and set off at
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
            let a = fx.mem.i16(KIND);
            fx.mem.set16(t + 0x50, a);
            let a = fx.mem.i16(ANGLE);
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

/// 0x1E01320(D, 0): the colour, the update counter and the spawns. Frame
/// 0x30 (the entry's s1 = the parameter, s0 = 0); 0x1E031C0's saves: the
/// runner's s4 (0 [H]), the entry's s3 = 0 / s2 = D, the emitter's s1 and
/// s0 = D.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x20, 0x01E0_2D64), (0x10, p), (0, 0)]);
    let go = |fx: &mut Fx, f: u32, ra: u32, s1: u64| {
        spawn(fx, f, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, u64::from(D)), (0x10, s1), (0, u64::from(D))]);
    };
    let node = |fx: &mut Fx, slot: u32, s1: u64, ra: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i16(D + slot);
        node_position_saving(fx, h, s, AT, u64::from(D), Some(s1), ra);
    };
    for (k, v) in [0x80u8, 0, 0, 0].into_iter().enumerate() {
        fx.mem.write_u8(COLOUR + k as u32, v);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if fx.mem.i16(COUNTER) == 0x55 {
        fx.mem.set16(KIND, 0);
        go(fx, TRAIL, 0x01E0_13B4, p);
        fx.mem.set16(KIND, 1);
    }
    if fx.mem.i16(COUNTER) == 1 {
        go(fx, FLASH, 0x01E0_13F0, p);
    }
    if fx.mem.i16(COUNTER) == 0x59 {
        node(fx, 0xA94E, p, 0x01E0_1428);
        go(fx, ARC, 0x01E0_143C, p);
        for k in 0..0x28u64 {
            go(fx, BEAM, 0x01E0_1454, k);
        }
        for (angle, kind, ra) in [(0, 0, 0x01E0_1490u32), (0x5A, 1, 0x01E0_14C4), (0x2D, 2, 0x01E0_14F8), (0x87, 2, 0x01E0_152C)] {
            fx.mem.set16(ANGLE, angle);
            fx.mem.set16(KIND, kind);
            go(fx, PLANE, ra, 0x28);
        }
    }
    if fx.mem.i16(COUNTER) < 0x4B {
        for (slot, f, ra_n, ra_r, ra_s) in [
            (0xA950u32, SPARK, 0x01E0_1564u32, 0x01E0_156Cu32, 0x01E0_15A8u32),
            (0xA952, SPARK, 0x01E0_15C8, 0x01E0_15D0, 0x01E0_160C),
            (0xA950, DARK, 0x01E0_162C, 0x01E0_1634, 0x01E0_1670),
            (0xA952, DARK, 0x01E0_1690, 0x01E0_1698, 0x01E0_16D4),
        ] {
            node(fx, slot, p, ra_n);
            let v = fx.rand_from(D, ra_r);
            let (_, h) = fx.div(v, 0x14);
            let x = ee::add(fx.mem.f(AT), ee::from_i32(h - 0xA));
            fx.mem.setf(AT, x);
            go(fx, f, ra_s, p);
        }
    }
    fx.leave(0x30);
}

/// 0x1E03360(D, task, a, b, ...): a gouraud ribbon (frame 0x2D0) between
/// the histories `a` and `b` over +6 - 1 segments of four sub-steps
/// (spline::spline at t, t + 0.25, the first segment's points doubled):
/// a's edge in the task's colour with alpha 0, b's in the colour with its
/// alpha down by 3 a sub-step (0 at 4 or under); each quad on the OT at
/// key 0. `s`: the entry's s0..s3, s4..s6 and the return address.
fn ribbon(fx: &mut Fx, t: u32, a: u32, b: u32, s: [u64; 8]) {
    use ee::add;
    let sp = fx.enter(0x2D0);
    fx.saved(sp, &[(0x10, s[0]), (0x20, s[1]), (0x30, s[2]), (0x40, s[3]), (0x50, s[4]), (0x60, s[5]), (0x70, s[6]), (0xA0, s[7])]);
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x208;
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    let (ca, cb) = (sp + 0x2C8, sp + 0x2CC);
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(ca + k, v);
    }
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(cb + k, v);
    }
    fx.mem.write_u8(ca + 3, 0);
    let (pa, pb) = (sp + 0x140, sp + 0x180);
    for (p, src) in [(pa, a), (pb, b)] {
        copy4(fx, src, p);
        copy4(fx, src, p + 0x10);
        copy4(fx, src + 0x10, p + 0x20);
        copy4(fx, src + 0x20, p + 0x30);
    }
    rgbaq_at(fx, sp + 0x298, ca, d + 0x10);
    rgbaq_at(fx, sp + 0x2A0, ca, d + 0x20);
    rgbaq_at(fx, sp + 0x2A8, cb, d + 0x30);
    rgbaq_at(fx, sp + 0x2B0, cb, d + 0x40);
    let mut s0 = 0;
    while s0 < n - 1 {
        let off = (0x10 * (s0 - 1)) as u32;
        let (s4, s5) = (a.wrapping_add(off), b.wrapping_add(off));
        let mut f20 = 0.0f32;
        for _ in 0..4 {
            rgbaq_at(fx, sp + 0x2B8, cb, d + 0x30);
            let al = fx.mem.read_u8(cb + 3);
            fx.mem.write_u8(cb + 3, if al > 4 { al - 3 } else { 0 });
            rgbaq_at(fx, sp + 0x2C0, cb, d + 0x40);
            let (qa, qb, oa, ob) = if s0 != 0 { (s4, s5, 0x1C0u32, 0x1D0u32) } else { (pa, pb, 0x1E0, 0x1F0) };
            spline::spline(fx, sp + 0x100, qa, f20, 0, [0.0; 4]);
            spline::spline(fx, sp + 0x120, qb, f20, 0, [0.0; 4]);
            f20 = add(f20, 0.25);
            spline::spline(fx, sp + oa, qa, f20, 0, [0.0; 4]);
            copy4(fx, sp + oa, sp + 0x110);
            spline::spline(fx, sp + ob, qb, f20, 0, [0.0; 4]);
            copy4(fx, sp + ob, sp + 0x130);
            for (to, from) in [(0xC0u32, 0x100u32), (0xD0, 0x110), (0xE0, 0x120), (0xF0, 0x130)] {
                for c in 0..3 {
                    let w = fx.mem.read_u32(sp + from + 4 * c);
                    fx.mem.write_u32(sp + to + 4 * c, w);
                }
            }
            if l2::project(fx, sp + 0xC0, SPR, 0, d, sp + 0xB0) {
                fx.submit(d, OT, 0);
            }
        }
        s0 += 1;
    }
    fx.leave(0x2D0);
}

/// 0x1E038B0(D, task, desc, mode): the light column: mode 0 a strip3 of
/// sprite 0x14 cell +0x56 (texture D+0xA8E4), bottom +0x60 / top +0x64
/// colours; mode 1 its four corners (the task's i16 point, x + 4.7; the
/// top y - height +6, all y clamped to 0..200) through the camera x the
/// task's angles +0x20 at its point +0x10 (0x20E2B0 / 0x20C0B0 /
/// 0x20B370 into D+0x290), keyed by the mean w (D+0xA93C) on D+0xA90C.
fn column(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x100);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x56);
        l2::rect(fx, sp + 0xD0, 0x14, c);
        l2::desc_strip3(fx, d);
        rgbaq_at(fx, sp + 0xE0, t + 0x60, d + 0x10);
        rgbaq_at(fx, sp + 0xE8, t + 0x60, d + 0x28);
        rgbaq_at(fx, sp + 0xF0, t + 0x64, d + 0x40);
        rgbaq_at(fx, sp + 0xF8, t + 0x64, d + 0x58);
        l2::uv_strip3(fx, d, sp + 0xD0, 0);
        let w = fx.mem.read_u32(D + 0xA8E4);
        fx.desc_texture(d, w, 1);
    } else {
        camera_vp_into(fx, VP);
        l2::copy3_dir(fx, HIST, sp + 0xC0, 1);
        for k in 0..3u32 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * k);
            fx.mem.write_u32(sp + 0xC0 + 4 * k, w);
        }
        rot_trans_into(fx, sp + 0x80, t + 0x20, sp + 0xC0);
        mul_mem(fx, VP, sp + 0x80);
        let wide = f32::from_bits(0x4096_6666);
        for k in 0..3u32 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(sp + 0x40 + 4 * k, v);
        }
        copy4(fx, sp + 0x40, sp + 0x50);
        let x = ee::add(fx.mem.f(sp + 0x50), wide);
        fx.mem.setf(sp + 0x50, x);
        for k in 0..3u32 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(sp + 0x60 + 4 * k, v);
        }
        let y = ee::sub(fx.mem.f(sp + 0x64), ee::from_i32(fx.mem.i16(t + 6)));
        fx.mem.setf(sp + 0x64, y);
        if ee_lt(y, 0.0) {
            fx.mem.set32(sp + 0x64, 0);
        }
        copy4(fx, sp + 0x60, sp + 0x70);
        let x = ee::add(fx.mem.f(sp + 0x70), wide);
        fx.mem.setf(sp + 0x70, x);
        for o in [0x44u32, 0x54, 0x64, 0x74] {
            if !ee_le(fx.mem.f(sp + o), 200.0) {
                fx.mem.write_u32(sp + o, 0x4348_0000);
            }
        }
        for o in [0x44u32, 0x54, 0x64, 0x74] {
            if ee_lt(fx.mem.f(sp + o), 0.0) {
                fx.mem.set32(sp + o, 0);
            }
        }
        if l2::project(fx, sp + 0x40, SPR, VP, d, MEAN) {
            let m = fx.mem.f(MEAN);
            fx.fix_residue(m, 0x01E0_3C74);
            let key = ee::fixunssfsi(m) as i32;
            let ot = fx.mem.read_u32(OTW);
            fx.submit(d, ot, key);
        }
    }
    fx.leave(0x100);
}
