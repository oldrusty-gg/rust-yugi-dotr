//! The scripted practice lessons ("Play a Practice Duel" = Yes, and the
//! map's tutorial menu, SYSTEM icon 3): the original's controllers 32–36,
//! which drive **both** sides of a duel
//! through the same pad input as a person, edit the duel directly under a
//! fade-to-black, and page tutorial messages that the human confirms.
//!
//! Research (private): the project notes
//! with `steps-3N.txt` (every step of every lesson, decoded from the PAL
//! executable, run in an R5900 interpreter and matched to saved states).
//! Tags in comments: [F] from code, [emu] interpreter run, [O] saved state,
//! [H] hypothesis.
//!
//! Model [F]: one script state (the original keeps it in side 0's AI
//! scratch, S = 0x2DABA0, whatever side is active) with a step index, a
//! per-step poll counter and the message/fade/blink state. The duel input
//! dispatcher polls the script once per frame while the input layer is in a
//! polled sub-state and nothing is busy; the script returns one pad word for
//! the active side (used as held, edge and repeat key at once; side 1's
//! d-pad is mirrored by [`super::ui::DuelUi::press`] like any other key).
//! Every poll: a fresh Start on the human pad ends the series (−2); a step
//! index past the lesson's count ends the lesson (−1); otherwise the step
//! handler runs top to bottom: a *gate* that is not yet satisfied ends the
//! poll without a key, and when all gates pass the *actions* run and the
//! step advances (normally +1, counter := 0).
//!
//! Each lesson is Rust code with one arm per step ([`lesson1`] …
//! [`lesson5`]) over the primitives of [`Ctx`]. The direct duel
//! edits live in the `impl Duel` block below (`script_*`), so the rules
//! engine keeps its command interface for play.

use super::ui::DuelUi;
use super::{
    CardData, Duel, DuelError, DuelEvent, DuelSetup, EndReason, Result, DECK_SIZE, MAX_LP,
    MAX_STARS, PAD_CONTROLLER,
};
use crate::practice::{next_practice_counter, DeckInput, PRACTICE_PAIRS};
use crate::{GridPos, UnitId, CELL_COUNT, NORMAL_TERRAIN};
use serde::{Deserialize, Serialize};

pub mod destiny;
pub mod stand_in;
mod lesson1;
mod lesson2;
mod lesson3;
mod lesson4;
mod lesson5;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests45;

pub use destiny::{DestinyDraw, DestinyRows, DESTINY_MESSAGE, DESTINY_ROW_COUNT, DESTINY_USED};

/// Story event the game resumes at after the series (FLOW.md Y5/Y6) [F].
pub const STORY_RESUME_EVENT: u16 = 10060;
/// Music track restarted with it (0x217110(25, 1)) [F].
pub const STORY_RESUME_TRACK: u8 = 25;

/// Fade byte limit (0x2DF5B0/0x2DF5B1 at 0x80 = fully visible, 0 = black).
pub const FADE_VISIBLE: u8 = 0x80;
/// Message slide: 6 polls of 0x50 up to 0x1E0 (0x280C80).
pub const SLIDE_POLLS: u8 = 6;
pub const SLIDE_STEP: u16 = 0x50;
pub const SLIDE_FULL: u16 = 0x1E0;
/// Blink period in polls; the icon shows while `counter % 10 < 5` [F; H for
/// which counter].
pub const BLINK_PERIOD: u32 = 10;
/// Pointer kind of every lesson blink (0x2DB84E).
pub const BLINK_POINTER: u16 = 0x800;

/// SFX ids the script plays (0x1E8xxx sound calls) [F].
pub mod sfx {
    pub const PAGE: u8 = 3;
    pub const LAST_PAGE: u8 = 4;
    pub const BLINK: u8 = 0x13;
}

/// Keys of the pad words the scripts return (same bits as [`super::ai::pad`]).
pub use super::ai::pad;

/// The ids 0x280E00 replaces when bit2 of the active side's deck-buffer flag
/// byte (+0xFF9C) is set; the loader's buffer reset sets it (0x1C in every
/// practice state) [F, O 005-t5, emu].
pub const TEXT_REMAP: [(u16, u16); 19] = [
    (3589, 4092),
    (4041, 4093),
    (4042, 4094),
    (4043, 4095),
    (3873, 4096),
    (3881, 4097),
    (3887, 4098),
    (3890, 4099),
    (3900, 4100),
    (3901, 4101),
    (3903, 4102),
    (3915, 4103),
    (3920, 4104),
    (3922, 4105),
    (3943, 4106),
    (3948, 4107),
    (3950, 4108),
    (3989, 4109),
    (3992, 4110),
];

/// 0x280E00: the message id actually shown.
pub fn remap_text(id: u16, variant: bool) -> u16 {
    if !variant {
        return id;
    }
    TEXT_REMAP
        .iter()
        .find(|(from, _)| *from == id)
        .map_or(id, |&(_, to)| to)
}

/// The caption window of `NAME(card)` shows message id card + 323
/// (0x1E5540(0, 0, 0x50, card + 323)) [F, O 383/323].
pub const CAPTION_BASE: u16 = 323;

/// 0x280890: a side whose controller byte is a script (0x20 ≤ c < 100).
pub fn scripted(controller: u8) -> bool {
    (0x20..PAD_CONTROLLER).contains(&controller)
}

/// The five lessons, by controller byte [F]: the "Yes" series plays the
/// first three; the map's tutorial menu (W+546D rows 0..4, labels MSG
/// 4087–4091) any one of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lesson {
    /// Controller 32: basics (summon, move, battle, positions, magic,
    /// surrender); 158 steps.
    Basics,
    /// Controller 33: how a duel is decided; 64 steps.
    Victory,
    /// Controller 34: limits, stars, spells, terrain, spellbind, Destiny
    /// Draw, ranks; 288 steps.
    Rules,
    /// Controller 35: technique (combos, fusions, power-ups, card info,
    /// traps, rituals); 322 steps.
    Technique,
    /// Controller 36: the duel's functions (hand and graveyard views,
    /// settings, display toggle, camera); 69 steps.
    Functions,
}

