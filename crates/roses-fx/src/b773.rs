//! BMODULE b773 (card 528), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E049E0, data area 0x1ED0000, 0x2590 bytes) [F]
//!: a c620-library record (b642's kind) whose
//! one 1,384-word entry sets up and runs about thirty groups by the update
//! counter D+0x2554 (from 1; the timer D+0x2550 = 0x1AA): a streamer from
//! the attacker's node 0x66 (0x64..0x90), a ribbon trail D+0x70 / +0xA0
//! from node 0x57 that is thrown to the midpoint and then homes on the
//! target (from 0x154), two library plates on nodes 0x3F / 0x47 (from
//! 0x8E), on 0xA6..0x13C a screen-anchored show (0x103920 billboards at
//! fixed screen points: facing quads, an 8-quad fan 0x1E017B0, two
//! rect-sized plate pairs 0x1E038D0, scanlines 0x1E02B80 and two rect
//! sprites 0x1E00120 sliding in), discs and a spark fountain at the target
//! on 0x154..0x179 and from 0x182, and a spark spray (D+0x6F0) at the
//! target while the ribbon is not homing. The stage light byte D+0x2580
//! dims to 0x40 on 0xA5, back on 0x13D. Returns 2 on 0x19A, 1 on 0x178, 6
//! on 0x182..0x199, 8 once the timer has run out.
use crate::blib::{facing, plate, spawn, streamer, trail};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, ee_le, ee_lt, Fx, OT};
use crate::fxlib::{ALPHA_42, ALPHA_44, ALPHA_88};

const D: u32 = crate::bmodule::DATA_AREA;
const N: u32 = D + 0x2554;
const TIMER: u32 = D + 0x2550;
const WAIT: u32 = D + 0x255C;
/// RGBAQ / ST / XYZF2 x 4 (a textured strip).
const REGS_TEX4: u64 = 0x4214_2142_1421;

fn n(fx: &mut Fx) -> i32 {
    fx.mem.i32(N)
}

/// The entry 0x1E049E0 (frame 0xB0; s0 = D, s1 = the OT, s2 = the
/// parameter).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    // swc1 f20: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xB0);
    ret
}

fn self_node(fx: &mut Fx, at: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(D + at);
    fx.node_position(h, s, out);
}

fn target_pos(fx: &mut Fx, out: u32) {
    let h = fx.target_handle();
    fx.unit_position(h, out);
}

fn self_pos(fx: &mut Fx, out: u32) {
    let h = fx.self_handle();
    fx.unit_position(h, out);
}

/// 0x103540(p, floor, step): *p = floor (0) when below floor + step, else
/// *p -= step (1).
fn fstep_down(fx: &mut Fx, p: u32, floor: f32, step: f32) -> i32 {
    let v = fx.mem.f(p);
    if ee_lt(v, ee::add(floor, step)) {
        fx.mem.setf(p, floor);
        0
    } else {
        fx.mem.setf(p, ee::sub(v, step));
        1
    }
}

/// 0x103390(p, cap, step): *p += step while *p <= cap - step (1), else
/// *p = cap (0).
fn fstep_up(fx: &mut Fx, p: u32, cap: f32, step: f32) -> i32 {
    let v = fx.mem.f(p);
    if ee_le(v, ee::sub(cap, step)) {
        fx.mem.setf(p, ee::add(v, step));
        1
    } else {
        fx.mem.setf(p, cap);
        0
    }
}

/// 0x103300(p, cap, step): the i16 *p += step while *p <= cap - step (1),
/// else *p = cap (0).
fn hstep_up(fx: &mut Fx, p: u32, cap: i32, step: i32) -> i32 {
    let cur = fx.mem.i16(p);
    let lim = (cap as i16 as i32) - (step as i16 as i32);
    if lim < cur {
        fx.mem.set16(p, cap);
        0
    } else {
        fx.mem.set16(p, cur + step);
        1
    }
}

/// 0x2055F0(out, x, y): the screen point (x, y) at the camera's
/// projection distance (0x1FAC30(0)), w 1, through the inverted view
/// (0x20E310 into a local, 0x20B550 in place, 0x20C540).
fn screen_point(fx: &mut Fx, out: u32, x: f32, y: f32) {
    let sp = fx.enter(0x60);
    fx.mem.setf(out, x);
    fx.mem.setf(out + 4, y);
    let dist = fx.camera_distance();
    fx.mem.setf(out + 8, dist);
    fx.mem.write_u32(out + 0xC, 0x3F80_0000);
    fx.camera_view_to(sp + 0x20);
    fx.invert4(sp + 0x20, sp + 0x20);
    let m = fx.read_m4(sp + 0x20);
    let v = fx.read_v4(out);
    let o = fx.transform(Some(&m), v);
    fx.write_v4(out, o);
    // LO / HI as 0x20B550's last soft-double multiply (0x1A31D8 ->
    // __muldi3 0x1A0EA8, its last pipeline-0 `mult` by a zero high word)
    // leaves them [H].
    fx.set_hilo(0, 0);
    fx.leave(0x60);
}

