//! BMODULE b016 / b017 (cards 54 / 55; twins, 12 words apart), hand-ported
//! from the records (linked at 0x1E00000, entry 0x1E02F60, data area
//! 0x1ED0000) [F]: b491's task pool (150 x 0x70 at
//! D+0x2A0, alive +0x58, step +0x44 through `jalr`; spawned by 0x1E03700,
//! b627's spawn = `spawn::task70` with the colour D+0x44B4 and the slot
//! D+0x451A) and b491's unit flash (0x1E03840: the attacker drawn into the
//! target in a packet layer, the flash tasks 0x1E02D10 stepped), drawn
//! with the second library (`quad`, `arc_strips`, `ring_quads`,
//! `quads4::sparks`). Every update first stores the camera's +0x20 / +0x30
//! (0x1FACE0 / 0x1FAF40) at D+0x44C4 / D+0x44C8. The emitter 0x1E01BC0
//! counts D+0x4522: on [`V::stars`] seven white star rings (0x1E020F0) at
//! the attacker's node 9; on [`V::flash_at`] two white flashes; on
//! [`V::burst`] at the node (pushed [`V::dz`] along the facing) a flare
//! (0x1E01FF0), two arcs (0x1E023F0), seven red spark rings (0x1E02270),
//! six flickering rings (0x1E024F0), six spark bursts (0x1E026B0) and a
//! red full-window tint (0x1E02B40); 106..109 the spawn point moved to the
//! target's z. The steps 0x1E02800 / 0x1E02940 are drawn but never
//! spawned (not ported). Returns 1 / 4 / 2 on [`V::returns`], 8 once the
//! timer D+0x44B0 has run out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::blib::spawn::{self, Pool70};
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const FLARE: u32 = 0x01E0_1FF0;
const STAR: u32 = 0x01E0_20F0;
const SPARK: u32 = 0x01E0_2270;
const ARC: u32 = 0x01E0_23F0;
const RING: u32 = 0x01E0_24F0;
const BURST: u32 = 0x01E0_26B0;
const TINT: u32 = 0x01E0_2B40;
const FLASH: u32 = 0x01E0_2D10;
/// The return address of the entry's `jalr` into a step (0x1E03240 + 8).
const TASK_RA: u64 = 0x01E0_3248;
const POS: u32 = D + 0x250;
const TEX: u32 = D + 0x4490;
const TIMER: u32 = D + 0x44B0;
const LAYER: u32 = D + 0x44CC;
const FLASH_DESC: u32 = D + 0x44D0;
const NODE: u32 = D + 0x44EC;
const SLOT: u32 = D + 0x451A;
const FACING: u32 = D + 0x451E;
const COUNTER: u32 = D + 0x4522;
const BYTES: u32 = D + 0x4524;
const BYTES_B: u32 = D + 0x4624;
const POOL: Pool70 = Pool70 { base: 0x2A0, n: 150, colour: 0x44B4, pos: 0x250, slot: 0x451A };

/// What the twins differ in.
pub struct V {
    /// The update of the two flashes.
    pub flash_at: i32,
    /// The update of the burst.
    pub burst: i32,
    /// The burst point's push along the facing.
    pub dz: f32,
    /// The two updates of the star rings.
    pub stars: [i32; 2],
    /// The timer.
    pub timer: i32,
    /// The updates returning 1 / 4 / 2.
    pub returns: [i32; 3],
}

/// b016 (card 54).
pub const B016: V = V { flash_at: 0x63, burst: 0x6D, dz: 10.0, stars: [0x4B, 0x5B], timer: 0xA0, returns: [0x63, 0x77, 0x6D] };
/// b017 (card 55).
pub const B017: V = V { flash_at: 0x31, burst: 0x3A, dz: 5.0, stars: [0x1A, 0x28], timer: 0x64, returns: [0x30, 0x44, 0x3A] };

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn odd(n: i32) -> bool {
    n % 2 == 1
}

/// Card 54's program.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, &B016, param)
}

