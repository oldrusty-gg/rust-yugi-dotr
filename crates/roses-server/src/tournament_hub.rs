//! The hub's tournaments (a port addition): the scheduler on the hub
//! thread, the tournament requests of the lobby, the tournament matches
//! (ordinary 1v1 duels with the turn clock on and the hub's original-rules
//! options, the server recording the result) and the bracket file a web
//! page reads. The rules themselves are [`crate::tournament`]'s.
//!
//! Older clients (no `tournaments` in their lobby hello) are sent none of
//! this: no list, no bracket, no prompt and none of the new problems. Their
//! player lists carry the badge counts in fields they ignore.

use super::*;
use crate::tournament::{
    check_create, next_weekly, timeout_winner, Tournament, MIN_PLAYERS, RECENT_LISTED, WEEKLY_AHEAD_SECS, WEEKLY_CAP, WEEKLY_MINUTE,
    WEEKLY_NAME, WEEKLY_WEEKDAY,
};
use roses_net::lobby::{
    BracketView, TournamentPrompt, TournamentState, WinHow, CHECK_IN_OPENS_SECS, PREVIEW_BANNER, READY_SECS,
};
use std::collections::{BTreeSet, HashSet};

/// The scheduler's time steps run this often (ms of the hub's clock).
pub const TOURNAMENT_EVERY_MS: u64 = 1_000;
/// A tournament match's turn clock when the hub runs without one (ms):
/// tournaments always play with a clock.
pub const TOURNAMENT_TURN_MS: u64 = 120_000;

/// When the hub puts up its weekly tournament (UTC).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeeklySchedule {
    /// 0 = Monday ... 6 = Sunday.
    pub weekday: u8,
    /// Minutes after midnight.
    pub minute: u16,
    pub cap: u16,
    /// No weekly tournament starts before this (Unix seconds).
    pub not_before: Option<i64>,
}

impl Default for WeeklySchedule {
    /// Friday 19:00 UTC, 128 players.
    fn default() -> Self {
        Self { weekday: WEEKLY_WEEKDAY, minute: WEEKLY_MINUTE, cap: WEEKLY_CAP, not_before: None }
    }
}

impl WeeklySchedule {
    /// `fri 19:00` (a weekday's first three letters and the UTC time).
    pub fn parse(text: &str) -> Option<Self> {
        let (day, time) = text.trim().split_once(' ')?;
        let weekday = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"].iter().position(|d| day.to_ascii_lowercase().starts_with(d))? as u8;
        let (h, m) = time.trim().split_once(':')?;
        let (h, m): (u16, u16) = (h.parse().ok()?, m.parse().ok()?);
        (h < 24 && m < 60).then_some(Self { weekday, minute: h * 60 + m, ..Self::default() })
    }
}

/// The hub's tournament state.
#[derive(Default)]
pub struct TournamentRuntime {
    pub(super) list: BTreeMap<u64, Tournament>,
    /// Lobby connections whose client reads tournament messages.
    pub(super) conns: HashSet<ConnId>,
    /// The bracket each connection asked for last (kept up to date).
    pub(super) subs: HashMap<ConnId, u64>,
    /// Live tournament duels: match id -> (tournament, round, match).
    pub(super) by_match: HashMap<String, (u64, usize, usize)>,
    pub(super) at: Option<u64>,
    pub(super) loaded: bool,
    pub(super) dirty: BTreeSet<u64>,
    pub(super) max_id: u64,
    pub(super) file_dirty: bool,
}

/// What the hub writes to [`HubSettings::tournaments_file`] for a web page.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TournamentsFile {
    /// Unix seconds.
    pub written: i64,
    pub server: String,
    /// Shown over every bracket with `preview` set.
    pub preview_banner: String,
    pub tournaments: Vec<BracketView>,
}

fn order_key(t: &Tournament) -> (u8, i64, u64) {
    match t.state {
        _ if t.quick && t.live() => (0, 0, t.id),
        TournamentState::Open | TournamentState::Drawn | TournamentState::Running => (1, t.starts, t.id),
        _ => (2, -t.finished.unwrap_or(t.created), t.id),
    }
}

