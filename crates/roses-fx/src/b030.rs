//! BMODULE b030 (card 5) and its family b167 (card 8) / b382 (card 29) /
//! b009 (card 4) / b744 (card 83; helper E7: two / three attacker nodes,
//! the header 0x20 / 0x30 and the frame's scratch 0x20 higher, two-block
//! fleck / ember records), b755 (card 266; helper B31: b009's code in a
//! 0x870 frame, five nodes, two bursts, see `B755`),
//! hand-ported from the records (linked at 0x1E00000, one function
//! 0x1E00080, data area a0 = 0x1ED0000, 0x8720 bytes; b167 / b382 0x9C20)
//! [F]: a single unscheduled function in a 0x7B0
//! frame (s0 = the ordering table), b022's lens family. Every update the
//! camera turn with roll (0x2055E0 into sp + 0x570), the attacker's node
//! 0x32 (b167 7, its y lowered 2; b382 0x1C) into D+0x20, its point
//! (0x1F65D0 kind 1, D+0x50) and the screen points (0x204D80 through the
//! camera, x16 >> 4 - 0x700 clamped to 0..512) of that point (D+0x60, Z
//! D+0x68), of the point 1.5 x the attacker's radius in front of it (its
//! Z alone, D+0x74) and of the target (D+0x40). On update 0x43 (0x23 /
//! 0x2D) a lens (48 updates, 32): in a packet layer the frame copied as a
//! texture (0x207BB0 / 0x2095A0 / 0x207F50 / 0x207BB0), a full-window tint
//! (C8, B4, FF; FF, 20, 08; FF, 5A, 1C) at 128 (life - age) / life and the
//! frame drawn back at alpha 128 less that. In the fan window (1..0xF2,
//! 1..0x10E, 0x31..0x158) one of ten fans (D+0x78, 11 updates) a update:
//! in a packet layer the attacker drawn into the target (0x1F8270), eight
//! triangles from the attacker's screen point to the fan points 0x28ABE0
//! scaled about it by 1.01 + (0.9 + 0.2 age / 10), the texels the
//! unscaled pixels, at the front point's Z, alpha 255 sin((age + 1) << 15
//! / 11) x the window ramp outside, 0 inside. Then four sprite pools at
//! the node: 32 x 8 flecks (D+0xD0, 32 updates), 48 x 8 (12) sparks
//! (D+0x14D0, 48 updates: drawn towards the target by 1.7 age / 48 (b382:
//! arcing by -15 sin of that), sized by the soft-double 0.5 + rand / 512
//! and dimmed by their xy spread), 48 x 6 motes (D+0x44D0 / 0x59D0; b167
//! / b382 swaying by the sine of their random angles) and 32 x 8 embers
//! (D+0x7310 / 0x8810); in the ring window one of 32 rings (32 updates,
//! random turn, spun by (55, 77, 99) x age, odd ones the other way,
//! scaled 0.4 + 0.6 age / 32 flat) of 16 x 2 textured quads (ST,
//! scrolling) between radii 30 / 65 / 100 at the target; a flat sprite
//! (60) at the target; a second lens fan (0x84 / 0xA4 / 0x8A updates, no
//! layer) at the target's screen point (1.1 / 1.05, far Z, frame texture
//! 0x207EF0). The stage light 0x80 - 96 clamp(n) clamp(end - n) / 1024.
//! Returns 1 / 6 / 2 / 8 (b030: 0x61 / 0x6C..0xF2 / 0xF3 / 0x134; b167:
//! 0x4D / 0x58..0xCE / 0xCF / 0x110; b382: 0xA5 / 0xB0..0x118 / 0x119 /
//! 0x15A).
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_xy, KEEP, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
/// The fan's nine screen points (exe rodata, 0x10 apart).
const FAN: u32 = 0x0028_ABE0;

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

/// A pool's sprite size k (base + slope x age / d).
#[derive(Clone, Copy)]
struct Lin(f32, f32);

/// What differs between the three records.
struct V {
    bzero: u32,
    /// The frame and the shift of its scratch area (sp + 0x4F0..; the
    /// saves below stay put).
    frame: u32,
    fs: u32,
    /// The shift of the header (D+0x30..) and of the pools.
    hs: u32,
    /// The attacker's nodes (node, clear): one at D+0x20, else D+0x30 +
    /// 0x10 i (slots D+0x1C + 4 i).
    nodes: &'static [(i32, i32)],
    /// b167: node 0's y += -2.0 after the nodes.
    lower: bool,
    /// The node each pool spawns at (flecks, sparks, motes, embers).
    pool_nodes: [u32; 4],
    /// Blocks of eight sprites a fleck / ember record (block b at node
    /// pool_nodes + b), and the records' strides.
    fleck_blocks: u32,
    fleck_stride: u32,
    ember_blocks: u32,
    ember_stride: u32,
    /// The spark records' first byte (D+0x14D0 + stride k).
    sp_base: u32,
    /// The sparks' / motes' fade-in ratio_below limit.
    spark_below: f32,
    mote_below: f32,
    /// b744: the embers rise by this (else `ember_rise`'s 0).
    ember_lift: Option<Lin>,
    /// The spark records (D+0x14D0 + stride k): sprites a record and the
    /// offsets of their sizes / alphas / point / directions.
    sp_stride: u32,
    sp_n: u32,
    sp_sizes: u32,
    sp_alphas: u32,
    sp_pos: u32,
    sp_dirs: u32,
    motes: u32,
    ring_pts: u32,
    rings: u32,
    lens1: u32,
    embers: u32,
    lens2: u32,
    lens1_at: i32,
    lens1_life: i32,
    tint: [u8; 3],
    /// Spawn windows [lo, hi): the first clamp is of n - (lo - 1).
    fans: (i32, i32),
    fan_colours: [[u8; 3]; 2],
    flecks: (i32, i32),
    fleck_len: u32,
    /// The texture / alpha set per live record (else once).
    fleck_per_record: bool,
    /// The size's factor (b030 none), the point's spread, the rise.
    fleck_k: Option<f32>,
    fleck_spread: Lin,
    fleck_rise: Option<Lin>,
    fleck_colour: [u8; 3],
    sparks: (i32, i32),
    spark_len: f32,
    spark_pow: f32,
    spark_k: f32,
    spark_size: Lin,
    spark_spread: Lin,
    spark_amp: f32,
    spark_colour: [u8; 3],
    /// b382: the sparks / motes arc -15 sin(32768 pull).
    arc: bool,
    mote_len: u32,
    mote_k: f32,
    mote_size: Lin,
    mote_amp: f32,
    mote_colour: [u8; 3],
    /// b167 / b382: the spread (0.1 + 0.9 age / 48) x sin(angle + (age <<
    /// 15) / 48), else 0.2 + 0.8 age / 48.
    mote_sine: bool,
    rings_win: (i32, i32),
    ring_colours: [[u8; 3]; 2],
    ring_amp: f32,
    /// The flat sprite's window clamp(n - lo) x clamp(hi - n).
    flat: (i32, i32),
    flat_colour: [u8; 3],
    lens2_at: i32,
    lens2_ramp: (f32, f32),
    lens2_end: i32,
    lens2_colour: [u8; 3],
    lens2_scale: u32,
    embers_win: (i32, i32),
    ember_len: u32,
    ember_alpha: i32,
    /// b382: sin (else cos), the size and spread by 32 - age.
    ember_sin: bool,
    ember_amp: f32,
    ember_rev: bool,
    ember_rise: bool,
    ember_colour: [u8; 3],
    /// b009: the fan's second colour at f21 x amp x sin (else 0).
    fan2_amp: Option<f32>,
    /// The motes' sine spread (b167 / b382 0.1 + 0.9 age / 48).
    mote_sine_lin: Lin,
    /// The embers' size factor.
    ember_k: f32,
    light: i32,
    one: i32,
    six: (i32, i32),
    two: i32,
    end: i32,
    /// Return addresses: rand (fleck, spark cell / size, mote cell /
    /// angle, ring y / x / z, ember), the two fans' 0x1A2B80, 0x1A2C70
    /// (lens, fan x2, fleck x2, spark x2, mote x2, ring x3, flat x2,
    /// lens2, ember x2), 0x100BA0 x2, the arcs' 0x1A2B80 (sparks, motes).
    r_rand: [u32; 9],
    r_fan1: [u32; 12],
    r_fan2: [u32; 12],
    r_fixu: [u32; 17],
    r_pn: [u32; 2],
    r_arc: [u32; 2],
    /// b755's layout and parts (else [`X0`]).
    x: X,
}

/// What b755 (card 266) adds: the frame slots, explicit node slots /
/// positions, fixed pool spawn points, the ring radii, the flat sprite's
/// size, embers textured once, their sine shift and spread, the bursts.
struct X {
    /// sp + .. of D, the return value and the layer (b030 0xCC / 0xD0 /
    /// 0xEC).
    d_slot: u32,
    ret_slot: u32,
    layer_slot: u32,
    /// (slot, position) of each node (else D+0x1C + 4 i / `node_at`).
    node_tab: Option<&'static [(u32, u32)]>,
    /// D+.. the flecks / sparks / motes / embers spawn at (else the nodes).
    pool_at: Option<[u32; 4]>,
    ring_r: (f32, f32),
    flat_size: f32,
    /// The embers' texture / alpha set once before the records.
    ember_once: bool,
    ember_shift: i32,
    ember_spread: Lin,
    bursts: Option<Bursts>,
    /// Blocks of `sp_n` sparks / six motes a record (b031: 2; sprite m =
    /// blocks x j + b, block b at the pool's node + b; else 1).
    sp_blocks: u32,
    mote_blocks: u32,
}

/// b755's two bursts (0x470 a record: flag, age, 2 x 32 cells, the two
/// points at +0x50, 2 x 32 directions at +0x70).
struct Bursts {
    /// hd + ..
    base: u32,
    /// The updates spawning record 1 / record 0.
    at: [i32; 2],
    /// Record a's blocks spawn at nodes `node` + 2 a + b.
    node: u32,
    dir_len: f32,
    k: f32,
    amp: f32,
    colour: [u8; 3],
    r_rand: u32,
    r_fixu: u32,
}

const X0: X = X {
    d_slot: 0xCC,
    ret_slot: 0xD0,
    layer_slot: 0xEC,
    node_tab: None,
    pool_at: None,
    ring_r: (30.0, 100.0),
    flat_size: 60.0,
    ember_once: false,
    ember_shift: 14,
    ember_spread: Lin(f(0x3E4C_CCCD), f(0x3F4C_CCCD)),
    bursts: None,
    sp_blocks: 1,
    mote_blocks: 1,
};

