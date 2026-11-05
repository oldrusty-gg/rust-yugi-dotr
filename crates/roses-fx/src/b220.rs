//! BMODULE b220 (card 406), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03400, data area 0x1ED0000, 0x8F40 bytes) [F]
//!: b613's program without the quad trail (s0 =
//! D, s1 = the OT, s2 the parameter). Every update the attacker's nodes
//! 0x47 / 0x26 and the target's point (0x1F65D0 kind 1) are taken; from
//! update 44 to 167 eight zaps (+0x70.., `blib::zap`) run from node 0x26
//! to points round the target and a spark burst (+0x86F0) is thrown out
//! again on a flat ring; from 180 a sprite cloud (+0x8420) spreads from
//! it, re-thrown as it fades; from 122 spinning hoops (+0x2E0,
//! `blib::hoop`) round the target. The target is frozen (0x1F84D0, a
//! runner boundary) from 120 to 258. Returns 1 on update 78, 6 on
//! 88..259, 2 on 260, 8 once the 276-update timer has run out.
//!
//! Library: `colours4` (0x1E00080), `sprites` (0x1E00120),
//! `spawn::scatter` (0x1E01A10), `trail::sparks` (0x1E01BD0), `zap::zap`
//! (0x1E02930), `hoop::setup` / `draw` (0x1E02E50 / 0x1E02FE0).
use crate::b613::unit_freeze;
use crate::b628::halves;
use crate::blib::{self, hoop, zap};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const HOOP_SETUP: u32 = 0x01E0_2E50;
const HOOP_DRAW: u32 = 0x01E0_2FE0;
/// The timer, the update counter, the parameter, the node requests.
const TIMER: u32 = D + 0x8F20;
const N: u32 = D + 0x8F24;
const CLOUD: u32 = D + 0x8420;
const BURST: u32 = D + 0x86F0;
const HOOPS: u32 = D + 0x2E0;
/// The transforms: the zaps', the cloud's / hoops', the burst's.
const XFORMS: [u32; 3] = [0, 0x270, 0x8680];

/// What differs between the family's records.
pub(crate) struct Variant {
    pub nodes: [i32; 2],
    /// The timer's start.
    pub timer: i32,
    /// The cloud's flags +4, its sprite count, size +0xC.
    pub cloud_flags: u32,
    /// The update window of the zaps and sparks, the freeze updates, the
    /// cloud's and hoops' first updates, their fades.
    pub zaps: (i32, i32),
    pub freeze: (i32, i32),
    pub cloud_from: i32,
    pub cloud_fade: i32,
    pub hoops_from: i32,
    pub hoops_fade: i32,
    pub spark_fade: i32,
    /// Return 1 / 2 updates, the window returning 6.
    pub ret1: i32,
    pub ret2: i32,
    pub ret6: (i32, i32),
    /// rand's return addresses: the cloud's two in init, the sparks'
    /// three, 0x1A2B80's in the sparks.
    pub init_ras: [u32; 2],
    pub spark_ras: [u32; 3],
    pub fix_ra: u32,
    /// 0x1038D0's three return addresses.
    pub xform_ras: [u32; 3],
    /// The burst's, zaps' and hoops' colours (B, G, R, A).
    pub burst: [u8; 4],
    pub zap: [u8; 4],
    pub hoop: [u8; 4],
    pub cloud: [u8; 4],
    /// The zaps run from node 0 (kept at +0x86A0), else from node 1.
    pub from_node0: bool,
    /// The two 0x1F84D0 calls' return addresses.
    pub freeze_ras: [u32; 2],
}

