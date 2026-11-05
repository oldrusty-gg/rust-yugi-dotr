//! The window parts of the Bosworth story events, played inside the map:
//! 11120 (the trigger), 11124 (the reveal) and 11149 (the farewell). The
//! original never leaves the map mode for these: the story dispatcher
//! 0x21DD80 opens the letterbox 0x233750 over the running map, shows the
//! centred window 0x232C80 (W+5487 = 1) and closes or reopens the letterbox
//! afterwards. The painting and dialogue parts (11122, 11140..11148) are
//! played by the frontend's story scene (`Handler::Story`).
//!
//! Sources (PAL SLES_515.79; private research
//! `story/the project notes, the project notes
//! §5, traces `trace-trigger.json`, the project notes
//! and `trace-win3.json` made with `tools/trace_story.py`):
//!
//! - Per frame the event's handler runs first (it sees the page phase of
//!   the previous frame), then the page machine (W+537B), then the draw
//!   pass steps the letterbox; the model's `sub`
//!   numbers its own steps (W+536C reads 1 throughout 11120's letterbox,
//!   then 2, 3) [emu: the state after each frame in the traces].
//! - 11120 [emu f221..f275]: the trigger frame keeps the map as it is; the
//!   next frame starts the letterbox in mode 2 (and music (28, 0) [F
//!   RED-ROUTE §5; H its frame]) and steps it: 18 updates to 72 + 72; the
//!   frame after it is done the window shows message 2648 with the page
//!   machine in phase 3 (no opening phase); 2648's two lines reveal in 13
//!   updates (phase 4), Cross gives phase 5 for 5 updates, then 11 and 12;
//!   the next frame removes the window and starts letterbox mode 3 (14
//!   updates to 72 + 72 + 112, the screen fully black); the frame after it
//!   is done C2 = 11122 and the story scene takes over.
//! - 11124 [emu f416..f499]: entered from 11122 with the letterbox closed
//!   (72, 72, 112, done). The first frame puts the camera target on
//!   Bosworth (node 7) in one step (eye and target change together at
//!   f417; there is no interpolation: the "pan" happens behind the closed
//!   letterbox), clears node 7's rise flag and starts letterbox mode 2 (14
//!   updates back to 144 rows); then window 2656 (one line, 7 updates),
//!   Cross, close; music (1, 0) [F], letterbox mode 0 (9 updates to 0);
//!   the frame after it is done paths 12, 13, 11 are set to their forward
//!   state; C2 = 0 on the frame after Bosworth's rise flag is set.
//! - 11149 [emu f1750..f1787]: entered from 11148 with the 144-row
//!   letterbox (72, 72, 0, done). The first frame shows the window with
//!   message 2828 (0x232C74 hard-codes 2648/2656/2828 for 11120/11124/
//!   11149) while the page machine runs on message 2804 (W+5378: three
//!   lines, 19 updates); after the close music (4, 0) [F] and letterbox
//!   mode 0 (9 updates); the frame after it is done C2 = 108 (the reveal of
//!   node 7's list, whose own first update is the next frame).
//! - The page machine of these windows has one page: the counter steps
//!   over the blank messages after it in the same frame and the window
//!   closes (msg 2648 → 2651, 2656 → 2659, 2804 → 2807 at phase 11) [emu].
//! - The window (style 0 through 0x218E70: black fill GS alpha 64 inset 2,
//!   TEXSYS #7 frame at (64, 0) 16×14) is w = text width + 12 (480 at most,
//!   then x = 16), h = text height + 12 (22 per line), x = 256 − w/2,
//!   y = 256; the composed text is drawn whole at (x + 6, 262) from the
//!   first frame through phase 12 (the same 0x246B90 call every frame); the
//!   advance mark (slot 1 (0, 112) 16×16) at (472, 472) in phase 4 [emu].
//!   The map HUD is off screen during these events (W+70 = 0: the panels
//!   are drawn 320 rows down) and the cursor is not drawn [emu].
//!
//! [H] / not traced: how long the advance mark stays in place during a
//! long wait (the tracer pressed Cross on the next frame), the frame of
//! 11120's music request, and a save taken in the middle of these events
//! (resuming restarts the event at its first frame).
use super::*;
use super::sail::{LETTERBOX_BAR, LETTERBOX_THIRD};

