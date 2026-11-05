//! BMODULE b119 (card 164), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04240, data area 0x1ED0000, 0x4590 bytes) [F]
//!, from helper B37's map: b538's task pool (200
//! tasks of 0x40 at D+0x2A0: alive +0x36, age +0x32, step +0x24 compared
//! by address and `jal`'d), spawned by 0x1E05220 (= b538's spawn on this
//! record's globals: colour D+0x3ED4, point D+0x250, kind D+0x3F14 to
//! +0x30, the argument +0x20). Nodes (self) 0x5F (clear) / 0x68 / 0x20 /
//! 0x10 / 0x16 / 9 / 0xA / 1 at D+0x3EFC..; textures (4, 0xD..0xF)
//! D+0x294.., (5, 0..3) D+0x3EB0..; sines D+0x4420 / D+0xD8; jitter bytes
//! D+0x3F1E / D+0x401E / D+0x411E / D+0x421E; 150 points D+0x34D0 (+0x10
//! i, the index D+0x3F18); three cone records D+0x3E30 (+0x20 k).
//!
//! The emitter 0x1E01800 spawns the camera light 0x1E03ED0 (D+0x34A0's
//! block through 0x1F7E20) and the stage light 0x1E04100 on update 1,
//! three cones (0x1E02BE0 / 0x1E02F60 / 0x1E030F0 per colour) on 1,
//! twelve 0x1E03A00 bands on 0xEE, ten 0x1E03510 sprites every 10th
//! update 0x47..0xE5, white bands 0x1E026D0 / 0x1E03C10, point bands
//! 0x1E021F0 / 0x1E024C0 / 0x1E032A0 / 0x1E03810 and turned bands
//! 0x1E02990. 0x1E03690 and 0x1E041C0 are tested by the entry but never
//! spawned (not ported). Draws: the ring points 0x1E01050 (a 0x40-layout
//! copy of `billboard::ring_points` keyed by the mean w above 20),
//! `quads4::band18_turned`, `quads4::sprite_turned`, the cone 0x1E05340
//! and b582's ring band with inner radius 0 (0x1E056A0). Returns 1 on
//! update 0x104, 3 on 0x163, 6 on 0x10E..0x15D, 8 once the 400-update
//! timer D+0x3ED0 has run out.
use crate::blib::{quads4, uv};
use crate::bus::Bus;
use crate::c061::{blend_alpha, desc_uv4, rgbaq_at, to_degrees_neg};
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 200;
const POOL: u32 = D + 0x2A0;
const POS: u32 = D + 0x250;
const PTS: u32 = D + 0x34D0;
const CAM: u32 = D + 0x34A0;
const RECS: u32 = D + 0x3E30;
const TEX5: u32 = D + 0x3EB0;
const TIMER: u32 = D + 0x3ED0;
const COLOUR: u32 = D + 0x3ED4;
const NODES: u32 = D + 0x3EFC;
const KIND: u32 = D + 0x3F14;
const YAW: u32 = D + 0x3F16;
const INDEX: u32 = D + 0x3F18;
const PARAM: u32 = D + 0x3F1A;
const COUNTER: u32 = D + 0x3F1C;
const BYTES_A: u32 = D + 0x3F1E;
const BYTES_B: u32 = D + 0x401E;
const BYTES_C: u32 = D + 0x411E;
const BYTES_D: u32 = D + 0x421E;
const SINES: u32 = D + 0x4420;

const POINT_A: u32 = 0x01E0_21F0;
const POINT_B: u32 = 0x01E0_24C0;
const WHITE_A: u32 = 0x01E0_26D0;
const TURNED: u32 = 0x01E0_2990;
const CONE: u32 = 0x01E0_2BE0;
const CONE_TOP: u32 = 0x01E0_2F60;
const CONE_BASE: u32 = 0x01E0_30F0;
const POINT_C: u32 = 0x01E0_32A0;
const SPRITE: u32 = 0x01E0_3510;
const POINT_D: u32 = 0x01E0_3810;
const BAND: u32 = 0x01E0_3A00;
const WHITE_B: u32 = 0x01E0_3C10;
const CAMERA: u32 = 0x01E0_3ED0;
const STAGE: u32 = 0x01E0_4100;

/// The entry 0x1E04240 (D, param), frame 0x100.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x100);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x3EDC, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x100);
    ret
}

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// rand % 2^k as the compiler's signed remainder (andi / -2^k).
fn rpow(fx: &mut Fx, s0: u32, ra: u32, k: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    rem_pow(v, k)
}

fn rem_pow(v: i32, k: i32) -> i32 {
    let r = v & (k - 1);
    if v < 0 && r != 0 {
        r - k
    } else {
        r
    }
}

