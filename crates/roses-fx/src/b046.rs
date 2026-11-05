//! BMODULE b046 (card 230), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E032B0, data area 0x1ED0000, 0x3BD0 bytes) [F]
//!: b148's pool and globals (200 tasks of 0x40
//! at D+0x2F0, spawned by the same 0x1E03E20 = b148's 0x1E05120, timer
//! D+0x3610 = 0xA3, counter D+0x365A) with its own steps. Updates
//! 0x50..0x82 a grey sprite (0x1E022F0, drawn three times by
//! `billboard::sprite_square`) at each of nodes 0xA / 0x15; on 0x5E at
//! node 0x25 (kind 1): 15 yellow drifting sparkles (0x1E02450, drawn by
//! `billboard::sprite_drift`), a yellow band ring (0x1E02770,
//! `quads4::ring_band_task`), ten orange domes (0x1E02A00; b148's dome
//! band cut in 18 pieces of 10 degrees every 20), twelve grey quad rings
//! (0x1E02170, `quads4::ring_spun`), two discs (0x1E02690, `quads4::fan18`)
//! and a camera task (0x1E02F80, `task40::camera_z4`); to 0x82 the spawn
//! point kept at node 0x25. Returns 1 on update 0x54, 4 on 0x96, 2 on
//! 0x5E, 8 once the timer has run out.
//!
//! The steps 0x1E02890, 0x1E02B80 (`task40::mote_target`), 0x1E02DB0
//! (`task40::mote_node`), 0x1E030F0 (the stage light) and the object op
//! 0x1E031B0 are tested by the entry but never spawned [F: the emitter's
//! spawns name 0x1E022F0 / 0x1E02450 / 0x1E02770 / 0x1E02A00 / 0x1E02170
//! / 0x1E02690 / 0x1E02F80 only]; not ported.
//!
//! Also here: b527 (card 256, b046's twin through [`Rec`]) and b484 (card
//! 415) / b477 (card 431), which share b046's init ([`Init`]), pool and
//! step bodies with their own emitters and draw loops.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b148::{age, dome, facing_of, fade, rmod, spawn_task};
use crate::blib::{billboard, quads4, task40, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 200;
const POOL: u32 = D + 0x2F0;
const POS: u32 = D + 0x250;
const TIMER: u32 = D + 0x3610;
const COLOUR: u32 = D + 0x3614;
const NODES: u32 = D + 0x363C;
const KIND: u32 = D + 0x3656;
const COUNTER: u32 = D + 0x365A;
const TABLE: u32 = D + 0x365C;
const SINES_B: u32 = D + 0x3A5C;
/// b046's dome cut (0x1E03F40).
const DOME_18: (i32, i32, u32) = (0xA, 0x14, 0x12);

type Rgb = (i32, i32, i32);

/// One record of the family: b046 (card 230) or b527 (card 256: the
/// entry's init 0x3C8 later (three more nodes), its own emitter and step
/// constants).
struct Rec {
    init: Init,
    /// The steps ring, sprite, sparkle, disc, band, dome, camera.
    at: [u32; 7],
    /// The return addresses after the steps' `jal`s (same order).
    ra: [u32; 7],
    /// The return address after the target position (0x1F71A0).
    target_ra: u32,
    /// The sprite's draws a update.
    sprite_draws: u32,
    /// Ring: size, life base (+6 = rand % 9 + base).
    ring: (i32, i32),
    /// Sprite: drifts by +8 / +0xA / +0xC on ages 1 mod 4.
    sprite_moves: bool,
    /// Sparkle: the colour of kind != 0, fade (lowest kept, step).
    sparkle: (Rgb, u8, u8),
    disc: (Rgb, Rgb),
    /// Band: both colours, then (odd ages, even ages) each update.
    band: (Rgb, Option<(Rgb, Rgb)>),
    /// Dome: first colour, size, fade (lowest kept, step).
    dome: (Rgb, i32, u8, u8),
    camera: fn(&mut Fx, u32, u32, u32) -> i32,
    emit: fn(&mut Fx, &Rec, i32),
    ret: fn(i32) -> Option<i32>,
}

/// The init of a family record (b046's 0x1E032B0 with its constants).
struct Init {
    /// Added to b046's init return addresses.
    shift: i32,
    timer: i32,
    nodes: &'static [i32],
    /// The first table: rand % m - m / 2.
    table0: i32,
    /// The facing reads' first local (sp + 0xC0 / 0xB0), then + 0x10.
    facing: u32,
    /// Textures of group 5 loaded (D+0x35F0..).
    tex5: u32,
    /// The first table's 0xB4.. cleared (b477: not, its later code 0x20
    /// earlier).
    zero_b4: bool,
}

const B046: Rec = Rec {
    init: Init { shift: 0, timer: 0xA3, nodes: &[0xA, 0x15, 0x25, 0x3C, 0x25], table0: 0xA, facing: 0xC0, tex5: 3, zero_b4: true },
    at: [0x01E0_2170, 0x01E0_22F0, 0x01E0_2450, 0x01E0_2690, 0x01E0_2770, 0x01E0_2A00, 0x01E0_2F80],
    ra: [0x01E0_3710, 0x01E0_3730, 0x01E0_3750, 0x01E0_3770, 0x01E0_3790, 0x01E0_37D0, 0x01E0_3850],
    target_ra: 0x01E0_38C4,
    sprite_draws: 3,
    ring: (0x32, 8),
    sprite_moves: false,
    sparkle: ((0xF0, 0xF0, 0x20), 0xA, 8),
    disc: ((0xC0, 0xC0, 0xC0), (0xF0, 0xF0, 0x10)),
    band: ((0xF0, 0xF0, 0x20), None),
    dome: ((0x8F, 0x4F, 0x10), 0x258, 8, 6),
    camera: task40::camera_z4,
    emit,
    ret: |c| match c {
        0x54 => Some(1),
        0x96 => Some(4),
        0x5E => Some(2),
        _ => None,
    },
};

const B527: Rec = Rec {
    init: Init { shift: 0x3C8, timer: 0x78, nodes: &[0x21, 0x2F, 0xE, 0x3C, 0x25, 0x2C, 0x36, 0x37], table0: 0xA, facing: 0xC0, tex5: 3, zero_b4: true },
    at: [0x01E0_2440, 0x01E0_25C0, 0x01E0_2770, 0x01E0_29B0, 0x01E0_2A90, 0x01E0_2D70, 0x01E0_32F0],
    ra: [0x01E0_3AD8, 0x01E0_3AF8, 0x01E0_3B18, 0x01E0_3B38, 0x01E0_3B58, 0x01E0_3B98, 0x01E0_3BF8],
    target_ra: 0x01E0_3C8C,
    sprite_draws: 1,
    ring: (0x14, 2),
    sprite_moves: true,
    sparkle: ((0x80, 0x10, 0xF0), 0x11, 0xF),
    disc: ((0x80, 0x80, 0x80), (0xF0, 0x10, 0x10)),
    band: ((0xF0, 0x10, 0x20), Some(((0x20, 0x20, 0x80), (0x48, 0x10, 0x88)))),
    dome: ((0x8F, 0x10, 0x10), 0x1F4, 0xB, 9),
    camera: task40::camera,
    emit: emit_b527,
    ret: |c| match c {
        0x44 => Some(1),
        0x6E => Some(4),
        0x5A => Some(2),
        0x4F..=0x54 => Some(6),
        _ => None,
    },
};

/// The entry 0x1E032B0 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    program_by(fx, &B046, param)
}