/// 0x103920(x, sx, sy): a billboard (0x1039A0) placed at the screen point
/// (sx, sy) (0x2055F0), its matrix rebuilt there (0x1038D0's body).
fn screen_billboard(fx: &mut Fx, x: u32, sx: f32, sy: f32) {
    let sp = fx.enter(0x70);
    screen_point(fx, sp + 0x20, sx, sy);
    fx.xform_billboard(x);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x20 + 4 * k);
        fx.mem.write_u32(x + 0x20 + 4 * k, w);
    }
    fx.xform_world(x);
    // The TRS matrix 0x20C0B0 left in the frame (sp + 0x30), s0 = x, ra.
    let sc = std::array::from_fn(|k| fx.mem.f(x + 4 * k as u32));
    let rot = std::array::from_fn(|k| fx.mem.i32(x + 0x10 + 4 * k as u32));
    let tr = std::array::from_fn(|k| fx.mem.f(x + 0x20 + 4 * k as u32));
    let m = fx.trs(Some(sc), Some(rot), Some(tr));
    fx.write_m4(sp + 0x30, &m);
    fx.leave(0x70);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2590);
    for x in [0u32, 0x510, 0x580, 0x5F0, 0x680, 0x1BC0, 0x1CB0, 0x1D80, 0x1E70, 0x2230, 0x1F60, 0x2050, 0x2140, 0x2320, 0x23D0, 0x2480] {
        fx.xform_init(D + x);
    }
    for (k, (node, clear)) in [(0x57, 1), (0x56, 0), (0x3F, 0), (0x47, 0), (0x66, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + 0x2560 + 4 * k as u32, r);
    }
    fx.mem.write_u32(D + 0x2550, 0x1aa);
    fx.mem.write_u32(D + 0x2554, 0x1);
    fx.mem.write_u32(D + 0x2558, param as u32);
    fx.mem.write_u32(D + 0x255c, 0xa);
    fx.mem.write_u32(D + 0x2578, 0x43160000);
    fx.mem.write_u32(D + 0x257c, 0xc3160000);
    fx.mem.write_u32(D + 0x2574, 0x424a8000);
    fx.mem.write_u8(D + 0x2580, 0x80);
    fx.mem.write_u32(D + 0x4e0, 0x1000);
    fx.mem.write_u32(D + 0x4e4, 0x3);
    fx.mem.write_u8(D + 0x4ea, 0x80);
    fx.mem.write_u8(D + 0x4e9, 0x60);
    fx.mem.write_u8(D + 0x4e8, 0x60);
    fx.mem.write_u8(D + 0x4eb, 0x20);
    fx.mem.write_u32(D + 0x4f0, 0x10000000);
    fx.mem.write_u8(D + 0x4f6, 0xf8);
    fx.mem.write_u8(D + 0x4f5, 0x38);
    fx.mem.write_u8(D + 0x4f4, 0x10);
    fx.mem.write_u8(D + 0x4f7, 0x80);
    fx.mem.write_u16(D + 0x4f8, 0x100);
    fx.mem.write_u16(D + 0x4fa, 0x100);
    fx.mem.write_u16(D + 0x4fc, 0x178);
    fx.mem.write_u16(D + 0x4fe, 0x8b);
    fx.mem.write_u32(D + 0x500, 0);
    fx.mem.write_u8(D + 0x506, 0xf8);
    fx.mem.write_u8(D + 0x505, 0x38);
    fx.mem.write_u8(D + 0x504, 0x10);
    fx.mem.write_u8(D + 0x507, 0x80);
    fx.mem.write_u16(D + 0x508, 0x178);
    fx.mem.write_u16(D + 0x50a, 0x8b);
    fx.mem.write_u16(D + 0x50c, 0x1f0);
    fx.mem.write_u16(D + 0x50e, 0x8b);
    fx.mem.write_u32(D + 0x660, 0x80000);
    fx.mem.write_u8(D + 0x666, 0x80);
    fx.mem.write_u8(D + 0x665, 0xe0);
    fx.mem.write_u8(D + 0x664, 0x80);
    fx.mem.write_u8(D + 0x667, 0x80);
    fx.mem.write_u32(D + 0x668, 0x40000000);
    fx.mem.write_u16(D + 0x66c, 0x24);
    fx.mem.write_u16(D + 0x66e, 0);
    fx.mem.write_u32(D + 0x670, 0x80000);
    fx.mem.write_u8(D + 0x676, 0x80);
    fx.mem.write_u8(D + 0x675, 0xe0);
    fx.mem.write_u8(D + 0x674, 0x80);
    fx.mem.write_u8(D + 0x677, 0xff);
    fx.mem.write_u32(D + 0x678, 0x3f800000);
    fx.mem.write_u16(D + 0x67c, 0x24);
    fx.mem.write_u16(D + 0x67e, 0);
    fx.texture4(sp + 0xA0, 0xF);
    let t = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x70, t);
    fx.mem.write_u32(D + 0x78, 0x8003);
    fx.mem.write_u8(D + 0x76, 0x70);
    fx.mem.write_u8(D + 0x75, 0xd0);
    fx.mem.write_u8(D + 0x74, 0xa0);
    fx.mem.write_u8(D + 0x77, 0x80);
    fx.mem.write_u16(D + 0x80, 0xc8);
    fx.mem.write_u16(D + 0x82, 0x20);
    fx.mem.write_u16(D + 0x84, 0x7);
    fx.mem.write_u16(D + 0x86, 0x20);
    fx.mem.write_u16(D + 0x88, 0xc8);
    fx.mem.write_u16(D + 0x8a, 0x20);
    fx.mem.write_u16(D + 0x8c, 0x100);
    fx.mem.write_u16(D + 0x8e, 0x100);
    fx.mem.write_u32(D + 0x7c, 0x41f00000);
    fx.mem.write_u16(D + 0x92, 0x8);
    fx.mem.write_u16(D + 0x94, 0);
    fx.mem.write_u16(D + 0x96, 0x1e);
    fx.mem.write_u16(D + 0x98, 0x20);
    fx.mem.write_u16(D + 0x9a, 0);
    fx.mem.write_u16(D + 0x9c, 0x1);
    let t = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x2a0, t);
    fx.mem.write_u32(D + 0x2a8, 0x9003);
    fx.mem.write_u8(D + 0x2a6, 0x80);
    fx.mem.write_u8(D + 0x2a5, 0x80);
    fx.mem.write_u8(D + 0x2a4, 0x80);
    fx.mem.write_u8(D + 0x2a7, 0x30);
    fx.mem.write_u32(D + 0x2ac, 0x40400000);
    fx.mem.write_u16(D + 0x2b0, 0x80);
    fx.mem.write_u16(D + 0x2b2, 0xc0);
    fx.mem.write_u16(D + 0x2b4, 0x20);
    fx.mem.write_u16(D + 0x2b6, 0x20);
    fx.mem.write_u16(D + 0x2b8, 0x80);
    fx.mem.write_u16(D + 0x2ba, 0xc0);
    fx.mem.write_u16(D + 0x2bc, 0x80);
    fx.mem.write_u16(D + 0x2be, 0x100);
    fx.mem.write_u32(D + 0x2d0, 0x3e4ccccd);
    fx.mem.write_u16(D + 0x2d4, 0);
    fx.mem.write_u16(D + 0x2d6, 0x10);
    fx.mem.write_u16(D + 0x2d8, 0x20);
    fx.mem.write_u16(D + 0x2da, 0);
    fx.mem.write_u16(D + 0x2dc, 0x8);
    fx.texture5(sp + 0xA0, 0);
    fx.mem.write_u32(D + 0x1e48, 0x80080001);
    fx.mem.write_u32(D + 0x1e40, 0x3f800000);
    fx.mem.write_u16(D + 0x1e10, 0);
    fx.mem.write_u16(D + 0x1e12, 0x40);
    fx.mem.write_u16(D + 0x1e14, 0x28);
    fx.mem.write_u16(D + 0x1e16, 0x14);
    fx.mem.write_u16(D + 0x1e18, 0);
    fx.mem.write_u16(D + 0x1e1a, 0x40);
    fx.mem.write_u16(D + 0x1e1c, 0x80);
    fx.mem.write_u16(D + 0x1e1e, 0x40);
    fx.mem.write_u32(D + 0x1e54, 0);
    fx.mem.write_u16(D + 0x1e58, 0);
    fx.mem.write_u16(D + 0x1e5a, 0x1);
    fx.mem.write_u16(D + 0x1e5c, 0x5);
    fx.mem.write_u16(D + 0x1e5e, 0x6);
    fx.mem.write_u16(D + 0x1e60, 0);
    fx.mem.write_u16(D + 0x1e62, 0x3e7);
    fx.mem.write_u32(D + 0x1e08, 0);
    fx.mem.write_u32(D + 0x1e04, 0);
    fx.mem.write_u32(D + 0x1e00, 0);
    fx.mem.write_u32(D + 0x1df8, 0x42200000);
    let t = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x1df0, t);
    fx.mem.write_u8(D + 0x1dfe, 0x80);
    fx.mem.write_u8(D + 0x1dfd, 0x80);
    fx.mem.write_u8(D + 0x1dfc, 0x80);
    fx.mem.write_u8(D + 0x1dff, 0x80);
    fx.mem.write_u8(D + 0x1e46, 0x80);
    fx.mem.write_u8(D + 0x1e45, 0x80);
    fx.mem.write_u8(D + 0x1e44, 0x80);
    fx.mem.write_u8(D + 0x1e47, 0x80);
    fx.mem.write_u32(D + 0x1f38, 0x80080001);
    fx.mem.write_u32(D + 0x1f30, 0x3f800000);
    fx.mem.write_u16(D + 0x1f00, 0x50);
    fx.mem.write_u16(D + 0x1f02, 0x40);
    fx.mem.write_u16(D + 0x1f04, 0x20);
    fx.mem.write_u16(D + 0x1f06, 0x40);
    fx.mem.write_u16(D + 0x1f08, 0x50);
    fx.mem.write_u16(D + 0x1f0a, 0x40);
    fx.mem.write_u16(D + 0x1f0c, 0x80);
    fx.mem.write_u16(D + 0x1f0e, 0x40);
    fx.mem.write_u32(D + 0x1f44, 0);
    fx.mem.write_u16(D + 0x1f48, 0);
    fx.mem.write_u16(D + 0x1f4a, 0);
    fx.mem.write_u16(D + 0x1f4c, 0);
    fx.mem.write_u16(D + 0x1f4e, 0x1);
    fx.mem.write_u16(D + 0x1f50, 0);
    fx.mem.write_u16(D + 0x1f52, 0x3e7);
    fx.mem.write_u32(D + 0x1ef8, 0);
    fx.mem.write_u32(D + 0x1ef4, 0);
    fx.mem.write_u32(D + 0x1ef0, 0);
    fx.mem.write_u32(D + 0x1ee8, 0x42700000);
    let t = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x1ee0, t);
    fx.mem.write_u8(D + 0x1eee, 0x80);
    fx.mem.write_u8(D + 0x1eed, 0x80);
    fx.mem.write_u8(D + 0x1eec, 0x80);
    fx.mem.write_u8(D + 0x1eef, 0x80);
    fx.mem.write_u8(D + 0x1f36, 0x80);
    fx.mem.write_u8(D + 0x1f35, 0x80);
    fx.mem.write_u8(D + 0x1f34, 0x80);
    fx.mem.write_u8(D + 0x1f37, 0x80);
    fx.texture5(sp + 0xA0, 0);
    fx.mem.write_u32(D + 0x1c88, 0x80080003);
    fx.mem.write_u32(D + 0x1c80, 0x3f800000);
    fx.mem.write_u16(D + 0x1c50, 0x40);
    fx.mem.write_u16(D + 0x1c52, 0);
    fx.mem.write_u16(D + 0x1c54, 0x40);
    fx.mem.write_u16(D + 0x1c56, 0x40);
    fx.mem.write_u16(D + 0x1c58, 0x40);
    fx.mem.write_u16(D + 0x1c5a, 0);
    fx.mem.write_u16(D + 0x1c5c, 0x80);
    fx.mem.write_u16(D + 0x1c5e, 0x40);
    fx.mem.write_u32(D + 0x1c94, 0);
    fx.mem.write_u16(D + 0x1c98, 0);
    fx.mem.write_u16(D + 0x1c9a, 0);
    fx.mem.write_u16(D + 0x1c9c, 0);
    fx.mem.write_u16(D + 0x1c9e, 0x1);
    fx.mem.write_u16(D + 0x1ca0, 0);
    fx.mem.write_u16(D + 0x1ca2, 0x3e7);
    fx.mem.write_u32(D + 0x1c48, 0);
    fx.mem.write_u32(D + 0x1c44, 0);
    fx.mem.write_u32(D + 0x1c40, 0);
    fx.mem.write_u32(D + 0x1c38, 0x42c80000);
    let t = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x1c30, t);
    fx.mem.write_u8(D + 0x1c3e, 0x80);
    fx.mem.write_u8(D + 0x1c3d, 0x80);
    fx.mem.write_u8(D + 0x1c3c, 0x80);
    fx.mem.write_u8(D + 0x1c3f, 0x80);
    fx.mem.write_u8(D + 0x1c86, 0x80);
    fx.mem.write_u8(D + 0x1c85, 0x80);
    fx.mem.write_u8(D + 0x1c84, 0x80);
    fx.mem.write_u8(D + 0x1c87, 0x80);
    let t = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x1d20, t);
    fx.mem.write_u32(D + 0x1d24, 0x80000);
    fx.mem.write_u8(D + 0x1d2a, 0x80);
    fx.mem.write_u8(D + 0x1d29, 0x80);
    fx.mem.write_u8(D + 0x1d28, 0x80);
    fx.mem.write_u8(D + 0x1d2b, 0x80);
    fx.mem.write_u32(D + 0x1d2c, 0x43250000);
    fx.mem.write_u16(D + 0x1d30, 0);
    fx.mem.write_u16(D + 0x1d32, 0);
    fx.mem.write_u16(D + 0x1d34, 0x40);
    fx.mem.write_u16(D + 0x1d36, 0x40);
    fx.mem.write_u32(D + 0x1d40, 0);
    fx.mem.write_u32(D + 0x1d44, 0);
    fx.mem.write_u32(D + 0x1d48, 0xf00);
    fx.texture5(sp + 0xA0, 1);
    let t = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x1d50, t);
    fx.mem.write_u32(D + 0x1d54, 0x80000);
    fx.mem.write_u8(D + 0x1d5a, 0x80);
    fx.mem.write_u8(D + 0x1d59, 0x80);
    fx.mem.write_u8(D + 0x1d58, 0x80);
    fx.mem.write_u8(D + 0x1d5b, 0x80);
    fx.mem.write_u32(D + 0x1d5c, 0x42aa0000);
    fx.mem.write_u16(D + 0x1d60, 0);
    fx.mem.write_u16(D + 0x1d62, 0);
    fx.mem.write_u16(D + 0x1d64, 0x40);
    fx.mem.write_u16(D + 0x1d66, 0x40);
    fx.mem.write_u32(D + 0x1d70, 0);
    fx.mem.write_u32(D + 0x1d74, 0);
    fx.mem.write_u32(D + 0x1d78, 0xfffff100);
    fx.mem.write_u32(D + 0x22f8, 0x80080003);
    fx.mem.write_u32(D + 0x22f0, 0x3f800000);
    fx.mem.write_u16(D + 0x22c0, 0x60);
    fx.mem.write_u16(D + 0x22c2, 0xa);
    fx.mem.write_u16(D + 0x22c4, 0x14);
    fx.mem.write_u16(D + 0x22c6, 0x28);
    fx.mem.write_u16(D + 0x22c8, 0x60);
    fx.mem.write_u16(D + 0x22ca, 0xa);
    fx.mem.write_u16(D + 0x22cc, 0x80);
    fx.mem.write_u16(D + 0x22ce, 0x40);
    fx.mem.write_u32(D + 0x2304, 0xffffffa6);
    fx.mem.write_u16(D + 0x2308, 0);
    fx.mem.write_u16(D + 0x230a, 0);
    fx.mem.write_u16(D + 0x230c, 0);
    fx.mem.write_u16(D + 0x230e, 0x1);
    fx.mem.write_u16(D + 0x2310, 0);
    fx.mem.write_u16(D + 0x2312, 0x3e7);
    fx.mem.write_u32(D + 0x22b8, 0);
    fx.mem.write_u32(D + 0x22b4, 0);
    fx.mem.write_u32(D + 0x22b0, 0);
    fx.mem.write_u32(D + 0x22a8, 0x41f00000);
    let t = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x22a0, t);
    fx.mem.write_u8(D + 0x22ae, 0x80);
    fx.mem.write_u8(D + 0x22ad, 0x80);
    fx.mem.write_u8(D + 0x22ac, 0x80);
    fx.mem.write_u8(D + 0x22af, 0x80);
    fx.mem.write_u8(D + 0x22f6, 0x80);
    fx.mem.write_u8(D + 0x22f5, 0x80);
    fx.mem.write_u8(D + 0x22f4, 0x80);
    fx.mem.write_u8(D + 0x22f7, 0x80);
    fx.mem.write_u32(D + 0x2028, 0x80080023);
    fx.mem.write_u32(D + 0x2020, 0x3f800000);
    fx.mem.write_u16(D + 0x1ff0, 0x40);
    fx.mem.write_u16(D + 0x1ff2, 0xa);
    fx.mem.write_u16(D + 0x1ff4, 0x20);
    fx.mem.write_u16(D + 0x1ff6, 0);
    fx.mem.write_u16(D + 0x1ff8, 0x40);
    fx.mem.write_u16(D + 0x1ffa, 0xa);
    fx.mem.write_u16(D + 0x1ffc, 0x80);
    fx.mem.write_u16(D + 0x1ffe, 0x40);
    fx.mem.write_u32(D + 0x2034, 0);
    fx.mem.write_u16(D + 0x2038, 0);
    fx.mem.write_u16(D + 0x203a, 0);
    fx.mem.write_u16(D + 0x203c, 0);
    fx.mem.write_u16(D + 0x203e, 0x1);
    fx.mem.write_u16(D + 0x2040, 0);
    fx.mem.write_u16(D + 0x2042, 0x3e7);
    fx.mem.write_u32(D + 0x1fe8, 0);
    fx.mem.write_u32(D + 0x1fe4, 0);
    fx.mem.write_u32(D + 0x1fe0, 0);
    fx.mem.write_u32(D + 0x1fd8, 0x41f00000);
    let t = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x1fd0, t);
    fx.mem.write_u8(D + 0x1fde, 0x80);
    fx.mem.write_u8(D + 0x1fdd, 0x80);
    fx.mem.write_u8(D + 0x1fdc, 0x80);
    fx.mem.write_u8(D + 0x1fdf, 0x80);
    fx.mem.write_u8(D + 0x2026, 0x80);
    fx.mem.write_u8(D + 0x2025, 0x80);
    fx.mem.write_u8(D + 0x2024, 0x80);
    fx.mem.write_u8(D + 0x2027, 0x80);
    fx.mem.write_u32(D + 0x2118, 0x80080023);
    fx.mem.write_u32(D + 0x2110, 0x3f800000);
    fx.mem.write_u16(D + 0x20e0, 0x40);
    fx.mem.write_u16(D + 0x20e2, 0xa);
    fx.mem.write_u16(D + 0x20e4, 0x20);
    fx.mem.write_u16(D + 0x20e6, 0x5);
    fx.mem.write_u16(D + 0x20e8, 0x40);
    fx.mem.write_u16(D + 0x20ea, 0xa);
    fx.mem.write_u16(D + 0x20ec, 0x80);
    fx.mem.write_u16(D + 0x20ee, 0x40);
    fx.mem.write_u32(D + 0x2124, 0);
    fx.mem.write_u16(D + 0x2128, 0);
    fx.mem.write_u16(D + 0x212a, 0);
    fx.mem.write_u16(D + 0x212c, 0);
    fx.mem.write_u16(D + 0x212e, 0x1);
    fx.mem.write_u16(D + 0x2130, 0);
    fx.mem.write_u16(D + 0x2132, 0x3e7);
    fx.mem.write_u32(D + 0x20d8, 0);
    fx.mem.write_u32(D + 0x20d4, 0);
    fx.mem.write_u32(D + 0x20d0, 0);
    fx.mem.write_u32(D + 0x20c8, 0x41f00000);
    let t = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x20c0, t);
    fx.mem.write_u8(D + 0x20ce, 0x80);
    fx.mem.write_u8(D + 0x20cd, 0x80);
    fx.mem.write_u8(D + 0x20cc, 0x80);
    fx.mem.write_u8(D + 0x20cf, 0x80);
    fx.mem.write_u8(D + 0x2116, 0x80);
    fx.mem.write_u8(D + 0x2115, 0x80);
    fx.mem.write_u8(D + 0x2114, 0x80);
    fx.mem.write_u8(D + 0x2117, 0x80);
    fx.mem.write_u32(D + 0x2208, 0x80080023);
    fx.mem.write_u32(D + 0x2200, 0x3f800000);
    fx.mem.write_u16(D + 0x21d0, 0x40);
    fx.mem.write_u16(D + 0x21d2, 0);
    fx.mem.write_u16(D + 0x21d4, 0x40);
    fx.mem.write_u16(D + 0x21d6, 0xa);
    fx.mem.write_u16(D + 0x21d8, 0x40);
    fx.mem.write_u16(D + 0x21da, 0);
    fx.mem.write_u16(D + 0x21dc, 0x80);
    fx.mem.write_u16(D + 0x21de, 0x40);
    fx.mem.write_u32(D + 0x2214, 0);
    fx.mem.write_u16(D + 0x2218, 0);
    fx.mem.write_u16(D + 0x221a, 0);
    fx.mem.write_u16(D + 0x221c, 0);
    fx.mem.write_u16(D + 0x221e, 0x1);
    fx.mem.write_u16(D + 0x2220, 0);
    fx.mem.write_u16(D + 0x2222, 0x3e7);
    fx.mem.write_u32(D + 0x21c8, 0);
    fx.mem.write_u32(D + 0x21c4, 0);
    fx.mem.write_u32(D + 0x21c0, 0);
    fx.mem.write_u32(D + 0x21b8, 0x42400000);
    let t = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x21b0, t);
    fx.mem.write_u8(D + 0x21be, 0x80);
    fx.mem.write_u8(D + 0x21bd, 0x80);
    fx.mem.write_u8(D + 0x21bc, 0x80);
    fx.mem.write_u8(D + 0x21bf, 0x80);
    fx.mem.write_u8(D + 0x2206, 0x80);
    fx.mem.write_u8(D + 0x2205, 0x80);
    fx.mem.write_u8(D + 0x2204, 0x80);
    fx.mem.write_u8(D + 0x2207, 0x80);
    let t = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x24f0, t);
    fx.mem.write_u32(D + 0x24f4, 0x80000);
    fx.mem.write_u32(D + 0x2514, 0x3f800000);
    fx.mem.write_u32(D + 0x2510, 0);
    fx.mem.write_u32(D + 0x24fc, 0x42aa0000);
    fx.mem.write_u8(D + 0x24fa, 0x80);
    fx.mem.write_u8(D + 0x24f9, 0x80);
    fx.mem.write_u8(D + 0x24f8, 0x80);
    fx.mem.write_u8(D + 0x24fb, 0x80);
    fx.mem.write_u16(D + 0x2500, 0x60);
    fx.mem.write_u16(D + 0x2502, 0x32);
    fx.mem.write_u16(D + 0x2504, 0x20);
    fx.mem.write_u16(D + 0x2506, 0xe);
    fx.mem.write_u16(D + 0x2508, 0x60);
    fx.mem.write_u16(D + 0x250a, 0x32);
    fx.mem.write_u16(D + 0x250c, 0x80);
    fx.mem.write_u16(D + 0x250e, 0x40);
    let t = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x2520, t);
    fx.mem.write_u32(D + 0x2524, 0x80000);
    fx.mem.write_u32(D + 0x2544, 0x3f800000);
    fx.mem.write_u32(D + 0x2540, 0x4000);
    fx.mem.write_u32(D + 0x252c, 0x42aa0000);
    fx.mem.write_u8(D + 0x252a, 0x80);
    fx.mem.write_u8(D + 0x2529, 0x80);
    fx.mem.write_u8(D + 0x2528, 0x80);
    fx.mem.write_u8(D + 0x252b, 0x80);
    fx.mem.write_u16(D + 0x2530, 0x60);
    fx.mem.write_u16(D + 0x2532, 0x32);
    fx.mem.write_u16(D + 0x2534, 0x20);
    fx.mem.write_u16(D + 0x2536, 0xe);
    fx.mem.write_u16(D + 0x2538, 0x60);
    fx.mem.write_u16(D + 0x253a, 0x32);
    fx.mem.write_u16(D + 0x253c, 0x80);
    fx.mem.write_u16(D + 0x253e, 0x40);
    fx.texture5(sp + 0xA0, 3);
    let t = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x6f0, t);
    fx.mem.write_u32(D + 0x6f4, 0x20000003);
    fx.mem.write_u8(D + 0x6fa, 0x80);
    fx.mem.write_u8(D + 0x6f9, 0x80);
    fx.mem.write_u8(D + 0x6f8, 0x80);
    fx.mem.write_u8(D + 0x6fb, 0x80);
    fx.mem.write_u32(D + 0x710, 0x3f800000);
    fx.mem.write_u32(D + 0x714, 0x40c00000);
    fx.mem.write_u16(D + 0x700, 0);
    fx.mem.write_u16(D + 0x702, 0);
    fx.mem.write_u16(D + 0x704, 0x20);
    fx.mem.write_u16(D + 0x706, 0x20);
    fx.mem.write_u16(D + 0x708, 0);
    fx.mem.write_u16(D + 0x70a, 0);
    fx.mem.write_u16(D + 0x70c, 0x80);
    fx.mem.write_u16(D + 0x70e, 0x40);
    fx.mem.write_u32(D + 0x6fc, D + 0x720);
    fx.mem.write_u32(D + 0x718, 0x41200000);
    fx.mem.write_u16(D + 0x71c, 0x100);
    fx.mem.write_u8(D + 0x71e, 0x5);
    fx.mem.write_u8(D + 0x71f, 0);
    spawn::scatter(fx, D + 0x6F0, 0xC, 3.0);
    let t = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x1790, t);
    fx.mem.write_u32(D + 0x1794, 0x20000003);
    fx.mem.write_u8(D + 0x179a, 0x80);
    fx.mem.write_u8(D + 0x1799, 0x80);
    fx.mem.write_u8(D + 0x1798, 0x80);
    fx.mem.write_u8(D + 0x179b, 0x80);
    fx.mem.write_u32(D + 0x17b0, 0x3f800000);
    fx.mem.write_u32(D + 0x17b4, 0x40c00000);
    fx.mem.write_u16(D + 0x17a0, 0);
    fx.mem.write_u16(D + 0x17a2, 0);
    fx.mem.write_u16(D + 0x17a4, 0x20);
    fx.mem.write_u16(D + 0x17a6, 0x20);
    fx.mem.write_u16(D + 0x17a8, 0);
    fx.mem.write_u16(D + 0x17aa, 0);
    fx.mem.write_u16(D + 0x17ac, 0x80);
    fx.mem.write_u16(D + 0x17ae, 0x40);
    fx.mem.write_u32(D + 0x179c, D + 0x17c0);
    fx.mem.write_u32(D + 0x17b8, 0x41200000);
    fx.mem.write_u16(D + 0x17bc, 0x40);
    fx.mem.write_u8(D + 0x17be, 0x5);
    fx.mem.write_u8(D + 0x17bf, 0);
    spawn::scatter(fx, D + 0x1790, 0xC, 3.0);
    fx.texture5(sp + 0xA0, 2);
    let t = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x2390, t);
    fx.mem.write_u32(D + 0x2394, 0);
    fx.mem.write_u8(D + 0x239a, 0x80);
    fx.mem.write_u8(D + 0x2399, 0x80);
    fx.mem.write_u8(D + 0x2398, 0x80);
    fx.mem.write_u8(D + 0x239b, 0x80);
    fx.mem.write_u32(D + 0x239c, 0x3e800000);
    fx.mem.write_u32(D + 0x23a0, 0x40000000);
    fx.mem.write_u32(D + 0x23a4, 0x3e800000);
    fx.mem.write_u32(D + 0x23a8, 0x3f000000);
    fx.mem.write_u16(D + 0x23b0, 0);
    fx.mem.write_u16(D + 0x23b2, 0x20);
    fx.mem.write_u16(D + 0x23b4, 0x20);
    fx.mem.write_u16(D + 0x23b6, 0x20);
    fx.mem.write_u32(D + 0x23c0, 0);
    fx.mem.write_u32(D + 0x23c4, 0);
    fx.mem.write_u32(D + 0x23c8, 0);
    let t = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x2440, t);
    fx.mem.write_u32(D + 0x2444, 0);
    fx.mem.write_u8(D + 0x244a, 0x80);
    fx.mem.write_u8(D + 0x2449, 0x80);
    fx.mem.write_u8(D + 0x2448, 0x80);
    fx.mem.write_u8(D + 0x244b, 0x80);
    fx.mem.write_u32(D + 0x244c, 0x3e800000);
    fx.mem.write_u32(D + 0x2450, 0x40000000);
    fx.mem.write_u32(D + 0x2454, 0x3e800000);
    fx.mem.write_u32(D + 0x2458, 0x3f000000);
    fx.mem.write_u16(D + 0x2460, 0);
    fx.mem.write_u16(D + 0x2462, 0x20);
    fx.mem.write_u16(D + 0x2464, 0x20);
    fx.mem.write_u16(D + 0x2466, 0x20);
    fx.mem.write_u32(D + 0x2470, 0);
    fx.mem.write_u32(D + 0x2474, 0);
    fx.mem.write_u32(D + 0x2478, 0);
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    target_pos(fx, D + 0x5A0);
    self_node(fx, 0x2568, D + 0x2340);
    self_node(fx, 0x256C, D + 0x23F0);
    if fx.mem.i32(WAIT) == 0 {
        self_pos(fx, sp + 0x80);
        target_pos(fx, sp + 0x90);
        target_pos(fx, D + 0x6A0);
        fx.look_angles(sp + 0x80, sp + 0x90, D + 0x690);
    } else if n(fx) == 0x154 {
        self_node(fx, 0x2560, D + 0x610);
        self_node(fx, 0x2560, sp + 0x80);
        self_node(fx, 0x2564, sp + 0x90);
        self_node(fx, 0x2560, D + 0x6A0);
        fx.look_angles(sp + 0x80, sp + 0x90, D + 0x690);
    }
    if n(fx) == 0x17C {
        target_pos(fx, D + 0x610);
        self_pos(fx, sp + 0x80);
        target_pos(fx, sp + 0x90);
        fx.midpoint(sp + 0x80, sp + 0x90, D + 0x530);
        fx.mem.set32(WAIT, 0xA);
        fx.mem.set16(D + 0x9A, 0);
        fx.mem.write_u32(D + 0x7C, 0x4270_0000);
    }
    if n(fx) == 0x154 {
        self_node(fx, 0x2560, D + 0x530);
    }
    if n(fx) >= 0x154 {
        fx.ribbon_push(D + 0x70, D + 0xA0, D + 0x530);
        let m = fx.mem.i32(WAIT);
        fx.approach_by(D + 0x530, D + 0x5A0, m);
        let m = fx.mem.i32(WAIT);
        if m != 0 {
            fx.mem.set32(WAIT, m - 1);
        }
    }
    let m = n(fx);
    if (0x64..0x7D).contains(&m) {
        self_node(fx, 0x2570, sp + 0xA0);
        streamer::push(fx, D + 0x2A0, D + 0x2E0, sp + 0xA0);
    }
    fx.xform_world(D);
    for x in [0x510u32, 0x580, 0x5F0] {
        fx.xform_billboard(D + x);
    }
    let y0 = fx.mem.f(D + 0x2578);
    screen_billboard(fx, D + 0x1BC0, y0, 0.0);
    let y1 = fx.mem.f(D + 0x257C);
    screen_billboard(fx, D + 0x1CB0, y1, 0.0);
    let left = f32::from_bits(0xC352_0000);
    screen_billboard(fx, D + 0x1D80, left, 90.0);
    screen_billboard(fx, D + 0x1E70, left, 0.0);
    screen_billboard(fx, D + 0x2230, left, -90.0);
    let y = fx.mem.f(D + 0x2574);
    screen_billboard(fx, D + 0x1F60, left, y);
    screen_billboard(fx, D + 0x2050, left, -55.0);
    screen_billboard(fx, D + 0x2140, 176.0, -103.0);
    fx.xform_world(D + 0x680);
    fx.xform_billboard(D + 0x2320);
    fx.xform_billboard(D + 0x23D0);
    let y0 = fx.mem.f(D + 0x2578);
    screen_billboard(fx, D + 0x2480, y0, 0.0);
    if n(fx) >= 0x8E {
        plate::plates(fx, 0x01E0_25F0, OT, D + 0x2390, D + 0x2320);
        plate::plates(fx, 0x01E0_25F0, OT, D + 0x2440, D + 0x23D0);
    }
    let m = n(fx);
    if (0xA6..0x13D).contains(&m) {
        rect_plates(fx, OT, D + 0x24F0, D + 0x2480);
        rect_plates(fx, OT, D + 0x2520, D + 0x2480);
        lines(fx, OT, D + 0x4E0);
        facing::quad(fx, OT, D + 0x1C50, D + 0x1C30, D + 0x1BC0);
        fan(fx, OT, D + 0x1D20, D + 0x1CB0);
        let tens = fx.mem.i32(N) / 10;
        if tens % 3 != 0 {
            fan(fx, OT, D + 0x1D50, D + 0x1CB0);
        }
        let v = fx.mem.i32(D + 0x1D48);
        if v != 0 {
            fx.mem.set32(D + 0x1D48, v - 0x80);
        }
        let v = fx.mem.i32(D + 0x1D78);
        if v != 0 {
            fx.mem.set32(D + 0x1D78, v + 0x80);
        }
        fstep_down(fx, D + 0x2578, 0.0, 5.0);
        if fstep_up(fx, D + 0x257C, 0.0, 5.0) == 0 {
            fx.step_down(D + 0x1D2B, 0x20, 0x10);
            if fx.mem.i32(N) % 10 < 5 {
                facing::quad(fx, OT, D + 0x21D0, D + 0x21B0, D + 0x2140);
            }
            rect_sprite(fx, OT, D + 0x4F0);
            rect_sprite(fx, OT, D + 0x500);
            facing::quad(fx, OT, D + 0x1F00, D + 0x1EE0, D + 0x1E70);
            facing::quad(fx, OT, D + 0x1FF0, D + 0x1FD0, D + 0x1F60);
            fstep_down(fx, D + 0x2574, 0.0, f32::from_bits(0x4034_0000));
            if hstep_up(fx, D + 0x1FF6, 0x36, 3) == 0 {
                if fx.mem.i32(N) % 3 != 0 {
                    facing::quad(fx, OT, D + 0x22C0, D + 0x22A0, D + 0x2230);
                    facing::quad(fx, OT, D + 0x20E0, D + 0x20C0, D + 0x2050);
                }
                let tens = fx.mem.i32(N) / 10;
                let w = if tens.wrapping_rem(2) == 0 { 0x42AA_0000 } else { 0x42A0_0000 };
                fx.mem.write_u32(D + 0x252C, w);
                fx.mem.write_u32(D + 0x24FC, w);
            }
            facing::quad(fx, OT, D + 0x1E10, D + 0x1DF0, D + 0x1D80);
        }
    }
    if n(fx) == 0xA5 {
        fx.mem.write_u8(D + 0x2580, 0x40);
    }
    if n(fx) == 0x13D {
        fx.mem.write_u8(D + 0x2580, 0x80);
    }
    let m = n(fx);
    if (0x64..0x91).contains(&m) {
        streamer::draw(fx, 0x01E0_2F30, OT, D + 0x2A0, D + 0x2E0, D);
        let mut k = 0;
        while k < fx.mem.i16(D + 0x2DA) {
            fx.step_down(D + 0x2EC + 0x10 * k as u32, 0, 2);
            k += 1;
        }
    }
    let m = n(fx);
    if (0x154..0x17A).contains(&m) {
        fx.disc(D + 0x660, D + 0x5F0);
        fx.disc(D + 0x670, D + 0x5F0);
        if fx.approach(D + 0x5F0, 5.0, 0.5) == 0 {
            fx.step_down(D + 0x667, 0, 8);
            fx.step_down(D + 0x677, 0, 8);
        }
        trail::sparks(fx, OT, D + 0x1790, D + 0x680);
        fx.step_down(D + 0x179B, 0, 0x10);
        let s = f32::from_bits(0x3F99_999A);
        let mut k = 0;
        while k < fx.mem.i16(D + 0x17BC) {
            let p = D + 0x17C0 + 0x10 * k as u32;
            for c in 0..3u32 {
                let v = ee::mul(fx.mem.f(p + 4 * c), s);
                fx.mem.setf(p + 4 * c, v);
            }
            k += 1;
        }
    }
    if n(fx) == 0x182 {
        for k in [0x5F8u32, 0x5F4, 0x5F0] {
            fx.mem.write_u32(D + k, 0x3F80_0000);
        }
        fx.mem.write_u8(D + 0x667, 0x80);
        fx.mem.write_u8(D + 0x677, 0xFF);
    }
    if n(fx) >= 0x182 {
        fx.disc(D + 0x660, D + 0x5F0);
        fx.disc(D + 0x670, D + 0x5F0);
        fx.approach(D + 0x5F0, 50.0, 0.5);
        if !ee_le(fx.mem.f(D + 0x5F0), 10.0) {
            fx.step_down(D + 0x667, 0, 8);
            fx.step_down(D + 0x677, 0, 0x10);
        }
    }
    if fx.mem.i32(WAIT) == 0 {
        trail::sparks(fx, OT, D + 0x6F0, D + 0x680);
        spray(fx);
        if n(fx) >= 0x19B {
            fx.step_down(D + 0x6FB, 0, 8);
        }
    }
    if n(fx) >= 0x155 {
        fx.ribbon_draw(D + 0x70, D + 0xA0, D);
        if fx.mem.i32(WAIT) == 0 {
            fstep_down(fx, D + 0x7C, 0.0, 3.0);
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(N, m);
    let left = fx.mem.i32(TIMER) - 1;
    fx.mem.set32(TIMER, left);
    let m = n(fx);
    if m == 0x19A {
        2
    } else if m == 0x178 {
        1
    } else if (0x182..0x19A).contains(&m) {
        6
    } else if fx.mem.i32(TIMER) > 0 {
        0
    } else {
        8
    }
}

/// The spray's sparks (D+0x6F0: count +0x2C, sparks from D+0x720) grown
/// 1.4 a step and, above 20, respawned on a flat ring (as
/// `spawn::scatter` with flag 0x1D; radius modulus 10 x +0x28), life
/// -(rand % 12).
fn spray(fx: &mut Fx) {
    let mut k = 0;
    while k < fx.mem.i16(D + 0x71C) {
        let p = D + 0x720 + 16 * k as u32;
        k += 1;
        if fx.mem.i16(p + 0xE) < 0 {
            continue;
        }
        let s = f32::from_bits(0x3FB3_3333);
        for c in 0..3u32 {
            let x = ee::mul(fx.mem.f(p + 4 * c), s);
            fx.mem.setf(p + 4 * c, x);
        }
        if ee_le(fx.mem.f(p + 4), 20.0) {
            continue;
        }
        let f = ee::mul(10.0, fx.mem.f(D + 0x718));
        fx.fix_residue(f, 0x01E0_5D68);
        let w = ee::fixsfsi(f);
        let a = fx.rand_from(D, 0x01E0_5D70) & 0xFFFF;
        let r = fx.rand_from(D, 0x01E0_5D8C);
        let (_, h) = fx.div(r, w);
        fx.mem.write_u32(p + 4, 0x4080_0000);
        let radius = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h));
        let c = fx.tcos(a);
        fx.mem.setf(p + 8, ee::mul(radius, c));
        let s = fx.tsin(a);
        fx.mem.setf(p, ee::mul(radius, s));
        let r = fx.rand_from(D, 0x01E0_5DDC);
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0xE, h.wrapping_neg());
    }
}

