//! The end of a route and what Continue does after it (Claude, 4 Oct 2026).
//! Notes, disassembly, emulator logs and captures:
//! `work/research/story/claude-ending/NOTES.md`.
//!
//! Red route, map event 11184 (0x221584, the story dispatcher 0x21DD80) after
//! its 18 captions 2984..3028 [F, emu `ending-fresh`]:
//! - sub-step 2, when the text has closed (W+537B = 12): W+1C = 1, W+1A = 3
//!   (the letterbox 0x233750 grows its third band W+22 by 8 rows per update
//!   from 0 to 112: 144 → 256 rows each side, the screen is black after 14
//!   updates), **SAVE+0x3C = 0**;
//! - sub-step 3, when the letterbox reports W+1C = 2: on its first update
//!   (W+543E = 0) **SAVE[1] = 1**; when SAVE[2] is set too (both routes
//!   cleared) 0x2169C0(0, 1) sets bit 5 of the player's card block byte
//!   +0xFF9C; then the memory-card op 2 (0x1AB8F0(2)) starts and 0x1ABB20
//!   runs in the same update and every update after
//!   (`memory_card::CardDialog::ending_save`);
//! - when 0x1ABB20 returns −1 (the update after the save was acknowledged or
//!   refused): 0x1A8CD0(2, 1, 0) plays `MOVIE/EDMV_A6.PSS` to its end
//!   (table 0x28F190[2]; 0x242050 with a2 = 0: no button skips it), then
//!   0x312B90 = 1 (the map draws nothing), param[10] = 1 and the mode stack
//!   pops back to the title (mode 10, pushed by the title's New Game /
//!   Continue); mode 10's init skips the opening movie when param[10] ≠ 0
//!   (0x1A9B4C) and shows the title.
//! - Every update of 11184 also: W+64 ≥ 2001 → effect task 0x1F5B00(0, 402)
//!   once (W+5449), W+64 ≥ 3001 → overlay W+53A4 = 4 (no 2D output traced).
//!
//! There are no 2D credits: whatever the ending shows after the dialogue is
//! inside EDMV_A6 (120 s) [F: nothing else runs between the save and the
//! title]. The white route ends the same way at 0x222BF0..0x222D3C after
//! its 15 captions 3464..3508 (event 12156) [F, emu `claude-story-white`
//! trace-manawyddan: Cross f1466, text close f1467..f1471, letterbox 152
//! f1472 .. 256 f1485, op 2 f1486 — the red timing]: SAVE+0x3C = 0,
//! **SAVE[2] = 1**, block bit 5 when SAVE[1] is set too, op 2, then movie 3
//! `MOVIE/EDMV_C6.PSS` (a2 = 0) and **C2 = 0** (0x222CC8; the red tail
//! leaves C2 = 11184), title.
//!
//! Continue after an ending (story init 0x21BF50 with param[11] = 1, branch
//! 0x21C1A4) [F]: both route flags → the free-duel map (0x21C254, not ported);
//! SAVE+0x3C = 0 → 0x21C734: the other route's story from event 10090 (the
//! side choice comes next, its cursor W+53AA on the other side), node states
//! SAVE[3..0x23] cleared, C2 = 10090, SAVE+0x3C = 1.
use crate::campaign_map::CampaignSave;
use crate::memory_card::LoadedRecord;

/// The red route's last map event (its pages are `map-story-11184`).
pub const EVENT_RED_ENDING: u16 = 11184;
/// The white route's last map event (its pages are `map-story-12156`).
pub const EVENT_WHITE_ENDING: u16 = 12156;

/// Which route an ending closes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// 11184: SAVE[1] = 1, movie 2 (EDMV_A6), C2 kept.
    Red,
    /// 12156: SAVE[2] = 1, movie 3 (EDMV_C6), C2 = 0.
    White,
}

