//! BMODULE b148 (card 312), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04360, data area 0x1ED0000, 0x3BD0 bytes) [F]
//!: b124's older pool - 200 tasks of 0x40 bytes
//! at D+0x2F0 (alive +0x36, age +0x32, step +0x24, each step called by its
//! own `jal`; spawned by 0x1E05120 at the point D+0x250 with the colour
//! D+0x3614 and the node index D+0x3656) - on a 0x7C-update timer
//! D+0x3610. On update 0x40 at the attacker's node 0x37: eight blue
//! spinning rings (0x1E037A0, `billboard::ring_t20`), eight grey quad
//! rings (0x1E034C0, `quads4::ring_spun`), a blue band ring (0x1E03630,
//! `quads4::ring_band_task`), ten domes (0x1E03930, 0x1E05240: a 12 x 1
//! textured band on the second sine table), two discs
//! (`spawn::disc_step` eec8ab51b79f, `quads4::fan18`), two camera tasks
//! (`task40::camera`) and ten blue motes (0x1E02F40, stretched quad rings
//! `quads4::ring_spun_axis`); updates 0x40..0x42 three spark fans a
//! update (0x1E030C0, 0x1E02530: 9 textured wedges). Returns 1 on update
//! 0x36, 4 on 0x4A, 2 on 0x40, 8 once the timer has run out.
//!
//! The steps 0x1E032D0, 0x1E03AA0, 0x1E03C20 (`task40::mote_target`),
//! 0x1E03E50 (`task40::mote_node`), 0x1E041A0 (the stage light) and the
//! object op 0x1E04260 are tested by the entry but never spawned [F: the
//! emitter's spawns name 0x1E037A0 / 0x1E034C0 / 0x1E03630 / 0x1E03930 /
//! 0x1E033E0 / 0x1E04020 / 0x1E02F40 / 0x1E030C0 only]; not ported.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::blib::{billboard, quads4, spawn, task40, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 200;
const POOL: u32 = D + 0x2F0;
const POS: u32 = D + 0x250;
const MOTE: u32 = 0x01E0_2F40;
const SPARK: u32 = 0x01E0_30C0;
const DISC: u32 = 0x01E0_33E0;
const RING: u32 = 0x01E0_34C0;
const BAND: u32 = 0x01E0_3630;
const RING_T: u32 = 0x01E0_37A0;
const DOME: u32 = 0x01E0_3930;
const CAMERA: u32 = 0x01E0_4020;
const TIMER: u32 = D + 0x3610;
const COLOUR: u32 = D + 0x3614;
const NODES: u32 = D + 0x363C;
const KIND: u32 = D + 0x3656;
const COUNTER: u32 = D + 0x365A;
const TABLE: u32 = D + 0x365C;
const SINES_B: u32 = D + 0x3A5C;
const SPR: u32 = 0x7000_0000;
const SPRITE: u32 = 0x01E0_32D0;
const RING_B: u32 = 0x01E0_3AA0;
const MOTE_T: u32 = 0x01E0_3C20;
const MOTE_N: u32 = 0x01E0_3E50;

/// One record of the family: b148 (card 312) or its twin b417 (card 519,
/// the entry 0x450 later, every step 0x450 later, colours changed).
struct Rec {
    /// Added to b148's entry addresses (return addresses).
    entry: u32,
    /// Added to b148's step addresses.
    steps: u32,
    timer: i32,
    node0: i32,
    /// Textures of group 5 loaded (D+0x35F0..).
    tex5: u32,
    /// `spawn::disc_step` constants (`None`: the base step).
    disc: Option<&'static str>,
    mote: (i32, i32, i32),
    /// Spark: size, first colour for kind 0 / else, then by age % 3 (0, 1, 2).
    spark: (i32, [(i32, i32, i32); 5]),
    /// Quad ring: size, colour, growth base; its draw texture / atlas id.
    ring: (i32, (i32, i32, i32), i32, u32, i32),
    ring_t: (i32, i32, i32),
    /// Band: first, odd, even colours.
    band: [(i32, i32, i32); 3],
    dome: (i32, i32, i32),
    emit: fn(&mut Fx, &Rec, i32),
    ret: fn(i32) -> Option<i32>,
}

const B148: Rec = Rec {
    entry: 0,
    steps: 0,
    timer: 0x7C,
    node0: 0x37,
    tex5: 3,
    disc: Some("eec8ab51b79f"),
    mote: (0x20, 0x20, 0x60),
    spark: (0x1E, [(0x40, 0x40, 0x80), (0x80, 0x80, 0x80), (0x20, 0x20, 0x80), (0x40, 0x40, 0x80), (0x80, 0x80, 0x80)]),
    ring: (0x32, (0x60, 0x60, 0x60), 0x14, D + 0x35F8, 0x14),
    ring_t: (0x10, 0x10, 0x80),
    band: [(0x10, 0x10, 0xF0), (0x20, 0x20, 0xC0), (0x50, 0x50, 0x80)],
    dome: (0x10, 0x10, 0x20),
    emit,
    ret: |c| match c {
        0x36 => Some(1),
        0x4A => Some(4),
        0x40 => Some(2),
        _ => None,
    },
};

const B417: Rec = Rec {
    entry: 0x450,
    steps: 0x450,
    timer: 0x64,
    node0: 0xE,
    tex5: 2,
    disc: None,
    mote: (0x50, 0x50, 0x50),
    spark: (0xA, [(0x80, 0x40, 0), (0, 0x80, 0x80), (0x10, 0x10, 0x80), (0x80, 0x40, 0), (0x80, 0x80, 0)]),
    ring: (0x14, (0x80, 0x40, 0), 3, D + 0x2EC, 0x9B),
    ring_t: (0x80, 0x40, 0),
    band: [(0xF0, 0x10, 0x20), (0x80, 0x80, 0x10), (0x88, 0xF0, 8)],
    dome: (0x3F, 0x10, 8),
    emit: emit_b417,
    ret: |c| match c {
        0x25 => Some(1),
        0x5A => Some(4),
        0x45 => Some(2),
        0x29..=0x3F => Some(6),
        _ => None,
    },
};

