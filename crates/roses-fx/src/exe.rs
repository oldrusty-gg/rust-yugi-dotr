//! The executable's helpers the effect programs call, hand-ported, and the
//! per-update frame every port runs in ([`Fx`]): the memory picture, the
//! VU0 matrix registers, the rand state, the primitives submitted and the
//! state read back after the call, exactly as the interpreter runner
//! (`roses_ee::bmodule::AttackEffect::update`) frames an interpreted call.
//!
//! Addresses are the PAL executable's [F]. Float operations go through
//! [`crate::ee`] in the original's order.
use crate::bus::{Bus, Image, Mem};
use crate::ee;
use crate::types::*;
use crate::world;
use std::collections::BTreeMap;
use std::sync::Arc;

/// The ordering table 0x2E91B8 0x206E40 returns.
pub const OT: u32 = 0x002E_91B8;
/// The self / target handles 0x1F65B0 / 0x1F6590 read (set by 0x1F65A0 /
/// 0x1F6580 before every call).
pub const TARGET_HANDLE: u32 = 0x002D_F5A8;
pub const SELF_HANDLE: u32 = 0x002D_F5AC;
/// newlib's reentrancy pointer: the rand state is the u64 at *it + 0xA8.
pub const RAND_REENT: u32 = 0x0028_ED18;
/// The quarter-wave sine table of 0x20ABF0 / 0x20AD90 (1025 f32).
pub const SINE_TABLE: u32 = 0x0029_3430;
/// The camera object table 0x2DF5C0 (0x1FAC30 / 0x1FC170).
pub const CAMERA_TABLE: u32 = 0x002D_F5C0;
/// The VU0 matrix stack 0x2E9EA0 (0x70-byte records) and its index.
const MATRIX_STACK: u32 = 0x002E_9EA0;
const MATRIX_STACK_INDEX: u32 = 0x002E_D6A0;
/// Where the port builds descriptors it would keep on the stack (any free
/// address: a primitive's `desc` is research only).
pub const SCRATCH: u32 = 0x01F8_0000;
/// The stack the interpreter runner calls with.
pub const STACK: u32 = 0x01F0_0000;
/// Where module records are linked.
pub const LINK_ADDRESS: u32 = 0x01E0_0000;

pub type M4 = [[f32; 4]; 4];
pub type V4 = [f32; 4];

/// One running port's machine-free frame.
#[derive(Clone)]
pub struct Fx {
    pub mem: Mem,
    /// VU0 vf28..vf31, the "current matrix" of the matrix library.
    pub cur: M4,
    /// 0x202530's answers: (type, id) -> RH2 address in the texture area.
    pub textures: BTreeMap<(i32, i32), u32>,
    /// The update being built.
    pub out: EffectUpdate,
    /// The buffer this frame draws into (frame counter odd -> TBP 0).
    drawn_tbp: u16,
    /// Calls made (the launch is call 0).
    pub calls: usize,
    pub finished: bool,
    pub handles: [u16; 2],
    /// The stack pointer of the call being ported: locals the original
    /// keeps on the stack are kept at the same addresses (stale values
    /// read as the original would read them).
    pub sp: u32,
    /// LO / HI of the last division (a division by zero keeps them, as
    /// the EE does).
    lo: i32,
    hi: i32,
    /// Packet layers open (0x2078A0 .. 0x207910) and the last id handed
    /// out this update, numbered as the interpreter runner numbers them
    ///.
    pub layers_open: Vec<u32>,
    pub layer_next: u32,
    /// What the offscreen block at TBP 0x2000 last received (a unit drawn
    /// there by 0x1F8270 -> 0x1EF6F0), kept across updates as the runner
    /// keeps it: the texture of a primitive sampling TBP 0x2000.
    pub rt: Option<TextureKey>,
    /// The battle the outcome boundary 0x1F8100 answers from
    /// ([`EffectWorld::battle`], set by `begin`; helper D30).
    pub battle: Option<BattleState>,
}

impl Fx {
    /// The memory picture of a freshly loaded program: the executable's
    /// image, the record at 0x1E00000, the textures, the static state of
    /// `model` (as `AttackEffect::load` / `load_cmodule`).
    pub fn new(image: Arc<Image>, record: &[u8], model: u16, handles: [u16; 2], textures: &EffectTextures) -> Self {
        let mut mem = Mem::new(image);
        mem.load(LINK_ADDRESS, record);
        let textures_placed = world::place_textures(&mut mem, textures);
        world::static_state(&mut mem, model);
        Self { mem, cur: [[0.0; 4]; 4], textures: textures_placed, out: EffectUpdate::default(), drawn_tbp: 0x800, calls: 0, finished: false, handles, sp: STACK, lo: 0, hi: 0, layers_open: Vec::new(), layer_next: 0, rt: None, battle: None }
    }

