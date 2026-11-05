//! BMODULE b018 / b019 (cards 56 / 57; b019 the same code with its globals
//! moved, [`L57`]), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02E00, data area 0x1ED0000) [F]: a pool of 200 tasks of 0x40 bytes at D+0x478 (alive +0x36, step
//! +0x24, each step tested by the entry and called by its own `jal`, two
//! stepped inline); spawn 0x1E03640 on colour D+0x3824, parameter D+0x387A
//! and the point D+0x2D0. Two 10-point histories, D+0x3770 of the
//! attacker's node 0x19 and D+0x36D0 of its node 0x4C (filled on update
//! 0x18, then pushed by `spawn::node_history`). The emitter 0x1E01BD0
//! counts D+0x387E: on update 1 the object op task 0x1E02DB0 (0x1F8380(2,
//! 0, ..) every update from then on); on 0x18 a `ribbon_g` ribbon task
//! 0x1E02560 between the histories and a `ribbon_cross` task 0x1E02AC0
//! along the second; on 0x15..0x19 eight `ring_t20` ember rings 0x1E02950
//! at node 0x19 (and on 0x15 the camera light 0x1E02B70); on 0x60 / 0x61
//! eight more at node 0x4C (z the target's); on 0x60 eight `ring_t20`
//! spark rings 0x1E027C0, thirty `ring_t` ember clouds 0x1E02610
//! (`tstep::ember`), a `fan18` fire disc 0x1E02480 and the camera light.
//! The steps 0x1E01F60 / 0x1E02180 / 0x1E022E0 / 0x1E02CF0 (and the draws
//! 0x1E03760 / 0x1E03B10) are tested but never spawned (not ported).
//! Returns 1 on update 0x58, 4 on 0x69, 2 on 0x62, 8 once the 150-update
//! timer D+0x3820 has run out. No 0x1F8100.
use crate::blib::{billboard, quads4, spawn as bspawn, spline, tstep, uv};
use crate::bus::Bus;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;

const FIRE: u32 = 0x01E0_2480;
const RIBBON: u32 = 0x01E0_2560;
const EMBER: u32 = 0x01E0_2610;
const SPARK: u32 = 0x01E0_27C0;
const RING: u32 = 0x01E0_2950;
const CROSS: u32 = 0x01E0_2AC0;
const LIGHT: u32 = 0x01E0_2B70;
const OBJECT: u32 = 0x01E0_2DB0;

/// The data-area layout (b019 / card 57 is b018 with its globals moved).
pub(crate) struct L {
    tasks: u32,
    pool: u32,
    pos: u32,
    hist_a: u32,
    hist_b: u32,
    lamp: u32,
    timer: u32,
    colour: u32,
    nodes: u32,
    param: u32,
    counter: u32,
    bytes: u32,
    /// The second OT word, the bzero size, the sine table, the parameter
    /// copy, three cleared words, the target's node, textures (4, 0..2) /
    /// (4, 0xD..0xF), the node point the emitter keeps.
    ot2: u32,
    size: u32,
    sines: u32,
    param_copy: u32,
    cleared: u32,
    target_node: u32,
    tex: u32,
    tex_d: u32,
    keep: u32,
}

pub(crate) const L56: L = L {
    tasks: 200,
    pool: D + 0x478,
    pos: D + 0x2D0,
    hist_a: D + 0x36D0,
    hist_b: D + 0x3770,
    lamp: D + 0x3680,
    timer: D + 0x3820,
    colour: D + 0x3824,
    nodes: D + 0x3868,
    param: D + 0x387A,
    counter: D + 0x387E,
    bytes: D + 0x3882,
    ot2: D + 0x382C,
    size: 0x3AF0,
    sines: D + 0x3984,
    param_copy: D + 0x387C,
    cleared: D + 0x310,
    target_node: D + 0x3876,
    tex: D + 0x340,
    tex_d: D + 0x374,
    keep: D + 0x36C0,
};

/// b019 (card 57): b018's program with the globals 0x80 / 0x500 lower and
/// 180 tasks at D+0x474 [F word-compare: only these offsets differ].
const L57: L = L {
    tasks: 180,
    pool: D + 0x474,
    pos: D + 0x250,
    hist_a: D + 0x31D0,
    hist_b: D + 0x3270,
    lamp: D + 0x3180,
    timer: D + 0x3320,
    colour: D + 0x3324,
    nodes: D + 0x3358,
    param: D + 0x336A,
    counter: D + 0x336E,
    bytes: D + 0x3370,
    ot2: D + 0x332C,
    size: 0x35E0,
    sines: D + 0x3470,
    param_copy: D + 0x336C,
    cleared: D + 0x290,
    target_node: D + 0x3366,
    tex: D + 0x2C0,
    tex_d: D + 0x2F4,
    keep: D + 0x31C0,
};

