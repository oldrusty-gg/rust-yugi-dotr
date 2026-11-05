//! Private parity checks of the hand-ported CMODULE programs c401 and
//! c061..c063 (`roses_fx::c401`, `roses_fx::c06x`) against the interpreter,
//! update by update (`roses_ee::parity::run`). Ignored: they need the
//! user's import (`ROSES_TEST_GAME_DIR`: `duel-arena/{SLES_515.79,
//! CMODULE.MRG}`, `duel/TEXEFF.MRG`, `clouds/TEXEVE.MRG`).
//! `cargo test -p roses-ee --release --test parity_cmodule -- --ignored`.
use roses_ee::bmodule::{AttackEffect, CameraMirror, EffectTextures, EffectWorld, UnitMirror};
use roses_ee::parity::{memory_diff, run, Parity};
use roses_fx::EffectPort;

struct Game {
    exe: Vec<u8>,
    cmodule: Vec<u8>,
    texeve: Option<Vec<u8>>,
    texeff: Vec<Option<Vec<u8>>>,
}

fn game() -> Game {
    let dir = std::path::PathBuf::from(std::env::var_os("ROSES_TEST_GAME_DIR").expect("ROSES_TEST_GAME_DIR"));
    let exe = std::fs::read(dir.join("duel-arena/SLES_515.79")).unwrap();
    let cmodule = std::fs::read(dir.join("duel-arena/CMODULE.MRG")).unwrap();
    let texeve = ["clouds/TEXEVE.MRG", "story/TEXEVE.MRG", "TEXEVE.MRG"].iter().find_map(|p| std::fs::read(dir.join(p)).ok());
    let tex = std::fs::read(dir.join("duel/TEXEFF.MRG")).unwrap();
    let texeff = match roses_assets::inspect_pal_opening_archive(&tex, roses_assets::OpeningArchiveKind::TexEff) {
        Ok(a) => a.records.iter().map(|r| tex.get(r.file_offset as usize..r.file_offset as usize + r.byte_len as usize).map(<[u8]>::to_vec)).collect(),
        Err(_) => Vec::new(),
    };
    Game { exe, cmodule, texeve, texeff }
}

fn texeve_record(texeve: &[u8], id: usize) -> Option<Vec<u8>> {
    let a = roses_assets::inspect_pal_opening_archive(texeve, roses_assets::OpeningArchiveKind::TexEve).ok()?;
    let r = a.records.get(id)?;
    let s = r.file_offset as usize;
    texeve.get(s..s + r.byte_len as usize).map(<[u8]>::to_vec)
}

/// A story scene: a camera only.
struct Scene(CameraMirror);
impl EffectWorld for Scene {
    fn camera(&self) -> CameraMirror {
        self.0
    }
    fn unit(&self, _role: usize) -> UnitMirror {
        UnitMirror { position: [0.0; 3], rotation: [0; 3], animated: [0.0; 3], motion: 0, time: 0.0, speed: 1.0, play_mode: 0, frozen: false, frames: [0; 8], alpha: 0x80, nodes: 0, model: 0, bounds: [0.0; 4] }
    }
    fn node(&self, _role: usize, _node: u8) -> Option<[f32; 3]> {
        None
    }
}

/// A camera looking at the beam's origin (0, 800, 0) from below, fixed
/// or orbiting.
fn story_camera(k: usize, case: usize) -> CameraMirror {
    if case == 6 {
        // 11162's camera (eye (0, 0, 512), target (0, 20.2 + 768 level /
        // 4096)) with the level as trace-seto-cam (story_fx's test).
        let level = (4096 - 256 * (k as i32 - 12).max(0)).max(0) as f32;
        return CameraMirror { eye: [0.0, 0.0, 512.0], target: [0.0, 20.2 + 768.0 * level / 4096.0, 0.0], distance: 800.0, roll: 0.0 };
    }
    if case == 5 {
        // Close to the rays, sweeping across them: many segments clipped.
        let a = k as f32 * 0.05;
        return CameraMirror { eye: [60.0 * a.cos(), 500.0, 60.0 * a.sin()], target: [0.0, 700.0 - 4.0 * k as f32, 0.0], distance: 900.0, roll: 0.0 };
    }
    let a = k as f32 * 0.015 * case as f32 + case as f32;
    let r = 600.0 + 40.0 * case as f32;
    CameraMirror { eye: [r * a.cos(), 120.0 + 30.0 * case as f32, r * a.sin()], target: [0.0, 300.0 + 50.0 * case as f32, 0.0], distance: 800.0 - 20.0 * case as f32, roll: 0.0 }
}

