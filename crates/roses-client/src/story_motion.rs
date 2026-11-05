//! Motion of the map story events played in the story scene (red 11122..11182,
//! white 12130..12154): letterbox growth, painting slides and fades, figure
//! pans, speaker-panel slides, the text reveal and page turns, frame by frame
//! at the original's 25 Hz update. The imported pages stay the settled
//! fallback: this model only moves and re-cuts their layers (and the TEXEVE
//! quads named in `EVENTS`), so an import without what it needs is drawn as
//! before.
//!
//! Sources (PAL SLES_515.79; private research `story/the project notes
//! BACKGROUNDS.md`, the project notes, the project notes,
//! the project notes; traces `trace-trigger/win/seto/manawyddan
//! .json` (red) and `trace-brest/manawyddan.json` (white) made with
//! `tools/trace_story.py` / `trace_white.py`; frame numbers below are theirs):
//!
//! - Letterbox 0x233750 (W+1A mode, W+1E/20/22 bars): each bar moves 8 raw
//!   rows per update toward its mode's target (`campaign_map::Letterbox`,
//!   shared with the map). A mode starts and steps in the same update; the
//!   next step of the event follows the update after it is done [emu f276,
//!   f289/f290, f381/f389/f390].
//! - The C2 update of an event draws no painting (the figures are drawn)
//!   [emu f275, f872, f1326]; the next update starts its letterbox.
//! - Slides W+53B0/W+53B2 (one leads by `step` per update; the other follows
//!   by the floor of the proportion [emu 11182 f516.., 12154 f885..]); a step
//!   past the target is clamped and waits in that update (11122 f379), an
//!   exact landing waits the update after (11142 f1015, 11182 f594, 12154
//!   f1041). Cross on the wait (W+53AE = 2) is read the next update; the
//!   letterbox moves on the update after that [emu f380/f381].
//! - Fades W+54A6: the quad is drawn at 128 on the first update, 2 less each
//!   update; 11144 waits on the update it is drawn at 0 [emu f1142..f1206];
//!   12154's tail switches to 12156 instead [emu white f1254..f1318].
//! - Captions (layout 1 at raw (89, 384)): shown on the update after the
//!   letterbox is done, the wait 5 updates later; the next caption shows 5
//!   updates after the Cross's wait update; after the last one the event
//!   moves on 7 updates after it [emu f390/f395/f402, f1026/f1031/f1036].
//! - Dialogue (0x232EF0 panels, W+5439 pan, W+537B page machine): the first
//!   page opens on the update after the letterbox and its panel slides in at
//!   once (W+53BA / W+53B9 += 64, 16 updates); later pages open 6 updates
//!   after the wait (phase 5 for 5 updates, then phase 2); a newly latched
//!   panel slides in from the update after. Once the panels are in, the pan
//!   moves 10 per update toward the page's target from the next update;
//!   the reveal counts (6 per line + 1) on updates where the pan is within
//!   one step of a target >= 0, or on it for a negative target, then the page
//!   waits [emu: all 61 pages of trace-win/seto fit, NOTES.md].
//! - The figures (slots 6/7, 0x2306A0) are drawn at the pan of the previous
//!   update: bottom x = rest − (512 − rest)(100 − p)/100, top x = rest +
//!   (512 − rest)(p + 100)/100, floored [emu f96..f106, f1725..f1735].
//! - Panel geometry (W+53BA = s): bottom name bar x + (512 − s/2), face
//!   window, wings and ornament y + (256 − s/4); the top panel mirrors it
//!   (bar x − (512 − s/2), face part y − (256 − s/4)) [F 0x2332B0, emu
//!   emblem quads f80..f95, f242..f257, f841..f855].
//! - After the last page: phase 11, 12, then the pan returns to 0 (10 per
//!   update) and, from the update it arrives, every panel slides out (−64);
//!   then the event's tail (`Tail`) [emu f832..f856, f1229..f1253].
//!
//! - Faces: the speaking panel's face talks (kind 3) from the update after
//!   its first reveal count through its wait (`talking`, driving
//!   `face_anim::StoryFaces`) [emu trace-win2].
//! - Entry holds: 11140 waits 12 updates (the map's return panel slides
//!   out) before its letterbox moves, 12130 one (scene bank load); the
//!   scene-0 events 11160/11180/12150 are closed over the map first
//!   (`campaign_map::Handler::StoryEntry`) [emu].
//! - Shake W+5483 (`Shake`, `ShakeTrigger`) and the effect waits of
//!   11162/12132 (`Effect::Opening`) and 11180/12150 (`Effect::OnPage`);
//!   12150 hands its top panel to 12152 (`Tail::Carry`) [F, emu].
//!
//! [H]: the figures' rest formula for Seto (not traced across a pan), the
//! top face's slide direction, the caption alpha (drawn whole while shown),
//! the reveal rows' corner alphas (as the map's layout-0 reveal), the
//! effect modules 401/402 (timed, not drawn), the Cross sound timing (it
//! plays when the page turns, up to 7 updates after the press).

use crate::campaign_map::Letterbox;
use crate::{Button, Layer, Loaded, Model, Sampling, Stage};

/// Raw 512x512 2D space of the story draws.
pub const RAW: i32 = 512;
pub const CANVAS_W: i32 = 640;
pub const CANVAS_H: i32 = 448;
const BAR: i32 = 72;
const THIRD: i32 = 112;
/// Letterbox bars: open (0), 144 rows (dialogue/captions), closed (256).
pub const LETTERBOX_NONE: [i32; 3] = [0, 0, 0];
pub const LETTERBOX_CLOSED: [i32; 3] = [BAR, BAR, THIRD];
pub const PANEL_FULL: i32 = 1024;
pub const PANEL_STEP: i32 = 64;
pub const PAN_STEP: i32 = 10;
pub const ALPHA_FULL: i32 = 128;
pub const FADE_STEP: i32 = 2;
/// Dialogue page turn (phase 5) and caption wait/turn lengths.
pub const TURN_UPDATES: u32 = 5;
pub const CAPTION_WAIT: u32 = 5;
pub const REVEAL_PER_LINE: u32 = 6;
pub const REVEAL_EXTRA: u32 = 1;
/// Raw rows of one text line.
pub const LINE_ROWS: u32 = 22;

/// One quad of the background drawer 0x22E8B0: TEXEVE record, its UV extent
/// (the whole texture) and the raw rectangle relative to (W+53B0, W+53B2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Quad {
    pub record: u16,
    pub uv: [u32; 2],
    pub offset: [i32; 2],
    pub size: [i32; 2],
}

/// What a painting-only Cross wait does before it waits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StillMotion {
    Hold,
    /// The leading coordinate (W+53B0 when it changes, else W+53B2) moves by
    /// `step` per update to `to`.
    Slide { to: [i32; 2], step: i32 },
    /// Quad `quad` fades out (W+54A6).
    Fade { quad: usize },
}

/// A big figure at rest (slot 6 bottom speaker, slot 7 top).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Figure {
    pub record: u16,
    pub uv: [u32; 2],
    pub raw: [i32; 4],
}

/// What follows the last page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tail {
    /// Letterbox mode 3, then the next event after `extra` more updates.
    Close { extra: u32 },
    /// The letterbox stays (11148 -> 11149's window).
    Keep,
    /// The paintings fade out (12154 -> 12156's rising camera).
    FadeQuads,
    /// Mode 3; the event ends the update after the bars are closed (11162
    /// f1423, 12132 white f1680: the map bank is back under the closed
    /// letterbox) and the map opens the letterbox over itself
    /// (`campaign_map::Handler::StoryExit`, mode 0 from f1424 / f1681).
    CloseThenOpen,
    /// 12150 -> 12152: the close waits in phase 12 for the effect clock
    /// (W+5470 = 5) and the shake's end (W+5483 = 4), then the next event
    /// starts at once with the letterbox, pan and panels kept (its first
    /// page slides the old panel out) [F 0x221110..0x221158, emu white
    /// trace-manawyddan f189..f398].
    Carry,
}

