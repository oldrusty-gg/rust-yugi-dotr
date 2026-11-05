//! Adapter for the existing imported presentation model. No duplicated rules.
use roses_client::{Button, Loaded, Manifest, Model, Outcome, Stage, StoryEnd};
use std::path::Path;

/// Story stages drawn over the captured Simon/Stonehenge scene.
const SIMON_STORIES: [&str; 11] = [
    "simon-arrival",
    "simon-post-name",
    "simon-practice-intro",
    "simon-deck-intro",
    "simon-deck-prompt",
    "simon-deck-chosen",
    "seto-arrival",
    "seto-side-intro",
    "seto-leaves",
    // Event 12000 (the Yorkists row): Seto sends Simon away.
    "seto-yorkists",
    // Event 11020 at Brest: the imported 2D castle covers the black world.
    "yugi-at-brest",
];
/// Explicit stand-ins for untraced map events (black world, message in the title).
const PLACEHOLDERS: [&str; 2] = ["map-event-100", "map-events-101-102"];

pub struct OpeningPreview {
    pub loaded: Loaded,
    pub model: Model,
    historical_enabled: bool,
    /// Dialogue pan byte 0x1ED5439 target of the last story page shown.
    pan_target: i8,
    /// The practice choice "Yes" was confirmed: the scripted tutorial
    /// starts and the story resumes at this stage (event 10060) when it
    /// ends (`Outcome::Tutorial`; a frontend without a duel scene reports
    /// the boundary instead).
    tutorial: Option<String>,
    /// A map story event's pages ended: the campaign map resumes at this
    /// map event (`Outcome::MapEvent`), taken once by the game.
    map_event: Option<u16>,
    /// The title menu's Continue was confirmed (taken by the game, which
    /// loads its interim save).
    continue_requested: bool,
    /// The title menu's Custom Duel was confirmed (taken by the game, which
    /// runs memory-card op 3 and opens mode 9).
    custom_duel_requested: bool,
    trade_requested: bool,
    /// The title's port-added Online Play entry was confirmed.
    online_requested: bool,
    /// SYSTEM0 sound effects of the presses handled since the game last
    /// took them (`take_sounds`).
    sounds: Vec<u8>,
    /// Map story stages move per 25 Hz update (`roses_client::story_motion`)
    /// once enabled; the settled pages otherwise.
    motion_enabled: bool,
    motion: Option<roses_client::story_motion::StoryMotion>,
    /// Crosses the motion hands to the model (page turns, stage ends),
    /// taken by the game's input pass (`take_forwarded`).
    forwarded: Vec<Button>,
    /// The speaker panels' talking/blinking faces (0x247310/0x247A10), when
    /// the game gave the face tables of its executable (`set_face_tables`).
    faces: Option<roses_client::face_anim::StoryFaces>,
    /// The settled page shown (stage, page) and its updates so far.
    settled_page: (String, usize, u32),
}

/// The music requests the original makes when this stage begins
/// (the project notes 1, 2.1, 2.4) [F calls; the port issues
/// each at stage entry, H where the original makes it later in the event]:
/// title and menu 2 (mode 1); history 10020 24 (mode 0); Simon's stages 25
/// (mode 1; the history exit starts it); Seto 10080.. 26 (10071's end, mode
/// 0, then mode 1); Brest 11020 27 (mode 1); red-route events as
/// dispatched. Track 1 (11124) is not in PCMLIST: the request fades out.
/// Not placed: 11160's later (32, 1), 11162's closing (4, 0), 11149's and
/// 11020's (4, 0) (the map's own track 4 follows them).
pub fn stage_music(id: &str) -> Vec<crate::duel_audio::AudioCue> {
    use crate::duel_audio::AudioCue;
    let music = |track, keep| vec![AudioCue::Music { track, keep }];
    match id {
        // 0x1AB440(-2, 1) before the attract movie [F 0x1AD340].
        "intro" => vec![AudioCue::FadeOut { speed: 0x100 }],
        "title" | "main-menu" => music(2, true),
        "historical-story" => music(24, false),
        "seto-arrival" => music(26, false),
        "seto-side-intro" | "side-choice" | "seto-leaves" | "seto-yorkists" => music(26, true),
        "yugi-at-brest" => music(27, true),
        "map-story-11120" => music(28, false),
        "map-story-11122" => music(28, true),
        "map-story-11124" => music(1, false),
        "map-story-11140" => music(29, false),
        "map-story-11142" => music(30, false),
        "map-story-11143" => music(30, true),
        "map-story-11149" => music(4, false),
        "map-story-11160" => music(31, false),
        // The thunder pair, then (33, 1) [F 0x220BB0..; H: both at entry].
        "map-story-11162" => vec![
            AudioCue::Se { slot: 0, id: 0x22, skip: false },
            AudioCue::Se { slot: 0, id: 0x23, skip: false },
            AudioCue::Music { track: 33, keep: true },
        ],
        "map-story-11182" => music(34, true),
        // White route [F dispatch: 0x221B58, 0x221E48, 0x222270 (after the
        // thunder pair 0x2220FC/0x222108), 0x222710, 0x222B70; H: at entry].
        "map-story-12040" => music(35, true),
        "map-story-12130" => music(36, true),
        "map-story-12132" => vec![
            AudioCue::Se { slot: 0, id: 0x22, skip: false },
            AudioCue::Se { slot: 0, id: 0x23, skip: false },
            AudioCue::Music { track: 33, keep: true },
        ],
        "map-story-12152" => music(37, true),
        "map-story-12156" => music(38, true),
        id if id.starts_with("simon-") || matches!(id, "name-entry" | "practice-choice" | "deck-choice") => {
            music(25, true)
        }
        _ => Vec::new(),
    }
}

/// The PCMLIST track of a story import music asset.
pub fn story_track(asset: &str) -> Option<u32> {
    match asset {
        "historical-music" => Some(24),
        "simon-music" => Some(25),
        _ => None,
    }
}