const B030: V = V {
    bzero: 0x8720,
    frame: 0x7B0,
    fs: 0,
    hs: 0,
    nodes: &[(0x32, 1)],
    pool_nodes: [0; 4],
    fleck_blocks: 1,
    fleck_stride: 0xA0,
    ember_blocks: 1,
    ember_stride: 0xA0,
    sp_base: 0x14D0,
    spark_below: 1.0,
    mote_below: 1.0,
    ember_lift: None,
    lower: false,
    sp_stride: 0x100,
    sp_n: 8,
    sp_sizes: 0x14F8,
    sp_alphas: 0x1518,
    sp_pos: 0x1540,
    sp_dirs: 0x1550,
    motes: 0x44D0,
    ring_pts: 0x65D0,
    rings: 0x6900,
    lens1: 0x7300,
    embers: 0x7310,
    lens2: 0x8710,
    lens1_at: 0x43,
    lens1_life: 0x30,
    tint: [0xC8, 0xB4, 0xFF],
    fans: (1, 0xF3),
    fan_colours: [[0xFF, 0x40, 0x64], [0xFF, 0x32, 0x64]],
    flecks: (1, 0xF3),
    fleck_len: 0x3F80_0000,
    fleck_per_record: false,
    fleck_k: None,
    fleck_spread: Lin(f(0x3E4C_CCCD), f(0x3F4C_CCCD)),
    fleck_rise: Some(Lin(0.0, 0.0)),
    fleck_colour: [0xFF, 0xE9, 0xE0],
    sparks: (0x54, 0xF3),
    spark_len: 1.5,
    spark_pow: 24.0,
    spark_k: 20.0,
    spark_size: Lin(f(0x3E99_999A), f(0x3F33_3333)),
    spark_spread: Lin(f(0x3D4C_CCCD), f(0x3F73_3333)),
    spark_amp: 100.0,
    spark_colour: [0x76, 0x64, 0xFF],
    arc: false,
    mote_len: 0x3FCC_CCCD,
    mote_k: 3.0,
    mote_size: Lin(f(0x3ECC_CCCD), f(0x3F19_999A)),
    mote_amp: 128.0,
    mote_colour: [0x9B, 0x86, 0x90],
    mote_sine: false,
    rings_win: (0x6C, 0xD3),
    ring_colours: [[0x44, 0x3A, 0xFF], [0xE6, 0xB8, 0xC2]],
    ring_amp: 80.0,
    flat: (0x6B, 0xF3),
    flat_colour: [0xAE, 0xA4, 0xFF],
    lens2_at: 0x8F,
    lens2_ramp: (132.0, 100.0),
    lens2_end: 0x85,
    lens2_colour: [0x76, 0x6C, 0xBC],
    lens2_scale: 0x3DCC_CCCD,
    embers_win: (1, 0xF3),
    ember_len: 0x3F33_3333,
    ember_alpha: 1,
    ember_sin: false,
    ember_amp: 48.0,
    ember_rev: false,
    ember_rise: true,
    ember_colour: [0x82, 0x64, 0xC8],
    fan2_amp: None,
    mote_sine_lin: Lin(f(0x3DCC_CCCD), f(0x3F66_6666)),
    ember_k: 5.0,
    light: 0x133,
    one: 0x61,
    six: (0x6C, 0xF3),
    two: 0xF3,
    end: 0x134,
    r_rand: [0x01E0_1148, 0x01E0_1690, 0x01E0_16B0, 0x01E0_1D88, 0x01E0_1DA8, 0x01E0_2320, 0x01E0_2354, 0x01E0_2388, 0x01E0_3098],
    r_fan1: [
        0x01E0_0D14, 0x01E0_0D24, 0x01E0_0D70, 0x01E0_0D7C, 0x01E0_0DC8, 0x01E0_0DD4, 0x01E0_0EA4, 0x01E0_0EB4, 0x01E0_0F20, 0x01E0_0F2C, 0x01E0_0F98,
        0x01E0_0FA4,
    ],
    r_fan2: [
        0x01E0_2C7C, 0x01E0_2C8C, 0x01E0_2CD8, 0x01E0_2CE4, 0x01E0_2D30, 0x01E0_2D3C, 0x01E0_2E0C, 0x01E0_2E1C, 0x01E0_2E88, 0x01E0_2E94, 0x01E0_2F00,
        0x01E0_2F0C,
    ],
    r_fixu: [
        0x01E0_07EC, 0x01E0_0C00, 0x01E0_0C64, 0x01E0_12F0, 0x01E0_1530, 0x01E0_1AD0, 0x01E0_1C1C, 0x01E0_2084, 0x01E0_21FC, 0x01E0_2494, 0x01E0_2738,
        0x01E0_27BC, 0x01E0_2A14, 0x01E0_2A90, 0x01E0_2BDC, 0x01E0_3240, 0x01E0_3478,
    ],
    r_pn: [0x01E0_2724, 0x01E0_27A8],
    r_arc: [0, 0],
    x: X0,
};

/// b167 (card 8): the larger layout (12 sparks a record), node 7 lowered
/// 2, other windows, sizes and colours, the motes swaying [F].
const B167: V = V {
    bzero: 0x9C20,
    frame: 0x7B0,
    fs: 0,
    hs: 0,
    nodes: &[(7, 1)],
    pool_nodes: [0; 4],
    fleck_blocks: 1,
    fleck_stride: 0xA0,
    ember_blocks: 1,
    ember_stride: 0xA0,
    sp_base: 0x14D0,
    spark_below: 1.0,
    mote_below: 1.0,
    ember_lift: None,
    lower: true,
    sp_stride: 0x170,
    sp_n: 12,
    sp_sizes: 0x1508,
    sp_alphas: 0x1538,
    sp_pos: 0x1570,
    sp_dirs: 0x1580,
    motes: 0x59D0,
    ring_pts: 0x7AD0,
    rings: 0x7E00,
    lens1: 0x8800,
    embers: 0x8810,
    lens2: 0x9C10,
    lens1_at: 0x23,
    lens1_life: 0x20,
    tint: [0xFF, 0x20, 0x08],
    fans: (1, 0x10F),
    fan_colours: [[0xFF, 0x1C, 0x12], [0xFF, 0, 0]],
    flecks: (1, 0xCF),
    fleck_len: 0x3F33_3333,
    fleck_per_record: true,
    fleck_k: Some(0.5),
    fleck_spread: Lin(f(0x3E4C_CCCD), f(0x3F4C_CCCD)),
    fleck_rise: None,
    fleck_colour: [0xFF, 0x64, 0x3C],
    sparks: (0x38, 0xCF),
    spark_len: 1.0,
    spark_pow: 16.0,
    spark_k: 15.0,
    spark_size: Lin(f(0x3E99_999A), f(0x3F33_3333)),
    spark_spread: Lin(f(0x3DCC_CCCD), f(0x3F66_6666)),
    spark_amp: 128.0,
    spark_colour: [0x8A, 0x35, 0x1C],
    arc: false,
    mote_len: 0x3F80_0000,
    mote_k: 9.0,
    mote_size: Lin(f(0x3E99_999A), f(0x3F33_3333)),
    mote_amp: 80.0,
    mote_colour: [0xFF, 0xA4, 0x50],
    mote_sine: true,
    rings_win: (0x58, 0xAF),
    ring_colours: [[0xE4, 0x26, 0x1A], [0xFF, 0xAE, 0x5A]],
    ring_amp: 100.0,
    flat: (0x57, 0xCF),
    flat_colour: [0xFF, 0xA4, 0x64],
    lens2_at: 0x4B,
    lens2_ramp: (164.0, 132.0),
    lens2_end: 0xA5,
    lens2_colour: [0xBC, 0x6C, 0x44],
    lens2_scale: 0x3D4C_CCCD,
    embers_win: (1, 0xCF),
    ember_len: 0x3F33_3333,
    ember_alpha: 1,
    ember_sin: false,
    ember_amp: 48.0,
    ember_rev: false,
    ember_rise: false,
    ember_colour: [0x80, 0x3A, 0x1C],
    fan2_amp: None,
    mote_sine_lin: Lin(f(0x3DCC_CCCD), f(0x3F66_6666)),
    ember_k: 5.0,
    light: 0x10F,
    one: 0x4D,
    six: (0x58, 0xCF),
    two: 0xCF,
    end: 0x110,
    r_rand: [0x01E0_1190, 0x01E0_16E8, 0x01E0_1708, 0x01E0_1DE0, 0x01E0_1E00, 0x01E0_23B0, 0x01E0_23E4, 0x01E0_2418, 0x01E0_3140],
    r_fan1: [
        0x01E0_0D5C, 0x01E0_0D6C, 0x01E0_0DB8, 0x01E0_0DC4, 0x01E0_0E10, 0x01E0_0E1C, 0x01E0_0EEC, 0x01E0_0EFC, 0x01E0_0F68, 0x01E0_0F74, 0x01E0_0FE0,
        0x01E0_0FEC,
    ],
    r_fan2: [
        0x01E0_2D0C, 0x01E0_2D1C, 0x01E0_2D68, 0x01E0_2D74, 0x01E0_2DC0, 0x01E0_2DCC, 0x01E0_2E9C, 0x01E0_2EAC, 0x01E0_2F18, 0x01E0_2F24, 0x01E0_2F90,
        0x01E0_2F9C,
    ],
    r_fixu: [
        0x01E0_0824, 0x01E0_0C50, 0x01E0_0CB4, 0x01E0_1338, 0x01E0_1578, 0x01E0_1B28, 0x01E0_1C74, 0x01E0_20D4, 0x01E0_2290, 0x01E0_2524, 0x01E0_27C8,
        0x01E0_284C, 0x01E0_2AA4, 0x01E0_2B20, 0x01E0_2C6C, 0x01E0_3308, 0x01E0_354C,
    ],
    r_pn: [0x01E0_27B4, 0x01E0_2838],
    r_arc: [0, 0],
    x: X0,
};

/// b382 (card 29): b167 with node 0x1C (not lowered), other windows,
/// sizes and colours, the sparks / motes arcing, white embers by 32 - age
/// [F].
const B382: V = V {
    nodes: &[(0x1C, 1)],
    lower: false,
    lens1_at: 0x2D,
    tint: [0xFF, 0x5A, 0x1C],
    fans: (0x31, 0x159),
    fan_colours: [[0xFF, 0x3A, 0x1C], [0xFF, 0, 0]],
    flecks: (1, 0x119),
    fleck_len: 0x3FF3_3333,
    fleck_k: Some(1.5),
    fleck_spread: Lin(0.5, 0.5),
    fleck_rise: Some(Lin(-10.0, 30.0)),
    fleck_colour: [0xFF, 0x4C, 0x20],
    sparks: (0x92, 0x119),
    spark_size: Lin(f(0x3ECC_CCCD), f(0x3F19_999A)),
    spark_colour: [0x80, 0x30, 0x1C],
    arc: true,
    mote_size: Lin(f(0x3ECC_CCCD), f(0x3F19_999A)),
    rings_win: (0xB0, 0xF9),
    flat: (0xAF, 0x119),
    lens2_at: 0xAF,
    lens2_ramp: (138.0, 106.0),
    lens2_end: 0x8B,
    embers_win: (0x21, 0x70),
    ember_len: 0x3FD9_999A,
    ember_alpha: 0,
    ember_sin: true,
    ember_amp: 64.0,
    ember_rev: true,
    ember_colour: [0xFF, 0xFF, 0xFF],
    light: 0x159,
    one: 0xA5,
    six: (0xB0, 0x119),
    two: 0x119,
    end: 0x15A,
    r_rand: [0x01E0_1180, 0x01E0_16F8, 0x01E0_1718, 0x01E0_1E28, 0x01E0_1E48, 0x01E0_2430, 0x01E0_2464, 0x01E0_2498, 0x01E0_31C0],
    r_fan1: [
        0x01E0_0D4C, 0x01E0_0D5C, 0x01E0_0DA8, 0x01E0_0DB4, 0x01E0_0E00, 0x01E0_0E0C, 0x01E0_0EDC, 0x01E0_0EEC, 0x01E0_0F58, 0x01E0_0F64, 0x01E0_0FD0,
        0x01E0_0FDC,
    ],
    r_fan2: [
        0x01E0_2D8C, 0x01E0_2D9C, 0x01E0_2DE8, 0x01E0_2DF4, 0x01E0_2E40, 0x01E0_2E4C, 0x01E0_2F1C, 0x01E0_2F2C, 0x01E0_2F98, 0x01E0_2FA4, 0x01E0_3010,
        0x01E0_301C,
    ],
    r_fixu: [
        0x01E0_080C, 0x01E0_0C40, 0x01E0_0CA4, 0x01E0_1328, 0x01E0_1588, 0x01E0_1B70, 0x01E0_1CBC, 0x01E0_2158, 0x01E0_2310, 0x01E0_25A4, 0x01E0_2848,
        0x01E0_28CC, 0x01E0_2B24, 0x01E0_2BA0, 0x01E0_2CEC, 0x01E0_338C, 0x01E0_35BC,
    ],
    r_pn: [0x01E0_2834, 0x01E0_28B8],
    r_arc: [0x01E0_1A90, 0x01E0_2090],
    ..B167
};

