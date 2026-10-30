//! Extend a validated local opening import with researched story presentation.
use crate::{emit_json, open_pal, Result};
use roses_assets::{decode_pal_message_font, parse_pal_messages, MessageFont, PalMessage};
use roses_client::{
    Asset, AssetKind, Button, Layer, Loaded, Manifest, MenuAction, Sampling, Stage, StoryEnd,
    StoryPage,
};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
mod name_entry;

pub fn run(iso: &Path, opening: &Path, output: &Path) -> Result<()> {
    if output.exists() {
        return Err(
            "story output must be a new directory; existing imports are never overwritten".into(),
        );
    }
    let loaded = Loaded::open(opening)?;
    let (mut disc, report) = open_pal(iso)?;
    if loaded.manifest.source.disc_sha256 != report.iso_sha256
        || Some(&loaded.manifest.source.executable_sha256) != report.executable_sha256.as_ref()
    {
        return Err("opening import does not match the supplied ISO".into());
    }
    let mut manifest = loaded.manifest.clone();
    check_opening(&manifest)?;
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(output)?;
    // Copy only authenticated, referenced assets. Never copy arbitrary adjacent files.
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
            return Err("source asset changed size while copying the opening import".into());
        }
        target.sync_all()?;
    }
    let story_report = append(&mut disc, output, &mut manifest)?;
    emit_json(&report, Some(&output.join("disc.json")))?;
    emit_json(&story_report, Some(&output.join("story-import.json")))?;
    emit_json(
        &serde_json::json!({
            "schema_version":"roses-story-extension-1",
            "opening_manifest_sha256":loaded.manifest_sha256,
            "complete_game_import":false,
            "story_parity_validated":false
        }),
        Some(&output.join("import-report.json")),
    )?;
    let pending = output.join("manifest.pending.json");
    emit_json(&manifest, Some(&pending))?;
    Loaded::open(&pending)?;
    emit_json(&manifest, Some(&output.join("manifest.json")))?;
    eprintln!("Story imported: {}", output.join("manifest.json").display());
    Ok(())
}

fn check_opening(manifest: &Manifest) -> Result<()> {
    if manifest.canvas.width != 640 || manifest.canvas.height != 448 {
        return Err("story composition requires the supported 640x448 opening canvas".into());
    }
    let entries = manifest
        .stages
        .iter()
        .find_map(|stage| match stage {
            Stage::Menu { id, entries, .. } if id == "main-menu" => Some(entries),
            _ => None,
        })
        .ok_or("opening main menu is missing")?;
    let entry = entries
        .iter()
        .find(|entry| entry.id == "new-game")
        .ok_or("opening New Game entry is missing")?;
    if !matches!(entry.action, MenuAction::Unavailable)
        || manifest
            .stages
            .iter()
            .any(|stage| stage.id() == "historical-story")
    {
        return Err("opening import already has a New Game destination".into());
    }
    Ok(())
}

pub fn append(
    disc: &mut roses_disc::Disc,
    output: &Path,
    manifest: &mut Manifest,
) -> Result<serde_json::Value> {
    check_opening(manifest)?;
    build_story(disc, output, manifest)
}

// Event 10020 initializes counter=1, base message=2031 and 11 groups of four.
// The dispatcher skips original blank messages (raw token 0xA001) and advances
// one remaining message per confirmed paragraph. These are IDs, never retail text.
const HISTORY_MESSAGES: [u16; 15] = [
    2032, 2036, 2037, 2040, 2044, 2045, 2048, 2052, 2053, 2060, 2064, 2068, 2069, 2072, 2073,
];
// Event 10040 uses base2075 and32 entries, skipping blank first-token0xA001
// messages. Its final paragraph leads into the original name-entry subphase.
const SIMON_MESSAGES: [u16; 12] = [
    2076, 2080, 2081, 2084, 2088, 2089, 2092, 2096, 2097, 2100, 2101, 2104,
];

