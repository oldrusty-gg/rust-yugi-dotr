//! BMODULE b008 (card 101), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01880, data area 0x1ED0000) [F]: b086's older task pool: 150 tasks of 0x40 bytes at D+0x65C
//! (alive +0x36, step +0x24 as a record address) drawn as rings of
//! camera-facing quads (07c172c43e56 at 0x1E004D0, `quads4::ring_facing_i`,
//! UVs by uv_rect4 0x1E00080; spawn d4853f1f1fc1 at 0x1E01EB0, b086's
//! 0x1E01730). On updates 61, 71 .. 91 (while the 290-update timer +0x3950
//! has 63 / 66 or more left) two pale columns (0x1E012A0) and two dark
//! rings (0x1E00FB0) leave the attacker's node 1; each column drifts
//! toward the target in z and, on its ages 91..150, drops a red puff
//! (0x1E01660, rising by 3) 60 above itself every update. Returns 1 on
//! update 90, 3 when the timer reads 50, 6 on 101..259, 8 once the timer
//! has run out.
use crate::blib::{quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x24), as the record's addresses.
const SMOKE: u32 = 0x01E0_0C30;
const DARK: u32 = 0x01E0_0FB0;
const COLUMN: u32 = 0x01E0_12A0;
const PUFF: u32 = 0x01E0_1660;
const TIMER: u32 = D + 0x3950;
const COLOUR: u32 = D + 0x3954;
const COUNTER: u32 = D + 0x39B6;
const BYTES: u32 = D + 0x39BA;

