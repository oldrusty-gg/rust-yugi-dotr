//! BMODULE b578 (card 663), its twin b619 (card 665), b253 (card 328: one
//! trail from node 1 on update 0x55 of length 34 with no wobble, D+0x48
//! -= 5 x the side D+0xC every update, colours (FF, 87, 37) / (C8, 50, 23)
//! / (80, 58, 44), returns at 0x6D / 0x78.. / 0xB7 / 0xB9) and b243 (card 629:
//! four trails from the attacker's nodes 0x25 / 0x47 (k & 1) on updates
//! 0x38 / 0x56 / 0x74 / 0x92 with lengths 0x5F - k, the data 0x10 higher
//! (a second node slot), phase (i << 16) >> 2, puff alpha 80 (32 - age) >>
//! 5, big alpha 2 (64 - age) with the alpha preset set per sprite, colours
//! (FF, 50, 37) and (0C, 08, 08), returns at 0x8D / 0x98.. / 0x12E /
//! 0x130), hand-ported from
//! the records (linked at 0x1E00000, one function 0x1E00080, data area a0
//! = 0x1ED0000, 0x2900 / 0x4CE0 bytes) [F]:
//! b048's family (crates/roses-fx/src/b048.rs) with one trail (b619
//! three) in a 0x3E0 (0x3F0) frame. Every update the camera turn with
//! roll and the attacker's node 0x27 (b619 0x79) into D+0x40. On update
//! 0x5A (b619 0x9A / 0xAE / 0xC2) the trail record (+0x1700, 0x70) starts
//! at the node aimed straight at the target (D+0x20 - D+0x40, no random
//! point) with a 16-sprite burst record (+0x2370 (b619 +0x3C50), 0x130)
//! at the node; b619 keeps each trail's length at +0x170C (50, the third
//! 47). A live trail (40 updates; b619 while age <= the length) draws
//! b048's tube (17 x 17 rings swayed by the angle table D+0x1270) at the
//! direction x age / 39 (b619 / length) plus 20 sin((age << 15) / 39) in
//! y, leaves a 4-sprite puff entry (+0x1770 (b619 +0x1850), 0x60, groups
//! of 32) an update and at its end a 64-sprite-pair big burst (+0x24B0
//! (b619 +0x3FF0), 0x450: 64 cells +8, 64 directions +0x50, drawn round
//! the target D+0x20). The sprites as b048's with the puff alpha 90 (32 -
//! age) >> 5, the burst alpha 2 (64 - age), the big alpha 100 (64 - age)
//! >> 6; colours: tube (DC, FF, 37) / b619 (C8, FF, 37), puffs / bursts /
//! big (6E, 80, 1C) / b619 puffs and the big's first sprite (C8, FF, 37),
//! bursts and the big's second (64, 80, 37). Returns 1 on update 0x77, 6
//! on 0x82..0xC0, 2 on 0xC1, 8 once the count reaches 0xC3 (b619 0xC2 /
//! 0xCD..0x130 / 0x131 / 0x133).
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
/// The entry's s0 (0x206E40's return), rand's saved s0 in the step.
const S0: u32 = OT;
const TWO_PI: f32 = f32::from_bits(0x40C9_0FDB);

/// What differs between the two records.
struct V {
    frame: u32,
    /// b474: its own stack map (b578 offset -> b474 offset).
    map: Option<fn(u32) -> u32>,
    /// Stack locals at or above `high_from` (0x100) sit this much higher
    /// (b619's phase local; b226 0x20 from 0x170, with `low_shift` 0x10
    /// from 0xC0: D kept at sp + 0xD0).
    /// phase local).
    shift: u32,
    high_from: u32,
    low_shift: u32,
    /// The rings' phase kept at sp + 0x100 (b619 / b243).
    phase_local: bool,
    bzero: u32,
    node: i32,
    /// b243's second node (D+0x34, requested without a clear): the trails
    /// start at node k & 1.
    node2: Option<i32>,
    /// Data 0x10 higher from D+0x50 on (b243: the second node's slot).
    data_shift: u32,
    /// Trail records (= groups = bursts = big bursts).
    n: i32,
    spawn_at: &'static [i32],
    /// The per-record length at +0xC (b619: 50, the third record 47; b243
    /// 0x5F - k); b578 keeps 39 in the code.
    length_field: bool,
    lengths: fn(i32) -> i32,
    /// The trail's phase local: (i << 16) / 3 (b619), >> 2 (b243) or i <<
    /// 16 itself.
    phase: fn(&mut Fx, i32) -> i32,
    /// D+0x48 += this x the side D+0xC every update (b578 / b619 0.0;
    /// b253 -5.0; b243 none).
    z_add: Option<f32>,
    /// The constant length (b578 39, b253 34) and the compiled (age << 15)
    /// / length, when there is no length field.
    length: f32,
    sin_arg: fn(&mut Fx, i32) -> i32,
    /// The 20 sin wobble in y (b253 none; b226 40).
    wobble: bool,
    wobble_amp: f32,
    /// The big bursts' time (64; b226 128: size, falloff and rise, life
    /// time + 1) and b226's two alphas ((time - age) for the first sprite,
    /// 100 (time - age) >> 7 kept at sp + 0xC0 for the second).
    big_time: i32,
    big_split: bool,
    /// b226's glint records (+0x2900, 0xA0) and their return addresses
    /// (rand, the alpha, the key).
    glints: Option<[u32; 3]>,
    /// The puff alpha's factor (90 (32 - age) >> 5; b243 80).
    puff_alpha: i32,
    /// The big burst alpha 100 (64 - age) >> 6 (else 2 (64 - age)).
    big_alpha_hundred: bool,
    /// b243: the big burst's alpha preset set per sprite (1, then 0)
    /// instead of once in the descriptor setup.
    big_presets: bool,
    /// The bursts' alpha preset (1; b243 0).
    burst_preset: i32,
    r_table: u32,
    /// Data offsets: the puff groups, the bursts, the big burst index and
    /// the big bursts.
    puffs: u32,
    bursts: u32,
    big_index: u32,
    bigs: u32,
    tube_colour: [u8; 3],
    puff_colour: [u8; 3],
    burst_colour: [u8; 3],
    big_colour: [[u8; 3]; 2],
    one: i32,
    six_from: i32,
    two: i32,
    end: i32,
    /// Return addresses: rand in the spawn's burst, the pitch's 0x1A2B80,
    /// project_n, the tube's 0x1A2C70, rand in the puff entry, rand in the
    /// big burst, the puffs' / bursts' / big (two) 0x1A2C70.
    r_burst: u32,
    r_pitch: u32,
    r_proj: u32,
    r_tube: u32,
    r_puff: u32,
    r_big: u32,
    r_puffs: u32,
    r_bursts: u32,
    r_bigs: [u32; 2],
}

fn phase_same(_: &mut Fx, x: i32) -> i32 {
    x
}

/// (i << 16) / 3 as compiled (the multiply by 0x55555556).
fn phase_third(fx: &mut Fx, x: i32) -> i32 {
    fx.mult(0x5555_5556, x);
    let hi = fx.hilo().1;
    hi.wrapping_add(((x as u32) >> 31) as i32)
}

fn phase_quarter(_: &mut Fx, x: i32) -> i32 {
    x >> 2
}

/// x / 39 as compiled (the multiply by 0xD20D20D3).
fn div39(fx: &mut Fx, x: i32) -> i32 {
    fx.mult(0xD20D_20D3u32 as i32, x);
    let hi = fx.hilo().1;
    (hi.wrapping_add(x) >> 5).wrapping_add(((x as u32) >> 31) as i32)
}

/// x / 34 as compiled (the multiply by 0x78787879).
fn div34(fx: &mut Fx, x: i32) -> i32 {
    fx.mult(0x7878_7879, x);
    let hi = fx.hilo().1;
    (hi >> 4).wrapping_add(((x as u32) >> 31) as i32)
}

