//! Video aid (a port addition, never part of the 2003 game): an online
//! seat played through its pad by one of the game's own ported computer
//! opponents (`roses_core::duel::ai::weevil`, e.g. Yugi's or Seto's
//! module), at an optional human pace, for recorded online duels.
//!
//! - [`Pacer`]: how long a person waits before each press. Deterministic:
//!   the waits come from a seed (splitmix64), one draw per press, so a
//!   rehearsal with the same seed, decks and match seed repeats exactly.
//!   The waits are counted in 25 Hz updates while the seat may press (an
//!   animation or a server round trip does not eat the thinking time), and
//!   one turn's waits are capped ([`TURN_BUDGET`]) so a paced turn stays
//!   well inside the server's turn clock (120 s by default).
//! - [`VideoBot`]: plans with the module of a control byte on the seat's
//!   copy of the duel (its view: the other side's hidden cards are
//!   placeholders), turns each plan into the original's key sequence
//!   (`ai::keys`, as the offline computer seat does) and presses one key
//!   per update through the same input layer a person uses. Without a
//!   module (`pad`) it is the plain [`PadBot`].
//!
//! - N-player duels (MODES, a port addition): the module plans on the
//!   2-side projection the server's computer players use
//!   (`roses_core::duel::multi::computer::project`: the seat against one
//!   chosen enemy, the *focus*, every other unit a wall), the seat first
//!   aims its Target at the focus (as `MultiComputer` does), and the plan's
//!   keys are the seat's own: the east and west seats' board d-pad is
//!   turned a quarter ([`seat_keys`]). A refused plan ends the turn (the
//!   plain pad bot is a 2-player bot).
//!
//! Configured by `--bot-pace off|human`, `--bot-seed N`, `--bot-ai
//! pad|yugi|seto|<control byte>`, `--bot-tier N`, `--bot-log <file>` on the
//! headless `--online-bot` client, or the same as `ROSES_BOT_PACE`,
//! `ROSES_BOT_SEED`, `ROSES_BOT_AI`, `ROSES_BOT_TIER`, `ROSES_BOT_LOG` for
//! the connected game's `ROSES_ONLINE_PADBOT=1` seat and the menu script.

use crate::duel_online::PadBot;
#[cfg(not(feature = "online"))]
use crate::duel_online::PlayerView;
use roses_core::UnitId;
use roses_core::duel::ai::{self, Action, pad, weevil};
use roses_core::duel::ui::{DuelUi, Mode};
use roses_core::duel::{CardData, Catalogue, Duel};
#[cfg(feature = "online")]
use roses_net::PlayerView;
use std::collections::VecDeque;
use std::io::Write;

/// How the seat paces its presses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pace {
    /// One press per update as soon as the seat may press (the old bot).
    #[default]
    Off,
    /// A person's pace (module docs).
    Human,
}

impl Pace {
    pub fn parse(text: &str) -> Result<Self, String> {
        match text {
            "off" | "fast" | "0" => Ok(Self::Off),
            "human" | "on" | "1" => Ok(Self::Human),
            _ => Err(format!("the bot pace is off or human, not {text:?}")),
        }
    }
}

/// What a press is, for its wait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Moment {
    /// A cursor step on the board (0.6-1.2 s).
    Cursor,
    /// A held direction's repeat (deck builder, long lists: 0.12-0.2 s).
    Repeat,
    /// A menu key: hand scrolling, Square, R1/R2, Circle (0.25-0.5 s).
    Menu,
    /// Marking a hand card (0.5-0.9 s).
    Mark,
    /// Confirming (1.5-2.5 s).
    Confirm,
    /// Confirming a fusion, a big summon, an attack or a magic/trap
    /// activation (3-4.5 s).
    Big,
    /// Ending the turn (1-1.6 s).
    EndTurn,
    /// The first press of a turn (1.5-2.5 s on top of the press's own).
    Think,
    /// One typed character (0.1-0.2 s).
    Type,
    /// A menu screen's cursor step (0.35-0.6 s).
    MenuMove,
    /// A menu screen's confirm (0.8-1.4 s).
    MenuConfirm,
}

/// Updates (25 Hz) a moment waits: the inclusive range.
pub fn wait_range(m: Moment) -> (u32, u32) {
    match m {
        Moment::Cursor => (15, 30),
        Moment::Repeat => (3, 5),
        Moment::Menu => (6, 12),
        Moment::Mark => (12, 22),
        Moment::Confirm => (38, 62),
        Moment::Big => (75, 112),
        Moment::EndTurn => (25, 40),
        Moment::Think => (38, 62),
        Moment::Type => (3, 5),
        Moment::MenuMove => (9, 15),
        Moment::MenuConfirm => (20, 35),
    }
}

/// The most one turn's waits add up to (45 s at 25 Hz): later presses of a
/// long turn wait [`MIN_WAIT`] only, so the paced turn plus its animations
/// stays inside the server's 120 s turn clock, with room for a recording
/// that runs a little slower than the wall clock.
pub const TURN_BUDGET: u32 = 45 * 25;
pub const MIN_WAIT: u32 = 4;

/// The deterministic waits.
#[derive(Clone, Debug)]
pub struct Pacer {
    pub pace: Pace,
    rng: u64,
    /// The wait of the press being held back (None: none assigned yet).
    wait: Option<u32>,
    /// Waits assigned this turn (see [`TURN_BUDGET`]).
    spent: u32,
    /// Updates waited in total (the paced time the bot added).
    pub waited: u64,
    /// When this turn began on the wall clock (the server's turn clock is
    /// wall time; see [`Pacer::delay`]).
    turn_started: std::time::Instant,
}

/// Wall seconds into a turn after which waits shrink, and after which
/// they are the minimum ([`Pacer::delay`]).
pub const HURRY_AFTER_S: f32 = 40.0;
pub const RUSH_AFTER_S: f32 = 60.0;

impl Pacer {
    pub fn new(pace: Pace, seed: u64) -> Self {
        Self {
            pace,
            rng: seed ^ 0x5EED_0F_D07D,
            wait: None,
            spent: 0,
            waited: 0,
            turn_started: std::time::Instant::now(),
        }
    }

