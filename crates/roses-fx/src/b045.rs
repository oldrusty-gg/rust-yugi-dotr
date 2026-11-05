//! BMODULE b045 (card 229), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03630) [F]. The entry
//! takes (D, param). b194's task-pool family: 150 tasks of 0x40 at D+0x478
//! (alive +0x36, step +0x24), spawned by 0x1E04040 (b538's spawn at D+0x2D0
//! with the colour D+0x3704, +0x30 = the kind D+0x375A), the sines
//! D+0x3A64 and D+0xD8, timer D+0x3700 = 140, the OT at D+0x80 and
//! D+0x370C. Nodes: the attacker's 1 / 5 / 6 / 0xD / 0xE / 0xF / 0x17 /
//! 0x1A at D+0x3748..; textures (0x1E03580) (4, 0..2) D+0x340.., (4,
//! 0xD..0xF) D+0x374.., (5, 0 / 1) D+0x36D0; jitter bytes D+0x3762 (% 30 -
//! 15), D+0x3862 (% 10 - 5, 100..179 cleared), D+0x3962 (% 30 - 15).
//!
//! The emitter 0x1E01CE0 spawns only: on update 0x36 sixteen tendrils
//! 0x1E02D00 (kinds 0..15, each a ten-point arc into its own history
//! D+0x2A50 + 0xA0 k around the attacker's node 1, then the target from
//! update 0x5D); on 0x5D at the target a disc 0x1E03050, eight + eight
//! grey rings 0x1E02330 / 0x1E026C0 (one step twice), three violet streaks
//! 0x1E02880 and the camera light 0x1E032D0 (b350's); 0x2E..0x6D a dark
//! ring 0x1E024F0 a update at node 1. The record's other steps (0x2060 /
//! 0x21B0 / 0x2A00 / 0x2B90 / 0x3130 / 0x3440 / 0x3500) and their draws
//! are never spawned [F]. Draws: rings billboard::ring_t (uv_rect4), the
//! streaks 0x1E011B0 (D+0xB8 quads pushed out by the jitter bytes), the
//! tendrils b218's textured ribbon_cross 0x1E04270 (word for word), the
//! disc quads4::fan18. Returns 1 on 0x52, 4 on 0x87, 2 on 0x69, 6 on
//! 0x5E..0x63, 8 once the timer has run out.
use crate::blib::spawn::node_history;
use crate::blib::spline;
use crate::blib::uv::uv_rect4;
use crate::blib::{billboard, exe as bexe, quads4};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 0x96;
const POOL: u32 = D + 0x478;
const POS: u32 = D + 0x2D0;
const CAM: u32 = D + 0x2A00;
const HIST: u32 = D + 0x2A50;
const TEX5: u32 = D + 0x36D0;
const TIMER: u32 = D + 0x3700;
const COLOUR: u32 = D + 0x3704;
const OTW: u32 = D + 0x370C;
const NODE: u32 = D + 0x3748;
const KIND: u32 = D + 0x375A;
const PARAM: u32 = D + 0x375C;
const COUNTER: u32 = D + 0x375E;
const BYTES: u32 = D + 0x3762;
const BYTES_B: u32 = D + 0x3862;
const BYTES_C: u32 = D + 0x3962;
const TAB: u32 = D + 0x3A64;

const RING: u32 = 0x01E0_2330;
const DARK: u32 = 0x01E0_24F0;
const RING2: u32 = 0x01E0_26C0;
const STREAK: u32 = 0x01E0_2880;
const TENDRIL: u32 = 0x01E0_2D00;
const DISC: u32 = 0x01E0_3050;
const LIGHT: u32 = 0x01E0_32D0;

pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1A0);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(OTW, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp) };
    fx.leave(0x1A0);
    ret
}

fn rmod(fx: &mut Fx, m: i32) -> i32 {
    let v = fx.rand();
    fx.div(v, m).1
}

