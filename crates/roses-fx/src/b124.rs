//! BMODULE b124 (card 275), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02190, data area 0x1ED0000, 0x3B90 bytes) [F]
//!: b361's program on b003's older pool - 210
//! tasks of 0x40 bytes at D+0x2E0 (alive +0x36, age +0x32, step +0x24,
//! each step called by its own `jal`; spawned by 0x1E027C0) drawn as
//! spinning sprites (f7e939ff758a at 0x1E00D90, `billboard::sprite_spin_40`)
//! and a flaming disc (0cb01f69a41a at 0x1E00F80, `quads4::fan18`), plus a
//! unit flash (0x1E028E0: the attacker drawn into the target in a packet
//! layer; its tasks 0x1E01C00 (a widening sprite from the attacker's
//! screen point) and 0x1E01F30 (the target over the whole window) stepped
//! there). Updates 0xDE..0x159 a zoom task a update, on 0xDE the window
//! task and the disc (`spawn::disc_step` copy 38960db3edd0: size 1830,
//! 120 updates at 0x80) 60 above the attacker; 0xDC the stage light
//! (0x1E01A30); 0x102..0x154 4..12 grey sprites a update round node 1 (12
//! lower) drifting 7 a update toward the facing and sinking 2. Returns 2
//! on update 0x159, 6 on 0xE7..0x153, 8 once the 0x1CE-update timer
//! D+0x37D0 has run out.
//!
//! The steps 0x1E01500 (a ring drawn by `quads4::ring_spun`), 0x1E018B0
//! and 0x1E01B00 are tested by the entry but never spawned [F: the
//! emitter's spawns name 0x1E01C00 / 0x1E01F30 / 0x1E017D0 / 0x1E01A30 /
//! 0x1E01680 only]; not ported.
use crate::b590::project_residue;
use crate::blib::{billboard, quads4, spawn, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 210;
const POOL: u32 = D + 0x2E0;
const POS: u32 = D + 0x250;
const SPRITE: u32 = 0x01E0_1680;
const DISC: u32 = 0x01E0_17D0;
const LIGHT: u32 = 0x01E0_1A30;
const ZOOM: u32 = 0x01E0_1C00;
const WINDOW: u32 = 0x01E0_1F30;
const TEX5: u32 = D + 0x37B0;
const TIMER: u32 = D + 0x37D0;
const COLOUR: u32 = D + 0x37D4;
/// The zoom's projected w (0x204D80's).
const MEAN: u32 = D + 0x37E8;
const LAYER: u32 = D + 0x37FC;
const FLASH_DESC: u32 = D + 0x3800;
const NODES: u32 = D + 0x3804;
const KIND: u32 = D + 0x381C;
const FACING: u32 = D + 0x381E;
const COUNTER: u32 = D + 0x3822;
const BYTES_B: u32 = D + 0x3924;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02190 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x140);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x37DC, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x140);
    ret
}

