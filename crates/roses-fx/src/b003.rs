//! BMODULE b003 (card 21), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02630, data area 0x1ED0000) [F] (helper C8, 31
//! Oct 2026): b505's older task pool (170 tasks of 0x40 bytes at +0x2A0,
//! alive +0x36, step +0x24, each step called by its own `jal`) drawn with
//! the second library. The emitter 0x1E01400 (`emit`): on update 1 a
//! whole-window unit flash (0x1E02440, the unit drawn into the target in a
//! packet layer, 0x1E02F90); on 0xAF a red full-window tint (0x1E02110,
//! inlined in the entry: an untextured gouraud quad, 0x109DD0); on
//! 0xB5..0x137 one orb a update (two on 0x137) at the attacker's node 0x1F
//! (0x1E01860: its own position slot in the tables +0x2D94 / +0x303C,
//! drifting toward the target; `quads4::ring_at_m`); on 0xBF..0x13F a ring
//! a update around the target's node 1 (0x1E01C70 grey on odd updates,
//! 0x1E01EC0 dark on even; `quads4::ring_at_mt`). Returns 1 on update 0xAA,
//! 4 on 0x17C, 2 on 0x177, 6 on 0xB5..0x171, 8 once the 395-update timer
//! +0x2D70 has run out.
//!
//! The steps 0x1E01780, 0x1E021C0 (a frame tint through 0x1F7E20),
//! 0x1E02330 (the stage light) and 0x1E023F0 (an object op through
//! 0x1F8380) are tested by the entry but never spawned [F: the emitter's
//! spawns name 0x1E02110 / 0x1E02440 / 0x1E01860 / 0x1E01C70 / 0x1E01EC0
//! only]; not ported.
//!
//! Library at its own addresses: uv_rect (0x1E00080), `ring_at_mt`
//! (0x1E00920), `ring_at_m` (0x1E00EA0).
use crate::blib::{quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0xAA;
/// The step functions a task holds (+0x24), as the record's addresses.
const ORB: u32 = 0x01E0_1860;
const GREY: u32 = 0x01E0_1C70;
const DARK: u32 = 0x01E0_1EC0;
const TINT: u32 = 0x01E0_2110;
const FLASH: u32 = 0x01E0_2440;
const TIMER: u32 = D + 0x2D70;
const COLOUR: u32 = D + 0x2D74;
const XS: u32 = D + 0x2D94;
const ZS: u32 = D + 0x303C;
const LAYER: u32 = D + 0x32E4;
const FLASH_DESC: u32 = D + 0x32E8;
const NODES: u32 = D + 0x32F0;
const NEXT: u32 = D + 0x3304;
const INDEX: u32 = D + 0x330A;
const YAW: u32 = D + 0x330E;
const COUNTER: u32 = D + 0x3310;
const BYTES: u32 = D + 0x3312;

/// The entry 0x1E02630.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x160);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x2D7C, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x160);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x3580);
    l2::sine_table(fx, D + 0x3414);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x330C, param);
    fx.mem.set32(TIMER, 0x18B);
    for (k, (target, n)) in [(false, 0x1F), (false, 0x20), (true, 1), (false, 0x47), (false, 0x4E)].into_iter().enumerate() {
        let h = if target { fx.target_handle() } else { fx.self_handle() };
        let r = fx.request_node(h, n, i32::from(k == 0 || target));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    for id in 0xAu32..0x10 {
        let tex = fx.texture(4, id as i32);
        fx.mem.write_u32(D + 0x288 + 4 * (id - 0xA), tex);
    }
    let tex = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x268, tex);
    let tex = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, tex);
    for k in 0..4u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x2D20 + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.fix_residue(v, 0x01E0_27C0);
    fx.mem.set16(YAW, ee::fixsfsi(v));
    // rand's frame: s0 = the loop counter.
    for k in 0..0x100u32 {
        let r = fx.rand_from(k, 0x01E0_27D0);
        fx.mem.write_u8(BYTES + k, (r.wrapping_rem(16) - 8) as u8);
    }
    for k in 0..8u32 {
        let r = fx.rand_from(k, 0x01E0_280C);
        fx.mem.set16(D + 0xC0 + 2 * k, r.wrapping_rem(4));
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    // s1 counts the flash tasks (run by the unit flash).
    let mut flashes = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x24);
        match f {
            ORB => orb(fx, t, flashes),
            GREY => ring(fx, t, [0x80; 3], 0),
            DARK => ring(fx, t, [0x20; 3], 0x250),
            TINT => tint(fx, t),
            FLASH => flashes += 1,
            _ => debug_assert!(!matches!(f, 0x01E0_1780 | 0x01E0_21C0 | 0x01E0_2330 | 0x01E0_23F0)),
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x70);
    // s0 counts the live tasks of the draw loop (rand's saved s0).
    let mut live = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        live += 1;
        if fx.mem.read_u32(t + 0x24) == ORB {
            let tex = fx.mem.read_u32(D + 0x2D28);
            fx.mem.write_u32(D + 0x88, tex);
            cell(fx, t, sp + 0x80);
            // A first draw's set-up without its call: the cell's UVs
            // overwritten (1, 1, 0x3F, 0x3F), a row count and an orientation.
            for (o, v) in [(0x94u32, 1), (0x96, 1), (0x98, 0x3F), (0x9A, 0x3F)] {
                fx.mem.set16(D + o, v);
            }
            let rows = fx.mem.i16(t + 0xC);
            fx.mem.set16(D + 0xB8, rows);
            let r = fx.rand_from(live, 0x01E0_2AE0);
            fx.mem.set16(D + 0xBE, r.wrapping_rem(4));
            centre(fx, t);
            fx.mem.set16(D + 0xB8, 0);
            let tex = fx.mem.read_u32(D + 0x260);
            fx.mem.write_u32(D + 0x88, tex);
            centre(fx, t);
            cell(fx, t, sp + 0x90);
            if fx.mem.i16(t + 0x32) >= 4 {
                fx.mem.set16(D + 0xB8, 3);
                let r = fx.rand_from(live, 0x01E0_2BD0);
                fx.mem.set16(D + 0xBE, r.wrapping_rem(4));
                centre(fx, t);
                quads4::ring_at_m(fx, uv::uv_rect, D, t, 0x20, BYTES);
                fx.mem.set16(D + 0xB8, 0);
            }
        }
        if fx.mem.read_u32(t + 0x24) == GREY {
            let tex = fx.mem.read_u32(D + 0x2D2C);
            fx.mem.write_u32(D + 0x88, tex);
            cell(fx, t, sp + 0xA0);
            let rows = fx.mem.i16(t + 0xC);
            fx.mem.set16(D + 0xB8, rows);
            let r = fx.rand_from(live, 0x01E0_2C98);
            fx.mem.set16(D + 0xBE, r.wrapping_rem(4));
            quads4::ring_at_mt(fx, uv::uv_rect, D, t, 0x10, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x24) == DARK {
            let tex = fx.mem.read_u32(D + 0x2D24);
            fx.mem.write_u32(D + 0x88, tex);
            let u = (fx.mem.i16(t + 0x34) << 5) + 1;
            for (o, v) in [(0x94u32, u), (0x96, 1), (0x98, 0x1F), (0x9A, 0x3D)] {
                fx.mem.set16(D + o, v);
            }
            let rows = fx.mem.i16(t + 0xC);
            fx.mem.set16(D + 0xB8, rows);
            let r = fx.rand_from(live, 0x01E0_2D18);
            fx.mem.set16(D + 0xBE, r.wrapping_rem(4));
            quads4::ring_at_mt(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x24) == TINT {
            let ot = fx.mem.read_u32(D + 0x2D7C);
            let d = sp + 0xC8;
            l2::colour_strip4(fx, d, t + 0x28, 0);
            l2::desc_gouraud4(fx, d, 0);
            l2::fullscreen4(fx, d);
            fx.submit(d, ot, 0);
        }
    }
    if flashes != 0 {
        // a3 at the flash's first 0x207BB0: 0 (only the flash is alive then;
        // no call before it sets a3) [run].
        unit_flash(fx, 0);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xAA => 1,
        0x17C => 4,
        0x177 => 2,
        0xB5..=0x171 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
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

/// An orb's centre into D+0x20: x the i16 +0, y / z its slot +0xA in the
/// tables +0x2D94 / +0x303C.
fn centre(fx: &mut Fx, t: u32) {
    let x = ee::from_i32(fx.mem.i16(t));
    fx.mem.setf(D + 0x20, x);
    let i = fx.mem.i16(t + 0xA) as u32;
    let y = fx.mem.read_u32(XS + 4 * i);
    fx.mem.write_u32(D + 0x24, y);
    let z = fx.mem.read_u32(ZS + 4 * i);
    fx.mem.write_u32(D + 0x28, z);
}

/// 0x1E01400(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// +0x3310 and the emitters (frame 0x40: ra, s1 = D, s0 the entry's 0).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    let s3 = param as i64 as u64;
    fx.saved(sp, &[(0x20, 0x01E0_2848), (0x10, 0), (0, 0)]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    // The spawn's saves: the emitter's s0 / s1 = D, the entry's s2 = D, s3
    // (the parameter), s4.
    let mut s0 = 0u64;
    let regs = |s0: u64| [(0x40u32, 0u64), (0x30, s3), (0x20, u64::from(D)), (0x10, u64::from(D)), (0, s0)];
    if fx.mem.i16(COUNTER) == 0xAF {
        spawn(fx, TINT, &regs(s0), 0x01E0_1450);
    }
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, FLASH, &regs(s0), 0x01E0_1474);
    }
    let c = fx.mem.i16(COUNTER);
    if (0xB5..0x138).contains(&c) {
        orb_site(fx, sp);
        let i = fx.mem.i16(NEXT);
        fx.mem.set16(INDEX, i);
        spawn(fx, ORB, &regs(0), 0x01E0_1518);
        s0 = 1;
        next_slot(fx);
    }
    if fx.mem.i16(COUNTER) == 0x137 {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES);
        fx.node_position(h, s, sp + 0x30);
        for k in 0..2u64 {
            orb_xyz(fx, sp);
            let i = fx.mem.i16(NEXT);
            fx.mem.set16(INDEX, i);
            spawn(fx, ORB, &regs(k), 0x01E0_15DC);
            next_slot(fx);
        }
        s0 = 2;
    }
    let c = fx.mem.i16(COUNTER);
    if (0xBF..0x140).contains(&c) {
        let h = fx.target_handle();
        let s = fx.mem.i16(NODES + 4);
        fx.node_position(h, s, sp + 0x30);
        let s0w = s0 as u32;
        for (o, ra, m, off) in [(0u32, 0x01E0_164Cu32, 0x14, 10.0f32), (4, 0x01E0_1680, 0x28, 20.0), (8, 0x01E0_16B4, 0x14, 10.0)] {
            let r = fx.rand_from(s0w, ra);
            let (_, v) = fx.div(r, m);
            let p = ee::sub(ee::add(fx.mem.f(sp + 0x30 + o), ee::from_i32(v)), off);
            fx.mem.setf(D + 0x250 + o, p);
        }
        if c.wrapping_rem(2) == 1 {
            spawn(fx, GREY, &regs(0), 0x01E0_1720);
        } else if c.wrapping_rem(2) == 0 {
            spawn(fx, DARK, &regs(s0), 0x01E0_1764);
        }
    }
    fx.leave(0x40);
}

