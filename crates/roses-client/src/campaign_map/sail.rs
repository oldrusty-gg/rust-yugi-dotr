//! The opening voyage: map events 100 (Stonehenge → Brest), 101 (Brest →
//! Milford Haven) and 102 (Milford Haven's reveal). Source facts (PAL
//! SLES_515.79, private research `story/the project notes):
//!
//! - Dispatcher 0x2263B0: C2 100 → 0x2266FC, 101 → 0x226968, 102 → 0x226B78
//!   (the generic reveal 0x228210 on node 1) [F].
//! - Event 100, sub-step W+536C [F]: 0 letterbox on (W+1C = 1, mode W+1A = 2),
//!   ship 0x2286F0(0, kind 0), 0x228410(side, node 0, state 0, node 11's red
//!   list), location 11; 1 when the HUD fade-out 0x22C180(0), then the camera
//!   pan 0x228720 and then the letterbox are done: ship 0x2286F0(1, 0); 2
//!   camera = ship until the ship arrives (W+1A4 = 3): music (27, 0), caption
//!   (W+537B = 1, counter W+5380 = 1); 3 camera = ship until Brest's state is
//!   nonzero and the HUD fade-in 0x22C180(1) is done: letterbox mode 3; 4 the
//!   caption's wait phase 4 becomes 5 (no button), and once the caption is
//!   closed (12) and the letterbox done (W+1C = 2): C2 = 11020.
//! - Event 101 [F]: SAVE[3] = 1 every frame; 0 camera target = Brest's node,
//!   letterbox mode 2, ship 0x2286F0(0, kind 1), 0x228410(side, 0, 3, node
//!   0's red list), 0x228410(side, 1, 0, [1, 2]), SAVE[3] = 0 (1 again on
//!   the next frame); 1 HUD fade-out, pan, letterbox: ship 0x2286F0(1, 1); 2
//!   camera = ship until arrival; 3 camera = ship until Milford Haven's state
//!   is nonzero and the HUD fade-in is done: letterbox mode 0, C2 = 102.
//! - 0x2286F0(state, kind): W+1A4 = state, W+1B2 = kind, W+1B0 = 0 (sample),
//!   W+1A6 = 0 (sub-step), W+1B4 = 1 (visible). The draw pass (0x22AC94)
//!   maps the kind to (side, path, model): 0 → (0, 15, 27), 1 → (0, 0, 27),
//!   and for the White Rose route (`white.rs`) 2 → (1, 4, 30), 3 → (1, 5,
//!   30), 4 → (1, 11, 27) [F]; model 30 is slot 64, Mako's boat.
//! - Ship 0x22CA00 per frame [F]: position = x[i] + (x[i+1] − x[i])·t/1024
//!   (same for z, y = node 0's y) from the runtime spline arrays; heading
//!   (W+1A8) = trunc(65536·atan2(dx, dz)/2π) of segment (i−1, i) when i ≥
//!   count, of segment i when i = count − 1, else of segments i and i+1
//!   blended by t; the model slot is 34 + 27. Then, when the state is
//!   nonzero, t < 0x400 and i < count: state 1 plays SFX 0x193 and becomes 2;
//!   state 2 adds 0x100 to t and at 0x400 moves to the next sample; past the
//!   last one t = 0, i = count, state 3 and the path's end node starts rising
//!   (W+52EC). The node rise (0x22A780) finishing sets the node state 1 and
//!   the ship state 0.
//! - The runtime sea-route arrays hold one sample beyond `count`: the last
//!   segment at t = 0.9 [emu: both savestates' arrays for paths 0 and 15].
//! - 0x22C180(mode): HUD alpha W+5480 −4 to 0 (mode 0) or +4 to 0x80 (mode
//!   1), true once there [F]. 0x228720: when W+60 = 0 the start W+50/58 =
//!   target; W+60 += 0x10 (max 0x100); target = start·(256 − t)/256 +
//!   ship·t/256 in f32; at 0x100 W+60 = 0 and true [F].
//! - Letterbox 0x233750 (argument 0): only while W+1C = 1 and W+6C = 0, the
//!   three counters W+1E/W+20/W+22 step by 8 per mode (0 all to 0; 1 first
//!   to 0x48, others to 0; 2 first to 0x48 then second to 0x48, third to 0;
//!   3 first, then second to 0x48, then third to 0x70); three finished
//!   counters set W+1C = 2. Two opaque black bars (table 0x297E60) cover y
//!   0..S and 512−S..512 of the 512-line canvas, S = the counters' sum [F].
//! - W+6C (the map's `fade`) rises by 20 only while W+6A and W+6E are set;
//!   the draw pass clears W+6E during events 100/101 and while the camera
//!   pans (W+5372 ≠ 0) [F 0x237180, 0x22A888]. During the voyage the model
//!   keeps `fade` at 0 and, from event 102 on, lowers it while a pan runs and
//!   raises it otherwise, until the free map (whose own rule applies) [F for
//!   these branches; W+6A is taken as set, as in the logs].
//! - Caption (C2 = 100, side 0): base 0x987 (message base + W+5380 = 2440),
//!   W+537E = 1, style W+537C = 1 (0x22D0A0) [F]. Style 1 (0x2314B8) fades
//!   four alphas W+5382.. by 0x30 (the second and third once the first is
//!   above 0x40, the fourth once the second is), phase 1 → 2 → 3 → 4 each
//!   when the fourth is 0x80; phase 5 fades them out the same way and gives
//!   11 at 0; 11 → 12 [F, frame order checked against the emulator log]. The
//!   Cross handler skips the page turn for C2 < 10001 unless the cursor is
//!   over a node (0x239738), so the caption ignores the pad [F].
//!
//! Per-frame emulator logs: `sail/event100-ship.json` (588-x6) and
//! `sail/event101-ship.json` (620-y31) from `tools/run_sail_ship.py`.
//! Unknown and explicit [H]: the HUD alpha and letterbox at the map entry
//! before event 100 (taken as 0x80 and closed) and after story 11020 (taken
//! as event 100's end: 0x80 and 72/72/112, closed by the original to 96 on
//! the first frame of 101, i.e. one frame later here).
use super::*;

