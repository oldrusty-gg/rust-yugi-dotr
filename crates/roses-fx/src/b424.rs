//! BMODULE b424 (card 535) and b704 (card 10), hand-ported from the records
//! (linked at 0x1E00000, the entry 0x1E00080 in a 0x5C0 / 0x5A0 frame, data
//! area a0 = 0x1ED0000, 0x40F0 / 0x4120 bytes; two unit-target post
//! callbacks (0x220 / 0x50 frames: b424 0x1E03210 / 0x1E03730, b704
//! 0x1E03220 / 0x1E03750) with their static D at 0x1E03880) [F] (helper D25,
//! 3 Nov 2026): b182's program (card 40) with three new parts, on b182's
//! helpers; b704 is b424 with other parameters ([`V`]). The init lays out a
//! sphere of 17 x 17 points (D+0xF0, 0x110 a ring; radius 90 / 50) and the
//! colour ramps (D+0x1308: 13 greys with alpha 128 (12 - i) / 12, or 17
//! blues). Every update the camera turn with roll (sp + 0x2D0), the
//! attacker's / target's points (0x1F65D0 kind 1, D+0x30 / D+0x60), the
//! attacker's node 0x44 (D+0x90; b704: nodes 0xE / 0xF at D+0x90 / 0xA0 and
//! their midpoint D+0xB0) with a matrix pointing from the target's point to
//! it (D+0xA0 / D+0xC0) and the screen points of the attacker's point
//! (D+0x40) and the target's position (D+0x70). Then (b424's counts): on
//! 0xE5 a tint flash (FF, FA, C8) (33 updates, 128 (32 - age) / 32); on
//! 0x164 a white fade (0x73 updates, 128 ratio(age, 30) (1 -
//! falloff(clamp(age - 0x32, 0, 0x40), 0x40)); at age 0x1E it ends the
//! tint, the frame fan and the attacker pass) and a frame-textured fan at
//! the target's screen point (0x1F updates, alpha 128 ratio(age, 20), 1.1
//! wide); on even updates 0xE6..0x181 a two-update zoom of the frame; on
//! 0xE5 a glow at the node (0x9E updates, (96, 78, 00), flickering 0.7 /
//! 1.0); every update 0xE6..0x181 one of 16 spiral records (D+0x1340,
//! 0x1F0: six ribbons of 12 textured quads swinging out of the node, 11..16
//! updates) and from 0xE7 one of 4 bolt records (D+0x3260, 0x3A0: 32
//! textured strips at the target's point turned by random angles with a
//! sprite each, 1..4 updates); from 0x66 the attacker drawn into its target
//! with callback A (FRAME masked: a grey veil, then the sphere at the
//! attacker's point under a random turn as 16 x 16 textured quads) and
//! pulled through the eight-triangle fan at its screen point (alpha 128 x
//! ramp, keyed by its w); on even updates 0xE6..0x181 the target drawn into
//! its target with callback B (a dark veil) and composited by a full-window
//! sprite; on 0x10 a 33-update flash of the attacker (0x1F8270) as three
//! widening (96, 78, 00) fans. The stage light 0x80 - 96 clamp(n, 0, 0x40)
//! clamp(0x1D6 - n, 0, 0x40) / 4096. Returns 1 on 0xDC, 6 on 0xE7..0x181,
//! 3 on 0x182, 8 once the count reaches 0x1D7.
use crate::b182::{alpha, clamp_regs, far, far_px, fix_at, head, r256, rgba, uv};
use crate::b440::{copy8, fix, fixs, frame_mask_packet, rem2, vec4};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_xy, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's / 0x205210's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
/// The fan's nine screen points (exe rodata, 0x10 apart).
const FAN: u32 = 0x0028_ABE0;
/// The callbacks' D (static at 0x1E03880).
const D_STATIC: u32 = 0x01E0_3880;
/// The bolt strips' CLAMP (REGION_REPEAT).
const BOLT_CLAMP: u64 = 0x0000_03FC_811F_C0BA;

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

/// The record's parameters: b424 (card 535) or b704 (card 10) [F].
pub(crate) struct V {
    /// The entry's frame, and how far its locals sit below b424's.
    frame: u32,
    sps: u32,
    bzero: u32,
    /// D shifts: the attacker pass / sphere / glow and colours (`ea`), the
    /// spirals / parts / bolts / flash from D+0x1340 (`eb`).
    ea: u32,
    eb: u32,
    /// Node requests (node, kind): one node, or two with their midpoint at
    /// D+0xB0 (and the matrix at D+0xC0).
    nodes: &'static [(i32, i32)],
    radius: f32,
    /// The colour ramps: 13 greys (b424), or 17 of (64, 64, 255, 128) to
    /// (200, 200, 255, 0) (b704, mula / madd).
    ramps17: bool,
    tint: i32,
    fade: i32,
    /// Zoom / target-pass window, spiral spawns, bolt spawns.
    win: (i32, i32),
    spawn: (i32, i32),
    bolt: (i32, i32),
    /// Glow start, ramp (lo, hi), length.
    glow: (i32, f32, f32, i32),
    glow_c: [u8; 3],
    amp: f32,
    /// The spirals' turn spread (x c >> 8 - off).
    spread: (i32, i32),
    /// Quads a ribbon (12: angle / 12 through 0x2AAAAAAB; 16: >> 4).
    quads: i32,
    bolt_c: [u8; 3],
    attacker: i32,
    /// The attacker layer keyed by its point's w (b424) or 0.
    keyed: bool,
    white_c: [u8; 3],
    light: i32,
    /// Returns 1 at .0, 6 in .1, 3 at .2, 8 once the count reaches .3.
    ret: (i32, (i32, i32), i32, i32),
    sphere_c: [u8; 3],
    /// Callbacks A / B.
    cb: (u32, u32),
    /// b424's 0x1A2B80 / 0x1A2C70 / rand / 0x100BA0 return addresses (low
    /// halves) -> this record's (none: b424 itself).
    ras: &'static [(u16, u16)],
}

impl V {
    fn ra(&self, a: u32) -> u32 {
        match self.ras.iter().find(|p| u32::from(p.0) == a & 0xFFFF) {
            Some(p) => 0x01E0_0000 | u32::from(p.1),
            None => a,
        }
    }
    fn ras<const N: usize>(&self, a: &[u32; N]) -> [u32; N] {
        a.map(|x| self.ra(x))
    }
    fn two(&self) -> bool {
        self.nodes.len() == 2
    }
    /// The node point the glow / spirals use, and the spirals' matrix.
    fn mid(&self) -> u32 {
        if self.two() {
            D + 0xB0
        } else {
            D + 0x90
        }
    }
    fn mat(&self) -> u32 {
        if self.two() {
            D + 0xC0
        } else {
            D + 0xA0
        }
    }
}

pub(crate) const B424: V = V {
    frame: 0x5C0,
    sps: 0,
    bzero: 0x40F0,
    ea: 0,
    eb: 0,
    nodes: &[(0x44, 1)],
    radius: 90.0,
    ramps17: false,
    tint: 0xE5,
    fade: 0x164,
    win: (0xE6, 0x182),
    spawn: (0xE6, 0x182),
    bolt: (0xE7, 0x182),
    glow: (0xE5, 157.0, 153.0, 0x9E),
    glow_c: [0x96, 0x78, 0],
    amp: 30.0,
    spread: (1820, 0x38E),
    quads: 12,
    bolt_c: [0x96, 0x78, 0],
    attacker: 0x66,
    keyed: true,
    white_c: [0x96, 0x78, 0],
    light: 0x1D6,
    ret: (0xDC, (0xE7, 0x182), 0x182, 0x1D7),
    sphere_c: [0x80; 3],
    cb: (0x01E0_3210, 0x01E0_3730),
    ras: &[],
};

