//! The White Rose (Yorkist) side of the campaign map (SAVE[0] = 1). Source
//! facts (PAL SLES_515.79, private research
//! `story/claude-campaign-map/WHITE-ROUTE-AUDIT.md`, tags [F] code/data,
//! [emu] the game's code run frame by frame by `tools/run_white.py`, logs
//! `logs/white-*.txt`, [H] hypothesis):
//!
//! - The route is a chain: Stonehenge 11 → Windsor 12 → London 13 →
//!   Canterbury 14 → Dover 15 ⇒ (ship) Strait of Dover 16 ⇒ (ship)
//!   Boulogne 17 → Amiens 18 → Paris 19 → Le Mans 20 → Rennes 21 → Brest 0
//!   ⇒ (ship) Stonehenge 11; every node has one white path (+0x1C) [F].
//! - Dispatcher white branch 0x2270F0..0x2272A0 [F]: 100..=103 and
//!   106..=110 are the generic reveal 0x228210(1, node, [white path], 1) of
//!   nodes 11, 12, 13, 14, 17, 18, 19, 20, 21; 100 also clears Stonehenge's
//!   state every frame (delay slot 0x2272F8); 106 plays (4, 1) once. 104,
//!   105 and 111 are the voyages (below). 2xx/3xx are the return handler.
//! - 0x228210 for side 1: sub-step 0 letterbox mode 0 and 0x228410(1, node,
//!   3, list, 1); sub-step 1 marks only the list's first path (state 1)
//!   once the letterbox is done; the centroid pan reads the paths' forward
//!   ends (+0x46) [F 0x228210..0x2283CC, 0x228B18]. The pans and the dot
//!   timing are the red ones; the letterbox completes only while W+6C = 0,
//!   which the voyage's W+6C rule (`sail_fade`) lowers by 20 per frame
//!   [emu: windsor-win 101 f203 → mark f218 → C2 = 0 f252; mako-win 106
//!   f315 → mark f324 → C2 = 0 f364; start-12000 100 → mark f708 → C2 = 0
//!   f743, Windsor 1 f807].
//! - Dover: Cross only while the Strait (SAVE[3+16]) is hidden, no DC
//!   check, no text (0x239BDC..); the travel's lerp end gives C2 = 12040
//!   instead of the duel (0x2291F4..0x229210) [F]. Story 12040 (Mako's page
//!   3140) continues at 104.
//! - 104 (0x227398) [F, emu dover-12040]: SAVE[3+15] = 3 every frame; 0
//!   letterbox mode 2, ship (0, kind 2), 0x228410(1, 15, 3, [p4], 1); 1 HUD
//!   fade-out, pan to the ship, letterbox → ship (1, 2); 2 camera = ship,
//!   arrival → C2 = 12041, C7 = 16. Story 12041 (Mako's pages) ends with
//!   the tail played here: node 16's rise, HUD fade-in, letterbox mode 0,
//!   C2 = 0 and (4, 1) [emu: rise f369, Strait 1 f432, letterbox f464,
//!   C2 = 0 f473].
//! - 105 (0x227594) [F, emu mako-win]: SAVE[3+16] = 3 every frame; 0 W+1C =
//!   0, mode 2, ship (0, 3), 0x228410(1, 16, 3, [p5, p6], 2) and (1, 17, 0,
//!   ..), music (35, 1), C7 = 16; 1 W+6C = 0 → W+1C = 1; 2 HUD, pan,
//!   letterbox → ship (1, 3); 3 camera = ship, arrival → C7 = 17 and the
//!   caption 3180 (style 1, one Cross); 4 Boulogne's state ≠ 0 and HUD
//!   fade-in → letterbox mode 0, path 5 state 1, C2 = 106.
//! - 111 (0x227978, table 0x2D23C0) [F, emu brest-win]: 0 W+1C = 0, mode 2,
//!   ship (0, 4), 0x228410(1, 0, 3, [p11], 1), SAVE[3] = 3; 1 W+6C = 0 →
//!   W+1C = 1; 2 letterbox done → Seto's window 3300 (style 2); 3 the
//!   window closed; 4 HUD, pan → ship (1, 4); 5 arrival → C7 = 11; 6
//!   Stonehenge's state ≠ 0 and HUD fade-in → letterbox mode 0; 7 a
//!   17-frame pan to node 11 (W+60 += 16 to > 256); 8 letterbox mode 3; 9
//!   letterbox done → C2 = 12130.
//! - Close logic: 3xx → state 3, C2 −= 200 (the handler runs from the next
//!   frame); 312 → state 3 and 12150 [F 0x2311BC..0x2311E4; emu
//!   manawyddan-win]. Rose cards for nodes 0, 12, 13, 14, 18..=21 [F
//!   0x2293D4..0x22949C].
//! - Re-entry (0x21C568..0x21C5D0) [F]: path 0 drawn always, path k + 1 for
//!   node 12 + k at state 3, path 11 for Brest at state 3.
//!
//! [H] The story's last frame also runs the map event's sub-step 0 in the
//! original (100 at f694, 104 at f070); the model runs it on its first
//! update, so its update n is the log's frame (entry + n). The ship parked
//! at a re-entry (`park_white_ship`) is not traced.
use super::*;