fn counter(fx: &mut Fx, l: &L) -> i32 {
    fx.mem.i16(l.counter)
}

/// The entry 0x1E02E00 (D, param) of b018 (card 56).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, &L56, param)
}

/// The entry 0x1E02E00 of b019 (card 57).
pub fn program_b019(fx: &mut Fx, param: i32) -> i32 {
    run(fx, &L57, param)
}

fn run(fx: &mut Fx, l: &L, param: i32) -> i32 {
    let sp = fx.enter(0x160);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(l.ot2, OT);
    let ret = if param >= 0 { init(fx, l, param) } else { step(fx, l, sp, param) };
    fx.leave(0x160);
    ret
}

fn init(fx: &mut Fx, l: &L, param: i32) -> i32 {
    fx.bzero(D, l.size);
    l2::sine_table(fx, l.sines);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(l.param_copy, param);
    fx.mem.set32(l.timer, 0x96);
    for o in [0u32, 4, 8] {
        fx.mem.write_u32(l.cleared + o, 0);
    }
    for (k, (n, clear)) in [(0x19, 1), (0x4C, 0), (0xD, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(l.nodes + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 0);
    fx.mem.set16(l.target_node, r);
    for k in 0..3 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(l.tex + 4 * k as u32, t);
    }
    for k in 0xD..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(l.tex_d + 4 * (k as u32 - 0xD), t);
    }
    // rand's frame: s0 = D.
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_2F24);
        let r = fx.div(v, 0x1E).1;
        fx.mem.write_u8(l.bytes + k, (r - 0xF) as u8);
    }
    0
}

/// A history's nine points from `node`'s position (through `local`).
fn fill(fx: &mut Fx, node: u32, local: u32, hist: u32) {
    let h = fx.self_handle();
    let n = fx.mem.i16(node);
    fx.node_position(h, n, local);
    for i in 0..9 {
        for k in 0..3 {
            let w = fx.mem.read_u32(local + 4 * k);
            fx.mem.write_u32(hist + 0x10 * i + 4 * k, w);
        }
    }
}

fn step(fx: &mut Fx, l: &L, sp: u32, param: i32) -> i32 {
    emit(fx, l, param);
    if counter(fx, l) == 0x18 {
        fill(fx, l.nodes, sp + 0xA0, l.hist_b);
    }
    if counter(fx, l) == 0x18 {
        fill(fx, l.nodes + 2, sp + 0xB0, l.hist_a);
    }
    let n = fx.mem.i16(l.nodes);
    bspawn::node_history(fx, l.hist_b + 0x90, 0xA, n);
    let n = fx.mem.i16(l.nodes + 2);
    bspawn::node_history(fx, l.hist_a + 0x90, 0xA, n);
    for k in 0..l.tasks {
        let t = l.pool + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            FIRE => run_step(fx, t, 0x20, 0x01E0_30C8, |fx, t| fire(fx, FIRE, t)),
            RIBBON => ribbon_step(fx, t),
            EMBER => run_step(fx, t, 0x20, 0x01E0_3188, |fx, t| {
                tstep::ember(fx, EMBER, t);
            }),
            SPARK => run_step(fx, t, 0x20, 0x01E0_31A8, |fx, t| {
                tstep::spark(fx, SPARK, t);
            }),
            RING => run_step(fx, t, 0x20, 0x01E0_31C8, |fx, t| ring(fx, RING, t)),
            CROSS => cross_step(fx, t),
            LIGHT => run_step(fx, t, 0x30, 0x01E0_3290, |fx, t| light(fx, l, t)),
            OBJECT => {
                for (to, from) in [(0x15Cu32, 0x01E0_3E80u32), (0x158, 0x01E0_3E88)] {
                    let w = fx.mem.read_u32(from);
                    fx.mem.write_u32(sp + to, w);
                }
                fx.boundary(0x001F_8380, "object op 0x1F8380", [2, 0, sp + 0x15C, sp + 0x158]);
            }
            _ => {}
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    for k in 0..l.tasks {
        let t = l.pool + 0x40 * k;
        if fx.mem.i16(t + 0x36) != 0 {
            draw(fx, l, sp, t);
        }
    }
    let left = fx.mem.read_u32(l.timer);
    if left != 0 {
        fx.mem.write_u32(l.timer, left - 1);
    }
    match counter(fx, l) {
        0x58 => 1,
        0x69 => 4,
        0x62 => 2,
        _ if fx.mem.i32(l.timer) > 0 => 0,
        _ => 8,
    }
}

/// The texture at `tex` and the atlas cell `id` frame +0x34 (0x106960
/// into `r`) copied to D+0x88 / D+0x94..+0xA3.
pub(crate) fn cell(fx: &mut Fx, t: u32, tex: u32, r: u32, id: i32) {
    let v = fx.mem.read_u32(tex);
    fx.mem.write_u32(D + 0x88, v);
    let f = fx.mem.i16(t + 0x34);
    l2::rect(fx, r, id, f);
    for k in 0..4 {
        let w = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, w);
    }
}