/// The menu's row labels (0x23B770 composes MSG 0xFF7 + row) [F].
pub const MENU_LABELS: [u16; 5] = [4087, 4088, 4089, 4090, 4091];

impl Lesson {
    pub const ALL: [Lesson; 5] = [
        Lesson::Basics,
        Lesson::Victory,
        Lesson::Rules,
        Lesson::Technique,
        Lesson::Functions,
    ];
    /// The "Yes" series (0x23B590's counter 0, 1, 2).
    pub const SERIES: [Lesson; 3] = [Lesson::Basics, Lesson::Victory, Lesson::Rules];

    /// Side controller byte (+0x33C) of both sides during the lesson.
    pub fn controller(self) -> u8 {
        match self {
            Lesson::Basics => 32,
            Lesson::Victory => 33,
            Lesson::Rules => 34,
            Lesson::Technique => 35,
            Lesson::Functions => 36,
        }
    }

    /// 0x23B3B0: the tutorial menu's row W+546D picks controls 32..35 for
    /// rows 0..3 and 36 for any other value [F].
    pub fn from_menu_row(row: u8) -> Lesson {
        Lesson::ALL[usize::from(row.min(4))]
    }

    /// The menu row's label (MSG id).
    pub fn menu_label(self) -> u16 {
        MENU_LABELS[self.series_index()]
    }

    pub fn from_controller(controller: u8) -> Option<Lesson> {
        Lesson::ALL
            .into_iter()
            .find(|l| l.controller() == controller)
    }

    /// Step count (jump table size) [F].
    pub fn step_count(self) -> u16 {
        match self {
            Lesson::Basics => 158,
            Lesson::Victory => 64,
            Lesson::Rules => 288,
            Lesson::Technique => 322,
            Lesson::Functions => 69,
        }
    }

    /// Deck records (side 0, side 1) the setups 0x23B590 / 0x23B3B0 load
    /// for this lesson: pairs (0x11,0x12) … (0x19,0x1A) [F].
    pub fn deck_pair(self) -> (u8, u8) {
        PRACTICE_PAIRS[self.series_index()]
    }

    /// 0, 1, 2 in the series; 0..4 = the tutorial menu's row.
    pub fn series_index(self) -> usize {
        match self {
            Lesson::Basics => 0,
            Lesson::Victory => 1,
            Lesson::Rules => 2,
            Lesson::Technique => 3,
            Lesson::Functions => 4,
        }
    }

    /// The lesson that follows on a −1 return in the "Yes" series (none
    /// after the third; lessons 4 and 5 only run alone).
    pub fn next(self) -> Option<Lesson> {
        match self {
            Lesson::Basics => Some(Lesson::Victory),
            Lesson::Victory => Some(Lesson::Rules),
            Lesson::Rules | Lesson::Technique | Lesson::Functions => None,
        }
    }

    /// Step 0 of the lesson (every lesson's step 0 jumps to 2).
    fn run<C: CardData>(self, ctx: &mut Ctx<'_, C>) -> Res {
        match self {
            Lesson::Basics => lesson1::step(ctx),
            Lesson::Victory => lesson2::step(ctx),
            Lesson::Rules => lesson3::step(ctx),
            Lesson::Technique => lesson4::step(ctx),
            Lesson::Functions => lesson5::step(ctx),
        }
    }

    /// The per-lesson part of the duel init (0x277640 → 0x280440(ctrl, S))
    /// [F]: 35 sets the stars to 8 (0x1DF860(s, 4, 8) for both sides; the
    /// first poll shows 8 / 4 [emu], so side 1's is overwritten later in
    /// the init and only side 0's is kept here); 36 fills both graveyards
    /// (0x280610); 32 calls 0x280600 (nothing the port keeps); 33/34 none.
    pub fn init_duel(self, duel: &mut Duel) {
        match self {
            Lesson::Technique => duel.script_set_stars(0, 8),
            Lesson::Functions => {
                for (side, cards) in lesson5::GRAVEYARDS.iter().enumerate() {
                    for &card in cards {
                        duel.script_graveyard_add(side as u8, card);
                    }
                }
            }
            Lesson::Basics | Lesson::Victory | Lesson::Rules => (),
        }
    }
}

/// The lesson duel: board 0 (every cell terrain 7), the two deck records'
/// leaders at (3,6)/(3,0), decks in record order (no shuffle; hands stay
/// empty until the first summon refill), LP 4000, side 0 first, both
/// controllers = the lesson (the intro is skipped for control < 100) [F, O].
/// `leader_abilities` is not traced for the lessons (the ability word
/// 0x2DAB00 is 3 in all tutorial states; which rank abilities the leaders
/// list is not established), so callers pass what they know, or empty lists.
pub fn lesson_setup(
    lesson: Lesson,
    records: [&DeckInput; 2],
    leader_abilities: [Vec<u8>; 2],
) -> Result<DuelSetup> {
    let (a, b) = lesson.deck_pair();
    if records[0].index != a || records[1].index != b {
        return Err(DuelError::InvalidCommand(
            "deck records do not match the lesson",
        ));
    }
    if records.iter().any(|r| r.cards.len() != DECK_SIZE) {
        return Err(DuelError::InvalidCommand("deck record must hold 40 cards"));
    }
    Ok(DuelSetup {
        terrain: vec![NORMAL_TERRAIN; CELL_COUNT],
        leaders: [records[0].leader_card, records[1].leader_card],
        decks: [records[0].cards.clone(), records[1].cards.clone()],
        first: 0,
        leader_abilities,
        controllers: [lesson.controller(); 2],
    })
}

