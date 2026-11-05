//! BMODULE b554 (card 15) and its family members b560 (card 32) / b006
//! (card 22; eight heads in n % 5 windows, a fourth texture shifting the
//! header 0x10, the frame's scratch 0x10 higher, 16-update rings and a
//! two-pass mote pool in place of the sparks),
//! hand-ported from the records (linked at 0x1E00000, one function
//! 0x1E00080, data area a0 = 0x1ED0000, 0x7B90 / 0x7710 bytes) [F]
//!: a single unscheduled function in a 0x660
//! frame (s0 = the ordering table), b188's head / puff / smoke idiom.
//! Every update the camera turn with roll (0x2055E0 into sp + 0x460), the
//! attacker's node 0x22 (b560 0x26) into D+0x20, node 0xA (0x21) into
//! D+0x40 and its point (0x1F65D0 kind 1) into D+0x50. On updates 0x87 +
//! 4 c (b560 0x6E) one of three heads (one; D+0x80, 0x30) leaves the node
//! towards the target (x kept) with a ring record (D+0x2E20.., 0x350: 16
//! sprites thrown 1, 32 thrown 2). A head (sprite 10 / 8) leaves every
//! update a puff (8 a head, 8 updates, 4 / 16 sprites thrown 0.5 / 0.3)
//! and a smoke (32 a head, 32 updates, 4 / 16 sprites thrown 1 / 0.8).
//! The ring: a far glow (10) and 16 + 32 sprites. In the flash window
//! (0xD4 / 0x83 .. 0x104) a flash record a update (16 x 0x60: the target,
//! a random turn): a scrolling 16 x 2 quad ring (ST) between radii 25 /
//! 70 (15 / 50) far Z and a far glow (90 / 40). In the wave window (0x3F
//! .. 0x101 / 0xAA) a wave record a update (16 x 0x40: eight random
//! radii): in a packet layer the attacker drawn into the target
//! (0x1F8270), 2 x 8 quads at the node textured from the screen under
//! them (0x100C60), grown 2 r age / 16 (1.5). In the spark window (0x1F ..
//! 0x5A / 0x2F) a spark record (32 x 0x120, 16 sprites drawn in to the
//! node). On update 0x1B (8) a burst at node 0xA's point: a far glow, a
//! 32 x 2 quad disc (5.5 / 8.8 / 9) far Z, 128 far sprites. The stage
//! light 0x80 - 96 (80) clamp(n) clamp(0x134 - n) / 1024. Returns 1 on
//! 0xC9 (0x78), 6 in the flash window, 2 on 0x104, 8 once the count
//! reaches 0x135.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

type C3 = [u8; 3];

/// When a head leaves: n == base + step x count (count < heads), or n
/// in a window and n % m == 0.
enum Launch {
    Count(i32, i32),
    Windows([(i32, i32); 2], i32),
}

/// What differs between the records.
struct V {
    bzero: u32,
    /// The frame, its scratch shift (sp + 0x3E0.. ; the slots below stay)
    /// and the header shift (D+0x20..) with the texture count.
    frame: u32,
    fs: u32,
    hs: u32,
    ntex: u32,
    launch_len: f32,
    /// The heads keep the node's x (b554 / b560).
    keep_x: bool,
    ring32_len: f32,
    /// The rings' life (32 / 16) and its shift.
    ring_life: i32,
    ring_sh: u32,
    glow_size: f32,
    ring1_size: f32,
    ring2_size: f32,
    flash_hi: i32,
    flash_ramp: i32,
    flash_glow_amp: f32,
    wave_lo: i32,
    wave_off0: f32,
    /// b006: the motes (16 x 0xA0, two passes) in place of the sparks.
    motes: bool,
    burst_size: f32,
    burst_len: f32,
    disc: [f32; 3],
    light_end: i32,
    end: i32,
    nodes: [i32; 2],
    heads: u32,
    /// Launch on n == base + step x count.
    launch: Launch,
    puff_n: u32,
    smoke_n: u32,
    /// The ring points' radii and the disc's first scale.
    radii: [f32; 2],
    head_size: f32,
    head_div: f32,
    head_life: i32,
    head_colour: C3,
    puff_len: f32,
    smoke_len: f32,
    puff_size: f32,
    puff_colour: C3,
    smoke_size: f32,
    smoke_colour: C3,
    glow_colour: C3,
    ring2_colour: C3,
    flash_lo: i32,
    flash_size: f32,
    flash_amp: f32,
    flash_c0: C3,
    flash_c1: C3,
    flash_glow: C3,
    wave_hi: i32,
    wave_len: f32,
    /// A wave radius: base + (1 - k x rand / 256).
    wave_k: f32,
    wave_base: f32,
    wave_win: i32,
    wave_amp: f32,
    wave_grow: f32,
    wave_off: f32,
    wave_c0: C3,
    wave_c1: C3,
    spark_hi: i32,
    spark_colour: C3,
    burst_at: i32,
    burst_glow: C3,
    burst_c0: C3,
    burst_c1: C3,
    burst_sprite: C3,
    light_k: i32,
    one: i32,
    /// Return addresses: rand (ring 16 / 32, puff, smoke, flash x3, wave,
    /// spark, burst), 0x1A2C70 (head, puff, smoke, glow, ring 16 / 32,
    /// flash quad / glow, wave, spark alpha / draw, burst glow), 0x100BA0
    /// (flash x2, wave x4, disc x2).
    r_rand: [u32; 10],
    r_fix: &'static [u32],
    r_pn: [u32; 8],
}

const B554: V = V {
    bzero: 0x7B90,
    frame: 0x660,
    fs: 0,
    hs: 0,
    ntex: 3,
    launch_len: 0.0,
    keep_x: true,
    ring32_len: 2.0,
    ring_life: 0x20,
    ring_sh: 5,
    glow_size: 10.0,
    ring1_size: 10.0,
    ring2_size: 4.0,
    flash_hi: 0x104,
    flash_ramp: 0x40,
    flash_glow_amp: 128.0,
    wave_lo: 0x3F,
    motes: false,
    burst_size: 8.0,
    burst_len: f(0x3F4C_CCCD),
    light_end: 0x134,
    end: 0x135,
    wave_off0: 15.0,
    disc: [5.5, f(0x410C_CCCD), 9.0],
    nodes: [0x22, 0xA],
    heads: 3,
    launch: Launch::Count(0x87, 4),
    puff_n: 4,
    smoke_n: 4,
    radii: [25.0, 70.0],
    head_size: 10.0,
    head_div: 76.0,
    head_life: 0x4D,
    head_colour: [0xC8, 0x50, 0x32],
    puff_len: 0.5,
    smoke_len: 1.0,
    puff_size: 10.5,
    puff_colour: [0xC8, 0x1E, 0x14],
    smoke_size: 6.5,
    smoke_colour: [0xC8, 0x1E, 0x18],
    glow_colour: [0xFF, 0x48, 0x36],
    ring2_colour: [0xFF, 0x50, 0x28],
    flash_lo: 0xD4,
    flash_size: 90.0,
    flash_amp: 200.0,
    flash_c0: [0xFF, 0, 0],
    flash_c1: [0xFF, 0x64, 0x3C],
    flash_glow: [0xFF, 0xA4, 0x6C],
    wave_hi: 0x101,
    wave_len: f(0x3ECC_CCCD),
    wave_k: 0.5,
    wave_base: 0.25,
    wave_win: 0x3C,
    wave_amp: 200.0,
    wave_grow: 2.0,
    wave_off: 15.0,
    wave_c0: [0xFF, 0x5A, 0x0F],
    wave_c1: [0x80, 0, 0],
    spark_hi: 0x5A,
    spark_colour: [0xFF, 0x38, 0x18],
    burst_at: 0x1B,
    burst_glow: [0xFF, 0xC4, 0xB4],
    burst_c0: [0xFF, 0xC4, 0xAA],
    burst_c1: [0xFF, 0x20, 0x17],
    burst_sprite: [0xFF, 0xC4, 0xAA],
    light_k: -96,
    one: 0xC9,
    r_rand: [0x01E0_0628, 0x01E0_0680, 0x01E0_0AC0, 0x01E0_0B50, 0x01E0_1CC4, 0x01E0_1CF8, 0x01E0_1D2C, 0x01E0_26B4, 0x01E0_2F10, 0x01E0_32F4],
    r_fix: &[
        0x01E0_0A60, 0x01E0_0F20, 0x01E0_12C8, 0x01E0_151C, 0x01E0_18B8, 0x01E0_1B80, 0x01E0_1FC8, 0x01E0_2480, 0x01E0_2A84, 0x01E0_30DC, 0x01E0_3248,
        0x01E0_347C,
    ],
    r_pn: [0x01E0_21E0, 0x01E0_2328, 0x01E0_2B34, 0x01E0_2BB0, 0x01E0_2CB8, 0x01E0_2D34, 0x01E0_372C, 0x01E0_3838],
};

