//! The full-screen fade of the effect library: an untextured screen
//! quad (window 0x7000..0x9000, the far Z) whose alpha follows a counter.
use super::exe::countdown;
use crate::bus::Bus;
use crate::ee;
use crate::exe::Fx;

/// The window's corners in 12.4 GS units (0x7000 / 0x9000) and its centre
/// line (32768.0 = 0x8000).
const LEFT: i32 = 0x7000;
const RIGHT: i32 = 0x9000;
const MID: f32 = 32768.0;

/// 3d0567fb3df4 [56 B records; b098 0x1E01B50] and 58d5c5a9444d [35;
/// b002 0x1E003A0] (one routine scheduled two ways) [F]:
/// `fade(s, ot) -> 0 / 1`. A counter record s: s[0] count, s[1] delay
/// (0x101AF0: not due, 0), s[2] length, s[4] flags. The quad's alpha:
/// shape s[4] & 7 = 1 the count (its 128 / length ramp is computed and
/// dropped), 2 (length - count) x 128 / length, else 0x80; halved with
/// flag 8. Flag 0x80 restarts the count at 1 (s[4] becomes 1, or 2 with
/// flag 1). Flag 0x10: a wipe, two half quads opening from the centre
/// line (the one above with alpha from the count's half, the lower one
/// 0x80 / ...); else one quad with the alpha, flags 0x20 / 0x40 clearing
/// its lower / upper vertices' alpha. ALPHA 0x48, the descriptor a stack
/// local (sp + 0x98 of a 0xF0 frame), submitted at key 0 of `ot`; 1.
pub fn fade(fx: &mut Fx, s: u32, ot: u32) -> i32 {
    let sp = fx.enter(0xF0);
    let r = fade_body(fx, sp, s, ot, false, false);
    fx.leave(0xF0);
    r
}

/// 91c7742f873d [4 B records; b706 (card 75) 0x1E00870, 220 words]
/// [F]: [`fade`] without the wipe (flag 0x10 ignored) and
/// with a black quad (RGB 0, ALPHA 0x44), in a 0xE0 frame: the descriptor and
/// vertices 0x10 lower (sp + 0x88); the same shapes, flags 8 / 0x20 /
/// 0x40 / 0x80 and window.
pub fn fade_black(fx: &mut Fx, s: u32, ot: u32) -> i32 {
    let sp = fx.enter(0xE0);
    let r = fade_body(fx, sp.wrapping_sub(0x10), s, ot, false, true);
    fx.leave(0xE0);
    r
}

/// b7efc9adbb7c [1 B record; b532 0x1E025E0, 324 words]
/// [F]: [`fade`] with the quad's colour from the record (R +0xA, G +9, B
/// +8, not 0xFF) and without flags 0x20 / 0x40 (one quad: all four
/// alphas the shape's); the same 0xF0 frame and descriptor.
pub fn fade_rgb(fx: &mut Fx, s: u32, ot: u32) -> i32 {
    let sp = fx.enter(0xF0);
    let r = fade_body(fx, sp, s, ot, true, false);
    fx.leave(0xF0);
    r
}

/// 57ff8060a0a1 [11 B records; b116 0x1E04510, 348 words]
/// [F word-compare]: [`fade`] compiled without scheduling in a 0xC0 frame
/// (f20 / f21 saved at sp + 0 / 4): the descriptor and vertices 0x30
/// lower (sp + 0x68), the same program, flags and wipe.
pub fn fade_c(fx: &mut Fx, s: u32, ot: u32) -> i32 {
    let sp = fx.enter(0xC0);
    let r = fade_body(fx, sp.wrapping_sub(0x30), s, ot, false, false);
    fx.leave(0xC0);
    r
}

/// ec32fd6b7a72 [1 B record; card 12's 0x1E03F10, 336 words] (helper
/// A19, 1 Nov 2026) [F word-compare]: [`fade_rgb`] compiled as
/// [`fade_c`] (a 0xC0 frame, the descriptor and vertices 0x30 lower).
pub fn fade_rgb_c(fx: &mut Fx, s: u32, ot: u32) -> i32 {
    let sp = fx.enter(0xC0);
    let r = fade_body(fx, sp.wrapping_sub(0x30), s, ot, true, false);
    fx.leave(0xC0);
    r
}