/// The entry 0x1E01880.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    fx.mem.write_u32(D + 0x395C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0xA0);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3D30);
    l2::sine_table(fx, D + 0x3BBC);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x39B4, param);
    fx.mem.set32(TIMER, 0x122);
    for k in 0..3 {
        fx.mem.write_u32(D + 0x310 + 4 * k, 0);
    }
    for (k, node) in [0xF, 1, 0x1B, 0x26, 0xF, 0xA, 0x3D].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(D + 0x3998 + 2 * k as u32, r);
    }
    for k in 0xB..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x34C + 4 * (k as u32 - 0xB), t);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x328, t);
    let t = fx.texture(5, 0);
    fx.mem.write_u32(D + 0x3930, t);
    for k in 0x64..0xB4u32 {
        let v = rand_mod(fx, 0x14);
        fx.mem.set8(BYTES + k, v - 0xA);
    }
    for k in 0..0x64u32 {
        let v = rand_mod(fx, 0x50);
        fx.mem.set8(BYTES + k, v - 0x28);
    }
    for k in 0xB4..0x100u32 {
        let v = fx.rand();
        fx.mem.set8(BYTES + k, v % 2 + 1);
    }
    for k in 0..0x100u32 {
        let v = rand_mod(fx, 0x14);
        fx.mem.set8(BYTES + k, v - 0xA);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let slot = D + 0x40 * k;
        if fx.mem.i16(slot + 0x692) == 0 {
            continue;
        }
        let t = slot + 0x65C;
        if fx.mem.read_u32(slot + 0x680) == SMOKE {
            smoke(fx, t);
        }
        if fx.mem.read_u32(slot + 0x680) == DARK {
            dark(fx, t);
        }
        if fx.mem.read_u32(slot + 0x680) == COLUMN {
            column(fx, t, slot, k);
        }
        if fx.mem.read_u32(slot + 0x680) == PUFF {
            puff(fx, t);
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    // 0x1F71A0's frame below the entry's (its saved ra and the entry's s0
    // = D).
    fx.saved(sp - 0x20, &[(0x10, 0x01E0_1B94), (0, u64::from(D))]);
    let y = ee::sub(fx.mem.f(sp + 0x58), 25.0);
    fx.mem.setf(sp + 0x58, y);
    for k in 0..TASKS {
        let t = D + 0x40 * k;
        if fx.mem.i16(t + 0x692) == 0 {
            continue;
        }
        for (f, local, count, depth, table) in [(SMOKE, 0x60u32, 4, 0, BYTES), (DARK, 0x70, 2, 1, BYTES + 0x100), (COLUMN, 0x80, 8, 0, BYTES), (PUFF, 0x90, 4, 0, BYTES)] {
            if fx.mem.read_u32(t + 0x680) != f {
                continue;
            }
            let tex = fx.mem.read_u32(D + 0x3930);
            fx.mem.write_u32(D + 0x88, tex);
            let sub = fx.mem.i16(t + 0x690);
            l2::rect(fx, sp + local, 0x14, sub);
            for k in 0..4 {
                let v = fx.mem.read_u32(sp + local + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, v);
            }
            fx.mem.set16(D + 0xB8, count);
            let v = fx.rand();
            fx.mem.set16(D + 0xBE, v % 4);
            quads4::ring_facing_i(fx, uv::uv_rect4, D, t + 0x65C, depth, table);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    let c = fx.mem.i16(COUNTER);
    let left = fx.mem.i32(TIMER);
    if c == 0x5A {
        1
    } else if left == 0x32 {
        3
    } else if (0x65..0x104).contains(&c) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}

/// 0x1E00A50(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the emitters: on updates 61, 71, .. 91 (counter % 10 = 1, timer at
/// least 63 / 66) at node 1 two columns, then two dark rings (+0x30 = 0,
/// 1) (frame 0x50: ra, the entry's s1 = 0 / s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, 0x01E0_1ADC), (0x10, 0), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    for (f, min, ras) in [(COLUMN, 0x3F, [0x01E0_0B30u64, 0x01E0_0B4C]), (DARK, 0x42, [0x01E0_0BFC, 0x01E0_0C18])] {
        let c = fx.mem.i16(COUNTER);
        if c < 0x36 || fx.mem.i32(TIMER) < min {
            continue;
        }
        if fx.div(c, 0xA).1 != 1 || c >= 0x65 {
            continue;
        }
        for (slot, out) in [(0x3998u32, 0x30u32), (0x399A, 0x40)] {
            let h = fx.self_handle();
            let s = fx.mem.i16(D + slot);
            fx.node_position(h, s, sp + out);
        }
        for k in 0..3 {
            let w = fx.mem.read_u32(sp + 0x40 + 4 * k);
            fx.mem.write_u32(D + 0x2D0 + 4 * k, w);
        }
        for (k, ra) in ras.into_iter().enumerate() {
            fx.mem.set16(D + 0x39B0, k as i32);
            spawn(fx, f, 0, &[(0x50, ra), (0x20, param as i64 as u64), (0x10, u64::from(sp + 0x40)), (0, u64::from(D))]);
        }
    }
    fx.leave(0x50);
}

/// 0x1E01EB0(D, step, arg): the first free task cleared and set off at
/// D+0x2D0 (as i16) with the colour D+0x3954 (twice), +0x30 = D+0x39B0,
/// +0x3A = D+0x39AA; 1 when one was free (d4853f1f1fc1, b086's 0x1E01730
/// [F]). `saves`: its prologue's.
fn spawn(fx: &mut Fx, f: u32, arg: u32, saves: &[(u32, u64)]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x40 * k + 0x692) == 0) {
        Some(k) => {
            let t = D + 0x65C + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            for c in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x2D0, 0);
            let a = fx.mem.i16(D + 0x39B0);
            fx.mem.set16(t + 0x30, a);
            let a = fx.mem.i16(D + 0x39AA);
            fx.mem.set16(t + 0x3A, a);
            fx.mem.write_u32(t + 0x20, arg);
            fx.mem.write_u32(t + 0x24, f);
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

/// A first step's colour: `grey` when the spawn's +0x30 is 0, else `red`
/// (r, 0x10 / 0x40, ..); +0x30 then 0.
fn first_colour(fx: &mut Fx, t: u32, grey: (i32, i32, i32), red: (i32, i32, i32)) {
    let (r, g, b) = if fx.mem.i16(t + 0x30) == 0 { grey } else { red };
    l2::rgb(fx, t + 0x28, r, g, b);
    fx.mem.set16(t + 0x30, 0);
}

/// Alpha +0x2B up by `by` below `below` while the age is under `life`,
/// else down by 9 (the task ends below 11).
fn fade(fx: &mut Fx, t: u32, life: i32, below: u8, by: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < life {
        if a < below {
            fx.mem.write_u8(t + 0x2B, a + by);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x2B, a - 9);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// The target's and the attacker's positions at `sp + o` / `sp + o +
/// 0x10`; z of the first = the gap / `q` (`None`: the gap only) back.
fn gap_z(fx: &mut Fx, sp: u32, o: u32, q: Option<f32>) -> f32 {
    let h = fx.target_handle();
    fx.unit_position(h, sp + o);
    let h = fx.self_handle();
    fx.unit_position(h, sp + o + 0x10);
    let dz = ee::sub(fx.mem.f(sp + o + 8), fx.mem.f(sp + o + 0x18));
    fx.mem.setf(sp + o + 8, dz);
    match q {
        Some(q) => {
            let dz = ee::div(fx.mem.f(sp + o + 8), q);
            fx.mem.setf(sp + o + 8, dz);
            dz
        }
        None => dz,
    }
}

/// z (i16 +4) += dz.
fn move_z(fx: &mut Fx, t: u32, dz: f32) {
    let z = ee::add(ee::from_i32(fx.mem.i16(t + 4)), dz);
    fx.mem.set16(t + 4, ee::fixsfsi(z));
}

/// Above the attacker's height - 15 (`y` the attacker's y): true.
fn above_floor(fx: &mut Fx, t: u32, y: u32) -> bool {
    let floor = ee::sub(fx.mem.f(y), 15.0);
    !(ee::from_i32(fx.mem.i16(t + 2)) <= floor)
}

fn odd_age(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x32) % 2 == 1
}

/// The cell frame +0x34 up to 5 on odd ages.
fn cell_step(fx: &mut Fx, t: u32) {
    if odd_age(fx, t) && fx.mem.i16(t + 0x34) < 5 {
        let c = fx.mem.i16(t + 0x34) + 1;
        fx.mem.set16(t + 0x34, c);
    }
}

/// Three random angles (+0x10, +0x14, +0x18), the age counted.
fn angles_age(fx: &mut Fx, t: u32) {
    for o in [0x10u32, 0x14, 0x18] {
        let v = rand_mod(fx, 0x168);
        fx.mem.set32(t + o, v);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// The first step's ring: a random angle +0x3A, radius growth +0xE = rand % `gm` + `go`, table row +8 0..24.
fn ring_start(fx: &mut Fx, t: u32, gm: i32, go: i32) {
    let v = rand_mod(fx, 0x168);
    fx.mem.set16(t + 0x3A, v);
    let v = rand_mod(fx, gm);
    fx.mem.set16(t + 0xE, v + go);
    let v = rand_mod(fx, 0x19);
    fx.mem.set16(t + 8, v);
}

/// 0x1E00C30(D, task): a grey (or red) smoke ring: size 30, growth 7..13,
/// life 45..54; fades in by 0x2A, z by the gap / 40 sinking to 15 under
/// the attacker for 40 updates, then / 50 with the radius by 5 x growth
/// and 2 up; x += +0xC on odd ages; size += 3.
fn smoke(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x10, 0x01E0_1B10), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x1E);
        fx.mem.set16(t + 0x34, 0);
        first_colour(fx, t, (0x40, 0x40, 0x40), (0x50, 0x10, 0x10));
        ring_start(fx, t, 7, 7);
        let v = rand_mod(fx, 0xA);
        fx.mem.set16(t + 6, v + 0x2D);
        fx.mem.set16(t + 0x36, 2);
    }
    let life = fx.mem.i16(t + 6);
    fade(fx, t, life, 0x80, 0x2A);
    if fx.mem.i16(t + 0x32) < 0x28 {
        let dz = gap_z(fx, sp, 0x20, Some(40.0));
        move_z(fx, t, dz);
        if above_floor(fx, t, sp + 0x34) {
            let y = fx.mem.i16(t + 2) - 1;
            fx.mem.set16(t + 2, y);
        }
    } else {
        let dz = gap_z(fx, sp, 0x20, Some(50.0));
        move_z(fx, t, dz);
        let r = fx.mem.i16(t + 0x30) + 5 * fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
        let y = fx.mem.i16(t + 2) + 2;
        fx.mem.set16(t + 2, y);
    }
    if odd_age(fx, t) {
        let x = fx.mem.i16(t) + fx.mem.i16(t + 0xC);
        fx.mem.set16(t, x);
    }
    cell_step(fx, t);
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let s = fx.mem.i16(t + 0x38) + 3;
        fx.mem.set16(t + 0x38, s);
    }
    angles_age(fx, t);
    fx.leave(0x50);
}