pub(crate) fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E04360 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    program_by(fx, &B148, param)
}

/// BMODULE b417 (card 519): b148's twin (entry 0x1E047B0; see [`Rec`]).
pub fn program_b417(fx: &mut Fx, param: i32) -> i32 {
    program_by(fx, &B417, param)
}

fn program_by(fx: &mut Fx, r: &Rec, param: i32) -> i32 {
    let sp = fx.enter(0x1D0);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x361C, OT);
    let ret = if param >= 0 { init(fx, r, sp, param) } else { step(fx, r, sp, param) };
    fx.leave(0x1D0);
    ret
}

pub(crate) fn facing_of(fx: &mut Fx, out: u32, ra: u32) -> i32 {
    let h = fx.self_handle();
    fx.unit_rotation(h, out);
    let y = ee::div(ee::from_i32(fx.mem.i32(out + 4)), 180.0);
    fx.fix_residue(y, ra);
    ee::fixsfsi(y)
}

fn init(fx: &mut Fx, r: &Rec, sp: u32, param: i32) -> i32 {
    let e = r.entry;
    fx.bzero(D, 0x3BD0);
    l2::sine_table(fx, SINES_B);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x3658, param);
    fx.mem.set32(TIMER, r.timer);
    for (k, n) in [r.node0, 0xD, 0x36, 0x37, 0x1F, 0x34, 0x36, 0x37].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    // 0x1E042B0(0, D): the textures (frame 0x40: ra, the entry's s2 =
    // param, s1 = 0, s0 = D).
    let f = fx.enter(0x40);
    fx.saved(f, &[(0x30, u64::from(0x01E0_448C + e)), (0x20, param as i64 as u64), (0x10, 0), (0, u64::from(D))]);
    for k in 0..3 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x2B0 + 4 * k as u32, t);
    }
    for k in 0xD..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x2E4 + 4 * (k as u32 - 0xD), t);
    }
    for k in 0..r.tex5 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x35F0 + 4 * k as u32, t);
    }
    fx.leave(0x40);
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_4494 + e, 0x1E);
        fx.mem.set8(TABLE + k, v - 0xF);
    }
    let b = TABLE + 0x100;
    for k in 0..0x64u32 {
        fx.mem.write_u8(b + k, 0);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, D, 0x01E0_44E8 + e, 5);
        fx.mem.set8(b + k, v);
    }
    let f = facing_of(fx, sp + 0x100, 0x01E0_4544 + e);
    for k in 0xB4..0x100u32 {
        let (ra, sign) = if f != 0 { (0x01E0_455C + e, 1) } else { (0x01E0_4584 + e, -1) };
        let v = fx.rand_from(D, ra);
        fx.mem.set8(b + k, sign * (v % 4));
    }
    let b = TABLE + 0x200;
    for k in 0..0x64u32 {
        fx.mem.write_u8(b + k, 0);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, D, 0x01E0_45E4 + e, 6);
        fx.mem.set8(b + k, v);
    }
    let f = facing_of(fx, sp + 0x110, 0x01E0_4640 + e);
    for k in 0xB4..0x100u32 {
        let (ra, sign) = if f != 0 { (0x01E0_4658 + e, 1) } else { (0x01E0_4684 + e, -1) };
        let v = rmod(fx, D, ra, 0x14);
        fx.mem.set8(b + k, sign * (v + 5));
    }
    let b = TABLE + 0x300;
    for k in 0..0x64u32 {
        fx.mem.write_u8(b + k, 0);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, D, 0x01E0_46E4 + e, 3);
        fx.mem.set8(b + k, v);
    }
    for k in 0xB4..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_4710 + e);
        fx.mem.set8(b + k, v % 2 - 1);
        let f = facing_of(fx, sp + 0x120, 0x01E0_476C + e);
        let c = fx.mem.i8(b + k);
        fx.mem.set8(b + k, if f != 0 { c + 2 } else { c - 2 });
    }
    0
}

