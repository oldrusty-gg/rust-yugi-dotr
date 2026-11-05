//! BMODULE b192 (card 558), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02490, data area 0x1ED0000, 0x38D0 bytes) [F]
//!: its own pool of 0x40-byte tasks at D+0x764
//! (alive +0x36 = D+0x79A + 0x40 k, step +0x24), searched by the spawn
//! 0x1E02F00 over the first D+0xB8 slots (180 from the start; the bud
//! ring's draw sets D+0xB8 to 14 and then 0, so nothing spawns after the
//! first one is drawn [F]), colour D+0x8C (never written: 0), point D+0,
//! kind D+0xB2. Globals: timer D+0x3610 = 0x98, OT D+0x361C, nodes
//! D+0x3648.. (1 / 0x44 / 0xE / 0xD), angle D+0x3650 (up by 2), counter
//! D+0x365A, table D+0x365C (rand % 200 - 100), sines D+0x375C, node 0xE /
//! 0xD histories D+0x3560 / D+0x34C0 (kept from update 0x5A). The emitter
//! 0x1E00FB0 follows node 1: from update 0x20 a green drift (0x1F00,
//! 0x1E00780 = b709's sprite) and a green fan (0x20D0, 0x1E00CE0), from
//! 0x24 a grey four-sprite cross (0x2250, 0x1E03920); on 0x55 a big green
//! fan (0x15D0), fifteen green rings (0x1DA0, `billboard::ring_t`) about
//! node 0xE, the yellow beam (0x19C0, stepped inline; 0x1E03310 between
//! the two histories) and thirteen pairs of green blades (0x1A60, 0x1E03600)
//! along the facing. From update 0x15 D+0x300 / D+0x308 grow by 0.01
//! while (as doubles) below 0.5. Returns 1 on update 0x4B, 4 on 0x5F, 2 on
//! 0x55, 8 once the timer has run out.
//!
//! The steps 0x1E013B0, 0x1710, 0x1BD0, 0x2380 (the stage light) and the
//! object op 0x2440 are tested by the entry but never spawned [F: the
//! emitter names 0x1F00 / 0x20D0 / 0x2250 / 0x15D0 / 0x1DA0 / 0x19C0 /
//! 0x1A60 only]; not ported. No 0x1F8100.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b089::dcmp;
use crate::b148::{facing_of, rmod};
use crate::b709::drift_draw;
use crate::blib::{billboard, spawn, uv};
use crate::bus::Bus;
use crate::c061::{desc_uv4, rgbaq_at};
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 180;
const POOL: u32 = D + 0x764;
const TIMER: u32 = D + 0x3610;
const OT2: u32 = D + 0x361C;
const NODES: u32 = D + 0x3648;
const ANGLE: u32 = D + 0x3650;
const COUNTER: u32 = D + 0x365A;
const TABLE: u32 = D + 0x365C;
const SINES: u32 = D + 0x375C;
const SPR: u32 = 0x7000_0000;

const FAN_BIG: u32 = 0x01E0_15D0;
const BEAM: u32 = 0x01E0_19C0;
const BLADE: u32 = 0x01E0_1A60;
const RING: u32 = 0x01E0_1DA0;
const DRIFT: u32 = 0x01E0_1F00;
const FAN: u32 = 0x01E0_20D0;
const CROSS: u32 = 0x01E0_2250;

/// The entry 0x1E02490 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x190);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(OT2, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp) };
    fx.leave(0x190);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x38D0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x3658, param);
    fx.mem.set32(TIMER, 0x98);
    for o in [0x300u32, 0x304, 0x308] {
        fx.mem.write_u32(D + o, 0);
    }
    for (k, n) in [1, 0x44, 0xE, 0xD].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    fx.mem.set16(D + 0xB8, 0xB4);
    for k in 0..3 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x330 + 4 * k as u32, t);
    }
    for k in 0xE..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x368 + 4 * (k as u32 - 0xE), t);
    }
    // Ten quads (5 x 50) at D+0x370 + 0x40 k, the first nine bent: y by
    // 15 / 35 sin of 20-degree steps, less 10 / plus 15, x by -45 + 10 k.
    for k in 0..0xA {
        l2::quad_xy(fx, D + 0x370 + 0x40 * k, 5.0, 50.0);
    }
    for k in 0..9i32 {
        let q = D + 0x370 + 0x40 * k as u32;
        let (a, b) = (0x14 * k, 0x14 * k + 0x14);
        for (o, deg, m) in [(4u32, a, 15.0f32), (0x14, b, 15.0), (0x24, a, 35.0), (0x34, b, 35.0)] {
            let s = l2::sin_deg(fx, SINES, deg as i16 as i32, 0);
            fx.mem.setf(q + o, ee::mul(m, s));
        }
        let x = ee::from_i32(-0x2D + 0xA * k);
        for o in [0u32, 0x10, 0x20, 0x30] {
            let v = ee::add(fx.mem.f(q + o), x);
            fx.mem.setf(q + o, v);
        }
        for (o, by) in [(4u32, -10.0f32), (0x14, -10.0), (0x24, 15.0), (0x34, 15.0)] {
            let v = fx.mem.f(q + o);
            let v = if by < 0.0 { ee::sub(v, 10.0) } else { ee::add(v, 15.0) };
            fx.mem.setf(q + o, v);
        }
        for o in [8u32, 0x18, 0x28, 0x38] {
            fx.mem.write_u32(q + o, 0);
        }
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2748, 0xC8);
        fx.mem.set8(TABLE + k, v - 0x64);
    }
    0
}

