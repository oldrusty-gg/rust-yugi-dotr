//! BMODULE b539 (card 384), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02C60, data area 0x1ED0000, 0xB630 bytes) [F]
//!: b590's layout (a pool of 400 tasks of 0x70
//! bytes at D+0x2A0, alive +0x58, step +0x44 called through `jalr`,
//! 0x1E02C50 only returns and is skipped; spawned by 0x1E03530 =
//! [`crate::b590::spawn`]) on the second library: quads (add5d81dd3fe at
//! 0x1E00C80, `billboard::quad` with uv_rect 0x1E00080), arcs
//! (e6dce326551d at 0x1E00920, `billboard::arc_strips`), light columns
//! (792ca1ad9378 at 0x1E00F40, `quads4::sprite_quads`) and full-window
//! tints (561a6131a635 at 0x1E012B0, `quads4::tint4`). Update 1 eighteen
//! arcs circling the attacker's node 2; to 0x71 a quad a update flying from
//! node 0x22 (the point kept at D+0xB1D0), on 0x57 ten arcs from it drifting
//! toward node 0x21 by a fifth of the gap x their index, on 0x65..0x71
//! seven bigger quads a update stacked along z, below 0x64 every third
//! update a sinking quad at node 0x5B or 0x61; on 0x71 a white quad
//! pair, two red columns, a red arc ring and a tint (by kind), and odd
//! updates 0x71..0x7B more rings. The attacker's unit flag +0x20A = 1 from
//! 0x67 (0x1F69C0). Returns 1 on update 0x67, 4 on 0xA2, 2 on 0x96, 6 on
//! 0x72..0x90, 8 once the 0xA4-update timer D+0xB210 has run out.
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
const POS: u32 = D + 0x250;
/// The step functions a task holds (+0x44), as the record's addresses.
const FLY_A: u32 = 0x01E0_1A50;
const ARC_A: u32 = 0x01E0_1D40;
const FLY_B: u32 = 0x01E0_1F60;
const ORBIT: u32 = 0x01E0_2270;
const FLY_C: u32 = 0x01E0_2410;
const PAIR: u32 = 0x01E0_2740;
const COLUMN: u32 = 0x01E0_2880;
const RING: u32 = 0x01E0_2990;
const TINT: u32 = 0x01E0_2AC0;
/// A step that only returns 0 (skipped by the entry).
const IDLE: u32 = 0x01E0_2C50;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_2F90;
const TEX: u32 = D + 0xB1F0;
const TIMER: u32 = D + 0xB210;
const TINT_W: u32 = D + 0xB234;
/// Node 0x22's point, kept below update 0x72.
const KEPT: u32 = D + 0xB1D0;
const NODE_A: u32 = D + 0xB248;
const NODE_B: u32 = D + 0xB24A;
const NODE_C: u32 = D + 0xB24C;
const NODE_D: u32 = D + 0xB24E;
const NODE_E: u32 = D + 0xB250;
const SPAWN_A: u32 = D + 0xB2A6;
const SPAWN_K: u32 = D + 0xB2B4;
const FACING: u32 = D + 0xB2B8;
const COUNTER: u32 = D + 0xB2BC;
const BYTES: u32 = D + 0xB2BE;
const SINES: u32 = D + 0xB4C0;