impl Hub {
    fn wall(&self) -> i64 {
        (self.settings.wall_clock)()
    }

    /// A lobby hello that said `tournaments`.
    pub(super) fn tournament_hello(&mut self, id: ConnId) {
        self.tourn.conns.insert(id);
    }

    pub(super) fn tournament_conn_gone(&mut self, id: ConnId) {
        self.tourn.conns.remove(&id);
        self.tourn.subs.remove(&id);
    }

    /// The tournaments (tests and the status file).
    pub fn tournaments(&self) -> Vec<&Tournament> {
        let mut v: Vec<&Tournament> = self.tourn.list.values().collect();
        v.sort_by_key(|t| order_key(t));
        v
    }

    fn tournament_conn_of(&self, name: &str) -> Option<ConnId> {
        self.lobby.conn_of(name).filter(|c| self.tourn.conns.contains(c))
    }

    fn mark(&mut self, tid: u64) {
        self.tourn.dirty.insert(tid);
    }

    fn tournament_problem(&mut self, id: ConnId, problem: LobbyProblem, now: u64) {
        self.lobby_send(id, LobbyEvent::Problem { problem }, now);
    }

    /// A signed-in player's tournament request.
    pub(super) fn tournament_request(&mut self, id: ConnId, user: &SignedIn, request: LobbyRequest, now: u64) {
        self.tourn.conns.insert(id);
        self.tournaments_loaded();
        let wall = self.wall();
        let me = user.name.clone();
        let result: Result<(), LobbyProblem> = match request {
            LobbyRequest::ListTournaments => {
                self.send_list(id, Some(&me), now);
                Ok(())
            }
            LobbyRequest::TournamentBracket { id: tid } => match self.tourn.list.get(&tid) {
                Some(t) => {
                    let bracket = t.view(Some(&me));
                    self.tourn.subs.insert(id, tid);
                    self.lobby_send(id, LobbyEvent::TournamentBracket { now: wall, bracket }, now);
                    Ok(())
                }
                None => Err(LobbyProblem::NoSuchTournament),
            },
            LobbyRequest::CreateTournament { name, starts, cap } => self.create_for(id, &me, &name, starts, cap, now),
            LobbyRequest::CancelTournament { id: tid } => match self.tourn.list.get_mut(&tid) {
                Some(t) if t.creator.as_deref().map(username_key) == Some(username_key(&me)) && t.state == TournamentState::Open => {
                    t.state = TournamentState::Cancelled;
                    t.finished = Some(wall);
                    t.note = Some("cancelled by its creator".into());
                    let line = format!("tournament {tid}: cancelled by its creator {me}");
                    self.mark(tid);
                    self.log(&line);
                    Ok(())
                }
                Some(_) => Err(LobbyProblem::TournamentClosed),
                None => Err(LobbyProblem::NoSuchTournament),
            },
            LobbyRequest::JoinTournament { id: tid } => match self.tourn.list.get_mut(&tid) {
                Some(t) => t.sign_up(&me, wall).map(|filled| {
                    self.mark(tid);
                    if filled {
                        self.quick_filled(tid, now);
                    }
                }),
                None => Err(LobbyProblem::NoSuchTournament),
            },
            LobbyRequest::LeaveTournament { id: tid } => match self.tourn.list.get_mut(&tid) {
                Some(t) => t.withdraw(&me, wall).map(|()| self.mark(tid)),
                None => Err(LobbyProblem::NoSuchTournament),
            },
            LobbyRequest::CheckIn { id: tid } => match self.tourn.list.get_mut(&tid) {
                Some(t) => t.check_in(&me, wall).map(|()| self.mark(tid)),
                None => Err(LobbyProblem::NoSuchTournament),
            },
            LobbyRequest::TournamentReady { id: tid } => self.say_ready(tid, &me, now),
            _ => Ok(()),
        };
        if let Err(problem) = result {
            self.tournament_problem(id, problem, now);
        }
    }

