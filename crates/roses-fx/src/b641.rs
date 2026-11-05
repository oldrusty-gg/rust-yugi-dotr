//! BMODULE b641 (card 264), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02510, data area 0x1ED0000) [F]: b236's task pool of 400 (0x70 bytes at +0x2A0, alive +0x58, step
//! +0x44 as a record address) drawn with the second library: quads
//! (`billboard::quad`, 0x1E01230, UVs by uv_rect 0x1E00080), rings
//! (`billboard::ring_quads`, 0x1E00920), spark bursts (`quads4::sparks`,
//! 21cb40d76bec at 0x1E014F0) and an arc fan (`billboard::arc_strips`,
//! 0x1E00ED0). The stage light dims to 0x30 for 151 updates (0x1E023F0)
//! and comes back over the last 30. From update 168 to 254 a white flare
//! rises at the attacker's node 6, swaying; on update 237 at the target
//! 30 coloured spark bursts, 15 rings, 16 flares thrown out and the arc
//! fan start. Returns 1 on update 227, 4 on 280, 2 on 265, 6 on 238..259,
//! 8 once the 290-update timer +0xB210 has run out.
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
/// The step functions a task holds (+0x44), as the record's addresses.
const FLARE: u32 = 0x01E0_1AD0;
const RING: u32 = 0x01E0_1C90;
const BURST: u32 = 0x01E0_1E40;
const THROWN: u32 = 0x01E0_20B0;
const FAN: u32 = 0x01E0_2300;
const LIGHT: u32 = 0x01E0_23F0;
const IDLE: u32 = 0x01E0_2500;
/// The return address of the entry's `jalr` into a step (0x1E02818 + 8).
const TASK_RA: u64 = 0x01E0_2820;
const TIMER: u32 = D + 0xB210;
const COLOUR: u32 = D + 0xB214;
const COUNTER: u32 = D + 0xB2B8;
const FACING: u32 = D + 0xB2B4;
const BYTES: u32 = D + 0xB2BA;
const SINES: u32 = D + 0xB4BC;
/// 0.6 and 8.6 (the flares' sway and throw).
const SWAY: u32 = 0x3F19_999A;
const THROW: u32 = 0x4109_999A;

