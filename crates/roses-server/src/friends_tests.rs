//! Friends-online: accounts, the lobby state machine and the hub with
//! in-memory connections (the TCP/TLS end-to-end test is tests/friends.rs).

use crate::accounts::{self, Accounts, PasswordCost};
use crate::hub::{Hub, HubAcceptor, HubConn, HubData, HubSettings, COMPUTER_NAME};
use crate::lobby::{Action, Lobby};
use crate::play::{play_bot, PlayOptions};
use crate::serve::Limits;
use roses_net::friends::{LobbyClient, LobbyConn};
use roses_net::lobby::{
    AuthProblem, ChallengeEnd, ClientFrame, LobbyEvent, LobbyProblem, LobbyRequest, MatchResult, PlayerState,
    ServerFrame,
};
use roses_net::transport::memory_pair;
use roses_net::{synthetic, ClientMessage, Join, ServerMessage};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

// ---- accounts -------------------------------------------------------------

#[test]
fn passwords_are_argon2id_and_verify() {
    let phc = accounts::hash_password("correct horse", PasswordCost::TEST);
    assert!(phc.starts_with("$argon2id$"), "{phc}");
    assert!(accounts::verify_password("correct horse", &phc));
    assert!(!accounts::verify_password("correct hors", &phc));
    assert!(!accounts::verify_password("x", "not a phc"));
    // Two hashes of one password differ (fresh salts).
    assert_ne!(phc, accounts::hash_password("correct horse", PasswordCost::TEST));
}

#[test]
fn invites_create_accounts_once_and_sessions_resume() {
    let mut a = Accounts::in_memory().unwrap();
    let code = a.create_invite(2, Some("Jim's friends"), None).unwrap();
    assert_eq!(code.len(), 9);
    assert_eq!(&code[4..5], "-");
    let phc = accounts::hash_password("password1", PasswordCost::TEST);
    // Typed in lower case without the dash still works.
    let typed = code.replace('-', "").to_lowercase();
    assert_eq!(a.sign_up("Mai", &phc, &typed).unwrap(), Ok("Mai".into()));
    assert_eq!(a.sign_up("mai", &phc, &code).unwrap(), Err(AuthProblem::UsernameTaken));
    assert_eq!(a.sign_up("Joey", &phc, &code).unwrap(), Ok("Joey".into()));
    // Two uses: gone now.
    assert_eq!(a.sign_up("Tea", &phc, &code).unwrap(), Err(AuthProblem::BadInvite));
    assert!(a.invites().unwrap().is_empty());
    assert_eq!(a.sign_up("Tea", &phc, "ABCD-EFGH").unwrap(), Err(AuthProblem::BadInvite));
    let mai = a.user("MAI").unwrap().unwrap();
    assert_eq!(mai.name, "Mai");
    let token = a.create_session(mai.id).unwrap();
    assert_eq!(a.session_user(&token).unwrap().unwrap().name, "Mai");
    assert_eq!(a.session_user("guess").unwrap(), Err(AuthProblem::SessionExpired));
    a.end_session(&token).unwrap();
    assert_eq!(a.session_user(&token).unwrap(), Err(AuthProblem::SessionExpired));
    // Disabled accounts lose their sessions.
    let token = a.create_session(mai.id).unwrap();
    assert!(a.set_disabled("mai", true).unwrap());
    assert_eq!(a.session_user(&token).unwrap(), Err(AuthProblem::SessionExpired));
    assert!(a.users().unwrap().iter().any(|u| u.name == "Mai" && u.disabled));
}

#[test]
fn a_reset_code_sets_a_new_password_for_its_account_only() {
    let mut a = Accounts::in_memory().unwrap();
    let invite = a.create_invite(1, None, None).unwrap();
    a.sign_up("Bakura", &accounts::hash_password("oldpassword", PasswordCost::TEST), &invite).unwrap().unwrap();
    let id = a.user("bakura").unwrap().unwrap().id;
    let session = a.create_session(id).unwrap();
    assert!(a.create_reset("Nobody").is_err());
    let reset = a.create_reset("bakura").unwrap();
    // Only for that name.
    let other = accounts::hash_password("whatever1", PasswordCost::TEST);
    assert_eq!(a.sign_up("Ryou", &other, &reset).unwrap(), Err(AuthProblem::BadInvite));
    let new = accounts::hash_password("newpassword", PasswordCost::TEST);
    assert_eq!(a.sign_up("BAKURA", &new, &reset).unwrap(), Ok("Bakura".into()));
    let u = a.user("bakura").unwrap().unwrap();
    assert!(accounts::verify_password("newpassword", &u.phc));
    assert!(!accounts::verify_password("oldpassword", &u.phc));
    // The old sessions ended; the code is used up.
    assert_eq!(a.session_user(&session).unwrap(), Err(AuthProblem::SessionExpired));
    assert!(a.usable_invite(&reset, "bakura").unwrap().is_none());
}

