//! Extend the post-name import with the story from Simon's practice question to
//! the campaign map (PAL SLES_515.79; private research
//! `story/claude-to-first-duel/FLOW.md`, "Story to the map (Bevy)").
//!
//! Order: practice choice "No" → 10060 (Simon) → 10070 (Simon) → starter deck
//! select → 10071 (Simon) → 10080 (Seto arrives) → 10090 → side choice →
//! 11000 (Seto leaves) → map event 100 (stand-in) → 11020 (Yugi at Brest) →
//! map events 101/102 (stand-in) → explicit boundary at map event 0. The
//! side choice's Yorkists row → 12000 (Seto welcomes the Yorkist and sends
//! Simon away) → map event 100 of the White Rose side.
//!
//! Dialogue rules (message dispatcher 0x22D600..0x22E630): every page is the
//! base plus a counter, with blank messages skipped; group g =
//! (counter − 1)/4 + 1. Layout 2 (Simon, speaker 197) puts text at raw (136, 400) and turns the
//! bottom panel on (0x53B7). Layout 3 puts text at raw (16, 48); its speaker
//! (0x53BC) is Seto 198 with colour selector 1 or Yugi 216 with selector 0,
//! and some cases turn the top panel on (0x53B6), which then stays until the
//! event's closing phase. Speaker panels 0x232EF0: top panel label emblem
//! (0x297500 + 38·sel) at raw (16,16,32,32), style-2 bar (48,16)–(384,48)
//! with the gradient of selector `sel`, portrait TEXANM record speaker − 196
//! (0x2330x: 0x540F; 0x247C90 loads archive 7 record) at raw (400, 0) in a
//! style-0 frame (400,0,104,144), wings (0x2977CC/0x2977F2, sprites 51/52) and
//! ornament (0x297780 + 38·sel, sprite 53/54), name at raw (53, 21).
//! Big portraits (0x2306A0): pan byte 0x1ED5439 goes to +100 for layout 2
//! (Simon, sprite 171 at raw (152,132,208,288)), −100 for layout 3 with the
//! top panel (Seto, sprite 172; for events 10080..=11000 widened to x1 = 416
//! and u1 = 512 at 0x230290, drawn at x = 512 − (256/2 + 360) = 24) and 0
//! otherwise (no figure). The same byte pans the Simon scene camera.
//! Choices (0x224DB4): panel style 2 with selector 2 fill, Enter frame style 9
//! at (x+w−139, y+h−42, 130, 34) with the Enter icon (template 0x299E70,
//! sprite 112) at +(11, 9) and message 168 at +(36, 6). Text through
//! 0x21BDB0 is scaled horizontally to its maximum width when at least that
//! wide. Deck select: panel (8,196,496,160), title box style 1 (14,158,188,32)
//! with message 256 at (24,164), at most 172 wide. Side choice: panel
//! (27,168,458,176), rose icons sprite 11 at (106,200) and 10 at (106,256),
//! rows (140,200|256,320,33) style 8, labels 257/258 at (148,204|260) with
//! vertex colour 128 selected, 64 otherwise.
use crate::import_post_name::{name_cells, name_text_for, simon_common};
use crate::import_story::{frame_layers, import_png, message_image, raw_rect, story_layer};
use crate::{emit_json, open_pal, Result};
use roses_assets::{
    decode_pal_message_font, parse_pal_message_templates, parse_pal_messages, MessageTemplateToken,
    OpeningArchiveKind,
};
use roses_client::campaign_map as map;
use roses_client::{
    Button, CampaignSide, DeckChoiceConfig, Layer, Loaded, MenuAction, MenuEntry, Stage,
    StarterDeck, StoryEnd, StoryPage,
};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::Path;
mod brest;
mod deck_panel;