fn report(name: &str, p: &Parity) {
    for d in &p.diffs {
        eprintln!("{name} update {}:", d.update);
        for f in &d.fields {
            eprintln!("  {f}");
        }
    }
    eprintln!(
        "{name}: {} updates, {} primitives, returns 8 at {:?}, {} differing updates; interpreter {:?}, port {:?}",
        p.updates,
        p.prims,
        p.returns.iter().position(|&r| r == 8),
        p.diffs.len(),
        p.oracle_time,
        p.port_time
    );
}

fn c401_textures(g: &Game) -> EffectTextures {
    let other = g.texeve.as_deref().map(|t| [0x18, 0x19, 0x1A].iter().filter_map(|&id| texeve_record(t, id as usize).map(|b| (8, id, b))).collect()).unwrap_or_default();
    EffectTextures { texeff: Vec::new(), own: Vec::new(), other }
}

fn c401_case(case: usize, rand: Option<u64>) -> Parity {
    let g = game();
    let textures = c401_textures(&g);
    assert_eq!(textures.other.len(), 3, "TEXEVE 0x18..0x1A");
    let mut oracle = AttackEffect::load_cmodule(&g.exe, &g.cmodule, roses_fx::c401::ID, [0, 0], &textures).unwrap().expect("c401");
    let mut port = roses_fx::c401::C401::load(&g.exe, &g.cmodule, [0, 0], &textures).expect("c401 port");
    if let Some(s) = rand {
        oracle.set_rand_state(s);
        port.set_rand_state(s);
    }
    let p = run(&mut oracle, &mut port, 400, 4, |k| Box::new(Scene(story_camera(k, case))), |_, _, _| {});
    report(&format!("c401 case {case}"), &p);
    if let Some(d) = p.diffs.first() {
        let mem = memory_diff(&mut oracle, &mut port, 0x01E1_0000, 0x9A0);
        eprintln!("data-area bytes differing after update {}: {} (first {:x?})", d.update, mem.len(), &mem[..mem.len().min(16)]);
    }
    p
}

/// c401 hand port vs the interpreter: every update equal.
#[test]
#[ignore = "requires ROSES_TEST_GAME_DIR (the user's import)"]
fn private_c401_port_equals_the_interpreter() {
    let mut prims = 0;
    for (case, rand) in [(0, None), (1, Some(0x1234_5678_9ABC_DEF1)), (2, Some(7)), (3, Some(0xDEAD_BEEF)), (4, Some(0x0F0F_F0F0_1357_9BDF)), (5, Some(42)), (6, None)] {
        let p = c401_case(case, rand);
        assert!(p.equal(), "c401 differs in case {case}");
        assert_eq!(p.returns.len(), 0x72);
        prims += p.prims;
    }
    eprintln!("c401: {prims} primitives in all cases");
    // A divergence is seen: another rand state changes the rays.
    let g = game();
    let textures = c401_textures(&g);
    let mut oracle = AttackEffect::load_cmodule(&g.exe, &g.cmodule, roses_fx::c401::ID, [0, 0], &textures).unwrap().unwrap();
    let mut port = roses_fx::c401::C401::load(&g.exe, &g.cmodule, [0, 0], &textures).unwrap();
    port.set_rand_state(99);
    let p = run(&mut oracle, &mut port, 400, 1, |k| Box::new(Scene(story_camera(k, 0))), |_, _, _| {});
    assert!(!p.equal(), "a different rand state must be seen");
}

