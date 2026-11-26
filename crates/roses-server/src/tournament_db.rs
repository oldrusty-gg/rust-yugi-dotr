//! Tournaments in the account database (schema 2, a port addition): the
//! tournaments, their entries, a readable row per decided bracket match
//! (linked to the `matches` row of a played one), the operator's queued
//! commands (`roses-server accounts tournament cancel / kick`, run in
//! another process while the hub runs) and the winners' badges.

use super::{now_seconds, Accounts, DELETED_NAME};
use crate::tournament::{Bracket, Entry, Tournament};
use roses_net::lobby::{username_key, TournamentState};
use rusqlite::{params, OptionalExtension, Transaction};
use std::collections::HashMap;

/// The schema-2 tables (created in the upgrade's transaction).
pub const TOURNAMENT_TABLES: &str = "CREATE TABLE IF NOT EXISTS tournaments(
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        creator TEXT,
        weekly INTEGER NOT NULL DEFAULT 0,
        quick INTEGER NOT NULL DEFAULT 0,
        starts INTEGER NOT NULL,
        cap INTEGER NOT NULL,
        state TEXT NOT NULL,
        created INTEGER NOT NULL,
        seed TEXT,
        size INTEGER,
        champion TEXT,
        finished INTEGER,
        note TEXT,
        bracket TEXT);
     CREATE INDEX IF NOT EXISTS tournaments_state ON tournaments(state);
     CREATE TABLE IF NOT EXISTS tournament_entries(
        tournament_id INTEGER NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
        name_key TEXT NOT NULL,
        name TEXT NOT NULL,
        signed_up INTEGER NOT NULL,
        checked_in INTEGER NOT NULL DEFAULT 0,
        out INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY(tournament_id, name_key));
     CREATE TABLE IF NOT EXISTS tournament_matches(
        tournament_id INTEGER NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
        round INTEGER NOT NULL,
        slot INTEGER NOT NULL,
        player0 TEXT,
        player1 TEXT,
        winner TEXT,
        how TEXT NOT NULL,
        match_id TEXT,
        decided INTEGER NOT NULL,
        PRIMARY KEY(tournament_id, round, slot));
     CREATE TABLE IF NOT EXISTS tournament_actions(
        id INTEGER PRIMARY KEY,
        tournament_id INTEGER NOT NULL,
        action TEXT NOT NULL,
        name TEXT,
        created INTEGER NOT NULL);
     CREATE TABLE IF NOT EXISTS badges(
        name_key TEXT NOT NULL,
        kind TEXT NOT NULL,
        tournament_id INTEGER NOT NULL,
        awarded INTEGER NOT NULL,
        PRIMARY KEY(name_key, tournament_id));";

/// A badge's kind: a scheduled (weekly or player-made) tournament's win,
/// or a quick tournament's (a smaller badge, counted separately).
pub const BADGE_TOURNAMENT: &str = "tournament";
pub const BADGE_QUICK: &str = "quick";

/// An operator's command queued for the hub.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TournamentAction {
    pub id: i64,
    pub tournament: u64,
    /// "cancel" or "kick".
    pub action: String,
    pub name: Option<String>,
}

fn state_text(s: TournamentState) -> &'static str {
    match s {
        TournamentState::Open => "open",
        TournamentState::Drawn => "drawn",
        TournamentState::Running => "running",
        TournamentState::Finished => "finished",
        TournamentState::Cancelled => "cancelled",
    }
}

fn state_of(text: &str) -> TournamentState {
    match text {
        "drawn" => TournamentState::Drawn,
        "running" => TournamentState::Running,
        "finished" => TournamentState::Finished,
        "cancelled" => TournamentState::Cancelled,
        _ => TournamentState::Open,
    }
}