/// A step frame's saves: the entry's s0..s(n-1) (D, 0) and `ra`.
fn frame(fx: &mut Fx, low: u32, size: u32, n: u32, ra: u32) {
    let vals = [u64::from(D), 0];
    let base = low - size;
    for i in 0..n {
        fx.saved(base + 0x10 * i, &[(0, vals[i as usize])]);
    }
    fx.saved(base, &[(0x10 * n, u64::from(ra))]);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    let dd = u64::from(D);
    let c = fx.mem.i16(COUNTER);
    for (o, local, hist) in [(4u32, 0xC0u32, 0x3560u32), (6, 0xD0, 0x34C0)] {
        if c == 0x5A {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + o);
            fx.node_position(h, slot, sp + local);
            for i in 0..9 {
                for k in 0..3 {
                    let w = fx.mem.read_u32(sp + local + 4 * k);
                    fx.mem.write_u32(D + hist + 0x10 * i + 4 * k, w);
                }
            }
        }
    }
    let s2 = if c == 0x5A { u64::from(D + 0x3550) } else { u64::MAX };
    for (p, o, ra) in [(0x35F0u32, 4u32, 0x01E0_2840u32), (0x3550, 6, 0x01E0_2850)] {
        let n = fx.mem.i16(NODES + o);
        spawn::node_history(fx, D + p, 0xA, n);
        fx.saved(fx.sp - 0x70, &[(0, dd), (0x10, 0), (0x20, s2), (0x30, 0), (0x40, 0), (0x50, u64::from(ra))]);
    }
    let low = fx.sp;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            FAN_BIG => {
                fan_big(fx, t);
                frame(fx, low, 0x30, 2, 0x01E0_28A8);
            }
            BEAM => {
                l2::rgb(fx, t + 0x28, 0x50, 0x50, 0);
                if fx.mem.i16(t + 0x36) == 1 {
                    fx.mem.set16(t + 0x38, 5);
                    fx.mem.set16(t + 0x36, 2);
                }
                let h = fx.self_handle();
                let slot = fx.mem.i16(NODES);
                fx.node_position(h, slot, sp + 0xE0);
                fx.mem.write_u8(t + 0x2B, 0x50);
                fx.mem.set16(t + 0x32, 0xA);
                if fx.mem.i32(TIMER) == 0x32 {
                    fx.mem.set16(t + 0x36, 0);
                }
            }
            BLADE => {
                blade(fx, t);
                frame(fx, low, 0x20, 1, 0x01E0_2970);
            }
            RING => {
                ring(fx, t);
                frame(fx, low, 0x20, 1, 0x01E0_29B0);
            }
            DRIFT => {
                drift(fx, t);
                frame(fx, low, 0x30, 2, 0x01E0_29D0);
            }
            FAN => {
                fan(fx, t);
                frame(fx, low, 0x30, 2, 0x01E0_29F0);
            }
            CROSS => {
                cross(fx, t);
                frame(fx, low, 0x20, 1, 0x01E0_2A10);
            }
            f => {
                debug_assert!(false, "b192: unported task step {f:#x}");
            }
        }
    }
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x70, dd, 0x01E0_2AA4);
    let d = sp + 0xF8;
    // The entry's s1: the live tasks seen so far (counted in a delay slot).
    let mut s1 = 0u64;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        s1 += 1;
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x24);
        let set_rect = |fx: &mut Fx, tex: u32, local: u32, id: i32| {
            let w = fx.mem.read_u32(D + tex);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x34);
            l2::rect(fx, local, id, sub);
            for k in 0..4 {
                let v = fx.mem.read_u32(local + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, v);
            }
        };
        if f(fx) == FAN_BIG {
            fan_draw(fx, t, d);
        }
        if f(fx) == BEAM {
            beam_draw(fx, t);
            beam_draw(fx, t);
        }
        if f(fx) == BLADE {
            blade_draw(fx, t, d, 0, s1);
            blade_draw(fx, t, d, 1, s1);
        }
        if f(fx) == RING {
            set_rect(fx, 0x36C, sp + 0xA0, 0x9B);
            fx.mem.set16(D + 0xB8, 0xE);
            billboard::ring_t(fx, uv::uv_rect4, D, t, d, TABLE);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == DRIFT {
            set_rect(fx, 0x338, sp + 0xB0, 0x14);
            drift_draw(fx, 0x01E0_0780, t, d);
            drift_draw(fx, 0x01E0_0780, t, d);
        }
        if f(fx) == FAN {
            fan_draw(fx, t, d);
        }
        if f(fx) == CROSS {
            cross_draw(fx, t, d);
            cross_draw(fx, t, d);
        }
    }
    if fx.mem.i16(COUNTER) >= 0x15 {
        let half = 0x3FE0_0000_0000_0000u64;
        let v = fx.mem.f(D + 0x300);
        if dcmp(fx, v, half, D, 0x01E0_2DF0, 0x01E0_2E00, false) {
            let v = ee::add(fx.mem.f(D + 0x300), f32::from_bits(0x3C23_D70A));
            fx.mem.setf(D + 0x300, v);
        }
        fx.mem.write_u32(D + 0x304, 0x3DCC_CCCD);
        let v = fx.mem.f(D + 0x308);
        if dcmp(fx, v, half, D, 0x01E0_2E38, 0x01E0_2E48, false) {
            let v = ee::add(fx.mem.f(D + 0x308), f32::from_bits(0x3C23_D70A));
            fx.mem.setf(D + 0x308, v);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x4B => 1,
        0x5F => 4,
        0x55 => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E02F00(D, step, 0, pool): the first free task of the first D+0xB8,
/// cleared and set off at D+0 (as i16) with the colour D+0x8C (twice),
/// +0x30 = D+0xB2; 1 when one was free. Frame 0x80 with the emitter's s0
/// .. s4 (`s`), s5 / s6 the runner's (0 [H]).
fn spawn_task(fx: &mut Fx, step: u32, s: [u64; 5], ra: u32) -> i32 {
    let sp = fx.enter(0x80);
    fx.saved(sp, &[(0, s[0]), (0x10, s[1]), (0x20, s[2]), (0x30, s[3]), (0x40, s[4]), (0x50, 0), (0x60, 0), (0x70, u64::from(ra))]);
    let mut found = None;
    let mut k = 0i32;
    while k < fx.mem.i16(D + 0xB8) {
        let t = POOL + 0x40 * k as u32;
        if fx.mem.i16(t + 0x36) == 0 {
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            found = Some(t);
            k = fx.mem.i16(D + 0xB8);
        }
        k = (k + 1) as i16 as i32;
    }
    let ret = match found {
        Some(t) => {
            for c in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(D + 0x8C), fx.mem.read_u8(D + 0x8D), fx.mem.read_u8(D + 0x8E));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(D + 0x8F);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D, 0);
            let a = fx.mem.i16(D + 0xB2);
            fx.mem.set16(t + 0x30, a);
            fx.mem.write_u32(t + 0x20, 0);
            fx.mem.write_u32(t + 0x24, step);
            1
        }
        None => 0,
    };
    fx.leave(0x80);
    ret
}

