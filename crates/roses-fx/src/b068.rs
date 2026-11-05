//! BMODULE b068 (card 24), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E027F0, data area 0x1ED0000) [F]: an older task pool of 200 tasks of 0x40 bytes at D+0x2C0 (alive
//! +0x36, step +0x24 as a record address) drawn as rings of camera-facing
//! quads: cb18430f13b0 at 0x1E00920 (`quads4::ring_facing_wh`) and
//! 07c172c43e56 at 0x1E00EB0 (`quads4::ring_facing_i`), UVs by uv_rect4
//! 0x1E00080. For its first 20 updates two thin rings a update
//! (0x1E024E0) leave the attacker's nodes 0x2E / 0x2F; from update 100,
//! while the 380-update timer +0x35B0 has 68 or more left, two smoke
//! rings a update (0x1E01870) leave them, each of which may (1 in 5) drop
//! a white flash (0x1E01C50) on its 45th update; on updates 100..105 two
//! wider rings (0x1E01E70) start 1 above the nodes. Returns 1 on update
//! 170, 4 on 370, 2 on 360, 6 on 181..354, 8 once the timer has run out.
use crate::blib::{quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 200;
/// The step functions a task holds (+0x24), as the record's addresses.
const SMOKE: u32 = 0x01E0_1870;
const FLASH: u32 = 0x01E0_1C50;
const WIDE: u32 = 0x01E0_1E70;
const PALE: u32 = 0x01E0_21B0;
const THIN: u32 = 0x01E0_24E0;
const TIMER: u32 = D + 0x35B0;
const COLOUR: u32 = D + 0x35B4;
const COUNTER: u32 = D + 0x3616;
const BYTES: u32 = D + 0x361A;

/// The entry 0x1E027F0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    fx.mem.write_u32(D + 0x35BC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp) };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3890);
    l2::sine_table(fx, D + 0x371C);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x3614, param);
    fx.mem.set32(TIMER, 0x17C);
    for (k, node) in [0x2E, 0x2F, 0x6A, 0x26, 0xF, 0xA, 0x3D].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(D + 0x35F8 + 2 * k as u32, r);
    }
    for k in 0xE..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x2B8 + 4 * (k as u32 - 0xE), t);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x288, t);
    for k in 0..3u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x3590 + 4 * k, t);
    }
    for k in 0x64..0xB4u32 {
        let v = rand_mod(fx, 0xC);
        fx.mem.set8(BYTES + k, v - 4);
    }
    for k in 0..0x64u32 {
        let v = rand_mod(fx, 0x14);
        fx.mem.set8(BYTES + k, v - 0xA);
    }
    for k in 0xB4..0x100u32 {
        let v = rand_mod(fx, 6);
        fx.mem.set8(BYTES + k, v - 3);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    for k in 0..TASKS {
        let slot = D + 0x40 * k;
        if fx.mem.i16(slot + 0x2F6) == 0 {
            continue;
        }
        let t = slot + 0x2C0;
        if fx.mem.read_u32(slot + 0x2E4) == SMOKE {
            smoke(fx, t, slot, k);
        }
        if fx.mem.read_u32(slot + 0x2E4) == FLASH {
            flash(fx, t);
        }
        for (f, kind, ra) in [(WIDE, &WIDE_RING, 0x01E0_2A80u64), (PALE, &PALE_RING, 0x01E0_2AA0), (THIN, &THIN_RING, 0x01E0_2AC0)] {
            if fx.mem.read_u32(slot + 0x2E4) == f {
                ring(fx, t, kind, ra);
            }
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    // 0x1F71A0's frame below the entry's (its saved ra and the entry's s0
    // = D).
    fx.saved(sp - 0x20, &[(0x10, 0x01E0_2AE4), (0, u64::from(D))]);
    let y = ee::sub(fx.mem.f(sp + 0x58), 25.0);
    fx.mem.setf(sp + 0x58, y);
    for k in 0..TASKS {
        let slot = D + 0x40 * k;
        if fx.mem.i16(slot + 0x2F6) == 0 {
            continue;
        }
        for (f, tex, local) in [(SMOKE, 0x3598u32, 0x60u32), (FLASH, 0x3594, 0x70), (WIDE, 0x3598, 0x80), (PALE, 0x3594, 0x90), (THIN, 0x3598, 0xA0)] {
            if fx.mem.read_u32(slot + 0x2E4) != f {
                continue;
            }
            let w = fx.mem.read_u32(D + tex);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(slot + 0x2F4);
            l2::rect(fx, sp + local, 0x14, sub);
            for k in 0..4 {
                let v = fx.mem.read_u32(sp + local + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, v);
            }
            fx.mem.set16(D + 0xB8, 2);
            let v = fx.rand();
            fx.mem.set16(D + 0xBE, v % 4);
            if f == FLASH {
                quads4::ring_facing_i(fx, uv::uv_rect4, D, slot + 0x2C0, 0, BYTES);
            } else {
                quads4::ring_facing_wh(fx, uv::uv_rect4, D, slot + 0x2C0, 0, BYTES);
            }
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xAA => 1,
        0x172 => 4,
        0x168 => 2,
        0xB5..=0x162 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The attacker's node in slot D+`at` into D+0x250, y raised by `up`.
fn node_into(fx: &mut Fx, sp: u32, at: u32, up: bool) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(D + at);
    fx.node_position(h, slot, sp + 0x40);
    let x = fx.mem.read_u32(sp + 0x40);
    fx.mem.write_u32(D + 0x250, x);
    if up {
        let y = ee::add(1.0, fx.mem.f(sp + 0x44));
        fx.mem.setf(D + 0x254, y);
    } else {
        let y = fx.mem.read_u32(sp + 0x44);
        fx.mem.write_u32(D + 0x254, y);
    }
    let z = fx.mem.read_u32(sp + 0x48);
    fx.mem.write_u32(D + 0x258, z);
}

/// 0x1E01430(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the emitters (frame 0x50: ra, the entry's s2 = -1 / s1 = 0 / s0 =
/// D).
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x30, 0x01E0_2A08), (0x20, u64::MAX), (0x10, 0), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if fx.mem.i16(COUNTER) == 0x2D {
        fx.mem.set16(D + 0x3610, 1);
        fx.mem.set16(D + 0x360A, 0x5A);
    }
    // The emitter's s0 / s1 / s2 once set (sp + 0x44, sp + 0x48, D).
    let regs = |ra: u64| [(0x50u32, ra), (0x20, u64::from(D)), (0x10, u64::from(sp + 0x48)), (0, u64::from(sp + 0x44))];
    let c = fx.mem.i16(COUNTER);
    if c >= 0x64 && fx.mem.i32(TIMER) >= 0x44 {
        for (at, ra) in [(0x35F8u32, 0x01E0_1508u64), (0x35FA, 0x01E0_1550)] {
            node_into(fx, sp, at, false);
            fx.mem.set16(D + 0x3610, 0);
            spawn(fx, SMOKE, 0, &regs(ra));
        }
    }
    if fx.mem.i16(COUNTER) < 0x15 {
        for (at, ra) in [(0x35F8u32, 0x01E0_15B0u64), (0x35FA, 0x01E0_15F8)] {
            node_into(fx, sp, at, false);
            fx.mem.set16(D + 0x3610, 0);
            spawn(fx, THIN, 0, &regs(ra));
        }
    }
    if (0x64..0x6A).contains(&fx.mem.i16(COUNTER)) {
        for (at, ra) in [(0x35F8u32, 0x01E0_1670u64), (0x35FA, 0x01E0_16C4)] {
            node_into(fx, sp, at, true);
            fx.mem.set16(D + 0x3610, 0);
            spawn(fx, WIDE, 0, &regs(ra));
        }
    }
    if fx.mem.i16(COUNTER) == 1 {
        // Two jittered node points into D+0x250, nothing spawned [F].
        for at in [0x35FAu32, 0x35FE] {
            let h = fx.self_handle();
            let slot = fx.mem.i16(D + at);
            fx.node_position(h, slot, sp + 0x40);
            for k in 0..3 {
                let v = rand_mod(fx, 6);
                let p = ee::sub(ee::add(fx.mem.f(sp + 0x40 + 4 * k), ee::from_i32(v)), 3.0);
                fx.mem.setf(D + 0x250 + 4 * k, p);
            }
            fx.mem.set16(D + 0x3610, 0);
        }
    }
    fx.leave(0x50);
}

/// 0x1E02EA0(D, step, arg): the first free task cleared and set off at
/// D+0x250 (as i16) with the colour D+0x35B4 (twice), +0x30 = D+0x3610,
/// +0x3A = D+0x360A; 1 when one was free (b086's 0x1E01730 with other
/// offsets, 200 tasks [F]). `saves`: its prologue's.
fn spawn(fx: &mut Fx, f: u32, arg: u32, saves: &[(u32, u64)]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x40 * k + 0x2F6) == 0) {
        Some(k) => {
            let t = D + 0x2C0 + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            for c in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            let a = fx.mem.i16(D + 0x3610);
            fx.mem.set16(t + 0x30, a);
            let a = fx.mem.i16(D + 0x360A);
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

/// rand % m (HI) from the `jal` at `site` of a step whose s0 is `t` (its
/// frame left below the step's).
fn rand_at(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, site + 8);
    fx.div(v, m).1
}

/// Alpha +0x2B down by 9 (the task ends below 11).
fn fade_out(fx: &mut Fx, t: u32) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a >= 0xB {
        fx.mem.write_u8(t + 0x2B, a - 9);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// Alpha +0x2B up by `by` while below 0x61.
fn fade_in(fx: &mut Fx, t: u32, by: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a < 0x61 {
        fx.mem.write_u8(t + 0x2B, a + by);
    }
}

/// z (i16 +4) += (target z - attacker z) / (`early` for 10 updates, then
/// `late`), the units' positions at `sp + o` / `sp + o + 0x10`.
fn drift_z(fx: &mut Fx, t: u32, sp: u32, o: u32, early: f32, late: f32) {
    let h = fx.target_handle();
    fx.unit_position(h, sp + o);
    let h = fx.self_handle();
    fx.unit_position(h, sp + o + 0x10);
    let dz = ee::sub(fx.mem.f(sp + o + 8), fx.mem.f(sp + o + 0x18));
    fx.mem.setf(sp + o + 8, dz);
    let q = if fx.mem.i16(t + 0x32) < 0xA { early } else { late };
    let dz = ee::div(fx.mem.f(sp + o + 8), q);
    fx.mem.setf(sp + o + 8, dz);
    let z = ee::add(ee::from_i32(fx.mem.i16(t + 4)), dz);
    fx.mem.set16(t + 4, ee::fixsfsi(z));
}

/// The common tail: every fifth update 1 down (`sink`), the cell frame
/// +0x34 up to `cells` on odd ages, radius += growth, the quad +0xC /
/// +0xA by `grow` (its height by 4 more after 10 updates while below the
/// row +8), three random angles, the age counted.
fn tail(fx: &mut Fx, t: u32, sink: bool, cells: i32, grow: i32) {
    if sink {
        let age = fx.mem.i16(t + 0x32);
        if fx.div(age, 5).1 == 1 {
            let y = fx.mem.i16(t + 2) - 1;
            fx.mem.set16(t + 2, y);
        }
    }
    if fx.mem.i16(t + 0x32) % 2 == 1 && fx.mem.i16(t + 0x34) < cells {
        let c = fx.mem.i16(t + 0x34) + 1;
        fx.mem.set16(t + 0x34, c);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let w = fx.mem.i16(t + 0xC) + grow;
        fx.mem.set16(t + 0xC, w);
        let h = fx.mem.i16(t + 0xA) + grow;
        fx.mem.set16(t + 0xA, h);
        if fx.mem.i16(t + 0x32) >= 0xB && fx.mem.i16(t + 0xA) < fx.mem.i16(t + 8) {
            let h = fx.mem.i16(t + 0xA) + 4;
            fx.mem.set16(t + 0xA, h);
        }
    }
    for o in [0x10u32, 0x14, 0x18] {
        let v = rand_mod(fx, 0x168);
        fx.mem.set32(t + o, v);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// 0x1E01870(D, task): a smoke ring: size 70, cell frame 1, grey (alpha
/// 0), growth 7..11, quad 50 x 10, row 0..24, life 45..54; fades in by
/// 12 (not on its first update), z by the gap / 50 (/ 40 after 10
/// updates); on its 45th update 1 in 5 drop a flash at its position.
/// `slot` / `k` the entry's loop registers (s2 / s3).
fn smoke(fx: &mut Fx, t: u32, slot: u32, k: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x20, 0x01E0_2A3C), (0x10, 0), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x46);
        fx.mem.set16(t + 0x34, 1);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        fx.mem.write_u8(t + 0x2B, 0);
        fx.mem.set16(t + 0x30, 0);
        start(fx, t, (5, 7), (0x32, 0xA), (0xA, 0x2D));
    }
    let age = fx.mem.i16(t + 0x32);
    if age < fx.mem.i16(t + 6) {
        if age != 0 {
            fade_in(fx, t, 0xC);
        }
    } else {
        fade_out(fx, t);
    }
    drift_z(fx, t, sp, 0x30, 50.0, 40.0);
    if fx.mem.i16(t + 0x32) == 0x2D && rand_mod(fx, 5) == 1 {
        for (k, o) in [0u32, 2, 4].into_iter().enumerate() {
            let v = ee::from_i32(fx.mem.i16(t + o));
            fx.mem.setf(D + 0x250 + 4 * k as u32, v);
        }
        fx.mem.set16(D + 0x3610, 0);
        spawn(fx, FLASH, 0, &[(0x50, 0x01E0_1AF0), (0x30, u64::from(k)), (0x20, u64::from(slot)), (0x10, u64::from(D)), (0, u64::from(t))]);
    }
    tail(fx, t, true, 4, 4);
    fx.leave(0x60);
}

/// The first step's ring values: angle +0x3A = rand % 360, growth +0xE =
/// rand % `g.0` + `g.1`, quad (+0xC, +0xA) = `wh`, row +8 = rand % 25,
/// life +6 = rand % `life.0` + `life.1`; then running.
fn start(fx: &mut Fx, t: u32, g: (i32, i32), wh: (i32, i32), life: (i32, i32)) {
    let v = rand_mod(fx, 0x168);
    fx.mem.set16(t + 0x3A, v);
    let v = rand_mod(fx, g.0);
    fx.mem.set16(t + 0xE, v + g.1);
    fx.mem.set16(t + 0xC, wh.0);
    fx.mem.set16(t + 0xA, wh.1);
    let v = rand_mod(fx, 0x19);
    fx.mem.set16(t + 8, v);
    let v = rand_mod(fx, life.0);
    fx.mem.set16(t + 6, v + life.1);
    fx.mem.set16(t + 0x36, 2);
}

/// 0x1E01C50(D, task): a white flash: size 120 growing by 8, radius
/// 200..599 growing by 20..29, life 15..24, fading in by 13; cell frame
/// to 6.
fn flash(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, 0x01E0_2A60), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x78);
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, 0xF0, 0xF0, 0xF0);
        // Its rands leave their frame (s0 = t) at the entry's sp - 0x30,
        // a later quad's stale TEX1.
        let v = rand_at(fx, t, 0x01E0_1C8C, 0x190);
        fx.mem.set16(t + 0x30, v + 0xC8);
        let v = rand_at(fx, t, 0x01E0_1CAC, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rand_at(fx, t, 0x01E0_1CC8, 0xA);
        fx.mem.set16(t + 0xE, v + 0x14);
        fx.mem.set16(t + 0xC, 0x32);
        fx.mem.set16(t + 0xA, 0xA);
        let v = rand_at(fx, t, 0x01E0_1CF4, 0x19);
        fx.mem.set16(t + 8, v);
        let v = rand_at(fx, t, 0x01E0_1D10, 0xA);
        fx.mem.set16(t + 6, v + 0xF);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x61 {
            fx.mem.write_u8(t + 0x2B, a + 0xD);
        }
    } else {
        fade_out(fx, t);
    }
    if fx.mem.i16(t + 0x32) % 2 == 1 && fx.mem.i16(t + 0x34) < 6 {
        let c = fx.mem.i16(t + 0x34) + 1;
        fx.mem.set16(t + 0x34, c);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    let s = fx.mem.i16(t + 0x38) + 8;
    fx.mem.set16(t + 0x38, s);
    for (o, site) in [(0x10u32, 0x01E0_1DECu32), (0x14, 0x01E0_1E08), (0x18, 0x01E0_1E24)] {
        let v = rand_at(fx, t, site, 0x168);
        fx.mem.set32(t + o, v);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x20);
}