impl Route {
    /// The route whose ending map event `event` is.
    pub fn of_event(event: u16) -> Option<Self> {
        match event {
            EVENT_RED_ENDING => Some(Self::Red),
            EVENT_WHITE_ENDING => Some(Self::White),
            _ => None,
        }
    }
    pub fn event(self) -> u16 {
        match self {
            Self::Red => EVENT_RED_ENDING,
            Self::White => EVENT_WHITE_ENDING,
        }
    }
    pub fn movie(self) -> EndingMovie {
        match self {
            Self::Red => RED_MOVIE,
            Self::White => WHITE_MOVIE,
        }
    }
}
/// The story event Continue starts after an ending (0x21C7D8: C2 = 0x276A).
pub const EVENT_AFTER_ENDING: i16 = 10090;
/// The story stage of event 10090 and the side choice after it (story import).
pub const STAGE_AFTER_ENDING: &str = "seto-side-intro";
pub const SIDE_CHOICE_STAGE: &str = "side-choice";

/// A movie the original plays with 0x1A8CD0(index, 1, a2) ("MOVIE/%s" of
/// table 0x28F190).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EndingMovie {
    pub index: u8,
    /// Path on the disc.
    pub disc_path: &'static str,
    /// The story import's asset id of the copied PSS (`roses import-game`,
    /// `import-story-red`); imports made before 4 Oct 2026 have none.
    pub asset: &'static str,
    /// Where the story import keeps it (relative to its manifest).
    pub media_path: &'static str,
    /// 0x242050's a2 (0x313830): 0 = no button skips the movie.
    pub skip_mode: u8,
}

impl EndingMovie {
    pub const fn skippable(&self) -> bool {
        self.skip_mode != 0
    }
    /// Whether a new press at movie frame `frame` ends it (0x2421E0).
    pub fn skip_allowed(&self, frame: u64) -> bool {
        movie_skip_allowed(self.skip_mode, frame)
    }
}

/// 0x2421E0: new presses & 0x8E0 skip a movie only when 0x242050's a2 ≠ 0
/// and the frame counter is ≥ 11; a2 = 2 (the title's attract movie) also
/// needs frame ≥ 250 [F].
pub fn movie_skip_allowed(a2: u8, frame: u64) -> bool {
    a2 != 0 && frame >= 11 && (a2 != 2 || frame >= 250)
}

/// 0x1A8CD0(2, 1, 0) at the end of 11184.
pub const RED_MOVIE: EndingMovie = EndingMovie {
    index: 2,
    disc_path: "MOVIE/EDMV_A6.PSS",
    asset: "ending-movie-red",
    media_path: "media/ending-red.pss",
    skip_mode: 0,
};
/// 0x1A8CD0(3, 1, 0) at 0x222CA4 (the end of 12156).
pub const WHITE_MOVIE: EndingMovie = EndingMovie {
    index: 3,
    disc_path: "MOVIE/EDMV_C6.PSS",
    asset: "ending-movie-white",
    media_path: "media/ending-white.pss",
    skip_mode: 0,
};
/// Both, in the order the story import stores them.
pub const ENDING_MOVIES: [EndingMovie; 2] = [RED_MOVIE, WHITE_MOVIE];

/// The imported PSS of `movie` in a loaded story manifest, or why there is
/// none (imports made before the ending movies were imported).
pub fn movie_path(loaded: &crate::Loaded, movie: EndingMovie) -> Result<std::path::PathBuf, String> {
    let asset = loaded.assets.get(movie.asset).ok_or_else(|| {
        format!(
            "this import has no {} (asset {}; re-import the game to get the ending movies)",
            movie.disc_path, movie.asset
        )
    })?;
    if !asset.path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pss")) {
        return Err(format!("asset {} is not a PSS stream", movie.asset));
    }
    Ok(asset.path.clone())
}

/// What one update of [`MovieRun`] reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MovieTick {
    /// Still playing; `changed` = [`MovieRun::frame`] is new.
    Playing { changed: bool },
    /// Played to its end (or the stand-in was dismissed): call
    /// [`RouteEnding::movie_finished`].
    Ended,
}

