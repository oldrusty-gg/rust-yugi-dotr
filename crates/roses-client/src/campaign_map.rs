//! Campaign map (Lancastrian route): the first steps from "Choose a
//! destination" to the Weevil duel and back, as a frame-stepped model without
//! any renderer. Source facts (PAL SLES_515.79, private research
//! `story/claude-campaign-map/MAP.md`, tags [F] code/data, [emu] the game's
//! own code run frame by frame, [H] hypothesis):
//!
//! - Node table 0x297F80 (22 × 0x34) and path table 0x298400 (2 sides × 16 ×
//!   0x4C) are executable data; the importer reads them from the user's
//!   executable. Path dots are the runtime spline samples of 0x21B490: a
//!   natural cubic spline through the waypoints (uniform parameter), sampled
//!   per segment at `floor(chord / 4.0)` points `t = i / n`, or, on sea routes
//!   (curve flag 1), nine points `t = i / 10`; the samples reproduce the dumped
//!   runtime dots (`map-data.json`) to the dump's four decimals [F for the
//!   spacing and counts, formula fitted to every dumped path].
//! - Camera eye = target + (0, 112, 112) looking at the target on y = 0; the
//!   cursor-space projection is x = 256 + 800·Xc/Zc, y = 256 − 1074.3·Yc/Zc,
//!   anchors are floored to integers (five anchors of state 622 reproduced).
//!   A node is hovered when the cursor is closer than the projected length of
//!   five world units; eligible nodes are 0, 11 and every red node with a
//!   nonzero state (0x23AC80). Cursor 16 px per held direction, 0..=0x1E0; at
//!   an edge the camera target moves 4.0 per frame.
//! - Cross on a hovered node (0x2397D0..0x239BD8): node state 1 starts the
//!   encounter text (base table 0x2D2530, pages base+1.. until a message whose
//!   first code is 0xA001) and the travel; states 2/3 travel without text. The
//!   next frame sets the destination, starts the leader walk along the path
//!   between location and destination (4 frames per dot) and a 32-frame camera
//!   lerp. The duel is requested when the travel is done, the window closed and
//!   the walk arrived: parameters (opponent, side, 1100, 1000 + opponent).
//! - Duel end 0x227F00 writes the counters and C2 = win/loss event of the node;
//!   the map re-enters with the camera on the node, waits for its fade-in,
//!   shows the win/loss text, counts 100 → 0 by 10, plays the 128-frame
//!   rose-card animation on a win and then (0x230F8C) sets the node beaten and
//!   C2 −= 200 (303 → 103 = reveal of the node's paths, 0x228210) or, on a
//!   loss, node state 2 and C2 = 0. The reveal pans 16 frames to the node,
//!   marks the listed paths (dots appear 4 frames apart, each fading in over
//!   16 frames), pans 32 frames to the centroid of the new end nodes, and each
//!   finished path raises its end node over 64 frames to state 1.
//!
//! Frame numbers of the model match the emulator logs in
//! `story/claude-campaign-map/logs/` (hover-622, pre-duel-652, post-win-767,
//! post-loss-767, rematch-after-loss); the in-frame order of the updaters and
//! the four-frame dot start are reconstructed from those logs [H].
//!
//! The rest of the red route (`RED-ROUTE.md`, [emu] logs lancashire-win/loss,
//! exeter-win, bosworth-trigger, bosworth-win, seto-win/loss,
//! manawyddan-win/loss made with `tools/run_red.py` from a patched post-duel
//! state): the same return and reveal code for every node (103..=109 reveal
//! nodes 2..=8); arrival events 110..=113 (0x228D00) keep C2 and free the
//! cursor after two 16-frame pans; dialogue pages skip blank messages and
//! close after 4·groups (W+537E); the Bosworth trigger inside 0x228410
//! (eight of nodes 2..=10 beaten, path 12 undrawn → C2 = 11120); the close
//! logic's specials (308 → 11140, 312 → 11160 with state 3, 313 → 11180 with
//! state 5, 213 → state 4); event 11124's path writes and Bosworth's rise;
//! Stonehenge's two visits (hover/text branches on state ≥ 3 and W+53C4 ==
//! 1, the C9 mirror written on the first map frame after a story event or
//! return pan and copied to C9 at travel start) and its win/loss events
//! 313/213 from the duel end. The window parts of the Bosworth events
//! (11120, 11124, 11149: letterbox, centred window, Cross wait over the
//! running map) are played by the model itself (`story_window`, frame by
//! frame against the emulator traces); the painting and dialogue events
//! (11122, 11140–11148, 11160–11184) are played by the frontend from
//! `Stage::Story` pages (`roses import-story-red`): the model suspends
//! (`Handler::Story`) and resumes at the continuation event (11124, 11149,
//! 112) through `resume`, which rebuilds the map from the save [H: the
//! original never leaves the map mode there; no fade or camera motion of
//! that moment is traced].
//!
//! Not modelled (explicit boundaries): the □ scroll mode and SYSTEM icons
//! 1–3, the DC check needs the deck import (applied only when both DCs are
//! given), nodes 15/16/17, the hover panel's extra gate 0x235398, the
//! free-duel mode after the ending (SAVE[1] = 1, save op 2, movie EDMV_A6),
//! the white route, the ship, node shake and sounds beyond their cue
//! numbers, HUD/panel art, the painting/dialogue story events' own
//! presentation (paintings, scene bank, effect tasks: the story scene's).
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEMA: &str = "roses-campaign-map-import-1";
pub const NODE_COUNT: usize = 22;
/// Nodes 0..=11 form the red (Lancastrian) route.
pub const RED_NODES: usize = 12;
pub const PATHS_PER_SIDE: usize = 16;
pub const SIDES: usize = 2;
pub const NODE_TABLE_EE: u32 = 0x0029_7F80;
pub const NODE_RECORD_BYTES: usize = 0x34;
pub const PATH_TABLE_EE: u32 = 0x0029_8400;
pub const PATH_RECORD_BYTES: usize = 0x4C;
pub const PATH_SIDE_BYTES: usize = 0x4C0;
pub const MAX_WAYPOINTS: usize = 16;
pub const MAX_DOTS: usize = 64;
/// ELF file offset = EE address − this.
pub const EE_TO_FILE: u32 = 0x000F_9680;
pub const DOT_SPACING: f64 = 4.0;
pub const SEA_SAMPLES_PER_SEGMENT: usize = 9;
pub const SEA_SAMPLE_DIVISOR: f64 = 10.0;

pub const CAMERA_EYE_OFFSET: [f64; 3] = [0.0, 112.0, 112.0];
/// Anchor = node position + (0, 8.65, 0) (0x23AC80).
pub const ANCHOR_HEIGHT: f64 = 8.65;
pub const PROJECTION_X: f64 = 800.0;
pub const PROJECTION_Y: f64 = 1_074.285_522_460_937_5;
pub const SCREEN_CENTRE: f64 = 256.0;
pub const HOVER_UNITS: f64 = 5.0;
pub const CURSOR_START: (i16, i16) = (256, 222);
pub const CURSOR_MAX: i16 = 0x1E0;
pub const CURSOR_SPEED: i16 = 16;
pub const EDGE_PAN: f64 = 4.0;

pub const FADE_STEP: i32 = 20;
pub const FADE_FULL: i32 = 0x100;
pub const LERP_STEP: i32 = 8;
pub const LERP_FULL: i32 = 0x100;
pub const WALK_STEP: i32 = 0x100;
pub const WALK_DOT: i32 = 0x400;
/// (C7, C8, path) of the walk switch 0x2D2420 [F 0x22AE20..0x22B1B8].
pub const WALK_PATHS: [(u8, u8, u8); 15] = [
    (1, 8, 2),
    (1, 2, 1),
    (2, 7, 12),
    (2, 5, 4),
    (2, 3, 3),
    (3, 2, 3),
    (5, 7, 13),
    (5, 6, 10),
    (5, 2, 4),
    (6, 5, 10),
    (7, 11, 14),
    (8, 7, 11),
    (8, 6, 5),
    (9, 8, 6),
    (11, 7, 14),
];
/// The walk with no path (0x22BDD0): W+1DA += 16 per frame up to 1024.
pub const STRAIGHT_WALK_STEP: i32 = 16;
pub const STRAIGHT_WALK_FULL: i32 = 1024;
pub const WINDOW_STEP: i32 = 20;
pub const WINDOW_OPEN: i32 = 100;
/// Page-turn pause after Cross (phase 5) before the next page opens.
pub const PAGE_TURN_FRAMES: u32 = 5;
/// The style-2 speaker window (event 111's 3300): opening 15 frames,
/// the first page 9 frames longer to reveal, phase 13 for 24 frames
/// [emu logs/white-brest-win f289..f364; H: the rule behind the fit].
pub const STYLE2_WINDOW_STEP: i32 = 7;
pub const STYLE2_FIRST_REVEAL_EXTRA: u32 = 9;
pub const STYLE2_CLOSE_FRAMES: u32 = 24;
/// Text reveal: six frames per line plus one (19, 13 and 7 frames for the
/// traced three-, two- and one-line pages) [H: fitted to four traced pages].
pub const REVEAL_FRAMES_PER_LINE: u32 = 6;
pub const REVEAL_EXTRA_FRAMES: u32 = 1;
pub const DOT_ALPHA_STEP: i32 = 0x10;
pub const DOT_FULL_ALPHA: i32 = 0x100;
/// Dot k of a revealing path starts fading in 4·(k + 1) frames after the
/// path state was set [H: fitted to paths 3 and 4 of the post-win log].
pub const DOT_START_FRAMES: u32 = 4;
pub const RISE_STEP: i32 = 4;
pub const RISE_FULL: i32 = 0x100;
pub const COUNTDOWN_START: i32 = 100;
pub const COUNTDOWN_STEP: i32 = 10;
pub const ROSE_SCALE_STEP: i32 = 0x20;
pub const ROSE_SCALE_FULL: i32 = 0x1000;
pub const ROSE_SPIN_STEP: i32 = 0x1000;
pub const REVEAL_NODE_FRAMES: u32 = 16;
pub const REVEAL_CENTROID_FRAMES: u32 = 32;
pub const HUD_ALPHA_FULL: i32 = 0x80;
/// W+224 / W+226 dim to 96 by 8 per frame while the cursor is above
/// y 120 / at or below y 384 (0x23B200).
pub const HUD_ALPHA_DIM: i32 = 96;
pub const HUD_ALPHA_STEP: i32 = 8;
pub const HUD_TOP_DIM_Y: i16 = 120;
pub const HUD_BOTTOM_DIM_Y: i16 = 384;
/// W+70 ±20 per frame between 0 and 256 (0x237108..0x23717C).
pub const HUD_SLIDE_STEP: i32 = 20;
pub const HUD_SLIDE_FULL: i32 = 256;
/// W+82 + 2i: +4 per frame to 16 while the icon is hovered, −4 to 0.
pub const HUD_ICON_GROW_STEP: i32 = 4;
pub const HUD_ICON_GROW_FULL: i32 = 16;
pub const DUEL_PARAM_MODE: i32 = 1100;
pub const DUEL_PARAM_BASE: i32 = 1000;
/// Nodes the Cross handler refuses (C6 ∉ {0, 1, 15, 17}).
pub const NOT_SELECTABLE: [u8; 4] = [0, 1, 15, 17];
/// A message whose first raw code is this blank glyph ends a page list
/// (0x22E630); the message decoder expands it to `BLANK_GLYPH`.
pub const BLANK_CODE: u16 = 0xA001;
pub const BLANK_GLYPH: u16 = 0x2001;
pub const CAPTION_MESSAGE: u16 = 2520;
pub const ROSE_CARD_MESSAGE: u16 = 259;
pub const OVER_DC_MESSAGE: u16 = 250;
pub const SFX_SELECT: u16 = 4;
pub const SFX_PAGE: u16 = 2;
pub const SFX_OVER_DC: u16 = 6;
pub const SFX_RISE: u16 = 0x194;
pub const MUSIC_MAP: u16 = 4;
pub const MUSIC_REENTRY: u16 = 26;
pub const MAX_DUEL_COUNT: u8 = 5;
/// SYSTEM icon rectangles (0x297640, four records of 0x26 bytes: x0, y0,
/// x1, y1 as s16 in cursor space) read by the hover update 0x23AC80 and the
/// map update 0x2373FC; the cursor hovers an icon within a 2-px margin.
pub const SYSTEM_ICON_TABLE_EE: u32 = 0x0029_7640;
pub const SYSTEM_ICON_RECORD_BYTES: usize = 0x26;
pub const SYSTEM_ICONS: usize = 4;
pub const SYSTEM_ICON_MARGIN: i16 = 2;
/// Mode pushed by SYSTEM icon 0 (0x1A91E0(6, side)): the deck editor.
pub const DECK_EDITOR_MODE: u8 = 6;
/// SFX of the icon press (0x216EA0(0, 4)).
pub const SFX_SYSTEM_ICON: u16 = 4;

pub const STATE_HIDDEN: u8 = 0;
pub const STATE_OPEN: u8 = 1;
pub const STATE_LOST: u8 = 2;
pub const STATE_BEATEN: u8 = 3;
/// Stonehenge only (SAVE[3+11], RED-ROUTE.md §4): lost to / beat Manawyddan.
pub const STATE_LOST_SECOND: u8 = 4;
pub const STATE_BEATEN_SECOND: u8 = 5;

