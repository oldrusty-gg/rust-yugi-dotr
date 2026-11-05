//! Bevy-free glue between the arena battle (`duel_arena_battle`) and the
//! hand-ported attack effects `roses_fx::bmodule` (research:
//! `duel/the project notes):
//! - [`BattleWorld`] mirrors the battle into the runner: the arena camera,
//!   the two unit objects and their skeleton node results (node origin under
//!   the object matrix at the current motion time, arena objects not
//!   recentred) [emu: footage RAM 032 / 044 / 060].
//! - [`EffectWorker`] owns a copy of the battle on a worker thread and steps
//!   it one update ahead of the real one, running the attack effects there
//!   live; per update it hands over the effects' returns (which drive the
//!   real battle) and their captured GS primitives (drawn by the renderer).
//!   A skip press reaches the copy one update late [H: the effect's camera
//!   lags one update during the exit].
use crate::duel_arena_battle::{ArenaBattle, FxShared, Unit};
use roses_client::monster_pose;
use roses_fx::types::{BattleState, BattleUnit, CameraMirror, EffectTextures, EffectWorld, GsPrim, TextureKey, UnitMirror, UnitWrites};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long the real battle waits for the worker's update before it goes
/// on with the cached returns (nothing drawn that update).
pub const WAIT: Duration = Duration::from_millis(12);
/// [`WAIT`], or `ROSES_BMODULE_WAIT_MS` (captures wait for every update).
pub fn wait() -> Duration {
    static WAIT_MS: std::sync::OnceLock<Option<u64>> = std::sync::OnceLock::new();
    WAIT_MS
        .get_or_init(|| std::env::var("ROSES_BMODULE_WAIT_MS").ok().and_then(|v| v.parse().ok()))
        .map_or(WAIT, Duration::from_millis)
}

/// Research: `ROSES_ARENA_FORCE=attacker,defender,destroyed` (cards and the
/// destroyed bits: 1 defender, 2 attacker) for the next arena battles.
pub fn forced_battle() -> Option<(u16, u16, u8)> {
    let v = std::env::var("ROSES_ARENA_FORCE").ok()?;
    let p: Vec<u16> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
    (p.len() == 3).then(|| (p[0], p[1], p[2] as u8))
}

/// What a port's panic is reported with (in `WorkerStats::stops` too).
const PANICKED: &str = "port panicked";

/// The net under port code. The ports have branches that `panic!` (never
/// ported or never reached [F]) and no test proves them unreachable [H];
/// the battle has to go on all the same. So a panic in `f` is logged and
/// comes back as an `Err`: the caller stops
/// the effect and the game goes on. Nothing of the panicked port is used
/// again, so a half-done update is not seen.
pub fn caught<T>(what: impl std::fmt::Display, f: impl FnOnce() -> T) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).map_err(|p| {
        let cause = p.downcast_ref::<&str>().map(|s| (*s).to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_else(|| "no message".into());
        eprintln!("{what}: {PANICKED}, the effect stops, the game goes on: {cause}");
        format!("{PANICKED}: {cause}")
    })
}

/// [`caught`] for port code on the caller's thread; in this crate's tests
/// the panic goes on up, so a test (and the sweep's `panicked` rows) fails
/// on it. The worker uses [`caught`]: a panic there fails the test in
/// [`EffectWorker::tick`].
pub fn guarded<T>(what: impl std::fmt::Display, f: impl FnOnce() -> T) -> Result<T, String> {
    let result = caught(what, f);
    #[cfg(test)]
    if let Err(e) = &result {
        panic!("{e}");
    }
    result
}

/// Updates the worker may run ahead (absorbs the heaviest calls, B220's
/// ~13 ms at its end).
pub const LOOKAHEAD: usize = 8;

/// Every skeleton node's world origin of `unit` at its current time (see
/// [`node_world`]).
pub fn node_origins(unit: &Unit) -> Option<Vec<[f32; 3]>> {
    let model = &unit.data.model.model;
    let sine = &unit.data.model.sine;
    let world = monster_pose::object_matrix(model, &unit.object, sine);
    let blend = unit.blend.and_then(|b| {
        model.motion(b.previous.motion).map(|previous| monster_pose::PoseBlend {
            previous,
            time: b.previous.time,
            weight: b.weight(),
        })
    });
    let pose = monster_pose::evaluate_pose_blended(model, model.motion(unit.clock.motion), unit.clock.time, blend, sine).ok()?;
    Some(
        pose.nodes
            .iter()
            .map(|n| {
                let p = monster_pose::apply(&monster_pose::mul(&world, n), [0.0, 0.0, 0.0, 1.0]);
                [p[0] as f32, p[1] as f32, p[2] as f32]
            })
            .collect(),
    )
}

/// World position of skeleton node `node` of `unit` (the model renderer's
/// node result obj+0xA0 + 16 * slot): the node matrix's origin under the
/// unit's object matrix, at the unit's current motion time.
pub fn node_world(unit: &Unit, node: u8) -> Option<[f32; 3]> {
    let model = &unit.data.model.model;
    let sine = &unit.data.model.sine;
    let world = monster_pose::object_matrix(model, &unit.object, sine);
    let blend = unit.blend.and_then(|b| {
        model.motion(b.previous.motion).map(|previous| monster_pose::PoseBlend {
            previous,
            time: b.previous.time,
            weight: b.weight(),
        })
    });
    let pose = monster_pose::evaluate_pose_blended(model, model.motion(unit.clock.motion), unit.clock.time, blend, sine).ok()?;
    let n = pose.nodes.get(usize::from(node))?;
    let p = monster_pose::apply(&monster_pose::mul(&world, n), [0.0, 0.0, 0.0, 1.0]);
    Some([p[0] as f32, p[1] as f32, p[2] as f32])
}

/// RGBA of an indexed GS texture with alpha = GS alpha * 255 / 128 (the
/// FX shader reads it back as alpha * 128): C_16 palettes give 128 with
/// the alpha bit, 32-bit palettes their own alpha byte.
pub fn indexed_rgba(t: &roses_assets::OpeningIndexedTexture) -> Option<roses_assets::RgbaImage> {
    let mut pixels = Vec::with_capacity(t.palette_indices.len() * 4);
    for i in &t.palette_indices {
        let v = *t.palette_words_raw.get(usize::from(*i))?;
        let (rgb, a) = match t.palette_format {
            roses_assets::OpeningPaletteFormat::Rgb5551 => (
                [((v & 31) << 3) as u8, (((v >> 5) & 31) << 3) as u8, (((v >> 10) & 31) << 3) as u8],
                if v & 0x8000 != 0 { 128u32 } else { 0 },
            ),
            roses_assets::OpeningPaletteFormat::Rgba8888Alpha128 => ([v as u8, (v >> 8) as u8, (v >> 16) as u8], v >> 24),
        };
        pixels.extend([rgb[0], rgb[1], rgb[2], (a * 255 / 128).min(255) as u8]);
    }
    Some(roses_assets::RgbaImage { width: t.width, height: t.height, pixels })
}

/// TEXEFF.MRG's RH2 records by id (the effects' type-4 textures).
pub fn texeff_records(texeff: &[u8]) -> Vec<Option<Vec<u8>>> {
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

/// The battle as an effect of `attacker`'s attack sees it (runner role 0 =
/// the attacker, 1 = its target).
pub struct BattleWorld<'a> {
    pub battle: &'a ArenaBattle,
    pub attacker: usize,
    /// Monster model index (obj+0x206) of (attacker, target).
    pub models: [u16; 2],
    /// Node origins by role, evaluated once per update.
    pub nodes: [std::cell::OnceCell<Option<Vec<[f32; 3]>>>; 2],
    /// What the outcome boundary 0x1F8100 reads; `None`: not mirrored.
    pub state: Option<BattleState>,
}

impl<'a> BattleWorld<'a> {
    pub fn new(battle: &'a ArenaBattle, attacker: usize, models: [u16; 2]) -> Self {
        Self { battle, attacker, models, nodes: Default::default(), state: None }
    }
    fn unit_of(&self, role: usize) -> &Unit {
        &self.battle.units[if role == 0 { self.attacker } else { 1 - self.attacker }]
    }
    fn origins(&self, role: usize) -> Option<&Vec<[f32; 3]>> {
        self.nodes[role].get_or_init(|| node_origins(self.unit_of(role))).as_ref()
    }
}

impl EffectWorld for BattleWorld<'_> {
    fn camera(&self) -> CameraMirror {
        let c = &self.battle.camera;
        CameraMirror {
            eye: c.eye.map(|v| v as f32),
            target: c.target.map(|v| v as f32),
            distance: c.distance as f32,
            roll: c.roll as f32,
        }
    }
    fn unit(&self, role: usize) -> UnitMirror {
        let u = self.unit_of(role);
        let model = &u.data.model.model;
        UnitMirror {
            position: u.object.position,
            rotation: u.object.rotation,
            // obj+0x80 = node 1's origin (node 0's without one) [emu].
            animated: self.origins(role).and_then(|n| n.get(1).or(n.first()).copied()).unwrap_or(u.object.position),
            motion: u.clock.motion,
            time: u.clock.time,
            speed: u.clock.speed,
            play_mode: u.clock.direction,
            frozen: u.frozen,
            frames: std::array::from_fn(|m| model.motion(m as u8).map_or(0, |m| m.length)),
            alpha: u.alpha,
            nodes: model.nodes.len().min(255) as u8,
            model: self.models[role],
            bounds: model.nodes.first().map_or([0.0; 4], |n| n.bounding_sphere),
        }
    }
    fn node(&self, role: usize, node: u8) -> Option<[f32; 3]> {
        self.origins(role)?.get(usize::from(node)).copied()
    }
    fn battle(&self) -> Option<BattleState> {
        self.state
    }
    /// One counter for the battle's modules, one count per update, as the
    /// game's [F 0x209120 increments 0x2E963C per flip]; its origin is the
    /// footage battle's (update 90 = frame 95641, B125 at 95965: the
    /// launches on odd counters) [emu `emu_rand.py`; H: a real game's origin
    /// is its whole history's].
    fn frame_counter(&self) -> Option<u32> {
        Some(self.battle.updates.wrapping_add(FRAME_COUNTER_ORIGIN))
    }
}

/// The nine records that call the outcome boundary 0x1F8100 [F bscan.json;
/// OUTCOME-BOUNDARY.md 1]. Only they get the battle mirrored: every other
/// record's varied parity ran without it, so they stay as proven.
pub const OUTCOME_CARDS: [u16; 9] = [2, 16, 17, 88, 226, 339, 400, 614, 664];

/// The handle words the modules are loaded with, by role (0 the attacker).
pub const ROLE_HANDLES: [u16; 2] = [0x101, 0x001];

/// A real battle as [`BattleState`] on [`ROLE_HANDLES`]: `units` = (attacker,
/// defender) with their stats at the defender's `cell`; `attacker_active`:
/// the attacker's side is `Duel::active` (0x2DB8F4).
pub fn battle_state(units: [BattleUnit; 2], attacker_active: bool, cell: (u8, u8)) -> BattleState {
    let active = if attacker_active { ROLE_HANDLES[0] } else { ROLE_HANDLES[1] };
    BattleState { roles: ROLE_HANDLES, active_side: (active >> 8) as u8, cell, units: units.map(Some) }
}

/// The footage battle's frame counter at update 0 (95641 - 90).
pub const FRAME_COUNTER_ORIGIN: u32 = 95_551;

/// What the worker needs to run the effects: (BMODULE.MRG, executable),
/// TEXEFF's records and each role's own effect RH2s.
#[derive(Clone)]
pub struct EffectInputs {
    pub program: Arc<(Vec<u8>, Vec<u8>)>,
    pub texeff: Vec<Option<Vec<u8>>>,
    pub own: [Vec<Option<Vec<u8>>>; 2],
    /// The same decoded for drawing (alpha = GS alpha * 255 / 128).
    pub own_images: [Arc<Vec<Option<roses_assets::RgbaImage>>>; 2],
    /// The duel's battle behind this scene; `None` without one: the outcome
    /// boundary then answers 1, the zero image's answer.
    pub battle: Option<BattleState>,
}