/// Summoned Lord Exodia (object 0x20 = role 0) as the finale's modules
/// read it: standing at the board's centre, playing its motions, its
/// skeleton nodes at fixed offsets; the board camera turning round it.
struct Finale(usize, usize);
impl EffectWorld for Finale {
    fn camera(&self) -> CameraMirror {
        let a = self.0 as f32 * 0.004 * (1 + self.1) as f32 + self.1 as f32;
        let r = 420.0 + 30.0 * self.1 as f32;
        CameraMirror { eye: [r * a.cos(), 160.0 + 20.0 * self.1 as f32, r * a.sin()], target: [0.0, 90.0, 0.0], distance: 800.0, roll: 0.0 }
    }
    fn unit(&self, role: usize) -> UnitMirror {
        let t = self.0 as f32;
        UnitMirror {
            position: [0.0; 3],
            rotation: [0, if role == 0 { 0 } else { 0x8000 }, 0],
            animated: [0.0, (t * 0.05).sin() * 3.0, 0.0],
            motion: 1,
            time: t % 60.0,
            speed: 1.0,
            play_mode: 1,
            frozen: false,
            frames: [60, 60, 120, 40, 30, 20, 20, 20],
            alpha: 0x80,
            nodes: if role == 0 { 200 } else { 0 },
            model: 0x29F,
            bounds: [0.0, 80.0, 0.0, 120.0],
        }
    }
    fn node(&self, role: usize, node: u8) -> Option<[f32; 3]> {
        (role == 0).then(|| {
            let n = f32::from(node);
            let t = self.0 as f32;
            [(n * 0.7).sin() * 40.0 + (t * 0.03).sin() * 5.0, 10.0 + n * 1.2, (n * 0.3).cos() * 30.0]
        })
    }
}

const LORD_MOTIONS: [(u8, u32); 5] = [(0, 60), (1, 60), (2, 120), (3, 40), (4, 30)];

fn finale_textures(g: &Game) -> EffectTextures {
    let own = (0..5).map(|k| g.texeff.get(10 + k).cloned().flatten()).collect();
    EffectTextures { texeff: g.texeff.clone(), own, other: Vec::new() }
}

fn c061_case(case: usize, side: u16, rand: Option<u64>) -> Parity {
    let g = game();
    let textures = finale_textures(&g);
    // Case 4: the target handle is an opponent unit (side 0, slot 2) the
    // timeline keeps idle (role 1 with motions), as the board's units.
    let handles = if case == 4 { [side, 0x002] } else { [side, 0x463] };
    let mut oracle = AttackEffect::load_cmodule(&g.exe, &g.cmodule, roses_fx::c061::ID, handles, &textures).unwrap().expect("c061");
    oracle.place_role(0, 0x20, &LORD_MOTIONS);
    let mut port = roses_fx::c061::C061::load(&g.exe, &g.cmodule, handles, &textures).expect("c061 port");
    port.place_role(0, 0x20, &LORD_MOTIONS);
    if case == 4 {
        oracle.place_role(1, 20, &LORD_MOTIONS);
        port.place_role(1, 20, &LORD_MOTIONS);
    }
    if let Some(s) = rand {
        oracle.set_rand_state(s);
        port.set_rand_state(s);
    }
    let p = run(&mut oracle, &mut port, 1200, 4, |k| Box::new(Finale(k, case)), |_, _, _| {});
    report(&format!("c061 case {case} side {side:#x}"), &p);
    let mem = memory_diff(&mut oracle, &mut port, 0x01E1_0000, 0x6B40);
    eprintln!("c061 data-area bytes differing at the end: {} (first {:x?})", mem.len(), &mem[..mem.len().min(24)]);
    let obj = memory_diff(&mut oracle, &mut port, 0x01FE_42D0, 0x220);
    eprintln!("c061 Lord object bytes differing at the end: {} (first {:x?})", obj.len(), &obj[..obj.len().min(24)]);
    let other = memory_diff(&mut oracle, &mut port, 0x01FE_3830, 0x220);
    let units = memory_diff(&mut oracle, &mut port, 0x002D_B990, 3 * 0x3F4);
    eprintln!("c061 role-1 object / board unit bytes differing at the end: {} / {}", other.len(), units.len());
    assert!(obj.is_empty() && other.is_empty() && units.is_empty());
    // The only data-area bytes allowed to differ: the rect's half the
    // original copies from its stack without writing (+0x9C..+0xA3).
    assert!(mem.iter().all(|m| (0x01E1_009C..0x01E1_00A4).contains(&m.0)), "{mem:x?}");
    p
}

