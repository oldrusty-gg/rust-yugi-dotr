//! Stack residue of executable helpers (helper C, 30 Oct 2026): what a call
//! leaves below its caller's stack pointer, for ports whose later callees
//! read stale stack words (an uninitialised descriptor slot is the usual
//! case: e.g. the sprite group's TEX1 when the texture is missing).
//!
//! The ports keep locals at the original stack addresses ([`Fx::sp`]); the
//! saved registers a helper pushes depend on the caller's registers, which
//! a port knows and passes here (`None`: not known, left as it is). The
//! low 64 bits of each saved register are written.
use crate::bus::Bus;
use crate::exe::{Fx, M4, CAMERA_TABLE};

/// A 128-bit register save (`sq`) of a 32-bit value: the upper 64 bits 0
/// [H, as `Fx::saved`].
fn put(fx: &mut Fx, a: u32, v: Option<u32>) {
    if let Some(v) = v {
        fx.mem.write_u64(a, u64::from(v));
        fx.mem.write_u64(a + 8, 0);
    }
}

impl Fx {
    /// A callee's prologue saves (`sq reg, off(sp)`) left at `base` (its
    /// stack pointer): each (offset, register) as 128 bits, the upper 64
    /// bits 0 (registers set by 32-bit ops) [H: the upper halves are 0 in
    /// every interpreter run compared].
    pub fn saved(&mut self, base: u32, slots: &[(u32, u64)]) {
        for &(off, v) in slots {
            self.mem.write_u64(base + off, v);
            self.mem.write_u64(base + off + 8, 0);
        }
    }

    /// What a call of 0x1A2B80 / 0x1A2C70 (the soft-float conversions) on
    /// `v` leaves below the caller's stack pointer [F; the interpreter
    /// runner's `native_fix_frame`]: its frame (0x30) with the unpacked
    /// float (class +0, sign +4, exponent +8, mantissa +0xC), the bits
    /// +0x10 and the return address `ra` +0x20.
    pub fn fix_residue(&mut self, v: f32, ra: u32) {
        let bits = v.to_bits();
        let f = self.sp.wrapping_sub(0x30);
        self.mem.write_u32(f + 0x10, bits);
        self.mem.write_u64(f + 0x20, u64::from(ra));
        self.mem.write_u32(f + 4, bits >> 31);
        let field = (bits >> 23) & 0xFF;
        if field == 0 {
            self.mem.write_u32(f, 2);
        } else {
            self.mem.write_u32(f + 8, field.wrapping_sub(127));
            self.mem.write_u32(f, 3);
            self.mem.write_u32(f + 0xC, ((bits & 0x7F_FFFF) << 7) | 0x4000_0000);
        }
    }

    /// 0x19AE88 rand() called with the caller's s0 = `s0` from return
    /// address `ra` [F]: its frame (0x10: `sd s0` +0, `sd ra` +8) left
    /// below the caller's stack pointer.
    pub fn rand_from(&mut self, s0: u32, ra: u32) -> i32 {
        let f = self.sp.wrapping_sub(0x10);
        self.mem.write_u64(f, u64::from(s0));
        self.mem.write_u64(f + 8, u64::from(ra));
        self.rand()
    }

    /// 0x1038D0's body (as `xform_world`): the TRS matrix it builds.
    fn world_trs(&mut self, x: u32) -> M4 {
        let vp = self.camera_vp();
        self.write_m4(x + 0x30, &vp);
        let sc = std::array::from_fn(|k| self.mem.read_f32(x + 4 * k as u32));
        let rot = std::array::from_fn(|k| self.mem.read_u32(x + 0x10 + 4 * k as u32) as i32);
        let tr = std::array::from_fn(|k| self.mem.read_f32(x + 0x20 + 4 * k as u32));
        let m = self.trs(Some(sc), Some(rot), Some(tr));
        self.mul_into(Some(x + 0x30), &m);
        m
    }

