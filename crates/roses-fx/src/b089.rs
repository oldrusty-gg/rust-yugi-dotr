//! BMODULE b089 (card 368), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02770) [F]. The entry
//! takes (D, param). b064's family on two task pools of 0x40-byte tasks
//! (alive +0x36, step +0x24 as a record address): A, 20 tasks at D+0xB0
//! (spawn 0x1E030B0), B, 50 tasks at D+0x5B0 (spawn 0x1E031D0; both b636's
//! spawn with this layout). The emitter 0x1E00100 throws motes (0x1E00470)
//! from the attacker's node 0xC on odd updates 85..159 and on update 200
//! six spheres (0x1E00A30), rings (0x1E014C0) and orbs (0x1E016D0) at six
//! points round node 0x38 (D+0x1230 / D+0x1C50 rows); a head (step
//! 0x1E005A0, inline in the entry) keeps a 10-point trail at D+0x2640. A
//! sphere flies to the target from update 335 and bursts there (spawning a
//! camera step 0x1E022C0); orbs burst into sparks / drifts. From update 201
//! wisps / puffs at node 0x38, rising motes, a camera step (0x1E020E0) on
//! 200, the stage light (0x1E026B0). Draws: the records' sprites, a sphere
//! mesh (0x1E03930, a 12 x 12 grid from the init 0x1E048E0) and a gouraud
//! trail (0x1E03D60). Returns 1 on update 0x154, 3 on 0x17C, 6 on
//! 0x15F..0x176, 8 once the 400-update timer D+0x26E0 has run out.
use crate::blib::spawn as lspawn;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const POS: u32 = D;
const TEX: u32 = D + 0x70;
const POOL_A: u32 = D + 0xB0;
const POOL_B: u32 = D + 0x5B0;
const ROW: u32 = D + 0x1230;
const SWAY: u32 = D + 0x1290;
const GRID: u32 = D + 0x12F0;
const SCALE: u32 = D + 0x1BF0;
const CENTRE: u32 = D + 0x1C50;
const VEL: u32 = D + 0x1CB0;
const GRID3: u32 = D + 0x1D10;
const CAM: u32 = D + 0x2610;
const HIST: u32 = D + 0x2640;
const TIMER: u32 = D + 0x26E0;
const COLOUR: u32 = D + 0x26E4;
const OTW: u32 = D + 0x26EC;
const MEAN: u32 = D + 0x2714;
const STATE: u32 = D + 0x3B68;
const NODE_A: u32 = D + 0x3B74;
const NODE_B: u32 = D + 0x3B76;
const ANGLE: u32 = D + 0x3B7E;
const SEL: u32 = D + 0x3B84;
const PARAM: u32 = D + 0x3B86;
const COUNTER: u32 = D + 0x3B88;
const TAB: u32 = D + 0x3B8C;

const MOTE: u32 = 0x01E0_0470;
const HEAD: u32 = 0x01E0_05A0;
const PUFF: u32 = 0x01E0_06E0;
const PUFF_B: u32 = 0x01E0_0890;
const SPHERE: u32 = 0x01E0_0A30;
const SPARK: u32 = 0x01E0_13A0;
const RING: u32 = 0x01E0_14C0;
const ORB: u32 = 0x01E0_16D0;
const WISP: u32 = 0x01E0_1BA0;
const DRIFT: u32 = 0x01E0_1D90;
const RISE: u32 = 0x01E0_1F50;
const CAM_A: u32 = 0x01E0_20E0;
const CAM_B: u32 = 0x01E0_22C0;
const DRIFT_C: u32 = 0x01E0_24F0;
const LIGHT: u32 = 0x01E0_26B0;

pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x110);
    fx.mem.write_u32(OTW, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x110);
    ret
}

/// rand % m (HI) from return address `ra` (rand's frame: `s0`).
fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// 0x1A2B80 on `v` from return address `ra` (its residue).
fn fix(fx: &mut Fx, v: f32, ra: u32) -> i32 {
    fx.fix_residue(v, ra);
    ee::fixsfsi(v)
}

/// 0x1A2C70 on `v` from return address `ra` (its residue).
fn fixu(fx: &mut Fx, v: f32, ra: u32) -> i32 {
    fx.fix_residue(v, ra);
    ee::fixunssfsi(v) as i32
}

/// 0x1A2E08's unpack of the double `x` at `p` (class +0, sign +4, exponent
/// +8, mantissa +0x10).
fn unpack_d(fx: &mut Fx, x: u64, p: u32) {
    fx.mem.write_u32(p + 4, (x >> 63) as u32);
    let e = ((x >> 52) & 0x7FF) as u32;
    let m = x & 0xF_FFFF_FFFF_FFFF;
    if e == 0 {
        fx.mem.write_u32(p, 2);
    } else if e == 0x7FF {
        if m == 0 {
            fx.mem.write_u32(p, 4);
        } else {
            fx.mem.write_u32(p, u32::from(m & (1 << 51) != 0));
            fx.mem.write_u64(p + 0x10, m);
        }
    } else {
        fx.mem.write_u64(p + 0x10, (m << 8) | (1 << 60));
        fx.mem.write_u32(p + 8, e.wrapping_sub(0x3FF));
        fx.mem.write_u32(p, 3);
    }
}

/// (double) `v` < `b` (0x17BCA0, `gt` false) or > `b` (0x17BCC0) through
/// 0x1A2C30 (from `ra1`), the compare called from `ra2` with the caller's
/// s0 `s0`: the frames they leave below the caller's sp (0x1A2C30's unpack,
/// 0x1A38C8's, 0x17BCxx's ra, 0x1A3720's saves and unpacks) [F].
pub(crate) fn dcmp(fx: &mut Fx, v: f32, b: u64, s0: u32, ra1: u32, ra2: u32, gt: bool) -> bool {
    let sp = fx.sp;
    fx.fix_residue(v, ra1);
    let f = sp - 0x30;
    let (class, sign, exp, mant) = (fx.mem.read_u32(f), fx.mem.read_u32(f + 4), fx.mem.read_u32(f + 8), fx.mem.read_u32(f + 0xC));
    let g = sp - 0x60;
    fx.mem.write_u32(g, class);
    fx.mem.write_u32(g + 4, sign);
    fx.mem.write_u32(g + 8, exp);
    fx.mem.write_u64(g + 0x10, u64::from(mant) << 30);
    fx.mem.write_u64(g + 0x20, 0x001A_2C60);
    let bits = v.to_bits();
    let a = if (bits >> 23) & 0xFF == 0 { u64::from(bits >> 31) << 63 } else { f64::from(v).to_bits() };
    fx.saved(sp - 0x10, &[(0, u64::from(ra2))]);
    let h = sp - 0x70;
    fx.mem.write_u64(h + 0x40, a);
    fx.mem.write_u64(h + 0x48, b);
    fx.mem.write_u64(h + 0x50, u64::from(s0));
    fx.mem.write_u64(h + 0x58, if gt { 0x0017_BCD0 } else { 0x0017_BCB0 });
    unpack_d(fx, a, h);
    unpack_d(fx, b, h + 0x20);
    let (x, y) = (f64::from_bits(a), f64::from_bits(b));
    if gt {
        x > y
    } else {
        x < y
    }
}

fn copy3(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

fn idx(fx: &mut Fx, t: u32) -> u32 {
    fx.mem.i16(t + 0x30) as u32
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3D00);
    l2::sine_table(fx, TAB);
    fx.mem.set16(PARAM, param);
    fx.mem.set32(TIMER, 0x190);
    let h = fx.self_handle();
    let r = fx.request_node(h, 0xC, 1);
    fx.mem.set16(NODE_A, r);
    let h = fx.self_handle();
    let r = fx.request_node(h, 0x38, 0);
    fx.mem.set16(NODE_B, r);
    for k in 0..0x10u32 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(TEX + 4 * k, t);
    }
    grid(fx);
    0
}

