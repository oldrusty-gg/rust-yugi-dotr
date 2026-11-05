//! BMODULE b120 (card 235), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02890, data area 0x1ED0000, 0xAF30 bytes) [F]
//!: b064's task-pool shape (card 232, b064.rs)
//! shared with card 416 (b498, unported): 150 tasks of 0x40 bytes at
//! D+0x5E0 (alive +0x36, step +0x24 as a record address compared by the
//! entry, `jal`s), spawned by 0x1E034D0 (colour D+0xAC74, kind D+0xACB4 to
//! +0x30, angle D+0xACAE to +0x3A, the point D+0x2D0, the argument to
//! +0x20). Nodes: the attacker's 0x1F (clear) / 3 / 0xD / 0xC / 0x1C at
//! D+0xAC9C..; textures (4, 2) D+0x328, (4, 0xD..0xF) D+0x354.., (5, 0)
//! D+0xAC50; ten 5 x 50 quads at D+0x360 (the first four tilted 15 / 25 by
//! sines of 45 k and shifted); jitter bytes D+0xACC0; the yaw D+0xACB8.
//!
//! Only six steps are ever spawned (the entry also tests 0x1E01450 /
//! 0x1650 / 0x16D0 / 0x1890 / 0x1940 / 0x1EA0 and their draws 0x1E03650 /
//! 0x3990 / 0x3B30 / 0x40A0: never reached, not ported). The emitter
//! 0x1E00B70 counts D+0xACBC: on 0x2E two wings (1.2 / -1.2 from node 0x1F,
//! 2 up, z by the yaw's cosine) each a glow 0x1E01F60 (`fan18_far` + a
//! core quad 0x1E044A0) and two rings 0x1E02750 (`quarters_far_at`); from
//! 0x2E the wing points D+0x20 / D+0x30 follow the node; 0x65..0xDB on c %
//! 3 = 1 a violet rising quad band 0x1E02040 (draw 0x1E046E0, 15 jittered
//! quads); 0x65..0xF9 a column 0x1E01BE0 (draw 0x1E03DF0, D+0x360's quads
//! through D+0x290) that throws grey motes 0x1E01240 (b064's drift; draw
//! 0x1E037F0 = b064's sprite_a on D+0x328 + `sprite_spec`); 0x73..0xDB up
//! to fifty ribbons 0x1E021D0 (two 20-point histories D+0x2C30 / D+0x2D70
//! + 0x280 i about the attacker's point D+0xA930 + 0x10 i, draw 0x1E04DB0;
//! spawned at the attacker's 0x1F7370 rotation words read as floats, so
//! its i16 point is (0, 0, 0) for the usual yaws: sic).
//! Returns 1 on update 0x76, 4 on 0x109, 6 on 0x81..0xEF, 2 on 0xF5, 8
//! once the 0x118-update timer D+0xAC70 has run out.
use crate::blib::{quads4, spawn as bspawn, spline, uv};
use crate::bus::Bus;
use crate::c061::{blend_alpha, desc_uv4, rgbaq_at};
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 150;
const POOL: u32 = D + 0x5E0;
const POS: u32 = D + 0x2D0;
const QUADS: u32 = D + 0x360;
const PTS: u32 = D + 0xA930;
const TEX5: u32 = D + 0xAC50;
const TIMER: u32 = D + 0xAC70;
const COLOUR: u32 = D + 0xAC74;
const OTW: u32 = D + 0xAC7C;
const MEAN: u32 = D + 0xAC8C;
const NODES: u32 = D + 0xAC9C;
const ANGLE: u32 = D + 0xACAE;
const KIND: u32 = D + 0xACB4;
const RIBBONS: u32 = D + 0xACB6;
const YAW: u32 = D + 0xACB8;
const PARAM: u32 = D + 0xACBA;
const COUNTER: u32 = D + 0xACBC;
const BYTES: u32 = D + 0xACC0;
const TAB: u32 = D + 0xADC0;

const MOTE: u32 = 0x01E0_1240;
const COLUMN: u32 = 0x01E0_1BE0;
const GLOW: u32 = 0x01E0_1F60;
const BAND: u32 = 0x01E0_2040;
const RIBBON: u32 = 0x01E0_21D0;
const RING: u32 = 0x01E0_2750;

pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x180);
    fx.mem.write_u32(OTW, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x180);
    ret
}

/// rand % m (HI) from return address `ra` (rand's frame: `s0`).
fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// 0x1A2B80 on `v` from return address `ra` (its residue).
fn fix(fx: &mut Fx, v: f32, ra: u32) -> i32 {
    fx.fix_residue(v, ra);
    ee::fixsfsi(v)
}

/// 0x1A2C70 on the float at `w` from `ra` (its residue): the key.
fn key(fx: &mut Fx, w: u32, ra: u32) -> i32 {
    let v = fx.mem.f(w);
    fx.fix_residue(v, ra);
    ee::fixunssfsi(v) as i32
}