/// The effect waits of the Stonehenge scene (C-module effects 401/402 stay
/// undrawn; their timing is kept) [emu trace-seto-cam, white trace-brest /
/// trace-manawyddan].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    None,
    /// 11162/12132, after the opening letterbox (update L): W+64 lowers
    /// from 0x1000 by 256 per update from L+1; the effect overlay
    /// (W+53A4/53A6, undrawn) runs to L+149; the pan moves -4 per update
    /// from L+150 to -100 (L+174), which starts the stage clock; the
    /// first page opens the update after the shake has ramped down.
    Opening,
    /// 11180/12150: the clock and the shake start on `message`'s phase-3
    /// updates once the pan is at -100 (12150: 0x2210A4) [F, emu red
    /// trace-manawyddan f241, white f176]; the close waits in phase 12 for
    /// the effect's end.
    OnPage { message: u16 },
}

/// When the camera/figure shake W+5483 starts (state 1) in an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShakeTrigger {
    /// Once, `after` updates after `message`'s U0, amplitude from 0
    /// (11160 page 2912: 0x2209B8) [F, emu f661].
    Once { message: u16, after: u32 },
    /// Every phase-3 update of `message` from `after` updates after its U0
    /// (12130 page 3308: 0x221F14) [F, emu white f752].
    Phase3 { message: u16, after: u32 },
}

/// The camera shake of the story scene (0x23A6F0 mode 3, 0x23A218 mode 2):
/// state W+5483, amplitude W+5484, counter W+5482 and the offsets
/// W+5490/W+5494 added to the eye and the target (and, floored, to the big
/// figures' quads at 0x2306D4) [F; emu trace-seto-cam f661..f1181].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shake {
    pub state: u8,
    pub amplitude: i32,
    pub counter: u8,
    pub offset: [f32; 2],
}

impl Shake {
    /// One camera update; `stage` = W+5470 as the camera sees it.
    pub fn step(&mut self, stage: u8) {
        match self.state {
            0 => {
                self.offset = [0.0; 2];
                return;
            }
            // Ramp up 5 per update to 100.
            1 => {
                self.amplitude += 5;
                if self.amplitude >= 100 {
                    self.amplitude = 100;
                    self.state = 2;
                }
            }
            // Hold until the effect clock reaches 5.
            2 => {
                self.amplitude = 100;
                if stage == 5 {
                    self.state = 3;
                }
            }
            // Ramp down to 0.
            3 => {
                self.amplitude -= 5;
                if self.amplitude <= 0 {
                    self.amplitude = 0;
                    self.state = 4;
                }
            }
            4 => self.counter = 0,
            _ => {}
        }
        let a = self.amplitude as f32;
        self.offset[0] = match self.counter % 3 {
            1 => -2.0 * a / 100.0,
            2 => 2.0 * a / 100.0,
            _ => 0.0,
        };
        self.offset[1] = match self.counter % 5 {
            1 => -2.0 * a / 100.0,
            2 => 2.0 * a / 100.0,
            3 => -a / 100.0,
            4 => a / 100.0,
            _ => 0.0,
        };
        self.counter = self.counter.wrapping_add(1);
    }
}

/// W+64 at the start of 11162/12132 and its step per update.
pub const LEVEL_FULL: i32 = 0x1000;
pub const LEVEL_STEP: i32 = 256;
/// 11162/12132: the pan ramp after the letterbox (L+150 .. L+174).
pub const EFFECT_PAN_FROM: u32 = 150;
pub const EFFECT_PAN_STEP: i32 = 4;
/// The effect clock W+5470: 1 when started, +1 every 50 updates (the first
/// after 49), up to 5 [emu f961/f1010/f1060/f1110/f1160].
pub const EFFECT_STAGES: u32 = 5;
pub const EFFECT_STAGE_UPDATES: u32 = 50;

#[derive(Debug)]
pub struct EventMotion {
    pub event: u16,
    pub quads: &'static [Quad],
    /// (W+53B0, W+53B2) when the paintings first show.
    pub start: [i32; 2],
    /// One per painting-only page, in page order.
    pub stills: &'static [StillMotion],
    /// Bottom (slot 6) and top (slot 7) figures.
    pub figures: [Option<Figure>; 2],
    /// Captions over the closed letterbox (11143, 11147).
    pub curtain: bool,
    pub tail: Tail,
    /// Letterbox when the story scene starts at this event (not carried).
    pub fresh: [i32; 3],
    /// Updates between the C2 update and the first letterbox step when the
    /// event starts fresh: 11140 waits 12 while the map's return panel
    /// slides out (-25 per update) [emu trace-win2 f50..f61]; 12130 loads
    /// its scene bank in the C2 update and opens from C2+2 [emu white
    /// trace-brest f665..f667].
    pub entry_hold: u32,
    pub effect: Effect,
    pub shake: Option<ShakeTrigger>,
}

const fn q(record: u16, uv: [u32; 2], offset: [i32; 2], size: [i32; 2]) -> Quad {
    Quad {
        record,
        uv,
        offset,
        size,
    }
}
/// Slots 2/3/4/5 of the slide layout: (B0, B2+74), (B0+w, B2+74),
/// (B0, B2+512), (B0+w, B2+512), w x 438 each [emu].
const fn mosaic(records: [u16; 4], uv: [u32; 2], w: i32) -> [Quad; 4] {
    [
        q(records[0], uv, [0, 74], [w, 438]),
        q(records[1], uv, [w, 74], [w, 438]),
        q(records[2], uv, [0, 512], [w, 438]),
        q(records[3], uv, [w, 512], [w, 438]),
    ]
}
const QUADS_11122: [Quad; 4] = mosaic([3, 3, 2, 2], [640, 384], 512);
const QUADS_11142: [Quad; 4] = mosaic([5, 5, 4, 4], [640, 384], 512);
/// 11144: TEXEVE 7 under TEXEVE 6, both raw (0, 0) 512x512 (0x22EDB4).
const QUADS_11144: [Quad; 2] = [
    q(7, [640, 448], [0, 0], [512, 512]),
    q(6, [640, 448], [0, 0], [512, 512]),
];
const QUADS_11146: [Quad; 1] = [q(8, [640, 448], [0, 0], [512, 512])];
const QUADS_11182: [Quad; 4] = mosaic([10, 12, 9, 11], [512, 384], 412);
const QUADS_12154: [Quad; 4] = mosaic([14, 16, 13, 15], [512, 384], 412);
const RICHARD: Figure = Figure {
    record: 20,
    uv: [384, 256],
    raw: [88, 132, 312, 288],
};
const PEGASUS: Figure = Figure {
    record: 19,
    uv: [256, 256],
    raw: [152, 132, 208, 288],
};
const YUGI: Figure = Figure {
    record: 22,
    uv: [256, 256],
    raw: [152, 132, 208, 288],
};
const SETO: Figure = Figure {
    record: 18,
    uv: [512, 256],
    raw: [24, 132, 416, 288],
};
const fn ev(event: u16) -> EventMotion {
    EventMotion {
        event,
        quads: &[],
        start: [0, 0],
        stills: &[],
        figures: [None, None],
        curtain: false,
        tail: Tail::Close { extra: 0 },
        fresh: LETTERBOX_CLOSED,
        entry_hold: 0,
        effect: Effect::None,
        shake: None,
    }
}

