//! BMODULE b403 (card 250), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01750, data area 0x1ED0000, 0x3470 bytes) [F]
//!: 180 tasks of 0x40 at D+0x300 (alive +0x36,
//! step +0x24), spawned by 0x1E01D90 (first free; colour D+0x31A4, point
//! D+0x250, kind D+0x31F2 into +0x30, D+0x31EA into +0x3A). Globals: timer
//! D+0x31A0 = 150, OT D+0x31AC, nodes D+0x31D8.. (the attacker's 0x31
//! (clear), 0x4D, 0x16, 0x17, 0x18, 0x1B, 0x1A, 0x19), param D+0x31F4,
//! counter D+0x31F6, bytes D+0x31F8 (rand % 100 - 50), sines D+0x32F8,
//! textures (4, 7..0xF) D+0x2DC.., camera block D+0x3000.
//!
//! The emitter 0x1E00CF0: update 1 the stage light 0x1E015F0 (0x80, down
//! by 10 to 0x30 while the timer is above 20, then back up) and the object
//! op 0x1E016D0 (0x1F8380(3, ..), run inline by the entry, over on timer
//! 10); on 0x57 at node 0x31 fifty drifting sprites 0x1060 (0x1E02460),
//! fifty rings 0x1210 (`billboard::ring_t`), the disc c7cfb5b8be8e
//! (`quads4::band18`) and the camera 0x1460; on 0x57 / 0x5C / 0x62 / 0x67 a
//! growing fan 0x0EF0 (0x1E01EC0, nine quads). From update 0x15 D+0x290 /
//! 0x298 rise by 0.01 to 0.5, D+0x294 = 0.1. Returns 1 on update 0x4D, 4
//! on 0x82, 2 on 0x57, 8 once the timer has run out.
//!
//! The draw 0x1E02120 is reached only by tasks of step 0x1E01050 and the
//! 0x1E02460 draw's texture (4, 2) only by 0x1E01200: both are two-word
//! stubs nobody spawns [F], so neither is ported. No 0x1F8100.
use crate::b076::node_position_saving;
use crate::b148::rmod;
use crate::blib::{billboard, quads4, spawn, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 180;
const POOL: u32 = D + 0x300;
const TIMER: u32 = D + 0x31A0;
const COLOUR: u32 = D + 0x31A4;
const OT2: u32 = D + 0x31AC;
const NODES: u32 = D + 0x31D8;
const KIND: u32 = D + 0x31F2;
const COUNTER: u32 = D + 0x31F6;
const TABLE: u32 = D + 0x31F8;
const SINES: u32 = D + 0x32F8;
const TEX: u32 = D + 0x2FC;
const CAM: u32 = D + 0x3000;
const SPR: u32 = 0x7000_0000;

const FAN: u32 = 0x01E0_0EF0;
const DRIFT: u32 = 0x01E0_1060;
const RING: u32 = 0x01E0_1210;
const DISC: u32 = 0x01E0_1380;
const CAMERA: u32 = 0x01E0_1460;
const LIGHT: u32 = 0x01E0_15F0;
const OBJECT: u32 = 0x01E0_16D0;

/// The entry 0x1E01750 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x140);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(OT2, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp) };
    fx.leave(0x140);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3470);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x31F4, param);
    fx.mem.set32(TIMER, 0x96);
    for (k, n) in [0x31, 0x4D, 0x16, 0x17, 0x18, 0x1B, 0x1A, 0x19].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    for k in 7..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x2DC + 4 * (k as u32 - 7), t);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_18A8, 0x64);
        fx.mem.set8(TABLE + k, v - 0x32);
    }
    0
}

