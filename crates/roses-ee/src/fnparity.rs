//! Function-level parity for the hand-ported effect library
//! (`roses_fx::blib`): a library function's port is run from the exact
//! state the interpreter is in when a real record calls the original, and
//! everything the call leaves is compared.
//!
//! [`watch`] puts a probe on a running [`AttackEffect`]: at a call of the
//! function's address in that record (a `jal` / `jalr` reaching it: ra =
//! the jump + 8) the interpreter's whole memory (main and scratchpad), its
//! registers and VU0's current matrix (vf28..vf31) are copied into a fresh
//! port frame ([`Fx`]: the same memory picture, `sp`, `cur`), the port runs
//! on it, then the interpreter runs the original to its return. Compared
//! there: the return value the port claims (v0 low 32 bits or f0 bits),
//! every byte of main memory and the scratchpad outside the call's dead
//! stack frame (below the entry `sp`, counted apart: saved registers and
//! stale locals), the primitives submitted (0x206F30 / 0x206F90, every
//! field as `parity::prim_key`), textures asked for and missing
//! (0x202530) and the current matrix's bits. The interpreter's result is
//! kept; the port's is dropped, so every sample starts from the original's
//! state.
use crate::bmodule::{AttackEffect, CameraMirror, EffectTextures, EffectWorld, GsPrim, UnitMirror, OT_ENTRIES};
use crate::{Cpu, Hook};
use roses_fx::bus::{Bus, Image, Mem};
use roses_fx::exe::Fx;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Where a watched call returns first (never executed: the probe sends the
/// interpreter on to the real return address).
const PROBE_RETURN: u32 = 0x0DEA_DF00;
/// Bytes below the call's stack pointer taken as its frame.
const STACK_WINDOW: u32 = 0x1_0000;
const MAIN_BYTES: usize = 32 * 1024 * 1024;
const PAGE: usize = 4096;

/// The registers at a watched call.
#[derive(Clone, Debug)]
pub struct Call {
    /// The function's address in the running record.
    pub at: u32,
    /// GPR low halves.
    pub gpr: [u64; 32],
    pub fpr: [u32; 32],
    /// The card (BMODULE slot) running.
    pub card: u16,
}

impl Call {
    /// Integer argument `i` (a0..t3) as a u32.
    pub fn a(&self, i: usize) -> u32 {
        self.gpr[4 + i] as u32
    }
    pub fn ai(&self, i: usize) -> i32 {
        self.gpr[4 + i] as u32 as i32
    }
    /// Float argument `i` (f12..f19).
    pub fn f(&self, i: usize) -> f32 {
        f32::from_bits(self.fpr[12 + i])
    }
    pub fn sp(&self) -> u32 {
        self.gpr[29] as u32
    }
}

/// What a port claims to return.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ret {
    None,
    /// v0's low 32 bits.
    V0(u32),
    /// f0's bits.
    F0(f32),
}

/// A port adapter: the registers to the port's arguments.
pub type PortFn = Arc<dyn Fn(&mut Fx, &Call) -> Ret + Send + Sync>;

/// Perturbs a real call's inputs before both sides run (registers or the
/// memory the call reads; `seed` varies per sample), so branches the
/// records' own calls do not take are compared too. The interpreter goes on
/// from the perturbed state.
pub type Mutator = Arc<dyn Fn(&mut Cpu, u64) + Send + Sync>;

/// Results of watching one function.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub samples: usize,
    /// Of them, perturbed ([`Mutator`]).
    pub perturbed: usize,
    /// Calls seen (sampled or not).
    pub calls: usize,
    /// Cards that gave samples.
    pub cards: std::collections::BTreeSet<u16>,
    pub diffs: Vec<String>,
    /// Samples with a difference.
    pub differing: usize,
    pub prims: usize,
    /// Bytes written by the call in main memory (the oracle's), summed.
    pub bytes_written: usize,
    /// Samples whose dead stack frame differs (not a failure).
    pub stack_differs: usize,
    pub oracle: Duration,
    pub port: Duration,
    /// Word offsets of the function the sampled calls executed (the
    /// interpreter's instruction histogram before / after each).
    pub covered: std::collections::BTreeSet<u32>,
}

impl Report {
    pub fn equal(&self) -> bool {
        self.differing == 0
    }
    pub fn merge(&mut self, o: &Report) {
        self.samples += o.samples;
        self.perturbed += o.perturbed;
        self.calls += o.calls;
        self.cards.extend(o.cards.iter().copied());
        self.diffs.extend(o.diffs.iter().cloned());
        self.differing += o.differing;
        self.prims += o.prims;
        self.bytes_written += o.bytes_written;
        self.stack_differs += o.stack_differs;
        self.oracle += o.oracle;
        self.port += o.port;
        self.covered.extend(o.covered.iter().copied());
    }
}

/// An ELF-shaped image of a memory copy (one PT_LOAD at 0), so a port
/// [`Mem`] reads it as its untouched pages.
fn image_of(mem: &[u8]) -> Arc<Image> {
    let mut elf = vec![0u8; 0x54];
    elf[..4].copy_from_slice(b"\x7FELF");
    elf[0x1C..0x20].copy_from_slice(&0x34u32.to_le_bytes());
    elf[0x2A..0x2C].copy_from_slice(&0x20u16.to_le_bytes());
    elf[0x2C..0x2E].copy_from_slice(&1u16.to_le_bytes());
    elf[0x34..0x38].copy_from_slice(&1u32.to_le_bytes());
    elf[0x38..0x3C].copy_from_slice(&0x54u32.to_le_bytes());
    elf[0x3C..0x40].copy_from_slice(&0u32.to_le_bytes());
    elf[0x44..0x48].copy_from_slice(&(mem.len() as u32).to_le_bytes());
    elf.extend_from_slice(mem);
    Arc::new(Image::from_elf(&elf).expect("image"))
}

/// A world that only sets the frame counter (the port frame's begin; the
/// memory it mirrors is replaced).
struct Counter(u32);
impl EffectWorld for Counter {
    fn camera(&self) -> CameraMirror {
        CameraMirror { eye: [0.0, 0.0, -1.0], target: [0.0; 3], distance: 1.0, roll: 0.0 }
    }
    fn unit(&self, _role: usize) -> UnitMirror {
        UnitMirror { position: [0.0; 3], rotation: [0; 3], animated: [0.0; 3], motion: 0, time: 0.0, speed: 1.0, play_mode: 0, frozen: false, frames: [0; 8], alpha: 0x80, nodes: 0, model: 0, bounds: [0.0; 4] }
    }
    fn node(&self, _role: usize, _node: u8) -> Option<[f32; 3]> {
        None
    }
    fn frame_counter(&self) -> Option<u32> {
        Some(self.0)
    }
}

struct Active {
    ra: u64,
    sp: u32,
    fx: Fx,
    ret: Ret,
    prims: Vec<GsPrim>,
    missing: Vec<(i32, i32)>,
    drawn_tbp: u16,
    oracle_start: Instant,
    port_time: Duration,
    snapshot: Arc<Image>,
    /// The histogram's counts over the function at entry.
    hist: Vec<u32>,
    /// Packet layers (0x2078A0 / 0x207910) during the call, numbered from 1
    /// as a port numbers them from a fresh update: closed
    /// (id, key, primitives before), open, the last id.
    layers: Vec<(u32, Option<i32>, usize)>,
    open: Vec<u32>,
    next: u32,
    /// Boundary calls reported by the runner during the call (address,
    /// a0..a3).
    boundary: Vec<(u32, [u32; 4])>,
    /// Units drawn into the offscreen target (0x1EF6F0 / 0x1EF8F0: role,
    /// primitives before) and what it holds since (a primitive sampling
    /// TBP 0x2000 gets it, as the runner keys it).
    renders: Vec<(u8, usize)>,
    rt: Option<roses_fx::types::TextureKey>,
}

/// Which calls are sampled: the first four, then every seventh.
fn sampled(seen: usize) -> bool {
    seen < 4 || seen % 7 == 0
}

/// Watch calls of the function at `at` in `effect`'s record and compare
/// `port` on each sampled call (at most `max_samples`); results into
/// `report`.
pub fn watch(effect: &mut AttackEffect, at: u32, words: u32, handles: [u16; 2], textures: &EffectTextures, port: PortFn, mutate: Option<Mutator>, report: Arc<Mutex<Report>>, max_samples: usize) {
    if effect.cpu.histogram.is_none() {
        effect.cpu.histogram = Some(vec![0; MAIN_BYTES / 4]);
    }
    let range = ((at as usize & (MAIN_BYTES - 1)) >> 2)..((at as usize & (MAIN_BYTES - 1)) >> 2) + words as usize;
    let model = effect.model;
    let card = effect.card;
    let textures = textures.clone();
    // The texture placement both sides answer 0x202530 from.
    let placed = {
        let mut probe = Fx::new(Arc::new(Image::default()), &[], model, handles, &textures);
        std::mem::take(&mut probe.textures)
    };
    let mut seen = 0usize;
    let mut taken = 0usize;
    let mut active: Option<Active> = None;
    effect.probe = Some(Box::new(move |c: &mut Cpu, pc: u32| -> Option<Hook> {
        if active.is_some() {
            match pc {
                0x0020_6F30 | 0x0020_6F90 => {
                    let act = active.as_mut().unwrap();
                    let buffered = pc == 0x0020_6F90;
                    let a = if buffered { 5 } else { 4 };
                    let (desc, ot, key) = (c.reg(a) as u32, c.reg(a + 1) as u32, c.reg(a + 2) as u32 as i32);
                    if let Some(mut p) = roses_fx::world::decode_descriptor(c, desc) {
                        p.buffered = buffered;
                        p.ot_key = (ot != 0).then(|| if p.flags & 0x8000 != 0 { OT_ENTRIES as i32 - 1 } else { (key as u32).min(OT_ENTRIES - 2) as i32 });
                        p.drawn_buffer = p.textured() && p.tex0.is_some_and(|t| (t & 0x3FFF) as u16 == act.drawn_tbp);
                        p.layer = act.open.last().copied();
                        if p.texture.is_none() && p.tex0.map(|t| (t & 0x3FFF) as u16) == Some(roses_fx::types::UNIT_RT_TBP) {
                            p.texture = act.rt;
                        }
                        act.prims.push(p);
                    }
                    return None;
                }
                0x0020_78A0 => {
                    let act = active.as_mut().unwrap();
                    act.next += 1;
                    let id = act.next;
                    act.open.push(id);
                    return None;
                }
                0x0020_7910 => {
                    let act = active.as_mut().unwrap();
                    let (ot, key) = (c.reg(5) as u32, c.reg(6) as u32 as i32);
                    if let Some(id) = act.open.pop() {
                        let n = act.prims.len();
                        act.layers.push((id, (ot != 0).then_some((key as u32).min(OT_ENTRIES - 2) as i32), n));
                    }
                    return None;
                }
                0x001E_F6F0 | 0x001E_F8F0 => {
                    let act = active.as_mut().unwrap();
                    let obj = c.reg(5) as u32;
                    if let Some(role) = roses_fx::world::OBJECT_INDEX.iter().position(|i| u32::from(*i) == obj) {
                        act.rt = Some(roses_fx::types::TextureKey::UnitRt(role as u8));
                        let n = act.prims.len();
                        act.renders.push((role as u8, n));
                    }
                    return None;
                }
                // 0x207FC0 with a destination queues a frame copy
                // (as the runner records it): the offscreen target then
                // holds the frame for the primitives sampling it.
                0x0020_7FC0 => {
                    let args = [c.reg(4) as u32, c.reg(5) as u32, c.reg(6) as u32, c.reg(7) as u32];
                    let act = active.as_mut().unwrap();
                    act.boundary.push((pc, args));
                    let dest = args[1] as i32;
                    if dest > 0 && (dest & 0x3FFF) as u16 == roses_fx::types::UNIT_RT_TBP {
                        act.rt = Some(roses_fx::types::TextureKey::Frame);
                    }
                    return None;
                }
                pc if crate::bmodule::boundary_name(pc).is_some() => {
                    let args = [c.reg(4) as u32, c.reg(5) as u32, c.reg(6) as u32, c.reg(7) as u32];
                    active.as_mut().unwrap().boundary.push((pc, args));
                    return None;
                }
                0x0020_2530 => {
                    let key = (c.reg(4) as i32, c.reg(5) as i32);
                    if !placed.contains_key(&key) {
                        active.as_mut().unwrap().missing.push(key);
                    }
                    return None;
                }
                PROBE_RETURN => {
                    let act = active.take().unwrap();
                    let ra = act.ra;
                    let mut r = report.lock().unwrap();
                    r.oracle += act.oracle_start.elapsed();
                    r.port += act.port_time;
                    if let Some(h) = &c.histogram {
                        for (k, (a, b)) in h[range.clone()].iter().zip(&act.hist).enumerate() {
                            if a != b {
                                r.covered.insert(k as u32);
                            }
                        }
                    }
                    compare_call(c, act, card, taken, &mut r);
                    c.gpr[31][0] = ra;
                    return Some(Hook::Jump(ra as u32));
                }
                _ => return None,
            }
        }
        if pc != at || c.reg(31) as u32 != c.from.wrapping_add(8) {
            return None;
        }
        seen += 1;
        report.lock().unwrap().calls += 1;
        if taken >= max_samples || !sampled(seen - 1) {
            return None;
        }
        taken += 1;
        // Every other sample perturbed when a mutator is given.
        let perturbed = mutate.as_ref().is_some_and(|m| {
            if taken % 2 == 0 {
                m(c, (taken as u64) ^ (u64::from(card) << 20));
                true
            } else {
                false
            }
        });
        if perturbed {
            report.lock().unwrap().perturbed += 1;
        }
        let call = Call { at, gpr: std::array::from_fn(|r| c.gpr[r][0]), fpr: c.fpr, card };
        let snapshot = image_of(&c.mem[..MAIN_BYTES]);
        let mut fx = Fx::new(snapshot.clone(), &[], model, handles, &textures);
        let counter = c.read_u32(roses_fx::world::FRAME_COUNTER);
        fx.begin(&Counter(counter));
        fx.mem = Mem::new(snapshot.clone());
        let scratch = c.scratch.clone();
        fx.mem.load(0x7000_0000, &scratch);
        fx.sp = call.sp();
        fx.cur = std::array::from_fn(|r| c.vf[28 + r]);
        // LO / HI as the call finds them (a division by zero keeps them).
        fx.set_hilo(c.lo as u32 as i32, c.hi as u32 as i32);
        fx.out = Default::default();
        let t = Instant::now();
        let ret = port(&mut fx, &call);
        let port_time = t.elapsed();
        let ra = c.gpr[31][0];
        c.gpr[31][0] = u64::from(PROBE_RETURN);
        let drawn_tbp = if counter & 1 == 1 { 0 } else { 0x800 };
        let hist = c.histogram.as_ref().map(|h| h[range.clone()].to_vec()).unwrap_or_default();
        active = Some(Active { ra, sp: call.sp(), fx, ret, prims: Vec::new(), missing: Vec::new(), drawn_tbp, oracle_start: Instant::now(), port_time, snapshot, hist, layers: Vec::new(), open: Vec::new(), next: 0, boundary: Vec::new(), renders: Vec::new(), rt: None });
        None
    }));
}

/// Compare a finished call: the oracle's state in `c`, the port's in
/// `act`.
fn compare_call(c: &mut Cpu, mut act: Active, card: u16, sample: usize, r: &mut Report) {
    let mut d: Vec<String> = Vec::new();
    match act.ret {
        Ret::None => {}
        Ret::V0(v) => {
            if c.gpr[2][0] as u32 != v {
                d.push(format!("v0 {:#x} vs {v:#x}", c.gpr[2][0] as u32));
            }
        }
        Ret::F0(f) => {
            if c.fpr[0] != f.to_bits() {
                d.push(format!("f0 {:#x} ({}) vs {:#x} ({f})", c.fpr[0], f32::from_bits(c.fpr[0]), f.to_bits()));
            }
        }
    }
    let pa: Vec<String> = act.prims.iter().map(crate::parity::prim_key).collect();
    let pb: Vec<String> = act.fx.out.prims.iter().map(crate::parity::prim_key).collect();
    r.prims += pa.len();
    if pa.len() != pb.len() {
        d.push(format!("prims {} vs {}", pa.len(), pb.len()));
    }
    for (i, (x, y)) in pa.iter().zip(&pb).enumerate() {
        if x != y {
            d.push(format!("prim {i}:\n    oracle {x}\n    port   {y}"));
            break;
        }
    }
    if act.missing != act.fx.out.missing_textures {
        d.push(format!("missing textures {:?} vs {:?}", act.missing, act.fx.out.missing_textures));
    }
    let pbound: Vec<(u32, [u32; 4])> = act.fx.out.boundary.iter().map(|b| (b.0, b.2)).collect();
    if act.boundary != pbound {
        d.push(format!("boundary calls {:x?} vs {:x?}", act.boundary, pbound));
    }
    if act.layers != act.fx.out.layers {
        d.push(format!("layers {:?} vs {:?}", act.layers, act.fx.out.layers));
    }
    let prenders: Vec<(u8, usize)> = act.fx.out.unit_renders.iter().map(|u| (u.role, u.before)).collect();
    if act.renders != prenders {
        d.push(format!("unit renders {:?} vs {:?}", act.renders, prenders));
    }
    let cur: [[u32; 4]; 4] = std::array::from_fn(|i| c.vf[28 + i].map(f32::to_bits));
    let pcur: [[u32; 4]; 4] = act.fx.cur.map(|row| row.map(f32::to_bits));
    if cur != pcur {
        d.push(format!("vf28..31 {cur:x?} vs {pcur:x?}"));
    }
    // Memory: every page; the dead frame [sp - window, sp) apart.
    let (lo, hi) = (act.sp.wrapping_sub(STACK_WINDOW) & 0x01FF_FFFF, act.sp & 0x01FF_FFFF);
    let mut before = vec![0u8; PAGE];
    let mut differing = Vec::new();
    let mut stack = 0usize;
    // ROSES_FNPARITY_DEADFRAME=1: list dead-frame differences (bytes below
    // the entry sp) of differing samples, for debugging.
    let dead_list = std::env::var_os("ROSES_FNPARITY_DEADFRAME").is_some();
    let mut dead: Vec<(u32, u8, u8)> = Vec::new();
    for p in 0..MAIN_BYTES / PAGE {
        let base = (p * PAGE) as u32;
        let o = &c.mem[p * PAGE..(p + 1) * PAGE];
        act.snapshot.fill(base, &mut before);
        r.bytes_written += o.iter().zip(&before).filter(|(a, b)| a != b).count();
        for k in (0..PAGE).step_by(8) {
            let a = base + k as u32;
            let x = u64::from_le_bytes(o[k..k + 8].try_into().unwrap());
            let y = act.fx.mem.read_u64(a);
            if x != y {
                for j in 0..8u32 {
                    let (bx, by) = ((x >> (8 * j)) as u8, (y >> (8 * j)) as u8);
                    if bx != by {
                        if (lo..hi).contains(&(a + j)) {
                            stack += 1;
                            if dead_list && dead.len() < 0x1_0000 {
                                dead.push((hi - (a + j), bx, by));
                            }
                        } else {
                            differing.push((a + j, bx, by));
                        }
                    }
                }
            }
        }
    }
    for k in 0..c.scratch.len() as u32 {
        let (x, y) = (c.scratch[k as usize], act.fx.mem.read_u8(0x7000_0000 + k));
        if x != y {
            differing.push((0x7000_0000 + k, x, y));
        }
    }
    if !differing.is_empty() {
        d.push(format!("{} bytes differ (addr, oracle, port): {:x?}", differing.len(), &differing[..differing.len().min(24)]));
    }
    if stack > 0 {
        r.stack_differs += 1;
    }
    if dead_list && !d.is_empty() && !dead.is_empty() {
        let tail = &dead[dead.len().saturating_sub(64)..];
        d.push(format!("dead frame, the 64 nearest the entry sp (entry sp - k, oracle, port): {tail:x?}"));
    }
    r.samples += 1;
    r.cards.insert(card);
    if !d.is_empty() {
        r.differing += 1;
        if r.diffs.len() < 12 {
            r.diffs.push(format!("card {card} sample {sample}: {}", d.join("; ")));
        }
    }
}

/// Normalized code words of a function (record-relative address halves
/// and calls into the record masked, as the inventory's hash masks them).
pub fn normalized(words: &[u32]) -> Vec<u64> {
    let mut hi = [false; 32];
    let mut out = Vec::with_capacity(words.len());
    for &w in words {
        let op = w >> 26;
        if op == 3 {
            let t = (0x01E0_0000 & 0xF000_0000) | ((w & 0x3FF_FFFF) << 2);
            out.push(if (0x01E0_0000..0x01E0_8000).contains(&t) { 1 << 40 } else { u64::from(w) });
            continue;
        }
        if op == 0xF {
            let rt = ((w >> 16) & 31) as usize;
            if matches!(w & 0xFFFF, 0x1E0 | 0x1E1 | 0x1ED | 0x1EE) {
                hi[rt] = true;
                out.push((2 << 40) | rt as u64);
                continue;
            }
            hi[rt] = false;
        }
        let rs = ((w >> 21) & 31) as usize;
        if matches!(op, 9 | 0x20 | 0x21 | 0x23 | 0x24 | 0x25 | 0x27 | 0x28 | 0x29 | 0x2B | 0x31 | 0x39 | 0x37 | 0x3F | 0x1E | 0x1F) && hi[rs] {
            out.push((3 << 40) | u64::from(w & 0xFFFF_0000));
            continue;
        }
        out.push(u64::from(w));
    }
    out
}

/// Every BMODULE slot whose code holds the `words`-word function found at
/// `reference` (slot, address): (slot, address) [F static match].
pub fn instances(bmodule: &[u8], reference: (u16, u32), words: u32) -> Vec<(u16, u32)> {
    const RECORD: usize = 0x8000;
    let word_at = |slot: usize, o: usize| u32::from_le_bytes(bmodule[slot * RECORD + o..slot * RECORD + o + 4].try_into().unwrap());
    let n = words as usize;
    let ro = (reference.1 - 0x01E0_0000) as usize;
    let rw: Vec<u32> = (0..n).map(|i| word_at(usize::from(reference.0), ro + 4 * i)).collect();
    let want = normalized(&rw);
    let mut out = Vec::new();
    for slot in 0..bmodule.len() / RECORD {
        if bmodule[slot * RECORD..slot * RECORD + 4] != *b"MWo3" {
            continue;
        }
        let size = word_at(slot, 0xC) as usize;
        let end = (0x40 + size).min(RECORD) & !3;
        let mut o = 0x80;
        while o + 4 * n <= end {
            if normalized(&[word_at(slot, o)])[0] == want[0] {
                let ws: Vec<u32> = (0..n).map(|i| word_at(slot, o + 4 * i)).collect();
                if normalized(&ws) == want {
                    out.push((slot as u16, 0x01E0_0000 + o as u32));
                    o += 4 * n;
                    continue;
                }
            }
            o += 4;
        }
    }
    out
}

/// A battle as an attack effect sees it (the record-level harness's): the
/// attacker (role 0) facing its target 100 units away, the camera turning
/// slowly; skeleton nodes at fixed offsets from the animated positions.
pub struct Battle(pub usize);
impl EffectWorld for Battle {
    fn camera(&self) -> CameraMirror {
        let a = self.0 as f32 * 0.01;
        let eye = [120.0 * a.cos(), 60.0, -120.0 * a.sin()];
        CameraMirror { eye, target: [0.0, 15.0, 0.0], distance: 675.0, roll: 0.0 }
    }
    fn unit(&self, role: usize) -> UnitMirror {
        let z = if role == 0 { -50.0 } else { 50.0 };
        let t = self.0 as f32;
        UnitMirror {
            position: [0.0, 0.0, z],
            rotation: [0, if role == 0 { 0 } else { 0x8000 }, 0],
            animated: [0.0, (t * 0.1).sin() * 2.0, z * (1.0 - (t * 0.004).min(0.5))],
            motion: if role == 0 { 1 } else { 4 },
            time: t % 40.0,
            speed: 1.0,
            play_mode: 1,
            frozen: false,
            frames: [40, 40, 30, 30, 80, 20, 20, 20],
            alpha: 0x80,
            nodes: 64,
            model: 0,
            bounds: [0.0, 20.0, z, 25.0],
        }
    }
    fn node(&self, role: usize, node: u8) -> Option<[f32; 3]> {
        let u = self.unit(role);
        let n = f32::from(node);
        Some([u.animated[0] + (n * 0.7).sin() * 6.0, u.animated[1] + n * 1.5, u.animated[2] + (n * 0.3).cos() * 4.0])
    }
}

/// Run up to `records` of the slots carrying a function (spread over the
/// list) for up to `updates` updates each (both handle orders alternate,
/// rand re-seeded every update), watching the function with `port`.
pub fn check(exe: &[u8], bmodule: &[u8], textures: &EffectTextures, at: &[(u16, u32)], words: u32, port: PortFn, mutate: Option<Mutator>, records: usize, updates: usize, samples_per_record: usize) -> Report {
    let step = (at.len() as f64 / records.max(1) as f64).max(1.0);
    let picks: Vec<(u16, u32)> = (0..records).map(|k| (k as f64 * step) as usize).filter(|&i| i < at.len()).map(|i| at[i]).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    let total = Arc::new(Mutex::new(Report::default()));
    std::thread::scope(|s| {
        for (k, &(card, addr)) in picks.iter().enumerate() {
            let (port, total, mutate) = (port.clone(), total.clone(), mutate.clone());
            s.spawn(move || {
                let handles = if k % 2 == 0 { [0x101u16, 0x001] } else { [0x001, 0x101] };
                let Ok(Some(mut e)) = AttackEffect::load(exe, bmodule, card, handles, textures) else { return };
                let rep = Arc::new(Mutex::new(Report::default()));
                watch(&mut e, addr, words, handles, textures, port, mutate, rep.clone(), samples_per_record);
                for u in 0..updates {
                    let seed = 0x9E37_79B9_7F4A_7C15u64.wrapping_mul(u as u64 + 1 + 1000 * k as u64);
                    e.set_rand_state(seed);
                    if e.update(&Battle(u)).is_err() || e.finished {
                        break;
                    }
                    if rep.lock().unwrap().samples >= samples_per_record {
                        break;
                    }
                }
                total.lock().unwrap().merge(&rep.lock().unwrap());
            });
        }
    });
    let r = total.lock().unwrap().clone();
    r
}

/// Private checks against the user's import (`ROSES_TEST_GAME_DIR`).
#[cfg(test)]
mod private_tests {
    use super::*;
    use roses_fx::blib;

    struct Game {
        exe: Vec<u8>,
        bmodule: Vec<u8>,
        textures: EffectTextures,
    }

    fn game() -> Game {
        let dir = std::path::PathBuf::from(std::env::var_os("ROSES_TEST_GAME_DIR").expect("ROSES_TEST_GAME_DIR"));
        let exe = std::fs::read(dir.join("duel-arena/SLES_515.79")).unwrap();
        let bmodule = std::fs::read(dir.join("duel-arena/BMODULE.MRG")).unwrap();
        let tex = std::fs::read(dir.join("duel/TEXEFF.MRG")).unwrap();
        let texeff = match roses_assets::inspect_pal_opening_archive(&tex, roses_assets::OpeningArchiveKind::TexEff) {
            Ok(a) => a.records.iter().map(|r| tex.get(r.file_offset as usize..r.file_offset as usize + r.byte_len as usize).map(<[u8]>::to_vec)).collect(),
            Err(_) => Vec::new(),
        };
        Game { exe, bmodule, textures: EffectTextures { texeff, own: Vec::new(), other: Vec::new() } }
    }

