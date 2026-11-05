//! The oracle side of the effect hand ports (`roses-fx`): run a program in
//! the interpreter ([`AttackEffect`]) and its port side by side from the
//! same world, update by update, and list every difference in what the game
//! consumes: the return code (timing), every primitive bit for bit (kind,
//! flags, vertices with f32 bits, ALPHA / TEX0 / TEX1 / CLAMP, texture key,
//! ordering-table key, layer, target, buffer flags), packet layers, frame
//! copies, unit renders (with the pre-callback camera's bits), unit writes
//! (tint, add, scale, mode, fade), obj+0x20D flags, the stage light,
//! missing textures, unimplemented boundary calls, the rand state after the
//! call, the motion a program gives an object, and the end.
//!
//! Not compared: `GsPrim::desc` (the descriptor's address, research only)
//! and `EffectUpdate::steps` (instruction count).
use crate::bmodule::{AttackEffect, EffectUpdate, EffectWorld, GsPrim};
use roses_fx::EffectPort;

/// One update's differences (empty: equal).
#[derive(Clone, Debug, Default)]
pub struct UpdateDiff {
    pub update: usize,
    pub fields: Vec<String>,
}

fn bits4(v: [f32; 4]) -> [u32; 4] {
    v.map(f32::to_bits)
}

/// A primitive as comparable text (f32 as bits; `desc` left out).
pub fn prim_key(p: &GsPrim) -> String {
    let verts: Vec<_> = p.vertices.iter().map(|v| (v.x.to_bits(), v.y.to_bits(), v.z, v.rgba, v.q.to_bits(), v.uv.map(f32::to_bits))).collect();
    format!(
        "kind {} flags {:#x} alpha {:x?} tex0 {:x?} tex1 {:x?} clamp {:x?} texture {:?} buffered {} ot {:?} layer {:?} target {:?} pre {} post_copy {} drawn {} verts {:?}",
        p.kind, p.flags, p.alpha, p.tex0, p.tex1, p.clamp, p.texture, p.buffered, p.ot_key, p.layer, p.target, p.target_pre, p.post_copy, p.drawn_buffer, verts
    )
}

/// Differences between the oracle's and the port's update.
pub fn compare(a: &EffectUpdate, b: &EffectUpdate) -> Vec<String> {
    let mut d = Vec::new();
    if a.ret != b.ret {
        d.push(format!("ret {} vs {}", a.ret, b.ret));
    }
    if a.prims.len() != b.prims.len() {
        d.push(format!("prims {} vs {}", a.prims.len(), b.prims.len()));
    }
    for (i, (p, q)) in a.prims.iter().zip(&b.prims).enumerate() {
        let (x, y) = (prim_key(p), prim_key(q));
        if x != y {
            d.push(format!("prim {i}:\n  oracle {x}\n  port   {y}"));
            if d.len() > 8 {
                break;
            }
        }
    }
    if a.layers != b.layers {
        d.push(format!("layers {:?} vs {:?}", a.layers, b.layers));
    }
    if a.frame_copies != b.frame_copies {
        d.push(format!("frame copies {:?} vs {:?}", a.frame_copies, b.frame_copies));
    }
    let renders = |u: &EffectUpdate| -> Vec<String> {
        u.unit_renders.iter().map(|r| format!("{} {} {:#x} {:?} {:#x}", r.role, r.before, r.pre, r.pre_vp.map(|m| m.map(bits4)), r.extra)).collect()
    };
    if renders(a) != renders(b) {
        d.push(format!("unit renders {:?} vs {:?}", renders(a), renders(b)));
    }
    let writes = |u: &EffectUpdate| -> Vec<String> {
        u.unit_writes.iter().map(|w| format!("{:?} {:?} {:?} {} {:#x}", w.tint, w.add, bits4(w.scale), w.mode, w.fade.to_bits())).collect()
    };
    if writes(a) != writes(b) {
        d.push(format!("unit writes {:?} vs {:?}", writes(a), writes(b)));
    }
    if a.flags_20d != b.flags_20d {
        d.push(format!("obj+0x20D {:?} vs {:?}", a.flags_20d, b.flags_20d));
    }
    if a.stage_light != b.stage_light {
        d.push(format!("stage light {} vs {}", a.stage_light, b.stage_light));
    }
    if a.missing_textures != b.missing_textures {
        d.push(format!("missing textures {:?} vs {:?}", a.missing_textures, b.missing_textures));
    }
    let boundary = |u: &EffectUpdate| u.boundary.iter().map(|x| (x.0, x.2)).collect::<Vec<_>>();
    if boundary(a) != boundary(b) {
        d.push(format!("boundary {:x?} vs {:x?}", boundary(a), boundary(b)));
    }
    d
}

