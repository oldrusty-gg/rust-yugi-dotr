//! Texture-coordinate setters of the effect library.
use crate::bus::Bus;
use crate::exe::Fx;

/// Which corner of the rectangle each UV value takes: U from x (`X0`) or
/// x + w (`X1`), V from y (`Y0`) or y + h (`Y1`).
#[derive(Clone, Copy)]
enum C {
    X0,
    X1,
    Y0,
    Y1,
}
use C::*;

/// The eight orientations of f4170fdedb0b, in the original's store order
/// (offset of the u16 in the descriptor, corner) [F b003 0x1E00080..
/// 0x1E00918].
const ORIENTATIONS: [[(u32, C); 8]; 8] = [
    [(0x18, X0), (0x1A, Y0), (0x28, X1), (0x2A, Y0), (0x38, X0), (0x3A, Y1), (0x48, X1), (0x4A, Y1)],
    [(0x48, X0), (0x4A, Y0), (0x38, X1), (0x3A, Y0), (0x28, X0), (0x2A, Y1), (0x18, X1), (0x1A, Y1)],
    [(0x28, X0), (0x2A, Y0), (0x18, X1), (0x1A, Y0), (0x48, X0), (0x4A, Y1), (0x38, X1), (0x3A, Y1)],
    [(0x38, X0), (0x3A, Y0), (0x48, X1), (0x4A, Y0), (0x18, X0), (0x1A, Y1), (0x28, X1), (0x2A, Y1)],
    [(0x28, X0), (0x2A, Y0), (0x48, X1), (0x4A, Y0), (0x18, X0), (0x1A, Y1), (0x38, X1), (0x3A, Y1)],
    [(0x38, X0), (0x3A, Y0), (0x18, X1), (0x1A, Y0), (0x48, X0), (0x4A, Y1), (0x28, X1), (0x2A, Y1)],
    [(0x48, X0), (0x4A, Y0), (0x28, X1), (0x2A, Y0), (0x38, X0), (0x3A, Y1), (0x18, X1), (0x1A, Y1)],
    [(0x18, X0), (0x1A, Y0), (0x38, X1), (0x3A, Y0), (0x28, X0), (0x2A, Y1), (0x48, X1), (0x4A, Y1)],
];

/// f4170fdedb0b [183 B records; b003 0x1E00080, 552 words, a leaf]:
/// `uv_rect(desc, rect, mode)` puts the i16 rectangle at `rect` (x +0, y
/// +2, w +4, h +6) into the 14-bit UV fields (12.4, top bits kept) of the
/// four vertices of a textured strip (the u16 pairs at desc +0x18, +0x28,
/// +0x38, +0x48), corners picked by `mode` 0..7 (the identity, mirrors
/// and rotations); any other mode writes nothing.
pub fn uv_rect(fx: &mut Fx, desc: u32, rect: u32, mode: i32) {
    let Some(order) = usize::try_from(mode).ok().and_then(|m| ORIENTATIONS.get(m)) else { return };
    for &(off, c) in order {
        let v = match c {
            X0 => fx.mem.i16(rect),
            Y0 => fx.mem.i16(rect + 2),
            X1 => fx.mem.i16(rect).wrapping_add(fx.mem.i16(rect + 4)),
            Y1 => fx.mem.i16(rect + 2).wrapping_add(fx.mem.i16(rect + 6)),
        };
        fx.set_uv(desc + off, v);
    }
}

/// The four orientations of 7885b5d8bd58 (mode 0, 1, 2, any other), in
/// the original's store order [F b001 0x1E00100..0x1E0053C].
const ORIENTATIONS4: [[(u32, C); 8]; 4] = [
    [(0x18, X0), (0x1A, Y0), (0x28, X1), (0x2A, Y0), (0x38, X0), (0x3A, Y1), (0x48, X1), (0x4A, Y1)],
    [(0x18, X0), (0x1A, Y1), (0x28, X0), (0x2A, Y0), (0x38, X1), (0x3A, Y1), (0x48, X1), (0x4A, Y0)],
    [(0x38, X0), (0x3A, Y0), (0x18, X1), (0x1A, Y0), (0x48, X0), (0x4A, Y1), (0x28, X1), (0x2A, Y1)],
    [(0x28, X0), (0x2A, Y0), (0x18, X1), (0x1A, Y0), (0x48, X0), (0x4A, Y1), (0x38, X1), (0x3A, Y1)],
];

/// 2147599ccb75 [11 B records; b733 0x1E00F80, 84 words, a leaf] (helper
/// C20): `grid_rect(out, id, frame)`, a record's own atlas rectangle (as
/// 0x106960): cell `frame` of a 2-wide grid of 31 x 31 cells, x = 32 (f %
/// 2) + 1 (32 f + 1 below 2), y = 1 / 0x21 / 0x41 for f below 2 / 4 / 8;
/// from 8 only w / h (0x1F) are set. Written into the leaf's stack local
/// and its 16 bytes copied to `out` (bytes 8..15 what the stack held);
/// `id` (a1) and a3 unread [F].
pub fn grid_rect(fx: &mut Fx, out: u32, frame: i32) {
    let sp = fx.enter(0x10);
    let f = frame as i16 as i32;
    let col = ((f & 1) << 5) + 1;
    let xy = if f < 2 {
        Some(((frame << 5) + 1, 1))
    } else if f < 4 {
        Some((col, 0x21))
    } else if f < 8 {
        Some((col, 0x41))
    } else {
        None
    };
    if let Some((x, y)) = xy {
        fx.mem.write_u16(sp, x as u16);
        fx.mem.write_u16(sp + 2, y as u16);
    }
    fx.mem.write_u16(sp + 4, 0x1F);
    fx.mem.write_u16(sp + 6, 0x1F);
    for k in 0..4 {
        let w = fx.mem.read_u32(sp + 4 * k);
        fx.mem.write_u32(out + 4 * k, w);
    }
    fx.leave(0x10);
}

/// 7885b5d8bd58 [45 B records; b001 0x1E00100, 276 words, a leaf]: as
/// [`uv_rect`] with four orientations (0 the identity, 1 turned a
/// quarter, 2 mirrored, any other mode mirrored the other way).
pub fn uv_rect4(fx: &mut Fx, desc: u32, rect: u32, mode: i32) {
    let order = &ORIENTATIONS4[match mode {
        0 => 0,
        1 => 1,
        2 => 2,
        _ => 3,
    }];
    for &(off, c) in order {
        let v = match c {
            X0 => fx.mem.i16(rect),
            Y0 => fx.mem.i16(rect + 2),
            X1 => fx.mem.i16(rect).wrapping_add(fx.mem.i16(rect + 4)),
            Y1 => fx.mem.i16(rect + 2).wrapping_add(fx.mem.i16(rect + 6)),
        };
        fx.set_uv(desc + off, v);
    }
}
