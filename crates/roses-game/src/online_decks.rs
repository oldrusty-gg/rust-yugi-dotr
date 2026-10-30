//! The decks a player can bring online (a port addition), Bevy-free:
//!
//! - the decks of their own save: the card block's three deck records and
//!   the currently selected one (MEMORY CARD slot 1's save, else the
//!   running game's block, else the import's starter deck);
//! - every computer opponent's static deck of the game (campaign and
//!   Custom Duel opponents: Weevil, Pegasus, Seto, ...; records 0x1A..0x32
//!   with the leader and leader rank the game gives them,
//!   `roses_net::deck::opponent_decks`);
//! - their own custom decks (`roses_net::custom_decks`, any card of the
//!   game, built in the deck builder).
//!
//! Any of them can be looked at card by card ([`DeckPreview`], the deck
//! view: the leader and its rank, the 40 cards grouped by card with their
//! copies, sorted by the deck editor's own sort keys) and copied as a new
//! custom deck ([`DeckLibrary::fork`], "NAME (copy)"; the deck copied is
//! never changed).
//!
//! The last choice is remembered in `online-deck.json` beside the session
//! file. The chosen deck is only ever sent to the server, in the match
//! hello; the other player never receives it.

use roses_core::card_block::{CardBlock, EMPTY_CARD};
use roses_core::deck_editor::{self, StaticCards, DECKS_PER_BLOCK};
use roses_net::custom_decks::{CustomDeck, CustomDecks};
use roses_net::deck::OpponentDeck;
use roses_net::lobby::MapChoice;
use roses_net::DeckSubmission;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const CHOICE_FILE: &str = "online-deck.json";
pub const CHOICE_SCHEMA: &str = "roses-online-deck-1";

/// A deck by where it comes from (what `online-deck.json` remembers).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeckRef {
    /// The save's currently selected deck (whichever it is when the match
    /// starts), or the starter deck before there is a save.
    SaveCurrent,
    /// Deck record `deck` (0..=2) of the save.
    Save { deck: u8 },
    /// A computer opponent's static deck (opponent id, record 0x1A + id).
    Opponent { opponent: u8 },
    /// A custom deck by name.
    Custom { name: String },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChoiceFile {
    schema: String,
    choice: DeckRef,
}

/// The remembered choice at `path`, if any.
pub fn load_choice(path: &Path) -> Option<DeckRef> {
    let bytes = std::fs::read(path).ok().filter(|b| b.len() < 16 * 1024)?;
    let file: ChoiceFile = serde_json::from_slice(&bytes).ok()?;
    (file.schema == CHOICE_SCHEMA).then_some(file.choice)
}

pub fn save_choice(path: &Path, choice: &DeckRef) -> std::io::Result<()> {
    let file = ChoiceFile { schema: CHOICE_SCHEMA.into(), choice: choice.clone() };
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&file).map_err(std::io::Error::other)?)?;
    std::fs::rename(&tmp, path)
}

/// Which list a deck is in (the tag its row shows).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Save,
    Opponent,
    Custom,
}

impl Source {
    pub fn tag(self) -> &'static str {
        match self {
            Self::Save => "SAVE",
            Self::Opponent => "CPU",
            Self::Custom => "MINE",
        }
    }
}

/// One deck the player can choose.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeckEntry {
    pub reference: DeckRef,
    pub name: String,
    pub source: Source,
    /// 999 = none.
    pub leader: u16,
    pub leader_rank: u8,
    pub count: usize,
    /// The deck as submitted at join, or why it cannot be played yet.
    pub deck: Result<DeckSubmission, String>,
}

/// Every deck the player can bring, and the custom decks file.
pub struct DeckLibrary {
    /// The save's card block (MEMORY CARD slot 1, else the game's own).
    pub save: Option<CardBlock>,
    /// The deck used when there is no save (the import's starter deck).
    pub starter: Option<DeckSubmission>,
    pub opponents: Vec<OpponentDeck>,
    pub custom: CustomDecks,
    pub custom_path: PathBuf,
    /// The custom decks file could not be read: it is left alone (not
    /// overwritten) and no custom deck can be made.
    pub custom_error: Option<String>,
    /// Card names by id.
    pub card_names: Vec<String>,
    /// Rank names by rank (0 = NCO .. 12 = SD; MSG_E.DAT 83 + rank).
    pub rank_names: Vec<String>,
    /// The cards' static records (the deck view's sort keys; none: by id).
    pub cards: Option<StaticCards>,
    /// The game's boards (the executable's map table): the map choice of
    /// online matches.
    pub maps: Vec<Vec<u8>>,
}

