//! The lobby as a pure state machine (no sockets, no clock of its own):
//! who is online, challenges between them, private rooms by code and
//! requests to play the computer. [`crate::hub::Hub`] feeds it signed-in
//! players' requests and turns its [`Action`]s into messages and matches.

use roses_net::lobby::{
    normalize_code, username_key, ChallengeEnd, ComputerDeck, LobbyEvent, LobbyProblem, LobbyRequest, MapChoice, PlayerEntry,
    PlayerState, RoomInfo, RoomMode, RoomSeat, RoomSettings, ROOM_CODE_LEN,
};
use std::collections::BTreeMap;

/// A lobby connection's id (the hub's).
pub type ConnId = u64;

/// Challenges expire after this long unanswered (ms).
pub const CHALLENGE_MS: u64 = 60_000;
/// Rooms close after this long without a guest (ms).
pub const ROOM_MS: u64 = 30 * 60_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Send(ConnId, LobbyEvent),
    /// Start a match between these two players (display names) on the
    /// board the challenger or the room's host chose.
    Pair([String; 2], Option<MapChoice>),
    /// As [`Action::Pair`] for a private room's host and guest: the match is
    /// not listed for spectators and cannot be watched.
    PairPrivate([String; 2], Option<MapChoice>),
    /// Start a match between this player and the server's computer, on
    /// their board, with their deck for the computer.
    PairComputer(String, Option<MapChoice>, Option<ComputerDeck>),
    /// MODES (a port addition): start an N-player match from a room: the
    /// players by seat (none: the server's computer) and the settings.
    StartMulti(Vec<Option<String>>, RoomSettings),
    /// Who is online changed: send everyone [`Lobby::list`]. The hub
    /// coalesces these (one list per pass, spaced out as the lobby grows),
    /// since every list goes to every player.
    Players,
}

#[derive(Clone, Debug)]
struct Present {
    name: String,
    conn: ConnId,
    room: Option<String>,
    in_match: bool,
    /// MODES: the N-player room this player sits in.
    mode_room: Option<String>,
}

/// Seats of an N-player room.
pub const MODE_SEATS: usize = 4;

/// MODES (a port addition): an N-player room.
#[derive(Clone, Debug)]
struct ModeRoom {
    host: String,
    at: u64,
    settings: RoomSettings,
    /// By seat: the player sitting there.
    players: [Option<String>; MODE_SEATS],
    /// By seat: the server's computer plays it.
    computer: [bool; MODE_SEATS],
}

impl ModeRoom {
    fn team(&self, seat: usize) -> u8 {
        match self.settings.mode {
            RoomMode::Teams => (seat / 2) as u8,
            RoomMode::FreeForAll => seat as u8,
        }
    }

    fn info(&self, code: &str) -> RoomInfo {
        RoomInfo {
            code: code.to_string(),
            host: self.host.clone(),
            settings: self.settings.clone(),
            seats: (0..MODE_SEATS)
                .map(|s| RoomSeat { player: self.players[s].clone(), computer: self.computer[s], team: self.team(s) })
                .collect(),
        }
    }

    fn seat_of(&self, key: &str) -> Option<usize> {
        self.players.iter().position(|p| p.as_deref().is_some_and(|n| username_key(n) == key))
    }

    fn members(&self) -> Vec<String> {
        self.players.iter().flatten().cloned().collect()
    }
}

#[derive(Clone, Debug)]
struct Challenge {
    from: String,
    to: String,
    at: u64,
    map: Option<MapChoice>,
}

#[derive(Clone, Debug)]
struct Room {
    host: String,
    at: u64,
    map: Option<MapChoice>,
}

#[derive(Debug, Default)]
pub struct Lobby {
    /// By name key.
    players: BTreeMap<String, Present>,
    /// Players in a match whose lobby connection is gone (name key → name):
    /// still shown busy, so nobody challenges them.
    away_in_match: BTreeMap<String, String>,
    challenges: BTreeMap<u64, Challenge>,
    rooms: BTreeMap<String, Room>,
    /// MODES: N-player rooms by code.
    mode_rooms: BTreeMap<String, ModeRoom>,
    next_id: u64,
    /// Whether [`LobbyRequest::PlayComputer`] is offered.
    pub computer: bool,
}

