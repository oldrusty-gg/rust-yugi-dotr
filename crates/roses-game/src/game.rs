//! The connected game (`--game <game.json>`): one import folder made by
//! `roses import-game`, and the hand-offs between its parts without any
//! renderer: the story's last stand-in leads to the campaign map, the map's
//! duel request at Chester starts the first duel with the starter deck chosen
//! in the story, and the duel's end returns to the map.
use crate::duel_play::{FirstDuel, SideDeck};
use roses_assets::{DeckRecord, TerrainMap};
use roses_client::StarterDeckChoice;
use roses_client::campaign_map::DuelRequest;
use roses_core::card_block::CardBlock;
use roses_core::deck_editor::{self, StaticCards};
use roses_core::duel::{RANK_COUNT, queries};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const GAME_SCHEMA: &str = "roses-game-import-1";
/// The story stage whose end is the campaign map's event 0 ("Choose a
/// destination"); the story-to-map import stops there.
pub const LAST_STORY_STAGE: &str = "map-events-101-102";

#[derive(Deserialize)]
struct PartRef {
    path: String,
    manifest: String,
    manifest_sha256: String,
}

#[derive(Deserialize)]
struct GameFile {
    schema_version: String,
    story: PartRef,
    campaign_map: PartRef,
    duel: PartRef,
    duel_board: PartRef,
    duel_models: PartRef,
    /// The result screens' and slot machine's textures (`roses import-duel-end`);
    /// absent in imports made before that importer joined `import-game`.
    #[serde(default)]
    duel_end: Option<PartRef>,
    /// Simon's scene and the history clouds from the disc; absent in older
    /// imports (the game then needs the private research captures).
    #[serde(default)]
    opening_scene: Option<PartRef>,
    #[serde(default)]
    clouds: Option<PartRef>,
    /// The Stonehenge story events' world, MODEL.MRG pack 33
    /// (`roses import-opening-scene --world story-scene-0`).
    #[serde(default)]
    story_scene: Option<PartRef>,
    /// The battle arenas (`roses import-duel-arena`); absent in older imports
    /// (the battle scene then goes from the versus cards to the return).
    #[serde(default)]
    duel_arena: Option<PartRef>,
    /// The duel's music and sounds (`roses import-duel-audio`).
    #[serde(default)]
    duel_audio: Option<PartRef>,
    limitations: Vec<String>,
}

/// Paths of every part, each manifest checked against the hash recorded when
/// the import finished (a mixed or edited folder is refused).
#[derive(Clone, Debug)]
pub struct GameImport {
    pub root: PathBuf,
    pub story_manifest: PathBuf,
    pub campaign_map: PathBuf,
    pub duel: PathBuf,
    pub duel_board_manifest: PathBuf,
    pub duel_models_manifest: PathBuf,
    /// The duel-end import folder (TEXETC.MRG, PICTURE.MRG), when the game
    /// import has one; without it the result screens stop at an explicit
    /// boundary.
    pub duel_end: Option<PathBuf>,
    /// `opening-scene/scene.json` (textures relative to its folder).
    pub opening_scene: Option<PathBuf>,
    /// `clouds/source.json` (files relative to its folder).
    pub clouds: Option<PathBuf>,
    /// `story-scene/scene.json`: the 3D world behind events 11160-11184.
    pub story_scene: Option<PathBuf>,
    /// `duel-arena/manifest.json`.
    pub duel_arena: Option<PathBuf>,
    /// `duel-audio/manifest.json`.
    pub duel_audio: Option<PathBuf>,
    pub limitations: Vec<String>,
}

