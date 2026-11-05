//! BMODULE b092 (card 160), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E038F0, data area 0x1ED0000, 0xCF40 bytes) [F]
//!: b746's layout (card 222, b746.rs): 400 tasks
//! of 0x70 at D+0x2A0 (alive +0x58, step +0x44 through `jalr`) spawned by
//! 0x1E04080 (b746's spawn with +0x52 from D+0xCBC6), two kinds of flash
//! task (0x1E032F0 / 0x1E035C0: counted, not stepped, by the entry and
//! stepped by the unit flash 0x1E047A0 in a packet layer). Nodes: the
//! attacker's 0x2B / 0x2C / 0x14 / 0x15 / 1 / 0x51 / 0x4F at D+0xCB58..,
//! the target's 1 at D+0xCB68; textures (4, 0xF) D+0x29C, (5, 0..3)
//! D+0xCB00..; the facing D+0xCBCA; jitter bytes D+0xCBD0; cells D+0xCB78.
//!
//! The emitter 0x1E016F0: update 1 the stage light 0x1E02F80 (b746's, back
//! up under timer 30); on 15 eight light columns 0x1E02260 (b256's, at
//! nodes 0x2B / 0x2C by +0x52, turning by 2); on 0x34 / 0x4E 18 white
//! motes 0x1E02420 (b377's, thrown 2.6, drifting 1) at nodes 0x14 / 0x15;
//! odd 0x4E..0x70 and 0x20..0x46 with c % 5 = 1 a frame tint 0x1E03090
//! (b292's in (0x90, 0x70, 0x80)); before 5 a red flash 0x1E032F0 and
//! before 0x51 a grey zoom flash 0x1E035C0 every update; node 1's point
//! kept at D+0xCAF0; on 0x3C two bursts 0x1E02950 22 to each side of node
//! 1; 0x53..0x58 six standing flickers 0x1E02AB0 (b292's) around it. The
//! steps 0x1E025E0 / 0x1E02C40 and the draw 0x1E04230 are never spawned
//! (not ported). Returns 1 on update 0x49, 4 on 0x82, 2 on 0x7A, 6 on
//! 0x54..0x74, 8 once the 0x8C-update timer D+0xCB20 has run out.
use crate::b076::node_position_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
const POS: u32 = D + 0x250;
const COLUMN: u32 = 0x01E0_2260;
const MOTE: u32 = 0x01E0_2420;
const BURST: u32 = 0x01E0_2950;
const STAND: u32 = 0x01E0_2AB0;
const LIGHT: u32 = 0x01E0_2F80;
const TINT: u32 = 0x01E0_3090;
/// The flash tasks, run by the unit flash 0x1E047A0.
const FLASH: u32 = 0x01E0_32F0;
const ZOOM: u32 = 0x01E0_35C0;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_3C38;
/// Node 1's point, every update.
const KEPT: u32 = D + 0xCAF0;
const TEX: u32 = D + 0xCB00;
const TIMER: u32 = D + 0xCB20;
const COLOUR: u32 = D + 0xCB24;
const LAYER: u32 = D + 0xCB34;
const FLASH_DESC: u32 = D + 0xCB38;
const NODES: u32 = D + 0xCB58;
const SPAWN_A: u32 = D + 0xCBB6;
const SPAWN_K: u32 = D + 0xCBC4;
const SPAWN_K2: u32 = D + 0xCBC6;
const FACING: u32 = D + 0xCBCA;
const COUNTER: u32 = D + 0xCBCE;
const BYTES: u32 = D + 0xCBD0;
const TAB: u32 = D + 0xCDD0;

fn node(fx: &mut Fx, k: u32) -> i32 {
    fx.mem.i16(NODES + 2 * k)
}

fn tex(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

/// The atlas cell `id` / `sub` (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let w = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, w);
    }
}

/// The entry 0x1E038F0 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    fx.mem.write_u32(D + 0xCB2C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xA0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xCF40);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xCBCC, param);
    fx.mem.set32(TIMER, 0x8C);
    for (k, n) in [0x2B, 0x2C, 0x14, 0x15, 1, 0x51, 0x4F].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xCB68, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..4 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x90);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x94)), 180.0);
    fx.fix_residue(y, 0x01E0_3B08);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = fx.rand_from(k, 0x01E0_3B38);
        let v = fx.div(v, 0x1E).1;
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_3B8C);
        fx.mem.set16(D + 0xCB78 + 2 * k, v % 8);
    }
    0
}

