//! 3b4ae97aa05c (b648 0x1E00120; 4 B records: b000 b648 + 2): a full-window
//! flash over the frame in a packet layer [F].
use crate::bus::Bus;
use crate::exe::Fx;

/// `frame_flash(c) -> 0`: a packet layer (0x2078A0) holding, between two
/// TEXFLUSHes (0x207BB0) with the frame copied to the texture
/// destination (0x2095A0 / 0x207F50, filter 0), a window sprite (0x6FF8 ..
/// 0x8FF8, far Z; prim 6, 3 regs) of the colour at `c` (B, G, R, A) with
/// ALPHA preset 3 (0x208450), then a textured window sprite (prim 0x26, 5
/// regs, ST 0..1) of grey 0x80 and alpha 0x80 - `c`[3] with ALPHA preset
/// 0, both submitted with no ordering table; the layer closed into the OT
/// (0x206E40) at key 0xFFFFFF (clamped). A 0xC0 frame. `a123`: the
/// caller's a1..a3 at the call (the first TEXFLUSH is reported with them
/// behind a0 = `c`).
pub fn frame_flash(fx: &mut Fx, c: u32, a123: [u32; 3]) -> i32 {
    let sp = fx.enter(0xC0);
    let layer = fx.layer_begin();
    let da = sp + 0x68;
    fx.mem.write_u16(da + 2, 0x26);
    fx.mem.write_u16(da, 5);
    fx.mem.write_u64(da + 8, 0x4_2421);
    fx.mem.write_u32(da + 4, 0);
    let p0 = fx.mem.read_u32(crate::world::PACKET_POINTER);
    fx.texflush([c, a123[0], a123[1], a123[2]]);
    // a0..a3 as 0x207BB0 leaves them: the new packet pointer twice, the
    // old one, 0x8000 [run b483].
    let p1 = fx.mem.read_u32(crate::world::PACKET_POINTER);
    let dest = fx.copy_dest([p1, p1, p0, 0x8000]);
    let regs = fx.frame_texture_packet(dest, 0, sp + 0xA8, sp + 0xB0);
    fx.texflush(regs);
    let db = sp + 0x38;
    fx.mem.write_u16(db + 2, 6);
    fx.mem.write_u16(db, 3);
    fx.mem.write_u64(db + 8, 0x441);
    fx.mem.write_u32(db + 4, 0);
    for (o, src) in [(0x48u32, 2u32), (0x49, 1), (0x4A, 0), (0x4B, 3)] {
        let v = fx.mem.read_u8(c + src);
        fx.mem.write_u8(sp + o, v);
    }
    fx.mem.write_u32(sp + 0x4C, 0x3F80_0000);
    for (at, xy) in [(0x50u32, 0x6FF8u16), (0x58, 0x8FF8)] {
        fx.mem.write_u16(sp + at, xy);
        fx.mem.write_u16(sp + at + 2, xy);
        fx.set_xyz_z(sp + at, 0xFF_FFFF);
        fx.mem.write_u8(sp + at + 7, 0);
    }
    let alpha = fx.alpha_preset(3);
    fx.desc_alpha(db, alpha);
    fx.submit(db, 0, 0);
    for o in [0x78u32, 0x79, 0x7A] {
        fx.mem.write_u8(sp + o, 0x80);
    }
    let a = 0x80u8.wrapping_sub(fx.mem.read_u8(c + 3));
    fx.mem.write_u8(sp + 0x7B, a);
    fx.mem.write_u32(sp + 0x7C, 0x3F80_0000);
    for (at, xy) in [(0x88u32, 0x6FF8u16), (0x98, 0x8FF8)] {
        fx.mem.write_u16(sp + at, xy);
        fx.mem.write_u16(sp + at + 2, xy);
        fx.set_xyz_z(sp + at, 0xFF_FFFF);
        fx.mem.write_u8(sp + at + 7, 0);
    }
    fx.mem.write_u32(sp + 0x80, 0);
    fx.mem.write_u32(sp + 0x84, 0);
    fx.mem.write_u32(sp + 0x90, 0x3F80_0000);
    fx.mem.write_u32(sp + 0x94, 0x3F80_0000);
    let alpha = fx.alpha_preset(0);
    fx.desc_alpha(da, alpha);
    fx.submit(da, 0, 0);
    let ot = crate::exe::OT;
    fx.layer_end(layer, ot, 0xFF_FFFF);
    fx.leave(0xC0);
    0
}