/// The SYSTEM0 sound of a press in an opening stage, as the original's
/// input handlers play it (the project notes 1-2) [F sites]:
/// title Start 4; main menu Up / Down 3, confirm 4; paged dialogue 2; name
/// entry arrows 2, ○ 5, L1 / R1 7, insert 4, Start 1; practice / side
/// choice Up / Down 3, Start 1; deck choice Left / Right 9, Start 1.
/// The side choice locked after an ending answers Up / Down with 6 (FLOW
/// C5) [F]. [H: the dialogue's display-phase gate is not modelled: a
/// handled press sounds.]
fn press_sound(stage: &roses_client::Stage, button: Button, outcome: &Outcome, locked: bool) -> Option<u8> {
    use roses_client::Stage;
    let handled = !matches!(outcome, Outcome::None);
    if locked && matches!(button, Button::Up | Button::Down) {
        return Some(6);
    }
    match stage {
        Stage::NameEntry { .. } => match button {
            Button::Up | Button::Down | Button::Left | Button::Right => Some(2),
            Button::Circle => Some(5),
            Button::L1 | Button::R1 => Some(7),
            Button::Cross | Button::Square if handled => Some(4),
            Button::Start if handled => Some(1),
            _ => None,
        },
        Stage::Story { .. } if handled => Some(2),
        Stage::Screen { .. } if handled => Some(4),
        Stage::Menu { id, .. } => match button {
            Button::Up | Button::Down if handled => Some(3),
            _ if handled && id == "main-menu" => Some(4),
            _ if handled => Some(1),
            _ => None,
        },
        Stage::DeckChoice { .. } => match button {
            Button::Left | Button::Right if handled => Some(9),
            _ if handled => Some(1),
            _ => None,
        },
        _ => None,
    }
}

/// The story stage of event 10060 (the tutorial's re-entry, FLOW.md Y6).
pub const TUTORIAL_RESUME_STAGE: &str = "simon-deck-intro";
/// What the preview draws beneath the imported 2D overlay for a stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum World {
    /// Title and main menu are overlay-only; the canvas beneath is black.
    Black,
    /// Researched c400 cloud checkpoint (historical story).
    Clouds,
    /// Captured RH0 Simon/Stonehenge scene.
    Simon,
    /// The Stonehenge story events' world (MODEL.MRG pack 33, bank
    /// 0x1E91B0(3, 0)): events 11160, 11162, 11180 and 11184. Black when the
    /// game import has no story-scene part.
    StoryScene0,
}
pub fn stage_world(stage: &Stage) -> World {
    match stage.id() {
        "historical-story" => World::Clouds,
        "intro" | "title" | "main-menu" | "yugi-at-brest" => World::Black,
        id if is_scene0_stage(id) => World::StoryScene0,
        id if PLACEHOLDERS.contains(&id) || is_map_story(id) => World::Black,
        _ => World::Simon,
    }
}
/// Last dialogue page pan of the story whose end leads to `start` (the
/// original keeps 0x1ED5439 through menus and the deck select).
fn inherited_pan(manifest: &Manifest, start: &str) -> Option<i8> {
    manifest.stages.iter().find_map(|stage| match stage {
        Stage::Story {
            pages,
            on_end: StoryEnd::Stage { target },
            ..
        } if target == start => pages.last().map(|page| page.pan.unwrap_or(100)),
        _ => None,
    })
}
/// A campaign-map story event's pages (`roses import-story-red`). The
/// connected game plays them over the world the original draws behind them
/// (`roses_client::story_motion::behind`): the scene-0 stages in the story
/// scene, the map ones over the running map (`plays_over_map`); started
/// directly (`--start-stage`) the map ones are drawn over black.
pub fn is_map_story(id: &str) -> bool {
    id.starts_with(MAP_STORY_PREFIX)
}
pub const MAP_STORY_PREFIX: &str = "map-story-";
/// The event of a `map-story-<event>` stage.
pub fn map_story_event(id: &str) -> Option<i16> {
    id.strip_prefix(MAP_STORY_PREFIX)?.parse().ok()
}
fn behind(id: &str) -> Option<roses_client::story_motion::Behind> {
    map_story_event(id).and_then(|e| roses_client::story_motion::behind(u16::try_from(e).ok()?))
}
/// Stages drawn over the scene-0 world: 11160..11184 and 12130..12156
/// [emu: W+53B5 3/2/3/1 and 10 model draws per frame in these events;
/// 11182 and 12154 cover it with paintings; white: 12130 3, 12132 2, 12150
/// 3, 12152 3, 12156 1 (the project notes traces)].
pub fn is_scene0_stage(id: &str) -> bool {
    behind(id) == Some(roses_client::story_motion::Behind::Scene0)
}
/// Story stages the connected game plays inside the campaign map scene,
/// over the running map: 11122 and 11140..11148 (the original never leaves
/// the map mode there: 161..193 model draws every frame, the map shows
/// behind 11140's and 11148's figures and panels, the black fill or
/// paintings cover it in 11122..11147 [emu trace-trigger-cam, trace-win3;
/// the project notes]). The map's own window and dialogue events
/// (11120, 11124, 11149, 12040, 12041) are played by the map model.
pub fn plays_over_map(id: &str) -> bool {
    use roses_client::campaign_map::{DIALOGUE_EVENTS, MapModel};
    let Some(event) = map_story_event(id) else { return false };
    behind(id) == Some(roses_client::story_motion::Behind::Map)
        && !MapModel::is_window_event(event)
        && !DIALOGUE_EVENTS.iter().any(|(e, _)| *e == event)
}
/// The story stage of a campaign-map story event: 11020 (Brest, between the
/// two voyages) is the story import's `yugi-at-brest`; the red route's
/// events are `map-story-<event>` (`roses import-story-red`).
pub fn map_story_stage(event: i16) -> String {
    if event == roses_client::campaign_map::EVENT_BREST_STORY {
        BREST_STAGE.into()
    } else {
        format!("{MAP_STORY_PREFIX}{event}")
    }
}
pub const BREST_STAGE: &str = "yugi-at-brest";
/// The map event a story stage hands back to the map because the map plays
/// it with its story window over the live map (`map-story-11124`,
/// `map-story-11149`; 11120 is started by the map itself).
pub fn map_window_event(id: &str) -> Option<u16> {
    let event = map_story_event(id)?;
    (roses_client::campaign_map::MapModel::is_window_event(event)
        && event != roses_client::campaign_map::EVENT_BOSWORTH_TRIGGER)
        .then_some(event as u16)
}
fn mapped_stage(stage: &Stage, historical_enabled: bool) -> bool {
    matches!(stage, Stage::Story { id, .. } if SIMON_STORIES.contains(&id.as_str()) || is_map_story(id) || (historical_enabled && id == "historical-story"))
        || matches!(stage, Stage::NameEntry { id, .. } if id == "name-entry")
        || matches!(stage, Stage::Movie { id, .. } if id == "intro")
        || matches!(stage, Stage::Screen { id, .. } if id == "title")
        || matches!(stage, Stage::Menu { id, .. } if matches!(id.as_str(), "main-menu" | "practice-choice" | "side-choice"))
        || matches!(stage, Stage::DeckChoice { id, .. } if id == "deck-choice")
        || matches!(stage, Stage::Placeholder { id, .. } if PLACEHOLDERS.contains(&id.as_str()))
}
impl OpeningPreview {
    pub fn open(
        path: &Path,
        start: Option<&str>,
        historical_enabled: bool,
    ) -> Result<Self, String> {
        Self::open_with(path, start, historical_enabled, crate::extras())
    }