/// BMODULE b527 (card 256): b046's twin (entry 0x1E03630; see [`Rec`]).
pub fn program_b527(fx: &mut Fx, param: i32) -> i32 {
    program_by(fx, &B527, param)
}

fn program_by(fx: &mut Fx, r: &Rec, param: i32) -> i32 {
    let sp = fx.enter(0x190);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x361C, OT);
    let ret = if param >= 0 { init(fx, &r.init, sp, param) } else { step(fx, r, sp, param) };
    fx.leave(0x190);
    ret
}

fn init(fx: &mut Fx, r: &Init, sp: u32, param: i32) -> i32 {
    let e = |a: u32| {
        let back = if a > 0x01E0_33C4 && !r.zero_b4 { 0x20 } else { 0 };
        a.wrapping_add(r.shift as u32).wrapping_sub(back)
    };
    fx.bzero(D, 0x3BD0);
    l2::sine_table(fx, SINES_B);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x3658, param);
    fx.mem.set32(TIMER, r.timer);
    for (k, &n) in r.nodes.iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    // 0x1E03200(0, D): the textures (frame 0x40: ra, the entry's s2 =
    // param, s1 = 0, s0 = D).
    let f = fx.enter(0x40);
    fx.saved(f, &[(0x30, u64::from(e(0x01E0_3394))), (0x20, param as i64 as u64), (0x10, 0), (0, u64::from(D))]);
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
        let v = rmod(fx, D, e(0x01E0_339C), r.table0);
        fx.mem.set8(TABLE + k, v - r.table0 / 2);
    }
    if r.zero_b4 {
        for k in 0xB4..0x100u32 {
            fx.mem.write_u8(TABLE + k, 0);
        }
    }
    let b = TABLE + 0x100;
    for k in 0..0x64u32 {
        fx.mem.write_u8(b + k, 0);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, D, e(0x01E0_3410), 5);
        fx.mem.set8(b + k, v);
    }
    let f = facing_of(fx, sp + r.facing, e(0x01E0_346C));
    for k in 0xB4..0x100u32 {
        let (ra, sign) = if f != 0 { (e(0x01E0_3484), 1) } else { (e(0x01E0_34AC), -1) };
        let v = fx.rand_from(D, ra);
        fx.mem.set8(b + k, sign * (v % 4));
    }
    let b = TABLE + 0x200;
    for k in 0..0x64u32 {
        fx.mem.write_u8(b + k, 0);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, D, e(0x01E0_350C), 6);
        fx.mem.set8(b + k, v);
    }
    let f = facing_of(fx, sp + r.facing + 0x10, e(0x01E0_3568));
    for k in 0xB4..0x100u32 {
        let (ra, sign) = if f != 0 { (e(0x01E0_3580), 1) } else { (e(0x01E0_35AC), -1) };
        let v = rmod(fx, D, ra, 0x14);
        fx.mem.set8(b + k, sign * (v + 5));
    }
    let b = TABLE + 0x300;
    for k in 0..0x64u32 {
        fx.mem.write_u8(b + k, 0);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, D, e(0x01E0_360C), 3);
        fx.mem.set8(b + k, v);
    }
    for k in 0xB4..0x100u32 {
        let v = fx.rand_from(D, e(0x01E0_3638));
        fx.mem.set8(b + k, v % 2 - 1);
        let f = facing_of(fx, sp + r.facing + 0x20, e(0x01E0_3694));
        let c = fx.mem.i8(b + k);
        fx.mem.set8(b + k, if f != 0 { c + 2 } else { c - 2 });
    }
    0
}