/// Dialogue pages per group (W+537E): the page counter runs base+1.. with
/// blank messages skipped and closes once it exceeds 4·groups (0x22E630;
/// [emu] logs/lancashire-win: 2632, 2636, 2640, 2641, 2644 of groups 4).
pub const PAGES_PER_GROUP: u16 = 4;
/// Arrival events 110..=113 (0x228D00): after the close logic's camera pan
/// (W+60 += 16 to 0x100, then one frame) 0x228410 runs (node state 3, the
/// Bosworth trigger) and a second 16-frame pan follows; C2 stays at the
/// event and the handler behaves as event 0 [emu, logs/exeter-win: C2 = 110
/// at f189, free at f224].
pub const ARRIVE_PAN_FRAMES: u32 = 17;
/// The close logic's own pan plus the frame on which sub-step 1 calls
/// 0x228410 (C2 = 110 at f189 → the call at f207; C2 = 111 at f203 → the
/// Bosworth trigger at f221).
pub const ARRIVE_FIRST_FRAMES: u32 = ARRIVE_PAN_FRAMES + 1;
pub const BOSWORTH_NODE: u8 = 7;
pub const STONEHENGE_NODE: u8 = 11;
/// Path 12 (Chester → Bosworth): its undrawn state gates the Bosworth trigger.
pub const BOSWORTH_PATH: u8 = 12;
/// The trigger fires inside 0x228410 when at least this many of nodes 2..=10
/// are beaten (RED-ROUTE.md §1.1; [emu] logs/bosworth-trigger at f221).
pub const BOSWORTH_TRIGGER_COUNT: usize = 8;
/// Paths 11, 12, 13 in the order event 11124 sets their states (node 8's,
/// node 2's and node 5's extra list entries, RED-ROUTE.md §5).
pub const BOSWORTH_PATHS: [u8; 3] = [12, 13, 11];
/// Map events after a story event (the map model resumes here).
pub const EVENT_BOSWORTH_TRIGGER: i16 = 11120;
pub const EVENT_BOSWORTH_REVEAL: i16 = 11124;
pub const EVENT_BOSWORTH_WIN: i16 = 11140;
pub const EVENT_STONEHENGE_REVEAL: i16 = 108;
pub const EVENT_SETO_WIN: i16 = 11160;
pub const EVENT_STONEHENGE_ARRIVE: i16 = 112;
pub const EVENT_MANAWYDDAN_WIN: i16 = 11180;
pub const EVENT_FIRST_REVEAL: i16 = 103;
pub const EVENT_LAST_REVEAL: i16 = 109;
pub const EVENT_FIRST_ARRIVE: i16 = 110;
pub const EVENT_LAST_ARRIVE: i16 = 113;
/// Win/loss events of the second Stonehenge duel (0x227F00 when C6 == 11 and
/// C9 != 0; the dispatcher's 213/313 cases store C9 = 1, 212/312 store 0).
pub const EVENT_MANAWYDDAN_WON: i16 = 313;
pub const EVENT_MANAWYDDAN_LOST: i16 = 213;
/// Story events the map closes its letterbox over before the story scene
/// loads (`Handler::StoryEntry`).
pub const SCENE0_ENTRIES: [i16; 3] = [EVENT_SETO_WIN, EVENT_MANAWYDDAN_WIN, white::EVENT_WHITE_WIN];
/// Updates the entry's letterbox waits for the return panel (-25 per
/// update from y 0 to -300).
pub const STORY_ENTRY_HOLD: u32 = 12;
/// Stonehenge-scene events whose tail opens the letterbox over the map
/// (`Handler::StoryExit`): 11162 -> 112, 12132 -> 0.
pub const STORY_EXIT_EVENTS: [i16; 2] = [11162, 12132];
/// The story events whose pages a frontend plays before the map resumes
/// (11120, 11124 and 11149 are played by the map: `story_window`).
pub const STORY_EVENTS: [i16; 4] = [
    EVENT_BOSWORTH_PAINTINGS,
    EVENT_BOSWORTH_WIN,
    EVENT_SETO_WIN,
    EVENT_MANAWYDDAN_WIN,
];

pub type Result<T> = std::result::Result<T, String>;

// ---------------------------------------------------------------- data
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapNode {
    pub index: u8,
    pub name: String,
    pub position: [f32; 3],
    /// +0xC (6.0/7.0 on nodes 15/16, else 0; meaning untraced).
    pub facing_raw: f32,
    /// +0x10 base model (slot = model + 34).
    pub base_model: i16,
    /// +0x12 castle model (slot = model + 34).
    pub castle_model: i16,
    pub red_paths: [u16; 4],
    pub white_paths: [u16; 4],
    pub path_count: u16,
    /// +0x26: 1 = the path's forward end, 2 = its alternate end.
    pub path_dirs: [u16; 4],
    pub win_event: i16,
    pub loss_event: i16,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapPath {
    pub side: u8,
    pub index: u8,
    pub waypoints: Vec<[i16; 2]>,
    /// +0x42 curve flag: ship route, dense sampling, no dots drawn.
    pub sea_route: bool,
    pub ends: [u16; 2],
    pub alternate_ends: [u16; 2],
    /// Runtime spline samples (x, z), see `spline_dots`.
    pub dots: Vec<[f32; 2]>,
}

/// Per-node constants of the hover switch 0x2D2630 and the three text-base
/// switches (decoded from the executable's straight-line code at import).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeText {
    pub node: u8,
    pub speaker_message: Option<u16>,
    pub place_message: Option<u16>,
    /// speaker − 197 (SAVE+0xC4).
    pub opponent: Option<u8>,
    pub encounter_base: Option<u16>,
    pub encounter_groups: Option<u16>,
    pub loss_base: Option<u16>,
    pub win_base: Option<u16>,
    /// Page groups of the loss/win texts (W+537E, absent in older imports:
    /// one group).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loss_groups: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub win_groups: Option<u16>,
    /// The switch code branches on save state; the fields above hold the
    /// first-visit branch (Stonehenge: state < 3, Seto).
    pub conditional: bool,
    /// Stonehenge's second-visit branch (state ≥ 3 and W+53C4 == 1:
    /// Manawyddan; the loss/win texts of events 213/313), evaluated from
    /// the executable's switch code at import.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub second_visit: Option<NodeTextVariant>,
}

/// A node's hover/text constants under one save condition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeTextVariant {
    pub speaker_message: Option<u16>,
    pub place_message: Option<u16>,
    pub opponent: Option<u8>,
    pub encounter_base: Option<u16>,
    pub encounter_groups: Option<u16>,
    pub loss_base: Option<u16>,
    pub loss_groups: Option<u16>,
    pub win_base: Option<u16>,
    pub win_groups: Option<u16>,
}

