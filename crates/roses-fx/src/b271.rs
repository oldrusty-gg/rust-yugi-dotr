//! BMODULE b271 (card 277), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0xE8F0
//! bytes) [F]: a single unscheduled function in a
//! 0x590 frame (s0 = the ordering table). Every update the camera turn
//! with roll (0x2055E0 into sp + 0x450) and eight attacker nodes (0x2E,
//! 0x31, 0x35, 0x38, 0x3C, 0x3F, 0x43, 0x46) into D+0x50..; the target's
//! point (0x1F65D0 kind 1) is taken once at D+0xD0. Four sprite pools,
//! each record rising and spreading by 0.1 + 0.9 x the tan falloff (as
//! 0x100D50) of its age, sized k (0.5 + 0.5 (life - age) / life), alpha
//! amp sin((age << 15) / life) x the window: 32 x 2 x 20 (D+0xE0, 0x2D0,
//! 32 updates, at two random nodes of the first / second four) and 20 x
//! 2 x 8 (D+0x5AE0, 0x140, 20 updates) on 1..0x167; 64 x 20 (D+0x73E0,
//! 0x170, 64 updates) and 40 x 8 (D+0xCFE0, 0xA0, 40 updates) at the
//! target on 0x69..0x167. On 0x69..0x187 a full-window sprite of the
//! frame (0x207EF0, filter 1, ST 0x40..0x1FC0) at alpha 80 x the window;
//! from 0x68 for 0x120 updates a lens in a packet layer: the frame copied
//! as a texture (0x207BB0 / 0x2095A0 / 0x207F50 / 0x207BB0), drawn white
//! at 128 ramp, a dark red tint (80, 0, 0) at the same alpha and the
//! frame again at 128 less it; the layer at key 0xFFFFFF. Returns 1 on
//! 0x5E, 6 on 0x69..0x167, 2 on 0x168, 8 once the count reaches 0x191.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_xy, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
/// The lens flag (u8) / age.
const LENS: u32 = D + 0xE8E0;
/// The sprite descriptor.
const DESC: u32 = 0x500;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x590);
    fx.mem.write_u32(sp + 0xBC, D);
    fx.mem.write_u32(sp + 0xC0, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp) };
    fx.leave(0x590);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0xE8F0);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    for k in 0..5u32 {
        let r = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x10 + 4 * k, r);
    }
    for (k, node) in [0x2E, 0x31, 0x35, 0x38, 0x3C, 0x3F, 0x43, 0x46].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set32(D + 0x24 + 4 * k as u32, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, D + 0xD0);
    for p in POOLS {
        for k in 0..p.count {
            fx.mem.write_u8(D + p.flag + p.stride * k, 0);
        }
    }
    fx.mem.write_u8(LENS, 0);
    0
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
    uv(fx, sp + 0x518, u0);
    uv(fx, sp + 0x51A, v0);
    uv(fx, sp + 0x528, u1);
    uv(fx, sp + 0x52A, v1);
}

/// The sprite cell `b` (4 x n cells of 32).
fn cell_uvs(fx: &mut Fx, sp: u32, b: i32) {
    let (c, r) = (rem2(b, 4), b >> 2);
    let m = |x: i32| (x & 0x3FFF) as u32;
    uvs(fx, sp, m(c << 9), m(r << 9), m((32 * c + 32) << 4), m((32 * r + 32) << 4));
}

fn rgba(fx: &mut Fx, at: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(at, c[0]);
    fx.mem.write_u8(at + 1, c[1]);
    fx.mem.write_u8(at + 2, c[2]);
    fx.mem.write_u8(at + 3, a);
    fx.mem.write_u32(at + 4, 0x3F80_0000);
}

/// The sprite descriptor at sp + 0x500 (5 registers 0x42421, prim 0x26).
fn head(fx: &mut Fx, sp: u32) {
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
}

