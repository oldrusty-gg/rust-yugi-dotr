//! BMODULE b149 (card 555), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E023B0, data area 0x1ED0000) [F]: b228's task program (card 377) and globals word for word - 150
//! tasks of 0x70 bytes at +0x2A0 (alive +0x58, step +0x44 through `jalr`;
//! spawned by 0x1E02BA0 = `spawn::task70` on 4ac5b35fe8b8 (colour +0xBC94,
//! slot +0xBCEA into +0x50)), with a 190-update timer +0xBC90, the
//! attacker's nodes 1 / 0x19 / 0x1C and textures (4, 0xA..0xF), (4, 0)
//! then (5, 0..1). Every live step is drawn by `quads4::ring_facing_flat`
//! on the second texture. The emitter 0x1E01360 counts +0xBCF2: on
//! 0x24..0x46 two grey smoke rings a update (0x1E01770) at node 0x19 (odd
//! updates) / 0x1C (even), +-20 along x / z; on 0x24..0x64 two pairs of
//! grey rings (0x1E01C60) a update, one at the last point +-10, one at node
//! 1 +-10. Both bob by 7 sin(+0x5C) and rise for 12 updates; at age 0x48
//! each leaves a wide ring (0x1E01AC0) at its point. The smoke ring
//! drifts along z by 4.7 (toward the facing), sinks 1.1 and widens from
//! age 0x1F, spins and grows from 0x47. Returns 1 on update 0x55, 4 on
//! 0xB4, 2 on 0xA5, 6 on 0x60..0x9A, 8 once the timer has run out.
//!
//! The steps 0x1E01ED0 / 0x1E020F0 (and so the tube 0x1E02D30 and the band
//! 0x1E009B0 that draw them) are tested by the entry but never spawned
//! [F: the spawns name 0x1E01770 / 0x1E01C60 / 0x1E01AC0 only]; not ported.
use crate::b076::node_position_saving;
use crate::blib::spawn;
use crate::blib::{quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const SMOKE: u32 = 0x01E0_1770;
const WIDE: u32 = 0x01E0_1AC0;
const RING: u32 = 0x01E0_1C60;
const UNSPAWNED: [u32; 2] = [0x01E0_1ED0, 0x01E0_20F0];
/// The return address of the entry's `jalr` into a step (0x1E026B0 + 8).
const TASK_RA: u64 = 0x01E0_26B8;
const SINES: u32 = D + 0xBEF4;
const TEX: u32 = D + 0xBC70;
const TIMER: u32 = D + 0xBC90;
const NODE1: u32 = D + 0xBCBC;
const SLOT: u32 = D + 0xBCEA;
const NEXT: u32 = D + 0xBCEC;
const YAW: u32 = D + 0xBCEE;
const COUNTER: u32 = D + 0xBCF2;
const BYTES: u32 = D + 0xBCF4;
const BYTES_B: u32 = D + 0xBDF4;
const POS: u32 = D + 0x250;

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// rand (the caller's s0 = `s0`) from `ra`, mod `m` (truncated).
fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E023B0 (a0 = D, a1 = param; s0 = D, s1 = 0, s2 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    fx.mem.write_u32(D + 0xBC9C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xC060);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xBCF0, param);
    fx.mem.set32(TIMER, 0xBE);
    for (at, n, clear) in [(NODE1, 1, 1), (D + 0xBCBE, 0x19, 0), (D + 0xBCC0, 0x1C, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(at, r);
    }
    let t = fx.target_handle();
    let r = fx.request_node(t, 1, 1);
    fx.mem.set16(D + 0xBCCA, r);
    for k in 0xAu32..0x10 {
        let tex = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x260 + 4 * k, tex);
    }
    let tex = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, tex);
    for k in 0..2u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.fix_residue(v, 0x01E0_2550);
    fx.mem.set16(YAW, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2580, 0x1E);
        fx.mem.write_u8(BYTES + k, (v - 0xF) as u8);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_25DC);
        fx.mem.write_u8(BYTES + k, v.wrapping_rem(4) as u8);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2634, 0x28);
        fx.mem.write_u8(BYTES_B + k, (v - 0x14) as u8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 {
            continue;
        }
        // The steps' prologues: the entry's s0 = D, s1 = 0.
        let frame = match f {
            SMOKE | RING => 0x30,
            WIDE => 0x20,
            _ => panic!("b149: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        match f {
            WIDE => fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]),
            _ => fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]),
        }
        // The spawns' registers in a step: s0 = the task, s1 = D, the
        // entry's s2 = the slot, s3 = its index, s4 the runner's (0).
        let r: Regs = [u64::from(t), u64::from(D), u64::from(slot), u64::from(k), 0];
        match f {
            SMOKE => smoke(fx, t, r),
            WIDE => wide(fx, t),
            _ => ring(fx, t, r),
        }
        fx.leave(frame);
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        draw(fx, sp, t);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x55 => 1,
        0xB4 => 4,
        0xA5 => 2,
        0x60..=0x9A => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The entry's draw of one live task by its step (s0 = D): the atlas cell