fn part(root: &Path, part: &PartRef) -> Result<PathBuf, String> {
    let relative = Path::new(&part.path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
        || part.manifest.contains(['/', '\\'])
    {
        return Err(format!(
            "game.json names a part outside its folder: {}",
            part.path
        ));
    }
    let manifest = root.join(relative).join(&part.manifest);
    let bytes = crate::bounded_read(&manifest, 16 * 1024 * 1024)?;
    if format!("{:x}", Sha256::digest(&bytes)) != part.manifest_sha256 {
        return Err(format!(
            "{} changed since the game import finished",
            manifest.display()
        ));
    }
    Ok(manifest)
}

impl GameImport {
    pub fn open(game_json: &Path) -> Result<Self, String> {
        let root = game_json
            .parent()
            .ok_or("game.json has no folder")?
            .to_path_buf();
        let file: GameFile = serde_json::from_slice(&crate::bounded_read(game_json, 1024 * 1024)?)
            .map_err(|e| format!("{}: {e}", game_json.display()))?;
        if file.schema_version != GAME_SCHEMA {
            return Err(format!(
                "unsupported game import {} (expected {GAME_SCHEMA})",
                file.schema_version
            ));
        }
        let folder = |p: &PathBuf| p.parent().map(Path::to_path_buf).unwrap_or_default();
        Ok(Self {
            story_manifest: part(&root, &file.story)?,
            campaign_map: folder(&part(&root, &file.campaign_map)?),
            duel: folder(&part(&root, &file.duel)?),
            duel_board_manifest: part(&root, &file.duel_board)?,
            duel_models_manifest: part(&root, &file.duel_models)?,
            duel_end: match &file.duel_end {
                Some(p) => Some(folder(&part(&root, p)?)),
                None => None,
            },
            opening_scene: file
                .opening_scene
                .as_ref()
                .map(|p| part(&root, p))
                .transpose()?,
            clouds: file.clouds.as_ref().map(|p| part(&root, p)).transpose()?,
            story_scene: file
                .story_scene
                .as_ref()
                .map(|p| part(&root, p))
                .transpose()?,
            duel_arena: file.duel_arena.as_ref().map(|p| part(&root, p)).transpose()?,
            duel_audio: file.duel_audio.as_ref().map(|p| part(&root, p)).transpose()?,
            limitations: file.limitations,
            root,
        })
    }
}

/// Private research captures still used for the opening (Simon's scene and the
/// history cloud checkpoint) under a development workspace, when present.
#[derive(Clone, Debug, Default)]
pub struct ResearchInputs {
    pub data_root: Option<PathBuf>,
    pub scene: Option<PathBuf>,
    pub cloud_source: Option<PathBuf>,
}

impl ResearchInputs {
    pub fn from_root(root: &Path) -> Self {
        let story = root.join("work").join("research").join("story");
        let scene = story.join("mesh-scene-simon.json");
        let clouds = story
            .join("clouds-rust-oracle")
            .join("source-counter437-20260929.json");
        Self {
            data_root: Some(root.to_path_buf()),
            scene: scene.is_file().then_some(scene),
            cloud_source: clouds.is_file().then_some(clouds),
        }
    }
}

/// Deck record of a campaign opponent: 0x1A + opponent id (Weevil 2 ->
/// 0x1C, Pegasus 9 -> 0x23; sizing/opponents table, checked against the
/// first-duel import).
pub const OPPONENT_DECK_BASE: usize = 0x1A;
/// Opponents whose computer controller is ported. The opponent id is the
/// side's control byte; use the core list so campaign gating stays in sync.
pub const PORTED_OPPONENTS: &[u8] = &roses_core::duel::ai::weevil::PORTED_CONTROLLERS;
/// A duel request's identity of the player's side (0x232E40: 0x44C) and
/// the base of the opponent's (1000 + C4) [F].
pub const DUEL_PARAM_PLAYER: i32 = roses_client::campaign_map::DUEL_PARAM_MODE;
pub const DUEL_PARAM_OPPONENT_BASE: i32 = roses_client::campaign_map::DUEL_PARAM_BASE;

/// Ability table 0x2C26B8: one row of 20 u16 per card, copied to a leader
/// record at +0x08 (ABILITY-2.md section 1); ELF offset = EE − 0xF9680.
const ABILITY_TABLE_EE: usize = 0x2C_26B8;
const EE_TO_FILE: usize = 0xF_9680;

/// Leader abilities of `card` at `rank`, from its row in the user's
/// executable (the duel import's hash-checked copy).
pub fn leader_abilities_at(executable: &[u8], card: u16, rank: i32) -> Result<Vec<u8>, String> {
    let at = ABILITY_TABLE_EE - EE_TO_FILE + usize::from(card) * 40;
    let bytes = executable
        .get(at..at + 40)
        .filter(|_| card < 0x356)
        .ok_or_else(|| format!("card {card} has no ability row"))?;
    let mut row = [0_u16; 20];
    for (i, h) in row.iter_mut().enumerate() {
        *h = u16::from_le_bytes([bytes[2 * i], bytes[2 * i + 1]]);
    }
    Ok(queries::leader_abilities(&row, rank))
}

/// The player's side of a campaign duel: the current deck of the player's
/// card block (deck record `BLK+0xFD94+d*0x54`, read by the duel setup
/// 0x214120 through 0x2151F0/0x215160) and the leader's stored rank.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerDeck {
    /// The static deck record the story chose (kept for logging and the
    /// first-duel checks; the cards come from the block).
    pub deck_record: usize,
    pub leader: u16,
    /// The 40 cards in deck-record order.
    pub cards: Vec<u16>,
    /// The leader's stored rank (`CR+0x48 & 0x3FFF`), the rank the ability
    /// table 0x2C26B8 is read at.
    pub leader_rank: i32,
}

impl PlayerDeck {
    /// The story's starter deck as the loader 0x214120 writes it (table
    /// order), ranked by the static record: what a block made from it
    /// also yields.
    pub fn from_starter(chosen: &StarterDeckChoice, decks: &[DeckRecord]) -> Self {
        Self {
            deck_record: usize::from(chosen.record),
            leader: chosen.leader,
            cards: chosen.cards.clone(),
            leader_rank: decks
                .get(usize::from(chosen.record))
                .map_or(1, |d| i32::from(d.leader_rank_raw)),
        }
    }

    /// The block's current deck. Refused when it is not a valid deck (40
    /// cards and a leader, 0x1BB0A0): the editor does not let the player
    /// leave with one, so this only guards a corrupted block.
    pub fn from_block(block: &CardBlock, deck_record: usize) -> Result<Self, String> {
        let d = deck_editor::current_deck(block);
        let record = deck_editor::deck_record(block, d);
        if !record.is_valid() {
            return Err(format!(
                "the player's deck {d} is not valid ({} cards, leader {})",
                record.count(),
                record.leader
            ));
        }
        Ok(Self {
            deck_record,
            leader: record.leader,
            cards: record
                .cards
                .iter()
                .copied()
                .filter(|&c| c != roses_core::card_block::EMPTY_CARD)
                .collect(),
            leader_rank: block.stored_rank(record.leader).map_or(0, i32::from),
        })
    }
}

/// First static deck record of the campaign opponents: the hover panel
/// loads record 0x1A + C4 (C4 = speaker − 197) into side 1 (0x235824).
pub const OPPONENT_DECK_RECORD_BASE: usize = 0x1A;