/// One module call made by the worker's battle.
#[derive(Clone, Debug)]
pub struct CallResult {
    pub role: usize,
    pub call: usize,
    /// `None`: the program is missing or stopped (the cached return is used).
    pub ret: Option<i32>,
    /// In GS order (`roses_fx::types::draw_order`).
    pub prims: Vec<GsPrim>,
    pub boundary: Vec<(u32, &'static str, [u32; 4])>,
    pub missing_textures: Vec<(i32, i32)>,
    /// Roles drawn into the offscreen target (0x1F0FD0) this update.
    pub unit_rt: [bool; 2],
    /// The camera a pre-model callback drew that role into its target with
    /// (`UnitRender::pre_vp`).
    pub rt_vp: [Option<[[f32; 4]; 4]>; 2],
    /// The unit object fields as the module left them.
    pub unit_writes: [UnitWrites; 2],
    /// The stage light 0x2DF5B2 after the call.
    pub stage_light: u8,
    /// The game's rand state and the units' freeze bits after the call.
    pub shared: Option<FxShared>,
    pub steps: u64,
    pub micros: u64,
}

/// The module calls of one battle update.
#[derive(Clone, Debug, Default)]
pub struct TickResult {
    pub tick: u32,
    /// Bumped by each skip press (older results are stale).
    pub generation: u32,
    pub calls: Vec<CallResult>,
}

/// The worker's state before one of its updates, kept for a rewind (the
/// skip press reaches it after it ran ahead).
struct Checkpoint {
    tick: u32,
    battle: ArenaBattle,
    present: [bool; 2],
    dead: [bool; 2],
    /// A copy of each running effect.
    fx: [Option<Effect>; 2],
}

/// One role's running attack effect: the hand port (`roses_fx::bmodule`,
/// equal to the original update for update: `roses_ee::parity`).
type Effect = Box<roses_fx::bmodule::BPort>;

/// Updates the worker can rewind (it runs at most `LOOKAHEAD` + 1 ahead).
const CHECKPOINTS: usize = LOOKAHEAD + 4;

/// What the real battle draws for the attack effect this update.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EffectDraw {
    /// The attacker's card (its effect slots give the type-5 textures).
    pub card: u16,
    pub prims: Arc<Vec<GsPrim>>,
    /// The attacker's effect textures by slot (type 5).
    pub own: Arc<Vec<Option<roses_assets::RgbaImage>>>,
    /// Units drawn into the offscreen target (TBP 0x2000) for the
    /// effect's `TextureKey::UnitRt` sprites (afterimages).
    pub unit_rt: [bool; 2],
    /// The camera of a role's render into its target when a pre-model
    /// callback switched it.
    pub rt_vp: [Option<[[f32; 4]; 4]>; 2],
    /// obj+0x12C tint, +0x130 colour, +0x30 scale of (attacker, defender)
    /// after the module's update.
    pub unit_writes: [UnitWrites; 2],
    /// The stage light 0x2DF5B2 (128 = 1.0) after the module's update.
    pub stage_light: u8,
}

/// Running totals for reports.
#[derive(Clone, Debug, Default)]
pub struct WorkerStats {
    pub calls: usize,
    pub steps: u64,
    pub max_steps: u64,
    pub max_micros: u64,
    pub total_micros: u64,
    pub prims: usize,
    pub max_prims: usize,
    /// Real updates that found no result ready in time (cached returns used).
    pub late: usize,
    /// Updates undone by skip presses (the copy had run ahead).
    pub rewound: usize,
    pub stops: Vec<String>,
    pub boundary: std::collections::BTreeMap<&'static str, usize>,
    pub missing_textures: std::collections::BTreeSet<(i32, i32)>,
}

/// The worker thread stepping its own copy of the battle one update ahead.
pub struct EffectWorker {
    /// (In a mutex so the worker handle is `Sync` for the Bevy resource.)
    rx: Mutex<Receiver<TickResult>>,
    /// A skip press: (generation, the first update after it).
    skip: Arc<Mutex<Option<(u32, u32)>>>,
    generation: u32,
    stop: Arc<AtomicBool>,
    next: u32,
    pub stats: Arc<Mutex<WorkerStats>>,
    cards: [u16; 2],
    own_images: [Arc<Vec<Option<roses_assets::RgbaImage>>>; 2],
}

impl Drop for EffectWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Model indices of both cards (from the executable's table).
fn models_of(image: Option<&roses_fx::bus::Image>, cards: [u16; 2]) -> [u16; 2] {
    let Some(image) = image else {
        return [0; 2];
    };
    cards.map(|c| roses_fx::bmodule::model_index(image, c).unwrap_or(0))
}

/// The worker loop: tick the copy, running each effect call live.
fn work(mut battle: ArenaBattle, inputs: EffectInputs, tx: SyncSender<TickResult>, skip: Arc<Mutex<Option<(u32, u32)>>>, stop: Arc<AtomicBool>, stats: Arc<Mutex<WorkerStats>>) {
    let mut cards = [battle.units[0].data.card, battle.units[1].data.card];
    // Research: the attacker runs another card's program with its own
    // model index (the emulator footage of `emu_module.py`).
    if let Some(card) = std::env::var("ROSES_ARENA_EFFECT_CARD").ok().and_then(|v| v.parse::<u16>().ok()) {
        cards[0] = card;
    }
    let image = roses_fx::bus::Image::from_elf(&inputs.program.1).map(Arc::new);
    let models = models_of(image.as_deref(), cards);
    let mut effects: [Option<Effect>; 2] = [None, None];
    let mut dead = [false; 2];
    let mut tick = 0u32;
    let mut generation = 0u32;
    let mut pending: Option<(u32, u32)> = None;
    let mut checkpoints: std::collections::VecDeque<Checkpoint> = std::collections::VecDeque::new();
    loop {
        if let Some(p) = skip.lock().unwrap_or_else(|e| e.into_inner()).take() {
            pending = Some(p);
        }
        // The skip press: the real battle pressed it after update `at` - 1;
        // rewind to that point when ahead (checkpoints newest first), then skip
        // there as the real battle did.
        if let Some((g, at)) = pending.filter(|p| p.1 <= tick) {
            pending = None;
            if at < tick {
                if checkpoints.front().is_none_or(|c| c.tick > at) {
                    stats.lock().unwrap_or_else(|e| e.into_inner()).stops.push(format!("skip at update {at}: no checkpoint (worker at {tick})"));
                    return;
                }
                stats.lock().unwrap_or_else(|e| e.into_inner()).rewound += (tick - at) as usize;
                while let Some(c) = checkpoints.pop_back() {
                    let Checkpoint { tick: ct, battle: cb, present, dead: cd, fx } = c;
                    for (role, f) in fx.into_iter().enumerate() {
                        if !present[role] {
                            effects[role] = None;
                        } else {
                            if let (Some(e), Some(p)) = (effects[role].as_mut(), f) {
                                *e = p;
                            }
                        }
                    }
                    dead = cd;
                    if ct == at {
                        battle = cb;
                        break;
                    }
                }
                tick = at;
            }
            battle.skip();
            generation = g;
            checkpoints.clear();
        }
        if battle.done() || stop.load(Ordering::Relaxed) {
            break;
        }
        // This update's checkpoint.
        let checkpoint = Checkpoint {
            tick,
            battle: battle.clone(),
            present: [effects[0].is_some(), effects[1].is_some()],
            dead,
            fx: effects.clone(),
        };
        let mut calls = Vec::new();
        battle.tick_live(&mut |b: &ArenaBattle, role: usize, call: usize, shared: &mut FxShared| {
            let cached = b.cached_return(role, call);
            if call == 0 {
                let handles = [ROLE_HANDLES[role], ROLE_HANDLES[1 - role]];
                let textures = EffectTextures { texeff: inputs.texeff.clone(), own: inputs.own[role].clone(), other: Vec::new() };
                let port = caught(format_args!("card {} load", cards[role]), || image.clone().and_then(|i| roses_fx::bmodule::BPort::load(i, &inputs.program.0, cards[role], handles, &textures)));
                effects[role] = match port {
                    Err(e) => {
                        stats.lock().unwrap_or_else(|e| e.into_inner()).stops.push(format!("card {}: {e}", cards[role]));
                        None
                    }
                    Ok(p) => p.map(Box::new),
                };
                dead[role] = effects[role].is_none();
            }
            if dead[role] {
                return cached;
            }
            // (Gone with `dead` clear: stopped, then a rewind to before it.)
            let Some(fx) = effects[role].as_mut() else {
                return cached;
            };
            let mut world = BattleWorld::new(b, role, [models[role], models[1 - role]]);
            world.state = inputs.battle.filter(|_| OUTCOME_CARDS.contains(&cards[role]));
            // The module draws from the game's one rand state [emu emu_rand.py].
            roses_fx::EffectPort::set_rand_state(fx.as_mut(), shared.rand);
            let start = Instant::now();
            match caught(format_args!("card {} call {call}", cards[role]), || roses_fx::EffectPort::update(fx.as_mut(), &world)) {
                Ok(u) => {
                    let micros = start.elapsed().as_micros() as u64;
                    shared.rand = roses_fx::EffectPort::rand_state(fx.as_mut());
                    // obj+0x20D bit 0 as the module left it (module roles:
                    // 0 = the attacker).
                    for (r, f) in u.flags_20d.iter().enumerate() {
                        shared.frozen[if r == 0 { role } else { 1 - role }] = f & 1 != 0;
                    }
                    let order = roses_fx::types::draw_order(&u.prims, &u.layers);
                    // The first frame copy's place in that order: what comes
                    // after is not in the copy [F 0x207FC0].
                    let copy_at = u.frame_copies.first().map(|c| roses_fx::types::copy_position(&u.prims, &u.layers, c));
                    let mut unit_rt = [false; 2];
                    let mut rt_vp = [None; 2];
                    for r in &u.unit_renders {
                        unit_rt[usize::from(r.role & 1)] = true;
                        if r.pre_vp.is_some() {
                            rt_vp[usize::from(r.role & 1)] = r.pre_vp;
                        }
                    }
                    // A target sampled without a render this update keeps
                    // its unit drawn there [H].
                    for p in &u.prims {
                        if let Some(roses_fx::types::TextureKey::UnitRt(r)) = p.texture {
                            unit_rt[usize::from(r & 1)] = true;
                        }
                    }
                    calls.push(CallResult {
                        role,
                        call,
                        ret: Some(u.ret),
                        prims: order
                            .into_iter()
                            .enumerate()
                            .map(|(k, i)| {
                                let mut p = u.prims[i].clone();
                                p.post_copy = copy_at.is_some_and(|at| k >= at);
                                p
                            })
                            .collect(),
                        boundary: u.boundary,
                        missing_textures: u.missing_textures,
                        unit_rt,
                        rt_vp,
                        unit_writes: u.unit_writes,
                        stage_light: u.stage_light,
                        shared: Some(*shared),
                        steps: u.steps,
                        micros,
                    });
                    Some(u.ret)
                }
                Err(e) => {
                    stats.lock().unwrap_or_else(|e| e.into_inner()).stops.push(format!("card {} call {call}: {e}", cards[role]));
                    dead[role] = true;
                    effects[role] = None;
                    cached
                }
            }
        });
        {
            let mut s = stats.lock().unwrap_or_else(|e| e.into_inner());
            for c in &calls {
                s.calls += 1;
                s.steps += c.steps;
                s.max_steps = s.max_steps.max(c.steps);
                s.max_micros = s.max_micros.max(c.micros);
                s.total_micros += c.micros;
                s.prims += c.prims.len();
                s.max_prims = s.max_prims.max(c.prims.len());
                for b in &c.boundary {
                    *s.boundary.entry(b.1).or_insert(0) += 1;
                }
                s.missing_textures.extend(c.missing_textures.iter().copied());
            }
        }
        checkpoints.push_back(checkpoint);
        while checkpoints.len() > CHECKPOINTS {
            checkpoints.pop_front();
        }
        if tx.send(TickResult { tick, generation, calls }).is_err() {
            return;
        }
        tick += 1;
    }
}

impl EffectWorker {
    /// Start on a copy of `battle` (before its first update).
    pub fn start(battle: &ArenaBattle, inputs: EffectInputs) -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel(LOOKAHEAD);
        let skip = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let stats = Arc::new(Mutex::new(WorkerStats::default()));
        let copy = battle.clone();
        let cards = [battle.units[0].data.card, battle.units[1].data.card];
        let own_images = inputs.own_images.clone();
        let (s2, st2, stats2) = (skip.clone(), stop.clone(), stats.clone());
        std::thread::Builder::new()
            .name("bmodule".into())
            .spawn(move || work(copy, inputs, tx, s2, st2, stats2))
            .expect("spawn the attack effect worker");
        Self { rx: Mutex::new(rx), skip, generation: 0, stop, next: 0, stats, cards, own_images }
    }

    /// The real battle's skip press, after its update `next` - 1: the copy
    /// rewinds there (it may have run ahead) and skips too; results from
    /// before are dropped. The effect keeps running through the exit, as
    /// the executor keeps calling it.
    pub fn skip(&mut self) {
        self.generation += 1;
        *self.skip.lock().unwrap_or_else(|e| e.into_inner()) = Some((self.generation, self.next));
    }

    /// One update of the real `battle` with the worker's returns; waits up
    /// to `wait` for them (else the cached returns, nothing drawn). Returns
    /// this update's draws.
    pub fn tick(&mut self, battle: &mut ArenaBattle, wait: Duration) -> Option<EffectDraw> {
        let want = self.next;
        self.next += 1;
        let deadline = Instant::now() + wait;
        let result = loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let got = self.rx.get_mut().unwrap_or_else(|e| e.into_inner()).recv_timeout(left);
            match got {
                Ok(r) if r.generation != self.generation || r.tick < want => continue,
                Ok(r) if r.tick == want => break Some(r),
                Ok(_) => break None,
                Err(RecvTimeoutError::Timeout) | Err(RecvTimeoutError::Disconnected) => break None,
            }
        };
        if result.is_none() && battle.effect_task().is_some() {
            self.stats.lock().unwrap_or_else(|e| e.into_inner()).late += 1;
        }
        // The game goes on after a worker's port panic; a test does not.
        #[cfg(test)]
        {
            let stops = self.stats.lock().unwrap_or_else(|e| e.into_inner()).stops.clone();
            if let Some(e) = stops.iter().find(|s| s.contains(PANICKED)) {
                panic!("{e}");
            }
        }
        let calls = result.map(|r| r.calls).unwrap_or_default();
        battle.tick_live(&mut |b: &ArenaBattle, role: usize, call: usize, shared: &mut FxShared| {
            let c = calls.iter().find(|c| c.role == role && c.call == call);
            if let Some(r) = c.and_then(|c| c.shared) {
                *shared = r;
            }
            c.and_then(|c| c.ret).or_else(|| b.cached_return(role, call))
        });
        if std::env::var_os("ROSES_BMODULE_LOG").is_some() {
            for c in &calls {
                eprintln!(
                    "bmodule: update {want}: role {} call {} ret {:?} prims {} ({} us) camera eye {:?} target {:?} d {:.1}; units {:?}",
                    c.role,
                    c.call,
                    c.ret,
                    c.prims.len(),
                    c.micros,
                    battle.camera.eye,
                    battle.camera.target,
                    battle.camera.distance,
                    battle.units.iter().map(|u| (u.clock.motion, u.clock.time, u.alpha)).collect::<Vec<_>>()
                );
            }
        }
        let role = calls.first()?.role;
        let unit_writes = calls.last()?.unit_writes;
        let stage_light = calls.last()?.stage_light;
        let unit_rt = calls.iter().fold([false; 2], |a, c| [a[0] || c.unit_rt[0], a[1] || c.unit_rt[1]]);
        let rt_vp = calls.iter().fold([None; 2], |a, c| [c.rt_vp[0].or(a[0]), c.rt_vp[1].or(a[1])]);
        let prims: Vec<GsPrim> = calls.into_iter().flat_map(|c| c.prims).collect();
        Some(EffectDraw { card: self.cards[role], prims: Arc::new(prims), own: self.own_images[role].clone(), unit_rt, rt_vp, unit_writes, stage_light })
    }
}