/// `a` / 180 by the 0xB60B60B7 magic multiply.
fn div180(fx: &mut Fx, a: i32) -> i32 {
    fx.mult(0xB60B_60B7u32 as i32, a);
    let hi = fx.hilo().1;
    (hi.wrapping_add(a) >> 7).wrapping_add(((a as u32) >> 31) as i32)
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xAF30);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(PARAM, param);
    fx.mem.set32(TIMER, 0x118);
    for (k, (node, clear)) in [(0x1F, 1), (3, 0), (0xD, 0), (0xC, 0), (0x1C, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    for k in 0..10u32 {
        l2::quad_xy(fx, QUADS + 0x40 * k, 5.0, 50.0);
    }
    for k in 0..4i32 {
        let q = QUADS + 0x40 * k as u32;
        let (a, b) = (45 * k, 45 + 45 * k);
        let s = l2::sin_deg(fx, TAB, a, 0);
        fx.mem.setf(q + 4, ee::mul(15.0, s));
        let s = l2::sin_deg(fx, TAB, b, 0);
        fx.mem.setf(q + 0x14, ee::mul(15.0, s));
        let s = l2::sin_deg(fx, TAB, a, 0);
        fx.mem.setf(q + 0x24, ee::mul(25.0, s));
        let s = l2::sin_deg(fx, TAB, b, 0);
        fx.mem.setf(q + 0x34, ee::mul(25.0, s));
        let dx = ee::from_i32(-8 + 4 * k);
        for o in [0u32, 0x10, 0x20, 0x30] {
            let x = ee::add(fx.mem.f(q + o), dx);
            fx.mem.setf(q + o, x);
        }
        for o in [4u32, 0x14, 0x24, 0x34] {
            let y = ee::sub(fx.mem.f(q + o), 10.0);
            fx.mem.setf(q + o, y);
        }
    }
    for k in 0xDu32..0x10 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x354 + 4 * (k - 0xD), t);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x328, t);
    let t = fx.texture(5, 0);
    fx.mem.write_u32(TEX5, t);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xD0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xD4)), 180.0);
    let y = fix(fx, y, 0x01E0_2BC8);
    fx.mem.set16(YAW, y);
    for k in 0x64..0xB4u32 {
        let r = fx.rand_from(D, 0x01E0_2BE0);
        fx.mem.set8(BYTES + k, r % 8 - 4);
    }
    for k in 0..0x64u32 {
        let r = rmod(fx, D, 0x01E0_2C20, 10);
        fx.mem.set8(BYTES + k, r - 5);
    }
    for k in 0xB4..0x100u32 {
        let r = rmod(fx, D, 0x01E0_2C58, 0x14);
        fx.mem.set8(BYTES + k, -(r + 4));
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
        let (k64, row) = (u64::from(k), u64::from(D + 0x40 * k));
        let f = fx.sp;
        match s {
            MOTE => {
                fx.saved(f - 0x30, &[(0x20, 0x01E0_2CD0), (0x10, 0), (0, u64::from(D))]);
                fx.enter(0x30);
                mote(fx, t);
                fx.leave(0x30);
            }
            COLUMN => {
                fx.saved(f - 0x60, &[(0x40, 0x01E0_2E68), (0x30, k64), (0x20, row), (0x10, 0), (0, u64::from(D))]);
                let s = fx.enter(0x60);
                column(fx, t, s);
                fx.leave(0x60);
            }
            GLOW => {
                fx.saved(f - 0x20, &[(0x10, 0x01E0_2F38), (0, u64::from(D))]);
                fx.enter(0x20);
                glow(fx, t);
                fx.leave(0x20);
            }
            BAND => {
                fx.saved(f - 0x30, &[(0x20, 0x01E0_2F58), (0x10, 0), (0, u64::from(D))]);
                fx.enter(0x30);
                band(fx, t);
                fx.leave(0x30);
            }
            RIBBON => {
                fx.saved(f - 0x60, &[(0x40, 0x01E0_2F78), (0x30, row), (0x20, 0), (0x10, u64::from(D))]);
                let s = fx.enter(0x60);
                ribbon(fx, t, s);
                fx.leave(0x60);
            }
            RING => {
                fx.saved(f - 0x20, &[(0x10, 0x01E0_2F98), (0, u64::from(D))]);
                fx.enter(0x20);
                ring(fx, t);
                fx.leave(0x20);
            }
            _ => {}
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x70);
    let z = ee::sub(fx.mem.f(sp + 0x78), 25.0);
    fx.mem.setf(sp + 0x78, z);
    let mut count = 0u32;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        count += 1;
        draw(fx, sp, t, k, count);
    }
    let n = fx.mem.i32(TIMER);
    if n != 0 {
        fx.mem.set32(TIMER, n - 1);
    }
    let c = fx.mem.i16(COUNTER);
    if c == 0x76 {
        1
    } else if c == 0x109 {
        4
    } else if (0x81..0xF0).contains(&c) {
        6
    } else if c == 0xF5 {
        2
    } else if fx.mem.i32(TIMER) > 0 {
        0
    } else {
        8
    }
}

/// The entry's draws of one live task (index `k`, `count` the live tasks
/// so far: the entry's s3 / s1; s2 = D + 0x40 k).
fn draw(fx: &mut Fx, sp: u32, t: u32, k: u32, count: u32) {
    let s = fx.mem.read_u32(t + 0x24);
    let d = sp + 0xF0;
    let regs = [u64::from(D), u64::from(count), u64::from(D + 0x40 * k), u64::from(k)];
    // The callee's saved ra (at `rao`) and s0..s(n-1) from `at`.
    let saves = |fx: &mut Fx, size: u32, ra: u32, rao: u32, at: u32, n: usize| {
        let f = fx.sp - size;
        let mut v: Vec<(u32, u64)> = vec![(rao, u64::from(ra))];
        for (i, &r) in regs.iter().enumerate().take(n) {
            v.push((at + 0x10 * i as u32, r));
        }
        fx.saved(f, &v);
    };
    let rect_d = |fx: &mut Fx, local: u32, id: i32, sub: i32| {
        l2::rect(fx, sp + local, id, sub);
        copy4(fx, D + 0x94, sp + local);
    };
    match s {
        MOTE => {
            for (mode, ra) in [(0, 0x01E0_300C), (1, 0x01E0_3020)] {
                saves(fx, 0x110, ra, 0x50, 0, 4);
                sprite(fx, t, d, mode);
            }
            let w = fx.mem.read_u32(D + 0x328);
            fx.mem.write_u32(sp + 0x80, w);
            fx.mem.set32(sp + 0x88, 0);
            fx.mem.set32(sp + 0x84, 0x14);
            fx.mem.set32(sp + 0x8C, 0);
            let ot = fx.mem.read_u32(OTW);
            quads4::sprite_spec(fx, ot, t, sp + 0x80);
        }
        COLUMN => {
            for (mode, ra) in [(0, 0x01E0_3178), (1, 0x01E0_318C)] {
                saves(fx, 0x100, ra, 0x50, 0, 4);
                column_draw(fx, t, d, mode);
            }
        }
        GLOW => {
            let w = fx.mem.read_u32(TEX5);
            fx.mem.write_u32(D + 0x88, w);
            let c = fx.mem.i16(t + 0x34);
            rect_d(fx, 0x90, 0x14, c);
            fx.mem.set16(D + 0xBE, 0);
            quads4::fan18_far(fx, D, t, d);
            saves(fx, 0x110, 0x01E0_3258, 0x30, 0, 3);
            core(fx, t, d);
        }
        BAND => {
            fx.mem.set16(D + 0xB8, 0xF);
            let w = fx.mem.read_u32(D + 0x35C);
            fx.mem.write_u32(D + 0x88, w);
            let c = fx.mem.i16(t + 0x34);
            rect_d(fx, 0xA0, 0x9B, c);
            saves(fx, 0x140, 0x01E0_32CC, 0x60, 0, 4);
            band_draw(fx, t, d);
            fx.mem.set16(D + 0xB8, 0);
        }
        RIBBON => {
            let w = fx.mem.read_u32(D + 0x35C);
            fx.mem.write_u32(D + 0x88, w);
            let c = fx.mem.i16(t + 0x34);
            rect_d(fx, 0xB0, 0x97, c);
            let h = fx.mem.i16(D + 0x9A) >> 1;
            fx.mem.set16(D + 0x9A, h);
            let u = fx.mem.i16(D + 0x96) + fx.mem.i16(D + 0x9A);
            fx.mem.set16(D + 0x96, u);
            let me = fx.self_handle();
            fx.unit_position(me, sp + 0x70);
            fx.mem.set16(D + 0xBE, 1);
            let i = fx.mem.i16(t + 0x30);
            let base = (D as i32 + 0x280 * i) as u32;
            saves(fx, 0x340, 0x01E0_3380, 0xA0, 0x10, 4);
            ribbon_draw(fx, t, base + 0x2C30, base + 0x2D70);
        }
        RING => {
            let w = fx.mem.read_u32(D + 0x35C);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xBE, 0);
            rect_d(fx, 0xC0, 0x9A, 1);
            quads4::quarters_far_at(fx, D, t, d);
        }
        _ => {}
    }
}