/// The result of a side-by-side run.
#[derive(Clone, Debug, Default)]
pub struct Parity {
    /// Updates run (both sides).
    pub updates: usize,
    /// Updates with differences.
    pub diffs: Vec<UpdateDiff>,
    /// Returns of the oracle by update.
    pub returns: Vec<i32>,
    /// Primitives compared.
    pub prims: usize,
    /// The oracle stopped (interpreter error).
    pub stop: Option<String>,
    /// Time spent in the interpreter and in the port.
    pub oracle_time: std::time::Duration,
    pub port_time: std::time::Duration,
}

impl Parity {
    pub fn equal(&self) -> bool {
        self.diffs.is_empty() && self.stop.is_none()
    }
}

/// Run `oracle` and `port` side by side for at most `max` updates (or to
/// their end), each update with `world(k)`; `before(k, oracle, port)` runs
/// before update `k` (e.g. to set a shared rand state). Stops at the first
/// `stop_after` differing updates.
pub fn run(
    oracle: &mut AttackEffect,
    port: &mut dyn EffectPort,
    max: usize,
    stop_after: usize,
    mut world: impl FnMut(usize) -> Box<dyn EffectWorld>,
    mut before: impl FnMut(usize, &mut AttackEffect, &mut dyn EffectPort),
) -> Parity {
    let mut out = Parity::default();
    for k in 0..max {
        if oracle.finished && port.finished() {
            break;
        }
        before(k, oracle, port);
        let w = world(k);
        let t0 = std::time::Instant::now();
        let a = match oracle.update(w.as_ref()) {
            Ok(u) => u,
            Err(e) => {
                out.stop = Some(e.0);
                break;
            }
        };
        let t1 = std::time::Instant::now();
        let b = port.update(w.as_ref());
        out.oracle_time += t1 - t0;
        out.port_time += t1.elapsed();
        out.updates += 1;
        out.prims += a.prims.len();
        out.returns.push(a.ret);
        let mut fields = compare(&a, &b);
        let (ra, rb) = (oracle.rand_state(), port.rand_state());
        if ra != rb {
            fields.push(format!("rand state {ra:#x} vs {rb:#x}"));
        }
        if oracle.finished != port.finished() || oracle.calls != port.calls() {
            fields.push(format!("end/calls {} {} vs {} {}", oracle.finished, oracle.calls, port.finished(), port.calls()));
        }
        for role in 0..2 {
            if let Some(m) = port.object_motion(role) {
                let o = oracle.object_motion(role);
                if (o.0, o.1.to_bits(), o.2) != (m.0, m.1.to_bits(), m.2) {
                    fields.push(format!("object {role} motion {o:?} vs {m:?}"));
                }
            }
        }
        if !fields.is_empty() {
            out.diffs.push(UpdateDiff { update: k, fields });
            if out.diffs.len() >= stop_after {
                break;
            }
        }
    }
    out
}