impl Lobby {
    pub fn new(computer: bool) -> Self {
        Self { computer, next_id: 1, ..Self::default() }
    }

    pub fn is_online(&self, name: &str) -> bool {
        self.players.contains_key(&username_key(name))
    }

    pub fn conn_of(&self, name: &str) -> Option<ConnId> {
        self.players.get(&username_key(name)).map(|p| p.conn)
    }

    pub fn in_match(&self, name: &str) -> bool {
        let key = username_key(name);
        self.players.get(&key).is_some_and(|p| p.in_match) || self.away_in_match.contains_key(&key)
    }

    fn state(p: &Present) -> PlayerState {
        if p.in_match {
            PlayerState::InMatch
        } else if p.room.is_some() || p.mode_room.is_some() {
            PlayerState::InRoom
        } else {
            PlayerState::Idle
        }
    }

    /// Everyone online, as the list event.
    pub fn list(&self) -> LobbyEvent {
        let mut players: Vec<PlayerEntry> =
            self.players.values().map(|p| PlayerEntry { name: p.name.clone(), state: Self::state(p) }).collect();
        players.extend(
            self.away_in_match
                .values()
                .map(|name| PlayerEntry { name: name.clone(), state: PlayerState::InMatch }),
        );
        players.sort_by_key(|p| username_key(&p.name));
        LobbyEvent::Players { players }
    }

    fn broadcast(&self, out: &mut Vec<Action>) {
        if !out.contains(&Action::Players) {
            out.push(Action::Players);
        }
    }

    /// Every player's lobby connection (for [`Action::Players`]).
    pub fn conns(&self) -> Vec<ConnId> {
        self.players.values().map(|p| p.conn).collect()
    }

    /// Players online (in the lobby).
    pub fn online(&self) -> usize {
        self.players.len()
    }

    fn tell(&self, name: &str, event: LobbyEvent, out: &mut Vec<Action>) {
        if let Some(c) = self.conn_of(name) {
            out.push(Action::Send(c, event));
        }
    }

    /// A signed-in player's lobby connection arrives (`in_match`: the hub
    /// knows a running match of theirs).
    pub fn enter(&mut self, name: &str, conn: ConnId, in_match: bool) -> Vec<Action> {
        let mut out = Vec::new();
        let key = username_key(name);
        self.away_in_match.remove(&key);
        self.players.insert(key, Present { name: name.into(), conn, room: None, in_match, mode_room: None });
        // The newcomer sees the list at once; the others with the next one.
        out.push(Action::Send(conn, self.list()));
        self.broadcast(&mut out);
        out
    }

    /// A player's lobby connection is gone (log-out, disconnect): their
    /// challenges and room close.
    pub fn leave(&mut self, name: &str) -> Vec<Action> {
        let mut out = Vec::new();
        let key = username_key(name);
        let Some(p) = self.players.remove(&key) else { return out };
        if p.in_match {
            self.away_in_match.insert(key.clone(), p.name.clone());
        }
        self.close_challenges_of(&key, ChallengeEnd::Gone, &mut out);
        if let Some(code) = p.room {
            self.rooms.remove(&code);
        }
        if let Some(code) = p.mode_room {
            self.leave_mode_room(&code, &key, &mut out);
        }
        self.broadcast(&mut out);
        out
    }

    /// MODES: `key` leaves the room `code` (the host closes it).
    fn leave_mode_room(&mut self, code: &str, key: &str, out: &mut Vec<Action>) {
        let Some(room) = self.mode_rooms.get_mut(code) else { return };
        if username_key(&room.host) == key {
            let room = self.mode_rooms.remove(code).expect("present");
            for name in room.members() {
                if let Some(p) = self.players.get_mut(&username_key(&name)) {
                    p.mode_room = None;
                }
                self.tell(&name, LobbyEvent::ModeRoomClosed, out);
            }
            return;
        }
        if let Some(seat) = room.seat_of(key) {
            room.players[seat] = None;
        }
        if let Some(p) = self.players.get_mut(key) {
            p.mode_room = None;
            let name = p.name.clone();
            self.tell(&name, LobbyEvent::ModeRoomClosed, out);
        }
        self.tell_room(code, out);
    }

