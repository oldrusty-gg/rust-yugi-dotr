//! The memory picture an effect program works on: its data area, the
//! state the game leaves for it (camera record, unit objects, textures),
//! the executable's tables and the module record's constants, all at their
//! EE addresses. A port reads and writes it through [`Bus`]; the
//! interpreter's CPU implements the same trait, so both sides are set up by
//! the same code ([`crate::world`]) and can be compared byte for byte.
//!
//! [`Mem`] is the port's: sparse 4 KiB pages; a page not written yet reads
//! as the executable's loaded image (PT_LOAD file bytes, zero beyond), as
//! the interpreter's freshly loaded memory does. Addresses map as the
//! interpreter maps them: 0x7xxxxxxx is the 16 KiB scratchpad, anything
//! else is main memory modulo 32 MiB.

/// Byte-addressed EE memory (little endian; accesses aligned down as the
/// interpreter's).
pub trait Bus {
    fn read_u8(&mut self, addr: u32) -> u8;
    fn write_u8(&mut self, addr: u32, v: u8);
    fn read_u16(&mut self, addr: u32) -> u16 {
        let a = addr & !1;
        u16::from_le_bytes([self.read_u8(a), self.read_u8(a + 1)])
    }
    fn read_u32(&mut self, addr: u32) -> u32 {
        let a = addr & !3;
        u32::from_le_bytes(std::array::from_fn(|k| self.read_u8(a + k as u32)))
    }
    fn read_u64(&mut self, addr: u32) -> u64 {
        let a = addr & !7;
        u64::from_le_bytes(std::array::from_fn(|k| self.read_u8(a + k as u32)))
    }
    fn read_f32(&mut self, addr: u32) -> f32 {
        f32::from_bits(self.read_u32(addr))
    }
    fn write_u16(&mut self, addr: u32, v: u16) {
        let a = addr & !1;
        for (k, b) in v.to_le_bytes().into_iter().enumerate() {
            self.write_u8(a + k as u32, b);
        }
    }
    fn write_u32(&mut self, addr: u32, v: u32) {
        let a = addr & !3;
        for (k, b) in v.to_le_bytes().into_iter().enumerate() {
            self.write_u8(a + k as u32, b);
        }
    }
    fn write_u64(&mut self, addr: u32, v: u64) {
        let a = addr & !7;
        for (k, b) in v.to_le_bytes().into_iter().enumerate() {
            self.write_u8(a + k as u32, b);
        }
    }
    fn write_f32(&mut self, addr: u32, v: f32) {
        self.write_u32(addr, v.to_bits());
    }
    fn load(&mut self, addr: u32, bytes: &[u8]) {
        for (i, b) in bytes.iter().enumerate() {
            self.write_u8(addr.wrapping_add(i as u32), *b);
        }
    }
    // Typed helpers for ports.
    fn i8(&mut self, addr: u32) -> i32 {
        self.read_u8(addr) as i8 as i32
    }
    fn u8(&mut self, addr: u32) -> i32 {
        i32::from(self.read_u8(addr))
    }
    fn i16(&mut self, addr: u32) -> i32 {
        self.read_u16(addr) as i16 as i32
    }
    fn u16(&mut self, addr: u32) -> i32 {
        i32::from(self.read_u16(addr))
    }
    fn i32(&mut self, addr: u32) -> i32 {
        self.read_u32(addr) as i32
    }
    fn f(&mut self, addr: u32) -> f32 {
        self.read_f32(addr)
    }
    fn set8(&mut self, addr: u32, v: i32) {
        self.write_u8(addr, v as u8);
    }
    fn set16(&mut self, addr: u32, v: i32) {
        self.write_u16(addr, v as u16);
    }
    fn set32(&mut self, addr: u32, v: i32) {
        self.write_u32(addr, v as u32);
    }
    fn setf(&mut self, addr: u32, v: f32) {
        self.write_f32(addr, v);
    }
}

const PAGE: usize = 4096;
const MAIN_MASK: u32 = 32 * 1024 * 1024 - 1;
const SCRATCH_MASK: u32 = 16 * 1024 - 1;
/// Page keys of the scratchpad (outside main memory's).
const SCRATCH_KEY: u32 = (MAIN_MASK + 1) / PAGE as u32;

/// The loaded executable's bytes by address (PT_LOAD file bytes).
#[derive(Clone, Debug, Default)]
pub struct Image {
    segments: Vec<(u32, Vec<u8>)>,
}