/// b009 (card 4): two nodes (0x30, 0x32 cleared 0) and the header 0x20
/// higher, a 0x7D0 frame (scratch 0x20 higher), b167-like pools with
/// other windows, sizes and colours, the fan's second colour at 128 sin
/// [F].
const B009: V = V {
    bzero: 0x8740,
    frame: 0x7D0,
    fs: 0x20,
    hs: 0x20,
    nodes: &[(0x30, 1), (0x32, 0)],
    lower: false,
    pool_nodes: [1, 0, 0, 1],
    fleck_blocks: 1,
    fleck_stride: 0xA0,
    ember_blocks: 1,
    ember_stride: 0xA0,
    sp_base: 0x14D0,
    spark_below: 1.0,
    mote_below: 1.0,
    ember_lift: None,
    sp_stride: 0x100,
    sp_n: 8,
    sp_sizes: 0x14F8,
    sp_alphas: 0x1518,
    sp_pos: 0x1540,
    sp_dirs: 0x1550,
    motes: 0x44D0,
    ring_pts: 0x65D0,
    rings: 0x6900,
    lens1: 0x7300,
    embers: 0x7310,
    lens2: 0x8710,
    lens1_at: 0x32,
    lens1_life: 0x20,
    tint: [0xFF, 0x5A, 0x1C],
    fans: (1, 0xDE),
    fan_colours: [[0xFF, 0x58, 0x30], [0xFF, 0, 0]],
    flecks: (1, 0xDE),
    fleck_len: 0x3F33_3333,
    fleck_per_record: true,
    fleck_k: Some(1.5),
    fleck_spread: Lin(0.5, 0.5),
    fleck_rise: None,
    fleck_colour: [0xFF, 0x18, 0x10],
    sparks: (0x4D, 0xDE),
    spark_len: f(0x3F8C_CCCD),
    spark_pow: f(0x418C_CCCD),
    spark_k: 18.0,
    spark_size: Lin(0.5, 0.5),
    spark_spread: Lin(f(0x3E4C_CCCD), f(0x3F4C_CCCD)),
    spark_amp: 128.0,
    spark_colour: [0x80, 0x30, 0x1C],
    arc: false,
    mote_len: 0x3F66_6666,
    mote_k: 10.0,
    mote_size: Lin(0.5, 0.5),
    mote_amp: 100.0,
    mote_colour: [0xFF, 0xA4, 0x64],
    mote_sine: true,
    rings_win: (0x6B, 0xBE),
    ring_colours: [[0xE4, 0x26, 0x1A], [0xFF, 0xAE, 0x5A]],
    ring_amp: 100.0,
    flat: (0x6A, 0xDE),
    flat_colour: [0xFF, 0xA4, 0x64],
    lens2_at: 0x6A,
    lens2_ramp: (148.0, 116.0),
    lens2_end: 0x95,
    lens2_colour: [0xBC, 0x6C, 0x44],
    lens2_scale: 0x3D4C_CCCD,
    embers_win: (1, 0x64),
    ember_len: 0x3F33_3333,
    ember_alpha: 0,
    ember_sin: false,
    ember_amp: 128.0,
    ember_rev: false,
    ember_rise: false,
    ember_colour: [0x64, 0x64, 0x64],
    fan2_amp: Some(128.0),
    mote_sine_lin: Lin(f(0x3E4C_CCCD), f(0x3F4C_CCCD)),
    ember_k: 10.0,
    light: 0x11E,
    one: 0x60,
    six: (0x6B, 0xDE),
    two: 0xDE,
    end: 0x11F,
    r_rand: [0x01E0_1168, 0x01E0_16A0, 0x01E0_16C0, 0x01E0_1DA8, 0x01E0_1DC8, 0x01E0_2378, 0x01E0_23AC, 0x01E0_23E0, 0x01E0_30F0],
    r_fan1: [
        0x01E0_0D34, 0x01E0_0D44, 0x01E0_0D90, 0x01E0_0D9C, 0x01E0_0DE8, 0x01E0_0DF4, 0x01E0_0EC4, 0x01E0_0ED4, 0x01E0_0F40, 0x01E0_0F4C, 0x01E0_0FB8,
        0x01E0_0FC4,
    ],
    r_fan2: [
        0x01E0_2CD4, 0x01E0_2CE4, 0x01E0_2D30, 0x01E0_2D3C, 0x01E0_2D88, 0x01E0_2D94, 0x01E0_2E64, 0x01E0_2E74, 0x01E0_2EE0, 0x01E0_2EEC, 0x01E0_2F58,
        0x01E0_2F64,
    ],
    r_fixu: [
        0x01E0_080C, 0x01E0_0C18, 0x01E0_0C8C, 0x01E0_1310, 0x01E0_1538, 0x01E0_1AE8, 0x01E0_1C34, 0x01E0_209C, 0x01E0_2258, 0x01E0_24EC, 0x01E0_2790,
        0x01E0_2814, 0x01E0_2A6C, 0x01E0_2AE8, 0x01E0_2C34, 0x01E0_3298, 0x01E0_34B0,
    ],
    r_pn: [0x01E0_277C, 0x01E0_2800],
    r_arc: [0, 0],
    x: X0,
};

/// b009's entry 0x1E00080.
pub fn program_b009(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B009)
}

/// b744 (card 83): three nodes (0xB, then 0x1E / 0x39 cleared 0; node 0
/// lowered 2) and the header 0x30 higher, b009's frame, flecks / embers
/// of two blocks of eight at nodes 1 / 2, sparks / motes faded in over
/// two updates, rising embers, other windows, sizes and colours [F].
const B744: V = V {
    bzero: 0xAF50,
    frame: 0x7D0,
    fs: 0x20,
    hs: 0x30,
    nodes: &[(0xB, 1), (0x1E, 0), (0x39, 0)],
    lower: true,
    pool_nodes: [1, 0, 0, 1],
    fleck_blocks: 2,
    fleck_stride: 0x140,
    ember_blocks: 2,
    ember_stride: 0x140,
    sp_base: 0x28D0,
    spark_below: 2.0,
    mote_below: 2.0,
    ember_lift: Some(Lin(-1.0, 10.0)),
    sp_stride: 0x100,
    sp_n: 8,
    sp_sizes: 0x28F8,
    sp_alphas: 0x2918,
    sp_pos: 0x2940,
    sp_dirs: 0x2950,
    motes: 0x58D0,
    ring_pts: 0x79D0,
    rings: 0x7D00,
    lens1: 0x8700,
    embers: 0x8710,
    lens2: 0xAF10,
    lens1_at: 0x8A,
    lens1_life: 0x20,
    tint: [0xFF, 0, 0],
    fans: (1, 0x1B2),
    fan_colours: [[0x64, 0x40, 0xFF], [0x64, 0x32, 0xFF]],
    flecks: (1, 0x172),
    fleck_len: 0x3E4C_CCCD,
    fleck_per_record: false,
    fleck_k: Some(f(0x3E99_999A)),
    fleck_spread: Lin(0.5, 0.5),
    fleck_rise: Some(Lin(-1.0, 10.0)),
    fleck_colour: [0xFF, 0xE9, 0xE0],
    sparks: (0xC9, 0x157),
    spark_len: 1.5,
    spark_pow: 24.0,
    spark_k: 20.0,
    spark_size: Lin(f(0x3ECC_CCCD), f(0x3F19_999A)),
    spark_spread: Lin(f(0x3D4C_CCCD), f(0x3F73_3333)),
    spark_amp: 64.0,
    spark_colour: [0xFF, 0x64, 0x26],
    arc: false,
    mote_len: 0x3F8C_CCCD,
    mote_k: 13.0,
    mote_size: Lin(f(0x3ECC_CCCD), f(0x3F19_999A)),
    mote_amp: 100.0,
    mote_colour: [0x9B, 0x86, 0x64],
    mote_sine: true,
    rings_win: (0xE7, 0x137),
    ring_colours: [[0xFF, 0x3A, 0x26], [0xE6, 0xB8, 0x50]],
    ring_amp: 150.0,
    flat: (0xE6, 0x157),
    flat_colour: [0xFF, 0xA4, 0x64],
    lens2_at: 0xE6,
    lens2_ramp: (145.0, 113.0),
    lens2_end: 0x92,
    lens2_colour: [0xBC, 0x80, 0x6C],
    lens2_scale: 0x3DCC_CCCD,
    embers_win: (1, 0x172),
    ember_len: 0x3EB3_3333,
    ember_alpha: 1,
    ember_sin: false,
    ember_amp: 48.0,
    ember_rev: false,
    ember_rise: false,
    ember_colour: [0x82, 0x64, 0xC8],
    fan2_amp: None,
    mote_sine_lin: Lin(f(0x3D4C_CCCD), f(0x3F73_3333)),
    ember_k: 2.5,
    light: 0x1BC,
    one: 0xDC,
    six: (0xE7, 0x157),
    two: 0x157,
    end: 0x1BD,
    r_rand: [0x01E0_11D8, 0x01E0_1750, 0x01E0_1770, 0x01E0_1E48, 0x01E0_1E68, 0x01E0_2420, 0x01E0_2454, 0x01E0_2488, 0x01E0_31B8],
    r_fan1: [
        0x01E0_0D9C, 0x01E0_0DAC, 0x01E0_0DF8, 0x01E0_0E04, 0x01E0_0E50, 0x01E0_0E5C, 0x01E0_0F2C, 0x01E0_0F3C, 0x01E0_0FA8, 0x01E0_0FB4, 0x01E0_1020,
        0x01E0_102C,
    ],
    r_fan2: [
        0x01E0_2D7C, 0x01E0_2D8C, 0x01E0_2DD8, 0x01E0_2DE4, 0x01E0_2E30, 0x01E0_2E3C, 0x01E0_2F0C, 0x01E0_2F1C, 0x01E0_2F88, 0x01E0_2F94, 0x01E0_3000,
        0x01E0_300C,
    ],
    r_fixu: [
        0x01E0_0864, 0x01E0_0C88, 0x01E0_0CEC, 0x01E0_1390, 0x01E0_15F0, 0x01E0_1B90, 0x01E0_1CDC, 0x01E0_2144, 0x01E0_2300, 0x01E0_2594, 0x01E0_2838,
        0x01E0_28BC, 0x01E0_2B14, 0x01E0_2B90, 0x01E0_2CDC, 0x01E0_3390, 0x01E0_3604,
    ],
    r_pn: [0x01E0_2824, 0x01E0_28A8],
    r_arc: [0, 0],
    x: X0,
};

/// b744's entry 0x1E00080.
pub fn program_b744(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B744)
}

/// b031 (card 434; helper C29): b744's code with two attacker nodes 0x27
/// / 0x31 (not lowered; the header 0x20 higher), the frame 0x7F0 (scratch
/// 0x40 higher), a 0xFA40-byte area (the pools past 0x10000), flecks /
/// sparks / motes / embers of two blocks at nodes 0 / 1 (sparks 0x1F0,
/// motes 0x150 a record), a (64, 50, C8) lens on 0x2E for 48, every
/// window to 0xE2, fleck size 0.2 + 0.8 (32 - age) / 32 unscaled, rings
/// 0x6C..0xC2, a second lens on 0x6B (151 / 119 to 0x98), embers 5.0 x
/// size without a lift, its own colours. Returns 1 on 0x61, 6 on
/// 0x6C..0xE1, 2 on 0xE2, 8 on 0x123.
const B031: V = V {
    bzero: 0xFA40,
    frame: 0x7F0,
    fs: 0x40,
    hs: 0x20,
    nodes: &[(0x27, 1), (0x31, 0)],
    lower: false,
    pool_nodes: [0; 4],
    spark_below: 1.0,
    mote_below: 1.0,
    ember_lift: None,
    sp_stride: 0x1F0,
    sp_n: 8,
    sp_sizes: 0x2918,
    sp_alphas: 0x2958,
    sp_pos: 0x29A0,
    sp_dirs: 0x29C0,
    motes: 0x85D0,
    ring_pts: 0xC4D0,
    rings: 0xC800,
    lens1: 0xD200,
    embers: 0xD210,
    lens2: 0xFA10,
    lens1_at: 0x2E,
    lens1_life: 0x30,
    tint: [0x64, 0x50, 0xC8],
    fans: (1, 0xE2),
    flecks: (1, 0xE2),
    fleck_len: 0x3F80_0000,
    fleck_k: None,
    fleck_spread: Lin(f(0x3E4C_CCCD), f(0x3F4C_CCCD)),
    fleck_rise: Some(Lin(0.0, 0.0)),
    sparks: (0x4F, 0xE2),
    spark_amp: 48.0,
    spark_colour: [0xFF, 0x64, 0x76],
    mote_len: 0x3F99_999A,
    mote_amp: 64.0,
    mote_colour: [0x90, 0x86, 0x9B],
    rings_win: (0x6C, 0xC2),
    ring_colours: [[0xFF, 0x3A, 0x44], [0xC2, 0xB8, 0xE6]],
    ring_amp: 100.0,
    flat: (0x6B, 0xE2),
    flat_colour: [0xFF, 0xA4, 0xAE],
    lens2_at: 0x6B,
    lens2_ramp: (151.0, 119.0),
    lens2_end: 0x98,
    lens2_colour: [0xBC, 0x6C, 0x76],
    embers_win: (1, 0xE2),
    ember_len: 0x3F33_3333,
    ember_rise: true,
    ember_k: 5.0,
    light: 0x122,
    one: 0x61,
    six: (0x6C, 0xE2),
    two: 0xE2,
    end: 0x123,
    r_rand: [0x01E0_11F8, 0x01E0_1760, 0x01E0_1780, 0x01E0_1E80, 0x01E0_1EA8, 0x01E0_2504, 0x01E0_2538, 0x01E0_256C, 0x01E0_3310],
    r_fan1: [
        0x01E0_0DBC, 0x01E0_0DCC, 0x01E0_0E18, 0x01E0_0E24, 0x01E0_0E70, 0x01E0_0E7C, 0x01E0_0F4C, 0x01E0_0F5C, 0x01E0_0FC8, 0x01E0_0FD4, 0x01E0_1040,
        0x01E0_104C,
    ],
    r_fan2: [
        0x01E0_2ED4, 0x01E0_2EE4, 0x01E0_2F30, 0x01E0_2F3C, 0x01E0_2F88, 0x01E0_2F94, 0x01E0_3064, 0x01E0_3074, 0x01E0_30E0, 0x01E0_30EC, 0x01E0_3158,
        0x01E0_3164,
    ],
    r_fixu: [
        0x01E0_087C, 0x01E0_0CA8, 0x01E0_0D0C, 0x01E0_13B0, 0x01E0_15F0, 0x01E0_1BA8, 0x01E0_1CF4, 0x01E0_21CC, 0x01E0_23AC, 0x01E0_2688, 0x01E0_2968,
        0x01E0_29F8, 0x01E0_2C6C, 0x01E0_2CE8, 0x01E0_2E34, 0x01E0_34E8, 0x01E0_3744,
    ],
    r_pn: [0x01E0_2954, 0x01E0_29E4],
    x: X { sp_blocks: 2, mote_blocks: 2, ..X0 },
    ..B744
};