/// The entry 0x1E02510.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xA0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2B6, param);
    fx.mem.set32(TIMER, 0x122);
    for (at, node, clear) in [(0xB244u32, 6, 1), (0xB246, 0x34, 0), (0xB248, 0x35, 0), (0xB24A, 0x30, 0), (0xB24C, 0x2F, 0), (0xB24E, 0x2C, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(D + at, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB254, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..3u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0xB1F0 + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x90);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x94)), 180.0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = fx.rand();
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand();
        fx.mem.set16(D + 0xB264 + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    for k in 0..TASKS {
        let t = D + 0x70 * k;
        if fx.mem.i16(t + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x2E4);
        if f != 0 && f != IDLE {
            run_task(fx, f, t + 0x2A0);
        }
    }
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        let t = slot + 0x2A0;
        if fx.mem.read_u32(t + 0x44) == FLARE {
            quad_draw(fx, t, sp + 0x50);
        }
        if fx.mem.read_u32(t + 0x44) == RING {
            let w = fx.mem.read_u32(D + 0xB1F8);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x60, 0x14, sub);
            fx.mem.set16(D + 0xB8, 4);
            let v = fx.rand();
            fx.mem.set16(D + 0xBE, v % 4);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x21, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == BURST {
            let kind = fx.mem.i16(t + 0x50);
            if kind < 2 {
                let w = fx.mem.read_u32(D + 0xB1F4);
                fx.mem.write_u32(D + 0x88, w);
                for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x20), (0x9A, 0x40)] {
                    fx.mem.set16(D + o, v);
                }
            } else if kind < 4 {
                cell(fx, sp + 0x70, 0x9B, 0);
                let w = fx.mem.read_u32(D + 0x29C);
                fx.mem.write_u32(D + 0x88, w);
            }
            fx.mem.set16(D + 0xB8, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, w);
            }
            fx.mem.set16(D + 0xB8, 0xC);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sparks(fx, D, t, 0x20, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == THROWN {
            quad_draw(fx, t, sp + 0x80);
        }
        if fx.mem.read_u32(t + 0x44) == FAN {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xE3 => 1,
        0x118 => 4,
        0x109 => 2,
        0xEE..=0x103 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` / `sub` (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// A flare's quad: texture D+0xB1F0, cell 0x14 frame +0x56, orientation
/// 0, at its position +0x10.
fn quad_draw(fx: &mut Fx, t: u32, r: u32) {
    let w = fx.mem.read_u32(D + 0xB1F0);
    fx.mem.write_u32(D + 0x88, w);
    let sub = fx.mem.i16(t + 0x56);
    cell(fx, r, 0x14, sub);
    fx.mem.set16(D + 0xBE, 0);
    billboard::quad(fx, uv::uv_rect, D, t, 0x10, t + 0x10);
}

fn run_task(fx: &mut Fx, f: u32, t: u32) {
    match f {
        FLARE => flare(fx, t),
        RING => ring(fx, t),
        BURST => burst(fx, t),
        THROWN => thrown(fx, t),
        FAN => fan(fx, t),
        LIGHT => light(fx, t),
        _ => panic!("b641: unknown task step {f:#x}"),
    }
}

/// 0x1E018F0(D, 0): the update counter +0xB2B8 and the emitters (frame
/// 0x30: ra, the entry's s1 = D / s0 = 0).
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_27E0), (0x10, u64::from(D)), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = fx.mem.i16(COUNTER);
    if (0xA8..0xFF).contains(&c) {
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + 0xB244);
        fx.node_position(h, slot, D + 0x250);
        let y = ee::sub(fx.mem.f(D + 0x254), 2.0);
        fx.mem.setf(D + 0x254, y);
        spawn(fx, FLARE, D, 0x01E0_1988);
    }
    if fx.mem.i16(COUNTER) == 0xED {
        let h = fx.target_handle();
        fx.unit_position(h, D + 0x250);
        // 0x1F71A0's frame below the emitter's (its ra, the emitter's s0
        // = D).
        fx.saved(sp - 0x20, &[(0x10, 0x01E0_19B4), (0, u64::from(D))]);
        for (f, n, ra) in [(BURST, 0x1E, 0x01E0_19D0u32), (RING, 0xF, 0x01E0_1A10), (THROWN, 0x10, 0x01E0_1A50)] {
            for k in 0..n {
                spawn(fx, f, k, ra);
            }
        }
        spawn(fx, FAN, 0x10, 0x01E0_1A88);
    }
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, LIGHT, D, 0x01E0_1AB4);
    }
    fx.leave(0x30);
}

/// 0x1E02C00(D, step, 0): b631's spawn (the first free task cleared and
/// set off at D+0x250 with the colour +0xB214 twice, angle flag +0xB2B0);
/// its saves: ra, the emitter's s0 = D / s1 = `s1`, the entry's s2 = -1.
fn spawn(fx: &mut Fx, f: u32, s1: u32, ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, u64::MAX), (0x10, u64::from(s1)), (0, u64::from(D))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x70 * k + 0x2F8) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for c in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(D + 0x250 + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let a = fx.mem.i16(D + 0xB2B0);
            fx.mem.set16(t + 0x50, a);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// rand() from a step's `jal` at `site` (its frame: s0 = the task `t`).
fn rand_at(fx: &mut Fx, t: u32, site: u32) -> i32 {
    fx.rand_from(t, site + 8)
}

/// rand % m (HI) from `site`.
fn rand_mod(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = rand_at(fx, t, site);
    fx.div(v, m).1
}

/// (rand >> 4) % m (HI) from `site`.
fn rand4_mod(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = rand_at(fx, t, site);
    fx.div(v >> 4, m).1
}

/// axis `a` (+0x10 / +0x14 / +0x18) += `k` x sin (or cos) of the i16
/// angle at `t + deg` degrees (0x108910 on the table +0xB4BC).
fn sway(fx: &mut Fx, t: u32, a: u32, deg: u32, cos: i32, k: u32) {
    let d = fx.mem.i16(t + deg);
    let s = l2::sin_deg(fx, SINES, d, cos);
    let v = ee::add(fx.mem.f(t + a), ee::mul(f32::from_bits(k), s));
    fx.mem.setf(t + a, v);
}

/// 0x205580(t + 0x20) from a step (its saves: ra, the entry's s1 = D, s0
/// = t, below the step's frame).
fn angles(fx: &mut Fx, t: u32, ra: u32) {
    let base = fx.sp.wrapping_sub(0x30);
    fx.saved(base, &[(0x20, u64::from(ra)), (0x10, u64::from(D)), (0, u64::from(t))]);
    l2::camera_angles_to(fx, t + 0x20);
}

/// A flare's first step: size 150, cell frame 2..4, white, a random
/// heading +0x5A (0x1E01AD0 / 0x1E020B0).
fn flare_start(fx: &mut Fx, t: u32, sites: [u32; 2]) {
    fx.mem.set16(t + 8, 0x96);
    let s = fx.mem.i16(t + 8);
    fx.mem.set16(t + 0xA, s);
    let v = rand4_mod(fx, t, sites[0], 3);
    fx.mem.set16(t + 0x56, v + 2);
    l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
    fx.mem.set16(t + 0x58, 2);
    let v = rand4_mod(fx, t, sites[1], 0x168);
    fx.mem.set16(t + 0x5A, v);
}

/// Alpha +0x63 up by 0x60 below 0xC0 while the age is under `life`, then
/// down by 19 (the task ends below 21).
fn flare_fade(fx: &mut Fx, t: u32, life: i32) {
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < life {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a + 0x60);
        }
    } else if a >= 0x15 {
        fx.mem.write_u8(t + 0x63, a - 0x13);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E01AD0: a rising flare: sways in z / x by 0.6 sin / cos of its
/// heading and drifts 2 a update with the facing; faces the camera.
fn flare(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        flare_start(fx, t, [0x01E0_1B00, 0x01E0_1B38]);
    }
    flare_fade(fx, t, 6);
    if fx.mem.i16(t + 0x54) != 0 {
        sway(fx, t, 0x18, 0x5A, 0, SWAY);
        sway(fx, t, 0x10, 0x5A, 1, SWAY);
        let z = fx.mem.f(t + 0x18);
        let z = if fx.mem.i16(FACING) != 0 { ee::add(z, 2.0) } else { ee::sub(z, 2.0) };
        fx.mem.setf(t + 0x18, z);
    }
    angles(fx, t, 0x01E0_1C68);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x30);
}

/// 0x1E020B0: a thrown flare: first thrown 8.6 along its heading +0x5A
/// (y, x) and +0x5C (z), then swaying by 0.6 of the same; faces the
/// camera.
fn thrown(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        flare_start(fx, t, [0x01E0_20E0, 0x01E0_2118]);
        let v = rand4_mod(fx, t, 0x01E0_2138, 0x168);
        fx.mem.set16(t + 0x5C, v);
        sway(fx, t, 0x14, 0x5A, 0, THROW);
        sway(fx, t, 0x10, 0x5A, 1, THROW);
        sway(fx, t, 0x18, 0x5C, 1, THROW);
    }
    flare_fade(fx, t, 0x1A);
    if fx.mem.i16(t + 0x54) != 0 {
        sway(fx, t, 0x14, 0x5A, 0, SWAY);
        sway(fx, t, 0x10, 0x5A, 1, SWAY);
        sway(fx, t, 0x18, 0x5C, 1, SWAY);
    }
    angles(fx, t, 0x01E0_22D8);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x30);
}

/// 0x1E01C90: a ring: quad size +0x48 25..44, a random cell frame 0..2,
/// radius +0x50 10..49 growing by +0xE 26..55 a update, row +8 0..34,
/// life +6 3..5; fades in by 0x40 below 0xC0, then out by 14.
fn ring(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rand_mod(fx, t, 0x01E0_1CAC, 0x14);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0x19));
        let v = rand_mod(fx, t, 0x01E0_1CD8, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rand_mod(fx, t, 0x01E0_1D04, 0x28);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rand_mod(fx, t, 0x01E0_1D24, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rand_mod(fx, t, 0x01E0_1D40, 0x1E);
        fx.mem.set16(t + 0xE, v + 0x1A);
        let v = rand_mod(fx, t, 0x01E0_1D60, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, t, 0x01E0_1D7C, 3);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a >= 0x10 {
        fx.mem.write_u8(t + 0x63, a - 0xE);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E01E40: a spark burst in one of four colours (+0x50 = (rand >> 4) %
/// 4: red, pink, blue, pale blue): radii 200 / 142 growing by a random
/// 15.0..22.9 a update (outer = 0.1 x inner), 60-degree sparks of 2;
/// full alpha for 8 updates, then out by 5.
fn burst(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4348_0000);
        fx.mem.setf(t + 0x4C, ee::sub(200.0, 58.0));
        let v = rand_at(fx, t, 0x01E0_1E78);
        fx.mem.set16(t + 0x50, (v >> 4) % 4);
        let colours = [((0xC0, 0x20, 0x20), 0), ((0xFF, 0xA0, 0xA0), 0x30), ((0x20, 0x20, 0xC0), 0), ((0xA0, 0xA0, 0xFF), 0x30)];
        for (k, ((r, g, b), o)) in colours.into_iter().enumerate() {
            if fx.mem.i16(t + 0x50) == k as i32 {
                l2::rgb(fx, t + 0x60, r, g, b);
                l2::rgb(fx, t + 0x64, o, o, o);
            }
        }
        fx.mem.set16(t + 0x5A, 0x3C);
        fx.mem.set16(t + 0x5C, 2);
        let v = rand_mod(fx, t, 0x01E0_1F84, 0x50);
        fx.mem.setf(t + 0x34, ee::from_i32(v + 0x96));
        let s = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, s);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        // The third angle lands in +0x20 again [F 0x1E02020].
        for (o, site) in [(0x20u32, 0x01E0_1FCCu32), (0x24, 0x01E0_1FE8), (0x20, 0x01E0_2004)] {
            let v = rand_mod(fx, t, site, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
    }
    if fx.mem.i16(t + 0x54) < 8 {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 7 {
            fx.mem.write_u8(t + 0x63, a - 5);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let r = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, r);
    let o = ee::mul(f32::from_bits(0x3DCC_CCCD), fx.mem.f(t + 0x48));
    fx.mem.setf(t + 0x4C, o);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E02300: the arc fan (b236's 0x1E01B70 with white outer / blue inner
/// colours): outer radius 220, 10-degree arcs every 10 degrees; its outer
/// alpha up by 0x60 below 0xC0 for 14 updates, then down by 9; faces the
/// camera turned by 0x4000 a update about x.
fn fan(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x435C_0000);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x60, 0x40, 0x40, 0xFF);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let b = fx.mem.read_u8(t + 0x67);
    if fx.mem.i16(t + 0x54) < 0xE {
        if b < 0xC0 {
            fx.mem.write_u8(t + 0x67, b + 0x60);
        }
    } else if b >= 0xB {
        fx.mem.write_u8(t + 0x67, b - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    angles(fx, t, 0x01E0_23C0);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E023F0: the stage light (0x1F7F10): +0x50 from 0x80 down by 10 to
/// 0x30 for 151 updates; once the timer is below 30 back up by 10 to
/// 0x80; 0x80 and the task ends when the timer reads 3.
fn light(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x97 {
        let v = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if v < 0x31 { 0x30 } else { v - 0xA });
    } else if fx.mem.i32(TIMER) < 0x1E {
        let v = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if v < 0x80 { v + 0xA } else { 0x80 });
        if fx.mem.i16(t + 0x50) >= 0x81 {
            fx.mem.set16(t + 0x50, 0x80);
        }
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    if fx.mem.i32(TIMER) == 3 {
        fx.set_stage_light(0x80);
        fx.mem.set16(t + 0x58, 0);
    } else {
        let v = fx.mem.read_u8(t + 0x50);
        fx.set_stage_light(i32::from(v));
    }
    fx.leave(0x20);
}
