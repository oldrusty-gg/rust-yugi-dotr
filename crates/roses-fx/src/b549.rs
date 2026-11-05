//! BMODULE b549 (card 595), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02630, data area 0x1ED0000) [F]: b631's task pool of 400 (0x70 bytes at +0x2A0, alive +0x58, step
//! +0x44 as a record address) drawn with the second library: quads
//! (`billboard::quad`, 0x1E00ED0, UVs by uv_rect 0x1E00080), rings
//! (`billboard::ring_quads`, 0x1E00920) and facing rings
//! (`billboard::ring_facing`, 0x1E01190). Up to update 287 two pink rings
//! a update leave the attacker's nodes 0xB / 0xC (jittered, 2 toward the
//! facing); from update 120 to 287 three bubbles a update (one violet,
//! cycling its cell frame, two coloured) rise from node 9 and drift with
//! the facing, the violet ones leaving a white ring on their 61st update
//! (until 338). Returns 1 on update 130, 4 on 365, 2 on 355, 6 on 141..349,
//! 8 once the 370-update timer +0xB210 has run out.
use crate::blib::{billboard, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
/// The step functions a task holds (+0x44), as the record's addresses.
const BUBBLE: u32 = 0x01E0_1AE0;
const RING: u32 = 0x01E0_1F10;
const FACING_RING: u32 = 0x01E0_20C0;
const COLOURED: u32 = 0x01E0_2280;
const IDLE: u32 = 0x01E0_2620;
/// The return address of the entry's `jalr` into a step (0x1E02968 + 8).
const TASK_RA: u64 = 0x01E0_2970;
const TIMER: u32 = D + 0xB210;
const COLOUR: u32 = D + 0xB214;
const COUNTER: u32 = D + 0xB2B8;
const FACING: u32 = D + 0xB2B4;
const BYTES: u32 = D + 0xB2BA;
const BYTES2: u32 = D + 0xB3BA;
/// The attacker's node 9 every update.
const POINT: u32 = D + 0xB1D0;

/// The entry 0x1E02630.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xA0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, D + 0xB4BC);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2B6, param);
    fx.mem.set32(TIMER, 0x172);
    for (at, node, clear) in [(0xB244u32, 9, 1), (0xB246, 0xB, 0), (0xB248, 0xC, 0)] {
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
    for k in 0..3u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0xB1F0 + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x90);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x94)), 180.0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = fx.rand();
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0x64..0xB4u32 {
        fx.mem.write_u8(BYTES + k, 0);
    }
    for k in 0..0x100u32 {
        let v = fx.rand();
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES2 + k, h - 0xF);
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
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(slot + 0x2E4);
        if f != 0 && f != IDLE {
            run_task(fx, f, slot + 0x2A0, slot, k);
        }
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x44) == BUBBLE {
            let kind = fx.mem.i16(t + 0x52) as u32;
            let w = fx.mem.read_u32((D + 0xB1F0).wrapping_add(4 * kind));
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x50, sub);
            if fx.mem.i16(t + 0x52) == 1 {
                fx.mem.set16(D + 0x9A, 0x3F);
            }
            fx.mem.set16(D + 0xBE, 0);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
        }
        if fx.mem.read_u32(t + 0x44) == RING {
            let w = fx.mem.read_u32(D + 0xB1F0);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x60, sub);
            fx.mem.set16(D + 0xB8, 5);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES2);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if fx.mem.read_u32(t + 0x44) == FACING_RING {
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x70, sub);
            let w = fx.mem.read_u32(D + 0xB1F8);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 6);
            fx.mem.set16(D + 0xC0, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, w);
            }
            billboard::ring_facing(fx, uv::uv_rect, D, t, 0x10, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == COLOURED {
            let w = fx.mem.read_u32(D + 0xB1F8);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x80, sub);
            fx.mem.set16(D + 0xBE, 0);
            billboard::quad(fx, uv::uv_rect, D, t, 0x10, t + 0x10);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x82 => 1,
        0x16D => 4,
        0x163 => 2,
        0x8D..=0x15D => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell 0x14 / `sub` (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, sub: i32) {
    l2::rect(fx, r, 0x14, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

fn run_task(fx: &mut Fx, f: u32, t: u32, slot: u32, k: u32) {
    match f {
        BUBBLE => bubble(fx, t, slot, k),
        RING => ring(fx, t),
        FACING_RING => facing_ring(fx, t),
        COLOURED => coloured(fx, t),
        _ => panic!("b549: unknown task step {f:#x}"),
    }
}

/// The node point (4 words) into D+0x250.
fn from_point(fx: &mut Fx) {
    for k in 0..4 {
        let w = fx.mem.read_u32(POINT + 4 * k);
        fx.mem.write_u32(D + 0x250 + 4 * k, w);
    }
}

/// (rand % m - o) / 10 added to the float at `a`.
fn jitter(fx: &mut Fx, v: i32, a: u32, o: i32) {
    let j = ee::div(ee::from_i32(v - o), 10.0);
    let x = ee::add(fx.mem.f(a), j);
    fx.mem.setf(a, x);
}

/// 0x1E01700(D, 0): the update counter, node 9 into +0xB1D0 every update,
/// and the emitters (frame 0x60: ra, the entry's s3 = 0 / s2 = -1 / s1 =
/// D / s0 = 0, f20..f22 not written).
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, 0x01E0_2930), (0x40, 0), (0x30, u64::MAX), (0x20, u64::from(D)), (0x10, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let node9 = |fx: &mut Fx| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + 0xB244);
        fx.node_position(h, slot, POINT);
    };
    node9(fx);
    if (0x97..0x160).contains(&fx.mem.i16(COUNTER)) {
        node9(fx);
        from_point(fx);
    }
    if (0x78..0x120).contains(&fx.mem.i16(COUNTER)) {
        for k in 0..3u32 {
            from_point(fx);
            let a = rand_mod(fx, 0x32);
            let b = rand_mod(fx, 0x32);
            jitter(fx, a, D + 0x250, 0x19);
            jitter(fx, b, D + 0x254, 0x19);
            let (f, ra) = if k == 0 { (BUBBLE, 0x01E0_18B0) } else { (COLOURED, 0x01E0_18CC) };
            spawn(fx, f, &[(0x50, ra), (0x40, 0), (0x30, u64::from(D)), (0x20, u64::MAX), (0x10, u64::from(k)), (0, u64::from(k))]);
        }
    }
    if fx.mem.i16(COUNTER) < 0x120 {
        for k in 0..2u32 {
            let h = fx.self_handle();
            let slot = fx.mem.i16(D + 0xB246 + 2 * k);
            fx.node_position(h, slot, D + 0x250);
            let x = rand_mod(fx, 0x64);
            let z = rand_mod(fx, 0x64);
            let y = match k {
                0 => ee::from_i32(rand_mod(fx, 0x19)),
                _ => {
                    let v = fx.rand();
                    ee::from_i32(fx.div(v.wrapping_neg(), 0x19).1)
                }
            };
            jitter(fx, x, D + 0x250, 0x32);
            let yy = ee::add(fx.mem.f(D + 0x254), ee::div(y, 10.0));
            fx.mem.setf(D + 0x254, yy);
            jitter(fx, z, D + 0x258, 0x32);
            let zz = fx.mem.f(D + 0x258);
            let zz = if fx.mem.i16(FACING) != 0 { ee::add(zz, 2.0) } else { ee::sub(zz, 2.0) };
            fx.mem.setf(D + 0x258, zz);
            spawn(fx, FACING_RING, &[(0x50, 0x01E0_1A84), (0x40, 0), (0x30, u64::from(D)), (0x20, u64::from(k)), (0x10, u64::from(k)), (0, u64::from(D + 2 * k))]);
        }
    }
    fx.leave(0x60);
}

