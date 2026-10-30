//! Research: checks the mirrored boundary of `roses_ee::bmodule` against
//! the game's own RAM. From a private footage state (EE memory image taken
//! during a B module's run) it runs the module's next updates twice:
//! (raw) on the footage RAM itself, (mirror) on the plain executable with
//! only the module's data area, the rand state and the node requests copied
//! from the footage and everything else written by the mirror from values
//! read out of the footage (camera eye / target / distance, the unit
//! fields, the node results). Prints per update the primitive counts and
//! the largest vertex / colour / UV difference (TEX0 compared without its
//! TBP0 / CBP, which the shim assigns itself).
//! `bmodule_mirror_check <state.bin> <SLES_515.79> <BMODULE.MRG> <card> <attacker obj hex> <defender obj hex> <self handle hex> <target handle hex> <updates>`
use roses_ee::bmodule::{AttackEffect, CameraMirror, EffectTextures, EffectWorld, GsPrim, OBJECT_RECORD, UnitMirror, decode_descriptor};
use roses_ee::{Cpu, Hook};

struct DumpWorld {
    ram: Vec<u8>,
    objects: [u32; 2],
}

impl DumpWorld {
    fn f(&self, a: u32) -> f32 {
        let a = (a & 0x1FF_FFFF) as usize;
        f32::from_le_bytes(self.ram[a..a + 4].try_into().unwrap())
    }
    fn u(&self, a: u32) -> u32 {
        let a = (a & 0x1FF_FFFF) as usize;
        u32::from_le_bytes(self.ram[a..a + 4].try_into().unwrap())
    }
    fn b(&self, a: u32) -> u8 {
        self.ram[(a & 0x1FF_FFFF) as usize]
    }
}

impl EffectWorld for DumpWorld {
    fn camera(&self) -> CameraMirror {
        let c = 0x1FE_04E0;
        CameraMirror {
            eye: [self.f(c + 0x20), self.f(c + 0x24), self.f(c + 0x28)],
            target: [self.f(c + 0x30), self.f(c + 0x34), self.f(c + 0x38)],
            distance: self.f(c + 0x44),
            roll: self.f(c + 0x40),
        }
    }
    fn unit(&self, role: usize) -> UnitMirror {
        let o = self.objects[role];
        UnitMirror {
            position: [self.f(o), self.f(o + 4), self.f(o + 8)],
            rotation: [self.u(o + 0x10) as i32, self.u(o + 0x14) as i32, self.u(o + 0x18) as i32],
            animated: [self.f(o + 0x80), self.f(o + 0x84), self.f(o + 0x88)],
            motion: self.b(o + 0x1E0),
            time: self.f(o + 0x19C),
            speed: self.f(o + 0x198),
            play_mode: (self.b(o + 0x1F7) as i8) >> 6,
            frozen: self.b(o + 0x20D) & 1 != 0,
            frames: std::array::from_fn(|m| self.u(o + 0x1C0 + 4 * m as u32)),
            alpha: self.b(o + 0x12F),
            nodes: self.b(o + 0x1FE),
            model: (self.u(o + 0x204) >> 16) as u16,
            bounds: [self.f(o + 0x90), self.f(o + 0x94), self.f(o + 0x98), self.f(o + 0x1E4)],
        }
    }
    fn node(&self, role: usize, node: u8) -> Option<[f32; 3]> {
        let o = self.objects[role];
        (0..8u32).find_map(|s| {
            let key = (self.u(o + 0x1B0 + (2 * s & !3)) >> (16 * (s & 1))) as u16;
            (key & 0x8000 != 0 && key as u8 == node).then(|| [self.f(o + 0xA0 + 16 * s), self.f(o + 0xA4 + 16 * s), self.f(o + 0xA8 + 16 * s)])
        })
    }
}