fn fix(fx: &mut Fx, v: f32, ra: u32) -> i32 {
    fx.fix_residue(v, ra);
    ee::fixsfsi(v)
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4590);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(PARAM, param);
    fx.mem.set32(TIMER, 0x190);
    for (k, (node, clear)) in [(0x5F, 1), (0x68, 0), (0x20, 0), (0x10, 0), (0x16, 0), (9, 0), (0xA, 0), (1, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    for k in 0xDu32..0x10 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x294 + 4 * (k - 0xD), t);
    }
    for k in 0..4u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX5 + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xE0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xE4)), 180.0);
    let y = fix(fx, y, 0x01E0_43EC);
    fx.mem.set16(YAW, y);
    for k in 0..0x100u32 {
        let r = rmod(fx, D, 0x01E0_43FC, 0x14);
        fx.mem.set8(BYTES_A + k, r - 0xA);
    }
    for k in 0x64..0xB4u32 {
        let r = rmod(fx, D, 0x01E0_4430, 0xF);
        fx.mem.set8(BYTES_A + k, r);
    }
    for k in 0..0x100u32 {
        let r = rmod(fx, D, 0x01E0_445C, 0x12);
        fx.mem.set8(BYTES_B + k, r - 9);
    }
    for k in 0..0x100u32 {
        let r = rmod(fx, D, 0x01E0_4490, 0x12);
        fx.mem.set8(BYTES_C + k, r - 9);
    }
    for k in 0x64..0xB4u32 {
        fx.mem.set8(BYTES_C + k, 0);
    }
    for k in 0..0x100u32 {
        let r = rmod(fx, D, 0x01E0_44E4, 0x1E);
        fx.mem.set8(BYTES_D + k, r - 0xF);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let s = fx.mem.read_u32(t + 0x24);
        // The step's saved registers (the entry's s0 = D, s1 = 0, s2 = D +
        // 0x40 k) and return address, left below the entry's frame.
        let ra = match s {
            POINT_A => 0x01E0_4554,
            POINT_B => 0x01E0_4578,
            WHITE_A => 0x01E0_4598,
            TURNED => 0x01E0_45B8,
            CONE => 0x01E0_45D8,
            CONE_TOP => 0x01E0_45F8,
            CONE_BASE => 0x01E0_4618,
            POINT_C => 0x01E0_4638,
            SPRITE => 0x01E0_4658,
            POINT_D => 0x01E0_4698,
            BAND => 0x01E0_46B8,
            WHITE_B => 0x01E0_46D8,
            CAMERA => 0x01E0_46F8,
            STAGE => 0x01E0_4718,
            _ => 0,
        };
        let (d, row) = (u64::from(D), u64::from(D + 0x40 * k));
        match s {
            POINT_A | CONE | SPRITE | BAND => fx.saved(sp - 0x30, &[(0, d), (0x10, 0), (0x20, ra)]),
            CAMERA => fx.saved(sp - 0x60, &[(0, d), (0x10, 0), (0x20, row), (0x30, ra)]),
            STAGE => fx.saved(sp - 0x10, &[(0, ra)]),
            0 => {}
            _ if ra != 0 => fx.saved(sp - 0x20, &[(0, d), (0x10, ra)]),
            _ => {}
        }
        match s {
            POINT_A => point_a(fx, t),
            POINT_B => point_b(fx, t),
            WHITE_A => white(fx, t, 0x01E0_26D0, 2, 1),
            TURNED => turned(fx, t),
            CONE => cone(fx, t),
            CONE_TOP => cone_top(fx, t),
            CONE_BASE => cone_base(fx, t),
            POINT_C => point_cd(fx, t, [0x32CC, 0x3360, 0x3380, 0x339C, 0x33BC, 0x33D8, 0x33F8], true),
            SPRITE => sprite(fx, t),
            POINT_D => point_cd(fx, t, [0x383C, 0x385C, 0x387C, 0x3898, 0x38B8, 0x38D4, 0x38F4], false),
            BAND => band(fx, t),
            WHITE_B => white(fx, t, 0x01E0_3C10, 3, 6),
            CAMERA => camera(fx, t),
            STAGE => stage(fx, t),
            _ => {}
        }
    }
    let h = fx.target_handle();
    crate::b076::unit_position_saving(fx, h, sp + 0x50, u64::from(D), 0x01E0_47AC);
    let mut count = 0u64;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        count += 1;
        draw(fx, sp, t, count);
    }
    let n = fx.mem.i32(TIMER);
    if n != 0 {
        fx.mem.set32(TIMER, n - 1);
    }
    let c = fx.mem.i16(COUNTER);
    if c == 0x104 {
        1
    } else if c == 0x163 {
        3
    } else if (0x10E..0x15E).contains(&c) {
        6
    } else if fx.mem.i32(TIMER) > 0 {
        0
    } else {
        8
    }
}

/// The task's i16 point as floats at `out`.
fn point_f(fx: &mut Fx, t: u32, out: u32) {
    for k in 0..3 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(out + 4 * k, v);
    }
}

/// The cone record of the task's kind +0x30.
fn rec(fx: &mut Fx, t: u32) -> u32 {
    (RECS as i32 + 0x20 * fx.mem.i16(t + 0x30)) as u32
}

/// 0x1F6830 on the self's node at D+0x3EFC + `slot` into `out`, its
/// frame saving the caller's s0 / s1 and `ra`.
fn node_to(fx: &mut Fx, slot: u32, out: u32, s0: u64, s1: u64, ra: u32) {
    let h = fx.self_handle();
    let n = fx.mem.i16(NODES + slot);
    crate::b076::node_position_saving(fx, h, n, out, s0, Some(s1), ra);
}

/// The ring points' set-up: the rect (`id`, cell +0x34) at `local` into
/// D+0x94, texture `tex`, D+0xB8 = `n`, the centre D+0x20 (the point
/// D+0x34D0 + 0x10 +0xA, or the task's own point).
#[allow(clippy::too_many_arguments)]
fn points(fx: &mut Fx, sp: u32, t: u32, local: u32, id: i32, tex: u32, n: i32, table: u32, own: bool) {
    let c = fx.mem.i16(t + 0x34);
    l2::rect(fx, sp + local, id, c);
    copy4(fx, D + 0x94, sp + local);
    let w = fx.mem.read_u32(tex);
    fx.mem.write_u32(D + 0x88, w);
    fx.mem.set16(D + 0xB8, n);
    fx.mem.set16(D + 0xBE, 0);
    if own {
        point_f(fx, t, D + 0x20);
    } else {
        let p = (PTS as i32 + 0x10 * fx.mem.i16(t + 0xA)) as u32;
        for k in 0..3 {
            let w = fx.mem.read_u32(p + 4 * k);
            fx.mem.write_u32(D + 0x20 + 4 * k, w);
        }
    }
    ring_points40(fx, t, 0x20, table);
    fx.mem.set16(D + 0xB8, 0);
}

fn draw(fx: &mut Fx, sp: u32, t: u32, count: u64) {
    let dd = u64::from(D);
    let s = fx.mem.read_u32(t + 0x24);
    let p = sp + 0x50;
    match s {
        POINT_A => points(fx, sp, t, 0x60, 0x9B, D + 0x29C, 0xF, BYTES_A, false),
        POINT_B => {
            node_to(fx, 6, p, dd, count, 0x01E0_48A4);
            points(fx, sp, t, 0x70, 0x9B, D + 0x29C, 8, BYTES_A, false);
        }
        WHITE_A => points(fx, sp, t, 0x80, 0x14, D + 0x3EB8, 5, BYTES_C, true),
        TURNED => {
            point_f(fx, t, p);
            quads4::band18_turned(fx, D, t, 1, p);
        }
        CONE => {
            node_to(fx, 0xE, p, dd, count, 0x01E0_4A8C);
            fx.mem.set32(p + 4, 0);
            let r = rec(fx, t);
            let z = fx.mem.f(p + 8);
            for (o, add) in [(0u32, false), (4, false), (8, true), (0x10, false), (0x14, false), (0x18, true)] {
                let v = fx.mem.f(r + o);
                fx.mem.setf(D + 0x20 + o, if add { ee::add(v, z) } else { v });
            }
            cone_draw(fx, t, p);
        }
        CONE_TOP => {
            node_to(fx, 0xE, p, dd, count, 0x01E0_4B5C);
            let r = rec(fx, t);
            let x = fx.mem.f(r + 0x10);
            fx.mem.setf(p, x);
            fx.mem.write_u32(p + 4, 0x41F0_0000);
            let z = ee::add(fx.mem.f(r + 0x18), fx.mem.f(p + 8));
            fx.mem.setf(p + 8, z);
            quads4::band18_turned(fx, D, t, 0, p);
        }
        CONE_BASE => {
            node_to(fx, 0xE, p, dd, count, 0x01E0_4BDC);
            let r = rec(fx, t);
            let (x, y) = (fx.mem.f(r), fx.mem.f(r + 4));
            fx.mem.setf(p, x);
            fx.mem.setf(p + 4, y);
            let z = ee::add(fx.mem.f(r + 8), fx.mem.f(p + 8));
            fx.mem.setf(p + 8, z);
            quads4::band18_turned(fx, D, t, 0, p);
        }
        POINT_C => points(fx, sp, t, 0x90, 0x14, D + 0x3EB8, 0xA, BYTES_D, false),
        SPRITE => {
            let w = fx.mem.read_u32(TEX5);
            fx.mem.write_u32(D + 0x88, w);
            let c = fx.mem.i16(t + 0x34);
            l2::rect(fx, sp + 0xA0, 0x14, c);
            copy4(fx, D + 0x94, sp + 0xA0);
            fx.mem.set16(D + 0xBE, 0);
            point_f(fx, t, p);
            quads4::sprite_turned(fx, uv::uv_rect, D, t, 0, p);
        }
        POINT_D => points(fx, sp, t, 0xC0, 0x14, D + 0x3EBC, 0xF, BYTES_D, false),
        BAND => {
            node_to(fx, 0xE, p, dd, count, 0x01E0_4F34);
            fx.mem.set32(p + 4, 0);
            let r = rec(fx, t);
            let x = fx.mem.f(r);
            fx.mem.setf(p, x);
            let y = fx.mem.f(r + 4);
            fx.mem.setf(p + 4, y);
            let z = ee::add(fx.mem.f(r + 8), fx.mem.f(p + 8));
            fx.mem.setf(p + 8, z);
            let z = fx.mem.f(p + 8);
            for (o, add) in [(0u32, false), (4, false), (8, true), (0x10, false), (0x14, false), (0x18, true)] {
                let v = fx.mem.f(r + o);
                fx.mem.setf(D + 0x20 + o, if add { ee::add(v, z) } else { v });
            }
            for k in 0..3u32 {
                let v = ee::div(ee::sub(fx.mem.f(D + 0x30 + 4 * k), fx.mem.f(D + 0x20 + 4 * k)), 100.0);
                fx.mem.setf(D + 0x20 + 4 * k, v);
            }
            for k in 0..3u32 {
                let v = ee::mul(fx.mem.f(D + 0x20 + 4 * k), ee::from_i32(fx.mem.i16(t + 0xA)));
                fx.mem.setf(D + 0x20 + 4 * k, v);
            }
            band_draw(fx, t, p);
        }
        WHITE_B => points(fx, sp, t, 0xD0, 0x14, D + 0x3EB8, 5, BYTES_C, true),
        _ => {}
    }
}

