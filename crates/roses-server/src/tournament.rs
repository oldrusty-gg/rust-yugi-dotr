//! Tournaments (a port addition, not in the 2003 game): straight knockout,
//! best of one, original rules. Pure functions and state, no sockets and
//! no clock of their own (times are Unix seconds passed in); the hub
//! (`crate::hub::tournament_hub`) runs them, the account database keeps
//! them (`crate::accounts`).
//!
//! - **Sizing.** The bracket is the next power of two at or above the
//!   players, at least [`MIN_BRACKET`]: 4, 8, 16 ... 128. Empty slots are
//!   byes, a walkover for the player drawn against one.
//! - **Seeding.** The draw shuffles the checked-in players with a seed the
//!   hub logs ([`shuffle`]); seed 1 is the first after the shuffle. Slots
//!   follow the standard order ([`seed_slots`]: 1 v 16, 8 v 9, ...), so the
//!   byes go to the top seeds and two byes never meet while more than half
//!   the slots have players.
//! - **Timing** (`roses_net::lobby`): sign-up is open from creation until
//!   the draw; check-in from [`CHECK_IN_OPENS_SECS`] before the start until
//!   the draw at [`DRAW_SECS`] before it, which uses only the checked-in
//!   players (the others are dropped); matches begin at the start time.
//!   Before the draw the bracket is a preview of the players signed up, in
//!   sign-up order ([`Tournament::view`]).
//! - **Quick tournaments** ([`QUICK_PLAYERS`]): no fixed time; when the
//!   last place is taken the players have [`QUICK_CHECK_IN_SECS`] to check
//!   in, then the draw is made and the matches begin at once.
//! - **Matches.** When both players of a match are known, both are asked
//!   to say ready; one who does not within `READY_SECS` loses by
//!   walkover, and if neither does the higher seed goes through
//!   ([`timeout_winner`]).

use roses_net::lobby::{
    check_tournament_name, username_key, BracketGame, BracketSide, BracketView, EntryStatus, GameState, LobbyProblem,
    TournamentInfo, TournamentState, WinHow, CHECK_IN_OPENS_SECS, DRAW_SECS, QUICK_CHECK_IN_SECS, QUICK_PLAYERS,
    TOURNAMENT_CAP_MAX, TOURNAMENT_CAP_MIN,
};
use serde::{Deserialize, Serialize};

/// The smallest bracket.
pub const MIN_BRACKET: usize = 4;
/// A draw with fewer checked-in players than this cancels the tournament.
pub const MIN_PLAYERS: usize = 2;
/// A player's tournament must start at least this long after it is made
/// (the whole check-in hour ahead), and at most this far ahead (seconds).
pub const CREATE_MIN_LEAD_SECS: i64 = CHECK_IN_OPENS_SECS;
pub const CREATE_MAX_AHEAD_SECS: i64 = 30 * 86_400;
/// The server's weekly tournament is put up this long before its start, so
/// sign-up is open for at least 7 days (seconds).
pub const WEEKLY_AHEAD_SECS: i64 = 8 * 86_400;
/// Finished and cancelled tournaments listed besides the open ones.
pub const RECENT_LISTED: usize = 10;
/// The weekly default: Friday 19:00 UTC (weekday 0 = Monday).
pub const WEEKLY_WEEKDAY: u8 = 4;
pub const WEEKLY_MINUTE: u16 = 19 * 60;
pub const WEEKLY_NAME: &str = "Weekly Tournament";
pub const WEEKLY_CAP: u16 = TOURNAMENT_CAP_MAX;
pub const QUICK_NAME: &str = "Quick Tournament";

/// The bracket for `players`: the next power of two, at least 4.
pub fn bracket_size(players: usize) -> usize {
    players.max(1).next_power_of_two().max(MIN_BRACKET)
}

/// The seed (1-based) in each first-round slot of a `size` bracket, slot
/// by slot: pairs (0, 1), (2, 3) ... meet in the first round. The standard
/// order: 1 v size, and each half of the bracket keeps the seeds that
/// would meet in the later rounds apart.
pub fn seed_slots(size: usize) -> Vec<usize> {
    let mut slots = vec![1];
    while slots.len() < size {
        let n = slots.len() * 2;
        slots = slots.iter().flat_map(|&s| [s, n + 1 - s]).collect();
    }
    slots
}

