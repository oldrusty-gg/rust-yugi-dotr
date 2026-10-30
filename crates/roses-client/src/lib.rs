//! Native presentation of an explicitly observed opening flow. No duel logic.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{BufReader, Cursor, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

pub mod audio;
pub mod campaign;
pub mod campaign_map;
pub mod custom_duel;
pub mod map_editor;
pub mod card_face;
pub mod ending;
pub mod game_options;
pub mod duel_hud;
pub mod face_anim;
pub mod gs;
pub mod story_motion;
pub mod memory_card;
pub mod trade;
pub mod monster_pose;
pub mod sound_mix;
#[cfg(windows)]
pub mod movie;
pub mod movie_native;
#[cfg(windows)]
pub mod wave_out;
#[cfg(windows)]
pub mod sound_out;
pub use campaign::{CampaignSide, CampaignState, DeckChoiceConfig, StarterDeck, StarterDeckChoice};

pub const MANIFEST_SCHEMA: &str = "roses-opening-1";
pub type Result<T> = std::result::Result<T, String>;
const MAX_MANIFEST: u64 = 4 * 1024 * 1024;
const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_MEDIA_BYTES: u64 = 512 * 1024 * 1024;
const MAX_PIXELS: usize = 4096 * 4096;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: String,
    pub source: Source,
    pub canvas: Canvas,
    pub assets: Vec<Asset>,
    pub initial_stage: String,
    pub stages: Vec<Stage>,
    /// Named layer lists shared by story pages (original dialogue layouts);
    /// a page draws its set first, then its own layers.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub layer_sets: BTreeMap<String, Vec<Layer>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub profile: String,
    pub disc_sha256: String,
    pub executable_sha256: String,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    pub aspect_width: u32,
    pub aspect_height: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Png,
    Video,
    Wav,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub id: String,
    pub kind: AssetKind,
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}
fn white() -> [u8; 3] {
    [255; 3]
}
fn opaque() -> u8 {
    255
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sampling {
    Nearest,
    #[default]
    Linear,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    pub asset: String,
    #[serde(default)]
    pub source: Option<[u32; 4]>,
    /// Logical-canvas x, y, width, height; clipping is not implicit.
    pub destination: [u32; 4],
    #[serde(default = "white")]
    pub tint: [u8; 3],
    #[serde(default = "opaque")]
    pub opacity: u8,
    #[serde(default)]
    pub flip_x: bool,
    #[serde(default)]
    pub filter: Sampling,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Music {
    pub asset: String,
    pub looped: bool,
    /// Sample frames from the beginning of PCM data; the WAV's end is the loop end.
    #[serde(default)]
    pub loop_start_frame: Option<u32>,
}
/// Adjacent presentation stages may share one continuous music playback.
pub fn music_continues(current: Option<&Music>, next: Option<&Music>) -> bool {
    current.is_some() && current == next
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MenuAction {
    Unavailable,
    Stage {
        target: String,
    },
    /// Explicit boundary: the original continuation is not implemented.
    Unsupported {
        message: String,
    },
    /// The scripted tutorial (practice choice "Yes": practice setup
    /// 0x23B590(0), the three lessons as duel scenes, then the story
    /// resumes at event 10060 with track 25 restarted). A frontend that can
    /// play duels switches to the lessons and comes back to `resume`; one
    /// that cannot reports the boundary.
    Tutorial {
        resume: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MenuEntry {
    pub id: String,
    pub label: String,
    pub normal_layers: Vec<Layer>,
    pub selected_layers: Vec<Layer>,
    pub action: MenuAction,
    /// Confirming this entry records the campaign side (event 10090).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub campaign_side: Option<CampaignSide>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Button {
    Cross,
    Circle,
    Square,
    L1,
    R1,
    Start,
    Up,
    Down,
    Left,
    Right,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoryPage {
    pub id: String,
    /// Shared `Manifest::layer_sets` entry drawn before `layers`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer_set: Option<String>,
    pub layers: Vec<Layer>,
    /// Text that embeds the committed player name, composed at runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_text: Option<NameText>,
    /// Dialogue pan target of byte 0x1ED5439 (−100..=100; big portrait and
    /// scene camera) for 3D frontends. Absent means +100 (Simon, layout 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pan: Option<i8>,
}
/// Original dialogue text with mutable player-name slots (message 3 words).
/// The imported static pages precompose the same layout offline; these pages
/// follow the identical rules at runtime: left 12 columns of each 22x22 font
/// cell, 12-pixel advance, 22-pixel line pitch, then the page destination
/// scale (x 5/4, y 7/8). Unused name slots emit nothing.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NameText {
    /// Font atlas image asset.
    pub font: String,
    /// Canvas origin of the scaled text block.
    pub origin: [u32; 2],
    pub lines: Vec<Vec<TextCell>>,
    /// Committed name code to its narrow font cell origin.
    pub name_cells: Vec<NameCell>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TextCell {
    Glyph { cell: [u32; 2] },
    PlayerName { slot: u8 },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NameCell {
    pub stored_code: u16,
    pub cell: [u32; 2],
}
const TEXT_CELL: u32 = 22;
const TEXT_ADVANCE: u32 = 12;
impl NameText {
    /// Compose the raw text image and its canvas layer for `name`.
    pub fn compose(&self, font: &Image, name: Option<&PlayerName>) -> Result<(Image, Layer)> {
        let mut rows: Vec<Vec<[u32; 2]>> = Vec::with_capacity(self.lines.len());
        for line in &self.lines {
            let mut cells = Vec::new();
            for cell in line {
                match cell {
                    TextCell::Glyph { cell } => cells.push(*cell),
                    TextCell::PlayerName { slot } => {
                        let name = name.ok_or("Page needs a committed player name.")?;
                        if let Some(glyph) = name.glyphs.get(usize::from(*slot)) {
                            let found = self
                                .name_cells
                                .iter()
                                .find(|c| c.stored_code == glyph.stored_code)
                                .ok_or("Name glyph has no imported dialogue font cell.")?;
                            cells.push(found.cell);
                        }
                    }
                }
            }
            rows.push(cells);
        }
        let columns = rows.iter().map(Vec::len).max().unwrap_or(0) as u32;
        if rows.is_empty() || rows.len() > 3 || columns == 0 || columns > 30 {
            return Err("Name text exceeds the observed three-line, thirty-glyph area.".into());
        }
        let width = columns * TEXT_ADVANCE;
        let height = rows.len() as u32 * TEXT_CELL;
        let mut rgba = vec![0u8; (width * height * 4) as usize];
        for (row, cells) in rows.iter().enumerate() {
            for (column, cell) in cells.iter().enumerate() {
                if cell[0] + TEXT_CELL > font.width || cell[1] + TEXT_CELL > font.height {
                    return Err("Name text cell exceeds its font atlas.".into());
                }
                for y in 0..TEXT_CELL {
                    let from = (((cell[1] + y) * font.width + cell[0]) * 4) as usize;
                    let to = (((row as u32 * TEXT_CELL + y) * width + column as u32 * TEXT_ADVANCE)
                        * 4) as usize;
                    let span = (TEXT_ADVANCE * 4) as usize;
                    rgba[to..to + span].copy_from_slice(&font.rgba[from..from + span]);
                }
            }
        }
        let layer = Layer {
            asset: self.font.clone(),
            source: None,
            destination: [
                self.origin[0],
                self.origin[1],
                width * 5 / 4,
                (height * 7 + 4) / 8,
            ],
            tint: [255; 3],
            opacity: 255,
            flip_x: false,
            filter: Sampling::Linear,
        };
        Ok((
            Image {
                width,
                height,
                rgba,
            },
            layer,
        ))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum StoryEnd {
    Stage {
        target: String,
    },
    Unsupported {
        message: String,
    },
    /// The pages of a campaign-map story event end: the map resumes at
    /// this map event (`campaign_map::MapModel::resume`; 11124 after the
    /// Bosworth trigger, 108 after the Bosworth win, 112 after the Seto win).
    MapEvent {
        event: u16,
    },
}
/// Imported keyboard cell. Display and committed codes are deliberately distinct.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NameEntryGlyph {
    pub display_code: u16,
    pub stored_code: u16,
    pub asset: String,
    pub source: [u32; 4],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NameEntryPage {
    pub layers: Vec<Layer>,
    /// Row-major cells; newline tokens are not cells.
    pub cells: Vec<NameEntryGlyph>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NameEntryConfig {
    pub rows: u8,
    pub columns: u8,
    pub max_length: usize,
    pub pages: Vec<NameEntryPage>,
    /// Sprite whose destination is replaced by the selected cell's rectangle.
    pub cursor: Layer,
    pub cursor_rects: Vec<[u32; 4]>,
    pub name_positions: Vec<[u32; 4]>,
    #[serde(default = "white")]
    pub name_tint: [u8; 3],
    #[serde(default)]
    pub name_shadow: Option<NameShadow>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NameShadow {
    pub offset: [i32; 2],
    pub tint: [u8; 3],
    pub opacity: u8,
}
fn offset_rect(rect: [u32; 4], offset: [i32; 2]) -> Option<[u32; 4]> {
    Some([
        rect[0].checked_add_signed(offset[0])?,
        rect[1].checked_add_signed(offset[1])?,
        rect[2],
        rect[3],
    ])
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NameCharacter {
    pub page: u8,
    pub row: u8,
    pub column: u8,
    /// Raw glyph identifiers without message terminator flags.
    pub display_code: u16,
    pub stored_code: u16,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerName {
    pub glyphs: Vec<NameCharacter>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NameEntryState {
    page: usize,
    row: usize,
    column: usize,
    glyphs: Vec<NameCharacter>,
    committed: bool,
}
impl NameEntryState {
    pub fn page(&self) -> usize {
        self.page
    }
    pub fn row(&self) -> usize {
        self.row
    }
    pub fn column(&self) -> usize {
        self.column
    }
    pub fn glyphs(&self) -> &[NameCharacter] {
        &self.glyphs
    }
    pub fn committed(&self) -> bool {
        self.committed
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Stage {
    NameEntry {
        id: String,
        layers: Vec<Layer>,
        #[serde(default)]
        music: Option<Music>,
        config: NameEntryConfig,
        on_complete: StoryEnd,
    },
    Story {
        id: String,
        pages: Vec<StoryPage>,
        #[serde(default)]
        music: Option<Music>,
        advance_buttons: Vec<Button>,
        on_end: StoryEnd,
    },
    Movie {
        id: String,
        asset: String,
        on_end: String,
        #[serde(default)]
        skip_buttons: Vec<Button>,
        #[serde(default)]
        on_skip: Option<String>,
    },
    Screen {
        id: String,
        layers: Vec<Layer>,
        #[serde(default)]
        music: Option<Music>,
        #[serde(default)]
        advance_buttons: Vec<Button>,
        #[serde(default)]
        duration_ms: Option<u32>,
        next: String,
    },
    Menu {
        id: String,
        layers: Vec<Layer>,
        #[serde(default)]
        music: Option<Music>,
        entries: Vec<MenuEntry>,
        initial_selection: usize,
        confirm_buttons: Vec<Button>,
        #[serde(default)]
        cancel_target: Option<String>,
        #[serde(default)]
        wrap_navigation: bool,
    },
    /// Starter-deck select (event 10070): three candidates computed from the
    /// committed name; Left/Right wrap, confirm records the campaign deck.
    DeckChoice {
        id: String,
        layers: Vec<Layer>,
        #[serde(default)]
        music: Option<Music>,
        config: DeckChoiceConfig,
        confirm_buttons: Vec<Button>,
        on_confirm: StoryEnd,
    },
    /// Explicit stand-in for an untraced original segment: no original art is
    /// drawn and frontends must show `message`; advance buttons continue.
    Placeholder {
        id: String,
        message: String,
        advance_buttons: Vec<Button>,
        next: StoryEnd,
    },
}
impl Stage {
    pub fn id(&self) -> &str {
        match self {
            Self::Movie { id, .. }
            | Self::Screen { id, .. }
            | Self::Menu { id, .. }
            | Self::NameEntry { id, .. }
            | Self::Story { id, .. }
            | Self::DeckChoice { id, .. }
            | Self::Placeholder { id, .. } => id,
        }
    }
    pub fn music(&self) -> Option<&Music> {
        match self {
            Self::Screen { music, .. }
            | Self::Menu { music, .. }
            | Self::Story { music, .. }
            | Self::DeckChoice { music, .. }
            | Self::NameEntry { music, .. } => music.as_ref(),
            _ => None,
        }
    }
    pub fn layers(&self) -> &[Layer] {
        match self {
            Self::Screen { layers, .. }
            | Self::Menu { layers, .. }
            | Self::DeckChoice { layers, .. }
            | Self::NameEntry { layers, .. } => layers,
            _ => &[],
        }
    }
    pub fn primary_button(&self) -> Option<Button> {
        match self {
            Self::NameEntry { .. } => Some(Button::Cross),
            Self::Movie { skip_buttons, .. } => skip_buttons.first().copied(),
            Self::Screen {
                advance_buttons, ..
            }
            | Self::Story {
                advance_buttons, ..
            }
            | Self::Placeholder {
                advance_buttons, ..
            } => advance_buttons.first().copied(),
            Self::Menu {
                confirm_buttons, ..
            }
            | Self::DeckChoice {
                confirm_buttons, ..
            } => confirm_buttons.first().copied(),
        }
    }
}
fn hash_valid(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|c| c.is_ascii_hexdigit())
}
fn id_valid(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
}
fn within(rect: [u32; 4], width: u32, height: u32) -> bool {
    rect[2] > 0
        && rect[3] > 0
        && rect[0].checked_add(rect[2]).is_some_and(|x| x <= width)
        && rect[1].checked_add(rect[3]).is_some_and(|y| y <= height)
}
pub fn validate_manifest(manifest: &Manifest) -> Result<()> {
    let fail = |s: &str| Err(s.to_string());
    if manifest.schema_version != MANIFEST_SCHEMA
        || manifest.source.profile != "dotr-europe-sles-51579"
    {
        return fail("Unsupported opening manifest/profile.");
    }
    if !hash_valid(&manifest.source.disc_sha256) || !hash_valid(&manifest.source.executable_sha256)
    {
        return fail("Invalid source SHA256 metadata.");
    }
    let c = manifest.canvas;
    if !(16..=2048).contains(&c.width)
        || !(16..=2048).contains(&c.height)
        || !(1..=4096).contains(&c.aspect_width)
        || !(1..=4096).contains(&c.aspect_height)
    {
        return fail("Unsupported canvas or display aspect.");
    }
    if manifest.assets.is_empty()
        || manifest.assets.len() > 512
        || manifest.stages.is_empty()
        || manifest.stages.len() > 64
        || manifest.layer_sets.len() > 64
    {
        return fail("Opening manifest asset/stage count is out of bounds.");
    }
    let mut assets = BTreeMap::new();
    for asset in &manifest.assets {
        if !id_valid(&asset.id)
            || assets.insert(asset.id.as_str(), asset).is_some()
            || !hash_valid(&asset.sha256)
            || asset.bytes == 0
            || asset.bytes > MAX_MEDIA_BYTES
        {
            return fail("Invalid/duplicate asset metadata.");
        }
        let path = Path::new(&asset.path);
        if asset.path.is_empty()
            || asset.path.len() > 240
            || asset.path.contains([':', '\0'])
            || path
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
        {
            return fail(
                "Asset paths must be relative child paths, without traversal or alternate streams.",
            );
        }
        if asset.kind == AssetKind::Png && asset.bytes > MAX_IMAGE_BYTES {
            return fail("PNG file exceeds the image limit.");
        }
    }
    let ids: BTreeSet<_> = manifest.stages.iter().map(Stage::id).collect();
    if ids.len() != manifest.stages.len()
        || !ids.contains(manifest.initial_stage.as_str())
        || ids.iter().any(|id| !id_valid(id))
    {
        return fail("Invalid/duplicate stage IDs or missing initial stage.");
    }
    let target = |id: &str| -> Result<()> {
        if ids.contains(id) {
            Ok(())
        } else {
            Err(format!("Unknown stage {id}."))
        }
    };
    let asset_kind = |id: &str, kind: AssetKind| -> Result<()> {
        if assets.get(id).is_some_and(|a| a.kind == kind) {
            Ok(())
        } else {
            Err(format!("Unknown asset or wrong kind: {id}"))
        }
    };
    let buttons = |set: &[Button]| -> Result<()> {
        if set.len() <= 10 && set.iter().copied().collect::<BTreeSet<_>>().len() == set.len() {
            Ok(())
        } else {
            Err("Duplicate/invalid buttons.".into())
        }
    };
    let layers = |ls: &[Layer]| -> Result<()> {
        if ls.len() > 128 {
            return Err("Too many layers.".into());
        }
        for layer in ls {
            asset_kind(&layer.asset, AssetKind::Png)?;
            if !within(layer.destination, c.width, c.height) {
                return Err("Layer destination exceeds the logical canvas.".into());
            }
        }
        Ok(())
    };
    let ending = |end: &StoryEnd| -> Result<()> {
        match end {
            StoryEnd::Stage { target: next } => target(next),
            StoryEnd::Unsupported { message } => {
                if message.trim().is_empty()
                    || message.len() > 512
                    || message.chars().any(char::is_control)
                {
                    return fail("Unsupported boundary requires a concise status message.");
                }
                Ok(())
            }
            // 0 = the free map (12132 ends with C2 = 0 at 0x2223D0).
            StoryEnd::MapEvent { event } => {
                if *event >= 20000 {
                    return fail("Map event continuation must be a map event id.");
                }
                Ok(())
            }
        }
    };
    for (name, set) in &manifest.layer_sets {
        if !id_valid(name) || set.is_empty() {
            return fail("Layer sets require valid names and at least one layer.");
        }
        layers(set)?;
    }
    for stage in &manifest.stages {
        layers(stage.layers())?;
        if let Some(music) = stage.music() {
            asset_kind(&music.asset, AssetKind::Wav)?;
            if music.loop_start_frame.is_some() && !music.looped {
                return Err("An explicit music loop start requires looped playback.".into());
            }
        }
        match stage {
            Stage::NameEntry {
                config,
                on_complete,
                ..
            } => {
                if config.rows == 0
                    || config.rows > 8
                    || config.columns == 0
                    || config.columns > 16
                    || config.max_length == 0
                    || config.max_length > 12
                    || config.pages.is_empty()
                    || config.pages.len() > 4
                {
                    return fail("Name entry dimensions, pages or length exceed supported bounds.");
                }
                let cell_count = usize::from(config.rows) * usize::from(config.columns);
                if config.cursor_rects.len() != cell_count
                    || config.name_positions.len() != config.max_length
                {
                    return fail(
                        "Name entry requires complete cursor and name position rectangles.",
                    );
                }
                for &rect in config.cursor_rects.iter().chain(&config.name_positions) {
                    if !within(rect, c.width, c.height) {
                        return fail("Name entry rectangle exceeds the logical canvas.");
                    }
                }
                if let Some(shadow) = &config.name_shadow {
                    for &rect in &config.name_positions {
                        if !offset_rect(rect, shadow.offset)
                            .is_some_and(|rect| within(rect, c.width, c.height))
                        {
                            return fail("Name entry shadow exceeds the logical canvas.");
                        }
                    }
                }
                layers(std::slice::from_ref(&config.cursor))?;
                for page in &config.pages {
                    layers(&page.layers)?;
                    if page.cells.len() != cell_count {
                        return fail("Name entry page has an incomplete keyboard grid.");
                    }
                    for glyph in &page.cells {
                        if glyph.display_code == 0
                            || glyph.display_code >= 0x4000
                            || glyph.stored_code == 0
                            || glyph.stored_code >= 0x4000
                            || glyph.display_code == 0x2000
                            || glyph.stored_code == 0x2000
                        {
                            return fail("Name entry glyphs must be direct, non-newline codes without flags.");
                        }
                        asset_kind(&glyph.asset, AssetKind::Png)?;
                        if !within(glyph.source, 4096, 4096) {
                            return fail("Name entry glyph crop exceeds image limits.");
                        }
                    }
                }
                ending(on_complete)?;
            }
            Stage::Story {
                pages,
                advance_buttons,
                on_end,
                ..
            } => {
                if pages.is_empty() || pages.len() > 128 || advance_buttons.is_empty() {
                    return fail("Story requires 1..128 pages and explicit advance buttons.");
                }
                buttons(advance_buttons)?;
                let mut page_ids = BTreeSet::new();
                for page in pages {
                    let set = match &page.layer_set {
                        Some(name) => Some(
                            manifest
                                .layer_sets
                                .get(name)
                                .ok_or(format!("Unknown layer set {name}."))?,
                        ),
                        None => None,
                    };
                    if !id_valid(&page.id)
                        || !page_ids.insert(&page.id)
                        || (page.layers.is_empty() && set.is_none())
                    {
                        return fail(
                            "Story pages require unique valid IDs and at least one layer.",
                        );
                    }
                    if page.pan.is_some_and(|pan| !(-100..=100).contains(&pan)) {
                        return fail("Story page pan must lie in -100..=100.");
                    }
                    layers(&page.layers)?;
                }
                ending(on_end)?;
            }
            Stage::DeckChoice {
                config,
                confirm_buttons,
                on_confirm,
                ..
            } => {
                config.validate()?;
                for deck in &config.decks {
                    layers(&deck.presentation_layers)?;
                    if let Some(face) = &deck.card_face {
                        asset_kind(face, AssetKind::Png)?;
                    }
                }
                buttons(confirm_buttons)?;
                if confirm_buttons.is_empty()
                    || confirm_buttons
                        .iter()
                        .any(|b| matches!(b, Button::Left | Button::Right))
                {
                    return fail("Deck choice confirmation conflicts with navigation.");
                }
                ending(on_confirm)?;
            }
            Stage::Placeholder {
                message,
                advance_buttons,
                next,
                ..
            } => {
                buttons(advance_buttons)?;
                if advance_buttons.is_empty() {
                    return fail("Placeholder requires explicit advance buttons.");
                }
                ending(&StoryEnd::Unsupported {
                    message: message.clone(),
                })?;
                ending(next)?;
            }
            Stage::Movie {
                asset,
                on_end,
                skip_buttons,
                on_skip,
                ..
            } => {
                asset_kind(asset, AssetKind::Video)?;
                target(on_end)?;
                buttons(skip_buttons)?;
                if skip_buttons.is_empty() != on_skip.is_none() {
                    return fail("Movie skip buttons and target must be provided together.");
                }
                if let Some(t) = on_skip {
                    target(t)?;
                }
            }
            Stage::Screen {
                advance_buttons,
                duration_ms,
                next,
                ..
            } => {
                buttons(advance_buttons)?;
                target(next)?;
                if advance_buttons.is_empty() && duration_ms.is_none() {
                    return fail("Screen has no observed exit.");
                }
                if duration_ms.is_some_and(|t| t == 0 || t > 600_000) {
                    return fail("Timed screen must last 1..600000 ms.");
                }
            }
            Stage::Menu {
                entries,
                initial_selection,
                confirm_buttons,
                cancel_target,
                ..
            } => {
                if entries.is_empty()
                    || entries.len() > 16
                    || *initial_selection >= entries.len()
                    || confirm_buttons.is_empty()
                {
                    return fail("Invalid menu entries, selection or confirmation buttons.");
                }
                buttons(confirm_buttons)?;
                if confirm_buttons
                    .iter()
                    .any(|button| matches!(button, Button::Up | Button::Down))
                    || (cancel_target.is_some() && confirm_buttons.contains(&Button::Circle))
                {
                    return fail("Menu confirmation conflicts with navigation or cancellation.");
                }
                if let Some(t) = cancel_target {
                    target(t)?;
                }
                let mut names = BTreeSet::new();
                for e in entries {
                    if !id_valid(&e.id)
                        || !names.insert(&e.id)
                        || e.label.trim().is_empty()
                        || e.label.len() > 160
                        || e.label.chars().any(char::is_control)
                    {
                        return fail("Invalid/duplicate menu entry.");
                    }
                    layers(&e.normal_layers)?;
                    layers(&e.selected_layers)?;
                    match &e.action {
                        MenuAction::Stage { target: t } => target(t)?,
                        MenuAction::Unsupported { message } => ending(&StoryEnd::Unsupported {
                            message: message.clone(),
                        })?,
                        MenuAction::Tutorial { resume } => target(resume)?,
                        MenuAction::Unavailable => (),
                    }
                }
            }
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
pub struct LoadedAsset {
    pub path: PathBuf,
    pub image: Option<Image>,
    /// Exact hash-validated PCM, retained only for explicit loop-region playback.
    pub pcm16: Option<Arc<audio::Pcm16Wave>>,
    _read_lock: Option<File>,
}
pub struct Loaded {
    pub manifest: Manifest,
    pub manifest_sha256: String,
    pub assets: BTreeMap<String, LoadedAsset>,
}
fn locked_file(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1);
    }
    options
        .open(path)
        .map_err(|e| format!("Cannot open {}: {e}", path.display()))
}
impl Loaded {
    pub fn open(manifest_path: &Path) -> Result<Self> {
        let canonical = manifest_path.canonicalize().map_err(|e| e.to_string())?;
        let root = canonical
            .parent()
            .ok_or("Manifest has no parent directory.")?;
        let mut bytes = Vec::new();
        locked_file(&canonical)?
            .take(MAX_MANIFEST + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_MANIFEST {
            return Err(format!("Manifest exceeds {} MiB.", MAX_MANIFEST >> 20));
        }
        let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        validate_manifest(&manifest)?;
        let mut assets = BTreeMap::new();
        let mut decoded_bytes = 0usize;
        let mut pcm_bytes = 0usize;
        for asset in &manifest.assets {
            let needs_pcm = manifest
                .stages
                .iter()
                .filter_map(Stage::music)
                .any(|music| music.asset == asset.id && music.loop_start_frame.is_some());
            if needs_pcm {
                pcm_bytes = pcm_bytes
                    .checked_add(
                        usize::try_from(asset.bytes).map_err(|_| "PCM file is too large.")?,
                    )
                    .filter(|total| *total <= audio::MAX_PCM_WAV_BYTES)
                    .ok_or("Explicit loop WAV assets exceed the 128 MiB PCM budget.")?;
            }
            let path = root
                .join(&asset.path)
                .canonicalize()
                .map_err(|e| format!("{}: {e}", asset.path))?;
            if !path.starts_with(root) {
                return Err("Asset resolves outside the manifest directory.".into());
            }
            let mut file = locked_file(&path)?;
            if file.metadata().map_err(|e| e.to_string())?.len() != asset.bytes {
                return Err(format!("Asset size mismatch: {}", asset.id));
            }
            let mut digest = Sha256::new();
            let mut buffer = [0u8; 65536];
            let mut count = 0u64;
            let mut image_bytes = Vec::new();
            let mut prefix = Vec::new();
            loop {
                let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                count += n as u64;
                if count > asset.bytes {
                    return Err("Asset grew during validation.".into());
                }
                digest.update(&buffer[..n]);
                if prefix.len() < 16 {
                    prefix.extend_from_slice(&buffer[..n.min(16 - prefix.len())]);
                }
                if asset.kind == AssetKind::Png || needs_pcm {
                    image_bytes.extend_from_slice(&buffer[..n]);
                }
            }
            if count != asset.bytes
                || format!("{:x}", digest.finalize()) != asset.sha256.to_ascii_lowercase()
            {
                return Err(format!("Asset SHA256 mismatch: {}", asset.id));
            }
            if asset.kind == AssetKind::Wav
                && (prefix.get(..4) != Some(b"RIFF") || prefix.get(8..12) != Some(b"WAVE"))
            {
                return Err("Audio asset is not a RIFF/WAVE file.".into());
            }
            let image = if asset.kind == AssetKind::Png {
                let image = decode_png(&image_bytes)?;
                decoded_bytes += image.rgba.len();
                if decoded_bytes > 256 * 1024 * 1024 {
                    return Err("Decoded image budget exceeded.".into());
                }
                Some(image)
            } else {
                None
            };
            let pcm16 = if needs_pcm {
                Some(Arc::new(audio::Pcm16Wave::parse(&image_bytes)?))
            } else {
                None
            };
            assets.insert(
                asset.id.clone(),
                LoadedAsset {
                    path,
                    image,
                    pcm16,
                    _read_lock: if asset.kind == AssetKind::Png {
                        None
                    } else {
                        Some(file)
                    },
                },
            );
        }
        let loaded = Self {
            manifest,
            manifest_sha256: format!("{:x}", Sha256::digest(&bytes)),
            assets,
        };
        for layer in loaded.manifest.layer_sets.values().flatten() {
            let image = loaded.assets[&layer.asset]
                .image
                .as_ref()
                .ok_or("Layer asset is not an image.")?;
            if layer
                .source
                .is_some_and(|rect| !within(rect, image.width, image.height))
            {
                return Err("Layer crop exceeds the source PNG.".into());
            }
        }
        for stage in &loaded.manifest.stages {
            if let Some(music) = stage.music() {
                if let Some(start) = music.loop_start_frame {
                    loaded.assets[&music.asset]
                        .pcm16
                        .as_ref()
                        .ok_or("Explicit loop music has no validated PCM data.")?
                        .loop_regions(start)?;
                }
            }
            let mut layers = stage.layers().to_vec();
            if let Stage::DeckChoice { config, .. } = stage {
                layers.extend(
                    config
                        .decks
                        .iter()
                        .flat_map(|deck| deck.presentation_layers.iter())
                        .cloned(),
                );
                layers.extend(config.decks.iter().filter_map(|deck| {
                    deck.card_face.as_ref().map(|asset| Layer {
                        asset: asset.clone(),
                        source: None,
                        destination: campaign::STACK_RECTS[0],
                        tint: [255; 3],
                        opacity: 255,
                        flip_x: false,
                        filter: Sampling::Linear,
                    })
                }));
            }
            if let Stage::Menu { entries, .. } = stage {
                for e in entries {
                    layers.extend(e.normal_layers.clone());
                    layers.extend(e.selected_layers.clone());
                }
            }
            if let Stage::Story { pages, .. } = stage {
                for page in pages {
                    layers.extend(page.layers.clone());
                }
            }
            if let Stage::NameEntry { config, .. } = stage {
                layers.push(config.cursor.clone());
                for page in &config.pages {
                    layers.extend(page.layers.clone());
                    for glyph in &page.cells {
                        let image = loaded.assets[&glyph.asset]
                            .image
                            .as_ref()
                            .ok_or("Name glyph asset is not an image.")?;
                        if !within(glyph.source, image.width, image.height) {
                            return Err("Name glyph crop exceeds the source PNG.".into());
                        }
                    }
                }
            }
            for layer in layers {
                let image = loaded.assets[&layer.asset]
                    .image
                    .as_ref()
                    .ok_or("Layer asset is not an image.")?;
                if let Some(rect) = layer.source {
                    if !within(rect, image.width, image.height) {
                        return Err("Layer crop exceeds the source PNG.".into());
                    }
                }
            }
        }
        Ok(loaded)
    }
    pub fn render(&self, model: &Model) -> Result<Vec<u8>> {
        let c = self.manifest.canvas;
        let mut rgba = vec![0u8; c.width as usize * c.height as usize * 4];
        for p in rgba.chunks_exact_mut(4) {
            p[3] = 255;
        }
        for layer in self.active_layers(model)? {
            let image = self.assets[&layer.asset].image.as_ref().unwrap();
            composite(&mut rgba, c.width, image, &layer);
        }
        if let Some((image, layer)) = self.name_text(model)? {
            composite(&mut rgba, c.width, &image, &layer);
        }
        // GDI BI_RGB expects opaque BGRX; alpha composition was completed above.
        for pixel in rgba.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        Ok(rgba)
    }

    /// Render the validated presentation over transparent black for a 3D frontend.
    /// RGB is premultiplied by alpha in the original byte color space. Composite
    /// it as `overlay.rgb + background.rgb * (1 - overlay.a)` in that same space;
    /// do not apply a second source-alpha multiplication or an sRGB decode.
    pub fn render_overlay(&self, model: &Model) -> Result<Image> {
        self.render_overlay_with(model, None)
    }

    /// `render_overlay` with the story stage's motion
    /// (`story_motion::StoryMotion`) for this update, when there is one.
    pub fn render_overlay_with(
        &self,
        model: &Model,
        motion: Option<&story_motion::StoryMotion>,
    ) -> Result<Image> {
        self.render_overlay_faces(model, motion, None)
    }

    /// `render_overlay_with` with the story panels' animated faces
    /// (`face_anim::StoryFaces`, stepped by the frontend per update).
    pub fn render_overlay_faces(
        &self,
        model: &Model,
        motion: Option<&story_motion::StoryMotion>,
        faces: Option<&face_anim::StoryFaces>,
    ) -> Result<Image> {
        let c = self.manifest.canvas;
        let mut rgba = vec![0u8; c.width as usize * c.height as usize * 4];
        let motion = motion.filter(|m| m.stage() == model.stage().id());
        for layer in self.active_layers_faces(model, motion, faces)? {
            let image = self.assets[&layer.asset].image.as_ref().unwrap();
            composite(&mut rgba, c.width, image, &layer);
        }
        if let Some((image, layer)) = self.name_text(model)? {
            if motion.is_none_or(|m| m.name_text_visible()) {
                composite(&mut rgba, c.width, &image, &layer);
            }
        }
        Ok(Image {
            width: c.width,
            height: c.height,
            rgba,
        })
    }

    /// Runtime text for the current story page, if it embeds the player name.
    pub fn name_text(&self, model: &Model) -> Result<Option<(Image, Layer)>> {
        let Some(Stage::Story { pages, .. }) = self
            .manifest
            .stages
            .iter()
            .find(|stage| stage.id() == model.stage().id())
        else {
            return Ok(None);
        };
        let Some(text) = pages
            .get(model.page_index)
            .and_then(|page| page.name_text.as_ref())
        else {
            return Ok(None);
        };
        let font = self
            .assets
            .get(&text.font)
            .and_then(|a| a.image.as_ref())
            .ok_or("Name text font is not an image asset.")?;
        let (image, layer) = text.compose(font, model.player_name())?;
        let c = self.manifest.canvas;
        if !within(layer.destination, c.width, c.height) {
            return Err("Name text lies outside the canvas.".into());
        }
        Ok(Some((image, layer)))
    }

    /// Resolve the current page, menu selection or name-entry glyphs against the
    /// loaded manifest. Frontends share this ordering and these bounds checks.
    pub fn active_layers(&self, model: &Model) -> Result<Vec<Layer>> {
        self.active_layers_with(model, None)
    }

    /// `active_layers` with a story stage's motion: the page's settled set
    /// and text are moved, re-cut and revealed for the motion's update.
    pub fn active_layers_with(
        &self,
        model: &Model,
        motion: Option<&story_motion::StoryMotion>,
    ) -> Result<Vec<Layer>> {
        self.active_layers_faces(model, motion, None)
    }

    /// The settled layers of the current story page (its set, then its own
    /// layers) as the face animation reads them.
    pub fn story_page_layers(&self, model: &Model) -> Option<Vec<Layer>> {
        let Stage::Story { pages, .. } = self.manifest.stages.iter().find(|s| s.id() == model.stage().id())? else {
            return None;
        };
        let page = pages.get(model.page_index)?;
        let mut layers = page
            .layer_set
            .as_ref()
            .and_then(|n| self.manifest.layer_sets.get(n))
            .cloned()
            .unwrap_or_default();
        layers.extend(page.layers.iter().cloned());
        Some(layers)
    }

    /// `active_layers_with` with the panels' animated faces.
    pub fn active_layers_faces(
        &self,
        model: &Model,
        motion: Option<&story_motion::StoryMotion>,
        faces: Option<&face_anim::StoryFaces>,
    ) -> Result<Vec<Layer>> {
        let c = self.manifest.canvas;
        let stage = self
            .manifest
            .stages
            .iter()
            .find(|stage| stage.id() == model.stage().id())
            .ok_or("Model stage does not belong to the loaded manifest.")?;
        if let Stage::Menu { entries, .. } = stage {
            if model.selection >= entries.len() {
                return Err("Model selection exceeds the loaded menu.".into());
            }
        }
        if matches!(stage, Stage::DeckChoice { .. }) && model.selection >= campaign::CANDIDATES {
            return Err("Model selection exceeds the deck candidates.".into());
        }
        let mut layers: Vec<Layer> = stage.layers().to_vec();
        if let Stage::DeckChoice { config, .. } = stage {
            let records = config.candidates(
                model
                    .player_name()
                    .ok_or("Deck choice needs a committed player name.")?,
            )?;
            let deck = config
                .deck(records[model.selection])
                .ok_or("Deck candidate record is missing.")?;
            layers.extend(deck.presentation_layers.iter().cloned());
            // The stack is drawn last (0x229D9C after the panel 0x224B90).
            let decks: [&campaign::StarterDeck; campaign::CANDIDATES] = std::array::from_fn(|k| {
                config.deck(records[k]).expect("validated candidate record")
            });
            if let Some(stack) = campaign::stack_layers(decks, model.selection) {
                layers.extend(stack);
            }
        }
        if let Stage::Story { pages, .. } = stage {
            let page = pages
                .get(model.page_index)
                .ok_or("Model page exceeds the loaded story.")?;
            let set: &[Layer] = match &page.layer_set {
                Some(name) => self
                    .manifest
                    .layer_sets
                    .get(name)
                    .ok_or("Story page references a missing layer set.")?,
                None => &[],
            };
            // The panels' faces at this update's frames.
            let animated = faces.map(|f| (f.apply(set), f.apply(&page.layers)));
            let (set, page_layers): (&[Layer], &[Layer]) = match &animated {
                Some((set, own)) => (set, own),
                None => (set, &page.layers),
            };
            match motion.filter(|m| m.stage() == stage.id()) {
                Some(motion) => {
                    let dims = |id: &str| {
                        self.assets
                            .get(id)
                            .and_then(|a| a.image.as_ref())
                            .map(|i| [i.width, i.height])
                    };
                    layers.extend(motion.layers(set, page_layers, &dims));
                }
                None => {
                    layers.extend(set.iter().cloned());
                    layers.extend(page_layers.iter().cloned());
                }
            }
        }
        if let Stage::Menu { entries, .. } = stage {
            for (i, entry) in entries.iter().enumerate() {
                layers.extend(
                    if i == model.selection {
                        &entry.selected_layers
                    } else {
                        &entry.normal_layers
                    }
                    .iter()
                    .cloned(),
                );
            }
        }
        if let Stage::NameEntry { config, .. } = stage {
            let state = model.name_entry().ok_or("Name entry state is missing.")?;
            let page = config
                .pages
                .get(state.page)
                .ok_or("Name entry page is invalid.")?;
            if state.row >= usize::from(config.rows)
                || state.column >= usize::from(config.columns)
                || state.glyphs.len() > config.max_length
            {
                return Err("Name entry cursor or length is invalid.".into());
            }
            layers.extend(page.layers.clone());
            let mut cursor = config.cursor.clone();
            cursor.destination =
                config.cursor_rects[state.row * usize::from(config.columns) + state.column];
            layers.push(cursor);
            for (position, character) in state.glyphs.iter().enumerate() {
                if usize::from(character.row) >= usize::from(config.rows)
                    || usize::from(character.column) >= usize::from(config.columns)
                {
                    return Err("Entered name contains an invalid grid coordinate.".into());
                }
                let glyph = config
                    .pages
                    .get(usize::from(character.page))
                    .and_then(|page| {
                        page.cells.get(
                            usize::from(character.row) * usize::from(config.columns)
                                + usize::from(character.column),
                        )
                    })
                    .ok_or("Entered name references a missing glyph.")?;
                if glyph.display_code != character.display_code
                    || glyph.stored_code != character.stored_code
                {
                    return Err(
                        "Entered name glyph does not match its imported keyboard cell.".into(),
                    );
                }
                let glyph_layer = Layer {
                    asset: glyph.asset.clone(),
                    source: Some(glyph.source),
                    destination: config.name_positions[position],
                    tint: config.name_tint,
                    opacity: 255,
                    flip_x: false,
                    filter: Sampling::Linear,
                };
                if let Some(shadow) = &config.name_shadow {
                    let mut shadow_layer = glyph_layer.clone();
                    shadow_layer.destination = offset_rect(glyph_layer.destination, shadow.offset)
                        .ok_or("Name shadow coordinates overflow.")?;
                    shadow_layer.tint = shadow.tint;
                    shadow_layer.opacity = shadow.opacity;
                    layers.push(shadow_layer);
                }
                layers.push(glyph_layer);
            }
        }
        for layer in &layers {
            let image = self
                .assets
                .get(&layer.asset)
                .and_then(|a| a.image.as_ref())
                .ok_or("Missing decoded layer.")?;
            // Also protect callers constructing Loaded or Model values directly.
            if !within(layer.destination, c.width, c.height)
                || layer
                    .source
                    .is_some_and(|rect| !within(rect, image.width, image.height))
            {
                return Err("Render layer rectangle is outside its validated bounds.".into());
            }
        }
        Ok(layers)
    }
}
pub fn decode_png(bytes: &[u8]) -> Result<Image> {
    let mut decoder = png::Decoder::new(BufReader::new(Cursor::new(bytes)));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let info = reader.info();
    if info.width == 0
        || info.height == 0
        || info.width > 4096
        || info.height > 4096
        || info.width as usize * info.height as usize > MAX_PIXELS
    {
        return Err("PNG dimensions exceed the image limit.".into());
    }
    let mut buffer = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or("PNG buffer size overflow.")?
    ];
    let info = reader.next_frame(&mut buffer).map_err(|e| e.to_string())?;
    let bytes = &buffer[..info.buffer_size()];
    let mut rgba = Vec::with_capacity(info.width as usize * info.height as usize * 4);
    match info.color_type {
        png::ColorType::Rgba => rgba.extend_from_slice(bytes),
        png::ColorType::Rgb => {
            for p in bytes.chunks_exact(3) {
                rgba.extend_from_slice(&[p[0], p[1], p[2], 255]);
            }
        }
        png::ColorType::Grayscale => {
            for &v in bytes {
                rgba.extend_from_slice(&[v, v, v, 255]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for p in bytes.chunks_exact(2) {
                rgba.extend_from_slice(&[p[0], p[0], p[0], p[1]]);
            }
        }
        _ => return Err("Unsupported expanded PNG format.".into()),
    }
    Ok(Image {
        width: info.width,
        height: info.height,
        rgba,
    })
}
fn composite(frame: &mut [u8], width: u32, image: &Image, layer: &Layer) {
    let [sx, sy, sw, sh] = layer.source.unwrap_or([0, 0, image.width, image.height]);
    let [dx, dy, dw, dh] = layer.destination;
    // Clamp interpolation to the explicit crop so adjacent atlas sprites never bleed.
    let sample = |x: u32, y: u32| -> [f64; 4] {
        let i = (((sy + y) * image.width + sx + x) * 4) as usize;
        let a = f64::from(image.rgba[i + 3]) / 255.0;
        [
            f64::from(image.rgba[i]) * a,
            f64::from(image.rgba[i + 1]) * a,
            f64::from(image.rgba[i + 2]) * a,
            a,
        ]
    };
    for y in 0..dh {
        for x in 0..dw {
            let mut fx = ((f64::from(x) + 0.5) * f64::from(sw) / f64::from(dw) - 0.5)
                .clamp(0.0, f64::from(sw - 1));
            let fy = ((f64::from(y) + 0.5) * f64::from(sh) / f64::from(dh) - 0.5)
                .clamp(0.0, f64::from(sh - 1));
            if layer.flip_x {
                fx = f64::from(sw - 1) - fx;
            }
            let p = match layer.filter {
                Sampling::Nearest => sample(fx.round() as u32, fy.round() as u32),
                Sampling::Linear => {
                    let x0 = fx.floor() as u32;
                    let y0 = fy.floor() as u32;
                    let tx = fx - f64::from(x0);
                    let ty = fy - f64::from(y0);
                    let a = sample(x0, y0);
                    let b = sample((x0 + 1).min(sw - 1), y0);
                    let c = sample(x0, (y0 + 1).min(sh - 1));
                    let d = sample((x0 + 1).min(sw - 1), (y0 + 1).min(sh - 1));
                    std::array::from_fn(|i| {
                        (a[i] * (1.0 - tx) + b[i] * tx) * (1.0 - ty)
                            + (c[i] * (1.0 - tx) + d[i] * tx) * ty
                    })
                }
            };
            let target = (((dy + y) * width + dx + x) * 4) as usize;
            let opacity = f64::from(layer.opacity) / 255.0;
            for channel in 0..3 {
                frame[target + channel] = (p[channel] * f64::from(layer.tint[channel]) / 255.0
                    * opacity
                    + f64::from(frame[target + channel]) * (1.0 - p[3] * opacity))
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
            frame[target + 3] = (255.0 * p[3] * opacity
                + f64::from(frame[target + 3]) * (1.0 - p[3] * opacity))
                .round()
                .clamp(0.0, 255.0) as u8;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    None,
    Repaint,
    StageChanged,
    Unavailable(String),
    Unsupported(String),
    /// The scripted tutorial starts; the story resumes at the named stage
    /// (`Model::jump_to`) when it ends or is quit with Start.
    Tutorial {
        resume: String,
    },
    /// A campaign-map story event's pages ended: the frontend returns to
    /// the map at this map event.
    MapEvent(u16),
}
#[derive(Clone, Debug)]
pub struct Model {
    manifest: Manifest,
    stage_index: usize,
    pub selection: usize,
    page_index: usize,
    name_entry: Option<NameEntryState>,
    player_name: Option<PlayerName>,
    campaign: CampaignState,
    /// A menu cursor the game state sets for the next entry of that menu
    /// (`preset_selection`).
    selection_preset: Option<(String, usize)>,
    /// A menu whose cursor does not move (`lock_selection`).
    selection_lock: Option<String>,
}
impl Model {
    pub fn new(manifest: Manifest) -> Result<Self> {
        validate_manifest(&manifest)?;
        let stage_index = manifest
            .stages
            .iter()
            .position(|s| s.id() == manifest.initial_stage)
            .ok_or("Unknown initial stage")?;
        let mut model = Self {
            manifest,
            stage_index,
            selection: 0,
            page_index: 0,
            name_entry: None,
            player_name: None,
            campaign: CampaignState::default(),
            selection_preset: None,
            selection_lock: None,
        };
        model.reset_selection();
        Ok(model)
    }
    /// Starter deck and side chosen so far (serializable; see `campaign`).
    pub fn campaign(&self) -> &CampaignState {
        &self.campaign
    }
    /// Restore a saved game's committed name and campaign choices (the
    /// game's interim Continue), so later story pages show the name.
    pub fn restore_progress(&mut self, name: Option<PlayerName>, campaign: CampaignState) {
        self.player_name = name;
        self.campaign = campaign;
    }
    /// Current page's shared layer set and pan target, for 3D frontends.
    pub fn story_page(&self) -> Option<&StoryPage> {
        match self.stage() {
            Stage::Story { pages, .. } => pages.get(self.page_index),
            _ => None,
        }
    }
    /// Deck-select candidates for the committed name (records, 0x1ED53FA..).
    pub fn deck_candidates(&self) -> Option<Result<[&StarterDeck; campaign::CANDIDATES]>> {
        let Stage::DeckChoice { config, .. } = self.stage() else {
            return None;
        };
        Some((|| {
            let name = self
                .player_name
                .as_ref()
                .ok_or("Deck choice needs a committed player name.")?;
            let records = config.candidates(name)?;
            let mut decks = Vec::with_capacity(records.len());
            for record in records {
                decks.push(
                    config
                        .deck(record)
                        .ok_or("Deck candidate record is missing.")?,
                );
            }
            Ok([decks[0], decks[1], decks[2]])
        })())
    }
    /// Text every frontend must show for untraced parts of the current stage:
    /// a Placeholder's message, or the deck select's candidates and remaining
    /// missing presentation (the three-card stack is not yet rendered).
    pub fn explicit_status(&self) -> Option<String> {
        match self.stage() {
            Stage::Placeholder { message, .. } => Some(format!("[stand-in] {message}")),
            Stage::DeckChoice { .. } => Some(match self.deck_candidates()? {
                Ok(decks) => {
                    let list: Vec<String> = decks
                        .iter()
                        .enumerate()
                        .map(|(i, d)| {
                            format!(
                                "{}{} deck {} leader {} ({}) level {}.{:02}",
                                if i == self.selection { ">" } else { " " },
                                i + 1,
                                d.record,
                                d.leader_name,
                                d.leader,
                                d.average_level_x100 / 100,
                                d.average_level_x100 % 100
                            )
                        })
                        .collect();
                    let missing = if decks[self.selection].presentation_layers.is_empty() {
                        "[placeholder: deck information panel and card stack not imported]"
                    } else if decks.iter().any(|d| d.card_face.is_none()) {
                        "[placeholder: card stack not imported]"
                    } else {
                        "[partial: the stack's slide/rotate animation is not reproduced]"
                    };
                    format!(
                        "{missing} {} · Left/Right choose, Start (S key) confirms",
                        list.join(" |")
                    )
                }
                Err(error) => format!("[placeholder] {error}"),
            }),
            _ => None,
        }
    }
    pub fn stage(&self) -> &Stage {
        &self.manifest.stages[self.stage_index]
    }
    /// Every stage id of the manifest, in order.
    pub fn stage_ids(&self) -> Vec<&str> {
        self.manifest.stages.iter().map(Stage::id).collect()
    }
    pub fn page_index(&self) -> usize {
        self.page_index
    }
    pub fn name_entry(&self) -> Option<&NameEntryState> {
        self.name_entry.as_ref()
    }
    /// Last committed name. Editing another name does not partially overwrite it.
    pub fn player_name(&self) -> Option<&PlayerName> {
        self.player_name.as_ref()
    }
    fn reset_selection(&mut self) {
        self.page_index = 0;
        self.name_entry =
            matches!(self.stage(), Stage::NameEntry { .. }).then(NameEntryState::default);
        self.selection = match self.stage() {
            Stage::Menu {
                initial_selection, ..
            } => *initial_selection,
            _ => 0,
        };
        let preset = match (&self.selection_preset, self.stage()) {
            (Some((id, at)), Stage::Menu { entries, .. }) if id == self.stage().id() && *at < entries.len() => Some(*at),
            _ => None,
        };
        if let Some(at) = preset {
            self.selection = at;
            self.selection_preset = None;
        }
    }
    /// Start menu `stage` at entry `selection` the next time it is entered
    /// (once), instead of its imported initial selection: the side choice
    /// after an ending, whose cursor W+53AA story init 0x21C734 sets to the
    /// other side (`ending::continue_target`).
    pub fn preset_selection(&mut self, stage: &str, selection: usize) {
        self.selection_preset = Some((stage.to_owned(), selection));
    }
    /// Keep menu `stage`'s cursor where it is: the side choice after an
    /// ending moves only while SAVE[1] = SAVE[2] = 0, else Up/Down play
    /// SFX 6 and do nothing (event 10090, FLOW.md C5) [F].
    pub fn lock_selection(&mut self, stage: &str) {
        self.selection_lock = Some(stage.to_owned());
    }
    /// Whether the current menu's cursor is locked (`lock_selection`).
    pub fn selection_locked(&self) -> bool {
        self.selection_lock.as_deref() == Some(self.stage().id())
    }
    /// Continue at `stage` (a validated stage id), keeping the committed
    /// name and campaign state: the story's re-entry after the scripted
    /// tutorial (event 10060, FLOW.md Y6).
    pub fn jump_to(&mut self, stage: &str) -> Result<()> {
        if !self.manifest.stages.iter().any(|s| s.id() == stage) {
            return Err(format!("Unknown stage {stage}."));
        }
        self.go(stage);
        Ok(())
    }
    fn go(&mut self, target: &str) -> Outcome {
        self.stage_index = self
            .manifest
            .stages
            .iter()
            .position(|s| s.id() == target)
            .expect("validated stage target");
        self.reset_selection();
        Outcome::StageChanged
    }
    pub fn input(&mut self, button: Button) -> Outcome {
        match self.stage().clone() {
            Stage::NameEntry {
                config,
                on_complete,
                ..
            } => {
                let state = self
                    .name_entry
                    .as_mut()
                    .expect("name stage initializes editor");
                if state.committed {
                    return Outcome::None;
                }
                let rows = usize::from(config.rows);
                let columns = usize::from(config.columns);
                match button {
                    Button::Up => state.row = (state.row + rows - 1) % rows,
                    Button::Down => state.row = (state.row + 1) % rows,
                    Button::Left => state.column = (state.column + columns - 1) % columns,
                    Button::Right => state.column = (state.column + 1) % columns,
                    Button::L1 => {
                        state.page = (state.page + config.pages.len() - 1) % config.pages.len()
                    }
                    Button::R1 => state.page = (state.page + 1) % config.pages.len(),
                    Button::Circle => {
                        if state.glyphs.pop().is_none() {
                            return Outcome::None;
                        }
                    }
                    Button::Cross | Button::Square => {
                        if state.glyphs.len() == config.max_length {
                            return Outcome::None;
                        }
                        let glyph =
                            &config.pages[state.page].cells[state.row * columns + state.column];
                        state.glyphs.push(NameCharacter {
                            page: state.page as u8,
                            row: state.row as u8,
                            column: state.column as u8,
                            display_code: glyph.display_code,
                            stored_code: glyph.stored_code,
                        });
                    }
                    Button::Start => {
                        if state.glyphs.is_empty() {
                            return Outcome::None;
                        }
                        state.committed = true;
                        self.player_name = Some(PlayerName {
                            glyphs: state.glyphs.clone(),
                        });
                        return match on_complete {
                            StoryEnd::Stage { target } => self.go(&target),
                            StoryEnd::Unsupported { message } => Outcome::Unsupported(message),
                            StoryEnd::MapEvent { event } => Outcome::MapEvent(event),
                        };
                    }
                }
                Outcome::Repaint
            }
            Stage::Story {
                pages,
                advance_buttons,
                on_end,
                ..
            } => {
                if !advance_buttons.contains(&button) {
                    return Outcome::None;
                }
                if self.page_index + 1 < pages.len() {
                    self.page_index += 1;
                    Outcome::Repaint
                } else {
                    match on_end {
                        StoryEnd::Stage { target } => self.go(&target),
                        StoryEnd::Unsupported { message } => Outcome::Unsupported(message),
                        StoryEnd::MapEvent { event } => Outcome::MapEvent(event),
                    }
                }
            }
            Stage::Movie {
                skip_buttons,
                on_skip,
                ..
            } => {
                if skip_buttons.contains(&button) {
                    self.go(on_skip.as_deref().expect("validated skip target"))
                } else {
                    Outcome::None
                }
            }
            Stage::Screen {
                advance_buttons,
                next,
                ..
            } => {
                if advance_buttons.contains(&button) {
                    self.go(&next)
                } else {
                    Outcome::None
                }
            }
            Stage::Menu {
                entries,
                confirm_buttons,
                cancel_target,
                wrap_navigation,
                ..
            } => {
                if button == Button::Circle {
                    if let Some(t) = cancel_target {
                        return self.go(&t);
                    }
                }
                if matches!(button, Button::Up | Button::Down) && self.selection_locked() {
                    return Outcome::None;
                }
                if matches!(button, Button::Up | Button::Down) {
                    let old = self.selection;
                    if button == Button::Up {
                        self.selection = if self.selection > 0 {
                            self.selection - 1
                        } else if wrap_navigation {
                            entries.len() - 1
                        } else {
                            0
                        };
                    } else {
                        self.selection = if self.selection + 1 < entries.len() {
                            self.selection + 1
                        } else if wrap_navigation {
                            0
                        } else {
                            self.selection
                        };
                    }
                    return if old != self.selection {
                        Outcome::Repaint
                    } else {
                        Outcome::None
                    };
                }
                if confirm_buttons.contains(&button) {
                    let entry = &entries[self.selection];
                    if let Some(side) = entry.campaign_side {
                        self.campaign.side = Some(side);
                    }
                    match &entry.action {
                        MenuAction::Unavailable => Outcome::Unavailable(entry.label.clone()),
                        MenuAction::Unsupported { message } => {
                            Outcome::Unsupported(message.clone())
                        }
                        MenuAction::Tutorial { resume } => Outcome::Tutorial {
                            resume: resume.clone(),
                        },
                        MenuAction::Stage { target } => self.go(target),
                    }
                } else {
                    Outcome::None
                }
            }
            Stage::DeckChoice {
                config,
                confirm_buttons,
                on_confirm,
                ..
            } => match button {
                // 0x23916C: Right +1, Left −1, both wrapping over three.
                Button::Right => {
                    self.selection = (self.selection + 1) % campaign::CANDIDATES;
                    Outcome::Repaint
                }
                Button::Left => {
                    self.selection =
                        (self.selection + campaign::CANDIDATES - 1) % campaign::CANDIDATES;
                    Outcome::Repaint
                }
                _ if confirm_buttons.contains(&button) => {
                    let choice = (|| -> Result<StarterDeckChoice> {
                        let name = self
                            .player_name
                            .as_ref()
                            .ok_or("Deck choice needs a committed player name.")?;
                        let candidates = config.candidates(name)?;
                        let record = candidates[self.selection];
                        let deck = config.deck(record).ok_or("Deck record is missing.")?;
                        Ok(StarterDeckChoice {
                            name_codes: config.name_codes(name)?.to_vec(),
                            checksum: config.checksum(name)?,
                            candidates: candidates.to_vec(),
                            selection: self.selection as u8,
                            record,
                            leader: deck.leader,
                            cards: deck.cards.clone(),
                        })
                    })();
                    match choice {
                        Ok(choice) => {
                            self.campaign.starter_deck = Some(choice);
                            match on_confirm {
                                StoryEnd::Stage { target } => self.go(&target),
                                StoryEnd::Unsupported { message } => Outcome::Unsupported(message),
                                StoryEnd::MapEvent { event } => Outcome::MapEvent(event),
                            }
                        }
                        Err(message) => Outcome::Unsupported(message),
                    }
                }
                _ => Outcome::None,
            },
            Stage::Placeholder {
                advance_buttons,
                next,
                ..
            } => {
                if !advance_buttons.contains(&button) {
                    return Outcome::None;
                }
                match next {
                    StoryEnd::Stage { target } => self.go(&target),
                    StoryEnd::Unsupported { message } => Outcome::Unsupported(message),
                    StoryEnd::MapEvent { event } => Outcome::MapEvent(event),
                }
            }
        }
    }
    pub fn video_ended(&mut self) -> Outcome {
        if let Stage::Movie { on_end, .. } = self.stage().clone() {
            self.go(&on_end)
        } else {
            Outcome::None
        }
    }
    pub fn elapsed(&mut self, milliseconds: u128) -> Outcome {
        let next = match self.stage() {
            Stage::Screen {
                duration_ms: Some(duration),
                next,
                ..
            } if milliseconds >= u128::from(*duration) => Some(next.clone()),
            _ => None,
        };
        next.map_or(Outcome::None, |target| self.go(&target))
    }
}
/// Returns a centered letterbox rectangle with explicit display aspect.
pub fn fit_rect(width: i32, height: i32, aspect_width: u32, aspect_height: u32) -> [i32; 4] {
    let w = i64::from(width.max(0));
    let h = i64::from(height.max(0));
    let aw = i64::from(aspect_width.max(1));
    let ah = i64::from(aspect_height.max(1));
    let (rw, rh) = if w * ah <= h * aw {
        (w, w * ah / aw)
    } else {
        (h * aw / ah, h)
    };
    [
        ((w - rw) / 2) as i32,
        ((h - rh) / 2) as i32,
        rw as i32,
        rh as i32,
    ]
}

#[cfg(test)]
mod tests;

/// `ROSES_MUTE=1` keeps every sound output closed (automated runs: the
/// playability suite, captures, research runs), so nothing is audible.
pub fn muted() -> bool {
    std::env::var("ROSES_MUTE").is_ok_and(|v| v == "1")
}