const B578: V = V {
    frame: 0x3E0,
    map: None,
    shift: 0,
    high_from: 0x100,
    low_shift: 0,
    phase_local: false,
    bzero: 0x2900,
    node: 0x27,
    node2: None,
    data_shift: 0,
    n: 1,
    spawn_at: &[0x5A],
    length_field: false,
    lengths: |_| 39,
    phase: phase_same,
    z_add: Some(0.0),
    length: 39.0,
    sin_arg: div39,
    wobble: true,
    wobble_amp: 20.0,
    big_time: 0x40,
    big_split: false,
    glints: None,
    puff_alpha: 90,
    big_alpha_hundred: true,
    big_presets: false,
    burst_preset: 1,
    r_table: 0x01E0_0198,
    puffs: 0x1770,
    bursts: 0x2370,
    big_index: 0x24A0,
    bigs: 0x24B0,
    tube_colour: [0xDC, 0xFF, 0x37],
    puff_colour: [0x6E, 0x80, 0x1C],
    burst_colour: [0x6E, 0x80, 0x1C],
    big_colour: [[0x6E, 0x80, 0x1C], [0x6E, 0x80, 0x1C]],
    one: 0x77,
    six_from: 0x82,
    two: 0xC1,
    end: 0xC3,
    r_burst: 0x01E0_03A0,
    r_pitch: 0x01E0_05DC,
    r_proj: 0x01E0_09B4,
    r_tube: 0x01E0_09C8,
    r_puff: 0x01E0_0A78,
    r_big: 0x01E0_0B20,
    r_puffs: 0x01E0_0ED0,
    r_bursts: 0x01E0_1238,
    r_bigs: [0x01E0_15B0, 0x01E0_1668],
};

const B619: V = V {
    frame: 0x3F0,
    map: None,
    shift: 0x10,
    high_from: 0x100,
    low_shift: 0,
    phase_local: true,
    bzero: 0x4CE0,
    node: 0x79,
    node2: None,
    data_shift: 0,
    n: 3,
    spawn_at: &[0x9A, 0xAE, 0xC2],
    length_field: true,
    lengths: |k| if k == 2 { 0x2F } else { 0x32 },
    phase: phase_third,
    z_add: Some(0.0),
    length: 0.0,
    sin_arg: div39,
    wobble: true,
    wobble_amp: 20.0,
    big_time: 0x40,
    big_split: false,
    glints: None,
    puff_alpha: 90,
    big_alpha_hundred: true,
    big_presets: false,
    burst_preset: 1,
    r_table: 0x01E0_0198,
    puffs: 0x1850,
    bursts: 0x3C50,
    big_index: 0x3FE0,
    bigs: 0x3FF0,
    tube_colour: [0xC8, 0xFF, 0x37],
    puff_colour: [0xC8, 0xFF, 0x37],
    burst_colour: [0x64, 0x80, 0x37],
    big_colour: [[0xC8, 0xFF, 0x37], [0x64, 0x80, 0x37]],
    one: 0xC2,
    six_from: 0xCD,
    two: 0x131,
    end: 0x133,
    r_burst: 0x01E0_03E4,
    r_pitch: 0x01E0_0604,
    r_proj: 0x01E0_09FC,
    r_tube: 0x01E0_0A10,
    r_puff: 0x01E0_0AC0,
    r_big: 0x01E0_0B68,
    r_puffs: 0x01E0_0F18,
    r_bursts: 0x01E0_1280,
    r_bigs: [0x01E0_15F8, 0x01E0_16B0],
};

const B243: V = V {
    frame: 0x3F0,
    map: None,
    shift: 0x10,
    high_from: 0x100,
    low_shift: 0,
    phase_local: true,
    bzero: 0x5EE0,
    node: 0x25,
    node2: Some(0x47),
    data_shift: 0x10,
    n: 4,
    spawn_at: &[0x38, 0x56, 0x74, 0x92],
    length_field: true,
    lengths: |k| 0x5F - k,
    phase: phase_quarter,
    z_add: None,
    length: 0.0,
    sin_arg: div39,
    wobble: true,
    wobble_amp: 20.0,
    big_time: 0x40,
    big_split: false,
    glints: None,
    puff_alpha: 80,
    big_alpha_hundred: false,
    big_presets: true,
    burst_preset: 0,
    r_table: 0x01E0_01B8,
    puffs: 0x18D0,
    bursts: 0x48D0,
    big_index: 0x4D90,
    bigs: 0x4DA0,
    tube_colour: [0xFF, 0x50, 0x37],
    puff_colour: [0xFF, 0x50, 0x37],
    burst_colour: [0x0C, 0x08, 0x08],
    big_colour: [[0xFF, 0x50, 0x37], [0x0C, 0x08, 0x08]],
    one: 0x8D,
    six_from: 0x98,
    two: 0x12E,
    end: 0x130,
    r_burst: 0x01E0_04A0,
    r_pitch: 0x01E0_06C0,
    r_proj: 0x01E0_0AA4,
    r_tube: 0x01E0_0AB8,
    r_puff: 0x01E0_0B68,
    r_big: 0x01E0_0C10,
    r_puffs: 0x01E0_0FB8,
    r_bursts: 0x01E0_1320,
    r_bigs: [0x01E0_1684, 0x01E0_1754],
};

const B253: V = V {
    frame: 0x3E0,
    map: None,
    shift: 0,
    high_from: 0x100,
    low_shift: 0,
    phase_local: false,
    bzero: 0x2900,
    node: 1,
    node2: None,
    data_shift: 0,
    n: 1,
    spawn_at: &[0x55],
    length_field: false,
    lengths: |_| 34,
    phase: phase_same,
    z_add: Some(-5.0),
    length: 34.0,
    sin_arg: div34,
    wobble: false,
    wobble_amp: 20.0,
    big_time: 0x40,
    big_split: false,
    glints: None,
    puff_alpha: 90,
    big_alpha_hundred: true,
    big_presets: false,
    burst_preset: 1,
    r_table: 0x01E0_0198,
    puffs: 0x1770,
    bursts: 0x2370,
    big_index: 0x24A0,
    bigs: 0x24B0,
    tube_colour: [0xFF, 0x87, 0x37],
    puff_colour: [0xC8, 0x50, 0x23],
    burst_colour: [0x80, 0x58, 0x44],
    big_colour: [[0xC8, 0x50, 0x23], [0x80, 0x58, 0x44]],
    one: 0x6D,
    six_from: 0x78,
    two: 0xB7,
    end: 0xB9,
    r_burst: 0x01E0_03B4,
    r_pitch: 0x01E0_05E0,
    r_proj: 0x01E0_09B4,
    r_tube: 0x01E0_09C8,
    r_puff: 0x01E0_0A78,
    r_big: 0x01E0_0B20,
    r_puffs: 0x01E0_0ED0,
    r_bursts: 0x01E0_1238,
    r_bigs: [0x01E0_15B0, 0x01E0_1668],
};

const B226: V = V {
    frame: 0x400,
    map: None,
    shift: 0x20,
    high_from: 0x170,
    low_shift: 0x10,
    phase_local: false,
    bzero: 0x3D00,
    node: 0x63,
    node2: None,
    data_shift: 0,
    n: 1,
    spawn_at: &[0x50],
    length_field: false,
    lengths: |_| 59,
    phase: phase_same,
    z_add: Some(0.0),
    length: 59.0,
    sin_arg: div59,
    wobble: true,
    wobble_amp: 40.0,
    big_time: 0x80,
    big_split: true,
    glints: Some([0x01E0_177C, 0x01E0_1884, 0x01E0_1AD8]),
    puff_alpha: 90,
    big_alpha_hundred: true,
    big_presets: false,
    burst_preset: 1,
    r_table: 0x01E0_0198,
    puffs: 0x1770,
    bursts: 0x2370,
    big_index: 0x24A0,
    bigs: 0x24B0,
    tube_colour: [0x37, 0xFF, 0xDC],
    puff_colour: [0x1C, 0x80, 0x6E],
    burst_colour: [0x1C, 0x80, 0x6E],
    big_colour: [[0x1C, 0x80, 0x6E], [0x1C, 0x80, 0x6E]],
    one: 0x81,
    six_from: 0x8C,
    two: 0x10B,
    end: 0x10D,
    r_burst: 0x01E0_03C4,
    r_pitch: 0x01E0_0600,
    r_proj: 0x01E0_09D4,
    r_tube: 0x01E0_09E8,
    r_puff: 0x01E0_0A98,
    r_big: 0x01E0_0B40,
    r_puffs: 0x01E0_0EF0,
    r_bursts: 0x01E0_1258,
    r_bigs: [0x01E0_15D8, 0x01E0_1694],
};

/// x / 59 as compiled (the multiply by 0x22B63CBF).
fn div59(fx: &mut Fx, x: i32) -> i32 {
    fx.mult(0x22B6_3CBF, x);
    let hi = fx.hilo().1;
    (hi >> 3).wrapping_add(((x as u32) >> 31) as i32)
}

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B578)
}

/// b619's entry 0x1E00080.
pub fn program_b619(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B619)
}

/// b243's entry 0x1E00080.
pub fn program_b243(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B243)
}

/// b253's entry 0x1E00080.
pub fn program_b253(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B253)
}