/// The orbs' spawn point: the attacker's node 0x1F, z 3 toward the facing.
fn orb_site(fx: &mut Fx, sp: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i16(NODES);
    fx.node_position(h, s, sp + 0x30);
    orb_xyz(fx, sp);
}

fn orb_xyz(fx: &mut Fx, sp: u32) {
    for o in [0u32, 4] {
        let w = fx.mem.read_u32(sp + 0x30 + o);
        fx.mem.write_u32(D + 0x250 + o, w);
    }
    let z = fx.mem.f(sp + 0x38);
    let z = if fx.mem.i16(YAW) != 0 { ee::add(3.0, z) } else { ee::sub(z, 3.0) };
    fx.mem.setf(D + 0x258, z);
}

fn next_slot(fx: &mut Fx) {
    let n = fx.mem.i16(NEXT) + 1;
    fx.mem.set16(NEXT, n);
    if fx.mem.i16(NEXT) >= 0xAA {
        fx.mem.set16(NEXT, 0);
    }
}

/// 0x1E02E70(D, step, 0): the first free task cleared and set off at
/// D+0x250 (as i16) with the colour +0x2D74 (twice), +0x30 = +0x330A;
/// `saves` the caller's s4..s0, `ra` the return address.
fn spawn(fx: &mut Fx, f: u32, saves: &[(u32, u64)], ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
    fx.saved(sp, &[(0x50, u64::from(ra))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x2D6 + 0x40 * k) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x40 * k;
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

/// The alpha +0x2B up by `up` while at most 0x80 while `rising`, else down
/// by 9 while at least 0xB, else the task ends.
fn fade(fx: &mut Fx, t: u32, rising: bool, up: u8) {
    let v = fx.mem.read_u8(t + 0x2B);
    if rising {
        if v < 0x81 {
            fx.mem.write_u8(t + 0x2B, v.wrapping_add(up));
        }
    } else if v >= 0xB {
        fx.mem.write_u8(t + 0x2B, v - 9);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
}

fn odd(v: i32) -> bool {
    v.wrapping_rem(2) == 1
}

/// 0x1E01860(D, t): an orb (a ring of +0xC quads, size +0x38 12..23
/// growing, dark grey, radius +0x30 growing by +0xE 5..9, row +8, life +6
/// 37..61): its own slot +0xA in the tables (y = +2 - 0.7, z = +4); y
/// falls by 0.8 a update, z closes on the target by a 95th of the gap;
/// +0xC = the attacker's nodes 0x20 - 0x1F in x; angles +0x10 / +0x14 /
/// +0x18 random; fades in by 0x40, out by 9; from update 0x173 its age
/// stays 0x46.
fn orb(fx: &mut Fx, t: u32, flashes: u32) {
    let sp = fx.enter(0x70);
    // The entry's s2 = D, s1 = the flash count, s0 = 0.
    fx.saved(sp, &[(0x30, 0x01E0_28A0), (0x20, u64::from(D)), (0x10, u64::from(flashes)), (0, 0)]);
    if fx.mem.i16(t + 0x36) == 1 {
        let v = rand_mod(fx, t, 0x01E0_1890, 0xC);
        fx.mem.set16(t + 0x38, v + 0xC);
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, 0x40, 0x40, 0x40);
        let i = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, i);
        let i = fx.mem.i16(t + 0xA) as u32;
        let y = ee::from_i32(fx.mem.i16(t + 2));
        fx.mem.setf(XS + 4 * i, y);
        let z = ee::from_i32(fx.mem.i16(t + 4));
        fx.mem.setf(ZS + 4 * i, z);
        let y = ee::sub(fx.mem.f(XS + 4 * i), f32::from_bits(0x3F33_3333));
        fx.mem.setf(XS + 4 * i, y);
        fx.mem.set16(t + 0x30, 0);
        let v = rand_mod(fx, t, 0x01E0_1928, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rand_mod(fx, t, 0x01E0_1944, 5);
        fx.mem.set16(t + 0xE, v + 5);
        let v = rand_mod(fx, t, 0x01E0_1964, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, t, 0x01E0_1980, 0x19);
        fx.mem.set16(t + 6, v + 0x25);
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES);
        fx.node_position(h, s, sp + 0x40);
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES + 2);
        fx.node_position(h, s, sp + 0x50);
        let v = ee::sub(fx.mem.f(sp + 0x50), fx.mem.f(sp + 0x40));
        fx.fix_residue(v, 0x01E0_19DC);
        fx.mem.set16(t + 0xC, ee::fixsfsi(v));
        fx.mem.set16(t + 0x36, 2);
    }
    let rising = fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6);
    fade(fx, t, rising, 0x40);
    if fx.mem.i16(COUNTER) >= 0x156 {
        let s = fx.mem.i16(t + 0x38) + 4;
        fx.mem.set16(t + 0x38, s);
    }
    if fx.mem.i16(t + 0x32) != 0 && fx.mem.i16(t + 0x38) < 0x3C {
        let s = fx.mem.i16(t + 0x38) + 2;
        fx.mem.set16(t + 0x38, s);
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x40);
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x50);
    let i = fx.mem.i16(t + 0xA) as u32;
    if fx.mem.i16(t + 0x32) != 0 {
        let d = ee::sub(fx.mem.f(sp + 0x48), fx.mem.f(sp + 0x58));
        fx.mem.setf(sp + 0x48, d);
        let d = ee::div(d, 95.0);
        fx.mem.setf(sp + 0x48, d);
        let z = ee::add(fx.mem.f(ZS + 4 * i), d);
        fx.mem.setf(ZS + 4 * i, z);
    }
    let y = ee::sub(fx.mem.f(XS + 4 * i), f32::from_bits(0x3F4C_CCCD));
    fx.mem.setf(XS + 4 * i, y);
    if odd(fx.mem.i16(t + 0x32)) && fx.mem.i16(t + 0x34) < 5 {
        let f = fx.mem.i16(t + 0x34) + 1;
        fx.mem.set16(t + 0x34, f);
    }
    let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x30, r);
    if odd(fx.mem.i16(t + 0x32)) {
        let s = fx.mem.i16(t + 0x38) + 2;
        fx.mem.set16(t + 0x38, s);
    }
    for (o, ra) in [(0x10u32, 0x01E0_1B9Cu32), (0x14, 0x01E0_1BB8)] {
        let v = rand_mod(fx, t, ra, 0x168);
        fx.mem.set32(t + o, v);
    }
    let a = fx.mem.i16(t + 0x3A);
    let a = if odd(fx.mem.i16(t + 0xE)) { a + 2 } else { a - 2 };
    fx.mem.set16(t + 0x3A, a);
    let v = rand_mod(fx, t, 0x01E0_1C18, 0x168);
    fx.mem.set32(t + 0x18, v);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    if fx.mem.i16(COUNTER) >= 0x173 {
        fx.mem.set16(t + 0x32, 0x46);
    }
    fx.leave(0x70);
}