fn fixu(fx: &mut Fx, at: u32) -> i32 {
    let v = fx.mem.f(at);
    fx.fix_residue(v, 0);
    ee::fixunssfsi(v) as i32
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3BD0);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(PARAM, param);
    fx.mem.set32(TIMER, 0x8C);
    for o in [0x310u32, 0x314, 0x318] {
        fx.mem.set32(D + o, 0);
    }
    for (k, (n, clear)) in [(1, 1), (5, 0), (6, 0), (0xD, 0), (0xE, 0), (0xF, 0), (0x17, 0), (0x1A, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(NODE + 2 * k as u32, r);
    }
    // 0x1E03580(0, D): the textures.
    let sp = fx.enter(0x40);
    let _ = sp;
    for k in 0..3u32 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x340 + 4 * k, t);
    }
    for k in 0xDu32..0x10 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x374 + 4 * (k - 0xD), t);
    }
    for k in 0..2u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX5 + 4 * k, t);
    }
    fx.leave(0x40);
    for (at, m, off) in [(BYTES, 0x1E, 0xF), (BYTES_B, 0xA, 5)] {
        for k in 0..0x100u32 {
            let r = rmod(fx, m);
            fx.mem.write_u8(at + k, (r - off) as u8);
        }
    }
    for k in 0x64u32..0xB4 {
        fx.mem.write_u8(BYTES_B + k, 0);
    }
    for k in 0..0x100u32 {
        let r = rmod(fx, 0x1E);
        fx.mem.write_u8(BYTES_C + k, (r - 0xF) as u8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    if fx.mem.i16(COUNTER) == 1 {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE);
        fx.node_position(h, n, sp + 0xF0);
        for k in 0..9u32 {
            for c in 0..3 {
                let w = fx.mem.read_u32(sp + 0xF0 + 4 * c);
                fx.mem.write_u32(HIST + 0x10 * k + 4 * c, w);
            }
        }
    }
    let n = fx.mem.i16(NODE);
    node_history(fx, D + 0x2FF0, 0xA, n);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        // The steps' prologue saves: the entry's s0 = D, s1 = 0, s2 = D +
        // 0x40 k, s3 = k (s4 / s5 the runner's: 0 [H]).
        let low = fx.sp;
        let small = |fx: &mut Fx, ra: u32| fx.saved(low - 0x20, &[(0x10, u64::from(ra)), (0, u64::from(D))]);
        match fx.mem.read_u32(t + 0x24) {
            RING => {
                ring(fx, t, (0xC0, 0xC0, 0xC0), None);
                small(fx, 0x01E0_3910);
            }
            DARK => {
                ring(fx, t, (0x30, 0x28, 0x30), Some(()));
                small(fx, 0x01E0_3930);
            }
            RING2 => {
                ring(fx, t, (0xC0, 0xC0, 0xC0), None);
                small(fx, 0x01E0_3950);
            }
            STREAK => {
                streak(fx, t);
                small(fx, 0x01E0_3970);
            }
            TENDRIL => {
                tendril(fx, t);
                let s = [(0x60u32, 0x01E0_39D0u64), (0x50, 0), (0x40, 0), (0x30, u64::from(k)), (0x20, u64::from(D + 0x40 * k)), (0x10, 0), (0, u64::from(D))];
                fx.saved(low - 0x80, &s);
            }
            DISC => {
                disc(fx, t);
                small(fx, 0x01E0_39F0);
            }
            LIGHT => {
                light(fx, t);
                fx.saved(low - 0x30, &[(0x20, 0x01E0_3A30), (0x10, 0), (0, u64::from(D))]);
            }
            _ => {}
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    let d = sp + 0x108;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let cell = |fx: &mut Fx, tex: u32, local: u32, id: i32| {
            let w = fx.mem.read_u32(tex);
            fx.mem.write_u32(D + 0x88, w);
            let c = fx.mem.i16(t + 0x34);
            l2::rect(fx, local, id, c);
            for k in 0..4 {
                let w = fx.mem.read_u32(local + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, w);
            }
        };
        match fx.mem.read_u32(t + 0x24) {
            RING | DARK | RING2 => {
                let s = fx.mem.read_u32(t + 0x24);
                let (tex, local, rows, table) = match s {
                    RING => (TEX5, 0x70u32, 0xA, BYTES),
                    DARK => (D + 0x348, 0x80, 4, BYTES_B),
                    _ => (TEX5 + 4, 0x90, 0xA, BYTES),
                };
                cell(fx, tex, sp + local, 0x14);
                fx.mem.set16(D + 0xB8, rows);
                billboard::ring_t(fx, uv_rect4, D, t, d, table);
                fx.mem.set16(D + 0xB8, 0);
            }
            STREAK => {
                fx.mem.set16(D + 0xB8, 0xF);
                cell(fx, D + 0x37C, sp + 0xA0, 0x9B);
                streak_draw(fx, t, d, BYTES_C);
                fx.mem.set16(D + 0xB8, 0);
            }
            TENDRIL => {
                let w = fx.mem.read_u32(TEX5);
                fx.mem.write_u32(D + 0x88, w);
                for (o, v) in [(0x94u32, 1), (0x96, 0x10), (0x98, 0x1F), (0x9A, 1)] {
                    fx.mem.set16(D + o, v);
                }
                let kind = fx.mem.i16(t + 0x30);
                let a = HIST.wrapping_add((0xA0 * kind) as u32);
                ribbon_cross_tex(fx, t, a);
            }
            DISC => {
                cell(fx, D + 0x37C, sp + 0xD0, 0x9B);
                quads4::fan18(fx, D, t, d);
            }
            _ => {}
        }
    }
    let n = fx.mem.i32(TIMER);
    if n != 0 {
        fx.mem.set32(TIMER, n - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x52 => 1,
        0x87 => 4,
        0x69 => 2,
        0x5E..=0x63 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E04040(D, step, 0): the first free task from 0 (b538's spawn).
fn spawn(fx: &mut Fx, f: u32) -> i32 {
    let sp = fx.enter(0x60);
    let _ = sp;
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(POOL + 0x40 * k + 0x36) == 0) {
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
            l2::to_i16x4(fx, t, POS, 0);
            let a = fx.mem.i16(KIND);
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

/// 0x1E01CE0(D, 0) (frame 0x50): the spawn colour (0x80, 0, 0, 0), the
/// update counter and the spawns.
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x50);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    if c == 0x36 {
        for k in 0..0x10 {
            fx.mem.set16(KIND, k);
            spawn(fx, TENDRIL);
        }
    }
    if fx.mem.i16(COUNTER) == 0x5D {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE);
        fx.node_position(h, n, sp + 0x30);
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x40);
        for c in 0..3 {
            let w = fx.mem.read_u32(sp + 0x40 + 4 * c);
            fx.mem.write_u32(POS + 4 * c, w);
        }
        fx.mem.set16(KIND, 1);
        spawn(fx, DISC);
        for _ in 0..8 {
            spawn(fx, RING);
        }
        for _ in 0..8 {
            spawn(fx, RING2);
        }
        for _ in 0..3 {
            spawn(fx, STREAK);
        }
        spawn(fx, LIGHT);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x2E..0x6E).contains(&c) {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE);
        fx.node_position(h, n, sp + 0x30);
        for c in 0..3 {
            let w = fx.mem.read_u32(sp + 0x30 + 4 * c);
            fx.mem.write_u32(POS + 4 * c, w);
        }
        spawn(fx, DARK);
    }
    fx.leave(0x50);
}

