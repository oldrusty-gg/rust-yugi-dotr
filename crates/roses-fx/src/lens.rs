//! The lens records' shared pieces, moved out of
//! b451.rs (cards 587 / 603 / 586) for b165 (card 173): the far-Z XYZ2
//! rewrite of a sprite drawn on the camera plane, and the frame copy in
//! a packet layer (TEXFLUSH 0x207BB0, the copy destination 0x2095A0, the
//! frame as a texture 0x207F50) [F].
use crate::bus::Bus;
use crate::exe::Fx;

/// XYZ2 low 32 bits and the top byte kept: Z replaced.
pub const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
/// Z 0xFFFFF8 in an XYZ2.
pub const FAR_Z: u64 = 0x00FF_FFF8_0000_0000;
/// The lens' CLAMP (as b483).
pub const LENS_CLAMP: u64 = 0x0000_0800_0080_000A;

/// The far Z into the XYZ2 at `first`, then its Z into the others.
pub fn far_z(fx: &mut Fx, first: u32, others: &[u32]) {
    let v = fx.mem.read_u64(first);
    let v = (v & KEEP) | FAR_Z;
    fx.mem.write_u64(first, v);
    let z = ((fx.mem.read_u64(first) << 8) >> 40) << 32;
    for &o in others {
        let w = fx.mem.read_u64(o);
        fx.mem.write_u64(o, (w & KEEP) | z);
    }
}

/// An XYZ2 at `p`: x / y halfwords, the far Z and a zero top byte.
pub fn far_xy(fx: &mut Fx, p: u32, x: u16, y: u16) {
    fx.mem.write_u16(p, x);
    fx.mem.write_u16(p + 2, y);
    let v = fx.mem.read_u64(p);
    fx.mem.write_u64(p, (v & KEEP) | FAR_Z);
    fx.mem.write_u8(p + 7, 0);
}

/// TEXFLUSH (0x207BB0, `a` = the caller's a0..a3), then the frame copied
/// to the copy destination (0x2095A0, called with a0..a3 as 0x207BB0
/// leaves them: the new packet pointer twice, the old one, 0x8000 [run,
/// b483]) as a texture (0x207F50 with `filter`, TEX0 / TEX1 into `tex0` /
/// `tex1`). Returns a0..a3 as 0x207F50 leaves them.
pub fn frame_copy(fx: &mut Fx, a: [u32; 4], filter: i32, tex0: u32, tex1: u32) -> [u32; 4] {
    let p0 = fx.mem.read_u32(crate::world::PACKET_POINTER);
    fx.texflush(a);
    let p1 = fx.mem.read_u32(crate::world::PACKET_POINTER);
    let dest = fx.copy_dest([p1, p1, p0, 0x8000]);
    fx.frame_texture_packet(dest, filter, tex0, tex1)
}