/// One dispatched dialogue event: stage id, event, message base, pages.
pub(crate) struct Event {
    pub stage: &'static str,
    pub event: u16,
    pub base: u16,
    pub pages: &'static [u16],
}
pub(crate) const EVENTS: [Event; 8] = [
    Event {
        stage: "simon-deck-intro",
        event: 10060,
        base: 2183,
        pages: &[2184, 2188, 2189, 2192, 2196, 2197, 2200, 2201],
    },
    Event {
        stage: "simon-deck-prompt",
        event: 10070,
        base: 2203,
        pages: &[2204, 2205, 2208],
    },
    Event {
        stage: "simon-deck-chosen",
        event: 10071,
        base: 2211,
        pages: &[2212],
    },
    Event {
        stage: "seto-arrival",
        event: 10080,
        base: 2215,
        pages: &[
            2216, 2220, 2221, 2224, 2228, 2232, 2236, 2240, 2241, 2244, 2245, 2248, 2252, 2253,
            2256, 2260, 2264, 2268, 2269, 2270, 2272, 2276, 2280, 2281, 2284, 2285, 2288, 2289,
            2292, 2296, 2297, 2300, 2301, 2304, 2305, 2308, 2312, 2313, 2316, 2317, 2320, 2324,
            2325, 2328, 2329, 2332, 2333, 2336, 2340, 2344, 2345, 2348, 2349, 2352, 2356, 2357,
            2360, 2361, 2364, 2365, 2368, 2372, 2373, 2376, 2377, 2380, 2381, 2384, 2388, 2392,
            2396, 2400, 2404,
        ],
    },
    Event {
        stage: "seto-side-intro",
        event: 10090,
        base: 2407,
        pages: &[2408, 2409, 2412, 2416, 2420],
    },
    Event {
        stage: "seto-leaves",
        event: 11000,
        base: 2423,
        pages: &[2424, 2428, 2429, 2432, 2433, 2436],
    },
    Event {
        stage: "yugi-at-brest",
        event: 11020,
        base: 2443,
        pages: &[
            2444, 2448, 2452, 2456, 2460, 2464, 2468, 2472, 2473, 2476, 2480, 2481, 2484, 2485,
            2488, 2489, 2492, 2493, 2496, 2497, 2500, 2501, 2504, 2505, 2508, 2509, 2510, 2512,
            2513, 2516,
        ],
    },
    // 12000 (0x22181C): base 3031, 17 groups; the 4·17-page window ends
    // before 3100 (story_pages.json, WHITE-ROUTE-AUDIT.md §2) [F].
    Event {
        stage: WHITE_STAGE,
        event: 12000,
        base: 3031,
        pages: &[
            3032, 3036, 3040, 3044, 3048, 3049, 3052, 3056, 3057, 3060, 3061, 3064, 3065, 3068,
            3069, 3072, 3073, 3076, 3080, 3081, 3084, 3085, 3088, 3089, 3092, 3093, 3096,
        ],
    },
];
/// The story stage of event 12000, the side choice's Yorkists row.
pub(crate) const WHITE_STAGE: &str = "seto-yorkists";
/// 12000's group 4 (3044, "Wha... What are you doing!?") is Simon's, layout
/// 2; from then on the bottom panel is off (0x53B7 = 0) [F story_pages].
const SIMON_12000_GROUP: u16 = 4;
/// Pages whose text references the mutable player name (message 3).
pub(crate) const NAME_MESSAGES: [u16; 6] = [2184, 2200, 2364, 2388, 2444, 2452];
/// 10080 groups dispatched as layout 3 / Seto (0x22DB48..0x22DC4C).
const SETO_10080_GROUPS: [u16; 31] = [
    1, 2, 4, 6, 7, 8, 9, 10, 12, 13, 14, 20, 21, 22, 23, 25, 26, 30, 32, 33, 34, 36, 37, 39, 40,
    41, 42, 43, 45, 46, 47,
];
/// 11020 table 0x2D2490 over g − 4 (0..16): true = Yugi (0x22DDD8).
const YUGI_11020_TABLE: [bool; 16] = [
    true, false, true, true, true, true, true, true, true, false, true, true, true, true, true,
    true,
];
const SPEAKER_SIMON: u16 = 197;
const SPEAKER_SETO: u16 = 198;
const SPEAKER_YUGI: u16 = 216;
const DECK_TITLE: u16 = 256;
const SIDE_LABELS: [u16; 2] = [257, 258];
/// Static records the deck select can offer (info table 0x2990E0 rows 0..=16).
const STARTER_RECORDS: usize = 17;
/// EE → SLES_515.79 file offset for the main image (VA = offset + 0xF9680).
const EE_TO_FILE: usize = 0xF9680;
const CANDIDATE_TABLE_EE: usize = 0x2990C0;
const DECK_INFO_EE: usize = 0x2990E0;
/// Practice choice "Yes": the scripted tutorial is a duel-board scene (the
/// three lessons of controllers 32/33/34), which the story manifest only
/// names; the Bevy game plays it and resumes at event 10060 (FLOW.md Y1-Y6).
const TUTORIAL_NOTE: &str = "The practice choice Yes starts the scripted tutorial (menu action tutorial: three lessons on the duel board, then event 10060 with track 25); only a duel-capable frontend plays it.";
const WHITE_ROUTE_BOUNDARY: &str = "Siding with the Yorkists (0x21ED44: SAVE[0] = 1, C9 = 2) plays event 12000 and continues at map event 100 of the White Rose side; the White Rose story events after it (12040, 12041, 12130..=12156) are not in this import.";
const MAP_VOYAGES: &str = "Story 11000 ends at campaign-map event 100 (the voyage to Brest) and 11020 at event 101 (the voyage to Milford Haven, reveal 102, then the free map); a frontend with the campaign map plays them.";

/// Who speaks on a page and which panels the dispatcher left on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PageLayout {
    /// Layout 3 (top text) instead of layout 2 (Simon's bottom text).
    pub top_text: bool,
    pub speaker: u16,
    /// 0x53B6: the top speaker panel is drawn.
    pub top_panel: Option<u16>,
    pub group: u16,
    /// 0x53B7: the bottom (Simon's) panel is drawn.
    pub bottom_panel: bool,
}
impl PageLayout {
    /// Pan byte target 0x1ED5439 for this page.
    pub fn pan(self) -> i8 {
        match (self.top_text, self.top_panel.is_some()) {
            (false, _) => 100,
            (true, true) => -100,
            (true, false) => 0,
        }
    }
}