/// A step's prologue (`frame` bytes): ra, the entry's s1 = 0 and s0 = the
/// flash tasks counted so far.
fn enter_step(fx: &mut Fx, frame: u32, flashes: u32) -> u32 {
    let s = fx.enter(frame);
    if frame == 0x20 || frame == 0x80 {
        fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(flashes))]);
    } else {
        fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(flashes))]);
    }
    s
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let mut a3 = emit(fx, param);
    // The entry's s0: the flash tasks counted here (the steps' saved s0);
    // its s1 is 0 until the draw loop. a3 as the last call leaves it, for
    // the unit flash's first 0x207BB0 [H where not run].
    let mut flashes = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        match f {
            0 => continue,
            FLASH | ZOOM => {
                flashes += 1;
                continue;
            }
            _ => {}
        }
        let frame = match f {
            MOTE | STAND => 0x30,
            TINT => 0x80,
            _ => 0x20,
        };
        let s = enter_step(fx, frame, flashes);
        match f {
            COLUMN => column(fx, t, &mut a3),
            MOTE => mote(fx, t),
            BURST => a3 = burst(fx, t),
            STAND => a3 = stand(fx, t),
            LIGHT => light(fx, t),
            TINT => {
                tint(fx, t, s);
                a3 = 0;
            }
            _ => panic!("b092: unknown task step {f:#x}"),
        }
        fx.leave(frame);
    }
    let mut drawn = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        drawn += 1;
        draw(fx, sp, t, [flashes, drawn], &mut a3);
    }
    if flashes != 0 {
        unit_flash(fx, a3);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x49 => 1,
        0x82 => 4,
        0x7A => 2,
        0x54..=0x74 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The last XYZ2 of the last primitive drawn since `before` (0 when none).
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

/// The entry's draw cases for task `t`; `regs` the draw loop's s0 (flash
/// count) and s1 (drawn count).
fn draw(fx: &mut Fx, sp: u32, t: u32, regs: [u32; 2], a3: &mut u32) {
    let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
    if f(fx) == COLUMN {
        tex(fx, D + 0x29C);
        cell(fx, sp + 0x60, 0x9A, 0);
        fx.mem.set16(D + 0xBE, 0);
        for (side, ra) in [(0, 0x01E0_3D18u32), (1, 0x01E0_3D60)] {
            if fx.mem.i16(t + 0x52) == side {
                let h = fx.self_handle();
                let slot = node(fx, side as u32);
                node_position_saving(fx, h, slot, t + 0x10, u64::from(regs[0]), Some(u64::from(regs[1])), ra);
                let before = fx.out.prims.len();
                quads4::sprite_quads(fx, D, t, 0, t + 0x10);
                *a3 = last_xyz2(fx, before);
            }
        }
    }
    if f(fx) == MOTE {
        tex(fx, TEX);
        let c = fx.mem.i16(t + 0x56);
        cell(fx, sp + 0x70, 0x14, c);
        let o = fx.mem.i16(t + 0x52);
        fx.mem.set16(D + 0xBE, o);
        billboard::quad(fx, uv::uv_rect, D, t, 0x10, t + 0x10);
        *a3 = 0;
    }
    if f(fx) == BURST {
        if fx.mem.i16(t + 0x50) == 0 {
            tex(fx, TEX + 8);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            tex(fx, TEX + 4);
        }
        fx.mem.set16(D + 0xBE, 0);
        for o in [0x94u32, 0x96, 0x98, 0x9A] {
            fx.mem.set16(D + o, 0x40);
        }
        billboard::quad(fx, uv::uv_rect, D, t, 0x21, t + 0x10);
        *a3 = 0;
    }
    if f(fx) == STAND {
        tex(fx, TEX + 0xC);
        fx.mem.set16(D + 0xBE, 0);
        let c = fx.mem.i16(t + 0x50);
        fx.mem.set16(D + 0x94, c << 6);
        fx.mem.set16(D + 0x96, 0);
        fx.mem.set16(D + 0x98, 0x40);
        fx.mem.set16(D + 0x9A, 0x40);
        billboard::quad_standing(fx, uv::uv_rect, D, t, 0x21, t + 0x10);
        *a3 = 0;
    }
}

/// 0x1E04080(D, step, 0): b746's spawn (colour D+0xCB24, +0x50 from
/// D+0xCBC4, +0x52 from D+0xCBC6, +0x5A from D+0xCBB6); its frame's saves:
/// the emitter's s0 = D / s1 / s2, the entry's s3 = param, s4 (0) and the
/// return address `ra`.
fn spawn(fx: &mut Fx, step: u32, s1: u32, s2: u32, param: i32, ra: u32) -> i32 {
    let f = fx.enter(0x60);
    fx.saved(f, &[(0, u64::from(D)), (0x10, u64::from(s1)), (0x20, u64::from(s2)), (0x30, param as i64 as u64), (0x40, 0), (0x50, u64::from(ra))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x2F8 + 0x70 * k) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for o in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + o + 3, a);
            }
            l2::to_i16x4(fx, t, POS, 0);
            l2::copy3(fx, t + 0x10, POS);
            let s = fx.mem.i16(SPAWN_K);
            fx.mem.set16(t + 0x50, s);
            let s = fx.mem.i16(SPAWN_K2);
            fx.mem.set16(t + 0x52, s);
            let a = fx.mem.i16(SPAWN_A);
            fx.mem.set16(t + 0x5A, a);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, step);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// 0x1E016F0(D, 0): the update counter and the emitters (frame 0x40: ra,