/// 0x1E048E0(D): the 12 x 12 grid of a radius-20 sphere at D+0x12F0 (rows
/// of 0xC0: x / z by row angle 90 + 30 i (libm cosf), column 30 j; y 20
/// cos(30 i)) and its thirds at D+0x1D10.
fn grid(fx: &mut Fx) {
    fx.enter(0x90);
    let pi = f32::from_bits(0x4049_0FDB);
    let c = crate::blib::exe::libm_cosf(fx, f32::from_bits(0x3FC9_0FDB));
    let mut f20 = ee::mul(20.0, c);
    let mut s4 = 0x5Ai32;
    for i in 0..12u32 {
        let mut s3 = 0i32;
        for j in 0..12u32 {
            let a = ee::div(ee::mul(pi, ee::from_i32(s3)), 180.0);
            let c = crate::blib::exe::libm_cosf(fx, a);
            fx.mem.setf(GRID + 0xC0 * i + 0x10 * j, ee::mul(f20, c));
            let s = fx.libm_sinf(a);
            fx.mem.setf(GRID + 0xC0 * i + 0x10 * j + 8, ee::mul(f20, s));
            s3 = (s3 + 0x1E) as i16 as i32;
        }
        s4 = (s4 + 0x1E) as i16 as i32;
        let a = ee::div(ee::mul(pi, ee::from_i32(s4)), 180.0);
        let c = crate::blib::exe::libm_cosf(fx, a);
        f20 = ee::mul(20.0, c);
    }
    let mut s3 = 0i32;
    for i in 0..12u32 {
        let f = ee::mul(pi, ee::from_i32(s3));
        for j in 0..12u32 {
            let c = crate::blib::exe::libm_cosf(fx, ee::div(f, 180.0));
            fx.mem.setf(GRID + 0xC0 * i + 0x10 * j + 4, ee::mul(20.0, c));
        }
        s3 = (s3 + 0x1E) as i16 as i32;
    }
    for i in 0..12u32 {
        for j in 0..12u32 {
            let p = 0xC0 * i + 0x10 * j;
            copy4(fx, GRID3 + p, GRID + p);
            for c in 0..3 {
                let v = ee::div(fx.mem.f(GRID3 + p + 4 * c), 3.0);
                fx.mem.setf(GRID3 + p + 4 * c, v);
            }
        }
    }
    fx.leave(0x90);
}

/// Pool A's steps, the frame size and the entry's ra after the `jal`.
const STEPS_A: [(u32, u32, u32); 5] = [
    (PUFF, 0x30, 0x01E0_2988),
    (SPHERE, 0x60, 0x01E0_29A8),
    (RING, 0x30, 0x01E0_29C8),
    (ORB, 0x60, 0x01E0_29E8),
    (WISP, 0x50, 0x01E0_2A08),
];

const STEPS_B: [(u32, u32, u32); 10] = [
    (MOTE, 0x20, 0x01E0_2A4C),
    (PUFF_B, 0x30, 0x01E0_2A70),
    (SPARK, 0x30, 0x01E0_2A90),
    (WISP, 0x50, 0x01E0_2AB0),
    (DRIFT, 0x30, 0x01E0_2AD0),
    (RISE, 0x30, 0x01E0_2AF0),
    (CAM_A, 0x40, 0x01E0_2B10),
    (CAM_B, 0x40, 0x01E0_2B30),
    (DRIFT_C, 0x30, 0x01E0_2B50),
    (LIGHT, 0x10, 0x01E0_2B70),
];

/// A step's frame: ra and the entry's s0..s3 as its prologue saves them.
fn step_frame(fx: &mut Fx, a: u32, size: u32, ra: u32, s: [u32; 4]) {
    let f = fx.sp;
    let r = u64::from(ra);
    let s = s.map(u64::from);
    match (a, size) {
        (_, 0x10) => fx.saved(f, &[(0, r)]),
        (_, 0x20) => fx.saved(f, &[(0x10, r), (0, s[0])]),
        (SPHERE, _) => {
            fx.saved(f, &[(0x40, r), (0x30, s[2]), (0x20, s[1]), (0x10, s[0])]);
        }
        (ORB, _) => fx.saved(f, &[(0x40, r), (0x30, s[3]), (0x20, s[2]), (0x10, s[1]), (0, s[0])]),
        _ => fx.saved(f, &[(0x20, r), (0x10, s[1]), (0, s[0])]),
    }
}

fn run_step(fx: &mut Fx, a: u32, t: u32) {
    match a {
        MOTE => mote(fx, t),
        PUFF => puff(fx, t),
        PUFF_B => puff_b(fx, t),
        SPHERE => sphere(fx, t),
        SPARK => spark(fx, t),
        RING => ring(fx, t),
        ORB => orb(fx, t),
        WISP => wisp(fx, t),
        DRIFT => drift(fx, t, 0),
        RISE => rise(fx, t),
        CAM_A => cam_a(fx, t),
        CAM_B => cam_b(fx, t),
        DRIFT_C => drift(fx, t, DRIFT_C - DRIFT),
        _ => light(fx, t),
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..0x14u32 {
        let t = POOL_A + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x24) == HEAD {
            head(fx, sp, t);
        }
        for &(a, size, ra) in STEPS_A.iter() {
            if fx.mem.read_u32(t + 0x24) == a {
                fx.enter(size);
                step_frame(fx, a, size, ra, [D, k, 0, D + 0x40 * k]);
                run_step(fx, a, t);
                fx.leave(size);
            }
        }
    }
    for k in 0..0x32u32 {
        let t = POOL_B + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        for &(a, size, ra) in STEPS_B.iter() {
            if fx.mem.read_u32(t + 0x24) == a {
                fx.enter(size);
                step_frame(fx, a, size, ra, [D, D + 0x40 * k, 0, k]);
                run_step(fx, a, t);
                fx.leave(size);
            }
        }
    }
    let d = sp + 0x80;
    let mut count = 0u32;
    for k in 0..0x14u32 {
        let t = POOL_A + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        count += 1;
        let s = [D, D + 0x40 * k, count, k];
        let st = fx.mem.read_u32(t + 0x24);
        if st == HEAD {
            pair(fx, trail, 0x1E0, 0x90, t, d, 0x01E0_2BBC, s);
        }
        if st == PUFF {
            pair(fx, sprite_a, 0x110, 0x50, t, d, 0x01E0_2BF8, s);
        }
        if st == PUFF_B {
            pair(fx, sprite_p, 0x120, 0x50, t, d, 0x01E0_2C38, s);
        }
        if st == SPHERE {
            pair(fx, mesh, 0x150, 0x90, t, d, 0x01E0_2C78, s);
        }
        if st == RING {
            pair(fx, sprite_a, 0x110, 0x50, t, d, 0x01E0_2CB8, s);
        }
        if st == ORB {
            pair(fx, glow, 0x120, 0x50, t, d, 0x01E0_2CF8, s);
        }
    }
    for k in 0..0x32u32 {
        let t = POOL_B + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let s = [D, D + 0x40 * k, k, 0x14];
        let st = fx.mem.read_u32(t + 0x24);
        if st == MOTE {
            pair(fx, plate, 0x110, 0x50, t, d, 0x01E0_2D5C, s);
        }
        if st == PUFF_B {
            pair(fx, sprite_p, 0x120, 0x50, t, d, 0x01E0_2D98, s);
        }
        if st == SPARK {
            pair(fx, glow, 0x120, 0x50, t, d, 0x01E0_2DD8, s);
        }
        if st == WISP {
            pair(fx, fan, 0x110, 0x60, t, d, 0x01E0_2E18, s);
        }
        if st == DRIFT {
            pair(fx, sprite_d, 0x110, 0x50, t, d, 0x01E0_2E58, s);
        }
        if st == RISE {
            pair(fx, sprite_r, 0x110, 0x50, t, d, 0x01E0_2E98, s);
        }
        if st == DRIFT_C {
            pair(fx, sprite_d, 0x110, 0x50, t, d, 0x01E0_2ED8, s);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0xC9..0x154).contains(&c) {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE_B);
        fx.node_position(h, n, sp + 0x50);
        copy3(fx, POS, sp + 0x50);
        spawn(fx, POOL_B, 0x32, WISP);
        spawn(fx, POOL_B, 0x32, PUFF_B);
    }
    let c = fx.mem.i16(COUNTER);
    if (0xC9..0x136).contains(&c) && c % 4 == 1 {
        let r = rmod(fx, D, 0x01E0_2FCC, 6);
        fx.mem.set16(SEL, r);
        spawn(fx, POOL_B, 0x32, RISE);
    }
    if fx.mem.i16(COUNTER) == 0xC8 {
        spawn(fx, POOL_B, 0x32, CAM_A);
    }
    let n = fx.mem.i32(TIMER);
    if n != 0 {
        fx.mem.set32(TIMER, n - 1);
    }
    let c = fx.mem.i16(COUNTER);
    if c == 0x154 {
        1
    } else if c == 0x17C {
        3
    } else if (0x15F..0x177).contains(&c) {
        6
    } else if fx.mem.i32(TIMER) > 0 {
        0
    } else {
        8
    }
}