/// The entry's draw case for a live task (`sp` the entry's frame, the
/// descriptor at sp + 0xC8).
fn draw(fx: &mut Fx, l: &L, sp: u32, t: u32) {
    let d = sp + 0xC8;
    match fx.mem.read_u32(t + 0x24) {
        FIRE => {
            cell(fx, t, l.tex_d + 8, sp + 0x60, 0x9B);
            quads4::fan18(fx, D, t, d);
        }
        RIBBON => spline::ribbon_g(fx, t, l.hist_a, l.hist_b),
        EMBER => {
            cell(fx, t, l.tex_d + 8, sp + 0x70, 0x9B);
            billboard::ring_t(fx, uv::uv_rect4, D, t, d, l.bytes);
        }
        SPARK => {
            cell(fx, t, l.tex + 4, sp + 0x80, 0xA);
            billboard::ring_t20(fx, uv::uv_rect4, D, t, d, l.bytes);
        }
        RING => {
            cell(fx, t, l.tex_d + 8, sp + 0x90, 0x9B);
            billboard::ring_t20(fx, uv::uv_rect4, D, t, d, l.bytes);
        }
        CROSS => spline::ribbon_cross(fx, t, l.hist_a),
        _ => {}
    }
}

/// A step's frame and its saves of the entry's s0 = D (and s1 = 0 in a
/// 0x30 frame).
pub(crate) fn run_step(fx: &mut Fx, t: u32, size: u32, ra: u32, f: impl FnOnce(&mut Fx, u32)) {
    let sp = fx.enter(size);
    let (ra, d) = (u64::from(ra), u64::from(D));
    if size == 0x20 {
        fx.saved(sp, &[(0x10, ra), (0, d)]);
    } else {
        fx.saved(sp, &[(0x20, ra), (0x10, 0), (0, d)]);
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

/// Alpha +0x2B down by `down` while at least `floor`, else the task ends.
fn fall(fx: &mut Fx, t: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a >= floor {
        fx.mem.write_u8(t + 0x2B, a - down);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// The entry's inline step for 0x1E02560: the ribbon orange (0x80, 0x50,
/// 0), width 5, alpha 0x50 for 0x4B updates, then out by 9; +6 = 10.
pub(crate) fn ribbon_step(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x80, 0x50, 0);
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 5);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 0x4B {
        fx.mem.write_u8(t + 0x2B, 0x50);
    } else {
        fall(fx, t, 0xB, 9);
    }
    fx.mem.set16(t + 6, 0xA);
    grow(fx, t);
}

/// The entry's inline step for 0x1E02AC0: the cross ribbon (0xA0, 0x80,
/// 8), +6 = 9, width 8, alpha 0x60 for 0x4B updates, then out by 9.
pub(crate) fn cross_step(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        l2::rgb(fx, t + 0x28, 0xA0, 0x80, 8);
        fx.mem.set16(t + 6, 9);
        fx.mem.write_u8(t + 0x2B, 0x80);
        fx.mem.set16(t + 0x38, 8);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 0x4B {
        fx.mem.write_u8(t + 0x2B, 0x60);
    } else {
        fall(fx, t, 0xB, 9);
    }
    grow(fx, t);
}

/// 0x1E02480: the fire disc (radius 0x14A, (0xC0, 0x70, 0) centre, (0x20,
/// 0x10, 0) rim alpha 0x10, a random angle): alpha 0x80 for 5 updates,
/// then out by 0xA.
pub(crate) fn fire(fx: &mut Fx, at: u32, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x14A);
        l2::rgb(fx, t + 0x28, 0xC0, 0x70, 0);
        l2::rgb(fx, t + 0x2C, 0x20, 0x10, 0);
        let v = fx.rand_from(t, at + 0x54);
        let r = fx.div(v, 0x168).1;
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0x10);
    }
    if age(fx, t) < 5 {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fall(fx, t, 0x11, 0xA);
    }
    grow(fx, t);
}

