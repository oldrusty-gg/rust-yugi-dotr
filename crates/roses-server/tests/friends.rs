//! The friends-online server end to end over loopback: a hub with a
//! self-signed TLS listener (clients pin its fingerprint) and a plain
//! loopback listener; two clients sign up with an invite, log in, see each
//! other, one challenges, the other accepts, both play the match over TLS
//! with their tickets and come back to the lobby; a wrong pin, an old
//! client and a non-loopback address without TLS are refused.

use roses_net::friends::{connect, LobbyClient, OnlineConfig};
use roses_net::lobby::{LobbyEvent, LobbyRefusal, LobbyRequest, PlayerState, LOBBY_VERSION};
use roses_net::transport::{recv_timeout, Connection, Received};
use roses_net::{synthetic, ClientMessage, Compat, Join, ServerMessage};
use roses_server::accounts::{Accounts, PasswordCost};
use roses_server::hub::{Hub, HubAcceptor, HubData, HubSettings};
use roses_server::play::{play_bot, PlayOptions};
use roses_server::tls::{self_signed, TlsListener};
use roses_net::transport::LoopbackListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn compat() -> Compat {
    Compat::current(roses_net::data_fingerprint(&synthetic::catalogue(), &roses_net::RulesTables::empty()))
}

fn until(c: &mut LobbyClient, what: &str, mut done: impl FnMut(&LobbyClient) -> bool) {
    let start = Instant::now();
    loop {
        c.pump();
        if done(c) {
            return;
        }
        assert!(start.elapsed() < Duration::from_secs(20), "timed out waiting for {what} (note {:?})", c.note);
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn sees(c: &LobbyClient, name: &str, state: PlayerState) -> bool {
    c.players.iter().any(|p| p.name == name && p.state == state)
}

#[test]
fn two_friends_sign_up_challenge_play_over_tls_and_return_to_the_lobby() {
    let dir = std::env::temp_dir().join(format!("roses-friends-{:016x}", roses_server::random_u64()));
    let pin = self_signed(&dir, &["127.0.0.1".into(), "localhost".into()]).unwrap();
    let tls = TlsListener::bind("127.0.0.1:0".parse().unwrap(), &dir.join("cert.pem"), &dir.join("key.pem")).unwrap();
    let tls_addr = tls.local_addr().unwrap();
    let plain = LoopbackListener::bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let plain_addr = plain.local_addr().unwrap();
    let accounts = Accounts::open(&dir.join("accounts.db")).unwrap();
    let invite = accounts.create_invite(2, Some("test"), Some(1)).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let server = std::thread::spawn(move || {
        let settings = HubSettings {
            name: "Jim's test server".into(),
            password_cost: PasswordCost::TEST,
            replay_dir: Some(std::env::temp_dir().join("roses-friends-replays")),
            ..HubSettings::default()
        };
        let mut hub = Hub::new(settings, HubData::synthetic(false), accounts, |l| eprintln!("hub: {l}"));
        let mut acceptors: Vec<Box<dyn HubAcceptor + Send>> = vec![Box::new(tls), Box::new(plain)];
        hub.run(&mut acceptors, &flag).unwrap();
    });
    let pinned = OnlineConfig { server: tls_addr.to_string(), name: None, tls: true, pin: Some(pin.clone()) };
    let local = OnlineConfig { server: plain_addr.to_string(), name: None, tls: false, pin: None };

    // Refusals before any account: a wrong pin, plain text to the TLS
    // port, no TLS to another host, an old lobby version.
    let wrong = OnlineConfig { pin: Some(format!("sha256:{}", "00".repeat(32))), ..pinned.clone() };
    let e = connect::<LobbyRequest, LobbyEvent>(&wrong).err().expect("a wrong pin is refused");
    assert!(e.to_string().contains("not the pinned one"), "{e}");
    let e = connect::<LobbyRequest, LobbyEvent>(&OnlineConfig { server: "192.0.2.1:7979".into(), tls: false, name: None, pin: None })
        .err()
        .expect("no plain text off this machine");
    assert!(e.to_string().contains("loopback"), "{e}");
    let mut old = connect::<LobbyRequest, LobbyEvent>(&local).unwrap();
    old.send(&LobbyRequest::LobbyHello { lobby_version: LOBBY_VERSION + 1, compat: compat(), client: "old".into(), account: false, modes: false }).unwrap();
    assert!(matches!(
        recv_timeout(&mut old, Duration::from_secs(10)),
        Received::Message(LobbyEvent::LobbyRefused { reason: LobbyRefusal::LobbyVersion { .. } })
    ));

    // Yugi over TLS (pinned), Kaiba on the server's own PC (loopback).
    let mut yugi = LobbyClient::start(connect(&pinned).unwrap(), compat(), "test").unwrap();
    let mut kaiba = LobbyClient::start(connect(&local).unwrap(), compat(), "test").unwrap();
    yugi.send(LobbyRequest::SignUp { username: "Yugi".into(), password: "heart of the cards".into(), invite: invite.clone() });
    kaiba.send(LobbyRequest::SignUp { username: "Kaiba".into(), password: "blue eyes white".into(), invite });
    until(&mut yugi, "sign-up", |c| c.user.is_some());
    until(&mut kaiba, "sign-up", |c| c.user.is_some());
    assert_eq!(yugi.server, Some(("Jim's test server".into(), false)));
    // Log in again with the password on a fresh TLS connection.
    yugi.send(LobbyRequest::LogOut);
    until(&mut yugi, "log-out", |c| c.user.is_none());
    let mut yugi = LobbyClient::start(connect(&pinned).unwrap(), compat(), "test").unwrap();
    yugi.send(LobbyRequest::LogIn { username: "yugi".into(), password: "heart of the cards".into() });
    until(&mut yugi, "log-in", |c| c.user.as_deref() == Some("Yugi"));
    until(&mut yugi, "see Kaiba", |c| sees(c, "Kaiba", PlayerState::Idle) && sees(c, "Yugi", PlayerState::Idle));
    until(&mut kaiba, "see Yugi", |c| sees(c, "Yugi", PlayerState::Idle));

    yugi.send(LobbyRequest::Challenge { to: "Kaiba".into(), map: None });
    until(&mut kaiba, "the challenge", |c| c.incoming.iter().any(|(_, f)| f == "Yugi"));
    let id = kaiba.incoming[0].0;
    kaiba.send(LobbyRequest::Answer { id, accept: true });
    until(&mut yugi, "match ready", |c| c.ready.is_some());
    until(&mut kaiba, "match ready", |c| c.ready.is_some());

    // Each plays its seat with its ticket on a match connection of its own.
    let decks = synthetic::submissions();
    let seats: Vec<_> = [(yugi.ready.clone().unwrap(), pinned.clone()), (kaiba.ready.clone().unwrap(), local.clone())]
        .into_iter()
        .map(|(ready, config)| {
            let deck = decks[usize::from(ready.seat)].clone();
            std::thread::spawn(move || {
                play_bot(
                    || connect::<ClientMessage, ServerMessage>(&config),
                    &compat(),
                    &synthetic::catalogue(),
                    PlayOptions { join: Join::Ticket { ticket: ready.ticket }, deck: Some(deck), ..PlayOptions::default() },
                    |_| {},
                )
            })
        })
        .collect();
    let mut outcomes = Vec::new();
    // Keep the lobby connections read while the match runs.
    for s in seats {
        while !s.is_finished() {
            yugi.pump();
            kaiba.pump();
            std::thread::sleep(Duration::from_millis(5));
        }
        outcomes.push(s.join().unwrap().unwrap().state.ended.unwrap().0);
    }
    assert_eq!(outcomes[0], outcomes[1]);
    until(&mut yugi, "match over", |c| c.over.is_some() && sees(c, "Kaiba", PlayerState::Idle) && sees(c, "Yugi", PlayerState::Idle));
    until(&mut kaiba, "match over", |c| c.over.is_some());
    assert!(yugi.connected() && kaiba.connected(), "the lobby connections stayed up through the match");
    stop.store(true, Ordering::Relaxed);
    server.join().unwrap();
    let accounts = Accounts::open(&dir.join("accounts.db")).unwrap();
    assert_eq!(accounts.matches(5).unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_ticket_is_unknown_to_the_single_match_server() {
    // The single-match server never invented tickets: refused, no seat.
    let cards = synthetic::catalogue();
    let mut m = roses_server::OnlineMatch::with_submitted_decks(
        "m",
        HubData::synthetic(false).setup.as_ref()(1),
        cards,
        Default::default(),
        [roses_server::secret_token(), roses_server::secret_token()],
    )
    .unwrap();
    let r = m.hello(&compat(), &Join::Ticket { ticket: "x".into() }, 0);
    assert_eq!(r.unwrap_err(), roses_net::Refusal::BadToken);
}
