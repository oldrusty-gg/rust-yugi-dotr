//! Client side of the friends-online lobby: the server address file
//! (`online.json`), connecting with or without TLS, and [`LobbyClient`],
//! the lobby bookkeeping the game's Online Play screens and the headless
//! tests share.

use crate::lobby::{
    ChallengeEnd, LobbyEvent, LobbyRefusal, LobbyRequest, MatchResult, PlayerEntry, LOBBY_VERSION,
};
use crate::protocol::{Cause, Compat, MatchOptions};
use crate::transport::{Connection, Received};
use std::io;

// The server file, the saved session and connecting moved to
// `platform_net::config` (shared with the launcher); re-exported here.
pub use platform_net::config::{
    config_candidates, connect, find_config, OnlineConfig, SavedSession, CONFIG_FILE, DEFAULT_PORT, SESSION_FILE,
};

pub type LobbyConn = Box<dyn Connection<LobbyRequest, LobbyEvent> + Send>;

/// A match the lobby arranged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchReady {
    pub match_id: String,
    pub seat: u8,
    pub ticket: String,
    pub opponent: String,
    pub options: MatchOptions,
    /// The board (none: the server's default, or a server from before map
    /// choices).
    pub map: Option<crate::lobby::MapChoice>,
    /// The opponent's name to show when it is not `opponent` (the
    /// computer playing a computer opponent's deck).
    pub opponent_title: Option<String>,
}

/// An N-player match the lobby arranged (MODES, a port addition).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MultiMatchReady {
    pub match_id: String,
    pub seat: u8,
    pub ticket: String,
    pub seats: Vec<crate::protocol::SeatInfo>,
    pub settings: crate::lobby::RoomSettings,
}

/// A match the lobby lets this client watch ([`LobbyEvent::WatchReady`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatchReady {
    pub match_id: String,
    pub ticket: String,
    pub players: [String; 2],
    pub map: Option<crate::lobby::MapChoice>,
    pub options: MatchOptions,
}

/// The lobby as this client knows it, updated by [`LobbyClient::pump`].
pub struct LobbyClient {
    conn: Option<LobbyConn>,
    /// The server's name and whether it offers a computer opponent.
    pub server: Option<(String, bool)>,
    /// Whether sign-up needs an invite code (from the welcome; true until
    /// the server says otherwise).
    pub invite_required: bool,
    /// The server takes map and computer-deck choices.
    pub choices: bool,
    /// The server sends email (verification, password reset).
    pub email: bool,
    /// The server lets players watch live matches.
    pub spectate: bool,
    /// The server offers the N-player modes (MODES, a port addition).
    pub modes: bool,
    /// The N-player room this player is in.
    pub mode_room: Option<crate::lobby::RoomInfo>,
    /// The N-player match the lobby arranged.
    pub multi_ready: Option<MultiMatchReady>,
    /// The live matches that may be watched (the last list asked for).
    pub live: Option<Vec<crate::lobby::LiveMatch>>,
    /// The match the server let this client watch.
    pub watch: Option<WatchReady>,
    /// The account's email state (the answer to `AccountStatus`).
    pub account_status: Option<crate::lobby::AccountStatus>,
    /// The server confirmed a password change.
    pub password_changed: bool,
    pub refused: Option<LobbyRefusal>,
    pub user: Option<String>,
    pub session: Option<String>,
    pub players: Vec<PlayerEntry>,
    /// Challenges to this player (id, from).
    pub incoming: Vec<(u64, String)>,
    /// This player's open challenge (id, to).
    pub outgoing: Option<(u64, String)>,
    pub room: Option<String>,
    pub ready: Option<MatchReady>,
    pub over: Option<(String, MatchResult, Option<Cause>)>,
    /// The last thing worth telling the player (problems, closed challenges).
    pub note: Option<String>,
    /// The last authentication problem (cleared by the next request).
    pub auth_problem: Option<crate::lobby::AuthProblem>,
}

impl LobbyClient {
    /// Say hello on a fresh lobby connection.
    pub fn start(conn: LobbyConn, compat: Compat, client: &str) -> io::Result<Self> {
        Self::hello(conn, compat, client, false)
    }