fn textured(fx: &mut Fx, sp: u32, tex: u32) {
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(sp + DESC, rh, 1);
    let p = fx.alpha_preset(1);
    fx.desc_alpha(sp + DESC, p);
}

/// The corners (-s, -s, 0, 1) / (s, s, 0, 1) at sp + 0x490 / 0x4A0 turned
/// by the camera (sp + 0x450).
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    let n = ee::neg(s);
    fx.mem.setf(sp + 0x490, n);
    fx.mem.setf(sp + 0x494, n);
    fx.mem.write_u32(sp + 0x498, 0);
    fx.mem.write_u32(sp + 0x49C, 0x3F80_0000);
    fx.mem.setf(sp + 0x4A0, s);
    fx.mem.setf(sp + 0x4A4, s);
    fx.mem.write_u32(sp + 0x4A8, 0);
    fx.mem.write_u32(sp + 0x4AC, 0x3F80_0000);
    fx.vu_apply_to(sp + 0x490, sp + 0x450, sp + 0x490);
    fx.vu_apply_to(sp + 0x4A0, sp + 0x450, sp + 0x4A0);
}

fn window(a: i32, b: i32) -> f32 {
    ee::div(ee::mul(ee::from_i32(Fx::clamp_i(a, 0, 0x20)), ee::from_i32(Fx::clamp_i(b, 0, 0x20))), 1024.0)
}

/// A sprite pool [F 0x1E00348..0x1E019D8]: records of `groups` groups of
/// `per` sprites (cells u8, point, directions; offsets from the record).
struct Pool {
    flag: u32,
    stride: u32,
    count: u32,
    groups: u32,
    per: u32,
    cells: u32,
    cell_g: u32,
    point: u32,
    dirs: u32,
    dir_g: u32,
    life: i32,
    /// The spawn / draw window [lo, 0x168) (the draw clamp of n - (lo - 1)).
    lo: i32,
    /// At two random nodes (of the first / second four), else the target.
    at_nodes: bool,
    cell_mod: i32,
    len: u32,
    tex: u32,
    per_record: bool,
    k: f32,
    amp: f32,
    /// The rise base + k x age / life.
    rise: (f32, f32),
    colour: [u8; 3],
    /// Return addresses: rand (point, cell), 0x1A2C70 (alpha, key).
    r_rand: [u32; 2],
    r_fix: [u32; 2],
}

const POOLS: [Pool; 4] = [
    Pool {
        flag: 0xE0,
        stride: 0x2D0,
        count: 0x20,
        groups: 2,
        per: 0x14,
        cells: 0xE8,
        cell_g: 0x14,
        point: 0x110,
        dirs: 0x130,
        dir_g: 0x140,
        life: 0x20,
        lo: 1,
        at_nodes: true,
        cell_mod: 4,
        len: 0x3F80_0000,
        tex: 0x18,
        per_record: false,
        k: 0.5,
        amp: 128.0,
        rise: (-0.5, 5.0),
        colour: [0xFF, 0x64, 0x2D],
        r_rand: [0x01E0_03BC, 0x01E0_0418],
        r_fix: [0x01E0_05B4, 0x01E0_080C],
    },
    Pool {
        flag: 0x5AE0,
        stride: 0x140,
        count: 0x14,
        groups: 2,
        per: 8,
        cells: 0x5AE8,
        cell_g: 8,
        point: 0x5B00,
        dirs: 0x5B20,
        dir_g: 0x80,
        life: 0x14,
        lo: 1,
        at_nodes: true,
        cell_mod: 8,
        len: 0x3E99_999A,
        tex: 0x1C,
        per_record: true,
        k: 3.0,
        amp: 100.0,
        rise: (0.0, 0.0),
        colour: [0xFF, 0x64, 0x20],
        r_rand: [0x01E0_0930, 0x01E0_0998],
        r_fix: [0x01E0_0B70, 0x01E0_0DA4],
    },
    Pool {
        flag: 0x73E0,
        stride: 0x170,
        count: 0x40,
        groups: 1,
        per: 0x14,
        cells: 0x73E8,
        cell_g: 0,
        point: 0x7400,
        dirs: 0x7410,
        dir_g: 0,
        life: 0x40,
        lo: 0x69,
        at_nodes: false,
        cell_mod: 4,
        len: 0x4090_0000,
        tex: 0x18,
        per_record: false,
        k: 5.0,
        amp: 255.0,
        rise: (-30.0, 60.0),
        colour: [0xFF, 0xB4, 0x2D],
        r_rand: [0, 0x01E0_0F10],
        r_fix: [0x01E0_10BC, 0x01E0_1324],
    },
    Pool {
        flag: 0xCFE0,
        stride: 0xA0,
        count: 0x28,
        groups: 1,
        per: 8,
        cells: 0xCFE8,
        cell_g: 0,
        point: 0xCFF0,
        dirs: 0xD000,
        dir_g: 0,
        life: 0x28,
        lo: 0x69,
        at_nodes: false,
        cell_mod: 8,
        len: 0x4020_0000,
        tex: 0x1C,
        per_record: true,
        k: 15.0,
        amp: 200.0,
        rise: (-30.0, 60.0),
        colour: [0xFF, 0x64, 0x20],
        r_rand: [0, 0x01E0_1498],
        r_fix: [0x01E0_167C, 0x01E0_1900],
    },
];