    fn create_for(&mut self, id: ConnId, me: &str, name: &str, starts: i64, cap: u16, now: u64) -> Result<(), LobbyProblem> {
        let wall = self.wall();
        let key = username_key(me);
        if self.tourn.list.values().any(|t| t.live() && t.creator.as_deref().map(username_key) == Some(key.clone())) {
            return Err(LobbyProblem::AlreadyHosting);
        }
        match self.accounts.finished_matches_of(me) {
            Ok(n) if n >= 1 => {}
            Ok(_) => return Err(LobbyProblem::NotEligible),
            Err(e) => {
                self.log(&format!("account database error: {e}"));
                return Err(LobbyProblem::ServerBusy);
            }
        }
        let filter = self.settings.name_filter.clone();
        check_create(name, starts, cap, wall, |w| w.is_empty() || filter.allows(w))?;
        let t = Tournament::new(name, Some(me.to_string()), starts, cap, false, wall);
        match self.add_tournament(t) {
            Some(tid) => {
                self.log(&format!("tournament {tid} \"{name}\" created by {me}: starts {} UTC, cap {cap}", crate::friends_cli::utc_stamp(starts)));
                self.lobby_send(id, LobbyEvent::TournamentCreated { id: tid }, now);
                Ok(())
            }
            None => Err(LobbyProblem::ServerBusy),
        }
    }

    /// Store a new tournament and list it; its id.
    fn add_tournament(&mut self, mut t: Tournament) -> Option<u64> {
        match self.accounts.insert_tournament(&t) {
            Ok(tid) => {
                t.id = tid;
                self.tourn.max_id = self.tourn.max_id.max(tid);
                self.tourn.list.insert(tid, t);
                self.mark(tid);
                Some(tid)
            }
            Err(e) => {
                self.log(&format!("could not store a tournament: {e}"));
                None
            }
        }
    }

    /// A quick tournament's last place was taken: its players are asked to
    /// check in, and the next quick tournament opens.
    fn quick_filled(&mut self, tid: u64, now: u64) {
        let Some(t) = self.tourn.list.get(&tid) else { return };
        let (info, closes) = (t.info(None), t.starts);
        let names: Vec<String> = t.entries.iter().map(|e| e.name.clone()).collect();
        self.log(&format!("tournament {tid}: the quick tournament filled ({}); check-in until {} UTC", names.join(", "), crate::friends_cli::utc_stamp(closes)));
        let wall = self.wall();
        for n in &names {
            if let Some(cid) = self.tournament_conn_of(n) {
                let mut info = info.clone();
                info.you = self.tourn.list.get(&tid).and_then(|t| t.status_of(n));
                self.lobby_send(cid, LobbyEvent::TournamentCheckIn { now: wall, tournament: info, closes }, now);
            }
        }
        self.open_quick();
    }

    /// The always-open quick tournament (when the server runs them).
    fn open_quick(&mut self) {
        if !self.settings.quick_tournaments {
            return;
        }
        if self.tourn.list.values().any(|t| t.quick && t.state == TournamentState::Open && !t.filled()) {
            return;
        }
        let wall = self.wall();
        if let Some(tid) = self.add_tournament(Tournament::quick(wall)) {
            self.log(&format!("tournament {tid}: a quick tournament is open"));
        }
    }

    fn say_ready(&mut self, tid: u64, me: &str, now: u64) -> Result<(), LobbyProblem> {
        let Some(t) = self.tourn.list.get_mut(&tid) else { return Err(LobbyProblem::NoSuchTournament) };
        let Some(b) = t.bracket.as_mut() else { return Err(LobbyProblem::NotEntered) };
        let Some(me_index) = b.index_of(me) else { return Err(LobbyProblem::NotEntered) };
        let found = b.open_games().into_iter().find_map(|(r, i)| {
            let g = &b.rounds[r][i];
            (0..2).find(|&s| g.player(s) == Some(me_index)).map(|s| (r, i, s))
        });
        let Some((r, i, s)) = found else { return Err(LobbyProblem::NotEntered) };
        let g = &mut b.rounds[r][i];
        if g.prompted.is_none() || g.match_id.is_some() {
            return Err(LobbyProblem::NotEntered);
        }
        g.ready[s] = true;
        self.mark(tid);
        self.send_prompts(tid, r, i, now);
        Ok(())
    }

