//! BMODULE b734 (card 359), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03390, data area 0x1ED0000, 0x7180 bytes) [F]
//!: b567's program and layout (every global where
//! b567 has it) in purple, with b564's unit flash: pools of 150 x 0x40 at
//! D+0x380 and 150 x 0x70 at D+0x2900 (`jalr`; the flash tasks 0x1E030B0
//! counted and run by the unit flash 0x1E04320). The emitter 0x1E01770
//! counts D+0x6E0C: update 0x94 the purple tube along the trails of the
//! attacker's nodes 1 / 7; 0x9E at nodes 7 / 3 / 4 halfway to the target
//! purple and dark rings, fifty small purple quads, four white quads
//! (0x1E02BB0), a purple arc ring and a camera light; 0x69 / 0x9F
//! twenty-six white quads thrown from node 1, 0x2F..0x57 five a update;
//! updates 1..5 five grey flashes; 1..0x18 and 0x84..0xA0 a flickering
//! grey ring at node 1 (0x1E02970, `ring_facing_flat`). The steps
//! 0x1E02E20 / 0x1E02EE0 (an object op) / 0x1E02F30 are never spawned (not
//! ported). Returns 1 on update 0x99, 4 on 0xBE, 2 on 0xA3, 8 once the
//! 0xD2-update timer D+0x6DA0 has run out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, exe::camera_slots45, quads4, spawn::history, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS_A: u32 = 150;
const TASKS_B: u32 = 150;
/// The 0x40-byte tasks.
const POOL_A: u32 = D + 0x380;
/// The 0x70-byte tasks.
const POOL_B: u32 = D + 0x2900;
/// Pool A's steps.
const TRAIL: u32 = 0x01E0_1CE0;
const DARK: u32 = 0x01E0_1EE0;
const RED: u32 = 0x01E0_2090;
const CAMERA: u32 = 0x01E0_2CA0;
/// Pool B's steps.
const FLY: u32 = 0x01E0_2220;
const ARC: u32 = 0x01E0_24B0;
const WHITE: u32 = 0x01E0_25F0;
const WHITE_B: u32 = 0x01E0_27B0;
const FLARE: u32 = 0x01E0_2970;
const QUAD: u32 = 0x01E0_2BB0;
/// The flash tasks, run by the unit flash 0x1E04320.
const FLASH: u32 = 0x01E0_30B0;
/// The return address of the pool B loop's `jalr` (0x1E0388C + 8).
const TASK_RA: u64 = 0x01E0_3894;
const POS: u32 = D + 0x2D0;
const TEX4: u32 = D + 0x340;
const CAM: u32 = D + 0x6AA0;
const TRAIL_Q: u32 = D + 0x6AF0;
const TRAIL_P: u32 = D + 0x6B90;
const WAVE: u32 = D + 0x6D70;
const TEX: u32 = D + 0x6D80;
const TIMER: u32 = D + 0x6DA0;
const COLOUR: u32 = D + 0x6DA4;
const LAYER: u32 = D + 0x6DB4;
const FLASH_DESC: u32 = D + 0x6DB8;
const NODES: u32 = D + 0x6DEC;
const ANGLE: u32 = D + 0x6E02;
const KIND: u32 = D + 0x6E06;
const FACING: u32 = D + 0x6E0A;
const COUNTER: u32 = D + 0x6E0C;
const BYTES: u32 = D + 0x6E10;
const BYTES_B: u32 = D + 0x6F10;
const SINES: u32 = D + 0x7010;
const SPR: u32 = 0x7000_0000;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E03390.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1A0);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x6DAC, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x1A0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x7180);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x6E08, param);
    fx.mem.set32(TIMER, 0xD2);
    for o in [0x310u32, 0x314, 0x318] {
        fx.mem.write_u32(D + o, 0);
    }
    for (k, node) in [1, 7, 3, 4, 5, 6].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    // 0x1E032E0(0, D): the textures.
    for k in 0..3 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(TEX4 + 4 * k as u32, t);
    }
    for k in 0xE..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(TEX4 + 4 * k as u32, t);
    }
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xF0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xF4)), 180.0);
    fx.fix_residue(y, 0x01E0_34E0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    // rand's frames: the entry's s0 = D.
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_34EC, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3520, 0x1E);
        fx.mem.set8(BYTES_B + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, D, 0x01E0_3554, 0x14);
        fx.mem.set8(BYTES_B + k, v - 0xA);
    }
    0
}