/// Card 55's program.
pub fn program_b017(fx: &mut Fx, param: i32) -> i32 {
    run(fx, &B017, param)
}

/// The entry 0x1E02F60 (s0 = D, s1 = the flash count, s2 = the draw
/// count, s3 = param then the slot, s4 = k).
fn run(fx: &mut Fx, v: &V, param: i32) -> i32 {
    let sp = fx.enter(0xD0);
    let eye = fx.camera_eye(0);
    fx.mem.write_u32(D + 0x44C4, eye);
    let at = fx.camera_at30(0);
    fx.mem.write_u32(D + 0x44C8, at);
    fx.mem.write_u32(D + 0x44BC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, v, sp, param) } else { step(fx, v, sp, param) };
    fx.leave(0xD0);
    ret
}

fn init(fx: &mut Fx, v: &V, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4890);
    l2::sine_table(fx, D + 0x4724);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x4520, param);
    fx.mem.set32(TIMER, v.timer);
    for (k, node) in [9, 0x34, 0x35, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODE + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0x44FC, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xC0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xC4)), 180.0);
    fx.fix_residue(y, 0x01E0_3114);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let r = rmod(fx, D, 0x01E0_313C, 0x28);
        fx.mem.set8(BYTES + k, r - 0x14);
    }
    for k in 0..0x100u32 {
        let r = rmod(fx, D, 0x01E0_3184, 0x28);
        fx.mem.set8(BYTES_B + k, r - 0x14);
    }
    for k in 0x64..0xB4u32 {
        fx.mem.write_u8(BYTES_B + k, 0);
    }
    0
}

/// The last XYZ2 of the last primitive drawn since `before` (0 when none).
fn last_xyz2(fx: &mut Fx, before: usize) -> u32 {
    match fx.out.prims.get(before..).and_then(|p| p.last()) {
        Some(p) => {
            let d = p.desc;
            let n = u32::from(fx.mem.read_u16(d));
            fx.mem.read_u32(d + 8 + 8 * n)
        }
        None => 0,
    }
}

fn step(fx: &mut Fx, v: &V, sp: u32, param: i32) -> i32 {
    let from_emit = emit(fx, v, param);
    // The entry's s1: the flash tasks counted here.
    let mut flashes = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        let frame = match f {
            0 => continue,
            FLASH => {
                flashes += 1;
                continue;
            }
            TINT => 0x80,
            FLARE | STAR | SPARK | ARC | RING | BURST => 0x20,
            _ => panic!("b016: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        // The steps' saves: the entry's s0 = D.
        fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]);
        match f {
            FLARE => flare(fx, t, flashes),
            STAR => star(fx, t),
            SPARK => spark(fx, t),
            ARC => arc(fx, t, flashes),
            RING => ring(fx, t),
            BURST => burst(fx, t),
            _ => tint(fx, t, s),
        }
        fx.leave(frame);
    }
    // a3 as the draws' last library call left it (else the emitter's), for
    // the unit flash's first TEXFLUSH (b491's rule) [run].
    let mut a3 = from_emit;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        let tex = |fx: &mut Fx, at: u32| {
            let w = fx.mem.read_u32(at);
            fx.mem.write_u32(D + 0x88, w);
        };
        let uv4 = |fx: &mut Fx, u: i32, w: i32| {
            for (o, x) in [(0x94u32, u), (0x96, 0), (0x98, w), (0x9A, 0x3F)] {
                fx.mem.set16(D + o, x);
            }
        };
        let cell = |fx: &mut Fx, r: u32| {
            let sub = fx.mem.i16(t + 0x56);
            l2::rect(fx, r, 0x14, sub);
            for k in 0..4 {
                let x = fx.mem.read_u32(r + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, x);
            }
        };
        let rings = |fx: &mut Fx, table: u32| {
            fx.mem.set16(D + 0xB8, 0xA);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, table);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        };
        if f == FLARE {
            tex(fx, TEX);
            let u = fx.mem.i16(t + 0x56) << 6;
            uv4(fx, u, 0x3F);
            fx.mem.set16(D + 0xBE, 5);
            billboard::quad(fx, uv::uv_rect, D, t, 0x21, t + 0x10);
            a3 = 0;
        }
        if f == STAR {
            tex(fx, TEX + 4);
            cell(fx, sp + 0x70);
            rings(fx, BYTES_B);
            a3 = 0;
        }
        if f == SPARK {
            tex(fx, TEX + 4);
            cell(fx, sp + 0x80);
            rings(fx, BYTES);
            a3 = 0;
        }
        if f == ARC {
            fx.mem.set16(D + 0xB8, 0x24);
            let before = fx.out.prims.len();
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            a3 = last_xyz2(fx, before);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f == RING {
            tex(fx, TEX);
            cell(fx, sp + 0x90);
            let u = fx.mem.i16(t + 0x56) << 6;
            uv4(fx, u, 0x3F);
            rings(fx, BYTES);
            a3 = 0;
        }
        if f == BURST {
            tex(fx, TEX + 8);
            uv4(fx, 0, 0x1F);
            fx.mem.set16(D + 0xB8, 0x12);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sparks(fx, D, t, 0x20, t + 0x10);
            a3 = 0;
        }
        if f == 0x01E0_2800 || f == 0x01E0_2940 {
            panic!("b016: unported draw case {f:#x}");
        }
    }
    if flashes != 0 {
        unit_flash(fx, a3);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    let c = fx.mem.i16(COUNTER);
    if c == v.returns[0] {
        1
    } else if c == v.returns[1] {
        4
    } else if c == v.returns[2] {
        2
    } else if fx.mem.i32(TIMER) > 0 {
        0
    } else {
        8
    }
}