pub const EVENT_FIRST_VOYAGE: i16 = 100;
pub const EVENT_BREST_STORY: i16 = 11020;
pub const EVENT_SECOND_VOYAGE: i16 = 101;
pub const EVENT_MILFORD_REVEAL: i16 = 102;
pub const BREST_NODE: u8 = 0;
pub const MILFORD_NODE: u8 = 1;
/// Event 101's second 0x228410 list (immediates in 0x226A4C) [F].
pub const MILFORD_PATHS: [u8; 2] = [1, 2];
/// Ship kind → (side, path, model) of the draw pass 0x22AC94 [F].
pub const SHIP_ROUTES: [(u8, u8, u8, i16); 5] = [
    (0, 0, 15, 27),
    (1, 0, 0, 27),
    (2, 1, 4, 30),
    (3, 1, 5, 30),
    (4, 1, 11, 27),
];
/// The red ship's model index (0x22AD54: 0x1B; slot = 34 + model) [F].
pub const SHIP_MODEL: i16 = 27;

/// (side, path, model) of ship kind `kind` (0x22AC94).
pub fn ship_route(kind: u8) -> Option<(u8, u8, i16)> {
    SHIP_ROUTES
        .iter()
        .find(|r| r.0 == kind)
        .map(|r| (r.1, r.2, r.3))
}
pub const SHIP_STEP: i32 = 0x100;
pub const SHIP_SAMPLE: i32 = 0x400;
pub const SFX_SHIP: u16 = 0x193;
pub const MUSIC_ARRIVAL: u16 = 27;
pub const HUD_STEP: i32 = 4;
pub const CAMERA_PAN_STEP: i32 = 0x10;
pub const CAMERA_PAN_FULL: i32 = 0x100;
/// Caption base (0x22D0A0: W+5380 + 0x987) and groups (W+537E = 1) [F].
pub const VOYAGE_CAPTION_BASE: u16 = 0x987;
pub const CAPTION_ALPHA_STEP: i32 = 0x30;
pub const CAPTION_ALPHA_FULL: i32 = 0x80;
pub const CAPTION_ALPHA_GATE: i32 = 0x40;
pub const LETTERBOX_STEP: i32 = 8;
pub const LETTERBOX_BAR: i32 = 0x48;
pub const LETTERBOX_THIRD: i32 = 0x70;
pub const CANVAS_LINES: i32 = 512;
/// The sea-route arrays' extra sample (see the module notes) [emu].
pub const SEA_TAIL_T: f64 = 0.9;

/// The ship (W+1A4..W+1C0).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Ship {
    /// W+1A4: 0 idle, 1 start, 2 sailing, 3 arrived.
    pub state: u8,
    /// W+1B2.
    pub kind: u8,
    /// W+1B0: spline sample.
    pub index: usize,
    /// W+1A6: 0..0x400 between samples.
    pub sub: i32,
    /// W+1B4.
    pub visible: bool,
    /// W+1B8/1BC/1C0 (world x, y, z), computed by the draw pass.
    pub position: [f32; 3],
    /// W+1A8 (low 16 bits, 65536 per turn about y).
    pub heading: u16,
}

/// The letterbox (W+1C, W+1A, W+1E/20/22).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Letterbox {
    /// W+1C: 0 off, 1 animating, 2 done.
    pub active: u8,
    /// W+1A: target mode 0..=3.
    pub mode: u8,
    pub bars: [i32; 3],
}

impl Letterbox {
    /// The height of each bar in the 512-line canvas.
    pub fn height(&self) -> i32 {
        self.bars.iter().sum::<i32>().clamp(0, CANVAS_LINES)
    }

    pub(crate) fn start(&mut self, mode: u8) {
        self.active = 1;
        self.mode = mode;
    }

