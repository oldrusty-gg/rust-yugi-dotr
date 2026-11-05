//! Settled starter-deck information panel, PAL 0x22538C..0x225D38.
//!
//! All labels, names, glyphs and icon tables come from the user's files.
//! This does not implement the separate 3D card-stack drawer 0x229EF0 or
//! the slide animation. Private draw-contract evidence is under
//! the project notes (the EE128 probe).

use crate::import_story::{frame_layers, import_png, message_image, raw_rect, story_layer};
use crate::Result;
use roses_assets::{
    decode_pal_name_font, parse_pal_messages, MessageFont, MessageToken, PalMessage,
};
use roses_client::{Layer, Manifest, StarterDeck};
use std::path::Path;

const EE_TO_FILE: usize = 0xF9680;
const INFO: usize = 0x2990E0;
const SPRITES: usize = 0x294480;

fn bytes(executable: &[u8], address: usize, count: usize) -> Result<&[u8]> {
    let offset = address
        .checked_sub(EE_TO_FILE)
        .ok_or("invalid panel address")?;
    executable
        .get(offset..offset.checked_add(count).ok_or("panel table overflow")?)
        .ok_or_else(|| "starter-deck panel table lies outside the executable".into())
}

fn half(executable: &[u8], address: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        bytes(executable, address, 2)?.try_into()?,
    ))
}

fn word(executable: &[u8], address: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes(executable, address, 4)?.try_into()?,
    ))
}

fn packed_four(value: u32, bits: u32) -> [u32; 4] {
    std::array::from_fn(|index| (value >> (index as u32 * bits)) & ((1 << bits) - 1))
}

/// Both source switches jump either to a sentinel or an `addiu r,zero,id`.
/// Read their immediates rather than embedding retail icon ordering.
fn mapped_sprite(executable: &[u8], race: bool, value: u32) -> Result<Option<u16>> {
    let (table, count, sentinel, opcode) = if race {
        (0x2D2340, 22, 0x225A04, 0x2405_0000)
    } else {
        (0x2D23A0, 7, 0x225858, 0x2403_0000)
    };
    if value >= count {
        return Ok(None);
    }
    let destination = word(executable, table + value as usize * 4)? as usize;
    if destination == sentinel {
        return Ok(None);
    }
    let instruction = word(executable, destination)?;
    if instruction & 0xFFFF_0000 != opcode {
        return Err("unrecognised starter-deck icon switch instruction".into());
    }
    Ok(Some(instruction as u16))
}

/// The story binds TEXSYS #6 to its sprite slot 1 (the HUD calls it slot 6).
fn icon(executable: &[u8], sprite: u16, destination: [u32; 4]) -> Result<Layer> {
    let at = SPRITES + usize::from(sprite) * 20;
    if half(executable, at + 16)? != 1 {
        return Err("starter-deck icon is not in the traced TEXSYS #6 slot".into());
    }
    let source = [
        u32::from(half(executable, at)?),
        u32::from(half(executable, at + 2)?),
        u32::from(half(executable, at + 4)?),
        u32::from(half(executable, at + 6)?),
    ];
    // Avoid accepting a different sprite descriptor format as a rectangle.
    if source[2] == 0
        || source[3] == 0
        || source[0] + source[2] > 256
        || source[1] + source[3] > 256
    {
        return Err("starter-deck sprite rectangle exceeds its original atlas".into());
    }
    Ok(story_layer(
        "texsys-06",
        Some(source),
        raw_rect(destination),
    ))
}