/// An erased account's traces in the tournament tables (inside
/// [`Accounts::delete_user`]'s transaction): badges and entries go, names
/// in brackets and results become [`DELETED_NAME`].
pub(super) fn forget_player(tx: &Transaction, key: &str, name: &str) -> rusqlite::Result<()> {
    tx.execute("DELETE FROM badges WHERE name_key = ?1", params![key])?;
    tx.execute("DELETE FROM tournament_entries WHERE name_key = ?1", params![key])?;
    for column in ["player0", "player1", "winner"] {
        tx.execute(&format!("UPDATE tournament_matches SET {column} = ?1 WHERE lower({column}) = ?2"), params![DELETED_NAME, key])?;
    }
    tx.execute("UPDATE tournaments SET champion = ?1 WHERE lower(champion) = ?2", params![DELETED_NAME, key])?;
    tx.execute("UPDATE tournaments SET creator = NULL WHERE lower(creator) = ?1", params![key])?;
    // The bracket's own copy (JSON): the name as a whole string only.
    let quoted = format!("\"{name}\"");
    let ids: Vec<(i64, String)> = {
        let mut st = tx.prepare("SELECT id, bracket FROM tournaments WHERE bracket IS NOT NULL")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect::<Result<_, _>>()?
    };
    for (id, bracket) in ids {
        if bracket.contains(&quoted) {
            let text = bracket.replace(&quoted, &format!("\"{DELETED_NAME}\""));
            tx.execute("UPDATE tournaments SET bracket = ?1 WHERE id = ?2", params![text, id])?;
        }
    }
    Ok(())
}

impl Accounts {
    fn tournament_from_row(&self, r: &rusqlite::Row) -> rusqlite::Result<Tournament> {
        let bracket: Option<String> = r.get(12)?;
        Ok(Tournament {
            id: r.get::<_, i64>(0)? as u64,
            name: r.get(1)?,
            creator: r.get(2)?,
            weekly: r.get::<_, i64>(3)? != 0,
            quick: r.get::<_, i64>(4)? != 0,
            starts: r.get(5)?,
            cap: r.get::<_, i64>(6)?.clamp(0, i64::from(u16::MAX)) as u16,
            state: state_of(&r.get::<_, String>(7)?),
            created: r.get(8)?,
            champion: r.get(9)?,
            finished: r.get(10)?,
            note: r.get(11)?,
            bracket: bracket.and_then(|b| serde_json::from_str::<Bracket>(&b).ok()),
            entries: Vec::new(),
        })
    }

    const TOURNAMENT_COLUMNS: &'static str =
        "id, name, creator, weekly, quick, starts, cap, state, created, champion, finished, note, bracket";

    fn tournaments_where(&self, clause: &str, args: impl rusqlite::Params) -> Result<Vec<Tournament>, String> {
        let e = |e: rusqlite::Error| e.to_string();
        let sql = format!("SELECT {} FROM tournaments {clause}", Self::TOURNAMENT_COLUMNS);
        let mut list: Vec<Tournament> = {
            let mut st = self.db.prepare(&sql).map_err(e)?;
            let rows = st.query_map(args, |r| self.tournament_from_row(r)).map_err(e)?;
            rows.collect::<Result<_, _>>().map_err(e)?
        };
        let mut st = self
            .db
            .prepare("SELECT name, signed_up, checked_in, out FROM tournament_entries WHERE tournament_id = ?1 ORDER BY signed_up, rowid")
            .map_err(e)?;
        for t in &mut list {
            let rows = st
                .query_map(params![t.id as i64], |r| {
                    Ok(Entry { name: r.get(0)?, signed_up: r.get(1)?, checked_in: r.get::<_, i64>(2)? != 0, out: r.get::<_, i64>(3)? != 0 })
                })
                .map_err(e)?;
            t.entries = rows.collect::<Result<_, _>>().map_err(e)?;
        }
        Ok(list)
    }

    /// Open, drawn and running tournaments, and the last `recent` that
    /// finished or were cancelled.
    pub fn load_tournaments(&self, recent: usize) -> Result<Vec<Tournament>, String> {
        let mut live = self.tournaments_where("WHERE state IN ('open', 'drawn', 'running') ORDER BY id", [])?;
        let done = self.tournaments_where(
            "WHERE state IN ('finished', 'cancelled') ORDER BY COALESCE(finished, created) DESC, id DESC LIMIT ?1",
            params![recent as i64],
        )?;
        live.extend(done);
        Ok(live)
    }

    /// Every tournament, newest first (the operator's list).
    pub fn all_tournaments(&self, limit: usize) -> Result<Vec<Tournament>, String> {
        self.tournaments_where("ORDER BY id DESC LIMIT ?1", params![limit as i64])
    }