fn facing_of(fx: &mut Fx, out: u32, ra: u32) -> i32 {
    let h = fx.self_handle();
    fx.unit_rotation(h, out);
    let y = ee::div(ee::from_i32(fx.mem.i32(out + 4)), 180.0);
    fx.fix_residue(y, ra);
    ee::fixsfsi(y)
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x3B90);
    l2::sine_table(fx, D + 0x3A24);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x3820, param);
    fx.mem.set32(TIMER, 0x1CE);
    for (k, n) in [1, 0xD, 0x36, 0x37, 0x1F, 0x34, 0x36, 0x37].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    // 0x1E020E0(0, D): the textures (frame 0x40: ra, the entry's s2 = s1 =
    // 0, s0 = D).
    let f = fx.enter(0x40);
    fx.saved(f, &[(0x30, 0x01E0_22C4), (0x20, 0), (0x10, 0), (0, u64::from(D))]);
    for k in 0..3 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x2A0 + 4 * k as u32, t);
    }
    for k in 0xD..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x2D4 + 4 * (k as u32 - 0xD), t);
    }
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX5 + 4 * k as u32, t);
    }
    fx.leave(0x40);
    let f = facing_of(fx, sp + 0x90, 0x01E0_2300);
    fx.mem.set16(FACING, f);
    // rand's frames: the entry's s0 = D.
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_230C, 0x1E);
        fx.mem.set8(D + 0x3824 + k, v - 0xF);
    }
    for k in 0..0x64u32 {
        fx.mem.write_u8(BYTES_B + k, 0);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, D, 0x01E0_2360, 5);
        fx.mem.set8(BYTES_B + k, v);
    }
    let f = facing_of(fx, sp + 0xA0, 0x01E0_23BC);
    for k in 0xB4..0x100u32 {
        let (ra, sign) = if f != 0 { (0x01E0_23D4, 1) } else { (0x01E0_23FC, -1) };
        let v = fx.rand_from(D, ra);
        fx.mem.set8(BYTES_B + k, sign * (v % 4));
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    // The entry's s1: the flash tasks counted here (the steps' saved s1).
    let mut flashes = 0u32;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x24);
        match f {
            SPRITE => sprite(fx, t, flashes),
            DISC => {
                let k = spawn::disc("38953db3edd0").expect("constants");
                spawn::disc_step_with(fx, k, DISC, t);
                // Its prologue's saves (ra, the entry's s0 = D).
                fx.saved(fx.sp - 0x20, &[(0x10, 0x01E0_24B8), (0, u64::from(D))]);
            }
            LIGHT => light(fx, t),
            ZOOM | WINDOW => flashes = (flashes + 1) & 0xFFFF,
            _ => debug_assert!(false, "b124: unported task step {f:#x}"),
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x60);
    // The entry's s2: the live tasks counted in the draw loop.
    let mut live = 0u32;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        live += 1;
        if fx.mem.read_u32(t + 0x24) == SPRITE {
            let w = fx.mem.read_u32(TEX5 + 8);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x34);
            l2::rect(fx, sp + 0x80, 0x14, sub);
            for k in 0..4 {
                let v = fx.mem.read_u32(sp + 0x80 + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, v);
            }
            for k in 0..3 {
                let v = ee::from_i32(fx.mem.i16(t + 2 * k));
                fx.mem.setf(sp + 0x60 + 4 * k, v);
            }
            billboard::sprite_spin_40(fx, 0x01E0_0D90, uv::uv_rect, D, t, 0, sp + 0x60);
        }
        if fx.mem.read_u32(t + 0x24) == DISC {
            quads4::fan18(fx, D, t, sp + 0xB0);
        }
    }
    if flashes != 0 {
        unit_flash(fx, flashes, live);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x159 => 2,
        0xE7..=0x153 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E027C0(D, step, 0): the first free task cleared and set off at
/// D+0x250 (i16 +0 only) with the colour D+0x37D4 (+0x28 / +0x2C), kind
/// D+0x381C (+0x30); `saves` the caller's s0..s4, `ra` the return address.
fn spawn(fx: &mut Fx, step: u32, saves: [u64; 5], ra: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, saves[4]), (0x30, saves[3]), (0x20, saves[2]), (0x10, saves[1]), (0, saves[0])]);
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
        l2::to_i16x4(fx, t, POS, 0);
        let s = fx.mem.i16(KIND);
        fx.mem.set16(t + 0x30, s);
        fx.mem.write_u32(t + 0x20, 0);
        fx.mem.write_u32(t + 0x24, step);
    }
    fx.leave(0x60);
}