/// 0x1E02950: an ember ring (size 2, frame rand & 3, (0x80, 0x40, 0),
/// +0x18 rand % 0xFFFF, radius +0x30 from 0 growing by (rand & 0xF) + 3,
/// angle rand % 360, +8 rand % 10, life rand % 3 + 1): alpha 0x80 for the
/// life, then out by 0xA.
pub(crate) fn ring(fx: &mut Fx, at: u32, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 2);
        let v = fx.rand_from(t, at + 0x28);
        fx.mem.set16(t + 0x34, v & 3);
        l2::rgb(fx, t + 0x28, 0x80, 0x40, 0);
        let v = fx.rand_from(t, at + 0x5C);
        let r = fx.div(v, 0xFFFF).1;
        fx.mem.set32(t + 0x18, r);
        fx.mem.set16(t + 0x30, 0);
        let v = fx.rand_from(t, at + 0x7C);
        let r = fx.div(v, 0x168).1;
        fx.mem.set16(t + 0x3A, r);
        let v = fx.rand_from(t, at + 0x98);
        fx.mem.set16(t + 0xE, (v & 0xF) + 3);
        let v = fx.rand_from(t, at + 0xB8);
        let r = fx.div(v, 0xA).1;
        fx.mem.set16(t + 8, r);
        let v = fx.rand_from(t, at + 0xD4);
        let r = fx.div(v, 3).1;
        fx.mem.set16(t + 6, r + 1);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fall(fx, t, 0x11, 0xA);
    }
    let v = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x30, v);
    grow(fx, t);
}