/// b560 (card 32): one head, 16-sprite puffs / smokes, blue colours,
/// other nodes, windows and sizes [F].
const B560: V = V {
    bzero: 0x7710,
    frame: 0x660,
    fs: 0,
    hs: 0,
    ntex: 3,
    launch_len: 0.0,
    keep_x: true,
    ring32_len: 2.0,
    ring_life: 0x20,
    ring_sh: 5,
    glow_size: 10.0,
    ring1_size: 10.0,
    ring2_size: 4.0,
    flash_hi: 0x104,
    flash_ramp: 0x40,
    flash_glow_amp: 128.0,
    wave_lo: 0x3F,
    motes: false,
    burst_size: 8.0,
    burst_len: f(0x3F4C_CCCD),
    light_end: 0x134,
    end: 0x135,
    wave_off0: 5.0,
    disc: [8.5, f(0x410C_CCCD), 9.0],
    nodes: [0x26, 0x21],
    heads: 1,
    launch: Launch::Count(0x6E, 0),
    puff_n: 16,
    smoke_n: 16,
    radii: [15.0, 50.0],
    head_size: 8.0,
    head_div: 20.0,
    head_life: 0x15,
    head_colour: [0x5A, 0x80, 0xFF],
    puff_len: f(0x3E99_999A),
    smoke_len: f(0x3F4C_CCCD),
    puff_size: 8.5,
    puff_colour: [0x32, 0x3C, 0xFF],
    smoke_size: 4.5,
    smoke_colour: [0x40, 0x46, 0xFF],
    glow_colour: [0x36, 0x48, 0xFF],
    ring2_colour: [0x40, 0x46, 0xFF],
    flash_lo: 0x83,
    flash_size: 40.0,
    flash_amp: 100.0,
    flash_c0: [0, 0, 0xFF],
    flash_c1: [0x3C, 0x64, 0xFF],
    flash_glow: [0x6C, 0xA4, 0xFF],
    wave_hi: 0xAA,
    wave_len: f(0x3E4C_CCCD),
    wave_k: f(0x3E99_999A),
    wave_base: f(0x3E19_999A),
    wave_win: 0x10,
    wave_amp: 164.0,
    wave_grow: 1.5,
    wave_off: 5.0,
    wave_c0: [0x50, 0x60, 0xFF],
    wave_c1: [0, 0, 0x80],
    spark_hi: 0x2F,
    spark_colour: [0x18, 0x38, 0xFF],
    burst_at: 8,
    burst_glow: [0xB4, 0xC4, 0xFF],
    burst_c0: [0xE6, 0x60, 0xFF],
    burst_c1: [0x7B, 0x20, 0xFF],
    burst_sprite: [0x80, 0x20, 0xFF],
    light_k: -80,
    one: 0x78,
    r_rand: [0x01E0_0620, 0x01E0_0678, 0x01E0_0AC8, 0x01E0_0B58, 0x01E0_1CC0, 0x01E0_1CF4, 0x01E0_1D28, 0x01E0_26B4, 0x01E0_2F18, 0x01E0_32FC],
    r_fix: &[
        0x01E0_0A68, 0x01E0_0F20, 0x01E0_12C0, 0x01E0_1520, 0x01E0_18B8, 0x01E0_1B80, 0x01E0_1FC8, 0x01E0_2480, 0x01E0_2A8C, 0x01E0_30E4, 0x01E0_3250,
        0x01E0_3484,
    ],
    r_pn: [0x01E0_21E0, 0x01E0_2328, 0x01E0_2B3C, 0x01E0_2BB8, 0x01E0_2CC0, 0x01E0_2D3C, 0x01E0_372C, 0x01E0_3838],
};

/// b006 (card 22): eight heads launched every fifth update in two
/// windows, a fourth texture (the header 0x10 higher), the frame 0x10
/// larger, red / orange colours, 16-update rings, a two-pass mote pool
/// (16 x 0xA0, 8 sprites, textures 0 / 3) in place of the sparks [F].
const B006: V = V {
    bzero: 0xBE20,
    frame: 0x670,
    fs: 0x10,
    hs: 0x10,
    ntex: 4,
    launch_len: 1.5,
    keep_x: false,
    ring32_len: 1.0,
    ring_life: 0x10,
    ring_sh: 4,
    glow_size: 25.0,
    ring1_size: 5.0,
    ring2_size: 1.0,
    flash_hi: 0x108,
    flash_ramp: 0x20,
    flash_glow_amp: 64.0,
    wave_lo: 0x21,
    wave_off0: 35.0,
    motes: true,
    burst_size: 24.0,
    burst_len: 1.0,
    disc: [16.5, f(0x41D3_3333), 27.0],
    light_end: 0x138,
    end: 0x139,
    nodes: [0x21, 0x20],
    heads: 8,
    launch: Launch::Windows([(0x5A, 0x65), (0x83, 0xC8)], 5),
    puff_n: 4,
    smoke_n: 4,
    radii: [35.0, 100.0],
    head_size: 5.0,
    head_div: 32.0,
    head_life: 0x21,
    head_colour: [0xC8, 0x50, 0x32],
    puff_len: 0.25,
    smoke_len: 0.25,
    puff_size: 5.25,
    puff_colour: [0xFF, 0x5A, 0x2D],
    smoke_size: 2.0,
    smoke_colour: [0xFF, 0x5A, 0x2D],
    glow_colour: [0xFF, 0x5A, 0x2D],
    ring2_colour: [0xFF, 0x5A, 0x2D],
    flash_lo: 0xA3,
    flash_size: 90.0,
    flash_amp: 100.0,
    flash_c0: [0xFF, 0x20, 0],
    flash_c1: [0xFF, 0x5A, 0x2D],
    flash_glow: [0xFF, 0x5A, 0x2D],
    wave_hi: 0xE0,
    wave_len: f(0x3E99_999A),
    wave_k: f(0x3E99_999A),
    wave_base: f(0x3E19_999A),
    wave_win: 0x3C,
    wave_amp: 100.0,
    wave_grow: 2.0,
    wave_off: 15.0,
    wave_c0: [0xFF, 0x5A, 0x2D],
    wave_c1: [0, 0, 0x20],
    spark_hi: 0xBE,
    spark_colour: [0xFF, 0x20, 0x08],
    burst_at: 0x46,
    burst_glow: [0xFF, 0xC4, 0x84],
    burst_c0: [0xFF, 0xC4, 0x84],
    burst_c1: [0xFF, 0x20, 0x17],
    burst_sprite: [0xFF, 0xE8, 0x9B],
    light_k: -96,
    one: 0x98,
    r_rand: [0x01E0_06F0, 0x01E0_0748, 0x01E0_0B88, 0x01E0_0C18, 0x01E0_1DA8, 0x01E0_1DDC, 0x01E0_1E10, 0x01E0_2838, 0x01E0_3274, 0x01E0_39A0],
    r_fix: &[
        0x01E0_0B28, 0x01E0_0FE8, 0x01E0_1390, 0x01E0_15F4, 0x01E0_1990, 0x01E0_1C48, 0x01E0_20C4, 0x01E0_25B8, 0x01E0_2D5C, 0x01E0_3458, 0x01E0_35F4,
        0x01E0_36FC, 0x01E0_38B0, 0x01E0_3B40,
    ],
    r_pn: [0x01E0_2300, 0x01E0_2454, 0x01E0_2E1C, 0x01E0_2EA8, 0x01E0_2FBC, 0x01E0_3048, 0x01E0_3E14, 0x01E0_3F2C],
};

