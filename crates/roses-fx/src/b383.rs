//! BMODULE b383 (card 225), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04150, data area 0x1ED0000) [F]: b207's program shape on 150 tasks of 0x40 bytes at D+0x4F4
//! (alive +0x36, step +0x24), spawned by `tstep::spawn` on colour D+0x3004,
//! parameter D+0x3062 and the point D+0x2D0. Nodes 0x1B / 0x18 / 9 / 0x52
//! at D+0x3048..; two 10-point histories D+0x2AF0 / D+0x2B90 of nodes 0x1B
//! / 0x18 (filled on update 0x7D, pushed by `spawn::node_history`). The
//! emitter 0x1E02330 counts D+0x3068: 0x1B..0x3A and 0x49..0x64 a drift
//! mote 0x1E034A0 a update at nodes 9 and 0x52 (record sprite 0x1E05090);
//! 0x3A / 0x66 twenty swaying motes 0x1E03330 at each (y - 2); 0x7D a
//! ribbon (step inline in the entry, `ribbon_g` over the histories);
//! 0x7D..0x98 a cloud 0x1E03640 a update at node 0x1B (`cloud::cloud`);
//! 0x9B a disc 0x1E03DF0 (`fan18_far0`), a six-ray star 0x1E029C0 (record
//! draw 0x1E052A0), seven `band9_tex` slivers 0x1E03B60, three 18-wedge
//! fans 0x1E02AE0 (record draw 0x1E01CC0), five jittered fans 0x1E02CC0
//! (record draw 0x1E019A0), three more fans 0x1E02F00 (0x1E01CC0) and a
//! camera light 0x1E03ED0 (D+0x2AA0, 0x1F7E20); 0x9F nine clouds
//! 0x1E03130. 0x50..0x6F the spawn parameter cycles 0..0xD. Steps
//! 0x1E03830 / 0x38E0 / 0x3990 / 0x3A60 / 0x3CF0 / 0x4040 / 0x4100 are
//! tested but never spawned (not ported). Returns 1 on update 0x91, 4 on
//! 0xAF, 2 on 0x9B, 8 once the 200-update timer D+0x3000 has run out. No
//! 0x1F8100.
use crate::blib::{cloud, exe as bexe, quads4, spawn as bspawn, spline, tstep, uv};
use crate::bus::Bus;
use crate::c061::{desc_uv4, rgbaq_at};
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;

const STAR: u32 = 0x01E0_29C0;
const FAN_A: u32 = 0x01E0_2AE0;
const JITTER: u32 = 0x01E0_2CC0;
const FAN_B: u32 = 0x01E0_2F00;
const CLOUD_A: u32 = 0x01E0_3130;
const SWAY: u32 = 0x01E0_3330;
const MOTE: u32 = 0x01E0_34A0;
const RIBBON: u32 = 0x01E0_3590;
const CLOUD_B: u32 = 0x01E0_3640;
const SLIVER: u32 = 0x01E0_3B60;
const DISC: u32 = 0x01E0_3DF0;
const LIGHT: u32 = 0x01E0_3ED0;

const POOL: u32 = D + 0x4F4;
const POS: u32 = D + 0x2D0;
const HIST_A: u32 = D + 0x2AF0;
const HIST_B: u32 = D + 0x2B90;
const LAMP: u32 = D + 0x2AA0;
const TIMER: u32 = D + 0x3000;
const COLOUR: u32 = D + 0x3004;
const OT2: u32 = D + 0x300C;
const NODES: u32 = D + 0x3048;
const PARAM: u32 = D + 0x3062;
const CYCLE: u32 = D + 0x3064;
const PARAM_COPY: u32 = D + 0x3066;
const COUNTER: u32 = D + 0x3068;
const BYTES: u32 = D + 0x306C;
const SINES: u32 = D + 0x316C;

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

/// The entry 0x1E04150 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1F0);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(OT2, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp) };
    fx.leave(0x1F0);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x32E0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(PARAM_COPY, param);
    fx.mem.set32(TIMER, 0xC8);
    for o in [0x310u32, 0x314, 0x318] {
        fx.mem.write_u32(D + o, 0);
    }
    for (k, (n, clear)) in [(0x1B, 1), (0x18, 0), (9, 0), (0x52, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    for k in 0xA..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x368 + 4 * (k as u32 - 0xA), t);
    }
    let t = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x340, t);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(D + 0x2A74 + 4 * k as u32, t);
    }
    // rand's frame: s0 = the loop counter.
    for k in 0..0x100u32 {
        let v = fx.rand_from(k, 0x01E0_4288);
        let r = fx.div(v, 0x1E).1;
        fx.mem.write_u8(BYTES + k, (r - 0xF) as u8);
    }
    for k in 0..8u32 {
        let v = fx.rand_from(k, 0x01E0_42BC);
        fx.mem.set16(D + 0xC0 + 2 * k, v.wrapping_rem(4));
    }
    0
}