/// 0x14 frame +0x56 into D+0x94, the second texture, 6 a ring, the task's
/// +0xA at D+0xC0 and its point at D+0x20 (the smoke / grey rings bobbing
/// by 7 sin(+0x5C) on the first sine table), `ring_facing_flat` on BYTES.
fn draw(fx: &mut Fx, sp: u32, t: u32) {
    let f = fx.mem.read_u32(t + 0x44);
    if UNSPAWNED.contains(&f) {
        panic!("b149: unported draw case {f:#x}");
    }
    for (g, r) in [(SMOKE, 0x60u32), (WIDE, 0x70), (RING, 0x80)] {
        if fx.mem.read_u32(t + 0x44) != g {
            continue;
        }
        let frame = fx.mem.i16(t + 0x56);
        l2::rect(fx, sp + r, 0x14, frame);
        for k in 0..4 {
            let v = fx.mem.read_u32(sp + r + 4 * k);
            fx.mem.write_u32(D + 0x94 + 4 * k, v);
        }
        let w = fx.mem.read_u32(TEX + 4);
        fx.mem.write_u32(D + 0x88, w);
        fx.mem.set16(D + 0xB8, 6);
        let v = fx.mem.i16(t + 0xA);
        fx.mem.set16(D + 0xC0, v);
        let x = fx.mem.read_u32(t + 0x10);
        fx.mem.write_u32(D + 0x20, x);
        if g == WIDE {
            let y = fx.mem.read_u32(t + 0x14);
            fx.mem.write_u32(D + 0x24, y);
        } else {
            let a = fx.mem.i16(t + 0x5C);
            let s = l2::sin_deg(fx, D + 0xD8, a, 0);
            let y = ee::add(fx.mem.f(t + 0x14), ee::mul(7.0, s));
            fx.mem.setf(D + 0x24, y);
        }
        let z = fx.mem.read_u32(t + 0x18);
        fx.mem.write_u32(D + 0x28, z);
        quads4::ring_facing_flat(fx, uv::uv_rect, D, t, 0x20, BYTES);
        fx.mem.set16(D + 0xB8, 0);
    }
}

/// The spawn's registers: s0..s4 at the call.
type Regs = [u64; 5];

/// 0x1E02BA0(D, step, 0): `spawn::task70` on this record's layout
/// (4ac5b35fe8b8: 150 tasks, colour +0xBC94, slot +0xBCEA).
fn spawn(fx: &mut Fx, step: u32, r: Regs, ra: u32) -> i32 {
    let p = spawn::pool70_b4("4ac5b35fe8b8").expect("layout");
    let saves = [(0x50, u64::from(ra)), (0x40, r[4]), (0x30, r[3]), (0x20, r[2]), (0x10, r[1]), (0, r[0])];
    spawn::task70(fx, D, p, step, 0, &saves)
}

/// D+0x250's x / y / z (`axes`) moved by rand % m - h each (rand with the
/// emitter's s0 = `s0`, from the return addresses `ras`).
fn jitter(fx: &mut Fx, s0: u32, axes: &[u32], ras: &[u32], m: i32, h: i32) {
    for (&o, &ra) in axes.iter().zip(ras) {
        let v = rmod(fx, s0, ra, m) - h;
        let x = ee::add(fx.mem.f(POS + o), ee::from_i32(v));
        fx.mem.setf(POS + o, x);
    }
}

