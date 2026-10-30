//! The port's memory card: MEMORY CARD slot 1 as a folder of the user's
//! local data (never shipped), holding the original's data file
//! `BESLES-51579/BESLES-51579` byte for byte (`roses_core::memory_card`),
//! plus what the map/title dialogs need around it (Claude, 4 Oct 2026;
//! `work/research/story/claude-save/NOTES.md`).
//!
//! Explicit differences from a PS2 card: the virtual card is always
//! present and formatted with room to spare (the "no card", "unformatted"
//! and "insufficient space" boxes exist in the dialog model but cannot
//! occur here); the icon files (icon.sys, icon00-02.ico, built from
//! HOME/Merge/Icon.Mrg) are not written; a sidecar `port.json` keeps the
//! port's state the original keeps nowhere in this file (the starter-deck
//! choice record and the name-entry glyph positions; the shared LCG is not
//! saved at all, as in the original [H]).
//!
//! Migration: the interim JSON save (`saves/interim-slot1.json`, 3 Oct) is
//! converted into this card by [`migrate_interim`] the first time the
//! title's Continue finds no card file (the JSON is left in place).
use roses_client::campaign_map::CampaignSave;
use roses_client::memory_card::{self as mc, CardIo, CardStatus};
use roses_client::{CampaignState, PlayerName, StarterDeckChoice};
use roses_core::card_block::CardBlock;
use roses_core::memory_card::{SaveFile, FILE_BYTES, PAL_NAME, SAVE_BYTES};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const PORT_SCHEMA: &str = "roses-memory-card-port-1";
const MAX_PORT_BYTES: u64 = 1024 * 1024;

/// MEMORY CARD slot 1: `<game imports folder>/memory-card/slot1/`, beside
/// the versioned game imports (a newer import keeps the card).
/// `ROSES_SAVE_DIR` replaces `<game imports folder>` (test runs).
#[derive(Clone, Debug)]
pub struct VirtualCard {
    root: PathBuf,
    /// Written beside the data file by the next successful write.
    pub sidecar: Option<PortSidecar>,
    /// MEMORY CARD slot 2 (`memory-card/slot2/`): present only while its
    /// folder exists; an `UNFORMATTED` file in it makes it unformatted.
    /// Slot 1 is always present and formatted.
    optional: bool,
}

/// The marker file that makes a slot-2 virtual card unformatted.
pub const UNFORMATTED_MARKER: &str = "UNFORMATTED";

/// The port's state that has no place in the original's file.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortSidecar {
    pub schema: String,
    pub note: String,
    pub campaign: CampaignState,
    #[serde(default)]
    pub player_name: Option<PlayerName>,
}

impl PortSidecar {
    pub fn new(campaign: CampaignState, player_name: Option<PlayerName>) -> Self {
        Self {
            schema: PORT_SCHEMA.into(),
            note: "Port state beside the original's save file (not part of it).".into(),
            campaign,
            player_name,
        }
    }
}

impl VirtualCard {
    pub fn slot1(game_root: &Path) -> Self {
        let base = match std::env::var_os("ROSES_SAVE_DIR") {
            Some(dir) => PathBuf::from(dir),
            None => game_root.parent().unwrap_or(game_root).to_path_buf(),
        };
        Self::at(base.join("memory-card").join("slot1"))
    }

    pub fn at(root: PathBuf) -> Self {
        Self { root, sidecar: None, optional: false }
    }

    /// MEMORY CARD slot 2 beside slot 1 (`<imports parent>/memory-card/
    /// slot2/`, `ROSES_SAVE_DIR` as for slot 1). No folder = no card (the
    /// title's Custom Duel then seats the computer with a copy of the
    /// player's deck, control 22); an empty folder = a formatted card
    /// without this game's save (control 25, deck 0x32), as does an
    /// `UNFORMATTED` marker file in it; a valid save there = a second
    /// player (Custom Duel's two-player menu, Trade).
    /// `scripts/make-slot2-save.py` copies slot 1's file there for tests.
    pub fn slot2(game_root: &Path) -> Self {
        let base = match std::env::var_os("ROSES_SAVE_DIR") {
            Some(dir) => PathBuf::from(dir),
            None => game_root.parent().unwrap_or(game_root).to_path_buf(),
        };
        Self { root: base.join("memory-card").join("slot2"), sidecar: None, optional: true }
    }

    /// The card's folder (slot 1 or 2).
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn directory(&self) -> PathBuf {
        self.root.join(PAL_NAME)
    }

    pub fn data_path(&self) -> PathBuf {
        self.directory().join(PAL_NAME)
    }

    fn sidecar_path(&self) -> PathBuf {
        self.directory().join("port.json")
    }

