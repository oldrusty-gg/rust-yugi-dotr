//! The title menu's "Online Play" entry: a port addition below the
//! original four (New Game, Continue, Custom Duel, Trade), which stay as
//! the import draws them. The label is written with the game's own story
//! font (the `story-font` asset, first-bank cells: 12 of each cell's 22
//! columns, as the original's text renderer advances) and, when chosen,
//! framed with the same frame pieces the original entries use, sized to
//! the smaller label. Confirming it reports `Outcome::Unavailable("Online
//! Play")` like the other title entries the game handles itself.

use roses_client::{Layer, Manifest, MenuAction, MenuEntry, Sampling, Stage};

pub const ONLINE_ENTRY_ID: &str = "online-play";
pub const ONLINE_LABEL: &str = "Online Play";
/// The story font's asset id and cell size.
const FONT: &str = "story-font";
const CELL: u32 = 22;
const COLUMNS: u32 = 16;
const ADVANCE: u32 = 12;
/// Label scale and placement on the 640×448 title: centred, below Trade's
/// frame (which ends at y 371).
const SCALE_NUM: u32 = 3;
const SCALE_DEN: u32 = 2;
const LABEL_Y: u32 = 392;

/// The message-font cell of an ASCII character (the code − 0x2000 of
/// `roses_assets` text, e.g. 'A' = 0x2005); `None` for a space or a
/// character the font lacks.
pub fn font_cell(c: char) -> Option<u32> {
    Some(match c {
        'A'..='Z' => 0x05 + (c as u32 - 'A' as u32),
        'a'..='z' => 0x1F + (c as u32 - 'a' as u32),
        '1'..='9' => 0x39 + (c as u32 - '1' as u32),
        '0' => 0x42,
        ',' => 0x03,
        '-' => 0x43,
        '#' => 0x47,
        '&' => 0x4A,
        '\'' => 0x4B,
        '(' => 0x4C,
        ')' => 0x4D,
        '=' => 0x4E,
        '.' => 0x72,
        '!' => 0x75,
        '?' => 0x76,
        ':' => 0x9C,
        ';' => 0xA2,
        '<' => 0x96,
        '>' => 0x97,
        '%' => 0x49,
        '$' => 0x48,
        '"' => 0x46,
        '[' => 0x73,
        ']' => 0x74,
        '/' => 0x9D,
        _ => return None,
    })
}

fn label_layers(text: &str, x0: u32, y: u32, tint: [u8; 3]) -> Vec<Layer> {
    let mut out = Vec::new();
    for (i, c) in text.chars().enumerate() {
        let Some(cell) = font_cell(c) else { continue };
        out.push(Layer {
            asset: FONT.into(),
            source: Some([cell % COLUMNS * CELL, cell / COLUMNS * CELL, ADVANCE, CELL]),
            destination: [
                x0 + i as u32 * ADVANCE * SCALE_NUM / SCALE_DEN,
                y,
                ADVANCE * SCALE_NUM / SCALE_DEN,
                CELL * SCALE_NUM / SCALE_DEN,
            ],
            tint,
            opacity: 255,
            flip_x: false,
            filter: Sampling::Linear,
        });
    }
    out
}

/// The plate behind the label: New Game's plate (the first entry's
/// image) without its words: its left end, its blank middle repeated, its
/// right end, at the original entries' horizontal scale (5/4).
fn plate_layers(asset: &str, sy: u32, rect: [u32; 4], tint: [u8; 3]) -> Vec<Layer> {
    let [x, y, w, h] = rect;
    let layer = |source: [u32; 4], destination: [u32; 4]| Layer {
        asset: asset.into(),
        source: Some(source),
        destination,
        tint,
        opacity: 255,
        flip_x: false,
        filter: Sampling::Linear,
    };
    let mut out = vec![layer([0, sy, 32, 48], [x, y, 40, h]), layer([224, sy, 32, 48], [x + w - 40, y, 40, h])];
    let mut mx = x + 40;
    while mx < x + w - 40 {
        let step = 30.min(x + w - 40 - mx);
        out.push(layer([16, sy, step * 4 / 5, 48], [mx, y, step, h]));
        mx += step;
    }
    out
}

/// Map a coordinate of the original entry's frame (around the label span
/// `from`) to the same place around `to`: outside the span it moves with
/// the nearer edge, inside it scales.
fn remap(v: u32, from: (u32, u32), to: (u32, u32)) -> u32 {
    let (f0, f1) = (from.0 as i64, from.1 as i64);
    let (t0, t1) = (to.0 as i64, to.1 as i64);
    let v = v as i64;
    let r = if v <= f0 {
        v - f0 + t0
    } else if v >= f1 {
        v - f1 + t1
    } else {
        t0 + (v - f0) * (t1 - t0) / (f1 - f0)
    };
    r.max(0) as u32
}

/// Add the entry to the title menu (`main-menu`) of a story manifest that
/// has the story font; anything else is left as it is.
pub fn add_online_entry(manifest: &mut Manifest) {
    if !manifest.assets.iter().any(|a| a.id == FONT) {
        return;
    }
    for stage in &mut manifest.stages {
        let Stage::Menu { id, entries, .. } = stage else { continue };
        if id != "main-menu" || entries.iter().any(|e| e.id == ONLINE_ENTRY_ID) {
            continue;
        }
        let (Some(first), Some(last)) = (entries.first().cloned(), entries.last().cloned()) else { continue };
        let (Some(plate), Some(label)) = (first.normal_layers.first(), last.normal_layers.first()) else { continue };
        let Some(plate_source) = plate.source else { continue };
        let [lx, ly, lw, lh] = label.destination;
        let text_w = ONLINE_LABEL.chars().count() as u32 * ADVANCE * SCALE_NUM / SCALE_DEN;
        let text_h = CELL * SCALE_NUM / SCALE_DEN;
        // The plate: 260 × 36, centred, its top at LABEL_Y.
        let rect = [190, LABEL_Y, 260, 36];
        let tx = 320 - text_w / 2;
        let ty = LABEL_Y + (36 - text_h) / 2;
        // The selection frame of the last entry, moved around this plate.
        let frame: Vec<Layer> = last
            .selected_layers
            .iter()
            .filter(|l| l.asset != label.asset || l.source != label.source)
            .map(|l| {
                let [x, y, w, h] = l.destination;
                let (x0, x1) = (remap(x, (lx, lx + lw), (rect[0], rect[0] + rect[2])), remap(x + w, (lx, lx + lw), (rect[0], rect[0] + rect[2])));
                let (y0, y1) = (remap(y, (ly, ly + lh), (rect[1], rect[1] + rect[3])), remap(y + h, (ly, ly + lh), (rect[1], rect[1] + rect[3])));
                Layer { destination: [x0, y0, x1.saturating_sub(x0).max(1), y1.saturating_sub(y0).max(1)], ..l.clone() }
            })
            .collect();
        let mut normal = plate_layers(&plate.asset, plate_source[1], rect, [128, 128, 128]);
        normal.extend(label_layers(ONLINE_LABEL, tx, ty, [128, 128, 128]));
        let mut selected = plate_layers(&plate.asset, plate_source[1], rect, [255, 255, 255]);
        selected.extend(label_layers(ONLINE_LABEL, tx, ty, [255, 255, 255]));
        selected.extend(frame);
        entries.push(MenuEntry {
            id: ONLINE_ENTRY_ID.into(),
            label: ONLINE_LABEL.into(),
            normal_layers: normal,
            selected_layers: selected,
            action: MenuAction::Unavailable,
            campaign_side: None,
        });
    }
}