pub const EVENT_BOSWORTH_PAINTINGS: i16 = 11122;
pub const EVENT_BOSWORTH_FAREWELL: i16 = 11149;
/// The events whose window the map shows (0x232C74).
pub const WINDOW_EVENTS: [i16; 3] = [
    EVENT_BOSWORTH_TRIGGER,
    EVENT_BOSWORTH_REVEAL,
    EVENT_BOSWORTH_FAREWELL,
];
/// (event, message drawn in the window (0x232C74), page counter W+5378
/// the page machine runs on) [F 0x232C74, emu W+5378].
pub const WINDOW_MESSAGES: [(i16, u16, u16); 3] = [
    (EVENT_BOSWORTH_TRIGGER, 2648, 2648),
    (EVENT_BOSWORTH_REVEAL, 2656, 2656),
    (EVENT_BOSWORTH_FAREWELL, 2828, 2804),
];
/// Every message these windows read (for the import and the frontend).
pub const WINDOW_MESSAGE_IDS: [u16; 4] = [2648, 2656, 2804, 2828];
/// Music of the events (0x217110(track, keep)) [F RED-ROUTE §5].
pub const MUSIC_TRIGGER: u16 = 28;
pub const MUSIC_REVEAL_CLOSE: u16 = 1;
pub const MUSIC_FAREWELL_CLOSE: u16 = 4;
/// Window geometry of 0x232C80 (raw 512×512 space).
pub const WINDOW_Y: i32 = 256;
pub const WINDOW_PAD: i32 = 12;
pub const WINDOW_MAX: i32 = 480;
pub const WINDOW_MAX_X: i32 = 16;
pub const WINDOW_TEXT_DX: i32 = 6;
pub const WINDOW_TEXT_Y: i32 = 262;
pub const GLYPH_WIDTH: i32 = 12;
pub const LINE_HEIGHT: i32 = 22;
/// The advance mark of these windows (0x22E888's caller in the story draw).
pub const ADVANCE_MARK: (i32, i32) = (472, 472);
/// Letterbox counters on entry: 11124 after 11122 (closed), 11149 after
/// 11148 (144 rows) [emu f416, f1750].
pub const LETTERBOX_CLOSED: [i32; 3] = [LETTERBOX_BAR, LETTERBOX_BAR, LETTERBOX_THIRD];
pub const LETTERBOX_OPEN: [i32; 3] = [LETTERBOX_BAR, LETTERBOX_BAR, 0];

/// The centred story window (W+5487 = 1) and its page machine (W+537B).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StoryWindow {
    pub event: i16,
    /// The message drawn (0x232C74).
    pub message: u16,
    /// W+5378: the page the machine runs on (its line count times the
    /// reveal).
    pub counter: u16,
    /// 3 reveal, 4 wait for Cross, 5 page turn, 11, 12 closed.
    pub phase: u8,
    pub timer: u32,
    pub reveal_frames: u32,
}

/// The window rectangle (x, y, w, h) of 0x232CE4..0x232D38 for a message
/// `width` texels wide and `lines` lines high.
pub fn window_rect(width: i32, lines: i32) -> [i32; 4] {
    let w = width + WINDOW_PAD;
    let h = LINE_HEIGHT * lines.max(1) + WINDOW_PAD;
    if w > WINDOW_MAX {
        [WINDOW_MAX_X, WINDOW_Y, WINDOW_MAX, h]
    } else {
        [256 - w / 2, WINDOW_Y, w, h]
    }
}

impl MessageInfo {
    /// Message facts from the composed codes of the user's message file
    /// (0x2000 separates lines, 0x2001 first means blank); the text is not
    /// transcribed ('?' per glyph).
    pub fn from_codes(codes: &[u16]) -> Self {
        let separator = |c: &u16| c & 0x7FFF == 0x2000;
        let text = codes
            .split(separator)
            .map(|line| "?".repeat(line.len()))
            .collect::<Vec<_>>()
            .join("\n");
        Self {
            blank: codes.first().is_some_and(|c| c & 0x3FFF == BLANK_GLYPH),
            glyphs: codes.iter().filter(|c| !separator(c)).count().min(usize::from(u16::MAX)) as u16,
            lines: codes.split(separator).count().clamp(1, 255) as u8,
            text,
        }
    }

    /// Glyphs of the longest line (the composed width is 12 per glyph).
    pub fn widest_line(&self) -> u16 {
        self.text
            .split('\n')
            .map(|l| l.chars().count())
            .max()
            .unwrap_or(0)
            .min(usize::from(u16::MAX)) as u16
    }
}

