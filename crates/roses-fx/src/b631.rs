//! BMODULE b631 (card 645), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01EF0, data area 0x1ED0000) [F]: a task pool of 400 sprites (0x70 bytes at +0x2A0, alive flag
//! +0x58, step function +0x44 as a record address) drawn with the second
//! library: grey motes (`billboard::quad`, 0x1E00ED0), turning sparks
//! (`quads4::quad_turned`, b0ec19f6c988 at 0x1E01190) and rings
//! (`billboard::ring_quads`, 0x1E00920), UVs by uv_rect 0x1E00080. From
//! update 41 to 169 a mote leaves each of the attacker's nodes 0x10 / 0xD /
//! 0xC a update (x jittered by -8..7); from 47 three sparks a update leave
//! node 0x10 and fly toward the facing (z by 5 a update), each spawning a
//! ring at its position on its 30th update. Returns 1 on update 80, 4 on
//! 200, 2 on 185, 6 on 91..179, 8 once the 210-update timer +0xB210 has
//! run out.
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
/// The step functions a task holds (+0x44), as the record's addresses.
const MOTE: u32 = 0x01E0_1690;
const SPARK: u32 = 0x01E0_1920;
const RING: u32 = 0x01E0_1D30;
const IDLE: u32 = 0x01E0_1EE0;
/// The return address of the entry's `jalr` into a step (0x1E021F8 + 8).
const TASK_RA: u64 = 0x01E0_2200;
const TIMER: u32 = D + 0xB210;
const COLOUR: u32 = D + 0xB214;
const COUNTER: u32 = D + 0xB2B8;
const FACING: u32 = D + 0xB2B4;
const BYTES: u32 = D + 0xB2BA;

/// The entry 0x1E01EF0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x90);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, D + 0xB4BC);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2B6, param);
    fx.mem.set32(TIMER, 0xD2);
    for (at, node, clear) in [(0xB244u32, 0x10, 1), (0xB246, 0xD, 0), (0xB248, 0xC, 0), (0xB24A, 0x30, 0), (0xB24C, 0x2F, 0), (0xB24E, 0x2C, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(D + at, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB254, r);
    for k in 0..2u32 {
        let t = fx.texture(4, 0xE + k as i32);
        fx.mem.write_u32(D + 0x298 + 4 * k, t);
    }
    for k in 0..2u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0xB1F0 + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x80);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x84)), 180.0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = fx.rand();
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand();
        fx.mem.set16(D + 0xB264 + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    for k in 0..TASKS {
        let t = D + 0x70 * k;
        if fx.mem.i16(t + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x2E4);
        if f != 0 && f != IDLE {
            run_task(fx, f, t, k);
        }
    }
    for k in 0..TASKS {
        let t = D + 0x70 * k;
        if fx.mem.i16(t + 0x2F8) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x2E4) == MOTE {
            let frame = cell(fx, t, 0xB1F0, sp + 0x50, 0x9B);
            let (u, v) = if frame < 2 { (frame << 6, 0) } else { ((frame - 2) << 6, 0x40) };
            fx.mem.set16(D + 0x94, u);
            fx.mem.set16(D + 0x96, v);
            fx.mem.set16(D + 0x98, 0x40);
            fx.mem.set16(D + 0x9A, 0x40);
            billboard::quad(fx, uv::uv_rect, D, t + 0x2A0, 0x10, t + 0x2B0);
        }
        if fx.mem.read_u32(t + 0x2E4) == SPARK {
            let frame = cell(fx, t, 0xB1F4, sp + 0x60, 0x9B);
            fx.mem.set16(D + 0x94, frame << 5);
            fx.mem.set16(D + 0x96, 0);
            fx.mem.set16(D + 0x98, 0x20);
            fx.mem.set16(D + 0x9A, 0x20);
            quads4::quad_turned(fx, uv::uv_rect, D, t + 0x2A0, 0x10, t + 0x2B0);
        }
        if fx.mem.read_u32(t + 0x2E4) == RING {
            cell(fx, t, 0xB1F4, sp + 0x70, 0x14);
            fx.mem.set16(D + 0xB8, 8);
            fx.mem.set16(D + 0xBE, 0);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t + 0x2A0, 0x10, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x50 => 1,
        0xC8 => 4,
        0xB9 => 2,
        0x5B..=0xB3 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The draw's common head for task `t` (D + 0x70 k): texture D+`tex` into
/// D+0x88, the atlas cell `id` frame +0x2F6 (0x106960 into `r`) copied to
/// D+0x94..+0xA3, orientation +0x2F2 into D+0xBE; the frame back.
fn cell(fx: &mut Fx, t: u32, tex: u32, r: u32, id: i32) -> i32 {
    let w = fx.mem.read_u32(D + tex);
    fx.mem.write_u32(D + 0x88, w);
    let frame = fx.mem.i16(t + 0x2F6);
    l2::rect(fx, r, id, frame);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
    let o = fx.mem.i16(t + 0x2F2);
    fx.mem.set16(D + 0xBE, o);
    fx.mem.i16(t + 0x2F6)
}

fn run_task(fx: &mut Fx, f: u32, slot: u32, k: u32) {
    let t = slot + 0x2A0;
    match f {
        MOTE => mote(fx, t),
        SPARK => spark(fx, t, slot, k),
        RING => ring(fx, t),
        _ => panic!("b631: unknown task step {f:#x}"),
    }
}

/// 0x1E01470(D, 0): the update counter +0xB2B8 and the emitters.
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, 0x01E0_21C0), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if (0x2F..0xAA).contains(&n) {
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + 0xB244);
        fx.node_position(h, slot, D + 0x250);
        for ra in [0x01E0_14ECu32, 0x01E0_1500, 0x01E0_1514] {
            spawn(fx, SPARK, 0, &[D, D, u32::MAX, 0, 0], ra);
        }
    }
    let n = fx.mem.i16(COUNTER);
    if (0x29..0xAA).contains(&n) {
        for (at, site, ra) in [(0xB244u32, 0x01E0_155Cu32, 0x01E0_15A4u32), (0xB246, 0x01E0_15C4, 0x01E0_160C), (0xB248, 0x01E0_162C, 0x01E0_1674)] {
            let h = fx.self_handle();
            let slot = fx.mem.i16(D + at);
            fx.node_position(h, slot, D + 0x250);
            let v = fx.rand_from(D, site + 8);
            let x = ee::add(fx.mem.f(D + 0x250), ee::from_i32(v % 16 - 8));
            fx.mem.setf(D + 0x250, x);
            spawn(fx, MOTE, 0, &[D, D, u32::MAX, 0, 0], ra);
        }
    }
    fx.leave(0x20);
}