/// The map's DC check inputs: the player's current deck cost (W+5444 =
/// 0x215690(0, current deck), the stored +0x52) and, per opponent id, the
/// cost of its static deck record (W+5446 = 0x215690(1, 0) after
/// 0x214120(1, 0x1A + C4); the sum of card byte 3 as 0x2153D0 computes it).
pub fn map_deck_costs(
    block: &CardBlock,
    decks: &[DeckRecord],
    executable: &[u8],
) -> Result<(u16, std::collections::BTreeMap<u8, u16>), String> {
    let cards = StaticCards::from_executable(executable, 0)
        .ok_or("the executable is too short for the card records")?;
    let player = deck_editor::deck_record(block, deck_editor::current_deck(block)).cost;
    let opponents = decks
        .iter()
        .enumerate()
        .skip(OPPONENT_DECK_RECORD_BASE)
        .filter_map(|(record, deck)| {
            let opponent = u8::try_from(record - OPPONENT_DECK_RECORD_BASE).ok()?;
            let cost = deck
                .cards
                .iter()
                .filter_map(|&c| cards.cost(c))
                .fold(0u16, |acc, c| acc.wrapping_add(u16::from(c)));
            Some((opponent, cost))
        })
        .collect();
    Ok((player, opponents))
}

/// The player's card block of a new game, made when the story chooses the
/// starter deck: one instance per deck card plus the leader (ranked at the
/// static record's rank, `duel_end_screens::new_game_block`, an assumed
/// layout), laid out as deck 0 of the block (`lay_out_new_game_deck`, the
/// layout the editor's traced readers expect). `rng` is the shared LCG
/// (the owner key and the instance ids are drawn from it).
pub fn new_player_block(
    chosen: &StarterDeckChoice,
    decks: &[DeckRecord],
    executable: &[u8],
    thresholds: &[u16; RANK_COUNT],
    rng: &mut u64,
) -> Result<CardBlock, String> {
    let cards = StaticCards::from_executable(executable, 0)
        .ok_or("the executable is too short for the card records")?;
    let rank = decks
        .get(usize::from(chosen.record))
        .map_or(1, |d| u16::from(d.leader_rank_raw));
    let key = roses_core::duel::next_rand31(rng) | 1;
    let mut block = crate::duel_end_screens::new_game_block(
        key,
        &chosen.cards,
        chosen.leader,
        rank,
        thresholds,
        rng,
    );
    deck_editor::lay_out_new_game_deck(&mut block, &cards, 0, &chosen.cards, chosen.leader)?;
    // 0x213DB0: a new block's options are all on (+0xFF9C bits 2..4).
    roses_client::game_options::set_bits(&mut block, roses_client::game_options::DEFAULT_BITS);
    // 0x213A20 (boot) fills the map editor's five own maps with terrain 7.
    block.reset_own_maps();
    Ok(block)
}

/// A campaign duel for a map duel request (0x232E40 parameters: board = C4,
/// first = SAVE[0], 1100, 1000 + opponent): the player's side is `player`
/// (the card block's current deck), the opponent's side its static deck
/// record, the board the terrain map `board`. Each leader's abilities are
/// those of its rank (the player's stored rank, the opponent's record rank).
pub fn campaign_duel(
    request: &DuelRequest,
    template: &FirstDuel,
    player: &PlayerDeck,
    decks: &[DeckRecord],
    maps: &[TerrainMap],
    executable: &[u8],
) -> Result<FirstDuel, String> {
    // p2 = SAVE[0], the side byte: the side that moves first; p3/p4 are
    // side 0's and side 1's identities (1100: the pad, control 100; 1000 +
    // C4: the opponent's deck record and control byte, shuffled) [F
    // 0x232E40, FLOW.md C12/C13]. The White Rose route (p2 = 1) seats the
    // player on side 1 and the opponent on side 0.
    let player_side = match request.params[1] {
        0 => 0usize,
        1 => 1,
        other => return Err(format!("Duel parameter 2 = {other} is not a side.")),
    };
    let opponent_identity = DUEL_PARAM_OPPONENT_BASE + i32::from(request.opponent);
    let mut identities = [opponent_identity; 2];
    identities[player_side] = DUEL_PARAM_PLAYER;
    if [request.params[2], request.params[3]] != identities {
        return Err(format!(
            "Duel parameters 3/4 = {}/{} do not seat the player on side {player_side} against opponent {}.",
            request.params[2], request.params[3], request.opponent
        ));
    }
    let board = usize::try_from(request.params[0])
        .ok()
        .and_then(|b| maps.get(b))
        .ok_or_else(|| format!("board {} is not a terrain map", request.params[0]))?;
    let record = decks
        .get(OPPONENT_DECK_BASE + usize::from(request.opponent))
        .ok_or_else(|| format!("opponent {} has no deck record", request.opponent))?;
    let mut duel = template.clone();
    duel.board_map = board.index.into();
    duel.terrain = board.cells.clone();
    duel.first = player_side as u8;
    let mut sides = [
        SideDeck {
            deck_record: player.deck_record,
            leader: player.leader,
            cards: player.cards.clone(),
        },
        SideDeck {
            deck_record: usize::from(record.index),
            leader: record.leader_card,
            cards: record.cards.clone(),
        },
    ];
    let mut abilities = [
        leader_abilities_at(executable, player.leader, player.leader_rank)?,
        leader_abilities_at(
            executable,
            record.leader_card,
            i32::from(record.leader_rank_raw),
        )?,
    ];
    let mut ranks = [player.leader_rank.clamp(0, 12) as u8, record.leader_rank_raw.min(12)];
    if player_side == 1 {
        sides.swap(0, 1);
        abilities.swap(0, 1);
        ranks.swap(0, 1);
    }
    duel.sides = sides;
    duel.leader_abilities = abilities;
    duel.leader_ranks = ranks;
    duel.opponent = request.opponent;
    duel.player_side = player_side as u8;
    duel.computer_sides = vec![1 - player_side as u8];
    Ok(duel)
}