    /// Before a call: the world mirrored, the handles set (0x1F52E0 sets
    /// both before every call).
    pub fn begin(&mut self, w: &dyn EffectWorld) {
        world::mirror(&mut self.mem, w, self.calls);
        self.battle = w.battle();
        self.out = EffectUpdate::default();
        self.drawn_tbp = if self.mem.read_u32(world::FRAME_COUNTER) & 1 == 1 { 0 } else { 0x800 };
        let [own, target] = self.handles;
        self.mem.write_u32(TARGET_HANDLE, u32::from(target));
        self.mem.write_u32(SELF_HANDLE, u32::from(own));
        self.sp = STACK;
        self.layers_open.clear();
        self.layer_next = 0;
    }

    /// A function's frame of `bytes`: the new stack pointer.
    pub fn enter(&mut self, bytes: u32) -> u32 {
        self.sp -= bytes;
        self.sp
    }
    pub fn leave(&mut self, bytes: u32) {
        self.sp += bytes;
    }

    /// div: (quotient, remainder) truncating; by zero the previous ones.
    pub fn div(&mut self, a: i32, b: i32) -> (i32, i32) {
        if b != 0 {
            self.lo = a.wrapping_div(b);
            self.hi = a.wrapping_rem(b);
        }
        (self.lo, self.hi)
    }

    /// mult: LO / HI the low / high halves of the 64-bit product (helper
    /// B: so a later division by zero sees what the original's does).
    pub fn mult(&mut self, a: i32, b: i32) -> i32 {
        let p = i64::from(a) * i64::from(b);
        self.lo = p as i32;
        self.hi = (p >> 32) as i32;
        self.lo
    }

    /// LO / HI as a call finds them (the function-level harness sets the
    /// interpreter's).
    pub fn set_hilo(&mut self, lo: i32, hi: i32) {
        self.lo = lo;
        self.hi = hi;
    }
    pub fn hilo(&self) -> (i32, i32) {
        (self.lo, self.hi)
    }

    /// After a call returning `ret`: the state the battle reads back.
    pub fn end(&mut self, ret: i32) -> EffectUpdate {
        let mut out = std::mem::take(&mut self.out);
        out.ret = ret;
        self.calls += 1;
        if ret == 8 {
            self.finished = true;
        }
        let m = &mut self.mem;
        for role in 0..2 {
            let o = world::OBJECT_RECORD[role];
            out.flags_20d[role] = m.read_u8(o + 0x20D);
            out.stage_light = m.read_u8(STAGE_LIGHT);
            out.unit_writes[role] = UnitWrites {
                tint: std::array::from_fn(|k| m.read_u8(o + 0x12C + k as u32)),
                add: std::array::from_fn(|k| m.read_u8(o + 0x130 + k as u32)),
                scale: std::array::from_fn(|k| m.read_f32(o + 0x30 + 4 * k as u32)),
                mode: m.read_u8(o + 0x1F8),
                fade: m.read_f32(o + 0x128),
            };
        }
        out
    }

    pub fn rand_state(&mut self) -> u64 {
        let p = self.mem.read_u32(RAND_REENT);
        self.mem.read_u64(p.wrapping_add(0xA8))
    }
    pub fn set_rand_state(&mut self, s: u64) {
        let p = self.mem.read_u32(RAND_REENT);
        self.mem.write_u64(p.wrapping_add(0xA8), s);
    }

    // ------------------------------------------------------------ libc

    /// 0x19AE88 rand(): newlib's LCG on the u64 at *0x28ED18 + 0xA8.
    /// LO / HI after it are its __muldi3's (0x1A0EA8) last pipeline-0
    /// `mult`: the old state's low word (signed) x 0x5851F42D (helper B2:
    /// a later division by zero reads them).
    pub fn rand(&mut self) -> i32 {
        let p = self.mem.read_u32(RAND_REENT).wrapping_add(0xA8);
        let old = self.mem.read_u64(p);
        let s = old.wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(1);
        self.mem.write_u64(p, s);
        self.mult(old as u32 as i32, 0x5851_F42D);
        ((s >> 32) & 0x7FFF_FFFF) as i32
    }

    /// 0x1959C8 bzero(p, n).
    pub fn bzero(&mut self, p: u32, n: u32) {
        for k in 0..n {
            self.mem.write_u8(p + k, 0);
        }
    }

    // ------------------------------------------------------------ trig

