//! The camera chain of the executable, hand-ported
//! for the unit callbacks of cards 677 / 500 (0x1E02180 / 0x1E02230 run
//! 0x20D660 / 0x20DAD0 / 0x20E3D0): the 4 x 4 inverse 0x20B550 first.
//! Float operations go through [`crate::ee`] in the original's order,
//! COP1's accumulator (mula / madd / msub / adda / suba) as the
//! interpreter models it [F].
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, M4};
use crate::world::{CAMERA_RECORD, CAMERA_STACK_INDEX};

/// 0x20CD50's static basis (0x2E9E70: three rows of 4 floats).
const VIEW_BASIS: u32 = 0x002E_9E70;
/// 0x208B30's fog / depth copies (0x2E9260..0x2E927C).
const FOG_COPY: u32 = 0x002E_9260;

impl Fx {
    /// 0x20B550(out, m) [F]: `out` = the inverse of the 4 x 4
    /// `m` by cofactors (531 words, transliterated one statement per FPU
    /// instruction by the project notes): sixteen cofactors over the
    /// determinant (div.s), each through the soft-double chain 0x1A2C30
    /// (x -1.0 by 0x1A31D8 on the checkerboard's odd places) 0x1A38F8 -
    /// exact, so (float) x or its sign flipped; copied to `out` (which may
    /// be `m`) only after every input is read. A zero determinant (c.eq.s
    /// with 0) returns with `out` untouched. FPU state and the dead frame
    /// (its saved f20..f31, the soft-double residue) are not modelled.
    pub fn invert4(&mut self, out: u32, m: u32) {
        self.invert4_exp(out, m);
    }