    /// Both players of a ready match are told who they play and who is ready.
    fn send_prompts(&mut self, tid: u64, r: usize, i: usize, now: u64) {
        let wall = self.wall();
        let Some(t) = self.tourn.list.get(&tid) else { return };
        let Some(b) = t.bracket.as_ref() else { return };
        let g = &b.rounds[r][i];
        let (Some(p0), Some(p1), Some(at)) = (g.player(0), g.player(1), g.prompted) else { return };
        let names = [b.players[p0].clone(), b.players[p1].clone()];
        let prompts: Vec<(String, TournamentPrompt)> = (0..2)
            .map(|s| {
                (
                    names[s].clone(),
                    TournamentPrompt {
                        tournament: tid,
                        name: t.name.clone(),
                        round: r as u8 + 1,
                        rounds: b.rounds.len() as u8,
                        opponent: names[1 - s].clone(),
                        deadline: at + READY_SECS,
                        you_ready: g.ready[s],
                        opponent_ready: g.ready[1 - s],
                    },
                )
            })
            .collect();
        for (name, prompt) in prompts {
            if let Some(cid) = self.tournament_conn_of(&name) {
                self.lobby_send(cid, LobbyEvent::TournamentMatch { now: wall, prompt }, now);
            }
        }
    }

    /// Back in the lobby: the badges, and any prompt still waiting.
    pub(super) fn tournament_sign_in(&mut self, name: &str, now: u64) {
        self.tournaments_loaded();
        match self.accounts.badges_of(name) {
            Ok(titles) => {
                let actions = self.lobby.set_titles(name, titles);
                self.act(actions, now);
            }
            Err(e) => self.log(&format!("account database error: {e}")),
        }
        let key = username_key(name);
        let mut waiting = Vec::new();
        let mut check_ins = Vec::new();
        for (tid, t) in &self.tourn.list {
            if t.quick && t.filled() && t.state == TournamentState::Open && t.entry(name).is_some_and(|e| !e.checked_in && !e.out) {
                check_ins.push(*tid);
            }
            let Some(b) = &t.bracket else { continue };
            for (r, i) in b.open_games() {
                let g = &b.rounds[r][i];
                let mine = (0..2).any(|s| g.player(s).is_some_and(|p| username_key(&b.players[p]) == key));
                if mine && g.prompted.is_some() && g.match_id.is_none() && t.state == TournamentState::Running {
                    waiting.push((*tid, r, i));
                }
            }
        }
        for (tid, r, i) in waiting {
            self.send_prompts(tid, r, i, now);
        }
        let wall = self.wall();
        for tid in check_ins {
            let Some(t) = self.tourn.list.get(&tid) else { continue };
            let (info, closes) = (t.info(Some(name)), t.starts);
            if let Some(cid) = self.tournament_conn_of(name) {
                self.lobby_send(cid, LobbyEvent::TournamentCheckIn { now: wall, tournament: info, closes }, now);
            }
        }
    }

    fn send_list(&mut self, id: ConnId, viewer: Option<&str>, now: u64) {
        let wall = self.wall();
        let tournaments = self.tournaments().into_iter().map(|t| t.info(viewer)).collect();
        self.lobby_send(id, LobbyEvent::Tournaments { now: wall, tournaments }, now);
    }

    /// Load the stored tournaments once (live duels are gone after a
    /// restart: their bracket matches are asked to ready up again).
    fn tournaments_loaded(&mut self) {
        if self.tourn.loaded {
            return;
        }
        self.tourn.loaded = true;
        match self.accounts.load_tournaments(RECENT_LISTED) {
            Ok(list) => {
                for mut t in list {
                    if let Some(b) = t.bracket.as_mut() {
                        for g in b.rounds.iter_mut().flatten() {
                            if g.winner.is_none() && g.match_id.is_some() {
                                g.reset();
                            }
                        }
                    }
                    self.tourn.max_id = self.tourn.max_id.max(t.id);
                    self.tourn.list.insert(t.id, t);
                }
            }
            Err(e) => self.log(&format!("loading the tournaments failed: {e}")),
        }
        self.tourn.file_dirty = true;
    }