/// 0x1E01210(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the emitters (frame 0x50: ra, the entry's s2 = s1 = 0, s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x30, 0x01E0_2444), (0x20, 0), (0x10, 0), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    // The spawns' saves: the emitter's s0 / s1 / s2 (= D) and the entry's
    // s3 = the parameter, s4 the runner's (0 [H]).
    let p = param as i64 as u64;
    let saves = |s0: u64, s1: u64| [s0, s1, u64::from(D), p, 0];
    let node = |fx: &mut Fx, k: u32, out: u32| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2 * k);
        fx.node_position(h, slot, out);
    };
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    if (0xDE..0x15A).contains(&c(fx)) {
        spawn(fx, ZOOM, saves(u64::from(D), 0), 0x01E0_1270);
    }
    if c(fx) == 0xDE {
        spawn(fx, WINDOW, saves(u64::from(D), 0), 0x01E0_1294);
    }
    if c(fx) == 0xDE {
        let h = fx.self_handle();
        fx.unit_position(h, sp + 0x40);
        let w = fx.mem.read_u32(sp + 0x40);
        fx.mem.write_u32(POS, w);
        let y = ee::add(60.0, fx.mem.f(sp + 0x44));
        fx.mem.setf(POS + 4, y);
        let w = fx.mem.read_u32(sp + 0x48);
        fx.mem.write_u32(POS + 8, w);
        spawn(fx, DISC, saves(u64::from(D), 0), 0x01E0_12F0);
    }
    if c(fx) == 0xDC {
        spawn(fx, LIGHT, saves(u64::from(D), 0), 0x01E0_1314);
    }
    if (0x102..0x155).contains(&c(fx)) {
        node(fx, 0, sp + 0x40);
        let n = rmod(fx, D, 0x01E0_1350, 9) + 4;
        let s1 = n.max(0) as u32;
        for i in 0..s1 {
            let v = rmod(fx, i, 0x01E0_137C, 0x3C);
            let p = ee::sub(ee::add(fx.mem.f(sp + 0x40), ee::from_i32(v)), 30.0);
            fx.mem.setf(POS, p);
            let v = rmod(fx, i, 0x01E0_13B0, 0x3C);
            let p = ee::sub(ee::sub(ee::add(fx.mem.f(sp + 0x44), ee::from_i32(v)), 30.0), 12.0);
            fx.mem.setf(POS + 4, p);
            let v = rmod(fx, i, 0x01E0_13F0, 0x1E);
            let p = ee::sub(ee::add(fx.mem.f(sp + 0x48), ee::from_i32(v)), 15.0);
            fx.mem.setf(POS + 8, p);
            spawn(fx, SPRITE, saves(u64::from(i), u64::from(s1)), 0x01E0_1434);
        }
    }
    if c(fx) == 0x44 {
        node(fx, 4, sp + 0x40);
        for k in 0..3 {
            let w = fx.mem.read_u32(sp + 0x40 + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
        fx.mem.set16(KIND, 1);
    }
    if (0x50..0x65).contains(&c(fx)) {
        node(fx, 4, sp + 0x40);
        for k in 0..3 {
            let w = fx.mem.read_u32(sp + 0x40 + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
    }
    fx.leave(0x50);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// 0x1E01680 (D, task): a grey sprite (size 50, spin 50 / 10 / +-25 by
/// the facing, +0x30 rand % 180, +0x3A rand % 360) moving 7 a update
/// toward the facing (its i16 z) and 2 down; 0x80 for 11 updates, out by
/// 0x18. Frame 0x30 (the entry's s0 = D, s1 = the flashes).
fn sprite(fx: &mut Fx, t: u32, flashes: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_2498), (0x10, u64::from(flashes)), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x32);
        fx.mem.set32(t + 0x10, 0x32);
        fx.mem.set32(t + 0x14, 0xA);
        let s = if fx.mem.i16(FACING) != 0 { 0x19 } else { -0x19 };
        fx.mem.set32(t + 0x18, s);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, 0x01E0_16F4, 0xB4);
        fx.mem.set16(t + 0x30, v);
        let v = rmod(fx, t, 0x01E0_1710, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0xB {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0x1A {
            fx.mem.write_u8(t + 0x2B, a - 0x18);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    fx.mem.set16(t + 0x34, 2);
    let z = fx.mem.i16(t + 4);
    let z = if fx.mem.i16(FACING) != 0 { z - 7 } else { z + 7 };
    fx.mem.set16(t + 4, z);
    let y = fx.mem.i16(t + 2) - 2;
    fx.mem.set16(t + 2, y);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E01A30 (D, task): the stage light +0x30 from 0x80 down by 10 to 0x30
/// for 130 updates, then up by 10 (the task over once at 0x80). Frame
/// 0x10 (ra).
fn light(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x10);
    fx.saved(sp, &[(0, 0x01E0_24F8)]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x30, 0x80);
        fx.mem.set16(t + 0x36, 2);
    }
    let l = fx.mem.i16(t + 0x30);
    if fx.mem.i16(t + 0x32) < 0x82 {
        fx.mem.set16(t + 0x30, if l < 0x31 { 0x30 } else { l - 0xA });
    } else if l < 0x80 {
        fx.mem.set16(t + 0x30, l + 0xA);
    } else {
        fx.mem.set16(t + 0x30, 0x80);
        fx.mem.set16(t + 0x36, 0);
    }
    age(fx, t);
    let l = fx.mem.read_u8(t + 0x30);
    fx.set_stage_light(i32::from(l));
    fx.leave(0x10);
}

/// The descriptor's two corners at `at` (+0x20 / +0x30): x = y = `xy`, Z
/// 0xFFFFFF, the high byte cleared.
fn corner(fx: &mut Fx, d: u32, at: u32, xy: u16) {
    fx.mem.write_u16(d + at, xy);
    fx.mem.write_u16(d + at + 2, xy);
    let v = fx.mem.read_u64(d + at);
    fx.mem.write_u64(d + at, (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
    fx.mem.write_u8(d + at + 7, 0);
}

/// 0x1E01C00 (D, t): the zoom: the target drawn as a sprite into the
/// flash descriptor, its ST inset by the task's +8 / +0xA over 10 (the
/// lower edge 3 x), Z 4000 below the attacker's projected point (0x204D80
/// of the attacker's position into the descriptor's first corner, its w
/// at D+0x37E8), submitted twice at the key w - 2000 (no ordering table);
/// first white-grey 0xC0, alpha 0x78; the insets grow by 100..129 a
/// update; after the first update the alpha down by 0x31 (the task ends
/// under 0x29). Frame 0x50 (f20 / f21, the unit flash's s0 = k / s1).
fn zoom(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x30, 0x01E0_29AC), (0x20, u64::from(regs[1])), (0x10, u64::from(regs[0]))]);
    let f20 = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let f21 = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.write_u8(t + 0x2B, 0x78);
        l2::rgb(fx, t + 0x28, 0xC0, 0xC0, 0xC0);
        fx.mem.set16(t + 0x36, 2);
    }
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES);
    fx.node_position(h, slot, sp + 0x40);
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x40);
    let d = fx.mem.read_u32(FLASH_DESC);
    let p = [fx.mem.f(sp + 0x40), fx.mem.f(sp + 0x44), fx.mem.f(sp + 0x48)];
    let mut x = fx.mem.read_u64(d + 0x20);
    if let Some(w) = fx.project(p, None, &mut x) {
        fx.mem.write_u64(d + 0x20, x);
        fx.mem.setf(MEAN, w);
    }
    project_residue(fx, sp, sp + 0x40);
    let d = fx.mem.read_u32(FLASH_DESC);
    let z = ((fx.mem.read_u64(d + 0x20) >> 32) & 0xFF_FFFF).wrapping_sub(0xFA0) & 0xFF_FFFF;
    for k in 0..4u32 {
        let b = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(d + 0x10 + k, b);
    }
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    corner(fx, d, 0x20, 0x6FF8);
    corner(fx, d, 0x30, 0x8FF8);
    let lo = ee::div(ee::sub(512.0, ee::mul(3.0, f21)), 512.0);
    fx.mem.setf(d + 0x18, ee::div(f20, 512.0));
    fx.mem.setf(d + 0x1C, ee::div(f21, 512.0));
    fx.mem.setf(d + 0x28, ee::div(ee::sub(512.0, f20), 512.0));
    fx.mem.setf(d + 0x2C, lo);
    for at in [0x20u32, 0x30] {
        let v = fx.mem.read_u64(d + at);
        fx.mem.write_u64(d + at, (v & 0xFF00_0000_FFFF_FFFF) | (z << 32));
    }
    for ra in [0x01E0_1E3Cu32, 0x01E0_1E60] {
        let v = ee::sub(fx.mem.f(MEAN), 2000.0);
        fx.fix_residue(v, ra);
        let key = ee::fixunssfsi(v) as i32;
        let d = fx.mem.read_u32(FLASH_DESC);
        fx.submit(d, 0, key);
    }
    for (o, ra) in [(8u32, 0x01E0_1E78u32), (0xA, 0x01E0_1EA8)] {
        let v = rmod(fx, t, ra, 0x1E);
        let s = fx.mem.i16(t + o) + ((v + 0x64) as i16 as i32);
        fx.mem.set16(t + o, s);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0x29 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_sub(0x31));
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    age(fx, t);
    fx.leave(0x50);
}