/// Node slot `k`'s position into `out`.
fn node_at(fx: &mut Fx, k: u32, out: u32) {
    let h = fx.self_handle();
    let n = fx.mem.i16(NODES + 2 * k);
    fx.node_position(h, n, out);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    for (k, hist, buf) in [(0u32, HIST_A, sp + 0x130), (1, HIST_B, sp + 0x140)] {
        if counter(fx) == 0x7D {
            node_at(fx, k, buf);
            for i in 0..9 {
                for j in 0..3 {
                    let w = fx.mem.read_u32(buf + 4 * j);
                    fx.mem.write_u32(hist + 0x10 * i + 4 * j, w);
                }
            }
        }
    }
    let n = fx.mem.i16(NODES);
    bspawn::node_history(fx, HIST_A + 0x90, 0xA, n);
    let n = fx.mem.i16(NODES + 2);
    bspawn::node_history(fx, HIST_B + 0x90, 0xA, n);
    for k in 0..150 {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            STAR => run(fx, t, 0x30, star),
            FAN_A => run(fx, t, 0x20, fan_a),
            JITTER => run(fx, t, 0x20, |fx, t| fan_b(fx, t, JITTER)),
            FAN_B => run(fx, t, 0x20, |fx, t| fan_b(fx, t, FAN_B)),
            CLOUD_A => run(fx, t, 0x20, |fx, t| cloud_step(fx, t, CLOUD_A)),
            SWAY => run(fx, t, 0x30, |fx, t| mote(fx, t, SWAY)),
            MOTE => run(fx, t, 0x20, |fx, t| mote(fx, t, MOTE)),
            RIBBON => ribbon(fx, t),
            CLOUD_B => run(fx, t, 0x20, |fx, t| cloud_step(fx, t, CLOUD_B)),
            SLIVER => run(fx, t, 0x20, sliver),
            DISC => run(fx, t, 0x20, disc),
            LIGHT => run(fx, t, 0x30, light),
            _ => {}
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    let d = sp + 0x158;
    // The entry's s0: one more for each live task (rand's saved s0).
    let mut s0 = 0u32;
    for k in 0..150 {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        s0 += 1;
        match fx.mem.read_u32(t + 0x24) {
            STAR => {
                cell(fx, t, D + 0x340, sp + 0x60, 0x14);
                fx.mem.set16(D + 0xB8, 0);
                star_draw(fx, t, d);
                fx.mem.set16(D + 0xB8, 0);
            }
            FAN_A | FAN_B => {
                let r = if fx.mem.read_u32(t + 0x24) == FAN_A { sp + 0x70 } else { sp + 0x90 };
                cell(fx, t, D + 0x37C, r, 0x97);
                fx.mem.set16(D + 0xB8, 0);
                fan18(fx, t, d);
                fx.mem.set16(D + 0xB8, 0);
            }
            JITTER => {
                cell(fx, t, D + 0x37C, sp + 0x80, 0x97);
                fx.mem.set16(D + 0xB8, 1);
                jitter_fan(fx, t, d);
                fx.mem.set16(D + 0xB8, 0);
            }
            CLOUD_A | CLOUD_B => {
                let (r, ra) = if fx.mem.read_u32(t + 0x24) == CLOUD_A { (sp + 0xA0, 0x01E0_4A68) } else { (sp + 0xB0, 0x01E0_4BA0) };
                cell(fx, t, D + 0x2A74, r, 0x14);
                fx.mem.set16(D + 0xB8, 4);
                let v = fx.rand_from(s0, ra);
                fx.mem.set16(D + 0xBE, v.wrapping_rem(4));
                cloud::cloud(fx, D, t, d, BYTES);
                fx.mem.set16(D + 0xB8, 0);
            }
            SWAY | MOTE => {
                sprite(fx, t, d, false);
                sprite(fx, t, d, true);
            }
            RIBBON => spline::ribbon_g(fx, t, HIST_A, HIST_B),
            SLIVER => {
                cell(fx, t, D + 0x340, sp + 0x110, 0x14);
                fx.mem.set16(D + 0xB8, 0);
                quads4::band9_tex(fx, D, t, d);
                fx.mem.set16(D + 0xB8, 0);
            }
            DISC => quads4::fan18_far0(fx, D, t, d),
            _ => {}
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0x91 => 1,
        0xAF => 4,
        0x9B => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The texture word at `tex` and the atlas cell `id` frame +0x34 (0x106960
/// into `r`) copied to D+0x88 / D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, tex: u32, r: u32, id: i32) {
    let v = fx.mem.read_u32(tex);
    fx.mem.write_u32(D + 0x88, v);
    let f = fx.mem.i16(t + 0x34);
    l2::rect(fx, r, id, f);
    for k in 0..4 {
        let w = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, w);
    }
}

/// A step's frame (its saves: ra, s0 = the task; s1 = D in the 0x30
/// frames).
fn run(fx: &mut Fx, t: u32, size: u32, f: impl FnOnce(&mut Fx, u32)) {
    let sp = fx.enter(size);
    let at = fx.mem.read_u32(t + 0x24);
    let ra = u64::from(match at {
        STAR => 0x01E0_43FCu32,
        FAN_A => 0x01E0_4420,
        JITTER => 0x01E0_4440,
        FAN_B => 0x01E0_4460,
        CLOUD_A => 0x01E0_4480,
        SWAY => 0x01E0_44A0,
        MOTE => 0x01E0_44C0,
        CLOUD_B => 0x01E0_4588,
        SLIVER => 0x01E0_4738,
        DISC => 0x01E0_4778,
        _ => 0x01E0_4798,
    });
    if size == 0x20 {
        fx.saved(sp, &[(0x10, ra), (0, 0)]);
    } else {
        fx.saved(sp, &[(0x20, ra), (0x10, u64::from(D)), (0, 0)]);
    }
    f(fx, t);
    fx.leave(size);
}

fn age(fx: &mut Fx, t: u32) -> i32 {
    fx.mem.i16(t + 0x32)
}

fn grow(fx: &mut Fx, t: u32) {
    let n = age(fx, t) + 1;
    fx.mem.set16(t + 0x32, n);
}

fn first(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x36) == 1
}

fn odd(fx: &mut Fx, t: u32) -> bool {
    age(fx, t).wrapping_rem(2) == 1
}

/// Alpha +0x2B down by `down` while at least `floor`, else the task ends.
fn fall(fx: &mut Fx, t: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a >= floor {
        fx.mem.write_u8(t + 0x2B, a - down);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// Alpha +0x2B up by `up` while below `below`.
fn rise(fx: &mut Fx, t: u32, below: u8, up: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a < below {
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(up));
    }
}

fn rand_rem(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v, m).1
}

/// 0x1E029C0: the six-ray star (size 5, frame 0, angle rand % 360, white,
/// spin +8 = 3 into +6 after update 0): alpha 0x80 on update 0, then down
/// by 0x12 (ends below 0x14); the frame +0x34 up to 7.
fn star(fx: &mut Fx, t: u32) {
    let h = fx.self_handle();
    let sp = fx.sp;
    fx.unit_rotation(h, sp + 0x20);
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 5);
        fx.mem.set16(t + 0x34, 0);
        let r = rand_rem(fx, t, 0x01E0_2A00, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.set16(t + 8, 3);
        fx.mem.set16(t + 6, 0);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
    }
    if age(fx, t) <= 0 {
        if fx.mem.read_u8(t + 0x2B) < 0x80 {
            fx.mem.write_u8(t + 0x2B, 0x80);
        }
    } else {
        fall(fx, t, 0x14, 0x12);
    }
    if age(fx, t) != 0 && fx.mem.i16(t + 0x34) < 7 {
        let v = fx.mem.i16(t + 0x34) + 1;
        fx.mem.set16(t + 0x34, v);
    }
    if age(fx, t) != 0 {
        let v = fx.mem.i16(t + 6).wrapping_add(fx.mem.i16(t + 8));
        fx.mem.set16(t + 6, v);
    }
    grow(fx, t);
}

/// The flames' flicker: (0x80, 0x40) on odd ages, else (0xB0, 0x80).
fn flicker(fx: &mut Fx, t: u32) {
    if odd(fx, t) {
        l2::rgb(fx, t + 0x28, 0x80, 0x40, 0);
    } else {
        l2::rgb(fx, t + 0x28, 0xB0, 0x80, 0);
    }
}

/// The colour by the parameter +0x30: orange (0x80, 0x40, 0) on 0, else
/// cyan (0, 0x80, 0x80).
fn tint(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x30) == 0 {
        l2::rgb(fx, t + 0x28, 0x80, 0x40, 0);
    } else {
        l2::rgb(fx, t + 0x28, 0, 0x80, 0x80);
    }
}