pub(crate) fn story_layer(asset: &str, source: Option<[u32; 4]>, destination: [u32; 4]) -> Layer {
    Layer {
        asset: asset.into(),
        source,
        destination,
        tint: [255; 3],
        opacity: 255,
        flip_x: false,
        filter: Sampling::Linear,
    }
}

pub(crate) fn import_png(
    output: &Path,
    manifest: &mut Manifest,
    id: &str,
    image: &roses_assets::RgbaImage,
) -> Result<()> {
    let path = format!("story/{id}.png");
    crate::write_png(
        &output.join(&path),
        image.width,
        image.height,
        &image.pixels,
    )?;
    let bytes = fs::read(output.join(&path))?;
    manifest.assets.push(Asset {
        id: id.into(),
        kind: AssetKind::Png,
        path,
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        bytes: bytes.len() as u64,
    });
    Ok(())
}

pub(crate) fn message_image(
    font: &MessageFont,
    message: &PalMessage,
) -> Result<roses_assets::RgbaImage> {
    if message.lines.is_empty() || message.lines.len() > 3 {
        return Err("story paragraph exceeds its observed three-line text area".into());
    }
    let columns = message
        .lines
        .iter()
        .map(|line| line.tokens.len())
        .max()
        .unwrap_or(0);
    if columns == 0 || columns > 30 {
        return Err("story line exceeds its observed thirty-glyph text area".into());
    }
    // Original builder copies the LEFT12 pixels from each22x22 cell into rows
    // spaced22 pixels apart. Scaling full cells would distort the narrow font.
    let width = columns as u32 * 12;
    let height = message.lines.len() as u32 * 22;
    let mut image = roses_assets::RgbaImage {
        width,
        height,
        pixels: vec![0; (width * height * 4) as usize],
    };
    for (row, line) in message.lines.iter().enumerate() {
        for (column, token) in line.tokens.iter().enumerate() {
            let code = match token {
                roses_assets::MessageToken::Glyph { code, .. } => *code,
                roses_assets::MessageToken::Unknown { code } => {
                    return Err(format!("unsupported story message code {code:#06x}").into());
                }
            };
            let source = font
                .glyph_rect(code)
                .ok_or("story glyph has no verified font cell")?;
            for y in 0..22 {
                let from = (((source[1] + y) * font.image.width + source[0]) * 4) as usize;
                let to = (((row as u32 * 22 + y) * width + column as u32 * 12) * 4) as usize;
                image.pixels[to..to + 48].copy_from_slice(&font.image.pixels[from..from + 48]);
            }
        }
    }
    Ok(image)
}