impl MapModel {
    /// Whether `event` is played by the map with its story window.
    pub fn is_window_event(event: i16) -> bool {
        WINDOW_EVENTS.contains(&event)
    }

    /// The window rectangle of the open story window, from the message
    /// facts (`None` without a window or without the message's facts).
    pub fn story_window_rect(&self) -> Option<[i32; 4]> {
        let w = self.story_window.as_ref()?;
        let info = self.data.messages.get(&w.message)?;
        Some(window_rect(
            GLYPH_WIDTH * i32::from(info.widest_line()),
            i32::from(info.lines),
        ))
    }

    /// C2 became 11120 inside 0x228410 (`bosworth_trigger`): the map keeps
    /// running and plays the event's window itself.
    pub(super) fn start_map_story(&mut self, event: i16, events: &mut Vec<MapEvent>) {
        self.save.event = event;
        self.lerp = None;
        self.handler = Handler::MapStory { event, sub: 0 };
        events.push(MapEvent::EventChanged(event));
    }

    /// The map re-entered at a window event (resume): the state of the
    /// frame on which C2 became the event.
    pub(super) fn resume_map_story(&mut self, event: i16) -> Handler {
        self.fade = FADE_FULL;
        self.w53c4 = self.save.c9;
        match event {
            EVENT_BOSWORTH_REVEAL => {
                self.letterbox = Letterbox {
                    active: 2,
                    mode: 3,
                    bars: LETTERBOX_CLOSED,
                };
            }
            EVENT_BOSWORTH_FAREWELL => {
                self.letterbox = Letterbox {
                    active: 2,
                    mode: 2,
                    bars: LETTERBOX_OPEN,
                };
                // Path 14 (Bosworth → Stonehenge) is set by 108 (f1803):
                // not drawn behind the window.
                self.undraw_unrevealed_paths(BOSWORTH_NODE);
            }
            _ => {}
        }
        Handler::MapStory { event, sub: 0 }
    }

    fn open_story_window(&mut self, event: i16, events: &mut Vec<MapEvent>) {
        let Some(&(_, message, counter)) = WINDOW_MESSAGES.iter().find(|(e, ..)| *e == event)
        else {
            return;
        };
        if !self.data.messages.contains_key(&counter) {
            self.note("a story window's page is not in the map import; its reveal is taken as one line");
        }
        self.story_window = Some(StoryWindow {
            event,
            message,
            counter,
            phase: 3,
            timer: 0,
            reveal_frames: self.reveal_frames(counter),
        });
        events.push(MapEvent::TextPage {
            message,
            page: 1,
        });
    }

    /// The page machine of the story window (before the handler).
    fn story_window_update(&mut self, pad: Pad, events: &mut Vec<MapEvent>) {
        let Some(w) = self.story_window.as_mut() else {
            return;
        };
        match w.phase {
            3 => {
                // The reveal counts its updates, then the next update
                // finds it complete [emu: 13 updates f240..f252, phase 4
                // at f253].
                if w.timer >= w.reveal_frames {
                    w.phase = 4;
                } else {
                    w.timer += 1;
                }
            }
            4 => {
                if pad.cross {
                    events.push(MapEvent::Sfx(SFX_PAGE));
                    w.phase = 5;
                    w.timer = 0;
                }
            }
            5 => {
                w.timer += 1;
                if w.timer >= PAGE_TURN_FRAMES {
                    w.phase = 11;
                }
            }
            11 => w.phase = 12,
            _ => {}
        }
    }

