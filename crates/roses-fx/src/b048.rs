//! BMODULE b048 (card 396), its near-twin b204 (card 561) and b622 (card
//! 644: eight trails spawned every sixteenth update 0x70..0x100, puff
//! groups +0x1A80, bursts +0x7A80, big bursts +0x8410, phase (i << 16) >>
//! 3, colour (64, FF, 5A), returns at 0xA2 / 0xAD.. / 0x180 / 0x182),
//! hand-ported
//! from the records (linked at 0x1E00000, one function 0x1E00080, data
//! area a0 = 0x1ED0000, 0xCB10 bytes) [F]: a
//! single unscheduled function in a 0x410 frame on b150's helpers. Every
//! update the camera turn with roll (0x2055E0 into at(sp, v, 0x270)) and the
//! attacker's node 0x1D (b204 0x46) into D+0x40. On updates 0x95 / 0xBA /
//! 0xD8 / 0xF6 / 0x10D (b204: 0x48 / 0x50 / 0x58 / 0x60) one of ten
//! trail records (+0x1700, 0x70) starts at the node, aimed at a random
//! point (0x101080, 1.0) round the target, with one of ten burst records
//! (+0x9360, 0x130: 16 cells and 16 random directions of 0.7) at the same
//! point. A live trail record (64 updates, b204 32) draws a tube of 16 x
//! 16 textured quads (descriptor at(sp, v, 0x378), prim 0x34, 12 registers,
//! colour (FF, DC, 64) (b204 (64, 78, FF)) fading along the rings) on 17
//! rings of 17 vertices (D+0x60, 0x110 a ring) - radius 4 |sin|, height 8
//! cos - 4 along a half turn, each vertex swayed by two table sines of a
//! random angle table (D+0x1270, 17 x 17) - placed at the trail's point
//! (the direction x age / 64 (b204 / 32) plus a 20 sin(age << 9) wobble
//! (b204 none)) and turned about x by the pitch to the next point
//! (atan2f, 0x100E00); every update it leaves one of 32 puff entries
//! (+0x1B60, 0x60: 4 cells, 4 directions of 0.8) in its group, and at its
//! end one of ten big bursts (+0x9F50, 0x460: 64 cells, 64 directions of
//! 1.7). Then the sprites (descriptor at(sp, v, 0x320), 5 registers 0x42421,
//! prim 0x26 | 0x800, clamp 5): the puffs (32 updates, texture D+0x14,
//! half size 10 (0.6 + 0.4 age / 32), alpha 80 (32 - age) >> 5, spread
//! 0.2 + 0.8 age / 32), the bursts (64 updates, texture D+0x18, half size
//! 10 (0.6 + 0.4 age / 64), alpha 64 - age, spread 0.2 + 0.8 falloff)
//! and the big bursts (64 updates, two sprites each - texture D+0x14 at
//! the point and D+0x18 raised 50 age / 64 - half size 13, alpha 50 (64
//! - age) >> 6). Returns 1 on update 0xCB, 6 on 0xD6..0x18C, 2 on 0x18D,
//! 8 once the count reaches 0x18F (b204 0x5B / 0x66..0xC4 / 0xC5 / 0xC7).
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
/// The entry's s0 (0x206E40's return), rand's saved s0 in the step.
const S0: u32 = OT;
const TWO_PI: f32 = f32::from_bits(0x40C9_0FDB);
/// b022's lens flag (u8) / age at D+0xCB30 / 0xCB34.
const LENS: u32 = 0xCB30;
/// The fan's nine screen points (exe rodata, 0x10 apart).
const FAN: u32 = 0x0028_ABE0;