pub(crate) const B220: Variant = Variant {
    nodes: [0x47, 0x26],
    timer: 0x114,
    cloud_flags: 0x4800_0003,
    zaps: (0x2C, 0xA8),
    freeze: (0x78, 0x103),
    cloud_from: 0xB4,
    cloud_fade: 0x104,
    hoops_from: 0x7A,
    hoops_fade: 0xF4,
    spark_fade: 0x105,
    ret1: 0x4E,
    ret2: 0x104,
    ret6: (0x58, 0x104),
    init_ras: [0x01E0_36A8, 0x01E0_36D8],
    spark_ras: [0x01E0_3E04, 0x01E0_3E20, 0x01E0_3E88],
    fix_ra: 0x01E0_3DFC,
    xform_ras: [0x01E0_3A28, 0x01E0_3A30, 0x01E0_3A3C],
    burst: [0x80, 0x80, 0x80, 0xFF],
    zap: [0x80; 4],
    hoop: [0x80, 0x80, 0x80, 0x10],
    cloud: [0x80, 0x80, 0x80, 0],
    from_node0: false,
    freeze_ras: [0x01E0_3A68, 0x01E0_3A94],
};

/// b055 (card 401): nodes 0x39 / 0x38, orange, everything 50 updates
/// later (the timer 326), the zaps from node 0.
pub(crate) const B055: Variant = Variant {
    nodes: [0x39, 0x38],
    timer: 0x146,
    cloud_flags: 0x4800_1003,
    zaps: (0x6E, 0x122),
    freeze: (0xAA, 0x135),
    cloud_from: 0xE6,
    cloud_fade: 0x136,
    hoops_from: 0xAC,
    hoops_fade: 0x126,
    spark_fade: 0x137,
    ret1: 0x81,
    ret2: 0x136,
    ret6: (0x8B, 0x136),
    init_ras: [0x01E0_36B4, 0x01E0_36E4],
    spark_ras: [0x01E0_3E28, 0x01E0_3E44, 0x01E0_3EAC],
    fix_ra: 0x01E0_3E20,
    xform_ras: [0x01E0_3A4C, 0x01E0_3A54, 0x01E0_3A60],
    freeze_ras: [0x01E0_3A8C, 0x01E0_3AB8],
    burst: [0x60, 0xA0, 0xB0, 0xFF],
    zap: [0x60, 0xA0, 0xB0, 0x80],
    hoop: [0x60, 0xA0, 0xB0, 0x10],
    cloud: [0x60, 0xA0, 0xB0, 0],
    from_node0: true,
};

pub fn program_b055(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B055)
}

/// b366 (card 408): nodes 0x43 / 0x42, the cloud's flags 0x1000.
pub(crate) const B366: Variant = Variant { nodes: [0x43, 0x42], cloud_flags: 0x4800_1003, ..B220 };

/// b277 (card 407): nodes 0x13 / 0xF, green-tinted burst / zaps / hoops,
/// zaps from update 59; the code 8 bytes longer from the zaps' setup.
pub(crate) const B277: Variant = Variant {
    nodes: [0x13, 0xF],
    cloud_flags: 0x4800_1003,
    zaps: (0x3B, 0xA8),
    spark_ras: [0x01E0_3E0C, 0x01E0_3E28, 0x01E0_3E90],
    fix_ra: 0x01E0_3E04,
    xform_ras: [0x01E0_3A30, 0x01E0_3A38, 0x01E0_3A44],
    freeze_ras: [0x01E0_3A70, 0x01E0_3A9C],
    burst: [0x60, 0x80, 0x60, 0xFF],
    zap: [0x60, 0x80, 0x60, 0x80],
    hoop: [0x60, 0x80, 0x60, 0x10],
    ..B220
};

/// b409 (card 516): nodes 8 / 7, red-tinted, zaps from update 34, returns
/// 1 on 68 and 6 from 78.
pub(crate) const B409: Variant = Variant {
    nodes: [8, 7],
    cloud_flags: 0x4800_1003,
    zaps: (0x22, 0xA8),
    ret1: 0x44,
    ret6: (0x4E, 0x104),
    xform_ras: [0x01E0_3A2C, 0x01E0_3A34, 0x01E0_3A40],
    freeze_ras: [0x01E0_3A6C, 0x01E0_3A98],
    burst: [0x40, 0x40, 0x80, 0xFF],
    zap: [0x40, 0x40, 0x80, 0x80],
    hoop: [0x40, 0x40, 0x80, 0x10],
    ..B220
};