/// The ending movie as the frontend plays it (Bevy-independent): the
/// disc's PSS through the built-in decoder (`movie_native`), sound through
/// the game's mixer (none when muted: the clock is wall time), presses
/// honoured only as 0x2421E0 allows (never for the endings: a2 = 0). With
/// no imported movie a stand-in shows black until a press (logged).
pub struct MovieRun {
    pub movie: EndingMovie,
    player: Option<crate::movie_native::NativeMovie>,
    /// Why the stand-in shows instead of the movie.
    pub stand_in: Option<String>,
    started: bool,
}

impl MovieRun {
    /// `path` from [`movie_path`]; `mixer` = the open output's mixer.
    pub fn open(
        movie: EndingMovie,
        path: Result<std::path::PathBuf, String>,
        mixer: Option<std::sync::Arc<std::sync::Mutex<crate::sound_mix::Mixer>>>,
    ) -> Self {
        let opened = path.and_then(|p| crate::movie_native::NativeMovie::open(&p, mixer));
        match opened {
            Ok(player) => Self { movie, player: Some(player), stand_in: None, started: false },
            Err(reason) => Self { movie, player: None, stand_in: Some(reason), started: false },
        }
    }

    pub fn is_native(&self) -> bool {
        self.player.is_some()
    }

    /// The frame to show now (native playback).
    pub fn frame(&self) -> Option<&crate::movie_native::MovieFrame> {
        self.player.as_ref().and_then(|p| p.frame())
    }

    /// One update. `pressed` = a new face-button press this update.
    pub fn update(&mut self, pressed: bool) -> MovieTick {
        let Some(player) = self.player.as_mut() else {
            // Stand-in only: the original returns when the movie ends.
            return if pressed { MovieTick::Ended } else { MovieTick::Playing { changed: false } };
        };
        match player.update() {
            Ok((crate::movie_native::MovieState::Ended, _)) => {
                self.player = None;
                MovieTick::Ended
            }
            Ok((state, changed)) => {
                self.started |= state == crate::movie_native::MovieState::Playing;
                let frame = player.frame().map_or(0, |f| f.index);
                if pressed && self.started && self.movie.skip_allowed(frame) {
                    self.player = None;
                    return MovieTick::Ended;
                }
                MovieTick::Playing { changed }
            }
            Err(e) => {
                // Mid-play decode failure: black stand-in, as a missing movie.
                self.player = None;
                self.stand_in = Some(format!("{} failed: {e}", self.movie.disc_path));
                MovieTick::Playing { changed: false }
            }
        }
    }
}

/// Updates from the Cross on caption 3028 until sub-step 2 starts closing
/// the letterbox (the text window's close, W+537B 5 → 11 → 12) [emu f960..f964].
pub const TEXT_CLOSE_UPDATES: u32 = 5;
/// Letterbox rows each side while the 11184 captions show (W+1E + W+20).
pub const LETTERBOX_OPEN_ROWS: u32 = 144;
/// Closed: W+1E + W+20 + W+22 = 72 + 72 + 112.
pub const LETTERBOX_CLOSED_ROWS: u32 = 256;
/// W+22 grows 8 rows per update in letterbox mode 3 [F 0x233A3C.., emu].
pub const LETTERBOX_STEP: u32 = 8;

/// The card block's flag byte +0xFF9C inside `CardBlock::raw_tail` (the
/// tail is +0xFD92..+0xFE90, +0xFEA0..+0xFF98, +0xFF9C..+0xFFA0).
pub(crate) const BLOCK_FLAGS_TAIL_OFFSET: usize = (0xFE90 - 0xFD92) + (0xFF98 - 0xFEA0);
/// 0x2169C0(slot, 1): bit 5 of +0xFF9C, set when the second route ends
/// (who reads it is not traced).
pub const BOTH_ROUTES_BLOCK_BIT: u8 = 0x20;

/// 0x2169C0(0, 1) on the player's block (both routes cleared).
pub fn mark_both_routes(block: &mut roses_core::card_block::CardBlock) {
    if let Some(b) = block.raw_tail.get_mut(BLOCK_FLAGS_TAIL_OFFSET) {
        *b |= BOTH_ROUTES_BLOCK_BIT;
    }
}