/// What differs between the two records.
struct V {
    frame: u32,
    /// Stack offsets: (from, shift) runs; an offset at or above `from`
    /// sits `shift` higher than b048's (b639 / b604: 0x3F0 frame).
    sp_map: &'static [(u32, i32)],
    bzero: u32,
    node: i32,
    /// Trail records (= puff groups = bursts = big bursts).
    n: i32,
    /// The trail's spawn updates (b048) or the window (b204: 0x46..0x64,
    /// every eighth update; b622 0x65..0x10D, every sixteenth).
    spawn_at: &'static [i32],
    spawn_window: Option<(i32, i32, i32)>,
    /// The rings' phase: (i << 16) / this (the compiled multiply; 8 / 4 a
    /// shift).
    phase_div: i32,
    /// The trail aimed straight at the target (b639 / b604; else at a
    /// random point round it): no point kept for the big burst, which
    /// then sits at the target.
    straight: bool,
    /// The trail's turn: 0x100E40 along the step to the next point
    /// (b639), else the pitch to it (atan2f, 0x100E00).
    turn_matrix: bool,
    /// b604: record k's life +0x170C (the point's divisor, the wobble
    /// (age << 15) / life, ending once the age passes it), and record 1
    /// (and its burst) lowered 5 x the side in z.
    lives: &'static [i32],
    /// D+0x48 += 0.0 every update.
    z_add: bool,
    /// The big burst records' size and the offset of their directions.
    big_size: u32,
    big_dirs: u32,
    /// The tube's scroll: -(age % this) / this.
    tube_mod: i32,
    /// Data offsets: the puff groups, the bursts, the big burst index and
    /// the big bursts.
    puffs: u32,
    bursts: u32,
    big_index: u32,
    bigs: u32,
    /// The direction's divisor (128 / 32) and the wobble (10 sin(age <<
    /// shift), none for b204).
    divisor: f32,
    wobble: bool,
    /// The trail's life (0x41 / 0x21).
    life: i32,
    /// The tube's colour; the puffs' (alpha `puff_alpha` x (32 - age) >>
    /// 5), the bursts' (alpha (64 - age) x `burst_alpha`), the big
    /// bursts' two (alpha `big_alpha` x (64 - age) >> 6).
    colour: [u8; 3],
    puff_colour: [u8; 3],
    puff_alpha: i32,
    burst_colour: [u8; 3],
    burst_alpha: i32,
    big_colours: [[u8; 3]; 2],
    big_alpha: i32,
    /// b022: data offsets from 0x20 up sit this much higher (D+0x20..0x3F
    /// the attacker's screen point), the clear loop leaves the stack copy
    /// of a0 alone, and the screen point / lens / stage light run.
    dshift: u32,
    lens: bool,
    /// The spawn's random point spread (0x101080), the puff entry's
    /// direction length, the rings' radius / height offset, the puffs'
    /// half size, the bursts' length (life - 1, the size divisor, the tan
    /// falloff's n, alpha (n - age) x burst_alpha).
    spread: f32,
    puff_dir: u32,
    ring_r: f32,
    ring_h: f32,
    puff_size: f32,
    burst_n: i32,
    /// Returns: 1 at, 6 from, 2 at (6 below), 8 once the count reaches.
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

const B048: V = V {
    frame: 0x410,
    sp_map: &[(0, 0)],
    bzero: 0xCB10,
    node: 0x1D,
    n: 10,
    spawn_at: &[0x95, 0xBA, 0xD8, 0xF6, 0x10D],
    spawn_window: None,
    phase_div: 10,
    straight: false,
    turn_matrix: false,
    lives: &[],
    z_add: true,
    big_size: 0x460,
    big_dirs: 0x60,
    tube_mod: 24,
    puffs: 0x1B60,
    bursts: 0x9360,
    big_index: 0x9F40,
    bigs: 0x9F50,
    divisor: 64.0,
    wobble: true,
    life: 0x41,
    colour: [0xFF, 0xDC, 0x64],
    puff_colour: [0xFF, 0xDC, 0x64],
    puff_alpha: 80,
    burst_colour: [0xFF, 0xDC, 0x64],
    burst_alpha: 1,
    big_colours: [[0xFF, 0xDC, 0x64], [0xFF, 0xDC, 0x64]],
    big_alpha: 50,
    dshift: 0,
    lens: false,
    spread: 1.0,
    puff_dir: 0x3F4C_CCCD,
    ring_r: 4.0,
    ring_h: -2.0,
    puff_size: 10.0,
    burst_n: 0x40,
    one: 0xCB,
    six_from: 0xD6,
    two: 0x18D,
    end: 0x18F,
    r_burst: 0x01E0_042C,
    r_pitch: 0x01E0_0638,
    r_proj: 0x01E0_0A44,
    r_tube: 0x01E0_0A58,
    r_puff: 0x01E0_0B08,
    r_big: 0x01E0_0BF0,
    r_puffs: 0x01E0_0FC8,
    r_bursts: 0x01E0_1364,
    r_bigs: [0x01E0_1738, 0x01E0_17F0],
};

const B204: V = V {
    frame: 0x410,
    sp_map: &[(0, 0)],
    bzero: 0xCB10,
    node: 0x46,
    n: 10,
    spawn_at: &[],
    spawn_window: Some((0x46, 0x65, 8)),
    phase_div: 10,
    straight: false,
    turn_matrix: false,
    lives: &[],
    z_add: true,
    big_size: 0x460,
    big_dirs: 0x60,
    tube_mod: 24,
    puffs: 0x1B60,
    bursts: 0x9360,
    big_index: 0x9F40,
    bigs: 0x9F50,
    divisor: 32.0,
    wobble: false,
    life: 0x21,
    colour: [0x64, 0x78, 0xFF],
    puff_colour: [0x64, 0x78, 0xFF],
    puff_alpha: 80,
    burst_colour: [0x64, 0x78, 0xFF],
    burst_alpha: 1,
    big_colours: [[0x64, 0x78, 0xFF], [0x64, 0x78, 0xFF]],
    big_alpha: 50,
    dshift: 0,
    lens: false,
    spread: 1.0,
    puff_dir: 0x3F4C_CCCD,
    ring_r: 4.0,
    ring_h: -2.0,
    puff_size: 10.0,
    burst_n: 0x40,
    one: 0x5B,
    six_from: 0x66,
    two: 0xC5,
    end: 0xC7,
    r_burst: 0x01E0_0424,
    r_pitch: 0x01E0_0620,
    r_proj: 0x01E0_0A2C,
    r_tube: 0x01E0_0A40,
    r_puff: 0x01E0_0AF0,
    r_big: 0x01E0_0BD8,
    r_puffs: 0x01E0_0FB0,
    r_bursts: 0x01E0_134C,
    r_bigs: [0x01E0_1720, 0x01E0_17D8],
};

const B622: V = V {
    frame: 0x410,
    sp_map: &[(0, 0)],
    bzero: 0xA710,
    node: 0x12,
    n: 8,
    spawn_at: &[],
    spawn_window: Some((0x65, 0x10E, 16)),
    phase_div: 8,
    straight: false,
    turn_matrix: false,
    lives: &[],
    z_add: true,
    big_size: 0x460,
    big_dirs: 0x60,
    tube_mod: 24,
    puffs: 0x1A80,
    bursts: 0x7A80,
    big_index: 0x8400,
    bigs: 0x8410,
    divisor: 64.0,
    wobble: true,
    life: 0x41,
    colour: [0x64, 0xFF, 0x5A],
    puff_colour: [0x64, 0xFF, 0x5A],
    puff_alpha: 80,
    burst_colour: [0x64, 0xFF, 0x5A],
    burst_alpha: 1,
    big_colours: [[0x64, 0xFF, 0x5A], [0x64, 0xFF, 0x5A]],
    big_alpha: 50,
    dshift: 0,
    lens: false,
    spread: 1.0,
    puff_dir: 0x3F4C_CCCD,
    ring_r: 4.0,
    ring_h: -2.0,
    puff_size: 10.0,
    burst_n: 0x40,
    one: 0xA2,
    six_from: 0xAD,
    two: 0x180,
    end: 0x182,
    r_burst: 0x01E0_040C,
    r_pitch: 0x01E0_060C,
    r_proj: 0x01E0_09FC,
    r_tube: 0x01E0_0A10,
    r_puff: 0x01E0_0AC0,
    r_big: 0x01E0_0BA8,
    r_puffs: 0x01E0_0F80,
    r_bursts: 0x01E0_12E8,
    r_bigs: [0x01E0_1698, 0x01E0_1750],
};

const B639: V = V {
    frame: 0x3F0,
    sp_map: &[(0, 0), (0x160, -0x10), (0x320, -0x20)],
    bzero: 0x4CE0,
    node: 0x12,
    n: 3,
    spawn_at: &[0x73, 0x87, 0x9B],
    spawn_window: None,
    phase_div: 3,
    straight: true,
    turn_matrix: true,
    lives: &[],
    z_add: false,
    big_size: 0x450,
    big_dirs: 0x50,
    tube_mod: 16,
    puffs: 0x1850,
    bursts: 0x3C50,
    big_index: 0x3FE0,
    bigs: 0x3FF0,
    divisor: 64.0,
    wobble: true,
    life: 0x41,
    colour: [0xFF, 0x9B, 0x37],
    puff_colour: [0xC8, 0x64, 0x37],
    puff_alpha: 90,
    burst_colour: [0x80, 0x6C, 0x58],
    burst_alpha: 2,
    big_colours: [[0xC8, 0x64, 0x37], [0x80, 0x6C, 0x58]],
    big_alpha: 100,
    dshift: 0,
    lens: false,
    spread: 1.0,
    puff_dir: 0x3F4C_CCCD,
    ring_r: 4.0,
    ring_h: -2.0,
    puff_size: 10.0,
    burst_n: 0x40,
    one: 0xA9,
    six_from: 0xB4,
    two: 0x11B,
    end: 0x11D,
    r_burst: 0x01E0_03B0,
    r_pitch: 0,
    r_proj: 0x01E0_0964,
    r_tube: 0x01E0_0978,
    r_puff: 0x01E0_0A28,
    r_big: 0x01E0_0AD0,
    r_puffs: 0x01E0_0E80,
    r_bursts: 0x01E0_11E8,
    r_bigs: [0x01E0_1560, 0x01E0_1618],
};

const B604: V = V {
    frame: 0x3F0,
    sp_map: &[(0, 0), (0x160, -0x10), (0x320, -0x20)],
    bzero: 0x5ED0,
    node: 0x46,
    n: 4,
    spawn_at: &[0x86, 0x95, 0xA4, 0xB3],
    spawn_window: None,
    phase_div: 4,
    straight: true,
    turn_matrix: false,
    lives: &[0x5F, 0x67, 0x71, 0x7A],
    z_add: true,
    big_size: 0x450,
    big_dirs: 0x50,
    tube_mod: 24,
    puffs: 0x18C0,
    bursts: 0x48C0,
    big_index: 0x4D80,
    bigs: 0x4D90,
    divisor: 64.0,
    wobble: true,
    life: 0,
    colour: [0xC8, 0x80, 0x5A],
    puff_colour: [0xC8, 0x80, 0x5A],
    puff_alpha: 80,
    burst_colour: [0xC8, 0x80, 0x5A],
    burst_alpha: 1,
    big_colours: [[0xC8, 0x80, 0x5A], [0xC8, 0x80, 0x5A]],
    big_alpha: 50,
    dshift: 0,
    lens: false,
    spread: 1.0,
    puff_dir: 0x3F4C_CCCD,
    ring_r: 4.0,
    ring_h: -2.0,
    puff_size: 10.0,
    burst_n: 0x40,
    one: 0xDB,
    six_from: 0xE6,
    two: 0x16D,
    end: 0x16F,
    r_burst: 0x01E0_0498,
    r_pitch: 0x01E0_06B8,
    r_proj: 0x01E0_0A9C,
    r_tube: 0x01E0_0AB0,
    r_puff: 0x01E0_0B60,
    r_big: 0x01E0_0C08,
    r_puffs: 0x01E0_0FB0,
    r_bursts: 0x01E0_1318,
    r_bigs: [0x01E0_1690, 0x01E0_1748],
};

/// b022 (card 228): b204 with the data from 0x20 up 0x20 higher, a 0x4B0
/// frame, node 0x66, spawns every eighth update 0x3C..0xCC spread 1.5,
/// rings of radius 3 |sin| (height 3 cos - 1.5 doubled), colour (70, FF,
/// 60), puffs of half size 8 thrown 0.7, bursts over 16 updates (alpha
/// (16 - age) x 4), and every update the attacker's screen point, from
/// 0x14 for 32 updates a packet-layer fan, and the stage light [F].
const B022: V = V {
    frame: 0x4B0,
    sp_map: &[(0, 0), (0xB0, 0x10), (0xC0, -4), (0xD0, 0x10), (0x190, 0x20), (0x40C, 0xA0)],
    bzero: 0xCB40,
    node: 0x66,
    spawn_window: Some((0x3C, 0xCD, 8)),
    colour: [0x70, 0xFF, 0x60],
    puff_colour: [0x70, 0xFF, 0x60],
    burst_colour: [0x70, 0xFF, 0x60],
    burst_alpha: 4,
    big_colours: [[0x70, 0xFF, 0x60], [0x70, 0xFF, 0x60]],
    dshift: 0x20,
    lens: true,
    spread: 1.5,
    puff_dir: 0x3F33_3333,
    ring_r: 3.0,
    ring_h: -1.5,
    puff_size: 8.0,
    burst_n: 0x10,
    one: 0x51,
    six_from: 0x5C,
    two: 0x12D,
    end: 0x14E,
    r_burst: 0x01E0_0560,
    r_pitch: 0x01E0_075C,
    r_proj: 0x01E0_0B6C,
    r_tube: 0x01E0_0B80,
    r_puff: 0x01E0_0C30,
    r_big: 0x01E0_0D18,
    r_puffs: 0x01E0_10F0,
    r_bursts: 0x01E0_148C,
    r_bigs: [0x01E0_1860, 0x01E0_1918],
    ..B204
};

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B048)
}

