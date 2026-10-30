//! Extend a validated name-entry import with Simon's post-name dialogue.
//!
//! Source (PAL executable, see private post-name-flow.json): event 10041 uses
//! base message 2107 with counter 1..48 and event 10050 base 2155 with counter
//! 1..28; the shared dispatcher skips blank messages, leaving the visible IDs
//! below. Both use speaker message 197, layout 2 and no per-page overrides
//! (0x22DA08/0x22DA44), i.e. the same settled layout as Simon's pre-name pages.
//! Only 2112 and 2136 reference the mutable player name (message 3). These are
//! message IDs, never retail text. The practice choice after 10050 is not yet
//! implemented and stays an explicit boundary.
use crate::{emit_json, open_pal, Result};
use roses_assets::{
    decode_pal_message_font, parse_pal_message_templates, parse_pal_messages, MessageTemplateToken,
};
use roses_client::{
    Button, Layer, Loaded, Manifest, NameCell, NameText, Stage, StoryEnd, StoryPage, TextCell,
};
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::Path;

const POST_NAME_MESSAGES: [u16; 20] = [
    2108, 2112, 2116, 2117, 2120, 2121, 2124, 2125, 2128, 2129, 2132, 2133, 2136, 2140, 2141, 2144,
    2145, 2148, 2149, 2152,
];
const PRACTICE_INTRO_MESSAGES: [u16; 11] = [
    2156, 2157, 2160, 2164, 2168, 2169, 2172, 2173, 2176, 2177, 2180,
];
const NAME_MESSAGES: [u16; 2] = [2112, 2136];
const NEWLINE: u16 = 0x2000;
/// Practice choice labels 254/255 and the Enter prompt 168 (0x224DB4..0x225354).
const CHOICE_MESSAGES: [u16; 3] = [254, 255, 168];