/// 0x1E01BC0(D, 0) (frame 0x40: ra, the entry's s0 = D): the update
/// counter and the emitters; what its last call leaves in a3.
fn emit(fx: &mut Fx, v: &V, param: i32) -> u32 {
    let mut a3 = 0;
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x10, 0x01E0_3208), (0, u64::from(D))]);
    // The spawns' saves: s0 = D, the entry's s1 = 0, s2 = 0, s3 = the
    // parameter, s4 the runner's (0 [H]).
    let p = param as i64 as u64;
    let go = |fx: &mut Fx, f: u32, jal: u32| {
        let saves = [(0x50, u64::from(jal + 8)), (0x40, 0), (0x30, p), (0x20, 0), (0x10, 0), (0, u64::from(D))];
        spawn::task70(fx, D, POOL, f, 0, &saves);
    };
    let node = |fx: &mut Fx, jal: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODE);
        node_position_saving(fx, h, s, POS, u64::from(D), Some(0), jal + 8);
    };
    let push = |fx: &mut Fx, by: f32| {
        let z = fx.mem.f(POS + 8);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, by) } else { ee::add(z, by) };
        fx.mem.setf(POS + 8, z);
    };
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if fx.mem.i16(COUNTER) == v.flash_at {
        go(fx, FLASH, 0x01E0_1BF0);
        go(fx, FLASH, 0x01E0_1C04);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x6A..0x6E).contains(&c) {
        node(fx, 0x01E0_1C38);
        push(fx, 10.0);
        let h = fx.target_handle();
        unit_position_saving(fx, h, sp + 0x20, u64::from(D), 0x01E0_1C94);
        object_a3(fx, h, &mut a3);
        let z = fx.mem.read_u32(sp + 0x28);
        fx.mem.write_u32(POS + 8, z);
        for by in [10.0f32, -20.0] {
            fx.mem.set16(SLOT, 0);
            fx.mem.set16(SLOT, 1);
            let x = if by > 0.0 { ee::add(fx.mem.f(POS), by) } else { ee::sub(fx.mem.f(POS), 20.0) };
            fx.mem.setf(POS, x);
        }
        fx.mem.set16(SLOT, 0);
        fx.mem.set16(SLOT, 1);
    }
    if fx.mem.i16(COUNTER) == v.burst {
        node(fx, 0x01E0_1D04);
        push(fx, v.dz);
        let mut jal = 0x01E0_1D58u32;
        let mut next = |fx: &mut Fx, f: u32| {
            go(fx, f, jal);
            jal += 0x14;
        };
        next(fx, FLARE);
        for _ in 0..2 {
            next(fx, ARC);
        }
        for _ in 0..7 {
            next(fx, SPARK);
        }
        for _ in 0..6 {
            next(fx, RING);
        }
        fx.mem.set16(SLOT, 0);
        go(fx, BURST, 0x01E0_1E9C);
        let mut jal = 0x01E0_1EB0u32;
        for _ in 0..5 {
            go(fx, BURST, jal);
            jal += 0x14;
        }
        go(fx, TINT, 0x01E0_1F14);
    }
    let c = fx.mem.i16(COUNTER);
    if c == v.stars[0] || c == v.stars[1] {
        node(fx, 0x01E0_1F48);
        let mut jal = 0x01E0_1F5Cu32;
        for _ in 0..7 {
            go(fx, STAR, jal);
            jal += 0x14;
        }
    }
    fx.leave(0x40);
    a3
}