pub fn both_routes_marked(block: &roses_core::card_block::CardBlock) -> bool {
    block
        .raw_tail
        .get(BLOCK_FLAGS_TAIL_OFFSET)
        .is_some_and(|b| b & BOTH_ROUTES_BLOCK_BIT != 0)
}

/// The two black bands of `rows` raw rows each on the 640×448 logical
/// canvas: (y, height) of the top and the bottom band (raw 512 rows map to
/// 448 as the story import's letterbox does).
pub fn letterbox_bands(rows: u32) -> [(u32, u32); 2] {
    let rows = rows.min(256);
    let canvas_y = |y: u32| (y * 7 + 4) / 8;
    let top = canvas_y(rows);
    let bottom = canvas_y(512 - rows);
    [(0, top), (bottom, 448 - bottom)]
}

/// Where the ending is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EndingPhase {
    /// The last caption's window closes (letterbox still 144 rows).
    TextClose { left: u32 },
    /// Letterbox mode 3: `rows` each side, +8 per update up to 256.
    Closing { rows: u32 },
    /// Sub-step 3: op 2 runs (the frontend owns the `CardDialog`).
    Save,
    /// The movie plays (the frontend plays it or a stand-in), then the title.
    Movie(EndingMovie),
    /// Back at the title (mode 10 with param[10] = 1: no opening movie).
    Title,
}

/// What an update asks of the frontend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EndingStep {
    Running,
    /// Sub-step 3's first update: SAVE is final (`RouteEnding::save`);
    /// build the file, start `CardDialog::ending_save` and update it once in
    /// this same update.
    StartSave,
    /// 0x1ABB20 returned −1: play this movie, then call
    /// [`RouteEnding::movie_finished`].
    PlayMovie(EndingMovie),
}

/// The ending of a route from the Cross on its last caption (3028 red,
/// 3508 white) on.
#[derive(Clone, Debug)]
pub struct RouteEnding {
    pub route: Route,
    /// SAVE as the ending leaves it (after `apply`).
    pub save: CampaignSave,
    /// SAVE[2] (the white route was cleared): the block flag; the white
    /// ending sets it at sub-step 3.
    pub white_route_cleared: bool,
    pub phase: EndingPhase,
    pub movie: EndingMovie,
    /// Updates since the Cross.
    pub updates: u32,
}

impl RouteEnding {
    /// The red ending; `save` = SAVE at event 11184 (C2 = 11184, C9 = 1),
    /// `white_route_cleared` = SAVE[2].
    pub fn red(save: CampaignSave, white_route_cleared: bool) -> Self {
        Self {
            route: Route::Red,
            save,
            white_route_cleared,
            phase: EndingPhase::TextClose { left: TEXT_CLOSE_UPDATES },
            movie: RED_MOVIE,
            updates: 0,
        }
    }

    /// The white ending; `save` = SAVE at event 12156 (SAVE[1] in
    /// `save.red_route_cleared` decides the block flag),
    /// `white_route_cleared` = SAVE[2] before (0 on a first clear).
    pub fn white(save: CampaignSave, white_route_cleared: bool) -> Self {
        Self {
            route: Route::White,
            movie: WHITE_MOVIE,
            ..Self::red(save, white_route_cleared)
        }
    }

    /// SAVE bytes the port's `CampaignSave` does not hold, as the ending
    /// leaves them: SAVE[2] (`base` = the SAVE last loaded or saved, or
    /// zeros). Pass the result as `memory_card::save_record`'s base.
    pub fn record_base(&self, base: Option<&[u8; roses_core::memory_card::SAVE_BYTES]>) -> [u8; roses_core::memory_card::SAVE_BYTES] {
        let mut b = base.copied().unwrap_or([0; roses_core::memory_card::SAVE_BYTES]);
        b[2] = u8::from(self.white_route_cleared);
        b
    }

    /// Whether 0x2169C0(0, 1) marks the card block before the save.
    pub fn marks_block(&self) -> bool {
        self.save.red_route_cleared && self.white_route_cleared
    }