/// b031's entry 0x1E00080.
pub fn program_b031(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B031)
}

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B030)
}

/// b167's entry 0x1E00080.
pub fn program_b167(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B167)
}

/// b382's entry 0x1E00080.
pub fn program_b382(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B382)
}

/// b441 (card 478; helper C28): b009's code in b030's 0x7B0 frame (the
/// scratch unshifted) with the attacker's nodes 0x21 / 0x20, a blue tint
/// (80, BE, FF), the fan window to 0x11E, its own (mostly reversed, blue)
/// colours, fleck length 0.9 and factor 2.5, sparks / motes from 0x49 and
/// in over 2, embers through ALPHA preset 1 at 48.
const B441: V = V {
    frame: 0x7B0,
    fs: 0,
    nodes: &[(0x21, 1), (0x20, 0)],
    tint: [0x80, 0xBE, 0xFF],
    fans: (1, 0x11E),
    fan_colours: [[0x30, 0x58, 0xFF], [0, 0, 0xFF]],
    fleck_len: 0x3F66_6666,
    fleck_k: Some(2.5),
    fleck_colour: [0x32, 0x64, 0xFF],
    sparks: (0x49, 0xDE),
    spark_below: 2.0,
    mote_below: 2.0,
    spark_colour: [0x1C, 0x30, 0x80],
    mote_colour: [0x64, 0xA4, 0xFF],
    ring_colours: [[0x1A, 0x26, 0xE4], [0x82, 0xAE, 0xFF]],
    flat_colour: [0x64, 0xA4, 0xFF],
    lens2_colour: [0x58, 0x80, 0xBC],
    ember_alpha: 1,
    ember_amp: 48.0,
    ember_colour: [0x64, 0x8C, 0xC8],
    r_rand: [0x01E0_1168, 0x01E0_1698, 0x01E0_16B8, 0x01E0_1D90, 0x01E0_1DB0, 0x01E0_2358, 0x01E0_238C, 0x01E0_23C0, 0x01E0_30D0],
    r_fan2: [
        0x01E0_2CB4, 0x01E0_2CC4, 0x01E0_2D10, 0x01E0_2D1C, 0x01E0_2D68, 0x01E0_2D74, 0x01E0_2E44, 0x01E0_2E54, 0x01E0_2EC0, 0x01E0_2ECC, 0x01E0_2F38,
        0x01E0_2F44,
    ],
    r_fixu: [
        0x01E0_080C, 0x01E0_0C18, 0x01E0_0C8C, 0x01E0_1310, 0x01E0_1538, 0x01E0_1AD8, 0x01E0_1C24, 0x01E0_207C, 0x01E0_2238, 0x01E0_24CC, 0x01E0_2770,
        0x01E0_27F4, 0x01E0_2A4C, 0x01E0_2AC8, 0x01E0_2C14, 0x01E0_3278, 0x01E0_3498,
    ],
    r_pn: [0x01E0_275C, 0x01E0_27E0],
    ..B009
};

/// b441's entry 0x1E00080.
pub fn program_b441(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B441)
}

/// b374 (card 409; helper C28): b167's code with the attacker's node 0x19
/// (not lowered), a 0x139-update fan window and light, the pools to 0xF9
/// (sparks / motes from 0x54), its own colours and sizes, rings 20 / 50
/// at 0x6F..0xD9 (the first colour black), a 15-wide flat sprite from
/// 0x6E, the second lens on 0x6E (171 / 139, grey, to 0xAC). Returns 1 on
/// 0x64, 6 on 0x6F..0xF8, 2 on 0xF9, 8 on 0x13A.
const B374: V = V {
    nodes: &[(0x19, 1)],
    lower: false,
    fans: (1, 0x139),
    fan_colours: [[0xFF, 0x62, 0x30], [0xFF, 0x20, 0x08]],
    flecks: (1, 0xF9),
    fleck_len: 0x3F80_0000,
    sparks: (0x54, 0xF9),
    spark_amp: 80.0,
    spark_colour: [0xA8, 0x3A, 0x1C],
    mote_len: 0x3F66_6666,
    mote_size: Lin(0.5, 0.5),
    mote_colour: [0xA8, 0x3A, 0x1C],
    mote_sine_lin: Lin(f(0x3E4C_CCCD), f(0x3F4C_CCCD)),
    rings_win: (0x6F, 0xD9),
    ring_colours: [[0, 0, 0], [0xFF, 0x6C, 0x36]],
    ring_amp: 128.0,
    flat: (0x6E, 0xF9),
    lens2_at: 0x6E,
    lens2_ramp: (171.0, 139.0),
    lens2_end: 0xAC,
    lens2_colour: [0x80, 0x80, 0x80],
    embers_win: (1, 0xF9),
    ember_len: 0x3F80_0000,
    ember_k: 8.0,
    light: 0x139,
    one: 0x64,
    six: (0x6F, 0xF9),
    two: 0xF9,
    end: 0x13A,
    r_rand: [0x01E0_1180, 0x01E0_16D8, 0x01E0_16F8, 0x01E0_1DD0, 0x01E0_1DF0, 0x01E0_2398, 0x01E0_23CC, 0x01E0_2400, 0x01E0_3110],
    r_fan1: [
        0x01E0_0D4C, 0x01E0_0D5C, 0x01E0_0DA8, 0x01E0_0DB4, 0x01E0_0E00, 0x01E0_0E0C, 0x01E0_0EDC, 0x01E0_0EEC, 0x01E0_0F58, 0x01E0_0F64, 0x01E0_0FD0,
        0x01E0_0FDC,
    ],
    r_fan2: [
        0x01E0_2CDC, 0x01E0_2CEC, 0x01E0_2D38, 0x01E0_2D44, 0x01E0_2D90, 0x01E0_2D9C, 0x01E0_2E6C, 0x01E0_2E7C, 0x01E0_2EE8, 0x01E0_2EF4, 0x01E0_2F60,
        0x01E0_2F6C,
    ],
    r_fixu: [
        0x01E0_080C, 0x01E0_0C38, 0x01E0_0C9C, 0x01E0_1328, 0x01E0_1568, 0x01E0_1B18, 0x01E0_1C64, 0x01E0_20BC, 0x01E0_2278, 0x01E0_2500, 0x01E0_27A0,
        0x01E0_2824, 0x01E0_2A7C, 0x01E0_2AF8, 0x01E0_2C44, 0x01E0_32D0, 0x01E0_3514,
    ],
    r_pn: [0x01E0_278C, 0x01E0_2810],
    x: X { ring_r: (20.0, 50.0), flat_size: 15.0, ..X0 },
    ..B167
};

/// b374's entry 0x1E00080.
pub fn program_b374(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B374)
}

fn run(fx: &mut Fx, param: i32, v: &V) -> i32 {
    let sp = fx.enter(v.frame);
    fx.mem.write_u32(sp + v.x.d_slot, D);
    fx.mem.write_u32(sp + v.x.ret_slot, 0);
    let ret = if param >= 0 { init(fx, sp, v) } else { step(fx, sp + v.fs, v) };
    fx.leave(v.frame);
    ret
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
    for (i, &(node, clear)) in v.nodes.iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(slot_at(v, i as u32), r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, hd(v) + 0x30);
    let h = fx.mem.i32(D + 4);
    let r = crate::blib::afterimage::unit_radius(fx, h);
    fx.mem.setf(hd(v) + 0x70, ee::mul(1.5, r));
    for k in 0..10u32 {
        fx.mem.write_u8(hd(v) + 0x78 + 8 * k, 0);
    }
    for k in 0..0x20u32 {
        fx.mem.write_u8(hd(v) + 0xD0 + v.fleck_stride * k, 0);
    }
    for k in 0..0x30u32 {
        fx.mem.write_u8(hd(v) + v.sp_base + v.sp_stride * k, 0);
    }
    for k in 0..0x30u32 {
        fx.mem.write_u8(hd(v) + v.motes + mote_at(v).3 * k, 0);
    }
    // The ring points: 17 x (30 / 100 at k / 16 of a turn, the middle).
    for k in 0..0x11u32 {
        let p = hd(v) + v.ring_pts + 0x30 * k;
        let a = (k << 12) as i32;
        let s = fx.tsin(a);
        let c = fx.tcos(a);
        fx.mem.setf(p, ee::mul(v.x.ring_r.0, s));
        fx.mem.setf(p + 4, ee::mul(v.x.ring_r.0, c));
        fx.mem.write_u32(p + 8, 0);
        fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        fx.mem.setf(p + 0x10, ee::mul(v.x.ring_r.1, s));
        fx.mem.setf(p + 0x14, ee::mul(v.x.ring_r.1, c));
        fx.mem.write_u32(p + 0x18, 0);
        fx.mem.write_u32(p + 0x1C, 0x3F80_0000);
        fx.vu_add4(p + 0x20, p, p + 0x10);
        fx.vu_scale4(p + 0x20, p + 0x20, 0.5);
    }
    for k in 0..0x20u32 {
        fx.mem.write_u8(hd(v) + v.rings + 0x50 * k, 0);
    }
    fx.mem.write_u8(hd(v) + v.lens1, 0);
    for k in 0..0x20u32 {
        fx.mem.write_u8(hd(v) + v.embers + v.ember_stride * k, 0);
    }
    fx.mem.write_u8(hd(v) + v.lens2, 0);
    if let Some(b) = &v.x.bursts {
        // The clearing loop steps the D slot itself: 0x8770 / 0x8BE0 (and
        // leaves D + 0x8E0 in it).
        for k in 0..2u32 {
            fx.mem.write_u8(hd(v) + b.base + 0x470 * k, 0);
        }
        fx.mem.write_u32(sp + v.x.d_slot, D + 0x8E0);
    }
    0
}

/// A mote record's (angles, point, directions, stride): b030 (0x20, 0x40,
/// 0x50, 0xB0), b031's two blocks (0x38, 0x70, 0x90, 0x150); the cells
/// at +8.
fn mote_at(v: &V) -> (u32, u32, u32, u32) {
    let n = 6 * v.x.mote_blocks;
    let pos = (8 + 8 * n + 15) & !15;
    let dirs = pos + 0x10 * v.x.mote_blocks;
    (8 + 4 * n, pos, dirs, dirs + 0x10 * n)
}

/// The header / pool base D + `hs`.
fn hd(v: &V) -> u32 {
    D + v.hs
}

/// Node `i`'s request slot.
fn slot_at(v: &V, i: u32) -> u32 {
    match v.x.node_tab {
        Some(t) => D + t[i as usize].0,
        None => D + 0x1C + 4 * i,
    }
}

/// Pool `p`'s (flecks, sparks, motes, embers) spawn point, block `b`.
fn pool_pos(v: &V, p: usize, b: u32) -> u32 {
    match v.x.pool_at {
        Some(a) => D + a[p],
        None => node_at(v, v.pool_nodes[p] + b),
    }
}

/// Node `i`'s position.
fn node_at(v: &V, i: u32) -> u32 {
    if let Some(t) = v.x.node_tab {
        D + t[i as usize].1
    } else if v.nodes.len() == 1 {
        D + 0x20
    } else {
        D + 0x30 + 0x10 * i
    }
}

/// A record of `nb` blocks (flag, age, 8 nb cells, the blocks' points
/// on a 16-byte boundary, then 8 directions a block): block `b`'s point.
fn blk_pos(nb: u32, b: u32) -> u32 {
    ((8 + 8 * nb + 15) & !15) + 0x10 * b
}

/// Block `b`'s direction `j`.
fn blk_dir(nb: u32, b: u32, j: u32) -> u32 {
    blk_pos(nb, nb) + 0x80 * b + 0x10 * j
}

fn rem2(x: i32, m: i32) -> i32 {
    let r = x & (m - 1);
    if x < 0 && r != 0 {
        r - m
    } else {
        r
    }
}