/// 0x1E02CE0(D, step, 0): b631's spawn (the first free task cleared and
/// set off at D+0x250 with the colour +0xB214 twice, angle flag +0xB2B0).
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
            for k in 0..3 {
                let w = fx.mem.read_u32(D + 0x250 + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let a = fx.mem.i16(D + 0xB2B0);
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

/// rand % m (HI, the remainder).
fn rand_mod(fx: &mut Fx, m: i32) -> i32 {
    let v = fx.rand();
    fx.div(v, m).1
}

/// rand() from a step's `jal` at `site` (its frame: s0 = the task `t`).
fn rand_at(fx: &mut Fx, t: u32, site: u32) -> i32 {
    fx.rand_from(t, site + 8)
}

/// rand % m (HI) from `site`.
fn rmod(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = rand_at(fx, t, site);
    fx.div(v, m).1
}

/// (rand >> 4) % m (HI) from `site`.
fn rmod4(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = rand_at(fx, t, site);
    fx.div(v >> 4, m).1
}

/// 0x205580(t + 0x20) from a step of frame 0x30 (its saves: ra, the
/// step's s1 = D, s0 = t).
fn angles(fx: &mut Fx, t: u32, ra: u32) {
    let base = fx.sp.wrapping_sub(0x30);
    fx.saved(base, &[(0x20, u64::from(ra)), (0x10, u64::from(D)), (0, u64::from(t))]);
    l2::camera_angles_to(fx, t + 0x20);
}

/// A bubble's drift (+0x30 / +0x34 = rand % 100 - 50, / 200), heading
/// +0x5C, camera angles and spins (+4, +2, +0 = rand % 0x800 - 0x400).
/// `sites`: the `jal`s of its six rands; `ra` 0x205580's return.
fn bubble_drift(fx: &mut Fx, t: u32, sites: [u32; 6], ra: u32) {
    let v = rmod(fx, t, sites[0], 0x64);
    fx.mem.setf(t + 0x34, ee::from_i32(v - 0x32));
    let v = rmod(fx, t, sites[1], 0x64);
    fx.mem.setf(t + 0x30, ee::from_i32(v - 0x32));
    for o in [0x34u32, 0x30] {
        let q = ee::div(fx.mem.f(t + o), 200.0);
        fx.mem.setf(t + o, q);
    }
    let v = rmod(fx, t, sites[2], 0x168);
    fx.mem.set16(t + 0x5C, v);
    angles(fx, t, ra);
    for (o, site) in [4u32, 2, 0].into_iter().zip([sites[3], sites[4], sites[5]]) {
        let v = rand_at(fx, t, site);
        fx.mem.set16(t + o, v % 0x800 - 0x400);
    }
}

/// A bubble's alpha: while the counter is below 350, up by 0x40 below
/// 0x80 for 76 updates; else down by 9 (the task ends below 11).
fn bubble_fade(fx: &mut Fx, t: u32) {
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(COUNTER) < 0x15E && fx.mem.i16(t + 0x54) < 0x4C {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// A bubble's move (not on its first update): 3 toward the facing, x by
/// +0x30, y by +0x34 - 0.09.
fn bubble_move(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x54) == 0 {
        return;
    }
    let z = fx.mem.f(t + 0x18);
    let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 3.0) } else { ee::add(z, 3.0) };
    fx.mem.setf(t + 0x18, z);
    let x = ee::add(fx.mem.f(t + 0x10), fx.mem.f(t + 0x30));
    fx.mem.setf(t + 0x10, x);
    let dy = ee::sub(fx.mem.f(t + 0x34), f32::from_bits(0x3DB8_51EC));
    let y = ee::add(fx.mem.f(t + 0x14), dy);
    fx.mem.setf(t + 0x14, y);
}

/// The angles +0x20.. += the spins (32-bit), the age counted.
fn spin_age(fx: &mut Fx, t: u32) {
    for (a, s) in [(0x28u32, 4u32), (0x24, 2), (0x20, 0)] {
        let v = fx.mem.read_u32(t + a).wrapping_add(fx.mem.i16(t + s) as u32);
        fx.mem.write_u32(t + a, v);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E01AE0: a violet bubble of size 20..39 (height + 4), its cell frame
/// cycling 0..7, growing by 1 every third update to 60; on its 61st
/// update (counter below 339) a white ring starts at it. `slot` / `k` the
/// entry's loop registers (s2 / s3).
fn bubble(fx: &mut Fx, t: u32, slot: u32, k: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_1B04, 0x14);
        fx.mem.set16(t + 8, v + 0x14);
        let h = fx.mem.i16(t + 8) + 4;
        fx.mem.set16(t + 0xA, h);
        fx.mem.set16(t + 0x52, 0);
        // +0x52 was just cleared: the pink branch (0x1E01B80) is never
        // taken [F].
        l2::rgb(fx, t + 0x60, 0x80, 0, 0xFF);
        let v = rand_at(fx, t, 0x01E0_1B58);
        fx.mem.set16(t + 0x56, (v >> 4) % 8);
        fx.mem.set16(t + 0x58, 2);
        bubble_drift(fx, t, [0x01E0_1BBC, 0x01E0_1BE8, 0x01E0_1C34, 0x01E0_1C58, 0x01E0_1C78, 0x01E0_1C98], 0x01E0_1C58);
    }
    bubble_fade(fx, t);
    if fx.mem.i16(t + 0x52) == 0 {
        let c = fx.mem.i16(t + 0x56) + 1;
        fx.mem.set16(t + 0x56, if c >= 8 { 0 } else { c });
    }
    let age = fx.mem.i16(t + 0x54);
    if fx.div(age, 3).1 == 1 {
        let w = fx.mem.i16(t + 8);
        if w < 0x3C {
            fx.mem.set16(t + 8, w + 1);
        }
        let h = fx.mem.i16(t + 0xA);
        if h < fx.mem.i16(t + 8) + 4 {
            fx.mem.set16(t + 0xA, h + 1);
        }
    }
    bubble_move(fx, t);
    if fx.mem.i16(COUNTER) < 0x153 && fx.mem.i16(t + 0x54) == 0x3D {
        for k in 0..4 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * k);
            fx.mem.write_u32(D + 0x250 + 4 * k, w);
        }
        spawn(fx, RING, &[(0x50, 0x01E0_1EB0), (0x40, 0), (0x30, u64::from(k)), (0x20, u64::from(slot)), (0x10, u64::from(D)), (0, u64::from(t))]);
    }
    angles(fx, t, 0x01E0_1EBC);
    spin_age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02280: a coloured bubble (red, violet or green) of size 43..72