    /// 0x233750 with argument 0 (the caller checks W+1C = 1 and W+6C = 0).
    pub(crate) fn step(&mut self) {
        fn down(c: &mut i32, done: &mut u8) {
            if *c >= 0 {
                *c -= LETTERBOX_STEP;
                if *c <= 0 {
                    *c = 0;
                    *done += 1;
                }
            }
        }
        let mut done = 0u8;
        let [b0, b1, b2] = &mut self.bars;
        match self.mode {
            0 => {
                down(b0, &mut done);
                down(b1, &mut done);
                down(b2, &mut done);
            }
            1 => {
                if *b0 <= LETTERBOX_BAR {
                    *b0 += LETTERBOX_STEP;
                    if *b0 >= LETTERBOX_BAR {
                        *b0 = LETTERBOX_BAR;
                        done += 1;
                    }
                }
                down(b1, &mut done);
                down(b2, &mut done);
            }
            2 => {
                if *b0 < LETTERBOX_BAR {
                    *b0 += LETTERBOX_STEP;
                    if *b0 >= LETTERBOX_BAR {
                        *b0 = LETTERBOX_BAR;
                        done += 1;
                    }
                } else {
                    *b0 = LETTERBOX_BAR;
                    // The branch delay slot counts the first bar.
                    done += 1;
                    if *b1 <= LETTERBOX_BAR {
                        *b1 += LETTERBOX_STEP;
                        if *b1 >= LETTERBOX_BAR {
                            *b1 = LETTERBOX_BAR;
                            done += 1;
                        }
                    }
                }
                down(b2, &mut done);
            }
            3 => {
                if *b0 < LETTERBOX_BAR {
                    *b0 += LETTERBOX_STEP;
                    if *b0 >= LETTERBOX_BAR {
                        *b0 = LETTERBOX_BAR;
                        done += 1;
                    }
                } else if *b1 < LETTERBOX_BAR {
                    *b0 = LETTERBOX_BAR;
                    *b1 += LETTERBOX_STEP;
                    done += 1;
                    if *b1 >= LETTERBOX_BAR {
                        *b1 = LETTERBOX_BAR;
                        done += 1;
                    }
                } else {
                    *b0 = LETTERBOX_BAR;
                    *b1 = LETTERBOX_BAR;
                    *b2 += LETTERBOX_STEP;
                    done += 2;
                    if *b2 >= LETTERBOX_THIRD {
                        *b2 = LETTERBOX_THIRD;
                        done += 1;
                    }
                }
            }
            _ => {}
        }
        if done >= 3 {
            self.active = 2;
        }
    }
}

/// A style-1 place caption (W+537B phases, W+5382.. alphas).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Caption {
    pub base: u16,
    pub message: u16,
    /// 1 start, 2 fade-in, 3 shown, 4 wait, 5 fade-out, 11, 12 closed.
    pub phase: u8,
    pub alpha: [i32; 4],
}

impl Caption {
    pub(super) fn new(base: u16) -> Self {
        Self {
            base,
            message: base + 1,
            phase: 1,
            alpha: [0; 4],
        }
    }

    /// The draw-pass update of 0x2314B8.
    pub(super) fn step(&mut self) {
        match self.phase {
            1..=3 => {
                if self.phase == 1 {
                    self.alpha = [0; 4];
                    self.phase = 2;
                }
                let a = &mut self.alpha;
                a[0] = (a[0] + CAPTION_ALPHA_STEP).min(CAPTION_ALPHA_FULL);
                for k in 1..3 {
                    if a[0] > CAPTION_ALPHA_GATE {
                        a[k] = (a[k] + CAPTION_ALPHA_STEP).min(CAPTION_ALPHA_FULL);
                    }
                }
                if a[1] > CAPTION_ALPHA_GATE {
                    a[3] = (a[3] + CAPTION_ALPHA_STEP).min(CAPTION_ALPHA_FULL);
                }
                if a[3] >= CAPTION_ALPHA_FULL {
                    self.phase += 1;
                }
            }
            4 => self.alpha = [CAPTION_ALPHA_FULL; 4],
            5 => {
                let a = &mut self.alpha;
                a[0] = (a[0] - CAPTION_ALPHA_STEP).max(0);
                for k in 1..3 {
                    if a[0] <= CAPTION_ALPHA_GATE {
                        a[k] = (a[k] - CAPTION_ALPHA_STEP).max(0);
                    }
                }
                if a[1] <= CAPTION_ALPHA_GATE {
                    a[3] = (a[3] - CAPTION_ALPHA_STEP).max(0);
                }
                if a[3] <= 0 {
                    self.phase = 11;
                }
            }
            11 => self.phase = 12,
            _ => {}
        }
    }
}

/// The sea-route arrays' extra sample (last segment at t = 0.9).
pub fn sea_route_tail(waypoints: &[[i16; 2]]) -> Option<[f32; 2]> {
    let points: Vec<[f64; 2]> = waypoints
        .iter()
        .map(|w| [f64::from(w[0]), f64::from(w[1])])
        .collect();
    let n = points.len();
    if n < 2 {
        return None;
    }
    let m = natural_spline_tangents(&points);
    let p = hermite(points[n - 2], points[n - 1], m[n - 2], m[n - 1], SEA_TAIL_T);
    Some([p[0] as f32, p[1] as f32])
}

/// trunc(65536·atan2(y, x)/2π) as the low 16 bits (0x1A74D8, 0x1A2B80).
pub fn heading(y: f32, x: f32) -> u16 {
    let turn = y.atan2(x) / 6.283_185_5_f32;
    (65536.0_f32 * turn) as i32 as u16
}

