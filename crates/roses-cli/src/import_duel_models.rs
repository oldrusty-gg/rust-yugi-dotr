//! `roses import-duel-models`: decode the duel's 3D unit models (MONSTER.MRG,
//! one 1 MiB slot per monster card; leaders use their card's slot) from the
//! user's ISO into a new private directory (default use: `work/imports/`).
//!
//! Default selection: the cards and leaders of the first duel's two decks
//! (deck records 0x09 and 0x1C of the executable's static table) plus the
//! fusion results reachable from them and every ritual result
//! ([`first_duel_cards`]); `--cards all` decodes all 683 slots. Output: one compact `models/<card>.json`
//! (`roses_assets::MonsterModel`: skeleton, draws, raw vertex lanes, GS state,
//! motions), GS-byte PNG textures, the executable's sine table and unit DIMX
//! entry, and a manifest with SHA-256 values for every file and source.
//! The whole MONSTER.MRG is also copied verbatim (`MONSTER.MRG`, with every
//! slot's SHA-256 in the manifest's `archive`) so the renderer can decode a
//! card the selection left out (effect results such as a metamorphosis).
use crate::import_duel_board::{dither_matrix, DIMX_TABLE_FILE_OFFSET};
use crate::{emit_json, open_pal, write_png, Result};
use roses_assets::{
    MonsterModel, MonsterTextureRef, OpeningIndexedTexture, PAL_MONSTER_ARCHIVE_BYTES,
    PAL_MONSTER_ARCHIVE_ISO_PATH, PAL_MONSTER_ARCHIVE_SHA256, PAL_MONSTER_SLOT_BYTES,
    PAL_MONSTER_SLOT_COUNT,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

pub const SCHEMA: &str = "roses-duel-models-import-1";
/// Static deck records of the first duel (player deck and Weevil's).
pub const FIRST_DUEL_DECKS: [usize; 2] = [0x09, 0x1C];
/// The verbatim MONSTER.MRG copy inside the import.
pub const ARCHIVE_FILE: &str = "MONSTER.MRG";
/// DIMX table entry used by the offscreen unit pass (dump draws 6997-7006).
const UNIT_DIMX_ENTRY: usize = 1;
const SECTOR: u64 = 2048;

/// Selected slots of the ISO's MONSTER.MRG, read by byte range.
pub struct MonsterArchive {
    file: File,
    start: u64,
}

impl MonsterArchive {
    /// Locate `DATA/MONSTER.MRG` in an opened disc; the ISO path must be the
    /// same image (the importer fingerprints it first).
    pub fn open(iso: &Path, disc: &roses_disc::Disc) -> Result<Self> {
        let entry = disc
            .entries()
            .iter()
            .find(|e| !e.is_directory && e.path.eq_ignore_ascii_case(PAL_MONSTER_ARCHIVE_ISO_PATH))
            .ok_or("DATA/MONSTER.MRG is missing from the ISO")?;
        if entry.size != PAL_MONSTER_ARCHIVE_BYTES {
            return Err("MONSTER.MRG has an unsupported length".into());
        }
        let file = File::open(iso)?;
        let start = u64::from(entry.extent_lba) * SECTOR;
        if file.metadata()?.len() < start + entry.size {
            return Err("MONSTER.MRG extent lies outside the ISO".into());
        }
        Ok(Self { file, start })
    }

    /// Hash the complete archive and require the supported fingerprint.
    pub fn verify(&mut self) -> Result<String> {
        self.file.seek(SeekFrom::Start(self.start))?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; PAL_MONSTER_SLOT_BYTES];
        for _ in 0..PAL_MONSTER_SLOT_COUNT {
            self.file.read_exact(&mut buffer)?;
            hasher.update(&buffer);
        }
        let sha256 = format!("{:x}", hasher.finalize());
        if sha256 != PAL_MONSTER_ARCHIVE_SHA256 {
            return Err(format!("unsupported MONSTER.MRG SHA-256 {sha256}").into());
        }
        Ok(sha256)
    }

    pub fn slot(&mut self, card: u16) -> Result<Vec<u8>> {
        if usize::from(card) >= PAL_MONSTER_SLOT_COUNT {
            return Err(format!("card {card} has no MONSTER.MRG slot").into());
        }
        let mut buffer = vec![0u8; PAL_MONSTER_SLOT_BYTES];
        self.file.seek(SeekFrom::Start(
            self.start + u64::from(card) * PAL_MONSTER_SLOT_BYTES as u64,
        ))?;
        self.file.read_exact(&mut buffer)?;
        Ok(buffer)
    }
}

/// GS texel bytes of an indexed texture (`roses_assets::monster_gs_texels`).
pub fn gs_texels(texture: &OpeningIndexedTexture) -> Vec<u8> {
    roses_assets::monster_gs_texels(texture)
}

/// Card ids the first duel can put on the board, split into those with a
/// MONSTER.MRG slot (sorted, unique) and the others: both decks and
/// leaders, every fusion result reachable from them (fusion table pairs
/// `[a, b, result]`, repeatedly, since results fuse again) and every ritual
/// result (`ritual_results`; which ritual card summons which result is not
/// used here).
pub fn first_duel_cards(
    decks: &roses_assets::DeckTable,
    fusions: &[[u16; 3]],
    ritual_results: &[u16],
) -> Result<(Vec<u16>, Vec<u16>)> {
    let mut cards = BTreeSet::new();
    for index in FIRST_DUEL_DECKS {
        let deck = decks
            .decks
            .get(index)
            .ok_or("deck table lacks a first-duel record")?;
        cards.insert(deck.leader_card);
        cards.extend(deck.cards.iter().copied());
    }
    loop {
        let before = cards.len();
        let results: Vec<u16> = fusions
            .iter()
            .filter(|[a, b, _]| cards.contains(a) && cards.contains(b))
            .map(|f| f[2])
            .collect();
        cards.extend(results);
        if cards.len() == before {
            break;
        }
    }
    cards.extend(ritual_results.iter().copied());
    Ok(cards
        .into_iter()
        .partition(|card| usize::from(*card) < PAL_MONSTER_SLOT_COUNT))
}