/// growing by 2 to 300 (square), its cell frame random.
fn coloured(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_22A4, 0x1E);
        fx.mem.set16(t + 8, v + 0x2B);
        let h = fx.mem.i16(t + 8) + 4;
        fx.mem.set16(t + 0xA, h);
        let v = rmod4(fx, t, 0x01E0_22D0, 3);
        fx.mem.set16(t + 0x52, v);
        for (kind, (r, g, b)) in [(0, (0xFF, 0, 0)), (1, (0x80, 0, 0xFF)), (2, (0, 0xFF, 0))] {
            if fx.mem.i16(t + 0x52) == kind {
                l2::rgb(fx, t + 0x60, r, g, b);
            }
        }
        let v = rand_at(fx, t, 0x01E0_235C);
        fx.mem.set16(t + 0x56, (v >> 4) % 8);
        fx.mem.set16(t + 0x58, 2);
        bubble_drift(fx, t, [0x01E0_2384, 0x01E0_23B0, 0x01E0_23FC, 0x01E0_2420, 0x01E0_2440, 0x01E0_2460], 0x01E0_2420);
    }
    bubble_fade(fx, t);
    let w = fx.mem.i16(t + 8);
    if w < 0x12C {
        fx.mem.set16(t + 8, w + 2);
    }
    let w = fx.mem.i16(t + 8);
    fx.mem.set16(t + 0xA, w);
    bubble_move(fx, t);
    angles(fx, t, 0x01E0_25C0);
    spin_age(fx, t);
    fx.leave(0x30);
}