/// Why a lesson stopped (the negative controller return) [F].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LessonEnd {
    /// −1 with the practice counter ≠ 0: `0x23B590(1)` starts the next
    /// lesson (mode replace) with a fresh duel; the new counter is given.
    NextLesson { lesson: Lesson, series_counter: u8 },
    /// The duel mode is popped: after the third lesson of the "Yes" series
    /// (counter already 0) or on Start anywhere (`quit`) the story resumes
    /// at [`STORY_RESUME_EVENT`] with [`STORY_RESUME_TRACK`] restarted; a
    /// lesson started from the map's tutorial menu (counter 0) returns to
    /// the map, whose mode switch restores the card block (0x21D14C).
    SeriesEnd { quit: bool },
}

/// Where a message window is in its life (0x280C80) [F, emu].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum MessagePhase {
    /// The first page slides in: `polls` of [`SLIDE_POLLS`] done, offset
    /// from the resting place = `SLIDE_FULL − polls × SLIDE_STEP`.
    SlideIn { polls: u8 },
    /// Fully shown; every poll reads the human's face buttons.
    Shown,
    /// The last page was confirmed (S+0x160 = 0xFFFE): `polls` of
    /// [`SLIDE_POLLS`] slid out; the poll after the sixth closes the window
    /// and advances the step.
    SlideOut { polls: u8 },
}

/// The open tutorial message window (state 0x0F, id at 0x2DB850).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub first: u16,
    pub last: u16,
    /// Current page id before the text remap.
    pub page: u16,
    pub phase: MessagePhase,
}

impl Message {
    /// Message id to draw (after 0x280E00).
    pub fn shown(&self, text_variant: bool) -> u16 {
        remap_text(self.page, text_variant)
    }

    /// Horizontal slide offset from the resting place (S+0x1A4 counts up to
    /// 0x1E0 while sliding in; the window is drawn at 0x1E0 − pos).
    pub fn offset(&self) -> u16 {
        match self.phase {
            MessagePhase::SlideIn { polls } => SLIDE_FULL - SLIDE_STEP * u16::from(polls),
            MessagePhase::Shown => 0,
            MessagePhase::SlideOut { polls } => SLIDE_STEP * u16::from(polls),
        }
    }
}

/// The blinking pointer (0x280690: kind 0x2DB84E at screen offset
/// 0x2DB842/0x2DB844, marker 0x2DB854 = 0x17).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blink {
    pub kind: u16,
    pub x: i16,
    pub y: i16,
    /// Polls since it was switched on.
    pub counter: u32,
}

impl Blink {
    /// Shown while `counter % 10 < 5` [F; H: which counter].
    pub fn visible(&self) -> bool {
        self.counter % BLINK_PERIOD < BLINK_PERIOD / 2
    }
}

/// A camera or model request the script made (presentation only; the
/// meanings marked [H] are not established).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum CameraOp {
    /// 0x280B10: save the camera into S+0x1B0.
    Save,
    /// 0x280A90: restore it.
    Restore,
    /// 0x172DE0/0x174EC0/0x176470: close-up on the leader or first unit
    /// showing `card`.
    CloseUp { card: u16 },
    /// 0x262060(v).
    Preset { value: u8 },
    /// 0x281020.
    Zoom,
    /// 0x1E2B80(side, slot, value): unit model visibility [H].
    UnitFx { side: u8, slot: u8, value: i32 },
    /// 0x1E2D70(side, slot, value) [H].
    UnitFx2 { side: u8, slot: u8, value: i32 },
}

/// What one poll produced besides the key: one-shots for the presentation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptFrame {
    /// Pad word returned for the active side (0 = none).
    pub key: u16,
    /// Step index before and after the poll.
    pub step: (u16, u16),
    /// Sounds requested this poll ([`sfx`]).
    pub sounds: Vec<u8>,
    /// Camera requests made this poll (repeats of the previous request are
    /// dropped; the scripts call the close-up every poll of a step).
    pub camera: Vec<CameraOp>,
    /// Engine events of the script's direct edits (created units).
    pub events: Vec<DuelEvent>,
    /// Side 0's turn started with Exodia complete: the original shows the
    /// Exodia event but a scripted side gets no win (turn-verified.md §7 D).
    pub exodia_event: bool,
    /// The summon this frame opened drew the Destiny card (event 0x72 with
    /// message [`DESTINY_MESSAGE`], then the card last in the refill).
    #[serde(default)]
    pub destiny: Option<DestinyDraw>,
    /// The lesson stopped this poll.
    pub ended: Option<LessonEnd>,
}

