//! BMODULE b225 (card 140), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03760, data area 0x1ED0000, 0x3860 bytes) [F]
//!: b709's sibling (B26's map) on b636's older
//! pool (180 tasks of 0x40 at D+0x478, spawned by 0x1E04340 = b709's spawn
//! on colour D+0x3384 / kind D+0x33E2) with the globals shifted (timer
//! D+0x3380 = 0xFA, OT D+0x338C, nodes D+0x33D0.. (the attacker's 0x1A /
//! 0x19 / 0x35 / 0x34, 0xC / 0x3F / 0x64), facing D+0x33E4, counter
//! D+0x33E8, tables D+0x33EC / 0x34EC / 0x35EC, sines D+0x36EC). b709's
//! steps / draws run on [`b709::Rec`] (dome 0x1E01EA0, blue 0x2120, orange
//! 0x2540, strips 0x2810, grey 0x2A70, dome draw 0x4460, strip ring 0x4890).
//!
//! The emitter 0x1E01560 follows nodes 0x1A / 0x19 (D+0x31B0 / 0x320) and
//! 0x35 / 0x34 (D+0x31C0 / 0x330) to update 0xDB, the first point pushed
//! +-5 in z every update by the facing; updates 1..7 seven grey flashes and
//! on 1 a pink one (0x1E033B0, run by the unit flash 0x1E04F40: the
//! attacker drawn into the target over the window, growing from age 0x24);
//! 0x79 / 0x8C / 0xA0 at node 0xC and 0x82 / 0x96 at node 0x3F ten smoke
//! sprites 0x2290 (`billboard::sprite_square`); 0xB9..0xC8 / 0xC8..0xD9
//! blue rings at nodes 0x19 / 0x34; 0xC0..0xC8 / 0xD2..0xD9 domes, on even
//! / odd updates a purple ring 0x23F0 and an orange strip ring at the
//! points D+0x31F0 / 0x3200 and the camera 0x3120 (`task40::camera_at`) on
//! 0xC0 / 0xD2; 0xC1 / 0xD3 at the first / second point the disc
//! 5225257f1409, three grey rings, ten strip rings 0x2CD0
//! (`billboard::ring_strip3`), four blue strip rings and the disc
//! 78621d35edfb. Returns 1 on update 0xB8, 4 on 0xF5, 2 on 0xE1, 6 on
//! 0xC3..0xDB, 8 once the timer has run out.
//!
//! The steps 0x1E026A0, 0x2F50, 0x32A0 and the object op 0x3360 are tested
//! by the entry but never spawned [F: the emitter names 0x1EA0 / 0x2120 /
//! 0x2290 / 0x23F0 / 0x2540 / 0x2810 / 0x2A70 / 0x2BF0 / 0x2CD0 / 0x2E70 /
//! 0x3120 / 0x33B0 only]; not ported. No 0x1F8100.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b148::{facing_of, rmod};
use crate::b709::{self, Rec};
use crate::blib::{billboard, quads4, spawn, task40, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 180;
const TIMER: u32 = D + 0x3380;
const COLOUR: u32 = D + 0x3384;
const OT2: u32 = D + 0x338C;
const FLASH_DESC: u32 = D + 0x3398;
const LAYER: u32 = D + 0x3394;
const NODES: u32 = D + 0x33D0;
const KIND: u32 = D + 0x33E2;
const FACING: u32 = D + 0x33E4;
const COUNTER: u32 = D + 0x33E8;
const TABLE: u32 = D + 0x33EC;
const TABLE2: u32 = D + 0x34EC;
const TABLE3: u32 = D + 0x35EC;
const SINES: u32 = D + 0x36EC;

const DOME: u32 = 0x01E0_1EA0;
const BLUE: u32 = 0x01E0_2120;
const SMOKE: u32 = 0x01E0_2290;
const PURPLE: u32 = 0x01E0_23F0;
const ORANGE: u32 = 0x01E0_2540;
const STRIPS: u32 = 0x01E0_2810;
const GREY: u32 = 0x01E0_2A70;
const DISC_B: u32 = 0x01E0_2BF0;
const RING: u32 = 0x01E0_2CD0;
const DISC: u32 = 0x01E0_2E70;
const CAMERA: u32 = 0x01E0_3120;
const FLASH: u32 = 0x01E0_33B0;

const R: Rec = Rec { counter: COUNTER, sines: SINES, ot2: OT2, key: D + 0x33BC, dome_step: 4, rows_swapped: true, orange_e: 0x14, orange_fade: 0xF, grey_size: 0x1A, grey_e: 0x14 };

/// The entry 0x1E03760 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1D0);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(OT2, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x1D0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x3860);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x33E6, param);
    fx.mem.set32(TIMER, 0xFA);
    for k in 0..3 {
        fx.mem.write_u32(D + 0x310 + 4 * k, 0);
    }
    for (o, n) in [(0u32, 0x1A), (2, 0x19), (4, 0x35), (6, 0x34), (0xA, 0xC), (0xC, 0x3F), (0xE, 0x64)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(o == 0));
        fx.mem.set16(NODES + o, r);
    }
    // 0x1E036B0(0, D): the textures (frame 0x40: the entry's s0 = D, s1 =
    // 0, s2 = 0, ra).
    let f = fx.enter(0x40);
    fx.saved(f, &[(0, u64::from(D)), (0x10, 0), (0x20, 0), (0x30, 0x01E0_3888)]);
    for k in 0..3 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x340 + 4 * k as u32, t);
    }
    for k in 0xD..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x374 + 4 * (k as u32 - 0xD), t);
    }
    for k in 0..5 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(D + 0x3360 + 4 * k as u32, t);
    }
    fx.leave(0x40);
    let f = facing_of(fx, sp + 0x110, 0x01E0_38C4);
    let g = facing_of(fx, sp + 0x120, 0x01E0_3900);
    fx.mem.set16(FACING, g);
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_390C, 0x14);
        fx.mem.set8(TABLE + k, v - 0xA);
    }
    for k in 0xB4..0x100u32 {
        let v = if f != 0 { rmod(fx, D, 0x01E0_3948, 6) + 2 } else { -(rmod(fx, D, 0x01E0_3974, 6) + 2) };
        fx.mem.set8(TABLE + k, v);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_39B4, 0x1E);
        fx.mem.set8(TABLE2 + k, v - 0xF);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_39E8, 0x1E) - 0xF;
        let v = match v {
            0..=4 => v + 7,
            -4..=-1 => v - 7,
            _ => v,
        };
        fx.mem.set8(TABLE3 + k, v);
    }
    0
}

