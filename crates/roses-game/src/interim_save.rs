//! The interim campaign save (Jim's choice, 3 Oct 2026), superseded on 4 Oct
//! by the original's memory-card save (`memory_card_store`, `roses_client::
//! memory_card`, `roses_core::memory_card`): SYSTEM → SAVE now runs the
//! card dialogs and writes the original's 0x100F0-byte file to a virtual
//! card. This module is kept only to read an existing interim JSON once:
//! the title's Continue converts it into the card when the card is empty
//! (`memory_card_store::migrate_interim`).
//!
//! The file keeps what the original's file keeps that the port has: the
//! player's card block (its own byte layout), the map's SAVE fields
//! (`CampaignSave`), plus the port's state the original keeps elsewhere in
//! SAVE (the starter choice with the player's name codes, the story's
//! committed name, the shared LCG).
use roses_client::campaign_map::CampaignSave;
use roses_client::{CampaignState, PlayerName};
use roses_core::card_block::CardBlock;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SCHEMA: &str = "roses-interim-save-1";
const MAX_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InterimSave {
    pub schema: String,
    pub note: String,
    pub campaign: CampaignState,
    #[serde(default)]
    pub player_name: Option<PlayerName>,
    /// The card block as `CardBlock::to_bytes`, lower-case hex.
    pub card_block: String,
    pub map: CampaignSave,
    pub rng: u64,
}

impl InterimSave {
    pub fn new(
        campaign: CampaignState,
        player_name: Option<PlayerName>,
        block: &CardBlock,
        map: CampaignSave,
        rng: u64,
    ) -> Result<Self, String> {
        let bytes = block.to_bytes().map_err(|e| format!("card block: {e:?}"))?;
        Ok(Self {
            schema: SCHEMA.into(),
            note: "Interim save of the port (not the original's memory-card format).".into(),
            campaign,
            player_name,
            card_block: bytes.iter().map(|b| format!("{b:02x}")).collect(),
            map,
            rng,
        })
    }

    pub fn block(&self) -> Result<CardBlock, String> {
        if self.card_block.len() % 2 != 0 {
            return Err("save: card block hex has an odd length".into());
        }
        let bytes = (0..self.card_block.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&self.card_block[i..i + 2], 16))
            .collect::<Result<Vec<u8>, _>>()
            .map_err(|e| format!("save: card block hex: {e}"))?;
        CardBlock::from_bytes(&bytes).map_err(|e| format!("save: card block: {e:?}"))
    }
}

/// `<game imports folder>/saves/interim-slot1.json`: beside the versioned
/// game imports, so a newer import keeps the save. `ROSES_SAVE_DIR`
/// replaces the folder (test runs).
pub fn path(game_root: &Path) -> PathBuf {
    if let Some(dir) = std::env::var_os("ROSES_SAVE_DIR") {
        return PathBuf::from(dir).join("interim-slot1.json");
    }
    let base = game_root.parent().unwrap_or(game_root);
    base.join("saves").join("interim-slot1.json")
}

pub fn write(game_root: &Path, save: &InterimSave) -> Result<PathBuf, String> {
    let target = path(game_root);
    let folder = target.parent().ok_or("save: no folder")?;
    std::fs::create_dir_all(folder).map_err(|e| format!("save: {e}"))?;
    let json = serde_json::to_vec_pretty(save).map_err(|e| format!("save: {e}"))?;
    // Write beside, then replace: a failed write never loses the old save.
    let staging = target.with_extension("json.new");
    std::fs::write(&staging, &json).map_err(|e| format!("save: {e}"))?;
    std::fs::rename(&staging, &target).map_err(|e| format!("save: {e}"))?;
    Ok(target)
}

pub fn read(game_root: &Path) -> Result<Option<InterimSave>, String> {
    let target = path(game_root);
    if !target.is_file() {
        return Ok(None);
    }
    let bytes = crate::bounded_read(&target, MAX_BYTES)?;
    let save: InterimSave =
        serde_json::from_slice(&bytes).map_err(|e| format!("save {}: {e}", target.display()))?;
    if save.schema != SCHEMA {
        return Err(format!("save {}: unsupported schema {}", target.display(), save.schema));
    }
    save.block()?;
    Ok(Some(save))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_save_round_trips_through_its_file() {
        let dir = std::env::temp_dir().join(format!("roses-save-test-{}", std::process::id()));
        let game = dir.join("game-x");
        std::fs::create_dir_all(&game).unwrap();
        let block = CardBlock::empty(1);
        let mut map = CampaignSave::after_landing();
        map.wins = 3;
        let save = InterimSave::new(CampaignState::default(), None, &block, map.clone(), 77).unwrap();
        let written = write(&game, &save).unwrap();
        assert_eq!(written, dir.join("saves").join("interim-slot1.json"));
        let back = read(&game).unwrap().unwrap();
        assert_eq!(back.map, map);
        assert_eq!(back.rng, 77);
        assert_eq!(back.block().unwrap().to_bytes().unwrap(), block.to_bytes().unwrap());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