/// 0x1E02AE0: a flame fan (size 10 growing by +0xE = rand % 2 + 2, turn
/// +0x10 rand % 0x4000 + 0x2000, +0x14 / +0x18 rand % 0x4000 - 0x2000,
/// frame rand % 7): alpha up by 0x40 (below 0x80) on ages 0 / 1, then
/// down by 9 (ends below 0xB).
fn fan_a(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0xA);
        let v = fx.rand_from(t, 0x01E0_2B08);
        fx.mem.set32(t + 0x10, v.wrapping_rem(0x4000) + 0x2000);
        let v = fx.rand_from(t, 0x01E0_2B28);
        fx.mem.set32(t + 0x14, v.wrapping_rem(0x4000) - 0x2000);
        let v = fx.rand_from(t, 0x01E0_2B48);
        fx.mem.set32(t + 0x18, v.wrapping_rem(0x4000) - 0x2000);
        let r = rand_rem(fx, t, 0x01E0_2B68, 7);
        fx.mem.set16(t + 0x34, r);
        fx.mem.set16(t + 0x36, 2);
        let v = fx.rand_from(t, 0x01E0_2B8C);
        fx.mem.set16(t + 0xE, v.wrapping_rem(2) + 2);
        tint(fx, t);
    }
    if age(fx, t) < 2 {
        rise(fx, t, 0x80, 0x40);
    } else {
        fall(fx, t, 0xB, 9);
    }
    flicker(fx, t);
    if age(fx, t) != 0 {
        let v = fx.mem.i16(t + 0x38).wrapping_add(fx.mem.i16(t + 0xE));
        fx.mem.set16(t + 0x38, v);
    }
    grow(fx, t);
}