    fn next(&mut self) -> u64 {
        // splitmix64
        self.rng = self.rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A draw in `lo..=hi`.
    pub fn draw(&mut self, lo: u32, hi: u32) -> u32 {
        lo + (self.next() % u64::from(hi - lo + 1)) as u32
    }

    /// A new turn: its waits start from nothing.
    pub fn new_turn(&mut self) {
        self.spent = 0;
        self.wait = None;
        self.turn_started = std::time::Instant::now();
    }

    /// The wait for a press of this moment (0 when the pace is off).
    pub fn delay(&mut self, m: Moment) -> u32 {
        if self.pace == Pace::Off {
            return 0;
        }
        let (lo, hi) = wait_range(m);
        let mut d = self.draw(lo, hi);
        if self.spent >= TURN_BUDGET {
            d = MIN_WAIT.min(d);
        }
        // The turn clock runs on the wall clock: a turn whose animations (or
        // a recording slower than real time) took long hurries, so it never
        // reaches the server's limit. Only the waits change; the moves, and
        // so the duel, stay the same.
        let wall = self.turn_started.elapsed().as_secs_f32();
        if wall >= RUSH_AFTER_S {
            d = MIN_WAIT.min(d);
        } else if wall >= HURRY_AFTER_S {
            d = (d * 2 / 5).max(MIN_WAIT.min(d));
        }
        self.spent += d;
        d
    }

    /// Call once per update while a press of moment `m` is ready: true when
    /// it is pressed now (its wait has passed).
    pub fn ready(&mut self, m: Moment) -> bool {
        if self.pace == Pace::Off {
            return true;
        }
        let w = match self.wait {
            Some(w) => w,
            None => self.delay(m),
        };
        if w == 0 {
            self.wait = None;
            true
        } else {
            self.wait = Some(w - 1);
            self.waited += 1;
            false
        }
    }

    /// Words of a scripted pad (one per update, 0 = no press) with each
    /// press's wait put before it as empty updates. A direction pressed
    /// again right after the same direction is a held key's repeat.
    pub fn pace_words(&mut self, words: &[u16]) -> Vec<u16> {
        if self.pace == Pace::Off {
            return words.to_vec();
        }
        let mut out = Vec::with_capacity(words.len() * 8);
        let mut last = 0u16;
        let mut run = 0u32;
        for &w in words {
            if w == 0 {
                out.push(0);
                continue;
            }
            let dpad = w & (pad::UP | pad::DOWN | pad::LEFT | pad::RIGHT) != 0;
            run = if dpad && w == last { run + 1 } else { 0 };
            let m = if dpad && run >= 2 {
                Moment::Repeat
            } else if dpad {
                Moment::MenuMove
            } else if w & (pad::CROSS | pad::START) != 0 {
                Moment::MenuConfirm
            } else {
                Moment::Menu
            };
            let d = self.delay(m);
            out.extend(std::iter::repeat_n(0, d as usize));
            out.push(w);
            last = w;
        }
        out
    }
}

/// The bot's configuration (flags or environment).
#[derive(Clone, Debug, Default)]
pub struct BotConfig {
    pub pace: Pace,
    pub seed: u64,
    /// The planning module's control byte (None: the plain pad bot).
    pub controller: Option<u8>,
    pub tier: i16,
    pub log: Option<std::path::PathBuf>,
    /// N-player video aid (`ROSES_BOT_EXODIA=1`, for an Exodia deck): the
    /// limbs come first ([`exodia_assist`]); the module plays the rest.
    pub exodia: bool,
}

/// A control byte by a name or number.
pub fn controller_of(text: &str) -> Result<Option<u8>, String> {
    Ok(match text.to_ascii_lowercase().as_str() {
        "pad" | "simple" | "" => None,
        "seto" | "kaiba" => Some(1),
        "yugi" => Some(19),
        "weevil" => Some(2),
        "pegasus" => Some(9),
        "joey" => Some(15),
        "vs" | "custom" => Some(22),
        n => {
            let c: u8 = n
                .parse()
                .map_err(|_| format!("--bot-ai is pad, yugi, seto or a control byte, not {text:?}"))?;
            if !weevil::is_ported(c) {
                return Err(format!("control byte {c} has no ported module"));
            }
            Some(c)
        }
    })
}

impl BotConfig {
    /// `ROSES_BOT_PACE`, `ROSES_BOT_SEED`, `ROSES_BOT_AI`, `ROSES_BOT_TIER`,
    /// `ROSES_BOT_LOG`.
    pub fn from_env() -> Self {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        let mut c = Self {
            tier: 8,
            ..Self::default()
        };
        if let Some(p) = var("ROSES_BOT_PACE") {
            c.pace = Pace::parse(p.trim()).unwrap_or_else(|e| {
                eprintln!("Video bot: {e}; pace off");
                Pace::Off
            });
        }
        if let Some(s) = var("ROSES_BOT_SEED").and_then(|s| s.trim().parse().ok()) {
            c.seed = s;
        }
        if let Some(a) = var("ROSES_BOT_AI") {
            c.controller = controller_of(a.trim()).unwrap_or_else(|e| {
                eprintln!("Video bot: {e}; the plain pad bot plays");
                None
            });
        }
        if let Some(t) = var("ROSES_BOT_TIER").and_then(|s| s.trim().parse().ok()) {
            c.tier = t;
        }
        c.log = var("ROSES_BOT_LOG").map(Into::into);
        c.exodia = var("ROSES_BOT_EXODIA").is_some_and(|v| v.trim() == "1");
        c
    }

