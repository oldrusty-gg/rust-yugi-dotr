//! BMODULE b236 (card 628), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01D80, data area 0x1ED0000) [F]: b631's task pool of 400 (0x70 bytes at +0x2A0, alive +0x58, step
//! +0x44 as a record address) drawn with the second library: spark
//! bursts (`quads4::sparks`, 21cb40d76bec at 0x1E01230), rings
//! (`billboard::ring_quads`, 0x1E00920, UVs by uv_rect 0x1E00080) and an
//! arc fan (`billboard::arc_strips`, 0x1E00ED0). The stage light dims to
//! 0x30 over the first 20 updates (0x1E01C60, 0x1F7F10 every update) and
//! comes back by 10 a update for the last 20. On update 52 the attacker's
//! node 0x24 is taken; on updates 52..62 a violet spark burst and three
//! rings start there each update, on 52 also the arc fan. Returns 1 on
//! update 42, 4 on 85, 2 on 75, 6 on 53..69, 8 once the 90-update timer
//! +0xB210 has run out.
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
/// The step functions a task holds (+0x44), as the record's addresses.
const BURST: u32 = 0x01E0_17E0;
const RING: u32 = 0x01E0_19C0;
const FAN: u32 = 0x01E0_1B70;
const LIGHT: u32 = 0x01E0_1C60;
const IDLE: u32 = 0x01E0_1D70;
/// The return address of the entry's `jalr` into a step (0x1E02080 + 8).
const TASK_RA: u64 = 0x01E0_2088;
const TIMER: u32 = D + 0xB210;
const COLOUR: u32 = D + 0xB214;
const COUNTER: u32 = D + 0xB2B8;
const BYTES: u32 = D + 0xB2BA;
/// The attacker's node 0x24 taken on update 52.
const POINT: u32 = D + 0xB1D0;

/// The entry 0x1E01D80.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x80);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, D + 0xB4BC);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2B6, param);
    fx.mem.set32(TIMER, 0x5A);
    for (at, node, clear) in [(0xB244u32, 0x24, 1), (0xB246, 0x34, 0), (0xB248, 0x35, 0), (0xB24A, 0x30, 0), (0xB24C, 0x2F, 0), (0xB24E, 0x2C, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(D + at, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB254, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    let t = fx.texture(5, 0);
    fx.mem.write_u32(D + 0xB1F0, t);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x70);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x74)), 180.0);
    fx.mem.set16(D + 0xB2B4, ee::fixsfsi(y));
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
        let t = D + 0x70 * k;
        if fx.mem.i16(t + 0x2F8) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x2E4) == BURST {
            let sub = fx.mem.i16(t + 0x2F6);
            cell(fx, sp + 0x50, 0x97, sub);
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(t + 0x2B0 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, w);
            }
            fx.mem.set16(D + 0xB8, 0xC);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sparks(fx, D, t + 0x2A0, 0x20, t + 0x2B0);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x2E4) == RING {
            let w = fx.mem.read_u32(D + 0xB1F0);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x2F6);
            cell(fx, sp + 0x60, 0x14, sub);
            fx.mem.set16(D + 0xB8, 4);
            let v = fx.rand();
            fx.mem.set16(D + 0xBE, v % 4);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t + 0x2A0, 0x21, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x2E4) == FAN {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t + 0x2A0, 0x21, t + 0x2B0);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x2A => 1,
        0x55 => 4,
        0x4B => 2,
        0x35..=0x45 => 6,
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

fn run_task(fx: &mut Fx, f: u32, t: u32) {
    match f {
        BURST => burst(fx, t),
        RING => ring(fx, t),
        FAN => fan(fx, t),
        LIGHT => light(fx, t),
        _ => panic!("b236: unknown task step {f:#x}"),
    }
}

/// 0x1E01630(D, 0): the update counter +0xB2B8 and the emitters (frame
/// 0x20: ra, the entry's s0 = 0).
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, 0x01E0_2048), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if fx.mem.i16(COUNTER) == 0x34 {
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + 0xB244);
        fx.node_position(h, slot, POINT);
    }
    let from_point = |fx: &mut Fx| {
        for k in 0..4 {
            let w = fx.mem.read_u32(POINT + 4 * k);
            fx.mem.write_u32(D + 0x250 + 4 * k, w);
        }
        fx.mem.set16(D + 0xB2B0, 0);
    };
    if (0x34..0x3F).contains(&fx.mem.i16(COUNTER)) {
        from_point(fx);
        spawn(fx, BURST, 0x01E0_1704);
        for ra in [0x01E0_1718, 0x01E0_172C, 0x01E0_1740] {
            spawn(fx, RING, ra);
        }
    }
    if fx.mem.i16(COUNTER) == 0x34 {
        from_point(fx);
        spawn(fx, FAN, 0x01E0_17A0);
    }
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, LIGHT, 0x01E0_17CC);
    }
    fx.leave(0x20);
}