/// 0x1E00FB0(D, task): a dark ring (far Z): size 40, growth 5..9, life
/// 2..5; fades in by 0x20 below 0x41; z by the gap / 63, sinking by 1 on
/// odd ages to 15 under the attacker; size += 1.
fn dark(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x10, 0x01E0_1B30), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x28);
        fx.mem.set16(t + 0x34, 0);
        first_colour(fx, t, (0x30, 0x30, 0x30), (0x40, 0x10, 0x10));
        ring_start(fx, t, 5, 5);
        let v = fx.rand();
        fx.mem.set16(t + 6, v % 4 + 2);
        fx.mem.set16(t + 0x36, 2);
    }
    let life = fx.mem.i16(t + 6);
    fade(fx, t, life, 0x41, 0x20);
    let dz = gap_z(fx, sp, 0x20, None);
    let dz = ee::div(dz, 63.0);
    fx.mem.setf(sp + 0x28, dz);
    move_z(fx, t, dz);
    if above_floor(fx, t, sp + 0x34) && odd_age(fx, t) {
        let y = fx.mem.i16(t + 2) - 1;
        fx.mem.set16(t + 2, y);
    }
    cell_step(fx, t);
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    let s = fx.mem.i16(t + 0x38) + 1;
    fx.mem.set16(t + 0x38, s);
    angles_age(fx, t);
    fx.leave(0x50);
}