/// the entry's s2 = D, s1 = 0, s0 = 0; then s0 = D, s1 / s2 as below).
fn emit(fx: &mut Fx, param: i32) -> u32 {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_3BE8), (0x20, u64::from(D)), (0x10, 0), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    // The emitter's s1 / s2 (the entry's 0 / D until set).
    let mut s1 = 0u32;
    let mut s2 = D;
    if c(fx) == 1 {
        spawn(fx, LIGHT, s1, s2, param, 0x01E0_1748);
    }
    if c(fx) == 0xF {
        fx.mem.set16(SPAWN_K2, 0);
        let cols = [
            (None, 0x2D, 0, 0x01E0_179Cu32),
            (None, 0x87, 0, 0x01E0_17CC),
            (None, 0, 1, 0x01E0_17FC),
            (None, 0x5A, 1, 0x01E0_1830),
            (Some(1), 0x5A, 0, 0x01E0_1870),
            (None, 0xB4, 0, 0x01E0_18A0),
            (None, 0x2D, 1, 0x01E0_18D4),
            (None, 0x87, 1, 0x01E0_1908),
        ];
        for (k2, a, k, ra) in cols {
            if let Some(v) = k2 {
                fx.mem.set16(SPAWN_K2, v);
            }
            fx.mem.set16(SPAWN_A, a);
            fx.mem.set16(SPAWN_K, k);
            spawn(fx, COLUMN, s1, s2, param, ra);
        }
    }
    if c(fx) == 0x34 || c(fx) == 0x4E {
        // Loop 1: s2 the angle, s1 the count; loop 2 the other way round.
        for (nk, ra_n, ra_s, swap) in [(2u32, 0x01E0_1954u32, 0x01E0_1988u32, false), (3, 0x01E0_19E0, 0x01E0_1A14, true)] {
            let (mut j, mut a) = (0u32, 0u32);
            while j < 0x12 {
                let (r1, r2) = if swap { (a, j) } else { (j, a) };
                let h = fx.self_handle();
                let slot = node(fx, nk);
                node_position_saving(fx, h, slot, POS, u64::from(D), Some(u64::from(r1)), ra_n);
                fx.mem.set16(SPAWN_A, a as i32);
                let y = ee::sub(fx.mem.f(POS + 4), 1.0);
                fx.mem.setf(POS + 4, y);
                spawn(fx, MOTE, r1, r2, param, ra_s);
                a += 0x14;
                j += 1;
            }
            (s1, s2) = if swap { (a, j) } else { (j, a) };
        }
    } else {
        s1 = 0;
    }
    let v = c(fx);
    if (0x4E..0x71).contains(&v) && v % 2 == 1 {
        spawn(fx, TINT, s1, s2, param, 0x01E0_1A9C);
    }
    let v = c(fx);
    if (0x20..0x47).contains(&v) && fx.div(v, 5).1 == 1 {
        spawn(fx, TINT, s1, s2, param, 0x01E0_1AEC);
    }
    if c(fx) < 5 {
        spawn(fx, FLASH, s1, s2, param, 0x01E0_1B18);
    }
    if c(fx) < 0x51 {
        spawn(fx, ZOOM, s1, s2, param, 0x01E0_1B40);
    }
    let h = fx.self_handle();
    let slot = node(fx, 4);
    node_position_saving(fx, h, slot, KEPT, u64::from(D), Some(u64::from(s1)), 0x01E0_1B64);
    // a3 as the emitter leaves it: 0x1F6830's object rule, or after a
    // spawn its 0x107390 call's (the colour's third byte) [H].
    let mut a3 = 0u32;
    object_a3(fx, h, &mut a3);
    if c(fx) == 0x3C {
        for (k, ra_n, ra_s) in [(0, 0x01E0_1B9Cu32, 0x01E0_1C04u32), (1, 0x01E0_1C24, 0x01E0_1C90)] {
            let h = fx.self_handle();
            let slot = node(fx, 4);
            node_position_saving(fx, h, slot, POS, u64::from(D), Some(u64::from(s1)), ra_n);
            // k 0: x - 22 facing, else + 22 (the sign `by` flipped).
            let x = fx.mem.f(POS);
            let facing = fx.mem.i16(FACING) != 0;
            let x = match (k, facing) {
                (0, true) | (1, false) => ee::sub(x, 22.0),
                _ => ee::add(x, 22.0),
            };
            fx.mem.setf(POS, x);
            fx.mem.set16(SPAWN_K, k);
            spawn(fx, BURST, s1, s2, param, ra_s);
            a3 = u32::from(fx.mem.read_u8(COLOUR + 2));
        }
    }
    // 0x53..0x58: the standing flickers around node 1 (z, x, y offsets).
    let stands: [(i32, f32, Option<f32>, f32, u32, u32); 6] = [
        (0x53, 4.0, Some(20.0), 0.0, 0x01E0_1CC8, 0x01E0_1D78),
        (0x54, 12.0, Some(10.0), 1.0, 0x01E0_1DB0, 0x01E0_1E78),
        (0x55, 24.0, None, 2.0, 0x01E0_1EB0, 0x01E0_1F40),
        (0x56, 24.0, Some(-10.0), 3.0, 0x01E0_1F78, 0x01E0_2040),
        (0x57, 12.0, Some(-20.0), 4.0, 0x01E0_2078, 0x01E0_2140),
        (0x58, 4.0, Some(-30.0), 5.0, 0x01E0_2178, 0x01E0_2240),
    ];
    for (k, (at, dz, dx, dy, ra_n, ra_s)) in stands.into_iter().enumerate() {
        if c(fx) != at {
            continue;
        }
        let h = fx.self_handle();
        let slot = node(fx, 4);
        node_position_saving(fx, h, slot, POS, u64::from(D), Some(u64::from(s1)), ra_n);
        by_facing_neg(fx, POS + 8, dz);
        if let Some(dx) = dx {
            by_facing_neg(fx, POS, dx);
        }
        if k != 0 {
            let y = ee::add(fx.mem.f(POS + 4), dy);
            fx.mem.setf(POS + 4, y);
        }
        fx.mem.set16(SPAWN_K, k as i32);
        spawn(fx, STAND, s1, s2, param, ra_s);
        a3 = u32::from(fx.mem.read_u8(COLOUR + 2));
    }
    fx.leave(0x40);
    a3
}