/// What 0x1F71A0 / 0x1F6830 leave in a3: a valid handle's object index
/// runs 0x1F40D0, which leaves a3 = (index < 0x54) (b611's rule) [F, run].
fn object_a3(fx: &mut Fx, h: i32, a3: &mut u32) {
    let (side, slot) = (h >> 8, h & 0xFF);
    if (0..3).contains(&side) && slot < 0xB {
        let i = fx.object_index(side, slot);
        if i >= 0 {
            *a3 = u32::from(i < 0x54);
        }
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// The alpha byte `a` held at `hold` while the age is under +6, else down
/// by 11 (the task ends under 13).
fn hold_fade(fx: &mut Fx, t: u32, hold: u8) {
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, hold);
    } else {
        fade_out(fx, t, 0x63, 0xD, 0xB);
    }
}

fn fade_in(fx: &mut Fx, t: u32, at: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + at);
    if a < cap {
        fx.mem.write_u8(t + at, a.wrapping_add(up));
    }
}

/// Byte `at` down by `down` while at least `floor`, else the task ends.
fn fade_out(fx: &mut Fx, t: u32, at: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + at);
    if a >= floor {
        fx.mem.write_u8(t + at, a.wrapping_sub(down));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// rand % 8 + 1 (the row) and rand % 3 + `life` (the hold) from the
/// calls returning to `ra` / `ra` + 0x20.
fn row_life(fx: &mut Fx, t: u32, ra: u32, life: i32) {
    let r = fx.rand_from(t, ra);
    fx.mem.set16(t + 8, r % 8 + 1);
    let r = rmod(fx, t, ra + 0x20, 3);
    fx.mem.set16(t + 6, r + life);
    fx.mem.set16(t + 0x58, 2);
}

/// 0x1E01FF0: a white flare (size 140, cell 0, half +0x50 = rand >> 4 % 2)
/// turned rand % 360; in by 0x60 below 0xC0 for 8 updates, then out by 19;
/// facing the camera. `s1` the entry's (the flash count).
fn flare(fx: &mut Fx, t: u32, s1: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x8C);
        fx.mem.set16(t + 0xA, 0x8C);
        fx.mem.set16(t + 0x56, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        let r = rmod(fx, t, 0x01E0_2038, 0x168);
        fx.mem.set16(t + 0x5A, r);
        let r = fx.rand_from(t, 0x01E0_2054);
        fx.mem.set16(t + 0x50, (r >> 4) % 2);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 8 {
        fade_in(fx, t, 0x63, 0xC0, 0x60);
    } else {
        fade_out(fx, t, 0x63, 0x15, 0x13);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(s1), 0x01E0_20D0);
    age(fx, t);
}

/// 0x1E020F0: a white star ring (size 50, cell rand % 4, radius 10..29
/// growing by 15..39, turned rand % 360); alpha 0x60 for 2..4 updates,
/// then out by 11; +0x52 counting from 1.
fn star(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4248_0000);
        let r = fx.rand_from(t, 0x01E0_2118);
        fx.mem.set16(t + 0x56, r % 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x52, 1);
        let r = rmod(fx, t, 0x01E0_2150, 0x14);
        fx.mem.set16(t + 0x50, r + 0xA);
        let r = rmod(fx, t, 0x01E0_2170, 0x168);
        fx.mem.set16(t + 0x5A, r);
        let r = rmod(fx, t, 0x01E0_218C, 0x19);
        fx.mem.set16(t + 0xE, r + 0xF);
        row_life(fx, t, 0x01E0_21AC, 2);
    }
    hold_fade(fx, t, 0x60);
    let n = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, n);
    let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, r);
    age(fx, t);
}

