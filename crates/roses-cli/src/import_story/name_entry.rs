//! Original two-page keyboard and glyph mappings imported from the user's disc.
use super::*;
use roses_assets::{decode_pal_name_font, RgbaImage};
use roses_client::{NameEntryConfig, NameEntryGlyph, NameEntryPage, NameShadow};

fn blank(width: u32, height: u32) -> RgbaImage {
    RgbaImage {
        width,
        height,
        pixels: vec![0; (width * height * 4) as usize],
    }
}

/// Place an imported cell without filtering in the original512-coordinate plane.
fn paste(
    target: &mut RgbaImage,
    source: &RgbaImage,
    [sx, sy, w, h]: [u32; 4],
    x: u32,
    y: u32,
    tint: [u8; 3],
) {
    for dy in 0..h {
        for dx in 0..w {
            let from = (((sy + dy) * source.width + sx + dx) * 4) as usize;
            let to = (((y + dy) * target.width + x + dx) * 4) as usize;
            let alpha = u32::from(source.pixels[from + 3]);
            let old_alpha = u32::from(target.pixels[to + 3]);
            let out_alpha = alpha + old_alpha * (255 - alpha) / 255;
            if out_alpha == 0 {
                continue;
            }
            for (channel, &color) in tint.iter().enumerate() {
                let input = u32::from(source.pixels[from + channel]) * u32::from(color) / 255;
                let previous = u32::from(target.pixels[to + channel]);
                target.pixels[to + channel] = ((input * alpha
                    + previous * old_alpha * (255 - alpha) / 255)
                    / out_alpha) as u8;
            }
            target.pixels[to + 3] = out_alpha as u8;
        }
    }
}

fn cursor_image(atlas: &RgbaImage) -> RgbaImage {
    let mut image = blank(36, 36);
    for y in 0..36 {
        for x in 0..36 {
            if (8..28).contains(&x) && (8..28).contains(&y) {
                continue;
            }
            let sx = if x < 8 {
                64 + x
            } else if x >= 28 {
                72 + x - 28
            } else {
                72
            };
            let sy = if y < 8 {
                240 + y
            } else if y >= 28 {
                248 + y - 28
            } else {
                248
            };
            paste(&mut image, atlas, [sx, sy, 1, 1], x, y, [255; 3]);
        }
    }
    image
}