/// 0x1E02CC0 (`JITTER`: turn +0x10..+0x18 three rand % 0xFFFF, growth
/// rand % 2 + 1, fading by 9 below 0xB) / 0x1E02F00 (`FAN_B`: turn +0x10
/// rand % 0x4000, +0x14 rand % 0xFFFF, growth rand % 3 + 4, fading by 0xD
/// below 0xF): size 0, frame rand % 7, +6 = 1 (parameter 0) or 4; alpha
/// up by 0x40 (below 0x80) on ages 0 / 1; on parameter 1 +6 up by 3 after
/// update 0.
fn fan_b(fx: &mut Fx, t: u32, at: u32) {
    let jit = at == JITTER;
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0);
        if jit {
            for (o, ra) in [(0x10u32, 0x01E0_2CE4u32), (0x14, 0x01E0_2D00), (0x18, 0x01E0_2D1C)] {
                let r = rand_rem(fx, t, ra, 0xFFFF);
                fx.mem.set32(t + o, r);
            }
            let r = rand_rem(fx, t, 0x01E0_2D38, 7);
            fx.mem.set16(t + 0x34, r);
        } else {
            let v = fx.rand_from(t, 0x01E0_2F24);
            fx.mem.set32(t + 0x10, v.wrapping_rem(0x4000));
            let r = rand_rem(fx, t, 0x01E0_2F44, 0xFFFF);
            fx.mem.set32(t + 0x14, r);
            let r = rand_rem(fx, t, 0x01E0_2F60, 7);
            fx.mem.set16(t + 0x34, r);
        }
        fx.mem.set16(t + 0x36, 2);
        let p = fx.mem.i16(t + 0x30);
        let g = if jit {
            let v = fx.rand_from(t, if p == 0 { 0x01E0_2D6C } else { 0x01E0_2D9C });
            v.wrapping_rem(2) + 1
        } else {
            rand_rem(fx, t, if p == 0 { 0x01E0_2F94 } else { 0x01E0_2FC4 }, 3) + 4
        };
        fx.mem.set16(t + 0xE, g);
        fx.mem.set16(t + 6, if p == 0 { 1 } else { 4 });
        tint(fx, t);
    }
    if age(fx, t) < 2 {
        rise(fx, t, 0x80, 0x40);
    } else if jit {
        fall(fx, t, 0xB, 9);
    } else {
        fall(fx, t, 0xF, 0xD);
    }
    flicker(fx, t);
    if age(fx, t) != 0 {
        let v = fx.mem.i16(t + 0x38).wrapping_add(fx.mem.i16(t + 0xE));
        fx.mem.set16(t + 0x38, v);
    }
    if fx.mem.i16(t + 0x30) == 1 && age(fx, t) != 0 {
        let v = fx.mem.i16(t + 6) + 3;
        fx.mem.set16(t + 6, v);
    }
    grow(fx, t);
}