/// A pool record's layout: `n` cell bytes at +8, the point on the next
/// 16-byte boundary, `n` directions after it.
const fn cells_end(n: u32) -> u32 {
    (8 + n + 15) & !15
}
const fn rec(n: u32) -> u32 {
    cells_end(n) + 0x10 + 0x10 * n
}

/// The offsets that follow from the head count and the pool sizes.
struct L {
    puffs: u32,
    smokes: u32,
    ring_i: u32,
    rings: u32,
    /// The ring points; everything after sits at a fixed offset from it
    /// up to the sparks / motes, the burst after them.
    x: u32,
    burst: u32,
}

fn lay(v: &V) -> L {
    let puffs = 0x80 + v.hs + 0x30 * v.heads;
    let smokes = puffs + 8 * rec(v.puff_n) * v.heads;
    let ring_i = smokes + 0x20 * rec(v.smoke_n) * v.heads;
    let rings = ring_i + 0x10;
    let x = rings + 0x350 * v.heads;
    let burst = x + SPARKS + if v.motes { 0x10 * 0xA0 } else { 0x20 * 0x120 };
    L { puffs, smokes, ring_i, rings, x, burst }
}

// Offsets from the ring points (b554: D+0x3810).
const FLASH_I: u32 = 0x330;
const FLASHES: u32 = 0x340;
const WAVE_I: u32 = 0x940;
const WAVE_V0: u32 = 0x950;
const WAVE_V1: u32 = 0xB00;
const WAVES: u32 = 0xCB0;
const SPARKS: u32 = 0x10B0;
// Offsets from the burst (b554: D+0x6CC0).
const DISC: u32 = 0x10;
const B_CELLS: u32 = 0x640;
const B_POS: u32 = 0x6C0;
const B_DIRS: u32 = 0x6D0;

/// The header D + `hs` + `o` (from D+0x20 up).
fn hdr(v: &V, o: u32) -> u32 {
    D + v.hs + o
}

pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B554)
}

pub fn program_b560(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B560)
}

pub fn program_b006(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B006)
}

fn run(fx: &mut Fx, param: i32, v: &V) -> i32 {
    let sp = fx.enter(v.frame);
    fx.mem.write_u32(sp + 0xF0, D);
    fx.mem.write_u32(sp + 0xC0, 0);
    // `sp` below is the scratch base (the frame + fs); the slots at
    // 0xC0 / 0xD0 / 0xEC / 0xF0 stay at the frame.
    let s = sp + v.fs;
    let ret = if param >= 0 { init(fx, s, v) } else { step(fx, s, v) };
    fx.leave(v.frame);
    ret
}

fn init(fx: &mut Fx, sp: u32, v: &V) -> i32 {
    let l = lay(v);
    fx.bzero(D, v.bzero);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    for k in 0..v.ntex {
        let r = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x10 + 4 * k, r);
    }
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, v.nodes[0], 1);
    fx.mem.set32(D + 0x10 + 4 * v.ntex, r);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, v.nodes[1], 0);
    fx.mem.set32(hdr(v, 0x30), r);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, hdr(v, 0x60));
    let h = fx.mem.i32(D + 4);
    let r = crate::blib::afterimage::unit_radius(fx, h);
    fx.mem.setf(hdr(v, 0x70), r);
    fx.mem.write_u32(hdr(v, 0x74), 0);
    fx.mem.write_u32(hdr(v, 0x78), 0);
    for k in 0..v.heads {
        let hd = hdr(v, 0x30 * k);
        fx.mem.write_u8(hd + 0x80, 0);
        fx.mem.write_u32(hd + 0x88, 0);
        fx.mem.write_u32(hd + 0x8C, 0);
    }
    for h in 0..v.heads {
        for j in 0..8 {
            fx.mem.write_u8(puff(&l, v, h, j), 0);
        }
    }
    for h in 0..v.heads {
        for j in 0..0x20 {
            fx.mem.write_u8(smoke(&l, v, h, j), 0);
        }
    }
    fx.mem.write_u32(D + l.ring_i, 0);
    for k in 0..v.heads {
        fx.mem.write_u8(D + l.rings + 0x350 * k, 0);
    }
    let x = D + l.x;
    for k in 0..0x11u32 {
        let p = x + 0x30 * k;
        let a = (k << 12) as i32;
        for (o, rr) in [(0u32, v.radii[0]), (0x10, v.radii[1])] {
            let s = fx.tsin(a);
            fx.mem.setf(p + o, ee::mul(rr, s));
            let c = fx.tcos(a);
            fx.mem.setf(p + o + 4, ee::mul(rr, c));
            fx.mem.write_u32(p + o + 8, 0);
            fx.mem.write_u32(p + o + 0xC, 0x3F80_0000);
        }
        fx.vu_add4(p + 0x20, p, p + 0x10);
        fx.vu_scale4(p + 0x20, p + 0x20, 0.5);
    }
    fx.mem.write_u32(x + FLASH_I, 0);
    for k in 0..0x10u32 {
        fx.mem.write_u8(x + FLASHES + 0x60 * k, 0);
    }
    fx.mem.write_u32(x + WAVE_I, 0);
    for k in 0..0x10u32 {
        fx.mem.write_u8(x + WAVES + 0x40 * k, 0);
    }
    if v.motes {
        for k in 0..0x10u32 {
            fx.mem.write_u8(x + SPARKS + 0xA0 * k, 0);
        }
    } else {
        for k in 0..0x20u32 {
            fx.mem.write_u8(x + SPARKS + 0x120 * k, 0);
        }
    }
    let bu = D + l.burst;
    fx.mem.write_u8(bu, 0);
    for k in 0..0x21u32 {
        let a = (k << 11) as i32;
        let s = fx.tsin(a);
        fx.mem.setf(sp + 0x500, s);
        let c = fx.tcos(a);
        fx.mem.setf(sp + 0x504, c);
        fx.mem.write_u32(sp + 0x508, 0);
        fx.mem.write_u32(sp + 0x50C, 0x3F80_0000);
        let p = bu + DISC + 0x30 * k;
        fx.vu_scale4(p, sp + 0x500, v.disc[0]);
        fx.vu_scale4(p + 0x10, sp + 0x500, v.disc[1]);
        fx.vu_scale4(p + 0x20, sp + 0x500, v.disc[2]);
    }
    // The disc loop steps the stack copy of a0.
    fx.mem.write_u32(sp - v.fs + 0xF0, D + 0x21 * 0x30);
    0
}

fn puff(l: &L, v: &V, h: u32, j: u32) -> u32 {
    D + l.puffs + 8 * rec(v.puff_n) * h + rec(v.puff_n) * j
}

fn smoke(l: &L, v: &V, h: u32, j: u32) -> u32 {
    D + l.smokes + 0x20 * rec(v.smoke_n) * h + rec(v.smoke_n) * j
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

fn fixu(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, sp + 0x618, u0);
    uv(fx, sp + 0x61A, v0);
    uv(fx, sp + 0x628, u1);
    uv(fx, sp + 0x62A, v1);
}

/// The sprite cell `b` (4 x n cells of 32).
fn cell_uvs(fx: &mut Fx, sp: u32, b: i32) {
    let (c, r) = (rem2(b, 4), b >> 2);
    let m = |x: i32| (x & 0x3FFF) as u32;
    uvs(fx, sp, m(c << 9), m(r << 9), m((32 * c + 32) << 4), m((32 * r + 32) << 4));
}