/// Script state S (0x2DABA0) plus the presentation bytes it drives, shared
/// by both seats.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tutorial {
    pub lesson: Lesson,
    /// S+0x1A2.
    pub step: u16,
    /// S+0x1A4: polls of the current step counted by its wait gate.
    pub counter: u32,
    /// Open message window (S+0x160 ≠ 0xFFFF).
    pub message: Option<Message>,
    /// 0x2DF5B0 / 0x2DF5B1 (0x80 visible, 0 black).
    pub fade: [u8; 2],
    pub blink: Option<Blink>,
    /// `NAME(card)` + `CAPTION`: the card-name box (window id card + 323).
    pub caption: Option<u16>,
    /// FLAG906 = 0x2DB906 bit7, mirrored to 0x2DB907 bit0 every poll (set
    /// around fades; HUD/camera mode [H]).
    pub hud_flag: bool,
    /// Last camera request (for de-duplication and late viewers).
    pub camera: Option<CameraOp>,
    /// The step that made the last camera request: a step arm repeats its
    /// camera calls on every poll of a gate, so a request equal to the
    /// last one is reported once per step, but a later step asking for the
    /// same view again (lesson 3 steps 51 and 61 both close up on the
    /// Celtic Guardian, replaced in between) is reported again.
    #[serde(default)]
    pub camera_step: Option<u16>,
    /// 0x312B80 after this lesson began: 1, 2, 0 in the "Yes" series
    /// (`practice::next_practice_counter`), 0 for a single lesson.
    pub series_counter: u8,
    /// Deck-buffer flag bit2: the text remap applies.
    pub text_variant: bool,
    /// 0x2DB907 bit5 (`NO_LP0_END`): the LP-0 end check is skipped.
    pub no_lp0_end: bool,
    /// Polls so far (diagnostics).
    pub polls: u32,
    pub ended: Option<LessonEnd>,
    /// The shared LCG state (0x19AE88) the Destiny Draw's pick consumes one
    /// rand31 of ([`destiny::pick`]); the frontend seeds it, the value in the
    /// real game depends on frame timing.
    #[serde(default)]
    pub rng: u64,
    /// The card rows of the Destiny Draw (0x2B7AF0,
    /// `roses-assets::parse_pal_destiny_rows`); without them a lesson's
    /// special draw is refused.
    #[serde(default)]
    pub destiny_rows: Option<DestinyRows>,
    /// `LEADERRANK(s, card, rank)` 0x216A60: the rank the script wrote into
    /// the persistent deck buffer's trunk entry of each side's leader card
    /// (lesson 3 step 259: both to 12) [F]. Presentation state (the leader
    /// details views of steps 265–276 list the abilities of this rank);
    /// the engine's ability lists are not derived from it — the rank-12
    /// abilities the lesson relies on (#13 Open Opponent's Card, #14) are
    /// not modelled, so the step whose outcome needs one is refused.
    #[serde(default)]
    pub leader_ranks: [Option<u8>; 2],
    /// `LEADERRANK` of a card that is not the side's leader (lesson 4 step
    /// 208: the Celtic Guardian to 12, whose details then list its leader
    /// abilities): (side, card, rank), presentation state.
    #[serde(default)]
    pub card_ranks: Vec<(u8, u16, u8)>,
    /// Pad 0's analog bytes (+0x206..+0x209) as the script left them this
    /// poll: centred (0x80) at every poll's start, lesson 5 tilts them
    /// (0x2035B0) to show the camera sticks [F]; presentation only.
    #[serde(default = "centred")]
    pub analog: [u8; 4],
    /// S+0x162: the stick steps' own poll counter.
    #[serde(default)]
    pub aux: u32,
    /// 0x280760(S, kind, c, r) this poll: the trap-area highlight (lesson
    /// 4 steps 265–266, 277–278) [F; drawn while the step calls it, H].
    #[serde(default)]
    pub area: Option<(u8, u8, u8)>,
}

fn centred() -> [u8; 4] {
    [0x80; 4]
}

impl Tutorial {
    /// A lesson with a given practice counter (0x312B80 as it stands
    /// *after* 0x23B590 advanced it) and text-variant flag.
    pub fn new(lesson: Lesson, series_counter: u8, text_variant: bool) -> Self {
        Self {
            lesson,
            step: 0,
            counter: 0,
            message: None,
            fade: [FADE_VISIBLE; 2],
            blink: None,
            caption: None,
            // 0x2DB906 bit7 is set when a lesson starts (0x80 in the saved
            // states at step 0 of lessons 1 and 3 [O 187-n55, 000]); the
            // first close-up clears it.
            hud_flag: true,
            camera: None,
            camera_step: None,
            series_counter,
            text_variant,
            no_lp0_end: false,
            polls: 0,
            ended: None,
            rng: 0,
            destiny_rows: None,
            leader_ranks: [None; 2],
            card_ranks: Vec::new(),
            analog: centred(),
            aux: 0,
            area: None,
        }
    }

    /// A lesson on its own, as the map's tutorial menu starts it (0x23B710
    /// → 0x23B3B0 leaves the practice counter at 0, so −1 pops back to the
    /// map) with the card-name option of the player's card block as the
    /// text variant (+0xFF9C bit2, `0x214490(0, 0)`) [F].
    pub fn single(lesson: Lesson, text_variant: bool) -> Self {
        Self::new(lesson, 0, text_variant)
    }

    /// The Destiny Draw's card rows (needed by lesson 3 step 248).
    pub fn with_destiny_rows(mut self, rows: DestinyRows) -> Self {
        self.destiny_rows = Some(rows);
        self
    }

    /// The RNG state the special draw consumes.
    pub fn with_rng(mut self, rng: u64) -> Self {
        self.rng = rng;
        self
    }

    /// The "Yes" series: lesson n starts with counter n+1 (wrapping to 0 for
    /// the third, so its end pops to the story), text variant on [F, O].
    /// Lessons 4 and 5 are not in the series: they start alone (counter 0).
    pub fn series(lesson: Lesson) -> Self {
        if !Lesson::SERIES.contains(&lesson) {
            return Self::single(lesson, true);
        }
        let mut counter = 0;
        for _ in 0..=lesson.series_index() {
            counter = next_practice_counter(counter);
        }
        Self::new(lesson, counter, true)
    }

    /// The lesson `NextLesson` asks for, as a fresh state.
    pub fn continue_with(end: LessonEnd, text_variant: bool) -> Option<Self> {
        match end {
            LessonEnd::NextLesson {
                lesson,
                series_counter,
            } => Some(Self::new(lesson, series_counter, text_variant)),
            LessonEnd::SeriesEnd { .. } => None,
        }
    }

    /// Whether the next poll reads the human's face buttons (a page is
    /// fully shown).
    pub fn message_accepts_input(&self) -> bool {
        matches!(
            self.message,
            Some(Message {
                phase: MessagePhase::Shown,
                ..
            })
        )
    }

    /// The negative return: −1 → next lesson or pop, −2 → pop [F Y5].
    fn end(&mut self, code: i32) -> LessonEnd {
        let end = if code == -1 && self.series_counter != 0 {
            match self.lesson.next() {
                Some(lesson) => LessonEnd::NextLesson {
                    lesson,
                    series_counter: next_practice_counter(self.series_counter),
                },
                // Counter ≠ 0 with no fourth lesson cannot happen in the
                // series (the counter wraps to 0 when the third starts).
                None => LessonEnd::SeriesEnd { quit: false },
            }
        } else {
            LessonEnd::SeriesEnd { quit: code == -2 }
        };
        self.ended = Some(end);
        end
    }