pub const WHITE_DOVER_NODE: u8 = 15;
pub const WHITE_STRAIT_NODE: u8 = 16;
pub const WHITE_BOULOGNE_NODE: u8 = 17;
/// Rose-card nodes of the White Rose side (0x2293D4..0x22949C, HUD count
/// 0x2345EC..) [F].
pub const WHITE_ROSE_NODES: [u8; 8] = [0, 12, 13, 14, 18, 19, 20, 21];
pub const EVENT_DOVER_STORY: i16 = 12040;
pub const EVENT_DOVER_VOYAGE: i16 = 104;
pub const EVENT_STRAIT_STORY: i16 = 12041;
pub const EVENT_STRAIT_VOYAGE: i16 = 105;
pub const EVENT_BOULOGNE_REVEAL: i16 = 106;
pub const EVENT_STONEHENGE_VOYAGE: i16 = 111;
pub const EVENT_WHITE_FINAL: i16 = 12130;
pub const EVENT_WHITE_WIN: i16 = 12150;
/// The white story events the map hands to the story scene (12040 and
/// 12041 are played by the map itself: `story_window::StoryDialogue`).
pub const WHITE_STORY_EVENTS: [i16; 2] = [EVENT_WHITE_FINAL, EVENT_WHITE_WIN];
pub const WHITE_TEXTS_MISSING: &str = "the White Rose node texts are not in this campaign-map import (run roses import-campaign-map again)";
const KIND_DOVER: u8 = 2;
const KIND_STRAIT: u8 = 3;
const KIND_HOME: u8 = 4;
const MUSIC_MAKO: u16 = 35;
/// 111 sub-step 7: W+60 += 16 until above 256 [F 0x227CC0..0x227CDC].
const HOME_PAN_STEP: i32 = 0x10;
const HOME_PAN_FULL: i32 = 0x100;

/// The map event a White Rose story event's pages continue at: 12040 →
/// 104 (0x221BE4); 12041 → its own tail, played by the map (node 16's
/// rise, then C2 = 0). The map now plays both pages itself; this stays for
/// the story stages `map-story-12040/12041` started directly. 12130 → 12132 → 0 and 12150 → … → the ending are
/// the story scene's.
pub fn white_story_continuation(event: i16) -> Option<i16> {
    match event {
        EVENT_DOVER_STORY => Some(EVENT_DOVER_VOYAGE),
        EVENT_STRAIT_STORY => Some(EVENT_STRAIT_STORY),
        _ => None,
    }
}

/// The node a white reveal event reveals (0x2272C8..0x227970) [F].
pub fn white_reveal_node(event: i16) -> Option<u8> {
    match event {
        100 => Some(STONEHENGE_NODE),
        101 => Some(12),
        102 => Some(13),
        103 => Some(14),
        106 => Some(WHITE_BOULOGNE_NODE),
        107 => Some(18),
        108 => Some(19),
        109 => Some(20),
        110 => Some(21),
        _ => None,
    }
}