/// 0x1E00FB0(D, 0): the colour D+0x3614 (0x80, 0, 0, 0), the counter, the
/// spawn point at node 1 and the spawns (frame 0xA0: the entry's s0 = D,
/// s1 = 0, s2 = param (-1), s3 / s4 the runner's (0 [H]), ra).
fn emit(fx: &mut Fx) {
    let dd = u64::from(D);
    let sp = fx.enter(0xA0);
    fx.saved(sp, &[(0x10, dd), (0x20, 0), (0x30, u64::MAX), (0x40, 0), (0x50, 0), (0x60, 0x01E0_2784)]);
    fx.mem.write_u8(D + 0x3614, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(D + 0x3614 + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let (s2, s3) = (u64::from(sp + 0x74), u64::from(sp + 0x78));
    let node = |fx: &mut Fx, o: u32, s0: u64, s1: u64, ra: u32| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + o);
        node_position_saving(fx, h, slot, sp + 0x70, s0, Some(s1), ra);
    };
    let at_node = |fx: &mut Fx| {
        for k in 0..3 {
            let w = fx.mem.read_u32(sp + 0x70 + 4 * k);
            fx.mem.write_u32(D + 4 * k, w);
        }
    };
    node(fx, 0, dd, 0, 0x01E0_100C);
    at_node(fx);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let base = [dd, 0, s2, s3, dd];
    if c(fx) >= 0x20 {
        spawn_task(fx, DRIFT, base, 0x01E0_1054);
    }
    if c(fx) >= 0x20 {
        spawn_task(fx, FAN, base, 0x01E0_107C);
    }
    if c(fx) >= 0x24 {
        spawn_task(fx, CROSS, base, 0x01E0_10A4);
    }
    if c(fx) == 0x55 {
        spawn_task(fx, FAN_BIG, base, 0x01E0_10CC);
    }
    node(fx, 4, dd, 0, 0x01E0_10E4);
    let (mut s0, mut s1) = (dd, 0u64);
    if c(fx) == 0x55 {
        let f = facing_of(fx, sp + 0x80, 0x01E0_1130);
        s1 = f as i64 as u64;
        for i in 0..0xFu32 {
            fx.rand_from(i, 0x01E0_1140);
            l2::sin_deg(fx, SINES, (f - 0xB4) as i16 as i32, 1);
            for (k, ra) in [(0u32, 0x01E0_1160u32), (1, 0x01E0_1194), (2, 0x01E0_11C8)] {
                let r = rmod(fx, i, ra, 0x14);
                let v = ee::sub(ee::add(fx.mem.f(sp + 0x70 + 4 * k), ee::from_i32(r)), 10.0);
                fx.mem.setf(D + 4 * k, v);
            }
            spawn_task(fx, RING, [u64::from(i), s1, s2, s3, dd], 0x01E0_1210);
        }
        s0 = 0xF;
    }
    if c(fx) == 0x55 {
        spawn_task(fx, BEAM, [s0, s1, s2, s3, dd], 0x01E0_1248);
    }
    if c(fx) == 0x55 {
        let f = facing_of(fx, sp + 0x90, 0x01E0_1294);
        let s1 = f as i64 as u64;
        let f20 = ee::neg(l2::sin_deg(fx, SINES, (f - 0xB4) as i16 as i32, 1));
        for i in 0..0xDu32 {
            let f21 = ee::mul(f20, ee::from_i32(i as i32));
            for (kind, ra) in [(0, 0x01E0_12F8u32), (1, 0x01E0_1334)] {
                for k in 0..2 {
                    let w = fx.mem.read_u32(sp + 0x70 + 4 * k);
                    fx.mem.write_u32(D + 4 * k, w);
                }
                let z = ee::add(fx.mem.f(sp + 0x78), f21);
                fx.mem.setf(D + 8, z);
                fx.mem.set16(D + 0xB2, kind);
                spawn_task(fx, BLADE, [u64::from(i), s1, s2, s3, dd], ra);
            }
        }
    }
    let a = fx.mem.i16(ANGLE) + 2;
    fx.mem.set16(ANGLE, a);
    let a = fx.mem.i16(ANGLE);
    if a >= 0x168 {
        fx.mem.set16(ANGLE, a % 0x168);
    }
    fx.leave(0xA0);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// Alpha down by `down` while at least `floor`, else the task ends.
fn fade_out(fx: &mut Fx, t: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a >= floor {
        fx.mem.write_u8(t + 0x2B, a - down);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// The two fans' shared fade: in by `up` (below `cap`) for 2 updates;
/// then while the size is at least 0x15 out by 0xC (0xD for the big one,
/// floor 0xE / 0xF), and once the timer is under 10 out by 0xF.
fn fan_fade(fx: &mut Fx, t: u32, cap: u8, up: u8, floor: u8, down: u8) {
    if fx.mem.i16(t + 0x32) < 2 {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < cap {
            fx.mem.write_u8(t + 0x2B, a + up);
        }
        return;
    }
    if fx.mem.i16(t + 0x38) >= 0x15 {
        fade_out(fx, t, floor, down);
    }
    if fx.mem.i32(TIMER) < 0xA {
        fade_out(fx, t, 0x11, 0xF);
    }
}

/// 0x1E015D0 (D, task): the big green fan (size 650 up by 4, cell rand %
/// 7, alpha +0x2F 0x10; colours (0x40, 0xF0, 0x40) / (0x10, 0x80, 0x10)
/// every update).
fn fan_big(fx: &mut Fx, t: u32) {
    let at = FAN_BIG;
    fx.enter(0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x28A);
        let v = rmod(fx, t, at + 0x30, 7);
        fx.mem.set16(t + 0x34, v);
        fx.mem.write_u8(t + 0x2F, 0x10);
        fx.mem.set16(t + 0x36, 2);
    }
    fan_fade(fx, t, 0xA0, 0x50, 0xF, 0xD);
    l2::rgb(fx, t + 0x28, 0x40, 0xF0, 0x40);
    l2::rgb(fx, t + 0x2C, 0x10, 0x80, 0x10);
    age(fx, t);
    let s = fx.mem.i16(t + 0x38) + 4;
    fx.mem.set16(t + 0x38, s);
    fx.leave(0x30);
}

/// 0x1E020D0 (D, task): a green fan (size 260 up by 4, cell rand % 7,
/// alpha +0x2F 8; (0x10, 0x60, 0x10) / (8, 0x20, 8) before update 0x4A,
/// else (0x10, 0x80, 0x10) / (8, 0x40, 8)).
fn fan(fx: &mut Fx, t: u32) {
    let at = FAN;
    fx.enter(0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x104);
        let v = rmod(fx, t, at + 0x30, 7);
        fx.mem.set16(t + 0x34, v);
        fx.mem.write_u8(t + 0x2F, 8);
        if fx.mem.i16(COUNTER) < 0x4A {
            l2::rgb(fx, t + 0x28, 0x10, 0x60, 0x10);
            l2::rgb(fx, t + 0x2C, 8, 0x20, 8);
        } else {
            l2::rgb(fx, t + 0x28, 0x10, 0x80, 0x10);
            l2::rgb(fx, t + 0x2C, 8, 0x40, 8);
        }
        fx.mem.set16(t + 0x36, 2);
    }
    fan_fade(fx, t, 0x20, 0x10, 0xE, 0xC);
    age(fx, t);
    let s = fx.mem.i16(t + 0x38) + 4;
    fx.mem.set16(t + 0x38, s);
    fx.leave(0x30);
}

/// 0x1E01A60 (D, task): a green blade ((0x10, 0x40, 0x10) every update;
/// size 4, angles +0x18 / +0x14 / +0x18 rand % 0xFFFF, sideways speed +8
/// 15 (kind 0) / -15, growth +0xA 10..24); in by 0x2A for 3 updates, then
/// out by 10 (over below 0xC); +6 by +8, size by +0xA.
fn blade(fx: &mut Fx, t: u32) {
    let at = BLADE;
    fx.enter(0x20);
    l2::rgb(fx, t + 0x28, 0x10, 0x40, 0x10);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 4);
        for (o, ra) in [(0x18u32, 0x40u32), (0x14, 0x5C), (0x18, 0x78)] {
            let v = rmod(fx, t, at + ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        let v = if fx.mem.i16(t + 0x30) != 0 { -0xF } else { 0xF };
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, at + 0xB8, 0xF);
        fx.mem.set16(t + 0xA, v + 0xA);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 3 {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x80 {
            fx.mem.write_u8(t + 0x2B, a + 0x2A);
        }
    } else {
        fade_out(fx, t, 0xC, 0xA);
    }
    let v = fx.mem.i16(t + 6) + fx.mem.i16(t + 8);
    fx.mem.set16(t + 6, v);
    let v = fx.mem.i16(t + 0x38) + fx.mem.i16(t + 0xA);
    fx.mem.set16(t + 0x38, v);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E01DA0 (D, task): a green ring (size 35, cell rand & 3, (0x20,
/// 0x80, 0x20), +0x3A rand % 360, +0xE 3..18, row +8 rand % 10, life 5..11);
/// 0x80 for its life, then out by 10; radius up by +0xE after the first
/// update.
fn ring(fx: &mut Fx, t: u32) {
    let at = RING;
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x23);
        let v = fx.rand_from(t, at + 0x28);
        fx.mem.set16(t + 0x34, v & 3);
        l2::rgb(fx, t + 0x28, 0x20, 0x80, 0x20);
        fx.mem.set16(t + 0x30, 0);
        let v = rmod(fx, t, at + 0x5C, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = fx.rand_from(t, at + 0x78);
        fx.mem.set16(t + 0xE, (v & 0xF) + 3);
        let v = rmod(fx, t, at + 0x98, 0xA);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, at + 0xB4, 7);
        fx.mem.set16(t + 6, v + 5);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fade_out(fx, t, 0x11, 0xA);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    age(fx, t);
    fx.leave(0x20);
}

fn fix(fx: &mut Fx, v: f32, ra: u32) -> i32 {
    fx.fix_residue(v, ra);
    ee::fixsfsi(v)
}

/// 0x1E01F00 (D, task): b709's drift (0x1E04220 there) in green (size
/// 130, (0x20, 0x40, 0x20)), y by 6 cos +0x30.
fn drift(fx: &mut Fx, t: u32) {
    let at = DRIFT;
    fx.enter(0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x82);
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, 0x20, 0x40, 0x20);
        let v = rmod(fx, t, at + 0x4C, 0x168);
        fx.mem.set16(t + 0x30, v);
        let v = rmod(fx, t, at + 0x68, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0xA {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fade_out(fx, t, 0x11, 0xA);
    }
    if fx.mem.i16(t + 0x32) % 3 == 1 && fx.mem.i16(t + 0x34) < 7 {
        let v = fx.mem.i16(t + 0x34) + 1;
        fx.mem.set16(t + 0x34, v);
    }
    age(fx, t);
    for (o, deg, cos, k, ra) in [(8u32, 0x3Au32, 0, 14.0f32, 0x140u32), (0xA, 0x30, 1, 6.0, 0x178), (0xC, 0x3A, 1, 14.0, 0x1B0)] {
        let a = fx.mem.i16(t + deg);
        let s = l2::sin_deg(fx, SINES, a, cos);
        let v = ee::add(ee::from_i32(fx.mem.i16(t + o)), ee::mul(k, s));
        let r = fix(fx, v, at + ra);
        fx.mem.set16(t + o, r);
    }
    fx.leave(0x30);
}

/// 0x1E02250 (D, task): the grey cross (size 60 (110 from update 0x4A) up
/// by 2, +0x3A from D+0x3650 turning by 15, cell rand % 7, alpha +0x2F 8,
/// (0x18, 0x18, 0x18)); in by 0x40 (below 0x80) for 2 updates, then out
/// by 0xC (over below 0xE).
fn cross(fx: &mut Fx, t: u32) {
    let at = CROSS;
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x3C);
        if fx.mem.i16(COUNTER) >= 0x4A {
            fx.mem.set16(t + 0x38, 0x6E);
        }
        let a = fx.mem.i16(ANGLE);
        fx.mem.set16(t + 0x3A, a);
        let v = rmod(fx, t, at + 0x48, 7);
        fx.mem.set16(t + 0x34, v);
        fx.mem.write_u8(t + 0x2F, 8);
        l2::rgb(fx, t + 0x28, 0x18, 0x18, 0x18);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 2 {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x80 {
            fx.mem.write_u8(t + 0x2B, a + 0x40);
        }
    } else {
        fade_out(fx, t, 0xE, 0xC);
    }
    age(fx, t);
    let s = fx.mem.i16(t + 0x38) + 2;
    fx.mem.set16(t + 0x38, s);
    let a = fx.mem.i16(t + 0x3A) + 0xF;
    fx.mem.set16(t + 0x3A, a);
    let a = fx.mem.i16(t + 0x3A);
    if a >= 0x168 {
        fx.mem.set16(t + 0x3A, a % 0x168);
    }
    fx.leave(0x20);
}

/// 0x1E00CE0 (D, task, desc, _): an untextured gouraud disc of 18 strips
/// into `desc` (centre colour +0x28, rim +0x2C; 20 degrees each from i16
/// +0x3A, rim radius i16 +0x38) at the task's i16 point, turned by the
/// camera's angles plus the task's (+0x4000 on x; scale 0.1); each keyed
/// by its mean w into D+0x80 (`quads4::fan18` without the drift, the
/// task's angles added) [F].
fn fan_draw(fx: &mut Fx, t: u32, d: u32) {
    let sp = fx.enter(0x160);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    for (local, c, to) in [(0x140u32, 0x28u32, 0x10u32), (0x148, 0x28, 0x20), (0x150, 0x2C, 0x30), (0x158, 0x2C, 0x40)] {
        rgbaq_at(fx, sp + local, t + c, d + to);
    }
    for k in 0..3 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(sp + 0xC0 + 4 * k, v);
        fx.mem.write_u32(sp + 0xD0 + 4 * k, 0x3DCC_CCCD);
    }
    crate::b494::camera_angles_saving(fx, sp + 0x130, u64::from(D), 0, 0x01E0_0DFC);
    for k in 0..3u32 {
        let add = fx.mem.read_u32(t + 0x10 + 4 * k).wrapping_add(if k == 0 { 0x4000 } else { 0 });
        let v = fx.mem.read_u32(sp + 0x130 + 4 * k).wrapping_add(add);
        fx.mem.write_u32(sp + 0x130 + 4 * k, v);
    }
    l2::xform_vp(fx, sp + 0xE0, sp + 0xD0, sp + 0x130, sp + 0xC0);
    for o in [0x84u32, 0x94, 0xA4, 0xB4, 0x80, 0x88, 0x90, 0x98] {
        fx.mem.set32(sp + o, 0);
    }
    let tab = D + 0xD8;
    let mut a = 0i32;
    for _ in 0..0x12 {
        let s = (fx.mem.i16(t + 0x3A) + a) as i16 as i32;
        let e = (a + (fx.mem.i16(t + 0x3A) + 0x14)) as i16 as i32;
        for (deg, v) in [(s, 0xA0u32), (e, 0xB0)] {
            let c = l2::sin_deg(fx, tab, deg, 1);
            let r = ee::from_i32(fx.mem.i16(t + 0x38));
            fx.mem.setf(sp + v, ee::mul(r, c));
            let sn = l2::sin_deg(fx, tab, deg, 0);
            let r = ee::from_i32(fx.mem.i16(t + 0x38));
            fx.mem.setf(sp + v + 8, ee::mul(r, sn));
        }
        if l2::project(fx, sp + 0x80, SPR, sp + 0xE0, d, sp + 0x120) {
            let key = ee::fixunssfsi(fx.mem.f(sp + 0x120)) as i32;
            let ot = fx.mem.read_u32(D + 0x80);
            fx.submit(d, ot, key);
        }
        a = (a + 0x14) as i16 as i32;
    }
    fx.leave(0x160);
}

/// 0x1E03310 (D, task, _, _): the beam: an untextured gouraud strip in
/// its own descriptor (sp + 0x128) for each of the task's age - 1
/// segments of the node 0xD / 0xE histories D+0x34C0 / D+0x3560 (the
/// second edge lowered by half the first points' y gap + 1 / 50 and back
/// by half their z gap + 1 / 20), colour +0x28 with its alpha down by
/// alpha / age a segment (wrapping); at the far Z, key 0.
fn beam_draw(fx: &mut Fx, t: u32) {
    use ee::{add, div, sub};
    let sp = fx.enter(0x1C0);
    let n = fx.mem.i16(t + 0x32);
    let d = sp + 0x128;
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    let colour = sp + 0x1BC;
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(colour + k, v);
    }
    for (to, from) in [(0x100u32, 0x34C0u32), (0x110, 0x3560)] {
        for k in 0..4 {
            let w = fx.mem.read_u32(D + from + 4 * k);
            fx.mem.write_u32(sp + to + 4 * k, w);
        }
    }
    for (o, by) in [(0x104u32, 50.0f32), (0x108, 20.0)] {
        let g = sub(fx.mem.f(sp + o), fx.mem.f(sp + o + 0x10));
        fx.mem.setf(sp + o, g);
        let g = fx.mem.f(sp + o);
        fx.mem.setf(sp + o, add(g, div(g, by)));
    }
    let (f21, f22) = (fx.mem.f(sp + 0x104), fx.mem.f(sp + 0x108));
    let mut s0 = 0i32;
    let mut s6 = D;
    while s0 < n - 1 {
        l2::colour_strip4(fx, d, colour, 1);
        let al = i32::from(fx.mem.read_u8(t + 0x2B));
        let q = fx.div(al, n).0 & 0xFF;
        let v = i32::from(fx.mem.read_u8(colour + 3)) - q;
        fx.mem.write_u8(colour + 3, v as u8);
        l2::colour_strip4(fx, d, colour, 2);
        let (h2, h3) = (div(f21, 2.0), div(f22, 2.0));
        for (to, from) in [(0xC0u32, 0x34C0u32), (0xD0, 0x3560), (0xE0, 0x34D0), (0xF0, 0x3570)] {
            for k in 0..3 {
                let w = fx.mem.read_u32(s6 + from + 4 * k);
                fx.mem.write_u32(sp + to + 4 * k, w);
            }
        }
        for (o, by) in [(0xD4u32, h2), (0xF4, h2), (0xD8, h3), (0xF8, h3)] {
            let v = sub(fx.mem.f(sp + o), by);
            fx.mem.setf(sp + o, v);
        }
        if l2::project(fx, sp + 0xC0, SPR, 0, d, sp + 0xB0) {
            l2::far_z_desc(fx, d, 1);
            fx.submit(d, OT, 0);
        }
        s6 += 0x10;
        s0 += 1;
    }
    fx.leave(0x1C0);
}