    /// [`Self::open`] with the port additions offered or not (`extras`:
    /// false is faithful mode, the original's title menu).
    pub fn open_with(
        path: &Path,
        start: Option<&str>,
        historical_enabled: bool,
        extras: bool,
    ) -> Result<Self, String> {
        #[cfg_attr(not(feature = "online"), allow(unused_mut))]
        let mut loaded = Loaded::open(path)?;
        // Port addition: the title's "Online Play" entry (online_title.rs);
        // faithful mode (--faithful, a build without `online`) leaves the
        // original four entries.
        #[cfg(feature = "online")]
        if extras {
            crate::online_title::add_online_entry(&mut loaded.manifest);
        }
        #[cfg(not(feature = "online"))]
        let _ = extras;
        let start = start.unwrap_or("simon-arrival");
        let known = matches!(
            start,
            "intro"
                | "title"
                | "main-menu"
                | "historical-story"
                | "name-entry"
                | "practice-choice"
                | "deck-choice"
                | "side-choice"
        ) || SIMON_STORIES.contains(&start)
            || PLACEHOLDERS.contains(&start)
            || is_map_story(start);
        if !known {
            return Err("Preview supports intro, title, main-menu, historical-story, name-entry, practice-choice, deck-choice, side-choice, the Simon/Seto/Yugi story stages and the map stand-ins as starts".into());
        }
        let c = loaded.manifest.canvas;
        if (c.width, c.height, c.aspect_width, c.aspect_height) != (640, 448, 4, 3) {
            return Err("Opening overlay requires original 640x448 / 4:3 canvas".into());
        }
        let mut manifest = loaded.manifest.clone();
        manifest.initial_stage = start.into();
        let model = Model::new(manifest)?;
        if !mapped_stage(model.stage(), historical_enabled) {
            return Err("Preview stage has an unsupported presentation kind".into());
        }
        // A menu or deck-select start keeps the pan of the story leading to it.
        let pan_target = inherited_pan(&loaded.manifest, start).unwrap_or(100);
        let mut preview = Self {
            loaded,
            model,
            historical_enabled,
            pan_target,
            tutorial: None,
            map_event: None,
            continue_requested: false,
            custom_duel_requested: false,
            trade_requested: false,
            online_requested: false,
            sounds: Vec::new(),
            motion_enabled: false,
            motion: None,
            forwarded: Vec::new(),
            faces: None,
            settled_page: (String::new(), 0, 0),
        };
        preview.follow_pan();
        Ok(preview)
    }

    /// The map event a finished map story stage continues at (taken once).
    pub fn take_map_event(&mut self) -> Option<u16> {
        self.map_event.take()
    }

    /// Whether the title menu's Continue was confirmed since last asked.
    pub fn take_continue(&mut self) -> bool {
        std::mem::take(&mut self.continue_requested)
    }

    /// Whether the title menu's Custom Duel was confirmed since last asked.
    /// The title's Trade was chosen (memory-card op 4, then mode 8).
    pub fn take_trade(&mut self) -> bool {
        std::mem::take(&mut self.trade_requested)
    }

    /// The title's Online Play (a port addition) was chosen.
    pub fn take_online(&mut self) -> bool {
        std::mem::take(&mut self.online_requested)
    }

    pub fn take_custom_duel(&mut self) -> bool {
        std::mem::take(&mut self.custom_duel_requested)
    }

    /// The stage the story resumes at once the tutorial a confirmed
    /// practice "Yes" started has ended (taken once).
    pub fn take_tutorial(&mut self) -> Option<String> {
        self.tutorial.take()
    }

    /// Re-enter the story at `stage` after the tutorial, keeping the
    /// committed name and campaign state (the original reloads the scene
    /// and restarts track 25; its camera on re-entry is not traced, the
    /// captured Simon pose is used).
    pub fn resume_at(&mut self, stage: &str) -> Result<(), String> {
        let previous = self.model.clone();
        self.model.jump_to(stage)?;
        self.motion = None;
        self.forwarded.clear();
        self.ensure_motion();
        if !mapped_stage(self.model.stage(), self.historical_enabled) {
            self.model = previous;
            return Err(format!(
                "Stage {stage} has no mapped captured scene in this preview"
            ));
        }
        self.pan_target = inherited_pan(&self.loaded.manifest, stage).unwrap_or(100);
        self.follow_pan();
        Ok(())
    }