/// Draw state of a batch of effect triangles (one material).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FxState {
    /// The shim's texture, or `None` untextured.
    pub texture: Option<roses_fx::types::TextureKey>,
    /// Textured from the frame buffer (TEX0 TBP0 0 / 0x800,
    /// `TextureKey::Frame`).
    pub frame: bool,
    /// 0 additive (Cs As + Cd), 1 subtractive (Cd - Cs As), 2 replace, 3
    /// normal ((Cs - Cd) As + Cd), 4 ALPHA 0x84 ((Cs - Cd) As, clamped).
    pub blend: u8,
    /// TFX (0 modulate, 1 decal, 2 / 3 highlight), TCC, the blend's alpha
    /// source (0 As, 1 FIX, 2 none: unblended), FIX.
    pub gs: [f32; 4],
    /// GS texture sampling: CLAMP WMS, WMT (0 repeat, 1 clamp, 2 region
    /// clamp, 3 region repeat; repeat without a CLAMP write), TEX1 MMAG and
    /// MMIN as linear (1) or nearest (0).
    pub wrap: [f32; 4],
    /// CLAMP MINU, MAXU, MINV, MAXV (texels).
    pub region: [f32; 4],
    /// 2^TW, 2^TH, TEX1 K (LOD bias) and L.
    pub lod: [f32; 4],
    /// A PSMCT32 read of the 16-bit frame buffer (TEX0 PSM 0 at TBP 0 / 0x800):
    /// the type-5 effect textures a monster's slot lacks leave TEX0 / TEX1 as
    /// low RAM's 0x30 / 0x40 (0: TBP 0, 1 x 1 texels, TCC 0) [F 0x202530 /
    /// 0x207E50; emu card 133 c200 draws 14-21], so the texel is the frame's
    /// first word: pixels (0, 0) and (8, 0) of the C_16S buffer as R, G and B,
    /// A [F: the PSMCT16S column layout puts x 8 in the word's upper half].
    pub frame_word: bool,
    /// Textured from a GS copy of the frame being drawn (0x207FC0 with a
    /// destination: TBP 0x2000 or another block, not the display buffers
    /// 0 / 0x800): the frame as drawn up to the copy, not the previous one [F
    /// 0x207FC0 BITBLTBUF SBP = 0x209640(1), the buffer being drawn].
    pub copy: bool,
    /// Drawn into this role's offscreen unit target (`GsPrim::target`),
    /// before the composite that samples it.
    pub target: Option<u8>,
    /// Into the target before the unit's model (`GsPrim::target_pre`).
    pub target_pre: bool,
    /// Drawn after the update's first frame copy (`GsPrim::post_copy`).
    pub post_copy: bool,
}

/// Effect triangles in GS window coordinates sharing one [`FxState`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FxBatch {
    pub state: Option<FxState>,
    pub positions: Vec<[f32; 2]>,
    /// Per vertex: GS Z24 as depth, (Z + 2048.25) / 16779264 (= 1 / w, the
    /// units' and stage's depth).
    pub depths: Vec<f32>,
    /// Normalised to the texture image; ST still multiplied by Q (the GS
    /// interpolates S, T, Q across the screen and divides per pixel).
    pub uvs: Vec<[f32; 2]>,
    /// Per vertex: (the Q dividing `uvs`: Q for ST, 1 for UV; the RGBAQ Q
    /// for the LOD).
    pub qs: Vec<[f32; 2]>,
    pub colours: Vec<[u8; 4]>,
}

/// What [`fx_batches`] could not reproduce, by kind.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FxReport {
    pub blends: std::collections::BTreeMap<u64, usize>,
    pub textures: std::collections::BTreeMap<u16, usize>,
    pub points: usize,
}

fn fx_state(p: &GsPrim, report: &mut FxReport) -> FxState {
    let (blend, alpha_source, fix) = match p.alpha.filter(|_| p.blended()) {
        None => (2, 2.0, 128.0),
        Some(a) => {
            let (aa, bb, cc, dd) = (a & 3, (a >> 2) & 3, (a >> 4) & 3, (a >> 6) & 3);
            let blend = match (aa, bb, dd) {
                (0, 2, 1) => 0,
                (2, 0, 1) => 1,
                (0, 2, 2) => 2,
                (0, 1, 1) => 3,
                (0, 1, 2) => 4,
                _ => {
                    *report.blends.entry(a & 0xFF).or_insert(0) += 1;
                    0
                }
            };
            if cc == 1 {
                *report.blends.entry(a & 0xFF).or_insert(0) += 1;
            }
            (blend, if cc == 2 { 1.0 } else { 0.0 }, ((a >> 32) & 0xFF) as f32)
        }
    };
    let (texture, frame, tfx, tcc) = match p.tex0.filter(|_| p.textured()) {
        Some(t) => {
            let tbp = (t & 0x3FFF) as u16;
            let frame = p.texture == Some(TextureKey::Frame);
            if p.texture.is_none() {
                *report.textures.entry(tbp).or_insert(0) += 1;
            }
            (p.texture, frame, ((t >> 35) & 3) as f32, ((t >> 34) & 1) as f32)
        }
        None => (None, false, 0.0, 1.0),
    };
    let (mut wrap, mut region, mut lod) = ([0.0; 4], [0.0; 4], [1.0, 1.0, 0.0, 0.0]);
    if let (Some(t), true) = (p.tex0, p.textured()) {
        let c = p.clamp.unwrap_or(0);
        let t1 = p.tex1.unwrap_or(0x60);
        // TEX1: MMAG bit 5, MMIN bits 6-8 (1 linear, 4 / 5 linear on the
        // base level: MXL 0), L bits 19-20, K bits 32-43 (signed, 1/16).
        let mmin = (t1 >> 6) & 7;
        wrap = [(c & 3) as f32, ((c >> 2) & 3) as f32, ((t1 >> 5) & 1) as f32, f32::from(u8::from(matches!(mmin, 1 | 4 | 5)))];
        region = [((c >> 4) & 0x3FF) as f32, ((c >> 14) & 0x3FF) as f32, ((c >> 24) & 0x3FF) as f32, ((c >> 34) & 0x3FF) as f32];
        let k = (((t1 >> 32) & 0xFFF) as i32) << 20 >> 20;
        lod = [(1u32 << ((t >> 26) & 15).min(10)) as f32, (1u32 << ((t >> 30) & 15).min(10)) as f32, k as f32 / 16.0, ((t1 >> 19) & 3) as f32];
    }
    let frame_word = frame && p.tex0.is_some_and(|t| (t >> 20) & 0x3F == 0);
    // A frame copy, or the buffer being drawn read directly (the low-RAM
    // TEX0 on every other frame): both hold the frame drawn so far.
    let copy = frame && (p.drawn_buffer || p.tex0.is_some_and(|t| !matches!(t & 0x3FFF, 0 | 0x800)));
    FxState { texture, frame, blend, gs: [tfx, tcc, alpha_source, fix], wrap, region, lod, frame_word, copy, target: p.target, target_pre: p.target_pre, post_copy: p.post_copy }
}