/// 0x1E034D0(D, step, arg): the first free task (bzero 0x40, alive 1)
/// takes the colour D+0xAC74 (+0x28 / +0x2C, alpha +0x2B / +0x2F), the
/// point D+0x2D0 as i16, kind D+0xACB4 (+0x30), angle D+0xACAE (+0x3A),
/// `arg` (+0x20) and the step (+0x24).
fn spawn(fx: &mut Fx, step: u32, arg: u32) {
    fx.enter(0x60);
    let mut task = None;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            task = Some(t);
            break;
        }
    }
    if let Some(t) = task {
        for at in [0x28u32, 0x2C] {
            let c: [i32; 3] = std::array::from_fn(|k| i32::from(fx.mem.read_u8(COLOUR + k as u32)));
            l2::rgb(fx, t + at, c[0], c[1], c[2]);
            let a = fx.mem.read_u8(COLOUR + 3);
            fx.mem.write_u8(t + at + 3, a);
        }
        l2::to_i16x4(fx, t, POS, 0);
        let i = fx.mem.i16(KIND);
        fx.mem.set16(t + 0x30, i);
        let a = fx.mem.i16(ANGLE);
        fx.mem.set16(t + 0x3A, a);
        fx.mem.write_u32(t + 0x20, arg);
        fx.mem.write_u32(t + 0x24, step);
    }
    fx.leave(0x60);
}

fn set_pos(fx: &mut Fx, x: f32, y: f32, z: f32) {
    fx.mem.setf(POS, x);
    fx.mem.setf(POS + 4, y);
    fx.mem.setf(POS + 8, z);
}