/// A small deterministic generator for the logged draw (SplitMix64).
fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// `names` in the order the draw with `seed` gives (Fisher-Yates): the same
/// seed and names always give the same order, so a logged seed lets the
/// draw be checked.
pub fn shuffle(names: &[String], seed: u64) -> Vec<String> {
    let mut out = names.to_vec();
    let mut state = seed;
    for i in (1..out.len()).rev() {
        let j = (splitmix(&mut state) % (i as u64 + 1)) as usize;
        out.swap(i, j);
    }
    out
}

/// One side of a bracket match (internal: players by their index, which is
/// their seed minus one).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Side {
    Tbd,
    Bye,
    Player { index: u16 },
}

/// A match of the bracket.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Game {
    pub sides: [Side; 2],
    pub winner: Option<u8>,
    pub how: Option<WinHow>,
    /// Each side said ready.
    pub ready: [bool; 2],
    /// When both players were asked to say ready (Unix seconds).
    pub prompted: Option<i64>,
    /// The live match playing it.
    pub match_id: Option<String>,
}

impl Game {
    fn new(sides: [Side; 2]) -> Self {
        Self { sides, winner: None, how: None, ready: [false; 2], prompted: None, match_id: None }
    }

    /// Both sides are players and it is not decided.
    pub fn open(&self) -> bool {
        self.winner.is_none() && matches!(self.sides, [Side::Player { .. }, Side::Player { .. }])
    }

    pub fn player(&self, side: usize) -> Option<usize> {
        match self.sides[side] {
            Side::Player { index } => Some(usize::from(index)),
            _ => None,
        }
    }

    /// Back to waiting for both to say ready (a draw, or a match that
    /// never ran).
    pub fn reset(&mut self) {
        self.ready = [false; 2];
        self.prompted = None;
        self.match_id = None;
    }
}

/// Who goes through when the ready time is over: the one who said ready;
/// if neither did, the higher seed (the lower index). Both ready: none
/// (they wait for each other to be free).
pub fn timeout_winner(game: &Game) -> Option<(u8, WinHow)> {
    match game.ready {
        [true, false] => Some((0, WinHow::Walkover)),
        [false, true] => Some((1, WinHow::Walkover)),
        [false, false] => {
            let seed = |s: usize| game.player(s).unwrap_or(usize::MAX);
            Some((if seed(0) <= seed(1) { 0 } else { 1 }, WinHow::NoShow))
        }
        [true, true] => None,
    }
}

/// A bracket: the players by seed, and the rounds from the first to the
/// final.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bracket {
    /// By seed: `players[0]` is seed 1.
    pub players: Vec<String>,
    pub rounds: Vec<Vec<Game>>,
    /// The draw's seed (none: a preview).
    pub seed: Option<u64>,
}

impl Bracket {
    /// `players` by seed in a bracket of the right size, nothing decided.
    pub fn seeded(players: Vec<String>) -> Self {
        let size = bracket_size(players.len());
        let slots = seed_slots(size);
        let side = |slot: usize| {
            let seed = slots[slot];
            if seed <= players.len() { Side::Player { index: (seed - 1) as u16 } } else { Side::Bye }
        };
        let mut rounds = vec![(0..size / 2).map(|i| Game::new([side(2 * i), side(2 * i + 1)])).collect::<Vec<_>>()];
        let mut games = size / 4;
        while games >= 1 {
            rounds.push((0..games).map(|_| Game::new([Side::Tbd; 2])).collect());
            games /= 2;
        }
        Self { players, rounds, seed: None }
    }

    /// The draw: `entrants` shuffled with `seed`, byes settled.
    pub fn draw(entrants: &[String], seed: u64) -> Self {
        let mut b = Self::seeded(shuffle(entrants, seed));
        b.seed = Some(seed);
        b.settle();
        b
    }

    pub fn size(&self) -> usize {
        self.rounds.first().map_or(0, |r| r.len() * 2)
    }

    /// Decide `game` of `round` for `winner`, then carry results forward.
    pub fn decide(&mut self, round: usize, game: usize, winner: u8, how: WinHow) {
        let g = &mut self.rounds[round][game];
        if g.winner.is_some() {
            return;
        }
        g.winner = Some(winner);
        g.how = Some(how);
        // The match id stays: the history row links to the duel's record.
        self.settle();
    }