    /// Letterbox rows each side this update (`None` once the dialog runs
    /// over the closed screen and after).
    pub fn letterbox_rows(&self) -> u32 {
        match self.phase {
            EndingPhase::TextClose { .. } => LETTERBOX_OPEN_ROWS,
            EndingPhase::Closing { rows } => rows,
            _ => LETTERBOX_CLOSED_ROWS,
        }
    }

    /// One update (25 Hz) before the dialog and between it and the movie.
    /// `dialog_closed` = 0x1ABB20 returned −1 this update (only read in
    /// `Save`).
    pub fn update(&mut self, dialog_closed: bool) -> EndingStep {
        self.updates += 1;
        match self.phase.clone() {
            EndingPhase::TextClose { left } if left > 0 => {
                self.phase = EndingPhase::TextClose { left: left - 1 };
                EndingStep::Running
            }
            EndingPhase::TextClose { .. } => {
                // Sub-step 2: SAVE+0x3C = 0, letterbox mode 3 (the band
                // grows in the same update).
                self.save.map_entered = false;
                self.phase = EndingPhase::Closing {
                    rows: LETTERBOX_OPEN_ROWS + LETTERBOX_STEP,
                };
                EndingStep::Running
            }
            EndingPhase::Closing { rows } if rows < LETTERBOX_CLOSED_ROWS => {
                self.phase = EndingPhase::Closing {
                    rows: (rows + LETTERBOX_STEP).min(LETTERBOX_CLOSED_ROWS),
                };
                EndingStep::Running
            }
            EndingPhase::Closing { .. } => {
                // Sub-step 3, first update: SAVE[1] = 1 (red) or
                // SAVE[2] = 1 (white, 0x222C44), then op 2.
                self.save.map_entered = false;
                match self.route {
                    Route::Red => self.save.red_route_cleared = true,
                    Route::White => self.white_route_cleared = true,
                }
                self.phase = EndingPhase::Save;
                EndingStep::StartSave
            }
            EndingPhase::Save if dialog_closed => {
                // White: C2 = 0 after the movie call (0x222CC8).
                if self.route == Route::White {
                    self.save.event = 0;
                }
                self.phase = EndingPhase::Movie(self.movie);
                EndingStep::PlayMovie(self.movie)
            }
            EndingPhase::Save | EndingPhase::Movie(_) | EndingPhase::Title => EndingStep::Running,
        }
    }

    /// The movie ended (or its stand-in was dismissed): 0x312B90 = 1,
    /// param[10] = 1, pop to the title.
    pub fn movie_finished(&mut self) {
        if matches!(self.phase, EndingPhase::Movie(_)) {
            self.phase = EndingPhase::Title;
        }
    }
}

/// Where Continue goes with a loaded save (0x21BF50(1) → 0x21C1A4) [F].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContinueTarget {
    /// SAVE+0x3C ≠ 0, not both routes: the map at the saved event (0x21C440).
    Map,
    /// SAVE[1] and SAVE[2] (W+18C = 1, 0x21C254): every node state 3, all
    /// paths drawn, C2 = 0, track 4 — the free-duel map
    /// (`campaign_map::MapModel::resume` → `free_duel`).
    FreeDuel,
    /// SAVE+0x3C = 0 (0x21C734): the other route's story at event 10090
    /// with `save` (SAVE after the switch) and the side choice's cursor
    /// W+53AA (`side_cursor`: 0 = Yorkists row, 1 = Lancastrians row).
    OtherRoute { save: CampaignSave, white_route_cleared: bool, side_cursor: u8 },
}

