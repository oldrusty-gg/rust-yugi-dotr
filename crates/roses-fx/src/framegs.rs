//! Frame-buffer textures (helper C, 30 Oct 2026): the executable's
//! 0x207EF0 / 0x207FC0 (a descriptor's TEX0 / TEX1 pointed at the frame
//! being drawn) and the effect library's full-window frame quad
//! (b4807149dc95, 9 B records), hand-ported.
//!
//! The interpreter runner lets 0x207EF0 and 0x207FC0 run and reports both
//! calls (address, a0..a3) as boundary calls; the ports report the same.
use crate::bus::Bus;
use crate::exe::{Fx, OT};
use crate::fxlib::ALPHA_44;
use crate::types::{FrameCopy, TextureKey, UNIT_RT_TBP};

/// The display state 0x209640 reads (0x2E93D0: +0x26C the frame parity,
/// +0x60 / +0x150 the two FRAME registers, +0x250 / +0x258).
const DISPLAY: u32 = 0x002E_93D0;
/// The screen width / height 0x2094B0 reads.
const SCREEN_W: u32 = 0x002E_960C;
const SCREEN_H: u32 = 0x002E_9610;

impl Fx {
    /// 0x209640(other, tbp, psm, out3) [F]: the frame buffer's TBP (FBP x
    /// 32 from the FRAME of the parity, or the other one when `other`),
    /// +0x250 and +0x258 into the non-null outputs.
    fn display_frame(&mut self, other: bool, tbp: u32, psm: u32, out3: u32) {
        if tbp != 0 {
            let parity = self.mem.read_u32(DISPLAY + 0x26C) & 1;
            let at = if parity == u32::from(other) { DISPLAY + 0x150 } else { DISPLAY + 0x60 };
            let v = (u32::from(self.mem.read_u16(at)) & 0x1FF) << 5;
            self.mem.write_u32(tbp, v);
        }
        if psm != 0 {
            let v = self.mem.read_u32(DISPLAY + 0x250);
            self.mem.write_u32(psm, v);
        }
        if out3 != 0 {
            let v = self.mem.read_u32(DISPLAY + 0x258);
            self.mem.write_u32(out3, v);
        }
    }

    /// 0x207FC0(packet, dest, filter, tex0, tex1) [F]: TEX0 at `tex0` =
    /// the frame (TBP from `dest` when > 0, else the frame drawn; TBW =
    /// width / 64; PSM; TW / TH = log2 of the screen size rounded up; TCC,
    /// TFX, CBP, CPSM, CSM, CSA 0; CLD 1); TEX1 at `tex1`: LCM, MXL, MTBA,
    /// L, K 0, MMAG = filter & 1, MMIN = filter & 7. Returns `packet`.
    /// A positive `dest` (the GS local copy packet) is not ported: no ported
    /// record asks for it.
    pub fn frame_texture(&mut self, packet: u32, dest: i32, filter: i32, tex0: u32, tex1: u32) -> u32 {
        self.boundary(0x0020_7FC0, "packet writer 0x207FC0", [packet, dest as u32, filter as u32, tex0]);
        let sp = self.enter(0x80);
        self.display_frame(dest != 0, sp + 0x70, sp + 0x74, 0);
        let w = self.mem.read_u32(SCREEN_W) as i32;
        let h = self.mem.read_u32(SCREEN_H) as i32;
        self.mem.write_u32(sp + 0x78, w as u32);
        self.mem.write_u32(sp + 0x7C, h as u32);
        let mut tw = 0i32;
        while (1i32.wrapping_shl(tw as u32)) < w {
            tw += 1;
        }
        let mut th = 0i32;
        while (1i32.wrapping_shl(th as u32)) < h {
            th += 1;
        }
        if tex0 != 0 {
            let tbp = if dest > 0 { dest as u32 } else { self.mem.read_u32(sp + 0x70) };
            let psm = u64::from(self.mem.read_u32(sp + 0x74) & 0x3F);
            let mut t = self.mem.read_u64(tex0);
            t = (t & !0x3FFF) | u64::from(tbp & 0x3FFF);
            t = (t & !(0x3F << 14)) | ((((w >> 6) as u64) & 0x3F) << 14);
            t = (t & !(0x3F << 20)) | (psm << 20);
            t = (t & !(0xF << 26)) | (((tw & 0xF) as u64) << 26);
            t = (t & !(0xF << 30)) | (((th & 0xF) as u64) << 30);
            // TCC, TFX, CBP, CPSM, CSM, CSA 0; CLD 1.
            t &= !(0x3FFF_FFFF << 34);
            t |= 1 << 61;
            self.mem.write_u64(tex0, t);
        }
        if tex1 != 0 {
            let mut t = self.mem.read_u64(tex1);
            t &= !1;
            t &= !(7 << 2);
            t = (t & !(1 << 5)) | (((filter & 1) as u64) << 5);
            t = (t & !(7 << 6)) | (((filter & 7) as u64) << 6);
            t &= !(1 << 9);
            t &= !(3 << 19);
            t &= !(0xFFF << 32);
            self.mem.write_u64(tex1, t);
        }
        let mut packet = packet;
        if dest > 0 {
            packet = self.frame_copy_packet(packet, dest, sp);
        }
        self.leave(0x80);
        packet
    }

