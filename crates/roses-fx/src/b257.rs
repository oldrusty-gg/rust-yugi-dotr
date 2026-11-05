//! BMODULE b257 (card 567), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03670, data area 0x1ED0000, 0x39A0 bytes) [F]
//!: b207 / b383's task shape on 200 tasks of 0x40
//! bytes at D+0x300 (alive +0x36, step +0x24, spawned by 0x1E04400 on the
//! colour D+0x36C4 and the point D+0x250). Nodes 0x32 / 0x34 / 0x31 /
//! 0x37 / 0x36 at D+0x3708..; a 10-point history D+0x3570 of node 0x37 or
//! 0x36 (picked by the attacker's turn and the camera's +0x30 x; filled on
//! update 0x14). The emitter 0x1E01B90 counts D+0x3726: 0x4E..0x11A three
//! smoke rings a update (0x1E01EB0 / 0x1E021E0 / 0x1E024F0) at node 0x32
//! (y + 2), 0x25..0x36 a ring 0x1E028B0 at node 0x31; 0x14 two quarter
//! flares 0x1E02BC0 (turns 0 / 90), a cross ribbon 0x1E02CF0 along the
//! history and a camera-facing square 0x1E02DC0 (stepped in the entry).
//! Draws: `quads4::ring_facing_i` with `uv::uv_rect4`, b112's quarters
//! (0x1E00B20), b709's cross ribbon (0x1E01270), the square variant
//! 0x1E01880. Steps 0x1E01DB0 / 0x2E70 / 0x2F20 / 0x2FD0 / 0x31C0 / 0x32A0 /
//! 0x3560 / 0x3620 are tested but never spawned (not ported). Returns 1 on
//! update 0x5A, 3 on 0x131, 6 on 0x65..0x12B, 8 once the 0x140-update timer
//! D+0x36C0 has run out.
use crate::b284::{angles, fix, rmod};
use crate::blib::{quads4, spawn as lib, spline, uv};
use crate::bus::Bus;
use crate::c061::{rgbaq_at, to_degrees_neg};
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 200;
const SMOKE_A: u32 = 0x01E0_1EB0;
const SMOKE_B: u32 = 0x01E0_21E0;
const SMOKE_C: u32 = 0x01E0_24F0;
const SMOKE_D: u32 = 0x01E0_28B0;
const FLARE: u32 = 0x01E0_2BC0;
const RIBBON: u32 = 0x01E0_2CF0;
const SQUARE: u32 = 0x01E0_2DC0;
const TIMER: u32 = D + 0x36C0;
const COLOUR: u32 = D + 0x36C4;
const NODES: u32 = D + 0x3708;
const TURN: u32 = D + 0x371A;
const COUNTER: u32 = D + 0x3726;
const BYTES: u32 = D + 0x372A;
const HIST: u32 = D + 0x3570;

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

/// The entry 0x1E03670 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x150);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x36CC, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x150);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x39A0);
    l2::sine_table(fx, D + 0x382C);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x3724, param);
    fx.mem.set32(TIMER, 0x140);
    for o in [0x290u32, 0x294, 0x298] {
        fx.mem.write_u32(D + o, 0);
    }
    for (k, n) in [0x32, 0x34, 0x31, 0x37, 0x36].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    for (k, id) in [(0u32, 0xE), (1, 0xF)] {
        let t = fx.texture(4, id);
        fx.mem.write_u32(D + 0x2F8 + 4 * k, t);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x2C8, t);
    for k in 0..4 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(D + 0x3500 + 4 * k as u32, t);
    }
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_37C8);
        let r = fx.div(v, 0x1E).1;
        fx.mem.set8(BYTES + k, r - 0xF);
    }
    0
}

/// Node 0x37 (D+0x370E) or 0x36 (D+0x3710): by the attacker's turn (sp +
/// 0x74 below 0x4000) and the camera's +0x30 x (`cam`).
fn pick(fx: &mut Fx, sp: u32, cam: u32) -> i32 {
    let x = fx.mem.f(cam);
    let a = if fx.mem.i32(sp + 0x74) < 0x4000 { x < 0.0 } else { x > 0.0 };
    fx.mem.i16(if a { D + 0x370E } else { D + 0x3710 })
}