fn labels(executable: &[u8]) -> Result<Vec<Layer>> {
    // 0x1B7B50 size 1, flags 0: nine-pixel cells, centred glyphs. The
    // descriptor rectangles and cell width are still read from the file.
    let cell = u32::from(bytes(executable, 0x2BDCD2, 1)?[0]);
    let mut layers = Vec::new();
    for (address, x, y) in [
        (0x2D26EB, 26, 250),
        (0x2D26F7, 62, 265),
        (0x2D2704, 366, 206),
        (0x2D270D, 402, 221),
        (0x2D2718, 366, 267),
    ] {
        let text = bytes(executable, address, 32)?;
        let end = text
            .iter()
            .position(|&byte| byte == 0)
            .ok_or("unterminated panel label")?;
        for (index, &ch) in text[..end].iter().enumerate() {
            if ch == b' ' {
                continue;
            }
            if !ch.is_ascii_uppercase() {
                return Err("unsupported character in the original starter-deck label".into());
            }
            let at = 0x28FDF0 + 10 * (37 + usize::from(ch - b'A'));
            if bytes(executable, at, 1)?[0] != 6 {
                return Err("starter-deck label font is not TEXSYS #6".into());
            }
            let (u0, v0, u1, v1) = (
                half(executable, at + 2)?,
                half(executable, at + 4)?,
                half(executable, at + 6)?,
                half(executable, at + 8)?,
            );
            let (width, height) = (
                u32::from(u1.checked_sub(u0).ok_or("reversed panel glyph")?),
                u32::from(v1.checked_sub(v0).ok_or("reversed panel glyph")?),
            );
            if width > cell || width == 0 || height == 0 {
                return Err("starter-deck glyph exceeds its fixed cell".into());
            }
            // Preserve the source half-pixel centring until projection to
            // the opening compositor's integer 640x448 canvas.
            let x2 = (x + index as u32 * cell) * 2 + cell - width;
            let mut destination = raw_rect([0, y, width, height]);
            destination[0] = (x2 * 5 + 4) / 8;
            layers.push(story_layer(
                "texsys-06",
                Some([u32::from(u0), u32::from(v0), width, height]),
                destination,
            ));
        }
    }
    Ok(layers)
}

/// 0x225A88 uses the four-corner sprite drawer, not the rectangle drawer
/// used by the other icons. 0x225AC8 reverses the second quad's geometry;
/// flag 1 in 0x2177F0 also subtracts one texel from all four U coordinates.
fn arrows(executable: &[u8]) -> Result<Vec<Layer>> {
    let descriptor = 0x299EF0;
    let sprite = half(executable, descriptor + 36)?;
    let at = SPRITES + usize::from(sprite) * 20;
    if half(executable, at + 16)? != 0 {
        return Err("starter-deck arrow is not in the traced TEXSYS #7 slot".into());
    }
    let coordinates = |address| -> Result<Vec<u32>> {
        (0..8)
            .map(|index| half(executable, address + index * 2).map(u32::from))
            .collect()
    };
    let vertices = coordinates(descriptor)?;
    let uv = coordinates(at)?;
    let rectangle = |quad: &[u32]| {
        quad[0] < quad[2]
            && quad[1] < quad[5]
            && quad[0] == quad[4]
            && quad[1] == quad[3]
            && quad[2] == quad[6]
            && quad[5] == quad[7]
    };
    if !rectangle(&vertices)
        || vertices[0] != 0
        || vertices[1] != 0
        || !rectangle(&uv)
        || uv[0] == 0
        || uv[2] > 256
        || uv[5] > 256
        || bytes(executable, descriptor + 16, 4)? != [128; 4]
    {
        return Err("unrecognised starter-deck arrow quad".into());
    }
    let mut layers = Vec::new();
    for (x, flipped) in [(178, false), (326, true)] {
        let mut layer = story_layer(
            "texsys-07",
            Some([
                uv[0] - u32::from(flipped),
                uv[1],
                uv[2] - uv[0],
                uv[5] - uv[1],
            ]),
            raw_rect([x, 241, vertices[2], vertices[5]]),
        );
        layer.flip_x = flipped;
        layers.push(layer);
    }
    Ok(layers)
}

/// 0x222D50 selects full 22x22 cells from message 6's keyboard grid for
/// digits, and message 7 row 2 column 8 for the decimal point. Its four
/// quads start at (22,308), advance 16, and are 22x22.
fn number_cell(
    font: &MessageFont,
    grid: &PalMessage,
    row: usize,
    column: usize,
) -> Result<roses_assets::RgbaImage> {
    let token = grid
        .lines
        .get(row)
        .and_then(|line| line.tokens.get(column))
        .ok_or("starter-deck numeric glyph lies outside its source grid")?;
    let code = match token {
        MessageToken::Glyph { code, .. } | MessageToken::Unknown { code } => *code,
    };
    let source = font
        .name_glyph_rect(code)
        .ok_or("starter-deck numeric glyph is outside the verified wide font")?;
    let mut image = roses_assets::RgbaImage {
        width: 22,
        height: 22,
        pixels: vec![0; 22 * 22 * 4],
    };
    for y in 0..22u32 {
        let from = (((source[1] + y) * font.image.width + source[0]) * 4) as usize;
        let to = y as usize * 22 * 4;
        image.pixels[to..to + 88].copy_from_slice(&font.image.pixels[from..from + 88]);
    }
    Ok(image)
}