    /// Say hello on a fresh account-only connection (the launcher's
    /// Profile page: no rules data needed, never enters the lobby).
    pub fn start_account(conn: LobbyConn, client: &str) -> io::Result<Self> {
        Self::hello(conn, Compat::current(""), client, true)
    }

    /// Say hello as a client from before MODES (tests of older clients:
    /// no `modes` in the hello).
    #[doc(hidden)]
    pub fn start_older(conn: LobbyConn, compat: Compat, client: &str) -> io::Result<Self> {
        Self::hello_with(conn, compat, client, false, false)
    }

    fn hello(conn: LobbyConn, compat: Compat, client: &str, account: bool) -> io::Result<Self> {
        // A game client plays the N-player modes; the launcher's account
        // page does not ask for them.
        Self::hello_with(conn, compat, client, account, !account)
    }

    fn hello_with(mut conn: LobbyConn, compat: Compat, client: &str, account: bool, modes: bool) -> io::Result<Self> {
        conn.send(&LobbyRequest::LobbyHello { lobby_version: LOBBY_VERSION, compat, client: client.into(), account, modes })?;
        Ok(Self {
            conn: Some(conn),
            server: None,
            invite_required: true,
            choices: false,
            email: false,
            spectate: false,
            modes: false,
            mode_room: None,
            multi_ready: None,
            live: None,
            watch: None,
            account_status: None,
            password_changed: false,
            refused: None,
            user: None,
            session: None,
            players: Vec::new(),
            incoming: Vec::new(),
            outgoing: None,
            room: None,
            ready: None,
            over: None,
            note: None,
            auth_problem: None,
        })
    }

    pub fn connected(&self) -> bool {
        self.conn.is_some()
    }

    pub fn send(&mut self, request: LobbyRequest) {
        if matches!(
            request,
            LobbyRequest::SignUp { .. }
                | LobbyRequest::LogIn { .. }
                | LobbyRequest::ResumeSession { .. }
                | LobbyRequest::ChangePassword { .. }
        ) {
            self.auth_problem = None;
            self.password_changed = false;
        }
        let Some(conn) = self.conn.as_mut() else { return };
        if conn.send(&request).is_err() {
            self.close("the connection to the server broke");
        }
    }

    pub fn close(&mut self, why: &str) {
        if let Some(mut c) = self.conn.take() {
            c.close();
        }
        self.note = Some(why.into());
    }

    /// Read everything waiting; returns the events in order.
    pub fn pump(&mut self) -> Vec<LobbyEvent> {
        let mut out = Vec::new();
        loop {
            let Some(conn) = self.conn.as_mut() else { break };
            match conn.poll() {
                Received::Message(e) => {
                    self.apply(&e);
                    out.push(e);
                }
                Received::Empty => break,
                Received::Closed => {
                    // A refusal said why; the close that follows it is not
                    // news (the game's version, a full server).
                    let why = self.refused.is_some().then(|| self.note.take()).flatten();
                    self.close("the server closed the connection");
                    if why.is_some() {
                        self.note = why;
                    }
                    break;
                }
                Received::Malformed(m) => {
                    self.close(&format!("the server sent something unreadable: {m}"));
                    break;
                }
            }
        }
        out
    }