    /// Whether anything asks for this bot (otherwise the plain pad bot).
    pub fn wanted() -> bool {
        ["ROSES_BOT_PACE", "ROSES_BOT_AI"].iter().any(|k| std::env::var_os(k).is_some())
    }
}

/// What the bot did (for the seed search and the rehearsal's report).
#[derive(Clone, Debug, Default)]
pub struct BotStats {
    pub presses: u64,
    pub plans: u64,
    pub plan_errors: u64,
    pub fallback_presses: u64,
    pub watchdog: u64,
    pub fusions: u64,
    pub attacks: u64,
    pub activations: u64,
}

/// The seat's player (module docs).
pub struct VideoBot {
    pub config: BotConfig,
    pub pacer: Pacer,
    fallback: PadBot,
    memory: Option<(u8, weevil::Memory)>,
    plan_rng: u64,
    queue: VecDeque<(u16, Moment)>,
    turn: Option<(u8, u8)>,
    /// The module failed this turn: the pad bot plays the rest of it.
    fallback_turn: Option<(u8, u8)>,
    calls: usize,
    skip: Vec<UnitId>,
    think: bool,
    pub stats: BotStats,
    /// Updates seen (the bot's clock: 25 per second in a live window).
    pub updates: u64,
    log: Option<std::io::BufWriter<std::fs::File>>,
    observed: Option<Observed>,
    /// The last big moment pressed (for key frames): (update, label).
    pub last_big: Option<(u64, String)>,
    /// N-player: this turn's focus (the enemy the module plans against).
    focus: Option<u8>,
    /// N-player: the module's memory and the (side, projected side,
    /// focus) it belongs to.
    #[cfg_attr(not(feature = "online"), allow(dead_code))]
    multi_memory: Option<((u8, u8, u8), weevil::Memory)>,
    /// N-player: Target presses this turn (a few at most).
    targets_asked: u32,
    /// N-player: the Target key is due ([`VideoBot::take_target`]).
    want_target: bool,
    /// The Exodia aid played a hand card on this turn ([`exodia_cycle`]).
    #[cfg_attr(not(feature = "online"), allow(dead_code))]
    cycled: Option<(u8, u8)>,
}

/// A queued "press" that is the N-player Target key (keyboard T; the
/// caller sets `MultiPlay::target_key`), not a pad word.
const TARGET_PRESS: u16 = 0;

#[derive(Clone, Debug, PartialEq)]
struct Observed {
    revision: u64,
    round: u8,
    active: u8,
    lp: Vec<u16>,
    /// N-player: the sides knocked out.
    out: Vec<bool>,
    units: Vec<(u16, u8, u16, bool)>,
    outcome: Option<String>,
}

impl VideoBot {
    pub fn new(config: BotConfig) -> Self {
        let log = config.log.as_ref().and_then(|p| match std::fs::File::create(p) {
            Ok(f) => Some(std::io::BufWriter::new(f)),
            Err(e) => {
                eprintln!("Video bot: cannot write {}: {e}", p.display());
                None
            }
        });
        eprintln!(
            "Video bot: pace {:?}, seed {}, module {}, tier {}",
            config.pace,
            config.seed,
            config.controller.map_or("pad bot".to_string(), |c| format!("control byte {c}")),
            config.tier
        );
        Self {
            pacer: Pacer::new(config.pace, config.seed),
            plan_rng: config.seed.wrapping_mul(0x2545_F491_4F6C_DD1D) | 1,
            config,
            fallback: PadBot::new(),
            memory: None,
            queue: VecDeque::new(),
            turn: None,
            fallback_turn: None,
            calls: 0,
            skip: Vec::new(),
            think: false,
            stats: BotStats::default(),
            updates: 0,
            log,
            observed: None,
            last_big: None,
            focus: None,
            multi_memory: None,
            targets_asked: 0,
            want_target: false,
            cycled: None,
        }
    }

    /// N-player: whether the Target key is to be pressed now (the caller
    /// sets `MultiPlay::target_key` for the next update); taken once.
    pub fn take_target(&mut self) -> bool {
        std::mem::take(&mut self.want_target)
    }

    pub fn from_env() -> Self {
        Self::new(BotConfig::from_env())
    }

    fn note(&mut self, line: serde_json::Value) {
        if let Some(log) = self.log.as_mut() {
            let _ = writeln!(log, "{line}");
            let _ = log.flush();
        }
    }

    /// Once per update with what the screen shows (the presented copy and
    /// its revision): logs the changes a timeline needs.
    pub fn observe(&mut self, shown_revision: u64, duel: &Duel, names: &[String]) {
        let now = Observed {
            revision: shown_revision,
            round: duel.round,
            active: duel.active,
            lp: duel.sides.iter().map(|s| s.lp).collect(),
            out: (0..duel.side_count()).map(|s| duel.is_out(s)).collect(),
            units: duel.units.iter().map(|u| (u.id.0, u.side, u.card, u.face_up)).collect(),
            outcome: duel.outcome.map(|o| format!("{o:?}")),
        };
        if self.observed.as_ref() == Some(&now) {
            return;
        }
        let before = self.observed.replace(now.clone());
        let name = |c: u16| names.get(usize::from(c)).cloned().unwrap_or_else(|| format!("card {c}"));
        let mut notes: Vec<String> = Vec::new();
        if let Some(b) = &before {
            if (b.round, b.active) != (now.round, now.active) {
                notes.push(format!("round {} side {} to act", now.round, now.active));
            }
            for s in 0..now.lp.len().min(b.lp.len()) {
                if b.lp[s] != now.lp[s] {
                    notes.push(format!("side {s} LP {} -> {}", b.lp[s], now.lp[s]));
                }
            }
            for s in 0..now.out.len().min(b.out.len()) {
                if now.out[s] && !b.out[s] {
                    notes.push(format!("side {s} knocked out"));
                }
            }
            for u in &now.units {
                match b.units.iter().find(|v| v.0 == u.0) {
                    None => notes.push(format!(
                        "side {} new unit {}",
                        u.1,
                        if u.3 { name(u.2) } else { "(face down)".into() }
                    )),
                    Some(v) if !v.3 && u.3 => notes.push(format!("side {} revealed {}", u.1, name(u.2))),
                    _ => {}
                }
            }
            for v in &b.units {
                if !now.units.iter().any(|u| u.0 == v.0) {
                    notes.push(format!(
                        "side {} lost {}",
                        v.1,
                        if v.3 { name(v.2) } else { "a face-down card".into() }
                    ));
                }
            }
            if b.outcome != now.outcome {
                notes.push(format!("outcome {:?}", now.outcome));
            }
        }
        if !notes.is_empty() || before.is_none() {
            let line = serde_json::json!({
                "update": self.updates, "revision": now.revision, "round": now.round, "active": now.active,
                "lp": now.lp, "notes": notes,
            });
            self.note(line);
        }
    }