fn stats(executable: &[u8], record: u8, average: u16) -> Result<Vec<Layer>> {
    let at = INFO + usize::from(record) * 12;
    let mut layers = Vec::new();
    let full = u32::from(average / 100);
    let fraction = u32::from(average % 100) * 16 / 100;
    for index in 0..full {
        layers.push(icon(executable, 146, [26 + 16 * index, 286, 16, 16])?);
    }
    if fraction != 0 {
        let mut layer = icon(executable, 169, [26 + 16 * full, 286, fraction, 16])?;
        layer.source.as_mut().unwrap()[2] = fraction;
        layers.push(layer);
    }
    let digits = [average / 100 % 10, 10, average / 10 % 10, average % 10];
    for (index, digit) in digits.into_iter().enumerate() {
        layers.push(story_layer(
            &format!("deck-number-{digit}"),
            None,
            raw_rect([22 + index as u32 * 16, 308, 22, 22]),
        ));
    }
    // The scratch sprite survives both ranking loops. Index 3 copies its
    // current ID to the name icon even when that ranking entry is absent.
    let mut last_sprite = half(executable, 0x299E40 + 36)?;
    for (race, packed, bits, y) in [
        (false, u32::from(half(executable, at + 4)?), 3, 239),
        (true, word(executable, at + 8)?, 5, 283),
    ] {
        for (index, value) in packed_four(packed, bits).into_iter().enumerate() {
            if let Some(sprite) = mapped_sprite(executable, race, value)? {
                last_sprite = sprite;
                layers.push(icon(
                    executable,
                    sprite,
                    [452 - index as u32 * 28, y, 24, 24],
                )?);
            }
        }
        if race {
            layers.extend(frame_layers([14, 202, 187, 34], [64, 56, 16, 14]));
            layers.push(icon(executable, last_sprite, [19, 207, 24, 24])?);
        }
    }
    Ok(layers)
}