/// 0x1E03130 (`CLOUD_A`: size rand % 0x50 + 0x1E, radius +0x30 rand %
/// 0xFA + 0x64, spread +0xE rand % 5 + 5) / 0x1E03640 (`CLOUD_B`: size
/// 0xA, radius rand % 100 + 0xA, spread rand % 5 + 4): grey (0x60, 0x60,
/// 0x60), frame 0, angle rand % 360, +8 rand % 10, life +6 (rand & 7) +
/// 3; alpha up by 0x40 (to 0xC0) for the life, then the task ends; the
/// frame up to 6 on odd ages; the radius up by +0xE after update 0; the
/// turn +0x10..+0x18 three rand % 360 every update.
fn cloud_step(fx: &mut Fx, t: u32, at: u32) {
    let a = at == CLOUD_A;
    if first(fx, t) {
        if a {
            let r = rand_rem(fx, t, 0x01E0_3154, 0x50);
            fx.mem.set16(t + 0x38, r + 0x1E);
            fx.mem.set16(t + 0x34, 0);
            l2::rgb(fx, t + 0x28, 0x60, 0x60, 0x60);
            let r = rand_rem(fx, t, 0x01E0_3188, 0xFA);
            fx.mem.set16(t + 0x30, r + 0x64);
            let r = rand_rem(fx, t, 0x01E0_31A8, 0x168);
            fx.mem.set16(t + 0x3A, r);
            let r = rand_rem(fx, t, 0x01E0_31C4, 5);
            fx.mem.set16(t + 0xE, r + 5);
            let r = rand_rem(fx, t, 0x01E0_31E4, 0xA);
            fx.mem.set16(t + 8, r);
            let v = fx.rand_from(t, 0x01E0_3200);
            fx.mem.set16(t + 6, v.wrapping_rem(8) + 3);
        } else {
            fx.mem.set16(t + 0x38, 0xA);
            fx.mem.set16(t + 0x34, 0);
            l2::rgb(fx, t + 0x28, 0x60, 0x60, 0x60);
            let r = rand_rem(fx, t, 0x01E0_3684, 0x64);
            fx.mem.set16(t + 0x30, r + 0xA);
            let r = rand_rem(fx, t, 0x01E0_36A4, 0x168);
            fx.mem.set16(t + 0x3A, r);
            let r = rand_rem(fx, t, 0x01E0_36C0, 5);
            fx.mem.set16(t + 0xE, r + 4);
            let r = rand_rem(fx, t, 0x01E0_36E0, 0xA);
            fx.mem.set16(t + 8, r);
            let v = fx.rand_from(t, 0x01E0_36FC);
            fx.mem.set16(t + 6, v.wrapping_rem(8) + 3);
        }
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < fx.mem.i16(t + 6) {
        rise(fx, t, 0x81, 0x40);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    if odd(fx, t) && fx.mem.i16(t + 0x34) < 6 {
        let v = fx.mem.i16(t + 0x34) + 1;
        fx.mem.set16(t + 0x34, v);
    }
    if age(fx, t) != 0 {
        let v = fx.mem.i16(t + 0x30).wrapping_add(fx.mem.i16(t + 0xE));
        fx.mem.set16(t + 0x30, v);
    }
    let ras = if a { [0x01E0_32B8u32, 0x01E0_32D4, 0x01E0_32F0] } else { [0x01E0_37B4, 0x01E0_37D0, 0x01E0_37EC] };
    for (k, ra) in ras.into_iter().enumerate() {
        let r = rand_rem(fx, t, ra, 0x168);
        fx.mem.set32(t + 0x10 + 4 * k as u32, r);
    }
    grow(fx, t);
}

/// 0x1E03330 (`SWAY`: size 0x3C, alpha 0xF0 for 7 updates, swaying: +8 /
/// +0xC += 11 sin / cos of +0x3A through 0x1A2B80) / 0x1E034A0 (`MOTE`:
/// size 0x28, alpha 0xF0 for 2 updates): white, frame rand % 7, +0x30 /
/// angle +0x3A rand % 360; then down by 0x14 (ends below 0x16).
fn mote(fx: &mut Fx, t: u32, at: u32) {
    let sway = at == SWAY;
    let b = if sway { 0x01E0_3330 } else { 0x01E0_34A0 - 0x8 };
    if first(fx, t) {
        fx.mem.set16(t + 0x38, if sway { 0x3C } else { 0x28 });
        let r = rand_rem(fx, t, b + 0x30, 7);
        fx.mem.set16(t + 0x34, r);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        let r = rand_rem(fx, t, b + 0x5C, 0x168);
        fx.mem.set16(t + 0x30, r);
        let r = rand_rem(fx, t, b + 0x78, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < if sway { 7 } else { 2 } {
        fx.mem.write_u8(t + 0x2B, 0xF0);
    } else {
        fall(fx, t, 0x16, 0x14);
    }
    if sway {
        for (o, cos, ra) in [(8u32, 0, 0x01E0_3438u32), (0xC, 1, 0x01E0_3470)] {
            let deg = fx.mem.i16(t + 0x3A);
            let s = l2::sin_deg(fx, SINES, deg, cos);
            let m = ee::mul(11.0, s);
            let v = ee::add(ee::from_i32(fx.mem.i16(t + o)), m);
            fx.fix_residue(v, ra);
            fx.mem.set16(t + o, ee::fixsfsi(v));
        }
    }
    grow(fx, t);
}

/// 0x1E03590 (inline in the entry): the ribbon (size 6, (0xF0, 0x10,
/// 0x10), +6 = 9): alpha 0x60 for 0x19 updates, then down by 9 (ends
/// below 0xB).
fn ribbon(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        l2::rgb(fx, t + 0x28, 0xF0, 0x10, 0x10);
        fx.mem.set16(t + 6, 9);
        fx.mem.write_u8(t + 0x2B, 0x80);
        fx.mem.set16(t + 0x38, 6);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 0x19 {
        fx.mem.write_u8(t + 0x2B, 0x60);
    } else {
        fall(fx, t, 0xB, 9);
    }
    grow(fx, t);
}

/// 0x1E03B60: a sliver band (size 0 growing by +8 = rand % 3 + 2, turn
/// +0x10 rand % 0x4000 - 0x2000, +0x14 rand % 0xFFFF, frame rand % 7, +6
/// = 0x14, (0xC0, 0x80, 0) / rim (0xA0, 8, 8)): alpha 0x80 for 3
/// updates, then down by 0xE (ends below 0x10).
fn sliver(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0);
        let v = fx.rand_from(t, 0x01E0_3B84);
        fx.mem.set32(t + 0x10, v.wrapping_rem(0x4000) - 0x2000);
        let r = rand_rem(fx, t, 0x01E0_3BA4, 0xFFFF);
        fx.mem.set32(t + 0x14, r);
        let r = rand_rem(fx, t, 0x01E0_3BC0, 7);
        fx.mem.set16(t + 0x34, r);
        fx.mem.set16(t + 0x36, 2);
        let r = rand_rem(fx, t, 0x01E0_3BE4, 3);
        fx.mem.set16(t + 8, r + 2);
        fx.mem.set16(t + 6, 0x14);
        fx.mem.set16(t + 0x30, 0);
        l2::rgb(fx, t + 0x28, 0xC0, 0x80, 0);
        l2::rgb(fx, t + 0x2C, 0xA0, 8, 8);
    }
    if age(fx, t) < 3 {
        if fx.mem.read_u8(t + 0x2B) < 0x80 {
            fx.mem.write_u8(t + 0x2B, 0x80);
        }
    } else {
        fall(fx, t, 0x10, 0xE);
    }
    if age(fx, t) != 0 {
        let v = fx.mem.i16(t + 0x38).wrapping_add(fx.mem.i16(t + 8));
        fx.mem.set16(t + 0x38, v);
    }
    grow(fx, t);
}

/// 0x1E03DF0: the bright disc (size 0xC8, (0xF0, 0xF0, 0xF0) / (0x10,
/// 0x10, 0xF0), angle rand % 360): alpha up by 0x60 on ages 0 / 1, then
/// down by 0xF (ends below 0x11).
fn disc(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0xC8);
        l2::rgb(fx, t + 0x28, 0xF0, 0xF0, 0xF0);
        l2::rgb(fx, t + 0x2C, 0x10, 0x10, 0xF0);
        let r = rand_rem(fx, t, 0x01E0_3E44, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if age(fx, t) < 2 {
        let a = fx.mem.read_u8(t + 0x2B);
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x60));
    } else {
        fall(fx, t, 0x11, 0xF);
    }
    grow(fx, t);
}