    /// The key for this update (0 = none). `can_press`: the client reads
    /// the pad now (its turn, nothing in flight, nothing animating).
    #[allow(clippy::too_many_arguments)]
    pub fn key(
        &mut self,
        duel: &Duel,
        input: &DuelUi,
        view: &PlayerView,
        revision: u64,
        can_press: bool,
        cards: &Catalogue,
        fusions: &impl weevil::FusionData,
    ) -> u16 {
        self.updates += 1;
        if !can_press {
            return 0;
        }
        let turn = (duel.round, duel.active);
        if self.turn != Some(turn) {
            self.turn = Some(turn);
            self.queue.clear();
            self.calls = 0;
            self.skip.clear();
            self.pacer.new_turn();
            self.think = true;
            self.focus = None;
            self.targets_asked = 0;
        }
        if self.think {
            if !self.pacer.ready(Moment::Think) {
                return 0;
            }
            self.think = false;
        }
        let modular = self.config.controller.is_some() && self.fallback_turn != Some(turn);
        if !modular && duel.multi().is_some() {
            // N-player: the plain pad bot plays 2-player boards only; a
            // turn the module refused ends.
            if !self.pacer.ready(Moment::EndTurn) {
                return 0;
            }
            self.stats.presses += 1;
            self.stats.fallback_presses += 1;
            return pad::START;
        }
        if !modular {
            // The plain pad bot, paced: wait, then take its key.
            if !self.pacer.ready(Moment::Cursor) {
                return 0;
            }
            let k = self.fallback.key(duel, input, view, revision, true, cards);
            if k != 0 {
                self.stats.presses += 1;
                self.stats.fallback_presses += 1;
            }
            return k;
        }
        if self.queue.is_empty() {
            match self.plan(duel, input, cards, fusions) {
                Some(()) => {}
                None => return 0,
            }
        }
        let Some(&(k, m)) = self.queue.front() else { return 0 };
        if !self.pacer.ready(m) {
            return 0;
        }
        self.queue.pop_front();
        if k == TARGET_PRESS {
            self.want_target = true;
            return 0;
        }
        self.stats.presses += 1;
        if m == Moment::Big {
            self.last_big = Some((self.updates, "big".into()));
        }
        k
    }