/// b226's entry 0x1E00080.
pub fn program_b226(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B226)
}

fn run(fx: &mut Fx, param: i32, v: &V) -> i32 {
    let sp = fx.enter(v.frame);
    fx.mem.write_u32(at(sp, v, 0xC0), D);
    fx.mem.write_u32(sp + 0xB0, 0);
    let ret = if param >= 0 { init(fx, sp, v) } else { step(fx, sp, v) };
    fx.leave(v.frame);
    ret
}

/// A stack local: b578's offset, b619's 0x10 higher from 0x100 on (b226
/// 0x10 from 0xC0, 0x20 from 0x170).
fn at(sp: u32, v: &V, off: u32) -> u32 {
    if let Some(f) = v.map {
        return sp + f(off);
    }
    sp + off + if off >= v.high_from { v.shift } else if off >= 0xC0 { v.low_shift } else { 0 }
}

/// The compiler's signed remainder by a power of two.
fn rem2(x: i32, m: i32) -> i32 {
    let r = x & (m - 1);
    if x < 0 && r != 0 {
        r - m
    } else {
        r
    }
}

fn words(fx: &mut Fx, to: u32, from: u32) {
    for c in 0..4u32 {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

fn init(fx: &mut Fx, sp: u32, v: &V) -> i32 {
    fx.bzero(D, v.bzero);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    for k in 0..3u32 {
        let r = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x10 + 4 * k, r);
    }
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, v.node, 1);
    fx.mem.set32(D + 0x30, r);
    if let Some(node) = v.node2 {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, 0);
        fx.mem.set32(D + 0x34, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x20);
    let ds = v.data_shift;
    fx.mem.write_u32(D + 0x50 + ds, 0);
    for k in 0..0x11u32 {
        let row = D + 0x44 * k + ds;
        for j in 0..0x10u32 {
            let r = fx.rand_from(k, v.r_table) >> 23;
            fx.mem.set32(row + 0x1270 + 4 * j, (rem2(r, 256) << 16) >> 8);
        }
        let w = fx.mem.read_u32(row + 0x1270);
        fx.mem.write_u32(row + 0x12B0, w);
    }
    let n = v.n as u32;
    for k in 0..n {
        fx.mem.write_u8(D + 0x1700 + ds + 0x70 * k, 0);
        fx.mem.write_u32(D + 0x1708 + ds + 0x70 * k, 0);
    }
    for g in 0..n {
        for e in 0..0x20u32 {
            fx.mem.write_u8(D + v.puffs + 0xC00 * g + 0x60 * e, 0);
        }
    }
    for k in 0..n {
        fx.mem.write_u8(D + v.bursts + 0x130 * k, 0);
    }
    fx.mem.write_u32(D + v.big_index, 0);
    for k in 0..n {
        fx.mem.write_u8(D + v.bigs + 0x450 * k, 0);
    }
    if v.glints.is_some() {
        // b226: the glint clear loop steps the stack copy of a0.
        for k in 0..0x20u32 {
            fx.mem.write_u8(D + 0x2900 + 0xA0 * k, 0);
        }
        fx.mem.write_u32(at(sp, v, 0xC0), D + 0xA0 * 0x20);
    } else {
        // The clear loop steps the stack copy of a0.
        fx.mem.write_u32(at(sp, v, 0xC0), D + 0x450 * n);
    }
    0
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

/// A 4 x 8 cell `b` of the texture: (u0, v0, u1, v1).
fn cell(b: u8) -> (u32, u32, u32, u32) {
    let (c, r) = (u32::from(b & 3), u32::from(b >> 2));
    ((c << 9) & 0x3FFF, (r << 9) & 0x3FFF, ((32 * c + 32) << 4) & 0x3FFF, ((32 * r + 32) << 4) & 0x3FFF)
}

/// The sprite cell at the descriptor's UV halfwords.
fn sprite_uvs(fx: &mut Fx, sp: u32, v: &V, b: u8) {
    let (u0, v0, u1, v1) = cell(b);
    uv(fx, at(sp, v, 0x308), u0);
    uv(fx, at(sp, v, 0x30A), v0);
    uv(fx, at(sp, v, 0x318), u1);
    uv(fx, at(sp, v, 0x31A), v1);
}

/// The view-projection (x identity).
fn camera(fx: &mut Fx, sp: u32, v: &V) {
    let (vp, m) = (at(sp, v, 0x1D0), at(sp, v, 0x210));
    fx.camera_vp_to(vp);
    fx.identity(m);
    let id = fx.read_m4(m);
    fx.mul_into(Some(vp), &id);
}

/// The sprite's corners (-s, -s, 0, 1) and (s, s, 0, 1) turned by the
/// camera.
fn corners(fx: &mut Fx, sp: u32, v: &V, s: f32) {
    let (a, b, cam) = (at(sp, v, 0x290), at(sp, v, 0x2A0), at(sp, v, 0x250));
    let n = ee::neg(s);
    fx.mem.setf(a, n);
    fx.mem.setf(a + 4, n);
    fx.mem.write_u32(a + 8, 0);
    fx.mem.write_u32(a + 0xC, 0x3F80_0000);
    fx.mem.setf(b, s);
    fx.mem.setf(b + 4, s);
    fx.mem.write_u32(b + 8, 0);
    fx.mem.write_u32(b + 0xC, 0x3F80_0000);
    fx.vu_apply_to(a, cam, a);
    fx.vu_apply_to(b, cam, b);
}

/// `m` x (0.6 + 0.4 age / `d`).
fn half_size(age: i32, d: f32, m: f32) -> f32 {
    let f = ee::div(ee::mul(f32::from_bits(0x3ECC_CCCD), ee::from_i32(age)), d);
    ee::mul(m, ee::add(f32::from_bits(0x3F19_999A), f))
}

/// The sprite colour (Q = 1.0).
fn colour(fx: &mut Fx, sp: u32, v: &V, c: [u8; 3], a: u8) {
    let p = at(sp, v, 0x300);
    fx.mem.write_u8(p, c[0]);
    fx.mem.write_u8(p + 1, c[1]);
    fx.mem.write_u8(p + 2, c[2]);
    fx.mem.write_u8(p + 3, a);
    fx.mem.write_u32(p + 4, 0x3F80_0000);
}

/// The sprite descriptor (5 registers 0x42421, prim 0x26 | 0x800) with
/// its pointers, alpha preset 1, texture `rh`, clamp 5, and the camera.
fn sprite_desc(fx: &mut Fx, sp: u32, v: &V, rh: u32, alpha: Option<i32>) {
    let d = at(sp, v, 0x2F0);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u32(at(sp, v, 0x140), d + 2);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u32(at(sp, v, 0x150), d + 8);
    fx.mem.write_u32(at(sp, v, 0x160), d + 4);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    if let Some(preset) = alpha {
        let a = fx.alpha_preset(preset);
        fx.desc_alpha(d, a);
    }
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp, v);
}

/// Projected (0x204F70 into 0x2D3600 through the view-projection) and
/// submitted at the mean w.
fn draw(fx: &mut Fx, sp: u32, v: &V, d: u32, ra: u32) {
    let w = at(sp, v, 0x3DC);
    if crate::fxlib2::project(fx, at(sp, v, 0x2B0), PROJ_OUT, at(sp, v, 0x1D0), d, w) {
        let w = fx.mem.f(w);
        fx.fix_residue(w, ra);
        let key = ee::fixunssfsi(w) as i32;
        fx.submit(d, OT, key);
    }
}

/// A sprite at the direction `dir` x `f` plus the point `p`: the two
/// corners into the vertex slots.
fn sprite(fx: &mut Fx, sp: u32, v: &V, dir: u32, f: f32, p: u32) {
    let q = at(sp, v, 0x2D0);
    fx.vu_scale4(q, dir, f);
    fx.vu_add4(q, q, p);
    fx.vu_add4(at(sp, v, 0x2B0), at(sp, v, 0x290), q);
    fx.vu_add4(at(sp, v, 0x2C0), at(sp, v, 0x2A0), q);
}