impl DeckLibrary {
    /// The library with the custom decks of `custom_path`.
    pub fn new(
        save: Option<CardBlock>,
        starter: Option<DeckSubmission>,
        opponents: Vec<OpponentDeck>,
        custom_path: &Path,
        card_names: Vec<String>,
        rank_names: Vec<String>,
    ) -> Self {
        let (custom, custom_error) = match CustomDecks::load(custom_path) {
            Ok(c) => (c, None),
            Err(e) => (CustomDecks::default(), Some(e)),
        };
        Self { save, starter, opponents, custom, custom_path: custom_path.to_path_buf(), custom_error, card_names, rank_names, cards: None, maps: Vec::new() }
    }

    /// The game's boards, for the map choice.
    pub fn with_maps(mut self, maps: Vec<Vec<u8>>) -> Self {
        self.maps = maps;
        self
    }

    /// The maps a player may choose for an online match: RANDOM, the
    /// game's boards (Custom Duel's VS maps first, then the campaign's)
    /// and the save's own maps (map editor).
    pub fn map_entries(&self) -> Vec<MapChoice> {
        let mut out = vec![MapChoice::Random];
        let n = self.maps.len().min(usize::from(u8::MAX));
        let vs = (VS_FIRST..n).chain(0..VS_FIRST.min(n));
        out.extend(vs.map(|i| MapChoice::Game { index: i as u8 }));
        if let Some(block) = &self.save {
            for k in 0..roses_core::card_block::OWN_MAP_COUNT {
                if let Some(cells) = block.own_map(k).filter(|c| c.iter().all(|&t| t <= roses_net::lobby::TERRAIN_MAX)) {
                    out.push(MapChoice::Own { name: format!("MY MAP {}", k + 1), cells: cells.to_vec() });
                }
            }
        }
        out
    }

    /// A map choice's cells (none: RANDOM, or a board this game lacks).
    pub fn map_cells(&self, map: &MapChoice) -> Option<Vec<u8>> {
        match map {
            MapChoice::Random => None,
            MapChoice::Game { index } => self.maps.get(usize::from(*index)).cloned(),
            MapChoice::Own { cells, .. } => Some(cells.clone()),
        }
    }

    /// The cards' static records, for the deck view's sort keys.
    pub fn with_cards(mut self, cards: StaticCards) -> Self {
        self.cards = Some(cards);
        self
    }

    pub fn card_name(&self, card: u16) -> String {
        if card == EMPTY_CARD {
            return "no leader".into();
        }
        self.card_names.get(usize::from(card)).cloned().unwrap_or_else(|| format!("card {card}"))
    }

    pub fn rank_name(&self, rank: u8) -> String {
        self.rank_names.get(usize::from(rank)).filter(|s| !s.is_empty()).cloned().unwrap_or_else(|| format!("rank {rank}"))
    }

    fn save_entry(&self, reference: DeckRef) -> Option<DeckEntry> {
        let Some(block) = &self.save else {
            // No save yet: the starter deck stands in for "your deck".
            let deck = self.starter.clone()?;
            return (reference == DeckRef::SaveCurrent).then(|| DeckEntry {
                reference,
                name: "STARTER DECK".into(),
                source: Source::Save,
                leader: deck.leader,
                leader_rank: deck.leader_rank,
                count: deck.cards.len(),
                deck: Ok(deck),
            });
        };
        let current = deck_editor::current_deck(block);
        let (d, name) = match reference {
            DeckRef::SaveCurrent => (current, format!("YOUR DECK ({})", current + 1)),
            DeckRef::Save { deck } if deck < DECKS_PER_BLOCK => (deck, format!("SAVE DECK {}", deck + 1)),
            _ => return None,
        };
        let record = deck_editor::deck_record(block, d);
        Some(DeckEntry {
            reference,
            name,
            source: Source::Save,
            leader: record.leader,
            leader_rank: block.stored_rank(record.leader).map_or(0, |r| u8::try_from(r).unwrap_or(u8::MAX)),
            count: record.count(),
            deck: roses_net::deck::from_card_block(block, Some(d)).map_err(|_| {
                if record.count() < deck_editor::DECK_SIZE {
                    format!("{} of 40 cards", record.count())
                } else {
                    "no leader".into()
                }
            }),
        })
    }

