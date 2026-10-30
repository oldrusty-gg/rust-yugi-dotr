//! The server's match record: the setup (including the secret seed and the
//! deck sources) and every accepted change in revision order. It stays on
//! the server (it holds hidden information); replaying it with the same
//! rules data must reach the same final state.

use roses_core::duel::{
    build_campaign_decks, Catalogue, Duel, DuelCommand, DuelError, DuelSetup, PAD_CONTROLLER,
};
use roses_net::data::ImportDuel;
use roses_core::duel::script::DestinyRows;
use roses_net::{data_fingerprint, Actor, Cause, Compat, DeckSubmission, MatchOptions, RulesTables};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const REPLAY_SCHEMA: &str = "roses-online-replay-1";

/// Everything that determines the initial duel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MatchSetup {
    /// Seeds the campaign deck shuffle and then the duel's LCG.
    pub seed: u64,
    pub terrain: Vec<u8>,
    pub leaders: [u16; 2],
    /// Deck lists in stored order (shuffled at match start).
    pub deck_sources: [Vec<u16>; 2],
    pub first: u8,
    pub leader_abilities: [Vec<u8>; 2],
    /// Custom Duel LP and SP recharge rate, when set.
    pub custom_rules: Option<([u16; 2], [u8; 2])>,
    /// Each leader's stored rank (the HUD's insignia, the Destiny Draw's
    /// rank bonus); 0 when the setup does not say.
    #[serde(default)]
    pub leader_ranks: [u8; 2],
    /// The Destiny Draw rows of the executable (none: an armed Destiny
    /// Draw is refused).
    #[serde(default)]
    pub destiny_rows: Option<DestinyRows>,
}

impl MatchSetup {
    /// The first duel's board and both decks from the user's import (the
    /// player's starter deck against Weevil's), as a two-human match.
    pub fn from_import(first_duel: &ImportDuel, seed: u64) -> Self {
        Self {
            seed,
            terrain: first_duel.terrain.clone(),
            leaders: [first_duel.sides[0].leader, first_duel.sides[1].leader],
            deck_sources: [
                first_duel.sides[0].cards.clone(),
                first_duel.sides[1].cards.clone(),
            ],
            first: first_duel.first,
            leader_abilities: [vec![], vec![]],
            custom_rules: None,
            leader_ranks: [0; 2],
            destiny_rows: None,
        }
    }

    /// This setup with the seats' submitted decks (leaders, ranks, deck
    /// lists in stored order) and each leader's abilities at its rank from
    /// the ability table when that seat's Leader Ability switch is USE
    /// (NOT USE: none, as param 0x1C's clear bit makes 0x1E1F00 refuse
    /// every ability query of that side).
    pub fn with_decks(&self, decks: [&DeckSubmission; 2], tables: &RulesTables, options: MatchOptions) -> Self {
        let abilities = |s: usize| {
            if options.leader_ability[s] {
                tables.leader_abilities(decks[s].leader, decks[s].leader_rank)
            } else {
                Vec::new()
            }
        };
        Self {
            leaders: decks.map(|d| d.leader),
            leader_ranks: decks.map(|d| d.leader_rank),
            deck_sources: decks.map(|d| d.cards.clone()),
            leader_abilities: [abilities(0), abilities(1)],
            ..self.clone()
        }
    }

    /// The initial authoritative duel (both sides pad-controlled).
    pub fn build(&self, cards: &Catalogue) -> Result<Duel, DuelError> {
        let mut rng = self.seed;
        let decks = build_campaign_decks(
            &mut rng,
            [&self.deck_sources[0], &self.deck_sources[1]],
        );
        let (mut duel, _) = Duel::new(
            DuelSetup {
                terrain: self.terrain.clone(),
                leaders: self.leaders,
                decks,
                first: self.first,
                leader_abilities: self.leader_abilities.clone(),
                controllers: [PAD_CONTROLLER; 2],
            },
            cards,
        )?;
        duel.seed_rng(rng);
        duel.set_leader_ranks(self.leader_ranks);
        if let Some(rows) = self.destiny_rows {
            duel.set_destiny_rows(rows);
        }
        if let Some((lp, sp)) = self.custom_rules {
            duel.set_custom_rules(lp, sp);
        }
        Ok(duel)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReplayAction {
    Command { side: u8, command: DuelCommand },
    Forfeit { side: u8 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayEntry {
    /// The revision this change produced (1, 2, 3, ...).
    pub revision: u64,
    pub actor: Actor,
    pub cause: Cause,
    pub action: ReplayAction,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Replay {
    pub schema: String,
    pub match_id: String,
    pub compat: Compat,
    pub setup: MatchSetup,
    pub entries: Vec<ReplayEntry>,
    /// [`state_hash`] of the final (or current) state.
    pub final_state: String,
}

/// SHA-256 of the full state's JSON (server-side only).
pub fn state_hash(duel: &Duel) -> String {
    let bytes = serde_json::to_vec(duel).expect("duel serialises");
    format!("sha256:{:x}", Sha256::digest(&bytes))
}

/// Re-run a record. Incompatible protocol/rules/schema/data are refused
/// before anything runs; every entry must apply and reproduce its revision,
/// and the final state must match the recorded hash.
pub fn replay(record: &Replay, cards: &Catalogue, tables: &RulesTables) -> Result<Duel, String> {
    if record.schema != REPLAY_SCHEMA {
        return Err(format!("unsupported replay schema {}", record.schema));
    }
    Compat::current(data_fingerprint(cards, tables))
        .check(&record.compat)
        .map_err(|e| format!("incompatible replay: {e:?}"))?;
    let mut duel = record.setup.build(cards).map_err(|e| e.to_string())?;
    for (i, entry) in record.entries.iter().enumerate() {
        if entry.revision != i as u64 + 1 {
            return Err(format!("entry {i} has revision {}", entry.revision));
        }
        let result = match &entry.action {
            ReplayAction::Command { side, command } => duel.apply(*side, command, cards),
            ReplayAction::Forfeit { side } => duel.forfeit(*side),
        };
        duel = result
            .map_err(|e| format!("revision {} does not apply: {e}", entry.revision))?
            .0;
    }
    let hash = state_hash(&duel);
    if hash != record.final_state {
        return Err(format!(
            "replay reached {hash}, the server recorded {}",
            record.final_state
        ));
    }
    Ok(duel)
}

/// Delete the hub's replay files (`<match id>.json`) in `dir` last written
/// more than `max_age` ago (replay retention, `roses-server hub
/// --replay-days N`). Other files are left alone. Returns how many were
/// deleted.
pub fn prune_replays(dir: &std::path::Path, max_age: std::time::Duration) -> std::io::Result<usize> {
    let now = std::time::SystemTime::now();
    let mut deleted = 0;
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") || !entry.file_type()?.is_file() {
            continue;
        }
        let modified = entry.metadata()?.modified()?;
        if now.duration_since(modified).is_ok_and(|age| age > max_age) {
            std::fs::remove_file(&path)?;
            deleted += 1;
        }
    }
    Ok(deleted)
}