/// A trail record started at the node, aimed at the target [F 0x1E002E8].
fn spawn(fx: &mut Fx, v: &V) {
    let ds = v.data_shift;
    let k = fx.mem.i32(D + 0x50 + ds);
    let rec = D + 0x70 * k as u32 + 0x1700 + ds;
    fx.mem.write_u8(rec, 1);
    fx.mem.write_u32(rec + 4, 0);
    if v.length_field {
        fx.mem.set32(rec + 0xC, (v.lengths)(k));
    }
    let src = if v.node2.is_some() { D + 0x40 + 0x10 * (k & 1) as u32 } else { D + 0x40 };
    words(fx, rec + 0x10, src);
    fx.vu_sub4(rec + 0x20, D + 0x20, if v.node2.is_some() { rec + 0x10 } else { src });
    let b = D + 0x130 * k as u32 + v.bursts;
    fx.mem.write_u8(b, 1);
    fx.mem.write_u32(b + 4, 0);
    words(fx, b + 0x20, src);
    for j in 0..0x10u32 {
        let r = fx.rand_from(S0, v.r_burst) >> 23;
        fx.mem.set8(b + j + 8, rem2(r, 8));
        fx.random_table_vec(b + 0x10 * j + 0x30, f32::from_bits(0x3F33_3333));
    }
    let k = fx.mem.i32(D + 0x50 + ds) + 1;
    fx.mem.set32(D + 0x50 + ds, k);
    if k >= v.n {
        fx.mem.write_u32(D + 0x50 + ds, 0);
    }
}

/// The trail's point at `age`: the direction x age / length plus 20
/// sin((age << 15) / length) in y, into `out`.
fn point(fx: &mut Fx, rec: u32, age: i32, out: u32, v: &V) {
    let x = age << 15;
    let (len, a) = if v.length_field {
        let len = fx.mem.i32(rec + 0xC);
        let (q, _) = fx.div(x, len);
        (ee::from_i32(len), q)
    } else {
        (v.length, (v.sin_arg)(fx, x))
    };
    let f = ee::div(ee::from_i32(age), len);
    fx.vu_scale4(out, rec + 0x20, f);
    let s = fx.tsin(a & 0xFFFF);
    let y = fx.mem.f(out + 4);
    let y = if v.wobble { ee::add(y, ee::mul(v.wobble_amp, s)) } else { ee::add(y, 0.0) };
    fx.mem.setf(out + 4, y);
}

/// The tube's 17 rings of 17 vertices at D+0x60 [F 0x1E00640].
fn rings(fx: &mut Fx, sp: u32, v: &V, i: i32, age: i32) {
    fx.mem.write_u32(at(sp, v, 0xD0), 0);
    fx.mem.write_u32(at(sp, v, 0xE0), D);
    fx.mem.write_u32(at(sp, v, 0xF0), D);
    let x = i << 16;
    let phase = (v.phase)(fx, x);
    if v.phase_local {
        fx.mem.set32(sp + 0x100, phase);
    }
    let ds = v.data_shift;
    for k in 0..0x11i32 {
        let c = fx.tcos((k * 0x8000) >> 4);
        let f21 = ee::mul(4.0, c);
        let f20 = Fx::powf2(4.0);
        let f0 = Fx::powf2(f21);
        let radius = fx.sqrtf(ee::sub(f20, f0));
        let height = ee::mul(2.0, ee::add(-2.0, f21));
        let s7 = phase.wrapping_add((rem2(k, 8) << 16) >> 3);
        let row = D + 0x44 * k as u32 + ds;
        let out = D + 0x110 * k as u32 + ds;
        for j in 0..0x11i32 {
            let t = fx.mem.i32(row + 0x1270 + 4 * j as u32);
            let m = fx.mult(age, 0x457);
            let a0 = t.wrapping_sub(m) & 0xFFFF;
            let m = fx.mult(age, 0x8AE);
            let s1 = ((rem2(j, 8) << 16) >> 3).wrapping_add(s7).wrapping_sub(m) & 0xFFFF;
            let sn = fx.tsin(a0);
            let f21 = ee::add(1.0, ee::mul(f32::from_bits(0x3DCC_CCCD), sn));
            let sn = fx.tsin(s1);
            let f0 = ee::add(f21, ee::mul(f32::from_bits(0x3DCC_CCCD), sn));
            let a = j << 12;
            let sn = fx.tsin(a);
            let f21 = ee::mul(radius, f0);
            let vx = ee::mul(f21, sn);
            let cs = fx.tcos(a);
            let p = out + 0x60 + 0x10 * j as u32;
            fx.mem.setf(p, vx);
            fx.mem.setf(p + 4, ee::mul(f21, cs));
            fx.mem.setf(p + 8, height);
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        }
        fx.mem.set32(at(sp, v, 0xD0), (k + 1) * 0x8000);
        fx.mem.write_u32(at(sp, v, 0xE0), row + 0x44);
        fx.mem.write_u32(at(sp, v, 0xF0), out + 0x110);
    }
}

/// The tube's 16 x 16 quads drawn [F 0x1E007E4].
fn tube(fx: &mut Fx, sp: u32, v: &V, age: i32) {
    let d = at(sp, v, 0x348);
    let (_, r) = fx.div(age, 0x18);
    let f20 = ee::div(ee::mul(-1.0, ee::from_i32(r)), 24.0);
    for k in 0..0x10i32 {
        let f3 = ee::div(ee::from_i32(rem2(k, 16)), 16.0);
        let f2 = ee::div(ee::from_i32(rem2(k, 16) + 1), 16.0);
        let a0 = (150 * (16 - k)) >> 4;
        let a1 = (150 * (16 - (k + 1))) >> 4;
        for (o, a) in [(0x10u32, a0), (0x40, a1)] {
            fx.mem.write_u8(d + o, v.tube_colour[0]);
            fx.mem.write_u8(d + o + 1, v.tube_colour[1]);
            fx.mem.write_u8(d + o + 2, v.tube_colour[2]);
            fx.mem.write_u8(d + o + 3, a as u8);
            fx.mem.write_u32(d + o + 4, 0x3F80_0000);
            let (c, q) = (fx.mem.read_u32(d + o), fx.mem.read_u32(d + o + 4));
            fx.mem.write_u32(d + o + 0x18, c);
            fx.mem.write_u32(d + o + 0x1C, q);
        }
        let f21 = ee::add(f3, f20);
        let f22 = ee::add(f2, f20);
        let row0 = D + 0x110 * k as u32 + v.data_shift;
        let row1 = D + 0x110 * (k + 1) as u32 + v.data_shift;
        for j in 0..0x10i32 {
            let f2 = ee::div(ee::from_i32(rem2(j, 16)), 16.0);
            let f0 = ee::div(ee::from_i32(rem2(j, 16) + 1), 16.0);
            fx.mem.setf(d + 0x18, f21);
            fx.mem.setf(d + 0x1C, f2);
            fx.mem.setf(d + 0x30, f21);
            fx.mem.setf(d + 0x34, f0);
            fx.mem.setf(d + 0x48, f22);
            fx.mem.setf(d + 0x4C, f2);
            fx.mem.setf(d + 0x60, f22);
            fx.mem.setf(d + 0x64, f0);
            let (p0, p1) = (0x60 + 0x10 * j as u32, 0x60 + 0x10 * (j + 1) as u32);
            let w = at(sp, v, 0x3DC);
            let r = fx.project_n(&[row0 + p0, row0 + p1, row1 + p0, row1 + p1], at(sp, v, 0x1D0), d, w, v.r_proj);
            if r == 1 {
                let w = fx.mem.f(w);
                fx.fix_residue(w, v.r_tube);
                let key = ee::fixunssfsi(w) as i32;
                fx.submit(d, OT, key);
            }
        }
    }
}