/// The events this model moves [emu, see the module notes]. 11120/11124/
/// 11149 are the map's windows; 11184/12156 run into the ending, whose own
/// model owns the letterbox: both keep their settled pages.
pub const EVENTS: [EventMotion; 16] = [
    EventMotion {
        quads: &QUADS_11122,
        start: [0, -448],
        stills: &[StillMotion::Slide {
            to: [0, -91],
            step: 4,
        }],
        ..ev(11122)
    },
    EventMotion {
        figures: [Some(RICHARD), Some(PEGASUS)],
        tail: Tail::Close { extra: 1 },
        fresh: LETTERBOX_NONE,
        entry_hold: 12,
        ..ev(11140)
    },
    EventMotion {
        quads: &QUADS_11142,
        start: [0, 0],
        stills: &[StillMotion::Slide {
            to: [0, -512],
            step: -4,
        }],
        ..ev(11142)
    },
    EventMotion {
        curtain: true,
        ..ev(11143)
    },
    EventMotion {
        quads: &QUADS_11144,
        stills: &[StillMotion::Hold, StillMotion::Fade { quad: 1 }],
        ..ev(11144)
    },
    EventMotion {
        quads: &QUADS_11146,
        stills: &[StillMotion::Hold],
        ..ev(11146)
    },
    EventMotion {
        curtain: true,
        ..ev(11147)
    },
    EventMotion {
        figures: [Some(YUGI), Some(PEGASUS)],
        tail: Tail::Keep,
        ..ev(11148)
    },
    EventMotion {
        figures: [Some(SETO), None],
        shake: Some(ShakeTrigger::Once { message: 2912, after: 1 }),
        ..ev(11160)
    },
    EventMotion {
        tail: Tail::CloseThenOpen,
        effect: Effect::Opening,
        ..ev(11162)
    },
    EventMotion {
        effect: Effect::OnPage { message: 2964 },
        ..ev(11180)
    },
    EventMotion {
        quads: &QUADS_11182,
        start: [0, -438],
        stills: &[
            StillMotion::Slide {
                to: [-156, -219],
                step: -2,
            },
            StillMotion::Slide {
                to: [-312, 0],
                step: -2,
            },
        ],
        ..ev(11182)
    },
    EventMotion {
        figures: [Some(SETO), None],
        entry_hold: 1,
        shake: Some(ShakeTrigger::Phase3 { message: 3308, after: 2 }),
        ..ev(12130)
    },
    EventMotion {
        tail: Tail::CloseThenOpen,
        effect: Effect::Opening,
        ..ev(12132)
    },
    EventMotion {
        figures: [Some(SETO), None],
        ..ev(12152)
    },
    EventMotion {
        quads: &QUADS_12154,
        start: [-312, 0],
        stills: &[StillMotion::Slide {
            to: [0, -470],
            step: 2,
        }],
        tail: Tail::FadeQuads,
        ..ev(12154)
    },
];

/// 12150: no figure; its close waits for the effect and hands its top
/// panel to 12152 (listed apart to keep the table's length readable).
const EVENT_12150: EventMotion = EventMotion {
    tail: Tail::Carry,
    effect: Effect::OnPage { message: 3388 },
    ..ev(12150)
};

/// The 3D world the original draws every frame under a map story event's
/// 2D layers: the campaign map (W+53B5 = 0: 161..207 model draws in the red
/// traces, 52/53 under Mako's pages) or the Stonehenge scene 0 (pack 33,
/// W+53B5 1..3: 10 model draws) [emu `story/the project notes
/// .json`, `trace-win3.json`, `trace-seto-cam.json`, `trace-man-cam.json`;
/// the project notes; the project notes]. Where the
/// event draws a black fill or paintings over the whole screen (0x22EEEC,
/// 0x22EDB4) or closes the letterbox, the world is drawn but covered; the
/// frontend draws it all the same (map events never leave the map mode).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Behind {
    Map,
    Scene0,
}

pub fn behind(event: u16) -> Option<Behind> {
    match event {
        11120 | 11122 | 11124 | 11140 | 11142 | 11143 | 11144 | 11146 | 11147 | 11148 | 11149
        | 12040 | 12041 => Some(Behind::Map),
        11160 | 11162 | 11180 | 11182 | 11184 | 12130 | 12132 | 12150 | 12152 | 12154 | 12156 => {
            Some(Behind::Scene0)
        }
        _ => None,
    }
}

pub fn event_motion(event: u16) -> Option<&'static EventMotion> {
    if event == 12150 {
        return Some(&EVENT_12150);
    }
    EVENTS.iter().find(|e| e.event == event)
}

/// The map story event of a stage id (`map-story-<event>`).
pub fn stage_event(id: &str) -> Option<u16> {
    id.strip_prefix("map-story-")?.parse().ok()
}

/// How a page is presented.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Still,
    Caption,
    /// A dialogue page: pan target (+100 bottom speaker, −100 top, 0 voice
    /// without a speaker) and the panel it latches (0 bottom, 1 top).
    Dialogue { target: i32, latch: Option<usize> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageInfo {
    pub role: Role,
    pub lines: u32,
    /// The page's message id (0 = unknown).
    pub message: u16,
}

/// State carried from one event to the next.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Carry {
    pub bars: [i32; 3],
    pub pan: i32,
    /// The camera shake runs on across events (11160 -> 11162, 12130 ->
    /// 12132) [emu].
    pub shake: Shake,
    /// `Tail::Carry`: the dialogue continues (the next event's first page
    /// opens on the update after its C2 update) and these panels, of the
    /// given stage and page's set, slide out from that page's U0.
    pub dialogue: bool,
    pub panels: [Option<i32>; 2],
    pub from: Option<(String, usize)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Then {
    Still(usize),
    Page,
    End,
    /// 11162/12132's tail: one closed update, then the map (`CloseThenOpen`).
    Open,
    /// 11162/12132: the effect wait, then the first page.
    Effect,
}

/// A state whose first update is the next one.
const PENDING: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// Not stepped yet; its first update is the C2 update.
    Start,
    /// A letterbox mode runs (`first` = not yet started).
    Letterbox { mode: u8, then: Then, first: bool },
    Still { k: usize, first: bool },
    StillWait { k: usize },
    /// Cross on a still was read on this update (`t` = 0).
    AfterStill { next: bool },
    Caption { t: u32 },
    CaptionTurn { t: u32 },
    /// Dialogue page machine phases 2/3/4/5, 11, 12.
    Dialogue { phase: u8, t: u32 },
    Closing,
    TailFade { first: bool },
    Pause { left: u32, then: Then },
    /// The fresh entry's hold before the first letterbox step.
    Hold { left: u32 },
    /// `Effect::Opening`, `t` updates after the letterbox was done.
    Effect { t: u32 },
    Done,
}

/// The story camera of this update (0x23A02C): W+64, W+5439 and the shake
/// offsets W+5490/W+5494.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StoryCamera {
    pub level: i32,
    pub pan: i32,
    pub shake: [f32; 2],
}

/// The motion of one story stage.
#[derive(Clone, Debug)]
pub struct StoryMotion {
    stage: String,
    spec: &'static EventMotion,
    pages: Vec<PageInfo>,
    /// The page shown (the model's page index once forwarded).
    pub page: usize,
    /// Updates stepped in this stage.
    pub update: u32,
    pub letterbox: Letterbox,
    /// W+53B0, W+53B2.
    pub slide: [i32; 2],
    /// Drawn alpha (0..=128) of the fading quads.
    pub alpha: i32,
    /// W+5439 and its value at the start of the update (the figures).
    pub pan: i32,
    pub drawn_pan: i32,
    /// Panel slides W+53BA (bottom) / top; `None` = not latched.
    pub panels: [Option<i32>; 2],
    state: State,
    count: u32,
    ready: bool,
    cross: bool,
    still: usize,
    /// The quad fading (W+54A6; `usize::MAX` = all).
    fading: Option<usize>,
    ended: bool,
    /// W+64 (the camera level of 11162/12132).
    pub level: i32,
    pub shake: Shake,
    /// The update the effect clock W+5470 started (= 1).
    clock: Option<u32>,
    /// U0 of the shown page.
    page_u0: u32,
    /// Panel sides whose face talks this update (kind 3).
    talking: [bool; 2],
    /// Panels of the previous event sliding out: side, W+53BA-like slide,
    /// that panel's layers (`Tail::Carry`).
    leaving: Vec<(usize, i32, Vec<Layer>)>,
    /// The state after the entry hold.
    after_hold: State,
    carried_dialogue: bool,
}