/// Which of [`pick`]'s four branches runs (low turn and x < 0, low and
/// not, high and x > 0, high and not).
fn branch(fx: &mut Fx, sp: u32, cam: u32) -> usize {
    let x = fx.mem.f(cam);
    if fx.mem.i32(sp + 0x74) < 0x4000 {
        if x < 0.0 { 0 } else { 1 }
    } else if x > 0.0 {
        2
    } else {
        3
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    l2::camera_angles_to(fx, sp + 0x70);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x70);
    let cam = fx.camera_at30(0);
    let x = fx.mem.f(cam);
    let low = fx.mem.i32(sp + 0x74) < 0x4000;
    let a = if low { x < 0.0 } else { x > 0.0 };
    let local = match (low, a) {
        (true, true) => 0x100,
        (true, false) => 0x110,
        (false, true) => 0x120,
        (false, false) => 0x130,
    };
    let slot = pick(fx, sp, cam);
    if counter(fx) == 0x14 {
        let h = fx.self_handle();
        fx.node_position(h, slot, sp + local);
        for k in 0..9u32 {
            for c in 0..3 {
                let w = fx.mem.read_u32(sp + local + 4 * c);
                fx.mem.write_u32(HIST + 0x10 * k + 4 * c, w);
            }
        }
    }
    let slot = pick(fx, sp, cam);
    lib::node_history(fx, D + 0x3600, 0xA, slot);
    // node_history's frame (0x70) below the entry's: the entry's s0 = D,
    // s1 = the camera pointer, s2 = 0, s3 (the history pointer after the
    // fill, else the runner's 0), s4 = sp + 0x74 and ra by the branch; its
    // s3 slot (sp - 0x40) is the cross ribbon's missing-texture TEX0 [run].
    let s3 = if counter(fx) == 0x14 { HIST + 0x90 } else { 0 };
    let ra = [0x01E0_38C0u32, 0x01E0_3930, 0x01E0_39B8, 0x01E0_3A28][branch(fx, sp, cam)];
    fx.saved(sp - 0x70, &[(0, u64::from(D)), (0x10, u64::from(cam)), (0x20, 0), (0x30, u64::from(s3)), (0x40, u64::from(sp + 0x74)), (0x50, u64::from(ra))]);
    for k in 0..TASKS {
        let t = D + 0x300 + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x24);
        for (g, ra) in [(SMOKE_A, 0x01E0_3A80u32), (SMOKE_B, 0x01E0_3AA0), (SMOKE_C, 0x01E0_3AC0), (SMOKE_D, 0x01E0_3AE0)] {
            if f(fx) == g {
                smoke(fx, t, g, cam, ra);
            }
        }
        if f(fx) == FLARE {
            flare(fx, t);
        }
        if f(fx) == RIBBON {
            ribbon(fx, t);
        }
        if f(fx) == SQUARE {
            square_step(fx, t);
        }
        debug_assert!(!matches!(f(fx), 0x01E0_1DB0 | 0x01E0_2E70 | 0x01E0_2F20 | 0x01E0_2FD0 | 0x01E0_31C0 | 0x01E0_32A0 | 0x01E0_3560 | 0x01E0_3620));
    }
    for k in 0..TASKS {
        let t = D + 0x300 + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x24);
        for (g, tex, local, b8, ra) in [
            (SMOKE_A, 0x3500u32, 0x90u32, 4, 0x01E0_3E74u32),
            (SMOKE_B, 0x3504, 0xA0, 4, 0x01E0_3F08),
            (SMOKE_C, 0x3508, 0xB0, 6, 0x01E0_3F98),
            (SMOKE_D, 0x3508, 0xC0, 1, 0x01E0_4028),
        ] {
            if f(fx) == g {
                cell(fx, t, D + tex, 0x14, sp + local);
                fx.mem.set16(D + 0xB8, b8);
                let v = fx.rand_from(D, ra);
                fx.mem.set16(D + 0xBE, v % 4);
                quads4::ring_facing_i(fx, uv::uv_rect4, D, t, 0, BYTES);
                fx.mem.set16(D + 0xB8, 0);
            }
        }
        if f(fx) == FLARE {
            cell(fx, t, D + 0x2FC, 0x9A, sp + 0xD0);
            let slot = pick(fx, sp, cam);
            let ra = [0x01E0_40F0, 0x01E0_4110, 0x01E0_4150, 0x01E0_4170][branch(fx, sp, cam)];
            let h = fx.self_handle();
            crate::b076::node_position_saving(fx, h, slot, sp + 0x80, u64::from(D), Some(u64::from(cam)), ra);
            fx.mem.set16(D + 0xBE, 0);
            quarters(fx, t, 1, sp + 0x80, false);
        }
        if f(fx) == RIBBON {
            let w = fx.mem.read_u32(D + 0x2FC);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, D + 0x350C, 0x14, sp + 0xE0);
            fx.mem.set16(D + 0xBE, 0);
            for (o, v) in [(0x94u32, 0xF), (0x96, 0x10), (0x98, 0xF), (0x9A, 1)] {
                fx.mem.set16(D + o, v);
            }
            cross_draw(fx, t, HIST);
        }
        if f(fx) == SQUARE {
            cell(fx, t, D + 0x350C, 0x14, sp + 0xF0);
            let slot = pick(fx, sp, cam);
            let ra = [0x01E0_42B0, 0x01E0_42D0, 0x01E0_4310, 0x01E0_4330][branch(fx, sp, cam)];
            let h = fx.self_handle();
            crate::b076::node_position_saving(fx, h, slot, sp + 0x80, u64::from(D), Some(u64::from(cam)), ra);
            fx.mem.set16(D + 0xBE, 0);
            quarters(fx, t, 1, sp + 0x80, true);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0x5A => 1,
        0x131 => 3,
        0x65..=0x12B => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The texture word at `tex` into D+0x88 and the atlas cell `id` / +0x34
/// (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, tex: u32, id: i32, r: u32) {
    let w = fx.mem.read_u32(tex);
    fx.mem.write_u32(D + 0x88, w);
    let sub = fx.mem.i16(t + 0x34);
    l2::rect(fx, r, id, sub);
    copy4(fx, D + 0x94, r);
}

