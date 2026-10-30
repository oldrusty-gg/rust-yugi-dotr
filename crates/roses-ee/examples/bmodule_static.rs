//! Research: run one B module against a static arena (camera, two units,
//! every node at the unit position + 20 up) and print what it draws per
//! update: primitive kinds, textures, and TEX0 values not mapped to a shim
//! texture (with the 0x202530 requests). No textures are placed.
//! `bmodule_static <SLES_515.79> <BMODULE.MRG> <card> [updates] [TEXEFF.MRG MONSTER.MRG]`
use roses_ee::bmodule::{AttackEffect, CameraMirror, EffectTextures, EffectWorld, UnitMirror};

struct Static;

impl EffectWorld for Static {
    fn camera(&self) -> CameraMirror {
        CameraMirror { eye: [60.0, 70.0, 10.0], target: [0.0, 50.0, -40.0], distance: 675.0, roll: 0.0 }
    }
    fn unit(&self, role: usize) -> UnitMirror {
        let z = if role == 0 { 80.0 } else { -80.0 };
        UnitMirror {
            position: [0.0, 50.0, z],
            rotation: [0, if role == 0 { 0x8000 } else { 0 }, 0],
            animated: [0.0, 55.0, z],
            motion: if role == 0 { 2 } else { 0 },
            time: 30.0,
            speed: 1.0,
            play_mode: 1,
            frozen: false,
            frames: [61, 0, 241, 0, 81, 76, 21, 0],
            alpha: 0x80,
            nodes: 80,
            model: 0,
            bounds: [0.0, 0.0, 0.0, 20.0],
        }
    }
    fn node(&self, role: usize, _node: u8) -> Option<[f32; 3]> {
        let p = self.unit(role).position;
        Some([p[0], p[1] + 20.0, p[2]])
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let exe = std::fs::read(&a[1]).unwrap();
    let bmodule = std::fs::read(&a[2]).unwrap();
    let card: u16 = a[3].parse().unwrap();
    let updates: usize = a.get(4).map_or(400, |s| s.parse().unwrap());
    let mut textures = EffectTextures::default();
    if let (Some(t), Some(m)) = (a.get(5), a.get(6)) {
        let texeff = std::fs::read(t).unwrap();
        let archive = roses_assets::inspect_pal_opening_archive(&texeff, roses_assets::OpeningArchiveKind::TexEff).unwrap();
        textures.texeff = archive.records.iter().map(|r| Some(texeff[r.file_offset as usize..r.file_offset as usize + r.byte_len as usize].to_vec())).collect();
        let monster = std::fs::read(m).unwrap();
        let slot = &monster[usize::from(card) * 0x10_0000..(usize::from(card) + 1) * 0x10_0000];
        let model = roses_assets::decode_pal_monster_slot(slot, card).unwrap();
        textures.own = vec![None; 8];
        for t in &model.effect_textures {
            let o = t.slot_offset as usize;
            textures.own[usize::from(t.effect_slot)] = Some(slot[o..o + t.byte_len as usize].to_vec());
        }
    }
    let mut fx = AttackEffect::load(&exe, &bmodule, card, [0x101, 0x001], &textures).unwrap().unwrap();
    // STORES=1: the store targets per (pc, 0x10-byte block) outside the
    // stack, printed at the end.
    let stores: std::sync::Arc<std::sync::Mutex<std::collections::BTreeMap<u32, (u32, usize)>>> = Default::default();
    if std::env::var_os("STORES").is_some() {
        let s2 = stores.clone();
        fx.cpu.tracer = Some(Box::new(move |pc, addr, store| {
            let a = addr & 0x1FF_FFFF;
            if store && addr >> 28 != 7 && !(0x1F0_0000..0x200_0000).contains(&a) {
                let mut g = s2.lock().unwrap();
                let e = g.entry(a & !0xF).or_insert((pc, 0));
                e.1 += 1;
            }
        }));
    }
    for k in 0..updates {
        let u = fx.update(&Static).unwrap();
        let unmapped: Vec<String> = u
            .prims
            .iter()
            .filter(|p| p.textured() && p.texture.is_none() && p.tex0.is_some_and(|t| !matches!(t & 0x3FFF, 0 | 0x800)))
            .take(3)
            .map(|p| format!("flags {:x} tex0 {:016x} kind {} n {}", p.flags, p.tex0.unwrap(), p.kind, p.vertices.len()))
            .collect();
        if let Some(p) = u.prims.iter().find(|p| p.textured() && p.texture.is_none() && p.tex0.is_some_and(|t| !matches!(t & 0x3FFF, 0 | 0x800))) {
            if std::env::var_os("DESC").is_some() {
                let words: Vec<String> = (0..24).map(|i| format!("{:016x}", fx.cpu.read_u64(p.desc + 8 * i))).collect();
                println!("desc {:x}: {}", p.desc, words.join(" "));
            }
        }
        if std::env::var_os("SUMMARY").is_some() {
            use roses_ee::bmodule::TextureKey;
            let rt = u.prims.iter().filter(|p| matches!(p.texture, Some(TextureKey::UnitRt(_)))).count();
            let frame = u.prims.iter().filter(|p| p.texture == Some(TextureKey::Frame)).count();
            let rtex: Vec<String> = u.prims.iter().filter(|p| matches!(p.texture, Some(TextureKey::UnitRt(_)))).take(1).map(|p| format!("{:016x} flags {:x} v {:?}", p.tex0.unwrap(), p.flags, p.vertices.iter().map(|v| (v.x, v.y, v.uv, v.rgba)).collect::<Vec<_>>())).collect();
            let _ = rtex;
            let keys: std::collections::BTreeSet<String> = u.prims.iter().map(|p| format!("{:?}/{:x}", p.texture, p.alpha.unwrap_or(0) & 0xFF)).collect();
            let ots: std::collections::BTreeSet<Option<i32>> = u.prims.iter().map(|p| p.ot_key).collect();
            let w: Vec<String> = u.unit_writes.iter().map(|w| format!("t{:?}a{:?}s{:?}", w.tint, w.add, w.scale)).collect();
            println!("{k}: light {} ret {} prims {} renders {:?} rt {rt} frame {frame} unmapped {} layers {} ots {:?} tex {:?} w {:?}", u.stage_light, u.ret, u.prims.len(), u.unit_renders.iter().map(|r| (r.role, r.before, r.pre, r.extra)).collect::<Vec<_>>(), unmapped.len(), u.layers.len(), ots, keys, w);
            if u.ret == 8 {
                break;
            }
            continue;
        }
        if !u.prims.is_empty() || !u.missing_textures.is_empty() {
            println!(
                "{k}: ret {} prims {} missing {:?} boundary {:?} unmapped {:?}",
                u.ret,
                u.prims.len(),
                u.missing_textures.iter().take(4).collect::<Vec<_>>(),
                u.boundary.iter().map(|b| b.1).collect::<std::collections::BTreeSet<_>>(),
                unmapped
            );
        }
        if u.ret == 8 {
            break;
        }
    }
    if std::env::var_os("STORES").is_some() {
        for (a, (pc, n)) in stores.lock().unwrap().iter() {
            println!("store {a:07x} first pc {pc:07x} x{n}");
        }
    }
}