    /// One controller call (0x1CFF80 → the lesson driver) for the active
    /// side. `human` holds the fresh presses of pad 0 this frame (only Start
    /// and the face buttons are read). Returns the pad word for the active
    /// side in `ScriptFrame::key`. An `Err` means the port cannot reproduce
    /// the step; the duel and script state may then be partially edited, so
    /// callers restore their copies.
    pub fn poll<C: CardData>(
        &mut self,
        duel: &mut Duel,
        ui: &mut DuelUi,
        cards: &C,
        human: u16,
    ) -> Result<ScriptFrame> {
        let mut frame = ScriptFrame {
            step: (self.step, self.step),
            ..Default::default()
        };
        if self.ended.is_some() {
            return Ok(frame);
        }
        self.polls += 1;
        // Every poll before the step [F]: 0x2DB907 bit0 := bit7 of
        // 0x2DB906 (kept as one flag here; it is the duel's display flag
        // d66 bit7, which L2 toggles [emu lesson 5 steps 47–50]); pad-0
        // analog bytes centred (presentation); fresh Start → −2; step ≥
        // count → −1.
        self.hud_flag = !ui.hud_hidden;
        self.analog = centred();
        self.area = None;
        if human & pad::START != 0 {
            frame.ended = Some(self.end(-2));
            return Ok(frame);
        }
        if self.step >= self.lesson.step_count() {
            frame.ended = Some(self.end(-1));
            return Ok(frame);
        }
        let lesson = self.lesson;
        let mut ctx = Ctx {
            tut: self,
            duel,
            ui,
            cards,
            human,
            frame,
            ret: None,
        };
        match lesson.run(&mut ctx) {
            Ok(()) | Err(Halt::Pending) => (),
            Err(Halt::Error(e)) => return Err(e),
        }
        let ret = ctx.ret;
        let mut frame = ctx.frame;
        if let Some(blink) = self.blink.as_mut() {
            // The icon's period starts on the poll it is switched on and
            // every tenth poll after; SFX 0x13 at each start [F; H counter].
            if blink.counter.is_multiple_of(BLINK_PERIOD) {
                frame.sounds.push(sfx::BLINK);
            }
            blink.counter += 1;
        }
        if let Some(code) = ret {
            frame.ended = Some(self.end(code));
        }
        frame.step.1 = self.step;
        Ok(frame)
    }

    /// After a command was applied: with `NO_LP0_END` set the original's
    /// cursor-frame judge (0x1D33CC, D67 bit5) never runs, so LP 0 does
    /// not end the duel [F]. The engine's check is the same frame check at
    /// the end of `Duel::apply`, so its outcome is undone here.
    pub fn suppress_lp0_end(&self, duel: &mut Duel, events: &mut Vec<DuelEvent>) {
        if self.no_lp0_end {
            duel.script_suppress_lp0_end(events);
        }
    }
}

/// Why a step arm stopped before its end.
#[derive(Debug)]
pub enum Halt {
    /// A gate is not satisfied yet; the poll returns whatever key was set
    /// (normally none).
    Pending,
    Error(DuelError),
}

impl From<DuelError> for Halt {
    fn from(e: DuelError) -> Self {
        Halt::Error(e)
    }
}

/// Result of a step arm: `Ok` when it ran to its end this poll.
pub type Res = std::result::Result<(), Halt>;

/// One poll of a lesson: the primitives the step arms are written with, in
/// the order the original runs them (gates end the poll with
/// `Halt::Pending` via `?`).
pub struct Ctx<'a, C: CardData> {
    pub tut: &'a mut Tutorial,
    pub duel: &'a mut Duel,
    pub ui: &'a mut DuelUi,
    pub cards: &'a C,
    /// Fresh presses of pad 0 this frame.
    pub human: u16,
    pub frame: ScriptFrame,
    /// An explicit negative return requested by the arm (`RETURN -1`).
    ret: Option<i32>,
}