/// 0x21C1A4's choice for a loaded save (the 0x312B88 switch, set by the
/// practice/duel-count code 0x23B590 and 0x1CFF80, is RAM-only and false
/// after a load).
pub fn continue_target(record: &LoadedRecord) -> ContinueTarget {
    let map = &record.map;
    if map.red_route_cleared && record.white_route_cleared {
        return ContinueTarget::FreeDuel;
    }
    if map.map_entered {
        return ContinueTarget::Map;
    }
    let mut save = map.clone();
    let mut white = record.white_route_cleared;
    let mut side_cursor = 1;
    if save.red_route_cleared {
        // 0x21C75C: SAVE[0] = 1, SAVE[1] = 1, SAVE[2] = 0, W+18C = 0,
        // W+53AA = 0, C9 = 0.
        save.side = 1;
        white = false;
        side_cursor = 0;
        save.c9 = 0;
    } else if white {
        // 0x21C790: SAVE[0] = 0, SAVE[1] = 0, SAVE[2] = 1, W+53AA = 1, C9 = 2.
        save.side = 0;
        save.red_route_cleared = false;
        side_cursor = 1;
        save.c9 = 2;
    }
    save.white_route_cleared = white;
    // SAVE[3..0x23] = 0 (the port models SAVE[3..0x19]); C2 = 10090;
    // SAVE+0x3C = 1.
    save.node_states.iter_mut().for_each(|s| *s = 0);
    save.event = EVENT_AFTER_ENDING;
    save.map_entered = true;
    ContinueTarget::OtherRoute {
        save,
        white_route_cleared: white,
        side_cursor,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::campaign_map::{STATE_BEATEN, STATE_BEATEN_SECOND};
    use crate::memory_card::{load_record, save_record};

    fn at_11184() -> CampaignSave {
        let mut s = CampaignSave::after_landing();
        for n in 2..=10 {
            s.node_states[n] = STATE_BEATEN;
        }
        s.node_states[11] = STATE_BEATEN_SECOND;
        s.event = EVENT_RED_ENDING as i16;
        s.opponent = 20;
        s.hovered = 11;
        s.location = 11;
        s.destination = 11;
        s.c9 = 1;
        s
    }

    /// The emulated original from the Cross on 3028 (f959): text close
    /// f960..f964, letterbox 152 at f965 (SAVE+0x3C = 0) .. 256 at f978,
    /// "Save?" at f979 with SAVE[1] = 1 [emu ending-fresh].
    #[test]
    fn the_red_ending_closes_the_letterbox_then_saves() {
        let mut e = RouteEnding::red(at_11184(), false);
        let mut rows = Vec::new();
        let mut start = None;
        for u in 1..=30 {
            let step = e.update(false);
            rows.push(e.letterbox_rows());
            if step == EndingStep::StartSave {
                start = Some(u);
                break;
            }
            if u == 6 {
                assert!(!e.save.map_entered, "SAVE+0x3C = 0 with the first band step");
                assert!(!e.save.red_route_cleared);
            }
        }
        assert_eq!(start, Some(20));
        assert_eq!(&rows[..5], &[144; 5]);
        assert_eq!(rows[5], 152);
        assert_eq!(rows[18], 256);
        assert!(e.save.red_route_cleared && !e.save.map_entered);
        assert_eq!(e.save.event, 11184);
        assert!(!e.marks_block());
        // The dialog runs until 0x1ABB20 returns −1; then the movie.
        assert_eq!(e.update(false), EndingStep::Running);
        assert_eq!(e.update(true), EndingStep::PlayMovie(RED_MOVIE));
        assert_eq!(RED_MOVIE.disc_path, "MOVIE/EDMV_A6.PSS");
        assert!(!RED_MOVIE.skippable());
        e.movie_finished();
        assert_eq!(e.phase, EndingPhase::Title);
        // Both routes: the block flag.
        let both = RouteEnding::red(at_11184(), true);
        let mut both = both;
        while both.update(false) != EndingStep::StartSave {}
        assert!(both.marks_block());
    }

    fn at_12156() -> CampaignSave {
        let mut s = CampaignSave::white_start();
        for n in [0usize, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21] {
            s.node_states[n] = STATE_BEATEN;
        }
        s.event = EVENT_WHITE_ENDING as i16;
        s.opponent = 21;
        s.location = 11;
        s
    }

    /// The emulated white tail [emu claude-story-white trace-manawyddan]:
    /// Cross on 3508 at f1466, text close f1467..f1471, letterbox 152 at
    /// f1472 .. 256 at f1485, op 2 at f1486 (update 20), then movie 3 and
    /// C2 = 0 in the update the dialog returns −1.
    #[test]
    fn the_white_ending_saves_save2_then_plays_edmv_c6() {
        let mut e = RouteEnding::white(at_12156(), false);
        let mut start = None;
        let mut rows = Vec::new();
        for u in 1..=30 {
            let step = e.update(false);
            rows.push(e.letterbox_rows());
            if step == EndingStep::StartSave {
                start = Some(u);
                break;
            }
        }
        assert_eq!(start, Some(20));
        assert_eq!((rows[5], rows[18]), (152, 256));
        assert!(e.white_route_cleared && !e.save.red_route_cleared && !e.save.map_entered);
        assert_eq!(e.save.event, 12156, "C2 is still 12156 in the saved record");
        assert!(!e.marks_block());
        let base = e.record_base(None);
        let s = crate::memory_card::save_record(Some(&base), &e.save, &[]);
        assert_eq!((s[0], s[1], s[2]), (1, 0, 1));
        assert_eq!(u16::from_le_bytes([s[0x3C], s[0x3D]]), 0);
        assert_eq!(e.update(true), EndingStep::PlayMovie(WHITE_MOVIE));
        assert_eq!(e.save.event, 0);
        e.movie_finished();
        assert_eq!(e.phase, EndingPhase::Title);
        // Red cleared before: the block flag.
        let mut both = at_12156();
        both.red_route_cleared = true;
        let mut both = RouteEnding::white(both, false);
        while both.update(false) != EndingStep::StartSave {}
        assert!(both.marks_block());
        assert_eq!(Route::of_event(12156), Some(Route::White));
        assert_eq!(Route::of_event(11184), Some(Route::Red));
        assert_eq!(Route::White.movie(), WHITE_MOVIE);
        // Continue after it: the red route from 10090, cursor on the
        // Lancastrians, C9 = 2 (0x21C790).
        let record = load_record(&s);
        match continue_target(&record) {
            ContinueTarget::OtherRoute { save, white_route_cleared, side_cursor } => {
                assert_eq!((save.side, save.c9, side_cursor), (0, 2, 1));
                assert!(white_route_cleared && !save.red_route_cleared);
                assert_eq!(save.event, EVENT_AFTER_ENDING);
            }
            other => panic!("{other:?}"),
        }
        // The red ending keeps SAVE[2] as the card had it.
        let mut red = RouteEnding::red(at_11184(), true);
        while red.update(false) != EndingStep::StartSave {}
        assert_eq!(red.record_base(None)[2], 1);
    }

    #[test]
    fn the_ending_save_record_matches_the_original_file() {
        // The file the original wrote at the ending [emu ending-fresh]:
        // SAVE[1] = 1, SAVE+0x3C = 0, C2 = 11184, node 11 = 5, C9 = 1.
        let mut e = RouteEnding::red(at_11184(), false);
        while e.update(false) != EndingStep::StartSave {}
        let s = save_record(None, &e.save, &[]);
        assert_eq!((s[0], s[1], s[2]), (0, 1, 0));
        assert_eq!(u16::from_le_bytes([s[0x3C], s[0x3D]]), 0);
        assert_eq!(u16::from_le_bytes([s[0xC2], s[0xC3]]), 11184);
        assert_eq!(s[3 + 11], 5);
        let back = load_record(&s);
        assert!(!back.map_entered && !back.map.map_entered && back.map.red_route_cleared);
    }

    #[test]
    fn continue_after_the_red_ending_starts_the_other_route() {
        let mut e = RouteEnding::red(at_11184(), false);
        while e.update(false) != EndingStep::StartSave {}
        let record = load_record(&save_record(None, &e.save, &[0x2005; 12]));
        match continue_target(&record) {
            ContinueTarget::OtherRoute { save, white_route_cleared, side_cursor } => {
                assert_eq!(save.side, 1);
                assert!(save.red_route_cleared && !white_route_cleared);
                assert_eq!(side_cursor, 0);
                assert_eq!(save.c9, 0);
                assert_eq!(save.event, EVENT_AFTER_ENDING);
                assert!(save.node_states.iter().all(|s| *s == 0));
                assert!(save.map_entered);
                assert_eq!((save.wins, save.location), (e.save.wins, 11));
            }
            other => panic!("{other:?}"),
        }
        // A map save resumes the map; both routes give the free-duel map.
        let mut map = record.clone();
        map.map.map_entered = true;
        assert_eq!(continue_target(&map), ContinueTarget::Map);
        let mut both = record;
        both.white_route_cleared = true;
        assert_eq!(continue_target(&both), ContinueTarget::FreeDuel);
    }

    #[test]
    fn the_block_flag_is_bit_5_of_ff9c() {
        let mut block = roses_core::card_block::CardBlock::empty(1);
        assert!(!both_routes_marked(&block));
        mark_both_routes(&mut block);
        assert!(both_routes_marked(&block));
        assert_eq!(block.to_bytes().unwrap()[0xFF9C], 0x20);
    }

    /// 0x2421E0 with a2 = 0 (both endings): no press ever ends the movie;
    /// a2 = 1 from frame 11, a2 = 2 (attract) from frame 250 [F].
    #[test]
    fn the_ending_movies_cannot_be_skipped() {
        for movie in ENDING_MOVIES {
            assert_eq!(movie.skip_mode, 0);
            assert!(!movie.skippable());
            assert!((0..20_000).all(|f| !movie.skip_allowed(f)));
        }
        assert!(!movie_skip_allowed(1, 10) && movie_skip_allowed(1, 11));
        assert!(!movie_skip_allowed(2, 249) && movie_skip_allowed(2, 250));
        assert_eq!(WHITE_MOVIE.disc_path, "MOVIE/EDMV_C6.PSS");
        assert_eq!((RED_MOVIE.index, WHITE_MOVIE.index), (2, 3));
        assert_ne!(RED_MOVIE.asset, WHITE_MOVIE.asset);
        assert!(ENDING_MOVIES.iter().all(|m| m.media_path.ends_with(".pss")));
    }

    /// No imported movie (an import from before 4 Oct): the stand-in waits
    /// for a press and says why.
    #[test]
    fn a_missing_ending_movie_falls_back_to_the_stand_in() {
        let mut run = MovieRun::open(RED_MOVIE, Err("no asset".into()), None);
        assert!(!run.is_native());
        assert_eq!(run.stand_in.as_deref(), Some("no asset"));
        assert_eq!(run.update(false), MovieTick::Playing { changed: false });
        assert_eq!(run.update(true), MovieTick::Ended);
        let mut gone = MovieRun::open(RED_MOVIE, Ok("does/not/exist.pss".into()), None);
        assert!(gone.stand_in.is_some() && !gone.is_native());
        assert_eq!(gone.update(true), MovieTick::Ended);
    }

    /// Needs ROSES_PSS_DIR with DUMMY.PSS (11 s) from the user's disc: the
    /// native run ignores presses (a2 = 0) and ends with the movie. Muted:
    /// no mixer, wall clock; nothing is played.
    #[test]
    fn the_native_ending_movie_plays_to_its_end_despite_presses() {
        let Some(dir) = std::env::var_os("ROSES_PSS_DIR") else {
            eprintln!("ROSES_PSS_DIR not set; skipped");
            return;
        };
        let path = std::path::Path::new(&dir).join("DUMMY.PSS");
        let started = std::time::Instant::now();
        let mut run = MovieRun::open(RED_MOVIE, Ok(path), None);
        assert!(run.is_native(), "{:?}", run.stand_in);
        let mut last = 0;
        loop {
            match run.update(true) {
                MovieTick::Ended => break,
                MovieTick::Playing { .. } => {
                    assert!(run.stand_in.is_none(), "{:?}", run.stand_in);
                    last = run.frame().map_or(last, |f| f.index);
                }
            }
            assert!(started.elapsed().as_secs() < 120, "stalled");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(last, 329, "every press ignored up to the last frame");
        assert!(started.elapsed().as_secs_f64() >= 11.0);
    }

    #[test]
    fn letterbox_bands_cover_the_canvas_when_closed() {
        assert_eq!(letterbox_bands(256), [(0, 224), (224, 224)]);
        assert_eq!(letterbox_bands(144), [(0, 126), (322, 126)]);
    }
}