type Draw = fn(&mut Fx, u32, u32, i32);

/// A draw called with mode 0 then 1 (the entry's ra after each `jal`:
/// `ra`, `ra` + 0x14), its frame (`size`, ra at `ro`) holding the entry's
/// s0..s3.
#[allow(clippy::too_many_arguments)]
fn pair(fx: &mut Fx, f: Draw, size: u32, ro: u32, t: u32, d: u32, ra: u32, s: [u32; 4]) {
    for (mode, r) in [(0, ra), (1, ra + 0x14)] {
        let fr = fx.sp - size;
        fx.saved(fr, &[(ro, u64::from(r)), (0x30, u64::from(s[3])), (0x20, u64::from(s[2])), (0x10, u64::from(s[1])), (0, u64::from(s[0]))]);
        f(fx, t, d, mode);
    }
}

/// 0x1E030B0 / 0x1E031D0(D, step, arg): the first free task of the pool
/// cleared and set off at D+0 (as i16) with the colour D+0x26E4 (twice),
/// +0x30 = D+0x3B84; 1 when one was free.
fn spawn(fx: &mut Fx, pool: u32, n: u32, step: u32) -> i32 {
    fx.enter(0x60);
    let ret = match (0..n).find(|&k| fx.mem.i16(pool + 0x40 * k + 0x36) == 0) {
        Some(k) => {
            let t = pool + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            for c in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, POS, 0);
            let a = fx.mem.i16(SEL);
            fx.mem.set16(t + 0x30, a);
            fx.mem.set32(t + 0x20, 0);
            fx.mem.write_u32(t + 0x24, step);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// 0x1E00100(D, 0): the spawn colour, the update counter and the spawns.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x90);
    fx.saved(sp, &[(0x60, 0x01E0_2834), (0x20, 0), (0x10, param as i64 as u64), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, POOL_B, 0x32, LIGHT);
    }
    let h = fx.self_handle();
    let n = fx.mem.i16(NODE_A);
    fx.node_position(h, n, sp + 0x70);
    let c = fx.mem.i16(COUNTER);
    if c >= 0x55 && c % 2 == 1 && c < 0xA0 {
        for _ in 0..3 {
            let r = rmod(fx, D, 0x01E0_01C4, 0x168);
            fx.mem.set16(ANGLE, r);
            let x = fx.mem.read_u32(sp + 0x70);
            fx.mem.write_u32(POS, x);
            let r = rmod(fx, D, 0x01E0_01E8, 0x1E);
            fx.mem.setf(POS + 4, ee::from_i32(r + 0x20));
            let z = fx.mem.read_u32(sp + 0x78);
            fx.mem.write_u32(POS + 8, z);
            spawn(fx, POOL_B, 0x32, MOTE);
        }
    }
    let h = fx.self_handle();
    let n = fx.mem.i16(NODE_B);
    fx.node_position(h, n, sp + 0x70);
    if fx.mem.i16(COUNTER) == 0xC8 {
        let r = rmod(fx, D, 0x01E0_0268, 0x168);
        fx.mem.set16(ANGLE, r);
        copy3(fx, POS, sp + 0x70);
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE_A);
        fx.node_position(h, n, sp + 0x80);
        for k in 0..9 {
            copy3(fx, HIST + 0x10 * k, sp + 0x80);
        }
        let mut a = 0i32;
        for k in 0..6u32 {
            let c = l2::sin_deg(fx, TAB, a, 1);
            let x = ee::add(fx.mem.f(sp + 0x70), ee::mul(14.0, c));
            fx.mem.setf(CENTRE + 0x10 * k, x);
            fx.mem.setf(ROW + 0x10 * k, x);
            let s = l2::sin_deg(fx, TAB, a, 0);
            let y = ee::add(fx.mem.f(sp + 0x74), ee::mul(14.0, s));
            fx.mem.setf(CENTRE + 0x10 * k + 4, y);
            fx.mem.setf(ROW + 0x10 * k + 4, y);
            let z = fx.mem.read_u32(sp + 0x78);
            fx.mem.write_u32(CENTRE + 0x10 * k + 8, z);
            fx.mem.write_u32(ROW + 0x10 * k + 8, z);
            a = (a + 0x3C) as i16 as i32;
        }
        for step in [SPHERE, RING, ORB] {
            for s in 0..6 {
                fx.mem.set16(SEL, s);
                spawn(fx, POOL_A, 0x14, step);
            }
        }
        for k in 0..10 {
            copy3(fx, HIST + 0x10 * k, sp + 0x70);
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x70);
    fx.leave(0x90);
}

/// The yaw of the attacker (0x1F7370's rotation y / 180 through 0x1A2B80
/// from `ra`, as i16) with the rotation at `out`.
fn yaw(fx: &mut Fx, out: u32, ra: u32) -> i32 {
    let h = fx.self_handle();
    fx.unit_rotation(h, out);
    let y = ee::div(ee::from_i32(fx.mem.i32(out + 4)), 180.0);
    fix(fx, y, ra) as i16 as i32
}

/// The entry's case for step 0x1E005A0 (pool A): a head pushed 7 a update
/// along the attacker's facing, its point pushed into the trail D+0x2640.
fn head(fx: &mut Fx, sp: u32, t: u32) {
    l2::rgb(fx, t + 0x28, 0x5F, 0x50, 0x50);
    for c in 0..3u32 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * c));
        fx.mem.setf(sp + 0x60 + 4 * c, v);
    }
    lspawn::history(fx, HIST + 0x90, 0xA, sp + 0x60);
    let y = yaw(fx, sp + 0x70, 0x01E0_28F8);
    let c = l2::sin_deg(fx, TAB, y - 0xB4, 1);
    let z = ee::sub(ee::from_i32(fx.mem.i16(t + 4)), ee::mul(7.0, c));
    let r = fix(fx, z, 0x01E0_2934);
    fx.mem.set16(t + 4, r);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 5);
        fx.mem.set16(t + 0x36, 2);
    }
    fx.mem.write_u8(t + 0x2B, 0x50);
    fx.mem.set16(t + 0x32, 8);
}

/// i16 `at` += `k` x (sine or cosine of i16 `deg`) through 0x1A2B80.
fn wobble(fx: &mut Fx, t: u32, at: u32, deg: u32, cos: i32, k: f32, ra: u32) {
    let a = fx.mem.i16(t + deg);
    let s = l2::sin_deg(fx, TAB, a, cos);
    let f = ee::add(ee::from_i32(fx.mem.i16(t + at)), ee::mul(k, s));
    let r = fix(fx, f, ra);
    fx.mem.set16(t + at, r);
}

fn sway(fx: &mut Fx, t: u32, k: f32, ras: [u32; 3]) {
    wobble(fx, t, 8, 0x3A, 0, k, ras[0]);
    wobble(fx, t, 0xA, 0x30, 1, k, ras[1]);
    wobble(fx, t, 0xC, 0x3A, 1, k, ras[2]);
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

fn fade_in(fx: &mut Fx, t: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a < cap {
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(up));
    }
}

/// The cell +0x34 cycling 0..6.
fn cycle(fx: &mut Fx, t: u32) {
    let c = fx.mem.i16(t + 0x34);
    fx.mem.set16(t + 0x34, if c < 6 { c + 1 } else { 0 });
}

/// Three random angles +0x10 / +0x14 / +0x18 (rand % 0xFFFF).
fn spin3(fx: &mut Fx, t: u32, s0: u32, ra: u32) {
    for k in 0..3u32 {
        let r = rmod(fx, s0, ra + 0x1C * k, 0xFFFF);
        fx.mem.set32(t + 0x10 + 4 * k, r);
    }
}