/// What one update produced.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StepResult {
    /// Hand this button to the shared model now (page turn / stage end).
    pub forward: Option<Button>,
    /// The drawn frame changed.
    pub changed: bool,
}

impl StoryMotion {
    /// The motion of `model`'s current story stage, or `None` when the stage
    /// has no motion here (or the import lacks what it needs: the settled
    /// pages are drawn then).
    pub fn new(loaded: &Loaded, model: &Model, carry: Option<Carry>) -> Option<Self> {
        let Stage::Story { id, pages, .. } = model.stage() else {
            return None;
        };
        let spec = event_motion(stage_event(id)?)?;
        let has = |asset: &str| loaded.assets.get(asset).is_some_and(|a| a.image.is_some());
        if spec
            .quads
            .iter()
            .map(|q| q.record)
            .chain(spec.figures.iter().flatten().map(|f| f.record))
            .any(|r| !has(&format!("texeve-{r:02}")))
            || !has("story-black")
        {
            return None;
        }
        let mut infos = Vec::new();
        for page in pages {
            let lines = page_lines(loaded, page);
            let set = page.layer_set.as_deref().unwrap_or("");
            let role = if page.id.contains("-painting-") {
                Role::Still
            } else if set.contains("caption") || set.contains("closed-letterbox") {
                Role::Caption
            } else if set.contains("world-letterbox") {
                return None;
            } else {
                let target = i32::from(page.pan.unwrap_or(100));
                Role::Dialogue {
                    target,
                    latch: match target {
                        t if t > 0 => Some(0),
                        t if t < 0 => Some(1),
                        _ => None,
                    },
                }
            };
            let message = page
                .id
                .rsplit_once("message-")
                .and_then(|(_, m)| m.parse().ok())
                .unwrap_or(0);
            infos.push(PageInfo { role, lines, message });
        }
        let from = carry.as_ref().filter(|c| c.dialogue).and_then(|c| c.from.clone());
        let mut motion = Self::from_pages(id, spec, infos, carry)?;
        motion.page = model.page_index();
        // The previous event's panels that slide out under the first page.
        if let Some((stage, page)) = from {
            let set = loaded
                .manifest
                .stages
                .iter()
                .find(|s| s.id() == stage)
                .and_then(|s| match s {
                    Stage::Story { pages, .. } => pages.get(page),
                    _ => None,
                })
                .and_then(|p| p.layer_set.as_ref())
                .and_then(|name| loaded.manifest.layer_sets.get(name));
            for (side, _, layers) in &mut motion.leaving {
                if let Some(set) = set {
                    *layers = panel_layers(set, *side);
                }
            }
        }
        Some(motion)
    }

    /// The model of `stage` (event `spec.event`) over `pages`.
    pub fn from_pages(
        stage: &str,
        spec: &'static EventMotion,
        pages: Vec<PageInfo>,
        carry: Option<Carry>,
    ) -> Option<Self> {
        let stills = pages.iter().filter(|p| p.role == Role::Still).count();
        if pages.is_empty() || stills != spec.stills.len() {
            return None;
        }
        let fresh = carry.is_none();
        let carry = carry.unwrap_or(Carry {
            bars: spec.fresh,
            ..Carry::default()
        });
        let (bars, pan) = (carry.bars, carry.pan);
        let leaving = if carry.dialogue {
            (0..2)
                .filter_map(|side| carry.panels[side].filter(|&s| s > 0).map(|s| (side, s, Vec::new())))
                .collect()
        } else {
            Vec::new()
        };
        Some(Self {
            stage: stage.into(),
            spec,
            pages,
            page: 0,
            update: 0,
            letterbox: Letterbox {
                active: 2,
                mode: if bars == LETTERBOX_NONE { 0 } else { 3 },
                bars,
            },
            slide: spec.start,
            alpha: ALPHA_FULL,
            pan,
            drawn_pan: pan,
            panels: [None, None],
            state: State::Start,
            count: 0,
            ready: false,
            cross: false,
            still: 0,
            fading: None,
            ended: false,
            level: if spec.effect == Effect::Opening { LEVEL_FULL } else { 0 },
            shake: if fresh { Shake::default() } else { carry.shake },
            clock: None,
            page_u0: 0,
            talking: [false; 2],
            leaving,
            after_hold: State::Done,
            carried_dialogue: carry.dialogue,
            // (A fresh entry holds `spec.entry_hold` updates.)
        })
        .map(|mut m| {
            if fresh && spec.entry_hold > 0 {
                m.after_hold = State::Hold { left: spec.entry_hold };
            }
            m
        })
    }

    pub fn stage(&self) -> &str {
        &self.stage
    }
    pub fn event(&self) -> u16 {
        self.spec.event
    }
    /// The stage has handed its last Cross to the model.
    pub fn finished(&self) -> bool {
        self.ended
    }
    pub fn carry(&self) -> Carry {
        let dialogue = self.spec.tail == Tail::Carry;
        Carry {
            bars: self.letterbox.bars,
            pan: self.pan,
            shake: self.shake,
            dialogue,
            panels: if dialogue { self.panels } else { [None; 2] },
            from: Some((self.stage.clone(), self.page)),
        }
    }
    /// Sides (bottom, top) whose face talks this update: the speaking
    /// panel from the update after its page's first reveal count through
    /// its wait [emu trace-win2: kind 3 f105..f123 for 2688 (counts
    /// f104..f122, wait f123), f278..f284 for 2708].
    pub fn talking(&self) -> [bool; 2] {
        self.talking
    }
    /// The story camera of this update.
    pub fn camera(&self) -> StoryCamera {
        StoryCamera {
            level: self.level,
            pan: self.pan,
            shake: self.shake.offset,
        }
    }
    /// W+5470 after update `u` (0 before the clock starts).
    pub fn effect_stage(&self, u: u32) -> u32 {
        match self.clock {
            Some(t1) if u >= t1 => (1 + (u - t1 + 1) / EFFECT_STAGE_UPDATES).min(EFFECT_STAGES),
            _ => 0,
        }
    }
    /// The page waits for Cross (W+53AE = 2, W+537B = 4, a caption's wait).
    pub fn waiting(&self) -> bool {
        matches!(
            self.state,
            State::StillWait { .. } | State::Dialogue { phase: 4, .. }
        ) || matches!(self.state, State::Caption { t } if t >= CAPTION_WAIT && t != PENDING)
    }
    /// A press from the pad: Cross is read on the next update when the page
    /// waits; every other press (and Cross at other times) is ignored, as
    /// the original does. Always consumed.
    pub fn press(&mut self, button: Button) -> bool {
        if button == Button::Cross && self.waiting() {
            self.cross = true;
        }
        true
    }