/// What 0x1F6830 leaves in a3: a valid handle's object index runs
/// 0x1F40D0, which leaves a3 = (index < 0x54) (b016's rule).
fn object_a3(fx: &mut Fx, h: i32, a3: &mut u32) {
    let (side, slot) = (h >> 8, h & 0xFF);
    if (0..3).contains(&side) && slot < 0xB {
        let i = fx.object_index(side, slot);
        if i >= 0 {
            *a3 = u32::from(i < 0x54);
        }
    }
}

/// `at` -= `by` when facing, else += `by`.
fn by_facing_neg(fx: &mut Fx, at: u32, by: f32) {
    let v = fx.mem.f(at);
    let v = if fx.mem.i16(FACING) != 0 { ee::sub(v, by) } else { ee::add(v, by) };
    fx.mem.setf(at, v);
}

/// Byte `at` up by `up` while under `cap`.
fn fade_in(fx: &mut Fx, t: u32, at: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + at);
    if a < cap {
        fx.mem.write_u8(t + at, a.wrapping_add(up));
    }
}

/// Byte `at` down by `down` while at least `floor`, else the task ends.
fn fade_out(fx: &mut Fx, t: u32, at: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + at);
    if a >= floor {
        fx.mem.write_u8(t + at, a.wrapping_sub(down));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn add(fx: &mut Fx, at: u32, by: f32) {
    let v = ee::add(fx.mem.f(at), by);
    fx.mem.setf(at, v);
}

fn rand4_mod(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v >> 4, m).1
}