/// 0x1E00B70(D, 0): the emitter (frame 0x80).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x80);
    fx.saved(sp, &[(0x50, 0x01E0_2C9C), (0x40, 0), (0x30, 0), (0x20, param as i64 as u64), (0x10, 0), (0, u64::from(D))]);
    l2::rgb(fx, COLOUR, 0x80, 0, 0);
    fx.mem.write_u8(COLOUR + 3, 0);
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    let h = fx.self_handle();
    let n = fx.mem.i16(NODES);
    fx.node_position(h, n, sp + 0x70);
    if fx.mem.i16(COUNTER) == 0x2E {
        let h = fx.self_handle();
        fx.unit_rotation(h, sp + 0x60);
        let a0 = fx.mem.i32(sp + 0x64);
        let a = div180(fx, a0);
        let c = l2::sin_deg(fx, TAB, a as i16 as i32, 1);
        let s3 = fix(fx, ee::mul(-1.0, c), 0x01E0_0C74) as i16 as i32;
        fx.mem.set16(KIND, 0xF);
        let r = rmod(fx, 0, 0x01E0_0C98, 0x168);
        fx.mem.set16(ANGLE, r);
        fx.mem.set16(KIND, 0);
        let (px, py, pz) = (fx.mem.f(sp + 0x70), fx.mem.f(sp + 0x74), fx.mem.f(sp + 0x78));
        let dz = ee::from_i32(s3);
        let wing = |fx: &mut Fx, x: f32, at: u32| {
            let (y, z) = (ee::add(2.0, py), ee::add(pz, dz));
            fx.mem.setf(D + at, x);
            fx.mem.setf(D + at + 4, y);
            fx.mem.setf(D + at + 8, z);
            set_pos(fx, x, y, z);
            spawn(fx, GLOW, 0);
            fx.mem.set16(ANGLE, 0);
            spawn(fx, RING, 0);
            fx.mem.set16(ANGLE, 0x5A);
            spawn(fx, RING, 0);
        };
        wing(fx, (1.2f64 + f64::from(px)) as f32, 0x20);
        let r = rmod(fx, 0, 0x01E0_0D90, 0x168);
        fx.mem.set16(ANGLE, r);
        fx.mem.set16(KIND, 1);
        wing(fx, (f64::from(px) - 1.2f64) as f32, 0x30);
        let k = fx.mem.i16(KIND) + 0xF;
        fx.mem.set16(KIND, k);
    }
    if fx.mem.i16(COUNTER) >= 0x2E {
        let h = fx.self_handle();
        fx.unit_rotation(h, sp + 0x60);
        let a0 = fx.mem.i32(sp + 0x64);
        let a = div180(fx, a0);
        l2::sin_deg(fx, TAB, a as i16 as i32, 1);
        let (px, py, pz) = (fx.mem.f(sp + 0x70), fx.mem.f(sp + 0x74), fx.mem.f(sp + 0x78));
        fx.mem.setf(D + 0x20, (0.9f64 + f64::from(px)) as f32);
        fx.mem.setf(D + 0x24, (2.3f64 + f64::from(py)) as f32);
        fx.mem.setf(D + 0x28, pz);
        fx.mem.setf(D + 0x30, (f64::from(px) - 1.1f64) as f32);
        fx.mem.setf(D + 0x34, (2.3f64 + f64::from(py)) as f32);
        fx.mem.setf(D + 0x38, pz);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x65..0xDC).contains(&c) && fx.div(c, 3).1 == 1 {
        let (px, py, pz) = (fx.mem.f(sp + 0x70), fx.mem.f(sp + 0x74), fx.mem.f(sp + 0x78));
        set_pos(fx, px, ee::add(2.0, py), pz);
        fx.mem.set16(KIND, 1);
        spawn(fx, BAND, 0);
    }
    let c = fx.mem.i16(COUNTER);
    let r = fx.div(c, 3).1;
    if c >= 0x65 && r == 1 && c < 0xFA {
        let (px, py, pz) = (fx.mem.f(sp + 0x70), fx.mem.f(sp + 0x74), fx.mem.f(sp + 0x78));
        set_pos(fx, px, py, pz);
        fx.mem.set16(KIND, 1);
        fx.mem.setf(POS + 4, ee::sub(py, 10.0));
        fx.mem.set16(KIND, 0);
        spawn(fx, COLUMN, 0);
    }
    let c = fx.mem.i16(COUNTER);
    if fx.div(c, 3).1 == 1 && (0x73..0xDC).contains(&c) {
        if fx.mem.i16(RIBBONS) < 0x32 {
            let h = fx.self_handle();
            // 0x1F7370: the rotation words, read back as floats (sic).
            fx.unit_rotation(h, sp + 0x70);
            let (px, py, pz) = (fx.mem.f(sp + 0x70), fx.mem.f(sp + 0x74), fx.mem.f(sp + 0x78));
            set_pos(fx, px, py, pz);
            let n = fx.mem.i16(RIBBONS);
            fx.mem.set16(KIND, n);
            spawn(fx, RIBBON, 0);
        }
        let n = fx.mem.i16(RIBBONS) + 1;
        fx.mem.set16(RIBBONS, n);
        if fx.mem.i16(RIBBONS) >= 0x32 {
            fx.mem.set16(RIBBONS, 0);
        }
    }
    fx.leave(0x80);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// Alpha +0x2B down by `down` while at least `floor`, else the task ends.
fn fade_out(fx: &mut Fx, t: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a >= floor {
        fx.mem.write_u8(t + 0x2B, a.wrapping_sub(down));
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
}

fn add_alpha(fx: &mut Fx, t: u32, at: u32, up: u8) {
    let a = fx.mem.read_u8(t + at);
    fx.mem.write_u8(t + at, a.wrapping_add(up));
}

/// i16 `at` += `k` x (sine or cosine of i16 `deg`) through 0x1A2B80.
fn wobble(fx: &mut Fx, t: u32, at: u32, deg: u32, cos: i32, k: f32, ra: u32) {
    let a = fx.mem.i16(t + deg);
    let s = l2::sin_deg(fx, TAB, a, cos);
    let f = ee::add(ee::from_i32(fx.mem.i16(t + at)), ee::mul(k, s));
    let r = fix(fx, f, ra);
    fx.mem.set16(t + at, r);
}

/// 0x1E01240 (D, task): b064's drifting mote in grey (0x10 x 3), size 5
/// growing to 0x12 on odd ages, a random cell 0..6.
fn mote(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x10, 0x10, 0x10);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 5);
        let r = rmod(fx, t, 0x01E0_1288, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let r = rmod(fx, t, 0x01E0_12A4, 7);
        fx.mem.set16(t + 0x34, r);
        let r = rmod(fx, t, 0x01E0_12C0, 0x1E);
        fx.mem.set16(t + 0x30, r - 0xF);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0xA {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x60 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(9));
        }
    } else {
        fade_out(fx, t, 0x11, 0xA);
    }
    age(fx, t);
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let s = fx.mem.i16(t + 0x38);
        if s < 0x12 {
            fx.mem.set16(t + 0x38, s + 1);
        }
    }
    wobble(fx, t, 8, 0x3A, 0, 10.0, 0x01E0_13B0);
    wobble(fx, t, 0xA, 0x30, 1, 4.0, 0x01E0_13EC);
    wobble(fx, t, 0xC, 0x3A, 1, 10.0, 0x01E0_1428);
}