    /// [`Fx::invert4`], returning the exponent field of the last cofactor
    /// written (out + 0x3C): a3 as its last soft-double call 0x1A38F8
    /// leaves it (0x1A2A50's biased exponent; 0 for a zero), with a1 = 0
    /// (the copy loop's count) and a2 = the caller's sp; None for a zero
    /// determinant (a1 = `m`, a2 / a3 untouched).
    #[allow(unused_assignments)]
    pub fn invert4_exp(&mut self, out: u32, m: u32) -> Option<u32> {
        let mut f = [0.0f32; 32];
        let mut st = [0.0f32; 0x110 / 4];
        let mut acc: f32;
        f[0] = self.mem.f(m.wrapping_add(0x18));
        f[22] = self.mem.f(m.wrapping_add(0x20));
        f[20] = self.mem.f(m.wrapping_add(0x28));
        f[21] = self.mem.f(m.wrapping_add(0x30));
        f[23] = self.mem.f(m.wrapping_add(0x3c));
        f[27] = self.mem.f(m.wrapping_add(0x24));
        st[23] = f[0];
        f[0] = self.mem.f(m.wrapping_add(0x10));
        f[24] = self.mem.f(m.wrapping_add(0x2c));
        f[10] = st[23];
        f[25] = self.mem.f(m.wrapping_add(0x38));
        f[26] = self.mem.f(m.wrapping_add(0x34));
        f[30] = self.mem.f(m.wrapping_add(0x0));
        st[24] = f[0];
        f[0] = self.mem.f(m.wrapping_add(0x14));
        f[4] = st[24];
        f[12] = ee::mul(f[10], f[24]);
        f[28] = self.mem.f(m.wrapping_add(0x4));
        f[29] = self.mem.f(m.wrapping_add(0xc));
        st[25] = f[0];
        f[0] = st[23];
        f[4] = ee::mul(f[4], f[24]);
        f[6] = ee::mul(f[0], f[22]);
        f[0] = self.mem.f(m.wrapping_add(0x1c));
        st[22] = f[0];
        f[0] = st[25];
        f[3] = st[22];
        f[10] = st[22];
        f[1] = ee::mul(f[0], f[20]);
        f[0] = self.mem.f(m.wrapping_add(0x8));
        acc = ee::mul(f[23], f[1]);
        f[11] = ee::add(acc, ee::mul(f[26], f[12]));
        f[3] = ee::mul(f[3], f[27]);
        st[29] = f[0];
        f[0] = ee::mul(f[21], f[1]);
        f[1] = st[24];
        f[14] = ee::mul(f[10], f[20]);
        f[5] = ee::mul(f[1], f[27]);
        f[1] = st[23];
        f[10] = ee::mul(f[25], f[3]);
        f[7] = ee::mul(f[1], f[27]);
        f[1] = st[25];
        acc = ee::add(f[10], f[11]);
        f[10] = ee::mul(f[23], f[7]);
        f[8] = ee::mul(f[1], f[22]);
        f[1] = st[24];
        f[13] = ee::sub(acc, ee::mul(f[26], f[14]));
        acc = ee::sub(f[13], f[10]);
        f[12] = ee::mul(f[21], f[12]);
        f[9] = ee::mul(f[1], f[20]);
        f[1] = st[25];
        f[13] = ee::mul(f[23], f[9]);
        f[2] = ee::mul(f[1], f[24]);
        f[1] = st[22];
        f[10] = ee::mul(f[21], f[7]);
        f[1] = ee::mul(f[1], f[22]);
        f[13] = ee::add(f[13], f[12]);
        f[12] = ee::mul(f[25], f[1]);
        f[11] = ee::mul(f[21], f[14]);
        f[12] = ee::add(f[12], f[13]);
        f[12] = ee::sub(f[12], f[11]);
        f[11] = ee::mul(f[26], f[1]);
        f[1] = ee::mul(f[23], f[6]);
        f[12] = ee::sub(f[12], f[1]);
        f[1] = ee::mul(f[25], f[4]);
        f[1] = ee::sub(f[12], f[1]);
        f[7] = ee::sub(acc, ee::mul(f[25], f[2]));
        st[26] = f[1];
        acc = ee::mul(f[30], f[7]);
        f[1] = ee::sub(acc, ee::mul(f[28], f[1]));
        acc = ee::mul(f[23], f[5]);
        f[2] = ee::add(acc, ee::mul(f[21], f[2]));
        acc = ee::add(f[11], f[2]);
        f[2] = ee::mul(f[25], f[5]);
        f[6] = ee::mul(f[26], f[6]);
        f[0] = ee::add(f[2], f[0]);
        f[0] = ee::add(f[6], f[0]);
        f[2] = ee::sub(f[0], f[10]);
        f[0] = ee::mul(f[23], f[8]);
        f[3] = ee::sub(acc, ee::mul(f[21], f[3]));
        acc = ee::sub(f[3], f[0]);
        f[0] = ee::sub(acc, ee::mul(f[26], f[4]));
        st[27] = f[0];
        f[0] = ee::mul(f[25], f[8]);
        f[0] = ee::sub(f[2], f[0]);
        f[9] = ee::mul(f[26], f[9]);
        f[2] = st[29];
        f[0] = ee::sub(f[0], f[9]);
        st[28] = f[0];
        f[0] = st[27];
        f[0] = ee::mul(f[2], f[0]);
        acc = ee::add(f[0], f[1]);
        f[0] = st[28];
        f[31] = ee::sub(acc, ee::mul(f[29], f[0]));
        f[0] = 0.0;
        if f[0] == f[31] {
            return None;
        }
        f[12] = ee::div(f[7], f[31]);
        f[0] = f[12];
        st[52] = f[0];
        f[0] = st[26];
        f[12] = ee::div(f[0], f[31]);
        f[0] = ee::neg(f[12]);
        st[56] = f[0];
        f[0] = st[27];
        f[12] = ee::div(f[0], f[31]);
        f[0] = f[12];
        st[60] = f[0];
        f[0] = st[28];
        f[12] = ee::div(f[0], f[31]);
        f[0] = ee::neg(f[12]);
        st[64] = f[0];
        f[0] = ee::mul(f[28], f[20]);
        st[36] = f[0];
        acc = ee::mul(f[23], f[0]);
        f[0] = st[29];
        f[0] = ee::mul(f[0], f[24]);
        st[30] = f[0];
        f[1] = ee::add(acc, ee::mul(f[26], f[0]));
        f[0] = ee::mul(f[29], f[27]);
        st[34] = f[0];
        f[0] = ee::mul(f[25], f[0]);
        acc = ee::add(f[0], f[1]);
        f[0] = ee::mul(f[29], f[20]);
        st[31] = f[0];
        f[1] = ee::sub(acc, ee::mul(f[26], f[0]));
        f[0] = st[29];
        f[0] = ee::mul(f[0], f[27]);
        st[39] = f[0];
        f[0] = ee::mul(f[23], f[0]);
        acc = ee::sub(f[1], f[0]);
        f[0] = ee::mul(f[28], f[24]);
        st[32] = f[0];
        f[0] = ee::sub(acc, ee::mul(f[25], f[0]));
        f[12] = ee::div(f[0], f[31]);
        f[0] = ee::neg(f[12]);
        st[53] = f[0];
        f[0] = ee::mul(f[30], f[20]);
        st[41] = f[0];
        acc = ee::mul(f[23], f[0]);
        f[0] = st[30];
        f[1] = ee::add(acc, ee::mul(f[21], f[0]));
        f[0] = ee::mul(f[29], f[22]);
        st[33] = f[0];
        f[0] = ee::mul(f[25], f[0]);
        acc = ee::add(f[0], f[1]);
        f[0] = st[31];
        f[1] = ee::sub(acc, ee::mul(f[21], f[0]));
        f[0] = st[29];
        f[0] = ee::mul(f[0], f[22]);
        st[38] = f[0];
        f[0] = ee::mul(f[23], f[0]);
        acc = ee::sub(f[1], f[0]);
        f[0] = ee::mul(f[30], f[24]);
        st[35] = f[0];
        f[0] = ee::sub(acc, ee::mul(f[25], f[0]));
        f[12] = ee::div(f[0], f[31]);
        f[0] = f[12];
        st[57] = f[0];
        f[0] = st[33];
        f[1] = ee::mul(f[26], f[0]);
        f[0] = ee::mul(f[30], f[27]);
        st[37] = f[0];
        acc = ee::mul(f[23], f[0]);
        f[0] = st[32];
        f[0] = ee::add(acc, ee::mul(f[21], f[0]));
        acc = ee::add(f[1], f[0]);
        f[0] = st[34];
        f[1] = ee::sub(acc, ee::mul(f[21], f[0]));
        f[0] = ee::mul(f[28], f[22]);
        st[40] = f[0];
        f[0] = ee::mul(f[23], f[0]);
        acc = ee::sub(f[1], f[0]);
        f[0] = st[35];
        f[0] = ee::sub(acc, ee::mul(f[26], f[0]));
        f[12] = ee::div(f[0], f[31]);
        f[0] = ee::neg(f[12]);
        st[61] = f[0];
        f[0] = st[37];
        acc = ee::mul(f[25], f[0]);
        f[0] = st[36];
        f[1] = ee::add(acc, ee::mul(f[21], f[0]));
        f[0] = st[38];
        f[0] = ee::mul(f[26], f[0]);
        acc = ee::add(f[0], f[1]);
        f[0] = st[39];
        f[1] = ee::sub(acc, ee::mul(f[21], f[0]));
        f[0] = st[40];
        f[0] = ee::mul(f[25], f[0]);
        acc = ee::sub(f[1], f[0]);
        f[0] = st[41];
        f[0] = ee::sub(acc, ee::mul(f[26], f[0]));
        f[12] = ee::div(f[0], f[31]);
        f[0] = f[12];
        st[65] = f[0];
        f[0] = st[23];
        f[1] = st[29];
        f[0] = ee::mul(f[28], f[0]);
        st[48] = f[0];
        acc = ee::mul(f[23], f[0]);
        f[0] = st[22];
        f[0] = ee::mul(f[1], f[0]);
        st[42] = f[0];
        f[1] = ee::add(acc, ee::mul(f[26], f[0]));
        f[0] = st[25];
        f[0] = ee::mul(f[29], f[0]);
        st[46] = f[0];
        f[0] = ee::mul(f[25], f[0]);
        acc = ee::add(f[0], f[1]);
        f[0] = st[23];
        f[1] = st[29];
        f[0] = ee::mul(f[29], f[0]);
        st[43] = f[0];
        f[2] = ee::sub(acc, ee::mul(f[26], f[0]));
        f[0] = st[25];
        f[0] = ee::mul(f[1], f[0]);
        st[50] = f[0];
        f[0] = ee::mul(f[23], f[0]);
        acc = ee::sub(f[2], f[0]);
        f[0] = st[22];
        f[0] = ee::mul(f[28], f[0]);
        st[44] = f[0];
        f[0] = ee::sub(acc, ee::mul(f[25], f[0]));
        f[12] = ee::div(f[0], f[31]);
        f[0] = f[12];
        st[54] = f[0];
        f[0] = st[23];
        f[1] = st[29];
        f[0] = ee::mul(f[30], f[0]);
        st[51] = f[0];
        acc = ee::mul(f[23], f[0]);
        f[0] = st[24];
        f[0] = ee::mul(f[29], f[0]);
        st[45] = f[0];
        f[0] = st[24];
        f[0] = ee::mul(f[1], f[0]);
        st[49] = f[0];
        f[0] = st[22];
        f[0] = ee::mul(f[30], f[0]);
        st[47] = f[0];
        f[0] = st[42];
        f[1] = ee::add(acc, ee::mul(f[21], f[0]));
        f[0] = st[45];
        f[0] = ee::mul(f[25], f[0]);
        acc = ee::add(f[0], f[1]);
        f[0] = st[43];
        f[1] = ee::sub(acc, ee::mul(f[21], f[0]));
        f[0] = st[49];
        f[0] = ee::mul(f[23], f[0]);
        acc = ee::sub(f[1], f[0]);
        f[0] = st[47];
        f[0] = ee::sub(acc, ee::mul(f[25], f[0]));
        f[12] = ee::div(f[0], f[31]);
        f[0] = ee::neg(f[12]);
        st[58] = f[0];
        f[0] = st[25];
        f[29] = ee::mul(f[30], f[0]);
        f[0] = st[24];
        acc = ee::mul(f[23], f[29]);
        f[28] = ee::mul(f[28], f[0]);
        f[0] = st[44];
        f[1] = ee::mul(f[23], f[28]);
        f[2] = ee::add(acc, ee::mul(f[21], f[0]));
        f[0] = st[45];
        f[0] = ee::mul(f[26], f[0]);
        acc = ee::add(f[0], f[2]);
        f[0] = st[46];
        f[0] = ee::sub(acc, ee::mul(f[21], f[0]));
        acc = ee::sub(f[0], f[1]);
        f[0] = st[47];
        f[0] = ee::sub(acc, ee::mul(f[26], f[0]));
        f[12] = ee::div(f[0], f[31]);
        f[0] = f[12];
        st[62] = f[0];
        f[0] = st[48];
        acc = ee::mul(f[25], f[29]);
        f[1] = ee::mul(f[25], f[28]);
        f[2] = ee::add(acc, ee::mul(f[21], f[0]));
        f[0] = st[49];
        f[0] = ee::mul(f[26], f[0]);
        acc = ee::add(f[0], f[2]);
        f[0] = st[50];
        f[0] = ee::sub(acc, ee::mul(f[21], f[0]));
        acc = ee::sub(f[0], f[1]);
        f[0] = st[51];
        f[0] = ee::sub(acc, ee::mul(f[26], f[0]));
        f[12] = ee::div(f[0], f[31]);
        f[0] = ee::neg(f[12]);
        st[66] = f[0];
        f[0] = st[48];
        acc = ee::mul(f[24], f[0]);
        f[0] = st[42];
        f[1] = ee::add(acc, ee::mul(f[27], f[0]));
        f[0] = st[46];
        f[0] = ee::mul(f[20], f[0]);
        acc = ee::add(f[0], f[1]);
        f[0] = st[43];
        f[1] = ee::sub(acc, ee::mul(f[27], f[0]));
        f[0] = st[50];
        f[0] = ee::mul(f[24], f[0]);
        acc = ee::sub(f[1], f[0]);
        f[0] = st[44];
        f[0] = ee::sub(acc, ee::mul(f[20], f[0]));
        f[12] = ee::div(f[0], f[31]);
        f[0] = ee::neg(f[12]);
        st[55] = f[0];
        f[0] = st[51];
        acc = ee::mul(f[24], f[0]);
        f[0] = st[42];
        f[1] = ee::add(acc, ee::mul(f[22], f[0]));
        f[0] = st[45];
        f[0] = ee::mul(f[20], f[0]);
        acc = ee::add(f[0], f[1]);
        f[0] = st[43];
        f[1] = ee::sub(acc, ee::mul(f[22], f[0]));
        f[0] = st[49];
        f[0] = ee::mul(f[24], f[0]);
        acc = ee::sub(f[1], f[0]);
        f[0] = st[47];
        f[0] = ee::sub(acc, ee::mul(f[20], f[0]));
        f[12] = ee::div(f[0], f[31]);
        f[0] = f[12];
        st[59] = f[0];
        f[0] = st[44];
        acc = ee::mul(f[24], f[29]);
        f[1] = ee::add(acc, ee::mul(f[22], f[0]));
        f[0] = st[45];
        f[0] = ee::mul(f[27], f[0]);
        acc = ee::add(f[0], f[1]);
        f[0] = st[46];
        f[1] = ee::sub(acc, ee::mul(f[22], f[0]));
        f[0] = ee::mul(f[24], f[28]);
        acc = ee::sub(f[1], f[0]);
        f[0] = st[47];
        f[0] = ee::sub(acc, ee::mul(f[27], f[0]));
        f[12] = ee::div(f[0], f[31]);
        f[0] = ee::neg(f[12]);
        st[63] = f[0];
        f[0] = st[48];
        acc = ee::mul(f[20], f[29]);
        f[1] = ee::mul(f[20], f[28]);
        f[2] = ee::add(acc, ee::mul(f[22], f[0]));
        f[0] = st[49];
        f[0] = ee::mul(f[27], f[0]);
        acc = ee::add(f[0], f[2]);
        f[0] = st[50];
        f[0] = ee::sub(acc, ee::mul(f[22], f[0]));
        acc = ee::sub(f[0], f[1]);
        f[0] = st[51];
        f[0] = ee::sub(acc, ee::mul(f[27], f[0]));
        f[12] = ee::div(f[0], f[31]);
        f[0] = f[12];
        st[67] = f[0];
        for k in 0..16u32 {
            self.mem.write_u32(out.wrapping_add(4 * k), st[(0xD0 / 4 + k) as usize].to_bits());
        }
        Some((st[67].to_bits() >> 23) & 0xFF)
    }