fn build_story(
    disc: &mut roses_disc::Disc,
    output: &Path,
    manifest: &mut Manifest,
) -> Result<serde_json::Value> {
    let bytes = disc.read_file("MSG/MSG_E.DAT", 300_000)?;
    let mut ids = HISTORY_MESSAGES.to_vec();
    ids.extend(SIMON_MESSAGES);
    ids.push(197);
    let messages = parse_pal_messages(&bytes, &ids)?;
    let font = decode_pal_message_font(&bytes)?;
    let music = roses_assets::decode_pal_story_music(
        &disc.read_file(roses_assets::PAL_STORY_MUSIC_ISO_PATH, 10 * 1024 * 1024)?,
        &disc.read_file("SOUND/BGM/PCMLIST.BIN", 4096)?,
    )?;
    let native_music = music.original_rate_wav()?;
    fs::create_dir(output.join("story"))?;
    import_png(output, manifest, "story-font", &font.image)?;
    let mut wav = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("story/historical-music.wav"))?;
    wav.write_all(&native_music)?;
    wav.sync_all()?;
    manifest.assets.push(Asset {
        id: "historical-music".into(),
        kind: AssetKind::Wav,
        path: "story/historical-music.wav".into(),
        sha256: format!("{:x}", Sha256::digest(&native_music)),
        bytes: native_music.len() as u64,
    });
    emit_json(&music.metadata, Some(&output.join("story/music.json")))?;
    let artwork = disc.read_file(
        "DATA/TEXEVE.MRG",
        roses_assets::MAX_OPENING_ARCHIVE_BYTES as u64,
    )?;
    for record in [18, 19, 20, 21, 22] {
        let mut image = roses_assets::decode_pal_opening_texture(
            &artwork,
            roses_assets::OpeningArchiveKind::TexEve,
            record,
        )?;
        // Historical portraits use two original quads, split at source y184.
        // The second quad fades toward its bottom edge. Bake that vertical
        // modulation into a separately named derived asset, keeping originals.
        for y in 184..image.height {
            for x in 0..image.width {
                let index = ((y * image.width + x) * 4 + 3) as usize;
                image.pixels[index] = (u32::from(image.pixels[index]) * (image.height - y)
                    / (image.height - 184)) as u8;
            }
        }
        import_png(
            output,
            manifest,
            &format!("story-portrait-{record}"),
            &image,
        )?;
    }
    let mut pages = Vec::new();
    for message in &messages.messages[..HISTORY_MESSAGES.len()] {
        let mut layers = historical_portrait(message.message_id)
            .into_iter()
            .collect::<Vec<_>>();
        let text = message_image(&font, message)?;
        let text_id = format!("story-text-{}", message.message_id);
        import_png(output, manifest, &text_id, &text)?;
        // Original(89,384), converted from512x512 to640x448. Integer raster
        // rounds fractional origin/extent; exact filtering parity is pending.
        layers.push(story_layer(
            &text_id,
            None,
            [111, 336, text.width * 5 / 4, (text.height * 7 + 4) / 8],
        ));
        pages.push(StoryPage {
            id: format!("message-{}", message.message_id),
            layer_set: None,
            layers,
            name_text: None,
            pan: None,
        });
    }
    manifest.stages.push(Stage::Story {
        id: "historical-story".into(),
        pages,
        music: Some(roses_client::Music {
            asset: "historical-music".into(),
            looped: true,
            loop_start_frame: Some(music.metadata.loop_start_frame * 2),
        }),
        advance_buttons: vec![Button::Cross],
        on_end: StoryEnd::Stage {
            target: "simon-arrival".into(),
        },
    });
    let simon_music = roses_assets::decode_pal_simon_music(
        &disc.read_file(roses_assets::PAL_SIMON_MUSIC_ISO_PATH, 10 * 1024 * 1024)?,
        &disc.read_file("SOUND/BGM/PCMLIST.BIN", 4096)?,
    )?;
    let wave = simon_music.original_rate_wav()?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("story/simon-music.wav"))?;
    file.write_all(&wave)?;
    file.sync_all()?;
    manifest.assets.push(Asset {
        id: "simon-music".into(),
        kind: AssetKind::Wav,
        path: "story/simon-music.wav".into(),
        sha256: format!("{:x}", Sha256::digest(&wave)),
        bytes: wave.len() as u64,
    });
    let simon_music_config = roses_client::Music {
        asset: "simon-music".into(),
        looped: true,
        loop_start_frame: Some(simon_music.metadata.loop_start_frame * 2),
    };
    let small_portrait = roses_assets::decode_pal_opening_texture(
        &disc.read_file(
            "DATA/TEXANM.MRG",
            roses_assets::MAX_OPENING_ARCHIVE_BYTES as u64,
        )?,
        roses_assets::OpeningArchiveKind::TexAnm,
        0,
    )?;
    import_png(output, manifest, "simon-small-portrait", &small_portrait)?;
    let speaker = message_image(
        &font,
        messages.messages.last().ok_or("missing speaker message")?,
    )?;
    import_png(output, manifest, "simon-speaker", &speaker)?;
    let ui_atlas = roses_assets::decode_pal_opening_texture(
        &disc.read_file(
            "DATA/TEXSYS.MRG",
            roses_assets::MAX_OPENING_ARCHIVE_BYTES as u64,
        )?,
        roses_assets::OpeningArchiveKind::TexSys,
        7,
    )?;
    let common_layers = simon_layers(output, manifest, &speaker, &ui_atlas)?;
    let mut simon_pages = Vec::new();
    for message in
        &messages.messages[HISTORY_MESSAGES.len()..HISTORY_MESSAGES.len() + SIMON_MESSAGES.len()]
    {
        let text = message_image(&font, message)?;
        let id = format!("story-text-{}", message.message_id);
        import_png(output, manifest, &id, &text)?;
        let mut layers = common_layers.clone();
        layers.push(story_layer(
            &id,
            None,
            [170, 350, text.width * 5 / 4, (text.height * 7 + 4) / 8],
        ));
        simon_pages.push(StoryPage {
            id: format!("message-{}", message.message_id),
            layer_set: None,
            layers,
            name_text: None,
            pan: None,
        });
    }
    manifest.stages.push(Stage::Story {
        id: "simon-arrival".into(),
        pages: simon_pages,
        music: Some(simon_music_config.clone()),
        advance_buttons: vec![Button::Cross],
        on_end: StoryEnd::Stage {
            target: "name-entry".into(),
        },
    });
    name_entry::append(disc, output, manifest, &bytes, simon_music_config)?;
    emit_json(
        &simon_music.metadata,
        Some(&output.join("story/simon-music.json")),
    )?;
    for stage in &mut manifest.stages {
        if let Stage::Menu { id, entries, .. } = stage {
            if id == "main-menu" {
                if let Some(entry) = entries.iter_mut().find(|entry| entry.id == "new-game") {
                    entry.action = MenuAction::Stage {
                        target: "historical-story".into(),
                    };
                }
            }
        }
    }
    emit_json(&messages, Some(&output.join("story/messages.json")))?;
    emit_json(&font.metadata, Some(&output.join("story/font.json")))?;
    Ok(
        serde_json::json!({"schema_version":"roses-historical-story-1",
        "message_ids":HISTORY_MESSAGES,"message_sha256":messages.message_sha256,
        "music":music.metadata,
        "native_music":{"sample_rate":48000,"frame_count":music.metadata.frame_count*2,
            "loop_start_frame":music.metadata.loop_start_frame*2,
            "conversion":"Each original stereo frame repeated twice, matching the original CPU conversion routine"},
        "simon_message_ids":SIMON_MESSAGES,"simon_speaker_message_id":197,
        "simon_music":simon_music.metadata,
        "story_parity_validated":false,"name_entry_implemented":true,
        "presentation":"Historical paragraphs, Simon's pre-name dialogue and interactive name entry; incomplete scene rendering",
        "limitations":["The original cloudy 3D backdrop is not rendered; its area remains black.",
            "Portrait entrance/exit motion, page fades, text reveal and advance-indicator animation remain incomplete.",
            "Simon's small portrait is static; its expressions and timing are not reproduced. The dialogue's black lower mask follows the preserved screenshot boundary, not a recovered clipping rule.",
            "Raster rounding, filtering and text placement remain approximations; full original rendering parity is not established.",
            "Music conversion and loop are source-verified; native audible playback and final mixer parity are not validated."]}),
    )
}