impl MapModel {
    /// Story 11000's end: the map at event 100 (location Stonehenge, every
    /// node hidden). The original enters the map mode with C2 = 100.
    pub fn first_voyage(data: MapData, save: CampaignSave) -> Result<Self> {
        if save.event != EVENT_FIRST_VOYAGE {
            return Err(format!(
                "The first voyage starts at event {EVENT_FIRST_VOYAGE}, not {}",
                save.event
            ));
        }
        let mut model = Self::sail_base(data, save)?;
        model.handler = Handler::Sail {
            event: EVENT_FIRST_VOYAGE,
            sub: 0,
        };
        Ok(model)
    }

    pub(super) fn sail_base(data: MapData, save: CampaignSave) -> Result<Self> {
        let position = data.node(save.location).position;
        let target = [f64::from(position[0]), f64::from(position[2])];
        let mut model = Self::base(data, save, target)?;
        model.fade = 0;
        model.sail_fade = true;
        model.w6e = false;
        model.hud_alpha = HUD_ALPHA_FULL;
        Ok(model)
    }

    /// Event 101's entry after story 11020 (see the module notes for the
    /// assumed HUD and letterbox).
    pub(super) fn resume_second_voyage(data: MapData, mut save: CampaignSave) -> Result<Self> {
        // C6 = 0 throughout event 101 [emu 620-y31; set during story 11020].
        save.hovered = 0;
        let mut model = Self::sail_base(data, save)?;
        model.letterbox = Letterbox {
            active: 2,
            mode: 3,
            bars: [LETTERBOX_BAR, LETTERBOX_BAR, LETTERBOX_THIRD],
        };
        model.ship_setup(0, 0);
        if let Some(path) = model.ship_path() {
            model.ship.index = model.data.path(model.ship_side(), path).dots.len();
        }
        model.place_ship();
        model.handler = Handler::Sail {
            event: EVENT_SECOND_VOYAGE,
            sub: 0,
        };
        Ok(model)
    }

    /// 0x2286F0.
    pub(super) fn ship_setup(&mut self, state: u8, kind: u8) {
        self.ship.state = state;
        self.ship.kind = kind;
        self.ship.index = 0;
        self.ship.sub = 0;
        self.ship.visible = true;
    }

    /// The ship's path for its kind (on `ship_side`).
    pub fn ship_path(&self) -> Option<u8> {
        ship_route(self.ship.kind).map(|(_, path, _)| path)
    }
    /// The side whose path table the ship's kind sails.
    pub fn ship_side(&self) -> u8 {
        ship_route(self.ship.kind).map_or(0, |(side, _, _)| side)
    }
    /// The ship's model index (slot = 34 + model): 27 or 30.
    pub fn ship_model(&self) -> i16 {
        ship_route(self.ship.kind).map_or(SHIP_MODEL, |(_, _, model)| model)
    }

    /// 0x22C180.
    pub(super) fn hud_fade(&mut self, up: bool) -> bool {
        if up {
            self.hud_alpha += HUD_STEP;
            if self.hud_alpha < HUD_ALPHA_FULL {
                return false;
            }
            self.hud_alpha = HUD_ALPHA_FULL;
        } else {
            self.hud_alpha -= HUD_STEP;
            if self.hud_alpha > 0 {
                return false;
            }
            self.hud_alpha = 0;
        }
        true
    }

    /// 0x228720: the camera pan to the ship.
    pub(super) fn pan_to_ship(&mut self) -> bool {
        if self.pan_t == 0 {
            self.pan_start = [self.camera_target[0] as f32, self.camera_target[1] as f32];
        }
        self.pan_t = (self.pan_t + CAMERA_PAN_STEP).min(CAMERA_PAN_FULL);
        let a = (CAMERA_PAN_FULL - self.pan_t) as f32;
        let b = self.pan_t as f32;
        let full = CAMERA_PAN_FULL as f32;
        let x = self.pan_start[0] * a / full + self.ship.position[0] * b / full;
        let z = self.pan_start[1] * a / full + self.ship.position[2] * b / full;
        self.camera_target = [f64::from(x), f64::from(z)];
        if self.pan_t < CAMERA_PAN_FULL {
            return false;
        }
        self.pan_t = 0;
        self.pan_start = [x, z];
        true
    }

    pub(super) fn camera_on_ship(&mut self) {
        self.camera_target = [
            f64::from(self.ship.position[0]),
            f64::from(self.ship.position[2]),
        ];
    }

    /// 0x228410(side, node, state, list): location, node state, and the
    /// listed paths whose (hidden) end nodes are reset; the Bosworth trigger.
    fn sail_setup(&mut self, node: u8, state: u8, paths: &[u8], events: &mut Vec<MapEvent>) {
        self.lerp = None;
        self.pan_t = 0;
        self.travel = 0;
        self.text = None;
        self.caption = None;
        self.figure.state = 0;
        self.save.location = node;
        self.save.node_states[usize::from(node)] = state;
        let dirs = self.data.node(node).path_dirs;
        for (k, path) in paths.iter().enumerate() {
            let end = self
                .data
                .reveal_end(*path, dirs.get(k).copied().unwrap_or(1));
            if self.save.state(end) == STATE_HIDDEN {
                self.rises[usize::from(end)] = RiseState::default();
                self.save.node_states[usize::from(end)] = STATE_HIDDEN;
                let p = &mut self.paths[usize::from(*path)];
                p.state = 0;
                p.frames = 0;
                p.alpha.iter_mut().for_each(|a| *a = 0);
            }
        }
        self.bosworth_trigger(events);
    }