impl<C: CardData> Ctx<'_, C> {
    pub fn step(&self) -> u16 {
        self.tut.step
    }

    // ---- step flow ----------------------------------------------------

    /// `next`: step + 1, counter 0.
    pub fn next(&mut self) {
        self.goto(self.tut.step + 1);
    }

    /// `goto N` / `STEP:=N`: jump, counter 0.
    pub fn goto(&mut self, step: u16) {
        self.tut.step = step;
        self.tut.counter = 0;
    }

    /// `KEY k`: the pad word this poll returns (the last one set wins).
    pub fn key(&mut self, key: u16) {
        self.frame.key = key;
    }

    /// `RETURN -1`: the lesson ends on this poll.
    pub fn end_lesson(&mut self) {
        self.ret = Some(-1);
    }

    // ---- gates ------------------------------------------------------------

    /// `wait N polls`: passes on the N-th poll of the step (counter old ≥
    /// N − 1) [F]. Fades do not count polls; a message after it counts from
    /// the poll the fade completes.
    pub fn wait(&mut self, polls: u32) -> Res {
        let old = self.tut.counter;
        self.tut.counter += 1;
        if old + 1 >= polls {
            Ok(())
        } else {
            Err(Halt::Pending)
        }
    }

    /// `FADEOUT(v)`: 0x2808F0(v, −1) moves both fade bytes toward 0 by `v`
    /// per poll; passes on the first poll both are already there (0x80/v +
    /// 1 polls from visible) [F, emu].
    pub fn fade_out(&mut self, v: u8) -> Res {
        self.fade(v, 0)
    }

    /// `FADEIN(v)`: toward 0x80.
    pub fn fade_in(&mut self, v: u8) -> Res {
        self.fade(v, FADE_VISIBLE)
    }

    fn fade(&mut self, v: u8, target: u8) -> Res {
        if self.tut.fade == [target; 2] {
            return Ok(());
        }
        for b in self.tut.fade.iter_mut() {
            *b = if target > *b {
                b.saturating_add(v).min(target)
            } else {
                b.saturating_sub(v).max(target)
            };
        }
        Err(Halt::Pending)
    }

    /// `MODELREADY(g<<8|slot)` 0x1F70D0: the unit's model is loaded. No
    /// model loading is modelled: always ready.
    pub fn model_ready(&mut self, _unit: u16) -> Res {
        Ok(())
    }

    /// `MSG first..last` (0x280C80) [F, emu]: opens the window if idle; the
    /// first page slides in over [`SLIDE_POLLS`] polls; from the poll after
    /// that every poll reads a fresh Cross/Circle/Triangle/Square on pad 0:
    /// not the last page → SFX 3, next page; last → SFX 4, closing. The
    /// closing slides out over six polls; the poll after that closes the
    /// window, advances the step (counter 0) and passes so the actions
    /// after the message run in the same poll.
    pub fn msg(&mut self, first: u16, last: u16) -> Res {
        let Some(m) = self.tut.message.as_mut() else {
            self.tut.message = Some(Message {
                first,
                last,
                page: first,
                phase: MessagePhase::SlideIn { polls: 1 },
            });
            return Err(Halt::Pending);
        };
        match m.phase {
            MessagePhase::SlideIn { polls } => {
                let polls = polls + 1;
                m.phase = if polls >= SLIDE_POLLS {
                    MessagePhase::Shown
                } else {
                    MessagePhase::SlideIn { polls }
                };
                Err(Halt::Pending)
            }
            MessagePhase::Shown => {
                if self.human & (pad::CROSS | pad::CIRCLE | pad::TRIANGLE | pad::SQUARE) != 0 {
                    if m.page < m.last {
                        m.page += 1;
                        self.frame.sounds.push(sfx::PAGE);
                    } else {
                        m.phase = MessagePhase::SlideOut { polls: 0 };
                        self.frame.sounds.push(sfx::LAST_PAGE);
                    }
                }
                Err(Halt::Pending)
            }
            MessagePhase::SlideOut { polls } => {
                if polls >= SLIDE_POLLS {
                    self.tut.message = None;
                    self.next();
                    Ok(())
                } else {
                    m.phase = MessagePhase::SlideOut { polls: polls + 1 };
                    Err(Halt::Pending)
                }
            }
        }
    }

    /// `BLINK(kind@x,y) ; for N polls ; BLINKOFF`: switches the pointer on
    /// (idempotent), passes on the N-th poll and switches it off then
    /// [F, emu: 62 polls per blink step].
    pub fn blink_for(&mut self, kind: u16, x: i16, y: i16, polls: u32) -> Res {
        if self.tut.blink.is_none() {
            self.tut.blink = Some(Blink {
                kind,
                x,
                y,
                counter: 0,
            });
        }
        self.wait(polls)?;
        self.tut.blink = None;
        Ok(())
    }

    /// `until <cond>: press K every 5 polls` (32:138, 32:145): counts the
    /// step's polls and returns `true` on every `period`-th one.
    pub fn every(&mut self, period: u32) -> bool {
        self.tut.counter += 1;
        self.tut.counter.is_multiple_of(period)
    }

    /// Cursor of the active side (S+0x33E row, +0x33D column).
    pub fn cursor(&self) -> GridPos {
        self.ui.cursor(self.duel)
    }

    // ---- presentation actions -------------------------------------------

    /// `FLAG906=v`: 0x2DB906 bit7 = d66 bit7, the display flag L2 toggles
    /// (set: info windows shown) [emu].
    pub fn flag906(&mut self, on: bool) {
        self.tut.hud_flag = on;
        self.ui.hud_hidden = !on;
    }

    /// 0x280760(S, kind, c, r): the area highlight around a cell.
    pub fn area(&mut self, kind: u8, column: u8, row: u8) {
        self.tut.area = Some((kind, column, row));
    }

    /// 0x17BA48-style stick step: byte `axis` := `value` on every poll; the
    /// 8th poll (S+0x162 old ≥ 7) passes and advances [F].
    pub fn stick_hold(&mut self, axis: usize, value: u8) -> Res {
        self.tut.analog[axis] = value;
        let old = self.tut.aux;
        self.tut.aux += 1;
        if old < 7 {
            return Err(Halt::Pending);
        }
        self.tut.aux = 0;
        self.next();
        Ok(())
    }

    /// 0x17BA84-style stick step: centred for 12 polls, then byte `axis` :=
    /// `value` until S+0x162 reaches 19, which advances [F].
    pub fn stick_return(&mut self, axis: usize, value: u8) -> Res {
        let old = self.tut.aux;
        self.tut.aux += 1;
        if old < 12 {
            return Err(Halt::Pending);
        }
        self.tut.analog[axis] = value;
        if self.tut.aux < 19 {
            return Err(Halt::Pending);
        }
        self.tut.aux = 0;
        self.next();
        Ok(())
    }

    /// 0x1E02F0 + 0x1E03B0 ×n (0x178BF0): the side's hand replaced.
    pub fn set_hand(&mut self, side: u8, cards: &[u16]) {
        self.duel.script_set_hand(side, cards);
    }

    /// `CAMSAVE`, `CAMRESTORE`, `CLOSEUP`, `CAMPRESET`, `CAMZOOM`, `ufx`:
    /// reported once per step (the arms call them on every poll of a gate).
    pub fn camera(&mut self, op: CameraOp) {
        let step = self.tut.step;
        if self.tut.camera != Some(op) || self.tut.camera_step != Some(step) {
            self.tut.camera = Some(op);
            self.tut.camera_step = Some(step);
            self.frame.camera.push(op);
        }
    }

    /// `NAME(card) ; CAPTION`: the card-name box.
    pub fn caption(&mut self, card: u16) {
        self.tut.caption = Some(card);
    }

    pub fn caption_off(&mut self) {
        self.tut.caption = None;
    }

    /// `CURSOR(c,r)` 0x1DFAE0(0, c, r): side 0's cursor (and camera target).
    pub fn cursor_to(&mut self, column: u8, row: u8) {
        self.ui.cursors[0] = GridPos { column, row };
    }

    /// `NO_LP0_END=1` 0x1E60E0(1).
    pub fn no_lp0_end(&mut self) {
        self.tut.no_lp0_end = true;
    }

    // ---- duel edits (see the `impl Duel` block) ---------------------------

    pub fn lp(&mut self, side: u8, value: u16) {
        self.duel.script_set_lp(side, value);
    }

    pub fn stars(&mut self, side: u8, value: u8) {
        self.duel.script_set_stars(side, value);
    }

    /// `PLACE sG cardC@(c,r)`.
    pub fn place(&mut self, side: u8, card: u16, column: u8, row: u8) -> Res {
        let mut events = Vec::new();
        self.duel
            .script_place(side, card, GridPos { column, row }, self.cards, &mut events)?;
        self.frame.events.extend(events);
        Ok(())
    }

    pub fn remove(&mut self, side: u8, slot: u8) {
        self.duel.script_remove(side, slot);
    }

    pub fn clear_units(&mut self) {
        self.duel.script_clear_units();
    }

    pub fn teleport(&mut self, side: u8, slot: u8, column: u8, row: u8) -> Res {
        self.duel
            .script_teleport(side, slot, GridPos { column, row })?;
        Ok(())
    }

    /// `UFLAG(g,slot,fN=v)`.
    pub fn unit_flag(&mut self, side: u8, slot: u8, field: u8, value: bool) -> Res {
        self.duel.script_unit_flag(side, slot, field, value)?;
        Ok(())
    }

    pub fn terrain(&mut self, column: u8, row: u8, code: u8) -> Res {
        self.duel
            .script_set_terrain(GridPos { column, row }, code)?;
        Ok(())
    }

    pub fn round(&mut self, round: u8) {
        self.duel.script_set_round(round);
    }

    pub fn summon_ok(&mut self, side: u8, on: bool) {
        self.duel.script_set_summon_ok(side, on);
    }

    pub fn spellbind(&mut self, side: u8, slot: u8, turns: u8) -> Res {
        self.duel.script_spellbind(side, slot, turns)?;
        Ok(())
    }

    pub fn set_bind(&mut self, side: u8, slot: u8, value: u8) -> Res {
        self.duel.script_set_bind(side, slot, value)?;
        Ok(())
    }

    pub fn deck_reset(&mut self) {
        self.duel.script_deck_reset();
    }

    /// `S0+0x344=v` 0x1DF9E0: the Destiny Draw byte (the session draws the
    /// card at the next summon refill, [`destiny`]).
    pub fn destiny_byte(&mut self, side: u8, value: u8) {
        self.duel.script_set_destiny(side, value);
    }

    /// `LEADERRANK(s, card, rank)` 0x216A60: recorded in
    /// [`Tutorial::leader_ranks`] for the side's leader, else in
    /// [`Tutorial::card_ranks`] (the persistent deck buffers are not
    /// modelled).
    pub fn leader_rank(&mut self, side: u8, card: u16, rank: u8) -> Res {
        if self.duel.leader(side).card == card {
            self.tut.leader_ranks[usize::from(side)] = Some(rank);
        } else {
            self.tut.card_ranks.push((side, card, rank));
        }
        Ok(())
    }
}

