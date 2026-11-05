//! BMODULE b555 (card 129), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02E90, data area 0x1ED0000) [F] (helper C9, 31
//! Oct 2026): b003's older task pool (200 tasks of 0x40 bytes at +0x2C0,
//! alive +0x36, step +0x24, each step called by its own `jal`) drawn with
//! the second library. The emitter 0x1E01BE0 (`emit`): on update 1 the
//! stage light (0x1E02BA0), a flickering camera light at the attacker's
//! node 0x25 (0x1E029E0), a whole-window unit flash (0x1E02CB0, the unit
//! drawn into the target in a packet layer, 0x1E03E20) and a blinking pair
//! of sprites 0.55 of the way from node 0x23 to node 0x25 (0x1E01DC0); on
//! 0x33..0xC7 a grey ring (0x1E01F90, `ring_facing_i`) and a ring of quads
//! (0x1E022B0, `ring_at_m`) a update at node 0x26; on 0x22..0x2E two dark
//! rings (0x1E02830, `ring_facing_m`) there. Returns 1 on update 0x5A, 3
//! on 0x127, 6 on 0x65..0x121, 8 once the 320-update timer +0x351C has
//! run out.
//!
//! The steps 0x1E01ED0 (inlined in the entry), 0x1E02650 and 0x1E02C60
//! (an object op through 0x1F8380) are tested by the entry but never
//! spawned [F: the emitter's spawns name 0x1E02BA0 / 0x1E029E0 / 0x1E02CB0
//! / 0x1E01DC0 / 0x1E01F90 / 0x1E022B0 / 0x1E02830 only]; not ported.
//!
//! Library at its own addresses: uv_rect (0x1E00080), `ring_facing_i`
//! (0x1E00920), `sprite_turned_at` (0x1E00EA0), `ring_at_m` (0x1E01170),
//! `ring_facing_m` (0x1E016D0).
use crate::blib::{quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0xC8;
const POOL: u32 = D + 0x2C0;
/// The step functions a task holds (+0x24), as the record's addresses.
const BLINK: u32 = 0x01E0_1DC0;
const RING_I: u32 = 0x01E0_1F90;
const RING_M: u32 = 0x01E0_22B0;
const DARK: u32 = 0x01E0_2830;
const LIGHT: u32 = 0x01E0_29E0;
const STAGE: u32 = 0x01E0_2BA0;
const FLASH: u32 = 0x01E0_2CB0;
const BYTES: u32 = D + 0x3564;
const BYTES2: u32 = D + 0x3664;
const CAMERA: u32 = D + 0x34E0;
const LAYER: u32 = D + 0x3510;
const FLASH_DESC: u32 = D + 0x3514;
const TIMER: u32 = D + 0x351C;
const COLOUR: u32 = D + 0x3520;
const NODES: u32 = D + 0x3548;
const INDEX: u32 = D + 0x355C;
const YAW: u32 = D + 0x3560;
const COUNTER: u32 = D + 0x3562;

/// The entry 0x1E02E90.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x150);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x3528, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x150);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x38D0);
    l2::sine_table(fx, D + 0x3764);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x355E, param);
    fx.mem.set32(TIMER, 0x140);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x130);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0x134)), 180.0);
    fx.fix_residue(v, 0x01E0_2F40);
    fx.mem.set16(YAW, ee::fixsfsi(v));
    for (k, n) in [0x26, 0x25, 0x23, 0x47, 0x4E].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    for id in 0xAu32..0x10 {
        let tex = fx.texture(4, id as i32);
        fx.mem.write_u32(D + 0x2A8 + 4 * (id - 0xA), tex);
    }
    let tex = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x288, tex);
    for k in 0..4u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x34C0 + 4 * k, tex);
    }
    // rand's frame: s0 = the loop counter.
    for (table, m, ra) in [(BYTES, 0x14, 0x01E0_3028u32), (BYTES2, 0x16, 0x01E0_307C)] {
        for k in 0..0x100u32 {
            let r = fx.rand_from(k, ra);
            fx.mem.write_u8(table + k, (r.wrapping_rem(m) - m / 2) as u8);
        }
        for k in 0xB4..0x100u32 {
            fx.mem.write_u8(table + k, 0);
        }
    }
    for k in 0x64..0xB4u32 {
        let r = fx.rand_from(k, 0x01E0_30D0);
        fx.mem.write_u8(BYTES2 + k, (r.wrapping_rem(8) - 4) as u8);
    }
    for k in 0..8u32 {
        let r = fx.rand_from(k, 0x01E0_310C);
        fx.mem.set16(D + 0xC0 + 2 * k, r.wrapping_rem(4));
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    // s1 counts the flash tasks (run by the unit flash); s0 = the task's
    // pool base (task - 0x2C0).
    let mut flashes = 0u32;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let saves = [u64::from(t - 0x2C0), u64::from(flashes)];
        let f = fx.mem.read_u32(t + 0x24);
        match f {
            BLINK => blink(fx, t, saves[0]),
            RING_I => ring_i(fx, t, saves),
            RING_M => ring_m(fx, t, saves),
            DARK => dark(fx, t, saves[0]),
            LIGHT => light(fx, t, saves),
            STAGE => stage(fx, t),
            FLASH => flashes += 1,
            _ => debug_assert!(!matches!(f, 0x01E0_1ED0 | 0x01E0_2650 | 0x01E0_2C60)),
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0xA0);
    // s0 = the task's index (rand's saved s0).
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x24);
        if f == BLINK {
            let tex = fx.mem.read_u32(D + 0x34C8);
            fx.mem.write_u32(D + 0x88, tex);
            cell(fx, t, sp + 0xD0);
            let r = fx.rand_from(k, 0x01E0_3404);
            fx.mem.set16(D + 0xBE, r.wrapping_rem(4));
            for right in [true, false] {
                between(fx, sp, [u64::from(flashes), u64::from(t - 0x2C0)]);
                let x = fx.mem.f(sp + 0xA0);
                let x = if right { ee::add(x, 0.5) } else { ee::sub(x, 0.5) };
                fx.mem.setf(sp + 0xA0, x);
                quads4::sprite_turned_at(fx, uv::uv_rect, D, t, 1, sp + 0xA0);
            }
        }
        if f == RING_I {
            let tex = fx.mem.read_u32(D + 0x34C4);
            fx.mem.write_u32(D + 0x88, tex);
            cell(fx, t, sp + 0xF0);
            let rows = fx.mem.i16(t + 0xC);
            fx.mem.set16(D + 0xB8, rows);
            let r = fx.rand_from(k, 0x01E0_39D0);
            fx.mem.set16(D + 0xBE, r.wrapping_rem(4));
            quads4::ring_facing_i(fx, uv::uv_rect, D, t, 0, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f == RING_M {
            let tex = fx.mem.read_u32(D + 0x34C0);
            fx.mem.write_u32(D + 0x88, tex);
            centre(fx, t);
            cell(fx, t, sp + 0x100);
            fx.mem.set16(D + 0xB8, 2);
            let r = fx.rand_from(k, 0x01E0_3A9C);
            fx.mem.set16(D + 0xBE, r.wrapping_rem(4));
            centre(fx, t);
            quads4::ring_at_m(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f == DARK {
            let tex = fx.mem.read_u32(D + 0x34C4);
            fx.mem.write_u32(D + 0x88, tex);
            cell(fx, t, sp + 0x120);
            fx.mem.set16(D + 0xB8, 3);
            let r = fx.rand_from(k, 0x01E0_3C00);
            fx.mem.set16(D + 0xBE, r.wrapping_rem(4));
            quads4::ring_facing_m(fx, uv::uv_rect, D, t, 0x10, BYTES2);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    if flashes != 0 {
        unit_flash(fx, 0);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x5A => 1,
        0x127 => 3,
        0x65..=0x121 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// sp + 0xA0 = 0.55 of the way from the attacker's node 0x23 to node 0x25
/// (sp + 0xB0): each axis a + (float) (0.55 x (double) (b - a)) through
/// the soft-float 0x1A2C30 / 0x1A31D8 / 0x1A38F8 (exact product, one
/// rounding). `s13` the entry's s1 / s3 (the flash count, the task's pool
/// base) [run: their stack words are a later missing texture's TEX0 /
/// TEX1, store-watched].
fn between(fx: &mut Fx, sp: u32, s13: [u64; 2]) {
    let h = fx.self_handle();
    let s = fx.mem.i16(NODES + 4);
    fx.node_position(h, s, sp + 0xA0);
    let h = fx.self_handle();
    let s = fx.mem.i16(NODES + 2);
    fx.node_position(h, s, sp + 0xB0);
    let f = f64::from_bits(0x3FE1_9999_9999_999A);
    for c in 0..3u32 {
        let (a, b) = (fx.mem.f(sp + 0xA0 + 4 * c), fx.mem.f(sp + 0xB0 + 4 * c));
        let p = f * f64::from(ee::sub(b, a));
        soft_float_saved(fx, sp, p, s13);
        fx.mem.setf(sp + 0xA0 + 4 * c, ee::add(a, p as f32));
    }
}

/// What the soft-float chain leaves at the entry's sp - 0x38..-0x20
/// (`top` the entry's sp): 0x1A31D8's saves of the entry's s1 / s2 (D) /
/// s3 at its frame +0x78.., then 0x1A38F8's unpack (0x1A2E08) of the
/// product into its frame +0: for a normal number the exponent - 0x3FF
/// at +8 and the fraction << 8 | 1 << 60 at +0x10; a NaN's raw fraction
/// at +0x10; zero / denormal / infinity neither [F].
fn soft_float_saved(fx: &mut Fx, top: u32, p: f64, s13: [u64; 2]) {
    fx.mem.write_u64(top - 0x38, s13[0]);
    fx.mem.write_u64(top - 0x30, u64::from(D));
    fx.mem.write_u64(top - 0x28, s13[1]);
    let bits = p.to_bits();
    let exp = ((bits >> 52) & 0x7FF) as u32;
    let frac = bits & ((1 << 52) - 1);
    if exp == 0x7FF {
        if frac != 0 {
            fx.mem.write_u64(top - 0x30, frac);
        }
    } else if exp != 0 {
        fx.mem.write_u32(top - 0x38, exp.wrapping_sub(0x3FF));
        fx.mem.write_u64(top - 0x30, (frac << 8) | (1 << 60));
    }
}

/// The atlas cell 0x14 frame +0x34 (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32) {
    let frame = fx.mem.i16(t + 0x34);
    l2::rect(fx, r, 0x14, frame);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// The task's i16 position into D+0x20.
fn centre(fx: &mut Fx, t: u32) {
    for k in 0..3u32 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(D + 0x20 + 4 * k, v);
    }
}

/// 0x1E01BE0(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// +0x3562 and the spawns (frame 0x40: ra, s1 the entry's 0, s0 the
/// entry's parameter).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x20, 0x01E0_3148), (0x10, 0), (0, param as i64 as u64)]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    // The spawn's saves: s4 / s3 the runner's (0), the entry's s2 = D, the
    // emitter's s1 (its loop counter) and s0 = D.
    let regs = |s1: u64| [(0x40u32, 0u64), (0x30, 0), (0x20, u64::from(D)), (0x10, s1), (0, u64::from(D))];
    for (f, ra) in [(STAGE, 0x01E0_1C30u32), (LIGHT, 0x01E0_1C54), (FLASH, 0x01E0_1C78), (BLINK, 0x01E0_1C9C)] {
        if fx.mem.i16(COUNTER) == 1 {
            spawn(fx, f, &regs(0), ra);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x33..0xC8).contains(&c) {
        site(fx, sp);
        spawn(fx, RING_I, &regs(0), 0x01E0_1D00);
        spawn(fx, RING_M, &regs(0), 0x01E0_1D24);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x22..0x2F).contains(&c) {
        site(fx, sp);
        for k in 0..2u64 {
            spawn(fx, DARK, &regs(k), 0x01E0_1D94);
        }
    }
    fx.leave(0x40);
}

/// The spawn point D+0x250: the attacker's node 0x26.
fn site(fx: &mut Fx, sp: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i16(NODES);
    fx.node_position(h, s, sp + 0x30);
    for o in [0u32, 4, 8] {
        let w = fx.mem.read_u32(sp + 0x30 + o);
        fx.mem.write_u32(D + 0x250 + o, w);
    }
}

/// 0x1E03D00(D, step, 0): the first free task cleared and set off at
/// D+0x250 (as i16) with the colour +0x3520 (twice), +0x30 = +0x355C;
/// `saves` the caller's s4..s0, `ra` the return address.
fn spawn(fx: &mut Fx, f: u32, saves: &[(u32, u64)], ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
    fx.saved(sp, &[(0x50, u64::from(ra))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(POOL + 0x36 + 0x40 * k) == 0) {
        Some(k) => {
            let t = POOL + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            for c in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            let a = fx.mem.i16(INDEX);
            fx.mem.set16(t + 0x30, a);
            fx.mem.write_u32(t + 0x20, 0);
            fx.mem.write_u32(t + 0x24, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// rand % m (HI) from a step's rand returning to `ra` (s0 = the task).
fn rand_mod(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v, m).1
}

fn odd(v: i32) -> bool {
    v.wrapping_rem(2) == 1
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// The alpha +0x2B up by `up` while below `cap` while `rising`, else down
/// by `down` while at least `floor`, else the task ends.
fn fade(fx: &mut Fx, t: u32, rising: bool, up: u8, cap: u8, down: u8, floor: u8) {
    let v = fx.mem.read_u8(t + 0x2B);
    if rising {
        if v < cap {
            fx.mem.write_u8(t + 0x2B, v.wrapping_add(up));
        }
    } else if v >= floor {
        fx.mem.write_u8(t + 0x2B, v - down);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// 0x1E01DC0(D, t): the blinking pair (size 2, frame 4): fades in by 0x50
/// for 30 updates, out by 15; grey on odd updates, dark on even.
fn blink(fx: &mut Fx, t: u32, s0: u64) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, 0x01E0_317C), (0, s0)]);
    if fx.mem.i16(t + 0x36) == 1 {
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x38, 2);
        fx.mem.set16(t + 0x34, 4);
        fx.mem.set16(t + 0x36, 2);
    }
    let rising = fx.mem.i16(t + 0x32) < 0x1E;
    fade(fx, t, rising, 0x50, 0xA0, 0xF, 0x11);
    let c = if odd(fx.mem.i16(t + 0x32)) { 0x80 } else { 0x20 };
    l2::rgb(fx, t + 0x28, c, c, c);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E01F90(D, t): a dark ring of +0xC 1..3 quads (size +0x38 16..25
/// growing by 3 then 2 up to 100, radius +0x30 from 0 by +0xE 5..9, row
/// +8, life +6 40..74) about the node, 4 toward the facing; z closes on
/// the target by a 72nd (100th after 72 updates) of the gap; frames 3..5
/// every 8th update; fades in by 0x30, out by 9.
fn ring_i(fx: &mut Fx, t: u32, saves: [u64; 2]) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x20, 0x01E0_3260), (0x10, saves[1]), (0, saves[0])]);
    if fx.mem.i16(t + 0x36) == 1 {
        let v = rand_mod(fx, t, 0x01E0_1FBC, 0xA);
        fx.mem.set16(t + 0x38, v + 0x10);
        fx.mem.set16(t + 0x34, 3);
        l2::rgb(fx, t + 0x28, 0x20, 0x20, 8);
        fx.mem.set16(t + 0x30, 0);
        let v = rand_mod(fx, t, 0x01E0_1FF4, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rand_mod(fx, t, 0x01E0_2010, 5);
        fx.mem.set16(t + 0xE, v + 5);
        let v = rand_mod(fx, t, 0x01E0_2030, 0x19);
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, t, 0x01E0_204C, 0x23);
        fx.mem.set16(t + 6, v + 0x28);
        let v = rand_mod(fx, t, 0x01E0_206C, 3);
        fx.mem.set16(t + 0xC, v + 1);
        toward(fx, t);
        fx.mem.set16(t + 0x36, 2);
    }
    let rising = fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6);
    fade(fx, t, rising, 0x30, 0x61, 9, 0xB);
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x30);
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x40);
    let d = ee::sub(fx.mem.f(sp + 0x38), fx.mem.f(sp + 0x48));
    fx.mem.setf(sp + 0x38, d);
    let n = fx.mem.i16(t + 0x32);
    if n != 0 {
        let (by, ra) = if n < 0x48 { (72.0f32, 0x01E0_2198u32) } else { (100.0, 0x01E0_21D4) };
        let d = ee::div(d, by);
        fx.mem.setf(sp + 0x38, d);
        let v = ee::add(ee::from_i32(fx.mem.i16(t + 4)), d);
        fx.fix_residue(v, ra);
        fx.mem.set16(t + 4, ee::fixsfsi(v));
        if n >= 0x48 {
            let r = fx.mem.i16(t + 0x30) + 6 * fx.mem.i16(t + 0xE);
            fx.mem.set16(t + 0x30, r);
            let s = fx.mem.i16(t + 0x38) + 3;
            fx.mem.set16(t + 0x38, s);
        }
    }
    let n = fx.mem.i16(t + 0x32);
    if n >= 4 && n.wrapping_rem(8) == 1 {
        let f = fx.mem.i16(t + 0x34);
        fx.mem.set16(t + 0x34, if f < 5 { f + 1 } else { 0 });
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    if fx.mem.i16(t + 0x38) < 0x64 {
        let s = fx.mem.i16(t + 0x38) + 2;
        fx.mem.set16(t + 0x38, s);
    }
    age(fx, t);
    fx.leave(0x60);
}

/// z 4 toward the facing (+0x3560 set: -4).
fn toward(fx: &mut Fx, t: u32) {
    let z = fx.mem.i16(t + 4);
    let z = if fx.mem.i16(YAW) != 0 { z - 4 } else { z + 4 };
    fx.mem.set16(t + 4, z);
}

/// 0x1E022B0(D, t): a ring of quads (size +0x38 13..27 growing by 2 up to
/// 60 (90 on odd updates), frame rand % 8, grey 0x50, its index +0xA =
/// +0x30, radius from 0 by +0xE 2..6, row +8, life +6 35..69) at the node
/// (as i16), 4 toward the facing; z closes on the target by an 80th of
/// the gap; frames 1..5 every 7th update; fades in by 0x10, out by 7
/// before update 0x104, by 9 after; the angle +0x3A turns by +-1.
fn ring_m(fx: &mut Fx, t: u32, saves: [u64; 2]) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x20, 0x01E0_3280), (0x10, saves[1]), (0, saves[0])]);
    if fx.mem.i16(t + 0x36) == 1 {
        let v = rand_mod(fx, t, 0x01E0_22DC, 0xF);
        fx.mem.set16(t + 0x38, v + 0xD);
        let r = fx.rand_from(t, 0x01E0_22FC);
        fx.mem.set16(t + 0x34, r.wrapping_rem(8));
        l2::rgb(fx, t + 0x28, 0x50, 0x50, 0x50);
        let i = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, i);
        fx.mem.set16(t + 0x30, 0);
        let v = rand_mod(fx, t, 0x01E0_2338, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rand_mod(fx, t, 0x01E0_2354, 5);
        fx.mem.set16(t + 0xE, v + 2);
        let v = rand_mod(fx, t, 0x01E0_2374, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, t, 0x01E0_2390, 0x23);
        fx.mem.set16(t + 6, v + 0x23);
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES);
        fx.node_position(h, s, sp + 0x30);
        for (k, ra) in [(0u32, 0x01E0_23C8u32), (1, 0x01E0_23D4), (2, 0x01E0_23E0)] {
            let v = fx.mem.f(sp + 0x30 + 4 * k);
            fx.fix_residue(v, ra);
            fx.mem.set16(t + 2 * k, ee::fixsfsi(v));
        }
        toward(fx, t);
        fx.mem.set16(t + 0x36, 2);
    }
    let rising = fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6);
    let c = fx.mem.i16(COUNTER);
    if c < 0x104 {
        fade(fx, t, rising, 0x10, 0x80, 7, 9);
    }
    if fx.mem.i16(COUNTER) >= 0x105 {
        fade(fx, t, false, 0, 0, 9, 0xB);
    }
    if fx.mem.i16(t + 0x32) != 0 && fx.mem.i16(t + 0x38) < 0x3C {
        let s = fx.mem.i16(t + 0x38) + 2;
        fx.mem.set16(t + 0x38, s);
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x30);
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x40);
    if fx.mem.i16(t + 0x32) != 0 {
        let d = ee::sub(fx.mem.f(sp + 0x38), fx.mem.f(sp + 0x48));
        fx.mem.setf(sp + 0x38, d);
        let d = ee::div(d, 80.0);
        fx.mem.setf(sp + 0x38, d);
        let v = ee::add(ee::from_i32(fx.mem.i16(t + 4)), d);
        fx.fix_residue(v, 0x01E0_2554);
        fx.mem.set16(t + 4, ee::fixsfsi(v));
    }
    if fx.mem.i16(t + 0x32).wrapping_rem(7) == 1 {
        let f = fx.mem.i16(t + 0x34);
        fx.mem.set16(t + 0x34, if f < 5 { f + 1 } else { 0 });
    }
    let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x30, r);
    if fx.mem.i16(t + 0x38) < 0x5A && odd(fx.mem.i16(t + 0x32)) {
        let s = fx.mem.i16(t + 0x38) + 2;
        fx.mem.set16(t + 0x38, s);
    }
    let a = fx.mem.i16(t + 0x3A);
    let a = if odd(fx.mem.i16(t + 0xE)) { a + 1 } else { a - 1 };
    fx.mem.set16(t + 0x3A, a);
    age(fx, t);
    fx.leave(0x60);
}