    fn red_list(&self, node: u8) -> Vec<u8> {
        let n = self.data.node(node);
        (0..usize::from(n.path_count.min(4)))
            .map(|k| n.red_paths[k] as u8)
            .collect()
    }

    /// Events 100 and 101 (the dispatcher's handlers); the White Rose
    /// voyages in `white.rs`.
    pub(super) fn sail_update(&mut self, event: i16, sub: u8, pad: Pad, events: &mut Vec<MapEvent>) {
        if self.save.side != 0 {
            self.white_sail_update(event, sub, pad, events);
            return;
        }
        let mut sub = sub;
        if event == EVENT_SECOND_VOYAGE {
            self.save.node_states[usize::from(BREST_NODE)] = STATE_OPEN;
        }
        let kind = if event == EVENT_FIRST_VOYAGE { 0 } else { 1 };
        match sub {
            0 => {
                if event == EVENT_SECOND_VOYAGE {
                    self.camera_target = self.node_position(BREST_NODE);
                }
                self.letterbox.start(2);
                self.ship_setup(0, kind);
                if event == EVENT_FIRST_VOYAGE {
                    let list = self.red_list(STONEHENGE_NODE);
                    self.sail_setup(BREST_NODE, STATE_HIDDEN, &list, events);
                    self.save.location = STONEHENGE_NODE;
                } else {
                    let list = self.red_list(BREST_NODE);
                    self.sail_setup(BREST_NODE, STATE_BEATEN, &list, events);
                    self.sail_setup(MILFORD_NODE, STATE_HIDDEN, &MILFORD_PATHS, events);
                    self.save.node_states[usize::from(BREST_NODE)] = STATE_HIDDEN;
                }
                sub = 1;
            }
            1 => {
                if self.hud_fade(false) && self.pan_to_ship() && self.letterbox.active == 2 {
                    self.ship_setup(1, kind);
                    sub = 2;
                }
            }
            2 => {
                self.camera_on_ship();
                if self.ship.state == 3 {
                    sub = 3;
                    if event == EVENT_FIRST_VOYAGE {
                        events.push(MapEvent::Music {
                            track: MUSIC_ARRIVAL,
                            keep: false,
                        });
                        self.caption = Some(Caption::new(VOYAGE_CAPTION_BASE));
                        events.push(MapEvent::TextPage {
                            message: VOYAGE_CAPTION_BASE + 1,
                            page: 1,
                        });
                    }
                }
            }
            3 => {
                self.camera_on_ship();
                let node = if event == EVENT_FIRST_VOYAGE {
                    BREST_NODE
                } else {
                    MILFORD_NODE
                };
                if self.save.state(node) != STATE_HIDDEN && self.hud_fade(true) {
                    if event == EVENT_FIRST_VOYAGE {
                        self.letterbox.start(3);
                        sub = 4;
                    } else {
                        self.letterbox.start(0);
                        self.save.event = EVENT_MILFORD_REVEAL;
                        events.push(MapEvent::EventChanged(EVENT_MILFORD_REVEAL));
                        self.handler = self.reveal_handler(MILFORD_NODE);
                        return;
                    }
                }
            }
            _ => {
                if let Some(caption) = &mut self.caption {
                    if caption.phase == 4 {
                        caption.phase = 5;
                    }
                    if caption.phase == 12 && self.letterbox.active == 2 {
                        self.caption = None;
                        events.push(MapEvent::TextClosed);
                        self.start_story(EVENT_BREST_STORY, events);
                        return;
                    }
                }
            }
        }
        self.handler = Handler::Sail { event, sub };
    }

    /// The ship's samples: the path's spline samples plus the extra one.
    fn ship_samples(&self, path: u8) -> (Vec<[f32; 2]>, usize) {
        let p = self.data.path(self.ship_side(), path);
        let mut samples = p.dots.clone();
        let count = samples.len();
        if p.sea_route {
            if let Some(tail) = sea_route_tail(&p.waypoints) {
                samples.push(tail);
            }
        }
        (samples, count)
    }

    /// The ship parked at the end of its landing route (kind 1, path 0's
    /// last sample, as event 101 leaves it) for map entries after landing
    /// [H: the original's state after a map re-entry is not traced].
    pub(super) fn park_ship(&mut self) {
        if self.save.side != 0 {
            self.park_white_ship();
            return;
        }
        self.ship_setup(0, 1);
        if let Some(path) = self.ship_path() {
            self.ship.index = self.data.path(0, path).dots.len();
        }
        self.place_ship();
    }