/// 0x1A2B80 / 0x1A2C70 with their residue.
fn fixs(fx: &mut Fx, f: f32, ra: u32) -> i32 {
    fx.fix_residue(f, ra);
    ee::fixsfsi(f)
}
fn fix_at(fx: &mut Fx, a: u32, ra: u32) -> i32 {
    let f = fx.mem.f(a);
    fixs(fx, f, ra)
}
fn fixu(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

/// The sprite cell `b` (4 x n cells of 32) into UV0 / UV1 at sp + 0x658 /
/// 0x65A / 0x668 / 0x66A.
fn cell_uvs(fx: &mut Fx, sp: u32, b: i32) {
    let (c, r) = (rem2(b, 4), b >> 2);
    uv(fx, sp + 0x658, ((c << 9) & 0x3FFF) as u32);
    uv(fx, sp + 0x65A, ((r << 9) & 0x3FFF) as u32);
    uv(fx, sp + 0x668, (((32 * c + 32) << 4) & 0x3FFF) as u32);
    uv(fx, sp + 0x66A, (((32 * r + 32) << 4) & 0x3FFF) as u32);
}

fn rgba(fx: &mut Fx, at: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(at, c[0]);
    fx.mem.write_u8(at + 1, c[1]);
    fx.mem.write_u8(at + 2, c[2]);
    fx.mem.write_u8(at + 3, a);
    fx.mem.write_u32(at + 4, 0x3F80_0000);
}

fn copy8(fx: &mut Fx, to: u32, from: u32) {
    let (a, b) = (fx.mem.read_u32(from), fx.mem.read_u32(from + 4));
    fx.mem.write_u32(to, a);
    fx.mem.write_u32(to + 4, b);
}

/// The view-projection (x identity) at sp + 0x4F0.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x4F0);
    fx.identity(sp + 0x530);
    let id = fx.read_m4(sp + 0x530);
    fx.mul_into(Some(sp + 0x4F0), &id);
}

/// The sprite descriptor at sp + 0x640 (5 registers 0x42421, prim 0x26).
fn sprite_head(fx: &mut Fx, sp: u32) {
    let d = sp + 0x640;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
}

/// The sprite's corners (-s, -s, 0, 1) at sp + 0x5B0 and (s, s, 0, 1) at
/// sp + 0x5C0 turned by the camera (sp + 0x570).
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    let n = ee::neg(s);
    fx.mem.setf(sp + 0x5B0, n);
    fx.mem.setf(sp + 0x5B4, n);
    fx.mem.write_u32(sp + 0x5B8, 0);
    fx.mem.write_u32(sp + 0x5BC, 0x3F80_0000);
    fx.mem.setf(sp + 0x5C0, s);
    fx.mem.setf(sp + 0x5C4, s);
    fx.mem.write_u32(sp + 0x5C8, 0);
    fx.mem.write_u32(sp + 0x5CC, 0x3F80_0000);
    fx.vu_apply_to(sp + 0x5B0, sp + 0x570, sp + 0x5B0);
    fx.vu_apply_to(sp + 0x5C0, sp + 0x570, sp + 0x5C0);
}

/// The sprite at sp + 0x5D0 / 0x5E0 projected (0x204F70) and submitted at
/// the mean w.
fn draw(fx: &mut Fx, sp: u32, at: u32, ra: u32) {
    let d = sp + 0x640;
    if crate::fxlib2::project(fx, at, PROJ_OUT, sp + 0x4F0, d, sp + 0x7AC) {
        let w = fx.mem.f(sp + 0x7AC);
        let key = fixu(fx, w, ra) as i32;
        fx.submit(d, OT, key);
    }
}

/// The corners about sp + 0x5F0 (sp + 0x5D0 / 0x5E0), drawn.
fn draw_at(fx: &mut Fx, sp: u32, ra: u32) {
    fx.vu_add4(sp + 0x5D0, sp + 0x5B0, sp + 0x5F0);
    fx.vu_add4(sp + 0x5E0, sp + 0x5C0, sp + 0x5F0);
    draw(fx, sp, sp + 0x5D0, ra);
}

/// clamp(a, 0, m) x clamp(b, 0, m) / d.
fn window(a: i32, b: i32, m: i32, d: f32) -> f32 {
    ee::div(ee::mul(ee::from_i32(Fx::clamp_i(a, 0, m)), ee::from_i32(Fx::clamp_i(b, 0, m))), d)
}

/// The screen point of `p` (0x204D80 through sp + 0x4F0 into sp + 0x7A0):
/// x / y into `out` / `out` + 4 ((x16 >> 4) - 0x700); the Z (bits 32..55).
fn screen(fx: &mut Fx, sp: u32, p: u32, out: Option<u32>) -> u64 {
    let m = fx.read_m4(sp + 0x4F0);
    fx.project_at(p, &m, sp + 0x7A0, sp + 0x7AC);
    if let Some(o) = out {
        let x = (i32::from(fx.mem.read_u16(sp + 0x7A0)) >> 4) - 0x700;
        fx.mem.setf(o, ee::from_i32(x));
        let y = (i32::from(fx.mem.read_u16(sp + 0x7A2)) >> 4) - 0x700;
        fx.mem.setf(o + 4, ee::from_i32(y));
    }
    (fx.mem.read_u64(sp + 0x7A0) << 8) >> 40
}

/// The screen point at `o`: Z (0x17BC10) into +8, 1.0 into +0xC, x / y
/// clamped to 0..512.
fn screen_point(fx: &mut Fx, sp: u32, p: u32, o: u32) {
    let z = screen(fx, sp, p, Some(o));
    fx.mem.setf(o + 8, ee::from_i32(z as i32));
    fx.mem.write_u32(o + 0xC, 0x3F80_0000);
    for k in [0u32, 4] {
        if crate::exe::ee_lt(fx.mem.f(o + k), 0.0) {
            fx.mem.write_u32(o + k, 0);
        }
    }
    for k in [0u32, 4] {
        if !crate::exe::ee_le(fx.mem.f(o + k), 512.0) {
            fx.mem.write_u32(o + k, 0x4400_0000);
        }
    }
}

/// An XYZ2 at `p`: x / y halfwords of pixel (x, y), Z `z` and a zero top
/// byte.
fn xyz_z(fx: &mut Fx, p: u32, x: i32, y: i32, z: u64) {
    fx.mem.write_u16(p, ((x + 0x700) << 4) as u16);
    fx.mem.write_u16(p + 2, ((y + 0x700) << 4) as u16);
    let v = fx.mem.read_u64(p);
    fx.mem.write_u64(p, (v & KEEP) | (z << 32));
    fx.mem.write_u8(p + 7, 0);
}

/// One ring of the eight-triangle fan (descriptor sp + 0x698) about the
/// screen point `c`: texels the fan points, positions scaled by `f`, at
/// Z `z` (0xFFFFF8 far); `ras` the 0x1A2B80 return addresses.
fn fan(fx: &mut Fx, sp: u32, c: u32, f: f32, z: u64, ot: u32, ras: &[u32; 12]) {
    for k in 0..8u32 {
        let s1 = FAN + 0x10 * k;
        let x = fix_at(fx, c, ras[0]);
        let y = fix_at(fx, c + 4, ras[1]);
        uv(fx, sp + 0x6B0, ((x << 4) & 0x3FFF) as u32);
        uv(fx, sp + 0x6B2, ((y << 4) & 0x3FFF) as u32);
        let x = fix_at(fx, s1, ras[2]);
        let y = fix_at(fx, s1 + 4, ras[3]);
        uv(fx, sp + 0x6C8, ((x << 4) & 0x3FFF) as u32);
        uv(fx, sp + 0x6CA, ((y << 4) & 0x3FFF) as u32);
        let x = fix_at(fx, s1 + 0x10, ras[4]);
        let y = fix_at(fx, s1 + 0x14, ras[5]);
        uv(fx, sp + 0x6E0, ((x << 4) & 0x3FFF) as u32);
        uv(fx, sp + 0x6E2, ((y << 4) & 0x3FFF) as u32);
        fx.vu_sub4(sp + 0x5D0, s1, c);
        fx.vu_sub4(sp + 0x5E0, s1 + 0x10, c);
        fx.vu_scale4(sp + 0x5D0, sp + 0x5D0, f);
        fx.vu_scale4(sp + 0x5E0, sp + 0x5E0, f);
        fx.vu_add4(sp + 0x5D0, sp + 0x5D0, c);
        fx.vu_add4(sp + 0x5E0, sp + 0x5E0, c);
        let x = fix_at(fx, c, ras[6]);
        let y = fix_at(fx, c + 4, ras[7]);
        xyz_z(fx, sp + 0x6B8, x, y, z);
        let x = fix_at(fx, sp + 0x5D0, ras[8]);
        let y = fix_at(fx, sp + 0x5D4, ras[9]);
        xyz_z(fx, sp + 0x6D0, x, y, z);
        let x = fix_at(fx, sp + 0x5E0, ras[10]);
        let y = fix_at(fx, sp + 0x5E4, ras[11]);
        xyz_z(fx, sp + 0x6E8, x, y, z);
        fx.submit(sp + 0x698, ot, 0);
    }
}

/// The fan's triangle descriptor at sp + 0x698 (prim 0x33 | 0x800, 9
/// registers), alpha preset `a`, the lens CLAMP.
fn fan_desc(fx: &mut Fx, sp: u32, a: i32) {
    let d = sp + 0x698;
    fx.mem.write_u16(d + 2, 0x33);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, 0x4_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let p = fx.alpha_preset(a);
    fx.desc_alpha(d, p);
    fx.desc_clamp(d, LENS_CLAMP);
}


/// `l`.0 + `l`.1 x `x` / `d`.
fn lin(l: Lin, x: f32, d: f32) -> f32 {
    ee::add(l.0, ee::div(ee::mul(l.1, x), d))
}

fn in_window(n: i32, w: (i32, i32)) -> bool {
    n >= w.0 && n < w.1
}