fn age(fx: &mut Fx, t: u32) -> i32 {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    n
}

/// Alpha +0x2B down by `down` while at least `floor`, else the task ends.
fn fade_out(fx: &mut Fx, t: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a >= floor {
        fx.mem.write_u8(t + 0x2B, a.wrapping_sub(down));
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// 0x1E02330 / 0x1E026C0 (grey, size (rand & 7) + 3, radius growth 25..44)
/// and 0x1E024F0 (`dark`: dark violet, size 30..79, +0x18 (rand & 0x7FFF)
/// - 4000, growth 20..44) (D, task): a ring turned at random, +8 rand % 10,
/// life 3..9 in by 0x30 to 0x60, then out by 10; the radius +0x30 from 0
/// growing by +0xE; the cell up on odd ages to 6.
fn ring(fx: &mut Fx, t: u32, c: (i32, i32, i32), dark: Option<()>) {
    if fx.mem.i16(t + 0x36) == 1 {
        if dark.is_none() {
            let r = fx.rand();
            fx.mem.set16(t + 0x38, (r & 7) + 3);
            fx.mem.set16(t + 0x34, 0);
            let r = rmod(fx, 0xFFFF);
            fx.mem.set32(t + 0x18, r);
        } else {
            let r = rmod(fx, 0x32);
            fx.mem.set16(t + 0x38, r + 0x1E);
            fx.mem.set16(t + 0x34, 0);
            let r = fx.rand();
            fx.mem.set32(t + 0x18, (r & 0x7FFF) - 0xFA0);
        }
        l2::rgb(fx, t + 0x28, c.0, c.1, c.2);
        fx.mem.set16(t + 0x30, 0);
        let r = rmod(fx, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let r = if dark.is_none() { rmod(fx, 0x14) + 0x19 } else { rmod(fx, 0x19) + 0x14 };
        fx.mem.set16(t + 0xE, r);
        let r = rmod(fx, 0xA);
        fx.mem.set16(t + 8, r);
        let r = rmod(fx, 7);
        fx.mem.set16(t + 6, r + 3);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x61 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x30));
        }
    } else {
        fade_out(fx, t, 0x11, 0xA);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let v = fx.mem.i16(t + 0x30).wrapping_add(fx.mem.i16(t + 0xE));
        fx.mem.set16(t + 0x30, v);
    }
    let a = age(fx, t);
    if a % 2 == 1 {
        let c = fx.mem.i16(t + 0x34);
        if c < 6 {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
}

/// 0x1E02880 (D, task): a violet streak (size 10, cell rand & 3, (0xC0,
/// 0x30, 0xC0), +0x18 rand % 360, +0x3A = the kind, +0x30 rand % 5 growing
/// by 20..39, +8 rand % 50, life 5..24 at 0x80, then out by 0x20).
fn streak(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0xA);
        let r = fx.rand();
        fx.mem.set16(t + 0x34, r & 3);
        l2::rgb(fx, t + 0x28, 0xC0, 0x30, 0xC0);
        let r = rmod(fx, 0x168);
        fx.mem.set32(t + 0x18, r);
        let k = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0x3A, k);
        let r = rmod(fx, 5);
        fx.mem.set16(t + 0x30, r);
        let r = rmod(fx, 0x14);
        fx.mem.set16(t + 0xE, r + 0x14);
        let r = rmod(fx, 0x32);
        fx.mem.set16(t + 8, r);
        let r = rmod(fx, 0x14);
        fx.mem.set16(t + 6, r + 5);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fade_out(fx, t, 0x21, 0x20);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let v = fx.mem.i16(t + 0x30).wrapping_add(fx.mem.i16(t + 0xE));
        fx.mem.set16(t + 0x30, v);
    }
    age(fx, t);
}