    /// Pan target for the Simon scene camera and big portraits (0x1ED5439).
    /// Menus and the deck select keep the last dialogue page's value; a
    /// moving story stage gives its own W+5439.
    pub fn pan_target(&self) -> i8 {
        match self.active_motion() {
            Some(m) => m.pan.clamp(-100, 100) as i8,
            None => self.pan_target,
        }
    }

    /// Let map story stages move (letterbox, slides, panels, text reveal).
    pub fn enable_motion(&mut self, on: bool) {
        self.motion_enabled = on;
        self.motion = None;
        self.ensure_motion();
    }

    /// The motion of the current stage, if it moves.
    pub fn active_motion(&self) -> Option<&roses_client::story_motion::StoryMotion> {
        self.motion.as_ref().filter(|m| m.stage() == self.model.stage().id())
    }

    /// (Re)build the motion when the stage or page moved without it.
    fn ensure_motion(&mut self) {
        if !self.motion_enabled || !self.forwarded.is_empty() {
            return;
        }
        let id = self.model.stage().id();
        let page = self.model.page_index();
        if self
            .motion
            .as_ref()
            .is_some_and(|m| m.stage() == id && (m.finished() || m.page == page))
        {
            return;
        }
        let carry = self.motion.take().filter(|m| m.finished()).map(|m| m.carry());
        self.motion = roses_client::story_motion::StoryMotion::new(&self.loaded, &self.model, carry);
    }

    /// Animate the story panels' faces with the tables of the user's
    /// executable (`FaceTables::from_executable`).
    pub fn set_face_tables(&mut self, tables: roses_client::face_anim::FaceTables) {
        self.faces = Some(roses_client::face_anim::StoryFaces::new(tables));
    }

    /// One 25 Hz update of the story motion and the panels' faces; true
    /// when the frame changed.
    pub fn tick(&mut self) -> bool {
        self.ensure_motion();
        let mut changed = false;
        if let Some(m) = self.motion.as_mut().filter(|m| !m.finished()) {
            let step = m.step();
            if let Some(button) = step.forward {
                self.forwarded.push(button);
            }
            changed = step.changed;
        }
        changed | self.step_faces()
    }

    /// The faces' requests and step for this update (0x247A10 once per
    /// update): kind 3 on the motion's talking side, or on a settled
    /// page's speaker for its reveal's length.
    fn step_faces(&mut self) -> bool {
        if self.faces.is_none() {
            return false;
        }
        let layers = self.loaded.story_page_layers(&self.model).unwrap_or_default();
        let talking = match self.active_motion() {
            Some(m) => m.talking(),
            None => {
                let (stage, page) = (self.model.stage().id().to_owned(), self.model.page_index());
                if self.settled_page.0 != stage || self.settled_page.1 != page {
                    self.settled_page = (stage, page, 0);
                }
                self.settled_page.2 += 1;
                match self.model.story_page() {
                    Some(p) => roses_client::story_motion::settled_talking(
                        p.pan.unwrap_or(100),
                        roses_client::story_motion::page_lines(&self.loaded, p),
                        self.settled_page.2,
                    ),
                    None => [false; 2],
                }
            }
        };
        self.faces.as_mut().is_some_and(|f| f.step(&layers, talking))
    }

    /// Crosses the motion released since the last call (to `input_forwarded`).
    pub fn take_forwarded(&mut self) -> Vec<Button> {
        std::mem::take(&mut self.forwarded)
    }

    /// The overlay for this update (moving story stages included).
    pub fn overlay(&self) -> Result<roses_client::Image, String> {
        self.loaded.render_overlay_faces(&self.model, self.active_motion(), self.faces.as_ref())
    }
    fn follow_pan(&mut self) {
        if let Some(page) = self.model.story_page() {
            self.pan_target = page.pan.unwrap_or(100);
        }
    }

    /// The story import's music entry for this asset, as a stage carries it.
    pub fn story_music(&self, asset: &str) -> Option<roses_client::Music> {
        self.loaded.manifest.stages.iter().filter_map(Stage::music).find(|m| m.asset == asset).cloned()
    }