    fn custom_entry(&self, i: usize) -> Option<DeckEntry> {
        let d = self.custom.decks.get(i)?;
        Some(DeckEntry {
            reference: DeckRef::Custom { name: d.name.clone() },
            name: d.name.clone(),
            source: Source::Custom,
            leader: d.leader,
            leader_rank: d.leader_rank,
            count: d.cards.len(),
            deck: d.submission(),
        })
    }

    /// Every deck: the save's (current first), the custom decks, then the
    /// computer opponents'.
    pub fn entries(&self) -> Vec<DeckEntry> {
        let mut out = Vec::new();
        out.extend(self.save_entry(DeckRef::SaveCurrent));
        for d in 0..DECKS_PER_BLOCK {
            out.extend(self.save_entry(DeckRef::Save { deck: d }));
        }
        for i in 0..self.custom.decks.len() {
            out.extend(self.custom_entry(i));
        }
        for o in &self.opponents {
            out.push(DeckEntry {
                reference: DeckRef::Opponent { opponent: o.opponent },
                name: o.name.clone(),
                source: Source::Opponent,
                leader: o.deck.leader,
                leader_rank: o.deck.leader_rank,
                count: o.deck.cards.len(),
                deck: Ok(o.deck.clone()),
            });
        }
        out
    }

    /// The deck a reference names now (`None`: it no longer exists).
    pub fn entry(&self, reference: &DeckRef) -> Option<DeckEntry> {
        match reference {
            DeckRef::SaveCurrent | DeckRef::Save { .. } => self.save_entry(reference.clone()),
            DeckRef::Custom { name } => self.custom.find(name).and_then(|i| self.custom_entry(i)),
            DeckRef::Opponent { .. } => self.entries().into_iter().find(|e| &e.reference == reference),
        }
    }

    /// Write the custom decks file (refused while it could not be read).
    pub fn save_custom(&self) -> Result<(), String> {
        if let Some(e) = &self.custom_error {
            return Err(format!("custom decks not saved: {e}"));
        }
        self.custom.save(&self.custom_path).map_err(|e| format!("{}: {e}", self.custom_path.display()))
    }

    /// An entry's deck as it stands (an unfinished save or custom deck as
    /// far as it goes): leader, leader rank and cards, named as the entry.
    pub fn deck_of(&self, entry: &DeckEntry) -> CustomDeck {
        if let Ok(d) = &entry.deck {
            return CustomDeck { name: entry.name.clone(), leader: d.leader, leader_rank: d.leader_rank, cards: d.cards.clone() };
        }
        let mut d = CustomDeck { name: entry.name.clone(), leader: entry.leader, leader_rank: entry.leader_rank, cards: Vec::new() };
        match (&entry.reference, &self.save) {
            (DeckRef::Save { .. } | DeckRef::SaveCurrent, Some(block)) => {
                let n = match entry.reference {
                    DeckRef::Save { deck } => deck,
                    _ => deck_editor::current_deck(block),
                };
                let r = deck_editor::deck_record(block, n);
                d.cards = r.cards.iter().copied().filter(|&c| c != EMPTY_CARD).collect();
                d.leader = r.leader;
            }
            (DeckRef::Custom { name }, _) => {
                if let Some(c) = self.custom.find(name).map(|i| &self.custom.decks[i]) {
                    d.cards = c.cards.clone();
                    d.leader = c.leader;
                    d.leader_rank = c.leader_rank;
                }
            }
            _ => {}
        }
        d
    }

    /// A new custom deck with an entry's cards, leader and leader rank,
    /// named "NAME (copy)" (made unique); its index (`None`: no room, or the
    /// custom decks file could not be read). The entry's own deck (a save's,
    /// a computer opponent's or a custom one) is left as it is.
    pub fn fork(&mut self, entry: &DeckEntry) -> Option<usize> {
        if self.custom_error.is_some() {
            return None;
        }
        let d = self.deck_of(entry);
        let mut copy = CustomDeck::from_submission(&d.name, &DeckSubmission { leader: d.leader, leader_rank: d.leader_rank, cards: d.cards });
        copy.name = self.custom.copy_name(&entry.name);
        self.custom.add(copy)
    }
}

/// The first of Custom Duel's VS maps in the board table (0x16; the
/// campaign's boards are 0..0x16).
pub const VS_FIRST: usize = 0x16;