fn file_entry(output: &Path, path: &str) -> Result<serde_json::Value> {
    let bytes = fs::read(output.join(path))?;
    Ok(serde_json::json!({
        "path": path,
        "sha256": format!("{:x}", Sha256::digest(&bytes)),
        "bytes": bytes.len(),
    }))
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn export_model(output: &Path, slot: &[u8], model: &MonsterModel) -> Result<serde_json::Value> {
    let card = model.card;
    let model_path = format!("models/{card:03}.json");
    write_new(&output.join(&model_path), &serde_json::to_vec(model)?)?;
    let mut textures = Vec::new();
    let mut texture_refs = Vec::new();
    for (index, texture) in model.textures.iter().enumerate() {
        texture_refs.push((
            format!("textures/{card:03}-t{index}.png"),
            MonsterTextureRef::Model(index),
            texture.record.as_ref().map(|r| r.palette_bits),
            texture.unsupported.clone(),
            "model",
            index,
        ));
    }
    for (index, texture) in model.effect_textures.iter().enumerate() {
        texture_refs.push((
            format!("textures/{card:03}-e{}.png", texture.effect_slot),
            MonsterTextureRef::Effect(index),
            texture.record.as_ref().map(|r| r.palette_bits),
            texture.unsupported.clone(),
            "effect",
            usize::from(texture.effect_slot),
        ));
    }
    for (path, reference, bits, unsupported, kind, index) in texture_refs {
        let Some(bits) = bits else {
            textures.push(serde_json::json!({
                "kind": kind, "index": index, "unsupported": unsupported,
            }));
            continue;
        };
        let texture = roses_assets::decode_pal_monster_texture(slot, model, reference)?;
        write_png(
            &output.join(&path),
            texture.width,
            texture.height,
            &gs_texels(&texture),
        )?;
        let mut entry = file_entry(output, &path)?;
        entry["kind"] = kind.into();
        entry["index"] = index.into();
        entry["width"] = texture.width.into();
        entry["height"] = texture.height.into();
        entry["palette_bits"] = bits.into();
        textures.push(entry);
    }
    let draws = model.passes.iter().map(|p| p.draws.len()).sum::<usize>();
    let strips = model
        .passes
        .iter()
        .flat_map(|p| &p.draws)
        .map(|d| d.strips.len())
        .sum::<usize>();
    let vertices = model
        .passes
        .iter()
        .flat_map(|p| &p.draws)
        .flat_map(|d| &d.strips)
        .map(|s| s.vertices.len())
        .sum::<usize>();
    let mut entry = file_entry(output, &model_path)?;
    entry["card"] = card.into();
    entry["slot_sha256"] = model.slot_sha256.clone().into();
    entry["nodes"] = model.nodes.len().into();
    entry["draws"] = draws.into();
    entry["strips"] = strips.into();
    entry["vertices"] = vertices.into();
    entry["motions"] = serde_json::json!(model
        .motions
        .iter()
        .map(|m| serde_json::json!({"motion": m.motion, "length": m.length, "kind": m.kind}))
        .collect::<Vec<_>>());
    entry["textures"] = textures.into();
    Ok(entry)
}

pub fn run(iso: &Path, output: &Path, selection: Option<&Path>) -> Result<()> {
    let all = match selection
        .map(|p| p.to_string_lossy().into_owned())
        .as_deref()
    {
        None | Some("first-duel") => false,
        Some("all") => true,
        Some(other) => return Err(format!("--cards must be first-duel or all, not {other}").into()),
    };
    if output.exists() {
        return Err(
            "duel models output must be a new directory; existing imports are never overwritten"
                .into(),
        );
    }
    let (mut disc, report) = open_pal(iso)?;
    let executable = disc.read_file("SLES_515.79", 8 * 1024 * 1024)?;
    let decks = roses_assets::parse_pal_decks(&executable)?;
    let sine = roses_assets::parse_pal_sine_table(&executable)?;
    let dimx_at = DIMX_TABLE_FILE_OFFSET + 8 * UNIT_DIMX_ENTRY;
    let dimx = u64::from_le_bytes(
        executable
            .get(dimx_at..dimx_at + 8)
            .ok_or("executable is too short for the DIMX table")?
            .try_into()
            .unwrap(),
    );
    let unit_dither = dither_matrix(dimx)?;
    let mut archive = MonsterArchive::open(iso, &disc)?;
    eprintln!("Verifying the MONSTER.MRG fingerprint…");
    let archive_sha256 = archive.verify()?;
    let catalogue = crate::duel_catalogue::build(&executable)?;
    let ritual_results: Vec<u16> = catalogue.rituals.iter().map(|r| r.0).collect();
    let (first_duel, no_model) = first_duel_cards(&decks, &catalogue.fusions, &ritual_results)?;
    let cards: Vec<u16> = if all {
        (0..PAL_MONSTER_SLOT_COUNT as u16).collect()
    } else {
        first_duel.clone()
    };
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(output)?;
    fs::create_dir(output.join("models"))?;
    fs::create_dir(output.join("textures"))?;
    // The verbatim archive, slot by slot, with each slot's hash.
    let mut slot_sha256 = Vec::with_capacity(PAL_MONSTER_SLOT_COUNT);
    {
        let mut copy = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output.join(ARCHIVE_FILE))?;
        let mut whole = Sha256::new();
        for card in 0..PAL_MONSTER_SLOT_COUNT as u16 {
            let slot = archive.slot(card)?;
            whole.update(&slot);
            slot_sha256.push(format!("{:x}", Sha256::digest(&slot)));
            copy.write_all(&slot)?;
        }
        copy.sync_all()?;
        if format!("{:x}", whole.finalize()) != archive_sha256 {
            return Err("MONSTER.MRG changed while it was copied".into());
        }
    }
    let mut models = Vec::new();
    let mut failures = Vec::new();
    for card in &cards {
        let slot = archive.slot(*card)?;
        match roses_assets::decode_pal_monster_slot(&slot, *card) {
            Ok(model) => models.push(export_model(output, &slot, &model)?),
            Err(error) => {
                eprintln!("card {card}: {error}");
                failures.push(serde_json::json!({"card": card, "error": error.to_string()}));
            }
        }
    }
    emit_json(&report, Some(&output.join("disc.json")))?;
    let first_duel_decks: Vec<_> = FIRST_DUEL_DECKS
        .iter()
        .map(|index| {
            let deck = &decks.decks[*index];
            serde_json::json!({
                "deck_index": index,
                "leader_card": deck.leader_card,
                "leader_rank_raw": deck.leader_rank_raw,
                "cards": deck.cards,
            })
        })
        .collect();
    emit_json(
        &serde_json::json!({
            "schema": SCHEMA,
            "importer_version": env!("CARGO_PKG_VERSION"),
            "profile": roses_disc::PAL_PROFILE_ID,
            "source": {
                "iso_sha256": report.iso_sha256,
                "executable_sha256": decks.executable_sha256,
                "monster_archive": {
                    "iso_path": PAL_MONSTER_ARCHIVE_ISO_PATH,
                    "sha256": archive_sha256,
                    "bytes": PAL_MONSTER_ARCHIVE_BYTES,
                    "slot_bytes": PAL_MONSTER_SLOT_BYTES,
                },
            },
            "selection": if all { "all" } else { "first-duel" },
            "first_duel_decks": first_duel_decks,
            "first_duel_cards_without_model": no_model,
            "sine_table": {
                "ee_address": roses_assets::PAL_SINE_TABLE_EE_ADDRESS,
                "file_offset": roses_assets::PAL_SINE_TABLE_FILE_OFFSET,
                "values": sine,
            },
            "unit_dither_matrix": {
                "table_file_offset": DIMX_TABLE_FILE_OFFSET,
                "entry": UNIT_DIMX_ENTRY,
                "layout": "rows y & 3, columns x & 3 (signed DIMX fields)",
                "matrix": unit_dither,
            },
            "archive": {
                "path": ARCHIVE_FILE,
                "sha256": archive_sha256,
                "slot_bytes": PAL_MONSTER_SLOT_BYTES,
                "slot_sha256": slot_sha256,
            },
            "texture_encoding": "PNG RGBA = GS texel bytes: C_16 RGB5 << 3 with alpha 128 (CLUT alpha bit set, TEXA TA1) or 0 (TA0); C_32 raw bytes (alpha 128 = 1.0)",
            "models": models,
            "failed_cards": failures,
            "unresolved": [
                "CMOT camera records (kept raw)",
                "texture variants (texture-table word 2, RH0 section 9, 0x211700)",
                "rigid vertex UV third lane and weighted influence third lane",
                "MOT per-node u16 map",
                "motion cross-fade 0x1F2770, summon grow-in, Z write and blend order of pass 2",
            ],
            "complete_game_import": false,
            "rendering_validated": false,
        }),
        Some(&output.join("manifest.json")),
    )?;
    eprintln!(
        "Imported {} unit models ({} failed) into {}",
        models.len(),
        failures.len(),
        output.display()
    );
    if !all && !failures.is_empty() {
        return Err("some first-duel models failed to decode (see manifest.json)".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use roses_assets::MonsterModel;
    use roses_assets::OpeningPaletteFormat;
    use roses_client::monster_pose::{
        self, apply, evaluate_pose, mul, object_matrix, skin_model, Mat4, SineTable, UnitObject,
        BOARD_UNIT_LIGHT,
    };
    use serde_json::Value;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    #[test]
    fn gs_texels_expand_both_palette_formats() {
        let mut texture = OpeningIndexedTexture {
            width: 2,
            height: 1,
            palette_format: OpeningPaletteFormat::Rgb5551,
            palette_words_raw: (0..256)
                .map(|i| if i == 1 { 0xffff } else { 0x7c1f })
                .collect(),
            palette_indices: vec![0, 1],
        };
        assert_eq!(gs_texels(&texture), [248, 0, 248, 0, 248, 248, 248, 128]);
        texture.palette_format = OpeningPaletteFormat::Rgba8888Alpha128;
        texture.palette_words_raw[0] = 0x8011_2233;
        assert_eq!(&gs_texels(&texture)[..4], [0x33, 0x22, 0x11, 0x80]);
    }

    #[test]
    fn first_duel_selection_splits_monsters_from_other_cards() {
        let deck = |leader: u16, cards: Vec<u16>| roses_assets::DeckRecord {
            index: 0,
            leader_card: leader,
            leader_rank_raw: 1,
            cards,
        };
        let mut decks = roses_assets::DeckTable {
            schema_version: 1,
            profile_id: String::new(),
            executable_sha256: String::new(),
            table_ee_address: 0,
            decks: (0..51).map(|_| deck(0, vec![])).collect(),
            gameplay_validated: false,
        };
        decks.decks[0x09] = deck(392, vec![26, 700, 26]);
        decks.decks[0x1C] = deck(397, vec![396, 848]);
        let (models, other) = first_duel_cards(&decks, &[], &[]).unwrap();
        assert_eq!(models, [26, 392, 396, 397]);
        assert_eq!(other, [700, 848]);
        // Fusion results join, also when they fuse again; rituals always.
        let fusions = [[26, 396, 500], [500, 397, 501], [1, 2, 3]];
        let (models, _) = first_duel_cards(&decks, &fusions, &[600]).unwrap();
        assert_eq!(models, [26, 392, 396, 397, 500, 501, 600]);
    }

    // ------------------------------------------------------------ private
    fn iso() -> PathBuf {
        PathBuf::from(std::env::var_os("ROSES_TEST_ISO_PATH").expect("ROSES_TEST_ISO_PATH"))
    }
    fn research() -> PathBuf {
        PathBuf::from(
            std::env::var_os("ROSES_MONSTER_RESEARCH_DIR")
                .expect("ROSES_MONSTER_RESEARCH_DIR (work/research/duel/claude-models)"),
        )
    }
    fn open_archive() -> (roses_disc::Disc, MonsterArchive) {
        let disc = roses_disc::Disc::open(iso()).unwrap();
        let archive = MonsterArchive::open(&iso(), &disc).unwrap();
        (disc, archive)
    }
    fn sine_table(disc: &mut roses_disc::Disc) -> SineTable {
        let executable = disc.read_file("SLES_515.79", 8 * 1024 * 1024).unwrap();
        SineTable::new(roses_assets::parse_pal_sine_table(&executable).unwrap()).unwrap()
    }
    fn mat(value: &Value) -> Mat4 {
        std::array::from_fn(|c| std::array::from_fn(|r| value[c][r].as_f64().unwrap()))
    }
    fn max_rel(a: &Mat4, b: &Mat4) -> f64 {
        let scale = b.iter().flatten().fold(1.0f64, |m, v| m.max(v.abs()));
        a.iter()
            .flatten()
            .zip(b.iter().flatten())
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max)
            / scale
    }
    fn invert(m: &Mat4) -> Mat4 {
        let mut a: [[f64; 8]; 4] = std::array::from_fn(|r| {
            std::array::from_fn(|c| {
                if c < 4 {
                    m[c][r]
                } else {
                    f64::from(u8::from(c - 4 == r))
                }
            })
        });
        for col in 0..4 {
            let pivot = (col..4)
                .max_by(|x, y| a[*x][col].abs().total_cmp(&a[*y][col].abs()))
                .unwrap();
            a.swap(col, pivot);
            let p = a[col][col];
            for v in a[col].iter_mut() {
                *v /= p;
            }
            for row in 0..4 {
                if row != col {
                    let f = a[row][col];
                    let pivot_row = a[col];
                    for (v, pv) in a[row].iter_mut().zip(pivot_row) {
                        *v -= f * pv;
                    }
                }
            }
        }
        std::array::from_fn(|c| std::array::from_fn(|r| a[r][c + 4]))
    }
    fn object(value: &Value) -> UnitObject {
        let f3 =
            |k: &str| -> [f32; 3] { std::array::from_fn(|i| value[k][i].as_f64().unwrap() as f32) };
        let i3 =
            |k: &str| -> [i32; 3] { std::array::from_fn(|i| value[k][i].as_i64().unwrap() as i32) };
        UnitObject {
            position: f3("position"),
            rotation: i3("rotation"),
            scale: f3("scale"),
            rotation2: i3("rotation2"),
            grow: value["grow"].as_f64().unwrap() as f32,
            recentre: value["recentre"].as_u64().unwrap() as u8,
        }
    }

    #[test]
    #[ignore = "Requires the user's supported ISO (ROSES_TEST_ISO_PATH)"]
    fn private_all_monster_slots_decode() {
        let (_, mut archive) = open_archive();
        assert_eq!(archive.verify().unwrap(), PAL_MONSTER_ARCHIVE_SHA256);
        let mut failures = Vec::new();
        let (mut vertices, mut unsupported, mut textures, mut no_gs) = (0usize, 0, 0, 0);
        let mut unsupported_cards = BTreeSet::new();
        let mut trailing = BTreeSet::new();
        for card in 0..PAL_MONSTER_SLOT_COUNT as u16 {
            let slot = archive.slot(card).unwrap();
            match roses_assets::decode_pal_monster_slot(&slot, card) {
                Ok(model) => {
                    vertices += model
                        .passes
                        .iter()
                        .flat_map(|p| &p.draws)
                        .flat_map(|d| &d.strips)
                        .map(|s| s.vertices.len())
                        .sum::<usize>();
                    textures += model.textures.len();
                    for (i, t) in model.textures.iter().enumerate() {
                        if t.record.is_none() {
                            unsupported += 1;
                            unsupported_cards.insert(card);
                            let users = model
                                .passes
                                .iter()
                                .flat_map(|p| &p.draws)
                                .filter(|d| d.texture_index == Some(i as u16))
                                .count();
                            eprintln!("card {card} t{i}: {:?} ({users} draws)", t.unsupported);
                        }
                    }
                    for t in &model.effect_textures {
                        if t.record.is_none() {
                            unsupported += 1;
                            unsupported_cards.insert(card);
                            eprintln!("card {card} e{}: {:?}", t.effect_slot, t.unsupported);
                        }
                    }
                    // Every texture with a record decodes.
                    let refs = (0..model.textures.len())
                        .filter(|i| model.textures[*i].record.is_some())
                        .map(MonsterTextureRef::Model)
                        .chain(
                            (0..model.effect_textures.len())
                                .filter(|i| model.effect_textures[*i].record.is_some())
                                .map(MonsterTextureRef::Effect),
                        );
                    for reference in refs {
                        match roses_assets::decode_pal_monster_texture(&slot, &model, reference) {
                            Ok(t) => {
                                if let Some(dir) = std::env::var_os("ROSES_TEXTURE_DUMP") {
                                    write_png(
                                        &Path::new(&dir).join(format!("{card:03}-{reference:?}.png")),
                                        t.width,
                                        t.height,
                                        &gs_texels(&t),
                                    )
                                    .unwrap();
                                }
                            }
                            Err(e) => failures.push((card, format!("{reference:?}: {e}"))),
                        }
                    }
                    no_gs += model
                        .passes
                        .iter()
                        .flat_map(|p| &p.draws)
                        .filter(|d| d.gs_registers.is_none())
                        .count();
                    if slot[model.used_slot_bytes as usize..]
                        .iter()
                        .any(|b| *b != 0)
                    {
                        trailing.insert(card);
                    }
                }
                Err(error) => failures.push((card, error.to_string())),
            }
        }
        eprintln!(
            "683 slots: {vertices} vertices, {textures} textures ({unsupported} without a record, cards {unsupported_cards:?}), {no_gs} draws without a GS packet; {} slots with nonzero bytes after the chained records",
            trailing.len()
        );
        assert!(failures.is_empty(), "{failures:?}");
    }

    fn check_equal(a: f64, b: f64, what: &str) {
        assert!(
            a == b || (a - b).abs() <= 1e-6 * b.abs().max(1e-30),
            "{what}: {a} != {b}"
        );
    }
    fn f32_equal(a: f32, b: &Value, what: &str) {
        assert_eq!(a, b.as_f64().unwrap() as f32, "{what}");
    }

    #[test]
    #[ignore = "Requires the user's ISO and the private research export (ROSES_MONSTER_RESEARCH_DIR)"]
    fn private_models_equal_the_research_export() {
        let (_, mut archive) = open_archive();
        for card in [383u16, 392] {
            let slot = archive.slot(card).unwrap();
            let model = roses_assets::decode_pal_monster_slot(&slot, card).unwrap();
            let dir = research().join("export").join(format!("{card:03}"));
            let doc: Value =
                serde_json::from_slice(&fs::read(dir.join("model.json")).unwrap()).unwrap();
            assert_eq!(doc["card"], card);
            assert_eq!(doc["slot_sha256"], model.slot_sha256.as_str());
            assert_eq!(doc["flags"], format!("{:08x}", model.flags).as_str());
            assert_eq!(doc["rh0_size"], model.rh0_bytes);
            f32_equal(
                model.inv_root_scale,
                &doc["inv_root_scale"],
                "inv root scale",
            );
            let names = [
                "nodes", "dma", "geom", "dma2", "geom2", "textab", "reloc1", "reloc2", "sec8",
                "sec9",
            ];
            for (mine, name) in model.sections.iter().zip(names) {
                let theirs = &doc["sections"][name];
                assert_eq!(
                    (theirs[0].as_u64(), theirs[1].as_u64()),
                    (Some(mine.offset.into()), Some(mine.byte_len.into()))
                );
            }
            // Nodes.
            let nodes = doc["nodes"].as_array().unwrap();
            assert_eq!(nodes.len(), model.nodes.len());
            for (mine, theirs) in model.nodes.iter().zip(nodes) {
                for k in 0..3 {
                    f32_equal(mine.scale[k], &theirs["scale"][k], "scale");
                    f32_equal(mine.translation[k], &theirs["trans"][k], "trans");
                    assert_eq!(
                        i64::from(mine.rotation[k]),
                        theirs["rot"][k].as_i64().unwrap()
                    );
                }
                for k in 0..4 {
                    f32_equal(mine.bounding_sphere[k], &theirs["sphere"][k], "sphere");
                }
                assert_eq!(theirs["flags"], mine.flags);
                assert_eq!(theirs["block"], mine.matrix_block);
                assert_eq!(theirs["child"].as_u64(), mine.child.map(u64::from));
                assert_eq!(theirs["sibling"].as_u64(), mine.sibling.map(u64::from));
            }
            // Textures (records and pixels, within the 5-bit expansion step).
            let textures = doc["textures"].as_array().unwrap();
            assert_eq!(textures.len(), model.textures.len());
            for (index, (mine, theirs)) in model.textures.iter().zip(textures).enumerate() {
                let record = mine.record.as_ref().unwrap();
                assert_eq!(theirs["offset"], record.slot_offset);
                assert_eq!(theirs["size"], record.byte_len);
                assert_eq!(
                    (theirs["width"].as_u64(), theirs["height"].as_u64()),
                    (Some(record.width.into()), Some(record.height.into()))
                );
                assert_eq!(theirs["clut_bits"], record.palette_bits);
                let decoded = roses_assets::decode_pal_monster_texture(
                    &slot,
                    &model,
                    MonsterTextureRef::Model(index),
                )
                .unwrap();
                let png = png::Decoder::new(std::io::Cursor::new(
                    fs::read(dir.join(theirs["png"].as_str().unwrap())).unwrap(),
                ));
                let mut reader = png.read_info().unwrap();
                let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
                reader.next_frame(&mut pixels).unwrap();
                let mine = gs_texels(&decoded);
                assert_eq!(pixels.len(), mine.len());
                for (m, t) in mine.chunks_exact(4).zip(pixels.chunks_exact(4)) {
                    for c in 0..3 {
                        assert!((i32::from(m[c]) - i32::from(t[c])).abs() <= 7);
                    }
                    assert_eq!(m[3] == 128, t[3] == 255);
                }
            }
            // Passes, draws, strips, vertices.
            let passes = doc["passes"].as_array().unwrap();
            assert_eq!(passes.len(), model.passes.len());
            let mut compared = 0usize;
            for (mine, theirs) in model.passes.iter().zip(passes) {
                assert_eq!(theirs["name"], if mine.pass == 0 { "dma" } else { "dma2" });
                let draws = theirs["draws"].as_array().unwrap();
                assert_eq!(draws.len(), mine.draws.len());
                for (d, t) in mine.draws.iter().zip(draws) {
                    assert_eq!(t["texture"].as_u64(), d.texture_index.map(u64::from));
                    let material = d.material().unwrap();
                    for k in 0..12 {
                        f32_equal(material[k], &t["material"][k], "material");
                    }
                    let gs = d.gs_registers.as_ref().unwrap();
                    for (pair, theirs) in gs.iter().zip(t["gs"].as_array().unwrap()) {
                        assert_eq!(theirs[0], format!("{:#x}", pair[1]).as_str());
                        assert_eq!(theirs[1], format!("{:#x}", pair[0]).as_str());
                    }
                    let mats = t["mats"].as_object().unwrap();
                    assert_eq!(mats.len(), d.matrix_slots.len());
                    for slot in &d.matrix_slots {
                        assert_eq!(mats[&slot.vu_address.to_string()], slot.matrix_block);
                    }
                    let strips = t["strips"].as_array().unwrap();
                    assert_eq!(strips.len(), d.strips.len());
                    for (s, ts) in d.strips.iter().zip(strips) {
                        assert_eq!(ts["prim"], s.prim);
                        assert_eq!(ts["skinned"], s.skinned);
                        assert_eq!(ts["vu_tag"].as_u64(), s.rigid_matrix_vu.map(u64::from));
                        let vs = ts["v"].as_array().unwrap();
                        assert_eq!(vs.len(), s.vertices.len());
                        for (v, tv) in s.vertices.iter().zip(vs) {
                            for k in 0..3 {
                                assert_eq!(tv[k], v.texture_raw[k]);
                            }
                            let infl = tv[3].as_array().unwrap();
                            assert_eq!(infl.len(), v.influences.len());
                            for (i, ti) in v.influences.iter().zip(infl) {
                                assert_eq!(ti[0], i.matrix_vu);
                                check_equal(
                                    f64::from(i.weight_raw) / 4096.0,
                                    ti[1].as_f64().unwrap(),
                                    "weight",
                                );
                                for k in 0..3 {
                                    assert_eq!(ti[2 + k], i.normal_raw[k]);
                                    assert_eq!(ti[5 + k], i.position_raw[k]);
                                }
                            }
                            compared += 1;
                        }
                    }
                }
            }
            // Motions.
            let motions = doc["motions"].as_object().unwrap();
            assert_eq!(motions.len(), model.motions.len());
            for m in &model.motions {
                let t = &motions[&m.motion.to_string()];
                assert_eq!(
                    (t["offset"].as_u64(), t["size"].as_u64()),
                    (Some(m.slot_offset.into()), Some(m.byte_len.into()))
                );
                assert_eq!(
                    (
                        t["length"].as_u64(),
                        t["type"].as_u64(),
                        t["nodes"].as_u64()
                    ),
                    (
                        Some(m.length.into()),
                        Some(m.kind.into()),
                        Some(m.node_count.into())
                    )
                );
                for (track, tt) in m.tracks.iter().zip(t["tracks"].as_array().unwrap()) {
                    let vec_keys =
                        |keys: &Option<Vec<roses_assets::MonsterVecKey>>, tk: &Value| match keys {
                            None => assert!(tk.is_null()),
                            Some(keys) => {
                                let tk = tk.as_array().unwrap();
                                assert_eq!(tk.len(), keys.len());
                                for (k, t) in keys.iter().zip(tk) {
                                    assert_eq!(t[0], k.0);
                                    for c in 0..3 {
                                        f32_equal(k.1[c], &t[1 + c], "key");
                                    }
                                }
                            }
                        };
                    vec_keys(&track.translation, &tt[0]);
                    match &track.rotation {
                        None => assert!(tt[1].is_null()),
                        Some(keys) => {
                            let tk = tt[1].as_array().unwrap();
                            assert_eq!(tk.len(), keys.len());
                            for (k, t) in keys.iter().zip(tk) {
                                assert_eq!(t[0], k.0);
                                for c in 0..3 {
                                    assert_eq!(t[1 + c], k.1[c]);
                                }
                            }
                        }
                    }
                    if m.kind & 4 != 0 {
                        vec_keys(&track.scale, &tt[2]);
                    }
                }
            }
            let effects = doc["effect_textures"].as_array().unwrap();
            assert_eq!(effects.len(), model.effect_textures.len());
            for (e, t) in model.effect_textures.iter().zip(effects) {
                assert_eq!(
                    (t["slot"].as_u64(), t["offset"].as_u64(), t["size"].as_u64()),
                    (
                        Some(e.effect_slot.into()),
                        Some(e.slot_offset.into()),
                        Some(e.byte_len.into())
                    )
                );
            }
            for record in &model.camera_records {
                let key = record.kind.trim_start_matches("cmot_");
                assert_eq!(doc["cmot"][key][0], record.slot_offset);
                assert_eq!(doc["cmot"][key][1], record.byte_len);
            }
            let cues = doc["sound_cues"].as_array().unwrap();
            assert_eq!(cues.len(), model.sound_cues.len());
            for (c, t) in model.sound_cues.iter().zip(cues) {
                assert_eq!(
                    (
                        t["motion"].as_u64(),
                        t["frame"].as_u64(),
                        t["sound"].as_u64()
                    ),
                    (
                        Some(c.motion.into()),
                        Some(c.frame.into()),
                        Some(c.sound.into())
                    )
                );
            }
            eprintln!(
                "card {card}: {compared} vertices, {} motions equal the research export",
                model.motions.len()
            );
        }
    }

    fn decode_cached<'a>(
        archive: &mut MonsterArchive,
        cache: &'a mut BTreeMap<u16, MonsterModel>,
        card: u16,
    ) -> &'a MonsterModel {
        cache.entry(card).or_insert_with(|| {
            roses_assets::decode_pal_monster_slot(&archive.slot(card).unwrap(), card).unwrap()
        })
    }

    #[test]
    #[ignore = "Requires the user's ISO and the private reference dump (ROSES_MONSTER_RESEARCH_DIR)"]
    fn private_pose_reproduces_the_savestate_matrices() {
        let (mut disc, mut archive) = open_archive();
        let table = sine_table(&mut disc);
        let reference: Value = serde_json::from_slice(
            &fs::read(research().join("reference").join("pose-reference.json")).unwrap(),
        )
        .unwrap();
        let rows = reference["rows"].as_array().unwrap();
        let mut cache = BTreeMap::new();
        let (mut matrices, mut exact, mut pose_exact) = (0usize, 0usize, 0usize);
        let (mut worst_python, mut worst_pose_only) = (0.0f64, 0.0f64);
        let mut states = BTreeSet::new();
        let mut hoshiningen_px = None;
        for row in rows {
            let card = row["card"].as_u64().unwrap() as u16;
            let model = decode_cached(&mut archive, &mut cache, card).clone();
            let motion = model.motion(row["motion"].as_u64().unwrap() as u8);
            let t = row["time"].as_f64().unwrap() as f32;
            let pose = evaluate_pose(&model, motion, t, &table).unwrap();
            // Rust vs the Python reference evaluator.
            for (block, theirs) in row["python_blocks"].as_object().unwrap() {
                let block: u16 = block.parse().unwrap();
                worst_python = worst_python.max(max_rel(&pose.blocks[&block], &mat(theirs)));
            }
            let world = object_matrix(&model, &object(&row["object"]), &table);
            worst_python = worst_python.max(max_rel(&world, &mat(&row["python_world"])));
            // Rust vs the savestate DMA list.
            let pv = mat(&row["camera"]["PV"]);
            let v = mat(&row["camera"]["V"]);
            let mut worst = 0.0f64;
            let mut gs = Vec::new();
            // The pose-only check uses its own best time (as verify_pose.py).
            let t_pose = row["time_pose_only"].as_f64().unwrap() as f32;
            let pose_only = evaluate_pose(&model, motion, t_pose, &table).unwrap();
            for draw in row["draws"].as_array().unwrap() {
                let base = if draw["entry"].as_u64() == Some(0x1a4) {
                    &v
                } else {
                    &pv
                };
                for m in draw["matrices"].as_array().unwrap() {
                    let index = m["block"].as_u64().unwrap() as u16;
                    let block = &pose.blocks[&index];
                    let list = mat(&m["list_matrix"]);
                    let mine = mul(&mul(base, &world), block);
                    worst = worst.max(max_rel(&mine, &list));
                    gs.push(mul(&list, &invert(&pose_only.blocks[&index])));
                    matrices += 1;
                }
            }
            // Pose-only: one global transform G per object (element median).
            let median: Mat4 = std::array::from_fn(|c| {
                std::array::from_fn(|r| {
                    let mut values: Vec<f64> = gs.iter().map(|g| g[c][r]).collect();
                    values.sort_by(f64::total_cmp);
                    let n = values.len();
                    if n % 2 == 1 {
                        values[n / 2]
                    } else {
                        0.5 * (values[n / 2 - 1] + values[n / 2])
                    }
                })
            });
            let spread = gs.iter().map(|g| max_rel(g, &median)).fold(0.0, f64::max);
            worst_pose_only = worst_pose_only.max(spread);
            exact += usize::from(worst < 1e-6);
            pose_exact += usize::from(spread < 1e-6);
            let python = row["python_rel_err"].as_f64().unwrap();
            assert_eq!(
                worst < 1e-6,
                python < 1e-6,
                "{} obj {}: {worst} vs python {python}",
                row["state"],
                row["obj"]
            );
            states.insert(row["state"].as_str().unwrap().to_string());
            if row["state"] == "742-x8-flip" && row["obj"] == 20 {
                // Hoshiningen: vertices through the list's matrices vs ours.
                let mut px = 0.0f64;
                let mut z = 0.0f64;
                for draw in row["draws"].as_array().unwrap() {
                    let pass = draw["pass_index"].as_u64().unwrap() as usize;
                    let index = draw["draw"].as_u64().unwrap() as usize;
                    let d = &model.passes[pass].draws[index];
                    let ram: BTreeMap<u16, Mat4> = draw["matrices"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|m| (m["vu"].as_u64().unwrap() as u16, mat(&m["list_matrix"])))
                        .collect();
                    for s in &d.strips {
                        for vertex in &s.vertices {
                            if vertex
                                .influences
                                .iter()
                                .any(|i| !ram.contains_key(&i.matrix_vu))
                            {
                                continue;
                            }
                            let (mut a, mut b) = ([0.0f64; 4], [0.0f64; 4]);
                            for i in &vertex.influences {
                                let p = [
                                    f64::from(i.position_raw[0]),
                                    f64::from(i.position_raw[1]),
                                    f64::from(i.position_raw[2]),
                                    1.0,
                                ];
                                let w = f64::from(i.weight_raw) / 4096.0;
                                let block = &pose.blocks[&d.matrix_block(i.matrix_vu).unwrap()];
                                let qa = apply(&ram[&i.matrix_vu], p);
                                let qb = apply(&mul(&mul(&pv, &world), block), p);
                                for k in 0..4 {
                                    a[k] += w * qa[k];
                                    b[k] += w * qb[k];
                                }
                            }
                            let dx = a[0] / a[3] - b[0] / b[3];
                            let dy = a[1] / a[3] - b[1] / b[3];
                            px = px.max(dx.hypot(dy));
                            z = z.max((a[2] / a[3] - b[2] / b[3]).abs());
                        }
                    }
                }
                hoshiningen_px = Some((px, z));
            }
        }
        eprintln!(
            "{} states, {} objects, {matrices} node matrices: {exact} exact (rel < 1e-6), {pose_exact} pose-exact (worst spread {worst_pose_only:.2e}); Rust vs Python pose max rel {worst_python:.2e}; Hoshiningen px/Z24 {hoshiningen_px:?}",
            states.len(),
            rows.len()
        );
        // verification.json scanned 57 savestates; 48 of them draw unit models.
        assert_eq!((states.len(), rows.len(), matrices), (48, 131, 18_834));
        assert!(worst_python < 1e-9, "{worst_python}");
        assert_eq!(pose_exact, 131);
        assert!(exact >= 112);
        let (px, z) = hoshiningen_px.unwrap();
        // Notes: <= 4e-4 px and Z24 <= 0.09 (two decimals).
        assert!(px <= 4e-4 && z < 0.095, "{px} {z}");
    }

    #[test]
    #[ignore = "Requires the user's ISO and the private reference dump (ROSES_MONSTER_RESEARCH_DIR)"]
    fn private_round2_dump_vertices_and_lighting() {
        let (mut disc, mut archive) = open_archive();
        let table = sine_table(&mut disc);
        let reference: Value = serde_json::from_slice(
            &fs::read(research().join("reference").join("round2-dump.json")).unwrap(),
        )
        .unwrap();
        let model =
            roses_assets::decode_pal_monster_slot(&archive.slot(392).unwrap(), 392).unwrap();
        let t = reference["time"].as_f64().unwrap() as f32;
        let pose = evaluate_pose(&model, model.motion(0), t, &table).unwrap();
        let object = object(&reference["object"]);
        assert_eq!(
            object,
            UnitObject::board_unit(&model, 3, 6, 0),
            "board placement rule"
        );
        let world = object_matrix(&model, &object, &table);
        assert!(max_rel(&world, &mat(&reference["python_world"])) < 1e-9);
        let pv = mat(&reference["camera"]["PV"]);
        let offset = reference["xy_offset"].as_f64().unwrap();
        let skinned = skin_model(&model, &pose, &world, &BOARD_UNIT_LIGHT, 1.0).unwrap();
        let (mut worst, mut total, mut matched) = (0.0f64, 0usize, 0usize);
        let (mut exact, mut within_one, mut alpha_ok) = (0usize, 0usize, 0usize);
        for draw in reference["draws"].as_array().unwrap() {
            let texture = draw["texture"].as_u64().unwrap() as u16;
            let (tw, th) = (
                f64::from(1u32 << draw["tw"].as_u64().unwrap()),
                f64::from(1u32 << draw["th"].as_u64().unwrap()),
            );
            // Our projected vertices of the pass-0 draws using this texture.
            let mut ours: Vec<[f64; 4]> = Vec::new();
            let mut colours: Vec<[u8; 4]> = Vec::new();
            for sd in skinned.iter().filter(|s| s.pass == 0) {
                if model.passes[0].draws[sd.draw].texture_index != Some(texture) {
                    continue;
                }
                for strip in &sd.strips {
                    for (i, p) in strip.positions.iter().enumerate() {
                        let c = apply(&pv, [p[0], p[1], p[2], 1.0]);
                        ours.push([
                            c[0] / c[3] - offset,
                            c[1] / c[3] - offset,
                            f64::from(strip.uv[i][0]) * tw,
                            f64::from(strip.uv[i][1]) * th,
                        ]);
                        colours.push(strip.colours[i]);
                    }
                }
            }
            let mut order: Vec<usize> = (0..ours.len()).collect();
            order.sort_by(|a, b| ours[*a][2].total_cmp(&ours[*b][2]));
            for v in draw["vertices"].as_array().unwrap() {
                let v: Vec<f64> = v
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_f64().unwrap())
                    .collect();
                total += 1;
                let first = order.partition_point(|i| ours[*i][2] < v[3] - 0.07);
                let mut best: Option<(f64, usize)> = None;
                for &i in &order[first..] {
                    if ours[i][2] >= v[3] + 0.07 {
                        break;
                    }
                    if (ours[i][3] - v[4]).abs() < 0.07 {
                        let d = (ours[i][0] - v[0]).hypot(ours[i][1] - v[1]);
                        if best.is_none_or(|(bd, _)| d < bd) {
                            best = Some((d, i));
                        }
                    }
                }
                let Some((d, i)) = best else { continue };
                matched += 1;
                worst = worst.max(d);
                let diff: Vec<i32> = (0..3)
                    .map(|c| i32::from(colours[i][c]) - v[5 + c] as i32)
                    .collect();
                exact += usize::from(diff.iter().all(|d| *d == 0));
                within_one += usize::from(diff.iter().all(|d| d.abs() <= 1));
                alpha_ok += usize::from(f64::from(colours[i][3]) == v[8]);
            }
        }
        let (exact_share, one_share) = (
            exact as f64 / matched as f64,
            within_one as f64 / matched as f64,
        );
        eprintln!(
            "round-2 dump (card 392, t {t}): {matched}/{total} vertices matched by UV, max {worst:.4} px; colour exact {:.2}% (+-1 {:.2}%), alpha {alpha_ok}",
            100.0 * exact_share,
            100.0 * one_share
        );
        assert_eq!((total, matched), (12_933, 12_933));
        assert!(worst <= 0.09, "{worst}");
        assert!(
            exact_share >= 0.99 && one_share >= 0.999,
            "{exact_share} {one_share}"
        );
        assert_eq!(alpha_ok, matched);
        let _ = monster_pose::identity();
    }
}