impl CampaignSave {
    /// Paths drawn at a White Rose re-entry (0x21C568..0x21C5D0) [F].
    pub(super) fn white_drawn_paths(&self) -> Vec<u8> {
        let mut out = vec![0];
        for k in 0..10u8 {
            if self.state(12 + k) == STATE_BEATEN {
                out.push(k + 1);
            }
        }
        if self.state(BREST_NODE) == STATE_BEATEN {
            out.push(11);
        }
        out
    }
}

impl MapModel {
    /// Re-enter the White Rose map at the save's event.
    pub(super) fn resume_white(data: MapData, save: CampaignSave) -> Result<Self> {
        if !save.map_entered {
            return Err(
                "a save after the ending (SAVE+0x3C = 0) continues with the other route's story (ending::continue_target), not the map"
                    .into(),
            );
        }
        let event = save.event;
        let position = data.node(save.location).position;
        let target = [f64::from(position[0]), f64::from(position[2])];
        let mut model = Self::base(data, save, target)?;
        model.handler = match event {
            0 => {
                model.fade = FADE_FULL;
                model.hud.shown = true;
                model.hud.hidden = false;
                model.hud.slide = HUD_SLIDE_FULL;
                Handler::Free
            }
            200..=399 => Handler::Return {
                node: model.save.location,
                won: event >= 300,
                phase: 0,
            },
            e if white_reveal_node(e).is_some() => {
                // The entry after story 12000 (or a save inside a reveal):
                // W+6C full, falling from the first update [emu
                // start-12000: letterbox done f707 = update 13].
                model.sail_fade = true;
                model.fade = FADE_FULL;
                model.w6e = false;
                model.reveal_handler(white_reveal_node(e).unwrap_or(STONEHENGE_NODE))
            }
            EVENT_DOVER_VOYAGE => {
                model.dover_voyage_entry();
                Handler::Sail { event, sub: 0 }
            }
            EVENT_STRAIT_STORY => {
                model.strait_tail_entry();
                Handler::Sail { event, sub: 3 }
            }
            // A save inside Mako's Dover page restarts it [H].
            EVENT_DOVER_STORY => model.resume_map_story(event),
            EVENT_STRAIT_VOYAGE | EVENT_STONEHENGE_VOYAGE => {
                // A save made inside the voyage restarts it [H].
                model.sail_fade = true;
                model.fade = FADE_FULL;
                model.w6e = true;
                Handler::Sail { event, sub: 0 }
            }
            e if WHITE_STORY_EVENTS.contains(&e) => {
                model.story_request = Some(e);
                Handler::Story { event: e }
            }
            other => {
                return Err(format!(
                    "Campaign map event {other} of the White Rose route is not traced"
                ))
            }
        };
        Ok(model)
    }

    /// 104 after 12040's page (played over the map, or resumed): camera on
    /// Dover (the travel's lerp end), letterbox mode 2 done, W+6C 0 [emu
    /// dover-12040 f018..f069; H: W+6C].
    pub(super) fn dover_voyage_entry(&mut self) {
        self.sail_fade = true;
        self.fade = 0;
        self.w6e = false;
        self.camera_target = self.node_position(WHITE_DOVER_NODE);
        self.letterbox = Letterbox {
            active: 2,
            mode: 2,
            bars: [sail::LETTERBOX_BAR, sail::LETTERBOX_BAR, 0],
        };
    }

    /// The tail of 12041 after its pages: the ship arrived at the Strait,
    /// the HUD faded out, camera on the ship, letterbox bars 72/72/0 [emu
    /// dover-12040 f190..f368]; node 16's rise flag W+530C (0x221CFC) [F].
    pub(super) fn strait_tail_entry(&mut self) {
        self.sail_fade = true;
        self.fade = 0;
        self.w6e = false;
        self.hud_alpha = 0;
        self.save.location = WHITE_STRAIT_NODE;
        self.ship.state = 3;
        self.camera_on_ship();
        self.letterbox = Letterbox {
            active: 0,
            mode: 2,
            bars: [sail::LETTERBOX_BAR, sail::LETTERBOX_BAR, 0],
        };
        self.rises[usize::from(WHITE_STRAIT_NODE)] = RiseState {
            rising: true,
            level: 0,
        };
    }