    /// 0x20ABF0 (cosine, `cos`) / 0x20ACC0 (sine) of a 16-bit angle from the
    /// quarter-wave table 0x293430: |a| folded into 0..0x4000, index a >> 4,
    /// (T[i] (16 - f) + T[i + 1] f) / 16 with the sign.
    pub fn table_trig(&mut self, angle: i32, cos: bool) -> f32 {
        let mut a = i64::from((angle as u32).wrapping_add(if cos { 0x4000 } else { 0 }) as u16 as i16);
        let mut sign = 1.0f32;
        if a < 0 {
            sign = -1.0;
            a = -a;
        }
        if a >= 0x4001 {
            a = 0x8000 - a;
        }
        if a & 0x3FFF == 0 {
            return if a == 0x4000 { sign } else { 0.0 };
        }
        let at = SINE_TABLE + 4 * (a >> 4) as u32;
        let (t0, t1) = (self.mem.read_f32(at), self.mem.read_f32(at + 4));
        let frac = (a & 15) as i32;
        let acc = ee::mul(t0, ee::from_i32(16 - frac));
        let m = ee::add(acc, ee::mul(t1, ee::from_i32(frac)));
        ee::div(ee::mul(sign, m), 16.0)
    }
    pub fn tcos(&mut self, a: i32) -> f32 {
        self.table_trig(a, true)
    }
    pub fn tsin(&mut self, a: i32) -> f32 {
        self.table_trig(a, false)
    }

    /// 0x20AD90 cosf / 0x20AEA0 sinf of radians through the table: the
    /// angle 0x1A2B80(65536 x / 6.2831855 + 0.5 (+ 16384 for the cosine)),
    /// its low 16 bits signed.
    pub fn cosf(&mut self, x: f32) -> f32 {
        let t = ee::add(0.5, ee::div(ee::mul(65536.0, x), f32::from_bits(0x40C9_0FDB)));
        let a = ee::fixsfsi(ee::add(16384.0, t));
        self.fold_trig(a as i16 as i32)
    }
    pub fn sinf(&mut self, x: f32) -> f32 {
        let t = ee::add(0.5, ee::div(ee::mul(65536.0, x), f32::from_bits(0x40C9_0FDB)));
        let a = ee::fixsfsi(t);
        self.fold_trig(a as i16 as i32)
    }
    fn fold_trig(&mut self, a: i32) -> f32 {
        self.table_trig(a, false)
    }

    // ------------------------------------------------------------ matrices

    /// 0x20B260(m): push vf28..31 on the stack 0x2E9EA0 and load `m`.
    pub fn push(&mut self, m: Option<M4>) {
        let idx = self.mem.read_u32(MATRIX_STACK_INDEX);
        let base = MATRIX_STACK.wrapping_add(idx.wrapping_mul(0x70));
        for r in 0..4 {
            for k in 0..4 {
                self.mem.write_f32((base + 16 * r as u32) & !15 | (4 * k as u32), self.cur[r][k]);
            }
        }
        if let Some(m) = m {
            self.cur = m;
        }
        self.mem.write_u32(MATRIX_STACK_INDEX, idx.wrapping_add(1));
    }
    /// 0x20B2C0: pop vf28..31.
    pub fn pop(&mut self) {
        let idx = self.mem.read_u32(MATRIX_STACK_INDEX).wrapping_sub(1);
        self.mem.write_u32(MATRIX_STACK_INDEX, idx);
        let base = MATRIX_STACK.wrapping_add(idx.wrapping_mul(0x70));
        for r in 0..4 {
            for k in 0..4 {
                self.cur[r][k] = self.mem.read_f32(((base + 16 * r as u32) & !15) + 4 * k as u32);
            }
        }
    }

    pub fn read_m4(&mut self, a: u32) -> M4 {
        std::array::from_fn(|r| std::array::from_fn(|k| self.mem.read_f32(((a + 16 * r as u32) & !15) + 4 * k as u32)))
    }
    pub fn write_m4(&mut self, a: u32, m: &M4) {
        for r in 0..4 {
            for k in 0..4 {
                self.mem.write_f32(((a + 16 * r as u32) & !15) + 4 * k as u32, m[r][k]);
            }
        }
    }
    pub fn read_v4(&mut self, a: u32) -> V4 {
        std::array::from_fn(|k| self.mem.read_f32(a + 4 * k as u32))
    }
    pub fn write_v4(&mut self, a: u32, v: V4) {
        for k in 0..4 {
            self.mem.write_f32(a + 4 * k as u32, v[k]);
        }
    }

    /// VU0 row vector times matrix: ACC = M0 x; += M1 y; += M2 z; + M3 w.
    pub fn vu_apply(m: &M4, v: V4) -> V4 {
        std::array::from_fn(|j| {
            let acc = ee::vmul(m[0][j], v[0]);
            let acc = ee::vmadd(acc, m[1][j], v[1]);
            let acc = ee::vmadd(acc, m[2][j], v[2]);
            ee::vmadd(acc, m[3][j], v[3])
        })
    }

    /// 0x20C540(out, m, v): v times `m` (FPU: ACC = v1 m10; f2 = v2 m20;
    /// f4 = ACC + v0 m00; ACC = f2 + f4; + v3 m30, per column), or times
    /// the current matrix (VU0) when `m` is none.
    pub fn transform(&self, m: Option<&M4>, v: V4) -> V4 {
        match m {
            None => Self::vu_apply(&self.cur, v),
            Some(m) => std::array::from_fn(|k| {
                let acc = ee::mul(v[1], m[1][k]);
                let f2 = ee::mul(v[2], m[2][k]);
                let f4 = ee::add(acc, ee::mul(v[0], m[0][k]));
                let acc = ee::add(f2, f4);
                ee::add(acc, ee::mul(v[3], m[3][k]))
            }),
        }
    }