fn spawn(fx: &mut Fx, p: &Pool) {
    let n = fx.mem.i32(D);
    if n < p.lo || n >= 0x168 {
        return;
    }
    let i = if p.count.is_power_of_two() { rem2(n, p.count as i32) } else { fx.div(n, p.count as i32).1 };
    let r = D + p.stride * i as u32;
    fx.mem.write_u8(r + p.flag, 1);
    fx.mem.write_u32(r + p.flag + 4, 0);
    for g in 0..p.groups {
        let src = if p.at_nodes {
            let b = fx.rand_from(OT, p.r_rand[0]) >> 23;
            D + 0x50 + 0x10 * (4 * g as i32 + rem2(b, 4)) as u32
        } else {
            D + 0xD0
        };
        for c in 0..4u32 {
            let w = fx.mem.read_u32(src + 4 * c);
            fx.mem.write_u32(r + p.point + 0x10 * g + 4 * c, w);
        }
        for j in 0..p.per {
            let b = fx.rand_from(OT, p.r_rand[1]) >> 23;
            fx.mem.set8(r + p.cells + p.cell_g * g + j, rem2(b, p.cell_mod));
            fx.random_table_vec(r + p.dirs + p.dir_g * g + 0x10 * j, f32::from_bits(p.len));
        }
    }
}