/// 0x1E02830(D, t): a dark ring (size 3 growing by 1 to 20, frames 0..5
/// on odd updates, radius from 0 by +0xE 2..6, row +8, +0xC 3..5 quads,
/// life +6 3..7); fades in by 0x12, out by 9.
fn dark(fx: &mut Fx, t: u32, s0: u64) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, 0x01E0_32C0), (0, s0)]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 3);
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x20);
        fx.mem.set16(t + 0x30, 0);
        let v = rand_mod(fx, t, 0x01E0_2874, 5);
        fx.mem.set16(t + 0xE, v + 2);
        let v = rand_mod(fx, t, 0x01E0_2894, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, t, 0x01E0_28B0, 3);
        fx.mem.set16(t + 0xC, v + 3);
        let v = rand_mod(fx, t, 0x01E0_28D0, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x36, 2);
    }
    let rising = fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6);
    fade(fx, t, rising, 0x12, 0x81, 9, 0xB);
    if fx.mem.i16(t + 0x32) != 0 && fx.mem.i16(t + 0x38) < 0x14 {
        let s = fx.mem.i16(t + 0x38) + 1;
        fx.mem.set16(t + 0x38, s);
    }
    if odd(fx.mem.i16(t + 0x32)) && fx.mem.i16(t + 0x34) < 5 {
        let f = fx.mem.i16(t + 0x34) + 1;
        fx.mem.set16(t + 0x34, f);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E029E0(D, t): camera slots 4 / 5 (0x1F7E20) from +0x34E0 (240 /
/// 160 / 160 on odd updates, else 128 grey; the attacker's node 0x25 less
/// 4 in y; 0, 140, w rising by 0.2 to 1 for 200 updates, then down by
/// 0.1); cleared (0x1F7E20(0)) when the task ends (w at 0.1, or the timer
/// at 1).
fn light(fx: &mut Fx, t: u32, saves: [u64; 2]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x20, 0x01E0_32E0), (0x10, saves[1]), (0, saves[0])]);
    let h = fx.self_handle();
    let s = fx.mem.i16(NODES + 2);
    fx.node_position(h, s, sp + 0x30);
    let c = CAMERA;
    if fx.mem.i16(t + 0x36) == 1 {
        for (o, v) in [(0u32, 0u32), (4, 0x4180_0000), (8, 0x4180_0000), (0xC, 0x4180_0000), (0x20, 0), (0x24, 0x430C_0000), (0x28, 0)] {
            fx.mem.write_u32(c + o, v);
        }
        fx.mem.set16(t + 0x36, 2);
    }
    let x = fx.mem.read_u32(sp + 0x30);
    fx.mem.write_u32(c + 0x10, x);
    let y = ee::sub(fx.mem.f(sp + 0x34), 4.0);
    fx.mem.setf(c + 0x14, y);
    let z = fx.mem.read_u32(sp + 0x38);
    fx.mem.write_u32(c + 0x18, z);
    let w = fx.mem.f(c + 0x28);
    if fx.mem.i16(t + 0x32) < 0xC8 {
        if crate::exe::ee_lt(w, 1.0) {
            fx.mem.setf(c + 0x28, ee::add(w, f32::from_bits(0x3E4C_CCCD)));
        }
    } else {
        let tenth = f32::from_bits(0x3DCC_CCCD);
        if crate::exe::ee_le(w, tenth) {
            fx.mem.set16(t + 0x36, 0);
        } else {
            fx.mem.setf(c + 0x28, ee::sub(w, tenth));
        }
    }
    let (r, g) = if odd(fx.mem.i16(t + 0x32)) { (0x4370_0000u32, 0x4320_0000u32) } else { (0x4300_0000, 0x4300_0000) };
    fx.mem.write_u32(c + 4, r);
    fx.mem.write_u32(c + 8, g);
    fx.mem.write_u32(c + 0xC, g);
    age(fx, t);
    let (p, ra) = if fx.mem.i16(t + 0x36) != 0 { (c, 0x01E0_2B70) } else { (0, 0x01E0_2B5C) };
    crate::blib::exe::camera_slots45(fx, p);
    // 0x1F7E20's frame (s0 = the task, ra).
    fx.saved(sp - 0x20, &[(0, u64::from(t)), (0x10, ra)]);
    if fx.mem.i32(TIMER) == 1 {
        fx.mem.set16(t + 0x36, 0);
    }
    fx.leave(0x40);
}

