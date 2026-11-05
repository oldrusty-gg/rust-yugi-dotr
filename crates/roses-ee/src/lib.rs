//! An R5900 (PS2 Emotion Engine) interpreter for running the original game's
//! own effect programs (BMODULE / CMODULE records) and the executable
//! helpers they call, from the user's disc, against a 32 MiB memory image.
//!
//! Scope: the integer core (MIPS III plus the EE's three-operand
//! multiplies and pipeline-1 HI/LO), the MMI subset the effect programs and
//! their helpers use, COP1 single precision and VU0 macro mode (COP2) with
//! the float behaviour of `roses_assets::opening_scene::ee_float` (PCSX2's
//! rounding, clamped overflow, flushed denormals), and a 16 KiB scratchpad
//! at 0x70000000. Interrupts, exceptions, the MMU, DMA and the GS are not
//! modelled; COP0 reads return a counter; `syscall` returns 0. Anything
//! else stops the run with [`Stop`] naming the instruction.
//!
//! Hooks: a caller-supplied function is asked at every control transfer
//! target (function entries are reached by `j`/`jal`/`jr`/`jalr`/branches)
//! whether to run the code there or to return at once with a value; this is
//! how the boundary (draw output, unit and camera state) is implemented by
//! the host.
//!
//! Derived from the research interpreter used to trace the game (Python,
//! `ee_emu_vu.py`); its semantics were checked against PCSX2 footage RAM.
use roses_assets::opening_scene::ee_float::{ee, vu};

pub mod bmodule;
pub mod effects;
pub mod fnparity;
pub mod parity;

pub const MAIN_BYTES: usize = 32 * 1024 * 1024;
pub const SCRATCH_BYTES: usize = 16 * 1024;
/// Return-address sentinel: [`Cpu::call`] returns when execution reaches it.
pub const RETURN_SENTINEL: u32 = 0x0DEA_DBE0;

/// Why a run stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stop(pub String);

impl std::fmt::Display for Stop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EE stop: {}", self.0)
    }
}
impl std::error::Error for Stop {}

/// What a hook does at a function entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hook {
    /// Run the original code.
    Run,
    /// Return to `ra` at once, setting `v0` when given.
    Return(Option<u64>),
    /// Continue at this address instead (the hook set up the registers).
    Jump(u32),
}

pub struct Cpu {
    pub mem: Vec<u8>,
    pub scratch: Vec<u8>,
    /// 128-bit GPRs as (low, high) 64-bit halves.
    pub gpr: [[u64; 2]; 32],
    pub hi: u64,
    pub lo: u64,
    pub hi1: u64,
    pub lo1: u64,
    pub sa: u64,
    pub fpr: [u32; 32],
    pub fcc: bool,
    pub facc: f32,
    pub vf: [[f32; 4]; 32],
    pub vi: [u16; 16],
    pub vacc: [f32; 4],
    pub q: f32,
    pub i: f32,
    /// Instructions executed by the last [`call`](Self::call).
    pub steps: u64,
    count: u32,
    /// Research: called before every load / store with (pc, address,
    /// store); `None` in normal runs.
    pub tracer: Option<Box<dyn FnMut(u32, u32, bool) + Send>>,
    /// The control transfer (branch / jump) that led to the pc passed to
    /// the current hook call; 0 at a call's start.
    pub from: u32,
    /// Research: instructions executed per word address (pc >> 2 within the
    /// 32 MiB image); `None` in normal runs.
    pub histogram: Option<Vec<u32>>,
    /// The memory pages written since [`journal_start`](Self::journal_start)
    /// with their contents before the first write (`None`: not kept).
    pub journal: Option<Journal>,
}

/// A copy of the register state ([`Cpu::regs`]).
#[derive(Clone, Debug)]
pub struct Regs {
    gpr: [[u64; 2]; 32],
    hilo: [u64; 5],
    fpr: [u32; 32],
    fcc: bool,
    facc: f32,
    vf: [[f32; 4]; 32],
    vi: [u16; 16],
    vacc: [f32; 4],
    qi: [f32; 2],
}

/// Bytes per journal page.
pub const JOURNAL_PAGE: usize = 4096;
const MAIN_PAGES: usize = MAIN_BYTES / JOURNAL_PAGE;
const ALL_PAGES: usize = MAIN_PAGES + SCRATCH_BYTES.div_ceil(JOURNAL_PAGE);

/// Pages written in one stretch of execution and their previous contents:
/// [`Cpu::journal_undo`] puts them back (rewinding the memory to the
/// stretch's start).
#[derive(Clone, Debug)]
pub struct Journal {
    pages: Vec<(u32, Box<[u8]>)>,
    marked: Vec<u64>,
}

impl Default for Journal {
    fn default() -> Self {
        Self { pages: Vec::new(), marked: vec![0; ALL_PAGES.div_ceil(64)] }
    }
}

impl Journal {
    /// Pages saved.
    pub fn len(&self) -> usize {
        self.pages.len()
    }
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }
}

const M32: u64 = 0xFFFF_FFFF;

fn se32(x: u64) -> u64 {
    x as u32 as i32 as i64 as u64
}

/// Clamp a value to the PS2's float range (no infinities, denormals to 0).
fn clamp(x: f64) -> f32 {
    if x.is_nan() {
        return 0.0;
    }
    if x >= f64::from(f32::MAX) {
        return f32::MAX;
    }
    if x <= -f64::from(f32::MAX) {
        return -f32::MAX;
    }
    let f = x as f32;
    if f != 0.0 && f.abs() < f32::MIN_POSITIVE {
        if f.is_sign_negative() {
            -0.0
        } else {
            0.0
        }
    } else {
        f
    }
}

/// A register value as the PS2 sees it (infinities and NaNs are never
/// produced; a stored bit pattern of one is read as the clamped extreme).
fn fin(x: f32) -> f32 {
    if x.is_nan() {
        if x.is_sign_negative() {
            -f32::MAX
        } else {
            f32::MAX
        }
    } else if x.is_infinite() {
        if x > 0.0 {
            f32::MAX
        } else {
            -f32::MAX
        }
    } else {
        x
    }
}

fn div_zero(a: f32, b: f32) -> f32 {
    if a.is_sign_negative() != b.is_sign_negative() {
        -f32::MAX
    } else {
        f32::MAX
    }
}