/// 0x1E02D00 (D, task) (frame 0x80): a violet tendril (life +6 = 9 points,
/// height +0xA 44..60, radius +8 5..12, angle rand % 360): its history
/// (D+0x2A50 + 0xA0 kind) set every update to ten points at the radius on
/// angles +0x3A - 10 j about the attacker's node 1 (from update 0x5D the
/// target); in by 0x1B under 0xC0 for 12 updates, out by 9 from update
/// 0x5D; from update 0x5A size and radius up by 1 / 4; the angle up by 18.
fn tendril(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x80);
    if fx.mem.i16(t + 0x36) == 1 {
        l2::rgb(fx, t + 0x28, 0x80, 0x10, 0x80);
        fx.mem.set16(t + 6, 9);
        fx.mem.write_u8(t + 0x2B, 0);
        let r = rmod(fx, 0x11);
        fx.mem.set16(t + 0xA, r + 0x2C);
        let r = fx.rand();
        fx.mem.set16(t + 8, (r & 7) + 5);
        let r = rmod(fx, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x38, 0xC);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0xC {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x1B));
        }
    } else if fx.mem.i16(COUNTER) >= 0x5D {
        fade_out(fx, t, 0xB, 9);
    }
    age(fx, t);
    if fx.mem.i16(COUNTER) < 0x5D {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE);
        fx.node_position(h, n, sp + 0x70);
    } else {
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x70);
    }
    let mut s1 = 0i32;
    for k in 0..0xAu32 {
        let a = (fx.mem.i16(t + 0x3A) + s1) as i16 as i32;
        let base = |fx: &mut Fx| HIST.wrapping_add((0xA0 * fx.mem.i16(t + 0x30)) as u32) + 0x10 * k;
        let s = l2::sin_deg(fx, TAB, a, 0);
        let v = ee::mul(ee::from_i32(fx.mem.i16(t + 8)), s);
        let h = base(fx);
        fx.mem.setf(h, v);
        let c = l2::sin_deg(fx, TAB, a, 1);
        let v = ee::mul(ee::from_i32(fx.mem.i16(t + 8)), c);
        let h = base(fx);
        fx.mem.setf(h + 8, v);
        s1 = (s1 - 0xA) as i16 as i32;
        let h = base(fx);
        let x = ee::add(fx.mem.f(h), fx.mem.f(sp + 0x70));
        fx.mem.setf(h, x);
        let h = base(fx);
        let z = ee::add(fx.mem.f(h + 8), fx.mem.f(sp + 0x78));
        fx.mem.setf(h + 8, z);
        let y = ee::from_i32(fx.mem.i16(t + 0xA));
        let h = base(fx);
        fx.mem.setf(h + 4, y);
    }
    if fx.mem.i16(COUNTER) >= 0x5A {
        let v = fx.mem.i16(t + 0x38) + 1;
        fx.mem.set16(t + 0x38, v);
        let v = fx.mem.i16(t + 8) + 4;
        fx.mem.set16(t + 8, v);
    }
    let a = fx.mem.i16(t + 0x3A) + 0x12;
    fx.mem.set16(t + 0x3A, a);
    let a = fx.mem.i16(t + 0x3A);
    if a >= 0x168 {
        let r = fx.div(a, 0x168).1;
        fx.mem.set16(t + 0x3A, r);
    }
    fx.leave(0x80);
}