fn rmod(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v, m).1
}

/// 0x1E02260 (D, task): b256's light column: size 6, (0xF0, 0xE0, 0xFF)
/// over (0xFF, 0xFF, 0x80), by kind +0x50 (1.5, 40) / (1.5, 20) / (18,
/// 400) at +0x30 / +0x34; in by 0x60 under 0xC0 for 4 updates, out by 0xF;
/// from age 1 +0x34 grows by 5 (kind 0) / 2.5 (kind 1); +0x5A up by 2.
fn column(fx: &mut Fx, t: u32, a3: &mut u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0xF0, 0xE0, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0x80);
        *a3 = 0x80;
        for (kind, w, h) in [(0, 0x3FC0_0000u32, 0x4220_0000u32), (1, 0x3FC0_0000, 0x41A0_0000), (2, 0x4190_0000, 0x43C8_0000)] {
            if fx.mem.i16(t + 0x50) == kind {
                fx.mem.write_u32(t + 0x30, w);
                fx.mem.write_u32(t + 0x34, h);
            }
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 4 {
        fade_in(fx, t, 0x63, 0xC0, 0x60);
    } else {
        fade_out(fx, t, 0x63, 0x11, 0xF);
    }
    for (kind, by) in [(0, 5.0f32), (1, 2.5)] {
        if fx.mem.i16(t + 0x50) == kind && fx.mem.i16(t + 0x54) != 0 {
            add(fx, t + 0x34, by);
        }
    }
    age(fx, t);
    for side in 0..2 {
        if fx.mem.i16(t + 0x52) == side {
            let a = fx.mem.i16(t + 0x5A) + 2;
            fx.mem.set16(t + 0x5A, a);
        }
    }
}

/// x += `by` cos +0x5A, z += `by` sin +0x5A (z first).
fn throw(fx: &mut Fx, t: u32, by: Option<f32>) {
    let a = fx.mem.i16(t + 0x5A);
    let s = l2::sin_deg(fx, TAB, a, 0);
    add(fx, t + 0x18, by.map_or(s, |b| ee::mul(b, s)));
    let a = fx.mem.i16(t + 0x5A);
    let c = l2::sin_deg(fx, TAB, a, 1);
    add(fx, t + 0x10, by.map_or(c, |b| ee::mul(b, c)));
}

/// 0x1E02420 (D, task): b377's white mote: size 50, cell 2..4, thrown 2.6
/// out at its spawn angle, +0x52 a random 0..3, then drifting 1 a update;
/// in by 0x60 under 0xC0 for 6 updates, out by 0x13.
fn mote(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x32);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let r = rand4_mod(fx, t, 0x01E0_2458, 3);
        fx.mem.set16(t + 0x56, r + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        throw(fx, t, Some(f32::from_bits(0x4026_6666)));
        let v = fx.rand_from(t, 0x01E0_24F0);
        fx.mem.set16(t + 0x52, (v >> 4).wrapping_rem(4));
    }
    if fx.mem.i16(t + 0x54) < 6 {
        fade_in(fx, t, 0x63, 0xC0, 0x60);
    } else {
        fade_out(fx, t, 0x63, 0x15, 0x13);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        throw(fx, t, None);
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_25B0);
    age(fx, t);
}

/// 0x1E02950 (D, task): a burst (200 x 200 growing by 2, cell (rand >> 4)
/// % 3, white, +0x5C rand % 360) facing the camera; in by 0x60 under 0xC0
/// for 10 updates, out by 9; white on odd ages, grey 0xC0 on even.
fn burst(fx: &mut Fx, t: u32) -> u32 {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0xC8);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let r = rand4_mod(fx, t, 0x01E0_2980, 3);
        fx.mem.set16(t + 0x56, r);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = rmod(fx, t, 0x01E0_29B4, 0x168);
        fx.mem.set16(t + 0x5C, v);
    }
    if fx.mem.i16(t + 0x54) < 0xA {
        fade_in(fx, t, 0x63, 0xC0, 0x60);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2A24);
    age(fx, t);
    let w = fx.mem.i16(t + 8) + 2;
    fx.mem.set16(t + 8, w);
    fx.mem.set16(t + 0xA, w);
    let c = if fx.mem.i16(t + 0x54) % 2 == 1 { 0xFF } else { 0xC0 };
    l2::rgb(fx, t + 0x60, c, c, c);
    c as u32
}