/// 0x1E00470 (D, task): a grey mote (pool B).
fn mote(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x3F, 0x3F, 0x3F);
    if fx.mem.i16(t + 0x36) == 1 {
        let r = rmod(fx, t, 0x01E0_04AC, 0xF);
        fx.mem.set16(t + 0x38, r + 0xA);
        let r = rmod(fx, t, 0x01E0_04CC, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.write_u8(t + 0x2B, 0);
        let r = rmod(fx, t, 0x01E0_04EC, 0xF);
        fx.mem.set16(t + 0x34, r);
        let r = rmod(fx, t, 0x01E0_0508, 0x168);
        fx.mem.set16(t + 0x30, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0xA {
        fade_in(fx, t, 0x60, 9);
    } else {
        fade_out(fx, t, 7, 5);
    }
    age(fx, t);
}

/// 0x1E006E0 (D, task): a blue puff (pool A).
fn puff(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 5);
        let v = fx.rand_from(t, 0x01E0_0710);
        let mut m = v & 3;
        if v < 0 && m != 0 {
            m -= 4;
        }
        fx.mem.set16(t + 0x34, m);
        fx.rand_from(t, 0x01E0_072C);
        l2::rgb(fx, t + 0x28, 0x1F, 0x1F, 0xFF);
        let r = rmod(fx, t, 0x01E0_0748, 0x168);
        fx.mem.set16(t + 0x30, r);
        let r = rmod(fx, t, 0x01E0_0764, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0xA {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fade_out(fx, t, 0x11, 0xA);
    }
    age(fx, t);
    sway(fx, t, 9.0, [0x01E0_0800, 0x01E0_0838, 0x01E0_0870]);
}

/// 0x1E00890 (D, task): a dark puff (both pools).
fn puff_b(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 4);
        fx.mem.set16(t + 0x34, 2);
        fx.rand_from(t, 0x01E0_08C8);
        l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x20);
        let r = rmod(fx, t, 0x01E0_08E4, 0x168);
        fx.mem.set16(t + 0x30, r);
        let r = rmod(fx, t, 0x01E0_0900, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 4 {
        fx.mem.write_u8(t + 0x2B, 0x60);
    } else {
        fade_out(fx, t, 0xB, 9);
    }
    age(fx, t);
    sway(fx, t, 24.0, [0x01E0_099C, 0x01E0_09D4, 0x01E0_0A0C]);
}

const D03: u64 = 0x3FD3_3333_3333_3333;
const D10: u64 = 0x3FF0_0000_0000_0000;
const D01: u64 = 0x3FB9_9999_9999_999A;

/// 0x1E00A30 (D, task): a sphere (pool A, index +0x30 = i): its colour
/// pulses on ages 11..23; from update 335 its row point flies to the target
/// (velocity D+0x1CB0 + 0x10 i from update 335, from update 335 + 5 i on)
/// while its scale grows by 0.02 under 0.3; close to the target state 100
/// (red, scale + 0.1, D+0x3B68 i = 2, sphere 0 spawning the camera step);
/// it sways (D+0x1290) about the row point into D+0x1C50 + 0x10 i.
fn sphere(fx: &mut Fx, t: u32) {
    let sp = fx.sp;
    let i = idx(fx, t);
    if fx.mem.i16(t + 0x36) == 1 {
        l2::rgb(fx, t + 0x28, 0x10, 0x10, 0x30);
        for o in [0x10u32, 0x14, 0x18] {
            fx.mem.set32(t + o, 0);
        }
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x38, 0);
        let r = rmod(fx, D, 0x01E0_0A88, 3);
        fx.mem.set16(t + 0x36, r + 2);
        fx.mem.write_u8(t + 0x2B, 0);
        for c in 0..3 {
            fx.mem.write_u32(SCALE + 0x10 * i + 4 * c, 0x3E4C_CCCD);
        }
    }
    let a = fx.mem.i16(t + 0x32);
    if a < 0xA {
        fade_in(fx, t, 0x80, 0xC);
    } else if fx.mem.i16(t + 0x36) != 0x64 {
        const PULSE: [(i32, (i32, i32, i32)); 7] = [
            (0xB, (0x10, 0x10, 0x30)),
            (0xD, (0x18, 0x18, 0x38)),
            (0xF, (0x20, 0x20, 0x40)),
            (0x11, (0x28, 0x28, 0x48)),
            (0x13, (0x28, 0x28, 0x48)),
            (0x15, (0x20, 0x20, 0x40)),
            (0x17, (0x18, 0x18, 0x38)),
        ];
        for (n, c) in PULSE {
            if fx.mem.i16(t + 0x32) == n {
                l2::rgb(fx, t + 0x28, c.0, c.1, c.2);
            }
        }
        if fx.mem.i16(t + 0x32) >= 0x18 {
            fx.mem.set16(t + 0x32, 0xB);
        }
    }
    age(fx, t);
    let h = fx.target_handle();
    crate::b076::unit_position_saving(fx, h, sp + 0x50, u64::from(D), 0x01E0_0C40);
    if fx.mem.i16(COUNTER) == 0x14F {
        let i = idx(fx, t);
        let r = rmod(fx, i << 4, 0x01E0_0C5C, 0xE);
        let x = ee::sub(ee::add(ee::sub(fx.mem.f(sp + 0x50), fx.mem.f(ROW + 0x10 * i)), ee::from_i32(r)), 7.0);
        fx.mem.setf(VEL + 0x10 * i, x);
        let i = idx(fx, t);
        let r = rmod(fx, i << 4, 0x01E0_0CA4, 0x1E);
        let y = ee::sub(ee::add(ee::sub(fx.mem.f(sp + 0x54), fx.mem.f(ROW + 0x10 * i + 4)), ee::from_i32(r)), 15.0);
        fx.mem.setf(VEL + 0x10 * i + 4, y);
        let z = ee::sub(fx.mem.f(sp + 0x58), fx.mem.f(ROW + 0x10 * i + 8));
        fx.mem.setf(VEL + 0x10 * i + 8, z);
        for c in 0..3 {
            let v = ee::div(fx.mem.f(VEL + 0x10 * i + 4 * c), 18.0);
            fx.mem.setf(VEL + 0x10 * i + 4 * c, v);
        }
    }
    let i = idx(fx, t);
    if (5 * i as i32 + 0x14F) < fx.mem.i16(COUNTER) {
        if fx.mem.i16(t + 0x36) != 0x64 {
            for c in 0..3 {
                let v = ee::add(fx.mem.f(ROW + 0x10 * i + 4 * c), fx.mem.f(VEL + 0x10 * i + 4 * c));
                fx.mem.setf(ROW + 0x10 * i + 4 * c, v);
            }
            for (c, ra1, ra2) in [(0u32, 0x01E0_0DE4u32, 0x01E0_0E04u32), (1, 0x01E0_0E40, 0x01E0_0E60), (2, 0x01E0_0E9C, 0x01E0_0EBC)] {
                let p = SCALE + 0x10 * i + 4 * c;
                let v = fx.mem.f(p);
                if dcmp(fx, v, D03, p, ra1, ra2, false) {
                    let r = ee::add(fx.mem.f(p), f32::from_bits(0x3CA3_D70A));
                    fx.mem.setf(p, r);
                }
            }
            fx.mem.set16(STATE + 2 * i, 1);
        }
        for (o, ra, ra_b) in [(0x14u32, 0x01E0_0F10u32, 0x01E0_0F64u32), (0x10, 0x01E0_0F84, 0x01E0_0FD8)] {
            let v = ee::add(ee::from_i32(fx.mem.i32(t + o)), 8.0);
            let r = fix(fx, v, ra);
            fx.mem.set32(t + o, r);
            if !ee_lt(ee::from_i32(fx.mem.i32(t + o)), 360.0) {
                let h = fx.mem.i16(t + o);
                let m = fx.div(h, 0x168).1 as i16 as i32;
                let r = fix(fx, ee::from_i32(m), ra_b);
                fx.mem.set32(t + o, r);
            }
        }
    }
    let i = idx(fx, t);
    let dx = ee::sub(fx.mem.f(sp + 0x50), fx.mem.f(ROW + 0x10 * i));
    let dz = ee::sub(fx.mem.f(sp + 0x58), fx.mem.f(ROW + 0x10 * i + 8));
    let lx = fx.sqrtf(ee::mul(dx, dx));
    let i = idx(fx, t);
    if fx.mem.i16(STATE + 2 * i) == 2 {
        fx.mem.set16(STATE + 2 * i, 3);
    }
    if ee_lt(lx, 11.0) {
        let lz = fx.sqrtf(ee::mul(dz, dz));
        if ee_lt(lz, 3.0) && fx.mem.i16(t + 0x36) != 0x64 {
            fx.mem.set16(t + 0x36, 0x64);
            l2::rgb(fx, t + 0x28, 0x80, 0x20, 0x20);
            let i = idx(fx, t);
            for c in 0..3 {
                let p = SCALE + 0x10 * i + 4 * c;
                let v = ee::add(fx.mem.f(p), f32::from_bits(0x3DCC_CCCD));
                fx.mem.setf(p, v);
            }
            fx.mem.set16(STATE + 2 * i, 2);
            if fx.mem.i16(t + 0x30) == 0 {
                spawn(fx, POOL_B, 0x32, CAM_B);
            }
        }
    }
    let i = idx(fx, t);
    let a = (60 * i as i32) as i16 as i32;
    let c = l2::sin_deg(fx, TAB, a, 1);
    let up = fx.mem.i16(t + 0x38) < 0xB5;
    let x = if up { ee::add(fx.mem.f(SWAY + 0x10 * i), ee::mul(0.5, c)) } else { ee::sub(fx.mem.f(SWAY + 0x10 * i), ee::mul(0.5, c)) };
    fx.mem.setf(SWAY + 0x10 * i, x);
    let s = l2::sin_deg(fx, TAB, a, 0);
    let y = if up { ee::add(fx.mem.f(SWAY + 0x10 * i + 4), ee::mul(0.5, s)) } else { ee::sub(fx.mem.f(SWAY + 0x10 * i + 4), ee::mul(0.5, s)) };
    fx.mem.setf(SWAY + 0x10 * i + 4, y);
    for c in 0..2 {
        let v = ee::add(fx.mem.f(ROW + 0x10 * i + 4 * c), fx.mem.f(SWAY + 0x10 * i + 4 * c));
        fx.mem.setf(CENTRE + 0x10 * i + 4 * c, v);
    }
    let z = fx.mem.read_u32(ROW + 0x10 * i + 8);
    fx.mem.write_u32(CENTRE + 0x10 * i + 8, z);
    let s = fx.mem.i16(t + 0x38) + 0xA;
    fx.mem.set16(t + 0x38, s);
    let s = fx.mem.i16(t + 0x38);
    if s >= 0x168 {
        let m = fx.div(s, 0x168).1;
        fx.mem.set16(t + 0x38, m);
    }
    if fx.mem.i16(t + 0x36) != 0x64 {
        let i = idx(fx, t);
        let p = D + 0x10 * i + 0x14;
        let v = fx.mem.i32(p).wrapping_add(0x400);
        fx.mem.set32(p, v);
    }
    if fx.mem.i32(TIMER) < 0x14 {
        fade_out(fx, t, 0xB, 9);
    }
}