fn step(fx: &mut Fx, r: &Rec, sp: u32, param: i32) -> i32 {
    (r.emit)(fx, r, param);
    let [ring_at, sprite_at, sparkle_at, disc_at, band_at, dome_at, camera_at] = r.at;
    let low = fx.sp;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x24);
        // (frame, ra after the step's jal, the step's slot of the entry's s1).
        let Some(i) = r.at.iter().position(|&a| a == f) else {
            debug_assert!(false, "b046: unported task step {f:#x}");
            continue;
        };
        let (frame, s1): (u32, Option<u64>) = match i {
            0 => {
                ring(fx, r, t);
                (0x20, None)
            }
            1 => {
                sprite(fx, r, t);
                (0x20, None)
            }
            2 => {
                sparkle(fx, r, t);
                (0x30, Some(0))
            }
            3 => {
                disc(fx, r, t);
                (0x20, None)
            }
            4 => {
                band(fx, r, t);
                (0x20, None)
            }
            5 => {
                dome_step(fx, r, t);
                (0x30, None)
            }
            _ => {
                (r.camera)(fx, camera_at, D, t);
                (0x30, Some(0))
            }
        };
        let ra = r.ra[i];
        let base = low - frame;
        match s1 {
            Some(v) => fx.saved(base, &[(0x20, u64::from(ra)), (0x10, v), (0, u64::from(D))]),
            None => fx.saved(base, &[(0x10, u64::from(ra)), (0, u64::from(D))]),
        }
    }
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x50, u64::from(D), r.target_ra);
    let d = sp + 0xF8;
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
        let point_at = |fx: &mut Fx, to: u32| {
            for k in 0..3 {
                let v = ee::from_i32(fx.mem.i16(t + 2 * k));
                fx.mem.setf(to + 4 * k, v);
            }
        };
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x24);
        if f(fx) == ring_at {
            set_rect(fx, D + 0x35F8, sp + 0x60, 0x14);
            fx.mem.set16(D + 0xB8, 0xA);
            point_at(fx, D + 0x20);
            quads4::ring_spun(fx, uv::uv_rect, D, t, 0, TABLE);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == sprite_at {
            set_rect(fx, D + 0x35F0, sp + 0x70, 0x14);
            point_at(fx, sp + 0x50);
            for _ in 0..r.sprite_draws {
                billboard::sprite_square(fx, 0x01E0_0DD0, uv::uv_rect, D, t, 1, sp + 0x50);
            }
        }
        if f(fx) == sparkle_at {
            set_rect(fx, D + 0x35F0, sp + 0x80, 0x14);
            point_at(fx, sp + 0x50);
            billboard::sprite_drift(fx, 0x01E0_0BB0, uv::uv_rect, D, t, 0);
        }
        if f(fx) == disc_at {
            quads4::fan18(fx, D, t, d);
        }
        if f(fx) == band_at {
            set_rect(fx, D + 0x2EC, sp + 0x90, 0x97);
            fx.mem.set16(D + 0x94, 0xA0);
            let h = fx.mem.i16(D + 0x9A) >> 1;
            fx.mem.set16(D + 0x9A, h);
            fx.mem.set16(D + 0xB8, 0xA);
            quads4::ring_band_task(fx, 0x01E0_1520, D, t, d);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == dome_at {
            dome(fx, t, d, false, DOME_18);
            dome(fx, t, d, true, DOME_18);
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

/// 0x1E01ED0(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the spawns (frame 0x60: ra, the entry's s3 (the runner's, 0 [H]),
/// s2 = param, s1 = 0, s0 = D). After the first node its s2 / s3 are sp +
/// 0x54 / + 0x58 and s1 each loop's count.
fn emit(fx: &mut Fx, r: &Rec, param: i32) {
    let [ring_at, sprite_at, sparkle_at, disc_at, band_at, dome_at, camera_at] = r.at;
    let p = param as i64 as u64;
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x40, 0x01E0_36DC), (0x30, 0), (0x20, p), (0x10, 0), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let node = |fx: &mut Fx, k: u32, s1: u64, ra: u32| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2 * k);
        node_position_saving(fx, h, slot, sp + 0x50, u64::from(D), Some(s1), ra);
        for k in 0..3 {
            let w = fx.mem.read_u32(sp + 0x50 + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
    };
    let dd = u64::from(D);
    let (mut s1, mut s2, mut s3) = (0u64, p, 0u64);
    if (0x50..0x83).contains(&c(fx)) {
        node(fx, 0, s1, 0x01E0_1F40);
        (s2, s3) = (u64::from(sp + 0x54), u64::from(sp + 0x58));
        spawn_task(fx, sprite_at, [dd, 0, s2, s3, 0], 0x01E0_1F78);
        s1 = 1;
        node(fx, 1, s1, 0x01E0_1F9C);
        spawn_task(fx, sprite_at, [dd, 0, s2, s3, 0], 0x01E0_1FCC);
        s1 = 1;
    }
    if c(fx) == 0x5E {
        node(fx, 2, s1, 0x01E0_2000);
        fx.mem.set16(KIND, 1);
        for (f, n, ra) in [(sparkle_at, 0xFu32, 0x01E0_2038u32), (band_at, 1, 0x01E0_2060), (dome_at, 0xA, 0x01E0_2084), (ring_at, 0xC, 0x01E0_20AC)] {
            for i in 0..n {
                spawn_task(fx, f, [dd, u64::from(i), s2, s3, 0], ra);
            }
            s1 = u64::from(n);
        }
        spawn_task(fx, disc_at, [dd, s1, s2, s3, 0], 0x01E0_20D0);
        spawn_task(fx, disc_at, [dd, s1, s2, s3, 0], 0x01E0_20E4);
        spawn_task(fx, camera_at, [dd, s1, s2, s3, 0], 0x01E0_20F8);
    }
    if (0x5E..0x83).contains(&c(fx)) {
        node(fx, 2, s1, 0x01E0_212C);
        fx.mem.set16(KIND, 1);
    }
    fx.leave(0x60);
}

/// b527's emitter 0x1E01ED0(D, 0) (frame 0x60: ra, the entry's s3 (0
/// [H]), s2 = param, s1 = 0, s0 = D; its s2 is D): on updates 0x19 / 0x1A
/// eight grey sprites at each of nodes 0x21 / 0x2F (the point moved by
/// rand % 2 - 1 per axis), on 0x36 / 0x43 fifteen at each of nodes 0xE /
/// 0x3C (x / z by rand % 10 - 5, y by rand % 2 - 1); on 0x4E at node 0x25
/// (kind 1) b046's set: 15 sparkles, the band, ten domes, twelve rings, a
/// disc and the camera. Its s0 / s1 are sp + 0x54 / + 0x58 from each
/// jitter's third rand, s0 / s3 the loops' counts.
fn emit_b527(fx: &mut Fx, r: &Rec, param: i32) {
    let [ring_at, sprite_at, sparkle_at, disc_at, band_at, dome_at, camera_at] = r.at;
    let p = param as i64 as u64;
    let sp = fx.enter(0x60);
    let dd = u64::from(D);
    fx.saved(sp, &[(0x40, 0x01E0_3AA4), (0x30, 0), (0x20, p), (0x10, 0), (0, dd)]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let (q0, q1) = (u64::from(sp + 0x54), u64::from(sp + 0x58));
    let node = |fx: &mut Fx, k: u32, s0: u64, s1: u64, ra: u32| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2 * k);
        node_position_saving(fx, h, slot, sp + 0x50, s0, Some(s1), ra);
    };
    // The point: the node plus rand % m[k] (HI for 10, the AND for 2) less
    // `half`[k] per axis, rand called with s0 = `s0`[k], returning to ra[k].
    let jitter = |fx: &mut Fx, m: [i32; 3], s0: [u64; 3], ra: [u32; 3]| {
        for k in 0..3 {
            let v = fx.rand_from(s0[k] as u32, ra[k]);
            let (d, half) = if m[k] == 2 { (v % 2, 1.0) } else { (fx.div(v, m[k]).1, 5.0) };
            let x = ee::sub(ee::add(fx.mem.f(sp + 0x50 + 4 * k as u32), ee::from_i32(d)), half);
            fx.mem.setf(POS + 4 * k as u32, x);
        }
    };
    // Two nodes' sprites: (node, its jitter's rand returns, the spawns' ra).
    let pair = |fx: &mut Fx, m: [i32; 3], n: u32, a: (u32, [u32; 3], u32), b: (u32, [u32; 3], u32)| {
        node(fx, a.0, dd, 0, a.1[0] - 8);
        jitter(fx, m, [dd, dd, q0], a.1);
        for i in 0..n {
            spawn_task(fx, sprite_at, [q0, q1, dd, u64::from(i), 0], a.2);
        }
        node(fx, b.0, q0, q1, b.1[0] - 8);
        jitter(fx, m, [q0, q0, q0], b.1);
        for i in 0..n {
            spawn_task(fx, sprite_at, [u64::from(i), q1, dd, u64::from(n), 0], b.2);
        }
    };
    if matches!(c(fx), 0x19 | 0x1A) {
        pair(fx, [2, 2, 2], 8, (0, [0x01E0_1F48, 0x01E0_1F84, 0x01E0_1FC4], 0x01E0_2018), (1, [0x01E0_2048, 0x01E0_2084, 0x01E0_20C0], 0x01E0_2110));
    }
    if matches!(c(fx), 0x36 | 0x43) {
        pair(fx, [10, 2, 10], 0xF, (2, [0x01E0_215C, 0x01E0_2190, 0x01E0_21D0], 0x01E0_221C), (3, [0x01E0_224C, 0x01E0_2280, 0x01E0_22BC], 0x01E0_2304));
    }
    if c(fx) == 0x4E {
        node(fx, 4, dd, 0, 0x01E0_2340);
        for k in 0..3 {
            let w = fx.mem.read_u32(sp + 0x50 + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
        fx.mem.set16(KIND, 1);
        let mut s0 = 0u64;
        for (f, n, ra) in [(sparkle_at, 0xFu32, 0x01E0_2378u32), (band_at, 1, 0x01E0_23A0), (dome_at, 0xA, 0x01E0_23C4), (ring_at, 0xC, 0x01E0_23EC)] {
            for i in 0..n {
                spawn_task(fx, f, [u64::from(i), 0, dd, 0, 0], ra);
            }
            s0 = u64::from(n);
        }
        spawn_task(fx, disc_at, [s0, 0, dd, 0, 0], 0x01E0_2410);
        spawn_task(fx, camera_at, [s0, 0, dd, 0, 0], 0x01E0_2424);
    }
    fx.leave(0x60);
}

/// 0x1E02170 (D, task): a grey quad ring (size 50 (b527: 20), cell 0..7,
/// angle z 0..359, radius from 0 growing by 70..109 from the second
/// update, row 0..9, life 8..16 (b527: 2..10); 0x80 during the life, out
/// by 0xF).
fn ring(fx: &mut Fx, r: &Rec, t: u32) {
    ring_by(fx, r.at[0], (r.ring.0, r.ring.1, 0x46), t);
}

/// [`ring`] at `at` with (size, life base, growth base (+0xE = rand % 40 +
/// it; b500: 0x14)) `k`.
pub(crate) fn ring_by(fx: &mut Fx, at: u32, k: (i32, i32, i32), t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, k.0);
        let v = fx.rand_from(t, at + 0x28);
        fx.mem.set16(t + 0x34, v % 8);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        fx.mem.set32(t + 0x18, 0);
        let v = rmod(fx, t, at + 0x5C, 0x168);
        fx.mem.set32(t + 0x18, v);
        fx.mem.set16(t + 0x30, 0);
        let v = rmod(fx, t, at + 0x7C, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rmod(fx, t, at + 0x98, 0x28);
        fx.mem.set16(t + 0xE, v + k.2);
        let v = rmod(fx, t, at + 0xB8, 0xA);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, at + 0xD4, 9);
        fx.mem.set16(t + 6, v + k.1);
        fx.mem.set16(t + 0x36, 2);
    }
    fade(fx, t, Some(0x80));
    if fx.mem.i16(t + 0x32) != 0 {
        let v = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, v);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// The sprites' first update: +0x30 rand % `m30`, +0x3A rand % 360, the
/// drift +8 / +0xA / +0xC rand % 4 - 2 each (rand returning to `at` + `ra`
/// + 0, 0x1C, 0x38, 0x58, 0x78).
fn sprite_first(fx: &mut Fx, t: u32, at: u32, ra: u32, m30: i32) {
    let v = rmod(fx, t, at + ra, m30);
    fx.mem.set16(t + 0x30, v);
    let v = rmod(fx, t, at + ra + 0x1C, 0x168);
    fx.mem.set16(t + 0x3A, v);
    for (k, o) in [8u32, 0xA, 0xC].into_iter().enumerate() {
        let v = fx.rand_from(t, at + ra + 0x38 + 0x20 * k as u32);
        fx.mem.set16(t + o, v % 4 - 2);
    }
    fx.mem.set16(t + 0x36, 2);
}

/// 0x1E022F0 (D, task): a grey sprite (size 60, +0x30 rand % 180, +0x3A
/// rand % 360, drift rand % 4 - 2: b527 moves by it on ages 1 mod 4), its
/// cell rand % 8 each update; 0x80 for 7 updates, out by 0xF.
fn sprite(fx: &mut Fx, r: &Rec, t: u32) {
    sprite_by(fx, r.at[1], (0x3C, (0x80, 0x80, 0x80), 0xB4), r.sprite_moves, t);
}

/// [`sprite`] at `at` with (size, colour, +0x30's modulus) `k`.
fn sprite_by(fx: &mut Fx, at: u32, k: (i32, Rgb, i32), moves: bool, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, k.0);
        l2::rgb(fx, t + 0x28, k.1 .0, k.1 .1, k.1 .2);
        sprite_first(fx, t, at, 0x40, k.2);
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
    let v = fx.rand_from(t, at + 0x120);
    fx.mem.set16(t + 0x34, v % 8);
    if moves && fx.mem.i16(t + 0x32) % 4 == 1 {
        for k in 0..3 {
            let v = fx.mem.i16(t + 2 * k) + fx.mem.i16(t + 8 + 2 * k);
            fx.mem.set16(t + 2 * k, v);
        }
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02450 (D, task): a sparkle (size 200, white for kind 0, else
/// yellow (0xF0, 0xF0, 0x20) (b527: purple (0x80, 0x10, 0xF0))), +0x30 /
/// +0x3A rand % 360, drift rand % 4 - 2 then pushed each update by 40 sin
/// / 4 cos / 40 cos of its angles (the second sine table); its cell rand %
/// 8; 0x80 for 7 updates, out by 8 (b527: 0xF). Frame 0x30.
fn sparkle(fx: &mut Fx, r: &Rec, t: u32) {
    sparkle_by(fx, r.at[2], r.sparkle, t);
}

/// [`sparkle`] at `at` with (kind 1 colour, fade) `k`.
fn sparkle_by(fx: &mut Fx, at: u32, k: (Rgb, u8, u8), t: u32) {
    let (k1, low, fall) = k;
    fx.enter(0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0xC8);
        if fx.mem.i16(t + 0x30) == 0 {
            l2::rgb(fx, t + 0x28, 0xF0, 0xF0, 0xF0);
        } else {
            l2::rgb(fx, t + 0x28, k1.0, k1.1, k1.2);
        }
        sprite_first(fx, t, at, 0x70, 0x168);
    }
    if fx.mem.i16(t + 0x32) < 7 {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= low {
            fx.mem.write_u8(t + 0x2B, a - fall);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    let v = fx.rand_from(t, at + 0x150);
    fx.mem.set16(t + 0x34, v % 8);
    for (o, ang, cos, k, ra) in [(8u32, 0x3Au32, 0, 40.0f32, 0x19Cu32), (0xC, 0x30, 1, 4.0, 0x1D4), (0xA, 0x3A, 1, 40.0, 0x20C)] {
        let deg = fx.mem.i16(t + ang);
        let s = l2::sin_deg(fx, SINES_B, deg, cos);
        let v = ee::add(ee::from_i32(fx.mem.i16(t + o)), ee::mul(k, s));
        fx.fix_residue(v, at + ra);
        fx.mem.set16(t + o, ee::fixsfsi(v));
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02690 (D, task): the disc (size 430, grey (0xC0, 0xC0, 0xC0) /
/// yellow rim (0xF0, 0xF0, 0x10) (b527: (0x80, 0x80, 0x80) / red (0xF0,
/// 0x10, 0x10)), +0x3A rand % 360, +0x2F 0); 0x80 for 5 updates, out by
/// 0xA.
fn disc(fx: &mut Fx, r: &Rec, t: u32) {
    disc_by(fx, r.at[3], r.disc, 0, t);
}

/// [`disc`] at `at` with the colours `c`, its size growing by `grow` a
/// update after the age.
fn disc_by(fx: &mut Fx, at: u32, c: (Rgb, Rgb), grow: i32, t: u32) {
    let (c1, c2) = c;
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x1AE);
        l2::rgb(fx, t + 0x28, c1.0, c1.1, c1.2);
        l2::rgb(fx, t + 0x2C, c2.0, c2.1, c2.2);
        let v = rmod(fx, t, at + 0x54, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if fx.mem.i16(t + 0x32) < 5 {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0x11 {
            fx.mem.write_u8(t + 0x2B, a - 0xA);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    age(fx, t);
    if grow != 0 {
        let v = fx.mem.i16(t + 0x38) + grow;
        fx.mem.set16(t + 0x38, v);
    }
    fx.leave(0x20);
}

/// 0x1E02770 (D, task): the band ring, yellow (0xF0, 0xF0, 0x20) both
/// colours (b527: (0xF0, 0x10, 0x20), then blue (0x20, 0x20, 0x80) on odd
/// ages, else purple (0x48, 0x10, 0x88)) (angle x 0x4000, radius from 0
/// growing by 1..3 from the second update, width 8 growing by 3; in at
/// once, out by 0xE once the radius is 6+).
fn band(fx: &mut Fx, r: &Rec, t: u32) {
    let (c0, alt) = r.band;
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0);
        fx.mem.set32(t + 0x10, 0x4000);
        fx.mem.set16(t + 0x34, 1);
        fx.mem.set16(t + 0x36, 2);
        let v = rmod(fx, t, r.at[4] + 0x38, 3);
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
    } else if fx.mem.i16(t + 0x38) >= 6 {
        if a >= 0x10 {
            fx.mem.write_u8(t + 0x2B, a - 0xE);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    if let Some((odd, even)) = alt {
        let k = if fx.mem.i16(t + 0x32) % 2 == 1 { odd } else { even };
        l2::rgb(fx, t + 0x28, k.0, k.1, k.2);
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

/// 0x1E02A00 (D, task): an orange dome ((0x8F, 0x4F, 0x10) (b527: red
/// (0x8F, 0x10, 0x10)), rim (0x40, 0x10, 0x40)), turned -0x4000 / 0x4000
/// about x by the facing, size 600 (b527: 500), latitude +0x30 from 90
/// rising by 3..10 a update (the task over at 270); alpha in by 0x50 for
/// 4 updates, out by 6 (b527: 9). Frame 0x30.
fn dome_step(fx: &mut Fx, r: &Rec, t: u32) {
    let at = r.at[5];
    let (c1, size, low, fall) = r.dome;
    let sp = fx.enter(0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        l2::rgb(fx, t + 0x28, c1.0, c1.1, c1.2);
        l2::rgb(fx, t + 0x2C, 0x40, 0x10, 0x40);
        let h = fx.self_handle();
        fx.unit_rotation(h, sp + 0x20);
        let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x24)), 180.0);
        fx.fix_residue(y, at + 0x80);
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
    if fx.mem.i16(t + 0x32) < 4 {
        if a < 0x50 {
            fx.mem.write_u8(t + 0x2B, a + 0x50);
        }
    } else if a >= low {
        fx.mem.write_u8(t + 0x2B, a - fall);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    fx.mem.set16(t + 0x38, size);
    age(fx, t);
    if fx.mem.i16(t + 0x30) < 0x10E {
        let v = fx.rand_from(t, at + 0x134);
        let l = fx.mem.i16(t + 0x30) + (v % 8 + 3);
        fx.mem.set16(t + 0x30, l);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    fx.leave(0x30);
}

// ---- b484 (card 415): the family's init and pool with its own steps ----

const B484_INIT: Init = Init { shift: -0x1098, timer: 0xB4, nodes: &[2, 0x21, 0x3A, 0xE, 0x25, 0x34, 0x36, 0x37], table0: 0x1E, facing: 0xB0, tex5: 2, zero_b4: true };
const B484_RING: u32 = 0x01E0_16C0;
const B484_PUSH: u32 = 0x01E0_1830;
const B484_SPARKLE: u32 = 0x01E0_1A10;
const B484_SPRITE: u32 = 0x01E0_1C50;
const B484_DISC: u32 = 0x01E0_1DB0;
const B484_CAMERA: u32 = 0x01E0_1E90;

/// BMODULE b484 (card 415; entry 0x1E021D0, frame 0x180): b046's init
/// (timer 0xB4, nodes 2 / 0x21 / 0x3A / 0xE / 0x25 / 0x34 / 0x36 / 0x37,
/// the first table rand % 30 - 15, two group-5 textures) and pool. Update
/// 0x33 node 0x21 kept at D+0x3530 (the spawn point to 0x40); on 0x53 at
/// node 2 thirty b527 sparkles (0x1E01A10, kinds 0 / 1, the point
/// jittered), fourteen pink quad rings (0x1E016C0), a camera
/// (`task40::camera_far`) and a growing purple disc (0x1E01DB0); 0x4C..0x66
/// two purple sprites (0x1E01C50) a update 5 above node 2; on 0x6C /
/// 0x78 / 0x83 / 0x8C fifteen grey pushed sprites (0x1E01830) 10 above
/// it. The sprites and sparkles are drawn by `billboard::sprite_drift`.
/// Returns 1 on update 0x4B, 4 on 0x84, 2 on 0x55, 8 once the timer has
/// run out. The steps 0x1E02010 (the stage light) and the object op
/// 0x1E020D0 are tested but never spawned; not ported.
pub fn program_b484(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x180);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x361C, OT);
    let ret = if param >= 0 { init(fx, &B484_INIT, sp, param) } else { step_b484(fx, sp, param) };
    fx.leave(0x180);
    ret
}

fn step_b484(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit_b484(fx, param);
    let low = fx.sp;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x24);
        // (frame, ra after the step jal, the step slot of the entry s1).
        let (frame, ra, s1): (u32, u32, Option<u64>) = match f {
            B484_RING => {
                ring4(fx, t);
                (0x20, 0x01E0_2678, None)
            }
            B484_PUSH => {
                sprite_push(fx, t);
                (0x30, 0x01E0_2698, Some(0))
            }
            B484_SPARKLE => {
                sparkle_by(fx, B484_SPARKLE, ((0x80, 0x10, 0xF0), 0x11, 0xF), t);
                (0x30, 0x01E0_26B8, Some(0))
            }
            B484_SPRITE => {
                sprite_by(fx, B484_SPRITE, (0xC8, (0x80, 0x10, 0xF0), 0x168), false, t);
                (0x20, 0x01E0_26D8, None)
            }
            B484_DISC => {
                disc_by(fx, B484_DISC, ((0x80, 0x10, 0xF0), (0x80, 0x80, 0x80)), 0x14, t);
                (0x20, 0x01E0_26F8, None)
            }
            B484_CAMERA => {
                task40::camera_far(fx, B484_CAMERA, D, t);
                (0x30, 0x01E0_2718, Some(0))
            }
            _ => {
                debug_assert!(false, "b484: unported task step {f:#x}");
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
    unit_position_saving(fx, h, sp + 0x50, u64::from(D), 0x01E0_27AC);
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
        let point_at = |fx: &mut Fx, to: u32| {
            for k in 0..3 {
                let v = ee::from_i32(fx.mem.i16(t + 2 * k));
                fx.mem.setf(to + 4 * k, v);
            }
        };
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x24);
        if f(fx) == B484_RING {
            set_rect(fx, D + 0x2EC, sp + 0x60, 0x9B);
            fx.mem.set16(D + 0xB8, 0xA);
            point_at(fx, D + 0x20);
            quads4::ring_spun(fx, uv::uv_rect, D, t, 0, TABLE);
            fx.mem.set16(D + 0xB8, 0);
        }
        for (step, local) in [(B484_PUSH, 0x70u32), (B484_SPARKLE, 0x80), (B484_SPRITE, 0x90)] {
            if f(fx) == step {
                set_rect(fx, D + 0x35F0, sp + local, 0x14);
                point_at(fx, sp + 0x50);
                billboard::sprite_drift(fx, 0x01E0_0BB0, uv::uv_rect, D, t, 0);
            }
        }
        if f(fx) == B484_DISC {
            set_rect(fx, D + 0x2EC, sp + 0xA0, 0x9B);
            quads4::fan18(fx, D, t, sp + 0xE8);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x4B => 1,
        0x84 => 4,
        0x55 => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// b484's emitter 0x1E01240(D, 0) (frame 0x60: ra, the entry s3 (0 [H]),
/// s2 = param, s1 = 0, s0 = D; its s2 is D, s0 / s1 sp + 0x54 / + 0x58
/// after each jitter's second rand, s0 / s3 the loops' counts).
fn emit_b484(fx: &mut Fx, param: i32) {
    let p = param as i64 as u64;
    let sp = fx.enter(0x60);
    let dd = u64::from(D);
    fx.saved(sp, &[(0x40, 0x01E0_2644), (0x30, 0), (0x20, p), (0x10, 0), (0, dd)]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let (q0, q1) = (u64::from(sp + 0x54), u64::from(sp + 0x58));
    let node = |fx: &mut Fx, k: u32, out: u32, s0: u64, s1: u64, ra: u32| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2 * k);
        node_position_saving(fx, h, slot, out, s0, Some(s1), ra);
    };
    // The point: the node plus rand % 10 - 5 / rand % 2 - 1 / rand % 10 -
    // 5, rand called with s0 = `s0`[k], returning to ra[k].
    let jitter = |fx: &mut Fx, s0: [u64; 3], ra: [u32; 3]| {
        for k in 0..3 {
            let v = fx.rand_from(s0[k] as u32, ra[k]);
            let (d, half) = if k == 1 { (v % 2, 1.0) } else { (fx.div(v, 10).1, 5.0) };
            let x = ee::sub(ee::add(fx.mem.f(sp + 0x50 + 4 * k as u32), ee::from_i32(d)), half);
            fx.mem.setf(POS + 4 * k as u32, x);
        }
    };
    let at_node = |fx: &mut Fx, dy: f32| {
        for k in 0..3 {
            let w = fx.mem.read_u32(sp + 0x50 + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
        let y = ee::sub(fx.mem.f(sp + 0x54), dy);
        fx.mem.setf(POS + 4, y);
    };
    let (mut s0, mut s1, mut s3) = (dd, 0u64, 0u64);
    if c(fx) == 0x33 {
        node(fx, 1, D + 0x3530, s0, s1, 0x01E0_12A4);
    }
    if (0x34..0x41).contains(&c(fx)) {
        node(fx, 1, sp + 0x50, s0, s1, 0x01E0_12D8);
        for k in 0..3 {
            let w = fx.mem.read_u32(D + 0x3530 + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
    }
    if c(fx) == 0x53 {
        node(fx, 0, sp + 0x50, s0, s1, 0x01E0_1328);
        jitter(fx, [dd, dd, q0], [0x01E0_1330, 0x01E0_1364, 0x01E0_13A4]);
        fx.mem.set16(KIND, 0);
        for i in 0..0xFu32 {
            spawn_task(fx, B484_SPARKLE, [q0, q1, dd, u64::from(i), 0], 0x01E0_13F4);
        }
        node(fx, 0, sp + 0x50, q0, q1, 0x01E0_141C);
        jitter(fx, [q0, q0, q0], [0x01E0_1424, 0x01E0_1458, 0x01E0_1494]);
        fx.mem.set16(KIND, 1);
        for i in 0..0xFu32 {
            spawn_task(fx, B484_SPARKLE, [u64::from(i), q1, dd, 0xF, 0], 0x01E0_14E4);
        }
        for i in 0..0xEu32 {
            spawn_task(fx, B484_RING, [u64::from(i), q1, dd, 0xF, 0], 0x01E0_150C);
        }
        spawn_task(fx, B484_CAMERA, [0xE, q1, dd, 0xF, 0], 0x01E0_1530);
        spawn_task(fx, B484_DISC, [0xE, q1, dd, 0xF, 0], 0x01E0_1544);
        (s0, s1, s3) = (0xE, q1, 0xF);
    }
    if (0x4C..0x67).contains(&c(fx)) {
        node(fx, 0, sp + 0x50, s0, s1, 0x01E0_1578);
        at_node(fx, 5.0);
        spawn_task(fx, B484_SPRITE, [q0, q1, dd, s3, 0], 0x01E0_15B8);
        node(fx, 0, sp + 0x50, q0, q1, 0x01E0_15D0);
        at_node(fx, 5.0);
        spawn_task(fx, B484_SPRITE, [q0, q1, dd, s3, 0], 0x01E0_1608);
        (s0, s1) = (q0, q1);
    }
    if matches!(c(fx), 0x6C | 0x78 | 0x83 | 0x8C) {
        node(fx, 0, sp + 0x50, s0, s1, 0x01E0_1654);
        at_node(fx, 10.0);
        for i in 0..0xFu32 {
            spawn_task(fx, B484_PUSH, [u64::from(i), s1, dd, s3, 0], 0x01E0_1690);
        }
    }
    fx.leave(0x60);
}

/// 0x1E016C0 (D, task): a pink quad ring (size 17, (0x80, 0x40, 0x80),
/// cell 0..3, angle y 0..359, radius from 0 growing by 30..79, row 0..9,
/// life 5..14; 0x80 during the life, out by 0xF).
fn ring4(fx: &mut Fx, t: u32) {
    let at = B484_RING;
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x11);
        let v = fx.rand_from(t, at + 0x28);
        fx.mem.set16(t + 0x34, v % 4);
        l2::rgb(fx, t + 0x28, 0x80, 0x40, 0x80);
        fx.mem.set32(t + 0x18, 0);
        let v = rmod(fx, t, at + 0x5C, 0x168);
        fx.mem.set32(t + 0x14, v);
        fx.mem.set16(t + 0x30, 0);
        let v = rmod(fx, t, at + 0x7C, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rmod(fx, t, at + 0x98, 0x32);
        fx.mem.set16(t + 0xE, v + 0x1E);
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

/// 0x1E01830 (D, task): a grey pushed sprite (size 100, +0x30 / +0x3A rand
/// % 360, drift rand % 4 - 2 then pushed each update by 30 sin / 30 cos of
/// +0x3A into +8 / +0xC); its cell rand % 8; 0x80 for 7 updates, out by
/// 0xF. Frame 0x30.
fn sprite_push(fx: &mut Fx, t: u32) {
    let at = B484_PUSH;
    fx.enter(0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x64);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        sprite_first(fx, t, at, 0x48, 0x168);
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
    let v = fx.rand_from(t, at + 0x128);
    fx.mem.set16(t + 0x34, v % 8);
    for (o, cos, ra) in [(8u32, 0, 0x174u32), (0xC, 1, 0x1AC)] {
        let deg = fx.mem.i16(t + 0x3A);
        let s = l2::sin_deg(fx, SINES_B, deg, cos);
        let v = ee::add(ee::from_i32(fx.mem.i16(t + o)), ee::mul(30.0, s));
        fx.fix_residue(v, at + ra);
        fx.mem.set16(t + o, ee::fixsfsi(v));
    }
    age(fx, t);
    fx.leave(0x30);
}

// ---- b477 (card 431): the family's init and pool with its own steps ----

const B477_INIT: Init = Init { shift: -0xFB8, timer: 0x91, nodes: &[0xE, 0xD, 0x17, 0x18, 0x22, 0x2C, 0x36, 0x37], table0: 0x1E, facing: 0xA0, tex5: 3, zero_b4: false };
const B477_RING: u32 = 0x01E0_18B0;
const B477_DISC: u32 = 0x01E0_1D50;
const B477_TICK: u32 = 0x01E0_1E30;

/// BMODULE b477 (card 431; entry 0x1E022B0, frame 0x170): b046's init
/// (timer 0x91, nodes 0xE / 0xD / 0x17 / 0x18 / 0x22 / 0x2C / 0x36 /
/// 0x37, the first table rand % 30 - 15 not cleared from 0xB4, three
/// group-5 textures) and pool. To update 0x88 the spawn point at node
/// 0x2C (from 0x44, 0x50..0x64 node 0x22, kind 1); on 0x4E / 0x54 / 0x5A
/// at node 0xE pushed away from node 0xD by their distance: eight b527
/// grey quad rings (0x1E018B0), a disc (`spawn::disc_step`) and three red
/// tick rings of each kind (0x1E01E30 `task40::band_grow`, drawn by
/// `quads4::ring_ticks` on the side of the kind). Returns 1 on update
/// 0x44, 4 on 0x67, 2 on 0x5D, 6 on 0x4E..0x59, 8 once the timer has run
/// out. The steps 0x1E01A30 (b417's spark), 0x1E01C40 (b148's sprite),
/// 0x1E01F70 (`task40::camera`), 0x1E020F0 (the stage light) and the
/// object op 0x1E021B0 are tested but never spawned; not ported.
pub fn program_b477(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x170);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x361C, OT);
    let ret = if param >= 0 { init(fx, &B477_INIT, sp, param) } else { step_b477(fx, sp, param) };
    fx.leave(0x170);
    ret
}

fn step_b477(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit_b477(fx, param);
    let low = fx.sp;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x24);
        let ra: u32 = match f {
            B477_RING => {
                ring_by(fx, B477_RING, (0x14, 2, 0x46), t);
                0x01E0_2738
            }
            B477_DISC => {
                crate::blib::spawn::disc_step(fx, B477_DISC, t);
                0x01E0_2798
            }
            B477_TICK => {
                task40::band_grow(fx, B477_TICK, D, t);
                0x01E0_27B8
            }
            _ => {
                debug_assert!(false, "b477: unported task step {f:#x}");
                continue;
            }
        };
        fx.saved(low - 0x20, &[(0x10, u64::from(ra)), (0, u64::from(D))]);
    }
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x50, u64::from(D), 0x01E0_286C);
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
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x24);
        if f(fx) == B477_RING {
            set_rect(fx, D + 0x35F8, sp + 0x60, 0x14);
            fx.mem.set16(D + 0xB8, 0xA);
            for k in 0..3 {
                let v = ee::from_i32(fx.mem.i16(t + 2 * k));
                fx.mem.setf(D + 0x20 + 4 * k, v);
            }
            quads4::ring_spun(fx, uv::uv_rect, D, t, 0, TABLE);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == B477_DISC {
            set_rect(fx, D + 0x2EC, sp + 0x80, 0x9B);
            quads4::fan18(fx, D, t, sp + 0xD8);
        }
        if f(fx) == B477_TICK {
            set_rect(fx, D + 0x2EC, sp + 0x90, 0x97);
            fx.mem.set16(D + 0xB8, 1);
            let side = fx.mem.i16(t + 0x30);
            quads4::ring_ticks(fx, D, t, side);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x44 => 1,
        0x67 => 4,
        0x5D => 2,
        0x4E..=0x59 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// b477's emitter 0x1E01600(D, 0) (frame 0x70: ra, the entry s2 = param,
/// s1 = 0, s0 = D; its s2 sp + 0x60 from the push, s1 the loops' counts).
fn emit_b477(fx: &mut Fx, param: i32) {
    let p = param as i64 as u64;
    let sp = fx.enter(0x70);
    let dd = u64::from(D);
    fx.saved(sp, &[(0x30, 0x01E0_2704), (0x20, p), (0x10, 0), (0, dd)]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let node = |fx: &mut Fx, k: u32, out: u32, ra: u32| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2 * k);
        node_position_saving(fx, h, slot, out, dd, Some(0), ra);
    };
    let pos = |fx: &mut Fx| {
        for k in 0..3 {
            let w = fx.mem.read_u32(sp + 0x40 + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
    };
    if c(fx) < 0x89 {
        node(fx, 5, sp + 0x40, 0x01E0_165C);
        pos(fx);
    }
    if c(fx) == 0x44 {
        node(fx, 4, sp + 0x40, 0x01E0_169C);
        pos(fx);
        fx.mem.set16(KIND, 1);
    }
    if (0x50..0x65).contains(&c(fx)) {
        node(fx, 4, sp + 0x40, 0x01E0_16F0);
        pos(fx);
        fx.mem.set16(KIND, 1);
    }
    if matches!(c(fx), 0x4E | 0x54 | 0x5A) {
        node(fx, 0, sp + 0x50, 0x01E0_1774);
        node(fx, 1, sp + 0x60, 0x01E0_1790);
        for k in 0..3 {
            let v = ee::sub(fx.mem.f(sp + 0x50 + 4 * k), fx.mem.f(sp + 0x60 + 4 * k));
            fx.mem.setf(sp + 0x60 + 4 * k, v);
        }
        for k in 0..3 {
            let v = ee::add(fx.mem.f(sp + 0x50 + 4 * k), fx.mem.f(sp + 0x60 + 4 * k));
            fx.mem.setf(POS + 4 * k, v);
        }
        let s2 = u64::from(sp + 0x60);
        for i in 0..8u32 {
            spawn_task(fx, B477_RING, [dd, u64::from(i), s2, 0, 0], 0x01E0_1818);
        }
        spawn_task(fx, B477_DISC, [dd, 8, s2, 0, 0], 0x01E0_183C);
        for (kind, ra) in [(0, 0x01E0_1858u32), (1, 0x01E0_1888)] {
            fx.mem.set16(KIND, kind);
            for i in 0..3u32 {
                spawn_task(fx, B477_TICK, [dd, u64::from(i), s2, 0, 0], ra);
            }
        }
    }
    fx.leave(0x70);
}