fn step(fx: &mut Fx, r: &Rec, sp: u32, param: i32) -> i32 {
    let e = r.entry;
    (r.emit)(fx, r, param);
    let low = fx.sp;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x24).wrapping_sub(r.steps);
        // (frame, ra after the step's jal, the step's slot of the entry's s1).
        let (frame, ra, s1): (u32, u32, Option<u64>) = match f {
            MOTE => {
                mote(fx, r, t);
                (0x20, 0x01E0_47E8 + e, None)
            }
            SPARK => {
                spark(fx, r, t);
                (0x20, 0x01E0_4808 + e, None)
            }
            DISC => {
                match r.disc {
                    Some(h) => {
                        let c = spawn::disc(h).expect("constants");
                        spawn::disc_step_with(fx, c, DISC + r.steps, t);
                    }
                    None => {
                        spawn::disc_step(fx, DISC + r.steps, t);
                    }
                }
                (0x20, 0x01E0_4848 + e, None)
            }
            RING => {
                ring(fx, r, t);
                (0x20, 0x01E0_4868 + e, None)
            }
            BAND => {
                band(fx, r, t);
                (0x20, 0x01E0_4888 + e, None)
            }
            RING_T => {
                ring_t(fx, r, t);
                (0x20, 0x01E0_48A8 + e, None)
            }
            DOME => {
                dome_step(fx, r, t);
                (0x30, 0x01E0_48C8 + e, None)
            }
            CAMERA => {
                task40::camera(fx, CAMERA + r.steps, D, t);
                (0x30, 0x01E0_4948 + e, Some(0u64))
            }
            SPRITE => {
                sprite(fx, r, t);
                (0x20, 0x01E0_4828 + e, None)
            }
            RING_B => {
                ring_b(fx, r, t);
                (0x20, 0x01E0_48E8 + e, None)
            }
            MOTE_T => {
                task40::mote_target(fx, MOTE_T + r.steps, D, t);
                (0x60, 0x01E0_4908 + e, Some(0u64))
            }
            MOTE_N => {
                task40::mote_node(fx, MOTE_N + r.steps, D, t);
                (0x40, 0x01E0_4928 + e, Some(0u64))
            }
            _ => {
                debug_assert!(false, "b148: unported task step {f:#x}");
                continue;
            }
        };
        let base = low - frame;
        match s1 {
            Some(v) => fx.saved(base, &[(0x20, u64::from(ra)), (0x10, v), (0, u64::from(D))]),
            None => fx.saved(base, &[(0x10, u64::from(ra)), (0, u64::from(D))]),
        }
    }
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x50, u64::from(D), 0x01E0_49DC + e);
    let d = sp + 0x138;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let set_rect = |fx: &mut Fx, tex: u32, local: u32, id: i32| {
            let w = fx.mem.read_u32(tex);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x34);
            l2::rect(fx, local, id, sub);
            for k in 0..4 {
                let v = fx.mem.read_u32(local + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, v);
            }
        };
        let at_task = |fx: &mut Fx| {
            for k in 0..3 {
                let v = ee::from_i32(fx.mem.i16(t + 2 * k));
                fx.mem.setf(D + 0x20 + 4 * k, v);
            }
        };
        let half_h = |fx: &mut Fx| {
            let h = fx.mem.i16(D + 0x9A) >> 1;
            fx.mem.set16(D + 0x9A, h);
        };
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x24).wrapping_sub(r.steps);
        if f(fx) == MOTE {
            set_rect(fx, D + 0x35F4, sp + 0x60, 0x14);
            half_h(fx);
            fx.mem.set16(D + 0xB8, 0xA);
            fx.mem.set16(D + 0xBE, 0);
            fx.mem.write_u32(D + 0xA4, 0);
            at_task(fx);
            quads4::ring_spun_axis(fx, uv::uv_rect, D, t, 0, TABLE);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == SPARK {
            set_rect(fx, D + 0x2EC, sp + 0x70, 0x97);
            fx.mem.set16(D + 0xB8, 0);
            fan9(fx, t, d);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == SPRITE {
            set_rect(fx, D + 0x35F0, sp + 0x80, 0x14);
            for k in 0..3 {
                let v = ee::from_i32(fx.mem.i16(t + 2 * k));
                fx.mem.setf(sp + 0x50 + 4 * k, v);
            }
            billboard::sprite_square(fx, 0x01E0_1350, uv::uv_rect, D, t, 0, sp + 0x50);
        }
        if f(fx) == DISC {
            set_rect(fx, D + 0x2EC, sp + 0x90, 0x9B);
            quads4::fan18(fx, D, t, d);
        }
        if f(fx) == RING {
            set_rect(fx, r.ring.3, sp + 0xA0, r.ring.4);
            fx.mem.set16(D + 0xB8, 0xA);
            at_task(fx);
            quads4::ring_spun(fx, uv::uv_rect, D, t, 0, TABLE);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == BAND {
            set_rect(fx, D + 0x2EC, sp + 0xB0, 0x97);
            fx.mem.set16(D + 0x94, 0xA0);
            half_h(fx);
            fx.mem.set16(D + 0xB8, 0xA);
            quads4::ring_band_task(fx, 0x01E0_2790, D, t, d);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == RING_T {
            set_rect(fx, D + 0x2B4, sp + 0xC0, 0xA);
            billboard::ring_t20(fx, uv::uv_rect, D, t, d, TABLE);
        }
        if f(fx) == DOME {
            dome(fx, t, d, false, DOME_12);
            dome(fx, t, d, true, DOME_12);
        }
        if f(fx) == RING_B {
            set_rect(fx, D + 0x35F0, sp + 0xD0, 0x14);
            fx.mem.set16(D + 0xB8, 3);
            let v = fx.rand_from(D, 0x01E0_4E78 + e);
            fx.mem.set16(D + 0xBE, v & 7);
            fx.mem.write_u32(D + 0xA4, 0x3F80_0000);
            at_task(fx);
            quads4::ring_spun_blend(fx, uv::uv_rect, D, t, 1, TABLE + 0x100);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == MOTE_T {
            set_rect(fx, D + 0x35F4, sp + 0xE0, 0x14);
            fx.mem.set16(D + 0xB8, 5);
            let v = fx.rand_from(D, 0x01E0_4F48 + e);
            fx.mem.set16(D + 0xBE, v & 7);
            fx.mem.write_u32(D + 0xA4, 0);
            at_task(fx);
            quads4::ring_spun_axis_z(fx, uv::uv_rect, D, t, 0, TABLE + 0x200);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == MOTE_N {
            set_rect(fx, D + 0x35F4, sp + 0xF0, 0x14);
            half_h(fx);
            fx.mem.set16(D + 0xB8, 0xA);
            fx.mem.set16(D + 0xBE, 0);
            fx.mem.write_u32(D + 0xA4, 0);
            at_task(fx);
            quads4::ring_spun_axis_z(fx, uv::uv_rect, D, t, 0, TABLE + 0x300);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match (r.ret)(fx.mem.i16(COUNTER)) {
        Some(v) => v,
        None if fx.mem.i32(TIMER) > 0 => 0,
        None => 8,
    }
}

/// 0x1E05120(D, step, 0): the first free task cleared and set off at
/// D+0x250 (i16 x / y / z / w) with the colour D+0x3614 (+0x28 / +0x2C),
/// kind D+0x3656 (+0x30); `s1` the emitter's s1 (the caller's s0 = D),
/// s2 the entry's (param), s3 / s4 the runner's (0 [H]); `ra` the return
/// address.
fn spawn(fx: &mut Fx, r: &Rec, step: u32, s1: u64, s2: u64, ra: u32) {
    spawn_task(fx, step + r.steps, [u64::from(D), s1, s2, 0, 0], ra);
}

/// [`spawn`] of the step at `step` with the caller's s0..s4 = `s` (b046 /
/// b527 share the spawn and its globals).
pub(crate) fn spawn_task(fx: &mut Fx, step: u32, s: [u64; 5], ra: u32) {
    spawn_task_on(fx, TASKS, COLOUR, KIND, step, s, ra);
}

/// [`spawn_task`] on a pool of `tasks` with the colour / kind globals
/// `colour` / `kind` (b500, card 417: 200, D+0x3BB4 / D+0x3BF6; b476,
/// card 412: 150, D+0x2DF4 / D+0x2E36; the same code otherwise).
pub(crate) fn spawn_task_on(fx: &mut Fx, tasks: u32, colour: u32, kind: u32, step: u32, s: [u64; 5], ra: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, s[4]), (0x30, s[3]), (0x20, s[2]), (0x10, s[1]), (0, s[0])]);
    if let Some(k) = (0..tasks).find(|&k| fx.mem.i16(POOL + 0x36 + 0x40 * k) == 0) {
        let t = POOL + 0x40 * k;
        fx.bzero(t, 0x40);
        fx.mem.set16(t + 0x36, 1);
        for o in [0x28u32, 0x2C] {
            let (r, g, b) = (fx.mem.read_u8(colour), fx.mem.read_u8(colour + 1), fx.mem.read_u8(colour + 2));
            l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
            let a = fx.mem.read_u8(colour + 3);
            fx.mem.write_u8(t + o + 3, a);
        }
        l2::to_i16x4(fx, t, POS, 0);
        let s = fx.mem.i16(kind);
        fx.mem.set16(t + 0x30, s);
        fx.mem.write_u32(t + 0x20, 0);
        fx.mem.write_u32(t + 0x24, step);
    }
    fx.leave(0x60);
}

/// 0x1E02CD0(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the spawns (frame 0x40: ra, the entry's s1 = 0, s0 = D).
fn emit(fx: &mut Fx, r: &Rec, param: i32) {
    let p = param as i64 as u64;
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x20, 0x01E0_47B4), (0x10, 0), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let node = |fx: &mut Fx, s1: u32, ra: u32| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        node_position_saving(fx, h, slot, sp + 0x30, u64::from(D), Some(u64::from(s1)), ra);
        for k in 0..3 {
            let w = fx.mem.read_u32(sp + 0x30 + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
    };
    // The emitter's s1 (0 from the entry, then each loop's count).
    let mut s1 = 0u32;
    if fx.mem.i16(COUNTER) == 0x40 {
        node(fx, s1, 0x01E0_2D2C);
        fx.mem.set16(KIND, 1);
        for (f, n, ra) in [(RING_T, 8u32, 0x01E0_2D64u32), (RING, 8, 0x01E0_2D8C), (BAND, 1, 0x01E0_2DB4), (DOME, 0xA, 0x01E0_2DD8)] {
            for i in 0..n {
                spawn(fx, r, f, u64::from(i), p, ra);
            }
            s1 = n;
        }
        spawn(fx, r, DISC, u64::from(s1), p, 0x01E0_2DFC);
        spawn(fx, r, CAMERA, u64::from(s1), p, 0x01E0_2E10);
    }
    if fx.mem.i16(COUNTER) == 0x40 {
        node(fx, s1, 0x01E0_2E38);
        fx.mem.set16(KIND, 1);
        spawn(fx, r, DISC, u64::from(s1), p, 0x01E0_2E6C);
        for i in 0..0xAu32 {
            spawn(fx, r, MOTE, u64::from(i), p, 0x01E0_2E84);
        }
        s1 = 0xA;
        spawn(fx, r, CAMERA, u64::from(s1), p, 0x01E0_2EA8);
    }
    if (0x40..=0x42).contains(&fx.mem.i16(COUNTER)) {
        node(fx, s1, 0x01E0_2EE8);
        for i in 0..3u32 {
            spawn(fx, r, SPARK, u64::from(i), p, 0x01E0_2F18);
        }
    }
    fx.leave(0x40);
}

/// b417's emitter 0x1E02CD0(D, 0) (frame 0x60: ra, the entry's s2 =
/// param, s1 = 0, s0 = D): node 0xE kept at D+0x3530 on update 0x33 (and
/// read again to 0x40); to 0x88 a grey sprite at each of nodes 0x37 / 0xE;
/// 0x17..0x24 a target-pulled mote ring (`task40::mote_target`, 15 lower)
/// at nodes 0xD / 0x36, kinds 0 / 1; 0x17..0x2D a turned ring (0x1E03EF0,
/// 10 lower) at each; 5..7 node motes (`task40::mote_node`, 15 lower) at
/// nodes 0x37 / 0xE; 0x1A / 0x2F the spawn point moved; 0x2F at node 0x1F
/// b148's update-0x40 set, 0x44 its second, 0x44..0x46 the spark fans.
fn emit_b417(fx: &mut Fx, r: &Rec, param: i32) {
    let sp = fx.enter(0x60);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, 0x01E0_4C04), (0x20, p), (0x10, 0), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    // node k of the table into `out` (0x1F6830 with the emitter's s1).
    let node = |fx: &mut Fx, k: u32, out: u32, s1: u64, ra: u32| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2 * k);
        node_position_saving(fx, h, slot, out, u64::from(D), Some(s1), ra);
    };
    let pos = |fx: &mut Fx, dy: f32| {
        for k in 0..3 {
            let w = fx.mem.read_u32(sp + 0x40 + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
        if dy != 0.0 {
            let y = ee::sub(fx.mem.f(sp + 0x44), dy);
            fx.mem.setf(POS + 4, y);
        }
    };
    let (q1, q2) = (u64::from(sp + 0x44), u64::from(sp + 0x48));
    let mut s1 = 0u64;
    let mut s2 = p;
    if c(fx) == 0x33 {
        node(fx, 1, D + 0x3530, s1, 0x01E0_2D30);
    }
    if (0x34..0x41).contains(&c(fx)) {
        node(fx, 1, sp + 0x40, s1, 0x01E0_2D64);
        for k in 0..3 {
            let w = fx.mem.read_u32(D + 0x3530 + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
    }
    if c(fx) < 0x89 {
        node(fx, 0, sp + 0x40, s1, 0x01E0_2DB8);
        (s1, s2) = (q1, q2);
        pos(fx, 0.0);
        spawn(fx, r, SPRITE, s1, s2, 0x01E0_2DEC);
        node(fx, 1, sp + 0x40, s1, 0x01E0_2E04);
        pos(fx, 0.0);
        spawn(fx, r, SPRITE, s1, s2, 0x01E0_2E30);
    }
    if (0x17..0x25).contains(&c(fx)) {
        node(fx, 2, sp + 0x40, s1, 0x01E0_2E64);
        (s1, s2) = (q1, q2);
        pos(fx, 15.0);
        fx.mem.set16(KIND, 0);
        spawn(fx, r, MOTE_T, s1, s2, 0x01E0_2EA8);
        node(fx, 3, sp + 0x40, s1, 0x01E0_2EC0);
        pos(fx, 15.0);
        fx.mem.set16(KIND, 1);
        spawn(fx, r, MOTE_T, s1, s2, 0x01E0_2F00);
    }
    if (0x17..0x2E).contains(&c(fx)) {
        node(fx, 2, sp + 0x40, s1, 0x01E0_2F34);
        (s1, s2) = (q1, q2);
        pos(fx, 10.0);
        spawn(fx, r, RING_B, s1, s2, 0x01E0_2F74);
        node(fx, 3, sp + 0x40, s1, 0x01E0_2F8C);
        pos(fx, 10.0);
        spawn(fx, r, RING_B, s1, s2, 0x01E0_2FC4);
    }
    if (5..8).contains(&c(fx)) {
        node(fx, 0, sp + 0x40, s1, 0x01E0_2FF8);
        (s1, s2) = (q1, q2);
        pos(fx, 15.0);
        fx.mem.set16(KIND, 0);
        spawn(fx, r, MOTE_N, s1, s2, 0x01E0_303C);
        node(fx, 1, sp + 0x40, s1, 0x01E0_3054);
        pos(fx, 15.0);
        fx.mem.set16(KIND, 1);
        spawn(fx, r, MOTE_N, s1, s2, 0x01E0_3094);
    }
    if c(fx) == 0x1A {
        node(fx, 2, sp + 0x40, s1, 0x01E0_30BC);
        pos(fx, 0.0);
        fx.mem.set16(KIND, 1);
        let y = ee::add(1.0, fx.mem.f(sp + 0x44));
        fx.mem.setf(POS + 4, y);
    }
    if c(fx) == 0x2F {
        node(fx, 0, sp + 0x40, s1, 0x01E0_311C);
        let h = fx.target_handle();
        unit_position_saving(fx, h, sp + 0x50, u64::from(D), 0x01E0_3130);
        pos(fx, 0.0);
        fx.mem.set16(KIND, 1);
    }
    if c(fx) == 0x2F {
        node(fx, 4, sp + 0x40, s1, 0x01E0_3178);
        pos(fx, 0.0);
        fx.mem.set16(KIND, 1);
        for (f, n, ra) in [(RING_T, 8u32, 0x01E0_31B0u32), (RING, 8, 0x01E0_31D8), (BAND, 1, 0x01E0_3200), (DOME, 0xA, 0x01E0_3224)] {
            for i in 0..n {
                spawn(fx, r, f, u64::from(i), s2, ra);
            }
            s1 = u64::from(n);
        }
        spawn(fx, r, DISC, s1, s2, 0x01E0_3248);
        spawn(fx, r, CAMERA, s1, s2, 0x01E0_325C);
    }
    if c(fx) == 0x44 {
        node(fx, 4, sp + 0x40, s1, 0x01E0_3284);
        pos(fx, 0.0);
        fx.mem.set16(KIND, 1);
        spawn(fx, r, DISC, s1, s2, 0x01E0_32B8);
        for i in 0..0xAu32 {
            spawn(fx, r, MOTE, u64::from(i), s2, 0x01E0_32D0);
        }
        s1 = 0xA;
        spawn(fx, r, CAMERA, s1, s2, 0x01E0_32F4);
    }
    if (0x44..=0x46).contains(&c(fx)) {
        node(fx, 4, sp + 0x40, s1, 0x01E0_3334);
        pos(fx, 0.0);
        for i in 0..3u32 {
            spawn(fx, r, SPARK, u64::from(i), s2, 0x01E0_3364);
        }
    }
    fx.leave(0x60);
}

pub(crate) fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// The steps' shared fade: alpha `held` (or + 0x40 while at most 0x80
/// when `None`) during the life +6, then down by 0xF (the task over below
/// 0x11).
pub(crate) fn fade(fx: &mut Fx, t: u32, held: Option<u8>) {
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        match held {
            Some(v) => fx.mem.write_u8(t + 0x2B, v),
            None if a < 0x81 => fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x40)),
            None => {}
        }
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x2B, a - 0xF);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// 0x1E02F40 (D, task): a blue mote ring (size 0x36, (0x20, 0x20, 0x60),
/// node +0xA = +0x30, radius from rand % 100 growing by 80..109, row
/// 0..19, life 1..2; alpha in by 0x40, out by 0xF).
fn mote(fx: &mut Fx, r: &Rec, t: u32) {
    let at = MOTE + r.steps;
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x36);
        fx.mem.set16(t + 0x34, 1);
        let v = rmod(fx, t, at + 0x2C, 0xFFFF);
        fx.mem.set32(t + 0x18, v);
        l2::rgb(fx, t + 0x28, r.mote.0, r.mote.1, r.mote.2);
        let n = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, n);
        let v = rmod(fx, t, at + 0x5C, 0x64);
        fx.mem.set16(t + 0x30, v);
        let v = rmod(fx, t, at + 0x78, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rmod(fx, t, at + 0x94, 0x1E);
        fx.mem.set16(t + 0xE, v + 0x50);
        let v = rmod(fx, t, at + 0xB4, 0x14);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, at + 0xD0);
        fx.mem.set16(t + 6, v % 2 + 1);
        fx.mem.set16(t + 0x36, 2);
    }
    fade(fx, t, None);
    if fx.mem.i16(t + 0x32) != 0 {
        let v = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, v);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E030C0 (D, task): a spark fan (size 30 growing by 2..11, random
/// angles 0x2000 + / -0x2000 + rand % 0x4000, cell 0..6; blue-white
/// (0x40, 0x40, 0x80) for kind 0, else white; then by age % 3 blue /
/// white / dark blue; in by 0x40 for 2 updates, out by 9).
fn spark(fx: &mut Fx, r: &Rec, t: u32) {
    let at = SPARK + r.steps;
    let c = r.spark.1;
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, r.spark.0);
        for (o, ra, b) in [(0x10u32, 0x28u32, 0x2000), (0x14, 0x48, -0x2000), (0x18, 0x68, -0x2000)] {
            let v = fx.rand_from(t, at + ra);
            fx.mem.set32(t + o, v % 0x4000 + b);
        }
        let v = rmod(fx, t, at + 0x88, 7);
        fx.mem.set16(t + 0x34, v);
        fx.mem.set16(t + 0x36, 2);
        let v = rmod(fx, t, at + 0xAC, 0xA);
        fx.mem.set16(t + 0xE, v + 2);
        let k = if fx.mem.i16(t + 0x30) == 0 { c[0] } else { c[1] };
        l2::rgb(fx, t + 0x28, k.0, k.1, k.2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < 2 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x2B, a + 0x40);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x2B, a - 9);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    let g = fx.mem.i16(t + 0x32);
    let age3 = fx.div(g, 3).1;
    let k = match age3 {
        1 => c[3],
        2 => c[4],
        _ => c[2],
    };
    l2::rgb(fx, t + 0x28, k.0, k.1, k.2);
    if fx.mem.i16(t + 0x32) != 0 {
        let v = fx.mem.i16(t + 0x38) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x38, v);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E034C0 (D, task): a grey quad ring (size 50, cell 0..3, angle x
/// 0..359, radius from 0 growing by 20..35, row 0..9, life 5..14; 0x80
/// during the life, out by 0xF).
fn ring(fx: &mut Fx, r: &Rec, t: u32) {
    let at = RING + r.steps;
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, r.ring.0);
        let v = fx.rand_from(t, at + 0x28);
        fx.mem.set16(t + 0x34, v % 4);
        let k = r.ring.1;
        l2::rgb(fx, t + 0x28, k.0, k.1, k.2);
        fx.mem.set32(t + 0x18, 0);
        let v = rmod(fx, t, at + 0x5C, 0x168);
        fx.mem.set32(t + 0x10, v);
        fx.mem.set16(t + 0x30, 0);
        let v = rmod(fx, t, at + 0x7C, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = fx.rand_from(t, at + 0x98);
        fx.mem.set16(t + 0xE, v % 0x10 + r.ring.2);
        let v = rmod(fx, t, at + 0xB8, 0xA);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, at + 0xD4, 0xA);
        fx.mem.set16(t + 6, v + 5);
        fx.mem.set16(t + 0x36, 2);
    }
    fade(fx, t, Some(0x80));
    let v = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x30, v);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E037A0 (D, task): a blue spinning ring (size 2, cell 0..3, random
/// angles z / x, radius from 0 growing by 3..18, row 0..9, life 5..14;
/// 0x80 during the life, out by 0xF).
fn ring_t(fx: &mut Fx, r: &Rec, t: u32) {
    let at = RING_T + r.steps;
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 2);
        let v = fx.rand_from(t, at + 0x28);
        fx.mem.set16(t + 0x34, v % 4);
        l2::rgb(fx, t + 0x28, r.ring_t.0, r.ring_t.1, r.ring_t.2);
        let v = rmod(fx, t, at + 0x5C, 0xFFFF);
        fx.mem.set32(t + 0x18, v);
        let v = rmod(fx, t, at + 0x78, 0xFFFF);
        fx.mem.set32(t + 0x10, v);
        fx.mem.set16(t + 0x30, 0);
        let v = rmod(fx, t, at + 0x98, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = fx.rand_from(t, at + 0xB4);
        fx.mem.set16(t + 0xE, v % 0x10 + 3);
        let v = rmod(fx, t, at + 0xD4, 0xA);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, at + 0xF0, 0xA);
        fx.mem.set16(t + 6, v + 5);
        fx.mem.set16(t + 0x36, 2);
    }
    fade(fx, t, Some(0x80));
    let v = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x30, v);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03630 (D, task): the blue band ring (angle x 0x4000, radius from 0
/// growing by 1..3, width 8 growing by 3; in at once, out by 0x13 once
/// the radius is 11+; blue (0x20, 0x20, 0xC0) on odd ages, else (0x50,
/// 0x50, 0x80)).
fn band(fx: &mut Fx, r: &Rec, t: u32) {
    let [c0, c1, c2] = r.band;
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0);
        fx.mem.set32(t + 0x10, 0x4000);
        fx.mem.set16(t + 0x34, 1);
        fx.mem.set16(t + 0x36, 2);
        let v = rmod(fx, t, BAND + r.steps + 0x38, 3);
        fx.mem.set16(t + 8, v + 1);
        fx.mem.set16(t + 6, 8);
        l2::rgb(fx, t + 0x28, c0.0, c0.1, c0.2);
        l2::rgb(fx, t + 0x2C, c0.0, c0.1, c0.2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) <= 0 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x80));
        }
    } else if fx.mem.i16(t + 0x38) >= 0xB {
        if a >= 0x15 {
            fx.mem.write_u8(t + 0x2B, a - 0x13);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        l2::rgb(fx, t + 0x28, c1.0, c1.1, c1.2);
    } else {
        l2::rgb(fx, t + 0x28, c2.0, c2.1, c2.2);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let v = fx.mem.i16(t + 0x38) + fx.mem.i16(t + 8);
        fx.mem.set16(t + 0x38, v);
    }
    let w = fx.mem.i16(t + 6) + 3;
    fx.mem.set16(t + 6, w);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E032D0 (D, task): a grey sprite (size 60, +0x30 rand % 180, +0x3A
/// rand % 360) rising 1 a update, its cell rand % 8 each update; 0x80 for
/// 7 updates, out by 0xF.
fn sprite(fx: &mut Fx, r: &Rec, t: u32) {
    let at = SPRITE + r.steps;
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x3C);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, at + 0x40, 0xB4);
        fx.mem.set16(t + 0x30, v);
        let v = rmod(fx, t, at + 0x5C, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 7 {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0x11 {
            fx.mem.write_u8(t + 0x2B, a - 0xF);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    let v = fx.rand_from(t, at + 0xC0);
    fx.mem.set16(t + 0x34, v % 8);
    let y = fx.mem.i16(t + 2) + 1;
    fx.mem.set16(t + 2, y);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03AA0 (D, task): a grey turned quad ring (size 100, cell 3, node
/// +0xA from 0 rising by 2 a update, radius from 0 growing by 60..77, row
/// 0..19, life 2..5; alpha in by 0x40, out by 10).
fn ring_b(fx: &mut Fx, r: &Rec, t: u32) {
    let at = RING_B + r.steps;
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x64);
        fx.mem.set16(t + 0x34, 3);
        let v = rmod(fx, t, at + 0x30, 0xFFFF);
        fx.mem.set32(t + 0x18, v);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x30, 0);
        let v = rmod(fx, t, at + 0x5C, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rmod(fx, t, at + 0x78, 0x12);
        fx.mem.set16(t + 0xE, v + 0x3C);
        let v = rmod(fx, t, at + 0x98, 0x14);
        fx.mem.set16(t + 8, v);
        fx.mem.set16(t + 0xA, 0);
        let v = fx.rand_from(t, at + 0xB8);
        fx.mem.set16(t + 6, v % 4 + 2);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < 0x81 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x40));
        }
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x2B, a - 0xA);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let v = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, v);
    }
    let n = fx.mem.i16(t + 0xA) + 2;
    fx.mem.set16(t + 0xA, n);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03930 (D, task): a dark-blue dome (0x10, 0x10, 0x20), turned