    /// One pass: the time steps (every [`TOURNAMENT_EVERY_MS`]), then
    /// whatever changed is stored and sent.
    pub(super) fn tournament_pass(&mut self, now: u64) {
        if self.tourn.at.is_none_or(|t| now.saturating_sub(t) >= TOURNAMENT_EVERY_MS) {
            self.tourn.at = Some(now);
            self.tournaments_loaded();
            self.tournament_admin();
            self.schedule_weekly();
            self.open_quick();
            let ids: Vec<u64> = self.tourn.list.keys().copied().collect();
            for tid in ids {
                self.progress(tid, now);
            }
            self.prune_listed();
        }
        self.flush_tournaments(now);
    }

    /// The operator's commands: new tournaments in the database, queued
    /// cancels and kicks.
    fn tournament_admin(&mut self) {
        match self.accounts.tournaments_after(self.tourn.max_id) {
            Ok(new) => {
                for t in new {
                    let line = format!("tournament {} \"{}\" loaded (made by the operator): starts {} UTC", t.id, t.name, crate::friends_cli::utc_stamp(t.starts));
                    self.tourn.max_id = self.tourn.max_id.max(t.id);
                    let tid = t.id;
                    self.tourn.list.insert(tid, t);
                    self.mark(tid);
                    self.log(&line);
                }
            }
            Err(e) => self.log(&format!("account database error: {e}")),
        }
        let actions = match self.accounts.take_tournament_actions() {
            Ok(a) => a,
            Err(e) => return self.log(&format!("account database error: {e}")),
        };
        let wall = self.wall();
        for a in actions {
            let Some(t) = self.tourn.list.get_mut(&a.tournament) else {
                self.log(&format!("tournament {}: no such live tournament for {}", a.tournament, a.action));
                continue;
            };
            let line = match (a.action.as_str(), a.name.as_deref()) {
                ("cancel", _) if t.live() => {
                    t.state = TournamentState::Cancelled;
                    t.finished = Some(wall);
                    t.note = Some("cancelled by the server's admin".into());
                    format!("tournament {}: cancelled by the admin", a.tournament)
                }
                ("kick", Some(name)) => match t.state {
                    TournamentState::Open if !t.filled() => {
                        let _ = t.withdraw(name, wall);
                        format!("tournament {}: {name} removed by the admin", a.tournament)
                    }
                    _ if t.live() => {
                        let _ = t.withdraw(name, wall);
                        format!("tournament {}: {name} removed by the admin (their matches are lost)", a.tournament)
                    }
                    _ => format!("tournament {}: is over; {name} not removed", a.tournament),
                },
                (other, _) => format!("tournament {}: nothing done for {other}", a.tournament),
            };
            let tid = a.tournament;
            self.mark(tid);
            self.log(&line);
        }
        let cancelled: Vec<String> = self
            .tourn
            .by_match
            .iter()
            .filter(|(_, (tid, ..))| self.tourn.list.get(tid).is_none_or(|t| !t.live()))
            .map(|(m, _)| m.clone())
            .collect();
        for m in cancelled {
            self.tourn.by_match.remove(&m);
        }
    }

    /// Put up the weekly tournament [`WEEKLY_AHEAD_SECS`] ahead (so sign-up
    /// is open for at least 7 days).
    fn schedule_weekly(&mut self) {
        let Some(w) = self.settings.weekly else { return };
        let wall = self.wall();
        let from = wall.max(w.not_before.unwrap_or(i64::MIN));
        let mut at = next_weekly(from, w.weekday, w.minute);
        while at <= wall + WEEKLY_AHEAD_SECS {
            // Too close to put up now (the first run of a new server).
            let soon = at - wall <= CHECK_IN_OPENS_SECS;
            let known = self.tourn.list.values().any(|t| t.weekly && t.starts == at);
            if !soon && !known {
                match self.accounts.weekly_exists(at) {
                    Ok(false) => {
                        if let Some(tid) = self.add_tournament(Tournament::new(WEEKLY_NAME, None, at, w.cap, true, wall)) {
                            self.log(&format!("tournament {tid}: the weekly tournament for {} UTC is open for sign-up", crate::friends_cli::utc_stamp(at)));
                        }
                    }
                    Ok(true) => {}
                    Err(e) => self.log(&format!("account database error: {e}")),
                }
            }
            at += 7 * 86_400;
        }
    }