/// The blend by the flag word at `f`: bit 0xC 0x44, 0xA 0x42, else 0x48.
fn blend_ca(fx: &mut Fx, d: u32, f: u32) {
    fx.alpha_for(d, f, &[(0xC, ALPHA_44), (0xA, ALPHA_42)]);
}

/// A vertex's XYZ2 at `a`: x, y (u16), z 0xFFFFFF, the top byte 0.
fn xyz_far(fx: &mut Fx, a: u32, x: u32, y: u32) {
    fx.mem.write_u16(a, x as u16);
    fx.mem.write_u16(a + 2, y as u16);
    let v = fx.mem.read_u64(a);
    fx.mem.write_u64(a, (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
    fx.mem.write_u8(a + 7, 0);
}

/// The RGBAQ at `to` from the B, G, R, A bytes at `c`, Q 1.
fn rgbaq(fx: &mut Fx, to: u32, c: u32) {
    for (k, o) in [(0u32, 2u32), (1, 1), (2, 0), (3, 3)] {
        let v = fx.mem.read_u8(c + o);
        fx.mem.write_u8(to + k, v);
    }
    fx.mem.write_u32(to + 4, 0x3F80_0000);
}

/// 0x1E00120(ot, r): an untextured sprite rectangle (descriptor at sp +
/// 0x48: 4 values, prim 0x11, flag 0x400; blend by r bits 0xC / 0xA) from
/// (r+8, r+0xA) to (r+0xC, r+0xE) (+0x700, 12.4) in the colour r+4 at the
/// far Z, key 0; again with bit 0x1C.
fn rect_sprite(fx: &mut Fx, ot: u32, r: u32) {
    let sp = fx.enter(0x80);
    let d = sp + 0x48;
    fx.desc_init(d, 4, 0x11, 0x4141);
    blend_ca(fx, d, r);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x400);
    rgbaq(fx, d + 0x10, r + 4);
    rgbaq(fx, d + 0x20, r + 4);
    let c = |fx: &mut Fx, o: u32| (u32::from(fx.mem.read_u16(r + o)) + 0x700) << 4;
    let (x0, y0) = (c(fx, 8), c(fx, 0xA));
    xyz_far(fx, d + 0x18, x0, y0);
    let (x1, y1) = (c(fx, 0xC), c(fx, 0xE));
    xyz_far(fx, d + 0x28, x1, y1);
    fx.submit(d, ot, 0);
    if bit(fx.mem.read_u32(r), 0x1C) {
        fx.submit(d, ot, 0);
    }
    fx.leave(0x80);
}

/// 0x1E02B80(ot, g): screen-wide scanlines (sprite rectangles x 0x7000 to
/// 0x9000, prim 0x11, flag 0x400; blend by g bits 0xC / 0xA): 0x200 / (g+4
/// + 3) groups of three one-pixel lines from y 0x700, the groups g+4 + 3
/// apart; colour g+8 (B, G, R, A) to g+0xC (copied from g+8 unless bit 9);
/// each line again with bit 0x1C.
fn lines(fx: &mut Fx, ot: u32, g: u32) {
    let sp = fx.enter(0xF0);
    let step = fx.mem.i32(g + 4) + 3;
    let count = fx.div(0x200, step).0;
    fx.mem.set32(sp + 0xA0, count);
    let d = sp + 0xB8;
    fx.desc_init(d, 4, 0x11, 0x4141);
    blend_ca(fx, d, g);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x400);
    if !bit(fx.mem.read_u32(g), 9) {
        for k in 0..4u32 {
            let v = fx.mem.read_u8(g + 8 + k);
            fx.mem.write_u8(g + 0xC + k, v);
        }
    }
    rgbaq(fx, d + 0x10, g + 8);
    rgbaq(fx, d + 0x20, g + 0xC);
    if 0 < fx.mem.i32(sp + 0xA0) {
        let base = fx.mult(0, step);
        let mut s7 = (base as u32) & 0xFFFF;
        let mut s0 = 0;
        loop {
            let mut s2 = (s7 + 0x700) & 0xFFFF;
            for _ in 0..3 {
                let y = s2 << 4;
                xyz_far(fx, d + 0x18, 0x7000, y);
                xyz_far(fx, d + 0x28, 0x9000, y);
                fx.submit(d, ot, 0);
                if bit(fx.mem.read_u32(g), 0x1C) {
                    fx.submit(d, ot, 0);
                }
                s2 = (s2 + 1) & 0xFFFF;
            }
            s7 = (s7 + step as u32) & 0xFFFF;
            s0 += 1;
            if s0 >= fx.mem.i32(sp + 0xA0) {
                break;
            }
        }
    }
    fx.leave(0xF0);
}