/// 0x1E04400(D, step, 0): the first free task cleared and set off (colour
/// D+0x36C4 twice, the point D+0x250, +0x30 / +0x3A from D+0x3722 /
/// D+0x371A); its frame's saves: the emitter's s0 = D / s1 = 0, the
/// entry's s2 = 0, the runner's s3 / s4 (0 [H]) and `ra`.
fn spawn(fx: &mut Fx, step: u32, ra: u32) -> i32 {
    let f = fx.enter(0x60);
    fx.saved(f, &[(0, u64::from(D)), (0x10, 0), (0x20, 0), (0x30, 0), (0x40, 0), (0x50, u64::from(ra))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x336 + 0x40 * k) == 0) {
        Some(k) => {
            let t = D + 0x300 + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            for c in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            let a = fx.mem.i16(D + 0x3722);
            fx.mem.set16(t + 0x30, a);
            let a = fx.mem.i16(TURN);
            fx.mem.set16(t + 0x3A, a);
            fx.mem.write_u32(t + 0x20, 0);
            fx.mem.write_u32(t + 0x24, step);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// The spawn point: node `k`'s position with y + 2.
fn at_node(fx: &mut Fx, sp: u32, k: u32) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES + 2 * k);
    fx.node_position(h, slot, sp + 0x30);
    let x = fx.mem.read_u32(sp + 0x30);
    fx.mem.write_u32(D + 0x250, x);
    let y = ee::add(2.0, fx.mem.f(sp + 0x34));
    fx.mem.setf(D + 0x254, y);
    let z = fx.mem.read_u32(sp + 0x38);
    fx.mem.write_u32(D + 0x258, z);
}

/// 0x1E01B90(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the emitters (frame 0x40: the entry's s0 = D, s1 = param, ra).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x20, 0x01E0_3804), (0x10, param as i64 as u64), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    if (0x4E..0x11B).contains(&counter(fx)) {
        at_node(fx, sp, 0);
        spawn(fx, SMOKE_A, 0x01E0_1C34);
        spawn(fx, SMOKE_B, 0x01E0_1C58);
        spawn(fx, SMOKE_C, 0x01E0_1C7C);
    }
    if (0x25..0x37).contains(&counter(fx)) {
        at_node(fx, sp, 2);
        spawn(fx, SMOKE_D, 0x01E0_1CF8);
    }
    if counter(fx) == 0x14 {
        fx.mem.set16(TURN, 0);
        spawn(fx, FLARE, 0x01E0_1D30);
        let a = fx.mem.i16(TURN) + 0x5A;
        fx.mem.set16(TURN, a);
        spawn(fx, FLARE, 0x01E0_1D50);
    }
    if counter(fx) == 0x14 {
        spawn(fx, RIBBON, 0x01E0_1D74);
    }
    if counter(fx) == 0x14 {
        spawn(fx, SQUARE, 0x01E0_1D98);
    }
    fx.leave(0x40);
}