    /// 0x228410 for side 1: location, node state, and the listed paths
    /// whose (hidden) forward ends are reset.
    fn white_setup(&mut self, node: u8, state: u8, paths: &[u8]) {
        self.travel = 0;
        self.text = None;
        self.caption = None;
        self.figure.state = 0;
        self.save.location = node;
        self.save.node_states[usize::from(node)] = state;
        for &path in paths {
            let end = self.data.path(1, path).ends[1] as u8;
            if self.save.state(end) == STATE_HIDDEN {
                self.rises[usize::from(end)] = RiseState::default();
                let p = &mut self.paths[usize::from(path)];
                p.state = 0;
                p.frames = 0;
                p.alpha.iter_mut().for_each(|a| *a = 0);
            }
        }
    }

    /// 0x228210 sub-step 0 on side 1: letterbox mode 0, 0x228410.
    pub(super) fn white_reveal_setup(&mut self, node: u8, paths: &[(u8, u16)]) {
        self.letterbox.start(0);
        self.w5372 = false;
        let list: Vec<u8> = paths.iter().map(|(p, _)| *p).collect();
        self.white_setup(node, STATE_BEATEN, &list);
    }

    /// The node's white path (+0x1C).
    fn white_path(&self, node: u8) -> u8 {
        self.data.node(node).white_paths[0] as u8
    }

    /// The close logic's 3xx of the White Rose side (won, node unbeaten).
    pub(super) fn white_close(&mut self, node: u8, event: i16, events: &mut Vec<MapEvent>) {
        self.save.node_states[usize::from(node)] = STATE_BEATEN;
        events.push(MapEvent::NodeBeaten(node));
        if event == 312 {
            self.start_story(EVENT_WHITE_WIN, events);
            return;
        }
        let next = event - 200;
        self.save.event = next;
        events.push(MapEvent::EventChanged(next));
        // W+6C follows the voyage rule from here; the event's handler runs
        // from the next frame [emu: C2 = 101 f203, sub-step 0 f204].
        self.sail_fade = true;
        self.w6e = true;
        self.handler = match next {
            EVENT_STRAIT_VOYAGE | EVENT_STONEHENGE_VOYAGE => Handler::Sail { event: next, sub: 0 },
            e => match white_reveal_node(e) {
                Some(n) => self.reveal_handler(n),
                None => {
                    self.note("a White Rose close event without a traced handler (C2 = 0 instead)");
                    self.save.event = 0;
                    Handler::Free
                }
            },
        };
    }