/// The trail records [F 0x1E0046C]: each live one draws its tube, leaves a
/// puff entry in its group, and at its end a big burst.
fn trails(fx: &mut Fx, sp: u32, v: &V) {
    let (pt, nx, vp, m) = (at(sp, v, 0x2D0), at(sp, v, 0x2E0), at(sp, v, 0x1D0), at(sp, v, 0x210));
    fx.mem.write_u32(at(sp, v, 0x1A0), 0);
    fx.mem.write_u32(at(sp, v, 0x100), 0);
    fx.mem.write_u32(at(sp, v, 0x110), D);
    for i in 0..v.n {
        let rec = D + 0x70 * i as u32 + 0x1700 + v.data_shift;
        let grp = D + 0xC00 * i as u32;
        if fx.mem.read_u8(rec) != 0 {
            fx.camera_vp_to(vp);
            let age = fx.mem.i32(rec + 4);
            point(fx, rec, age, pt, v);
            let age1 = fx.mem.i32(rec + 4) + 1;
            point(fx, rec, age1, nx, v);
            fx.vu_sub4(nx, nx, pt);
            let (y, z) = (fx.mem.f(nx + 4), fx.mem.f(nx + 8));
            let a = fx.atan2f(y, z);
            let f = ee::mul(65536.0, ee::div(a, TWO_PI));
            fx.fix_residue(f, v.r_pitch);
            let pitch = ee::fixsfsi(f).wrapping_neg() & 0xFFFF;
            fx.identity(rec + 0x30);
            fx.rotate_axis(rec + 0x30, pitch, 0);
            fx.vu_add4(pt, pt, rec + 0x10);
            fx.translate_rows(m, rec + 0x30, pt);
            let mm = fx.read_m4(m);
            fx.mul_into(Some(vp), &mm);
            let age = fx.mem.i32(rec + 4);
            rings(fx, sp, v, i, age);
            let age = fx.mem.i32(rec + 4);
            tube(fx, sp, v, age);
            let c = fx.mem.i32(rec + 8);
            fx.mem.set32(rec + 8, c + 1);
            if c + 1 >= 0x20 {
                fx.mem.write_u32(rec + 8, 0);
            }
            let e = grp + 0x60 * c as u32 + v.puffs;
            fx.mem.write_u8(e, 1);
            fx.mem.write_u32(e + 4, 0);
            words(fx, e + 0x10, pt);
            for q in 0..4u32 {
                let r = fx.rand_from(S0, v.r_puff) >> 23;
                fx.mem.set8(e + q + 8, rem2(r, 8));
                fx.random_table_vec(e + 0x10 * q + 0x20, f32::from_bits(0x3F4C_CCCD));
            }
            let age = fx.mem.i32(rec + 4) + 1;
            fx.mem.set32(rec + 4, age);
            let dead = if v.length_field { fx.mem.i32(rec + 0xC) < age } else { age > (v.lengths)(0) };
            if dead {
                fx.mem.write_u8(rec, 0);
                let k = fx.mem.i32(D + v.big_index);
                let b = D + 0x450 * k as u32 + v.bigs;
                fx.mem.write_u8(b, 1);
                fx.mem.write_u32(b + 4, 0);
                for q in 0..0x40u32 {
                    let r = fx.rand_from(S0, v.r_big) >> 23;
                    fx.mem.set8(b + q + 8, rem2(r, 8));
                    fx.random_table_vec(b + 0x10 * q + 0x50, f32::from_bits(0x3FD9_999A));
                }
                let k = fx.mem.i32(D + v.big_index) + 1;
                fx.mem.set32(D + v.big_index, k);
                if k >= v.n {
                    fx.mem.write_u32(D + v.big_index, 0);
                }
            }
        }
        fx.mem.set32(at(sp, v, 0x100), (i + 1) << 16);
        fx.mem.write_u32(at(sp, v, 0x110), grp + 0xC00);
        fx.mem.set32(at(sp, v, 0x1A0), i + 1);
    }
}

/// The puffs [F 0x1E00BC4]: groups of 32 entries.
fn puffs(fx: &mut Fx, sp: u32, v: &V) {
    let d = at(sp, v, 0x2F0);
    let rh = fx.mem.read_u32(D + 0x14);
    sprite_desc(fx, sp, v, rh, Some(1));
    fx.mem.write_u32(at(sp, v, 0x190), 0);
    fx.mem.write_u32(at(sp, v, 0x130), D);
    for i in 0..v.n {
        let grp = D + 0xC00 * i as u32;
        fx.mem.write_u32(at(sp, v, 0x1B0), 0);
        fx.mem.write_u32(at(sp, v, 0x120), grp);
        fx.mem.write_u32(at(sp, v, 0x1C0), 0);
        for e in 0..0x20u32 {
            let s4 = grp + 0x60 * e + v.puffs;
            if fx.mem.read_u8(s4) != 0 {
                let age = fx.mem.i32(s4 + 4);
                let s = half_size(age, 32.0, 10.0);
                corners(fx, sp, v, s);
                let age = fx.mem.i32(s4 + 4);
                let a = ((0x20 - age) * v.puff_alpha) >> 5;
                colour(fx, sp, v, v.puff_colour, a as u8);
                let age = fx.mem.i32(s4 + 4);
                let f20 = ee::add(f32::from_bits(0x3E4C_CCCD), ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), ee::from_i32(age)), 32.0));
                for q in 0..4u32 {
                    let b = fx.mem.read_u8(s4 + q + 8);
                    sprite_uvs(fx, sp, v, b);
                    sprite(fx, sp, v, s4 + 0x10 * q + 0x20, f20, s4 + 0x10);
                    draw(fx, sp, v, d, v.r_puffs);
                }
                let age = fx.mem.i32(s4 + 4) + 1;
                fx.mem.set32(s4 + 4, age);
                if age >= 0x21 {
                    fx.mem.write_u8(s4, 0);
                }
            }
            fx.mem.write_u32(at(sp, v, 0x1C0), e + 1);
        }
        fx.mem.write_u32(at(sp, v, 0x120), grp + 0xC00);
        fx.mem.write_u32(at(sp, v, 0x1B0), 1);
        fx.mem.write_u32(at(sp, v, 0x130), grp + 0xC00);
        fx.mem.set32(at(sp, v, 0x190), i + 1);
    }
}

/// The bursts [F 0x1E00F7C]: records of 16 sprites.
fn bursts(fx: &mut Fx, sp: u32, v: &V) {
    let d = at(sp, v, 0x2F0);
    let rh = fx.mem.read_u32(D + 0x18);
    sprite_desc(fx, sp, v, rh, Some(v.burst_preset));
    fx.mem.write_u32(at(sp, v, 0x180), 0);
    for k in 0..v.n {
        let s4 = D + 0x130 * k as u32 + v.bursts;
        if fx.mem.read_u8(s4) != 0 {
            let age = fx.mem.i32(s4 + 4);
            let s = half_size(age, 64.0, 10.0);
            corners(fx, sp, v, s);
            let age = fx.mem.i32(s4 + 4);
            let a = ((0x40 - age) << 7) >> 6;
            colour(fx, sp, v, v.burst_colour, a as u8);
            let age = fx.mem.i32(s4 + 4);
            let f0 = fx.tan_falloff_p(age, 0x40, 1);
            let f20 = ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), f0));
            for j in 0..0x10u32 {
                let b = fx.mem.read_u8(s4 + j + 8);
                sprite_uvs(fx, sp, v, b);
                sprite(fx, sp, v, s4 + 0x10 * j + 0x30, f20, s4 + 0x20);
                draw(fx, sp, v, d, v.r_bursts);
            }
            let age = fx.mem.i32(s4 + 4) + 1;
            fx.mem.set32(s4 + 4, age);
            if age >= 0x41 {
                fx.mem.write_u8(s4, 0);
            }
        }
        fx.mem.set32(at(sp, v, 0x180), k + 1);
    }
}

/// The big bursts [F 0x1E01298]: records of 64 sprite pairs round the
/// target.
fn big_bursts(fx: &mut Fx, sp: u32, v: &V) {
    let d = at(sp, v, 0x2F0);
    let rh = fx.mem.read_u32(D + 0x18);
    sprite_desc(fx, sp, v, rh, if v.big_presets { None } else { Some(1) });
    fx.mem.write_u32(at(sp, v, 0x170), 0);
    let (pt, va, vb, ca, cb) = (at(sp, v, 0x2D0), at(sp, v, 0x2B0), at(sp, v, 0x2C0), at(sp, v, 0x290), at(sp, v, 0x2A0));
    for m in 0..v.n {
        let s7 = D + 0x450 * m as u32 + v.bigs;
        if fx.mem.read_u8(s7) != 0 {
            let bt = v.big_time;
            let age = fx.mem.i32(s7 + 4);
            let s = half_size(age, ee::from_i32(bt), 13.0);
            corners(fx, sp, v, s);
            let age = fx.mem.i32(s7 + 4);
            let (a, a2) = if v.big_split {
                let a2 = ((100 * (bt - age)) >> 7) & 0xFF;
                fx.mem.write_u8(sp + 0xC0, a2 as u8);
                ((bt - age) & 0xFF, a2)
            } else if v.big_alpha_hundred {
                let a = ((100 * (0x40 - age)) >> 6) & 0xFF;
                (a, a)
            } else {
                let a = (((0x40 - age) << 7) >> 6) & 0xFF;
                (a, a)
            };
            let f0 = fx.tan_falloff_p(age, bt, 1);
            let f21 = ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), f0));
            let age = fx.mem.i32(s7 + 4);
            let f20 = ee::mul(50.0, ee::div(ee::from_i32(age), ee::from_i32(bt)));
            for j in 0..0x40u32 {
                if v.big_presets {
                    let p = fx.alpha_preset(1);
                    fx.desc_alpha(d, p);
                }
                let rh = fx.mem.read_u32(D + 0x14);
                fx.desc_texture(d, rh, 1);
                let b = fx.mem.read_u8(s7 + j + 8);
                sprite_uvs(fx, sp, v, b);
                sprite(fx, sp, v, s7 + 0x10 * j + 0x50, f21, D + 0x20);
                colour(fx, sp, v, v.big_colour[0], a as u8);
                draw(fx, sp, v, d, v.r_bigs[0]);
                if v.big_presets {
                    let p = fx.alpha_preset(0);
                    fx.desc_alpha(d, p);
                }
                let rh = fx.mem.read_u32(D + 0x18);
                fx.desc_texture(d, rh, 1);
                let y = fx.mem.f(pt + 4);
                fx.mem.setf(pt + 4, ee::add(y, f20));
                fx.vu_add4(va, ca, pt);
                fx.vu_add4(vb, cb, pt);
                colour(fx, sp, v, v.big_colour[1], a2 as u8);
                draw(fx, sp, v, d, v.r_bigs[1]);
            }
            let age = fx.mem.i32(s7 + 4) + 1;
            fx.mem.set32(s7 + 4, age);
            if age >= bt + 1 {
                fx.mem.write_u8(s7, 0);
            }
        }
        fx.mem.set32(at(sp, v, 0x170), m + 1);
    }
}