/// The four smoke-ring steps' constants.
struct Smoke {
    /// +0x38 (before / from update 0x50).
    size: (i32, i32),
    /// +0x34 = rand % 4 (`Some(jal)`) or 0.
    frame: Option<u32>,
    /// The colour pick: None grey `grey`, Some(jal) one of three by rand % 3.
    pick: Option<u32>,
    grey: i32,
    /// +0x30 = rand % 0x32 + 0x64 (`Some(jal)`) or 0.
    radius: Option<u32>,
    jals: [u32; 4],
    grow: (i32, i32),
    row: i32,
    life: (i32, i32),
    second: u32,
    node_ras: [u32; 2],
    fix_dx: u32,
    fade_in: (u8, u8),
    fade_out: (u8, u8),
    div: f32,
    first_z: Option<u32>,
    fix_z: u32,
    frames: i32,
    size_up: i32,
    angles: u32,
}

const SMOKES: [Smoke; 4] = [
    Smoke { size: (0x35, 0x3F), frame: Some(0x01E0_1EF8), pick: None, grey: 0x70, radius: None, jals: [0x01E0_1F2C, 0x01E0_1F48, 0x01E0_1F68, 0x01E0_1F84], grow: (5, 5), row: 0xA, life: (0xA, 0x11), second: 0x370A, node_ras: [0x01E0_1FBC, 0x01E0_1FD8], fix_dx: 0x01E0_1FE0, fade_in: (0x81, 0x40), fade_out: (0x11, 0xF), div: 40.0, first_z: None, fix_z: 0x01E0_20AC, frames: 6, size_up: 1, angles: 0x01E0_215C },
    Smoke { size: (0x37, 0x41), frame: None, pick: None, grey: 0x40, radius: None, jals: [0x01E0_2240, 0x01E0_225C, 0x01E0_227C, 0x01E0_2298], grow: (5, 4), row: 0x1E, life: (3, 5), second: 0x370A, node_ras: [0x01E0_22D0, 0x01E0_22EC], fix_dx: 0x01E0_22F4, fade_in: (0x81, 0x20), fade_out: (0x11, 0xF), div: 40.0, first_z: None, fix_z: 0x01E0_23C0, frames: 6, size_up: 3, angles: 0x01E0_2470 },
    Smoke { size: (0x64, 0x64), frame: None, pick: Some(0x01E0_251C), grey: 0, radius: Some(0x01E0_25A8), jals: [0x01E0_25C8, 0x01E0_25E4, 0x01E0_2604, 0x01E0_2620], grow: (0xA, 5), row: 0xA, life: (0xA, 0x18), second: 0x370A, node_ras: [0x01E0_2658, 0x01E0_2674], fix_dx: 0x01E0_267C, fade_in: (0xC1, 0x18), fade_out: (0x15, 0x13), div: 40.0, first_z: Some(0x01E0_2764), fix_z: 0x01E0_2784, frames: 3, size_up: 1, angles: 0x01E0_2834 },
    Smoke { size: (0x46, 0x46), frame: Some(0x01E0_28D8), pick: None, grey: 0x70, radius: None, jals: [0x01E0_290C, 0x01E0_2928, 0x01E0_2948, 0x01E0_2964], grow: (5, 5), row: 0xA, life: (2, 3), second: 0x370C, node_ras: [0x01E0_299C, 0x01E0_29B8], fix_dx: 0x01E0_29C0, fade_in: (0x81, 0x40), fade_out: (0x11, 0xF), div: 100.0, first_z: None, fix_z: 0x01E0_2A8C, frames: 6, size_up: 1, angles: 0x01E0_2B3C },
];