/// A ring step's constants (0x1E01E70 / 0x1E021B0 / 0x1E024E0).
struct Ring {
    size: i32,
    cell: i32,
    colour: i32,
    grow: (i32, i32),
    wh: (i32, i32),
    life: (i32, i32),
    fade: u8,
    drift: (f32, f32),
    sink: bool,
    step: i32,
}

/// 0x1E01E70: a wide grey ring (quad 50 x 15).
const WIDE_RING: Ring = Ring { size: 0x46, cell: 4, colour: 0x80, grow: (5, 7), wh: (0x32, 0xF), life: (0xA, 5), fade: 0x30, drift: (90.0, 80.0), sink: true, step: 4 };
/// 0x1E021B0: a pale ring (white).
const PALE_RING: Ring = Ring { size: 0x1E, cell: 0, colour: 0xF0, grow: (5, 7), wh: (0x32, 0xF), life: (6, 4), fade: 0x30, drift: (90.0, 80.0), sink: true, step: 4 };
/// 0x1E024E0: a thin grey ring (quad 23 x 8, growing by 2).
const THIN_RING: Ring = Ring { size: 0x23, cell: 4, colour: 0x80, grow: (7, 6), wh: (0x17, 8), life: (0xA, 5), fade: 0x18, drift: (120.0, 130.0), sink: false, step: 2 };

/// A ring step (frame 0x50: ra, the entry's s0 = D): first step as
/// `kind`; fades in by `fade` for its life, then out by 9; z by the gap /
/// `drift`; the common tail.
fn ring(fx: &mut Fx, t: u32, kind: &Ring, ra: u64) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x10, ra), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, kind.size);
        fx.mem.set16(t + 0x34, kind.cell);
        l2::rgb(fx, t + 0x28, kind.colour, kind.colour, kind.colour);
        fx.mem.set16(t + 0x30, 0);
        start(fx, t, kind.grow, kind.wh, kind.life);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fade_in(fx, t, kind.fade);
    } else {
        fade_out(fx, t);
    }
    drift_z(fx, t, sp, 0x20, kind.drift.0, kind.drift.1);
    tail(fx, t, kind.sink, 5, kind.step);
    fx.leave(0x50);
}