/// b226's glint records [F 0x1E016F8 / 0x1E017C0]: from update 0x1D to
/// 0x45 one an update (+0x2900, 0xA0) of 8 random sprites (0.4) at the
/// node, 32 updates (texture D+0x18, half size 2 (0.3 + 0.7 falloff),
/// spread 0.3 + 0.7 age / 32, rising 5 age / 32, z - 5 age / 32 x the
/// side, alpha 100 cos((age << 14) >> 5), colour (37, FF, DC)).
fn glints(fx: &mut Fx, sp: u32, v: &V, ra: [u32; 3]) {
    let n = fx.mem.i32(D);
    if (0x1D..0x46).contains(&n) {
        let g = D + 0x2900 + 0xA0 * rem2(n, 0x20) as u32;
        fx.mem.write_u8(g, 1);
        fx.mem.write_u32(g + 4, 0);
        words(fx, g + 0x10, D + 0x40);
        for j in 0..8u32 {
            let r = fx.rand_from(S0, ra[0]) >> 23;
            fx.mem.set8(g + 8 + j, rem2(r, 8));
            fx.random_table_vec(g + 0x20 + 0x10 * j, f32::from_bits(0x3ECC_CCCD));
        }
    }
    let d = at(sp, v, 0x2F0);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(d, rh, 1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 5);
    camera(fx, sp, v);
    let (ca, cb, cam, pt) = (at(sp, v, 0x290), at(sp, v, 0x2A0), at(sp, v, 0x250), at(sp, v, 0x2D0));
    fx.mem.write_u32(sp + 0x180, 0);
    for i in 0..0x20u32 {
        let g = D + 0x2900 + 0xA0 * i;
        if fx.mem.read_u8(g) != 0 {
            let age = fx.mem.i32(g + 4);
            let cs = fx.tcos((age << 14) >> 5);
            let al = ee::mul(100.0, cs);
            fx.fix_residue(al, ra[1]);
            let al = ee::fixunssfsi(al);
            colour(fx, sp, v, [0x37, 0xFF, 0xDC], al as u8);
            let age = fx.mem.i32(g + 4);
            let f0 = fx.tan_falloff_p(age, 0x20, 1);
            let s = ee::mul(2.0, ee::add(f32::from_bits(0x3E99_999A), ee::mul(f32::from_bits(0x3F33_3333), f0)));
            fx.mem.setf(ca, s);
            fx.mem.setf(ca + 4, s);
            fx.mem.write_u32(ca + 8, 0);
            fx.mem.write_u32(ca + 0xC, 0x3F80_0000);
            let ns = ee::neg(s);
            fx.mem.setf(cb, ns);
            fx.mem.setf(cb + 4, ns);
            fx.mem.write_u32(cb + 8, 0);
            fx.mem.write_u32(cb + 0xC, 0x3F80_0000);
            fx.vu_apply_to(ca, cam, ca);
            fx.vu_apply_to(cb, cam, cb);
            let age = ee::from_i32(fx.mem.i32(g + 4));
            let f2 = ee::div(ee::mul(f32::from_bits(0x3F33_3333), age), 32.0);
            let f3 = ee::div(age, 32.0);
            let f20 = ee::mul(5.0, f3);
            let f22 = ee::add(f32::from_bits(0x3E99_999A), f2);
            let f21 = ee::mul(-5.0, f3);
            for j in 0..8u32 {
                let b = fx.mem.read_u8(g + 8 + j);
                sprite_uvs(fx, sp, v, b);
                fx.vu_scale4(pt, g + 0x20 + 0x10 * j, f22);
                let y = ee::add(fx.mem.f(pt + 4), f20);
                fx.mem.setf(pt + 4, y);
                let side = ee::from_i32(fx.mem.i32(D + 0xC));
                let z = ee::add(fx.mem.f(pt + 8), ee::mul(f21, side));
                fx.mem.setf(pt + 8, z);
                fx.vu_add4(pt, pt, g + 0x10);
                fx.vu_add4(at(sp, v, 0x2B0), ca, pt);
                fx.vu_add4(at(sp, v, 0x2C0), cb, pt);
                draw(fx, sp, v, d, ra[2]);
            }
            let age = fx.mem.i32(g + 4) + 1;
            fx.mem.set32(g + 4, age);
            if age >= 0x21 {
                fx.mem.write_u8(g, 0);
            }
        }
        fx.mem.write_u32(sp + 0x180, i + 1);
    }
}

fn step(fx: &mut Fx, sp: u32, v: &V) -> i32 {
    fx.camera_turn_roll(at(sp, v, 0x250));
    let nodes = if v.node2.is_some() { 2 } else { 1 };
    for j in 0..nodes {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x30 + 4 * j));
        fx.node_position(h, slot, D + 0x40 + 0x10 * j);
    }
    if let Some(k) = v.z_add {
        let add = if k == 0.0 { 0.0 } else { ee::mul(k, ee::from_i32(fx.mem.i32(D + 0xC))) };
        let z = fx.mem.f(D + 0x48);
        fx.mem.setf(D + 0x48, ee::add(z, add));
    }
    let n = fx.mem.i32(D);
    if v.spawn_at.contains(&n) {
        spawn(fx, v);
    }
    let d = at(sp, v, 0x348);
    fx.mem.write_u16(d + 2, 0x34);
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 0);
    trails(fx, sp, v);
    puffs(fx, sp, v);
    bursts(fx, sp, v);
    big_bursts(fx, sp, v);
    if let Some(ra) = v.glints {
        glints(fx, sp, v, ra);
    }
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == v.one {
        ret = 1;
    } else if (v.six_from..v.two).contains(&n) {
        ret = 6;
    } else if n == v.two {
        ret = 2;
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= v.end {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xB0, ret as u32);
    ret
}

/// b474 (card 456) [F]: the family compiled with
/// one straight trail (no wobble, no pitch: an aim matrix 0x100E40 along
/// the direction) from node 0x29 on updates 0x50 / 0xB8 / 0x11B, 51 long,
/// in a 0x3B0 frame, the data 0x10 lower (no trail index), the rings'
/// sway 0.15 at 0xD05 / 0x15B3 x age, the tube's colour (6C, FF, 80) and
/// alpha 255 (16 - k) >> 4, the texture's s scrolled by -(age % 16) / 16,
/// puffs alpha 2 (32 - age), the burst (8 wide, (C8, FF, DC), 64 - age),
/// three big bursts of 128 sprite pairs (0x890 records, 134 updates, alpha
/// 48 (134 - age) / 134, (82, FF, 96)); returns 1 on 0x79 / 0xE1 / 0x144,
/// 6 on 0x84..0xC6 / 0xEC..0x12E / 0x14F..0x191, 7 on 0xC6 / 0x12E, 2 on
/// 0x191, 8 once the count reaches 0x1D5.
const B474: V = V {
    frame: 0x3B0,
    map: Some(map474),
    ..B578
};

/// b578's stack offsets in b474's frame.
fn map474(off: u32) -> u32 {
    match off {
        0x140..=0x16F => off - 0x10,
        0x1D0..=0x2EF => off - 0x20,
        0x2F0.. => off - 0x30,
        _ => off,
    }
}

/// b474's entry 0x1E00080.
pub fn program_b474(fx: &mut Fx, param: i32) -> i32 {
    let v = &B474;
    let sp = fx.enter(v.frame);
    fx.mem.write_u32(sp + 0xC0, D);
    fx.mem.write_u32(sp + 0xB0, 0);
    let ret = if param >= 0 { init474(fx, sp) } else { step474(fx, sp, v) };
    fx.leave(v.frame);
    ret
}