    /// 0x207FC0's copy packet for `dest` > 0 [F 0x2082E8]: a
    /// GS local-to-local copy of the frame (BITBLTBUF from the frame's TBP,
    /// TBW = width / 64 and PSM to `dest`, TRXPOS 0, TRXREG width x height,
    /// TRXDIR 2) as a DMA cnt / VIF FLUSH / DIRECT 5 packet at `p`; the
    /// pointer after it. Recorded as the interpreter runner records it
    /// (a [`FrameCopy`] before the next primitive, in the open layer;
    /// TBP 0x2000 = the offscreen target then holds the frame).
    fn frame_copy_packet(&mut self, p: u32, dest: i32, sp: u32) -> u32 {
        let tbp = (dest & 0x3FFF) as u16;
        self.out.frame_copies.push(FrameCopy { tbp, before: self.out.prims.len(), layer: self.layers_open.last().copied() });
        // [H] other destinations would need the runner's `copies` list
        // (primitives sampling them become the frame); none is ported.
        debug_assert!(tbp == UNIT_RT_TBP, "frame copy to TBP {tbp:#x} is not ported");
        if tbp == UNIT_RT_TBP {
            self.rt = Some(TextureKey::Frame);
        }
        let w = i64::from(self.mem.read_u32(sp + 0x78) as i32);
        let fbp = i64::from(self.mem.read_u32(sp + 0x70) as i32);
        let psm = i64::from(self.mem.read_u32(sp + 0x74) as i32);
        let h = i64::from(self.mem.read_u32(sp + 0x7C) as i32);
        self.mem.write_u64(p + 8, 0);
        self.mem.write_u64(p, 0);
        self.mem.write_u8(p + 3, 0x10);
        self.mem.write_u64(p + 0x10, 0x1000_0000_0000_8004);
        self.mem.write_u64(p + 0x18, 0xE);
        let tbw = w >> 6;
        let bitblt = fbp | (tbw << 16) | (psm << 24) | (i64::from(dest) << 32) | (tbw << 48) | (psm << 56);
        self.mem.write_u64(p + 0x20, bitblt as u64);
        self.mem.write_u64(p + 0x28, 0x50);
        self.mem.write_u64(p + 0x30, 0);
        self.mem.write_u64(p + 0x38, 0x51);
        self.mem.write_u64(p + 0x40, (w | (h << 32)) as u64);
        self.mem.write_u64(p + 0x48, 0x52);
        self.mem.write_u64(p + 0x50, 2);
        self.mem.write_u64(p + 0x58, 0x53);
        let q = p + 0x60;
        let n = ((((q.wrapping_sub(p)) as i32) >> 3) >> 1) - 1;
        self.mem.write_u16(p, n as u16);
        self.mem.write_u32(p + 8, 0x1100_0000);
        self.mem.write_u32(p + 0xC, (n as u32 & 0xFFFF) | 0x5100_0000);
        q
    }