/// The last XYZ2 word of the last primitive drawn since `before` (0 when
/// none): a3 as an arc's far-Z submit left it (b564).
fn last_xyz2(fx: &mut Fx, before: usize) -> u32 {
    match fx.out.prims.get(before..).and_then(|p| p.last()) {
        Some(p) => {
            let d = p.desc;
            let n = u32::from(fx.mem.read_u16(d));
            fx.mem.read_u32(d + 8 + 8 * n)
        }
        None => 0,
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS_A {
        let slot = D + 0x40 * k;
        let t = POOL_A + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x24) == TRAIL {
            let s = fx.enter(0x60);
            fx.saved(s, &[(0x40, 0x01E0_35C4), (0x30, u64::from(slot)), (0x20, 0), (0x10, 0), (0, u64::from(D))]);
            trail(fx, t);
            fx.leave(0x60);
        }
        for (f, ra) in [(DARK, 0x01E0_35E8u64), (RED, 0x01E0_3608)] {
            if fx.mem.read_u32(t + 0x24) == f {
                let s = fx.enter(0x20);
                fx.saved(s, &[(0x10, ra), (0, u64::from(D))]);
                ring_a(fx, t, f == RED);
                fx.leave(0x20);
            }
        }
        if fx.mem.read_u32(t + 0x24) == CAMERA {
            let s = fx.enter(0x30);
            fx.saved(s, &[(0x20, 0x01E0_3628), (0x10, 0), (0, u64::from(D))]);
            camera(fx, t);
            fx.leave(0x30);
        }
    }
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x70, u64::from(D), 0x01E0_36DC);
    for k in 0..TASKS_A {
        let t = POOL_A + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x24) == TRAIL {
            if fx.mem.i16(t + 0x30) == 0 {
                fx.mem.write_u32(D + 4, 0);
            }
            if fx.mem.i16(t + 0x30) == 1 {
                fx.mem.write_u32(D + 4, 0x3F80_0000);
            }
            fx.mem.write_u32(D, 0x4168_0000);
            let z = if fx.mem.i16(FACING) != 0 { 0xC120_0000 } else { 0x4120_0000 };
            fx.mem.write_u32(D + 8, z);
            if fx.mem.i16(t + 0x30) == 0 {
                tube(fx, t, TRAIL_P, TRAIL_Q);
            }
        }
        for (f, local, t20) in [(DARK, 0x80u32, false), (RED, 0x90, true)] {
            if fx.mem.read_u32(t + 0x24) != f {
                continue;
            }
            let w = fx.mem.read_u32(TEX4 + 0x3C);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x34);
            cell(fx, sp + local, 0x9B, sub);
            if t20 {
                billboard::ring_t20(fx, uv::uv_rect4, D, t, sp + 0x108, BYTES);
            } else {
                billboard::ring_t(fx, uv::uv_rect4, D, t, sp + 0x108, BYTES);
            }
        }
    }
    // The entry's s1: the flash tasks counted here.
    let mut flashes = 0u32;
    // a3 as the last call left it, for the unit flash's first TEXFLUSH: 1
    // after a ring step's 0x1F6830, 0 after a quad or a ring draw, the last
    // XYZ2 after an arc (0x205580 / 0x108910 leave it).
    let mut a3 = 0u32;
    for k in 0..TASKS_B {
        let t = POOL_B + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 {
            continue;
        }
        if f == FLASH {
            flashes += 1;
            continue;
        }
        // The steps' prologues: the entry's s0 = D (and s1 the flashes).
        if f == FLY || f == QUAD {
            let s = fx.enter(0x20);
            fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]);
            if f == FLY {
                fly(fx, t, flashes);
            } else {
                quad(fx, t, flashes);
            }
            fx.leave(0x20);
            continue;
        }
        let s = fx.enter(0x30);
        fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(flashes)), (0, u64::from(D))]);
        match f {
            ARC => arc(fx, t),
            WHITE => white(fx, t, false),
            WHITE_B => white(fx, t, true),
            FLARE => {
                flare(fx, t);
                a3 = 1;
            }
            _ => panic!("b734: unknown task step {f:#x}"),
        }
        fx.leave(0x30);
    }
    for k in 0..TASKS_B {
        let t = POOL_B + 0x70 * k;
        // The original reads pool A's task k (+0x34 / +0xA).
        let ta = POOL_A + 0x40 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        if f(fx) == FLY {
            let w = fx.mem.read_u32(TEX4 + 0x3C);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(ta + 0x34);
            cell(fx, sp + 0xA0, 0x9B, sub);
            billboard::quad(fx, uv::uv_rect4, D, t, 0x20, t + 0x10);
            a3 = 0;
        }
        if f(fx) == ARC {
            fx.mem.set16(D + 0xB8, 0x24);
            let before = fx.out.prims.len();
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            a3 = last_xyz2(fx, before);
            fx.mem.set16(D + 0xB8, 0);
        }
        for (g, local) in [(WHITE, 0xB0u32), (WHITE_B, 0xC0)] {
            if f(fx) != g {
                continue;
            }
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(ta + 0x34);
            cell(fx, sp + local, 0x14, sub);
            billboard::quad(fx, uv::uv_rect4, D, t, 0x20, t + 0x10);
            a3 = 0;
        }
        if f(fx) == FLARE {
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0xD0, 0x14, sub);
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 6);
            let a = fx.mem.i16(ta + 0xA);
            fx.mem.set16(D + 0xC0, a);
            for c in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * c);
                fx.mem.write_u32(D + 0x20 + 4 * c, w);
            }
            quads4::ring_facing_flat(fx, uv::uv_rect4, D, t, 0x20, BYTES_B);
            fx.mem.set16(D + 0xB8, 0);
            a3 = 0;
        }
        if f(fx) == QUAD {
            let w = fx.mem.read_u32(TEX + 8);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(ta + 0x34);
            cell(fx, sp + 0xE0, 0x14, sub);
            let sub = fx.mem.i16(ta + 0x34);
            fx.mem.set16(D + 0x94, sub << 6);
            fx.mem.set16(D + 0x96, 0);
            fx.mem.set16(D + 0x98, 0x3F);
            fx.mem.set16(D + 0x9A, 0x3F);
            billboard::quad(fx, uv::uv_rect4, D, t, 0x21, t + 0x10);
            a3 = 0;
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
        0x99 => 1,
        0xBE => 4,
        0xA3 => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` frame `sub` (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// 0x1E03C30(D, step, 0) (b567's 0x1E03590, word for word): the first free
/// 0x40-byte task set off.
fn spawn_a(fx: &mut Fx, step: u32, saves: [u64; 5], ra: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, saves[4]), (0x30, saves[3]), (0x20, saves[2]), (0x10, saves[1]), (0, saves[0])]);
    if let Some(k) = (0..TASKS_A).find(|&k| fx.mem.i16(POOL_A + 0x40 * k + 0x36) == 0) {
        let t = POOL_A + 0x40 * k;
        fx.bzero(t, 0x40);
        fx.mem.set16(t + 0x36, 1);
        colour(fx, t, [0x28, 0x2C]);
        l2::to_i16x4(fx, t, POS, 0);
        let s = fx.mem.i16(KIND);
        fx.mem.set16(t + 0x30, s);
        fx.mem.write_u32(t + 0x20, 0);
        fx.mem.write_u32(t + 0x24, step);
    }
    fx.leave(0x60);
}

/// 0x1E03D50(D, step, 0) (b567's 0x1E036B0, word for word).
fn spawn_b(fx: &mut Fx, step: u32, saves: [u64; 5], ra: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, saves[4]), (0x30, saves[3]), (0x20, saves[2]), (0x10, saves[1]), (0, saves[0])]);
    if let Some(k) = (0..TASKS_B).find(|&k| fx.mem.i16(POOL_B + 0x70 * k + 0x58) == 0) {
        let t = POOL_B + 0x70 * k;
        fx.bzero(t, 0x70);
        fx.mem.set16(t + 0x58, 1);
        colour(fx, t, [0x60, 0x64]);
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

fn colour(fx: &mut Fx, t: u32, at: [u32; 2]) {
    for o in at {
        let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
        l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
        let a = fx.mem.read_u8(COLOUR + 3);
        fx.mem.write_u8(t + o + 3, a);
    }
}

/// The emitter's s0..s3 (s4 = D).
struct Regs {
    s: [u64; 4],
}

impl Regs {
    fn saves(&self) -> [u64; 5] {
        [self.s[0], self.s[1], self.s[2], self.s[3], u64::from(D)]
    }
}

/// POS at the self node in slot `o` halfway (z) to the target (the 0x9E
/// spawns; locals sp+0x60 / sp+0x70).
fn halfway(fx: &mut Fx, sp: u32, r: &mut Regs, o: u32, ras: [u32; 2]) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES + o);
    node_position_saving(fx, h, slot, sp + 0x60, r.s[0], Some(r.s[1]), ras[0]);
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x70, r.s[0], ras[1]);
    let w = fx.mem.read_u32(sp + 0x60);
    fx.mem.write_u32(POS, w);
    r.s[1] = u64::from(sp + 0x64);
    r.s[2] = u64::from(sp + 0x68);
    r.s[3] = u64::from(sp + 0x78);
    r.s[0] = 0;
    let w = fx.mem.read_u32(sp + 0x64);
    fx.mem.write_u32(POS + 4, w);
    let a = fx.mem.f(sp + 0x68);
    let z = ee::add(a, ee::div(ee::sub(fx.mem.f(sp + 0x78), a), 2.0));
    fx.mem.setf(POS + 8, z);
    fx.mem.set16(KIND, 1);
}

/// POS at the self node 1 (0x1F6830 straight into POS), less 1.5 in y
/// when `lift`.
fn at_node1(fx: &mut Fx, r: &Regs, lift: bool, ra: u32) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES);
    node_position_saving(fx, h, slot, POS, r.s[0], Some(r.s[1]), ra);
    if lift {
        let y = ee::sub(fx.mem.f(POS + 4), 1.5);
        fx.mem.setf(POS + 4, y);
    }
}

/// 0x1E01770(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter,
/// the spawns and the wave D+0x6D70 = 100 sin(D+0x6E02) (the angle up by
/// 5 a update). Frame 0x80 (ra, the entry's s4 / s3 = the parameter / s2
/// = 0 / s1 = 0 / s0 = D); its own s4 = D, s0 the loop counts, s1 / s2 /
/// s3 locals after the 0x9E spawns.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x80);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x50, 0x01E0_3590), (0x40, 0), (0x30, p), (0x20, 0), (0x10, 0), (0, u64::from(D))]);
    let mut r = Regs { s: [u64::from(D), 0, 0, p] };
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    if c(fx) == 0x94 {
        fx.mem.set16(KIND, 0);
        spawn_a(fx, TRAIL, r.saves(), 0x01E0_17D0);
    }
    if c(fx) == 0x9E {
        halfway(fx, sp, &mut r, 2, [0x01E0_17F8, 0x01E0_180C]);
        for k in 0..8u64 {
            r.s[0] = k;
            spawn_a(fx, RED, r.saves(), 0x01E0_1868);
        }
        for k in 0..0x14u64 {
            r.s[0] = k;
            spawn_a(fx, DARK, r.saves(), 0x01E0_1890);
        }
        for k in 0..0x32u64 {
            r.s[0] = k;
            spawn_b(fx, FLY, r.saves(), 0x01E0_18B8);
        }
        r.s[0] = 0x32;
        for ra in [0x01E0_18DCu32, 0x01E0_18F0, 0x01E0_1904, 0x01E0_1918] {
            spawn_b(fx, QUAD, r.saves(), ra);
        }
        spawn_b(fx, ARC, r.saves(), 0x01E0_192C);
        halfway(fx, sp, &mut r, 4, [0x01E0_1944, 0x01E0_1958]);
        for k in 0..8u64 {
            r.s[0] = k;
            spawn_a(fx, RED, r.saves(), 0x01E0_19A8);
        }
        for k in 0..0x14u64 {
            r.s[0] = k;
            spawn_a(fx, DARK, r.saves(), 0x01E0_19D0);
        }
        r.s[0] = 0x14;
        halfway(fx, sp, &mut r, 6, [0x01E0_19F8, 0x01E0_1A0C]);
        for k in 0..8u64 {
            r.s[0] = k;
            spawn_a(fx, RED, r.saves(), 0x01E0_1A5C);
        }
        r.s[0] = 8;
        spawn_a(fx, CAMERA, r.saves(), 0x01E0_1A80);
    }
    if c(fx) == 0x69 || c(fx) == 0x9F {
        for (node_ra, ra) in [(0x01E0_1AB4u32, 0x01E0_1AE0u32), (0x01E0_1B08, 0x01E0_1B34)] {
            at_node1(fx, &r, true, node_ra);
            for k in 0..0xDu64 {
                r.s[0] = k;
                spawn_b(fx, WHITE, r.saves(), ra);
            }
            r.s[0] = 0xD;
        }
    }
    if c(fx) < 6 {
        spawn_b(fx, FLASH, r.saves(), 0x01E0_1B6C);
    }
    if (0x2F..0x58).contains(&c(fx)) {
        at_node1(fx, &r, true, 0x01E0_1BA0);
        for k in 0..5u64 {
            r.s[0] = k;
            spawn_b(fx, WHITE_B, r.saves(), 0x01E0_1BCC);
        }
        r.s[0] = 5;
    }
    if c(fx) < 0x19 {
        at_node1(fx, &r, false, 0x01E0_1C08);
        spawn_b(fx, FLARE, r.saves(), 0x01E0_1C1C);
    }
    if (0x84..0xA1).contains(&c(fx)) {
        at_node1(fx, &r, false, 0x01E0_1C50);
        spawn_b(fx, FLARE, r.saves(), 0x01E0_1C64);
    }
    let a = fx.mem.i16(ANGLE);
    let s = l2::sin_deg(fx, SINES, a, 0);
    fx.mem.setf(WAVE, ee::mul(100.0, s));
    let a = fx.mem.i16(ANGLE) + 5;
    fx.mem.set16(ANGLE, a);
    let a = fx.mem.i16(ANGLE);
    if a >= 0x168 {
        let m = fx.div(a, 0x168).1;
        fx.mem.set16(ANGLE, m);
    }
    fx.leave(0x80);
}

/// 0x1E01CE0 (D, task): b567's trail in purple (0x80, 0x20, 0xFF), the
/// trails of nodes 1 (+0x6AF0) / 7 (+0x6B90) to update 0x9F, alpha 0x80 to
/// update 0x9F then down by 0x1D, length +6 10.
fn trail(fx: &mut Fx, t: u32) {
    let sp = fx.sp;
    l2::rgb(fx, t + 0x28, 0x80, 0x20, 0xFF);
    let mut s01 = [u64::from(D), 0u64];
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 5);
        if fx.mem.i16(t + 0x30) == 0 {
            fx.mem.set16(t + 0xE, 0x1E);
            for (slot, buf, ra) in [(NODES, TRAIL_Q, 0x01E0_1D54u32), (NODES + 2, TRAIL_P, 0x01E0_1DA4)] {
                let h = fx.self_handle();
                let s = fx.mem.i16(slot);
                node_position_saving(fx, h, s, sp + 0x50, s01[0], Some(s01[1]), ra);
                for k in 0..9u32 {
                    for c in 0..3 {
                        let w = fx.mem.read_u32(sp + 0x50 + 4 * c);
                        fx.mem.write_u32(buf + 0x10 * k + 4 * c, w);
                    }
                }
                s01 = [u64::from(sp + 0x54), u64::from(sp + 0x58)];
            }
        }
        if fx.mem.i16(t + 0x30) == 1 {
            fx.mem.set16(t + 0xE, 7);
        }
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(COUNTER) < 0xA0 {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0x1F {
            fx.mem.write_u8(t + 0x2B, a - 0x1D);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    if fx.mem.i16(t + 0x30) == 0 && fx.mem.i16(COUNTER) < 0xA0 {
        for (slot, buf, ra) in [(NODES, TRAIL_Q, 0x01E0_1E68u32), (NODES + 2, TRAIL_P, 0x01E0_1E94)] {
            let h = fx.self_handle();
            let s = fx.mem.i16(slot);
            node_position_saving(fx, h, s, sp + 0x50, s01[0], Some(s01[1]), ra);
            history(fx, buf + 0x90, 0xA, sp + 0x50);
        }
    }
    fx.mem.set16(t + 6, 0xA);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// 0x1E01EE0 (`dark`: size 10, (0x28, 0x10, 8), growth 3..18, row 0..9,
/// life 5..11; in by 0x38 under 0x71, out by 10) / 0x1E02090 (`red`: size
/// 6, purple (0x80, 0x40, 0xFF), growth 3..18 from the start, row 0..9,
/// life 10..29; 0x80 during the life, out by 10) (D, task): b567's
/// 0x40-byte rings.
fn ring_a(fx: &mut Fx, t: u32, red: bool) {
    if fx.mem.i16(t + 0x36) == 1 {
        if red {
            fx.mem.set16(t + 0x38, 6);
            let v = fx.rand_from(t, 0x01E0_20B8);
            fx.mem.set16(t + 0x34, v % 4);
            l2::rgb(fx, t + 0x28, 0x80, 0x40, 0xFF);
            let v = rmod(fx, t, 0x01E0_20EC, 0xFFFF);
            fx.mem.set32(t + 0x18, v);
            let v = rmod(fx, t, 0x01E0_2108, 0xFFFF);
            fx.mem.set32(t + 0x10, v);
            fx.mem.set16(t + 0x30, 0);
            let v = rmod(fx, t, 0x01E0_2128, 0x168);
            fx.mem.set16(t + 0x3A, v);
            let v = fx.rand_from(t, 0x01E0_2144);
            fx.mem.set16(t + 0xE, v % 0x10 + 3);
            let v = rmod(fx, t, 0x01E0_2164, 0xA);
            fx.mem.set16(t + 8, v);
            let v = rmod(fx, t, 0x01E0_2180, 0x14);
            fx.mem.set16(t + 6, v + 0xA);
        } else {
            fx.mem.set16(t + 0x38, 0xA);
            let v = fx.rand_from(t, 0x01E0_1F08);
            fx.mem.set16(t + 0x34, v % 4);
            for (o, ra) in [(0x10u32, 0x01E0_1F24u32), (0x14, 0x01E0_1F40), (0x18, 0x01E0_1F5C)] {
                let v = rmod(fx, t, ra, 0xFFFF);
                fx.mem.set32(t + o, v);
            }
            l2::rgb(fx, t + 0x28, 0x28, 0x10, 8);
            fx.mem.set16(t + 0x30, 0);
            let v = rmod(fx, t, 0x01E0_1F88, 0x168);
            fx.mem.set16(t + 0x3A, v);
            let v = fx.rand_from(t, 0x01E0_1FA4);
            fx.mem.set16(t + 0xE, v % 0x10 + 3);
            let v = rmod(fx, t, 0x01E0_1FC4, 0xA);
            fx.mem.set16(t + 8, v);
            let v = rmod(fx, t, 0x01E0_1FE0, 7);
            fx.mem.set16(t + 6, v + 5);
        }
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if red {
            fx.mem.write_u8(t + 0x2B, 0x80);
        } else if a < 0x71 {
            fx.mem.write_u8(t + 0x2B, a + 0x38);
        }
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x2B, a - 0xA);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    if red || fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// 0x1E02CA0 (D, task): b567's camera light at +0x6AA0 (word for word).
fn camera(fx: &mut Fx, t: u32) {
    let sp = fx.sp;
    if fx.mem.i16(t + 0x36) == 1 {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x4080_0000), (8, 0x4000_0000), (0xC, 0), (0x20, 0), (0x24, 0x42F0_0000), (0x28, 0)] {
            fx.mem.write_u32(CAM + o, v);
        }
        for k in 0..3 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(CAM + 0x10 + 4 * k, v);
        }
        fx.mem.set16(t + 0x36, 2);
        fx.mem.set16(t + 0x30, 0);
    }
    let w = fx.mem.f(CAM + 0x28);
    if fx.mem.i16(t + 0x30) == 0 {
        if w < 1.0 {
            fx.mem.setf(CAM + 0x28, ee::add(w, f32::from_bits(0x3E4C_CCCD)));
        } else {
            fx.mem.set16(t + 0x30, 1);
        }
    } else if w <= f32::from_bits(0x3DCC_CCCD) {
        fx.mem.set16(t + 0x36, 0);
    } else {
        fx.mem.setf(CAM + 0x28, ee::sub(w, f32::from_bits(0x3DCC_CCCD)));
    }
    let (p, ra) = if fx.mem.i16(t + 0x36) == 0 { (0, 0x01E0_2DD4) } else { (CAM, 0x01E0_2DE8) };
    camera_slots45(fx, p);
    // 0x1F7E20's frame (s0 = the task, ra).
    fx.saved(sp - 0x20, &[(0, u64::from(t)), (0x10, ra)]);
    if fx.mem.i32(TIMER) == 1 {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// 0x1E02220 (D, task): b567's small quad in purple (0x80, 0x40, 0xFF).
/// `flashes` the entry's s1.
fn fly(fx: &mut Fx, t: u32, flashes: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = fx.rand_from(t, 0x01E0_2244);
        fx.mem.set16(t + 8, v % 8 + 4);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_226C);
        fx.mem.set16(t + 0x56, (v >> 4) % 3);
        l2::rgb(fx, t + 0x60, 0x80, 0x40, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        for (o, ra) in [(0x34u32, 0x01E0_22A0u32), (0x30, 0x01E0_22CC), (0x38, 0x01E0_22F8)] {
            let v = rmod(fx, t, ra, 0x64);
            fx.mem.setf(t + o, ee::from_i32(v - 0x32));
        }
        for o in [0x34u32, 0x30, 0x38] {
            let q = ee::div(fx.mem.f(t + o), 60.0);
            fx.mem.setf(t + o, q);
        }
        let v = rmod(fx, t, 0x01E0_2350, 0x168);
        fx.mem.set16(t + 0x5C, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(flashes), 0x01E0_236C);
        for (o, ra) in [(4u32, 0x01E0_2374u32), (2, 0x01E0_2394), (0, 0x01E0_23B4)] {
            let v = fx.rand_from(t, ra);
            fx.mem.set16(t + o, v % 0x800 - 0x400);
        }
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0x14 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [0u32, 4, 8] {
            let v = ee::add(fx.mem.f(t + 0x10 + o), fx.mem.f(t + 0x30 + o));
            fx.mem.setf(t + 0x10 + o, v);
        }
    }
    for (o, by) in [(0x28u32, 4u32), (0x24, 2), (0x20, 0)] {
        let v = fx.mem.read_u32(t + o).wrapping_add(fx.mem.i16(t + by) as u32);
        fx.mem.write_u32(t + o, v);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(flashes), 0x01E0_2490);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E024B0 (D, task): b567's arc ring (radius 180) in (0xFF, 0x40,
/// 0x80), its second colour (0xFF, 0x40, 0x40).
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x4334_0000);
        l2::rgb(fx, t + 0x64, 0xFF, 0x40, 0x40);
        l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x80);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let a = fx.mem.read_u8(t + 0x67);
    if fx.mem.i16(t + 0x54) < 8 {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x67, a + 0x60);
        }
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x67, a - 0xF);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2578);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = ee::add(fx.mem.f(t + 0x4C), f32::from_bits(0x3ECC_CCCD));
        fx.mem.setf(t + 0x4C, r);
    }
    if fx.mem.i16(COUNTER) == 0xC3 {
        fx.mem.set16(t + 0x58, 0);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E025F0 / 0x1E027B0 (`b`) (D, task): b567's white quad of size 130,
/// cell row 2..4, thrown 8.6 along a random heading and drifting 0.8 (0.6
/// for `b`) a update along it; in by 0x40 under 0x80 for 10 updates, out
/// by 9.
fn white(fx: &mut Fx, t: u32, b: bool) {
    let o = if b { 0x1C0 } else { 0 };
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x82);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_2628 + o);
        fx.mem.set16(t + 0x56, (v >> 4) % 3 + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = fx.rand_from(t, 0x01E0_2660 + o);
        fx.mem.set16(t + 0x5A, (v >> 4) % 0x168);
        step_along(fx, t, f32::from_bits(0x4109_999A));
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0xA {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        step_along(fx, t, f32::from_bits(if b { 0x3F19_999A } else { 0x3F4C_CCCD }));
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_278C + o);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// z += by sin(+0x5A), x += by cos(+0x5A) (D+0x7010's sines).
fn step_along(fx: &mut Fx, t: u32, by: f32) {
    let a = fx.mem.i16(t + 0x5A);
    let s = l2::sin_deg(fx, SINES, a, 0);
    let z = ee::add(fx.mem.f(t + 0x18), ee::mul(by, s));
    fx.mem.setf(t + 0x18, z);
    let a = fx.mem.i16(t + 0x5A);
    let c = l2::sin_deg(fx, SINES, a, 1);
    let x = ee::add(fx.mem.f(t + 0x10), ee::mul(by, c));
    fx.mem.setf(t + 0x10, x);
}

/// 0x1E02970 (D, task): a grey ring at the attacker's node 1 (cell row
/// 0..6, +0x48 10, random angles +0x5A / +0x5C, +0xA rand % 8, +0xC the
/// spawn kind, +0x50 50..149, +0x52 300..749, +0xE 10..29, +8 0..34, +6
/// 3..7), in by 0x20 under 0x80 for 5 updates, out by 8, (0x80, 0x80,
/// 0x80) on odd updates and (0xC0, 0xC0, 0xC0) on even ones.
fn flare(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        let v = rmod(fx, t, 0x01E0_29A0, 7);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, 0x01E0_29CC, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_29E8, 0x168);
        fx.mem.set16(t + 0x5C, v);
        let v = fx.rand_from(t, 0x01E0_2A04);
        fx.mem.set16(t + 0xA, v % 8);
        let w = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, w);
        let v = fx.rand_from(t, 0x01E0_2A28);
        let m = fx.div(v >> 4, 0x64).1;
        fx.mem.set16(t + 0x50, m + 0x32);
        let v = rmod(fx, t, 0x01E0_2A4C, 0x1C2);
        fx.mem.set16(t + 0x52, v + 0x12C);
        let v = rmod(fx, t, 0x01E0_2A6C, 0x14);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = rmod(fx, t, 0x01E0_2A8C, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_2AA8, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 5 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x20);
        }
    } else if a >= 0xA {
        fx.mem.write_u8(t + 0x63, a - 8);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    let g = if fx.mem.i16(t + 0x54) % 2 == 1 { 0x80 } else { 0xC0 };
    l2::rgb(fx, t + 0x60, g, g, g);
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES);
    node_position_saving(fx, h, slot, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_2B84);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E02BB0 (D, task): a white quad of size 100 (row 0, a random heading
/// +0x5A), in by 0x40 under 0x80 for 7 updates, out by 0xB, turned by the
/// camera's angles. `flashes` the entry's s1.
fn quad(fx: &mut Fx, t: u32, flashes: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x64);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        fx.mem.set16(t + 0x56, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = fx.rand_from(t, 0x01E0_2C00);
        let m = fx.div(v >> 4, 0x168).1;
        fx.mem.set16(t + 0x5A, m);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 7 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a >= 0xD {
        fx.mem.write_u8(t + 0x63, a - 0xB);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(flashes), 0x01E0_2C74);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E030B0(D, t): b564's flash step in grey (0x30, 0x30, 0x30), the
/// descriptor at +0x6DB8: alpha up by 9 below 0x80 to update 0xB3, then
/// from its age 41 down by 9. `regs` the unit flash's s0 / s1 (the slot
/// index, the live count).
fn flash(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_43F4), (0x20, u64::from(regs[1])), (0x10, u64::from(regs[0]))]);
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
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(COUNTER) < 0xB4 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 9);
        }
    } else if fx.mem.i16(t + 0x54) >= 0x29 {
        if a >= 0xB {
            fx.mem.write_u8(t + 0x63, a - 9);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x40);
}

/// 0x1E04320(D) (b564's 0x1E036B0 at b567's offsets): in a packet layer
/// the attacker drawn into the target (0x1F8270 filter 0; ALPHA 0x48),
/// TEXFLUSH before and after; the flash tasks among pool B stepped; the
/// layer linked at key 0 when any task was alive.
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
    let mut live = 0u32;
    for k in 0..TASKS_B {
        let t = POOL_B + 0x70 * k;
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

/// 0x1E03E90(D, task, p, q): b564's tube (b701's 0x1E037D0 with the z
/// radius shifted back by 0.6 of itself: rings 0..18 of four gouraud strips
/// nudged 0.2) but with b701's far-Z depth (0x109C60(d, 1), key 0) instead
/// of the mean-w key.
fn tube(fx: &mut Fx, t: u32, p: u32, q: u32) {
    let sp = fx.enter(0x220);
    let d = sp + 0x188;
    let n = fx.mem.i16(t + 6);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    for k in 0..4 {
        let b = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(sp + 0x21C + k, b);
    }
    let a = sp + 0x160;
    for k in 0..4 {
        let w = fx.mem.read_u32(p + 4 * k);
        fx.mem.write_u32(a + 4 * k, w);
        let w = fx.mem.read_u32(q + 4 * k);
        fx.mem.write_u32(sp + 0x170 + 4 * k, w);
    }
    for c in 0..3 {
        let v = ee::sub(fx.mem.f(a + 4 * c), fx.mem.f(sp + 0x170 + 4 * c));
        fx.mem.setf(a + 4 * c, v);
    }
    let w = fx.mem.read_u32(D);
    fx.mem.write_u32(a, w);
    let y = ee::neg(fx.mem.f(a + 4));
    fx.mem.setf(a + 4, y);
    let w = fx.mem.read_u32(D + 8);
    fx.mem.write_u32(a + 8, w);
    let f25 = fx.mem.f(a + 8);
    let f24 = fx.mem.f(a + 4);
    let f23 = ee::mul(f32::from_bits(0x3F19_999A), f25);
    let mut seg = p;
    let mut i = 0;
    while i < n - 1 {
        l2::colour_strip4(fx, d, sp + 0x21C, 1);
        let alpha = i32::from(fx.mem.read_u8(t + 0x2B));
        let q = fx.div(alpha, n).0 & 0xFF;
        let v = fx.mem.read_u8(sp + 0x21F).wrapping_sub(q as u8);
        fx.mem.write_u8(sp + 0x21F, v);
        l2::colour_strip4(fx, d, sp + 0x21C, 2);
        let mut deg = -0x5Ai32;
        for _ in 0..0x12 {
            let s = l2::sin_deg(fx, D + 0xD8, deg as i16 as i32, 0);
            let f21 = ee::mul(fx.mem.f(a), s);
            let c = l2::sin_deg(fx, D + 0xD8, deg as i16 as i32, 1);
            let f22 = ee::sub(ee::mul(f25, c), f23);
            for s0 in 0..4 {
                let m = sp + 0x120;
                for (row, from) in [(0u32, seg), (0x10, seg), (0x20, seg + 0x10), (0x30, seg + 0x10)] {
                    for c in 0..3 {
                        let w = fx.mem.read_u32(from + 4 * c);
                        fx.mem.write_u32(m + row + 4 * c, w);
                    }
                }
                l2::add_column(fx, m, 0, f21);
                l2::add_column(fx, m, 2, f22);
                l2::add_column(fx, m, 1, f24);
                let nudge = f32::from_bits(0x3E4C_CCCD);
                let (o, up) = match s0 {
                    0 => (8u32, true),
                    1 => (8, false),
                    2 => (0, true),
                    _ => (0, false),
                };
                for row in [0u32, 0x20] {
                    let at = m + row + o;
                    let v = fx.mem.f(at);
                    fx.mem.setf(at, if up { ee::add(v, nudge) } else { ee::sub(v, nudge) });
                }
                if l2::project(fx, m, SPR, 0, d, sp + 0x110) {
                    l2::far_z_desc(fx, d, 1);
                    fx.submit(d, OT, 0);
                }
            }
            deg += 0xA;
        }
        seg += 0x10;
        i += 1;
    }
    fx.leave(0x220);
}