/// b204's entry 0x1E00080.
pub fn program_b204(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B204)
}

/// b622's entry 0x1E00080.
pub fn program_b622(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B622)
}

/// b639's entry 0x1E00080.
pub fn program_b639(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B639)
}

/// b604's entry 0x1E00080.
pub fn program_b604(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B604)
}

/// b022's entry 0x1E00080.
pub fn program_b022(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B022)
}

fn run(fx: &mut Fx, param: i32, v: &V) -> i32 {
    let sp = fx.enter(v.frame);
    fx.mem.write_u32(at(sp, v, 0xC0), D);
    fx.mem.write_u32(at(sp, v, 0xB0), 0);
    let ret = if param >= 0 { init(fx, sp, v) } else { step(fx, sp, v) };
    fx.leave(v.frame);
    ret
}

/// The data from 0x20 up (b022: 0x20 higher).
fn e(v: &V) -> u32 {
    D + v.dshift
}

/// A stack local: b048's offset mapped to the record's.
fn at(sp: u32, v: &V, off: u32) -> u32 {
    let shift = v.sp_map.iter().filter(|(from, _)| *from <= off).last().map_or(0, |(_, s)| *s);
    (sp + off).wrapping_add_signed(shift)
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
    for (k, id) in [0u32, 1, 2].into_iter().enumerate() {
        let r = fx.texture(5, id as i32);
        fx.mem.write_u32(D + 0x10 + 4 * k as u32, r);
    }
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, v.node, 1);
    fx.mem.set32(e(v) + 0x30, r);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, e(v) + 0x20);
    fx.mem.write_u32(e(v) + 0x50, 0);
    // The angle table: 17 rows of 16 random angles ((rand >> 23) % 256 <<
    // 8) and the first again (rand's saved s0 = the row).
    for k in 0..0x11u32 {
        let row = e(v) + 0x44 * k;
        for j in 0..0x10u32 {
            let r = fx.rand_from(k, 0x01E0_0198) >> 23;
            fx.mem.set32(row + 0x1270 + 4 * j, (rem2(r, 256) << 16) >> 8);
        }
        let w = fx.mem.read_u32(row + 0x1270);
        fx.mem.write_u32(row + 0x12B0, w);
    }
    let n = v.n as u32;
    for k in 0..n {
        fx.mem.write_u8(e(v) + 0x1700 + 0x70 * k, 0);
        fx.mem.write_u32(e(v) + 0x1708 + 0x70 * k, 0);
    }
    for g in 0..n {
        for e in 0..0x20u32 {
            fx.mem.write_u8(self::e(v) + v.puffs + 0xC00 * g + 0x60 * e, 0);
        }
    }
    for k in 0..n {
        fx.mem.write_u8(e(v) + v.bursts + 0x130 * k, 0);
    }
    fx.mem.write_u32(e(v) + v.big_index, 0);
    for k in 0..n {
        fx.mem.write_u8(e(v) + v.bigs + v.big_size * k, 0);
    }
    if v.lens {
        fx.mem.write_u8(D + LENS, 0);
    } else {
        // The clear loop steps the stack copy of a0.
        fx.mem.write_u32(at(sp, v, 0xC0), D + v.big_size * n);
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

/// The sprite cell at at(sp, v, 0x338) / 0x33A / 0x348 / 0x34A.
fn sprite_uvs(fx: &mut Fx, sp: u32, v: &V, b: u8) {
    let (u0, v0, u1, v1) = cell(b);
    uv(fx, at(sp, v, 0x338), u0);
    uv(fx, at(sp, v, 0x33A), v0);
    uv(fx, at(sp, v, 0x348), u1);
    uv(fx, at(sp, v, 0x34A), v1);
}

/// The view-projection (x identity) at at(sp, v, 0x1F0).
fn camera(fx: &mut Fx, sp: u32, v: &V) {
    fx.camera_vp_to(at(sp, v, 0x1F0));
    fx.identity(at(sp, v, 0x230));
    let id = fx.read_m4(at(sp, v, 0x230));
    fx.mul_into(Some(at(sp, v, 0x1F0)), &id);
}

/// The sprite's corners (-s, -s, 0, 1) at at(sp, v, 0x2B0) and (s, s, 0, 1) at
/// at(sp, v, 0x2C0) turned by the camera (at(sp, v, 0x270)).
fn corners(fx: &mut Fx, sp: u32, v: &V, s: f32) {
    let n = ee::neg(s);
    fx.mem.setf(at(sp, v, 0x2B0), n);
    fx.mem.setf(at(sp, v, 0x2B4), n);
    fx.mem.write_u32(at(sp, v, 0x2B8), 0);
    fx.mem.write_u32(at(sp, v, 0x2BC), 0x3F80_0000);
    fx.mem.setf(at(sp, v, 0x2C0), s);
    fx.mem.setf(at(sp, v, 0x2C4), s);
    fx.mem.write_u32(at(sp, v, 0x2C8), 0);
    fx.mem.write_u32(at(sp, v, 0x2CC), 0x3F80_0000);
    fx.vu_apply_to(at(sp, v, 0x2B0), at(sp, v, 0x270), at(sp, v, 0x2B0));
    fx.vu_apply_to(at(sp, v, 0x2C0), at(sp, v, 0x270), at(sp, v, 0x2C0));
}

/// `m` x (0.6 + 0.4 age / `d`).
fn half_size(age: i32, d: f32, m: f32) -> f32 {
    let f = ee::div(ee::mul(f32::from_bits(0x3ECC_CCCD), ee::from_i32(age)), d);
    ee::mul(m, ee::add(f32::from_bits(0x3F19_999A), f))
}

/// The sprite colour at at(sp, v, 0x330) (Q = 1.0).
fn colour(fx: &mut Fx, sp: u32, v: &V, c: [u8; 3], a: u8) {
    fx.mem.write_u8(at(sp, v, 0x330), c[0]);
    fx.mem.write_u8(at(sp, v, 0x331), c[1]);
    fx.mem.write_u8(at(sp, v, 0x332), c[2]);
    fx.mem.write_u8(at(sp, v, 0x333), a);
    fx.mem.write_u32(at(sp, v, 0x334), 0x3F80_0000);
}

/// The sprite descriptor at at(sp, v, 0x320) (5 registers 0x42421, prim 0x26 |
/// 0x800) with its pointers at at(sp, v, 0x160) / 0x170 / 0x180, alpha preset
/// 1, texture `rh`, clamp 5, and the camera.
fn sprite_desc(fx: &mut Fx, sp: u32, v: &V, rh: u32) {
    let d = at(sp, v, 0x320);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u32(at(sp, v, 0x160), d + 2);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u32(at(sp, v, 0x170), d + 8);
    fx.mem.write_u32(at(sp, v, 0x180), d + 4);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp, v);
}