/// b704 (card 10) [F]: b424's code 0.94 alike (masked): two
/// nodes 0xE / 0xF (their midpoint D+0xB0), sphere radius 50, 17 blue
/// ramps, earlier windows, other colours, an unkeyed attacker layer, its
/// locals 0x20 lower and its D fields 0x20 / 0x30 higher.
pub(crate) const B704: V = V {
    frame: 0x5A0,
    sps: 0x20,
    bzero: 0x4120,
    ea: 0x20,
    eb: 0x30,
    nodes: &[(0xE, 1), (0xF, 0)],
    radius: 50.0,
    ramps17: true,
    tint: 0x63,
    fade: 0x126,
    win: (0x64, 0x144),
    spawn: (0x63, 0x144),
    bolt: (0x63, 0x144),
    glow: (0x62, 226.0, 222.0, 0xE3),
    glow_c: [0xC8, 0x8C, 0x3C],
    amp: 35.0,
    spread: (3640, 0x71C),
    quads: 16,
    bolt_c: [0x96, 0x78, 0x20],
    attacker: 0x30,
    keyed: false,
    white_c: [0xFA, 0xDC, 0x80],
    light: 0x1B6,
    ret: (0x58, (0x63, 0x144), 0x144, 0x1B7),
    sphere_c: [0x58, 0x58, 0xFF],
    cb: (0x01E0_3220, 0x01E0_3750),
    ras: &[
        (0x02A4, 0x02C0),
        (0x02B0, 0x02CC),
        (0x02BC, 0x02E8),
        (0x02C8, 0x02FC),
        (0x0668, 0x06C4),
        (0x0804, 0x0860),
        (0x098C, 0x09E8),
        (0x0A40, 0x0A98),
        (0x0A4C, 0x0AA4),
        (0x0A90, 0x0AE8),
        (0x0A9C, 0x0AF4),
        (0x0AE0, 0x0B38),
        (0x0AEC, 0x0B44),
        (0x0BA0, 0x0BF8),
        (0x0BAC, 0x0C04),
        (0x0C18, 0x0C70),
        (0x0C24, 0x0C7C),
        (0x0C90, 0x0CE8),
        (0x0C9C, 0x0CF4),
        (0x0E68, 0x0EC0),
        (0x0F34, 0x0F8C),
        (0x0F40, 0x0F98),
        (0x1130, 0x1188),
        (0x1370, 0x13C8),
        (0x13D4, 0x142C),
        (0x13DC, 0x1434),
        (0x1438, 0x1490),
        (0x14A4, 0x14FC),
        (0x150C, 0x1564),
        (0x1554, 0x15AC),
        (0x17C8, 0x1818),
        (0x180C, 0x185C),
        (0x1B8C, 0x1B94),
        (0x1BF0, 0x1BF8),
        (0x1BF8, 0x1C00),
        (0x1C60, 0x1C68),
        (0x1E74, 0x1E7C),
        (0x1EA4, 0x1EB0),
        (0x2660, 0x2670),
        (0x266C, 0x267C),
        (0x26C8, 0x26D8),
        (0x26D4, 0x26E4),
        (0x2718, 0x2728),
        (0x2724, 0x2734),
        (0x2768, 0x2778),
        (0x2774, 0x2784),
        (0x2830, 0x2840),
        (0x283C, 0x284C),
        (0x28A8, 0x28B8),
        (0x28B4, 0x28C4),
        (0x2920, 0x2930),
        (0x292C, 0x293C),
        (0x2DB8, 0x2DC8),
        (0x2DC4, 0x2DD4),
        (0x2E08, 0x2E18),
        (0x2E14, 0x2E24),
        (0x2E58, 0x2E68),
        (0x2E64, 0x2E74),
        (0x2F18, 0x2F28),
        (0x2F24, 0x2F34),
        (0x2F90, 0x2FA0),
        (0x2F9C, 0x2FAC),
        (0x3008, 0x3018),
        (0x3014, 0x3024),
        (0x33CC, 0x33DC),
        (0x3400, 0x3410),
        (0x3434, 0x3444),
        (0x35D4, 0x35EC),
    ],
};

/// b424's entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, &B424, param)
}

/// b704's entry 0x1E00080 (card 10).
pub fn program_b704(fx: &mut Fx, param: i32) -> i32 {
    run(fx, &B704, param)
}

fn run(fx: &mut Fx, v: &V, param: i32) -> i32 {
    let sp = fx.enter(v.frame);
    fx.mem.write_u32(sp + 0xD0, 0);
    let ret = if param >= 0 { init(fx, v) } else { step(fx, v, sp) };
    fx.leave(v.frame);
    ret
}

fn init(fx: &mut Fx, v: &V) -> i32 {
    let (ea, eb) = (v.ea, v.eb);
    fx.bzero(D, v.bzero);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let side = fx.unit_side(h);
    fx.mem.set32(D + 0xC, side);
    for (k, id) in [(0x10u32, 0), (0x14, 1), (0x18, 2), (0x1C, 3), (0x20, 4)] {
        let r = fx.texture(5, id);
        fx.mem.write_u32(D + k, r);
    }
    let r = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x24, r);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x50);
    for (k, &(node, kind)) in v.nodes.iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let s = fx.request_node(h, node, kind);
        fx.mem.set32(D + 0x80 + 4 * k as u32, s);
    }
    fx.mem.write_u8(D + 0xE0 + ea, 0);
    // The sphere: 17 rings (angle 0x800 i) of 17 points.
    for i in 0..0x11i32 {
        let c = fx.tcos((i * 0x8000) >> 4);
        let f21 = ee::mul(v.radius, c);
        let a = Fx::powf2(v.radius);
        let b = Fx::powf2(f21);
        let f20 = fx.sqrtf(ee::sub(a, b));
        let row = D + ea + 0x110 * i as u32;
        for j in 0..0x11i32 {
            let a = (j << 16) >> 4;
            let p = row + 0x10 * j as u32;
            let sn = fx.tsin(a);
            fx.mem.setf(p + 0xF0, ee::mul(f20, sn));
            fx.mem.setf(p + 0xF4, f21);
            let cs = fx.tcos(a);
            fx.mem.setf(p + 0xF8, ee::mul(f20, cs));
            fx.mem.write_u32(p + 0xFC, 0x3F80_0000);
        }
    }
    fx.mem.write_u8(D + 0x1300 + ea, 0);
    fx.mem.write_u32(D + 0x1304 + ea, 0);
    let ra = v.ras(&[0x01E0_02A4, 0x01E0_02B0, 0x01E0_02BC, 0x01E0_02C8]);
    let n = if v.ramps17 { 0x11 } else { 0xD };
    for i in 0..n {
        let c = D + 0x1308 + ea + 4 * i as u32;
        if v.ramps17 {
            let f22 = ee::div(ee::from_i32(i), 16.0);
            let f21 = ee::sub(1.0, f22);
            let f20 = ee::add(ee::mul(64.0, f21), ee::mul(200.0, f22));
            let b = fix(fx, f20, ra[0]);
            fx.mem.write_u8(c, b as u8);
            let b = fix(fx, f20, ra[1]);
            fx.mem.write_u8(c + 1, b as u8);
            let b = fix(fx, ee::add(ee::mul(255.0, f21), ee::mul(255.0, f22)), ra[2]);
            fx.mem.write_u8(c + 2, b as u8);
            let b = fix(fx, ee::mul(128.0, f21), ra[3]);
            fx.mem.write_u8(c + 3, b as u8);
            continue;
        }
        let f3 = ee::div(ee::from_i32(i), 12.0);
        let f0 = ee::mul(128.0, f3);
        let f21 = ee::mul(128.0, ee::sub(1.0, f3));
        let f20 = ee::add(f21, f0);
        for k in 0..3u32 {
            let b = fix(fx, f20, ra[k as usize]);
            fx.mem.write_u8(c + k, b as u8);
        }
        let b = fix(fx, f21, ra[3]);
        fx.mem.write_u8(c + 3, b as u8);
    }
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x1340 + eb + 0x1F0 * k, 0);
    }
    for o in [0x3240u32, 0x3248, 0x3250, 0x3258] {
        fx.mem.write_u8(D + o + eb, 0);
    }
    for k in 0..4u32 {
        fx.mem.write_u8(D + 0x3260 + eb + 0x3A0 * k, 0);
    }
    fx.mem.write_u8(D + 0x40E0 + eb, 0);
    0
}