fn simon_layers(
    output: &Path,
    manifest: &mut Manifest,
    speaker: &roses_assets::RgbaImage,
    atlas: &roses_assets::RgbaImage,
) -> Result<Vec<Layer>> {
    // Original red label fill is style0: RGB48 at its edges and RGB96 at center.
    let mut bar = roses_assets::RgbaImage {
        width: 336,
        height: 32,
        pixels: vec![0; 336 * 32 * 4],
    };
    for y in 0..32u32 {
        for x in 0..336u32 {
            let at = ((y * 336 + x) * 4) as usize;
            let red = 48 + 48 * y.min(31 - y) / 15;
            bar.pixels[at..at + 4].copy_from_slice(&[red as u8, 0, 0, 255]);
        }
    }
    import_png(output, manifest, "simon-label-bar", &bar)?;
    let black = roses_assets::RgbaImage {
        width: 1,
        height: 1,
        pixels: vec![0, 0, 0, 255],
    };
    import_png(output, manifest, "story-black", &black)?;
    let mut wing = roses_assets::RgbaImage {
        width: 56,
        height: 12,
        pixels: vec![0; 56 * 12 * 4],
    };
    for y in 0..12 {
        for x in 0..56 {
            let from = (((127 - y) * atlas.width + 69 + x) * 4) as usize;
            let to = ((y * 56 + x) * 4) as usize;
            wing.pixels[to..to + 4].copy_from_slice(&atlas.pixels[from..from + 4]);
        }
    }
    import_png(output, manifest, "simon-frame-wing", &wing)?;
    let mut layers = vec![
        story_layer("texeve-17", None, [190, 116, 260, 252]),
        story_layer("story-black", None, [0, 322, 640, 126]),
        story_layer(
            "simon-small-portrait",
            Some([0, 0, 128, 128]),
            [20, 322, 128, 126],
        ),
        story_layer("simon-label-bar", None, [210, 322, 420, 28]),
        story_layer("texsys-07", Some([224, 0, 32, 32]), [170, 322, 40, 28]),
    ];
    layers.extend(frame_layers([16, 368, 104, 144], [64, 0, 16, 14]));
    layers.extend(frame_layers([168, 368, 336, 32], [48, 64, 8, 8]));
    layers.push(story_layer(
        "simon-frame-wing",
        None,
        raw_rect([16, 358, 48, 12]),
    ));
    let mut right_wing = story_layer("simon-frame-wing", None, raw_rect([72, 358, 48, 12]));
    right_wing.flip_x = true;
    layers.push(right_wing);
    layers.push(story_layer(
        "texsys-07",
        Some([0, 0, 64, 23]),
        raw_rect([44, 349, 48, 23]),
    ));
    layers.push(story_layer(
        "simon-speaker",
        None,
        [
            216,
            326,
            speaker.width * 5 / 4,
            (speaker.height * 7 + 4) / 8,
        ],
    ));
    Ok(layers)
}