/// 0x1E01EB0 / 0x1E021E0 / 0x1E024F0 / 0x1E028B0 (D, task): smoke rings
/// (frame 0x60: the entry's s0 = D, s1 = the camera pointer, ra): row,
/// grow, life random; +0xC the x gap of node 0x32 to the second node; fade
/// in / out; z drifting towards the target by the gap / 40 (100); x by
/// +0xC on odd ages, the cell frame counting up, radius / size growing;
/// random angles.
fn smoke(fx: &mut Fx, t: u32, g: u32, cam: u32, ra: u32) {
    let s = &SMOKES[match g {
        SMOKE_A => 0,
        SMOKE_B => 1,
        SMOKE_C => 2,
        _ => 3,
    }];
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x20, u64::from(ra)), (0x10, u64::from(cam)), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x36) == 1 {
        let size = if counter(fx) < 0x50 { s.size.0 } else { s.size.1 };
        fx.mem.set16(t + 0x38, size);
        let fr = match s.frame {
            Some(j) => rmod(fx, t, j, 4),
            None => 0,
        };
        fx.mem.set16(t + 0x34, fr);
        match s.pick {
            Some(j) => {
                let r = rmod(fx, t, j, 3) as i16;
                let (a, b, c) = match r {
                    0 => (0x10, 0x80, 0x10),
                    1 => (0x40, 0, 0x80),
                    _ => (0x80, 0x10, 0x10),
                };
                l2::rgb(fx, t + 0x28, a, b, c);
            }
            None => l2::rgb(fx, t + 0x28, s.grey, s.grey, s.grey),
        }
        let r = match s.radius {
            Some(j) => rmod(fx, t, j, 0x32) + 0x64,
            None => 0,
        };
        fx.mem.set16(t + 0x30, r);
        let v = rmod(fx, t, s.jals[0], 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rmod(fx, t, s.jals[1], s.grow.0);
        fx.mem.set16(t + 0xE, v + s.grow.1);
        let v = rmod(fx, t, s.jals[2], s.row);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, s.jals[3], s.life.0);
        fx.mem.set16(t + 6, v + s.life.1);
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        crate::b076::node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), s.node_ras[0]);
        let h = fx.self_handle();
        let slot = fx.mem.i16(D + s.second);
        crate::b076::node_position_saving(fx, h, slot, sp + 0x40, u64::from(t), Some(u64::from(D)), s.node_ras[1]);
        let dx = ee::sub(fx.mem.f(sp + 0x40), fx.mem.f(sp + 0x30));
        let dx = fix(fx, dx, s.fix_dx);
        fx.mem.set16(t + 0xC, dx);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < s.fade_in.0 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(s.fade_in.1));
        }
    } else if a >= s.fade_out.0 {
        fx.mem.write_u8(t + 0x2B, a - s.fade_out.1);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x30);
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x40);
    let dz = ee::sub(fx.mem.f(sp + 0x38), fx.mem.f(sp + 0x48));
    fx.mem.setf(sp + 0x38, dz);
    let dz = ee::div(dz, s.div);
    fx.mem.setf(sp + 0x38, dz);
    if let Some(j) = s.first_z {
        if fx.mem.i16(t + 0x32) == 0 {
            let z = ee::add(ee::from_i32(fx.mem.i16(t + 4)), ee::mul(17.0, dz));
            let z = fix(fx, z, j);
            fx.mem.set16(t + 4, z);
        }
    }
    let z = ee::add(ee::from_i32(fx.mem.i16(t + 4)), dz);
    let z = fix(fx, z, s.fix_z);
    fx.mem.set16(t + 4, z);
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let x = fx.mem.i16(t) + fx.mem.i16(t + 0xC);
        fx.mem.set16(t, x);
    }
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let c = fx.mem.i16(t + 0x34);
        if c < s.frames {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let g = fx.mem.i16(t + 0x38);
        fx.mem.set16(t + 0x38, g + s.size_up);
    }
    angles(fx, t, s.angles);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x60);
}