    /// One frame of a window event: page machine, handler, letterbox.
    pub(super) fn map_story_update(
        &mut self,
        event: i16,
        sub: u8,
        pad: Pad,
        events: &mut Vec<MapEvent>,
    ) {
        if Self::is_dialogue_event(event) {
            self.dialogue_update(event, sub, pad, events);
            return;
        }
        let mut sub = sub;
        match sub {
            0 => {
                match event {
                    EVENT_BOSWORTH_TRIGGER => {
                        events.push(MapEvent::Music {
                            track: MUSIC_TRIGGER,
                            keep: false,
                        });
                        self.letterbox.start(2);
                        sub = 1;
                    }
                    EVENT_BOSWORTH_REVEAL => {
                        // 11124 sub-step 0: camera target = node 7 (one
                        // step), W+52FA = 0, letterbox mode 2.
                        self.camera_target = self.node_position(BOSWORTH_NODE);
                        self.rises[usize::from(BOSWORTH_NODE)] = RiseState::default();
                        self.letterbox.start(2);
                        sub = 1;
                    }
                    _ => {
                        // 11149: the 144-row letterbox is already there.
                        self.open_story_window(event, events);
                        sub = 2;
                    }
                }
            }
            1 => {
                if self.letterbox.active == 2 {
                    self.open_story_window(event, events);
                    sub = 2;
                }
            }
            2 => {
                if self.story_window.as_ref().is_some_and(|w| w.phase >= 12) {
                    self.story_window = None;
                    events.push(MapEvent::TextClosed);
                    match event {
                        EVENT_BOSWORTH_TRIGGER => self.letterbox.start(3),
                        EVENT_BOSWORTH_REVEAL => {
                            events.push(MapEvent::Music {
                                track: MUSIC_REVEAL_CLOSE,
                                keep: false,
                            });
                            self.letterbox.start(0);
                        }
                        _ => {
                            events.push(MapEvent::Music {
                                track: MUSIC_FAREWELL_CLOSE,
                                keep: false,
                            });
                            self.letterbox.start(0);
                        }
                    }
                    sub = 3;
                }
            }
            3 => {
                if self.letterbox.active == 2 {
                    match event {
                        EVENT_BOSWORTH_TRIGGER => {
                            // C2 = 11122: the paintings play in the story scene.
                            self.start_story(EVENT_BOSWORTH_PAINTINGS, events);
                            return;
                        }
                        EVENT_BOSWORTH_REVEAL => {
                            // 11124 sub-step 3: paths 12, 13, 11 forward.
                            for p in BOSWORTH_PATHS {
                                let state = &mut self.paths[usize::from(p)];
                                if state.state == 0 {
                                    state.state = 1;
                                    state.frames = 0;
                                    events.push(MapEvent::PathRevealing(p));
                                }
                            }
                            sub = 4;
                        }
                        _ => {
                            // C2 = 108: node 7's reveal (its first update
                            // is the next frame).
                            self.save.event = EVENT_STONEHENGE_REVEAL;
                            events.push(MapEvent::EventChanged(EVENT_STONEHENGE_REVEAL));
                            self.handler = self.reveal_event_handler(EVENT_STONEHENGE_REVEAL);
                            return;
                        }
                    }
                }
            }
            _ => {
                // 11124 sub-step 4: C2 = 0 once Bosworth's rise flag is set
                // (by the path updater, after this handler).
                if self.rises[usize::from(BOSWORTH_NODE)].rising
                    || self.save.state(BOSWORTH_NODE) != STATE_HIDDEN
                {
                    self.save.event = 0;
                    events.push(MapEvent::EventChanged(0));
                    self.handler = Handler::Free;
                    return;
                }
            }
        }
        self.handler = Handler::MapStory { event, sub };
        self.story_window_update(pad, events);
    }

    /// The draw pass's letterbox during a window event (every frame while
    /// it animates [emu]).
    pub(super) fn map_story_draw_update(&mut self) {
        // (Not after the handler moved on to a voyage this frame: the
        // voyage's own draw pass steps it, `sail_draw_update`.)
        if self.letterbox.active == 1 && matches!(self.handler, Handler::MapStory { .. }) {
            self.letterbox.step();
        }
    }
}