/// A map choice's name on the screens.
pub fn map_label(map: &MapChoice) -> String {
    match map {
        MapChoice::Random => "RANDOM".into(),
        MapChoice::Game { index } if usize::from(*index) >= VS_FIRST => format!("VS MAP {:02}", usize::from(*index) - VS_FIRST + 1),
        MapChoice::Game { index } => format!("CAMPAIGN MAP {:02}", index + 1),
        MapChoice::Own { name, .. } => name.clone(),
    }
}

/// The deck view's sort keys, the deck editor's own comparators
/// (`deck_editor::compare_key`; NEWCOMER and RANK sorts read a save's
/// recent list and ranks, which a previewed deck does not have): card
/// number, name, ATK, DEF, type, attribute, level, deck cost, kind.
pub const SORT_KEYS: [u8; 9] = [10, 1, 2, 3, 4, 5, 6, 7, 9];

/// A sort key's label (gold font: letters and spaces).
pub fn sort_label(key: u8) -> &'static str {
    match key {
        1 => "NAME SORT",
        2 => "ATK SORT",
        3 => "DEF SORT",
        4 => "TYPE SORT",
        5 => "ATTRIBUTE SORT",
        6 => "LEVEL SORT",
        7 => "COST SORT",
        9 => "KIND SORT",
        _ => "NUMBER SORT",
    }
}

/// Rows the deck view shows at once (the deck editor's six).
pub const PREVIEW_ROWS: usize = deck_editor::VISIBLE_ROWS as usize;
/// The detail pane's pages (data, text, power-ups, abilities).
pub const DETAIL_PAGES: u8 = 4;

/// One deck looked at card by card: row 0 is the leader, then one row per
/// card of the deck with its copies, in the chosen sort.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeckPreview {
    pub entry: DeckEntry,
    /// The deck as it stands.
    pub deck: CustomDeck,
    /// Each card once, with its copies, sorted.
    pub groups: Vec<(u16, u8)>,
    /// Index into [`SORT_KEYS`].
    pub sort: usize,
    /// The highlighted row (0 = the leader).
    pub cursor: usize,
    /// The first row shown.
    pub first: usize,
    /// The detail pane's page while it is open (Triangle).
    pub detail: Option<u8>,
}

impl DeckPreview {
    pub fn new(entry: DeckEntry, lib: &DeckLibrary) -> Self {
        let deck = lib.deck_of(&entry);
        let mut p = Self { entry, deck, groups: Vec::new(), sort: 0, cursor: 0, first: 0, detail: None };
        p.regroup(lib.cards.as_ref());
        p
    }

    fn regroup(&mut self, cards: Option<&StaticCards>) {
        let mut ids: Vec<u16> = self.deck.cards.clone();
        ids.sort_unstable();
        ids.dedup();
        if let Some(cards) = cards {
            let block = CardBlock::empty(0);
            let keys = [SORT_KEYS[self.sort], 4, 10];
            ids.sort_by(|&a, &b| deck_editor::compare_keys(&keys, &block, cards, a, b));
        }
        self.groups = ids
            .into_iter()
            .map(|c| (c, self.deck.cards.iter().filter(|&&k| k == c).count().min(usize::from(u8::MAX)) as u8))
            .collect();
    }

    pub fn sort_key(&self) -> u8 {
        SORT_KEYS[self.sort]
    }

