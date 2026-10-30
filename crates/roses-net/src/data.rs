//! Duel data from the user's own `roses import-duel` folder (or the `duel`
//! folder of a game import): the rules catalogue and the first duel's
//! decks/board, hash-checked against the import manifest. Nothing is
//! embedded in source; the server and each client read their own import.

use roses_core::duel::Catalogue;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::Path;

pub const DUEL_IMPORT_SCHEMA: &str = "roses-duel-import-1";

#[derive(Deserialize)]
struct Manifest {
    schema: String,
    files: Vec<FileRef>,
}

#[derive(Deserialize)]
struct FileRef {
    path: String,
    sha256: String,
}

/// One side of `first-duel.json`.
#[derive(Clone, Debug, Deserialize)]
pub struct ImportSide {
    pub leader: u16,
    /// Deck in stored order (the campaign shuffle runs at match start).
    pub cards: Vec<u16>,
}

/// The parts of `first-duel.json` a match needs.
#[derive(Clone, Debug, Deserialize)]
pub struct ImportDuel {
    pub terrain: Vec<u8>,
    pub first: u8,
    pub sides: [ImportSide; 2],
}

pub struct DuelImport {
    pub catalogue: Catalogue,
    pub first_duel: ImportDuel,
    /// The executable's leader ability table and Destiny Draw rows.
    pub tables: crate::rules::RulesTables,
    /// The executable's boards (49 terrain codes each; empty when the
    /// table cannot be read).
    pub maps: Vec<Vec<u8>>,
    /// The computer opponents' decks, named from MSG_E.DAT (empty when
    /// they cannot be read).
    pub opponents: Vec<crate::deck::OpponentDeck>,
    /// The executable's fusion and equip tables the ported computer
    /// players plan with (the N-player modes' computer seats; none when
    /// the executable is too short).
    pub computer_tables: Option<roses_core::duel::ai::weevil::Tables>,
}

fn read_checked(dir: &Path, manifest: &Manifest, name: &str) -> Result<Vec<u8>, String> {
    let entry = manifest
        .files
        .iter()
        .find(|f| f.path == name)
        .ok_or_else(|| format!("duel import lacks {name}"))?;
    let bytes = std::fs::read(dir.join(name)).map_err(|e| format!("{name}: {e}"))?;
    if format!("{:x}", Sha256::digest(&bytes)) != entry.sha256 {
        return Err(format!("{name} does not match its import hash"));
    }
    Ok(bytes)
}

/// Load `dir` (a duel import directory, or a game import containing
/// `duel/`).
pub fn load_duel_import(dir: &Path) -> Result<DuelImport, String> {
    let dir = if dir.join("manifest.json").is_file() {
        dir.to_path_buf()
    } else {
        dir.join("duel")
    };
    let manifest: Manifest = serde_json::from_slice(
        &std::fs::read(dir.join("manifest.json"))
            .map_err(|e| format!("{}: {e}", dir.join("manifest.json").display()))?,
    )
    .map_err(|e| e.to_string())?;
    if manifest.schema != DUEL_IMPORT_SCHEMA {
        return Err(format!("unsupported duel import {}", manifest.schema));
    }
    let catalogue: Catalogue =
        serde_json::from_slice(&read_checked(&dir, &manifest, "catalogue.json")?)
            .map_err(|e| e.to_string())?;
    let first_duel: ImportDuel =
        serde_json::from_slice(&read_checked(&dir, &manifest, "first-duel.json")?)
            .map_err(|e| e.to_string())?;
    let executable = read_checked(&dir, &manifest, "SLES_515.79")?;
    let tables = crate::rules::RulesTables::from_executable(&executable)?;
    let maps = roses_assets::parse_pal_rules(&executable).map(|r| r.maps.into_iter().map(|m| m.cells).collect()).unwrap_or_default();
    let messages = read_checked(&dir, &manifest, "MSG_E.DAT").ok();
    let opponents = crate::deck::opponent_decks(&executable, messages.as_deref()).unwrap_or_default();
    let computer_tables = roses_core::duel::ai::weevil::Tables::new(&catalogue.fusions, &executable);
    Ok(DuelImport {
        catalogue,
        first_duel,
        tables,
        maps,
        opponents,
        computer_tables,
    })
}
