//! Campaign state chosen during the opening (PAL SLES_515.79), kept so a later
//! step can start the first campaign duel with it. Source facts (see the
//! private research `story/claude-to-first-duel/FLOW.md`):
//!
//! - Name commit 0x224890 stores twelve codes at 0x299214; slots after the
//!   entered name keep the editor's unused selection (page 0, row 5, column 0,
//!   the blank glyph) and the last selected slot carries end bit 0x8000.
//!   0x224AB8 sums the low four bits of all twelve words and masks the sum to
//!   four bits (0x1ED53F8).
//! - That checksum indexes the u16 table 0x2990C0; candidate `k` is
//!   `(word >> 5k) & 0x1F`, a static deck record (0x1ED53FA..); the leader is
//!   the low ten bits of word 0 of the 12-byte info record 0x2990E0 + 12·deck.
//! - Deck select input 0x23916C: Right +1, Left −1 (both wrap over three),
//!   Start confirms; Cross does nothing. The loader 0x214120(0, record) then
//!   fills the player's deck buffer with the record's 40 cards in table order.
//!   Its serial (rand-based) and the slide/fly-in animations are not modelled.
//! - Side choice 0x21ED18: Lancastrians sets story byte 0x2991F0 = 0 (event
//!   11000), Yorkists sets 1 (event 12000, not traced).
use crate::{Layer, PlayerName, Result};
use serde::{Deserialize, Serialize};

/// Stored name words, including blank-filled unused slots.
pub const NAME_SLOTS: usize = 12;
/// Starter decks shown by the deck select.
pub const CANDIDATES: usize = 3;
/// Cards per static deck record (loader 0x214120).
pub const DECK_CARDS: usize = 40;
/// End flag on the last selected name slot (commit 0x224A6C).
pub const NAME_END_BIT: u16 = 0x8000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignSide {
    /// Red rose, story byte 0x2991F0 = 0.
    Lancastrians,
    /// White rose, story byte 0x2991F0 = 1.
    Yorkists,
}
impl CampaignSide {
    /// Original story byte 0x2991F0; also the first-duel parameter p2.
    pub fn story_byte(self) -> u8 {
        match self {
            Self::Lancastrians => 0,
            Self::Yorkists => 1,
        }
    }
}

/// One static deck record offered by the deck select.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StarterDeck {
    /// Static deck record (EE 0x2D10E0 + 82·record).
    pub record: u8,
    /// Low ten bits of info word 0 (0x2990E0 + 12·record).
    pub leader: u16,
    /// Info word 1, shown by the original as the average summon level ×100.
    pub average_level_x100: u16,
    /// The leader's card-name message text (presentation data only).
    pub leader_name: String,
    /// The record's 40 cards in table order.
    pub cards: Vec<u16>,
    /// Optional imported name, level and type panel for this record. Older
    /// imports have no panel; this is presentation data, not duel state.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presentation_layers: Vec<Layer>,
    /// Imported 256×368 face of the leader card (`card_face`), drawn in the
    /// three-card stack. Older imports have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_face: Option<String>,
}

/// The three-card stack of the deck select at rest (0x229EF0): draw order
/// back to front, candidate `(selection + 2 − k) % 3` for draw index `k`,
/// x0 = 186 + term (terms 54, 36, 18 at offset 100), 120 × 241.69 raw pixels
/// from y 124.375 [F for the terms and order; emu for the placed quads:
/// 0x2073E0's vertex list in savestate 437-deck1]. Logical 640×448 canvas
/// (raw x·5/4, y·7/8, rounded as the other deck-select layers).
pub const STACK_RECTS: [[u32; 4]; CANDIDATES] = [
    [300, 109, 150, 212],
    [278, 109, 150, 212],
    [255, 109, 150, 212],
];