    /// Carry every decided match's winner into the next round, and decide
    /// the matches against a bye (two byes give a bye), until nothing moves.
    pub fn settle(&mut self) {
        loop {
            let mut moved = false;
            for r in 0..self.rounds.len() {
                for i in 0..self.rounds[r].len() {
                    let g = &self.rounds[r][i];
                    if g.winner.is_none() {
                        let w = match g.sides {
                            [Side::Bye, Side::Bye] | [Side::Player { .. }, Side::Bye] => Some(0),
                            [Side::Bye, Side::Player { .. }] => Some(1),
                            _ => None,
                        };
                        if let Some(w) = w {
                            let g = &mut self.rounds[r][i];
                            g.winner = Some(w);
                            g.how = Some(WinHow::Bye);
                            moved = true;
                        }
                    }
                    let g = &self.rounds[r][i];
                    if let (Some(w), Some(next)) = (g.winner, self.rounds.get(r + 1)) {
                        let through = g.sides[usize::from(w)];
                        let target = &next[i / 2].sides[i % 2];
                        if *target == Side::Tbd && through != Side::Tbd {
                            self.rounds[r + 1][i / 2].sides[i % 2] = through;
                            moved = true;
                        }
                    }
                }
            }
            if !moved {
                return;
            }
        }
    }

    /// The player who won the final.
    pub fn champion(&self) -> Option<usize> {
        let f = self.rounds.last()?.first()?;
        f.player(usize::from(f.winner?))
    }

    /// The final is decided (a champion, or no player left in it).
    pub fn done(&self) -> bool {
        self.rounds.last().and_then(|r| r.first()).is_some_and(|g| g.winner.is_some())
    }

    /// The open matches (both players known, not decided): (round, match).
    pub fn open_games(&self) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for (r, games) in self.rounds.iter().enumerate() {
            for (i, g) in games.iter().enumerate() {
                if g.open() {
                    out.push((r, i));
                }
            }
        }
        out
    }

    pub fn index_of(&self, name: &str) -> Option<usize> {
        let key = username_key(name);
        self.players.iter().position(|p| username_key(p) == key)
    }

    /// Whether the player is still in (not knocked out).
    pub fn alive(&self, index: usize) -> bool {
        let me = Side::Player { index: index as u16 };
        !self.rounds.iter().flatten().any(|g| g.winner.is_some_and(|w| g.sides[usize::from(1 - w)] == me))
    }

    fn side_view(&self, s: Side) -> BracketSide {
        match s {
            Side::Tbd => BracketSide::Tbd,
            Side::Bye => BracketSide::Bye,
            Side::Player { index } => {
                BracketSide::Player { name: self.players[usize::from(index)].clone(), seed: index + 1 }
            }
        }
    }

    /// The rounds as the protocol shows them (`started`: matches may be
    /// played, so a known pairing is Ready).
    pub fn rounds_view(&self, started: bool) -> Vec<Vec<BracketGame>> {
        self.rounds
            .iter()
            .map(|games| {
                games
                    .iter()
                    .map(|g| BracketGame {
                        sides: g.sides.map(|s| self.side_view(s)),
                        winner: g.winner,
                        how: g.how,
                        state: if g.winner.is_some() {
                            GameState::Done
                        } else if g.match_id.is_some() {
                            GameState::Playing
                        } else if started && g.open() {
                            GameState::Ready
                        } else {
                            GameState::Waiting
                        },
                    })
                    .collect()
            })
            .collect()
    }
}

/// A player signed up.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    /// Unix seconds.
    pub signed_up: i64,
    pub checked_in: bool,
    /// Withdrew after the draw, or removed by an admin: their matches are lost.
    pub out: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tournament {
    pub id: u64,
    pub name: String,
    pub creator: Option<String>,
    pub weekly: bool,
    pub quick: bool,
    /// Unix seconds (a quick tournament: 0 until it fills).
    pub starts: i64,
    pub cap: u16,
    pub state: TournamentState,
    /// In sign-up order.
    pub entries: Vec<Entry>,
    pub bracket: Option<Bracket>,
    pub champion: Option<String>,
    pub created: i64,
    pub finished: Option<i64>,
    pub note: Option<String>,
}

/// Whether a player may create a tournament of `name` starting at `starts`
/// for `cap` players now (`filter`: the server's name filter). The hub
/// checks the account's own limits (one finished match, one open
/// tournament) besides.
pub fn check_create(name: &str, starts: i64, cap: u16, now: i64, filter: impl Fn(&str) -> bool) -> Result<(), LobbyProblem> {
    let words_ok = name.split(' ').all(&filter) && filter(&name.replace(' ', ""));
    let ok = check_tournament_name(name)
        && words_ok
        && (TOURNAMENT_CAP_MIN..=TOURNAMENT_CAP_MAX).contains(&cap)
        && starts >= now + CREATE_MIN_LEAD_SECS
        && starts <= now + CREATE_MAX_AHEAD_SECS;
    if ok { Ok(()) } else { Err(LobbyProblem::BadTournament) }
}