    /// 0x1038D0(x) called with the caller's s0..s3 = `s` and return address
    /// `ra` [F]: as [`Fx::xform_world`], plus its frame (0x60: s0 +0, ra
    /// +0x10, the TRS matrix +0x20) and 0x20C0B0's below it (0x70: s0 = x
    /// +0x20, s1..s3 +0x30..+0x50, ra 0x1038FC +0x60).
    pub fn xform_world_saving(&mut self, x: u32, ra: u32, s: [Option<u32>; 4]) {
        let m = self.world_trs(x);
        let f = self.sp - 0x60;
        self.write_m4(f + 0x20, &m);
        put(self, f, s[0]);
        put(self, f + 0x10, Some(ra));
        let g = f - 0x70;
        put(self, g + 0x20, Some(x));
        put(self, g + 0x30, s[1]);
        put(self, g + 0x40, s[2]);
        put(self, g + 0x50, s[3]);
        put(self, g + 0x60, Some(0x0010_38FC));
    }

    /// 0x1039A0(x) (the billboard) called with the caller's s0..s3 = `s`
    /// and return address `ra` [F]: as [`Fx::xform_billboard`], plus its
    /// frame (0x80: s0 +0, s1 +0x10, ra +0x20, the camera angles +0x30
    /// (when there is a camera object), the TRS matrix +0x40) and
    /// 0x20C0B0's below it (0x70: s0 = frame + 0x34, s1 = x, s2 / s3, ra
    /// 0x103A08). Returns the raw yaw angle.
    pub fn xform_billboard_saving(&mut self, x: u32, ra: u32, s: [Option<u32>; 4]) -> i32 {
        let a = self.camera_angles();
        let cam = self.mem.read_u32(CAMERA_TABLE) != 0;
        self.mem.write_u32(x + 0x14, (a[1] & 0xFFFF) as u32);
        self.mem.write_u32(x + 0x10, a[0] as u32);
        self.mem.write_u32(x + 0x18, a[2] as u32);
        let m = self.world_trs(x);
        let f = self.sp - 0x80;
        if cam {
            for k in 0..4u32 {
                self.mem.write_u32(f + 0x30 + 4 * k, a[k as usize] as u32);
            }
        }
        self.write_m4(f + 0x40, &m);
        put(self, f, s[0]);
        put(self, f + 0x10, s[1]);
        put(self, f + 0x20, Some(ra));
        let g = f - 0x70;
        put(self, g + 0x20, Some(f + 0x34));
        put(self, g + 0x30, Some(x));
        put(self, g + 0x40, s[2]);
        put(self, g + 0x50, s[3]);
        put(self, g + 0x60, Some(0x0010_3A08));
        a[1]
    }

    /// 0x20C0B0(out, scale, rot, trans) called with the caller's f20..f24
    /// = `f`, s0..s3 = `s` and return address `ra` [F]: as [`Fx::trs`] (a
    /// pointer 0 leaves that part out: the vec3 of floats at `trans`, the
    /// i32 angles at `rot`, the row scales at `scale`) into `out`, plus its
    /// frame (0x70) below the caller's sp: f20..f24 +0..+0x10, s0..s3
    /// +0x20..+0x50, ra +0x60. Its callees (0x20B320 / 0x20ABF0 /
    /// 0x20ACC0) are leaves.
    #[allow(clippy::too_many_arguments)]
    pub fn trs_into_saving(&mut self, out: u32, scale: u32, rot: u32, trans: u32, f: [f32; 5], s: [u64; 4], ra: u32) {
        let v3 = |fx: &mut Fx, p: u32| [fx.mem.read_f32(p), fx.mem.read_f32(p + 4), fx.mem.read_f32(p + 8)];
        let t = (trans != 0).then(|| v3(self, trans));
        let r = (rot != 0).then(|| [self.mem.read_u32(rot) as i32, self.mem.read_u32(rot + 4) as i32, self.mem.read_u32(rot + 8) as i32]);
        let sc = (scale != 0).then(|| v3(self, scale));
        let m = self.trs(sc, r, t);
        self.write_m4(out, &m);
        let g = self.sp - 0x70;
        for (k, v) in f.iter().enumerate() {
            self.mem.write_f32(g + 4 * k as u32, *v);
        }
        self.saved(g, &[(0x20, s[0]), (0x30, s[1]), (0x40, s[2]), (0x50, s[3]), (0x60, u64::from(ra))]);
    }
}