impl NodeText {
    pub fn first_visit(&self) -> NodeTextVariant {
        NodeTextVariant {
            speaker_message: self.speaker_message,
            place_message: self.place_message,
            opponent: self.opponent,
            encounter_base: self.encounter_base,
            encounter_groups: self.encounter_groups,
            loss_base: self.loss_base,
            loss_groups: self.loss_groups,
            win_base: self.win_base,
            win_groups: self.win_groups,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageInfo {
    pub blank: bool,
    /// Codes other than the 0x2000 line separator.
    pub glyphs: u16,
    pub lines: u8,
    /// Transcribed characters ('?' for cells without a transcription).
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapData {
    pub nodes: Vec<MapNode>,
    /// Side-major: `paths[side * 16 + index]`.
    pub paths: Vec<MapPath>,
    pub texts: Vec<NodeText>,
    /// Nodes counted as rose cards when their state is ≥ 3 (table 0x2D25A0).
    pub rose_card_nodes: Vec<u8>,
    pub messages: BTreeMap<u16, MessageInfo>,
    /// The White Rose side's node constants (SAVE[0] = 1): the hover
    /// switch 0x2D2630 and the text start 0x22CF00 run with side 1 at
    /// import (nodes 0 and 11..=21). Empty in imports made before the
    /// white route: the map then refuses those texts explicitly.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub white_texts: Vec<NodeText>,
    /// The White Rose route's event captions (0x22CF00 with side 1 and C2
    /// = 105 / 111: the voyage captions 3180 and 3300).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub white_captions: Vec<EventCaption>,
}

/// A text the text start 0x22CF00 opens for a map event rather than a
/// node (W+5378 base with the counter W+5380 = 1, W+537E groups, W+537C
/// style; style 2 also sets the bottom speaker W+53BE).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventCaption {
    pub event: i16,
    pub base: u16,
    pub groups: u16,
    /// W+537C: 1 the place caption (four staggered alphas), 2 the speaker
    /// window.
    pub style: u8,
    /// W+53BE (style 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<u16>,
}

impl MapData {
    pub fn validate(&self) -> Result<()> {
        if self.nodes.len() != NODE_COUNT {
            return Err("Campaign map needs 22 nodes".into());
        }
        for (i, node) in self.nodes.iter().enumerate() {
            if usize::from(node.index) != i
                || node
                    .position
                    .iter()
                    .any(|v| !v.is_finite() || v.abs() > 1000.0)
                || !(0..32).contains(&node.base_model)
                || !(0..32).contains(&node.castle_model)
                || node.path_count > 4
                || node
                    .red_paths
                    .iter()
                    .chain(&node.white_paths)
                    .any(|p| usize::from(*p) >= PATHS_PER_SIDE)
                || node.path_dirs.iter().any(|d| *d > 2)
                || node.name.len() > 32
            {
                return Err(format!("Campaign map node {i} is out of range"));
            }
        }
        if self.paths.len() != SIDES * PATHS_PER_SIDE {
            return Err("Campaign map needs 2 × 16 paths".into());
        }
        for (i, path) in self.paths.iter().enumerate() {
            if usize::from(path.side) != i / PATHS_PER_SIDE
                || usize::from(path.index) != i % PATHS_PER_SIDE
                || path.waypoints.len() > MAX_WAYPOINTS
                || path.dots.len() > MAX_DOTS
                || path
                    .ends
                    .iter()
                    .chain(&path.alternate_ends)
                    .any(|n| usize::from(*n) >= NODE_COUNT)
                || path.dots.iter().flatten().any(|v| !v.is_finite())
            {
                return Err(format!("Campaign map path {i} is out of range"));
            }
        }
        if self.texts.len() > NODE_COUNT
            || self.texts.iter().any(|t| usize::from(t.node) >= NODE_COUNT)
            || self.white_texts.len() > NODE_COUNT
            || self.white_texts.iter().any(|t| usize::from(t.node) >= NODE_COUNT)
            || self.white_captions.len() > 8
            || self.white_captions.iter().any(|c| c.groups > 16 || c.style > 2)
            || self.rose_card_nodes.len() > RED_NODES
            || self
                .rose_card_nodes
                .iter()
                .any(|n| usize::from(*n) >= RED_NODES)
        {
            return Err("Campaign map text or rose tables are out of range".into());
        }
        if self.messages.len() > 1024 || self.messages.values().any(|m| m.text.len() > 512) {
            return Err("Campaign map message table is too large".into());
        }
        Ok(())
    }
    pub fn node(&self, index: u8) -> &MapNode {
        &self.nodes[usize::from(index)]
    }
    pub fn path(&self, side: u8, index: u8) -> &MapPath {
        &self.paths[usize::from(side) * PATHS_PER_SIDE + usize::from(index)]
    }
    pub fn text(&self, node: u8) -> Option<&NodeText> {
        self.texts.iter().find(|t| t.node == node)
    }
    /// The node's constants for a visit: Stonehenge with state ≥ 3 and
    /// W+53C4 == 1 takes the second-visit branch (hover 0x235004..,
    /// text start 0x22D030), every other case the first (RED-ROUTE.md §4).
    pub fn text_variant(&self, node: u8, state: u8, w53c4: u8) -> Option<NodeTextVariant> {
        let text = self.text(node)?;
        if node == STONEHENGE_NODE && state >= STATE_BEATEN && w53c4 == 1 {
            return text.second_visit.clone();
        }
        Some(text.first_visit())
    }
    /// The node's constants on `side`'s map: the red side's
    /// (`text_variant`) or the White Rose side's (one visit per node;
    /// `None` also when the import predates the white texts).
    pub fn side_text(&self, side: u8, node: u8, state: u8, w53c4: u8) -> Option<NodeTextVariant> {
        if side == 0 {
            return self.text_variant(node, state, w53c4);
        }
        self.white_texts
            .iter()
            .find(|t| t.node == node)
            .map(NodeText::first_visit)
    }
    /// The White Rose caption of map event `event` (105, 111).
    pub fn white_caption(&self, event: i16) -> Option<&EventCaption> {
        self.white_captions.iter().find(|c| c.event == event)
    }
    /// The node a path of `side` reveals: side 0 follows the direction
    /// (`reveal_end`); side 1 always reads the forward end +0x46
    /// (0x228880 mode 2 and the dot updater, 0x228B18) [F].
    pub fn reveal_end_on(&self, side: u8, path: u8, dir: u16) -> u8 {
        if side == 0 {
            self.reveal_end(path, dir)
        } else {
            self.path(1, path).ends[1] as u8
        }
    }
    /// Pages of a dialogue: base+1.. with blank messages skipped, closing
    /// once the counter exceeds 4·groups (`None` for a page that was not
    /// imported, where the model closes the window and notes it).
    pub fn dialogue_pages(&self, base: u16, groups: u16) -> (Vec<u16>, Option<u16>) {
        let mut pages = Vec::new();
        for counter in 1..=groups.max(1) * PAGES_PER_GROUP {
            let id = base + counter;
            match self.is_blank(id) {
                Some(true) => {}
                Some(false) => pages.push(id),
                None => return (pages, Some(id)),
            }
        }
        (pages, None)
    }
    pub fn is_blank(&self, message: u16) -> Option<bool> {
        self.messages.get(&message).map(|m| m.blank)
    }
    /// Pages of a text base: base+1, base+2, … until a blank or unimported
    /// message (an unimported page is reported as `None`).
    pub fn pages(&self, base: u16) -> (Vec<u16>, Option<u16>) {
        let mut pages = Vec::new();
        for page in 1..=16u16 {
            let id = base + page;
            match self.is_blank(id) {
                Some(true) => return (pages, None),
                Some(false) => pages.push(id),
                None => return (pages, Some(id)),
            }
        }
        (pages, None)
    }
    /// The red path joining two nodes, and whether it runs forward from `from`.
    /// The path the leader figure walks from C7 to C8: only the pairs of the
    /// switch 0x2D2420 (0x22AE20 on C7, then C8) select one; every other
    /// pair, even one a drawn path joins (3→4 on path 7), walks straight
    /// (0x22BB58). The direction is the path's own.
    pub fn walk_path(&self, from: u8, to: u8) -> Option<(u8, bool)> {
        let (_, _, index) = WALK_PATHS
            .iter()
            .find(|(a, b, _)| (*a, *b) == (from, to))?;
        self.path_between(from, to)
            .filter(|(found, _)| found == index)
    }

    pub fn path_between(&self, from: u8, to: u8) -> Option<(u8, bool)> {
        for index in 0..PATHS_PER_SIDE {
            let path = &self.paths[index];
            let (a, b) = (path.ends[0] as u8, path.ends[1] as u8);
            if path.waypoints.is_empty() {
                continue;
            }
            if (a, b) == (from, to) {
                return Some((index as u8, true));
            }
            if (a, b) == (to, from) {
                return Some((index as u8, false));
            }
        }
        None
    }
    /// The node a path reveals when entered with direction `dir` (1 = forward
    /// end, 2 = alternate end).
    pub fn reveal_end(&self, path: u8, dir: u16) -> u8 {
        let p = self.path(0, path);
        if dir == 2 {
            p.alternate_ends[1] as u8
        } else {
            p.ends[1] as u8
        }
    }
}

// ---------------------------------------------------------------- spline
/// Tangents of the natural cubic spline through `points` (uniform
/// parameter): 2m₀ + m₁ = 3c₀, mₖ₋₁ + 4mₖ + mₖ₊₁ = 3(cₖ₋₁ + cₖ), mₙ₋₁ + 2mₙ =
/// 3cₙ₋₁ with cₖ = pₖ₊₁ − pₖ.
pub fn natural_spline_tangents(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let n = points.len();
    if n < 2 {
        return vec![[0.0; 2]; n];
    }
    let chord = |k: usize| {
        [
            points[k + 1][0] - points[k][0],
            points[k + 1][1] - points[k][1],
        ]
    };
    let mut diag = vec![4.0; n];
    diag[0] = 2.0;
    diag[n - 1] = 2.0;
    let mut rhs: Vec<[f64; 2]> = (0..n)
        .map(|k| {
            if k == 0 {
                let c = chord(0);
                [3.0 * c[0], 3.0 * c[1]]
            } else if k == n - 1 {
                let c = chord(n - 2);
                [3.0 * c[0], 3.0 * c[1]]
            } else {
                let (a, b) = (chord(k - 1), chord(k));
                [3.0 * (a[0] + b[0]), 3.0 * (a[1] + b[1])]
            }
        })
        .collect();
    // Thomas algorithm (sub- and super-diagonal are 1).
    for k in 1..n {
        let w = 1.0 / diag[k - 1];
        diag[k] -= w;
        rhs[k][0] -= w * rhs[k - 1][0];
        rhs[k][1] -= w * rhs[k - 1][1];
    }
    let mut m = vec![[0.0; 2]; n];
    m[n - 1] = [rhs[n - 1][0] / diag[n - 1], rhs[n - 1][1] / diag[n - 1]];
    for k in (0..n - 1).rev() {
        m[k] = [
            (rhs[k][0] - m[k + 1][0]) / diag[k],
            (rhs[k][1] - m[k + 1][1]) / diag[k],
        ];
    }
    m
}

fn hermite(p0: [f64; 2], p1: [f64; 2], m0: [f64; 2], m1: [f64; 2], t: f64) -> [f64; 2] {
    let h00 = 2.0 * t * t * t - 3.0 * t * t + 1.0;
    let h10 = t * t * t - 2.0 * t * t + t;
    let h01 = -2.0 * t * t * t + 3.0 * t * t;
    let h11 = t * t * t - t * t;
    std::array::from_fn(|i| h00 * p0[i] + h10 * m0[i] + h01 * p1[i] + h11 * m1[i])
}

/// Runtime dot positions of a path (0x21B490 at story init, spacing 4.0).
pub fn spline_dots(waypoints: &[[i16; 2]], sea_route: bool) -> Vec<[f32; 2]> {
    let points: Vec<[f64; 2]> = waypoints
        .iter()
        .map(|w| [f64::from(w[0]), f64::from(w[1])])
        .collect();
    if points.len() < 2 {
        return Vec::new();
    }
    let tangents = natural_spline_tangents(&points);
    let mut dots = Vec::new();
    for k in 0..points.len() - 1 {
        let (p0, p1) = (points[k], points[k + 1]);
        let chord = ((p1[0] - p0[0]).powi(2) + (p1[1] - p0[1]).powi(2)).sqrt();
        let (count, divisor) = if sea_route {
            (SEA_SAMPLES_PER_SEGMENT, SEA_SAMPLE_DIVISOR)
        } else {
            let n = (chord / DOT_SPACING).floor() as usize;
            (n, n as f64)
        };
        for i in 0..count {
            if dots.len() >= MAX_DOTS {
                return dots;
            }
            let p = hermite(p0, p1, tangents[k], tangents[k + 1], i as f64 / divisor);
            dots.push([p[0] as f32, p[1] as f32]);
        }
    }
    dots
}

// ---------------------------------------------------------------- camera
pub fn camera_eye(target: [f64; 2]) -> [f64; 3] {
    [
        target[0] + CAMERA_EYE_OFFSET[0],
        CAMERA_EYE_OFFSET[1],
        target[1] + CAMERA_EYE_OFFSET[2],
    ]
}

/// Camera-space (right, down, forward) of a world point for the map camera:
/// forward = eye → target (0, −1, −1)/√2, right = +x, down = (0, −1, 1)/√2.
pub fn camera_space(target: [f64; 2], point: [f64; 3]) -> [f64; 3] {
    let eye = camera_eye(target);
    let r = [point[0] - eye[0], point[1] - eye[1], point[2] - eye[2]];
    let a = std::f64::consts::FRAC_1_SQRT_2;
    [r[0], -a * r[1] + a * r[2], -a * r[1] - a * r[2]]
}

/// Cursor-space projection (x right, y down, origin top-left of the 512 raw
/// canvas) of a point in front of the camera.
pub fn project(target: [f64; 2], point: [f64; 3]) -> Option<[f64; 2]> {
    let c = camera_space(target, point);
    (c[2] > 1e-6).then(|| {
        [
            SCREEN_CENTRE + PROJECTION_X * c[0] / c[2],
            SCREEN_CENTRE + PROJECTION_Y * c[1] / c[2],
        ]
    })
}

/// Integer anchor of a node (floor of the projected anchor point) and the
/// hover radius (projected length of five world units) at this camera target.
pub fn node_anchor(target: [f64; 2], node: &MapNode) -> Option<((i32, i32), f64)> {
    let point = [
        f64::from(node.position[0]),
        f64::from(node.position[1]) + ANCHOR_HEIGHT,
        f64::from(node.position[2]),
    ];
    let c = camera_space(target, point);
    let p = project(target, point)?;
    Some((
        (p[0].floor() as i32, p[1].floor() as i32),
        PROJECTION_X * HOVER_UNITS / c[2],
    ))
}

// ---------------------------------------------------------------- save
/// The campaign bytes the map reads and writes (SAVE = 0x2991F0).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignSave {
    /// SAVE[0]: 0 Lancastrians, 1 Yorkists (also duel parameter 2).
    pub side: u8,
    /// SAVE[3..0x19]: 0 hidden, 1 open, 2 lost there, 3 beaten.
    pub node_states: Vec<u8>,
    /// SAVE+0xC2 event: 0 free map, 303/203 return from Chester, 103 reveal.
    pub event: i16,
    /// SAVE+0xC4 opponent (speaker − 197).
    pub opponent: i16,
    /// SAVE+0xC6 hovered/current node.
    pub hovered: u8,
    /// SAVE+0xC7 location.
    pub location: u8,
    /// SAVE+0xC8 destination.
    pub destination: u8,
    /// SAVE+0xC9 (copied from W+53C4 at travel).
    pub c9: u8,
    /// SAVE+0xCA duel count (max 5).
    pub duel_count: u8,
    /// SAVE+0x3E / +0x40 totals (HUD WIN/LOSS).
    pub wins: u16,
    pub losses: u16,
    /// SAVE+0x42 + 2·opponent / SAVE+0x82 + 2·opponent.
    pub wins_by_opponent: Vec<u16>,
    pub losses_by_opponent: Vec<u16>,
    /// SAVE[1]: the red route was cleared (event 11184 sub-step 3,
    /// 0x221728; `ending::RouteEnding::apply`).
    #[serde(default)]
    pub red_route_cleared: bool,
    /// SAVE+0x3C ≠ 0: the map was entered (0x21C5D8 stores 1). The ending
    /// clears it (0x2216E4) so that Continue starts the other route
    /// (0x21C734; `ending::continue_target`).
    #[serde(default = "map_entered_default")]
    pub map_entered: bool,
    /// SAVE[2]: the White Rose route was cleared (0x222C44). With SAVE[1]
    /// it makes the map the free-duel map (W+18C, 0x21BFBC; `free_duel.rs`).
    #[serde(default)]
    pub white_route_cleared: bool,
}

fn map_entered_default() -> bool {
    true
}

impl CampaignSave {
    /// The state after events 100/101/102 (Brest open, Milford Haven beaten,
    /// Chester and Tewkesbury open, location Milford Haven).
    pub fn after_landing() -> Self {
        let mut node_states = vec![STATE_HIDDEN; NODE_COUNT];
        node_states[0] = STATE_OPEN;
        node_states[1] = STATE_BEATEN;
        node_states[2] = STATE_OPEN;
        node_states[8] = STATE_OPEN;
        Self {
            side: 0,
            node_states,
            event: 0,
            opponent: 0,
            hovered: 1,
            location: 1,
            destination: 0,
            c9: 0,
            duel_count: 0,
            wins: 0,
            losses: 0,
            wins_by_opponent: vec![0; 32],
            losses_by_opponent: vec![0; 32],
            red_route_cleared: false,
            map_entered: true,
            white_route_cleared: false,
        }
    }
    /// The state when the story enters the map at event 100 (story 11000's
    /// end): every node hidden, location and hovered node Stonehenge [emu:
    /// C6 = C7 = 11 and nodes 0 in the 588-x6 log; the counters are the
    /// new game's].
    pub fn before_landing() -> Self {
        Self {
            node_states: vec![STATE_HIDDEN; NODE_COUNT],
            event: EVENT_FIRST_VOYAGE,
            hovered: STONEHENGE_NODE,
            location: STONEHENGE_NODE,
            ..Self::after_landing()
        }
    }
    /// The state when story 12000 enters the map at event 100 (the side
    /// choice's Yorkists row: SAVE[0] = 1, C9 = 2 at 0x21ED44..0x21ED54
    /// [F]): every node hidden, location and hovered node Stonehenge [emu
    /// logs/white-start-12000: C6 = C7 = 11].
    pub fn white_start() -> Self {
        Self {
            side: 1,
            c9: 2,
            destination: STONEHENGE_NODE,
            ..Self::before_landing()
        }
    }
    pub fn state(&self, node: u8) -> u8 {
        self.node_states
            .get(usize::from(node))
            .copied()
            .unwrap_or(STATE_HIDDEN)
    }
    /// Nodes 2..=10 with state ≥ 3 (the Bosworth trigger's count, 0x228644).
    pub fn beaten_count(&self) -> usize {
        (2..=10).filter(|n| self.state(*n) >= STATE_BEATEN).count()
    }
    /// Duel end 0x227F00: location = hovered = destination, duel count + 1
    /// (max 5), totals and per-opponent counters, then C2 = the node's win
    /// or loss event when the node is unbeaten (state < 3) or it is
    /// Stonehenge with C9 == 1 (the Manawyddan duel: 313/213); a rematch at
    /// a beaten node leaves C2 = 0 (RED-ROUTE.md §2.1).
    pub fn apply_duel_end(&mut self, data: &MapData, won: bool) {
        let node = self.hovered;
        self.location = node;
        self.destination = node;
        self.duel_count = (self.duel_count + 1).min(MAX_DUEL_COUNT);
        let opponent = usize::try_from(self.opponent).unwrap_or(0);
        let entry = data.node(node);
        // The C9 exception is the red side's only (`lb a1, 0(v1)` at
        // 0x228074 reads SAVE[0]) [F].
        let second = self.side == 0 && node == STONEHENGE_NODE && self.c9 == 1;
        // Both flags set (the free-duel map): no event, only the counts.
        let first = !self.free_duel() && (self.state(node) < STATE_BEATEN || second);
        if won {
            self.wins = self.wins.saturating_add(1);
            if let Some(v) = self.wins_by_opponent.get_mut(opponent) {
                *v = v.saturating_add(1);
            }
            self.event = match (first, second) {
                (false, _) => 0,
                (true, true) => EVENT_MANAWYDDAN_WON,
                (true, false) => entry.win_event,
            };
        } else {
            self.losses = self.losses.saturating_add(1);
            if let Some(v) = self.losses_by_opponent.get_mut(opponent) {
                *v = v.saturating_add(1);
            }
            self.event = match (first, second) {
                (false, _) => 0,
                (true, true) => EVENT_MANAWYDDAN_LOST,
                (true, false) => entry.loss_event,
            };
        }
    }
    /// Rose cards: listed nodes with state ≥ 3 (0x2344BC; the White Rose
    /// side counts `WHITE_ROSE_NODES`, 0x234594 → 0x2345EC..).
    pub fn rose_cards(&self, data: &MapData) -> usize {
        self.rose_nodes(data)
            .iter()
            .filter(|n| self.state(**n) >= STATE_BEATEN)
            .count()
    }
    /// The rose-card nodes of this save's side.
    pub fn rose_nodes(&self, data: &MapData) -> Vec<u8> {
        if self.side == 0 {
            data.rose_card_nodes.clone()
        } else {
            WHITE_ROSE_NODES.to_vec()
        }
    }
    /// Whether the win text at `node` is followed by the rose card
    /// (0x229260 ph1, table 0x2D23F0): listed nodes, Stonehenge only for
    /// the Seto win (C9 == 0).
    pub fn rose_card_on_win(&self, data: &MapData, node: u8) -> bool {
        if self.side != 0 {
            // 0x2293D4..0x22949C: white nodes 0, 12, 13, 14, 18..=21 [F].
            return WHITE_ROSE_NODES.contains(&node);
        }
        data.rose_card_nodes.contains(&node) && (node != STONEHENGE_NODE || self.c9 == 0)
    }
    /// Paths drawn at (re)entry (0x21C544): every red path of a node with
    /// state ≥ 3, plus paths 11–13 once eight of nodes 2..=10 are beaten
    /// (unless the map is at events 11120..=11124, before 11124 animates
    /// them).
    pub fn drawn_paths(&self, data: &MapData) -> Vec<u8> {
        if self.side != 0 {
            return self.white_drawn_paths();
        }
        let mut out = Vec::new();
        for node in &data.nodes[..RED_NODES] {
            if self.state(node.index) >= STATE_BEATEN {
                for k in 0..usize::from(node.path_count.min(4)) {
                    let p = node.red_paths[k] as u8;
                    if !out.contains(&p) {
                        out.push(p);
                    }
                }
            }
        }
        if self.beaten_count() >= BOSWORTH_TRIGGER_COUNT
            && !(EVENT_BOSWORTH_TRIGGER..=EVENT_BOSWORTH_REVEAL).contains(&self.event)
        {
            for p in BOSWORTH_PATHS {
                if !out.contains(&p) {
                    out.push(p);
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------- model
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pad {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    /// Newly pressed this frame.
    pub cross: bool,
    /// Held this frame (0x203380 mode 1); true on the press frame too.
    pub cross_held: bool,
    /// The system dialogs' presses (Select, Circle, Square, L1/R1, and the
    /// directions as new presses / repeats; `system::MenuPad`).
    #[serde(default)]
    pub menu: MenuPad,
}
impl Pad {
    pub const NONE: Pad = Pad {
        up: false,
        down: false,
        left: false,
        right: false,
        cross: false,
        cross_held: false,
        menu: MenuPad::NONE,
    };
    pub const CROSS: Pad = Pad {
        cross: true,
        cross_held: true,
        ..Pad::NONE
    };
    pub const UP: Pad = Pad {
        up: true,
        ..Pad::NONE
    };
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DuelRequest {
    pub node: u8,
    pub opponent: u8,
    /// (0x232E40 → 0x1A8C90) parameters 1..=4: opponent, side, 1100, 1000 + opponent.
    pub params: [i32; 4],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum MapEvent {
    Sfx(u16),
    /// Track and whether a playing track is kept (0x217110 second argument).
    Music {
        track: u16,
        keep: bool,
    },
    Hover(Option<u8>),
    OverDc,
    TextPage {
        message: u16,
        page: u16,
    },
    TextClosed,
    TravelStarted {
        from: u8,
        to: u8,
        path: Option<(u8, bool)>,
    },
    DuelRequest(DuelRequest),
    EventChanged(i16),
    PathRevealing(u8),
    PathRevealed(u8),
    NodeRising(u8),
    NodeRevealed(u8),
    RoseCard {
        node: u8,
    },
    NodeBeaten(u8),
    NodeLost(u8),
    /// The cursor entered (Some) or left (None) a SYSTEM icon (W+0x547E,
    /// W+0x80).
    SystemIcon(Option<u8>),
    /// × on SYSTEM icon 0 (0x2374C0): mode 6 is pushed with the side as its
    /// parameter; the map's state is left as it is (the editor overwrites
    /// the work area, so the map re-initialises on return [H]).
    DeckEditor {
        side: u8,
    },
    /// A SYSTEM icon the port does not handle (none since icon 3's menu).
    SystemIconUntraced(u8),
    /// The tutorial menu (SYSTEM icon 3, `tutorial_menu.rs`) chose a
    /// lesson: 0x23B710 backs up the card block and pushes the duel mode,
    /// which plays row 0..3 → controls 32..35, other → 36.
    TutorialLesson { row: u8 },
    /// Select → MSG 146 → YES: 0x1A9290 pops the map to the title (every
    /// update while W+5440 = 3).
    ReturnToTitle,
    /// The OPTION panel's Cross/Square: the three option bits (D36, bit i =
    /// block bit 2 + i) written to both card blocks (0x214340), the sound
    /// row also to the output mode (0x217200).
    OptionsSaved { bits: u8 },
    /// SYSTEM icon 1 (SAVE): the game writes its interim save (the
    /// original's memory-card screens are not traced).
    SaveRequested,
    /// C2 became a story event the story scene plays (11122 the Bosworth
    /// paintings after 11120's window, 11140 Bosworth win, 11160 Seto win,
    /// 11180 Manawyddan win): the map model suspends until
    /// the frontend has played the story pages and resumes it at the
    /// event's map continuation (`MapModel::resume`).
    StoryEvent(i16),
    /// The arrival events 110..=113 finished their pans; C2 keeps the event.
    Arrived(u8),
}

/// The four SYSTEM icon rectangles of the map's HUD, read from the user's
/// executable (table 0x297640, stride 0x26: s16 x0, y0, x1, y1 in cursor
/// space). [F]
pub fn system_icon_rects(executable: &[u8]) -> Result<Vec<[i16; 4]>> {
    let start = (SYSTEM_ICON_TABLE_EE - EE_TO_FILE) as usize;
    let table = executable
        .get(start..start + SYSTEM_ICONS * SYSTEM_ICON_RECORD_BYTES)
        .ok_or("the SYSTEM icon table lies outside the executable")?;
    let rects: Vec<[i16; 4]> = table
        .chunks_exact(SYSTEM_ICON_RECORD_BYTES)
        .map(|r| {
            let h = |o: usize| i16::from_le_bytes([r[o], r[o + 1]]);
            [h(0), h(2), h(4), h(6)]
        })
        .collect();
    for r in &rects {
        if r[0] >= r[2] || r[1] >= r[3] || r.iter().any(|&v| !(0..=CURSOR_MAX + 64).contains(&v)) {
            return Err(format!(
                "SYSTEM icon rectangle {r:?} is not a cursor-space box"
            ));
        }
    }
    Ok(rects)
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Lerp {
    pub start: [f64; 2],
    pub end: [f64; 2],
    pub t: i32,
    pub full: i32,
    pub step: i32,
}
impl Lerp {
    pub fn value(&self) -> [f64; 2] {
        let f = f64::from(self.t.min(self.full)) / f64::from(self.full);
        [
            self.start[0] + (self.end[0] - self.start[0]) * f,
            self.start[1] + (self.end[1] - self.start[1]) * f,
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PathState {
    /// 0 hidden, 1 dots appearing forward, 2 backward, 3 drawn.
    pub state: u8,
    /// Frames since the state was set to 1/2.
    pub frames: u32,
    /// Alpha per dot, 0..=0x100.
    pub alpha: Vec<i32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct RiseState {
    pub rising: bool,
    pub level: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TextWindow {
    pub base: u16,
    pub page: u16,
    pub message: u16,
    /// W+537E: the page counter closes the window once it exceeds 4·groups.
    pub groups: u16,
    /// 1 start, 2 opening, 3 reveal, 4 wait, 5 page turn, 11 last page shown, 12 closing.
    pub phase: u8,
    pub openness: i32,
    pub timer: u32,
    pub reveal_frames: u32,
    /// Reveal frames elapsed (for presentation: lines appear six frames apart).
    pub revealed: u32,
    /// W+537C: 0/1 the map's dialogue window, 2 the speaker window of
    /// event 111 (W+53BE `speaker`, bottom panel W+53B7) [F 0x22D254].
    pub style: u8,
    pub speaker: Option<u16>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Figure {
    /// 0 idle, 1 walking forward, 2 walking in reverse, 3 arrived.
    pub state: u8,
    pub sub: i32,
    pub index: usize,
    pub path: Option<u8>,
    pub forward: bool,
    /// World (x, z) for presentation.
    pub position: [f64; 2],
    pub node: u8,
    /// The start node (C7) of a walk with no path (straight line).
    pub straight_from: Option<u8>,
    /// (W+1D8, W+1DA, walk index) as this frame's draw pass read them,
    /// i.e. before its advance (0x22AE20 places the figure, then steps the
    /// walk); `figure_object` places the figure from these.
    pub drawn: (u8, i32, usize),
}

/// The 2D HUD words the map's update and draw passes keep (see
/// `MapModel::hud_update`).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HudState {
    /// W+6E.
    pub shown: bool,
    /// W+70: 0 (off-screen) ..= 256 (in place).
    pub slide: i32,
    /// W+224: the hover panel's window alpha.
    pub alpha_top: i32,
    /// W+226: the bottom panel's window alpha.
    pub alpha_bottom: i32,
    /// W+82 + 2i.
    pub icon_grow: [i32; 4],
    /// W+5374: set while the HUD is not shown, cleared when W+70 reaches
    /// 256 again (0x237108..0x237154); the cursor, tooltip and caption are
    /// drawn only while it is clear.
    pub hidden: bool,
    /// The hover panel's slide (the original's W+6C: +20 per frame to 256
    /// while a node is hovered and the HUD is shown, −20 to 0 otherwise,
    /// 0x237180..0x237360). The model's `fade` keeps the input gating it
    /// was matched with [H: the original gates Cross on this word].
    pub panel: i32,
}

impl Default for HudState {
    fn default() -> Self {
        Self {
            shown: false,
            slide: 0,
            alpha_top: HUD_ALPHA_FULL,
            alpha_bottom: HUD_ALPHA_FULL,
            icon_grow: [0; 4],
            hidden: true,
            panel: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RoseAnimation {
    pub spin: i32,
    pub scale: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum Handler {
    /// Event 0: free cursor.
    Free,
    /// Events 303/203 (0x229260): return from the duel at `node`.
    Return { node: u8, won: bool, phase: u8 },
    /// Events 103..=109 and 108 (0x228210): reveal the paths of `node`.
    Reveal {
        node: u8,
        paths: Vec<(u8, u16)>,
        phase: u8,
        timer: u32,
        centroid: [f64; 2],
    },
    /// Events 110..=113 (0x228D00): the close logic's pan, then 0x228410
    /// (state 3, Bosworth trigger) and a second pan; afterwards the free
    /// cursor with C2 unchanged.
    Arrive { node: u8, phase: u8, timer: u32 },
    /// A story event (11122/11140/11160/11180): the frontend plays its pages;
    /// the model does nothing until `resume`.
    Story { event: i16 },
    /// A Stonehenge-scene story event entered from the map's return page
    /// (11160, 11180, 12150): C2 is the event but the map is still drawn
    /// while its letterbox (mode 3 from C2+1) waits 12 updates for the
    /// return panel to slide out, then closes 8 rows per update (C2+13 ..
    /// C2+44); the scene bank loads after, so the frontend's story scene
    /// takes over at C2+45 with the letterbox closed [emu trace-seto-cam
    /// f203..f248, trace-manawyddan f61..f106, white trace-manawyddan
    /// f77..f122]. `t` = updates since C2.
    StoryEntry { event: i16, t: u32 },
    /// The tail of a Stonehenge-scene event back on the map (11162 -> 112,
    /// 12132 -> 0, `after_story`): the bank switch has brought the map back
    /// under the closed letterbox; C2 is still the story event while the
    /// letterbox opens (mode 0, 8 rows per update from the first map
    /// update, the HUD out); the update after it is done C2 = `event` and
    /// the resumed handler `then` runs [emu trace-seto-cam f1423..f1438,
    /// white trace-brest f1680..f1695].
    StoryExit {
        event: i16,
        then: Box<Handler>,
        /// The HUD words the resumed handler starts with (the slide stays
        /// out: W+70 = 0 at the C2 update [emu f1438, white f1695]).
        hud: Box<HudState>,
    },
    /// A story event the map plays itself (11120, 11124, 11149): letterbox,
    /// the centred story window and its Cross wait over the running map;
    /// 11124 then sets paths 12, 13, 11 and C2 = 0 once Bosworth starts
    /// rising (`story_window`).
    MapStory { event: i16, sub: u8 },
    /// Events 100 and 101: the opening voyages (`sail.rs`).
    Sail { event: i16, sub: u8 },
}

#[derive(Clone, Debug, Serialize)]
pub struct MapModel {
    pub data: MapData,
    pub save: CampaignSave,
    pub frame: u64,
    pub cursor: (i16, i16),
    /// W+4: 0 free, 2 over a node.
    pub cursor_mode: u8,
    /// W+6C fade-in 0..=0x100.
    pub fade: i32,
    pub camera_target: [f64; 2],
    pub lerp: Option<Lerp>,
    /// Projected anchors of eligible nodes (all 22; the red side's are
    /// 0..=11) and their hover radii.
    pub anchors: Vec<Option<((i32, i32), f64)>>,
    /// The save side's path states (`paths[index]`).
    pub paths: Vec<PathState>,
    pub rises: Vec<RiseState>,
    pub text: Option<TextWindow>,
    /// W+5373: 0 idle, 1 pressed, 2 travelling, 3 arrived, 4 duel requested.
    pub travel: u8,
    pub figure: Figure,
    /// W+5442.
    pub countdown: i32,
    pub rose: Option<RoseAnimation>,
    pub hud_alpha: i32,
    pub handler: Handler,
    pub duel_request: Option<DuelRequest>,
    /// W+5448 this frame: Cross held over a node whose opponent deck cost
    /// is below the player's (MSG 250 replaces the hover panel).
    pub over_dc: bool,
    /// Deck costs for the DC check: the player's (W+5444, 0x2342D0) and the
    /// hovered opponent's (W+5446, 0x215690); `None` when not known.
    pub player_dc: Option<u16>,
    pub opponent_dc: Option<u16>,
    /// Deck cost per opponent id (C4), from which the hover panel sets
    /// `opponent_dc`; empty when the deck import is unavailable.
    pub opponent_costs: std::collections::BTreeMap<u8, u16>,
    pub hovered: Option<u8>,
    /// The SYSTEM icon rectangles (`system_icon_rects`); empty when the
    /// executable was not given, then the icons are an explicit boundary.
    pub system_icons: Vec<[i16; 4]>,
    /// W+0x547E / W+0x80: the hovered SYSTEM icon.
    pub hovered_icon: Option<u8>,
    /// × on icon 0 pushed the deck editor (mode 6, side `save.side`).
    pub editor_request: bool,
    /// C2 became a story event: the frontend plays `story-<event>` and
    /// resumes the map at the event's continuation (`save.event` holds the
    /// story event meanwhile).
    pub story_request: Option<i16>,
    /// The centred story window of 11120/11124/11149 (W+5487 = 1).
    pub story_window: Option<StoryWindow>,
    /// The bottom speaker panel and its pages of 12040/12041 (Mako over
    /// the map, W+53BA / W+537B; `story_window::StoryDialogue`).
    pub story_dialogue: Option<StoryDialogue>,
    /// SYSTEM icon 1 was chosen (taken by the game).
    pub save_request: bool,
    /// W+53C4: the C9 mirror the hover/Cross/text code reads, written from
    /// C9 when the map fade reaches 0 after a story event or a return pan
    /// (0x23727C) [emu for the write site, H for its exact frame here].
    pub w53c4: u8,
    /// Untraced situations reached by the model, reported once.
    pub notes: Vec<String>,
    /// The ship, letterbox and place caption of the opening voyage.
    pub ship: Ship,
    pub letterbox: Letterbox,
    pub caption: Option<Caption>,
    /// W+6C follows the voyage's W+6E rule (events 100..=102) until the
    /// free map.
    pub sail_fade: bool,
    /// W+6E of the previous draw pass (voyage only).
    pub w6e: bool,
    /// The 2D HUD's state (`hud_update`): W+6E (shown), W+70 (slide
    /// 0..=256), W+224 / W+226 (top / bottom window alpha 96..=128) and
    /// W+82 + 2i (SYSTEM icon growth 0..=16).
    pub hud: HudState,
    /// W+60 / W+50, W+58 of the ship pan 0x228720.
    pub pan_t: i32,
    /// W+5372 left set by a White Rose voyage's sub-step 0 (cleared by
    /// the camera code's end): the draw pass then keeps W+6E clear on the
    /// frame the voyage hands over to a reveal (105 → 106) [emu mako-win:
    /// letterbox f315..f323, mark f324].
    pub w5372: bool,
    pub pan_start: [f32; 2],
    /// W+18C: the free-duel map after both routes (`free_duel.rs`).
    pub free_duel: bool,
    /// W+53C3: L1/R1 at Stonehenge slid the hover panel out to change its
    /// opponent (`system::stonehenge_flip`).
    pub flip: bool,
    /// Select's "Return to Title Screen?" (W+5440) and the OPTION panel
    /// (W+543E with icon 2) (`system.rs`).
    pub title_dialog: Option<TitleDialog>,
    pub options: Option<OptionsPanel>,
    /// SYSTEM icon 3's lesson menu (W+543E / W+546D, `tutorial_menu.rs`).
    pub tutorial_menu: Option<TutorialMenu>,
    /// A lesson row was chosen (taken by the game: the lesson's duel, then
    /// back to this map).
    pub tutorial_request: Option<u8>,
    /// The card block's options (D36 layout, `game_options`), set by the
    /// frontend from the player's block; the panel starts from them.
    pub option_bits: u8,
    /// YES on "Return to Title Screen?" (taken by the frontend).
    pub title_request: bool,
    /// The walk advanced on its start frame (see `start_walk`).
    #[serde(skip)]
    walk_started: bool,
    /// The fade-in restarts on the next frame (return handler phase 0).
    #[serde(skip)]
    fade_hold: bool,
}

impl MapModel {
    fn base(data: MapData, save: CampaignSave, target: [f64; 2]) -> Result<Self> {
        data.validate()?;
        if save.side > 1 {
            return Err(format!("Campaign side {} does not exist", save.side));
        }
        // The path states of the save's side (W+42E + 0x2040·side).
        let paths = (0..PATHS_PER_SIDE)
            .map(|i| PathState {
                state: 0,
                frames: 0,
                alpha: vec![0; data.path(save.side, i as u8).dots.len()],
            })
            .collect();
        let position = data.node(save.location).position;
        let save_c9 = save.c9;
        let mut model = Self {
            save,
            frame: 0,
            cursor: CURSOR_START,
            cursor_mode: 0,
            fade: 0,
            camera_target: target,
            lerp: None,
            anchors: vec![None; NODE_COUNT],
            paths,
            rises: vec![RiseState::default(); NODE_COUNT],
            text: None,
            travel: 0,
            figure: Figure {
                state: 0,
                sub: 0,
                index: 0,
                path: None,
                forward: true,
                position: [f64::from(position[0]), f64::from(position[2])],
                node: 0,
                straight_from: None,
                drawn: (0, 0, 0),
            },
            countdown: 0,
            rose: None,
            hud_alpha: HUD_ALPHA_FULL,
            handler: Handler::Free,
            duel_request: None,
            over_dc: false,
            player_dc: None,
            opponent_dc: None,
            opponent_costs: std::collections::BTreeMap::new(),
            hovered: None,
            system_icons: Vec::new(),
            hovered_icon: None,
            editor_request: false,
            story_request: None,
            story_window: None,
            story_dialogue: None,
            save_request: false,
            w53c4: save_c9,
            notes: Vec::new(),
            ship: Ship::default(),
            letterbox: Letterbox::default(),
            caption: None,
            sail_fade: false,
            w6e: false,
            hud: HudState::default(),
            pan_t: 0,
            pan_start: [0.0; 2],
            w5372: false,
            free_duel: false,
            flip: false,
            title_dialog: None,
            options: None,
            tutorial_menu: None,
            tutorial_request: None,
            option_bits: crate::game_options::DEFAULT_BITS,
            title_request: false,
            walk_started: false,
            fade_hold: false,
            data,
        };
        model.figure.node = model.save.location;
        model.park_ship();
        for p in model.save.drawn_paths(&model.data) {
            model.set_path_drawn(p);
        }
        model.update_anchors();
        Ok(model)
    }

    /// The free map at event 0 after landing (camera on the centroid of the
    /// new end nodes of Milford Haven's paths, fade complete, state 622).
    pub fn at_event_zero(data: MapData) -> Result<Self> {
        let save = CampaignSave::after_landing();
        let (a, b) = (data.node(2).position, data.node(8).position);
        let target = [f64::from(a[0] + b[0]) / 2.0, f64::from(a[2] + b[2]) / 2.0];
        let mut model = Self::base(data, save, target)?;
        model.fade = FADE_FULL;
        // The HUD is in place at 622 (W+70 = 256 [emu hud-oracle-622]).
        model.hud.shown = true;
        model.hud.hidden = false;
        model.hud.slide = HUD_SLIDE_FULL;
        Ok(model)
    }

    /// Re-entry after the first duel at Chester (0x227F00 then 0x21C1A4):
    /// camera on the node, cursor reset, dispatcher on the win/loss event.
    pub fn after_duel(data: MapData, won: bool) -> Result<Self> {
        let mut save = CampaignSave::after_landing();
        save.hovered = 2;
        save.destination = 2;
        save.opponent = 2;
        save.apply_duel_end(&data, won);
        Self::resume(data, save)
    }

    /// Back from a duel the map requested with `save` (the SAVE block as
    /// the map left it: node states, counters, the hovered node and its
    /// opponent): 0x227F00 applies the result, then 0x21C1A4 resumes.
    pub fn after_duel_from(data: MapData, mut save: CampaignSave, won: bool) -> Result<Self> {
        save.apply_duel_end(&data, won);
        Self::resume(data, save)
    }

    /// Re-enter the map with a save whose event is a return event (0x21C1A4),
    /// the free map, a reveal/arrival event, or the map continuation of a
    /// story event (11124 after 11122; 11149 after 11140–11148; 112 after
    /// 11160–11162, where 11162 stored C9 = 1); the map plays 11120, 11124
    /// and 11149 from the frame C2 became the event (`story_window`). The
    /// original plays the story events inside the map mode; this model
    /// rebuilds the map from the save (paths of beaten nodes drawn, camera on
    /// the location) [H: no fade or camera motion of that moment is traced].
    pub fn resume(data: MapData, save: CampaignSave) -> Result<Self> {
        if save.free_duel() {
            // 0x21C1A4 tests W+18C before SAVE+0x3C (`ending::continue_target`).
            return Self::free_duel_map(data, save);
        }
        if !save.map_entered {
            // SAVE+0x3C = 0 only after an ending: story init takes the
            // route switch 0x21C734 instead (`ending::continue_target`).
            return Err(
                "a save after the ending (SAVE+0x3C = 0) continues with the other route's story (ending::continue_target), not the map"
                    .into(),
            );
        }
        if save.side != 0 {
            return Self::resume_white(data, save);
        }
        match save.event {
            EVENT_FIRST_VOYAGE => return Self::first_voyage(data, save),
            EVENT_SECOND_VOYAGE => return Self::resume_second_voyage(data, save),
            _ => {}
        }
        let position = data.node(save.location).position;
        let target = [f64::from(position[0]), f64::from(position[2])];
        let mut model = Self::base(data, save, target)?;
        let event = model.save.event;
        model.handler = match event {
            200..=399 => Handler::Return {
                node: model.save.location,
                won: event >= 300,
                phase: 0,
            },
            0 => {
                model.fade = FADE_FULL;
                model.hud.shown = true;
                model.hud.hidden = false;
                model.hud.slide = HUD_SLIDE_FULL;
                Handler::Free
            }
            EVENT_MILFORD_REVEAL => {
                // A save taken during 102 (the dispatcher's 0x228210 on
                // node 1) [H: only the in-map continuation from 101 is
                // traced frame by frame].
                model.fade = FADE_FULL;
                model.w53c4 = model.save.c9;
                model.reveal_handler(MILFORD_NODE)
            }
            EVENT_FIRST_REVEAL..=EVENT_LAST_REVEAL => {
                model.fade = FADE_FULL;
                model.w53c4 = model.save.c9;
                model.reveal_event_handler(event)
            }
            EVENT_FIRST_ARRIVE..=EVENT_LAST_ARRIVE => {
                model.fade = FADE_FULL;
                // 110, 111, 112, 113 → nodes 9, 10, 11, 11.
                let node = if event == EVENT_LAST_ARRIVE {
                    STONEHENGE_NODE
                } else {
                    (event - EVENT_FIRST_ARRIVE) as u8 + 9
                };
                if event == EVENT_STONEHENGE_ARRIVE {
                    // 11162 sub-steps 9/10 store C9 = 1 before C2 = 112.
                    model.save.c9 = 1;
                }
                model.w53c4 = model.save.c9;
                Handler::Arrive {
                    node,
                    phase: 0,
                    timer: 0,
                }
            }
            e if Self::is_window_event(e) => model.resume_map_story(e),
            e if STORY_EVENTS.contains(&e) => {
                model.story_request = Some(e);
                Handler::Story { event: e }
            }
            other => return Err(format!("Campaign map event {other} is not traced")),
        };
        Ok(model)
    }

    /// Events 103..=109 (dispatcher 0x2263B0: reveal nodes 2..=8). The node
    /// is already beaten (close logic), but the paths towards its hidden
    /// ends are what the event animates: they are not drawn yet (the
    /// original never re-entered the map).
    pub(super) fn reveal_event_handler(&mut self, event: i16) -> Handler {
        let node = (event - EVENT_FIRST_REVEAL) as u8 + 2;
        self.undraw_unrevealed_paths(node);
        self.reveal_handler(node)
    }

    /// The paths of `node`'s red list towards a still hidden end: not drawn
    /// before its reveal event animates them.
    pub(super) fn undraw_unrevealed_paths(&mut self, node: u8) {
        if let Handler::Reveal { paths, .. } = self.reveal_handler(node) {
            for (path, dir) in paths {
                if self.save.state(self.data.reveal_end(path, dir)) == STATE_HIDDEN {
                    let p = &mut self.paths[usize::from(path)];
                    p.state = 0;
                    p.frames = 0;
                    p.alpha.iter_mut().for_each(|a| *a = 0);
                }
            }
        }
    }

    /// The reveal handler of `node` (0x228210 with the node's red list;
    /// the White Rose events pass the node's first white path, count 1
    /// [F 0x2272C8..0x227970]).
    pub(super) fn reveal_handler(&self, node: u8) -> Handler {
        let entry = self.data.node(node);
        let paths: Vec<(u8, u16)> = if self.save.side != 0 {
            vec![(entry.white_paths[0] as u8, 1)]
        } else {
            (0..usize::from(entry.path_count.min(4)))
                .map(|k| (entry.red_paths[k] as u8, entry.path_dirs[k]))
                .collect()
        };
        Handler::Reveal {
            node,
            paths,
            phase: 0,
            timer: 0,
            centroid: self.camera_target,
        }
    }

    /// The Bosworth trigger inside 0x228410 (0x228644..): path 12 undrawn and
    /// eight of nodes 2..=10 beaten → C2 = 11120 (the running 1xx event is
    /// abandoned; W+5370 keeps it, never read on the red route). The map
    /// plays 11120's window itself.
    fn bosworth_trigger(&mut self, events: &mut Vec<MapEvent>) -> bool {
        if self.paths[usize::from(BOSWORTH_PATH)].state != 0
            || self.save.beaten_count() < BOSWORTH_TRIGGER_COUNT
        {
            return false;
        }
        self.start_map_story(EVENT_BOSWORTH_TRIGGER, events);
        true
    }

    pub(super) fn start_story(&mut self, event: i16, events: &mut Vec<MapEvent>) {
        self.save.event = event;
        self.lerp = None;
        if SCENE0_ENTRIES.contains(&event) {
            self.handler = Handler::StoryEntry { event, t: 0 };
        } else {
            self.story_request = Some(event);
            self.handler = Handler::Story { event };
        }
        events.push(MapEvent::EventChanged(event));
        events.push(MapEvent::StoryEvent(event));
    }

    /// The story event the frontend has to play, taken once.
    pub fn take_story_request(&mut self) -> Option<i16> {
        self.story_request.take()
    }

    /// The map re-entered after story `story_event`'s pages, at the save's
    /// continuation event: as `resume`, except that the Stonehenge-scene
    /// tails (`STORY_EXIT_EVENTS`) first open the closed letterbox over the
    /// map (`Handler::StoryExit`).
    pub fn after_story(data: MapData, save: CampaignSave, story_event: i16) -> Result<Self> {
        let mut model = Self::resume(data, save)?;
        if STORY_EXIT_EVENTS.contains(&story_event) {
            let then = std::mem::replace(&mut model.handler, Handler::Free);
            let resumed = model.hud.clone();
            model.hud = HudState {
                shown: false,
                slide: 0,
                hidden: true,
                panel: 0,
                ..resumed.clone()
            };
            let hud = HudState {
                shown: resumed.shown,
                hidden: resumed.hidden,
                ..model.hud.clone()
            };
            model.handler = Handler::StoryExit {
                event: model.save.event,
                then: Box::new(then),
                hud: Box::new(hud),
            };
            model.save.event = story_event;
            model.letterbox = Letterbox {
                active: 0,
                mode: 3,
                bars: story_window::LETTERBOX_CLOSED,
            };
        }
        Ok(model)
    }

    /// A frontend playing a story chain over the running map (the red
    /// route's 11122 and 11140..11148: the original never leaves the map
    /// mode there) moved on to the chain's next event: C2 follows.
    pub fn story_progress(&mut self, event: i16) {
        if matches!(self.handler, Handler::Story { .. }) && self.save.event != event {
            self.save.event = event;
            self.handler = Handler::Story { event };
        }
    }

    /// The story chain played over the running map ended at the window
    /// event `event` (11124, 11149): the map plays it from the update on
    /// which C2 became the event, as `resume` would, with the map's own
    /// state (camera, HUD) kept.
    pub fn end_story(&mut self, event: i16) -> Result<()> {
        if !matches!(self.handler, Handler::Story { .. }) {
            return Err(format!("the map is not playing a story event (continuation {event})"));
        }
        if !Self::is_window_event(event) {
            return Err(format!(
                "map event {event} after a story played over the map is not a window event"
            ));
        }
        self.save.event = event;
        self.handler = self.resume_map_story(event);
        Ok(())
    }

    pub(super) fn set_path_drawn(&mut self, path: u8) {
        let p = &mut self.paths[usize::from(path)];
        p.state = 3;
        p.frames = 0;
        for a in &mut p.alpha {
            *a = DOT_FULL_ALPHA;
        }
    }

    pub(super) fn note(&mut self, text: &str) {
        if !self.notes.iter().any(|n| n == text) {
            self.notes.push(text.to_owned());
        }
    }

    pub fn node_position(&self, node: u8) -> [f64; 2] {
        let p = self.data.node(node).position;
        [f64::from(p[0]), f64::from(p[2])]
    }

    /// Hover eligibility 0x23AC80: nodes 0 and 11 always, the side's own
    /// nodes (red 1..=10, white 12..=21) when their state is nonzero
    /// (white branch 0x23ADD4..0x23AE2C) [F].
    fn eligible(&self, node: u8) -> bool {
        if self.free_duel {
            // 0x23AD50: every node.
            return true;
        }
        let own = if self.save.side == 0 {
            usize::from(node) < RED_NODES
        } else {
            node >= 12 && usize::from(node) < NODE_COUNT
        };
        node == 0 || node == 11 || (own && self.save.state(node) != STATE_HIDDEN)
    }

    fn update_anchors(&mut self) {
        for n in 0..NODE_COUNT {
            self.anchors[n] = if self.eligible(n as u8) {
                node_anchor(self.camera_target, &self.data.nodes[n])
            } else {
                None
            };
        }
    }

    /// The hover panel's place tooltip (0x236124..0x2363E4): the hovered
    /// node's place message (s5 of the switch 0x2D2630, 221 "Chester") in
    /// a window near the cursor; MSG 250 takes its place when W+5448 is set
    /// (`over_dc`). Gates [F 0x2360C0..0x236154]: the HUD in place (W+5374
    /// clear, W+6E set), the cursor over a node (W+4 = 2), no SYSTEM icon
    /// (W+547E), paths idle (W+5376). It stays up over an open page [emu
    /// 653-mgo]. W+72, W+5485, W+5486 and 0x1F6450 are assumed clear [H].
    pub fn place_tooltip(&self) -> Option<u16> {
        if !self.hud.shown
            || self.hud.hidden
            || self.over_dc
            || self.cursor_mode != 2
            || self.hovered_icon.is_some()
            || !self.paths_idle()
        {
            return None;
        }
        self.hover_variant(self.save.hovered)?.place_message
    }

    /// W+5376 (the dot updater 0x22C330): no path animating. The sea
    /// routes (red 0 and 15, white 4, 5 and 11, skipped by index there)
    /// neither animate nor count [F 0x22C408..0x22C4D8].
    pub(super) fn paths_idle(&self) -> bool {
        let side = self.save.side;
        self.paths.iter().enumerate().all(|(i, p)| {
            p.state == 0 || p.state == 3 || self.data.path(side, i as u8).sea_route
        })
    }
    fn busy(&self) -> bool {
        self.travel != 0
            || self.text.is_some()
            || self.rises.iter().any(|r| r.rising)
            || self.rose.is_some()
    }

    pub fn start_text(&mut self, base: u16, groups: u16, events: &mut Vec<MapEvent>) {
        self.start_text_styled(base, groups, 0, None, events);
    }

    /// The text start with a style (W+537C) and bottom speaker (W+53BE).
    pub fn start_text_styled(
        &mut self,
        base: u16,
        groups: u16,
        style: u8,
        speaker: Option<u16>,
        events: &mut Vec<MapEvent>,
    ) {
        let message = base + 1;
        let extra = if style == 2 { STYLE2_FIRST_REVEAL_EXTRA } else { 0 };
        self.text = Some(TextWindow {
            base,
            page: 1,
            message,
            groups: groups.max(1),
            phase: 1,
            openness: 0,
            timer: 0,
            reveal_frames: self.reveal_frames(message) + extra,
            revealed: 0,
            style,
            speaker,
        });
        events.push(MapEvent::TextPage { message, page: 1 });
    }

    fn reveal_frames(&self, message: u16) -> u32 {
        let lines = self
            .data
            .messages
            .get(&message)
            .map_or(1, |m| u32::from(m.lines).max(1));
        lines * REVEAL_FRAMES_PER_LINE + REVEAL_EXTRA_FRAMES
    }

    /// One 25 Hz update. Order: travel/camera → dispatcher (cursor/Cross,
    /// return, reveal) → duel push check → leader walk → node rises → path
    /// dots → text window → fade-in [H: reconstructed from the frame logs].
    pub fn step(&mut self, pad: Pad) -> Vec<MapEvent> {
        self.step_with(pad, pad.menu)
    }

    /// `step` with the system dialogs' presses (Select, Circle, Square,
    /// Left/Right, Up/Down repeat, L1/R1).
    pub fn step_with(&mut self, pad: Pad, menu: MenuPad) -> Vec<MapEvent> {
        self.frame += 1;
        self.over_dc = false;
        let mut events = Vec::new();
        if let Handler::StoryExit { event, then, hud } = &self.handler {
            if self.letterbox.active != 2 {
                if self.letterbox.active == 0 {
                    self.letterbox.start(0);
                }
                self.letterbox.step();
                return events;
            }
            // The update after the letterbox is open: C2 = the event and
            // its handler's first update.
            let (event, then, hud) = (*event, (**then).clone(), (**hud).clone());
            self.save.event = event;
            self.hud = hud;
            self.handler = then;
            events.push(MapEvent::EventChanged(event));
        }
        if self.sail_fade {
            self.sail_fade_update();
        }
        self.travel_update(&mut events);
        let map_story = matches!(self.handler, Handler::MapStory { .. });
        match self.handler.clone() {
            Handler::Free => self.free_update(pad, menu, &mut events),
            Handler::Return { node, won, phase } => {
                self.return_update(node, won, phase, &mut events);
                if matches!(self.handler, Handler::Return { .. }) {
                    self.hover_update(&mut events);
                }
            }
            Handler::Reveal {
                node,
                paths,
                phase,
                timer,
                centroid,
            } => self.reveal_update(node, paths, phase, timer, centroid, &mut events),
            Handler::Arrive { node, phase, timer } => {
                self.arrive_update(node, phase, timer, &mut events)
            }
            // The draw pass keeps running under the story: the HUD slides
            // out (W+70 -20 per update from C2+1, the cursor gone) [emu
            // trace-win3 f50..f62, trace-seto-cam f204..f216].
            Handler::Story { .. } => {
                self.hud_update();
                return events;
            }
            Handler::StoryExit { .. } => return events,
            Handler::StoryEntry { event, t } => {
                let t = t + 1;
                if t == 1 {
                    self.letterbox.start(3);
                } else if t > STORY_ENTRY_HOLD && self.letterbox.active == 1 {
                    self.letterbox.step();
                }
                self.handler = if self.letterbox.active == 2 {
                    self.story_request = Some(event);
                    Handler::Story { event }
                } else {
                    Handler::StoryEntry { event, t }
                };
                self.hud_update();
                return events;
            }
            Handler::MapStory { event, sub } => {
                self.map_story_update(event, sub, pad, &mut events)
            }
            Handler::Sail { event, sub } => self.sail_update(event, sub, pad, &mut events),
        }
        // Event 100 of the White Rose side clears Stonehenge's state every
        // frame (the dispatcher's delay slot 0x2272F8) [F; emu
        // logs/white-start-12000].
        if self.save.side != 0 && self.save.event == EVENT_FIRST_VOYAGE {
            self.save.node_states[usize::from(STONEHENGE_NODE)] = STATE_HIDDEN;
        }
        if map_story {
            self.map_story_draw_update();
        }
        if matches!(self.handler, Handler::Story { .. }) {
            // (The C2 update of the event: the draw pass decides W+6E.)
            self.hud_update();
            return events;
        }
        self.push_check(&mut events);
        self.walk_update();
        self.rise_update(&mut events);
        self.dots_update(&mut events);
        self.ship_update(&mut events);
        self.sail_draw_update();
        self.text_update(pad, &mut events);
        if self.sail_fade {
            if matches!(self.handler, Handler::Free) {
                self.sail_fade = false;
            } else {
                self.sail_w6e();
            }
        } else if self.fade_hold {
            self.fade_hold = false;
        } else if self.fade < FADE_FULL {
            self.fade = (self.fade + FADE_STEP).min(FADE_FULL);
        }
        self.hud_update();
        self.update_anchors();
        events
    }

    /// The HUD words of 0x237080 (W+70 ±20 by W+6E), the W+6E decision of
    /// the node pass (0x22A884..0x22AA48, red side), the cursor's window
    /// alphas (0x23B200..0x23B2F4) and the SYSTEM icon growth the draw pass
    /// 0x2349B0 applies [F]. [H] W+5485 = 1, W+5486 = 0 and no sound wait
    /// (0x1F6450); the node pass's "busy" local is any node rising and its
    /// camera-pan word W+5372 is a camera lerp outside the travel.
    fn hud_update(&mut self) {
        let event = self.save.event;
        // The arrival events' first pan (0x228D00) holds W+5372 = 1 until
        // its lerp is done [emu trace-trigger-cam: the HUD slides out from
        // f205 (C2 = 111 at f203) and starts back at f221].
        let arrive_pan = matches!(
            self.handler,
            Handler::Arrive { phase: 0, timer, .. } if timer < ARRIVE_PAN_FRAMES
        );
        let idle = self.paths_idle()
            && !self.rises.iter().any(|r| r.rising)
            && !(self.lerp.is_some() && self.travel == 0)
            && !arrive_pan;
        let decided = if self.free_duel {
            // 0x22A888: W+6E = 1 on the free-duel map.
            true
        } else if self.sail_fade {
            self.w6e
        } else {
            event < 10000
                && self.paths_idle()
                && (event == 0 || (idle && event != 100 && event != 101))
        };
        // 0x237080 runs at the frame's start: W+70 follows the W+6E the
        // previous draw pass decided (the voyage's `w6e` already is that
        // value) [emu: f221 slide 20 from the draw of f220, none at f1787
        // when C2 became 108].
        let w6e = if self.sail_fade { decided } else { self.hud.shown };
        self.hud.shown = decided;
        if w6e {
            self.hud.slide = (self.hud.slide + HUD_SLIDE_STEP).min(HUD_SLIDE_FULL);
            if self.hud.slide >= HUD_SLIDE_FULL {
                self.hud.hidden = false;
            }
        } else {
            self.hud.hidden = true;
            self.hud.slide = (self.hud.slide - HUD_SLIDE_STEP).max(0);
        }
        let step = |alpha: i32, dim: bool| {
            if dim {
                (alpha - HUD_ALPHA_STEP).max(HUD_ALPHA_DIM)
            } else {
                (alpha + HUD_ALPHA_STEP).min(HUD_ALPHA_FULL)
            }
        };
        self.hud.alpha_top = step(self.hud.alpha_top, self.cursor.1 < HUD_TOP_DIM_Y);
        self.hud.alpha_bottom = step(self.hud.alpha_bottom, self.cursor.1 >= HUD_BOTTOM_DIM_Y);
        // W+6C reads the W+6E of the previous draw pass, as W+70 does
        // (0x237190), and C2 before this update's dispatcher (C2 ≥ 10000
        // takes 0x2372A4, W+6C −20; the previous W+6E already says it)
        // [emu trace-win3: the panel is still in (W+6C 256) on 11140's C2
        // update f49 and 236 on f50].
        self.hud.panel = if self.hovered.is_some() && w6e && !self.flip {
            (self.hud.panel + HUD_SLIDE_STEP).min(HUD_SLIDE_FULL)
        } else {
            (self.hud.panel - HUD_SLIDE_STEP).max(0)
        };
        if self.flip && self.hud.panel <= 0 {
            // 0x237260: the panel is out: W+53C3 = 0, W+53C4 = C9.
            self.flip = false;
            self.w53c4 = self.save.c9;
        }
        self.free_duel_side();
        for (i, grow) in self.hud.icon_grow.iter_mut().enumerate() {
            *grow = if self.hovered_icon == Some(i as u8) {
                (*grow + HUD_ICON_GROW_STEP).min(HUD_ICON_GROW_FULL)
            } else {
                (*grow - HUD_ICON_GROW_STEP).max(0)
            };
        }
    }

    /// The node hover of 0x23AC80 (called at the end of the HUD update
    /// 0x237080 on every map update): the first node whose anchor circle
    /// holds the cursor. It also runs during the post-duel return, where
    /// W+537A lets the node count (0x23B184..0x23B1A8), so the hover panel
    /// slides in with the HUD [emu trace-win3 f3..f15: the panel windows
    /// dy −295 .. 0].
    fn hover_update(&mut self, events: &mut Vec<MapEvent>) {
        self.update_anchors();
        let mut hovered = None;
        for n in 0..NODE_COUNT {
            if let Some((anchor, radius)) = self.anchors[n] {
                let dx = f64::from(i32::from(self.cursor.0) - anchor.0);
                let dy = f64::from(i32::from(self.cursor.1) - anchor.1);
                if dx * dx + dy * dy < radius * radius {
                    hovered = Some(n as u8);
                    break;
                }
            }
        }
        if hovered != self.hovered {
            self.hovered = hovered;
            events.push(MapEvent::Hover(hovered));
        }
        self.cursor_mode = if hovered.is_some() { 2 } else { 0 };
        if let Some(node) = hovered {
            self.save.hovered = node;
            if let Some(opponent) = self.hover_variant(node).and_then(|t| t.opponent) {
                self.save.opponent = i16::from(opponent);
                if let Some(&cost) = self.opponent_costs.get(&opponent) {
                    self.opponent_dc = Some(cost);
                }
            }
        }
    }

    fn free_update(&mut self, pad: Pad, menu: MenuPad, events: &mut Vec<MapEvent>) {
        if self.fade < FADE_FULL || self.travel != 0 || self.text.is_some() {
            return;
        }
        // The system dialogs run inside the gates of 0x237398 (paths idle,
        // no node rising, the HUD in place, C2 < 200); while one is up the
        // cursor stays [emu].
        let gates = self.paths_idle()
            && !self.rises.iter().any(|r| r.rising)
            && !self.hud.hidden
            && self.save.event < 200;
        if gates && self.system_dialogs(menu, events) {
            return;
        }
        if gates && self.tutorial_menu_update(menu, events) {
            return;
        }
        // Cursor (0x203380 mode 1, 16 px per held direction) and edge pan.
        let mut moved = false;
        if pad.up {
            self.cursor.1 -= CURSOR_SPEED;
            moved = true;
        }
        if pad.down {
            self.cursor.1 += CURSOR_SPEED;
            moved = true;
        }
        if pad.left {
            self.cursor.0 -= CURSOR_SPEED;
            moved = true;
        }
        if pad.right {
            self.cursor.0 += CURSOR_SPEED;
            moved = true;
        }
        if moved {
            if self.cursor.1 < 0 {
                self.camera_target[1] -= EDGE_PAN;
            }
            if self.cursor.1 > CURSOR_MAX {
                self.camera_target[1] += EDGE_PAN;
            }
            if self.cursor.0 < 0 {
                self.camera_target[0] -= EDGE_PAN;
            }
            if self.cursor.0 > CURSOR_MAX {
                self.camera_target[0] += EDGE_PAN;
            }
            self.cursor.0 = self.cursor.0.clamp(0, CURSOR_MAX);
            self.cursor.1 = self.cursor.1.clamp(0, CURSOR_MAX);
        }
        self.hover_update(events);
        // SYSTEM icons (0x23AFDC loop of the hover update, 0x2373FC of the
        // map update): the cursor within a rectangle plus the 2-px margin
        // hovers the icon; an icon takes precedence over a node (W+0x7E
        // set, W+0x6A cleared, cursor mode 2) and the node Cross handler
        // needs W+0x547E = 0.
        let (cx, cy) = (self.cursor.0, self.cursor.1);
        let hovered_icon = self
            .system_icons
            .iter()
            .position(|r| {
                (r[0] - SYSTEM_ICON_MARGIN..=r[2] + SYSTEM_ICON_MARGIN).contains(&cx)
                    && (r[1] - SYSTEM_ICON_MARGIN..=r[3] + SYSTEM_ICON_MARGIN).contains(&cy)
            })
            .map(|i| i as u8);
        if hovered_icon != self.hovered_icon {
            self.hovered_icon = hovered_icon;
            events.push(MapEvent::SystemIcon(hovered_icon));
        }
        if let Some(icon) = hovered_icon {
            self.cursor_mode = 2;
            // 0x237398..0x2373E8: paths idle, no node rising, not busy, no
            // sound playing (not modelled), event < 200.
            if !self.paths_idle() || self.busy() || self.save.event >= 200 {
                return;
            }
            if !pad.cross {
                // 0x2378E8: no icon chosen, Select may open the dialog.
                if menu.select && gates {
                    self.open_title_dialog(events);
                }
                return;
            }
            match icon {
                0 => {
                    events.push(MapEvent::Sfx(SFX_SYSTEM_ICON));
                    events.push(MapEvent::DeckEditor {
                        side: self.save.side,
                    });
                    self.editor_request = true;
                }
                1 => {
                    // 0x23753C: 0x1AB8F0(1) starts the memory-card save
                    // (W+543E = 1); the frontend runs `memory_card::
                    // CardDialog::save` from the next update [F, emu].
                    events.push(MapEvent::Sfx(SFX_SYSTEM_ICON));
                    events.push(MapEvent::SaveRequested);
                    self.save_request = true;
                }
                2 => self.open_options(events),
                3 => self.open_tutorial_menu(events),
                other => events.push(MapEvent::SystemIconUntraced(other)),
            }
            return;
        }
        if gates && !self.busy() {
            if menu.select {
                // 0x23793C: no icon hovered, Select opens the dialog.
                self.open_title_dialog(events);
                return;
            }
            self.stonehenge_flip(menu, events);
        }
        // Cross handler (0x2397D0..0x239BD8).
        let Some(node) = self.hovered else {
            return;
        };
        // The handler needs W+6C = 0x100: the hover panel fully in [F
        // MAP.md §3]; `hud.panel` is last update's value [H: one frame].
        // Side 1 (0x239BDC): C6 17 or 1 never; 15 only while the Strait
        // (SAVE[3+16]) is hidden, then without the DC check and text.
        let white = self.save.side != 0;
        let dover = white && node == WHITE_DOVER_NODE;
        let refused = if white {
            node == WHITE_BOULOGNE_NODE
                || node == MILFORD_NODE
                || (dover && self.save.state(WHITE_STRAIT_NODE) != STATE_HIDDEN)
        } else {
            NOT_SELECTABLE.contains(&node)
        };
        if refused
            || self.save.state(node) == STATE_HIDDEN
            || !self.paths_idle()
            || self.busy()
            || self.save.event >= 200
            || self.hud.panel < HUD_SLIDE_FULL
        {
            return;
        }
        // Over DC (0x239810: opponent deck cost W+5446 < player deck cost
        // W+5444, no text window open): the block at 0x239E28 replaces the
        // handler. While Cross is held W+5448 = 1 and the hover panel draws
        // MSG 250 centred at y 288 instead of the node's panel (0x236170,
        // cleared again after each draw at 0x2363E0); the press frame plays
        // SFX 6 (0x216EA0(0, 6)). Nothing else happens.
        if let (Some(player), Some(opponent)) = (self.player_dc, self.opponent_dc) {
            if opponent < player && self.text.is_none() && !dover {
                if pad.cross_held {
                    self.over_dc = true;
                    if pad.cross {
                        events.push(MapEvent::Sfx(SFX_OVER_DC));
                        events.push(MapEvent::OverDc);
                    }
                }
                return;
            }
        }
        if !pad.cross {
            return;
        }
        events.push(MapEvent::Sfx(SFX_SELECT));
        // The cursor snaps to the node's anchor as projected on the press frame
        // and keeps that value while travelling: the hover update (0x23AC80)
        // does not run during the travel, so W+8C stays stale (logs/pre-duel-652:
        // cur=(243,135) from f003 to f055 while the target lerps to (-30,40)).
        if let Some((anchor, _)) = self.anchors[usize::from(node)] {
            self.cursor = (
                anchor.0.clamp(0, i32::from(CURSOR_MAX)) as i16,
                anchor.1.clamp(0, i32::from(CURSOR_MAX)) as i16,
            );
        }
        // Text: state 1 → the generic taunt; otherwise only Stonehenge at
        // state 3 with W+53C4 == 1 (Manawyddan, 0x239AA4..); every other
        // state travels without text (0x239B78).
        let state = self.save.state(node);
        let taunt = !dover
            && (state == STATE_OPEN
                || (!white && node == STONEHENGE_NODE && state == STATE_BEATEN && self.w53c4 == 1));
        if taunt {
            match self.hover_variant(node) {
                Some(NodeTextVariant {
                    encounter_base: Some(base),
                    encounter_groups,
                    ..
                }) => self.start_text(base, encounter_groups.unwrap_or(1), events),
                _ if white && self.data.white_texts.is_empty() => self.note(WHITE_TEXTS_MISSING),
                _ => self.note("encounter text base of this node is not decoded"),
            }
        }
        self.travel = 1;
    }

    /// The hover panel's constants for `node` now (0x234CC0 switch, with
    /// Stonehenge's state/W+53C4 branches; the save's side).
    pub fn hover_text(&self, node: u8) -> Option<NodeTextVariant> {
        self.hover_variant(node)
    }

    fn hover_variant(&self, node: u8) -> Option<NodeTextVariant> {
        if self.free_duel {
            return self.free_duel_text(node);
        }
        self.data
            .side_text(self.save.side, node, self.save.state(node), self.w53c4)
    }

    fn travel_update(&mut self, events: &mut Vec<MapEvent>) {
        match self.travel {
            1 => {
                // 0x228EF0 the frame after the press: destination, walk, lerp.
                let to = self.save.hovered;
                let from = self.save.location;
                self.save.destination = to;
                // 0x229024: C9 = W+53C4.
                self.save.c9 = self.w53c4;
                let path = self.data.walk_path(from, to);
                self.start_walk(from, to, path);
                let end = self.node_position(to);
                if (end[0] - self.camera_target[0]).abs() > 1e-9
                    || (end[1] - self.camera_target[1]).abs() > 1e-9
                {
                    self.lerp = Some(Lerp {
                        start: self.camera_target,
                        end,
                        t: 0,
                        full: LERP_FULL,
                        step: LERP_STEP,
                    });
                }
                self.travel = 2;
                events.push(MapEvent::TravelStarted { from, to, path });
            }
            2 => {
                if let Some(lerp) = &mut self.lerp {
                    lerp.t += lerp.step;
                    self.camera_target = lerp.value();
                    if lerp.t >= lerp.full {
                        self.camera_target = lerp.end;
                        self.lerp = None;
                    }
                }
                if self.lerp.is_none() {
                    self.travel = 3;
                    // 0x2291F4..0x229210: C6 = 15 with the Strait hidden
                    // gives C2 = 12040 instead of the duel push [F].
                    if self.save.side != 0
                        && self.save.hovered == WHITE_DOVER_NODE
                        && self.save.state(WHITE_STRAIT_NODE) == STATE_HIDDEN
                    {
                        // Mako's page plays over the map (`story_dialogue`).
                        self.start_map_story(EVENT_DOVER_STORY, events);
                        // The story replaces the push: travel goes on as
                        // after a push (the walk ends at Dover) but
                        // `push_check` never fires (a duel against the
                        // stale SAVE opponent was requested at the walk's
                        // end; claude-route-e2e PLAYTHROUGH-2026-10-06).
                        self.travel = 4;
                    }
                }
            }
            _ => {}
        }
    }

    fn start_walk(&mut self, from: u8, to: u8, path: Option<(u8, bool)>) {
        self.figure.node = to;
        match path {
            Some((index, forward)) if from != to => {
                self.figure.state = if forward { 1 } else { 2 };
                self.figure.sub = 0;
                self.figure.index = 0;
                self.figure.path = Some(index);
                self.figure.forward = forward;
                self.figure.position = self.node_position(from);
                self.figure.drawn = (self.figure.state, 0, 0);
                // The walk advances on its start frame (fig 1 → 3 in 52 frames for 13 dots).
                self.walk_advance();
                self.walk_started = true;
            }
            _ if from != to => {
                // No 0x2D2420 entry (3→4, 3→10, 4→5, 8→9, 6→8, ...): the
                // figure goes straight from C7 to C8 (0x22BB58..0x22BEA8).
                // W+1DA += 16 per frame; position = C7 + (C8 − C7)·W+1DA/1024
                // (y kept), heading atan2 of the difference (W+200); at
                // 1024 W+1DA = 0, W+1D8 = 3 on C8: 64 frames in all.
                self.figure.state = 1;
                self.figure.sub = 0;
                self.figure.index = 0;
                self.figure.path = None;
                self.figure.forward = true;
                self.figure.straight_from = Some(from);
                self.figure.position = self.node_position(from);
                self.figure.drawn = (1, 0, 0);
                self.walk_advance();
                self.walk_started = true;
            }
            _ => {
                // C7 == C8 (0x22BEA8): arrived at once.
                self.figure.state = 3;
                self.figure.path = None;
                self.figure.straight_from = None;
                self.figure.position = self.node_position(to);
            }
        }
    }

    fn walk_update(&mut self) {
        if !self.walk_started {
            self.figure.drawn = (self.figure.state, self.figure.sub, self.figure.index);
        }
        if matches!(self.figure.state, 1 | 2) && !self.walk_started {
            self.walk_advance();
        }
        self.walk_started = false;
    }

    fn walk_advance(&mut self) {
        if let Some(from) = self.figure.straight_from {
            self.figure.sub += STRAIGHT_WALK_STEP;
            let (a, b) = (self.node_position(from), self.node_position(self.figure.node));
            if self.figure.sub >= STRAIGHT_WALK_FULL {
                self.figure.sub = 0;
                self.figure.state = 3;
                self.figure.straight_from = None;
                self.figure.position = b;
            } else {
                let t = f64::from(self.figure.sub) / f64::from(STRAIGHT_WALK_FULL);
                self.figure.position = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            }
            return;
        }
        let Some(path) = self.figure.path else {
            return;
        };
        let dots: Vec<[f64; 2]> = {
            let p = &self.data.paths[usize::from(path)];
            let mut d: Vec<[f64; 2]> = p
                .dots
                .iter()
                .map(|d| [f64::from(d[0]), f64::from(d[1])])
                .collect();
            if !self.figure.forward {
                d.reverse();
            }
            d
        };
        self.figure.sub += WALK_STEP;
        if self.figure.sub >= WALK_DOT {
            self.figure.sub -= WALK_DOT;
            self.figure.index += 1;
        }
        let end = self.node_position(self.figure.node);
        if self.figure.index >= dots.len() {
            self.figure.state = 3;
            self.figure.position = end;
            return;
        }
        let a = dots[self.figure.index];
        let b = dots.get(self.figure.index + 1).copied().unwrap_or(end);
        let f = f64::from(self.figure.sub) / f64::from(WALK_DOT);
        self.figure.position = [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f];
    }

    fn push_check(&mut self, events: &mut Vec<MapEvent>) {
        if self.travel == 3 && self.text.is_none() && self.figure.state == 3 {
            let opponent = u8::try_from(self.save.opponent).unwrap_or(0);
            // 0x232E40: p3/p4 are side 0's and side 1's identities; the
            // player (1100) takes side SAVE[0], who moves first (p2)
            // [F 0x232E70..0x232EC4].
            let (p3, p4) = if self.save.side == 0 {
                (DUEL_PARAM_MODE, DUEL_PARAM_BASE + i32::from(opponent))
            } else {
                (DUEL_PARAM_BASE + i32::from(opponent), DUEL_PARAM_MODE)
            };
            let request = DuelRequest {
                node: self.save.destination,
                opponent,
                params: [i32::from(opponent), i32::from(self.save.side), p3, p4],
            };
            self.duel_request = Some(request.clone());
            self.travel = 4;
            events.push(MapEvent::DuelRequest(request));
        }
    }

    fn dots_update(&mut self, events: &mut Vec<MapEvent>) {
        let side = self.save.side;
        for index in 0..PATHS_PER_SIDE {
            // Sea routes keep their state: the updater skips them by index
            // [F 0x22C408..0x22C4D8; emu logs/white-mako-win: white path 5
            // stays 1].
            if self.data.path(side, index as u8).sea_route {
                continue;
            }
            let count = self.paths[index].alpha.len();
            let path = &mut self.paths[index];
            if !matches!(path.state, 1 | 2) {
                continue;
            }
            path.frames += 1;
            let mut complete = count > 0;
            for k in 0..count {
                let dot = if path.state == 2 { count - 1 - k } else { k };
                if path.frames > DOT_START_FRAMES * (k as u32 + 1) {
                    path.alpha[dot] = (path.alpha[dot] + DOT_ALPHA_STEP).min(DOT_FULL_ALPHA);
                }
                complete &= path.alpha[dot] >= DOT_FULL_ALPHA;
            }
            if complete {
                let dir = path.state;
                path.state = 3;
                events.push(MapEvent::PathRevealed(index as u8));
                let node = self.data.reveal_end_on(side, index as u8, u16::from(dir));
                if self.save.state(node) == STATE_HIDDEN && !self.rises[usize::from(node)].rising {
                    self.rises[usize::from(node)] = RiseState {
                        rising: true,
                        level: 0,
                    };
                    events.push(MapEvent::Sfx(SFX_RISE));
                    events.push(MapEvent::NodeRising(node));
                }
            }
        }
    }

    fn rise_update(&mut self, events: &mut Vec<MapEvent>) {
        // Mako's pages hold the rise the ship's arrival flagged: node 16
        // rises from 12041's tail [emu trace-dover: rise f369, Strait 1
        // f432; H: the node pass's rise step is what pauses].
        if matches!(self.handler, Handler::MapStory { event, .. } if Self::is_dialogue_event(event)) {
            return;
        }
        for node in 0..NODE_COUNT {
            let rise = &mut self.rises[node];
            if !rise.rising {
                continue;
            }
            rise.level += RISE_STEP;
            if rise.level >= RISE_FULL {
                rise.level = 0;
                rise.rising = false;
                self.save.node_states[node] = STATE_OPEN;
                // 0x22A7C8: a finished rise also stops the ship (W+1A4 = 0).
                self.ship.state = 0;
                events.push(MapEvent::NodeRevealed(node as u8));
            }
        }
    }

    fn text_update(&mut self, pad: Pad, events: &mut Vec<MapEvent>) {
        let Some(mut text) = self.text.take() else {
            return;
        };
        match text.phase {
            1 => text.phase = 2,
            2 => {
                let step = if text.style == 2 { STYLE2_WINDOW_STEP } else { WINDOW_STEP };
                text.openness = (text.openness + step).min(WINDOW_OPEN);
                if text.openness >= WINDOW_OPEN {
                    text.phase = 3;
                    text.timer = 0;
                }
            }
            3 => {
                text.timer += 1;
                text.revealed = text.timer;
                if text.timer >= text.reveal_frames {
                    text.phase = 4;
                }
            }
            4 => {
                if pad.cross {
                    events.push(MapEvent::Sfx(SFX_PAGE));
                    text.phase = 5;
                    text.timer = 0;
                }
            }
            5 => {
                text.timer += 1;
                if text.timer >= PAGE_TURN_FRAMES {
                    // 0x22E630: the counter steps over blank messages and the
                    // window closes once it exceeds 4·groups.
                    let limit = text.groups * PAGES_PER_GROUP;
                    let mut page = text.page + 1;
                    loop {
                        if page > limit {
                            text.phase = 11;
                            break;
                        }
                        let next = text.base + page;
                        match self.data.is_blank(next) {
                            Some(false) => {
                                text.page = page;
                                text.message = next;
                                text.reveal_frames = self.reveal_frames(next);
                                text.revealed = 0;
                                text.phase = 2;
                                events.push(MapEvent::TextPage {
                                    message: next,
                                    page: text.page,
                                });
                                break;
                            }
                            Some(true) => page += 1,
                            None => {
                                self.note("a text page after the imported ones is not available; the window closes");
                                text.phase = 11;
                                break;
                            }
                        }
                    }
                }
            }
            11 => text.phase = 12,
            12 | 13 if text.style == 2 => {
                // Style 2 [emu brest-win f339 12, f340 13, f364 14 → closed].
                if text.phase == 12 {
                    text.phase = 13;
                    text.timer = 0;
                } else {
                    text.timer += 1;
                    if text.timer >= STYLE2_CLOSE_FRAMES {
                        events.push(MapEvent::TextClosed);
                        return;
                    }
                }
            }
            _ => {
                text.openness -= WINDOW_STEP;
                if text.openness <= 0 {
                    events.push(MapEvent::TextClosed);
                    return;
                }
            }
        }
        self.text = Some(text);
    }

    fn return_update(&mut self, node: u8, won: bool, phase: u8, events: &mut Vec<MapEvent>) {
        let mut phase = phase;
        match phase {
            0 => {
                // ph0: camera on the node, cursor reset, countdown armed, fade restarted.
                self.camera_target = self.node_position(node);
                self.cursor = CURSOR_START;
                self.countdown = COUNTDOWN_START;
                self.fade = 0;
                self.fade_hold = true;
                phase = 1;
            }
            1 => {
                if self.fade >= FADE_FULL {
                    // The loss/win switch cases of Stonehenge branch on C2
                    // (212/312 Seto, 213/313 Manawyddan).
                    let second = matches!(
                        self.save.event,
                        EVENT_MANAWYDDAN_WON | EVENT_MANAWYDDAN_LOST
                    );
                    let text = if self.save.side != 0 {
                        Some(self.data.side_text(1, node, 0, 0))
                    } else {
                        self.data.text(node).map(|t| {
                            if node == STONEHENGE_NODE && second {
                                t.second_visit.clone()
                            } else {
                                Some(t.first_visit())
                            }
                        })
                    };
                    let base = match (&text, won) {
                        (Some(Some(t)), true) => t.win_base.map(|b| (b, t.win_groups)),
                        (Some(Some(t)), false) => t.loss_base.map(|b| (b, t.loss_groups)),
                        _ => None,
                    };
                    match base {
                        Some((base, groups)) => self.start_text(base, groups.unwrap_or(1), events),
                        None if self.save.side != 0 && self.data.white_texts.is_empty() => {
                            self.note(WHITE_TEXTS_MISSING)
                        }
                        None => self.note("win/loss text base of this node is not decoded"),
                    }
                    phase = if won && self.save.rose_card_on_win(&self.data, node) {
                        2
                    } else {
                        3
                    };
                }
            }
            2 | 3 => {
                if self.text.is_none() {
                    self.countdown -= COUNTDOWN_STEP;
                    if self.countdown <= 0 {
                        self.countdown = 0;
                        if phase == 2 {
                            self.rose = Some(RoseAnimation { spin: 0, scale: 0 });
                            events.push(MapEvent::RoseCard { node });
                            phase = 4;
                        } else {
                            self.close_return(node, won, events);
                            return;
                        }
                    }
                }
            }
            _ => {
                if let Some(rose) = &mut self.rose {
                    rose.spin = rose.spin.wrapping_add(ROSE_SPIN_STEP);
                    rose.scale += ROSE_SCALE_STEP;
                    if rose.scale >= ROSE_SCALE_FULL {
                        self.rose = None;
                        self.close_return(node, won, events);
                        return;
                    }
                }
            }
        }
        self.handler = Handler::Return { node, won, phase };
    }

    /// Close logic 0x230F8C (RED-ROUTE.md §2.3).
    fn close_return(&mut self, node: u8, won: bool, events: &mut Vec<MapEvent>) {
        let event = self.save.event;
        let state = self.save.state(node);
        if self.save.side != 0 && won && state < STATE_BEATEN && (300..10000).contains(&event) {
            self.white_close(node, event, events);
        } else if won && self.save.side == 0 && event == EVENT_MANAWYDDAN_WON && state != STATE_HIDDEN {
            // Manawyddan beaten: state 5, story 11180 (ending).
            self.save.node_states[usize::from(node)] = STATE_BEATEN_SECOND;
            events.push(MapEvent::NodeBeaten(node));
            self.start_story(EVENT_MANAWYDDAN_WIN, events);
        } else if won && state < STATE_BEATEN && (300..10000).contains(&event) {
            self.save.node_states[usize::from(node)] = STATE_BEATEN;
            events.push(MapEvent::NodeBeaten(node));
            match event {
                308 => self.start_story(EVENT_BOSWORTH_WIN, events),
                312 => self.start_story(EVENT_SETO_WIN, events),
                _ => {
                    self.save.event = event - 200;
                    events.push(MapEvent::EventChanged(self.save.event));
                    if (EVENT_FIRST_ARRIVE..=EVENT_LAST_ARRIVE).contains(&self.save.event) {
                        self.handler = Handler::Arrive {
                            node,
                            phase: 0,
                            timer: 0,
                        };
                        return;
                    }
                    self.handler = self.reveal_handler(node);
                    // The reveal's first frame runs now (cam = 1 in the same frame).
                    if let Handler::Reveal {
                        node,
                        paths,
                        phase,
                        timer,
                        centroid,
                    } = self.handler.clone()
                    {
                        self.reveal_update(node, paths, phase, timer, centroid, events);
                    }
                }
            }
        } else {
            if !won && self.save.side == 0 && event == EVENT_MANAWYDDAN_LOST {
                self.save.node_states[usize::from(node)] = STATE_LOST_SECOND;
                events.push(MapEvent::NodeLost(node));
            } else if !won && state == STATE_OPEN {
                self.save.node_states[usize::from(node)] = STATE_LOST;
                events.push(MapEvent::NodeLost(node));
            }
            self.save.event = 0;
            events.push(MapEvent::EventChanged(0));
            self.handler = Handler::Free;
        }
    }

    /// Events 110..=113 (0x228D00) [emu, logs/exeter-win and bosworth-trigger]:
    /// the close logic leaves the pan state running (W+60 += 16 to 0x100,
    /// cleared the frame after), then sub-step 1 runs 0x228410 (state 3,
    /// the Bosworth trigger) and the same pan again; the handler then acts
    /// as event 0 while C2 keeps 110..=113.
    fn arrive_update(&mut self, node: u8, phase: u8, timer: u32, events: &mut Vec<MapEvent>) {
        let timer = timer + 1;
        match phase {
            0 => {
                if timer >= ARRIVE_FIRST_FRAMES {
                    self.w53c4 = self.save.c9;
                    self.save.node_states[usize::from(node)] = STATE_BEATEN;
                    if self.bosworth_trigger(events) {
                        return;
                    }
                    self.handler = Handler::Arrive {
                        node,
                        phase: 1,
                        timer: 0,
                    };
                } else {
                    self.handler = Handler::Arrive { node, phase, timer };
                }
            }
            _ => {
                if timer >= ARRIVE_PAN_FRAMES {
                    events.push(MapEvent::Arrived(node));
                    self.handler = Handler::Free;
                } else {
                    self.handler = Handler::Arrive { node, phase, timer };
                }
            }
        }
    }

    /// Generic reveal 0x228210(side 0, node, list): 16-frame pan to the node,
    /// paths set to their direction, 32-frame pan to the centroid, C2 = 0.
    fn reveal_update(
        &mut self,
        node: u8,
        paths: Vec<(u8, u16)>,
        phase: u8,
        timer: u32,
        centroid: [f64; 2],
        events: &mut Vec<MapEvent>,
    ) {
        let mut phase = phase;
        let mut timer = timer;
        let mut centroid = centroid;
        match phase {
            0 | 1 => {
                if phase == 0 {
                    // 0x228210 sub-step 0 → 0x228410: node state 3 and the
                    // Bosworth trigger, which abandons this event.
                    self.save.node_states[usize::from(node)] = STATE_BEATEN;
                    self.w53c4 = self.save.c9;
                    if self.save.side != 0 {
                        // 0x228210 sub-step 0: letterbox mode 0, then
                        // 0x228410(1, node, 3, list, 1) (location, the
                        // hidden end's path cleared) [F].
                        self.white_reveal_setup(node, &paths);
                        if node == WHITE_BOULOGNE_NODE {
                            // 106: (4, 1) once (W+546E) [F 0x227884].
                            events.push(MapEvent::Music {
                                track: MUSIC_MAP,
                                keep: true,
                            });
                        }
                    } else if self.bosworth_trigger(events) {
                        return;
                    }
                    // The pan starts and advances on the entry frame.
                    self.lerp = Some(Lerp {
                        start: self.camera_target,
                        end: self.node_position(node),
                        t: 0,
                        full: REVEAL_NODE_FRAMES as i32,
                        step: 1,
                    });
                    phase = 1;
                }
                if let Some(lerp) = &mut self.lerp {
                    lerp.t += lerp.step;
                    self.camera_target = lerp.value();
                    if lerp.t >= lerp.full {
                        self.camera_target = lerp.end;
                        self.lerp = None;
                    }
                }
                // 0x228210 sub-step 1 marks the paths once the letterbox
                // it closed is done (W+1C = 2); only the voyage (event 102)
                // models that letterbox, the other reveals mark them at the
                // end of the pan (fitted to their logs, see `sail.rs`).
                let mark = if self.sail_fade {
                    self.letterbox.active == 2
                } else {
                    self.lerp.is_none()
                };
                if mark {
                    for (path, dir) in &paths {
                        let state = &mut self.paths[usize::from(*path)];
                        if state.state == 0 {
                            state.state = if *dir == 2 { 2 } else { 1 };
                            state.frames = 0;
                            events.push(MapEvent::PathRevealing(*path));
                        }
                    }
                }
                if self.lerp.is_none() {
                    let mut ends = Vec::new();
                    for (path, dir) in &paths {
                        ends.push(self.node_position(self.data.reveal_end_on(self.save.side, *path, *dir)));
                    }
                    if !ends.is_empty() {
                        centroid = [
                            ends.iter().map(|e| e[0]).sum::<f64>() / ends.len() as f64,
                            ends.iter().map(|e| e[1]).sum::<f64>() / ends.len() as f64,
                        ];
                    } else {
                        centroid = self.camera_target;
                    }
                    phase = 2;
                }
            }
            2 => {
                self.lerp = Some(Lerp {
                    start: self.camera_target,
                    end: centroid,
                    t: 0,
                    full: REVEAL_CENTROID_FRAMES as i32,
                    step: 1,
                });
                phase = 3;
            }
            3 => {
                if let Some(lerp) = &mut self.lerp {
                    lerp.t += lerp.step;
                    self.camera_target = lerp.value();
                    if lerp.t >= lerp.full {
                        self.camera_target = lerp.end;
                        self.lerp = None;
                        phase = 4;
                        // 0x228880 mode 2 clears C2 on the pan's last frame
                        // [F; emu 620-y31 f372]; the other reveals keep
                        // their fitted extra frame.
                        if self.sail_fade {
                            self.save.event = 0;
                            events.push(MapEvent::EventChanged(0));
                            self.handler = Handler::Free;
                            return;
                        }
                    }
                }
            }
            _ => {
                timer += 1;
                if timer >= 1 {
                    self.save.event = 0;
                    events.push(MapEvent::EventChanged(0));
                    self.handler = Handler::Free;
                    return;
                }
            }
        }
        self.handler = Handler::Reveal {
            node,
            paths,
            phase,
            timer,
            centroid,
        };
    }

    /// Camera phase for logs: 0 static, 1 pan to node, 2 pan to centroid.
    pub fn camera_phase(&self) -> u8 {
        match &self.handler {
            Handler::Reveal { phase, .. } => match phase {
                0 | 1 => 1,
                2 | 3 => 2,
                _ => 0,
            },
            _ => 0,
        }
    }

    /// Text phase (W+537B) for logs: the map's page window, or the story
    /// window of 11120/11124/11149.
    pub fn text_phase(&self) -> u8 {
        self.text
            .as_ref()
            .map(|t| t.phase)
            .or(self.story_window.as_ref().map(|w| w.phase))
            .or(self.story_dialogue.as_ref().map(|d| d.phase))
            .unwrap_or(0)
    }

    pub fn window_openness(&self) -> i32 {
        self.text.as_ref().map_or(0, |t| t.openness)
    }

    /// One-line caption of the model state.
    pub fn caption(&self) -> String {
        let hovered = self
            .hovered
            .map(|n| self.data.node(n).name.clone())
            .unwrap_or_else(|| "-".into());
        let text = self
            .text
            .as_ref()
            .map(|t| format!(" · text {} phase {}", t.message, t.phase))
            .or(self
                .story_window
                .as_ref()
                .map(|w| format!(" · story window {} phase {}", w.message, w.phase)))
            .or(self
                .story_dialogue
                .as_ref()
                .filter(|d| d.phase < 12)
                .map(|d| format!(" · story page {} phase {}", d.message, d.phase)))
            .unwrap_or_default();
        let duel = self
            .duel_request
            .as_ref()
            .map(|d| {
                format!(
                    " · duel against {} would start here (params {:?})",
                    self.data.node(d.node).name,
                    d.params
                )
            })
            .unwrap_or_default();
        format!(
            "frame {} · C2 {} · cursor ({},{}) · hover {hovered} · travel {} · figure {}{text}{duel}",
            self.frame,
            self.save.event,
            self.cursor.0,
            self.cursor.1,
            self.travel,
            self.figure.state
        )
    }
}

/// Draw role of a map object slot (MAP.md §1, from the draw code).
pub fn slot_role(slot: u16) -> &'static str {
    match slot {
        34..=39 => "board tile (always drawn, 0x22A504)",
        40 | 41 => "piece (always drawn, 0x22ADB4)",
        42 => "node base model 8 (Brest and white nodes)",
        43 => "node base model 9 (red nodes 1-10)",
        44 => "node base model 10 (Stonehenge)",
        45..=60 => "castle (node castle model + 34)",
        61 => "red ship (route kinds 0/1/4, 0x22CA00)",
        62 => "red path dot (0x22C330)",
        63 => "white path dot (0x22C330)",
        64 => "second boat (route kinds 2/3)",
        65 => "small record (role untraced)",
        _ => "not a map slot",
    }
}

/// Read a `map-data.json` written by `roses import-campaign-map`.
pub fn load_map_data(path: &std::path::Path) -> Result<MapData> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(
        &mut std::io::Read::take(file, 4 * 1024 * 1024 + 1),
        &mut bytes,
    )
    .map_err(|e| e.to_string())?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("map-data.json exceeds 4 MiB".into());
    }
    let data: MapData =
        serde_json::from_slice(&bytes).map_err(|e| format!("map-data.json: {e}"))?;
    data.validate()?;
    Ok(data)
}

mod figure;
pub use figure::{
    figure_height, figure_heights, node_pass_runs, FigureObject, CURSOR_RAY_SCALE,
    FIGURE_HEIGHT_CARDS, FIGURE_HEIGHT_TABLE_EE, FIGURE_MOTION, FIGURE_OBJECT, FIGURE_RECENTRE,
    FIGURE_SCALE, FIGURE_BASE_ROTATION, GROUND_PLANE,
};
mod free_duel;
pub use free_duel::{FREE_DUEL_PLACES, SIDE0_BASE_MODEL};
mod sail;
mod story_window;
mod system;
mod tutorial_menu;
pub use tutorial_menu::{TutorialMenu, SFX_LESSON, TUTORIAL_LABEL_BASE, TUTORIAL_ROWS};
pub use system::{
    MenuPad, OptionsPanel, TitleDialog, DIALOG_HIDDEN_Y, DIALOG_SLIDE_STEP, DIALOG_SLIDE_UPDATES, OPTION_ROWS,
    RETURN_TO_TITLE_MESSAGE,
};
mod white;
pub use white::{
    white_story_continuation, EVENT_DOVER_STORY, EVENT_DOVER_VOYAGE, EVENT_STONEHENGE_VOYAGE,
    EVENT_STRAIT_STORY, EVENT_STRAIT_VOYAGE, EVENT_BOULOGNE_REVEAL, EVENT_WHITE_FINAL,
    EVENT_WHITE_WIN, WHITE_BOULOGNE_NODE, WHITE_DOVER_NODE, WHITE_ROSE_NODES, WHITE_STORY_EVENTS,
    WHITE_STRAIT_NODE, WHITE_TEXTS_MISSING,
};
pub use story_window::{
    dialogue_pages, map_story_message_ids, window_rect as story_window_rect, StoryDialogue, StoryWindow,
    DIALOGUE_EVENTS, DIALOGUE_FACE_X, DIALOGUE_FACE_Y, DIALOGUE_PANEL_FULL, DIALOGUE_TEXT_X, DIALOGUE_TEXT_Y, ADVANCE_MARK as STORY_ADVANCE_MARK,
    EVENT_BOSWORTH_FAREWELL, EVENT_BOSWORTH_PAINTINGS, WINDOW_EVENTS, WINDOW_MESSAGES,
    WINDOW_MESSAGE_IDS, WINDOW_TEXT_DX as STORY_WINDOW_TEXT_DX,
    WINDOW_TEXT_Y as STORY_WINDOW_TEXT_Y,
};
pub use sail::{
    heading, sea_route_tail, ship_route, Caption, Letterbox, Ship, BREST_NODE, EVENT_BREST_STORY,
    EVENT_FIRST_VOYAGE, EVENT_MILFORD_REVEAL, EVENT_SECOND_VOYAGE, MILFORD_NODE, SHIP_MODEL,
    VOYAGE_CAPTION_BASE,
};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod white_tests;
#[cfg(test)]
mod mode_tests;