/// The duel a map request starts, refused explicitly while the opponent's
/// computer controller is not ported.
pub fn duel_for_request(
    request: &DuelRequest,
    template: &FirstDuel,
    player: &PlayerDeck,
    decks: &[DeckRecord],
    maps: &[TerrainMap],
    executable: &[u8],
) -> Result<FirstDuel, String> {
    let duel = campaign_duel(request, template, player, decks, maps, executable)?;
    let opponent_side = usize::from(1 - duel.player_side);
    if !PORTED_OPPONENTS.contains(&request.opponent) {
        return Err(format!(
            "The duel at node {} (opponent {}, leader {}) needs that opponent's computer player, which is not ported yet; the ported controllers are {:?}.",
            request.node, request.opponent, duel.sides[opponent_side].leader, PORTED_OPPONENTS
        ));
    }
    Ok(duel)
}

/// Side 1 of a Custom Duel against the computer: the mirror's block (a copy
/// of the player's, selector 1) or a static deck record (selectors 2..4,
/// 0x214120(1, record)).
#[derive(Clone, Debug)]
pub enum CustomOpponent {
    Mirror(CardBlock),
    Record(usize),
    /// Selector 0: a second human (control 101, pad 2) with MEMORY CARD
    /// slot 2's block.
    Second(CardBlock),
}

/// An own map of the map editor as a duel board: maps 0x2E..0x32 are side
/// 0's card block's own maps 0..4, 0x33..0x37 side 1's (0x1DF0F0 reads
/// 0x213D70(side) + 0xFEA3 + 49·i) [F]. None for the executable's maps.
pub fn own_map_cells(
    map: u8,
    player_block: &CardBlock,
    opponent: &CustomOpponent,
) -> Option<Result<[u8; roses_core::card_block::OWN_MAP_CELLS], String>> {
    let base = roses_client::custom_duel::OWN_MAP_BASE;
    let count = roses_core::card_block::OWN_MAP_COUNT as u8;
    if !(base..base + 2 * count).contains(&map) {
        return None;
    }
    let (side, i) = ((map - base) / count, usize::from((map - base) % count));
    let block = match (side, opponent) {
        (0, _) => player_block,
        (_, CustomOpponent::Mirror(b) | CustomOpponent::Second(b)) => b,
        (_, CustomOpponent::Record(r)) => {
            return Some(Err(format!("Custom Duel map {map:#x}: side 1 (deck record {r:#x}) has no card block")));
        }
    };
    Some(block.own_map(i).ok_or_else(|| format!("Custom Duel map {map:#x}: own map {i} is missing")))
}