/// 0x1E02520(D, step, arg): the first free task cleared and set off at
/// D+0x250 with the colour +0xB214 (twice), angle flag +0xB2B0; 1 when one
/// was free. `regs` the caller's s0..s4 (saved in its frame), `ra` the
/// return address.
fn spawn(fx: &mut Fx, f: u32, arg: u32, regs: &[u32; 5], ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    let r = |k: usize| if regs[k] == u32::MAX { u64::MAX } else { u64::from(regs[k]) };
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, r(4)), (0x30, r(3)), (0x20, r(2)), (0x10, r(1)), (0, r(0))]);
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
            let a = fx.mem.i16(D + 0xB2B0);
            fx.mem.set16(t + 0x50, a);
            fx.mem.write_u32(t + 0x40, arg);
            fx.mem.write_u32(t + 0x44, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// rand() from a step's `jal` at `site` (its frame: s0 = the task `t`).
fn rand_at(fx: &mut Fx, t: u32, site: u32) -> i32 {
    fx.rand_from(t, site + 8)
}

/// rand % m (HI, the remainder).
fn rand_mod(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = rand_at(fx, t, site);
    fx.div(v, m).1
}

/// (rand >> 4) % m (HI).
fn rand4_mod(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = rand_at(fx, t, site);
    fx.div(v >> 4, m).1
}

/// 0x205580(t + 0x20) from a step (its saves: ra, the step's s1 = D, s0 =
/// t, below the step's frame).
fn angles(fx: &mut Fx, t: u32, ra: u32) {
    let base = fx.sp.wrapping_sub(0x30);
    fx.saved(base, &[(0x20, u64::from(ra)), (0x10, u64::from(D)), (0, u64::from(t))]);
    l2::camera_angles_to(fx, t + 0x20);
}

/// The velocity +0x30..+0x38: rand % 100 - 50 each (+0x34, +0x30, +0x38),
/// divided by `xy` (x, y) and `z`.
fn velocity(fx: &mut Fx, t: u32, sites: [u32; 3], xy: f32, z: f32) {
    for (o, site) in [0x34u32, 0x30, 0x38].into_iter().zip(sites) {
        let v = rand_mod(fx, t, site, 0x64);
        fx.mem.setf(t + o, ee::from_i32(v - 0x32));
    }
    for (o, q) in [(0x34u32, xy), (0x30, xy), (0x38, z)] {
        let v = ee::div(fx.mem.f(t + o), q);
        fx.mem.setf(t + o, v);
    }
}

/// The spin steps +4, +2, +0: rand % 0x800 - 0x400 each.
fn spins(fx: &mut Fx, t: u32, sites: [u32; 3]) {
    for (o, site) in [4u32, 2, 0].into_iter().zip(sites) {
        let v = rand_at(fx, t, site);
        fx.mem.set16(t + o, v % 0x800 - 0x400);
    }
}

/// pos +0x10 += velocity +0x30.
fn move_by_velocity(fx: &mut Fx, t: u32) {
    for k in 0..3 {
        let p = ee::add(fx.mem.f(t + 0x10 + 4 * k), fx.mem.f(t + 0x30 + 4 * k));
        fx.mem.setf(t + 0x10 + 4 * k, p);
    }
}

/// angles +0x20.. += spins (32-bit, +0x28 by +4 first).
fn spin(fx: &mut Fx, t: u32) {
    for (a, s) in [(0x28u32, 4u32), (0x24, 2), (0x20, 0)] {
        let v = fx.mem.read_u32(t + a).wrapping_add(fx.mem.i16(t + s) as u32);
        fx.mem.write_u32(t + a, v);
    }
}

/// Alpha +0x63 down by `by` while at least `min`, else the task ends.
fn fade_out(fx: &mut Fx, t: u32, min: u8, by: u8) {
    let a = fx.mem.read_u8(t + 0x63);
    if a >= min {
        fx.mem.write_u8(t + 0x63, a - by);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// Alpha +0x63 up by 0x40 while below `max`.
fn fade_in(fx: &mut Fx, t: u32, max: u8) {
    let a = fx.mem.read_u8(t + 0x63);
    if a < max {
        fx.mem.write_u8(t + 0x63, a + 0x40);
    }
}

/// 0x1E01690: a grey mote of size 50 drifting by a random velocity / 120,
/// spinning, fading in for 5 updates then out by 9.
fn mote(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x32);
        let s = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, s);
        let v = rand4_mod(fx, t, 0x01E0_16B8, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x40, 0x40, 0x40);
        fx.mem.set16(t + 0x58, 2);
        velocity(fx, t, [0x01E0_16EC, 0x01E0_1718, 0x01E0_1744], 120.0, 120.0);
        let v = rand_mod(fx, t, 0x01E0_179C, 0x168);
        fx.mem.set16(t + 0x5C, v);
        angles(fx, t, 0x01E0_17C0);
        spins(fx, t, [0x01E0_17C0, 0x01E0_17E0, 0x01E0_1800]);
        let v = rand_at(fx, t, 0x01E0_1820);
        fx.mem.set16(t + 0x52, (v >> 4) % 4);
    }
    if fx.mem.i16(t + 0x54) < 5 {
        fade_in(fx, t, 0x80);
    } else {
        fade_out(fx, t, 0xB, 9);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        move_by_velocity(fx, t);
    }
    spin(fx, t);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E01920: a spark of size 8..23 flying by a random velocity (/ 80,
/// z / 100) and 5 a update toward the facing, turning by +0x5C (-10..9) a
/// update; fades in until its life +6 (32..41) while the counter is below
/// 180, then out; on its 30th update a ring starts at its position.
/// `slot` / `k` the entry's loop registers (s2 / s3).
fn spark(fx: &mut Fx, t: u32, slot: u32, k: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rand_at(fx, t, 0x01E0_1944);
        fx.mem.set16(t + 8, (v >> 4) % 16 + 8);
        let s = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, s);
        let v = rand4_mod(fx, t, 0x01E0_1970, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        velocity(fx, t, [0x01E0_19A4, 0x01E0_19D0, 0x01E0_19FC], 80.0, 100.0);
        let v = rand_mod(fx, t, 0x01E0_1A5C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rand4_mod(fx, t, 0x01E0_1A78, 0x14);
        fx.mem.set16(t + 0x5C, v - 0xA);
        angles(fx, t, 0x01E0_1AA4);
        spins(fx, t, [0x01E0_1AA4, 0x01E0_1AC4, 0x01E0_1AE4]);
        let v = rand_at(fx, t, 0x01E0_1B04);
        fx.mem.set16(t + 0x52, (v >> 4) % 4);
        let v = rand_mod(fx, t, 0x01E0_1B24, 0xA);
        fx.mem.set16(t + 6, v + 0x20);
    }
    if fx.mem.i16(COUNTER) < 0xB4 && fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x80);
    } else {
        fade_out(fx, t, 0xB, 9);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        move_by_velocity(fx, t);
        let z = fx.mem.f(t + 0x18);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 5.0) } else { ee::add(z, 5.0) };
        fx.mem.setf(t + 0x18, z);
    }
    spin(fx, t);
    if fx.mem.i16(COUNTER) < 0xB4 && fx.mem.i16(t + 0x54) == 0x1E {
        for k in 0..4 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * k);
            fx.mem.write_u32(D + 0x250 + 4 * k, w);
        }
        spawn(fx, RING, 0, &[t, D, slot, k, 0], 0x01E0_1CE4);
    }
    angles(fx, t, 0x01E0_1CF0);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    let a = fx.mem.i16(t + 0x5A) + fx.mem.i16(t + 0x5C);
    fx.mem.set16(t + 0x5A, a);
    fx.leave(0x30);
}

/// 0x1E01D30: a ring: quad size +0x48 5..19, a random cell frame 0..2,
/// radius +0x50 from 10..24 growing by +0xE (16..35) a update, row +8
/// 0..34, life +6 3..5; fades in by 0x40 below 0xC0, then out by 14.
fn ring(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rand_mod(fx, t, 0x01E0_1D4C, 0xF);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 5));
        let v = rand_mod(fx, t, 0x01E0_1D78, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rand_mod(fx, t, 0x01E0_1DA4, 0xF);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rand_mod(fx, t, 0x01E0_1DC4, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rand_mod(fx, t, 0x01E0_1DE0, 0x14);
        fx.mem.set16(t + 0xE, v + 0x10);
        let v = rand_mod(fx, t, 0x01E0_1E00, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, t, 0x01E0_1E1C, 3);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0xC0);
    } else {
        fade_out(fx, t, 0x10, 0xE);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}