// ------------------------------------------------------------ dialogue
//
// 12040 (Dover) and 12041 (the Strait): Mako's pages over the running map
// (bottom speaker panel 0x232EF0..0x233650 with W+53BA, face slot 0x247310,
// page machine W+537B). Sources [emu `story/the project notes
// trace-dover.json`, made with `tools/trace_white.py dover`; F the panel
// draw 0x2332B0..0x233628]:
//
// - 12040 f1..f18: letterbox mode 2 from the first update (+8 per update,
//   18 updates to 72 + 72) and music (35, 1); 52 map models drawn behind.
// - f19 (the frame after the letterbox is done): the bottom panel opens
//   (W+53BA += 64 per update, f19 = 64 .. f34 = 1024) with page 3140 in
//   phase 2; phase 3 from f34; the page box W+5439 rises +10 per update
//   f35..f44; the reveal counts from f43 (first page after the slide-in:
//   8 updates of hold [emu fit]) and the page waits (phase 4) at f62;
//   the face talks (kind 3) from the update after the reveal's first count
//   through phase 4. Cross: phase 5 for 5 updates, 11, 12 (f69).
// - f70: C2 = 104 with the panel still up (W+53BA 1024, face kind 1, no
//   text): the voyage runs under Mako's panel; f190 C2 = 12041.
// - 12041 f191: page 3144 in phase 3 at once (the panel is up); pages
//   3144 3145 3148 3152 3156 3160 3161; between pages phase 5 (5 updates),
//   phase 2 (1 update), phase 3. After 3161: 11 (f342), 12 (f343); f344
//   the close phase 13: W+5439 100 -> 0 (-10 per update, f344..f353), then
//   W+53BA -64 per update from f353 (960) to 0 at f368 (phase 14, the face
//   slot freed, nothing drawn); f369 the tail (node 16's rise, sub-step 3).
// - Panel geometry (W+53BA = s): x offset s3 = 512 - s/2 (the emblem
//   0x297500 + 38*sel at (s3 + 120, 352), the name bar = window 0x296F20
//   with x1 = 384 at (s3 + 120, 352), the name W+53BE at (s3 + 173, 373)
//   squeezed to 324); y offset s0 = 256 - s/4 (the face window (0, 0, 104,
//   144, style 0) at (16, s0 + 368), the face slot at (16, s0 + 368), the
//   wings 0x297818/0x29783E (mode 2) at (-384, s0 + 216), the ornament
//   0x297780 + 38*sel at (-384, s0 + 208)); the page rows at (136, 400 +
//   22*r); the advance mark at (472, 472) in phase 4 [F, emu f40/f62].

/// The map-played dialogue events and their pages [emu trace-dover; the
/// pages also in `roses import-story-red` WHITE_EVENTS].
pub const DIALOGUE_EVENTS: [(i16, &[u16]); 2] = [
    (super::white::EVENT_DOVER_STORY, &[3140]),
    (
        super::white::EVENT_STRAIT_STORY,
        &[3144, 3145, 3148, 3152, 3156, 3160, 3161],
    ),
];
/// Mako: face slot speaker 15, name message 211 (W+53BE), selector 0
/// (W+53C1), mood 0 (W+546B) [emu trace-dover req_bot, F import].
pub const MAKO_SPEAKER: u8 = 15;
pub const MAKO_NAME_MESSAGE: u16 = 211;
pub const MAKO_SELECTOR: u8 = 0;
/// Music of 12040 at its first update (0x217110(35, 1)) [emu dover f0/f1].
pub const MUSIC_DOVER_DIALOGUE: u16 = 35;
/// W+53BA: the panel's slide (0 .. 1024, 64 per update).
pub const DIALOGUE_PANEL_FULL: i32 = 1024;
pub const DIALOGUE_PANEL_STEP: i32 = 64;
/// W+5439: the page box (0 .. 100, 10 per update).
pub const DIALOGUE_BOX_FULL: i32 = 100;
pub const DIALOGUE_BOX_STEP: i32 = 10;
/// The first page after the slide-in holds its reveal for 8 updates (the
/// page box opening; first count f43 after phase 3 at f34) [emu fit].
pub const DIALOGUE_FIRST_HOLD: u32 = 8;
/// Positions in raw 512x512 space [F 0x2332B0.., emu].
pub const DIALOGUE_TEXT_X: i32 = 136;
pub const DIALOGUE_TEXT_Y: i32 = 400;
pub const DIALOGUE_FACE_X: i16 = 16;
pub const DIALOGUE_FACE_Y: i32 = 368;

/// Every message the map's story windows and dialogues read (the import
/// lists them; the frontend fills older imports from the message file).
pub fn map_story_message_ids() -> Vec<u16> {
    let mut ids = WINDOW_MESSAGE_IDS.to_vec();
    for (_, pages) in DIALOGUE_EVENTS {
        ids.extend_from_slice(pages);
    }
    ids
}

/// The pages of a map-played dialogue event.
pub fn dialogue_pages(event: i16) -> Option<&'static [u16]> {
    DIALOGUE_EVENTS.iter().find(|(e, _)| *e == event).map(|(_, p)| *p)
}

/// Mako's bottom panel over the map and its page machine.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StoryDialogue {
    pub event: i16,
    pub pages: Vec<u16>,
    pub page: usize,
    /// The page shown (W+5378).
    pub message: u16,
    /// 2 open/turn, 3 reveal, 4 wait for Cross, 5 page turn, 11, 12
    /// done, 13 closing, 14 closed.
    pub phase: u8,
    pub timer: u32,
    pub reveal_frames: u32,
    pub hold: u32,
    /// W+53BA.
    pub slide: i32,
    /// W+5439.
    pub page_box: i32,
    pub speaker: u8,
    pub selector: u8,
    pub name_message: u16,
    pub mood: u8,
}