/// 0x1E03ED0: the camera light D+0x2AA0 (camera 0's slots 4 / 5,
/// 0x1F7E20) at the task's point: (128, 5, 0, 0) / (0, 140, level), the
/// level D+0x2AC8 up by 0.2 to 1, then down by 0.1 to 0.1, when the task
/// ends (the slots cleared); it ends with the timer at 1 too.
fn light(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x40A0_0000), (8, 0), (0xC, 0), (0x20, 0), (0x24, 0x430C_0000), (0x28, 0)] {
            fx.mem.write_u32(LAMP + o, v);
        }
        for k in 0..3 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(LAMP + 0x10 + 4 * k, v);
        }
        fx.mem.set16(t + 0x36, 2);
        fx.mem.set16(t + 0x30, 0);
    }
    let lv = fx.mem.f(LAMP + 0x28);
    if fx.mem.i16(t + 0x30) == 0 {
        if lv < 1.0 {
            fx.mem.setf(LAMP + 0x28, ee::add(lv, f32::from_bits(0x3E4C_CCCD)));
        } else {
            fx.mem.set16(t + 0x30, 1);
        }
    } else {
        let tenth = f32::from_bits(0x3DCC_CCCD);
        if lv <= tenth {
            fx.mem.set16(t + 0x36, 0);
        } else {
            fx.mem.setf(LAMP + 0x28, ee::sub(lv, tenth));
        }
    }
    let p = if fx.mem.i16(t + 0x36) == 0 { 0 } else { LAMP };
    bexe::camera_slots45(fx, p);
    if fx.mem.i32(TIMER) == 1 {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// Submit `d` D+0xB8 + 1 times at the mean w `w` into D+0x80.
fn submit_n(fx: &mut Fx, d: u32, w: u32) {
    let mut n = 0;
    while n < fx.mem.i16(D + 0xB8) + 1 {
        let key = ee::fixunssfsi(fx.mem.f(w)) as i32;
        let ot = fx.mem.read_u32(D + 0x80);
        fx.submit(d, ot, key);
        n += 1;
    }
}

/// The strip3 wedge descriptor of b148's fan9 (centre +0x28, rim +0x2C,
/// UVs D+0x94 in D+0xBE's order, texture D+0x88) and the task's TRS
/// (point +0, turn +0x10, no scale) at `m` from the position `p`.
fn wedge_desc(fx: &mut Fx, t: u32, d: u32, m: u32, p: u32) {
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x28, 1);
    l2::colour_strip3(fx, d, t + 0x2C, 2);
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(p + 4 * k, v);
    }
    l2::xform_vp(fx, m, 0, t + 0x10, p);
}

/// bf602f806bb7 at 0x1E01CC0 (D, task, desc, 0): b148's fan9 (554995841cb2)
/// with eighteen wedges of 60 degrees every 20 [F fd.sh: only those three
/// constants differ].
fn fan18(fx: &mut Fx, t: u32, d: u32) {
    let sp = fx.enter(0x120);
    wedge_desc(fx, t, d, sp + 0xD0, sp + 0xC0);
    for o in [0x84u32, 0x94, 0xA4, 0xB4, 0x80, 0x88, 0x90, 0x98] {
        fx.mem.set32(sp + o, 0);
    }
    let tab = D + 0xD8;
    let mut a = 0i32;
    for _ in 0..0x12 {
        let s = (fx.mem.i16(t + 0x3A) + a) as i16 as i32;
        let e = (a + (fx.mem.i16(t + 0x3A) + 0x3C)) as i16 as i32;
        for (deg, v) in [(s, 0xA0u32), (e, 0xB0)] {
            let c = l2::sin_deg(fx, tab, deg, 1);
            let r = ee::from_i32(fx.mem.i16(t + 0x38));
            fx.mem.setf(sp + v, ee::mul(r, c));
            let sn = l2::sin_deg(fx, tab, deg, 0);
            let r = ee::from_i32(fx.mem.i16(t + 0x38));
            fx.mem.setf(sp + v + 8, ee::mul(r, sn));
        }
        if l2::project(fx, sp + 0x80, SPR, sp + 0xD0, d, sp + 0x110) {
            submit_n(fx, d, sp + 0x110);
        }
        a = (a + 0x14) as i16 as i32;
    }
    fx.leave(0x120);
}