/// 0x1E01BE0 (D, task): the column (frame `sp`): violet for kinds 0 / 1,
/// else grey; grows +0x38 by 2 (fading by 0x10 from 0x51, under timer 0x1F
/// or from age 0x1E); its kind +0x30 counts up, its z sways by 7 x the
/// yaw's cosine; every fourth update two motes 0x1E01240 about it.
fn column(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x30) < 2 {
        l2::rgb(fx, t + 0x28, 0x40, 8, 0x80);
    } else {
        l2::rgb(fx, t + 0x28, 0x20, 0x10, 0x20);
    }
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set32(t + 0x10, -0x4000);
        fx.mem.set32(t + 0x14, 0);
        fx.mem.set32(t + 0x18, 0);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x38, 2);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0x80);
    }
    if fx.mem.i32(TIMER) < 0x1F || fx.mem.i16(t + 0x32) >= 0x1E || fx.mem.i16(t + 0x38) >= 0x51 {
        fade_out(fx, t, 0x11, 0x10);
    }
    let g = fx.mem.i16(t + 0x38) + 2;
    fx.mem.set16(t + 0x38, g);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x50);
    let k = fx.mem.i16(t + 0x30) + 1;
    fx.mem.set16(t + 0x30, k);
    let a0 = fx.mem.i32(sp + 0x54);
        let a = div180(fx, a0);
    let c = l2::sin_deg(fx, TAB, a as i16 as i32, 1);
    let f = ee::add(ee::from_i32(fx.mem.i16(t + 4)), ee::mul(7.0, c));
    let z = fix(fx, f, 0x01E0_1D7C);
    fx.mem.set16(t + 4, z);
    age(fx, t);
    let a = fx.mem.i16(t + 0x3A) + 0xC;
    fx.mem.set16(t + 0x3A, a);
    if fx.mem.i16(t + 0x32) % 4 == 1 {
        for s0 in 0..2u32 {
            let r = rmod(fx, s0, 0x01E0_1DCC, 0x168);
            fx.mem.set16(ANGLE, r);
            let w = fx.mem.i16(t + 0x38);
            let r = rmod(fx, s0, 0x01E0_1DEC, w);
            let dx = (r - (w >> 1)) << 1;
            let x = ee::from_i32(fx.mem.i16(t) + dx);
            let y = ee::from_i32(fx.mem.i16(t + 2));
            let z = ee::from_i32(fx.mem.i16(t + 4));
            set_pos(fx, x, y, z);
            spawn(fx, MOTE, 0);
        }
    }
}

/// 0x1E01F60 (D, task): a glow (size 6, white over (0x10, 0x10, 8)),
/// alpha +0x40 for two updates, then down by 0xC.
fn glow(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 6);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x2C, 0x10, 0x10, 8);
        let r = rmod(fx, t, 0x01E0_1FB4, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if fx.mem.i16(t + 0x32) < 2 {
        add_alpha(fx, t, 0x2B, 0x40);
    } else {
        fade_out(fx, t, 0x11, 0xC);
    }
    age(fx, t);
}

/// 0x1E02040 (D, task): a violet quad band (size 10, cell rand % 4, turned
/// 0x8000 when the yaw is 0; +0x3A = its kind, then +0x30 = 5..14 growing
/// by +0xE = 30..49; jitter row +8 = rand % 50; alpha +0xC until age +6 =
/// 30..49, then down by 0xA).
fn band(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0xA);
        let r = fx.rand_from(t, 0x01E0_2070);
        l2::rgb(fx, t + 0x28, 0x70, 8, 0xC0);
        fx.mem.set16(t + 0x34, r % 4);
        if fx.mem.i16(YAW) != 0 {
            fx.mem.set32(t + 0x14, 0);
        } else {
            fx.mem.set32(t + 0x14, 0x8000);
        }
        fx.mem.set32(t + 0x10, 0);
        fx.mem.set32(t + 0x18, 0);
        let k = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0x3A, k);
        let r = rmod(fx, t, 0x01E0_20D4, 0xA);
        fx.mem.set16(t + 0x30, r + 5);
        let r = rmod(fx, t, 0x01E0_20F4, 0x14);
        fx.mem.set16(t + 0xE, r + 0x1E);
        let r = rmod(fx, t, 0x01E0_2114, 0x32);
        fx.mem.set16(t + 8, r);
        let r = rmod(fx, t, 0x01E0_2130, 0x14);
        fx.mem.set16(t + 6, r + 0x1E);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        add_alpha(fx, t, 0x2B, 0xC);
    } else {
        fade_out(fx, t, 0x11, 0xA);
    }
    let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x30, r);
    age(fx, t);
}

/// The ribbon's point D+0xA930 + 0x10 i16 +0x30.
fn point(fx: &mut Fx, t: u32) -> u32 {
    (PTS as i32 + 0x10 * fx.mem.i16(t + 0x30)) as u32
}