    /// 0x20B370(a, b): `a` = `b` x `a` (every row of `b` through `a`); with
    /// no `a`, the current matrix = `b` x current.
    pub fn mul_into(&mut self, a: Option<u32>, b: &M4) {
        match a {
            None => {
                let c = self.cur;
                self.cur = std::array::from_fn(|r| Self::vu_apply(&c, b[r]));
            }
            Some(a) => {
                let m = self.read_m4(a);
                let r: M4 = std::array::from_fn(|r| Self::vu_apply(&m, b[r]));
                self.write_m4(a, &r);
            }
        }
    }

    /// The rotation part of 0x20BDA0 / 0x20C0B0 from angles (rx, ry, rz)
    /// (table cosines / sines): rows 0..2, columns 0..2.
    fn rotation(&mut self, rot: [i32; 3]) -> [[f32; 3]; 3] {
        let cx = self.tcos(rot[0]);
        let sx = self.tsin(rot[0]);
        let cy = self.tcos(rot[1]);
        let sy = self.tsin(rot[1]);
        let cz = self.tcos(rot[2]);
        let sz = self.tsin(rot[2]);
        let f3 = ee::mul(sx, sy);
        let f4 = ee::mul(cx, sy);
        [
            [ee::mul(cy, cz), ee::mul(cy, sz), ee::neg(sy)],
            [ee::sub(ee::mul(cz, f3), ee::mul(cx, sz)), ee::add(ee::mul(sz, f3), ee::mul(cx, cz)), ee::mul(sx, cy)],
            [ee::add(ee::mul(cz, f4), ee::mul(sx, sz)), ee::sub(ee::mul(sz, f4), ee::mul(sx, cz)), ee::mul(cx, cy)],
        ]
    }

    /// 0x20BDA0(out, rot): the rotation matrix (translation 0, w 1).
    pub fn rotation_matrix(&mut self, rot: [i32; 3]) -> M4 {
        let r = self.rotation(rot);
        [[r[0][0], r[0][1], r[0][2], 0.0], [r[1][0], r[1][1], r[1][2], 0.0], [r[2][0], r[2][1], r[2][2], 0.0], [0.0, 0.0, 0.0, 1.0]]
    }

    /// 0x20C0B0(out, scale, rot, trans): identity, the translation, the
    /// rotation, then rows 0..2 scaled.
    pub fn trs(&mut self, scale: Option<[f32; 3]>, rot: Option<[i32; 3]>, trans: Option<[f32; 3]>) -> M4 {
        let mut m: M4 = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
        if let Some(t) = trans {
            m[3][..3].copy_from_slice(&t);
        }
        if let Some(rot) = rot {
            let r = self.rotation(rot);
            for i in 0..3 {
                m[i][..3].copy_from_slice(&r[i]);
            }
        }
        if let Some(s) = scale {
            for i in 0..3 {
                for k in 0..3 {
                    m[i][k] = ee::mul(m[i][k], s[i]);
                }
            }
        }
        m
    }

    /// 0x20E2B0: the current camera record's view-projection (0x2ED720 +
    /// 0x1A0 x index 0x2ED6C0).
    pub fn camera_vp(&mut self) -> M4 {
        let idx = self.mem.read_u32(world::CAMERA_STACK_INDEX);
        let base = world::CAMERA_RECORD.wrapping_add(idx.wrapping_mul(0x1A0));
        std::array::from_fn(|r| std::array::from_fn(|k| self.mem.read_f32(base + 16 * r as u32 + 4 * k as u32)))
    }