impl StoryDialogue {
    /// (x offset s3, y offset s0) of 0x2332C0.
    pub fn offsets(&self) -> (i32, i32) {
        (512 - self.slide / 2, 256 - self.slide / 4)
    }

    /// Whether the panel is drawn (W+53BA != 0).
    pub fn drawn(&self) -> bool {
        self.slide > 0 && self.phase != 14
    }

    /// The face request (speaker, kind, x, y): kind 3 (blink and talk)
    /// from the update after the reveal's first count through phase 4
    /// [emu f44..f62, f192..f210], else 1.
    pub fn face_request(&self) -> Option<(u8, u8, i16, i16)> {
        if !self.drawn() {
            return None;
        }
        let talking = self.phase == 4 || (self.phase == 3 && self.timer >= 2);
        let (_, s0) = self.offsets();
        Some((
            self.speaker,
            if talking { 3 } else { 1 },
            DIALOGUE_FACE_X,
            (s0 + DIALOGUE_FACE_Y) as i16,
        ))
    }
}

impl MapModel {
    /// Whether `event` is a dialogue the map plays (12040, 12041).
    pub fn is_dialogue_event(event: i16) -> bool {
        dialogue_pages(event).is_some()
    }

    fn open_dialogue_page(&mut self, page: usize, events: &mut Vec<MapEvent>) {
        let Some(message) = self
            .story_dialogue
            .as_ref()
            .and_then(|d| d.pages.get(page).copied())
        else {
            return;
        };
        if !self.data.messages.contains_key(&message) {
            self.note("a dialogue page is not in the map import; its reveal is taken as one line");
        }
        let frames = self.reveal_frames(message);
        if let Some(d) = self.story_dialogue.as_mut() {
            d.page = page;
            d.message = message;
            d.timer = 0;
            d.hold = 0;
            d.reveal_frames = frames;
            d.phase = if page == 0 && d.slide >= DIALOGUE_PANEL_FULL { 3 } else { 2 };
        }
        events.push(MapEvent::TextPage {
            message,
            page: page as u16 + 1,
        });
    }

    /// Open (or re-use, after 104) Mako's panel for `event`'s pages.
    fn start_dialogue(&mut self, event: i16, events: &mut Vec<MapEvent>) {
        let pages = dialogue_pages(event).unwrap_or(&[]).to_vec();
        match self.story_dialogue.as_mut() {
            Some(d) => {
                d.event = event;
                d.pages = pages;
            }
            None => {
                self.story_dialogue = Some(StoryDialogue {
                    event,
                    pages,
                    page: 0,
                    message: 0,
                    phase: 2,
                    timer: 0,
                    reveal_frames: 0,
                    hold: 0,
                    slide: 0,
                    page_box: 0,
                    speaker: MAKO_SPEAKER,
                    selector: MAKO_SELECTOR,
                    name_message: MAKO_NAME_MESSAGE,
                    mood: 0,
                });
            }
        }
        self.open_dialogue_page(0, events);
    }