/// Apply the dispatcher cases of one event to its pages in order.
pub(crate) fn page_layouts(event: &Event) -> Vec<PageLayout> {
    // 0x53B6 is cleared by the Simon cases of 10040..10071 and by the closing
    // phase 13 of 11000, so 10060 and 11020 both start without a top panel;
    // 11000 keeps the panel 10090 turned on.
    let mut top_panel = match event.event {
        11000 | 12000 => Some(SPEAKER_SETO),
        _ => None,
    };
    event
        .pages
        .iter()
        .map(|&id| {
            let group = (id - event.base - 1) / 4 + 1;
            let top = match event.event {
                10080 => SETO_10080_GROUPS.contains(&group).then(|| {
                    if group >= 4 {
                        top_panel = Some(SPEAKER_SETO);
                    }
                    SPEAKER_SETO
                }),
                10090 => (group != 3).then(|| {
                    top_panel = Some(SPEAKER_SETO);
                    SPEAKER_SETO
                }),
                11000 => Some(SPEAKER_SETO),
                12000 => (group != SIMON_12000_GROUP).then_some(SPEAKER_SETO),
                11020 => group
                    .checked_sub(4)
                    .and_then(|i| YUGI_11020_TABLE.get(usize::from(i)))
                    .copied()
                    .unwrap_or(false)
                    .then(|| {
                        top_panel = Some(SPEAKER_YUGI);
                        SPEAKER_YUGI
                    }),
                _ => None,
            };
            PageLayout {
                top_text: top.is_some(),
                speaker: top.unwrap_or(SPEAKER_SIMON),
                top_panel,
                group,
                bottom_panel: !(event.event == 12000 && group > SIMON_12000_GROUP),
            }
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Figure {
    None,
    Simon,
    Seto,
}
fn figure(event: u16, layout: PageLayout) -> Figure {
    // 11020 loads the castle paintings, without a standing figure.
    if event == 11020 {
        return Figure::None;
    }
    match layout.pan() {
        100 => Figure::Simon,
        -100 => Figure::Seto,
        _ => Figure::None,
    }
}
fn set_name(figure: Figure, top: Option<u16>, bottom: bool) -> String {
    let figure = match figure {
        Figure::None => "none",
        Figure::Simon => "simon",
        Figure::Seto => "seto",
    };
    let top = match top {
        Some(SPEAKER_SETO) => "seto",
        Some(_) => "yugi",
        None => "none",
    };
    if bottom {
        format!("dialogue-figure-{figure}-top-{top}")
    } else {
        format!("dialogue-figure-{figure}-top-{top}-no-bottom")
    }
}

struct Panels {
    simon_figure: Layer,
    mask: Layer,
    simon_panel: Vec<Layer>,
    seto_figure: Layer,
    seto_top: Vec<Layer>,
    yugi_top: Vec<Layer>,
}
impl Panels {
    fn set(&self, figure: Figure, top: Option<u16>, bottom: bool) -> Vec<Layer> {
        let mut layers = match figure {
            Figure::Simon => vec![self.simon_figure.clone()],
            Figure::Seto => vec![self.seto_figure.clone()],
            Figure::None => vec![],
        };
        layers.push(self.mask.clone());
        if bottom {
            layers.extend(self.simon_panel.iter().cloned());
        }
        match top {
            Some(SPEAKER_SETO) => layers.extend(self.seto_top.iter().cloned()),
            Some(_) => layers.extend(self.yugi_top.iter().cloned()),
            None => (),
        }
        layers
    }
}

pub(crate) fn text_layer(asset: &str, image: &roses_assets::RgbaImage, origin: [u32; 2]) -> Layer {
    story_layer(
        asset,
        None,
        [
            origin[0],
            origin[1],
            image.width * 5 / 4,
            (image.height * 7 + 4) / 8,
        ],
    )
}
/// Choice text through 0x21BDB0: a text texture at least `max_raw` wide is
/// scaled horizontally to exactly `max_raw` (f12); narrower text is 1:1.
fn fitted_text_layer(
    asset: &str,
    image: &roses_assets::RgbaImage,
    raw: [u32; 2],
    max_raw: u32,
) -> Layer {
    let mut layer = text_layer(asset, image, raw_origin(raw[0], raw[1]));
    if image.width >= max_raw {
        layer.destination[2] = max_raw * 5 / 4;
    }
    layer
}
pub(crate) fn raw_origin(x: u32, y: u32) -> [u32; 2] {
    let rect = raw_rect([x, y, 0, 0]);
    [rect[0], rect[1]]
}

/// Style-2 bar gradient for colour selector 0 (red) or 1 (0,96,96), built
/// exactly like the imported Simon bar (half value at the edges).
pub(crate) fn label_bar(selector: u8) -> roses_assets::RgbaImage {
    let mut bar = roses_assets::RgbaImage {
        width: 336,
        height: 32,
        pixels: vec![0; 336 * 32 * 4],
    };
    for y in 0..32u32 {
        let v = (48 + 48 * y.min(31 - y) / 15) as u8;
        let rgb = if selector == 0 { [v, 0, 0] } else { [0, v, v] };
        for x in 0..336u32 {
            let at = ((y * 336 + x) * 4) as usize;
            bar.pixels[at..at + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
    }
    bar
}

/// Top speaker panel at rest (0x232EF0 left panel, offsets 0).
pub(crate) fn top_panel(
    selector: u8,
    portrait: &str,
    bar: &str,
    name: &str,
    name_image: &roses_assets::RgbaImage,
) -> Vec<Layer> {
    let atlas =
        |source: [u32; 4], raw: [u32; 4]| story_layer("texsys-07", Some(source), raw_rect(raw));
    // Portrait size follows the imported Simon portrait layer (128x126 canvas).
    let mut layers = vec![
        story_layer(portrait, Some([0, 0, 128, 128]), [500, 0, 128, 126]),
        story_layer(bar, None, raw_rect([48, 16, 336, 32])),
        atlas([224, 32 * u32::from(selector), 32, 32], [16, 16, 32, 32]),
    ];
    layers.extend(frame_layers([400, 0, 104, 144], [64, 0, 16, 14]));
    layers.extend(frame_layers([48, 16, 336, 32], [48, 64, 8, 8]));
    layers.push(atlas([69, 116, 56, 12], [400, 142, 48, 12]));
    let mut right = atlas([69, 116, 56, 12], [456, 142, 48, 12]);
    right.flip_x = true;
    layers.push(right);
    layers.push(if selector == 0 {
        atlas([0, 0, 64, 23], [428, 141, 48, 23])
    } else {
        atlas([0, 23, 64, 21], [428, 141, 48, 21])
    });
    layers.push(text_layer(name, name_image, raw_origin(53, 21)));
    layers
}

/// Settled choice panel (0x224DB4) with the Enter frame, icon and message 168.
fn choice_panel(panel: [u32; 4]) -> Vec<Layer> {
    let [x, y, w, h] = panel;
    let mut fill = story_layer("story-black", None, raw_rect([x + 2, y + 2, w - 4, h - 4]));
    fill.opacity = 128; // GS alpha 64 of 128
    let mut layers = vec![fill];
    layers.extend(frame_layers(panel, [48, 64, 8, 8]));
    let enter = [x + w - 139, y + h - 42, 130, 34];
    layers.extend(frame_layers(enter, [64, 56, 16, 14]));
    layers.push(enter_icon(enter));
    layers
}
fn enter_icon(enter: [u32; 4]) -> Layer {
    story_layer(
        "texsys-07",
        Some([80, 240, 16, 16]),
        raw_rect([enter[0] + 11, enter[1] + 9, 16, 16]),
    )
}
fn enter_text(panel: [u32; 4], image: &roses_assets::RgbaImage) -> Layer {
    let [x, y, w, h] = panel;
    // Maximum width 80.0 (0x224FC4).
    fitted_text_layer(
        "choice-text-168",
        image,
        [x + w - 139 + 36, y + h - 42 + 6],
        80,
    )
}

pub fn run(iso: &Path, post_name: &Path, output: &Path) -> Result<()> {
    if output.exists() {
        return Err(
            "story-to-map output must be a new directory; imports are never overwritten".into(),
        );
    }
    let loaded = Loaded::open(post_name)?;
    let (mut disc, report) = open_pal(iso)?;
    if loaded.manifest.source.disc_sha256 != report.iso_sha256
        || Some(&loaded.manifest.source.executable_sha256) != report.executable_sha256.as_ref()
    {
        return Err("post-name import does not match the supplied ISO".into());
    }
    let mut manifest = loaded.manifest.clone();
    if !manifest
        .stages
        .iter()
        .any(|s| s.id() == "practice-choice" && matches!(s, Stage::Menu { .. }))
    {
        return Err("import has no practice choice (run import-post-name first)".into());
    }
    if manifest.stages.iter().any(|s| s.id() == EVENTS[0].stage) {
        return Err("import already continues past the practice choice".into());
    }
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(output)?;
    for asset in &manifest.assets {
        let destination = output.join(&asset.path);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        let source = File::open(&loaded.assets[&asset.id].path)?;
        let mut target = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)?;
        if std::io::copy(&mut source.take(asset.bytes + 1), &mut target)? != asset.bytes {
            return Err("source asset changed size while copying the import".into());
        }
        target.sync_all()?;
    }
    let bytes = disc.read_file("MSG/MSG_E.DAT", 300_000)?;
    let executable = disc.read_file("SLES_515.79", 8 * 1024 * 1024)?;
    let texanm = disc.read_file(
        "DATA/TEXANM.MRG",
        roses_assets::MAX_OPENING_ARCHIVE_BYTES as u64,
    )?;
    brest::import(&mut disc, output, &mut manifest)?;

    let page_ids: Vec<u16> = EVENTS
        .iter()
        .flat_map(|e| e.pages.iter().copied())
        .collect();
    let templates = parse_pal_message_templates(&bytes, &page_ids)?;
    let name_ids: Vec<u16> = templates
        .messages
        .iter()
        .filter(|m| {
            m.tokens
                .iter()
                .any(|t| matches!(t, MessageTemplateToken::PlayerNameSlot { .. }))
        })
        .map(|m| m.message_id)
        .collect();
    if name_ids != NAME_MESSAGES {
        return Err(format!("unexpected player-name pages {name_ids:?}").into());
    }
    let mut static_ids: Vec<u16> = page_ids
        .iter()
        .copied()
        .filter(|id| !NAME_MESSAGES.contains(id))
        .collect();
    static_ids.extend([SPEAKER_SETO, SPEAKER_YUGI, DECK_TITLE]);
    static_ids.extend(SIDE_LABELS);
    static_ids.push(168);
    let messages = parse_pal_messages(&bytes, &static_ids)?;
    let message = |id: u16| {
        messages
            .messages
            .iter()
            .find(|m| m.message_id == id)
            .ok_or("story message missing")
    };
    let font = decode_pal_message_font(&bytes)?;
    let (common, text_origin, simon_music) = simon_common(&manifest)?;
    if common.len() < 3 || common[0].asset != "texeve-17" || common[1].asset != "story-black" {
        return Err("Simon's settled layers no longer start with figure and mask".into());
    }
    let cells = name_cells(&manifest, &font)?;
    let blank_name_code = manifest
        .stages
        .iter()
        .find_map(|stage| match stage {
            // Editor init: unused slots select page 0, row 5, column 0.
            Stage::NameEntry { config, .. } if config.rows == 6 && config.columns == 13 => {
                config.pages.first().map(|p| p.cells[5 * 13].stored_code)
            }
            _ => None,
        })
        .ok_or("name entry does not have the original 6x13 keyboard")?;

    // New images.
    let image = |id: u16| message_image(&font, message(id)?);
    for (asset, record) in [("seto-small-portrait", 2), ("yugi-small-portrait", 20)] {
        let portrait =
            roses_assets::decode_pal_opening_texture(&texanm, OpeningArchiveKind::TexAnm, record)?;
        import_png(output, &mut manifest, asset, &portrait)?;
    }
    import_png(output, &mut manifest, "seto-label-bar", &label_bar(1))?;
    let seto_name = image(SPEAKER_SETO)?;
    import_png(output, &mut manifest, "seto-speaker", &seto_name)?;
    let yugi_name = image(SPEAKER_YUGI)?;
    import_png(output, &mut manifest, "yugi-speaker", &yugi_name)?;
    let panels = Panels {
        simon_figure: common[0].clone(),
        mask: common[1].clone(),
        simon_panel: common[2..].to_vec(),
        // Sprite 172 widened for events 10080..=11000: raw (24,132)-(440,420).
        seto_figure: story_layer("texeve-18", None, raw_rect([24, 132, 416, 288])),
        seto_top: top_panel(
            1,
            "seto-small-portrait",
            "seto-label-bar",
            "seto-speaker",
            &seto_name,
        ),
        yugi_top: top_panel(
            0,
            "yugi-small-portrait",
            "simon-label-bar",
            "yugi-speaker",
            &yugi_name,
        ),
    };

    // Dialogue stages.
    let mut sets: BTreeMap<String, Vec<Layer>> = BTreeMap::new();
    let mut records = Vec::new();
    let mut stages = Vec::new();
    let mut last_set = BTreeMap::new();
    for event in &EVENTS {
        let mut pages = Vec::new();
        for (&id, layout) in event.pages.iter().zip(page_layouts(event)) {
            let figure = figure(event.event, layout);
            let set = brest::set_name(
                event.event,
                set_name(figure, layout.top_panel, layout.bottom_panel),
            );
            sets.entry(set.clone()).or_insert_with(|| {
                brest::layers(
                    event.event,
                    panels.set(figure, layout.top_panel, layout.bottom_panel),
                )
            });
            let origin = if layout.top_text {
                raw_origin(16, 48)
            } else {
                text_origin
            };
            let (layers, name_text) = if NAME_MESSAGES.contains(&id) {
                let template = templates
                    .messages
                    .iter()
                    .find(|m| m.message_id == id)
                    .ok_or("name template missing")?;
                (
                    vec![],
                    Some(name_text_for(template, &font, origin, &cells)?),
                )
            } else {
                let text = image(id)?;
                let asset = format!("story-text-{id}");
                import_png(output, &mut manifest, &asset, &text)?;
                (vec![text_layer(&asset, &text, origin)], None)
            };
            records.push(serde_json::json!({"stage":event.stage,"event":event.event,
                "message_id":id,"group":layout.group,"layout":if layout.top_text {3} else {2},
                "speaker":layout.speaker,"top_panel":layout.top_panel,"pan":layout.pan(),
                "figure":format!("{figure:?}"),"layer_set":set,"runtime_name":name_text.is_some()}));
            last_set.insert(event.stage, (figure, layout.top_panel, layout.bottom_panel));
            pages.push(StoryPage {
                id: format!("message-{id}"),
                layer_set: Some(set),
                layers,
                name_text,
                pan: Some(layout.pan()),
            });
        }
        stages.push((event.stage, event.event, pages));
    }
    manifest.layer_sets.extend(sets);

    // Deck select data from the user's executable.
    let word = |ee: usize| -> Result<u16> {
        let at = ee - EE_TO_FILE;
        Ok(u16::from_le_bytes(
            executable
                .get(at..at + 2)
                .ok_or("executable table lies outside the file")?
                .try_into()?,
        ))
    };
    let deck_table = roses_assets::parse_pal_decks(&executable)?;
    let names = roses_assets::parse_pal_card_names(&bytes)?;
    let mut decks = Vec::new();
    for record in 0..STARTER_RECORDS {
        let leader = word(DECK_INFO_EE + 12 * record)? & 0x3FF;
        let static_deck = &deck_table.decks[record];
        if static_deck.leader_card != leader {
            return Err(
                format!("deck {record}: info leader {leader} differs from the deck table").into(),
            );
        }
        decks.push(StarterDeck {
            record: record as u8,
            leader,
            average_level_x100: word(DECK_INFO_EE + 12 * record + 2)?,
            leader_name: names.names[usize::from(leader)].name.clone(),
            cards: static_deck.cards.clone(),
            presentation_layers: vec![],
            card_face: None,
        });
    }
    let deck_presentations =
        deck_panel::import(&executable, &bytes, output, &mut manifest, &decks)?;
    for (deck, layers) in decks.iter_mut().zip(deck_presentations) {
        deck.presentation_layers = layers;
    }
    // Leader faces of the three-card stack (0x229EF0 draws each candidate's
    // leader as 0x1DBA80 composes it; `roses_client::card_face`).
    let faces = roses_client::card_face::FaceAssets::new(
        executable.clone(),
        bytes.clone(),
        &disc.read_file("DATA/TEXSYS.MRG", 2 * 1024 * 1024)?,
        disc.read_file("DATA/PICTURE.MRG", 32 * 1024 * 1024)?,
    )?;
    let catalogue = crate::duel_catalogue::build(&executable)?;
    for deck in &mut decks {
        let info = catalogue
            .cards
            .get(usize::from(deck.leader))
            .ok_or("starter leader lies outside the card catalogue")?;
        let face = faces.face(
            deck.leader,
            info.level,
            i32::from(info.attack),
            i32::from(info.defense),
            roses_client::card_face::FaceSize::Large,
        )?;
        let id = format!("starter-card-{}", deck.record);
        import_png(output, &mut manifest, &id, &face)?;
        deck.card_face = Some(id);
    }
    let config = DeckChoiceConfig {
        candidate_table: (0..16)
            .map(|i| word(CANDIDATE_TABLE_EE + 2 * i))
            .collect::<Result<_>>()?,
        blank_name_code,
        decks,
    };
    config.validate()?;

    // Screens after the dialogue: the last page's panels stay without text.
    let settled = |stage: &str| {
        let (figure, top, bottom) = last_set[stage];
        panels.set(figure, top, bottom)
    };
    let text_168 = message_image(&font, message(168)?)?;
    let deck_panel = [8, 196, 496, 160];
    let mut deck_layers = settled("simon-deck-prompt");
    deck_layers.extend(choice_panel(deck_panel));
    deck_layers.push(enter_text(deck_panel, &text_168));
    let title = image(DECK_TITLE)?;
    import_png(output, &mut manifest, "choice-text-256", &title)?;
    let mut title_fill = story_layer("story-black", None, raw_rect([16, 160, 184, 28]));
    title_fill.opacity = 128;
    deck_layers.push(title_fill);
    deck_layers.extend(frame_layers([14, 158, 188, 32], [64, 28, 16, 14]));
    // Title box width 188 → maximum text width 188 − 16 (0x2255A4).
    deck_layers.push(fitted_text_layer("choice-text-256", &title, [24, 164], 172));

    let side_panel = [27, 168, 458, 176];
    let mut side_layers = settled("seto-side-intro");
    side_layers.extend(choice_panel(side_panel));
    side_layers.push(enter_text(side_panel, &text_168));
    side_layers.push(story_layer(
        "texsys-07",
        Some([224, 0, 32, 32]),
        raw_rect([106, 256, 32, 32]),
    ));
    side_layers.push(story_layer(
        "texsys-07",
        Some([224, 32, 32, 32]),
        raw_rect([106, 200, 32, 32]),
    ));
    let mut side_entries = Vec::new();
    for (index, id) in SIDE_LABELS.into_iter().enumerate() {
        let text = image(id)?;
        let asset = format!("choice-text-{id}");
        import_png(output, &mut manifest, &asset, &text)?;
        let row_y = [200, 256][index];
        // Row width 320 → maximum label width 304 (0x225320).
        let label = fitted_text_layer(&asset, &text, [148, row_y + 4], 304);
        let mut dim = label.clone();
        dim.tint = [128; 3]; // vertex colour 64 of 128
        let mut selected = frame_layers([140, row_y, 320, 33], [0, 224, 16, 14]);
        selected.push(label);
        side_entries.push(MenuEntry {
            id: ["yorkists", "lancastrians"][index].into(),
            label: format!("message-{id}"),
            normal_layers: vec![dim],
            selected_layers: selected,
            // Row 0 (Yorkists) → 12000, row 1 → 11000 (0x21ED18) [F].
            action: MenuAction::Stage {
                target: if index == 0 { WHITE_STAGE } else { "seto-leaves" }.into(),
            },
            campaign_side: Some(if index == 0 {
                CampaignSide::Yorkists
            } else {
                CampaignSide::Lancastrians
            }),
        });
    }

    // Practice choice: "No" continues to 10060; "Yes" starts the scripted
    // tutorial and resumes at 10060 afterwards (FLOW.md Y1-Y6).
    for stage in &mut manifest.stages {
        if let Stage::Menu {
            id,
            layers,
            entries,
            ..
        } = stage
        {
            if id != "practice-choice" {
                continue;
            }
            for entry in entries.iter_mut() {
                entry.action = match entry.id.as_str() {
                    "practice" => MenuAction::Tutorial {
                        resume: EVENTS[0].stage.into(),
                    },
                    "skip-practice" => MenuAction::Stage {
                        target: EVENTS[0].stage.into(),
                    },
                    other => return Err(format!("unexpected practice entry {other}").into()),
                };
            }
            // The Enter icon (template 0x299E70) precedes message 168.
            let at = layers
                .iter()
                .position(|l| l.asset == "choice-text-168")
                .ok_or("practice choice has no Enter text")?;
            layers.insert(at, enter_icon([346, 302, 130, 34]));
        }
    }

    let simon = |event: u16| (event <= 10071).then(|| simon_music.clone()).flatten();
    // 11000 ends with C2 = 100 (the voyage to Brest, campaign map) and
    // 11020 with C2 = 101 (the voyage to Milford Haven, then reveal 102 and
    // the free map): the campaign map plays both (`campaign_map::sail`).
    let next_of = |stage: &str| -> StoryEnd {
        let target = match stage {
            "simon-deck-intro" => "simon-deck-prompt",
            "simon-deck-prompt" => "deck-choice",
            "simon-deck-chosen" => "seto-arrival",
            "seto-arrival" => "seto-side-intro",
            "seto-side-intro" => "side-choice",
            // 11000 and 12000 both continue at map event 100 (the side
            // byte picks the red voyage or the White Rose reveal).
            "seto-leaves" | WHITE_STAGE => {
                return StoryEnd::MapEvent {
                    event: map::EVENT_FIRST_VOYAGE as u16,
                };
            }
            _ => {
                return StoryEnd::MapEvent {
                    event: map::EVENT_SECOND_VOYAGE as u16,
                };
            }
        };
        StoryEnd::Stage {
            target: target.into(),
        }
    };
    for (stage, event, pages) in stages {
        manifest.stages.push(Stage::Story {
            id: stage.into(),
            pages,
            music: simon(event),
            advance_buttons: vec![Button::Cross],
            on_end: next_of(stage),
        });
        if stage == "simon-deck-prompt" {
            manifest.stages.push(Stage::DeckChoice {
                id: "deck-choice".into(),
                layers: std::mem::take(&mut deck_layers),
                music: simon(10070),
                config: config.clone(),
                confirm_buttons: vec![Button::Start],
                on_confirm: StoryEnd::Stage {
                    target: "simon-deck-chosen".into(),
                },
            });
        }
        if stage == "seto-side-intro" {
            manifest.stages.push(Stage::Menu {
                id: "side-choice".into(),
                layers: std::mem::take(&mut side_layers),
                music: None,
                entries: std::mem::take(&mut side_entries),
                // 10080 end sets 0x53AA = 1 (Lancastrians).
                initial_selection: 1,
                confirm_buttons: vec![Button::Start],
                cancel_target: None,
                wrap_navigation: false,
            });
        }
    }

    emit_json(&report, Some(&output.join("disc.json")))?;
    emit_json(
        &serde_json::json!({
            "schema_version":"roses-story-to-map-extension-1",
            "post_name_manifest_sha256":loaded.manifest_sha256,
            "message_sha256":messages.message_sha256,
            "pages":records,
            "runtime_name_message_ids":NAME_MESSAGES,
            "deck_choice":{"candidate_table_ee":CANDIDATE_TABLE_EE,"info_table_ee":DECK_INFO_EE,
                "blank_name_code":blank_name_code,"records":STARTER_RECORDS},
            "story_parity_validated":false,
            "boundaries":[TUTORIAL_NOTE, WHITE_ROUTE_BOUNDARY, MAP_VOYAGES],
            "limitations":[
                "Pages are settled states: text reveal, page fades, panel slides, portrait and figure motion, the advance indicator and talking portraits are not reproduced.",
                "Deck select imports the original panel/title/Enter, leader name and frame, labels, average level digits/stars, attribute/race icons and selection arrows. The three-card stack and its fly-in/slide animation remain absent. The deck serial (rand) is not modelled.",
                "Music: track 25 continues through 10071; tracks 26 (10080..10090), 4 (map) and 27 (11020) are not imported, so those stages are silent.",
                "11020: the settled castle backdrop is imported from TEXEVE record 1 with the original 640x448 UV extent and letterbox masks. Its entrance slide from record 0, letterbox animation and transition timing are not reproduced. No standing figure is drawn (the original disables it).",
                "Map events 100 and 101/102 are explicit stand-ins; map event 0 is the final boundary."]
        }),
        Some(&output.join("import-report.json")),
    )?;
    let pending = output.join("manifest.pending.json");
    emit_json(&manifest, Some(&pending))?;
    Loaded::open(&pending)?;
    emit_json(&manifest, Some(&output.join("manifest.json")))?;
    eprintln!(
        "Story to the map imported: {}",
        output.join("manifest.json").display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(event: u16) -> &'static Event {
        EVENTS.iter().find(|e| e.event == event).unwrap()
    }

    #[test]
    fn page_lists_match_the_researched_dispatch() {
        let counts: Vec<usize> = EVENTS.iter().map(|e| e.pages.len()).collect();
        assert_eq!(counts, [8, 3, 1, 73, 5, 6, 30, 27]);
        for e in &EVENTS {
            let groups: Vec<u16> = page_layouts(e).iter().map(|l| l.group).collect();
            assert!(groups.windows(2).all(|w| w[0] <= w[1]));
            assert!(e.pages.iter().all(|&id| id > e.base));
        }
        assert_eq!(page_layouts(event(10080)).last().unwrap().group, 48);
        assert_eq!(page_layouts(event(11020)).last().unwrap().group, 19);
        assert_eq!(page_layouts(event(12000)).last().unwrap().group, 17);
        for id in NAME_MESSAGES {
            assert!(EVENTS.iter().any(|e| e.pages.contains(&id)));
        }
    }

    #[test]
    fn simon_events_keep_layout_two_without_a_top_panel() {
        for id in [10060, 10070, 10071] {
            for layout in page_layouts(event(id)) {
                assert_eq!((layout.top_text, layout.speaker), (false, SPEAKER_SIMON));
                assert_eq!((layout.top_panel, layout.pan()), (None, 100));
            }
        }
    }

    #[test]
    fn seto_arrival_follows_the_group_table_and_sticky_top_panel() {
        let layouts = page_layouts(event(10080));
        // Groups 1-2: Seto's text at the top, no panel yet, no figure (pan 0).
        assert!(layouts[..3]
            .iter()
            .all(|l| l.top_text && l.top_panel.is_none()));
        assert_eq!(layouts[0].pan(), 0);
        // Group 3 "Who's there!?" is Simon; group 4 turns Seto's panel on.
        assert_eq!((layouts[3].group, layouts[3].speaker), (3, SPEAKER_SIMON));
        assert_eq!(layouts[4].group, 4);
        assert_eq!(layouts[4].top_panel, Some(SPEAKER_SETO));
        assert_eq!(layouts[4].pan(), -100);
        // Group 5 is Simon's page with Seto's panel still shown.
        assert_eq!(
            (layouts[5].speaker, layouts[5].top_panel),
            (SPEAKER_SIMON, Some(SPEAKER_SETO))
        );
        assert_eq!(layouts[5].pan(), 100);
        let seto = layouts.iter().filter(|l| l.speaker == SPEAKER_SETO).count();
        let simon = layouts.len() - seto;
        assert_eq!((seto, simon), (50, 23));
        assert_eq!(layouts.last().unwrap().speaker, SPEAKER_SIMON);
    }

    #[test]
    fn side_intro_leaves_and_brest_follow_their_cases() {
        let side: Vec<u16> = page_layouts(event(10090))
            .iter()
            .map(|l| l.speaker)
            .collect();
        assert_eq!(side, [198, 198, 198, 197, 198]);
        assert!(page_layouts(event(10090))
            .iter()
            .all(|l| l.top_panel.is_some()));
        assert!(page_layouts(event(11000))
            .iter()
            .all(|l| l.top_text && l.top_panel == Some(SPEAKER_SETO)));
        let brest = page_layouts(event(11020));
        let yugi_groups: Vec<u16> = brest
            .iter()
            .filter(|l| l.speaker == SPEAKER_YUGI)
            .map(|l| l.group)
            .collect();
        assert!(yugi_groups
            .iter()
            .all(|g| *g == 4 || (6..=12).contains(g) || (14..=19).contains(g)));
        assert!(brest[..3]
            .iter()
            .all(|l| l.top_panel.is_none() && !l.top_text));
        let g5 = brest.iter().find(|l| l.group == 5).unwrap();
        assert_eq!(
            (g5.speaker, g5.top_panel),
            (SPEAKER_SIMON, Some(SPEAKER_YUGI))
        );
        assert!(brest.iter().all(|l| figure(11020, *l) == Figure::None));
        // 12000: Seto with his panel, Simon's page 3044, then no bottom panel.
        let white = page_layouts(event(12000));
        let simon: Vec<usize> = (0..white.len())
            .filter(|&i| white[i].speaker == SPEAKER_SIMON)
            .collect();
        assert_eq!(simon, [3]);
        assert_eq!((white[3].pan(), figure(12000, white[3])), (100, Figure::Simon));
        assert!(white.iter().all(|l| l.top_panel == Some(SPEAKER_SETO)));
        assert!(white[..4].iter().all(|l| l.bottom_panel));
        assert!(white[4..].iter().all(|l| !l.bottom_panel && l.pan() == -100));
    }

    #[test]
    fn top_panel_and_choices_use_the_traced_raw_geometry() {
        let name = roses_assets::RgbaImage {
            width: 48,
            height: 22,
            pixels: vec![0; 48 * 22 * 4],
        };
        let seto = top_panel(1, "p", "b", "n", &name);
        assert_eq!(seto[0].destination, [500, 0, 128, 126]);
        assert_eq!(seto[1].destination, [60, 14, 420, 28]);
        assert_eq!(seto[2].source, Some([224, 32, 32, 32]));
        assert_eq!(seto.last().unwrap().destination, [66, 18, 60, 19]);
        let ornament = &seto[seto.len() - 2];
        assert_eq!(ornament.source, Some([0, 23, 64, 21]));
        let yugi = top_panel(0, "p", "b", "n", &name);
        assert_eq!(yugi[2].source, Some([224, 0, 32, 32]));
        // Practice-choice panel: Enter frame (346,302), icon at +(11,9).
        let panel = choice_panel([27, 168, 458, 176]);
        assert_eq!(
            panel.last().unwrap().destination,
            raw_rect([357, 311, 16, 16])
        );
        assert_eq!(raw_origin(24, 164), [30, 144]);
        let wide = roses_assets::RgbaImage {
            width: 228,
            height: 22,
            pixels: vec![0; 228 * 22 * 4],
        };
        assert_eq!(
            fitted_text_layer("t", &wide, [24, 164], 172).destination,
            [30, 144, 215, 19]
        );
        assert_eq!(
            fitted_text_layer("t", &name, [24, 164], 172).destination,
            [30, 144, 60, 19]
        );
        assert_eq!(raw_rect([24, 132, 416, 288]), [30, 116, 520, 252]);
        let bar = label_bar(1);
        assert_eq!(&bar.pixels[..4], &[0, 48, 48, 255]);
        let middle = ((15 * 336) * 4) as usize;
        assert_eq!(&bar.pixels[middle..middle + 4], &[0, 96, 96, 255]);
    }

    #[test]
    #[ignore = "Requires the user's supported ISO (ROSES_TEST_ISO_PATH)"]
    fn private_pages_candidates_and_name_text_match_the_disc() {
        let iso = std::path::PathBuf::from(std::env::var_os("ROSES_TEST_ISO_PATH").unwrap());
        let (mut disc, _) = open_pal(&iso).unwrap();
        let bytes = disc.read_file("MSG/MSG_E.DAT", 300_000).unwrap();
        let executable = disc.read_file("SLES_515.79", 8 * 1024 * 1024).unwrap();
        // Every listed page is non-blank and every other counter is blank
        // (the dispatcher skips messages whose first raw token is 0xA001).
        for e in &EVENTS {
            let last = *e.pages.last().unwrap();
            let range: Vec<u16> = (e.base + 1..=last).collect();
            let messages = parse_pal_messages(&bytes, &range).unwrap();
            for m in &messages.messages {
                let blank = m.expanded_codes.iter().all(|&c| c & 0x3FFF == 0x2001);
                assert_eq!(
                    !blank,
                    e.pages.contains(&m.message_id),
                    "event {} message {} codes {:04x?}",
                    e.event,
                    m.message_id,
                    &m.expanded_codes[..m.expanded_codes.len().min(4)]
                );
            }
        }
        // FLOW.md test name: twelve 0x2005 → checksum 12 → decks 9, 13, 16.
        let word = |ee: usize| {
            let at = ee - EE_TO_FILE;
            u16::from_le_bytes([executable[at], executable[at + 1]])
        };
        let decks = roses_assets::parse_pal_decks(&executable).unwrap().decks;
        let config = DeckChoiceConfig {
            candidate_table: (0..16).map(|i| word(CANDIDATE_TABLE_EE + 2 * i)).collect(),
            blank_name_code: 0x2001,
            decks: (0..STARTER_RECORDS)
                .map(|r| StarterDeck {
                    record: r as u8,
                    leader: word(DECK_INFO_EE + 12 * r) & 0x3FF,
                    average_level_x100: word(DECK_INFO_EE + 12 * r + 2),
                    leader_name: String::new(),
                    cards: decks[r].cards.clone(),
                    presentation_layers: vec![],
                    card_face: None,
                })
                .collect(),
        };
        config.validate().unwrap();
        let name = roses_client::PlayerName {
            glyphs: vec![
                roses_client::NameCharacter {
                    page: 0,
                    row: 0,
                    column: 0,
                    display_code: 0x2005,
                    stored_code: 0x2005,
                };
                12
            ],
        };
        assert_eq!(config.checksum(&name).unwrap(), 12);
        assert_eq!(config.candidates(&name).unwrap(), [9, 13, 16]);
        let shown: Vec<(u16, u16)> = [9u8, 13, 16]
            .iter()
            .map(|&r| {
                let d = config.deck(r).unwrap();
                (d.leader, d.average_level_x100)
            })
            .collect();
        assert_eq!(shown, [(392, 347), (612, 329), (622, 359)]);
        assert!(config
            .decks
            .iter()
            .all(|d| decks[usize::from(d.record)].leader_card == d.leader));
        // Runtime name text equals offline template resolution (1, 5, 12 glyphs).
        let font = decode_pal_message_font(&bytes).unwrap();
        let templates = parse_pal_message_templates(&bytes, &NAME_MESSAGES).unwrap();
        let atlas = roses_client::Image {
            width: font.image.width,
            height: font.image.height,
            rgba: font.image.pixels.clone(),
        };
        let codes: Vec<u16> = (0..=u16::MAX)
            .filter(|&c| font.glyph_rect(c).is_some())
            .take(12)
            .collect();
        let cells: Vec<roses_client::NameCell> = codes
            .iter()
            .map(|&c| {
                let r = font.glyph_rect(c).unwrap();
                roses_client::NameCell {
                    stored_code: c,
                    cell: [r[0], r[1]],
                }
            })
            .collect();
        for length in [1, 5, 12] {
            let name = roses_client::PlayerName {
                glyphs: codes[..length]
                    .iter()
                    .map(|&c| roses_client::NameCharacter {
                        page: 0,
                        row: 0,
                        column: 0,
                        display_code: c,
                        stored_code: c,
                    })
                    .collect(),
            };
            for template in &templates.messages {
                let text = name_text_for(template, &font, [20, 42], &cells).unwrap();
                let (runtime, _) = text.compose(&atlas, Some(&name)).unwrap();
                let resolved =
                    roses_assets::resolve_pal_message_template(template, &codes[..length]).unwrap();
                let offline = message_image(&font, &resolved).unwrap();
                assert_eq!(
                    runtime.rgba, offline.pixels,
                    "message {}",
                    template.message_id
                );
            }
        }
    }
}