    fn reveal_frames(&self) -> u32 {
        self.pages
            .get(self.page)
            .map_or(1, |p| p.lines.max(1) * REVEAL_PER_LINE + REVEAL_EXTRA)
    }
    fn role(&self, page: usize) -> Option<Role> {
        self.pages.get(page).map(|p| p.role)
    }
    fn target(&self) -> i32 {
        match self.role(self.page) {
            Some(Role::Dialogue { target, .. }) => target,
            _ => 0,
        }
    }
    fn start_letterbox(&mut self, mode: u8) {
        self.letterbox.start(mode);
        self.letterbox.step();
    }
    fn closed(&self) -> bool {
        self.letterbox.bars == LETTERBOX_CLOSED
    }
    /// Step the slide coordinates toward `to`; true when it waits now.
    fn slide_step(&mut self, to: [i32; 2], step: i32) -> bool {
        if self.slide == to {
            return true;
        }
        let from = if self.spec.stills.len() > 1 && self.still > 0 {
            match self.spec.stills[self.still - 1] {
                StillMotion::Slide { to, .. } => to,
                _ => self.spec.start,
            }
        } else {
            self.spec.start
        };
        let lead = usize::from(from[0] == to[0]);
        let other = 1 - lead;
        let mut v = self.slide[lead] + step;
        let mut wait = false;
        if (step > 0 && v > to[lead]) || (step < 0 && v < to[lead]) {
            v = to[lead];
            wait = true;
        }
        self.slide[lead] = v;
        let (mut num, mut den) = (
            i64::from(from[lead] - v) * i64::from(to[other] - from[other]),
            i64::from(from[lead] - to[lead]),
        );
        if den < 0 {
            num = -num;
            den = -den;
        }
        self.slide[other] = if den == 0 {
            to[other]
        } else {
            from[other] + num.div_euclid(den) as i32
        };
        wait
    }
    fn pan_step(&mut self, target: i32) {
        if self.pan < target {
            self.pan = (self.pan + PAN_STEP).min(target);
        } else if self.pan > target {
            self.pan = (self.pan - PAN_STEP).max(target);
        }
    }
    /// The reveal gate [emu fit over every traced page].
    fn gate(&self, target: i32) -> bool {
        if target < 0 {
            self.pan == target
        } else {
            (target - self.pan).abs() <= PAN_STEP
        }
    }
    fn open_page(&mut self, first: bool) {
        self.count = 0;
        self.ready = false;
        self.page_u0 = self.update;
        if let Some(Role::Dialogue {
            latch: Some(side), ..
        }) = self.role(self.page)
        {
            if self.panels[side].is_none() {
                self.panels[side] = Some(0);
            }
        }
        if first {
            self.slide_panels(PANEL_STEP);
            self.slide_leaving();
        }
    }
    /// The previous event's panels move out by one step.
    fn slide_leaving(&mut self) {
        for (_, s, _) in &mut self.leaving {
            *s = (*s - PANEL_STEP).max(0);
        }
        self.leaving.retain(|(_, s, _)| *s > 0);
    }
    /// Move every latched panel by `delta`; true when all are at rest.
    fn slide_panels(&mut self, delta: i32) -> bool {
        let mut rest = true;
        for s in self.panels.iter_mut().flatten() {
            if delta > 0 && *s < PANEL_FULL {
                *s = (*s + delta).min(PANEL_FULL);
            } else if delta < 0 && *s > 0 {
                *s = (*s + delta).max(0);
            }
            if (delta > 0 && *s < PANEL_FULL) || (delta < 0 && *s > 0) {
                rest = false;
            }
        }
        rest
    }
    fn panels_in(&self) -> bool {
        self.panels.iter().flatten().all(|&s| s >= PANEL_FULL)
    }