/// The stack layers for a selection: the leaders' faces back to front, or
/// `None` when an older import has no faces.
pub fn stack_layers(decks: [&StarterDeck; CANDIDATES], selection: usize) -> Option<Vec<Layer>> {
    (0..CANDIDATES)
        .map(|k| {
            let deck = decks[(selection + 2 + CANDIDATES - k) % CANDIDATES];
            deck.card_face.as_ref().map(|asset| Layer {
                asset: asset.clone(),
                source: None,
                destination: STACK_RECTS[k],
                tint: [255; 3],
                opacity: 255,
                flip_x: false,
                filter: crate::Sampling::Linear,
            })
        })
        .collect()
}

/// Imported deck-select data. Records are stored by index (0..=16).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeckChoiceConfig {
    /// u16 table 0x2990C0, indexed by the name checksum.
    pub candidate_table: Vec<u16>,
    /// Stored code of the editor's unused selection (page 0, row 5, column 0).
    pub blank_name_code: u16,
    pub decks: Vec<StarterDeck>,
}
impl DeckChoiceConfig {
    pub fn validate(&self) -> Result<()> {
        if self.candidate_table.len() != 16 || self.decks.is_empty() || self.decks.len() > 32 {
            return Err("Deck choice needs 16 candidate words and 1..32 deck records.".into());
        }
        for (index, deck) in self.decks.iter().enumerate() {
            if usize::from(deck.record) != index
                || deck.cards.len() != DECK_CARDS
                || deck.leader_name.len() > 64
                || deck.leader_name.chars().any(char::is_control)
            {
                return Err("Deck choice records must be ordered with 40 cards each.".into());
            }
        }
        for &word in &self.candidate_table {
            for k in 0..CANDIDATES {
                if usize::from((word >> (5 * k)) & 0x1F) >= self.decks.len() {
                    return Err("Deck candidate table references a missing record.".into());
                }
            }
        }
        if self.blank_name_code == 0 || self.blank_name_code >= 0x4000 {
            return Err("Deck choice blank name code must be a direct glyph code.".into());
        }
        Ok(())
    }
    /// The twelve stored words at 0x299214 as commit 0x224890 writes them:
    /// direct codes (`& 0x3FFF`), blank-filled after the name, with the end
    /// bit 0x8000 on the last selected slot (0x1ED53D0 = length − 1; ORed at
    /// 0x224A5C..0x224A70 before `sh` to 0x299214 + 2·slot).
    pub fn name_codes(&self, name: &PlayerName) -> Result<[u16; NAME_SLOTS]> {
        if name.glyphs.is_empty() || name.glyphs.len() > NAME_SLOTS {
            return Err("Deck choice needs a committed name of 1..12 glyphs.".into());
        }
        let mut codes = [self.blank_name_code; NAME_SLOTS];
        for (code, glyph) in codes.iter_mut().zip(&name.glyphs) {
            *code = glyph.stored_code & 0x3FFF;
        }
        codes[name.glyphs.len() - 1] |= NAME_END_BIT;
        Ok(codes)
    }
    /// 0x224AB8: low nibbles of the twelve words, summed, masked to 4 bits.
    pub fn checksum(&self, name: &PlayerName) -> Result<u8> {
        let sum: u32 = self
            .name_codes(name)?
            .iter()
            .map(|&code| u32::from(code & 0xF))
            .sum();
        Ok((sum & 0xF) as u8)
    }
    /// Three candidate deck records for the committed name (0x224B28).
    pub fn candidates(&self, name: &PlayerName) -> Result<[u8; CANDIDATES]> {
        let word = self.candidate_table[usize::from(self.checksum(name)?)];
        Ok(std::array::from_fn(|k| ((word >> (5 * k)) & 0x1F) as u8))
    }
    pub fn deck(&self, record: u8) -> Option<&StarterDeck> {
        self.decks.get(usize::from(record))
    }
}

/// The confirmed deck select (0x1ED53F8, 0x1ED53FA.., 0x1ED53AA).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StarterDeckChoice {
    /// The twelve words at 0x299214, end bit included (see `name_codes`).
    pub name_codes: Vec<u16>,
    pub checksum: u8,
    pub candidates: Vec<u8>,
    pub selection: u8,
    /// Chosen static record; the first campaign duel's player deck.
    pub record: u8,
    pub leader: u16,
    /// Deck buffer contents after loader 0x214120 (table order, no shuffle).
    pub cards: Vec<u16>,
}