/// Draws the pool; true when a record was live (0x204F70 called: its
/// 0x20C670 leaves a3 = 0).
fn draw(fx: &mut Fx, sp: u32, p: &Pool) -> bool {
    let mut live = false;
    head(fx, sp);
    let d = sp + DESC;
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    fx.desc_clamp(d, 5);
    if !p.per_record {
        textured(fx, sp, p.tex);
    }
    fx.camera_vp_to(sp + 0x3D0);
    fx.identity(sp + 0x410);
    let id = fx.read_m4(sp + 0x410);
    fx.mul_into(Some(sp + 0x3D0), &id);
    let n = fx.mem.i32(D);
    let f20 = window(n - (p.lo - 1), 0x168 - n);
    let life = ee::from_i32(p.life);
    for k in 0..p.count {
        let r = D + p.stride * k;
        if fx.mem.read_u8(r + p.flag) == 0 {
            continue;
        }
        live = true;
        if p.per_record {
            textured(fx, sp, p.tex);
        }
        let age = fx.mem.i32(r + p.flag + 4);
        let s = fx.tsin((age << 15).wrapping_div(p.life));
        let al = fixu(fx, ee::mul(f20, ee::mul(p.amp, s)), p.r_fix[0]) as u8;
        let age = fx.mem.i32(r + p.flag + 4);
        let t = ee::div(ee::mul(0.5, ee::from_i32(p.life - age)), life);
        corners(fx, sp, ee::mul(p.k, ee::add(0.5, t)));
        let age = fx.mem.i32(r + p.flag + 4);
        let t = fx.tan_falloff_p(age, p.life, 1);
        let spread = ee::add(f32::from_bits(0x3DCC_CCCD), ee::mul(f32::from_bits(0x3F66_6666), t));
        let age = ee::from_i32(fx.mem.i32(r + p.flag + 4));
        let rise = ee::add(p.rise.0, ee::div(ee::mul(p.rise.1, age), life));
        rgba(fx, sp + 0x510, p.colour, al);
        for g in 0..p.groups {
            for j in 0..p.per {
                let b = i32::from(fx.mem.read_u8(r + p.cells + p.cell_g * g + j));
                cell_uvs(fx, sp, b);
                fx.vu_scale4(sp + 0x4F0, r + p.dirs + p.dir_g * g + 0x10 * j, spread);
                fx.vu_add4(sp + 0x4F0, sp + 0x4F0, r + p.point + 0x10 * g);
                let y = fx.mem.f(sp + 0x4F4);
                fx.mem.setf(sp + 0x4F4, ee::add(y, rise));
                fx.vu_add4(sp + 0x4D0, sp + 0x490, sp + 0x4F0);
                fx.vu_add4(sp + 0x4E0, sp + 0x4A0, sp + 0x4F0);
                if crate::fxlib2::project(fx, sp + 0x4D0, PROJ_OUT, sp + 0x3D0, d, sp + 0x58C) {
                    let w = fx.mem.f(sp + 0x58C);
                    let key = fixu(fx, w, p.r_fix[1]) as i32;
                    fx.submit(d, OT, key);
                }
            }
        }
        let age = fx.mem.i32(r + p.flag + 4) + 1;
        fx.mem.set32(r + p.flag + 4, age);
        if age >= p.life + 1 {
            fx.mem.write_u8(r + p.flag, 0);
        }
    }
    live
}

/// The full-window frame sprite (0x69..0x187) [F 0x1E019D8..0x1E01BC4].
/// Returns a0..a3 as the next boundary call sees them.
fn frame_sprite(fx: &mut Fx, sp: u32) -> Option<[u32; 4]> {
    let n = fx.mem.i32(D);
    let f20 = window(n - 0x68, 0x188 - n);
    if crate::exe::ee_le(f20, 0.0) {
        return None;
    }
    head(fx, sp);
    let d = sp + DESC;
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let p = fx.alpha_preset(0);
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, p);
    // a2 / a3 as 0x207E20 leaves them.
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.desc_clamp(d, 5);
    uvs(fx, sp, 0x40, 0x40, 0x1FC0, 0x1FC0);
    let al = fixu(fx, ee::mul(80.0, f20), 0x01E0_1B1C) as u8;
    rgba(fx, sp + 0x510, [0xA8, 0x8A, 0x80], al);
    far_xy(fx, sp + 0x520, 0x7000, 0x7000);
    far_xy(fx, sp + 0x530, 0x9000, 0x9000);
    fx.submit(d, OT, 0);
    // The submit (answered by the runner) leaves a0..a3 as set: the
    // descriptor, s0, 0 and the far Z's low word.
    Some([d, OT, 0, 0])
}