    /// One 25 Hz update.
    pub fn step(&mut self) -> StepResult {
        let before = self.visual();
        if self.ended {
            return StepResult::default();
        }
        let mut forward = false;
        self.update += 1;
        self.drawn_pan = self.pan;
        let cross = std::mem::take(&mut self.cross);
        let count_before = self.count;
        // The 11180/12150 handler reads the page, phase and pan as the
        // previous update left them [emu red f240/f241, white f175/f176].
        let on_page_before = matches!(self.state, State::Dialogue { phase: 3, .. })
            && self.pan <= -100
            && self.page_u0 > 0
            && matches!(self.spec.effect, Effect::OnPage { message } if self.pages.get(self.page).map(|p| p.message) == Some(message));
        let mut state = self.state;
        // A transition that also acts in this update runs once more.
        for _ in 0..3 {
            let mut again = false;
            match state {
                State::Done => {}
                // The C2 update; the event's first step is the next one.
                State::Start => {
                    let next = match self.role(self.page) {
                        // (A stage entered at a later page: its stills so far.)
                        Some(Role::Still) => State::Letterbox {
                            mode: 1,
                            then: Then::Still(
                                self.pages[..self.page].iter().filter(|p| p.role == Role::Still).count(),
                            ),
                            first: true,
                        },
                        Some(Role::Caption) if self.spec.curtain => State::Caption { t: PENDING },
                        _ => State::Letterbox {
                            mode: 2,
                            then: if self.spec.effect == Effect::Opening {
                                Then::Effect
                            } else {
                                Then::Page
                            },
                            first: true,
                        },
                    };
                    state = if self.carried_dialogue {
                        // The letterbox is up: U0 on the next update [emu
                        // white f397/f398].
                        State::Dialogue { phase: 2, t: PENDING }
                    } else if let State::Hold { left } = self.after_hold {
                        self.after_hold = next;
                        State::Hold { left }
                    } else {
                        next
                    };
                }
                State::Hold { left } => {
                    state = if left > 1 {
                        State::Hold { left: left - 1 }
                    } else {
                        self.after_hold
                    };
                }
                State::Effect { t } => {
                    self.level = (LEVEL_FULL - LEVEL_STEP * t as i32).max(0);
                    if t >= EFFECT_PAN_FROM {
                        self.pan = (self.pan - EFFECT_PAN_STEP).max(-100);
                    }
                    if self.pan <= -100 && self.clock.is_none() {
                        self.clock = Some(self.update);
                        // (A stage entered without 11160/12130's shake.)
                        if self.shake.state == 0 {
                            self.shake.state = 2;
                            self.shake.amplitude = 100;
                        }
                    }
                    state = State::Effect { t: t + 1 };
                }
                State::Letterbox { mode, then, first } => {
                    if first {
                        self.start_letterbox(mode);
                    } else if self.letterbox.active == 1 {
                        self.letterbox.step();
                    }
                    state = if self.letterbox.active != 2 {
                        State::Letterbox {
                            mode,
                            then,
                            first: false,
                        }
                    } else {
                        // The next step follows on the next update.
                        match then {
                            Then::Still(k) => State::Still { k, first: true },
                            Then::Page => match self.role(self.page) {
                                Some(Role::Caption) => State::Caption { t: PENDING },
                                _ => State::Dialogue {
                                    phase: 2,
                                    t: PENDING,
                                },
                            },
                            Then::End => match self.spec.tail {
                                Tail::Close { extra } if extra > 0 => State::Pause {
                                    left: extra,
                                    then: Then::End,
                                },
                                _ => {
                                    forward = true;
                                    State::Done
                                }
                            },
                            Then::Open => State::Pause {
                                left: 1,
                                then: Then::Open,
                            },
                            Then::Effect => State::Effect { t: 1 },
                        }
                    };
                }
                State::Pause { left, then } => {
                    if left > 1 {
                        state = State::Pause {
                            left: left - 1,
                            then,
                        };
                    } else {
                        forward = true;
                        state = State::Done;
                    }
                }
                State::Still { k, first } => {
                    self.still = k;
                    let wait = match self.spec.stills.get(k) {
                        Some(StillMotion::Slide { to, step }) => self.slide_step(*to, *step),
                        Some(StillMotion::Fade { quad }) => {
                            if first {
                                self.fading = Some(*quad);
                                self.alpha = ALPHA_FULL;
                            } else {
                                self.alpha = (self.alpha - FADE_STEP).max(0);
                            }
                            self.alpha == 0
                        }
                        _ => true,
                    };
                    state = if wait {
                        State::StillWait { k }
                    } else {
                        State::Still { k, first: false }
                    };
                }
                State::StillWait { .. } => {
                    if cross {
                        let next = self.role(self.page + 1).is_some();
                        if next {
                            forward = true;
                            self.page += 1;
                        }
                        state = State::AfterStill { next };
                    }
                }
                State::AfterStill { next } => {
                    state = State::Letterbox {
                        mode: if next { 2 } else { 3 },
                        then: if next { Then::Page } else { Then::End },
                        first: true,
                    };
                    again = true;
                }
                State::Caption { t } => {
                    state = if t == PENDING {
                        State::Caption { t: 0 }
                    } else if t >= CAPTION_WAIT {
                        if cross {
                            State::CaptionTurn { t: 0 }
                        } else {
                            state
                        }
                    } else {
                        State::Caption { t: t + 1 }
                    };
                }
                State::CaptionTurn { t } => {
                    let t = t + 1;
                    state = match self.role(self.page + 1) {
                        Some(Role::Caption) if t == 4 => {
                            forward = true;
                            self.page += 1;
                            State::Caption { t: 0 }
                        }
                        Some(Role::Still) if t == 6 => {
                            forward = true;
                            self.page += 1;
                            again = true;
                            State::Letterbox {
                                mode: 1,
                                then: Then::Still(self.still + 1),
                                first: true,
                            }
                        }
                        // The caption stays drawn on the last update.
                        None if self.spec.curtain && self.closed() && t == 5 => {
                            forward = true;
                            self.ended = true;
                            State::CaptionTurn { t }
                        }
                        None if t == 6 => {
                            again = true;
                            State::Letterbox {
                                mode: 3,
                                then: Then::End,
                                first: true,
                            }
                        }
                        _ => State::CaptionTurn { t },
                    };
                }
                State::Dialogue { phase, t } => match phase {
                    2 => {
                        // U0 of a page; the first page of the event slides
                        // its panel in at once.
                        self.open_page(t == PENDING);
                        state = State::Dialogue { phase: 3, t: 0 };
                    }
                    3 => {
                        if self.count >= self.reveal_frames() {
                            state = State::Dialogue { phase: 4, t: 0 };
                        } else {
                            let target = self.target();
                            if !self.ready {
                                self.slide_panels(PANEL_STEP);
                                self.slide_leaving();
                                self.ready = self.panels_in();
                            } else {
                                self.pan_step(target);
                            }
                            if self.ready && self.gate(target) {
                                self.count += 1;
                            }
                        }
                    }
                    4 => {
                        if cross {
                            state = State::Dialogue { phase: 5, t: 0 };
                        }
                    }
                    5 => {
                        if t + 1 >= TURN_UPDATES {
                            if self.role(self.page + 1).is_some() {
                                forward = true;
                                self.page += 1;
                                state = State::Dialogue { phase: 2, t: 0 };
                                again = true;
                            } else {
                                state = State::Dialogue { phase: 11, t: 0 };
                            }
                        } else {
                            state = State::Dialogue { phase: 5, t: t + 1 };
                        }
                    }
                    11 => state = State::Dialogue { phase: 12, t: 0 },
                    // 11180/12150: phase 12 waits for the effect (checked
                    // after the camera below).
                    12 if matches!(self.spec.effect, Effect::OnPage { .. }) => {
                        if self.clock.is_none() {
                            // (Its page was left before the trigger.)
                            self.clock = Some(self.update);
                            self.shake.state = 1;
                        }
                    }
                    _ => {
                        state = State::Closing;
                        again = true;
                    }
                },
                State::Closing => {
                    self.pan_step(0);
                    if self.pan == 0 && self.slide_panels(-PANEL_STEP) {
                        state = match self.spec.tail {
                            Tail::Keep => {
                                forward = true;
                                State::Done
                            }
                            Tail::FadeQuads => State::TailFade { first: true },
                            Tail::Close { .. } => State::Letterbox {
                                mode: 3,
                                then: Then::End,
                                first: true,
                            },
                            Tail::CloseThenOpen => State::Letterbox {
                                mode: 3,
                                then: Then::Open,
                                first: true,
                            },
                            Tail::Carry => {
                                forward = true;
                                State::Done
                            }
                        };
                    }
                }
                State::TailFade { first } => {
                    if first {
                        self.fading = Some(usize::MAX);
                        self.alpha = ALPHA_FULL;
                    } else {
                        self.alpha = (self.alpha - FADE_STEP).max(0);
                    }
                    state = State::TailFade { first: false };
                    // The next event's C2 update would draw alpha 0.
                    if self.alpha <= FADE_STEP {
                        forward = true;
                        state = State::Done;
                    }
                }
            }
            if !again {
                break;
            }
        }
        // The event handler's triggers (before the camera) [F].
        let message = self.pages.get(self.page).map_or(0, |p| p.message);
        let phase3 = matches!(state, State::Dialogue { phase: 3, .. });
        match self.spec.shake {
            Some(ShakeTrigger::Once { message: m, after })
                if m == message && self.page_u0 > 0 && self.update == self.page_u0 + after =>
            {
                self.shake.state = 1;
                self.shake.amplitude = 0;
            }
            Some(ShakeTrigger::Phase3 { message: m, after })
                if m == message && phase3 && self.page_u0 > 0 && self.update >= self.page_u0 + after =>
            {
                self.shake.state = 1;
            }
            _ => {}
        }
        if on_page_before && self.clock.is_none() {
            self.clock = Some(self.update);
            self.shake.state = 1;
        }
        // The camera (0x23A6F0) sees W+5470 as the effect left it.
        let seen = self.effect_stage(self.update.saturating_sub(1));
        self.shake.step(seen as u8);
        // The handler's waits on the effect (after the camera) [F
        // 0x220CD4, 0x221120; emu f1181/f1182, white f396/f397].
        let effect_done = self.shake.state == 4 && self.effect_stage(self.update) == EFFECT_STAGES;
        match state {
            State::Effect { .. } if effect_done => {
                state = State::Dialogue { phase: 2, t: PENDING };
            }
            // W+537B = 13 and W+5483 = 0: 12150 hands over to 12152, 11180
            // returns the pan from the next update [emu red trace-manawyddan
            // f461/f462, white f396/f397].
            State::Dialogue { phase: 12, .. } if matches!(self.spec.effect, Effect::OnPage { .. }) && effect_done => {
                self.shake.state = 0;
                if self.spec.tail == Tail::Carry {
                    forward = true;
                    state = State::Done;
                } else {
                    state = State::Closing;
                }
            }
            _ => {}
        }
        let speaking = match (state, self.role(self.page)) {
            (State::Dialogue { phase: 3 | 4, .. }, Some(Role::Dialogue { latch: Some(side), .. }))
                if count_before.min(self.count) > 0 =>
            {
                Some(side)
            }
            _ => None,
        };
        self.talking = [speaking == Some(0), speaking == Some(1)];
        self.state = state;
        if state == State::Done {
            self.ended = true;
        }
        StepResult {
            forward: forward.then_some(Button::Cross),
            changed: self.visual() != before,
        }
    }

    /// Everything that changes the drawn frame.
    #[allow(clippy::type_complexity)]
    fn visual(
        &self,
    ) -> (i32, [i32; 2], i32, i32, [Option<i32>; 2], usize, Option<[[u8; 4]; 3]>, bool, [i32; 2], Vec<i32>) {
        (
            self.letterbox.height(),
            self.slide,
            self.alpha,
            self.drawn_pan,
            self.panels,
            self.page,
            self.text_state(),
            self.painting_visible(),
            self.figure_shake(),
            self.leaving.iter().map(|l| l.1).collect(),
        )
    }

    /// The figures' whole-pixel shake (x + W+5490, y + W+5494 floored).
    fn figure_shake(&self) -> [i32; 2] {
        if self.spec.figures.iter().all(Option::is_none) {
            return [0; 2];
        }
        self.shake.offset.map(|d| d.floor() as i32)
    }

    fn painting_visible(&self) -> bool {
        self.update > 1
    }

    /// The text of the shown page: `None` hidden, else per-row corner alphas.
    pub fn text_state(&self) -> Option<[[u8; 4]; 3]> {
        let rows = self.pages.get(self.page).map_or(1, |p| p.lines as usize);
        match self.state {
            State::Caption { t } if t != PENDING => Some(reveal_alphas(rows, u32::MAX)),
            State::CaptionTurn { .. } => Some(reveal_alphas(rows, u32::MAX)),
            State::Dialogue { phase: 3, .. } => Some(reveal_alphas(rows, self.count)),
            State::Dialogue { phase: 4, .. } => Some(reveal_alphas(rows, u32::MAX)),
            State::Dialogue { phase: 5, t } => {
                Some(fade_alphas(reveal_alphas(rows, u32::MAX), t + 1))
            }
            _ => None,
        }
    }