fn fade_body(fx: &mut Fx, sp: u32, s: u32, ot: u32, rgb: bool, black: bool) -> i32 {
    if countdown(fx, s) == 0 {
        return 0;
    }
    let d = sp + 0x98;
    fx.desc_init(d, 8, 0x14, 0x4141_4141);
    let flags = fx.mem.u8(s + 4);
    let mut alpha = match flags & 7 {
        1 => {
            let ramp = ee::div(128.0, ee::from_i32(fx.mem.u8(s + 2)));
            let v = ee::mul(ramp, ee::from_i32(fx.mem.u8(s)));
            ee::fixunssfsi(v) as i32 & 0xFF
        }
        2 => {
            let (len, n) = (fx.mem.u8(s + 2), fx.mem.u8(s));
            fx.div((len - n) << 7, len).0 & 0xFF
        }
        _ => 0x80,
    };
    if fx.mem.u8(s) == 1 && flags & 0x80 != 0 {
        let len = fx.mem.read_u8(s + 2);
        fx.mem.write_u8(s, len);
        let v = if fx.mem.u8(s + 4) & 1 != 0 { 2 } else { 1 };
        fx.mem.write_u8(s + 4, v);
    }
    if fx.mem.u8(s + 4) & 8 != 0 {
        alpha = (alpha >> 1) & 0xFF;
    }
    // RGB 0xFF (fade_rgb: the record's +0xA / +9 / +8), Q 1 (B, G, R in
    // fade's store order, R, G, B in fade_rgb's).
    for byte in [2u32, 1, 0] {
        let c = if rgb { fx.mem.read_u8(s + 0xA - byte) } else if black { 0 } else { 0xFF };
        for v in [0xD8u32, 0xC8, 0xB8, 0xA8] {
            fx.mem.write_u8(sp + v + byte, c);
        }
    }
    for v in [0xDCu32, 0xCC, 0xBC, 0xAC] {
        fx.mem.write_u32(sp + v, 0x3F80_0000);
    }
    // The far Z down the chain (x, y and the top byte kept).
    let mut z = 0xFF_FFFF;
    for v in [0xE0u32, 0xD0, 0xC0, 0xB0] {
        fx.set_xyz_z(sp + v, z);
        z = fx.xyz_z(sp + v);
    }
    fx.desc_alpha(d, if black { 0x44 } else { 0x48 });
    let a = |o: u32| sp + o;
    if !black && fx.mem.u8(s + 4) & 0x10 != 0 {
        let len = fx.mem.u8(s + 2);
        let half = len >> 1;
        let step = ee::div(8192.0, ee::from_i32(half));
        let left = len - fx.mem.u8(s);
        let q = fx.div(0x80, half).0;
        let top = fx.mult(q, left >> 1) & 0xFF;
        let open = if half < left {
            fx.mem.set8(a(0xBB), top);
            fx.mem.set8(a(0xAB), top);
            fx.mem.u8(s + 2) >> 1
        } else {
            fx.mem.set8(a(0xBB), 0);
            fx.mem.set8(a(0xAB), 0);
            left
        };
        fx.mem.set8(a(0xDB), 0x80);
        fx.mem.set8(a(0xCB), 0x80);
        fx.mem.set16(a(0xD0), LEFT);
        fx.mem.set16(a(0xB0), LEFT);
        fx.mem.set16(a(0xE0), RIGHT);
        fx.mem.set16(a(0xC0), RIGHT);
        let off = ee::mul(step, ee::from_i32(open));
        let y = ee::fixunssfsi(ee::sub(MID, off)) as i32;
        fx.mem.set16(a(0xC2), y);
        fx.mem.set16(a(0xB2), y);
        let y = ee::fixunssfsi(MID) as i32;
        fx.mem.set16(a(0xE2), y);
        fx.mem.set16(a(0xD2), y);
        fx.submit(d, ot, 0);
        let v = fx.mem.u8(a(0xBB));
        fx.mem.set8(a(0xDB), v);
        fx.mem.set8(a(0xCB), v);
        fx.mem.set8(a(0xAB), 0x80);
        fx.mem.set8(a(0xBB), 0x80);
        let y = ee::fixunssfsi(MID) as i32;
        fx.mem.set16(a(0xC2), y);
        fx.mem.set16(a(0xB2), y);
        let y = ee::fixunssfsi(ee::add(MID, off)) as i32;
        fx.mem.set16(a(0xE2), y);
        fx.mem.set16(a(0xD2), y);
        fx.submit(d, ot, 0);
        return 1;
    }
    for o in [0xDBu32, 0xCB, 0xBB, 0xAB] {
        fx.mem.set8(a(o), alpha);
    }
    if !rgb && fx.mem.u8(s + 4) & 0x20 != 0 {
        fx.mem.set8(a(0xDB), 0);
        fx.mem.set8(a(0xCB), 0);
    }
    if !rgb && fx.mem.u8(s + 4) & 0x40 != 0 {
        fx.mem.set8(a(0xBB), 0);
        fx.mem.set8(a(0xAB), 0);
    }
    fx.mem.set16(a(0xD0), LEFT);
    fx.mem.set16(a(0xB0), LEFT);
    fx.mem.set16(a(0xE0), RIGHT);
    fx.mem.set16(a(0xC0), RIGHT);
    fx.mem.set16(a(0xC2), LEFT);
    fx.mem.set16(a(0xB2), LEFT);
    fx.mem.set16(a(0xE2), RIGHT);
    fx.mem.set16(a(0xD2), RIGHT);
    fx.submit(d, ot, 0);
    1
}