/// 0x1E01F30 (D, t): the target over the whole window (white, the task's
/// alpha) into the flash descriptor, no ordering table; alpha up by 0x30
/// below 0xC0 for 0x87 updates, then down by 0x13 (the task ends under
/// 0x15). Frame 0x30 (the unit flash's s0 = k / s1).
fn window(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_29D0), (0x10, u64::from(regs[1])), (0, u64::from(regs[0]))]);
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
    corner(fx, d, 0x20, 0x6FF8);
    corner(fx, d, 0x30, 0x8FF8);
    for (o, v) in [(0x18u32, 0u32), (0x1C, 0), (0x28, 0x3F80_0000), (0x2C, 0x3F80_0000)] {
        fx.mem.write_u32(d + o, v);
    }
    fx.submit(d, 0, 0);
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < 0x87 {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x2B, a + 0x30);
        }
    } else if a >= 0x15 {
        fx.mem.write_u8(t + 0x2B, a - 0x13);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E028E0(D): in a packet layer (0x2078A0) the attacker drawn into the
/// target (0x1F8270 filter 0, its TEX0 / TEX1 into the sprite descriptor
/// at sp + 0x48; ALPHA 0x48), TEXFLUSH before and after; every zoom /
/// window task stepped; the layer linked into the ordering table at the
/// key 0x1A2C70(D+0x37E8 - 2000). Frame 0xA0 (ra, the entry's s2 = the
/// live count, s1 = the flash count, s0 = D).
fn unit_flash(fx: &mut Fx, flashes: u32, live: u32) {
    let sp = fx.enter(0xA0);
    fx.saved(sp, &[(0x30, 0x01E0_2730), (0x20, u64::from(live)), (0x10, u64::from(flashes)), (0, u64::from(D))]);
    let d = sp + 0x48;
    fx.mem.write_u32(FLASH_DESC, d);
    let layer = fx.layer_begin();
    fx.mem.write_u32(LAYER, layer);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.texflush([5, d, 0x26, 0]);
    fx.desc_alpha(d, 0x80_0000_0048);
    let h = fx.self_handle();
    let (_, regs) = fx.unit_draw(h, 0, sp + 0x88, sp + 0x90);
    fx.texflush(regs);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x24) == ZOOM {
            zoom(fx, t, [k, D + 0x40 * k]);
        }
        if fx.mem.read_u32(t + 0x24) == WINDOW {
            window(fx, t, [k, D + 0x40 * k]);
        }
    }
    let v = ee::sub(fx.mem.f(MEAN), 2000.0);
    fx.fix_residue(v, 0x01E0_2A00);
    let key = ee::fixunssfsi(v) as i32;
    let layer = fx.mem.read_u32(LAYER);
    fx.layer_end(layer, OT, key);
    fx.leave(0xA0);
}