/// 0x1E02270: a red spark ring (size 120, cell rand % 4, (0xFF, 0x60,
/// 0x60), radius 10..29 growing by 8..22 from age 1); alpha 0xC0 for 2..4
/// updates, then out by 11.
fn spark(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42F0_0000);
        let r = fx.rand_from(t, 0x01E0_2298);
        fx.mem.set16(t + 0x56, r % 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0x60, 0x60);
        let r = rmod(fx, t, 0x01E0_22CC, 0x14);
        fx.mem.set16(t + 0x50, r + 0xA);
        let r = rmod(fx, t, 0x01E0_22EC, 0x168);
        fx.mem.set16(t + 0x5A, r);
        let r = rmod(fx, t, 0x01E0_2308, 0xF);
        fx.mem.set16(t + 0xE, r + 8);
        row_life(fx, t, 0x01E0_2328, 2);
    }
    hold_fade(fx, t, 0xC0);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E023F0: an arc ring (inner 0, outer 200, grey / (0xC0, 0x40,
/// 0x40)); the outer alpha in by 0x50 below 0xA0 for 12 updates, then out
/// by 12; facing the camera, spun by 0x4000 a update.
fn arc(fx: &mut Fx, t: u32, s1: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0xC0, 0x40, 0x40);
        fx.mem.set16(t + 6, 0xC);
        fx.mem.write_u32(t + 0x4C, 0x4348_0000);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x67, 0xA0, 0x50);
    } else {
        fade_out(fx, t, 0x67, 0xE, 0xC);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(s1), 0x01E0_24BC);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
}

/// 0x1E024F0: a flickering ring (size 8, cell 0, white / (0x80, 0x60,
/// 0x60) by age, radius 10..29 growing by 4..10 from age 1); alpha 0xC0
/// for 20..22 updates, then out by 11.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4100_0000);
        fx.mem.set16(t + 0x56, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0x60, 0x60);
        let r = rmod(fx, t, 0x01E0_2534, 0x14);
        fx.mem.set16(t + 0x50, r + 0xA);
        let r = rmod(fx, t, 0x01E0_2554, 0x168);
        fx.mem.set16(t + 0x5A, r);
        let r = rmod(fx, t, 0x01E0_2570, 7);
        fx.mem.set16(t + 0xE, r + 4);
        row_life(fx, t, 0x01E0_2590, 0x14);
    }
    hold_fade(fx, t, 0xC0);
    if odd(fx.mem.i16(t + 0x54)) {
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
    } else {
        l2::rgb(fx, t + 0x60, 0x80, 0x60, 0x60);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E026B0: a white spark burst (sizes 100 growing by 180 / 11, +0x5A
/// 20, +0x5C 3, turned rand % 0xFFFF on each axis); alpha 0x80 for 11
/// updates, then out by 14.
fn burst(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42C8_0000);
        fx.mem.write_u32(t + 0x4C, 0x42C8_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x5A, 0x14);
        fx.mem.set16(t + 0x5C, 3);
        fx.mem.setf(t + 0x3C, ee::div(180.0, 11.0));
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        for (o, ra) in [(0x20u32, 0x01E0_273Cu32), (0x24, 0x01E0_2758), (0x28, 0x01E0_2774)] {
            let r = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, r);
        }
    }
    if fx.mem.i16(t + 0x54) < 0xB {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade_out(fx, t, 0x63, 0x10, 0xE);
    }
    let s = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x3C));
    fx.mem.setf(t + 0x48, s);
    age(fx, t);
}