    /// 0x204D80(v, m, out, w): `v` (w forced 1) through `m` (the camera's
    /// view-projection when none) with VU0, divided by w; on screen (1 <= z
    /// <= 16777215, 0 <= x, y <= 4095) the XYZ2 value (16 x, 16 y as
    /// 0x1A2C70, z & 0xFFFFFF at bit 32) is merged into `xyz` (bits 56..63
    /// kept) and w returned.
    pub fn project(&mut self, v: [f32; 3], m: Option<&M4>, xyz: &mut u64) -> Option<f32> {
        let m = match m {
            Some(m) => *m,
            None => self.camera_vp(),
        };
        self.push(Some(m));
        let o = self.transform(None, [v[0], v[1], v[2], 1.0]);
        self.pop();
        let q = ee::div(1.0, o[3]);
        let x = ee::mul(o[0], q);
        let y = ee::mul(o[1], q);
        let z = ee::mul(o[2], q);
        // The original keeps the scaled values in its local vector.
        if ee_lt(z, 1.0) || !ee_le(z, f32::from_bits(0x4B7F_FFFF)) || ee_lt(x, 0.0) || !ee_le(x, f32::from_bits(0x457F_F000)) || ee_lt(y, 0.0) || !ee_le(y, f32::from_bits(0x457F_F000)) {
            return None;
        }
        // 0x1A2C70's frames below 0x204D80's (0xB0) as the original
        // leaves them: 16 x from 0x204EE8, 16 y from 0x204F00, z from
        // 0x204F0C.
        let (fx16, fy16) = (ee::mul(16.0, x), ee::mul(16.0, y));
        let save = self.sp;
        self.sp = save.wrapping_sub(0xB0);
        self.fix_residue(fx16, 0x0020_4EE8);
        self.fix_residue(fy16, 0x0020_4F00);
        self.fix_residue(z, 0x0020_4F0C);
        self.sp = save;
        let x16 = u64::from(ee::fixunssfsi(fx16) as u16);
        let y16 = u64::from(ee::fixunssfsi(fy16) as u16);
        let fz = u64::from(ee::fixunssfsi(z) & 0xFF_FFFF);
        *xyz = (*xyz & !0xFFFF_FFFF) | x16 | (y16 << 16);
        *xyz = (*xyz & 0xFF00_0000_FFFF_FFFF) | (fz << 32);
        Some(o[3])
    }

    // ------------------------------------------------------------ effect library

    /// 0x1E2370(col, row, height, out): a cell's centre (50 (col - 3), 35
    /// height / 128, 50 (row - 3), 1); out of range axes 0.
    pub fn cell_position(col: i32, row: i32, height: i32) -> V4 {
        let mut v = [0.0, 0.0, 0.0, 1.0];
        if (0..7).contains(&col) {
            v[0] = ee::from_i32((col - 3) * 50);
        }
        if (0..7).contains(&row) {
            v[2] = ee::from_i32((row - 3) * 50);
        }
        v[1] = ee::div(ee::from_i32(height.wrapping_mul(35)), 128.0);
        v
    }

    /// 0x1FAC30(0): the camera object's projection distance (+0x44).
    pub fn camera_distance(&mut self) -> f32 {
        let cam = self.mem.read_u32(CAMERA_TABLE);
        if cam == 0 {
            0.0
        } else {
            self.mem.read_f32(cam + 0x44)
        }
    }

    /// 0x1018E0(size, depth): size / (depth / 256) x (distance / 800).
    pub fn view_scale(&mut self, size: f32, depth: f32) -> f32 {
        let d = ee::div(self.camera_distance(), 800.0);
        ee::mul(ee::div(size, ee::div(depth, 256.0)), d)
    }

    /// 0x101600(xyz): both 12.4 window coordinates (u16 at +0, +2) within
    /// [24576, 40960] (pixels 1536..2560 of the 4096 space).
    pub fn on_screen(xyz: u64) -> bool {
        let inside = |v: u16| {
            let f = ee::from_i32(i32::from(v));
            !ee_lt(f, 24576.0) && ee_le(f, 40960.0)
        };
        inside(xyz as u16) && inside((xyz >> 16) as u16)
    }

    /// 0x1033D0(v, target, step): each component steps by `step` while at
    /// most target - step, else is set to `target`; bit k set when
    /// component k stepped.
    pub fn approach(&mut self, v: u32, target: f32, step: f32) -> i32 {
        let lim = ee::sub(target, step);
        let mut bits = 0;
        for k in 0..3u32 {
            let a = v + 4 * k;
            let x = self.mem.read_f32(a);
            if ee_le(x, lim) {
                self.mem.write_f32(a, ee::add(x, step));
                bits |= 1 << k;
            } else {
                self.mem.write_f32(a, target);
                bits &= !(1 << k);
            }
        }
        bits
    }

    /// 0x103620(a, b): a += b (xyz).
    pub fn add3(&mut self, a: u32, b: u32) {
        for k in 0..3u32 {
            let v = ee::add(self.mem.read_f32(a + 4 * k), self.mem.read_f32(b + 4 * k));
            self.mem.write_f32(a + 4 * k, v);
        }
    }
    /// 0x1036A0(a, b): a *= b (xyz).
    pub fn mul3(&mut self, a: u32, b: u32) {
        for k in 0..3u32 {
            let v = ee::mul(self.mem.read_f32(a + 4 * k), self.mem.read_f32(b + 4 * k));
            self.mem.write_f32(a + 4 * k, v);
        }
    }

    /// 0x103890(x): a transform record: scale (1, 1, 1, 1) +0, rotation 0
    /// +0x10, translation 0 +0x20.
    pub fn xform_init(&mut self, x: u32) {
        for k in 0..4 {
            self.mem.write_f32(x + 4 * k, 1.0);
            self.mem.write_u32(x + 0x10 + 4 * k, 0);
            self.mem.write_u32(x + 0x20 + 4 * k, 0);
        }
    }

