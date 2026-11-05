//! Unit helpers of the executable for the BMODULE ports [F].
use crate::bus::Bus;
use crate::exe::Fx;
use crate::world;

impl Fx {
    /// 0x1F7790(handle, c) = 0x1F77A0(handle, -1, c) [F]: for a valid
    /// handle (side 0..2, slot 0..10) with a unit object (0x1E25E0), the
    /// object's added colour obj+0x130..0x133 = the 4 bytes at `c` (0:
    /// cleared) (0x1F3A70(index, -1, c); mode -1 leaves obj+0x1F8). 1 when
    /// written. The 0x30 frame it leaves: [`Fx::unit_add_colour_from`].
    pub fn unit_add_colour(&mut self, handle: i32, c: u32) -> i32 {
        let (side, slot) = (handle >> 8, handle & 0xFF);
        if !(0..3).contains(&side) || slot >= 0xB {
            return 0;
        }
        let i = self.object_index(side, slot);
        if !(0..0x54).contains(&i) {
            return 0;
        }
        let o = self.mem.read_u32(world::OBJECT_TABLE + 4 * i as u32);
        if o == 0 {
            return 0;
        }
        for k in 0..4u32 {
            let v = if c != 0 { self.mem.read_u8(c + k) } else { 0 };
            self.mem.write_u8(o + 0x130 + k, v);
        }
        1
    }

    /// 0x1F7790 called with the caller's s0 / s1 = `s` from return address
    /// `ra` [F]: as [`Fx::unit_add_colour`], plus 0x1F77A0's frame (0x30:
    /// s0 +0, s1 +0x10, ra +0x20) left below the caller's stack pointer
    /// (0x1E25E0 is answered by the interpreter runner without a frame).
    pub fn unit_add_colour_from(&mut self, handle: i32, c: u32, s: [u32; 2], ra: u32) -> i32 {
        let f = self.sp.wrapping_sub(0x30);
        self.saved(f, &[(0, u64::from(s[0])), (0x10, u64::from(s[1])), (0x20, u64::from(ra))]);
        self.unit_add_colour(handle, c)
    }
    /// 0x1F77A0(handle, mode, c) [F]: for a unit object (side
    /// 0..2, slot 0..10, 0x1E25E0) 0x1F3A70(index, mode, c): obj+0x1F8 bits
    /// 2..3 = `mode` when 0..3 (the full-window colour pass's ALPHA), the
    /// added colour obj+0x130..0x133 = the 4 bytes at `c` (0: cleared); 1,
    /// else 0. Its 0x30 frame (the caller's s0 / s1 = `s`, `ra`) is left
    /// below the caller's stack pointer. The outcome records' fade
    /// (0x1F77A0(D+8, 2, c) after 0x1F8100).
    pub fn unit_add_mode(&mut self, handle: i32, mode: i32, c: u32, s: [u64; 2], ra: u32) -> i32 {
        let f = self.sp.wrapping_sub(0x30);
        self.saved(f, &[(0, s[0]), (0x10, s[1]), (0x20, u64::from(ra))]);
        let (side, slot) = (handle >> 8, handle & 0xFF);
        if !(0..3).contains(&side) || slot >= 0xB {
            return 0;
        }
        let i = self.object_index(side, slot);
        if !(0..0x54).contains(&i) {
            return 0;
        }
        let o = self.mem.read_u32(world::OBJECT_TABLE + 4 * i as u32);
        if o == 0 {
            return 0;
        }
        if (0..4).contains(&mode) {
            let b = self.mem.read_u8(o + 0x1F8);
            self.mem.write_u8(o + 0x1F8, (b & !0xC) | (((mode & 3) as u8) << 2));
        }
        for k in 0..4u32 {
            let v = if c != 0 { self.mem.read_u8(c + k) } else { 0 };
            self.mem.write_u8(o + 0x130 + k, v);
        }
        1
    }
}