impl Tournament {
    pub fn new(name: &str, creator: Option<String>, starts: i64, cap: u16, weekly: bool, now: i64) -> Self {
        Self {
            id: 0,
            name: name.into(),
            creator,
            weekly,
            quick: false,
            starts,
            cap,
            state: TournamentState::Open,
            entries: Vec::new(),
            bracket: None,
            champion: None,
            created: now,
            finished: None,
            note: None,
        }
    }

    /// A fresh quick tournament (open until its last place is taken).
    pub fn quick(now: i64) -> Self {
        Self { quick: true, ..Self::new(QUICK_NAME, None, 0, QUICK_PLAYERS, false, now) }
    }

    /// Open, drawn or running.
    pub fn live(&self) -> bool {
        matches!(self.state, TournamentState::Open | TournamentState::Drawn | TournamentState::Running)
    }

    /// A quick tournament that has filled and is checking in.
    pub fn filled(&self) -> bool {
        self.quick && self.starts > 0
    }

    /// When the draw is made (Unix seconds; none: a quick one not filled).
    pub fn draw_at(&self) -> Option<i64> {
        match (self.quick, self.starts) {
            (true, 0) => None,
            (true, s) => Some(s),
            (false, s) => Some(s - DRAW_SECS),
        }
    }

    /// When check-in opens.
    pub fn check_in_opens(&self) -> Option<i64> {
        if self.quick {
            (self.starts > 0).then(|| self.starts - QUICK_CHECK_IN_SECS)
        } else {
            Some(self.starts - CHECK_IN_OPENS_SECS)
        }
    }

    pub fn sign_up_open(&self, now: i64) -> bool {
        self.state == TournamentState::Open && !self.filled() && self.draw_at().is_none_or(|d| now < d)
    }

    pub fn check_in_open(&self, now: i64) -> bool {
        self.state == TournamentState::Open
            && self.check_in_opens().is_some_and(|o| now >= o)
            && self.draw_at().is_some_and(|d| now < d)
    }

    pub fn entry(&self, name: &str) -> Option<&Entry> {
        let key = username_key(name);
        self.entries.iter().find(|e| username_key(&e.name) == key)
    }

    fn entry_mut(&mut self, name: &str) -> Option<&mut Entry> {
        let key = username_key(name);
        self.entries.iter_mut().find(|e| username_key(&e.name) == key)
    }

    /// Sign `name` up. A quick tournament that this fills starts its
    /// check-in: returns true then.
    pub fn sign_up(&mut self, name: &str, now: i64) -> Result<bool, LobbyProblem> {
        if !self.sign_up_open(now) {
            return Err(LobbyProblem::TournamentClosed);
        }
        if self.entry(name).is_some() {
            return Ok(false);
        }
        if self.entries.len() >= usize::from(self.cap) {
            return Err(LobbyProblem::TournamentFull);
        }
        self.entries.push(Entry { name: name.into(), signed_up: now, checked_in: false, out: false });
        if self.quick && self.entries.len() >= usize::from(self.cap) {
            self.starts = now + QUICK_CHECK_IN_SECS;
            return Ok(true);
        }
        Ok(false)
    }

    /// `name` leaves: before the draw the entry goes; after it they are out
    /// and lose their next match.
    pub fn withdraw(&mut self, name: &str, now: i64) -> Result<(), LobbyProblem> {
        let _ = now;
        if self.entry(name).is_none() {
            return Err(LobbyProblem::NotEntered);
        }
        match self.state {
            TournamentState::Open if self.filled() => {
                // A filled quick tournament: leaving is not checking in.
                if let Some(e) = self.entry_mut(name) {
                    e.checked_in = false;
                    e.out = true;
                }
                Ok(())
            }
            TournamentState::Open => {
                let key = username_key(name);
                self.entries.retain(|e| username_key(&e.name) != key);
                Ok(())
            }
            TournamentState::Drawn | TournamentState::Running => {
                if let Some(e) = self.entry_mut(name) {
                    e.out = true;
                }
                Ok(())
            }
            _ => Err(LobbyProblem::TournamentClosed),
        }
    }

    pub fn check_in(&mut self, name: &str, now: i64) -> Result<(), LobbyProblem> {
        if self.entry(name).is_none_or(|e| e.out) {
            return Err(LobbyProblem::NotEntered);
        }
        if !self.check_in_open(now) {
            return Err(LobbyProblem::NotCheckInTime);
        }
        if let Some(e) = self.entry_mut(name) {
            e.checked_in = true;
        }
        Ok(())
    }

