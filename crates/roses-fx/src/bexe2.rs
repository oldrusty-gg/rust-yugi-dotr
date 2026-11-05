//! Executable helpers the second-library BMODULE records call (helper A2,
//! 30 Oct 2026). Addresses are the PAL executable's [F].
use crate::bus::Bus;
use crate::exe::Fx;

impl Fx {
    /// 0x1F7370(handle, out): `out` (vec4) cleared, then for a valid handle
    /// (side 0..2, slot 0..10) with a unit object (0x1E25E0) the object's
    /// rotation (+0x10..+0x1F, 0x1F3540) copied into it; 1 when copied (or
    /// when `out` is 0 and the object exists).
    pub fn unit_rotation(&mut self, handle: i32, out: u32) -> i32 {
        for k in [0xCu32, 8, 4, 0] {
            self.mem.write_u32(out + k, 0);
        }
        let (side, slot) = (handle >> 8, handle & 0xFF);
        if !(0..3).contains(&side) || slot >= 0xB {
            return 0;
        }
        let i = self.object_index(side, slot);
        if i < 0 {
            return 0;
        }
        let o = self.object_at(i);
        if o == 0 {
            return 0;
        }
        if out != 0 {
            for k in 0..4 {
                let w = self.mem.read_u32(o + 0x10 + 4 * k);
                self.mem.write_u32(out + 4 * k, w);
            }
        }
        1
    }

    /// 0x1FAF40(index) [F]: the camera object `index`'s +0x30
    /// (beside 0x1FACE0's +0x20 eye), 0 for a missing object or an index
    /// other than 0 (the original branches away on index < 0 and > 0).
    pub fn camera_at30(&mut self, index: i32) -> u32 {
        if index != 0 {
            return 0;
        }
        let cam = self.mem.read_u32(crate::exe::CAMERA_TABLE);
        if cam == 0 {
            0
        } else {
            cam.wrapping_add(0x30)
        }
    }

    /// 0x1F66F0(handle) [F]: the unit's node count (object
    /// byte +0x1FE) for a valid handle (side 0..2, slot 0..10) whose object
    /// (0x1E25E0) exists and is placed (i8 +0x1F2 >= 0, 0x1F3D40); else -1.
    pub fn unit_nodes(&mut self, handle: i32) -> i32 {
        let (side, slot) = (handle >> 8, handle & 0xFF);
        if !(0..3).contains(&side) || slot >= 0xB {
            return -1;
        }
        let i = self.object_index(side, slot);
        if i < 0 {
            return -1;
        }
        let o = self.object_at(i);
        if o == 0 || self.mem.i8(o + 0x1F2) < 0 {
            return -1;
        }
        i32::from(self.mem.read_u8(o + 0x1FE))
    }
}