/// Unit flag fields of `UFLAG` (0x1E30F0 field N) [F].
pub mod unit_flag {
    /// +0x57 bit0: acted / cannot act.
    pub const ACTED: u8 = 0;
    /// +0x57 bit1 (no engine field).
    pub const BIT1: u8 = 1;
    /// +0x57 bit2: face-up.
    pub const FACE_UP: u8 = 2;
    /// +0x57 bit3: defence.
    pub const DEFENCE: u8 = 3;
    /// bit4 + bit2: committed face-up.
    pub const COMMITTED_FACE_UP: u8 = 4;
    /// bit5 + bit3: committed defence.
    pub const COMMITTED_DEFENCE: u8 = 5;
}

/// `DECKRESET` (0x176730): both decks emptied, then these five cards each;
/// both hands emptied [F] (side 0 = the first five of record 0x11, side 1 =
/// of record 0x12, as the lesson-1 hands show [O]).
pub const DECK_RESET: [[u16; 5]; 2] = [[321, 285, 156, 249, 505], [21, 423, 522, 12, 433]];

/// The direct duel edits of the scripts (TUTORIAL.md §2), on the engine
/// state without the command interface.
impl Duel {
    /// `LPs=v` 0x1E0710(s, 4, v): set, clamped to 0..=9999.
    pub fn script_set_lp(&mut self, side: u8, value: u16) {
        self.sides[usize::from(side)].lp = value.min(MAX_LP);
    }

    /// `STARSs op4 v` 0x1DF860(s, 4, v): set, clamped to 0..=12.
    pub fn script_set_stars(&mut self, side: u8, value: u8) {
        self.sides[usize::from(side)].stars = value.min(MAX_STARS);
    }