    /// The draw: the checked-in players, shuffled with `seed`. Too few:
    /// cancelled. Returns the players drawn.
    pub fn make_draw(&mut self, seed: u64) -> usize {
        let players: Vec<String> = self.entries.iter().filter(|e| e.checked_in && !e.out).map(|e| e.name.clone()).collect();
        if players.len() < MIN_PLAYERS {
            self.state = TournamentState::Cancelled;
            self.note = Some(format!("not enough players checked in ({})", players.len()));
            return players.len();
        }
        self.bracket = Some(Bracket::draw(&players, seed));
        self.state = TournamentState::Drawn;
        players.len()
    }

    /// The bracket to show: the drawn one, or a preview of the players
    /// signed up so far in sign-up order (and a preview of byes).
    pub fn shown_bracket(&self) -> (Bracket, bool) {
        match &self.bracket {
            Some(b) => (b.clone(), false),
            None => (Bracket::seeded(self.entries.iter().filter(|e| !e.out).map(|e| e.name.clone()).collect()), true),
        }
    }

    pub fn status_of(&self, name: &str) -> Option<EntryStatus> {
        let e = self.entry(name)?;
        if let Some(c) = &self.champion {
            if username_key(c) == username_key(name) {
                return Some(EntryStatus::Champion);
            }
        }
        Some(match &self.bracket {
            None if e.out => EntryStatus::Out,
            None if e.checked_in => EntryStatus::CheckedIn,
            None => EntryStatus::SignedUp,
            Some(b) => match b.index_of(name) {
                Some(i) if b.alive(i) && !e.out && self.live() => EntryStatus::InBracket,
                _ => EntryStatus::Out,
            },
        })
    }

    /// The tournament as `viewer` sees it in a list (none: for everyone).
    pub fn info(&self, viewer: Option<&str>) -> TournamentInfo {
        TournamentInfo {
            id: self.id,
            name: self.name.clone(),
            creator: self.creator.clone(),
            weekly: self.weekly,
            quick: self.quick,
            starts: self.starts,
            cap: self.cap,
            state: self.state,
            entrants: self.entries.iter().filter(|e| !e.out || self.bracket.is_some()).count() as u16,
            checked_in: self.entries.iter().filter(|e| e.checked_in && !e.out).count() as u16,
            you: viewer.and_then(|v| self.status_of(v)),
            champion: self.champion.clone(),
            note: self.note.clone(),
        }
    }

    /// The bracket view (a preview before the draw).
    pub fn view(&self, viewer: Option<&str>) -> BracketView {
        let (b, preview) = self.shown_bracket();
        BracketView {
            tournament: self.info(viewer),
            preview,
            size: b.size() as u16,
            rounds: b.rounds_view(matches!(self.state, TournamentState::Running)),
        }
    }
}

/// The next weekly start at or after `from` (Unix seconds): `weekday`
/// (0 = Monday) at `minute` past midnight UTC.
pub fn next_weekly(from: i64, weekday: u8, minute: u16) -> i64 {
    let day = from.div_euclid(86_400);
    // 1970-01-01 was a Thursday (weekday 3).
    let today = (day + 3).rem_euclid(7);
    let mut t = (day + (i64::from(weekday) - today).rem_euclid(7)) * 86_400 + i64::from(minute) * 60;
    if t < from {
        t += 7 * 86_400;
    }
    t
}

/// Unix seconds of `YYYY-MM-DD hh:mm` (UTC), or of a plain number.
pub fn parse_utc(text: &str) -> Option<i64> {
    let text = text.trim();
    if let Ok(n) = text.parse::<i64>() {
        return Some(n);
    }
    let (date, time) = text.split_once([' ', 'T'])?;
    let mut d = date.split('-').map(|p| p.parse::<i64>().ok());
    let (y, m, dd) = (d.next()??, d.next()??, d.next()??);
    let mut t = time.trim_end_matches('Z').split(':').map(|p| p.parse::<i64>().ok());
    let (h, mi) = (t.next()??, t.next()??);
    if !(1..=12).contains(&m) || !(1..=31).contains(&dd) || !(0..24).contains(&h) || !(0..60).contains(&mi) {
        return None;
    }
    // Days from civil (Howard Hinnant's).
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + dd - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3600 + mi * 60)
}

#[cfg(test)]
#[path = "tournament_tests.rs"]
mod tests;