/// Projected (0x204F70 into 0x2D3600 through at(sp, v, 0x1F0)) and submitted at
/// the mean w.
fn draw(fx: &mut Fx, sp: u32, v: &V, d: u32, ra: u32) {
    if crate::fxlib2::project(fx, at(sp, v, 0x2D0), PROJ_OUT, at(sp, v, 0x1F0), d, at(sp, v, 0x40C)) {
        let w = fx.mem.f(at(sp, v, 0x40C));
        fx.fix_residue(w, ra);
        let key = ee::fixunssfsi(w) as i32;
        fx.submit(d, OT, key);
    }
}

/// A sprite at `p` (the direction `dir` x `f` plus the record's point `at`):
/// the two corners into at(sp, v, 0x2D0) / 0x2E0 and drawn.
fn sprite(fx: &mut Fx, sp: u32, v: &V, dir: u32, f: f32, base: u32) {
    fx.vu_scale4(at(sp, v, 0x2F0), dir, f);
    fx.vu_add4(at(sp, v, 0x2F0), at(sp, v, 0x2F0), base);
    fx.vu_add4(at(sp, v, 0x2D0), at(sp, v, 0x2B0), at(sp, v, 0x2F0));
    fx.vu_add4(at(sp, v, 0x2E0), at(sp, v, 0x2C0), at(sp, v, 0x2F0));
}