pub fn run(iso: &Path, opening: &Path, output: &Path) -> Result<()> {
    if output.exists() {
        return Err(
            "post-name output must be a new directory; imports are never overwritten".into(),
        );
    }
    let loaded = Loaded::open(opening)?;
    let (mut disc, report) = open_pal(iso)?;
    if loaded.manifest.source.disc_sha256 != report.iso_sha256
        || Some(&loaded.manifest.source.executable_sha256) != report.executable_sha256.as_ref()
    {
        return Err("name-entry import does not match the supplied ISO".into());
    }
    let mut manifest = loaded.manifest.clone();
    if manifest
        .stages
        .iter()
        .any(|stage| stage.id() == "simon-post-name")
    {
        return Err("import already contains the post-name dialogue".into());
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
    let static_ids: Vec<u16> = POST_NAME_MESSAGES
        .iter()
        .chain(&PRACTICE_INTRO_MESSAGES)
        .chain(&CHOICE_MESSAGES)
        .copied()
        .filter(|id| !NAME_MESSAGES.contains(id))
        .collect();
    let messages = parse_pal_messages(&bytes, &static_ids)?;
    let templates = parse_pal_message_templates(&bytes, &NAME_MESSAGES)?;
    let font = decode_pal_message_font(&bytes)?;
    // Reuse Simon's settled page layers (speaker, portraits, panels) exactly:
    // every Simon page is those shared layers plus its text layer last.
    let (common, text_origin, music) = simon_common(&manifest)?;
    let name_cells = name_cells(&manifest, &font)?;

    let page_for = |id: u16, manifest: &mut Manifest| -> Result<StoryPage> {
        let mut layers = common.clone();
        let mut name_text = None;
        if let Some(template) = templates.messages.iter().find(|m| m.message_id == id) {
            name_text = Some(name_text_for(template, &font, text_origin, &name_cells)?);
        } else {
            let message = messages
                .messages
                .iter()
                .find(|m| m.message_id == id)
                .ok_or("post-name message missing")?;
            let text = crate::import_story::message_image(&font, message)?;
            let asset = format!("story-text-{id}");
            crate::import_story::import_png(output, manifest, &asset, &text)?;
            layers.push(crate::import_story::story_layer(
                &asset,
                None,
                [
                    text_origin[0],
                    text_origin[1],
                    text.width * 5 / 4,
                    (text.height * 7 + 4) / 8,
                ],
            ));
        }
        Ok(StoryPage {
            id: format!("message-{id}"),
            layer_set: None,
            layers,
            name_text,
            pan: None,
        })
    };
    let mut post_name = Vec::new();
    for id in POST_NAME_MESSAGES {
        post_name.push(page_for(id, &mut manifest)?);
    }
    let mut practice_intro = Vec::new();
    for id in PRACTICE_INTRO_MESSAGES {
        practice_intro.push(page_for(id, &mut manifest)?);
    }
    for stage in &mut manifest.stages {
        if let Stage::NameEntry {
            id, on_complete, ..
        } = stage
        {
            if id == "name-entry" {
                *on_complete = StoryEnd::Stage {
                    target: "simon-post-name".into(),
                };
            }
        }
    }
    manifest.stages.push(Stage::Story {
        id: "simon-post-name".into(),
        pages: post_name,
        music: music.clone(),
        advance_buttons: vec![Button::Cross],
        on_end: StoryEnd::Stage {
            target: "simon-practice-intro".into(),
        },
    });
    manifest.stages.push(Stage::Story {
        id: "simon-practice-intro".into(),
        pages: practice_intro,
        music: music.clone(),
        advance_buttons: vec![Button::Cross],
        on_end: StoryEnd::Stage {
            target: "practice-choice".into(),
        },
    });
    let choice = practice_choice(output, &mut manifest, &messages, &font, &common, music)?;
    manifest.stages.push(choice);
    emit_json(&report, Some(&output.join("disc.json")))?;
    emit_json(
        &serde_json::json!({
            "schema_version":"roses-post-name-extension-1",
            "opening_manifest_sha256":loaded.manifest_sha256,
            "post_name_message_ids":POST_NAME_MESSAGES,
            "practice_intro_message_ids":PRACTICE_INTRO_MESSAGES,
            "runtime_name_message_ids":NAME_MESSAGES,
            "message_sha256":messages.message_sha256,
            "story_parity_validated":false,
            "limitations":["Page layout reuses Simon's settled pre-name layers; text reveal, box fades and portrait motion are not reproduced.",
                "The practice choice is static (settled state): open/close animation, the Enter icon and held Up/Down suppression are not reproduced; both choices stop at explicit boundaries (practice duel and event 10060 are not implemented)."]
        }),
        Some(&output.join("import-report.json")),
    )?;
    let pending = output.join("manifest.pending.json");
    emit_json(&manifest, Some(&pending))?;
    Loaded::open(&pending)?;
    emit_json(&manifest, Some(&output.join("manifest.json")))?;
    eprintln!(
        "Post-name dialogue imported: {}",
        output.join("manifest.json").display()
    );
    Ok(())
}

pub(crate) fn name_text_for(
    template: &roses_assets::PalMessageTemplate,
    font: &roses_assets::MessageFont,
    origin: [u32; 2],
    name_cells: &[NameCell],
) -> Result<NameText> {
    let cell = |code: u16| -> Result<[u32; 2]> {
        let rect = font
            .glyph_rect(code)
            .ok_or_else(|| format!("post-name code {code:#06x} has no verified font cell"))?;
        Ok([rect[0], rect[1]])
    };
    let mut lines = vec![Vec::new()];
    for token in &template.tokens {
        match token {
            MessageTemplateToken::Direct { code, .. } if *code == NEWLINE => lines.push(Vec::new()),
            MessageTemplateToken::Direct { code, .. } => lines
                .last_mut()
                .unwrap()
                .push(TextCell::Glyph { cell: cell(*code)? }),
            MessageTemplateToken::PlayerNameSlot { slot, .. } => lines
                .last_mut()
                .unwrap()
                .push(TextCell::PlayerName { slot: *slot }),
        }
    }
    Ok(NameText {
        font: "story-font".into(),
        origin,
        lines,
        name_cells: name_cells.to_vec(),
    })
}

/// Settled practice choice (UI state 2). Source facts from the layout routine
/// 0x224DB4 and panel routine 0x218E70: panel raw (27,168,458,176) style 2 with
/// fill selector 2 (RGB 0,0,0) at GS alpha 64; the selected row gets a style-8
/// frame at raw (92,200|256,328,33) with fill selector 3, which the panel
/// routine draws without any fill (0x219254 → 0x219548); labels
/// 254/255 at raw (100,204|260) with vertex colour 128 selected, 64 otherwise;
/// Enter panel style 9 at raw (346,302,130,34) with message 168 at raw (382,308).
/// Fills are inset 2 raw pixels (0x219028..0x2190D0). Start confirms; choice 0
/// runs practice setup 0x23B590(0) and choice 1 continues to event 10060: both
/// are unavailable here. Not reproduced: the open/close animation, the Enter
/// icon sprite (template 0x299E70) and held Up/Down confirmation suppression.
/// Simon's settled dialogue layers stay (portraits, speaker bar, lower frame)
/// with no page text, as in the saved original practice-choice screenshot.
fn practice_choice(
    output: &Path,
    manifest: &mut Manifest,
    messages: &roses_assets::PalMessages,
    font: &roses_assets::MessageFont,
    common: &[Layer],
    music: Option<roses_client::Music>,
) -> Result<Stage> {
    use crate::import_story::{frame_layers, import_png, message_image, raw_rect, story_layer};

    let fill = |asset: &str, [x, y, w, h]: [u32; 4]| {
        let mut layer = story_layer(asset, None, raw_rect([x + 2, y + 2, w - 4, h - 4]));
        layer.opacity = 128; // GS alpha 64 of 128
        layer
    };
    let text = |id: u16, raw: [u32; 2], manifest: &mut Manifest| -> Result<Layer> {
        let message = messages
            .messages
            .iter()
            .find(|m| m.message_id == id)
            .ok_or("choice message missing")?;
        let image = message_image(font, message)?;
        let asset = format!("choice-text-{id}");
        import_png(output, manifest, &asset, &image)?;
        let at = raw_rect([raw[0], raw[1], 0, 0]);
        Ok(story_layer(
            &asset,
            None,
            [
                at[0],
                at[1],
                image.width * 5 / 4,
                (image.height * 7 + 4) / 8,
            ],
        ))
    };
    let mut layers: Vec<Layer> = common.to_vec();

    let panel = [27, 168, 458, 176];
    layers.push(fill("story-black", panel));
    layers.extend(frame_layers(panel, [48, 64, 8, 8]));
    let enter = [346, 302, 130, 34];
    layers.extend(frame_layers(enter, [64, 56, 16, 14]));
    layers.push(text(168, [382, 308], manifest)?);
    let mut entries = Vec::new();
    for (index, (id, raw_y, highlight_y)) in
        [(254, 204, 200), (255, 260, 256)].into_iter().enumerate()
    {
        let label = text(id, [100, raw_y], manifest)?;
        let mut dim = label.clone();
        dim.tint = [128; 3]; // vertex colour 64 of 128
        let row = [92, highlight_y, 328, 33];
        let mut selected = Vec::new();
        selected.extend(frame_layers(row, [0, 224, 16, 14]));
        selected.push(label);
        entries.push(roses_client::MenuEntry {
            id: if index == 0 {
                "practice".into()
            } else {
                "skip-practice".into()
            },
            label: format!("message-{id}"),
            normal_layers: vec![dim],
            selected_layers: selected,
            action: roses_client::MenuAction::Unavailable,
            campaign_side: None,
        });
    }
    Ok(Stage::Menu {
        id: "practice-choice".into(),
        layers,
        music,
        entries,
        initial_selection: 0,
        confirm_buttons: vec![Button::Start],
        cancel_target: None,
        wrap_navigation: false,
    })
}

pub(crate) type Common = (Vec<Layer>, [u32; 2], Option<roses_client::Music>);
pub(crate) fn simon_common(manifest: &Manifest) -> Result<Common> {
    let (pages, music) = manifest
        .stages
        .iter()
        .find_map(|stage| match stage {
            Stage::Story {
                id, pages, music, ..
            } if id == "simon-arrival" => Some((pages, music.clone())),
            _ => None,
        })
        .ok_or("import has no Simon pre-name dialogue")?;
    let first = pages.first().ok_or("Simon dialogue has no pages")?;
    let (text, common) = first
        .layers
        .split_last()
        .ok_or("Simon page has no layers")?;
    if !text.asset.starts_with("story-text-")
        || pages.iter().any(|page| {
            page.layers.len() != first.layers.len()
                || page.layers[..common.len()]
                    .iter()
                    .zip(common)
                    .any(|(a, b)| a.asset != b.asset || a.destination != b.destination)
        })
    {
        return Err("Simon pages do not share one layout followed by a text layer".into());
    }
    Ok((
        common.to_vec(),
        [text.destination[0], text.destination[1]],
        music,
    ))
}

pub(crate) fn name_cells(
    manifest: &Manifest,
    font: &roses_assets::MessageFont,
) -> Result<Vec<NameCell>> {
    let config = manifest
        .stages
        .iter()
        .find_map(|stage| match stage {
            Stage::NameEntry { config, .. } => Some(config),
            _ => None,
        })
        .ok_or("import has no name entry")?;
    let mut cells: Vec<NameCell> = Vec::new();
    for glyph in config.pages.iter().flat_map(|page| &page.cells) {
        if cells.iter().any(|c| c.stored_code == glyph.stored_code) {
            continue;
        }
        let rect = font.glyph_rect(glyph.stored_code).ok_or_else(|| {
            format!(
                "name code {:#06x} has no verified narrow dialogue cell",
                glyph.stored_code
            )
        })?;
        cells.push(NameCell {
            stored_code: glyph.stored_code,
            cell: [rect[0], rect[1]],
        });
    }
    Ok(cells)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Requires the user's supported ISO (ROSES_TEST_ISO_PATH)"]
    fn private_runtime_name_text_equals_offline_template_resolution() {
        let iso = std::path::PathBuf::from(std::env::var_os("ROSES_TEST_ISO_PATH").unwrap());
        let (mut disc, _) = open_pal(&iso).unwrap();
        let bytes = disc.read_file("MSG/MSG_E.DAT", 300_000).unwrap();
        let font = decode_pal_message_font(&bytes).unwrap();
        let templates = parse_pal_message_templates(&bytes, &NAME_MESSAGES).unwrap();
        let atlas = roses_client::Image {
            width: font.image.width,
            height: font.image.height,
            rgba: font.image.pixels.clone(),
        };
        // Narrow codes that the name keyboard stores (any supported cell works).
        let codes: Vec<u16> = (0..=u16::MAX)
            .filter(|&c| font.glyph_rect(c).is_some())
            .take(12)
            .collect();
        let cells: Vec<NameCell> = codes
            .iter()
            .map(|&c| {
                let r = font.glyph_rect(c).unwrap();
                NameCell {
                    stored_code: c,
                    cell: [r[0], r[1]],
                }
            })
            .collect();
        for length in [1, 2, 5, 12] {
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
                let text = name_text_for(template, &font, [170, 350], &cells).unwrap();
                let (runtime, _) = text.compose(&atlas, Some(&name)).unwrap();
                let resolved =
                    roses_assets::resolve_pal_message_template(template, &codes[..length]).unwrap();
                let offline = crate::import_story::message_image(&font, &resolved).unwrap();
                assert_eq!(
                    (runtime.width, runtime.height),
                    (offline.width, offline.height)
                );
                assert_eq!(
                    runtime.rgba, offline.pixels,
                    "message {}",
                    template.message_id
                );
            }
        }
    }
}