    /// The frame's layers from the page's settled `set` and its own
    /// `page_layers` (text). `dims` gives an image asset's size.
    pub fn layers(
        &self,
        set: &[Layer],
        page_layers: &[Layer],
        dims: &dyn Fn(&str) -> Option<[u32; 2]>,
    ) -> Vec<Layer> {
        let is_band = |l: &Layer| {
            let [x, y, w, h] = l.destination;
            l.asset == "story-black"
                && x == 0
                && w == 640
                && h < 448
                && (y == 0 || y + h == 448)
        };
        let Some(first_band) = set.iter().position(is_band) else {
            let mut all = set.to_vec();
            all.extend(page_layers.iter().cloned());
            return all;
        };
        let last_band = set.iter().rposition(is_band).unwrap_or(first_band);
        let mut out = Vec::new();
        // Backdrop: the black base, the paintings or the figures.
        if !self.spec.quads.is_empty() {
            out.push(black([0, 0, CANVAS_W, CANVAS_H]));
            if self.painting_visible() {
                let fade = self.fading;
                for (i, quad) in self.spec.quads.iter().enumerate() {
                    let raw = [
                        self.slide[0] + quad.offset[0],
                        self.slide[1] + quad.offset[1],
                        quad.size[0],
                        quad.size[1],
                    ];
                    let opacity = match fade {
                        Some(f) if f == usize::MAX || f == i => opacity(self.alpha),
                        _ => 255,
                    };
                    if opacity == 0 {
                        continue;
                    }
                    if let Some(mut l) =
                        raw_quad_layer(&format!("texeve-{:02}", quad.record), quad.uv, raw)
                    {
                        l.opacity = opacity;
                        out.push(l);
                    }
                }
            }
        } else if self.spec.figures.iter().any(Option::is_some) {
            for (slot, figure) in self.spec.figures.iter().enumerate() {
                let Some(f) = figure else { continue };
                // The shake moves the quad by the float offsets, floored
                // (Seto f662: (24, 132) - (0.2, 0.2) -> (23, 131)) [emu].
                let x = figure_x(f.raw[0], slot, self.drawn_pan);
                let [dx, dy] = self.shake.offset;
                let (x, y) = ((x as f32 + dx).floor() as i32, (f.raw[1] as f32 + dy).floor() as i32);
                if let Some(l) = raw_quad_layer(
                    &format!("texeve-{:02}", f.record),
                    f.uv,
                    [x, y, f.raw[2], f.raw[3]],
                ) {
                    out.push(l);
                }
            }
        } else {
            out.extend(set[..first_band].iter().cloned());
        }
        out.extend(letterbox_layers(self.letterbox.height()));
        // Speaker panels.
        for layer in &set[last_band + 1..] {
            let Some(s) = self.panels[panel_side(layer)] else { continue };
            out.extend(panel_layer(layer, s, dims));
        }
        // The previous event's panels sliding out (12152).
        for (_, s, layers) in &self.leaving {
            for layer in layers {
                out.extend(panel_layer(layer, *s, dims));
            }
        }
        // Text rows.
        let text = self.text_state();
        for layer in page_layers {
            if !layer.asset.starts_with("story-text-") {
                out.push(layer.clone());
                continue;
            }
            let Some(alphas) = text else { continue };
            out.extend(text_rows(layer, alphas, dims));
        }
        out
    }

    /// Whether the page's runtime name text (if any) is drawn now.
    pub fn name_text_visible(&self) -> bool {
        self.text_state().is_some()
    }
}

/// Text lines of a story page (1..=3): its composed text layer's rows, or
/// its runtime name text's lines.
pub fn page_lines(loaded: &Loaded, page: &crate::StoryPage) -> u32 {
    let text = page.layers.iter().find(|l| l.asset.starts_with("story-text-"));
    text.and_then(|l| loaded.assets.get(&l.asset))
        .and_then(|a| a.image.as_ref())
        .map(|i| (i.height / LINE_ROWS).clamp(1, 3))
        .or(page.name_text.as_ref().map(|t| (t.lines.len() as u32).clamp(1, 3)))
        .unwrap_or(1)
}

/// Settled pages (no motion): the speaking side talks from the second
/// update after the page shows through the reveal's length + its wait,
/// as a moving page does after its reveal starts [H: the settled stages do
/// not reproduce the pan/panel wait before the reveal].
pub fn settled_talking(pan: i8, lines: u32, updates: u32) -> [bool; 2] {
    let reveal = lines.clamp(1, 3) * REVEAL_PER_LINE + REVEAL_EXTRA;
    let on = (2..=reveal + 1).contains(&updates);
    [on && pan > 0, on && pan < 0]
}

/// The panel side of a speaker-panel layer (0 bottom, 1 top).
fn panel_side(layer: &Layer) -> usize {
    let [_, y, _, h] = layer.destination.map(|v| v as i32);
    usize::from(y + h / 2 < CANVAS_H / 2)
}

/// A panel layer at slide `s` (W+53BA): the face part moves vertically,
/// the bar part horizontally.
fn panel_layer(layer: &Layer, s: i32, dims: &dyn Fn(&str) -> Option<[u32; 2]>) -> Option<Layer> {
    if s <= 0 {
        return None;
    }
    let [x, y, w, h] = layer.destination.map(|v| v as i32);
    let top = panel_side(layer) == 1;
    let s3 = RAW - s / 2;
    let s0 = RAW / 2 - s / 4;
    let face_part = if top { x >= 490 } else { x < 160 };
    let sign = if top { -1 } else { 1 };
    let (dx, dy) = if face_part {
        (0, sign * canvas_dy(s0))
    } else {
        (sign * canvas_dx(s3), 0)
    };
    shifted_layer(layer, [x + dx, y + dy, w, h], dims)
}

/// The speaker-panel layers of `side` in a settled set (after its last
/// letterbox band).
fn panel_layers(set: &[Layer], side: usize) -> Vec<Layer> {
    let is_band = |l: &Layer| {
        let [x, y, w, h] = l.destination;
        l.asset == "story-black" && x == 0 && w == 640 && h < 448 && (y == 0 || y + h == 448)
    };
    let Some(last) = set.iter().rposition(is_band) else {
        return Vec::new();
    };
    set[last + 1..].iter().filter(|l| panel_side(l) == side).cloned().collect()
}

/// The figure's raw x at pan `p` (slot 0 bottom, 1 top) [emu].
pub fn figure_x(rest: i32, slot: usize, p: i32) -> i32 {
    let span = i64::from(RAW - rest);
    let p = i64::from(p);
    let rest = i64::from(rest);
    let x = if slot == 0 {
        rest * 100 - span * (100 - p)
    } else {
        rest * 100 + span * (p + 100)
    };
    x.div_euclid(100) as i32
}

fn opacity(alpha: i32) -> u8 {
    ((alpha.clamp(0, ALPHA_FULL) * 255 + ALPHA_FULL / 2) / ALPHA_FULL) as u8
}

fn canvas_dx(raw: i32) -> i32 {
    (raw * 5 + 2).div_euclid(4)
}
fn canvas_dy(raw: i32) -> i32 {
    (raw * 7 + 4).div_euclid(8)
}
/// Raw edge to canvas edge (so adjacent quads meet), clamped to the screen.
fn edge_x(raw: i32) -> u32 {
    ((raw.clamp(0, RAW) * 5 + 2) / 4) as u32
}
fn edge_y(raw: i32) -> u32 {
    ((raw.clamp(0, RAW) * 7 + 4) / 8) as u32
}

fn black(d: [i32; 4]) -> Layer {
    Layer {
        asset: "story-black".into(),
        source: None,
        destination: d.map(|v| v as u32),
        tint: [255; 3],
        opacity: 255,
        flip_x: false,
        filter: Sampling::Linear,
    }
}

/// The two letterbox bands of `rows` raw rows each.
pub fn letterbox_layers(rows: i32) -> Vec<Layer> {
    let rows = rows.clamp(0, RAW / 2);
    if rows == 0 {
        return Vec::new();
    }
    let top = edge_y(rows) as i32;
    let bottom = edge_y(RAW - rows) as i32;
    vec![
        black([0, 0, CANVAS_W, top]),
        black([0, bottom, CANVAS_W, CANVAS_H - bottom]),
    ]
}