pub(crate) fn raw_rect([x, y, w, h]: [u32; 4]) -> [u32; 4] {
    [
        (x * 5 + 2) / 4,
        (y * 7 + 4) / 8,
        (w * 5 + 2) / 4,
        (h * 7 + 4) / 8,
    ]
}

/// Eight original atlas border pieces around a source-space rectangle.
pub(crate) fn frame_layers([x, y, w, h]: [u32; 4], [sx, sy, cw, ch]: [u32; 4]) -> Vec<Layer> {
    [
        ([sx, sy, cw, ch], [x, y, cw, ch]),
        ([sx + cw, sy, cw, ch], [x + w - cw, y, cw, ch]),
        ([sx, sy + ch, cw, ch], [x, y + h - ch, cw, ch]),
        ([sx + cw, sy + ch, cw, ch], [x + w - cw, y + h - ch, cw, ch]),
        ([sx + cw, sy, 1, ch], [x + cw, y, w - 2 * cw, ch]),
        (
            [sx + cw, sy + ch, 1, ch],
            [x + cw, y + h - ch, w - 2 * cw, ch],
        ),
        ([sx, sy + ch, cw, 1], [x, y + ch, cw, h - 2 * ch]),
        (
            [sx + cw, sy + ch, cw, 1],
            [x + w - cw, y + ch, cw, h - 2 * ch],
        ),
    ]
    .into_iter()
    .map(|(source, dest)| story_layer("texsys-07", Some(source), raw_rect(dest)))
    .collect()
}

fn historical_portrait(message_id: u16) -> Option<Layer> {
    // Code-derived resting endpoints, scaled from the original512x512 viewport
    // to our640x448 canvas and rounded to integer pixels. Continuation pages retain the layers
    // whose exit threshold has not been reached; Mai exits during message2073.
    let (record, destination) = match message_id {
        2040 => (20, [125, 116, 390, 224]),
        2044 | 2045 => (22, [190, 109, 260, 224]),
        2048 => (19, [190, 123, 260, 224]),
        2052 | 2053 => (18, [60, 116, 520, 224]),
        2072 => (21, [190, 123, 260, 224]),
        _ => return None,
    };
    let mut layer = story_layer(&format!("story-portrait-{record}"), None, destination);
    // Original ALPHA0x44 is(Cs-Cd)*As/128+Cd; modulated vertex alpha64
    // therefore yields a half-opacity portrait above the lower gradient.
    layer.opacity = 128;
    Some(layer)
}