fn step(fx: &mut Fx, sp: u32, v: &V) -> i32 {
    fx.camera_turn_roll(sp + 0x570);
    for i in 0..v.nodes.len() as u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(slot_at(v, i)));
        fx.node_position(h, slot, node_at(v, i));
    }
    if v.lower {
        let y = node_at(v, 0) + 4;
        let f = fx.mem.f(y);
        fx.mem.setf(y, ee::add(f, -2.0));
    }
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, hd(v) + 0x50);
    camera(fx, sp);
    screen_point(fx, sp, hd(v) + 0x50, hd(v) + 0x60);
    // The point 1.5 radii in front of the attacker: its Z.
    fx.mem.write_u32(sp + 0x5F0, 0);
    fx.mem.write_u32(sp + 0x5F4, 0);
    let r = fx.mem.f(hd(v) + 0x70);
    fx.mem.setf(sp + 0x5F8, ee::neg(r));
    fx.mem.write_u32(sp + 0x5FC, 0x3F80_0000);
    fx.vu_apply_to(sp + 0x5F0, sp + 0x570, sp + 0x5F0);
    fx.vu_add4(sp + 0x5F0, sp + 0x5F0, hd(v) + 0x50);
    let z = screen(fx, sp, sp + 0x5F0, None);
    fx.mem.write_u32(hd(v) + 0x74, z as u32);
    camera(fx, sp);
    screen_point(fx, sp, hd(v) + 0x30, hd(v) + 0x40);

    if fx.mem.i32(D) == v.lens1_at {
        fx.mem.write_u8(hd(v) + v.lens1, 1);
        fx.mem.write_u32(hd(v) + v.lens1 + 4, 0);
    }
    // The tint (sp + 0x610: prim 6, 3 registers) and the frame sprite
    // (sp + 0x640).
    let t = sp + 0x610;
    fx.mem.write_u16(t + 2, 6);
    fx.mem.write_u16(t, 3);
    fx.mem.write_u64(t + 8, 0x441);
    fx.mem.write_u32(t + 4, 0);
    let p = fx.alpha_preset(3);
    fx.desc_alpha(t, p);
    let d = sp + 0x640;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let p = fx.alpha_preset(0);
    fx.desc_alpha(d, p);
    fx.desc_clamp(d, LENS_CLAMP);
    // a0..a3 as 0x208400 leaves them.
    let a = [u32::from(fx.mem.read_u16(d + 2)), LENS_CLAMP as u32, d + 0x10, d + 2];
    let fl = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, fl | 0x800);
    lens1(fx, sp, v, a);

    let n = fx.mem.i32(D);
    if in_window(n, v.fans) {
        let r = hd(v) + 0x78 + 8 * fx.div(n, 10).1 as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
    }
    fans(fx, sp, v);

    let n = fx.mem.i32(D);
    if in_window(n, v.flecks) {
        let r = hd(v) + 0xD0 + v.fleck_stride * rem2(n, 0x20) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        let nb = v.fleck_blocks;
        for bk in 0..nb {
            for c in 0..4u32 {
                let w = fx.mem.read_u32(pool_pos(v, 0, bk) + 4 * c);
                fx.mem.write_u32(r + blk_pos(nb, bk) + 4 * c, w);
            }
            for j in 0..8u32 {
                let b = fx.rand_from(OT, v.r_rand[0]) >> 23;
                fx.mem.set8(r + 8 + 8 * bk + j, rem2(b, 4));
                fx.random_table_vec(r + blk_dir(nb, bk, j), f(v.fleck_len));
            }
        }
    }
    flecks(fx, sp, v);

    let n = fx.mem.i32(D);
    if in_window(n, v.sparks) {
        let r = hd(v) + v.sp_stride * fx.div(n, 0x30).1 as u32;
        fx.mem.write_u8(r + v.sp_base, 1);
        fx.mem.write_u32(r + v.sp_base + 4, 0);
        let nb = v.x.sp_blocks;
        for bk in 0..nb {
            for c in 0..4u32 {
                let w = fx.mem.read_u32(pool_pos(v, 1, bk) + 4 * c);
                fx.mem.write_u32(r + v.sp_pos + 0x10 * bk + 4 * c, w);
            }
            for j in 0..v.sp_n {
                let m = nb * j + bk;
                let b = fx.rand_from(OT, v.r_rand[1]) >> 23;
                fx.mem.set32(r + v.sp_base + 8 + 4 * m, rem2(b, 8));
                let b = fx.rand_from(OT, v.r_rand[2]) >> 23;
                let x = ee::div(ee::mul(0.5, ee::from_i32(rem2(b, 0x100))), 256.0);
                // 0x1A2C30 / 0x1A3118 / 0x1A38F8: (float) (0.5 + (double) x).
                let s = (0.5f64 + f64::from(x)) as f32;
                let dir = r + v.sp_dirs + 0x10 * m;
                fx.random_table_vec(dir, v.spark_len);
                fx.mem.setf(r + v.sp_sizes + 4 * m, s);
                let a = Fx::powf2(fx.mem.f(dir));
                let b = Fx::powf2(fx.mem.f(dir + 4));
                let l = fx.sqrtf(ee::add(a, b));
                let q = fx.sqrtf(ee::mul(2.0, Fx::powf2(v.spark_pow)));
                let k = ee::mul(f(0x3E4C_CCCD), ee::sub(q, l));
                let q = fx.sqrtf(ee::mul(2.0, Fx::powf2(v.spark_pow)));
                fx.mem.setf(r + v.sp_alphas + 4 * m, ee::add(f(0x3F4C_CCCD), ee::div(k, q)));
            }
        }
    }
    sparks(fx, sp, v);

    let n = fx.mem.i32(D);
    if in_window(n, v.sparks) {
        let (ang, pos, dirs, stride) = mote_at(v);
        let r = hd(v) + v.motes + stride * fx.div(n, 0x30).1 as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        let nb = v.x.mote_blocks;
        for bk in 0..nb {
            for c in 0..4u32 {
                let w = fx.mem.read_u32(pool_pos(v, 2, bk) + 4 * c);
                fx.mem.write_u32(r + pos + 0x10 * bk + 4 * c, w);
            }
            for j in 0..6u32 {
                let m = nb * j + bk;
                let b = fx.rand_from(OT, v.r_rand[3]) >> 23;
                fx.mem.set32(r + 8 + 4 * m, rem2(b, 4));
                let b = fx.rand_from(OT, v.r_rand[4]) >> 23;
                fx.mem.set32(r + ang + 4 * m, (rem2(b, 0x100) << 16) >> 8);
                fx.random_table_vec(r + dirs + 0x10 * m, f(v.mote_len));
            }
        }
    }
    motes(fx, sp, v);

    let n = fx.mem.i32(D);
    if in_window(n, v.rings_win) {
        let r = hd(v) + v.rings + 0x50 * rem2(n, 0x20) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        let m = r + 0x10;
        fx.identity(m);
        for (axis, ra) in [(1, v.r_rand[5]), (0, v.r_rand[6]), (2, v.r_rand[7])] {
            let b = fx.rand_from(OT, ra) >> 23;
            fx.rotate_axis(m, (rem2(b, 0x100) << 16) >> 8, axis);
        }
    }
    rings(fx, sp, v);
    flat(fx, sp, v);
    lens2(fx, sp, v);

    let n = fx.mem.i32(D);
    if in_window(n, v.embers_win) {
        let r = hd(v) + v.embers + v.ember_stride * rem2(n, 0x20) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        let nb = v.ember_blocks;
        for bk in 0..nb {
            for c in 0..4u32 {
                let w = fx.mem.read_u32(pool_pos(v, 3, bk) + 4 * c);
                fx.mem.write_u32(r + blk_pos(nb, bk) + 4 * c, w);
            }
            for j in 0..8u32 {
                let b = fx.rand_from(OT, v.r_rand[8]) >> 23;
                fx.mem.set8(r + 8 + 8 * bk + j, rem2(b, 8));
                fx.random_table_vec(r + blk_dir(nb, bk, j), f(v.ember_len));
            }
        }
    }
    embers(fx, sp, v);
    bursts(fx, sp, v);

    let n = fx.mem.i32(D);
    let s0 = Fx::clamp_i(v.light - n, 0, 0x20);
    let k = Fx::clamp_i(n, 0, 0x20);
    let l = ((k.wrapping_mul(-3) << 5).wrapping_mul(s0) >> 10) + 0x80;
    fx.set_stage_light(l & 0xFF);

    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == v.one {
        ret = 1;
    } else if in_window(n, v.six) {
        ret = 6;
    }
    if n == v.two {
        ret = 2;
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= v.end {
        ret = 8;
    }
    fx.mem.write_u32(sp - v.fs + v.x.ret_slot, ret as u32);
    ret
}

/// The lens (48 / 32 updates) [F 0x1E00740..0x1E00A00].
fn lens1(fx: &mut Fx, sp: u32, v: &V, a: [u32; 4]) {
    let l = hd(v) + v.lens1;
    if fx.mem.read_u8(l) == 0 {
        return;
    }
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp - v.fs + v.x.layer_slot, layer);
    let regs = crate::lens::frame_copy(fx, a, 0, sp + 0x680, sp + 0x688);
    fx.texflush(regs);
    let age = fx.mem.i32(l + 4);
    let t = ee::div(ee::from_i32(v.lens1_life - age), ee::from_i32(v.lens1_life));
    let al = fixu(fx, ee::mul(128.0, t), v.r_fixu[0]) as u8;
    rgba(fx, sp + 0x620, v.tint, al);
    far_xy(fx, sp + 0x628, 0x7000, 0x7000);
    far_xy(fx, sp + 0x630, 0x9000, 0x9000);
    fx.submit(sp + 0x610, 0, 0);
    rgba(fx, sp + 0x650, [0x80, 0x80, 0x80], 0x80u8.wrapping_sub(al));
    uv(fx, sp + 0x658, 0);
    uv(fx, sp + 0x65A, 0);
    uv(fx, sp + 0x668, 0x2000);
    uv(fx, sp + 0x66A, 0x2000);
    far_xy(fx, sp + 0x660, 0x6FF8, 0x6FF8);
    far_xy(fx, sp + 0x670, 0x8FF8, 0x8FF8);
    fx.submit(sp + 0x640, 0, 0);
    let age = fx.mem.i32(l + 4) + 1;
    fx.mem.set32(l + 4, age);
    if age >= v.lens1_life + 1 {
        fx.mem.write_u8(l, 0);
    }
    let layer = fx.mem.read_u32(sp - v.fs + v.x.layer_slot);
    fx.layer_end(layer, OT, 0);
}

/// The ten fans (D+0x78, 11 updates) [F 0x1E00A44..0x1E010A4].
fn fans(fx: &mut Fx, sp: u32, v: &V) {
    fan_desc(fx, sp, 1);
    let n = fx.mem.i32(D);
    let a0 = Fx::clamp_i(n - (v.fans.0 - 1), 0, 0x40);
    let b0 = Fx::clamp_i(v.fans.1 - n, 0, 0x40);
    let f21 = ee::div(ee::mul(ee::from_i32(a0), ee::from_i32(b0)), 4096.0);
    let f20 = ee::mul(f21, 0.0);
    // a0..a3 as the second 0x101050 leaves them.
    let a = [b0 as u32, 0, 0x40, sp + 0x69A];
    let mut open = false;
    for k in 0..10u32 {
        let r = hd(v) + 0x78 + 8 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        if !open {
            open = true;
            let layer = fx.layer_begin();
            fx.mem.write_u32(sp - v.fs + v.x.layer_slot, layer);
            fx.texflush(a);
            let h = fx.mem.i32(D + 4);
            let (_, regs) = fx.unit_draw(h, 1, sp + 0x6F8, sp + 0x700);
            fx.texflush(regs);
        }
        let x = (fx.mem.i32(r + 4) + 1) << 15;
        let s = fx.tsin(x.wrapping_div(11));
        let al = fixu(fx, ee::mul(f21, ee::mul(255.0, s)), v.r_fixu[1]) as u8;
        rgba(fx, sp + 0x6A8, v.fan_colours[0], al);
        let x = (fx.mem.i32(r + 4) + 1) << 15;
        let s = fx.tsin(x.wrapping_div(11));
        let al = match v.fan2_amp {
            Some(amp) => fixu(fx, ee::mul(f21, ee::mul(amp, s)), v.r_fixu[2]) as u8,
            None => fixu(fx, f20, v.r_fixu[2]) as u8,
        };
        rgba(fx, sp + 0x6C0, v.fan_colours[1], al);
        copy8(fx, sp + 0x6D8, sp + 0x6C0);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let t = ee::div(ee::mul(f(0x3E4C_CCCD), age), 10.0);
        let t = ee::add(1.0, ee::add(f(0xBDCC_CCCD), t));
        let f22 = ee::add(f(0x3C23_D70A), t);
        let z = u64::from(fx.mem.read_u32(hd(v) + 0x74) & 0xFF_FFFF);
        fan(fx, sp, hd(v) + 0x60, f22, z, 0, &v.r_fan1);
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0xB {
            fx.mem.write_u8(r, 0);
        }
    }
    if open {
        let layer = fx.mem.read_u32(sp - v.fs + v.x.layer_slot);
        fx.layer_end(layer, OT, 0);
    }
}

