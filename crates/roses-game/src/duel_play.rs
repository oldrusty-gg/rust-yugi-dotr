//! The first campaign duel as this preview plays it: the rules engine, the
//! pad-driven input states and Weevil's ported controller
//! (`roses_core::duel::session`), loaded from a private `roses import-duel`
//! folder. Bevy-independent: the renderer feeds pad presses once per 25 Hz
//! frame and draws from the session state.
//!
//! The campaign shuffle needs the original RNG state at duel start, which is
//! not reproduced; the preview seeds it explicitly ([`DEFAULT_SEED`]).
//!
//! [`DuelPlay`] is the renderer's per-update driver (pad → session frame →
//! status), [`overlay_input`] turns the session into the board's cell and
//! unit layer input, [`follow_cursor`] moves the board camera after the
//! active side's cursor, and [`DuelHud`] draws the original HUD for the
//! session into a premultiplied RGBA layer; all are GPU-free so they are
//! tested here.

use crate::Result;
use crate::duel_board::BoardCamera;
use crate::duel_overlay::{self, OverlayInput, UnitInput};
use roses_assets::RgbaImage;
use roses_client::card_face::{self, FaceAssets, FaceSize};
use roses_client::duel_hud::{
    self, FlashWave, HandFace, HudFrame, HudTables, HudTextures, HudUpdate, Primitive,
    UnsupportedKind,
};
use roses_core::GridPos;
use roses_core::duel::ai::{pad, weevil};
use roses_core::duel::queries::Context;
use roses_core::duel::script::ScriptFrame;
use roses_core::duel::script::stand_in::stand_in;
use roses_core::duel::session::{ComputerSeat, DuelSession, Frame};
use roses_core::duel::ui::{Mode, Response};
use roses_core::duel::{Catalogue, Duel, DuelSetup, EndReason, build_campaign_decks};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

pub const IMPORT_SCHEMA: &str = "roses-duel-import-1";
const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Deserialize)]
struct Manifest {
    schema: String,
    files: Vec<ManifestFile>,
}

#[derive(Deserialize)]
struct ManifestFile {
    path: String,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SideDeck {
    pub deck_record: usize,
    pub leader: u16,
    pub cards: Vec<u16>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FirstDuel {
    pub board_map: usize,
    pub terrain: Vec<u8>,
    pub first: u8,
    pub sides: [SideDeck; 2],
    pub computer_sides: Vec<u8>,
    pub planner_tier: i16,
    /// Leader abilities active at each leader's rank (table 0x2C26B8); the
    /// first-duel import leaves them empty (neither leader has one the
    /// rules consult at ranks 1 and 2).
    #[serde(default)]
    pub leader_abilities: [Vec<u8>; 2],
    /// The computer side's controller byte (opponent id; Weevil 2).
    #[serde(default = "weevil")]
    pub opponent: u8,
    /// Each leader's rank (0 NCO .. 12 SD) for the Destiny byte's rank bonus
    /// (the player's saved rank; the opponent's deck-record rank [H]).
    #[serde(default)]
    pub leader_ranks: [u8; 2],
    /// The side the player's pad plays (0x232E40: p2 = SAVE[0]; the White
    /// Rose route seats the player on side 1, identity 1100 in p4) [F].
    #[serde(default)]
    pub player_side: u8,
    /// Custom Duel's duel words (side +0x33A LP, +0x342 SP recharge rate),
    /// applied after the duel's creation; `None` = 4000 / 3 (campaign).
    #[serde(default)]
    pub custom_rules: Option<([u16; 2], [u8; 2])>,
    /// Custom Duel's params 0x1C / 0x1D (leader-ability and card-display
    /// switches, bit = side); `None` = 3 / 3 (every campaign duel).
    #[serde(default)]
    pub options: Option<[u32; 2]>,
}

fn weevil() -> u8 {
    2
}

/// Everything read from the import folder.
pub struct DuelData {
    pub catalogue: Catalogue,
    pub names: Vec<String>,
    pub first_duel: FirstDuel,
    pub tables: weevil::Tables,
    /// Verbatim disc files for the HUD (executable tables, messages,
    /// TEXSYS.MRG, PICMINI.MRG).
    pub executable: Vec<u8>,
    pub messages: Vec<u8>,
    pub texsys: Vec<u8>,
    pub picmini: Vec<u8>,
    /// TEXEFF.MRG (battle scene effect textures), in imports that have it.
    pub texeff: Option<Vec<u8>>,
    /// PICTURE.MRG of the duel-end import (the hand row's card faces), when
    /// the game import has one ([`load_with_end`]).
    pub pictures: Option<Vec<u8>>,
}

fn read_checked(dir: &Path, manifest: &Manifest, name: &str) -> Result<Vec<u8>> {
    let entry = manifest
        .files
        .iter()
        .find(|f| f.path == name)
        .ok_or_else(|| format!("duel import lacks {name}"))?;
    let bytes = crate::bounded_read(&dir.join(name), MAX_FILE_BYTES)?;
    if format!("{:x}", Sha256::digest(&bytes)) != entry.sha256 {
        return Err(format!("{name} does not match its import hash"));
    }
    Ok(bytes)
}

/// Only the import's hash-checked executable (for callers that need its
/// tables before the whole duel import is loaded).
pub fn load_executable(dir: &Path) -> Result<Vec<u8>> {
    let manifest: Manifest = serde_json::from_slice(&crate::bounded_read(
        &dir.join("manifest.json"),
        1024 * 1024,
    )?)
    .map_err(|e| e.to_string())?;
    if manifest.schema != IMPORT_SCHEMA {
        return Err(format!("unsupported duel import {}", manifest.schema));
    }
    read_checked(dir, &manifest, "SLES_515.79")
}

pub fn load(dir: &Path) -> Result<DuelData> {
    let manifest: Manifest = serde_json::from_slice(&crate::bounded_read(
        &dir.join("manifest.json"),
        1024 * 1024,
    )?)
    .map_err(|e| e.to_string())?;
    if manifest.schema != IMPORT_SCHEMA {
        return Err(format!("unsupported duel import {}", manifest.schema));
    }
    let json = |name: &str| read_checked(dir, &manifest, name);
    let catalogue: Catalogue =
        serde_json::from_slice(&json("catalogue.json")?).map_err(|e| e.to_string())?;
    let names: Vec<String> =
        serde_json::from_slice(&json("card-names.json")?).map_err(|e| e.to_string())?;
    let first_duel: FirstDuel =
        serde_json::from_slice(&json("first-duel.json")?).map_err(|e| e.to_string())?;
    let executable = read_checked(dir, &manifest, "SLES_515.79")?;
    let tables = weevil::Tables::new(&catalogue.fusions, &executable)
        .ok_or("the imported executable is too short for the planner tables")?;
    Ok(DuelData {
        catalogue,
        names,
        first_duel,
        tables,
        executable,
        messages: read_checked(dir, &manifest, "MSG_E.DAT")?,
        texsys: read_checked(dir, &manifest, "TEXSYS.MRG")?,
        picmini: read_checked(dir, &manifest, "PICMINI.MRG")?,
        texeff: match manifest.files.iter().any(|f| f.path == "TEXEFF.MRG") {
            true => Some(read_checked(dir, &manifest, "TEXEFF.MRG")?),
            false => None,
        },
        pictures: None,
    })
}

/// [`load`] plus the duel-end import's PICTURE.MRG (hash-checked) when
/// `duel_end` is given.
pub fn load_with_end(dir: &Path, duel_end: Option<&Path>) -> Result<DuelData> {
    let mut data = load(dir)?;
    if let Some(end) = duel_end {
        data.pictures = Some(crate::duel_end_screens::load_duel_end_import(end)?.pictures);
    }
    Ok(data)
}

/// A new first-duel session: decks shuffled by the campaign shuffle from
/// `seed`, the side `first` first, the player's pad on `player_side` (0, or
/// 1 on the White Rose route), the opponent (and optionally the player's
/// side too) played by the ported controller.
pub fn start(data: &DuelData, seed: u64, computer_player: bool) -> Result<DuelSession> {
    let f = &data.first_duel;
    let mut rng = seed;
    let decks = build_campaign_decks(&mut rng, [&f.sides[0].cards, &f.sides[1].cards]);
    let setup = DuelSetup {
        terrain: f.terrain.clone(),
        leaders: [f.sides[0].leader, f.sides[1].leader],
        decks,
        first: f.first,
        leader_abilities: f.leader_abilities.clone(),
        // The player's pad and the opponent's controller byte, by side.
        controllers: [0u8, 1].map(|side| {
            if side == f.player_side {
                roses_core::duel::PAD_CONTROLLER
            } else {
                f.opponent
            }
        }),
    };
    let (mut duel, _) = Duel::new(setup, &data.catalogue).map_err(|e| e.to_string())?;
    // The Destiny Draw's rows (0x2B7AF0) and the leaders' ranks (its bonus).
    duel.set_destiny_rows(roses_assets::parse_pal_destiny_rows(&data.executable).map_err(|e| e.to_string())?);
    duel.set_leader_ranks(f.leader_ranks);
    if let Some((lp, sp)) = f.custom_rules {
        duel.set_custom_rules(lp, sp);
    }
    // The opponent's side plays with its own module (the control byte of
    // the opponent id); a computer-played player side uses Weevil's, as
    // the terminal harness does.
    let seat = |side: u8| {
        (f.computer_sides.contains(&side) || (side == f.player_side && computer_player)).then(|| {
            if side != f.player_side {
                ComputerSeat::with_controller(&duel, side, f.planner_tier, f.opponent)
            } else {
                ComputerSeat::new(&duel, side, f.planner_tier)
            }
        })
    };
    let seats = [seat(0), seat(1)];
    Ok(DuelSession::new(duel, seats))
}

/// Human pad presses with the original's auto-repeat (0x203380 mode 2 and
/// 0x203490): a button counts as pressed on the frame it goes down and,
/// while held, again on every frame once its counter (set to -12 at the
/// press, advanced each frame) reaches 0, every 1 frame (pad bytes +0x59 =
/// 12 and +0x5A & 0xF = 1 in every saved duel state). The repeat period and
/// delay are [F] from the code and saved RAM.
///
/// Only [`REPEAT_MASK`] (the d-pad) repeats; every other button is a new
/// press only. [V] The duel dispatcher 0x1D00F0 reads the human pad three
/// ways (0x1D3570-0x1D35A8): held (mode 1), new presses (mode 0, `$s7`) and
/// repeats (mode 2, `$s4`). The repeat word is used for the d-pad (0x5000,
/// 0x1000, 0xA000, 0x8000) and L1/R1 masks at 0x1D365C-0x1D36B8, but the
/// states modelled here read L1/R1 and every face button, Start and Select
/// from the new-press word (state 0x18 tests `$s5` = `$s7` at 0x1D38BC,
/// 0x1D38C4, 0x1D397C, 0x1D3BD0; state 0x1D tests `$s0` = `$s7` at
/// 0x1D4254 and its shoulders at 0x1D46F4), so only the d-pad repeats here.
///
/// Counter timing [V] (0x203180, after the pad read 0x203230 in each frame):
/// a button's counter goes up by one only when it was down in both of the
/// two previous samples, otherwise it is reset to -12 (pad +0x59); a repeat
/// is reported while the counter is >= 0 and a multiple of the period
/// (+0x5A & 0xF = 1). The counter uses the older sample pair, so the first
/// repeat comes 13 frames after the press and one more comes on the frame
/// the release is first seen.
#[derive(Clone, Debug, Default)]
pub struct PadRepeat {
    /// The last two samples.
    prev1: u16,
    prev2: u16,
    counters: [i32; 16],
}

pub const REPEAT_DELAY: i32 = 12;
pub const REPEAT_PERIOD: i32 = 1;
/// Buttons the modelled duel states read with auto-repeat: the d-pad.
pub const REPEAT_MASK: u16 = pad::UP | pad::RIGHT | pad::DOWN | pad::LEFT;

impl PadRepeat {
    /// Buttons down in the last sample.
    pub fn held(&self) -> u16 {
        self.prev1
    }

    /// `held`: buttons down this frame. Returns the presses to feed: new
    /// presses of every button, plus repeats of [`REPEAT_MASK`] buttons.
    pub fn frame(&mut self, held: u16) -> u16 {
        let (edge, repeats) = self.frame_split(held);
        edge | (repeats & REPEAT_MASK)
    }

    /// One sample: (new presses, auto-repeats of every button) — the menus
    /// that read 0x203380 kind 2 on face buttons and shoulders (Trade).
    pub fn frame_split(&mut self, held: u16) -> (u16, u16) {
        let edge = held & !self.prev1;
        let steady = self.prev1 & self.prev2;
        let mut repeats = 0;
        for bit in 0..16 {
            let mask = 1u16 << bit;
            let c = &mut self.counters[bit];
            *c = if steady & mask != 0 {
                *c + 1
            } else {
                -REPEAT_DELAY
            };
            // REPEAT_PERIOD is the traced value, which happens to be 1.
            #[allow(clippy::modulo_one)]
            if *c >= 0 && *c % REPEAT_PERIOD == 0 {
                repeats |= mask;
            }
        }
        self.prev2 = self.prev1;
        self.prev1 = held;
        (edge, repeats)
    }
}

/// One 25 Hz frame of the session with this frame's pad presses.
pub fn tick(session: &mut DuelSession, data: &DuelData, pad: u16) -> Result<Frame> {
    session
        .frame(pad, &data.catalogue, &data.tables)
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------- pad names
/// `--seed` default: an arbitrary fixed campaign-shuffle seed (the original's
/// RNG state at duel start is not reproduced).
pub const DEFAULT_SEED: u64 = 5;

/// Pad buttons by name (`--replay-pad`, status messages), in pad-bit order.
pub const BUTTONS: [(&str, u16); 14] = [
    ("l2", pad::L2),
    ("r2", pad::R2),
    ("l1", pad::L1),
    ("r1", pad::R1),
    ("triangle", pad::TRIANGLE),
    ("circle", pad::CIRCLE),
    ("cross", pad::CROSS),
    ("square", pad::SQUARE),
    ("select", pad::SELECT),
    ("start", pad::START),
    ("up", pad::UP),
    ("right", pad::RIGHT),
    ("down", pad::DOWN),
    ("left", pad::LEFT),
];

/// Keyboard layout of duel play: physical keys by their Bevy `KeyCode` name.
/// Arrows, X (Cross), A (Square), Z (Circle), S (Start), Q (L1) and E (R1)
/// follow the Simon preview; D (Triangle), 1 (L2), W (R2) and Tab (Select)
/// are this preview's choice.
pub const KEYBOARD: [(&str, u16); 14] = [
    ("ArrowUp", pad::UP),
    ("ArrowDown", pad::DOWN),
    ("ArrowLeft", pad::LEFT),
    ("ArrowRight", pad::RIGHT),
    ("KeyX", pad::CROSS),
    ("KeyA", pad::SQUARE),
    ("KeyZ", pad::CIRCLE),
    ("KeyD", pad::TRIANGLE),
    ("KeyS", pad::START),
    ("Tab", pad::SELECT),
    ("KeyQ", pad::L1),
    ("KeyE", pad::R1),
    ("Digit1", pad::L2),
    ("KeyW", pad::R2),
];

/// Pad bits of a physical key (Bevy `KeyCode` name); 0 when unmapped.
pub fn key_pad(key_code: &str) -> u16 {
    KEYBOARD
        .iter()
        .find(|(name, _)| *name == key_code)
        .map_or(0, |&(_, bits)| bits)
}

/// Pad bits as `cross+up`, or `none`.
pub fn button_names(bits: u16) -> String {
    let names: Vec<&str> = BUTTONS
        .iter()
        .filter(|&&(_, b)| bits & b != 0)
        .map(|&(name, _)| name)
        .collect();
    if names.is_empty() {
        "none".into()
    } else {
        names.join("+")
    }
}

/// Longest `--replay-pad` (updates).
pub const MAX_REPLAY_PAD: usize = 10_000;

/// `--replay-pad`: comma-separated updates, each `none` or button names
/// joined by `+`, optionally `*n` for n consecutive updates. Every entry is a
/// fresh press on its own update (no auto-repeat), so `right*3` is three
/// presses.
pub fn parse_replay_pad(text: &str) -> Result<Vec<u16>> {
    let mut out = Vec::new();
    for item in text.split(',') {
        let item = item.trim();
        let (buttons, count) = match item.split_once('*') {
            Some((buttons, count)) => (
                buttons.trim(),
                count
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| format!("Invalid replay count in {item:?}"))?,
            ),
            None => (item, 1),
        };
        let mut bits = 0;
        if buttons != "none" {
            for name in buttons.split('+') {
                let name = name.trim().to_ascii_lowercase();
                bits |= BUTTONS
                    .iter()
                    .find(|(n, _)| *n == name)
                    .ok_or_else(|| {
                        format!(
                            "Unknown pad button {name:?} (use none or {})",
                            button_names(u16::MAX).replace('+', ", ")
                        )
                    })?
                    .1;
            }
        }
        if count == 0 || out.len() + count > MAX_REPLAY_PAD {
            return Err(format!(
                "--replay-pad needs counts >= 1 and at most {MAX_REPLAY_PAD} updates"
            ));
        }
        out.extend(std::iter::repeat_n(bits, count));
    }
    Ok(out)
}

// ---------------------------------------------------------------- board
/// The board's cell and unit layer input for this session frame: the input
/// state (`Mode::state`), the active side, the move selection's marks,
/// allowance and path (empty in every other state, including 0x3E, whose
/// reach map is not produced by the port), and every unit with its drawn
/// flags. Face-up means shown face-up (`committed_face_up || face_up`); the
/// outline pulse (+0x57 bit1) is not part of the rules state and stays off.
pub fn overlay_input(session: &DuelSession) -> OverlayInput {
    let duel = &session.duel;
    let (marks, move_allowance, path) = match &session.input.mode {
        Mode::Move(sel) => (
            sel.marks.clone(),
            sel.allowance,
            sel.path.iter().map(|p| [p.column, p.row]).collect(),
        ),
        _ => (Vec::new(), 0, Vec::new()),
    };
    OverlayInput {
        state: session.input.mode.state(),
        active_side: duel.active,
        marks,
        move_allowance,
        path,
        brightness: 128,
        units: duel
            .units
            .iter()
            .map(|u| UnitInput {
                side: u.side,
                slot: u.slot,
                cell: [u.position.column, u.position.row],
                card: u.card,
                face_up: u.committed_face_up || u.face_up,
                defence: u.defence,
                // N-player (a port addition): a knocked-out player's cards
                // kept on the board (STAY) are drawn dim, as a unit that
                // has acted (brightness 64).
                acted: u.acted || duel.is_out(u.side),
                outline_pulse: false,
                face: None,
                offset: [0.0; 2],
                turn: 0,
                flip: None,
                alpha: None,
                sideways: None,
                spin: None,
                model_scale: None,
                emblem: None,
            })
            .collect(),
        pulse: None,
        settled: false,
        hidden_cells: false,
    }
}

/// The values of every face-up card image on the board (`UnitInput::face`):
/// cost, ATK/DEF with the terrain bonus at the unit's cell (at the cursor
/// for the unit a move selection holds) and the effect frame, through the
/// same getters as the HUD (`roses_core::duel::queries`).
pub fn add_board_faces(input: &mut OverlayInput, session: &DuelSession, cards: &Catalogue, ctx: &Context) {
    use roses_core::duel::queries::{Query, Record};
    let duel = &session.duel;
    let q = Query::new(duel, cards, ctx);
    let held = match &session.input.mode {
        Mode::Move(sel) => Some(sel.unit),
        _ => None,
    };
    let cursor = session.input.cursors[side_index(duel, duel.active)];
    for (unit, u) in input.units.iter_mut().zip(&duel.units) {
        if unit.slot == 0 || !unit.face_up {
            continue;
        }
        let cell = if held == Some(u.id) {
            cursor
        } else {
            u.position
        };
        let r = Record::Field(u);
        let Some(stats) = q.stats(r, cell) else {
            continue;
        };
        unit.face = Some(duel_overlay::BoardFace {
            level: q.unit_cost(u).max(0) as u8,
            attack: stats.attack,
            defence: stats.defence,
            effect: q.has_effect(r),
        });
    }
}

/// The first `Battle` of `events` as the attack effects' outcome boundary
/// 0x1F8100 reads it: `before` is the duel before the step. Both units'
/// `Query::stats` at the defender's cell, with the values the step fought
/// with in place of the attacker's ATK and the defender's stat: they already
/// hold what the step changed before the battle (Mirror Wall, a flip effect,
/// the pre-pass) [H: the original's records hold those by the scene].
fn effect_battle(
    before: &Duel,
    events: &[roses_core::duel::DuelEvent],
    cards: &Catalogue,
    ctx: &Context,
) -> Option<roses_fx::types::BattleState> {
    use roses_core::duel::queries::{Query, Record};
    let (attacker, defender, attack, defend, defence) = events.iter().find_map(|e| match e {
        roses_core::duel::DuelEvent::Battle { attacker, defender, attack, defend, defence } => {
            Some((*attacker, *defender, i32::from(*attack), i32::from(*defend), *defence))
        }
        _ => None,
    })?;
    let (a, d) = (before.unit(attacker)?, before.unit(defender)?);
    let q = Query::new(before, cards, ctx);
    let stats = |u| q.stats(Record::Field(u), d.position);
    let (sa, sd) = (stats(a)?, stats(d)?);
    let unit = |attack, defence, defence_position| roses_fx::types::BattleUnit { attack, defence, defence_position };
    let (d_attack, d_defence) = if defence { (sd.attack, defend) } else { (defend, sd.defence) };
    Some(crate::duel_bmodule::battle_state(
        [unit(attack, sa.defence, a.defence), unit(d_attack, d_defence, defence)],
        a.side == before.active,
        (d.position.column, d.position.row),
    ))
}

/// Face-up card images still without values (`face: None`) take them from
/// `duel`, the state before the presented step: a unit a monster effect
/// reveals can be gone from the live duel already (Mushroom Man revealed by
/// a step and destroyed by the battle the rules resolved with it), and its
/// card is drawn face up with the values it had on the board.
pub fn fill_faces_from(input: &mut OverlayInput, duel: &roses_core::duel::Duel, cards: &Catalogue, ctx: &Context) {
    use roses_core::duel::queries::{Query, Record};
    let q = Query::new(duel, cards, ctx);
    for unit in input.units.iter_mut().filter(|u| u.slot != 0 && u.face_up && u.face.is_none()) {
        let Some(u) = duel.units.iter().find(|u| (u.side, u.slot) == (unit.side, unit.slot) && u.card == unit.card) else {
            continue;
        };
        let r = Record::Field(u);
        if let Some(stats) = q.stats(r, u.position) {
            unit.face = Some(duel_overlay::BoardFace {
                level: q.unit_cost(u).max(0) as u8,
                attack: stats.attack,
                defence: stats.defence,
                effect: q.has_effect(r),
            });
        }
    }
}

/// Move the board camera's cursor toward `target` by at most one step per
/// update (one cell along each differing axis, so a diagonal counts as one
/// step), each step starting the original 8-update track. A single cursor
/// press, the normal case, is exactly the original's step; longer jumps
/// (Circle back to the path origin or the leader, the other side's cursor at
/// a turn change) are walked one step per update because the original's
/// motion for them is not traced. Returns whether a step started.
pub fn follow_cursor(camera: &mut BoardCamera, target: GridPos) -> bool {
    let (column, row) = camera.cursor;
    let dc = (target.column as i8 - column as i8).signum();
    let dr = (target.row as i8 - row as i8).signum();
    (dc, dr) != (0, 0) && camera.step(dc, dr)
}

// ---------------------------------------------------------------- driver
/// The duel as the renderer plays it: one [`DuelPlay::update`] per 25 Hz
/// update. A session error (e.g. `Unsupported`) stops it for good, keeping
/// the last state on screen.
pub struct DuelPlay {
    pub data: DuelData,
    pub session: DuelSession,
    pub repeat: PadRepeat,
    /// Pad 2's auto-repeat (a duel with two humans).
    pub repeat2: PadRepeat,
    /// Both pads' held words of the last update (R2 for the HUD getters).
    pub held_pads: [u16; 2],
    /// Pad 2's new-press word of the last update.
    pub last_presses2: u16,
    pub seed: u64,
    /// Updates run so far.
    pub frames: u64,
    /// The last refused or unsupported press.
    pub status: Option<String>,
    /// The session error that stopped the duel.
    pub halted: Option<String>,
    /// The new-press word of the last update (the duel-end result screens
    /// read △○×□ from it once the duel has ended).
    pub last_presses: u16,
    /// Terrain the board tiles were instanced with (the duel's start).
    pub board_terrain: Vec<u8>,
    terrain_logged: bool,
    reported: BTreeSet<&'static str>,
    /// Scripted lessons: the operator pages the tutorial messages (a fresh
    /// Cross on every fifth accepting poll) instead of the keyboard.
    pub auto_pages: bool,
    operator: crate::tutorial::Operator,
    /// The script's poll of the last update (presentation one-shots).
    pub last_script: Option<ScriptFrame>,
    /// Traced outcomes applied where the engine refused a lesson key
    /// (`script::stand_in`), as (step, note).
    pub stand_ins: Vec<(u16, &'static str)>,
    /// The walk being played (states 0x36/0x37); input waits until it ends.
    pub walk: Option<Walk>,
    /// The battle opening being played (turn to face, battle camera).
    pub intro: Option<Box<BattleIntro>>,
    /// A camera move the renderer starts once (taken by [`drive_camera`]).
    pub camera_command: Option<CameraMove>,
    /// A unit camera the renderer resolves with the unit's model (taken by
    /// [`drive_camera`]).
    pub unit_camera: Option<UnitCamera>,
    /// A polar yaw turn (frames, 16-bit dyaw) around the current target
    /// (0x2622B0), taken by [`drive_camera`].
    pub camera_orbit: Option<(u32, i32)>,
    /// Each side's board camera setting (side record +0x1D0: distance,
    /// yaw, pitch), which the analog sticks change ([`stick_camera`]).
    pub side_cameras: [SideCamera; 2],
    /// Each pad port's analog stick bytes this update (0x203580 indices
    /// 0..3: right X, right Y, left X, left Y; 0x80 = centre), `None`
    /// without an analog pad (0x203500 = 0). Set by the renderer.
    pub sticks: [Option<[u8; 4]>; 2],
    /// Updates left on the board camera's tracks (cursor step or stick
    /// turn) during which state 0x18 reads no pad (0x1FB900).
    pub board_track: u32,
    /// d63 (0x2DB903): 0x3C when state 0x18's cursor handler takes a
    /// direction (a step, or a press at the edge), -1 per update (0x1D0970),
    /// 0 when Cross picks a card or a unit there (0x1D3A60 / 0x1D3B74) [F;
    /// interpreter the project notes on session 000-start].
    pub d63: u8,
    /// d63 was set at this update's start: the cursor glow's word 0x2DB826
    /// steps rising (0x1D081C, before the decrement and the handler).
    pub glow_hold: bool,
    /// The side whose turn the board camera last moved to (0x1DAAA0).
    pub camera_side: Option<u8>,
    /// Event 0x72 "Destiny Draw!" playing (updates left): message 0x116 at
    /// y 0x50 before the refill's hand row (handler 0x280180; its leader
    /// script 0x271 and SEs are not drawn) [F SPECIAL-DRAW.md §3].
    pub destiny_event: Option<u32>,
    /// The Exodia event 0x6D before the result (`duel_exodia`).
    pub exodia: Option<Box<crate::duel_exodia::ExodiaRun>>,
    /// The renderer has the arena import: the battle scene plays the arena
    /// crossfade and orbit (else the return follows the versus rest).
    pub arena_available: bool,
    /// Loads the attack phase's units and cameras (set with the arena by
    /// the renderer); without it the arena ends with its orbit.
    pub arena_provider: Option<crate::duel_arena::ArenaProvider>,
    /// The arena battle's attack effects run live (BMODULE on a worker).
    pub arena_effects: Option<crate::duel_bmodule::EffectWorker>,

    /// The hand row's motion while it plays (`duel_hand`; input is not
    /// polled, as in the original's animation states).
    pub hand_anim: Option<crate::duel_hand::HandAnim>,
    /// Cards each side's row held when it last closed or summoned (the
    /// rest fly in when it next opens).
    hand_seen: [usize; 4],
    /// The details view's slide d3a (0 .. [`DETAILS_REST`]) and what it
    /// showed last (drawn while it slides out).
    pub details_slide: u8,
    pub details_shown: Option<DetailsShown>,
    /// The summon flight while it plays (`duel_summon`).
    pub summon_flight: Option<Box<crate::duel_summon::SummonFlight>>,
    /// Sound requests of the updates since the presentation last took them.
    pub audio: Vec<crate::duel_audio::AudioCue>,
    /// A trap or magic activation being presented (`duel_activate`).
    pub activation: Option<Box<crate::duel_activate::Activation>>,
    /// Each board card's +0x55 (0 upright .. 0xFF sideways), stepped 0x20 per
    /// update toward its defence flag.
    sideways: BTreeMap<roses_core::UnitId, u8>,
    /// A monster flip effect being presented (`duel_monster_fx`), before
    /// the battle it interrupts.
    pub monster_fx: Option<Box<crate::duel_monster_fx::MonsterShow>>,
    /// The battle opening waiting for a monster effect's presentation:
    /// the step's events, board, positions and faces from before it.
    deferred_battle: Option<DeferredBattle>,
    /// A trap sprung on a cell the walker entered (walker states 0x36 /
    /// 0x37): its presentation follows the walk, from the board before
    /// the step with the walker on that cell.
    deferred_activation: Option<Box<DeferredActivation>>,
    /// A magic card's own event's presentation, started when its
    /// activation (c602, c601) ends.
    after_activation: Option<Box<crate::duel_monster_fx::MonsterShow>>,
    /// A battle's death effect (event 9 after the battle scene).
    after_battle: Option<Box<crate::duel_monster_fx::MonsterShow>>,
    reported_activation: BTreeSet<&'static str>,
    reported_cards: BTreeSet<u16>,
    reported_scene: bool,
    /// Riryoku's c537 params (summon chain order) waiting for their pair.
    riryoku: Vec<u32>,
    /// The running Riryoku pair's module (c537's number and library) and
    /// its update (`duel_equip::EquipFx::riryoku`).
    pub equip_number: Option<Box<(crate::duel_equip::EquipFx, u32)>>,
    /// The HUD's duel while a Riryoku pair plays: the monster's stats, the
    /// opponent's LP and the graveyard change at its u 23 [F 8.4].
    summon_duel: Option<roses_core::duel::Duel>,
    /// An online duel: the server connection; `session.duel` is then the
    /// seat's copy rebuilt from its view (`duel_online`).
    pub online: Option<Box<crate::duel_online::OnlineLink>>,
    /// Online: each seat's name for the HUD's plates (the account names,
    /// the computer as its deck's character); none offline (the
    /// original's PLAYER / ENEMY plates).
    pub seat_names: Option<[String; 2]>,
    /// Online: units of the batch being presented whose summon this seat
    /// does not see (their flight shows the card's back).
    hidden_summons: BTreeSet<roses_core::UnitId>,
    /// The duel's opening (states 3..=0x12) while it plays: the session
    /// waits; see [`crate::duel_opening`].
    pub opening: Option<crate::duel_opening::DuelOpening>,
    /// The opening's board camera as the renderer moves it (the same
    /// requests and steps), for c620's camera.
    opening_camera: Option<BoardCamera>,
    /// Both leaders' c620 while it runs (`duel_opening_fx`).
    pub opening_fx: [Option<Box<crate::duel_opening_fx::LeaderEffect>>; 2],
    /// An N-player duel (a port addition, MODES; `duel_multi`): its seats'
    /// names and cameras, the HUD pair, the offline preview's computers.
    /// Always `None` in a 2-player duel and without the `online` feature.
    pub multi: Option<Box<crate::duel_multi::MultiPlay>>,
}

/// The index of `side` in the duel's per-side lists: the side itself in an
/// N-player duel (a port addition), else `side & 1` as the original's
/// two-side tables read it (the same value for sides 0 and 1).
pub fn side_index(duel: &Duel, side: u8) -> usize {
    if duel.multi().is_some() {
        usize::from(side).min(duel.sides.len().saturating_sub(1))
    } else {
        usize::from(side & 1)
    }
}

/// The board, units and input state from before a change, for its
/// presentation (taken before a local frame, or before an online
/// revision with the input state from before the press that caused it).
struct Before {
    mode: Mode,
    hand: Vec<u16>,
    field_chain: Option<u16>,
    selection: Option<(roses_core::UnitId, Vec<GridPos>)>,
    overlay: OverlayInput,
    positions: Vec<(roses_core::UnitId, (u8, u8), GridPos)>,
    faces: Vec<(roses_core::UnitId, (u16, u8, i32, i32), bool)>,
    duel: Duel,
}

/// Turn-to-face step of 0x1DED30: a unit's yaw offset (record +0x30) moves
/// by at most 0x800 per call toward the angle of the other unit's cell
/// (rounded to a multiple of 0x800) minus its side's base yaw (0x8000 for
/// side 0) [F 0x1DED30, emu K2: 0 -> 0x4000 for both units].
pub const TURN_STEP: i32 = 0x800;
/// Updates of the battle camera track (0x1DA430: K2 from update 7 to 23) [emu].
pub const BATTLE_CAMERA_FRAMES: u32 = 16;

/// Event 0x72's length with the interpreter's stand-ins (about 34 updates)
/// [emu SPECIAL-DRAW.md T8; H real timing].
pub const DESTINY_EVENT_UPDATES: u32 = 34;

/// Dark Hole (magic 699: type 4, hi 0; event 4, handler 0x258C60).
pub const DARK_HOLE: u16 = 699;

/// d3a of the details view at rest (0x20 stops at 0x40) [F 0x1D4C04].
pub const DETAILS_REST: u8 = 0x40;

/// What overlay slot 1 code 0x11 draws (0x1B0120 case 0x1B1338): the card,
/// the page 0x2DB85D, the state the view returns to (d35) and the block
/// 0x2154C0 / 0x215E60 read (D5C: the field unit's side, the shown
/// graveyard list, else the active side d54) [F 0x1B1338..0x1B13E4].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DetailsShown {
    pub view: roses_core::duel::ui::DetailsView,
    pub d35: u8,
    pub d5c: u8,
}

/// A camera move for the board camera: target and polar (radians) over N
/// updates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraMove {
    pub target: [f64; 3],
    pub polar: [f64; 3],
    pub frames: u32,
}

/// The opening of a battle as the original plays it before the battle scene
/// (event 0x5E): the attacker and the defender turn to face each other and
/// the camera moves to the battle camera. The rules engine has already
/// resolved the battle, so the board shows the state from before it
/// (`frozen`) while the opening plays; the battle scene itself is not drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct BattleIntro {
    pub frozen: OverlayInput,
    /// ((side, slot), yaw offset now, target) for both units.
    pub turns: Vec<((u8, u8), i32, i32)>,
    pub frame: u32,
    /// The battle camera the opening moves to.
    pub camera: CameraMove,
    /// The two cards of the versus screen (attacker, defender): side, cell
    /// and (card, level, ATK, DEF) as the units have them before the
    /// battle. `None` for a battle with a leader (not traced).
    pub versus_cards: Option<[VersusSource; 2]>,
    /// Update (of `frame`) on which the camera and the turns finished.
    pub arrived: Option<u32>,
    /// The card-versus screen once its cards fly.
    pub versus: Option<crate::duel_versus::Versus>,
    /// The return's mode (`duel_return::mode`) and the LP change (negative,
    /// 0 for none), from the engine's `Damage` / `Destroyed` events.
    pub damage: (u8, i32),
    /// c605's mode: bit 1 attacker destroyed, bit 0 defender destroyed.
    pub destroyed: u8,
    /// The arena's terrain: the defender's cell's (the battle cell, cell
    /// table +0x16 through 0x1E3530 [H: which unit's cell]).
    pub arena_terrain: u8,
    /// What the attack effects' outcome boundary reads (`effect_battle`).
    pub effect_battle: Option<roses_fx::types::BattleState>,
    /// The update the arena ends on (set when it starts, extended by the
    /// attack phase's length when that starts).
    pub arena_end: Option<u32>,
    /// A leader hit (event 0x5D, handler 0x275790): no 0x1F4 load hold, the
    /// card versus screen one update later, the attacker's card only, no
    /// arena, c604 straight after the fly-in [F/emu LEADER-BATTLE.md].
    pub leader: bool,
    /// The scene lights (0x1F7F00 / 0x1F7EF0, 128 = full): dimmed to 0 by
    /// 15 per update before the card versus screen, raised by 15 from c605's
    /// start [F/emu LEADER-BATTLE.md: 113 .. 8, 0; 15 .. 120, 128].
    pub light: u8,
    /// The music after the scene (k * 1000 + 8, k = 1..3 drawn at its start).
    pub after_track: Option<u32>,
    /// Both units' faces, shown face up from the unit camera on (a
    /// face-down defender is revealed: its model shows in the footage's
    /// close-up) [emu dumps-battle-frames 83-106; the flip itself is not
    /// traced].
    pub revealed: Vec<((u8, u8), duel_overlay::BoardFace)>,
    /// Dark Elf (battle pre-pass type 0xD, no event): the flagged units
    /// (side, cell); box 0x105 "Effect activated!" and c513 on each at the
    /// scene's first stage, the scene going on when they end (+17 U)
    /// [F NEWLY-ALLOWED.md 8.2; its timing against the scene's own E H].
    pub effect_units: Vec<(u8, (u8, u8))>,
    pub effect_c513: Vec<crate::duel_monster_mods::C513>,
}

/// An activation waiting for the walk that sprang it: the step's events,
/// the frozen board, positions and duel.
type DeferredActivation = (
    Vec<roses_core::duel::DuelEvent>,
    OverlayInput,
    Vec<(roses_core::UnitId, (u8, u8), GridPos)>,
    roses_core::duel::Duel,
);

/// The window caption's name of a campaign opponent (controller byte).
pub fn opponent_name(opponent: u8) -> &'static str {
    match opponent {
        1 => "Seto",
        2 => "Weevil",
        3 => "Rex Raptor",
        4 => "Keith",
        5 => "Ishtar",
        6 => "Necromancer",
        7 => "Darkness-ruler",
        8 => "Labyrinth-ruler",
        9 => "Pegasus",
        10 => "Richard Slysheen of York",
        20 => "Manawyddan fab Llyr",
        _ => "the opponent",
    }
}

/// 0x1AB440 code 1: the duel's track by scene params 1 (opponent) and 2
/// [F 0x1AB440 jump table 0x2BDA80].
pub fn duel_music_track(param1: u8, param2: u32) -> u32 {
    match param1 {
        0 => 7,
        1 => 10,
        0x13 => 11,
        0x14 => 12,
        0x15 => 44,
        p if p < 0x16 => {
            if param2 != 0 { 9 } else { 8 }
        }
        _ => 40,
    }
}

/// A battle opening deferred behind a monster effect's presentation.
type DeferredBattle = (
    Vec<roses_core::duel::DuelEvent>,
    OverlayInput,
    Vec<(roses_core::UnitId, (u8, u8), GridPos)>,
    Vec<(roses_core::UnitId, (u16, u8, i32, i32), bool)>,
    Option<roses_fx::types::BattleState>,
);

impl BattleIntro {
    /// Whether each versus card's unit lies in defence on the board.
    fn defence(&self, cards: &[VersusSource; 2]) -> [bool; 2] {
        cards.map(|(side, cell, _)| {
            self.frozen
                .units
                .iter()
                .any(|u| u.side == side && (u.cell[0], u.cell[1]) == cell && u.slot != 0 && u.defence)
        })
    }
}

/// A versus card's unit: side, cell and (card, level, ATK, DEF).
pub type VersusSource = (u8, (u8, u8), (u16, u8, i32, i32));

/// The battle scene's unit camera 0x262410(frames, unit, add, cell
/// distance) [F, CAMERA-TRACKS.md; emu footage 005-008].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnitCamera {
    pub side: u8,
    pub cell: (u8, u8),
    pub card: u16,
    /// The unit's yaw offset (record +0x30).
    pub turn: i32,
    /// Distance added to the model's bounding radius (300 far, 100 near).
    pub add_distance: i32,
    /// Distance for a unit without a model (280 far, 180 near).
    pub cell_distance: i32,
    /// 0 = snap on the next update.
    pub frames: u32,
    /// The side whose turn it is (0x2DB8F4).
    pub active_side: u8,
}

/// The camera of a [`UnitCamera`]. With the unit's model (`radius` = its
/// node 0 bounding-sphere radius, object +0x1E4 [F 0x1E9F78]) the camera
/// follows the object (kind 7; its position (x, 50, z), the animated
/// +0x80 bob of a few units is not modelled [H]) at distance add + radius,
/// yaw = base + (0x47FF - turn) (0x147FE when negative), pitch 0x2000.
/// Without a model it looks at the cell (x, 0, z) from the cell distance,
/// yaw 0x3FFF (active side 0) or 0xBFFF, pitch 0x1000.
pub fn unit_camera(c: &UnitCamera, radius: Option<f32>) -> CameraMove {
    let [x, z] = crate::duel_board::cell_center(c.cell.0, c.cell.1);
    let angle = |v: i32| f64::from(v as f32 * std::f32::consts::PI / 32768.0);
    match radius {
        Some(r) => {
            let base = side_model_base(c.side);
            let bracket = 0x47FF - c.turn;
            let yaw = base
                + if bracket < 0 {
                    0x147FE - c.turn
                } else {
                    bracket
                };
            CameraMove {
                target: [x, 50.0, z],
                polar: [
                    f64::from(c.add_distance as f32 + r),
                    angle(yaw).rem_euclid(std::f64::consts::TAU),
                    angle(0x2000),
                ],
                frames: c.frames,
            }
        }
        None => CameraMove {
            target: [x, 0.0, z],
            polar: [
                f64::from(c.cell_distance),
                angle(side_yaw(c.active_side) - 1),
                angle(0x1000),
            ],
            frames: c.frames,
        },
    }
}

/// Updates at the battle camera before the battle scene's unit camera
/// starts. It waits for the scene's code modules to load from the disc:
/// 36 in the PCSX2 footage (dumps-battle-frames 23 -> 59), 19 in the
/// interpreter run (K2 23 -> 42) [emu].
pub const SCENE_LOAD_HOLD: u32 = 36;
/// The unit camera: 24 updates on the attacker, then 24 on the defender
/// (0x2760E0 sub-phases 0 and 1, counter +0x102) [F; footage 59 -> 107].
pub const UNIT_CAMERA_UPDATES: u32 = 48;
/// Back at the battle camera until c604 launches (K2 90 -> 101, footage
/// 107 -> 118) [emu].
pub const VERSUS_LAUNCH_HOLD: u32 = 11;
/// The versus cards at rest before the arena crossfade (footage 138 ->
/// 209) [emu, PCSX2 load timing].
pub const VERSUS_REST_HOLD: u32 = 71;

/// c604's start: both card objects from the units' board cards, their
/// targets through the battle camera's view [F c604 init].
fn start_versus(
    camera: &CameraMove,
    cards: &[VersusSource; 2],
    defence: [bool; 2],
) -> Option<crate::duel_versus::Versus> {
    use crate::duel_versus::{Versus, VersusCard, board_card_pose};
    let view = crate::duel_board::straight_down_view(camera.target, camera.polar[0])?;
    let view: roses_assets::opening_scene::Matrix = view.map(|r| r.map(|v| v as f32));
    let card = |i: usize| {
        let (side, cell, face) = cards[i];
        VersusCard::new(i, board_card_pose(side, cell, defence[i]), &view, camera.polar, face)
    };
    Some(Versus {
        cards: [card(0)?, card(1)?],
        frame: 0,
        ret: None,
        end: None,
        arena: None,
        single: false,
    })
}

/// 16-bit angle of a world direction as 0x1DED30 / 0x1DA430 take it:
/// atan2(x, z) scaled to 65536 per turn.
fn angle16(dx: f64, dz: f64) -> i32 {
    (dx.atan2(dz) * 65536.0 / std::f64::consts::TAU) as i32
}

/// The yaw offset that makes a unit of `side` on `from` face `to`.
pub fn face_target(side: u8, from: GridPos, to: GridPos) -> i32 {
    let dx = (f64::from(to.column) - f64::from(from.column)) * 50.0;
    let dz = (f64::from(to.row) - f64::from(from.row)) * 50.0;
    let yaw = ((angle16(dx, dz) + 0x400) >> 11) << 11;
    let base = side_model_base(side);
    ((yaw - base) as i16) as i32
}

/// A versus card's attack value (the engine's, shown on the card).
fn cards_attack(cards: &[crate::duel_versus::VersusCard; 2], i: usize) -> i32 {
    cards[i].attack
}

impl BattleIntro {
    /// Updates before the unit camera: 0x5E's 0x1F4 load hold, none for a
    /// leader hit (0x5D) [F LEADER-BATTLE.md].
    pub fn load_hold(&self) -> u32 {
        self.scene_hold() + self.effect_stage()
    }

    /// The scene's load (0x1F4) before its first stage.
    pub fn scene_hold(&self) -> u32 {
        if self.leader { 0 } else { SCENE_LOAD_HOLD }
    }

    /// Dark Elf's stage: c513 (16 U), the box closed the update after.
    pub fn effect_stage(&self) -> u32 {
        if self.effect_units.is_empty() { 0 } else { crate::duel_monster_mods::C513::UPDATES + 1 }
    }

    /// The scene's update after the camera arrived (0 = arrival).
    fn scene_t(&self) -> Option<u32> {
        self.arrived.map(|at| self.frame - at)
    }

    /// Box 0x105 while Dark Elf's stage plays (x 0, y 0x50).
    pub fn effect_box(&self) -> Option<u16> {
        let t = self.scene_t()?.checked_sub(self.scene_hold())?;
        (self.versus_cards.is_some() && t < self.effect_stage()).then_some(0x105)
    }

    /// Dark Elf's c513 triangles.
    pub fn effect_tris(&self, eye: [f64; 3], target: [f64; 3]) -> Vec<crate::duel_c621::FxTri> {
        self.effect_c513.iter().flat_map(|m| m.draw(eye, target)).collect()
    }
}

/// The board camera moves of a presentation's camera step (`duel_monster_fx`
/// / a trap plan): (unit camera, camera command).
pub fn camera_for_step(step: crate::duel_monster_fx::CameraStep, active_side: u8, cursor: GridPos) -> (Option<UnitCamera>, Option<CameraMove>) {
    use crate::duel_monster_fx::CameraStep;
    let angle = |v: i32| f64::from(v as f32 * std::f32::consts::PI / 32768.0);
    match step {
        CameraStep::Unit(t, frames) => (
            Some(UnitCamera {
                side: t.unit.0,
                cell: t.cell,
                card: if t.face_up { t.card } else { NO_MODEL },
                turn: 0,
                add_distance: 100,
                cell_distance: 180,
                frames,
                active_side,
            }),
            None,
        ),
        CameraStep::Cell(t, frames, distance) => (
            Some(UnitCamera {
                side: t.unit.0,
                cell: t.cell,
                card: NO_MODEL,
                turn: 0,
                add_distance: 100,
                cell_distance: distance as i32,
                frames,
                active_side,
            }),
            None,
        ),
        CameraStep::Overview(frames) => {
            let yaw = side_yaw(active_side) - 1;
            (None, Some(CameraMove { target: [0.0; 3], polar: [500.0, angle(yaw), angle(0x17FF)], frames }))
        }
        CameraStep::Back(frames) => (
            None,
            Some(CameraMove {
                target: crate::duel_board::cursor_target((cursor.column, cursor.row)),
                polar: crate::duel_board::default_polar(),
                frames,
            }),
        ),
        // Relative to the live camera: `DuelPlay::camera_orbit`.
        CameraStep::Orbit(..) => (None, None),
    }
}

/// 0x2622B0(frames, dyaw): every track cleared (the target stays where it
/// is), then a polar track to the current polar with yaw + dyaw [F].
pub fn orbit_camera(camera: &mut BoardCamera, frames: u32, dyaw: i32) {
    let mut polar = camera.polar;
    polar[1] += f64::from(dyaw as f32 * std::f32::consts::PI / 32768.0);
    let target = camera.target;
    camera.track_to(target, polar, frames);
}

/// Whether a unit's battle pre-pass (0x263AB0 on its e0 words) returns bit
/// 16 ("effect"), which 0x263840 passes to the scene record (+0xC bits 20 /
/// 22): the scene then opens with box 0x105 "Effect activated!" and c513 on
/// the unit [F 0x263AB0, NEWLY-ALLOWED.md 8.2]. For the types the rules
/// engine supports: 0xD (Dark Elf) and 2 (terrain) always; 5 (Blocker's
/// damage shield) and 1 (Jirai Gumo) between two non-leaders; 4 (Rogue
/// Doll, mode 2) and 0x1B when the other monster's kind matches the filter /
/// hi, between two non-leaders. `other_kind` is the other card's kind byte
/// when it is a monster [F; emu B1-B3 for Blocker].
pub fn prepass_flags(words: [u32; 2], own_slot: u8, other_slot: u8, other_kind: Option<u8>) -> bool {
    let both = own_slot != 0 && other_slot != 0;
    let kind = other_kind.map(|k| u32::from(k & 0x1F));
    match words[0] >> 8 {
        0xD | 2 => true,
        5 | 1 => both,
        4 => both && (words[0] >> 6) & 3 == 2 && kind == Some(words[0] & 0x3F),
        0x1B => both && kind == Some(words[1] >> 16),
        _ => false,
    }
}

/// Event 0x36's init cut 0x262630(0, trap, 180, 180): the trap's cell, polar
/// (180, 112.5 deg (active side 1: 292.5), 33.75 deg) (0x257890: yaw 0x4FFF /
/// 0xCFFF, pitch 0x1800) [F/emu NEWLY-ALLOWED.md 1.1].
pub fn trap_cut(cell: (u8, u8), active_side: u8) -> CameraMove {
    let [x, z] = crate::duel_board::cell_center(cell.0, cell.1);
    let angle = |v: i32| f64::from(v as f32 * std::f32::consts::PI / 32768.0);
    let yaw = side_yaw(active_side) + 0xFFF;
    CameraMove { target: [x, 0.0, z], polar: [180.0, angle(yaw), angle(0x1800)], frames: 0 }
}

/// c605's sound (0, 2) at its 21st update (d0 == 20) [F/emu LEADER-BATTLE.md].
fn burn_sound_at(start: u32, mode: u8) -> u32 {
    start + crate::duel_return::Return::length(mode) + 20
}

/// The system sound of a handled press in the input state it was read in
/// (DUEL-AUDIO-SCOPE.md 4.3: 3 cursor, 4 confirm, 5 cancel, 6 refused, 9
/// toggle, 0x10 details) [F sites; per-key reading H].
fn input_sound(state: u8, key: u16, response: &Response) -> Option<u8> {
    use roses_core::duel::ai::pad;
    if matches!(response, Response::Refused) {
        return Some(6);
    }
    if !matches!(response, Response::Moved | Response::Commands(_)) {
        return None;
    }
    let arrows = pad::UP | pad::DOWN | pad::LEFT | pad::RIGHT;
    match state {
        0x18 if key & (pad::CROSS | pad::SQUARE) != 0 => Some(4),
        0x18 if key & pad::TRIANGLE != 0 => Some(0x10),
        // 0x18: 0x1D4120 after the cursor step [F; emu 011-t11 held Right].
        0x18 | 0x1D | 0x3E if key & arrows != 0 => Some(3),
        0x43 if key & (pad::LEFT | pad::RIGHT) != 0 => Some(3),
        0x43 if key & (pad::UP | pad::DOWN) != 0 => Some(9),
        0x43 if key & pad::TRIANGLE != 0 => Some(0x10),
        0x1D | 0x3E | 0x43 | 0x46 if key & pad::CROSS != 0 => Some(4),
        0x1D | 0x3E | 0x43 | 0x46 if key & pad::CIRCLE != 0 => Some(5),
        _ => None,
    }
}

/// One turn step toward `target` (wrapped difference, at most 0x800).
pub fn turn_step(current: i32, target: i32) -> i32 {
    let delta = ((target - current) as i16) as i32;
    ((current + delta.clamp(-TURN_STEP, TURN_STEP)) as i16) as i32
}

/// The battle camera of 0x1DA430 for the attacker's and defender's cells:
/// target the midpoint (y 40), distance 150, pitch 0x4000, yaw =
/// round_0x100(atan2(drow * 50, dcol * 50)) + 0x4000 [F 0x1DA430; emu K2
/// yaw 4.712, footage yaw pi].
pub fn battle_camera(attacker: GridPos, defender: GridPos) -> CameraMove {
    let [ax, az] = crate::duel_board::cell_center(attacker.column, attacker.row);
    let [bx, bz] = crate::duel_board::cell_center(defender.column, defender.row);
    let drow = (f64::from(defender.row) - f64::from(attacker.row)) * 50.0;
    let dcol = (f64::from(defender.column) - f64::from(attacker.column)) * 50.0;
    let a = (drow.atan2(dcol) * 65536.0 / std::f64::consts::TAU) as i32;
    let yaw16 = ((((a + 0x80) >> 8) << 8) + 0x4000) & 0xFFFF;
    let r = |v: i32| f64::from(v) * std::f64::consts::TAU / 65536.0;
    CameraMove {
        target: [
            (ax + bx) / 2.0,
            crate::duel_board::CAMERA_TARGET_Y,
            (az + bz) / 2.0,
        ],
        polar: [150.0, r(yaw16), r(0x4000)],
        frames: BATTLE_CAMERA_FRAMES,
    }
}

/// A [`UnitCamera`] card id for a unit that shows no model (face down):
/// the camera's cell branch.
pub const NO_MODEL: u16 = u16::MAX;

/// A side's board camera setting (side record +0x1D0 / +0x1D4 / +0x1D8,
/// 0x2DABA0 + 0x350 * side): distance, yaw and pitch, 16-bit angles.
/// The original's values in the duel savestate 011-t11 are side 0 (200,
/// 0x4000, 0x2000) and side 1 (200, 0xC000, 0x2000) [emu], the values L3
/// resets to [F]: each side's turn is seen from its own end
/// ([`TURN_CAMERA_FRAMES`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SideCamera {
    pub distance: i32,
    pub yaw: i32,
    pub pitch: i32,
}

impl Default for SideCamera {
    fn default() -> Self {
        Self {
            distance: crate::duel_board::CAMERA_DISTANCE as i32,
            yaw: i32::from(crate::duel_board::CAMERA_YAW_SIDE0),
            pitch: i32::from(crate::duel_board::CAMERA_PITCH),
        }
    }
}

/// A side's model facing (unit record +0x30's base): 0x8000 for side 0,
/// 0 for side 1; an N-player seat's own (a port addition).
pub fn side_model_base(side: u8) -> i32 {
    #[cfg(feature = "online")]
    if side > 1 {
        return crate::duel_multi::model_yaw(side);
    }
    if side == 0 { 0x8000 } else { 0 }
}

/// A side's board camera yaw (L3's reset): 0x4000, or 0xC000 for side 1;
/// an N-player seat's own end (a port addition, `duel_multi::seat_yaw`).
pub fn side_yaw(side: u8) -> i32 {
    #[cfg(feature = "online")]
    if side > 1 {
        return crate::duel_multi::seat_yaw(side);
    }
    0x4000 + if side == 1 { 0x8000 } else { 0 }
}

impl SideCamera {
    /// A side's starting setting: side 1 looks from the other end.
    pub fn for_side(side: u8) -> Self {
        let mut c = Self::default();
        if side & 1 == 1 {
            c.yaw += 0x8000;
        }
        c
    }
    /// The float polar (0x1FDB60's conversion; the same scale as
    /// `duel_board::default_polar`, which the default setting equals).
    pub fn polar(&self) -> [f64; 3] {
        let r = |v: i32| f64::from(v) * std::f64::consts::TAU / 65536.0;
        [f64::from(self.distance), r(self.yaw), r(self.pitch)]
    }
}

/// The turn camera 0x1DAAA0(_, side) [F; emu 011-t11 Start]: at the duel's
/// start (0x1D02CC, 0x1D0808) and at each turn change (0x1D3728, side ^ 1)
/// it makes `side` active and starts 32-update tracks of the target to
/// that side's cursor cell ((column - 3) * 50, side record +0x345 = 40,
/// (row - 3) * 50) and of the polar to that side's camera setting (+0x1D0).
/// Measured: side 0's Start in state 0x18, state 0x14 three updates later
/// starts kind 3 (0, 40, -150) and kind 4 (200, 4.7124, 0.7854) over 32.
pub const TURN_CAMERA_FRAMES: u32 = 32;

/// Stick past the dead zone (0x1D3D54 & co: |byte - 0x80| >= 0x30).
pub const STICK_DEAD_ZONE: i32 = 0x30;
/// Updates of the camera track a stick step starts (`$s4` = 8).
pub const STICK_TRACK_FRAMES: u32 = 8;
/// Updates of the track an L3 / R3 reset starts (`$s4` = 0x1E).
pub const STICK_RESET_FRAMES: u32 = 30;

/// The duel's analog camera, state 0x18 of the dispatcher 0x1D00F0
/// (0x1D3C7C-0x1D3EE0) [F], run when no camera track is running
/// (0x1FB900(-1, -1) = 0) on the active side's setting `cam`:
/// - L3 newly pressed (`$s5` = the new-press word): yaw 0x4000 (+0x8000
///   when the active side is 1), pitch 0x2000; the left stick is not read;
/// - otherwise the left stick X past the dead zone turns the yaw by
///   -0x1000 to the right, +0x1000 to the left (the yaw first wrapped to
///   0..0xFFFF), and its Y tilts the pitch by -0x800 down, +0x800 up,
///   clamped to 0x800..0x4000;
/// - R3 newly pressed: distance 200; otherwise the right stick Y moves the
///   distance by +0x32 down, -0x32 up, clamped to 0x32..0x1F4;
///
/// the right stick X (index 0) is read and unused. Any change starts a
/// camera track to the new setting (0x1FDB60, 0x1FBAB0(0, 0, 4, frames)):
/// 30 updates after L3 / R3, else 8. Returns the track's length.
pub fn stick_camera(cam: &mut SideCamera, presses: u16, sticks: [u8; 4], active_side: u8) -> Option<u32> {
    use roses_core::duel::ai::pad as bits;
    let centred = |b: u8| i32::from(b) - 0x80;
    let (ry, lx, ly) = (centred(sticks[1]), centred(sticks[2]), centred(sticks[3]));
    let mut frames = STICK_TRACK_FRAMES;
    let mut changed = false;
    if presses & bits::L3 != 0 {
        cam.yaw = side_yaw(active_side);
        cam.pitch = 0x2000;
        frames = STICK_RESET_FRAMES;
        changed = true;
    } else {
        if lx.abs() >= STICK_DEAD_ZONE {
            cam.yaw = ((cam.yaw + 0x10000) & 0xFFFF) + if lx >= STICK_DEAD_ZONE { -0x1000 } else { 0x1000 };
            changed = true;
        }
        if ly.abs() >= STICK_DEAD_ZONE {
            cam.pitch = (cam.pitch + if ly >= STICK_DEAD_ZONE { -0x800 } else { 0x800 }).clamp(0x800, 0x4000);
            changed = true;
        }
    }
    if presses & bits::R3 != 0 {
        cam.distance = 200;
        frames = STICK_RESET_FRAMES;
        changed = true;
    } else if ry.abs() >= STICK_DEAD_ZONE {
        cam.distance = (cam.distance + if ry >= STICK_DEAD_ZONE { 0x32 } else { -0x32 }).clamp(0x32, 0x1F4);
        changed = true;
    }
    changed.then_some(frames)
}

/// Drive the board camera for this update: a pending camera move starts
/// once; while a battle opening plays the camera is left to its tracks;
/// otherwise it follows `target` (the cursor) with the active side's
/// camera setting, which the analog sticks change ([`stick_camera`]).
pub fn drive_camera(
    camera: &mut BoardCamera,
    play: &mut DuelPlay,
    target: GridPos,
    radius: &dyn Fn(u16) -> Option<f32>,
) {
    // The burn module (c509) draws with this camera on the next update.
    if let Some(m) = play.monster_fx.as_mut() {
        m.view = Some((camera.eye(), camera.target));
    }
    if let Some(a) = play.activation.as_mut() {
        a.view = Some((camera.eye(), camera.target));
    }
    if let Some((frames, dyaw)) = play.camera_orbit.take() {
        orbit_camera(camera, frames, dyaw);
        return;
    }
    if play.exodia.is_some() {
        exodia_camera(camera, play, radius);
        return;
    }
    camera.zoom = 1.0;
    if let Some(c) = play.unit_camera.take() {
        let r = if c.card == NO_MODEL { None } else { radius(c.card) };
        let m = unit_camera(&c, r);
        camera.track_to(m.target, m.polar, m.frames);
        return;
    }
    if let Some(m) = play.camera_command.take() {
        camera.track_to(m.target, m.polar, m.frames);
        return;
    }
    if play.opening.is_some() || play.intro.is_some() || play.activation.is_some() || play.monster_fx.is_some() || play.knockout_playing() {
        return;
    }
    // A new active side (the duel's start, a turn change): its end of the
    // board, its cursor (0x1DAAA0).
    let side = play.session.duel.active;
    if play.camera_side != Some(side) {
        play.camera_side = Some(side);
        let c = (target.column, target.row);
        camera.cursor = c;
        camera.track_to(crate::duel_board::cursor_target(c), play.board_polar(), TURN_CAMERA_FRAMES);
        play.board_track = play.board_track.max(TURN_CAMERA_FRAMES - 1);
        return;
    }
    let rest = play.board_polar();
    if camera.polar_track.is_none() && camera.polar != rest {
        // Back from the battle camera [H: the original's return move after
        // the battle scene is not traced; a 16-update track].
        let c = (target.column, target.row);
        camera.cursor = c;
        camera.track_to(crate::duel_board::cursor_target(c), rest, BATTLE_CAMERA_FRAMES);
        return;
    }
    if camera.polar_track.is_some() {
        return;
    }
    // The sticks (state 0x18, no camera track running).
    if !camera.moving() {
        if let Some(sticks) = play.stick_input() {
            let side = play.session.duel.active;
            let presses = play.last_presses;
            let cam = play.side_camera_mut(side);
            if let Some(frames) = stick_camera(cam, presses, sticks, side) {
                let polar = cam.polar();
                camera.polar_track = Some(crate::duel_board::PolarTrack { start: camera.polar, end: polar, frames, elapsed: 0 });
                // Started in this update's state 0x18 and advanced once.
                play.board_track = play.board_track.max(frames - 1);
            }
        }
    }
    follow_cursor(camera, target);
}

/// The Exodia event's camera: the handler's unit cameras (0x262410 /
/// 0x2627D0), then Summoned Lord Exodia's CMOT (kind 8: eye, target and
/// field of view every update; the roll is not applied [H: none in its
/// CMOT's keys is non-zero]).
fn exodia_camera(camera: &mut BoardCamera, play: &mut DuelPlay, radius: &dyn Fn(u16) -> Option<f32>) {
    let active = play.session.duel.active;
    let Some(run) = play.exodia.as_mut() else { return };
    run.view = Some((camera.eye(), camera.target));
    if let Some(c) = run.lord.as_ref().and_then(|l| l.camera(run.event.side)) {
        let d: [f64; 3] = std::array::from_fn(|i| c.eye[i] - c.target[i]);
        let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let yaw = d[2].atan2(d[0]).rem_euclid(std::f64::consts::TAU);
        let pitch = d[1].atan2((d[0] * d[0] + d[2] * d[2]).sqrt());
        camera.track = None;
        camera.polar_track = None;
        camera.target = c.target;
        camera.polar = [dist, yaw, pitch];
        camera.zoom = c.distance / 800.0;
        return;
    }
    camera.zoom = 1.0;
    let Some(crate::duel_exodia::Cue::Camera { frames, side, slot, add, cell, low }) = run.camera.take() else { return };
    let Some(u) = play.session.duel.units.iter().find(|u| u.side == side && u.slot == slot) else { return };
    let c = UnitCamera {
        side,
        cell: (u.position.column, u.position.row),
        card: u.card,
        turn: 0,
        add_distance: add,
        cell_distance: cell,
        frames,
        active_side: active,
    };
    let r = radius(u.card);
    let mut m = unit_camera(&c, r);
    if low && r.is_some() {
        let (yaw, pitch) = crate::duel_exodia::low_angles(side_model_base(side), 0);
        let angle = |v: i32| f64::from(v as f32 * std::f32::consts::PI / 32768.0);
        m.polar[1] = angle(yaw).rem_euclid(std::f64::consts::TAU);
        m.polar[2] = angle(pitch);
    }
    camera.track_to(m.target, m.polar, m.frames);
}

/// Units a walking unit's +0x34/+0x36 offsets advance per dispatch of state
/// 0x37 (0x1D5E14): at most 5 toward the next cell, 50 per cell, so 10
/// dispatches per cell; on arrival 0x1E0FD0 commits the cell and zeroes the
/// offsets (movement-verified.md section 5) [V]. One dispatch per update
/// [H].
pub const WALK_STEP: f64 = 5.0;
/// Dispatches per cell (50 / [`WALK_STEP`]).
pub const WALK_DISPATCHES_PER_CELL: u32 = 10;

/// A unit sliding along the cells of its move. The rules engine has already
/// placed it on the last cell; the walk only moves what is drawn, and the
/// input states are not polled until it ends, as the original polls them
/// only between animations.
#[derive(Clone, Debug, PartialEq)]
pub struct Walk {
    pub unit: roses_core::UnitId,
    /// Origin first, destination last.
    pub cells: Vec<GridPos>,
    /// Dispatches played so far.
    pub dispatches: u32,
}

impl Walk {
    pub fn total(&self) -> u32 {
        WALK_DISPATCHES_PER_CELL * (self.cells.len().saturating_sub(1) as u32)
    }

    /// Drawn offset from the destination cell's centre (world x, z).
    pub fn offset(&self) -> [f64; 2] {
        let n = self.cells.len();
        if n < 2 {
            return [0.0; 2];
        }
        let d = self.dispatches.min(self.total());
        let segment = ((d / WALK_DISPATCHES_PER_CELL) as usize).min(n - 2);
        let travelled = f64::from(d - WALK_DISPATCHES_PER_CELL * segment as u32) * WALK_STEP;
        let centre = |p: GridPos| crate::duel_board::cell_center(p.column, p.row);
        let (a, b) = (centre(self.cells[segment]), centre(self.cells[segment + 1]));
        let end = centre(self.cells[n - 1]);
        std::array::from_fn(|k| {
            let step = (b[k] - a[k]).signum() * travelled.min((b[k] - a[k]).abs());
            a[k] + step - end[k]
        })
    }
}

impl DuelPlay {
    pub fn new(data: DuelData, seed: u64, computer_player: bool) -> Result<Self> {
        let session = start(&data, seed, computer_player)?;
        Self::with_session(data, session, seed)
    }

    /// A session built by the caller (the practice lessons).
    pub fn with_session(data: DuelData, session: DuelSession, seed: u64) -> Result<Self> {
        let board_terrain = session.duel.terrain.clone();
        Ok(Self {
            data,
            session,
            repeat: PadRepeat::default(),
            repeat2: PadRepeat::default(),
            held_pads: [0; 2],
            last_presses2: 0,
            seed,
            frames: 0,
            status: None,
            halted: None,
            last_presses: 0,
            board_terrain,
            terrain_logged: false,
            reported: BTreeSet::new(),
            auto_pages: false,
            operator: crate::tutorial::Operator::default(),
            last_script: None,
            stand_ins: Vec::new(),
            walk: None,
            intro: None,
            camera_command: None,
            camera_orbit: None,
            side_cameras: [SideCamera::for_side(0), SideCamera::for_side(1)],
            camera_side: None,
            sticks: [None; 2],
            board_track: 0,
            d63: 0,
            glow_hold: false,
            destiny_event: None,
            exodia: None,
            unit_camera: None,
            arena_available: false,
            arena_provider: None,
            arena_effects: None,

            hand_anim: None,
            hand_seen: [0; 4],
            details_slide: 0,
            details_shown: None,
            summon_flight: None,
            audio: Vec::new(),
            activation: None,
            sideways: BTreeMap::new(),
            monster_fx: None,
            deferred_battle: None,
            deferred_activation: None,
            after_activation: None,
            after_battle: None,
            reported_cards: BTreeSet::new(),
            reported_activation: BTreeSet::new(),
            reported_scene: false,
            riryoku: Vec::new(),
            equip_number: None,
            summon_duel: None,
            online: None,
            seat_names: None,
            hidden_summons: BTreeSet::new(),
            opening: None,
            opening_camera: None,
            opening_fx: [None, None],
            multi: None,
        })
    }

    /// An online duel for the seat `link` joined (after its start
    /// snapshot): the copy of the duel from the seat's view, the board and
    /// leaders from it, the presentation of a two-player Custom Duel
    /// (opponent byte 101: track 40; two pads; the match's Leader Ability /
    /// Card Display words; 1P / 2P WIN and DRAW result screens). A
    /// spectator's link (`OnlineLink::watch`) is presented from seat 0's end
    /// with both hands hidden.
    #[cfg(feature = "online")]
    pub fn online(mut data: DuelData, mut link: crate::duel_online::OnlineLink) -> Result<Self> {
        let (duel, _) = link
            .take_snapshot(&data.catalogue)
            .ok_or("the online match has not started")??;
        let seat = link.seat;
        let f = &mut data.first_duel;
        for s in 0..2u8 {
            let leader = duel
                .units
                .iter()
                .find(|u| u.side == s && u.slot == 0)
                .ok_or("a side without a leader")?;
            f.sides[usize::from(s)].leader = leader.card;
            // The other seat's deck is never known; the own deck's order
            // is the server's secret too.
            f.sides[usize::from(s)].cards = Vec::new();
            f.leader_abilities[usize::from(s)] = duel.sides[usize::from(s)].leader_abilities.clone();
        }
        f.leader_ranks = [duel.leader_ranks[0], duel.leader_ranks[1]];
        f.terrain = duel.terrain.clone();
        f.first = duel.first;
        f.opponent = crate::duel_online::ONLINE_OPPONENT;
        f.player_side = seat;
        f.computer_sides = Vec::new();
        f.custom_rules = None;
        f.options = Some(link.options().words());
        let session = DuelSession::new(duel, [None, None]);
        let mut play = Self::with_session(data, session, 0)?;
        play.online = Some(Box::new(link));
        Ok(play)
    }

    /// An online duel that may be an N-player match (a port addition,
    /// MODES): with the seats' names the HUD shows every player.
    #[cfg(feature = "online")]
    pub fn online_multi(data: DuelData, link: crate::duel_online::OnlineLink, names: Vec<String>) -> Result<Self> {
        let seat = (!link.spectator).then_some(link.seat);
        let mut play = Self::online(data, link)?;
        if play.session.duel.multi().is_some() {
            play.multi = Some(Box::new(crate::duel_multi::MultiPlay::new(&play.session.duel, seat, names)));
        }
        Ok(play)
    }

    /// The offline N-player preview (a test aid, `--multi-preview`): the
    /// computer players play every seat but `seat` (none: all of them) on
    /// this computer, one command every `pace` updates.
    #[cfg(feature = "online")]
    pub fn multi_preview(mut data: DuelData, duel: Duel, seat: Option<u8>, names: Vec<String>, pace: u32) -> Result<Self> {
        let f = &mut data.first_duel;
        for s in 0..2u8 {
            if let Some(leader) = duel.units.iter().find(|u| u.side == s && u.slot == 0) {
                f.sides[usize::from(s)].leader = leader.card;
            }
            f.sides[usize::from(s)].cards = Vec::new();
            f.leader_abilities[usize::from(s)] = duel.sides[usize::from(s)].leader_abilities.clone();
        }
        f.leader_ranks = [duel.leader_ranks[0], duel.leader_ranks[1]];
        f.terrain = duel.terrain.clone();
        f.first = duel.first;
        f.opponent = crate::duel_online::ONLINE_OPPONENT;
        f.player_side = seat.unwrap_or(0);
        f.computer_sides = Vec::new();
        f.custom_rules = None;
        f.options = Some([3, 0]);
        let computers: Vec<u8> = (0..duel.side_count()).filter(|&s| Some(s) != seat).collect();
        let mut multi = crate::duel_multi::MultiPlay::new(&duel, seat, names);
        multi.local = Some(crate::duel_multi::LocalMulti::new(&duel, &computers, pace));
        let session = DuelSession::new(duel, [None, None]);
        let mut play = Self::with_session(data, session, 0)?;
        play.multi = Some(Box::new(multi));
        Ok(play)
    }

    /// N-player, once per update before the presses are read: the Target
    /// key (keyboard T, `MultiPlay::target_key`; or L2 while R2 is held),
    /// and the placing list once the match is over (Cross, Circle or Start
    /// closes it after a moment, or it closes by itself: `MULTI_DONE`).
    #[cfg(feature = "online")]
    fn multi_update(&mut self, mut presses: u16) -> u16 {
        use crate::duel_multi::{RESULTS_MIN_UPDATES, RESULTS_UPDATES, MULTI_DONE};
        let duel_over = self.session.duel.outcome.is_some();
        let animating = self.animating();
        let Some(m) = self.multi.as_mut() else { return presses };
        m.note_leaders(&self.session.duel);
        // The OUT note counts down once the board is free (the battle that
        // knocked the player out and the knockout's pieces are over).
        if !animating {
            m.out_note = m.out_note.and_then(|(s, n)| n.checked_sub(1).map(|n| (s, n))).filter(|n| n.1 > 0);
        }
        let mut want = std::mem::take(&mut m.target_key);
        if presses & pad::L2 != 0 && self.held_pads[0] & pad::R2 != 0 {
            want = true;
            presses &= !pad::L2;
        }
        if duel_over && !animating {
            m.results += 1;
            let pressed = presses & (pad::CROSS | pad::CIRCLE | pad::START) != 0;
            let close = (m.results >= RESULTS_MIN_UPDATES && pressed) || m.results >= RESULTS_UPDATES;
            // The preview keeps its list on screen.
            if close && m.local.is_none() {
                self.halted = Some(MULTI_DONE.into());
            }
            return 0;
        }
        if want {
            self.choose_next_target();
        }
        presses
    }

    /// The Target key: aim this seat's single-target effects at the next
    /// living enemy (`DuelCommand::Target`), on its own turn in the field.
    #[cfg(feature = "online")]
    fn choose_next_target(&mut self) {
        let Some(me) = self.multi.as_ref().and_then(|m| m.seat) else { return };
        let duel = &self.session.duel;
        let input = &self.session.input;
        let free = duel.active == me
            && duel.outcome.is_none()
            && duel.pending.is_none()
            && duel.pending_summon.is_none()
            && matches!(input.mode, Mode::Field)
            && input.details.is_none()
            && input.surrender.is_none()
            && input.settings.is_none()
            && !self.animating();
        let Some(target) = crate::duel_multi::next_target(duel, me).filter(|_| free) else {
            self.audio.push(crate::duel_audio::AudioCue::Se { slot: 0, id: 6, skip: false });
            return;
        };
        let command = roses_core::duel::DuelCommand::Target { side: target };
        if let Some(link) = self.online.as_mut() {
            if !link.may_act() {
                return;
            }
            let input = self.session.input.clone();
            link.submit(input.clone(), input, 0, Response::Commands(vec![command]));
        } else {
            match self.session.duel.apply(me, &command, &self.data.catalogue) {
                Ok((next, _)) => self.session.duel = next,
                Err(e) => self.status = Some(format!("target: {e:?}")),
            }
        }
        self.audio.push(crate::duel_audio::AudioCue::Se { slot: 0, id: 4, skip: false });
    }

    /// N-player: each `KnockedOut` event notes "<NAME> OUT"; with "cards
    /// leave" its units' presentation waits for the board to be free
    /// (`knockout_step`). `before`: the duel before the step (the units
    /// still on the board); `faces`: their images then.
    #[cfg(feature = "online")]
    pub(crate) fn queue_knockouts(
        &mut self,
        events: &[roses_core::duel::DuelEvent],
        before: &Duel,
        faces: &[(roses_core::UnitId, (u16, u8, i32, i32), bool)],
    ) {
        let leave = self.session.duel.multi().is_some_and(|m| m.rules.cards_leave);
        let Some(m) = self.multi.as_mut() else { return };
        for e in events {
            let roses_core::duel::DuelEvent::KnockedOut { side, .. } = e else { continue };
            m.out_note = Some((*side, crate::duel_multi::OUT_NOTE_UPDATES));
            let units = crate::duel_multi::knockout_units(before, *side, faces);
            if leave && !units.is_empty() {
                let seed = self.frames as u32 ^ (u32::from(*side) << 8);
                m.knockouts_waiting.push(crate::duel_multi::KnockoutShow::new(*side, &units, seed));
            }
        }
    }

    /// One update of the knockout presentations: start the next waiting one
    /// when nothing else plays, step it (its sounds and camera); true while
    /// one plays.
    #[cfg(feature = "online")]
    fn knockout_step(&mut self) -> bool {
        let busy = {
            let Some(m) = self.multi.as_ref() else { return false };
            m.knockout.is_none() && m.knockouts_waiting.is_empty()
        };
        if busy {
            return false;
        }
        let others = self.opening.is_some()
            || self.intro.is_some()
            || self.destiny_event.is_some()
            || self.monster_fx.is_some()
            || self.activation.is_some()
            || self.walk.is_some()
            || self.summon_flight.is_some()
            || self.hand_anim.is_some();
        let active = self.session.duel.active;
        let cursor = self.cursor();
        let Some(m) = self.multi.as_mut() else { return false };
        if m.knockout.is_none() {
            if others {
                return false;
            }
            let show = m.knockouts_waiting.remove(0);
            eprintln!("Duel play: seat {} knocked out at update {}: {} units leave the board", show.side, self.frames, show.units.len());
            m.knockout = Some(show);
        }
        let Some(show) = m.knockout.as_mut() else { return false };
        let sounds = show.tick();
        let step = show.camera();
        let done = show.done();
        if done {
            m.knockout = None;
        }
        for id in sounds {
            self.audio.push(crate::duel_audio::AudioCue::Se { slot: 1, id, skip: false });
        }
        if let Some(step) = step {
            let (unit, command) = camera_for_step(step, active, cursor);
            if unit.is_some() {
                self.unit_camera = unit;
            }
            if command.is_some() {
                self.camera_command = command;
            }
        }
        true
    }

    /// The offline N-player preview's update: the acting computer's next
    /// command (applied here, presented as another seat's change online),
    /// or the local seat's pad through the input layer.
    #[cfg(feature = "online")]
    fn local_multi_press(&mut self, presses: u16) -> Option<(u8, String, std::result::Result<Frame, String>, Before)> {
        if self.session.duel.outcome.is_some() {
            return None;
        }
        let side = self.session.duel.active;
        let computer = self.multi.as_ref()?.local.as_ref()?.plays(side);
        if computer {
            let local = self.multi.as_mut()?.local.as_mut()?;
            let (side, command) = local.decide(&self.session.duel, &self.data.catalogue)?;
            let applied = match self.session.duel.apply(side, &command, &self.data.catalogue) {
                Ok(r) => {
                    local.accepted();
                    r
                }
                Err(_) => local.refused(&self.session.duel, side, &command, &self.data.catalogue)?,
            };
            let (next, events) = applied;
            let who = self.side_name(side);
            let before = self.capture_before(side);
            let turn_changed = next.active != side;
            self.session.duel = next;
            crate::duel_online::settle_input(&mut self.session.input, &self.session.duel, turn_changed);
            if let Some(cell) = crate::duel_online::remote_focus(&events, &self.session.duel) {
                let i = side_index(&self.session.duel, side);
                if let Some(c) = self.session.input.cursors.get_mut(i) {
                    *c = cell;
                }
            }
            return Some((side, who, Ok(Frame { key: None, response: None, events, script: None }), before));
        }
        if presses == 0 {
            return None;
        }
        let who = self.side_name(side);
        let before = self.capture_before(side);
        let input_before = self.session.input.clone();
        match self.session.input.press(&self.session.duel, &self.data.catalogue, presses) {
            Err(e) => {
                self.session.input = input_before;
                crate::duel_online::settle_input(&mut self.session.input, &self.session.duel, false);
                self.status = Some(format!("{who} {}: {e}", button_names(presses)));
                None
            }
            Ok(response @ Response::Commands(_)) => {
                let Response::Commands(commands) = &response else { unreachable!() };
                let mut next = self.session.duel.clone();
                let mut events = Vec::new();
                for c in commands {
                    match next.apply(side, c, &self.data.catalogue) {
                        Ok((d, e)) => {
                            next = d;
                            events.extend(e);
                        }
                        Err(e) => {
                            self.session.input = input_before;
                            self.status = Some(format!("{who}: refused: {e:?}"));
                            return None;
                        }
                    }
                }
                self.session.duel = next;
                Some((side, who, Ok(Frame { key: Some(presses), response: Some(response), events, script: None }), before))
            }
            Ok(response) => Some((side, who, Ok(Frame { key: Some(presses), response: Some(response), events: Vec::new(), script: None }), before)),
        }
    }

    #[cfg(not(feature = "online"))]
    fn local_multi_press(&mut self, _: u16) -> Option<(u8, String, std::result::Result<Frame, String>, Before)> {
        None
    }

    /// The online seat, in an online duel (none when watching).
    pub fn online_seat(&self) -> Option<u8> {
        self.online.as_ref().filter(|l| !l.spectator).map(|l| l.seat)
    }

    /// Watching an online duel (a spectator: no seat, nothing hidden known).
    pub fn spectating(&self) -> bool {
        self.online.as_ref().is_some_and(|l| l.spectator)
    }

    /// The small note an online duel shows over the HUD (a port addition):
    /// SPECTATING for a spectator (with how many watch), "N WATCHING" for a
    /// player while anyone watches; none otherwise and offline. While the
    /// duel waits for a seat the note says so: the board just stands still.
    pub fn online_note(&self) -> Option<String> {
        let link = self.online.as_ref()?;
        online_note_text(link.spectator, link.watchers(), link.away_note())
    }

    /// Online, before a scripted press (replays, captures): run updates
    /// (presenting what arrives) until the seat may act with nothing
    /// animating, the match is over, or `timeout` passes.
    pub fn online_wait_until_ready(&mut self, timeout: std::time::Duration) {
        let start = std::time::Instant::now();
        while start.elapsed() < timeout && self.halted.is_none() {
            let Some(link) = self.online.as_mut() else {
                return;
            };
            link.pump();
            let ready = link.may_act() || link.ended();
            if ready && !self.animating() {
                return;
            }
            self.press(0);
            if !self.animating() {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
    }

    /// A presentation is playing (the pad is not read, as in the
    /// original's animation states).
    pub fn animating(&self) -> bool {
        self.opening.is_some()
            || self.intro.is_some()
            || self.destiny_event.is_some()
            || self.monster_fx.is_some()
            || self.activation.is_some()
            || self.walk.is_some()
            || self.summon_flight.is_some()
            || self.hand_anim.is_some()
            || self.knockout_playing()
            || self.exodia.is_some()
    }

    /// The board during the Exodia event: revealed cards face up (their
    /// flip), the leader's card object on its cell at its level, and the
    /// limbs gone once the finale removed them.
    fn exodia_overlay(&self, run: &crate::duel_exodia::ExodiaRun) -> OverlayInput {
        use roses_core::duel::CardData;
        let mut input = overlay_input(&self.session);
        for u in &mut input.units {
            if let Some(flip) = run.reveal(u.side, u.slot) {
                if !u.face_up {
                    u.flip = flip;
                }
                u.face_up = true;
            }
        }
        {
            let ctx = self.hud_context();
            add_board_faces(&mut input, &self.session, &self.data.catalogue, &ctx);
        }
        let side = run.event.side;
        if run.event.removed {
            let limbs: Vec<u8> = run.event.limbs.iter().map(|l| l.slot).collect();
            input.units.retain(|u| !(u.side == side && limbs.contains(&u.slot)));
        } else if let Some(level) = run.leader_card.filter(|l| *l > 0) {
            // The leader's card object (0x1CED30) as a card on its cell, in
            // a free card slot of the side (the overlay keys units by slot).
            let free = (6..=10u8).rev().find(|s| !input.units.iter().any(|u| u.side == side && u.slot == *s));
            let card = self.data.catalogue.card(roses_core::duel::EXODIA_HEAD);
            if let (Some(slot), Some(card)) = (free, card) {
                let (column, row) = run.event.leader_cell;
                input.units.push(UnitInput {
                    side,
                    slot,
                    cell: [column, row],
                    card: roses_core::duel::EXODIA_HEAD,
                    face_up: true,
                    defence: false,
                    acted: false,
                    outline_pulse: false,
                    face: Some(duel_overlay::BoardFace { level: card.level, attack: i32::from(card.attack), defence: i32::from(card.defense), effect: false }),
                    offset: [0.0; 2],
                    turn: 0,
                    flip: None,
                    alpha: Some(level),
                    sideways: None,
                    spin: None,
                    model_scale: None,
                    emblem: None,
                });
            }
        }
        input
    }

    /// A duel won by Exodia (`EndReason::Exodia`) plays the event 0x6D
    /// first (`duel_exodia`); the result waits for it.
    fn start_exodia(&mut self, events: &[roses_core::duel::DuelEvent]) {
        let won = events.iter().find_map(|e| match e {
            roses_core::duel::DuelEvent::Ended { outcome } if outcome.reason == EndReason::Exodia => outcome.winner,
            _ => None,
        });
        if let Some(side) = won {
            self.exodia = crate::duel_exodia::ExodiaRun::start(self, side, false);
        }
    }

    /// N-player (a port addition): a knockout's presentation plays or
    /// waits for the board.
    pub fn knockout_playing(&self) -> bool {
        self.multi.as_ref().is_some_and(|m| {
            #[cfg(feature = "online")]
            {
                m.knockout.is_some() || !m.knockouts_waiting.is_empty()
            }
            #[cfg(not(feature = "online"))]
            {
                let _ = m;
                false
            }
        })
    }

    fn capture_before(&self, side: u8) -> Before {
        let duel = &self.session.duel;
        Before {
            mode: self.session.input.mode.clone(),
            hand: duel.sides[side_index(duel, side)].hand.clone(),
            field_chain: duel
                .pending_summon
                .and_then(|p| duel.unit_at(p).filter(|u| u.side == side).map(|u| u.card)),
            selection: match &self.session.input.mode {
                Mode::Move(sel) => Some((sel.unit, sel.path.clone())),
                _ => None,
            },
            overlay: self.overlay_input(),
            positions: duel.units.iter().map(|u| (u.id, (u.side, u.slot), u.position)).collect(),
            faces: self.unit_faces(),
            duel: duel.clone(),
        }
    }

    /// One online update after the animations: the server's next batch
    /// (the local press's revisions, the other seat's press with or without
    /// revisions, or a server change) with its presentation context, or the
    /// local pad through the input layer (intentions go to the server, the
    /// press is streamed to the other seat). `None`: nothing to present.
    #[cfg(not(feature = "online"))]
    fn online_press(&mut self, _: u16) -> Option<(u8, String, std::result::Result<Frame, String>, Before)> {
        None
    }

    #[cfg(feature = "online")]
    fn online_press(&mut self, presses: u16) -> Option<(u8, String, std::result::Result<Frame, String>, Before)> {
        use crate::duel_online::{batch_events, remote_focus, remote_response, settle_input, Batch};
        use roses_net::presence::plausible;
        let link = self.online.as_mut()?;
        link.pump();
        let seat = link.seat;
        if let Some(snapshot) = link.take_snapshot(&self.data.catalogue) {
            match snapshot {
                Ok((duel, revision)) => {
                    let turn_changed = duel.active != self.session.duel.active;
                    self.session.duel = duel;
                    settle_input(&mut self.session.input, &self.session.duel, turn_changed);
                    self.status = Some(format!("resynchronised at revision {revision}"));
                }
                Err(e) => {
                    let side = self.session.duel.active;
                    return Some((side, self.side_name(side), Err(e), self.capture_before(side)));
                }
            }
        }
        let link = self.online.as_mut()?;
        match link.next_batch() {
            Batch::Waiting => return None,
            Batch::Idle => {}
            Batch::Press(remote) => {
                // The other seat moved its cursor or a menu: the same input
                // layer replays the key on this copy (for the response and
                // the presentation), the seat's reported state is shown.
                // A spectator shows either seat's.
                let side = self.session.duel.active;
                if side == seat && !link.spectator {
                    return None;
                }
                let who = self.side_name(side);
                let before = self.capture_before(side);
                let mut replayed = self.session.input.clone();
                let response = if remote.at_base {
                    replayed.press(&self.session.duel, &self.data.catalogue, remote.press.key).ok()
                } else {
                    None
                };
                if plausible(&remote.press.input, &self.session.duel) {
                    self.session.input = remote.press.input.clone();
                } else if response.is_some() {
                    self.session.input = replayed;
                }
                let response = remote_response(remote.press.response, response);
                let frame = Frame { key: Some(remote.press.key), response: Some(response), events: Vec::new(), script: None };
                return Some((side, who, Ok(frame), before));
            }
            Batch::Revisions { revisions, mut own, rejected, remote } => {
                if let Some(reason) = &rejected {
                    self.status = Some(format!("the server refused the press: {reason:?}"));
                    if matches!(reason, roses_net::Rejection::Stale { .. }) {
                        link.sync();
                    }
                }
                let hidden = link.hidden;
                // The other seat's press: replay its key on the copy from
                // before its revisions (the response and the input state
                // after it), as the local seat's own press is presented.
                let mut remote_after = None;
                if let Some(r) = &remote {
                    let input_before = self.session.input.clone();
                    let mut replayed = input_before.clone();
                    let response = if r.at_base {
                        replayed.press(&self.session.duel, &self.data.catalogue, r.press.key).ok()
                    } else {
                        None
                    };
                    let response = remote_response(r.press.response, response);
                    self.session.input = replayed;
                    own = Some((input_before, r.press.key, response));
                    remote_after = Some(r.press.input.clone());
                }
                if revisions.is_empty() {
                    // Nothing was accepted: the input state from before the
                    // press (the other seat reports its own).
                    if let Some((input_before, _, _)) = own {
                        self.session.input = input_before;
                    }
                    if let Some(after) = remote_after.filter(|a| plausible(a, &self.session.duel)) {
                        self.session.input = after;
                    }
                    settle_input(&mut self.session.input, &self.session.duel, false);
                    return None;
                }
                // Summons whose card this seat does not see fly face down.
                self.hidden_summons = revisions
                    .iter()
                    .flat_map(|r| &r.events)
                    .filter_map(|e| match e {
                        roses_net::SeenEvent::Summoned { unit, .. } => Some(*unit),
                        _ => None,
                    })
                    .collect();
                let mut patched = self.session.duel.clone();
                let events = batch_events(&mut patched, &revisions, hidden);
                self.session.duel = patched;
                let side = self.session.duel.active;
                let who = self.side_name(side);
                let (key, response) = match &own {
                    Some((_, k, r)) => (Some(*k), Some(r.clone())),
                    None => (None, None),
                };
                let after = own.map(|(input_before, _, _)| std::mem::replace(&mut self.session.input, input_before));
                let before = self.capture_before(side);
                if let Some(after) = after {
                    self.session.input = after;
                }
                let view = &revisions.last().expect("not empty").view;
                match roses_net::mirror_duel(view, &self.data.catalogue) {
                    Ok(d) => self.session.duel = d,
                    Err(e) => return Some((side, who, Err(e), before)),
                }
                if let Some(after) = remote_after.filter(|a| plausible(a, &self.session.duel)) {
                    self.session.input = after;
                }
                let spectator = self.online.as_ref().is_some_and(|l| l.spectator);
                if key.is_none() && (side != seat || spectator) {
                    if let Some(cell) = remote_focus(&events, &self.session.duel) {
                        let i = side_index(&self.session.duel, side);
                        self.session.input.cursors[i] = cell;
                    }
                }
                let frame = Frame { key, response, events, script: None };
                return Some((side, who, Ok(frame), before));
            }
        }
        let link = self.online.as_ref()?;
        if presses == 0 || !link.may_act() {
            return None;
        }
        let side = self.session.duel.active;
        let who = self.side_name(side);
        let before = self.capture_before(side);
        let input_before = self.session.input.clone();
        match self.session.input.press(&self.session.duel, &self.data.catalogue, presses) {
            Err(e) => {
                // The input state does not fit the copy: back to the field.
                self.session.input = input_before;
                settle_input(&mut self.session.input, &self.session.duel, false);
                self.status = Some(format!("{who} {}: {e}", button_names(presses)));
                None
            }
            Ok(response @ Response::Commands(_)) => {
                let after = self.session.input.clone();
                self.online.as_mut()?.submit(input_before, after, presses, response);
                None
            }
            Ok(response) => {
                let input = self.session.input.clone();
                self.online.as_mut()?.stream_press(presses, &response, &input);
                Some((
                    side,
                    who,
                    Ok(Frame { key: Some(presses), response: Some(response), events: Vec::new(), script: None }),
                    before,
                ))
            }
        }
    }

    /// The running lesson's script state.
    pub fn tutorial(&self) -> Option<&roses_core::duel::script::Tutorial> {
        self.session.tutorial.as_ref()
    }

    /// The active side's cursor (the one the board camera follows).
    pub fn cursor(&self) -> GridPos {
        self.session.input.cursor(&self.session.duel)
    }

    pub fn overlay_input(&self) -> OverlayInput {
        if let Some(run) = &self.exodia {
            return self.exodia_overlay(run);
        }
        if let Some(o) = &self.opening {
            // No cell overlay; the emblems at d34 and the leaders at their
            // alpha (hidden until c620 shows them).
            let mut input = overlay_input(&self.session);
            input.state = o.state();
            input.hidden_cells = true;
            for u in &mut input.units {
                if u.slot == 0 {
                    u.emblem = Some(o.emblem);
                    u.alpha = Some(o.leader_alpha);
                }
            }
            return input;
        }
        if let Some(intro) = &self.intro {
            let mut input = intro.frozen.clone();
            let reveal = self.scene_hides_hud();
            for unit in &mut input.units {
                if let Some(t) = intro.turns.iter().find(|t| t.0 == (unit.side, unit.slot)) {
                    unit.turn = t.1;
                }
                if reveal {
                    if let Some(r) = intro
                        .revealed
                        .iter()
                        .find(|r| r.0 == (unit.side, unit.slot))
                    {
                        unit.face_up = true;
                        unit.face = Some(r.1);
                    }
                }
            }
            // c605 hides both battle cards (0x1F6BE0); a survivor's shows
            // again when its copy lands, a destroyed one stays hidden.
            if let (Some(end), Some(cards)) = (
                intro.versus.as_ref().and_then(|v| v.end.as_ref()),
                intro.versus_cards,
            ) {
                input.units.retain(|u| {
                    let i = intro.turns.iter().position(|t| t.0 == (u.side, u.slot));
                    match i {
                        Some(i) if i < 2 && cards[i].0 == u.side && !(intro.leader && i == 1) => {
                            end.board_card_shown(i)
                        }
                        _ => true,
                    }
                });
            }
            return input;
        }
        if let Some(m) = &self.monster_fx {
            use crate::duel_monster_fx::Effect;
            let mut input = m.frozen.clone();
            let live = {
                let mut l = overlay_input(&self.session);
                add_board_faces(&mut l, &self.session, &self.data.catalogue, &self.hud_context());
                l
            };
            let level = m.others_level();
            let targets: Vec<(u8, u8)> = m.targets.iter().map(|t| t.unit).collect();
            for u in &mut input.units {
                let key = (u.side, u.slot);
                if Some(key) == m.revealed {
                    u.flip = m.flip();
                    if m.revealed_face_up() || u.flip.is_some() {
                        u.face_up = true;
                        if let Some(l) = live.units.iter().find(|l| (l.side, l.slot) == key) {
                            u.face = l.face.or(u.face);
                        }
                    }
                }
                if !targets.contains(&key) && Some(key) != m.revealed && level < 128 {
                    u.alpha = Some(level);
                }
                if m.effect == Effect::Countdown && targets.contains(&key) && !u.face_up {
                    // Flipped by the step itself (a manual flip): face up
                    // before the handler runs.
                    u.face_up = true;
                    if let Some(l) = live.units.iter().find(|l| (l.side, l.slot) == key) {
                        u.face = l.face.or(u.face);
                    }
                }
                if let Some(t) = m.targets.iter().find(|t| t.unit == key) {
                    if let Some(level) = m.struck_level(t) {
                        // Raigeki: the struck monster fades (mode 1).
                        u.alpha = Some(level);
                    }
                }
                if let Some((unit, flip, up)) = m.trap_reveal().filter(|r| r.0 == key) {
                    // Fake Trap / Anti-Raigeki revealed at E (8 U).
                    let _ = unit;
                    u.flip = flip;
                    if up || flip.is_some() {
                        u.face_up = true;
                        u.face.get_or_insert(duel_overlay::BoardFace { level: 0, attack: 0, defence: 0, effect: false });
                    }
                }
                if targets.contains(&key) && m.applied() {
                    match m.effect {
                        Effect::Defence => u.defence = true,
                        Effect::Reveal => {
                            u.face_up = true;
                            if let Some(l) = live.units.iter().find(|l| (l.side, l.slot) == key) {
                                u.face = l.face.or(u.face);
                            }
                        }
                        Effect::Conceal => u.face_up = false,
                        Effect::Boost => {
                            if let Some(l) = live.units.iter().find(|l| (l.side, l.slot) == key) {
                                u.face = l.face;
                            }
                        }
                        _ => {}
                    }
                }
            }
            if let Some(v) = m.victim_hidden() {
                input.units.retain(|u| (u.side, u.slot) != v.unit);
            }
            if m.effect == Effect::Destroy && m.targets.len() > 1 {
                // The other targets fade and shrink (0x2519E0 mode 7) from
                // E+60 and all are removed at E+120 [F; H: -8/U].
                let e = m.e().unwrap_or(0);
                for u in input.units.iter_mut().filter(|u| targets[1..].contains(&(u.side, u.slot))) {
                    if e >= 60 {
                        // Mode 7: card scale level / 1280 (x level / 128 of
                        // its 0.1), model min(1, 2 level / 128) [F 1.3].
                        let level = 128u32.saturating_sub(8 * (e - 59)) as u8;
                        u.alpha = Some(level);
                        u.spin = Some((0, f32::from(level) / 128.0));
                        u.model_scale = Some((2.0 * f32::from(level) / 128.0).min(1.0));
                    }
                }
                if e >= 120 {
                    input.units.retain(|u| !targets.contains(&(u.side, u.slot)));
                }
            }
            input.units.retain(|u| !m.new_hidden((u.side, u.slot)));
            if m.effect == Effect::DarkHole {
                // c021 fades every other card and shrinks every model; the
                // targets' cards are its own card objects (`ritual_cards`),
                // removed when its end is seen.
                if let Some(c) = &m.c021 {
                    for u in &mut input.units {
                        let h = (u16::from(u.side) << 8) | u16::from(u.slot);
                        if let Some(k) = c.card(h) {
                            u.alpha = Some(k.colour[3]);
                        }
                        if u.slot == 0 || (u.face_up && u.slot <= 5) {
                            if let Some(s) = c.model_scale(h) {
                                u.model_scale = Some(s);
                            }
                        }
                    }
                    input.units.retain(|u| !targets.contains(&(u.side, u.slot)));
                }
            }
            if m.effect == Effect::Shift {
                if let (Some((alpha, cell)), Some(t)) = (m.shift_card(), m.targets.first()) {
                    for u in input.units.iter_mut().filter(|u| (u.side, u.slot) == t.unit) {
                        u.cell = [cell.0, cell.1];
                        u.alpha = Some(alpha);
                    }
                }
            }
            if m.effect == Effect::Crush {
                if m.crushed_removed() {
                    input.units.retain(|u| !targets.contains(&(u.side, u.slot)));
                } else if let Some(u) = m.module_u() {
                    let level = crate::duel_c612::victim_level(u);
                    for v in input.units.iter_mut().filter(|v| targets.contains(&(v.side, v.slot))) {
                        v.alpha = Some(level);
                    }
                }
            }
            if m.effect == Effect::Revive && m.applied() {
                // E+34: the revived unit, face down, as the rules made it.
                if let Some(new) = m.targets.first().and_then(|t| live.units.iter().find(|l| (l.side, l.slot) == t.unit)) {
                    input.units.retain(|u| (u.side, u.slot) != (new.side, new.slot));
                    input.units.push(new.clone());
                }
            }
            if m.effect == Effect::Ritual {
                let e = m.e().unwrap_or(0);
                if e > m.ritual_start + 216 {
                    // The ritual's end: the board as the rules left it.
                    let mut l = live.clone();
                    for u in &mut l.units {
                        u.alpha = None;
                    }
                    return l;
                }
                // The sacrifices are revealed at E [H: their 8-update flip
                // is not drawn]; c059 moves their card objects (drawn as
                // `ritual_cards`) and fades every other card and model.
                for u in input.units.iter_mut().filter(|u| targets.contains(&(u.side, u.slot))) {
                    u.face_up = true;
                }
                if let Some(c) = &m.c059 {
                    input.units.retain(|u| !targets.contains(&(u.side, u.slot)));
                    for u in &mut input.units {
                        let handle = (u16::from(u.side) << 8) | u16::from(u.slot);
                        if let Some(o) = c.others().iter().find(|o| o.handle == handle) {
                            u.alpha = Some(o.alpha);
                            u.model_scale = o.model_scale;
                        }
                    }
                }
            }
            if m.effect == Effect::Field && m.e().is_some_and(|e| e >= 77) {
                // c603 sets each trap's card alpha to 0 (then removes it).
                input.units.retain(|u| !targets.contains(&(u.side, u.slot)));
            }
            if m.effect == Effect::Evolve {
                if m.swapped() {
                    // E+105: the old moth removed, the new card created on
                    // its cell, face down, attack; not in the fade lists.
                    input.units.retain(|u| !targets.contains(&(u.side, u.slot)));
                    if let Some(new) = m.spawned.and_then(|k| live.units.iter().find(|l| (l.side, l.slot) == k)) {
                        input.units.retain(|u| (u.side, u.slot) != (new.side, new.slot));
                        input.units.push(new.clone());
                    }
                } else if let Some(c) = &m.c560 {
                    for u in input.units.iter_mut().filter(|u| targets.contains(&(u.side, u.slot))) {
                        u.spin = Some((c.card_yaw, c.card_size()));
                        u.turn += c.model_yaw;
                        u.model_scale = Some(c.model_scale);
                    }
                }
            }
            fill_faces_from(&mut input, &self.session.duel, &self.data.catalogue, &self.hud_context());
            if let Some(duel) = m.duel.as_ref() {
                fill_faces_from(&mut input, duel, &self.data.catalogue, &self.hud_context());
            }
            return input;
        }
        if let Some(a) = &self.activation {
            let mut input = a.frozen.clone();
            let hidden = a.board_card_hidden();
            input.units.retain(|u| !(hidden && (u.side, u.slot) == a.unit));
            if let Some(level) = a.unit_level() {
                // Every shown unit (0x261550: the 22 units), leaders'
                // models included; the emblems keep their quads.
                for u in &mut input.units {
                    u.alpha = Some(level);
                }
            }
            if let Some((side, cell)) = a.victim_hidden() {
                input.units.retain(|u| !(u.side == side && (u.cell[0], u.cell[1]) == cell && u.slot != 0));
            }
            for l in a.unit_looks() {
                if l.hidden {
                    input.units.retain(|u| (u.side, u.slot) != l.unit);
                    continue;
                }
                for u in input.units.iter_mut().filter(|u| (u.side, u.slot) == l.unit) {
                    if let Some(level) = l.level {
                        u.alpha = Some(level);
                        if l.shrink {
                            // Mode 7: card scale level / 1280, model min(1,
                            // 2 level / 128) [F 1.3].
                            u.spin = Some((0, f32::from(level) / 128.0));
                            u.model_scale = Some((2.0 * f32::from(level) / 128.0).min(1.0));
                        }
                    }
                    if let Some(d) = l.defence {
                        u.defence = d;
                        u.sideways = l.sideways;
                    }
                }
            }
            let flip = a.flip();
            if a.revealed() || flip.is_some() {
                for u in &mut input.units {
                    if (u.side, u.slot) == a.unit {
                        u.flip = flip;
                        u.alpha = a.card_alpha().or(a.plan_card_level());
                        u.face_up = true;
                        u.face.get_or_insert(duel_overlay::BoardFace { level: 0, attack: 0, defence: 0, effect: false });
                    }
                }
            }
            return input;
        }
        let mut input = overlay_input(&self.session);
        add_board_faces(&mut input, &self.session, &self.data.catalogue, &self.hud_context());
        for (unit, u) in input.units.iter_mut().zip(&self.session.duel.units) {
            if let Some(&p55) = self.sideways.get(&u.id) {
                if p55 != 0 && p55 != 0xFF {
                    unit.sideways = Some(p55);
                }
            }
        }
        // The summoned card is the flying card until it lands.
        if let Some(f) = self.summon_flight.as_ref().filter(|f| f.flying()) {
            input.units.retain(|u| (u.side, u.slot) != f.unit);
        }
        if let Some(walk) = &self.walk {
            let offset = walk.offset().map(|v| v as f32);
            if let Some((_, frozen, _, duel)) = self.deferred_activation.as_deref() {
                // The trap and its victim as they stood before the step.
                input = frozen.clone();
                for (unit, u) in input.units.iter_mut().zip(&duel.units) {
                    if u.id == walk.unit {
                        unit.offset = offset;
                    }
                }
                return input;
            }
            for (unit, u) in input.units.iter_mut().zip(&self.session.duel.units) {
                if u.id == walk.unit {
                    unit.offset = offset;
                }
            }
        }
        input
    }

    /// One update with the human pad's held buttons (new presses, plus the
    /// original auto-repeat of the d-pad; see [`PadRepeat`]).
    pub fn update(&mut self, held: u16) -> Option<Frame> {
        self.update_pads([held, 0])
    }

    /// Whether both sides are pad-driven (Custom Duel's two players).
    pub fn two_humans(&self) -> bool {
        self.session.tutorial.is_none()
            && self.session.seats.iter().all(Option::is_none)
            && self.session.duel.controllers.iter().all(|&c| c >= roses_core::duel::PAD_CONTROLLER)
    }

    /// The board camera at the duel's start, before the start's turn
    /// camera (0x1DAAA0): the duel init 0x1AE640(p1, p2 = the player's
    /// side) sets the yaw 0x4000, or 0xC000 for a player on side 1, before
    /// any track (0x1AE89C..0x1AE910) [F], so a White Rose duel opens from
    /// the player's end of the board instead of turning there from side 0's
    /// over the first 32 updates. (The init's opening zoom from distance 50
    /// straight down, 0x1D02E4.., is not modelled: the camera starts at the
    /// player side's setting.)
    /// Play the duel's opening before the first turn (the game's campaign
    /// duels): when the side that moves first is pad-driven (+0x33C ≥ 100,
    /// 0x1D0298); a computer, a lesson script or an online seat skips it.
    pub fn begin_opening(&mut self) {
        let active = self.session.duel.active & 1;
        let human = self.session.seats.get(usize::from(active)).is_some_and(Option::is_none);
        if self.session.tutorial.is_some() || self.online.is_some() || self.multi.is_some() || !human {
            return;
        }
        let Some(leader) = self.session.duel.units.iter().find(|u| u.side == active && u.slot == 0) else {
            return;
        };
        let cell = (leader.position.column, leader.position.row);
        self.opening = Some(crate::duel_opening::DuelOpening::new(cell, self.data.first_duel.player_side));
        self.opening_camera = Some(self.start_camera());
    }

    pub fn start_camera(&self) -> BoardCamera {
        if let Some(o) = &self.opening {
            let (target, polar) = o.start_camera();
            let mut camera = BoardCamera::snapped((self.cursor().column, self.cursor().row));
            camera.target = target;
            camera.polar = polar;
            return camera;
        }
        let c = self.cursor();
        let mut camera = BoardCamera::snapped((c.column, c.row));
        camera.polar = SideCamera::for_side(self.data.first_duel.player_side).polar();
        if let Some(m) = &self.multi {
            if let Some(cam) = m.cameras.get(usize::from(self.session.duel.active)) {
                camera.polar = cam.polar();
            }
        }
        camera
    }

    /// The active side's board camera setting as a polar (the cursor
    /// camera's rest).
    pub fn board_polar(&self) -> [f64; 3] {
        // N-player (a port addition): every seat's own end.
        if let Some(c) = self.multi.as_ref().and_then(|m| m.cameras.get(usize::from(self.session.duel.active))) {
            return c.polar();
        }
        self.side_cameras[usize::from(self.session.duel.active & 1)].polar()
    }

    /// The active side's camera setting the sticks change.
    pub fn side_camera_mut(&mut self, side: u8) -> &mut SideCamera {
        if let Some(m) = self.multi.as_mut() {
            if usize::from(side) < m.cameras.len() {
                return &mut m.cameras[usize::from(side)];
            }
        }
        &mut self.side_cameras[usize::from(side & 1)]
    }

    /// The sticks the duel's camera reads this update (0x1D3C7C) [F]: in
    /// the board cursor state (0x18), nothing animating; a pad side reads
    /// its own port when that pad is analog (0x203500(control - 100)); a
    /// side with control 0x20..0x63 reads port 0 without the analog test
    /// (here: when port 0 has sticks); any other side none.
    pub fn stick_input(&self) -> Option<[u8; 4]> {
        // (0x22's slide-out reads no sticks either.)
        if self.halted.is_some() || self.animating() || self.session.input.state() != 0x18 || self.details_slide > 0 {
            return None;
        }
        let c = *self.session.duel.controllers.get(usize::from(self.session.duel.active & 1))?;
        if c >= roses_core::duel::PAD_CONTROLLER {
            self.sticks[usize::from(c - roses_core::duel::PAD_CONTROLLER).min(1)]
        } else if (0x20..0x64).contains(&c) {
            self.sticks[0]
        } else {
            None
        }
    }

    /// The pad port of a side: control − 100 (0x1CFF80) [F]; a computer or
    /// script side has none.
    pub fn pad_port(&self, side: u8) -> Option<usize> {
        let c = *self.session.duel.controllers.get(usize::from(side & 1))?;
        (c >= roses_core::duel::PAD_CONTROLLER).then(|| usize::from(c - roses_core::duel::PAD_CONTROLLER).min(1))
    }

    /// One update with both pads' held buttons: the active side reads its
    /// own port (pad 1 = control 100, pad 2 = control 101); with one human
    /// the duel reads pad 1 only, as before.
    pub fn update_pads(&mut self, held: [u16; 2]) -> Option<Frame> {
        if self.halted.is_some() {
            return None;
        }
        // 0x1D081C..0x1D0980: the scroll words step with this update's
        // starting d63, then d63 counts down.
        self.glow_hold = self.d63 != 0;
        self.d63 = self.d63.saturating_sub(1);
        if let Some(o) = self.opening.as_mut() {
            // The opening's states read no pad; the session waits.
            self.held_pads = held;
            self.repeat.frame(held[0]);
            self.repeat2.frame(held[1]);
            let camera = o.step();
            let launch = o.launch_c620;
            let done = o.done();
            self.camera_command = camera.map(|c| match c {
                crate::duel_opening::OpeningCamera::Set { target, polar } => CameraMove { target, polar, frames: 1 },
                crate::duel_opening::OpeningCamera::Track { target, polar, frames } => CameraMove { target, polar, frames },
            });
            // The tracks step after the mode update; c620 runs in the
            // render pass with that camera (0x1F52E0).
            if let Some(cam) = self.opening_camera.as_mut() {
                if let Some(m) = &self.camera_command {
                    cam.track_to(m.target, m.polar, m.frames);
                }
                cam.tick();
            }
            let view = self.opening_camera.as_ref().map(|c| crate::duel_opening_fx::board_camera(c.eye(), c.target));
            if launch {
                let program = self.arena_provider.as_ref().and_then(|p| p.cmodule.clone());
                for side in 0..2u8 {
                    let leader = self.session.duel.units.iter().find(|u| u.side == side && u.slot == 0);
                    self.opening_fx[usize::from(side)] = (match (&program, leader, view) {
                        (Some(cmodule), Some(l), Some(view)) => crate::duel_opening_fx::LeaderEffect::launch(
                            &self.data.executable,
                            cmodule,
                            self.data.texeff.as_deref(),
                            (l.position.column, l.position.row),
                            view,
                        ),
                        _ => None,
                    }).map(Box::new);
                }
            } else if let Some(view) = view {
                for fx in self.opening_fx.iter_mut().flatten() {
                    fx.update(view);
                }
            }
            if done {
                self.opening = None;
                self.opening_camera = None;
                self.opening_fx = [None, None];
            }
            return None;
        }
        self.held_pads = held;
        let p1 = self.repeat.frame(held[0]);
        let p2 = self.repeat2.frame(held[1]);
        self.last_presses2 = p2;
        let port1 = self.two_humans() && self.pad_port(self.session.duel.active) == Some(1);
        let mut presses = if port1 { p2 } else { p1 };
        // The board cursor state 0x18 [F 0x1D3C64, emu]: while a camera
        // track runs (0x1FB900(-1, -1) != 0: the 8-update track of the
        // last cursor step or a stick turn) the state reads nothing; else
        // the d-pad moves the cursor while held (the held word, not the
        // new presses or the auto-repeat), so a held direction steps one
        // cell every 8 updates from the press.
        let board = self.board_reads_pad();
        if board {
            presses = if self.board_track > 0 {
                0
            } else {
                (presses & !REPEAT_MASK) | (held[usize::from(port1)] & REPEAT_MASK)
            };
        }
        if self.auto_pages {
            presses |= self.operator.pad(&self.session);
        }
        self.last_presses = presses;
        let before = self.cursor();
        let frame = self.press(presses);
        if board && presses & REPEAT_MASK != 0 {
            // 0x1D3F8C / 0x1D3F90: the handler took a direction (moved or
            // stopped at the edge) [interpreter: both set 60].
            self.d63 = 0x3C;
        }
        if board && presses & pad::CROSS != 0 && self.session.input.state() != 0x18 {
            // 0x1D3A60 / 0x1D3B74: Cross picked a card or a unit.
            self.d63 = 0;
        }
        if board && self.cursor() != before {
            // 0x1DA360(8, ...): the cursor's target and polar tracks.
            self.board_track = crate::duel_board::CURSOR_TRACK_FRAMES;
        }
        // 0x1FB430 after the mode update: the tracks advance.
        self.board_track = self.board_track.saturating_sub(1);
        frame
    }

    /// Whether this update's pad goes to the board cursor state 0x18 of a
    /// pad-driven side (nothing animating, the duel running).
    fn board_reads_pad(&self) -> bool {
        let side = self.session.duel.active;
        self.halted.is_none()
            && !self.animating()
            && self.session.input.state() == 0x18
            && self.pad_port(side).is_some()
            && self.session.seats.get(usize::from(side & 1)).is_some_and(Option::is_none)
    }

    /// What the open details view shows (overlay slot 1, code 0x11).
    pub fn details_of(&self, view: roses_core::duel::ui::DetailsView) -> DetailsShown {
        let input = &self.session.input;
        let duel = &self.session.duel;
        let d35 = if input.hand_view.is_some() && matches!(input.mode, Mode::Field) { 0x53 } else { input.mode.state() };
        let d5c = match (view.unit.and_then(|id| duel.unit(id)), &input.mode) {
            (Some(u), _) => u.side,
            (None, Mode::Graveyard(pick)) => pick.shown & 1,
            _ => duel.active,
        };
        DetailsShown { view, d35, d5c }
    }

    /// The hand row's motion for a change of input state: open (0x3E ->
    /// 0x43), close (0x43 -> 0x3E), pick (0x43 -> 0x46) and its cancel; a
    /// summon from 0x46 records what the row holds.
    fn start_hand(&mut self, before: &Mode, side: u8) {
        use crate::duel_hand::{HandAnim, HandKind};
        let after = &self.session.input.mode;
        let count = self.session.duel.sides[side_index(&self.session.duel, side)].hand.len();
        let seen = &mut self.hand_seen[usize::from(side & 3)];
        let picks = |m: &roses_core::duel::ui::HandMenu| {
            if m.marks.is_empty() { vec![m.cursor] } else { m.marks.clone() }
        };
        let anim = |kind, old, picks| HandAnim { kind, step: 0, count, old, picks };
        self.hand_anim = match (before, after) {
            (Mode::SummonCell { .. }, Mode::Hand(_)) => Some(anim(HandKind::Open, (*seen).min(count), Vec::new())),
            (Mode::Hand(_), Mode::SummonCell { .. }) => Some(anim(HandKind::Close, count, Vec::new())),
            (Mode::Hand(_), Mode::Confirm(m)) => Some(anim(HandKind::Pick, count, picks(m))),
            (Mode::Confirm(m), Mode::Hand(_)) => Some(anim(HandKind::Unpick, count, picks(m))),
            (Mode::Confirm(_), _) => {
                *seen = count;
                None
            }
            _ => None,
        };
    }

    /// A summon confirmed in 0x46 flies the card to its cell
    /// (`duel_summon`); a fusion's modules (C600 / C538) are not played
    /// (reported), its result flies the same way.
    fn start_summon(
        &mut self,
        before: &Mode,
        events: &[roses_core::duel::DuelEvent],
        hand_before: &[u16],
        field_chain: Option<u16>,
    ) {
        let Mode::Confirm(menu) = before else {
            return;
        };
        use roses_core::duel::CardData;
        // Online, the other seat's hidden hand: its chain is not shown and
        // its card flies face down (only placeholders are known here).
        let hidden_chain = self.unknown_hand();
        let face = |card: u16| {
            let info = self.data.catalogue.card(card);
            (
                card,
                info.map_or(0, |i| i.level),
                info.map_or(0, |i| i32::from(i.attack)),
                info.map_or(0, |i| i32::from(i.defense)),
            )
        };
        // The chain's pairs from its events: Fused -> C600, the older card
        // dropped -> C538, a power-up used -> its equip module.
        let mut picks: Vec<u16> = menu.marks.iter().filter_map(|&i| hand_before.get(i).copied()).collect();
        if picks.is_empty() {
            picks.extend(hand_before.get(menu.cursor).copied());
        }
        // A summon onto an own unit: the field unit is the chain's slot 0 at
        // view (-101, 8), the picks follow [F SUMMON-PRESENTATION.md 4.1].
        if let Some(card) = field_chain {
            picks.insert(0, card);
        }
        let mut fusion = None;
        if picks.len() > 1 && !hidden_chain {
            let mut element = picks[0];
            let mut next = picks.iter().skip(1).copied();
            let mut pairs = Vec::new();
            let mut equipped = None;
            for e in events {
                match e {
                    roses_core::duel::DuelEvent::Equipped { monster, power_up, value, .. } => {
                        equipped = Some((*monster, *power_up, *value));
                    }
                    roses_core::duel::DuelEvent::Fused { cards, result } => {
                        let b = next.next().unwrap_or(cards[1]);
                        pairs.push((face(b), crate::duel_summon::Pair::Fuse { result: face(*result) }));
                        element = *result;
                    }
                    roses_core::duel::DuelEvent::Dropped { card, .. } => {
                        let b = next.next().unwrap_or(*card);
                        if let Some((monster, _, value)) = equipped.take().filter(|e| e.1 == *card) {
                            // The stat power-ups' module 0x218 [F
                            // EQUIP-MODULES.md 0]; Riryoku (category 0,
                            // value 0) 0x219 with the drain as its number:
                            // param = amount << 16 (sign 0, mode 0) [F 8.4].
                            let riryoku = self.is_riryoku(*card);
                            let module = if riryoku { 0x219 } else { 0x218 };
                            if riryoku {
                                self.riryoku.push(u32::from(value.max(0) as u16) << 16);
                            }
                            pairs.push((face(b), crate::duel_summon::Pair::Equip { module, monster: face(monster) }));
                            element = monster;
                        } else if *card == element {
                            pairs.push((face(b), crate::duel_summon::Pair::Fail));
                            element = b;
                        }
                    }
                    _ => {}
                }
            }
            if !pairs.is_empty() {
                fusion = Some(crate::duel_summon::FusionShow::new(face(picks[0]), pairs));
            }
        }
        for e in events {
            let roses_core::duel::DuelEvent::Summoned { unit, card, at, .. } = e else {
                continue;
            };
            let Some(u) = self.session.duel.units.iter().find(|u| u.id == *unit) else {
                continue;
            };
            let face = if self.hidden_summons.contains(unit) {
                (roses_client::card_face::BACK_CARD, 0, 0, 0)
            } else {
                face(*card)
            };
            // The camera has held the summon cell since 0x3E (polar (200,
            // pi/2, pi/4) about it) [emu].
            let camera = crate::duel_board::BoardCamera::snapped((at.column, at.row));
            let Some(from) = camera
                .view()
                .and_then(|v| crate::duel_summon::held_pose(&v, camera.polar))
            else {
                continue;
            };
            let mut flight = crate::duel_summon::SummonFlight::new(
                face,
                from,
                u.side,
                (at.column, at.row),
                (u.side, u.slot),
                (u.committed_face_up, u.committed_defence),
            );
            flight.fusion = fusion.take();
            self.summon_flight = Some(Box::new(flight));
            break;
        }
    }

    /// The hand row as the screen shows it now: (d45, d47, poses top
    /// first), while it animates or holds the picked cards (0x46).
    pub fn hand_view(&self) -> Option<(u8, u8, Vec<crate::duel_hand::HandPose>)> {
        if let Some(a) = &self.hand_anim {
            let (d45, d47) = a.bytes();
            return Some((d45, d47, a.poses()));
        }
        if let Mode::Confirm(m) = &self.session.input.mode {
            let count = self.session.duel.sides[side_index(&self.session.duel, self.session.duel.active)].hand.len();
            let picks = if m.marks.is_empty() { vec![m.cursor] } else { m.marks.clone() };
            return Some((0, 30, crate::duel_hand::pick_poses(count, &picks, 30, true)));
        }
        None
    }

    /// Online: the acting side is the other seat and its hand is hidden
    /// (Card Display HIDE), or this screen is a spectator's (no hand is
    /// known): the copy holds only placeholders for it.
    pub fn unknown_hand(&self) -> bool {
        self.online.as_ref().is_some_and(|l| {
            let side = self.session.duel.active;
            // N-player (a port addition): a 2v2 team-mate's hand is sent.
            let mate = self.session.duel.team_mates(l.seat, side);
            l.spectator || (side != l.seat && !mate && !l.options().card_display_open[usize::from(side & 1)])
        })
    }

    /// Hand card `i` of the active side costs more stars than the side has
    /// (0x1D9B00: drawn with vertex RGB 64) [F/emu].
    pub fn hand_unaffordable(&self, i: usize) -> bool {
        let duel = &self.session.duel;
        let side = duel.active;
        // 0x1D9B00(side, 1): with param 0x1D's bit of the side set the
        // shading shows unless a computer side faces a pad side; without it
        // ("HIDE") only while R2 is held on pad port `side` [F].
        let ctx = self.hud_context();
        let ctl = |s: u8| ctx.controllers[usize::from(s & 1)];
        // N-player (a port addition): the bit of the real side.
        let bit = if duel.multi().is_some() { side } else { side & 1 };
        let shade = if ctx.option_29 & (1 << bit) != 0 {
            !(ctl(side) < 32 && ctl(side ^ 1) >= 100)
        } else {
            ctx.r2_held[usize::from(side & 1)]
        };
        if !shade {
            return false;
        }
        let s = side_index(duel, side);
        let Some(&card) = duel.sides[s].hand.get(i) else {
            return false;
        };
        use roses_core::duel::CardData;
        self.data
            .catalogue
            .card(card)
            .is_some_and(|info| duel.sides[s].stars < duel.summon_cost(side, info, &self.data.catalogue))
    }

    /// An `Activated` trap or magic card starts its presentation
    /// (`duel_activate`) from the board before the step; monster effects,
    /// a trap sprung by a walk waits for the walk; activations within a
    /// battle are reported, not presented.
    fn start_activation(
        &mut self,
        events: &[roses_core::duel::DuelEvent],
        before: &OverlayInput,
        positions: &[(roses_core::UnitId, (u8, u8), GridPos)],
        duel_before: roses_core::duel::Duel,
    ) {
        use crate::duel_activate::{Activation, Kind, Victim};
        use roses_core::duel::{CardData, DuelEvent, class};
        let Some((unit, card)) = events.iter().find_map(|e| match e {
            DuelEvent::Activated { unit, card, .. } => Some((*unit, *card)),
            _ => None,
        }) else {
            return;
        };
        let mut report = |what: &'static str| {
            if self.reported_activation.insert(what) {
                eprintln!("Duel play: unsupported, not presented: {what}");
            }
        };
        let kind = match self.data.catalogue.card(card).map(|c| c.class()) {
            Some(class::NEAR_TRAP | class::FAR_TRAP) => Kind::Trap,
            Some(class::MAGIC | class::RITUAL) => Kind::Magic,
            _ => {
                self.start_monster_fx(events, before, positions, duel_before, unit);
                return;
            }
        };
        // A trap with its own steps after c602 (`duel_activate::TrapPlan`)
        // [F NEWLY-ALLOWED.md 15.2].
        let plan = {
            use crate::duel_activate::TrapPlan;
            use roses_core::duel::trap_type;
            let info = self.data.catalogue.card(card);
            match (kind, info.and_then(|c| c.card_effect_type())) {
                (Kind::Trap, Some(trap_type::MIRROR_FORCE)) => Some(TrapPlan::MirrorForce),
                (Kind::Trap, Some(trap_type::MIRROR_WALL)) => Some(TrapPlan::MirrorWall),
                (Kind::Trap, Some(trap_type::SHADOW_OF_EYES)) => Some(TrapPlan::ShadowOfEyes),
                // Widespread Ruin: limited-range trap case 11 (0x24DA54).
                (Kind::Trap, _) if info.is_some_and(|c| c.class() == class::NEAR_TRAP && c.kind_index == 11) => Some(TrapPlan::WidespreadRuin),
                _ => None,
            }
        };
        // Mirror Wall and an immune attacker's Mirror Force run before the
        // battle, which goes on afterwards (`deferred_battle`, MEP 4.2).
        let battle_after = matches!(plan, Some(crate::duel_activate::TrapPlan::MirrorWall | crate::duel_activate::TrapPlan::MirrorForce));
        if events.iter().any(|e| matches!(e, DuelEvent::Battle { .. })) && !battle_after {
            report("an activation within a battle (the presentation order is not traced)");
            return;
        }
        if let Some(walk) = &self.walk {
            // The walker entered the trap's cell and stays there (state
            // 0x36: "the attacker stays where it walked to"); the trap's
            // handler runs after the walk [F ACTIVATION-PRESENTATION.md
            // 3.2-3.4; H: no walk dispatch overlaps the handler].
            if self.deferred_activation.is_none() {
                let (mover, at) = (walk.unit, *walk.cells.last().expect("walk cells"));
                let mut before = before.clone();
                let mut duel = duel_before;
                if let Some(i) = duel.units.iter().position(|u| u.id == mover) {
                    duel.units[i].position = at;
                    if let Some(u) = before.units.get_mut(i) {
                        u.cell = [at.column, at.row];
                    }
                }
                let positions = positions.iter().map(|&(id, key, p)| (id, key, if id == mover { at } else { p })).collect();
                self.deferred_activation = Some(Box::new((events.to_vec(), before, positions, duel)));
            }
            return;
        }
        let Some(&(_, key, at)) = positions.iter().find(|p| p.0 == unit) else {
            return;
        };
        // The victim's module after the trap's c601, kept as a wait of its
        // length: c544 46 "Spellbound!" (with c557 50 "Power decrease!"),
        // c603 60 for a destroyed victim [F/emu 2.7, 3.2-3.3].
        let spellbound = events.iter().find_map(|e| match e {
            DuelEvent::Spellbound { unit: u } => Some(*u),
            _ => None,
        });
        let lowered = events.iter().any(|e| matches!(e, DuelEvent::Modified { .. }));
        let destroyed = events.iter().find_map(|e| match e {
            DuelEvent::Destroyed { unit: u, .. } if *u != unit => Some(*u),
            _ => None,
        });
        let victim_at = |id: roses_core::UnitId| {
            positions.iter().find(|p| p.0 == id).map(|p| (p.1.0, (p.2.column, p.2.row)))
        };
        let unit_of = |id: roses_core::UnitId| before.units.iter().zip(&duel_before.units).find(|(_, u)| u.id == id).map(|(o, _)| o.clone());
        let card_of = |id: roses_core::UnitId| before.units.iter().zip(&duel_before.units).find(|(_, u)| u.id == id).map_or(0, |(o, _)| o.card);
        // Negate Attack (event 0x36): c602 with the card fading, then the
        // box shows its name, the camera cuts to the attacker and c034 plays
        // (43 U) [F ACTIVATION-PRESENTATION.md 3.4].
        let negate = kind == Kind::Trap
            && self.data.catalogue.card(card).and_then(|c| c.card_effect_type()) == Some(roses_core::duel::trap_type::NEGATE_ATTACK);
        // Event 0x36 shows c034 on the defender (record +4), not the
        // attacker [F NEWLY-ALLOWED.md 1.4].
        let defender = events.iter().find_map(|e| match e {
            DuelEvent::Attacked { defender, .. } => Some(*defender),
            _ => None,
        });
        let target = defender.or_else(|| events.iter().find_map(|e| match e {
            DuelEvent::Activated { target, .. } => *target,
            _ => None,
        }));
        let mut magic_bind: Vec<roses_core::UnitId> = Vec::new();
        let mut magic_burn: Option<(u8, u16)> = None;
        let mut magic_field: Option<Vec<roses_core::UnitId>> = None;
        let mut magic_ritual = false;
        let mut magic_boost = false;
        let mut magic_destroy: Vec<roses_core::UnitId> = Vec::new();
        let mut magic_shift: Option<(roses_core::UnitId, GridPos, GridPos)> = None;
        let mut magic_cross = false;
        let mut magic_lawbook = false;
        let mut magic_dark_hole = false;
        let victim = match (kind, spellbound, lowered, destroyed) {
            _ if plan.is_some() => None,
            _ if negate => target.and_then(|v| victim_at(v)).map(|(side, cell)| Victim {
                text: 323 + card,
                updates: 43,
                side,
                cell,
                card: card_of(target.expect("checked")),
                spellbound: false,
                destroyed: false,
                camera_frames: 0,
                lowered: false,
                camera: true,
                recovery: None,
                face_up: unit_of(target.expect("checked")).is_some_and(|u| u.face_up),
                defence: false,
                face: None,
            }),
            (Kind::Trap, Some(v), lowered, _) => victim_at(v).map(|(side, cell)| Victim {
                text: if lowered { 0x108 } else { 0x109 },
                updates: if lowered { 50 } else { 46 },
                side,
                cell,
                card: card_of(v),
                spellbound: true,
                destroyed: false,
                camera_frames: 16,
                lowered,
                camera: true,
                recovery: None,
                face_up: unit_of(v).is_some_and(|u| u.face_up),
                defence: unit_of(v).is_some_and(|u| u.defence),
                face: unit_of(v).and_then(|u| u.face),
            }),
            (Kind::Trap, None, _, Some(v)) => victim_at(v).map(|(side, cell)| Victim {
                text: 0x104,
                updates: 60,
                side,
                cell,
                card: card_of(v),
                spellbound: false,
                destroyed: true,
                camera_frames: 16,
                lowered: false,
                camera: true,
                recovery: None,
                face_up: unit_of(v).is_some_and(|u| u.face_up),
                defence: unit_of(v).is_some_and(|u| u.defence),
                face: unit_of(v).and_then(|u| u.face),
            }),
            // Field magic (event 2): after c601, c502 on every changed cell
            // (17 U unit fade, 116 U scripts) [F EFFECT-MODULES-2.md 1.5].
            (Kind::Magic, ..) if self.session.duel.terrain != duel_before.terrain => Some(Victim {
                text: 0,
                updates: crate::duel_activate::FIELD_FADE - 1 + crate::duel_c502::UPDATES + 1,
                side: key.0,
                cell: (at.column, at.row),
                card,
                spellbound: false,
                destroyed: false,
                camera_frames: 0,
                lowered: false,
                camera: false,
                recovery: None,
                face_up: true,
                defence: false,
                face: None,
            }),
            // Remedy and other LP gains (event 8): after c601, the box
            // "LP recovery!" with c508 (85 U); the LP shows after it [F 2.4].
            (Kind::Magic, ..) => {
                let side = usize::from(key.0 & 1);
                let gain = i32::from(self.session.duel.sides[side].lp) - i32::from(duel_before.sides[side].lp);
                (gain > 0).then_some(Victim {
                    text: 0x10B,
                    updates: crate::duel_c508::UPDATES,
                    side: key.0,
                    cell: (at.column, at.row),
                    card,
                    spellbound: false,
                    destroyed: false,
                    camera_frames: 0,
                    lowered: false,
                    camera: false,
                    recovery: Some(gain),
                    face_up: true,
                    defence: false,
                    face: None,
                })
            }
            _ => None,
        };
        let event_36 = negate || matches!(plan, Some(crate::duel_activate::TrapPlan::MirrorForce | crate::duel_activate::TrapPlan::MirrorWall));
        if event_36 {
            // Event 0x36's init: 0x262630(0, trap, 180, 180) [F 1.1, 1.4.2].
            self.unit_camera = None;
            self.camera_command = Some(trap_cut((at.column, at.row), self.session.duel.active));
        } else if kind == Kind::Trap {
            // 0x262410(0, trap, 100, 180): an instant cut to the cell; it
            // replaces the step's own camera move (the cancelled mover's
            // cursor return) [F].
            self.camera_command = None;
            self.unit_camera = Some(UnitCamera {
                side: key.0,
                cell: (at.column, at.row),
                card,
                turn: 0,
                add_distance: 100,
                cell_distance: 180,
                frames: 0,
                active_side: self.session.duel.active,
            });
        } else if victim.is_none() {
            // Dragon Capture Jar (event 0, 0x2579E0 = Nightmare Scorpion's
            // handler) after c601: the Bind sequence on every bound unit
            // [F PEGASUS-MAGIC.md 2.1].
            let bound: Vec<roses_core::UnitId> = events
                .iter()
                .filter_map(|e| match e {
                    DuelEvent::Spellbound { unit: u } => Some(*u),
                    _ => None,
                })
                .collect();
            let damage = events.iter().find_map(|e| match e {
                DuelEvent::Damage { side, amount } => Some((*side, *amount)),
                _ => None,
            });
            let effect_type = self.data.catalogue.card(card).and_then(|c| c.card_effect_type());
            let ritual = self.data.catalogue.card(card).map(|c| c.class()) == Some(class::RITUAL);
            if ritual && events.iter().any(|e| matches!(e, DuelEvent::Created { .. })) {
                magic_ritual = true;
            } else if roses_core::duel::ELIMINATIONS.contains(&card) {
                // Warrior Elimination and its siblings: event 0x15, Man-Eater
                // Bug's handler on every destroyed unit [F NEWLY-ALLOWED.md 5].
                let mut ids: Vec<roses_core::UnitId> = events
                    .iter()
                    .filter_map(|e| match e {
                        DuelEvent::Destroyed { unit: u, .. } if *u != unit => Some(*u),
                        _ => None,
                    })
                    .collect();
                // The c603 victim: face-up committed monsters first [H: the
                // rand31 pick is not the original's].
                ids.sort_by_key(|id| !duel_before.units.iter().any(|u| u.id == *id && u.committed_face_up));
                magic_destroy = ids;
            } else if effect_type == Some(0x20) {
                // Magical Neutralizing Force Field (event 0x1C): the traps
                // it removes are the step's other destroyed units.
                magic_field = Some(
                    events
                        .iter()
                        .filter_map(|e| match e {
                            DuelEvent::Destroyed { unit: u, .. } if *u != unit => Some(*u),
                            _ => None,
                        })
                        .collect(),
                );
            } else if effect_type == Some(0x25) {
                // Shift (event 0x1F): the unit the step moved onto the card's
                // cell.
                magic_shift = events.iter().rev().find_map(|e| match e {
                    DuelEvent::Moved { unit: u, from, to } if *u != unit => Some((*u, *from, *to)),
                    _ => None,
                });
            } else if effect_type == Some(0x14) {
                // Mimicat / Monster Reborn: c501 runs after the graveyard
                // pick (`start_heart`).
            } else if effect_type == Some(0x1F) {
                // Change of Heart: its handler runs after the selection
                // (`start_heart`).
            } else if card == roses_core::duel::FEATHER_DUSTER || card == roses_core::duel::RAIGEKI {
                // Events 7 / 5 (0x25A080 / 0x2593C0) with their Fake Trap /
                // Anti-Raigeki reactions [F NEWLY-ALLOWED.md 4, 6].
                magic_cross = true;
            } else if effect_type == Some(0x26) {
                // Solomon's Lawbook (event 0x21, 0x260760) [F 8.1].
                magic_lawbook = true;
            } else if card == DARK_HOLE {
                // Dark Hole (event 4, 0x258C60): its White Hole / Royal
                // Decree reactions, then c021 on the mask [F/emu
                // DARK-HOLE-BLOCKER-GRAVEROBBER.md].
                magic_dark_hole = true;
            } else if let (true, Some((side, amount))) = (bound.is_empty(), damage) {
                magic_burn = Some((side, amount));
            } else if bound.is_empty() && events.iter().any(|e| matches!(e, DuelEvent::Modified { .. })) {
                // Limiter Removal / Aqua Chorus: event 0x1D (0x25EF60), the
                // Boost sequence on every modified unit [F MORE-EFFECTS.md 1].
                magic_bind = events
                    .iter()
                    .filter_map(|e| match e {
                        DuelEvent::Modified { unit: u, .. } => Some(*u),
                        _ => None,
                    })
                    .collect();
                magic_boost = true;
            } else if bound.is_empty() {
                // Only c602 and c601 are shown: the card's own module is
                // not ported.
                report("a magic card's own effect presentation after c601 (field magic c502, LP recovery c508 and the spellbinding event 0 are ported)");
                Self::report_card(&mut self.reported_cards, card, events);
            } else {
                magic_bind = bound;
            }
        }
        let mut frozen = before.clone();
        frozen.marks.clear();
        frozen.path.clear();
        frozen.move_allowance = 0;
        let field: Vec<((u8, u8), u8)> = if kind == Kind::Magic {
            (0..49)
                .filter(|&i| self.session.duel.terrain.get(i) != duel_before.terrain.get(i))
                .map(|i| (((i % 7) as u8, (i / 7) as u8), self.session.duel.terrain[i]))
                .collect()
        } else {
            Vec::new()
        };
        // c502's scripts launch columns outer, rows inner [F 1.5].
        let mut field = field;
        field.sort_by_key(|&((c, r), _)| (c, r));
        let field_before = duel_before.terrain.clone();
        if !magic_bind.is_empty() {
            use crate::duel_monster_fx::{Effect, MonsterShow, Target};
            // The card is in the graveyard before the event's handler runs.
            let mut after = frozen.clone();
            after.units.retain(|u| (u.side, u.slot) != key);
            let targets: Vec<Target> = magic_bind
                .iter()
                .filter_map(|id| {
                    let p = positions.iter().find(|p| p.0 == *id)?;
                    let u = before.units.iter().zip(&duel_before.units).find(|(_, u)| u.id == *id).map(|(o, _)| o)?;
                    Some(Target { unit: p.1, cell: (p.2.column, p.2.row), card: u.card, face_up: u.face_up, defence: u.defence, face: u.face })
                })
                .collect();
            let effect = if magic_boost { Effect::Boost } else { Effect::Bind };
            self.after_activation = Some(Box::new(MonsterShow::new(effect, after, Some(duel_before.clone()), None, targets)));
        }
        if magic_ritual {
            use crate::duel_c059::{ResultSpec, RitualInput, Sacrifice, Unit};
            use crate::duel_monster_fx::{Effect, MonsterShow, Target};
            // Event 0x38: the sacrifices in mask order (g0 slots ascending,
            // then g1), every other card object, the result on the ritual
            // card's cell [F C059-RITUAL.md 8].
            let mut sacrificed: Vec<Target> = events
                .iter()
                .filter_map(|e| match e {
                    DuelEvent::Destroyed { unit: id, .. } if *id != unit => {
                        let p = positions.iter().find(|p| p.0 == *id)?;
                        let u = before.units.iter().zip(&duel_before.units).find(|(_, u)| u.id == *id).map(|(o, _)| o)?;
                        Some(Target { unit: p.1, cell: (p.2.column, p.2.row), card: u.card, face_up: u.face_up, defence: u.defence, face: u.face })
                    }
                    _ => None,
                })
                .collect();
            sacrificed.sort_by_key(|t| t.unit);
            let created = events.iter().find_map(|e| match e {
                DuelEvent::Created { unit: id, card: c, .. } => self.session.duel.units.iter().find(|u| u.id == *id).map(|u| (*c, u.slot)),
                _ => None,
            });
            if sacrificed.len() == 3 {
                let model = |t: &Target| t.face_up && (1..=5).contains(&t.unit.1);
                let sacrifices: [Sacrifice; 3] = std::array::from_fn(|k| {
                    let t = &sacrificed[k];
                    Sacrifice::on_cell(t.unit.0, t.unit.1, t.card, t.cell, model(t))
                });
                let others = before
                    .units
                    .iter()
                    .filter(|o| (o.side, o.slot) != key && !sacrificed.iter().any(|t| t.unit == (o.side, o.slot)))
                    .map(|o| Unit {
                        handle: (u16::from(o.side) << 8) | u16::from(o.slot),
                        alpha: 128,
                        model_scale: (o.slot == 0 || (o.face_up && o.slot <= 5)).then_some(1.0),
                    })
                    .collect();
                let input = RitualInput {
                    sacrifices,
                    others,
                    result: created.map(|(card_id, slot)| ResultSpec { card_id, slot }),
                    ritual_cell: (at.column, at.row),
                };
                let mut after = frozen.clone();
                after.units.retain(|u| (u.side, u.slot) != key);
                let all_up = sacrificed.iter().all(|t| t.face_up);
                let mut show = MonsterShow::new(Effect::Ritual, after, Some(duel_before.clone()), None, sacrificed);
                show.ritual = Some(input);
                show.ritual_start = if all_up { 2 } else { 10 };
                self.after_activation = Some(Box::new(show));
            }
        }
        if !magic_destroy.is_empty() {
            use crate::duel_monster_fx::{Effect, MonsterShow, Target};
            let mut after = frozen.clone();
            after.units.retain(|u| (u.side, u.slot) != key);
            let targets: Vec<Target> = magic_destroy
                .iter()
                .filter_map(|id| {
                    let p = positions.iter().find(|p| p.0 == *id)?;
                    let u = before.units.iter().zip(&duel_before.units).find(|(_, u)| u.id == *id).map(|(o, _)| o)?;
                    Some(Target { unit: p.1, cell: (p.2.column, p.2.row), card: u.card, face_up: u.face_up, defence: u.defence, face: u.face })
                })
                .collect();
            self.after_activation = Some(Box::new(MonsterShow::new(Effect::Destroy, after, Some(duel_before.clone()), None, targets)));
        }
        if let Some((moved, from, to)) = magic_shift {
            use crate::duel_monster_fx::{Effect, MonsterShow, Target};
            let mut after = frozen.clone();
            after.units.retain(|u| (u.side, u.slot) != key);
            let target = before.units.iter().zip(&duel_before.units).find(|(_, u)| u.id == moved).map(|(o, u)| Target {
                unit: (u.side, u.slot),
                cell: (from.column, from.row),
                card: o.card,
                face_up: o.face_up,
                defence: o.defence,
                face: o.face,
            });
            let mut show = MonsterShow::new(Effect::Shift, after, Some(duel_before.clone()), None, target.into_iter().collect());
            show.shift_to = Some((to.column, to.row));
            self.after_activation = Some(Box::new(show));
        }
        if let Some(traps) = magic_field {
            use crate::duel_monster_fx::{Effect, MonsterShow, Target};
            let mut after = frozen.clone();
            after.units.retain(|u| (u.side, u.slot) != key);
            let targets: Vec<Target> = traps
                .iter()
                .filter_map(|id| {
                    let p = positions.iter().find(|p| p.0 == *id)?;
                    let u = before.units.iter().zip(&duel_before.units).find(|(_, u)| u.id == *id).map(|(o, _)| o)?;
                    Some(Target { unit: p.1, cell: (p.2.column, p.2.row), card: u.card, face_up: u.face_up, defence: u.defence, face: u.face })
                })
                .collect();
            self.after_activation = Some(Box::new(MonsterShow::new(Effect::Field, after, Some(duel_before.clone()), None, targets)));
        }
        if let Some((side, amount)) = magic_burn {
            use crate::duel_monster_fx::{Effect, MonsterShow, Target};
            // Event 9 after c601: c509 from the card's cell to the victim's
            // leader [F PEGASUS-MAGIC.md 2.2].
            let mut after = frozen.clone();
            after.units.retain(|u| (u.side, u.slot) != key);
            let leader = duel_before.units.iter().find(|u| u.side == side && u.slot == 0).and_then(|l| {
                let o = before.units.iter().zip(&duel_before.units).find(|(_, u)| u.id == l.id).map(|(o, _)| o)?;
                Some(Target { unit: (side, 0), cell: (l.position.column, l.position.row), card: o.card, face_up: true, defence: false, face: o.face })
            });
            let mut show = MonsterShow::new(Effect::Burn, after, Some(duel_before.clone()), None, leader.into_iter().collect());
            show.burn = Some(((at.column, at.row), u32::from(amount)));
            self.after_activation = Some(Box::new(show));
        }
        let target_of = |id: roses_core::UnitId| -> Option<crate::duel_monster_fx::Target> {
            let p = positions.iter().find(|p| p.0 == id)?;
            let u = before.units.iter().zip(&duel_before.units).find(|(_, u)| u.id == id).map(|(o, _)| o)?;
            Some(crate::duel_monster_fx::Target { unit: p.1, cell: (p.2.column, p.2.row), card: u.card, face_up: u.face_up, defence: u.defence, face: u.face })
        };
        if magic_dark_hole {
            use crate::duel_monster_fx::{Effect, MonsterShow};
            let mut after = frozen.clone();
            after.units.retain(|u| (u.side, u.slot) != key);
            // Every destroyed unit (the record's mask after the reactions),
            // in mask order (g0 slots, then g1).
            let mut targets: Vec<crate::duel_monster_fx::Target> = events
                .iter()
                .filter_map(|e| match e {
                    DuelEvent::Destroyed { unit: u, .. } if *u != unit => target_of(*u),
                    _ => None,
                })
                .collect();
            targets.sort_by_key(|t| t.unit);
            // The reacting traps: one `Activated` per search.
            let mut reacting: Vec<crate::duel_monster_fx::Target> = events
                .iter()
                .filter_map(|e| match e {
                    DuelEvent::Activated { unit: u, .. } if *u != unit => target_of(*u),
                    _ => None,
                })
                .collect();
            let was_down = reacting.first().is_some_and(|t| !t.face_up);
            for t in &mut reacting {
                t.face_up = true;
            }
            let mask = targets.iter().fold(0u32, |m, t| m | crate::duel_c021::mask_bit(t.unit.0, t.unit.1));
            // c021's view of the board: every unit before the step but the
            // Dark Hole itself (in the graveyard before the event).
            let units: Vec<crate::duel_c021::UnitIn> = before
                .units
                .iter()
                .zip(&duel_before.units)
                .filter(|(o, _)| (o.side, o.slot) != key)
                .map(|(o, u)| {
                    let rot_y = side_model_base(o.side) - if o.defence { 0x4000 } else { 0 };
                    crate::duel_c021::UnitIn {
                        handle: (u16::from(o.side) << 8) | u16::from(o.slot),
                        position: crate::duel_c059::cell_centre((u.position.column, u.position.row)),
                        rotation: [0x4000, rot_y, if o.face_up { 0x8000 } else { 0 }],
                        colour: [128, 128, 128, 128],
                        card: o.slot != 0,
                        model: o.slot == 0 || (o.face_up && o.slot <= 5),
                    }
                })
                .collect();
            let mut show = MonsterShow::new(Effect::DarkHole, after, Some(duel_before.clone()), None, targets);
            show.reacting = reacting;
            show.trap_was_down = was_down;
            show.card_cell = Some((at.column, at.row));
            show.active_side = self.session.duel.active;
            show.c021_input = Some((mask, units, u64::from(self.frames as u32) ^ 0x15_0000_0015));
            self.after_activation = Some(Box::new(show));
        }
        if magic_cross || magic_lawbook {
            use crate::duel_monster_fx::{Effect, MonsterShow};
            let mut after = frozen.clone();
            after.units.retain(|u| (u.side, u.slot) != key);
            // A reacting trap: the step's second `Activated` (Fake Trap /
            // Anti-Raigeki; a Royal Decree lets the card go on [H: the
            // Decree's own c602 stages are not shown]).
            let reacting = events.iter().find_map(|e| match e {
                DuelEvent::Activated { unit: u, card: c, .. } if *u != unit => Some((*u, *c)),
                _ => None,
            });
            let react_type = reacting.and_then(|(_, c)| self.data.catalogue.card(c).and_then(|i| i.card_effect_type()));
            let destroyed_ids: Vec<roses_core::UnitId> = events
                .iter()
                .filter_map(|e| match e {
                    DuelEvent::Destroyed { unit: u, .. } if *u != unit => Some(*u),
                    _ => None,
                })
                .collect();
            let (effect, ids) = if magic_lawbook {
                (Some(Effect::Lawbook), Vec::new())
            } else if card == roses_core::duel::FEATHER_DUSTER {
                match (reacting, react_type) {
                    (Some((fake, _)), Some(roses_core::duel::trap_type::FAKE_TRAP)) => (Some(Effect::FakeTrap), vec![fake]),
                    // An empty cross mask: no event, nothing after c601 [F].
                    _ if destroyed_ids.is_empty() => (None, Vec::new()),
                    _ => (Some(Effect::Duster), destroyed_ids),
                }
            } else {
                match (reacting, react_type) {
                    (Some((anti, _)), Some(roses_core::duel::trap_type::ANTI_RAIGEKI)) => (Some(Effect::AntiRaigeki), vec![anti]),
                    // Event 5 runs for an empty mask too (its bolts) [F RG6].
                    _ => (Some(Effect::Raigeki), destroyed_ids),
                }
            };
            if let Some(effect) = effect {
                let mut targets: Vec<crate::duel_monster_fx::Target> = ids.into_iter().filter_map(target_of).collect();
                let was_down = targets.first().is_some_and(|t| !t.face_up);
                if matches!(effect, Effect::FakeTrap | Effect::AntiRaigeki) {
                    // Revealed at E: its face shows from then on.
                    for t in &mut targets {
                        t.face_up = true;
                    }
                }
                let mut show = MonsterShow::new(effect, after, Some(duel_before.clone()), None, targets);
                show.card_cell = Some((at.column, at.row));
                show.active_side = self.session.duel.active;
                show.trap_was_down = was_down;
                show.rng = crate::duel_fxlib::Rng::new(self.frames as u32 ^ 0x1F9);
                self.after_activation = Some(Box::new(show));
            }
        }
        // The trap plans' units: event 0x36's defender (record +4) and
        // attacker (+8), the mover of events 0x47 / 0x2E.
        let attacked = events.iter().find_map(|e| match e {
            DuelEvent::Attacked { attacker, defender } => Some((*attacker, *defender)),
            _ => None,
        });
        let plan_mover = events.iter().find_map(|e| match e {
            DuelEvent::Activated { unit: u, target, .. } if *u == unit => *target,
            _ => None,
        });
        // The units the trap removes: its `Destroyed` before any battle
        // (not a crush sweep's).
        let crushed: Vec<roses_core::UnitId> = events.iter().filter_map(|e| match e {
            DuelEvent::Crushed { unit: u } => Some(*u),
            _ => None,
        }).collect();
        let removed: Vec<roses_core::UnitId> = events
            .iter()
            .take_while(|e| !matches!(e, DuelEvent::Battle { .. }))
            .filter_map(|e| match e {
                DuelEvent::Destroyed { unit: u, .. } if *u != unit && !crushed.contains(u) => Some(*u),
                _ => None,
            })
            .collect();
        let mut applied_duel = None;
        if plan == Some(crate::duel_activate::TrapPlan::MirrorWall) {
            // The HUD after E+138: the attacker's ATK change as the step
            // left it before the battle.
            let mut d = duel_before.clone();
            for e in events.iter().take_while(|e| !matches!(e, DuelEvent::Battle { .. })) {
                if let DuelEvent::Modified { unit: u, attack, defense } = e {
                    if let Some(x) = d.units.iter_mut().find(|x| x.id == *u) {
                        x.attack_bonus = *attack;
                        x.defense_bonus = *defense;
                    }
                }
            }
            applied_duel = Some(d);
        }
        let plan_units = (
            attacked.and_then(|(_, d)| target_of(d)),
            attacked.and_then(|(a, _)| target_of(a)),
            plan_mover.and_then(target_of),
            removed.iter().copied().filter_map(target_of).collect::<Vec<_>>(),
        );
        let mut activation = Activation::new(kind, frozen, key, (at.column, at.row), card, victim, Some(duel_before));
        if let Some(plan) = plan {
            let (defender, attacker, mover, mut targets) = plan_units;
            if plan == crate::duel_activate::TrapPlan::WidespreadRuin {
                // A mover on the trap's cell is not listed (its c603).
                targets.retain(|t| !(mover.is_some_and(|m| m.unit == t.unit) && t.cell == (at.column, at.row)));
            }
            activation.defender = defender;
            activation.attacker = attacker;
            activation.mover = mover;
            activation.targets = targets;
            activation.applied_duel = applied_duel;
            activation = activation.with_plan(plan, self.frames as u32 ^ 0x22B);
        }
        if !field.is_empty() {
            activation.field = field;
            activation.field_before = field_before;
            report("field magic's sunk tile against the arena base (the depth test is not modelled)");
        }
        if negate {
            activation.big = false;
        }
        self.activation = Some(Box::new(activation));
    }

    /// Riryoku (power-up category 0, value 0: 0x24AB20 -> module 0x219).
    fn is_riryoku(&self, card: u16) -> bool {
        use roses_core::duel::CardData;
        self.data.catalogue.card(card).and_then(|c| c.power_up).is_some_and(|p| p.category == 0 && p.value == 0)
    }

    /// Silver Zombie / Tao (flip entry type 0x36 / 0x37): the units whose
    /// record kind or attribute the step rewrote.
    fn retyped(
        &self,
        before: &OverlayInput,
        duel_before: &roses_core::duel::Duel,
        unit: roses_core::UnitId,
    ) -> Option<(crate::duel_monster_fx::Effect, Vec<roses_core::UnitId>)> {
        use roses_core::duel::CardData;
        let card = before.units.iter().zip(&duel_before.units).find(|(_, u)| u.id == unit).map_or(0, |(o, _)| o.card);
        let ty = self.data.catalogue.card(card).and_then(|c| c.entries[3]).map(|[w, _]| w >> 8)?;
        if !matches!(ty, 0x36 | 0x37) {
            return None;
        }
        let changed: Vec<roses_core::UnitId> = self
            .session
            .duel
            .units
            .iter()
            .filter(|u| self.session.duel.records.get(&u.id.0) != duel_before.records.get(&u.id.0))
            .map(|u| u.id)
            .collect();
        (!changed.is_empty()).then_some((crate::duel_monster_fx::Effect::Retype, changed))
    }

    /// Patrol Robo / Water Element (flip entry type 0x18): the units the
    /// step revealed or concealed, other than the flipped card.
    fn flip_faces(
        &self,
        events: &[roses_core::duel::DuelEvent],
        before: &OverlayInput,
        duel_before: &roses_core::duel::Duel,
        unit: roses_core::UnitId,
    ) -> Option<(crate::duel_monster_fx::Effect, Vec<roses_core::UnitId>)> {
        use crate::duel_monster_fx::Effect;
        use roses_core::duel::{CardData, DuelEvent};
        let card = before.units.iter().zip(&duel_before.units).find(|(_, u)| u.id == unit).map_or(0, |(o, _)| o.card);
        if !self.data.catalogue.card(card).and_then(|c| c.entries[3]).is_some_and(|[w, _]| w >> 8 == 0x18) {
            return None;
        }
        let concealed: Vec<_> = events.iter().filter_map(|e| match e {
            DuelEvent::Concealed { unit: u, .. } => Some(*u),
            _ => None,
        }).collect();
        if !concealed.is_empty() {
            return Some((Effect::Conceal, concealed));
        }
        let revealed: Vec<_> = events.iter().filter_map(|e| match e {
            DuelEvent::Revealed { unit: u, .. } if *u != unit => Some(*u),
            _ => None,
        }).collect();
        (!revealed.is_empty()).then_some((Effect::Reveal, revealed))
    }

    /// Larva / Pupa of Moth's flip (flip entry type 0xC, event 0x43): the
    /// countdown starts and nothing is created (the evolution, event 0x3C,
    /// creates the next form).
    fn is_moth_countdown(
        &self,
        events: &[roses_core::duel::DuelEvent],
        before: &OverlayInput,
        duel_before: &roses_core::duel::Duel,
        unit: roses_core::UnitId,
    ) -> bool {
        let card = before.units.iter().zip(&duel_before.units).find(|(_, u)| u.id == unit).map_or(0, |(o, _)| o.card);
        use roses_core::duel::CardData;
        let moth = self.data.catalogue.card(card).and_then(|c| c.entries[3]).is_some_and(|[w, _]| w >> 8 == 0xC);
        moth && !events.iter().any(|e| matches!(e, roses_core::duel::DuelEvent::Created { .. }))
    }

    /// The turn start's crush (event 0x70, 0x274D90) on every crushed unit.
    fn start_crush(
        &mut self,
        events: &[roses_core::duel::DuelEvent],
        before: &OverlayInput,
        positions: &[(roses_core::UnitId, (u8, u8), GridPos)],
        duel_before: &roses_core::duel::Duel,
    ) {
        use crate::duel_monster_fx::{Effect, MonsterShow, Target};
        if self.monster_fx.is_some() || events.iter().any(|e| matches!(e, roses_core::duel::DuelEvent::Activated { .. })) {
            return;
        }
        let targets: Vec<Target> = events
            .iter()
            .filter_map(|e| match e {
                roses_core::duel::DuelEvent::Crushed { unit } => {
                    let p = positions.iter().find(|p| p.0 == *unit)?;
                    let u = before.units.iter().zip(&duel_before.units).find(|(_, u)| u.id == *unit).map(|(o, _)| o)?;
                    Some(Target { unit: p.1, cell: (p.2.column, p.2.row), card: u.card, face_up: u.face_up, defence: u.defence, face: u.face })
                }
                _ => None,
            })
            .collect();
        if targets.is_empty() {
            return;
        }
        let mut frozen = before.clone();
        frozen.marks.clear();
        frozen.path.clear();
        frozen.move_allowance = 0;
        self.monster_fx = Some(Box::new(MonsterShow::new(Effect::Crush, frozen, Some(duel_before.clone()), None, targets)));
    }

    /// Change of Heart's handler (event 0x1A, 0x25E300) once its selection
    /// is answered: the chosen unit changes controller at E+173
    /// [F C026-C027-CHANGE-OF-HEART.md].
    fn start_heart(
        &mut self,
        events: &[roses_core::duel::DuelEvent],
        before: &OverlayInput,
        positions: &[(roses_core::UnitId, (u8, u8), GridPos)],
        duel_before: &roses_core::duel::Duel,
    ) {
        use crate::duel_monster_fx::{Effect, MonsterShow, Target};
        if matches!(duel_before.pending, Some(roses_core::duel::Pending::GraveyardCopy { .. })) {
            // Mimicat / Monster Reborn: the revived unit appears at E+34.
            let Some((id, at, card)) = events.iter().find_map(|e| match e {
                roses_core::duel::DuelEvent::Revived { unit, at, card, .. } => Some((*unit, *at, *card)),
                _ => None,
            }) else {
                return;
            };
            let Some(u) = self.session.duel.units.iter().find(|u| u.id == id) else {
                return;
            };
            let target = Target { unit: (u.side, u.slot), cell: (at.column, at.row), card, face_up: false, defence: false, face: None };
            let mut frozen = before.clone();
            frozen.marks.clear();
            frozen.path.clear();
            frozen.move_allowance = 0;
            self.monster_fx = Some(Box::new(MonsterShow::new(Effect::Revive, frozen, Some(duel_before.clone()), None, vec![target])));
            return;
        }
        if !matches!(duel_before.pending, Some(roses_core::duel::Pending::ControlChange { .. })) {
            return;
        }
        let Some(id) = events.iter().find_map(|e| match e {
            roses_core::duel::DuelEvent::Controlled { unit, .. } => Some(*unit),
            _ => None,
        }) else {
            return;
        };
        let target = positions.iter().find(|p| p.0 == id).and_then(|p| {
            let u = before.units.iter().zip(&duel_before.units).find(|(_, u)| u.id == id).map(|(o, _)| o)?;
            Some(Target { unit: p.1, cell: (p.2.column, p.2.row), card: u.card, face_up: u.face_up, defence: u.defence, face: u.face })
        });
        let mut frozen = before.clone();
        frozen.marks.clear();
        frozen.path.clear();
        frozen.move_allowance = 0;
        self.monster_fx = Some(Box::new(MonsterShow::new(Effect::Heart, frozen, Some(duel_before.clone()), None, target.into_iter().collect())));
    }

    /// Larva / Pupa of Moth's evolution at its owner's turn start (entry 2
    /// type 0xC, event 0x3C): the moth is removed and the next form created.
    fn is_moth_evolution(
        &self,
        events: &[roses_core::duel::DuelEvent],
        before: &OverlayInput,
        duel_before: &roses_core::duel::Duel,
        unit: roses_core::UnitId,
    ) -> bool {
        use roses_core::duel::{CardData, DuelEvent};
        let card = before.units.iter().zip(&duel_before.units).find(|(_, u)| u.id == unit).map_or(0, |(o, _)| o.card);
        let moth = self.data.catalogue.card(card).and_then(|c| c.entries[2]).is_some_and(|[w, _]| w >> 8 == 0xC);
        // Shadow Ghoul's Labyrinth transform (123) runs the same event-0x3C
        // handler [F MOTH-EVOLUTION.md 2.6].
        let ghoul = card == 123;
        (moth || ghoul) && events.iter().any(|e| matches!(e, DuelEvent::Destroyed { unit: u, .. } if *u == unit))
            && events.iter().any(|e| matches!(e, DuelEvent::Created { .. }))
    }

    /// Names the card and the step's event kinds of an unpresented
    /// effect, once per card.
    fn report_card(reported: &mut BTreeSet<u16>, card: u16, events: &[roses_core::duel::DuelEvent]) {
        if reported.insert(card) {
            let kinds: Vec<String> = events
                .iter()
                .map(|e| format!("{e:?}").split([' ', '{', '(']).next().unwrap_or("").to_owned())
                .collect();
            eprintln!("Duel play: card {card}'s effect shown without its own module; step events {kinds:?}");
        }
    }

    /// A monster flip effect (`duel_monster_fx`): the reveal, then the
    /// effect's handler on the board; a battle in the same step waits for it
    /// (`deferred_battle`) [F MONSTER-EFFECTS-PRESENTATION.md 1].
    fn start_monster_fx(
        &mut self,
        events: &[roses_core::duel::DuelEvent],
        before: &OverlayInput,
        positions: &[(roses_core::UnitId, (u8, u8), GridPos)],
        duel_before: roses_core::duel::Duel,
        unit: roses_core::UnitId,
    ) {
        use crate::duel_monster_fx::{Effect, MonsterShow, Target};
        use roses_core::duel::DuelEvent;
        // Blocker's damage shield (battle pre-pass type 5): the engine marks
        // it with an `Activated` after the `Battle`; the original shows it in
        // the battle scene's first stage (box 0x105 + c513 on the unit,
        // `BattleIntro::effect_units`), nothing on the board [F 0x263D54;
        // DARK-HOLE-BLOCKER-GRAVEROBBER.md 3].
        {
            use roses_core::duel::CardData;
            let battle = events.iter().position(|e| matches!(e, DuelEvent::Battle { .. }));
            let shield = events.iter().position(|e| matches!(e, DuelEvent::Activated { unit: u, .. } if *u == unit));
            let card = duel_before.units.iter().find(|u| u.id == unit).map_or(0, |u| u.card);
            let pre_pass_5 = self.data.catalogue.card(card).and_then(|c| c.entries[0]).is_some_and(|[w, _]| w >> 8 == 5);
            if let (Some(b), Some(a), true) = (battle, shield, pre_pass_5) {
                if a > b {
                    return;
                }
            }
        }
        let target = |id: roses_core::UnitId| -> Option<Target> {
            let p = positions.iter().find(|p| p.0 == id)?;
            let u = before.units.iter().zip(&duel_before.units).find(|(_, u)| u.id == id).map(|(o, _)| o)?;
            Some(Target { unit: p.1, cell: (p.2.column, p.2.row), card: u.card, face_up: u.face_up, defence: u.defence, face: u.face })
        };
        let bound = events.iter().find_map(|e| match e {
            DuelEvent::Spellbound { unit: u } => Some(*u),
            _ => None,
        });
        let destroyed = events.iter().find_map(|e| match e {
            DuelEvent::Destroyed { unit: u, .. } if *u != unit => Some(*u),
            _ => None,
        });
        let boosted: Vec<roses_core::UnitId> = events
            .iter()
            .filter_map(|e| match e {
                DuelEvent::Modified { unit: u, .. } => Some(*u),
                _ => None,
            })
            .collect();
        // Key Mace's defence switch emits no event: compare the units.
        let turned: Vec<roses_core::UnitId> = duel_before
            .units
            .iter()
            .filter(|u| u.id != unit)
            .filter(|u| self.session.duel.units.iter().any(|n| n.id == u.id && n.defence && !u.defence))
            .map(|u| u.id)
            .collect();
        let (effect, ids) = if let Some(b) = bound {
            (Effect::Bind, vec![b])
        } else if let Some(d) = destroyed {
            (Effect::Destroy, vec![d])
        } else if !boosted.is_empty() {
            (Effect::Boost, boosted)
        } else if !turned.is_empty() {
            (Effect::Defence, turned)
        } else if let Some(flipped) = self.flip_faces(events, before, &duel_before, unit) {
            flipped
        } else if let Some(retyped) = self.retyped(before, &duel_before, unit) {
            retyped
        } else if events.iter().any(|e| matches!(e, DuelEvent::Battle { .. }))
            && events.iter().skip_while(|e| !matches!(e, DuelEvent::Activated { .. })).any(|e| matches!(e, DuelEvent::Damage { .. }))
        {
            // Yaiba Robo / Needle Ball: event 9 after the battle scene,
            // c509 from the dying unit's cell [F MORE-EFFECTS.md 2.5].
            let after = events.iter().skip_while(|e| !matches!(e, DuelEvent::Activated { .. })).find_map(|e| match e {
                DuelEvent::Damage { side, amount } => Some((*side, *amount)),
                _ => None,
            });
            if let (Some((side, amount)), Some(p)) = (after, positions.iter().find(|p| p.0 == unit)) {
                let cell = (p.2.column, p.2.row);
                let leader = duel_before.units.iter().find(|u| u.side == side && u.slot == 0).and_then(|l| {
                    let o = before.units.iter().zip(&duel_before.units).find(|(_, u)| u.id == l.id).map(|(o, _)| o)?;
                    Some(Target { unit: (side, 0), cell: (l.position.column, l.position.row), card: o.card, face_up: true, defence: false, face: o.face })
                });
                let mut frozen = before.clone();
                frozen.marks.clear();
                frozen.path.clear();
                frozen.move_allowance = 0;
                let mut show = MonsterShow::new(Effect::Burn, frozen, Some(duel_before.clone()), None, leader.into_iter().collect());
                show.burn = Some((cell, u32::from(amount)));
                self.after_battle = Some(Box::new(show));
            }
            return;
        } else if events.iter().any(|e| matches!(e, DuelEvent::Damage { .. })) {
            // Jirai Gumo: its LP damage on entering, then event 0xFF.
            (Effect::Notice, vec![unit])
        } else if self.is_moth_countdown(events, before, &duel_before, unit) {
            (Effect::Countdown, vec![unit])
        } else if self.is_moth_evolution(events, before, &duel_before, unit) {
            (Effect::Evolve, vec![unit])
        } else if let Some((id, at, card)) = events.iter().find_map(|e| match e {
            DuelEvent::Created { unit: u, at, card } => Some((*u, *at, *card)),
            _ => None,
        }) {
            // Kageningen (137): event 1 (0x2586A0) clones it, the Revive
            // sequence with c501 [F MORE-EFFECTS.md 8; H: the clone's
            // 8-update face-up flip at E+63 is not drawn].
            let Some(u) = self.session.duel.units.iter().find(|u| u.id == id) else {
                return;
            };
            let t = Target { unit: (u.side, u.slot), cell: (at.column, at.row), card, face_up: false, defence: false, face: None };
            let mut frozen = before.clone();
            frozen.marks.clear();
            frozen.path.clear();
            frozen.move_allowance = 0;
            let revealed = events.iter().find_map(|e| match e {
                DuelEvent::Revealed { unit: r, .. } if *r == unit => positions.iter().find(|p| p.0 == unit).map(|p| p.1),
                _ => None,
            });
            self.camera_command = None;
            self.monster_fx = Some(Box::new(MonsterShow::new(Effect::Revive, frozen, Some(duel_before), revealed, vec![t])));
            return;
        } else {
            if self.reported_activation.insert("a monster effect without a traced presentation") {
                eprintln!("Duel play: unsupported, not presented: a monster effect without a traced presentation");
            }
            let card = before.units.iter().zip(&duel_before.units).find(|(_, u)| u.id == unit).map_or(0, |(o, _)| o.card);
            Self::report_card(&mut self.reported_cards, card, events);
            return;
        };
        let mut targets: Vec<Target> = ids.into_iter().filter_map(target).collect();
        if effect == Effect::Countdown {
            // The moth's reveal precedes the handler: it is face up then.
            for t in &mut targets {
                t.face_up = true;
            }
        }
        let revealed = events.iter().find_map(|e| match e {
            DuelEvent::Revealed { unit: u, .. } if *u == unit => positions.iter().find(|p| p.0 == unit).map(|p| p.1),
            _ => None,
        });
        let mut frozen = before.clone();
        frozen.marks.clear();
        frozen.path.clear();
        frozen.move_allowance = 0;
        self.camera_command = None;
        let retype_attribute = {
            use roses_core::duel::CardData;
            let card = before.units.iter().zip(&duel_before.units).find(|(_, u)| u.id == unit).map_or(0, |(o, _)| o.card);
            self.data.catalogue.card(card).and_then(|c| c.entries[3]).is_some_and(|[w, _]| w >> 8 == 0x37)
        };
        let mut show = MonsterShow::new(effect, frozen, Some(duel_before), revealed, targets);
        show.retype_attribute = retype_attribute;
        if effect == Effect::Evolve {
            show.spawned = events.iter().find_map(|e| match e {
                roses_core::duel::DuelEvent::Created { unit: id, .. } => {
                    self.session.duel.units.iter().find(|u| u.id == *id).map(|u| (u.side, u.slot))
                }
                _ => None,
            });
        }
        self.monster_fx = Some(Box::new(show));
    }

    /// A `Battle` event starts the battle opening from the units' positions
    /// before the battle: both turn to face each other while the camera
    /// moves to the battle camera; a walk that the battle started (the
    /// attacker advancing) is played afterwards.
    fn start_intro(
        &mut self,
        events: &[roses_core::duel::DuelEvent],
        before: OverlayInput,
        positions: &[(roses_core::UnitId, (u8, u8), GridPos)],
        faces: &[(roses_core::UnitId, (u16, u8, i32, i32), bool)],
        fight: Option<roses_fx::types::BattleState>,
    ) {
        let trap_first = self.activation.as_ref().is_some_and(|a| a.t == 0 && a.plan.is_some())
            || self.deferred_activation.as_ref().is_some_and(|d| d.0.as_slice() == events);
        if (self.monster_fx.as_ref().is_some_and(|m| m.t == 0) || trap_first)
            && self.deferred_battle.is_none()
            && events.iter().any(|e| matches!(e, roses_core::duel::DuelEvent::Battle { .. }))
        {
            let mut faces = faces.to_vec();
            // Mirror Wall: the versus card shows the halved ATK (the step's
            // `Modified` before its battle).
            let duel = self.activation.as_ref().and_then(|a| a.duel.as_ref()).or(self.deferred_activation.as_ref().map(|d| &d.3));
            if let (true, Some(duel)) = (trap_first, duel) {
                for e in events.iter().take_while(|e| !matches!(e, roses_core::duel::DuelEvent::Battle { .. })) {
                    if let roses_core::duel::DuelEvent::Modified { unit, attack, .. } = e {
                        if let (Some(old), Some(f)) = (duel.units.iter().find(|u| u.id == *unit), faces.iter_mut().find(|f| f.0 == *unit)) {
                            f.1.2 = (f.1.2 + i32::from(*attack) - i32::from(old.attack_bonus)).max(0);
                        }
                    }
                }
            }
            self.deferred_battle = Some((events.to_vec(), before, positions.to_vec(), faces, fight));
            return;
        }
        for event in events {
            let roses_core::duel::DuelEvent::Battle {
                attacker, defender, ..
            } = event
            else {
                continue;
            };
            let find = |id: &roses_core::UnitId| positions.iter().find(|p| p.0 == *id).copied();
            let (Some(a), Some(d)) = (find(attacker), find(defender)) else {
                continue;
            };
            let walk = self.walk.take();
            let camera = battle_camera(a.2, d.2);
            self.camera_command = Some(camera);
            let face = |id: &roses_core::UnitId| faces.iter().find(|f| f.0 == *id).map(|f| f.1);
            // 0x2760E0's mode: damage and event bit 28, read here as "the
            // attacker is hit" (its side loses LP or it is destroyed) [H].
            let (mut amount, mut attacker_hit, mut destroyed) = (0i32, false, 0u8);
            // The battle's own events (Dark Elf's pre-pass LP change and a
            // trap's removals come before it).
            for e in events.iter().skip_while(|e| !matches!(e, roses_core::duel::DuelEvent::Battle { .. })) {
                match e {
                    roses_core::duel::DuelEvent::Damage { side, amount: a_ } => {
                        amount = -i32::from(*a_);
                        attacker_hit |= *side == a.1.0;
                    }
                    roses_core::duel::DuelEvent::Destroyed { unit, .. } if unit == attacker => {
                        attacker_hit = true;
                        destroyed |= 2;
                    }
                    roses_core::duel::DuelEvent::Destroyed { unit, .. } if unit == defender => {
                        destroyed |= 1;
                    }
                    _ => {}
                }
            }
            let damage = (crate::duel_return::mode(amount != 0, attacker_hit), amount);
            let revealed: Vec<((u8, u8), duel_overlay::BoardFace)> =
                [(a.1, attacker), (d.1, defender)]
                    .iter()
                    .filter_map(|(key, id)| {
                        let f = faces.iter().find(|f| f.0 == **id)?;
                        Some((
                            *key,
                            duel_overlay::BoardFace {
                                level: f.1.1,
                                attack: f.1.2,
                                defence: f.1.3,
                                effect: f.2,
                            },
                        ))
                    })
                    .collect();
            // A monster hitting the enemy leader (event 0x5D): the same
            // scene with the attacker's card only, no arena [F/emu
            // LEADER-BATTLE.md]; the leader's place holds no card.
            let leader = d.1.1 == 0;
            let versus_cards = match (a.1.1, d.1.1, face(attacker), face(defender)) {
                (1.., 1.., Some(fa), Some(fd)) => Some([
                    (a.1.0, (a.2.column, a.2.row), fa),
                    (d.1.0, (d.2.column, d.2.row), fd),
                ]),
                (1.., 0, Some(fa), _) => {
                    // The leader's card id picks its model for the close-up.
                    let card = before
                        .units
                        .iter()
                        .find(|u| u.side == d.1.0 && u.slot == 0)
                        .map_or(0, |u| u.card);
                    Some([
                        (a.1.0, (a.2.column, a.2.row), fa),
                        (d.1.0, (d.2.column, d.2.row), (card, 0, 0, 0)),
                    ])
                }
                _ => None,
            };
            if std::env::var_os("ROSES_TRACE_BATTLE").is_some() {
                eprintln!(
                    "battle at update {}: versus {}, leader {leader} ({:?} -> {:?}), damage {:?}",
                    self.frames,
                    versus_cards.is_some(),
                    a.1,
                    d.1,
                    damage
                );
            }
            // A pre-pass returning bit 16 (Dark Elf 0xD, Blocker 5 and the
            // others of `prepass_flags`) flags the unit for box 0x105 + c513
            // in the scene's first stage [F 8.2; F/emu
            // DARK-HOLE-BLOCKER-GRAVEROBBER.md 3].
            let effect_units: Vec<(u8, (u8, u8))> = if versus_cards.is_some() {
                use roses_core::duel::CardData;
                let card_at = |key: (u8, u8)| before.units.iter().find(|u| (u.side, u.slot) == key).map_or(0, |u| u.card);
                [(a.1, a.2, d.1), (d.1, d.2, a.1)]
                    .into_iter()
                    .filter(|(key, _, other)| {
                        let other_kind = self.data.catalogue.card(card_at(*other)).filter(|c| c.is_monster()).map(|c| c.kind);
                        self.data.catalogue.card(card_at(*key)).and_then(|c| c.entries[0]).is_some_and(|w| prepass_flags(w, key.1, other.1, other_kind))
                    })
                    .map(|(key, p, _)| (key.0, (p.column, p.row)))
                    .collect()
            } else {
                Vec::new()
            };
            // The battle states (0x36 turn, 0x3C camera, 0x39 scene) show
            // no move marks or path [emu K2 state log].
            let mut frozen = before.clone();
            frozen.state = 0x39;
            frozen.marks.clear();
            frozen.path.clear();
            frozen.move_allowance = 0;
            self.intro = Some(Box::new(BattleIntro {
                frozen,
                turns: vec![
                    (a.1, 0, face_target(a.1.0, a.2, d.2)),
                    (d.1, 0, face_target(d.1.0, d.2, a.2)),
                ],
                frame: 0,
                camera,
                versus_cards,
                arrived: None,
                versus: None,
                revealed,
                damage,
                destroyed,
                effect_battle: fight,
                arena_terrain: self
                    .session
                    .duel
                    .terrain
                    .get(usize::from(d.2.row) * 7 + usize::from(d.2.column))
                    .copied()
                    .unwrap_or(0),
                arena_end: None,
                leader,
                effect_units,
                effect_c513: Vec::new(),
                light: 128,
                after_track: if self.session.tutorial.is_none() && versus_cards.is_some() {
                    // 0x1D23B0: c84 = (floor(rand * 3) + 1) * 1000 + 1, then
                    // the battle track 17 after a fade [F; the port's own
                    // random value]. 0x1AB440(c84) selects by c84 % 1000 = 1
                    // (the duel's own track, as at the duel's entry) and adds
                    // the thousands: 1008..3008 for most opponents, 1010..3010,
                    // 1011.., 1012.., 1044.., 1040.. for the others [F
                    // DUEL-AUDIO-SCOPE.md 3.2].
                    let k = (self.frames.wrapping_mul(2_654_435_761) >> 16) % 3 + 1;
                    Some(k as u32 * 1000 + duel_music_track(self.data.first_duel.opponent, u32::from(self.data.first_duel.player_side)))
                } else {
                    None
                },
            }));
            if self.session.tutorial.is_none() && versus_cards.is_some() {
                self.audio.push(crate::duel_audio::AudioCue::Music { track: 17, keep: false });
            }
            // The advance (result code 6) walks after the battle scene.
            self.walk = walk;
            break;
        }
    }

    /// The duel's outcome once the screen may show it: a battle scene that
    /// ended the duel (a leader hit to 0 LP) plays to its end first; the
    /// original checks the end only in state 0x18 [F LEADER-BATTLE.md].
    pub fn shown_outcome(&self) -> Option<roses_core::duel::Outcome> {
        if self.intro.is_some() || self.exodia.is_some() {
            return None;
        }
        self.session.duel.outcome
    }

    /// The battle scene's light level (128 = full) for the board.
    pub fn board_light(&self) -> u8 {
        // While the hand row is up the board draws at 128 - d45 / 2
        // (0x1D0984 -> 0x2DF5B0 / 0x2DF5B1) [F/emu HAND-SLIDE-AND-FADE.md 1.7].
        let d45 = match (self.hand_view(), &self.session.input.mode) {
            (Some(v), _) => v.0,
            (None, Mode::Hand(_)) => 128,
            _ => 0,
        };
        let hand = 128 - d45 / 2;
        let activation = self.activation.as_ref().map_or(128, |a| a.light()).min(self.monster_fx.as_ref().map_or(128, |m| m.light()));
        let exodia = self.exodia.as_ref().map_or(128, |e| e.light());
        self.intro.as_ref().map_or(128, |i| i.light).min(hand).min(activation).min(exodia)
    }

    /// The unit camera's motion blur this update: the previous frame drawn
    /// over the new one with alpha n * 16 (0x1A9010(n): 6 from the first
    /// update of each close-up, 4 from its counter 12, 0 at 24) [F 0x2760E0,
    /// emu dumps-battle-blur]. 0 when off.
    pub fn motion_blur(&self) -> u8 {
        if let Some(e) = &self.exodia {
            return e.blur;
        }
        let Some(intro) = &self.intro else {
            return 0;
        };
        let (Some(at), Some(_)) = (intro.arrived, intro.versus_cards) else {
            return 0;
        };
        match (intro.frame - at).checked_sub(intro.load_hold()) {
            Some(u @ 1..=47) => match (u - 1) % 24 {
                0..=10 => 96,
                11..=22 => 64,
                _ => 0,
            },
            _ => 0,
        }
    }

    /// The battle scene hides the HUD (it slides out) from the unit camera
    /// on [emu dumps-battle-frames 59-73: the panels leave as the attacker
    /// close-up starts]; it shows again after the scene [H].
    pub fn scene_hides_hud(&self) -> bool {
        // The opening runs without the HUD (it comes in with the first
        // turn) [PCSX2 modes/older/intro].
        if self.opening.is_some() {
            return true;
        }
        // The Exodia event draws no HUD [emu: the tutorial's 183 / 184; H
        // for a duel with the HUD shown].
        if self.exodia.is_some() {
            return true;
        }
        // A trap's presentation runs without the HUD panels (689: c602 with
        // the box and no HUD; 690 after it: the HUD back) [emu; the slide
        // moments H: until the camera's way back].
        if self.activation.as_ref().is_some_and(|a| a.hides_hud()) {
            return true;
        }
        self.intro.as_ref().is_some_and(|i| {
            i.versus_cards.is_some() && i.arrived.is_some_and(|at| i.frame - at >= i.load_hold())
        })
    }

    /// (card, level, ATK, DEF) of every monster unit as the versus screen
    /// shows it (its current values on its cell).
    fn unit_faces(&self) -> Vec<(roses_core::UnitId, (u16, u8, i32, i32), bool)> {
        use roses_core::duel::queries::{Query, Record};
        let duel = &self.session.duel;
        let ctx = self.hud_context();
        let q = Query::new(duel, &self.data.catalogue, &ctx);
        duel.units
            .iter()
            .filter(|u| u.slot != 0)
            .filter_map(|u| {
                let r = Record::Field(u);
                let stats = q.stats(r, u.position)?;
                Some((
                    u.id,
                    (
                        u.card,
                        q.unit_cost(u).max(0) as u8,
                        stats.attack,
                        stats.defence,
                    ),
                    q.has_effect(r),
                ))
            })
            .collect()
    }

    /// A `Moved` event starts a walk along the selection's cells (origin
    /// first) to the destination, or straight from the origin when the
    /// selection does not hold that unit.
    fn start_walk(
        &mut self,
        events: &[roses_core::duel::DuelEvent],
        selection: Option<(roses_core::UnitId, Vec<GridPos>)>,
    ) {
        for event in events {
            let roses_core::duel::DuelEvent::Moved { unit, from, to } = event else {
                continue;
            };
            if from == to {
                continue;
            }
            let mut cells = match &selection {
                Some((held, path)) if held == unit && path.first() == Some(from) => path.clone(),
                _ => vec![*from],
            };
            if cells.last() != Some(to) {
                cells.push(*to);
            }
            self.walk = Some(Walk {
                unit: *unit,
                cells,
                dispatches: 0,
            });
        }
    }

    /// One update with exactly these presses (replays; no auto-repeat).
    /// `None` once the session has stopped.
    pub fn press(&mut self, presses: u16) -> Option<Frame> {
        if self.halted.is_some() {
            return None;
        }
        // Watching: Start leaves; a match that may no longer be watched ends
        // the watch, and a seat whose match is gone (the server restarted, or
        // cannot be reached) goes back to the menu.
        if let Some(link) = self.online.as_ref() {
            if link.spectator && presses & pad::START != 0 {
                self.halted = Some(crate::duel_online::STOPPED_WATCHING.into());
                return None;
            }
            if let Some(why) = link.sent_away.clone() {
                self.halted = Some(why);
                return None;
            }
        }
        // N-player (a port addition): the Target key and the placing list.
        #[cfg(feature = "online")]
        let presses = if self.multi.is_some() {
            let p = self.multi_update(presses);
            if self.halted.is_some() {
                return None;
            }
            p
        } else {
            presses
        };
        let side = self.session.duel.active;
        let active_at_start = side;
        let who = self.side_name(side);
        self.frames += 1;
        if let Some(link) = self.online.as_mut() {
            link.tick();
        }
        // The unit controller turns a card toward its position, 0x20 per
        // update (+0x55) [F 0x1DE540]; a new card starts at its position.
        let units = &self.session.duel.units;
        self.sideways.retain(|id, _| units.iter().any(|u| u.id == *id));
        for u in units.iter().filter(|u| u.slot != 0) {
            let target = if u.defence { 0xFFu8 } else { 0 };
            let p = self.sideways.entry(u.id).or_insert(target);
            *p = match target.cmp(p) {
                std::cmp::Ordering::Greater => p.saturating_add(0x20),
                std::cmp::Ordering::Less => p.saturating_sub(0x20),
                // At rest: without this a defence card (0xFF) dropped to
                // 0xDF and back every other update (the board rocked it).
                std::cmp::Ordering::Equal => *p,
            };
        }
        self.last_script = None;
        // A private test, not gameplay: `ROSES_EXODIA_TEST=<side>` gives
        // that side a complete Exodia on the first other side's turn (when
        // nothing animates), so its next turn start wins
        // (`duel_exodia::rig_for_test`). Local duels only.
        if self.online.is_none() && !self.animating() && self.session.tutorial.is_none() {
            static RIGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if let Some(side) = std::env::var("ROSES_EXODIA_TEST").ok().and_then(|v| v.trim().parse::<u8>().ok()) {
                let d = &mut self.session.duel;
                if d.outcome.is_none() && d.active != side && !RIGGED.load(std::sync::atomic::Ordering::Relaxed) {
                    RIGGED.store(true, std::sync::atomic::Ordering::Relaxed);
                    if crate::duel_exodia::rig_for_test(d, side) {
                        eprintln!("Duel play: ROSES_EXODIA_TEST: side {side} has Exodia complete from duel update {} (a private test)", self.frames);
                    }
                }
            }
        }
        if self.frames == 1 {
            // Duel scene entry: 0x1AB440(1, 1) by scene param 1 (the
            // opponent; 0 for the practice duels) [F/emu DUEL-AUDIO-SCOPE.md
            // 3.2-3.3; H: param 2 is 0 in campaign duels].
            // Scene param 2 is the player's side (0x232E40 p2 = SAVE[0];
            // 0x1AE640's second argument): a White Rose duel plays 9.
            let opponent = if self.session.tutorial.is_some() { 0 } else { self.data.first_duel.opponent };
            let track = duel_music_track(opponent, u32::from(self.data.first_duel.player_side));
            self.audio.push(crate::duel_audio::AudioCue::Music { track, keep: true });
        }
        // N-player (a port addition): a knockout's presentation (LEAVE)
        // once the board is free; the pad is not read while it plays.
        #[cfg(feature = "online")]
        if self.knockout_step() {
            return Some(Frame::default());
        }
        let arena_available = self.arena_available;
        // A human side's ○ (pad 0x20) skips the arena's attack phase when it
        // is the active side or the other side is the computer (0x1DF760
        // controller bytes) [F ARENA-ATTACK.md 2].
        let seats = &self.session.seats;
        let skip_pressed = presses & pad::CIRCLE != 0
            && (0..2).any(|s| seats[s].is_none() && (usize::from(self.session.duel.active & 1) == s || seats[1 - s].is_some()));
        if let Some(intro) = &mut self.intro {
            intro.frame += 1;
            for t in &mut intro.turns {
                t.1 = turn_step(t.1, t.2);
            }
            let turned = intro.turns.iter().all(|t| t.1 == t.2);
            if intro.arrived.is_none() && intro.frame > BATTLE_CAMERA_FRAMES && turned {
                intro.arrived = Some(intro.frame);
            }
            let done = match (intro.arrived, intro.versus_cards) {
                (None, _) => false,
                (Some(_), None) => true,
                (Some(at), Some(cards)) => {
                    let t = intro.frame - at;
                    // The unit camera (0x2760E0 sub-phases 0 and 1).
                    let unit = |i: usize, near: bool, frames: u32| {
                        let (side, cell, face) = cards[i];
                        // Attacker and defender, in the order of `turns`.
                        let turn = intro.turns.get(i).map_or(0, |u| u.1);
                        UnitCamera {
                            side,
                            cell,
                            card: face.0,
                            turn,
                            add_distance: if near { 100 } else { 300 },
                            cell_distance: if near { 180 } else { 280 },
                            frames,
                            active_side: cards[0].0,
                        }
                    };
                    let hold = intro.load_hold();
                    // Dark Elf's stage: c513 on each flagged unit (16 U, SE
                    // (1, 0x10)) at the scene's first stage.
                    if !intro.effect_units.is_empty() {
                        match t.checked_sub(intro.scene_hold()) {
                            Some(u) if u < crate::duel_monster_mods::C513::UPDATES => {
                                if u == 0 {
                                    intro.effect_c513 = intro
                                        .effect_units
                                        .iter()
                                        .enumerate()
                                        .map(|(i, &(_, cell))| {
                                            let [x, z] = crate::duel_board::cell_center(cell.0, cell.1);
                                            crate::duel_monster_mods::C513::new([x, 0.0, z], 0x201 + i as u32 + intro.frame)
                                        })
                                        .collect();
                                    self.audio.push(crate::duel_audio::AudioCue::Se { slot: 1, id: 0x10, skip: false });
                                }
                                for c in &mut intro.effect_c513 {
                                    c.update();
                                }
                            }
                            _ => intro.effect_c513.clear(),
                        }
                    }
                    match t.checked_sub(hold) {
                        Some(0) => self.unit_camera = Some(unit(0, false, 0)),
                        Some(1) => {
                            self.unit_camera = Some(unit(0, true, 12));
                            // The close-up sound (0, 0x27) [F/emu LEADER-BATTLE.md].
                            self.audio.push(crate::duel_audio::AudioCue::Se { slot: 0, id: 0x27, skip: false });
                        }
                        Some(24) => self.unit_camera = Some(unit(1, false, 0)),
                        Some(25) => {
                            self.unit_camera = Some(unit(1, true, 12));
                            self.audio.push(crate::duel_audio::AudioCue::Se { slot: 0, id: 0x27, skip: false });
                        }
                        Some(48) => {
                            // 0x262390: back to the camera of the scene's start.
                            self.camera_command = Some(CameraMove {
                                frames: 0,
                                ..intro.camera
                            });
                        }
                        _ => {}
                    }
                    // 0x5D launches c604 one update after the dim ends.
                    let launch = hold + UNIT_CAMERA_UPDATES + VERSUS_LAUNCH_HOLD + u32::from(intro.leader);
                    if t == launch {
                        let defence = intro.defence(&cards);
                        intro.versus = start_versus(&intro.camera, &cards, defence);
                        if let Some(v) = &mut intro.versus {
                            v.single = intro.leader;
                        }
                    } else if let Some(v) = &mut intro.versus {
                        v.tick_with(self.arena_effects.as_mut());
                        if let Some(b) = v.arena.as_mut().and_then(|a| a.battle.as_mut()) {
                            // A live effect that returned other than the
                            // cached run predicted may end at another update.
                            if std::mem::take(&mut b.effect_diverged) {
                                intro.arena_end = Some(t + b.length());
                            }
                            if b.done() {
                                self.arena_effects = None;
                            }
                        }
                        if skip_pressed {
                            if let Some(b) = v.arena.as_mut().and_then(|a| a.battle.as_mut()) {
                                if b.skip() {
                                    intro.arena_end = Some(t + b.length());
                                    // The executor keeps calling the effect
                                    // through the exit: the worker rewinds to
                                    // this update and skips there too.
                                    if let Some(w) = self.arena_effects.as_mut() {
                                        w.skip();
                                    }
                                }
                            }
                        }
                        // The arena units' motion sound cues (slots 2 / 3).
                        if let Some(b) = v.arena.as_mut().and_then(|a| a.battle.as_mut()) {
                            for (slot, id) in b.sounds.drain(..) {
                                self.audio.push(crate::duel_audio::AudioCue::Se {
                                    slot,
                                    id: id as u8,
                                    skip: id & 0x8000 != 0,
                                });
                            }
                        }
                    }
                    let fly = crate::duel_versus::FLY_UPDATES as u32;
                    // The arena: the crossfade and the StageCam orbit; its
                    // attack phase is not drawn, so the return (c604's flash
                    // passes and the number) follows the orbit.
                    let arena_start = launch + fly + VERSUS_REST_HOLD;
                    let arena_start = if intro.leader { launch + fly } else { arena_start };
                    if arena_available && !intro.leader && t == arena_start {
                        if let Some(v) = &mut intro.versus {
                            v.arena = Some(crate::duel_versus::ArenaShot {
                                terrain: intro.arena_terrain,
                                frame: 0,
                                battle: None,
                                effect: None,
                            });
                            if let Some(provider) = &self.arena_provider {
                                (provider.prefetch)([cards[0].2 .0, cards[1].2 .0]);
                            }
                            intro.arena_end = Some(
                                arena_start
                                    + crate::duel_versus::ARENA_FADE_UPDATES
                                    + crate::duel_versus::ARENA_ORBIT_UPDATES,
                            );
                        }
                    }
                    // Phase 5: the attack starts when the orbit ends.
                    if let (Some(v), Some(end)) = (&mut intro.versus, intro.arena_end) {
                        if t == end {
                            if let (Some(shot), Some(provider)) = (&mut v.arena, &self.arena_provider) {
                                if shot.battle.is_none() {
                                    let mut cards = [cards[0].2 .0, cards[1].2 .0];
                                    let mut destroyed = intro.destroyed;
                                    let mut fight = intro.effect_battle;
                                    // Research: `ROSES_ARENA_FORCE=attacker,defender,destroyed`
                                    // plays another pair of monsters in the arena (captures).
                                    if let Some(f) = crate::duel_bmodule::forced_battle() {
                                        cards = [f.0, f.1];
                                        destroyed = f.2;
                                        fight = None;
                                    }
                                    eprintln!("Duel arena: attack phase of cards {cards:?} starts at frame {}", self.frames);
                                    match (provider.setup)(shot.terrain, cards) {
                                        Ok(setup) => {
                                            let outcome = crate::duel_arena_battle::Outcome::from_battle(
                                                destroyed,
                                                cards_attack(&v.cards, 0),
                                                cards_attack(&v.cards, 1),
                                            );
                                            let battle = crate::duel_arena_battle::ArenaBattle::new(
                                                setup.units,
                                                outcome,
                                                setup.stage_end,
                                                t ^ 0xa7ac,
                                            );
                                            intro.arena_end = Some(end + battle.length());
                                            // The attack effects, run live one update ahead.
                                            self.arena_effects = (provider.effect_inputs)(cards, self.data.texeff.as_deref())
                                                .filter(|_| std::env::var_os("ROSES_BMODULE_OFF").is_none())
                                                .map(|mut inputs| {
                                                    inputs.battle = fight;
                                                    crate::duel_bmodule::EffectWorker::start(&battle, inputs)
                                                });
                                            // Phase 2 loads both monsters' sound groups
                                            // (0x1E64E0 -> 0x216D20(2 / 3, card)) [F].
                                            self.audio.push(crate::duel_audio::AudioCue::Group { slot: 2, card: cards[0] });
                                            self.audio.push(crate::duel_audio::AudioCue::Group { slot: 3, card: cards[1] });
                                            shot.battle = Some(battle);
                                        }
                                        Err(error) => eprintln!(
                                            "Duel play: unsupported, not drawn: the arena's attack phase ({error})"
                                        ),
                                    }
                                }
                            }
                        }
                    }
                    let start = intro.arena_end.unwrap_or(arena_start);
                    if arena_available && t == start {
                        if let Some(v) = &mut intro.versus {
                            v.arena = None;
                        }
                        self.arena_effects = None;
                    }
                    let (mode, amount) = intro.damage;
                    if amount != 0 && t == start + 6 {
                        // The damage peak (1, 0x41) [F/emu LEADER-BATTLE.md: U85].
                        self.audio.push(crate::duel_audio::AudioCue::Se { slot: 1, id: 0x41, skip: false });
                    }
                    if t == burn_sound_at(start, mode) {
                        self.audio.push(crate::duel_audio::AudioCue::Se { slot: 0, id: 2, skip: false });
                    }
                    if t == start {
                        if let Some(v) = &mut intro.versus {
                            v.ret = Some(crate::duel_return::Return::new(mode, amount, t ^ 0x5eed));
                        }
                    }
                    // c605 follows c604's return.
                    let burn = start + crate::duel_return::Return::length(mode);
                    // The lights: 9 updates of -15 ending at 0 in the update
                    // c604 starts (0x5E) or the one before (0x5D) [F], dark
                    // until c605 starts, then +15 per update [emu for 0x5D,
                    // H for 0x5E].
                    let dark = launch - u32::from(intro.leader);
                    intro.light = if t + 9 <= dark {
                        128
                    } else if t <= dark {
                        (128 - 15 * (9 - (dark - t)) as i32).max(0) as u8
                    } else if t <= burn {
                        0
                    } else {
                        (15 * (t - burn)).min(128) as u8
                    };
                    if t == burn {
                        let defence = intro.defence(&cards);
                        if let Some(v) = &mut intro.versus {
                            let board = [0, 1].map(|i| {
                                crate::duel_versus::board_card_pose(cards[i].0, cards[i].1, defence[i])
                            });
                            let mut end = crate::duel_burn::Return605::new(
                                intro.destroyed,
                                &v.cards,
                                board,
                                t ^ 0xb0b,
                            );
                            if v.single {
                                end.survivors.retain(|s| s.index == 0);
                            }
                            v.end = Some(end);
                        }
                    }
                    let end = burn + crate::duel_burn::Return605::length(intro.destroyed);
                    (intro.versus.is_none() && t >= launch) || t >= end
                }
            };
            if done {
                if let Some(track) = self.intro.as_ref().and_then(|i| i.after_track) {
                    // The battle's end restores the duel music as the
                    // random variant k * 1000 + 8 (0x1AB440(c84, 0)) [F/emu].
                    self.audio.push(crate::duel_audio::AudioCue::Music { track, keep: false });
                }
                self.intro = None;
                // A unit's death effect (Yaiba Robo) follows the battle.
                if let Some(mut show) = self.after_battle.take() {
                    // The board as the battle left it [H: the dying unit's
                    // fade on its cell is not drawn].
                    let mut live = overlay_input(&self.session);
                    { let ctx = self.hud_context(); add_board_faces(&mut live, &self.session, &self.data.catalogue, &ctx); }
                    show.frozen = live;
                    self.monster_fx = Some(show);
                }
                if !self.reported_scene {
                    self.reported_scene = true;
                    eprintln!(
                        "Duel play: unsupported, not drawn: in the battle scene the attack effects' 0x1EF8F0 matrix argument, effect draws into a unit's offscreen target, the previous frame (frame-buffer textures) in single-frame captures (the effects themselves with their GS clamp / filter / perspective, afterimages, unit tints, colour passes and composites, scales, stage light, depth test, frame copies of the frame drawn so far and sound cues are drawn and played: BMODULE run live), terrain 6's extra lights and terrains 5-7's per-update light changes, effect point lights, terrains whose stage records are not decoded, exact random values; the battle is resolved after the return"
                    );
                }
            }
            return Some(Frame::default());
        }
        let cursor = self.cursor();
        if let Some(n) = &mut self.destiny_event {
            *n -= 1;
            if *n == 0 {
                self.destiny_event = None;
            }
            return Some(Frame::default());
        }
        // The cursor camera's rest (the active side's setting).
        let board_rest = self.board_polar();
        if let Some(m) = &mut self.monster_fx {
            use crate::duel_monster_fx::CameraStep;
            m.tick();
            for (bank, id) in m.sounds() {
                self.audio.push(crate::duel_audio::AudioCue::Se { slot: bank, id, skip: false });
            }
            if let Some(step) = m.camera() {
                if let CameraStep::Orbit(frames, dyaw) = step {
                    self.camera_orbit = Some((frames, dyaw));
                    self.camera_command = None;
                    self.unit_camera = None;
                }
                let back = matches!(step, CameraStep::Back(_));
                let (unit, command) = camera_for_step(step, self.session.duel.active, cursor);
                if unit.is_some() {
                    self.unit_camera = unit;
                }
                if command.is_some() {
                    let rest = board_rest;
                    self.camera_command = command.map(|m| if back { CameraMove { polar: rest, ..m } } else { m });
                }
            }
            if m.done() {
                self.monster_fx = None;
                if let Some((events, before, positions, mut faces, fight)) = self.deferred_battle.take() {
                    // The scene shows the units as the effect left them
                    // (Hoshiningen's +300 in the versus cards) [F order].
                    let live = self.unit_faces();
                    for f in &mut faces {
                        if let Some(l) = live.iter().find(|l| l.0 == f.0) {
                            *f = *l;
                        }
                    }
                    self.start_intro(&events, before, &positions, &faces, fight);
                }
            }
            return Some(Frame::default());
        }
        if let Some(a) = &mut self.activation {
            // The activation handlers run with the pad unread.
            for id in a.tick() {
                self.audio.push(crate::duel_audio::AudioCue::Se { slot: 1, id, skip: false });
            }
            if let Some(v) = a.victim_camera_starts() {
                self.unit_camera = Some(UnitCamera {
                    side: v.side,
                    cell: v.cell,
                    // A face-down victim shows no model: the cell branch.
                    card: if v.face_up { v.card } else { NO_MODEL },
                    turn: 0,
                    add_distance: 100,
                    cell_distance: 180,
                    frames: v.camera_frames,
                    active_side: self.session.duel.active,
                });
            }
            if let Some(n) = a.field_camera_starts() {
                if n == 1 {
                    let (cell, _) = a.field[0];
                    self.unit_camera = Some(UnitCamera {
                        side: a.unit.0,
                        cell,
                        card: NO_MODEL,
                        turn: 0,
                        add_distance: 100,
                        cell_distance: 180,
                        frames: 0,
                        active_side: self.session.duel.active,
                    });
                } else {
                    // 0x262060(30): the overview, target 0, polar (500, yaw
                    // 0x3FFF / 0xBFFF, pitch 0x17FF) [F 1.5].
                    let angle = |v: i32| f64::from(v as f32 * std::f32::consts::PI / 32768.0);
                    let yaw = side_yaw(self.session.duel.active) - 1;
                    self.camera_command = Some(CameraMove {
                        target: [0.0, 0.0, 0.0],
                        polar: [500.0, angle(yaw), angle(0x17FF)],
                        frames: 30,
                    });
                }
            }
            if let Some(step) = a.plan_camera() {
                let back = matches!(step, crate::duel_monster_fx::CameraStep::Back(_));
                let (unit, command) = camera_for_step(step, self.session.duel.active, cursor);
                if unit.is_some() {
                    self.unit_camera = unit;
                }
                if command.is_some() {
                    let rest = board_rest;
                    self.camera_command = command.map(|m| if back { CameraMove { polar: rest, ..m } } else { m });
                }
            }
            if a.restore_starts() {
                // 0x262390(ctx, 16): back to the camera of the handler's
                // start (the cursor camera) [F].
                let c = cursor;
                self.camera_command = Some(CameraMove {
                    target: crate::duel_board::cursor_target((c.column, c.row)),
                    polar: board_rest,
                    frames: a.restore_frames(),
                });
            }
            if a.done() {
                self.activation = None;
                // A magic card's own event follows c601 (the Jar's Bind).
                self.monster_fx = self.after_activation.take();
                // Mirror Wall's battle goes on with the halved attacker.
                if self.monster_fx.is_none() {
                    if let Some((events, before, positions, faces, fight)) = self.deferred_battle.take() {
                        self.start_intro(&events, before, &positions, &faces, fight);
                    }
                }
            }
            return Some(Frame::default());
        }
        if let Some(walk) = &mut self.walk {
            // Presses during the walk are not polled (dropped).
            walk.dispatches += 1;
            if walk.dispatches >= walk.total() {
                self.walk = None;
                if let Some((events, before, positions, duel)) = self.deferred_activation.take().map(|d| *d) {
                    self.start_activation(&events, &before, &positions, duel);
                }
            }
            return Some(Frame::default());
        }
        if let Some(flight) = &mut self.summon_flight {
            // 0x4C-0x50 and 0x16 do not read the pad.
            let more = flight.tick();
            if let Some(id) = flight.fusion_playing().and_then(|f| f.sound()) {
                self.audio.push(crate::duel_audio::AudioCue::Se { slot: 1, id, skip: false });
            }
            // Riryoku's pair (0x219): c537 with the drained amount; the
            // stats, the opponent's LP and the graveyard at its u 23.
            if flight.fusion_playing().and_then(|f| f.equip_running()) == Some(0x219) {
                if self.equip_number.is_none() && !self.riryoku.is_empty() {
                    let param = self.riryoku.remove(0);
                    self.equip_number = Some(Box::new((crate::duel_equip::EquipFx::riryoku(0x219 + self.frames as u32, param), 0)));
                }
                if let Some((fx, u)) = self.equip_number.as_deref_mut() {
                    *u += 1;
                    fx.update(*u);
                    if *u == crate::duel_c537::APPLY_U {
                        self.summon_duel = None;
                    }
                }
            } else {
                self.equip_number = None;
            }
            if flight.fusion_playing().is_none() && flight.step == crate::duel_summon::FLIGHT_UPDATES + 1 {
                // 0x4F commit: placement sound (0, 8).
                self.audio.push(crate::duel_audio::AudioCue::Se { slot: 0, id: 8, skip: false });
            }
            if !more {
                self.summon_flight = None;
                self.summon_duel = None;
                self.riryoku.clear();
                self.equip_number = None;
            }
            return Some(Frame::default());
        }
        if let Some(anim) = &mut self.hand_anim {
            // States 0x40-0x42 / 0x45 do not read the pad.
            anim.step += 1;
            if anim.kind == crate::duel_hand::HandKind::Open && anim.step >= 9 && (anim.step - 9) % 3 == 0 && anim.step < anim.total() {
                // 0x41: each new card slides in with (0, 7).
                self.audio.push(crate::duel_audio::AudioCue::Se { slot: 0, id: 7, skip: false });
            }
            if anim.done() {
                if anim.kind == crate::duel_hand::HandKind::Close {
                    self.hand_seen[usize::from(side & 3)] = anim.count;
                }
                self.hand_anim = None;
            }
            return Some(Frame::default());
        }
        // The Exodia event 0x6D (handler 0x26DAA0): after the turn start's
        // other presentations; the pad is not read until the result.
        if let Some(mut run) = self.exodia.take() {
            run.tick(self);
            if !run.done {
                self.exodia = Some(run);
            }
            return Some(Frame::default());
        }
        // The details view: 0x20 slides it in (d3a + 8 per update, the
        // Triangle update itself leaves it at 0) and polls from the update
        // it reaches 0x40 (0x21 runs in the same update); Circle's 0x22 runs
        // in its own update (d3a − 8) and goes on − 8 per update until 0,
        // then the opening state returns. 0x20 and 0x22 do not read the pad
        // [F 0x1D4C04..0x1D4DA8; emu modes/ui3 dd-leader.json].
        let details_before = self.session.input.details;
        if details_before.is_some() && self.details_slide < DETAILS_REST {
            self.details_slide += 8;
            if self.details_slide < DETAILS_REST {
                return Some(Frame::default());
            }
        } else if details_before.is_none() && self.details_slide > 0 {
            self.details_slide -= 8;
            if self.details_slide == 0 {
                self.details_shown = None;
            }
            return Some(Frame::default());
        }
        let local_multi = self.multi.as_ref().is_some_and(|m| m.local.is_some());
        let (side, who, result, b) = if self.online.is_some() {
            match self.online_press(presses) {
                Some(step) => step,
                None => return Some(Frame::default()),
            }
        } else if local_multi {
            match self.local_multi_press(presses) {
                Some(step) => step,
                None => return Some(Frame::default()),
            }
        } else {
            let b = self.capture_before(side);
            (side, who, tick(&mut self.session, &self.data, presses), b)
        };
        match (details_before, self.session.input.details) {
            // Opened this update: 0x20 with d3a 0.
            (None, Some(_)) => self.details_slide = 0,
            // Circle: 0x22's first step in the same update.
            (Some(_), None) => {
                self.details_slide = self.details_slide.saturating_sub(8);
                if self.details_slide == 0 {
                    self.details_shown = None;
                }
            }
            _ => {}
        }
        if let Some(view) = self.session.input.details {
            self.details_shown = Some(self.details_of(view));
        }
        let Before {
            mode: mode_before,
            hand: hand_before,
            field_chain,
            selection,
            overlay: before,
            positions,
            faces,
            duel: duel_before,
        } = b;
        let terrain_before = duel_before.terrain.clone();
        if let Ok(frame) = &result {
            let fight = effect_battle(&duel_before, &frame.events, &self.data.catalogue, &self.hud_context());
            self.start_walk(&frame.events, selection);
            self.start_heart(&frame.events, &before, &positions, &duel_before);
            self.start_crush(&frame.events, &before, &positions, &duel_before);
            if frame.events.iter().any(|e| matches!(e, roses_core::duel::DuelEvent::Equipped { power_up, .. } if self.is_riryoku(*power_up))) {
                self.summon_duel = Some(duel_before.clone());
            }
            #[cfg(feature = "online")]
            self.queue_knockouts(&frame.events, &duel_before, &faces);
            self.start_activation(&frame.events, &before, &positions, duel_before);
            if frame.events.iter().any(|e| matches!(e, roses_core::duel::DuelEvent::DestinyDraw { .. })) {
                self.destiny_event = Some(DESTINY_EVENT_UPDATES);
                if self.reported_activation.insert("the Destiny Draw's leader script 0x271 and sounds") {
                    eprintln!("Duel play: unsupported, not drawn: the Destiny Draw's leader animation (script 0x271) and its sounds; the \"Destiny Draw!\" box is shown");
                }
            }
            // Field magic's change is presented by c502 (`Activation`); any
            // other change only switches the tiles.
            if !self.terrain_logged
                && self.session.duel.terrain != terrain_before
                && self.activation.is_none()
                && self.deferred_activation.is_none()
            {
                self.terrain_logged = true;
                eprintln!("Duel play: unsupported, not presented: a terrain change outside field magic (the board tiles switch at once)");
            }
            self.start_intro(&frame.events, before, &positions, &faces, fight);
            self.start_hand(&mode_before, side);
            self.start_summon(&mode_before, &frame.events, &hand_before, field_chain);
            self.start_exodia(&frame.events);
        }
        let shown = self.present_result(result, &who, &mode_before);
        if (self.online.is_some() || local_multi) && shown.is_some() {
            let turn_changed = self.session.duel.active != active_at_start;
            crate::duel_online::settle_input(&mut self.session.input, &self.session.duel, turn_changed);
        }
        shown
    }

    /// The end of [`Self::press`]: sounds, status and the halt of an error.
    fn present_result(
        &mut self,
        result: std::result::Result<Frame, String>,
        who: &str,
        mode_before: &Mode,
    ) -> Option<Frame> {
        match result {
            Err(error) if self.apply_stand_in(&error) => Some(Frame::default()),
            Err(error) => {
                let message = format!("update {}: {who}: {error}", self.frames);
                eprintln!("Duel play stopped (last state kept): {message}");
                self.halted = Some(message);
                None
            }
            Ok(frame) => {
                self.last_script = frame.script.clone();
                for e in &frame.events {
                    if matches!(e, roses_core::duel::DuelEvent::TurnEnded { .. }) {
                        // 0x13 end of turn (0, 0x1D).
                        self.audio.push(crate::duel_audio::AudioCue::Se { slot: 0, id: 0x1D, skip: false });
                    }
                }
                if let (Some(key), Some(response)) = (frame.key, &frame.response) {
                    if let Some(id) = input_sound(mode_before.state(), key, response) {
                        self.audio.push(crate::duel_audio::AudioCue::Se { slot: 0, id, skip: false });
                    }
                    let keys = button_names(key);
                    match response {
                        Response::Refused => {
                            self.status = Some(format!("{who} {keys} refused"));
                        }
                        Response::Unsupported(what) => {
                            if self.reported.insert(*what) {
                                eprintln!("Duel play: unsupported, nothing changed: {what}");
                            }
                            self.status = Some(format!("{who} {keys}: unsupported {what}"));
                        }
                        _ => {}
                    }
                }
                Some(frame)
            }
        }
    }

    /// A lesson key the engine refused at a step with a traced outcome
    /// (`script::stand_in`): apply that outcome explicitly and count the
    /// refused frame as the poll it was in the original (the script moves
    /// to the next step, counter 0), as the model's tests do. Returns
    /// whether the refusal was taken over.
    fn apply_stand_in(&mut self, error: &str) -> bool {
        let Some(t) = self.session.tutorial.as_ref() else {
            return false;
        };
        let Some(stand) = stand_in(t.lesson, t.step) else {
            return false;
        };
        // `DuelError::Unsupported` prints as "not yet supported: ..."; a
        // refusal of any other kind is a real stop.
        if !(error.contains("not yet supported") || error.contains("nsupported")) {
            return false;
        }
        let step = t.step;
        let t = self.session.tutorial.as_mut().expect("checked above");
        t.polls += 1;
        t.step = step + 1;
        t.counter = 0;
        let controller = t.lesson.controller();
        stand.apply(&mut self.session.duel, &mut self.session.input);
        eprintln!(
            "Tutorial: stand-in at lesson {controller} step {step}: {} (engine: {error})",
            stand.note
        );
        self.stand_ins.push((step, stand.note));
        self.status = Some(format!("stand-in at step {step}: {}", stand.note));
        true
    }

    /// The terrain the board shows: the duel's from before an activation
    /// while it plays (field magic's change shows at its end), else live.
    pub fn shown_terrain(&self) -> Vec<u8> {
        if let Some((terrain, _)) = self.activation.as_ref().and_then(|a| a.field_board()) {
            return terrain;
        }
        self.activation
            .as_ref()
            .and_then(|a| a.duel.as_ref())
            .map_or(&self.session.duel.terrain[..], |d| &d.terrain[..])
            .to_vec()
    }

    /// The cells' heights (cell +0x14, 0 normally; field magic's c502
    /// slides a changing tile down and back up).
    pub fn shown_heights(&self) -> [i16; 49] {
        if let Some(o) = &self.opening {
            return [o.height; 49];
        }
        self.activation.as_ref().and_then(|a| a.field_board()).map_or([0; 49], |(_, h)| h)
    }

    /// Set while the duel's terrain differs from the instanced tiles.
    pub fn terrain_note(&self) -> Option<String> {
        let changed = self
            .session
            .duel
            .terrain
            .iter()
            .zip(&self.board_terrain)
            .filter(|(a, b)| a != b)
            .count();
        (changed > 0).then(|| {
            format!("terrain changed at {changed} cells since the duel's start")
        })
    }

    fn side_name(&self, side: u8) -> String {
        if let Some(m) = &self.multi {
            let name = m.name(side);
            return if m.seat == Some(side) { format!("you ({name})") } else { name };
        }
        if self.spectating() {
            let name = self.seat_names.as_ref().map(|n| n[usize::from(side & 1)].clone()).unwrap_or_default();
            return format!("{}P {name}", side + 1);
        }
        if let Some(seat) = self.online_seat() {
            return if side == seat { format!("you (seat {seat})") } else { format!("the opponent (seat {})", seat ^ 1) };
        }
        if self.session.tutorial.is_some() {
            return if side == 0 {
                "you (scripted)".into()
            } else {
                "enemy (scripted)".into()
            };
        }
        let computer = self.session.seats[usize::from(side)].is_some();
        match (side, computer) {
            (0, false) => "you".into(),
            (0, true) => "player (computer)".into(),
            _ => opponent_name(self.data.first_duel.opponent).into(),
        }
    }

    fn card_name(&self, id: u16) -> &str {
        self.data
            .names
            .get(usize::from(id))
            .map_or("?", String::as_str)
    }

    fn mode_text(&self) -> String {
        let duel = &self.session.duel;
        let side = usize::from(duel.active);
        let hand = &duel.sides[side].hand;
        // A computer's hand stays hidden, as its cards are face down.
        let shown = match (self.online_seat(), &self.multi) {
            (_, Some(m)) => m.seat.is_some_and(|s| usize::from(s) == side),
            (Some(seat), None) => usize::from(seat) == side,
            (None, None) => self.session.seats[side].is_none(),
        };
        let card = |i: usize| {
            hand.get(i)
                .map_or("?".to_string(), |&id| self.card_name(id).to_string())
        };
        let picks = |cursor: usize, marks: &[usize]| {
            let list: Vec<String> = if marks.is_empty() {
                vec![card(cursor)]
            } else {
                marks.iter().map(|&i| card(i)).collect()
            };
            list.join(" + ")
        };
        match &self.session.input.mode {
            Mode::Field => "field 0x18".into(),
            Mode::Move(sel) => format!(
                "move 0x1D step {}/{}{}{}",
                sel.path.len(),
                sel.allowance,
                if sel.defence { " defence" } else { "" },
                if sel.face_up { " face-up" } else { "" }
            ),
            Mode::SummonCell { .. } => "summon cell 0x3E".into(),
            Mode::Hand(menu) if shown => format!(
                "hand 0x43 [{}/{}] {}{}",
                menu.cursor + 1,
                hand.len(),
                card(menu.cursor),
                if menu.marks.is_empty() {
                    String::new()
                } else {
                    format!(" (marked {})", picks(menu.cursor, &menu.marks))
                }
            ),
            Mode::Hand(menu) => format!("hand 0x43 [{}/{}]", menu.cursor + 1, hand.len()),
            Mode::Confirm(menu) if shown => {
                format!("confirm 0x46: {}", picks(menu.cursor, &menu.marks))
            }
            Mode::Confirm(_) => "confirm 0x46".into(),
            Mode::SelectUnit(_) => "select unit 0x30".into(),
            Mode::Graveyard(_) => "graveyard 0x2D".into(),
        }
    }

    /// Window title: round, side to act, LP, stars, input state, the last
    /// refused/unsupported press, an unsupported terrain change and the end.
    pub fn title(&self) -> String {
        let duel = &self.session.duel;
        let cursor = self.cursor();
        let heading = match self.session.tutorial.as_ref() {
            Some(t) => {
                let page = match t.message {
                    Some(m) => format!(" page {}", m.shown(t.text_variant)),
                    None => String::new(),
                };
                let hud = if t.hud_flag {
                    ""
                } else {
                    " [HUD hidden; camera close-up not reproduced]"
                };
                format!(
                    "Roses tutorial lesson {} (controller {}) step {}/{}{page}{hud}",
                    t.lesson.series_index() + 1,
                    t.lesson.controller(),
                    t.step,
                    t.lesson.step_count(),
                )
            }
            None => match &self.online {
                Some(link) if link.spectator => format!(
                    "Roses online duel, watching (revision {}){}",
                    link.shown_revision,
                    link.note.as_ref().map_or(String::new(), |n| format!(", {n}"))
                ),
                Some(link) => format!(
                    "Roses online duel, seat {} (revision {}){}",
                    link.seat,
                    link.shown_revision,
                    link.note.as_ref().map_or(String::new(), |n| format!(", {n}"))
                ),
                None => format!("Roses duel vs {}", opponent_name(self.data.first_duel.opponent)),
            },
        };
        let mut parts = vec![
            heading,
            format!("round {}", duel.round),
            format!("{} to act", self.side_name(duel.active)),
            format!("LP {}", duel.sides.iter().map(|s| s.lp.to_string()).collect::<Vec<_>>().join(" / ")),
            format!("stars {}", duel.sides.iter().map(|s| s.stars.to_string()).collect::<Vec<_>>().join(" / ")),
            format!("{} at ({},{})", self.mode_text(), cursor.column, cursor.row),
        ];
        if let Some(outcome) = &duel.outcome {
            let reason = match outcome.reason {
                EndReason::LifePoints => "life points",
                EndReason::Surrender => "surrender",
                EndReason::TurnLimit => "turn limit",
                EndReason::Exodia => "Exodia",
                EndReason::LeaderTrapped => "leader trapped",
            };
            parts.push(match (outcome.winner, self.online.is_some()) {
                // Online: the 2-player result (seat 0 is 1P).
                (Some(side), true) => format!("{}P WIN ({}; {reason})", side + 1, self.side_name(side)),
                (None, true) => format!("DRAW ({reason})"),
                (Some(side), false) => format!("DUEL OVER: {} won ({reason})", self.side_name(side)),
                (None, false) => format!("DUEL OVER: draw ({reason})"),
            });
        }
        if let Some(halted) = &self.halted {
            parts.push(format!("STOPPED: {halted}"));
        }
        if let Some(status) = &self.status {
            parts.push(format!("last: {status}"));
        }
        if let Some(note) = self.terrain_note() {
            parts.push(note);
        }
        parts.join(" · ")
    }
}

// ---------------------------------------------------------------- HUD
/// Side of the HUD layer (the GS frame the HUD routine draws into).
pub const HUD_SIZE: usize = 512;

/// Controller type (side +0x33C) of a seat: 100 for the pad (the player's
/// value in every saved state of the first duel), 2 for a computer seat
/// (Weevil's value; the port's computer uses Weevil's controller on either
/// side).
pub fn controller_type(computer: bool) -> u8 {
    if computer { 2 } else { 100 }
}

/// The getters' original context for the session: option words 0x1C/0x1D
/// both 3 (every recorded duel), controller types by seat, no control
/// changes (the engine keeps none), R2 on the pad of a human side 0.
pub fn hud_context(session: &DuelSession, held: u16) -> Context {
    hud_context_with(session, None, [held, 0])
}

impl DuelPlay {
    /// The getters' context of this duel: Custom Duel's params 0x1C/0x1D,
    /// both pads' R2.
    pub fn hud_context(&self) -> Context {
        #[allow(unused_mut)]
        let mut ctx = self.hud_context_2p();
        // N-player (a port addition, MODES): a player always sees its own
        // cards and a 2v2 team-mate's (Card Display OPEN for those sides,
        // bit = the real side), enemies' stay hidden (HIDE, and R2 shows
        // nothing: their hidden cards are not sent).
        #[cfg(feature = "online")]
        if let Some(mask) = self.multi_own_sides() {
            ctx.option_29 = mask;
            ctx.r2_held = [false; 2];
        }
        ctx
    }

    /// N-player: the sides whose cards this screen shows as its own (bit
    /// per side): its seat and the seat's team-mate; the offline preview's
    /// viewer seat. None outside an N-player duel and for a spectator.
    #[cfg(feature = "online")]
    pub fn multi_own_sides(&self) -> Option<u32> {
        let m = self.multi.as_ref()?;
        let me = if m.local.is_some() { m.viewer() } else { self.online_seat()? };
        let duel = &self.session.duel;
        Some((0..duel.side_count()).filter(|&s| s == me || duel.team_mates(me, s)).fold(0, |b, s| b | 1 << s))
    }

    /// The side of a 2-seat online duel this screen belongs to
    /// ([`roses_client::duel_hud::HudExtra::own_side`]). None offline and
    /// for a spectator.
    pub fn hud_own_side(&self) -> Option<u8> {
        self.online.as_ref().filter(|l| !l.spectator).map(|l| l.seat & 1)
    }

    fn hud_context_2p(&self) -> Context {
        if let Some(link) = self.online.as_ref() {
            // Online: the original's 2-player duel (pad 1 = seat 0, pad 2 =
            // seat 1) with the match's Leader Ability / Card Display words;
            // only this seat's pad is here, so only its R2 is read (the
            // other seat's R2 never reveals anything: its hidden cards are
            // not sent). HIDE exists because the original's two players
            // share a screen; this screen is the seat's own, so its own side
            // counts as OPEN here (a port addition, as `multi_own_sides`).
            let seat = usize::from(link.seat & 1);
            let [option_28, mut option_29] = link.options().words();
            if !link.spectator {
                option_29 |= 1 << seat;
            }
            let pad = roses_core::duel::PAD_CONTROLLER;
            let mut r2_held = [false; 2];
            // A spectator has no cards of its own to show.
            r2_held[seat] = !link.spectator && self.held_pads[0] & pad::R2 != 0;
            return Context { option_28, option_29, controllers: [pad, pad + 1], control_changed: Vec::new(), r2_held };
        }
        hud_context_with(&self.session, self.data.first_duel.options, self.held_pads)
    }
}

/// [`DuelPlay::online_note`]'s lines (two spaces split them).
pub fn online_note_text(spectator: bool, watchers: u16, away: Option<&str>) -> Option<String> {
    let line = match (spectator, watchers) {
        (true, n) if n > 1 => Some(format!("SPECTATING  {n} WATCHING")),
        (true, _) => Some("SPECTATING".into()),
        (false, 0) => None,
        (false, n) => Some(format!("{n} WATCHING")),
    };
    match (away, line) {
        (Some(a), Some(l)) => Some(format!("{a}  {l}")),
        (Some(a), None) => Some(a.into()),
        (None, l) => l,
    }
}

/// [`hud_context`] with the duel's option words and both pads: with two
/// pad sides the controllers are the duel's (100 / 101) and R2 is read on
/// port `side` (0x203380(d3c, 1, 2)) [F].
pub fn hud_context_with(session: &DuelSession, options: Option<[u32; 2]>, held: [u16; 2]) -> Context {
    let two = session.tutorial.is_none()
        && session.seats.iter().all(Option::is_none)
        && session.duel.controllers.iter().all(|&c| c >= roses_core::duel::PAD_CONTROLLER);
    let [o28, o29] = options.unwrap_or([3, 3]);
    if two {
        return Context {
            option_28: o28,
            option_29: o29,
            controllers: [session.duel.controllers[0], session.duel.controllers[1]],
            control_changed: Vec::new(),
            r2_held: [held[0] & pad::R2 != 0, held[1] & pad::R2 != 0],
        };
    }
    let held = held[0];
    Context {
        option_28: o28,
        option_29: o29,
        // A practice lesson scripts both sides (+0x33C = the lesson).
        controllers: match session.tutorial.as_ref() {
            Some(t) => [t.lesson.controller(); 2],
            None => [0, 1].map(|s| controller_type(session.seats[s].is_some())),
        },
        control_changed: Vec::new(),
        // R2 on the pad's side (the first human side).
        r2_held: {
            let human = (0..2).find(|&s| session.seats[s].is_none());
            [0, 1].map(|s| human == Some(s) && held & pad::R2 != 0)
        },
    }
}

/// 0x2154C0(side, card) for the first duel: the leaders' ranks from their
/// deck records (the leader word's high nibble: player 1, Weevil 2, as the
/// saved states hold them); card 671 (read as 999), ids >= 683 and sides >= 2
/// have none (-1). Other cards get rank 0: the HUD shows ranks and ability
/// icons only for leaders, so their save-data rank does not change a
/// primitive.
pub fn first_duel_rank(leaders: [u16; 2], ranks: [i32; 2], side: u8, card: u16) -> i32 {
    if side > 1 || card == 671 || card >= 683 {
        -1
    } else if card == leaders[usize::from(side)] {
        ranks[usize::from(side)]
    } else {
        0
    }
}

/// The original duel HUD for a [`DuelPlay`] session: tables and textures
/// from the import's verbatim files, the HUD state from the engine
/// (`duel_hud::ui_from_input` + `duel_hud::from_duel`) once per update with
/// the routine's write-backs carried over, and a premultiplied RGBA8
/// [`HUD_SIZE`]² layer re-rendered only when the primitive list changes.
/// C9C of a hidden OPTION panel.
const SETTINGS_HIDDEN: i16 = 0x200;

pub struct DuelHud {
    tables: HudTables,
    textures: HudTextures,
    leaders: [u16; 2],
    ranks: [i32; 2],
    /// The HUD bytes the routine wrote back in the last update.
    pub previous: HudUpdate,
    /// The input state (d57) of the last update.
    last_state: u8,
    /// The d57 the HUD drew with in the last update (with the states the
    /// port's input machine does not model: 0x20/0x22, 0x52, 0x4D).
    pub shown_d57: u8,
    pub flash: FlashWave,
    /// The duel-end result states hide the HUD (its panel slides out by 8
    /// per update until D3E reaches 0x80, which state 0x56 waits for):
    /// clears the shown bit of d67 every update while set.
    pub hidden: bool,
    /// The last update's frame.
    pub frame: Option<HudFrame>,
    /// Premultiplied RGBA8 layer of the last rendered frame.
    pub layer: Vec<u8>,
    /// Bumped whenever `layer` changes.
    pub version: u64,
    /// Layer renders so far and their total time.
    pub renders: u64,
    pub render_time: std::time::Duration,
    rendered: Option<(Vec<Primitive>, Vec<HandCard>, u64)>,
    reported: BTreeSet<String>,
    /// Small card faces of the hand row (0x1AFAC0), when PICTURE.MRG is
    /// attached ([`DuelHud::attach_pictures`]).
    faces: Option<HandFaces>,
    /// The hand cards drawn under the HUD this update.
    hand: Vec<HandCard>,
    /// The last update showed the hand row's motion or picked cards.
    hand_shown: bool,
    /// The terrain picture's renderer (RT 0x2000), when the board import
    /// is attached ([`DuelHud::attach_picture`]).
    picture: Option<crate::duel_hud_picture::TerrainPicture>,
    /// The main camera's forward vector for the picture's preview camera.
    pub picture_forward: [f64; 3],
    /// Bumped whenever the picture changes (it forces a layer render).
    picture_version: u64,
    /// The GRAVEYARD panel's slide (0..=64) and its last contents.
    graveyard_slide: i16,
    graveyard_last: Option<duel_hud::GraveyardUi>,
    /// The YES/NO tables the OPTION panel's builder takes.
    yes_no: Option<roses_client::duel_hud::YesNoTables>,
    /// L1's settings panel: C9C (0x200 hidden .. 0 in place) and what it
    /// showed last (drawn while it slides out).
    settings_slide: i16,
    settings_last: Option<roses_client::duel_hud::OptionPanel>,
    /// The hand view's fade d45 (0x52) and its last HUD input (drawn while
    /// it fades out).
    hand_view_fade: u8,
    hand_view_last: Option<duel_hud::HudUi>,
    /// The details view's pane 0x1BE6E0 (`card_detail`): the executable's
    /// tables, the static card records, PICTURE.MRG when attached and the
    /// card whose picture `textures.picture` holds.
    detail: Option<(crate::card_detail::DetailData, roses_core::deck_editor::StaticCards)>,
    detail_pictures: Option<crate::card_detail::Pictures>,
    detail_block: roses_core::card_block::CardBlock,
    detail_picture: Option<u16>,
    /// N-player (a port addition): the pair of sides the HUD showed last.
    #[cfg_attr(not(feature = "online"), allow(dead_code))]
    multi_pair: Option<[u8; 2]>,
    /// N-player: the leader cards in the compact plates' portrait slots
    /// (slot 2 + side).
    #[cfg_attr(not(feature = "online"), allow(dead_code))]
    multi_portraits: Vec<u16>,
}

/// One hand card object: its screen position floats, face, turn angle,
/// large-face flag and the unaffordable dimming (`duel_hand`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandCard {
    pub position: [f32; 2],
    pub face: HandFace,
    pub theta: f32,
    pub large: bool,
    pub dim: bool,
}

/// Draw a hand card under the premultiplied `layer`: the quad of `pose`
/// (its turn about the vertical axis at camera depth 800, `duel_hand`),
/// sampled per screen column through the inverse projection; `dim` halves
/// the colour (vertex RGB 64).
pub fn draw_hand_card_under(
    layer: &mut [u8],
    width: usize,
    height: usize,
    image: &RgbaImage,
    pose: &crate::duel_hand::HandPose,
    dim: bool,
) {
    use crate::duel_hand::{local_x, project, HALF};
    let xs = [project(pose, -HALF[0], 0.0)[0], project(pose, HALF[0], 0.0)[0]];
    let (x0, x1) = (xs[0].min(xs[1]), xs[0].max(xs[1]));
    let back = pose.theta.cos() < 0.0;
    let (sin, _) = pose.theta.sin_cos();
    let cy = pose.position[1];
    for x in (x0.floor().max(0.0) as usize)..(x1.ceil().max(0.0) as usize).min(width) {
        let lx = local_x(pose, x as f32 + 0.5);
        if !(lx.abs() <= HALF[0]) {
            continue;
        }
        let z = 800.0 + lx * sin;
        let top = 256.0 + (1.342_857 * cy - HALF[1]) * 800.0 / z;
        let bottom = 256.0 + (1.342_857 * cy + HALF[1]) * 800.0 / z;
        let mut u = (lx + HALF[0]) / (2.0 * HALF[0]);
        if back {
            u = 1.0 - u;
        }
        for y in (top.floor().max(0.0) as usize)..(bottom.ceil().max(0.0) as usize).min(height) {
            let v = (y as f32 + 0.5 - top) / (bottom - top);
            if !(0.0..=1.0).contains(&v) {
                continue;
            }
            let t = card_face::sample_bilinear(
                image,
                f64::from(u * image.width as f32),
                f64::from(v * image.height as f32),
            );
            if t[3] == 0 {
                continue;
            }
            let o = (y * width + x) * 4;
            let cover = 255 - i32::from(layer[o + 3]);
            for c in 0..3 {
                let texel = if dim { t[c].clamp(0, 255) * 64 / 128 } else { t[c].clamp(0, 255) };
                let v = i32::from(layer[o + c]) + texel * cover / 255;
                layer[o + c] = v.min(255) as u8;
            }
            layer[o + 3] = 255;
        }
    }
}

/// The card-face compositor for the hand row and its composed faces.
struct HandFaces {
    assets: FaceAssets,
    /// Faces by (face, large) for the hand row (`draw_hand_card_under`).
    sized: HashMap<(HandFace, bool), Option<RgbaImage>>,
    /// Large board card images by (card, values).
    board: HashMap<(u16, duel_overlay::BoardFace), Option<RgbaImage>>,
}

/// Screen rectangle of a hand card object at rest relative to its position
/// floats (0x2073E0 vertices on 683-p2-x1: x 208 + fx .. 304 + fx,
/// y 162.057 + fy .. 355.429 + fy) [emu].
pub const HAND_CARD_RECT: [f32; 4] = [208.0, 162.057, 304.0, 355.429];

/// Draw `face` stretched over `rect` (HUD layer pixels) under the
/// premultiplied `layer`, sampled bilinearly. The card is drawn opaque
/// where the face has coverage [H: the object's GS blend state is not
/// traced; the composed face covers its whole rectangle].
pub fn draw_card_under(
    layer: &mut [u8],
    width: usize,
    height: usize,
    face: &RgbaImage,
    rect: [f32; 4],
) {
    let [x0, y0, x1, y1] = rect;
    let (su, sv) = (
        face.width as f32 / (x1 - x0),
        face.height as f32 / (y1 - y0),
    );
    let first = |a: f32| (a.ceil().max(0.0)) as usize;
    for y in first(y0)..(y1.ceil().max(0.0) as usize).min(height) {
        for x in first(x0)..(x1.ceil().max(0.0) as usize).min(width) {
            let u = (x as f32 - x0) * su;
            let v = (y as f32 - y0) * sv;
            let t = card_face::sample_bilinear(face, f64::from(u), f64::from(v));
            if t[3] == 0 {
                continue;
            }
            let o = (y * width + x) * 4;
            let cover = 255 - i32::from(layer[o + 3]);
            for c in 0..3 {
                let v = i32::from(layer[o + c]) + t[c].clamp(0, 255) * cover / 255;
                layer[o + c] = v.min(255) as u8;
            }
            layer[o + 3] = 255;
        }
    }
}

impl DuelHud {
    pub fn new(play: &DuelPlay) -> Result<Self> {
        let data = &play.data;
        let decks = roses_assets::parse_pal_decks(&data.executable).map_err(|e| e.to_string())?;
        let leaders = [0, 1].map(|s| data.first_duel.sides[s].leader);
        let mut ranks = [0; 2];
        for side in 0..2 {
            let record = decks
                .decks
                .get(data.first_duel.sides[side].deck_record)
                .ok_or("deck record outside the executable's table")?;
            // A deck whose leader is not its record's (an edited player
            // deck, Custom Duel's mirror) shows the duel's leader rank.
            ranks[side] = if record.leader_card == leaders[side] && play.online.is_none() {
                i32::from(record.leader_rank_raw)
            } else {
                i32::from(data.first_duel.leader_ranks[side])
            };
        }
        let mut tables = HudTables::from_files(&data.executable, &data.messages)?;
        // The details view's pane composes the card labels, descriptions
        // and the rest of `card_detail::message_ids`.
        let detail = match (
            crate::card_detail::DetailData::from_executable(&data.executable),
            roses_core::deck_editor::StaticCards::from_executable(&data.executable, 0),
        ) {
            (Ok(d), Some(c)) => {
                for chunk in crate::card_detail::message_ids().chunks(256) {
                    let parsed = roses_assets::parse_pal_messages(&data.messages, chunk).map_err(|e| e.to_string())?;
                    for m in parsed.messages {
                        if tables.message(m.message_id).is_none() {
                            tables.set_message(m.message_id, m.expanded_codes);
                        }
                    }
                }
                Some((d, c))
            }
            (d, c) => {
                eprintln!("Duel HUD: the details view's pane is not drawn: {:?} (card records read: {})", d.err(), c.is_some());
                None
            }
        };
        let detail_pictures = data.pictures.as_ref().and_then(|p| crate::card_detail::Pictures::new(p.clone(), &data.executable).ok());
        let textures = HudTextures::new(&data.texsys, &data.picmini, leaders, &data.messages)?;
        let session = &play.session;
        let mut hud = Self {
            tables,
            textures,
            leaders,
            ranks,
            previous: duel_hud::initial_update(&session.duel, &session.input),
            last_state: session.input.mode.state(),
            shown_d57: session.input.mode.state(),
            flash: FlashWave::default(),
            hidden: false,
            frame: None,
            layer: vec![0; HUD_SIZE * HUD_SIZE * 4],
            version: 0,
            renders: 0,
            render_time: std::time::Duration::ZERO,
            rendered: None,
            reported: BTreeSet::new(),
            faces: None,
            hand: Vec::new(),
            hand_shown: false,
            picture: None,
            picture_forward: [0.0, -std::f64::consts::FRAC_1_SQRT_2, -std::f64::consts::FRAC_1_SQRT_2],
            picture_version: 0,
            graveyard_slide: 0,
            graveyard_last: None,
            yes_no: roses_client::duel_hud::YesNoTables::from_executable(&data.executable).ok(),
            settings_slide: SETTINGS_HIDDEN,
            settings_last: None,
            hand_view_fade: 0,
            hand_view_last: None,
            detail,
            detail_pictures,
            detail_block: roses_core::card_block::CardBlock::empty(0),
            detail_picture: None,
            multi_pair: None,
            multi_portraits: Vec::new(),
        };
        if let Some(pictures) = &data.pictures {
            hud.attach_pictures(data, pictures.clone())?;
        }
        Ok(hud)
    }

    /// The large card image of a face-up board card (composed once per card
    /// and values), as the card quad samples it: the 256x368 face written
    /// through the 16-bit 0x2000 target (RGB truncated to 5 bits [H: no
    /// dither], alpha 255 where drawn = GS 128); `None` without PICTURE.MRG
    /// or when the composer fails (reported).
    pub fn board_face(&mut self, card: u16, face: duel_overlay::BoardFace) -> Option<&RgbaImage> {
        let faces = self.faces.as_mut()?;
        let key = (card, face);
        if !faces.board.contains_key(&key) {
            let image = match faces.assets.face_with_effect(
                card,
                face.level,
                face.attack,
                face.defence,
                Some(face.effect),
                FaceSize::Large,
            ) {
                Ok(mut image) => {
                    for pixel in image.pixels.chunks_mut(4) {
                        for c in &mut pixel[..3] {
                            *c &= 0xF8;
                        }
                    }
                    Some(image)
                }
                Err(e) => {
                    eprintln!("Duel board: card image {card} not drawn: {e}");
                    None
                }
            };
            faces.board.insert(key, image);
        }
        faces.board[&key].as_ref()
    }

    /// The large face c604 composes for a versus card (0x1DBA80 large mode
    /// into 0x2000, the effect frame from the card table), or the large
    /// card back for [`card_face::BACK_CARD`]; masked to 5 bits like the
    /// board's card images.
    pub fn versus_face(
        &mut self,
        card: u16,
        level: u8,
        attack: i32,
        defence: i32,
    ) -> Option<&RgbaImage> {
        let faces = self.faces.as_mut()?;
        let key = (
            card,
            duel_overlay::BoardFace {
                level,
                attack,
                defence,
                effect: false,
            },
        );
        let key = (key.0 | 0x8000, key.1);
        if !faces.board.contains_key(&key) {
            let image = match faces
                .assets
                .face(card, level, attack, defence, FaceSize::Large)
            {
                Ok(mut image) => {
                    for pixel in image.pixels.chunks_mut(4) {
                        for c in &mut pixel[..3] {
                            *c &= 0xF8;
                        }
                    }
                    Some(image)
                }
                Err(e) => {
                    eprintln!("Duel board: versus card image {card} not drawn: {e}");
                    None
                }
            };
            faces.board.insert(key, image);
        }
        faces.board[&key].as_ref()
    }

    /// Enable the hand row's card faces with the user's PICTURE.MRG (the
    /// duel-end import's); the other files come from the duel import.
    pub fn attach_pictures(&mut self, data: &DuelData, pictures: Vec<u8>) -> Result<()> {
        let assets = FaceAssets::new(
            data.executable.clone(),
            data.messages.clone(),
            &data.texsys,
            pictures,
        )?;
        self.faces = Some(HandFaces {
            assets,
            sized: HashMap::new(),
            board: HashMap::new(),
        });
        Ok(())
    }

    /// Leader ranks used for the HUD (deck records).
    pub fn ranks(&self) -> [i32; 2] {
        self.ranks
    }

    /// Append the details view's pane (`card_detail`) to the frame: the rank
    /// is the HUD's 0x2154C0 stand-in for D5C; the picture is the card's
    /// PICTURE.MRG record (else the pane's blinking NOW LOADING box).
    fn details_pane(&mut self, frame: &mut HudFrame, shown: DetailsShown, slide: u8, flash: u16) {
        let Some((data, cards)) = &self.detail else { return };
        let Some(yes_no) = self.yes_no.as_ref() else { return };
        let card = shown.view.card;
        let texture = self.detail_pictures.as_mut().and_then(|p| p.texture(card, &self.detail_block));
        if self.detail_picture != Some(card) || texture.is_none() != self.textures.picture.is_none() {
            self.detail_picture = Some(card);
            self.textures.picture = texture.clone();
            self.picture_version += 1;
        }
        let pane = crate::card_detail::Pane {
            card,
            x: -0x180,
            y: 0x220 - 8 * i32::from(slide),
            side: 1,
            page: shown.view.page,
            raised_tab: true,
            tinted: true,
            editor_state: None,
            block: &self.detail_block,
            cards,
            picture: texture.is_some(),
            blink: (flash >> 8) as u8,
            rank: Some(first_duel_rank(self.leaders, self.ranks, shown.d5c, card)),
        };
        let (calls, notes) = crate::card_detail::build(data, &self.tables, &pane);
        for n in notes {
            if self.reported.insert(n.clone()) {
                eprintln!("Duel HUD: details view, not drawn: {n}");
            }
        }
        let extra = roses_client::duel_hud::menu_frame(&self.tables, yes_no, &calls);
        frame.prims.extend(extra.prims);
        frame.unsupported.extend(extra.unsupported);
    }

    /// One 25 Hz update after the session frame: the flash wave steps, the
    /// last write-backs take what the states the port skips would have done
    /// (`duel_hud::after_skipped_states`), the HUD state is built from the
    /// session and those write-backs, the routine runs and its write-backs
    /// are kept.
    pub fn update(&mut self, play: &DuelPlay) {
        let session = &play.session;
        // During an activation the HUD shows the duel from before it (the
        // original applies the rule change after the modules) [F].
        let duel = play
            .monster_fx
            .as_ref()
            .filter(|m| {
                use crate::duel_monster_fx::Effect;
                !(matches!(
                    m.effect,
                    Effect::Burn | Effect::Field | Effect::Heart | Effect::Ritual | Effect::Revive | Effect::Crush | Effect::Retype | Effect::Duster | Effect::FakeTrap | Effect::Raigeki | Effect::AntiRaigeki | Effect::Lawbook | Effect::DarkHole
                ) && m.applied())
            })
            .and_then(|m| m.duel.as_ref())
            .or_else(|| play.activation.as_ref().and_then(|a| a.shown_duel()))
            .or(play.summon_duel.as_ref())
            .unwrap_or(&session.duel);
        // N-player (a port addition): the original HUD draws a pair of
        // sides through a 2-side copy of the duel (`duel_multi`).
        #[cfg(feature = "online")]
        let multi_view = play.multi.as_ref().map(|m| {
            let pair = crate::duel_multi::hud_pair(duel, &session.input, m.viewer());
            (pair, crate::duel_multi::project_duel(duel, pair), crate::duel_multi::project_input(&session.input, pair))
        });
        #[cfg(not(feature = "online"))]
        let multi_view: Option<([u8; 2], roses_core::duel::Duel, roses_core::duel::ui::DuelUi)> = None;
        let (duel, hud_input) = match &multi_view {
            Some((_, d, i)) => (d, i),
            None => (duel, &session.input),
        };
        #[cfg(feature = "online")]
        if let (Some((pair, d, _)), Some(m)) = (&multi_view, play.multi.as_ref()) {
            if self.multi_pair != Some(*pair) {
                self.multi_pair = Some(*pair);
                let leaders = pair.map(|s| m.leaders.get(usize::from(s)).copied().unwrap_or(0));
                if let Err(e) = self.textures.set_leaders(&play.data.picmini, leaders) {
                    eprintln!("Duel HUD: N-player portraits {leaders:?}: {e}");
                }
                self.leaders = leaders;
                self.ranks = [0, 1].map(|i| i32::from(d.leader_ranks.get(i).copied().unwrap_or(0)));
            }
            // The compact plates' portraits: every seat's leader picture in
            // slot 2 + side (the same PICMINI art as the LP plates').
            if self.multi_portraits != m.leaders {
                self.multi_portraits = m.leaders.clone();
                if let Err(e) = self.textures.set_extra_portraits(&play.data.picmini, &m.leaders) {
                    eprintln!("Duel HUD: N-player plate portraits {:?}: {e}", m.leaders);
                }
            }
        }
        let flash = self.flash.step();
        let d57 = session.input.mode.state();
        self.previous = duel_hud::after_skipped_states(&self.previous, self.last_state, d57);
        self.last_state = d57;
        let mut ui = duel_hud::ui_from_input(duel, hud_input, &self.previous, flash);
        // The hand row's motion (states 0x40-0x42, 0x45) and the picked
        // cards of 0x46 replace the settled bytes and positions.
        let view = play.hand_view();
        // Leaving the hand row (its close or the summon) drops the hand
        // card's box as leaving 0x43 / 0x46 does.
        if self.hand_shown && view.is_none() && !matches!(d57, 0x43 | 0x46) {
            self.previous.d3f = 0;
            ui = duel_hud::ui_from_input(duel, hud_input, &self.previous, flash);
        }
        self.hand_shown = view.is_some();
        // The hand view (Square on the leader): 0x51 starts d45 at 0 and
        // 0x52 adds 0x10 per update (the same update too) up to 0x80, then
        // 0x53; Circle fades it out through 0x52 with d67 bit 3 set (-0x10
        // per update) [F 0x1D8188..0x1D8254, 0x1D8458].
        // The details view over it (d35 0x53) keeps it shown.
        if ui.d57 == 0x53 || (ui.d57 == 0x21 && ui.d35 == 0x53) {
            self.hand_view_fade = (self.hand_view_fade + 0x10).min(0x80);
            if self.hand_view_fade < 0x80 && ui.d57 == 0x53 {
                ui.d57 = 0x52;
            }
            (ui.d45, ui.d48) = (self.hand_view_fade, self.hand_view_fade);
            self.hand_view_last = Some(ui.clone());
        } else if self.hand_view_fade > 0 {
            self.hand_view_fade = self.hand_view_fade.saturating_sub(0x10);
            match self.hand_view_last.clone() {
                Some(mut last) if self.hand_view_fade > 0 && ui.d57 == 0x18 => {
                    last.d57 = 0x52;
                    (last.d45, last.d48) = (self.hand_view_fade, self.hand_view_fade);
                    last.d67 |= 8;
                    ui = last;
                }
                _ => {
                    self.hand_view_fade = 0;
                    self.hand_view_last = None;
                }
            }
        }
        // The details view: 0x20 while it slides in, 0x21 at rest, 0x22
        // while it slides out (d35 = the state it returns to); the legend
        // has no rows in these states [F 0x1B3D54; emu dd-leader.json].
        let details = play.details_shown.filter(|_| play.details_slide > 0 || session.input.details.is_some());
        if let Some(shown) = details {
            ui.d35 = shown.d35;
            ui.d57 = if session.input.details.is_none() {
                0x22
            } else if play.details_slide < DETAILS_REST {
                0x20
            } else {
                0x21
            };
        }
        // The GRAVEYARD panel slides up 8 per update (0x2C) and back down
        // after the pick or Circle (0x2E) [F GRAVEYARD-PANEL.md; the port's
        // input machine has no slide states, so the HUD animates it].
        match ui.graveyard.as_mut() {
            Some(g) => {
                self.graveyard_slide = (self.graveyard_slide + 8).min(duel_hud::GRAVEYARD_REST);
                g.slide = self.graveyard_slide;
                self.graveyard_last = Some(g.clone());
            }
            None if self.graveyard_slide > 0 => {
                self.graveyard_slide = (self.graveyard_slide - 8).max(0);
                if let Some(mut g) = self.graveyard_last.clone() {
                    g.slide = self.graveyard_slide;
                    ui.graveyard = Some(g);
                }
            }
            None => {}
        }
        if play.summon_flight.is_some() {
            // 0x47-0x50: Cross closed the legend (0x1E57B0) and the info
            // frames stay empty until the cursor [emu].
            ui.d57 = 0x4D;
            ui.unit_a = None;
            ui.unit_b = None;
            // The equip handler's message box: 0x1E5540(0, 0, -0xA0, 0x107
            // "Power increase!") for 0x218, 0x10D "Force!" for 0x219
            // [F DUEL-MESSAGE-BOX.md, EQUIP-MODULES.md 5].
            let text = match play.summon_flight.as_ref().and_then(|f| f.fusion_playing()).and_then(|f| f.equip_running()) {
                Some(0x218) => Some(0x107),
                Some(0x219) => Some(0x10D),
                _ => None,
            };
            ui.message_box = text.map(|text| roses_client::duel_hud::MessageBox { x: 0, y: -0xA0, text });
        }
        if let Some((d45, d47, poses)) = &view {
            let count = duel.sides[usize::from(duel.active & 1)].hand.len();
            let mut positions = vec![None; count];
            for p in poses {
                if let Some(slot) = positions.get_mut(p.index) {
                    *slot = Some(p.position);
                }
            }
            ui.d45 = *d45;
            ui.d48 = *d45;
            ui.d47 = *d47;
            ui.hand_records = true;
            ui.extra.hand_positions = positions;
            if *d45 > 0 {
                ui.d67 |= 4;
            }
        }
        if let Some(text) = play
            .activation
            .as_ref()
            .and_then(|a| a.box_text())
            .or_else(|| play.monster_fx.as_ref().and_then(|m| m.box_text()))
            .or_else(|| play.intro.as_ref().and_then(|i| i.effect_box()))
            .or_else(|| play.destiny_event.map(|_| roses_core::duel::DESTINY_MESSAGE_BOX))
        {
            // 0x1E5540(0, 0, 0x50, text) [F ACTIVATION-PRESENTATION.md 3].
            ui.message_box = Some(roses_client::duel_hud::MessageBox { x: 0, y: 0x50, text });
        }
        if self.hidden || play.scene_hides_hud() {
            ui.d67 &= !1;
        }
        self.shown_d57 = ui.d57;
        #[allow(unused_mut)]
        let mut ctx = play.hud_context();
        #[cfg(feature = "online")]
        if multi_view.is_some() {
            // The pair as the 2-player duel's two pads; Card Display OPEN
            // for a projected side this screen owns (its seat or team-mate,
            // `DuelPlay::multi_own_sides`), HIDE for enemies.
            let pad_ctl = roses_core::duel::PAD_CONTROLLER;
            ctx.controllers = [pad_ctl, pad_ctl + 1];
            ctx.r2_held = [false; 2];
            if let Some((pair, _, _)) = &multi_view {
                let own = play.multi_own_sides().unwrap_or(0);
                // The right side holds every other player's units too: it
                // stays HIDE even when a team-mate's graveyard is shown.
                ctx.option_29 = u32::from(own & (1 << pair[0]) != 0);
            }
        }
        ui.extra.own_side = play.hud_own_side();
        #[cfg(feature = "online")]
        if let Some((pair, _, _)) = &multi_view {
            // The projected left side (0) when this screen owns it.
            ui.extra.own_side = play.multi_own_sides().filter(|own| own & (1 << pair[0]) != 0).map(|_| 0);
        }
        let (leaders, ranks) = (self.leaders, self.ranks);
        let rank = move |side: u8, card: u16| first_duel_rank(leaders, ranks, side, card);
        let mut state = duel_hud::from_duel(
            duel,
            &ui,
            &play.data.catalogue,
            &ctx,
            &rank,
            &self.previous,
        );
        #[cfg(feature = "online")]
        if let Some(names) = &play.seat_names {
            state.names = names.clone().map(|n| Some(n.into_bytes()));
        }
        #[cfg(feature = "online")]
        {
            state.notes = play.online_note().into_iter().flat_map(|n| n.split("  ").map(|l| l.as_bytes().to_vec()).collect::<Vec<_>>()).collect();
        }
        #[cfg(feature = "online")]
        if let (Some((pair, _, _)), Some(m)) = (&multi_view, play.multi.as_ref()) {
            let real = &session.duel;
            state.names = pair.map(|s| Some(m.name(s).into_bytes()));
            // This seat (or its team-mate) on the left LP plate always shows
            // its stars and counts; the enemy on the right shows ?? as the
            // original shows a computer side's against a pad side (0x1B2590:
            // with the right side's controller below 100 the left plate is
            // never hidden and the right one always). A spectator's plates
            // stay as the 2-player duel's.
            if play.multi_own_sides().is_some_and(|own| own & (1 << pair[0]) != 0) {
                state.sides[1].controller = roses_core::duel::PAD_CONTROLLER - 1;
            }
            for (i, &s) in pair.iter().enumerate() {
                state.sides[i].monsters = crate::duel_multi::count_units(real, s, 1..=5);
                state.sides[i].magic_traps = crate::duel_multi::count_units(real, s, 6..=10);
            }
            // While the Exodia event takes over no turn note slides out with
            // the HUD (the outcome is the event's to show).
            let turn = if play.exodia.is_some() { Vec::new() } else { crate::duel_multi::turn_lines(real, m.seat, &m.names) };
            let mut notes: Vec<Vec<u8>> = turn.into_iter().map(String::into_bytes).collect();
            notes.append(&mut state.notes);
            state.notes = notes;
        }
        if let Some(flight) = &play.summon_flight {
            // The summoned unit is placed by the 0x4F commit; until then its
            // cell is empty for the card boxes (info frames empty) [emu].
            let hidden = state.units.iter().position(|u| (u.side, u.slot) == flight.unit);
            for cell in &mut state.board {
                if *cell == hidden {
                    *cell = None;
                }
            }
        }
        let mut frame = duel_hud::frame(&state, &self.tables);
        #[cfg(feature = "online")]
        if let Some((pair, _, _)) = &multi_view {
            // The HUD is drawn unless 0x1AF0A0's D67 bit 1 is set or it has
            // slid fully out (D3E 128): the plates go with it.
            let hud_shown = state.d67 & 2 == 0 && frame.next.d3e < 128;
            self.multi_overlay(play, &mut frame, *pair, hud_shown);
        }
        // L1's settings panel: 0x27 opens overlay slot 0 kind 7 (the OPTION
        // panel, as the map's SYSTEM icon 2) with C9C = 0x200 and 0x28
        // slides it in by 0x40 per update (the same update too); Cross /
        // Square / Circle leave through 0x2A, which slides it out by 0x40
        // per update; 0x1B0120 draws it after the HUD with D54's colour
        // [F 0x1D4F38..0x1D5180].
        let panel = match session.input.settings {
            Some(p) => {
                self.settings_slide = (self.settings_slide - 0x40).max(0);
                Some(roses_client::duel_hud::OptionPanel { side: state.d54, y: self.settings_slide, row: p.row, bits: p.bits })
            }
            None if self.settings_slide < SETTINGS_HIDDEN => {
                self.settings_slide = (self.settings_slide + 0x40).min(SETTINGS_HIDDEN);
                self.settings_last.filter(|_| self.settings_slide < SETTINGS_HIDDEN).map(|p| roses_client::duel_hud::OptionPanel { y: self.settings_slide, ..p })
            }
            None => None,
        };
        self.settings_last = panel;
        if let (Some(panel), Some(yes_no)) = (panel, self.yes_no.as_ref()) {
            let extra = roses_client::duel_hud::option_panel(&self.tables, yes_no, &panel);
            frame.prims.extend(extra.prims);
            frame.unsupported.extend(extra.unsupported);
        }
        // Overlay slot 1 code 0x11: the details view's pane, 0x1BE6E0(card,
        // −0x180, 0x220 − 8·d3a, side 1, raised tab, tinted, not the
        // editor's) after the HUD and slot 0 [F 0x1B1338..0x1B13E4; emu
        // modes/ui3 dd-*.json: the pictures are resident in the duel].
        if let Some(shown) = details {
            self.details_pane(&mut frame, shown, play.details_slide, flash);
        }
        // 0x1CE970 → 0x1AFAC0 draws the hand records' card objects while the
        // hand overlay (0x1AFF00) shows them.
        self.hand.clear();
        // Online, the other seat's hidden hand: only placeholders are known
        // here, so its cards show their back (never a stand-in face).
        let unknown_hand = play.unknown_hand();
        let shown = |face: HandFace| {
            if unknown_hand {
                HandFace { card: card_face::BACK_CARD, level: 0, attack: 0, defence: 0 }
            } else {
                face
            }
        };
        if let (Some((_, _, poses)), true) = (&view, self.faces.is_some()) {
            for p in poses {
                if let Some(face) = state.hand_overlay.get(p.index).and_then(|c| c.face) {
                    self.hand.push(HandCard {
                        position: p.position,
                        face: shown(face),
                        theta: p.theta,
                        large: p.large,
                        dim: play.hand_unaffordable(p.index),
                    });
                }
            }
        } else if self.faces.is_some() && state.overlay_mode == 1 && state.d45 != 0 && state.d47 == 0 {
            for (i, card) in state.hand_overlay.iter().enumerate() {
                if let (Some(position), Some(face)) = (card.position, card.face) {
                    self.hand.push(HandCard {
                        position,
                        face: shown(face),
                        theta: 0.0,
                        large: false,
                        dim: play.hand_unaffordable(i),
                    });
                }
            }
        } else if self.faces.is_none() && state.overlay_mode == 1 && state.d45 != 0 {
            let what = "hand card faces (PICTURE.MRG not attached)".to_string();
            if self.reported.insert(what.clone()) {
                eprintln!("Duel HUD: unsupported, not drawn: {what}");
            }
        }
        self.previous = frame.next;
        // The terrain picture, re-rendered and turned every HUD frame
        // (0x1B4370) [F/emu TERRAIN-PICTURE.md].
        let tile = frame.unsupported.iter().find_map(|u| match u.kind {
            UnsupportedKind::TerrainTileRender { terrain } => Some(terrain),
            _ => None,
        });
        if let (Some(terrain), Some(picture)) = (tile, self.picture.as_mut()) {
            self.textures.render_target = u8::try_from(terrain)
                .ok()
                .and_then(|t| picture.render(t, self.picture_forward, self.tables.dimx));
            if terrain >= 0 {
                picture.advance();
            }
            self.picture_version += 1;
        }
        for u in &frame.unsupported {
            if self.picture.is_some() && matches!(u.kind, UnsupportedKind::TerrainTileRender { .. }) {
                continue;
            }
            let what = match &u.kind {
                UnsupportedKind::TerrainTileRender { .. } => {
                    "terrain tile picture (VU1 render of the tile model into RT 0x2000; its backing is drawn)"
                        .to_string()
                }
                other => format!("{other:?}"),
            };
            if self.reported.insert(what.clone()) {
                eprintln!("Duel HUD: unsupported, not drawn: {what}");
            }
        }
        self.frame = Some(frame);
    }

    /// Render the layer if the last frame's primitives differ from the
    /// rendered ones; returns whether `layer` changed.
    pub fn render(&mut self) -> bool {
        let Some(frame) = &self.frame else {
            return false;
        };
        if self
            .rendered
            .as_ref()
            .is_some_and(|(prims, hand, picture)| *prims == frame.prims && *hand == self.hand && *picture == self.picture_version)
        {
            return false;
        }
        let start = std::time::Instant::now();
        let (mut layer, _skipped) =
            duel_hud::render_layer(frame, &self.tables, &mut self.textures, HUD_SIZE, HUD_SIZE);
        if let Some(faces) = &mut self.faces {
            // The cards are drawn right to left (0x1AFAC0), before the HUD;
            // the first in the list is on top.
            for card in &self.hand {
                let back = card.theta.cos() < 0.0;
                let size = if card.large { FaceSize::Large } else { FaceSize::Small };
                let key = (
                    if back { HandFace { card: card_face::BACK_CARD, level: 0, attack: 0, defence: 0 } } else { card.face },
                    card.large,
                );
                let face = faces.sized.entry(key).or_insert_with(|| {
                    let f = key.0;
                    match faces.assets.face(f.card, f.level, f.attack, f.defence, size) {
                        Ok(image) => Some(image),
                        Err(e) => {
                            eprintln!("Duel HUD: card face {} not drawn: {e}", f.card);
                            None
                        }
                    }
                });
                let Some(face) = face else { continue };
                let pose = crate::duel_hand::HandPose {
                    index: 0,
                    position: card.position,
                    theta: card.theta,
                    large: card.large,
                };
                draw_hand_card_under(&mut layer, HUD_SIZE, HUD_SIZE, face, &pose, card.dim);
            }
        }
        self.render_time += start.elapsed();
        self.renders += 1;
        self.layer = layer;
        self.rendered = Some((frame.prims.clone(), self.hand.clone(), self.picture_version));
        self.version += 1;
        true
    }

    /// Draw the terrain picture from the board import's terrain models.
    pub fn attach_picture(&mut self, board: &crate::duel_board::LoadedBoard) -> Result<()> {
        self.picture = Some(crate::duel_hud_picture::TerrainPicture::new(board)?);
        Ok(())
    }

    /// Unsupported parts reported so far.
    pub fn unsupported(&self) -> Vec<String> {
        self.reported.iter().cloned().collect()
    }

    /// One line of the message font (a port addition's own text, message
    /// ids from 0xF100), squeezed to `limit` pixels.
    #[cfg(feature = "online")]
    fn multi_text(&mut self, calls: &mut Vec<roses_client::duel_hud::MenuCall>, id: &mut u16, text: &str, x: i32, y: i32, limit: i32, dim: bool) {
        let codes = crate::online_screen::text_codes(text);
        let w = 12 * codes.len() as i32;
        self.tables.set_message(*id, codes);
        let sx = if w > limit { limit as f32 / w as f32 } else { 1.0 };
        calls.push(roses_client::duel_hud::MenuCall::Text { line: 1, message: *id, x, y, sx, sy: 1.0, lines: 1, dim });
        *id += 1;
    }

    /// N-player (a port addition): the compact plates of the players
    /// outside the HUD's pair (the LP plate's panel in the team colour,
    /// lit on that player's turn, OUT once knocked out), and the placing
    /// list once the match is over.
    #[cfg(feature = "online")]
    /// `hud_shown`: the 2-player HUD is on screen this frame; the compact
    /// plates and the plate labels are hidden with it (battle approach,
    /// Exodia event), and the knockout note during the Exodia event.
    fn multi_overlay(&mut self, play: &DuelPlay, frame: &mut HudFrame, pair: [u8; 2], hud_shown: bool) {
        use roses_client::duel_hud::MenuCall;
        let (Some(m), Some(yes_no)) = (play.multi.as_ref(), self.yes_no.clone()) else { return };
        let duel = &play.session.duel;
        let me = m.viewer();
        let mut calls = Vec::new();
        let mut id = 0xF100u16;
        let (w, h) = (180, 48);
        // The portrait: the LP plate's PICMINI leader picture (drawn 48 x
        // 48 there, lit on that side's turn), here 32 x 32 inside the
        // plate's panel (its gradient starts 16 / 21 pixels in) under the
        // name, with the same frame pieces (panel type 4); the LP line
        // beside it.
        let pw = crate::duel_multi::PLATE_PORTRAIT;
        let tx = crate::duel_multi::PLATE_TEXT_X;
        let mut used = [0; 2];
        let plates = crate::duel_multi::plates_shown(hud_shown, play.exodia.is_some());
        for p in crate::duel_multi::compact_plates(duel, me, pair, &m.names).into_iter().filter(|_| plates) {
            let column = usize::from(p.place != crate::duel_multi::Place::Left);
            let x = if column == 0 { 6 } else { 512 - 6 - w };
            let y = 108 + used[column] * (h + 4);
            used[column] += 1;
            calls.push(MenuCall::Panel { grad: p.colour, deco: p.colour, ptype: 0, x, y, wu: -w, hu: -h, bright: p.active });
            let (px, py) = (x + 20, y + 25);
            calls.push(MenuCall::Portrait { slot: 2 + p.side, x: px, y: py, w: pw, h: pw, bright: p.active && !p.out });
            calls.push(MenuCall::Panel { grad: -1, deco: -1, ptype: 4, x: px, y: py, wu: -pw, hu: -pw, bright: p.active && !p.out });
            let name = crate::duel_multi::plate_title(p.side, &p.name);
            let text_w = crate::duel_multi::PLATE_TEXT_W;
            self.multi_text(&mut calls, &mut id, &name, x + tx, y + crate::duel_multi::PLATE_NAME_Y, text_w, p.out);
            let status = if p.out {
                "OUT"
            } else if p.active {
                "TURN"
            } else if p.target {
                "TARGET"
            } else {
                ""
            };
            let line = format!("LP {} {status}", p.lp);
            self.multi_text(&mut calls, &mut id, &line, x + tx, y + crate::duel_multi::PLATE_LP_Y, text_w, p.out || !p.active);
        }
        // A knockout just happened: "<NAME> OUT" over the board.
        let note = crate::duel_multi::out_note_shown(duel.outcome.is_some(), play.animating(), play.exodia.is_some(), play.scene_hides_hud());
        if let Some((side, _)) = m.out_note.filter(|_| note) {
            let text = crate::duel_multi::out_note(&m.name(side));
            calls.push(MenuCall::Gold { x: 256, y: 176, size: 2, flags: 3, text, dim: false });
        }
        // A knocked-out player on the LP plates: OUT.
        for (i, &s) in pair.iter().enumerate().filter(|_| plates) {
            if duel.is_out(s) {
                calls.push(MenuCall::Gold { x: if i == 0 { 0x60 } else { 0x1A0 }, y: 0x5E, size: 1, flags: 3, text: b"OUT".to_vec(), dim: false });
            }
        }
        // This seat's target on the right LP plate.
        if let Some(seat) = m.seat.filter(|&s| plates && duel.alive(s) && duel.outcome.is_none()) {
            if crate::duel_multi::target_of(duel, seat) == pair[1] && pair[1] != duel.active {
                calls.push(MenuCall::Gold { x: 0x1A0, y: 0x5E, size: 1, flags: 3, text: b"TARGET".to_vec(), dim: false });
            }
        }
        if duel.outcome.is_some() && !play.animating() {
            let list = crate::duel_multi::placings(duel);
            let (px, py, pw) = (96, 140, 320);
            let ph = 74 + 24 * list.len() as i32 + 26;
            calls.push(MenuCall::Panel { grad: 0, deco: 0, ptype: 0, x: px, y: py, wu: -pw, hu: -ph, bright: true });
            let heading = crate::duel_multi::result_line(duel, m.seat).unwrap_or_else(|| "MATCH OVER".into());
            let gold: Vec<u8> = heading.bytes().map(|b| if b.is_ascii_alphanumeric() { b.to_ascii_uppercase() } else { b' ' }).collect();
            calls.push(MenuCall::Gold { x: 256, y: py + 14, size: 2, flags: 3, text: gold, dim: false });
            let mut y = py + 52;
            for (place, side) in list {
                let mine = Some(side) == m.seat;
                let line = format!("{}  {}", crate::duel_multi::place_word(place), m.name(side));
                self.multi_text(&mut calls, &mut id, &line, px + 24, y, pw - 48, !mine);
                y += 24;
            }
            if m.local.is_none() && m.results >= crate::duel_multi::RESULTS_MIN_UPDATES {
                self.multi_text(&mut calls, &mut id, "Cross: back to the lobby", px + 24, y + 4, pw - 48, true);
            }
        }
        let extra = roses_client::duel_hud::menu_frame(&self.tables, &yes_no, &calls);
        frame.prims.extend(extra.prims);
        frame.unsupported.extend(extra.unsupported);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The duel is moved by value through the scene setup (`DuelPlay::new`
    /// / `online` -> the board scene -> the ECS resource) on the main
    /// thread, whose stack is 1 MiB on Windows: each move is a stack copy
    /// in a dev build. The presentations (monster effects, activations,
    /// battle openings, flights, leader effects, the online link) live on
    /// the heap so the struct stays small; at 49 KB the online duel's setup
    /// overflowed the main thread's stack in the dev build (seat 1 vs the
    /// server's computer, 9 Oct).
    #[test]
    fn the_duel_stays_small_enough_to_move_on_the_main_thread() {
        let size = std::mem::size_of::<DuelPlay>();
        assert!(size <= 8 * 1024, "DuelPlay is {size} bytes: box its large presentation fields");
    }

    // ------------------------------------------------ the analog camera
    const C: u8 = 0x80;

    /// 0x1D3CFC-0x1D3EAC step by step [F].
    #[test]
    fn stick_camera_follows_the_dispatcher() {
        use roses_core::duel::ai::pad as bits;
        let mut cam = SideCamera::default();
        // Centred and inside the dead zone (|v| < 0x30): nothing.
        assert_eq!(stick_camera(&mut cam, 0, [C; 4], 0), None);
        assert_eq!(stick_camera(&mut cam, 0, [0, C + 0x2F, C - 0x2F, C + 0x2F], 0), None);
        assert_eq!(cam, SideCamera::default());
        // Left stick right (index 2 >= 0x30): yaw - 0x1000, an 8-update track.
        assert_eq!(stick_camera(&mut cam, 0, [C, C, C + 0x30, C], 0), Some(8));
        assert_eq!(cam.yaw, 0x3000);
        assert_eq!(SideCamera::for_side(1), SideCamera { distance: 200, yaw: 0xC000, pitch: 0x2000 });
        // Left: + 0x1000; the yaw is wrapped to 0..0xFFFF before the step.
        cam.yaw = 0xF800;
        stick_camera(&mut cam, 0, [C, C, 0, C], 0);
        assert_eq!(cam.yaw, 0x10800);
        stick_camera(&mut cam, 0, [C, C, 0, C], 0);
        assert_eq!(cam.yaw, 0x1800);
        cam.yaw = -0x800;
        stick_camera(&mut cam, 0, [C, C, 0xFF, C], 0);
        assert_eq!(cam.yaw, 0xE800);
        // Left stick down (index 3): pitch - 0x800, clamped to 0x800..0x4000.
        let mut cam = SideCamera::default();
        for want in [0x1800, 0x1000, 0x800, 0x800] {
            assert_eq!(stick_camera(&mut cam, 0, [C, C, C, 0xFF], 0), Some(8));
            assert_eq!(cam.pitch, want);
        }
        for _ in 0..10 {
            stick_camera(&mut cam, 0, [C, C, C, 0], 0);
        }
        assert_eq!(cam.pitch, 0x4000);
        // Right stick Y (index 1): down +0x32, up -0x32, 0x32..0x1F4; the
        // right stick X (index 0) is not used.
        let mut cam = SideCamera::default();
        assert_eq!(stick_camera(&mut cam, 0, [0xFF, 0xFF, C, C], 0), Some(8));
        assert_eq!(cam.distance, 250);
        for _ in 0..20 {
            stick_camera(&mut cam, 0, [C, 0xFF, C, C], 0);
        }
        assert_eq!(cam.distance, 0x1F4);
        for _ in 0..20 {
            stick_camera(&mut cam, 0, [C, 0, C, C], 0);
        }
        assert_eq!(cam.distance, 0x32);
        assert_eq!(stick_camera(&mut cam, 0, [0, C, C, C], 0), None);
        // L3 (new press): yaw 0x4000 (+0x8000 on side 1), pitch 0x2000, a
        // 30-update track; the left stick is not read but the right is.
        let mut cam = SideCamera { distance: 300, yaw: 0x1234, pitch: 0x800 };
        assert_eq!(stick_camera(&mut cam, bits::L3, [C, 0, 0, 0], 0), Some(30));
        assert_eq!(cam, SideCamera { distance: 250, yaw: 0x4000, pitch: 0x2000 });
        stick_camera(&mut cam, bits::L3, [C; 4], 1);
        assert_eq!((cam.yaw, cam.pitch), (0xC000, 0x2000));
        // R3: distance 200, 30 updates; the left stick still turns.
        let mut cam = SideCamera { distance: 400, ..SideCamera::default() };
        assert_eq!(stick_camera(&mut cam, bits::R3, [C, 0, 0xFF, C], 0), Some(30));
        assert_eq!((cam.distance, cam.yaw), (200, 0x3000));
        // The default setting is the port's side-0 board polar.
        assert_eq!(SideCamera::default().polar(), crate::duel_board::default_polar());
    }

    /// A match's duel from the synthetic catalogue, both sides on pad 1
    /// (control 100, as online).
    fn stick_play() -> DuelPlay {
        stick_play_with(0, 0)
    }

    fn stick_play_with(first: u8, player_side: u8) -> DuelPlay {
        let cards = roses_net::synthetic::catalogue();
        let setup = roses_server::MatchSetup {
            seed: 99,
            terrain: roses_net::synthetic::board(),
            leaders: roses_net::synthetic::leaders(),
            deck_sources: roses_net::synthetic::decks(),
            first,
            leader_abilities: [vec![], vec![]],
            custom_rules: None,
            leader_ranks: [0; 2],
            destiny_rows: None,
        };
        let duel = setup.build(&cards).unwrap();
        let executable = vec![0u8; 0x1C_4D80 + 854 * 20 + 0x2000];
        let tables = roses_core::duel::ai::weevil::Tables::new(&cards.fusions, &executable).unwrap();
        let side = |leader| SideDeck { deck_record: 0, leader, cards: Vec::new() };
        let data = DuelData {
            catalogue: cards,
            names: Vec::new(),
            first_duel: FirstDuel {
                board_map: 0,
                terrain: duel.terrain.clone(),
                first,
                sides: [side(0), side(1)],
                computer_sides: Vec::new(),
                planner_tier: 0,
                leader_abilities: [vec![], vec![]],
                opponent: 1,
                leader_ranks: [0; 2],
                player_side,
                custom_rules: None,
                options: None,
            },
            tables,
            executable,
            messages: Vec::new(),
            texsys: Vec::new(),
            picmini: Vec::new(),
            texeff: None,
            pictures: None,
        };
        let session = DuelSession::new(duel, [None, None]);
        let mut play = DuelPlay::with_session(data, session, 0).unwrap();
        for _ in 0..400 {
            if play.session.input.state() == 0x18 && !play.animating() {
                break;
            }
            play.update_pads([0, 0]);
        }
        assert_eq!(play.session.input.state(), 0x18);
        play
    }

    /// A computer-vs-computer 1v1 duel (both seats the ported controller,
    /// no network) from the synthetic catalogue, the match seed `seed`,
    /// played through the game's duel presentation (`update_pads`) until it
    /// ends or `updates` pass; a SHA-256 of the final duel, the update count
    /// and any halt.
    fn offline_duel_hash(seed: u64, updates: usize) -> String {
        use sha2::{Digest, Sha256};
        let cards = roses_net::synthetic::catalogue();
        let setup = roses_server::MatchSetup {
            seed,
            terrain: roses_net::synthetic::board(),
            leaders: roses_net::synthetic::leaders(),
            deck_sources: roses_net::synthetic::decks(),
            first: (seed & 1) as u8,
            leader_abilities: [vec![], vec![]],
            custom_rules: None,
            leader_ranks: [0; 2],
            destiny_rows: None,
        };
        let duel = setup.build(&cards).unwrap();
        let executable = vec![0u8; 0x1C_4D80 + 854 * 20 + 0x2000];
        let tables = roses_core::duel::ai::weevil::Tables::new(&cards.fusions, &executable).unwrap();
        let side = |leader| SideDeck { deck_record: 0, leader, cards: Vec::new() };
        let data = DuelData {
            catalogue: cards,
            names: Vec::new(),
            first_duel: FirstDuel {
                board_map: 0,
                terrain: duel.terrain.clone(),
                first: duel.first,
                sides: [side(0), side(1)],
                computer_sides: vec![0, 1],
                planner_tier: 0,
                leader_abilities: [vec![], vec![]],
                opponent: 1,
                leader_ranks: [0; 2],
                player_side: 0,
                custom_rules: None,
                options: None,
            },
            tables,
            executable,
            messages: Vec::new(),
            texsys: Vec::new(),
            picmini: Vec::new(),
            texeff: None,
            pictures: None,
        };
        let seats = [
            Some(roses_core::duel::session::ComputerSeat::new(&duel, 0, 0)),
            Some(roses_core::duel::session::ComputerSeat::new(&duel, 1, 0)),
        ];
        let session = DuelSession::new(duel, seats);
        let mut play = DuelPlay::with_session(data, session, seed).unwrap();
        let mut n = 0;
        while n < updates && play.session.duel.outcome.is_none() && play.halted.is_none() {
            play.update_pads([0, 0]);
            n += 1;
        }
        eprintln!("seed {seed}: {n} updates, round {}, outcome {:?}, halted {:?}", play.session.duel.round, play.session.duel.outcome, play.halted);
        let mut hash = Sha256::new();
        hash.update(serde_json::to_vec(&play.session.duel).unwrap());
        hash.update(format!("{n} {:?} {}", play.halted, play.frames).as_bytes());
        hash.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }

    /// The game's 1v1 offline duel is the same with and without the
    /// `online` feature (and so in faithful mode): `cargo test --features
    /// renderer` and `--no-default-features --features renderer` give these
    /// same hashes (docs/ADDITIONS.md).
    #[test]
    fn offline_duels_are_the_same_with_and_without_the_online_feature() {
        let got: Vec<(u64, String)> = [1u64, 2, 3, 7].iter().map(|&s| (s, offline_duel_hash(s, 30_000))).collect();
        for (seed, hash) in &got {
            eprintln!("game offline duel seed {seed}: {hash}");
        }
        let want: Vec<(u64, String)> = OFFLINE_DUEL_HASHES.iter().map(|(s, h)| (*s, h.to_string())).collect();
        assert_eq!(got, want);
    }

    /// Recorded once; both builds must give them.
    /// (After 30,000 updates the synthetic decks are 16 to 18 rounds in.)
    const OFFLINE_DUEL_HASHES: [(u64, &str); 4] = [
        (1, "1c28e7aa5ddaa6de0d20c313c76ce2f2dad65c8e48ce3044d995154a6c3c733c"),
        (2, "5dd679634426b3ef60517218220890ebcf08ab5160a9e2af7d80529f61bf69b3"),
        (3, "c35452830085d7a87c3adfdb235fbeca728ccc6de680bb51e30d085f9812a2ad"),
        (7, "a9918230df03c0559b000ed112a558310c4f088461fb466e96e9933c5afe48bb"),
    ];

    /// One renderer update: the pad, then the camera (play_tick's order;
    /// board_tick ticks the camera).
    fn stick_update(play: &mut DuelPlay, camera: &mut crate::duel_board::BoardCamera, held: u16, sticks: Option<[u8; 4]>) {
        play.sticks = [sticks, None];
        play.update_pads([held, 0]);
        let cursor = play.cursor();
        drive_camera(camera, play, cursor, &|_| None);
        camera.tick();
    }

    /// The board cursor under a held or tapped d-pad, as measured in the
    /// EE interpreter from the duel savestate 011-t11 (side 0 human, state
    /// 0x18; main-loop order 0x1D00F0, 0x1FB430, pad read, 0x203180):
    /// held Right moved the cursor on the first update the word was seen
    /// and then every 8 updates (offsets 0, 8, 16), with sound 3 each
    /// step; taps at offsets 0, 2, 4 moved it once, a tap at 9 again and
    /// one at 18 again (presses during the cursor's 8-update camera track
    /// are not read). The port's driver gives the same steps.
    #[test]
    fn a_held_d_pad_steps_the_board_cursor_every_8_updates() {
        use roses_core::duel::ai::pad as bits;
        let run = |words: &[u16]| {
            let mut play = stick_play();
            let start = play.cursor();
            let dir = if start.column <= 2 { bits::RIGHT } else { bits::LEFT };
            for _ in 0..20 {
                play.update_pads([0, 0]);
            }
            play.audio.clear();
            let mut steps = Vec::new();
            let mut last = play.cursor();
            for (u, &w) in words.iter().enumerate() {
                play.update_pads([if w != 0 { dir } else { 0 }, 0]);
                let c = play.cursor();
                if c != last {
                    steps.push(u);
                    last = c;
                }
            }
            let sounds = play
                .audio
                .iter()
                .filter(|a| matches!(a, crate::duel_audio::AudioCue::Se { slot: 0, id: 3, .. }))
                .count();
            (steps, sounds, (start.column as i32 - last.column as i32).abs())
        };
        // Held for 20 updates: steps at 0, 8, 16 (the original's 1, 9, 17
        // counted from the frame before the word is first seen).
        let (steps, sounds, _) = run(&[1; 20]);
        assert_eq!(steps, [0, 8, 16]);
        assert_eq!(sounds, 3);
        // Taps: the ones inside a step's track are not read.
        let mut taps = [0u16; 20];
        for u in [0, 2, 4, 9, 18] {
            taps[u] = 1;
        }
        let (steps, _, moved) = run(&taps);
        assert_eq!((steps, moved), (vec![0, 9, 18], 3));
        // Held to the board's edge: no step, no sound past it.
        let (steps, sounds, _) = run(&[1; 80]);
        assert!(steps.len() <= 6 && steps.windows(2).all(|w| w[1] - w[0] == 8), "{steps:?}");
        assert_eq!(sounds, steps.len());
    }

    /// The turn camera (0x1DAAA0) as measured in the EE interpreter from
    /// 011-t11 with both sides on pads: side 0's Start; three updates later
    /// (state 0x14) 32-update tracks to side 1's cursor cell and setting
    /// (200, 0xC000, 0x2000); side 1's Start: back to side 0's cursor and
    /// (200, 0x4000, 0x2000) over 32. Through the live driver: each turn
    /// is seen from the active side's end, and a stick change of one side
    /// is kept for its next turn.
    #[test]
    fn each_turn_is_seen_from_the_active_sides_end() {
        use roses_core::duel::ai::pad as bits;
        let mut play = stick_play();
        let c = play.cursor();
        let mut camera = crate::duel_board::BoardCamera::snapped((c.column, c.row));
        let yaw16 = |c: &crate::duel_board::BoardCamera| (c.polar[1] * 65536.0 / std::f64::consts::TAU).round() as i32 & 0xFFFF;
        for _ in 0..40 {
            stick_update(&mut play, &mut camera, 0, None);
        }
        assert_eq!((play.session.duel.active, yaw16(&camera)), (0, 0x4000));
        // Side 0 turns its camera with the stick before ending its turn.
        stick_update(&mut play, &mut camera, 0, Some([C, C, 0xFF, C]));
        for _ in 0..10 {
            stick_update(&mut play, &mut camera, 0, None);
        }
        assert_eq!(play.side_cameras[0].yaw, 0x3000);
        let end_turn = |play: &mut DuelPlay, camera: &mut crate::duel_board::BoardCamera, side: u8| {
            for u in 0..2000 {
                stick_update(play, camera, if u % 2 == 0 { bits::START } else { 0 }, None);
                if play.session.duel.active != side {
                    return;
                }
            }
            panic!("side {side}'s turn did not end");
        };
        end_turn(&mut play, &mut camera, 0);
        assert_eq!(play.session.duel.active, 1);
        let track = camera.polar_track.expect("the turn camera");
        assert_eq!(track.frames, TURN_CAMERA_FRAMES);
        assert_eq!(track.end, SideCamera { distance: 200, yaw: 0xC000, pitch: 0x2000 }.polar());
        let c1 = play.cursor();
        assert_eq!(camera.track.map(|t| t.end), Some(crate::duel_board::cursor_target((c1.column, c1.row))));
        for _ in 0..TURN_CAMERA_FRAMES {
            stick_update(&mut play, &mut camera, 0, None);
        }
        assert_eq!(yaw16(&camera), 0xC000);
        // While side 1 plays the camera stays at its end (no pull back to
        // side 0's view).
        for _ in 0..40 {
            stick_update(&mut play, &mut camera, 0, None);
        }
        assert_eq!(yaw16(&camera), 0xC000);
        // Back to side 0: its own (stick-turned) setting.
        end_turn(&mut play, &mut camera, 1);
        for _ in 0..TURN_CAMERA_FRAMES {
            stick_update(&mut play, &mut camera, 0, None);
        }
        assert_eq!(play.session.duel.active, 0);
        assert_eq!(yaw16(&camera), 0x3000);
    }

    /// A White Rose duel (the player on side 1, side 1 first): the duel
    /// init 0x1AE640 sets the camera yaw from p2 = the player's side
    /// (0xC000) before the start's turn camera, so the start never turns
    /// through side 0's view [F 0x1AE89C..0x1AE910].
    #[test]
    fn a_side_1_duel_starts_from_side_1s_end() {
        let mut play = stick_play_with(1, 1);
        assert_eq!(play.session.duel.active, 1);
        let mut camera = play.start_camera();
        let yaw16 = |c: &crate::duel_board::BoardCamera| (c.polar[1] * 65536.0 / std::f64::consts::TAU).round() as i32 & 0xFFFF;
        assert_eq!(yaw16(&camera), 0xC000);
        for _ in 0..40 {
            stick_update(&mut play, &mut camera, 0, None);
            assert_eq!(yaw16(&camera), 0xC000);
        }
        // Side 0's own duel keeps the side-0 start.
        let play0 = stick_play();
        assert_eq!(play0.start_camera().polar, crate::duel_board::default_polar());
    }

    /// Through the live driver: a held stick turns the board camera one
    /// 0x1000 step per 8-update track (the next step waits for the track,
    /// 0x1FB900); the cursor camera keeps the new view; L3 resets it over
    /// 30 updates; a keyboard (no sticks) changes nothing.
    #[test]
    fn the_left_stick_turns_the_duel_camera() {
        use roses_core::duel::ai::pad as bits;
        let mut play = stick_play();
        let mut camera = crate::duel_board::BoardCamera::snapped({
            let c = play.cursor();
            (c.column, c.row)
        });
        let yaw16 = |c: &crate::duel_board::BoardCamera| (c.polar[1] * 65536.0 / std::f64::consts::TAU).round() as i32;
        // The start's 32-update turn camera (0x1DAAA0) first.
        for _ in 0..40 {
            stick_update(&mut play, &mut camera, 0, None);
        }
        assert_eq!(camera.polar, crate::duel_board::default_polar());
        // Left stick held right for 20 updates: steps start at updates 0,
        // 8 and 16 (the 8-update track advances from the update it starts,
        // as the port's other camera tracks [H: the original's track
        // update order relative to the dispatcher]).
        let mut starts = Vec::new();
        for u in 0..20 {
            let before = play.side_cameras[0].yaw;
            stick_update(&mut play, &mut camera, 0, Some([C, C, 0xFF, C]));
            if play.side_cameras[0].yaw != before {
                starts.push(u);
            }
        }
        assert_eq!(starts, [0, 8, 16]);
        assert_eq!(play.side_cameras[0].yaw, 0x4000 - 3 * 0x1000);
        // Released: the track ends at the new view and it stays there.
        for _ in 0..20 {
            stick_update(&mut play, &mut camera, 0, Some([C; 4]));
        }
        assert_eq!(yaw16(&camera), 0x1000);
        assert_eq!(camera.polar, play.board_polar());
        // A cursor step keeps the view.
        stick_update(&mut play, &mut camera, bits::RIGHT, Some([C; 4]));
        for _ in 0..20 {
            stick_update(&mut play, &mut camera, 0, Some([C; 4]));
        }
        assert_eq!(yaw16(&camera), 0x1000);
        // L3: back to the side-0 view over 30 updates.
        stick_update(&mut play, &mut camera, bits::L3, Some([C; 4]));
        assert_eq!(play.side_cameras[0], SideCamera::default());
        assert_eq!(camera.polar_track.map(|t| t.frames), Some(30));
        for _ in 0..30 {
            stick_update(&mut play, &mut camera, 0, None);
        }
        assert_eq!(camera.polar, crate::duel_board::default_polar());
        // The keyboard alone (no analog pad, 0x203500 = 0): no camera.
        for _ in 0..20 {
            stick_update(&mut play, &mut camera, 0, None);
        }
        assert_eq!(play.side_cameras[0], SideCamera::default());
    }

    #[test]
    fn hand_cards_go_under_the_hud_layer() {
        // A 2×2 layer: one opaque HUD pixel, the rest empty.
        let mut layer = vec![0u8; 2 * 2 * 4];
        layer[..4].copy_from_slice(&[10, 20, 30, 255]);
        let face = RgbaImage {
            width: 2,
            height: 2,
            pixels: [200, 100, 50, 255].repeat(4),
        };
        draw_card_under(&mut layer, 2, 2, &face, [0.0, 0.0, 2.0, 2.0]);
        assert_eq!(layer[..4], [10, 20, 30, 255]);
        for pixel in layer[4..].chunks(4) {
            assert_eq!(pixel, [200, 100, 50, 255]);
        }
    }

    #[test]
    fn battle_opening_matches_the_traced_k2_and_footage_values() {
        let g = |column, row| GridPos { column, row };
        // K2: attacker side 0 on (2,6) attacks (1,6): both turn to 0x4000.
        assert_eq!(face_target(0, g(2, 6), g(1, 6)), 0x4000);
        assert_eq!(face_target(1, g(1, 6), g(2, 6)), 0x4000);
        let mut yaw = 0;
        let mut steps = 0;
        while yaw != 0x4000 {
            yaw = turn_step(yaw, 0x4000);
            steps += 1;
        }
        assert_eq!(steps, 8);
        // Battle camera: K2 yaw 3pi/2 target (-75, 40, 150); footage
        // (attacker (3,3), defender (3,4)) yaw pi target (0, 40, 25).
        let k2 = battle_camera(g(2, 6), g(1, 6));
        assert_eq!(k2.target, [-75.0, 40.0, 150.0]);
        assert!((k2.polar[1] - 3.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        assert!((k2.polar[2] - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        let footage = battle_camera(g(3, 3), g(3, 4));
        assert_eq!(footage.target, [0.0, 40.0, 25.0]);
        assert!((footage.polar[1] - std::f64::consts::PI).abs() < 1e-9);
        // The K2 polar track: dist 200 -> 150 and yaw down through 1.374.
        let mut camera = crate::duel_board::BoardCamera::snapped((2, 6));
        camera.track_to(k2.target, k2.polar, k2.frames);
        camera.tick();
        assert!((camera.polar[0] - 196.875).abs() < 1e-9);
        assert!((camera.polar[1] - 1.374).abs() < 1e-3);
    }

    #[test]
    fn walks_slide_five_units_per_dispatch_along_the_path() {
        let g = |column, row| GridPos { column, row };
        let mut walk = Walk {
            unit: roses_core::UnitId(1),
            cells: vec![g(3, 5), g(2, 5), g(2, 4)],
            dispatches: 0,
        };
        assert_eq!(walk.total(), 20);
        // Drawn from the origin: (+50, +50) from the destination (2,4).
        assert_eq!(walk.offset(), [50.0, 50.0]);
        walk.dispatches = 3;
        assert_eq!(walk.offset(), [35.0, 50.0]);
        walk.dispatches = 10;
        assert_eq!(walk.offset(), [0.0, 50.0]);
        walk.dispatches = 14;
        assert_eq!(walk.offset(), [0.0, 30.0]);
        walk.dispatches = 20;
        assert_eq!(walk.offset(), [0.0, 0.0]);
    }

    #[test]
    fn held_arrows_repeat_after_thirteen_frames_and_once_on_release() {
        let mut pad = PadRepeat::default();
        let right = roses_core::duel::ai::pad::RIGHT;
        let pressed: Vec<bool> = (0..16).map(|_| pad.frame(right) != 0).collect();
        // Frame 0 is the press; the counter (older sample pair) is -12 on
        // frames 0 and 1 and reaches 0 on frame 13.
        assert!(pressed[0]);
        assert!(pressed[1..13].iter().all(|p| !p));
        assert!(pressed[13..].iter().all(|p| *p));
        // The release frame still reports a repeat, the next one does not.
        assert_eq!(pad.frame(0), right);
        assert_eq!(pad.frame(0), 0);
        assert_eq!(pad.frame(right), right);
    }

    #[test]
    fn only_the_d_pad_repeats() {
        assert_eq!(REPEAT_MASK, 0xF000);
        for (name, bit) in BUTTONS {
            let mut repeat = PadRepeat::default();
            let presses: Vec<u16> = (0..40).map(|_| repeat.frame(bit | pad::UP)).collect();
            assert_eq!(presses[0], bit | pad::UP, "{name}");
            let repeats = presses[13..].iter().all(|&p| p & bit != 0);
            let once = presses[1..].iter().all(|&p| p & bit == 0);
            assert!(
                if REPEAT_MASK & bit != 0 {
                    repeats
                } else {
                    once
                },
                "{name}"
            );
            // Released and pressed again: a new press.
            repeat.frame(pad::UP);
            assert_eq!(repeat.frame(bit | pad::UP) & bit, bit, "{name}");
        }
    }

    #[test]
    fn keyboard_maps_every_pad_button_once() {
        assert_eq!(key_pad("ArrowUp"), pad::UP);
        assert_eq!(key_pad("KeyX"), pad::CROSS);
        assert_eq!(key_pad("KeyA"), pad::SQUARE);
        assert_eq!(key_pad("KeyZ"), pad::CIRCLE);
        assert_eq!(key_pad("KeyD"), pad::TRIANGLE);
        assert_eq!(key_pad("KeyS"), pad::START);
        assert_eq!(key_pad("Tab"), pad::SELECT);
        assert_eq!(key_pad("KeyQ"), pad::L1);
        assert_eq!(key_pad("KeyE"), pad::R1);
        assert_eq!(key_pad("Digit1"), pad::L2);
        assert_eq!(key_pad("KeyW"), pad::R2);
        assert_eq!(key_pad("KeyP"), 0);
        assert_eq!(key_pad("Numpad6"), 0);
        let all = KEYBOARD.iter().fold(0u16, |acc, &(_, bits)| {
            assert_eq!(bits.count_ones(), 1);
            assert_eq!(acc & bits, 0, "two keys share a button");
            acc | bits
        });
        assert_eq!(all, BUTTONS.iter().fold(0, |acc, &(_, bits)| acc | bits));
        assert_eq!(all.count_ones(), 14);
    }

    #[test]
    fn replay_pad_lists_parse_names_combinations_and_counts() {
        assert_eq!(
            parse_replay_pad("square, up,cross*2,none,up+left,R1").unwrap(),
            vec![
                pad::SQUARE,
                pad::UP,
                pad::CROSS,
                pad::CROSS,
                0,
                pad::UP | pad::LEFT,
                pad::R1
            ]
        );
        assert_eq!(parse_replay_pad("none*3").unwrap(), vec![0; 3]);
        assert!(parse_replay_pad("jump").is_err());
        assert!(parse_replay_pad("cross*0").is_err());
        assert!(parse_replay_pad("cross*x").is_err());
        assert!(parse_replay_pad("none*10001").is_err());
        assert_eq!(button_names(pad::CROSS | pad::UP), "cross+up");
        assert_eq!(button_names(0), "none");
    }

    /// The executors never read fusion tables in these tests (no computer).
    struct NoFusions;
    impl weevil::FusionData for NoFusions {
        fn fuse(&self, _a: u16, _b: u16) -> Option<u16> {
            None
        }
        fn equip(&self, _monster: u16, _card: u16) -> i32 {
            -1
        }
        fn category(&self, _card: u16) -> u8 {
            0
        }
        fn value(&self, _card: u16) -> u16 {
            0
        }
    }

    /// Two human sides with an invented catalogue (not retail data): four
    /// level-1 monsters, leaders 0 and 1, decks of cards 2 and 3.
    fn synthetic_session() -> (DuelSession, Catalogue) {
        use roses_core::duel::{CardInfo, kind, terrain};
        let monster = |id: u16| CardInfo {
            id,
            kind: kind::MACHINE,
            level: 1,
            attribute: 0,
            attack: 100,
            defense: 100,
            effects: [false; 5],
            toon: false,
            kind_index: 0,
            entries: [None; 5],
            equip_bits: [0; 7],
            power_up: None,
        };
        let cards = Catalogue::new((0..4).map(monster).collect(), []);
        let setup = DuelSetup {
            terrain: vec![terrain::NORMAL; 49],
            leaders: [0, 1],
            decks: [vec![2; 10], vec![3; 10]],
            first: 0,
            leader_abilities: [vec![], vec![]],
            controllers: [roses_core::duel::PAD_CONTROLLER; 2],
        };
        let (duel, _) = Duel::new(setup, &cards).unwrap();
        (DuelSession::new(duel, [None, None]), cards)
    }

    /// A real engine battle of (attack, defence) monsters at (3,4) -> (3,3):
    /// the state the effects get, what 0x1F8100 answers for the attacker's
    /// effect (0x101) and the defender's counter (0x001), and whether the
    /// engine destroyed the target.
    fn outcome_of(attacker: (u16, u16), defender: (u16, u16), defence: bool) -> (roses_fx::types::BattleState, i32, i32, bool) {
        use roses_core::duel::{CardInfo, DuelCommand, kind, terrain};
        use roses_core::UnitId;
        let monster = |id: u16, (attack, defense): (u16, u16)| CardInfo {
            id,
            kind: kind::MACHINE,
            level: 1,
            attribute: 0,
            attack,
            defense,
            effects: [false; 5],
            toon: false,
            kind_index: 0,
            entries: [None; 5],
            equip_bits: [0; 7],
            power_up: None,
        };
        let cards = Catalogue::new(vec![monster(0, (100, 100)), monster(1, (100, 100)), monster(2, attacker), monster(3, defender)], []);
        let setup = DuelSetup {
            terrain: vec![terrain::NORMAL; 49],
            leaders: [0, 1],
            decks: [vec![], vec![]],
            first: 0,
            leader_abilities: [vec![], vec![]],
            controllers: [roses_core::duel::PAD_CONTROLLER; 2],
        };
        let (mut duel, _) = Duel::new(setup, &cards).unwrap();
        let mut place = |id, card, side, row, defence, acted| {
            let mut u = duel.units[0].clone();
            (u.id, u.card, u.side, u.slot, u.position) = (UnitId(id), card, side, 1, GridPos { column: 3, row });
            (u.face_up, u.committed_face_up, u.defence, u.committed_defence, u.acted) = (true, true, defence, defence, acted);
            duel.units.push(u);
        };
        place(90, 2, 0, 4, false, false);
        place(91, 3, 1, 3, defence, true);
        let command = DuelCommand::Move { unit: UnitId(90), path: vec![GridPos { column: 3, row: 3 }], face_up: false, defence: None };
        let (after, events) = duel.apply(0, &command, &cards).unwrap();
        let state = effect_battle(&duel, &events, &cards, &Context::default()).expect("a battle");
        (state, state.outcome(0x101, 0x001), state.outcome(0x001, 0x101), after.unit(UnitId(91)).is_none())
    }

    #[test]
    fn a_waiting_duel_says_why_it_stands_still() {
        assert_eq!(online_note_text(false, 0, None), None);
        assert_eq!(online_note_text(false, 2, None).as_deref(), Some("2 WATCHING"));
        assert_eq!(online_note_text(true, 1, None).as_deref(), Some("SPECTATING"));
        assert_eq!(online_note_text(false, 0, Some("OPPONENT AWAY")).as_deref(), Some("OPPONENT AWAY"));
        assert_eq!(online_note_text(false, 1, Some("RECONNECTING")).as_deref(), Some("RECONNECTING  1 WATCHING"));
    }

    #[test]
    fn a_real_battle_gives_the_effects_its_answer() {
        // The target survives: its DEF holds, or it wins.
        let (state, own, _, gone) = outcome_of((1000, 500), (800, 1500), true);
        assert_eq!((own, gone), (0, false));
        assert_eq!(state.cell, (3, 3));
        assert_eq!(state.units[1].map(|u| (u.defence, u.defence_position)), Some((1500, true)));
        let (_, own, counter, gone) = outcome_of((1000, 500), (1800, 900), false);
        assert_eq!((own, counter, gone), (0, 1, false));
        // The target is destroyed.
        let (_, own, _, gone) = outcome_of((2000, 500), (800, 1500), true);
        assert_eq!((own, gone), (1, true));
        let (state, own, counter, gone) = outcome_of((2000, 500), (1800, 900), false);
        assert_eq!((own, counter, gone), (1, 0, true));
        assert_eq!(state.active_side, 1);
        // No battle in the step: nothing for the effects.
        let (session, cards) = synthetic_session();
        assert_eq!(effect_battle(&session.duel, &[], &cards, &Context::default()), None);
    }

    fn press(session: &mut DuelSession, cards: &Catalogue, keys: u16) -> Response {
        session
            .frame(keys, cards, &NoFusions)
            .unwrap()
            .response
            .unwrap()
    }

    #[test]
    fn session_states_map_to_overlay_input() {
        let (mut session, cards) = synthetic_session();
        let input = overlay_input(&session);
        input.validate().unwrap();
        assert_eq!(
            (input.state, input.active_side, input.brightness),
            (0x18, 0, 128)
        );
        assert!(input.marks.is_empty() && input.path.is_empty() && !input.settled);
        let leaders: Vec<_> = input
            .units
            .iter()
            .map(|u| (u.side, u.slot, u.cell, u.face_up, u.acted))
            .collect();
        assert_eq!(
            leaders,
            [(0, 0, [3, 6], true, false), (1, 0, [3, 0], true, true)]
        );

        // Holding Cross on the summon cell opens the hand once: the held
        // face button does not repeat into a card choice or a confirmation.
        {
            let (mut held, cards) = synthetic_session();
            press(&mut held, &cards, pad::SQUARE);
            press(&mut held, &cards, pad::UP);
            let mut repeat = PadRepeat::default();
            assert_eq!(repeat.frame(pad::CROSS), pad::CROSS);
            press(&mut held, &cards, pad::CROSS);
            assert_eq!(held.input.mode.state(), 0x43);
            let hand = held.duel.sides[0].hand.clone();
            assert!(!hand.is_empty());
            for _ in 1..40 {
                let presses = repeat.frame(pad::CROSS);
                assert_eq!(presses, 0);
                held.frame(presses, &cards, &NoFusions).unwrap();
            }
            assert_eq!(held.input.mode.state(), 0x43);
            assert_eq!(held.duel.sides[0].hand, hand);
            assert_eq!(held.duel.units.len(), 2);
        }

        // Summon a face-down unit on (3,5): Square at the leader, Up, Cross
        // opens the hand, Cross picks the first card, Cross confirms.
        assert_eq!(press(&mut session, &cards, pad::SQUARE), Response::Moved);
        assert_eq!(overlay_input(&session).state, 0x3E);
        assert!(overlay_input(&session).marks.is_empty());
        press(&mut session, &cards, pad::UP);
        press(&mut session, &cards, pad::CROSS);
        assert_eq!(overlay_input(&session).state, 0x43);
        press(&mut session, &cards, pad::CROSS);
        assert_eq!(overlay_input(&session).state, 0x46);
        press(&mut session, &cards, pad::CROSS);
        let input = overlay_input(&session);
        input.validate().unwrap();
        assert_eq!(input.state, 0x18);
        let summoned = input.units.iter().find(|u| u.slot != 0).unwrap();
        assert_eq!(
            (
                summoned.side,
                summoned.cell,
                summoned.card,
                summoned.face_up
            ),
            (0, [3, 5], 2, false)
        );

        // Select the leader and step left: marks, allowance and path follow
        // the selection, origin first.
        press(&mut session, &cards, pad::DOWN);
        assert_eq!(press(&mut session, &cards, pad::CROSS), Response::Moved);
        let Mode::Move(sel) = &session.input.mode else {
            panic!("not moving")
        };
        let allowance = sel.allowance;
        let input = overlay_input(&session);
        input.validate().unwrap();
        assert_eq!((input.state, input.move_allowance), (0x1D, allowance));
        assert_eq!(input.marks.len(), 49);
        assert_eq!(input.marks[6 * 7 + 3], 0);
        assert!(input.path.is_empty());
        assert_eq!(press(&mut session, &cards, pad::LEFT), Response::Moved);
        let input = overlay_input(&session);
        assert_eq!(input.path, vec![[3, 6]]);
        assert_eq!(input.marks[6 * 7 + 3], 0);
        // Circle returns the cursor to the origin and leaves the move state.
        press(&mut session, &cards, pad::CIRCLE);
        let input = overlay_input(&session);
        assert_eq!(input.state, 0x18);
        assert!(input.marks.is_empty() && input.path.is_empty());
        assert_eq!(session.input.cursors[0], GridPos { column: 3, row: 6 });

        // Ending the turn hands the board to side 1.
        press(&mut session, &cards, pad::START);
        let input = overlay_input(&session);
        input.validate().unwrap();
        assert_eq!(input.active_side, 1);
        assert!(input.units.iter().all(|u| u.acted == (u.side == 0)));
    }

    #[test]
    fn the_board_camera_walks_to_the_cursor_one_step_per_update() {
        let mut camera = BoardCamera::snapped((3, 6));
        assert!(!follow_cursor(&mut camera, GridPos { column: 3, row: 6 }));
        assert!(camera.track.is_none());
        // One press: exactly one original step with its 8-update track.
        assert!(follow_cursor(&mut camera, GridPos { column: 3, row: 5 }));
        assert_eq!(camera.cursor, (3, 5));
        assert_eq!(
            camera.track.map(|t| t.frames),
            Some(crate::duel_board::CURSOR_TRACK_FRAMES)
        );
        // A jump is walked, diagonals first, one step per update.
        let target = GridPos { column: 5, row: 1 };
        let mut cells = Vec::new();
        while follow_cursor(&mut camera, target) {
            camera.tick();
            cells.push(camera.cursor);
        }
        assert_eq!(cells, [(4, 4), (5, 3), (5, 2), (5, 1)]);
    }

    #[test]
    #[ignore = "Requires a private `roses import-duel` folder (ROSES_DUEL_IMPORT)"]
    fn private_computer_duel_gives_valid_overlay_input_every_update() {
        let dir = std::path::PathBuf::from(std::env::var_os("ROSES_DUEL_IMPORT").unwrap());
        for seed in [DEFAULT_SEED, 1, 2] {
            let mut play = DuelPlay::new(load(&dir).unwrap(), seed, true).unwrap();
            let start = play.cursor();
            let mut camera = BoardCamera::snapped((start.column, start.row));
            let (mut commands, mut sides) = (0, BTreeSet::new());
            let mut states = BTreeSet::new();
            for _ in 0..40_000 {
                let frame = play.update(0);
                if frame.is_some_and(|f| matches!(f.response, Some(Response::Commands(_)))) {
                    commands += 1;
                }
                follow_cursor(&mut camera, play.cursor());
                camera.tick();
                let input = play.overlay_input();
                input
                    .validate()
                    .unwrap_or_else(|e| panic!("seed {seed} update {}: {e}", play.frames));
                if play.intro.is_some() || play.activation.is_some() || play.monster_fx.is_some() || play.deferred_activation.is_some() {
                    // The battle scene and activations (and the walk that
                    // springs a trap) show the board from before the step.
                    continue;
                }
                assert_eq!(input.state, play.session.input.mode.state());
                // A summoned card is the flying card until it lands.
                let flying = play.summon_flight.as_ref().is_some_and(|f| f.flying());
                assert_eq!(input.units.len() + usize::from(flying), play.session.duel.units.len());
                assert_eq!(input.marks.is_empty(), input.state != 0x1D);
                states.insert(input.state);
                sides.insert(input.active_side);
                if play.halted.is_some() || play.session.duel.outcome.is_some() {
                    break;
                }
            }
            let title = play.title();
            eprintln!(
                "seed {seed}: {} updates, {commands} commands; {title}",
                play.frames
            );
            // The duel must reach its end: no session error, an outcome,
            // and the title saying so.
            assert_eq!(play.halted, None, "seed {seed}: {title}");
            assert!(
                play.session.duel.outcome.is_some(),
                "seed {seed}: no outcome after {} updates: {title}",
                play.frames
            );
            assert!(title.contains("DUEL OVER"), "seed {seed}: {title}");
            assert!(commands > 10 && sides.len() == 2, "seed {seed}: {title}");
            assert!(states.contains(&0x1D) && states.contains(&0x43));
            assert!(title.contains("round"));
        }
    }

    #[test]
    #[ignore = "Requires a private `roses import-duel` folder (ROSES_DUEL_IMPORT)"]
    fn private_import_starts_a_session_that_plays_weevils_turn() {
        let dir = std::path::PathBuf::from(std::env::var_os("ROSES_DUEL_IMPORT").unwrap());
        let data = load(&dir).unwrap();
        let mut session = start(&data, 5, false).unwrap();
        // The human ends the first turn; Weevil then plays his.
        tick(&mut session, &data, roses_core::duel::ai::pad::START).unwrap();
        for _ in 0..20_000 {
            if session.duel.active == 0 {
                break;
            }
            tick(&mut session, &data, 0).unwrap();
        }
        assert_eq!(session.duel.active, 0);
        assert!(
            session
                .duel
                .units
                .iter()
                .any(|u| u.side == 1 && u.slot != 0)
        );
    }

    #[test]
    fn hud_context_and_ranks_follow_the_seats_and_deck_records() {
        let (mut session, _) = synthetic_session();
        let ctx = hud_context(&session, pad::R2);
        assert_eq!((ctx.option_28, ctx.option_29), (3, 3));
        assert_eq!(ctx.controllers, [100, 100]);
        assert_eq!(ctx.r2_held, [true, false]);
        session.seats[1] = Some(ComputerSeat::new(&session.duel, 1, 0));
        assert_eq!(hud_context(&session, 0).controllers, [100, 2]);
        session.seats[0] = Some(ComputerSeat::new(&session.duel, 0, 0));
        assert_eq!(hud_context(&session, pad::R2).r2_held, [false, false]);
        let rank = |side, card| first_duel_rank([392, 397], [1, 2], side, card);
        assert_eq!((rank(0, 392), rank(1, 397), rank(0, 397)), (1, 2, 0));
        assert_eq!((rank(0, 671), rank(1, 683), rank(2, 392)), (-1, -1, -1));
    }

    fn private_import() -> Option<std::path::PathBuf> {
        std::env::var_os("ROSES_DUEL_IMPORT").map(std::path::PathBuf::from)
    }

    #[test]
    fn two_pad_sides_read_their_own_r2_and_the_option_words() {
        let (mut session, _) = synthetic_session();
        session.duel.controllers = vec![100, 101];
        let ctx = hud_context_with(&session, Some([1, 2]), [0, pad::R2]);
        assert_eq!(ctx.controllers, [100, 101]);
        assert_eq!(ctx.r2_held, [false, true]);
        assert_eq!((ctx.option_28, ctx.option_29), (1, 2));
        // One pad side: pad 1's R2 on that side only, the words default 3.
        session.duel.controllers = vec![100, 2];
        session.seats[1] = Some(ComputerSeat::new(&session.duel, 1, 0));
        let ctx = hud_context_with(&session, None, [pad::R2, pad::R2]);
        assert_eq!(ctx.r2_held, [true, false]);
        assert_eq!((ctx.option_28, ctx.option_29), (3, 3));
    }

    /// The duel's opening before the player's first turn (Weevil's duel,
    /// the player moves first): the camera starts straight down from 50
    /// over the board's centre, the session waits 191 updates (no pad is
    /// read), the board camera follows the opening's requests, then the
    /// first turn's camera starts as before [PCSX2 modes/older/intro].
    #[test]
    #[ignore = "Requires a private `roses import-duel` folder (ROSES_DUEL_IMPORT)"]
    fn private_the_opening_plays_before_the_first_turn() {
        let Some(dir) = private_import() else { return };
        let mut play = DuelPlay::new(load(&dir).unwrap(), DEFAULT_SEED, false).unwrap();
        play.begin_opening();
        assert!(play.opening.is_some() && play.animating() && play.scene_hides_hud());
        let mut camera = play.start_camera();
        assert_eq!(camera.target, [0.0; 3]);
        assert_eq!(camera.polar[0], 50.0);
        assert_eq!(play.shown_heights(), [-128; 49]);
        let state = play.session.input.mode.state();
        let mut updates = 0;
        let mut distances = Vec::new();
        while play.opening.is_some() {
            assert!(play.update_pads([pad::CROSS, 0]).is_none());
            let cursor = play.cursor();
            drive_camera(&mut camera, &mut play, cursor, &|_| None);
            camera.tick();
            distances.push(camera.polar[0]);
            updates += 1;
            assert!(updates < 400);
        }
        assert_eq!(updates, 191);
        assert_eq!(play.session.input.mode.state(), state, "the session waited");
        assert_eq!((distances[0], distances[27], distances[73]), (70.0, 610.0, 150.0));
        assert!((distances[119] - 100.0).abs() < 1e-9);
        assert_eq!(camera.target, [0.0, 40.0, 150.0]);
        assert_eq!(play.shown_heights(), [0; 49]);
        // The first turn's camera starts on the next update.
        let cursor = play.cursor();
        drive_camera(&mut camera, &mut play, cursor, &|_| None);
        assert!(camera.moving());
    }

    /// Custom Duel's two players: side 1 (control 101) reads pad 2 only
    /// (0x1CFF80), side 0 pad 1 only.
    #[test]
    #[ignore = "Requires a private `roses import-duel` folder (ROSES_DUEL_IMPORT)"]
    fn private_two_humans_each_read_their_own_pad() {
        let Some(dir) = private_import() else { return };
        let mut data = load(&dir).unwrap();
        data.first_duel.computer_sides.clear();
        data.first_duel.opponent = 101;
        data.first_duel.options = Some([3, 1]);
        let mut play = DuelPlay::new(data, DEFAULT_SEED, false).unwrap();
        assert!(play.two_humans());
        assert_eq!((play.pad_port(0), play.pad_port(1)), (Some(0), Some(1)));
        let turn = |play: &mut DuelPlay, pads: [u16; 2]| {
            for _ in 0..400 {
                play.update_pads(pads);
                play.update_pads([0, 0]);
            }
            play.session.duel.active
        };
        assert_eq!(play.session.duel.active, 0);
        // Pad 2's Start does nothing on side 0's turn.
        assert_eq!(turn(&mut play, [0, pad::START]), 0);
        assert_eq!(turn(&mut play, [pad::START, 0]), 1);
        assert_eq!(turn(&mut play, [pad::START, 0]), 1);
        assert_eq!(turn(&mut play, [0, pad::START]), 0);
        // Side 1's card display is HIDE: its hand shading needs R2 on pad 2.
        assert_eq!(play.hud_context().option_29, 1);
    }

    fn opaque(layer: &[u8], x: usize, y: usize) -> bool {
        layer[(y * HUD_SIZE + x) * 4 + 3] == 255
    }

    #[test]
    #[ignore = "Requires a private `roses import-duel` folder (ROSES_DUEL_IMPORT)"]
    fn private_hud_follows_the_player_through_summon_and_move() {
        let dir = private_import().expect("ROSES_DUEL_IMPORT");
        let mut play = DuelPlay::new(load(&dir).unwrap(), DEFAULT_SEED, false).unwrap();
        let mut hud = DuelHud::new(&play).unwrap();
        assert_eq!(hud.ranks(), [1, 2]);
        let run = |play: &mut DuelPlay, hud: &mut DuelHud, keys: &[u16]| {
            for &k in keys {
                play.press(k);
                hud.update(play);
            }
            hud.render();
        };
        run(&mut play, &mut hud, &[0; 8]);
        let frame = hud.frame.clone().unwrap();
        let elements: BTreeSet<String> = frame
            .prims
            .iter()
            .map(|p| format!("{:?}", p.element))
            .collect();
        assert!(elements.contains("LpPanel(0)") && elements.contains("CardBox"));
        assert!(elements.contains("LegendBox") && elements.contains("RoundNumber"));
        assert!(opaque(&hud.layer, 100, 50), "LP panel gradient");
        assert!(!opaque(&hud.layer, 256, 256), "board centre");
        // Square on the leader, Up, Cross: the hand menu with its boxes.
        // The row opens over 8 updates, then new cards fly in 3 each
        // (`duel_hand`); presses are not read meanwhile.
        run(&mut play, &mut hud, &[pad::SQUARE, pad::UP, pad::CROSS]);
        run(&mut play, &mut hud, &[0; 32]);
        assert_eq!(play.session.input.mode.state(), 0x43);
        let frame = hud.frame.clone().unwrap();
        for element in ["HandBoxLeft", "HandBoxRight", "HandOverlay"] {
            assert!(
                frame
                    .prims
                    .iter()
                    .any(|p| format!("{:?}", p.element) == element),
                "{element}"
            );
        }
        assert_eq!(hud.previous.d3f, 128, "the hand card's box is up");
        // Circle back to the summon cell: the original's closing state 0x40
        // has slid the hand card's box away before 0x3E draws again.
        run(&mut play, &mut hud, &[pad::CIRCLE]);
        assert_eq!(play.session.input.mode.state(), 0x3E);
        run(&mut play, &mut hud, &[0; 8]);
        assert_eq!(hud.previous.d3f, 0, "no upper box after the hand menu");
        run(&mut play, &mut hud, &[pad::CROSS]);
        run(&mut play, &mut hud, &[0; 16]);
        assert_eq!(play.session.input.mode.state(), 0x43);
        // Summon the first card, then end the turn: the HUD slides out,
        // latches Weevil's turn and comes back.
        run(&mut play, &mut hud, &[pad::CROSS]);
        run(&mut play, &mut hud, &[0; 15]);
        run(&mut play, &mut hud, &[pad::CROSS]);
        assert_eq!(play.session.input.mode.state(), 0x18);
        assert_eq!(hud.previous.d3f, 0, "no upper box after the summon");
        // The card flies to its cell (32 updates, the pad unread).
        run(&mut play, &mut hud, &[0; 32]);
        run(&mut play, &mut hud, &[pad::START]);
        let mut latched = None;
        for i in 0..40 {
            play.update(0);
            hud.update(&play);
            if latched.is_none() && hud.previous.d3c == 1 {
                latched = Some(i);
            }
        }
        assert!(latched.is_some_and(|i| i <= 16), "{latched:?}");
        assert_eq!(hud.previous.d3e, 0);
        let unsupported = hud.unsupported();
        assert!(
            unsupported
                .iter()
                .all(|u| u.starts_with("terrain tile picture")
                    || u.starts_with("hand card faces (PICTURE.MRG")),
            "{unsupported:?}"
        );
    }

    /// Triangle on the leader, Down, Circle: the states and d3a the
    /// interpreter logged on 765-f3-after (modes/ui3 dd-leader.json: the
    /// Triangle update 0x20 / 0, then 8..56, 0x21 / 64 from the 8th; Circle
    /// 0x22 / 56 in its own update, then 48..8, then the return state with
    /// slot 1 cleared), the pane at (−0x180, 0x220 − 8·d3a) side 1, the
    /// legend without rows, presses unread while it slides.
    #[test]
    #[ignore = "Requires a private `roses import-duel` folder (ROSES_DUEL_IMPORT)"]
    fn private_details_view_slides_as_the_original() {
        let dir = private_import().expect("ROSES_DUEL_IMPORT");
        let mut play = DuelPlay::new(load(&dir).unwrap(), DEFAULT_SEED, false).unwrap();
        let mut hud = DuelHud::new(&play).unwrap();
        let run = |play: &mut DuelPlay, hud: &mut DuelHud, k: u16| {
            play.press(k);
            hud.update(play);
            let legend = hud.frame.as_ref().unwrap().prims.iter().filter(|p| p.element == duel_hud::Element::LegendBox).count();
            let pane = hud.frame.as_ref().unwrap().prims.iter().filter(|p| p.element == duel_hud::Element::Banner).count();
            (hud.shown_d57, play.details_slide, legend, pane)
        };
        for _ in 0..8 {
            run(&mut play, &mut hud, 0);
        }
        let (d57, _, legend_field, _) = run(&mut play, &mut hud, 0);
        assert_eq!(d57, 0x18);
        let active = play.session.duel.active;
        let card = play.session.duel.units.iter().find(|u| u.side == active && u.slot == 0).unwrap().card;
        let mut seen = vec![run(&mut play, &mut hud, pad::TRIANGLE)];
        // Circle while it slides in is not read.
        seen.push(run(&mut play, &mut hud, pad::CIRCLE));
        for _ in 0..7 {
            seen.push(run(&mut play, &mut hud, 0));
        }
        let states: Vec<(u8, u8)> = seen.iter().map(|s| (s.0, s.1)).collect();
        assert_eq!(states, [(0x20, 0), (0x20, 8), (0x20, 16), (0x20, 24), (0x20, 32), (0x20, 40), (0x20, 48), (0x20, 56), (0x21, 64)]);
        assert!(seen.iter().all(|s| s.2 < legend_field && s.3 > 0), "{seen:?} {legend_field}");
        let shown = play.details_shown.unwrap();
        assert_eq!((shown.view.card, shown.view.page, shown.d35, shown.d5c), (card, 0, 0x18, play.session.duel.active));
        run(&mut play, &mut hud, pad::DOWN);
        assert_eq!(play.details_shown.unwrap().view.page, 1);
        let mut out = vec![run(&mut play, &mut hud, pad::CIRCLE)];
        for _ in 0..7 {
            out.push(run(&mut play, &mut hud, 0));
        }
        let states: Vec<(u8, u8)> = out.iter().map(|s| (s.0, s.1)).collect();
        assert_eq!(states, [(0x22, 56), (0x22, 48), (0x22, 40), (0x22, 32), (0x22, 24), (0x22, 16), (0x22, 8), (0x18, 0)]);
        assert_eq!(out[7].2, legend_field);
        assert_eq!(out[7].3, 0, "slot 1 cleared");
        assert!(out[6].3 > 0);
    }

    #[test]
    #[ignore = "Requires private imports (ROSES_DUEL_IMPORT, ROSES_AUDIO_MANIFEST, ROSES_AUDIO_WAV)"]
    fn private_computer_duel_sound_track() {
        // A computer duel's sound requests mixed headless into a WAV for
        // listening (the first `updates` updates).
        use crate::duel_audio::{AudioCue, DuelAudio};
        let dir = std::path::PathBuf::from(std::env::var_os("ROSES_DUEL_IMPORT").unwrap());
        let mut audio = DuelAudio::load(std::path::Path::new(&std::env::var("ROSES_AUDIO_MANIFEST").unwrap())).unwrap();
        let updates: usize = std::env::var("ROSES_AUDIO_UPDATES").ok().and_then(|v| v.parse().ok()).unwrap_or(6000);
        let mut play = DuelPlay::new(load(&dir).unwrap(), 1, true).unwrap();
        let mut mixer = roses_client::sound_mix::Mixer::new();
        let mut out = Vec::new();
        let mut counts = std::collections::BTreeMap::new();
        for _ in 0..updates {
            play.update(0);
            for cue in std::mem::take(&mut play.audio) {
                *counts.entry(format!("{cue:?}").split_whitespace().take(5).collect::<String>()).or_insert(0) += 1;
                match cue {
                    AudioCue::Music { track, keep } => {
                        if let Some(p) = audio.track(track) {
                            mixer.play_music(p, keep);
                        }
                    }
                    AudioCue::FadeOut { speed } => mixer.fade_out_music(speed),
                    AudioCue::Se { slot, id, skip } => {
                        if let Some(se) = audio.se(slot, id) {
                            mixer.play_se(slot, id, &se, skip);
                        }
                    }
                    AudioCue::StopSe { slot, id } => mixer.stop_se(slot, Some(id)),
                    AudioCue::Group { slot, card } => {
                        mixer.stop_se(slot, None);
                        audio.load_group(slot, card);
                    }
                }
            }
            let mut buf = vec![0i16; 960 * 2];
            mixer.mix(&mut buf);
            out.extend(buf);
            if play.session.duel.outcome.is_some() && play.intro.is_none() {
                break;
            }
        }
        for (k, v) in &counts {
            eprintln!("{v:5} {k}");
        }
        let mut wav = Vec::new();
        let data = (out.len() * 2) as u32;
        wav.extend(b"RIFF");
        wav.extend((36 + data).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(48_000u32.to_le_bytes());
        wav.extend((48_000u32 * 4).to_le_bytes());
        wav.extend(4u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend(data.to_le_bytes());
        for v in out {
            wav.extend(v.to_le_bytes());
        }
        std::fs::write(std::env::var("ROSES_AUDIO_WAV").unwrap(), wav).unwrap();
        assert!(counts.keys().any(|k| k.contains("Music")));
    }

    #[test]
    #[ignore = "Requires a private `roses import-duel` folder (ROSES_DUEL_IMPORT)"]
    fn private_computer_duel_plays_a_c600_fusion() {
        // A computer duel (seed ROSES_FUSION_SEED, default 5) reaches a
        // successful fusion; prints its first update for captures.
        let dir = private_import().expect("ROSES_DUEL_IMPORT");
        let seed = std::env::var("ROSES_FUSION_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(5);
        let mut play = DuelPlay::new(load(&dir).unwrap(), seed, true).unwrap();
        for u in 1..=40_000u32 {
            play.update(0);
            if let Some(show) = play.summon_flight.as_ref().and_then(|f| f.fusion_playing()) {
                if !show.effects().is_empty() {
                    eprintln!("seed {seed}: C600 fusion drawing at update {u} ({} triangles)", show.effects().len());
                    return;
                }
            }
            if play.session.duel.outcome.is_some() && play.intro.is_none() {
                break;
            }
        }
        panic!("seed {seed}: no successful fusion");
    }

    #[test]
    #[ignore = "Requires a private `roses import-duel` folder (ROSES_DUEL_IMPORT)"]
    fn private_player_hand_chain_reaches_an_equip_pair() {
        // The player's first turn: every ordered pair of hand cards marked
        // and summoned until one is an equip pair; prints the replay.
        let dir = private_import().expect("ROSES_DUEL_IMPORT");
        for seed in 1..=40u64 {
            for i in 0..5usize {
                for j in 0..5usize {
                    if i == j {
                        continue;
                    }
                    let mut play = DuelPlay::new(load(&dir).unwrap(), seed, false).unwrap();
                    let mut keys = vec![0u16; 8];
                    keys.extend([pad::SQUARE, pad::UP, pad::CROSS]);
                    keys.extend([0; 32]);
                    keys.extend(std::iter::repeat_n(pad::RIGHT, i));
                    keys.push(pad::UP);
                    let (step, dir) = if j > i { (j - i, pad::RIGHT) } else { (i - j, pad::LEFT) };
                    keys.extend(std::iter::repeat_n(dir, step));
                    keys.extend([pad::UP, pad::CROSS]);
                    keys.extend([0; 20]);
                    keys.push(pad::CROSS);
                    keys.extend([0; 3]);
                    for (n, &k) in keys.iter().enumerate() {
                        play.press(k);
                        if let Some(show) = play.summon_flight.as_ref().and_then(|f| f.fusion_playing()) {
                            if show.pair_kinds().any(|k| matches!(k, crate::duel_summon::Pair::Equip { .. })) {
                                eprintln!("seed {seed} picks {i} then {j}: equip at press {n}; keys {keys:?}");
                                return;
                            }
                        }
                    }
                }
            }
        }
        panic!("no equip pair");
    }

    #[test]
    #[ignore = "Requires a private `roses import-duel` folder (ROSES_DUEL_IMPORT)"]
    fn private_computer_duels_present_activations() {
        // Computer duels (seeds 1..=ROSES_ACTIVATION_SEEDS, default 10): the
        // updates where a trap or magic presentation starts, for captures.
        let dir = private_import().expect("ROSES_DUEL_IMPORT");
        let seeds: u64 = std::env::var("ROSES_ACTIVATION_SEEDS").ok().and_then(|v| v.parse().ok()).unwrap_or(10);
        let mut found = 0;
        for seed in 1..=seeds {
            let mut play = DuelPlay::new(load(&dir).unwrap(), seed, true).unwrap();
            let mut was = false;
            for u in 1..=30_000u32 {
                play.update(0);
                if let Some(m) = &play.monster_fx {
                    if m.t == 1 {
                        eprintln!("seed {seed} update {u}: monster {:?} revealed {:?} targets {:?} battle deferred {}", m.effect, m.revealed, m.targets.len(), play.deferred_battle.is_some());
                        found += 1;
                    }
                }
                let now = play.activation.is_some();
                if now && !was {
                    let a = play.activation.as_ref().unwrap();
                    eprintln!("seed {seed} update {u}: {:?} card {} unit {:?} victim {:?}", a.kind, a.card, a.unit, a.victim);
                    if std::env::var_os("ROSES_ACTIVATION_DUMP").is_some() {
                        eprintln!("  cell {:?} frozen unit {:?}", a.cell, a.frozen.units.iter().find(|x| (x.side, x.slot) == a.unit));
                        let shown = play.overlay_input();
                        eprintln!("  shown {:?}", shown.units.iter().find(|x| (x.side, x.slot) == a.unit));
                    }
                    found += 1;
                }
                was = now;
                if play.session.duel.outcome.is_some() && play.intro.is_none() && play.activation.is_none() && play.monster_fx.is_none() {
                    break;
                }
            }
        }
        assert!(found > 0);
    }

    #[test]
    #[ignore = "Requires a private `roses import-duel` folder (ROSES_DUEL_IMPORT)"]
    fn private_player_field_magic_changes_the_terrain() {
        // The player sets a field magic card (689 / 691) from the opening
        // hand, ends the turn, and on the next turn flips it in place:
        // the activation's field presentation runs; prints the replay.
        let dir = private_import().expect("ROSES_DUEL_IMPORT");
        let name = |k: u16| BUTTONS.iter().find(|b| b.1 == k).map_or("none", |b| b.0);
        for seed in 1..=60u64 {
            let mut play = DuelPlay::new(load(&dir).unwrap(), seed, false).unwrap();
            let mut keys: Vec<u16> = Vec::new();
            let send = |play: &mut DuelPlay, k: u16, keys: &mut Vec<u16>| {
                play.press(k);
                keys.push(k);
            };
            for _ in 0..8 {
                send(&mut play, 0, &mut keys);
            }
            for &k in [pad::SQUARE, pad::UP, pad::CROSS].iter().chain(&[0; 32]) {
                send(&mut play, k, &mut keys);
            }
            let Some(i) = play.session.duel.sides[0].hand.iter().position(|&c| c == 689 || c == 691) else {
                continue;
            };
            for _ in 0..i {
                send(&mut play, pad::RIGHT, &mut keys);
                send(&mut play, 0, &mut keys);
            }
            for &k in [pad::CROSS].iter().chain(&[0; 20]).chain(&[pad::CROSS]).chain(&[0; 40]) {
                send(&mut play, k, &mut keys);
            }
            let set_at = play.session.duel.units.iter().find(|u| u.side == 0 && (u.card == 689 || u.card == 691)).map(|u| (u.id, u.position));
            let Some((id, at)) = set_at else {
                eprintln!("seed {seed}: hand index {i}, not set (state {:#x}, status {:?})", play.session.input.mode.state(), play.status);
                continue;
            };
            send(&mut play, pad::START, &mut keys);
            let mut n = 0;
            while play.session.duel.active != 0 || play.intro.is_some() || play.walk.is_some() || play.activation.is_some() {
                send(&mut play, 0, &mut keys);
                n += 1;
                if n > 20_000 || play.session.duel.outcome.is_some() {
                    break;
                }
            }
            for _ in 0..40 {
                send(&mut play, 0, &mut keys);
            }
            if !play.session.duel.units.iter().any(|u| u.id == id) {
                eprintln!("seed {seed}: the set card is gone");
                continue;
            }
            // Walk the cursor to the card.
            for _ in 0..20 {
                let c = play.cursor();
                let k = if c.column < at.column {
                    pad::RIGHT
                } else if c.column > at.column {
                    pad::LEFT
                } else if c.row < at.row {
                    pad::DOWN
                } else if c.row > at.row {
                    pad::UP
                } else {
                    break;
                };
                send(&mut play, k, &mut keys);
                for _ in 0..8 {
                    send(&mut play, 0, &mut keys);
                }
            }
            for &k in [pad::CROSS, 0, 0, 0, 0, pad::R2, 0, 0, 0, 0, pad::CROSS].iter() {
                send(&mut play, k, &mut keys);
            }
            if let Some(a) = &play.activation {
                if !a.field.is_empty() {
                    let mut text = Vec::new();
                    let mut run = (0u16, 0usize);
                    for &k in &keys {
                        if k == run.0 {
                            run.1 += 1;
                        } else {
                            if run.1 > 0 {
                                text.push(if run.1 > 1 { format!("{}*{}", name(run.0), run.1) } else { name(run.0).to_string() });
                            }
                            run = (k, 1);
                        }
                    }
                    text.push(if run.1 > 1 { format!("{}*{}", name(run.0), run.1) } else { name(run.0).to_string() });
                    eprintln!("seed {seed}: field magic at press {}: {} cells; replay {}", keys.len(), a.field.len(), text.join(","));
                    return;
                }
            }
            eprintln!("seed {seed}: no activation ({:?}, status {:?})", play.session.input.mode.state(), play.status);
        }
        panic!("no field magic activation");
    }

    /// Research print: the updates where battle scenes start and where
    /// their arena would start (`ROSES_BATTLE_SEED`, default 1).
    #[test]
    #[ignore = "research print; requires ROSES_DUEL_IMPORT"]
    fn print_battle_scene_starts() {
        let dir = std::path::PathBuf::from(std::env::var_os("ROSES_DUEL_IMPORT").unwrap());
        let seed = std::env::var("ROSES_BATTLE_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(1);
        let mut play = DuelPlay::new(load(&dir).unwrap(), seed, true).unwrap();
        play.arena_available = true;
        let mut was = false;
        for _ in 0..20_000 {
            play.update(0);
            let now = play.intro.is_some();
            if now && !was {
                let i = play.intro.as_ref().unwrap();
                eprintln!("update {}: battle scene starts (leader {}, terrain {})", play.frames, i.leader, i.arena_terrain);
            }
            was = now;
            if play.halted.is_some() || play.session.duel.outcome.is_some() {
                break;
            }
        }
    }

    #[test]
    fn duel_music_follows_the_scene_params() {
        assert_eq!([0, 1, 2, 0x12, 0x13, 0x14, 0x15, 0x16, 0x20].map(|p| duel_music_track(p, 0)), [7, 10, 8, 8, 11, 12, 44, 40, 40]);
        assert_eq!(duel_music_track(2, 1), 9);
    }

    #[test]
    #[ignore = "Requires a private `roses import-duel` folder (ROSES_DUEL_IMPORT)"]
    fn private_no_card_rocks_between_upright_and_sideways() {
        // Whole computer duels: a card's +0x55 never turns back the
        // update after it turned (no back-and-forth rocking).
        let dir = private_import().expect("ROSES_DUEL_IMPORT");
        for seed in [7u64, 124, 229] {
            let mut play = DuelPlay::new(load(&dir).unwrap(), seed, true).unwrap();
            let mut last: std::collections::BTreeMap<_, (u8, i16)> = Default::default();
            let mut rocks = 0;
            for u in 1..=30_000u32 {
                play.update(0);
                for (&id, &p) in &play.sideways {
                    let (prev, dir) = last.get(&id).copied().unwrap_or((p, 0));
                    let d = (i16::from(p) - i16::from(prev)).signum();
                    if d != 0 && dir != 0 && d != dir {
                        rocks += 1;
                        if rocks <= 10 {
                            let unit = play.session.duel.units.iter().find(|x| x.id == id);
                            eprintln!(
                                "seed {seed} update {u}: unit {id:?} {prev:#x} -> {p:#x} (card {:?}, defence {:?}, mode {:?})",
                                unit.map(|x| x.card),
                                unit.map(|x| x.defence),
                                play.session.input.mode.state()
                            );
                        }
                    }
                    last.insert(id, (p, if p == 0 || p == 0xFF { 0 } else if d != 0 { d } else { dir }));
                }
                if play.halted.is_some() || play.session.duel.outcome.is_some() {
                    break;
                }
            }
            eprintln!("seed {seed}: {rocks} turn-backs");
            assert_eq!(rocks, 0, "seed {seed}");
        }
    }

    #[test]
    #[ignore = "Requires a private `roses import-duel` folder (ROSES_DUEL_IMPORT)"]
    fn private_cards_turn_sideways_over_eight_updates() {
        // A computer duel: the first card that turns into defence passes
        // through 0x20 .. 0xE0 on consecutive updates; prints the update.
        let dir = private_import().expect("ROSES_DUEL_IMPORT");
        let mut play = DuelPlay::new(load(&dir).unwrap(), 7, true).unwrap();
        for u in 1..=20_000u32 {
            play.update(0);
            let input = play.overlay_input();
            if let Some(unit) = input.units.iter().find(|x| x.sideways.is_some()) {
                let first = unit.sideways.unwrap();
                eprintln!("update {u}: card {} sideways {first:#x}", unit.card);
                let (side, slot) = (unit.side, unit.slot);
                let mut seen = vec![first];
                for _ in 0..8 {
                    play.update(0);
                    if let Some(x) = play.overlay_input().units.iter().find(|x| (x.side, x.slot) == (side, slot)) {
                        seen.push(x.sideways.unwrap_or(0xFF));
                    }
                }
                eprintln!("  {seen:x?}");
                // 0x20 per update while it turns (it may turn back if the
                // selection-time position is undone).
                assert!(seen.windows(2).all(|w| w[0].abs_diff(w[1]) == 0x20 || w[0].abs_diff(w[1]) == 0x1F || w[0] == w[1]));
                return;
            }
        }
        panic!("no card turned");
    }

    #[test]
    #[ignore = "Requires a private `roses import-duel` folder (ROSES_DUEL_IMPORT)"]
    fn private_destiny_draw_shows_its_message_box() {
        // Seto's module (controller 1) at 500 LP arms the Destiny Draw at its
        // turn start; its next summon opens with event 0x72's box 0x116 for
        // DESTINY_EVENT_UPDATES updates, then the hand ends with the pick.
        let dir = private_import().expect("ROSES_DUEL_IMPORT");
        let mut play = DuelPlay::new(load(&dir).unwrap(), DEFAULT_SEED, true).unwrap();
        play.session.duel.controllers[1] = 1;
        play.session.duel.sides[1].lp = 500;
        let mut hud = DuelHud::new(&play).unwrap();
        let mut shown = 0;
        for _ in 0..40_000 {
            play.update(0);
            hud.update(&play);
            if play.destiny_event.is_some() {
                hud.render();
                let frame = hud.frame.clone().unwrap();
                assert!(frame.prims.iter().any(|p| format!("{:?}", p.element) == "MessageBox"));
                shown += 1;
            } else if shown > 0 {
                break;
            }
            if play.halted.is_some() || play.session.duel.outcome.is_some() {
                break;
            }
        }
        assert_eq!(shown, DESTINY_EVENT_UPDATES, "{}", play.title());
        let side = &play.session.duel.sides[1];
        assert!(side.destiny & 0x80 != 0, "{:#x}", side.destiny);
    }

    #[test]
    #[ignore = "Requires a private `roses import-duel` folder (ROSES_DUEL_IMPORT)"]
    fn private_hud_draws_a_whole_computer_duel() {
        let dir = private_import().expect("ROSES_DUEL_IMPORT");
        let mut play = DuelPlay::new(load(&dir).unwrap(), DEFAULT_SEED, true).unwrap();
        let mut hud = DuelHud::new(&play).unwrap();
        let (mut updates, mut sides, mut states) = (0u64, BTreeSet::new(), BTreeSet::new());
        let mut update_time = std::time::Duration::ZERO;
        for _ in 0..40_000 {
            play.update(0);
            let start = std::time::Instant::now();
            hud.update(&play);
            update_time += start.elapsed();
            hud.render();
            updates += 1;
            sides.insert(hud.previous.d3c);
            states.insert(play.session.input.mode.state());
            if play.halted.is_some() || play.session.duel.outcome.is_some() {
                break;
            }
        }
        let per_render = hud.render_time.as_secs_f64() * 1000.0 / hud.renders.max(1) as f64;
        let per_update = update_time.as_secs_f64() * 1000.0 / updates as f64;
        eprintln!(
            "{updates} updates: HUD state + frame {per_update:.3} ms per update; {} layer renders ({:.1}% of updates), {per_render:.2} ms each; unsupported {:?}",
            hud.renders,
            100.0 * hud.renders as f64 / updates as f64,
            hud.unsupported()
        );
        assert!(play.session.duel.outcome.is_some(), "{}", play.title());
        assert_eq!(sides.len(), 2, "both turns latched");
        assert!(states.contains(&0x1D) && states.contains(&0x43));
        assert!(
            hud.unsupported()
                .iter()
                .all(|u| u.starts_with("terrain tile picture")
                    || u.starts_with("hand card faces (PICTURE.MRG"))
        );
    }
    /// Online names on the HUD (`ROSES_DUEL_IMPORT`): with seat names the
    /// LP panels' PLAYER / ENEMY plates give way to the names in the gold
    /// font; without them the frame is the original's. With
    /// `ROSES_HUD_NAMES_OUT` both frames are written there as PNGs (over
    /// black).
    #[test]
    fn private_online_names_replace_the_hud_plates() {
        let Some(dir) = private_import() else { return };
        let mut play = DuelPlay::new(load(&dir).unwrap(), DEFAULT_SEED, true).unwrap();
        let mut hud = DuelHud::new(&play).unwrap();
        for _ in 0..400 {
            play.update(0);
            hud.update(&play);
        }
        hud.render();
        let plain = hud.layer.clone();
        let plain_prims = hud.frame.as_ref().unwrap().prims.len();
        play.seat_names = Some(["Yugi".into(), "Pegasus Crawford".into()]);
        hud.update(&play);
        hud.render();
        let named = hud.layer.clone();
        assert_ne!(plain, named, "the plates changed");
        assert!(hud.frame.as_ref().unwrap().prims.len() > plain_prims, "gold glyphs for the names");
        play.seat_names = None;
        hud.update(&play);
        hud.render();
        assert_eq!(hud.layer, plain, "offline: the original's plates");
        if let Some(out) = std::env::var_os("ROSES_HUD_NAMES_OUT").map(std::path::PathBuf::from) {
            std::fs::create_dir_all(&out).unwrap();
            for (name, layer) in [("hud-plates-original", &plain), ("hud-plates-online-names", &named)] {
                // Premultiplied over black: the colour channels as they are.
                let rgba: Vec<u8> = layer.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2], 255]).collect();
                let image = bevy::image::Image::new(
                    bevy::render::render_resource::Extent3d { width: HUD_SIZE as u32, height: HUD_SIZE as u32, depth_or_array_layers: 1 },
                    bevy::render::render_resource::TextureDimension::D2,
                    rgba,
                    bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                    bevy::asset::RenderAssetUsages::MAIN_WORLD,
                );
                image.try_into_dynamic().unwrap().save(out.join(format!("{name}.png"))).unwrap();
            }
        }
    }

    /// N-player plates on the real art (`ROSES_DUEL_IMPORT`): with
    /// `ROSES_HUD_NAMES_OUT` the frame is written there as a PNG (over black).
    #[cfg(feature = "online")]
    #[test]
    fn private_compact_plates_draw_their_names_with_long_ones_shortened() {
        let Some(dir) = private_import() else { return };
        let data = load(&dir).unwrap();
        let (_, decks) = crate::duel_multi::preview_seats(&data, 4);
        let duel = crate::duel_multi::preview_duel(data.first_duel.terrain.clone(), &decks, roses_core::duel::MultiMode::Teams, 1, &data.catalogue).unwrap();
        let names = vec!["Christopher1".to_string(), "Pegasus Crawford".to_string(), String::new(), "Mai Valentine".to_string()];
        let mut play = DuelPlay::multi_preview(data, duel, Some(0), names, 1_000_000).unwrap();
        let mut hud = DuelHud::new(&play).unwrap();
        for _ in 0..200 {
            play.update(0);
            hud.update(&play);
        }
        hud.render();
        if let Some(out) = std::env::var_os("ROSES_HUD_NAMES_OUT").map(std::path::PathBuf::from) {
            std::fs::create_dir_all(&out).unwrap();
            let rgba: Vec<u8> = hud.layer.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2], 255]).collect();
            let image = bevy::image::Image::new(
                bevy::render::render_resource::Extent3d { width: HUD_SIZE as u32, height: HUD_SIZE as u32, depth_or_array_layers: 1 },
                bevy::render::render_resource::TextureDimension::D2,
                rgba,
                bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                bevy::asset::RenderAssetUsages::MAIN_WORLD,
            );
            image.try_into_dynamic().unwrap().save(out.join("hud-compact-plates.png")).unwrap();
        }
    }

    #[test]
    fn event_0x36_init_camera_is_0x262630() {
        // Cell target, polar (180, 112.5 deg / side 1 292.5 deg, 33.75 deg).
        let m = trap_cut((6, 0), 0);
        assert_eq!((m.target, m.frames), ([150.0, 0.0, -150.0], 0));
        let deg = |r: f64| r.to_degrees();
        assert!((m.polar[0] - 180.0).abs() < 1e-9);
        assert!((deg(m.polar[1]) - 112.5).abs() < 0.01 && (deg(m.polar[2]) - 33.75).abs() < 0.01, "{:?}", m.polar);
        let m = trap_cut((6, 0), 1);
        assert!((deg(m.polar[1]) - 292.5).abs() < 0.01);
        // The plans' camera steps: a face-down unit takes the cell branch,
        // the overview cut (500, 90 deg, 33.75 deg) in 0 frames.
        use crate::duel_monster_fx::{CameraStep, Target};
        let t = Target { unit: (1, 3), cell: (2, 2), card: 7, face_up: false, defence: false, face: None };
        let cursor = GridPos { column: 3, row: 6 };
        let (u, c) = camera_for_step(CameraStep::Unit(t, 0), 1, cursor);
        assert_eq!((u.map(|u| (u.card, u.cell_distance, u.frames)), c), (Some((NO_MODEL, 180, 0)), None));
        let (u, c) = camera_for_step(CameraStep::Cell(t, 16, 500), 0, cursor);
        assert_eq!((u.map(|u| (u.cell_distance, u.frames)), c), (Some((500, 16)), None));
        let (u, c) = camera_for_step(CameraStep::Overview(0), 0, cursor);
        assert!(u.is_none() && c.is_some_and(|c| c.frames == 0 && c.polar[0] == 500.0));
        let (_, c) = camera_for_step(CameraStep::Back(16), 0, cursor);
        assert_eq!(c.map(|c| (c.frames, c.polar)), Some((16, crate::duel_board::default_polar())));
    }

    #[test]
    fn blocker_flags_the_scene_only_between_two_monsters() {
        // 0x263AB0: type 5 (Blocker 493, e0 0x55B) returns 0x10000 with the
        // rec+0x10 shield bit only when both slots are non-zero; type 0xD
        // (Dark Elf) always [F; emu B1-B3: 0x5E +0xC bits 22 / 20].
        let blocker = [0x55B, 0];
        assert!(prepass_flags(blocker, 3, 5, Some(1)));
        assert!(!prepass_flags(blocker, 3, 0, None), "against a leader: result 0");
        let elf = [0xD00, 0x10032];
        assert!(prepass_flags(elf, 3, 0, None) && prepass_flags(elf, 0, 2, Some(1)));
        // 0x1B: only against the kind in hi; type 4 mode 2 against its filter.
        assert!(prepass_flags([0x1B00, 0x0005_01F4], 1, 2, Some(5)) && !prepass_flags([0x1B00, 0x0005_01F4], 1, 2, Some(6)));
        assert!(prepass_flags([0x482, 0], 1, 2, Some(2)) && !prepass_flags([0x482, 0], 1, 2, Some(3)));
        assert!(!prepass_flags([0x3A00, 0], 1, 2, Some(2)), "types the engine refuses stay unflagged");
    }

    #[test]
    fn dark_elf_stage_opens_the_scene_with_box_0x105() {
        let frozen: OverlayInput = serde_json::from_str("{}").unwrap();
        let camera = battle_camera(GridPos { column: 3, row: 4 }, GridPos { column: 3, row: 3 });
        let mut intro = BattleIntro {
            frozen,
            turns: Vec::new(),
            frame: 0,
            camera,
            versus_cards: Some([(0, (3, 4), (79, 4, 2000, 800)), (1, (3, 3), (2, 4, 1400, 1200))]),
            arrived: Some(0),
            versus: None,
            damage: (0, 0),
            destroyed: 0,
            effect_battle: None,
            arena_terrain: 0,
            arena_end: None,
            leader: false,
            effect_units: vec![(0, (3, 4))],
            effect_c513: Vec::new(),
            light: 128,
            after_track: None,
            revealed: Vec::new(),
        };
        // The scene's load (36), then 16 U of c513 and the box closed the
        // update after: the unit camera starts 17 U later.
        assert_eq!((intro.scene_hold(), intro.effect_stage(), intro.load_hold()), (SCENE_LOAD_HOLD, 17, SCENE_LOAD_HOLD + 17));
        let boxes: Vec<Option<u16>> = (0..60).map(|f| {
            intro.frame = f;
            intro.effect_box()
        }).collect();
        assert_eq!((boxes[35], boxes[36], boxes[52], boxes[53]), (None, Some(0x105), Some(0x105), None));
        intro.effect_units.clear();
        assert_eq!(intro.load_hold(), SCENE_LOAD_HOLD);
    }
}