    /// MODES: every member of the room gets its state.
    fn tell_room(&self, code: &str, out: &mut Vec<Action>) {
        let Some(room) = self.mode_rooms.get(code) else { return };
        let info = room.info(code);
        for name in room.members() {
            self.tell(&name, LobbyEvent::ModeRoom { room: info.clone() }, out);
        }
    }

    fn new_code(&self) -> String {
        loop {
            let c = crate::accounts::random_code(ROOM_CODE_LEN);
            if !self.rooms.contains_key(&c) && !self.mode_rooms.contains_key(&c) {
                break c;
            }
        }
    }

    /// MODES requests (the hub checked the settings).
    fn mode_request(&mut self, name: &str, request: &LobbyRequest, now: u64, out: &mut Vec<Action>) {
        let key = username_key(name);
        let problem = |p: LobbyProblem| LobbyEvent::Problem { problem: p };
        let mine = self.players.get(&key).and_then(|p| p.mode_room.clone());
        let host_of = |lobby: &Self, code: &str| lobby.mode_rooms.get(code).is_some_and(|r| username_key(&r.host) == key);
        match request {
            LobbyRequest::CreateModeRoom { settings } => {
                if !self.idle(&key) || self.challenging(&key) {
                    return self.tell(name, problem(LobbyProblem::YouAreBusy), out);
                }
                let code = self.new_code();
                let me = self.players[&key].name.clone();
                let mut players: [Option<String>; MODE_SEATS] = Default::default();
                players[0] = Some(me);
                let room = ModeRoom {
                    host: self.players[&key].name.clone(),
                    at: now,
                    settings: settings.clone(),
                    players,
                    computer: [false; MODE_SEATS],
                };
                self.mode_rooms.insert(code.clone(), room);
                self.players.get_mut(&key).expect("online").mode_room = Some(code.clone());
                self.tell_room(&code, out);
                self.broadcast(out);
            }
            LobbyRequest::JoinModeRoom { code } => {
                let code = normalize_code(code);
                if !self.idle(&key) || self.challenging(&key) {
                    return self.tell(name, problem(LobbyProblem::YouAreBusy), out);
                }
                let Some(room) = self.mode_rooms.get_mut(&code) else {
                    return self.tell(name, problem(LobbyProblem::BadRoomCode), out);
                };
                let Some(seat) = (0..MODE_SEATS).find(|&s| room.players[s].is_none() && !room.computer[s]) else {
                    return self.tell(name, problem(LobbyProblem::SeatTaken), out);
                };
                room.players[seat] = Some(self.players[&key].name.clone());
                room.at = now;
                self.players.get_mut(&key).expect("online").mode_room = Some(code.clone());
                self.tell_room(&code, out);
                self.broadcast(out);
            }
            LobbyRequest::ModeRoomSeat { seat } => {
                let Some(code) = mine else { return self.tell(name, problem(LobbyProblem::BadRoomCode), out) };
                let room = self.mode_rooms.get_mut(&code).expect("a member's room");
                let s = usize::from(*seat);
                if s >= MODE_SEATS || room.players[s].is_some() || room.computer[s] {
                    return self.tell(name, problem(LobbyProblem::SeatTaken), out);
                }
                if let Some(from) = room.seat_of(&key) {
                    room.players[s] = room.players[from].take();
                }
                self.tell_room(&code, out);
            }
            LobbyRequest::ModeRoomComputer { seat, computer } => {
                let Some(code) = mine else { return self.tell(name, problem(LobbyProblem::BadRoomCode), out) };
                if !host_of(self, &code) {
                    return self.tell(name, problem(LobbyProblem::NotHost), out);
                }
                let room = self.mode_rooms.get_mut(&code).expect("a member's room");
                let s = usize::from(*seat);
                if s >= MODE_SEATS || room.players[s].is_some() {
                    return self.tell(name, problem(LobbyProblem::SeatTaken), out);
                }
                room.computer[s] = *computer;
                self.tell_room(&code, out);
            }
            LobbyRequest::ModeRoomSettings { settings } => {
                let Some(code) = mine else { return self.tell(name, problem(LobbyProblem::BadRoomCode), out) };
                if !host_of(self, &code) {
                    return self.tell(name, problem(LobbyProblem::NotHost), out);
                }
                self.mode_rooms.get_mut(&code).expect("a member's room").settings = settings.clone();
                self.tell_room(&code, out);
            }
            LobbyRequest::StartModeRoom => {
                let Some(code) = mine else { return self.tell(name, problem(LobbyProblem::BadRoomCode), out) };
                if !host_of(self, &code) {
                    return self.tell(name, problem(LobbyProblem::NotHost), out);
                }
                let room = self.mode_rooms.remove(&code).expect("a member's room");
                for member in room.members() {
                    if let Some(p) = self.players.get_mut(&username_key(&member)) {
                        p.mode_room = None;
                    }
                    self.tell(&member, LobbyEvent::ModeRoomClosed, out);
                }
                out.push(Action::StartMulti(room.players.to_vec(), room.settings));
                self.broadcast(out);
            }
            LobbyRequest::LeaveRoom => {
                if let Some(code) = mine {
                    self.leave_mode_room(&code, &key, out);
                    self.broadcast(out);
                }
            }
            _ => {}
        }
    }