/// 0x1E03050 (D, task): the disc (radius 400, (0xF0, 0x30, 0xF0) centre,
/// grey 0x10 rim of alpha 0x10), in by 0x40 for 2 updates, then out by 10.
fn disc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x190);
        l2::rgb(fx, t + 0x28, 0xF0, 0x30, 0xF0);
        l2::rgb(fx, t + 0x2C, 0x10, 0x10, 0x10);
        let r = rmod(fx, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0x10);
    }
    if fx.mem.i16(t + 0x32) < 2 {
        let a = fx.mem.read_u8(t + 0x2B);
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x40));
    } else {
        fade_out(fx, t, 0x11, 0xA);
    }
    age(fx, t);
}

/// 0x1E032D0 (D, task) (frame 0x30): b350's camera light: camera 0's
/// slots 4 / 5 (0x1F7E20) from D+0x2A00 = (128, 4, 4, 4), the task's point,
/// (0, 120, w): w up by 0.2 to 1, then down by 0.1 (the task over once at
/// most 0.1); cleared on its last update; over on timer 1.
fn light(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x30);
    let _ = sp;
    if fx.mem.i16(t + 0x36) == 1 {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x4080_0000), (8, 0x4080_0000), (0xC, 0x4080_0000), (0x20, 0), (0x24, 0x42F0_0000), (0x28, 0)] {
            fx.mem.write_u32(CAM + o, v);
        }
        for k in 0..3 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(CAM + 0x10 + 4 * k, v);
        }
        fx.mem.set16(t + 0x36, 2);
        fx.mem.set16(t + 0x30, 0);
    }
    let w = fx.mem.f(CAM + 0x28);
    if fx.mem.i16(t + 0x30) == 0 {
        if w < 1.0 {
            fx.mem.setf(CAM + 0x28, ee::add(w, f32::from_bits(0x3E4C_CCCD)));
        } else {
            fx.mem.set16(t + 0x30, 1);
        }
    } else if w <= f32::from_bits(0x3DCC_CCCD) {
        fx.mem.set16(t + 0x36, 0);
    } else {
        fx.mem.setf(CAM + 0x28, ee::sub(w, f32::from_bits(0x3DCC_CCCD)));
    }
    let p = if fx.mem.i16(t + 0x36) == 0 { 0 } else { CAM };
    bexe::camera_slots45(fx, p);
    if fx.mem.i32(TIMER) == 1 {
        fx.mem.set16(t + 0x36, 0);
    }
    fx.leave(0x30);
}