/// 0x1E01C70 (`ras` 0) / 0x1E01EC0 (+0x250): a ring round the target (+0xC
/// 2..4 quads, size 45 growing to 90, colour `c`, radius 130..429, row
/// +8, life +6 15..24), rising by 1, angles random each update; fades in
/// by 0x12, out by 9.
fn ring(fx: &mut Fx, t: u32, c: [i32; 3], ras: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, 0x01E0_28C0 + 0x20 * u64::from(ras != 0)), (0, 0)]);
    let ra = |a: u32| a + ras;
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x2D);
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, c[0], c[1], c[2]);
        let v = rand_mod(fx, t, ra(0x01E0_1CB4), 0x12C);
        fx.mem.set16(t + 0x30, v + 0x82);
        let v = rand_mod(fx, t, ra(0x01E0_1CD4), 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0xE, 0);
        let v = rand_mod(fx, t, ra(0x01E0_1CF4), 0x23);
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, t, ra(0x01E0_1D10), 3);
        fx.mem.set16(t + 0xC, v + 2);
        let v = rand_mod(fx, t, ra(0x01E0_1D30), 0xA);
        fx.mem.set16(t + 6, v + 0xF);
        fx.mem.set16(t + 0x36, 2);
    }
    let rising = fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6);
    fade(fx, t, rising, 0x12);
    if fx.mem.i16(t + 0x32) != 0 && fx.mem.i16(t + 0x38) < 0x5A {
        let s = fx.mem.i16(t + 0x38) + 2;
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
    if fx.mem.i16(t + 0x32) != 0 {
        let s = fx.mem.i16(t + 0x38) + 2;
        fx.mem.set16(t + 0x38, s);
    }
    let y = fx.mem.i16(t + 2) + 1;
    fx.mem.set16(t + 2, y);
    for (o, a) in [(0x10u32, 0x01E0_1E48u32), (0x14, 0x01E0_1E64), (0x18, 0x01E0_1E80)] {
        let v = rand_mod(fx, t, ra(a), 0x168);
        fx.mem.set32(t + o, v);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x20);
}