fn init474(fx: &mut Fx, sp: u32) -> i32 {
    fx.bzero(D, 0x3E50);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    for k in 0..3u32 {
        let r = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x10 + 4 * k, r);
    }
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x29, 1);
    fx.mem.set32(D + 0x30, r);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x20);
    for k in 0..0x11u32 {
        let row = D + 0x44 * k;
        for j in 0..0x10u32 {
            let r = fx.rand_from(k, 0x01E0_0190) >> 23;
            fx.mem.set32(row + 0x1260 + 4 * j, (rem2(r, 256) << 16) >> 8);
        }
        let w = fx.mem.read_u32(row + 0x1260);
        fx.mem.write_u32(row + 0x12A0, w);
    }
    fx.mem.write_u8(D + 0x16F0, 0);
    fx.mem.write_u32(D + 0x16F8, 0);
    for e in 0..0x20u32 {
        fx.mem.write_u8(D + 0x1760 + 0x60 * e, 0);
    }
    fx.mem.write_u8(D + 0x2360, 0);
    fx.mem.write_u32(D + 0x2490, 0);
    for k in 0..3u32 {
        fx.mem.write_u8(D + 0x24A0 + 0x890 * k, 0);
    }
    // The clear loop steps the stack copy of a0.
    fx.mem.write_u32(sp + 0xC0, D + 3 * 0x890);
    0
}

/// The trail [F 0x1E00438]: its tube at the node + the direction x age /
/// 51, a puff entry an update, at its end a big burst.
fn trail474(fx: &mut Fx, sp: u32) {
    let rec = D + 0x16F0;
    let (pt, vp, m) = (sp + 0x2B0, sp + 0x1B0, sp + 0x1F0);
    fx.mem.write_u32(sp + 0x180, 0);
    fx.mem.write_u32(sp + 0x100, D);
    if fx.mem.read_u8(rec) != 0 {
        fx.camera_vp_to(vp);
        let age = fx.mem.i32(rec + 4);
        let f = ee::div(ee::from_i32(age), 51.0);
        fx.vu_scale4(pt, rec + 0x20, f);
        fx.vu_add4(pt, pt, rec + 0x10);
        fx.translate_rows(m, rec + 0x30, pt);
        let mm = fx.read_m4(m);
        fx.mul_into(Some(vp), &mm);
        // The 17 rings of 17 vertices at D+0x50.
        fx.mem.write_u32(sp + 0xD0, 0);
        fx.mem.write_u32(sp + 0xE0, D);
        fx.mem.write_u32(sp + 0xF0, D);
        for k in 0..0x11i32 {
            let c = fx.tcos((k * 0x8000) >> 4);
            let f21 = ee::mul(4.0, c);
            let f20 = Fx::powf2(4.0);
            let f0 = Fx::powf2(f21);
            let radius = fx.sqrtf(ee::sub(f20, f0));
            let s7 = (rem2(k, 8) << 16) >> 3;
            let height = ee::mul(2.0, ee::add(-2.0, f21));
            let row = D + 0x44 * k as u32;
            let out = D + 0x110 * k as u32;
            for j in 0..0x11i32 {
                let age = fx.mem.i32(rec + 4);
                let t = fx.mem.i32(row + 0x1260 + 4 * j as u32);
                let mm = fx.mult(age, 0xD05);
                let a0 = t.wrapping_sub(mm) & 0xFFFF;
                let mm = fx.mult(age, 0x15B3);
                let s1 = s7.wrapping_add((rem2(j, 8) << 16) >> 3).wrapping_sub(mm) & 0xFFFF;
                let sn = fx.tsin(a0);
                let f21 = ee::add(1.0, ee::mul(f32::from_bits(0x3E19_999A), sn));
                let sn = fx.tsin(s1);
                let f0 = ee::add(f21, ee::mul(f32::from_bits(0x3E19_999A), sn));
                let a = j << 12;
                let f21 = ee::mul(radius, f0);
                let sn = fx.tsin(a);
                let p = out + 0x50 + 0x10 * j as u32;
                fx.mem.setf(p, ee::mul(f21, sn));
                let cs = fx.tcos(a);
                fx.mem.setf(p + 4, ee::mul(f21, cs));
                fx.mem.setf(p + 8, height);
                fx.mem.write_u32(p + 0xC, 0x3F80_0000);
            }
            fx.mem.set32(sp + 0xD0, (k + 1) * 0x8000);
            fx.mem.write_u32(sp + 0xE0, row + 0x44);
            fx.mem.write_u32(sp + 0xF0, out + 0x110);
        }
        // The tube's 16 x 16 quads.
        let d = sp + 0x318;
        let age = fx.mem.i32(rec + 4);
        let f20 = ee::div(ee::mul(-1.0, ee::from_i32(rem2(age, 16))), 16.0);
        for k in 0..0x10i32 {
            let f3 = ee::div(ee::from_i32(rem2(k, 16)), 16.0);
            let f2 = ee::div(ee::from_i32(rem2(k, 16) + 1), 16.0);
            let a0 = (255 * (16 - k)) >> 4;
            let a1 = (255 * (16 - (k + 1))) >> 4;
            for (o, a) in [(0x10u32, a0), (0x40, a1)] {
                fx.mem.write_u8(d + o, 0x6C);
                fx.mem.write_u8(d + o + 1, 0xFF);
                fx.mem.write_u8(d + o + 2, 0x80);
                fx.mem.write_u8(d + o + 3, a as u8);
                fx.mem.write_u32(d + o + 4, 0x3F80_0000);
                let (c, q) = (fx.mem.read_u32(d + o), fx.mem.read_u32(d + o + 4));
                fx.mem.write_u32(d + o + 0x18, c);
                fx.mem.write_u32(d + o + 0x1C, q);
            }
            let f21 = ee::add(f3, f20);
            let f22 = ee::add(f2, f20);
            let row0 = D + 0x110 * k as u32;
            let row1 = D + 0x110 * (k + 1) as u32;
            for j in 0..0x10i32 {
                let f2 = ee::div(ee::from_i32(rem2(j, 16)), 16.0);
                let f0 = ee::div(ee::from_i32(rem2(j, 16) + 1), 16.0);
                fx.mem.setf(d + 0x18, f21);
                fx.mem.setf(d + 0x1C, f2);
                fx.mem.setf(d + 0x30, f21);
                fx.mem.setf(d + 0x34, f0);
                fx.mem.setf(d + 0x48, f22);
                fx.mem.setf(d + 0x4C, f2);
                fx.mem.setf(d + 0x60, f22);
                fx.mem.setf(d + 0x64, f0);
                let (p0, p1) = (0x50 + 0x10 * j as u32, 0x50 + 0x10 * (j + 1) as u32);
                let w = sp + 0x3AC;
                let r = fx.project_n(&[row0 + p0, row0 + p1, row1 + p0, row1 + p1], vp, d, w, 0x01E0_0814);
                if r == 1 {
                    let w = fx.mem.f(w);
                    fx.fix_residue(w, 0x01E0_0828);
                    let key = ee::fixunssfsi(w) as i32;
                    fx.submit(d, OT, key);
                }
            }
        }
        let c = fx.mem.i32(rec + 8);
        fx.mem.set32(rec + 8, c + 1);
        if c + 1 >= 0x20 {
            fx.mem.write_u32(rec + 8, 0);
        }
        let e = D + 0x60 * c as u32 + 0x1760;
        fx.mem.write_u8(e, 1);
        fx.mem.write_u32(e + 4, 0);
        words(fx, e + 0x10, pt);
        for q in 0..4u32 {
            let r = fx.rand_from(S0, 0x01E0_08D8) >> 23;
            fx.mem.set8(e + q + 8, rem2(r, 8));
            fx.random_table_vec(e + 0x10 * q + 0x20, f32::from_bits(0x3F4C_CCCD));
        }
        let age = fx.mem.i32(rec + 4) + 1;
        fx.mem.set32(rec + 4, age);
        if age >= 0x34 {
            fx.mem.write_u8(rec, 0);
            let k = fx.mem.i32(D + 0x2490);
            let b = D + 0x890 * k as u32 + 0x24A0;
            fx.mem.write_u8(b, 1);
            fx.mem.write_u32(b + 4, 0);
            for q in 0..0x80u32 {
                let r = fx.rand_from(S0, 0x01E0_0980) >> 23;
                fx.mem.set8(b + q + 8, rem2(r, 8));
                fx.random_table_vec(b + 0x10 * q + 0x90, f32::from_bits(0x3FD9_999A));
            }
            let k = fx.mem.i32(D + 0x2490) + 1;
            fx.mem.set32(D + 0x2490, k);
            if k >= 3 {
                fx.mem.write_u32(D + 0x2490, 0);
            }
        }
    }
    fx.mem.write_u32(sp + 0x100, D + 0xC00);
    fx.mem.write_u32(sp + 0x180, 1);
}