/// 0x1E05220 (D, step, 0): the first free task cleared, alive 1, the
/// colour D+0x3ED4 at +0x28 / +0x2C, the point D+0x250 as i16, the kind
/// D+0x3F14 at +0x30, arg +0x20, the step +0x24.
fn spawn(fx: &mut Fx, step: u32) {
    fx.enter(0x60);
    if let Some(k) = (0..TASKS).find(|&k| fx.mem.i16(POOL + 0x40 * k + 0x36) == 0) {
        let t = POOL + 0x40 * k;
        fx.bzero(t, 0x40);
        fx.mem.set16(t + 0x36, 1);
        for c in [0x28u32, 0x2C] {
            let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
            l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
            let a = fx.mem.read_u8(COLOUR + 3);
            fx.mem.write_u8(t + c + 3, a);
        }
        l2::to_i16x4(fx, t, POS, 0);
        let a = fx.mem.i16(KIND);
        fx.mem.set16(t + 0x30, a);
        fx.mem.write_u32(t + 0x20, 0);
        fx.mem.write_u32(t + 0x24, step);
    }
    fx.leave(0x60);
}

/// The emitter's 0x1F6830 into sp + 0x40, copied to the spawn point.
fn node_pos(fx: &mut Fx, slot: u32, sp: u32, s0: u64, ra: u32) {
    node_to(fx, slot, sp + 0x40, s0, u64::from(D), ra);
    copy3(fx, POS, sp + 0x40);
}

fn copy3(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// The next point D+0x34D0 + 0x10 i (i = D+0x3F18, wrapping at 150).
fn next_index(fx: &mut Fx) {
    let i = fx.mem.i16(INDEX) + 1;
    fx.mem.set16(INDEX, i);
    if fx.mem.i16(INDEX) >= 0x96 {
        fx.mem.set16(INDEX, 0);
    }
}

fn index_point(fx: &mut Fx) -> u32 {
    (PTS as i32 + 0x10 * fx.mem.i16(INDEX)) as u32
}

/// A point band at node slot `slot`'s position into the next point.
fn point_at(fx: &mut Fx, slot: u32, step: u32, s0: u64, ra: u32) {
    let p = index_point(fx);
    node_to(fx, slot, p, s0, u64::from(D), ra);
    let i = fx.mem.i16(INDEX);
    fx.mem.set16(KIND, i);
    spawn(fx, step);
    next_index(fx);
}

/// 0x1E01800 (D, 0): the emitter (frame 0x50; s0 tracked for its
/// callees' frames, s1 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0, u64::from(D)), (0x10, 0), (0x20, param as i64 as u64), (0x30, 0x01E0_4520)]);
    let mut s0 = u64::from(D);
    l2::rgb(fx, COLOUR, 0x80, 0, 0);
    fx.mem.write_u8(COLOUR + 3, 0);
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    if c == 1 {
        spawn(fx, STAGE);
    }
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, CAMERA);
    }
    if fx.mem.i16(COUNTER) == 0x1F {
        spawn(fx, POINT_A);
    }
    if fx.mem.i16(COUNTER) == 1 {
        for k in 0..3 {
            fx.mem.set16(KIND, k);
            spawn(fx, CONE);
            spawn(fx, CONE_TOP);
            spawn(fx, CONE_BASE);
        }
    }
    if fx.mem.i16(COUNTER) == 0xEE {
        for k in 0..3 {
            fx.mem.set16(KIND, k);
            for _ in 0..4 {
                spawn(fx, BAND);
            }
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x46..0xE7).contains(&c) && fx.div(c, 10).1 == 1 {
        fx.mem.set16(KIND, 0);
        node_pos(fx, 0, sp, s0, 0x01E0_1ADC);
        for _ in 0..10 {
            spawn(fx, SPRITE);
        }
        s0 = 10;
    }
    let c = fx.mem.i16(COUNTER);
    if (0x46..0x123).contains(&c) {
        node_pos(fx, 2, sp, s0, 0x01E0_1B54);
        spawn(fx, WHITE_A);
        s0 = u64::from(sp + 0x44);
        node_pos(fx, 4, sp, s0, 0x01E0_1BA0);
        spawn(fx, WHITE_A);
    }
    let c = fx.mem.i16(COUNTER);
    if (0xC8..0x123).contains(&c) {
        node_pos(fx, 0xE, sp, s0, 0x01E0_1C00);
        let y = ee::sub(fx.mem.f(sp + 0x44), 15.0);
        fx.mem.setf(POS + 4, y);
        spawn(fx, WHITE_B);
    }
    for (lo, hi, split, ras) in [(0x14, 0x3D, 0x21, [0x01E0_1C6C, 0x01E0_1CD8, 0x01E0_1D48]), (0xF5, 0x120, 0xFA, [0x01E0_1DCC, 0x01E0_1E38, 0x01E0_1EA8])] {
        let c = fx.mem.i16(COUNTER);
        if (lo..hi).contains(&c) {
            node_pos(fx, 4, sp, s0, ras[0]);
            fx.mem.set16(KIND, 0);
            spawn(fx, TURNED);
            let i = fx.mem.i16(INDEX);
            fx.mem.set16(KIND, i);
            if fx.mem.i16(COUNTER) < split {
                let p = index_point(fx);
                node_to(fx, 4, p, 0, u64::from(D), ras[1]);
                spawn(fx, POINT_A);
                next_index(fx);
                s0 = 1;
            } else {
                for k in 0..2 {
                    point_at(fx, 4, POINT_A, k, ras[2]);
                }
                s0 = 2;
            }
        }
    }
    if fx.mem.i16(COUNTER) < 0x15F {
        point_at(fx, 6, POINT_B, s0, 0x01E0_1F2C);
        point_at(fx, 8, POINT_B, s0, 0x01E0_1F8C);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x122..0x15F).contains(&c) {
        let h = fx.target_handle();
        crate::b076::unit_position_saving(fx, h, sp + 0x40, s0, 0x01E0_1FF8);
        copy3(fx, POS, sp + 0x40);
        fx.mem.set16(KIND, 1);
        spawn(fx, TURNED);
    }
    for (step, ra) in [(POINT_C, 0x01E0_2074), (POINT_D, 0x01E0_2108)] {
        let c = fx.mem.i16(COUNTER);
        if (0x118..0x15F).contains(&c) {
            let i = fx.mem.i16(INDEX);
            fx.mem.set16(KIND, i);
            let h = fx.target_handle();
            let p = index_point(fx);
            crate::b076::unit_position_saving(fx, h, p, 0, ra);
            let i = fx.mem.i16(INDEX);
            fx.mem.set16(KIND, i);
            spawn(fx, step);
            next_index(fx);
            s0 = 1;
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x50..0x65).contains(&c) {
        node_pos(fx, 8, sp, s0, 0x01E0_218C);
        copy3(fx, POS, sp + 0x40);
        fx.mem.set16(KIND, 0);
        fx.mem.set16(KIND, 1);
    }
    fx.leave(0x50);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// The three-way colour at +0x28 by `r`: blue, red or yellow (0x20 / 0x80).
fn colour3(fx: &mut Fx, t: u32, r: i32) {
    match r as i16 {
        0 => l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x80),
        1 => l2::rgb(fx, t + 0x28, 0x80, 0x20, 0x20),
        2 => l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x20),
        _ => {}
    }
}