    /// Finished tournaments beyond the recent ones leave memory (they stay
    /// in the database).
    fn prune_listed(&mut self) {
        let mut done: Vec<(i64, u64)> =
            self.tourn.list.values().filter(|t| !t.live() && !self.tourn.dirty.contains(&t.id)).map(|t| (t.finished.unwrap_or(t.created), t.id)).collect();
        if done.len() <= RECENT_LISTED {
            return;
        }
        done.sort_unstable();
        for (_, tid) in &done[..done.len() - RECENT_LISTED] {
            self.tourn.list.remove(tid);
            self.tourn.file_dirty = true;
        }
    }

    /// Whether a player can start a tournament match now.
    fn tournament_free(&self, name: &str) -> bool {
        !self.lobby.in_match(name) && self.live_match_of(name).is_none() && self.live_multi_of(name).is_none()
    }

    /// A tournament's time steps: the draw, the start, the matches.
    fn progress(&mut self, tid: u64, now: u64) {
        let wall = self.wall();
        let Some(t) = self.tourn.list.get_mut(&tid) else { return };
        if t.state == TournamentState::Open && t.draw_at().is_some_and(|d| wall >= d) {
            let seed = self.settings.fixed_seed.unwrap_or_else(crate::random_u64);
            let checked = t.entries.iter().filter(|e| e.checked_in && !e.out).count();
            let n = t.make_draw(seed);
            let line = if t.state == TournamentState::Cancelled {
                format!("tournament {tid} \"{}\": cancelled at the draw, {n} checked in (at least {MIN_PLAYERS} needed)", t.name)
            } else {
                let b = t.bracket.as_ref().expect("drawn");
                format!(
                    "tournament {tid} \"{}\": drawn with seed {seed:016x} from {checked} checked-in players of {}: a bracket of {}; order {}",
                    t.name,
                    t.entries.len(),
                    b.size(),
                    b.players.join(", ")
                )
            };
            if t.state == TournamentState::Cancelled {
                t.finished = Some(wall);
            }
            self.mark(tid);
            self.log(&line);
        }
        let Some(t) = self.tourn.list.get_mut(&tid) else { return };
        if t.state == TournamentState::Drawn && wall >= t.starts {
            t.state = TournamentState::Running;
            let line = format!("tournament {tid} \"{}\": started", t.name);
            self.mark(tid);
            self.log(&line);
        }
        if self.tourn.list.get(&tid).is_some_and(|t| t.state == TournamentState::Running) {
            self.run_games(tid, wall, now);
        }
    }