/// 0x1E02110 (inlined in the entry): the tint's colour (0x60, 8, 8), alpha
/// up by 8 (at most 0x80) for 0x96 updates, then out by 9.
fn tint(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        l2::rgb(fx, t + 0x28, 0x60, 8, 8);
        fx.mem.set16(t + 0x36, 2);
    }
    let rising = fx.mem.i16(t + 0x32) < 0x96;
    fade(fx, t, rising, 8);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// 0x1E02440(D, t): the flash's frame sprite (the target over the window,
/// white, UVs inset by the size +8 over 512 both ways) into the
/// descriptor at +0x32E8, no ordering table; alpha up by 10 below 0x80
/// for 0x1D updates, then down by 9 while the size grows by 7. `regs` the
/// unit flash's s0 / s1 (the slot index, the live count).
fn flash(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_3064), (0x20, u64::from(regs[1])), (0x10, u64::from(regs[0]))]);
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
    if fx.mem.i16(t + 0x32) < 0x1D {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x80 {
            fx.mem.write_u8(t + 0x2B, a + 0xA);
        }
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0xB {
            fx.mem.write_u8(t + 0x2B, a - 9);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
        let s = fx.mem.i16(t + 8) + 7;
        fx.mem.set16(t + 8, s);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x40);
}

/// 0x1E02F90(D): in a packet layer (0x2078A0) the attacker drawn into the
/// target (0x1F8270 filter 0, its TEX0 / TEX1 into the sprite descriptor
/// at sp + 0x58; ALPHA 0x48), TEXFLUSH before and after; every flash task
/// stepped (0x1E02440); the layer linked into the ordering table, when
/// any task was alive, at the key 0x1A2C70(+0x32EC - 2000).
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
        let t = D + 0x2A0 + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        alive += 1;
        if fx.mem.read_u32(t + 0x24) == FLASH {
            flash(fx, t, [k, alive]);
        }
    }
    if alive != 0 {
        let v = ee::sub(fx.mem.f(D + 0x32EC), 2000.0);
        fx.fix_residue(v, 0x01E0_30A0);
        let key = ee::fixunssfsi(v) as i32;
        let layer = fx.mem.read_u32(LAYER);
        fx.layer_end(layer, OT, key);
    }
    fx.leave(0xB0);
}