#[test]
fn the_database_survives_reopening_and_backs_up() {
    let dir = std::env::temp_dir().join(format!("roses-accounts-{:016x}", crate::random_u64()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("accounts.db");
    {
        let mut a = Accounts::open(&path).unwrap();
        let invite = a.create_invite(1, None, Some(30)).unwrap();
        a.sign_up("Pegasus", &accounts::hash_password("millennium", PasswordCost::TEST), &invite).unwrap().unwrap();
        a.record_match(&accounts::MatchRow {
            id: "match-1".into(),
            players: ["Pegasus".into(), COMPUTER_NAME.into()],
            started: 1,
            ended: 2,
            outcome: "Pegasus won (LifePoints)".into(),
            cause: "Command".into(),
            revisions: 40,
        })
        .unwrap();
        a.backup(&dir.join("backup.db")).unwrap();
        assert!(a.backup(&dir.join("backup.db")).is_err(), "never overwrites");
    }
    for file in ["accounts.db", "backup.db"] {
        let a = Accounts::open(&dir.join(file)).unwrap();
        assert_eq!(a.users().unwrap().len(), 1, "{file}");
        assert_eq!(a.matches(10).unwrap().len(), 1, "{file}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn codes_use_the_unambiguous_alphabet() {
    for _ in 0..200 {
        let c = accounts::random_code(12);
        assert_eq!(c.len(), 12);
        assert!(c.bytes().all(|b| roses_net::lobby::CODE_ALPHABET.contains(&b)), "{c}");
    }
}

// ---- the lobby state machine ----------------------------------------------------

fn sent(actions: &[Action], conn: u64) -> Vec<LobbyEvent> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::Send(c, e) if *c == conn => Some(e.clone()),
            _ => None,
        })
        .collect()
}

fn states(lobby: &Lobby) -> Vec<(String, PlayerState)> {
    match lobby.list() {
        LobbyEvent::Players { players } => players.into_iter().map(|p| (p.name, p.state)).collect(),
        _ => unreachable!(),
    }
}

#[test]
fn challenges_pair_two_idle_players() {
    let mut l = Lobby::new(false);
    l.enter("Yugi", 1, false);
    let a = l.enter("Kaiba", 2, false);
    // The newcomer gets the list at once; everyone else with the hub's
    // next (coalesced) list.
    assert!(sent(&a, 2).iter().any(|e| matches!(e, LobbyEvent::Players { players } if players.len() == 2)));
    assert!(a.contains(&Action::Players));
    let r = |l: &mut Lobby, who: &str, req: LobbyRequest| l.request(who, &req, 0);
    assert_eq!(sent(&r(&mut l, "Yugi", LobbyRequest::Challenge { to: "yugi".into(), map: None }), 1), [LobbyEvent::Problem { problem: LobbyProblem::NotYourself }]);
    assert_eq!(sent(&r(&mut l, "Yugi", LobbyRequest::Challenge { to: "Joey".into(), map: None }), 1), [LobbyEvent::Problem { problem: LobbyProblem::UnknownPlayer }]);
    let a = r(&mut l, "Yugi", LobbyRequest::Challenge { to: "KAIBA".into(), map: None });
    assert_eq!(sent(&a, 1), [LobbyEvent::ChallengeSent { id: 1, to: "Kaiba".into() }]);
    assert_eq!(sent(&a, 2), [LobbyEvent::Challenged { id: 1, from: "Yugi".into(), map: None }]);
    // One open challenge at a time.
    assert_eq!(sent(&r(&mut l, "Yugi", LobbyRequest::Challenge { to: "Kaiba".into(), map: None }), 1), [LobbyEvent::Problem { problem: LobbyProblem::YouAreBusy }]);
    // Only the challenged player answers.
    assert_eq!(sent(&r(&mut l, "Yugi", LobbyRequest::Answer { id: 1, accept: true }), 1), [LobbyEvent::Problem { problem: LobbyProblem::NoSuchChallenge }]);
    let a = r(&mut l, "Kaiba", LobbyRequest::Answer { id: 1, accept: true });
    assert!(a.contains(&Action::Pair(["Yugi".into(), "Kaiba".into()], None)));
    assert!(sent(&a, 1).contains(&LobbyEvent::ChallengeClosed { id: 1, end: ChallengeEnd::Accepted }));
    l.started(&["Yugi", "Kaiba"]);
    assert_eq!(states(&l), [("Kaiba".into(), PlayerState::InMatch), ("Yugi".into(), PlayerState::InMatch)]);
    // Busy players cannot be challenged; after the match they can.
    l.enter("Joey", 3, false);
    assert_eq!(sent(&r(&mut l, "Joey", LobbyRequest::Challenge { to: "Yugi".into(), map: None }), 3), [LobbyEvent::Problem { problem: LobbyProblem::PlayerBusy }]);
    l.ended(&["Yugi", "Kaiba"]);
    let a = r(&mut l, "Joey", LobbyRequest::Challenge { to: "Yugi".into(), map: None });
    assert_eq!(sent(&a, 1), [LobbyEvent::Challenged { id: 2, from: "Joey".into(), map: None }]);
    // Declined, then a challenge that expires, then one whose player leaves.
    let a = r(&mut l, "Yugi", LobbyRequest::Answer { id: 2, accept: false });
    assert_eq!(sent(&a, 3), [LobbyEvent::ChallengeClosed { id: 2, end: ChallengeEnd::Declined }]);
    l.request("Joey", &LobbyRequest::Challenge { to: "Kaiba".into(), map: None }, 1_000);
    assert!(l.tick(1_000 + crate::lobby::CHALLENGE_MS - 1).is_empty());
    let a = l.tick(1_000 + crate::lobby::CHALLENGE_MS);
    assert_eq!(sent(&a, 3), [LobbyEvent::ChallengeClosed { id: 3, end: ChallengeEnd::Expired }]);
    l.request("Joey", &LobbyRequest::Challenge { to: "Kaiba".into(), map: None }, 2_000);
    let a = l.leave("Kaiba");
    assert!(sent(&a, 3).contains(&LobbyEvent::ChallengeClosed { id: 4, end: ChallengeEnd::Gone }));
    assert!(!l.is_online("kaiba"));
}

#[test]
fn rooms_pair_by_code_and_close() {
    let mut l = Lobby::new(true);
    l.enter("Mai", 1, false);
    l.enter("Joey", 2, false);
    let a = l.request("Mai", &LobbyRequest::CreateRoom { map: None }, 0);
    let code = sent(&a, 1)
        .iter()
        .find_map(|e| match e {
            LobbyEvent::RoomOpened { code } => Some(code.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(code.len(), roses_net::lobby::ROOM_CODE_LEN);
    assert!(states(&l).contains(&("Mai".into(), PlayerState::InRoom)));
    // Mai cannot challenge from a room; a wrong code is refused.
    assert_eq!(sent(&l.request("Mai", &LobbyRequest::Challenge { to: "Joey".into(), map: None }, 0), 1), [LobbyEvent::Problem { problem: LobbyProblem::YouAreBusy }]);
    assert_eq!(sent(&l.request("Joey", &LobbyRequest::JoinRoom { code: "ZZZZZ".into() }, 0), 2), [LobbyEvent::Problem { problem: LobbyProblem::BadRoomCode }]);
    let a = l.request("Joey", &LobbyRequest::JoinRoom { code: code.to_lowercase() }, 0);
    assert!(a.contains(&Action::PairPrivate(["Mai".into(), "Joey".into()], None)));
    assert_eq!(sent(&a, 1), [LobbyEvent::RoomClosed]);
    // A room nobody joins closes.
    l.request("Mai", &LobbyRequest::CreateRoom { map: None }, 10);
    let a = l.tick(10 + crate::lobby::ROOM_MS);
    assert!(sent(&a, 1).contains(&LobbyEvent::RoomClosed));
    // The computer.
    assert_eq!(l.request("Joey", &LobbyRequest::PlayComputer { map: None, computer: None }, 0), [Action::PairComputer("Joey".into(), None, None)]);
    let mut l = Lobby::new(false);
    l.enter("Joey", 2, false);
    assert_eq!(sent(&l.request("Joey", &LobbyRequest::PlayComputer { map: None, computer: None }, 0), 2), [LobbyEvent::Problem { problem: LobbyProblem::NoComputer }]);
}

// ---- the hub, in memory -----------------------------------------------------------

pub(crate) struct Running {
    tx: mpsc::Sender<HubConn>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<Hub>>,
}

impl Running {
    pub(crate) fn start(computer: bool, invite_uses: i64) -> (Self, String) {
        Self::with(computer, invite_uses, false, None)
    }

    /// `open_signup`: the hub's open sign-up mode; `peer`: the address
    /// every connection seems to come from.
    fn with(computer: bool, invite_uses: i64, open_signup: bool, peer: Option<std::net::IpAddr>) -> (Self, String) {
        let settings = HubSettings { open_signup, ..HubSettings::default() };
        Self::with_settings(computer, invite_uses, settings, peer)
    }

    /// A hub with these settings (the test name, no limits and the test
    /// password cost are set here).
    pub(crate) fn with_settings(computer: bool, invite_uses: i64, settings: HubSettings, peer: Option<std::net::IpAddr>) -> (Self, String) {
        let accounts = Accounts::in_memory().unwrap();
        let invite = accounts.create_invite(invite_uses, None, None).unwrap();
        let (tx, rx) = mpsc::channel::<HubConn>();
        let stop = Arc::new(AtomicBool::new(false));
        let settings = HubSettings {
            name: "Test server".into(),
            limits: Limits::UNLIMITED,
            password_cost: PasswordCost::TEST,
            ..settings
        };
        let flag = stop.clone();
        let thread = std::thread::spawn(move || {
            let mut hub = Hub::new(settings, HubData::synthetic(computer), accounts, |line| eprintln!("hub: {line}"));
            let acceptor: Box<dyn HubAcceptor + Send> = match peer {
                Some(ip) => Box::new(FromAddress(rx, ip)),
                None => Box::new(rx),
            };
            let mut acceptors: Vec<Box<dyn HubAcceptor + Send>> = vec![acceptor];
            hub.run(&mut acceptors, &flag).unwrap();
            hub
        });
        (Self { tx, stop, thread: Some(thread) }, invite)
    }

    pub(crate) fn lobby(&self) -> LobbyClient {
        self.try_lobby().unwrap()
    }

    /// A lobby connection; an error when the hub closed it before the
    /// hello went out (a refused connection).
    fn try_lobby(&self) -> std::io::Result<LobbyClient> {
        let (client, server) = memory_pair::<LobbyRequest, LobbyEvent>();
        self.tx.send(Box::new(server.retype::<ServerFrame, ClientFrame>())).unwrap();
        let compat = roses_net::Compat::current(roses_net::data_fingerprint(&synthetic::catalogue(), &roses_net::RulesTables::empty()));
        LobbyClient::start(Box::new(client) as LobbyConn, compat, "test")
    }

    /// A lobby connection from a client older than MODES (its hello has
    /// no `modes`).
    pub(crate) fn old_lobby(&self) -> LobbyClient {
        let (client, server) = memory_pair::<LobbyRequest, LobbyEvent>();
        self.tx.send(Box::new(server.retype::<ServerFrame, ClientFrame>())).unwrap();
        let compat = roses_net::Compat::current(roses_net::data_fingerprint(&synthetic::catalogue(), &roses_net::RulesTables::empty()));
        LobbyClient::start_older(Box::new(client) as LobbyConn, compat, "old test").unwrap()
    }

    /// The launcher's account-only connection (no rules data).
    fn account(&self) -> LobbyClient {
        let (client, server) = memory_pair::<LobbyRequest, LobbyEvent>();
        self.tx.send(Box::new(server.retype::<ServerFrame, ClientFrame>())).unwrap();
        LobbyClient::start_account(Box::new(client) as LobbyConn, "launcher test").unwrap()
    }

    pub(crate) fn match_connector(&self) -> impl FnMut() -> std::io::Result<roses_net::transport::Memory<ClientMessage, ServerMessage>> + Send + 'static {
        let tx = self.tx.clone();
        move || {
            let (client, server) = memory_pair::<ClientMessage, ServerMessage>();
            tx.send(Box::new(server.retype::<ServerFrame, ClientFrame>())).map_err(|_| std::io::Error::other("hub gone"))?;
            Ok(client)
        }
    }

    pub(crate) fn stop(mut self) -> Hub {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap()
    }
}

/// In-memory connections that seem to come from one address.
struct FromAddress(mpsc::Receiver<HubConn>, std::net::IpAddr);

impl HubAcceptor for FromAddress {
    fn accept(&mut self) -> std::io::Result<Option<(HubConn, Option<std::net::IpAddr>)>> {
        Ok(self.0.try_recv().ok().map(|c| (c, Some(self.1))))
    }
}

/// Pump `c` until `done` holds (10 s).
pub(crate) fn until(c: &mut LobbyClient, what: &str, mut done: impl FnMut(&LobbyClient) -> bool) {
    let start = Instant::now();
    loop {
        c.pump();
        if done(c) {
            return;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "timed out waiting for {what} (note {:?})", c.note);
        std::thread::sleep(Duration::from_millis(2));
    }
}

pub(crate) fn sees(c: &LobbyClient, name: &str, state: PlayerState) -> bool {
    c.players.iter().any(|p| p.name == name && p.state == state)
}

#[test]
fn the_hub_signs_up_pairs_plays_and_returns_both_to_the_lobby() {
    let (hub, invite) = Running::start(false, 2);
    let mut yugi = hub.lobby();
    let mut kaiba = hub.lobby();
    until(&mut yugi, "welcome", |c| c.server.is_some());
    assert_eq!(yugi.server, Some(("Test server".into(), false)));
    // Requests before signing in, bad names and passwords, bad invites.
    yugi.send(LobbyRequest::Challenge { to: "x".into(), map: None });
    until(&mut yugi, "not signed in", |c| c.auth_problem == Some(AuthProblem::NotSignedIn));
    for (name, pw, inv, problem) in [
        ("yu", "password1", invite.as_str(), AuthProblem::BadUsername),
        ("Yugi_M", "password1", invite.as_str(), AuthProblem::BadUsername),
        ("Yugi", "short", invite.as_str(), AuthProblem::WeakPassword),
        ("Yugi", "password1", "NOPE-NOPE", AuthProblem::BadInvite),
    ] {
        yugi.send(LobbyRequest::SignUp { username: name.into(), password: pw.into(), invite: inv.into() });
        until(&mut yugi, "a refusal", |c| c.auth_problem == Some(problem));
    }
    yugi.send(LobbyRequest::SignUp { username: "Yugi".into(), password: "heart of the cards".into(), invite: invite.clone() });
    until(&mut yugi, "sign-up", |c| c.user.is_some());
    kaiba.send(LobbyRequest::SignUp { username: "yugi".into(), password: "blue eyes".into(), invite: invite.clone() });
    until(&mut kaiba, "name taken", |c| c.auth_problem == Some(AuthProblem::UsernameTaken));
    kaiba.send(LobbyRequest::SignUp { username: "Kaiba".into(), password: "blue eyes".into(), invite: invite.clone() });
    until(&mut kaiba, "sign-up", |c| c.user.is_some());
    // Log out and back in with the password, then wrong passwords.
    kaiba.send(LobbyRequest::LogOut);
    until(&mut kaiba, "log-out", |c| c.user.is_none());
    kaiba.send(LobbyRequest::LogIn { username: "KAIBA".into(), password: "white dragon".into() });
    until(&mut kaiba, "wrong password", |c| c.auth_problem == Some(AuthProblem::BadCredentials));
    kaiba.send(LobbyRequest::LogIn { username: "Nobody".into(), password: "white dragon".into() });
    until(&mut kaiba, "unknown name", |c| c.auth_problem == Some(AuthProblem::BadCredentials));
    kaiba.send(LobbyRequest::LogIn { username: "kaiba".into(), password: "blue eyes".into() });
    until(&mut kaiba, "log-in", |c| c.user.as_deref() == Some("Kaiba"));
    until(&mut yugi, "Kaiba online", |c| sees(c, "Kaiba", PlayerState::Idle));
    until(&mut kaiba, "Yugi online", |c| sees(c, "Yugi", PlayerState::Idle));
    // The session token signs in again on a new connection; the old one is
    // signed out.
    let session = yugi.session.clone().unwrap();
    let mut yugi2 = hub.lobby();
    yugi2.send(LobbyRequest::ResumeSession { session });
    until(&mut yugi2, "resume", |c| c.user.as_deref() == Some("Yugi"));
    until(&mut yugi, "signed out elsewhere", |c| c.user.is_none());
    let mut yugi = yugi2;
    // Challenge and accept.
    yugi.send(LobbyRequest::Challenge { to: "kaiba".into(), map: None });
    until(&mut kaiba, "challenge", |c| !c.incoming.is_empty());
    let id = kaiba.incoming[0].0;
    kaiba.send(LobbyRequest::Answer { id, accept: true });
    until(&mut yugi, "match ready", |c| c.ready.is_some());
    until(&mut kaiba, "match ready", |c| c.ready.is_some());
    until(&mut yugi, "both in match", |c| sees(c, "Kaiba", PlayerState::InMatch) && sees(c, "Yugi", PlayerState::InMatch));
    let (ry, rk) = (yugi.ready.clone().unwrap(), kaiba.ready.clone().unwrap());
    assert_eq!(ry.match_id, rk.match_id);
    assert_eq!((ry.opponent.as_str(), rk.opponent.as_str()), ("Kaiba", "Yugi"));
    assert_ne!(ry.seat, rk.seat);
    assert_ne!(ry.ticket, rk.ticket);
    // A wrong ticket is refused; the real ones play the match (seat 1
    // drops once and comes back with its resume token).
    let mut bad = hub.match_connector();
    let mut conn = bad().unwrap();
    use roses_net::transport::Connection;
    conn.send(&ClientMessage::Hello { compat: yugi_compat(), join: Join::Ticket { ticket: "guess".into() }, deck: None, presence: false, spectate: false }).unwrap();
    assert!(matches!(
        roses_net::transport::recv_timeout(&mut conn, Duration::from_secs(5)),
        roses_net::transport::Received::Message(ServerMessage::Refused { reason: roses_net::Refusal::BadToken })
    ));
    let decks = synthetic::submissions();
    let players: Vec<_> = [ry.clone(), rk.clone()]
        .into_iter()
        .map(|r| {
            let connect = hub.match_connector();
            let deck = decks[usize::from(r.seat)].clone();
            std::thread::spawn(move || {
                play_bot(
                    connect,
                    &yugi_compat(),
                    &synthetic::catalogue(),
                    PlayOptions {
                        join: Join::Ticket { ticket: r.ticket },
                        deck: Some(deck),
                        drop_at_revision: (r.seat == 1).then_some(5),
                        ..PlayOptions::default()
                    },
                    |_| {},
                )
            })
        })
        .collect();
    let reports: Vec<_> = players.into_iter().map(|t| t.join().unwrap().unwrap()).collect();
    let outcome = reports[0].state.ended.unwrap().0;
    assert_eq!(Some(outcome), reports[1].state.ended.map(|e| e.0));
    assert!(reports.iter().any(|r| r.reconnects == 1));
    // Both are told the result and are idle again.
    until(&mut yugi, "match over", |c| c.over.is_some());
    until(&mut kaiba, "match over", |c| c.over.is_some());
    let result = |c: &LobbyClient| c.over.as_ref().unwrap().1;
    let expect = |seat: u8| match outcome.winner {
        None => MatchResult::Draw,
        Some(w) if w == seat => MatchResult::Won,
        Some(_) => MatchResult::Lost,
    };
    assert_eq!(result(&yugi), expect(ry.seat));
    assert_eq!(result(&kaiba), expect(rk.seat));
    until(&mut yugi, "idle again", |c| sees(c, "Kaiba", PlayerState::Idle) && sees(c, "Yugi", PlayerState::Idle));
    // A room by code, then the computer is not offered here.
    kaiba.send(LobbyRequest::CreateRoom { map: None });
    until(&mut kaiba, "room", |c| c.room.is_some());
    yugi.send(LobbyRequest::JoinRoom { code: kaiba.room.clone().unwrap() });
    until(&mut yugi, "room match", |c| c.ready.is_some());
    until(&mut kaiba, "room match", |c| c.ready.is_some() && c.room.is_none());
    yugi.send(LobbyRequest::PlayComputer { map: None, computer: None });
    until(&mut yugi, "no computer", |c| c.note.as_deref() == Some(&*LobbyProblem::NoComputer.to_string()));
    let hub = hub.stop();
    let log = hub.accounts().matches(10).unwrap();
    assert_eq!(log.len(), 1, "the finished match is logged (the room match never started)");
}

pub(crate) fn yugi_compat() -> roses_net::Compat {
    roses_net::Compat::current(roses_net::data_fingerprint(&synthetic::catalogue(), &roses_net::RulesTables::empty()))
}

#[test]
fn the_hub_plays_its_computer_and_rejoins_a_restarted_client() {
    let (hub, invite) = Running::start(true, 1);
    let mut joey = hub.lobby();
    joey.send(LobbyRequest::SignUp { username: "Joey".into(), password: "red eyes black".into(), invite });
    until(&mut joey, "sign-up", |c| c.user.is_some() && c.server == Some(("Test server".into(), true)));
    joey.send(LobbyRequest::PlayComputer { map: None, computer: None });
    until(&mut joey, "match ready", |c| c.ready.is_some());
    let ready = joey.ready.clone().unwrap();
    assert_eq!(ready.opponent, COMPUTER_NAME);
    // The game "restarts": a new lobby connection with the session gets
    // the same seat's ticket again.
    let session = joey.session.clone().unwrap();
    drop(joey);
    let mut joey = hub.lobby();
    joey.send(LobbyRequest::ResumeSession { session });
    until(&mut joey, "the ticket again", |c| c.ready.is_some());
    assert_eq!(joey.ready.as_ref().unwrap().ticket, ready.ticket);
    assert!(sees(&joey, "Joey", PlayerState::InMatch));
    let report = play_bot(
        hub.match_connector(),
        &yugi_compat(),
        &synthetic::catalogue(),
        PlayOptions {
            join: Join::Ticket { ticket: ready.ticket },
            deck: Some(synthetic::submissions()[usize::from(ready.seat)].clone()),
            ..PlayOptions::default()
        },
        |_| {},
    )
    .unwrap();
    assert!(report.state.ended.is_some());
    until(&mut joey, "match over", |c| c.over.is_some() && sees(c, "Joey", PlayerState::Idle));
    let hub = hub.stop();
    let log = hub.accounts().matches(10).unwrap();
    assert_eq!(log.len(), 1);
    assert!(log[0].players.contains(&COMPUTER_NAME.to_string()));
}

#[test]
fn failed_sign_ins_are_limited_per_name() {
    let (hub, invite) = Running::start(false, 1);
    let mut c = hub.lobby();
    c.send(LobbyRequest::SignUp { username: "Weevil".into(), password: "insect armor".into(), invite });
    until(&mut c, "sign-up", |c| c.user.is_some());
    c.send(LobbyRequest::LogOut);
    until(&mut c, "log-out", |c| c.user.is_none());
    for _ in 0..crate::hub::ATTEMPTS_PER_NAME {
        c.send(LobbyRequest::LogIn { username: "weevil".into(), password: "wrong guess".into() });
        until(&mut c, "refusal", |c| c.auth_problem == Some(AuthProblem::BadCredentials));
        c.auth_problem = None;
    }
    // Even the right password waits now.
    c.send(LobbyRequest::LogIn { username: "weevil".into(), password: "insect armor".into() });
    until(&mut c, "limited", |c| c.auth_problem == Some(AuthProblem::TooManyAttempts));
    hub.stop();
}

#[test]
fn failed_sign_ins_lock_a_name_only_from_the_guessing_address() {
    use crate::hub::{Attempts, ATTEMPTS_PER_NAME, ATTEMPTS_PER_NAME_ALL_ADDRESSES, ATTEMPT_WINDOW_MS};
    let ip = |n: u32| -> Option<std::net::IpAddr> { Some(std::net::Ipv4Addr::from(0xC000_0200 + n).into()) };
    let mut a = Attempts::default();
    for _ in 0..ATTEMPTS_PER_NAME {
        assert!(a.allowed(ip(1), Some("Weevil"), 0));
        a.failed(ip(1), Some("Weevil"), 0);
    }
    // The guessing address is locked out of that name (any case)...
    assert!(!a.allowed(ip(1), Some("weevil"), 0));
    // ...but not out of other names, and the player's own address still
    // signs in.
    assert!(a.allowed(ip(1), Some("Rex"), 0));
    assert!(a.allowed(ip(2), Some("Weevil"), 0));
    // The window passes.
    assert!(a.allowed(ip(1), Some("Weevil"), ATTEMPT_WINDOW_MS));
    // A guessing attack spread over many addresses stops at the
    // all-addresses limit.
    let mut a = Attempts::default();
    for n in 0..ATTEMPTS_PER_NAME_ALL_ADDRESSES as u32 {
        assert!(a.allowed(ip(n), Some("Mai"), 0), "{n}");
        a.failed(ip(n), Some("Mai"), 0);
    }
    assert!(!a.allowed(ip(9999), Some("Mai"), 0));
    assert!(a.allowed(ip(9999), Some("Tea"), 0));
}

#[test]
fn one_address_cannot_take_every_connection() {
    let ip: std::net::IpAddr = "203.0.113.20".parse().unwrap();
    let (hub, _) = Running::with(false, 1, true, Some(ip));
    let mut open = Vec::new();
    for _ in 0..crate::hub::CONNECTIONS_PER_ADDRESS {
        let mut c = hub.lobby();
        until(&mut c, "welcome", |c| c.server.is_some());
        open.push(c);
    }
    // Refused: either the refusal arrives or the hub closed the
    // connection before the hello was sent.
    if let Ok(mut c) = hub.try_lobby() {
        let start = Instant::now();
        while c.refused.is_none() && c.server.is_none() && start.elapsed() < Duration::from_secs(10) {
            c.pump();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(c.refused, Some(roses_net::lobby::LobbyRefusal::ServerFull));
    }
    // One closes: the address may connect again.
    drop(open.pop());
    let start = Instant::now();
    loop {
        assert!(start.elapsed() < Duration::from_secs(10), "the address never got a slot back");
        let Ok(mut c) = hub.try_lobby() else { continue };
        let ok = (0..500).find_map(|_| {
            c.pump();
            std::thread::sleep(Duration::from_millis(2));
            if c.server.is_some() {
                Some(true)
            } else if c.refused.is_some() {
                Some(false)
            } else {
                None
            }
        });
        if ok == Some(true) {
            break;
        }
    }
    hub.stop();
}

#[test]
fn the_name_filter_refuses_new_names_only() {
    let (hub, invite) = Running::with(false, 2, true, None);
    let mut c = hub.lobby();
    until(&mut c, "welcome", |c| c.server.is_some());
    for name in ["KonamiStaff", "RosesDev2", "N4zi"] {
        sign_up(&mut c, name, "password1");
        until(&mut c, "refused", |c| c.auth_problem == Some(AuthProblem::ReservedName));
        c.auth_problem = None;
        // With an invite code too.
        c.send(LobbyRequest::SignUp { username: name.into(), password: "password1".into(), invite: invite.clone() });
        until(&mut c, "refused", |c| c.auth_problem == Some(AuthProblem::ReservedName));
        c.auth_problem = None;
    }
    sign_up(&mut c, "Cassandra", "password1");
    until(&mut c, "an ordinary name", |c| c.user.as_deref() == Some("Cassandra"));
    hub.stop();
    // An operator's own list adds to the built-in one.
    let mut f = crate::name_filter::NameFilter::builtin();
    let before = f.len();
    f.add("# ours\nrivalgame\n=boss\n").unwrap();
    assert_eq!(f.len(), before + 2);
    assert!(!f.allows("RivalGame1") && !f.allows("Boss7") && f.allows("Bossman"));
}

#[test]
fn deleting_an_account_erases_it_and_anonymises_its_matches() {
    use crate::accounts::{MatchRow, DELETED_NAME};
    let mut a = Accounts::in_memory().unwrap();
    let invite = a.create_invite(2, None, None).unwrap();
    let phc = accounts::hash_password("password1", PasswordCost::TEST);
    a.sign_up("Rex", &phc, &invite).unwrap().unwrap();
    a.sign_up("Weevil", &phc, &invite).unwrap().unwrap();
    let rex = a.user("rex").unwrap().unwrap();
    let session = a.create_session(rex.id).unwrap();
    a.create_session(rex.id).unwrap();
    a.create_reset("rex").unwrap();
    let row = |id: &str, p0: &str, p1: &str| MatchRow {
        id: id.into(),
        players: [p0.into(), p1.into()],
        started: 1,
        ended: 2,
        outcome: "x".into(),
        cause: "y".into(),
        revisions: 3,
    };
    a.record_match(&row("m1", "Rex", "Weevil")).unwrap();
    a.record_match(&row("m2", COMPUTER_NAME, "Rex")).unwrap();
    a.record_match(&row("m3", "Weevil", COMPUTER_NAME)).unwrap();
    let d = a.delete_user("REX").unwrap().unwrap();
    assert_eq!((d.name.as_str(), d.sessions, d.resets), ("Rex", 2, 1));
    assert_eq!(d.match_ids.len(), 2);
    assert!(a.user("rex").unwrap().is_none());
    assert_eq!(a.session_user(&session).unwrap(), Err(AuthProblem::SessionExpired));
    assert!(a.invites().unwrap().iter().all(|i| i.for_user.is_none()));
    let log = a.matches(10).unwrap();
    assert!(log.iter().all(|m| !m.players.iter().any(|p| p == "Rex")));
    assert_eq!(log.iter().filter(|m| m.players.contains(&DELETED_NAME.to_string())).count(), 2);
    assert!(log.iter().any(|m| m.players == ["Weevil".to_string(), COMPUTER_NAME.to_string()]));
    // Gone: nothing more to delete, and the name is free again.
    assert!(a.delete_user("Rex").unwrap().is_none());
    assert!(a.user("weevil").unwrap().is_some());
    assert_eq!(a.sign_up_open("Rex", &phc).unwrap(), Ok("Rex".into()));
    // Match-log retention.
    assert_eq!(a.prune_matches(1).unwrap(), 3);
    assert!(a.matches(10).unwrap().is_empty());
}

#[test]
fn old_replays_are_pruned() {
    let dir = std::env::temp_dir().join(format!("roses-replays-{}", crate::random_u64()));
    std::fs::create_dir_all(&dir).unwrap();
    let old = dir.join("old.json");
    std::fs::write(&old, b"{}").unwrap();
    std::fs::write(dir.join("new.json"), b"{}").unwrap();
    std::fs::write(dir.join("notes.txt"), b"keep").unwrap();
    let file = std::fs::File::options().write(true).open(&old).unwrap();
    file.set_modified(std::time::SystemTime::now() - Duration::from_secs(40 * 86_400)).unwrap();
    drop(file);
    assert_eq!(crate::replay::prune_replays(&dir, Duration::from_secs(30 * 86_400)).unwrap(), 1);
    assert!(!old.exists() && dir.join("new.json").exists() && dir.join("notes.txt").exists());
    assert_eq!(crate::replay::prune_replays(&dir.join("missing"), Duration::ZERO).unwrap(), 0);
    std::fs::remove_dir_all(&dir).unwrap();
}

// ---- open sign-up and the launcher's account page --------------------------

fn sign_up(c: &mut LobbyClient, name: &str, password: &str) {
    c.send(LobbyRequest::SignUp { username: name.into(), password: password.into(), invite: String::new() });
}

#[test]
fn open_sign_up_needs_no_invite_but_limits_names_and_addresses() {
    let ip: std::net::IpAddr = "203.0.113.9".parse().unwrap();
    let (hub, invite) = Running::with(false, 1, true, Some(ip));
    let mut c = hub.lobby();
    until(&mut c, "welcome", |c| c.server.is_some());
    assert!(!c.invite_required, "the welcome says sign-up is open");
    for (name, problem) in [("Computer", AuthProblem::ReservedName), ("admin2", AuthProblem::ReservedName), ("x", AuthProblem::BadUsername)] {
        sign_up(&mut c, name, "password1");
        until(&mut c, "a refusal", |c| c.auth_problem == Some(problem));
    }
    let names = ["Yugi", "Joey", "Tea", "Tristan", "Bakura"];
    assert_eq!(names.len(), crate::hub::SIGNUPS_PER_ADDRESS);
    for name in names {
        let mut c = hub.lobby();
        sign_up(&mut c, name, "password1");
        until(&mut c, "open sign-up", |c| c.user.as_deref() == Some(name));
    }
    // A taken name is refused before it counts; the sixth new account
    // from this address waits.
    let mut c = hub.lobby();
    sign_up(&mut c, "yugi", "password1");
    until(&mut c, "name taken", |c| c.auth_problem == Some(AuthProblem::UsernameTaken));
    sign_up(&mut c, "Marik", "password1");
    until(&mut c, "limited", |c| c.auth_problem == Some(AuthProblem::SignUpLimit));
    // An invite code from the owner still works (not counted as open).
    c.send(LobbyRequest::SignUp { username: "Marik".into(), password: "password1".into(), invite });
    until(&mut c, "invite sign-up", |c| c.user.as_deref() == Some("Marik"));
    hub.stop();

    // Without open sign-up an invite is needed and the welcome says so.
    let (hub, _) = Running::start(false, 1);
    let mut c = hub.lobby();
    until(&mut c, "welcome", |c| c.server.is_some());
    assert!(c.invite_required);
    sign_up(&mut c, "Yugi", "password1");
    until(&mut c, "invite needed", |c| c.auth_problem == Some(AuthProblem::BadInvite));
    hub.stop();
}

#[test]
fn the_account_page_signs_up_and_changes_the_password_without_the_lobby() {
    let (hub, _) = Running::with(false, 1, true, None);
    // The launcher: no rules data, sign up, never in the lobby.
    let mut page = hub.account();
    until(&mut page, "welcome", |c| c.server.is_some());
    sign_up(&mut page, "Yugi", "heart of the cards");
    until(&mut page, "sign-up", |c| c.user.is_some());
    let session = page.session.clone().unwrap();
    // The game resumes with the launcher's session and enters the lobby.
    let mut game = hub.lobby();
    game.send(LobbyRequest::ResumeSession { session: session.clone() });
    until(&mut game, "resume", |c| c.user.as_deref() == Some("Yugi"));
    until(&mut game, "in the lobby", |c| sees(c, "Yugi", PlayerState::Idle) && c.players.len() == 1);
    // The page logs in again: the game stays signed in.
    let mut page = hub.account();
    page.send(LobbyRequest::LogIn { username: "yugi".into(), password: "heart of the cards".into() });
    until(&mut page, "page log-in", |c| c.user.is_some());
    page.send(LobbyRequest::Challenge { to: "Yugi".into(), map: None });
    page.send(LobbyRequest::ChangePassword { old_password: "wrong one!".into(), new_password: "millennium puzzle".into() });
    until(&mut page, "wrong old password", |c| c.auth_problem == Some(AuthProblem::BadCredentials));
    page.send(LobbyRequest::ChangePassword { old_password: "heart of the cards".into(), new_password: "short".into() });
    until(&mut page, "weak new password", |c| c.auth_problem == Some(AuthProblem::WeakPassword));
    page.send(LobbyRequest::ChangePassword { old_password: "heart of the cards".into(), new_password: "millennium puzzle".into() });
    until(&mut page, "changed", |c| c.password_changed);
    // The page's own session still works; the older ones ended.
    let kept = page.session.clone().unwrap();
    let mut again = hub.account();
    again.send(LobbyRequest::ResumeSession { session: kept });
    until(&mut again, "kept session", |c| c.user.is_some());
    page.send(LobbyRequest::LogOut);
    until(&mut page, "page log-out", |c| c.user.is_none());
    game.pump();
    assert_eq!(game.user.as_deref(), Some("Yugi"), "the page never signs the game out");
    let mut other = hub.account();
    other.send(LobbyRequest::ResumeSession { session });
    until(&mut other, "old session ended", |c| c.auth_problem == Some(AuthProblem::SessionExpired));
    other.send(LobbyRequest::LogIn { username: "Yugi".into(), password: "heart of the cards".into() });
    until(&mut other, "old password refused", |c| c.auth_problem == Some(AuthProblem::BadCredentials));
    other.send(LobbyRequest::LogIn { username: "Yugi".into(), password: "millennium puzzle".into() });
    until(&mut other, "new password", |c| c.user.is_some());
    drop(other);
    drop(again);
    drop(page);
    std::thread::sleep(Duration::from_millis(50));
    game.send(LobbyRequest::Ping);
    until(&mut game, "still in the lobby", |c| sees(c, "Yugi", PlayerState::Idle) && c.user.is_some());
    // A game hello without the rules data is refused; the page's is not.
    let (client, server) = memory_pair::<LobbyRequest, LobbyEvent>();
    hub.tx.send(Box::new(server.retype::<ServerFrame, ClientFrame>())).unwrap();
    let mut wrong = LobbyClient::start(Box::new(client) as LobbyConn, roses_net::Compat::current(""), "old game").unwrap();
    until(&mut wrong, "refused", |c| c.refused.is_some());
    hub.stop();
}

#[test]
fn player_lists_are_spaced_out_as_the_lobby_grows() {
    use crate::hub::players_interval_ms;
    assert_eq!(players_interval_ms(2), 0);
    assert_eq!(players_interval_ms(16), 1);
    assert_eq!(players_interval_ms(128), 64);
    assert_eq!(players_interval_ms(256), 256);
    assert_eq!(players_interval_ms(512), 1024);
    assert_eq!(players_interval_ms(5000), 2000);
}

#[test]
fn a_client_that_stops_reading_does_not_hold_up_the_others() {
    use roses_net::lobby::{PlayerEntry, LOBBY_VERSION};
    use roses_net::transport::{Connection, FramedTcp, LoopbackListener, Received};
    let listener = LoopbackListener::bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();
    let mut acceptors: Vec<Box<dyn HubAcceptor + Send>> = vec![Box::new(listener)];
    let settings = HubSettings { limits: Limits::UNLIMITED, password_cost: PasswordCost::TEST, ..HubSettings::default() };
    let mut hub = Hub::new(settings, HubData::synthetic(false), Accounts::in_memory().unwrap(), |l| eprintln!("hub: {l}"));
    let start = Instant::now();
    let now = || start.elapsed().as_millis() as u64;
    let hello = LobbyRequest::LobbyHello {
        lobby_version: LOBBY_VERSION,
        compat: roses_net::Compat::current(""),
        client: "test".into(),
        account: true,
        modes: false,
    };
    let welcome = |hub: &mut Hub, acceptors: &mut Vec<Box<dyn HubAcceptor + Send>>, c: &mut FramedTcp<LobbyRequest, LobbyEvent>| loop {
        hub.step(acceptors, now()).unwrap();
        if let Received::Message(LobbyEvent::LobbyWelcome { .. }) = c.poll() {
            return;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "no welcome");
        std::thread::sleep(Duration::from_millis(1));
    };
    // The stalled player says hello, reads its welcome and then nothing more.
    let mut stalled = FramedTcp::<LobbyRequest, LobbyEvent>::connect(addr).unwrap();
    stalled.send(&hello).unwrap();
    welcome(&mut hub, &mut acceptors, &mut stalled);
    let stalled_id = hub.conn_ids()[0];
    let mut active = FramedTcp::<LobbyRequest, LobbyEvent>::connect(addr).unwrap();
    active.send(&hello).unwrap();
    welcome(&mut hub, &mut acceptors, &mut active);
    // About 600 KB per event, 40 of them: far beyond the socket buffers
    // and the send queue. Each round the active player's ping is answered
    // promptly; the old blocking send would hold the whole hub up to 10 s.
    let big = LobbyEvent::Players {
        players: (0..20_000).map(|i| PlayerEntry { name: format!("player{i}"), state: PlayerState::Idle }).collect(),
    };
    let mut worst = Duration::ZERO;
    for _ in 0..40 {
        let t = Instant::now();
        hub.send_lobby_for_test(stalled_id, big.clone(), now());
        active.send(&LobbyRequest::Ping).unwrap();
        loop {
            hub.step(&mut acceptors, now()).unwrap();
            match active.poll() {
                Received::Message(LobbyEvent::Pong) => break,
                Received::Closed => panic!("the active player was cut off"),
                _ => {}
            }
            assert!(t.elapsed() < Duration::from_secs(5), "the ping was not answered");
        }
        worst = worst.max(t.elapsed());
    }
    assert!(worst < Duration::from_millis(500), "a ping took {worst:?} while another client stalled");
    assert!(!hub.conn_ids().contains(&stalled_id), "the stalled client was not cut off");
    drop(stalled);
}

// ---- match choices: the board and the computer's deck ----------------------------

/// The welcome offers the choices; a board outside the table, a bad own
/// map and a computer deck that breaks the deck rules are refused; PLAY
/// COMPUTER on board 1 with "Pegasus Crawford"'s deck tells the player
/// the board and the computer's name and plays on that terrain; a
/// challenger's own map is shown to the one challenged and both seats get
/// it; RANDOM becomes one of the boards.
#[test]
fn the_hub_plays_the_chosen_board_and_computer_deck() {
    use roses_net::lobby::{ComputerDeck, MapChoice};
    let (hub, invite) = Running::start(true, 3);
    let mut joey = hub.lobby();
    joey.send(LobbyRequest::SignUp { username: "Joey".into(), password: "red eyes black".into(), invite: invite.clone() });
    until(&mut joey, "sign-up", |c| c.user.is_some());
    assert!(joey.choices, "the welcome offers the choices");
    let note = |p: LobbyProblem| p.to_string();
    joey.send(LobbyRequest::PlayComputer { map: Some(MapChoice::Game { index: 2 }), computer: None });
    until(&mut joey, "bad board", |c| c.note.as_deref() == Some(&*note(LobbyProblem::BadMap)));
    joey.note = None;
    joey.send(LobbyRequest::PlayComputer { map: Some(MapChoice::Own { name: "MINE".into(), cells: vec![10; 49] }), computer: None });
    until(&mut joey, "bad own map", |c| c.note.as_deref() == Some(&*note(LobbyProblem::BadMap)));
    let short = roses_net::DeckSubmission { leader: 1, leader_rank: 1, cards: vec![2; 3] };
    joey.send(LobbyRequest::PlayComputer { map: None, computer: Some(ComputerDeck::Deck { name: "Short".into(), deck: short }) });
    until(&mut joey, "bad deck", |c| c.note.as_deref() == Some(&*note(LobbyProblem::BadComputerDeck)));
    let pegasus = synthetic::submissions()[1].clone();
    joey.send(LobbyRequest::PlayComputer {
        map: Some(MapChoice::Game { index: 1 }),
        computer: Some(ComputerDeck::Deck { name: "Pegasus Crawford".into(), deck: pegasus }),
    });
    until(&mut joey, "match ready", |c| c.ready.is_some());
    let ready = joey.ready.clone().unwrap();
    assert_eq!((ready.opponent.as_str(), ready.opponent_title.as_deref()), (COMPUTER_NAME, Some("Pegasus Crawford")));
    assert_eq!(ready.map, Some(MapChoice::Game { index: 1 }));
    let report = play_bot(
        hub.match_connector(),
        &yugi_compat(),
        &synthetic::catalogue(),
        PlayOptions {
            join: Join::Ticket { ticket: ready.ticket },
            deck: Some(synthetic::submissions()[0].clone()),
            ..PlayOptions::default()
        },
        |_| {},
    )
    .unwrap();
    let terrain = &report.state.view.as_ref().unwrap().board.terrain;
    assert_eq!(terrain, &vec![3; 49], "the duel is played on board 1");
    until(&mut joey, "match over", |c| c.over.is_some());
    // A challenge on an own map: shown before answering, both seats get it.
    let mut kaiba = hub.lobby();
    kaiba.send(LobbyRequest::SignUp { username: "Kaiba".into(), password: "blue eyes white".into(), invite });
    until(&mut kaiba, "sign-up", |c| c.user.is_some());
    until(&mut joey, "idle", |c| sees(c, "Kaiba", PlayerState::Idle) && sees(c, "Joey", PlayerState::Idle));
    let own = MapChoice::Own { name: "MY MAP 1".into(), cells: (0..49).map(|i| (i % 10) as u8).collect() };
    kaiba.send(LobbyRequest::Challenge { to: "Joey".into(), map: Some(own.clone()) });
    until(&mut joey, "challenged", |c| !c.incoming.is_empty());
    let id = joey.incoming[0].0;
    joey.send(LobbyRequest::Answer { id, accept: true });
    until(&mut joey, "ready", |c| c.ready.is_some());
    until(&mut kaiba, "ready", |c| c.ready.is_some());
    assert_eq!(joey.ready.as_ref().unwrap().map, Some(own.clone()));
    assert_eq!(kaiba.ready.as_ref().unwrap().map, Some(own));
    assert_eq!(joey.ready.as_ref().unwrap().opponent_title, None);
    let _ = hub.stop();
}

/// The lobby keeps the challenger's and the host's board: the challenged
/// player is told it, and the pair starts on it.
#[test]
fn the_lobby_carries_the_board_to_the_match() {
    use roses_net::lobby::MapChoice;
    let mut l = Lobby::new(true);
    l.enter("Yugi", 1, false);
    l.enter("Kaiba", 2, false);
    let map = Some(MapChoice::Game { index: 0x17 });
    let a = l.request("Yugi", &LobbyRequest::Challenge { to: "Kaiba".into(), map: map.clone() }, 0);
    assert!(a.contains(&Action::Send(2, LobbyEvent::Challenged { id: 1, from: "Yugi".into(), map: map.clone() })));
    let a = l.request("Kaiba", &LobbyRequest::Answer { id: 1, accept: true }, 0);
    assert!(a.contains(&Action::Pair(["Yugi".into(), "Kaiba".into()], map.clone())));
    let mut l = Lobby::new(true);
    l.enter("Mai", 1, false);
    l.enter("Joey", 2, false);
    let a = l.request("Mai", &LobbyRequest::CreateRoom { map: Some(MapChoice::Random) }, 0);
    let code = a.iter().find_map(|x| match x { Action::Send(_, LobbyEvent::RoomOpened { code }) => Some(code.clone()), _ => None }).unwrap();
    let a = l.request("Joey", &LobbyRequest::JoinRoom { code }, 0);
    assert!(a.contains(&Action::PairPrivate(["Mai".into(), "Joey".into()], Some(MapChoice::Random))));
    let computer = Some(roses_net::lobby::ComputerDeck::Random);
    // (The hub has not started the room's match here: Joey is idle.)
    assert_eq!(
        l.request("Joey", &LobbyRequest::PlayComputer { map: map.clone(), computer: computer.clone() }, 0),
        [Action::PairComputer("Joey".into(), map, computer)]
    );
}