/// -0x4000 / 0x4000 about x by the facing, size 500, latitude +0x30 from
/// 90 rising by 0..6 a update (the task over at 270); alpha in by 0x50
/// for 8 updates, out by 9. Frame 0x30.
fn dome_step(fx: &mut Fx, r: &Rec, t: u32) {
    let sp = fx.enter(0x30);
    l2::rgb(fx, t + 0x28, r.dome.0, r.dome.1, r.dome.2);
    if fx.mem.i16(t + 0x36) == 1 {
        let h = fx.self_handle();
        fx.unit_rotation(h, sp + 0x20);
        let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x24)), 180.0);
        fx.fix_residue(y, DOME + r.steps + 0x70);
        let f = ee::fixsfsi(y);
        fx.mem.set32(t + 0x10, if f != 0 { -0x4000 } else { 0x4000 });
        fx.mem.set32(t + 0x14, 0);
        fx.mem.set32(t + 0x18, 0);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x30, 0x5A);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < 8 {
        if a < 0x50 {
            fx.mem.write_u8(t + 0x2B, a + 0x50);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x2B, a - 9);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    fx.mem.set16(t + 0x38, 0x1F4);
    age(fx, t);
    if fx.mem.i16(t + 0x30) < 0x10E {
        let v = rmod(fx, t, DOME + r.steps + 0x124, 7);
        let l = fx.mem.i16(t + 0x30) + v;
        fx.mem.set16(t + 0x30, l);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    fx.leave(0x30);
}

/// b148's dome cut: 12 pieces of 30 degrees every 30 (b046's 0x1E03F40:
/// 18 of 10 every 20).
pub(crate) const DOME_12: (i32, i32, u32) = (0x1E, 0x1E, 0xC);

/// 0x1E05240(D, task, desc, draw): `draw` 0 sets the descriptor (strip3,
/// atlas 0x97 cell +0x34, colour +0x28, texture D+0x2EC), 1 draws the
/// dome band: 12 pieces of 30 degrees from +0x3A between latitudes +0x30
/// and +0x30 + 20 (radius 90, y 90 sin - 90; the second sine table) at
/// the task's point, scale size / 1500, turned by +0x10; each keyed by its
/// mean w (into D+0x362C) into D+0x361C.
pub(crate) fn dome(fx: &mut Fx, t: u32, d: u32, draw: bool, cut: (i32, i32, u32)) {
    let sp = fx.enter(0x140);
    if !draw {
        let sub = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0x130, 0x97, sub);
        l2::desc_strip3(fx, d);
        l2::colour_strip3(fx, d, t + 0x28, 0);
        l2::uv_strip3(fx, d, sp + 0x130, 0);
        let tex = fx.mem.read_u32(D + 0x2EC);
        fx.desc_texture(d, tex, 1);
        fx.leave(0x140);
        return;
    }
    for k in 0..3 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(sp + 0xD0 + 4 * k, v);
    }
    let s = ee::div(ee::from_i32(fx.mem.i16(t + 0x38)), 1500.0);
    l2::set3(fx, sp + 0xE0, s, s, s);
    l2::xform_vp(fx, sp + 0xF0, sp + 0xE0, t + 0x10, sp + 0xD0);
    let lat = fx.mem.i16(t + 0x30);
    let lat2 = (lat + 0x14) as i16 as i32;
    let y = ee::sub(ee::mul(90.0, l2::sin_deg(fx, SINES_B, lat, 0)), 90.0);
    fx.mem.setf(sp + 0x94, y);
    fx.mem.setf(sp + 0xA4, y);
    let y = ee::sub(ee::mul(90.0, l2::sin_deg(fx, SINES_B, lat2, 0)), 90.0);
    fx.mem.setf(sp + 0xB4, y);
    fx.mem.setf(sp + 0xC4, y);
    let f20 = ee::mul(90.0, l2::sin_deg(fx, SINES_B, lat, 1));
    let f21 = ee::mul(90.0, l2::sin_deg(fx, SINES_B, lat2, 1));
    let (width, every, n) = cut;
    let mut a = 0i32;
    for _ in 0..n {
        for (off, outer, inner) in [(0i32, 0xB0u32, 0x90u32), (width, 0xC0, 0xA0)] {
            let deg = (fx.mem.i16(t + 0x3A) + off + a) as i16 as i32;
            let c = l2::sin_deg(fx, SINES_B, deg, 1);
            fx.mem.setf(sp + outer, ee::mul(f21, c));
            let s = l2::sin_deg(fx, SINES_B, deg, 0);
            fx.mem.setf(sp + outer + 8, ee::mul(f21, s));
            let c = l2::sin_deg(fx, SINES_B, deg, 1);
            fx.mem.setf(sp + inner, ee::mul(f20, c));
            let s = l2::sin_deg(fx, SINES_B, deg, 0);
            fx.mem.setf(sp + inner + 8, ee::mul(f20, s));
        }
        if l2::project(fx, sp + 0x90, SPR, sp + 0xF0, d, D + 0x362C) {
            let key = ee::fixunssfsi(fx.mem.f(D + 0x362C)) as i32;
            let ot = fx.mem.read_u32(D + 0x361C);
            fx.submit(d, ot, key);
        }
        a = (a + every) as i16 as i32;
    }
    fx.leave(0x140);
}