pub(super) fn append(
    disc: &mut roses_disc::Disc,
    output: &Path,
    manifest: &mut Manifest,
    messages: &[u8],
    music: roses_client::Music,
) -> Result<()> {
    let font = decode_pal_name_font(messages)?;
    import_png(output, manifest, "name-font", &font.image)?;
    let selected = parse_pal_messages(messages, &[6, 7, 9, 10, 19, 20, 168, 174, 170])?;
    let atlas = roses_assets::decode_pal_opening_texture(
        &disc.read_file(
            "DATA/TEXSYS.MRG",
            roses_assets::MAX_OPENING_ARCHIVE_BYTES as u64,
        )?,
        roses_assets::OpeningArchiveKind::TexSys,
        7,
    )?;
    import_png(output, manifest, "name-cursor", &cursor_image(&atlas))?;
    let mut pages = Vec::new();
    for page in 0..2 {
        let visual = &selected.messages[page];
        let stored = &selected.messages[page + 2];
        if visual.lines.len() != 6
            || stored.lines.len() != 6
            || visual
                .lines
                .iter()
                .chain(&stored.lines)
                .any(|line| line.tokens.len() != 13)
        {
            return Err("original name grid has unexpected dimensions".into());
        }
        let visual_codes: Vec<_> = visual
            .expanded_codes
            .iter()
            .copied()
            .filter(|&c| c != 0x2000)
            .collect();
        let stored_codes: Vec<_> = stored
            .expanded_codes
            .iter()
            .copied()
            .filter(|&c| c != 0x2000)
            .collect();
        let mut grid = blank(512, 512);
        let mut cells = Vec::new();
        let colors = [
            [96, 0, 0],
            [80, 80, 0],
            [48, 80, 0],
            [0, 96, 0],
            [0, 96, 96],
            [0, 0, 96],
            [64, 64, 96],
        ];
        for row in 0..6u32 {
            for column in 0..13u32 {
                let index = (row * 13 + column) as usize;
                let code = visual_codes[index];
                let source = font
                    .name_glyph_rect(code)
                    .ok_or("unverified name-grid font code")?;
                if font.glyph_rect(stored_codes[index]).is_none() {
                    return Err("unverified committed name glyph".into());
                }
                let mut color = [0, 0, 0];
                // The settled second page reverses each row's interpolation.
                let tint_column = if page == 0 { column } else { 12 - column };
                for channel in 0..3 {
                    color[channel] = (colors[row as usize][channel] * (12 - tint_column) / 12
                        + colors[row as usize + 1][channel] * tint_column / 12)
                        as u8;
                }
                let mut key = blank(28, 28);
                paste(&mut key, &atlas, [192, 176, 28, 28], 0, 0, [255; 3]);
                for pixel in key.pixels.chunks_exact_mut(4) {
                    for channel in 0..3 {
                        pixel[channel] = (u32::from(pixel[channel]) * u32::from(color[channel])
                            / 128)
                            .min(255) as u8;
                    }
                }
                paste(
                    &mut grid,
                    &key,
                    [0, 0, 28, 28],
                    26 + 36 * column,
                    160 + 36 * row,
                    [255; 3],
                );
                paste(
                    &mut grid,
                    &font.image,
                    source,
                    31 + 36 * column,
                    165 + 36 * row,
                    [0; 3],
                );
                paste(
                    &mut grid,
                    &font.image,
                    source,
                    29 + 36 * column,
                    163 + 36 * row,
                    [255; 3],
                );
                cells.push(NameEntryGlyph {
                    display_code: code,
                    stored_code: stored_codes[index],
                    asset: "name-font".into(),
                    source,
                });
            }
        }
        let id = format!("name-grid-{page}");
        import_png(output, manifest, &id, &grid)?;
        let mut layers = vec![story_layer(&id, None, [0, 0, 640, 448])];
        // Both shoulder-button labels describe the other available page.
        let page_label = message_image(&font, &selected.messages[5 - page])?;
        let id = format!("name-page-label-{page}");
        import_png(output, manifest, &id, &page_label)?;
        for x in [60, 386] {
            layers.push(story_layer(
                &id,
                None,
                raw_rect([x, 408, page_label.width, page_label.height]),
            ));
        }
        pages.push(NameEntryPage { layers, cells });
    }
    let mut layers = frame_layers([112, 88, 288, 40], [64, 0, 16, 14]);
    layers.extend(frame_layers([24, 400, 464, 68], [64, 56, 16, 14]));
    layers.push(story_layer(
        "texsys-07",
        Some([160, 240, 96, 16]),
        raw_rect([208, 74, 96, 16]),
    ));
    // The two original name-field end pieces rotate atlas coordinates.
    for (id, sx, sy, clockwise, x) in [
        ("name-left-ornament", 63, 84, false, 100),
        ("name-right-ornament", 64, 83, true, 400),
    ] {
        let mut piece = blank(12, 32);
        for y in 0..32 {
            for x in 0..12 {
                let (u, v) = if clockwise {
                    (sx + y, sy + 11 - x)
                } else {
                    (sx + 31 - y, sy + x)
                };
                paste(&mut piece, &atlas, [u, v, 1, 1], x, y, [255; 3]);
            }
        }
        import_png(output, manifest, id, &piece)?;
        layers.push(story_layer(id, None, raw_rect([x, 92, 12, 32])));
    }
    for (source, destination) in [
        ([44, 176, 22, 22], [32, 436, 22, 22]),
        ([22, 176, 22, 22], [57, 436, 22, 22]),
        ([66, 176, 22, 22], [200, 436, 22, 22]),
        ([0, 209, 22, 11], [32, 414, 22, 11]),
        ([0, 198, 22, 11], [360, 414, 22, 11]),
        ([80, 240, 16, 16], [360, 440, 16, 16]),
    ] {
        layers.push(story_layer(
            "texsys-07",
            Some(source),
            raw_rect(destination),
        ));
    }
    for (index, x) in [(6, 83), (7, 228), (8, 386)] {
        let text = message_image(&font, &selected.messages[index])?;
        let id = format!("name-help-{}", selected.messages[index].message_id);
        import_png(output, manifest, &id, &text)?;
        layers.push(story_layer(
            &id,
            None,
            raw_rect([x, 436, text.width, text.height]),
        ));
    }
    append_heading(disc, output, manifest, &mut layers)?;
    let cursor_rects = (0..6)
        .flat_map(|row| (0..13).map(move |col| raw_rect([22 + 36 * col, 156 + 36 * row, 36, 36])))
        .collect();
    let name_positions = (0..12)
        .map(|i| raw_rect([124 + 22 * i, 97, 22, 22]))
        .collect();
    manifest.stages.push(Stage::NameEntry{id:"name-entry".into(),layers,music:Some(music),
        config:NameEntryConfig{rows:6,columns:13,max_length:12,pages,
            cursor:story_layer("name-cursor",None,raw_rect([22,156,36,36])),cursor_rects,
            name_positions,name_tint:[255;3],
            name_shadow:Some(NameShadow{offset:[3,2],tint:[0;3],opacity:255})},
        on_complete:StoryEnd::Unsupported{message:"Your name is entered. The following conversation, story choices and connected first duel are still under development.".into()}});
    emit_json(&selected, Some(&output.join("story/name-messages.json")))?;
    emit_json(&font.metadata, Some(&output.join("story/name-font.json")))?;
    emit_json(
        &serde_json::json!({"schema_version":"roses-name-entry-1",
        "original_message_ids":{"display_pages":[6,7],"committed_pages":[9,10]},
        "maximum_length":12,"initial_name_empty":true,
        "controls":{"insert":["cross","square"],"delete":"circle","page":["l1","r1"],"confirm_nonempty":"start"},
        "parity_validated":false,"limitations":["The3Dscene remains absent.","Raster rounding, frame details, key shading and animation remain incomplete.","Name-dependent starter-deck selection and persistent saves remain unimplemented."]}),
        Some(&output.join("story/name-entry.json")),
    )?;
    Ok(())
}