    /// The data file's SAVE when the card has a valid file (the base the
    /// next save writes the port's fields over).
    pub fn current_save(&self) -> Option<[u8; SAVE_BYTES]> {
        let bytes = crate::bounded_read(&self.data_path(), FILE_BYTES as u64).ok()?;
        SaveFile::parse(&bytes).ok().filter(|p| p.checksum_ok).map(|p| p.file.save)
    }

    pub fn write_sidecar(&self, sidecar: &PortSidecar) -> Result<(), String> {
        std::fs::create_dir_all(self.directory()).map_err(|e| format!("memory card: {e}"))?;
        let json = serde_json::to_vec_pretty(sidecar).map_err(|e| e.to_string())?;
        replace(&self.sidecar_path(), &json)
    }

    pub fn read_sidecar(&self) -> Option<PortSidecar> {
        let bytes = crate::bounded_read(&self.sidecar_path(), MAX_PORT_BYTES).ok()?;
        serde_json::from_slice::<PortSidecar>(&bytes)
            .ok()
            .filter(|s| s.schema == PORT_SCHEMA)
    }
}

/// Write beside, then replace: a failed write never loses the old file.
fn replace(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let staging = target.with_extension("new");
    std::fs::write(&staging, bytes).map_err(|e| format!("memory card {}: {e}", staging.display()))?;
    std::fs::rename(&staging, target).map_err(|e| format!("memory card {}: {e}", target.display()))
}

impl CardIo for VirtualCard {
    fn status(&self) -> CardStatus {
        if self.optional && !self.root.is_dir() {
            CardStatus::Missing
        } else if self.optional && self.root.join(UNFORMATTED_MARKER).is_file() {
            CardStatus::Unformatted
        } else {
            CardStatus::Formatted { free_kb: u32::MAX }
        }
    }
    fn has_save(&self) -> bool {
        self.status() != CardStatus::Unformatted && self.directory().is_dir()
    }
    fn read(&mut self) -> Result<Vec<u8>, String> {
        crate::bounded_read(&self.data_path(), FILE_BYTES as u64)
    }
    fn write(&mut self, file: &[u8]) -> Result<(), String> {
        std::fs::create_dir_all(self.directory()).map_err(|e| format!("memory card: {e}"))?;
        replace(&self.data_path(), file)?;
        if let Some(sidecar) = &self.sidecar {
            self.write_sidecar(sidecar)?;
        }
        Ok(())
    }
    fn format(&mut self) -> Result<(), String> {
        Err("the virtual memory card is always formatted".into())
    }
}

/// The data file a map save writes: the player's block and SAVE = the
/// map's state over the card's current SAVE (unmodelled bytes carried).
pub fn build_file(
    block: &CardBlock,
    map: &CampaignSave,
    name_codes: &[u16],
    base: Option<&[u8; SAVE_BYTES]>,
) -> Result<Vec<u8>, String> {
    let block = block.to_bytes().map_err(|e| format!("card block: {e:?}"))?;
    let save = mc::save_record(base, map, name_codes);
    SaveFile::new(block, save)
        .and_then(|f| f.to_bytes())
        .map_err(|e| e.to_string())
}

/// What Continue restores from a loaded file.
#[derive(Clone, Debug)]
pub struct Restored {
    pub block: CardBlock,
    pub record: mc::LoadedRecord,
}

pub fn restore(bytes: &[u8]) -> Result<Restored, String> {
    let parsed = SaveFile::parse(bytes).map_err(|e| e.to_string())?;
    if !parsed.checksum_ok {
        return Err("the save file's checksum does not match".into());
    }
    let block = CardBlock::from_bytes(&parsed.file.block).map_err(|e| format!("card block: {e:?}"))?;
    Ok(Restored {
        block,
        record: mc::load_record(&parsed.file.save),
    })
}

/// The campaign state after Continue: the sidecar's when present, else
/// rebuilt from SAVE (side, name words) with the starter record unknown
/// (0) [explicit: the original file does not keep it].
pub fn campaign_after_load(card: &VirtualCard, record: &mc::LoadedRecord) -> (CampaignState, Option<PlayerName>) {
    if let Some(s) = card.read_sidecar() {
        let mut campaign = s.campaign;
        if let Some(c) = campaign.starter_deck.as_mut() {
            c.name_codes = record.name_codes.clone();
        }
        return (campaign, s.player_name);
    }
    let side = match record.map.side {
        0 => roses_client::CampaignSide::Lancastrians,
        _ => roses_client::CampaignSide::Yorkists,
    };
    let campaign = CampaignState {
        starter_deck: Some(StarterDeckChoice {
            name_codes: record.name_codes.clone(),
            checksum: 0,
            candidates: Vec::new(),
            selection: 0,
            record: 0,
            leader: 0,
            cards: Vec::new(),
        }),
        side: Some(side),
    };
    (campaign, None)
}