/// 0x1E02B40 (frame 0x80 at `sp`): a red full-window tint (0x80, 0x20,
/// 0x20) on the frame (b585's frame flash shape, ST inset +0x48 = 0, ALPHA
/// 0x48); in by 0x28 below 0x50 for 8 updates, then out by 14.
fn tint(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x28;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for (k, c) in [0x80u8, 0x20, 0x20].into_iter().enumerate() {
        fx.mem.write_u8(d + 0x10 + k as u32, c);
    }
    let a = fx.mem.read_u8(t + 0x63);
    fx.mem.write_u8(d + 0x13, a);
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    for (at, v) in [(d + 0x20, 0x6FF8u16), (d + 0x30, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(d + 0x18, ee::div(s, 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(d + 0x1C, ee::div(s, 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(d + 0x28, ee::div(ee::sub(512.0, s), 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(d + 0x2C, ee::div(ee::sub(512.0, s), 512.0));
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, 0x80_0000_0048);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if fx.mem.i16(t + 0x54) < 8 {
        fade_in(fx, t, 0x63, 0x50, 0x28);
    } else {
        fade_out(fx, t, 0x63, 0x10, 0xE);
    }
    age(fx, t);
}

/// 0x1E02D10(D, t): b491's flash step in white: the target over the
/// window (UVs from +8 / 10 and +0xA / 10 over 512, the bottom edge at
/// (512 - 3 (+0xA / 10)) / 512) into the descriptor at D+0x44D0, submitted
/// once with no ordering table; alpha up by 0x20 below 0xC0 for 10
/// updates, then down by 9. `k` the unit flash's s1 (the slot index).
fn flash(fx: &mut Fx, t: u32, k: u32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x30, 0x01E0_390C), (0x20, u64::from(k)), (0x10, u64::from(D))]);
    // swc1 f20 / f21: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 4, 0);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let h = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
    }
    let hd = fx.self_handle();
    let s = fx.mem.i16(NODE);
    node_position_saving(fx, hd, s, sp + 0x40, u64::from(t), Some(u64::from(D)), 0x01E0_2DB0);
    let hd = fx.self_handle();
    unit_position_saving(fx, hd, sp + 0x40, u64::from(t), 0x01E0_2DC4);
    let d = fx.mem.read_u32(FLASH_DESC);
    for k in 0..4u32 {
        let b = fx.mem.read_u8(t + 0x60 + k);
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
    let bottom = ee::div(ee::sub(512.0, ee::mul(3.0, h)), 512.0);
    for (o, v) in [(0x18u32, ee::div(w, 512.0)), (0x1C, ee::div(h, 512.0)), (0x28, ee::div(ee::sub(512.0, w), 512.0)), (0x2C, bottom)] {
        fx.mem.setf(d + o, v);
    }
    fx.submit(d, 0, 0);
    if fx.mem.i16(t + 0x54) < 0xA {
        fade_in(fx, t, 0x63, 0xC0, 0x20);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    age(fx, t);
    fx.leave(0x50);
}

/// 0x1E03840(D) (b491's 0x1E03F90): in a packet layer (0x2078A0) the
/// attacker drawn into the target (0x1F8270 filter 0; ALPHA 0x48),
/// TEXFLUSH before and after; every flash task stepped (0x1E02D10); the
/// layer linked into the ordering table at key 0.
fn unit_flash(fx: &mut Fx, a3: u32) {
    let sp = fx.enter(0xA0);
    let d = sp + 0x48;
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
    let (_, regs) = fx.unit_draw(h, 0, sp + 0x88, sp + 0x90);
    fx.texflush(regs);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) != 0 && fx.mem.read_u32(t + 0x44) == FLASH {
            flash(fx, t, k);
        }
    }
    let layer = fx.mem.read_u32(LAYER);
    fx.layer_end(layer, OT, 0);
    fx.leave(0xA0);
}