    /// Rows: the leader, then each card.
    pub fn len(&self) -> usize {
        1 + self.groups.len()
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    /// The card of row `i` (row 0: the leader, 999 when none).
    pub fn card_at(&self, i: usize) -> Option<u16> {
        match i {
            0 => Some(self.deck.leader),
            i => self.groups.get(i - 1).map(|g| g.0),
        }
    }

    /// The highlighted card (`None`: the leader row of a deck with none).
    pub fn highlighted(&self) -> Option<u16> {
        self.card_at(self.cursor).filter(|&c| c != EMPTY_CARD)
    }

    /// Move the cursor; false at an end.
    pub fn step(&mut self, delta: isize) -> bool {
        let to = self.cursor as isize + delta;
        if to < 0 || to as usize >= self.len() {
            return false;
        }
        self.cursor = to as usize;
        if self.cursor < self.first {
            self.first = self.cursor;
        } else if self.cursor >= self.first + PREVIEW_ROWS {
            self.first = self.cursor + 1 - PREVIEW_ROWS;
        }
        true
    }

    /// The next (or previous) sort key; the highlighted card stays
    /// highlighted.
    pub fn step_sort(&mut self, delta: isize, cards: Option<&StaticCards>) {
        let n = SORT_KEYS.len() as isize;
        self.sort = (self.sort as isize + delta).rem_euclid(n) as usize;
        let keep = self.card_at(self.cursor);
        self.regroup(cards);
        if self.cursor > 0 {
            if let Some(i) = keep.and_then(|c| self.groups.iter().position(|g| g.0 == c)) {
                self.cursor = i + 1;
            }
        }
        self.first = self.first.min(self.cursor).max((self.cursor + 1).saturating_sub(PREVIEW_ROWS));
    }

    /// Triangle: the detail pane opens on the highlighted card, or closes.
    pub fn toggle_detail(&mut self) -> bool {
        if self.detail.is_some() {
            self.detail = None;
            return true;
        }
        if self.highlighted().is_none() {
            return false;
        }
        self.detail = Some(0);
        true
    }

    /// The detail pane's next (or previous) page.
    pub fn step_page(&mut self, delta: i8) {
        if let Some(p) = self.detail.as_mut() {
            *p = (*p as i8 + delta).rem_euclid(DETAIL_PAGES as i8) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use roses_net::custom_decks::CustomDeck;

    fn opponents() -> Vec<OpponentDeck> {
        let deck = DeckSubmission { leader: 61, leader_rank: 9, cards: (100..140).collect() };
        vec![OpponentDeck { opponent: 9, record: 0x23, name: "Pegasus Crawford".into(), deck }]
    }

    #[test]
    fn every_source_is_listed_and_resolved() {
        let dir = std::env::temp_dir().join(format!("roses-online-decks-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(roses_net::custom_decks::CUSTOM_DECKS_FILE);
        let starter = DeckSubmission { leader: 5, leader_rank: 1, cards: (0..40).collect() };
        let mut lib = DeckLibrary::new(None, Some(starter.clone()), opponents(), &path, vec![], vec![]);
        // No save: the starter deck, then the opponents.
        let e = lib.entries();
        assert_eq!(e.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["STARTER DECK", "Pegasus Crawford"]);
        assert_eq!(e[1].deck.as_ref().unwrap().leader_rank, 9);
        assert_eq!(lib.entry(&DeckRef::SaveCurrent).unwrap().deck, Ok(starter));
        assert_eq!(lib.entry(&DeckRef::Save { deck: 1 }), None);
        // Copy Pegasus's deck as a custom deck, then an unfinished one.
        let i = lib.fork(&e[1]).unwrap();
        lib.custom.add(CustomDeck::new("Half"));
        lib.save_custom().unwrap();
        let lib = DeckLibrary::new(None, None, opponents(), &path, vec![], vec![]);
        assert_eq!(lib.custom.decks[i].name, "Pegasus Crawford (copy)");
        let mine = lib.entry(&DeckRef::Custom { name: "pegasus crawford (copy)".into() }).unwrap();
        assert_eq!(mine.source, Source::Custom);
        assert_eq!(mine.deck, e[1].deck);
        let half = lib.entry(&DeckRef::Custom { name: "Half".into() }).unwrap();
        assert_eq!(half.deck, Err("0 of 40 cards".into()));
        // The remembered choice.
        let choice = dir.join(CHOICE_FILE);
        assert_eq!(load_choice(&choice), None);
        save_choice(&choice, &DeckRef::Opponent { opponent: 9 }).unwrap();
        assert_eq!(load_choice(&choice), Some(DeckRef::Opponent { opponent: 9 }));
        // An unreadable custom file is never overwritten.
        std::fs::write(&path, b"{ not json").unwrap();
        let lib = DeckLibrary::new(None, None, opponents(), &path, vec![], vec![]);
        assert!(lib.custom_error.is_some());
        assert!(lib.save_custom().is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"{ not json");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_saves_three_decks_and_its_current_one() {
        use roses_core::deck_editor::StaticCards;
        let records = (0..roses_core::CARD_COUNT).map(|_| [0u8; deck_editor::RECORD_BYTES]).collect();
        let cards = StaticCards { records, name_order: (0..roses_core::CARD_COUNT).collect(), password_key: [0; 8] };
        let deck = CustomDeck { name: "x".into(), leader: 7, leader_rank: 3, cards: (200..240).collect() };
        let mut block = roses_net::custom_decks::editor_block(&deck, &cards);
        deck_editor::set_current_deck(&mut block, 0);
        let lib = DeckLibrary::new(Some(block), None, vec![], &std::env::temp_dir().join("roses-none").join("x.json"), vec![], vec![]);
        let e = lib.entries();
        assert_eq!(e.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["YOUR DECK (1)", "SAVE DECK 1", "SAVE DECK 2", "SAVE DECK 3"]);
        assert_eq!(e[0].deck.as_ref().unwrap(), &DeckSubmission { leader: 7, leader_rank: 3, cards: (200..240).collect() });
        assert_eq!(e[2].deck, Err("0 of 40 cards".into()));
    }

    /// The deck view of a computer opponent's deck: the leader row, each
    /// card once with its copies, the editor's sort keys (the highlighted
    /// card kept), the detail pane's pages; COPY TO MY DECKS makes
    /// "Pegasus Crawford (copy)" with its cards, leader and rank and leaves
    /// the opponent's deck as it was.
    #[test]
    fn a_deck_is_previewed_sorted_and_forked_unchanged() {
        use roses_core::CARD_COUNT;
        let mut records = vec![[0u8; deck_editor::RECORD_BYTES]; usize::from(CARD_COUNT)];
        for (c, r) in records.iter_mut().enumerate() {
            r[8..10].copy_from_slice(&((c as u16 * 7) % 3000).to_le_bytes());
        }
        let cards = StaticCards { records, name_order: (0..CARD_COUNT).rev().collect(), password_key: [0; 8] };
        let mut list: Vec<u16> = (100..113).flat_map(|c| [c, c, c]).collect();
        list.push(140);
        let deck = DeckSubmission { leader: 61, leader_rank: 9, cards: list.clone() };
        let pegasus = vec![OpponentDeck { opponent: 9, record: 0x23, name: "Pegasus Crawford".into(), deck: deck.clone() }];
        let dir = std::env::temp_dir().join(format!("roses-online-preview-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join(roses_net::custom_decks::CUSTOM_DECKS_FILE);
        let mut lib = DeckLibrary::new(None, None, pegasus.clone(), &path, vec![], vec![]).with_cards(cards.clone());
        let entry = lib.entries().into_iter().find(|e| e.source == Source::Opponent).unwrap();
        let mut p = DeckPreview::new(entry.clone(), &lib);
        assert_eq!((p.len(), p.card_at(0), p.groups[0], p.groups[13]), (15, Some(61), (100, 3), (140, 1)));
        assert_eq!(p.groups.iter().map(|g| usize::from(g.1)).sum::<usize>(), 40);
        assert!(!p.step(-1));
        for _ in 0..14 {
            assert!(p.step(1));
        }
        assert!(!p.step(1));
        assert_eq!((p.cursor, p.first, p.highlighted()), (14, 9, Some(140)));
        // NAME sort (the name order reversed here): 140 first, still highlighted.
        p.step_sort(1, lib.cards.as_ref());
        assert_eq!((sort_label(p.sort_key()), p.groups[0].0, p.cursor, p.first), ("NAME SORT", 140, 1, 1));
        // ATK sort: descending.
        p.step_sort(1, lib.cards.as_ref());
        let atk = |c: u16| (c * 7) % 3000;
        assert!(p.groups.windows(2).all(|w| atk(w[0].0) >= atk(w[1].0)), "{:?}", p.groups);
        p.step_sort(-2, lib.cards.as_ref());
        assert_eq!(p.sort_key(), 10);
        // The detail pane: open on the card, pages wrap, closes.
        assert!(p.toggle_detail());
        assert_eq!(p.detail, Some(0));
        p.step_page(-1);
        assert_eq!(p.detail, Some(3));
        assert!(p.toggle_detail());
        assert_eq!(p.detail, None);
        // COPY TO MY DECKS.
        let i = lib.fork(&entry).unwrap();
        let copy = &lib.custom.decks[i];
        assert_eq!((copy.name.as_str(), copy.leader, copy.leader_rank, &copy.cards), ("Pegasus Crawford (copy)", 61, 9, &list));
        assert_eq!(copy.submission().unwrap(), deck);
        assert_eq!(lib.fork(&entry).map(|i| lib.custom.decks[i].name.clone()).as_deref(), Some("Pegasus Crawfor (copy 2)"));
        assert_eq!(lib.opponents, pegasus, "the opponent's deck is unchanged");
        assert_eq!(lib.entries().into_iter().find(|e| e.source == Source::Opponent).unwrap(), entry);
        // A deck without a leader: its leader row has no card to detail.
        lib.custom.add(CustomDeck::new("Half"));
        let half = lib.entry(&DeckRef::Custom { name: "Half".into() }).unwrap();
        let mut p = DeckPreview::new(half, &lib);
        assert_eq!((p.len(), p.highlighted()), (1, None));
        assert!(!p.toggle_detail());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