/// efd5fd90f76e at 0x1E019A0 (D, task, desc, 0): [`fan18`] in a 0x130
/// frame with each rim coordinate jittered by rand % 30 - 15 (rand's
/// saved s0 the descriptor).
fn jitter_fan(fx: &mut Fx, t: u32, d: u32) {
    let sp = fx.enter(0x130);
    wedge_desc(fx, t, d, sp + 0xE0, sp + 0xD0);
    for o in [0x94u32, 0xA4, 0xB4, 0xC4, 0x90, 0x98, 0xA0, 0xA8] {
        fx.mem.set32(sp + o, 0);
    }
    let tab = D + 0xD8;
    let mut a = 0i32;
    for _ in 0..0x12 {
        let s = (fx.mem.i16(t + 0x3A) + a) as i16 as i32;
        let e = (a + (fx.mem.i16(t + 0x3A) + 0x3C)) as i16 as i32;
        for (deg, v, ra) in [(s, 0xB0u32, 0x01E0_1AD4u32), (e, 0xC0, 0x01E0_1B90)] {
            let c = l2::sin_deg(fx, tab, deg, 1);
            let r = ee::from_i32(fx.mem.i16(t + 0x38));
            let f = ee::mul(r, c);
            let j = rand_rem(fx, d, ra, 0x1E);
            fx.mem.setf(sp + v, ee::sub(ee::add(f, ee::from_i32(j)), 15.0));
            let sn = l2::sin_deg(fx, tab, deg, 0);
            let r = ee::from_i32(fx.mem.i16(t + 0x38));
            let f = ee::mul(r, sn);
            let j = rand_rem(fx, d, ra + 0x58, 0x1E);
            fx.mem.setf(sp + v + 8, ee::sub(ee::add(f, ee::from_i32(j)), 15.0));
        }
        if l2::project(fx, sp + 0x90, SPR, sp + 0xE0, d, sp + 0x120) {
            submit_n(fx, d, sp + 0x120);
        }
        a = (a + 0x14) as i16 as i32;
    }
    fx.leave(0x130);
}

/// The textured-quad header (9 values, FST, ALPHA 0x48, vertex alphas 0)
/// of `d`, coloured +0x28 (RGBAQ through `local`).
fn quad_desc(fx: &mut Fx, t: u32, d: u32, local: u32) {
    desc_uv4(fx, d);
    fx.desc_alpha(d, 0x80_0000_0048);
    fx.mem.write_u32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    rgbaq_at(fx, local, t + 0x28, d + 0x10);
}

/// The camera's negated yaw in degrees at `a` (the 0xB60B60B7 `mult`).
fn yaw_degrees_neg(fx: &mut Fx, a: u32) {
    let v = fx.mem.i32(a);
    fx.mult(0xB60B_60B7u32 as i32, v);
    let hi = fx.hilo().1;
    let q = (hi.wrapping_add(v) >> 7).wrapping_add(((v as u32) >> 31) as i32);
    fx.mem.set32(a, q.wrapping_neg());
}

/// 0x1E052A0 (D, task, desc, 0): the six-ray star: six textured squares
/// (UVs D+0x94 orientation 0, texture D+0x88) of the i16 size +0x38
/// turned about y by the camera's negated yaw, pushed out at radius i16
/// +6 in the xz plane at angles +0x3A + 60 k, in the task's TRS (point,
/// turn +0x10); each submitted D+0xB8 + 1 times at its mean w.
fn star_draw(fx: &mut Fx, t: u32, d: u32) {
    let sp = fx.enter(0x150);
    quad_desc(fx, t, d, sp + 0x148);
    uv::uv_rect4(fx, d, D + 0x94, 0);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(sp + 0xD0 + 4 * k, v);
    }
    crate::b494::camera_angles_saving(fx, sp + 0x130, 0, u64::from(sp + 0xD4), 0x01E0_53C4);
    yaw_degrees_neg(fx, sp + 0x134);
    l2::xform_vp(fx, sp + 0xE0, 0, t + 0x10, sp + 0xD0);
    fx.mem.write_u32(sp + 0xD0, 0);
    let mut a = 0i32;
    for _ in 0..6 {
        let v = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x90, v, v);
        l2::rotate4(fx, sp + 0x90, sp + 0x130, 0, D + 0xD8);
        let deg = (fx.mem.i16(t + 0x3A) + a) as i16 as i32;
        let c = l2::sin_deg(fx, D + 0xD8, deg, 1);
        let r = ee::from_i32(fx.mem.i16(t + 6));
        fx.mem.setf(sp + 0xD8, ee::mul(r, c));
        let s = l2::sin_deg(fx, D + 0xD8, deg, 0);
        let r = ee::from_i32(fx.mem.i16(t + 6));
        fx.mem.setf(sp + 0xD4, ee::mul(r, s));
        for k in 0..3 {
            let f = fx.mem.f(sp + 0xD0 + 4 * k);
            l2::add_column(fx, sp + 0x90, k as i32, f);
        }
        if l2::project(fx, sp + 0x90, SPR, sp + 0xE0, d, sp + 0x120) {
            submit_n(fx, d, sp + 0x120);
        }
        a = (a + 0x3C) as i16 as i32;
    }
    fx.leave(0x150);
}