/// 0x1E013A0 (D, task): a spark (pool B): cell rand % 5 cycling, size 13.
fn spark(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x10, 0x10, 0x10);
    if fx.mem.i16(t + 0x36) == 1 {
        let r = rmod(fx, t, 0x01E0_13E4, 5);
        fx.mem.set16(t + 0x34, r);
        fx.mem.set16(t + 0x38, 0xD);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0);
    }
    if fx.mem.i16(t + 0x32) < 2 {
        fade_in(fx, t, 0x60, 0x30);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        cycle(fx, t);
    }
    age(fx, t);
    if fx.mem.i32(TIMER) < 0x19 {
        fade_out(fx, t, 7, 5);
    }
}

/// The task's i16 point = the centre D+0x1C50 + 0x10 i (+ `dy` in y).
fn to_centre(fx: &mut Fx, t: u32, dy: Option<f32>, ras: [u32; 3]) {
    for c in 0..3u32 {
        let i = idx(fx, t);
        let mut v = fx.mem.f(CENTRE + 0x10 * i + 4 * c);
        if c == 1 {
            if let Some(d) = dy {
                v = ee::add(d, v);
            }
        }
        let r = fix(fx, v, ras[c as usize]);
        fx.mem.set16(t + 2 * c, r);
    }
}

/// 0x1E014C0 (D, task): a ring sprite at the sphere's centre (pool A),
/// growing to 12 while D+0x3B68 i = 1, 14 at 2.
fn ring(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x40, 0x20, 0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        spin3(fx, t, t, 0x01E0_1504);
        let r = rmod(fx, t, 0x01E0_1558, 5);
        fx.mem.set16(t + 0x34, r);
        fx.mem.set16(t + 0x38, 7);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0);
    }
    if fx.mem.i16(t + 0x32) < 0xA {
        fade_in(fx, t, 0x60, 9);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        cycle(fx, t);
    }
    age(fx, t);
    to_centre(fx, t, None, [0x01E0_15F8, 0x01E0_1610, 0x01E0_1628]);
    let i = idx(fx, t);
    if fx.mem.i16(STATE + 2 * i) == 1 {
        let s = fx.mem.i16(t + 0x38);
        if s < 0xC {
            fx.mem.set16(t + 0x38, s + 1);
        }
    }
    let i = idx(fx, t);
    if fx.mem.i16(STATE + 2 * i) == 2 {
        fx.mem.set16(t + 0x38, 0xE);
    }
    if fx.mem.i32(TIMER) < 0x14 {
        fade_out(fx, t, 0xB, 9);
    }
}

/// 0x1E016D0 (D, task): an orb at the sphere's centre (pool A); on the
/// burst (D+0x3B68 i = 2) dark, size 13, three sparks and four drifts /
/// pushed drifts about it into pool B.
fn orb(fx: &mut Fx, t: u32) {
    let sp = fx.sp;
    l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
    if fx.mem.i16(t + 0x36) == 1 {
        spin3(fx, t, D, 0x01E0_171C);
        let r = rmod(fx, D, 0x01E0_1770, 5);
        fx.mem.set16(t + 0x34, r);
        fx.mem.set16(t + 0x38, 3);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0);
    }
    if fx.mem.i16(t + 0x32) < 0xA {
        fade_in(fx, t, 0x80, 0xC);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        cycle(fx, t);
    }
    age(fx, t);
    to_centre(fx, t, Some(1.0), [0x01E0_1810, 0x01E0_1834, 0x01E0_184C]);
    let i = idx(fx, t);
    if fx.mem.i16(STATE + 2 * i) == 1 {
        let s = fx.mem.i16(t + 0x38);
        let g = fx.mem.i16(t + 0x32);
        if s < 6 && fx.div(g, 3).1 == 1 {
            fx.mem.set16(t + 0x38, s + 1);
        }
    }
    let i = idx(fx, t);
    if fx.mem.i16(STATE + 2 * i) == 2 {
        l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x20);
        fx.mem.set16(t + 0x38, 0xD);
        let jitter = |fx: &mut Fx, s0: u32, ra: u32, m: i32, h: i32, c: u32| {
            let r = rmod(fx, s0, ra, m);
            let v = fx.mem.i16(t + 2 * c) + r - h;
            fx.mem.setf(POS + 4 * c, ee::from_i32(v));
        };
        for s0 in 0..3u32 {
            jitter(fx, s0, 0x01E0_18EC, 0x14, 0xA, 0);
            jitter(fx, s0, 0x01E0_1920, 0x14, 0xA, 1);
            let z = fx.mem.i16(t + 4);
            fx.mem.setf(POS + 8, ee::from_i32(z));
            fx.mem.set16(SEL, s0 as i32);
            spawn(fx, POOL_B, 0x32, SPARK);
        }
        for s0 in 0..4u32 {
            jitter(fx, s0, 0x01E0_19A4, 0x1E, 0xF, 0);
            jitter(fx, s0, 0x01E0_19D8, 0x1E, 0xF, 1);
            let z = fx.mem.i16(t + 4);
            fx.mem.setf(POS + 8, ee::from_i32(z));
            spawn(fx, POOL_B, 0x32, DRIFT);
            let y = yaw(fx, sp + 0x50, 0x01E0_1A64);
            let c = l2::sin_deg(fx, TAB, y - 0xB4, 1);
            let s1 = fix(fx, ee::mul(8.0, ee::neg(c)), 0x01E0_1A90) as i16 as i32;
            jitter(fx, s0, 0x01E0_1A9C, 0x14, 0xA, 0);
            jitter(fx, s0, 0x01E0_1AD0, 0x14, 0xA, 1);
            let z = fx.mem.i16(t + 4) + s1;
            fx.mem.setf(POS + 8, ee::from_i32(z));
            spawn(fx, POOL_B, 0x32, DRIFT_C);
        }
    }
    if fx.mem.i32(TIMER) < 0x19 {
        fade_out(fx, t, 9, 7);
    }
}