    /// 0x22CA00's position and heading for the current sample.
    pub(super) fn place_ship(&mut self) -> Option<(u8, usize)> {
        let path = self.ship_path()?;
        let (x, count) = self.ship_samples(path);
        let at = |k: usize| x.get(k).copied().unwrap_or([0.0, 0.0]);
        let i = self.ship.index;
        let t = self.ship.sub;
        let (p0, p1, p2) = (at(i), at(i + 1), at(i + 2));
        let d1 = [p1[0] - p0[0], p1[1] - p0[1]];
        let d2 = [p2[0] - p1[0], p2[1] - p1[1]];
        let tf = t as f32;
        let y = self.data.node(0).position[1];
        self.ship.position = [p0[0] + d1[0] * tf / 1024.0, y, p0[1] + d1[1] * tf / 1024.0];
        self.ship.heading = if i >= count {
            let prev = at(i.saturating_sub(1));
            heading(p0[0] - prev[0], p0[1] - prev[1])
        } else if i + 1 >= count {
            heading(d1[0], d1[1])
        } else {
            let a = (SHIP_SAMPLE - t) as f32;
            heading(
                d1[0] * a / 1024.0 + d2[0] * tf / 1024.0,
                d1[1] * a / 1024.0 + d2[1] * tf / 1024.0,
            )
        };
        Some((path, count))
    }

    /// 0x22CA00 (draw pass): place, then advance.
    pub(super) fn ship_update(&mut self, events: &mut Vec<MapEvent>) {
        if !self.ship.visible {
            return;
        }
        let Some((path, count)) = self.place_ship() else {
            self.note("a ship kind beyond 0..=4 has no route");
            return;
        };
        if self.ship.state > 0 && self.ship.sub < SHIP_SAMPLE && self.ship.index < count {
            if self.ship.state == 1 {
                events.push(MapEvent::Sfx(SFX_SHIP));
                self.ship.state = 2;
            }
            if self.ship.state == 2 {
                self.ship.sub += SHIP_STEP;
                if self.ship.sub >= SHIP_SAMPLE {
                    self.ship.index += 1;
                    self.ship.sub = 0;
                    if self.ship.index >= count {
                        self.ship.state = 3;
                        self.ship.index = count;
                        // W+52EC[path +0x46]: the end node's rise [F 0x22CE48].
                        let end = self.data.path(self.ship_side(), path).ends[1] as u8;
                        if usize::from(end) < NODE_COUNT && !self.rises[usize::from(end)].rising {
                            self.rises[usize::from(end)] = RiseState {
                                rising: true,
                                level: 0,
                            };
                            events.push(MapEvent::Sfx(SFX_RISE));
                            events.push(MapEvent::NodeRising(end));
                        }
                    }
                }
            }
        }
    }

    /// The letterbox and caption of the draw pass.
    pub(super) fn sail_draw_update(&mut self) {
        if self.letterbox.active == 1 && self.fade == 0 && !matches!(self.handler, Handler::MapStory { .. }) {
            self.letterbox.step();
        }
        if let Some(caption) = &mut self.caption {
            caption.step();
        }
    }

    /// W+6C during the voyage (update pass, from the previous draw's W+6E).
    pub(super) fn sail_fade_update(&mut self) {
        if self.w6e {
            self.fade = (self.fade + FADE_STEP).min(FADE_FULL);
        } else {
            self.fade = (self.fade - FADE_STEP).max(0);
        }
    }