/// The step frames' saves: the entry's s0..s(n-1) (D, the flashes counted
/// so far, 0) and the return address `ra` above them.
fn frame(fx: &mut Fx, low: u32, size: u32, n: u32, flashes: u32, ra: u32) {
    let vals = [u64::from(D), u64::from(flashes), 0];
    let base = low - size;
    for i in 0..n {
        fx.saved(base + 0x10 * i, &[(0, vals[i as usize])]);
    }
    fx.saved(base, &[(0x10 * n, u64::from(ra))]);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    // a3 as the last call left it, for the unit flash's first TEXFLUSH.
    let mut a3 = emit(fx).unwrap_or(0);
    let dd = u64::from(D);
    let low = fx.sp;
    let mut flashes = 0u32;
    for k in 0..TASKS {
        let t = D + 0x478 + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let rand = fx.rand_state();
        let f = fx.mem.read_u32(t + 0x24);
        match f {
            DOME => {
                b709::dome(fx, &R, DOME, t);
                frame(fx, low, 0x50, 3, flashes, 0x01E0_3AB0);
            }
            BLUE => {
                b709::blue(fx, BLUE, t);
                frame(fx, low, 0x20, 1, flashes, 0x01E0_3AD0);
            }
            SMOKE => {
                smoke(fx, t);
                frame(fx, low, 0x30, 2, flashes, 0x01E0_3AF0);
            }
            PURPLE => {
                purple(fx, t);
                frame(fx, low, 0x20, 1, flashes, 0x01E0_3B10);
            }
            ORANGE => {
                b709::orange(fx, &R, ORANGE, t);
                frame(fx, low, 0x20, 1, flashes, 0x01E0_3B30);
            }
            STRIPS => {
                b709::strips(fx, &R, STRIPS, t);
                frame(fx, low, 0x30, 2, flashes, 0x01E0_3B70);
            }
            GREY => {
                b709::grey(fx, &R, GREY, t);
                frame(fx, low, 0x20, 1, flashes, 0x01E0_3B90);
            }
            DISC_B => {
                let c = spawn::disc("78621d35edfb").expect("constants");
                spawn::disc_step_with(fx, c, DISC_B, t);
                frame(fx, low, 0x20, 1, flashes, 0x01E0_3BB0);
            }
            RING => {
                ring(fx, t);
                frame(fx, low, 0x20, 1, flashes, 0x01E0_3BD0);
            }
            DISC => {
                let c = spawn::disc("5225257f1409").expect("constants");
                spawn::disc_step_with(fx, c, DISC, t);
                frame(fx, low, 0x20, 1, flashes, 0x01E0_3BF0);
            }
            CAMERA => {
                task40::camera_at(fx, CAMERA, D, t, 0x3180, 0x3380);
                frame(fx, low, 0x30, 2, flashes, 0x01E0_3C30);
            }
            FLASH => flashes += 1,
            f => {
                debug_assert!(false, "b225: unported task step {f:#x}");
            }
        }
        // a3 after a step: the last rand's new state's low word - 1 (its
        // __muldi3), or the late blue strips' 0x107FD0 b = 0xC0 [run].
        if fx.rand_state() != rand {
            a3 = (fx.rand_state() as u32).wrapping_sub(1);
        } else if f == STRIPS {
            let late = match fx.mem.i16(t + 0x3A) {
                1 => fx.mem.i16(COUNTER) >= 0xBA,
                0 => fx.mem.i16(COUNTER) >= 0xC1,
                _ => false,
            };
            if late {
                a3 = 0xC0;
            }
        }
    }
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x60, dd, 0x01E0_3CE4);
    let d = sp + 0x138;
    for k in 0..TASKS {
        let t = D + 0x478 + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        fx.mem.set16(D + 0xB8, 0);
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
        if f(fx) == DOME {
            b709::dome_draw(fx, &R, t, d, 0);
            b709::dome_draw(fx, &R, t, d, 1);
            a3 = 0;
        }
        if f(fx) == BLUE {
            set_rect(fx, 0x37C, sp + 0x70, 0x9B);
            fx.mem.set16(D + 0xB8, 4);
            quads4::ring_into(fx, uv::uv_rect4, D, t, d, TABLE);
            a3 = 0;
        }
        if f(fx) == SMOKE {
            set_rect(fx, 0x3370, sp + 0x80, 0x14);
            let p = sp + 0x60;
            for (o, from) in [(0u32, 0u32), (4, 2), (8, 4)] {
                let v = ee::from_i32(fx.mem.i16(t + from));
                fx.mem.setf(p + o, v);
            }
            for (o, by) in [(0u32, 8u32), (8, 0xC)] {
                let q = ee::div(ee::from_i32(fx.mem.i16(t + by)), 10.0);
                let v = ee::add(fx.mem.f(p + o), q);
                fx.mem.setf(p + o, v);
            }
            billboard::sprite_square(fx, 0x01E0_1370, uv::uv_rect4, D, t, 0, p);
            a3 = 0;
        }
        if f(fx) == PURPLE {
            set_rect(fx, 0x3360, sp + 0x90, 0x14);
            fx.mem.set16(D + 0xB8, 4);
            quads4::ring_into(fx, uv::uv_rect4, D, t, d, TABLE);
            a3 = 0;
        }
        if f(fx) == ORANGE {
            set_rect(fx, 0x344, sp + 0xA0, 0xA);
            fx.mem.set16(D + 0xB8, 6);
            b709::strip_ring(fx, 0x01E0_4890, t, d, TABLE);
            a3 = 0;
        }
        if f(fx) == STRIPS {
            set_rect(fx, 0x344, sp + 0xC0, 0xA);
            fx.mem.set16(D + 0xB8, 0xA);
            b709::strip_ring(fx, 0x01E0_4890, t, d, TABLE);
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == GREY {
            set_rect(fx, 0x336C, sp + 0xD0, 0x14);
            fx.mem.set16(D + 0xB8, 0xF);
            billboard::ring_t(fx, uv::uv_rect4, D, t, d, TABLE2);
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == DISC_B {
            set_rect(fx, 0x37C, sp + 0xE0, 0x9B);
            quads4::fan18(fx, D, t, d);
            a3 = 0;
        }
        if f(fx) == RING {
            set_rect(fx, 0x37C, sp + 0xF0, 0x9A);
            fx.mem.set16(D + 0xB8, 5);
            billboard::ring_strip3(fx, D, t, d, TABLE3);
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == DISC {
            quads4::fan18(fx, D, t, d);
            a3 = 0;
        }
    }
    if flashes != 0 {
        unit_flash(fx, flashes, a3);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xB8 => 1,
        0xF5 => 4,
        0xE1 => 2,
        0xC3..=0xDB => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E04340(D, step, 0): b709's spawn on this record's globals (colour
/// D+0x3384, kind D+0x33E2); frame 0x60 with the emitter's s0 = D / s1,
/// the entry's s2 = 0 / s3 = param (-1), s4 the runner's (0 [H]).
fn spawn_task(fx: &mut Fx, step: u32, s1: u64, ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0, u64::from(D)), (0x10, s1), (0x20, 0), (0x30, u64::MAX), (0x40, 0), (0x50, u64::from(ra))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x40 * k + 0x4AE) == 0) {
        Some(k) => {
            let t = D + 0x478 + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            for c in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x2D0, 0);
            let a = fx.mem.i16(KIND);
            fx.mem.set16(t + 0x30, a);
            fx.mem.write_u32(t + 0x20, 0);
            fx.mem.write_u32(t + 0x24, step);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// 0x1E01560(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter,
/// the followed points and the spawns (frame 0x60: the entry's s0 = D, s1
/// = 0, ra; its s0 = D, s1 the loops' counts).
fn emit(fx: &mut Fx) -> Option<u32> {
    let dd = u64::from(D);
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0, dd), (0x10, 0), (0x20, 0x01E0_3A7C)]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let mut s1 = 0u64;
    // a3 as the last call left it: 1 after 0x1F6830, 0 after a spawn.
    let a3 = std::cell::Cell::new(None);
    let node = |fx: &mut Fx, o: u32, out: u32, s1: u64, ra: u32| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + o);
        node_position_saving(fx, h, slot, out, dd, Some(s1), ra);
        a3.set(Some(1));
    };
    let spawn_task = |fx: &mut Fx, step: u32, s1: u64, ra: u32| {
        spawn_task(fx, step, s1, ra);
        a3.set(Some(0));
    };
    let gap = |fx: &mut Fx, to: u32, a: u32, b: u32| {
        for k in 0..3 {
            let v = ee::sub(fx.mem.f(D + a + 4 * k), fx.mem.f(D + b + 4 * k));
            fx.mem.setf(D + to + 4 * k, v);
        }
    };
    if c(fx) < 0xDC {
        node(fx, 0, D + 0x31B0, s1, 0x01E0_15B8);
    }
    if c(fx) < 0xDC {
        node(fx, 2, D + 0x320, s1, 0x01E0_15E0);
    }
    if c(fx) < 0xDC {
        gap(fx, 0x20, 0x320, 0x31B0);
    }
    if c(fx) < 0xDC {
        node(fx, 4, D + 0x31C0, s1, 0x01E0_1648);
    }
    if c(fx) < 0xDC {
        node(fx, 6, D + 0x330, s1, 0x01E0_1670);
    }
    if c(fx) < 0xDC {
        gap(fx, 0x30, 0x330, 0x31C0);
    }
    let z = fx.mem.f(D + 0x31B8);
    let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 5.0) } else { ee::add(z, 5.0) };
    fx.mem.setf(D + 0x31B8, z);
    for (below, to, from) in [(0xC2, 0x40u32, 0x31B0u32), (0xD4, 0x50, 0x31C0)] {
        if c(fx) < below {
            for k in 0..4 {
                let w = fx.mem.read_u32(D + from + 4 * k);
                fx.mem.write_u32(D + to + 4 * k, w);
            }
        }
    }
    // Pushed back along the facing by 3 a update.
    let push = |fx: &mut Fx, local: u32, ra: u32, z: u32| {
        let f = facing_of(fx, sp + local, ra);
        let cs = l2::sin_deg(fx, SINES, (f - 0xB4) as i16 as i32, 1);
        let v = ee::add(fx.mem.f(D + z), ee::mul(3.0, ee::neg(cs)));
        fx.mem.setf(D + z, v);
    };
    if c(fx) == 0xC4 {
        push(fx, 0x40, 0x01E0_17A8, 0x31B8);
    }
    if (0xBB..0xBF).contains(&c(fx)) {
        push(fx, 0x50, 0x01E0_1834, 0x31C8);
    }
    let at = |fx: &mut Fx, from: u32| {
        for k in 0..3 {
            let w = fx.mem.read_u32(from + 4 * k);
            fx.mem.write_u32(D + 0x2D0 + 4 * k, w);
        }
    };
    if c(fx) < 8 {
        fx.mem.set16(KIND, 0);
        spawn_task(fx, FLASH, s1, 0x01E0_1890);
    }
    if c(fx) == 1 {
        fx.mem.set16(KIND, 1);
        spawn_task(fx, FLASH, s1, 0x01E0_18B8);
    }
    // (update, point, strips' kind, return addresses: disc, grey rings,
    // strip rings, blue strips, the second disc).
    for (when, p, kind, ra) in [(0xD3, 0x31C0u32, 0, [0x01E0_18F4u32, 0x01E0_190C, 0x01E0_1934, 0x01E0_1960, 0x01E0_1984]), (0xC1, 0x31B0, 1, [0x01E0_19C0, 0x01E0_19D8, 0x01E0_1A00, 0x01E0_1A30, 0x01E0_1A54])] {
        if c(fx) == when {
            at(fx, D + p);
            spawn_task(fx, DISC, s1, ra[0]);
            for i in 0..3u64 {
                spawn_task(fx, GREY, i, ra[1]);
            }
            for i in 0..0xAu64 {
                spawn_task(fx, RING, i, ra[2]);
            }
            fx.mem.set16(KIND, kind);
            for i in 0..4u64 {
                spawn_task(fx, STRIPS, i, ra[3]);
            }
            s1 = 4;
            spawn_task(fx, DISC_B, s1, ra[4]);
        }
    }
    for (from, below, o, kind, ra) in [(0xB9, 0xC9, 2u32, 0, [0x01E0_1A88u32, 0x01E0_1AB8]), (0xC8, 0xDA, 6, 1, [0x01E0_1AEC, 0x01E0_1B20])] {
        if (from..below).contains(&c(fx)) {
            node(fx, o, sp + 0x30, s1, ra[0]);
            at(fx, sp + 0x30);
            fx.mem.set16(KIND, kind);
            spawn_task(fx, BLUE, s1, ra[1]);
        }
    }
    // (range, odd, node, point, kind, return addresses: the point, the
    // node, purple, orange, dome, camera).
    for (from, below, odd, o, p, kind, ra) in [
        (0xC0, 0xC9, 0, 2u32, 0x31F0u32, 0, [0x01E0_1B60u32, 0x01E0_1B98, 0x01E0_1BCC, 0x01E0_1C08, 0x01E0_1C30, 0x01E0_1C54]),
        (0xD2, 0xDA, 1, 6, 0x3200, 1, [0x01E0_1C94, 0x01E0_1CD0, 0x01E0_1D08, 0x01E0_1D44, 0x01E0_1D6C, 0x01E0_1D90]),
    ] {
        if !(from..below).contains(&c(fx)) {
            continue;
        }
        if c(fx) == from {
            node(fx, o, D + p, s1, ra[0]);
        }
        if c(fx) & 1 == odd {
            node(fx, o, sp + 0x30, s1, ra[1]);
            at(fx, sp + 0x30);
            fx.mem.set16(KIND, kind);
            spawn_task(fx, PURPLE, 0, ra[2]);
            at(fx, D + p);
            spawn_task(fx, ORANGE, 0, ra[3]);
            s1 = 1;
        }
        fx.mem.set16(KIND, kind);
        spawn_task(fx, DOME, s1, ra[4]);
        if c(fx) == from {
            spawn_task(fx, CAMERA, s1, ra[5]);
        }
    }
    for (when, o, ra) in [(&[0x82, 0x96][..], 0xAu32, [0x01E0_1DC4u32, 0x01E0_1DF4]), (&[0x79, 0x8C, 0xA0][..], 0xC, [0x01E0_1E48, 0x01E0_1E78])] {
        if when.contains(&c(fx)) {
            node(fx, o, sp + 0x30, s1, ra[0]);
            at(fx, sp + 0x30);
            for i in 0..0xAu64 {
                spawn_task(fx, SMOKE, i, ra[1]);
            }
            s1 = 0xA;
        }
    }
    fx.leave(0x60);
    a3.get()
}