/// 0x1E02BA0(D, t): the stage light +0x30 from 0x80: down by 10 to 0x30
/// while the timer is at least 0x1F, then up by 10 to 0x80 (0x1F7F10).
fn stage(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x10);
    fx.saved(sp, &[(0, 0x01E0_3300)]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x30, 0x80);
        fx.mem.set16(t + 0x36, 2);
    }
    let v = fx.mem.i16(t + 0x30);
    let v = if fx.mem.i32(TIMER) >= 0x1F {
        if v < 0x31 {
            0x30
        } else {
            v - 0xA
        }
    } else if v < 0x80 {
        v + 0xA
    } else {
        0x80
    };
    fx.mem.set16(t + 0x30, v);
    let b = fx.mem.read_u8(t + 0x30);
    fx.set_stage_light(i32::from(b));
    fx.leave(0x10);
}

/// 0x1E02CB0(D, t): the flash's frame sprite (the target over the window,
/// white, UVs inset by the size +8 over 512 both ways) into the
/// descriptor at +0x3514, no ordering table; alpha up by 10 below 0x80
/// for 0x1D updates, then down by 9. `regs` the unit flash's s0 / s1 (the
/// slot index, the live count).
fn flash(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_3EF4), (0x20, u64::from(regs[1])), (0x10, u64::from(regs[0]))]);
    // swc1 f20: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    let w = ee::from_i32(fx.mem.i16(t + 8));
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.write_u8(t + 0x2B, 0);
        l2::rgb(fx, t + 0x28, 0xFF, 0xFF, 0xFF);
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
    let lo = ee::div(w, 512.0);
    let hi = ee::div(ee::sub(512.0, w), 512.0);
    for (o, v) in [(0x18u32, lo), (0x1C, lo), (0x28, hi), (0x2C, hi)] {
        fx.mem.setf(d + o, v);
    }
    fx.submit(d, 0, 0);
    let rising = fx.mem.i16(t + 0x32) < 0x1D;
    fade(fx, t, rising, 0xA, 0x80, 9, 0xB);
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E03E20(D): in a packet layer (0x2078A0) the attacker drawn into the
/// target (0x1F8270 filter 0, its TEX0 / TEX1 into the sprite descriptor
/// at sp + 0x58; ALPHA 0x48), TEXFLUSH before and after; every flash task
/// stepped (0x1E02CB0); the layer linked into the ordering table, when
/// any task was alive, at the key 0x1A2C70(+0x3518 - 2000) [b003's 0x1E02F90].
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
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        alive += 1;
        if fx.mem.read_u32(t + 0x24) == FLASH {
            flash(fx, t, [k, alive]);
        }
    }
    if alive != 0 {
        let v = ee::sub(fx.mem.f(D + 0x3518), 2000.0);
        fx.fix_residue(v, 0x01E0_3F30);
        let key = ee::fixunssfsi(v) as i32;
        let layer = fx.mem.read_u32(LAYER);
        fx.layer_end(layer, OT, key);
    }
    fx.leave(0xB0);
}