    pub fn tournament(&self, id: u64) -> Result<Option<Tournament>, String> {
        Ok(self.tournaments_where("WHERE id = ?1", params![id as i64])?.pop())
    }

    /// Live tournaments made after `id` (by `accounts tournament create`
    /// while the hub runs).
    pub fn tournaments_after(&self, id: u64) -> Result<Vec<Tournament>, String> {
        self.tournaments_where("WHERE id > ?1 AND state IN ('open', 'drawn', 'running') ORDER BY id", params![id as i64])
    }

    /// Store a new tournament; returns its id.
    pub fn insert_tournament(&self, t: &Tournament) -> Result<u64, String> {
        self.db
            .execute(
                "INSERT INTO tournaments(name, creator, weekly, quick, starts, cap, state, created) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![t.name, t.creator, i64::from(t.weekly), i64::from(t.quick), t.starts, i64::from(t.cap), state_text(t.state), t.created],
            )
            .map_err(|e| e.to_string())?;
        Ok(self.db.last_insert_rowid() as u64)
    }

    /// Store a tournament's state, bracket and entries (one transaction).
    pub fn save_tournament(&mut self, t: &Tournament) -> Result<(), String> {
        let e = |e: rusqlite::Error| e.to_string();
        let bracket = t.bracket.as_ref().map(|b| serde_json::to_string(b).unwrap_or_default());
        let seed = t.bracket.as_ref().and_then(|b| b.seed).map(|s| format!("{s:016x}"));
        let size = t.bracket.as_ref().map(|b| b.size() as i64);
        let tx = self.db.transaction().map_err(e)?;
        tx.execute(
            "UPDATE tournaments SET name = ?2, starts = ?3, cap = ?4, state = ?5, seed = ?6, size = ?7, champion = ?8, finished = ?9, note = ?10, bracket = ?11 WHERE id = ?1",
            params![t.id as i64, t.name, t.starts, i64::from(t.cap), state_text(t.state), seed, size, t.champion, t.finished, t.note, bracket],
        )
        .map_err(e)?;
        tx.execute("DELETE FROM tournament_entries WHERE tournament_id = ?1", params![t.id as i64]).map_err(e)?;
        for en in &t.entries {
            tx.execute(
                "INSERT OR REPLACE INTO tournament_entries(tournament_id, name_key, name, signed_up, checked_in, out) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![t.id as i64, username_key(&en.name), en.name, en.signed_up, i64::from(en.checked_in), i64::from(en.out)],
            )
            .map_err(e)?;
        }
        // A readable row per decided match (the bracket history).
        if let Some(b) = &t.bracket {
            for (r, games) in b.rounds.iter().enumerate() {
                for (i, g) in games.iter().enumerate() {
                    let (Some(w), Some(how)) = (g.winner, g.how) else { continue };
                    let name = |s: usize| g.player(s).map(|p| b.players[p].clone());
                    tx.execute(
                        "INSERT OR IGNORE INTO tournament_matches(tournament_id, round, slot, player0, player1, winner, how, match_id, decided) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                        params![
                            t.id as i64,
                            r as i64 + 1,
                            i as i64,
                            name(0),
                            name(1),
                            name(usize::from(w)),
                            format!("{how:?}").to_lowercase(),
                            g.match_id,
                            now_seconds()
                        ],
                    )
                    .map_err(e)?;
                }
            }
        }
        tx.commit().map_err(e)
    }

    /// Link a decided bracket match to the `matches` row of its duel.
    pub fn link_tournament_match(&self, id: u64, round: usize, slot: usize, match_id: &str) -> Result<(), String> {
        self.db
            .execute(
                "UPDATE tournament_matches SET match_id = ?4 WHERE tournament_id = ?1 AND round = ?2 AND slot = ?3",
                params![id as i64, round as i64 + 1, slot as i64, match_id],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// The decided bracket matches of a tournament: (round, slot, player0,
    /// player1, winner, how, match id).
    #[allow(clippy::type_complexity)]
    pub fn tournament_results(&self, id: u64) -> Result<Vec<(i64, i64, Option<String>, Option<String>, Option<String>, String, Option<String>)>, String> {
        let mut st = self
            .db
            .prepare("SELECT round, slot, player0, player1, winner, how, match_id FROM tournament_matches WHERE tournament_id = ?1 ORDER BY round, slot")
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map(params![id as i64], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
    }

    /// Whether a weekly tournament starting then exists (in any state).
    pub fn weekly_exists(&self, starts: i64) -> Result<bool, String> {
        self.db
            .query_row("SELECT 1 FROM tournaments WHERE weekly <> 0 AND starts = ?1", params![starts], |_| Ok(()))
            .optional()
            .map(|r| r.is_some())
            .map_err(|e| e.to_string())
    }

    /// Queue an operator's command for the hub.
    pub fn queue_tournament_action(&self, id: u64, action: &str, name: Option<&str>) -> Result<(), String> {
        self.db
            .execute(
                "INSERT INTO tournament_actions(tournament_id, action, name, created) VALUES (?1, ?2, ?3, ?4)",
                params![id as i64, action, name, now_seconds()],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// The queued commands, taken (deleted) at once.
    pub fn take_tournament_actions(&mut self) -> Result<Vec<TournamentAction>, String> {
        let e = |e: rusqlite::Error| e.to_string();
        let tx = self.db.transaction().map_err(e)?;
        let list: Vec<TournamentAction> = {
            let mut st = tx.prepare("SELECT id, tournament_id, action, name FROM tournament_actions ORDER BY id").map_err(e)?;
            let rows = st
                .query_map([], |r| Ok(TournamentAction { id: r.get(0)?, tournament: r.get::<_, i64>(1)? as u64, action: r.get(2)?, name: r.get(3)? }))
                .map_err(e)?;
            rows.collect::<Result<_, _>>().map_err(e)?
        };
        if let Some(last) = list.last() {
            tx.execute("DELETE FROM tournament_actions WHERE id <= ?1", params![last.id]).map_err(e)?;
        }
        tx.commit().map_err(e)?;
        Ok(list)
    }

    /// Record a tournament win.
    pub fn award_badge(&self, name: &str, tournament: u64, quick: bool) -> Result<(), String> {
        let kind = if quick { BADGE_QUICK } else { BADGE_TOURNAMENT };
        self.db
            .execute(
                "INSERT OR IGNORE INTO badges(name_key, kind, tournament_id, awarded) VALUES (?1, ?2, ?3, ?4)",
                params![username_key(name), kind, tournament as i64, now_seconds()],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// A player's badges: (tournaments won, quick tournaments won).
    pub fn badges_of(&self, name: &str) -> Result<(u16, u16), String> {
        let count = |kind: &str| {
            self.db.query_row("SELECT COUNT(*) FROM badges WHERE name_key = ?1 AND kind = ?2", params![username_key(name), kind], |r| {
                r.get::<_, i64>(0)
            })
        };
        let (a, b) = (count(BADGE_TOURNAMENT).map_err(|e| e.to_string())?, count(BADGE_QUICK).map_err(|e| e.to_string())?);
        Ok((a.min(i64::from(u16::MAX)) as u16, b.min(i64::from(u16::MAX)) as u16))
    }

    /// Every player's badges by name key.
    pub fn all_badges(&self) -> Result<HashMap<String, (u16, u16)>, String> {
        let mut st = self.db.prepare("SELECT name_key, kind, COUNT(*) FROM badges GROUP BY name_key, kind").map_err(|e| e.to_string())?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))).map_err(|e| e.to_string())?;
        let mut out: HashMap<String, (u16, u16)> = HashMap::new();
        for row in rows {
            let (key, kind, n) = row.map_err(|e| e.to_string())?;
            let slot = out.entry(key).or_default();
            if kind == BADGE_QUICK {
                slot.1 = n as u16;
            } else {
                slot.0 = n as u16;
            }
        }
        Ok(out)
    }

    /// Finished online matches the player played (the create rule).
    pub fn finished_matches_of(&self, name: &str) -> Result<i64, String> {
        self.db
            .query_row(
                "SELECT COUNT(*) FROM matches WHERE lower(player0) = ?1 OR lower(player1) = ?1",
                params![username_key(name)],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())
    }
}