/// A trail record started at the node [F 0x1E0032C].
fn spawn(fx: &mut Fx, sp: u32, v: &V) {
    let k = fx.mem.i32(e(v) + 0x50);
    let rec = e(v) + 0x70 * k as u32;
    fx.mem.write_u8(rec + 0x1700, 1);
    fx.mem.write_u32(rec + 0x1704, 0);
    if let Some(&life) = v.lives.get(k as usize) {
        fx.mem.set32(rec + 0x170C, life);
    }
    words(fx, rec + 0x1710, e(v) + 0x40);
    if v.straight {
        if !v.lives.is_empty() && k == 1 {
            side_z(fx, rec + 0x1718);
        }
        fx.vu_sub4(rec + 0x1720, e(v) + 0x20, e(v) + 0x40);
    } else {
        fx.random_table_vec(at(sp, v, 0x2F0), v.spread);
        fx.vu_add4(at(sp, v, 0x2F0), at(sp, v, 0x2F0), e(v) + 0x20);
        fx.vu_sub4(rec + 0x1720, at(sp, v, 0x2F0), e(v) + 0x40);
    }
    let b = e(v) + 0x130 * k as u32 + v.bursts;
    fx.mem.write_u8(b, 1);
    fx.mem.write_u32(b + 4, 0);
    words(fx, b + 0x20, e(v) + 0x40);
    if !v.lives.is_empty() && k == 1 {
        side_z(fx, b + 0x28);
    }
    for j in 0..0x10u32 {
        let r = fx.rand_from(S0, v.r_burst) >> 23;
        fx.mem.set8(b + j + 8, rem2(r, 8));
        fx.random_table_vec(b + 0x10 * j + 0x30, f32::from_bits(0x3F33_3333));
    }
    let k = fx.mem.i32(e(v) + 0x50) + 1;
    fx.mem.set32(e(v) + 0x50, k);
    if k >= v.n {
        fx.mem.write_u32(e(v) + 0x50, 0);
    }
}

/// z += -5 x the side (D+0xC).
fn side_z(fx: &mut Fx, p: u32) {
    let side = ee::from_i32(fx.mem.i32(D + 0xC));
    let z = ee::add(fx.mem.f(p), ee::mul(-5.0, side));
    fx.mem.setf(p, z);
}

/// The trail's point at `age`: the direction x age / divisor (plus the
/// wobble) into `out`.
fn point(fx: &mut Fx, rec: u32, age: i32, out: u32, v: &V) {
    let a = if v.lives.is_empty() {
        let f = ee::div(ee::from_i32(age), v.divisor);
        fx.vu_scale4(out, rec + 0x1720, f);
        (age << 15) >> 6
    } else {
        let life = fx.mem.i32(rec + 0x170C);
        let f = ee::div(ee::from_i32(age), ee::from_i32(life));
        fx.vu_scale4(out, rec + 0x1720, f);
        let life = fx.mem.i32(rec + 0x170C);
        fx.div(age << 15, life).0
    };
    let s = fx.tsin(a & 0xFFFF);
    let y = fx.mem.f(out + 4);
    let y = if v.wobble { ee::add(y, ee::mul(20.0, s)) } else { ee::add(y, 0.0) };
    fx.mem.setf(out + 4, y);
}