/// The duel a Custom Duel start opens (mode 9 → mode 1, params 1..4, 0x1C):
/// board = the menu's map, side 0 = the player's current deck (pad 100,
/// moving first, param[2] = 0), side 1 = the computer (control 22..25)
/// with the mirror's current deck or the static record; the leaders'
/// abilities at their ranks unless the leader-ability switch of that side
/// is off (param[0x1C] gates every query, 0x1E1F00); LP and SP recharge
/// from the duel words the menu set.
pub fn custom_duel(
    start: &roses_client::custom_duel::DuelStart,
    template: &FirstDuel,
    player: &PlayerDeck,
    player_block: &CardBlock,
    opponent: &CustomOpponent,
    decks: &[DeckRecord],
    maps: &[TerrainMap],
    executable: &[u8],
) -> Result<FirstDuel, String> {
    let control = start.sides[1].control;
    let human = control >= roses_core::duel::PAD_CONTROLLER;
    if !human && !roses_core::duel::ai::weevil::is_ported(control) {
        return Err(format!("Custom Duel's opponent control {control} has no computer player"));
    }
    let (board_index, board_cells) = match own_map_cells(start.map, player_block, opponent) {
        Some(own) => (usize::from(start.map), own?.to_vec()),
        None => {
            let board = maps
                .get(usize::from(start.map))
                .ok_or_else(|| format!("Custom Duel map {} is not a terrain map", start.map))?;
            (usize::from(board.index), board.cells.clone())
        }
    };
    let cpu = match opponent {
        CustomOpponent::Mirror(block) => PlayerDeck::from_block(block, player.deck_record)?,
        CustomOpponent::Second(block) => PlayerDeck::from_block(block, player.deck_record)
            .map_err(|e| format!("player 2: {e}"))?,
        CustomOpponent::Record(record) => {
            let r = decks.get(*record).ok_or_else(|| format!("deck record {record:#x} is missing"))?;
            PlayerDeck {
                deck_record: *record,
                leader: r.leader_card,
                cards: r.cards.clone(),
                leader_rank: i32::from(r.leader_rank_raw),
            }
        }
    };
    let abilities = |side: u8, d: &PlayerDeck| -> Result<Vec<u8>, String> {
        if start.leader_ability(side) {
            leader_abilities_at(executable, d.leader, d.leader_rank)
        } else {
            Ok(Vec::new())
        }
    };
    let mut duel = template.clone();
    duel.board_map = board_index;
    duel.terrain = board_cells;
    duel.first = start.first;
    duel.leader_abilities = [abilities(0, player)?, abilities(1, &cpu)?];
    duel.leader_ranks = [player.leader_rank.clamp(0, 12) as u8, cpu.leader_rank.clamp(0, 12) as u8];
    duel.sides = [
        SideDeck { deck_record: player.deck_record, leader: player.leader, cards: player.cards.clone() },
        SideDeck { deck_record: cpu.deck_record, leader: cpu.leader, cards: cpu.cards.clone() },
    ];
    duel.opponent = control;
    duel.player_side = 0;
    // Two humans: control 101 reads pad 2 (0x1CFF80) [F].
    duel.computer_sides = if human { Vec::new() } else { vec![1] };
    duel.custom_rules = Some((start.sides.map(|s| s.lp), start.sides.map(|s| s.sp)));
    // params 0x1C / 0x1D: the leader-ability and card-display switches.
    duel.options = Some([u32::from(start.leader_abilities), u32::from(start.card_display)]);
    Ok(duel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use roses_client::{CampaignState, StarterDeckChoice};

    fn starter(record: u8, leader: u16) -> StarterDeckChoice {
        StarterDeckChoice {
            name_codes: vec![],
            checksum: 12,
            candidates: vec![9, 13, 16],
            selection: 0,
            record,
            leader,
            cards: vec![7; 40],
        }
    }

    fn campaign(record: u8, leader: u16) -> PlayerDeck {
        PlayerDeck::from_starter(&starter(record, leader), &[])
    }

    #[test]
    fn player_deck_from_starter_and_an_empty_block() {
        let no_deck = CampaignState::default();
        assert!(no_deck.starter_deck.is_none());
        let from_starter = PlayerDeck::from_starter(&starter(9, 392), &[]);
        assert_eq!(from_starter.leader_rank, 1);
        assert_eq!(from_starter.cards.len(), 40);
        // A block whose deck record is empty has no valid deck.
        let mut empty = CardBlock::empty(5);
        roses_core::deck_editor::write_deck_record(
            &mut empty,
            0,
            &roses_core::deck_editor::DeckRecord::EMPTY,
        );
        assert!(
            PlayerDeck::from_block(&empty, 9)
                .unwrap_err()
                .contains("not valid")
        );
    }

    fn chester() -> DuelRequest {
        DuelRequest {
            node: 2,
            opponent: 2,
            params: [2, 0, 1100, 1002],
        }
    }

    #[test]
    fn missing_deck_yorkists_and_unknown_boards_are_refused() {
        let template = FirstDuel {
            board_map: 2,
            terrain: vec![0; 49],
            first: 0,
            sides: [
                SideDeck {
                    deck_record: 9,
                    leader: 392,
                    cards: vec![],
                },
                SideDeck {
                    deck_record: 28,
                    leader: 397,
                    cards: vec![],
                },
            ],
            computer_sides: vec![1],
            planner_tier: 0,
            leader_abilities: [vec![], vec![]],
            opponent: 2,
            leader_ranks: [0; 2],
            player_side: 0,
            custom_rules: None,
            options: None,
        };
        let cards: &[u8] = &[];
        // The White Rose identities are swapped (p3 = 1000 + C4, p4 =
        // 1100); the red order with p2 = 1 does not seat the player.
        let crossed = DuelRequest {
            params: [2, 1, 1100, 1002],
            ..chester()
        };
        assert!(
            duel_for_request(&crossed, &template, &campaign(9, 392), &[], &[], cards)
                .unwrap_err()
                .contains("do not seat the player on side 1")
        );
        assert!(
            duel_for_request(&DuelRequest { params: [2, 2, 1100, 1002], ..chester() }, &template, &campaign(9, 392), &[], &[], cards)
                .unwrap_err()
                .contains("not a side")
        );
        assert!(
            duel_for_request(&chester(), &template, &campaign(9, 392), &[], &[], cards)
                .unwrap_err()
                .contains("board 2")
        );
    }

    #[test]
    fn custom_duel_seats_a_second_human_and_refuses_unknown_controls_and_maps() {
        use roses_client::custom_duel::{DuelSide, DuelStart};
        let template = FirstDuel {
            board_map: 2,
            terrain: vec![0; 49],
            first: 0,
            sides: [
                SideDeck { deck_record: 9, leader: 392, cards: vec![] },
                SideDeck { deck_record: 28, leader: 397, cards: vec![] },
            ],
            computer_sides: vec![1],
            planner_tier: 0,
            leader_abilities: [vec![], vec![]],
            opponent: 2,
            leader_ranks: [0; 2],
            player_side: 0,
            custom_rules: None,
            options: None,
        };
        let side = |control| DuelSide { control, lp: 4000, sp: 3 };
        let mut start = DuelStart {
            map: 0x16,
            first: 0,
            identities: [1100, 1101],
            leader_abilities: 3,
            card_display: 3,
            sides: [side(100), side(101)],
        };
        let player = campaign(9, 392);
        let opp = CustomOpponent::Record(0x32);
        // Control 101 (pad 2) needs no computer player: the map is checked next.
        assert!(custom_duel(&start, &template, &player, &CardBlock::empty(0), &opp, &[], &[], &[]).unwrap_err().contains("not a terrain map"));
        start.sides[1] = side(99);
        assert!(custom_duel(&start, &template, &player, &CardBlock::empty(0), &opp, &[], &[], &[]).unwrap_err().contains("no computer player"));
        start.sides[1] = side(25);
        assert!(custom_duel(&start, &template, &player, &CardBlock::empty(0), &opp, &[], &[], &[]).unwrap_err().contains("not a terrain map"));
    }

    #[test]
    fn own_maps_come_from_each_sides_card_block() {
        let mut mine = CardBlock::empty(1);
        mine.reset_own_maps();
        let edited: [u8; 49] = std::array::from_fn(|i| (i % 10) as u8);
        mine.set_own_map(2, &edited);
        let mut theirs = CardBlock::empty(2);
        theirs.set_own_map(4, &[9; 49]);
        let mirror = CustomOpponent::Mirror(theirs);
        // 0x1DF0F0: 0x2E + i is side 0's map i, 0x33 + i side 1's.
        assert_eq!(own_map_cells(0x30, &mine, &mirror).unwrap().unwrap(), edited);
        assert_eq!(own_map_cells(0x2E, &mine, &mirror).unwrap().unwrap(), [7; 49]);
        assert_eq!(own_map_cells(0x37, &mine, &mirror).unwrap().unwrap(), [9; 49]);
        assert!(own_map_cells(0x2D, &mine, &mirror).is_none());
        assert!(own_map_cells(0x38, &mine, &mirror).is_none());
        assert!(own_map_cells(0x33, &mine, &CustomOpponent::Record(0x32)).unwrap().is_err());
    }

    /// Research run: computer duels against a campaign opponent
    /// (`ROSES_OPPONENT`, default 9 Pegasus at `ROSES_NODE` 3) over
    /// `ROSES_SEEDS` seeds (default 5); the presentation's unsupported
    /// reports go to stderr.
    #[test]
    #[ignore = "research run; requires ROSES_GAME_IMPORT"]
    fn print_campaign_opponent_duels() {
        let path = std::env::var_os("ROSES_GAME_IMPORT").expect("ROSES_GAME_IMPORT");
        let env = |k: &str, d: u64| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
        let (opponent, node, seeds) = (env("ROSES_OPPONENT", 9) as u8, env("ROSES_NODE", 3) as u8, env("ROSES_SEEDS", 5));
        let game = GameImport::open(Path::new(&path)).unwrap();
        let data = crate::duel_play::load(&game.duel).unwrap();
        let decks = roses_assets::parse_pal_decks(&data.executable).unwrap().decks;
        let maps = roses_assets::parse_pal_rules(&data.executable).unwrap().maps;
        let request = DuelRequest { node, opponent, params: [i32::from(opponent), 0, 1100, 1000 + i32::from(opponent)] };
        // The player's deck as the capture's `--opponent` builds it: the
        // duel import's side 0.
        let side = &data.first_duel.sides[0];
        let mut player = PlayerDeck {
            deck_record: side.deck_record,
            leader: side.leader,
            cards: side.cards.clone(),
            leader_rank: decks.get(side.deck_record).map_or(1, |d| i32::from(d.leader_rank_raw)),
        };
        // `ROSES_PLAYER_DECK_RECORD`: play another static deck record on the
        // player's side (progressed-deck robustness sweeps).
        if let Ok(record) = std::env::var("ROSES_PLAYER_DECK_RECORD") {
            let record: usize = record.parse().expect("ROSES_PLAYER_DECK_RECORD");
            let deck = &decks[record];
            player = PlayerDeck {
                deck_record: record,
                leader: deck.leader_card,
                cards: deck.cards.clone(),
                leader_rank: i32::from(deck.leader_rank_raw),
            };
        }
        let built = duel_for_request(&request, &data.first_duel, &player, &decks, &maps, &data.executable).unwrap();
        for seed in 1..=seeds {
            let mut d = crate::duel_play::load(&game.duel).unwrap();
            d.first_duel = built.clone();
            let mut play = crate::duel_play::DuelPlay::new(d, seed, true).unwrap();
            let mut was = false;
            let start = play.cursor();
            let mut camera = crate::duel_board::BoardCamera::snapped((start.column, start.row));
            for _ in 0..env("ROSES_UPDATES", 40_000) {
                if play.update(0).is_none() {
                    break;
                }
                let cursor = play.cursor();
                crate::duel_play::drive_camera(&mut camera, &mut play, cursor, &|_| None);
                camera.tick();
                if let (Some(m), Ok(at)) = (&play.monster_fx, std::env::var("ROSES_PRINT_FX")) {
                    if at.split(',').any(|a| a.parse() == Ok(play.frames)) {
                        let tris = m.tris([0.0; 3], [0.0; 3]);
                        let area: f32 = tris.iter().map(|t| {
                            let [a, b, c] = t.screen;
                            ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs() / 2.0 * f32::from(t.colour[0][3]) / 128.0
                        }).sum::<f32>() / (512.0 * 512.0);
                        eprintln!("update {}: {:?} e {:?} view {:?} light {} tris {} cover {area} cards {:?}", play.frames, m.effect, m.e(), m.view, m.light(), tris.len(), m.ritual_cards().iter().map(|(c, p, k)| (*c, p.position, k[3])).collect::<Vec<_>>());
                    }
                }
                if let Some(m) = &play.monster_fx {
                    if m.t == 1 {
                        eprintln!("opponent {opponent} seed {seed} update {}: monster fx {:?}", play.frames, m.effect);
                    }
                }
                let now = play.activation.is_some();
                if now && !was {
                    eprintln!("opponent {opponent} seed {seed} update {}: activation of card {}", play.frames, play.activation.as_ref().unwrap().card);
                }
                was = now;
                if play.session.duel.outcome.is_some() && play.intro.is_none() && play.activation.is_none() && play.monster_fx.is_none() {
                    break;
                }
            }
            eprintln!("opponent {opponent} seed {seed} after {} updates (round {}): {}", play.frames, play.session.duel.round, play.title());
        }
    }

    #[test]
    #[ignore = "Requires a private game import (ROSES_GAME_IMPORT = its game.json)"]
    fn private_game_import_opens_and_campaign_duels_match_the_first_duel() {
        let path = std::env::var_os("ROSES_GAME_IMPORT").expect("ROSES_GAME_IMPORT");
        let game = GameImport::open(Path::new(&path)).unwrap();
        let data = crate::duel_play::load(&game.duel).unwrap();
        let decks = roses_assets::parse_pal_decks(&data.executable)
            .unwrap()
            .decks;
        let maps = roses_assets::parse_pal_rules(&data.executable)
            .unwrap()
            .maps;
        assert!(game.story_manifest.is_file());
        assert!(game.campaign_map.join("manifest.json").is_file());
        // With the recorded player deck the general builder rebuilds the
        // imported first duel exactly (board 2, Weevil's record 0x1C).
        let imported = &data.first_duel;
        let built = duel_for_request(
            &chester(),
            imported,
            &campaign(9, 392),
            &decks,
            &maps,
            &data.executable,
        )
        .unwrap();
        assert_eq!(built.terrain, imported.terrain);
        assert_eq!(built.board_map, imported.board_map);
        assert_eq!(built.sides[1].leader, imported.sides[1].leader);
        assert_eq!(built.sides[1].cards, imported.sides[1].cards);
        assert_eq!(built.sides[1].deck_record, imported.sides[1].deck_record);
        assert_eq!(built.opponent, 2);
        // Airknight Parshath at rank 1 and Basic Insect at rank 2: only
        // abilities the first duel never consults (#0/#1 bookkeeping; #3's
        // special draw is armed for controllers 1 and 0x13 only).
        assert_eq!(built.leader_abilities, [vec![0], vec![1, 3]]);
        // The same duel built from a new player's card block (the deck
        // record the story chose, laid out as deck 0): identical player side.
        let chosen = StarterDeckChoice {
            name_codes: vec![],
            checksum: 0,
            candidates: vec![9],
            selection: 0,
            record: 9,
            leader: imported.sides[0].leader,
            cards: imported.sides[0].cards.clone(),
        };
        let thresholds = roses_assets::parse_pal_rank_thresholds(&data.executable).unwrap();
        let mut rng = 7u64;
        let block =
            new_player_block(&chosen, &decks, &data.executable, &thresholds, &mut rng).unwrap();
        let player = PlayerDeck::from_block(&block, 9).unwrap();
        assert_eq!(player, PlayerDeck::from_starter(&chosen, &decks));
        let from_block = duel_for_request(
            &chester(),
            imported,
            &player,
            &decks,
            &maps,
            &data.executable,
        )
        .unwrap();
        assert_eq!(from_block.sides[0].cards, imported.sides[0].cards);
        assert_eq!(from_block.sides[0].leader, imported.sides[0].leader);
        assert_eq!(from_block.leader_abilities, built.leader_abilities);
        assert_eq!(roses_core::deck_editor::deck_record(&block, 0).count(), 40);
        // The map's DC check at frame 652 of the capture: player W+5444 =
        // 822, Weevil's (C4 = 2, record 0x1C) W+5446 = 854 [emu, FLOW.md].
        let (player_cost, opponent_costs) =
            map_deck_costs(&block, &decks, &data.executable).unwrap();
        assert_eq!((player_cost, opponent_costs.get(&2)), (822, Some(&854)));
        // The comparison with the original's player block during this duel
        // is `private_capture_player_block_matches_the_new_game_layout`.
        // Pegasus (Lancashire, opponent 9): deck 0x23, Illusionist Faceless
        // Mage at rank 9 with abilities 0, 2, 3, 9, 10 (ABILITY-9-10.md).
        let lancashire = DuelRequest {
            node: 3,
            opponent: 9,
            params: [3, 0, 1100, 1009],
        };
        let pegasus = campaign_duel(
            &lancashire,
            imported,
            &campaign(9, 392),
            &decks,
            &maps,
            &data.executable,
        )
        .unwrap();
        assert_eq!(
            (pegasus.sides[1].deck_record, pegasus.sides[1].leader),
            (0x23, 61)
        );
        assert_eq!(pegasus.leader_abilities[1], vec![0, 2, 3, 9, 10]);
        // His controller (0x135B00) is ported: the request is accepted with
        // his control byte as the opponent.
        let accepted = duel_for_request(
            &lancashire,
            imported,
            &campaign(9, 392),
            &decks,
            &maps,
            &data.executable,
        )
        .unwrap();
        assert_eq!((accepted.opponent, accepted.computer_sides), (9, vec![1]));
        // Newcastle (opponent 8, the Labyrinth-ruler) is also ported.
        let newcastle = DuelRequest {
            node: 4,
            opponent: 8,
            params: [4, 0, 1100, 1008],
        };
        let accepted = duel_for_request(
            &newcastle,
            imported,
            &campaign(9, 392),
            &decks,
            &maps,
            &data.executable,
        )
        .unwrap();
        assert_eq!((accepted.opponent, accepted.computer_sides), (8, vec![1]));
        // A controller of 11..=21 that has a deck record but is not ported
        // yet (another port is adding them); its request must still stop
        // before a campaign duel starts.
        let missing = (11..=21u8).find(|o| !PORTED_OPPONENTS.contains(o)).unwrap_or(15);
        let unported = DuelRequest {
            opponent: missing,
            params: [4, 0, 1100, 1000 + i32::from(missing)],
            ..newcastle
        };
        assert!(
            duel_for_request(
                &unported,
                imported,
                &campaign(9, 392),
                &decks,
                &maps,
                &data.executable
            )
            .unwrap_err()
            .contains("not ported")
            || PORTED_OPPONENTS.contains(&missing)
        );
        // The White Rose route's Windsor (Tea, opponent 11): the request
        // (11, 1, 1011, 1100) seats the opponent's record 0x25 (Dancing
        // Elf) on side 0 and the player on side 1, who moves first.
        let windsor = DuelRequest {
            node: 12,
            opponent: 11,
            params: [11, 1, 1011, 1100],
        };
        let white = campaign_duel(
            &windsor,
            imported,
            &campaign(9, 392),
            &decks,
            &maps,
            &data.executable,
        )
        .unwrap();
        assert_eq!((white.sides[0].deck_record, white.sides[0].leader), (0x25, 394));
        assert_eq!(white.sides[1].leader, 392);
        assert_eq!((white.first, white.player_side, white.computer_sides.clone()), (1, 1, vec![0]));
        assert_eq!(white.leader_ranks[1], campaign(9, 392).leader_rank as u8);
        // Tea's controller (0x13E570) is ported: accepted with her control
        // byte on side 0.
        let accepted = duel_for_request(
            &windsor,
            imported,
            &campaign(9, 392),
            &decks,
            &maps,
            &data.executable,
        )
        .unwrap();
        assert_eq!((accepted.opponent, accepted.player_side), (11, 1));
        // An unported white opponent is refused, naming its leader (side 0).
        if !PORTED_OPPONENTS.contains(&missing) {
            let request = DuelRequest {
                node: 18,
                opponent: missing,
                params: [i32::from(missing), 1, 1000 + i32::from(missing), 1100],
            };
            let leader = decks[OPPONENT_DECK_BASE + usize::from(missing)].leader_card;
            let refused = duel_for_request(
                &request,
                imported,
                &campaign(9, 392),
                &decks,
                &maps,
                &data.executable,
            )
            .unwrap_err();
            assert!(refused.contains(&format!("leader {leader}")) && refused.contains("not ported"), "{refused}");
        }
    }

    /// The original's player block during the first duel (the private
    /// capture `duel/claude-end/fixture-767-weevil-after-t15/blk0.bin` under
    /// `ROSES_RESEARCH_DIR`, default `work/research`; turn 15, before
    /// the result screens) against the new game's layout up to the key and
    /// the random ids: recent list, the three deck records, current deck,
    /// every card's rank bits and instance shape (occupied, leader flag,
    /// category bits), the owned bit of every instanced card and the
    /// leader's experience. The duel itself had already marked five cards
    /// seen (bit 14 without an instance) and given five deck cards
    /// experience, so those are left out.
    #[test]
    #[ignore = "Requires a private game import (ROSES_GAME_IMPORT) and the private capture blk0.bin (ROSES_RESEARCH_DIR)"]
    fn private_capture_player_block_matches_the_new_game_layout() {
        use roses_core::card_block::{CardRecord, OWNED_BIT, RANK_MASK};
        use roses_core::deck_editor::{current_deck, deck_record};
        let path = std::env::var_os("ROSES_GAME_IMPORT").expect("ROSES_GAME_IMPORT");
        let game = GameImport::open(Path::new(&path)).unwrap();
        let data = crate::duel_play::load(&game.duel).unwrap();
        let decks = roses_assets::parse_pal_decks(&data.executable)
            .unwrap()
            .decks;
        let imported = &data.first_duel;
        let chosen = StarterDeckChoice {
            name_codes: vec![],
            checksum: 0,
            candidates: vec![9],
            selection: 0,
            record: 9,
            leader: imported.sides[0].leader,
            cards: imported.sides[0].cards.clone(),
        };
        let thresholds = roses_assets::parse_pal_rank_thresholds(&data.executable).unwrap();
        let mut rng = 7u64;
        let block =
            new_player_block(&chosen, &decks, &data.executable, &thresholds, &mut rng).unwrap();
        let research = std::env::var_os("ROSES_RESEARCH_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../work/research"));
        let fixture = research.join("duel/claude-end/fixture-767-weevil-after-t15/blk0.bin");
        let bytes = std::fs::read(&fixture)
            .unwrap_or_else(|e| panic!("private capture {}: {e}", fixture.display()));
        let original = CardBlock::from_bytes(&bytes).unwrap();
        assert_eq!(original.recent, block.recent);
        for d in 0..3 {
            assert_eq!(
                deck_record(&original, d),
                deck_record(&block, d),
                "deck {d}"
            );
        }
        assert_eq!(current_deck(&original), current_deck(&block));
        let shape = |r: &CardRecord| -> Vec<(bool, u32, u32)> {
            r.instances
                .iter()
                .map(|i| (i.is_free(), i.owner >> 31, i.id >> 30))
                .collect()
        };
        for (c, (o, b)) in original.records.iter().zip(&block.records).enumerate() {
            assert_eq!(
                o.rank_word & RANK_MASK,
                b.rank_word & RANK_MASK,
                "card {c} rank"
            );
            assert_eq!(shape(o), shape(b), "card {c} instances");
            if b.copies() > 0 {
                assert_eq!(o.rank_word & OWNED_BIT, OWNED_BIT, "card {c} owned");
            }
        }
        assert_eq!(original.experience(392), block.experience(392));
    }
}