fn fix(fx: &mut Fx, v: f32, ra: u32) -> i32 {
    fx.fix_residue(v, ra);
    ee::fixsfsi(v)
}

/// 0x1E02290 (D, task): a grey smoke sprite (size 60, cell 5, (0x80, 0x80,
/// 0x80), +0x30 / +0x3A rand % 360); 0x80 for 7 updates, then out by 15;
/// drifting by 16 sin / cos +0x3A in x / z.
fn smoke(fx: &mut Fx, t: u32) {
    let at = SMOKE;
    fx.enter(0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x3C);
        fx.mem.set16(t + 0x34, 5);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, at + 0x50, 0x168);
        fx.mem.set16(t + 0x30, v);
        let v = rmod(fx, t, at + 0x6C, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
    }
    b709::hold_fade_by(fx, t, 0x80, 7, 0xF);
    for (o, cos, ra) in [(8u32, 0, 0xFCu32), (0xC, 1, 0x134)] {
        let a = fx.mem.i16(t + 0x3A);
        let s = l2::sin_deg(fx, SINES, a, cos);
        let v = ee::add(ee::from_i32(fx.mem.i16(t + o)), ee::mul(16.0, s));
        let r = fix(fx, v, at + ra);
        fx.mem.set16(t + o, r);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x30);
}