fn rgba(fx: &mut Fx, at: u32, c: C3, a: u8) {
    fx.mem.write_u8(at, c[0]);
    fx.mem.write_u8(at + 1, c[1]);
    fx.mem.write_u8(at + 2, c[2]);
    fx.mem.write_u8(at + 3, a);
    fx.mem.write_u32(at + 4, 0x3F80_0000);
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for c in 0..4u32 {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

fn copy8(fx: &mut Fx, to: u32, from: u32) {
    for c in 0..2u32 {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

fn vec4(fx: &mut Fx, at: u32, x: f32, y: f32, z: f32) {
    fx.mem.setf(at, x);
    fx.mem.setf(at + 4, y);
    fx.mem.setf(at + 8, z);
    fx.mem.write_u32(at + 0xC, 0x3F80_0000);
}

/// The sprite descriptor at sp + 0x600 (5 registers 0x42421, prim 0x26 |
/// 0x800).
fn head(fx: &mut Fx, sp: u32) {
    let d = sp + 0x600;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let fl = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, fl | 0x800);
}

/// Alpha preset `a`, the texture at D + `tex` (filter 1), CLAMP 5.
fn textured(fx: &mut Fx, sp: u32, a: i32, tex: u32) {
    let d = sp + 0x600;
    let p = fx.alpha_preset(a);
    fx.desc_alpha(d, p);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
}

/// The view-projection (x identity) at sp + 0x3E0.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x3E0);
    fx.identity(sp + 0x420);
    let id = fx.read_m4(sp + 0x420);
    fx.mul_into(Some(sp + 0x3E0), &id);
}

/// The view-projection x (the camera turn moved to `p`) at sp + 0x3E0,
/// scaled by `s` first when given.
fn camera_at(fx: &mut Fx, sp: u32, p: u32, s: Option<(f32, f32)>) {
    fx.camera_vp_to(sp + 0x3E0);
    fx.translate_rows(sp + 0x420, sp + 0x460, p);
    if let Some((s, z)) = s {
        vec4(fx, sp + 0x500, s, s, z);
        fx.scale_rows(sp + 0x420, sp + 0x500);
    }
    let m = fx.read_m4(sp + 0x420);
    fx.mul_into(Some(sp + 0x3E0), &m);
}

/// The corners (-s, -s, 0, 1) / (s, s, 0, 1) at sp + 0x4A0 / 0x4B0,
/// turned by the camera (sp + 0x460) when `turn`.
fn corners(fx: &mut Fx, sp: u32, s: f32, turn: bool) {
    let n = ee::neg(s);
    vec4(fx, sp + 0x4A0, n, n, 0.0);
    vec4(fx, sp + 0x4B0, s, s, 0.0);
    if turn {
        fx.vu_apply_to(sp + 0x4A0, sp + 0x460, sp + 0x4A0);
        fx.vu_apply_to(sp + 0x4B0, sp + 0x460, sp + 0x4B0);
    }
}

/// k (0.5 + 0.5 x / d).
fn half(k: f32, x: i32, d: f32) -> f32 {
    ee::mul(k, ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(x)), d)))
}

/// base + slope x the tan falloff.
fn spread(fx: &mut Fx, age: i32, n: i32, p: i32, base: u32, slope: u32) -> f32 {
    let t = fx.tan_falloff_p(age, n, p);
    ee::add(f(base), ee::mul(f(slope), t))
}

/// The corners about sp + 0x500 (sp + 0x4E0 / 0x4F0) projected (0x204F70)
/// through sp + 0x3E0; submitted at the mean w (`ra`) or far at key 0.
fn draw(fx: &mut Fx, sp: u32, ra: Option<u32>) {
    fx.vu_add4(sp + 0x4E0, sp + 0x4A0, sp + 0x500);
    fx.vu_add4(sp + 0x4F0, sp + 0x4B0, sp + 0x500);
    project_sprite(fx, sp, sp + 0x4E0, ra);
}

fn project_sprite(fx: &mut Fx, sp: u32, input: u32, ra: Option<u32>) {
    let d = sp + 0x600;
    if !crate::fxlib2::project(fx, input, PROJ_OUT, sp + 0x3E0, d, sp + 0x65C) {
        return;
    }
    match ra {
        Some(ra) => {
            let w = fx.mem.f(sp + 0x65C);
            let key = fixu(fx, w, ra) as i32;
            fx.submit(d, OT, key);
        }
        None => {
            crate::lens::far_z(fx, sp + 0x620, &[sp + 0x630]);
            fx.submit(d, OT, 0);
        }
    }
}

/// A sprite of the direction `dir` x `k` about `pos`, drawn.
fn sprite(fx: &mut Fx, sp: u32, dir: u32, k: f32, pos: u32, ra: Option<u32>) {
    fx.vu_scale4(sp + 0x500, dir, k);
    fx.vu_add4(sp + 0x500, sp + 0x500, pos);
    draw(fx, sp, ra);
}

/// A quad (descriptor `d`, four vertices) through 0x100BA0, far Z at
/// key 0 into `ot` when drawn.
fn quad_far(fx: &mut Fx, sp: u32, vs: [u32; 4], d: u32, xyz: [u32; 4], ot: u32, ra: u32) -> bool {
    if fx.project_n(&vs, sp + 0x3E0, d, sp + 0x65C, ra) != 1 {
        return false;
    }
    crate::lens::far_z(fx, xyz[0], &xyz[1..]);
    fx.submit(d, ot, 0);
    true
}