    /// 0x2095A0() [F]: the frame-copy destination 0x2E9638 (+ 0x2E93B0 <<
    /// 5 with the display flags 0x2E93C0 bit 0). `a` = the caller's a0..a3
    /// (reported, as the runner reports the call).
    pub fn copy_dest(&mut self, a: [u32; 4]) -> i32 {
        self.boundary(0x0020_95A0, "packet writer 0x2095A0", a);
        let v = if self.mem.read_u8(0x002E_93C0) & 1 != 0 { self.mem.read_u32(0x002E_93B0) } else { 0 };
        self.mem.read_u32(DISPLAY + 0x268).wrapping_add(v << 5) as i32
    }

    /// 0x207F50(dest, filter, tex0, tex1) [F]: [`Fx::frame_texture`] on the
    /// frame's packet (the pointer 0x2E9650 read by 0x209470, stored back by
    /// 0x209480). Returns a0..a3 as the call leaves them (for a later
    /// boundary call): the new pointer, then 0x207FC0's last a1..a3 (the
    /// copy path: width (TRXREG's low word), width, BITBLTBUF's low word
    /// below PSM; [H] without a copy not modelled: the same).
    pub fn frame_texture_packet(&mut self, dest: i32, filter: i32, tex0: u32, tex1: u32) -> [u32; 4] {
        self.boundary(0x0020_7F50, "packet writer 0x207F50", [dest as u32, filter as u32, tex0, tex1]);
        self.enter(0x50);
        let p = self.mem.read_u32(crate::world::PACKET_POINTER);
        let q = self.frame_texture(p, dest, filter, tex0, tex1);
        self.mem.write_u32(crate::world::PACKET_POINTER, q);
        self.leave(0x50);
        let w = self.mem.read_u32(0x002E_960C);
        let fbp_at = if self.mem.read_u32(DISPLAY + 0x26C) & 1 == u32::from(dest != 0) { DISPLAY + 0x150 } else { DISPLAY + 0x60 };
        let fbp = (u32::from(self.mem.read_u16(fbp_at)) & 0x1FF) << 5;
        [q, w, w, fbp | ((w >> 6) << 16)]
    }

    /// 0x207EF0(desc, filter) [F]: a textured descriptor (flag 0x20) gets
    /// the frame as its texture (TEX0 / TEX1 after the ALPHA slot, 0x207FC0
    /// with no packet); returns 1, else 0. `a23`: the caller's a2 / a3 at
    /// the call (reported).
    pub fn frame_desc(&mut self, d: u32, filter: i32, a23: [u32; 2]) -> i32 {
        self.boundary(0x0020_7EF0, "packet writer 0x207EF0", [d, filter as u32, a23[0], a23[1]]);
        self.enter(0x10);
        let flags = self.mem.read_u16(d + 2);
        let n = u32::from(self.mem.read_u16(d));
        let r = if flags & 0x20 == 0 {
            0
        } else {
            let base = d + 0x10 + 8 * n;
            self.frame_texture(0, 0, filter, base + 8, base + 0x10);
            1
        };
        self.leave(0x10);
        r
    }