fn append_heading(
    disc: &mut roses_disc::Disc,
    output: &Path,
    manifest: &mut Manifest,
    layers: &mut Vec<Layer>,
) -> Result<()> {
    // Read the heading from the authenticated executable. Never embed retail UI text.
    let executable = disc.read_file("SLES_515.79", 2_000_000)?;
    let start = 0x2D26E0 - 0xF9680;
    let span = executable
        .get(start..start + 32)
        .ok_or("name heading exceeds executable")?;
    let end = span
        .iter()
        .position(|&byte| byte == 0)
        .ok_or("unterminated name heading")?;
    if end == 0 {
        return Err("empty name heading".into());
    }
    let atlas = roses_assets::decode_pal_opening_texture(
        &disc.read_file(
            "DATA/TEXSYS.MRG",
            roses_assets::MAX_OPENING_ARCHIVE_BYTES as u64,
        )?,
        roses_assets::OpeningArchiveKind::TexSys,
        6,
    )?;
    let mut glyphs = Vec::new();
    let mut width = 0;
    for &byte in &span[..end] {
        if byte == b' ' {
            width += 11;
            continue;
        }
        let index = match byte {
            b'A'..=b'Z' => usize::from(byte - b'A'),
            _ => return Err("unverified heading character".into()),
        };
        let offset = 0x28FDF0 - 0xF9680 + (74 + index) * 10;
        let fields = executable
            .get(offset..offset + 10)
            .ok_or("heading font table exceeds executable")?;
        let fields: Vec<u32> = fields
            .chunks_exact(2)
            .map(|v| u32::from(u16::from_le_bytes([v[0], v[1]])))
            .collect();
        if fields[0] != 6
            || fields[1] >= fields[3]
            || fields[2] >= fields[4]
            || fields[3] > atlas.width
            || fields[4] > atlas.height
            || fields[4] - fields[2] != 24
        {
            return Err("unverified heading font table record".into());
        }
        let rect = [fields[1], fields[2], fields[3] - fields[1], 24];
        glyphs.push((width, rect));
        width += rect[2];
    }
    if width > 236 {
        return Err("heading exceeds its frame".into());
    }
    let mut image = blank(width, 24);
    for (x, source) in glyphs {
        paste(&mut image, &atlas, source, x, 0, [255; 3]);
    }
    import_png(output, manifest, "name-heading", &image)?;
    let mut bar = blank(236, 32);
    for y in 0..32 {
        for x in 0..236 {
            let at = ((y * 236 + x) * 4) as usize;
            let blue = 48 + 48 * y.min(31 - y) / 15;
            bar.pixels[at..at + 4].copy_from_slice(&[0, 0, blue as u8, 255]);
        }
    }
    import_png(output, manifest, "name-heading-bar", &bar)?;
    layers.push(story_layer(
        "name-heading-bar",
        None,
        raw_rect([28, 28, 236, 32]),
    ));
    layers.extend(frame_layers([28, 28, 236, 32], [48, 64, 8, 8]));
    layers.push(story_layer(
        "name-heading",
        None,
        raw_rect([71, 35, image.width, image.height]),
    ));
    Ok(())
}