impl Image {
    /// PT_LOAD segments of an ELF (file bytes; the rest of each segment
    /// reads zero), as `roses_ee::Cpu::load_elf` loads them. `None`: not
    /// an ELF.
    pub fn from_elf(elf: &[u8]) -> Option<Self> {
        let u16_at = |o: usize| elf.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
        let u32_at = |o: usize| elf.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        if elf.get(..4) != Some(b"\x7FELF") {
            return None;
        }
        let phoff = u32_at(0x1C)? as usize;
        let phentsize = usize::from(u16_at(0x2A)?);
        let phnum = usize::from(u16_at(0x2C)?);
        let mut segments = Vec::new();
        for i in 0..phnum {
            let ph = phoff + i * phentsize;
            if u32_at(ph)? != 1 {
                continue;
            }
            let (offset, vaddr, filesz) = (u32_at(ph + 4)? as usize, u32_at(ph + 8)?, u32_at(ph + 16)? as usize);
            let bytes = elf.get(offset..offset + filesz)?.to_vec();
            segments.push((vaddr & MAIN_MASK, bytes));
        }
        Some(Self { segments })
    }

    /// The image's bytes from `addr` into `out` (segments in load order, so
    /// later ones win as loading them in order does; zero elsewhere).
    pub fn fill(&self, addr: u32, out: &mut [u8]) {
        out.fill(0);
        let a = addr & MAIN_MASK;
        let end = a as u64 + out.len() as u64;
        for (base, bytes) in &self.segments {
            let (s, e) = (u64::from(*base), u64::from(*base) + bytes.len() as u64);
            let (lo, hi) = (s.max(u64::from(a)), e.min(end));
            if lo < hi {
                out[(lo - u64::from(a)) as usize..(hi - u64::from(a)) as usize].copy_from_slice(&bytes[(lo - s) as usize..(hi - s) as usize]);
            }
        }
    }

    /// The byte at a main-memory address (later segments win, as loading
    /// them in order does).
    pub fn byte(&self, addr: u32) -> u8 {
        let a = addr & MAIN_MASK;
        self.segments
            .iter()
            .rev()
            .find_map(|(base, b)| a.checked_sub(*base).and_then(|o| b.get(o as usize)).copied())
            .unwrap_or(0)
    }
}

/// The port's memory: sparse pages over the executable's image (a page
/// is materialised, from the image, on its first access; cloning copies
/// only the pages made).
#[derive(Clone, Debug, Default)]
pub struct Mem {
    image: std::sync::Arc<Image>,
    pages: Vec<Option<Box<[u8; PAGE]>>>,
}

/// Main memory pages, then the scratchpad's.
const PAGES: usize = (MAIN_MASK as usize + 1) / PAGE + (SCRATCH_MASK as usize + 1) / PAGE;

impl Mem {
    pub fn new(image: std::sync::Arc<Image>) -> Self {
        Self { image, pages: vec![None; PAGES] }
    }

    #[inline]
    fn key(addr: u32) -> (usize, usize) {
        if addr >> 28 == 7 {
            let a = addr & SCRATCH_MASK;
            (SCRATCH_KEY as usize + (a as usize / PAGE), a as usize % PAGE)
        } else {
            let a = addr & MAIN_MASK;
            (a as usize / PAGE, a as usize % PAGE)
        }
    }

    #[inline]
    fn page(&mut self, key: usize) -> &mut [u8; PAGE] {
        if self.pages.len() < PAGES {
            self.pages.resize(PAGES, None);
        }
        let image = &self.image;
        self.pages[key].get_or_insert_with(|| {
            let mut p = Box::new([0u8; PAGE]);
            if key < SCRATCH_KEY as usize {
                image.fill((key * PAGE) as u32, &mut p[..]);
            }
            p
        })
    }

    /// `N` bytes at an aligned address (within one page).
    #[inline]
    fn get<const N: usize>(&mut self, addr: u32) -> [u8; N] {
        let (k, o) = Self::key(addr);
        let p = self.page(k);
        let mut out = [0u8; N];
        out.copy_from_slice(&p[o..o + N]);
        out
    }
    #[inline]
    fn put<const N: usize>(&mut self, addr: u32, v: [u8; N]) {
        let (k, o) = Self::key(addr);
        self.page(k)[o..o + N].copy_from_slice(&v);
    }
}

impl Bus for Mem {
    #[inline]
    fn read_u8(&mut self, addr: u32) -> u8 {
        let (k, o) = Self::key(addr);
        self.page(k)[o]
    }
    #[inline]
    fn write_u8(&mut self, addr: u32, v: u8) {
        let (k, o) = Self::key(addr);
        self.page(k)[o] = v;
    }
    fn read_u16(&mut self, addr: u32) -> u16 {
        u16::from_le_bytes(self.get(addr & !1))
    }
    fn read_u32(&mut self, addr: u32) -> u32 {
        u32::from_le_bytes(self.get(addr & !3))
    }
    fn read_u64(&mut self, addr: u32) -> u64 {
        u64::from_le_bytes(self.get(addr & !7))
    }
    fn write_u16(&mut self, addr: u32, v: u16) {
        self.put(addr & !1, v.to_le_bytes());
    }
    fn write_u32(&mut self, addr: u32, v: u32) {
        self.put(addr & !3, v.to_le_bytes());
    }
    fn write_u64(&mut self, addr: u32, v: u64) {
        self.put(addr & !7, v.to_le_bytes());
    }
}