/// 0x1E02BC0 (D, task): a quarter flare (dark (0x20 x3) / (0xC0, 8, 8),
/// half extents 0x14 / 0x50 growing by 10, turning by 7; in by 0x40 under
/// 0x80 for 2 updates, then out by 9).
fn flare(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, 0x01E0_3B00), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 6);
        l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x20);
        l2::rgb(fx, t + 0x2C, 0xC0, 8, 8);
        fx.mem.set16(t + 8, 0x14);
        fx.mem.set16(t + 0x34, 1);
        fx.mem.set16(t + 0xA, 0x50);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if fx.mem.i16(t + 0x32) < 2 {
        for o in [0x2Bu32, 0x2F] {
            let a = fx.mem.read_u8(t + o);
            if a < 0x80 {
                fx.mem.write_u8(t + o, a + 0x40);
            }
        }
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0xB {
            fx.mem.write_u8(t + 0x2B, a - 9);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
        let a = fx.mem.read_u8(t + 0x2F);
        fx.mem.write_u8(t + 0x2F, if a >= 0xB { a - 9 } else { 0 });
    }
    let v = fx.mem.i16(t + 0xA) + 0xA;
    fx.mem.set16(t + 0xA, v);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    let a = fx.mem.i16(t + 0x3A) + 7;
    fx.mem.set16(t + 0x3A, a);
    fx.leave(0x20);
}

/// 0x1E02CF0 (D, task): the cross ribbon ((0xC0, 0x10, 0x10) twice, 9
/// points, size 0x14): alpha 0x80 for 0xAF updates, then down by 9.
fn ribbon(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, 0x01E0_3B20), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x36) == 1 {
        l2::rgb(fx, t + 0x28, 0xC0, 0x10, 0x10);
        l2::rgb(fx, t + 0x2C, 0xC0, 0x10, 0x10);
        fx.mem.set16(t + 6, 9);
        fx.mem.write_u8(t + 0x2B, 0);
        fx.mem.set16(t + 0x34, 1);
        fx.mem.set16(t + 0x38, 0x14);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0xAF {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0xB {
            fx.mem.write_u8(t + 0x2B, a - 9);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x20);
}

/// The entry's case 0x1E02DC0: a dark square (0x20 x3, size 100): in by 9
/// under 0x60 to age 0x140, then out by 9.
fn square_step(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x20);
        fx.mem.write_u8(t + 0x2B, 0);
        fx.mem.set16(t + 0x38, 0x64);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < 0x140 {
        if a < 0x60 {
            fx.mem.write_u8(t + 0x2B, a + 9);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x2B, a - 9);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// 0x1E00B20 (`square` false; = b112's 0x1E00850) / 0x1E01880 (`square`
/// true) (D, task, far, pos): strip3 quads at `pos` facing the camera
/// (scale 0.05, frame 0x1E0, the descriptor at sp + 0x130). 0x1E00B20: the
/// quarters of the half extents (+8, +0xA) turned about z by +0x3A,
/// colours +0x2C x3 / +0x28; 0x1E01880: four times the square of size
/// +0x38 turned by the camera's angles, colours +0x28. `far` 0 keyed by
/// mean w, else at the far Z, key 0.
fn quarters(fx: &mut Fx, t: u32, far: i32, pos: u32, square: bool) {
    let sp = fx.enter(0x1E0);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0x60 + 4 * k, w);
    }
    let d = sp + 0x130;
    l2::desc_strip3(fx, d);
    let c = if square { 0x28 } else { 0x2C };
    for (local, c, to) in [(0x1C0u32, c, 0x10u32), (0x1C8, c, 0x28), (0x1D0, c, 0x40), (0x1D8, 0x28, 0x58)] {
        rgbaq_at(fx, sp + local, t + c, d + to);
    }
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xC0 + 4 * k, 0x3D4C_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0x120);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x60 + 4 * k);
        fx.mem.write_u32(sp + 0xB0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0xE0, sp + 0xC0, sp + 0x120, sp + 0xB0);
    if !square {
        let z = fx.mem.i16(t + 0x3A);
        fx.mem.set32(sp + 0x128, z);
        fx.mem.set32(sp + 0x124, 0);
        fx.mem.set32(sp + 0x120, 0);
    }
    let quad = sp + 0x70;
    for q in 0..4 {
        if square {
            let s = ee::from_i32(fx.mem.i16(t + 0x38));
            l2::quad_xy(fx, quad, s, s);
        } else {
            let (a, b) = (fx.mem.i16(t + 8), fx.mem.i16(t + 0xA));
            l2::quad_ints(fx, quad, a, b, q);
        }
        l2::rotate4(fx, quad, sp + 0x120, 2, D + 0xD8);
        if far == 0 {
            if l2::project(fx, quad, SPR, sp + 0xE0, d, sp + 0xD0) {
                let key = ee::fixunssfsi(fx.mem.f(sp + 0xD0)) as i32;
                let ot = fx.mem.read_u32(D + 0x80);
                fx.submit(d, ot, key);
            }
        } else if far == 1 && l2::project(fx, quad, SPR, sp + 0xE0, d, sp + 0xD0) {
            l2::far_z_desc(fx, d, 0);
            let ot = fx.mem.read_u32(D + 0x80);
            fx.submit(d, ot, 0);
        }
    }
    fx.leave(0x1E0);
}

