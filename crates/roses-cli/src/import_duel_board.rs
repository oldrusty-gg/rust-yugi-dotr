//! `roses import-duel-board`: decode the original duel board models (terrain
//! tiles, arena, leader windows) and the executable's terrain maps from the
//! user's ISO into a new private directory (default `work/imports/...`).
//! Output: raw RH0 geometry JSON, GS-byte PNG textures, the overlay textures
//! (TEXSYS #2 card frames/back, #5 leader emblems), the executable's GS dither
//! matrix table and a hashed manifest.
use crate::{emit_json, open_pal, write_png, Result};
use roses_assets::{
    BoardModel, BoardRole, DuelBoardGeometry, OpeningArchiveKind, OpeningPaletteFormat,
    MAX_OPENING_ARCHIVE_BYTES,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub const SCHEMA: &str = "roses-duel-board-import-2";
const MODEL_ISO_PATH: &str = "DATA/MODEL.MRG";
/// Three consecutive 64-bit GS DIMX values in the fingerprinted executable
/// (EE 0x2931F8). The duel frame's dump uses entry 0 for the board and the
/// cursor tint pass, 1 for the HUD and offscreen units, 2 for the card image
/// and the cell/unit overlays.
pub const DIMX_TABLE_FILE_OFFSET: usize = 0x19_9B78;
/// TEXSYS records drawn by the board overlays (dump draws 6991 and 6992).
const TEXSYS_CARD_FRAMES: usize = 2;
const TEXSYS_EMBLEMS: usize = 5;

#[derive(Serialize)]
struct FileRef {
    path: String,
    sha256: String,
    bytes: u64,
}

#[derive(Serialize)]
struct TextureRef {
    index: usize,
    path: String,
    sha256: String,
    width: u32,
    height: u32,
}

#[derive(Serialize)]
struct ModelRef {
    id: String,
    #[serde(flatten)]
    role: BoardRole,
    ordinal: u16,
    file_offset: u64,
    record_sha256: String,
    draws: usize,
    strips: usize,
    vertices: usize,
    matrix_slots: u8,
    textures: Vec<TextureRef>,
}

pub fn model_id(role: BoardRole) -> String {
    match role {
        BoardRole::Terrain { code } => format!("terrain-{code}"),
        BoardRole::Arena { slot } => format!("arena-{slot}"),
        BoardRole::Window { side, leader_kind } => format!("window-{side}-{leader_kind}"),
    }
}

/// GS view of a 16-bit CLUT texel: RGB5 expanded by a left shift of three
/// (not rescaled), alpha bit -> TEXA TA1 (128, stored as PNG 255) or TA0 (0).
pub fn gs_rgba(texture: &roses_assets::OpeningIndexedTexture) -> Result<Vec<u8>> {
    if texture.palette_format != OpeningPaletteFormat::Rgb5551 {
        return Err("board textures with 32-bit palettes are not supported".into());
    }
    let mut pixels = Vec::with_capacity(texture.palette_indices.len() * 4);
    for index in &texture.palette_indices {
        let value = texture.palette_words_raw[usize::from(*index)];
        pixels.extend([
            ((value & 31) << 3) as u8,
            (((value >> 5) & 31) << 3) as u8,
            (((value >> 10) & 31) << 3) as u8,
            if value & 0x8000 != 0 { 255 } else { 0 },
        ]);
    }
    Ok(pixels)
}

/// Decode one DIMX value: DMij (row i = y & 3, column j = x & 3) is a signed
/// 3-bit field at bit 16 * i + 4 * j; the fourth bit of every nibble is zero.
pub fn dither_matrix(value: u64) -> Result<[[i8; 4]; 4]> {
    if value & 0x8888_8888_8888_8888 != 0 {
        return Err("DIMX value has bits outside its 3-bit fields".into());
    }
    Ok(std::array::from_fn(|i| {
        std::array::from_fn(|j| {
            let field = ((value >> (16 * i + 4 * j)) & 7) as i8;
            if field >= 4 {
                field - 8
            } else {
                field
            }
        })
    }))
}

fn dither_table(executable: &[u8]) -> Result<[[[i8; 4]; 4]; 3]> {
    let bytes = executable
        .get(DIMX_TABLE_FILE_OFFSET..DIMX_TABLE_FILE_OFFSET + 24)
        .ok_or("executable is too short for the DIMX table")?;
    let mut out = [[[0; 4]; 4]; 3];
    for (index, entry) in out.iter_mut().enumerate() {
        let value = u64::from_le_bytes(bytes[index * 8..index * 8 + 8].try_into().unwrap());
        *entry = dither_matrix(value)?;
    }
    Ok(out)
}

fn file_ref(output: &Path, path: &str) -> Result<FileRef> {
    let bytes = fs::read(output.join(path))?;
    Ok(FileRef {
        path: path.into(),
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        bytes: bytes.len() as u64,
    })
}

fn models(geometry: &DuelBoardGeometry) -> impl Iterator<Item = &BoardModel> {
    geometry
        .terrain
        .iter()
        .chain(&geometry.arena)
        .chain(&geometry.windows)
}

pub fn run(iso: &Path, output: &Path) -> Result<()> {
    if output.exists() {
        return Err(
            "duel board output must be a new directory; existing imports are never overwritten"
                .into(),
        );
    }
    let (mut disc, report) = open_pal(iso)?;
    let executable = disc.read_file("SLES_515.79", 8 * 1024 * 1024)?;
    // parse_pal_rules authenticates the executable before the DIMX table is read.
    let rules = roses_assets::parse_pal_rules(&executable)?;
    let dither = dither_table(&executable)?;
    let archive = disc.read_file(MODEL_ISO_PATH, 32 * 1024 * 1024)?;
    let geometry = roses_assets::decode_pal_duel_board(&archive)?;
    let texsys = disc.read_file(
        OpeningArchiveKind::TexSys.iso_path(),
        MAX_OPENING_ARCHIVE_BYTES as u64,
    )?;
    let texsys_sha256 = format!("{:x}", Sha256::digest(&texsys));
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(output)?;
    fs::create_dir(output.join("textures"))?;
    let mut refs = Vec::new();
    for model in models(&geometry) {
        let id = model_id(model.role);
        let mut textures = Vec::new();
        for index in 0..model.textures.len() {
            let texture = roses_assets::decode_pal_duel_board_texture(&archive, model, index)?;
            let path = format!("textures/{id}-t{index}.png");
            write_png(
                &output.join(&path),
                texture.width,
                texture.height,
                &gs_rgba(&texture)?,
            )?;
            let file = file_ref(output, &path)?;
            textures.push(TextureRef {
                index,
                path,
                sha256: file.sha256,
                width: texture.width,
                height: texture.height,
            });
        }
        refs.push(ModelRef {
            id,
            role: model.role,
            ordinal: model.ordinal,
            file_offset: model.file_offset,
            record_sha256: model.source_sha256.clone(),
            draws: model.draws.len(),
            strips: model.strips.len(),
            vertices: model.strips.iter().map(|s| s.vertices.len()).sum(),
            matrix_slots: model.matrix_slots,
            textures,
        });
    }
    let mut overlay = serde_json::Map::new();
    for (name, index) in [
        ("card_frames", TEXSYS_CARD_FRAMES),
        ("emblems", TEXSYS_EMBLEMS),
    ] {
        let texture = roses_assets::decode_pal_opening_indexed_texture(
            &texsys,
            OpeningArchiveKind::TexSys,
            index,
        )?;
        let path = format!("textures/texsys-{index}.png");
        write_png(
            &output.join(&path),
            texture.width,
            texture.height,
            &gs_rgba(&texture)?,
        )?;
        let file = file_ref(output, &path)?;
        overlay.insert(
            name.into(),
            serde_json::json!({
                "source": format!("{} record {index}", OpeningArchiveKind::TexSys.iso_path()),
                "path": path,
                "sha256": file.sha256,
                "width": texture.width,
                "height": texture.height,
            }),
        );
    }
    emit_json(&geometry, Some(&output.join("geometry.json")))?;
    emit_json(&report, Some(&output.join("disc.json")))?;
    let geometry_file = file_ref(output, "geometry.json")?;
    let maps: Vec<&[u8]> = rules.maps.iter().map(|m| m.cells.as_slice()).collect();
    emit_json(
        &serde_json::json!({
            "schema": SCHEMA,
            "importer_version": env!("CARGO_PKG_VERSION"),
            "profile": roses_disc::PAL_PROFILE_ID,
            "source": {
                "iso_sha256": report.iso_sha256,
                "executable_sha256": rules.executable_sha256,
                "model_archive": {
                    "iso_path": MODEL_ISO_PATH,
                    "sha256": geometry.archive_sha256,
                    "bytes": archive.len(),
                },
                "texsys_archive": {
                    "iso_path": OpeningArchiveKind::TexSys.iso_path(),
                    "sha256": texsys_sha256,
                },
            },
            "overlay_textures": overlay,
            "gs_dither_matrices": {
                "table_file_offset": DIMX_TABLE_FILE_OFFSET,
                "layout": "rows y & 3, columns x & 3 (signed DIMX fields)",
                "board": dither[0],
                "hud": dither[1],
                "overlay": dither[2],
            },
            "geometry": geometry_file,
            "texture_encoding": "PNG RGB = GS expansion of the 16-bit CLUT entry (5-bit value << 3); PNG alpha 255 = CLUT alpha bit set (GS alpha 128 via TEXA TA1), 0 otherwise",
            "terrain_maps": {
                "table_file_offset": rules.map_table_file_offset,
                "cell_order": "row-major: index = row * 7 + column",
                "maps": maps,
            },
            "models": refs,
            "unsupported": geometry.unsupported,
            "not_included": [
                "arena records #3/#4 (end pieces): geometry packet form not decoded",
                "runtime node animation (node flag 0x8000: arena #2 backdrop and crush node 0 are rotated at runtime; rate unresolved)",
                "crush terrain extra nodes: draws are tagged with their matrix block only",
                "unit models, face-up card images, HUD"
            ],
            "complete_game_import": false,
            "rendering_validated": false,
        }),
        Some(&output.join("manifest.json")),
    )?;
    eprintln!(
        "Imported {} terrain, {} arena and {} window models into {}",
        geometry.terrain.len(),
        geometry.arena.len(),
        geometry.windows.len(),
        output.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gs_texel_expansion_shifts_and_maps_the_alpha_bit() {
        let texture = roses_assets::OpeningIndexedTexture {
            width: 2,
            height: 1,
            palette_format: OpeningPaletteFormat::Rgb5551,
            palette_words_raw: (0..256)
                .map(|i| if i == 1 { 0xffff } else { 0x7c1f })
                .collect(),
            palette_indices: vec![0, 1],
        };
        assert_eq!(
            gs_rgba(&texture).unwrap(),
            [248, 0, 248, 0, 248, 248, 248, 255]
        );
        let mut wide = texture;
        wide.palette_format = OpeningPaletteFormat::Rgba8888Alpha128;
        assert!(gs_rgba(&wide).is_err());
        assert_eq!(
            model_id(BoardRole::Window {
                side: 1,
                leader_kind: 9
            }),
            "window-1-9"
        );
        // Signed 3-bit fields, row-major by y: DM00 = -1 (7), DM01 = 2, DM13 = -4.
        let m = dither_matrix(0x0000_0000_4000_0027).unwrap();
        assert_eq!((m[0][0], m[0][1], m[1][3], m[3][3]), (-1, 2, -4, 0));
        assert!(dither_matrix(0x8).is_err());
    }

    #[test]
    #[ignore = "Requires the user's supported ISO (ROSES_TEST_ISO_PATH)"]
    fn private_board_models_decode_from_the_iso() {
        let iso = std::path::PathBuf::from(std::env::var_os("ROSES_TEST_ISO_PATH").unwrap());
        let mut disc = roses_disc::Disc::open(&iso).unwrap();
        let archive = disc.read_file(MODEL_ISO_PATH, 32 * 1024 * 1024).unwrap();
        let geometry = roses_assets::decode_pal_duel_board(&archive).unwrap();
        assert_eq!(geometry.terrain.len(), 10);
        assert_eq!(geometry.arena.len(), 3);
        assert_eq!(geometry.windows.len(), 42);
        assert_eq!(geometry.unsupported.len(), 2);
        // Counts from the independent private research parser (board-spec.json).
        let counts = |m: &BoardModel| -> Vec<(usize, usize)> {
            (0..m.draws.len())
                .map(|d| {
                    let strips: Vec<_> = m
                        .strips
                        .iter()
                        .filter(|s| s.draw_index as usize == d)
                        .collect();
                    (strips.len(), strips.iter().map(|s| s.vertices.len()).sum())
                })
                .collect()
        };
        let expected: [&[(usize, usize)]; 13] = [
            &[(31, 144), (3, 14)],
            &[(23, 94), (10, 36), (8, 42), (2, 14)],
            &[(14, 104), (4, 16)],
            &[(1, 4), (3, 14)],
            &[(13, 104), (2, 12)],
            &[(17, 120), (3, 14)],
            &[(10, 55), (10, 59), (2, 12)],
            &[(4, 20), (14, 110), (4, 20), (2, 12)],
            &[(9, 33), (15, 57), (8, 30), (8, 40), (4, 16)],
            &[(18, 84), (28, 124), (3, 14)],
            &[
                (17, 92),
                (48, 264),
                (24, 104),
                (74, 480),
                (16, 80),
                (64, 258),
                (133, 894),
                (28, 112),
                (1, 4),
            ],
            &[(36, 220), (593, 3402)],
            &[(8, 32), (16, 48)],
        ];
        for (model, expected) in geometry.terrain.iter().chain(&geometry.arena).zip(expected) {
            assert_eq!(counts(model), expected, "{}", model_id(model.role));
        }
        for window in &geometry.windows {
            assert_eq!(counts(window), [(2, 17), (22, 164)]);
        }
        assert_eq!(geometry.terrain[9].matrix_slots, 2);
        assert_eq!(
            geometry.terrain[9]
                .draws
                .iter()
                .map(|d| d.matrix_slot)
                .collect::<Vec<_>>(),
            [0, 1, 1]
        );
        // Node rotation Y word 0x7FFF on wasteland and labyrinth only.
        let rotated: Vec<_> = geometry
            .terrain
            .iter()
            .filter(|m| m.node_zero_rotation_words[1] == 0x7fff)
            .map(|m| m.ordinal)
            .collect();
        assert_eq!(rotated, [246, 253]);
        // Untextured strips carry the black side faces (material all zero).
        let forest = &geometry.terrain[0];
        assert!(forest
            .strips
            .iter()
            .all(|s| s.textured == (s.draw_index == 0)));
        for model in models(&geometry) {
            for index in 0..model.textures.len() {
                let texture =
                    roses_assets::decode_pal_duel_board_texture(&archive, model, index).unwrap();
                assert_eq!(
                    gs_rgba(&texture).unwrap().len(),
                    (texture.width * texture.height * 4) as usize
                );
            }
        }
        let executable = disc.read_file("SLES_515.79", 8 * 1024 * 1024).unwrap();
        roses_assets::parse_pal_rules(&executable).unwrap();
        let dither = dither_table(&executable).unwrap();
        // Entry 2 (overlays) only rounds upward; entry 0 (board) is signed.
        assert!(dither[2].iter().flatten().all(|v| (0..=3).contains(v)));
        assert!(dither[0].iter().flatten().any(|v| *v < 0));
        let sizes: Vec<_> = geometry.arena[0]
            .textures
            .iter()
            .map(|t| (t.width, t.height))
            .collect();
        assert_eq!(
            sizes,
            [
                (256, 256),
                (128, 128),
                (128, 128),
                (64, 64),
                (64, 64),
                (64, 32)
            ]
        );
    }
}