/// 0x1E01F10: a white ring: quad size 15, cell frame 3, radius +0x50
/// 10..24 growing by 25 a update, row +8 1..8, life +6 4..11; full alpha
/// 0xC0 for its life, then out by 13. (+0x52 = 1 would flicker it; never
/// set [F].)
fn ring(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4170_0000);
        fx.mem.set16(t + 0x56, 3);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        let v = rmod(fx, t, 0x01E0_1F50, 0xF);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, 0x01E0_1F70, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0xE, 0x19);
        let v = rand_at(fx, t, 0x01E0_1F94);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rand_at(fx, t, 0x01E0_1FB4);
        fx.mem.set16(t + 6, v % 8 + 4);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xF {
            fx.mem.write_u8(t + 0x63, a - 0xD);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    if fx.mem.i16(t + 0x52) == 1 {
        let c = if fx.mem.i16(t + 0x54) % 2 == 1 { 0xFF } else { 0x80 };
        l2::rgb(fx, t + 0x60, c, c, c);
    }
    fx.leave(0x20);
}

/// 0x1E020C0: a pink facing ring: quad size 30 growing by 2, a random cell
/// frame and orientation +0xA (rand % 8), radius +0x50 10..59 growing by
/// +0xE 2..9, row +8 0..34, life +6 2..5; fades in by 0x20 below 0x80,
/// then out by 8.
fn facing_ring(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x41F0_0000);
        let v = rand_at(fx, t, 0x01E0_20E0);
        fx.mem.set16(t + 0x56, v % 8);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        let v = rmod(fx, t, 0x01E0_2114, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rand_at(fx, t, 0x01E0_2130);
        fx.mem.set16(t + 0xA, v % 8);
        let r = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, r);
        let v = rmod(fx, t, 0x01E0_2154, 0x32);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rand_at(fx, t, 0x01E0_2174);
        fx.mem.set16(t + 0xE, v % 8 + 2);
        let v = rmod(fx, t, 0x01E0_2194, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rand_at(fx, t, 0x01E0_21B0);
        fx.mem.set16(t + 6, v % 4 + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x20);
        }
    } else if a >= 0xA {
        fx.mem.write_u8(t + 0x63, a - 8);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    let s = ee::add(fx.mem.f(t + 0x48), 2.0);
    fx.mem.setf(t + 0x48, s);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}