/// A step's frame below the entry's sp: the entry's s0 = D / s1 = 0 (the
/// first `n`) and the return address `ra` above them.
fn frame(fx: &mut Fx, low: u32, size: u32, n: u32, ra: u32) {
    let base = low - size;
    if n > 0 {
        fx.saved(base, &[(0, u64::from(D))]);
    }
    if n > 1 {
        fx.saved(base + 0x10, &[(0, 0)]);
    }
    fx.saved(base, &[(0x10 * n, u64::from(ra))]);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    let low = fx.sp;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            FAN => {
                fan(fx, FAN, t);
                frame(fx, low, 0x30, 2, 0x01E0_1918);
            }
            DRIFT => {
                drift(fx, DRIFT, t);
                frame(fx, low, 0x30, 2, 0x01E0_1938);
            }
            RING => {
                ring(fx, RING, t);
                frame(fx, low, 0x20, 1, 0x01E0_1958);
            }
            DISC => {
                let c = spawn::disc("c7cfb5b8be8e").expect("constants");
                spawn::disc_step_with(fx, c, DISC, t);
                frame(fx, low, 0x20, 1, 0x01E0_1978);
            }
            CAMERA => {
                camera(fx, CAMERA, t);
                frame(fx, low, 0x30, 2, 0x01E0_1998);
            }
            LIGHT => {
                light(fx, t);
                frame(fx, low, 0x10, 0, 0x01E0_19B8);
            }
            OBJECT => {
                for (to, from) in [(0x13Cu32, 0x2680u32), (0x138, 0x2688)] {
                    let w = fx.mem.read_u32(0x01E0_0000 + from);
                    fx.mem.write_u32(sp + to, w);
                }
                fx.boundary(0x001F_8380, "object op 0x1F8380", [3, 0, sp + 0x13C, sp + 0x138]);
                if fx.mem.i32(TIMER) == 0xA {
                    fx.mem.set16(t + 0x36, 0);
                }
            }
            f => {
                debug_assert!(false, "b403: unported task step {f:#x}");
            }
        }
    }
    let d = sp + 0xA8;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x24);
        let set_rect = |fx: &mut Fx, local: u32| {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x34);
            l2::rect(fx, local, 0x9B, sub);
            for k in 0..4 {
                let v = fx.mem.read_u32(local + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, v);
            }
        };
        if f(fx) == FAN {
            fan_draw(fx, t, d, 0);
            fan_draw(fx, t, d, 1);
        }
        if f(fx) == DRIFT {
            drift_draw(fx, 0x01E0_2460, t, d, 0);
            drift_draw(fx, 0x01E0_2460, t, d, 1);
        }
        if f(fx) == RING {
            set_rect(fx, sp + 0x80);
            fx.mem.set16(D + 0xB8, 0xE);
            billboard::ring_t(fx, uv::uv_rect4, D, t, d, TABLE);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == DISC {
            set_rect(fx, sp + 0x90);
            let p = sp + 0x70;
            for c in 0..3u32 {
                let v = ee::from_i32(fx.mem.i16(t + 2 * c));
                fx.mem.setf(p + 4 * c, v);
            }
            quads4::band18(fx, D, t, 1, p);
        }
    }
    if fx.mem.i16(COUNTER) >= 0x15 {
        for o in [0x290u32, 0x298] {
            let v = fx.mem.f(D + o);
            if v < 0.5 {
                fx.mem.setf(D + o, ee::add(v, f32::from_bits(0x3C23_D70A)));
            }
            if o == 0x290 {
                fx.mem.write_u32(D + 0x294, 0x3DCC_CCCD);
            }
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x4D => 1,
        0x82 => 4,
        0x57 => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E01D90(D, step, 0): the first free task cleared and set off at the
/// point D+0x250 with the colour D+0x31A4 (twice, alpha included), +0x30
/// the kind D+0x31F2, +0x3A D+0x31EA; frame 0x60 with the emitter's s0 = D
/// / s1, the entry's s2 = param (-1) and s3 / s4 the runner's (0 [H]).
fn spawn_task(fx: &mut Fx, step: u32, s1: u64, ra: u32) {
    spawn_regs(fx, step, [u64::from(D), s1, u64::MAX, 0, 0], ra);
}

/// [`spawn_task`] with the caller's s0..s4 `s` (b112 / card 234 shares
/// the spawn word for word).
pub(crate) fn spawn_regs(fx: &mut Fx, step: u32, s: [u64; 5], ra: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0, s[0]), (0x10, s[1]), (0x20, s[2]), (0x30, s[3]), (0x40, s[4]), (0x50, u64::from(ra))]);
    if let Some(k) = (0..TASKS).find(|&k| fx.mem.i16(POOL + 0x36 + 0x40 * k) == 0) {
        let t = POOL + 0x40 * k;
        fx.bzero(t, 0x40);
        fx.mem.set16(t + 0x36, 1);
        for o in [0x28u32, 0x2C] {
            let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
            l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
            let a = fx.mem.read_u8(COLOUR + 3);
            fx.mem.write_u8(t + o + 3, a);
        }
        l2::to_i16x4(fx, t, D + 0x250, 0);
        let s = fx.mem.i16(KIND);
        fx.mem.set16(t + 0x30, s);
        let a = fx.mem.i16(D + 0x31EA);
        fx.mem.set16(t + 0x3A, a);
        fx.mem.write_u32(t + 0x20, 0);
        fx.mem.write_u32(t + 0x24, step);
    }
    fx.leave(0x60);
}