/// 0x1E021D0 (D, task): a ribbon (frame `sp`) from the attacker's point
/// (x shifted 4 x rand % 60 - 30 tenths), two 20-point histories 2 apart
/// in y, swaying in z by 6.5 a update toward the yaw's side.
fn ribbon(fx: &mut Fx, t: u32, sp: u32) {
    let i = fx.mem.i16(t + 0x30);
    let base = (D as i32 + 0x280 * i) as u32;
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0xA, 0x14);
        let r = fx.rand_from(D, 0x01E0_2218);
        let f20 = ee::div(ee::from_i32(0x14), 10.0);
        fx.mem.set16(t + 8, r % 2);
        if fx.mem.i16(YAW) != 0 {
            fx.mem.set32(t + 0x14, 0);
        } else {
            fx.mem.set32(t + 0x14, 0x4000);
        }
        let r = rmod(fx, D, 0x01E0_2270, 0x3C);
        fx.mem.set32(t + 0x1C, r - 0x1E);
        let f = ee::div(ee::from_i32(fx.mem.i32(t + 0x1C)), 10.0);
        let v = fix(fx, f, 0x01E0_22A8);
        fx.mem.set32(t + 0x1C, v);
        l2::rgb(fx, t + 0x2C, 0x80, 0x80, 0x80);
        let h = fx.self_handle();
        let p = point(fx, t);
        fx.unit_position(h, p);
        let x = ee::add(fx.mem.f(p), ee::mul(4.0, ee::from_i32(fx.mem.i32(t + 0x1C))));
        fx.mem.setf(p, x);
        fx.mem.set32(sp + 0x54, 0);
        let r = rmod(fx, sp + 0x54, 0x01E0_232C, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let z = fx.mem.read_u32(p + 8);
        fx.mem.write_u32(sp + 0x58, z);
        let x = fx.mem.read_u32(p);
        fx.mem.write_u32(sp + 0x50, x);
        for (h, y) in [(base + 0x2C30, None), (base + 0x2D70, Some(f20))] {
            if let Some(y) = y {
                fx.mem.setf(sp + 0x54, y);
            }
            for k in 0..0x13u32 {
                for c in 0..3u32 {
                    let w = fx.mem.read_u32(sp + 0x50 + 4 * c);
                    fx.mem.write_u32(h + 0x10 * k + 4 * c, w);
                }
            }
        }
        fx.mem.set16(t + 6, 2);
        fx.mem.write_u8(t + 0x2B, 0);
        fx.mem.set16(t + 8, 0);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x38, 5);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0x12 {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x80 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0xA));
        }
    } else {
        fade_out(fx, t, 0xB, 9);
    }
    let f1 = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    let a = fx.mem.i16(t + 0x3A);
    if a >= 0x168 {
        let r = fx.div(a, 0x168).1;
        fx.mem.set16(t + 0x3A, r);
    }
    let p = point(fx, t);
    let y8 = ee::from_i32(fx.mem.i16(t + 8));
    fx.mem.setf(sp + 0x54, y8);
    let z = fx.mem.read_u32(p + 8);
    fx.mem.write_u32(sp + 0x58, z);
    let x = fx.mem.read_u32(p);
    fx.mem.write_u32(sp + 0x50, x);
    bspawn::history(fx, base + 0x2D60, 0x14, sp + 0x50);
    let y = ee::add(f1, ee::from_i32(fx.mem.i16(t + 8)));
    fx.mem.setf(sp + 0x54, y);
    let z = if fx.mem.i16(YAW) != 0 { ee::add(10.0, fx.mem.f(p + 8)) } else { ee::sub(fx.mem.f(p + 8), 10.0) };
    fx.mem.setf(sp + 0x58, z);
    let x = fx.mem.read_u32(p);
    fx.mem.write_u32(sp + 0x50, x);
    bspawn::history(fx, base + 0x2EA0, 0x14, sp + 0x50);
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        l2::rgb(fx, t + 0x28, 0x80, 0x20, 0x40);
    } else {
        l2::rgb(fx, t + 0x28, 0x70, 0x10, 0x30);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let n = fx.mem.i16(t + 6);
        if n < 0x12 {
            fx.mem.set16(t + 6, n + 1);
        }
    }
    let z = if fx.mem.i16(YAW) != 0 { ee::sub(fx.mem.f(p + 8), 6.5) } else { ee::add(fx.mem.f(p + 8), 6.5) };
    fx.mem.setf(p + 8, z);
    if fx.mem.i16(t + 0x32) != 0 {
        let x = ee::add(fx.mem.f(p), ee::from_i32(fx.mem.i32(t + 0x1C)));
        fx.mem.setf(p, x);
    }
    age(fx, t);
}

/// 0x1E02750 (D, task): a blue ring of quarters (size 6, extents 2 x 2,
/// +0xA growing by 5, or 3 for the second ring (+0x3A != 0)), both alphas
/// +0x40 for two updates, then down by 0xD; turning by 10.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 6);
        l2::rgb(fx, t + 0x28, 0x30, 0x30, 0x80);
        l2::rgb(fx, t + 0x2C, 0x30, 0x30, 0x80);
        fx.mem.set16(t + 8, 2);
        fx.mem.set16(t + 0xA, 2);
        let e = if fx.mem.i16(t + 0x3A) != 0 { 1 } else { 0 };
        fx.mem.set16(t + 0xE, e);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if fx.mem.i16(t + 0x32) < 2 {
        add_alpha(fx, t, 0x2B, 0x40);
        add_alpha(fx, t, 0x2F, 0x40);
    } else {
        fade_out(fx, t, 0xF, 0xD);
        let a = fx.mem.read_u8(t + 0x2F);
        if a >= 0xF {
            fx.mem.write_u8(t + 0x2F, a - 0xD);
        } else {
            fx.mem.write_u8(t + 0x2F, 0);
        }
    }
    let up = if fx.mem.i16(t + 0xE) == 0 { 5 } else { 3 };
    let h = fx.mem.i16(t + 0xA) + up;
    fx.mem.set16(t + 0xA, h);
    age(fx, t);
    let a = fx.mem.i16(t + 0x3A) + 0xA;
    fx.mem.set16(t + 0x3A, a);
}

/// 0x1E037F0(D, task, desc, mode): b064's sprite_a (0x1E03D70) on the
/// texture D+0x328: a camera-facing quad of half size i16 +0x38 at the
/// task's position (+ offsets / 10), UVs of sprite 0x14 cell +0x34, keyed
/// by the mean w (0x109870).
fn sprite(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x110);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0xB0, 0x14, c);
        l2::desc_strip3(fx, d);
        l2::colour_strip3(fx, d, t + 0x28, 0);
        l2::uv_strip3(fx, d, sp + 0xB0, 0);
        let w = fx.mem.read_u32(D + 0x328);
        fx.desc_texture(d, w, 1);
    } else {
        l2::camera_angles_to(fx, sp + 0x100);
        for k in 0..3u32 {
            let off = fx.mem.i16(t + 8 + 2 * k);
            fx.mult(0x6666_6667, off);
            let hi = fx.hilo().1;
            let q = (hi >> 2).wrapping_add(((off as u32) >> 31) as i32);
            let v = fx.mem.i16(t + 2 * k).wrapping_add(q);
            fx.mem.setf(sp + 0xA0 + 4 * k, ee::from_i32(v));
        }
        l2::xform_vp(fx, sp + 0xC0, 0, sp + 0x100, sp + 0xA0);
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x60, s, s);
        l2::project_submit(fx, sp + 0x60, d, sp + 0xC0, OT);
    }
    fx.leave(0x110);
}