    /// The running tournament's open matches: withdrawals, ready prompts,
    /// starting the duel, the ready time; then its end.
    fn run_games(&mut self, tid: u64, wall: i64, now: u64) {
        enum Step {
            Decide(usize, usize, u8, WinHow),
            Prompt(usize, usize),
            Start(usize, usize, String, String),
        }
        let Some(t) = self.tourn.list.get(&tid) else { return };
        let Some(b) = t.bracket.as_ref() else { return };
        let mut steps = Vec::new();
        for (r, i) in b.open_games() {
            let g = &b.rounds[r][i];
            let names = [b.players[g.player(0).expect("open")].clone(), b.players[g.player(1).expect("open")].clone()];
            let out = names.clone().map(|n| t.entry(&n).is_none_or(|e| e.out));
            if out[0] || out[1] {
                let w = if out[0] && !out[1] { 1 } else { 0 };
                steps.push(Step::Decide(r, i, w, WinHow::Withdrew));
                continue;
            }
            if g.match_id.is_some() {
                continue;
            }
            match g.prompted {
                None => steps.push(Step::Prompt(r, i)),
                Some(_) if g.ready == [true, true] => {
                    if self.tournament_free(&names[0]) && self.tournament_free(&names[1]) {
                        let [a, b] = names;
                        steps.push(Step::Start(r, i, a, b));
                    }
                }
                Some(at) if wall - at >= READY_SECS => {
                    if let Some((w, how)) = timeout_winner(g) {
                        steps.push(Step::Decide(r, i, w, how));
                    }
                }
                Some(_) => {}
            }
        }
        for step in steps {
            match step {
                Step::Decide(r, i, w, how) => {
                    let Some(b) = self.tourn.list.get_mut(&tid).and_then(|t| t.bracket.as_mut()) else { continue };
                    let names: Vec<String> = (0..2).filter_map(|s| b.rounds[r][i].player(s)).map(|p| b.players[p].clone()).collect();
                    b.decide(r, i, w, how);
                    self.mark(tid);
                    let line = format!("tournament {tid}: round {} {} goes through ({how:?})", r + 1, names.get(usize::from(w)).cloned().unwrap_or_default());
                    self.log(&line);
                }
                Step::Prompt(r, i) => {
                    if let Some(g) = self.tourn.list.get_mut(&tid).and_then(|t| t.bracket.as_mut()).map(|b| &mut b.rounds[r][i]) {
                        g.prompted = Some(wall);
                    }
                    self.mark(tid);
                    self.send_prompts(tid, r, i, now);
                }
                Step::Start(r, i, a, b) => {
                    if let Some(mid) = self.create_match(a.clone(), Some(b.clone()), None, None, true, true, now) {
                        if let Some(g) = self.tourn.list.get_mut(&tid).and_then(|t| t.bracket.as_mut()).map(|b| &mut b.rounds[r][i]) {
                            g.match_id = Some(mid.clone());
                        }
                        self.log(&format!("tournament {tid}: round {} {a} vs {b} is {mid}", r + 1));
                        self.tourn.by_match.insert(mid, (tid, r, i));
                        self.mark(tid);
                    }
                }
            }
        }
        self.end_if_done(tid, wall, now);
    }

    fn end_if_done(&mut self, tid: u64, wall: i64, now: u64) {
        let Some(t) = self.tourn.list.get_mut(&tid) else { return };
        let Some(b) = t.bracket.as_ref() else { return };
        if !b.done() || !t.live() {
            return;
        }
        let champion = b.champion().map(|p| b.players[p].clone());
        t.state = TournamentState::Finished;
        t.finished = Some(wall);
        t.champion = champion.clone();
        let (quick, name) = (t.quick, t.name.clone());
        self.mark(tid);
        let Some(champion) = champion else {
            return self.log(&format!("tournament {tid} \"{name}\": finished with no champion"));
        };
        self.log(&format!("tournament {tid} \"{name}\": {champion} is the champion"));
        if let Err(e) = self.accounts.award_badge(&champion, tid, quick) {
            self.log(&format!("could not record {champion}'s badge: {e}"));
        }
        if let Ok(titles) = self.accounts.badges_of(&champion) {
            let actions = self.lobby.set_titles(&champion, titles);
            self.act(actions, now);
        }
    }