    fn xform_parts(&mut self, x: u32) -> ([f32; 3], [i32; 3], [f32; 3]) {
        let s = std::array::from_fn(|k| self.mem.read_f32(x + 4 * k as u32));
        let r = std::array::from_fn(|k| self.mem.read_u32(x + 0x10 + 4 * k as u32) as i32);
        let t = std::array::from_fn(|k| self.mem.read_f32(x + 0x20 + 4 * k as u32));
        (s, r, t)
    }

    /// 0x1038D0(x): x+0x30 = TRS(x) x the camera's view-projection.
    pub fn xform_world(&mut self, x: u32) {
        let vp = self.camera_vp();
        self.write_m4(x + 0x30, &vp);
        let (s, r, t) = self.xform_parts(x);
        let m = self.trs(Some(s), Some(r), Some(t));
        self.mul_into(Some(x + 0x30), &m);
    }

    /// 0x205580 -> 0x1FC1C0(0, out): the camera object's angles as 16-bit
    /// (-0x1A2B80(32768 pitch / pi), 0x4000 - 0x1A2B80(32768 yaw / pi), 0,
    /// 0).
    pub fn camera_angles(&mut self) -> [i32; 4] {
        let cam = self.mem.read_u32(CAMERA_TABLE);
        if cam == 0 {
            return [0; 4];
        }
        let pi = f32::from_bits(0x4049_0FDB);
        let p = self.mem.read_f32(cam + 8);
        let y = self.mem.read_f32(cam + 4);
        let a0 = ee::fixsfsi(ee::div(ee::mul(32768.0, p), pi)).wrapping_neg();
        let a1 = 0x4000i32.wrapping_sub(ee::fixsfsi(ee::div(ee::mul(32768.0, y), pi)));
        [a0, a1, 0, 0]
    }

    /// 0x1039A0(x): a billboard: rotation (pitch, yaw & 0xFFFF, 0) from
    /// the camera, then x+0x30 as 0x1038D0; returns the raw yaw angle.
    pub fn xform_billboard(&mut self, x: u32) -> i32 {
        let a = self.camera_angles();
        self.mem.write_u32(x + 0x14, (a[1] & 0xFFFF) as u32);
        self.mem.write_u32(x + 0x10, a[0] as u32);
        self.mem.write_u32(x + 0x18, a[2] as u32);
        self.xform_world(x);
        a[1]
    }

    /// 0x202530(type, id): the placed RH2's address, 0 (reported) when the
    /// import has none.
    pub fn texture(&mut self, kind: i32, id: i32) -> u32 {
        match self.textures.get(&(kind, id)) {
            Some(&a) => a,
            None => {
                self.out.missing_textures.push((kind, id));
                0
            }
        }
    }

    /// 0x1032C0(p, cap, step): *p += step while *p <= cap - step (1), else
    /// *p = cap (0).
    pub fn step_up(&mut self, p: u32, cap: i32, step: i32) -> i32 {
        let cur = i32::from(self.mem.read_u8(p));
        if (cap & 0xFF) - (step & 0xFF) < cur {
            self.mem.write_u8(p, cap as u8);
            0
        } else {
            self.mem.write_u8(p, (cur + step) as u8);
            1
        }
    }
    /// 0x103470(p, floor, step): *p -= step while *p >= floor + step (1),
    /// else *p = floor (0).
    pub fn step_down(&mut self, p: u32, floor: i32, step: i32) -> i32 {
        let cur = i32::from(self.mem.read_u8(p));
        if cur < (floor & 0xFF) + (step & 0xFF) {
            self.mem.write_u8(p, floor as u8);
            0
        } else {
            self.mem.write_u8(p, (cur - step) as u8);
            1
        }
    }

    // ------------------------------------------------------------ descriptors

    /// 0x207E20(desc, alpha): ALPHA after the values, flag 0x200.
    pub fn desc_alpha(&mut self, d: u32, alpha: u64) {
        let n = u32::from(self.mem.read_u16(d));
        let f = self.mem.read_u16(d + 2);
        self.mem.write_u16(d + 2, f | 0x200);
        self.mem.write_u64(d + 0x10 + 8 * n, alpha);
    }

    /// 0x207E50(desc, rh2, filter): with flag 0x20 and a texture, TEX0 /
    /// TEX1 from the RH2 (+0x30 / +0x40) after ALPHA, TEX1's MMAG / MMIN
    /// (bits 5..8) from `filter` & 7.
    pub fn desc_texture(&mut self, d: u32, rh2: u32, filter: i32) {
        let f = self.mem.read_u16(d + 2);
        if f & 0x20 == 0 || rh2 == 0 {
            return;
        }
        let n = u32::from(self.mem.read_u16(d));
        let t0 = d + 0x10 + 8 * n;
        let tex0 = self.mem.read_u64(rh2 + 0x30);
        self.mem.write_u64(t0 + 8, tex0);
        let tex1 = self.mem.read_u64(rh2 + 0x40);
        self.mem.write_u64(t0 + 0x10, tex1);
        let h = self.mem.read_u16(t0 + 0x10);
        let h = (h & !0x1C0) | (((filter & 7) as u16) << 6);
        self.mem.write_u16(t0 + 0x10, h);
        let b = self.mem.read_u8(t0 + 0x10);
        // Bit 5 = bit 8 of the halfword (the low MMIN bit) [F 0x207EC0].
        let bit = ((u64::from(h) << 55) >> 61) as u8 & 1;
        self.mem.write_u8(t0 + 0x10, (b & !0x20) | (bit << 5));
    }