/// 0x1E03DF0(D, task, desc, mode): the column: mode 0 the strip3 of sprite
/// 0x9C cell 0 (colour +0x28 on all four), texture D+0x35C; mode 1 the
/// camera's view-projection at D+0x290 times the task's TRS (turn +0x10 /
/// +0x18, its point) and a scale ((+0x30) / 4, 1, 1) turned y by +0x14,
/// then D+0x360's first four quads at the far Z keyed by their mean w
/// (D+0xAC8C) on D+0xAC7C.
fn column_draw(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x100);
    if mode == 0 {
        l2::rect(fx, sp + 0x70, 0x9C, 0);
        l2::desc_strip3(fx, d);
        for (local, to) in [(0xE0u32, 0x10u32), (0xE8, 0x28), (0xF0, 0x40), (0xF8, 0x58)] {
            rgbaq_at(fx, sp + local, t + 0x28, d + to);
        }
        l2::uv_strip3(fx, d, sp + 0x70, 0);
        let w = fx.mem.read_u32(D + 0x35C);
        fx.desc_texture(d, w, 1);
    } else {
        let vp = D + 0x290;
        crate::blib::exe::camera_vp_into(fx, vp);
        let p: [f32; 3] = std::array::from_fn(|k| ee::from_i32(fx.mem.i16(t + 2 * k as u32)));
        l2::set3(fx, sp + 0x60, p[0], p[1], p[2]);
        let s = ee::div(ee::from_i32(fx.mem.i16(t + 0x30)), 4.0);
        fx.mem.setf(sp + 0x90, s);
        fx.mem.setf(sp + 0x94, 1.0);
        fx.mem.setf(sp + 0x98, 1.0);
        let (rx, rz) = (fx.mem.i32(t + 0x10), fx.mem.i32(t + 0x18));
        fx.mem.set32(sp + 0x80, rx);
        fx.mem.set32(sp + 0x84, 0);
        fx.mem.set32(sp + 0x88, rz);
        let m = fx.trs(None, Some([rx, 0, rz]), Some(p));
        fx.write_m4(sp + 0xA0, &m);
        let q: [f32; 3] = std::array::from_fn(|k| ee::from_i32(fx.mem.i16(t + 8 + 2 * k as u32)));
        l2::set3(fx, sp + 0x60, q[0], q[1], q[2]);
        crate::blib::exe::mul_mem(fx, vp, sp + 0xA0);
        let ry = fx.mem.i32(t + 0x14);
        fx.mem.set32(sp + 0x80, 0);
        fx.mem.set32(sp + 0x84, ry);
        fx.mem.set32(sp + 0x88, 0);
        let m = fx.trs(Some([s, 1.0, 1.0]), Some([0, ry, 0]), None);
        fx.write_m4(sp + 0xA0, &m);
        crate::blib::exe::mul_mem(fx, vp, sp + 0xA0);
        for k in 0..4u32 {
            if l2::project(fx, QUADS + 0x40 * k, SPR, vp, d, MEAN) {
                l2::far_z_desc(fx, d, 0);
                let key = key(fx, MEAN, 0x01E0_4038);
                let ot = fx.mem.read_u32(OTW);
                fx.submit(d, ot, key);
            }
        }
    }
    fx.leave(0x100);
}

/// The 9-value textured quad header of the glow / band draws: ALPHA 0x48,
/// vertex alphas 0, RGBAQ +0x28 (through `local`), UVs D+0x94 (uv_rect4
/// mode 0), texture D+0x88.
fn quad_desc(fx: &mut Fx, t: u32, d: u32, local: u32) {
    desc_uv4(fx, d);
    fx.desc_alpha(d, blend_alpha(2));
    fx.mem.set32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    rgbaq_at(fx, local, t + 0x28, d + 0x10);
    uv::uv_rect4(fx, d, D + 0x94, 0);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
}

/// 0x1E044A0(D, task, d, 0): b069's core quad (0x1E01830): size i16 +0x38
/// facing the camera at the point D+0x20 + 0x10 i16 +0x30 (scale 0.1), far
/// Z, keyed by the mean w (0x1A2C70) on D+0x80.
fn core(fx: &mut Fx, t: u32, d: u32) {
    let sp = fx.enter(0x110);
    quad_desc(fx, t, d, sp + 0x108);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0x90 + 4 * k, 0x3DCC_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0xF0);
    let p = (D as i32 + 0x20 + 16 * fx.mem.i16(t + 0x30)) as u32;
    for k in 0..3 {
        let w = fx.mem.read_u32(p + 4 * k);
        fx.mem.write_u32(sp + 0x80 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0xB0, sp + 0x90, sp + 0xF0, sp + 0x80);
    let s = ee::from_i32(fx.mem.i16(t + 0x38));
    l2::quad_xy(fx, sp + 0x40, s, s);
    if l2::project(fx, sp + 0x40, SPR, sp + 0xB0, d, sp + 0xA0) {
        l2::far_z(fx, &[d + 0x50, d + 0x40, d + 0x30, d + 0x20]);
        let key = key(fx, sp + 0xA0, 0x01E0_46B8);
        let ot = fx.mem.read_u32(D + 0x80);
        fx.submit(d, ot, key);
    }
    fx.leave(0x110);
}

/// 0x1E046E0(D, task, d, bytes): i16 D+0xB8 quads of size i16 +0x38 facing
/// the camera (the yaw turned to -yaw / 180, then each quad rotated about
/// x by the camera's angles) at the task's point turned by its angles
/// +0x10 (scale 0.1); quad s's corners pushed by the jitter bytes
/// [+8 + s] / 20, [+0x64 ..] / 20, [+0xB4 ..] / 10 times (+0x30 + s / 2)
/// (the lower pair 0.7 of it), each keyed by its mean w on D+0x80.
fn band_draw(fx: &mut Fx, t: u32, d: u32) {
    let sp = fx.enter(0x140);
    quad_desc(fx, t, d, sp + 0x138);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xC0 + 4 * k, 0x3DCC_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0x120);
    let a0 = fx.mem.i32(sp + 0x124);
        let y = div180(fx, a0);
    fx.mem.set32(sp + 0x124, y.wrapping_neg());
    for k in 0..3u32 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(sp + 0xB0 + 4 * k, v);
    }
    l2::xform_vp(fx, sp + 0xE0, sp + 0xC0, t + 0x10, sp + 0xB0);
    let k7 = f32::from_bits(0x3F33_3333);
    let mut s4 = 0i32;
    while s4 < fx.mem.i16(D + 0xB8) {
        let s5 = s4 as i16 as i32;
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x70, s, s);
        l2::rotate4(fx, sp + 0x70, sp + 0x120, 0, D + 0xD8);
        let row = (BYTES as i32 + s5 + fx.mem.i16(t + 8)) as u32;
        for (c, off, div) in [(0u32, 0u32, 20.0f32), (4, 0x64, 20.0), (8, 0xB4, 10.0)] {
            let b = i32::from(fx.mem.read_u8(row + off) as i8);
            let f4 = ee::div(ee::from_i32(b), div);
            let m = ee::from_i32(fx.mem.i16(t + 0x30) + (s5 >> 1));
            let f2 = ee::mul(f4, m);
            for o in [0u32, 0x10] {
                let v = ee::add(fx.mem.f(sp + 0x70 + o + c), f2);
                fx.mem.setf(sp + 0x70 + o + c, v);
            }
            for o in [0x20u32, 0x30] {
                let m = ee::from_i32(fx.mem.i16(t + 0x30) + (s5 >> 1));
                let f2 = ee::mul(k7, ee::mul(f4, m));
                let v = ee::add(fx.mem.f(sp + 0x70 + o + c), f2);
                fx.mem.setf(sp + 0x70 + o + c, v);
            }
        }
        if l2::project(fx, sp + 0x70, SPR, sp + 0xE0, d, sp + 0xD0) {
            let key = key(fx, sp + 0xD0, 0x01E0_4AA8);
            let ot = fx.mem.read_u32(D + 0x80);
            fx.submit(d, ot, key);
        }
        s4 = (s4 + 1) as i16 as i32;
    }
    fx.leave(0x140);
}