/// The primitives (in GS order) as triangle batches for the FX pass, in
/// draw order: a primitive joins an earlier batch of its state only across
/// batches it commutes with (additive and subtractive draws commute up to
/// saturation [H]; replace and normal blends keep their place). Lines are 1-pixel quads, sprites two
/// triangles; flat shading takes each primitive's last vertex colour.
/// `size` gives a texture's image size (texels) for UV normalisation.
pub fn fx_batches(prims: &[GsPrim], size: &dyn Fn(&FxState, &GsPrim) -> Option<(f32, f32)>) -> (Vec<FxBatch>, FxReport) {
    let mut report = FxReport::default();
    let mut batches: Vec<FxBatch> = Vec::new();
    for p in prims {
        let state = fx_state(p, &mut report);
        if state.texture.is_none() && p.textured() {
            continue;
        }
        let commutes = |s: &FxState| s.blend <= 1 && state.blend <= 1;
        let mut found = None;
        for (k, b) in batches.iter().enumerate().rev() {
            match b.state {
                Some(s) if s == state => {
                    found = Some(k);
                    break;
                }
                // Nothing joins a batch across a frame copy's first sampler
                // (the copy's place in the draw order [H]).
                Some(s) if commutes(&s) && !s.copy => {}
                _ => break,
            }
        }
        let k = match found {
            Some(k) => k,
            None => {
                batches.push(FxBatch { state: Some(state), ..Default::default() });
                batches.len() - 1
            }
        };
        let b = &mut batches[k];
        let (w, h) = if p.textured() { size(&state, p).unwrap_or((1.0, 1.0)) } else { (1.0, 1.0) };
        let (tw, th) = p.texture_size().map_or((1.0, 1.0), |(a, b)| (a as f32, b as f32));
        let uv = |v: &roses_fx::types::GsVertex| -> [f32; 2] {
            if p.uv_texels() {
                [v.uv[0] / w, v.uv[1] / h]
            } else {
                [v.uv[0] * tw / w, v.uv[1] * th / h]
            }
        };
        let q = |v: &roses_fx::types::GsVertex| -> [f32; 2] {
            let lod = if v.q != 0.0 { v.q } else { 1.0 };
            [if p.uv_texels() { 1.0 } else { lod }, lod]
        };
        let vs = &p.vertices;
        let flat = !p.gouraud();
        let depth = |v: &roses_fx::types::GsVertex| ((f64::from(v.z) + 2048.25) / 16_779_264.0) as f32;
        let tri = |b: &mut FxBatch, i: [usize; 3]| {
            let last = vs[i[2]].rgba;
            for j in i {
                b.positions.push([vs[j].x, vs[j].y]);
                b.depths.push(depth(&vs[j]));
                b.uvs.push(uv(&vs[j]));
                b.qs.push(q(&vs[j]));
                b.colours.push(if flat { last } else { vs[j].rgba });
            }
        };
        match p.kind {
            0 => report.points += vs.len(),
            1 | 2 => {
                let pairs: Vec<(usize, usize)> = if p.kind == 1 { (0..vs.len() / 2).map(|i| (2 * i, 2 * i + 1)).collect() } else { (1..vs.len()).map(|i| (i - 1, i)).collect() };
                for (i, j) in pairs {
                    let (a, c) = (&vs[i], &vs[j]);
                    let (dx, dy) = (c.x - a.x, c.y - a.y);
                    let len = (dx * dx + dy * dy).sqrt();
                    if len <= 0.0 {
                        continue;
                    }
                    // Half a pixel either side of the segment.
                    let (nx, ny) = (-dy / len * 0.5, dx / len * 0.5);
                    // AA1 (descriptor 0x400) is drawn as a plain line: the
                    // emulator's frames keep the vertex alpha (B220's strands:
                    // alpha 128 at call 126 single bright pixels, 16 at 195+)
                    // [emu dumps-bmodule-036 / 043].
                    let ca = if flat { c.rgba } else { a.rgba };
                    let cc = c.rgba;
                    let quad = [([a.x + nx, a.y + ny], ca, uv(a)), ([a.x - nx, a.y - ny], ca, uv(a)), ([c.x + nx, c.y + ny], cc, uv(c)), ([c.x - nx, c.y - ny], cc, uv(c))];
                    let (da, dc) = (depth(a), depth(c));
                    let (qa, qc) = (q(a), q(c));
                    for k in [0, 1, 2, 1, 3, 2] {
                        b.positions.push(quad[k].0);
                        b.colours.push(quad[k].1);
                        b.uvs.push(quad[k].2);
                        b.qs.push(if k < 2 { qa } else { qc });
                        b.depths.push(if k < 2 { da } else { dc });
                    }
                }
            }
            3 => {
                for t in 0..vs.len() / 3 {
                    tri(b, [3 * t, 3 * t + 1, 3 * t + 2]);
                }
            }
            4 => {
                for t in 2..vs.len() {
                    tri(b, [t - 2, t - 1, t]);
                }
            }
            5 => {
                for t in 2..vs.len() {
                    tri(b, [0, t - 1, t]);
                }
            }
            6 => {
                for s in 0..vs.len() / 2 {
                    let (a, c) = (&vs[2 * s], &vs[2 * s + 1]);
                    // Sprite corners take S / Q of their own vertex.
                    let div = |v: &roses_fx::types::GsVertex| {
                        let (t, k) = (uv(v), q(v)[0]);
                        [t[0] / k, t[1] / k]
                    };
                    let (ua, uc) = (div(a), div(c));
                    let corner = |x: &roses_fx::types::GsVertex, y: &roses_fx::types::GsVertex, u: [f32; 2], v: [f32; 2]| ([x.x, y.y], [u[0], v[1]]);
                    let quad = [corner(a, a, ua, ua), corner(c, a, uc, ua), corner(a, c, ua, uc), corner(c, c, uc, uc)];
                    // A sprite's Z (and Q) is its second vertex's [GS].
                    let dc = depth(c);
                    let qc = [1.0, q(c)[1]];
                    for k in [0, 1, 2, 1, 3, 2] {
                        b.positions.push(quad[k].0);
                        b.uvs.push(quad[k].1);
                        b.qs.push(qc);
                        b.colours.push(c.rgba);
                        b.depths.push(dc);
                    }
                }
            }
            _ => {}
        }
    }
    batches.retain(|b| !b.positions.is_empty());
    (batches, report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use roses_fx::types::{GsVertex, TextureKey};

    /// A port's panic comes back as an `Err` with its message.
    #[test]
    fn the_net_turns_a_port_panic_into_an_err() {
        assert_eq!(caught("test", || 5), Ok(5));
        let e = caught("test", || -> i32 { panic!("unported draw case {:#x}", 7) }).unwrap_err();
        assert!(e.contains(PANICKED) && e.contains("unported draw case 0x7"), "{e}");
        let e = caught("test", || -> i32 { panic!("static text") }).unwrap_err();
        assert!(e.contains("static text"), "{e}");
    }

    /// The net never hides a port's panic from a test.
    #[test]
    #[should_panic(expected = "unported task step")]
    fn a_port_panic_still_fails_a_test() {
        let _ = guarded("test", || -> i32 { panic!("unported task step") });
    }

    fn v(x: f32, y: f32, rgba: [u8; 4], uv: [f32; 2]) -> GsVertex {
        GsVertex { x, y, z: 1, rgba, q: 1.0, uv }
    }

    fn prim(flags: u16, vertices: Vec<GsVertex>, alpha: Option<u64>, tex0: Option<u64>) -> GsPrim {
        GsPrim {
            flags,
            kind: (flags & 7) as u8,
            vertices,
            alpha,
            texture: tex0.and_then(|t| TextureKey::from_tbp((t & 0x3FFF) as u16)),
            tex0,
            tex1: None,
            clamp: None,
            buffered: false,
            desc: 0,
            ot_key: None,
            layer: None,
            ..Default::default()
        }
    }

    #[test]
    fn lines_sprites_and_blends_become_batches() {
        let add = Some(0x80_0000_0048);
        let sub = Some(0x80_0000_0042);
        let tex = Some(u64::from(TextureKey::TexEff(15).tbp()) | (7 << 26) | (6u64 << 30));
        let prims = vec![
            // An untextured flat line (B220's 0x601): one 1-pixel quad, last vertex colour.
            prim(0x601, vec![v(0.0, 0.0, [1, 2, 3, 4], [0.0; 2]), v(10.0, 0.0, [5, 6, 7, 8], [0.0; 2])], add, None),
            // A textured FST sprite: two triangles, UV over the 128 x 64 image.
            prim(0xA26, vec![v(0.0, 0.0, [128; 4], [0.0, 0.0]), v(16.0, 8.0, [128; 4], [64.0, 32.0])], add, tex),
            // A subtractive triangle.
            prim(0x203, vec![v(0.0, 0.0, [9; 4], [0.0; 2]), v(1.0, 0.0, [9; 4], [0.0; 2]), v(0.0, 1.0, [9; 4], [0.0; 2])], sub, None),
        ];
        let (b, report) = fx_batches(&prims, &|_, _| Some((128.0, 64.0)));
        assert_eq!(report, FxReport::default());
        assert_eq!(b.len(), 3);
        assert_eq!(b[0].positions.len(), 6);
        assert!(b[0].colours.iter().all(|c| *c == [5, 6, 7, 8]));
        assert_eq!(b[0].positions[0], [0.0, 0.5]);
        assert_eq!(b[1].state.unwrap().texture, Some(TextureKey::TexEff(15)));
        assert_eq!((b[1].uvs[4], b[1].positions[4]), ([0.5, 0.5], [16.0, 8.0]));
        assert_eq!(b[2].state.unwrap().blend, 1);
        assert_eq!(b[2].positions.len(), 3);
        // Depth from GS Z: (Z + 2048.25) / 16779264.
        assert_eq!(b[2].depths, vec![(2049.25f64 / 16_779_264.0) as f32; 3]);
    }

    /// ST primitives keep S, T and Q per vertex (divided per pixel), FST
    /// ones Q 1; CLAMP / TEX1 / TW reach the state; ALPHA 0x84 is blend 4.
    #[test]
    fn st_q_wrap_and_alpha_84_reach_the_batches() {
        let tex = u64::from(TextureKey::TexEff(15).tbp()) | (7 << 26) | (6u64 << 30);
        let mut st = prim(0x234, vec![v(0.0, 0.0, [128; 4], [0.5, 0.25]), v(9.0, 0.0, [128; 4], [1.0, 0.5]), v(0.0, 9.0, [128; 4], [0.0, 0.0])], Some(0x80_0000_0084), Some(tex));
        st.vertices[1].q = 2.0;
        st.tex1 = Some(0x20);
        st.clamp = Some(2 | (1 << 2) | (3 << 4) | (60 << 14) | (5 << 24) | (40 << 34));
        let (b, report) = fx_batches(&[st], &|_, _| Some((128.0, 64.0)));
        assert_eq!(report, FxReport::default());
        let s = b[0].state.unwrap();
        assert_eq!(s.blend, 4);
        assert_eq!(s.wrap, [2.0, 1.0, 1.0, 0.0]);
        assert_eq!(s.region, [3.0, 60.0, 5.0, 40.0]);
        assert_eq!(s.lod, [128.0, 64.0, 0.0, 0.0]);
        // S * 2^TW / w (not divided by Q) and Q per vertex.
        assert_eq!(b[0].uvs[1], [1.0, 0.5]);
        assert_eq!(b[0].qs[1], [2.0, 2.0]);
        let fst = prim(0xA26, vec![v(0.0, 0.0, [128; 4], [0.0, 0.0]), v(16.0, 8.0, [128; 4], [64.0, 32.0])], None, Some(tex));
        let (b, _) = fx_batches(&[fst], &|_, _| Some((128.0, 64.0)));
        assert!(b[0].qs.iter().all(|q| q[0] == 1.0));
        // Without a CLAMP write: repeat; no TEX1: linear.
        assert_eq!(b[0].state.unwrap().wrap, [0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn batches_keep_the_draw_order_across_normal_blends() {
        let tri = |rgba: [u8; 4], alpha: u64| prim(0x203, vec![v(0.0, 0.0, rgba, [0.0; 2]), v(1.0, 0.0, rgba, [0.0; 2]), v(0.0, 1.0, rgba, [0.0; 2])], Some(alpha), None);
        let (add, sub, normal) = (0x80_0000_0048, 0x80_0000_0042, 0x80_0000_0044);
        // Additive and subtractive commute: the second additive joins the first.
        let (b, _) = fx_batches(&[tri([1; 4], add), tri([2; 4], sub), tri([3; 4], add)], &|_, _| None);
        assert_eq!(b.iter().map(|b| b.positions.len()).collect::<Vec<_>>(), vec![6, 3]);
        // A normal blend in between keeps its place.
        let (b, _) = fx_batches(&[tri([1; 4], add), tri([2; 4], normal), tri([3; 4], add)], &|_, _| None);
        assert_eq!(b.iter().map(|b| (b.state.unwrap().blend, b.positions.len())).collect::<Vec<_>>(), vec![(0, 3), (3, 3), (0, 3)]);
    }
}

#[cfg(test)]
pub(crate) mod private_tests {
    use super::*;
    use crate::duel_arena_battle::{ArenaUnitData, Outcome, Unit};
    use roses_client::monster_pose::MotionClock;
    use std::sync::Arc;

    fn unit(card: u16, role: usize, motion: u8, t: f32) -> Unit {
        let models = std::path::PathBuf::from(std::env::var("ROSES_MODELS_MANIFEST").unwrap());
        let model = crate::duel_models::load_model(&models, card).unwrap();
        let data = Arc::new(ArenaUnitData { card, size: 0, flags: 0, model, attack_camera: None, victory_camera: None, effect: None });
        let mut u = Unit::new(data, role);
        let length = u.data.model.model.motion(motion).unwrap().length;
        u.clock = MotionClock::new(motion, length);
        u.clock.time = t;
        u
    }

    /// Research: writes card ROSES_FX_CARD's effect textures as raw RGBA
    /// files (`<dir>/own-<card>-<slot>-<w>x<h>.rgba`) to ROSES_FX_DUMP.
    #[test]
    #[ignore = "research dump; needs a private models import"]
    fn dump_effect_textures() {
        let models = std::path::PathBuf::from(std::env::var("ROSES_MODELS_MANIFEST").unwrap());
        let card: u16 = std::env::var("ROSES_FX_CARD").unwrap().parse().unwrap();
        let dir = std::path::PathBuf::from(std::env::var("ROSES_FX_DUMP").unwrap());
        for (slot, t) in crate::duel_models::effect_textures(&models, card).unwrap().into_iter().enumerate() {
            let Some(t) = t else { continue };
            let h: Vec<String> = t.rh2[..0x60].chunks(4).map(|w| format!("{:08x}", u32::from_le_bytes([w[0], w[1], w[2], w[3]]))).collect();
            eprintln!("slot {slot}: {} bytes, header {}", t.rh2.len(), h.join(" "));
            if let Some(i) = t.image {
                std::fs::write(dir.join(format!("own-{card}-{slot}-{}x{}.rgba", i.width, i.height)), &i.pixels).unwrap();
            }
        }
    }

    /// Research: where the arena renderer puts the defender at update
    /// ROSES_PROBE_UPDATE of the footage battle (screen bbox of its frame).
    #[test]
    #[ignore = "research probe; needs private imports"]
    fn probe_defender_on_screen() {
        let (setup, _) = footage_inputs([406, 370]);
        let mut b = ArenaBattle::new(setup.units, Outcome::from_battle(2, 1200, 1400), setup.stage_end, 1);
        let n: usize = std::env::var("ROSES_PROBE_UPDATE").map(|v| v.parse().unwrap()).unwrap_or(332);
        for _ in 0..=n {
            b.tick();
        }
        let ndc = crate::duel_arena::ndc(&b.camera.view(), &crate::camera::identity()).unwrap();
        for role in 0..2 {
            let u = &b.units[role];
            let mut scene = crate::duel_models::UnitScene::new(u.data.model.clone(), 0, (3, 3), role as u8, 1.0).unwrap();
            scene.set_pose(u.object.position, u.object.rotation[1]);
            scene.motion = u.clock.motion;
            scene.clock = u.clock;
            let frame = scene.frame().unwrap();
            let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
            for m in &frame {
                for p in &m.positions {
                    let v = [p[0] as f64, p[1] as f64, p[2] as f64, 1.0];
                    let o: [f64; 4] = std::array::from_fn(|r| (0..4).map(|c| ndc[c][r] * v[c]).sum());
                    let c = [o[0] / o[3], o[1] / o[3], o[3]];
                    for k in 0..3 {
                        lo[k] = lo[k].min(c[k]);
                        hi[k] = hi[k].max(c[k]);
                    }
                }
            }
            eprintln!("role {role} card {} motion {} t {}: clip-space bbox {lo:?} .. {hi:?}; camera {:?}", u.data.card, u.clock.motion, u.clock.time, b.camera);
        }
    }

    /// obj+0x80 in the footage RAM against `Unit::position` [emu].
    #[test]
    #[ignore = "needs a private models import"]
    fn animated_positions_match_the_footage() {
        let cases: [(u16, usize, u8, f32, [f32; 3]); 4] = [
            (406, 0, 0, 21.0, [0.0, 50.37019, 90.21621]),
            (406, 0, 2, 59.0, [0.0, 49.33564, 82.94838]),
            (370, 1, 2, 208.0, [0.0, 47.05362, -84.44221]),
            (370, 1, 4, 12.0, [0.0, 48.41872, -84.44224]),
        ];
        for (card, role, motion, t, want) in cases {
            let u = unit(card, role, motion, t);
            let got = u.position();
            eprintln!("card {card} m{motion} t{t}: {got:?} want {want:?}");
            let err = (0..3).map(|i| (got[i] as f32 - want[i]).abs()).fold(0.0f32, f32::max);
            assert!(err < 1e-3, "{err}");
        }
    }

    /// Node results in the footage RAM (battle-pine 032 / 044 / 060,
    /// obj+0xA0): Kumootoko (406, attacker) nodes 0x47 / 0x26, Orion (370)
    /// 0x2F / 0x41 [emu].
    #[test]
    #[ignore = "needs a private models import"]
    fn node_results_match_the_footage() {
        let cases: [(u16, usize, u8, f32, u8, [f32; 3]); 6] = [
            (406, 0, 2, 59.0, 0x47, [-0.36296, 53.96535, 76.16504]),
            (406, 0, 2, 59.0, 0x26, [-0.18711, 53.74480, 79.04549]),
            (406, 0, 0, 21.0, 0x47, [0.30193, 53.34974, 84.37895]),
            (406, 0, 0, 28.0, 0x26, [0.57131, 54.47805, 86.32563]),
            (370, 1, 2, 208.0, 0x2F, [0.98699, 51.58561, -82.44344]),
            (370, 1, 2, 208.0, 0x41, [-0.00003, 53.11908, -83.21835]),
        ];
        for (card, role, motion, t, node, want) in cases {
            let u = unit(card, role, motion, t);
            let got = node_world(&u, node).unwrap();
            let err = (0..3).map(|i| (got[i] - want[i]).abs()).fold(0.0f32, f32::max);
            eprintln!("card {card} m{motion} t{t} node {node:#x}: {got:?} want {want:?} err {err:.5}");
            assert!(err < 1e-3);
        }
    }

    pub(crate) fn footage_inputs(cards: [u16; 2]) -> (crate::duel_arena::ArenaSetup, EffectInputs) {
        let models = std::path::PathBuf::from(std::env::var("ROSES_MODELS_MANIFEST").unwrap());
        let arena = crate::duel_arena::load_arena(std::path::Path::new(&std::env::var("ROSES_ARENA_MANIFEST").unwrap())).unwrap();
        let texeff = std::fs::read(std::env::var("ROSES_TEXEFF").unwrap()).unwrap();
        let provider = crate::duel_arena::arena_provider(&arena, models.clone()).unwrap();
        let setup = (provider.setup)(0, cards).unwrap();
        let tex = cards.map(|c| crate::duel_models::effect_textures(&models, c).unwrap());
        let own = tex.clone().map(|t| t.into_iter().map(|t| t.map(|t| t.rh2)).collect());
        let own_images = tex.map(|t| Arc::new(t.into_iter().map(|t| t.and_then(|t| t.image)).collect()));
        let inputs = EffectInputs { program: arena.effects.clone().unwrap(), texeff: texeff_records(&texeff), own, own_images, battle: None };
        (setup, inputs)
    }

    /// `models_of`'s table read against the interpreter's, every card and
    /// past the end. Needs ROSES_ARENA_MANIFEST.
    #[test]
    #[ignore = "needs private imports"]
    fn model_index_equals_the_interpreters() {
        let arena = crate::duel_arena::load_arena(std::path::Path::new(&std::env::var("ROSES_ARENA_MANIFEST").unwrap())).unwrap();
        let exe = &arena.effects.as_ref().unwrap().1;
        let image = roses_fx::bus::Image::from_elf(exe).unwrap();
        let mut cpu = roses_ee::Cpu::new();
        cpu.load_elf(exe).unwrap();
        for card in 0..700u16 {
            assert_eq!(roses_fx::bmodule::model_index(&image, card), roses_ee::effects::model_index(&mut cpu, card), "card {card}");
        }
    }

    /// The footage battle with the effects run live on the worker: the same
    /// path as with the headless cache (B220 277 / B125 341 calls),
    /// primitives from Kumootoko's web and Orion's counter, and the cost.
    /// Needs ROSES_MODELS_MANIFEST, ROSES_ARENA_MANIFEST, ROSES_TEXEFF.
    #[test]
    #[ignore = "needs private imports"]
    fn footage_battle_draws_both_effects_live() {
        let (setup, inputs) = footage_inputs([406, 370]);
        let battle = ArenaBattle::new(setup.units, Outcome::from_battle(2, 1200, 1400), setup.stage_end, 1);
        let mut cached = battle.clone();
        let mut real = battle.clone();
        let mut worker = EffectWorker::start(&battle, inputs);
        let mut drawn = Vec::new();
        let mut n = 0u32;
        let mut first_apart = None;
        let start = Instant::now();
        while !real.done() && n < 5000 {
            let d = worker.tick(&mut real, Duration::from_secs(5));
            cached.tick();
            // The live run freezes Orion (obj+0x20D, B220's 0x1F84D0) and
            // shares the game's rand where the headless cache cannot: the
            // paths part from there (the live one is the emulator's, see
            // `footage_battle_rand_and_sounds_match_the_emulator`).
            if (real.phase, real.sub, real.camera) != (cached.phase, cached.sub, cached.camera) && first_apart.is_none() {
                first_apart = Some(n);
            }
            drawn.push(d.map_or(0, |d| d.prims.len()));
            if let Some((role, call)) = real.effect_task() {
                if call == 1 {
                    eprintln!("update {n}: role {role}'s effect call 1 (launch at update {})", n);
                }
            }
            n += 1;
        }
        let s = worker.stats.lock().unwrap().clone();
        eprintln!(
            "{n} updates in {:?}; {} calls, {} steps (max {} / call), max {} us / call, mean {} us; prims {} (max {}); late {}; stops {:?}; boundary {:?}; missing textures {:?}",
            start.elapsed(),
            s.calls,
            s.steps,
            s.max_steps,
            s.max_micros,
            s.total_micros / s.calls.max(1) as u64,
            s.prims,
            s.max_prims,
            s.late,
            s.stops,
            s.boundary,
            s.missing_textures
        );
        let first = drawn.iter().position(|&p| p > 0);
        eprintln!("first draw at update {first:?}; every 20th update: {:?}; live and cached apart from update {first_apart:?}", drawn.iter().step_by(20).collect::<Vec<_>>());
        assert_eq!(s.calls, 277 + 341);
        assert!(s.stops.is_empty());
        assert!(s.prims > 100_000);
    }

    /// The battle's answer reaches the nine outcome records on the worker,
    /// on one arena path (the same `Outcome`, only the state differs): a
    /// target that survives is left alone, a destroyed one is faded / tinted
    /// exactly as without a state (the zero image's answer 1). Another card
    /// ignores the state. Needs ROSES_MODELS_MANIFEST, ROSES_ARENA_MANIFEST,
    /// ROSES_TEXEFF.
    #[test]
    #[ignore = "needs private imports"]
    fn real_battle_answer_reaches_the_outcome_records() {
        let unit = |attack, defence| BattleUnit { attack, defence, defence_position: false };
        let survives = battle_state([unit(1000, 900), unit(1800, 1200)], true, (3, 3));
        let destroyed = battle_state([unit(2400, 900), unit(1800, 1200)], true, (3, 3));
        assert_eq!(survives.outcome(0x101, 0x001), 0);
        assert_eq!(destroyed.outcome(0x101, 0x001), 1);
        let run = |card: u16, state: Option<BattleState>| -> Vec<UnitWrites> {
            let (setup, mut inputs) = footage_inputs([card, 370]);
            inputs.battle = state;
            let battle = ArenaBattle::new(setup.units, Outcome::from_battle(1, 2000, 1000), setup.stage_end, 1);
            let mut real = battle.clone();
            let mut worker = EffectWorker::start(&battle, inputs);
            let mut writes = Vec::new();
            let mut n = 0;
            while !real.done() && n < 5000 {
                if let Some(d) = worker.tick(&mut real, Duration::from_secs(5)) {
                    if d.card == card {
                        writes.push(d.unit_writes[1]);
                    }
                }
                n += 1;
            }
            assert!(worker.stats.lock().unwrap().stops.is_empty(), "card {card}");
            assert!(!writes.is_empty(), "card {card}");
            writes
        };
        for card in OUTCOME_CARDS {
            let (none, lives, dies) = (run(card, None), run(card, Some(survives)), run(card, Some(destroyed)));
            let apart = lives.iter().zip(&dies).position(|(a, b)| a != b);
            eprintln!("card {card}: {} updates, survivor apart from update {apart:?}; last target writes {:?} / {:?}", dies.len(), lives.last().unwrap(), dies.last().unwrap());
            assert_eq!(dies, none, "card {card}: answer 1 as without a state");
            assert!(apart.is_some(), "card {card}: the surviving target took the destroyed branch");
        }
        assert_eq!(run(406, None), run(406, Some(survives)));
    }

    /// A skip press during B220 after the worker ran ahead: the worker
    /// rewinds (checkpoint copies) and its draws and returns through the exit
    /// equal a worker that skipped there without running ahead.
    #[test]
    #[ignore = "needs private imports"]
    fn skip_rewinds_the_worker() {
        const S: u32 = 150;
        let (setup, inputs) = footage_inputs([406, 370]);
        let battle = ArenaBattle::new(setup.units, Outcome::from_battle(2, 1200, 1400), setup.stage_end, 1);
        let run = |rewind: bool| -> Vec<(u8, u8, Option<EffectDraw>)> {
            let mut real = battle.clone();
            let mut worker = EffectWorker::start(&battle, inputs.clone());
            if !rewind {
                // The reference: the copy skips when it reaches S.
                *worker.skip.lock().unwrap() = Some((1, S));
            }
            let mut out = Vec::new();
            let mut n = 0u32;
            while !real.done() && n < 5000 {
                if rewind && n == S {
                    // Let the copy run ahead before the press.
                    std::thread::sleep(Duration::from_millis(400));
                }
                if n == S {
                    assert!(real.skip());
                    if rewind {
                        worker.skip();
                    } else {
                        worker.generation = 1;
                    }
                }
                let d = worker.tick(&mut real, Duration::from_secs(5));
                if n >= S - 2 {
                    out.push((real.phase, real.sub, d));
                }
                n += 1;
            }
            let s = worker.stats.lock().unwrap();
            assert!(s.stops.is_empty(), "{:?}", s.stops);
            assert_eq!(s.late, 0);
            eprintln!("rewind {rewind}: {} updates undone", s.rewound);
            assert_eq!(s.rewound > 0, rewind);
            out
        };
        let a = run(true);
        let b = run(false);
        assert_eq!(a.len(), b.len());
        let drawn = a.iter().filter(|x| x.2.as_ref().is_some_and(|d| !d.prims.is_empty())).count();
        eprintln!("{} updates from the press, {drawn} with effect primitives", a.len());
        assert!(drawn > 10);
        for (i, (x, y)) in a.iter().zip(&b).enumerate() {
            assert_eq!((x.0, x.1), (y.0, y.1), "phase at {i}");
            assert!(x.2 == y.2, "draw at update {}", S as usize - 2 + i);
        }
    }

    /// The footage battle live with the game's one rand stream started from
    /// the emulator's state: the state at each frame's start equals the
    /// emulator's (`emu_rand.py 28`: the 2 frame draws, the camera mirror,
    /// B220 / B125's own draws and the reaction holds) and so do the sound
    /// requests, with no shift (`emu_se.py 28`; the headless test needs a
    /// 4-update shift after Kumootoko's hold). Needs ROSES_RAND_TRACE /
    /// ROSES_SE_TRACE (the two jsons) besides the inputs.
    #[test]
    #[ignore = "needs private imports and emulator traces"]
    fn footage_battle_rand_and_sounds_match_the_emulator() {
        const LAUNCH_FRAME: u64 = 95641;
        const M: u64 = 0x5851_F42D_4C95_7F2D;
        let (setup, inputs) = footage_inputs([406, 370]);
        let mut battle = ArenaBattle::new(setup.units, Outcome::from_battle(2, 1200, 1400), setup.stage_end, 1);
        let launch = {
            let mut c = battle.clone();
            let mut n = 0u64;
            while c.effect_task().is_none() {
                c.tick();
                n += 1;
            }
            n
        };
        let trace: serde_json::Value = serde_json::from_slice(&std::fs::read(std::env::var("ROSES_RAND_TRACE").unwrap()).unwrap()).unwrap();
        // Each frame's state before its first draw (0x20A950's, ra 0x20A980).
        let mut frame_start = std::collections::BTreeMap::new();
        for e in trace["rand"].as_array().unwrap() {
            if e["ra"].as_u64() == Some(0x20A980) {
                frame_start.entry(e["frame"].as_u64().unwrap()).or_insert(e["state"].as_u64().unwrap());
            }
        }
        // Port update n (1-based) runs in frame LAUNCH_FRAME - launch + n;
        // back from the first traced frame at 2 draws per frame.
        let frame_of = |n: u64| LAUNCH_FRAME - launch + n;
        let (&f0, &s0) = frame_start.iter().next().unwrap();
        let mut inv = 1u64;
        for _ in 0..6 {
            inv = inv.wrapping_mul(2u64.wrapping_sub(M.wrapping_mul(inv)));
        }
        assert_eq!(M.wrapping_mul(inv), 1);
        let mut s = s0;
        for _ in 0..2 * (f0 - frame_of(1)) {
            s = s.wrapping_sub(1).wrapping_mul(inv);
        }
        battle.rand = s;
        let mut real = battle.clone();
        let mut worker = EffectWorker::start(&battle, inputs);
        let (mut compared, mut first_bad) = (0, None);
        let mut port_se = Vec::new();
        let mut n = 0u64;
        while !real.done() && n < 5000 {
            // c621 (the destroy flash) draws 1,848 + 24 per update from the
            // game's rand in the original; the port's c621 has its own
            // generator, so the streams part there.
            let c621 = real.destroy_effects().next().is_some();
            if let Some(&want) = frame_start.get(&frame_of(n + 1)).filter(|_| !c621) {
                compared += 1;
                if real.rand != want && first_bad.is_none() {
                    eprintln!("rand mismatch at update {}: port {:#x} emulator {:#x}", n + 1, real.rand, want);
                    first_bad = Some((n + 1, frame_of(n + 1), real.effect_task()));
                }
            }
            worker.tick(&mut real, Duration::from_secs(5));
            n += 1;
            if std::env::var_os("ROSES_RAND_UNITS").is_some() {
                eprintln!(
                    "tick {n} frame {}: phase {} sub {} task {:?} rand {:#x} units {:?}",
                    frame_of(n),
                    real.phase,
                    real.sub,
                    real.effect_task(),
                    real.rand,
                    real.units.iter().map(|u| (u.clock.motion, u.clock.time, u.clock.direction, u.frozen, u.blend.is_some())).collect::<Vec<_>>()
                );
            }
            for (slot, id) in real.sounds.drain(..) {
                port_se.push((n as u32, slot, id & 0x7FFF));
            }
        }
        let s = worker.stats.lock().unwrap().clone();
        eprintln!("launch {launch}; {compared} frame starts compared, first mismatch {first_bad:?}; late {}; stops {:?}", s.late, s.stops);
        let se: serde_json::Value = serde_json::from_slice(&std::fs::read(std::env::var("ROSES_SE_TRACE").unwrap()).unwrap()).unwrap();
        let emu: Vec<(u32, u8, u16)> = se["se"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| ((e["frame"].as_u64().unwrap() + launch - LAUNCH_FRAME) as u32, e["bank"].as_u64().unwrap() as u8, e["id"].as_u64().unwrap() as u16 & 0x7FFF))
            .take_while(|e| !(e.1 == 1 && e.2 == 0x41))
            .collect();
        for x in emu.iter().filter(|x| !port_se.contains(x)) {
            eprintln!("emulator only {x:?}");
        }
        for x in port_se.iter().filter(|x| !emu.contains(x)) {
            eprintln!("port only {x:?}");
        }
        eprintln!("sound requests: port {}, emulator {}", port_se.len(), emu.len());
        assert_eq!(s.late, 0);
        assert!(compared > 650);
        assert_eq!(first_bad, None);
        assert_eq!(port_se, emu);
    }
}

#[cfg(test)]
mod private_sweep {
    use super::private_tests::footage_inputs;
    use super::*;
    use roses_ee::bmodule::AttackEffect;
    use crate::duel_arena_battle::Outcome;
    use roses_fx::world::OBJECT_RECORD;
    use std::collections::{BTreeMap, BTreeSet};

    /// Object fields the mirror writes (start, end) by offset.
    const OBJECT_MIRRORED: &[(u32, u32)] = &[
        (0x00, 0x1C),
        (0x30, 0x40),
        (0x80, 0xA0),
        (0xA0, 0x130),
        (0x198, 0x1A0),
        (0x1B0, 0x1E2),
        (0x1E4, 0x1EC),
        (0x1F0, 0x200),
        (0x204, 0x208),
        (0x20C, 0x210),
    ];
    const CAMERA_OBJECT: u32 = 0x01FE_04E0;
    /// Globals the runner sets or answers (start, end).
    const GLOBALS_KNOWN: &[(u32, u32)] = &[
        (0x2D_E2C0, 0x2D_E410),
        (0x2D_F5A0, 0x2D_F5C4),
        (0x2E_9EA0, 0x2E_A600),
        (0x2E_D6A0, 0x2E_D6A4),
        (0x2E_D6C0, 0x2E_D6C4),
        (0x2E_D720, 0x2E_D8C0),
        (0x2E_9650, 0x2E_9654),
        (0x2E_9430, 0x2E_943C),
        (0x2E_9520, 0x2E_952C),
        (0x2E_960C, 0x2E_9614),
        (0x2E_9620, 0x2E_9630),
        (0x2E_9638, 0x2E_9640),
    ];

    #[derive(Debug, Default, Clone, serde::Serialize)]
    struct Row {
        card: u16,
        record: String,
        model: Option<u16>,
        status: String,
        calls: usize,
        cached_calls: Option<usize>,
        returns_equal_cached: Option<bool>,
        returns: String,
        prims: usize,
        max_prims: usize,
        kinds: BTreeMap<String, usize>,
        max_steps: u64,
        max_ms: f64,
        total_ms: f64,
        /// The worst mean over 8 consecutive calls (the worker's lookahead).
        worst8_ms: f64,
        /// Calls whose output differed with the native helpers off
        /// (ROSES_SWEEP_HLE_CHECK); first such call.
        hle_mismatches: usize,
        hle_first_mismatch: Option<usize>,
        stop: Option<String>,
        boundary: BTreeMap<String, usize>,
        missing_textures: BTreeSet<String>,
        unmapped_tbp: BTreeSet<String>,
        frame_texture_prims: usize,
        /// Primitives sampling a unit target (afterimages), unit renders.
        unit_rt_prims: usize,
        unit_renders: usize,
        /// Primitives linked into the ordering table / into packet layers.
        ot_prims: usize,
        layer_prims: usize,
        /// Lowest stage light 0x2DF5B2 seen, tint / scale changes seen.
        min_stage_light: u8,
        tinted: bool,
        scaled: bool,
        blends_other: BTreeSet<String>,
        buffered_prims: usize,
        object_reads: BTreeSet<String>,
        object_writes: BTreeSet<String>,
        camera_reads: BTreeSet<String>,
        global_reads: BTreeSet<String>,
        campaign: u32,
        starter: u32,
        /// Slice 3: bytes the module (or a helper) wrote at the packet
        /// pointer 0x2E9650 itself (0x206F30 / 0x206F90 are hooked and write
        /// nothing): sum and the update's maximum; GIF tags seen there.
        raw_packet_bytes: u64,
        max_raw_packet_bytes: u64,
        raw_gif: BTreeSet<String>,
        /// Point primitives (GS PRIM 0, descriptor or PRIM register).
        points: usize,
        /// CLAMP of textured primitives (WMS | WMT << 2) and how many.
        clamp_modes: BTreeMap<String, usize>,
        /// ST-textured primitives whose vertex Q differs (perspective).
        st_q_varies: usize,
        /// 0x1EF8F0 unit renders with a matrix argument.
        unit_render_matrices: usize,
        /// obj+0x130..0x133 seen other than 0 (the full-window colour).
        add_seen: BTreeSet<String>,
        /// (role, obj+0x1F8, +0x130..0x133, +0x128) of updates where the
        /// colour pass or a composite mode is on (+0x1F8 bits 0-3 not 0).
        pass_seen: BTreeSet<String>,
        /// Calls where the colour pass runs (+0x1F8 bits 2-3 and +0x133 not 0).
        pass_calls: usize,
        /// Frame-buffer primitives with a PSMCT32 TEX0 and the TEX0s seen.
        frame_word_prims: usize,
        frame_word_tex0: BTreeSet<String>,
    }

    fn in_ranges(v: u32, r: &[(u32, u32)]) -> bool {
        r.iter().any(|(a, b)| (*a..*b).contains(&v))
    }

    /// The 4 bytes at `addr` of the executable's loaded image.
    fn elf_image_word(elf: &[u8], addr: u32) -> Option<[u8; 4]> {
        let u32_at = |o: usize| elf.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        let phoff = u32_at(0x1C)? as usize;
        let size = usize::from(u16::from_le_bytes([elf[0x2A], elf[0x2B]]));
        let count = usize::from(u16::from_le_bytes([elf[0x2C], elf[0x2D]]));
        for i in 0..count {
            let h = phoff + i * size;
            let (kind, offset, vaddr, filesz, memsz) = (u32_at(h)?, u32_at(h + 4)? as usize, u32_at(h + 8)?, u32_at(h + 16)?, u32_at(h + 20)?);
            if kind == 1 && (vaddr..vaddr + memsz).contains(&addr) {
                let k = (addr - vaddr) as usize;
                return Some(std::array::from_fn(|j| if k + j < filesz as usize { elf[offset + k + j] } else { 0 }));
            }
        }
        Some([0; 4])
    }

    fn sweep_card(card: u16, defender: u16, provider: &crate::duel_arena::ArenaProvider, base: &EffectInputs, models: &std::path::Path, reference: Option<&[u8]>) -> Row {
        let mut row = Row { card, min_stage_light: 128, ..Default::default() };
        let elf = &base.program.1;
        let at = usize::from(card) * 0x8000;
        row.record = String::from_utf8_lossy(base.program.0[at + 0x20..at + 0x30].split(|b| *b == 0).next().unwrap_or(&[])).into_owned();
        let setup = match (provider.setup)(0, [card, defender]) {
            Ok(s) => s,
            Err(e) => {
                row.status = format!("no arena setup: {e}");
                return row;
            }
        };
        row.cached_calls = setup.units[0].effect.as_ref().map(Vec::len);
        let cached = setup.units[0].effect.clone();
        let own: Vec<Option<Vec<u8>>> = crate::duel_models::effect_textures(models, card).map(|t| t.into_iter().map(|t| t.map(|t| t.rh2)).collect()).unwrap_or_default();
        let mut battle = ArenaBattle::new(setup.units, Outcome::from_battle(1, 2000, 1000), setup.stage_end, 1);
        let mut model_cpu = roses_ee::Cpu::new();
        let _ = model_cpu.load_elf(elf);
        let models_ix = [card, defender].map(|c| roses_ee::effects::model_index(&mut model_cpu, c).unwrap_or(0));
        row.model = roses_ee::effects::model_index(&mut model_cpu, card);
        drop(model_cpu);
        let mut effect: Option<AttackEffect> = None;
        let mut plain: Option<AttackEffect> = None;
        let hle_check = std::env::var_os("ROSES_SWEEP_HLE_CHECK").is_some();
        let reads: Arc<Mutex<(BTreeSet<u32>, BTreeSet<u32>)>> = Default::default();
        let mut returns = Vec::new();
        let mut times: Vec<f64> = Vec::new();
        let mut dead = false;
        let mut started = false;
        let mut n = 0;
        while !battle.done() && n < 3000 {
            battle.tick_live(&mut |b: &ArenaBattle, role: usize, call: usize, _shared: &mut FxShared| {
                if role != 0 || dead {
                    return b.cached_return(role, call);
                }
                if call == 0 {
                    started = true;
                    let textures = EffectTextures { texeff: base.texeff.clone(), own: own.clone(), other: Vec::new() };
                    effect = match AttackEffect::load(&base.program.1, &base.program.0, card, [0x101, 0x001], &textures) {
                        Ok(Some(mut e)) if std::env::var_os("ROSES_SWEEP_NO_TRACE").is_some() => {
                            if std::env::var_os("ROSES_SWEEP_PROFILE").is_some() {
                                e.cpu.histogram = Some(vec![0; 1 << 23]);
                            }
                            Some(e)
                        }
                        Ok(Some(mut e)) => {
                            let r2 = reads.clone();
                            e.cpu.tracer = Some(Box::new(move |_pc, addr, store| {
                                if addr >> 28 == 7 {
                                    return;
                                }
                                let a = addr & 0x1FF_FFFF;
                                let interesting = (0x2D_0000..0x2F_0000).contains(&a)
                                    || OBJECT_RECORD.iter().any(|o| (*o..*o + 0x220).contains(&a))
                                    || (CAMERA_OBJECT..CAMERA_OBJECT + 0x100).contains(&a);
                                if interesting {
                                    let mut g = r2.lock().unwrap();
                                    if store {
                                        g.1.insert(a);
                                    } else if !g.1.contains(&a) {
                                        g.0.insert(a);
                                    }
                                }
                            }));
                            Some(e)
                        }
                        Ok(None) => None,
                        Err(e) => {
                            row.stop = Some(format!("load: {e}"));
                            None
                        }
                    };
                    if effect.is_none() {
                        dead = true;
                        return b.cached_return(role, call);
                    }
                    if hle_check {
                        plain = AttackEffect::load(&base.program.1, &base.program.0, card, [0x101, 0x001], &textures).ok().flatten();
                        if let Some(p) = plain.as_mut() {
                            p.hle = false;
                        }
                    }
                }
                let fx = effect.as_mut()?;
                let world = BattleWorld::new(b, 0, models_ix);
                let t = Instant::now();
                let result = fx.update(&world);
                // Raw packet output: the pointer's advance past the scratch.
                let end = fx.cpu.read_u32(0x002E_9650);
                if (0x01F4_0000..0x01F9_0000).contains(&end) && end > 0x01F4_0000 {
                    let bytes = u64::from(end - 0x01F4_0000);
                    row.raw_packet_bytes += bytes;
                    row.max_raw_packet_bytes = row.max_raw_packet_bytes.max(bytes);
                    // GIF tags (bit 15 EOP, 46 PRE, 47.. PRIM, 58 FLG, 60 NREG)
                    // or DMA tags, by their first qword.
                    let mut at = 0x01F4_0000u32;
                    let mut guard = 0;
                    while at + 16 <= end && guard < 64 {
                        guard += 1;
                        let lo = fx.cpu.read_u64(at);
                        let hi = fx.cpu.read_u64(at + 8);
                        let nloop = (lo & 0x7FFF) as u32;
                        let flg = (lo >> 58) & 3;
                        let nreg = { let n = (lo >> 60) as u32; if n == 0 { 16 } else { n } };
                        let prim = if lo >> 46 & 1 != 0 { format!("{:x}", (lo >> 47) & 0x7FF) } else { "-".into() };
                        row.raw_gif.insert(format!("flg {flg} nloop {nloop} nreg {nreg} prim {prim} regs {hi:x}"));
                        let qw = match flg {
                            0 => nloop * nreg,
                            1 => (nloop * nreg).div_ceil(2),
                            _ => nloop,
                        };
                        if std::env::var("ROSES_SWEEP_RAW_DUMP").ok().and_then(|v| v.parse::<u16>().ok()) == Some(card) && guard == 1 {
                            let words: Vec<String> = (0..((end - at) / 8).min(40)).map(|k| format!("{:016x}", fx.cpu.read_u64(at + 8 * k))).collect();
                            eprintln!("raw packet card {card} call {call} bytes {bytes}: {}", words.join(" "));
                        }
                        at += 16 * (1 + qw);
                    }
                }
                if let (Some(p), Ok(u)) = (plain.as_mut(), result.as_ref()) {
                    let q = p.update(&world);
                    let same = q.as_ref().is_ok_and(|q| q.ret == u.ret && q.prims == u.prims);
                    if !same && row.hle_mismatches == 0 {
                        if let Ok(q) = &q {
                            let diff = u.prims.iter().zip(&q.prims).find(|(a, b)| a != b).map(|(a, b)| {
                                let mut a2 = a.clone();
                                a2.tex0 = b.tex0;
                                a2.texture = b.texture;
                                a2.desc = b.desc;
                                let vd: Vec<String> = a.vertices.iter().zip(&b.vertices).filter(|(x, y)| x != y).take(2).map(|(x, y)| format!("{x:?} vs {y:?}")).collect();
                                format!("flags {:x}/{:x} tex0 {:x?}/{:x?} only-tex0 {} vertices {vd:?}", a.flags, b.flags, a.tex0, b.tex0, a2 == *b)
                            });
                            eprintln!("hle mismatch card {card} call {call}: ret {}/{} prims {}/{} first diff {diff:?}", u.ret, q.ret, u.prims.len(), q.prims.len());
                        }
                    }
                    if !same {
                        row.hle_mismatches += 1;
                        row.hle_first_mismatch.get_or_insert(call);
                    }
                }
                match result {
                    Ok(u) => {
                        let ms = t.elapsed().as_secs_f64() * 1e3;
                        row.calls += 1;
                        row.max_ms = row.max_ms.max(ms);
                        row.total_ms += ms;
                        times.push(ms);
                        row.max_steps = row.max_steps.max(u.steps);
                        row.prims += u.prims.len();
                        row.max_prims = row.max_prims.max(u.prims.len());
                        for b in &u.boundary {
                            *row.boundary.entry(b.1.to_string()).or_insert(0) += 1;
                        }
                        for m in &u.missing_textures {
                            row.missing_textures.insert(format!("{}/{}", m.0, m.1));
                        }
                        row.unit_renders += u.unit_renders.len();
                        row.unit_render_matrices += u.unit_renders.iter().filter(|r| r.pre != 0).count();
                        for (role, w) in u.unit_writes.iter().enumerate() {
                            if w.add != [0; 4] {
                                row.add_seen.insert(format!("{:?}", w.add));
                            }
                            if w.mode & 0xF != 0 {
                                row.pass_seen.insert(format!("{role} {:#x} {:?} {}", w.mode, w.add, w.fade));
                                if std::env::var_os("ROSES_SWEEP_UNIT_LOG").is_some() {
                                    eprintln!("unit card {card} call {call} role {role} mode {:#x} add {:?} tint {:?} fade {}", w.mode, w.add, w.tint, w.fade);
                                }
                            }
                            row.pass_calls += usize::from((w.mode >> 2) & 3 != 0 && w.add[3] != 0);
                        }
                        row.min_stage_light = row.min_stage_light.min(u.stage_light);
                        for w in &u.unit_writes {
                            row.tinted |= w.tint[..3] != [0x80; 3];
                            row.scaled |= w.scale[..3] != [1.0; 3];
                        }
                        // ROSES_SWEEP_PRIM_DUMP = "card,call": that update's primitives.
                        if std::env::var("ROSES_SWEEP_PRIM_DUMP").ok().is_some_and(|v| v == format!("{card},{call}")) {
                            for p in &u.prims {
                                eprintln!(
                                    "prim desc {:x} flags {:x} kind {} n {} tex0 {:x?} tex1 {:x?} clamp {:x?} alpha {:x?} texture {:?} first {:?}",
                                    p.desc, p.flags, p.kind, p.vertices.len(), p.tex0, p.tex1, p.clamp, p.alpha, p.texture, p.vertices.first()
                                );
                            }
                        }
                        for p in &u.prims {
                            *row.kinds.entry(format!("{:x}", p.flags)).or_insert(0) += 1;
                            row.ot_prims += usize::from(p.ot_key.is_some());
                            row.points += usize::from(p.kind == 0);
                            if p.textured() {
                                if let Some(c) = p.clamp {
                                    *row.clamp_modes.entry(format!("{:x}", c & 3 | ((c >> 2) & 3) << 2)).or_insert(0) += 1;
                                } else {
                                    *row.clamp_modes.entry("none".into()).or_insert(0) += 1;
                                }
                                if !p.uv_texels() && p.vertices.windows(2).any(|w| (w[0].q - w[1].q).abs() > 0.01 * w[0].q.abs().max(1e-6)) {
                                    row.st_q_varies += 1;
                                }
                            }
                            row.layer_prims += usize::from(p.layer.is_some());
                            row.unit_rt_prims += usize::from(matches!(p.texture, Some(roses_fx::types::TextureKey::UnitRt(_))));
                            if p.buffered {
                                row.buffered_prims += 1;
                            }
                            if let Some(t) = p.tex0.filter(|_| p.textured()) {
                                let tbp = t & 0x3FFF;
                                if matches!(tbp, 0 | 0x800) {
                                    row.frame_texture_prims += 1;
                                    // TEX0 PSMCT32 on the frame buffer: the low-RAM
                                    // TEX0 of a missing type-5 texture.
                                    if (t >> 20) & 0x3F == 0 {
                                        row.frame_word_prims += 1;
                                        row.frame_word_tex0.insert(format!("{t:x}"));
                                    }
                                } else if p.texture.is_none() {
                                    row.unmapped_tbp.insert(format!("{tbp:x}"));
                                }
                            }
                            if let Some(a) = p.alpha.filter(|_| p.blended()) {
                                if !matches!(a & 0xFF, 0x48 | 0x42 | 0x44 | 0x68 | 0x62 | 0x64 | 0x88) {
                                    row.blends_other.insert(format!("{:x}", a & 0xFF));
                                }
                            }
                        }
                        returns.push(u.ret);
                        Some(u.ret)
                    }
                    Err(e) => {
                        row.stop = Some(format!("call {call}: {e}"));
                        dead = true;
                        b.cached_return(role, call)
                    }
                }
            });
            n += 1;
            if started && battle.effect_task().is_none() {
                break;
            }
        }
        row.status = if !started {
            "no attack effect launched".into()
        } else if row.stop.is_some() {
            "stopped".into()
        } else if effect.is_none() {
            "no program".into()
        } else {
            "ran".into()
        };
        if let Some(fx) = effect.as_mut() {
            if let Some(h) = fx.cpu.histogram.take() {
                let mut starts = std::collections::BTreeSet::new();
                for range in [(0x10_0000u32, 0x22_0000u32), (0x1E0_0000, 0x1E0_8000)] {
                    for at in (range.0..range.1).step_by(4) {
                        let w = fx.cpu.read_u32(at);
                        if w >> 26 == 3 {
                            starts.insert((w & 0x3FF_FFFF) << 2);
                        }
                    }
                }
                let mut per: BTreeMap<u32, u64> = BTreeMap::new();
                for (i, n) in h.iter().enumerate() {
                    if *n > 0 {
                        let pc = (i as u32) << 2;
                        *per.entry(starts.range(..=pc).next_back().copied().unwrap_or(0)).or_insert(0) += u64::from(*n);
                    }
                }
                let mut v: Vec<_> = per.into_iter().collect();
                v.sort_by(|a, b| b.1.cmp(&a.1));
                let sum: u64 = v.iter().map(|x| x.1).sum();
                eprintln!("profile card {card}: {}", v.iter().take(16).map(|(f, n)| format!("{f:x} {:.1}%", 100.0 * *n as f64 / sum as f64)).collect::<Vec<_>>().join(", "));
            }
        }
        row.returns_equal_cached = cached.as_ref().map(|c| *c == returns);
        row.worst8_ms = times.windows(8).map(|w| w.iter().sum::<f64>() / 8.0).fold(0.0, f64::max);
        let mut counts: BTreeMap<i32, usize> = BTreeMap::new();
        for r in &returns {
            *counts.entry(*r).or_insert(0) += 1;
        }
        row.returns = format!("{counts:?}");
        let g = reads.lock().unwrap();
        for a in &g.0 {
            if let Some(o) = OBJECT_RECORD.iter().position(|o| (*o..*o + 0x220).contains(a)) {
                let off = a - OBJECT_RECORD[o];
                if !in_ranges(off, OBJECT_MIRRORED) {
                    row.object_reads.insert(format!("{}+{off:x}", ["self", "target"][o]));
                }
            } else if (CAMERA_OBJECT..CAMERA_OBJECT + 0x100).contains(a) {
                if a - CAMERA_OBJECT >= 0x70 {
                    row.camera_reads.insert(format!("+{:x}", a - CAMERA_OBJECT));
                }
            } else if !in_ranges(*a, GLOBALS_KNOWN) {
                // Runtime state only: where the footage RAM differs from the ELF image.
                let differs = reference.is_none_or(|r| {
                    let i = *a as usize;
                    r.get(i..i + 4) != elf_image_word(elf, *a).as_ref().map(|w| &w[..])
                });
                if differs {
                    row.global_reads.insert(format!("{a:x}"));
                }
            }
        }
        for a in &g.1 {
            if let Some(o) = OBJECT_RECORD.iter().position(|o| (*o..*o + 0x220).contains(a)) {
                let off = a - OBJECT_RECORD[o];
                if !matches!(off, 0x1B0..=0x1BF | 0x1F9) {
                    row.object_writes.insert(format!("{}+{off:x}", ["self", "target"][o]));
                }
            }
        }
        row
    }

    /// Every BMODULE record run live in an arena battle against Orion (370;
    /// the defender is destroyed, so only the attacker's effect runs):
    /// interpreter stops, unhooked boundary calls, unmirrored state reads,
    /// textures, blends, cost. ROSES_SWEEP_OUT: JSON output;
    /// ROSES_SWEEP_REF_RAM: a footage EE image (global reads are kept only
    /// where it differs from the ELF image); ROSES_SWEEP_CARDS = "a-b";
    /// ROSES_DECK_FREQ: deck counts per card (JSON).
    #[test]
    #[ignore = "research sweep; needs private imports"]
    fn sweep_all_attack_effects() {
        let (_, base) = footage_inputs([406, 370]);
        let models = std::path::PathBuf::from(std::env::var("ROSES_MODELS_MANIFEST").unwrap());
        let arena = crate::duel_arena::load_arena(std::path::Path::new(&std::env::var("ROSES_ARENA_MANIFEST").unwrap())).unwrap();
        let provider = Arc::new(crate::duel_arena::arena_provider(&arena, models.clone()).unwrap());
        let reference = Arc::new(std::env::var("ROSES_SWEEP_REF_RAM").ok().map(|p| std::fs::read(p).unwrap()));
        let freq: BTreeMap<u16, (u32, u32)> = std::env::var("ROSES_DECK_FREQ")
            .ok()
            .map(|p| {
                let v: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
                v.iter().map(|e| (e["card"].as_u64().unwrap() as u16, (e["campaign"].as_u64().unwrap() as u32, e["starter"].as_u64().unwrap() as u32))).collect()
            })
            .unwrap_or_default();
        let (lo, hi) = std::env::var("ROSES_SWEEP_CARDS")
            .ok()
            .and_then(|r| r.split_once('-').map(|(a, b)| (a.parse().unwrap(), b.parse().unwrap())))
            .unwrap_or((0u16, 682u16));
        // ROSES_SWEEP_LIST = "a,b,c": only these cards.
        let list: Option<Vec<u16>> = std::env::var("ROSES_SWEEP_LIST").ok().map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect());
        let threads: usize = std::env::var("ROSES_SWEEP_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or(8);
        let next = Arc::new(std::sync::atomic::AtomicU16::new(lo));
        let rows = Arc::new(Mutex::new(Vec::new()));
        let base = Arc::new(base);
        let start = Instant::now();
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                let (next, rows, base, provider, models, reference, list) = (next.clone(), rows.clone(), base.clone(), provider.clone(), models.clone(), reference.clone(), list.clone());
                std::thread::spawn(move || loop {
                    let card = next.fetch_add(1, Ordering::Relaxed);
                    if card > hi {
                        break;
                    }
                    if list.as_ref().is_some_and(|l: &Vec<u16>| !l.contains(&card)) {
                        continue;
                    }
                    let at = usize::from(card) * 0x8000;
                    if base.program.0.get(at..at + 4) != Some(b"MWo3") {
                        continue;
                    }
                    let defender = if card == 370 { 406 } else { 370 };
                    let row = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sweep_card(card, defender, &provider, &base, &models, reference.as_deref().map(|v| &v[..]))))
                        .unwrap_or_else(|_| Row { card, status: "panicked".into(), ..Default::default() });
                    eprintln!(
                        "card {card} {} {}: {} calls, {} prims, max {:.1} ms{}",
                        row.record,
                        row.status,
                        row.calls,
                        row.prims,
                        row.max_ms,
                        row.stop.as_deref().map(|s| format!(", STOP {s}")).unwrap_or_default()
                    );
                    rows.lock().unwrap().push(row);
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        let mut rows = rows.lock().unwrap().clone();
        for r in rows.iter_mut() {
            let (c, s) = freq.get(&r.card).copied().unwrap_or((0, 0));
            r.campaign = c;
            r.starter = s;
        }
        rows.sort_by_key(|r| (std::cmp::Reverse(r.campaign + r.starter), r.card));
        eprintln!("{} records in {:?}", rows.len(), start.elapsed());
        if let Ok(out) = std::env::var("ROSES_SWEEP_OUT") {
            std::fs::write(out, serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
        }
    }
}