/// The projected-vertex flag (0x14 + `i`) of the group flags `f` set when
/// `v` projects through `x` + 0x30 into `xyz` / `w`.
fn proj(fx: &mut Fx, f: u32, i: u32, v: u32, x: u32, xyz: u32, w: u32) {
    let m = fx.read_m4(x + 0x30);
    if fx.project_at(v, &m, xyz, w) {
        fx.flag(f, 0x14 + i);
    }
}

/// The plate draw common to [`fan`] and [`rect_plates`]: REGION_CLAMP from
/// the rectangle +0x10, the texture +0 (filter 1), drawn when all four
/// vertices projected (far Z key 0 with flag 0x13, else the mean w), the
/// vertex flags cleared.
fn plate_submit(fx: &mut Fx, ot: u32, g: u32, d: u32, xyz: [u32; 4], ws: [u32; 4]) {
    fx.region_clamp(d, g + 0x10, g + 0x14, g + 0x12, g + 0x16);
    let rh2 = fx.mem.read_u32(g);
    fx.desc_texture(d, rh2, 1);
    if fx.all_projected(g + 4) {
        if bit(fx.mem.read_u32(g + 4), 0x13) {
            fx.far_z(&xyz);
            fx.submit(d, ot, 0);
        } else {
            let key = fx.mean_key4(ws);
            fx.submit(d, ot, key);
        }
    }
    fx.clear_projected(g + 4);
}