/// The entry 0x1E02C60 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x130);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x130);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2BA, param);
    fx.mem.set32(TIMER, 0xA4);
    for (k, node) in [0x21, 0x22, 2, 0x5B, 0x61, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODE_A + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB258, r);
    for k in 0xE..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x298 + 4 * (k as u32 - 0xE), t);
    }
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x90);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x94)), 180.0);
    fx.fix_residue(y, 0x01E0_2E54);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = fx.rand_from(k, 0x01E0_2E84);
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_2ED4);
        fx.mem.set16(D + 0xB268 + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    if fx.mem.i16(COUNTER) >= 0x67 {
        crate::b369::unit_flag(fx, 1, 0, 0x01E0_2F44);
    }
    emit(fx, param);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 || f == IDLE {
            continue;
        }
        // The steps' prologues: ra, the entry's s3 = k, s2 = the task -
        // 0x2A0, s1 = D, s0 = 0.
        match f {
            ARC_A => {
                let s = fx.enter(0x80);
                fx.saved(s, &[(0x50, TASK_RA), (0x40, u64::from(k)), (0x30, u64::from(t - 0x2A0)), (0x20, u64::from(D)), (0x10, 0)]);
                arc_a(fx, t, s);
                fx.leave(0x80);
            }
            ORBIT => {
                let s = fx.enter(0x30);
                fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
                orbit(fx, t);
                fx.leave(0x30);
            }
            _ => {
                let s = fx.enter(0x20);
                fx.saved(s, &[(0x10, TASK_RA), (0, 0)]);
                match f {
                    FLY_A => fly(fx, t, &FLY_A_V),
                    FLY_B => fly(fx, t, &FLY_B_V),
                    FLY_C => fly(fx, t, &FLY_C_V),
                    PAIR => pair(fx, t),
                    COLUMN => column(fx, t),
                    RING => ring(fx, t),
                    TINT => tint(fx, t),
                    _ => panic!("b539: unknown task step {f:#x}"),
                }
                fx.leave(0x20);
            }
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
        draw(fx, sp, t, s0);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x67 => 1,
        0xA2 => 4,
        0x96 => 2,
        0x72..=0x90 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` / `sub` (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// An arc with D+0xB8 = 0x24 (mode 0x21).
fn arc_draw(fx: &mut Fx, t: u32) {
    fx.mem.set16(D + 0xB8, 0x24);
    billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
    fx.mem.set16(D + 0xB8, 0);
}

fn draw(fx: &mut Fx, sp: u32, t: u32, s0: u32) {
    let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
    for g in [FLY_A, ARC_A, FLY_B, ORBIT, FLY_C] {
        if f(fx) != g {
            continue;
        }
        if g == ARC_A || g == ORBIT {
            arc_draw(fx, t);
            continue;
        }
        let w = fx.mem.read_u32(TEX);
        fx.mem.write_u32(D + 0x88, w);
        let o = fx.mem.i16(t + 0x52);
        fx.mem.set16(D + 0xBE, o);
        let c = fx.mem.i16(t + 0x56);
        if c < 2 {
            fx.mem.set16(D + 0x94, c << 6);
            fx.mem.set16(D + 0x96, 0);
        } else {
            fx.mem.set16(D + 0x94, (c - 2) << 6);
            fx.mem.set16(D + 0x96, 0x40);
        }
        fx.mem.set16(D + 0x98, 0x40);
        fx.mem.set16(D + 0x9A, 0x40);
        billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
    }
    if f(fx) == PAIR {
        let w = fx.mem.read_u32(TEX + 4);
        fx.mem.write_u32(D + 0x88, w);
        fx.mem.set16(D + 0xBE, 0);
        let c = fx.mem.i16(t + 0x56);
        cell(fx, sp + 0x70, 0x14, c);
        billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
        billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
    }
    if f(fx) == COLUMN {
        let w = fx.mem.read_u32(D + 0x29C);
        fx.mem.write_u32(D + 0x88, w);
        cell(fx, sp + 0x80, 0x9A, 0);
        fx.mem.set16(D + 0xBE, 0);
        for k in 0..3 {
            let v = fx.mem.read_u32(t + 0x10 + 4 * k);
            fx.mem.write_u32(sp + 0x60 + 4 * k, v);
        }
        fx.mem.write_u32(sp + 0x6C, 0);
        quads4::sprite_quads(fx, D, t, 0, t + 0x10);
        quads4::sprite_quads(fx, D, t, 0, t + 0x10);
    }
    if f(fx) == RING {
        arc_draw(fx, t);
    }
    if f(fx) == TINT {
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x60);
        // 0x1F71A0's frame below the entry's (its saved ra, s0).
        fx.saved(sp - 0x20, &[(0x10, 0x01E0_3358), (0, u64::from(s0))]);
        let z = fx.mem.f(sp + 0x68);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 50.0) } else { ee::add(z, 50.0) };
        fx.mem.setf(sp + 0x68, z);
        let p = [fx.mem.f(sp + 0x60), fx.mem.f(sp + 0x64), fx.mem.f(sp + 0x68)];
        let mut x = fx.mem.read_u64(sp + 0xB8);
        if let Some(w) = fx.project(p, None, &mut x) {
            fx.mem.write_u64(sp + 0xB8, x);
            fx.mem.setf(TINT_W, w);
        }
        crate::b590::project_residue(fx, sp, sp + 0x60);
        for (kind, mode) in [(0, 0x21), (1, 0x20), (2, 0x20)] {
            if fx.mem.i16(t + 0x50) == kind {
                let w = fx.mem.f(TINT_W);
                quads4::tint4(fx, t, mode, w);
            }
        }
    }
}

/// The attacker's node at `at` into `out`.
fn node(fx: &mut Fx, at: u32, out: u32) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(at);
    fx.node_position(h, slot, out);
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let v = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, v);
    }
}

fn spawn(fx: &mut Fx, step: u32) {
    crate::b590::spawn(fx, step);
}

/// 0x1E01480(D, 0): the update counter and the emitters (frame 0x60: ra,
/// the entry's s2 = param, s1 = D, s0 = 0).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x30, 0x01E0_2F50), (0x20, param as i64 as u64), (0x10, u64::from(D)), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    if c(fx) < 0x72 {
        node(fx, NODE_A, sp + 0x40);
        node(fx, NODE_B, sp + 0x50);
        for k in 0..3 {
            let v = fx.mem.read_u32(sp + 0x50 + 4 * k);
            fx.mem.write_u32(POS + 4 * k, v);
        }
        for k in 0..3 {
            let v = fx.mem.read_u32(POS + 4 * k);
            fx.mem.write_u32(KEPT + 4 * k, v);
        }
    }
    if c(fx) == 0x57 {
        for k in 0..0xA {
            fx.mem.set16(SPAWN_K, k);
            spawn(fx, ARC_A);
        }
    }
    if c(fx) < 0x72 {
        copy4(fx, POS, KEPT);
        spawn(fx, FLY_A);
    }
    if (0x65..0x72).contains(&c(fx)) {
        node(fx, NODE_A, sp + 0x40);
        node(fx, NODE_B, sp + 0x50);
        let dz = ee::div(ee::sub(fx.mem.f(sp + 0x48), fx.mem.f(sp + 0x58)), 5.0);
        fx.mem.setf(sp + 0x48, dz);
        copy4(fx, POS, KEPT);
        for _ in 0..7 {
            spawn(fx, FLY_B);
            let z = ee::add(fx.mem.f(POS + 8), fx.mem.f(sp + 0x48));
            fx.mem.setf(POS + 8, z);
        }
    }
    if c(fx) == 1 {
        for k in 0..0x12 {
            fx.mem.set16(SPAWN_K, k);
            spawn(fx, ORBIT);
        }
    }
    if c(fx) < 0x64 {
        // rand's s0: the FLY_A loop's count (1).
        for (rem, at, ra) in [(0, NODE_D, 0x01E0_17F4u32), (3, NODE_E, 0x01E0_1898)] {
            let n = c(fx);
            if fx.div(n, 6).1 != rem {
                continue;
            }
            node(fx, at, POS);
            let v = fx.rand_from(1, ra);
            let r = fx.div(v, 10).1 - 5;
            let x = ee::add(fx.mem.f(POS), ee::from_i32(r));
            fx.mem.setf(POS, x);
            let y = ee::sub(fx.mem.f(POS + 4), 1.5);
            fx.mem.setf(POS + 4, y);
            spawn(fx, FLY_C);
        }
    }
    if c(fx) == 0x71 {
        copy4(fx, POS, KEPT);
        spawn(fx, PAIR);
        fx.mem.set16(SPAWN_A, 0);
        spawn(fx, COLUMN);
        fx.mem.set16(SPAWN_A, 0x5A);
        spawn(fx, COLUMN);
        spawn(fx, RING);
        fx.mem.set16(SPAWN_K, 0);
        spawn(fx, TINT);
    }
    let n = c(fx);
    if (0x71..0x7C).contains(&n) && n % 2 == 1 {
        copy4(fx, POS, KEPT);
        spawn(fx, RING);
    }
    fx.leave(0x60);
}

/// What differs between the three flying quads.
struct Fly {
    /// The size: fixed, or (rand >> 4) % 30 + 30 (the rand's `jal`).
    size: Option<u32>,
    /// The `jal`s: cell row, the velocities (z, x, y), the angle, the
    /// first camera angles, the spins (4, 2, 0), the life, the turn, the
    /// last camera angles.
    jals: [u32; 12],
    /// life = (rand >> 4) % life.0 + life.1.
    life: (i32, i32),
    /// y -= 0.4 after the move.
    sinks: bool,
}

const FLY_A_V: Fly = Fly {
    size: None,
    jals: [0x01E0_1A78, 0x01E0_1AAC, 0x01E0_1AD8, 0x01E0_1B04, 0x01E0_1B5C, 0x01E0_1B78, 0x01E0_1B80, 0x01E0_1BA0, 0x01E0_1BC0, 0x01E0_1BE0, 0x01E0_1C04, 0x01E0_1D10],
    life: (15, 5),
    sinks: false,
};
const FLY_B_V: Fly = Fly {
    size: Some(0x01E0_1F7C),
    jals: [0x01E0_1FA8, 0x01E0_1FDC, 0x01E0_2008, 0x01E0_2034, 0x01E0_208C, 0x01E0_20A8, 0x01E0_20B0, 0x01E0_20D0, 0x01E0_20F0, 0x01E0_2110, 0x01E0_2134, 0x01E0_2240],
    life: (15, 5),
    sinks: false,
};
const FLY_C_V: Fly = Fly {
    size: Some(0x01E0_242C),
    jals: [0x01E0_2458, 0x01E0_248C, 0x01E0_24B8, 0x01E0_24E4, 0x01E0_253C, 0x01E0_2558, 0x01E0_2560, 0x01E0_2580, 0x01E0_25A0, 0x01E0_25C0, 0x01E0_25E4, 0x01E0_270C],
    life: (10, 3),
    sinks: true,
};

fn rmod(fx: &mut Fx, t: u32, jal: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, jal + 8);
    fx.div(v, m).1
}

fn camera_angles(fx: &mut Fx, t: u32, s0: u32, s1: u32, jal: u32) {
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(s0), u64::from(s1), jal + 8);
}

/// 0x1E01A50 / 0x1E01F60 / 0x1E02410 (D, task): a red quad (size 20, or
/// 30..59) growing by 1 to 70, a random cell row 0..2, flying (-50..49) /
/// 300 a update (0x1E02410 sinking 0.4 more), turned by the camera's
/// angles plus spins of -0x400..0x3FF; in by 0x40 under 0x80 for its life
/// 5..19 (3..12), then out by 9; the orientation +0x52 0..3 signed.
fn fly(fx: &mut Fx, t: u32, v: &Fly) {
    let j = v.jals;
    if fx.mem.i16(t + 0x58) == 1 {
        let s = match v.size {
            Some(jal) => {
                let r = fx.rand_from(t, jal + 8);
                fx.div(r >> 4, 0x1E).1 + 0x1E
            }
            None => 0x14,
        };
        fx.mem.set16(t + 8, s);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let r = fx.rand_from(t, j[0] + 8);
        let c = fx.div(r >> 4, 3).1;
        fx.mem.set16(t + 0x56, c);
        l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x40);
        fx.mem.set16(t + 0x58, 2);
        for (o, jal) in [(0x34u32, j[1]), (0x30, j[2]), (0x38, j[3])] {
            let r = rmod(fx, t, jal, 0x64);
            fx.mem.setf(t + o, ee::from_i32(r - 0x32));
        }
        for o in [0x34u32, 0x30, 0x38] {
            let q = ee::div(fx.mem.f(t + o), 300.0);
            fx.mem.setf(t + o, q);
        }
        let r = rmod(fx, t, j[4], 0x168);
        fx.mem.set16(t + 0x5C, r);
        camera_angles(fx, t, t, D, j[5]);
        for (o, jal) in [(4u32, j[6]), (2, j[7]), (0, j[8])] {
            let r = fx.rand_from(t, jal + 8);
            fx.mem.set16(t + o, r % 0x800 - 0x400);
        }
        let r = fx.rand_from(t, j[9] + 8);
        let life = fx.div(r >> 4, v.life.0).1 + v.life.1;
        fx.mem.set16(t + 6, life);
        let r = fx.rand_from(t, j[10] + 8);
        fx.mem.set16(t + 0x52, (r >> 4) % 4);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x40));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let s = fx.mem.i16(t + 8);
        if s < 0x46 {
            fx.mem.set16(t + 8, s + 1);
        }
        let s = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, s);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [0u32, 4, 8] {
            let p = ee::add(fx.mem.f(t + 0x10 + o), fx.mem.f(t + 0x30 + o));
            fx.mem.setf(t + 0x10 + o, p);
        }
        if v.sinks {
            let y = ee::sub(fx.mem.f(t + 0x14), f32::from_bits(0x3ECC_CCCD));
            fx.mem.setf(t + 0x14, y);
        }
    }
    for (o, by) in [(0x28u32, 4u32), (0x24, 2), (0x20, 0)] {
        let a = fx.mem.read_u32(t + o).wrapping_add(fx.mem.i16(t + by) as u32);
        fx.mem.write_u32(t + o, a);
    }
    camera_angles(fx, t, t, D, j[11]);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E01D40 (D, task): an arc (outer radius 10, grey outer / red inner,
/// angles 10 / 10) at the kept point plus a fifth of node 0x21 - node
/// 0x22 x its index +0x52 (while the count is below 0x72), facing the
/// camera turned a quarter; its outer alpha in by 0x18 under 0xC0 for 55
/// updates, then out by 15.
fn arc_a(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x4120_0000);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x40);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    node(fx, NODE_A, sp + 0x60);
    node(fx, NODE_B, sp + 0x70);
    for o in [0u32, 4, 8] {
        let d = ee::div(ee::sub(fx.mem.f(sp + 0x60 + o), fx.mem.f(sp + 0x70 + o)), 5.0);
        fx.mem.setf(sp + 0x60 + o, d);
    }
    let a = fx.mem.read_u8(t + 0x67);
    if fx.mem.i16(t + 0x54) < 0x37 {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x67, a.wrapping_add(0x18));
        }
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x67, a - 0xF);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    let f20 = ee::from_i32(fx.mem.i16(t + 0x52));
    camera_angles(fx, t, sp + 0x64, sp + 0x68, 0x01E0_1EA0);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    if fx.mem.i16(COUNTER) < 0x72 {
        copy4(fx, t + 0x10, KEPT);
        for o in [0u32, 4, 8] {
            let p = ee::add(fx.mem.f(t + 0x10 + o), ee::mul(fx.mem.f(sp + 0x60 + o), f20));
            fx.mem.setf(t + 0x10 + o, p);
        }
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E02270 (D, task): an arc (outer radius 15, grey outer / yellow
/// inner, angles 10 / 10) at node 2 plus 2 (sin, cos) of 20 x its index
/// +0x52 degrees, facing the camera turned a quarter; its outer alpha in
/// by 0x10 under 0x80 for 102 updates, then out by 19.
fn orbit(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x4170_0000);
        l2::rgb(fx, t + 0x64, 0x48, 0x48, 0x48);
        l2::rgb(fx, t + 0x60, 0x60, 0x60, 0x30);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    node(fx, NODE_C, t + 0x10);
    let a = i32::from((fx.mem.i16(t + 0x52) * 20) as i16);
    let s = l2::sin_deg(fx, SINES, a, 0);
    let z = ee::add(fx.mem.f(t + 0x18), ee::mul(2.0, s));
    fx.mem.setf(t + 0x18, z);
    let a = i32::from((fx.mem.i16(t + 0x52) * 20) as i16);
    let c = l2::sin_deg(fx, SINES, a, 1);
    let x = ee::add(fx.mem.f(t + 0x10), ee::mul(2.0, c));
    fx.mem.setf(t + 0x10, x);
    let a = fx.mem.read_u8(t + 0x67);
    if fx.mem.i16(t + 0x54) < 0x66 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x67, a.wrapping_add(0x10));
        }
    } else if a >= 0x15 {
        fx.mem.write_u8(t + 0x67, a - 0x13);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    camera_angles(fx, t, t, D, 0x01E0_23CC);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E02740 (D, task): a white quad of size 40 growing by 1 to 70 (cell
/// 0), turned by the camera's angles (+0x5C a random angle); in by 0x40
/// under 0x80 for 25 updates, then out by 9; orientation 0..3 signed.
fn pair(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x28);
        let s = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, s);
        fx.mem.set16(t + 0x56, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let r = rmod(fx, t, 0x01E0_2788, 0x168);
        fx.mem.set16(t + 0x5C, r);
        camera_angles(fx, t, t, D, 0x01E0_27A4);
        fx.mem.set16(t + 6, 0x19);
        let r = fx.rand_from(t, 0x01E0_27B8);
        fx.mem.set16(t + 0x52, (r >> 4) % 4);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x40));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let s = fx.mem.i16(t + 8);
        if s < 0x46 {
            fx.mem.set16(t + 8, s + 1);
        }
        let s = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, s);
    }
    camera_angles(fx, t, t, D, 0x01E0_2854);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E02880 (D, task): a light column (size 6, width 25, height 130
/// growing by 5, white / dark red) turning by 6 a update; in by 0x40 under
/// 0x80 for 4 updates, then out by 9.
fn column(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x80, 0x10, 0x10);
        fx.mem.write_u32(t + 0x30, 0x41C8_0000);
        fx.mem.write_u32(t + 0x34, 0x4302_0000);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
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
    if fx.mem.i16(t + 0x54) != 0 {
        let h = ee::add(fx.mem.f(t + 0x34), 5.0);
        fx.mem.setf(t + 0x34, h);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    let a = fx.mem.i16(t + 0x5A) + 6;
    fx.mem.set16(t + 0x5A, a);
}

/// 0x1E02990 (D, task): a red arc ring (inner radius 3 growing by 5, the
/// outer 3 less, angles 10 / 10) facing the camera turned a quarter; its
/// outer alpha in by 0x60 under 0xC0 for 10 updates, then out by 15.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4040_0000);
        l2::rgb(fx, t + 0x64, 0xFF, 0x30, 0x30);
        l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x40);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.write_u32(t + 0x20, 0x4000);
    }
    let a = fx.mem.read_u8(t + 0x67);
    if fx.mem.i16(t + 0x54) < 0xA {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x67, a.wrapping_add(0x60));
        }
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x67, a - 0xF);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    camera_angles(fx, t, t, D, 0x01E0_2A50);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = ee::add(fx.mem.f(t + 0x48), 5.0);
        fx.mem.setf(t + 0x48, r);
    }
    let r = ee::sub(fx.mem.f(t + 0x48), 3.0);
    fx.mem.setf(t + 0x4C, r);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E02AC0 (D, task): a tint's colours by its kind +0x50 (0 violet,
/// life 14; 1 pale yellow, 20; 2 pale pink, 20); both alphas in by 0x12
/// under 0x80 while young, then out by 5 (the task ends when the first is
/// below 7, the second is cleared).
fn tint(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let kinds: [(i16, [i32; 3], [i32; 3], i32); 3] = [(0, [0x80, 0x40, 0xF0], [0x80, 0x40, 0xF0], 0xE), (1, [0xD0, 0xD0, 0x80], [0xC0, 0xC0, 0x80], 0x14), (2, [0xF0, 0xE0, 0xFF], [0xF0, 0xE0, 0xFF], 0x14)];
        for (kind, a, b, life) in kinds {
            if fx.mem.i16(t + 0x50) == i32::from(kind) {
                l2::rgb(fx, t + 0x60, a[0], a[1], a[2]);
                l2::rgb(fx, t + 0x64, b[0], b[1], b[2]);
                fx.mem.set16(t + 6, life);
            }
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0x80 {
                fx.mem.write_u8(t + o, a.wrapping_add(0x12));
            }
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 7 {
            fx.mem.write_u8(t + 0x63, a - 5);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let a = fx.mem.read_u8(t + 0x67);
        fx.mem.write_u8(t + 0x67, if a >= 7 { a - 5 } else { 0 });
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}