/// Render the same native compositor used by the window, for reference audits.
/// No original screenshots are inputs to this operation.
pub fn render(manifest_path: &Path, output: &Path) -> Result<()> {
    if output.exists() {
        return Err("render output must be a new directory".into());
    }
    let loaded = Loaded::open(manifest_path)?;
    let stories: Vec<_> = loaded
        .manifest
        .stages
        .iter()
        .filter_map(|stage| match stage {
            Stage::Story {
                id,
                pages,
                advance_buttons,
                ..
            } => Some((id, pages, advance_buttons)),
            _ => None,
        })
        .collect();
    if stories.is_empty() {
        return Err("manifest has no story stages".into());
    }
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(output)?;
    let mut records = Vec::new();
    for (id, pages, advance_buttons) in stories {
        let mut manifest = loaded.manifest.clone();
        manifest.initial_stage = id.clone();
        let mut model = roses_client::Model::new(manifest)?;
        for (index, page) in pages.iter().enumerate() {
            // Pages that embed the player name need a committed name, which a
            // standalone render does not have: record them as skipped.
            let rendered = if page.name_text.is_some() {
                None
            } else {
                Some(loaded.render(&model)?)
            };
            let Some(mut pixels) = rendered else {
                records.push(serde_json::json!({"stage":id,"page":page.id,"index":index,
                    "skipped":"needs a committed player name"}));
                if index + 1 < pages.len()
                    && !matches!(
                        model.input(advance_buttons[0]),
                        roses_client::Outcome::Repaint
                    )
                {
                    return Err("story input did not advance to its next page".into());
                }
                continue;
            };
            for pixel in pixels.chunks_exact_mut(4) {
                pixel.swap(0, 2);
                pixel[3] = 255;
            }
            let filename = format!("{id}-{index:02}-{}.png", page.id);
            crate::write_png(
                &output.join(&filename),
                loaded.manifest.canvas.width,
                loaded.manifest.canvas.height,
                &pixels,
            )?;
            records.push(serde_json::json!({"stage":id,"page":page.id,"index":index,
                "file":filename,"rgba_sha256":format!("{:x}",<sha2::Sha256 as sha2::Digest>::digest(&pixels))}));
            if index + 1 < pages.len()
                && !matches!(
                    model.input(advance_buttons[0]),
                    roses_client::Outcome::Repaint
                )
            {
                return Err("story input did not advance to its next page".into());
            }
        }
    }
    for stage in &loaded.manifest.stages {
        let Stage::NameEntry { id, config, .. } = stage else {
            continue;
        };
        for page in 0..config.pages.len() {
            let mut manifest = loaded.manifest.clone();
            manifest.initial_stage = id.clone();
            let mut model = roses_client::Model::new(manifest)?;
            for _ in 0..page {
                model.input(Button::R1);
            }
            for entered in [false, true] {
                if entered {
                    for _ in 0..config.max_length {
                        model.input(Button::Cross);
                    }
                }
                let mut pixels = loaded.render(&model)?;
                for pixel in pixels.chunks_exact_mut(4) {
                    pixel.swap(0, 2);
                    pixel[3] = 255;
                }
                let filename = format!(
                    "{id}-page-{page}-{}.png",
                    if entered { "filled" } else { "empty" }
                );
                crate::write_png(
                    &output.join(&filename),
                    loaded.manifest.canvas.width,
                    loaded.manifest.canvas.height,
                    &pixels,
                )?;
                records.push(
                    serde_json::json!({"stage":id,"keyboard_page":page,"entered_name":entered,
                    "file":filename,"rgba_sha256":format!("{:x}",Sha256::digest(&pixels))}),
                );
            }
        }
    }
    emit_json(
        &serde_json::json!({"schema_version":"roses-story-render-1",
        "manifest_sha256":loaded.manifest_sha256,"native_render":true,
        "original_parity_validated":false,"pages":records}),
        Some(&output.join("render.json")),
    )?;
    Ok(())
}