/// c061 hand port vs the interpreter: every update equal.
#[test]
#[ignore = "requires ROSES_TEST_GAME_DIR (the user's import)"]
fn private_c061_port_equals_the_interpreter() {
    let mut prims = 0;
    for (case, side, rand) in [(0, 0u16, None), (1, 0x100, Some(0x1234_5678_9ABC_DEF1)), (2, 0, Some(7)), (3, 0x100, Some(0xDEAD_BEEF)), (4, 0x100, Some(0x5555))] {
        let p = c061_case(case, side, rand);
        assert!(p.equal(), "c061 differs in case {case}");
        prims += p.prims;
    }
    eprintln!("c061: {prims} primitives in all cases");
    let g = game();
    // What the port drew (coverage of the draw paths) in case 0.
    let textures = finale_textures(&g);
    let mut port = roses_fx::c061::C061::load(&g.exe, &g.cmodule, [0, 0x463], &textures).unwrap();
    port.place_role(0, 0x20, &LORD_MOTIONS);
    let mut kinds = std::collections::BTreeMap::new();
    let mut extra = std::collections::BTreeSet::new();
    for k in 0..960 {
        let u = port.update(&Finale(k, 0));
        for p in &u.prims {
            *kinds.entry((p.flags, p.vertices.len(), format!("{:?}", p.texture), p.ot_key.map(|k| k.min(1)))).or_insert(0usize) += 1;
        }
        for m in &u.missing_textures {
            extra.insert(format!("missing {m:?}"));
        }
        for b in &u.boundary {
            extra.insert(format!("boundary {:#x}", b.0));
        }
    }
    eprintln!("c061 draws (flags, vertices, texture, key>0): {kinds:?} {extra:?}");
    let textures = finale_textures(&g);
    let mut oracle = AttackEffect::load_cmodule(&g.exe, &g.cmodule, roses_fx::c061::ID, [0, 0x463], &textures).unwrap().unwrap();
    oracle.place_role(0, 0x20, &LORD_MOTIONS);
    let mut port = roses_fx::c061::C061::load(&g.exe, &g.cmodule, [0, 0x463], &textures).unwrap();
    port.place_role(0, 0x20, &LORD_MOTIONS);
    port.set_rand_state(99);
    let p = run(&mut oracle, &mut port, 1200, 1, |k| Box::new(Finale(k, 0)), |_, _, _| {});
    assert!(!p.equal(), "a different rand state must be seen");
}

/// Board card objects for the units c062 moves (unit records 0x2DB990 +
/// 0x3F4 side + 0x5C slot, +4 the card object): the limbs (packed slots of
/// 0x463 and the leader) and a few others, written into both memories.
fn place_cards(bus: &mut dyn roses_fx::bus::Bus, side: u32) {
    for slot in 0..11u32 {
        if slot > 6 {
            continue;
        }
        let rec = 0x002D_B990 + 0x3F4 * side + 0x5C * slot;
        let obj = 0x01F9_0000 + 0x400 * side + 0x80 * slot;
        bus.write_u32(rec + 4, obj);
        bus.write_u8(rec + 0x45, 0);
        bus.write_u8(rec + 0x46, if slot == 2 { 25 } else { 0 });
        let x = 50.0 * slot as f32 - 150.0;
        for (k, v) in [x, 0.0, if side == 0 { 120.0 } else { -120.0 }, 1.0].into_iter().enumerate() {
            bus.write_f32(obj + 4 * k as u32, v);
        }
        for (k, v) in [0x1000u32 * slot, if side == 0 { 0 } else { 0x8000 }, 0x300 * slot, 0].into_iter().enumerate() {
            bus.write_u32(obj + 0x10 + 4 * k as u32, v);
        }
        for k in 0..4 {
            bus.write_f32(obj + 0x30 + 4 * k, 1.0);
        }
        for k in 0..4 {
            bus.write_u8(obj + 0x40 + k, 0x80);
        }
    }
}