/// The kind's colour (red / green / blue in `hi` over `lo`) at +0x28 and
/// +0x2C.
fn kind_colour(fx: &mut Fx, t: u32, hi: i32, lo: i32) {
    let c = match fx.mem.i16(t + 0x30) {
        0 => [hi, lo, lo],
        1 => [lo, hi, lo],
        2 => [lo, lo, hi],
        _ => return,
    };
    for o in [0x28u32, 0x2C] {
        l2::rgb(fx, t + o, c[0], c[1], c[2]);
    }
}

/// Alpha +0x2B: up by `up` under `cap` during the life +6, then down by
/// `down` (the task over under `floor`).
fn fade(fx: &mut Fx, t: u32, cap: u8, up: u8, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < cap {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(up));
        }
    } else if a < floor {
        fx.mem.set16(t + 0x36, 0);
    } else {
        fx.mem.write_u8(t + 0x2B, a.wrapping_sub(down));
    }
}

fn add16(fx: &mut Fx, at: u32, v: i32) {
    let n = fx.mem.i16(at) + v;
    fx.mem.set16(at, n);
}

fn turn(fx: &mut Fx, t: u32, by: i32) {
    let d = if fx.mem.i16(t + 0xC) == 0 { by } else { -by };
    add16(fx, t + 0x3A, d);
}