/// 0x1E012A0(D, task): a pale column (white, or pink): size 30, growth
/// 7..13, life +6 655..1854 (handed to its puffs); fades in by 0x2A for
/// 160 updates; for 90 updates z by the gap / 80, sinking by 1 on odd ages
/// to 15 under the attacker; after 130 it shrinks by 3. On ages 91..150 a
/// puff starts 60 above it. Radius += growth to age 129, size += 3 to 119.
/// `slot` / `k` the entry's loop registers (s2 / s3).
fn column(fx: &mut Fx, t: u32, slot: u32, k: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x20, 0x01E0_1B50), (0x10, 0), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x1E);
        fx.mem.set16(t + 0x34, 0);
        first_colour(fx, t, (0x80, 0x80, 0x80), (0x80, 0x40, 0x40));
        ring_start(fx, t, 7, 7);
        let v = rand_mod(fx, 0x4B0);
        fx.mem.set16(t + 6, v + 0x28F);
        fx.mem.set16(t + 0x36, 2);
    }
    fade(fx, t, 0xA0, 0x80, 0x2A);
    let age = fx.mem.i16(t + 0x32);
    let dz = gap_z(fx, sp, 0x30, None);
    if age < 0x5A {
        let dz = ee::div(dz, 80.0);
        fx.mem.setf(sp + 0x38, dz);
        move_z(fx, t, dz);
        if above_floor(fx, t, sp + 0x44) && odd_age(fx, t) {
            let y = fx.mem.i16(t + 2) - 1;
            fx.mem.set16(t + 2, y);
        }
    } else if age >= 0x83 {
        let s = fx.mem.i16(t + 0x38) - 3;
        fx.mem.set16(t + 0x38, s);
    }
    if (0x5B..0x97).contains(&fx.mem.i16(t + 0x32)) {
        let x = ee::from_i32(fx.mem.i16(t));
        fx.mem.setf(D + 0x2D0, x);
        let y = ee::from_i32(fx.mem.i16(t + 2) + 0x3C);
        fx.mem.setf(D + 0x2D4, y);
        let z = ee::from_i32(fx.mem.i16(t + 4));
        fx.mem.setf(D + 0x2D8, z);
        let life = fx.mem.i16(t + 6);
        fx.mem.set16(D + 0x39B0, life);
        spawn(fx, PUFF, 0, &[(0x50, 0x01E0_155C), (0x30, u64::from(k)), (0x20, u64::from(slot)), (0x10, u64::from(D)), (0, u64::from(t))]);
    }
    cell_step(fx, t);
    let age = fx.mem.i16(t + 0x32);
    if age < 0x82 && age != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    let age = fx.mem.i16(t + 0x32);
    if age < 0x78 && age != 0 {
        let s = fx.mem.i16(t + 0x38) + 3;
        fx.mem.set16(t + 0x38, s);
    }
    angles_age(fx, t);
    fx.leave(0x60);
}

/// 0x1E01660(D, task): a red puff: size 40 growing by 1, growth 7..13,
/// life 55..64; fades in by 13 below 0x60 for 15 updates, then out by 9;
/// rises by 3 a update.
fn puff(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x10, 0x01E0_1B70), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x28);
        l2::rgb(fx, t + 0x28, 0xC0, 0x40, 0x40);
        fx.mem.set16(t + 0x34, 0);
        ring_start(fx, t, 7, 7);
        let v = rand_mod(fx, 0xA);
        fx.mem.set16(t + 6, v + 0x37);
        fx.mem.set16(t + 0x36, 2);
    }
    fade(fx, t, 0xF, 0x60, 0xD);
    gap_z(fx, sp, 0x20, None);
    let y = fx.mem.i16(t + 2) + 3;
    fx.mem.set16(t + 2, y);
    cell_step(fx, t);
    if fx.mem.i16(t + 0x32) != 0 {
        let s = fx.mem.i16(t + 0x38) + 1;
        fx.mem.set16(t + 0x38, s);
    }
    angles_age(fx, t);
    fx.leave(0x50);
}