pub fn program_b366(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B366)
}
pub fn program_b277(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B277)
}
pub fn program_b409(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B409)
}

/// The entry 0x1E03400.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B220)
}

pub(crate) fn run(fx: &mut Fx, param: i32, v: &Variant) -> i32 {
    let sp = fx.enter(0xD0);
    let ret = if param >= 0 { init(fx, sp, param, v) } else { step(fx, sp, v) };
    fx.leave(0xD0);
    ret
}

fn colour(fx: &mut Fx, g: u32, bgra: [u8; 4]) {
    for (k, x) in [(0xAu32, bgra[2]), (9, bgra[1]), (8, bgra[0]), (0xB, bgra[3])] {
        fx.mem.write_u8(g + k, x);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32, v: &Variant) -> i32 {
    fx.bzero(D, 0x8F40);
    for x in XFORMS {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x8F30u32, v.nodes[0], 1), (0x8F34, v.nodes[1], 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(TIMER, v.timer);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x8F28, param);
    fx.mem.set32(D + 0x8F2C, 0);
    fx.texture5(sp + 0xC0, 0);
    let tex = |fx: &mut Fx| fx.mem.read_u32(sp + 0xC0);
    // The sprite cloud: 32 sprites at +0x8480, size 20.
    let g = CLOUD;
    let t = tex(fx);
    fx.mem.write_u32(g, t);
    fx.mem.write_u32(g + 4, v.cloud_flags);
    colour(fx, g, v.cloud);
    fx.mem.write_u32(g + 0xC, 0x41A0_0000);
    for o in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + o, 0);
    }
    halves(fx, g, &[(0x30, 0), (0x32, 0), (0x34, 0x20), (0x36, 0x20), (0x38, 0), (0x3A, 0), (0x3C, 0x80), (0x3E, 0x40)]);
    fx.mem.write_u32(g + 0x40, g + 0x60);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4170_0000);
    fx.mem.write_u32(g + 0x50, 0x4120_0000);
    halves(fx, g, &[(0x5A, 0x20), (0x5C, 8), (0x5E, 0x10)]);
    let mut k = 0;
    while k < fx.mem.i16(g + 0x5A) {
        let p = g + 0x60 + 16 * k as u32;
        fx.random_dir(sp + 0x90, 14.0);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x90 + 4 * c);
            fx.mem.write_u32(p + 4 * c, w);
        }
        let r = fx.rand_from(D, v.init_ras[0]);
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0xE, h.wrapping_neg());
        fx.mem.write_u8(p + 0xC, 0x80);
        let r = fx.rand_from(D, v.init_ras[1]);
        fx.mem.write_u8(p + 0xD, (r % 8) as u8);
        k += 1;
    }
    // The spark burst: 128 sparks at +0x8720, width 0.5, length 5.
    let g = BURST;
    let t = tex(fx);
    fx.mem.write_u32(g, t);
    fx.mem.write_u32(g + 4, 0x2000_0003);
    colour(fx, g, v.burst);
    fx.mem.write_u32(g + 0x20, 0x3F00_0000);
    fx.mem.write_u32(g + 0x24, 0x40A0_0000);
    halves(fx, g, &[(0x10, 0x20), (0x12, 0x20), (0x14, 0x20), (0x16, 0x20), (0x18, 0x20), (0x1A, 0x20), (0x1C, 0x80), (0x1E, 0x40)]);
    fx.mem.write_u32(g + 0xC, g + 0x30);
    fx.mem.write_u32(g + 0x28, 0x40A0_0000);
    fx.mem.set16(g + 0x2C, 0x80);
    fx.mem.write_u8(g + 0x2E, 1);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 4.0);
    // Eight zaps (+0x70 + 0x40 k): 28 points, width 5, cell (0x5A, 0, 6,
    // 32), starting k updates late.
    for k in 0..8i32 {
        let g = D + 0x70 + 0x40 * k as u32;
        let t = tex(fx);
        fx.mem.write_u32(g, t);
        fx.mem.write_u32(g + 4, 0);
        fx.mem.write_u32(g + 0xC, 0x40A0_0000);
        halves(fx, g, &[(0x10, 0x5A), (0x12, 0), (0x14, 6), (0x16, 0x20)]);
        colour(fx, g, v.zap);
        halves(fx, g, &[(0x28, 0x1C), (0x2C, -k), (0x2A, 0x28)]);
        fx.mem.write_u32(g + 0x20, 0);
        fx.mem.write_u32(g + 0x24, 0);
    }
    // The hoops (+0x2E0): at +0x420, 4 points at +0x320, radius 18, delays
    // up to 160, growing 3 a step.
    let g = HOOPS;
    let t = tex(fx);
    fx.mem.write_u32(g, t);
    fx.mem.write_u32(g + 4, 0x400_0000);
    colour(fx, g, v.hoop);
    fx.mem.write_u32(g + 0xC, 0x4190_0000);
    halves(fx, g, &[(0x10, 0x5A), (0x12, 0), (0x14, 6), (0x16, 0x20)]);
    fx.mem.write_u32(g + 0x20, D + 0x420);
    fx.mem.write_u32(g + 0x24, D + 0x320);
    halves(fx, g, &[(0x28, 4), (0x2A, 0x800), (0x2C, 0x28), (0x2E, 0), (0x30, 0xA0), (0x32, 3)]);
    hoop::setup(fx, HOOP_SETUP, g);
    fx.mem.write_u32(D + 0x274, 0x3FC0_0000);
    0
}