fn c062_case(case: usize, side: u16, rand: Option<u64>, cards: bool) -> Parity {
    let g = game();
    let textures = finale_textures(&g);
    let handles = [side, 0x463];
    let mut oracle = AttackEffect::load_cmodule(&g.exe, &g.cmodule, roses_fx::c062::ID, handles, &textures).unwrap().expect("c062");
    oracle.place_role(0, 0x20, &LORD_MOTIONS);
    let mut port = roses_fx::c062::C062::load(&g.exe, &g.cmodule, handles, &textures).expect("c062 port");
    port.place_role(0, 0x20, &LORD_MOTIONS);
    if cards {
        let s = u32::from(side >> 8);
        place_cards(&mut oracle.cpu, s);
        place_cards(port.bus().unwrap(), s);
    }
    if let Some(s) = rand {
        oracle.set_rand_state(s);
        port.set_rand_state(s);
    }
    let p = run(&mut oracle, &mut port, 1200, 4, |k| Box::new(Finale(k, case)), |_, _, _| {});
    report(&format!("c062 case {case} side {side:#x} cards {cards}"), &p);
    let mem = memory_diff(&mut oracle, &mut port, 0x01E1_0000, 0x6CB0);
    eprintln!("c062 data-area bytes differing at the end: {} (first {:x?})", mem.len(), &mem[..mem.len().min(24)]);
    let cardmem = memory_diff(&mut oracle, &mut port, 0x01F9_0000, 0x800);
    let obj = memory_diff(&mut oracle, &mut port, 0x01FE_42D0, 0x220);
    let other = memory_diff(&mut oracle, &mut port, 0x01FE_3830, 0x220);
    let units = memory_diff(&mut oracle, &mut port, 0x002D_B990, 3 * 0x3F4);
    eprintln!("c062 card / object / role-1 / unit bytes differing at the end: {} / {} / {} / {}", cardmem.len(), obj.len(), other.len(), units.len());
    let stage = port.bus().unwrap().read_u32(0x01E1_68FC);
    eprintln!("c062 stage counter at the end: {stage:#x}");
    if cards {
        // 0x3E: the last stage the limbs wait for (state 5 -> 6) [F].
        assert!(stage >= 0x3E, "the limbs' machine ran to its end");
    }
    assert!(mem.is_empty() && cardmem.is_empty() && obj.is_empty() && other.is_empty() && units.is_empty());
    p
}

/// c062 hand port vs the interpreter: every update equal.
#[test]
#[ignore = "requires ROSES_TEST_GAME_DIR (the user's import)"]
fn private_c062_port_equals_the_interpreter() {
    let mut prims = 0;
    for (case, side, rand, cards) in [(0, 0u16, None, false), (1, 0x100, Some(0x1234_5678_9ABC_DEF1), false), (2, 0, Some(7), true), (3, 0x100, Some(0xDEAD_BEEF), true)] {
        let p = c062_case(case, side, rand, cards);
        assert!(p.equal(), "c062 differs in case {case}");
        prims += p.prims;
    }
    eprintln!("c062: {prims} primitives in all cases");
    let g = game();
    let textures = finale_textures(&g);
    let mut oracle = AttackEffect::load_cmodule(&g.exe, &g.cmodule, roses_fx::c062::ID, [0, 0x463], &textures).unwrap().unwrap();
    let mut port = roses_fx::c062::C062::load(&g.exe, &g.cmodule, [0, 0x463], &textures).unwrap();
    place_cards(&mut oracle.cpu, 0);
    place_cards(port.bus().unwrap(), 0);
    port.set_rand_state(99);
    let p = run(&mut oracle, &mut port, 1200, 1, |k| Box::new(Finale(k, 0)), |_, _, _| {});
    assert!(!p.equal(), "a different rand state must be seen");
}