/// The camera's view-projection at sp + 0x250 times the identity at sp +
/// 0x290.
pub(crate) fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x250);
    fx.identity(sp + 0x290);
    crate::blib::exe::mul_mem(fx, sp + 0x250, sp + 0x290);
}

/// The screen point of `p` (0x204D80 through sp + 0x250 into sp + 0x5B0):
/// x / y ((x16 >> 4) - 0x700) into `o` / `o` + 4, Z (0x17BC10) into +8,
/// 1.0 into +0xC, x / y clamped to 0..512.
pub(crate) fn screen_point(fx: &mut Fx, sp: u32, p: u32, o: u32) {
    let m = fx.read_m4(sp + 0x250);
    fx.project_at(p, &m, sp + 0x5B0, sp + 0x5BC);
    let x = (i32::from(fx.mem.read_u16(sp + 0x5B0)) >> 4) - 0x700;
    fx.mem.setf(o, ee::from_i32(x));
    let y = (i32::from(fx.mem.read_u16(sp + 0x5B2)) >> 4) - 0x700;
    fx.mem.setf(o + 4, ee::from_i32(y));
    let z = (fx.mem.read_u64(sp + 0x5B0) << 8) >> 40;
    fx.mem.setf(o + 8, z as u32 as f32);
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

/// The eight-triangle fan (descriptor `d`: UV +0x18 / +0x30 / +0x48, XYZ2
/// +0x20 / +0x38 / +0x50) about the screen point `c`: texels the fan
/// points, positions scaled by `s` about it (temporaries `tmp` / `tmp` +
/// 0x10), far Z, into `ot`; `ras` the 0x1A2B80 return addresses.
pub(crate) fn fan(fx: &mut Fx, d: u32, c: u32, tmp: u32, s: f32, ot: u32, ras: &[u32; 12]) {
    for k in 0..8u32 {
        let s1 = FAN + 0x10 * k;
        let x = fix_at(fx, c, ras[0]);
        let y = fix_at(fx, c + 4, ras[1]);
        uv(fx, d + 0x18, ((x << 4) & 0x3FFF) as u32);
        uv(fx, d + 0x1A, ((y << 4) & 0x3FFF) as u32);
        let x = fix_at(fx, s1, ras[2]);
        let y = fix_at(fx, s1 + 4, ras[3]);
        uv(fx, d + 0x30, ((x << 4) & 0x3FFF) as u32);
        uv(fx, d + 0x32, ((y << 4) & 0x3FFF) as u32);
        let x = fix_at(fx, s1 + 0x10, ras[4]);
        let y = fix_at(fx, s1 + 0x14, ras[5]);
        uv(fx, d + 0x48, ((x << 4) & 0x3FFF) as u32);
        uv(fx, d + 0x4A, ((y << 4) & 0x3FFF) as u32);
        fx.vu_sub4(tmp, s1, c);
        fx.vu_sub4(tmp + 0x10, s1 + 0x10, c);
        fx.vu_scale4(tmp, tmp, s);
        fx.vu_scale4(tmp + 0x10, tmp + 0x10, s);
        fx.vu_add4(tmp, tmp, c);
        fx.vu_add4(tmp + 0x10, tmp + 0x10, c);
        let x = fix_at(fx, c, ras[6]);
        let y = fix_at(fx, c + 4, ras[7]);
        far_px(fx, d + 0x20, x, y);
        let x = fix_at(fx, tmp, ras[8]);
        let y = fix_at(fx, tmp + 4, ras[9]);
        far_px(fx, d + 0x38, x, y);
        let x = fix_at(fx, tmp + 0x10, ras[10]);
        let y = fix_at(fx, tmp + 0x14, ras[11]);
        far_px(fx, d + 0x50, x, y);
        fx.submit(d, ot, 0);
    }
}

/// The fan's triangle descriptor at sp + 0x4C0 (prim 0x33 | 0x800, 9
/// registers), ALPHA preset `a`, the lens CLAMP.
pub(crate) fn fan_desc(fx: &mut Fx, sp: u32, a: i32) -> [u32; 4] {
    let d = sp + 0x4C0;
    head(fx, d, 0x33, 9, 0x4_2142_1421, 0x800);
    alpha(fx, d, a);
    fx.desc_clamp(d, LENS_CLAMP);
    clamp_regs(fx, d, LENS_CLAMP)
}

const FAN_T: [u32; 12] = [
    0x01E0_0A40, 0x01E0_0A4C, 0x01E0_0A90, 0x01E0_0A9C, 0x01E0_0AE0, 0x01E0_0AEC, 0x01E0_0BA0, 0x01E0_0BAC, 0x01E0_0C18, 0x01E0_0C24, 0x01E0_0C90,
    0x01E0_0C9C,
];
const FAN_A: [u32; 12] = [
    0x01E0_26C8, 0x01E0_26D4, 0x01E0_2718, 0x01E0_2724, 0x01E0_2768, 0x01E0_2774, 0x01E0_2830, 0x01E0_283C, 0x01E0_28A8, 0x01E0_28B4, 0x01E0_2920,
    0x01E0_292C,
];
const FAN_B: [u32; 12] = [
    0x01E0_2DB8, 0x01E0_2DC4, 0x01E0_2E08, 0x01E0_2E14, 0x01E0_2E58, 0x01E0_2E64, 0x01E0_2F18, 0x01E0_2F24, 0x01E0_2F90, 0x01E0_2F9C, 0x01E0_3008,
    0x01E0_3014,
];

fn within(n: i32, w: (i32, i32)) -> bool {
    (w.0..w.1).contains(&n)
}

/// `spr` the entry's stack pointer; the locals are addressed from `sp` =
/// `spr` - `v.sps` at b424's offsets.
fn step(fx: &mut Fx, v: &V, spr: u32) -> i32 {
    let sp = spr - v.sps;
    fx.camera_turn_roll(sp + 0x2D0);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x30);
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, D + 0x60);
    for k in 0..v.nodes.len() as u32 {
        let h = fx.mem.i32(D + 4);
        let slot = fx.mem.i32(D + 0x80 + 4 * k);
        fx.node_position(h, slot, D + 0x90 + 0x10 * k);
    }
    if v.two() {
        fx.vu_add4(sp + 0x390, D + 0x90, D + 0xA0);
        fx.vu_scale4(D + 0xB0, sp + 0x390, 0.5);
    }
    fx.vu_sub4(sp + 0x390, v.mid(), D + 0x60);
    fx.point_along(v.mat(), sp + 0x390);
    camera(fx, sp);
    screen_point(fx, sp, D + 0x30, D + 0x40);
    camera(fx, sp);
    screen_point(fx, sp, D + 0x50, D + 0x70);

    tint(fx, v, sp);
    fade(fx, v, sp);
    frame_fan(fx, v, sp);
    zoom(fx, v, sp);
    let f23 = glow(fx, v, sp);
    spirals(fx, v, sp, f23);
    let hi = bolts(fx, v, sp);
    attacker_pass(fx, v, sp, hi);
    target_pass(fx, v, sp);
    white_pass(fx, v, sp);

    let n = fx.mem.i32(D);
    let s0 = Fx::clamp_i(v.light - n, 0, 0x40);
    let k = Fx::clamp_i(n, 0, 0x40);
    let l = ((k.wrapping_mul(-3) << 5).wrapping_mul(s0) >> 12) + 0x80;
    fx.set_stage_light(l & 0xFF);

    let n = fx.mem.i32(D);
    let mut ret = fx.mem.i32(spr + 0xD0);
    if n == v.ret.0 {
        ret = 1;
    } else {
        if within(n, v.ret.1) {
            ret = 6;
        }
        if n == v.ret.2 {
            ret = 3;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= v.ret.3 {
        ret = 8;
    }
    fx.mem.set32(spr + 0xD0, ret);
    ret
}

/// A full-window sprite (descriptor sp + 0x3A8) of colour `c` / alpha `a`
/// at ALPHA preset `preset`, into the ordering table.
pub(crate) fn window(fx: &mut Fx, sp: u32, preset: i32, c: [u8; 3], a: impl FnOnce(&mut Fx) -> u32) {
    let t = sp + 0x3A8;
    head(fx, t, 6, 3, 0x441, 0);
    alpha(fx, t, preset);
    let a = a(fx);
    rgba(fx, sp + 0x3B8, c, a as u8);
    far_xy(fx, sp + 0x3C0, 0x7000, 0x7000);
    far_xy(fx, sp + 0x3C8, 0x9000, 0x9000);
    fx.submit(t, OT, 0);
}

/// The tint flash (D+0x3258, 33 updates) [F 0x1E005D0..0x1E0072C].
fn tint(fx: &mut Fx, v: &V, sp: u32) {
    let r = D + v.eb;
    if fx.mem.i32(D) == v.tint {
        fx.mem.write_u8(r + 0x3258, 1);
        fx.mem.write_u32(r + 0x325C, 0);
    }
    if fx.mem.read_u8(r + 0x3258) != 1 {
        return;
    }
    let ra = v.ra(0x01E0_0668);
    window(fx, sp, 0, [0xFF, 0xFA, 0xC8], |fx| {
        let age = fx.mem.i32(r + 0x325C);
        fix(fx, ee::mul(128.0, ee::div(ee::from_i32(0x20 - age), 32.0)), ra)
    });
    let age = fx.mem.i32(r + 0x325C) + 1;
    fx.mem.set32(r + 0x325C, age);
    if age >= 0x21 {
        fx.mem.write_u8(r + 0x3258, 0);
    }
}

/// The white fade (D+0x3248, 0x73 updates); at age 0x1E it ends the tint,
/// the frame fan and the attacker pass [F 0x1E00730..0x1E008C0].
fn fade(fx: &mut Fx, v: &V, sp: u32) {
    let r = D + v.eb;
    if fx.mem.i32(D) == v.fade {
        fx.mem.write_u8(r + 0x3248, 1);
        fx.mem.write_u32(r + 0x324C, 0);
    }
    if fx.mem.read_u8(r + 0x3248) != 1 {
        return;
    }
    if fx.mem.i32(r + 0x324C) == 0x1E {
        fx.mem.write_u8(r + 0x3240, 0);
        fx.mem.write_u8(r + 0x3258, 0);
        fx.mem.write_u8(D + 0xE0 + v.ea, 0);
    }
    let ra = v.ra(0x01E0_0804);
    window(fx, sp, 1, [0xFF; 3], |fx| {
        let age = fx.mem.i32(r + 0x324C);
        let c = Fx::clamp_i(age - 0x32, 0, 0x40);
        let f20 = ee::sub(1.0, fx.tan_falloff_p(c, 0x40, 1));
        let age = ee::from_i32(fx.mem.i32(r + 0x324C));
        let q = Fx::ratio_below(age, 30.0);
        fix(fx, ee::mul(128.0, ee::mul(q, f20)), ra)
    });
    let age = fx.mem.i32(r + 0x324C) + 1;
    fx.mem.set32(r + 0x324C, age);
    if age >= 0x73 {
        fx.mem.write_u8(r + 0x3248, 0);
    }
}

/// The frame-textured fan at the target's screen point (D+0x3240, 0x1F
/// updates from the fade's start) [F 0x1E008C4..0x1E00D54].
fn frame_fan(fx: &mut Fx, v: &V, sp: u32) {
    let r = D + v.eb;
    if fx.mem.i32(D) == v.fade {
        fx.mem.write_u8(r + 0x3240, 1);
        fx.mem.write_u32(r + 0x3244, 0);
    }
    if fx.mem.read_u8(r + 0x3240) != 1 {
        return;
    }
    let d = sp + 0x4C0;
    fan_desc(fx, sp, 0);
    fx.frame_desc(d, 1, [d + 0x10, d + 2]);
    let age = ee::from_i32(fx.mem.i32(r + 0x3244));
    let q = Fx::ratio_below(age, 20.0);
    let a = fix(fx, ee::mul(128.0, q), v.ra(0x01E0_098C));
    rgba(fx, sp + 0x4D0, [0x80; 3], a as u8);
    copy8(fx, sp + 0x4E8, sp + 0x4D0);
    copy8(fx, sp + 0x500, sp + 0x4D0);
    let s = ee::add(1.0, ee::mul(f(0x3DCC_CCCD), ee::from_i32(1)));
    fan(fx, d, D + 0x70, sp + 0x350, s, OT, &v.ras(&FAN_T));
    let age = fx.mem.i32(r + 0x3244) + 1;
    fx.mem.set32(r + 0x3244, age);
    if age >= 0x1F {
        fx.mem.write_u8(r + 0x3240, 0);
    }
}

/// The frame zoom (D+0x3250, two updates, on even updates of the window)
/// [F 0x1E00D58..0x1E01014].
fn zoom(fx: &mut Fx, v: &V, sp: u32) {
    let r = D + v.eb;
    let n = fx.mem.i32(D);
    if within(n, v.win) && rem2(n, 2) == 0 {
        fx.mem.write_u8(r + 0x3250, 1);
        fx.mem.write_u32(r + 0x3254, 0);
    }
    if fx.mem.read_u8(r + 0x3250) != 1 {
        return;
    }
    let d = sp + 0x3D8;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    alpha(fx, d, 0);
    fx.desc_clamp(d, LENS_CLAMP);
    let a = clamp_regs(fx, d, LENS_CLAMP);
    let layer = fx.layer_begin();
    let regs = crate::lens::frame_copy(fx, a, 1, sp + 0x418, sp + 0x420);
    fx.texflush(regs);
    let age = fx.mem.i32(r + 0x3254);
    let al = fix(fx, ee::mul(32.0, ee::from_i32(1 - age)), v.ra(0x01E0_0E68));
    rgba(fx, sp + 0x3E8, [0xFF; 3], al as u8);
    uv(fx, sp + 0x3F0, 0);
    uv(fx, sp + 0x3F2, 0);
    uv(fx, sp + 0x400, 0x2000);
    uv(fx, sp + 0x402, 0x2000);
    let ras = v.ras(&[0x01E0_0F34, 0x01E0_0F40]);
    for k in 0..4i32 {
        let t = ee::div(ee::mul(f(0x3DCC_CCCD), ee::from_i32(k + 1)), 4.0);
        let f20 = ee::mul(256.0, ee::add(1.0, t));
        let x = fixs(fx, f20, ras[0]);
        let y = fixs(fx, f20, ras[1]);
        far_xy(fx, sp + 0x3F8, ((0x800 - x) << 4) as u16, ((0x800 - y) << 4) as u16);
        far_xy(fx, sp + 0x408, ((x + 0x800) << 4) as u16, ((y + 0x800) << 4) as u16);
        fx.submit(d, 0, 0);
    }
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(r + 0x3254) + 1;
    fx.mem.set32(r + 0x3254, age);
    if age >= 2 {
        fx.mem.write_u8(r + 0x3250, 0);
    }
}

/// The node glow (D+0x1300) [F 0x1E01018..0x1E01300].
fn glow(fx: &mut Fx, v: &V, sp: u32) -> f32 {
    let g = D + v.ea;
    if fx.mem.i32(D) == v.glow.0 {
        fx.mem.write_u8(g + 0x1300, 1);
        fx.mem.write_u32(g + 0x1304, 0);
    }
    let age = ee::from_i32(fx.mem.i32(g + 0x1304));
    let f20 = Fx::ratio_below(age, 4.0);
    let age = ee::from_i32(fx.mem.i32(g + 0x1304));
    let f0 = Fx::ramp_above(age, v.glow.1, v.glow.2);
    let f23 = ee::mul(f20, f0);
    if fx.mem.read_u8(g + 0x1300) != 1 {
        return f23;
    }
    let d = sp + 0x3D8;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    let rh = fx.mem.read_u32(D + 0x1C);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    alpha(fx, d, 1);
    let m = rem2(fx.mem.i32(g + 0x1304), 2);
    let s = ee::add(f(0x3F33_3333), ee::mul(f(0x3E99_999A), ee::from_i32(m)));
    let a = fix(fx, ee::mul(ee::mul(255.0, s), f23), v.ra(0x01E0_1130));
    rgba(fx, sp + 0x3E8, v.glow_c, a as u8);
    fx.mem.setf(sp + 0x310, -35.0);
    fx.mem.setf(sp + 0x314, -35.0);
    fx.mem.write_u32(sp + 0x318, 0);
    fx.mem.write_u32(sp + 0x31C, 0x3F80_0000);
    vec4(fx, sp + 0x320, [0x420C_0000, 0x420C_0000, 0, 0x3F80_0000]);
    fx.vu_apply_to(sp + 0x310, sp + 0x2D0, sp + 0x310);
    fx.vu_apply_to(sp + 0x320, sp + 0x2D0, sp + 0x320);
    uv(fx, sp + 0x3F0, 0x600);
    uv(fx, sp + 0x3F2, 0);
    uv(fx, sp + 0x400, 0x800);
    uv(fx, sp + 0x402, 0x200);
    fx.camera_vp_to(sp + 0x250);
    fx.identity(sp + 0x290);
    fx.translate_rows(sp + 0x290, sp + 0x290, v.mid());
    crate::blib::exe::mul_mem(fx, sp + 0x250, sp + 0x290);
    if crate::fxlib2::project(fx, sp + 0x310, PROJ_OUT, sp + 0x250, d, sp + 0x5BC) {
        far(fx, sp + 0x3F8, &[sp + 0x408]);
        fx.submit(d, OT, 0);
    }
    let age = fx.mem.i32(g + 0x1304) + 1;
    fx.mem.set32(g + 0x1304, age);
    if age >= v.glow.3 {
        fx.mem.write_u8(g + 0x1300, 0);
    }
    f23
}

/// HI of 0x2AAAAAAB x `x` (the record's mult) and the sign bit of `x`.
fn div6(fx: &mut Fx, x: i32) -> i32 {
    fx.mult(0x2AAA_AAAB, x);
    let hi = fx.hilo().1;
    hi.wrapping_add(((x as u32) >> 31) as i32)
}

/// `x` / 12 as b424 divides (0x2AAAAAAB, HI >> 1 + the sign).
fn div12(fx: &mut Fx, x: i32) -> i32 {
    fx.mult(0x2AAA_AAAB, x);
    let hi = fx.hilo().1;
    (hi >> 1).wrapping_add(((x as u32) >> 31) as i32)
}

/// The spiral records (D+0x1340, 16 x 0x1F0): spawned at n % 16 every
/// update of the spawn window, drawn as six ribbons [F 0x1E01304..0x1E01B20].
fn spirals(fx: &mut Fx, v: &V, sp: u32, f23: f32) {
    let e = D + v.eb;
    let n = fx.mem.i32(D);
    if within(n, v.spawn) {
        let r = e + 0x1F0 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(r + 0x1340, 1);
        fx.mem.write_u32(r + 0x1344, 0);
        let ra = v.ras(&[0x01E0_1370, 0x01E0_13D4, 0x01E0_13DC, 0x01E0_1438, 0x01E0_14A4, 0x01E0_150C, 0x01E0_1554]);
        for m in 0..6u32 {
            let s1 = r + 4 * m;
            let s2 = r + 0x40 * m;
            let s3 = (m << 16) as i32;
            let c = r256(fx, ra[0]);
            let x = ee::div(ee::mul(f(0x3E99_999A), ee::from_i32(c)), 256.0);
            let life = fixs(fx, ee::mul(16.0, ee::add(f(0x3F33_3333), x)), ra[1]);
            fx.mem.set32(s1 + 0x1348, life);
            let c = r256(fx, ra[2]) + 1;
            let x = ee::div(ee::mul(0.5, ee::from_i32(c)), 256.0);
            fx.mem.setf(s1 + 0x1360, ee::mul(10.0, ee::add(0.5, x)));
            let c = r256(fx, ra[3]) + 1;
            let x = ee::div(ee::mul(f(0x3E99_999A), ee::from_i32(c)), 256.0);
            fx.mem.setf(s1 + 0x1378, ee::mul(-400.0, ee::add(f(0x3F33_3333), x)));
            let c = r256(fx, ra[4]) + 1;
            let x = ee::div(ee::mul(0.5, ee::from_i32(c)), 256.0);
            fx.mem.setf(s1 + 0x1390, ee::mul(v.amp, ee::add(0.5, x)));
            fx.identity(s2 + 0x13B0);
            let c = r256(fx, ra[5]);
            fx.rotate_axis(s2 + 0x13B0, ((c * v.spread.0) >> 8) - v.spread.1, 1);
            let c = r256(fx, ra[6]);
            fx.rotate_axis(s2 + 0x13B0, ((c * v.spread.0) >> 8) - v.spread.1, 0);
            let a = div6(fx, s3);
            fx.rotate_axis(s2 + 0x13B0, a, 2);
        }
    }
    let q = sp + 0x430;
    head(fx, q, 0x34, 0xC, 0x4214_2142_1421, 0);
    alpha(fx, q, 1);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(q, rh, 1);
    fx.desc_clamp(q, 5);
    for k in 0..0x10u32 {
        let rec = e + 0x1F0 * k;
        if fx.mem.read_u8(rec + 0x1340) == 0 {
            continue;
        }
        let mut s5 = (0xD05 * k) as i32;
        for m in 0..6u32 {
            let s4 = rec + 4 * m;
            let age = fx.mem.i32(rec + 0x1344);
            let life = fx.mem.i32(s4 + 0x1348);
            if life >= age {
                ribbon(fx, v, sp, rec, m, s4, s5, f23);
            }
            s5 = s5.wrapping_add(0x15B3);
        }
        let age = fx.mem.i32(rec + 0x1344) + 1;
        fx.mem.set32(rec + 0x1344, age);
        if age >= 0x11 {
            fx.mem.write_u8(rec + 0x1340, 0);
        }
    }
}

/// One ribbon of 12 / 16 quads (sub-record `s4`, phase `s5`).
#[allow(clippy::too_many_arguments)]
fn ribbon(fx: &mut Fx, v: &V, sp: u32, rec: u32, m: u32, s4: u32, s5: i32, f23: f32) {
    let q = sp + 0x430;
    fx.camera_vp_to(sp + 0x250);
    let mid = v.mid();
    let t = std::array::from_fn(|c| fx.mem.f(mid + 4 * c as u32));
    let mm = fx.trs(None, None, Some(t));
    fx.write_m4(sp + 0x290, &mm);
    crate::blib::exe::mul_mem(fx, sp + 0x290, v.mat());
    crate::blib::exe::mul_mem(fx, sp + 0x290, rec + 0x40 * m + 0x13B0);
    crate::blib::exe::mul_mem(fx, sp + 0x250, sp + 0x290);
    let age = fx.mem.i32(rec + 0x1344);
    let life = fx.mem.i32(s4 + 0x1348);
    let f0 = fx.tan_falloff_p(age, life, 1);
    let f26 = ee::sub(1.0, f0);
    let f20 = ee::div(ee::mul(32.0, ee::from_i32(rem2(m as i32, 4))), 128.0);
    let f24 = ee::add(0.25, f20);
    let f25 = ee::add(1.0, ee::sub(1.0, f26));
    let ra = v.ras(&[0x01E0_17C8, 0x01E0_180C]);
    let nq = ee::from_i32(v.quads);
    let mut s3 = 0i32;
    for s1 in 0..v.quads {
        let c = D + v.ea + 0x1308 + 4 * s1 as u32;
        let a0 = ee::from_i32(i32::from(fx.mem.read_u8(c + 3)));
        let s6 = fix(fx, ee::mul(f23, ee::mul(a0, f26)), ra[0]) & 0xFF;
        let a1 = ee::from_i32(i32::from(fx.mem.read_u8(c + 7)));
        let v0 = fix(fx, ee::mul(f23, ee::mul(a1, f26)), ra[1]);
        for k in 0..3 {
            let b = fx.mem.read_u8(c + k);
            fx.mem.write_u8(sp + 0x440 + k, b);
        }
        fx.mem.write_u8(sp + 0x443, s6 as u8);
        fx.mem.write_u32(sp + 0x444, 0x3F80_0000);
        for k in 0..3 {
            let b = fx.mem.read_u8(c + 4 + k);
            fx.mem.write_u8(sp + 0x470 + k, b);
        }
        fx.mem.write_u8(sp + 0x473, v0 as u8);
        fx.mem.write_u32(sp + 0x474, 0x3F80_0000);
        copy8(fx, sp + 0x458, sp + 0x440);
        copy8(fx, sp + 0x488, sp + 0x470);
        let t1 = rem2(s1, 4);
        let f3 = ee::div(ee::div(ee::mul(128.0, ee::from_i32(t1)), 4.0), 128.0);
        fx.mem.setf(sp + 0x448, f20);
        fx.mem.setf(sp + 0x44C, f3);
        fx.mem.setf(sp + 0x460, f24);
        fx.mem.setf(sp + 0x464, f3);
        fx.mem.setf(sp + 0x478, f20);
        let f0 = ee::div(ee::div(ee::mul(128.0, ee::from_i32(t1 + 1)), 4.0), 128.0);
        fx.mem.setf(sp + 0x47C, f0);
        fx.mem.setf(sp + 0x490, f24);
        fx.mem.setf(sp + 0x494, f0);
        let part = |fx: &mut Fx, x: i32| if v.quads == 12 { div12(fx, x) } else { x >> 4 };
        let a = s5.wrapping_add(part(fx, s3)) & 0xFFFF;
        let f21 = ee::div(ee::from_i32(s1), nq);
        let f22 = ee::div(ee::from_i32(s1 + 1), nq);
        let f4 = fx.mem.f(s4 + 0x1378);
        let f29 = ee::mul(fx.mem.f(s4 + 0x1360), f26);
        let f27 = ee::mul(f4, f21);
        let f28 = ee::mul(f4, f22);
        let sn = fx.tsin(a);
        let amp = fx.mem.f(s4 + 0x1390);
        let f21 = ee::mul(f21, ee::mul(f25, ee::mul(amp, sn)));
        let a = s5.wrapping_add(part(fx, (s1 + 1) << 16)) & 0xFFFF;
        let sn = fx.tsin(a);
        let amp = fx.mem.f(s4 + 0x1390);
        let f3 = ee::neg(f29);
        let f1 = ee::mul(f22, ee::mul(f25, ee::mul(amp, sn)));
        fx.mem.setf(sp + 0x310, f21);
        fx.mem.setf(sp + 0x314, ee::add(f3, f21));
        fx.mem.setf(sp + 0x318, f27);
        fx.mem.write_u32(sp + 0x31C, 0x3F80_0000);
        fx.mem.setf(sp + 0x320, f21);
        fx.mem.setf(sp + 0x324, ee::add(f29, f21));
        fx.mem.setf(sp + 0x328, f27);
        fx.mem.write_u32(sp + 0x32C, 0x3F80_0000);
        fx.mem.setf(sp + 0x330, f1);
        fx.mem.setf(sp + 0x334, ee::add(f3, f1));
        fx.mem.setf(sp + 0x338, f28);
        fx.mem.write_u32(sp + 0x33C, 0x3F80_0000);
        fx.mem.setf(sp + 0x340, f1);
        fx.mem.setf(sp + 0x344, ee::add(f29, f1));
        fx.mem.setf(sp + 0x348, f28);
        fx.mem.write_u32(sp + 0x34C, 0x3F80_0000);
        let mm = fx.read_m4(sp + 0x250);
        if fx.project_desc(sp + 0x310, PROJ_OUT, Some(&mm), q, sp + 0x5BC) {
            fx.submit(q, OT, 0);
        }
        s3 = s3.wrapping_add(0x10000);
    }
}

/// The bolt records (D+0x3260, 4 x 0x3A0): spawned at n % 4 every update
/// of the bolt window at the target's point, 32 strips each with a sprite
/// [F 0x1E01B24..0x1E02480]. Returns the upper half of the record's t0
/// after the last 0x188420 (`lq t0, 0x10(a1)`: the turn's row 1 z / w), 0
/// when no bolt is drawn (the glow's identity row 1).
fn bolts(fx: &mut Fx, v: &V, sp: u32) -> u64 {
    let e = D + v.eb;
    let mut hi = 0;
    let n = fx.mem.i32(D);
    if within(n, v.bolt) {
        let r = e + 0x3A0 * rem2(n, 4) as u32;
        fx.mem.write_u8(r + 0x3260, 1);
        fx.mem.write_u32(r + 0x3264, 0);
        let ra = v.ras(&[0x01E0_1B8C, 0x01E0_1BF0, 0x01E0_1BF8, 0x01E0_1C60]);
        for s4 in 0..0x20u32 {
            let s1 = r + 4 * s4;
            let s2 = r + 0x10 * s4;
            let c = r256(fx, ra[0]);
            let x = ee::div(ee::mul(f(0x3F33_3333), ee::from_i32(c)), 256.0);
            let life = fixs(fx, ee::mul(4.0, ee::add(f(0x3E99_999A), x)), ra[1]);
            fx.mem.set32(s1 + 0x3268, life);
            let c = r256(fx, ra[2]);
            let x = ee::div(ee::mul(f(0x3F33_3333), ee::from_i32(c)), 256.0);
            fx.mem.setf(s1 + 0x3368, ee::mul(10.0, ee::add(f(0x3E99_999A), x)));
            let c = r256(fx, ra[3]);
            fx.mem.set32(s1 + 0x32E8, (c << 16) >> 8);
            for w in 0..4u32 {
                let x = fx.mem.read_u32(D + 0x60 + 4 * w);
                fx.mem.write_u32(r + 0x33F0 + 4 * w, x);
            }
            fx.random_table_vec(s2 + 0x3400, 1.5);
        }
    }
    let b = sp + 0x538;
    head(fx, b, 0x24, 9, 0x4_2424_2421, 0x800);
    alpha(fx, b, 1);
    let rh = fx.mem.read_u32(D + 0x24);
    fx.desc_texture(b, rh, 1);
    fx.desc_clamp(b, BOLT_CLAMP);
    let d = sp + 0x3D8;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    alpha(fx, d, 1);
    let rh = fx.mem.read_u32(D + 0x1C);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    let ra = v.ras(&[0x01E0_1E74, 0x01E0_1EA4]);
    for k in 0..4u32 {
        let s3 = e + 0x3A0 * k;
        if fx.mem.read_u8(s3 + 0x3260) == 0 {
            continue;
        }
        let s6 = s3 + 0x33F0;
        for s5 in 0..0x20i32 {
            let s1 = s3 + 4 * s5 as u32;
            let s2 = s3 + 0x10 * s5 as u32;
            let age = fx.mem.i32(s3 + 0x3264);
            let life = fx.mem.i32(s1 + 0x3268);
            if life < age {
                continue;
            }
            let (q, _) = fx.div(age << 14, life);
            let f20 = fx.tcos(q);
            let a = fix(fx, ee::mul(64.0, f20), ra[0]);
            rgba(fx, sp + 0x548, v.bolt_c, a as u8);
            let a = fix(fx, ee::mul(255.0, f20), ra[1]);
            rgba(fx, sp + 0x3E8, v.bolt_c, a as u8);
            let c4 = rem2(s5, 4);
            let a1 = rem2(c4, 2);
            let row = ((c4 >> 1) << 6) + 0x80;
            let u0 = 59 * a1 + 0xA;
            let (t2, a3) = (((u0 << 4) & 0x3FFF) as u32, (((u0 + 0x3B) << 4) & 0x3FFF) as u32);
            let (t0, a2) = (((row << 4) & 0x3FFF) as u32, (((row + 0x40) << 4) & 0x3FFF) as u32);
            uv(fx, sp + 0x550, t2);
            uv(fx, sp + 0x552, t0);
            uv(fx, sp + 0x560, a3);
            uv(fx, sp + 0x562, t0);
            uv(fx, sp + 0x570, t2);
            uv(fx, sp + 0x572, a2);
            uv(fx, sp + 0x580, a3);
            uv(fx, sp + 0x582, a2);
            let c8 = rem2(s5, 8);
            let cu = rem2(c8, 4);
            let cv = c8 >> 2;
            uv(fx, sp + 0x3F0, ((cu << 9) & 0x3FFF) as u32);
            uv(fx, sp + 0x3F2, ((cv << 9) & 0x3FFF) as u32);
            uv(fx, sp + 0x400, ((((cu << 5) + 0x20) << 4) & 0x3FFF) as u32);
            uv(fx, sp + 0x402, ((((cv << 5) + 0x20) << 4) & 0x3FFF) as u32);
            let age = fx.mem.i32(s3 + 0x3264);
            let life = fx.mem.i32(s1 + 0x3268);
            let (q, _) = fx.div(age << 14, life);
            let sn = fx.tsin(q);
            let f20 = fx.mem.f(s1 + 0x3368);
            let f3 = ee::mul(ee::mul(0.5, f20), sn);
            let f2 = ee::neg(f20);
            let (a, b1, c2, d0) = (ee::add(f2, f3), ee::sub(f2, f3), ee::add(f20, f3), ee::sub(f20, f3));
            for (o, x, y) in [(0x310u32, a, b1), (0x320, c2, b1), (0x330, b1, c2), (0x340, d0, c2)] {
                fx.mem.setf(sp + o, x);
                fx.mem.setf(sp + o + 4, y);
                fx.mem.write_u32(sp + o + 8, 0);
                fx.mem.write_u32(sp + o + 0xC, 0x3F80_0000);
            }
            fx.camera_vp_to(sp + 0x250);
            fx.vu_add4(sp + 0x390, s2 + 0x3400, s6);
            fx.translate_rows(sp + 0x290, sp + 0x2D0, sp + 0x390);
            let ang = fx.mem.i32(s1 + 0x32E8);
            fx.rotate_axis(sp + 0x290, ang, 2);
            crate::blib::exe::mul_mem(fx, sp + 0x250, sp + 0x290);
            if crate::fxlib2::project(fx, sp + 0x310, PROJ_OUT, sp + 0x250, b, sp + 0x5BC) {
                far(fx, sp + 0x558, &[sp + 0x568, sp + 0x578, sp + 0x588]);
                fx.submit(b, OT, 0);
            }
            let s = ee::mul(f(0x3DCC_CCCD), fx.mem.f(s1 + 0x3368));
            let ns = ee::neg(s);
            for (o, x) in [(0x310u32, ns), (0x320, s)] {
                fx.mem.setf(sp + o, x);
                fx.mem.setf(sp + o + 4, x);
                fx.mem.write_u32(sp + o + 8, 0);
                fx.mem.write_u32(sp + o + 0xC, 0x3F80_0000);
            }
            fx.camera_vp_to(sp + 0x250);
            let age = ee::from_i32(fx.mem.i32(s3 + 0x3264));
            let life = ee::from_i32(fx.mem.i32(s1 + 0x3268));
            let t = ee::add(f(0x3F4C_CCCD), ee::div(ee::mul(f(0x3E4C_CCCD), age), life));
            fx.vu_scale4(sp + 0x390, s2 + 0x3400, ee::mul(-1.0, t));
            fx.vu_add4(sp + 0x390, sp + 0x390, s6);
            hi = fx.mem.read_u64(sp + 0x2E8);
            fx.translate_rows(sp + 0x290, sp + 0x2D0, sp + 0x390);
            crate::blib::exe::mul_mem(fx, sp + 0x250, sp + 0x290);
            if crate::fxlib2::project(fx, sp + 0x310, PROJ_OUT, sp + 0x250, d, sp + 0x5BC) {
                far(fx, sp + 0x3F8, &[sp + 0x408]);
                fx.submit(d, OT, 0);
            }
        }
        let age = fx.mem.i32(s3 + 0x3264) + 1;
        fx.mem.set32(s3 + 0x3264, age);
        if age >= 5 {
            fx.mem.write_u8(s3 + 0x3260, 0);
        }
    }
    hi
}

/// The attacker drawn into its target with callback A and pulled through
/// the fan (D+0xE0, ended by the fade) [F 0x1E02484..0x1E029FC]. `hi`: the
/// upper half of the record's t0 at the 0x1F8280 call.
fn attacker_pass(fx: &mut Fx, v: &V, sp: u32, hi: u64) {
    let g = D + v.ea;
    if fx.mem.i32(D) == v.attacker {
        fx.mem.write_u8(g + 0xE0, 1);
        fx.mem.write_u32(g + 0xE4, 0);
    }
    if fx.mem.read_u8(g + 0xE0) != 1 {
        return;
    }
    camera(fx, sp);
    vec4(fx, sp + 0x390, [0, 0, 0, 0x3F80_0000]);
    let y = fx.mem.read_u32(D + 0x34);
    fx.mem.write_u32(sp + 0x394, y);
    let m = fx.read_m4(sp + 0x250);
    fx.project_at(sp + 0x390, &m, sp + 0x5B0, sp + 0x5BC);
    let a = fan_desc(fx, sp, 0);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0x24C, layer);
    fx.texflush(a);
    fx.mem.write_u32(D_STATIC, D);
    let (cb, ea) = (v.cb.0, v.ea);
    let ras = v.ras(&[0x01E0_33CC, 0x01E0_3400, 0x01E0_3434, 0x01E0_35D4]);
    let c = v.sphere_c;
    let (_, regs) = fx.unit_draw_post(1, sp + 0x520, sp + 0x528, cb, &mut |fx, p| sphere(fx, p, hi, cb, D_STATIC, ea, c, &ras));
    fx.texflush(regs);
    let ramp = |fx: &mut Fx| {
        let age = ee::from_i32(fx.mem.i32(g + 0xE4));
        let a = Fx::ratio_below(age, 48.0);
        let age = ee::from_i32(fx.mem.i32(g + 0xE4));
        ee::mul(a, Fx::ramp_above(age, f(0x4587_F000), f(0x4586_7000)))
    };
    let f21 = ramp(fx);
    let f1 = ramp(fx);
    let f20 = ee::mul(128.0, f21);
    let f21 = ee::mul(128.0, f1);
    let s1 = fix(fx, f20, v.ra(0x01E0_2660)) & 0xFF;
    let v0 = fix(fx, f21, v.ra(0x01E0_266C));
    rgba(fx, sp + 0x4D0, [0x80; 3], s1 as u8);
    rgba(fx, sp + 0x4E8, [0x80; 3], v0 as u8);
    copy8(fx, sp + 0x500, sp + 0x4E8);
    fan(fx, sp + 0x4C0, D + 0x40, sp + 0x350, 1.0, 0, &v.ras(&FAN_A));
    let key = if v.keyed {
        let w = fx.mem.f(sp + 0x5BC);
        fix(fx, w, 0x01E0_29D0) as i32
    } else {
        0
    };
    let layer = fx.mem.read_u32(sp + 0x24C);
    fx.layer_end(layer, OT, key);
    let age = fx.mem.i32(g + 0xE4) + 1;
    fx.mem.set32(g + 0xE4, age);
    if age >= 0x10FF {
        fx.mem.write_u8(g + 0xE0, 0);
    }
}