    /// Plan the next action into the queue; None = nothing this update.
    fn plan(&mut self, duel: &Duel, input: &DuelUi, cards: &Catalogue, fusions: &impl weevil::FusionData) -> Option<()> {
        let side = duel.active;
        let s = usize::from(side & 1);
        // Whatever the input layer has open when no plan is running is
        // closed first (a plan's keys always start on the field).
        if !matches!(input.mode, Mode::Field)
            || input.details.is_some()
            || input.surrender.is_some()
            || input.settings.is_some()
            || input.hand_view.is_some()
        {
            self.queue.push_back((pad::CIRCLE, Moment::Menu));
            return Some(());
        }
        self.calls += 1;
        if self.calls > 602 {
            self.stats.watchdog += 1;
            self.queue.push_back((pad::START, Moment::EndTurn));
            return Some(());
        }
        if self.calls >= 600 {
            self.queue.push_back((pad::CIRCLE, Moment::Menu));
            return Some(());
        }
        #[cfg(feature = "online")]
        if duel.multi().is_some() {
            return self.plan_multi(duel, input, cards, fusions);
        }
        let mut d = duel.clone();
        d.rng = self.plan_rng;
        for u in d.units.iter_mut() {
            if self.skip.contains(&u.id) {
                u.acted = true;
            }
        }
        if self.memory.as_ref().is_none_or(|(ms, _)| *ms != side) {
            self.memory = Some((side, weevil::Memory::new(&d, side)));
        }
        let controller = self.config.controller.unwrap_or(22);
        let (_, memory) = self.memory.as_mut().expect("set above");
        weevil::sync(memory, &d, side);
        self.stats.plans += 1;
        let plan = weevil::plan_for_rng(controller, &d, side, memory, cards, fusions, self.config.tier);
        match plan {
            Err(e) => {
                self.stats.plan_errors += 1;
                eprintln!("Video bot: the module refused a plan ({e}); the pad bot plays the rest of this turn");
                let line = serde_json::json!({"update": self.updates, "plan_error": e.to_string()});
                self.note(line);
                self.fallback_turn = self.turn;
                None
            }
            Ok((plan, rng)) => {
                self.plan_rng = rng;
                match plan {
                    weevil::Plan::EndTurn => {
                        self.queue.push_back((pad::START, Moment::EndTurn));
                        Some(())
                    }
                    weevil::Plan::NoKey => None,
                    weevil::Plan::MarkActed(id) => {
                        self.skip.push(id);
                        None
                    }
                    weevil::Plan::Act(action) => {
                        let keys = ai::keys(&d, side, input.cursors[s], &action, cards);
                        if keys.is_empty() {
                            if let Action::Act { unit, .. } | Action::Activate { unit, .. } = &action {
                                self.skip.push(*unit);
                            }
                            return None;
                        }
                        let tagged = tag(&d, side, &action, &keys, cards);
                        let line = serde_json::json!({"update": self.updates, "plan": format!("{action:?}"), "keys": keys.len()});
                        self.note(line);
                        match &action {
                            Action::Summon { hand, .. } if hand.len() > 1 => self.stats.fusions += 1,
                            Action::Activate { .. } => self.stats.activations += 1,
                            Action::Act { target, .. } if d.unit_at(*target).is_some_and(|u| u.controller() != side) => {
                                self.stats.attacks += 1
                            }
                            _ => {}
                        }
                        self.queue.extend(tagged);
                        Some(())
                    }
                }
            }
        }
    }
}

impl VideoBot {
    /// [`VideoBot::plan`] in an N-player duel (module docs): the focus and
    /// the Target, the projection, the module, the seat's keys.
    #[cfg(feature = "online")]
    fn plan_multi(&mut self, duel: &Duel, input: &DuelUi, cards: &Catalogue, fusions: &impl weevil::FusionData) -> Option<()> {
        use roses_core::duel::multi::computer;
        let side = duel.active;
        let focus = match self.focus {
            Some(f) if duel.alive(f) && duel.foe(side, f) => f,
            _ => {
                let f = computer::choose_focus(duel, side, cards);
                self.focus = Some(f);
                f
            }
        };
        // The single-target effects aim at the focus (MultiComputer sends
        // DuelCommand::Target; a person presses Target until it is there).
        let enemies = (0..duel.side_count()).filter(|&o| duel.alive(o) && duel.foe(side, o)).count();
        let aimed = duel.multi().and_then(|m| m.targets.get(usize::from(side)).copied());
        if enemies >= 2 && aimed != Some(focus) && duel.alive(focus) && self.targets_asked < 4 {
            self.targets_asked += 1;
            self.queue.push_back((TARGET_PRESS, Moment::Menu));
            return Some(());
        }
        // The Exodia aid: the limbs first; the leader and the limbs on the
        // board are kept out of the module's plans (they stay put).
        let mut skip = self.skip.clone();
        let assist = if self.config.exodia { exodia_assist(duel, side, cards, &self.skip) } else { None };
        if self.config.exodia {
            skip.extend(exodia_units(duel, side));
        }
        let (mut p, ps) = computer::project(duel, side, focus, &skip);
        if let Some(action) = assist {
            let cursor = input.cursors.get(usize::from(side)).copied().unwrap_or(roses_core::GridPos { column: 3, row: 3 });
            let keys = seat_keys(duel, &p, ps, side, cursor, &action, cards);
            if let Action::Act { unit, .. } = &action {
                self.skip.push(*unit);
            }
            if !keys.is_empty() {
                let line = serde_json::json!({"update": self.updates, "plan": format!("{action:?}"), "keys": keys.len(), "exodia_aid": true});
                self.note(line);
                let tagged = tag(&p, ps, &action, &keys, cards);
                self.queue.extend(tagged);
                return Some(());
            }
        }
        p.rng = self.plan_rng;
        let key = (side, ps, focus);
        if self.multi_memory.as_ref().is_none_or(|(k, _)| *k != key) {
            self.multi_memory = Some((key, weevil::Memory::new(&p, ps)));
        }
        let controller = self.config.controller.unwrap_or(22);
        let (_, memory) = self.multi_memory.as_mut().expect("set above");
        weevil::sync(memory, &p, ps);
        self.stats.plans += 1;
        match weevil::plan_for_rng(controller, &p, ps, memory, cards, fusions, self.config.tier) {
            Err(e) => {
                self.stats.plan_errors += 1;
                eprintln!("Video bot: the module refused a plan ({e}); this turn ends");
                let line = serde_json::json!({"update": self.updates, "plan_error": e.to_string()});
                self.note(line);
                self.fallback_turn = self.turn;
                None
            }
            Ok((plan, rng)) => {
                self.plan_rng = rng;
                match plan {
                    weevil::Plan::EndTurn => {
                        // The Exodia aid: a turn the module ends without a
                        // summon plays a card from the hand (the hand refills
                        // and the limbs come up).
                        if self.config.exodia && self.cycled != self.turn {
                            if let Some(action) = exodia_cycle(duel, side, cards) {
                                self.cycled = self.turn;
                                let cursor = input.cursors.get(usize::from(side)).copied().unwrap_or(roses_core::GridPos { column: 3, row: 3 });
                                let keys = seat_keys(duel, &p, ps, side, cursor, &action, cards);
                                if !keys.is_empty() {
                                    let line = serde_json::json!({"update": self.updates, "plan": format!("{action:?}"), "keys": keys.len(), "exodia_aid": "cycle"});
                                    self.note(line);
                                    let tagged = tag(&p, ps, &action, &keys, cards);
                                    self.queue.extend(tagged);
                                    return Some(());
                                }
                            }
                        }
                        self.queue.push_back((pad::START, Moment::EndTurn));
                        Some(())
                    }
                    weevil::Plan::NoKey => None,
                    weevil::Plan::MarkActed(id) => {
                        self.skip.push(id);
                        None
                    }
                    weevil::Plan::Act(action) => {
                        let cursor = input
                            .cursors
                            .get(usize::from(side))
                            .copied()
                            .unwrap_or(roses_core::GridPos { column: 3, row: 3 });
                        let keys = seat_keys(duel, &p, ps, side, cursor, &action, cards);
                        if keys.is_empty() {
                            if let Action::Act { unit, .. } | Action::Activate { unit, .. } = &action {
                                self.skip.push(*unit);
                            }
                            return None;
                        }
                        let tagged = tag(&p, ps, &action, &keys, cards);
                        let line = serde_json::json!({"update": self.updates, "plan": format!("{action:?}"), "keys": keys.len(), "focus": focus});
                        self.note(line);
                        match &action {
                            Action::Summon { hand, .. } if hand.len() > 1 => self.stats.fusions += 1,
                            Action::Activate { .. } => self.stats.activations += 1,
                            Action::Act { target, .. } if p.unit_at(*target).is_some_and(|u| u.controller() != ps) => {
                                self.stats.attacks += 1
                            }
                            _ => {}
                        }
                        self.queue.extend(tagged);
                        Some(())
                    }
                }
            }
        }
    }
}

/// The Exodia aid's units: the side's leader (when it is Exodia the
/// Forbidden One) and its limbs on the board.
pub fn exodia_units(duel: &Duel, side: u8) -> Vec<UnitId> {
    use roses_core::duel::{EXODIA_HEAD, EXODIA_LIMBS};
    let Some(leader) = duel.units.iter().find(|u| u.side == side && u.slot == 0) else { return Vec::new() };
    if leader.card != EXODIA_HEAD {
        return Vec::new();
    }
    let mut out = vec![leader.id];
    out.extend(duel.units.iter().filter(|u| u.side == side && u.slot != 0 && EXODIA_LIMBS.contains(&u.card)).map(|u| u.id));
    out
}

/// The Exodia aid (a video aid for an Exodia deck, not in the original's
/// modules): with Exodia the Forbidden One leading, a limb in the hand is
/// summoned face down on an empty cell next to the leader (the one away
/// from the enemies first); once all four stand round the leader, each is
/// revealed in place, so the next turn starts with Exodia complete (the
/// engine's `exodia_complete`). Units in `done` acted already this turn.
pub fn exodia_assist(duel: &Duel, side: u8, cards: &Catalogue, done: &[UnitId]) -> Option<Action> {
    use roses_core::duel::{EXODIA_HEAD, EXODIA_LIMBS};
    use roses_core::GridPos;
    let leader = duel.units.iter().find(|u| u.side == side && u.slot == 0)?;
    if leader.card != EXODIA_HEAD || duel.pending.is_some() || duel.pending_summon.is_some() {
        return None;
    }
    let at = leader.position;
    let near = |p: GridPos| p != at && p.column.abs_diff(at.column) <= 1 && p.row.abs_diff(at.row) <= 1;
    let limbs: Vec<&roses_core::duel::DuelUnit> =
        duel.units.iter().filter(|u| u.side == side && u.slot != 0 && EXODIA_LIMBS.contains(&u.card)).collect();
    let placed = EXODIA_LIMBS.iter().all(|&l| limbs.iter().any(|u| u.card == l && near(u.position)));
    if placed {
        return limbs
            .iter()
            .find(|u| !u.committed_face_up && !u.acted && u.spellbind == 0 && !done.contains(&u.id))
            .map(|u| Action::Act { unit: u.id, target: u.position, reveal: true, position: ai::Position::Keep });
    }
    // A magic card the aid set (or any set magic of the side next to the
    // leader) is played in place, so its cell is free again.
    if let Some(u) = duel.units.iter().find(|u| {
        u.side == side
            && u.slot != 0
            && !u.committed_face_up
            && !u.acted
            && !done.contains(&u.id)
            && near(u.position)
            && roses_core::duel::CardData::card(cards, u.card).is_some_and(|i| i.class() == 1)
    }) {
        return Some(Action::Act { unit: u.id, target: u.position, reveal: true, position: ai::Position::Keep });
    }
    // A leader on the board's edge has five neighbours: before the first
    // limb it steps one cell toward the middle (eight round it).
    let edge = at.column == 0 || at.column == 6 || at.row == 0 || at.row == 6;
    if edge && limbs.is_empty() && !leader.acted && !done.contains(&leader.id) {
        let step = |v: u8| if v < 3 { v + 1 } else if v > 3 { v - 1 } else { v };
        let to = if at.row == 0 || at.row == 6 { GridPos { column: at.column, row: step(at.row) } } else { GridPos { column: step(at.column), row: at.row } };
        if duel.unit_at(to).is_none() && duel.terrain_of(to) != Some(roses_core::duel::terrain::LABYRINTH) {
            return Some(Action::Act { unit: leader.id, target: to, reveal: false, position: ai::Position::Keep });
        }
    }
    let me = &duel.sides[usize::from(side)];
    if !me.can_summon {
        return None;
    }
    let index = me.hand.iter().position(|&c| {
        EXODIA_LIMBS.contains(&c)
            && !limbs.iter().any(|u| u.card == c)
            && duel.hand_cost(side, c, cards).is_some_and(|cost| cost <= me.stars)
    })?;
    // The free cells round the leader, the farthest from the enemy leaders
    // first (the limbs hide behind Exodia).
    let enemies: Vec<GridPos> =
        (0..duel.side_count()).filter(|&o| duel.alive(o) && duel.foe(side, o)).filter_map(|o| duel.leader_cell(o)).collect();
    let mut cells: Vec<GridPos> = (0..7u8)
        .flat_map(|c| (0..7u8).map(move |r| GridPos { column: c, row: r }))
        .filter(|&p| near(p) && duel.summon_area(side, p) && duel.unit_at(p).is_none())
        .collect();
    let distance = |p: GridPos| enemies.iter().map(|e| u32::from(e.column.abs_diff(p.column) + e.row.abs_diff(p.row))).min().unwrap_or(0);
    cells.sort_by_key(|&p| (std::cmp::Reverse(distance(p)), p.row, p.column));
    let cell = *cells.first()?;
    Some(Action::Summon { cell, hand: vec![index] })
}

/// The Exodia aid's hand cycle (module docs of [`exodia_assist`]): with no
/// limb to summon and the summon unused, one card of the hand is set next
/// to the leader so the hand refills: the strongest affordable monster of
/// 1500 ATK or more, else a magic or trap card, else the cheapest monster;
/// on the free cell nearest the enemies, and only while enough cells stay
/// free round the leader for the limbs still missing.
pub fn exodia_cycle(duel: &Duel, side: u8, cards: &Catalogue) -> Option<Action> {
    use roses_core::duel::{EXODIA_HEAD, EXODIA_LIMBS};
    use roses_core::GridPos;
    let leader = duel.units.iter().find(|u| u.side == side && u.slot == 0)?;
    let me = &duel.sides[usize::from(side)];
    if leader.card != EXODIA_HEAD || !me.can_summon || duel.pending.is_some() || duel.pending_summon.is_some() {
        return None;
    }
    let at = leader.position;
    let near = |p: GridPos| p != at && p.column.abs_diff(at.column) <= 1 && p.row.abs_diff(at.row) <= 1;
    let placed = EXODIA_LIMBS
        .iter()
        .filter(|&&l| duel.units.iter().any(|u| u.side == side && u.card == l && near(u.position)))
        .count();
    let free: Vec<GridPos> = (0..7u8)
        .flat_map(|c| (0..7u8).map(move |r| GridPos { column: c, row: r }))
        .filter(|&p| near(p) && duel.summon_area(side, p) && duel.unit_at(p).is_none())
        .collect();
    if free.is_empty() {
        return None;
    }
    // A monster stays on its cell until it moves; a magic card is played
    // at once (revealed in place next, [`exodia_assist`]) and leaves it.
    let room = free.len() > 4 - placed;
    let affordable = |c: u16| duel.hand_cost(side, c, cards).is_some_and(|cost| cost <= me.stars);
    let card = |c: u16| roses_core::duel::CardData::card(cards, c);
    let hand: Vec<(usize, u16)> = me.hand.iter().copied().enumerate().filter(|&(_, c)| !EXODIA_LIMBS.contains(&c) && affordable(c)).collect();
    let strong = hand
        .iter()
        .filter(|&&(_, c)| room && card(c).is_some_and(|i| i.is_monster() && i.attack >= 1500))
        .max_by_key(|&&(_, c)| card(c).map_or(0, |i| i.attack));
    let magic = hand.iter().find(|&&(_, c)| card(c).is_some_and(|i| i.class() == 1));
    let weak = hand.iter().filter(|_| room).min_by_key(|&&(_, c)| card(c).filter(|i| i.is_monster()).map_or(99, |i| i.level));
    let weak = weak.filter(|&&(_, c)| card(c).is_some_and(|i| i.is_monster()));
    let &(index, _) = strong.or(magic).or(weak)?;
    let enemies: Vec<GridPos> =
        (0..duel.side_count()).filter(|&o| duel.alive(o) && duel.foe(side, o)).filter_map(|o| duel.leader_cell(o)).collect();
    let distance = |p: GridPos| enemies.iter().map(|e| u32::from(e.column.abs_diff(p.column) + e.row.abs_diff(p.row))).min().unwrap_or(0);
    let cell = *free.iter().min_by_key(|&&p| (distance(p), p.row, p.column))?;
    Some(Action::Summon { cell, hand: vec![index] })
}

/// A board direction (as side 0's d-pad means it: Left = the column below,
/// Up = the row below) as the pad word `side` presses for it in an
/// N-player duel: side 1's d-pad is mirrored (the original's), the east
/// (2) and west (3) seats' turned a quarter (the inverse of `ui.rs`
/// `board_keys`).
pub fn seat_direction(side: u8, board: u16) -> u16 {
    let [l, r, u, d] = match side {
        1 => [pad::RIGHT, pad::LEFT, pad::DOWN, pad::UP],
        2 => [pad::UP, pad::DOWN, pad::RIGHT, pad::LEFT],
        3 => [pad::DOWN, pad::UP, pad::LEFT, pad::RIGHT],
        _ => [pad::LEFT, pad::RIGHT, pad::UP, pad::DOWN],
    };
    match board {
        pad::LEFT => l,
        pad::RIGHT => r,
        pad::UP => u,
        pad::DOWN => d,
        other => other,
    }
}

/// The cursor walk from `from` to `to` for `side` (columns first or rows).
fn seat_walk(out: &mut Vec<u16>, side: u8, from: roses_core::GridPos, to: roses_core::GridPos, columns_first: bool) {
    let dc = i32::from(from.column) - i32::from(to.column);
    let dr = i32::from(from.row) - i32::from(to.row);
    let column = seat_direction(side, if dc > 0 { pad::LEFT } else { pad::RIGHT });
    let row = seat_direction(side, if dr > 0 { pad::UP } else { pad::DOWN });
    let columns = std::iter::repeat_n(column, dc.unsigned_abs() as usize);
    let rows = std::iter::repeat_n(row, dr.unsigned_abs() as usize);
    if columns_first {
        out.extend(columns.chain(rows));
    } else {
        out.extend(rows.chain(columns));
    }
}

/// `ai::keys` for a seat of an N-player duel: the same sequences (0x277BD0)
/// with the seat's board d-pad ([`seat_direction`]); `projected` / `ps`
/// are the duel and side the action was planned on (its route choice and
/// its graveyard choice are read there; units and cells are the same).
pub fn seat_keys(
    duel: &Duel,
    projected: &Duel,
    ps: u8,
    side: u8,
    cursor: roses_core::GridPos,
    action: &Action,
    cards: &impl CardData,
) -> Vec<u16> {
    let mut out = Vec::new();
    match action {
        Action::Summon { cell, hand } => {
            let Some(leader) = duel.leader_cell(side) else { return out };
            seat_walk(&mut out, side, cursor, leader, true);
            out.push(pad::SQUARE);
            seat_walk(&mut out, side, leader, *cell, true);
            out.push(pad::CROSS);
            // Hand scrolling is never turned: Right is the next card.
            let mut at = 0usize;
            for &index in hand {
                let key = if index >= at { pad::RIGHT } else { pad::LEFT };
                out.extend(std::iter::repeat_n(key, index.abs_diff(at)));
                out.push(pad::UP);
                at = index;
            }
            out.extend([pad::CROSS, pad::CROSS]);
        }
        Action::Act {
            unit,
            target,
            reveal,
            position,
        } => {
            let Some(u) = duel.unit(*unit) else { return out };
            seat_walk(&mut out, side, cursor, u.position, true);
            out.push(pad::CROSS);
            if u.slot != 0 {
                if *reveal {
                    out.push(pad::R2);
                }
                if *target == u.position {
                    match position {
                        ai::Position::Attack if u.committed_defence => out.push(pad::L1),
                        ai::Position::Defence if !u.committed_defence => out.push(pad::R1),
                        _ => (),
                    }
                }
            }
            // The route's first step tells columns-first from rows-first.
            let path = ai::path(projected, *unit, *target, cards);
            let first = path.first().is_none_or(|c| c.column != u.position.column);
            seat_walk(&mut out, side, u.position, *target, first);
            out.push(pad::CROSS);
        }
        Action::Activate { at, graveyard, down, .. } => {
            seat_walk(&mut out, side, cursor, *at, true);
            out.extend([pad::CROSS, pad::R2, pad::CROSS]);
            if let Some(g) = graveyard {
                if *g != ps {
                    out.push(if ps != 0 { pad::LEFT } else { pad::RIGHT });
                }
            }
            out.extend(std::iter::repeat_n(pad::DOWN, usize::from(*down)));
            out.push(pad::CROSS);
        }
    }
    out
}

/// Each key of an action's sequence with its moment.
fn tag(duel: &Duel, side: u8, action: &Action, keys: &[u16], cards: &Catalogue) -> Vec<(u16, Moment)> {
    let dpad = |k: u16| k & (pad::UP | pad::DOWN | pad::LEFT | pad::RIGHT) != 0;
    let last = keys.len().saturating_sub(1);
    match action {
        Action::Summon { hand, .. } => {
            let s = usize::from(side & 1);
            let big_card = hand.iter().any(|&i| {
                duel.sides[s]
                    .hand
                    .get(i)
                    .and_then(|&c| cards.card(c))
                    .is_some_and(|c| c.is_monster() && c.attack >= 2000)
            });
            let big = hand.len() > 1 || big_card;
            let mut crosses = 0;
            keys.iter()
                .enumerate()
                .map(|(i, &k)| {
                    let m = if k == pad::CROSS {
                        crosses += 1;
                        if i == last && big { Moment::Big } else { Moment::Confirm }
                    } else if k == pad::UP && crosses >= 1 {
                        Moment::Mark
                    } else if dpad(k) && crosses == 0 {
                        Moment::Cursor
                    } else {
                        Moment::Menu
                    };
                    (k, m)
                })
                .collect()
        }
        Action::Act { target, .. } => {
            let attack = duel.unit_at(*target).is_some_and(|u| u.controller() != side);
            keys.iter()
                .enumerate()
                .map(|(i, &k)| {
                    let m = if k == pad::CROSS {
                        if i == last && attack { Moment::Big } else { Moment::Confirm }
                    } else if dpad(k) {
                        Moment::Cursor
                    } else {
                        Moment::Menu
                    };
                    (k, m)
                })
                .collect()
        }
        Action::Activate { .. } => {
            let mut crosses = 0;
            keys.iter()
                .map(|&k| {
                    let m = if k == pad::CROSS {
                        crosses += 1;
                        if crosses == 2 { Moment::Big } else { Moment::Confirm }
                    } else if dpad(k) && crosses == 0 {
                        Moment::Cursor
                    } else {
                        Moment::Menu
                    };
                    (k, m)
                })
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_waits_repeat_from_a_seed_and_stay_in_range() {
        let mut a = Pacer::new(Pace::Human, 42);
        let mut b = Pacer::new(Pace::Human, 42);
        let seq: Vec<u32> = (0..50).map(|_| a.delay(Moment::Cursor)).collect();
        assert_eq!(seq, (0..50).map(|_| b.delay(Moment::Cursor)).collect::<Vec<_>>());
        assert!(seq.iter().all(|&d| (15..=30).contains(&d)));
        let mut c = Pacer::new(Pace::Human, 43);
        assert_ne!(seq, (0..50).map(|_| c.delay(Moment::Cursor)).collect::<Vec<_>>());
        let mut off = Pacer::new(Pace::Off, 42);
        assert_eq!(off.delay(Moment::Big), 0);
        assert!(off.ready(Moment::Big));
    }

    #[test]
    fn a_long_turn_waits_less_once_its_budget_is_spent() {
        let mut p = Pacer::new(Pace::Human, 7);
        let mut total = 0;
        for _ in 0..200 {
            total += p.delay(Moment::Big);
        }
        assert!(total <= TURN_BUDGET + 112 + 200 * MIN_WAIT, "{total}");
        p.new_turn();
        assert!(p.delay(Moment::Big) >= 75);
    }

    /// The seat keys walk every seat's cursor where the plan means, on
    /// the input layer with each seat's turned d-pad.
    #[cfg(feature = "online")]
    #[test]
    fn seat_keys_walk_each_seats_cursor_to_the_cell() {
        use roses_core::duel::MultiMode;
        use roses_core::GridPos;
        let cards = roses_net::synthetic::catalogue();
        let decks: Vec<(u16, Vec<u16>)> = (0..4)
            .map(|i| {
                let d = &roses_net::synthetic::submissions()[i % 2];
                (d.leader, d.cards.clone())
            })
            .collect();
        let base = crate::duel_multi::preview_duel(roses_net::synthetic::board(), &decks, MultiMode::FreeForAll, 3, &cards).unwrap();
        for side in 0..4u8 {
            let mut duel = base.clone();
            duel.active = side;
            let mut ui = DuelUi::new(&duel);
            for to in [GridPos { column: 1, row: 2 }, GridPos { column: 5, row: 4 }, GridPos { column: 3, row: 3 }] {
                for first in [true, false] {
                    let start = ui.cursor(&duel);
                    let mut keys = Vec::new();
                    seat_walk(&mut keys, side, start, to, first);
                    for k in keys {
                        ui.press(&duel, &cards, k).unwrap();
                        ui.press(&duel, &cards, 0).unwrap();
                    }
                    assert_eq!(ui.cursor(&duel), to, "side {side} from {start:?} columns first {first}");
                }
            }
        }
        // The original's two sides keep `ai::keys`' words.
        for b in [pad::LEFT, pad::RIGHT, pad::UP, pad::DOWN] {
            assert_eq!(seat_direction(0, b), b);
        }
        assert_eq!(seat_direction(1, pad::LEFT), pad::RIGHT);
        assert_eq!(seat_direction(1, pad::UP), pad::DOWN);
    }

    #[test]
    fn ready_holds_a_press_for_its_wait() {
        let mut p = Pacer::new(Pace::Human, 1);
        let mut n = 0;
        while !p.ready(Moment::Confirm) {
            n += 1;
        }
        assert!((38..=62).contains(&n), "{n}");
        let words = p.pace_words(&[pad::DOWN, pad::DOWN, pad::DOWN, pad::CROSS]);
        assert_eq!(words.iter().filter(|&&w| w != 0).count(), 4);
        assert!(words.len() > 4 + 9 + 9 + 3 + 20);
    }
}