    /// A tournament duel ended (or was cancelled): the bracket learns it.
    /// `winner`: none for a cancelled match, `Some(None)` for a draw (it is
    /// played again).
    pub(super) fn tournament_match_over(&mut self, mid: &str, players: &[String; 2], winner: Option<Option<u8>>, joined: [bool; 2], now: u64) {
        let Some((tid, r, i)) = self.tourn.by_match.remove(mid) else { return };
        let wall = self.wall();
        let Some(b) = self.tourn.list.get_mut(&tid).and_then(|t| t.bracket.as_mut()) else { return };
        if b.rounds[r][i].winner.is_some() {
            return;
        }
        let side_of = |b: &crate::tournament::Bracket, name: &str| {
            let g = &b.rounds[r][i];
            (0..2u8).find(|&s| g.player(usize::from(s)).is_some_and(|p| username_key(&b.players[p]) == username_key(name)))
        };
        let line = match winner {
            Some(Some(w)) => match side_of(b, &players[usize::from(w & 1)]) {
                Some(s) => {
                    b.decide(r, i, s, WinHow::Played);
                    format!("tournament {tid}: round {} won by {} ({mid})", r + 1, players[usize::from(w & 1)])
                }
                None => format!("tournament {tid}: {mid} has a winner outside its match"),
            },
            Some(None) => {
                b.rounds[r][i].reset();
                format!("tournament {tid}: round {} was a draw; it is played again", r + 1)
            }
            None => {
                let came: Vec<u8> = (0..2u8).filter(|&s| joined[usize::from(s)]).filter_map(|s| side_of(b, &players[usize::from(s)])).collect();
                match came.as_slice() {
                    [s] => {
                        b.decide(r, i, *s, WinHow::Walkover);
                        format!("tournament {tid}: round {} walkover, only one player joined {mid}", r + 1)
                    }
                    [] => {
                        let mut g = b.rounds[r][i].clone();
                        g.ready = [false; 2];
                        let (w, _) = timeout_winner(&g).unwrap_or((0, WinHow::NoShow));
                        b.decide(r, i, w, WinHow::NoShow);
                        format!("tournament {tid}: round {} nobody joined {mid}; the higher seed goes through", r + 1)
                    }
                    _ => {
                        b.rounds[r][i].reset();
                        format!("tournament {tid}: round {} {mid} broke off; it is played again", r + 1)
                    }
                }
            }
        };
        self.mark(tid);
        self.log(&line);
        self.end_if_done(tid, wall, now);
    }

    /// Store and send what changed; the file is written by the admin pass.
    fn flush_tournaments(&mut self, now: u64) {
        if self.tourn.dirty.is_empty() {
            return;
        }
        let dirty = std::mem::take(&mut self.tourn.dirty);
        for tid in &dirty {
            if let Some(t) = self.tourn.list.get(tid).cloned() {
                if let Err(e) = self.accounts.save_tournament(&t) {
                    self.log(&format!("tournament {tid}: not stored: {e}"));
                }
                // Link the played matches' history rows to their duels.
                if let Some(b) = &t.bracket {
                    for (r, games) in b.rounds.iter().enumerate() {
                        for (i, g) in games.iter().enumerate() {
                            if let (Some(_), Some(mid)) = (g.winner, &g.match_id) {
                                let _ = self.accounts.link_tournament_match(*tid, r, i, mid);
                            }
                        }
                    }
                }
            }
        }
        self.tourn.file_dirty = true;
        let conns: Vec<ConnId> = self.tourn.conns.iter().copied().collect();
        for cid in conns {
            let viewer = self.signed_in(cid).map(|u| u.name);
            let Some(viewer) = viewer else { continue };
            self.send_list(cid, Some(&viewer), now);
            if let Some(tid) = self.tourn.subs.get(&cid).copied().filter(|t| dirty.contains(t)) {
                if let Some(t) = self.tourn.list.get(&tid) {
                    let bracket = t.view(Some(&viewer));
                    let wall = self.wall();
                    self.lobby_send(cid, LobbyEvent::TournamentBracket { now: wall, bracket }, now);
                }
            }
        }
    }

    /// The brackets for a web page ([`HubSettings::tournaments_file`]).
    pub fn tournaments_file(&self) -> TournamentsFile {
        TournamentsFile {
            written: self.wall(),
            server: self.settings.name.clone(),
            preview_banner: PREVIEW_BANNER.into(),
            tournaments: self.tournaments().into_iter().map(|t| t.view(None)).collect(),
        }
    }

    pub(super) fn write_tournaments_file(&mut self) {
        let Some(path) = self.settings.tournaments_file.clone() else { return };
        if !self.tourn.file_dirty {
            return;
        }
        let file = self.tournaments_file();
        let written = serde_json::to_vec_pretty(&file).map_err(|e| e.to_string()).and_then(|bytes| {
            let tmp = path.with_extension("json.new");
            std::fs::write(&tmp, bytes).and_then(|_| std::fs::rename(&tmp, &path)).map_err(|e| e.to_string())
        });
        match written {
            Ok(()) => self.tourn.file_dirty = false,
            Err(e) => self.log(&format!("writing the tournaments file {} failed: {e}", path.display())),
        }
    }
}