/// 0x1E00CF0(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the spawns (frame 0x30: the entry's s0 = D, s1 = 0, ra; its s0 =
/// D, s1 the loops' counts).
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0, u64::from(D)), (0x10, 0), (0x20, 0x01E0_18E4)]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let at = |fx: &mut Fx| {
        for k in 0..3 {
            let w = fx.mem.read_u32(D + 0x3190 + 4 * k);
            fx.mem.write_u32(D + 0x250 + 4 * k, w);
        }
    };
    if c(fx) == 0x57 {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        node_position_saving(fx, h, slot, D + 0x3190, u64::from(D), Some(0), 0x01E0_0D4C);
    }
    if c(fx) == 1 {
        spawn_task(fx, LIGHT, 0, 0x01E0_0D70);
    }
    if c(fx) == 1 {
        spawn_task(fx, OBJECT, 0, 0x01E0_0D94);
    }
    if c(fx) == 0x57 {
        at(fx);
        for i in 0..0x32u64 {
            spawn_task(fx, DRIFT, i, 0x01E0_0DD4);
        }
    }
    if [0x57, 0x5C, 0x62, 0x67].contains(&c(fx)) {
        at(fx);
        fx.mem.set16(KIND, 0);
        spawn_task(fx, FAN, 0, 0x01E0_0E50);
    }
    if c(fx) == 0x57 {
        at(fx);
        for i in 0..0x32u64 {
            spawn_task(fx, RING, i, 0x01E0_0EA0);
        }
        spawn_task(fx, DISC, 0x32, 0x01E0_0EC4);
        spawn_task(fx, CAMERA, 0x32, 0x01E0_0ED8);
    }
    fx.leave(0x30);
}

fn fix(fx: &mut Fx, v: f32, ra: u32) -> i32 {
    fx.fix_residue(v, ra);
    ee::fixsfsi(v)
}

/// 0x1E00EF0 (D, task): a growing fan (size 8 up by 2, +0x10 0x4000, cell
/// rand % 7, life 1, kind 0 (0x40, 0x20, 0) else (0, 0x80, 0x80)): alpha up
/// by 0x50 (below 0xA0) for 2 updates, then from size 36 down by 8 (over
/// below 0xA); from timer 9 down by 15 (over below 0x11).
pub(crate) fn fan(fx: &mut Fx, at: u32, t: u32) {
    fx.enter(0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 8);
        fx.mem.set32(t + 0x10, 0x4000);
        fx.mem.write_u32(t + 0x14, 0);
        let v = rmod(fx, t, at + 0x3C, 7);
        fx.mem.set16(t + 0x34, v);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.set16(t + 6, 1);
        if fx.mem.i16(t + 0x30) == 0 {
            l2::rgb(fx, t + 0x28, 0x40, 0x20, 0);
        } else {
            l2::rgb(fx, t + 0x28, 0, 0x80, 0x80);
        }
    }
    if fx.mem.i16(t + 0x32) < 2 {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0xA0 {
            fx.mem.write_u8(t + 0x2B, a + 0x50);
        }
    } else {
        if fx.mem.i16(t + 0x38) >= 0x24 {
            let a = fx.mem.read_u8(t + 0x2B);
            if a >= 0xA {
                fx.mem.write_u8(t + 0x2B, a - 8);
            } else {
                fx.mem.set16(t + 0x36, 0);
            }
        }
        if fx.mem.i32(TIMER) < 0xA {
            let a = fx.mem.read_u8(t + 0x2B);
            if a >= 0x11 {
                fx.mem.write_u8(t + 0x2B, a - 0xF);
            } else {
                fx.mem.set16(t + 0x36, 0);
            }
        }
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    let s = fx.mem.i16(t + 0x38) + 2;
    fx.mem.set16(t + 0x38, s);
    fx.leave(0x30);
}