mod fpu {
    use super::{clamp, div_zero, ee, fin, vu};
    pub fn add(a: f32, b: f32) -> f32 {
        let (a, b) = (fin(a), fin(b));
        ee::add(a, b).unwrap_or_else(|_| clamp(f64::from(a) + f64::from(b)))
    }
    pub fn sub(a: f32, b: f32) -> f32 {
        let (a, b) = (fin(a), fin(b));
        ee::sub(a, b).unwrap_or_else(|_| clamp(f64::from(a) - f64::from(b)))
    }
    pub fn mul(a: f32, b: f32) -> f32 {
        let (a, b) = (fin(a), fin(b));
        ee::mul(a, b).unwrap_or_else(|_| clamp(f64::from(a) * f64::from(b)))
    }
    pub fn div(a: f32, b: f32) -> f32 {
        let (a, b) = (fin(a), fin(b));
        if b == 0.0 {
            return div_zero(a, b);
        }
        ee::div(a, b).unwrap_or_else(|_| clamp(f64::from(a) / f64::from(b)))
    }
    pub fn sqrt(b: f32) -> f32 {
        clamp(f64::from(fin(b).abs()).sqrt())
    }
    pub fn from_i32(v: i32) -> f32 {
        ee::from_i32(v).unwrap_or(v as f32)
    }
    pub fn to_i32(a: f32) -> i32 {
        let a = fin(a);
        if a >= 2_147_483_648.0 {
            i32::MAX
        } else if a <= -2_147_483_648.0 {
            i32::MIN
        } else {
            a.trunc() as i32
        }
    }
    pub fn vadd(a: f32, b: f32) -> f32 {
        let (a, b) = (fin(a), fin(b));
        vu::add(a, b).unwrap_or_else(|_| clamp(f64::from(a) + f64::from(b)))
    }
    pub fn vsub(a: f32, b: f32) -> f32 {
        let (a, b) = (fin(a), fin(b));
        vu::sub(a, b).unwrap_or_else(|_| clamp(f64::from(a) - f64::from(b)))
    }
    pub fn vmul(a: f32, b: f32) -> f32 {
        let (a, b) = (fin(a), fin(b));
        vu::mul(a, b).unwrap_or_else(|_| clamp(f64::from(a) * f64::from(b)))
    }
    pub fn vmadd(acc: f32, a: f32, b: f32) -> f32 {
        vadd(acc, vmul(a, b))
    }
    pub fn vmsub(acc: f32, a: f32, b: f32) -> f32 {
        vsub(acc, vmul(a, b))
    }
    pub fn vdiv(a: f32, b: f32) -> f32 {
        let (a, b) = (fin(a), fin(b));
        if b == 0.0 {
            return div_zero(a, b);
        }
        vu::div(a, b).unwrap_or_else(|_| clamp(f64::from(a) / f64::from(b)))
    }
    pub fn vrsqrt(a: f32, b: f32) -> f32 {
        let (a, b) = (fin(a), fin(b));
        if b == 0.0 {
            return div_zero(a, 1.0);
        }
        vu::rsqrt(a, b).unwrap_or_else(|_| clamp(f64::from(a) / f64::from(b.abs()).sqrt()))
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    pub fn new() -> Self {
        let mut vf = [[0.0; 4]; 32];
        vf[0] = [0.0, 0.0, 0.0, 1.0];
        Self {
            mem: vec![0; MAIN_BYTES],
            scratch: vec![0; SCRATCH_BYTES],
            gpr: [[0; 2]; 32],
            hi: 0,
            lo: 0,
            hi1: 0,
            lo1: 0,
            sa: 0,
            fpr: [0; 32],
            fcc: false,
            facc: 0.0,
            vf,
            vi: [0; 16],
            vacc: [0.0; 4],
            q: 0.0,
            i: 0.0,
            steps: 0,
            count: 0,
            tracer: None,
            from: 0,
            histogram: None,
            journal: None,
        }
    }

    /// The register file (for a rewind with [`journal_undo`](Self::journal_undo)).
    pub fn regs(&self) -> Regs {
        Regs {
            gpr: self.gpr,
            hilo: [self.hi, self.lo, self.hi1, self.lo1, self.sa],
            fpr: self.fpr,
            fcc: self.fcc,
            facc: self.facc,
            vf: self.vf,
            vi: self.vi,
            vacc: self.vacc,
            qi: [self.q, self.i],
        }
    }

    pub fn set_regs(&mut self, r: &Regs) {
        self.gpr = r.gpr;
        [self.hi, self.lo, self.hi1, self.lo1, self.sa] = r.hilo;
        self.fpr = r.fpr;
        self.fcc = r.fcc;
        self.facc = r.facc;
        self.vf = r.vf;
        self.vi = r.vi;
        self.vacc = r.vacc;
        [self.q, self.i] = r.qi;
    }

    /// Start keeping the pages written from now on (a fresh journal).
    pub fn journal_start(&mut self) {
        self.journal = Some(Journal::default());
    }

    /// The journal so far, a fresh one continuing (`None` when not kept).
    pub fn journal_take(&mut self) -> Option<Journal> {
        let j = self.journal.as_mut()?;
        Some(std::mem::take(j))
    }

    /// Put back the pages of `j` (taken in order, undone newest first). The
    /// running journal is not touched.
    pub fn journal_undo(&mut self, j: Journal) {
        for (page, bytes) in j.pages.into_iter().rev() {
            let page = page as usize;
            if page < MAIN_PAGES {
                self.mem[page * JOURNAL_PAGE..(page + 1) * JOURNAL_PAGE].copy_from_slice(&bytes);
            } else {
                let at = (page - MAIN_PAGES) * JOURNAL_PAGE;
                let end = (at + JOURNAL_PAGE).min(SCRATCH_BYTES);
                self.scratch[at..end].copy_from_slice(&bytes[..end - at]);
            }
        }
    }

    #[inline]
    fn touch(&mut self, addr: u32) {
        let Some(j) = self.journal.as_mut() else { return };
        let page = if addr >> 28 == 7 {
            MAIN_PAGES + (addr as usize & (SCRATCH_BYTES - 1)) / JOURNAL_PAGE
        } else {
            (addr as usize & (MAIN_BYTES - 1)) / JOURNAL_PAGE
        };
        let (w, b) = (page / 64, 1u64 << (page % 64));
        if j.marked[w] & b != 0 {
            return;
        }
        j.marked[w] |= b;
        let bytes: Box<[u8]> = if page < MAIN_PAGES {
            self.mem[page * JOURNAL_PAGE..(page + 1) * JOURNAL_PAGE].into()
        } else {
            let at = (page - MAIN_PAGES) * JOURNAL_PAGE;
            self.scratch[at..(at + JOURNAL_PAGE).min(SCRATCH_BYTES)].into()
        };
        j.pages.push((page as u32, bytes));
    }

    // ------------------------------------------------------------ memory

    #[inline]
    fn slot(&mut self, addr: u32) -> (&mut [u8], usize) {
        if addr >> 28 == 7 {
            (&mut self.scratch, (addr & (SCRATCH_BYTES as u32 - 1)) as usize)
        } else {
            (&mut self.mem, (addr & (MAIN_BYTES as u32 - 1)) as usize)
        }
    }

    pub fn read_u8(&mut self, addr: u32) -> u8 {
        let (m, a) = self.slot(addr);
        m[a]
    }
    pub fn write_u8(&mut self, addr: u32, v: u8) {
        if self.journal.is_some() {
            self.touch(addr);
        }
        let (m, a) = self.slot(addr);
        m[a] = v;
    }
    fn read_n<const N: usize>(&mut self, addr: u32) -> [u8; N] {
        let (m, a) = self.slot(addr);
        let mut out = [0; N];
        if a + N <= m.len() {
            out.copy_from_slice(&m[a..a + N]);
        }
        out
    }
    fn write_n<const N: usize>(&mut self, addr: u32, bytes: [u8; N]) {
        if self.journal.is_some() {
            self.touch(addr);
        }
        let (m, a) = self.slot(addr);
        if a + N <= m.len() {
            m[a..a + N].copy_from_slice(&bytes);
        }
    }
    pub fn read_u16(&mut self, addr: u32) -> u16 {
        u16::from_le_bytes(self.read_n(addr & !1))
    }
    pub fn read_u32(&mut self, addr: u32) -> u32 {
        u32::from_le_bytes(self.read_n(addr & !3))
    }
    pub fn read_u64(&mut self, addr: u32) -> u64 {
        u64::from_le_bytes(self.read_n(addr & !7))
    }
    pub fn read_f32(&mut self, addr: u32) -> f32 {
        f32::from_bits(self.read_u32(addr))
    }
    pub fn write_u16(&mut self, addr: u32, v: u16) {
        self.write_n(addr & !1, v.to_le_bytes());
    }
    pub fn write_u32(&mut self, addr: u32, v: u32) {
        self.write_n(addr & !3, v.to_le_bytes());
    }
    pub fn write_u64(&mut self, addr: u32, v: u64) {
        self.write_n(addr & !7, v.to_le_bytes());
    }
    pub fn write_f32(&mut self, addr: u32, v: f32) {
        self.write_u32(addr, v.to_bits());
    }
    /// Copy `bytes` into memory at `addr`.
    pub fn load(&mut self, addr: u32, bytes: &[u8]) {
        for (i, b) in bytes.iter().enumerate() {
            self.write_u8(addr.wrapping_add(i as u32), *b);
        }
    }

    /// Load an ELF executable's PT_LOAD segments (file bytes; the rest of
    /// each segment stays zero); returns the entry point.
    pub fn load_elf(&mut self, elf: &[u8]) -> Result<u32, Stop> {
        let u16_at = |o: usize| elf.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
        let u32_at = |o: usize| elf.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        if elf.get(..4) != Some(b"ELF") {
            return Err(Stop("not an ELF executable".into()));
        }
        let bad = || Stop("truncated ELF header".into());
        let entry = u32_at(0x18).ok_or_else(bad)?;
        let phoff = u32_at(0x1C).ok_or_else(bad)? as usize;
        let size = usize::from(u16_at(0x2A).ok_or_else(bad)?);
        let count = usize::from(u16_at(0x2C).ok_or_else(bad)?);
        for i in 0..count {
            let h = phoff + i * size;
            let (kind, offset, vaddr, filesz) = (
                u32_at(h).ok_or_else(bad)?,
                u32_at(h + 4).ok_or_else(bad)? as usize,
                u32_at(h + 8).ok_or_else(bad)?,
                u32_at(h + 16).ok_or_else(bad)? as usize,
            );
            if kind == 1 && filesz > 0 {
                let bytes = elf.get(offset..offset + filesz).ok_or_else(|| Stop("ELF segment outside the file".into()))?;
                self.load(vaddr, bytes);
            }
        }
        Ok(entry)
    }

    // ------------------------------------------------------------ native helpers

    /// 0x20ABF0 (`cosine`) / 0x20ACC0: a 16-bit angle's cosine / sine from
    /// the executable's quarter table at 0x293430, as their code computes
    /// it [F]: fold to 0..0x4000 with the sign in f0, then (T[i] (16 - r) +
    /// T[i + 1] r) * sign / 16 with the EE's mula / madd rounding. Sets f0.
    pub fn native_table_trig(&mut self, angle: u32, cosine: bool) {
        let mut a = i64::from(angle.wrapping_add(if cosine { 0x4000 } else { 0 }) as u16 as i16);
        let mut sign = 1.0f32;
        if a < 0 {
            sign = -1.0;
            a = -a;
        }
        if a >= 0x4001 {
            a = 0x8000 - a;
        }
        let r = if a & 0x3FFF == 0 {
            if a == 0x4000 { sign } else { 0.0 }
        } else {
            let at = 0x0029_3430 + 4 * (a >> 4) as u32;
            let (t0, t1) = (self.read_f32(at), self.read_f32(at + 4));
            let frac = (a & 15) as i32;
            self.facc = fpu::mul(t0, fpu::from_i32(16 - frac));
            let m = fpu::add(self.facc, fpu::mul(t1, fpu::from_i32(frac)));
            fpu::div(fpu::mul(sign, m), 16.0)
        };
        self.fpr[0] = r.to_bits();
    }

    /// 0x205210(in, out, matrix, desc, mean_w) [F]: projects a primitive
    /// descriptor's XYZF2 vertices in place. Each XYZF2 input vector (vec4
    /// array `in`, w set to 1.0) is transformed by `matrix` (the current
    /// view-projection when 0) into `out` with VU0 (ACC = M0 x; ACC += M1 y;
    /// ACC += M2 z; v = ACC + M3 w); then per XYZF2 register in list order q
    /// = 1 / w, the preceding RGBAQ's Q and ST scaled by q when the
    /// descriptor is ST-textured with IIP (flags 0x20 | 0x10, no 0x800), x y
    /// z scaled by q, the primitive rejected (v0 = 0) when z < 1, z >
    /// 16777215, x or y outside [0, 4095]; X / Y = 16 x / 16 y and Z = z
    /// through 0x1A2C70 into the register value (F kept). The mean w goes to
    /// `mean_w` when not 0. Returns v0. Uses the interpreter's own float
    /// rules (`fpu`), so its results equal the interpreted code's.
    pub fn native_project(&mut self, input: u32, out: u32, matrix: u32, desc: u32, mean_w: u32) -> u64 {
        let flags = self.read_u16(desc + 2);
        let n = u32::from(self.read_u16(desc));
        let regs = self.read_u64(desc + 8);
        let perspective_st = flags & 0x800 == 0 && flags & 0x20 != 0 && flags & 0x10 != 0;
        // The original's 0x120-byte frame: saved registers, locals.
        let frame = (self.gpr[29][0] as u32).wrapping_sub(0x120);
        for (reg, at) in [(31, 0xA0), (30, 0x90), (23, 0x80), (22, 0x70), (21, 0x60), (20, 0x50), (19, 0x40), (18, 0x30), (17, 0x20), (16, 0x10)] {
            let [lo, hi] = self.gpr[reg];
            self.write_u64(frame + at, lo);
            self.write_u64(frame + at + 8, hi);
        }
        self.write_u32(frame, self.fpr[20]);
        self.write_u32(frame + 0xD0, input);
        self.write_u32(frame + 0xB0, desc + 8);
        self.write_u32(frame + 0xCC, mean_w);
        // Input w = 1.0 for every XYZF2 vertex.
        let mut count = 0u32;
        for i in 0..n {
            if (regs >> (4 * i)) & 15 == 4 {
                self.write_u32(input + 16 * count + 12, 0x3F80_0000);
                count += 1;
            }
        }
        // The matrix: given, or a copy of the current view-projection.
        let m: [[f32; 4]; 4] = if matrix != 0 {
            std::array::from_fn(|r| std::array::from_fn(|k| self.read_f32(((matrix + 16 * r as u32) & !15) + 4 * k as u32)))
        } else {
            // 0x20E2B0 copies it to the frame (+0xE0).
            let idx = self.read_u32(0x002E_D6C0);
            let base = 0x002E_D720u32.wrapping_add(idx.wrapping_mul(0x1A0));
            for k in 0..16 {
                let w = self.read_u32(base + 4 * k);
                self.write_u32(frame + 0xE0 + 4 * k, w);
            }
            std::array::from_fn(|r| std::array::from_fn(|k| self.read_f32(base + 16 * r as u32 + 4 * k as u32)))
        };
        // 0x20B260(matrix) / 0x20B2C0 around the transform: the stack keeps
        // vf28-vf31 (memory side effect kept), the registers end unchanged.
        let saved = [self.vf[28], self.vf[29], self.vf[30], self.vf[31]];
        self.native_matrix_push(0);
        for k in 0..count {
            let v: [f32; 4] = std::array::from_fn(|j| self.read_f32(((input + 16 * k) & !15) + 4 * j as u32));
            let mut acc: [f32; 4] = std::array::from_fn(|j| fpu::vmul(m[0][j], v[0]));
            for j in 0..4 {
                acc[j] = fpu::vmadd(acc[j], m[1][j], v[1]);
            }
            for j in 0..4 {
                acc[j] = fpu::vmadd(acc[j], m[2][j], v[2]);
            }
            let o: [f32; 4] = std::array::from_fn(|j| fpu::vmadd(acc[j], m[3][j], v[3]));
            for j in 0..4 {
                self.write_f32(((out + 16 * k) & !15) + 4 * j as u32, o[j]);
            }
        }
        // The last vertex leaves vf8 (input), vf9 (output) and ACC.
        if let Some(k) = count.checked_sub(1) {
            let v: [f32; 4] = std::array::from_fn(|j| self.read_f32(((input + 16 * k) & !15) + 4 * j as u32));
            let o: [f32; 4] = std::array::from_fn(|j| self.read_f32(((out + 16 * k) & !15) + 4 * j as u32));
            let mut acc: [f32; 4] = std::array::from_fn(|j| fpu::vmul(m[0][j], v[0]));
            for j in 0..4 {
                acc[j] = fpu::vmadd(acc[j], m[1][j], v[1]);
            }
            for j in 0..4 {
                acc[j] = fpu::vmadd(acc[j], m[2][j], v[2]);
            }
            self.vf[8] = v;
            self.vf[9] = o;
            self.vacc = acc;
        }
        self.native_matrix_pop();
        self.vf[28..32].copy_from_slice(&saved);
        let (mut rgbaq, mut st): (Option<u32>, Option<u32>) = (None, None);
        let mut sum_w = 0.0f32;
        let mut v = out;
        for i in 0..n {
            let value = desc + 0x10 + 8 * i;
            match (regs >> (4 * i)) & 15 {
                4 => {
                    let w = self.read_f32(v + 12);
                    let q = fpu::div(1.0, w);
                    if perspective_st {
                        if let (Some(c), Some(t)) = (rgbaq, st) {
                            self.write_f32(c + 4, q);
                            let s0 = fpu::mul(self.read_f32(t), q);
                            self.write_f32(t, s0);
                            let t0 = fpu::mul(self.read_f32(t + 4), q);
                            self.write_f32(t + 4, t0);
                            rgbaq = None;
                            st = None;
                        }
                    }
                    let x = fpu::mul(self.read_f32(v), q);
                    self.write_f32(v, x);
                    let y = fpu::mul(self.read_f32(v + 4), q);
                    self.write_f32(v + 4, y);
                    let z = fpu::mul(self.read_f32(v + 8), q);
                    self.write_f32(v + 8, z);
                    let (one, zmax, xmax) = (1.0f32, f32::from_bits(0x4B7F_FFFF), f32::from_bits(0x457F_F000));
                    if fin(z) < one || !(fin(z) <= zmax) || fin(x) < 0.0 || !(fin(x) <= xmax) || fin(y) < 0.0 || !(fin(y) <= xmax) {
                        return 0;
                    }
                    let x16 = fpu::mul(16.0, x).to_bits();
                    self.native_fix_frame(frame, x16, 0x0020_54A4);
                    self.write_u16(value, fixunssfsi(x16) as u16);
                    let y16 = fpu::mul(16.0, y).to_bits();
                    self.native_fix_frame(frame, y16, 0x0020_54BC);
                    self.write_u16(value + 2, fixunssfsi(y16) as u16);
                    self.native_fix_frame(frame, z.to_bits(), 0x0020_54C8);
                    let fz = u64::from(fixunssfsi(z.to_bits()) & 0xFF_FFFF);
                    let old = self.read_u64(value);
                    self.write_u64(value, (old & 0xFF00_0000_FFFF_FFFF) | (fz << 32));
                    sum_w = fpu::add(sum_w, w);
                    v += 16;
                }
                2 => st = Some(value),
                1 => rgbaq = Some(value),
                _ => {}
            }
        }
        if mean_w != 0 {
            let mean = fpu::div(sum_w, fpu::from_i32(count as i32));
            self.write_f32(mean_w, mean);
        }
        1
    }

    /// The stack writes of 0x1A2C70 / 0x1A2B80 (soft-float conversions of
    /// f12, called with the stack at `sp` and return address `ra`) [F]: a
    /// 0x30-byte frame with f12 at +0x10, ra at +0x20 and 0x1A2B20's unpacked
    /// float at +0 (class 2 for a zero exponent field, else 3; sign +4;
    /// exponent - 127 +8; mantissa << 7 | 1 << 30 +0xC). Native callers
    /// keep these so later reads of stale stack see what the original left.
    pub fn native_fix_frame(&mut self, sp: u32, bits: u32, ra: u64) {
        let f = sp.wrapping_sub(0x30);
        self.write_u32(f + 0x10, bits);
        self.write_u64(f + 0x20, ra);
        self.write_u32(f + 4, bits >> 31);
        let field = (bits >> 23) & 0xFF;
        if field == 0 {
            self.write_u32(f, 2);
        } else {
            self.write_u32(f + 8, field.wrapping_sub(127));
            self.write_u32(f, 3);
            self.write_u32(f + 0xC, ((bits & 0x7F_FFFF) << 7) | 0x4000_0000);
        }
    }

    /// 0x20B260(matrix): push VU0 vf28-vf31 on the stack at 0x2E9EA0
    /// (index 0x2ED6A0, 0x70-byte records) and load `matrix` when not 0 [F].
    pub fn native_matrix_push(&mut self, matrix: u32) {
        let idx = self.read_u32(0x002E_D6A0);
        let base = 0x002E_9EA0u32.wrapping_add(idx.wrapping_mul(0x70));
        for r in 0..4 {
            for k in 0..4 {
                let v = self.vf[28 + r][k];
                self.write_f32((base + 16 * r as u32) & !15 | (4 * k as u32), v);
            }
        }
        if matrix != 0 {
            for r in 0..4 {
                for k in 0..4 {
                    self.vf[28 + r][k] = self.read_f32(((matrix + 16 * r as u32) & !15) + 4 * k as u32);
                }
            }
        }
        self.write_u32(0x002E_D6A0, idx.wrapping_add(1));
    }

    /// 0x20B2C0: pop vf28-vf31 from that stack [F].
    pub fn native_matrix_pop(&mut self) {
        let idx = self.read_u32(0x002E_D6A0).wrapping_sub(1);
        self.write_u32(0x002E_D6A0, idx);
        let base = 0x002E_9EA0u32.wrapping_add(idx.wrapping_mul(0x70));
        for r in 0..4 {
            for k in 0..4 {
                self.vf[28 + r][k] = self.read_f32(((base + 16 * r as u32) & !15) + 4 * k as u32);
            }
        }
    }

    // ------------------------------------------------------------ registers

    #[inline]
    pub fn reg(&self, r: usize) -> u64 {
        self.gpr[r][0]
    }
    #[inline]
    pub fn set_reg(&mut self, r: usize, v: u64) {
        if r != 0 {
            self.gpr[r][0] = v;
        }
    }
    fn set_reg128(&mut self, r: usize, lo: u64, hi: u64) {
        if r != 0 {
            self.gpr[r] = [lo, hi];
        }
    }
    fn f(&self, r: usize) -> f32 {
        f32::from_bits(self.fpr[r])
    }
    fn set_f(&mut self, r: usize, v: f32) {
        self.fpr[r] = v.to_bits();
    }

    /// Call the function at `pc` with integer arguments in a0..t3 and the
    /// stack at `sp`; returns v0 when execution returns to the sentinel.
    pub fn call(
        &mut self,
        pc: u32,
        args: &[u64],
        sp: u32,
        max_steps: u64,
        hooks: &mut dyn FnMut(&mut Cpu, u32) -> Hook,
    ) -> Result<u64, Stop> {
        self.gpr = [[0; 2]; 32];
        for (i, a) in args.iter().take(8).enumerate() {
            self.gpr[4 + i][0] = *a;
        }
        self.gpr[29][0] = u64::from(sp);
        self.gpr[31][0] = u64::from(RETURN_SENTINEL);
        self.run(pc, max_steps, hooks)
    }

    /// Run from `pc` until the sentinel is reached.
    pub fn run(
        &mut self,
        mut pc: u32,
        max_steps: u64,
        hooks: &mut dyn FnMut(&mut Cpu, u32) -> Hook,
    ) -> Result<u64, Stop> {
        self.steps = 0;
        self.from = 0;
        let mut delay: Option<u32> = None;
        let mut entered = true;
        loop {
            if delay.is_none() {
                if pc == RETURN_SENTINEL {
                    return Ok(self.gpr[2][0]);
                }
                if entered {
                    entered = false;
                    match hooks(self, pc) {
                        Hook::Return(v) => {
                            if let Some(v) = v {
                                self.gpr[2][0] = v;
                            }
                            pc = self.gpr[31][0] as u32;
                            entered = true;
                            continue;
                        }
                        Hook::Jump(target) => {
                            pc = target;
                            entered = true;
                            continue;
                        }
                        Hook::Run => {}
                    }
                }
            }
            self.steps += 1;
            if self.steps > max_steps {
                return Err(Stop(format!("{max_steps} steps reached at {pc:#x}")));
            }
            let w = self.read_u32(pc);
            if self.tracer.is_some() {
                self.trace_access(pc, w);
            }
            if let Some(h) = &mut self.histogram {
                h[((pc >> 2) & 0x7F_FFFF) as usize] += 1;
            }
            let next = self.step(pc, w)?;
            self.gpr[0] = [0, 0];
            if let Some(target) = delay.take() {
                self.from = pc.wrapping_sub(4);
                pc = target;
                entered = true;
                continue;
            }
            match next {
                Next::Seq => pc = pc.wrapping_add(4),
                Next::Branch(t) => {
                    delay = Some(t);
                    pc = pc.wrapping_add(4);
                }
                Next::SkipDelay => pc = pc.wrapping_add(8),
            }
        }
    }

    fn trace_access(&mut self, pc: u32, w: u32) {
        let op = w >> 26;
        let store = matches!(op, 0x1F | 0x28..=0x2E | 0x39 | 0x3E | 0x3F);
        if !(store || matches!(op, 0x1A | 0x1B | 0x1E | 0x20..=0x27 | 0x31 | 0x36 | 0x37)) {
            return;
        }
        let rs = ((w >> 21) & 31) as usize;
        let a = self.gpr[rs][0].wrapping_add((w & 0xFFFF) as u16 as i16 as i64 as u64) as u32;
        if let Some(mut t) = self.tracer.take() {
            t(pc, a, store);
            self.tracer = Some(t);
        }
    }

    fn step(&mut self, pc: u32, w: u32) -> Result<Next, Stop> {
        let op = w >> 26;
        let rs = ((w >> 21) & 31) as usize;
        let rt = ((w >> 16) & 31) as usize;
        let rd = ((w >> 11) & 31) as usize;
        let sa = (w >> 6) & 31;
        let fn_ = w & 63;
        let imm = w & 0xFFFF;
        let si = imm as u16 as i16 as i64;
        let bt = pc.wrapping_add(4).wrapping_add((si << 2) as u32);
        let stop = |what: &str| Err(Stop(format!("{what} at {pc:#x} ({w:08x})")));
        let r = |c: &Cpu, i: usize| c.gpr[i][0];
        let addr = |c: &Cpu| r(c, rs).wrapping_add(si as u64) as u32;
        let cond = |taken: bool, likely: bool| {
            if taken {
                Next::Branch(bt)
            } else if likely {
                Next::SkipDelay
            } else {
                Next::Seq
            }
        };
        match op {
            0x00 => return self.special(pc, w, rs, rt, rd, sa, fn_),
            0x01 => {
                let v = r(self, rs) as i64;
                let (taken, likely, link) = match rt {
                    0x00 => (v < 0, false, false),
                    0x01 => (v >= 0, false, false),
                    0x02 => (v < 0, true, false),
                    0x03 => (v >= 0, true, false),
                    0x10 => (v < 0, false, true),
                    0x11 => (v >= 0, false, true),
                    0x12 => (v < 0, true, true),
                    0x13 => (v >= 0, true, true),
                    0x18 | 0x19 => return Ok(Next::Seq), // mtsab / mtsah
                    _ => return stop("REGIMM"),
                };
                if link {
                    self.set_reg(31, u64::from(pc.wrapping_add(8)));
                }
                return Ok(cond(taken, likely));
            }
            0x02 => return Ok(Next::Branch(((w & 0x03FF_FFFF) << 2) | (pc & 0xF000_0000))),
            0x03 => {
                self.set_reg(31, u64::from(pc.wrapping_add(8)));
                return Ok(Next::Branch(((w & 0x03FF_FFFF) << 2) | (pc & 0xF000_0000)));
            }
            0x04 | 0x14 => return Ok(cond(r(self, rs) == r(self, rt), op == 0x14)),
            0x05 | 0x15 => return Ok(cond(r(self, rs) != r(self, rt), op == 0x15)),
            0x06 | 0x16 => return Ok(cond(r(self, rs) as i64 <= 0, op == 0x16)),
            0x07 | 0x17 => return Ok(cond(r(self, rs) as i64 > 0, op == 0x17)),
            0x08 | 0x09 => self.set_reg(rt, se32(r(self, rs).wrapping_add(si as u64))),
            0x0A => self.set_reg(rt, u64::from((r(self, rs) as i64) < si)),
            0x0B => self.set_reg(rt, u64::from(r(self, rs) < si as u64)),
            0x0C => self.set_reg(rt, r(self, rs) & u64::from(imm)),
            0x0D => self.set_reg(rt, r(self, rs) | u64::from(imm)),
            0x0E => self.set_reg(rt, r(self, rs) ^ u64::from(imm)),
            0x0F => self.set_reg(rt, se32(u64::from(imm) << 16)),
            0x10 => {
                // COP0: mfc0 reads a counter, the rest is ignored.
                if rs == 0 {
                    self.count = self.count.wrapping_add(1000);
                    let v = if rd == 9 { self.count } else { 0 };
                    self.set_reg(rt, se32(u64::from(v)));
                }
            }
            0x11 => return self.cop1(pc, w, rs, rt, rd, sa, fn_, bt),
            0x12 => self.cop2(pc, w, rs, rt)?,
            0x18 | 0x19 => self.set_reg(rt, r(self, rs).wrapping_add(si as u64)),
            0x1A | 0x1B => {
                // ldl / ldr (little endian), byte by byte.
                let a = addr(self);
                let o = (a & 7) as usize;
                let mut v = r(self, rt).to_le_bytes();
                if op == 0x1A {
                    for i in 0..=o {
                        v[7 - i] = self.read_u8(a.wrapping_sub(i as u32));
                    }
                } else {
                    for i in 0..(8 - o) {
                        v[i] = self.read_u8(a.wrapping_add(i as u32));
                    }
                }
                self.set_reg(rt, u64::from_le_bytes(v));
            }
            0x1C => return self.mmi(pc, w, rs, rt, rd, sa, fn_),
            0x1E => {
                let a = addr(self) & !15;
                let (lo, hi) = (self.read_u64(a), self.read_u64(a + 8));
                self.set_reg128(rt, lo, hi);
            }
            0x1F => {
                let a = addr(self) & !15;
                let [lo, hi] = self.gpr[rt];
                self.write_u64(a, lo);
                self.write_u64(a + 8, hi);
            }
            0x20 => {
                let a = addr(self);
                let v = self.read_u8(a) as i8 as i64 as u64;
                self.set_reg(rt, v);
            }
            0x21 => {
                let a = addr(self);
                let v = self.read_u16(a) as i16 as i64 as u64;
                self.set_reg(rt, v);
            }
            0x22 | 0x26 => {
                // lwl / lwr
                let a = addr(self);
                let o = (a & 3) as usize;
                let mut v = (r(self, rt) as u32).to_le_bytes();
                if op == 0x22 {
                    for i in 0..=o {
                        v[3 - i] = self.read_u8(a.wrapping_sub(i as u32));
                    }
                } else {
                    for i in 0..(4 - o) {
                        v[i] = self.read_u8(a.wrapping_add(i as u32));
                    }
                }
                let v = u32::from_le_bytes(v);
                if op == 0x26 && o != 0 {
                    let keep = r(self, rt) & !M32;
                    self.set_reg(rt, keep | u64::from(v));
                } else {
                    self.set_reg(rt, se32(u64::from(v)));
                }
            }
            0x23 => {
                let a = addr(self);
                let v = se32(u64::from(self.read_u32(a)));
                self.set_reg(rt, v);
            }
            0x24 => {
                let a = addr(self);
                let v = u64::from(self.read_u8(a));
                self.set_reg(rt, v);
            }
            0x25 => {
                let a = addr(self);
                let v = u64::from(self.read_u16(a));
                self.set_reg(rt, v);
            }
            0x27 => {
                let a = addr(self);
                let v = u64::from(self.read_u32(a));
                self.set_reg(rt, v);
            }
            0x28 => {
                let a = addr(self);
                self.write_u8(a, r(self, rt) as u8);
            }
            0x29 => {
                let a = addr(self);
                self.write_u16(a, r(self, rt) as u16);
            }
            0x2A | 0x2E => {
                // swl / swr
                let a = addr(self);
                let o = (a & 3) as usize;
                let v = (r(self, rt) as u32).to_le_bytes();
                if op == 0x2A {
                    for i in 0..=o {
                        self.write_u8(a.wrapping_sub(i as u32), v[3 - i]);
                    }
                } else {
                    for i in 0..(4 - o) {
                        self.write_u8(a.wrapping_add(i as u32), v[i]);
                    }
                }
            }
            0x2B => {
                let a = addr(self);
                self.write_u32(a, r(self, rt) as u32);
            }
            0x2C | 0x2D => {
                // sdl / sdr
                let a = addr(self);
                let o = (a & 7) as usize;
                let v = r(self, rt).to_le_bytes();
                if op == 0x2C {
                    for i in 0..=o {
                        self.write_u8(a.wrapping_sub(i as u32), v[7 - i]);
                    }
                } else {
                    for i in 0..(8 - o) {
                        self.write_u8(a.wrapping_add(i as u32), v[i]);
                    }
                }
            }
            0x2F | 0x33 => {} // cache, pref
            0x31 => {
                let a = addr(self);
                self.fpr[rt] = self.read_u32(a);
            }
            0x36 => {
                // lqc2
                let a = addr(self) & !15;
                if rt != 0 {
                    for k in 0..4 {
                        self.vf[rt][k] = self.read_f32(a + 4 * k as u32);
                    }
                }
            }
            0x37 => {
                let a = addr(self);
                let v = self.read_u64(a);
                self.set_reg(rt, v);
            }
            0x39 => {
                let a = addr(self);
                self.write_u32(a, self.fpr[rt]);
            }
            0x3E => {
                // sqc2
                let a = addr(self) & !15;
                for k in 0..4 {
                    let v = self.vf[rt][k];
                    self.write_f32(a + 4 * k as u32, v);
                }
            }
            0x3F => {
                let a = addr(self);
                self.write_u64(a, r(self, rt));
            }
            _ => return stop("opcode"),
        }
        Ok(Next::Seq)
    }

    #[allow(clippy::too_many_arguments)]
    fn special(&mut self, pc: u32, w: u32, rs: usize, rt: usize, rd: usize, sa: u32, fn_: u32) -> Result<Next, Stop> {
        let s = self.gpr[rs][0];
        let t = self.gpr[rt][0];
        let v: Option<u64> = match fn_ {
            0x00 => Some(se32((t << sa) & M32)),
            0x02 => Some(se32((t & M32) >> sa)),
            0x03 => Some(((t as u32 as i32) >> sa) as i64 as u64),
            0x04 => Some(se32((t << (s & 31)) & M32)),
            0x06 => Some(se32((t & M32) >> (s & 31))),
            0x07 => Some(((t as u32 as i32) >> (s & 31)) as i64 as u64),
            0x08 => return Ok(Next::Branch(s as u32)),
            0x09 => {
                self.set_reg(rd, u64::from(pc.wrapping_add(8)));
                return Ok(Next::Branch(s as u32));
            }
            0x0A => (t == 0).then_some(s),
            0x0B => (t != 0).then_some(s),
            0x0C => {
                self.gpr[2][0] = 0;
                None
            }
            0x0D => return Err(Stop(format!("break at {pc:#x}"))),
            0x0F => None,
            0x10 => Some(self.hi),
            0x11 => {
                self.hi = s;
                None
            }
            0x12 => Some(self.lo),
            0x13 => {
                self.lo = s;
                None
            }
            0x14 => Some(t << (s & 63)),
            0x16 => Some(t >> (s & 63)),
            0x17 => Some(((t as i64) >> (s & 63)) as u64),
            0x18 => {
                let p = i64::from(s as u32 as i32) * i64::from(t as u32 as i32);
                self.lo = se32(p as u64 & M32);
                self.hi = se32((p >> 32) as u64 & M32);
                Some(self.lo)
            }
            0x19 => {
                let p = (s & M32) * (t & M32);
                self.lo = se32(p & M32);
                self.hi = se32(p >> 32);
                Some(self.lo)
            }
            0x1A => {
                let (a, b) = (s as u32 as i32, t as u32 as i32);
                if b != 0 {
                    let q = a.wrapping_div(b);
                    self.lo = q as i64 as u64;
                    self.hi = a.wrapping_rem(b) as i64 as u64;
                }
                None
            }
            0x1B => {
                let (a, b) = (s as u32, t as u32);
                if b != 0 {
                    self.lo = se32(u64::from(a / b));
                    self.hi = se32(u64::from(a % b));
                }
                None
            }
            0x20 | 0x21 => Some(se32(s.wrapping_add(t) & M32)),
            0x22 | 0x23 => Some(se32(s.wrapping_sub(t) & M32)),
            0x24 => Some(s & t),
            0x25 => Some(s | t),
            0x26 => Some(s ^ t),
            0x27 => Some(!(s | t)),
            0x28 => Some(self.sa),
            0x29 => {
                self.sa = s;
                None
            }
            0x2A => Some(u64::from((s as i64) < (t as i64))),
            0x2B => Some(u64::from(s < t)),
            0x2C | 0x2D => Some(s.wrapping_add(t)),
            0x2E | 0x2F => Some(s.wrapping_sub(t)),
            0x30..=0x36 => None, // traps: never taken here
            0x38 => Some(t << sa),
            0x3A => Some(t >> sa),
            0x3B => Some(((t as i64) >> sa) as u64),
            0x3C => Some(t << (sa + 32)),
            0x3E => Some(t >> (sa + 32)),
            0x3F => Some(((t as i64) >> (sa + 32)) as u64),
            _ => return Err(Stop(format!("SPECIAL {fn_:#x} at {pc:#x} ({w:08x})"))),
        };
        if let Some(v) = v {
            self.set_reg(rd, v);
        }
        Ok(Next::Seq)
    }

    #[allow(clippy::too_many_arguments)]
    fn cop1(&mut self, pc: u32, w: u32, fmt: usize, rt: usize, rd: usize, sa: u32, fn_: u32, bt: u32) -> Result<Next, Stop> {
        let (fs, ft, fd) = (rd, rt, sa as usize);
        match fmt {
            0x00 => self.set_reg(rt, se32(u64::from(self.fpr[fs]))),
            0x02 => self.set_reg(rt, 0),
            0x04 => self.fpr[fs] = self.gpr[rt][0] as u32,
            0x06 => {}
            0x08 => {
                let taken = if rt & 1 == 1 { self.fcc } else { !self.fcc };
                return Ok(if taken {
                    Next::Branch(bt)
                } else if rt & 2 != 0 {
                    Next::SkipDelay
                } else {
                    Next::Seq
                });
            }
            0x10 => {
                let (a, b) = (self.f(fs), self.f(ft));
                match fn_ {
                    0x00 => self.set_f(fd, fpu::add(a, b)),
                    0x01 => self.set_f(fd, fpu::sub(a, b)),
                    0x02 => self.set_f(fd, fpu::mul(a, b)),
                    0x03 => self.set_f(fd, fpu::div(a, b)),
                    0x04 => self.set_f(fd, fpu::sqrt(b)),
                    0x05 => self.set_f(fd, a.abs()),
                    0x06 => self.fpr[fd] = self.fpr[fs],
                    0x07 => self.set_f(fd, -a),
                    0x16 => {
                        let s = fpu::sqrt(b);
                        self.set_f(fd, fpu::div(a, s));
                    }
                    0x18 => self.facc = fpu::add(a, b),
                    0x19 => self.facc = fpu::sub(a, b),
                    0x1A => self.facc = fpu::mul(a, b),
                    0x1C => self.set_f(fd, fpu::add(self.facc, fpu::mul(a, b))),
                    0x1D => self.set_f(fd, fpu::sub(self.facc, fpu::mul(a, b))),
                    0x1E => self.facc = fpu::add(self.facc, fpu::mul(a, b)),
                    0x1F => self.facc = fpu::sub(self.facc, fpu::mul(a, b)),
                    0x24 => self.fpr[fd] = fpu::to_i32(a) as u32,
                    0x28 => self.set_f(fd, fin(a).max(fin(b))),
                    0x29 => self.set_f(fd, fin(a).min(fin(b))),
                    0x30 => self.fcc = false,
                    0x32 => self.fcc = fin(a) == fin(b),
                    0x34 => self.fcc = fin(a) < fin(b),
                    0x36 => self.fcc = fin(a) <= fin(b),
                    _ => return Err(Stop(format!("COP1.S {fn_:#x} at {pc:#x} ({w:08x})"))),
                }
            }
            0x14 if fn_ == 0x20 => {
                let v = fpu::from_i32(self.fpr[fs] as i32);
                self.set_f(fd, v);
            }
            _ => return Err(Stop(format!("COP1 fmt {fmt:#x} at {pc:#x} ({w:08x})"))),
        }
        Ok(Next::Seq)
    }

    fn cop2(&mut self, pc: u32, w: u32, rs: usize, rt: usize) -> Result<(), Stop> {
        let co = (w >> 25) & 1;
        if co == 0 {
            let fs = ((w >> 11) & 31) as usize;
            match rs {
                0x01 => {
                    let v = self.vf[fs].map(f32::to_bits);
                    self.set_reg128(rt, u64::from(v[0]) | u64::from(v[1]) << 32, u64::from(v[2]) | u64::from(v[3]) << 32);
                }
                0x05 => {
                    if fs != 0 {
                        let [lo, hi] = self.gpr[rt];
                        self.vf[fs] = [lo as u32, (lo >> 32) as u32, hi as u32, (hi >> 32) as u32].map(f32::from_bits);
                    }
                }
                0x02 => {
                    let v = match fs {
                        0..=15 => u64::from(self.vi[fs]),
                        21 => u64::from(self.i.to_bits()),
                        22 => u64::from(self.q.to_bits()),
                        _ => 0,
                    };
                    self.set_reg(rt, v);
                }
                0x06 => {
                    let v = self.gpr[rt][0];
                    match fs {
                        1..=15 => self.vi[fs] = v as u16,
                        21 => self.i = f32::from_bits(v as u32),
                        22 => self.q = f32::from_bits(v as u32),
                        _ => {}
                    }
                }
                0x08 => {} // bc2: never used by the effect code
                _ => return Err(Stop(format!("COP2 fmt {rs:#x} at {pc:#x} ({w:08x})"))),
            }
            return Ok(());
        }
        let dest = (w >> 21) & 15;
        let mask = [dest & 8 != 0, dest & 4 != 0, dest & 2 != 0, dest & 1 != 0];
        let ft = ((w >> 16) & 31) as usize;
        let fs = ((w >> 11) & 31) as usize;
        let fd = ((w >> 6) & 31) as usize;
        let fn_ = w & 63;
        let s = self.vf[fs];
        let t = self.vf[ft];
        let acc = self.vacc;
        let (q, i) = (self.q, self.i);
        let put = |c: &mut Cpu, d: usize, v: [f32; 4]| {
            if d != 0 {
                for k in 0..4 {
                    if mask[k] {
                        c.vf[d][k] = v[k];
                    }
                }
            }
        };
        let putacc = |c: &mut Cpu, v: [f32; 4]| {
            for k in 0..4 {
                if mask[k] {
                    c.vacc[k] = v[k];
                }
            }
        };
        let m4 = |f: &dyn Fn(usize) -> f32| -> [f32; 4] { std::array::from_fn(f) };
        if fn_ < 0x3C {
            let bc = (fn_ & 3) as usize;
            match fn_ {
                0x00..=0x03 => put(self, fd, m4(&|k| fpu::vadd(s[k], t[bc]))),
                0x04..=0x07 => put(self, fd, m4(&|k| fpu::vsub(s[k], t[bc]))),
                0x08..=0x0B => put(self, fd, m4(&|k| fpu::vmadd(acc[k], s[k], t[bc]))),
                0x0C..=0x0F => put(self, fd, m4(&|k| fpu::vmsub(acc[k], s[k], t[bc]))),
                0x10..=0x13 => put(self, fd, m4(&|k| fin(s[k]).max(fin(t[bc])))),
                0x14..=0x17 => put(self, fd, m4(&|k| fin(s[k]).min(fin(t[bc])))),
                0x18..=0x1B => put(self, fd, m4(&|k| fpu::vmul(s[k], t[bc]))),
                0x1C => put(self, fd, m4(&|k| fpu::vmul(s[k], q))),
                0x1E => put(self, fd, m4(&|k| fpu::vmul(s[k], i))),
                0x20 => put(self, fd, m4(&|k| fpu::vadd(s[k], q))),
                0x21 => put(self, fd, m4(&|k| fpu::vmadd(acc[k], s[k], q))),
                0x22 => put(self, fd, m4(&|k| fpu::vadd(s[k], i))),
                0x23 => put(self, fd, m4(&|k| fpu::vmadd(acc[k], s[k], i))),
                0x24 => put(self, fd, m4(&|k| fpu::vsub(s[k], q))),
                0x25 => put(self, fd, m4(&|k| fpu::vmsub(acc[k], s[k], q))),
                0x26 => put(self, fd, m4(&|k| fpu::vsub(s[k], i))),
                0x27 => put(self, fd, m4(&|k| fpu::vmsub(acc[k], s[k], i))),
                0x28 => put(self, fd, m4(&|k| fpu::vadd(s[k], t[k]))),
                0x29 => put(self, fd, m4(&|k| fpu::vmadd(acc[k], s[k], t[k]))),
                0x2A => put(self, fd, m4(&|k| fpu::vmul(s[k], t[k]))),
                0x2B => put(self, fd, m4(&|k| fin(s[k]).max(fin(t[k])))),
                0x2C => put(self, fd, m4(&|k| fpu::vsub(s[k], t[k]))),
                0x2D => put(self, fd, m4(&|k| fpu::vmsub(acc[k], s[k], t[k]))),
                0x2E => {
                    let v = [
                        fpu::vmsub(acc[0], s[1], t[2]),
                        fpu::vmsub(acc[1], s[2], t[0]),
                        fpu::vmsub(acc[2], s[0], t[1]),
                        self.vf[fd][3],
                    ];
                    put(self, fd, v);
                }
                0x2F => put(self, fd, m4(&|k| fin(s[k]).min(fin(t[k])))),
                0x30 => {
                    if fd & 15 != 0 {
                        self.vi[fd & 15] = self.vi[fs & 15].wrapping_add(self.vi[ft & 15]);
                    }
                }
                0x31 => {
                    if fd & 15 != 0 {
                        self.vi[fd & 15] = self.vi[fs & 15].wrapping_sub(self.vi[ft & 15]);
                    }
                }
                0x32 => {
                    let mut imm = ((w >> 6) & 31) as i32;
                    if imm & 16 != 0 {
                        imm -= 32;
                    }
                    if ft & 15 != 0 {
                        self.vi[ft & 15] = (i32::from(self.vi[fs & 15]) + imm) as u16;
                    }
                }
                0x34 => {
                    if fd & 15 != 0 {
                        self.vi[fd & 15] = self.vi[fs & 15] & self.vi[ft & 15];
                    }
                }
                0x35 => {
                    if fd & 15 != 0 {
                        self.vi[fd & 15] = self.vi[fs & 15] | self.vi[ft & 15];
                    }
                }
                _ => return Err(Stop(format!("VU0 {fn_:#x} at {pc:#x} ({w:08x})"))),
            }
            return Ok(());
        }
        let k = (((w >> 6) & 31) << 2) | (fn_ & 3);
        let bc = (fn_ & 3) as usize;
        match k {
            0x00..=0x03 => putacc(self, m4(&|x| fpu::vadd(s[x], t[bc]))),
            0x04..=0x07 => putacc(self, m4(&|x| fpu::vsub(s[x], t[bc]))),
            0x08..=0x0B => putacc(self, m4(&|x| fpu::vmadd(acc[x], s[x], t[bc]))),
            0x0C..=0x0F => putacc(self, m4(&|x| fpu::vmsub(acc[x], s[x], t[bc]))),
            0x10..=0x13 => {
                let sh = [0, 4, 12, 15][(k - 0x10) as usize];
                let v = m4(&|x| {
                    let n = s[x].to_bits() as i32;
                    fpu::from_i32(n) / (1u32 << sh) as f32
                });
                put(self, ft, v);
            }
            0x14..=0x17 => {
                let sh = [0, 4, 12, 15][(k - 0x14) as usize];
                let v = m4(&|x| f32::from_bits(fpu::to_i32(fin(s[x]) * (1u32 << sh) as f32) as u32));
                put(self, ft, v);
            }
            0x18..=0x1B => putacc(self, m4(&|x| fpu::vmul(s[x], t[bc]))),
            0x1C => putacc(self, m4(&|x| fpu::vmul(s[x], q))),
            0x1D => put(self, ft, m4(&|x| fin(s[x]).abs())),
            0x1E => putacc(self, m4(&|x| fpu::vmul(s[x], i))),
            0x1F => {} // vclipw: clip flags are not modelled
            0x20 => putacc(self, m4(&|x| fpu::vadd(s[x], q))),
            0x21 => putacc(self, m4(&|x| fpu::vmadd(acc[x], s[x], q))),
            0x22 => putacc(self, m4(&|x| fpu::vadd(s[x], i))),
            0x23 => putacc(self, m4(&|x| fpu::vmadd(acc[x], s[x], i))),
            0x24 => putacc(self, m4(&|x| fpu::vsub(s[x], q))),
            0x26 => putacc(self, m4(&|x| fpu::vsub(s[x], i))),
            0x28 => putacc(self, m4(&|x| fpu::vadd(s[x], t[x]))),
            0x29 => putacc(self, m4(&|x| fpu::vmadd(acc[x], s[x], t[x]))),
            0x2A => putacc(self, m4(&|x| fpu::vmul(s[x], t[x]))),
            0x2C => putacc(self, m4(&|x| fpu::vsub(s[x], t[x]))),
            0x2D => putacc(self, m4(&|x| fpu::vmsub(acc[x], s[x], t[x]))),
            0x2E => {
                self.vacc[0] = fpu::vmul(s[1], t[2]);
                self.vacc[1] = fpu::vmul(s[2], t[0]);
                self.vacc[2] = fpu::vmul(s[0], t[1]);
            }
            0x2F | 0x3B => {} // vnop, vwaitq
            0x30 => put(self, ft, s),
            0x31 => put(self, ft, [s[1], s[2], s[3], s[0]]),
            0x38..=0x3A => {
                let a = s[((w >> 21) & 3) as usize];
                let b = t[((w >> 23) & 3) as usize];
                self.q = match k {
                    0x38 => fpu::vdiv(a, b),
                    0x39 => fpu::sqrt(b),
                    _ => fpu::vrsqrt(a, b),
                };
            }
            0x3C => {
                let a = s[((w >> 21) & 3) as usize];
                if ft & 15 != 0 {
                    self.vi[ft & 15] = a.to_bits() as u16;
                }
            }
            0x3D => {
                let v = f32::from_bits(self.vi[fs & 15] as i16 as i32 as u32);
                put(self, ft, [v; 4]);
            }
            _ => return Err(Stop(format!("VU0 special2 {k:#x} at {pc:#x} ({w:08x})"))),
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn mmi(&mut self, pc: u32, w: u32, rs: usize, rt: usize, rd: usize, sa: u32, fn_: u32) -> Result<Next, Stop> {
        let [sl, sh] = self.gpr[rs];
        let [tl, th] = self.gpr[rt];
        let words = |lo: u64, hi: u64| [lo as u32, (lo >> 32) as u32, hi as u32, (hi >> 32) as u32];
        let pack = |v: [u32; 4]| (u64::from(v[0]) | u64::from(v[1]) << 32, u64::from(v[2]) | u64::from(v[3]) << 32);
        let bad = || Err(Stop(format!("MMI {fn_:#x}/{sa:#x} at {pc:#x} ({w:08x})")));
        match fn_ {
            0x00 | 0x01 | 0x20 | 0x21 => {
                // madd / maddu (pipeline 0 or 1)
                let one = fn_ >= 0x20;
                let (hi, lo) = if one { (self.hi1, self.lo1) } else { (self.hi, self.lo) };
                let accv = ((hi & M32) << 32 | (lo & M32)) as i64;
                let p = if fn_ & 1 == 0 {
                    i64::from(sl as u32 as i32) * i64::from(tl as u32 as i32)
                } else {
                    ((sl & M32) * (tl & M32)) as i64
                };
                let r = accv.wrapping_add(p) as u64;
                let (nlo, nhi) = (se32(r & M32), se32(r >> 32));
                if one {
                    self.lo1 = nlo;
                    self.hi1 = nhi;
                } else {
                    self.lo = nlo;
                    self.hi = nhi;
                }
                self.set_reg(rd, nlo);
            }
            0x04 => {
                // plzcw: leading sign bits - 1 of each word
                let v = [sl as u32, (sl >> 32) as u32].map(|x| {
                    let x = x as i32;
                    let n = if x < 0 { (!x).leading_zeros() } else { x.leading_zeros() };
                    n.saturating_sub(1)
                });
                self.set_reg(rd, u64::from(v[0]) | u64::from(v[1]) << 32);
            }
            0x10 => self.set_reg(rd, self.hi1),
            0x11 => self.hi1 = sl,
            0x12 => self.set_reg(rd, self.lo1),
            0x13 => self.lo1 = sl,
            0x18 | 0x19 => {
                let p = if fn_ == 0x18 {
                    (i64::from(sl as u32 as i32) * i64::from(tl as u32 as i32)) as u64
                } else {
                    (sl & M32) * (tl & M32)
                };
                self.lo1 = se32(p & M32);
                self.hi1 = se32(p >> 32);
                self.set_reg(rd, self.lo1);
            }
            0x1A => {
                let (a, b) = (sl as u32 as i32, tl as u32 as i32);
                if b != 0 {
                    self.lo1 = a.wrapping_div(b) as i64 as u64;
                    self.hi1 = a.wrapping_rem(b) as i64 as u64;
                }
            }
            0x1B => {
                let (a, b) = (sl as u32, tl as u32);
                if b != 0 {
                    self.lo1 = se32(u64::from(a / b));
                    self.hi1 = se32(u64::from(a % b));
                }
            }
            0x08 => {
                // MMI0
                let (s, t) = (words(sl, sh), words(tl, th));
                let v: [u32; 4] = match sa {
                    0x00 => std::array::from_fn(|k| s[k].wrapping_add(t[k])),
                    0x01 => std::array::from_fn(|k| s[k].wrapping_sub(t[k])),
                    0x02 => std::array::from_fn(|k| if (s[k] as i32) > (t[k] as i32) { u32::MAX } else { 0 }),
                    0x03 => std::array::from_fn(|k| (s[k] as i32).max(t[k] as i32) as u32),
                    0x12 => [t[0], s[0], t[1], s[1]],
                    0x13 => [t[0], t[2], s[0], s[2]],
                    _ => return bad(),
                };
                let (lo, hi) = pack(v);
                self.set_reg128(rd, lo, hi);
            }
            0x28 => {
                // MMI1
                let (s, t) = (words(sl, sh), words(tl, th));
                let v: [u32; 4] = match sa {
                    0x01 => std::array::from_fn(|k| (t[k] as i32).wrapping_abs() as u32),
                    0x02 => std::array::from_fn(|k| if s[k] == t[k] { u32::MAX } else { 0 }),
                    0x03 => std::array::from_fn(|k| (s[k] as i32).min(t[k] as i32) as u32),
                    0x10 => std::array::from_fn(|k| s[k].saturating_add(t[k])),
                    0x11 => std::array::from_fn(|k| s[k].saturating_sub(t[k])),
                    0x12 => [t[2], s[2], t[3], s[3]],
                    0x18 => {
                        let (sb, tb) = (pack_bytes(sl, sh), pack_bytes(tl, th));
                        let out: [u8; 16] = std::array::from_fn(|k| sb[k].saturating_add(tb[k]));
                        let (lo, hi) = unpack_bytes(out);
                        self.set_reg128(rd, lo, hi);
                        return Ok(Next::Seq);
                    }
                    _ => return bad(),
                };
                let (lo, hi) = pack(v);
                self.set_reg128(rd, lo, hi);
            }
            0x09 => {
                // MMI2
                match sa {
                    0x08 => self.set_reg128(rd, self.hi, self.hi1),
                    0x09 => self.set_reg128(rd, self.lo, self.lo1),
                    0x0E => self.set_reg128(rd, tl, sl),
                    0x12 => self.set_reg128(rd, sl & tl, sh & th),
                    0x13 => self.set_reg128(rd, sl ^ tl, sh ^ th),
                    _ => return bad(),
                }
            }
            0x29 => {
                // MMI3
                match sa {
                    0x08 => {
                        self.hi = sl;
                        self.hi1 = sh;
                    }
                    0x09 => {
                        self.lo = sl;
                        self.lo1 = sh;
                    }
                    0x0E => self.set_reg128(rd, sh, th),
                    0x12 => self.set_reg128(rd, sl | tl, sh | th),
                    0x13 => self.set_reg128(rd, !(sl | tl), !(sh | th)),
                    0x1B => {
                        let (h0, h4) = (tl & 0xFFFF, th & 0xFFFF);
                        self.set_reg128(rd, h0 * 0x0001_0001_0001_0001, h4 * 0x0001_0001_0001_0001);
                    }
                    _ => return bad(),
                }
            }
            0x3C | 0x3E | 0x3F => {
                let v = words(tl, th).map(|x| match fn_ {
                    0x3C => x << sa,
                    0x3E => x >> sa,
                    _ => ((x as i32) >> sa) as u32,
                });
                let (lo, hi) = pack(v);
                self.set_reg128(rd, lo, hi);
            }
            _ => return bad(),
        }
        Ok(Next::Seq)
    }
}

fn pack_bytes(lo: u64, hi: u64) -> [u8; 16] {
    let mut b = [0; 16];
    b[..8].copy_from_slice(&lo.to_le_bytes());
    b[8..].copy_from_slice(&hi.to_le_bytes());
    b
}
fn unpack_bytes(b: [u8; 16]) -> (u64, u64) {
    (
        u64::from_le_bytes(b[..8].try_into().unwrap()),
        u64::from_le_bytes(b[8..].try_into().unwrap()),
    )
}

/// 0x1A2C70's float -> unsigned int (see `bmodule::fixunssfsi`).
fn fixunssfsi(bits: u32) -> u32 {
    let field = (bits >> 23) & 0xFF;
    if field == 0 || bits >> 31 != 0 {
        return 0;
    }
    let e = field as i32 - 127;
    if e < 0 {
        return 0;
    }
    if e >= 32 {
        return u32::MAX;
    }
    let m = ((bits & 0x7F_FFFF) << 7) | 0x4000_0000;
    if e < 31 { m >> (30 - e) } else { m << 1 }
}

enum Next {
    Seq,
    Branch(u32),
    SkipDelay,
}

#[cfg(test)]
mod tests {
    #[test]
    fn journal_undo_restores_written_pages() {
        let mut c = super::Cpu::new();
        c.write_u32(0x10_0000, 7);
        c.write_u8(0x7000_0010, 3);
        c.journal_start();
        c.write_u32(0x10_0000, 9);
        c.write_u64(0x10_2000, 5);
        c.write_u8(0x7000_0010, 4);
        let first = c.journal_take().unwrap();
        c.write_u32(0x10_0000, 11);
        let second = c.journal_take().unwrap();
        assert_eq!((first.len(), second.len()), (3, 1));
        c.journal_undo(second);
        assert_eq!(c.read_u32(0x10_0000), 9);
        c.journal_undo(first);
        assert_eq!((c.read_u32(0x10_0000), c.read_u64(0x10_2000), c.read_u8(0x7000_0010)), (7, 0, 3));
    }

    use super::*;

    fn asm(cpu: &mut Cpu, at: u32, words: &[u32]) {
        for (i, w) in words.iter().enumerate() {
            cpu.write_u32(at + 4 * i as u32, *w);
        }
    }

    #[test]
    fn calls_return_v0_through_the_delay_slot() {
        let mut cpu = Cpu::new();
        // addu v0, a0, a1 ; jr ra ; addiu v0, v0, 1
        asm(&mut cpu, 0x10_0000, &[0x0085_1021, 0x03E0_0008, 0x2442_0001]);
        let v = cpu.call(0x10_0000, &[40, 1], 0x1F0_0000, 100, &mut |_, _| Hook::Run).unwrap();
        assert_eq!(v, 42);
    }

    #[test]
    fn hooks_replace_called_functions() {
        let mut cpu = Cpu::new();
        // jal 0x200000 ; nop ; jr ra ; nop  (ra saved in s0 around the call)
        asm(&mut cpu, 0x10_0000, &[0x03E0_8025, 0x0C08_0000, 0x0000_0000, 0x0200_F825, 0x03E0_0008, 0x0000_0000]);
        let mut seen = Vec::new();
        let v = cpu
            .call(0x10_0000, &[], 0x1F0_0000, 100, &mut |_, pc| {
                seen.push(pc);
                if pc == 0x20_0000 {
                    Hook::Return(Some(7))
                } else {
                    Hook::Run
                }
            })
            .unwrap();
        assert_eq!(v, 7);
        assert!(seen.contains(&0x20_0000));
    }

    #[test]
    fn branch_likely_skips_its_delay_slot_when_not_taken() {
        let mut cpu = Cpu::new();
        // beql a0, a1, +2 ; addiu v0, zero, 5 ; jr ra ; nop ; (target) addiu v0, v0, 1
        asm(&mut cpu, 0x10_0000, &[0x5085_0002, 0x2402_0005, 0x03E0_0008, 0x0000_0000]);
        let v = cpu.call(0x10_0000, &[1, 2], 0x1F0_0000, 100, &mut |_, _| Hook::Run).unwrap();
        assert_eq!(v, 0);
    }

    #[test]
    fn floats_follow_the_pcsx2_model() {
        let mut cpu = Cpu::new();
        // mtc1 a0, f0 ; mtc1 a1, f1 ; div.s f2, f0, f1 ; mfc1 v0, f2 ; jr ra ; nop
        asm(&mut cpu, 0x10_0000, &[0x4484_0000, 0x4485_0800, 0x4601_0083, 0x4402_1000, 0x03E0_0008, 0x0000_0000]);
        let v = cpu
            .call(0x10_0000, &[u64::from(1.0f32.to_bits()), 0], 0x1F0_0000, 100, &mut |_, _| Hook::Run)
            .unwrap();
        assert_eq!(f32::from_bits(v as u32), f32::MAX, "x / 0 is +FLT_MAX");
    }

    #[test]
    fn scratchpad_is_separate_from_main_memory() {
        let mut cpu = Cpu::new();
        cpu.write_u32(0x7000_0010, 0xAABB_CCDD);
        assert_eq!(cpu.read_u32(0x7000_0010), 0xAABB_CCDD);
        assert_eq!(cpu.read_u32(0x0000_0010), 0);
    }
}

/// The interpreter's memory as the ports' [`roses_fx::bus::Bus`], so the
/// same setup code ([`roses_fx::world`]) writes both.
impl roses_fx::bus::Bus for Cpu {
    fn read_u8(&mut self, addr: u32) -> u8 {
        Cpu::read_u8(self, addr)
    }
    fn write_u8(&mut self, addr: u32, v: u8) {
        Cpu::write_u8(self, addr, v)
    }
    fn read_u16(&mut self, addr: u32) -> u16 {
        Cpu::read_u16(self, addr)
    }
    fn read_u32(&mut self, addr: u32) -> u32 {
        Cpu::read_u32(self, addr)
    }
    fn read_u64(&mut self, addr: u32) -> u64 {
        Cpu::read_u64(self, addr)
    }
    fn write_u16(&mut self, addr: u32, v: u16) {
        Cpu::write_u16(self, addr, v)
    }
    fn write_u32(&mut self, addr: u32, v: u32) {
        Cpu::write_u32(self, addr, v)
    }
    fn write_u64(&mut self, addr: u32, v: u64) {
        Cpu::write_u64(self, addr, v)
    }
    fn load(&mut self, addr: u32, bytes: &[u8]) {
        Cpu::load(self, addr, bytes)
    }
}