/// 0x1E017B0(ot, g, x): a fan of four textured quads round the record's
/// 9-vector template (0x1E05F80 at sp + 0xA0; the eight outer points set to
/// the corners / edge middles of a square of half size +0xC) turned by the
/// angles +0x20 (0x20BDA0 into sp + 0x1C0, 0x20C540 into sp + 0x130..):
/// quad k = (outer 2k + 1, 2 / 6, 4 / 0, and the rotation matrix's first
/// row at sp + 0x1C0 as the original reads it); descriptor at sp + 0x210
/// (12 values, prim 0x34; blend by flags 0xC / 0xB / 0xA, colour +8, UVs
/// from the rectangle +0x10 in order 0).
fn fan(fx: &mut Fx, ot: u32, g: u32, x: u32) {
    let sp = fx.enter(0x2A0);
    for k in 0..9u32 {
        let v = fx.read_v4(0x01E0_5F80 + 16 * k);
        fx.write_v4(sp + 0xA0 + 16 * k, v);
    }
    let s = |fx: &mut Fx| fx.mem.f(g + 0xC);
    for (o, neg) in [(0xA0u32, true), (0xB0, true), (0xB4, false), (0xC4, false), (0xD0, false), (0xD4, false), (0xE0, false), (0xF0, false), (0xF4, true), (0x104, true), (0x110, true), (0x114, true)] {
        let v = s(fx);
        fx.mem.setf(sp + o, if neg { ee::neg(v) } else { v });
    }
    let rot = [fx.mem.i32(g + 0x20), fx.mem.i32(g + 0x24), fx.mem.i32(g + 0x28)];
    let r = fx.rotation_matrix(rot);
    fx.write_m4(sp + 0x1C0, &r);
    for k in 0..8u32 {
        let v = fx.read_v4(sp + 0xA0 + 16 * k);
        let o = fx.transform(Some(&r), v);
        fx.write_v4(sp + 0x130 + 16 * k, o);
    }
    let d = sp + 0x210;
    fx.desc_init(d, 12, 0x34, REGS_TEX4);
    fx.alpha_for(d, g + 4, &[(0xC, ALPHA_44), (0xB, ALPHA_88), (0xA, ALPHA_42)]);
    fx.colours4(d, g + 8);
    fx.uv_rect(d, g + 0x10, 0);
    let xyz = [sp + 0x230, sp + 0x248, sp + 0x260, sp + 0x278];
    let ws = [sp + 0x200, sp + 0x204, sp + 0x208, sp + 0x20C];
    for k in 0..4u32 {
        let a = sp + 0x130 + 16 * (2 * k + 1);
        proj(fx, g + 4, 0, a, x, xyz[0], ws[0]);
        let b = if k < 2 { sp + 0x150 } else { sp + 0x190 };
        proj(fx, g + 4, 1, b, x, xyz[1], ws[1]);
        let c = if k.wrapping_sub(1) < 2 { sp + 0x170 } else { sp + 0x130 };
        proj(fx, g + 4, 2, c, x, xyz[2], ws[2]);
        proj(fx, g + 4, 3, sp + 0x1C0, x, xyz[3], ws[3]);
        plate_submit(fx, ot, g, d, xyz, ws);
    }
    fx.leave(0x2A0);
}