pub(super) fn import(
    executable: &[u8],
    message_bytes: &[u8],
    output: &Path,
    manifest: &mut Manifest,
    decks: &[StarterDeck],
) -> Result<Vec<Vec<Layer>>> {
    let font = decode_pal_name_font(message_bytes)?;
    let grids = parse_pal_messages(message_bytes, &[6, 7])?;
    for digit in 0..=10u16 {
        let (grid, row, column) = if digit == 10 {
            (&grids.messages[1], 2, 8)
        } else {
            (
                &grids.messages[0],
                4,
                usize::from(if digit == 0 { 9 } else { digit - 1 }),
            )
        };
        let image = number_cell(&font, grid, row, column)?;
        import_png(output, manifest, &format!("deck-number-{digit}"), &image)?;
    }
    let ids: Vec<u16> = decks.iter().map(|deck| 323 + deck.leader).collect();
    let names = parse_pal_messages(message_bytes, &ids)?;
    let labels = labels(executable)?;
    let mut result = Vec::new();
    for (deck, name) in decks.iter().zip(&names.messages) {
        if half(executable, INFO + usize::from(deck.record) * 12)? & 0x3FF != deck.leader
            || half(executable, INFO + usize::from(deck.record) * 12 + 2)?
                != deck.average_level_x100
        {
            return Err("starter-deck presentation differs from its imported deck record".into());
        }
        let name = message_image(&font, name)?;
        let id = format!("starter-leader-{}", deck.record);
        import_png(output, manifest, &id, &name)?;
        let mut layers = labels.clone();
        layers.extend(stats(executable, deck.record, deck.average_level_x100)?);
        layers.extend(arrows(executable)?);
        layers.push(super::fitted_text_layer(&id, &name, [45, 207], 152));
        result.push(layers);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranking_fields_are_read_low_first_without_sorting_or_counting() {
        assert_eq!(
            packed_four(1 | (6 << 3) | (2 << 6) | (4 << 9), 3),
            [1, 6, 2, 4]
        );
        assert_eq!(packed_four(31 | (7 << 5) | (21 << 15), 5), [31, 7, 0, 21]);
    }

    #[test]
    fn private_panel_matches_original_ee128_draw_contract() {
        let Some(directory) = std::env::var_os("ROSES_DECK_PANEL_ORACLE") else {
            return;
        };
        let root = Path::new(&directory);
        let executable = std::fs::read(root.join("../../../inspection/SLES_515.79")).unwrap();
        let cases: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("draw-calls-128.json")).unwrap())
                .unwrap();
        let cases = cases.as_array().unwrap();
        assert_eq!(cases.len(), 17, "oracle must contain every starter record");
        for (index, case) in cases.iter().enumerate() {
            let record = case["record"].as_u64().unwrap() as u8;
            assert_eq!(
                usize::from(record),
                index,
                "oracle records must be ordered and unique"
            );
            let average = case["info"][1].as_u64().unwrap() as u16;
            let native = stats(&executable, record, average).unwrap();
            let mut expected_icons = Vec::new();
            for call in case["calls"].as_array().unwrap() {
                if call["addr"] != "0x217360" {
                    continue;
                }
                let descriptor = call["descriptor"].as_array().unwrap();
                let sprite = descriptor[18].as_u64().unwrap() as u16;
                if sprite == 112 {
                    continue;
                } // common Enter icon
                let value = |i: usize| descriptor[i].as_i64().unwrap();
                let x = value(0) + i64::from(call["args"][2].as_u64().unwrap() as u32 as i32);
                let y = value(1) + i64::from(call["args"][3].as_u64().unwrap() as u32 as i32);
                let (w, h) = (value(2) - value(0), value(3) - value(1));
                if w == 0 {
                    continue;
                }
                let mut expected = icon(
                    &executable,
                    sprite,
                    [x as u32, y as u32, w as u32, h as u32],
                )
                .unwrap();
                if sprite == 169 {
                    expected.source.as_mut().unwrap()[2] = w as u32;
                }
                expected_icons.push((expected.source, expected.destination));
            }
            let actual_icons: Vec<_> = native
                .iter()
                .filter(|layer| layer.asset == "texsys-06")
                .map(|layer| (layer.source, layer.destination))
                .collect();
            assert_eq!(
                actual_icons, expected_icons,
                "record {record}: ordered icon/star draws"
            );
        }
    }

    #[test]
    fn private_arrows_match_original_ee128_quads() {
        let Some(directory) = std::env::var_os("ROSES_DECK_PANEL_ORACLE") else {
            return;
        };
        let root = Path::new(&directory);
        let executable = std::fs::read(root.join("../../../inspection/SLES_515.79")).unwrap();
        let cases: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("arrow-quads-128.json")).unwrap())
                .unwrap();
        let cases = cases.as_array().unwrap();
        assert_eq!(cases.len(), 34, "two arrow quads for all 17 records");
        let native = arrows(&executable).unwrap();
        for (index, case) in cases.iter().enumerate() {
            assert_eq!(case["record"].as_u64().unwrap(), index as u64 / 2);
            let flipped = index % 2 == 1;
            assert_eq!(case["flags"].as_u64().unwrap(), u64::from(flipped));
            assert_eq!(case["texture_slot"], 0);
            assert_eq!(case["filter"], 1);
            assert_eq!(case["rgba"], serde_json::json!([128, 128, 128, 128]));
            let values = |field: &str| -> Vec<u32> {
                case[field]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_u64().unwrap() as u32)
                    .collect()
            };
            let vertices = values("vertices");
            let uv = values("uv");
            let layer = &native[index % 2];
            assert_eq!(layer.asset, "texsys-07");
            assert_eq!(layer.flip_x, flipped);
            assert_eq!(
                layer.source,
                Some([uv[0], uv[1], uv[2] - uv[0], uv[5] - uv[1]])
            );
            assert_eq!(
                layer.destination,
                raw_rect([
                    vertices[0].min(vertices[2]),
                    vertices[1],
                    vertices[0].abs_diff(vertices[2]),
                    vertices[5] - vertices[1],
                ])
            );
        }
    }
}