    /// `PLACE` 0x1E0800(g, c, r, card, 0): a fresh unit (face-down, attack
    /// position, not acted) in the first free slot of its zone; nothing
    /// when the cell is occupied or the zone is full [F]. Unknown cards are
    /// an error. Returns the unit created.
    pub fn script_place(
        &mut self,
        side: u8,
        card: u16,
        cell: GridPos,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<Option<UnitId>> {
        if cell.index().is_none() || side > 1 {
            return Err(DuelError::InvalidCommand("script place off the board"));
        }
        if cards.card(card).is_none() {
            return Err(DuelError::InvalidCommand("script place of an unknown card"));
        }
        if self.unit_at(cell).is_some() {
            return Ok(None);
        }
        Ok(self.create(side, card, cell, cards, events))
    }

    /// `REMOVE(g, slot)` 0x1E0E30: delete the unit (no graveyard entry);
    /// nothing when the slot is empty.
    pub fn script_remove(&mut self, side: u8, slot: u8) {
        self.units.retain(|u| !(u.side == side && u.slot == slot));
    }

    /// `CLEARUNITS`: slots 1..=10 of both sides.
    pub fn script_clear_units(&mut self) {
        self.units.retain(|u| u.slot == 0);
    }

    /// `TELEPORT(g, slot) -> (c, r)` 0x1E0FD0: move the unit; an occupant
    /// of the target is deleted [F]. An empty source slot is an error (the
    /// scripts never do that).
    pub fn script_teleport(&mut self, side: u8, slot: u8, cell: GridPos) -> Result<()> {
        if cell.index().is_none() {
            return Err(DuelError::InvalidCommand("script teleport off the board"));
        }
        let Some(i) = self
            .units
            .iter()
            .position(|u| u.side == side && u.slot == slot)
        else {
            return Err(DuelError::InvalidCommand(
                "script teleport of an empty slot",
            ));
        };
        let id = self.units[i].id;
        self.units.retain(|u| u.position != cell || u.id == id);
        let unit = self
            .units
            .iter_mut()
            .find(|u| u.id == id)
            .expect("kept above");
        unit.position = cell;
        Ok(())
    }

    /// `UFLAG(g, slot, fN = v)` 0x1E30F0 ([`unit_flag`]).
    pub fn script_unit_flag(&mut self, side: u8, slot: u8, field: u8, value: bool) -> Result<()> {
        let Some(u) = self
            .units
            .iter_mut()
            .find(|u| u.side == side && u.slot == slot)
        else {
            return Err(DuelError::InvalidCommand("script flag of an empty slot"));
        };
        match field {
            unit_flag::ACTED => u.acted = value,
            unit_flag::FACE_UP => u.face_up = value,
            unit_flag::DEFENCE => u.defence = value,
            unit_flag::COMMITTED_FACE_UP => {
                u.face_up = value;
                u.committed_face_up = value;
            }
            unit_flag::COMMITTED_DEFENCE => {
                u.defence = value;
                u.committed_defence = value;
            }
            _ => {
                return Err(DuelError::Unsupported(format!(
                    "script unit flag field {field} (+0x57 bit{field} has no engine field)"
                )))
            }
        }
        Ok(())
    }

    /// `TERR(c, r) = t` 0x1E34A0.
    pub fn script_set_terrain(&mut self, cell: GridPos, code: u8) -> Result<()> {
        let Some(i) = cell.index().filter(|_| code <= 9) else {
            return Err(DuelError::InvalidCommand("script terrain off the board"));
        };
        self.terrain[i] = code;
        Ok(())
    }

    /// `ROUND = v` 0x1DF7F0 (D53).
    pub fn script_set_round(&mut self, round: u8) {
        self.round = round;
    }

    /// `SUMMONOKs = v` 0x1DF980 (S+0x346 bit0).
    pub fn script_set_summon_ok(&mut self, side: u8, on: bool) {
        self.sides[usize::from(side)].can_summon = on;
    }

    /// `SPELLBIND(g, slot, n)` 0x251CD0: +0x49 := n (0xFF eternal), with
    /// 0x1E26A0(g, slot, 1) marking it fresh (+0x58 bit3 [H]) so the turn
    /// end does not shorten it.
    pub fn script_spellbind(&mut self, side: u8, slot: u8, turns: u8) -> Result<()> {
        let u = self.script_unit_mut(side, slot)?;
        u.spellbind = turns;
        u.spellbind_fresh = true;
        Ok(())
    }

    /// `SETBIND(g, slot, v)` 0x1E27A0: +0x49 := v only.
    pub fn script_set_bind(&mut self, side: u8, slot: u8, value: u8) -> Result<()> {
        self.script_unit_mut(side, slot)?.spellbind = value;
        Ok(())
    }

    fn script_unit_mut(&mut self, side: u8, slot: u8) -> Result<&mut super::DuelUnit> {
        self.units
            .iter_mut()
            .find(|u| u.side == side && u.slot == slot)
            .ok_or(DuelError::InvalidCommand("script edit of an empty slot"))
    }

    /// 0x1E02F0 + 0x1E03B0: the hand emptied, then `cards` added in order
    /// (at most five, 0x1E03B0 refuses a sixth) [F].
    pub fn script_set_hand(&mut self, side: u8, cards: &[u16]) {
        let hand = &mut self.sides[usize::from(side)].hand;
        hand.clear();
        hand.extend(cards.iter().take(5).copied());
    }

    /// 0x1E00F0(side, card): appended to the graveyard (at most 0x78
    /// entries; 0x29F is stored as 999) [F].
    pub fn script_graveyard_add(&mut self, side: u8, card: u16) {
        let graveyard = &mut self.sides[usize::from(side)].graveyard;
        if graveyard.len() < 0x78 {
            graveyard.push(if card == 0x29F { 999 } else { card });
        }
    }

    /// `DECKRESET` 0x176730 ([`DECK_RESET`]).
    pub fn script_deck_reset(&mut self) {
        for (side, cards) in self.sides.iter_mut().zip(DECK_RESET) {
            side.deck = cards.to_vec();
            side.hand.clear();
        }
    }

    /// With `NO_LP0_END` the LP-0 judge never runs (cursor frame 0x1D33CC
    /// requires D67 bit5 = 0): undo an end `Duel::apply` reached for that
    /// reason and drop its event.
    pub fn script_suppress_lp0_end(&mut self, events: &mut Vec<DuelEvent>) {
        if self
            .outcome
            .is_some_and(|o| o.reason == EndReason::LifePoints)
        {
            self.outcome = None;
            events.retain(|e| !matches!(e, DuelEvent::Ended { .. }));
        }
    }

    /// Exodia complete at the active side's turn start: a scripted side gets
    /// the event only (turn-verified.md §7 D); `Duel::start_turn` skips the
    /// win for scripted controllers.
    pub fn script_exodia_event(&self, cards: &impl CardData) -> bool {
        scripted(self.controllers[usize::from(self.active)])
            && self.exodia_complete(self.active, cards)
    }
}