/// 0x1E02B70: the camera light D+0x3680 (camera 0's slots 4 / 5, 0x1F7E20)
/// at the task's point: its level D+0x36A8 up by 0.2 to 1, then down by
/// 0.1 to 0.1, when the task ends (the slots cleared); it ends with the
/// timer at 1 too.
pub(crate) fn light(fx: &mut Fx, l: &L, t: u32) {
    if first(fx, t) {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x4080_0000), (8, 0x4000_0000), (0xC, 0), (0x20, 0), (0x24, 0x42F0_0000), (0x28, 0)] {
            fx.mem.write_u32(l.lamp + o, v);
        }
        for k in 0..3 {
            let v = crate::ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(l.lamp + 0x10 + 4 * k, v);
        }
        fx.mem.set16(t + 0x36, 2);
        fx.mem.set16(t + 0x30, 0);
    }
    let lv = fx.mem.f(l.lamp + 0x28);
    if fx.mem.i16(t + 0x30) == 0 {
        if lv < 1.0 {
            fx.mem.setf(l.lamp + 0x28, crate::ee::add(lv, f32::from_bits(0x3E4C_CCCD)));
        } else {
            fx.mem.set16(t + 0x30, 1);
        }
    } else {
        let tenth = f32::from_bits(0x3DCC_CCCD);
        if lv <= tenth {
            fx.mem.set16(t + 0x36, 0);
        } else {
            fx.mem.setf(l.lamp + 0x28, crate::ee::sub(lv, tenth));
        }
    }
    let p = if fx.mem.i16(t + 0x36) == 0 { 0 } else { l.lamp };
    crate::blib::exe::camera_slots45(fx, p);
    if fx.mem.i32(l.timer) == 1 {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// 0x1E01BD0(D, 0) (frame 0x60: ra +0x30, s2 +0x20 = the parameter, s1
/// +0x10 = 0, s0 = D): the spawn colour (0x80, 0, 0, 0), the update
/// counter and the spawns.
fn emit(fx: &mut Fx, l: &L, param: i32) {
    let sp = fx.enter(0x60);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, 0x01E0_2F60), (0x20, p), (0x10, 0), (0, u64::from(D))]);
    fx.mem.write_u8(l.colour, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(l.colour + k, 0);
    }
    let n = counter(fx, l) + 1;
    fx.mem.set16(l.counter, n);
    let self_node = |fx: &mut Fx, node: u32, out: u32| {
        let h = fx.self_handle();
        let n = fx.mem.i16(node);
        fx.node_position(h, n, out);
    };
    if counter(fx, l) == 0x33 {
        self_node(fx, l.nodes + 2, l.keep);
    }
    let c = counter(fx, l);
    if (0x34..0x41).contains(&c) {
        self_node(fx, l.nodes + 2, sp + 0x40);
        for k in 0..3 {
            let w = fx.mem.read_u32(l.keep + 4 * k);
            fx.mem.write_u32(l.pos + 4 * k, w);
        }
    }
    // (s1, s2) the emitter's at each spawn.
    let (mut s1, mut s2) = (0u64, p);
    if counter(fx, l) == 1 {
        spawn(fx, l, OBJECT, 0x01E0_1CB4, s1, s2);
    }
    if counter(fx, l) == 0x18 {
        spawn(fx, l, RIBBON, 0x01E0_1CD8, s1, s2);
    }
    if counter(fx, l) == 0x18 {
        spawn(fx, l, CROSS, 0x01E0_1CFC, s1, s2);
    }
    if (0x15..=0x19).contains(&counter(fx, l)) {
        self_node(fx, l.nodes, sp + 0x40);
        for k in 0..3 {
            let w = fx.mem.read_u32(sp + 0x40 + 4 * k);
            fx.mem.write_u32(l.pos + 4 * k, w);
        }
        fx.mem.set16(l.param, 1);
        s1 = u64::from(sp + 0x44);
        for k in 0..8u64 {
            s2 = k;
            spawn(fx, l, RING, 0x01E0_1D90, s1, s2);
        }
        s2 = 8;
        let y = fx.mem.f(sp + 0x44);
        fx.mem.setf(l.pos + 4, crate::ee::add(1.0, y));
        if counter(fx, l) == 0x15 {
            spawn(fx, l, LIGHT, 0x01E0_1DD8, s1, s2);
        }
    }
    let at = |fx: &mut Fx| {
        let h = fx.self_handle();
        let n = fx.mem.i16(l.nodes + 2);
        fx.node_position(h, n, sp + 0x40);
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x50);
        for (to, from) in [(0u32, 0x40u32), (4, 0x44), (8, 0x58)] {
            let w = fx.mem.read_u32(sp + from);
            fx.mem.write_u32(l.pos + to, w);
        }
        fx.mem.set16(l.param, 1);
    };
    let c = counter(fx, l);
    if c == 0x60 || c == 0x61 {
        at(fx);
        for k in 0..8u64 {
            s1 = k;
            spawn(fx, l, RING, 0x01E0_1E58, s1, s2);
        }
    }
    if counter(fx, l) == 0x60 {
        at(fx);
        for k in 0..8u64 {
            s1 = k;
            spawn(fx, l, SPARK, 0x01E0_1EDC, s1, s2);
        }
        for k in 0..0x1Eu64 {
            s1 = k;
            spawn(fx, l, EMBER, 0x01E0_1F04, s1, s2);
        }
        s1 = 0x1E;
        spawn(fx, l, FIRE, 0x01E0_1F28, s1, s2);
        spawn(fx, l, LIGHT, 0x01E0_1F3C, s1, s2);
    }
    fx.leave(0x60);
}

/// 0x1E03640(D, step, 0) (frame 0x60: ra +0x50, s4 / s3 the runner's (0)
/// [H], s2 / s1 the emitter's, s0 = D): the first free task cleared and
/// set off at D+0x2D0 (as i16) with the colour D+0x3824 (twice) and +0x30
/// = D+0x387A.
pub(crate) fn spawn(fx: &mut Fx, l: &L, f: u32, ra: u32, s1: u64, s2: u64) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, s2), (0x10, s1), (0, u64::from(D))]);
    let ret = match (0..l.tasks).find(|&k| fx.mem.i16(l.pool + 0x36 + 0x40 * k) == 0) {
        Some(k) => {
            let t = l.pool + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            for c in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(l.colour), fx.mem.read_u8(l.colour + 1), fx.mem.read_u8(l.colour + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(l.colour + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, l.pos, 0);
            let a = fx.mem.i16(l.param);
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