    /// The page machine of the dialogue (after the handler).
    fn dialogue_page_update(&mut self, pad: Pad, events: &mut Vec<MapEvent>) {
        let mut next_page = None;
        {
            let Some(d) = self.story_dialogue.as_mut() else {
                return;
            };
            if d.phase == 2 {
                if d.slide < DIALOGUE_PANEL_FULL {
                    // The slide-in: phase 3 on the update it completes,
                    // the reveal held for the page box [emu f19..f34].
                    d.slide = (d.slide + DIALOGUE_PANEL_STEP).min(DIALOGUE_PANEL_FULL);
                    if d.slide >= DIALOGUE_PANEL_FULL {
                        d.phase = 3;
                        d.hold = DIALOGUE_FIRST_HOLD;
                    }
                    return;
                }
                d.phase = 3;
            }
            match d.phase {
                3 => {
                    if d.page_box < DIALOGUE_BOX_FULL {
                        d.page_box = (d.page_box + DIALOGUE_BOX_STEP).min(DIALOGUE_BOX_FULL);
                    }
                    if d.hold > 0 {
                        d.hold -= 1;
                    } else if d.timer >= d.reveal_frames {
                        d.phase = 4;
                    } else {
                        d.timer += 1;
                    }
                }
                4 => {
                    if pad.cross {
                        events.push(MapEvent::Sfx(SFX_PAGE));
                        d.phase = 5;
                        d.timer = 0;
                    }
                }
                5 => {
                    d.timer += 1;
                    if d.timer >= PAGE_TURN_FRAMES {
                        if d.page + 1 < d.pages.len() {
                            next_page = Some(d.page + 1);
                        } else {
                            d.phase = 11;
                        }
                    }
                }
                11 => d.phase = 12,
                13 => {
                    // The close [emu f344..f368].
                    if d.page_box > 0 {
                        d.page_box = (d.page_box - DIALOGUE_BOX_STEP).max(0);
                    }
                    if d.page_box == 0 {
                        d.slide = (d.slide - DIALOGUE_PANEL_STEP).max(0);
                        if d.slide == 0 {
                            d.phase = 14;
                        }
                    }
                }
                _ => {}
            }
        }
        if let Some(page) = next_page {
            self.open_dialogue_page(page, events);
        }
    }

    /// One frame of 12040 / 12041 (handler, then the page machine).
    fn dialogue_update(&mut self, event: i16, sub: u8, pad: Pad, events: &mut Vec<MapEvent>) {
        let phase = self.story_dialogue.as_ref().map(|d| d.phase);
        let mut sub = sub;
        match (event, sub) {
            (super::white::EVENT_DOVER_STORY, 0) => {
                // The frame C2 became 12040 (the travel's lerp end runs
                // before the dispatcher) changes nothing on screen, as
                // 11120's trigger frame [H: f0 of trace-dover is the
                // patched state].
                sub = 1;
            }
            (super::white::EVENT_DOVER_STORY, 1) => {
                events.push(MapEvent::Music {
                    track: MUSIC_DOVER_DIALOGUE,
                    keep: true,
                });
                self.letterbox.start(2);
                sub = 2;
            }
            (super::white::EVENT_DOVER_STORY, 2) => {
                if self.letterbox.active == 2 {
                    self.start_dialogue(event, events);
                    sub = 3;
                }
            }
            (super::white::EVENT_DOVER_STORY, 3) => {
                if phase.is_some_and(|p| p >= 12) {
                    // C2 = 104 (f070) with Mako's panel still up.
                    events.push(MapEvent::TextClosed);
                    self.save.event = super::white::EVENT_DOVER_VOYAGE;
                    events.push(MapEvent::EventChanged(super::white::EVENT_DOVER_VOYAGE));
                    self.handler = Handler::MapStory { event, sub: 4 };
                    return;
                }
            }
            (super::white::EVENT_DOVER_STORY, _) => {
                // The voyage's sub-step 0 from the next update, on the
                // state `resume_white` gives 104 (update 1 = f071, fitted
                // to dover-12040 [emu f118, f189, f190]).
                self.dover_voyage_entry();
                let voyage = super::white::EVENT_DOVER_VOYAGE;
                self.handler = Handler::Sail { event: voyage, sub: 0 };
                self.sail_update(voyage, 0, pad, events);
                return;
            }
            (_, 0) => {
                self.start_dialogue(event, events);
                sub = 1;
            }
            (_, 1) => {
                if phase.is_some_and(|p| p >= 12) {
                    if let Some(d) = self.story_dialogue.as_mut() {
                        d.phase = 13;
                    }
                    sub = 2;
                }
            }
            (_, 2) => {}
            _ => {
                // The tail (f369, update 1 of `resume_white` at 12041):
                // node 16's rise, HUD fade-in, letterbox mode 0, C2 = 0.
                self.strait_tail_entry();
                self.handler = Handler::Sail { event, sub: 3 };
                self.sail_update(event, 3, pad, events);
                return;
            }
        }
        self.handler = Handler::MapStory { event, sub };
        self.dialogue_page_update(pad, events);
        if event == super::white::EVENT_STRAIT_STORY
            && self.story_dialogue.as_ref().is_some_and(|d| d.phase == 14)
        {
            // The panel is gone (f368, the face slot freed); the tail
            // runs from the next update [emu f369..f473].
            self.story_dialogue = None;
            events.push(MapEvent::TextClosed);
            self.handler = Handler::MapStory { event, sub: 3 };
        }
    }
}