/// 0x1E01270 (D, task, history) = b709's 0x1E01CF0: the cross ribbon along
/// the i16 +6 points of `history` (four strips a segment by
/// `spline::spline` basis 0), four textured strips each crossing at the
/// curve; its alpha down by alpha / (3 n) a strip; at the far Z, key 0.
fn cross_draw(fx: &mut Fx, t: u32, p: u32) {
    use ee::{add, div, sub};
    let sp = fx.enter(0x260);
    let w = div(ee::from_i32(fx.mem.i16(t + 0x38)), 10.0);
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x1A8;
    l2::desc_strip3(fx, d);
    for (local, c, to) in [(0x238u32, 0x28u32, 0x10u32), (0x240, 0x2C, 0x28), (0x248, 0x28, 0x40), (0x250, 0x2C, 0x58)] {
        rgbaq_at(fx, sp + local, t + c, d + to);
    }
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    let colour = sp + 0x25C;
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(colour + k, v);
    }
    copy4(fx, sp + 0xE0, p);
    copy4(fx, sp + 0xF0, p);
    copy4(fx, sp + 0x100, p + 0x10);
    copy4(fx, sp + 0x110, p + 0x20);
    crate::b494::camera_angles_saving(fx, sp + 0x170, n as i64 as u64, u64::from(D), 0x01E0_1454);
    to_degrees_neg(fx, sp + 0x174);
    fx.mem.set32(sp + 0xB0, n - 1);
    let s7 = 3 * n;
    let mut s0 = 0i32;
    while s0 < n - 1 {
        let s5 = p.wrapping_add((0x10 * (s0 - 1)) as u32);
        let mut f20 = 0.0f32;
        for _ in 0..4 {
            let (src, o) = if s0 != 0 { (s5, 0x180u32) } else { (sp + 0xE0, 0x190) };
            spline::spline(fx, sp + 0xC0, src, f20, 0, [0.0; 4]);
            f20 = add(f20, 0.25);
            spline::spline(fx, sp + o, src, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0xD0, sp + o);
            l2::colour_strip3(fx, d, colour, 1);
            let al = i32::from(fx.mem.read_u8(t + 0x2B));
            let q = fx.div(al, s7).0 as i16 as i32;
            let v = i32::from(fx.mem.read_u8(colour + 3)) - q;
            fx.mem.write_u8(colour + 3, if v > 0 { v as u8 } else { 0 });
            l2::colour_strip3(fx, d, colour, 2);
            for s1 in 0..4 {
                for (to, from) in [(0x120u32, 0xC0u32), (0x130, 0xC0), (0x140, 0xD0), (0x150, 0xD0)] {
                    for c in 0..3 {
                        let w = fx.mem.read_u32(sp + from + 4 * c);
                        fx.mem.write_u32(sp + to + 4 * c, w);
                    }
                }
                let bump = |fx: &mut Fx, o: u32, by: f32, up: bool| {
                    let v = fx.mem.f(sp + o);
                    fx.mem.setf(sp + o, if up { add(v, by) } else { sub(v, by) });
                };
                match s1 {
                    0 | 1 => {
                        bump(fx, 0x134, w, s1 == 0);
                        bump(fx, 0x154, w, s1 == 0);
                    }
                    _ => {
                        let h = div(w, 2.0);
                        bump(fx, 0x130, w, s1 == 2);
                        bump(fx, 0x150, w, s1 == 2);
                        bump(fx, 0x138, h, s1 == 2);
                        bump(fx, 0x158, h, s1 == 2);
                    }
                }
                if l2::project(fx, sp + 0x120, SPR, 0, d, sp + 0x160) {
                    l2::far_z(fx, &[d + 0x68, d + 0x50, d + 0x38, d + 0x20]);
                    fx.submit(d, OT, 0);
                }
            }
        }
        s0 += 1;
    }
    fx.leave(0x260);
}