    /// W+6E after the draw pass: clear during 100/101 and camera pans.
    pub(super) fn sail_w6e(&mut self) {
        // W+5372 (camera busy) is set from the reveal's first frame on.
        self.w6e = match &self.handler {
            // A White Rose voyage the close logic just set: its sub-step 0
            // (W+5372 = 1) runs next frame, so this draw still sets W+6E
            // [emu mako-win: C2 = 105 f087, W+6C 0 at f101].
            Handler::Sail { sub: 0, .. } if self.save.side != 0 => true,
            Handler::Sail { .. } => false,
            // Mako's pages (12041 after the voyage): the HUD stays out
            // [emu trace-dover: the HUD drawn 320 rows down f190..f473].
            Handler::MapStory { event, .. } if Self::is_dialogue_event(*event) => false,
            Handler::Reveal { phase, .. } => *phase == 0 && !self.w5372,
            _ => self.lerp.is_none(),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letterbox_modes_follow_the_counters_of_0x233750() {
        // Mode 2 from closed: the first bar, then the second; the third closes.
        let mut lb = Letterbox::default();
        lb.start(2);
        let mut frames = 0;
        while lb.active == 1 {
            lb.step();
            frames += 1;
        }
        assert_eq!((lb.bars, frames), ([72, 72, 0], 18));
        // Mode 3 from mode 2's result: the third bar opens to 0x70 in 14.
        lb.start(3);
        let mut frames = 0;
        while lb.active == 1 {
            lb.step();
            frames += 1;
        }
        assert_eq!((lb.bars, lb.height(), frames), ([72, 72, 112], 256, 14));
        // Mode 2 from mode 3's result (event 101's start): the third closes.
        lb.start(2);
        let mut frames = 0;
        while lb.active == 1 {
            lb.step();
            frames += 1;
        }
        assert_eq!((lb.bars, frames), ([72, 72, 0], 14));
        // Mode 0 closes everything (event 102): nine frames from 72.
        lb.start(0);
        let mut frames = 0;
        while lb.active == 1 {
            lb.step();
            frames += 1;
        }
        assert_eq!((lb.bars, frames), ([0, 0, 0], 9));
    }

    #[test]
    fn caption_alphas_stagger_in_and_out() {
        let mut c = Caption::new(VOYAGE_CAPTION_BASE);
        let mut seen = Vec::new();
        for _ in 0..6 {
            c.step();
            seen.push((c.phase, c.alpha));
        }
        assert_eq!(
            seen,
            [
                (2, [48, 0, 0, 0]),
                (2, [96, 48, 48, 0]),
                (2, [128, 96, 96, 48]),
                (2, [128, 128, 128, 96]),
                (3, [128, 128, 128, 128]),
                (4, [128, 128, 128, 128]),
            ]
        );
        c.phase = 5;
        let mut seen = Vec::new();
        for _ in 0..6 {
            c.step();
            seen.push((c.phase, c.alpha));
        }
        assert_eq!(
            seen,
            [
                (5, [80, 128, 128, 128]),
                (5, [32, 80, 80, 128]),
                (5, [0, 32, 32, 80]),
                (5, [0, 0, 0, 32]),
                (11, [0, 0, 0, 0]),
                (12, [0, 0, 0, 0]),
            ]
        );
    }

    #[test]
    fn heading_truncates_to_sixteen_bits() {
        assert_eq!(heading(0.0, 1.0), 0);
        assert_eq!(heading(1.0, 0.0), 16384);
        assert_eq!(heading(-1.0, 0.0) as i16, -16384);
        assert_eq!(heading(0.0, -1.0), 32768);
    }
}

#[cfg(test)]
mod private_tests {
    use super::*;
    use serde_json::Value;

    fn data() -> MapData {
        let path = std::env::var_os("ROSES_CAMPAIGN_MAP_DATA").expect("ROSES_CAMPAIGN_MAP_DATA");
        load_map_data(std::path::Path::new(&path)).unwrap()
    }

    /// `sail/event100-ship.json` or `event101-ship.json` (run_sail_ship.py).
    fn log(name: &str) -> Value {
        let dir = std::env::var_os("ROSES_SAIL_LOGS").expect("ROSES_SAIL_LOGS");
        let bytes = std::fs::read(std::path::Path::new(&dir).join(name)).unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn f(v: &Value) -> f64 {
        v.as_f64().unwrap()
    }
    fn i(v: &Value) -> i64 {
        v.as_i64().unwrap()
    }
    fn list(v: &Value) -> Vec<i64> {
        v.as_array().unwrap().iter().map(i).collect()
    }

    /// Compare one model frame with one log row; `letterbox` false skips
    /// the bars (the unknown entry state of 101).
    fn compare(m: &MapModel, row: &Value, letterbox: bool) -> Vec<String> {
        let mut out = Vec::new();
        let fr = i(&row["f"]);
        let mut check = |what: &str, ok: bool, detail: String| {
            if !ok {
                out.push(format!("row f{fr} {what}: {detail}"));
            }
        };
        let s = &row["ship"];
        check(
            "C2",
            i64::from(m.save.event) == i(&row["C2"]),
            format!("{} vs {}", m.save.event, row["C2"]),
        );
        check(
            "ship state/index/sub",
            (
                i64::from(m.ship.state),
                m.ship.index as i64,
                i64::from(m.ship.sub),
            ) == (i(&s["state"]), i(&s["i"]), i(&s["sub"])),
            format!("{:?} vs {s}", (m.ship.state, m.ship.index, m.ship.sub)),
        );
        let pos = &s["pos"];
        let dp = (0..3)
            .map(|k| (f64::from(m.ship.position[k]) - f(&pos[k])).abs())
            .fold(0.0, f64::max);
        check(
            "ship position",
            dp < 2e-3,
            format!("{:?} vs {pos}", m.ship.position),
        );
        let dh = (i64::from(m.ship.heading as i16) - i(&s["head"])).abs();
        check(
            "ship heading",
            dh <= 2,
            format!("{} vs {}", m.ship.heading as i16, s["head"]),
        );
        let tgt = &row["tgt"];
        let dt = (m.camera_target[0] - f(&tgt[0]))
            .abs()
            .max((m.camera_target[1] - f(&tgt[1])).abs());
        check(
            "camera",
            dt < 2e-3,
            format!("{:?} vs {tgt}", m.camera_target),
        );
        check(
            "hud",
            i64::from(m.hud_alpha) == i(&row["hud"]),
            format!("{} vs {}", m.hud_alpha, row["hud"]),
        );
        // After C2 = 0 the original's W+6C needs W+5485/W+5376 (unset in
        // this run) while the model's free map uses its fitted fade-in.
        check(
            "fade",
            i(&row["C2"]) == 0 || i64::from(m.fade) == i(&row["fade"]),
            format!("{} vs {}", m.fade, row["fade"]),
        );
        let nodes: Vec<i64> = m.save.node_states[..12]
            .iter()
            .map(|v| i64::from(*v))
            .collect();
        let want = list(&row["nodes"]);
        check("nodes", nodes == want, format!("{nodes:?} vs {want:?}"));
        let rise: Vec<i64> = m.rises[..12].iter().map(|r| i64::from(r.level)).collect();
        let want = list(&row["rise"]);
        check("rise", rise == want, format!("{rise:?} vs {want:?}"));
        let paths: Vec<i64> = m.paths.iter().map(|p| i64::from(p.state)).collect();
        let want = list(&row["paths"]);
        check("paths", paths == want, format!("{paths:?} vs {want:?}"));
        if letterbox {
            let mine = vec![
                i64::from(m.letterbox.active),
                i64::from(m.letterbox.mode),
                i64::from(m.letterbox.bars[0]),
                i64::from(m.letterbox.bars[1]),
                i64::from(m.letterbox.bars[2]),
            ];
            let want = list(&row["lb"]);
            check("letterbox", mine == want, format!("{mine:?} vs {want:?}"));
        }
        if i(&row["C2"]) == 100 {
            let (phase, alpha) = m
                .caption
                .as_ref()
                .map_or((0, [0; 4]), |c| (c.phase, c.alpha));
            check(
                "caption phase",
                i64::from(phase) == i(&row["disp"]),
                format!("{phase} vs {}", row["disp"]),
            );
            let mine: Vec<i64> = alpha.iter().map(|a| i64::from(*a)).collect();
            let want = list(&row["alpha"]);
            check(
                "caption alpha",
                mine == want,
                format!("{mine:?} vs {want:?}"),
            );
        }
        out
    }

    #[test]
    #[ignore = "Requires the private map import and sail logs (ROSES_CAMPAIGN_MAP_DATA, ROSES_SAIL_LOGS)"]
    fn private_first_voyage_matches_event100_frame_by_frame() {
        let log = log("event100-ship.json");
        let rows = log["frames"].as_array().unwrap();
        let mut m = MapModel::first_voyage(data(), CampaignSave::before_landing()).unwrap();
        // The savestate starts inside sub-step 1; align on the sailing start
        // (row f9: ship state 2, sub-step 0x100).
        let mut steps = 0;
        while m.ship.state != 2 {
            m.step(Pad::NONE);
            steps += 1;
            assert!(steps < 200, "the ship never set sail");
        }
        assert_eq!((m.pan_t, m.camera_target), (0, [-31.0, 126.0]));
        // HUD fade-out (32 updates) and pan (16) from the map entry.
        assert_eq!(steps, 48);
        let start = rows.iter().position(|r| i(&r["f"]) == 9).unwrap();
        let mut errors = Vec::new();
        let mut story = None;
        for row in &rows[start..] {
            if i(&row["f"]) > 9 {
                // The log's driver presses Cross whenever the text phase is
                // 4; the caption has to ignore it.
                let pad = if i(&row["pad"]) != 0 {
                    Pad::CROSS
                } else {
                    Pad::NONE
                };
                for e in m.step(pad) {
                    if let MapEvent::StoryEvent(s) = e {
                        story = Some((i(&row["f"]), s));
                    }
                }
            }
            errors.extend(compare(&m, row, true));
        }
        assert_eq!(story, Some((226, EVENT_BREST_STORY)));
        // Pad input changes nothing during the voyage: Cross on every
        // update, held directions, or nothing at all.
        for pad in [Pad::NONE, Pad::CROSS, Pad::UP] {
            let mut m = MapModel::first_voyage(data(), CampaignSave::before_landing()).unwrap();
            let mut at = None;
            for update in 1..400u32 {
                if m.step(pad)
                    .contains(&MapEvent::StoryEvent(EVENT_BREST_STORY))
                {
                    at = Some(update);
                    break;
                }
            }
            assert_eq!(at, Some(265), "{pad:?}");
            assert_eq!(m.cursor, CURSOR_START);
        }
        assert!(
            errors.is_empty(),
            "{} differences:\n{}",
            errors.len(),
            errors.join("\n")
        );
    }

    #[test]
    #[ignore = "Requires the private map import and sail logs (ROSES_CAMPAIGN_MAP_DATA, ROSES_SAIL_LOGS)"]
    fn private_second_voyage_and_reveal_match_events_101_102() {
        let log = log("event101-ship.json");
        let rows = log["frames"].as_array().unwrap();
        let mut save = CampaignSave::before_landing();
        save.event = EVENT_SECOND_VOYAGE;
        save.node_states[usize::from(BREST_NODE)] = STATE_OPEN;
        // C6 = 0 throughout the 620-y31 run (left so by story 11020).
        save.hovered = 0;
        let mut m = MapModel::resume(data(), save).unwrap();
        let mut errors = Vec::new();
        for row in rows {
            m.step(Pad::NONE);
            // The entry letterbox (after story 11020) is not traced: the
            // original is at 96 on the first frame, the model one step behind.
            let letterbox = i(&row["f"]) > 14;
            errors.extend(compare(&m, row, letterbox));
        }
        assert!(
            errors.is_empty(),
            "{} differences:\n{}",
            errors.len(),
            errors.join("\n")
        );
        let mut landed = CampaignSave::after_landing();
        landed.hovered = 0;
        assert_eq!(m.save, landed);
    }
}