fn step(fx: &mut Fx, sp: u32, v: &V) -> i32 {
    let l = lay(v);
    let x = D + l.x;
    fx.camera_turn_roll(sp + 0x460);
    let (hh, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x10 + 4 * v.ntex));
    fx.node_position(hh, slot, hdr(v, 0x20));
    let (hh, slot) = (fx.mem.i32(D + 4), fx.mem.i32(hdr(v, 0x30)));
    fx.node_position(hh, slot, hdr(v, 0x40));
    let hh = fx.mem.i32(D + 4);
    fx.unit_point(1, hh, hdr(v, 0x50));
    let go = match v.launch {
        Launch::Count(base, k) => {
            let c = fx.mem.i32(hdr(v, 0x74));
            c < v.heads as i32 && fx.mem.i32(D) == base + k * c
        }
        Launch::Windows(ws, m) => {
            let n = fx.mem.i32(D);
            ws.iter().any(|&(lo, hi)| n >= lo && n < hi) && n % m == 0
        }
    };
    if go {
        launch(fx, sp, v, &l);
    }
    heads(fx, sp, v, &l);
    puffs(fx, sp, v, &l);
    smokes(fx, sp, v, &l);
    rings(fx, sp, v, &l);

    let n = fx.mem.i32(D);
    if n >= v.flash_lo && n < v.flash_hi {
        let r = x + FLASHES + 0x60 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        copy4(fx, r + 0x10, hdr(v, 0x60));
        fx.identity(r + 0x20);
        for (axis, ra, sh) in [(0, v.r_rand[4], 15), (1, v.r_rand[5], 15), (2, v.r_rand[6], 16)] {
            let b = fx.rand_from(OT, ra) >> 23;
            fx.rotate_axis(r + 0x20, (rem2(b, 0x100) << sh) >> 8, axis);
        }
    }
    flashes(fx, sp, v, x);

    let n = fx.mem.i32(D);
    if n >= v.wave_lo && n < v.wave_hi {
        let i = fx.mem.read_u32(x + WAVE_I);
        let r = x + WAVES + 0x40 * i;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        fx.random_table_vec(r + 0x30, v.wave_len);
        for j in 0..8u32 {
            let b = fx.rand_from(OT, v.r_rand[7]) >> 23;
            let t = ee::div(ee::mul(v.wave_k, ee::from_i32(rem2(b, 0x100))), 256.0);
            fx.mem.setf(r + 8 + 4 * j, ee::add(v.wave_base, ee::sub(1.0, t)));
        }
        let w = fx.mem.read_u32(r + 8);
        fx.mem.write_u32(r + 0x28, w);
        let i = fx.mem.i32(x + WAVE_I) + 1;
        fx.mem.set32(x + WAVE_I, if i >= 0x10 { 0 } else { i });
    }
    waves(fx, sp, v, x);

    let n = fx.mem.i32(D);
    if v.motes {
        if n > 0 && n < v.spark_hi {
            let r = x + SPARKS + 0xA0 * rem2(n, 0x10) as u32;
            fx.mem.write_u8(r, 1);
            fx.mem.write_u32(r + 4, 0);
            copy4(fx, r + 0x10, hdr(v, 0x20));
            for j in 0..8u32 {
                let b = fx.rand_from(OT, v.r_rand[8]) >> 23;
                fx.mem.set8(r + 8 + j, rem2(b, 4));
                fx.random_table_vec(r + 0x20 + 0x10 * j, 1.0);
            }
        }
        motes(fx, sp, v, x);
    } else if n >= 0x1F && n < v.spark_hi {
        let r = x + SPARKS + 0x120 * rem2(n, 0x20) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for j in 0..0x10u32 {
            let b = fx.rand_from(OT, v.r_rand[8]) >> 23;
            fx.mem.set8(r + 8 + j, rem2(b, 4));
            let dir = r + 0x20 + 0x10 * j;
            fx.random_table_vec(dir, 1.0);
            fx.vu_add4(dir, dir, hdr(v, 0x20));
        }
    }
    if !v.motes {
        sparks(fx, sp, v, x);
    }

    let bu = D + l.burst;
    if fx.mem.i32(D) == v.burst_at {
        fx.mem.write_u8(bu, 1);
        fx.mem.write_u32(bu + 4, 0);
        copy4(fx, bu + B_POS, hdr(v, 0x40));
        for j in 0..0x80u32 {
            let b = fx.rand_from(OT, v.r_rand[9]) >> 23;
            fx.mem.set8(bu + B_CELLS + j, rem2(b, 4));
            fx.random_table_vec(bu + B_DIRS + 0x10 * j, v.burst_len);
        }
    }
    if fx.mem.read_u8(bu) == 1 {
        burst(fx, sp, v, bu);
    }

    let n = fx.mem.i32(D);
    let s0 = Fx::clamp_i(v.light_end - n, 0, 0x20);
    let k = Fx::clamp_i(n, 0, 0x20);
    let lt = (k.wrapping_mul(v.light_k).wrapping_mul(s0) >> 10) + 0x80;
    fx.set_stage_light(lt & 0xFF);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == v.one {
        ret = 1;
    } else {
        if n >= v.flash_lo && n < v.flash_hi {
            ret = 6;
        }
        if n == v.flash_hi {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= v.end {
        ret = 8;
    }
    fx.mem.write_u32(sp - v.fs + 0xC0, ret as u32);
    ret
}

/// A head and its ring [F 0x1E00540..0x1E00758].
fn launch(fx: &mut Fx, sp: u32, v: &V, l: &L) {
    let hd = hdr(v, 0x30 * fx.mem.read_u32(hdr(v, 0x78)));
    fx.mem.write_u8(hd + 0x80, 1);
    fx.mem.write_u32(hd + 0x84, 0);
    let rg = D + l.rings + 0x350 * fx.mem.read_u32(D + l.ring_i);
    fx.mem.write_u8(rg, 1);
    fx.mem.write_u32(rg + 4, 0);
    copy4(fx, hd + 0x90, hdr(v, 0x20));
    copy4(fx, rg + 0x10, hdr(v, 0x20));
    fx.random_table_vec(sp + 0x500, v.launch_len);
    fx.vu_add4(sp + 0x500, sp + 0x500, hdr(v, 0x60));
    if v.keep_x {
        let px = fx.mem.f(hd + 0x90);
        fx.mem.setf(sp + 0x500, px);
    }
    fx.vu_sub4(hd + 0xA0, sp + 0x500, hd + 0x90);
    for j in 0..0x10u32 {
        let b = fx.rand_from(OT, v.r_rand[0]) >> 23;
        fx.mem.set8(rg + 0x20 + j, rem2(b, 8));
        fx.random_table_vec(rg + 0x30 + 0x10 * j, 1.0);
    }
    for j in 0..0x20u32 {
        let b = fx.rand_from(OT, v.r_rand[1]) >> 23;
        fx.mem.set8(rg + 0x130 + j, rem2(b, 4));
        fx.random_table_vec(rg + 0x150 + 0x10 * j, v.ring32_len);
    }
    let c = fx.mem.i32(hdr(v, 0x74)) + 1;
    fx.mem.set32(hdr(v, 0x74), c);
    let i = fx.mem.i32(hdr(v, 0x78)) + 1;
    fx.mem.set32(hdr(v, 0x78), if i >= v.heads as i32 { 0 } else { i });
    let i = fx.mem.i32(D + l.ring_i) + 1;
    fx.mem.set32(D + l.ring_i, if i >= v.heads as i32 { 0 } else { i });
}

/// The heads (sprite, along their direction by age / d) and what they
/// leave [F 0x1E00758..0x1E00C38].
fn heads(fx: &mut Fx, sp: u32, v: &V, l: &L) {
    head(fx, sp);
    textured(fx, sp, 1, 0x10);
    camera(fx, sp);
    uvs(fx, sp, 0x400, 0, 0x600, 0x200);
    corners(fx, sp, v.head_size, true);
    let (pc, sc) = (cells_end(v.puff_n), cells_end(v.smoke_n));
    for k in 0..v.heads {
        let hd = hdr(v, 0x30 * k);
        if fx.mem.read_u8(hd + 0x80) == 0 {
            continue;
        }
        rgba(fx, sp + 0x610, v.head_colour, 0xFF);
        let f21 = ee::div(ee::from_i32(fx.mem.i32(hd + 0x84)), v.head_div);
        let a = fx.mem.i32(hd + 0x88);
        fx.mem.set32(hd + 0x88, if a + 1 >= 8 { 0 } else { a + 1 });
        let b = fx.mem.i32(hd + 0x8C);
        fx.mem.set32(hd + 0x8C, if b + 1 >= 0x20 { 0 } else { b + 1 });
        let pf = puff(l, v, k, a as u32);
        let sm = smoke(l, v, k, b as u32);
        fx.vu_scale4(sp + 0x500, hd + 0xA0, f21);
        fx.vu_add4(sp + 0x500, sp + 0x500, hd + 0x90);
        draw(fx, sp, Some(v.r_fix[0]));
        fx.mem.write_u8(pf, 1);
        fx.mem.write_u32(pf + 4, 0);
        copy4(fx, pf + pc, sp + 0x500);
        for j in 0..v.puff_n {
            let b = fx.rand_from(OT, v.r_rand[2]) >> 23;
            fx.mem.set8(pf + 8 + j, rem2(b, 8));
            fx.random_table_vec(pf + pc + 0x10 + 0x10 * j, v.puff_len);
        }
        fx.mem.write_u8(sm, 1);
        fx.mem.write_u32(sm + 4, 0);
        copy4(fx, sm + sc, sp + 0x500);
        for j in 0..v.smoke_n {
            let b = fx.rand_from(OT, v.r_rand[3]) >> 23;
            fx.mem.set8(sm + 8 + j, b % 3);
            fx.random_table_vec(sm + sc + 0x10 + 0x10 * j, v.smoke_len);
        }
        let age = fx.mem.i32(hd + 0x84) + 1;
        fx.mem.set32(hd + 0x84, age);
        if age >= v.head_life {
            fx.mem.write_u8(hd + 0x80, 0);
        }
    }
}

/// The puffs (8 updates) [F 0x1E00C38..0x1E00FD0].
fn puffs(fx: &mut Fx, sp: u32, v: &V, l: &L) {
    head(fx, sp);
    textured(fx, sp, 1, 0x14);
    camera(fx, sp);
    let pc = cells_end(v.puff_n);
    for h in 0..v.heads {
        for j in 0..8u32 {
            let r = puff(l, v, h, j);
            if fx.mem.read_u8(r) == 0 {
                continue;
            }
            let age = fx.mem.i32(r + 4);
            corners(fx, sp, half(v.puff_size, 8 - age, 8.0), true);
            let age = fx.mem.i32(r + 4);
            rgba(fx, sp + 0x610, v.puff_colour, (((8 - age) << 7) >> 3) as u8);
            let age = fx.mem.i32(r + 4);
            let f21 = spread(fx, age, 8, 1, 0x3ECC_CCCD, 0x3F19_999A);
            for i in 0..v.puff_n {
                let b = i32::from(fx.mem.read_u8(r + 8 + i));
                cell_uvs(fx, sp, b);
                sprite(fx, sp, r + pc + 0x10 + 0x10 * i, f21, r + pc, Some(v.r_fix[1]));
            }
            let age = fx.mem.i32(r + 4) + 1;
            fx.mem.set32(r + 4, age);
            if age >= 9 {
                fx.mem.write_u8(r, 0);
            }
        }
    }
}

/// The smokes (32 updates) [F 0x1E00FD0..0x1E01378].
fn smokes(fx: &mut Fx, sp: u32, v: &V, l: &L) {
    head(fx, sp);
    textured(fx, sp, 1, 0x10);
    camera(fx, sp);
    let sc = cells_end(v.smoke_n);
    for h in 0..v.heads {
        for j in 0..0x20u32 {
            let r = smoke(l, v, h, j);
            if fx.mem.read_u8(r) == 0 {
                continue;
            }
            let age = fx.mem.i32(r + 4);
            corners(fx, sp, half(v.smoke_size, 0x20 - age, 32.0), true);
            let age = fx.mem.i32(r + 4);
            rgba(fx, sp + 0x610, v.smoke_colour, ((0x20 - age).wrapping_mul(200) >> 5) as u8);
            let age = fx.mem.i32(r + 4);
            let f21 = spread(fx, age, 0x20, 1, 0x3E4C_CCCD, 0x3F4C_CCCD);
            for i in 0..v.smoke_n {
                let b = i32::from(fx.mem.read_u8(r + 8 + i));
                cell_uvs(fx, sp, b);
                sprite(fx, sp, r + sc + 0x10 + 0x10 * i, f21, r + sc, Some(v.r_fix[2]));
            }
            let age = fx.mem.i32(r + 4) + 1;
            fx.mem.set32(r + 4, age);
            if age >= 0x21 {
                fx.mem.write_u8(r, 0);
            }
        }
    }
}

/// The rings (32 updates): a far glow, 16 + 32 sprites [F
/// 0x1E01378..0x1E01C28].
fn rings(fx: &mut Fx, sp: u32, v: &V, l: &L) {
    head(fx, sp);
    camera(fx, sp);
    for k in 0..v.heads {
        let r = D + l.rings + 0x350 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        textured(fx, sp, 1, 0x10);
        uvs(fx, sp, 0x600, 0, 0x800, 0x200);
        corners(fx, sp, v.glow_size, true);
        let age = fx.mem.i32(r + 4);
        let t = fx.tan_falloff_p(age, v.ring_life, 1);
        let al = fixu(fx, ee::mul(255.0, ee::sub(1.0, t)), v.r_fix[3]) as u8;
        rgba(fx, sp + 0x610, v.glow_colour, al);
        copy4(fx, sp + 0x500, r + 0x10);
        draw(fx, sp, None);

        textured(fx, sp, 0, 0x14);
        let age = fx.mem.i32(r + 4);
        let (life, sh) = (v.ring_life, v.ring_sh);
        corners(fx, sp, half(v.ring1_size, age, ee::from_i32(life)), true);
        let age = fx.mem.i32(r + 4);
        rgba(fx, sp + 0x610, [0x80; 3], (((life - age) << 7) >> sh) as u8);
        let age = fx.mem.i32(r + 4);
        let f21 = fx.tan_falloff_p(age, life, 1);
        for i in 0..0x10u32 {
            let b = i32::from(fx.mem.read_u8(r + 0x20 + i));
            cell_uvs(fx, sp, b);
            sprite(fx, sp, r + 0x30 + 0x10 * i, f21, r + 0x10, Some(v.r_fix[4]));
        }

        textured(fx, sp, 1, 0x10);
        let age = fx.mem.i32(r + 4);
        corners(fx, sp, half(v.ring2_size, life - age, ee::from_i32(life)), true);
        let age = fx.mem.i32(r + 4);
        rgba(fx, sp + 0x610, v.ring2_colour, ((life - age).wrapping_mul(255) >> sh) as u8);
        let age = fx.mem.i32(r + 4);
        let f21 = fx.tan_falloff_p(age, life, 1);
        for i in 0..0x20u32 {
            let b = i32::from(fx.mem.read_u8(r + 0x130 + i));
            cell_uvs(fx, sp, b);
            sprite(fx, sp, r + 0x150 + 0x10 * i, f21, r + 0x10, Some(v.r_fix[5]));
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= life + 1 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The quad descriptor at sp + 0x570 (12 registers, prim 0x34).
fn quad_head(fx: &mut Fx, sp: u32, flag: bool) {
    let d = sp + 0x570;
    fx.mem.write_u16(d + 2, 0x34);
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    if flag {
        let fl = fx.mem.read_u16(d + 2);
        fx.mem.write_u16(d + 2, fl | 0x800);
    }
}

const QXYZ: [u32; 4] = [0x590, 0x5A8, 0x5C0, 0x5D8];

fn qxyz(sp: u32) -> [u32; 4] {
    QXYZ.map(|o| sp + o)
}

/// The flashes (16 updates) [F 0x1E01D68..0x1E02654].
fn flashes(fx: &mut Fx, sp: u32, v: &V, x: u32) {
    quad_head(fx, sp, false);
    let d = sp + 0x570;
    let p = fx.alpha_preset(1);
    fx.desc_alpha(d, p);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 0);
    head(fx, sp);
    textured(fx, sp, 1, 0x10);
    uvs(fx, sp, 0x400, 0, 0x600, 0x200);
    corners(fx, sp, v.flash_size, false);
    let n = fx.mem.i32(D);
    let f21 = ee::div(ee::from_i32(Fx::clamp_i(v.flash_hi - n, 0, v.flash_ramp)), ee::from_i32(v.flash_ramp));
    for k in 0..0x10u32 {
        let r = x + FLASHES + 0x60 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let f0 = Fx::ramp_above(age, 16.0, 8.0);
        rgba(fx, sp + 0x580, v.flash_c0, 0);
        copy8(fx, sp + 0x598, sp + 0x580);
        let al = fixu(fx, ee::mul(ee::mul(v.flash_amp, f0), f21), v.r_fix[6]) as u8;
        rgba(fx, sp + 0x5B0, v.flash_c1, al);
        copy8(fx, sp + 0x5C8, sp + 0x5B0);
        fx.camera_vp_to(sp + 0x3E0);
        let t = [fx.mem.f(r + 0x10), fx.mem.f(r + 0x14), fx.mem.f(r + 0x18)];
        let m = fx.trs(None, None, Some(t));
        fx.write_m4(sp + 0x420, &m);
        let rm = fx.read_m4(r + 0x20);
        fx.mul_into(Some(sp + 0x420), &rm);
        fx.rotate_axis(sp + 0x420, 0, 0);
        let age = fx.mem.i32(r + 4);
        let s = spread(fx, age, 0x10, 1, 0x3DCC_CCCD, 0x3F66_6666);
        vec4(fx, sp + 0x500, s, s, 0.0);
        fx.scale_rows(sp + 0x420, sp + 0x500);
        let m = fx.read_m4(sp + 0x420);
        fx.mul_into(Some(sp + 0x3E0), &m);
        let age = fx.mem.i32(r + 4);
        let f20 = ee::div(ee::mul(-1.0, ee::from_i32(rem2(age, 0x20))), 32.0);
        let f22 = ee::add(0.5, f20);
        let f23 = ee::add(1.0, f20);
        for s6 in 0..0x10u32 {
            let (s1, s4) = (x + 0x30 * s6, x + 0x30 * (s6 + 1));
            let f25 = ee::div(ee::mul(6.0, ee::from_i32(s6 as i32)), 16.0);
            let f24 = ee::div(ee::mul(6.0, ee::from_i32(s6 as i32 + 1)), 16.0);
            for (o, y) in [(0x588u32, f25), (0x58C, f20), (0x5A0, f24), (0x5A4, f20), (0x5B8, f25), (0x5BC, f22), (0x5D0, f24), (0x5D4, f22)] {
                fx.mem.setf(sp + o, y);
            }
            quad_far(fx, sp, [s1, s4, s1 + 0x20, s4 + 0x20], d, qxyz(sp), OT, v.r_pn[0]);
            for (o, y) in [(0x588u32, f25), (0x58C, f23), (0x5A0, f24), (0x5A4, f23), (0x5B8, f25), (0x5BC, f22), (0x5D0, f24), (0x5D4, f22)] {
                fx.mem.setf(sp + o, y);
            }
            quad_far(fx, sp, [s1 + 0x10, s4 + 0x10, s1 + 0x20, s4 + 0x20], d, qxyz(sp), OT, v.r_pn[1]);
        }
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let f0 = Fx::ramp_above(age, 16.0, 8.0);
        let al = fixu(fx, ee::mul(ee::mul(v.flash_glow_amp, f0), f21), v.r_fix[7]) as u8;
        rgba(fx, sp + 0x610, v.flash_glow, al);
        let age = fx.mem.i32(r + 4);
        fx.camera_vp_to(sp + 0x3E0);
        fx.translate_rows(sp + 0x420, sp + 0x460, r + 0x10);
        let s = spread(fx, age, 0x10, 1, 0x3DCC_CCCD, 0x3F66_6666);
        vec4(fx, sp + 0x500, s, s, s);
        fx.scale_rows(sp + 0x420, sp + 0x500);
        let m = fx.read_m4(sp + 0x420);
        fx.mul_into(Some(sp + 0x3E0), &m);
        project_sprite(fx, sp, sp + 0x4A0, None);
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x11 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The waves (16 updates) in a packet layer [F 0x1E0275C..0x1E02EAC].
fn waves(fx: &mut Fx, sp: u32, v: &V, x: u32) {
    quad_head(fx, sp, true);
    let d = sp + 0x570;
    let p = fx.alpha_preset(1);
    fx.desc_alpha(d, p);
    fx.desc_clamp(d, 0x0000_0800_0080_000A);
    let n = fx.mem.i32(D);
    let c = Fx::clamp_i(n - (v.wave_lo - 1), 0, v.wave_win);
    let f20 = ee::mul(v.wave_amp, ee::div(ee::from_i32(c), ee::from_i32(v.wave_win)));
    let fr = sp - v.fs;
    fx.mem.write_u8(fr + 0xD0, 0);
    let (v0, v1) = (x + WAVE_V0, x + WAVE_V1);
    for k in 0..0x10u32 {
        let r = x + WAVES + 0x40 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        fx.camera_vp_to(sp + 0x3E0);
        fx.vu_add4(sp + 0x500, r + 0x30, hdr(v, 0x50));
        fx.translate_rows(sp + 0x420, sp + 0x460, sp + 0x500);
        let m = fx.read_m4(sp + 0x420);
        fx.mul_into(Some(sp + 0x3E0), &m);
        let f22 = ee::div(ee::from_i32(fx.mem.i32(r + 4)), 16.0);
        for s6 in 0..9u32 {
            let a = ((s6 << 16) >> 3) as i32;
            let f21 = fx.tsin(a);
            let f0 = fx.tcos(a);
            let rr = r + 8 + 4 * s6;
            let o = 0x30 * s6;
            let g = |fx: &mut Fx| ee::mul(ee::mul(v.wave_grow, fx.mem.f(hdr(v, 0x70))), f22);
            let pos = |y: f32| if y < 0.0 { 0.0 } else { y };
            let r0 = g(fx);
            let r0 = pos(ee::sub(ee::sub(r0, v.wave_off0), v.wave_off));
            let r1 = g(fx);
            let r1 = pos(ee::sub(r1, v.wave_off));
            let r2 = g(fx);
            for (m, rad) in [(0u32, r0), (0x10, r1), (0x20, r2)] {
                let (px, py) = (ee::mul(rad, f21), ee::mul(rad, f0));
                vec4(fx, v0 + o + m, px, py, 0.0);
                let w = fx.mem.f(rr);
                let qx = ee::mul(w, px);
                let w = fx.mem.f(rr);
                vec4(fx, v1 + o + m, qx, ee::mul(w, py), 0.0);
            }
        }
        if fx.mem.read_u8(fr + 0xD0) == 0 {
            fx.mem.write_u8(fr + 0xD0, 1);
            let layer = fx.layer_begin();
            fx.mem.write_u32(fr + 0xEC, layer);
            // a0 / a1 as the point loop's last luis left them (the growth,
            // 1.0), a2 the
            // translation, a3 as 0x188420 left it (the turn's first word).
            let a3 = fx.mem.read_u32(sp + 0x460);
            fx.texflush([v.wave_grow.to_bits(), 0x3F80_0000, sp + 0x500, a3]);
            let h = fx.mem.i32(D + 4);
            let (_, regs) = fx.unit_draw(h, 1, sp + 0x5E8, sp + 0x5F0);
            fx.texflush(regs);
        }
        let age = fx.mem.i32(r + 4);
        let cc = fx.tcos((age << 14) >> 4);
        let al = fixu(fx, ee::mul(f20, cc), v.r_fix[8]) as u8;
        rgba(fx, sp + 0x580, v.wave_c0, al);
        copy8(fx, sp + 0x598, sp + 0x580);
        rgba(fx, sp + 0x5B0, v.wave_c1, 0);
        copy8(fx, sp + 0x5C8, sp + 0x5B0);
        for s6 in 0..8u32 {
            let (a, b) = (0x30 * s6, 0x30 * (s6 + 1));
            for (half, ras) in [(0u32, [v.r_pn[2], v.r_pn[3]]), (0x10, [v.r_pn[4], v.r_pn[5]])] {
                let (i0, o0) = (0x10, if half == 0 { 0 } else { 0x20 });
                let vs = [v1 + a + i0, v1 + b + i0, v1 + a + o0, v1 + b + o0];
                if fx.project_n(&vs, sp + 0x3E0, d, sp + 0x65C, ras[0]) == 1 {
                    for (uvo, xyz) in [(0x588u32, 0x590u32), (0x5A0, 0x5A8), (0x5B8, 0x5C0), (0x5D0, 0x5D8)] {
                        fx.uv_from_xy(sp + uvo, sp + xyz, 0, 0);
                    }
                    let vs = [v0 + a + i0, v0 + b + i0, v0 + a + o0, v0 + b + o0];
                    quad_far(fx, sp, vs, d, qxyz(sp), 0, ras[1]);
                }
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x11 {
            fx.mem.write_u8(r, 0);
        }
    }
    if fx.mem.read_u8(fr + 0xD0) == 1 {
        let layer = fx.mem.read_u32(fr + 0xEC);
        fx.layer_end(layer, OT, 0);
    }
}

/// The sparks (32 updates, drawn in to the node) [F 0x1E02F68..0x1E032A0].
fn sparks(fx: &mut Fx, sp: u32, v: &V, x: u32) {
    head(fx, sp);
    textured(fx, sp, 1, 0x10);
    camera(fx, sp);
    for k in 0..0x20u32 {
        let r = x + SPARKS + 0x120 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let s = ee::mul(2.0, ee::add(f(0x3E4C_CCCD), ee::div(ee::mul(f(0x3F4C_CCCD), age), 32.0)));
        corners(fx, sp, s, true);
        let age = fx.mem.i32(r + 4);
        let sn = fx.tsin((age << 14) >> 5);
        let al = fixu(fx, ee::mul(255.0, sn), v.r_fix[9]) as u8;
        rgba(fx, sp + 0x610, v.spark_colour, al);
        let age = fx.mem.i32(r + 4);
        let f20 = fx.tan_falloff_p(0x20 - age, 0x20, 1);
        for j in 0..0x10u32 {
            fx.vu_sub4(sp + 0x500, r + 0x20 + 0x10 * j, hdr(v, 0x20));
            fx.vu_scale4(sp + 0x500, sp + 0x500, f20);
            fx.vu_add4(sp + 0x500, sp + 0x500, hdr(v, 0x20));
            fx.vu_add4(sp + 0x4E0, sp + 0x4A0, sp + 0x500);
            fx.vu_add4(sp + 0x4F0, sp + 0x4B0, sp + 0x500);
            let b = i32::from(fx.mem.read_u8(r + 8 + j));
            cell_uvs(fx, sp, b);
            project_sprite(fx, sp, sp + 0x4E0, Some(v.r_fix[10]));
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The burst (32 updates): a far glow, a 32 x 2 quad disc, 128 far
/// sprites [F 0x1E03354..0x1E03C54].
fn burst(fx: &mut Fx, sp: u32, v: &V, bu: u32) {
    head(fx, sp);
    let d = sp + 0x600;
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    let p = fx.alpha_preset(1);
    fx.desc_alpha(d, p);
    fx.desc_clamp(d, 5);
    uvs(fx, sp, 0x600, 0, 0x800, 0x200);
    camera_at(fx, sp, bu + B_POS, None);
    let age = fx.mem.i32(bu + 4);
    let t = fx.tan_falloff_p(age, 0x20, 1);
    let al = fixu(fx, ee::mul(255.0, ee::sub(1.0, t)), v.r_fix[v.r_fix.len() - 1]) as u8;
    rgba(fx, sp + 0x610, v.burst_glow, al);
    corners(fx, sp, v.burst_size, false);
    project_sprite(fx, sp, sp + 0x4A0, None);

    let q = sp + 0x518;
    fx.mem.write_u16(q + 2, 0x14);
    fx.mem.write_u16(q, 8);
    fx.mem.write_u64(q + 8, 0x4141_4141);
    fx.mem.write_u32(q + 4, 0);
    let p = fx.alpha_preset(1);
    fx.desc_alpha(q, p);
    fx.camera_vp_to(sp + 0x3E0);
    fx.translate_rows(sp + 0x420, sp + 0x460, bu + B_POS);
    let age = fx.mem.i32(bu + 4);
    let f20 = spread(fx, age, 0x20, 2, 0x3DCC_CCCD, 0x3F66_6666);
    vec4(fx, sp + 0x500, f20, f20, 0.0);
    fx.scale_rows(sp + 0x420, sp + 0x500);
    let m = fx.read_m4(sp + 0x420);
    fx.mul_into(Some(sp + 0x3E0), &m);
    let age = fx.mem.i32(bu + 4);
    rgba(fx, sp + 0x528, v.burst_c0, (((0x20 - age) << 7) >> 5) as u8);
    copy8(fx, sp + 0x538, sp + 0x528);
    rgba(fx, sp + 0x548, v.burst_c1, 0);
    copy8(fx, sp + 0x558, sp + 0x548);
    let xyz = [sp + 0x530, sp + 0x540, sp + 0x550, sp + 0x560];
    for s5 in 0..0x20u32 {
        let (a, b) = (bu + DISC + 0x30 * s5, bu + DISC + 0x30 * (s5 + 1));
        quad_far(fx, sp, [a + 0x10, b + 0x10, a, b], q, xyz, OT, v.r_pn[6]);
        quad_far(fx, sp, [a + 0x10, b + 0x10, a + 0x20, b + 0x20], q, xyz, OT, v.r_pn[7]);
    }

    head(fx, sp);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    let p = fx.alpha_preset(1);
    fx.desc_alpha(d, p);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    let age = fx.mem.i32(bu + 4);
    rgba(fx, sp + 0x610, v.burst_sprite, ((0x20 - age).wrapping_mul(255) >> 5) as u8);
    corners(fx, sp, f(0x3F4C_CCCD), true);
    for j in 0..0x80u32 {
        let b = i32::from(fx.mem.read_u8(bu + B_CELLS + j));
        cell_uvs(fx, sp, b);
        sprite(fx, sp, bu + B_DIRS + 0x10 * j, f20, bu + B_POS, None);
    }
    let age = fx.mem.i32(bu + 4) + 1;
    fx.mem.set32(bu + 4, age);
    if age >= 0x21 {
        fx.mem.write_u8(bu, 0);
    }
}

/// b006's motes (16 x 0xA0, 16 updates): 8 sprites (texture 0) out from
/// the node and 8 (texture 3, the next cell row) drawn back in [F
/// 0x1E032C0..0x1E03934].
fn motes(fx: &mut Fx, sp: u32, v: &V, x: u32) {
    head(fx, sp);
    let d = sp + 0x600;
    let p = fx.alpha_preset(1);
    fx.desc_alpha(d, p);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    for k in 0..0x10u32 {
        let r = x + SPARKS + 0xA0 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let rh = fx.mem.read_u32(D + 0x10);
        fx.desc_texture(d, rh, 1);
        let age = fx.mem.i32(r + 4);
        let t = ee::div(ee::mul(f(0x3F4C_CCCD), ee::from_i32(0x10 - age)), 16.0);
        corners(fx, sp, ee::mul(0.5, ee::add(f(0x3E4C_CCCD), t)), true);
        let age = fx.mem.i32(r + 4);
        let sn = fx.tsin((age << 15) >> 4);
        let al = fixu(fx, ee::mul(255.0, sn), v.r_fix[9]) as u8;
        rgba(fx, sp + 0x610, v.spark_colour, al);
        let age = fx.mem.i32(r + 4);
        let f20 = spread(fx, age, 0x10, 1, 0x3DCC_CCCD, 0x3F66_6666);
        for j in 0..8u32 {
            let b = i32::from(fx.mem.read_u8(r + 8 + j));
            cell_uvs(fx, sp, b);
            sprite(fx, sp, r + 0x20 + 0x10 * j, f20, r + 0x10, Some(v.r_fix[10]));
        }
        let rh = fx.mem.read_u32(D + 0x1C);
        fx.desc_texture(d, rh, 1);
        let age = fx.mem.i32(r + 4);
        corners(fx, sp, half(5.0, age, 16.0), true);
        let age = fx.mem.i32(r + 4);
        let c = fx.tcos((age << 14) >> 4);
        let al = fixu(fx, ee::mul(48.0, c), v.r_fix[11]) as u8;
        rgba(fx, sp + 0x610, [0xFF, 0x5A, 0x2D], al);
        let age = fx.mem.i32(r + 4);
        let t = fx.tan_falloff_p(age, 0x10, 1);
        let f20 = ee::mul(f(0xBF33_3333), ee::add(f(0x3E99_999A), ee::mul(f(0x3F33_3333), t)));
        for j in 0..8u32 {
            let b = i32::from(fx.mem.read_u8(r + 8 + j)) + 4;
            cell_uvs(fx, sp, b);
            sprite(fx, sp, r + 0x20 + 0x10 * j, f20, r + 0x10, Some(v.r_fix[12]));
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x11 {
            fx.mem.write_u8(r, 0);
        }
    }
}