/// 0x1E021F0 (D, task): a point band (size 100) at the point +0xA, its
/// y jittering by rand % 2, turning by 5.
fn point_a(fx: &mut Fx, t: u32) {
    fx.enter(0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x64);
        fx.mem.set16(t + 0x34, 0);
        let r = rmod(fx, t, 0x01E0_2224, 3);
        colour3(fx, t, r);
        let k = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, k);
        let r = rmod(fx, t, 0x01E0_22B8, 0x19);
        fx.mem.set16(t + 0x30, r + 1);
        let r = rmod(fx, t, 0x01E0_22D8, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let r = rmod(fx, t, 0x01E0_22F4, 0xA);
        fx.mem.set16(t + 0xE, r + 2);
        let r = rmod(fx, t, 0x01E0_2314, 0x23);
        fx.mem.set16(t + 8, r);
        let r = rpow(fx, t, 0x01E0_2330, 2);
        fx.mem.set16(t + 6, r + 2);
        let r = rpow(fx, t, 0x01E0_2350, 2);
        fx.mem.set16(t + 0xC, r);
        fx.mem.set16(t + 0x36, 2);
    }
    fade(fx, t, 0x80, 0x2A, 0xF, 0xD);
    if fx.mem.i16(t + 0x32) != 0 {
        let r = rpow(fx, t, 0x01E0_23D4, 2);
        add16(fx, t + 2, r as i16 as i32);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = rpow(fx, t, 0x01E0_2410, 2);
        let p = (PTS as i32 + 0x10 * fx.mem.i16(t + 0xA)) as u32 + 4;
        let y = ee::add(fx.mem.f(p), ee::from_i32(r));
        fx.mem.setf(p, y);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        turn(fx, t, 5);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let e = fx.mem.i16(t + 0xE);
        add16(fx, t + 0x30, e);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E024C0 (D, task): a point band (size 40) from radius 0.
fn point_b(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x28);
        fx.mem.set16(t + 0x34, 0);
        let r = rmod(fx, t, 0x01E0_24EC, 3);
        colour3(fx, t, r);
        let k = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, k);
        fx.mem.set16(t + 0x30, 0);
        let r = rmod(fx, t, 0x01E0_2584, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let r = rmod(fx, t, 0x01E0_25A0, 0xA);
        fx.mem.set16(t + 0xE, r + 2);
        let r = rmod(fx, t, 0x01E0_25C0, 0x23);
        fx.mem.set16(t + 8, r);
        let r = rpow(fx, t, 0x01E0_25DC, 2);
        fx.mem.set16(t + 6, r + 3);
        fx.mem.set16(t + 0xC, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    fade(fx, t, 0x80, 0x2A, 0xF, 0xD);
    if fx.mem.i16(t + 0x32) != 0 {
        let r = rpow(fx, t, 0x01E0_2668, 2);
        add16(fx, t + 2, r as i16 as i32);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let e = fx.mem.i16(t + 0xE);
        add16(fx, t + 0x30, e);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E026D0 / 0x1E03C10 (D, task) at record address `at`: a white band
/// (size 200, cell rand % 8 cycling), radius 100..499; life rand % 4 +
/// `life`; y jitter rand % 2 (0x26D0) or % 6 (0x3C10, `jit` 6).
fn white(fx: &mut Fx, t: u32, at: u32, life: i32, jit: i32) {
    fx.enter(0x20);
    let ra = |o: u32| at + o;
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0xC8);
        let r = rpow(fx, t, ra(0x28), 8);
        fx.mem.set16(t + 0x34, r);
        let r = rmod(fx, t, ra(0x44), 3);
        colour3(fx, t, r);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        let k = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, k);
        let r = rmod(fx, t, ra(0xEC), 0x190);
        fx.mem.set16(t + 0x30, r + 0x64);
        let r = rmod(fx, t, ra(0x10C), 0x168);
        fx.mem.set16(t + 0x3A, r);
        let r = rmod(fx, t, ra(0x128), 0xA);
        fx.mem.set16(t + 0xE, r + 2);
        let r = rmod(fx, t, ra(0x148), 0x23);
        fx.mem.set16(t + 8, r);
        let r = rpow(fx, t, ra(0x164), 4);
        fx.mem.set16(t + 6, r + life);
        let r = rpow(fx, t, ra(0x184), 2);
        fx.mem.set16(t + 0xC, r);
        fx.mem.set16(t + 0x36, 2);
    }
    fade(fx, t, 0xC0, 0x40, 0x15, 0x13);
    add16(fx, t + 0x34, 1);
    if fx.mem.i16(t + 0x34) >= 8 {
        fx.mem.set16(t + 0x34, 0);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = if jit == 1 { rpow(fx, t, ra(0x228), 2) } else { rmod(fx, t, ra(0x228), jit) };
        add16(fx, t + 2, r as i16 as i32);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        turn(fx, t, 1);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let e = fx.mem.i16(t + 0xE);
        add16(fx, t + 0x30, e);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02990 (D, task): a turned band (size 80 growing by 10..14), grey
/// inside, one of six dark colours outside, alpha +0x2F in by 0x80 for 20
/// updates then out by 9; facing the camera turned by 0x4000.
fn turned(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x50);
        l2::rgb(fx, t + 0x28, 8, 8, 8);
        l2::rgb(fx, t + 0x2C, 0x10, 0x10, 0x40);
        let r = rmod(fx, t, 0x01E0_29E4, 6);
        let c = match r as i16 {
            0 => Some((8, 8, 0x20)),
            1 => Some((0x20, 8, 8)),
            2 => Some((0x20, 0x20, 8)),
            3 => Some((8, 0x20, 8)),
            4 => Some((0x20, 0x10, 8)),
            5 => Some((0x10, 8, 0x20)),
            _ => None,
        };
        if let Some((r, g, b)) = c {
            l2::rgb(fx, t + 0x2C, r, g, b);
        }
        let r = rmod(fx, t, 0x01E0_2AEC, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    let a = fx.mem.read_u8(t + 0x2F);
    if fx.mem.i16(t + 0x32) < 0x14 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x2F, a.wrapping_add(0x80));
        }
    } else if a < 0xB {
        fx.mem.set16(t + 0x36, 0);
    } else {
        fx.mem.write_u8(t + 0x2F, a - 9);
    }
    face(fx, t, 0, 0x01E0_2B6C);
    if fx.mem.i16(t + 0x32) != 0 {
        let r = rmod(fx, t, 0x01E0_2B8C, 5);
        add16(fx, t + 0x38, (r + 0xA) as i16 as i32);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x205580 into +0x10 (its frame saves the step's s0 = the task, s1 and
/// `ra`), then x + 0x4000.
fn face(fx: &mut Fx, t: u32, s1: u32, ra: u32) {
    crate::b494::camera_angles_saving(fx, t + 0x10, u64::from(t), u64::from(s1), ra);
    let x = fx.mem.i32(t + 0x10).wrapping_add(0x4000);
    fx.mem.set32(t + 0x10, x);
}

/// 0x1E02BE0 (D, task): a cone (size 50..129, the kind's colour 0x80 /
/// 0x10): its record's apex (x, z rand % 30 - 15, y 100) and rim centre
/// (+0x10 / +0x18 words x sine / cosine of +0x3A, y 30), spinning by the
/// word +0x14 (-5..4, 0 made 1); alpha +0x2F in by 10 for 20 updates,
/// then 0x90 / 0x70 by age parity to 250, then out by 9 (size down by 2).
fn cone(fx: &mut Fx, t: u32) {
    fx.enter(0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        let r = rmod(fx, t, 0x01E0_2C0C, 0x50);
        fx.mem.set16(t + 0x38, r + 0x32);
        kind_colour(fx, t, 0x80, 0x10);
        let r = rmod(fx, t, 0x01E0_2CDC, 0x1E);
        fx.mem.set32(t + 0x10, r + 5);
        let r = rmod(fx, t, 0x01E0_2CFC, 0xA);
        fx.mem.set32(t + 0x14, r - 5);
        let r = rmod(fx, t, 0x01E0_2D1C, 0x1E);
        fx.mem.set32(t + 0x18, r + 5);
        let r = rmod(fx, t, 0x01E0_2D3C, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0);
        fx.mem.write_u8(t + 0x2F, 0);
        let r = rmod(fx, t, 0x01E0_2D68, 0xA);
        fx.mem.set16(t + 0xE, r);
        let r = rmod(fx, t, 0x01E0_2D84, 0x1E);
        let p = rec(fx, t);
        fx.mem.setf(p, ee::from_i32(r - 0xF));
        let r = rmod(fx, t, 0x01E0_2DB8, 0x1E);
        let p = rec(fx, t);
        fx.mem.setf(p + 8, ee::from_i32(r - 0xF));
        if fx.mem.i32(t + 0x14) == 0 {
            fx.mem.set32(t + 0x14, 1);
        }
    }
    let n = fx.mem.i16(t + 0x32);
    if n < 0xFA {
        if n < 0x14 {
            let a = fx.mem.read_u8(t + 0x2F);
            if a < 0x80 {
                fx.mem.write_u8(t + 0x2F, a + 0xA);
            }
        } else {
            let v = if rem_pow(n, 2) == 1 { 0x90 } else { 0x70 };
            fx.mem.write_u8(t + 0x2F, v);
        }
    } else {
        let a = fx.mem.read_u8(t + 0x2F);
        if a < 0xB {
            fx.mem.set16(t + 0x36, 0);
        } else {
            fx.mem.write_u8(t + 0x2F, a - 9);
        }
        add16(fx, t + 0x38, -2);
        if fx.mem.i16(t + 0x38) < 0 {
            fx.mem.set16(t + 0x38, 0);
        }
    }
    let p = rec(fx, t);
    fx.mem.write_u32(p + 4, 0x42C8_0000);
    let p = rec(fx, t);
    fx.mem.write_u32(p + 0x14, 0x41F0_0000);
    let a = fx.mem.i16(t + 0x3A);
    let s = l2::sin_deg(fx, SINES, a, 0);
    let p = rec(fx, t);
    let v = ee::mul(ee::from_i32(fx.mem.i32(t + 0x10)), s);
    fx.mem.setf(p + 0x10, v);
    let a = fx.mem.i16(t + 0x3A);
    let c = l2::sin_deg(fx, SINES, a, 1);
    let p = rec(fx, t);
    let v = ee::mul(ee::from_i32(fx.mem.i32(t + 0x18)), c);
    fx.mem.setf(p + 0x18, v);
    let s = fx.mem.i16(t + 0x14);
    add16(fx, t + 0x3A, s);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02F60 (D, task): the cone's rim band (size 280, the kind's colour
/// 0x80 / 0x10), alpha 0x80 / 0x70 by age parity to 230, then out by 9.
fn cone_top(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x118);
        kind_colour(fx, t, 0x80, 0x10);
        let r = rmod(fx, t, 0x01E0_3034, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0x80);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    let n = fx.mem.i16(t + 0x32);
    if n < 0xE6 {
        let v = if rem_pow(n, 2) == 1 { 0x80 } else { 0x70 };
        fx.mem.write_u8(t + 0x2B, v);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0xB {
            fx.mem.set16(t + 0x36, 0);
        } else {
            fx.mem.write_u8(t + 0x2B, a - 9);
        }
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E030F0 (D, task): the cone's apex band (size 50, the kind's colour
/// 0xFF / 0x80), alpha 0xC0 / 0x80 by age parity to 260, then out by 9
/// (size down by 1); facing the camera turned by 0x4000.
fn cone_base(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x32);
        kind_colour(fx, t, 0xFF, 0x80);
        let r = rmod(fx, t, 0x01E0_31C4, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0x80);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    let n = fx.mem.i16(t + 0x32);
    if n < 0x104 {
        let v = if rem_pow(n, 2) == 1 { 0xC0 } else { 0x80 };
        fx.mem.write_u8(t + 0x2B, v);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0xB {
            fx.mem.set16(t + 0x36, 0);
        } else {
            fx.mem.write_u8(t + 0x2B, a - 9);
        }
        add16(fx, t + 0x38, -1);
    }
    face(fx, t, 0, 0x01E0_326C);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E032A0 / 0x1E03810 (D, task) at `at`: a point band (size 300) with
/// radius 400..599 shrinking by 20..39 (over below 0), turning by 5
/// (0x32A0, `colours`: the three-way colour) or grey 0x80 with its cell
/// stepping on odd ages (0x3810).
fn point_cd(fx: &mut Fx, t: u32, ras: [u32; 7], colours: bool) {
    fx.enter(0x20);
    let ra = |k: usize| 0x01E0_0000 + ras[k];
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x12C);
        fx.mem.set16(t + 0x34, 0);
        if colours {
            let r = rmod(fx, t, ra(0), 3);
            colour3(fx, t, r);
        } else {
            fx.rand_from(t, ra(0));
            l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        }
        let k = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, k);
        let r = rmod(fx, t, ra(1), 0xC8);
        fx.mem.set16(t + 0x30, r + 0x190);
        let r = rmod(fx, t, ra(2), 0x168);
        fx.mem.set16(t + 0x3A, r);
        let r = rmod(fx, t, ra(3), 0x14);
        fx.mem.set16(t + 0xE, r + 0x14);
        let r = rmod(fx, t, ra(4), 0x23);
        fx.mem.set16(t + 8, r);
        let r = rmod(fx, t, ra(5), 6);
        fx.mem.set16(t + 6, r + 6);
        let r = rpow(fx, t, ra(6), 2);
        fx.mem.set16(t + 0xC, r);
        fx.mem.set16(t + 0x36, 2);
    }
    fade(fx, t, 0x80, 0x2A, 0xF, 0xD);
    if colours {
        add16(fx, t + 0x34, 1);
        if fx.mem.i16(t + 0x34) >= 8 {
            fx.mem.set16(t + 0x34, 0);
        }
        if fx.mem.i16(t + 0x32) != 0 {
            turn(fx, t, 5);
        }
        if fx.mem.i16(t + 0x30) < 0 {
            fx.mem.set16(t + 0x36, 0);
        }
    } else {
        if rem_pow(fx.mem.i16(t + 0x32), 2) == 1 {
            add16(fx, t + 0x34, 1);
        }
        if fx.mem.i16(t + 0x34) >= 8 {
            fx.mem.set16(t + 0x34, 0);
        }
        if fx.mem.i16(t + 0x32) != 0 && fx.mem.i16(t + 0x30) < 0 {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let e = fx.mem.i16(t + 0xE);
        add16(fx, t + 0x30, -e);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03510 (D, task): a pushed sprite (size 60, cell 5, (0x80, 0x60,
/// 0x60)): alpha 0x80 for 7 updates then out by 15; when +0x30 (rand %
/// 360 for kind 0) is set, +8 / +0xC move by 10 x sine / cosine of +0x3A.
fn sprite(fx: &mut Fx, t: u32) {
    fx.enter(0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x3C);
        fx.mem.set16(t + 0x34, 5);
        l2::rgb(fx, t + 0x28, 0x80, 0x60, 0x60);
        if fx.mem.i16(t + 0x30) == 0 {
            let r = rmod(fx, t, 0x01E0_356C, 0x168);
            fx.mem.set16(t + 0x30, r);
        } else {
            fx.mem.set16(t + 0x30, 0);
        }
        let r = rmod(fx, t, 0x01E0_3594, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 7 {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x11 {
            fx.mem.set16(t + 0x36, 0);
        } else {
            fx.mem.write_u8(t + 0x2B, a - 0xF);
        }
    }
    if fx.mem.i16(t + 0x30) != 0 {
        for (o, cos, ra) in [(8u32, 0, 0x01E0_3630), (0xC, 1, 0x01E0_3668)] {
            let a = fx.mem.i16(t + 0x3A);
            let s = l2::sin_deg(fx, SINES, a, cos);
            let v = ee::add(ee::from_i32(fx.mem.i16(t + o)), ee::mul(10.0, s));
            let v = fix(fx, v, ra);
            fx.mem.set16(t + o, v);
        }
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E03A00 (D, task): a band ring (grey 0x60, outer size (rand % 50 +
/// 25) / 3 + rand % 20, kept in +0xA as the spread); alpha in by 10 to
/// update 0x10E, then out by 9; facing the camera turned by 0x4000.
fn band(fx: &mut Fx, t: u32) {
    fx.enter(0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        let r = rmod(fx, t, 0x01E0_3A2C, 0x46);
        fx.mem.set16(t + 0x38, r + 3);
        l2::rgb(fx, t + 0x28, 0x60, 0x60, 0x60);
        l2::rgb(fx, t + 0x2C, 0x60, 0x60, 0x60);
        let r = rmod(fx, t, 0x01E0_3A70, 0x1E);
        fx.mem.set32(t + 0x10, r + 5);
        let r = rmod(fx, t, 0x01E0_3A90, 0xA);
        fx.mem.set32(t + 0x14, r - 5);
        let r = rmod(fx, t, 0x01E0_3AB0, 0x1E);
        fx.mem.set32(t + 0x18, r + 5);
        let r = rmod(fx, t, 0x01E0_3AD0, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        let r = rmod(fx, t, 0x01E0_3AF4, 0x32);
        fx.mem.set16(t + 0xA, r + 0x19);
        let a = fx.mem.i16(t + 0xA);
        fx.mult(0x5555_5556, a);
        let hi = fx.hilo().1;
        fx.mem.set16(t + 0x38, hi.wrapping_add(((a as u32) >> 31) as i32));
        let r = rmod(fx, t, 0x01E0_3B3C, 0x14);
        let v = fx.mem.i16(t + 0x38) + (r as i16 as i32);
        fx.mem.set16(t + 0x38, v);
        let r = rmod(fx, t, 0x01E0_3B68, 0xA);
        fx.mem.set16(t + 0xE, r);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(COUNTER) < 0x10E {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x2B, a + 0xA);
        }
    } else if a < 0xB {
        fx.mem.set16(t + 0x36, 0);
    } else {
        fx.mem.write_u8(t + 0x2B, a - 9);
    }
    face(fx, t, D, 0x01E0_3BD8);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E03ED0 (D, task): camera 0's slots 4 / 5 (0x1F7E20) from D+0x34A0:
/// (128, 8, 0, 0), the point node 9 + 2 x (node 10 - node 9), (0, 120, 0),
/// the weight +0x28 up by 0.1 to 1 and back down by 0.1 (over at 300 once
/// at most 0.1); the slots cleared when over; also over on timer 1.
fn camera(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x60);
    node_to(fx, 0xA, sp + 0x40, u64::from(D), u64::from(t), 0x01E0_3F00);
    node_to(fx, 0xC, sp + 0x50, u64::from(sp + 0x50), u64::from(t), 0x01E0_3F1C);
    if fx.mem.i16(t + 0x36) == 1 {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x4100_0000), (8, 0), (0xC, 0), (0x20, 0), (0x24, 0x42F0_0000), (0x28, 0)] {
            fx.mem.write_u32(CAM + o, v);
        }
        fx.mem.set16(t + 0x36, 2);
        fx.mem.set16(t + 0x30, 0);
    }
    for k in 0..3u32 {
        let a = fx.mem.f(sp + 0x40 + 4 * k);
        let b = fx.mem.f(sp + 0x50 + 4 * k);
        let v = ee::add(a, ee::mul(2.0, ee::sub(b, a)));
        fx.mem.setf(CAM + 0x10 + 4 * k, v);
    }
    let tenth = f32::from_bits(0x3DCC_CCCD);
    let w = fx.mem.f(CAM + 0x28);
    if fx.mem.i16(t + 0x32) < 0x12C {
        if fx.mem.i16(t + 0x30) == 0 {
            if w < 1.0 {
                fx.mem.setf(CAM + 0x28, ee::add(w, tenth));
            } else {
                fx.mem.set16(t + 0x30, 1);
            }
        } else if w <= tenth {
            fx.mem.set16(t + 0x30, 0);
        } else {
            fx.mem.setf(CAM + 0x28, ee::sub(w, tenth));
        }
    } else if w <= tenth {
        fx.mem.set16(t + 0x36, 0);
    } else {
        fx.mem.setf(CAM + 0x28, ee::sub(w, tenth));
    }
    let (p, ra) = if fx.mem.i16(t + 0x36) == 0 { (0u32, 0x01E0_40A8u32) } else { (CAM, 0x01E0_40BCu32) };
    crate::blib::exe::camera_slots45(fx, p);
    fx.saved(sp - 0x20, &[(0, u64::from(sp + 0x50)), (0x10, u64::from(ra))]);
    if fx.mem.i32(TIMER) == 1 {
        fx.mem.set16(t + 0x36, 0);
    }
    age(fx, t);
    fx.leave(0x60);
}

/// 0x1E04100 (D, task): the stage light +0x30 (0x80 at first): down by 10
/// to 0x30 while the timer is above 30, then up by 10 to 0x80; 0x1F7F10
/// every update (0x80 on timer 1).
fn stage(fx: &mut Fx, t: u32) {
    fx.enter(0x10);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x30, 0x80);
        fx.mem.set16(t + 0x36, 2);
    }
    let v = fx.mem.i16(t + 0x30);
    if fx.mem.i32(TIMER) >= 0x1F {
        fx.mem.set16(t + 0x30, if v < 0x31 { 0x30 } else { v - 0xA });
    } else {
        fx.mem.set16(t + 0x30, if v < 0x80 { v + 0xA } else { 0x80 });
    }
    if fx.mem.i32(TIMER) == 1 {
        fx.set_stage_light(0x80);
    } else {
        let l = fx.mem.read_u8(t + 0x30);
        fx.set_stage_light(i32::from(l));
    }
    fx.leave(0x10);
}

/// 0x1E01050 `ring_points(D, task, mode, table)` on the 0x40 layout:
/// `billboard::ring_points` with the colour +0x28, the turn +0x3A, the
/// size i16 +0x38, the radius i16 +0x30; depth 0 submits only when the
/// mean w is above 20, keyed by it (0x1A2C70 from 0x1E01658) [F fdiff vs
/// b363's 0x1E01160: the only differences].
fn ring_points40(fx: &mut Fx, t: u32, mode: i32, table: u32) {
    let d0 = D;
    let sp = fx.enter(0x220);
    let d = sp + 0x188;
    desc_uv4(fx, d);
    fx.desc_alpha(d, blend_alpha((mode >> 4) as i16 as i32));
    rgbaq_at(fx, sp + 0x218, t + 0x28, d + 0x10);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0x100 + 4 * k, 0x3DCC_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0x160);
    to_degrees_neg(fx, sp + 0x164);
    to_degrees_neg(fx, sp + 0x160);
    let a = fx.mem.i16(t + 0x3A);
    fx.mem.set32(sp + 0x174, a);
    for k in 0..3 {
        let w = fx.mem.read_u32(d0 + 0x20 + 4 * k);
        fx.mem.write_u32(sp + 0xF0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x120, sp + 0x100, 0, sp + 0xF0);
    let order = fx.mem.i16(d0 + 0xC0);
    uv::uv_rect(fx, d, d0 + 0x94, order);
    let mut dist = fx.camera_distance();
    if dist == 0.0 {
        dist = 400.0;
    }
    let depth = mode & 0xF;
    let tab = d0 + 0xD8;
    let mut k = 0i32;
    while k < fx.mem.i16(d0 + 0xB8) {
        let mut size = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0xA0, 0.0, 0.0);
        for (j, off) in [0u32, 100, 180].into_iter().enumerate() {
            let i = fx.mem.i16(t + 8);
            let b = fx.mem.i8(table.wrapping_add(k as u32).wrapping_add(i as u32).wrapping_add(off));
            let r = ee::from_i32(fx.mem.i16(t + 0x30) + (k >> 1));
            let f = ee::mul(ee::div(ee::from_i32(b), 20.0), r);
            fx.mem.setf(sp + 0xE0 + 4 * j as u32, f);
        }
        l2::rotate_deg(fx, sp + 0xE0, sp + 0x170, 0, tab);
        for axis in 0..3 {
            let f = fx.mem.f(sp + 0xE0 + 4 * axis as u32);
            l2::add_column(fx, sp + 0xA0, axis, f);
        }
        let ot = fx.mem.read_u32(d0 + 0x80);
        if depth <= 2 && l2::project(fx, sp + 0xA0, SPR, sp + 0x120, d, sp + 0x110) {
            match depth {
                0 => {
                    let w = ee::div(fx.mem.f(sp + 0x110), ee::div(dist, 10.0));
                    size = ee::div(size, w);
                    for (o, neg) in [(0x20u32, true), (0x22, false), (0x30, false), (0x32, false), (0x40, true), (0x42, true), (0x50, false), (0x52, true)] {
                        let v = ee::from_i32(i32::from(fx.mem.read_u16(d + o)));
                        let x = if neg { ee::sub(v, size) } else { ee::add(v, size) };
                        fx.mem.write_u16(d + o, ee::fixunssfsi(x) as u16);
                    }
                    let mw = fx.mem.f(sp + 0x110);
                    if mw > 20.0 {
                        fx.fix_residue(mw, 0x01E0_1658);
                        let key = ee::fixunssfsi(mw) as i32;
                        fx.submit(d, ot, key);
                    }
                }
                1 => {
                    l2::far_z(fx, &[d + 0x50, d + 0x40, d + 0x30, d + 0x20]);
                    fx.submit(d, ot, 0);
                }
                _ => fx.submit(d, ot, -1),
            }
        }
        k = (k + 1) as i16 as i32;
    }
    fx.leave(0x220);
}

/// 0x1E05340 (D, task, 0, pos): the cone: 18 untextured gouraud strips
/// (20 degrees each from +0x3A) from the apex D+0x20 / D+0x28 (radius 0,
/// height D+0x24) to the rim D+0x30 / D+0x38 + (i16 +0x38 / 10) x cosine
/// / sine (height D+0x34); colours +0x28 (apex) / +0x2C (rim); projected
/// through the camera (its 0x1097D0 matrix unused), keyed by the mean w.
fn cone_draw(fx: &mut Fx, t: u32, pos: u32) {
    let sp = fx.enter(0x1E0);
    copy4(fx, sp + 0xA0, pos);
    let d = sp + 0x150;
    l2::colour_strip4(fx, d, t + 0x28, 2);
    l2::colour_strip4(fx, d, t + 0x2C, 1);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    copy3(fx, sp + 0xF0, sp + 0xA0);
    l2::xform_vp(fx, sp + 0x100, 0, 0, sp + 0xF0);
    let (y0, y1) = (fx.mem.read_u32(D + 0x24), fx.mem.read_u32(D + 0x34));
    for (o, v) in [(0xB4u32, y0), (0xC4, y0), (0xD4, y1), (0xE4, y1)] {
        fx.mem.write_u32(sp + o, v);
    }
    let r = ee::div(ee::from_i32(fx.mem.i16(t + 0x38)), 10.0);
    let tab = D + 0xD8;
    let mut s2 = 0i32;
    for _ in 0..0x12 {
        let s4 = s2 as i16 as i32;
        let a = (fx.mem.i16(t + 0x3A) + s4) as i16 as i32;
        let e = (s4 + (fx.mem.i16(t + 0x3A) + 0x14)) as i16 as i32;
        for (deg, x, z) in [(a, 0xB0u32, 0xD0u32), (e, 0xC0, 0xE0)] {
            let c = l2::sin_deg(fx, tab, deg, 1);
            let v = ee::add(fx.mem.f(D + 0x20), ee::mul(0.0, c));
            fx.mem.setf(sp + x, v);
            let s = l2::sin_deg(fx, tab, deg, 0);
            let v = ee::add(fx.mem.f(D + 0x28), ee::mul(0.0, s));
            fx.mem.setf(sp + x + 8, v);
            let c = l2::sin_deg(fx, tab, deg, 1);
            let v = ee::add(fx.mem.f(D + 0x30), ee::mul(r, c));
            fx.mem.setf(sp + z, v);
            let s = l2::sin_deg(fx, tab, deg, 0);
            let v = ee::add(fx.mem.f(D + 0x38), ee::mul(r, s));
            fx.mem.setf(sp + z + 8, v);
        }
        if l2::project(fx, sp + 0xB0, SPR, 0, d, sp + 0x140) {
            let mw = fx.mem.f(sp + 0x140);
            fx.fix_residue(mw, 0x01E0_55BC);
            let key = ee::fixunssfsi(mw) as i32;
            let ot = fx.mem.read_u32(D + 0x80);
            fx.submit(d, ot, key);
        }
        s2 = (s2 + 0x14) as i16 as i32;
    }
    fx.leave(0x1E0);
}

/// 0x1E056A0 (D, task, 0, pos): b582's band ring (0x1E05460) with inner
/// radius 0 and outer i16 +0x38 / 10, at the far Z (0x109C60 kind 1) and
/// key 0 [F fdiff: the only differences].
fn band_draw(fx: &mut Fx, t: u32, pos: u32) {
    let sp = fx.enter(0x1D0);
    copy4(fx, sp + 0x90, pos);
    let d = sp + 0x140;
    l2::colour_strip4(fx, d, t + 0x28, 2);
    l2::colour_strip4(fx, d, t + 0x2C, 1);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    copy3(fx, sp + 0xE0, sp + 0x90);
    for k in 0..3u32 {
        let v = ee::add(fx.mem.f(sp + 0xE0 + 4 * k), fx.mem.f(D + 0x20 + 4 * k));
        fx.mem.setf(sp + 0xE0 + 4 * k, v);
    }
    l2::xform_vp(fx, sp + 0xF0, 0, t + 0x10, sp + 0xE0);
    for o in [0xA4u32, 0xB4, 0xC4, 0xD4] {
        fx.mem.set32(sp + o, 0);
    }
    let outer = ee::div(ee::from_i32(fx.mem.i16(t + 0x38)), 10.0);
    let inner = 0.0f32;
    let mut a = 0i32;
    for _ in 0..0x12 {
        let s = (fx.mem.i16(t + 0x3A) + a) as i16 as i32;
        let e = (a + (fx.mem.i16(t + 0x3A) + 0x14)) as i16 as i32;
        for (deg, v0, v1) in [(s, 0xA0u32, 0xC0u32), (e, 0xB0, 0xD0)] {
            for (r, v) in [(inner, v0), (outer, v1)] {
                let c = l2::sin_deg(fx, D + 0xD8, deg, 1);
                fx.mem.setf(sp + v, ee::mul(r, c));
                let sn = l2::sin_deg(fx, D + 0xD8, deg, 0);
                fx.mem.setf(sp + v + 8, ee::mul(r, sn));
            }
        }
        if l2::project(fx, sp + 0xA0, SPR, sp + 0xF0, d, sp + 0x130) {
            l2::far_z_desc(fx, d, 1);
            let ot = fx.mem.read_u32(D + 0x80);
            fx.submit(d, ot, 0);
        }
        a = (a + 0x14) as i16 as i32;
    }
    fx.leave(0x1D0);
}