/// 0x1E038D0(ot, g, x): two textured plates sized by the rectangle +0x10
/// (half sizes +0x24 x (w >> 1), +0x24 x (h >> 1)) at heights +-(+0xC) on
/// the record's template (0x1E060D0, 8 vectors) turned by (the record's
/// 0x1E06150 angles, z = +0x20) (0x20BDA0 into sp + 0x1A0); descriptor at
/// sp + 0x200 (12 values, prim 0x34; blend by flags 0xC / 0xB / 0xA,
/// colour +8); the upper plate's UVs in order 3, the lower's in order 0.
fn rect_plates(fx: &mut Fx, ot: u32, g: u32, x: u32) {
    let sp = fx.enter(0x290);
    let sc = fx.mem.f(g + 0x24);
    let hx = ee::mul(sc, ee::from_i32(fx.mem.i16(g + 0x14) >> 1));
    let hy = ee::mul(sc, ee::from_i32(fx.mem.i16(g + 0x16) >> 1));
    for k in 0..8u32 {
        let v = fx.read_v4(0x01E0_60D0 + 16 * k);
        fx.write_v4(sp + 0xA0 + 16 * k, v);
    }
    let nx = ee::neg(hx);
    let c = |fx: &mut Fx| fx.mem.f(g + 0xC);
    let rows: [(u32, f32, bool, bool); 8] = [(0xA0, nx, false, true), (0xB0, hx, false, true), (0xC0, nx, false, false), (0xD0, hx, false, false), (0xE0, nx, true, true), (0xF0, hx, true, true), (0x100, nx, true, false), (0x110, hx, true, false)];
    for (o, vx, low, up) in rows {
        fx.mem.setf(sp + o, vx);
        let cv = c(fx);
        let cv = if low { ee::neg(cv) } else { cv };
        fx.mem.setf(sp + o + 4, if up { ee::add(cv, hy) } else { ee::sub(cv, hy) });
    }
    let w = fx.read_v4(0x01E0_6150);
    fx.write_v4(sp + 0x1F0, w);
    let a = fx.mem.read_u32(g + 0x20);
    fx.mem.write_u32(sp + 0x1F8, a);
    let rot = [fx.mem.i32(sp + 0x1F0), fx.mem.i32(sp + 0x1F4), fx.mem.i32(sp + 0x1F8)];
    let r = fx.rotation_matrix(rot);
    fx.write_m4(sp + 0x1A0, &r);
    for k in 0..8u32 {
        let v = fx.read_v4(sp + 0xA0 + 16 * k);
        let o = fx.transform(Some(&r), v);
        fx.write_v4(sp + 0x120 + 16 * k, o);
    }
    let d = sp + 0x200;
    fx.desc_init(d, 12, 0x34, REGS_TEX4);
    fx.alpha_for(d, g + 4, &[(0xC, ALPHA_44), (0xB, ALPHA_88), (0xA, ALPHA_42)]);
    fx.colours4(d, g + 8);
    let xyz = [sp + 0x220, sp + 0x238, sp + 0x250, sp + 0x268];
    let ws = [sp + 0x1E0, sp + 0x1E4, sp + 0x1E8, sp + 0x1EC];
    for q in 0..2u32 {
        fx.uv_rect(d, g + 0x10, if q == 0 { 3 } else { 0 });
        for i in 0..4u32 {
            proj(fx, g + 4, i, sp + 0x120 + 0x40 * q + 16 * i, x, xyz[i as usize], ws[i as usize]);
        }
        plate_submit(fx, ot, g, d, xyz, ws);
    }
    fx.leave(0x290);
}