/// 0x1E01060 (D, task): a drifting sprite (size 13, cell 2, (0x60, 0x40,
/// 0), +0x30 / +0x3A rand % 360): 0x80 for 10 updates, then out by 10;
/// drifting by 9 sin +0x3A (x), 9 cos +0x30 (y), 9 cos +0x3A (z).
pub(crate) fn drift(fx: &mut Fx, at: u32, t: u32) {
    fx.enter(0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0xD);
        fx.mem.set16(t + 0x34, 2);
        l2::rgb(fx, t + 0x28, 0x60, 0x40, 0);
        let v = rmod(fx, t, at + 0x50, 0x168);
        fx.mem.set16(t + 0x30, v);
        let v = rmod(fx, t, at + 0x6C, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0xA {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0x11 {
            fx.mem.write_u8(t + 0x2B, a - 0xA);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    for (o, deg, cos, ra) in [(8u32, 0x3Au32, 0, 0x108u32), (0xA, 0x30, 1, 0x140), (0xC, 0x3A, 1, 0x178)] {
        let a = fx.mem.i16(t + deg);
        let s = l2::sin_deg(fx, SINES, a, cos);
        let v = ee::add(ee::from_i32(fx.mem.i16(t + o)), ee::mul(9.0, s));
        let r = fix(fx, v, at + ra);
        fx.mem.set16(t + o, r);
    }
    fx.leave(0x30);
}

/// 0x1E01210 (D, task): a ring (size 25, cell 0..3, (0x20, 0x10, 8),
/// +0x3A rand % 360, +0xE 3..18, row +8 rand % 10, life 5..11): in by 0x38
/// (below 0x71) for its life, then out by 10; radius up by +0xE after the
/// first update.
pub(crate) fn ring(fx: &mut Fx, at: u32, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x19);
        let v = fx.rand_from(t, at + 0x28);
        fx.mem.set16(t + 0x34, v % 4);
        l2::rgb(fx, t + 0x28, 0x20, 0x10, 8);
        fx.mem.set16(t + 0x30, 0);
        let v = rmod(fx, t, at + 0x5C, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = fx.rand_from(t, at + 0x78);
        fx.mem.set16(t + 0xE, v % 16 + 3);
        let v = rmod(fx, t, at + 0x98, 0xA);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, at + 0xB4, 7);
        fx.mem.set16(t + 6, v + 5);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < 0x71 {
            fx.mem.write_u8(t + 0x2B, a + 0x38);
        }
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x2B, a - 0xA);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x20);
}

/// 0x1E01460 (D, task): camera 0's slots 4 / 5 (0x1F7E20) from the block
/// D+0x3000: on the first update (128, 6, 3, 0), the task's i16 point as
/// floats, (0, 153, 0), weight 0; the weight up by 0.2 while below 1 (then
/// +0x30 = 1), then down by 0.08 (the task over once at most 0.1); the
/// slots cleared on its last update; also over on timer 1. Frame 0x30.
pub(crate) fn camera(fx: &mut Fx, at: u32, t: u32) {
    let sp = fx.enter(0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x40C0_0000), (8, 0x4040_0000), (0xC, 0), (0x20, 0), (0x24, 0x4319_0000), (0x28, 0)] {
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
        fx.mem.setf(CAM + 0x28, ee::sub(w, f32::from_bits(0x3DA3_D70A)));
    }
    let (p, ra) = if fx.mem.i16(t + 0x36) == 0 { (0, at + 0x144) } else { (CAM, at + 0x158) };
    crate::blib::exe::camera_slots45(fx, p);
    // 0x1F7E20's frame (s0 = the task, ra).
    fx.saved(sp - 0x20, &[(0, u64::from(t)), (0x10, u64::from(ra))]);
    if fx.mem.i32(TIMER) == 1 {
        fx.mem.set16(t + 0x36, 0);
    }
    fx.leave(0x30);
}

/// 0x1E015F0 (D, task): the stage light +0x30 (0x80 at first): down by 10
/// to 0x30 while the timer is above 20, then up by 10 to 0x80 (the task
/// over there); 0x1F7F10 every update. Frame 0x10 (ra).
fn light(fx: &mut Fx, t: u32) {
    fx.enter(0x10);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x30, 0x80);
        fx.mem.set16(t + 0x36, 2);
    }
    let v = fx.mem.i16(t + 0x30);
    if fx.mem.i32(TIMER) >= 0x15 {
        fx.mem.set16(t + 0x30, if v < 0x31 { 0x30 } else { v - 0xA });
    } else {
        if v < 0x80 {
            fx.mem.set16(t + 0x30, v + 0xA);
        } else {
            fx.mem.set16(t + 0x30, 0x80);
            fx.mem.set16(t + 0x36, 0);
        }
        if fx.mem.i16(t + 0x30) >= 0x81 {
            fx.mem.set16(t + 0x30, 0x80);
        }
    }
    let l = fx.mem.read_u8(t + 0x30);
    fx.set_stage_light(i32::from(l));
    fx.leave(0x10);
}