    /// 0x208400(desc, clamp): with flag 0x20, CLAMP after TEX1, flag 0x4000.
    pub fn desc_clamp(&mut self, d: u32, clamp: u64) {
        let f = self.mem.read_u16(d + 2);
        if f & 0x20 == 0 {
            return;
        }
        let n = u32::from(self.mem.read_u16(d));
        self.mem.write_u16(d + 2, f | 0x4000);
        self.mem.write_u64(d + 0x10 + 8 * n + 0x18, clamp);
    }

    /// 0x206F30(desc, ot, key): the primitive into the frame, linked into
    /// the ordering table at `key` (flag 0x8000: the last entry; keys
    /// clamped unsigned to 4094) [F 0x206FF0..0x207040], as the
    /// interpreter runner's hook records it.
    pub fn submit(&mut self, d: u32, ot: u32, key: i32) {
        self.submit_with(d, ot, key, false);
    }

    /// 0x206F90(packet, desc, ot, key) [F]: as [`Fx::submit`],
    /// the primitive written into the caller's packet (`buffered`, as the
    /// runner records it); returns `packet`.
    pub fn submit_at(&mut self, packet: u32, d: u32, ot: u32, key: i32) -> u32 {
        self.submit_with(d, ot, key, true);
        packet
    }

    fn submit_with(&mut self, d: u32, ot: u32, key: i32, buffered: bool) {
        if let Some(mut p) = world::decode_descriptor(&mut self.mem, d) {
            p.buffered = buffered;
            p.ot_key = (ot != 0).then(|| if p.flags & 0x8000 != 0 { OT_ENTRIES as i32 - 1 } else { (key as u32).min(OT_ENTRIES - 2) as i32 });
            p.drawn_buffer = p.textured() && p.tex0.is_some_and(|t| (t & 0x3FFF) as u16 == self.drawn_tbp);
            p.layer = self.layers_open.last().copied();
            if p.texture.is_none() && p.tex0.map(|t| (t & 0x3FFF) as u16) == Some(UNIT_RT_TBP) {
                p.texture = self.rt;
            }
            self.out.prims.push(p);
        }
    }
}

/// c.olt.s / c.ole.s with the EE's clamped operands.
pub fn ee_lt(a: f32, b: f32) -> bool {
    clampf(a) < clampf(b)
}
pub fn ee_le(a: f32, b: f32) -> bool {
    clampf(a) <= clampf(b)
}
fn clampf(x: f32) -> f32 {
    if x.is_nan() {
        if x.is_sign_negative() {
            -f32::MAX
        } else {
            f32::MAX
        }
    } else if x.is_infinite() {
        x.signum() * f32::MAX
    } else {
        x
    }
}

/// 0x104520(bits, n): bit `n` of `bits`.
pub fn bit(bits: u32, n: u32) -> bool {
    bits & (1 << n) != 0
}

impl Fx {
    /// 0x1F65B0: the self handle; 0x1F6590: the target handle.
    pub fn self_handle(&mut self) -> i32 {
        self.mem.i32(SELF_HANDLE)
    }
    pub fn target_handle(&mut self) -> i32 {
        self.mem.i32(TARGET_HANDLE)
    }

    /// 0x1E25E0(side, slot): the unit object's index, -1 when none (the
    /// interpreter runner answers it from the two mirrored roles).
    pub fn object_index(&mut self, side: i32, slot: i32) -> i32 {
        let h = (((side & 0xFF) << 8) | (slot & 0xFF)) as u16;
        match self.handles.iter().position(|x| *x == h) {
            Some(role) => i32::from(world::OBJECT_INDEX[role]),
            None => -1,
        }
    }

    /// The object index of a handle (side << 8 | slot), checked as 0x1F6770
    /// / 0x1F6830 / 0x1F71A0 check it (side 0..2, slot 0..10).
    fn handle_object(&mut self, handle: i32) -> Option<i32> {
        let (side, slot) = (handle >> 8, handle & 0xFF);
        if !(0..3).contains(&side) || slot >= 11 {
            return None;
        }
        let i = self.object_index(side, slot);
        (i >= 0).then_some(i)
    }

    /// The object record at index `i` (table 0x2DE2C0, 0x54 entries).
    fn object(&mut self, i: i32) -> Option<u32> {
        if !(0..0x54).contains(&i) {
            return None;
        }
        let o = self.mem.read_u32(world::OBJECT_TABLE + 4 * i as u32);
        (o != 0).then_some(o)
    }