fn c063_case(case: usize, side: u16, rand: Option<u64>) -> Parity {
    let g = game();
    let textures = finale_textures(&g);
    let handles = [side, 0x463];
    let mut oracle = AttackEffect::load_cmodule(&g.exe, &g.cmodule, roses_fx::c063::ID, handles, &textures).unwrap().expect("c063");
    oracle.place_role(0, 0x20, &LORD_MOTIONS);
    let mut port = roses_fx::c063::C063::load(&g.exe, &g.cmodule, handles, &textures).expect("c063 port");
    port.place_role(0, 0x20, &LORD_MOTIONS);
    if let Some(s) = rand {
        oracle.set_rand_state(s);
        port.set_rand_state(s);
    }
    let p = run(&mut oracle, &mut port, 1200, 4, |k| Box::new(Finale(k, case)), |_, _, _| {});
    report(&format!("c063 case {case} side {side:#x}"), &p);
    let mem = memory_diff(&mut oracle, &mut port, 0x01E1_0000, 0x6FB0);
    eprintln!("c063 data-area bytes differing at the end: {} (first {:x?})", mem.len(), &mem[..mem.len().min(24)]);
    let obj = memory_diff(&mut oracle, &mut port, 0x01FE_42D0, 0x220);
    let other = memory_diff(&mut oracle, &mut port, 0x01FE_3830, 0x220);
    let units = memory_diff(&mut oracle, &mut port, 0x002D_B990, 3 * 0x3F4);
    eprintln!("c063 object / role-1 / unit bytes differing at the end: {} / {} / {}", obj.len(), other.len(), units.len());
    assert!(obj.is_empty() && other.is_empty() && units.is_empty());
    // As c061: only the rect's unwritten half (+0x9C..+0xA3) may differ.
    assert!(mem.iter().all(|m| (0x01E1_009C..0x01E1_00A4).contains(&m.0)), "{mem:x?}");
    p
}

/// c063 hand port vs the interpreter: every update equal.
#[test]
#[ignore = "requires ROSES_TEST_GAME_DIR (the user's import)"]
fn private_c063_port_equals_the_interpreter() {
    let mut prims = 0;
    for (case, side, rand) in [(0, 0u16, None), (1, 0x100, Some(0x1234_5678_9ABC_DEF1)), (2, 0, Some(7)), (3, 0x100, Some(0xDEAD_BEEF))] {
        let p = c063_case(case, side, rand);
        assert!(p.equal(), "c063 differs in case {case}");
        prims += p.prims;
    }
    eprintln!("c063: {prims} primitives in all cases");
    // Which step functions ran (coverage) in case 0.
    let g = game();
    let textures = finale_textures(&g);
    let mut port = roses_fx::c063::C063::load(&g.exe, &g.cmodule, [0, 0x463], &textures).unwrap();
    port.place_role(0, 0x20, &LORD_MOTIONS);
    let mut steps = std::collections::BTreeMap::new();
    for k in 0..960 {
        let u = port.update(&Finale(k, 0));
        let bus = port.bus().unwrap();
        for t in 0..200u32 {
            let a = 0x01E1_02A0 + 0x70 * t;
            if bus.read_u16(a + 0x58) != 0 {
                *steps.entry(bus.read_u32(a + 0x44)).or_insert(0usize) += 1;
            }
        }
        if u.ret != 0 {
            eprintln!("c063 returned {} at update {k}", u.ret);
            break;
        }
    }
    eprintln!("c063 live task-updates by step function: {steps:x?}");
    let mut oracle = AttackEffect::load_cmodule(&g.exe, &g.cmodule, roses_fx::c063::ID, [0, 0x463], &textures).unwrap().unwrap();
    oracle.place_role(0, 0x20, &LORD_MOTIONS);
    let mut port = roses_fx::c063::C063::load(&g.exe, &g.cmodule, [0, 0x463], &textures).unwrap();
    port.place_role(0, 0x20, &LORD_MOTIONS);
    port.set_rand_state(99);
    let p = run(&mut oracle, &mut port, 1200, 1, |k| Box::new(Finale(k, 0)), |_, _, _| {});
    assert!(!p.equal(), "a different rand state must be seen");
}