/// The visible part of a raw quad: the destination clipped to the screen
/// and the source cut in proportion (rounded to whole texels); the same
/// cut as the settled import (`import_story_red::painting_layer`).
pub fn raw_quad_layer(asset: &str, uv: [u32; 2], raw: [i32; 4]) -> Option<Layer> {
    let [x, y, w, h] = raw;
    let (top, bottom) = (y.max(0), (y + h).min(RAW));
    let (left, right) = (x.max(0), (x + w).min(RAW));
    if bottom <= top || right <= left {
        return None;
    }
    let cut = |edge: i32, origin: i32, extent: i32, texels: u32| -> u32 {
        (((edge - origin) as i64 * i64::from(texels) + i64::from(extent) / 2) / i64::from(extent))
            as u32
    };
    let (u0, u1) = (cut(left, x, w, uv[0]), cut(right, x, w, uv[0]));
    let (v0, v1) = (cut(top, y, h, uv[1]), cut(bottom, y, h, uv[1]));
    let (cx, cy) = (edge_x(left), edge_y(top));
    let (cw, ch) = (edge_x(right) - cx, edge_y(bottom) - cy);
    if u1 <= u0 || v1 <= v0 || cw == 0 || ch == 0 {
        return None;
    }
    Some(Layer {
        asset: asset.into(),
        source: Some([u0, v0, u1 - u0, v1 - v0]),
        destination: [cx, cy, cw, ch],
        tint: [255; 3],
        opacity: 255,
        flip_x: false,
        filter: Sampling::Linear,
    })
}

/// `layer` moved to the canvas rectangle `d` (may extend past the screen):
/// clipped, its source cut in proportion.
fn shifted_layer(layer: &Layer, d: [i32; 4], dims: &dyn Fn(&str) -> Option<[u32; 2]>) -> Option<Layer> {
    let [x, y, w, h] = d;
    if w <= 0 || h <= 0 {
        return None;
    }
    let (l, t) = (x.max(0), y.max(0));
    let (r, b) = ((x + w).min(CANVAS_W), (y + h).min(CANVAS_H));
    if r <= l || b <= t {
        return None;
    }
    let mut out = layer.clone();
    out.destination = [l as u32, t as u32, (r - l) as u32, (b - t) as u32];
    if (l, t, r, b) == (x, y, x + w, y + h) {
        return Some(out);
    }
    let [sx, sy, sw, sh] = match layer.source {
        Some(s) => s,
        None => {
            let [iw, ih] = dims(&layer.asset)?;
            [0, 0, iw, ih]
        }
    };
    let cut = |edge: i32, origin: i32, extent: i32, texels: u32| -> u32 {
        ((i64::from(edge - origin) * i64::from(texels) + i64::from(extent) / 2) / i64::from(extent))
            as u32
    };
    let (mut u0, mut u1) = (cut(l, x, w, sw), cut(r, x, w, sw));
    if layer.flip_x {
        (u0, u1) = (sw - u1, sw - u0);
    }
    let (v0, v1) = (cut(t, y, h, sh), cut(b, y, h, sh));
    if u1 <= u0 || v1 <= v0 {
        return None;
    }
    out.source = Some([sx + u0, sy + v0, u1 - u0, v1 - v0]);
    Some(out)
}

/// The rows of a composed text layer with the reveal's corner alphas:
/// each row as 8 columns x 2 bands, each at the bilinear alpha of its
/// centre (the original Gouraud-shades the row's quad).
fn text_rows(layer: &Layer, alphas: [[u8; 4]; 3], dims: &dyn Fn(&str) -> Option<[u32; 2]>) -> Vec<Layer> {
    let Some([iw, ih]) = dims(&layer.asset) else {
        return vec![layer.clone()];
    };
    let rows = (ih / LINE_ROWS).clamp(1, 3);
    let [dx, dy, dw, dh] = layer.destination;
    let mut out = Vec::new();
    const COLS: u32 = 8;
    const BANDS: u32 = 2;
    for r in 0..rows {
        let a = alphas[r as usize];
        if a.iter().all(|&v| v == 0) {
            continue;
        }
        let full = a.iter().all(|&v| v >= 0x80);
        let (sy0, sy1) = (r * LINE_ROWS, ((r + 1) * LINE_ROWS).min(ih));
        let y_of = |sy: u32| dy + (u64::from(sy) * u64::from(dh) / u64::from(ih)) as u32;
        if full {
            let (y0, y1) = (y_of(sy0), y_of(sy1));
            if y1 > y0 {
                let mut l = layer.clone();
                l.source = Some([0, sy0, iw, sy1 - sy0]);
                l.destination = [dx, y0, dw, y1 - y0];
                out.push(l);
            }
            continue;
        }
        for band in 0..BANDS {
            let by0 = sy0 + (sy1 - sy0) * band / BANDS;
            let by1 = sy0 + (sy1 - sy0) * (band + 1) / BANDS;
            let (y0, y1) = (y_of(by0), y_of(by1));
            if y1 <= y0 || by1 <= by0 {
                continue;
            }
            for c in 0..COLS {
                let sx0 = iw * c / COLS;
                let sx1 = iw * (c + 1) / COLS;
                let x0 = dx + dw * c / COLS;
                let x1 = dx + dw * (c + 1) / COLS;
                if sx1 <= sx0 || x1 <= x0 {
                    continue;
                }
                let fx = (f64::from(c) + 0.5) / f64::from(COLS);
                let fy = (f64::from(band) + 0.5) / f64::from(BANDS);
                let [tl, tr, bl, br] = a.map(f64::from);
                let v = (tl * (1.0 - fx) + tr * fx) * (1.0 - fy) + (bl * (1.0 - fx) + br * fx) * fy;
                let o = opacity(v.round() as i32);
                if o == 0 {
                    continue;
                }
                let mut l = layer.clone();
                l.source = Some([sx0, by0, sx1 - sx0, by1 - by0]);
                l.destination = [x0, y0, x1 - x0, y1 - y0];
                l.opacity = o;
                out.push(l);
            }
        }
    }
    out
}

/// The row reveal of layouts 0/2 (0x230B94..0x230D0C) after `updates`
/// updates: each update walks the rows, then TL, TR, BL, BR, adding 0x30
/// (clamped to 0x80) where the gate holds, the gates seeing this pass's
/// values: TL of row 0 always, of row r when row r−1's TR ≥ 0x80; BL when
/// TL ≥ 0x41; TR when BL ≥ 0x80; BR when TR ≥ 0x21. 1/2/3-line pages
/// finish in 7/13/19 updates [F, emu the project notes].
pub fn reveal_alphas(rows: usize, updates: u32) -> [[u8; 4]; 3] {
    let mut a = [[0u8; 4]; 3];
    let rows = rows.clamp(1, 3);
    let step = |v: &mut u8| *v = (*v + 0x30).min(0x80);
    for _ in 0..updates.min(64) {
        for r in 0..rows {
            if r == 0 || a[r - 1][1] >= 0x80 {
                step(&mut a[r][0]);
            }
            if a[r][2] >= 0x80 {
                step(&mut a[r][1]);
            }
            if a[r][0] >= 0x41 {
                step(&mut a[r][2]);
            }
            if a[r][1] >= 0x21 {
                step(&mut a[r][3]);
            }
        }
        if a[rows - 1][3] >= 0x80 {
            break;
        }
    }
    a
}

/// The page-turn fade of layout 0 (0x230D14..): every alpha −0x18 per update.
pub fn fade_alphas(mut a: [[u8; 4]; 3], updates: u32) -> [[u8; 4]; 3] {
    let k = (updates.min(16) * 0x18).min(255) as u8;
    for row in &mut a {
        for v in row.iter_mut() {
            *v = v.saturating_sub(k);
        }
    }
    a
}

#[cfg(test)]
mod tests;