/// The tube's 17 rings of 17 vertices at D+0x60 [F 0x1E006A8].
fn rings(fx: &mut Fx, sp: u32, v: &V, i: i32, age: i32) {
    fx.mem.write_u32(at(sp, v, 0xD0), 0);
    fx.mem.write_u32(at(sp, v, 0xE0), D);
    fx.mem.write_u32(at(sp, v, 0xF0), D);
    let x = i << 16;
    let phase = match v.phase_div {
        10 => {
            fx.mult(0x6666_6667, x);
            let hi = fx.hilo().1;
            (hi >> 2).wrapping_add(((x as u32) >> 31) as i32)
        }
        3 => {
            fx.mult(0x5555_5556, x);
            let hi = fx.hilo().1;
            hi.wrapping_add(((x as u32) >> 31) as i32)
        }
        4 => x >> 2,
        _ => x >> 3,
    };
    fx.mem.set32(at(sp, v, 0x100), phase);
    for k in 0..0x11i32 {
        let c = fx.tcos((k * 0x8000) >> 4);
        let f21 = ee::mul(v.ring_r, c);
        let f20 = Fx::powf2(v.ring_r);
        let f0 = Fx::powf2(f21);
        let radius = fx.sqrtf(ee::sub(f20, f0));
        let height = ee::mul(2.0, ee::add(v.ring_h, f21));
        let s7 = phase.wrapping_add((rem2(k, 8) << 16) >> 3);
        let row = e(v) + 0x44 * k as u32;
        let out = e(v) + 0x110 * k as u32;
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

/// The tube's 16 x 16 quads drawn [F 0x1E00874].
fn tube(fx: &mut Fx, sp: u32, age: i32, v: &V) {
    let d = at(sp, v, 0x378);
    let r = if v.tube_mod == 24 { fx.div(age, 0x18).1 } else { age % v.tube_mod };
    let f20 = ee::div(ee::mul(-1.0, ee::from_i32(r)), ee::from_i32(v.tube_mod));
    for k in 0..0x10i32 {
        let f3 = ee::div(ee::from_i32(rem2(k, 16)), 16.0);
        let f2 = ee::div(ee::from_i32(rem2(k, 16) + 1), 16.0);
        let a0 = (150 * (16 - k)) >> 4;
        let a1 = (150 * (16 - (k + 1))) >> 4;
        for (o, a) in [(0x388u32, a0), (0x3B8, a1)] {
            let o = at(sp, v, o);
            fx.mem.write_u8(o, v.colour[0]);
            fx.mem.write_u8(o + 1, v.colour[1]);
            fx.mem.write_u8(o + 2, v.colour[2]);
            fx.mem.write_u8(o + 3, a as u8);
            fx.mem.write_u32(o + 4, 0x3F80_0000);
            let (c, q) = (fx.mem.read_u32(o), fx.mem.read_u32(o + 4));
            fx.mem.write_u32(o + 0x18, c);
            fx.mem.write_u32(o + 0x1C, q);
        }
        let f21 = ee::add(f3, f20);
        let f22 = ee::add(f2, f20);
        let row0 = e(v) + 0x110 * k as u32;
        let row1 = e(v) + 0x110 * (k + 1) as u32;
        for j in 0..0x10i32 {
            let f2 = ee::div(ee::from_i32(rem2(j, 16)), 16.0);
            let f0 = ee::div(ee::from_i32(rem2(j, 16) + 1), 16.0);
            fx.mem.setf(at(sp, v, 0x390), f21);
            fx.mem.setf(at(sp, v, 0x394), f2);
            fx.mem.setf(at(sp, v, 0x3A8), f21);
            fx.mem.setf(at(sp, v, 0x3AC), f0);
            fx.mem.setf(at(sp, v, 0x3C0), f22);
            fx.mem.setf(at(sp, v, 0x3C4), f2);
            fx.mem.setf(at(sp, v, 0x3D8), f22);
            fx.mem.setf(at(sp, v, 0x3DC), f0);
            let (p0, p1) = (0x60 + 0x10 * j as u32, 0x60 + 0x10 * (j + 1) as u32);
            let r = fx.project_n(&[row0 + p0, row0 + p1, row1 + p0, row1 + p1], at(sp, v, 0x1F0), d, at(sp, v, 0x40C), v.r_proj);
            if r == 1 {
                let w = fx.mem.f(at(sp, v, 0x40C));
                fx.fix_residue(w, v.r_tube);
                let key = ee::fixunssfsi(w) as i32;
                fx.submit(d, OT, key);
            }
        }
    }
}

/// The trail records [F 0x1E00508]: each live one draws its tube, leaves a
/// puff entry in its group, and at its end a big burst.
fn trails(fx: &mut Fx, sp: u32, v: &V) {
    fx.mem.write_u32(at(sp, v, 0x1C0), 0);
    fx.mem.write_u32(at(sp, v, 0x110), 0);
    fx.mem.write_u32(at(sp, v, 0x120), D);
    for i in 0..v.n {
        let rec = e(v) + 0x70 * i as u32;
        let grp = e(v) + 0xC00 * i as u32;
        if fx.mem.read_u8(rec + 0x1700) != 0 {
            fx.camera_vp_to(at(sp, v, 0x1F0));
            let age = fx.mem.i32(rec + 0x1704);
            point(fx, rec, age, at(sp, v, 0x2F0), v);
            let age1 = fx.mem.i32(rec + 0x1704) + 1;
            point(fx, rec, age1, at(sp, v, 0x300), v);
            fx.vu_sub4(at(sp, v, 0x300), at(sp, v, 0x300), at(sp, v, 0x2F0));
            if v.turn_matrix {
                fx.point_along(rec + 0x1730, at(sp, v, 0x300));
            } else {
                let (y, z) = (fx.mem.f(at(sp, v, 0x304)), fx.mem.f(at(sp, v, 0x308)));
                let a = fx.atan2f(y, z);
                let f = ee::mul(65536.0, ee::div(a, TWO_PI));
                fx.fix_residue(f, v.r_pitch);
                let pitch = ee::fixsfsi(f).wrapping_neg() & 0xFFFF;
                fx.identity(rec + 0x1730);
                fx.rotate_axis(rec + 0x1730, pitch, 0);
            }
            fx.vu_add4(at(sp, v, 0x2F0), at(sp, v, 0x2F0), rec + 0x1710);
            fx.translate_rows(at(sp, v, 0x230), rec + 0x1730, at(sp, v, 0x2F0));
            if !v.straight {
                words(fx, at(sp, v, 0x310), at(sp, v, 0x2F0));
            }
            let m = fx.read_m4(at(sp, v, 0x230));
            fx.mul_into(Some(at(sp, v, 0x1F0)), &m);
            let age = fx.mem.i32(rec + 0x1704);
            rings(fx, sp, v, i, age);
            let age = fx.mem.i32(rec + 0x1704);
            tube(fx, sp, age, v);
            let c = fx.mem.i32(rec + 0x1708);
            fx.mem.set32(rec + 0x1708, c + 1);
            if c + 1 >= 0x20 {
                fx.mem.write_u32(rec + 0x1708, 0);
            }
            let e = grp + 0x60 * c as u32 + v.puffs;
            fx.mem.write_u8(e, 1);
            fx.mem.write_u32(e + 4, 0);
            words(fx, e + 0x10, at(sp, v, 0x2F0));
            for q in 0..4u32 {
                let r = fx.rand_from(S0, v.r_puff) >> 23;
                fx.mem.set8(e + q + 8, rem2(r, 8));
                fx.random_table_vec(e + 0x10 * q + 0x20, f32::from_bits(v.puff_dir));
            }
            let age = fx.mem.i32(rec + 0x1704) + 1;
            fx.mem.set32(rec + 0x1704, age);
            let life = if v.lives.is_empty() { v.life } else { fx.mem.i32(rec + 0x170C) + 1 };
            if age >= life {
                fx.mem.write_u8(rec + 0x1700, 0);
                let m = fx.mem.i32(self::e(v) + v.big_index);
                let b = self::e(v) + v.big_size * m as u32 + v.bigs;
                fx.mem.write_u8(b, 1);
                fx.mem.write_u32(b + 4, 0);
                if !v.straight {
                    words(fx, b + 0x50, at(sp, v, 0x310));
                }
                for q in 0..0x40u32 {
                    let r = fx.rand_from(S0, v.r_big) >> 23;
                    fx.mem.set8(b + q + 8, rem2(r, 8));
                    fx.random_table_vec(b + 0x10 * q + v.big_dirs, f32::from_bits(0x3FD9_999A));
                }
                let m = fx.mem.i32(self::e(v) + v.big_index) + 1;
                fx.mem.set32(self::e(v) + v.big_index, m);
                if m >= v.n {
                    fx.mem.write_u32(self::e(v) + v.big_index, 0);
                }
            }
        }
        fx.mem.set32(at(sp, v, 0x110), (i + 1) << 16);
        fx.mem.write_u32(at(sp, v, 0x120), grp + 0xC00);
        fx.mem.set32(at(sp, v, 0x1C0), i + 1);
    }
}

/// The puffs [F 0x1E00CC8]: 10 groups of 32 entries.
fn puffs(fx: &mut Fx, sp: u32, v: &V) {
    let d = at(sp, v, 0x320);
    let rh = fx.mem.read_u32(D + 0x14);
    sprite_desc(fx, sp, v, rh);
    fx.mem.write_u32(at(sp, v, 0x1B0), 0);
    fx.mem.write_u32(at(sp, v, 0x140), D);
    for i in 0..v.n as u32 {
        let grp = e(v) + 0xC00 * i;
        fx.mem.write_u32(at(sp, v, 0x130), grp);
        fx.mem.write_u32(at(sp, v, 0x1D0), 0);
        fx.mem.write_u32(at(sp, v, 0x1E0), 0);
        for e in 0..0x20u32 {
            let s4 = grp + 0x60 * e + v.puffs;
            if fx.mem.read_u8(s4) != 0 {
                let age = fx.mem.i32(s4 + 4);
                let s = half_size(age, 32.0, v.puff_size);
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
            fx.mem.write_u32(at(sp, v, 0x1E0), e + 1);
        }
        fx.mem.write_u32(at(sp, v, 0x130), grp + 0xC00);
        fx.mem.write_u32(at(sp, v, 0x1D0), 1);
        fx.mem.write_u32(at(sp, v, 0x140), grp + 0xC00);
        fx.mem.write_u32(at(sp, v, 0x1B0), i + 1);
    }
}

/// The bursts [F 0x1E01078]: 10 records of 16 sprites.
fn bursts(fx: &mut Fx, sp: u32, v: &V) {
    let d = at(sp, v, 0x320);
    let rh = fx.mem.read_u32(D + 0x18);
    sprite_desc(fx, sp, v, rh);
    fx.mem.write_u32(at(sp, v, 0x1A0), 0);
    for k in 0..v.n as u32 {
        let s4 = e(v) + 0x130 * k + v.bursts;
        if fx.mem.read_u8(s4) != 0 {
            let age = fx.mem.i32(s4 + 4);
            let s = half_size(age, ee::from_i32(v.burst_n), 10.0);
            corners(fx, sp, v, s);
            let age = fx.mem.i32(s4 + 4);
            colour(fx, sp, v, v.burst_colour, ((v.burst_n - age) * v.burst_alpha) as u8);
            let age = fx.mem.i32(s4 + 4);
            let f0 = fx.tan_falloff_p(age, v.burst_n, 1);
            let f20 = ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), f0));
            for j in 0..0x10u32 {
                let b = fx.mem.read_u8(s4 + j + 8);
                sprite_uvs(fx, sp, v, b);
                sprite(fx, sp, v, s4 + 0x10 * j + 0x30, f20, s4 + 0x20);
                draw(fx, sp, v, d, v.r_bursts);
            }
            let age = fx.mem.i32(s4 + 4) + 1;
            fx.mem.set32(s4 + 4, age);
            if age >= v.burst_n + 1 {
                fx.mem.write_u8(s4, 0);
            }
        }
        fx.mem.write_u32(at(sp, v, 0x1A0), k + 1);
    }
}