    /// 0x2079D0(ot, key, filter, alpha, colour) [F]: a
    /// full-window sprite (0x6FF8, 0x6FF8) .. (0x8FF8, 0x8FF8) at Z 0xFFFFF7,
    /// ST 0..1, colour (B, G, R, A) at `colour` ([H] the default colour of a
    /// null pointer, 0x2931E0's float, is not ported), ALPHA `alpha`
    /// (0x207E20), textured with the frame drawn (0x207EF0, `filter`),
    /// written into the frame's packet (0x206F90 at 0x209470's pointer,
    /// stored back by 0x209480: unchanged) and linked into `ot` at `key`
    /// (0x207A50). Reported as the runner reports it: the call, then
    /// 0x207EF0 inside. Its 0x60 frame (s4..s0 = the arguments, `ra`) and
    /// 0x207A50's 0xB0 frame (s3..s0 = packet, ot, key, filter) are left.
    pub fn frame_pass(&mut self, ot: u32, key: i32, filter: i32, alpha: u64, colour: u32, ra: u32) {
        self.boundary(0x0020_79D0, "previous-frame copy 0x2079D0", [ot, key as u32, filter as u32, alpha as u32]);
        let sp = self.enter(0x60);
        self.saved(sp, &[(0x50, u64::from(ra)), (0x40, u64::from(ot)), (0x30, i64::from(key) as u64), (0x20, i64::from(filter) as u64), (0x10, alpha), (0, u64::from(colour))]);
        let packet = self.mem.read_u32(crate::world::PACKET_POINTER);
        let sp = self.enter(0xB0);
        self.saved(sp, &[(0x40, 0x0020_7A20), (0x30, u64::from(packet)), (0x20, u64::from(ot)), (0x10, i64::from(key) as u64), (0, i64::from(filter) as u64)]);
        let d = sp + 0x50;
        self.mem.write_u16(d + 2, 0x26);
        self.mem.write_u16(d, 5);
        self.mem.write_u64(d + 8, 0x4_2421);
        self.mem.write_u32(d + 4, 0);
        debug_assert!(colour != 0, "0x2079D0's default colour is not ported");
        for (k, src) in [(0u32, 2u32), (1, 1), (2, 0), (3, 3)] {
            let v = self.mem.read_u8(colour + src);
            self.mem.write_u8(sp + 0x60 + k, v);
        }
        self.mem.write_u32(sp + 0x64, 0x3F80_0000);
        self.mem.write_u64(sp + 0x70, 0x6FF8 | (0x6FF8 << 16) | (0xFF_FFF7 << 32));
        self.mem.write_u64(sp + 0x80, 0x8FF8 | (0x8FF8 << 16) | (0xFF_FFF7 << 32));
        self.mem.write_u32(sp + 0x68, 0);
        self.mem.write_u32(sp + 0x6C, 0);
        self.mem.write_u32(sp + 0x78, 0x3F80_0000);
        self.mem.write_u32(sp + 0x7C, 0x3F80_0000);
        self.desc_alpha(d, alpha);
        // After 0x207E20: a2 = the flags | 0x200, a3 = the flags.
        self.frame_desc(d, filter, [0x226, 0x26]);
        self.submit_at(packet, d, ot, key);
        self.leave(0xB0);
        self.leave(0x60);
    }

    /// The effect library's full-window frame quad (b610 0x1E00120,
    /// b4807149dc95: 9 B records) [F]: a sprite (0x6FF8, 0x6FF8) ..
    /// (0x8FF8, 0x8FF8) at the far Z, ST 0..1 over the frame drawn (bilinear),
    /// ALPHA 0x44, the colour at `c` (B, G, R, A).
    pub fn frame_quad(&mut self, c: u32) {
        let rgba = [self.mem.read_u8(c + 2), self.mem.read_u8(c + 1), self.mem.read_u8(c), self.mem.read_u8(c + 3)];
        self.frame_quad_rgba(rgba);
    }

    /// The same quad of a constant colour (R, G, B, A) (b753 0x1E00120,
    /// b26f4b13c68d) [F].
    pub fn frame_quad_rgba(&mut self, rgba: [u8; 4]) {
        let sp = self.enter(0x70);
        let d = sp + 0x18;
        self.mem.write_u16(d + 2, 0x26);
        self.mem.write_u16(d, 5);
        self.mem.write_u64(d + 8, 0x4_2421);
        self.mem.write_u32(d + 4, 0);
        for k in 0..4u32 {
            self.mem.write_u8(sp + 0x28 + k, rgba[k as usize]);
        }
        self.mem.write_f32(sp + 0x2C, 1.0);
        for (at, xy) in [(0x38u32, 0x6FF8u16), (0x48, 0x8FF8)] {
            self.mem.write_u16(sp + at, xy);
            self.mem.write_u16(sp + at + 2, xy);
            let v = self.mem.read_u64(sp + at);
            self.mem.write_u64(sp + at, (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
            self.mem.write_u8(sp + at + 7, 0);
        }
        self.mem.write_u32(sp + 0x30, 0);
        self.mem.write_u32(sp + 0x34, 0);
        self.mem.write_f32(sp + 0x40, 1.0);
        self.mem.write_f32(sp + 0x44, 1.0);
        self.desc_alpha(d, ALPHA_44);
        // After 0x207E20: a2 = the flags | 0x200, a3 = the flags.
        self.frame_desc(d, 1, [0x226, 0x26]);
        self.submit(d, OT, 0);
        self.leave(0x70);
    }
}