/// The 32 x 8 flecks (D+0xD0, 0xA0) [F 0x1E011C8..0x1E015E8].
fn flecks(fx: &mut Fx, sp: u32, v: &V) {
    sprite_head(fx, sp);
    let d = sp + 0x640;
    fx.desc_clamp(d, 5);
    if !v.fleck_per_record {
        let rh = fx.mem.read_u32(D + 0x10);
        fx.desc_texture(d, rh, 1);
        let p = fx.alpha_preset(1);
        fx.desc_alpha(d, p);
    }
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f20 = window(n - (v.flecks.0 - 1), v.flecks.1 - n, 0x20, 1024.0);
    for k in 0..0x20u32 {
        let r = hd(v) + 0xD0 + v.fleck_stride * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        if v.fleck_per_record {
            let rh = fx.mem.read_u32(D + 0x10);
            fx.desc_texture(d, rh, 1);
            let p = fx.alpha_preset(1);
            fx.desc_alpha(d, p);
        }
        let age = fx.mem.i32(r + 4);
        let s = fx.tsin((age << 15) >> 5);
        let al = fixu(fx, ee::mul(f20, ee::mul(255.0, s)), v.r_fixu[3]) as u8;
        let age = fx.mem.i32(r + 4);
        let t = ee::div(ee::mul(f(0x3F4C_CCCD), ee::from_i32(0x20 - age)), 32.0);
        let s = ee::add(f(0x3E4C_CCCD), t);
        corners(fx, sp, v.fleck_k.map_or(s, |k| ee::mul(k, s)));
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let spread = lin(v.fleck_spread, age, 32.0);
        let rise = v.fleck_rise.map(|l| lin(l, age, 32.0));
        let nb = v.fleck_blocks;
        for bk in 0..nb {
            rgba(fx, sp + 0x650, v.fleck_colour, al);
            for j in 0..8u32 {
                let b = i32::from(fx.mem.read_u8(r + 8 + 8 * bk + j));
                cell_uvs(fx, sp, b);
                fx.vu_scale4(sp + 0x5F0, r + blk_dir(nb, bk, j), spread);
                fx.vu_add4(sp + 0x5F0, sp + 0x5F0, r + blk_pos(nb, bk));
                if let Some(rise) = rise {
                    let y = fx.mem.f(sp + 0x5F4);
                    fx.mem.setf(sp + 0x5F4, ee::add(y, rise));
                }
                draw_at(fx, sp, v.r_fixu[4]);
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The arc -15 sin(32768 `pull`) (0x1A2B80 from `ra`, 0x20ACC0).
fn arc(fx: &mut Fx, pull: f32, ra: u32) -> f32 {
    let a = fixs(fx, ee::mul(32768.0, pull), ra) & 0xFFFF;
    ee::mul(-15.0, fx.tsin(a))
}

/// The 48 x 8 (12) sparks (D+0x14D0) [F 0x1E01840..0x1E01CD8].
fn sparks(fx: &mut Fx, sp: u32, v: &V) {
    sprite_head(fx, sp);
    let d = sp + 0x640;
    let p = fx.alpha_preset(1);
    fx.desc_alpha(d, p);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f21 = ee::div(ee::from_i32(Fx::clamp_i(v.sparks.1 - n, 0, 0x20)), 32.0);
    for k in 0..0x30u32 {
        let r = hd(v) + v.sp_stride * k;
        if fx.mem.read_u8(r + v.sp_base) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(r + v.sp_base + 4));
        corners(fx, sp, ee::mul(v.spark_k, lin(v.spark_size, age, 48.0)));
        let age = ee::from_i32(fx.mem.i32(r + v.sp_base + 4));
        let pull = ee::div(ee::mul(f(0x3FD9_999A), age), 48.0);
        let spread = lin(v.spark_spread, age, 48.0);
        let rise = if v.arc { Some(arc(fx, pull, v.r_arc[0])) } else { None };
        let nb = v.x.sp_blocks;
        for bk in 0..nb {
            let at = r + v.sp_pos + 0x10 * bk;
            fx.vu_sub4(sp + 0x600, hd(v) + 0x30, at);
            fx.vu_scale4(sp + 0x600, sp + 0x600, pull);
            fx.vu_add4(sp + 0x600, sp + 0x600, at);
            if let Some(rise) = rise {
                let y = fx.mem.f(sp + 0x604);
                fx.mem.setf(sp + 0x604, ee::add(y, rise));
            }
            for j in 0..v.sp_n {
                let m = nb * j + bk;
                let age = ee::from_i32(fx.mem.i32(r + v.sp_base + 4));
                let f22 = ee::mul(v.spark_amp, Fx::ratio_below(age, v.spark_below));
                let age = ee::from_i32(fx.mem.i32(r + v.sp_base + 4));
                let f0 = Fx::ramp_above(age, 48.0, 32.0);
                let w = fx.mem.f(r + v.sp_alphas + 4 * m);
                let al = fixu(fx, ee::mul(w, ee::mul(f21, ee::mul(f22, f0))), v.r_fixu[5]) as u8;
                rgba(fx, sp + 0x650, v.spark_colour, al);
                let b = fx.mem.i32(r + v.sp_base + 8 + 4 * m);
                cell_uvs(fx, sp, b);
                let s = fx.mem.f(r + v.sp_sizes + 4 * m);
                fx.vu_scale4(sp + 0x5D0, sp + 0x5B0, s);
                let s = fx.mem.f(r + v.sp_sizes + 4 * m);
                fx.vu_scale4(sp + 0x5E0, sp + 0x5C0, s);
                fx.vu_scale4(sp + 0x5F0, r + v.sp_dirs + 0x10 * m, spread);
                fx.vu_add4(sp + 0x5F0, sp + 0x5F0, sp + 0x600);
                fx.vu_add4(sp + 0x5D0, sp + 0x5D0, sp + 0x5F0);
                fx.vu_add4(sp + 0x5E0, sp + 0x5E0, sp + 0x5F0);
                draw(fx, sp, sp + 0x5D0, v.r_fixu[6]);
            }
        }
        let age = fx.mem.i32(r + v.sp_base + 4) + 1;
        fx.mem.set32(r + v.sp_base + 4, age);
        if age >= 0x31 {
            fx.mem.write_u8(r + v.sp_base, 0);
        }
    }
}

/// The 48 x 6 motes (0xB0) [F 0x1E01E40..0x1E022B8].
fn motes(fx: &mut Fx, sp: u32, v: &V) {
    sprite_head(fx, sp);
    let d = sp + 0x640;
    let p = fx.alpha_preset(1);
    fx.desc_alpha(d, p);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f21 = ee::div(ee::from_i32(Fx::clamp_i(v.sparks.1 - n, 0, 0x20)), 32.0);
    let (ang, pos, dirs, stride) = mote_at(v);
    for k in 0..0x30u32 {
        let r = hd(v) + v.motes + stride * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(r + 4));
        corners(fx, sp, ee::mul(v.mote_k, lin(v.mote_size, age, 48.0)));
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let pull = ee::div(ee::mul(f(0x3FD9_999A), age), 48.0);
        let rise = if v.arc { Some(arc(fx, pull, v.r_arc[1])) } else { None };
        let nb = v.x.mote_blocks;
        for bk in 0..nb {
            let at = r + pos + 0x10 * bk;
            fx.vu_sub4(sp + 0x600, hd(v) + 0x30, at);
            fx.vu_scale4(sp + 0x600, sp + 0x600, pull);
            fx.vu_add4(sp + 0x600, sp + 0x600, at);
            if let Some(rise) = rise {
                let y = fx.mem.f(sp + 0x604);
                fx.mem.setf(sp + 0x604, ee::add(y, rise));
            }
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let f22 = ee::mul(v.mote_amp, Fx::ratio_below(age, v.mote_below));
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let f0 = Fx::ramp_above(age, 48.0, 32.0);
            let al = fixu(fx, ee::mul(f21, ee::mul(f22, f0)), v.r_fixu[7]) as u8;
            rgba(fx, sp + 0x650, v.mote_colour, al);
            for j in 0..6u32 {
                let m = nb * j + bk;
                let b = fx.mem.i32(r + 8 + 4 * m);
                let age_i = fx.mem.i32(r + 4);
                let age = ee::from_i32(age_i);
                let spread = if v.mote_sine {
                    let s = lin(v.mote_sine_lin, age, 48.0);
                    let a = fx.mem.i32(r + ang + 4 * m).wrapping_add((age_i << 15).wrapping_div(48)) & 0xFFFF;
                    ee::mul(s, fx.tsin(a))
                } else {
                    lin(Lin(f(0x3E4C_CCCD), f(0x3F4C_CCCD)), age, 48.0)
                };
                cell_uvs(fx, sp, b);
                fx.vu_scale4(sp + 0x5F0, r + dirs + 0x10 * m, spread);
                fx.vu_add4(sp + 0x5F0, sp + 0x5F0, sp + 0x600);
                draw_at(fx, sp, v.r_fixu[8]);
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x31 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The 32 rings (0x50) [F 0x1E023B4..0x1E02838].
fn rings(fx: &mut Fx, sp: u32, v: &V) {
    let d = sp + 0x710;
    fx.mem.write_u16(d + 2, 0x34);
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    let p = fx.alpha_preset(1);
    fx.desc_alpha(d, p);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 0);
    let pts = hd(v) + v.ring_pts;
    for k in 0..0x20u32 {
        let r = hd(v) + v.rings + 0x50 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 4);
        let s = fx.tsin((age << 15) >> 5);
        rgba(fx, sp + 0x720, v.ring_colours[0], 0);
        copy8(fx, sp + 0x738, sp + 0x720);
        let al = fixu(fx, ee::mul(v.ring_amp, s), v.r_fixu[9]) as u8;
        rgba(fx, sp + 0x750, v.ring_colours[1], al);
        copy8(fx, sp + 0x768, sp + 0x750);
        fx.camera_vp_to(sp + 0x4F0);
        let t = [fx.mem.f(hd(v) + 0x30), fx.mem.f(hd(v) + 0x34), fx.mem.f(hd(v) + 0x38)];
        let m = fx.trs(None, None, Some(t));
        fx.write_m4(sp + 0x530, &m);
        let rm = fx.read_m4(r + 0x10);
        fx.mul_into(Some(sp + 0x530), &rm);
        let s1 = 2 * rem2(k as i32, 2) - 1;
        let age = fx.mem.i32(r + 4);
        fx.rotate_axis(sp + 0x530, s1.wrapping_mul(55 * age), 0);
        let age = fx.mem.i32(r + 4);
        fx.rotate_axis(sp + 0x530, s1.wrapping_mul(77 * age), 1);
        let age = fx.mem.i32(r + 4);
        fx.rotate_axis(sp + 0x530, s1.wrapping_mul(99 * age), 2);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let s = lin(Lin(f(0x3ECC_CCCD), f(0x3F19_999A)), age, 32.0);
        fx.mem.setf(sp + 0x5F0, s);
        fx.mem.setf(sp + 0x5F4, s);
        fx.mem.write_u32(sp + 0x5F8, 0);
        fx.mem.write_u32(sp + 0x5FC, 0x3F80_0000);
        fx.scale_rows(sp + 0x530, sp + 0x5F0);
        let m = fx.read_m4(sp + 0x530);
        fx.mul_into(Some(sp + 0x4F0), &m);
        let age = fx.mem.i32(r + 4);
        let f20 = ee::div(ee::mul(-1.0, ee::from_i32(rem2(age, 16))), 16.0);
        let f21 = ee::add(0.5, f20);
        let f22 = ee::add(1.0, f20);
        for s7 in 0..0x10u32 {
            let (s1, fp) = (pts + 0x30 * s7, pts + 0x30 * (s7 + 1));
            let f23 = ee::div(ee::mul(8.0, ee::from_i32(s7 as i32)), 16.0);
            let f24 = ee::div(ee::mul(8.0, ee::from_i32(s7 as i32 + 1)), 16.0);
            for (o, x) in [(0x728u32, f23), (0x72C, f20), (0x740, f24), (0x744, f20), (0x758, f23), (0x75C, f21), (0x770, f24), (0x774, f21)] {
                fx.mem.setf(sp + o, x);
            }
            let vs = [s1, fp, s1 + 0x20, fp + 0x20];
            if fx.project_n(&vs, sp + 0x4F0, d, sp + 0x7AC, v.r_pn[0]) == 1 {
                let w = fx.mem.f(sp + 0x7AC);
                let key = fixu(fx, w, v.r_fixu[10]) as i32;
                fx.submit(d, OT, key);
            }
            for (o, x) in [(0x728u32, f23), (0x72C, f22), (0x740, f24), (0x744, f22), (0x758, f23), (0x75C, f21), (0x770, f24), (0x774, f21)] {
                fx.mem.setf(sp + o, x);
            }
            let vs = [s1 + 0x10, fp + 0x10, s1 + 0x20, fp + 0x20];
            if fx.project_n(&vs, sp + 0x4F0, d, sp + 0x7AC, v.r_pn[1]) == 1 {
                let w = fx.mem.f(sp + 0x7AC);
                let key = fixu(fx, w, v.r_fixu[11]) as i32;
                fx.submit(d, OT, key);
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The flat sprite at the target [F 0x1E02838..0x1E02AA0].
fn flat(fx: &mut Fx, sp: u32, v: &V) {
    let n = fx.mem.i32(D);
    let f20 = window(n - v.flat.0, v.flat.1 - n, 0x20, 1024.0);
    if crate::exe::ee_le(f20, 0.0) {
        return;
    }
    sprite_head(fx, sp);
    let d = sp + 0x640;
    let p = fx.alpha_preset(1);
    fx.desc_alpha(d, p);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    uv(fx, sp + 0x658, 0x400);
    uv(fx, sp + 0x65A, 0);
    uv(fx, sp + 0x668, 0x600);
    uv(fx, sp + 0x66A, 0x200);
    fx.mem.setf(sp + 0x5B0, ee::neg(v.x.flat_size));
    fx.mem.setf(sp + 0x5B4, ee::neg(v.x.flat_size));
    fx.mem.write_u32(sp + 0x5B8, 0);
    fx.mem.write_u32(sp + 0x5BC, 0x3F80_0000);
    fx.mem.setf(sp + 0x5C0, v.x.flat_size);
    fx.mem.setf(sp + 0x5C4, v.x.flat_size);
    fx.mem.write_u32(sp + 0x5C8, 0);
    fx.mem.write_u32(sp + 0x5CC, 0x3F80_0000);
    let n = fx.mem.i32(D);
    let f1 = ee::add(f(0x3F66_6666), ee::mul(f(0x3DCC_CCCD), ee::from_i32(rem2(n, 2))));
    let al = fixu(fx, ee::mul(ee::mul(255.0, f20), f1), v.r_fixu[12]) as u8;
    rgba(fx, sp + 0x650, v.flat_colour, al);
    fx.camera_vp_to(sp + 0x4F0);
    fx.translate_rows(sp + 0x530, sp + 0x570, hd(v) + 0x30);
    let m = fx.read_m4(sp + 0x530);
    fx.mul_into(Some(sp + 0x4F0), &m);
    draw(fx, sp, sp + 0x5B0, v.r_fixu[13]);
}

/// The second lens fan (no layer) [F 0x1E02AA0..0x1E02FF4].
fn lens2(fx: &mut Fx, sp: u32, v: &V) {
    let l = hd(v) + v.lens2;
    if fx.mem.i32(D) == v.lens2_at {
        fx.mem.write_u8(l, 1);
        fx.mem.write_u32(l + 4, 0);
    }
    if fx.mem.read_u8(l) != 1 {
        return;
    }
    fan_desc(fx, sp, 0);
    let d = sp + 0x698;
    fx.frame_desc(d, 1, [d + 0x10, d + 2]);
    let age = ee::from_i32(fx.mem.i32(l + 4));
    let f20 = Fx::ratio_below(age, 32.0);
    let age = ee::from_i32(fx.mem.i32(l + 4));
    let f0 = Fx::ramp_above(age, v.lens2_ramp.0, v.lens2_ramp.1);
    let al = fixu(fx, ee::mul(64.0, ee::mul(f20, f0)), v.r_fixu[14]) as u8;
    rgba(fx, sp + 0x6A8, v.lens2_colour, al);
    copy8(fx, sp + 0x6C0, sp + 0x6A8);
    copy8(fx, sp + 0x6D8, sp + 0x6A8);
    let s = ee::add(1.0, ee::mul(f(v.lens2_scale), 1.0));
    fan(fx, sp, hd(v) + 0x40, s, 0xFF_FFF8, OT, &v.r_fan2);
    let age = fx.mem.i32(l + 4) + 1;
    fx.mem.set32(l + 4, age);
    if age >= v.lens2_end {
        fx.mem.write_u8(l, 0);
    }
}

/// The 32 x 8 embers (0xA0) [F 0x1E03118..0x1E03530].
fn embers(fx: &mut Fx, sp: u32, v: &V) {
    sprite_head(fx, sp);
    let d = sp + 0x640;
    if v.x.ember_once {
        let rh = fx.mem.read_u32(D + 0x14);
        fx.desc_texture(d, rh, 1);
        let p = fx.alpha_preset(v.ember_alpha);
        fx.desc_alpha(d, p);
    }
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f20 = window(n - (v.embers_win.0 - 1), v.embers_win.1 - n, 0x20, 1024.0);
    for k in 0..0x20u32 {
        let r = hd(v) + v.embers + v.ember_stride * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        if !v.x.ember_once {
            let rh = fx.mem.read_u32(D + 0x14);
            fx.desc_texture(d, rh, 1);
            let p = fx.alpha_preset(v.ember_alpha);
            fx.desc_alpha(d, p);
        }
        let age = fx.mem.i32(r + 4);
        let sh = v.x.ember_shift;
        let c = if v.ember_sin { fx.tsin((age << sh) >> 5) } else { fx.tcos((age << sh) >> 5) };
        let al = fixu(fx, ee::mul(f20, ee::mul(v.ember_amp, c)), v.r_fixu[15]) as u8;
        let age = fx.mem.i32(r + 4);
        let x = ee::from_i32(if v.ember_rev { 0x20 - age } else { age });
        let s = ee::mul(v.ember_k, ee::add(0.5, ee::div(ee::mul(0.5, x), 32.0)));
        corners(fx, sp, s);
        let age = fx.mem.i32(r + 4);
        let spread = if v.ember_rev {
            ee::div(ee::from_i32(0x20 - age), 32.0)
        } else {
            lin(v.x.ember_spread, ee::from_i32(age), 32.0)
        };
        let lift = v.ember_lift.map(|l| lin(l, ee::from_i32(fx.mem.i32(r + 4)), 32.0));
        let nb = v.ember_blocks;
        for bk in 0..nb {
            rgba(fx, sp + 0x650, v.ember_colour, al);
            for j in 0..8u32 {
                let b = i32::from(fx.mem.read_u8(r + 8 + 8 * bk + j));
                cell_uvs(fx, sp, b);
                fx.vu_scale4(sp + 0x5F0, r + blk_dir(nb, bk, j), spread);
                fx.vu_add4(sp + 0x5F0, sp + 0x5F0, r + blk_pos(nb, bk));
                if v.ember_rise {
                    let y = fx.mem.f(sp + 0x5F4);
                    fx.mem.setf(sp + 0x5F4, ee::add(y, 0.0));
                }
                if let Some(l) = lift {
                    let y = fx.mem.f(sp + 0x5F4);
                    fx.mem.setf(sp + 0x5F4, ee::add(y, l));
                }
                draw_at(fx, sp, v.r_fixu[16]);
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// 0x1A7230 fabsf (its argument left at its sp).
fn fabsf(fx: &mut Fx, x: f32) -> f32 {
    fx.mem.write_f32(fx.sp - 0x10, x);
    f32::from_bits(x.to_bits() & 0x7FFF_FFFF)
}

/// b755's two bursts [F 0x1E03630..0x1E03BC8]: on `at` a record of two
/// blocks of 32 sprites at the nodes 2 a + 1 / 2 a + 2 (directions of
/// length 0.8 with y = 0.8 |y|), drawn 32 updates (alpha 64 cos((age <<
/// 14) >> 5), size k (0.5 + 0.5 age / 32), spread by 0x100D50's falloff
/// of the age, key 0).
fn bursts(fx: &mut Fx, sp: u32, v: &V) {
    let Some(b) = &v.x.bursts else { return };
    let n = fx.mem.i32(D);
    if n == b.at[0] || n == b.at[1] {
        let a = u32::from(n == b.at[0]);
        let r = hd(v) + b.base + 0x470 * a;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for bk in 0..2u32 {
            let p = node_at(v, b.node + 2 * a + bk);
            for c in 0..4u32 {
                let w = fx.mem.read_u32(p + 4 * c);
                fx.mem.write_u32(r + 0x50 + 0x10 * bk + 4 * c, w);
            }
            for j in 0..0x20u32 {
                let c = fx.rand_from(OT, b.r_rand) >> 23;
                fx.mem.set8(r + 8 + 0x20 * bk + j, rem2(c, 8));
                let d = r + 0x70 + 0x200 * bk + 0x10 * j;
                fx.random_table_vec(d, b.dir_len);
                let y = fx.mem.f(d + 4);
                let y = fabsf(fx, y);
                fx.mem.setf(d + 4, y);
                fx.mem.setf(d + 4, ee::mul(y, f(0x3F4C_CCCD)));
            }
        }
    }
    sprite_head(fx, sp);
    let d = sp + 0x640;
    let p = fx.alpha_preset(0);
    fx.desc_alpha(d, p);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    for k in 0..2u32 {
        let r = hd(v) + b.base + 0x470 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 4);
        let c = fx.tcos((age << 14) >> 5);
        let al = fixu(fx, ee::mul(b.amp, c), b.r_fixu) as u8;
        rgba(fx, sp + 0x650, b.colour, al);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        corners(fx, sp, ee::mul(b.k, ee::add(0.5, ee::div(ee::mul(0.5, age), 32.0))));
        let age = fx.mem.i32(r + 4);
        let f20 = fx.tan_falloff_p(age, 0x20, 1);
        for bk in 0..2u32 {
            for j in 0..0x20u32 {
                fx.vu_scale4(sp + 0x5F0, r + 0x70 + 0x200 * bk + 0x10 * j, f20);
                fx.vu_add4(sp + 0x5F0, sp + 0x5F0, r + 0x50 + 0x10 * bk);
                let y = fx.mem.f(sp + 0x5F4);
                fx.mem.setf(sp + 0x5F4, ee::add(y, 0.0));
                fx.vu_add4(sp + 0x5D0, sp + 0x5B0, sp + 0x5F0);
                fx.vu_add4(sp + 0x5E0, sp + 0x5C0, sp + 0x5F0);
                let c = i32::from(fx.mem.read_u8(r + 8 + 0x20 * bk + j));
                cell_uvs(fx, sp, c);
                if crate::fxlib2::project(fx, sp + 0x5D0, PROJ_OUT, sp + 0x4F0, d, sp + 0x7AC) {
                    fx.submit(d, OT, 0);
                }
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// b755 (card 266, helper B31): b009's code in a 0x870 frame (D at sp +
/// 0xE0, the return at sp + 0xC0, the layer at sp + 0xDC, the scratch
/// 0xC0 higher than b030's), the header 0x50 higher, five nodes (0x3C
/// at D+0x20; 0x1E / 0x28 / 0x51 / 0x5E cleared 0 at D+0x40..0x70),
/// flecks / embers at the attacker's point rising -10 + 30 age / 32,
/// sparks / motes at node 0x3C, rings of radii 15 / 50, a 30-wide flat
/// sprite, embers textured once (alpha 1, 48 sin), and two bursts at
/// D+0x8770 on 0xE3 / 0xDA [F].
const B755: V = V {
    bzero: 0x9050,
    frame: 0x870,
    fs: 0xC0,
    hs: 0x50,
    nodes: &[(0x3C, 1), (0x1E, 0), (0x28, 0), (0x51, 0), (0x5E, 0)],
    pool_nodes: [0; 4],
    lens1_at: 0x7D,
    tint: [0x94, 0x94, 0xFF],
    fans: (1, 0x27A),
    fan_colours: [[0x44, 0x44, 0xFF], [0, 0, 0xFF]],
    fan2_amp: Some(255.0),
    flecks: (1, 0x23A),
    fleck_len: 0x3FF3_3333,
    fleck_per_record: false,
    fleck_rise: Some(Lin(-10.0, 30.0)),
    fleck_colour: [0x4B, 0x4B, 0xFF],
    sparks: (0xF4, 0x23A),
    spark_len: f(0x3F0C_CCCD),
    spark_pow: f(0x410C_CCCD),
    spark_k: 9.0,
    spark_colour: [0x26, 0x26, 0x80],
    mote_len: 0x3EE6_6666,
    mote_k: 5.0,
    mote_colour: [0x86, 0x86, 0xFF],
    rings_win: (0x113, 0x21A),
    ring_colours: [[0x20, 0x20, 0xE4], [0x9A, 0x9A, 0xFF]],
    flat: (0x112, 0x23A),
    flat_colour: [0x86, 0x86, 0xFF],
    lens2_at: 0x198,
    lens2_ramp: (194.0, 162.0),
    lens2_end: 0xC3,
    lens2_colour: [0x6C, 0x6C, 0xBC],
    embers_win: (1, 0x23A),
    ember_len: 0x3FD9_999A,
    ember_alpha: 1,
    ember_sin: true,
    ember_amp: 48.0,
    ember_lift: Some(Lin(-10.0, 30.0)),
    ember_colour: [0x78, 0x78, 0xC8],
    light: 0x27A,
    one: 0x108,
    six: (0x113, 0x23A),
    two: 0x23A,
    end: 0x27B,
    r_rand: [0x01E0_1208, 0x01E0_1770, 0x01E0_1790, 0x01E0_1E60, 0x01E0_1E80, 0x01E0_2420, 0x01E0_2454, 0x01E0_2488, 0x01E0_3188],
    r_fan1: [
        0x01E0_0DD4, 0x01E0_0DE4, 0x01E0_0E30, 0x01E0_0E3C, 0x01E0_0E88, 0x01E0_0E94, 0x01E0_0F64, 0x01E0_0F74, 0x01E0_0FE0, 0x01E0_0FEC, 0x01E0_1058,
        0x01E0_1064,
    ],
    r_fan2: [
        0x01E0_2D6C, 0x01E0_2D7C, 0x01E0_2DC8, 0x01E0_2DD4, 0x01E0_2E20, 0x01E0_2E2C, 0x01E0_2EFC, 0x01E0_2F0C, 0x01E0_2F78, 0x01E0_2F84, 0x01E0_2FF0,
        0x01E0_2FFC,
    ],
    r_fixu: [
        0x01E0_08BC, 0x01E0_0CC0, 0x01E0_0D30, 0x01E0_13B4, 0x01E0_1610, 0x01E0_1BB0, 0x01E0_1CF8, 0x01E0_214C, 0x01E0_2300, 0x01E0_2590, 0x01E0_2830,
        0x01E0_28B4, 0x01E0_2B0C, 0x01E0_2B84, 0x01E0_2CD0, 0x01E0_3334, 0x01E0_3578,
    ],
    r_pn: [0x01E0_281C, 0x01E0_28A0],
    x: X {
        d_slot: 0xE0,
        ret_slot: 0xC0,
        layer_slot: 0xDC,
        node_tab: Some(&[(0x1C, 0x20), (0x30, 0x40), (0x34, 0x50), (0x38, 0x60), (0x3C, 0x70)]),
        pool_at: Some([0xA0, 0x20, 0x20, 0xA0]),
        ring_r: (15.0, 50.0),
        flat_size: 30.0,
        ember_once: true,
        ember_shift: 15,
        ember_spread: Lin(0.5, 0.5),
        bursts: Some(Bursts {
            base: 0x8720,
            at: [0xDA, 0xE3],
            node: 1,
            dir_len: f(0x3F4C_CCCD),
            k: 7.0,
            amp: 64.0,
            colour: [0xEE, 0xEE, 0xEE],
            r_rand: 0x01E0_3710,
            r_fixu: 0x01E0_38D4,
        }),
        ..X0
    },
    ..B009
};

/// b755's entry 0x1E00080.
pub fn program_b755(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B755)
}