/// 0x1E01360(D, 0) (frame 0x40: ra, the entry's s2 = param, s1 = 0, s0 =
/// D; s2 = D, s0 / s1 the loop counters): the update counter +0xBCF2 and
/// the spawns.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, 0x01E0_2688), (0x20, p), (0x10, 0), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = fx.mem.i16(COUNTER);
    if (0x24..0x47).contains(&c) {
        for k in 0..2u32 {
            let (s0, s1) = (k, 2u64);
            let odd = fx.mem.i16(COUNTER) % 2 == 1;
            let h = fx.self_handle();
            if odd {
                let s = fx.mem.i16(D + 0xBCBE);
                node_position_saving(fx, h, s, POS, u64::from(s0), Some(s1), 0x01E0_1434);
            } else {
                let s = fx.mem.i16(D + 0xBCC0);
                node_position_saving(fx, h, s, POS, u64::from(s0), Some(s1), 0x01E0_145C);
            }
            jitter(fx, s0, &[0, 8], &[0x01E0_1468, 0x01E0_149C], 0x28, 0x14);
            let v = fx.mem.i16(NEXT);
            fx.mem.set16(SLOT, v);
            spawn(fx, SMOKE, [u64::from(s0), s1, u64::from(D), 0, 0], 0x01E0_14F0);
            let n = fx.mem.i16(NEXT) + 1;
            fx.mem.set16(NEXT, n);
            if fx.mem.i16(NEXT) >= 0x78 {
                fx.mem.set16(NEXT, 0);
            }
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x24..0x65).contains(&c) {
        for k in 0..2u32 {
            let r: Regs = [2, u64::from(k), u64::from(D), 0, 0];
            jitter(fx, 2, &[0, 4, 8], &[0x01E0_15BC, 0x01E0_15F0, 0x01E0_1624], 0x14, 0xA);
            spawn(fx, RING, r, 0x01E0_1660);
            let h = fx.self_handle();
            let s = fx.mem.i16(NODE1);
            node_position_saving(fx, h, s, POS, 2, Some(u64::from(k)), 0x01E0_1680);
            jitter(fx, 2, &[0, 4, 8], &[0x01E0_1688, 0x01E0_16BC, 0x01E0_16F0], 0x14, 0xA);
            spawn(fx, RING, r, 0x01E0_172C);
        }
    }
    fx.leave(0x40);
}

/// The task's point +0x10..+0x1F copied to D+0x250 and a wide ring
/// spawned there.
fn leave_wide(fx: &mut Fx, t: u32, r: Regs, ra: u32) {
    for o in 0..4 {
        let w = fx.mem.read_u32(t + 0x10 + 4 * o);
        fx.mem.write_u32(POS + 4 * o, w);
    }
    spawn(fx, WIDE, r, ra);
}

/// The smoke / grey rings' first step (frame 0x30: s0 = the task): size
/// `size`, cell 0..6, grey, a random angle, +0xA rand % 8, a rise +0x34 of
/// -1.5..1.4, +0xC the slot, radius 10..29 growing by 15 to 100..249,
/// growth +0xE 40..79, row 0..34, life 3..7; `ras` the rand return
/// addresses.
fn ring_first(fx: &mut Fx, t: u32, size: u32, ras: &[u32; 9]) {
    fx.mem.write_u32(t + 0x48, size);
    let v = rmod(fx, t, ras[0], 7);
    fx.mem.set16(t + 0x56, v);
    l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
    let v = rmod(fx, t, ras[1], 0x168);
    fx.mem.set16(t + 0x5A, v);
    let v = fx.rand_from(t, ras[2]);
    fx.mem.set16(t + 0xA, v.wrapping_rem(8));
    let v = rmod(fx, t, ras[3], 0x1E) - 0xF;
    fx.mem.setf(t + 0x34, ee::from_i32(v));
    let v = ee::div(fx.mem.f(t + 0x34), 10.0);
    fx.mem.setf(t + 0x34, v);
    let v = fx.mem.i16(t + 0x50);
    fx.mem.set16(t + 0xC, v);
    let v = rmod(fx, t, ras[4], 0x14);
    fx.mem.set16(t + 0x50, v + 0xA);
    let v = rmod(fx, t, ras[5], 0x96);
    fx.mem.set16(t + 0x52, v + 0x64);
    let v = rmod(fx, t, ras[6], 0x28);
    fx.mem.set16(t + 0xE, v + 0x28);
    let v = rmod(fx, t, ras[7], 0x23);
    fx.mem.set16(t + 8, v);
    let v = rmod(fx, t, ras[8], 5);
    fx.mem.set16(t + 6, v + 3);
    fx.mem.set16(t + 0x58, 2);
}

/// +0x63 in by 0x20 below 0x80 while `rising`, else out by 8 (the task
/// ends below 10).
fn fade(fx: &mut Fx, t: u32, rising: bool) {
    let a = fx.mem.read_u8(t + 0x63);
    if rising {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x20);
        }
    } else if a >= 0xA {
        fx.mem.write_u8(t + 0x63, a - 8);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// For 12 updates the radius +0x50 grows by 15 to +0x52 and the point
/// rises by +0x34.
fn rise(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x54) < 0xC {
        let r = fx.mem.i16(t + 0x50);
        if r < fx.mem.i16(t + 0x52) {
            fx.mem.set16(t + 0x50, r + 0xF);
        }
        let y = ee::add(fx.mem.f(t + 0x14), fx.mem.f(t + 0x34));
        fx.mem.setf(t + 0x14, y);
    }
}