/// 0x1E03600 (D, task, desc, mode): mode 0 sets `desc` up (a strip3, the
/// rect 0x97 at half height in order 0, colour +0x28, texture D+0x36C);
/// mode 1 draws the blade: two quads (3 x size, corners 2 / 3 at y 0)
/// shifted by +-i16 +6 in x, scaled (0.2, 0.8, 1), turned by the camera's
/// angles plus the task's, at the task's point plus its drift / 10; keyed
/// by mean w into D+0x361C. `s1`: the entry's (the live tasks so far).
fn blade_draw(fx: &mut Fx, t: u32, d: u32, mode: i32, s1: u64) {
    let sp = fx.enter(0x150);
    if mode == 0 {
        l2::rect(fx, sp + 0xC0, 0x97, 0);
        let h = fx.mem.i16(sp + 0xC6) >> 1;
        fx.mem.set16(sp + 0xC6, h);
        l2::desc_strip3(fx, d);
        for (local, to) in [(0x130u32, 0x10u32), (0x138, 0x28), (0x140, 0x40), (0x148, 0x58)] {
            rgbaq_at(fx, sp + local, t + 0x28, d + to);
        }
        l2::uv_strip3(fx, d, sp + 0xC0, 0);
        let tex = fx.mem.read_u32(D + 0x36C);
        fx.desc_texture(d, tex, 1);
        fx.leave(0x150);
        return;
    }
    for (k, v) in [0x3E4C_CCCDu32, 0x3F4C_CCCD, 0x3F80_0000].into_iter().enumerate() {
        fx.mem.write_u32(sp + 0xB0 + 4 * k as u32, v);
    }
    crate::b494::camera_angles_saving(fx, sp + 0x120, mode as u64, s1, 0x01E0_3738);
    for k in 0..3 {
        let v = fx.mem.read_u32(sp + 0x120 + 4 * k).wrapping_add(fx.mem.read_u32(t + 0x10 + 4 * k));
        fx.mem.write_u32(sp + 0x120 + 4 * k, v);
    }
    for k in 0..3 {
        let p = ee::from_i32(fx.mem.i16(t + 2 * k));
        let o = ee::div(ee::from_i32(fx.mem.i16(t + 8 + 2 * k)), 10.0);
        fx.mem.setf(sp + 0xA0 + 4 * k, ee::add(p, o));
    }
    l2::xform_vp(fx, sp + 0xE0, sp + 0xB0, sp + 0x120, sp + 0xA0);
    let q = sp + 0x60;
    for i in 0..2 {
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, q, 3.0, s);
        fx.mem.write_u32(q + 0x24, 0);
        fx.mem.write_u32(q + 0x34, 0);
        for o in [0u32, 0x10] {
            let x = ee::from_i32(fx.mem.i16(t + 6));
            let v = fx.mem.f(q + o);
            fx.mem.setf(q + o, if i == 0 { ee::add(v, x) } else { ee::sub(v, x) });
        }
        if l2::project(fx, q, SPR, sp + 0xE0, d, sp + 0xD0) {
            let key = ee::fixunssfsi(fx.mem.f(sp + 0xD0)) as i32;
            let ot = fx.mem.read_u32(OT2);
            fx.submit(d, ot, key);
        }
    }
    fx.leave(0x150);
}