/// 0x1E05090 (D, task, desc, draw): `draw` 0 builds the quad header in
/// `desc` (UVs of atlas cell 0x14 frame +0x34, orientation 0, texture
/// D+0x2A78); 1 draws it: a camera-facing square of the i16 size +0x38 at
/// the task's point plus its drift +8 / +0xA / +0xC over 10 (scale 0.1),
/// keyed by its mean w D+0x3034 into D+0x300C.
fn sprite(fx: &mut Fx, t: u32, d: u32, draw: bool) {
    let sp = fx.enter(0x120);
    if !draw {
        let f = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0xB0, 0x14, f);
        quad_desc(fx, t, d, sp + 0x118);
        uv::uv_rect4(fx, d, sp + 0xB0, 0);
        let tex = fx.mem.read_u32(D + 0x2A78);
        fx.desc_texture(d, tex, 1);
    } else {
        for k in 0..3 {
            fx.mem.write_u32(sp + 0xA0 + 4 * k, 0x3DCC_CCCD);
        }
        crate::b494::camera_angles_saving(fx, sp + 0x100, u64::from(d), u64::from(t), 0x01E0_519C);
        for k in 0..3 {
            let p = ee::from_i32(fx.mem.i16(t + 2 * k));
            let o = ee::div(ee::from_i32(fx.mem.i16(t + 8 + 2 * k)), 10.0);
            fx.mem.setf(sp + 0x90 + 4 * k, ee::add(p, o));
        }
        l2::xform_vp(fx, sp + 0xC0, sp + 0xA0, sp + 0x100, sp + 0x90);
        let v = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x50, v, v);
        if l2::project(fx, sp + 0x50, SPR, sp + 0xC0, d, D + 0x3034) {
            let key = ee::fixunssfsi(fx.mem.f(D + 0x3034)) as i32;
            let ot = fx.mem.read_u32(OT2);
            fx.submit(d, ot, key);
        }
    }
    fx.leave(0x120);
}

/// `tstep::spawn(D, f, 0)`.
fn spawn(fx: &mut Fx, f: u32) {
    tstep::spawn(fx, D, f, 0);
}

/// Node slot `k`'s position (into the emitter's sp + 0x50) copied to the
/// spawn point, y less `dy`.
fn at_node(fx: &mut Fx, sp: u32, k: u32, dy: f32) {
    node_at(fx, k, sp + 0x50);
    let x = fx.mem.f(sp + 0x50);
    fx.mem.setf(POS, x);
    let y = fx.mem.f(sp + 0x54);
    fx.mem.setf(POS + 4, if dy == 0.0 { y } else { ee::sub(y, dy) });
    let z = fx.mem.f(sp + 0x58);
    fx.mem.setf(POS + 8, z);
}

/// 0x1E02330(D, 0) (frame 0x60): the spawn colour (0x80, 0, 0, 0), the
/// update counter, the parameter cycle and the spawns.
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x60);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    if counter(fx) == 0x7D {
        spawn(fx, RIBBON);
    }
    let c = counter(fx);
    if (0x50..0x70).contains(&c) {
        let v = fx.mem.i16(CYCLE);
        fx.mem.set16(PARAM, v);
        fx.mem.set16(CYCLE, v + 1);
        if fx.mem.i16(CYCLE) >= 0xE {
            fx.mem.set16(CYCLE, 0);
        }
    }
    if counter(fx) == 0x9B {
        at_node(fx, sp, 0, 0.0);
        spawn(fx, DISC);
        spawn(fx, STAR);
        for _ in 0..7 {
            spawn(fx, SLIVER);
        }
        for _ in 0..3 {
            spawn(fx, FAN_A);
        }
        fx.mem.set16(PARAM, 0);
        for _ in 0..5 {
            spawn(fx, JITTER);
        }
        fx.mem.set16(PARAM, 0);
        for _ in 0..3 {
            spawn(fx, FAN_B);
        }
        spawn(fx, LIGHT);
    }
    if counter(fx) == 0x9F {
        at_node(fx, sp, 0, 0.0);
        for _ in 0..9 {
            spawn(fx, CLOUD_A);
        }
    }
    let c = counter(fx);
    if c == 0x3A || c == 0x66 {
        for k in [2u32, 3] {
            at_node(fx, sp, k, 2.0);
            for _ in 0..0x14 {
                spawn(fx, SWAY);
            }
        }
    }
    let c = counter(fx);
    if (0x1B..0x3B).contains(&c) {
        for k in [2u32, 3] {
            at_node(fx, sp, k, 0.0);
            spawn(fx, MOTE);
        }
    }
    let c = counter(fx);
    if (0x49..0x65).contains(&c) {
        for k in [2u32, 3] {
            at_node(fx, sp, k, 0.0);
            spawn(fx, MOTE);
        }
    }
    let c = counter(fx);
    if (0x7D..0x99).contains(&c) {
        at_node(fx, sp, 0, 0.0);
        spawn(fx, CLOUD_B);
    }
    fx.leave(0x60);
}