/// 0x1E01BA0 (D, task): a wisp at node 0x38 (both pools) pushed 2 along
/// the attacker's facing.
fn wisp(fx: &mut Fx, t: u32) {
    let sp = fx.sp;
    let h = fx.self_handle();
    let n = fx.mem.i16(NODE_B);
    fx.node_position(h, n, sp + 0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x2C, 0x80, 0x80, 0x80);
        spin3(fx, t, t, 0x01E0_1C0C);
        let r = rmod(fx, t, 0x01E0_1C60, 0xA);
        fx.mem.set16(t + 0x38, r + 0xE);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if fx.mem.i16(t + 0x32) < 0xA {
        fade_in(fx, t, 0xA0, 0x10);
    } else {
        fade_out(fx, t, 0x15, 0x13);
    }
    age(fx, t);
    let y = yaw(fx, sp + 0x40, 0x01E0_1D1C);
    let c = l2::sin_deg(fx, TAB, y - 0xB4, 1);
    let s1 = fix(fx, ee::mul(2.0, ee::neg(c)), 0x01E0_1D48) as i16 as i32;
    let v = fx.mem.f(sp + 0x30);
    let x = fix(fx, v, 0x01E0_1D58);
    fx.mem.set16(t, x);
    let v = ee::sub(fx.mem.f(sp + 0x38), ee::from_i32(s1));
    let z = fix(fx, v, 0x01E0_1D70);
    fx.mem.set16(t + 4, z);
}

/// 0x1E01D90 / 0x1E024F0 (`d` the offset) (D, task): a grey drift (pool B).
fn drift(fx: &mut Fx, t: u32, d: u32) {
    let c = d != 0;
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, if c { 0xA } else { 0xB });
        fx.mem.set16(t + 0x34, 2);
        fx.rand_from(t, 0x01E0_1DC8 + d);
        l2::rgb(fx, t + 0x28, 0x40, 0x40, 0x40);
        let r = rmod(fx, t, 0x01E0_1DE4 + d, 0x168);
        fx.mem.set16(t + 0x30, r);
        let r = rmod(fx, t, 0x01E0_1E00 + d, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 4 {
        fx.mem.write_u8(t + 0x2B, 0x60);
    } else if c {
        fade_out(fx, t, 7, 5);
    } else {
        fade_out(fx, t, 0xB, 9);
    }
    cycle(fx, t);
    age(fx, t);
    sway(fx, t, 10.0, [0x01E0_1EBC + d, 0x01E0_1EF4 + d, 0x01E0_1F2C + d]);
}

/// 0x1E01F50 (D, task): a mote rising from a sphere's centre (pool B).
fn rise(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x10);
        fx.mem.set16(t + 0x34, 2);
        fx.rand_from(t, 0x01E0_1F88);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        to_centre(fx, t, None, [0x01E0_1FB0, 0x01E0_1FC8, 0x01E0_1FE0]);
        let r = rmod(fx, t, 0x01E0_1FE8, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0xA, -0x32);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 5 {
        let a = fx.mem.read_u8(t + 0x2B);
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x13));
    } else {
        fade_out(fx, t, 0xB, 9);
    }
    let y = fx.mem.i16(t + 0xA) + 2;
    fx.mem.set16(t + 0xA, y);
    cycle(fx, t);
    let i = idx(fx, t);
    let v = fx.mem.f(CENTRE + 0x10 * i);
    let x = fix(fx, v, 0x01E0_2090);
    fx.mem.set16(t, x);
    let i = idx(fx, t);
    let v = fx.mem.f(CENTRE + 0x10 * i + 8);
    let z = fix(fx, v, 0x01E0_20A8);
    fx.mem.set16(t + 4, z);
    age(fx, t);
    let a = fx.mem.i16(t + 0x3A) + 2;
    fx.mem.set16(t + 0x3A, a);
}

/// The camera slots' first quadword +1 (under 32) per component.
fn cam_grow(fx: &mut Fx) {
    for c in 0..4 {
        let v = fx.mem.f(CAM + 4 * c);
        if ee_lt(v, 32.0) {
            fx.mem.setf(CAM + 4 * c, ee::add(v, 1.0));
        }
    }
}

/// 0x1E020E0 (D, task): camera slots 4 / 5 on node 0x38 (D+0x2610..)
/// until update 341.
fn cam_a(fx: &mut Fx, t: u32) {
    let sp = fx.sp;
    let h = fx.self_handle();
    let n = fx.mem.i16(NODE_B);
    fx.node_position(h, n, sp + 0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x36, 2);
    }
    copy3(fx, CAM + 0x10, sp + 0x30);
    cam_grow(fx);
    fx.mem.set32(CAM + 0x20, 0);
    fx.mem.write_u32(CAM + 0x24, 0x41A0_0000);
    if fx.mem.i16(t + 0x36) == 2 {
        let v = fx.mem.f(CAM + 0x28);
        if dcmp(fx, v, D10, t, 0x01E0_2230, 0x01E0_2240, false) {
            let r = ee::add(fx.mem.f(CAM + 0x28), f32::from_bits(0x3D4C_CCCD));
            fx.mem.setf(CAM + 0x28, r);
        } else {
            fx.mem.set16(t + 0x36, 3);
        }
    }
    if fx.mem.i16(COUNTER) >= 0x155 {
        crate::blib::exe::camera_slots45(fx, 0);
        fx.mem.set16(t + 0x36, 0);
    } else {
        crate::blib::exe::camera_slots45(fx, CAM);
    }
}

/// 0x1E022C0 (D, task): camera slots 4 / 5 on the target, pulled out
/// until update 380, then back; until update 391.
fn cam_b(fx: &mut Fx, t: u32) {
    let sp = fx.sp;
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        for o in [0u32, 4, 8, 0xC, 0x28] {
            fx.mem.set32(CAM + o, 0);
        }
        fx.mem.set16(t + 0x36, 2);
    }
    copy3(fx, CAM + 0x10, sp + 0x30);
    if fx.mem.i16(COUNTER) < 0x17C {
        let v = fx.mem.f(CAM);
        if ee_lt(v, 128.0) {
            fx.mem.setf(CAM, ee::add(v, 25.0));
        }
        let v = fx.mem.f(CAM + 4);
        if ee_lt(v, 32.0) {
            fx.mem.setf(CAM + 4, ee::add(v, 6.0));
        }
        fx.mem.set32(CAM + 0x20, 0);
        fx.mem.write_u32(CAM + 0x24, 0x41F0_0000);
        let v = fx.mem.f(CAM + 0x28);
        if dcmp(fx, v, D10, t, 0x01E0_23B8, 0x01E0_23C8, false) {
            let r = ee::add(fx.mem.f(CAM + 0x28), f32::from_bits(0x3E4C_CCCD));
            fx.mem.setf(CAM + 0x28, r);
        }
    } else {
        let v = fx.mem.f(CAM);
        if !ee_le(v, 20.0) {
            fx.mem.setf(CAM, ee::sub(v, 12.0));
        }
        let v = fx.mem.f(CAM + 4);
        if !ee_le(v, 9.0) {
            fx.mem.setf(CAM + 4, ee::sub(v, 3.0));
        }
        let v = fx.mem.f(CAM + 0x28);
        if dcmp(fx, v, D01, t, 0x01E0_2460, 0x01E0_2484, true) {
            let r = ee::sub(fx.mem.f(CAM + 0x28), f32::from_bits(0x3DCC_CCCD));
            fx.mem.setf(CAM + 0x28, r);
        }
    }
    if fx.mem.i16(COUNTER) >= 0x187 {
        crate::blib::exe::camera_slots45(fx, 0);
        fx.mem.set16(t + 0x36, 0);
    } else {
        crate::blib::exe::camera_slots45(fx, CAM);
    }
}

/// 0x1E026B0 (D, task): the stage light (0x1F7F10) down by 10 to 0x30,
/// back up by 10 to 0x80 once the timer is under 31.
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x30, 0x80);
        fx.mem.set16(t + 0x36, 2);
    }
    let l = fx.mem.i16(t + 0x30);
    if fx.mem.i32(TIMER) >= 0x1F {
        fx.mem.set16(t + 0x30, if l < 0x31 { 0x30 } else { l - 0xA });
    } else {
        fx.mem.set16(t + 0x30, if l < 0x80 { l + 0xA } else { 0x80 });
    }
    let l = fx.mem.read_u8(t + 0x30);
    fx.set_stage_light(i32::from(l));
}

// ---------------------------------------------------------------- draws