/// On even updates of the window the target drawn into its target with
/// callback B, composited by a full-window sprite [F 0x1E02A00..0x1E02C10].
fn target_pass(fx: &mut Fx, v: &V, sp: u32) {
    let n = fx.mem.i32(D);
    if !(within(n, v.win) && rem2(n, 2) == 0) {
        return;
    }
    let d = sp + 0x3D8;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    alpha(fx, d, 0);
    fx.desc_clamp(d, LENS_CLAMP);
    let a = clamp_regs(fx, d, LENS_CLAMP);
    let layer = fx.layer_begin();
    fx.texflush(a);
    fx.mem.write_u32(D_STATIC, D);
    let t = fx.mem.i32(D + 8);
    let (_, regs) = fx.unit_target_post(t, 1, sp + 0x418, sp + 0x420, v.cb.1, &mut |fx, p| veil(fx, p));
    fx.texflush(regs);
    rgba(fx, sp + 0x3E8, [0x80; 3], 0x80);
    uv(fx, sp + 0x3F0, 0);
    uv(fx, sp + 0x3F2, 0);
    uv(fx, sp + 0x400, 0x2000);
    uv(fx, sp + 0x402, 0x2000);
    far_xy(fx, sp + 0x3F8, 0x7000, 0x7000);
    far_xy(fx, sp + 0x408, 0x9000, 0x9000);
    fx.submit(d, 0, 0);
    fx.layer_end(layer, OT, 0);
}