/// 0x1E02330(D, step, 0): b631's spawn (the first free task cleared and
/// set off at D+0x250 with the colour +0xB214 twice, angle flag +0xB2B0);
/// its saves: ra, the emitter's s0 = D and the entry's s1 = D / s2 = -1.
fn spawn(fx: &mut Fx, f: u32, ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, u64::MAX), (0x10, u64::from(D)), (0, u64::from(D))]);
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

/// rand % m (HI) from a step's `jal` at `site`: its frame (s0 = the task
/// `t`) lands at the entry's sp - 0x30, a later quad's stale TEX1.
fn rand_at(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, site + 8);
    fx.div(v, m).1
}

/// 0x1E017E0: a violet spark burst: radii 120 / 62 growing by a random
/// 20.0..27.9 a update (outer = 0.2 x inner), random angles; full alpha
/// for 10 updates, then out by 7.
fn burst(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42F0_0000);
        fx.mem.setf(t + 0x4C, ee::sub(120.0, 58.0));
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0x80, 0x40, 0xFF);
            l2::rgb(fx, t + 0x64, 0, 0, 0);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0xA0, 0x80, 0xFF);
            l2::rgb(fx, t + 0x64, 0x30, 0x30, 0x30);
        }
        fx.mem.set16(t + 0x5A, 0x1E);
        fx.mem.set16(t + 0x5C, 0x1E);
        let v = rand_at(fx, t, 0x01E0_1890, 0x50);
        fx.mem.setf(t + 0x34, ee::from_i32(v + 0xC8));
        let s = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, s);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        for (o, site) in [(0x20u32, 0x01E0_18D8u32), (0x24, 0x01E0_18F4), (0x28, 0x01E0_1910)] {
            let v = rand_at(fx, t, site, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
    }
    if fx.mem.i16(t + 0x54) < 0xA {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 9 {
            fx.mem.write_u8(t + 0x63, a - 7);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let r = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, r);
    let o = ee::mul(f32::from_bits(0x3E4C_CCCD), fx.mem.f(t + 0x48));
    fx.mem.setf(t + 0x4C, o);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E019C0: a ring: quad size +0x48 20..59, a random cell frame 0..2,
/// radius +0x50 10..49 growing by +0xE 10..29 a update, row +8 0..34,
/// life +6 0..14; fades in by 0x40 below 0xC0, then out by 14.
fn ring(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rand_at(fx, t, 0x01E0_19DC, 0x28);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0x14));
        let v = rand_at(fx, t, 0x01E0_1A08, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rand_at(fx, t, 0x01E0_1A34, 0x28);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rand_at(fx, t, 0x01E0_1A54, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rand_at(fx, t, 0x01E0_1A70, 0x14);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = rand_at(fx, t, 0x01E0_1A90, 0x23);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, 0x01E0_1AB4);
        fx.mem.set16(t + 6, v % 8 + 7);
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

/// 0x1E01B70: the arc fan: outer radius 220, violet, 10-degree arcs
/// every 10 degrees; its outer alpha +0x67 up by 0x60 below 0xC0 for 14
/// updates, then down by 9; faces the camera (0x205580) turned by 0x4000
/// a update about x.
fn fan(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x435C_0000);
        l2::rgb(fx, t + 0x64, 0x80, 0x40, 0xFF);
        l2::rgb(fx, t + 0x60, 0x80, 0x40, 0xFF);
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
    // 0x205580's saves (ra, the entry's s1 = D, s0 = t) below the step.
    let base = fx.sp.wrapping_sub(0x30);
    fx.saved(base, &[(0x20, 0x01E0_1C30), (0x10, u64::from(D)), (0, u64::from(t))]);
    l2::camera_angles_to(fx, t + 0x20);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E01C60: the stage light (0x1F7F10): +0x50 from 0x80 down by 10 to
/// 0x30 for 20 updates; once the timer is below 20 back up by 10 to 0x80;
/// 0x80 and the task ends when the timer reads 3.
fn light(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x14 {
        let v = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if v < 0x31 { 0x30 } else { v - 0xA });
    } else if fx.mem.i32(TIMER) < 0x14 {
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