/// The big bursts [F 0x1E013EC]: 10 records of 64 sprite pairs.
fn big_bursts(fx: &mut Fx, sp: u32, v: &V) {
    let d = at(sp, v, 0x320);
    let rh = fx.mem.read_u32(D + 0x18);
    sprite_desc(fx, sp, v, rh);
    fx.mem.write_u32(at(sp, v, 0x190), 0);
    for m in 0..v.n as u32 {
        let s7 = e(v) + v.big_size * m + v.bigs;
        if fx.mem.read_u8(s7) != 0 {
            let age = fx.mem.i32(s7 + 4);
            let s = half_size(age, 64.0, 13.0);
            corners(fx, sp, v, s);
            let age = fx.mem.i32(s7 + 4);
            let a = ((v.big_alpha * (0x40 - age)) >> 6) & 0xFF;
            let f0 = fx.tan_falloff_p(age, 0x40, 1);
            let f21 = ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), f0));
            let age = fx.mem.i32(s7 + 4);
            let f20 = ee::mul(50.0, ee::div(ee::from_i32(age), 64.0));
            if !v.straight {
                fx.mem.write_u32(at(sp, v, 0x150), s7 + 0x50);
            }
            for j in 0..0x40u32 {
                let rh = fx.mem.read_u32(D + 0x14);
                fx.desc_texture(d, rh, 1);
                let b = fx.mem.read_u8(s7 + j + 8);
                sprite_uvs(fx, sp, v, b);
                let base = if v.straight { e(v) + 0x20 } else { fx.mem.read_u32(at(sp, v, 0x150)) };
                sprite(fx, sp, v, s7 + 0x10 * j + v.big_dirs, f21, base);
                colour(fx, sp, v, v.big_colours[0], a as u8);
                draw(fx, sp, v, d, v.r_bigs[0]);
                let rh = fx.mem.read_u32(D + 0x18);
                fx.desc_texture(d, rh, 1);
                let y = fx.mem.f(at(sp, v, 0x2F4));
                fx.mem.setf(at(sp, v, 0x2F4), ee::add(y, f20));
                fx.vu_add4(at(sp, v, 0x2D0), at(sp, v, 0x2B0), at(sp, v, 0x2F0));
                fx.vu_add4(at(sp, v, 0x2E0), at(sp, v, 0x2C0), at(sp, v, 0x2F0));
                colour(fx, sp, v, v.big_colours[1], a as u8);
                draw(fx, sp, v, d, v.r_bigs[1]);
            }
            let age = fx.mem.i32(s7 + 4) + 1;
            fx.mem.set32(s7 + 4, age);
            if age >= 0x41 {
                fx.mem.write_u8(s7, 0);
            }
        }
        fx.mem.write_u32(at(sp, v, 0x190), m + 1);
    }
}

fn step(fx: &mut Fx, sp: u32, v: &V) -> i32 {
    fx.camera_turn_roll(at(sp, v, 0x270));
    if v.lens {
        screen_point(fx, sp, v);
    }
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(e(v) + 0x30));
    fx.node_position(h, slot, e(v) + 0x40);
    if v.z_add {
        let z = fx.mem.f(e(v) + 0x48);
        fx.mem.setf(e(v) + 0x48, ee::add(z, 0.0));
    }
    let n = fx.mem.i32(D);
    let spawning = match v.spawn_window {
        Some((lo, hi, m)) => (lo..hi).contains(&n) && rem2(n, m) == 0,
        None => v.spawn_at.contains(&n),
    };
    if spawning {
        spawn(fx, sp, v);
    }
    let d = at(sp, v, 0x378);
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
    if v.lens {
        lens(fx, sp);
        let n = fx.mem.i32(D);
        let s0 = Fx::clamp_i(0x14D - n, 0, 0x20);
        let n = fx.mem.i32(D);
        let k = Fx::clamp_i(n, 0, 0x20);
        let l = ((k.wrapping_neg() << 6).wrapping_mul(s0) >> 10) + 0x80;
        fx.set_stage_light(l & 0xFF);
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
    fx.mem.write_u32(at(sp, v, 0xB0), ret as u32);
    ret
}

/// b022's attacker screen point [F 0x1E002D4..0x1E00400]: its point
/// (0x1F65D0 kind 1) into D+0x20, projected through the camera (0x204D80
/// into sp + 0x4A0, w sp + 0x4AC); D+0x30 / 0x34 = the window pixel (x16
/// >> 4) - 0x700 clamped to 0..512, D+0x38 = the Z (0x17BC10, unsigned
/// to float: exact below 2^24), D+0x3C = 1.0.
fn screen_point(fx: &mut Fx, sp: u32, v: &V) {
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x20);
    camera(fx, sp, v);
    let m = fx.read_m4(at(sp, v, 0x1F0));
    fx.project_at(D + 0x20, &m, sp + 0x4A0, sp + 0x4AC);
    let x = (i32::from(fx.mem.read_u16(sp + 0x4A0)) >> 4) - 0x700;
    fx.mem.setf(D + 0x30, ee::from_i32(x));
    let y = (i32::from(fx.mem.read_u16(sp + 0x4A2)) >> 4) - 0x700;
    fx.mem.setf(D + 0x34, ee::from_i32(y));
    let z = (fx.mem.read_u64(sp + 0x4A0) << 8) >> 40;
    fx.mem.setf(D + 0x38, ee::from_i32(z as i32));
    fx.mem.write_u32(D + 0x3C, 0x3F80_0000);
    for o in [0x30u32, 0x34] {
        if crate::exe::ee_lt(fx.mem.f(D + o), 0.0) {
            fx.mem.write_u32(D + o, 0);
        }
    }
    for o in [0x30u32, 0x34] {
        if !crate::exe::ee_le(fx.mem.f(D + o), 512.0) {
            fx.mem.write_u32(D + o, 0x4400_0000);
        }
    }
}