/// The attacker's flash (D+0x40E0, 33 updates from 0x10) as three widening
/// fans [F 0x1E02C14..0x1E030E0].
fn white_pass(fx: &mut Fx, v: &V, sp: u32) {
    let e = D + v.eb;
    if fx.mem.i32(D) == 0x10 {
        fx.mem.write_u8(e + 0x40E0, 1);
        fx.mem.write_u32(e + 0x40E4, 0);
    }
    if fx.mem.read_u8(e + 0x40E0) != 1 {
        return;
    }
    let a = fan_desc(fx, sp, 1);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0x248, layer);
    fx.texflush(a);
    let h = fx.mem.i32(D + 4);
    let (_, regs) = fx.unit_draw(h, 1, sp + 0x520, sp + 0x528);
    fx.texflush(regs);
    let age = fx.mem.i32(e + 0x40E4);
    rgba(fx, sp + 0x4D0, v.white_c, 0);
    rgba(fx, sp + 0x4E8, v.white_c, (((0x20 - age) << 7) >> 5) as u8);
    copy8(fx, sp + 0x500, sp + 0x4E8);
    let age = fx.mem.i32(e + 0x40E4);
    let f20 = ee::mul(f(0x3E99_999A), fx.tan_falloff_p(age, 0x20, 1));
    let ras = v.ras(&FAN_B);
    for j in 0..3i32 {
        let s = ee::add(1.0, ee::div(ee::mul(f20, ee::from_i32(j + 1)), 3.0));
        fan(fx, sp + 0x4C0, D + 0x40, sp + 0x350, s, 0, &ras);
    }
    let layer = fx.mem.read_u32(sp + 0x248);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(e + 0x40E4) + 1;
    fx.mem.set32(e + 0x40E4, age);
    if age >= 0x21 {
        fx.mem.write_u8(e + 0x40E0, 0);
    }
}