/// The lens (from 0x68 for 0x120 updates) [F 0x1E01BC4..0x1E020C4].
fn lens(fx: &mut Fx, sp: u32, a: [u32; 4]) {
    if fx.mem.i32(D) == 0x68 {
        fx.mem.write_u8(LENS, 1);
        fx.mem.write_u32(LENS + 4, 0);
    }
    if fx.mem.read_u8(LENS) != 1 {
        return;
    }
    let layer = fx.layer_begin();
    head(fx, sp);
    let d = sp + DESC;
    let regs = crate::lens::frame_copy(fx, a, 0, sp + 0x540, sp + 0x548);
    fx.texflush(regs);
    let p = fx.alpha_preset(0);
    fx.desc_alpha(d, p);
    fx.desc_clamp(d, LENS_CLAMP);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let ramp = |fx: &mut Fx| {
        let age = ee::from_i32(fx.mem.i32(LENS + 4));
        let f20 = ee::mul(128.0, Fx::ratio_below(age, 32.0));
        let age = ee::from_i32(fx.mem.i32(LENS + 4));
        ee::mul(f20, Fx::ramp_above(age, 288.0, 256.0))
    };
    let f = ramp(fx);
    let al = fixu(fx, f, 0x01E0_1D18) as u8;
    rgba(fx, sp + 0x510, [0xFF, 0xFF, 0xFF], al);
    uvs(fx, sp, 0, 0, 0x2000, 0x2000);
    far_xy(fx, sp + 0x520, 0x6FF8, 0x6FF8);
    far_xy(fx, sp + 0x530, 0x8FF8, 0x8FF8);
    fx.submit(d, 0, 0);
    let t = sp + 0x558;
    fx.mem.write_u16(t + 2, 6);
    fx.mem.write_u16(t, 3);
    fx.mem.write_u64(t + 8, 0x441);
    fx.mem.write_u32(t + 4, 0);
    let p = fx.alpha_preset(3);
    fx.desc_alpha(t, p);
    let f = ramp(fx);
    let s7 = fixu(fx, f, 0x01E0_1EB4) as u8;
    rgba(fx, sp + 0x568, [0x80, 0, 0], s7);
    far_xy(fx, sp + 0x570, 0x7000, 0x7000);
    far_xy(fx, sp + 0x578, 0x9000, 0x9000);
    fx.submit(t, 0, 0);
    rgba(fx, sp + 0x510, [0x80, 0x80, 0x80], 0x80u8.wrapping_sub(s7));
    uvs(fx, sp, 0, 0, 0x2000, 0x2000);
    far_xy(fx, sp + 0x520, 0x6FF8, 0x6FF8);
    far_xy(fx, sp + 0x530, 0x8FF8, 0x8FF8);
    fx.submit(d, 0, 0);
    fx.layer_end(layer, OT, 0x00FF_FFFF);
    let age = fx.mem.i32(LENS + 4) + 1;
    fx.mem.set32(LENS + 4, age);
    if age >= 0x121 {
        fx.mem.write_u8(LENS, 0);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x450);
    for k in 0..8u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x24 + 4 * k));
        fx.node_position(h, slot, D + 0x50 + 0x10 * k);
    }
    let mut live = false;
    for p in &POOLS {
        spawn(fx, p);
        live = draw(fx, sp, p);
    }
    // a0..a3 at the lens's first TEXFLUSH: the frame sprite's submit, else
    // the window's second 0x101050 (a3 as the last pool's 0x204F70 left
    // it, 0, or with none drawn as its 0x20E2B0 did: the camera record +
    // 0x40) [run].
    let n = fx.mem.i32(D);
    let b = Fx::clamp_i(0x188 - n, 0, 0x20);
    let idx = fx.mem.read_u32(crate::world::CAMERA_STACK_INDEX);
    let cam = if live { 0 } else { crate::world::CAMERA_RECORD.wrapping_add(idx.wrapping_mul(0x1A0)) + 0x40 };
    let a = frame_sprite(fx, sp).unwrap_or([b as u32, 0, 0x20, cam]);
    lens(fx, sp, a);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x5E {
        ret = 1;
    } else if (0x69..0x168).contains(&n) {
        ret = 6;
    }
    if n == 0x168 {
        ret = 2;
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x191 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xC0, ret as u32);
    ret
}
