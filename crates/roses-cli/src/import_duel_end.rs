//! `roses import-duel-end`: the disc files the duel's result screens and the
//! reward slot machine (mode 0xF) draw from, copied verbatim from the user's
//! ISO into a new private directory: DATA/TEXETC.MRG (records 3 and 4 hold
//! the slot machine's stone backdrop and its side panels with the
//! "GRAVEYARDS SLOT MACHINE" title, the buttons and the arrows) and
//! DATA/PICTURE.MRG (the 128x128 card pictures shown on the reels and in the
//! GET CARD / PROMOTION popups). The result banners themselves use the duel
//! import's files (TEXSYS #6/#7, PICMINI, MSG_E.DAT, the executable's string
//! table at 0x2903B0). Sources were established from software-GS dumps and
//! the game's own draw routines run in an interpreter (private research
//! `work/research/duel/claude-end/screen`). Nothing here may be
//! distributed.
use crate::{emit_json, open_pal, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub const SCHEMA: &str = "roses-duel-end-import-1";

/// Disc files copied as they are (path on the disc, file name here, limit).
const COPIED: [(&str, &str, u64); 2] = [
    ("DATA/TEXETC.MRG", "TEXETC.MRG", 8 * 1024 * 1024),
    ("DATA/PICTURE.MRG", "PICTURE.MRG", 24 * 1024 * 1024),
];

#[derive(Serialize)]
struct FileRef {
    path: String,
    source: String,
    sha256: String,
    bytes: u64,
}

pub fn run(iso: &Path, output: &Path) -> Result<()> {
    if output.exists() {
        return Err(
            "duel-end import output must be a new directory; existing imports are never overwritten"
                .into(),
        );
    }
    let (mut disc, report) = open_pal(iso)?;
    let mut copies = Vec::new();
    for (source, name, limit) in COPIED {
        copies.push((source, name, disc.read_file(source, limit)?));
    }
    // The picture archive must be the aligned 128x128 RH2 layout the client decodes.
    let pictures = &copies[1].2;
    if pictures.is_empty() || pictures.len() % roses_assets::PICTURE_RECORD_STRIDE != 0 {
        return Err("DATA/PICTURE.MRG is not the supported aligned picture archive".into());
    }
    roses_assets::decode_picture(pictures, 0)?;
    for index in [3, 4] {
        roses_assets::decode_pal_opening_indexed_texture(
            &copies[0].2,
            roses_assets::OpeningArchiveKind::TexEtc,
            index,
        )?;
    }
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(output)?;
    let mut files = Vec::new();
    for (source, name, bytes) in &copies {
        fs::write(output.join(name), bytes)?;
        files.push(FileRef {
            path: (*name).into(),
            source: (*source).into(),
            sha256: format!("{:x}", Sha256::digest(bytes)),
            bytes: bytes.len() as u64,
        });
    }
    emit_json(
        &serde_json::json!({
            "schema": SCHEMA,
            "importer_version": env!("CARGO_PKG_VERSION"),
            "profile": roses_disc::PAL_PROFILE_ID,
            "iso_sha256": report.iso_sha256,
            "private_local_only": true,
            "files": files,
            "slot_machine_textures": {"backdrop": {"file": "TEXETC.MRG", "record": 3}, "panels": {"file": "TEXETC.MRG", "record": 4}},
            "card_pictures": {"file": "PICTURE.MRG", "record": "card id"},
        }),
        Some(&output.join("manifest.json")),
    )?;
    println!(
        "wrote the slot machine textures and {} card pictures to {}",
        pictures.len() / roses_assets::PICTURE_RECORD_STRIDE,
        output.display()
    );
    Ok(())
}