    /// The sound effects of the presses handled since the last call.
    pub fn take_sounds(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.sounds)
    }

    /// A pad press: a moving story stage takes it (Cross is read when the
    /// page waits; the page turns when the motion forwards it).
    pub fn input(&mut self, button: Button) -> Result<bool, String> {
        self.ensure_motion();
        let id = self.model.stage().id().to_owned();
        if let Some(m) = self.motion.as_mut().filter(|m| m.stage() == id && !m.finished()) {
            m.press(button);
            return Ok(false);
        }
        self.input_model(button)
    }

    /// A Cross the story motion forwarded: straight to the model.
    pub fn input_forwarded(&mut self, button: Button) -> Result<bool, String> {
        let result = self.input_model(button);
        self.ensure_motion();
        result
    }

    fn input_model(&mut self, button: Button) -> Result<bool, String> {
        let previous = self.model.clone();
        let outcome = self.model.input(button);
        if let Some(id) = press_sound(previous.stage(), button, &outcome, previous.selection_locked()) {
            self.sounds.push(id);
        }
        let result = match outcome {
            Outcome::None => Ok(false),
            Outcome::Repaint => Ok(true),
            // 11124 after 11122 and 11149 after 11148: the map plays these
            // window events over the running map (the story import's window
            // pages are not shown in the connected game).
            Outcome::StageChanged if map_window_event(self.model.stage().id()).is_some() => {
                self.map_event = map_window_event(self.model.stage().id());
                Ok(false)
            }
            Outcome::StageChanged if mapped_stage(self.model.stage(), self.historical_enabled) => {
                Ok(true)
            }
            Outcome::StageChanged => {
                self.model = previous;
                Err("Next stage has no mapped captured scene in this preview".into())
            }
            Outcome::Tutorial { resume } => {
                self.tutorial = Some(resume);
                Ok(false)
            }
            Outcome::MapEvent(event) => {
                self.map_event = Some(event);
                Ok(false)
            }
            // Story imports made before the ending was ported end 11184
            // with an unsupported boundary; its continuation is map event
            // 11184's tail, the ending (`roses_client::ending`).
            Outcome::Unsupported(_)
                if self.model.stage().id()
                    == map_story_stage(roses_client::ending::EVENT_RED_ENDING as i16) =>
            {
                self.map_event = Some(roses_client::ending::EVENT_RED_ENDING);
                Ok(false)
            }
            // Story imports made before the tutorial existed name the
            // practice "Yes" an unsupported boundary; the stage of event
            // 10060 is the same, so the game plays the tutorial for them too
            // (said on stderr).
            Outcome::Unsupported(message)
                if self.model.stage().id() == "practice-choice"
                    && message.contains("scripted tutorial")
                    && self.model.stage_ids().contains(&TUTORIAL_RESUME_STAGE) =>
            {
                eprintln!(
                    "Opening: the story import names the practice \"Yes\" a boundary (an older import); the game plays the scripted tutorial and resumes at {TUTORIAL_RESUME_STAGE}"
                );
                self.tutorial = Some(TUTORIAL_RESUME_STAGE.into());
                Ok(false)
            }
            // The title menu's Continue: the game loads its interim save
            // (the original's load screens are untraced).
            Outcome::Unavailable(label)
                if self.model.stage().id() == "main-menu" && label == "Continue" =>
            {
                self.continue_requested = true;
                Ok(false)
            }
            // The title menu's Custom Duel (the import names it unavailable,
            // as Continue): memory-card op 3, then mode 9.
            Outcome::Unavailable(label)
                if self.model.stage().id() == "main-menu" && label == "Custom Duel" =>
            {
                self.custom_duel_requested = true;
                Ok(false)
            }
            // The title menu's Trade (unavailable in the import too):
            // memory-card op 4, then mode 8 [F 0x1AD050, emu op4-*].
            Outcome::Unavailable(label)
                if self.model.stage().id() == "main-menu" && label == "Trade" =>
            {
                self.trade_requested = true;
                Ok(false)
            }
            #[cfg(feature = "online")]
            Outcome::Unavailable(label)
                if self.model.stage().id() == "main-menu" && label == crate::online_title::ONLINE_LABEL =>
            {
                self.online_requested = true;
                Ok(false)
            }
            Outcome::Unsupported(message) | Outcome::Unavailable(message) => {
                // Keep what the flow chose so far visible at the boundary.
                if *self.model.campaign() == roses_client::CampaignState::default() {
                    Err(message)
                } else {
                    Err(format!(
                        "{message} Campaign state: {}",
                        self.model.campaign().to_json()
                    ))
                }
            }
        };
        self.follow_pan();
        result
    }

    /// Explicit placeholder text for parts of the current stage that are not
    /// traced (deck-select details) or not implemented (map stand-ins); the
    /// same shared text the native client draws.
    pub fn placeholder(&self) -> Option<String> {
        self.model.explicit_status()
    }

    /// The frontend's movie finished playing (not skipped).
    pub fn movie_ended(&mut self) -> Result<bool, String> {
        match self.model.video_ended() {
            Outcome::StageChanged if mapped_stage(self.model.stage(), self.historical_enabled) => {
                Ok(true)
            }
            Outcome::None => Ok(false),
            _ => Err("Movie end has no mapped successor in this preview".into()),
        }
    }

    pub fn caption(&self) -> String {
        let detail = if let Some(name) = self.model.name_entry() {
            format!(
                "name page {} row {} col {} · {} glyphs",
                name.page() + 1,
                name.row() + 1,
                name.column() + 1,
                name.glyphs().len()
            )
        } else {
            format!(
                "{} page {}",
                self.model.stage().id(),
                self.model.page_index() + 1
            )
        };
        let scene = match stage_world(self.model.stage()) {
            World::Clouds => "cloud checkpoint",
            World::Simon => "captured Stonehenge scene",
            World::StoryScene0 => "Stonehenge story world (pack 33)",
            World::Black if self.model.stage().id() == "yugi-at-brest" => {
                if self.loaded.assets.contains_key("brest-castle") {
                    "Brest castle painting (settled dialogue; entrance animation absent)"
                } else {
                    "Brest castle backdrop not imported (black)"
                }
            }
            World::Black if PLACEHOLDERS.contains(&self.model.stage().id()) => {
                "campaign travel animation not implemented (black)"
            }
            World::Black if is_map_story(self.model.stage().id()) && self.active_motion().is_some() => {
                "map story event (letterbox, paintings, figures, panels and text moving per update; map behind windows not drawn)"
            }
            World::Black if is_map_story(self.model.stage().id()) => {
                "map story event (settled pages, no motion; Stonehenge scene bank and map behind windows not drawn)"
            }
            World::Black => "2D stage",
        };
        let audio = if matches!(self.model.stage(), Stage::Movie { .. }) {
            "the movie's own audio"
        } else if self.model.stage().music().is_some() {
            "music, no SFX"
        } else {
            "no music on this stage in the import, no SFX"
        };
        match self.placeholder() {
            Some(text) => format!("Roses Bevy preview · {detail} · {text} · {scene} · {audio}"),
            None => format!("Roses Bevy preview · {detail} · {scene} · {audio}"),
        }
    }
}