/// Bytes of `[start, start + len)` that differ between the oracle's memory
/// and the port's (address, oracle, port); empty when the port has no
/// memory picture.
pub fn memory_diff(oracle: &mut AttackEffect, port: &mut dyn EffectPort, start: u32, len: u32) -> Vec<(u32, u8, u8)> {
    let Some(bus) = port.bus() else { return Vec::new() };
    (start..start + len).filter_map(|a| {
        let (x, y) = (oracle.cpu.read_u8(a), bus.read_u8(a));
        (x != y).then_some((a, x, y))
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bmodule::{GsPrim, GsVertex};

    /// One flipped bit of one vertex coordinate, a key or a return is a
    /// difference.
    #[test]
    fn compare_sees_single_bit_changes() {
        let v = GsVertex { x: 10.0, y: 20.0, z: 3, rgba: [1, 2, 3, 4], q: 1.0, uv: [0.5, 0.25] };
        let p = GsPrim { kind: 4, vertices: vec![v; 4], ot_key: Some(7), ..Default::default() };
        let a = EffectUpdate { ret: 0, prims: vec![p.clone()], ..Default::default() };
        assert!(compare(&a, &a.clone()).is_empty());
        let mut b = a.clone();
        b.prims[0].vertices[2].uv[1] = f32::from_bits(0.25f32.to_bits() + 1);
        assert_eq!(compare(&a, &b).len(), 1);
        let mut b = a.clone();
        b.prims[0].ot_key = Some(8);
        assert_eq!(compare(&a, &b).len(), 1);
        let mut b = a.clone();
        b.ret = 2;
        assert_eq!(compare(&a, &b).len(), 1);
        let mut b = a.clone();
        b.prims[0].vertices[0].x = -0.0f32.max(10.0) + f32::EPSILON * 8.0;
        assert_eq!(compare(&a, &b).len(), 1);
    }
}

/// Private checks against the user's import (`ROSES_TEST_GAME_DIR`: a game
/// import with `duel-arena/{SLES_515.79, CMODULE.MRG, BMODULE.MRG}` and
/// `duel/TEXEFF.MRG`).
#[cfg(test)]
mod private_tests {
    use super::*;
    use crate::bmodule::{CameraMirror, EffectTextures, UnitMirror};

    pub(super) struct Game {
        pub exe: Vec<u8>,
        pub cmodule: Vec<u8>,
        pub bmodule: Vec<u8>,
        pub texeff: Vec<Option<Vec<u8>>>,
    }

    pub(super) fn game() -> Game {
        let dir = std::path::PathBuf::from(std::env::var_os("ROSES_TEST_GAME_DIR").expect("ROSES_TEST_GAME_DIR"));
        let exe = std::fs::read(dir.join("duel-arena/SLES_515.79")).unwrap();
        let cmodule = std::fs::read(dir.join("duel-arena/CMODULE.MRG")).unwrap();
        let bmodule = std::fs::read(dir.join("duel-arena/BMODULE.MRG")).unwrap();
        let tex = std::fs::read(dir.join("duel/TEXEFF.MRG")).unwrap();
        let texeff = match roses_assets::inspect_pal_opening_archive(&tex, roses_assets::OpeningArchiveKind::TexEff) {
            Ok(a) => a.records.iter().map(|r| tex.get(r.file_offset as usize..r.file_offset as usize + r.byte_len as usize).map(<[u8]>::to_vec)).collect(),
            Err(_) => Vec::new(),
        };
        Game { exe, cmodule, bmodule, texeff }
    }

    /// The board as c620 sees it: a camera, empty units.
    pub(super) struct Board(pub CameraMirror);
    impl EffectWorld for Board {
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

    /// The opening camera (PCSX2 u123) and a slow orbit round it, so the
    /// effect is seen from changing angles.
    pub(super) fn camera_at(k: usize, orbit: bool) -> CameraMirror {
        let (eye, target) = ([0.0f32, 95.557, 66.853], [0.0f32, 40.0, 150.0]);
        if !orbit {
            return CameraMirror { eye, target, distance: 800.0, roll: 0.0 };
        }
        let a = k as f32 * 0.02;
        let d = [eye[0] - target[0], eye[2] - target[2]];
        let e = [target[0] + d[0] * a.cos() - d[1] * a.sin(), eye[1] + 10.0 * (k as f32 * 0.05).sin(), target[2] + d[0] * a.sin() + d[1] * a.cos()];
        CameraMirror { eye: e, target, distance: 800.0 - k as f32, roll: 0.0 }
    }

    fn c620_case(cell: (u8, u8), orbit: bool, rand: Option<u64>) -> Parity {
        let g = game();
        let textures = EffectTextures { texeff: g.texeff.clone(), own: Vec::new(), other: Vec::new() };
        let target = (u16::from(cell.1) << 8) | u16::from(cell.0);
        let mut oracle = AttackEffect::load_cmodule(&g.exe, &g.cmodule, roses_fx::c620::ID, [0, target], &textures).unwrap().expect("c620");
        let mut port = roses_fx::c620::C620::load(&g.exe, &g.cmodule, [0, target], &textures).expect("c620 port");
        if let Some(s) = rand {
            oracle.set_rand_state(s);
            port.set_rand_state(s);
        }
        let p = run(&mut oracle, &mut port, 200, 4, |k| Box::new(Board(camera_at(k, orbit))), |_, _, _| {});
        for d in &p.diffs {
            eprintln!("update {}:", d.update);
            for f in &d.fields {
                eprintln!("  {f}");
            }
        }
        if let Some(d) = p.diffs.first() {
            let mem = memory_diff(&mut oracle, &mut port, 0x01E1_0000, 0x3740);
            eprintln!("data-area bytes differing after update {}: {} (first {:x?})", d.update, mem.len(), &mem[..mem.len().min(16)]);
        }
        eprintln!("c620 cell {cell:?} orbit {orbit}: {} updates, {} primitives, returns 2 at {:?}, 8 at {:?}, {} differing updates; interpreter {:?}, port {:?}", p.updates, p.prims, p.returns.iter().position(|&r| r == 2), p.returns.iter().position(|&r| r == 8), p.diffs.len(), p.oracle_time, p.port_time);
        p
    }

    /// c620 hand port vs the interpreter: every update equal [run].
    #[test]
    #[ignore = "requires ROSES_TEST_GAME_DIR (the user's import)"]
    fn private_c620_port_equals_the_interpreter() {
        for (cell, orbit, rand) in [((3, 6), false, None), ((3, 0), true, Some(0x1234_5678_9ABC_DEF1)), ((0, 3), true, Some(7)), ((6, 6), false, Some(0xDEAD_BEEF))] {
            let p = c620_case(cell, orbit, rand);
            assert!(p.equal(), "c620 differs at cell {cell:?}");
            assert_eq!(p.returns.len(), 73);
        }
        // The harness sees a divergence: the port with another rand state
        // differs (the sparkles' and the quads' random angles).
        let g = game();
        let textures = EffectTextures { texeff: g.texeff.clone(), own: Vec::new(), other: Vec::new() };
        let target = (6u16 << 8) | 3;
        let mut oracle = AttackEffect::load_cmodule(&g.exe, &g.cmodule, roses_fx::c620::ID, [0, target], &textures).unwrap().unwrap();
        let mut port = roses_fx::c620::C620::load(&g.exe, &g.cmodule, [0, target], &textures).unwrap();
        port.set_rand_state(99);
        let p = run(&mut oracle, &mut port, 200, 1, |k| Box::new(Board(camera_at(k, false))), |_, _, _| {});
        assert!(!p.equal(), "a different rand state must be seen");
    }
}

/// Private checks of the ported BMODULE programs.
#[cfg(test)]
mod private_bmodule_tests {
    use super::private_tests::game;
    use super::*;
    use crate::bmodule::{CameraMirror, EffectTextures, UnitMirror};

    /// A battle as an attack effect sees it: the attacker (role 0) facing
    /// its target 100 units away, both playing motion 1 / 4 forward, the
    /// arena camera turning slowly round them; skeleton nodes at fixed
    /// offsets from the animated positions.
    struct Battle(usize);
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

    /// Every ported BMODULE program equals the interpreter's run, both as
    /// the attacker's effect and its target's.
    #[test]
    #[ignore = "requires ROSES_TEST_GAME_DIR (the user's import)"]
    fn private_ported_bmodules_equal_the_interpreter() {
        let g = game();
        let image = std::sync::Arc::new(roses_fx::bus::Image::from_elf(&g.exe).unwrap());
        let textures = EffectTextures { texeff: g.texeff.clone(), own: Vec::new(), other: Vec::new() };
        for &(card, name, _) in roses_fx::bmodule::PORTED {
            for handles in [[0x101u16, 0x001], [0x001, 0x101]] {
                let mut oracle = AttackEffect::load(&g.exe, &g.bmodule, card, handles, &textures).unwrap().expect("record");
                let mut port = roses_fx::bmodule::BPort::load(image.clone(), &g.bmodule, card, handles, &textures).expect("port");
                let p = run(&mut oracle, &mut port, 4000, 4, |k| Box::new(Battle(k)), |k, o, q| {
                    // The game hands every module its one rand state.
                    let s = 0x9E37_79B9_7F4A_7C15u64.wrapping_mul(k as u64 + 1);
                    o.set_rand_state(s);
                    q.set_rand_state(s);
                });
                for d in &p.diffs {
                    eprintln!("{name} update {}:", d.update);
                    for f in &d.fields {
                        eprintln!("  {f}");
                    }
                }
                eprintln!("{name} (card {card}) handles {handles:x?}: {} updates, {} primitives, returns {:?}, {} differing; interpreter {:?}, port {:?}", p.updates, p.prims, &p.returns[..p.returns.len().min(12)], p.diffs.len(), p.oracle_time, p.port_time);
                if card == 631 {
                    // The chain's start (the attacker's node 0x26) came from the
                    // mirrored skeleton [run].
                    let v: Vec<f32> = (0..3).map(|k| oracle.cpu.read_f32(0x01ED_0140 + 4 * k)).collect();
                    eprintln!("b262 node 0x26 at the end: {v:?}");
                    assert!(v.iter().any(|x| *x != 0.0));
                }
                assert!(p.equal(), "{name} differs");
            }
        }
    }
}

/// Private checks of the ported BMODULE programs over varied battles
/// (helper C, 30 Oct 2026): three cameras (slow orbit, fast low orbit from
/// the other side, a fixed far camera), two unit spacings, three rand
/// schedules (re-seeded every update with two different sequences, or
/// seeded once), both handle orders. Helper Y1 (3 Nov 2026) added three
/// facing cases (6..8): the attacker facing its target square on (yaw 0,
/// target 0x8000), the units on the other side of the board (attacker at
/// +z with yaw 0x8000, target at -z with yaw 0) and an odd negative facing
/// (attacker -0x2A00, target 0x6000) - the first six all have the attacker
/// at 0x400, so facing-dependent branches (card 1's burst) never ran.
/// `ROSES_PARITY_CARDS=375,119` limits the run to those cards;
/// `ROSES_PARITY_THREADS=n` runs n cards at a time (default 4); every card
/// is run and the differing ones are listed before the test fails.
#[cfg(test)]
mod private_bmodule_varied_tests {
    use super::private_tests::game;
    use super::*;
    use crate::bmodule::{CameraMirror, EffectTextures, UnitMirror};
    use std::fmt::Write as _;
    use crate::bmodule::{BattleState, BattleUnit};

    #[derive(Clone, Copy)]
    struct Scene {
        k: usize,
        camera: u8,
        spacing: f32,
        /// The attacker's and the target's yaw (obj+0x14).
        yaw: [i32; 2],
        /// The attacker stands at -side x spacing in z (1: -z, -1: +z).
        side: f32,
        /// The outcome cases' battle.
        battle: Option<BattleState>,
    }
    impl EffectWorld for Scene {
        fn battle(&self) -> Option<BattleState> {
            self.battle
        }
        fn camera(&self) -> CameraMirror {
            let t = self.k as f32;
            match self.camera {
                0 => {
                    let a = t * 0.01;
                    CameraMirror { eye: [120.0 * a.cos(), 60.0, -120.0 * a.sin()], target: [0.0, 15.0, 0.0], distance: 675.0, roll: 0.0 }
                }
                1 => {
                    let a = 2.5 - t * 0.035;
                    CameraMirror { eye: [-90.0 * a.cos(), 25.0 + 10.0 * (t * 0.05).sin(), 140.0 * a.sin()], target: [5.0, 20.0, -10.0], distance: 800.0 - t * 0.5, roll: 0.0 }
                }
                _ => CameraMirror { eye: [30.0, 140.0, -260.0], target: [0.0, 10.0, 20.0], distance: 560.0, roll: 0.0 },
            }
        }
        fn unit(&self, role: usize) -> UnitMirror {
            let z = if role == 0 { -self.spacing * self.side } else { self.spacing * self.side };
            let t = self.k as f32;
            UnitMirror {
                position: [if role == 0 { 0.0 } else { 12.0 }, 0.0, z],
                rotation: [0, self.yaw[role.min(1)], 0],
                animated: [(t * 0.07).cos() * 3.0, (t * 0.1).sin() * 2.0 + 1.0, z * (1.0 - (t * 0.004).min(0.5))],
                motion: if role == 0 { 1 } else { 4 },
                time: t % 37.0,
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
            Some([u.animated[0] + (n * 0.7).sin() * 6.0, u.animated[1] + n * 1.5 + 4.0, u.animated[2] + (n * 0.3).cos() * 4.0])
        }
    }

    /// (camera, spacing, rand schedule, [attacker yaw, target yaw], side).
    const CASES: [(u8, f32, u8, [i32; 2], f32); 9] = [
        (0, 50.0, 0, [0x400, 0x8400], 1.0),
        (1, 50.0, 1, [0x400, 0x8400], 1.0),
        (2, 80.0, 2, [0x400, 0x8400], 1.0),
        (1, 30.0, 0, [0x400, 0x8400], 1.0),
        (2, 50.0, 1, [0x400, 0x8400], 1.0),
        (0, 80.0, 2, [0x400, 0x8400], 1.0),
        // Facing cases (Y1).
        (1, 50.0, 0, [0, 0x8000], 1.0),
        (0, 50.0, 1, [0x8000, 0], -1.0),
        (2, 30.0, 2, [-0x2A00, 0x6000], 1.0),
    ];

    /// Helper D30 (3 Nov 2026): the records that call the outcome boundary
    /// 0x1F8100 run five more cases with a mirrored battle (cases 9..13:
    /// destroyed / survives / defence holds / tie on the active side / a
    /// counter-effect tie), so both answers and their unit writes are
    /// compared; every other card runs cases 0..8 only.
    pub(super) const OUTCOME_CARDS: [u16; 9] = [2, 16, 17, 88, 226, 339, 400, 614, 664];
    const OUTCOME_CASES: [(u8, f32, u8, [i32; 2], f32); 5] = [
        (0, 50.0, 0, [0x400, 0x8400], 1.0),
        (1, 50.0, 1, [0x400, 0x8400], 1.0),
        (2, 80.0, 2, [0x400, 0x8400], 1.0),
        (1, 30.0, 0, [0, 0x8000], 1.0),
        (0, 50.0, 1, [0x8000, 0], -1.0),
    ];

    /// The battle of outcome case `ci` for the effect's handles [own,
    /// target]: 9 own 2400 beats 1800 (answer 1), 10 own 1200 loses to 1900
    /// (0), 11 the target in defence 2000 holds against 1500 (0), 12 a 1700
    /// tie with own on the active side (0), 13 the counter effect (roles
    /// [target, own], flag 1) tie 1300 (1).
    fn outcome_battle(ci: usize, [own, target]: [u16; 2]) -> Option<BattleState> {
        let u = |attack: i32, defence: i32, defence_position: bool| Some(BattleUnit { attack, defence, defence_position });
        let side = |h: u16| (h >> 8) as u8;
        let (roles, units) = match ci {
            9 => ([own, target], [u(2400, 1000, false), u(1800, 2100, false)]),
            10 => ([own, target], [u(1200, 900, false), u(1900, 1500, false)]),
            11 => ([own, target], [u(1500, 1200, false), u(2600, 2000, true)]),
            12 => ([own, target], [u(1700, 1100, false), u(1700, 1300, false)]),
            13 => ([target, own], [u(1300, 800, false), u(1300, 1600, false)]),
            _ => return None,
        };
        Some(BattleState { roles, active_side: side(roles[0]), cell: (3, 2), units })
    }

    struct CardResult {
        runs: usize,
        updates: usize,
        prims: usize,
        /// "case N update U: first field" for every differing case.
        fails: Vec<String>,
        log: String,
    }

    fn check_card(g: &super::private_tests::Game, image: &std::sync::Arc<roses_fx::bus::Image>, base: &EffectTextures, card: u16, name: &str) -> CardResult {
        let mut r = CardResult { runs: 0, updates: 0, prims: 0, fails: Vec::new(), log: String::new() };
        let watch = std::env::var("ROSES_PARITY_WATCH").ok().and_then(|w| u32::from_str_radix(w.trim_start_matches("0x"), 16).ok());
        let stack = std::env::var_os("ROSES_PARITY_STACK").is_some();
        let outcome = OUTCOME_CARDS.contains(&card);
        for (ci, &(camera, spacing, rand, yaw, side)) in CASES.iter().chain(OUTCOME_CASES.iter().filter(|_| outcome)).enumerate() {
            let handles = if ci % 2 == 0 { [0x101u16, 0x001] } else { [0x003, 0x104] };
            let battle = outcome_battle(ci, handles);
            // Cases 3..5 and 7 give the attacker effect textures (type 5):
            // TEXEFF records stand in for its own RH2s.
            let own = (3..6).contains(&ci) || ci == 7;
            let textures = if own { EffectTextures { own: g.texeff.iter().skip(1).take(8).cloned().collect(), ..base.clone() } } else { base.clone() };
            assert!(!own || (textures.own.len() == 8 && textures.own.iter().all(Option::is_some)));
            let mut oracle = AttackEffect::load(&g.exe, &g.bmodule, card, handles, &textures).unwrap().expect("record");
            let mut port = roses_fx::bmodule::BPort::load(image.clone(), &g.bmodule, card, handles, &textures).expect("port");
            let mut wlog = String::new();
            let p = run(&mut oracle, &mut port, 4000, 4, |k| Box::new(Scene { k, camera, spacing, yaw, side, battle }), |k, o, q| {
                if let Some(a) = watch {
                    let pv = q.bus().map(|b| b.read_u32(a));
                    let _ = writeln!(wlog, "watch {a:#x} before update {k}: oracle {:#x} port {:x?}", o.cpu.read_u32(a), pv);
                }
                let s = match rand {
                    0 => Some(0x9E37_79B9_7F4A_7C15u64.wrapping_mul(k as u64 + 1)),
                    1 => Some(0xD1B5_4A32_D192_ED03u64.wrapping_mul(k as u64 + 7) ^ 0x5555),
                    _ => (k == 0).then_some(0x0123_4567_89AB_CDEF),
                };
                if let Some(s) = s {
                    o.set_rand_state(s);
                    q.set_rand_state(s);
                }
            });
            r.log.push_str(&wlog);
            for d in &p.diffs {
                let _ = writeln!(r.log, "{name} case {ci} update {}:", d.update);
                for f in &d.fields {
                    let _ = writeln!(r.log, "  {f}");
                }
            }
            if let Some(d) = p.diffs.first() {
                let mem = memory_diff(&mut oracle, &mut port, 0x01ED_0000, 0x8000);
                let _ = writeln!(r.log, "data-area bytes differing after update {}: {} (first {:x?})", d.update, mem.len(), &mem[..mem.len().min(16)]);
                if stack {
                    let st = memory_diff(&mut oracle, &mut port, 0x01EF_F000, 0x1000);
                    let _ = writeln!(r.log, "stack bytes differing: {} {:x?}", st.len(), &st[..st.len().min(400)]);
                }
            }
            let _ = writeln!(r.log, "{name} (card {card}) case {ci} camera {camera} spacing {spacing} rand {rand} yaw {yaw:x?} side {side}: {} updates, {} primitives, returns {:?}, {} differing; interpreter {:?}, port {:?}", p.updates, p.prims, &p.returns[..p.returns.len().min(8)], p.diffs.len(), p.oracle_time, p.port_time);
            r.runs += 1;
            r.updates += p.updates;
            r.prims += p.prims;
            if !p.equal() {
                let first = p.diffs.first().map_or_else(|| format!("stop {:?}", p.stop), |d| format!("update {}: {}", d.update, d.fields.first().map_or("", |f| f.lines().next().unwrap_or(""))));
                r.fails.push(format!("case {ci} {first}"));
                continue;
            }
            // The data area ends byte for byte the same too.
            let mem = memory_diff(&mut oracle, &mut port, 0x01ED_0000, 0x8000);
            if !mem.is_empty() {
                r.fails.push(format!("case {ci}: {} data-area bytes differ at the end (first {:x?})", mem.len(), &mem[..mem.len().min(16)]));
            }
        }
        r
    }

    #[test]
    #[ignore = "requires ROSES_TEST_GAME_DIR (the user's import)"]
    fn private_ported_bmodules_equal_the_interpreter_in_varied_battles() {
        let g = game();
        let image = std::sync::Arc::new(roses_fx::bus::Image::from_elf(&g.exe).unwrap());
        let textures = EffectTextures { texeff: g.texeff.clone(), own: Vec::new(), other: Vec::new() };
        let only: Option<Vec<u16>> = std::env::var("ROSES_PARITY_CARDS").ok().map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect());
        let threads = std::env::var("ROSES_PARITY_THREADS").ok().and_then(|s| s.trim().parse().ok()).unwrap_or(4usize).max(1);
        let cards: Vec<(u16, &str)> = roses_fx::bmodule::PORTED
            .iter()
            .filter(|(card, _, _)| only.as_ref().is_none_or(|o| o.contains(card)))
            .map(|&(card, name, _)| (card, name))
            .collect();
        let next = std::sync::atomic::AtomicUsize::new(0);
        let total = std::sync::Mutex::new((0usize, 0usize, 0usize));
        let fails = std::sync::Mutex::new(Vec::<String>::new());
        std::thread::scope(|s| {
            for _ in 0..threads.min(cards.len().max(1)) {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(&(card, name)) = cards.get(i) else { break };
                    let r = check_card(&g, &image, &textures, card, name);
                    eprint!("{}", r.log);
                    {
                        let mut t = total.lock().unwrap();
                        t.0 += r.runs;
                        t.1 += r.updates;
                        t.2 += r.prims;
                    }
                    for f in r.fails {
                        eprintln!("DIFFERS {name} (card {card}) {f}");
                        fails.lock().unwrap().push(format!("{name} (card {card}) {f}"));
                    }
                });
            }
        });
        let total = total.into_inner().unwrap();
        let mut fails = fails.into_inner().unwrap();
        fails.sort();
        for f in &fails {
            eprintln!("differs: {f}");
        }
        eprintln!("varied battles: {} runs, {} updates, {} primitives, {} differences ({} cards, {} threads)", total.0, total.1, total.2, fails.len(), cards.len(), threads);
        assert!(fails.is_empty(), "{} runs differ", fails.len());
    }
}