/// Serializable campaign progress from the opening. A later step starts the
/// first duel from it (player deck = `starter_deck.record`, first player and
/// duel parameter p2 = `side.story_byte()`); nothing here is persisted to a
/// memory card (the original writes none in this segment either).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignState {
    #[serde(default)]
    pub starter_deck: Option<StarterDeckChoice>,
    #[serde(default)]
    pub side: Option<CampaignSide>,
}
impl CampaignState {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("campaign state serializes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NameCharacter;

    fn config() -> DeckChoiceConfig {
        // Synthetic table and decks; retail values come from the user's disc.
        let decks = (0..17u8)
            .map(|record| StarterDeck {
                record,
                leader: 100 + u16::from(record),
                average_level_x100: 300,
                leader_name: format!("leader {record}"),
                cards: vec![u16::from(record); DECK_CARDS],
                presentation_layers: vec![],
                card_face: None,
            })
            .collect();
        let mut candidate_table: Vec<u16> = (0..16u16)
            .map(|i| (i % 17) | ((i + 1) % 17) << 5 | ((i + 2) % 17) << 10)
            .collect();
        candidate_table[12] = 9 | 13 << 5 | 16 << 10;
        DeckChoiceConfig {
            candidate_table,
            blank_name_code: 0x2001,
            decks,
        }
    }
    fn name(codes: &[u16]) -> PlayerName {
        PlayerName {
            glyphs: codes
                .iter()
                .map(|&code| NameCharacter {
                    page: 0,
                    row: 0,
                    column: 0,
                    display_code: code,
                    stored_code: code,
                })
                .collect(),
        }
    }

    #[test]
    fn twelve_codes_sum_low_nibbles_like_the_original() {
        let config = config();
        // 12 × 0x2005: 60 & 0xF = 12 → word 12 → records 9, 13, 16.
        let full = name(&[0x2005; 12]);
        let mut stored = [0x2005; 12];
        stored[11] = 0xA005;
        assert_eq!(config.name_codes(&full).unwrap(), stored);
        assert_eq!(config.checksum(&full).unwrap(), 12);
        assert_eq!(config.candidates(&full).unwrap(), [9, 13, 16]);
    }

    #[test]
    fn unused_slots_contribute_the_blank_code() {
        let config = config();
        // Two glyphs plus ten blanks (0x2001): 5 + 5 + 10 × 1 = 20 → 4.
        let short = name(&[0x2005, 0x2005]);
        assert_eq!(config.name_codes(&short).unwrap()[..2], [0x2005, 0xA005]);
        assert_eq!(config.name_codes(&short).unwrap()[2..], [0x2001; 10]);
        assert_eq!(config.checksum(&short).unwrap(), 4);
        // A selected blank keeps the end bit, so equal sums stay distinguishable.
        let with_blank = config.name_codes(&name(&[0x2005, 0x2005, 0x2001])).unwrap();
        assert_eq!(with_blank[..3], [0x2005, 0x2005, 0xA001]);
        assert_eq!(
            config.checksum(&name(&[0x2005, 0x2005, 0x2001])).unwrap(),
            4
        );
        // An end flag above bit 13 never reaches the nibble sum.
        assert_eq!(config.checksum(&name(&[0xA005, 0x2005])).unwrap(), 4);
        assert!(config.checksum(&name(&[])).is_err());
        assert!(config.checksum(&name(&[0x2005; 13])).is_err());
    }

    #[test]
    fn invalid_tables_are_rejected() {
        let mut bad = config();
        bad.candidate_table[0] = 31;
        assert!(bad.validate().is_err());
        let mut short = config();
        short.decks[3].cards.pop();
        assert!(short.validate().is_err());
        assert!(config().validate().is_ok());
        let state = CampaignState {
            side: Some(CampaignSide::Lancastrians),
            ..CampaignState::default()
        };
        let text = state.to_json();
        assert_eq!(serde_json::from_str::<CampaignState>(&text).unwrap(), state);
        assert_eq!(CampaignSide::Yorkists.story_byte(), 1);
    }
}