/// 0x1A2B80 of the float at `a`.
fn fix_at(fx: &mut Fx, a: u32, ra: u32) -> i32 {
    let f = fx.mem.f(a);
    fx.fix_residue(f, ra);
    ee::fixsfsi(f)
}

/// The window XY of pixel (x, y) at the far Z.
fn fan_xy(fx: &mut Fx, p: u32, x: i32, y: i32) {
    crate::lens::far_xy(fx, p, ((x + 0x700) << 4) as u16, ((y + 0x700) << 4) as u16);
}

/// b022's fan [F 0x1E019A0..0x1E01EF8]: armed on update 0x14 (D+0xCB30,
/// age +0xCB34), for 32 updates in a packet layer (0x2078A0, at sp +
/// 0xDC) the attacker drawn into the target (0x1F8270 filter 1, TEX0 /
/// TEX1 into the descriptor) between two TEXFLUSHes, then three rings of
/// eight triangles (sp + 0x428, prim 0x33, 9 registers, alpha preset 1,
/// the lens CLAMP) from the screen point to each pair of the nine fan
/// points scaled about it by 1 + 0.3 falloff (k + 1) / 3, texels the
/// unscaled pixels; colour (FF, 40, FF), alpha 0 at the centre and 228
/// (32 - age) >> 5 outside; the layer linked at key 0.
fn lens(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x14 {
        fx.mem.write_u8(D + LENS, 1);
        fx.mem.write_u32(D + LENS + 4, 0);
    }
    if fx.mem.read_u8(D + LENS) != 1 {
        return;
    }
    let d = sp + 0x428;
    fx.mem.write_u16(d + 2, 0x33);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, 0x4_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, crate::lens::LENS_CLAMP);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xDC, layer);
    // a0..a3 as 0x208400 leaves them (as b483's paste).
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.texflush([flags, crate::lens::LENS_CLAMP as u32, d + 0x10, d + 2]);
    let h = fx.mem.i32(D + 4);
    let (_, regs) = fx.unit_draw(h, 1, sp + 0x488, sp + 0x490);
    fx.texflush(regs);
    let age = fx.mem.i32(D + LENS + 4);
    let a = (228 * (0x20 - age)) >> 5;
    for (o, c) in [(0x438u32, 0xFFu8), (0x439, 0x40), (0x43A, 0xFF), (0x43B, 0)] {
        fx.mem.write_u8(sp + o, c);
    }
    fx.mem.write_u32(sp + 0x43C, 0x3F80_0000);
    for (o, c) in [(0x450u32, 0xFFu8), (0x451, 0x40), (0x452, 0xFF), (0x453, a as u8)] {
        fx.mem.write_u8(sp + o, c);
    }
    fx.mem.write_u32(sp + 0x454, 0x3F80_0000);
    let (w0, w1) = (fx.mem.read_u32(sp + 0x450), fx.mem.read_u32(sp + 0x454));
    fx.mem.write_u32(sp + 0x468, w0);
    fx.mem.write_u32(sp + 0x46C, w1);
    let age = fx.mem.i32(D + LENS + 4);
    let f0 = fx.tan_falloff_p(age, 0x20, 1);
    let f20 = ee::mul(f32::from_bits(0x3E99_999A), f0);
    for k in 0..3i32 {
        fx.mem.set32(sp + 0x1A0, k);
        let f21 = ee::add(1.0, ee::div(ee::mul(f20, ee::from_i32(k + 1)), 3.0));
        for j in 0..8u32 {
            let (p0, p1) = (FAN + 0x10 * j, FAN + 0x10 * (j + 1));
            let x = fix_at(fx, D + 0x30, 0x01E0_1B9C);
            let y = fix_at(fx, D + 0x34, 0x01E0_1BAC);
            uv(fx, sp + 0x440, (x << 4) as u32);
            uv(fx, sp + 0x442, (y << 4) as u32);
            let x = fix_at(fx, p0, 0x01E0_1BF0);
            let y = fix_at(fx, p0 + 4, 0x01E0_1BFC);
            uv(fx, sp + 0x458, (x << 4) as u32);
            uv(fx, sp + 0x45A, (y << 4) as u32);
            let x = fix_at(fx, p1, 0x01E0_1C40);
            let y = fix_at(fx, p1 + 4, 0x01E0_1C4C);
            uv(fx, sp + 0x470, (x << 4) as u32);
            uv(fx, sp + 0x472, (y << 4) as u32);
            fx.vu_sub4(sp + 0x2F0, p0, D + 0x30);
            fx.vu_sub4(sp + 0x300, p1, D + 0x30);
            fx.vu_scale4(sp + 0x2F0, sp + 0x2F0, f21);
            fx.vu_scale4(sp + 0x300, sp + 0x300, f21);
            fx.vu_add4(sp + 0x2F0, sp + 0x2F0, D + 0x30);
            fx.vu_add4(sp + 0x300, sp + 0x300, D + 0x30);
            let x = fix_at(fx, D + 0x30, 0x01E0_1D14);
            let y = fix_at(fx, D + 0x34, 0x01E0_1D24);
            fan_xy(fx, sp + 0x448, x, y);
            let x = fix_at(fx, sp + 0x2F0, 0x01E0_1D90);
            let y = fix_at(fx, sp + 0x2F4, 0x01E0_1D9C);
            fan_xy(fx, sp + 0x460, x, y);
            let x = fix_at(fx, sp + 0x300, 0x01E0_1E00);
            let y = fix_at(fx, sp + 0x304, 0x01E0_1E0C);
            fan_xy(fx, sp + 0x478, x, y);
            fx.submit(d, 0, 0);
        }
    }
    let layer = fx.mem.read_u32(sp + 0xDC);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + LENS + 4) + 1;
    fx.mem.set32(D + LENS + 4, age);
    if age >= 0x21 {
        fx.mem.write_u8(D + LENS, 0);
    }
}