/// 0x1E04DB0(D, task, a, b): a textured strip3 ribbon (0x340 frame,
/// descriptor sp + 0x288) between the histories `a` / `b` (i16 +6 = n
/// points) lifted 40 in y through the camera: per segment s in 0 .. n - 1
/// four strips (t = 0, 0.25, 0.5, 0.75 by `spline` basis 0; the first
/// segment on stack copies a0 a0 a1 a2 / b0 b0 b1 b2, later ones a / b +
/// 0x10 (s - 1)); colours +0x28 (rows a) / +0x2C's rgb (rows b), both at
/// the running alpha, which falls by +0x2B / 2n a strip (not below 0);
/// UVs D+0x94 (order 0), texture D+0x88; each visible strip submitted
/// twice at its mean w's key on D+0x80.
fn ribbon_draw(fx: &mut Fx, t: u32, a: u32, b: u32) {
    let sp = fx.enter(0x340);
    let n = fx.mem.i16(t + 6);
    fx.mem.write_u32(sp + 0xC0, b);
    fx.mem.write_u32(sp + 0xD0, a);
    fx.mem.set32(sp + 0x130, 0);
    fx.mem.setf(sp + 0x134, 40.0);
    fx.mem.set32(sp + 0x138, 0);
    let m = sp + 0x200;
    l2::xform_vp(fx, m, 0, 0, sp + 0x130);
    let d = sp + 0x288;
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x28, 0);
    l2::uv_strip3(fx, d, D + 0x94, 0);
    let rh2 = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, rh2, 1);
    let (ca, cb) = (sp + 0x338, sp + 0x33C);
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(ca + k, v);
        let v = fx.mem.read_u8(t + 0x2C + k);
        fx.mem.write_u8(cb + k, v);
    }
    let n16 = n as i16 as i32;
    let fp = n16 << 1;
    fx.mem.set32(sp + 0xB0, n16 - 1);
    let (pa, pb) = (sp + 0x180, sp + 0x1C0);
    for (p, src) in [(pa, a), (pb, b)] {
        for (k, o) in [0u32, 0, 0x10, 0x20].into_iter().enumerate() {
            copy4(fx, p + 0x10 * k as u32, src + o);
        }
    }
    let mut s0 = 0i32;
    while s0 < fx.mem.i32(sp + 0xB0) {
        let off = (0x10 * (s0 - 1)) as u32;
        let (qa, qb, outs) = if s0 != 0 { (a.wrapping_add(off), b.wrapping_add(off), (0x240u32, 0x250u32)) } else { (pa, pb, (0x260, 0x270)) };
        let mut f20 = 0.0f32;
        for _ in 0..4 {
            let al = fx.mem.read_u8(ca + 3);
            fx.mem.write_u8(cb + 3, al);
            rgbaq_at(fx, sp + 0x318, ca, d + 0x10);
            rgbaq_at(fx, sp + 0x320, cb, d + 0x40);
            let al = i32::from(fx.mem.read_u8(t + 0x2B));
            let q = fx.div(al, fp).0 as i16 as i32;
            let v = i32::from(fx.mem.read_u8(ca + 3)) - q;
            fx.mem.write_u8(ca + 3, if v > 0 { v as u8 } else { 0 });
            let al = fx.mem.read_u8(ca + 3);
            fx.mem.write_u8(cb + 3, al);
            rgbaq_at(fx, sp + 0x328, ca, d + 0x28);
            rgbaq_at(fx, sp + 0x330, cb, d + 0x58);
            for j in 0..2u32 {
                spline::spline(fx, sp + outs.0, qa, f20, 0, [0.0; 4]);
                copy4(fx, sp + 0x140 + 0x10 * j, sp + outs.0);
                spline::spline(fx, sp + outs.1, qb, f20, 0, [0.0; 4]);
                copy4(fx, sp + 0x160 + 0x10 * j, sp + outs.1);
                if j == 0 {
                    f20 = ee::add(f20, 0.25);
                }
            }
            for (to, from) in [(0xF0u32, 0x140u32), (0x100, 0x150), (0x110, 0x160), (0x120, 0x170)] {
                for k in 0..3 {
                    let w = fx.mem.read_u32(sp + from + 4 * k);
                    fx.mem.write_u32(sp + to + 4 * k, w);
                }
            }
            if l2::project(fx, sp + 0xF0, SPR, m, d, sp + 0xE0) {
                for ra in [0x01E0_52D0u32, 0x01E0_52E8] {
                    let key = key(fx, sp + 0xE0, ra);
                    let ot = fx.mem.read_u32(D + 0x80);
                    fx.submit(d, ot, key);
                }
            }
        }
        s0 += 1;
    }
    fx.leave(0x340);
}