/// The interim JSON save → this card (once, when the card has no file).
pub fn migrate_interim(game_root: &Path, card: &VirtualCard) -> Result<bool, String> {
    if card.data_path().is_file() {
        return Ok(false);
    }
    let Some(interim) = crate::interim_save::read(game_root)? else {
        return Ok(false);
    };
    let block = interim.block()?;
    let name_codes = interim
        .campaign
        .starter_deck
        .as_ref()
        .map(|c| c.name_codes.clone())
        .unwrap_or_default();
    let mut writer = card.clone();
    writer.sidecar = Some(PortSidecar::new(interim.campaign, interim.player_name));
    let file = build_file(&block, &interim.map, &name_codes, None)?;
    writer.write(&file)?;
    Ok(true)
}

// ------------------------------------------------------------------ drawing
/// The overlay boxes (0x1E5540 kinds 5/6/7) as a premultiplied 512×512 GS
/// layer, from the duel import's executable, MSG_E.DAT and TEXSYS.MRG.
pub struct BoxRenderer {
    tables: roses_client::duel_hud::HudTables,
    textures: roses_client::duel_hud::HudTextures,
    yes_no: roses_client::duel_hud::YesNoTables,
}

/// The GS canvas of the HUD layers.
pub const CANVAS: usize = 512;

impl BoxRenderer {
    pub fn new(executable: &[u8], message_file: &[u8], texsys: &[u8], picmini: &[u8]) -> Result<Self, String> {
        use roses_client::duel_hud::{HudTables, HudTextures, YesNoTables};
        let mut codes = std::collections::BTreeMap::new();
        let parsed = roses_assets::parse_pal_messages(message_file, &mc::msg::ALL).map_err(|e| e.to_string())?;
        for m in parsed.messages {
            codes.insert(m.message_id, m.expanded_codes);
        }
        Ok(Self {
            tables: HudTables::from_executable(executable, codes)?,
            textures: HudTextures::new(texsys, picmini, [0, 0], message_file)?,
            yes_no: YesNoTables::from_executable(executable)?,
        })
    }

    pub fn from_duel_import(duel: &Path) -> Result<Self, String> {
        let data = crate::duel_play::load_with_end(duel, None)?;
        Self::new(&data.executable, &data.messages, &data.texsys, &data.picmini)
    }

    /// The box as primitives (to append to another HUD's primitives).
    pub fn primitives(&self, b: mc::CardBox, counter: u32) -> Vec<roses_client::duel_hud::Primitive> {
        let frame = roses_client::duel_hud::overlay_box(&self.tables, &self.yes_no, &overlay(b), counter);
        for u in &frame.unsupported {
            eprintln!("Memory-card box: unsupported {:?}", u.kind);
        }
        frame.prims
    }

    /// The box alone as a premultiplied RGBA 512×512 layer.
    pub fn layer(&mut self, b: mc::CardBox, counter: u32) -> Vec<u8> {
        let prims = self.primitives(b, counter);
        roses_client::duel_hud::render_primitives_layer(&prims, &self.tables, &mut self.textures, CANVAS, CANVAS).0
    }
}

/// The dialog box → the overlay drawer's slot-3 record (x 0, y 0).
pub fn overlay(b: mc::CardBox) -> roses_client::duel_hud::OverlayBox {
    use roses_client::duel_hud::{OverlayBox, OverlayKind};
    let (kind, text) = match b {
        mc::CardBox::Busy(t) => (OverlayKind::Centred, t),
        mc::CardBox::Acknowledge(t) => (OverlayKind::Press, t),
        mc::CardBox::YesNo { text, yes } => (OverlayKind::YesNo { yes }, text),
    };
    OverlayBox { kind, x: 0, y: 0, text }
}