fn raw_update(cpu: &mut Cpu, entry: u32) -> (i32, Vec<GsPrim>) {
    let mut prims = Vec::new();
    let r = cpu
        .call(entry, &[0x1ED_0000, -1i64 as u64], 0x1F0_0000, 300_000_000, &mut |c, pc| match pc {
            0x206F30 => {
                let d = c.reg(4) as u32;
                if std::env::var_os("TRACE_OT").is_some() && prims.len() % 500 == 0 {
                    let ot = c.reg(5) as u32;
                    let base = c.read_u32(ot);
                    let lg = c.read_u32(ot + 12);
                    let e: Vec<String> = (0..6).map(|k| format!("{:x}:{:08x}", base + 8 * k, c.read_u32(base + 8 * k + 4))).collect();
                    eprintln!("prim {} ot {ot:x} key {} base {base:x} log2 {lg} entries {e:?}", prims.len(), c.reg(6) as i64);
                }
                if let Some(p) = decode_descriptor(c, d) {
                    prims.push(p);
                }
                Hook::Return(Some(0))
            }
            0x206F90 => Hook::Return(Some(c.reg(4))),
            0x209640 | 0x207FC0 if std::env::var_os("TRACE_FRAME").is_some() => {
                eprintln!("raw {pc:x}: a0 {:x} a1 {:x} a2 {:x} a3 {:x} t0 {:x} 2e9430 {:x} 2e9520 {:x} cnt {:x}", c.reg(4), c.reg(5), c.reg(6), c.reg(7), c.reg(8), c.read_u32(0x2E9430), c.read_u32(0x2E9520), c.read_u32(0x2E963C));
                Hook::Run
            }
            0x1EF6F0 | 0x1EF8F0 => Hook::Return(Some(c.reg(4))),
            0x19A410 | 0x1A9080 => Hook::Return(Some(0)),
            _ => Hook::Run,
        })
        .unwrap() as u32 as i32;
    (r, prims)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let hex = |s: &str| u32::from_str_radix(s.trim_start_matches("0x"), 16).unwrap();
    let ram = std::fs::read(&a[1]).unwrap();
    let exe = std::fs::read(&a[2]).unwrap();
    let bmodule = std::fs::read(&a[3]).unwrap();
    let card: u16 = a[4].parse().unwrap();
    let objects = [hex(&a[5]), hex(&a[6])];
    let handles = [hex(&a[7]) as u16, hex(&a[8]) as u16];
    let updates: usize = a[9].parse().unwrap();
    let world = DumpWorld { ram: ram.clone(), objects };

    // Raw: the footage RAM as it is.
    let mut raw = Cpu::new();
    raw.load(0, &ram[..ram.len().min(roses_ee::MAIN_BYTES)]);
    let (_, entry) = roses_ee::bmodule::entry_of(&mut raw, card).unwrap();
    let hk = &mut |_: &mut Cpu, _: u32| Hook::Run;
    raw.call(0x1F6580, &[u64::from(handles[1])], 0x1F0_0000, 10_000, hk).unwrap();
    raw.call(0x1F65A0, &[u64::from(handles[0])], 0x1F0_0000, 10_000, hk).unwrap();

    // Mirror: the plain executable plus the module's own state.
    let mut fx = AttackEffect::load(&exe, &bmodule, card, handles, &EffectTextures::default()).unwrap().unwrap();
    fx.cpu.load(0x1ED_0000, &ram[0x1ED_0000..0x1EE_0000]);
    fx.cpu.load(0x28_EAD0, &ram[0x28_EAD0..0x28_EAE0]);
    fx.cpu.load(0x28_ED10, &ram[0x28_ED10..0x28_ED20]);
    for role in 0..2 {
        let (o, m) = (objects[role] as usize, OBJECT_RECORD[role]);
        fx.cpu.load(m + 0x1B0, &ram[o + 0x1B0..o + 0x1C0]);
        fx.cpu.load(m + 0x1F9, &ram[o + 0x1F9..o + 0x1FA]);
    }
    // Texture headers the data area points at (types 4 / 5 of the table).
    for i in 0..1024usize {
        let e = 0x2E_5E30 + 12 * i;
        let ptr = u32::from_le_bytes(ram[e..e + 4].try_into().unwrap()) as usize & 0x1FF_FFFF;
        let kind = i16::from_le_bytes(ram[e + 8..e + 10].try_into().unwrap()) >> 3;
        if ptr != 0 && (kind == 4 || kind == 5) {
            fx.cpu.load(ptr as u32, &ram[ptr..ptr + 0x60]);
        }
    }
    fx.calls = 1;
    // A second mirror run without the native helpers (must be identical).
    let mut plain = AttackEffect::load(&exe, &bmodule, card, handles, &EffectTextures::default()).unwrap().unwrap();
    plain.cpu.mem.copy_from_slice(&fx.cpu.mem);
    plain.calls = 1;
    plain.hle = false;
    for k in 0..updates {
        // Reads of the raw update whose value differs in the mirror image.
        fx.mirror(&world);
        let before_raw = raw.mem.clone();
        let reads = std::sync::Arc::new(std::sync::Mutex::new(std::collections::BTreeMap::<u32, u32>::new()));
        let r2 = reads.clone();
        raw.tracer = Some(Box::new(move |pc, addr, store| {
            if !store && addr >> 28 != 7 {
                r2.lock().unwrap().entry(addr & 0x1FF_FFFF & !3).or_insert(pc);
            }
        }));
        let (rr, rp) = raw_update(&mut raw, entry);
        raw.tracer = None;
        if let Ok(path) = std::env::var("DUMP_PRIMS") {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path).unwrap();
            for (i, p) in rp.iter().enumerate() {
                for v in &p.vertices {
                    writeln!(f, "{k} {i} {} {:x} {} {} {} {} {} {} {} {} {} {:x} {:x}", p.kind, p.flags, v.x + 0.0, v.y, v.z, v.rgba[0], v.rgba[1], v.rgba[2], v.rgba[3], v.uv[0], v.uv[1], p.tex0.unwrap_or(0), p.alpha.unwrap_or(0)).unwrap();
                }
            }
        }
        let mut diffs = Vec::new();
        for (a, pc) in reads.lock().unwrap().iter() {
            let a = *a as usize;
            if (0x1E0_0000..0x1F0_0000).contains(&a) {
                continue;
            }
            if before_raw[a..a + 4] != fx.cpu.mem[a..a + 4] {
                diffs.push(format!("{a:x}@{pc:x} {:08x}/{:08x}", u32::from_le_bytes(before_raw[a..a + 4].try_into().unwrap()), u32::from_le_bytes(fx.cpu.mem[a..a + 4].try_into().unwrap())));
            }
        }
        println!("update +{k}: {} differing reads: {}", diffs.len(), diffs.iter().take(60).cloned().collect::<Vec<_>>().join(" "));
        let t0 = std::time::Instant::now();
        let m = fx.update(&world).unwrap();
        let t1 = t0.elapsed();
        let t0 = std::time::Instant::now();
        let n = plain.update(&world).unwrap();
        let t2 = t0.elapsed();
        println!("  hle {:?} ({} steps) vs plain {:?} ({} steps): identical {}", t1, m.steps, t2, n.steps, m.prims == n.prims && m.ret == n.ret);
        let mut worst = (0.0f32, 0u32, 0.0f32, String::new());
        let f34 = |v: &[GsPrim]| v.iter().find(|p| p.flags == 0xA34 || p.flags == 0x226).map(|p| format!("{:x?} {:?}", p.tex0, p.vertices.iter().map(|v| (v.x, v.y, v.uv, v.rgba)).collect::<Vec<_>>()));
        println!("  0xA34 TEX0 raw {:?} mirror {:?}", f34(&rp), f34(&m.prims));
        if let Some(p) = rp.iter().find(|p| p.flags == 0x226) {
            let words: Vec<String> = (0..16).map(|i| format!("{:016x}", raw.read_u64(p.desc + 8 * i))).collect();
            println!("  desc {:x}: {}", p.desc, words.join(" "));
        }
        let mut kinds = std::collections::BTreeMap::new();
        for (p, q) in rp.iter().zip(&m.prims) {
            *kinds.entry((p.flags, p.vertices.len())).or_insert(0) += 1;
            if p.flags != q.flags || p.vertices.len() != q.vertices.len() || p.alpha != q.alpha {
                worst.3 = format!("structure differs: {:#x}/{} vs {:#x}/{}", p.flags, p.vertices.len(), q.flags, q.vertices.len());
                continue;
            }
            let mask = !(0x3FFFu64 | (0x3FFF << 37));
            if p.tex0.map(|t| t & mask) != q.tex0.map(|t| t & mask) {
                worst.3 = format!("tex0 {:x?} vs {:x?}", p.tex0, q.tex0);
            }
            for (v, w) in p.vertices.iter().zip(&q.vertices) {
                worst.0 = worst.0.max((v.x - w.x).abs()).max((v.y - w.y).abs());
                worst.1 = worst.1.max((0..4).map(|i| v.rgba[i].abs_diff(w.rgba[i]) as u32).max().unwrap());
                worst.2 = worst.2.max((v.uv[0] - w.uv[0]).abs()).max((v.uv[1] - w.uv[1]).abs());
            }
        }
        println!(
            "update +{k}: ret raw {rr} mirror {}; prims {} / {}; max |dxy| {:.4} px, |drgba| {}, |duv| {:.4} {}; kinds {kinds:x?}; boundary {:?} missing tex {:?}",
            m.ret,
            rp.len(),
            m.prims.len(),
            worst.0,
            worst.1,
            worst.2,
            worst.3,
            m.boundary.iter().map(|b| b.1).collect::<Vec<_>>(),
            m.missing_textures
        );
    }
}