/// 0x1E02AB0 (D, task): b292's standing flicker, 70 x 80 (cell 0..2),
/// turned 0x4000 (0xC000 when facing), +0x5C rand % 360; in by 0x60 under
/// 0xC0 for 8 updates, out by 9; from age 3 +0xA up by 40 a update; white
/// on odd ages, grey 0xC0 on even.
fn stand(fx: &mut Fx, t: u32) -> u32 {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x46);
        fx.mem.set16(t + 0xA, 0x50);
        let r = rand4_mod(fx, t, 0x01E0_2AE8, 3);
        fx.mem.set16(t + 0x56, r);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let turn = if fx.mem.i16(FACING) != 0 { 0xC000 } else { 0x4000 };
        fx.mem.write_u32(t + 0x20, turn);
        let v = rmod(fx, t, 0x01E0_2B48, 0x168);
        fx.mem.set16(t + 0x5C, v);
    }
    if fx.mem.i16(t + 0x54) < 8 {
        fade_in(fx, t, 0x63, 0xC0, 0x60);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    if fx.mem.i16(t + 0x54) >= 3 {
        let h = fx.mem.i16(t + 0xA) + 0x28;
        fx.mem.set16(t + 0xA, h);
    }
    age(fx, t);
    let c = if fx.mem.i16(t + 0x54) % 2 == 1 { 0xFF } else { 0xC0 };
    l2::rgb(fx, t + 0x60, c, c, c);
    c as u32
}

/// 0x1E02F80 (D, task): b746's stage light: down by 10 to 0x30 for 51
/// updates, back up once the timer is under 30.
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x33 {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x31 { 0x30 } else { l - 0xA });
    } else if fx.mem.i32(TIMER) < 0x1E {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x80 { l + 0xA } else { 0x80 });
        if fx.mem.i16(t + 0x50) >= 0x81 {
            fx.mem.set16(t + 0x50, 0x80);
        }
    }
    age(fx, t);
    if fx.mem.i32(TIMER) == 3 {
        fx.set_stage_light(0x80);
        fx.mem.set16(t + 0x58, 0);
    } else {
        let l = fx.mem.read_u8(t + 0x50);
        fx.set_stage_light(i32::from(l));
    }
}