/// Bounded explicit buttons pass directly into the same Model as live input.
pub fn parse_replay(text: &str) -> Result<Vec<Button>, String> {
    if text.is_empty() || text.len() > 4096 {
        return Err("Replay requires 1..256 comma-separated buttons (at most 4096 bytes)".into());
    }
    let mut buttons = Vec::new();
    for word in text.split(',') {
        if buttons.len() == 256 {
            return Err("Replay exceeds 256 buttons".into());
        }
        buttons.push(match word {
            "cross" => Button::Cross,
            "circle" => Button::Circle,
            "square" => Button::Square,
            "start" => Button::Start,
            "l1" => Button::L1,
            "r1" => Button::R1,
            "up" => Button::Up,
            "down" => Button::Down,
            "left" => Button::Left,
            "right" => Button::Right,
            _ => return Err(format!("Unknown replay button: {word}")),
        });
    }
    Ok(buttons)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_map_plays_the_window_events_after_the_story_stages() {
        assert_eq!(map_window_event("map-story-11124"), Some(11124));
        assert_eq!(map_window_event("map-story-11149"), Some(11149));
        for id in ["map-story-11120", "map-story-11122", "map-story-11140", "map-story-11148", "yugi-at-brest", "map-story-x"] {
            assert_eq!(map_window_event(id), None, "{id}");
        }
    }
    #[test]
    fn map_story_stages_play_over_the_world_the_original_draws() {
        let over_map: Vec<u16> = (11000..13000)
            .filter(|e| plays_over_map(&format!("map-story-{e}")))
            .collect();
        assert_eq!(over_map, [11122, 11140, 11142, 11143, 11144, 11146, 11147, 11148]);
        let scene0: Vec<u16> = [11160u16, 11162, 11180, 11182, 11184, 12130, 12132, 12150, 12152, 12154, 12156]
            .into_iter()
            .filter(|e| is_scene0_stage(&format!("map-story-{e}")))
            .collect();
        assert_eq!(scene0.len(), 11);
        for id in ["map-story-11140", "map-story-11124", "map-story-12040", "yugi-at-brest", "seto-yorkists"] {
            assert!(!is_scene0_stage(id), "{id}");
        }
        assert!(!plays_over_map("yugi-at-brest") && !plays_over_map("map-story-11160"));
    }
    #[test]
    fn stage_music_follows_the_original_requests() {
        use crate::duel_audio::AudioCue::{self, Music};
        let tracks = |id: &str| -> Vec<(u32, bool)> {
            stage_music(id).into_iter().filter_map(|c| match c {
                Music { track, keep } => Some((track, keep)),
                _ => None,
            }).collect()
        };
        assert_eq!(tracks("title"), [(2, true)]);
        assert_eq!(tracks("historical-story"), [(24, false)]);
        for id in ["simon-arrival", "name-entry", "practice-choice", "deck-choice", "simon-deck-chosen"] {
            assert_eq!(tracks(id), [(25, true)], "{id}");
        }
        assert_eq!(tracks("seto-arrival"), [(26, false)]);
        assert_eq!(tracks("map-story-11124"), [(1, false)]);
        assert!(matches!(stage_music("intro")[..], [AudioCue::FadeOut { speed: 0x100 }]));
        assert_eq!(story_track("simon-music"), Some(25));
    }
    #[test]
    fn replay_rejects_partial_unknown_and_oversized_sequences() {
        assert!(parse_replay("cross,,start").is_err());
        assert!(parse_replay("cross,play").is_err());
        assert!(parse_replay(&vec!["cross"; 257].join(",")).is_err());
        assert_eq!(
            parse_replay("cross,circle,start").unwrap(),
            [Button::Cross, Button::Circle, Button::Start]
        );
    }

    #[test]
    #[ignore = "Requires a privately imported opening bundle"]
    fn private_simon_to_name_flow_uses_shared_model() {
        let path = std::env::var_os("ROSES_OPENING_MANIFEST").expect("ROSES_OPENING_MANIFEST");
        let mut preview = OpeningPreview::open(Path::new(&path), None, false).unwrap();
        for _ in 0..12 {
            preview.input(Button::Cross).unwrap();
        }
        assert_eq!(preview.model.stage().id(), "name-entry");
        assert!(!preview.input(Button::Start).unwrap());
        preview.input(Button::Square).unwrap();
        preview.input(Button::R1).unwrap();
        preview.input(Button::Right).unwrap();
        preview.input(Button::Cross).unwrap();
        let overlay = preview.loaded.render_overlay(&preview.model).unwrap();
        assert_eq!((overlay.width, overlay.height), (640, 448));
        assert_eq!(preview.model.name_entry().unwrap().glyphs().len(), 2);
        assert!(preview.input(Button::Start).is_err());
        assert_eq!(preview.model.player_name().unwrap().glyphs.len(), 2);
    }

    #[test]
    #[ignore = "Requires a private post-name import (ROSES_POST_NAME_MANIFEST)"]
    fn private_name_to_post_name_dialogue_and_boundary() {
        let path = std::env::var_os("ROSES_POST_NAME_MANIFEST").expect("ROSES_POST_NAME_MANIFEST");
        let mut preview =
            OpeningPreview::open(Path::new(&path), Some("name-entry"), false).unwrap();
        for button in [Button::Square, Button::R1, Button::Right, Button::Cross] {
            preview.input(button).unwrap();
        }
        preview.input(Button::Start).unwrap();
        assert_eq!(preview.model.stage().id(), "simon-post-name");
        let mut name_pages = 0;
        for page in 0..20 {
            assert_eq!(preview.model.page_index(), page);
            let overlay = preview.loaded.render_overlay(&preview.model).unwrap();
            assert_eq!((overlay.width, overlay.height), (640, 448));
            if preview.loaded.name_text(&preview.model).unwrap().is_some() {
                name_pages += 1;
            }
            preview.input(Button::Cross).unwrap();
        }
        assert_eq!(name_pages, 2);
        assert_eq!(preview.model.stage().id(), "simon-practice-intro");
        for _ in 0..10 {
            preview.input(Button::Cross).unwrap();
        }
        preview.input(Button::Cross).unwrap();
        assert_eq!(preview.model.stage().id(), "practice-choice");
        // Cross is not handled by the original choice branch; Start confirms.
        assert!(!preview.input(Button::Cross).unwrap());
        preview.input(Button::Down).unwrap();
        assert_eq!(preview.model.selection, 1);
        preview.loaded.render_overlay(&preview.model).unwrap();
        // v3 stops here; a story-to-map import continues to event 10060.
        match preview.input(Button::Start) {
            Err(_) => assert_eq!(preview.model.stage().id(), "practice-choice"),
            Ok(_) => assert_eq!(preview.model.stage().id(), "simon-deck-intro"),
        }
    }

    #[test]
    #[ignore = "Requires a privately imported opening bundle"]
    fn private_faithful_title_menu_has_only_the_original_entries() {
        let path = std::env::var_os("ROSES_OPENING_MANIFEST").expect("ROSES_OPENING_MANIFEST");
        let ids = |extras: bool| -> Vec<String> {
            let preview = OpeningPreview::open_with(Path::new(&path), Some("main-menu"), true, extras).unwrap();
            let menu = preview.loaded.manifest.stages.iter().find(|s| s.id() == "main-menu").unwrap();
            let roses_client::Stage::Menu { entries, .. } = menu else { panic!("main-menu is a menu") };
            entries.iter().map(|e| e.id.clone()).collect()
        };
        // Faithful mode: the original's four entries, nothing added.
        let faithful = ids(false);
        assert_eq!(faithful.len(), 4, "{faithful:?}");
        assert!(!faithful.iter().any(|id| id == "online-play"));
        // With the additions (the online build): Online Play below them.
        let extras = ids(true);
        if cfg!(feature = "online") {
            assert_eq!(extras.len(), 5, "{extras:?}");
            assert_eq!(extras[..4], faithful[..]);
            assert_eq!(extras[4], "online-play");
        } else {
            assert_eq!(extras, faithful, "a build without `online` never adds it");
        }
    }

    #[test]
    #[ignore = "Requires a privately imported opening bundle"]
    fn private_title_menu_to_history_uses_shared_model() {
        let path = std::env::var_os("ROSES_OPENING_MANIFEST").expect("ROSES_OPENING_MANIFEST");
        let intro = OpeningPreview::open(Path::new(&path), Some("intro"), true).unwrap();
        assert_eq!(stage_world(intro.model.stage()), World::Black);
        assert!(OpeningPreview::open(Path::new(&path), Some("credits"), true).is_err());
        let mut preview = OpeningPreview::open(Path::new(&path), Some("title"), true).unwrap();
        assert_eq!(stage_world(preview.model.stage()), World::Black);
        preview.loaded.render_overlay(&preview.model).unwrap();
        preview.input(Button::Start).unwrap();
        assert_eq!(preview.model.stage().id(), "main-menu");
        preview.loaded.render_overlay(&preview.model).unwrap();
        // Unavailable menu destinations stay explicit instead of inventing screens.
        preview.input(Button::Down).unwrap();
        assert!(preview.input(Button::Cross).is_err());
        preview.input(Button::Up).unwrap();
        preview.input(Button::Cross).unwrap();
        assert_eq!(preview.model.stage().id(), "historical-story");
        assert_eq!(stage_world(preview.model.stage()), World::Clouds);
        // Without the private cloud renderer, New Game stops at an explicit boundary.
        let mut no_clouds =
            OpeningPreview::open(Path::new(&path), Some("main-menu"), false).unwrap();
        assert!(no_clouds.input(Button::Cross).is_err());
        assert_eq!(no_clouds.model.stage().id(), "main-menu");
    }

    #[test]
    #[ignore = "Requires a privately imported opening bundle"]
    fn private_history_to_simon_to_name_uses_shared_model() {
        let path = std::env::var_os("ROSES_OPENING_MANIFEST").expect("ROSES_OPENING_MANIFEST");
        assert!(OpeningPreview::open(Path::new(&path), Some("historical-story"), false).is_err());
        let mut preview =
            OpeningPreview::open(Path::new(&path), Some("historical-story"), true).unwrap();
        for page in 0..15 {
            assert_eq!(preview.model.stage().id(), "historical-story");
            assert_eq!(preview.model.page_index(), page);
            preview.loaded.render_overlay(&preview.model).unwrap();
            preview.input(Button::Cross).unwrap();
        }
        assert_eq!(preview.model.stage().id(), "simon-arrival");
        for _ in 0..12 {
            preview.input(Button::Cross).unwrap();
        }
        assert_eq!(preview.model.stage().id(), "name-entry");
        assert!(!preview.input(Button::Start).unwrap());
    }

    /// Enter the FLOW.md test name (twelve times cell page 0, row 0, column
    /// 0) and walk the post-name dialogue to the practice choice.
    fn to_practice_choice(path: &Path) -> OpeningPreview {
        let mut preview = OpeningPreview::open(path, Some("name-entry"), false).unwrap();
        for _ in 0..12 {
            preview.input(Button::Cross).unwrap();
        }
        preview.input(Button::Start).unwrap();
        assert_eq!(
            preview.model.player_name().unwrap().glyphs[0].stored_code,
            0x2005
        );
        for _ in 0..20 + 11 {
            preview.input(Button::Cross).unwrap();
        }
        assert_eq!(preview.model.stage().id(), "practice-choice");
        preview
    }

    fn story_pages(preview: &mut OpeningPreview, stage: &str, pages: usize) -> Vec<i8> {
        assert_eq!(preview.model.stage().id(), stage);
        let mut pans = Vec::new();
        for page in 0..pages {
            assert_eq!(preview.model.page_index(), page, "{stage}");
            let overlay = preview.loaded.render_overlay(&preview.model).unwrap();
            assert_eq!((overlay.width, overlay.height), (640, 448));
            pans.push(preview.pan_target());
            // Cross advances; nothing else does (Start/Left are ignored).
            assert!(!preview.input(Button::Start).unwrap());
            preview.input(Button::Cross).unwrap();
        }
        pans
    }

    #[test]
    #[ignore = "Requires a private story-to-map import (ROSES_STORY_MAP_MANIFEST)"]
    fn private_practice_no_runs_the_story_to_the_map_voyages() {
        let path = std::env::var_os("ROSES_STORY_MAP_MANIFEST").expect("ROSES_STORY_MAP_MANIFEST");
        let mut preview = to_practice_choice(Path::new(&path));
        preview.input(Button::Down).unwrap();
        preview.input(Button::Start).unwrap();
        assert_eq!(stage_world(preview.model.stage()), World::Simon);
        let pans = story_pages(&mut preview, "simon-deck-intro", 8);
        assert!(pans.iter().all(|&p| p == 100));
        story_pages(&mut preview, "simon-deck-prompt", 3);
        assert_eq!(preview.model.stage().id(), "deck-choice");
        let decks = preview.model.deck_candidates().unwrap().unwrap();
        assert_eq!(decks.map(|d| d.record), [9, 13, 16]);
        assert_eq!(decks.map(|d| d.leader), [392, 612, 622]);
        assert!(preview.placeholder().unwrap().contains("[placeholder"));
        // Cross does nothing at the deck select; Left wraps to the last deck.
        assert!(!preview.input(Button::Cross).unwrap());
        preview.input(Button::Left).unwrap();
        assert_eq!(preview.model.selection, 2);
        preview.input(Button::Right).unwrap();
        preview.input(Button::Right).unwrap();
        assert_eq!(preview.model.selection, 1);
        preview.loaded.render_overlay(&preview.model).unwrap();
        preview.input(Button::Start).unwrap();
        let chosen = preview.model.campaign().starter_deck.clone().unwrap();
        assert_eq!(
            (chosen.checksum, chosen.record, chosen.leader),
            (12, 13, 612)
        );
        assert_eq!(chosen.cards.len(), 40);
        story_pages(&mut preview, "simon-deck-chosen", 1);
        let pans = story_pages(&mut preview, "seto-arrival", 73);
        // Groups 1-2 without Seto's panel (0), Simon's page 3 (+100), then
        // Seto's panel pages (-100).
        assert_eq!(pans[..5], [0, 0, 0, 100, -100]);
        assert_eq!(*pans.last().unwrap(), 100);
        story_pages(&mut preview, "seto-side-intro", 5);
        assert_eq!(preview.model.stage().id(), "side-choice");
        assert_eq!(preview.model.selection, 1);
        assert_eq!(preview.pan_target(), -100);
        preview.input(Button::Start).unwrap();
        assert_eq!(
            preview.model.campaign().side,
            Some(roses_client::CampaignSide::Lancastrians)
        );
        story_pages(&mut preview, "seto-leaves", 6);
        // 11000 continues at campaign-map event 100 (the voyage); the
        // connected game plays it and returns for 11020 (yugi-at-brest).
        assert_eq!(preview.take_map_event(), Some(100));
        assert_eq!(preview.take_map_event(), None);
        preview.resume_at(BREST_STAGE).unwrap();
        assert_eq!(stage_world(preview.model.stage()), World::Black);
        let mut name_pages = 0;
        for _ in 0..30 {
            if preview.loaded.name_text(&preview.model).unwrap().is_some() {
                name_pages += 1;
            }
            preview.input(Button::Cross).unwrap();
        }
        assert_eq!(name_pages, 2);
        // 11020 continues at event 101 (the voyage to Milford Haven).
        assert_eq!(preview.take_map_event(), Some(101));
        assert_eq!(
            preview.model.campaign().side,
            Some(roses_client::CampaignSide::Lancastrians)
        );
    }

    #[test]
    #[ignore = "Requires a private story-to-map import (ROSES_STORY_MAP_MANIFEST)"]
    fn private_practice_yes_and_yorkists_stop_at_explicit_boundaries() {
        let path = std::env::var_os("ROSES_STORY_MAP_MANIFEST").expect("ROSES_STORY_MAP_MANIFEST");
        let mut preview = to_practice_choice(Path::new(&path));
        // Practice Yes hands over to the scripted tutorial (played by the
        // connected game), which resumes the story afterwards.
        assert!(!preview.input(Button::Start).unwrap());
        assert_eq!(
            preview.take_tutorial().as_deref(),
            Some(TUTORIAL_RESUME_STAGE)
        );
        // Starting at the side choice directly keeps seto-side-intro's pan;
        // Up selects the Yorkists: 12000 (Seto, Simon's page 3044) and then
        // map event 100 of the White Rose side. Older imports refuse it.
        let mut side = OpeningPreview::open(Path::new(&path), Some("side-choice"), false).unwrap();
        assert_eq!(side.pan_target(), -100);
        side.input(Button::Up).unwrap();
        match side.input(Button::Start) {
            Err(white) => assert!(white.contains("12000"), "{white}"),
            Ok(_) => {
                assert_eq!(side.model.stage().id(), "seto-yorkists");
                assert_eq!(
                    side.model.campaign().side,
                    Some(roses_client::CampaignSide::Yorkists)
                );
                for _ in 0..27 {
                    side.input(Button::Cross).unwrap();
                }
                assert_eq!(side.take_map_event(), Some(100));
            }
        }
        // After an ending the side choice is locked: Up sounds 6.
        let mut locked = OpeningPreview::open(Path::new(&path), Some("side-choice"), false).unwrap();
        locked.model.lock_selection("side-choice");
        assert!(!locked.input(Button::Up).unwrap());
        assert_eq!((locked.model.selection, locked.take_sounds()), (1, vec![6]));
        // A deck select without a committed name refuses to confirm.
        let mut decks = OpeningPreview::open(Path::new(&path), Some("deck-choice"), false).unwrap();
        assert_eq!(decks.pan_target(), 100);
        assert!(
            decks
                .placeholder()
                .unwrap()
                .contains("committed player name")
        );
        assert!(decks.input(Button::Start).is_err());
        assert!(OpeningPreview::open(Path::new(&path), Some("yugi-at-brest"), false).is_ok());
    }
}