/// 0x1E01EC0 (D, task, desc, mode): mode 0 sets `desc` up (a strip3, the
/// rect 0x97 frame +0x34, colour +0x28, texture D+0x2FC); mode 1 draws
/// nine quads of a fan, 40 degrees each from +0x3A between the centre and
/// radius size, turned by +0x10, scaled (1, 1.5, 1) at the task's i16
/// point; keyed by mean w (D+0x31C8) into D+0x31AC.
pub(crate) fn fan_draw(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    use ee::mul;
    let sp = fx.enter(0x130);
    if mode == 0 {
        let sub = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0x120, 0x97, sub);
        l2::desc_strip3(fx, d);
        l2::colour_strip3(fx, d, t + 0x28, 0);
        l2::uv_strip3(fx, d, sp + 0x120, 0);
        let tex = fx.mem.read_u32(TEX);
        fx.desc_texture(d, tex, 1);
        fx.leave(0x130);
        return;
    }
    for k in 0..3 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(sp + 0xC0 + 4 * k, v);
    }
    for (o, v) in [(0xD0u32, 0x3F80_0000u32), (0xD4, 0x3FC0_0000), (0xD8, 0x3F80_0000)] {
        fx.mem.write_u32(sp + o, v);
    }
    l2::xform_vp(fx, sp + 0xE0, sp + 0xD0, t + 0x10, sp + 0xC0);
    for o in [0x84u32, 0x94, 0xA4, 0xB4] {
        fx.mem.write_u32(sp + o, 0);
    }
    let mut s1 = 0i32;
    for _ in 0..9 {
        let a = (fx.mem.i16(t + 0x3A) + s1) as i16 as i32;
        let c = l2::sin_deg(fx, SINES, a, 1);
        let r = ee::from_i32(fx.mem.i16(t + 0x38));
        fx.mem.setf(sp + 0xA0, mul(r, c));
        let s = l2::sin_deg(fx, SINES, a, 0);
        let r = ee::from_i32(fx.mem.i16(t + 0x38));
        fx.mem.setf(sp + 0xA8, mul(r, s));
        fx.mem.write_u32(sp + 0x80, 0);
        fx.mem.write_u32(sp + 0x88, 0);
        let b = (s1 + (fx.mem.i16(t + 0x3A) + 0x28)) as i16 as i32;
        let c = l2::sin_deg(fx, SINES, b, 1);
        let r = ee::from_i32(fx.mem.i16(t + 0x38));
        fx.mem.setf(sp + 0xB0, mul(r, c));
        let s = l2::sin_deg(fx, SINES, b, 0);
        let r = ee::from_i32(fx.mem.i16(t + 0x38));
        fx.mem.setf(sp + 0xB8, mul(r, s));
        fx.mem.write_u32(sp + 0x90, 0);
        fx.mem.write_u32(sp + 0x98, 0);
        if l2::project(fx, sp + 0x80, SPR, sp + 0xE0, d, D + 0x31C8) {
            let key = ee::fixunssfsi(fx.mem.f(D + 0x31C8)) as i32;
            let ot = fx.mem.read_u32(OT2);
            fx.submit(d, ot, key);
        }
        s1 = (s1 + 0x28) as i16 as i32;
    }
    fx.leave(0x130);
}

/// 0x1E02460 (D, task, desc, mode): mode 0 sets `desc` up (a strip3, the
/// rect 0x9B frame +0x34, colour +0x28, texture D+0x2FC); mode 1 one
/// square of the i16 size +0x38 facing the camera (0x205580) at the task's
/// i16 point plus its drift / 10 (scale 0.1), keyed by its mean w into the
/// OT (0x109870). `at` the draw's record address (b112: 0x1E04260).
pub(crate) fn drift_draw(fx: &mut Fx, at: u32, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x120);
    let ot = OT;
    if mode == 0 {
        let sub = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0xC0, 0x9B, sub);
        l2::desc_strip3(fx, d);
        l2::colour_strip3(fx, d, t + 0x28, 0);
        l2::uv_strip3(fx, d, sp + 0xC0, 0);
        let tex = fx.mem.read_u32(TEX);
        fx.desc_texture(d, tex, 1);
        fx.leave(0x120);
        return;
    }
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xB0 + 4 * k, 0x3DCC_CCCD);
    }
    crate::b494::camera_angles_saving(fx, sp + 0x110, u64::from(ot), u64::from(d), at + 0x138);
    for k in 0..3 {
        let p = ee::from_i32(fx.mem.i16(t + 2 * k));
        let o = ee::div(ee::from_i32(fx.mem.i16(t + 8 + 2 * k)), 10.0);
        fx.mem.setf(sp + 0xA0 + 4 * k, ee::add(p, o));
    }
    l2::xform_vp(fx, sp + 0xD0, sp + 0xB0, sp + 0x110, sp + 0xA0);
    let s = ee::from_i32(fx.mem.i16(t + 0x38));
    l2::quad_xy(fx, sp + 0x60, s, s);
    l2::project_submit(fx, sp + 0x60, d, sp + 0xD0, ot);
    fx.leave(0x120);
}