    /// The White Rose voyages (104, 105, 111) and 12041's tail.
    pub(super) fn white_sail_update(
        &mut self,
        event: i16,
        sub: u8,
        pad: Pad,
        events: &mut Vec<MapEvent>,
    ) {
        let mut sub = sub;
        match event {
            EVENT_DOVER_VOYAGE => {
                self.save.node_states[usize::from(WHITE_DOVER_NODE)] = STATE_BEATEN;
                match sub {
                    0 => {
                        self.letterbox.start(2);
                        self.pan_t = 0;
                        self.w5372 = true;
                        self.ship_setup(0, KIND_DOVER);
                        let list = [self.white_path(WHITE_DOVER_NODE)];
                        self.white_setup(WHITE_DOVER_NODE, STATE_BEATEN, &list);
                        sub = 1;
                    }
                    1 => {
                        if self.hud_fade(false) && self.pan_to_ship() && self.letterbox.active == 2 {
                            self.ship_setup(1, KIND_DOVER);
                            sub = 2;
                        }
                    }
                    _ => {
                        self.camera_on_ship();
                        if self.ship.state == 3 {
                            self.save.location = WHITE_STRAIT_NODE;
                            // Mako's pages over the map (`story_dialogue`).
                            self.start_map_story(EVENT_STRAIT_STORY, events);
                            return;
                        }
                    }
                }
            }
            EVENT_STRAIT_STORY => match sub {
                3 => {
                    if self.save.state(WHITE_STRAIT_NODE) != STATE_HIDDEN && self.hud_fade(true) {
                        self.letterbox.start(0);
                        sub = 4;
                    }
                }
                _ => {
                    if self.letterbox.active == 2 {
                        self.pan_t = 0;
                        self.w5372 = false;
                        self.travel = 0;
                        self.save.event = 0;
                        events.push(MapEvent::EventChanged(0));
                        events.push(MapEvent::Music {
                            track: MUSIC_MAP,
                            keep: true,
                        });
                        self.handler = Handler::Free;
                        return;
                    }
                }
            },
            EVENT_STRAIT_VOYAGE => {
                self.save.node_states[usize::from(WHITE_STRAIT_NODE)] = STATE_BEATEN;
                match sub {
                    0 => {
                        self.letterbox.active = 0;
                        self.letterbox.mode = 2;
                        self.pan_t = 0;
                        self.w5372 = true;
                        self.ship_setup(0, KIND_STRAIT);
                        let list = [
                            self.white_path(WHITE_STRAIT_NODE),
                            self.white_path(WHITE_BOULOGNE_NODE),
                        ];
                        self.white_setup(WHITE_STRAIT_NODE, STATE_BEATEN, &list);
                        self.white_setup(WHITE_BOULOGNE_NODE, STATE_HIDDEN, &list);
                        events.push(MapEvent::Music {
                            track: MUSIC_MAKO,
                            keep: true,
                        });
                        self.save.location = WHITE_STRAIT_NODE;
                        sub = 1;
                    }
                    1 => {
                        if self.fade == 0 {
                            self.letterbox.active = 1;
                            sub = 2;
                        }
                    }
                    2 => {
                        if self.hud_fade(false) && self.pan_to_ship() && self.letterbox.active == 2 {
                            self.ship_setup(1, KIND_STRAIT);
                            sub = 3;
                        }
                    }
                    3 => {
                        self.camera_on_ship();
                        if self.ship.state == 3 {
                            sub = 4;
                            self.save.location = WHITE_BOULOGNE_NODE;
                            match self.data.white_caption(EVENT_STRAIT_VOYAGE) {
                                Some(c) => {
                                    let base = c.base;
                                    self.caption = Some(Caption::new(base));
                                    events.push(MapEvent::TextPage {
                                        message: base + 1,
                                        page: 1,
                                    });
                                }
                                None => self.note(WHITE_TEXTS_MISSING),
                            }
                        }
                    }
                    _ => {
                        self.camera_on_ship();
                        // The caption waits for Cross [emu mako-win f225
                        // phase 4, Cross f226 → 5].
                        if let Some(caption) = &mut self.caption {
                            if caption.phase == 4 && pad.cross {
                                caption.phase = 5;
                                events.push(MapEvent::Sfx(SFX_PAGE));
                            }
                            if caption.phase == 12 {
                                self.caption = None;
                                events.push(MapEvent::TextClosed);
                            }
                        }
                        let boulogne = self.data.path(1, self.white_path(WHITE_STRAIT_NODE)).ends[1] as u8;
                        if self.save.state(boulogne) != STATE_HIDDEN && self.hud_fade(true) {
                            self.letterbox.start(0);
                            let p5 = usize::from(self.white_path(WHITE_STRAIT_NODE));
                            self.paths[p5].state = 1;
                            self.paths[p5].frames = 0;
                            self.save.event = EVENT_BOULOGNE_REVEAL;
                            events.push(MapEvent::EventChanged(EVENT_BOULOGNE_REVEAL));
                            self.handler = self.reveal_handler(WHITE_BOULOGNE_NODE);
                            return;
                        }
                    }
                }
            }
            EVENT_STONEHENGE_VOYAGE => match sub {
                0 => {
                    self.letterbox.active = 0;
                    self.letterbox.mode = 2;
                    self.pan_t = 0;
                    self.w5372 = true;
                    self.ship_setup(0, KIND_HOME);
                    let list = [self.white_path(BREST_NODE)];
                    self.white_setup(BREST_NODE, STATE_BEATEN, &list);
                    self.save.node_states[usize::from(BREST_NODE)] = STATE_BEATEN;
                    sub = 1;
                }
                1 => {
                    if self.fade == 0 {
                        self.letterbox.active = 1;
                        sub = 2;
                    }
                }
                2 => {
                    if self.letterbox.active == 2 {
                        match self.data.white_caption(EVENT_STONEHENGE_VOYAGE).cloned() {
                            Some(c) => {
                                self.start_text_styled(c.base, c.groups, c.style, c.speaker, events)
                            }
                            None => self.note(WHITE_TEXTS_MISSING),
                        }
                        sub = 3;
                    }
                }
                3 => {
                    if self.text.is_none() {
                        sub = 4;
                    }
                }
                4 => {
                    if self.hud_fade(false) && self.pan_to_ship() {
                        self.ship_setup(1, KIND_HOME);
                        sub = 5;
                    }
                }
                5 => {
                    self.camera_on_ship();
                    if self.ship.state == 3 {
                        self.save.location = STONEHENGE_NODE;
                        sub = 6;
                    }
                }
                6 => {
                    self.camera_on_ship();
                    if self.save.state(STONEHENGE_NODE) != STATE_HIDDEN && self.hud_fade(true) {
                        self.letterbox.start(0);
                        sub = 7;
                    }
                }
                7 => {
                    if self.pan_t == 0 {
                        self.pan_start = [self.camera_target[0] as f32, self.camera_target[1] as f32];
                    }
                    let end = self.data.node(STONEHENGE_NODE).position;
                    let t = self.pan_t as f32;
                    let full = HOME_PAN_FULL as f32;
                    let x = self.pan_start[0] * (full - t) / full + end[0] * t / full;
                    let z = self.pan_start[1] * (full - t) / full + end[2] * t / full;
                    self.camera_target = [f64::from(x), f64::from(z)];
                    self.pan_t += HOME_PAN_STEP;
                    if self.pan_t > HOME_PAN_FULL {
                        self.pan_t = 0;
                        self.w5372 = false;
                        sub = 8;
                    }
                }
                8 => {
                    self.letterbox.start(3);
                    sub = 9;
                }
                _ => {
                    if self.letterbox.active == 2 {
                        self.start_story(EVENT_WHITE_FINAL, events);
                        return;
                    }
                }
            },
            other => {
                self.note("a White Rose voyage event without a traced handler");
                let _ = other;
                self.handler = Handler::Free;
                return;
            }
        }
        self.handler = Handler::Sail { event, sub };
    }

    /// The ship at a White Rose re-entry [H]: after Brest's voyage at
    /// Stonehenge (kind 4), after the Strait's at Boulogne (kind 3), after
    /// Dover's at the Strait (kind 2), else not shown.
    pub(super) fn park_white_ship(&mut self) {
        let kind = if self.save.state(BREST_NODE) == STATE_BEATEN {
            Some(KIND_HOME)
        } else if self.save.state(WHITE_STRAIT_NODE) == STATE_BEATEN {
            Some(KIND_STRAIT)
        } else if self.save.state(WHITE_DOVER_NODE) == STATE_BEATEN {
            Some(KIND_DOVER)
        } else {
            None
        };
        match kind {
            Some(kind) => {
                self.ship_setup(0, kind);
                if let Some(path) = self.ship_path() {
                    self.ship.index = self.data.path(1, path).dots.len();
                }
                self.place_ship();
            }
            None => {
                self.ship_setup(0, KIND_DOVER);
                self.place_ship();
                self.ship.visible = false;
            }
        }
    }
}