/// 0x1E03920 (D, task, desc, _): four textured squares (9 values, FST,
/// ALPHA 0x48; the rect 0x14 frame +0x34 in order 0, texture D+0x330,
/// colour +0x28) of the i16 size +0x38 at 90-degree steps from +0x3A on a
/// circle of radius 120, facing the camera at the task's point plus its
/// drift / 10 (scale 0.1); keyed by mean w into D+0x361C.
fn cross_draw(fx: &mut Fx, t: u32, d: u32) {
    let sp = fx.enter(0x160);
    let sub = fx.mem.i16(t + 0x34);
    l2::rect(fx, sp + 0xE0, 0x14, sub);
    desc_uv4(fx, d);
    fx.desc_alpha(d, 0x80_0000_0048);
    fx.mem.write_u32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    rgbaq_at(fx, sp + 0x158, t + 0x28, d + 0x10);
    uv::uv_rect4(fx, d, sp + 0xE0, 0);
    let tex = fx.mem.read_u32(D + 0x330);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xD0 + 4 * k, 0x3DCC_CCCD);
    }
    crate::b494::camera_angles_saving(fx, sp + 0x140, u64::from(D), 0, 0x01E0_3A24);
    for k in 0..3 {
        let p = ee::from_i32(fx.mem.i16(t + 2 * k));
        let o = ee::div(ee::from_i32(fx.mem.i16(t + 8 + 2 * k)), 10.0);
        fx.mem.setf(sp + 0xC0 + 4 * k, ee::add(p, o));
    }
    l2::xform_vp(fx, sp + 0x100, sp + 0xD0, sp + 0x140, sp + 0xC0);
    let q = sp + 0x80;
    let mut a = 0i32;
    for _ in 0..4 {
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, q, s, s);
        let deg = (a + fx.mem.i16(t + 0x3A)) as i16 as i32;
        let x = ee::mul(120.0, l2::sin_deg(fx, SINES, deg, 0));
        let deg = (a + fx.mem.i16(t + 0x3A)) as i16 as i32;
        let y = ee::mul(120.0, l2::sin_deg(fx, SINES, deg, 1));
        l2::add_column(fx, q, 0, x);
        l2::add_column(fx, q, 1, y);
        a = (a + 0x5A) as i16 as i32;
        if l2::project(fx, q, SPR, sp + 0x100, d, sp + 0xF0) {
            let key = ee::fixunssfsi(fx.mem.f(sp + 0xF0)) as i32;
            let ot = fx.mem.read_u32(OT2);
            fx.submit(d, ot, key);
        }
    }
    fx.leave(0x160);
}