/// 0x1E023F0 (D, task): a purple ring (size 25, cell 0..7, (0x40, 8,
/// 0x80), +0x3A the kind, +0xE 27..42, life 3..9); in by 0x38 for its life,
/// then out by 15; radius up by +0xE after the first update.
fn purple(fx: &mut Fx, t: u32) {
    let at = PURPLE;
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x19);
        let v = fx.rand_from(t, at + 0x28);
        fx.mem.set16(t + 0x34, v % 8);
        l2::rgb(fx, t + 0x28, 0x40, 8, 0x80);
        let k = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0x3A, k);
        fx.mem.set16(t + 0x30, 0);
        let v = fx.rand_from(t, at + 0x64);
        fx.mem.set16(t + 0xE, v % 16 + 0x1B);
        let v = rmod(fx, t, at + 0x84, 0xA);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, at + 0xA0, 7);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x71 {
            fx.mem.write_u8(t + 0x2B, a + 0x38);
        }
    } else {
        b709::fade_by(fx, t, 0xF);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x20);
}

/// 0x1E02CD0 (D, task): `task40::ring_strip` with +0x10 / +0x14 (rand >>
/// 4) % 0xFFFF (+0x18 left), radius rand % 150 + 350 and +0xC = 15.
fn ring(fx: &mut Fx, t: u32) {
    let at = RING;
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x10);
        let v = fx.rand_from(t, at + 0x28);
        fx.mem.set16(t + 0x34, v % 8);
        for (o, ra) in [(0x10u32, 0x44u32), (0x14, 0x64)] {
            let v = fx.rand_from(t, at + ra);
            let v = fx.div(v >> 4, 0xFFFF).1;
            fx.mem.set32(t + o, v);
        }
        l2::rgb(fx, t + 0x28, 0xA0, 0xA0, 0x10);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, at + 0xA8, 0x96);
        fx.mem.set16(t + 0x30, v + 0x15E);
        let v = rmod(fx, t, at + 0xC8, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rmod(fx, t, at + 0xE4, 0x12);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = rmod(fx, t, at + 0x104, 0xA);
        fx.mem.set16(t + 8, v);
        fx.mem.set16(t + 0xC, 0xF);
        fx.mem.set16(t + 6, 2);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < 0xB1 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x58));
        }
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x2B, a - 0xD);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x20);
}