    /// The port adapters: the registers at the call to each port's
    /// arguments.
    fn adapter(name: &str) -> PortFn {
        match name {
            "uv::uv_rect" => Arc::new(|fx, c| {
                blib::uv::uv_rect(fx, c.a(0), c.a(1), c.ai(2));
                Ret::None
            }),
            "shake::camera_shake" => Arc::new(|fx, c| Ret::V0(blib::shake::camera_shake(fx, c.a(0)) as u32)),
            "uv::uv_rect4" => Arc::new(|fx, c| {
                blib::uv::uv_rect4(fx, c.a(0), c.a(1), c.ai(2));
                Ret::None
            }),
            "spline::spline" => Arc::new(|fx, c| {
                let stale = std::array::from_fn(|i| f32::from_bits(c.fpr[i]));
                blib::spline::spline(fx, c.a(0), c.a(1), c.f(0), c.ai(2), stale);
                Ret::None
            }),
            "fade::fade" => Arc::new(|fx, c| Ret::V0(blib::fade::fade(fx, c.a(0), c.a(1)) as u32)),
            "fade::fade_rgb" => Arc::new(|fx, c| Ret::V0(blib::fade::fade_rgb(fx, c.a(0), c.a(1)) as u32)),
            "fade::fade_rgb_c" => Arc::new(|fx, c| Ret::V0(blib::fade::fade_rgb_c(fx, c.a(0), c.a(1)) as u32)),
            "fade::fade_c" => Arc::new(|fx, c| Ret::V0(blib::fade::fade_c(fx, c.a(0), c.a(1)) as u32)),
            "fade::fade_black" => Arc::new(|fx, c| Ret::V0(blib::fade::fade_black(fx, c.a(0), c.a(1)) as u32)),
            // ring_facing_flat with the push turned about z.
            "quads4::ring_facing_flat_z" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x1C4).expect("UV setter");
                blib::quads4::ring_facing_flat_z(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "spawn::node_history" => Arc::new(|fx, c| Ret::V0(blib::spawn::node_history(fx, c.a(0), c.ai(1), c.ai(2)) as u32)),
            "spawn::scatter" => Arc::new(|fx, c| Ret::V0(blib::spawn::scatter(fx, c.a(0), c.ai(1), c.f(0)) as u32)),
            "star::flash" => Arc::new(|fx, c| Ret::V0(blib::star::flash(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "star::flash_b" => Arc::new(|fx, c| Ret::V0(blib::star::flash_b(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "streak::ribbon" => Arc::new(|fx, c| Ret::V0(blib::streak::ribbon(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "sprite::bounce" => Arc::new(|fx, c| Ret::V0(blib::sprite::bounce(fx, c.a(0), c.a(1), c.ai(2)) as u32)),
            "trail::sparks" => Arc::new(|fx, c| Ret::V0(blib::trail::sparks(fx, c.a(0), c.a(1), c.a(2)) as u32)),
            "beam::beads" => Arc::new(|fx, c| Ret::V0(blib::beam::beads(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "beam::beads_at 34 3c" => Arc::new(|fx, c| Ret::V0(blib::beam::beads_at(fx, c.at, 0x34, 0x3C, c.a(0), c.a(1), c.ai(2)) as u32)),
            "beam::beads_at 40 44" => Arc::new(|fx, c| Ret::V0(blib::beam::beads_at(fx, c.at, 0x40, 0x44, c.a(0), c.a(1), c.ai(2)) as u32)),
            "burst::node_bursts" => Arc::new(|fx, c| Ret::V0(blib::burst::node_bursts(fx, c.a(0), c.ai(1)) as u32)),
            "burst::node_bursts_b" => Arc::new(|fx, c| Ret::V0(blib::burst::node_bursts_b(fx, c.a(0), c.ai(1)) as u32)),
            "burst::ring_spawn_b" => Arc::new(|fx, c| {
                blib::burst::ring_spawn_b(fx, c.a(0));
                Ret::None
            }),
            "burst::ring_spawn" => Arc::new(|fx, c| {
                blib::burst::ring_spawn(fx, c.a(0));
                Ret::None
            }),
            "arc::arcs" => Arc::new(|fx, c| Ret::V0(blib::arc::arcs(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "arc::arcs_b" => Arc::new(|fx, c| Ret::V0(blib::arc::arcs_b(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "spike::spikes" => Arc::new(|fx, c| Ret::V0(blib::spike::spikes(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "spike::spikes_b" => Arc::new(|fx, c| Ret::V0(blib::spike::spikes_b(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "spike::spikes_c" => Arc::new(|fx, c| Ret::V0(blib::spike::spikes_c(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "mote::motes" => Arc::new(|fx, c| Ret::V0(blib::mote::motes(fx, c.a(0), c.a(1), c.ai(2)) as u32)),
            "mote::motes_c" => Arc::new(|fx, c| Ret::V0(blib::mote::motes_c(fx, c.a(0), c.a(1), c.ai(2)) as u32)),
            "drop::drops" => Arc::new(|fx, c| Ret::V0(blib::drop::drops(fx, c.a(0), c.a(1), c.ai(2)) as u32)),
            // drops for 0x3C-byte scattering records.
            "drop::scatter_drops" => Arc::new(|fx, c| Ret::V0(blib::drop::scatter_drops(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "drop::scatter_drops_b" => Arc::new(|fx, c| Ret::V0(blib::drop::scatter_drops_b(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            // 
            "drop::drops_c" => Arc::new(|fx, c| Ret::V0(blib::drop::drops_c(fx, c.a(0), c.a(1), c.ai(2)) as u32)),
            "drop::drops_d" => Arc::new(|fx, c| Ret::V0(blib::drop::drops_d(fx, c.a(0), c.a(1), c.ai(2)) as u32)),
            "halo::halo" => Arc::new(|fx, c| Ret::V0(blib::halo::halo(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "cross::bars" => Arc::new(|fx, c| Ret::V0(blib::cross::bars(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            // bars rescheduled / unscheduled.
            "cross::bars_b" => Arc::new(|fx, c| Ret::V0(blib::cross::bars_b(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "cross::bars_c" => Arc::new(|fx, c| Ret::V0(blib::cross::bars_c(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "bolt::bolts" => Arc::new(|fx, c| Ret::V0(blib::bolt::bolts(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "bolt::bolts_b" => Arc::new(|fx, c| Ret::V0(blib::bolt::bolts_b(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            // 
            "plate::plates" => Arc::new(|fx, c| Ret::V0(blib::plate::plates(fx, c.at, c.a(0), c.a(1), c.a(2)) as u32)),
            "zap::zap" => Arc::new(|fx, c| Ret::V0(blib::zap::zap(fx, c.a(0), c.a(1), c.a(2), c.a(3), c.a(4)) as u32)),
            "quadtrail::push" => Arc::new(|fx, c| Ret::V0(blib::quadtrail::push(fx, c.a(0), c.a(1)) as u32)),
            "quadtrail::draw" => Arc::new(|fx, c| Ret::V0(blib::quadtrail::draw(fx, c.at, c.a(0), c.a(1), c.a(2)) as u32)),
            // 
            "pointquad::setup" => Arc::new(|fx, c| Ret::V0(blib::pointquad::setup(fx, c.at, c.a(0), c.a(1), c.a(2), c.f(0)) as u32)),
            "pointquad::draw" => Arc::new(|fx, c| Ret::V0(blib::pointquad::draw(fx, c.a(0), c.a(1), c.a(2), c.a(3)) as u32)),
            "pointquad::squares" => Arc::new(|fx, c| Ret::V0(blib::pointquad::squares(fx, c.a(0), c.a(1), c.a(2), c.a(3)) as u32)),
            "rain::colours2" => Arc::new(|fx, c| {
                blib::rain::colours2(fx, c.a(0), c.a(1), c.ai(2));
                Ret::None
            }),
            "rain::setup" => Arc::new(|fx, c| Ret::V0(blib::rain::setup(fx, c.at, c.a(0), c.ai(1), c.f(0)) as u32)),
            "rain::draw" => Arc::new(|fx, c| Ret::V0(blib::rain::draw(fx, c.at, c.a(0), c.a(1), c.a(2), c.f(0)) as u32)),
            "facing::quad" => Arc::new(|fx, c| Ret::V0(blib::facing::quad(fx, c.a(0), c.a(1), c.a(2), c.a(3)) as u32)),
            "wave::draw" => Arc::new(|fx, c| Ret::V0(blib::wave::draw(fx, c.a(0), c.a(1), c.a(2)) as u32)),
            // 
            "swath::push" => Arc::new(|fx, c| Ret::V0(blib::swath::push(fx, c.a(0), c.a(1), c.a(2)) as u32)),
            "swath::draw" => Arc::new(|fx, c| Ret::V0(blib::swath::draw(fx, c.a(0), c.a(1)) as u32)),
            "afterimage::strips" => Arc::new(|fx, c| Ret::V0(blib::afterimage::strips(fx, c.a(0), c.a(1)) as u32)),
            "hoop::draw" => Arc::new(|fx, c| Ret::V0(blib::hoop::draw(fx, c.at, c.a(0), c.a(1), c.a(2)) as u32)),
            "hoop::setup" => Arc::new(|fx, c| Ret::V0(blib::hoop::setup(fx, c.at, c.a(0)) as u32)),
            "glow::line" => Arc::new(|fx, c| Ret::V0(blib::glow::line(fx, c.a(0), c.a(1), c.a(2), c.a(3), c.a(4)) as u32)),
            "streamer::push" => Arc::new(|fx, c| Ret::V0(blib::streamer::push(fx, c.a(0), c.a(1), c.a(2)) as u32)),
            "streamer::draw" => Arc::new(|fx, c| Ret::V0(blib::streamer::draw(fx, c.at, c.a(0), c.a(1), c.a(2), c.a(3)) as u32)),
            "zoom::sprites" => Arc::new(|fx, c| Ret::V0(blib::zoom::sprites(fx, c.at, c.a(0), c.a(1)) as u32)),
            "zoom::quarters" => Arc::new(|fx, c| Ret::V0(blib::zoom::quarters(fx, c.a(0), c.a(1)) as u32)),
            "strand::draw" => Arc::new(|fx, c| Ret::V0(blib::strand::draw(fx, c.at, c.a(0), c.a(1), c.a(2), c.a(3), c.a(4)) as u32)),
            "sphere::draw" => Arc::new(|fx, c| Ret::V0(blib::sphere::draw(fx, c.a(0), c.a(1), c.a(2)) as u32)),
            "sphere::setup" => Arc::new(|fx, c| Ret::V0(blib::sphere::setup(fx, c.a(0), c.a(1), c.f(0)) as u32)),
            // 
            "sphere::draw_b" => Arc::new(|fx, c| Ret::V0(blib::sphere::draw_b(fx, c.a(0), c.a(1), c.a(2)) as u32)),
            "sphere::setup_b" => Arc::new(|fx, c| Ret::V0(blib::sphere::setup_b(fx, c.a(0), c.a(1), c.f(0)) as u32)),
            "blade::setup" => Arc::new(|fx, c| Ret::V0(blib::blade::setup(fx, c.at, c.a(0), c.a(1)) as u32)),
            "blade::draw" => Arc::new(|fx, c| Ret::V0(blib::blade::draw(fx, c.a(0), c.a(1), c.a(2)) as u32)),
            // 
            "sheet::sheets" => Arc::new(|fx, c| Ret::V0(blib::sheet::sheets(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "sheet::sheets_c" => Arc::new(|fx, c| Ret::V0(blib::sheet::sheets_c(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "sheet::tiles" => Arc::new(|fx, c| Ret::V0(blib::sheet::tiles(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "emit::bead_trails" => Arc::new(|fx, c| Ret::V0(blib::emit::bead_trails(fx, c.at, c.a(0), c.ai(2)) as u32)),
            "emit::bead_trails_d" => Arc::new(|fx, c| Ret::V0(blib::emit::bead_trails_d(fx, c.at, c.a(0), c.ai(2)) as u32)),
            "emit::bead_trails_e" => Arc::new(|fx, c| Ret::V0(blib::emit::bead_trails_e(fx, c.at, c.a(0), c.ai(2)) as u32)),
            "emit::segment_spawns" => Arc::new(|fx, c| Ret::V0(blib::emit::segment_spawns(fx, c.at, c.a(0), c.ai(2)) as u32)),
            "emit::segment_spawns_b" => Arc::new(|fx, c| Ret::V0(blib::emit::segment_spawns_b(fx, c.at, c.a(0), c.ai(2)) as u32)),
            // the saves are the caller's registers.
            "spawn::task400" => Arc::new(|fx, c| {
                let g = |r: usize| c.gpr[r];
                let saves = [(0x50, g(31)), (0x40, g(20)), (0x30, g(19)), (0x20, g(18)), (0x10, g(17)), (0, g(16))];
                Ret::V0(blib::spawn::task400(fx, c.a(0), c.a(1), c.a(2), &saves) as u32)
            }),
            // the saves are the caller's registers.
            "spawn::task200" => Arc::new(|fx, c| {
                let g = |r: usize| c.gpr[r];
                let saves = [(0x50, g(31)), (0x40, g(20)), (0x30, g(19)), (0x20, g(18)), (0x10, g(17)), (0, g(16))];
                Ret::V0(blib::spawn::task200(fx, c.a(0), c.a(1), c.a(2), &saves) as u32)
            }),
            "spawn::task300" => Arc::new(|fx, c| {
                let g = |r: usize| c.gpr[r];
                let saves = [(0x50, g(31)), (0x40, g(20)), (0x30, g(19)), (0x20, g(18)), (0x10, g(17)), (0, g(16))];
                Ret::V0(blib::spawn::task300(fx, c.a(0), c.a(1), c.a(2), &saves) as u32)
            }),
            "spawn::task400_b4" => Arc::new(|fx, c| {
                let g = |r: usize| c.gpr[r];
                let saves = [(0x50, g(31)), (0x40, g(20)), (0x30, g(19)), (0x20, g(18)), (0x10, g(17)), (0, g(16))];
                Ret::V0(blib::spawn::task400_b4(fx, c.a(0), c.a(1), c.a(2), &saves) as u32)
            }),
            // 
            "flicker::flickers" => Arc::new(|fx, c| Ret::V0(blib::flicker::flickers(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "band::bands" => Arc::new(|fx, c| Ret::V0(blib::band::bands(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "band::bands_b" => Arc::new(|fx, c| Ret::V0(blib::band::bands_b(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "band::bands_c" => Arc::new(|fx, c| Ret::V0(blib::band::bands_c(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "emit::bead_trails_b" => Arc::new(|fx, c| Ret::V0(blib::emit::bead_trails_b(fx, c.at, c.a(0), c.ai(2)) as u32)),
            "emit::bead_trails_c" => Arc::new(|fx, c| Ret::V0(blib::emit::bead_trails_c(fx, c.at, c.a(0), c.ai(2)) as u32)),
            // 
            "arch::arches" => Arc::new(|fx, c| Ret::V0(blib::arch::arches(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "arch::arches_b" => Arc::new(|fx, c| Ret::V0(blib::arch::arches_b(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "arch::arches_c" => Arc::new(|fx, c| Ret::V0(blib::arch::arches_c(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "spray::sprays" => Arc::new(|fx, c| Ret::V0(blib::spray::sprays(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "spray::sprays_b" => Arc::new(|fx, c| Ret::V0(blib::spray::sprays_b(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "band::bands_d" => Arc::new(|fx, c| Ret::V0(blib::band::bands_d(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "cone::cones" => Arc::new(|fx, c| Ret::V0(blib::cone::cones(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "cone::cones_b" => Arc::new(|fx, c| Ret::V0(blib::cone::cones_b(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "cone::cones_c" => Arc::new(|fx, c| Ret::V0(blib::cone::cones_c(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "dash::dashes" => Arc::new(|fx, c| Ret::V0(blib::dash::dashes(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            // 
            "swirl::arms" => Arc::new(|fx, c| Ret::V0(blib::swirl::arms(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "swirl::arms_c" => Arc::new(|fx, c| Ret::V0(blib::swirl::arms_c(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "ray::rays" => Arc::new(|fx, c| Ret::V0(blib::ray::rays(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            // 
            "spokes::lines" => Arc::new(|fx, c| Ret::V0(blib::spokes::lines(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "ray::rays_b" => Arc::new(|fx, c| Ret::V0(blib::ray::rays_b(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "ray::rays_c" => Arc::new(|fx, c| Ret::V0(blib::ray::rays_c(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "comet::comets" => Arc::new(|fx, c| Ret::V0(blib::comet::comets(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "comet::comets_b" => Arc::new(|fx, c| Ret::V0(blib::comet::comets_b(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "comet::comets_c" => Arc::new(|fx, c| Ret::V0(blib::comet::comets_c(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "flatquad::quads" => Arc::new(|fx, c| Ret::V0(blib::flatquad::quads(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "flatquad::lines" => Arc::new(|fx, c| Ret::V0(blib::flatquad::lines(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "iris::rings" => Arc::new(|fx, c| Ret::V0(blib::iris::rings(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "iris::rings_b" => Arc::new(|fx, c| Ret::V0(blib::iris::rings_b(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "wedge::fans" => Arc::new(|fx, c| Ret::V0(blib::wedge::fans(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "annulus::flat" => Arc::new(|fx, c| Ret::V0(blib::annulus::flat(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "dash::strokes" => Arc::new(|fx, c| Ret::V0(blib::dash::strokes(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "dash::strokes_b" => Arc::new(|fx, c| Ret::V0(blib::dash::strokes_b(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "sweep::sweeps" => Arc::new(|fx, c| Ret::V0(blib::sweep::sweeps(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "scrollquad::quads" => Arc::new(|fx, c| Ret::V0(blib::scrollquad::quads(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "haze::screen" => Arc::new(|fx, c| Ret::V0(blib::haze::screen(fx, c.at, c.a(0), c.a(1), c.ai(2), c.a(3)) as u32)),
            "segtube::frustums" => Arc::new(|fx, c| Ret::V0(blib::segtube::frustums(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "ray::arc_rays" => Arc::new(|fx, c| Ret::V0(blib::ray::arc_rays(fx, c.at, c.a(0), c.ai(2)) as u32)),
            "ray::arc_rays_c" => Arc::new(|fx, c| Ret::V0(blib::ray::arc_rays_c(fx, c.at, c.a(0), c.ai(2)) as u32)),
            // f22 is the caller's (the original never sets it).
            "rod::rods" => Arc::new(|fx, c| Ret::V0(blib::rod::rods(fx, c.at, c.a(0), c.a(1), c.ai(2), f32::from_bits(c.fpr[22])) as u32)),
            "rod::rods_b" => Arc::new(|fx, c| Ret::V0(blib::rod::rods_b(fx, c.at, c.a(0), c.a(1), c.ai(2), f32::from_bits(c.fpr[22])) as u32)),
            "rod::rods_c" => Arc::new(|fx, c| Ret::V0(blib::rod::rods_c(fx, c.at, c.a(0), c.a(1), c.ai(2), f32::from_bits(c.fpr[22])) as u32)),
            "segtube::tubes" => Arc::new(|fx, c| Ret::V0(blib::segtube::tubes(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "dome::domes" => Arc::new(|fx, c| Ret::V0(blib::dome::domes(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "dome::domes_550" => Arc::new(|fx, c| Ret::V0(blib::dome::domes_550(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            // a3 is reported with 0x207EF0's call.
            "window::frame_fade" => Arc::new(|fx, c| Ret::V0(blib::window::frame_fade(fx, c.at, c.a(0), c.a(1), c.ai(2), c.a(3)) as u32)),
            "radiate::streaks" => Arc::new(|fx, c| Ret::V0(blib::radiate::streaks(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            // 
            "radiate::streaks_b" => Arc::new(|fx, c| Ret::V0(blib::radiate::streaks_b(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            // 
            "radiate::streaks_c" => Arc::new(|fx, c| Ret::V0(blib::radiate::streaks_c(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            // v0 is what its callees left (not compared).
            "spawn::fan_ring" => Arc::new(|fx, c| {
                blib::spawn::fan_ring(fx, c.at, c.a(0));
                Ret::None
            }),
            "spawn::emitters" => Arc::new(|fx, c| Ret::V0(blib::spawn::emitters(fx, c.at, c.a(0), c.ai(1)) as u32)),
            "sprite::sparks" => Arc::new(|fx, c| Ret::V0(blib::sprite::sparks(fx, c.a(0), c.a(1), c.ai(2)) as u32)),
            "window::frame_fade_b" => Arc::new(|fx, c| Ret::V0(blib::window::frame_fade_b(fx, c.at, c.a(0), c.a(1), c.ai(2), c.a(3)) as u32)),
            "window::flat" => Arc::new(|fx, c| Ret::V0(blib::window::flat(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "dash::ribbons" => Arc::new(|fx, c| Ret::V0(blib::dash::ribbons(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            // 
            "window::frame_fade_c" => Arc::new(|fx, c| Ret::V0(blib::window::frame_fade_c(fx, c.at, c.a(0), c.a(1), c.ai(2), c.a(3)) as u32)),
            // a3 is reported with 0x207EF0's call.
            "window::frame_rays" => Arc::new(|fx, c| Ret::V0(blib::window::frame_rays(fx, c.at, c.a(0), c.a(1), c.ai(2), c.a(3)) as u32)),
            "window::frame_rays_c" => Arc::new(|fx, c| Ret::V0(blib::window::frame_rays_c(fx, c.at, c.a(0), c.a(1), c.ai(2), c.a(3)) as u32)),
            // the caller's f20 / f21 are saved at sp + 0 / + 4.
            "window::frame_rays_b" => Arc::new(|fx, c| Ret::V0(blib::window::frame_rays_b(fx, c.at, c.a(0), c.a(1), c.ai(2), c.a(3), [c.fpr[20], c.fpr[21]]) as u32)),
            // 
            "panel::panels" => Arc::new(|fx, c| Ret::V0(blib::panel::panels(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "smear::smears" => Arc::new(|fx, c| Ret::V0(blib::smear::smears(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            // 
            "smear::smears_a" => Arc::new(|fx, c| Ret::V0(blib::smear::smears_a(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            // 
            "smear::smears_c" => Arc::new(|fx, c| Ret::V0(blib::smear::smears_c(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "smear::smears_d" => Arc::new(|fx, c| Ret::V0(blib::smear::smears_d(fx, c.at, c.a(0), c.a(1), c.ai(2)) as u32)),
            "spin::motes" => Arc::new(|fx, c| {
                blib::spin::motes(fx, c.a(0), c.a(1), c.a(2), c.a(3));
                Ret::None
            }),
            "spin::strips" => Arc::new(|fx, c| Ret::V0(blib::spin::strips(fx, c.at, c.a(0), c.a(1), c.a(2), c.a(3)) as u32)),
            // the drifting ribbon (g, pts, p) / (ot, g, pts, x).
            "driftribbon::push" => Arc::new(|fx, c| Ret::V0(blib::driftribbon::push(fx, c.a(0), c.a(1), c.a(2)) as u32)),
            "driftribbon::draw" => Arc::new(|fx, c| Ret::V0(blib::driftribbon::draw(fx, c.a(0), c.a(1), c.a(2), c.a(3)) as u32)),
            // the frame flash (c), the first TEXFLUSH reported with
            // the caller's a1..a3.
            "flash::frame_flash" => Arc::new(|fx, c| Ret::V0(blib::flash::frame_flash(fx, c.a(0), [c.a(1), c.a(2), c.a(3)]) as u32)),
            // (ot, g, x).
            "flatsprite::sprites" => Arc::new(|fx, c| Ret::V0(blib::flatsprite::sprites(fx, c.a(0), c.a(1), c.a(2)) as u32)),
            // the frame quad of the colour at a0.
            "framegs::frame_quad" => Arc::new(|fx, c| {
                fx.frame_quad(c.a(0));
                Ret::V0(0)
            }),
            // the first TEXFLUSH reports the caller's a3.
            "radiate::echoes" => Arc::new(|fx, c| Ret::V0(blib::radiate::echoes(fx, c.at, c.a(0), c.a(1), c.ai(2), c.a(3)) as u32)),
            "radiate::echoes_b" => Arc::new(|fx, c| Ret::V0(blib::radiate::echoes_b(fx, c.at, c.a(0), c.a(1), c.ai(2), c.a(3)) as u32)),
            // s0 / a3 the caller's.
            "radiate::jitter" => Arc::new(|fx, c| Ret::V0(blib::radiate::jitter(fx, c.at, c.a(0), c.a(1), c.ai(2), c.gpr[16] as u32, c.a(3)) as u32)),
            // the record's UV setter from its `jal` (at + 0xF0 /
            // + 0x100).
            "billboard::ring_quads" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0xF0).expect("UV setter");
                blib::billboard::ring_quads(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            // 
            "billboard::ring_quads_tall" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0xF0).expect("UV setter");
                blib::billboard::ring_quads_tall(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            // 
            "billboard::ring_quads_turned" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0xF0).expect("UV setter");
                blib::billboard::ring_quads_turned(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            // 
            "billboard::ring_quads_angled" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0xF0).expect("UV setter");
                blib::billboard::ring_quads_angled(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            // 
            "billboard::ring_quads_yx" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0xF0).expect("UV setter");
                blib::billboard::ring_quads_yx(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "billboard::ring_points" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x1C8).expect("UV setter");
                blib::billboard::ring_points(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "uv::grid_rect" => Arc::new(|fx, c| {
                blib::uv::grid_rect(fx, c.a(0), c.ai(2));
                Ret::None
            }),
            "billboard::quad" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x100).expect("UV setter");
                blib::billboard::quad(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            // quad standing on its lower edge.
            "billboard::quad_standing" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x100).expect("UV setter");
                blib::billboard::quad_standing(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            // quad with the 0x42 / 0x44 / 0x48 blends.
            "billboard::quad_b" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x11C).expect("UV setter");
                blib::billboard::quad_b(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            // quad_b turned about z by +0x5A, submitted twice.
            "billboard::quad_b_turned2" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x11C).expect("UV setter");
                blib::billboard::quad_b_turned2(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            // quad_b in the 0x1B0 frame, submitted twice.
            "billboard::quad_b_twice" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x11C).expect("UV setter");
                blib::billboard::quad_b_twice(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            // quad_b turned about z by +0x5A, submitted once.
            "billboard::quad_b_turned" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x11C).expect("UV setter");
                blib::billboard::quad_b_turned(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "billboard::sprite_spin" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x10C).expect("UV setter");
                blib::billboard::sprite_spin(fx, c.at, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "billboard::ring_facing" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x1CC).expect("UV setter");
                blib::billboard::ring_facing(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "billboard::arc_strips" => Arc::new(|fx, c| {
                blib::billboard::arc_strips(fx, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "billboard::uv_rect_q" => Arc::new(|fx, c| {
                blib::billboard::uv_rect_q(fx, c.a(0), c.a(1), c.ai(2));
                Ret::None
            }),
            // 
            "quads4::sprite_quads" => Arc::new(|fx, c| {
                blib::quads4::sprite_quads(fx, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "quads4::sparks" => Arc::new(|fx, c| {
                blib::quads4::sparks(fx, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "quads4::ring_facing_wh" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x1B4).expect("UV setter");
                blib::quads4::ring_facing_wh(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "quads4::ring_quads_z" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0xF0).expect("UV setter");
                blib::quads4::ring_quads_z(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "quads4::ring_at" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0xF0).expect("UV setter");
                blib::quads4::ring_at(fx, uv, c.a(0), c.a(1), c.a(2), c.a(3));
                Ret::None
            }),
            // the UV setter's `jal` at +0x1D0.
            // ring_at_m with the setter called once (at +0x1C4).
            "quads4::ring_at_m_uv1" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x1C4).expect("UV setter");
                blib::quads4::ring_at_m_uv1(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "quads4::ring_at_m" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x1D0).expect("UV setter");
                blib::quads4::ring_at_m(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            // ring_facing without the z turn (setter at +0x1C4)
            // and the full-window gouraud tint (w in f12).
            "quads4::ring_facing_flat" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x1C4).expect("UV setter");
                blib::quads4::ring_facing_flat(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "quads4::quad_turned_f" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x100).expect("UV setter");
                blib::quads4::quad_turned_f(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "quads4::arc_strips_52" => Arc::new(|fx, c| {
                blib::quads4::arc_strips_52(fx, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            // the 18-strip camera-facing band (depth = a2) and
            // the four far-Z quarters into the caller's descriptor.
            "quads4::band18" => Arc::new(|fx, c| {
                blib::quads4::band18(fx, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "quads4::band18_turned" => Arc::new(|fx, c| {
                blib::quads4::band18_turned(fx, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "quads4::quarters_far" => Arc::new(|fx, c| {
                blib::quads4::quarters_far(fx, c.a(0), c.a(1), c.a(2), c.a(3));
                Ret::None
            }),
            "quads4::quarters_far_at" => Arc::new(|fx, c| {
                blib::quads4::quarters_far_at(fx, c.a(0), c.a(1), c.a(2));
                Ret::None
            }),
            "quads4::fan18" => Arc::new(|fx, c| {
                blib::quads4::fan18(fx, c.a(0), c.a(1), c.a(2));
                Ret::None
            }),
            "quads4::fan18_far" => Arc::new(|fx, c| {
                blib::quads4::fan18_far(fx, c.a(0), c.a(1), c.a(2));
                Ret::None
            }),
            "quads4::fan18_t" => Arc::new(|fx, c| {
                blib::quads4::fan18_t(fx, c.a(0), c.a(1), c.a(2));
                Ret::None
            }),
            // fan18 at the far Z key 0, the 9-sliver band.
            "quads4::fan18_far0" => Arc::new(|fx, c| {
                blib::quads4::fan18_far0(fx, c.a(0), c.a(1), c.a(2));
                Ret::None
            }),
            "quads4::band9_tex" => Arc::new(|fx, c| {
                blib::quads4::band9_tex(fx, c.a(0), c.a(1), c.a(2));
                Ret::None
            }),
            // a spec-driven camera-facing quad (ot, task, spec)
            // and the textured 18-quad band (D, task, desc).
            "quads4::sprite_spec" => Arc::new(|fx, c| {
                blib::quads4::sprite_spec(fx, c.a(0), c.a(1), c.a(2));
                Ret::None
            }),
            "quads4::band18_tex" => Arc::new(|fx, c| {
                blib::quads4::band18_tex(fx, c.a(0), c.a(1), c.a(2));
                Ret::None
            }),
            "quads4::ring_band_t" => Arc::new(|fx, c| {
                blib::quads4::ring_band_t(fx, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "quads4::ring_band_t20" => Arc::new(|fx, c| {
                blib::quads4::ring_band_t20(fx, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "spawn::disc_step" => Arc::new(|fx, c| Ret::V0(blib::spawn::disc_step(fx, c.at, c.a(1)) as u32)),
            // task steps (D, t) with rand frames from the code.
            "spawn::ring_rise" => Arc::new(|fx, c| Ret::V0(blib::spawn::ring_rise(fx, c.at, c.a(1)) as u32)),
            "spawn::flare_step" => Arc::new(|fx, c| Ret::V0(blib::spawn::flare_step(fx, c.at, c.a(1)) as u32)),
            // the constant frame quad (no arguments).
            "framegs::frame_quad_rgba" => Arc::new(|fx, _c| {
                fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
                Ret::V0(0)
            }),
            // the sprite's UV setter `jal` at +0x118.
            "quads4::sprite_turned" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x118).expect("UV setter");
                blib::quads4::sprite_turned(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            // 
            "plank::draw" => Arc::new(|fx, c| Ret::V0(blib::plank::draw(fx, c.a(0), c.a(1), c.a(2)) as u32)),
            // the UV setter `jal` at +0x118 as sprite_turned's.
            "quads4::sprite_turned_at" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x118).expect("UV setter");
                blib::quads4::sprite_turned_at(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            // 
            // the textured ribbon between two histories.
            "spline::ribbon_tex" => Arc::new(|fx, c| {
                blib::spline::ribbon_tex(fx, c.a(0), c.a(1), c.a(2), c.a(3));
                Ret::None
            }),
            // small task steps / a spawn.
            "tstep::blue_glow" => Arc::new(|fx, c| Ret::V0(blib::tstep::blue_glow(fx, c.a(1)) as u32)),
            "tstep::spawn" => Arc::new(|fx, c| Ret::V0(blib::tstep::spawn(fx, c.a(0), c.a(1), c.a(2)) as u32)),
            "tstep::white_flash" => Arc::new(|fx, c| Ret::V0(blib::tstep::white_flash(fx, c.a(1)) as u32)),
            "tstep::ember" => Arc::new(|fx, c| Ret::V0(blib::tstep::ember(fx, c.at, c.a(1)) as u32)),
            "tstep::spark" => Arc::new(|fx, c| Ret::V0(blib::tstep::spark(fx, c.at, c.a(1)) as u32)),
            "cloud::cloud" => Arc::new(|fx, c| {
                blib::cloud::cloud(fx, c.a(0), c.a(1), c.a(2), c.a(3));
                Ret::None
            }),
            "spline::ribbon_tex1" => Arc::new(|fx, c| {
                blib::spline::ribbon_tex1(fx, c.a(0), c.a(1), c.a(2), c.a(3));
                Ret::None
            }),
            "spline::ribbon_cross" => Arc::new(|fx, c| {
                blib::spline::ribbon_cross(fx, c.a(1), c.a(2));
                Ret::None
            }),
            "spline::ribbon_g" => Arc::new(|fx, c| {
                blib::spline::ribbon_g(fx, c.a(1), c.a(2), c.a(3));
                Ret::None
            }),
            "spline::tube" => Arc::new(|fx, c| {
                blib::spline::tube(fx, c.at, c.a(0), c.a(1), c.a(2), c.ai(3));
                Ret::None
            }),
            "spawn::history" => Arc::new(|fx, c| Ret::V0(blib::spawn::history(fx, c.a(0), c.ai(1), c.a(3)) as u32)),
            "quads4::ring_band" => Arc::new(|fx, c| {
                blib::quads4::ring_band(fx, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            // ring_band with a float outer radius; arc_quads
            // with the record's UV setter from its `jal` (at + 0x17C).
            "billboard::arc_strips_flat" => Arc::new(|fx, c| {
                blib::billboard::arc_strips_flat(fx, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "billboard::ring_t" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0xC8).expect("UV setter");
                blib::billboard::ring_t(fx, uv, c.a(0), c.a(1), c.a(2), c.a(3));
                Ret::None
            }),
            "billboard::ring_t20" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0xC4).expect("UV setter");
                blib::billboard::ring_t20(fx, uv, c.a(0), c.a(1), c.a(2), c.a(3));
                Ret::None
            }),
            "quads4::ring_band_f" => Arc::new(|fx, c| {
                blib::quads4::ring_band_f(fx, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "tubetrail::tube" => Arc::new(|fx, c| {
                blib::tubetrail::tube(fx, c.a(0), c.a(1), c.a(2), c.ai(3));
                Ret::None
            }),
            "tubetrail::tube_trail3" => Arc::new(|fx, c| {
                blib::tubetrail::tube_with(fx, &blib::tubetrail::TRAIL3, c.a(0), c.a(1), c.a(2), c.ai(3));
                Ret::None
            }),
            "tubetrail::tube_cam" => Arc::new(|fx, c| {
                blib::tubetrail::tube_cam(fx, c.a(0), c.a(1), c.a(2), c.ai(3));
                Ret::None
            }),
            "tubetrail::tube_const" => Arc::new(|fx, c| {
                blib::tubetrail::tube_const(fx, c.a(0), c.a(1), c.a(2), c.ai(3));
                Ret::None
            }),
            "tubetrail::tube_single" => Arc::new(|fx, c| {
                blib::tubetrail::tube_with(fx, &blib::tubetrail::SINGLE, c.a(0), c.a(1), c.a(2), 0);
                Ret::None
            }),
            "tubetrail::tube_gouraud" => Arc::new(|fx, c| {
                blib::tubetrail::tube_gouraud(fx, &blib::tubetrail::GOURAUD4, c.a(0), c.a(1), c.a(2), c.ai(3));
                Ret::None
            }),
            "tubetrail::tube_gouraud2" => Arc::new(|fx, c| {
                blib::tubetrail::tube_gouraud(fx, &blib::tubetrail::GOURAUD2, c.a(0), c.a(1), c.a(2), c.ai(3));
                Ret::None
            }),
            "tubetrail::tube_gouraud2_44" => Arc::new(|fx, c| {
                blib::tubetrail::tube_gouraud(fx, &blib::tubetrail::GOURAUD2_44, c.a(0), c.a(1), c.a(2), c.ai(3));
                Ret::None
            }),
            "tubetrail::tube_single_blend" => Arc::new(|fx, c| {
                blib::tubetrail::tube_with(fx, &blib::tubetrail::SINGLE_BLEND, c.a(0), c.a(1), c.a(2), c.ai(3));
                Ret::None
            }),
            "quads4::ring_band_stq" => Arc::new(|fx, c| {
                blib::quads4::ring_band_stq(fx, c.at, c.a(0), c.a(1), c.a(2), c.ai(3));
                Ret::None
            }),
            "quads4::ring_band_fy20" => Arc::new(|fx, c| {
                blib::quads4::ring_band_fy20(fx, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "quads4::ring_band_fy" => Arc::new(|fx, c| {
                blib::quads4::ring_band_fy(fx, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "billboard::arc_quads" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x17C).expect("UV setter");
                blib::billboard::arc_quads(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "quads4::arc_strips_tex" => Arc::new(|fx, c| {
                blib::quads4::arc_strips_tex(fx, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "quads4::tint4" => Arc::new(|fx, c| {
                blib::quads4::tint4(fx, c.a(1), c.ai(2), c.f(0));
                Ret::None
            }),
            // the same, its setter's `jal` at +0x1F4.
            "quads4::ring_at_mt" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x1F4).expect("UV setter");
                blib::quads4::ring_at_mt(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "quads4::quad_turned" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x100).expect("UV setter");
                blib::quads4::quad_turned(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            // 
            "quads4::quad_turned_raised" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x100).expect("UV setter");
                blib::quads4::quad_turned_raised(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "quads4::ring_into" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0xCC).expect("UV setter");
                blib::quads4::ring_into(fx, uv, c.a(0), c.a(1), c.a(2), c.a(3));
                Ret::None
            }),
            "quads4::ring_facing_i" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x1AC).expect("UV setter");
                blib::quads4::ring_facing_i(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            // 
            "quads4::ring_facing_m" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x1E4).expect("UV setter");
                blib::quads4::ring_facing_m(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            // Phase 1's library (`roses_fx::fxlib`, proven in c620 / b262),
            // checked here across the B records too.
            "fxlib::colours4" => Arc::new(|fx, c| {
                fx.colours4(c.a(0), c.a(1));
                Ret::None
            }),
            "fxlib::disc" => Arc::new(|fx, c| {
                fx.disc(c.a(1), c.a(2));
                Ret::None
            }),
            "fxlib::sprites" => Arc::new(|fx, c| {
                fx.sprites(c.a(1), c.a(2));
                Ret::None
            }),
            "fxlib::ring_draw" => Arc::new(|fx, c| {
                fx.ring_draw(c.a(1), c.a(2));
                Ret::None
            }),
            "fxlib::quads_draw" => Arc::new(|fx, c| {
                fx.quads_draw(c.a(1), c.a(2));
                Ret::None
            }),
            "fxlib::sprites_spawn" => Arc::new(|fx, c| {
                fx.sprites_spawn(c.a(0), c.f(0));
                Ret::None
            }),
            "fxlib::ring_setup" => Arc::new(|fx, c| {
                fx.ring_setup(c.a(0), c.a(1), c.f(0), c.f(1), c.f(2), c.f(3));
                Ret::None
            }),
            // the ribbon (phase 2's `roses_fx::ribbon`).
            "fxlib::ribbon_push" => Arc::new(|fx, c| {
                fx.ribbon_push(c.a(0), c.a(1), c.a(2));
                Ret::V0(0)
            }),
            "fxlib::ribbon_draw" => Arc::new(|fx, c| {
                fx.ribbon_draw(c.a(1), c.a(2), c.a(3));
                Ret::None
            }),
            "billboard::sprite_square" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x10C).expect("UV setter");
                blib::billboard::sprite_square(fx, c.at, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "billboard::sprite_spin_40" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x10C).expect("UV setter");
                blib::billboard::sprite_spin_40(fx, c.at, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "quads4::ring_spun" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0xD4).expect("UV setter");
                blib::quads4::ring_spun(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            // the disc family's 0x40-task steps.
            "task40::camera" => Arc::new(|fx, c| Ret::V0(blib::task40::camera(fx, c.at, c.a(0), c.a(1)) as u32)),
            "task40::camera_z4" => Arc::new(|fx, c| Ret::V0(blib::task40::camera_z4(fx, c.at, c.a(0), c.a(1)) as u32)),
            "task40::camera_far" => Arc::new(|fx, c| Ret::V0(blib::task40::camera_far(fx, c.at, c.a(0), c.a(1)) as u32)),
            // card 412's camera block D+0x2870, timer D+0x2DF0.
            "task40::camera_at" => Arc::new(|fx, c| Ret::V0(blib::task40::camera_at(fx, c.at, c.a(0), c.a(1), 0x2870, 0x2DF0) as u32)),
            "task40::band_grow" => Arc::new(|fx, c| Ret::V0(blib::task40::band_grow(fx, c.at, c.a(0), c.a(1)) as u32)),
            // card 417 / 596's strip ring step and draw.
            "task40::ring_strip" => Arc::new(|fx, c| Ret::V0(blib::task40::ring_strip(fx, c.at, c.a(0), c.a(1)) as u32)),
            "billboard::ring_strip3" => Arc::new(|fx, c| {
                blib::billboard::ring_strip3(fx, c.a(0), c.a(1), c.a(2), c.a(3));
                Ret::None
            }),
            "quads4::ring_ticks" => Arc::new(|fx, c| {
                blib::quads4::ring_ticks(fx, c.a(0), c.a(1), c.ai(2));
                Ret::None
            }),
            "billboard::sprite_drift" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0xE8).expect("UV setter");
                blib::billboard::sprite_drift(fx, c.at, uv, c.a(0), c.a(1), c.ai(2));
                Ret::None
            }),
            "billboard::quad_cross" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x114).expect("UV setter");
                blib::billboard::quad_cross(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "task40::mote_target" => Arc::new(|fx, c| Ret::V0(blib::task40::mote_target(fx, c.at, c.a(0), c.a(1)) as u32)),
            "task40::mote_node" => Arc::new(|fx, c| Ret::V0(blib::task40::mote_node(fx, c.at, c.a(0), c.a(1)) as u32)),
            "quads4::ring_spun_axis_z" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x17C).expect("UV setter");
                blib::quads4::ring_spun_axis_z(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "quads4::ring_spun_blend" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x17C).expect("UV setter");
                blib::quads4::ring_spun_blend(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "quads4::ring_spun_axis" => Arc::new(|fx, c| {
                let uv = blib::billboard::uv_callee(fx, c.at + 0x17C).expect("UV setter");
                blib::quads4::ring_spun_axis(fx, uv, c.a(0), c.a(1), c.ai(2), c.a(3));
                Ret::None
            }),
            "quads4::ring_band_task" => Arc::new(|fx, c| {
                blib::quads4::ring_band_task(fx, c.at, c.a(0), c.a(1), c.a(2));
                Ret::None
            }),
            // 
            "ribbon8::ribbon" => Arc::new(|fx, c| Ret::V0(blib::ribbon8::ribbon(fx, c.a(0), c.a(1), c.a(2), c.a(3)))),
            "ribbon8::strip_colours" => Arc::new(|fx, c| {
                blib::ribbon8::strip_colours(fx, c.a(0), c.a(1), c.ai(2));
                Ret::None
            }),
            // the layout variants (the hash after the name).
            n if n.starts_with("spawn::task400_b4 ") => {
                let p = blib::spawn::pool70_b4(&n[18..]).expect("layout");
                Arc::new(move |fx, c| {
                    let g = |r: usize| c.gpr[r];
                    let saves = [(0x50, g(31)), (0x40, g(20)), (0x30, g(19)), (0x20, g(18)), (0x10, g(17)), (0, g(16))];
                    Ret::V0(blib::spawn::task70(fx, c.a(0), p, c.a(1), c.a(2), &saves) as u32)
                })
            }
            n if n.starts_with("spawn::disc_step ") => {
                let k = blib::spawn::disc(&n[17..]).expect("constants");
                Arc::new(move |fx, c| Ret::V0(blib::spawn::disc_step_with(fx, k, c.at, c.a(1)) as u32))
            }
            _ => panic!("no adapter for {name}"),
        }
    }

    /// A small deterministic hash of a seed.
    fn mix(seed: u64, k: u64) -> u64 {
        let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ k.wrapping_mul(0xD1B5_4A32_D192_ED03);
        x ^= x >> 29;
        x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x ^ (x >> 32)
    }

    /// Perturbations per port: the modes and inputs real calls do not
    /// reach.
    fn mutator(name: &str) -> Option<Mutator> {
        Some(match name {
            // modes, blends, layouts and orientations (the data
            // area's D+0xB0 layout 0..2, D+0xBE orientation 0..9).
            // colour modes 0..2 (mode >> 4), orientations 0..9.
            "quads4::sprite_quads" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[6][0] = ((mix(s, 1) % 4) | ((mix(s, 2) % 3) << 4)) as u64;
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xBE, (mix(s, 4) % 10) as u16);
            }),
            // Blends 0..5, depth modes 0..2, orientations 0..9.
            "quads4::sparks" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[6][0] = ((mix(s, 1) % 3) | ((mix(s, 2) % 6) << 4)) as u64;
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xBE, (mix(s, 4) % 10) as u16);
            }),
            // Depth modes 0..3, per-quad orientations 0..9.
            "quads4::ring_facing_i" | "quads4::ring_facing_wh" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[6][0] = mix(s, 1) % 4;
                let d = c.reg(4) as u32;
                for k in 0..8 {
                    c.write_u16(d + 0xC0 + 2 * k, (mix(s, 4 + u64::from(k)) % 10) as u16);
                }
            }),
            // blends 0..5, depth modes 0..3, orientations 0..9.
            "quads4::ring_facing_m" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[6][0] = ((mix(s, 1) % 4) | ((mix(s, 2) % 6) << 4)) as u64;
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xC0, (mix(s, 4) % 10) as u16);
            }),
            // The number of quads too (0: 20).
            // depth modes 0..3, blends 0..5, orientations 0..9,
            // counts 0..8.
            // depth 0..2, blends 0..5.
            "quads4::tint4" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[6][0] = ((mix(s, 1) % 3) | ((mix(s, 2) % 6) << 4)) as u64;
            }),
            "quads4::ring_at_m" | "quads4::ring_at_m_uv1" | "quads4::ring_at_mt" | "quads4::ring_facing_flat" | "quads4::ring_facing_flat_z" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[6][0] = ((mix(s, 1) % 4) | ((mix(s, 2) % 6) << 4)) as u64;
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xC0, (mix(s, 4) % 10) as u16);
                c.write_u16(d + 0xB8, (mix(s, 5) % 9) as u16);
            }),
            // depth 0..3 (a2); orientations 0..9 and z turns.
            "quads4::band18" | "quads4::band18_turned" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[6][0] = mix(s, 1) % 4;
                let t = c.reg(5) as u32;
                c.write_u16(t + 0x3A, (mix(s, 2) % 720) as u16);
            }),
            // fewer entries (1..n), angles / orientations.
            "spawn::history" => Arc::new(|c: &mut Cpu, s: u64| {
                let n = c.reg(5) as u32 as i32;
                if n > 0 {
                    c.gpr[5][0] = (mix(s, 1) % n as u64) + 1;
                }
            }),
            "quads4::fan18" | "quads4::fan18_far" | "quads4::fan18_far0" => Arc::new(|c: &mut Cpu, s: u64| {
                let t = c.reg(5) as u32;
                c.write_u16(t + 0x3A, (mix(s, 2) % 720) as u16);
                c.write_u16(t + 0x38, (mix(s, 3) % 200) as u16);
            }),
            // start angles and rim radii.
            "quads4::fan18_t" => Arc::new(|c: &mut Cpu, s: u64| {
                let t = c.reg(5) as u32;
                c.write_u16(t + 0x5A, (mix(s, 2) % 720) as u16);
                c.write_u32(t + 0x48, ((mix(s, 3) % 200) as f32).to_bits());
            }),
            // offsets of both signs (the / 10), cells 0..7, sizes.
            "quads4::sprite_spec" => Arc::new(|c: &mut Cpu, s: u64| {
                let t = c.reg(5) as u32;
                for k in 0..3u32 {
                    c.write_u16(t + 8 + 2 * k, ((mix(s, 1 + u64::from(k)) % 401) as i32 - 200) as u16);
                }
                c.write_u16(t + 0x34, (mix(s, 5) % 8) as u16);
                c.write_u16(t + 0x38, (mix(s, 6) % 60) as u16);
                // The far-Z mode (spec+0xC) on in half the samples.
                let spec = c.reg(6) as u32;
                c.write_u32(spec + 0xC, (mix(s, 7) % 2) as u32);
            }),
            // blends 0..3, depth modes 0..2, orientations, offsets.
            "quads4::sprite_turned" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[6][0] = ((mix(s, 1) % 3) | ((mix(s, 2) % 4) << 4)) as u64;
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xBE, (mix(s, 4) % 10) as u16);
                let t = c.reg(5) as u32;
                c.write_u16(t + 8, ((mix(s, 5) % 201) as i32 - 100) as u16);
            }),
            // flags (blend 0xA / 0xB, UV kind 0, depth 0x13, the
            // texture filter 0x18, the direction 0x19, one pass 0x1A), the
            // direction, the half width, the UV rectangle.
            "plank::draw" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(5) as u32;
                let mut f = c.read_u32(g + 4) & !0x0708_0C01;
                for (k, b) in [0u32, 0xA, 0xB, 0x13, 0x18, 0x19, 0x1A].into_iter().enumerate() {
                    if mix(s, 1 + k as u64) % 3 == 0 {
                        f |= 1 << b;
                    }
                }
                c.write_u32(g + 4, f);
                for (k, o) in [0x40u32, 0x44, 0x48].into_iter().enumerate() {
                    let v = (mix(s, 10 + k as u64) % 200) as f32 / 100.0 - 1.0;
                    c.write_u32(g + o, v.to_bits());
                }
                c.write_u32(g + 0xC, ((mix(s, 13) % 8) as f32).to_bits());
                c.write_u16(g + 0x10, (mix(s, 14) % 0x100) as u16);
                c.write_u16(g + 0x14, (1 + mix(s, 15) % 0x40) as u16);
            }),
            // blends 0..3, depth modes 0..2, orientations, sizes.
            "quads4::sprite_turned_at" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[6][0] = ((mix(s, 1) % 3) | ((mix(s, 2) % 4) << 4)) as u64;
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xBE, (mix(s, 4) % 10) as u16);
                let t = c.reg(5) as u32;
                c.write_u16(t + 0x38, (mix(s, 5) % 40) as u16);
            }),
            // modes -1..3, point counts 0..6, sizes, alphas,
            // orientations 0..9.
            // point counts 0..7, orientations 0..9, alphas.
            "spline::ribbon_tex" => Arc::new(|c: &mut Cpu, s: u64| {
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xBE, (mix(s, 1) % 10) as u16);
                let t = c.reg(5) as u32;
                c.write_u16(t + 6, (mix(s, 2) % 8) as u16);
                c.write_u8(t + 0x2B, (mix(s, 3) % 0x100) as u8);
                c.write_u8(t + 0x2F, (mix(s, 4) % 0x100) as u8);
            }),
            // point counts 0..7, alphas.
            "spline::ribbon_g" => Arc::new(|c: &mut Cpu, s: u64| {
                let t = c.reg(5) as u32;
                c.write_u16(t + 6, (mix(s, 1) % 8) as u16);
                c.write_u8(t + 0x2B, (mix(s, 2) % 0x100) as u8);
            }),
            "spline::tube" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[7][0] = ((mix(s, 1) % 5) as i64 - 1) as u64;
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xBE, (mix(s, 2) % 10) as u16);
                let t = c.reg(5) as u32;
                c.write_u16(t + 6, (mix(s, 3) % 7) as u16);
                c.write_u16(t + 0x38, (mix(s, 4) % 120) as u16);
                c.write_u8(t + 0x2B, (mix(s, 5) % 0x100) as u8);
            }),
            // blends 0..2, orientations 0..9, sizes and turns.
            "billboard::sprite_spin" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[6][0] = (mix(s, 1) % 3) as u64;
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xBE, (mix(s, 2) % 10) as u16);
                let t = c.reg(5) as u32;
                c.write_u32(t + 0x20, (mix(s, 3) % 120) as u32);
                c.write_u32(t + 0x24, (mix(s, 4) % 60) as u32);
                c.write_u32(t + 0x28, ((mix(s, 5) % 721) as i32 - 360) as u32);
            }),
            // counts 0..24, rows, sizes, radii, angles.
            "billboard::ring_t" | "billboard::ring_t20" => Arc::new(|c: &mut Cpu, s: u64| {
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xB8, (mix(s, 1) % 25) as u16);
                let t = c.reg(5) as u32;
                c.write_u16(t + 8, (mix(s, 2) % 60) as u16);
                c.write_u16(t + 0x30, (mix(s, 3) % 200) as u16);
                c.write_u16(t + 0x38, (mix(s, 4) % 40) as u16);
                c.write_u32(t + 0x10, (mix(s, 5) % 0x10000) as u32);
                c.write_u8(t + 0x2B, (mix(s, 6) % 0x100) as u8);
            }),
            // blends 0..5, depth modes 0..3, orientations 0..9,
            // counts 0..11, spans and radii.
            "billboard::arc_quads" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[6][0] = ((mix(s, 1) % 4) | ((mix(s, 2) % 6) << 4)) as u64;
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xC0, (mix(s, 3) % 10) as u16);
                c.write_u16(d + 0xB8, (mix(s, 4) % 12) as u16);
                let t = c.reg(5) as u32;
                c.write_u16(t + 0x5A, (mix(s, 5) % 120) as u16);
                c.write_u16(t + 0x5C, (mix(s, 6) % 90) as u16);
                c.write_u32(t + 0x48, ((mix(s, 7) % 300) as f32).to_bits());
                c.write_u32(t + 0x4C, ((mix(s, 8) % 500) as f32).to_bits());
            }),
            // first updates, ages, lives, alphas, sizes, parameters.
            "tstep::blue_glow" | "tstep::white_flash" | "tstep::ember" | "tstep::spark" => Arc::new(|c: &mut Cpu, s: u64| {
                let t = c.reg(5) as u32;
                c.write_u16(t + 0x36, (1 + mix(s, 1) % 2) as u16);
                c.write_u16(t + 0x32, (mix(s, 2) % 0x90) as u16);
                c.write_u16(t + 6, (mix(s, 3) % 30) as u16);
                c.write_u8(t + 0x2B, (mix(s, 4) % 0x100) as u8);
                c.write_u16(t + 0x38, (mix(s, 5) % 120) as u16 ^ if mix(s, 7) % 2 == 0 { 0 } else { 0x8000 });
                c.write_u16(t + 0x30, (mix(s, 6) % 400) as u16);
                c.write_u16(t + 0xE, (mix(s, 8) % 30) as u16);
            }),
            // counts 0..19, orientations 0..4, sizes, radii, rows.
            "cloud::cloud" => Arc::new(|c: &mut Cpu, s: u64| {
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xB8, (mix(s, 1) % 20) as u16);
                for k in 0..20u32 {
                    c.write_u16(d + 0xC0 + 2 * k, (mix(s, 10 + u64::from(k)) % 5) as u16);
                }
                let t = c.reg(5) as u32;
                c.write_u16(t + 0x38, (mix(s, 2) % 200) as u16);
                c.write_u16(t + 0x30, (mix(s, 3) % 300) as u16);
                c.write_u16(t + 8, (mix(s, 4) % 40) as u16);
            }),
            // orders 0..9, point counts 0..7, alphas.
            "spline::ribbon_tex1" => Arc::new(|c: &mut Cpu, s: u64| {
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xBE, (mix(s, 1) % 10) as u16);
                let t = c.reg(5) as u32;
                c.write_u16(t + 6, (mix(s, 2) % 8) as u16);
                c.write_u8(t + 0x2B, (mix(s, 3) % 0x100) as u8);
            }),
            // point counts 0..7, widths, alphas.
            "spline::ribbon_cross" => Arc::new(|c: &mut Cpu, s: u64| {
                let t = c.reg(5) as u32;
                c.write_u16(t + 6, (mix(s, 1) % 8) as u16);
                c.write_u16(t + 0x38, (mix(s, 2) % 300) as u16);
                c.write_u8(t + 0x2B, (mix(s, 3) % 0x100) as u8);
                c.write_u8(t + 0x28, (mix(s, 4) % 0x100) as u8);
            }),
            // some, all or none of the 150 tasks free.
            "tstep::spawn" => Arc::new(|c: &mut Cpu, s: u64| {
                let d = c.reg(4) as u32;
                let m = mix(s, 1) % 4;
                for k in 0..150u32 {
                    let busy = match m {
                        0 => true,
                        1 => false,
                        _ => mix(s, 2 + u64::from(k)) % 8 != 0,
                    };
                    c.write_u16(d + 0x4F4 + 0x40 * k + 0x36, u16::from(busy));
                }
                c.write_u16(d + 0x3062, (mix(s, 200) % 8) as u16);
            }),
            // first updates, ages, lives, alphas, sizes, frames.
            "spawn::ring_rise" | "spawn::flare_step" => Arc::new(|c: &mut Cpu, s: u64| {
                let t = c.reg(5) as u32;
                c.write_u16(t + 0x36, (1 + mix(s, 1) % 2) as u16);
                c.write_u16(t + 0x32, (mix(s, 2) % 30) as u16);
                c.write_u16(t + 6, (mix(s, 3) % 30) as u16);
                c.write_u8(t + 0x2B, (mix(s, 4) % 0x100) as u8);
                c.write_u16(t + 0x38, (mix(s, 5) % 120) as u16);
                c.write_u16(t + 0x34, (mix(s, 6) % 8) as u16);
            }),
            // first updates, ages and alphas of the disc step.
            "spawn::disc_step" => Arc::new(|c: &mut Cpu, s: u64| {
                let t = c.reg(5) as u32;
                c.write_u16(t + 0x36, (1 + mix(s, 1) % 2) as u16);
                c.write_u16(t + 0x32, (mix(s, 2) % 10) as u16);
                c.write_u8(t + 0x2B, (mix(s, 3) % 0x100) as u8);
            }),
            // angles, widths, orientations 0..9, repeats 0..3.
            // angles, radii, orientations 0..9, repeats 0..3.
            "quads4::band9_tex" => Arc::new(|c: &mut Cpu, s: u64| {
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xBE, (mix(s, 4) % 10) as u16);
                c.write_u16(d + 0xB8, (mix(s, 5) % 4) as u16);
                let t = c.reg(5) as u32;
                c.write_u16(t + 0x3A, (mix(s, 2) % 720) as u16);
                c.write_u16(t + 0x38, (mix(s, 3) % 200) as u16);
            }),
            "quads4::band18_tex" => Arc::new(|c: &mut Cpu, s: u64| {
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xBE, (mix(s, 4) % 10) as u16);
                c.write_u16(d + 0xB8, (mix(s, 5) % 4) as u16);
                let t = c.reg(5) as u32;
                c.write_u16(t + 0x3A, (mix(s, 2) % 720) as u16);
                c.write_u16(t + 6, (mix(s, 3) % 40) as u16);
            }),
            "quads4::quarters_far_at" | "quads4::quarters_far" => Arc::new(|c: &mut Cpu, s: u64| {
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xBE, (mix(s, 4) % 10) as u16);
                let t = c.reg(5) as u32;
                c.write_u16(t + 0x3A, (mix(s, 2) % 720) as u16);
            }),
            // the 0x70-byte bands with their start angle +0x5A
            // (ring_band_fy20 turns only its first start point by it).
            "quads4::ring_band_fy" | "quads4::ring_band_fy20" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[6][0] = ((mix(s, 1) % 4) | ((mix(s, 2) % 6) << 4)) as u64;
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xB0, (mix(s, 3) % 3) as u16);
                c.write_u16(d + 0xBE, (mix(s, 4) % 10) as u16);
                let t = c.reg(5) as u32;
                c.write_u16(t + 0x5A, (mix(s, 5) % 720) as u16);
            }),
            // blend, submit count, cell rows, radii.
            "quads4::ring_band_stq" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[7][0] = ((mix(s, 1) % 6) << 4) as u64;
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xB8, (mix(s, 2) % 3) as u16);
                c.write_u16(d + 0x96, (mix(s, 3) % 0x100) as u16);
                c.write_u16(d + 0x9A, (mix(s, 4) % 0x80) as u16);
                let t = c.reg(5) as u32;
                c.write_u16(t + 0x5A, (mix(s, 5) % 720) as u16);
            }),
            "tubetrail::tube" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[7][0] = (mix(s, 1) % 4) as u64;
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xBE, (mix(s, 4) % 10) as u16);
            }),
            // a3 (mode / blend), orientations, points, sizes,
            // alphas, the first segment.
            "tubetrail::tube_single_blend" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[7][0] = ((mix(s, 1) % 3) | ((mix(s, 6) % 4) << 4)) as u64;
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xBE, (mix(s, 4) % 10) as u16);
                let t = c.reg(5) as u32;
                c.write_u16(t + 6, (2 + mix(s, 2) % 29) as u16);
                c.write_u16(t + 0xE, (mix(s, 7) % 3) as u16);
                c.write_u32(t + 0x48, ((5 + mix(s, 3) % 60) as f32).to_bits());
                c.write_u8(t + 0x63, (mix(s, 5) % 0x100) as u8);
            }),
            // a3 (mode 0..4), points, sizes, colours.
            "tubetrail::tube_gouraud" | "tubetrail::tube_gouraud2" | "tubetrail::tube_gouraud2_44" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[7][0] = (mix(s, 1) % 5) as u64;
                let t = c.reg(5) as u32;
                c.write_u16(t + 6, (2 + mix(s, 2) % 29) as u16);
                c.write_u32(t + 0x48, ((5 + mix(s, 3) % 60) as f32).to_bits());
                c.write_u8(t + 0x63, (mix(s, 5) % 0x100) as u8);
                c.write_u8(t + 0x60, (mix(s, 6) % 0x100) as u8);
            }),
            // Helper E1 (E4: tube_cam too): a3 (mode 0..3), orientations, points, sizes, alphas.
            "tubetrail::tube_trail3" | "tubetrail::tube_cam" | "tubetrail::tube_const" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[7][0] = (mix(s, 1) % 4) as u64;
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xBE, (mix(s, 4) % 10) as u16);
                let t = c.reg(5) as u32;
                c.write_u16(t + 6, (2 + mix(s, 2) % 29) as u16);
                c.write_u32(t + 0x48, ((5 + mix(s, 3) % 60) as f32).to_bits());
                c.write_u8(t + 0x63, (mix(s, 5) % 0x100) as u8);
            }),
            // a3 (unused), orientations, points, sizes, alphas.
            "tubetrail::tube_single" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[7][0] = (mix(s, 1) % 4) as u64;
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xBE, (mix(s, 4) % 10) as u16);
                let t = c.reg(5) as u32;
                c.write_u16(t + 6, (2 + mix(s, 2) % 29) as u16);
                c.write_u32(t + 0x48, ((5 + mix(s, 3) % 60) as f32).to_bits());
                c.write_u8(t + 0x63, (mix(s, 5) % 0x100) as u8);
            }),
            "quads4::ring_at" => Arc::new(|c: &mut Cpu, s: u64| {
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xBE, (mix(s, 4) % 10) as u16);
                c.write_u16(d + 0xB8, (mix(s, 5) % 9) as u16);
            }),
            "billboard::ring_quads" | "billboard::ring_quads_tall" | "billboard::ring_quads_turned" | "billboard::ring_quads_angled" | "billboard::ring_quads_yx" | "billboard::ring_points" | "billboard::quad" | "billboard::quad_standing" | "billboard::quad_b" | "billboard::quad_b_turned2" | "billboard::quad_b_twice" | "billboard::quad_b_turned" | "billboard::arc_strips" | "billboard::arc_strips_flat" | "billboard::ring_facing" | "quads4::ring_quads_z" | "quads4::quad_turned" | "quads4::arc_strips_tex" | "quads4::ring_band" | "quads4::ring_band_f" | "quads4::ring_band_t" | "quads4::ring_band_t20" | "quads4::quad_turned_f" | "quads4::quad_turned_raised" | "quads4::arc_strips_52" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[6][0] = ((mix(s, 1) % 4) | ((mix(s, 2) % 6) << 4)) as u64;
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xB0, (mix(s, 3) % 3) as u16);
                c.write_u16(d + 0xBE, (mix(s, 4) % 10) as u16);
            }),
            // any frame, negative and past the grid included.
            "uv::grid_rect" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[6][0] = (mix(s, 1) % 12) as i64 as u64 - 2;
            }),
            "uv::uv_rect" | "uv::uv_rect4" | "billboard::uv_rect_q" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[6][0] = (mix(s, 1) % 10) as i64 as u64 - 1;
                let r = c.reg(5) as u32;
                for k in 0..4 {
                    let v = (mix(s, 2 + k) % 600) as u16;
                    c.write_u16(r + 2 * k as u32, v.wrapping_sub(40));
                }
            }),
            "spline::spline" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[6][0] = (mix(s, 1) % 8) as u64;
                let t = (mix(s, 2) % 1000) as f32 / 999.0;
                c.fpr[12] = if mix(s, 3) % 5 == 0 { (t * 3.0 - 1.0).to_bits() } else { t.to_bits() };
            }),
            "shake::camera_shake" => Arc::new(|c: &mut Cpu, s: u64| {
                let p = c.reg(4) as u32;
                let modes = [1u8, 2, 4, 8, 3, 0x12, 0x24];
                c.write_u8(p + 4, modes[(mix(s, 1) % modes.len() as u64) as usize]);
                c.write_u8(p, (mix(s, 2) % 40) as u8);
                c.write_u8(p + 1, if mix(s, 3) % 4 == 0 { 2 } else { 0 });
                c.write_u8(p + 2, (mix(s, 4) % 50) as u8 + 1);
                c.write_u8(p + 3, if mix(s, 5) % 2 == 0 { 0xFF } else { (mix(s, 6) % 30) as u8 });
                c.write_u8(p + 5, (mix(s, 7) % 4) as u8);
                c.write_u8(p + 6, (mix(s, 8) % 3) as u8);
            }),
            "fade::fade" | "fade::fade_c" | "fade::fade_black" => Arc::new(|c: &mut Cpu, s: u64| {
                let p = c.reg(4) as u32;
                let flags = [1u8, 2, 4, 0, 9, 0x0A, 0x11, 0x12, 0x21, 0x41, 0x61, 0x81, 0x82, 0x83, 0x91, 0x18];
                c.write_u8(p + 4, flags[(mix(s, 1) % flags.len() as u64) as usize]);
                // 0 included: shape 2 then divides by zero (LO kept: the
                // port frame starts with the interpreter's LO / HI).
                let len = (mix(s, 2) % 64) as u8;
                c.write_u8(p + 2, len);
                c.write_u8(p, if mix(s, 3) % 4 == 0 { 1 } else { (mix(s, 4) % (u64::from(len) + 2)) as u8 });
                c.write_u8(p + 1, if mix(s, 5) % 5 == 0 { 1 } else { 0 });
            }),
            // history lengths 0..11 (shorter only: a longer one
            // writes below the record's table), node slots 0..9 (8, 9 none).
            "spawn::node_history" => Arc::new(|c: &mut Cpu, s: u64| {
                let n = c.reg(5) as i64;
                c.gpr[5][0] = (mix(s, 1) as i64 % (n.max(0) + 2)) as u64;
                c.gpr[6][0] = mix(s, 2) % 10;
            }),
            "spawn::scatter" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                let f = c.read_u32(g + 4) ^ (((mix(s, 1) & 1) as u32) << 0x1D);
                c.write_u32(g + 4, f);
                // A life of 0 included (rand % 0 keeps HI).
                if mix(s, 2) % 2 == 0 {
                    c.gpr[5][0] = mix(s, 3) % 90;
                }
            }),
            "star::flash" | "star::flash_b" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0xFF) as u8);
                c.write_u8(g + 7, (mix(s, 2) % 12) as u8);
                c.write_u8(g, (mix(s, 3) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 4) % 24) as u8 + 1);
                c.write_u8(g + 0xF, (mix(s, 5) & 0xFF) as u8);
            }),
            "streak::ribbon" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0xFF) as u8);
                c.write_u8(g, (mix(s, 2) % 20) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 3) % 30) as u8 + 1);
                c.write_u8(g + 0xD, (mix(s, 4) % 12) as u8);
                c.write_u8(g + 0xC, (mix(s, 5) % 10) as u8);
                c.write_u8(g + 0xB, (mix(s, 6) % 6) as u8);
            }),
            "sprite::bounce" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0xFF) as u8);
                c.write_u8(g, (mix(s, 2) % 30) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 3) % 40) as u8 + 1);
                c.write_u8(g + 5, (mix(s, 4) % 4) as u8);
                c.write_u8(g + 9, (mix(s, 5) % 10) as u8);
                // Layouts 0..9 and blend modes 1..4: past them the original
                // reads stale registers / stack bytes.
                c.write_u8(g + 0xA, (mix(s, 6) % 10) as u8);
                c.write_u8(g + 0xB, (mix(s, 7) % 4) as u8 + 1);
            }),
            "trail::sparks" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(5) as u32;
                // Flag 3 too: without it life % +0x2F (often 0: HI kept).
                let mask = 0x4008_1C0B;
                let f = c.read_u32(g + 4) ^ (mix(s, 1) as u32 & mask);
                c.write_u32(g + 4, f);
            }),
            "beam::beads" | "beam::beads_at 34 3c" | "beam::beads_at 40 44" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                let f = (mix(s, 1) & 0xFF) as u8;
                c.write_u8(g + 4, f);
                c.write_u8(g, (mix(s, 2) % 20) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 3) % 30) as u8 + 1);
                c.write_u8(g + 0xD, (mix(s, 4) % 5) as u8);
                // Without flag 4, +0xA picks one of the four specs.
                if f & 4 == 0 {
                    c.write_u8(g + 0xA, (mix(s, 5) % 4) as u8);
                }
                c.write_u8(g + 0xB, (mix(s, 6) % 8) as u8);
                c.write_u8(g + 0xC, (mix(s, 7) % 4) as u8 + 1);
            }),
            "burst::node_bursts" | "burst::node_bursts_b" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[5][0] = mix(s, 1) % 160;
            }),
            "burst::ring_spawn" | "burst::ring_spawn_b" => Arc::new(|c: &mut Cpu, s: u64| {
                let p = c.reg(4) as u32;
                c.write_u8(p + 4, (mix(s, 1) % 60) as u8 + 1);
                c.write_u8(p + 5, (mix(s, 2) & 0xFF) as u8);
            }),
            // 
            "arc::arcs" | "arc::arcs_b" => Arc::new(|c: &mut Cpu, s: u64| {
                let p = c.reg(4) as u32;
                let f = c.read_u8(p + 4) & !0x0F;
                c.write_u8(p + 4, f | (mix(s, 1) & 0x0F) as u8);
                c.write_u8(p + 0x34, (mix(s, 2) % 3) as u8);
                c.write_u16(p + 0x36, if mix(s, 3) % 2 == 0 { 0 } else { (mix(s, 4) % 128) as u16 });
                if mix(s, 5) % 3 == 0 {
                    let left = c.read_u8(p + 2);
                    c.write_u8(p, (mix(s, 6) % u64::from(left.max(1))) as u8);
                }
                c.write_f32(p + 0x38, (mix(s, 7) % 1000) as f32 / 999.0);
                c.write_f32(p + 0x3C, (mix(s, 8) % 1000) as f32 / 999.0);
                // A full pool (0x101C30 finds none).
                if mix(s, 9) % 6 == 0 {
                    c.write_u8(p + 0x30, 0);
                }
            }),
            "spike::spikes" | "spike::spikes_b" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0xFF) as u8);
                c.write_u8(g, (mix(s, 2) % 20) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 3) % 30) as u8 + 1);
                c.write_u8(g + 0xD, (mix(s, 4) % 12) as u8);
                c.write_u8(g + 8, (mix(s, 5) % 8) as u8 + 1);
                c.write_u8(g + 0xB, (mix(s, 6) % 4) as u8 + 1);
                c.write_u8(g + 0xC, (mix(s, 7) % 8) as u8);
                c.write_u8(g + 0xE, (mix(s, 8) % 40) as u8);
            }),
            // flags, counts, the length mask +3 (0xFF / 7 / 0x1F),
            // rays +8, texture +0xA, cell +0xC, angles +0x4C.. (0..0xFFFF).
            "spike::spikes_c" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0xFF) as u8);
                c.write_u8(g, (mix(s, 2) % 20) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 3) % 30) as u8 + 1);
                c.write_u8(g + 3, [0xFFu8, 7, 0x1F][(mix(s, 4) % 3) as usize]);
                c.write_u8(g + 8, (mix(s, 5) % 8) as u8 + 1);
                c.write_u8(g + 0xA, (mix(s, 6) % 4) as u8);
                c.write_u8(g + 0xC, (mix(s, 7) % 8) as u8);
                for k in 0..3u32 {
                    c.write_u32(g + 0x4C + 4 * k, (mix(s, 8 + u64::from(k)) & 0xFFFF) as u32);
                }
            }),
            "mote::motes" | "mote::motes_c" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                let f = c.read_u8(g + 4) & !0x0C;
                c.write_u8(g + 4, f | (mix(s, 1) & 0x0C) as u8);
                c.write_u8(g + 0x21, (mix(s, 2) % 4) as u8);
                c.write_u8(g + 0x20, (mix(s, 3) % 8) as u8);
                c.write_u8(g + 0x1E, (mix(s, 4) % 4) as u8 + 1);
                let pool = c.read_u32(g + 8);
                for k in 0..u32::from(c.read_u8(g + 0x1D)).min(16) {
                    let i = c.read_u16(g + 0xC + 2 * k) as i16;
                    if i >= 0 {
                        let e = pool.wrapping_add(i as u32 * 0x28);
                        let m = c.read_u8(e + 3) & !0x30;
                        c.write_u8(e + 3, m | (mix(s, 5 + u64::from(k)) & 0x30) as u8);
                    }
                }
            }),
            "bolt::bolts" | "bolt::bolts_b" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0xFF) as u8);
                c.write_u8(g + 0xC, (mix(s, 2) % 2) as u8);
                c.write_u8(g + 5, (mix(s, 3) % 3) as u8);
                c.write_u8(g + 8, (mix(s, 4) % 12) as u8 + 1);
                c.write_u8(g, (mix(s, 5) % 20) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 6) % 24) as u8 + 1);
            }),
            "drop::drops" | "drop::scatter_drops" | "drop::scatter_drops_b" | "drop::drops_c" | "drop::drops_d" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0xFF) as u8);
                c.write_u8(g + 0xA, (mix(s, 2) % 10) as u8);
                c.write_u8(g + 9, (mix(s, 3) % 8) as u8);
                c.write_u8(g + 0xB, (mix(s, 4) % 4) as u8 + 1);
                c.write_u8(g + 5, (mix(s, 5) % 3) as u8);
                if mix(s, 6) % 3 == 0 {
                    c.write_u8(g, (mix(s, 7) % 20) as u8 + 1);
                }
            }),
            // frames by column 1, blends 0xA..0xC, grow 0xF,
            // drift 0x10, far Z 0x13, every slot 0x1F.
            "quadtrail::draw" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(5) as u32;
                let m = 0x8009_9C02u32;
                let f = c.read_u32(g + 8) & !m;
                c.write_u32(g + 8, f | (mix(s, 1) as u32 & m));
            }),
            // far Z 0x13, every segment 0x1A, lengths, jitter.
            "zap::zap" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(5) as u32;
                let f = c.read_u32(g + 4) & !0x408_0000;
                c.write_u32(g + 4, f | (mix(s, 1) as u32 & 0x408_0000));
                c.write_u16(g + 0x28, (mix(s, 2) % 40) as u16);
                c.write_u16(g + 0x2C, (mix(s, 3) % 30) as u16);
                c.write_u16(g + 0x2A, (mix(s, 4) % 60) as u16 + 1);
            }),
            // modes 1..6, integer / float cells (0x38 bit 0, by
            // column 1), blends 0xA / 0xB, far Z 0x13, no clamp 0x18.
            "pointquad::setup" => Arc::new(|c: &mut Cpu, s: u64| {
                let (g, q) = (c.reg(4) as u32, c.reg(5) as u32);
                c.write_u32(q + 8, (mix(s, 1) % 6) as u32 + 1);
                let f = c.read_u32(g + 0x38) & !1;
                c.write_u32(g + 0x38, f | (mix(s, 2) as u32 & 1));
                c.write_u16(g + 4, (mix(s, 3) % 64) as u16 + 1);
                c.write_u16(g + 6, (mix(s, 4) % 64) as u16 + 1);
            }),
            "pointquad::draw" => Arc::new(|c: &mut Cpu, s: u64| {
                let (g, q) = (c.reg(5) as u32, c.reg(6) as u32);
                c.write_u32(q + 8, (mix(s, 1) % 6) as u32 + 1);
                let m = 0x108_0C03u32;
                let f = c.read_u32(g + 0x38) & !m;
                c.write_u32(g + 0x38, f | (mix(s, 2) as u32 & m));
                if mix(s, 3) % 2 == 0 {
                    for k in 0..4 {
                        c.write_u32(g + 4 * k, 0x3D80_0000 + (mix(s, 4 + u64::from(k)) % 0x80_0000) as u32);
                    }
                }
                c.write_u16(g + 0x50, (mix(s, 8) % 16) as u16);
            }),
            // blends 0xA / 0xB, keys 0x12 / 0x13, ageless 0x1E;
            // drawn count, arc, cell width (0: the division by zero), growth.
            "blade::draw" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(5) as u32;
                let m = 0x400C_0C00u32;
                let f = c.read_u32(g + 4) & !m;
                c.write_u32(g + 4, f | (mix(s, 1) as u32 & m));
                let n = u64::from(c.read_u16(g + 0x9A));
                c.write_u16(g + 0x9C, (mix(s, 2) % (n + 40)) as u16);
                c.write_u32(g + 0x94, (mix(s, 3) % 0x2_0000) as u32);
                c.write_u16(g + 0x14, (mix(s, 4) % 64) as u16);
                c.write_u16(g + 0x9E, (mix(s, 5) % 5) as u16);
            }),
            "blade::setup" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                let n = u64::from(c.read_u16(g + 0x9A));
                c.write_u16(g + 0x9A, (mix(s, 1) % (n + 1)) as u16);
                c.write_u32(g + 0x84, 0x3F80_0000 + (mix(s, 2) % 0x200_0000) as u32);
                c.write_u32(g + 0x88, 0xBF80_0000 + (mix(s, 3) % 0x200_0000) as u32);
            }),
            // blends 0xA..0xC, far Z 0x13, the cell (u, v, w, h).
            "sphere::draw" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(5) as u32;
                let m = 0x8_1C00u32;
                let f = c.read_u32(g + 4) & !m;
                c.write_u32(g + 4, f | (mix(s, 1) as u32 & m));
                for (k, o) in [0x10u32, 0x12, 0x14, 0x16].into_iter().enumerate() {
                    c.write_u16(g + o, (mix(s, 2 + k as u64) % 0x200) as u16);
                }
            }),
            // blends 0xA / 0xB, far Z 0x13, filter 0 0x18,
            // integer cell (bit 0: u, v, w, h) or ST floats.
            "sphere::draw_b" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(5) as u32;
                let m = 0x108_0C01u32;
                let f = c.read_u32(g + 4) & !m;
                let f = f | (mix(s, 1) as u32 & m);
                c.write_u32(g + 4, f);
                if f & 1 != 0 {
                    for (k, o) in [0x10u32, 0x12, 0x14, 0x16].into_iter().enumerate() {
                        c.write_u16(g + o, (mix(s, 2 + k as u64) % 0x200) as u16);
                    }
                } else {
                    for (k, o) in [0x10u32, 0x14, 0x18, 0x1C].into_iter().enumerate() {
                        c.write_u32(g + o, ((mix(s, 2 + k as u64) % 1000) as f32 / 999.0).to_bits());
                    }
                }
            }),
            // far Z 0x13, every piece 0x1A, own width 0x1B,
            // piece count (over 16: capped), jitter, drawn count, width,
            // the cell.
            "strand::draw" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(5) as u32;
                let f = c.read_u32(g + 4) & !0xC08_0000;
                c.write_u32(g + 4, f | (mix(s, 1) as u32 & 0xC08_0000));
                c.write_u16(g + 0xC, (mix(s, 2) % 20) as u16 + 1);
                c.write_u16(g + 0xE, (mix(s, 3) % 12) as u16);
                c.write_u16(g + 0x28, (mix(s, 4) % 20) as u16);
                c.write_u32(g + 0x2C, 0x4000_0000 + (mix(s, 5) % 0x80_0000) as u32);
                c.write_u16(g + 0x14, (mix(s, 6) % 64) as u16);
            }),
            // blends 0xA..0xC, far Z 0x13, layer count, scale.
            "zoom::sprites" => Arc::new(|c: &mut Cpu, s: u64| {
                let r = c.reg(4) as u32;
                let f = c.read_u32(r) & !0x8_1C00;
                c.write_u32(r, f | (mix(s, 1) as u32 & 0x8_1C00));
                c.write_u16(r + 0xC, (mix(s, 2) % 5) as u16);
                c.write_u32(r + 8, 0x4000_0000 + (mix(s, 3) % 0x100_0000) as u32);
            }),
            // blends 0xA..0xC, far Z 0x13, layer count.
            "zoom::quarters" => Arc::new(|c: &mut Cpu, s: u64| {
                let r = c.reg(4) as u32;
                let f = c.read_u32(r) & !0x8_1C00;
                c.write_u32(r, f | (mix(s, 1) as u32 & 0x8_1C00));
                c.write_u16(r + 0xC, (mix(s, 2) % 40) as u16);
            }),
            // blends 0xA..0xC, age scale 0xF, drift 0x10, far
            // Z 0x13, every slot 0x1F, sheet by column 1; frames, ticks.
            "streamer::draw" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(5) as u32;
                let f = c.read_u32(g + 8) & !0x8009_9C02;
                c.write_u32(g + 8, f | (mix(s, 1) as u32 & 0x8009_9C02));
                c.write_u16(g + 0x3C, (mix(s, 2) % 6) as u16 + 1);
                c.write_u16(g + 0x3E, (mix(s, 3) % 4) as u16 + 1);
            }),
            // slots, count, life.
            "streamer::push" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u16(g + 0x38, (mix(s, 1) % 20) as u16 + 1);
                c.write_u16(g + 0x3A, (mix(s, 2) % 20) as u16);
                c.write_u16(g + 0x36, (mix(s, 3) % 40) as u16);
            }),
            // fade's perturbation plus the record's colour.
            "fade::fade_rgb" | "fade::fade_rgb_c" => Arc::new(|c: &mut Cpu, s: u64| {
                let p = c.reg(4) as u32;
                let flags = [1u8, 2, 4, 0, 9, 0x0A, 0x11, 0x12, 0x21, 0x41, 0x61, 0x81, 0x82, 0x83, 0x91, 0x18];
                c.write_u8(p + 4, flags[(mix(s, 1) % flags.len() as u64) as usize]);
                let len = (mix(s, 2) % 64) as u8;
                c.write_u8(p + 2, len);
                c.write_u8(p, if mix(s, 3) % 4 == 0 { 1 } else { (mix(s, 4) % (u64::from(len) + 2)) as u8 });
                c.write_u8(p + 1, if mix(s, 5) % 5 == 0 { 1 } else { 0 });
                for (k, o) in [8u32, 9, 0xA].into_iter().enumerate() {
                    c.write_u8(p + o, mix(s, 6 + k as u64) as u8);
                }
            }),
            // piece count (over 32: capped), drawn count, hold
            // 0x1A, width, the cell.
            "glow::line" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(5) as u32;
                let f = c.read_u32(g + 4) & !0x400_0000;
                c.write_u32(g + 4, f | (mix(s, 1) as u32 & 0x400_0000));
                c.write_u16(g + 0x28, (mix(s, 2) % 40) as u16);
                c.write_u16(g + 0x2C, (mix(s, 3) % 40) as u16);
                c.write_u32(g + 0xC, 0x3F80_0000 + (mix(s, 4) % 0x200_0000) as u32);
                c.write_u16(g + 0x14, (mix(s, 5) % 64) as u16);
            }),
            // hold 0x1A, point count, growth.
            "hoop::draw" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(5) as u32;
                let f = c.read_u32(g + 4) & !0x400_0000;
                c.write_u32(g + 4, f | (mix(s, 1) as u32 & 0x400_0000));
                c.write_u16(g + 0x28, (mix(s, 2) % 5) as u16);
                c.write_u16(g + 0x32, (mix(s, 3) % 6) as u16);
            }),
            // integer UVs (bit 0), the alpha the strip fades
            // from, 3..8 points (+0xE 0..5: the outputs hold 28 samples).
            "swath::draw" => Arc::new(|c: &mut Cpu, s: u64| {
                let r = c.reg(4) as u32;
                let f = c.read_u32(r + 4) & !1;
                c.write_u32(r + 4, f | (mix(s, 1) as u32 & 1));
                c.write_u8(r + 0xB, (mix(s, 2) & 0xFF) as u8);
                c.write_u16(r + 0xE, (mix(s, 3) % 6) as u16);
            }),
            // blends 0xA..0xC, the unit's depth 0x12, far Z 0x13;
            // phase step and amplitude.
            "afterimage::strips" => Arc::new(|c: &mut Cpu, s: u64| {
                let r = c.reg(4) as u32;
                let m = 0x000C_1C00u32;
                let f = c.read_u32(r) & !m;
                c.write_u32(r, f | (mix(s, 1) as u32 & m));
                c.write_u32(r + 0xC, (mix(s, 2) & 0x3FFF) as u32);
                c.write_u32(r + 0x14, f32::to_bits((mix(s, 3) % 64) as f32));
            }),
            // the first push (+0xE = 0) or a shift.
            "swath::push" => Arc::new(|c: &mut Cpu, s: u64| {
                let r = c.reg(4) as u32;
                c.write_u16(r + 0xE, (mix(s, 1) % 2) as u16);
            }),
            // blends 0xA..0xC, far Z 0x13, the |x| / 0 axis flags
            // 0x18..0x1D, age (0: nothing drawn), table start.
            "wave::draw" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(5) as u32;
                let m = 0x3F08_1C00u32;
                let f = c.read_u32(g + 4) & !m;
                c.write_u32(g + 4, f | (mix(s, 1) as u32 & m));
                c.write_u16(g + 0x2A, (mix(s, 2) % 40) as u16);
                c.write_u8(g + 0x2C, (mix(s, 3) & 0xFF) as u8);
            }),
            // integer / float cells (0), height over width (5),
            // blends 0xA..0xC, far Z 0x13, no clamp 0x18, turned 0x1F; the
            // turn and the cell size.
            "facing::quad" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(5) as u32;
                let m = 0x8108_1C21u32;
                let f = c.read_u32(g + 0x38) & !m;
                c.write_u32(g + 0x38, f | (mix(s, 1) as u32 & m));
                c.write_u32(g + 0x44, (mix(s, 2) % 1000) as u32);
                c.write_u16(g + 4, (mix(s, 3) % 64) as u16 + 1);
                c.write_u16(g + 6, (mix(s, 4) % 64) as u16 + 1);
                c.write_f32(g + 8, (mix(s, 5) % 64) as f32 + 1.0);
                c.write_f32(g + 0xC, (mix(s, 6) % 64) as f32 + 1.0);
            }),
            // far Z 0x13, twice 0x1C, the wide box 0x1D.
            "rain::draw" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(5) as u32;
                let m = 0x3008_0000u32;
                let f = c.read_u32(g) & !m;
                c.write_u32(g, f | (mix(s, 1) as u32 & m));
            }),
            "pointquad::squares" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(5) as u32;
                let m = 0x8008_0001u32;
                let f = c.read_u32(g + 4) & !m;
                c.write_u32(g + 4, f | (mix(s, 1) as u32 & m));
            }),
            // colour, angle flag, argument; now and then a full
            // pool (the none-free path).
            // colours, angle / index slots, points, arguments;
            // now and then a full pool.
            "spawn::task200" => Arc::new(|c: &mut Cpu, s: u64| {
                let d = c.reg(4) as u32;
                c.write_u32(d + 0xBAB4, mix(s, 1) as u32);
                c.write_u16(d + 0xBB08, (mix(s, 2) % 360) as u16);
                c.write_u16(d + 0xBB10, (mix(s, 3) % 6) as u16);
                c.write_u32(d + 0x2F0, mix(s, 5) as u32);
                c.write_u32(d + 0x2D4, ((mix(s, 6) % 200) as f32).to_bits());
                c.gpr[6][0] = mix(s, 7) & 0xFFFF;
                if mix(s, 4) % 8 == 0 {
                    for k in 0..200 {
                        c.write_u16(d + 0x426 + 0x40 * k, 1);
                    }
                }
            }),
            "spawn::task300" => Arc::new(|c: &mut Cpu, s: u64| {
                let d = c.reg(4) as u32;
                c.write_u32(d + 0xBAB4, mix(s, 1) as u32);
                c.write_u16(d + 0xBB10, (mix(s, 2) % 6) as u16);
                c.write_u32(d + 0x2D0, ((mix(s, 5) % 300) as f32 - 150.0).to_bits());
                c.gpr[6][0] = mix(s, 3) & 0xFFFF;
                if mix(s, 4) % 8 == 0 {
                    for k in 0..300 {
                        c.write_u16(d + 0x3648 + 0x70 * k, 1);
                    }
                }
            }),
            "spawn::task400" => Arc::new(|c: &mut Cpu, s: u64| {
                let d = c.reg(4) as u32;
                c.write_u32(d + 0xB214, mix(s, 1) as u32);
                c.write_u16(d + 0xB2B0, (mix(s, 2) % 3) as u16);
                c.gpr[6][0] = mix(s, 3) & 0xFFFF;
                if mix(s, 4) % 8 == 0 {
                    for k in 0..400 {
                        c.write_u16(d + 0x2F8 + 0x70 * k, 1);
                    }
                }
            }),
            "spawn::task400_b4" => Arc::new(|c: &mut Cpu, s: u64| {
                let d = c.reg(4) as u32;
                c.write_u32(d + 0xB214, mix(s, 1) as u32);
                c.write_u16(d + 0xB2B4, (mix(s, 2) % 3) as u16);
                c.gpr[6][0] = mix(s, 3) & 0xFFFF;
                if mix(s, 4) % 8 == 0 {
                    for k in 0..400 {
                        c.write_u16(d + 0x2F8 + 0x70 * k, 1);
                    }
                }
            }),
            // blends 0xA..0xC, far Z 0x13, angles, rectangle.
            "plate::plates" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(5) as u32;
                let f = c.read_u32(g + 4) & !0x8_1C00;
                c.write_u32(g + 4, f | (mix(s, 1) as u32 & 0x8_1C00));
                for k in 0..3 {
                    c.write_u32(g + 0x30 + 4 * k, (mix(s, 2 + u64::from(k)) & 0xFFFF) as u32);
                }
                for k in 0..4 {
                    c.write_u16(g + 0x20 + 2 * k, (mix(s, 6 + u64::from(k)) % 200) as u16 + 1);
                }
            }),
            // flag 2, mask, node flag, jitter range (0 included).
            "emit::segment_spawns" | "emit::segment_spawns_b" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0xFF) as u8);
                c.write_u8(g + 3, [0u8, 1, 3, 0xFF][(mix(s, 2) % 4) as usize]);
                if mix(s, 3) % 3 == 0 {
                    c.write_u8(g + 5, (mix(s, 4) % 3) as u8);
                }
                c.write_u32(g + 0x14, (mix(s, 5) % 400) as u32);
                c.write_u8(g + 0xC, (mix(s, 6) % 20) as u8);
            }),
            // segment / curve, flags 2 / 4, node offset, scale 0.
            "emit::bead_trails" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 7) as u8);
                if mix(s, 2) % 3 == 0 {
                    c.write_u32(g + 0x18, 0);
                }
                c.write_u8(g + 0x10, (mix(s, 3) % 12) as u8);
                c.write_u8(g + 0x11, (mix(s, 4) % 30) as u8 + 1);
            }),
            // as bead_trails, and a running count (with a pool
            // after the record when it has none: b137's start late).
            "emit::bead_trails_d" | "emit::bead_trails_e" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 7) as u8);
                let k = [0.0f32, 1.5, 0.5][(mix(s, 2) % 3) as usize];
                c.write_u32(g + 0x18, k.to_bits());
                c.write_u8(g + 5, (mix(s, 6) % 2) as u8);
                c.write_u8(g + 0x10, (mix(s, 3) % 12) as u8);
                c.write_u8(g + 0x11, (mix(s, 4) % 30) as u8 + 1);
                if c.read_u32(g + 8) == 0 {
                    c.write_u32(g + 8, g + 0x100);
                }
                c.write_u8(g, (mix(s, 5) % 20) as u8 + 2);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, 30);
            }),
            // flags 1 / 0x40 / 0x80 / corner mode, mask, node,
            // jitter range (0 included), blend, counts.
            "flicker::flickers" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0xFF) as u8);
                c.write_u8(g + 3, [0u8, 1, 3, 7, 0xFF][(mix(s, 2) % 5) as usize]);
                if mix(s, 3) % 3 == 0 {
                    c.write_u8(g + 5, (mix(s, 4) % 3) as u8);
                }
                c.write_u32(g + 0x14, (mix(s, 5) % 400) as u32);
                c.write_u8(g + 0xA, (mix(s, 6) % 6) as u8);
                c.write_u8(g, (mix(s, 7) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 8) % 24) as u8 + 24);
                c.write_u8(g + 0xF, (mix(s, 9) & 0xFF) as u8);
            }),
            // far Z 0x80, clamp modes, layouts 0..9, cells,
            // blends, point counts (0 and > 36 included), counts.
            // segment / curve, flags 2, node offset.
            "emit::bead_trails_b" | "emit::bead_trails_c" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 3) as u8);
                c.write_u8(g + 0x10, (mix(s, 3) % 12) as u8);
                c.write_u8(g + 0x11, (mix(s, 4) % 30) as u8 + 1);
            }),
            "band::bands" | "band::bands_b" | "band::bands_c" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0xFF) as u8);
                c.write_u8(g + 0xC, (mix(s, 2) & 0xFF) as u8);
                c.write_u8(g + 0xA, (mix(s, 3) % 10) as u8);
                c.write_u8(g + 0xB, (mix(s, 4) % 10) as u8);
                c.write_u8(g + 9, (mix(s, 5) % 6) as u8);
                c.write_u8(g + 7, [0u8, 3, 12, 24, 36, 40][(mix(s, 6) % 6) as usize]);
                c.write_u8(g, (mix(s, 7) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 8) % 24) as u8 + 24);
                c.write_u8(g + 0x27, (mix(s, 9) & 0xFF) as u8);
                c.write_u8(g + 0x2B, (mix(s, 10) & 0xFF) as u8);
            }),
            // flags 1 / 0x40 / 0x80, frame kinds, cells, layouts
            // 0..9 (past 9 the original clamps by stale registers), blends,
            // counts, alphas.
            "flatquad::quads" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0xC1) as u8);
                c.write_u8(g + 9, (mix(s, 2) % 10) as u8);
                c.write_u8(g + 0xA, (mix(s, 3) % 10) as u8);
                c.write_u8(g + 0xB, (mix(s, 4) % 6) as u8);
                c.write_u8(g, (mix(s, 5) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 6) % 24) as u8 + 24);
                c.write_u8(g + 0xF, (mix(s, 7) & 0xFF) as u8);
            }),
            // flags 0x40 / 0x80, counts, alphas.
            "flatquad::lines" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0xC0) as u8);
                c.write_u8(g, (mix(s, 2) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 0xB, (mix(s, 3) & 0xFF) as u8);
                c.write_u8(g + 0xF, (mix(s, 4) & 0xFF) as u8);
            }),
            // +0xC flags (0x20 far base, 3 clamp modes, 8 / 0x10
            // fades), the unit slot, key step, layouts, cells, blends, point
            // counts (0 and > 36 included), counts, alphas.
            "band::bands_d" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 0xC, (mix(s, 1) & 0x3F) as u8);
                c.write_u8(g + 0xD, [0u8, 1, 2, 0xFF][(mix(s, 11) % 4) as usize]);
                c.write_u8(g + 0xE, (mix(s, 12) % 40) as u8);
                c.write_u8(g + 0xA, (mix(s, 3) % 10) as u8);
                c.write_u8(g + 0xB, (mix(s, 4) % 10) as u8);
                c.write_u8(g + 9, (mix(s, 5) % 6) as u8);
                c.write_u8(g + 7, [0u8, 3, 12, 24, 36, 40][(mix(s, 6) % 6) as usize]);
                c.write_u8(g, (mix(s, 7) % 40) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 8) % 40) as u8 + 24);
                c.write_u8(g + 0x27, (mix(s, 9) & 0xFF) as u8);
                c.write_u8(g + 0x2B, (mix(s, 10) & 0xFF) as u8);
            }),
            // flags 1 / 0x40 / 0x80, steps (0 and > 9 included),
            // ratio either side of 1, angle (0 included), counts.
            "arch::arches" | "arch::arches_b" | "arch::arches_c" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0xC1) as u8);
                c.write_u8(g + 7, (mix(s, 2) % 12) as u8);
                c.write_u32(g + 0x14, [0x3F00_0000u32, 0x3F80_0000, 0x4000_0000, 0x3E80_0000][(mix(s, 3) % 4) as usize]);
                if mix(s, 4) % 3 == 0 {
                    c.write_u32(g + 0x1C, 0);
                    c.write_u32(g + 0x20, 0);
                } else {
                    c.write_u32(g + 0x20, (mix(s, 5) % 0x800) as u32);
                }
                c.write_u8(g, (mix(s, 6) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 7) % 24) as u8 + 1);
                c.write_u8(g + 0xF, (mix(s, 8) & 0xFF) as u8);
                c.write_u8(g + 0xB, (mix(s, 9) & 0xFF) as u8);
            }),
            // flags 1 / 2 / 4 / 8 / 0x40, table index, layout,
            // cell, blend 1..4 (another reads stale selectors), sprites,
            // counts.
            "spray::sprays" | "spray::sprays_b" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0x4F) as u8);
                c.write_u8(g + 9, (mix(s, 2) & 0xFF) as u8);
                c.write_u8(g + 0xC, (mix(s, 3) % 10) as u8);
                c.write_u8(g + 0xB, (mix(s, 4) % 10) as u8);
                c.write_u8(g + 0xD, (mix(s, 5) % 4) as u8 + 1);
                c.write_u8(g + 8, (mix(s, 6) % 12) as u8);
                c.write_u8(g, (mix(s, 7) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 8) % 24) as u8 + 24);
                c.write_u8(g + 0x2B, (mix(s, 9) & 0xFF) as u8);
            }),
            // spec 0..3 (0: 0x101EE0's UVs), clamp byte, steps
            // (0 and > 9 included), counts.
            "cone::cones" | "cone::cones_b" | "cone::cones_c" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 8, (mix(s, 1) % 4) as u8);
                c.write_u8(g + 9, (mix(s, 2) & 0xFF) as u8);
                c.write_u8(g + 0xA, (mix(s, 3) & 0xFF) as u8);
                c.write_u8(g + 7, (mix(s, 4) % 12) as u8);
                c.write_u8(g, (mix(s, 5) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 6) % 24) as u8 + 24);
                c.write_u8(g + 0x23, (mix(s, 7) & 0xFF) as u8);
                c.write_u8(g + 0x27, (mix(s, 8) & 0xFF) as u8);
            }),
            // modes 0..3, counts.
            "dash::dashes" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0xFF) as u8);
                c.write_u8(g, (mix(s, 2) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 3) % 24) as u8 + 1);
                c.write_u8(g + 0xF, (mix(s, 4) & 0xFF) as u8);
            }),
            // flag 1, start / end swapped ("down"), arms, rings
            // (0 included), the growth limit, colours, counts.
            "swirl::arms" | "swirl::arms_c" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0xFF) as u8);
                if mix(s, 2) % 2 == 0 {
                    let (a, b) = (c.read_u32(g + 0xC), c.read_u32(g + 0x10));
                    c.write_u32(g + 0xC, b);
                    c.write_u32(g + 0x10, a);
                }
                c.write_u8(g + 8, (mix(s, 3) % 6) as u8 + 1);
                c.write_u8(g + 0xA, (mix(s, 4) % 9) as u8);
                c.write_u8(g + 9, (mix(s, 5) % 30) as u8);
                for o in [0x24u32, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x2B] {
                    c.write_u8(g + o, (mix(s, 6 + o as u64) & 0xFF) as u8);
                }
                c.write_u8(g, (mix(s, 7) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 8) % 24) as u8 + 1);
            }),
            // flags 1 / 2, samples, the ray pool size, counts,
            // the length scale (0 included).
            // flags 1 / 2 / 4 / 8 / 0x40 / 0x80, +0x24 2 / 4, the
            // tail cell, counts (0 included) and the history length.
            "comet::comets" | "comet::comets_b" | "comet::comets_c" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0xCF) as u8);
                c.write_u8(g + 0x24, (mix(s, 2) & 6) as u8);
                c.write_u8(g + 0x23, (mix(s, 3) % 8) as u8);
                if mix(s, 4) % 4 == 0 {
                    c.write_u8(g, 1);
                    c.write_u8(g + 1, 0);
                } else {
                    c.write_u8(g, (mix(s, 5) % 20) as u8 + 2);
                }
                c.write_u8(g + 2, (mix(s, 6) % 24) as u8 + 24);
            }),
            "ray::arc_rays" | "ray::arc_rays_c" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 3) as u8);
                c.write_u8(g + 0x17, (mix(s, 2) % 12) as u8);
                c.write_u8(g + 0x15, (mix(s, 3) % 24) as u8 + 8);
                c.write_u8(g + 0x16, (mix(s, 4) % 40) as u8);
                if mix(s, 5) % 3 == 0 {
                    c.write_u32(g + 0x18, 0);
                }
                c.write_u8(g, (mix(s, 6) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
            }),
            // flags 1 / 2 / 0x80, the extensions (0 and negative
            // included), alphas, counts.
            "rod::rods" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                let f = [0x00u8, 0x01, 0x02, 0x03, 0x80, 0x82, 0x81, 0x83][(mix(s, 1) % 8) as usize];
                c.write_u8(g + 4, f);
                let ext = [0.0f32, 0.25, 0.5, -0.5, 1.0];
                c.write_u32(g + 0x10, ext[(mix(s, 2) % 5) as usize].to_bits());
                c.write_u32(g + 0x14, ext[(mix(s, 3) % 5) as usize].to_bits());
                c.write_u8(g + 0x1B, (mix(s, 4) & 0xFF) as u8);
                c.write_u8(g + 0x1F, (mix(s, 5) & 0xFF) as u8);
                c.write_u8(g, (mix(s, 6) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
            }),
            // as rods.
            "rod::rods_b" | "rod::rods_c" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                let f = [0x00u8, 0x01, 0x02, 0x03, 0x80, 0x82, 0x81, 0x83][(mix(s, 1) % 8) as usize];
                c.write_u8(g + 4, f);
                let ext = [0.0f32, 0.25, 0.5, -0.5, 1.0];
                c.write_u32(g + 0x10, ext[(mix(s, 2) % 5) as usize].to_bits());
                c.write_u32(g + 0x14, ext[(mix(s, 3) % 5) as usize].to_bits());
                c.write_u8(g + 0x1B, (mix(s, 4) & 0xFF) as u8);
                c.write_u8(g + 0x1F, (mix(s, 5) & 0xFF) as u8);
                c.write_u8(g, (mix(s, 6) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
            }),
            // flags (planes, front test, rows capped, alpha
            // rules), ring sizes 0..40, rows, scroll rates, +4 bits.
            // flags (0xC plane, 1 / 2 / 0x10 / 0x20), ring points (0 /
            // 36 / above), textures 0..2, flags 0x40 / 0x80, scrolls, counts.
            "segtube::frustums" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 0xA, (mix(s, 1) & 0x3F) as u8);
                c.write_u8(g + 7, (mix(s, 2) % 41) as u8);
                c.write_u8(g + 8, (mix(s, 3) % 3) as u8);
                c.write_u8(g + 4, (mix(s, 4) & 0xC0) as u8);
                let sc = [0.0f32, 0.17, 0.5, 2.25, -0.3];
                c.write_u32(g + 0x14, sc[(mix(s, 5) % 5) as usize].to_bits());
                c.write_u32(g + 0x18, sc[(mix(s, 6) % 5) as usize].to_bits());
                c.write_u8(g, (mix(s, 7) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 8) % 24) as u8 + 24);
                c.write_u8(g + 0x47, (mix(s, 9) & 0xFF) as u8);
            }),
            "segtube::tubes" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 0xB, (mix(s, 1) & 0x7B) as u8);
                c.write_u8(g + 7, (mix(s, 2) % 41) as u8);
                c.write_u8(g + 8, (mix(s, 3) % 3) as u8);
                c.write_u8(g + 4, (mix(s, 4) & 0xC0) as u8);
                let sc = [0.0f32, 0.17, 0.5, 2.25, -0.3];
                c.write_u32(g + 0x18, sc[(mix(s, 5) % 5) as usize].to_bits());
                c.write_u32(g + 0x1C, sc[(mix(s, 6) % 5) as usize].to_bits());
                c.write_u8(g, (mix(s, 7) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
            }),
            // flags 1..0x0F (culling, odd-count alpha, half
            // domes, mirroring), sizes 0..12, counts (all of count & 3),
            // +4 bits, S / T rates.
            "dome::domes" | "dome::domes_550" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 0xA, (mix(s, 1) & 0xF) as u8);
                c.write_u8(g + 7, (mix(s, 2) % 13) as u8);
                c.write_u8(g + 4, (mix(s, 3) & 0xC0) as u8);
                let sc = [0.0f32, 0.25, 1.0, 2.5, -0.5];
                c.write_u32(g + 0xC, sc[(mix(s, 4) % 5) as usize].to_bits());
                c.write_u32(g + 0x10, sc[(mix(s, 5) % 5) as usize].to_bits());
                c.write_u8(g, (mix(s, 6) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
            }),
            // the window (n inside or at its ends), fade, blend.
            // flags (1 / 2 / 4 / 8 / 0x40 / 0x80), segments
            // (0..8), counts, alpha, blend / layout / cell.
            "dash::ribbons" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0xCF) as u8);
                c.write_u8(g + 8, (mix(s, 2) % 9) as u8);
                c.write_u8(g, (mix(s, 3) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 4) % 24) as u8 + 24);
                c.write_u8(g + 0x1F, (mix(s, 5) & 0xFF) as u8);
                c.write_u8(g + 0xA, (mix(s, 6) % 6) as u8);
                c.write_u8(g + 0xB, (mix(s, 7) % 10) as u8);
                c.write_u8(g + 0xC, (mix(s, 8) % 10) as u8);
            }),
            // the window opened round the update (or not), the
            // colour / alpha, the key.
            "window::flat" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                let n = c.reg(6) as u32 & 0xFFFF;
                if mix(s, 1) % 3 != 0 {
                    c.write_u16(g, n.saturating_sub((mix(s, 2) % 8) as u32) as u16);
                    c.write_u16(g + 2, (n + (mix(s, 3) % 8) as u32) as u16);
                }
                for k in 0..4u32 {
                    c.write_u8(g + 0xC + k, (mix(s, 4 + k as u64) & 0xFF) as u8);
                }
                c.write_u32(g + 8, 0x4000_0000 + ((mix(s, 9) & 0xFFF) << 16) as u32);
            }),
            "window::frame_fade" => Arc::new(|c: &mut Cpu, s: u64| {
                let p = c.reg(4) as u32;
                let n = c.reg(6) as u32 & 0xFFFF;
                c.write_u16(p, n.wrapping_sub((mix(s, 1) % 12) as u32) as u16);
                c.write_u16(p + 2, n.wrapping_add((mix(s, 2) % 12) as u32) as u16);
                c.write_u8(p + 5, (mix(s, 3) % 8) as u8);
                c.write_u8(p + 6, (mix(s, 4) % 4) as u8 + 1);
            }),
            "window::frame_fade_b" | "window::frame_fade_c" => Arc::new(|c: &mut Cpu, s: u64| {
                let p = c.reg(4) as u32;
                let n = c.reg(6) as u32 & 0xFFFF;
                c.write_u16(p, n.wrapping_sub((mix(s, 1) % 12) as u32) as u16);
                c.write_u16(p + 2, n.wrapping_add((mix(s, 2) % 12) as u32) as u16);
                c.write_u8(p + 5, (mix(s, 3) % 8) as u8);
                c.write_u8(p + 6, (mix(s, 4) % 4) as u8 + 1);
            }),
            // the first record's counts (its first count
            // included), reach, blend, triangle bits, colours.
            "window::frame_rays" | "window::frame_rays_b" | "window::frame_rays_c" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                let n = (mix(s, 1) % 24) as u8 + 1;
                c.write_u8(g, n);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, if mix(s, 2) % 3 == 0 { n } else { n + (mix(s, 3) % 24) as u8 });
                c.write_u8(g + 8, (mix(s, 4) % 40) as u8);
                c.write_u8(g + 9, (mix(s, 5) % 6) as u8);
                c.write_u8(g + 0xA, (mix(s, 6) & 0xFF) as u8);
                for o in 0x1Cu32..0x24 {
                    c.write_u8(g + o, (mix(s, 7 + u64::from(o)) & 0xFF) as u8);
                }
                // Sometimes far off (outside the window or behind the eye).
                let far = [0.0f32, 0.0, 4000.0, -4000.0][(mix(s, 50) % 4) as usize];
                if far != 0.0 {
                    for k in 0..3u32 {
                        let v = f32::from_bits(c.read_u32(g + 0xC + 4 * k)) + far * (k as f32 - 1.0);
                        c.write_u32(g + 0xC + 4 * k, v.to_bits());
                    }
                }
            }),
            // flags 1 / 0x80, directions, rings, spread, blend,
            // counts.
            "radiate::streaks" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0x81) as u8);
                c.write_u8(g + 0xA, (mix(s, 2) & 0xFF) as u8);
                c.write_u8(g + 0xB, (mix(s, 3) % 6) as u8);
                c.write_u8(g + 8, (mix(s, 4) % 16) as u8);
                c.write_u8(g + 9, (mix(s, 5) % 4) as u8 + 1);
                c.write_u8(g, (mix(s, 6) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
            }),
            "radiate::streaks_b" | "radiate::streaks_c" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0x81) as u8);
                c.write_u8(g + 0xA, (mix(s, 2) & 0xFF) as u8);
                c.write_u8(g + 0xB, (mix(s, 3) % 6) as u8);
                c.write_u8(g + 8, (mix(s, 4) % 16) as u8);
                c.write_u8(g + 9, (mix(s, 5) % 4) as u8 + 1);
                c.write_u8(g, (mix(s, 6) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
            }),
            // counts (the alpha ramp), alpha, the facing
            // threshold (both paths), width and history length.
            "smear::smears" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 0x37, (mix(s, 1) & 0xFF) as u8);
                c.write_u8(g, (mix(s, 2) % 12) as u8 + 1);
                c.write_u8(g + 1, 0);
                let t: [u32; 4] = [0xBF80_0000, 0x3F80_0000, 0, 0x3F00_0000];
                c.write_u32(g + 0x28, t[(mix(s, 3) % 4) as usize]);
                c.write_u32(g + 0x2C, 0x4100_0000 + ((mix(s, 4) % 64) << 17) as u32);
            }),
            // smears' perturbation plus the step cap +0x23.
            "smear::smears_a" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 0x37, (mix(s, 1) & 0xFF) as u8);
                c.write_u8(g, (mix(s, 2) % 12) as u8 + 1);
                c.write_u8(g + 1, 0);
                let t: [u32; 4] = [0xBF80_0000, 0x3F80_0000, 0, 0x3F00_0000];
                c.write_u32(g + 0x28, t[(mix(s, 3) % 4) as usize]);
                c.write_u32(g + 0x2C, 0x4100_0000 + ((mix(s, 4) % 64) << 17) as u32);
                c.write_u8(g + 0x23, (mix(s, 5) % 14) as u8);
            }),
            // integer / ST UVs (bit 0), far Z (bit 0x13), the
            // group turn (bit 0x1E) and the blends.
            "spin::motes" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(5) as u32;
                let blend = [0u32, 1 << 0xA, 1 << 0xB, 1 << 0xC][(mix(s, 1) % 4) as usize];
                let mut f = (c.read_u32(g + 4) & !(1 | (1 << 0x13) | (1 << 0x1E) | 0x1C00)) | blend;
                for (k, b) in [(2u64, 1u32), (3, 1 << 0x13), (4, 1 << 0x1E)] {
                    if mix(s, k) % 2 != 0 {
                        f |= b;
                    }
                }
                c.write_u32(g + 4, f);
            }),
            // blends, the frame order (bit 1), the size growth
            // (bit 0xF, x 0.99), the alpha cap, frames and period.
            "spin::strips" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(5) as u32;
                let blend = [0u32, 1 << 0xA, 1 << 0xB, 1 << 0xC][(mix(s, 1) % 4) as usize];
                let mut f = (c.read_u32(g + 8) & !(0x1C00 | 2 | 0x8000)) | blend;
                if mix(s, 2) & 1 != 0 {
                    f |= 2;
                }
                if mix(s, 3) % 3 == 0 {
                    f |= 0x8000;
                    c.write_u32(g + 0x64, 0x3F7D_70A4);
                }
                c.write_u32(g + 8, f);
                c.write_u8(g + 7, (mix(s, 4) & 0xFF) as u8);
                c.write_u16(g + 0x7C, (mix(s, 5) % 8) as u16 + 1);
                c.write_u16(g + 0x7E, (mix(s, 6) % 5) as u16 + 1);
            }),
            // blends (0xA..0xC), columns / rows (1), static
            // UVs (3), far Z (0x13), no clamp (0x18), growth (0x1B), bob,
            // alpha cap.
            "flatsprite::sprites" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(5) as u32;
                let keep = !(0x1C00u32 | 2 | 8 | (1 << 0x13) | (1 << 0x18) | (1 << 0x1B));
                let modes = [0u32, 0x400, 0x800, 0x1000];
                let mut f = (c.read_u32(g + 4) & keep) | modes[(mix(s, 1) % 4) as usize];
                for (k, b) in [(2u64, 1u32), (3, 3), (4, 0x13), (5, 0x18), (6, 0x1B)] {
                    f |= ((mix(s, k) & 1) as u32) << b;
                }
                c.write_u32(g + 4, f);
                c.write_u16(g + 0x58, (mix(s, 7) % 0x800) as u16);
                c.write_u32(g + 0x54, ((mix(s, 8) % 20) as f32).to_bits());
                c.write_u32(g + 0x50, ((mix(s, 9) % 8) as f32 * 0.25).to_bits());
                c.write_u8(g + 0xB, (mix(s, 10) & 0xFF) as u8);
                // A float rectangle in a quarter of the samples (bit 0 off).
                if mix(s, 11) % 4 == 0 {
                    c.write_u32(g + 4, f & !1);
                    let r = [0.0f32, 0.125, 0.25, 0.5];
                    for (k, v) in [(0x30u32, r[(mix(s, 12) % 4) as usize]), (0x34, r[(mix(s, 13) % 4) as usize]), (0x38, 0.25), (0x3C, 0.125)] {
                        c.write_u32(g + k, v.to_bits());
                    }
                }
            }),
            // the drifting ribbon's fade step, life, alpha, drift
            // and x factor; the draw's alpha modes (0xA..0xC), far Z (0x13),
            // two strips (0x1E), every slot (0x1F), width and growth.
            "driftribbon::push" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 0x3A, (mix(s, 1) % 32) as u8);
                c.write_u16(g + 0x3E, (mix(s, 2) % 0x60) as u16);
                c.write_u8(g + 7, (mix(s, 3) & 0xFF) as u8);
                for k in 0..3u32 {
                    c.write_u32(g + 0x20 + 4 * k, ((mix(s, 4 + u64::from(k)) % 9) as f32 - 4.0).to_bits());
                }
                c.write_u32(g + 0x30, ([0.5f32, 0.9, 1.0, 1.1][(mix(s, 7) % 4) as usize]).to_bits());
            }),
            "driftribbon::draw" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(5) as u32;
                let f = c.read_u32(g + 8) & !(0x1C00 | (1 << 0x13) | (1 << 0x1E) | (1 << 0x1F));
                let modes = [0u32, 0x400, 0x800, 0x1000];
                let f = f | modes[(mix(s, 1) % 4) as usize] | (((mix(s, 2) & 1) as u32) << 0x13) | (((mix(s, 3) & 1) as u32) << 0x1F) | (((mix(s, 5) & 1) as u32) << 0x1E);
                c.write_u32(g + 8, f);
                c.write_u32(g + 0xC, (1.0 + (mix(s, 4) % 40) as f32).to_bits());
                c.write_u32(g + 0x34, ((mix(s, 6) % 8) as f32 * 0.25).to_bits());
            }),
            // the flash's colour and alpha.
            "flash::frame_flash" => Arc::new(|c: &mut Cpu, s: u64| {
                let p = c.reg(4) as u32;
                for k in 0..4u32 {
                    c.write_u8(p + k, (mix(s, 1 + u64::from(k)) & 0xFF) as u8);
                }
            }),
            // the fade step, life and new points' alpha.
            "fxlib::ribbon_push" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 0x22, (mix(s, 1) % 32) as u8);
                c.write_u16(g + 0x26, (mix(s, 2) % 0x60) as u16);
                c.write_u8(g + 7, (mix(s, 3) & 0xFF) as u8);
            }),
            // alpha modes (bits 0xA..0xC), far Z (0x13), every
            // slot (0x1F), widths.
            "fxlib::ribbon_draw" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(5) as u32;
                let f = c.read_u32(g + 8) & !(0x1C00 | (1 << 0x13) | (1 << 0x1F));
                let modes = [0u32, 0x400, 0x800, 0x1000];
                let f = f | modes[(mix(s, 1) % 4) as usize] | (((mix(s, 2) & 1) as u32) << 0x13) | (((mix(s, 3) & 1) as u32) << 0x1F);
                c.write_u32(g + 8, f);
                c.write_u32(g + 0xC, (1.0 + (mix(s, 4) % 40) as f32).to_bits());
            }),
            // any colour and alpha.
            "framegs::frame_quad" => Arc::new(|c: &mut Cpu, s: u64| {
                let p = c.reg(4) as u32;
                for k in 0..4u32 {
                    c.write_u8(p + k, (mix(s, 1 + u64::from(k)) & 0xFF) as u8);
                }
            }),
            // smears' perturbation.
            "smear::smears_c" | "smear::smears_d" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 0x37, (mix(s, 1) & 0xFF) as u8);
                c.write_u8(g, (mix(s, 2) % 12) as u8 + 1);
                c.write_u8(g + 1, 0);
                let t: [u32; 4] = [0xBF80_0000, 0x3F80_0000, 0, 0x3F00_0000];
                c.write_u32(g + 0x28, t[(mix(s, 3) % 4) as usize]);
                c.write_u32(g + 0x2C, 0x4100_0000 + ((mix(s, 4) % 64) << 17) as u32);
            }),
            // flags 0x40 / 0x80, blends 0..5, layouts 0..9,
            // cells, alpha, counts and totals.
            "panel::panels" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                let f = c.read_u8(g + 4);
                c.write_u8(g + 4, (f & !0xC0) | (mix(s, 1) & 0xC0) as u8);
                c.write_u8(g + 0xB, (mix(s, 2) % 6) as u8);
                c.write_u8(g + 9, (mix(s, 3) % 10) as u8);
                c.write_u8(g + 0xA, (mix(s, 4) % 10) as u8);
                c.write_u8(g + 0xF, (mix(s, 5) & 0xFF) as u8);
                c.write_u8(g + 2, (mix(s, 6) % 24) as u8 + 1);
                c.write_u8(g, (mix(s, 7) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
            }),
            // texture / atlas flags, the atlas shift, alpha.
            "sprite::sparks" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                let f = c.read_u8(g + 4);
                c.write_u8(g + 4, (f & !0xC) | (mix(s, 1) & 0xC) as u8);
                c.write_u8(g + 0x21, (mix(s, 2) % 3) as u8);
                c.write_u8(g + 0x25, (mix(s, 3) & 0xFF) as u8);
            }),
            // the late emitter's update window round n.
            "spawn::emitters" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                let n = c.reg(5) as u32 & 0xFFFF;
                c.write_u16(g + 0x34, n.wrapping_sub((mix(s, 1) % 4) as u32) as u16);
                c.write_u16(g + 0x36, n.wrapping_add((mix(s, 2) % 4) as u32) as u16);
            }),
            // sparks per ring, count, delay mask.
            "spawn::fan_ring" => Arc::new(|c: &mut Cpu, s: u64| {
                let p = c.reg(4) as u32;
                let g = c.read_u32(p);
                c.write_u8(g + 0x1D, (mix(s, 1) % 9) as u8 + 1);
                c.write_u8(p + 4, (mix(s, 2) % 40) as u8 + 1);
                c.write_u8(p + 5, (mix(s, 3) & 0xF) as u8);
            }),
            // flag 1, steps, blend, offsets (both signs), counts.
            // phase, hold, masks, chance, range (0 included), step
            // (never 0: the original loops), blend, colour, row offsets, counts.
            "radiate::jitter" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u32(g + 8, (mix(s, 1) & 1) as u32);
                c.write_u8(g + 0xF, (mix(s, 2) % 4) as u8);
                c.write_u8(g + 0x10, (mix(s, 3) & 7) as u8);
                c.write_u8(g + 0x11, (mix(s, 4) & 7) as u8);
                c.write_u8(g + 0xD, (mix(s, 5) & 0xFF) as u8);
                c.write_u8(g + 0xE, (mix(s, 6) % 9) as u8);
                c.write_u8(g + 0x12, (mix(s, 7) % 0x40) as u8 + 1);
                c.write_u8(g + 0xC, (mix(s, 8) % 4) as u8 + 1);
                for k in 0..4u32 {
                    c.write_u8(g + 0x18 + k, (mix(s, 9 + u64::from(k)) & 0xFF) as u8);
                }
                for k in 0..0x200u32 {
                    if mix(s, 20 + u64::from(k)) % 3 == 0 {
                        c.write_u8(g + 0x2C + 2 * k, (mix(s, 13) % 7) as u8);
                    }
                }
                c.write_u8(g, (mix(s, 14) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
            }),
            "radiate::echoes" | "radiate::echoes_b" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0x81) as u8);
                c.write_u8(g + 8, (mix(s, 2) % 12) as u8);
                c.write_u8(g + 9, (mix(s, 3) % 4) as u8 + 1);
                c.write_u8(g + 0xA, (mix(s, 4) % 40) as u8);
                c.write_u8(g + 0xB, (mix(s, 5) % 40) as u8);
                c.write_u8(g + 0x13, (mix(s, 6) & 0xFF) as u8);
                c.write_u8(g, (mix(s, 7) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
            }),
            // flags 1 / 2, rays (0 included), the cursor, counts.
            // ring kinds 0..6, ring counts 1..3, flag 0x80,
            // counts, alphas.
            // ring kinds 0..6, ring counts 1..8, flag 0x80,
            // counts, alphas, the spacing and the aim offset.
            "iris::rings_b" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                for k in 0..8u32 {
                    c.write_u8(g + 8 + k, ((mix(s, 1) + 3 * u64::from(k)) % 7) as u8);
                }
                c.write_u8(g + 0x10, (mix(s, 4) % 8) as u8 + 1);
                c.write_u8(g + 4, (mix(s, 5) & 0x80) as u8);
                c.write_u8(g, (mix(s, 6) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 7) % 24) as u8 + 24);
                c.write_u8(g + 0x11, (mix(s, 8) & 0xFF) as u8);
                c.write_u8(g + 0x12, (mix(s, 9) & 0xFF) as u8);
                c.write_u32(g + 0x34, 0x3F00_0000 + ((mix(s, 16) & 0xFF) << 16) as u32);
                for k in 0..3u32 {
                    let v = f32::from_bits(c.read_u32(g + 0x48 + 4 * k)) + ((mix(s, 10 + u64::from(k)) % 41) as f32 - 20.0);
                    c.write_u32(g + 0x48 + 4 * k, v.to_bits());
                }
            }),
            "iris::rings" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                for k in 0..3u32 {
                    c.write_u8(g + 8 + k, ((mix(s, 1) + 3 * u64::from(k)) % 7) as u8);
                }
                c.write_u8(g + 0x10, (mix(s, 4) % 3) as u8 + 1);
                c.write_u8(g + 4, (mix(s, 5) & 0x80) as u8);
                c.write_u8(g, (mix(s, 6) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 7) % 24) as u8 + 24);
                c.write_u8(g + 0x11, (mix(s, 8) & 0xFF) as u8);
                c.write_u8(g + 0x12, (mix(s, 9) & 0xFF) as u8);
                for k in 0..3u32 {
                    c.write_u32(g + 0x4C + 4 * k, (mix(s, 10 + k as u64) & 0xFFFF) as u32);
                    c.write_u32(g + 0x14 + 4 * k, 0x3F80_0000 + ((mix(s, 13 + k as u64) & 0x7F) << 20) as u32);
                }
                c.write_u32(g + 0x34, 0x3F00_0000 + ((mix(s, 16) & 0xFF) << 16) as u32);
                c.write_u32(g + 0x38, 0x3F00_0000 + ((mix(s, 17) & 0xFF) << 16) as u32);
            }),
            // angles, steps (1..12), the step angle, counts, +9,
            // the alphas.
            "wedge::fans" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 8, (mix(s, 1) % 12) as u8 + 1);
                c.write_u32(g + 0x14, 0x3F80_0000 + ((mix(s, 2) & 0x3) << 22) as u32);
                c.write_u32(g + 0xC, 0x4180_0000 + ((mix(s, 3) & 0xFF) << 15) as u32);
                c.write_u32(g + 0x10, 0x4180_0000 + ((mix(s, 4) & 0xFF) << 15) as u32);
                c.write_u8(g, (mix(s, 5) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 6) % 24) as u8 + 24);
                c.write_u8(g + 9, (mix(s, 7) % 24) as u8);
                c.write_u8(g + 0x27, (mix(s, 8) & 0xFF) as u8);
                c.write_u8(g + 0x2B, (mix(s, 9) & 0xFF) as u8);
            }),
            // the wobble flag / timer / masks, chance, spread,
            // strip width (8..71), blend, counts.
            "haze::screen" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u32(g + 8, (mix(s, 1) & 1) as u32);
                c.write_u8(g + 0xF, (mix(s, 2) % 4) as u8);
                c.write_u8(g + 0x10, (mix(s, 3) & 7) as u8);
                c.write_u8(g + 0x11, (mix(s, 4) & 7) as u8);
                c.write_u8(g + 0xD, (mix(s, 5) & 0xFF) as u8);
                c.write_u8(g + 0xE, (mix(s, 6) % 9) as u8);
                c.write_u8(g + 0x12, (mix(s, 7) % 64) as u8 + 8);
                c.write_u8(g + 0xC, (mix(s, 8) % 6) as u8);
                c.write_u8(g, (mix(s, 9) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 0x1B, (mix(s, 10) & 0xFF) as u8);
            }),
            // flags 0x40 / 0x80, counts, scroll rates, alpha.
            "scrollquad::quads" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0xC0) as u8);
                c.write_u8(g, (mix(s, 2) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 3) % 24) as u8 + 24);
                c.write_u32(g + 0x14, 0x3D00_0000 + ((mix(s, 4) & 0xFF) << 16) as u32);
                c.write_u32(g + 0x18, 0x3D00_0000 + ((mix(s, 5) & 0xFF) << 16) as u32);
                c.write_u8(g + 0x1F, (mix(s, 6) & 0xFF) as u8);
            }),
            // flags (all eight bits), segments +8 / +0xE, steps
            // +0xD (1..), counts, +0xF.
            // flags 1 / 2 / 4, columns / rows 0..9 (9 skips), counts,
            // the start / step angles, layouts / cells.
            "sweep::sweeps" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0x7) as u8);
                c.write_u8(g + 0xB, (mix(s, 2) % 10) as u8);
                c.write_u8(g + 0xC, (mix(s, 3) % 10) as u8);
                c.write_u8(g, (mix(s, 4) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
                let sc = [0.0f32, 15.0, 33.5, 90.0, -20.0, 200.0];
                c.write_u32(g + 0x14, sc[(mix(s, 5) % 6) as usize].to_bits());
                c.write_u32(g + 0x24, sc[(mix(s, 6) % 6) as usize].to_bits());
                c.write_u32(g + 0x18, sc[(mix(s, 7) % 6) as usize].to_bits());
                c.write_u8(g + 9, (mix(s, 8) % 8) as u8);
                c.write_u8(g + 0xF, (mix(s, 9) % 10) as u8);
            }),
            "dash::strokes" | "dash::strokes_b" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0xFF) as u8);
                c.write_u8(g + 8, (mix(s, 2) % 8) as u8);
                c.write_u8(g + 0xE, (mix(s, 3) % 4) as u8);
                c.write_u8(g + 0xD, (mix(s, 4) % 6) as u8 + 1);
                c.write_u8(g, (mix(s, 5) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 6) % 24) as u8 + 24);
                c.write_u8(g + 0xF, (mix(s, 7) & 1) as u8);
                c.write_u8(g + 0x1B, (mix(s, 8) & 0xFF) as u8);
            }),
            // flag 0x40, segments (0..12), counts, alphas.
            "annulus::flat" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0x40) as u8);
                c.write_u8(g + 7, (mix(s, 2) % 13) as u8);
                c.write_u8(g, (mix(s, 3) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 4) % 24) as u8 + 24);
                c.write_u8(g + 0xB, (mix(s, 5) & 0xFF) as u8);
                c.write_u8(g + 0xF, (mix(s, 6) & 0xFF) as u8);
            }),
            // counts / lengths, line counts, cursor, mask, alpha, turn speeds.
            "spokes::lines" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g, (mix(s, 1) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 2) % 24) as u8 + 24);
                c.write_u8(g + 8, (mix(s, 3) % 40) as u8);
                c.write_u8(g + 9, (mix(s, 4) & 0xFF) as u8);
                c.write_u8(g + 0xA, (mix(s, 5) & 0xFF) as u8);
                c.write_u8(g + 0x1B, (mix(s, 6) & 0xFF) as u8);
                for k in 0..3u32 {
                    c.write_u32(g + 0x50 + 4 * k, (mix(s, 7 + u64::from(k)) % 0x800) as u32);
                }
            }),
            "ray::rays" | "ray::rays_b" | "ray::rays_c" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0xFF) as u8);
                c.write_u8(g + 8, (mix(s, 2) % 12) as u8);
                c.write_u8(g + 9, (mix(s, 3) & 0xFF) as u8);
                c.write_u8(g + 0x23, (mix(s, 4) & 0xFF) as u8);
                c.write_u8(g, (mix(s, 5) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 6) % 24) as u8 + 24);
            }),
            // flags 4 / 0x80, layout, cell, blend, counts, alpha.
            "sheet::tiles" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0x84) as u8);
                c.write_u8(g + 9, (mix(s, 2) % 10) as u8);
                c.write_u8(g + 0xA, (mix(s, 3) % 10) as u8);
                c.write_u8(g + 0xB, (mix(s, 4) % 6) as u8);
                c.write_u8(g, (mix(s, 5) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 6) % 24) as u8 + 24);
                c.write_u8(g + 0x17, (mix(s, 7) & 0xFF) as u8);
            }),
            // flags 0x40 / 0x80, layout, cell, blend, counts.
            "sheet::sheets" | "sheet::sheets_c" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0xC0) as u8);
                c.write_u8(g + 9, (mix(s, 2) % 10) as u8);
                c.write_u8(g + 0xA, (mix(s, 3) % 10) as u8);
                c.write_u8(g + 0xB, (mix(s, 4) % 6) as u8);
                c.write_u8(g, (mix(s, 5) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 6) % 24) as u8 + 24);
                c.write_u8(g + 0xF, (mix(s, 7) & 0xFF) as u8);
            }),
            "halo::halo" | "cross::bars" | "cross::bars_b" | "cross::bars_c" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                c.write_u8(g + 4, (mix(s, 1) & 0xFF) as u8);
                c.write_u8(g + 7, (mix(s, 2) % 12) as u8);
                c.write_u8(g, (mix(s, 3) % 24) as u8 + 1);
                c.write_u8(g + 1, 0);
                c.write_u8(g + 2, (mix(s, 4) % 24) as u8 + 1);
                c.write_u8(g + 0xF, (mix(s, 5) & 0xFF) as u8);
            }),
            // depth modes 0..3, orientations, counts 0..8,
            // sizes / radii.
            "billboard::sprite_square" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[6][0] = mix(s, 1) % 3;
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xBE, (mix(s, 2) % 10) as u16);
                let t = c.reg(5) as u32;
                c.write_u16(t + 0x38, (mix(s, 3) % 120) as u16);
                c.write_u32(t + 0x18, ((mix(s, 5) % 721) as i32 - 360) as u32);
            }),
            // first updates, ages round 2, sizes round 0x15, alphas.
            "task40::band_grow" => Arc::new(|c: &mut Cpu, s: u64| {
                let t = c.reg(5) as u32;
                c.write_u16(t + 0x36, (1 + mix(s, 1) % 2) as u16);
                c.write_u16(t + 0x32, (mix(s, 2) % 5) as u16);
                c.write_u16(t + 0x38, ((mix(s, 3) % 50) as i32 - 10) as u16);
                c.write_u8(t + 0x2B, (mix(s, 4) % 0x100) as u8);
                c.write_u16(t + 8, (mix(s, 5) % 7) as u16);
            }),
            // first updates, ages round the life, alphas.
            "task40::ring_strip" => Arc::new(|c: &mut Cpu, s: u64| {
                let t = c.reg(5) as u32;
                c.write_u16(t + 0x36, (1 + mix(s, 1) % 2) as u16);
                c.write_u16(t + 0x32, (mix(s, 2) % 5) as u16);
                c.write_u16(t + 6, (mix(s, 3) % 4) as u16);
                c.write_u8(t + 0x2B, (mix(s, 4) % 0x100) as u8);
            }),
            // counts 0..9, rows, sizes, radii, angles, ends.
            "billboard::ring_strip3" => Arc::new(|c: &mut Cpu, s: u64| {
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xB8, (mix(s, 1) % 10) as u16);
                let t = c.reg(5) as u32;
                c.write_u16(t + 8, (mix(s, 2) % 60) as u16);
                c.write_u16(t + 0xC, ((mix(s, 3) % 41) as i32 - 20) as u16);
                c.write_u16(t + 0x30, (mix(s, 4) % 400) as u16);
                c.write_u16(t + 0x38, (mix(s, 5) % 40) as u16);
                for k in 0..3u32 {
                    c.write_u32(t + 0x10 + 4 * k, (mix(s, 6 + u64::from(k)) % 0x10000) as u32);
                }
            }),
            // both sides, orientations, counts 0..3, radii, angles.
            "quads4::ring_ticks" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[6][0] = mix(s, 1) % 3;
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xBE, (mix(s, 2) % 10) as u16);
                c.write_u16(d + 0xB8, (mix(s, 3) % 4) as u16);
                let t = c.reg(5) as u32;
                c.write_u16(t + 0x38, (mix(s, 4) % 200) as u16);
                c.write_u16(t + 0x3A, (mix(s, 5) % 360) as u16);
                for k in 0..3u32 {
                    c.write_u32(t + 0x10 + 4 * k, ((mix(s, 6 + u64::from(k)) % 721) as i32 - 360) as u32);
                }
            }),
            // blends 0..2, orientations, sizes, drifts.
            "billboard::sprite_drift" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[6][0] = mix(s, 1) % 3;
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xBE, (mix(s, 2) % 10) as u16);
                let t = c.reg(5) as u32;
                c.write_u16(t + 0x38, (mix(s, 3) % 300) as u16);
                for k in 0..3u32 {
                    c.write_u16(t + 8 + 2 * k, ((mix(s, 4 + u64::from(k)) % 401) as i32 - 200) as u16);
                }
            }),
            // blends 0..5, both layouts (D+0xB0 0 / 1), orientations, sizes, lifts, angles.
            "billboard::quad_cross" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[6][0] = (mix(s, 1) % 6) << 4;
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xB0, (mix(s, 2) % 2) as u16);
                c.write_u16(d + 0xBE, (mix(s, 3) % 10) as u16);
                let t = c.reg(5) as u32;
                c.write_u16(t + 8, (mix(s, 4) % 40) as u16);
                c.write_u16(t + 0xA, (mix(s, 5) % 400) as u16);
                c.write_u16(t + 0xC, ((mix(s, 6) % 401) as i32 - 200) as u16);
                c.write_u16(t + 0x5A, (mix(s, 7) % 720) as u16);
                c.write_u16(t + 0x5C, (mix(s, 8) % 720) as u16);
            }),
            "billboard::sprite_spin_40" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[6][0] = mix(s, 1) % 3;
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xBE, (mix(s, 2) % 10) as u16);
                let t = c.reg(5) as u32;
                c.write_u32(t + 0x10, (mix(s, 3) % 120) as u32);
                c.write_u32(t + 0x14, (mix(s, 4) % 60) as u32);
                c.write_u32(t + 0x18, ((mix(s, 5) % 721) as i32 - 360) as u32);
            }),
            // first updates, the timer round 0x1F / 1, lights,
            // weights round 0.1 / 1.0, ages through the life and fade.
            // as task40::camera on card 412's timer / weight.
            "task40::camera_at" => Arc::new(|c: &mut Cpu, s: u64| {
                let d = c.reg(4) as u32;
                let t = c.reg(5) as u32;
                c.write_u16(t + 0x36, (1 + mix(s, 1) % 2) as u16);
                c.write_u32(d + 0x2DF0, [0, 1, 2, 0x1E, 0x1F, 0x20, 0x50][(mix(s, 2) % 7) as usize]);
                c.write_u16(t + 0x30, (mix(s, 3) % 0xA0) as u16 & if mix(s, 9) % 3 == 0 { 1 } else { 0xFFFF });
                c.write_u32(d + 0x2898, [0.0f32, 0.1, 0.15, 0.9, 1.0, 1.2][(mix(s, 4) % 6) as usize].to_bits());
            }),
            "task40::camera" | "task40::camera_z4" | "task40::camera_far" | "task40::mote_target" | "task40::mote_node" => Arc::new(|c: &mut Cpu, s: u64| {
                let d = c.reg(4) as u32;
                let t = c.reg(5) as u32;
                c.write_u16(t + 0x36, (1 + mix(s, 1) % 2) as u16);
                c.write_u32(d + 0x3610, [0, 1, 2, 0x1E, 0x1F, 0x20, 0x50][(mix(s, 2) % 7) as usize]);
                c.write_u16(t + 0x30, (mix(s, 3) % 0xA0) as u16 & if mix(s, 9) % 3 == 0 { 1 } else { 0xFFFF });
                c.write_u32(d + 0x3518, [0.0f32, 0.1, 0.15, 0.9, 1.0, 1.2][(mix(s, 4) % 6) as usize].to_bits());
                c.write_u16(t + 0x32, (mix(s, 5) % 8) as u16);
                c.write_u16(t + 6, (mix(s, 6) % 6) as u16);
                c.write_u8(t + 0x2B, (mix(s, 7) % 0x100) as u8);
                c.write_u16(t + 0xA, (mix(s, 8) % 4) as u16);
            }),
            "quads4::ring_spun_axis" | "quads4::ring_spun_axis_z" | "quads4::ring_spun_blend" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[6][0] = mix(s, 1) % 4;
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xBE, (mix(s, 2) % 10) as u16);
                c.write_u16(d + 0xB8, (mix(s, 3) % 9) as u16);
                c.write_u32(d + 0xA4, [0.0f32, 1.0, 2.0, 3.0][(mix(s, 6) % 4) as usize].to_bits());
                let t = c.reg(5) as u32;
                c.write_u16(t + 0x38, (mix(s, 4) % 120) as u16);
                c.write_u16(t + 0x30, (mix(s, 5) % 200) as u16);
            }),
            "quads4::ring_band_task" => Arc::new(|c: &mut Cpu, s: u64| {
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xB8, (mix(s, 1) % 3) as u16);
                let t = c.reg(5) as u32;
                c.write_u16(t + 0x38, (mix(s, 2) % 300) as u16);
                c.write_u16(t + 6, (mix(s, 3) % 200) as u16);
                c.write_u16(t + 0x3A, (mix(s, 4) % 720) as u16);
                c.write_u32(t + 0x14, (mix(s, 5) % 4096) as u32);
            }),
            "quads4::ring_spun" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[6][0] = mix(s, 1) % 4;
                let d = c.reg(4) as u32;
                c.write_u16(d + 0xBE, (mix(s, 2) % 10) as u16);
                c.write_u16(d + 0xB8, (mix(s, 3) % 9) as u16);
                let t = c.reg(5) as u32;
                c.write_u16(t + 0x38, (mix(s, 4) % 120) as u16);
                c.write_u16(t + 0x30, (mix(s, 5) % 200) as u16);
            }),
            // the variants' colour / slot / full pool, ages
            // through the hold and fade.
            n if n.starts_with("spawn::task400_b4 ") => {
                let p = blib::spawn::pool70_b4(&n[18..]).expect("layout");
                Arc::new(move |c: &mut Cpu, s: u64| {
                    let d = c.reg(4) as u32;
                    c.write_u32(d + p.colour, mix(s, 1) as u32);
                    c.write_u16(d + p.slot, (mix(s, 2) % 3) as u16);
                    c.gpr[6][0] = mix(s, 3) & 0xFFFF;
                    if mix(s, 4) % 8 == 0 {
                        for k in 0..p.n {
                            c.write_u16(d + p.base + 0x58 + 0x70 * k, 1);
                        }
                    }
                })
            }
            // preset corner bits, alphas, history points moved by up to 20.
            "ribbon8::ribbon" => Arc::new(|c: &mut Cpu, s: u64| {
                let g = c.reg(4) as u32;
                let f = c.read_u32(g + 4) & !0x00F0_0000;
                c.write_u32(g + 4, f | (mix(s, 1) as u32 & 0x00F0_0000));
                c.write_u8(g + 0xB, (mix(s, 2) % 0x100) as u8);
                for k in 0..16u32 {
                    let p = g + 0x10 + 0x10 * k;
                    for i in 0..3u32 {
                        let v = c.read_f32(p + 4 * i) + ((mix(s, 3 + u64::from(3 * k + i)) % 41) as f32 - 20.0);
                        c.write_f32(p + 4 * i, v);
                    }
                }
            }),
            "ribbon8::strip_colours" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[6][0] = mix(s, 1) % 8;
                let p = c.reg(5) as u32;
                c.write_u8(p + 3, (mix(s, 2) % 0x100) as u8);
            }),
            n if n.starts_with("spawn::disc_step ") => {
                let k = blib::spawn::disc(&n[17..]).expect("constants");
                Arc::new(move |c: &mut Cpu, s: u64| {
                    let t = c.reg(5) as u32;
                    c.write_u16(t + 0x36, (1 + mix(s, 1) % 2) as u16);
                    c.write_u16(t + 0x32, (mix(s, 2) % (k.hold as u64 + 8)) as u16);
                    c.write_u8(t + 0x2B, (mix(s, 3) % 0x100) as u8);
                })
            }
            _ => return None,
        })
    }

    fn run_one(g: &Game, f: &blib::LibFn, records: usize, updates: usize, samples: usize) -> Report {
        let at = instances(&g.bmodule, f.reference, f.words);
        let t = Instant::now();
        let r = check(&g.exe, &g.bmodule, &g.textures, &at, f.words, adapter(f.name), None, records, updates, samples);
        let at2 = at.clone();
        let rp = match mutator(f.name) {
            Some(m) => {
                let rp = check(&g.exe, &g.bmodule, &g.textures, &at2, f.words, adapter(f.name), Some(m), records, updates, samples);
                for d in &rp.diffs {
                    eprintln!("  perturbed: {d}");
                }
                rp
            }
            None => Report::default(),
        };
        let mut r = r;
        let real = r.samples;
        r.merge(&rp);
        // ROSES_FNPARITY_MISSING=1: the word offsets no sampled call ran.
        if std::env::var_os("ROSES_FNPARITY_MISSING").is_some() {
            let missing: Vec<String> = (0..f.words).filter(|k| !r.covered.contains(k)).map(|k| format!("{:x}", 4 * k)).collect();
            eprintln!("  not executed (byte offsets): {}", missing.join(" "));
        }
        for d in &r.diffs {
            eprintln!("  {d}");
        }
        eprintln!(
            "{} {} ({} words): {} instances (inventory {}), {} calls seen, {} samples ({} real, {} perturbed) from {} cards, {} differing, {} prims, {} bytes written, dead-frame differences in {}, words executed {} / {}; interpreter {:?}, port {:?}, wall {:?}",
            f.hash, f.name, f.words, at.len(), f.users, r.calls, r.samples, real, r.perturbed, r.cards.len(), r.differing, r.prims, r.bytes_written, r.stack_differs, r.covered.len(), f.words, r.oracle, r.port, t.elapsed()
        );
        r
    }

    /// Phase 1's library functions (`roses_fx::fxlib`) by their
    /// reference instances in BMODULE [F inventory].
    const PHASE1: &[blib::LibFn] = &[
        blib::LibFn { hash: "f8e6f44d8b9a", name: "fxlib::colours4", words: 40, users: 124, reference: (0, 0x01E0_0100) },
        blib::LibFn { hash: "4bcc6fc4bd32", name: "fxlib::disc", words: 344, users: 65, reference: (0, 0x01E0_0C20) },
        blib::LibFn { hash: "e853353d6443", name: "fxlib::sprites", words: 1596, users: 65, reference: (0, 0x01E0_1180) },
        blib::LibFn { hash: "32e108615b51", name: "fxlib::ring_draw", words: 316, users: 49, reference: (23, 0x01E0_1A40) },
        blib::LibFn { hash: "08386c4af2a1", name: "fxlib::quads_draw", words: 388, users: 49, reference: (0, 0x01E0_3460) },
        blib::LibFn { hash: "cbde567e8a8d", name: "fxlib::sprites_spawn", words: 100, users: 20, reference: (31, 0x01E0_09C0) },
        blib::LibFn { hash: "acd3c3277df2", name: "fxlib::ring_setup", words: 176, users: 50, reference: (23, 0x01E0_1780) },
        // the ribbon push / draw (b642), and b007's register
        // twin of the draw.
        blib::LibFn { hash: "ba974e6e98ad", name: "fxlib::ribbon_push", words: 84, users: 30, reference: (504, 0x01E0_2980) },
        blib::LibFn { hash: "4be6b21d7b18", name: "fxlib::ribbon_draw", words: 624, users: 28, reference: (504, 0x01E0_2AD0) },
        blib::LibFn { hash: "bfc5b76dc028", name: "fxlib::ribbon_draw", words: 624, users: 1, reference: (649, 0x01E0_2FC0) },
    ];

    /// Every ported library function equals the original at real calls
    /// (`ROSES_FNPARITY_ONLY=name` runs one).
    #[test]
    #[ignore = "requires ROSES_TEST_GAME_DIR (the user's import)"]
    fn private_library_functions_equal_the_interpreter() {
        let g = game();
        let only = std::env::var("ROSES_FNPARITY_ONLY").ok();
        let mut bad = Vec::new();
        for f in blib::FUNCTIONS.iter().chain(PHASE1) {
            if only.as_deref().is_some_and(|o| !f.name.contains(o) && f.hash != o) {
                continue;
            }
            let r = run_one(&g, f, 24, 400, 24);
            assert!(r.samples > 0, "{}: no call sampled", f.name);
            if !r.equal() {
                bad.push(f.name);
            }
        }
        assert!(bad.is_empty(), "differ: {bad:?}");
    }

    /// The harness sees a wrong port: a UV setter with the wrong corner.
    #[test]
    #[ignore = "requires ROSES_TEST_GAME_DIR (the user's import)"]
    fn private_fnparity_sees_a_wrong_port() {
        let g = game();
        let f = &blib::FUNCTIONS[0];
        let at = instances(&g.bmodule, f.reference, f.words);
        let wrong: PortFn = Arc::new(|fx, c| {
            blib::uv::uv_rect(fx, c.a(0), c.a(1), c.ai(2) ^ 1);
            Ret::None
        });
        let r = check(&g.exe, &g.bmodule, &g.textures, &at, f.words, wrong, None, 4, 200, 8);
        eprintln!("wrong port: {} samples, {} differing", r.samples, r.differing);
        assert!(r.samples > 0 && r.differing > 0);
    }

    /// Executable helpers the library ports carry: watched at
    /// their own address while records carrying a host library function
    /// run. (name, address, words, host reference, host words).
    const EXE_HELPERS: &[(&str, u32, u32, (u16, u32), u32)] = &[
        ("exe::jag 0x102A50", 0x0010_2A50, 151, (124, 0x01E0_1B50), 724),
        ("exe::resample 0x102550", 0x0010_2550, 317, (123, 0x01E0_1530), 1184),
        ("exe::aim_matrix 0x103C40", 0x0010_3C40, 48, (263, 0x01E0_2090), 128),
        ("exe::asinf 0x1A4C68", 0x001A_74C0, 6, (263, 0x01E0_2090), 128),
        ("exe::camera_turn 0x2055D0", 0x0020_55D0, 4, (263, 0x01E0_2290), 644),
        ("exe::cell_frame 0x104DC0", 0x0010_4DC0, 296, (213, 0x01E0_0460), 760),
        // the afterimage's depth helpers.
        ("exe::unit_radius 0x1F6A70", 0x001F_6A70, 30, (305, 0x01E0_0120), 412),
        ("exe::depth_key 0x205650", 0x0020_5650, 53, (305, 0x01E0_0120), 412),
        ("exe::depth_w 0x205730", 0x0020_5730, 29, (305, 0x01E0_0120), 412),
        // the VU / random / camera helpers of the single-function
        // records b150 / b475 (cards 168 / 411; " @card" picks the host).
        ("exe::vu_add4 0x1883C0 @168", 0x0018_83C0, 5, (168, 0x01E0_0080), 1312),
        ("exe::vu_add4 0x1883C0 @411", 0x0018_83C0, 5, (411, 0x01E0_0080), 1312),
        ("exe::vu_apply_to 0x1881A0 @168", 0x0018_81A0, 11, (168, 0x01E0_0080), 1312),
        ("exe::vu_apply_to 0x1881A0 @411", 0x0018_81A0, 11, (411, 0x01E0_0080), 1312),
        ("exe::vu_scale4 0x188408 @168", 0x0018_8408, 6, (168, 0x01E0_0080), 1312),
        ("exe::vu_scale4 0x188408 @411", 0x0018_8408, 6, (411, 0x01E0_0080), 1312),
        ("exe::clamp_i 0x101050 @168", 0x0010_1050, 12, (168, 0x01E0_0080), 1312),
        ("exe::clamp_i 0x101050 @411", 0x0010_1050, 12, (411, 0x01E0_0080), 1312),
        ("exe::random_table_vec 0x101080 @168", 0x0010_1080, 234, (168, 0x01E0_0080), 1312),
        ("exe::random_table_vec 0x101080 @411", 0x0010_1080, 234, (411, 0x01E0_0080), 1312),
        ("exe::unit_side 0x100D00 @168", 0x0010_0D00, 19, (168, 0x01E0_0080), 1312),
        ("exe::unit_side 0x100D00 @411", 0x0010_0D00, 19, (411, 0x01E0_0080), 1312),
        ("exe::ramp_above 0x100FF0 @168", 0x0010_0FF0, 10, (168, 0x01E0_0080), 1312),
        ("exe::camera_turn_roll 0x2055E0 @168", 0x0020_55E0, 3, (168, 0x01E0_0080), 1312),
        ("exe::camera_turn_roll 0x2055E0 @411", 0x0020_55E0, 3, (411, 0x01E0_0080), 1312),
        ("exe::tan_falloff_p 0x100D50 @168", 0x0010_0D50, 44, (168, 0x01E0_0080), 1312),
        // powf's general path (0x1A53D8 behind 0x1A7508) and
        // 0x100D50 with p = 3 in b246 (card 461), powf(x, 2) in b471 (367).
        ("exe::tan_falloff_p 0x100D50 @461", 0x0010_0D50, 44, (461, 0x01E0_0080), 1888),
        ("exe::powf 0x1A7508 @461", 0x001A_7508, 5, (461, 0x01E0_0080), 1888),
        ("exe::powf 0x1A7508 @367", 0x001A_7508, 5, (367, 0x01E0_0080), 1888),
        // the helpers b483 (card 589) adds.
        ("exe::uv_from_xy 0x100C60 @589", 0x0010_0C60, 37, (589, 0x01E0_0080), 1312),
        ("exe::project_n 0x100B00 @589", 0x0010_0B00, 39, (589, 0x01E0_0080), 1312),
        ("exe::project_n4 0x100BA0 @589", 0x0010_0BA0, 48, (589, 0x01E0_0080), 1312),
        ("exe::unit_tint_mode 0x1F7600 @589", 0x001F_7600, 36, (589, 0x01E0_0080), 1312),
        ("exe::unit_tint_mode 0x1F7600 @484", 0x001F_7600, 36, (484, 0x01E0_0080), 2592),
        ("exe::unit_tint_mode 0x1F7600 @556", 0x001F_7600, 36, (556, 0x01E0_0080), 2656),
        ("exe::unit_set_position 0x1F7110 @589", 0x001F_7110, 33, (589, 0x01E0_0080), 1312),
        ("exe::unit_point 0x1F65D0 @589", 0x001F_65D0, 69, (589, 0x01E0_0080), 1312),
        ("exe::unit_point 0x1F65D0 @508", 0x001F_65D0, 69, (508, 0x01E0_0080), 2688),
        ("exe::unit_point 0x1F65D0 @556", 0x001F_65D0, 69, (556, 0x01E0_0080), 2656),
        ("exe::unit_point 0x1F65D0 @484", 0x001F_65D0, 69, (484, 0x01E0_0080), 2592),
        ("exe::copy_dest 0x2095A0 @589", 0x0020_95A0, 16, (589, 0x01E0_0080), 1312),
        ("exe::frame_texture_packet 0x207F50 @589", 0x0020_7F50, 26, (589, 0x01E0_0080), 1312),
        // b083 (card 299).
        ("exe::frame_pass 0x2079D0 @299", 0x0020_79D0, 30, (299, 0x01E0_5540), 752),
        ("exe::unit_show 0x1F7940 @299", 0x001F_7940, 33, (299, 0x01E0_5540), 752),
        // 0x20C0B0 with its frame, in the single-function
        // records b290 / b571 (cards 618 / 199).
        ("exe::trs_into_saving 0x20C0B0 @618", 0x0020_C0B0, 117, (618, 0x01E0_0080), 1725),
        ("exe::trs_into_saving 0x20C0B0 @199", 0x0020_C0B0, 117, (199, 0x01E0_0080), 1744),
        ("exe::trs_into_saving 0x20C0B0 @503", 0x0020_C0B0, 117, (503, 0x01E0_0080), 1800),
        // ... and with rotation in sheet::sheets' users.
        ("exe::trs_into_saving 0x20C0B0 @sheets", 0x0020_C0B0, 117, (98, 0x01E0_2980), 348),
        // ... and with scale under 0x1038D0 in b244 (card 327).
        ("exe::trs_into_saving 0x20C0B0 @327", 0x0020_C0B0, 117, (327, 0x01E0_2B30), 300),
        // the screen-winding test of b222 / b765 (cards 563 / 612).
        ("exe::winding 0x101430 @563", 0x0010_1430, 113, (563, 0x01E0_0080), 64),
        ("exe::winding 0x101430 @612", 0x0010_1430, 113, (612, 0x01E0_0080), 64),
        // the outcome boundary's head (0x1F8100 / 0x1F8160 /
        // 0x1DF530 on the mirrored battle) in its nine callers.
        ("exe::outcome 0x1F8100 @16", 0x001F_8100, 23, (16, 0x01E0_0080), 3840),
        ("exe::outcome 0x1F8100 @17", 0x001F_8100, 23, (17, 0x01E0_0080), 3636),
        ("exe::outcome 0x1F8100 @226", 0x001F_8100, 23, (226, 0x01E0_0080), 3712),
        ("exe::outcome 0x1F8100 @339", 0x001F_8100, 23, (339, 0x01E0_0080), 4160),
        ("exe::outcome 0x1F8100 @88", 0x001F_8100, 23, (88, 0x01E0_0080), 1952),
        ("exe::outcome 0x1F8100 @614", 0x001F_8100, 23, (614, 0x01E0_0080), 1888),
        ("exe::outcome 0x1F8100 @664", 0x001F_8100, 23, (664, 0x01E0_0080), 2240),
        ("exe::outcome 0x1F8100 @400", 0x001F_8100, 23, (400, 0x01E0_0080), 3392),
        ("exe::outcome 0x1F8100 @2", 0x001F_8100, 23, (2, 0x01E0_5770), 1732),
        // 0x1F77A0 with a mode (the outcome records' fade and
        // their init's clear).
        ("exe::unit_add_mode 0x1F77A0 @226", 0x001F_77A0, 36, (226, 0x01E0_0080), 3712),
        ("exe::unit_add_mode 0x1F77A0 @16", 0x001F_77A0, 36, (16, 0x01E0_0080), 3840),
        ("exe::unit_add_mode 0x1F77A0 @664", 0x001F_77A0, 36, (664, 0x01E0_0080), 2240),
        ("exe::unit_add_mode 0x1F77A0 @400", 0x001F_77A0, 36, (400, 0x01E0_0080), 3392),
        // the third-point helper of b249 (card 206, unported).
        ("exe::third_point 0x100F60 @206", 0x0010_0F60, 33, (206, 0x01E0_0080), 64),
        // the camera save of card 677's pre-model callback.
        ("exe::camera_save 0x20D660 @677", 0x0020_D660, 20, (677, 0x01E0_2180), 44),
        // the 4 x 4 inverse of the camera chain (0x20DAD0 /
        // 0x20E3D0 in card 677's pre-model callback).
        ("exe::invert4 0x20B550 @677", 0x0020_B550, 531, (677, 0x01E0_2180), 44),
        // the rest of the camera chain (0x20DAD0 / 0x20E3D0).
        ("exe::copy_quads 0x20AAF0 @677", 0x0020_AAF0, 13, (677, 0x01E0_2180), 44),
        ("exe::load_cur 0x20B200 @677", 0x0020_B200, 14, (677, 0x01E0_2180), 44),
        ("exe::depth_range 0x2094D0 @677", 0x0020_94D0, 44, (677, 0x01E0_2180), 44),
        ("exe::view_matrix 0x20CD50 @677", 0x0020_CD50, 191, (677, 0x01E0_2180), 44),
        ("exe::projection 0x20D050 @677", 0x0020_D050, 68, (677, 0x01E0_2180), 44),
        ("exe::camera_build 0x20E160 @677", 0x0020_E160, 83, (677, 0x01E0_2180), 44),
        ("exe::camera_set 0x20DAD0 @677", 0x0020_DAD0, 64, (677, 0x01E0_2180), 44),
        ("exe::camera_view_to 0x20E310 @677", 0x0020_E310, 21, (677, 0x01E0_2180), 44),
        ("exe::vu1_matrix 0x2085B0 @677", 0x0020_85B0, 20, (677, 0x01E0_2180), 44),
        ("exe::vu1_matrix 0x208600 @677", 0x0020_8600, 20, (677, 0x01E0_2180), 44),
        ("exe::vu1_clip 0x208A70 @677", 0x0020_8A70, 46, (677, 0x01E0_2180), 44),
        ("exe::gs_fogcol 0x209A90 @677", 0x0020_9A90, 26, (677, 0x01E0_2180), 44),
        ("exe::vu1_fog 0x208B30 @677", 0x0020_8B30, 42, (677, 0x01E0_2180), 44),
        ("exe::camera_packet 0x20E3D0 @677", 0x0020_E3D0, 49, (677, 0x01E0_2180), 44),
        // 0x1F82B0 with a pre-model and a post callback
        // (packet.rs's unit_target_pre_post on b848's two callbacks).
        ("exe::unit_target_pre_post 0x1F82B0 @677", 0x001F_82B0, 49, (677, 0x01E0_2180), 44),
        // the same chain under card 500's twin callbacks.
        ("exe::camera_save 0x20D660 @500", 0x0020_D660, 20, (500, 0x01E0_4F10), 45),
        ("exe::invert4 0x20B550 @500", 0x0020_B550, 531, (500, 0x01E0_4F10), 45),
        ("exe::load_cur 0x20B200 @500", 0x0020_B200, 14, (500, 0x01E0_4F10), 45),
        ("exe::view_matrix 0x20CD50 @500", 0x0020_CD50, 191, (500, 0x01E0_4F10), 45),
        ("exe::projection 0x20D050 @500", 0x0020_D050, 68, (500, 0x01E0_4F10), 45),
        ("exe::camera_build 0x20E160 @500", 0x0020_E160, 83, (500, 0x01E0_4F10), 45),
        ("exe::camera_set 0x20DAD0 @500", 0x0020_DAD0, 64, (500, 0x01E0_4F10), 45),
        ("exe::camera_packet 0x20E3D0 @500", 0x0020_E3D0, 49, (500, 0x01E0_4F10), 45),
        ("exe::unit_target_pre_post 0x1F82B0 @500", 0x001F_82B0, 49, (500, 0x01E0_4F10), 45),
    ];

    fn exe_adapter(name: &str) -> PortFn {
        let name = name.split(" @").next().unwrap_or(name);
        match name {
            "exe::vu_add4 0x1883C0" => Arc::new(|fx, c| {
                fx.vu_add4(c.a(0), c.a(1), c.a(2));
                Ret::None
            }),
            "exe::vu_apply_to 0x1881A0" => Arc::new(|fx, c| {
                fx.vu_apply_to(c.a(0), c.a(1), c.a(2));
                Ret::None
            }),
            "exe::vu_scale4 0x188408" => Arc::new(|fx, c| {
                fx.vu_scale4(c.a(0), c.a(1), c.f(0));
                Ret::None
            }),
            "exe::clamp_i 0x101050" => Arc::new(|_, c| Ret::V0(Fx::clamp_i(c.ai(0), c.ai(1), c.ai(2)) as u32)),
            "exe::random_table_vec 0x101080" => Arc::new(|fx, c| {
                fx.random_table_vec(c.a(0), c.f(0));
                Ret::None
            }),
            "exe::unit_side 0x100D00" => Arc::new(|fx, c| Ret::V0(fx.unit_side(c.ai(0)) as u32)),
            "exe::ramp_above 0x100FF0" => Arc::new(|_, c| Ret::F0(Fx::ramp_above(c.f(0), c.f(1), c.f(2)))),
            "exe::winding 0x101430" => Arc::new(|fx, c| Ret::V0(roses_fx::b222::winding(fx, c.a(0), c.a(1), c.a(2)) as u32)),
            "exe::third_point 0x100F60" => Arc::new(|fx, c| {
                blib::exe::third_point(fx, c.a(0), c.a(1), c.a(2), c.a(3));
                Ret::None
            }),
            "exe::camera_turn_roll 0x2055E0" => Arc::new(|fx, c| Ret::V0(fx.camera_turn_roll(c.a(0)) as u32)),
            "exe::camera_save 0x20D660" => Arc::new(|fx, c| Ret::V0(fx.camera_save(c.a(0)) as u32)),
            "exe::invert4 0x20B550" => Arc::new(|fx, c| {
                fx.invert4(c.a(0), c.a(1));
                Ret::None
            }),
            "exe::copy_quads 0x20AAF0" => Arc::new(|fx, c| Ret::V0(fx.copy_quads(c.a(0), c.a(1), c.ai(2)))),
            "exe::load_cur 0x20B200" => Arc::new(|fx, c| {
                fx.load_cur(c.a(0));
                Ret::None
            }),
            "exe::depth_range 0x2094D0" => Arc::new(|fx, c| Ret::V0(fx.depth_range(c.a(0), c.a(1)) as u32)),
            "exe::view_matrix 0x20CD50" => Arc::new(|fx, c| {
                fx.view_matrix(c.a(0), c.a(1), c.a(2), c.ai(3));
                Ret::None
            }),
            "exe::projection 0x20D050" => Arc::new(|fx, c| {
                let far = fx.mem.f(c.sp());
                fx.projection(c.a(0), std::array::from_fn(|k| c.f(k)), far);
                Ret::None
            }),
            "exe::camera_build 0x20E160" => Arc::new(|fx, _| Ret::V0(fx.camera_build() as u32)),
            "exe::camera_set 0x20DAD0" => Arc::new(|fx, c| Ret::V0(fx.camera_set(c.a(0)) as u32)),
            "exe::camera_view_to 0x20E310" => Arc::new(|fx, c| {
                fx.camera_view_to(c.a(0));
                Ret::None
            }),
            "exe::vu1_matrix 0x2085B0" => Arc::new(|fx, c| Ret::V0(fx.vu1_matrix(c.a(0), c.a(1), 0x6C04_0044))),
            "exe::vu1_matrix 0x208600" => Arc::new(|fx, c| Ret::V0(fx.vu1_matrix(c.a(0), c.a(1), 0x6C04_0056))),
            "exe::vu1_clip 0x208A70" => Arc::new(|fx, c| Ret::V0(fx.vu1_clip(c.a(0), c.f(0), c.f(1), c.f(2), c.ai(1)))),
            "exe::gs_fogcol 0x209A90" => Arc::new(|fx, c| Ret::V0(fx.gs_fogcol(c.a(0), c.a(1)))),
            "exe::vu1_fog 0x208B30" => Arc::new(|fx, c| Ret::V0(fx.vu1_fog(c.a(0), std::array::from_fn(|k| c.f(k))))),
            "exe::camera_packet 0x20E3D0" => Arc::new(|fx, c| Ret::V0(fx.camera_packet(c.a(0)))),
            "exe::unit_target_pre_post 0x1F82B0" => Arc::new(|fx, c| {
                let (pre, post) = (c.a(4), c.a(5));
                // a1..a3 handed on: 0x1EF8F0's object index (0, 0) to the
                // first callback, then what it leaves to the second.
                let (side, slot) = (c.ai(0) >> 8, c.ai(0) & 0xFF);
                let i = if (0..3).contains(&side) && slot < 0xB { fx.object_index(side, slot) as u32 } else { 0 };
                let regs = std::rc::Rc::new(std::cell::Cell::new([i, 0, 0]));
                let (r1, r2) = (regs.clone(), regs.clone());
                let mut pcb = move |fx: &mut Fx, p: u32| {
                    let (v, r) = roses_fx::b848::callback(fx, pre, p, r1.get());
                    r1.set(r);
                    v
                };
                let mut qcb = move |fx: &mut Fx, p: u32| {
                    let (v, r) = roses_fx::b848::callback(fx, post, p, r2.get());
                    r2.set(r);
                    v
                };
                let r = if pre != 0 {
                    let q: Option<(u32, &mut dyn FnMut(&mut Fx, u32) -> u32)> = if post != 0 { Some((post, &mut qcb)) } else { None };
                    fx.unit_target_pre_post(c.ai(0), c.a(1), c.a(2), c.a(3), (pre, &mut pcb), q)
                } else {
                    fx.unit_target_post(c.ai(0), c.a(1), c.a(2), c.a(3), post, &mut qcb)
                };
                fx.out.boundary.retain(|b| b.0 != 0x001F_82B0);
                Ret::V0(r.0 as u32)
            }),
            "exe::tan_falloff_p 0x100D50" => Arc::new(|fx, c| Ret::F0(fx.tan_falloff_p(c.ai(0), c.ai(1), c.ai(2)))),
            "exe::powf 0x1A7508" => Arc::new(|fx, c| Ret::F0(fx.powf(c.f(0), c.f(1)))),
            "exe::uv_from_xy 0x100C60" => Arc::new(|fx, c| {
                fx.uv_from_xy(c.a(0), c.a(1), c.ai(2), c.ai(3));
                Ret::None
            }),
            "exe::project_n 0x100B00" => Arc::new(|fx, c| Ret::V0(fx.project_n(&[c.a(0), c.a(1), c.a(2)], c.a(3), c.a(4), c.a(5), c.gpr[31] as u32) as u32)),
            "exe::project_n4 0x100BA0" => Arc::new(|fx, c| Ret::V0(fx.project_n(&[c.a(0), c.a(1), c.a(2), c.a(3)], c.a(4), c.a(5), c.a(6), c.gpr[31] as u32) as u32)),
            "exe::unit_tint_mode 0x1F7600" => Arc::new(|fx, c| Ret::V0(fx.unit_tint_mode(c.ai(0), c.ai(1), c.a(2), [c.gpr[16], c.gpr[17]], c.gpr[31] as u32) as u32)),
            "exe::unit_set_position 0x1F7110" => Arc::new(|fx, c| Ret::V0(fx.unit_set_position(c.ai(0), c.a(1), c.gpr[16], c.gpr[31] as u32) as u32)),
            "exe::unit_add_mode 0x1F77A0" => Arc::new(|fx, c| Ret::V0(fx.unit_add_mode(c.ai(0), c.ai(1), c.a(2), [c.gpr[16], c.gpr[17]], c.gpr[31] as u32) as u32)),
            "exe::unit_point 0x1F65D0" => Arc::new(|fx, c| Ret::V0(fx.unit_point(c.ai(0), c.ai(1), c.a(2)) as u32)),
            // The watched call's own boundary report is the record's, not
            // its (the probe starts at its entry).
            "exe::copy_dest 0x2095A0" => Arc::new(|fx, c| {
                let v = fx.copy_dest([c.a(0), c.a(1), c.a(2), c.a(3)]);
                fx.out.boundary.retain(|b| b.0 != 0x0020_95A0);
                Ret::V0(v as u32)
            }),
            "exe::frame_texture_packet 0x207F50" => Arc::new(|fx, c| {
                let v = fx.frame_texture_packet(c.ai(0), c.ai(1), c.a(2), c.a(3))[0];
                fx.out.boundary.retain(|b| b.0 != 0x0020_7F50);
                Ret::V0(v)
            }),
            // the tint pass (its own report dropped as above;
            // v0 is 0x209480's, which the records discard) and the unit flag.
            "exe::frame_pass 0x2079D0" => Arc::new(|fx, c| {
                fx.frame_pass(c.a(0), c.ai(1), c.ai(2), c.gpr[7], c.gpr[8] as u32, c.gpr[31] as u32);
                fx.out.boundary.retain(|b| b.0 != 0x0020_79D0);
                Ret::None
            }),
            "exe::unit_show 0x1F7940" => Arc::new(|fx, c| Ret::V0(fx.unit_show(c.ai(0), c.ai(1), c.gpr[31] as u32) as u32)),
            "exe::trs_into_saving 0x20C0B0" => Arc::new(|fx, c| {
                let f = std::array::from_fn(|k| f32::from_bits(c.fpr[20 + k]));
                fx.trs_into_saving(c.a(0), c.a(1), c.a(2), c.a(3), f, [c.gpr[16], c.gpr[17], c.gpr[18], c.gpr[19]], c.gpr[31] as u32);
                Ret::None
            }),
            "exe::jag 0x102A50" => Arc::new(|fx, c| {
                blib::bolt::jag(fx, c.a(0));
                Ret::None
            }),
            "exe::resample 0x102550" => Arc::new(|fx, c| Ret::V0(blib::arc::resample(fx, c.a(0)) as u32)),
            "exe::aim_matrix 0x103C40" => Arc::new(|fx, c| {
                blib::quadtrail::aim_matrix(fx, c.a(0), c.a(1));
                Ret::V0(0)
            }),
            "exe::asinf 0x1A4C68" => Arc::new(|fx, c| Ret::F0(blib::quadtrail::asinf(fx, c.f(0)))),
            "exe::camera_turn 0x2055D0" => Arc::new(|fx, c| Ret::V0(blib::quadtrail::camera_turn(fx, c.a(0)) as u32)),
            "exe::unit_radius 0x1F6A70" => Arc::new(|fx, c| Ret::F0(blib::afterimage::unit_radius(fx, c.ai(0)))),
            "exe::depth_key 0x205650" => Arc::new(|fx, c| Ret::V0(blib::afterimage::depth_key(fx, c.a(0), c.f(0)))),
            "exe::depth_w 0x205730" => Arc::new(|fx, c| Ret::F0(blib::afterimage::depth_w(fx, c.a(0), c.f(0)))),
            // the battle the image holds (None: the zero image),
            // the call's own report dropped as above.
            "exe::outcome 0x1F8100" => Arc::new(|fx, c| {
                fx.battle = roses_fx::world::battle_in(&mut fx.mem);
                let v = fx.outcome([c.a(0), c.a(1), c.a(2), c.a(3)]);
                fx.out.boundary.retain(|b| b.0 != 0x001F_8100);
                Ret::V0(v as u32)
            }),
            "exe::cell_frame 0x104DC0" => Arc::new(|fx, c| {
                blib::pointquad::cell_frame(fx, c.a(0), c.a(1));
                Ret::None
            }),
            _ => panic!("no adapter for {name}"),
        }
    }

    /// Perturbed arguments for the executable helpers: random
    /// floats in the inputs, clamp values both sides of the bounds, lengths
    /// and ramps either way, the other unit's handle, the camera's roll.
    fn exe_mutator(name: &str) -> Option<Mutator> {
        let name = name.split(" @").next().unwrap_or(name);
        let fl = |s: u64, k: u64| ((mix(s, k) % 4001) as f32 - 2000.0) / 10.0;
        // the projections' vertices moved (a little, a lot).
        let proj = |n: usize| -> Mutator {
            Arc::new(move |c: &mut Cpu, s: u64| {
                let scale = [1.0f32, 10.0, 200.0][(mix(s, 7) % 3) as usize];
                for k in 0..n {
                    let v = (c.reg(4 + k) as u32) & !15;
                    for j in 0..3u32 {
                        let x = f32::from_bits(c.read_u32(v + 4 * j));
                        c.write_u32(v + 4 * j, (x + scale * fl(s, 3 * k as u64 + u64::from(j)) / 200.0).to_bits());
                    }
                }
            })
        };
        Some(match name {
            "exe::vu_add4 0x1883C0" | "exe::vu_apply_to 0x1881A0" => Arc::new(move |c: &mut Cpu, s: u64| {
                let a = (c.reg(5) as u32) & !15;
                let b = (c.reg(6) as u32) & !15;
                for k in 0..4u32 {
                    c.write_u32(b + 4 * k, fl(s, u64::from(k)).to_bits());
                }
                if mix(s, 9) % 2 == 0 {
                    for k in 0..4u32 {
                        c.write_u32(a + 4 * k, fl(s, 10 + u64::from(k)).to_bits());
                    }
                }
            }),
            "exe::vu_scale4 0x188408" | "exe::random_table_vec 0x101080" => Arc::new(move |c: &mut Cpu, s: u64| {
                c.fpr[12] = fl(s, 1).to_bits();
            }),
            // the three screen points anywhere near the window,
            // sometimes two equal (a degenerate triangle).
            // the three points moved (a little, a lot).
            "exe::third_point 0x100F60" => Arc::new(move |c: &mut Cpu, s: u64| {
                let scale = [1.0f32, 10.0, 200.0][(mix(s, 7) % 3) as usize];
                for k in 0..3usize {
                    let v = (c.reg(5 + k) as u32) & !15;
                    for j in 0..4u32 {
                        let x = f32::from_bits(c.read_u32(v + 4 * j));
                        c.write_u32(v + 4 * j, (x + scale * fl(s, 4 * k as u64 + u64::from(j)) / 200.0).to_bits());
                    }
                }
            }),
            "exe::winding 0x101430" => Arc::new(|c: &mut Cpu, s: u64| {
                for k in 0..3u64 {
                    let p = c.reg(4 + k as usize) as u32;
                    c.write_u16(p, (0x6000 + mix(s, 1 + 2 * k) % 0x4000) as u16);
                    c.write_u16(p + 2, (0x6000 + mix(s, 2 + 2 * k) % 0x4000) as u16);
                }
                if mix(s, 9) % 4 == 0 {
                    let (a, b) = (c.reg(4) as u32, c.reg(5) as u32);
                    let x = c.read_u32(a);
                    c.write_u32(b, x);
                }
            }),
            "exe::clamp_i 0x101050" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[4][0] = ((mix(s, 1) % 400) as i64 - 150) as u64;
                c.gpr[5][0] = ((mix(s, 2) % 100) as i64 - 50) as u64;
                c.gpr[6][0] = (mix(s, 3) % 200) as u64;
            }),
            "exe::unit_side 0x100D00" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[4][0] = [0x001u64, 0x101, 0x205, 0x00F][(mix(s, 1) % 4) as usize];
            }),
            "exe::ramp_above 0x100FF0" => Arc::new(move |c: &mut Cpu, s: u64| {
                c.fpr[12] = fl(s, 1).to_bits();
                c.fpr[13] = fl(s, 2).to_bits();
                c.fpr[14] = fl(s, 3).to_bits();
            }),
            "exe::tan_falloff_p 0x100D50" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[5][0] = 1 + mix(s, 2) % 100;
                c.gpr[4][0] = mix(s, 1) % (c.gpr[5][0] + 1);
                c.gpr[6][0] = 1 + mix(s, 3) % 4;
            }),
            // x anywhere (both signs, near 1, zero, large), y
            // small / large integers and fractions of either sign.
            "exe::powf 0x1A7508" => Arc::new(|c: &mut Cpu, s: u64| {
                let x = match mix(s, 1) % 6 {
                    0 => ((mix(s, 2) % 20001) as f32 - 10000.0) / 1000.0,
                    1 => 1.0 + ((mix(s, 2) % 2001) as f32 - 1000.0) / 1.0e6,
                    2 => [0.0f32, -0.0, 1.0, -1.0, 2.0, -2.0, 0.5][(mix(s, 2) % 7) as usize],
                    3 => ((mix(s, 2) % 2001) as f32 - 1000.0) * 1000.0,
                    4 => ((mix(s, 2) % 1000) as f32 + 1.0) / 1.0e5,
                    _ => -(((mix(s, 2) % 1000) as f32 + 1.0) / 100.0),
                };
                let y = match mix(s, 3) % 5 {
                    0 => ((mix(s, 4) % 13) as f32) - 6.0,
                    1 => ((mix(s, 4) % 4001) as f32 - 2000.0) / 100.0,
                    2 => [0.5f32, -0.5, 1.0, -1.0, 2.0, 3.0, 0.0, 1.0e9, -1.0e9, 200.0, -200.0][(mix(s, 4) % 11) as usize],
                    3 => ((mix(s, 4) % 64) as f32) + 0.5,
                    _ => ((mix(s, 4) % 201) as f32 - 100.0) / 7.0,
                };
                c.fpr[12] = x.to_bits();
                c.fpr[13] = y.to_bits();
            }),
            // b483's helpers.
            "exe::uv_from_xy 0x100C60" => Arc::new(|c: &mut Cpu, s: u64| {
                let xyz = c.reg(5) as u32;
                c.write_u16(xyz, (0x6800 + mix(s, 1) % 0x2800) as u16);
                c.write_u16(xyz + 2, (0x6800 + mix(s, 2) % 0x2800) as u16);
                c.gpr[6][0] = ((mix(s, 3) % 600) as i64 - 300) as u64;
                c.gpr[7][0] = ((mix(s, 4) % 600) as i64 - 300) as u64;
            }),
            "exe::project_n 0x100B00" => proj(3),
            "exe::project_n4 0x100BA0" => proj(4),
            "exe::unit_tint_mode 0x1F7600" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[4][0] = [0x001u64, 0x101, 0x205, 0x00F, 0xFFFF_FFFF_FFFF_FF01][(mix(s, 1) % 5) as usize];
                c.gpr[5][0] = ((mix(s, 2) % 6) as i64 - 1) as u64;
                if mix(s, 3) % 3 == 0 {
                    c.gpr[6][0] = 0;
                }
            }),
            "exe::unit_add_mode 0x1F77A0" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[4][0] = [0x001u64, 0x101, 0x205, 0x00F, 0xFFFF_FFFF_FFFF_FF01, 0x003, 0x104][(mix(s, 1) % 7) as usize];
                c.gpr[5][0] = ((mix(s, 2) % 7) as i64 - 2) as u64;
                if mix(s, 3) % 3 == 0 {
                    c.gpr[6][0] = 0;
                }
            }),
            "exe::unit_set_position 0x1F7110" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[4][0] = [0x001u64, 0x101, 0x205, 0x00F, 0xFFFF_FFFF_FFFF_FF01][(mix(s, 1) % 5) as usize];
            }),
            "exe::unit_point 0x1F65D0" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[4][0] = [1u64, 2, 3][(mix(s, 1) % 3) as usize];
                c.gpr[5][0] = [0x001u64, 0x101, 0x205, 0x00F][(mix(s, 2) % 4) as usize];
            }),
            "exe::copy_dest 0x2095A0" => Arc::new(|c: &mut Cpu, s: u64| {
                let b = c.read_u8(0x002E_93C0);
                c.write_u8(0x002E_93C0, (b & !1) | (mix(s, 1) % 2) as u8);
                c.write_u32(0x002E_93B0, (mix(s, 2) % 0x100) as u32);
            }),
            "exe::frame_texture_packet 0x207F50" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[5][0] = mix(s, 1) % 8;
                c.gpr[4][0] = [0x2000u64, 0x2000, 0x2800, 0x1800][(mix(s, 2) % 4) as usize];
            }),
            // the key, filter, ALPHA preset and colour bytes.
            "exe::frame_pass 0x2079D0" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[5][0] = [0u64, 1, 0x80, 0xFFF, 0x1_0000][(mix(s, 1) % 5) as usize];
                c.gpr[6][0] = mix(s, 2) % 8;
                let a = [0x44u64, 0x48, 0x42, 0x80_0000_0048, 0x20_0000_0044][(mix(s, 3) % 5) as usize];
                c.gpr[7][0] = a;
                let p = c.reg(8) as u32;
                if p != 0 {
                    for k in 0..4u32 {
                        c.write_u8(p + k, (mix(s, 4 + u64::from(k)) % 0x100) as u8);
                    }
                }
            }),
            "exe::unit_show 0x1F7940" => Arc::new(|c: &mut Cpu, s: u64| {
                c.gpr[4][0] = [0x001u64, 0x101, 0x205, 0x00F, 0xFFFF_FFFF_FFFF_FF01][(mix(s, 1) % 5) as usize];
                c.gpr[5][0] = mix(s, 2) % 4;
            }),
            // the translation moved, angles anywhere (only
            // where the call passes them).
            "exe::trs_into_saving 0x20C0B0" => Arc::new(move |c: &mut Cpu, s: u64| {
                let (r, t) = (c.reg(6) as u32, c.reg(7) as u32);
                for j in 0..3u32 {
                    if t != 0 {
                        let x = f32::from_bits(c.read_u32(t + 4 * j));
                        c.write_u32(t + 4 * j, (x + fl(s, u64::from(j))).to_bits());
                    }
                    if r != 0 {
                        c.write_u32(r + 4 * j, (mix(s, 5 + u64::from(j)) % 0x2_0000) as u32);
                    }
                }
            }),
            // a seeded battle mirrored for both sides
            // (`outcome_battle`).
            "exe::outcome 0x1F8100" => Arc::new(|c: &mut Cpu, s: u64| {
                let own = c.read_u32(roses_fx::exe::SELF_HANDLE) as u16;
                let target = c.read_u32(roses_fx::exe::TARGET_HANDLE) as u16;
                roses_fx::world::mirror_battle(c, &outcome_battle(s, own, target));
            }),
            // the input matrix anywhere (small, large, mixed
            // scales), singular 1 in 6 (a row repeated or zeroed).
            "exe::invert4 0x20B550" => Arc::new(|c: &mut Cpu, s: u64| {
                let m = c.reg(5) as u32;
                let scale = [1.0f32, 0.01, 50.0, 1e-3, 1e4][(mix(s, 40) % 5) as usize];
                for k in 0..16u32 {
                    if mix(s, 41 + u64::from(k)) % 4 != 0 {
                        let x = ((mix(s, u64::from(k)) % 4001) as f32 - 2000.0) / 1000.0 * scale;
                        c.write_u32(m + 4 * k, x.to_bits());
                    }
                }
                match mix(s, 60) % 6 {
                    0 => {
                        let (a, b) = ((mix(s, 61) % 4) as u32, (mix(s, 62) % 4) as u32);
                        for j in 0..4u32 {
                            let x = c.read_u32(m + 16 * a + 4 * j);
                            c.write_u32(m + 16 * b + 4 * j, x);
                        }
                        if a == b {
                            for j in 0..4u32 {
                                c.write_u32(m + 16 * a + 4 * j, 0);
                            }
                        }
                    }
                    _ => {}
                }
            }),
            // the camera chain's inputs moved: positions /
            // directions / roll of the input, the record's constants, the
            // float arguments, the matrices; the record index 0 / 1.
            "exe::view_matrix 0x20CD50" => Arc::new(|c: &mut Cpu, s: u64| {
                let (pos, at) = (c.reg(5) as u32, c.reg(6) as u32);
                for k in 0..3u32 {
                    let x = f32::from_bits(c.read_u32(pos + 4 * k));
                    c.write_u32(pos + 4 * k, (x + cam_fl(s, u64::from(k))).to_bits());
                    if mix(s, 30) % 5 != 0 {
                        c.write_u32(at + 4 * k, (cam_fl(s, 10 + u64::from(k)) / 50.0).to_bits());
                    }
                }
                if mix(s, 31) % 2 == 0 {
                    c.set_reg(7, mix(s, 32) % 0x2_0000);
                }
            }),
            "exe::projection 0x20D050" => Arc::new(|c: &mut Cpu, s: u64| {
                for k in 0..8usize {
                    if mix(s, 50 + k as u64) % 3 == 0 {
                        let x = f32::from_bits(c.fpr[12 + k]);
                        c.fpr[12 + k] = (x * (0.5 + (mix(s, k as u64) % 1000) as f32 / 500.0) + cam_fl(s, 20 + k as u64) / 100.0).to_bits();
                    }
                }
                let sp = c.reg(29) as u32;
                if mix(s, 60) % 2 == 0 {
                    c.write_u32(sp, (cam_fl(s, 61).abs() * 50.0 + 1.0).to_bits());
                }
            }),
            "exe::camera_build 0x20E160" => Arc::new(|c: &mut Cpu, s: u64| cam_record_moved(c, s)),
            "exe::camera_set 0x20DAD0" => Arc::new(|c: &mut Cpu, s: u64| {
                cam_record_moved(c, s);
                let a = c.reg(4) as u32;
                for k in 0..3u32 {
                    let x = f32::from_bits(c.read_u32(a + 0x40 + 4 * k));
                    c.write_u32(a + 0x40 + 4 * k, (x + cam_fl(s, 70 + u64::from(k))).to_bits());
                    if mix(s, 80) % 4 != 0 {
                        c.write_u32(a + 0x50 + 4 * k, (cam_fl(s, 75 + u64::from(k)) / 50.0).to_bits());
                    }
                }
                if mix(s, 81) % 2 == 0 {
                    c.write_u32(a + 0x80, (mix(s, 82) % 0x1_0000) as u32);
                }
            }),
            "exe::camera_packet 0x20E3D0" => Arc::new(|c: &mut Cpu, s: u64| {
                cam_record_moved(c, s);
                let i = c.read_u32(0x002E_D6C0);
                let r = 0x002E_D720u32.wrapping_add(0x1A0u32.wrapping_mul(i));
                for k in 0..0x30u32 {
                    if mix(s, 200 + u64::from(k)) % 3 == 0 {
                        c.write_u32(r + 4 * k, (cam_fl(s, 300 + u64::from(k)) / 100.0).to_bits());
                    }
                }
                c.write_u32(r + 0x184, mix(s, 90) as u32);
            }),
            // the camera record's constants and the
            // callbacks' distance word moved (677: 0x1E04510; 500: D + 0xC).
            "exe::unit_target_pre_post 0x1F82B0" => Arc::new(|c: &mut Cpu, s: u64| {
                // The record index kept (record 1 is unset: a singular
                // view-projection leaves 0x20E3D0's local stale).
                let i = c.read_u32(0x002E_D6C0);
                cam_record_moved(c, s);
                c.write_u32(0x002E_D6C0, i);
                let dist = if c.reg(8) as u32 == 0x01E0_4F10 { c.read_u32(0x01E0_5100).wrapping_add(0xC) } else { 0x01E0_4510 };
                if mix(s, 190) % 2 == 0 {
                    c.write_u32(dist, (mix(s, 191) % 400) as u32);
                }
            }),
            "exe::load_cur 0x20B200" => Arc::new(|c: &mut Cpu, s: u64| {
                if mix(s, 1) % 4 == 0 {
                    c.set_reg(4, 0);
                }
            }),
            "exe::vu1_clip 0x208A70" => Arc::new(|c: &mut Cpu, s: u64| {
                for k in 0..3usize {
                    c.fpr[12 + k] = (cam_fl(s, k as u64) * 5.0).to_bits();
                }
                c.set_reg(5, mix(s, 9) % 3);
            }),
            "exe::vu1_fog 0x208B30" => Arc::new(|c: &mut Cpu, s: u64| {
                for k in 0..4usize {
                    c.fpr[12 + k] = (cam_fl(s, k as u64) * 20.0).to_bits();
                }
            }),
            // the saved quads anywhere (and the index 0 / 1).
            "exe::camera_save 0x20D660" => Arc::new(|c: &mut Cpu, s: u64| {
                if mix(s, 99) % 3 == 0 {
                    let i = c.read_u32(0x002E_D6C0);
                    c.write_u32(0x002E_D6C0, i ^ 1);
                }
                let i = c.read_u32(0x002E_D6C0);
                let src = 0x002E_D7E0u32.wrapping_add(0x1A0u32.wrapping_mul(i));
                for k in 0..0x24u32 {
                    c.write_u32(src + 4 * k, mix(s, u64::from(k)) as u32);
                }
            }),
            "exe::camera_turn_roll 0x2055E0" => Arc::new(|c: &mut Cpu, s: u64| {
                let obj = c.read_u32(0x002D_F5C0);
                if obj != 0 {
                    c.write_u32(obj + 0x40, (mix(s, 1) % 0x10000) as u32);
                }
            }),
            _ => return None,
        })
    }

    /// a value in -200..200 for the camera mutators.
    fn cam_fl(s: u64, k: u64) -> f32 {
        ((mix(s, k) % 4001) as f32 - 2000.0) / 10.0
    }

    /// the camera record index flipped 1 in 3 and the current
    /// record's constants (+0x150..0x184) moved (scaled 0.5..2.5 or
    /// shifted), 1 in 2 each.
    fn cam_record_moved(c: &mut Cpu, s: u64) {
        if mix(s, 99) % 3 == 0 {
            let i = c.read_u32(0x002E_D6C0);
            c.write_u32(0x002E_D6C0, i ^ 1);
        }
        let i = c.read_u32(0x002E_D6C0);
        let r = 0x002E_D720u32.wrapping_add(0x1A0u32.wrapping_mul(i));
        for k in 0..13u32 {
            if mix(s, 100 + u64::from(k)) % 2 == 0 {
                let x = f32::from_bits(c.read_u32(r + 0x150 + 4 * k));
                let y = if mix(s, 120 + u64::from(k)) % 2 == 0 { x * (0.5 + (mix(s, 140 + u64::from(k)) % 1000) as f32 / 500.0) } else { x + cam_fl(s, 160 + u64::from(k)) };
                c.write_u32(r + 0x150 + 4 * k, y.to_bits());
            }
        }
        let roll = r + 0x140;
        if mix(s, 180) % 2 == 0 {
            c.write_u32(roll, (mix(s, 181) % 0x1_0000) as u32);
        }
    }

    /// a seeded battle for the effect's handles: roles (own,
    /// target) either way or off-board / other slots, units missing 1 in 10,
    /// stats 0..9999 or below 0 / above 9999, ties forced (ATK = ATK or DEF
    /// = ATK), defence 1 in 3, the active side mostly own's (the tie rule),
    /// sometimes 3.
    fn outcome_battle(s: u64, own: u16, target: u16) -> roses_fx::types::BattleState {
        use roses_fx::types::{BattleState, BattleUnit};
        let r = |k: u64, n: u64| mix(s, k) % n;
        let any = |k: u64| ((r(k, 4) << 8) | r(k + 1, 12)) as u16;
        let roles = match r(1, 8) {
            0..=3 => [own, target],
            4 | 5 => [target, own],
            6 => [own, any(20)],
            _ => [any(22), any(24)],
        };
        let stat = |k: u64| match r(k, 8) {
            0 => -(r(k + 1, 3000) as i32),
            1 => 9999 + r(k + 1, 40000) as i32,
            _ => r(k + 1, 10000) as i32,
        };
        let unit = |k: u64| (r(k, 10) != 0).then(|| BattleUnit { attack: stat(k + 1), defence: stat(k + 3), defence_position: r(k + 5, 3) == 0 });
        let mut units = [unit(30), unit(40)];
        if let (Some(a), Some(b)) = (units[0], units[1].as_mut()) {
            match r(50, 6) {
                0 | 1 => b.attack = a.attack,
                2 => b.defence = a.attack,
                _ => {}
            }
        }
        let active_side = match r(60, 6) {
            0 => 3,
            1 | 2 => r(61, 3) as u8,
            _ => (own >> 8) as u8,
        };
        BattleState { roles, active_side, cell: (r(70, 7) as u8, r(71, 7) as u8), units }
    }

    /// the outcome head (`BattleState::outcome`, what
    /// `Fx::outcome` answers) equals the original closure 0x1F8100 run in
    /// the interpreter on 20,000 seeded battles mirrored into the plain
    /// image (`world::mirror_battle`), for both harness handle orders and
    /// off-board handles; the answers' spread is printed.
    #[test]
    #[ignore = "requires ROSES_TEST_GAME_DIR (the user's import)"]
    fn private_outcome_head_equals_the_interpreter_on_random_battles() {
        let g = game();
        let mut cpu = Cpu::new();
        cpu.load_elf(&g.exe).unwrap();
        let pairs = [(0x101u16, 0x001u16), (0x003, 0x104), (0x001, 0x101), (0x204, 0x00A), (0x30A, 0x002), (0x10B, 0x101)];
        let (mut bad, mut ones, n) = (Vec::new(), 0usize, 20_000u64);
        let mut codes = std::collections::BTreeMap::new();
        for k in 0..n {
            let s = mix(0xD30, k);
            let (own, target) = pairs[(mix(s, 99) % pairs.len() as u64) as usize];
            let st = outcome_battle(s, own, target);
            roses_fx::world::mirror_battle(&mut cpu, &st);
            let run = |cpu: &mut Cpu, pc: u32, args: &[u64]| cpu.call(pc, args, 0x01F0_0000, 1_000_000, &mut |_c, _pc| Hook::Run).map(|r| r as u32 as i32).unwrap();
            run(&mut cpu, 0x001F_6580, &[u64::from(target)]);
            run(&mut cpu, 0x001F_65A0, &[u64::from(own)]);
            let v = run(&mut cpu, 0x001F_8100, &[]);
            let flag = u32::from(st.roles[0]) != u32::from(own);
            let code = run(&mut cpu, 0x001F_8160, &[u64::from(flag)]);
            let want = st.outcome(u32::from(own), u32::from(target));
            *codes.entry(code).or_insert(0usize) += 1;
            ones += usize::from(v == 1);
            if v != want || code != st.answer(u32::from(own), u32::from(target), flag) {
                if bad.len() < 12 {
                    bad.push(format!("seed {s:#x} own {own:#x} target {target:#x}: interpreter {v} / {code}, head {want} / {}: {st:?}", st.answer(u32::from(own), u32::from(target), flag)));
                } else {
                    bad.push(String::new());
                }
            }
        }
        for b in bad.iter().filter(|b| !b.is_empty()) {
            eprintln!("  {b}");
        }
        eprintln!("outcome head: {n} battles, {} differing; answer 1 in {ones}; 0x1F8160 codes {codes:?}", bad.len());
        assert!(bad.is_empty());
    }

    /// The executable helpers equal the original at real calls
    /// (`ROSES_FNPARITY_ONLY` filters by name as above).
    #[test]
    #[ignore = "requires ROSES_TEST_GAME_DIR (the user's import)"]
    fn private_library_exe_helpers_equal_the_interpreter() {
        let g = game();
        let only = std::env::var("ROSES_FNPARITY_ONLY").ok();
        let mut bad = Vec::new();
        for &(name, addr, words, host, host_words) in EXE_HELPERS {
            if only.as_deref().is_some_and(|o| !name.contains(o)) {
                continue;
            }
            let at: Vec<(u16, u32)> = instances(&g.bmodule, host, host_words).into_iter().map(|(card, _)| (card, addr)).collect();
            let mut r = check(&g.exe, &g.bmodule, &g.textures, &at, words, exe_adapter(name), None, 24, 400, 24);
            // perturbed arguments where a mutator is given.
            if let Some(m) = exe_mutator(name) {
                let rp = check(&g.exe, &g.bmodule, &g.textures, &at, words, exe_adapter(name), Some(m), 24, 400, 24);
                for d in &rp.diffs {
                    eprintln!("  perturbed: {d}");
                }
                r.merge(&rp);
            }
            for d in &r.diffs {
                eprintln!("  {d}");
            }
            eprintln!("{name}: {} samples ({} perturbed) from {} cards, {} differing, words executed {} / {words}", r.samples, r.perturbed, r.cards.len(), r.differing, r.covered.len());
            assert!(r.samples > 0, "{name}: no call sampled");
            if !r.equal() {
                bad.push(name);
            }
        }
        assert!(bad.is_empty(), "differ: {bad:?}");
    }
}