/// Callback A (b424 0x1E03210 / b704 0x1E03220)(p) [F]: b182's sphere
/// callback with the sphere at D+0xF0 (+ `ea`), texture D+0x14 every update
/// and the quads in `c`. The quad's TEX0 / TEX1 slots (sp + 0x200 / 0x208)
/// are not written when the texture is missing: they hold 0x209800's save
/// of 0x1F0FD0's s1 - the callback's address `cb` and `hi`; `dst` the
/// callbacks' static D. `ras`: the
/// three rand return addresses and 0x100BA0's.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sphere(fx: &mut Fx, p: u32, hi: u64, cb: u32, dst: u32, ea: u32, c: [u8; 3], ras: &[u32; 4]) -> u32 {
    let sp = fx.enter(0x220);
    fx.mem.write_u64(sp + 0x200, u64::from(cb));
    fx.mem.write_u64(sp + 0x208, hi);
    let d = fx.mem.read_u32(dst);
    fx.mem.write_u32(sp + 0xB0, d);
    let s0 = frame_mask_packet(fx, p, 0xFF00_0000, [0, 0]);
    let t = sp + 0x158;
    head(fx, t, 6, 3, 0x441, 0);
    alpha(fx, t, 2);
    rgba(fx, sp + 0x168, [0x40; 3], 0x80);
    far_xy(fx, sp + 0x170, 0x6FF8, 0x6FF8);
    far_xy(fx, sp + 0x178, 0x8FF8, 0x8FF8);
    let mut s6 = fx.submit_at(s0, t, 0, 0);
    let q = sp + 0x188;
    head(fx, q, 0x34, 0xC, 0x4214_2142_1421, 0);
    alpha(fx, q, 1);
    let rh = fx.mem.read_u32(d + 0x14);
    fx.desc_texture(q, rh, 1);
    fx.desc_clamp(q, 0);
    fx.camera_vp_to(sp + 0xD0);
    fx.identity(sp + 0x110);
    fx.translate_rows(sp + 0x110, sp + 0x110, d + 0x30);
    for axis in 0..3 {
        let v = fx.rand_from(s0, ras[axis as usize]) >> 23;
        fx.rotate_axis(sp + 0x110, (rem2(v, 0x100) << 16) >> 8, axis);
    }
    crate::blib::exe::mul_mem(fx, sp + 0xD0, sp + 0x110);
    rgba(fx, sp + 0x198, c, 0x80);
    for to in [0x1B0, 0x1C8, 0x1E0] {
        copy8(fx, sp + to, sp + 0x198);
    }
    // a2 / a3 at the closing 0x2096C0: 0x206F90's zeros after a drawn
    // quad, else 0x205210's untouched m / desc of the last rejected one.
    let mut a23 = [0u32, 0];
    for i in 0..0x10i32 {
        let s3 = d + ea + 0x110 * i as u32;
        let s5 = s3 + 0x110;
        let ri = rem2(i, 4);
        let f20 = ee::div(ee::from_i32(ri), 4.0);
        let f21 = ee::div(ee::from_i32(ri + 1), 4.0);
        for j in 0..0x10i32 {
            let rj = rem2(j, 4);
            let f2 = ee::div(ee::from_i32(rj), 4.0);
            let f0 = ee::div(ee::from_i32(rj + 1), 4.0);
            fx.mem.setf(sp + 0x1A0, f2);
            fx.mem.setf(sp + 0x1A4, f20);
            fx.mem.setf(sp + 0x1B8, f0);
            fx.mem.setf(sp + 0x1BC, f20);
            fx.mem.setf(sp + 0x1D0, f2);
            fx.mem.setf(sp + 0x1D4, f21);
            fx.mem.setf(sp + 0x1E8, f0);
            fx.mem.setf(sp + 0x1EC, f21);
            let (a, b) = (0x10 * j as u32 + 0xF0, 0x10 * (j + 1) as u32 + 0xF0);
            if fx.project_n(&[s3 + a, s3 + b, s5 + a, s5 + b], sp + 0xD0, q, sp + 0x21C, ras[3]) != 1 {
                a23 = [sp + 0xD0, q];
                continue;
            }
            far(fx, sp + 0x1A8, &[sp + 0x1C0, sp + 0x1D8, sp + 0x1F0]);
            s6 = fx.submit_at(s6, q, 0, 0);
            a23 = [0, 0];
        }
    }
    let r = frame_mask_packet(fx, s6, 0, a23);
    fx.leave(0x220);
    r
}

/// Callback B (b424 0x1E03730 / b704 0x1E03750)(p) [F]: b182's veil (80,
/// 80, 80, FF) at ALPHA preset 3 between FRAME_1 masks.
pub(crate) fn veil(fx: &mut Fx, p: u32) -> u32 {
    let sp = fx.enter(0x50);
    let s0 = frame_mask_packet(fx, p, 0xFF00_0000, [0, 0]);
    let t = sp + 0x20;
    head(fx, t, 6, 3, 0x441, 0);
    alpha(fx, t, 3);
    rgba(fx, sp + 0x30, [0x80; 3], 0xFF);
    far_xy(fx, sp + 0x38, 0x6FF8, 0x6FF8);
    far_xy(fx, sp + 0x40, 0x8FF8, 0x8FF8);
    let v = fx.submit_at(s0, t, 0, 0);
    let r = frame_mask_packet(fx, v, 0, [0, 0]);
    fx.leave(0x50);
    r
}
