//! `roses import-duel`: everything a client needs to run the first campaign
//! duel (player deck record 0x09 against Weevil's 0x1C on board map 2),
//! taken from the user's ISO into a new private directory (default
//! `work/imports/...`): the rules catalogue (cards, effect entries,
//! power-ups, fusions, ritual requirements), card names, both decks, the
//! board, and verbatim copies of the files the HUD and the computer
//! player read at runtime (the executable's tables, MSG_E.DAT, TEXSYS.MRG,
//! PICMINI.MRG) and the battle scene's TEXEFF.MRG. Nothing here may be distributed.
use crate::{emit_json, open_pal, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub const SCHEMA: &str = "roses-duel-import-1";

/// Disc files copied as they are (path on the disc, file name here, limit).
const COPIED: [(&str, &str, u64); 5] = [
    ("SLES_515.79", "SLES_515.79", 8 * 1024 * 1024),
    ("MSG/MSG_E.DAT", "MSG_E.DAT", 300_000),
    ("DATA/TEXSYS.MRG", "TEXSYS.MRG", 2 * 1024 * 1024),
    ("DATA/PICMINI.MRG", "PICMINI.MRG", 4 * 1024 * 1024),
    // The battle scene's effect textures (record 15: the damage digits).
    ("DATA/TEXEFF.MRG", "TEXEFF.MRG", 1024 * 1024),
];

#[derive(Serialize)]
struct FileRef {
    path: String,
    source: String,
    sha256: String,
    bytes: u64,
}

#[derive(Serialize)]
struct Side {
    deck_record: usize,
    leader: u16,
    /// Deck order as stored; the campaign shuffle runs at duel start.
    cards: Vec<u16>,
}

pub fn run(iso: &Path, output: &Path) -> Result<()> {
    if output.exists() {
        return Err(
            "duel import output must be a new directory; existing imports are never overwritten"
                .into(),
        );
    }
    let (mut disc, report) = open_pal(iso)?;
    let mut files = Vec::new();
    let mut copies = Vec::new();
    for (source, name, limit) in COPIED {
        copies.push((source, name, disc.read_file(source, limit)?));
    }
    let executable = &copies[0].2;
    let messages = &copies[1].2;
    let catalogue = crate::duel_catalogue::build(executable)?;
    let names: Vec<String> = roses_assets::parse_pal_card_names(messages)?
        .names
        .into_iter()
        .map(|n| n.name)
        .collect();
    let decks = roses_assets::parse_pal_decks(executable)?.decks;
    let rules = roses_assets::parse_pal_rules(executable)?;
    let side = |record: usize| Side {
        deck_record: record,
        leader: decks[record].leader_card,
        cards: decks[record].cards.clone(),
    };
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(output)?;
    for (source, name, bytes) in &copies {
        fs::write(output.join(name), bytes)?;
        files.push(FileRef {
            path: (*name).into(),
            source: (*source).into(),
            sha256: format!("{:x}", Sha256::digest(bytes)),
            bytes: bytes.len() as u64,
        });
    }
    let mut write = |name: &str, value: &serde_json::Value| -> Result<()> {
        let bytes = serde_json::to_vec(value)?;
        fs::write(output.join(name), &bytes)?;
        files.push(FileRef {
            path: name.into(),
            source: "derived".into(),
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            bytes: bytes.len() as u64,
        });
        Ok(())
    };
    write("catalogue.json", &serde_json::to_value(&catalogue)?)?;
    write("card-names.json", &serde_json::to_value(&names)?)?;
    write(
        "first-duel.json",
        &serde_json::json!({
            "board_map": crate::duel_text::WEEVIL_BOARD,
            "terrain": rules.maps[crate::duel_text::WEEVIL_BOARD].cells,
            "first": 0,
            "sides": [
                side(crate::duel_text::PLAYER_DECK),
                side(crate::duel_text::WEEVIL_DECK),
            ],
            "computer_sides": [1],
            "planner_tier": 0,
        }),
    )?;
    emit_json(&report, Some(&output.join("disc.json")))?;
    emit_json(
        &serde_json::json!({
            "schema": SCHEMA,
            "importer_version": env!("CARGO_PKG_VERSION"),
            "profile": roses_disc::PAL_PROFILE_ID,
            "iso_sha256": report.iso_sha256,
            "private_local_only": true,
            "files": files,
        }),
        Some(&output.join("manifest.json")),
    )?;
    println!(
        "wrote {} cards, {} fusions, {} ritual rows and the first duel's decks to {}",
        catalogue.cards.len(),
        catalogue.fusions.len(),
        catalogue.rituals.len(),
        output.display()
    );
    Ok(())
}