/// Composite a premultiplied 512×512 GS layer over a straight-alpha RGBA
/// image of another size (the title's 640×448 overlay): the 512-wide GS
/// frame spans the image (nearest sample) [H: the vertical 512 → 448
/// mapping follows the 4:3 presentation of both].
pub fn blend_layer(image: &mut [u8], width: usize, height: usize, layer: &[u8]) {
    if width == 0 || height == 0 || image.len() < width * height * 4 || layer.len() < CANVAS * CANVAS * 4 {
        return;
    }
    for y in 0..height {
        let sy = y * CANVAS / height;
        for x in 0..width {
            let sx = x * CANVAS / width;
            let s = &layer[(sy * CANVAS + sx) * 4..(sy * CANVAS + sx) * 4 + 4];
            let a = u32::from(s[3]);
            if a == 0 {
                continue;
            }
            let d = &mut image[(y * width + x) * 4..(y * width + x) * 4 + 4];
            let da = u32::from(d[3]);
            // out = src (premultiplied) + dst · (1 − a), back to straight.
            let out_a = a + da * (255 - a) / 255;
            for k in 0..3 {
                let dp = u32::from(d[k]) * da / 255;
                let c = u32::from(s[k]) + dp * (255 - a) / 255;
                d[k] = if out_a == 0 { 0 } else { (c * 255 / out_a).min(255) as u8 };
            }
            d[3] = out_a.min(255) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("roses-card-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_map_save_round_trips_through_the_virtual_card() {
        let dir = temp("rt");
        let mut card = VirtualCard::at(dir.join("slot1"));
        assert!(!card.has_save());
        let block = CardBlock::empty(1);
        let mut map = CampaignSave::after_landing();
        map.event = 303;
        map.wins = 1;
        let file = build_file(&block, &map, &[0x2005; 12], None).unwrap();
        assert_eq!(file.len(), FILE_BYTES);
        card.write(&file).unwrap();
        assert!(card.has_save());
        assert_eq!(card.data_path(), dir.join("slot1").join("BESLES-51579").join("BESLES-51579"));
        let back = restore(&card.read().unwrap()).unwrap();
        assert_eq!(back.record.map, {
            let mut m = map.clone();
            m.wins_by_opponent.resize(32, 0);
            m.losses_by_opponent.resize(32, 0);
            m
        });
        assert_eq!(back.block.to_bytes().unwrap(), block.to_bytes().unwrap());
        assert!(card.current_save().is_some());
        let (campaign, name) = campaign_after_load(&card, &back.record);
        assert_eq!(campaign.side, Some(roses_client::CampaignSide::Lancastrians));
        assert_eq!(campaign.starter_deck.unwrap().name_codes, vec![0x2005; 12]);
        assert!(name.is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_interim_json_save_migrates_once() {
        let dir = temp("mig");
        let game = dir.join("game-x");
        std::fs::create_dir_all(&game).unwrap();
        // The interim save's own writer (ROSES_SAVE_DIR is not set here:
        // its file goes to <dir>/saves).
        let map = CampaignSave::after_landing();
        let interim = crate::interim_save::InterimSave::new(CampaignState::default(), None, &CardBlock::empty(1), map.clone(), 5).unwrap();
        let target = dir.join("saves").join("interim-slot1.json");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, serde_json::to_vec(&interim).unwrap()).unwrap();
        let card = VirtualCard::at(dir.join("memory-card").join("slot1"));
        if std::env::var_os("ROSES_SAVE_DIR").is_none() {
            assert!(migrate_interim(&game, &card).unwrap());
            assert!(!migrate_interim(&game, &card).unwrap());
            let back = restore(&std::fs::read(card.data_path()).unwrap()).unwrap();
            assert_eq!(back.record.map.node_states, map.node_states);
            assert!(card.read_sidecar().is_some());
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn slot_two_is_absent_empty_unformatted_or_saved_by_its_folder() {
        use roses_client::memory_card::{CardDialog, CardOutcome, NoCard, op3};
        let dir = temp("slot2");
        let mut slot2 = VirtualCard::at(dir.join("slot2"));
        slot2.optional = true;
        assert_eq!(slot2.status(), CardStatus::Missing);
        std::fs::create_dir_all(slot2.root()).unwrap();
        assert_eq!(slot2.status(), CardStatus::Formatted { free_kb: u32::MAX });
        assert!(!slot2.has_save());
        std::fs::write(slot2.root().join(UNFORMATTED_MARKER), b"").unwrap();
        assert_eq!(slot2.status(), CardStatus::Unformatted);
        // Op 3 with slot 1 holding a save: the result word by slot 2.
        let mut slot1 = VirtualCard::at(dir.join("slot1"));
        let file = build_file(&CardBlock::empty(1), &CampaignSave::after_landing(), &[0x2005; 12], None).unwrap();
        slot1.write(&file).unwrap();
        let run = |slot1: &mut VirtualCard, slot2: &mut dyn CardIo| {
            let mut d = CardDialog::custom_duel();
            for _ in 0..40 {
                let pad = roses_client::memory_card::CardPad { cross: true, ..Default::default() };
                if let CardOutcome::CustomDuel { result, .. } = d.update_slots(pad, slot1, slot2) {
                    return result;
                }
            }
            panic!("op 3 did not finish");
        };
        assert_eq!(run(&mut slot1, &mut slot2), op3::NO_SAVE);
        assert_eq!(run(&mut slot1, &mut NoCard), op3::NO_CARD);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_premultiplied_layer_blends_over_a_straight_image() {
        let mut layer = vec![0u8; CANVAS * CANVAS * 4];
        layer[..4].copy_from_slice(&[128, 0, 0, 255]);
        let mut image = vec![0u8; 640 * 448 * 4];
        image[4..8].copy_from_slice(&[0, 0, 255, 255]);
        blend_layer(&mut image, 640, 448, &layer);
        assert_eq!(&image[..4], &[128, 0, 0, 255]);
    }
}