    fn apply(&mut self, e: &LobbyEvent) {
        match e {
            LobbyEvent::LobbyWelcome { server, computer, invite_required, choices, email, spectate, modes, .. } => {
                self.email = *email;
                self.spectate = *spectate;
                self.modes = *modes;
                self.server = Some((server.clone(), *computer));
                self.invite_required = *invite_required;
                self.choices = *choices;
            }
            LobbyEvent::PasswordChanged => {
                self.password_changed = true;
                self.auth_problem = None;
                self.note = Some("password changed".into());
            }
            LobbyEvent::LobbyRefused { reason } => {
                self.refused = Some(reason.clone());
                self.note = Some(match reason {
                    LobbyRefusal::LobbyVersion { .. } | LobbyRefusal::Incompatible { .. } => {
                        "this game version does not match the server's; update the game".into()
                    }
                    LobbyRefusal::ServerFull => "the server is full".into(),
                    LobbyRefusal::ExpectedHello => "the server did not understand this client".into(),
                });
            }
            LobbyEvent::SignedIn { username, session } => {
                self.user = Some(username.clone());
                self.session = Some(session.clone());
                self.auth_problem = None;
                self.note = None;
            }
            LobbyEvent::AuthFailed { problem } => {
                self.auth_problem = Some(*problem);
                self.note = Some(problem.to_string());
            }
            LobbyEvent::SignedOut { elsewhere } => {
                self.user = None;
                self.session = None;
                self.incoming.clear();
                self.outgoing = None;
                self.room = None;
                self.mode_room = None;
                self.note = Some(if *elsewhere { "signed in from another place".into() } else { "logged out".into() });
            }
            LobbyEvent::Players { players } => self.players = players.clone(),
            LobbyEvent::Challenged { id, from, .. } => {
                self.incoming.retain(|(i, _)| i != id);
                self.incoming.push((*id, from.clone()));
            }
            LobbyEvent::ChallengeSent { id, to } => self.outgoing = Some((*id, to.clone())),
            LobbyEvent::ChallengeClosed { id, end } => {
                let who = self
                    .incoming
                    .iter()
                    .find(|(i, _)| i == id)
                    .map(|(_, n)| n.clone())
                    .or_else(|| self.outgoing.as_ref().filter(|(i, _)| i == id).map(|(_, n)| n.clone()));
                self.incoming.retain(|(i, _)| i != id);
                if self.outgoing.as_ref().is_some_and(|(i, _)| i == id) {
                    self.outgoing = None;
                }
                if let Some(who) = who {
                    self.note = match end {
                        ChallengeEnd::Accepted => None,
                        ChallengeEnd::Declined => Some(format!("{who} declined")),
                        ChallengeEnd::Cancelled => Some(format!("{who}'s challenge was cancelled")),
                        ChallengeEnd::Expired => Some(format!("the challenge with {who} expired")),
                        ChallengeEnd::Gone => Some(format!("{who} is no longer available")),
                    };
                }
            }
            LobbyEvent::RoomOpened { code } => self.room = Some(code.clone()),
            LobbyEvent::RoomClosed => self.room = None,
            LobbyEvent::MatchReady { match_id, seat, ticket, opponent, options, map, opponent_title } => {
                self.room = None;
                self.outgoing = None;
                self.incoming.clear();
                self.over = None;
                self.ready = Some(MatchReady {
                    match_id: match_id.clone(),
                    seat: *seat,
                    ticket: ticket.clone(),
                    opponent: opponent.clone(),
                    options: *options,
                    map: map.clone(),
                    opponent_title: opponent_title.clone(),
                });
            }
            LobbyEvent::MatchOver { match_id, result, cause } => {
                self.over = Some((match_id.clone(), *result, *cause));
                if self.ready.as_ref().is_some_and(|r| &r.match_id == match_id) {
                    self.ready = None;
                }
                if self.multi_ready.as_ref().is_some_and(|r| &r.match_id == match_id) {
                    self.multi_ready = None;
                }
            }
            LobbyEvent::Problem { problem } => self.note = Some(problem.to_string()),
            LobbyEvent::Pong => {}
            LobbyEvent::LiveMatches { matches } => self.live = Some(matches.clone()),
            LobbyEvent::WatchReady { match_id, ticket, players, map, options } => {
                self.watch = Some(WatchReady {
                    match_id: match_id.clone(),
                    ticket: ticket.clone(),
                    players: players.clone(),
                    map: map.clone(),
                    options: *options,
                });
            }
            LobbyEvent::ModeRoom { room } => self.mode_room = Some(room.clone()),
            LobbyEvent::ModeRoomClosed => self.mode_room = None,
            LobbyEvent::MultiMatchReady { match_id, seat, ticket, seats, settings } => {
                self.mode_room = None;
                self.outgoing = None;
                self.incoming.clear();
                self.over = None;
                self.multi_ready = Some(MultiMatchReady {
                    match_id: match_id.clone(),
                    seat: *seat,
                    ticket: ticket.clone(),
                    seats: seats.clone(),
                    settings: settings.clone(),
                });
            }
            LobbyEvent::AccountStatus { status } => self.account_status = Some(status.clone()),
            LobbyEvent::EmailFailed { problem } => self.note = Some(problem.to_string()),
            LobbyEvent::EmailCodeSent | LobbyEvent::ResetCodeSent | LobbyEvent::PasswordReset => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lobby::{check_password, check_username, normalize_code, ClientFrame, ServerFrame};

    // The server file and session tests moved with them to
    // `platform_net::config`.

    #[test]
    fn a_refusal_keeps_its_reason_when_the_server_closes_after_it() {
        use crate::transport::{memory_pair, Connection};
        let (client, mut server) = memory_pair::<LobbyRequest, LobbyEvent>();
        let mut c = LobbyClient::start(Box::new(client), Compat::current("x"), "test").unwrap();
        server.send(&LobbyEvent::LobbyRefused { reason: LobbyRefusal::ServerFull }).unwrap();
        server.close();
        let events = c.pump();
        assert!(matches!(events[..], [LobbyEvent::LobbyRefused { .. }]));
        assert!(!c.connected());
        assert_eq!(c.note.as_deref(), Some("the server is full"));
    }

    #[test]
    fn new_names_skip_reserved_ones() {
        use crate::lobby::{check_new_username, AuthProblem};
        for ok in ["Yugi", "Joey2", "Moderna", "Hosting", "Player1", "Rootbeer"] {
            assert_eq!(check_new_username(ok), Ok(()), "{ok}");
        }
        for reserved in ["COMPUTER", "computer", "Admin", "admin7", "Administra", "AdminYugi", "Server", "Mod", "Moderator2", "Roses", "Guest"] {
            assert_eq!(check_new_username(reserved), Err(AuthProblem::ReservedName), "{reserved}");
        }
        assert_eq!(check_new_username("ad"), Err(AuthProblem::BadUsername));
        // Existing accounts still log in with any valid name.
        assert!(check_username("Admin").is_ok());
    }

    #[test]
    fn older_and_newer_lobby_messages_read_each_other() {
        // A welcome from a server before open sign-up: invites needed.
        let old: LobbyEvent = serde_json::from_str(r#"{"type":"lobby_welcome","lobby_version":1,"server":"s","computer":false}"#).unwrap();
        assert!(matches!(old, LobbyEvent::LobbyWelcome { invite_required: true, .. }));
        // A sign-up without an invite (open sign-up) and a game hello.
        let req: LobbyRequest = serde_json::from_str(r#"{"type":"sign_up","username":"Yugi","password":"password1"}"#).unwrap();
        assert_eq!(req, LobbyRequest::SignUp { username: "Yugi".into(), password: "password1".into(), invite: String::new() });
        let hello = LobbyRequest::LobbyHello { lobby_version: 1, compat: Compat::current("x"), client: "g".into(), account: false, modes: false };
        let text = serde_json::to_string(&hello).unwrap();
        assert!(!text.contains("account"), "the game's hello is unchanged on the wire: {text}");
        assert_eq!(serde_json::from_str::<LobbyRequest>(&text).unwrap(), hello);
    }

    #[test]
    fn names_passwords_and_codes() {
        assert!(check_username("Yugi").is_ok());
        assert!(check_username("Joey2").is_ok());
        for bad in ["Yu", "2Joey", "Yugi Muto", "Yugi_M", "ThirteenChars"] {
            assert!(check_username(bad).is_err(), "{bad}");
        }
        assert!(check_password("eight ch").is_ok());
        assert!(check_password("seven c").is_err());
        assert!(check_password("tab\tinside").is_err());
        assert_eq!(normalize_code("abcd-ef gh"), "ABCDEFGH");
    }

    #[test]
    fn both_connection_kinds_share_the_wire() {
        // A plain duel or lobby message reads as the server's envelope and back.
        let hello = crate::protocol::ClientMessage::Sync;
        let frame: ClientFrame = serde_json::from_slice(&serde_json::to_vec(&hello).unwrap()).unwrap();
        assert_eq!(frame, ClientFrame::Match(hello));
        let req = LobbyRequest::JoinRoom { code: "ABCDE".into() };
        let frame: ClientFrame = serde_json::from_slice(&serde_json::to_vec(&req).unwrap()).unwrap();
        assert_eq!(frame, ClientFrame::Lobby(req));
        let ev = LobbyEvent::RoomClosed;
        let back: LobbyEvent = serde_json::from_slice(&serde_json::to_vec(&ServerFrame::Lobby(ev.clone())).unwrap()).unwrap();
        assert_eq!(back, ev);
    }
}
