//! Research runner for the Exodia event (handler 0x26DAA0): run CMODULE
//! programs c061 / c062 / c063 (0x3D..0x3F) as original code with a fixed
//! board camera and print, per update, the return, the primitive count and
//! the boundary calls the shim does not implement.
//! `exodia_modules <SLES_515.79> <CMODULE.MRG> <TEXEFF.MRG> <id hex> <self hex> <target hex> [eye x,y,z] [target x,y,z]`
use roses_ee::bmodule::{AttackEffect, CameraMirror, EffectTextures, EffectWorld, UnitMirror};

struct Board(CameraMirror);

impl EffectWorld for Board {
    fn camera(&self) -> CameraMirror {
        self.0
    }
    fn unit(&self, _role: usize) -> UnitMirror {
        UnitMirror {
            position: [0.0; 3],
            rotation: [0; 3],
            animated: [0.0; 3],
            motion: 0,
            time: 0.0,
            speed: 1.0,
            play_mode: 0,
            frozen: false,
            frames: [0; 8],
            alpha: 0x80,
            nodes: 0,
            model: 0,
            bounds: [0.0; 4],
        }
    }
    fn node(&self, _role: usize, _node: u8) -> Option<[f32; 3]> {
        None
    }
}

fn texeff_records(texeff: &[u8]) -> Vec<Option<Vec<u8>>> {
    match roses_assets::inspect_pal_opening_archive(texeff, roses_assets::OpeningArchiveKind::TexEff) {
        Ok(a) => a
            .records
            .iter()
            .map(|r| {
                let s = r.file_offset as usize;
                texeff.get(s..s + r.byte_len as usize).map(<[u8]>::to_vec)
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let hex = |s: &str| u16::from_str_radix(s.trim_start_matches("0x"), 16).unwrap();
    let vec3 = |s: &str| -> [f32; 3] {
        let v: Vec<f32> = s.split(',').map(|x| x.parse().unwrap()).collect();
        [v[0], v[1], v[2]]
    };
    let exe = std::fs::read(&a[1]).unwrap();
    let cmodule = std::fs::read(&a[2]).unwrap();
    let texeff = std::fs::read(&a[3]).unwrap();
    let (id, own, target) = (hex(&a[4]), hex(&a[5]), hex(&a[6]));
    let eye = a.get(7).map_or([0.0, 300.0, 300.0], |s| vec3(s));
    let look = a.get(8).map_or([0.0, 0.0, 0.0], |s| vec3(s));
    let cam = CameraMirror { eye, target: look, distance: 800.0, roll: 0.0 };
    let textures = EffectTextures { texeff: texeff_records(&texeff), own: Vec::new(), other: Vec::new() };
    let mut fx = AttackEffect::load_cmodule(&exe, &cmodule, id, [own, target], &textures).unwrap().expect("module");
    let world = Board(cam);
    type Seen = std::collections::BTreeMap<u32, (u32, u32, bool, bool)>;
    let reads: std::sync::Arc<std::sync::Mutex<Seen>> = Default::default();
    if std::env::var_os("TRACE").is_some() {
        let s2 = reads.clone();
        fx.cpu.tracer = Some(Box::new(move |pc, addr, store| {
            let addr = addr & 0x1FF_FFFF | if addr >> 28 == 7 { 0x7000_0000 } else { 0 };
            if (0x1E0_0000..0x200_0000).contains(&addr) || (0x1A0_0000..0x1E0_0000).contains(&addr) || addr >= 0x7000_0000 || addr < 0x10_0000 {
                return;
            }
            if (0x1E0_0000..0x1E1_8000).contains(&pc) || true {
                let mut m = s2.lock().unwrap();
                let e = m.entry(addr & !0xF).or_insert((pc, 0, false, false));
                e.1 += 1;
                if store { e.3 = true } else { e.2 = true }
            }
        }));
    }
    let mut n = 0;
    let mut seen = std::collections::BTreeSet::new();
    while !fx.finished && n < 2000 {
        match fx.update(&world) {
            Ok(u) => {
                let textured = u.prims.iter().filter(|p| p.textured()).count();
                let kinds: std::collections::BTreeMap<u8, usize> = u.prims.iter().fold(Default::default(), |mut m, p| {
                    *m.entry(p.kind).or_default() += 1;
                    m
                });
                let alphas: std::collections::BTreeSet<u64> = u.prims.iter().filter_map(|p| p.alpha.map(|a| a & 0xFF)).collect();
                let tex: std::collections::BTreeSet<String> = u.prims.iter().filter_map(|p| p.texture.map(|t| format!("{t:?}"))).collect();
                println!(
                    "u{n:3} ret {} prims {} textured {} kinds {:?} alpha {:?} tex {:?} light {} renders {} writes {:?} missing {:?}",
                    u.ret,
                    u.prims.len(),
                    textured,
                    kinds,
                    alphas,
                    tex,
                    u.stage_light,
                    u.unit_renders.len(),
                    u.unit_writes.iter().map(|w| (w.mode, w.fade)).collect::<Vec<_>>(),
                    u.missing_textures
                );
                for (addr, name, args) in &u.boundary {
                    if seen.insert((*addr, *args)) {
                        println!("    boundary {addr:#x} {name} {args:x?}");
                    }
                }
            }
            Err(e) => {
                println!("u{n} stop: {}", e.0);
                break;
            }
        }
        n += 1;
    }
    println!("ended after {n} updates (finished {})", fx.finished);
    for (addr, (pc, count, r, w)) in reads.lock().unwrap().iter() {
        if *addr >= 0x28_0000 || *r {
            println!("  {addr:#09x} first pc {pc:#08x} x{count} {}{}", if *r { "R" } else { "" }, if *w { "W" } else { "" });
        }
    }
}