/// 0x1E011B0 (D, task, desc, table) (frame 0x140): D+0xB8 quads (4 x
/// +0x38, quad_xy) turned about x by the camera's first angle (0x1099E0,
/// D+0xD8's sines), each pushed out by the table bytes [k + +8 (+0, +100,
/// +180)] / 20 x (+0x30 + k / 2) (the far corners by 0.7 of that), at the
/// task's point (scale 0.1), on the 9-value descriptor (ALPHA 0x48, UVs
/// uv_rect4 D+0x94, texture D+0x88), keyed by the mean w on D+0x80.
fn streak_draw(fx: &mut Fx, t: u32, d: u32, table: u32) {
    use ee::{add, mul};
    let sp = fx.enter(0x140);
    fx.mem.set16(d + 2, 0x24);
    fx.mem.set16(d, 9);
    fx.mem.write_u64(d + 8, (4 << 32) | 0x2424_2421);
    fx.mem.set32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    fx.desc_alpha(d, (0x80 << 32) | 0x48);
    fx.mem.set32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    l2::rgbaq(fx, sp + 0x138, t + 0x28);
    for k in 0..2 {
        let w = fx.mem.read_u32(sp + 0x138 + 4 * k);
        fx.mem.write_u32(d + 0x10 + 4 * k, w);
    }
    uv_rect4(fx, d, D + 0x94, 0);
    let w = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, w, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xC0 + 4 * k, 0x3DCC_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0x120);
    for k in 0..3u32 {
        let v = fx.mem.i32(sp + 0x120 + 4 * k).wrapping_add(fx.mem.i32(t + 0x10 + 4 * k));
        fx.mem.set32(sp + 0x120 + 4 * k, v);
    }
    l2::camera_angles_to(fx, sp + 0x120);
    let y = fx.mem.i32(sp + 0x124);
    fx.mult(0xB60B_60B7_u32 as i32, y);
    let hi = fx.hilo().1;
    let q = (hi.wrapping_add(y) >> 7).wrapping_add(((y as u32) >> 31) as i32);
    fx.mem.set32(sp + 0x124, q.wrapping_neg());
    for k in 0..3u32 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(sp + 0xB0 + 4 * k, v);
    }
    l2::xform_vp(fx, sp + 0xE0, sp + 0xC0, 0, sp + 0xB0);
    let tenth = f32::from_bits(0x3F33_3333);
    let mut k = 0i32;
    while k < fx.mem.i16(D + 0xB8) {
        let h = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x70, 4.0, h);
        l2::rotate4(fx, sp + 0x70, sp + 0x120, 0, D + 0xD8);
        let row = table.wrapping_add(k as u32).wrapping_add(fx.mem.i16(t + 8) as u32);
        for (c, off) in [(0u32, 0u32), (4, 0x64), (8, 0xB4)] {
            let b = fx.mem.read_u8(row + off) as i8 as i32;
            let f4 = ee::div(ee::from_i32(b), 20.0);
            let r = ee::from_i32(fx.mem.i16(t + 0x30) + (k >> 1));
            let near = mul(f4, r);
            for v in [0x70u32, 0x80] {
                let x = add(fx.mem.f(sp + v + c), near);
                fx.mem.setf(sp + v + c, x);
            }
            for v in [0x90u32, 0xA0] {
                let r = ee::from_i32(fx.mem.i16(t + 0x30) + (k >> 1));
                let far = mul(tenth, mul(f4, r));
                let x = add(fx.mem.f(sp + v + c), far);
                fx.mem.setf(sp + v + c, x);
            }
        }
        if l2::project(fx, sp + 0x70, SPR, sp + 0xE0, d, sp + 0xD0) {
            let key = fixu(fx, sp + 0xD0);
            let ot = fx.mem.read_u32(D + 0x80);
            fx.submit(d, ot, key);
        }
        k = (k + 1) as i16 as i32;
    }
    fx.leave(0x140);
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn copy3(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// 0x108020(local, c) and the 8 bytes it builds copied to `to`.
fn rgbaq_to(fx: &mut Fx, local: u32, c: u32, to: u32) {
    l2::rgbaq(fx, local, c);
    for k in 0..2 {
        let w = fx.mem.read_u32(local + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// 0x1E04270(D, task, a): b218's 0x1E05710 word for word (copied from
/// b218.rs's private `ribbon_cross_tex`): `spline::ribbon_cross` on a
/// textured strip3 with two strips a piece, pushed by w = +0x38 / 10 in y
/// (+ on the first and third vertices, - on the second and fourth), then
/// in x by w and z by w / 2 [F fdiff: identical].
fn ribbon_cross_tex(fx: &mut Fx, t: u32, a: u32) {
    let sp = fx.enter(0x250);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 0x38)), 10.0);
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x198;
    l2::fullscreen4(fx, d);
    l2::desc_strip3(fx, d);
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let rh2 = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, rh2, 1);
    let c = sp + 0x24C;
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(c + k, v);
    }
    let n3 = 3 * n;
    let pa = sp + 0xE0;
    for (k, o) in [0u32, 0, 0x10, 0x20].into_iter().enumerate() {
        copy4(fx, pa + 0x10 * k as u32, a + o);
    }
    fx.mem.set32(sp + 0xB0, n - 1);
    let mut s0 = 0i32;
    while s0 < fx.mem.i32(sp + 0xB0) {
        let (q, end) = if s0 != 0 { (a.wrapping_add((0x10 * (s0 - 1)) as u32), 0x170u32) } else { (pa, 0x180) };
        let mut f20 = 0.0f32;
        for _ in 0..4 {
            spline::spline(fx, sp + 0xC0, q, f20, 0, [0.0; 4]);
            f20 = ee::add(f20, 0.25);
            spline::spline(fx, sp + end, q, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0xD0, sp + end);
            rgbaq_to(fx, sp + 0x228, c, d + 0x10);
            rgbaq_to(fx, sp + 0x230, c, d + 0x28);
            let al = i32::from(fx.mem.read_u8(t + 0x2B));
            let dq = fx.div(al, n3).0 as i16 as i32;
            let v = i32::from(fx.mem.read_u8(c + 3)) - dq;
            fx.mem.write_u8(c + 3, if v > 0 { v as u8 } else { 0 });
            rgbaq_to(fx, sp + 0x238, c, d + 0x40);
            rgbaq_to(fx, sp + 0x240, c, d + 0x58);
            for s1 in 0..2 {
                for (to, from) in [(0x120u32, 0xC0u32), (0x130, 0xC0), (0x140, 0xD0), (0x150, 0xD0)] {
                    copy3(fx, sp + to, sp + from);
                }
                let adj = |fx: &mut Fx, at: u32, by: f32, plus: bool| {
                    let x = fx.mem.f(sp + at);
                    fx.mem.setf(sp + at, if plus { ee::add(x, by) } else { ee::sub(x, by) });
                };
                if s1 == 0 {
                    adj(fx, 0x124, w, true);
                    adj(fx, 0x144, w, true);
                    adj(fx, 0x134, w, false);
                    adj(fx, 0x154, w, false);
                } else {
                    let half = ee::div(w, 2.0);
                    adj(fx, 0x120, w, true);
                    adj(fx, 0x140, w, true);
                    adj(fx, 0x130, w, false);
                    adj(fx, 0x150, w, false);
                    adj(fx, 0x128, half, true);
                    adj(fx, 0x148, half, true);
                    adj(fx, 0x138, half, false);
                    adj(fx, 0x158, half, false);
                }
                if l2::project(fx, sp + 0x120, SPR, 0, d, sp + 0x160) {
                    let key = ee::fixunssfsi(fx.mem.f(sp + 0x160)) as i32;
                    fx.submit(d, OT, key);
                }
            }
        }
        s0 += 1;
    }
    fx.leave(0x250);
}