/// 0x1E04F40(D): in a packet layer (0x2078A0) the attacker drawn into the
/// target (0x1F8270 filter 0; ALPHA 0x48), TEXFLUSH before and after; the
/// flash tasks stepped (0x1E033B0); the layer linked at key 0 when any
/// task was alive. Frame 0xB0 (the entry's s0 = D, s1 = the flashes, s2 =
/// the draw loop's live count, s3 = D + 0x40 x 180, ra) [H: the entry's s2
/// not tracked, written 0].
fn unit_flash(fx: &mut Fx, flashes: u32, a3: u32) {
    let sp = fx.enter(0xB0);
    fx.saved(sp, &[(0, u64::from(D)), (0x10, u64::from(flashes)), (0x20, 0), (0x30, u64::from(D + 0x40 * TASKS)), (0x40, 0x01E0_4288)]);
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
    for k in 0..TASKS {
        let t = D + 0x478 + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        live += 1;
        if fx.mem.read_u32(t + 0x24) == FLASH {
            flash(fx, t, [k, live]);
        }
    }
    if live != 0 {
        let layer = fx.mem.read_u32(LAYER);
        fx.layer_end(layer, OT, 0);
    }
    fx.leave(0xB0);
}

/// 0x1E033B0(D, t): the flash (the window, UVs inset by +8 / 10 and +0xA /
/// 10 over 512, into the descriptor at D+0x3398, no ordering table): kind
/// 0 grey (0x30, 0x30, 0x30), alpha up by 9 below 0x80 to age 13, then
/// from age 41 down by 7, its insets growing by 30 from age 36; kind 1
/// pink (0xC0, 0x80, 0x80), alpha up by 0xD below 0xC0 to update 0xDB,
/// then from age 41 down by 7. `regs` the unit flash's s0 / s1 (the slot
/// index, the live count).
fn flash(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_5014), (0x20, u64::from(regs[1])), (0x10, u64::from(regs[0]))]);
    // swc1 f20 / f21: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 4, 0);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let h = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.write_u8(t + 0x2B, 0);
        if fx.mem.i16(t + 0x30) == 0 {
            l2::rgb(fx, t + 0x28, 0x30, 0x30, 0x30);
        }
        if fx.mem.i16(t + 0x30) == 1 {
            l2::rgb(fx, t + 0x28, 0xC0, 0x80, 0x80);
        }
        fx.mem.set16(t + 0x36, 2);
    }
    let d = fx.mem.read_u32(FLASH_DESC);
    for k in 0..4u32 {
        let b = fx.mem.read_u8(t + 0x28 + k);
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
    let fade7 = |fx: &mut Fx| {
        if fx.mem.i16(t + 0x32) >= 0x29 {
            let a = fx.mem.read_u8(t + 0x2B);
            if a >= 9 {
                fx.mem.write_u8(t + 0x2B, a - 7);
            } else {
                fx.mem.set16(t + 0x36, 0);
            }
        }
    };
    if fx.mem.i16(t + 0x30) == 0 {
        if fx.mem.i16(t + 0x32) >= 0x24 {
            for o in [8u32, 0xA] {
                let v = fx.mem.i16(t + o) + 0x1E;
                fx.mem.set16(t + o, v);
            }
        }
        if fx.mem.i16(t + 0x32) < 0xE {
            let a = fx.mem.read_u8(t + 0x2B);
            if a < 0x80 {
                fx.mem.write_u8(t + 0x2B, a + 9);
            }
        } else {
            fade7(fx);
        }
    }
    if fx.mem.i16(t + 0x30) == 1 {
        if fx.mem.i16(COUNTER) < 0xDC {
            let a = fx.mem.read_u8(t + 0x2B);
            if a < 0xC0 {
                fx.mem.write_u8(t + 0x2B, a + 0xD);
            }
        } else {
            fade7(fx);
        }
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x40);
}