    fn close_challenges_of(&mut self, key: &str, end: ChallengeEnd, out: &mut Vec<Action>) {
        let ids: Vec<u64> = self
            .challenges
            .iter()
            .filter(|(_, c)| username_key(&c.from) == key || username_key(&c.to) == key)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let c = self.challenges.remove(&id).expect("listed");
            for who in [&c.from, &c.to] {
                self.tell(who, LobbyEvent::ChallengeClosed { id, end }, out);
            }
        }
    }

    fn idle(&self, key: &str) -> bool {
        self.players.get(key).is_some_and(|p| !p.in_match && p.room.is_none() && p.mode_room.is_none())
    }

    fn challenging(&self, key: &str) -> bool {
        self.challenges.values().any(|c| username_key(&c.from) == key)
    }

    /// The hub started a match for these players.
    pub fn started(&mut self, names: &[&str]) -> Vec<Action> {
        let mut out = Vec::new();
        for name in names {
            let key = username_key(name);
            self.close_challenges_of(&key, ChallengeEnd::Gone, &mut out);
            if let Some(p) = self.players.get_mut(&key) {
                if let Some(code) = p.room.take() {
                    self.rooms.remove(&code);
                }
                p.mode_room = None;
                p.in_match = true;
            } else {
                self.away_in_match.insert(key, (*name).to_string());
            }
        }
        self.broadcast(&mut out);
        out
    }

    /// A match of these players is over.
    pub fn ended(&mut self, names: &[&str]) -> Vec<Action> {
        let mut out = Vec::new();
        for name in names {
            let key = username_key(name);
            self.away_in_match.remove(&key);
            if let Some(p) = self.players.get_mut(&key) {
                p.in_match = false;
            }
        }
        self.broadcast(&mut out);
        out
    }

    /// A signed-in player's request (account requests are the hub's).
    pub fn request(&mut self, name: &str, request: &LobbyRequest, now: u64) -> Vec<Action> {
        let mut out = Vec::new();
        let key = username_key(name);
        let problem = |p: LobbyProblem| LobbyEvent::Problem { problem: p };
        match request {
            LobbyRequest::Challenge { to, map } => {
                let to_key = username_key(to);
                if to_key == key {
                    self.tell(name, problem(LobbyProblem::NotYourself), &mut out);
                } else if !self.idle(&key) || self.challenging(&key) {
                    self.tell(name, problem(LobbyProblem::YouAreBusy), &mut out);
                } else if !self.players.contains_key(&to_key) {
                    self.tell(name, problem(LobbyProblem::UnknownPlayer), &mut out);
                } else if !self.idle(&to_key) {
                    self.tell(name, problem(LobbyProblem::PlayerBusy), &mut out);
                } else {
                    let id = self.next_id;
                    self.next_id += 1;
                    let (from, to) = (self.players[&key].name.clone(), self.players[&to_key].name.clone());
                    self.challenges.insert(id, Challenge { from: from.clone(), to: to.clone(), at: now, map: map.clone() });
                    self.tell(&from, LobbyEvent::ChallengeSent { id, to: to.clone() }, &mut out);
                    self.tell(&to, LobbyEvent::Challenged { id, from, map: map.clone() }, &mut out);
                }
            }
            LobbyRequest::CancelChallenge { id } => match self.challenges.get(id) {
                Some(c) if username_key(&c.from) == key => {
                    let c = self.challenges.remove(id).expect("present");
                    for who in [&c.from, &c.to] {
                        self.tell(who, LobbyEvent::ChallengeClosed { id: *id, end: ChallengeEnd::Cancelled }, &mut out);
                    }
                }
                _ => self.tell(name, problem(LobbyProblem::NoSuchChallenge), &mut out),
            },
            LobbyRequest::Answer { id, accept } => match self.challenges.get(id) {
                Some(c) if username_key(&c.to) == key => {
                    let c = self.challenges.remove(id).expect("present");
                    let from_key = username_key(&c.from);
                    if !*accept {
                        for who in [&c.from, &c.to] {
                            self.tell(who, LobbyEvent::ChallengeClosed { id: *id, end: ChallengeEnd::Declined }, &mut out);
                        }
                    } else if !self.idle(&from_key) || !self.idle(&key) {
                        for who in [&c.from, &c.to] {
                            self.tell(who, LobbyEvent::ChallengeClosed { id: *id, end: ChallengeEnd::Gone }, &mut out);
                        }
                    } else {
                        for who in [&c.from, &c.to] {
                            self.tell(who, LobbyEvent::ChallengeClosed { id: *id, end: ChallengeEnd::Accepted }, &mut out);
                        }
                        out.push(Action::Pair([c.from, c.to], c.map));
                    }
                }
                _ => self.tell(name, problem(LobbyProblem::NoSuchChallenge), &mut out),
            },
            LobbyRequest::CreateRoom { map } => {
                if !self.idle(&key) || self.challenging(&key) {
                    self.tell(name, problem(LobbyProblem::YouAreBusy), &mut out);
                } else {
                    let code = loop {
                        let c = crate::accounts::random_code(ROOM_CODE_LEN);
                        if !self.rooms.contains_key(&c) {
                            break c;
                        }
                    };
                    self.rooms.insert(code.clone(), Room { host: self.players[&key].name.clone(), at: now, map: map.clone() });
                    self.players.get_mut(&key).expect("online").room = Some(code.clone());
                    self.tell(name, LobbyEvent::RoomOpened { code }, &mut out);
                    self.broadcast(&mut out);
                }
            }
            LobbyRequest::JoinRoom { code } => {
                let code = normalize_code(code);
                let host = self.rooms.get(&code).map(|r| (r.host.clone(), r.map.clone()));
                match host {
                    _ if !self.idle(&key) || self.challenging(&key) => {
                        self.tell(name, problem(LobbyProblem::YouAreBusy), &mut out)
                    }
                    Some((host, map)) if username_key(&host) != key => {
                        self.rooms.remove(&code);
                        if let Some(p) = self.players.get_mut(&username_key(&host)) {
                            p.room = None;
                        }
                        self.tell(&host, LobbyEvent::RoomClosed, &mut out);
                        out.push(Action::PairPrivate([host, self.players[&key].name.clone()], map));
                    }
                    _ => self.tell(name, problem(LobbyProblem::BadRoomCode), &mut out),
                }
            }
            LobbyRequest::LeaveRoom if self.players.get(&key).is_some_and(|p| p.mode_room.is_some()) => {
                self.mode_request(name, request, now, &mut out);
            }
            LobbyRequest::LeaveRoom => {
                if let Some(code) = self.players.get_mut(&key).and_then(|p| p.room.take()) {
                    self.rooms.remove(&code);
                    self.tell(name, LobbyEvent::RoomClosed, &mut out);
                    self.broadcast(&mut out);
                }
            }
            LobbyRequest::PlayComputer { map, computer } => {
                if !self.computer {
                    self.tell(name, problem(LobbyProblem::NoComputer), &mut out);
                } else if !self.idle(&key) || self.challenging(&key) {
                    self.tell(name, problem(LobbyProblem::YouAreBusy), &mut out);
                } else {
                    out.push(Action::PairComputer(self.players[&key].name.clone(), map.clone(), computer.clone()));
                }
            }
            LobbyRequest::Ping => self.tell(name, LobbyEvent::Pong, &mut out),
            // Account requests are handled by the hub.
            LobbyRequest::LobbyHello { .. }
            | LobbyRequest::SignUp { .. }
            | LobbyRequest::LogIn { .. }
            | LobbyRequest::ResumeSession { .. }
            | LobbyRequest::LogOut
            | LobbyRequest::ChangePassword { .. }
            // Email requests too (crate::hub's email_hub).
            | LobbyRequest::AccountStatus
            | LobbyRequest::SetEmail { .. }
            | LobbyRequest::VerifyEmail { .. }
            | LobbyRequest::DismissEmailPrompt
            | LobbyRequest::RequestPasswordReset { .. }
            | LobbyRequest::ResetPassword { .. }
            // Spectating is the hub's (it knows the matches).
            | LobbyRequest::ListMatches
            | LobbyRequest::Watch { .. }
            | LobbyRequest::AllowSpectators { .. } => {}
            // N-player rooms (MODES, a port addition).
            LobbyRequest::CreateModeRoom { .. }
            | LobbyRequest::JoinModeRoom { .. }
            | LobbyRequest::ModeRoomSeat { .. }
            | LobbyRequest::ModeRoomComputer { .. }
            | LobbyRequest::ModeRoomSettings { .. }
            | LobbyRequest::StartModeRoom => self.mode_request(name, request, now, &mut out),
        }
        out
    }

    /// Expire old challenges and rooms.
    pub fn tick(&mut self, now: u64) -> Vec<Action> {
        let mut out = Vec::new();
        let old: Vec<u64> =
            self.challenges.iter().filter(|(_, c)| now.saturating_sub(c.at) >= CHALLENGE_MS).map(|(id, _)| *id).collect();
        for id in old {
            let c = self.challenges.remove(&id).expect("listed");
            for who in [&c.from, &c.to] {
                self.tell(who, LobbyEvent::ChallengeClosed { id, end: ChallengeEnd::Expired }, &mut out);
            }
        }
        let old: Vec<String> =
            self.rooms.iter().filter(|(_, r)| now.saturating_sub(r.at) >= ROOM_MS).map(|(c, _)| c.clone()).collect();
        let mut any = !old.is_empty();
        for code in old {
            let r = self.rooms.remove(&code).expect("listed");
            if let Some(p) = self.players.get_mut(&username_key(&r.host)) {
                p.room = None;
            }
            self.tell(&r.host, LobbyEvent::RoomClosed, &mut out);
        }
        let old: Vec<String> =
            self.mode_rooms.iter().filter(|(_, r)| now.saturating_sub(r.at) >= ROOM_MS).map(|(c, _)| c.clone()).collect();
        any |= !old.is_empty();
        for code in old {
            let host = self.mode_rooms[&code].host.clone();
            self.leave_mode_room(&code, &username_key(&host), &mut out);
        }
        if any {
            self.broadcast(&mut out);
        }
        out
    }
}
