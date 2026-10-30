//! Invented test data (not retail card data): a small catalogue of plain
//! monsters and two 40-card decks, so the protocol and server tests run
//! without the user's import. Real-data runs use `data::load_duel_import`.

use roses_core::duel::{kind, terrain, CardInfo, Catalogue};

fn monster(id: u16, kind: u8, level: u8, attack: u16, defense: u16) -> CardInfo {
    CardInfo {
        id,
        kind,
        level,
        attribute: 3,
        attack,
        defense,
        effects: [false; 5],
        toon: false,
        kind_index: 0,
        entries: [None; 5],
        equip_bits: [0; 7],
        power_up: None,
    }
}

/// Ids 0 and 1 are leaders; 2..=29 plain monsters of levels 1..=4.
pub fn catalogue() -> Catalogue {
    let kinds = [kind::WARRIOR, kind::BEAST, kind::MACHINE, kind::DRAGON];
    let mut cards = vec![
        monster(0, kind::SPELLCASTER, 6, 1500, 1500),
        monster(1, kind::WARRIOR, 6, 1500, 1500),
    ];
    for id in 2..30u16 {
        let level = (id % 4) as u8 + 1;
        cards.push(monster(
            id,
            kinds[usize::from(id) % kinds.len()],
            level,
            400 + 300 * u16::from(level) + 10 * id,
            300 + 200 * u16::from(level),
        ));
    }
    Catalogue::new(cards, [])
}

/// Two 40-card deck sources.
pub fn decks() -> [Vec<u16>; 2] {
    [
        (0..40).map(|i| 2 + (i % 28) as u16).collect(),
        (0..40).map(|i| 2 + ((i * 7 + 3) % 28) as u16).collect(),
    ]
}

pub fn leaders() -> [u16; 2] {
    [0, 1]
}

pub fn board() -> Vec<u8> {
    vec![terrain::NORMAL; 49]
}

/// Two decks that follow the original's deck rules (40 cards, at most three
/// copies of a card), as seats submit them.
pub fn submissions() -> [crate::deck::DeckSubmission; 2] {
    let deck = |step: usize, offset: usize| -> Vec<u16> {
        (0..40).map(|i| 2 + ((i * step + offset) % 28) as u16).collect()
    };
    [
        crate::deck::DeckSubmission {
            leader: leaders()[0],
            leader_rank: 1,
            cards: deck(1, 0),
        },
        crate::deck::DeckSubmission {
            leader: leaders()[1],
            leader_rank: 3,
            cards: deck(3, 5),
        },
    ]
}