/// 554995841cb2 [2 B records] at 0x1E02530 (D, task, desc, 0): nine
/// textured wedges (a strip3: centre twice, rim at radius i16 +0x38) of
/// 20 degrees every 40 from i16 +0x3A, at the task's point turned by
/// +0x10 (no scale); colours +0x28 (centre) / +0x2C (rim), UVs from D+0x94
/// (D+0xBE's order), texture D+0x88; each submitted D+0xB8 + 1 times at
/// its mean w.
fn fan9(fx: &mut Fx, t: u32, d: u32) {
    let sp = fx.enter(0x120);
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x28, 1);
    l2::colour_strip3(fx, d, t + 0x2C, 2);
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(sp + 0xC0 + 4 * k, v);
    }
    l2::xform_vp(fx, sp + 0xD0, 0, t + 0x10, sp + 0xC0);
    for o in [0x84u32, 0x94, 0xA4, 0xB4, 0x80, 0x88, 0x90, 0x98] {
        fx.mem.set32(sp + o, 0);
    }
    let tab = D + 0xD8;
    let mut a = 0i32;
    for _ in 0..9 {
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
        if l2::project(fx, sp + 0x80, SPR, sp + 0xD0, d, sp + 0x110) {
            let mut n = 0;
            while n < fx.mem.i16(D + 0xB8) + 1 {
                let key = ee::fixunssfsi(fx.mem.f(sp + 0x110)) as i32;
                let ot = fx.mem.read_u32(D + 0x80);
                fx.submit(d, ot, key);
                n += 1;
            }
        }
        a = (a + 0x28) as i16 as i32;
    }
    fx.leave(0x120);
}