fn node(fx: &mut Fx, at: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(D + at);
    fx.node_position(h, s, out);
}

fn step(fx: &mut Fx, sp: u32, v: &Variant) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(N);
    node(fx, 0x8F30, sp + 0x90);
    node(fx, 0x8F34, sp + 0xA0);
    let t = fx.target_handle();
    fx.unit_point(1, t, sp + 0xC0);
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x290);
    fx.look_angles(sp + 0x90, sp + 0xA0, D + 0x8690);
    let src = if v.from_node0 { sp + 0x90 } else { sp + 0xA0 };
    for c in 0..4u32 {
        let w = fx.mem.read_u32(src + 4 * c);
        fx.mem.write_u32(D + 0x86A0 + 4 * c, w);
    }
    fx.mem.write_u32(D + 0x288, 0x1000);
    // s0 = D, s1 = the OT, s2 the param (-1, 64 bits), s3 the runner's.
    for (x, ra) in XFORMS.into_iter().zip(v.xform_ras) {
        fx.xform_world_saving(D + x, ra, [Some(D), Some(OT), None, None]);
        fx.saved(sp - 0xD0, &[(0x40, u64::MAX)]);
    }
    for (k, (at, f)) in [(v.freeze.0, 1u32), (v.freeze.1, 0)].into_iter().enumerate() {
        if n(fx) == at {
            let t = fx.target_handle();
            // a2 / a3 left by 0x103DB0 / the copy: the angles and +0x86A0 [run].
            fx.boundary(0x001F_84D0, "target flag 0x1F84D0", [t as u32, f, D + 0x8690, D + 0x86A0]);
            unit_freeze(fx, t, f as i32);
            // Its frame (0x20: s0 = D, ra): D's top byte is a later zap
            // XYZ's F byte, read as the sparks' missing-texture TEX0 [run].
            fx.saved(sp - 0x20, &[(0, u64::from(D)), (0x10, u64::from(v.freeze_ras[k]))]);
        }
    }
    if n(fx) >= v.cloud_from {
        fx.sprites(CLOUD, D + 0x270);
        if n(fx) >= v.cloud_fade {
            fx.step_down(CLOUD + 0xB, 0, 8);
        } else {
            fx.step_up(CLOUD + 0xB, 0x80, 2);
        }
        let mut k = 0;
        while k < fx.mem.i16(CLOUD + 0x5A) {
            let p = CLOUD + 0x60 + 16 * k as u32;
            k += 1;
            if fx.mem.i16(p + 0xE) >= 0 {
                let s = f32::from_bits(0x3F83_D70A);
                for c in 0..3u32 {
                    let x = ee::mul(fx.mem.f(p + 4 * c), s);
                    fx.mem.setf(p + 4 * c, x);
                }
                fx.step_down(p + 0xC, 0, 8);
            }
            if fx.mem.i16(p + 0xE) < 0x10 {
                continue;
            }
            fx.random_dir(sp + 0x90, 14.0);
            for c in 0..3u32 {
                let w = fx.mem.read_u32(sp + 0x90 + 4 * c);
                fx.mem.write_u32(p + 4 * c, w);
            }
            fx.mem.set16(p + 0xE, 0);
            fx.mem.write_u8(p + 0xC, 0x80);
        }
    }
    if (v.zaps.0..v.zaps.1).contains(&n(fx)) {
        for k in 0..8u32 {
            for c in 0..4u32 {
                let w = fx.mem.read_u32(sp + 0xC0 + 4 * c);
                fx.mem.write_u32(sp + 0xB0 + 4 * c, w);
            }
            let a = (0x2000 * k) as i32;
            let c = fx.tcos(a);
            let x = ee::add(fx.mem.f(sp + 0xB0), ee::mul(20.0, c));
            fx.mem.setf(sp + 0xB0, x);
            let s = fx.tsin(a);
            let y = ee::add(fx.mem.f(sp + 0xB4), ee::mul(20.0, s));
            fx.mem.setf(sp + 0xB4, y);
            let from = if v.from_node0 { D + 0x86A0 } else { sp + 0xA0 };
            zap::zap(fx, OT, D + 0x70 + 0x40 * k, D, from, sp + 0xB0);
        }
        blib::trail::sparks(fx, OT, BURST, D + 0x8680);
        if n(fx) >= v.spark_fade {
            fx.step_down(BURST + 0x13, 0, 0x10);
        }
        let mut k = 0;
        while k < fx.mem.i16(BURST + 0x2C) {
            let p = BURST + 0x30 + 16 * k as u32;
            k += 1;
            if fx.mem.i16(p + 0xE) < 0 {
                continue;
            }
            let s = f32::from_bits(0x3FA6_6666);
            for c in 0..3u32 {
                let x = ee::mul(fx.mem.f(p + 4 * c), s);
                fx.mem.setf(p + 4 * c, x);
            }
            if ee_le(fx.mem.f(p + 4), 50.0) {
                continue;
            }
            let f = ee::mul(10.0, fx.mem.f(BURST + 0x28));
            fx.fix_residue(f, v.fix_ra);
            let m = ee::fixsfsi(f);
            let a = fx.rand_from(D, v.spark_ras[0]) & 0xFFFF;
            let r = fx.rand_from(D, v.spark_ras[1]);
            let (_, h) = fx.div(r, m);
            fx.mem.write_u32(p + 4, 0x4080_0000);
            let rad = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h));
            let c = fx.tcos(a);
            fx.mem.setf(p + 8, ee::mul(rad, c));
            let s = fx.tsin(a);
            fx.mem.setf(p, ee::mul(rad, s));
            let r = fx.rand_from(D, v.spark_ras[2]);
            let (_, h) = fx.div(r, 0xC);
            fx.mem.set16(p + 0xE, h.wrapping_neg());
        }
    }
    if n(fx) >= v.hoops_from {
        hoop::draw(fx, HOOP_DRAW, OT, HOOPS, D + 0x270);
        if n(fx) >= v.hoops_fade {
            fx.step_down(HOOPS + 0xB, 0, 1);
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(N, m);
    let left = fx.mem.i32(TIMER) - 1;
    fx.mem.set32(TIMER, left);
    if m == v.ret2 {
        2
    } else if m == v.ret1 {
        1
    } else if (v.ret6.0..v.ret6.1).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