/// Mode 0 of the sprites: rect `id` (cell +0x34) into `r`, the strip3
/// header, colour +0x28, UVs, texture word `tex`.
fn setup(fx: &mut Fx, t: u32, d: u32, r: u32, id: i32, tex: u32) {
    let c = fx.mem.i16(t + 0x34);
    l2::rect(fx, r, id, c);
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x28, 0);
    l2::uv_strip3(fx, d, r, 0);
    let w = fx.mem.read_u32(tex);
    fx.desc_texture(d, w, 1);
}

/// The trans vector at `out`: the task's i16 position + i16 +8 / +0xA /
/// +0xC over 10 (integer division by the magic multiply).
fn trans_tenth(fx: &mut Fx, t: u32, out: u32) {
    for k in 0..3u32 {
        let off = fx.mem.i16(t + 8 + 2 * k);
        fx.mult(0x6666_6667, off);
        let hi = fx.hilo().1;
        let q = (hi >> 2).wrapping_add(((off as u32) >> 31) as i32);
        let v = fx.mem.i16(t + 2 * k).wrapping_add(q);
        fx.mem.setf(out + 4 * k, ee::from_i32(v));
    }
}

/// A camera-facing quad of half size (`w`, `h`) through the camera angles
/// at `sp` + 0x100 and the trans at `sp` + 0xA0.
fn facing(fx: &mut Fx, sp: u32, d: u32, w: f32, h: f32) {
    l2::xform_vp(fx, sp + 0xC0, 0, sp + 0x100, sp + 0xA0);
    l2::quad_xy(fx, sp + 0x60, w, h);
    l2::project_submit(fx, sp + 0x60, d, sp + 0xC0, OT);
}

/// 0x1E035E0(D, task, desc, mode): a camera-facing quad of half size i16
/// +0x38 at the task's position (+ offsets / 10), sprite 0x14, texture
/// D+0x78 (b064's sprite_a).
fn sprite_a(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x110);
    if mode == 0 {
        setup(fx, t, d, sp + 0xB0, 0x14, TEX + 8);
    } else {
        l2::camera_angles_to(fx, sp + 0x100);
        trans_tenth(fx, t, sp + 0xA0);
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        facing(fx, sp, d, s, s);
    }
    fx.leave(0x110);
}

/// 0x1E045A0(D, task, desc, mode): as sprite_a with texture D+0x70; for
/// step 0x1E024F0 additive (ALPHA 0x80_00000042).
fn sprite_d(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x110);
    if mode == 0 {
        setup(fx, t, d, sp + 0xB0, 0x14, TEX);
    } else {
        if fx.mem.read_u32(t + 0x24) == DRIFT_C {
            fx.desc_alpha(d, 0x80_0000_0042);
        }
        l2::camera_angles_to(fx, sp + 0x100);
        trans_tenth(fx, t, sp + 0xA0);
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        facing(fx, sp, d, s, s);
    }
    fx.leave(0x110);
}

/// 0x1E04770(D, task, desc, mode): a camera-facing quad 3 wide, i16 +0x38
/// high (lower corners at its y) at the task's position + (0, +0xA / 10,
/// 0), sprite 0xA, texture D+0x74.
fn sprite_r(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x110);
    if mode == 0 {
        setup(fx, t, d, sp + 0xB0, 0xA, TEX + 4);
    } else {
        l2::camera_angles_to(fx, sp + 0x100);
        for c in 0..3u32 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * c));
            fx.mem.setf(sp + 0xA0 + 4 * c, v);
        }
        let y = ee::add(fx.mem.f(sp + 0xA4), ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0));
        fx.mem.setf(sp + 0xA4, y);
        l2::xform_vp(fx, sp + 0xC0, 0, sp + 0x100, sp + 0xA0);
        let h = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x60, 3.0, h);
        fx.mem.set32(sp + 0x84, 0);
        fx.mem.set32(sp + 0x94, 0);
        l2::project_submit(fx, sp + 0x60, d, sp + 0xC0, OT);
    }
    fx.leave(0x110);
}

/// 0x1E03780(D, task, desc, mode): a camera-facing quad of half size i16
/// +0x38 scaled 0.1 at the task's position + offsets / 50, sprite 0x9B,
/// texture D+0xAC.
fn sprite_p(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x120);
    if mode == 0 {
        setup(fx, t, d, sp + 0xC0, 0x9B, TEX + 0x3C);
    } else {
        l2::camera_angles_to(fx, sp + 0x110);
        for c in 0..3u32 {
            let off = ee::div(ee::from_i32(fx.mem.i16(t + 8 + 2 * c)), 50.0);
            let v = ee::add(ee::from_i32(fx.mem.i16(t + 2 * c)), off);
            fx.mem.setf(sp + 0xA0 + 4 * c, v);
        }
        for c in 0..3u32 {
            fx.mem.write_u32(sp + 0xB0 + 4 * c, 0x3DCC_CCCD);
        }
        l2::xform_vp(fx, sp + 0xD0, sp + 0xB0, sp + 0x110, sp + 0xA0);
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x60, s, s);
        l2::project_submit(fx, sp + 0x60, d, sp + 0xD0, OT);
    }
    fx.leave(0x120);
}

/// 0x1E040E0(D, task, desc, mode): sprite 0xA (texture D+0x74) and over it
/// sprite 0x14 at half alpha (texture D+0x70), camera-facing, half size
/// i16 +0x38.
fn glow(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x120);
    let k = fx.mem.read_u32(0x01E0_4C80);
    fx.mem.write_u32(sp + 0x11C, k);
    if mode == 0 {
        setup(fx, t, d, sp + 0xB0, 0xA, TEX + 4);
    } else {
        l2::camera_angles_to(fx, sp + 0x100);
        trans_tenth(fx, t, sp + 0xA0);
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        facing(fx, sp, d, s, s);
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0xB0, 0x14, c);
        l2::uv_strip3(fx, d, sp + 0xB0, 0);
        copy_bytes4(fx, sp + 0x11C, t + 0x28);
        let a = fx.mem.read_u8(sp + 0x11F) >> 1;
        fx.mem.write_u8(sp + 0x11F, a);
        l2::colour_strip3(fx, d, sp + 0x11C, 0);
        let w = fx.mem.read_u32(TEX);
        fx.desc_texture(d, w, 1);
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x60, s, s);
        l2::project_submit(fx, sp + 0x60, d, sp + 0xC0, OT);
    }
    fx.leave(0x120);
}

fn copy_bytes4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let b = fx.mem.read_u8(from + k);
        fx.mem.write_u8(to + k, b);
    }
}

/// 0x1E032F0(D, task, desc, mode): a flat quad 10 high between the
/// directions +0x3A and +0x3A + 20 at radius i16 +0x38 about the task's
/// position (+ offsets / 10), sprite 0x1E, texture D+0x7C.
fn plate(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x110);
    if mode == 0 {
        setup(fx, t, d, sp + 0xB0, 0x1E, TEX + 0xC);
    } else {
        l2::camera_angles_to(fx, sp + 0x100);
        trans_tenth(fx, t, sp + 0xA0);
        l2::xform_vp(fx, sp + 0xC0, 0, 0, sp + 0xA0);
        let a0 = fx.mem.i16(t + 0x3A);
        let a1 = (a0 + 0x14) as i16 as i32;
        for (a, outs) in [(a0, [0x80u32, 0x88, 0x60, 0x68]), (a1, [0x90, 0x98, 0x70, 0x78])] {
            for (k, o) in outs.into_iter().enumerate() {
                let f = l2::sin_deg(fx, TAB, a, if k % 2 == 0 { 1 } else { 0 });
                let s = ee::from_i32(fx.mem.i16(t + 0x38));
                fx.mem.setf(sp + o, ee::mul(s, f));
            }
        }
        fx.mem.set32(sp + 0x64, 0);
        fx.mem.set32(sp + 0x74, 0);
        fx.mem.write_u32(sp + 0x84, 0x4120_0000);
        fx.mem.write_u32(sp + 0x94, 0x4120_0000);
        l2::project_submit(fx, sp + 0x60, d, sp + 0xC0, OT);
    }
    fx.leave(0x110);
}