/// 0x1E03090 (D, task; frame 0x80 at `sp`): b292's window sprite in (0x90,
/// 0x70, 0x80): before age 15 in by 4 below 0x18 and the inset up by 0.3
/// below 100, then out by 4 (ending under 6) and the inset down by 0.2 to
/// 0.
fn tint(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x3F80_0000);
        fx.mem.set16(t + 0x58, 2);
        l2::rgb(fx, t + 0x60, 0x90, 0x70, 0x80);
    }
    let d = sp + 0x28;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for k in 0..4 {
        let c = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(d + 0x10 + k, c);
    }
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    for (at, v) in [(d + 0x20, 0x6FF8u16), (d + 0x30, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    for o in [0x18u32, 0x1C] {
        let s = fx.mem.f(t + 0x48);
        fx.mem.setf(d + o, ee::div(s, 512.0));
    }
    for o in [0x28u32, 0x2C] {
        let s = fx.mem.f(t + 0x48);
        fx.mem.setf(d + o, ee::div(ee::sub(512.0, s), 512.0));
    }
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if fx.mem.i16(t + 0x54) < 0xF {
        let a = fx.mem.read_u8(t + 0x63);
        if a < 0x18 {
            fx.mem.write_u8(t + 0x63, a + 4);
        }
        let w = fx.mem.f(t + 0x48);
        if w < 100.0 {
            fx.mem.setf(t + 0x48, ee::add(w, f32::from_bits(0x3E99_999A)));
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 6 {
            fx.mem.write_u8(t + 0x63, a - 4);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let w = fx.mem.f(t + 0x48);
        if w <= 0.0 {
            fx.mem.write_u32(t + 0x48, 0);
        } else {
            fx.mem.setf(t + 0x48, ee::sub(w, f32::from_bits(0x3E4C_CCCD)));
        }
    }
    age(fx, t);
}

/// The window corner XYZ2 at `at` (x = y = `xy`, Z 0xFFFFFF).
fn corner(fx: &mut Fx, at: u32, xy: u16) {
    fx.mem.write_u16(at, xy);
    fx.mem.write_u16(at + 2, xy);
    let v = fx.mem.read_u64(at);
    fx.mem.write_u64(at, (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
    fx.mem.write_u8(at + 7, 0);
}

/// The flash descriptor: colour +0x60.., Q 1, the window inset by +8 / 10
/// and +0xA / 10 over 512, submitted with no ordering table.
fn flash_quad(fx: &mut Fx, t: u32) {
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let h = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    let d = fx.mem.read_u32(FLASH_DESC);
    for k in 0..4u32 {
        let b = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(d + 0x10 + k, b);
    }
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    corner(fx, d + 0x20, 0x6FF8);
    corner(fx, d + 0x30, 0x8FF8);
    fx.mem.setf(d + 0x18, ee::div(w, 512.0));
    fx.mem.setf(d + 0x1C, ee::div(h, 512.0));
    fx.mem.setf(d + 0x28, ee::div(ee::sub(512.0, w), 512.0));
    fx.mem.setf(d + 0x2C, ee::div(ee::sub(512.0, h), 512.0));
    fx.submit(d, 0, 0);
}

/// 0x1E032F0(D, t): b746's flash step in red (0x80, 0x20, 0x20): alpha up
/// by 9 below 0x80 until update 0x7D, then down by 7 (the task ends under
/// 9); it ends from update 0x87 anyway. `regs` the unit flash's s0 / s1
/// (the slot index, the live count).
fn flash(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_48AC), (0x20, u64::from(regs[1])), (0x10, u64::from(regs[0]))]);
    // swc1 f21 / f20: the runner's (0) [H].
    fx.mem.write_u32(sp + 4, 0);
    fx.mem.write_u32(sp, 0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0x80, 0x20, 0x20);
        fx.mem.set16(t + 0x58, 2);
    }
    flash_quad(fx, t);
    if fx.mem.i16(COUNTER) < 0x7D {
        fade_in(fx, t, 0x63, 0x80, 9);
    } else {
        fade_out(fx, t, 0x63, 9, 7);
    }
    age(fx, t);
    if fx.mem.i16(COUNTER) >= 0x87 {
        fx.mem.set16(t + 0x58, 0);
    }
    fx.leave(0x40);
}

/// 0x1E035C0(D, t): the zoom flash: the window in grey 0x40 (alpha 0x78)
/// inset by +8 / 10 and +0xA / 10 over 512; the insets grow by 60..99 a
/// update; after its first update the alpha down by 0x31 (the task ends
/// under 0x29).
fn zoom(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_48D0), (0x20, u64::from(regs[1])), (0x10, u64::from(regs[0]))]);
    fx.mem.write_u32(sp + 4, 0);
    fx.mem.write_u32(sp, 0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0x78);
        l2::rgb(fx, t + 0x60, 0x40, 0x40, 0x40);
        fx.mem.set16(t + 0x58, 2);
    }
    flash_quad(fx, t);
    for (o, ra) in [(8u32, 0x01E0_3834u32), (0xA, 0x01E0_3864)] {
        let v = rmod(fx, t, ra, 0x28);
        let s = fx.mem.i16(t + o) + ((v + 0x3C) as i16 as i32);
        fx.mem.set16(t + o, s);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        fade_out(fx, t, 0x63, 0x29, 0x31);
    }
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E047A0(D): b746's unit flash: in a packet layer (0x2078A0) the
/// attacker drawn into the target (0x1F8270 filter 0; ALPHA 0x48), TEXFLUSH
/// before and after; every flash / zoom task stepped; the layer linked into
/// the ordering table at key 0 when any task was alive.
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
    let mut alive = 0u32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        alive += 1;
        if fx.mem.read_u32(slot + 0x2E4) == FLASH {
            flash(fx, slot + 0x2A0, [k, alive]);
        }
        if fx.mem.read_u32(slot + 0x2E4) == ZOOM {
            zoom(fx, slot + 0x2A0, [k, alive]);
        }
    }
    if alive != 0 {
        let layer = fx.mem.read_u32(LAYER);
        fx.layer_end(layer, OT, 0);
    }
    fx.leave(0xB0);
}