    /// The current camera record (0x2ED720 + 0x1A0 x [0x2ED6C0]).
    pub fn camera_record(&mut self) -> u32 {
        let i = self.mem.read_u32(CAMERA_STACK_INDEX);
        CAMERA_RECORD.wrapping_add(0x1A0u32.wrapping_mul(i))
    }

    /// 0x20AAF0(a, b, n) [F]: `n` quads from `b` to `a` (lq / sq, so both
    /// 16-aligned); returns `a` + 16 n (`a` when n <= 0).
    pub fn copy_quads(&mut self, a: u32, b: u32, n: i32) -> u32 {
        let (mut a, mut b) = (a, b);
        let mut k = n;
        while k > 0 {
            for w in 0..4u32 {
                let x = self.mem.read_u32((b & !15) + 4 * w);
                self.mem.write_u32((a & !15) + 4 * w, x);
            }
            a = a.wrapping_add(16);
            b = b.wrapping_add(16);
            k -= 1;
        }
        a
    }

    /// 0x20B200(m) [F]: VU0 vf28..31 (the current matrix) = `m`; with no
    /// `m` the identity (vf31 = (0, 0, 0, 1), each row above it vf31's
    /// vmr32).
    pub fn load_cur(&mut self, m: u32) {
        self.cur = if m != 0 { self.read_m4(m) } else { [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]] };
    }

    /// 0x2094D0(lo, hi) [F]: the depth range of the frame's Z format
    /// ([0x2E9628], or [0x2E93B8] when bits 0 and 1 of the byte 0x2E93C0
    /// are set): `lo` = 0; `hi` = 0xFFFF (0x3A / 0x32), 0xFFFFFF (0x31),
    /// 0x7FFFFFFF (0x30), else untouched; returns 1.
    pub fn depth_range(&mut self, lo: u32, hi: u32) -> i32 {
        let mut psm = self.mem.read_u32(0x002E_9628);
        let flag = self.mem.read_u8(0x002E_93C0);
        if flag & 1 != 0 && flag & 2 != 0 {
            psm = self.mem.read_u32(0x002E_93B8);
        }
        self.mem.write_u32(lo, 0);
        let v = match psm {
            0x3A | 0x32 => Some(0xFFFF),
            0x31 => Some(0x00FF_FFFF),
            0x30 => Some(0x7FFF_FFFF),
            _ => None,
        };
        if let Some(v) = v {
            self.mem.write_u32(hi, v);
        }
        1
    }

    /// 0x20CD50(out, pos, at, roll) [F]: the view matrix of a camera at
    /// `pos` looking along `at` (the basis rows in the statics 0x2E9E70:
    /// (z, 0, -x), (-y x, x x + z z, -y z), `at` as it is (its w kept),
    /// each over its sqrtf length), `out` = (-row0, 0; -row1, 0; row2, 0;
    /// pos, 1), turned by the roll (0, 0, roll) (0x20BDA0 into its frame
    /// + 0x50, 0x20B370) and inverted (0x20B550 in place).
    pub fn view_matrix(&mut self, out: u32, pos: u32, at: u32, roll: i32) {
        let g = VIEW_BASIS;
        let sp = self.enter(0x90);
        let az = self.mem.f(at + 8);
        self.mem.setf(g, az);
        self.mem.write_u32(g + 4, 0);
        let ax = self.mem.f(at);
        self.mem.setf(g + 8, ee::neg(ax));
        let (ay, ax) = (self.mem.f(at + 4), self.mem.f(at));
        self.mem.setf(g + 0x10, ee::mul(ee::neg(ay), ax));
        let (ax, az) = (self.mem.f(at), self.mem.f(at + 8));
        self.mem.setf(g + 0x14, ee::add(ee::mul(ax, ax), ee::mul(az, az)));
        let (ay, az) = (self.mem.f(at + 4), self.mem.f(at + 8));
        self.mem.setf(g + 0x18, ee::mul(ee::neg(ay), az));
        for w in 0..4u32 {
            let x = self.mem.read_u32((at & !15) + 4 * w);
            self.mem.write_u32(g + 0x20 + 4 * w, x);
        }
        for row in 0..3u32 {
            let b = g + 0x10 * row;
            let (x, y, z) = (self.mem.f(b), self.mem.f(b + 4), self.mem.f(b + 8));
            let acc = ee::add(ee::mul(x, x), ee::mul(y, y));
            let len = self.sqrtf(ee::add(acc, ee::mul(z, z)));
            for k in 0..3u32 {
                let v = self.mem.f(b + 4 * k);
                self.mem.setf(b + 4 * k, ee::div(v, len));
            }
        }
        for row in 0..3u32 {
            for k in 0..3u32 {
                let v = self.mem.f(g + 0x10 * row + 4 * k);
                self.mem.setf(out + 0x10 * row + 4 * k, if row < 2 { ee::neg(v) } else { v });
            }
            self.mem.write_u32(out + 0x10 * row + 0xC, 0);
        }
        for k in 0..3u32 {
            let v = self.mem.read_u32(pos + 4 * k);
            self.mem.write_u32(out + 0x30 + 4 * k, v);
        }
        self.mem.write_u32(out + 0x3C, 0x3F80_0000);
        self.mem.write_u32(sp + 0x40, 0);
        self.mem.write_u32(sp + 0x44, 0);
        self.mem.write_u32(sp + 0x48, roll as u32);
        let r = self.rotation_matrix([0, 0, roll]);
        self.write_m4(sp + 0x50, &r);
        self.mul_into(Some(out), &r);
        self.invert4(out, out);
        self.leave(0x90);
    }

    /// 0x20D050(out, f12..f19, stack f) [F]: the projection: `out` =
    /// (d, 0, 0, 0; 0, d, 0, 0; 0, 0, 0, 1; 0, 0, 1, 0) (its frame + 0x50)
    /// x (identity with +0 = sx, +0x14 = sy, +0x28 = f q (hi - lo) / (f -
    /// q), row 3 = (cx, cy, (lo f - hi q) / (f - q), 1)) where `p` = (d,
    /// sx, sy, cx, cy, lo, hi, q) and `far` the stack argument.
    pub fn projection(&mut self, out: u32, p: [f32; 8], far: f32) {
        let [d, sx, sy, cx, cy, lo, hi, q] = p;
        let sp = self.enter(0x90);
        self.identity(sp + 0x50);
        self.mem.setf(sp + 0x50, d);
        self.mem.setf(sp + 0x64, d);
        self.mem.write_u32(sp + 0x78, 0);
        self.mem.write_u32(sp + 0x8C, 0);
        self.mem.write_u32(sp + 0x88, 0x3F80_0000);
        self.mem.write_u32(sp + 0x7C, 0x3F80_0000);
        self.identity(out);
        let f1 = ee::mul(far, q);
        self.mem.setf(out, sx);
        self.mem.setf(out + 0x14, sy);
        let f0 = ee::add(ee::neg(lo), hi);
        let f1 = ee::mul(f1, f0);
        let acc = ee::mul(ee::neg(hi), q);
        let f2 = ee::add(ee::neg(q), far);
        let f1 = ee::div(f1, f2);
        let f0 = ee::add(acc, ee::mul(lo, far));
        self.mem.setf(out + 0x28, f1);
        self.mem.setf(out + 0x30, cx);
        let f0 = ee::div(f0, f2);
        self.mem.setf(out + 0x34, cy);
        self.mem.setf(out + 0x38, f0);
        let m: M4 = self.read_m4(sp + 0x50);
        self.mul_into(Some(out), &m);
        self.leave(0x90);
    }

    /// 0x20E160() [F]: the current camera record r's matrices from its
    /// input (r + 0xC0..): the view r + 0x40 (0x20CD50 of r + 0x100 /
    /// r + 0x110 / roll r + 0x140), the projection r + 0x80 (0x20D050 of
    /// the distance r + 0x160, aspect r + 0x158 / r + 0x15C x 0x209490's,
    /// the window centre (GS offsets 0x209580 + r + 0x150 / 0x154 - half
    /// the window r + 0x164 / 0x168), the depth range 0x2094D0, r + 0x16C,
    /// 2 r + 0x170), r = projection, r = view x r (0x20B370), r's 4 quads
    /// copied to r + 0xC0; returns 1.
    pub fn camera_build(&mut self) -> i32 {
        let sp = self.enter(0x60);
        let r = self.camera_record();
        let s1 = r + 0xC0;
        let (ox, oy) = (self.mem.read_u32(0x002E_9630), self.mem.read_u32(0x002E_9634));
        self.mem.write_u32(sp + 0x50, ox);
        self.mem.write_u32(sp + 0x54, oy);
        let f6 = ee::from_i32(ox as i32);
        let f2 = ee::from_i32(oy as i32);
        let f3 = ee::div(self.mem.f(r + 0x164), 2.0);
        let f5 = ee::add(f6, self.mem.f(r + 0x150));
        let f0 = ee::div(self.mem.f(r + 0x168), 2.0);
        let f1 = ee::add(f2, self.mem.f(r + 0x154));
        let f20 = ee::sub(f5, f3);
        let f21 = ee::sub(f1, f0);
        self.depth_range(sp + 0x58, sp + 0x5C);
        let roll = self.mem.i32(s1 + 0x80);
        self.view_matrix(r + 0x40, s1 + 0x40, s1 + 0x50, roll);
        let aspect = self.mem.f(0x002E_93D0 + 0x244);
        let f14 = ee::mul(self.mem.f(r + 0x15C), aspect);
        let far = ee::mul(2.0, self.mem.f(r + 0x170));
        self.mem.setf(sp, far);
        let lo = ee::from_i32(self.mem.i32(sp + 0x58));
        let hi = ee::from_i32(self.mem.i32(sp + 0x5C));
        let p = [self.mem.f(r + 0x160), self.mem.f(r + 0x158), f14, f20, f21, lo, hi, self.mem.f(r + 0x16C)];
        self.projection(r + 0x80, p, far);
        self.copy_quads(r, r + 0x80, 4);
        let v = self.read_m4(r + 0x40);
        self.mul_into(Some(r), &v);
        self.copy_quads(s1, r, 4);
        self.leave(0x60);
        1
    }

    /// 0x20E310(out) [F]: the current camera record's view (r + 0x40, 16
    /// words) copied to `out`.
    pub fn camera_view_to(&mut self, out: u32) {
        let r = self.camera_record();
        for k in 0..16u32 {
            let x = self.mem.read_u32(r + 0x40 + 4 * k);
            self.mem.write_u32(out + 4 * k, x);
        }
    }

    /// 0x20DAD0(input) [F]: the current camera record r set from the 9-quad
    /// `input` (as 0x20D660 saves it): copied to r + 0xC0, built (0x20E160),
    /// VU0's current matrix = r + 0x40 (0x20B200), the view copied back
    /// into `input`'s first 4 quads, r + 0x188 / 0x18C / 0x190 =
    /// 0x4A7F8010 + d d, w w / 4 + d d, h h / 4 + d d (d = r + 0x160, w / h
    /// = r + 0x164 / 0x168), the view into 0x2EDBB0 (0x20E310) inverted in
    /// place (0x20B550); returns 1.
    pub fn camera_set(&mut self, input: u32) -> i32 {
        self.camera_set_regs(input);
        1
    }

    /// [`Fx::camera_set`], returning a1..a3 as it leaves them (from its
    /// last call 0x20B550: (0, its sp, the last cofactor's exponent); for
    /// a zero determinant (0x2EDBB0, 0, r + 0x80) as 0x20E310 leaves a2 /
    /// a3).
    pub fn camera_set_regs(&mut self, input: u32) -> [u32; 3] {
        let sp = self.enter(0x30);
        let r = self.camera_record();
        self.copy_quads(r + 0xC0, input, 9);
        self.camera_build();
        self.load_cur(r + 0x40);
        self.copy_quads(input, r + 0x40, 4);
        let d = self.mem.f(r + 0x160);
        self.mem.setf(r + 0x188, ee::add(f32::from_bits(0x4A7F_8010), ee::mul(d, d)));
        let w = self.mem.f(r + 0x164);
        let a = ee::div(ee::mul(w, w), 4.0);
        let d = self.mem.f(r + 0x160);
        self.mem.setf(r + 0x18C, ee::add(a, ee::mul(d, d)));
        let h = self.mem.f(r + 0x168);
        let d = self.mem.f(r + 0x160);
        let a = ee::div(ee::mul(h, h), 4.0);
        self.mem.setf(r + 0x190, ee::add(a, ee::mul(d, d)));
        self.camera_view_to(0x002E_DBB0);
        let e = self.invert4_exp(0x002E_DBB0, 0x002E_DBB0);
        self.leave(0x30);
        match e {
            Some(e) => [0, sp, e],
            None => [0x002E_DBB0, 0, r + 0x80],
        }
    }

    /// 0x2085B0 / 0x208600(p, m) [F]: a DMA cnt of 4 quads and the VIF
    /// UNPACK `vif` (0x6C040044: the projection; 0x6C040056: its inverse),
    /// `m`'s 4 quads at p + 0x10 (sq); returns p + 0x50.
    pub fn vu1_matrix(&mut self, p: u32, m: u32, vif: u32) -> u32 {
        self.mem.write_u32(p, 0x1000_0004);
        self.mem.write_u32(p + 4, 0);
        self.mem.write_u32(p + 8, 0);
        self.mem.write_u32(p + 0xC, vif);
        self.copy_quads(p + 0x10, m, 4);
        p + 0x50
    }

    /// 0x208A70(p, d, lo, hi, flag) [F]: the VU1 clip quads (UNPACK
    /// 0x6C014052: 645 / 2d, -645 / 2d, lo + 0.01, hi - 0.01; UNPACK
    /// 0x6C01405A: (flag == 1, 0, 0, 0)); returns p + 0x40.
    pub fn vu1_clip(&mut self, p: u32, d: f32, lo: f32, hi: f32, flag: i32) -> u32 {
        self.mem.write_u32(p, 0x1000_0001);
        self.mem.write_u32(p + 4, 0);
        self.mem.write_u32(p + 8, 0);
        self.mem.write_u32(p + 0xC, 0x6C01_4052);
        let f2 = ee::mul(2.0, d);
        let f0 = ee::div(f32::from_bits(0xC421_4000), f2);
        let f1 = ee::div(f32::from_bits(0x4421_4000), f2);
        self.mem.setf(p + 0x10, f1);
        self.mem.setf(p + 0x14, f0);
        let e = f32::from_bits(0x3C23_D70A);
        self.mem.setf(p + 0x18, ee::add(e, lo));
        self.mem.setf(p + 0x1C, ee::sub(hi, e));
        self.mem.write_u32(p + 0x20, 0x1000_0001);
        self.mem.write_u32(p + 0x24, 0);
        self.mem.write_u32(p + 0x28, 0);
        self.mem.write_u32(p + 0x2C, 0x6C01_405A);
        self.mem.write_u32(p + 0x30, u32::from(flag == 1));
        self.mem.write_u32(p + 0x34, 0);
        self.mem.write_u32(p + 0x38, 0);
        self.mem.write_u32(p + 0x3C, 0);
        p + 0x40
    }

    /// 0x209A90(p, rgb) [F]: a GIF A+D of FOGCOL (0x3D) from the bytes
    /// rgb[0..2] (r << 16 | g << 8 | b) behind DIRECT 2; returns p + 0x30.
    pub fn gs_fogcol(&mut self, p: u32, rgb: u32) -> u32 {
        self.mem.write_u32(p, 0x1000_0002);
        self.mem.write_u32(p + 4, 0);
        self.mem.write_u32(p + 8, 0);
        self.mem.write_u32(p + 0xC, 0x5100_0002);
        self.mem.write_u64(p + 0x10, 0x1000_0000_0000_8001);
        self.mem.write_u64(p + 0x18, 0xE);
        let (b0, b1, b2) = (self.mem.read_u8(rgb), self.mem.read_u8(rgb + 1), self.mem.read_u8(rgb + 2));
        let v = (u64::from(b0) << 16) | (u64::from(b1) << 8) | u64::from(b2);
        self.mem.write_u64(p + 0x20, v);
        self.mem.write_u64(p + 0x28, 0x3D);
        p + 0x30
    }

    /// 0x208B30(p, a, b, c, e) [F]: the VU1 fog / depth scale quad (UNPACK
    /// 0x6C014053: 255 / (e - c), c x that, 255 / (b - a), b x that), the
    /// four values also kept at 0x2E9260 (+2, +3, +0, +1 order) and a..e at
    /// 0x2E9270..; returns p + 0x20.
    pub fn vu1_fog(&mut self, p: u32, v: [f32; 4]) -> u32 {
        let [a, b, c, e] = v;
        self.mem.write_u32(p, 0x1000_0001);
        let k = f32::from_bits(0x437F_0000);
        let f2 = ee::div(k, ee::sub(e, c));
        self.mem.write_u32(p + 4, 0);
        self.mem.write_u32(p + 8, 0x1000_0000);
        self.mem.write_u32(p + 0xC, 0x6C01_4053);
        let f0 = ee::sub(b, a);
        self.mem.setf(p + 0x10, f2);
        let f1 = ee::div(k, f0);
        self.mem.setf(p + 0x14, ee::mul(c, f2));
        self.mem.setf(p + 0x18, f1);
        self.mem.setf(p + 0x1C, ee::mul(b, f1));
        for (o, s) in [(0u32, 0x18u32), (4, 0x1C), (8, 0x10), (0xC, 0x14)] {
            let x = self.mem.read_u32(p + s);
            self.mem.write_u32(FOG_COPY + o, x);
        }
        for (k, x) in v.iter().enumerate() {
            self.mem.setf(FOG_COPY + 0x10 + 4 * k as u32, *x);
        }
        p + 0x20
    }

    /// 0x20E3D0(p) [F]: the current camera record r's VU1 state into the
    /// packet at `p`: the projection r + 0x80 (0x2085B0), r's inverse
    /// (0x20B550 into a 16-aligned frame local) (0x208600), the clip quads
    /// (0x208A70 of r + 0x160 / 0x16C / 0x170, 1), FOGCOL from r + 0x184
    /// (0x209A90), the fog scale of r + 0x174..0x180 (0x208B30); returns
    /// the pointer after.
    pub fn camera_packet(&mut self, p: u32) -> u32 {
        self.camera_packet_regs(p).0
    }

    /// [`Fx::camera_packet`] with a1..a3 as it leaves them (0x209A90's
    /// FOGCOL bytes: (b0 << 16 | b1 << 8 | b2, b1 << 8 | b2, b2)).
    pub fn camera_packet_regs(&mut self, p: u32) -> (u32, [u32; 3]) {
        let sp = self.enter(0x90);
        let s0 = (sp + 0x4F) & !15;
        let r = self.camera_record();
        let v = self.vu1_matrix(p, r + 0x80, 0x6C04_0044);
        // A singular r (det 0) leaves the local as the stack had it
        // [H: not modelled; met only with a perturbed record index].
        self.invert4(s0, r);
        let v = self.vu1_matrix(v, s0, 0x6C04_0056);
        let (d, lo, hi) = (self.mem.f(r + 0x160), self.mem.f(r + 0x16C), self.mem.f(r + 0x170));
        let v = self.vu1_clip(v, d, lo, hi, 1);
        let v = self.gs_fogcol(v, r + 0x184);
        let f = std::array::from_fn(|k| self.mem.f(r + 0x174 + 4 * k as u32));
        let v = self.vu1_fog(v, f);
        self.leave(0x90);
        let b: [u32; 3] = std::array::from_fn(|k| u32::from(self.mem.read_u8(r + 0x184 + k as u32)));
        (v, [(b[0] << 16) | (b[1] << 8) | b[2], (b[1] << 8) | b[2], b[2]])
    }
}