    /// 0x1F6770(handle, node, clear): register a node-position request on
    /// the unit (the key carries the linked module type 0x2DF5A0 in bits
    /// 8..14); returns its request slot, -1 when refused (0x1F3E70).
    pub fn request_node(&mut self, handle: i32, node: i32, clear: i32) -> i32 {
        let Some(i) = self.handle_object(handle) else { return -1 };
        let key = if node >= 0 { ((self.mem.i32(world::LINKED_TYPE) << 8) & 0x7F00) | (node & 0xFF) } else { node };
        let Some(o) = self.object(i) else { return -1 };
        if clear != 0 {
            if key >= 0 {
                let kind = (key & 0x7F00) >> 8;
                for s in 0..8u32 {
                    if (self.mem.u16(o + 0x1B0 + 2 * s) & 0x7F00) >> 8 == kind {
                        self.mem.set16(o + 0x1B0 + 2 * s, 0x7FFF);
                    }
                }
            } else {
                for s in 0..8u32 {
                    self.mem.set16(o + 0x1B0 + 2 * s, 0x7FFF);
                }
            }
            self.mem.write_u8(o + 0x1F9, 0);
        }
        let n = self.mem.u8(o + 0x1F9);
        if n >= 8 {
            return -1;
        }
        if key >= 0 {
            let kind = (key & 0x7F00) >> 8;
            if (key & 0xFF) >= self.mem.u8(o + 0x1FE) {
                return -1;
            }
            let cur = self.mem.u16(o + 0x1B0 + 2 * n as u32);
            if (cur & 0x7FFF) < 0x7FFF && (cur & 0x7F00) >> 8 != kind {
                return -1;
            }
        }
        let key = if key < 0 { 0x7FFF } else { key };
        self.mem.set16(o + 0x1B0 + 2 * n as u32, key);
        self.mem.write_u8(o + 0x1F9, (n + 1) as u8);
        n
    }

    /// 0x1F6830(handle, slot, out): the request's node position (vec4)
    /// when the renderer has filled it (obj+0x20C bit 5, key bit 0x8000);
    /// else `out` = (0, 0, 0, 1) and 0 (0x1F40D0).
    pub fn node_position(&mut self, handle: i32, slot: i32, out: u32) -> bool {
        self.write_v4(out, [0.0, 0.0, 0.0, 1.0]);
        let Some(i) = self.handle_object(handle) else { return false };
        let Some(o) = self.object(i) else { return false };
        if self.mem.i8(o + 0x1F2) < 0 || self.mem.read_u8(o + 0x20C) & 0x20 == 0 || !(0..8).contains(&slot) {
            return false;
        }
        if self.mem.u16(o + 0x1B0 + 2 * slot as u32) & 0x8000 == 0 {
            return false;
        }
        let v = self.read_v4(o + 0xA0 + 16 * slot as u32);
        self.write_v4(out, v);
        true
    }

    /// 0x1F71A0(handle, out): the unit's position obj+0 (vec4), else (0, 0,
    /// 0, 1) (0x1F31E0).
    pub fn unit_position(&mut self, handle: i32, out: u32) -> bool {
        self.write_v4(out, [0.0, 0.0, 0.0, 1.0]);
        let Some(i) = self.handle_object(handle) else { return false };
        let Some(o) = self.object(i) else { return false };
        let v = self.read_v4(o);
        self.write_v4(out, v);
        true
    }

    /// 0x1F7F10(v): the stage light byte 0x2DF5B2.
    pub fn set_stage_light(&mut self, v: i32) {
        self.mem.write_u8(STAGE_LIGHT, v as u8);
    }

    /// 0x1F8100(): reported as the runner reports the boundary
    /// (a0..a3 at the jal), answered by the 0x1F8100 / 0x1F8160 / 0x1DF530
    /// head on the world's [`BattleState`] with the handle words 0x2DF5AC /
    /// 0x2DF5A8; without one 1, what the interpreter's zero image gives (both
    /// records card-less: ATK 0 = 0, flag 1 -> 4) [run D29]. The closure's
    /// dead frames below `sp` and its leftover registers are not written.
    pub fn outcome(&mut self, args: [u32; 4]) -> i32 {
        self.boundary(0x001F_8100, "outcome 0x1F8100", args);
        let Some(b) = self.battle else { return 1 };
        let (own, target) = (self.mem.read_u32(SELF_HANDLE), self.mem.read_u32(TARGET_HANDLE));
        b.outcome(own, target)
    }

    /// A boundary call the interpreter runner answers without running it,
    /// reported as it reports it (address, name, a0..a3) [the runner's
    /// BOUNDARY list].
    pub fn boundary(&mut self, pc: u32, name: &'static str, args: [u32; 4]) {
        self.out.boundary.push((pc, name, args));
    }
}