/// The puffs [F 0x1E00A14]: one group of 32 entries.
fn puffs474(fx: &mut Fx, sp: u32, v: &V) {
    let d = sp + 0x2C0;
    let rh = fx.mem.read_u32(D + 0x14);
    sprite_desc(fx, sp, v, rh, Some(1));
    fx.mem.write_u32(sp + 0x170, 0);
    fx.mem.write_u32(sp + 0x120, D);
    fx.mem.write_u32(sp + 0x190, 0);
    fx.mem.write_u32(sp + 0x110, D);
    fx.mem.write_u32(sp + 0x1A0, 0);
    for e in 0..0x20u32 {
        let s4 = D + 0x60 * e;
        if fx.mem.read_u8(s4 + 0x1760) != 0 {
            let age = fx.mem.i32(s4 + 0x1764);
            let s = half_size(age, 32.0, 10.0);
            corners(fx, sp, v, s);
            let age = fx.mem.i32(s4 + 0x1764);
            let a = ((0x20 - age) << 6) >> 5;
            colour(fx, sp, v, [0x6C, 0xFF, 0x80], a as u8);
            let age = fx.mem.i32(s4 + 0x1764);
            let f20 = ee::add(f32::from_bits(0x3E4C_CCCD), ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), ee::from_i32(age)), 32.0));
            for q in 0..4u32 {
                let b = fx.mem.read_u8(s4 + 0x1768 + q);
                sprite_uvs(fx, sp, v, b);
                sprite(fx, sp, v, s4 + 0x1780 + 0x10 * q, f20, s4 + 0x1770);
                draw(fx, sp, v, d, 0x01E0_0D10);
            }
            let age = fx.mem.i32(s4 + 0x1764) + 1;
            fx.mem.set32(s4 + 0x1764, age);
            if age >= 0x21 {
                fx.mem.write_u8(s4 + 0x1760, 0);
            }
        }
        fx.mem.write_u32(sp + 0x1A0, e + 1);
    }
    fx.mem.write_u32(sp + 0x110, D + 0xC00);
    fx.mem.write_u32(sp + 0x190, 1);
    fx.mem.write_u32(sp + 0x120, D + 0xC00);
    fx.mem.write_u32(sp + 0x170, 1);
}

/// The burst [F 0x1E00DBC]: 16 sprites.
fn burst474(fx: &mut Fx, sp: u32, v: &V) {
    let d = sp + 0x2C0;
    let rh = fx.mem.read_u32(D + 0x18);
    sprite_desc(fx, sp, v, rh, Some(1));
    fx.mem.write_u32(sp + 0x160, 0);
    let s4 = D;
    if fx.mem.read_u8(s4 + 0x2360) != 0 {
        let age = fx.mem.i32(s4 + 0x2364);
        let s = half_size(age, 64.0, 8.0);
        corners(fx, sp, v, s);
        let age = fx.mem.i32(s4 + 0x2364);
        colour(fx, sp, v, [0xC8, 0xFF, 0xDC], (0x40 - age) as u8);
        let age = fx.mem.i32(s4 + 0x2364);
        let f0 = fx.tan_falloff_p(age, 0x40, 1);
        let f20 = ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), f0));
        for j in 0..0x10u32 {
            let b = fx.mem.read_u8(s4 + 0x2368 + j);
            sprite_uvs(fx, sp, v, b);
            sprite(fx, sp, v, s4 + 0x2390 + 0x10 * j, f20, s4 + 0x2380);
            draw(fx, sp, v, d, 0x01E0_1078);
        }
        let age = fx.mem.i32(s4 + 0x2364) + 1;
        fx.mem.set32(s4 + 0x2364, age);
        if age >= 0x41 {
            fx.mem.write_u8(s4 + 0x2360, 0);
        }
    }
    fx.mem.write_u32(sp + 0x160, 1);
}

/// The big bursts [F 0x1E010D8]: three records of 128 sprite pairs round
/// the target, 134 updates.
fn bigs474(fx: &mut Fx, sp: u32, v: &V) {
    let d = sp + 0x2C0;
    let rh = fx.mem.read_u32(D + 0x18);
    sprite_desc(fx, sp, v, rh, Some(1));
    let (pt, va, vb, ca, cb) = (sp + 0x2B0, sp + 0x290, sp + 0x2A0, sp + 0x270, sp + 0x280);
    for m in 0..3u32 {
        let s4 = D + 0x890 * m;
        if fx.mem.read_u8(s4 + 0x24A0) == 0 {
            continue;
        }
        let age = fx.mem.i32(s4 + 0x24A4);
        let s = half_size(age, 134.0, 13.0);
        corners(fx, sp, v, s);
        let age = fx.mem.i32(s4 + 0x24A4);
        let x = (0x86 - age) * 48;
        fx.mult(0x07A4_4C6B, x);
        let hi = fx.hilo().1;
        let a = (hi >> 2).wrapping_add(((x as u32) >> 31) as i32);
        colour(fx, sp, v, [0x82, 0xFF, 0x96], a as u8);
        let age = fx.mem.i32(s4 + 0x24A4);
        let f0 = fx.tan_falloff_p(age, 0x86, 1);
        let f21 = ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), f0));
        let age = fx.mem.i32(s4 + 0x24A4);
        let f20 = ee::mul(50.0, ee::div(ee::from_i32(age), 134.0));
        for j in 0..0x80u32 {
            let rh = fx.mem.read_u32(D + 0x14);
            fx.desc_texture(d, rh, 1);
            let b = fx.mem.read_u8(s4 + 0x24A8 + j);
            sprite_uvs(fx, sp, v, b);
            sprite(fx, sp, v, s4 + 0x2530 + 0x10 * j, f21, D + 0x20);
            draw(fx, sp, v, d, 0x01E0_13E8);
            let rh = fx.mem.read_u32(D + 0x18);
            fx.desc_texture(d, rh, 1);
            let y = fx.mem.f(pt + 4);
            fx.mem.setf(pt + 4, ee::add(y, f20));
            fx.vu_add4(va, ca, pt);
            fx.vu_add4(vb, cb, pt);
            draw(fx, sp, v, d, 0x01E0_1468);
        }
        let age = fx.mem.i32(s4 + 0x24A4) + 1;
        fx.mem.set32(s4 + 0x24A4, age);
        if age >= 0x87 {
            fx.mem.write_u8(s4 + 0x24A0, 0);
        }
    }
}

fn step474(fx: &mut Fx, sp: u32, v: &V) -> i32 {
    fx.camera_turn_roll(sp + 0x230);
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x30));
    fx.node_position(h, slot, D + 0x40);
    let n = fx.mem.i32(D);
    if [0x50, 0xB8, 0x11B].contains(&n) {
        let rec = D + 0x16F0;
        fx.mem.write_u8(rec, 1);
        fx.mem.write_u32(rec + 4, 0);
        words(fx, rec + 0x10, D + 0x40);
        fx.vu_sub4(rec + 0x20, D + 0x20, D + 0x40);
        fx.point_along(rec + 0x30, rec + 0x20);
        let b = D + 0x2360;
        fx.mem.write_u8(b, 1);
        fx.mem.write_u32(b + 4, 0);
        words(fx, b + 0x20, D + 0x40);
        for j in 0..0x10u32 {
            let r = fx.rand_from(S0, 0x01E0_0380) >> 23;
            fx.mem.set8(b + j + 8, rem2(r, 8));
            fx.random_table_vec(b + 0x10 * j + 0x30, 0.5);
        }
    }
    let d = sp + 0x318;
    fx.mem.write_u16(d + 2, 0x34);
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 0);
    trail474(fx, sp);
    puffs474(fx, sp, v);
    burst474(fx, sp, v);
    bigs474(fx, sp, v);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if [0x79, 0xE1, 0x144].contains(&n) {
        ret = 1;
    } else {
        if (0x84..0xC6).contains(&n) || (0xEC..0x12E).contains(&n) || (0x14F..0x191).contains(&n) {
            ret = 6;
        }
        if n == 0xC6 || n == 0x12E {
            ret = 7;
        }
        if n == 0x191 {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x1D5 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xB0, ret as u32);
    ret
}