/// 0x1E01770 (D, task) (frame 0x30: s0 = the task, s1 = D): a grey smoke
/// ring (size 10); in by 0x20 for 0x50 updates, then out by 8; a wide ring
/// at age 0x48; the bob +0x5C up by 7; on 0x1F..0x45 4.7 along z (- when
/// the attacker's yaw is non-zero), 1.1 down and the size up by 0.5; from
/// 0x47 the angle up by 5, the radius by +0xE and the size by 3.
fn smoke(fx: &mut Fx, t: u32, r: Regs) {
    if fx.mem.i16(t + 0x58) == 1 {
        let ras = [0x01E0_17A0, 0x01E0_17CC, 0x01E0_17E8, 0x01E0_1804, 0x01E0_1844, 0x01E0_1864, 0x01E0_1884, 0x01E0_18A4, 0x01E0_18C0];
        ring_first(fx, t, 0x4120_0000, &ras);
    }
    let rising = fx.mem.i16(t + 0x54) < 0x50;
    fade(fx, t, rising);
    if fx.mem.i16(t + 0x54) == 0x48 {
        leave_wide(fx, t, r, 0x01E0_1974);
    }
    rise(fx, t);
    let v = fx.mem.i16(t + 0x5C) + 7;
    fx.mem.set16(t + 0x5C, v);
    let a = fx.mem.i16(t + 0x54);
    if (0x1F..0x46).contains(&a) {
        let z = fx.mem.f(t + 0x18);
        let z = if fx.mem.i16(YAW) != 0 { ee::sub(z, f32::from_bits(0x4096_6666)) } else { ee::add(z, f32::from_bits(0x4096_6666)) };
        fx.mem.setf(t + 0x18, z);
        let y = ee::sub(fx.mem.f(t + 0x14), f32::from_bits(0x3F8C_CCCD));
        fx.mem.setf(t + 0x14, y);
        let s = ee::add(fx.mem.f(t + 0x48), 0.5);
        fx.mem.setf(t + 0x48, s);
    }
    if fx.mem.i16(t + 0x54) >= 0x47 {
        let v = fx.mem.i16(t + 0x5A) + 5;
        fx.mem.set16(t + 0x5A, v);
        let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, v);
        let s = ee::add(fx.mem.f(t + 0x48), 3.0);
        fx.mem.setf(t + 0x48, s);
    }
    age(fx, t);
}

/// 0x1E01C60 (D, task) (frame 0x30: s0 = the task, s1 = D): a grey ring
/// (size 20) in by 0x20 for 3 updates, then out by 8; a wide ring at age
/// 0x48; the bob +0x5C up by 7.
fn ring(fx: &mut Fx, t: u32, r: Regs) {
    if fx.mem.i16(t + 0x58) == 1 {
        let ras = [0x01E0_1C90, 0x01E0_1CBC, 0x01E0_1CD8, 0x01E0_1CF4, 0x01E0_1D34, 0x01E0_1D54, 0x01E0_1D74, 0x01E0_1D94, 0x01E0_1DB0];
        ring_first(fx, t, 0x41A0_0000, &ras);
    }
    let rising = fx.mem.i16(t + 0x54) < 3;
    fade(fx, t, rising);
    if fx.mem.i16(t + 0x54) == 0x48 {
        leave_wide(fx, t, r, 0x01E0_1E64);
    }
    rise(fx, t);
    let v = fx.mem.i16(t + 0x5C) + 7;
    fx.mem.set16(t + 0x5C, v);
    age(fx, t);
}

/// 0x1E01AC0 (D, task) (frame 0x20: s0 = the task): a wide grey ring (size
/// 40, cell rand % 4, a random angle, +0xA rand % 8, +0xC the slot, radius
/// 10..59 growing by 16..45 from age 1, row 0..34, life 3..7) at 0x80, then
/// out by 8.
fn wide(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4220_0000);
        let v = fx.rand_from(t, 0x01E0_1AE8);
        fx.mem.set16(t + 0x56, v.wrapping_rem(4));
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, 0x01E0_1B1C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, 0x01E0_1B38);
        fx.mem.set16(t + 0xA, v.wrapping_rem(8));
        let v = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, v);
        let v = rmod(fx, t, 0x01E0_1B5C, 0x32);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, 0x01E0_1B7C, 0x1E);
        fx.mem.set16(t + 0xE, v + 0x10);
        let v = rmod(fx, t, 0x01E0_1B9C, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_1BB8, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade(fx, t, false);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, v);
    }
    age(fx, t);
}