/// 0x1E04330(D, task, desc, mode): a flat fan quad from the task's point
/// to radius i16 +0x38 between +0x3A and +0x3A + 20, turned by +0x10,
/// gouraud (+0x28 / +0x2C), sprite 0x9C, texture D+0xAC; additive from age
/// 17; keyed by its mean w on D+0x26EC.
fn fan(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x110);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0x100, 0x9C, c);
        l2::desc_strip3(fx, d);
        l2::colour_strip3(fx, d, t + 0x28, 1);
        l2::colour_strip3(fx, d, t + 0x2C, 2);
        l2::uv_strip3(fx, d, sp + 0x100, 0);
        let w = fx.mem.read_u32(TEX + 0x3C);
        fx.desc_texture(d, w, 1);
    } else {
        if fx.mem.i16(t + 0x32) >= 0x11 {
            fx.desc_alpha(d, 0x80_0000_0042);
        }
        for c in 0..3u32 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * c));
            fx.mem.setf(sp + 0xB0 + 4 * c, v);
        }
        l2::xform_vp(fx, sp + 0xC0, 0, t + 0x10, sp + 0xB0);
        for o in [0x74u32, 0x84, 0x94, 0xA4] {
            fx.mem.set32(sp + o, 0);
        }
        let a = fx.mem.i16(t + 0x3A) as i16 as i32;
        for (k, o) in [0x90u32, 0x98].into_iter().enumerate() {
            let f = l2::sin_deg(fx, TAB, a, 1 - k as i32);
            let s = ee::from_i32(fx.mem.i16(t + 0x38));
            fx.mem.setf(sp + o, ee::mul(s, f));
        }
        fx.mem.set32(sp + 0x70, 0);
        fx.mem.set32(sp + 0x78, 0);
        let a = (fx.mem.i16(t + 0x3A) + 0x14) as i16 as i32;
        for (k, o) in [0xA0u32, 0xA8].into_iter().enumerate() {
            let f = l2::sin_deg(fx, TAB, a, 1 - k as i32);
            let s = ee::from_i32(fx.mem.i16(t + 0x38));
            fx.mem.setf(sp + o, ee::mul(s, f));
        }
        fx.mem.set32(sp + 0x80, 0);
        fx.mem.set32(sp + 0x88, 0);
        if l2::project(fx, sp + 0x70, SPR, sp + 0xC0, d, MEAN) {
            let v = fx.mem.f(MEAN);
    let key = fixu(fx, v, 0x01E0_4550);
            let ot = fx.mem.read_u32(OTW);
            fx.submit(d, ot, key);
        }
    }
    fx.leave(0x110);
}

/// 0x1E03930(D, task, desc, mode): the sphere mesh: the 12 x 12 grid
/// D+0x12F0 turned by +0x10 (0x1099E0 axes 0 / 1), scaled D+0x1BF0 +
/// 0x10 i and turned by D+0x10 + 0x10 i (y spinning 0x400 a update), offset per vertex by the i16 arrays
/// D+0x2728 / D+0x2DE8 / D+0x34A8 (+ 0x120 i; at state 100 each falls by
/// the grid's third every update), at D+0x1C50 + 0x10 i; sprite 0x9C,
/// texture D+0xAC; keyed by the mean w on D+0x26EC.
fn mesh(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x150);
    fx.mem.set16(sp + 0xB0, 0);
    if mode == 0 {
        setup(fx, t, d, sp + 0x140, 0x9C, TEX + 0x3C);
    } else {
        let i = idx(fx, t);
        l2::xform_vp(fx, sp + 0x100, SCALE + 0x10 * i, D + 0x10 + 0x10 * i, CENTRE + 0x10 * i);
        fx.mem.set32(sp + 0xA0, 0);
        for r in 0..11u32 {
            let s3 = D + 0xC0 * r;
            let s4 = D + 0x18 * r;
            for c in 0..12u32 {
                let s1 = s3 + 0x10 * c;
                let s2 = 2 * c;
                let next = if c < 11 { s1 + 0x10 } else { s3 };
                copy4(fx, sp + 0xC0, s1 + 0x12F0);
                copy4(fx, sp + 0xD0, next + 0x12F0);
                copy4(fx, sp + 0xE0, s1 + 0x13B0);
                copy4(fx, sp + 0xF0, next + 0x13B0);
                l2::rotate4(fx, sp + 0xC0, t + 0x10, 0, TAB);
                l2::rotate4(fx, sp + 0xC0, t + 0x10, 1, TAB);
                let i = idx(fx, t);
                let o = s4 + 0x120 * i + s2;
                for (axis, arr) in [(0, 0x2728u32), (1, 0x2DE8), (2, 0x34A8)] {
                    let v = ee::from_i32(fx.mem.i16(o + arr));
                    l2::add_column(fx, sp + 0xC0, axis, v);
                }
                if fx.mem.i16(t + 0x36) >= 0x64 {
                    for (k, arr, ra) in [(0u32, 0x2728u32, 0x01E0_3C34u32), (1, 0x2DE8, 0x01E0_3C6C), (2, 0x34A8, 0x01E0_3CA4)] {
                        let i = idx(fx, t);
                        let p = s4 + 0x120 * i + s2 + arr;
                        let v = ee::sub(ee::from_i32(fx.mem.i16(p)), fx.mem.f(s1 + 0x1D10 + 4 * k));
                        let r = fix(fx, v, ra);
                        fx.mem.set16(p, r);
                    }
                }
                if l2::project(fx, sp + 0xC0, SPR, sp + 0x100, d, MEAN) {
                    let v = fx.mem.f(MEAN);
    let key = fixu(fx, v, 0x01E0_3CD0);
                    let ot = fx.mem.read_u32(OTW);
                    fx.submit(d, ot, key);
                }
                let h = fx.mem.i16(sp + 0xB0) + 0x1E;
                fx.mem.set16(sp + 0xB0, h);
            }
            fx.mem.set32(sp + 0xA0, r as i32 + 1);
            fx.mem.set16(sp + 0xB0, 0);
        }
    }
    fx.leave(0x150);
}

/// 0x1E03D60(D, task, desc, mode): the head's trail (both modes draw): for
/// each of the +0x32 - 1 segments of D+0x2640.. a ring of 18 gouraud
/// quads (radius 1, 20 degrees each) in the camera's view-projection, the
/// task's colour with alpha falling by alpha / +0x32 a segment, key 0.
fn trail(fx: &mut Fx, t: u32, _d: u32, _mode: i32) {
    let sp = fx.enter(0x1E0);
    fx.mem.write_u32(sp + 0xEC, t);
    fx.mem.write_u32(sp + 0xB8, OT);
    let s1 = sp + 0x148;
    let n = fx.mem.i16(t + 0x32);
    l2::desc_gouraud4(fx, s1, 0);
    l2::fullscreen4(fx, s1);
    fx.mem.set32(sp + 0xD0, n);
    fx.mem.set32(sp + 0xA0, 0);
    copy_bytes4(fx, sp + 0x1DC, t + 0x28);
    fx.mem.set32(sp + 0xC0, n - 1);
    let mut a = 0i32;
    while a < n - 1 {
        let s3 = D + 0x10 * a as u32;
        l2::colour_strip4(fx, s1, sp + 0x1DC, 1);
        let al = i32::from(fx.mem.read_u8(t + 0x2B));
        let q = fx.div(al, n).0;
        let b = fx.mem.read_u8(sp + 0x1DF).wrapping_sub(q as u8);
        fx.mem.write_u8(sp + 0x1DF, b);
        l2::colour_strip4(fx, s1, sp + 0x1DC, 2);
        let mut s2 = 0i32;
        for _ in 0..0x12 {
            let s4 = (s2 + 0x14) as i16 as i32;
            for (p, o) in [(s3 + 0x2640, 0x100u32), (s3 + 0x2650, 0x120)] {
                copy3(fx, sp + o, p);
                copy3(fx, sp + o + 0x10, p);
                for (ang, at) in [(s2, o), (s4, o + 0x10)] {
                    let c = l2::sin_deg(fx, TAB, ang, 1);
                    let y = ee::add(fx.mem.f(sp + at + 4), ee::mul(1.0, c));
                    fx.mem.setf(sp + at + 4, y);
                    let s = l2::sin_deg(fx, TAB, ang, 0);
                    let x = ee::add(fx.mem.f(sp + at), ee::mul(1.0, s));
                    fx.mem.setf(sp + at, x);
                }
            }
            if l2::project(fx, sp + 0x100, SPR, 0, s1, sp + 0xF0) {
                fx.submit(s1, OT, 0);
            }
            s2 = s4;
        }
        a += 1;
        fx.mem.set32(sp + 0xA0, a);
    }
    fx.leave(0x1E0);
}
