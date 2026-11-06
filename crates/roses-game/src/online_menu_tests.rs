//! The local end-to-end test of friends online: a hub (accounts, lobby,
//! matches) on loopback with a self-signed, pinned certificate; two players
//! drive their Online Play menus with key presses: sign up with the invite,
//! see each other, challenge, accept, play the whole match as the game does
//! (`OnlineLink` with the lobby's ticket, then `DuelPlay` driven through its
//! pad by `PadBot`), come back to the lobby with the result; then log out,
//! log in, and a restarted game resumes its saved session.

use super::*;
use crate::duel_online::{ClientConn, Connector, OnlineLink, PadBot};
use crate::duel_play::DuelPlay;
use roses_net::{synthetic, ClientMessage, Join, RulesTables, ServerMessage};
use roses_server::accounts::{Accounts, PasswordCost};
use roses_server::hub::{Hub, HubAcceptor, HubData, HubSettings};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn compat() -> Compat {
    Compat::current(roses_net::data_fingerprint(&synthetic::catalogue(), &RulesTables::empty()))
}

fn until(menus: &mut [&mut OnlineMenu], what: &str, mut done: impl FnMut(&[&mut OnlineMenu]) -> bool) {
    let start = Instant::now();
    loop {
        for m in menus.iter_mut() {
            m.update();
        }
        if done(menus) {
            return;
        }
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "timed out waiting for {what}: {:?}",
            menus.iter().map(|m| (m.screen.clone(), m.note.clone())).collect::<Vec<_>>()
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn type_text(m: &mut OnlineMenu, text: &str) {
    assert!(m.wants_text(), "{:?} row {}", m.screen, m.cursor);
    for c in text.chars() {
        m.press(Key::Char(c));
    }
}

/// Move the cursor to the first row `pick` accepts.
fn select(m: &mut OnlineMenu, pick: impl Fn(&Row) -> bool) {
    while m.cursor > 0 {
        m.press(Key::Up);
    }
    let rows = m.view().rows;
    let target = rows.iter().position(pick).unwrap_or_else(|| panic!("no such row in {rows:?}"));
    for _ in 0..target {
        m.press(Key::Down);
    }
    assert_eq!(m.cursor, target);
}

fn button(t: &str) -> impl Fn(&Row) -> bool + '_ {
    move |r| matches!(r, Row::Button(b) if b == t)
}

fn sign_up(m: &mut OnlineMenu, name: &str, password: &str, invite: &str) {
    select(m, button("SIGN UP"));
    m.press(Key::Cross);
    assert_eq!(m.screen, Screen::Form { signup: true });
    type_text(m, name);
    m.press(Key::Cross);
    type_text(m, password);
    m.press(Key::Cross);
    type_text(m, &invite.to_lowercase());
    m.press(Key::Cross);
    assert_eq!(m.view().rows[m.cursor], Row::Button("SIGN UP".into()));
    m.press(Key::Cross);
    assert_eq!(m.screen, Screen::Signing);
}

/// The game's online duel for one seat, headless, to the match's end.
fn play_seat(config: OnlineConfig, ready: MatchReady, deck: DeckSubmission) -> Option<roses_core::duel::Outcome> {
    let connector: Connector =
        Box::new(move || friends::connect::<ClientMessage, ServerMessage>(&config).map(|c| Box::new(c) as ClientConn));
    let cards = synthetic::catalogue();
    let link = OnlineLink::connect(
        connector,
        &cards,
        &RulesTables::empty(),
        Join::Ticket { ticket: ready.ticket },
        Some(deck),
        Duration::from_secs(60),
        |_| {},
    )
    .expect("joined with the ticket");
    assert_eq!(link.seat, ready.seat);
    let data = crate::duel_online::tests::bare_data(synthetic::catalogue(), synthetic::board());
    let mut play = DuelPlay::online(data, link).expect("online play");
    let mut bot = PadBot::new();
    for _ in 0..400_000u32 {
        let link = play.online.as_ref().expect("online");
        if link.ended() && link.state.revision == link.shown_revision && !play.animating() {
            break;
        }
        let can = !play.animating() && link.may_act();
        let view = link.state.view.clone().expect("a view");
        let key = bot.key(&play.session.duel, &play.session.input, &view, link.state.revision, can, &cards);
        play.press(key);
        assert_eq!(play.halted, None);
        if !play.animating() && play.online.as_ref().is_some_and(|l| !l.may_act()) {
            std::thread::sleep(Duration::from_micros(200));
        }
    }
    play.session.duel.outcome
}

#[test]
fn two_players_sign_up_challenge_play_and_return_through_the_online_menu() {
    let root = std::env::temp_dir().join(format!("roses-online-menu-{:016x}", roses_server::random_u64()));
    let certs = root.join("server");
    let pin = roses_server::tls::self_signed(&certs, &["127.0.0.1".into()]).unwrap();
    let tls = roses_server::tls::TlsListener::bind("127.0.0.1:0".parse().unwrap(), &certs.join("cert.pem"), &certs.join("key.pem"))
        .unwrap();
    let addr = tls.local_addr().unwrap();
    let accounts = Accounts::in_memory().unwrap();
    let invite = accounts.create_invite(2, None, None).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let server = std::thread::spawn(move || {
        let settings = HubSettings {
            name: "Jim's server".into(),
            password_cost: PasswordCost::TEST,
            limits: roses_server::serve::Limits::UNLIMITED,
            policy: roses_net::OnlinePolicy { turn_limit: None, ..Default::default() },
            ..HubSettings::default()
        };
        let mut hub = Hub::new(settings, HubData::synthetic(false), accounts, |l| eprintln!("hub: {l}"));
        let mut acceptors: Vec<Box<dyn HubAcceptor + Send>> = vec![Box::new(tls)];
        hub.run(&mut acceptors, &flag).unwrap();
    });
    // Each player's game folder has the online.json the owner handed out.
    let config = OnlineConfig { server: addr.to_string(), name: Some("Jim's server".into()), tls: true, pin: Some(pin) };
    let decks = synthetic::submissions();
    let folder = |who: &str| {
        let dir = root.join(who);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("online.json"), serde_json::to_vec(&config).unwrap()).unwrap();
        dir.join("online.json")
    };
    let (yugi_cfg, kaiba_cfg) = (folder("yugi"), folder("kaiba"));
    let deck = |i: usize| Ok(DeckChoice { deck: decks[i].clone(), source: "test deck".into() });
    let mut yugi = OnlineMenu::new(&yugi_cfg, deck(0), compat());
    let mut kaiba = OnlineMenu::new(&kaiba_cfg, deck(1), compat());
    assert_eq!(yugi.screen, Screen::Server);
    for m in [&mut yugi, &mut kaiba] {
        m.press(Key::Cross); // CONNECT
        assert_eq!(m.screen, Screen::Connecting);
    }
    until(&mut [&mut yugi, &mut kaiba], "the account screens", |ms| ms.iter().all(|m| m.screen == Screen::Account));
    // A wrong invite first, then the real one.
    sign_up(&mut yugi, "Yugi", "heart of the cards", "WRNG-CODE");
    until(&mut [&mut yugi], "a refusal", |ms| ms[0].screen == Screen::Form { signup: true });
    assert_eq!(yugi.note.as_deref(), Some("that invite code is not valid"));
    yugi.press(Key::Circle);
    assert_eq!(yugi.screen, Screen::Account);
    yugi.invite.clear();
    yugi.name.clear();
    sign_up(&mut yugi, "Yugi", "heart of the cards", &invite);
    sign_up(&mut kaiba, "Kaiba", "blue eyes white", &invite);
    until(&mut [&mut yugi, &mut kaiba], "the lobby", |ms| {
        ms.iter().all(|m| m.screen == Screen::Lobby)
            && ms[0].view().rows.contains(&Row::Player("Kaiba".into(), PlayerState::Idle))
            && ms[1].view().rows.contains(&Row::Player("Yugi".into(), PlayerState::Idle))
    });
    // Yugi challenges Kaiba, who accepts.
    select(&mut yugi, |r| matches!(r, Row::Player(n, _) if n == "Kaiba"));
    yugi.press(Key::Cross);
    until(&mut [&mut yugi, &mut kaiba], "the challenge", |ms| {
        matches!(ms[0].screen, Screen::Sent { .. }) && matches!(ms[1].screen, Screen::Challenged { .. })
    });
    assert!(kaiba.view().info[0].contains("Yugi challenges you"));
    kaiba.press(Key::Cross); // ACCEPT
    until(&mut [&mut yugi, &mut kaiba], "the match", |ms| ms.iter().all(|m| matches!(m.request, Some(MenuRequest::Play(_)))));
    let ready = |m: &mut OnlineMenu| match m.request.take() {
        Some(MenuRequest::Play(r)) => r,
        other => panic!("{other:?}"),
    };
    let (ry, rk) = (ready(&mut yugi), ready(&mut kaiba));
    assert_eq!((ry.opponent.as_str(), rk.opponent.as_str()), ("Kaiba", "Yugi"));
    assert_eq!(yugi.screen, Screen::Starting);
    // Both play the duel as the game does; the lobby connections stay read.
    let seats: Vec<_> = [(ry.clone(), decks[usize::from(ry.seat)].clone()), (rk.clone(), decks[usize::from(rk.seat)].clone())]
        .into_iter()
        .map(|(r, d)| {
            let c = config.clone();
            std::thread::spawn(move || play_seat(c, r, d))
        })
        .collect();
    while seats.iter().any(|s| !s.is_finished()) {
        yugi.update();
        kaiba.update();
        std::thread::sleep(Duration::from_millis(5));
    }
    let outcomes: Vec<_> = seats.into_iter().map(|s| s.join().unwrap().expect("an outcome")).collect();
    assert_eq!(outcomes[0], outcomes[1]);
    // Back from the duel: the result, then the lobby.
    yugi.returned();
    kaiba.returned();
    until(&mut [&mut yugi, &mut kaiba], "the results", |ms| ms.iter().all(|m| matches!(m.screen, Screen::Result { .. })));
    let expect = |seat: u8| match outcomes[0].winner {
        None => MatchResult::Draw,
        Some(w) if w == seat => MatchResult::Won,
        Some(_) => MatchResult::Lost,
    };
    assert_eq!(yugi.screen, Screen::Result { result: expect(ry.seat) });
    assert_eq!(kaiba.screen, Screen::Result { result: expect(rk.seat) });
    yugi.press(Key::Cross);
    kaiba.press(Key::Cross);
    until(&mut [&mut yugi, &mut kaiba], "the lobby again", |ms| {
        ms.iter().all(|m| m.screen == Screen::Lobby) && ms[0].view().rows.contains(&Row::Player("Kaiba".into(), PlayerState::Idle))
    });
    // Kaiba logs out and in with the password.
    select(&mut kaiba, button("LOG OUT"));
    kaiba.press(Key::Cross);
    assert_eq!(kaiba.screen, Screen::Account);
    select(&mut kaiba, button("LOG IN"));
    kaiba.press(Key::Cross);
    // The name is kept from before; type it again.
    assert_eq!(kaiba.name, "Kaiba");
    kaiba.name.clear();
    type_text(&mut kaiba, "kaiba");
    kaiba.press(Key::Cross);
    type_text(&mut kaiba, "blue eyes white");
    kaiba.press(Key::Cross);
    kaiba.press(Key::Cross);
    until(&mut [&mut kaiba], "logged in", |ms| ms[0].screen == Screen::Lobby);
    // Yugi's game "restarts": the saved session signs in without a password.
    select(&mut yugi, button("BACK TO TITLE"));
    yugi.press(Key::Cross);
    assert_eq!(yugi.request, Some(MenuRequest::Title));
    let mut yugi = OnlineMenu::new(&yugi_cfg, deck(0), compat());
    assert_eq!(yugi.screen, Screen::Connecting, "a saved session connects at once");
    until(&mut [&mut yugi], "the session resumed", |ms| ms[0].screen == Screen::Lobby);
    stop.store(true, Ordering::Relaxed);
    server.join().unwrap();
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_launcher_session_signs_in_at_once_and_open_sign_up_hides_the_invite() {
    let root = std::env::temp_dir().join(format!("roses-online-open-{:016x}", roses_server::random_u64()));
    let certs = root.join("server");
    let pin = roses_server::tls::self_signed(&certs, &["127.0.0.1".into()]).unwrap();
    let tls = roses_server::tls::TlsListener::bind("127.0.0.1:0".parse().unwrap(), &certs.join("cert.pem"), &certs.join("key.pem"))
        .unwrap();
    let addr = tls.local_addr().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let server = std::thread::spawn(move || {
        let settings = HubSettings { password_cost: PasswordCost::TEST, open_signup: true, ..HubSettings::default() };
        let mut hub = Hub::new(settings, HubData::synthetic(false), Accounts::in_memory().unwrap(), |l| eprintln!("hub: {l}"));
        let mut acceptors: Vec<Box<dyn HubAcceptor + Send>> = vec![Box::new(tls)];
        hub.run(&mut acceptors, &flag).unwrap();
    });
    let config = OnlineConfig { server: addr.to_string(), name: Some("Roses".into()), tls: true, pin: Some(pin) };
    let data = root.join("roses-data");
    std::fs::create_dir_all(&data).unwrap();
    // The package's default online.json beside the executables.
    std::fs::write(root.join("online.json"), serde_json::to_vec(&config).unwrap()).unwrap();
    let config_path = friends::find_config(&data);
    assert_eq!(config_path, root.join("online.json"));
    let session_path = data.join(friends::SESSION_FILE);
    // The launcher's Profile page signs up on an account-only connection
    // and saves the session where the game reads it.
    let mut page = LobbyClient::start_account(friends::connect(&config).unwrap(), "launcher test").unwrap();
    page.send(LobbyRequest::SignUp { username: "Joey".into(), password: "red eyes black".into(), invite: String::new() });
    let start = Instant::now();
    while page.session.is_none() {
        page.pump();
        assert!(start.elapsed() < Duration::from_secs(20), "launcher sign-up: {:?}", page.note);
        std::thread::sleep(Duration::from_millis(5));
    }
    SavedSession { server: config.server.clone(), username: "Joey".into(), session: page.session.clone().unwrap() }
        .save(&session_path)
        .unwrap();
    drop(page);
    let deck = Ok(DeckChoice { deck: synthetic::submissions()[0].clone(), source: "test deck".into() });
    let mut joey = OnlineMenu::with_session(&config_path, &session_path, deck, compat());
    assert_eq!(joey.screen, Screen::Connecting, "signed in from the launcher: no CONNECT press");
    until(&mut [&mut joey], "the lobby", |ms| ms[0].screen == Screen::Lobby);
    assert!(joey.view().info[0].starts_with("Joey on Roses"));
    // Logged out, the sign-up form has no invite field on this server.
    select(&mut joey, button("LOG OUT"));
    joey.press(Key::Cross);
    assert!(!session_path.exists());
    until(&mut [&mut joey], "the account screen", |ms| ms[0].screen == Screen::Account);
    assert!(joey.view().info.iter().any(|l| l.contains("a password of your own")));
    select(&mut joey, button("SIGN UP"));
    joey.press(Key::Cross);
    assert!(!joey.view().rows.iter().any(|r| matches!(r, Row::Field(l, _) if l == "INVITE CODE")), "{:?}", joey.view().rows);
    type_text(&mut joey, "Mai");
    joey.press(Key::Cross);
    type_text(&mut joey, "harpie lady");
    joey.press(Key::Cross);
    assert_eq!(joey.view().rows[joey.cursor], Row::Button("SIGN UP".into()));
    joey.press(Key::Cross);
    until(&mut [&mut joey], "signed up without an invite", |ms| ms[0].screen == Screen::Lobby);
    assert_eq!(SavedSession::load(&session_path, &config.server).map(|s| s.username).as_deref(), Some("Mai"));
    stop.store(true, Ordering::Relaxed);
    server.join().unwrap();
    let _ = std::fs::remove_dir_all(&root);
}

/// PLAY COMPUTER on both seats: the server's computer takes the other seat
/// (seat 0 moves first in the synthetic setup, so with the computer there
/// the game's first presentation is the computer's turn, the case that
/// overflowed the main thread's stack on 9 Oct). Each duel runs on a thread
/// with the 1 MiB stack Windows gives the game's main thread.
#[test]
fn a_player_plays_the_server_computer_on_either_seat() {
    let root = std::env::temp_dir().join(format!("roses-online-computer-{:016x}", roses_server::random_u64()));
    let certs = root.join("server");
    let pin = roses_server::tls::self_signed(&certs, &["127.0.0.1".into()]).unwrap();
    let tls = roses_server::tls::TlsListener::bind("127.0.0.1:0".parse().unwrap(), &certs.join("cert.pem"), &certs.join("key.pem"))
        .unwrap();
    let addr = tls.local_addr().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let server = std::thread::spawn(move || {
        let settings = HubSettings {
            password_cost: PasswordCost::TEST,
            open_signup: true,
            limits: roses_server::serve::Limits::UNLIMITED,
            policy: roses_net::OnlinePolicy { turn_limit: None, ..Default::default() },
            ..HubSettings::default()
        };
        let mut hub = Hub::new(settings, HubData::synthetic(true), Accounts::in_memory().unwrap(), |l| eprintln!("hub: {l}"));
        let mut acceptors: Vec<Box<dyn HubAcceptor + Send>> = vec![Box::new(tls)];
        hub.run(&mut acceptors, &flag).unwrap();
    });
    let config = OnlineConfig { server: addr.to_string(), name: Some("Roses".into()), tls: true, pin: Some(pin) };
    let path = root.join("online.json");
    std::fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
    let deck = synthetic::submissions()[0].clone();
    let mut m = OnlineMenu::new(&path, Ok(DeckChoice { deck: deck.clone(), source: "test deck".into() }), compat());
    m.press(Key::Cross); // CONNECT
    until(&mut [&mut m], "the account screen (open sign-up)", |ms| ms[0].screen == Screen::Account && !ms[0].invite_required());
    select(&mut m, button("SIGN UP"));
    m.press(Key::Cross);
    type_text(&mut m, "Yugi");
    m.press(Key::Cross);
    type_text(&mut m, "heart of the cards");
    m.press(Key::Cross);
    select(&mut m, button("SIGN UP"));
    m.press(Key::Cross);
    until(&mut [&mut m], "the lobby", |ms| ms[0].screen == Screen::Lobby);
    // The hub draws the seats at random: play until both were played.
    let mut seats = [false; 2];
    for _ in 0..16 {
        if seats == [true; 2] {
            break;
        }
        select(&mut m, button("PLAY COMPUTER"));
        m.press(Key::Cross);
        until(&mut [&mut m], "the computer match", |ms| matches!(ms[0].request, Some(MenuRequest::Play(_))));
        let Some(MenuRequest::Play(ready)) = m.request.take() else { unreachable!() };
        assert_eq!(ready.opponent, roses_server::hub::COMPUTER_NAME);
        let seat = ready.seat;
        let (c, d) = (config.clone(), deck.clone());
        let duel = std::thread::Builder::new().stack_size(1 << 20).spawn(move || play_seat(c, ready, d)).unwrap();
        while !duel.is_finished() {
            m.update();
            std::thread::sleep(Duration::from_millis(5));
        }
        let outcome = duel.join().unwrap_or_else(|_| panic!("seat {seat}: the duel against the computer failed"));
        assert!(outcome.is_some(), "seat {seat}: no outcome");
        seats[usize::from(seat)] = true;
        m.returned();
        until(&mut [&mut m], "the result", |ms| matches!(ms[0].screen, Screen::Result { .. }));
        m.press(Key::Cross);
        until(&mut [&mut m], "the lobby again", |ms| ms[0].screen == Screen::Lobby);
    }
    assert_eq!(seats, [true; 2], "the hub never gave this player both seats");
    stop.store(true, Ordering::Relaxed);
    server.join().unwrap();
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn without_online_json_the_menu_says_where_it_goes() {
    let path = std::env::temp_dir().join("roses-no-such-dir").join("online.json");
    let mut m = OnlineMenu::new(&path, Err("no save".into()), compat());
    assert_eq!(m.screen, Screen::NoServer);
    assert!(m.view().info.iter().any(|l| l.contains("online.json")));
    // The message font has no backslash: a Windows path is shown with '/'.
    assert!(m.view().info.iter().all(|l| !l.contains('\\')));
    m.press(Key::Cross);
    assert_eq!(m.request, Some(MenuRequest::Title));
}

/// The screens drawn with the user's own files (`ROSES_DUEL_IMPORT`): raw
/// RGBA frames written to `ROSES_ONLINE_SCREEN_OUT` for a look.
#[test]
#[ignore = "Requires a private `roses import-duel` folder (ROSES_DUEL_IMPORT)"]
fn private_the_online_screens_draw_with_the_games_pieces() {
    let Some(dir) = std::env::var_os("ROSES_DUEL_IMPORT").map(std::path::PathBuf::from) else { return };
    let mut screen = crate::online_screen::OnlineScreen::from_duel_import(&dir).unwrap();
    let path = std::env::temp_dir().join("roses-online-screen-test").join("online.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, br#"{"server":"roses.example.net:7979","name":"Jim's server"}"#).unwrap();
    let deck = DeckChoice { deck: synthetic::submissions()[0].clone(), source: "MEMORY CARD slot 1, deck 1".into() };
    let mut m = OnlineMenu::new(&path, Ok(deck), compat());
    let mut views = vec![("server", m.view())];
    m.screen = Screen::Form { signup: true };
    m.name = "Yugi".into();
    m.password = "secret pw".into();
    m.cursor = 1;
    m.note = Some("that invite code is not valid".into());
    views.push(("form", m.view()));
    let mut lobby = View {
        heading: "LOBBY".into(),
        info: vec!["Yugi on Jim's server".into(), "Choose a player to challenge:".into()],
        rows: vec![
            Row::Player("Kaiba".into(), PlayerState::Idle),
            Row::Player("Joey".into(), PlayerState::InMatch),
            Row::Button("CREATE ROOM".into()),
            Row::Button("JOIN ROOM".into()),
            Row::Button("PLAY COMPUTER".into()),
            Row::Button("LOG OUT".into()),
            Row::Button("BACK TO TITLE".into()),
        ],
        cursor: 0,
        note: None,
        typing: false,
        deck: None,
        map: None,
    };
    views.push(("lobby", lobby.clone()));
    lobby.cursor = 2;
    lobby.note = Some("Kaiba declined".into());
    views.push(("lobby-note", lobby));
    let out = std::env::var_os("ROSES_ONLINE_SCREEN_OUT").map(std::path::PathBuf::from);
    for (name, view) in views {
        let notes = screen.draw(&view, true);
        assert!(notes.is_empty(), "{name}: {notes:?}");
        if let Some(out) = &out {
            std::fs::create_dir_all(out).unwrap();
            std::fs::write(out.join(format!("{name}.rgba")), screen.rgba()).unwrap();
        }
    }
}

/// The deck screens with a hub on loopback: a custom deck is made (named,
/// leader rank chosen, its cards from the deck builder), duplicated and the
/// copy deleted; PLAY COMPUTER asks for the deck first and the chosen
/// custom deck plays a whole match (the server checks it at join); the
/// choice is remembered; then Pegasus's deck (a computer opponent's) plays
/// one; an unfinished deck cannot be brought.
#[test]
fn custom_and_opponent_decks_are_chosen_and_played_through_the_online_menu() {
    use crate::online_decks::{self, DeckLibrary, DeckRef};
    use roses_net::custom_decks::{CustomDeck, CustomDecks, CUSTOM_DECKS_FILE};
    use roses_net::deck::OpponentDeck;
    let root = std::env::temp_dir().join(format!("roses-online-decks-{:016x}", roses_server::random_u64()));
    let certs = root.join("server");
    let pin = roses_server::tls::self_signed(&certs, &["127.0.0.1".into()]).unwrap();
    let tls = roses_server::tls::TlsListener::bind("127.0.0.1:0".parse().unwrap(), &certs.join("cert.pem"), &certs.join("key.pem"))
        .unwrap();
    let addr = tls.local_addr().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let server = std::thread::spawn(move || {
        let settings = HubSettings {
            password_cost: PasswordCost::TEST,
            open_signup: true,
            limits: roses_server::serve::Limits::UNLIMITED,
            policy: roses_net::OnlinePolicy { turn_limit: None, ..Default::default() },
            ..HubSettings::default()
        };
        let mut hub = Hub::new(settings, HubData::synthetic(true), Accounts::in_memory().unwrap(), |l| eprintln!("hub: {l}"));
        let mut acceptors: Vec<Box<dyn HubAcceptor + Send>> = vec![Box::new(tls)];
        hub.run(&mut acceptors, &flag).unwrap();
    });
    let config = OnlineConfig { server: addr.to_string(), name: Some("Roses".into()), tls: true, pin: Some(pin) };
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("online.json");
    std::fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
    let decks_path = root.join(CUSTOM_DECKS_FILE);
    let choice_path = root.join(online_decks::CHOICE_FILE);
    let subs = synthetic::submissions();
    // "Pegasus" plays the second synthetic deck (rank 3 leader).
    let pegasus = OpponentDeck { opponent: 9, record: 0x23, name: "Pegasus Crawford".into(), deck: subs[1].clone() };
    let library = || {
        let names = (0..30).map(|c| format!("Card {c}")).collect();
        let ranks = (0..13).map(|r| format!("R{r}")).collect();
        DeckLibrary::new(None, Some(subs[0].clone()), vec![pegasus.clone()], &decks_path, names, ranks)
            .with_maps(vec![synthetic::board(), vec![3; 49]])
    };
    let start = Ok(DeckChoice { deck: subs[0].clone(), source: "the starter deck".into() });
    let mut m = OnlineMenu::new(&path, start.clone(), compat()).with_library(library(), &choice_path);
    assert_eq!(m.deck.as_ref().unwrap().source, "STARTER DECK");
    // DECKS -> NEW DECK: the name field is chosen; type it, choose rank 3.
    select(&mut m, button("DECKS"));
    m.press(Key::Cross);
    assert_eq!(m.screen, Screen::Decks);
    select(&mut m, button("NEW DECK"));
    m.press(Key::Cross);
    assert_eq!(m.screen, Screen::DeckMenu { index: 0 });
    assert!(m.wants_text());
    for _ in 0.."NEW DECK".len() {
        m.press(Key::Backspace);
    }
    type_text(&mut m, "Dragons~");
    m.press(Key::Down); // keeps the name ("~" was refused)
    assert_eq!(m.custom(0).unwrap().name, "Dragons");
    assert_eq!(m.view().rows[m.cursor], Row::Choice("LEADER RANK".into(), "< R1 >".into()));
    m.press(Key::Right);
    m.press(Key::Right);
    m.press(Key::Left);
    m.press(Key::Right);
    assert_eq!(m.custom(0).unwrap().leader_rank, 3);
    // EDIT CARDS: the game opens the deck builder; it closes with 40 cards.
    select(&mut m, button("EDIT CARDS"));
    m.press(Key::Cross);
    assert_eq!(m.request.take(), Some(MenuRequest::EditDeck(0)));
    let built = CustomDeck { name: "ignored".into(), leader: 1, leader_rank: 12, cards: subs[1].cards.clone() };
    m.deck_edited(0, CustomDeck { cards: built.cards[..39].to_vec(), ..built.clone() });
    assert!(m.view().info.iter().any(|l| l.contains("Not ready: 39 of 40 cards")), "{:?}", m.view().info);
    m.deck_edited(0, built.clone());
    assert_eq!(m.screen, Screen::DeckMenu { index: 0 });
    assert!(m.view().info.iter().any(|l| l == "40 cards, leader rank R3"), "{:?}", m.view().info);
    // DUPLICATE, then DELETE the copy.
    select(&mut m, button("DUPLICATE"));
    m.press(Key::Cross);
    assert_eq!(m.screen, Screen::DeckMenu { index: 1 });
    assert_eq!(m.deck_name, "Dragons 2");
    select(&mut m, button("DELETE"));
    m.press(Key::Cross);
    assert_eq!(m.screen, Screen::DeleteDeck { index: 1 });
    m.press(Key::Cross);
    assert_eq!(m.screen, Screen::Decks);
    // COPY A DECK (the starter deck): "(copy)", opened in the deck builder,
    // which leaves it unfinished.
    select(&mut m, button("COPY A DECK"));
    m.press(Key::Cross);
    assert!(matches!(m.screen, Screen::DeckSelect { then: Pending::Copy }));
    select(&mut m, |r| matches!(r, Row::Deck { name, .. } if name == "STARTER DECK"));
    m.press(Key::Cross);
    assert_eq!(m.screen, Screen::DeckMenu { index: 1 });
    assert_eq!(m.request.take(), Some(MenuRequest::EditDeck(1)));
    assert_eq!(m.deck_name, "STARTER DECK (copy)");
    m.deck_edited(1, CustomDeck { cards: vec![2, 3], ..CustomDeck::new("x") });
    let saved = CustomDecks::load(&decks_path).unwrap();
    let kept: Vec<_> = saved.decks.iter().map(|d| (d.name.as_str(), d.cards.len())).collect();
    assert_eq!(kept, [("Dragons", 40), ("STARTER DECK (copy)", 2)]);
    assert_eq!(saved.decks[0].leader_rank, 3);
    m.press(Key::Circle);
    m.press(Key::Circle);
    assert_eq!(m.screen, Screen::Server);
    // Online: sign up, PLAY COMPUTER asks for the deck.
    select(&mut m, button("CONNECT"));
    m.press(Key::Cross);
    until(&mut [&mut m], "the account screen", |ms| ms[0].screen == Screen::Account && !ms[0].invite_required());
    select(&mut m, button("SIGN UP"));
    m.press(Key::Cross);
    type_text(&mut m, "Yugi");
    m.press(Key::Cross);
    type_text(&mut m, "heart of the cards");
    m.press(Key::Cross);
    select(&mut m, button("SIGN UP"));
    m.press(Key::Cross);
    until(&mut [&mut m], "the lobby", |ms| ms[0].screen == Screen::Lobby);
    // PLAY COMPUTER: this player's deck, the computer's deck (`computer`:
    // RANDOM or a deck's name), the board (`map`'s label); the match
    // names the computer `title` and is played on `board`.
    let play = |m: &mut OnlineMenu, pick: &str, expect: &roses_net::DeckSubmission, computer: &str, map: &str, title: &str, board: Option<MapChoice>| {
        select(m, button("PLAY COMPUTER"));
        m.press(Key::Cross);
        assert!(matches!(m.screen, Screen::DeckSelect { then: Pending::Computer }), "{:?}", m.screen);
        select(m, |r| matches!(r, Row::Deck { name, .. } if name == pick));
        assert!(m.view().info.iter().any(|l| l.starts_with("Leader: Card ")), "{:?}", m.view().info);
        m.press(Key::Cross);
        assert_eq!(m.screen, Screen::ComputerSelect);
        assert_eq!(&m.deck.as_ref().unwrap().deck, expect);
        select(m, |r| matches!(r, Row::Button(b) if b == computer) || matches!(r, Row::Deck { name, .. } if name == computer));
        m.press(Key::Cross);
        assert!(matches!(m.screen, Screen::MapSelect { then: Pending::Computer }), "{:?}", m.screen);
        select(m, button(map));
        assert!(m.view().map.is_some(), "the terrain preview");
        m.press(Key::Cross);
        assert_eq!(m.screen, Screen::Lobby);
        until(&mut [&mut *m], "the computer match", |ms| matches!(ms[0].request, Some(MenuRequest::Play(_))));
        let Some(MenuRequest::Play(ready)) = m.request.take() else { unreachable!() };
        assert_eq!(m.opponent_name().as_deref(), Some(title));
        let names = m.seat_names().unwrap();
        assert_eq!((names[usize::from(ready.seat)].as_str(), names[usize::from(1 - ready.seat)].as_str()), ("Yugi", title));
        match &board {
            Some(b) => assert_eq!(ready.map.as_ref(), Some(b)),
            None => assert!(matches!(ready.map, Some(MapChoice::Game { index: 0 | 1 })), "RANDOM: {:?}", ready.map),
        }
        let (c, d) = (config.clone(), m.deck.as_ref().unwrap().deck.clone());
        let duel = std::thread::Builder::new().stack_size(1 << 20).spawn(move || play_seat(c, ready, d)).unwrap();
        while !duel.is_finished() {
            m.update();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(duel.join().unwrap().is_some(), "{pick}: no outcome");
        m.returned();
        until(&mut [&mut *m], "the result", |ms| matches!(ms[0].screen, Screen::Result { .. }));
        m.press(Key::Cross);
        until(&mut [&mut *m], "the lobby again", |ms| ms[0].screen == Screen::Lobby);
    };
    let dragons = roses_net::DeckSubmission { leader: 1, leader_rank: 3, cards: subs[1].cards.clone() };
    play(&mut m, "Dragons", &dragons, "RANDOM", "CAMPAIGN MAP 02", "Synthetic Pegasus", Some(MapChoice::Game { index: 1 }));
    assert_eq!(online_decks::load_choice(&choice_path), Some(DeckRef::Custom { name: "Dragons".into() }));
    assert!(m.view().info.iter().any(|l| l == "Deck: Dragons"), "{:?}", m.view().info);
    // The next PLAY COMPUTER starts on the remembered deck.
    select(&mut m, button("PLAY COMPUTER"));
    m.press(Key::Cross);
    let at = m.view().rows[m.cursor].clone();
    assert_eq!(at, Row::Deck { name: "Dragons".into(), tag: "MINE".into(), detail: "40".into(), ready: true });
    // The unfinished deck is refused.
    let rows = m.view().rows;
    let unfinished = rows.iter().position(|r| matches!(r, Row::Deck { ready: false, .. })).unwrap();
    select(&mut m, |r| *r == rows[unfinished]);
    m.press(Key::Cross);
    assert!(matches!(m.screen, Screen::DeckSelect { .. }));
    assert!(m.note.as_deref().unwrap().starts_with("STARTER DECK (copy) is not ready: 2 of"), "{:?}", m.note);
    m.press(Key::Circle);
    assert_eq!(m.screen, Screen::Lobby);
    // Pegasus's deck.
    play(&mut m, "Pegasus Crawford", &subs[1], "Pegasus Crawford", "RANDOM", "Pegasus Crawford", None);
    assert_eq!(online_decks::load_choice(&choice_path), Some(DeckRef::Opponent { opponent: 9 }));
    // A restarted game brings the remembered deck.
    let again = OnlineMenu::new(&path, start, compat()).with_library(library(), &choice_path);
    assert_eq!(again.deck.as_ref().unwrap().source, "Pegasus Crawford");
    stop.store(true, Ordering::Relaxed);
    server.join().unwrap();
    let _ = std::fs::remove_dir_all(&root);
}

/// The deck view from every deck list (no server needed): Triangle on a
/// computer opponent's deck shows its leader, rank and cards; the cursor,
/// the sort and the detail pane's pages move; Square copies it as "Pegasus
/// Crawford (copy)" and opens the copy in the deck builder, the opponent's
/// deck unchanged; Cross in a match's deck list chooses the deck viewed;
/// while answering a challenge nothing is copied; VIEW CARDS of a custom
/// deck goes back to its menu.
#[test]
fn decks_are_viewed_and_copied_from_the_deck_lists() {
    use crate::online_decks::{DeckLibrary, Source};
    use roses_net::custom_decks::{CustomDecks, CUSTOM_DECKS_FILE};
    use roses_net::deck::OpponentDeck;
    let root = std::env::temp_dir().join(format!("roses-online-view-{:016x}", roses_server::random_u64()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("online.json");
    std::fs::write(&path, br#"{"server":"127.0.0.1:9","name":"Nowhere"}"#).unwrap();
    let subs = synthetic::submissions();
    let pegasus = OpponentDeck { opponent: 9, record: 0x23, name: "Pegasus Crawford".into(), deck: subs[1].clone() };
    let names = (0..30).map(|c| format!("Card {c}")).collect();
    let ranks = (0..13).map(|r| format!("R{r}")).collect();
    let decks_path = root.join(CUSTOM_DECKS_FILE);
    let library = DeckLibrary::new(None, Some(subs[0].clone()), vec![pegasus.clone()], &decks_path, names, ranks)
        .with_maps((0..30).map(|i| vec![(i % 10) as u8; 49]).collect());
    let start = Ok(DeckChoice { deck: subs[0].clone(), source: "the starter deck".into() });
    let mut m = OnlineMenu::new(&path, start, compat()).with_library(library, &root.join("online-deck.json"));
    select(&mut m, button("DECKS"));
    m.press(Key::Cross);
    select(&mut m, button("COPY A DECK"));
    m.press(Key::Cross);
    select(&mut m, |r| matches!(r, Row::Deck { name, .. } if name == "Pegasus Crawford"));
    let at = m.cursor;
    m.press(Key::Triangle);
    assert!(matches!(m.screen, Screen::DeckView { .. }), "{:?}", m.screen);
    let v = m.view().deck.expect("the deck view");
    assert_eq!((v.name.as_str(), v.tag.as_str(), v.leader, v.rank_name.as_str(), v.count), ("Pegasus Crawford", "CPU", subs[1].leader, "R3", 40));
    assert_eq!(v.groups.iter().map(|g| usize::from(g.1)).sum::<usize>(), 40);
    assert_eq!((v.cross.as_deref(), v.can_copy, v.problem.clone()), (Some("COPY"), true, None));
    m.press(Key::Down);
    m.press(Key::Triangle);
    assert_eq!(m.view().deck.unwrap().detail, Some(0));
    m.press(Key::Right);
    assert_eq!(m.view().deck.unwrap().detail, Some(1));
    m.press(Key::Circle);
    let v = m.view().deck.unwrap();
    assert_eq!((v.detail, v.cursor), (None, 1));
    m.press(Key::R2);
    assert_eq!(m.view().deck.unwrap().sort_label, "NAME SORT");
    // Circle twice: back to the list, on the same row.
    m.press(Key::Circle);
    assert_eq!((m.screen.clone(), m.cursor), (Screen::DeckSelect { then: Pending::Copy }, at));
    m.press(Key::Triangle);
    m.press(Key::Square);
    assert_eq!(m.request.take(), Some(MenuRequest::EditDeck(0)));
    assert_eq!(m.screen, Screen::DeckMenu { index: 0 });
    let copy = m.custom(0).unwrap().clone();
    assert_eq!((copy.name.as_str(), copy.leader, copy.leader_rank, &copy.cards), ("Pegasus Crawford (copy)", subs[1].leader, subs[1].leader_rank, &subs[1].cards));
    assert_eq!(CustomDecks::load(&decks_path).unwrap().decks, vec![copy.clone()]);
    let lib = m.library.as_ref().unwrap();
    assert_eq!(lib.opponents, vec![pegasus.clone()]);
    assert_eq!(lib.entries().iter().find(|e| e.source == Source::Opponent).unwrap().deck, Ok(subs[1].clone()));
    // VIEW CARDS of the copy, Cross = EDIT, Circle back to its menu.
    select(&mut m, button("VIEW CARDS"));
    m.press(Key::Cross);
    assert_eq!(m.view().deck.unwrap().cross.as_deref(), Some("EDIT"));
    m.press(Key::Circle);
    assert_eq!((m.screen.clone(), m.cursor), (Screen::DeckMenu { index: 0 }, 1));
    // A match's deck list: Cross in the view chooses the deck viewed.
    m.choose_deck_then(Pending::Computer);
    select(&mut m, |r| matches!(r, Row::Deck { name, .. } if name == "Pegasus Crawford"));
    m.press(Key::Triangle);
    assert_eq!(m.view().deck.unwrap().cross.as_deref(), Some("CHOOSE"));
    m.press(Key::Cross);
    assert_eq!(m.screen, Screen::Lobby);
    assert_eq!(m.deck.as_ref().unwrap().deck, subs[1]);
    // Answering a challenge: the view copies nothing.
    m.choose_deck_then(Pending::Accept { id: 7, from: "Kaiba".into() });
    select(&mut m, |r| matches!(r, Row::Deck { name, .. } if name == "Pegasus Crawford"));
    m.press(Key::Triangle);
    assert!(!m.view().deck.unwrap().can_copy);
    m.press(Key::Square);
    assert_eq!(m.note.as_deref(), Some("answer the challenge first"));
    assert_eq!(m.library.as_ref().unwrap().custom.decks.len(), 1);
    assert!(matches!(m.screen, Screen::DeckView { .. }));
    // A challenge on a board: named and previewed before answering.
    m.go(Screen::Lobby);
    m.follow(LobbyEvent::Challenged { id: 8, from: "Kaiba".into(), map: Some(MapChoice::Game { index: 0x17 }) });
    let v = m.view();
    assert_eq!(m.screen, Screen::Challenged { id: 8, from: "Kaiba".into() });
    assert!(v.info.iter().any(|l| l == "Map: VS MAP 02"), "{:?}", v.info);
    assert_eq!(v.map, Some(MapPreview { cells: Some(vec![3; 49]) }));
    // The map list: RANDOM, the VS maps, then the campaign's.
    let maps = m.library.as_ref().unwrap().map_entries();
    let labels: Vec<String> = maps.iter().map(online_decks::map_label).collect();
    assert_eq!(&labels[..3], ["RANDOM", "VS MAP 01", "VS MAP 02"]);
    assert_eq!(labels[9], "CAMPAIGN MAP 01");
    assert_eq!(labels.len(), 31);
    let _ = std::fs::remove_dir_all(&root);
}

/// The deck view, its detail pane, the map choice and a challenge with its
/// board drawn with the user's own files (`ROSES_DUEL_IMPORT`; a duel-end
/// import beside it, `ROSES_DUEL_END_IMPORT`, gives the pane its
/// pictures): nothing is left undrawn; with `ROSES_ONLINE_SCREEN_OUT` the
/// frames are written there as PNGs.
#[test]
#[ignore = "Requires a private `roses import-duel` folder (ROSES_DUEL_IMPORT)"]
fn private_the_deck_view_and_map_choice_draw_with_the_games_pieces() {
    use crate::online_decks::DeckLibrary;
    let Some(dir) = std::env::var_os("ROSES_DUEL_IMPORT").map(std::path::PathBuf::from) else { return };
    let data = crate::duel_play::load(&dir).unwrap();
    let mut screen = crate::online_screen::OnlineScreen::new(&data.executable, &data.messages, &data.texsys, &data.picmini).unwrap();
    if let Some(end) = std::env::var_os("ROSES_DUEL_END_IMPORT") {
        let files = crate::duel_end_screens::load_duel_end_import(std::path::Path::new(&end)).unwrap();
        screen.set_pictures(files.pictures, &data.executable).unwrap();
    }
    let root = std::env::temp_dir().join(format!("roses-online-view-draw-{:016x}", roses_server::random_u64()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("online.json");
    std::fs::write(&path, br#"{"server":"127.0.0.1:9","name":"Nowhere"}"#).unwrap();
    let opponents = roses_net::deck::opponent_decks(&data.executable, Some(&data.messages)).unwrap();
    assert_eq!(opponents.len(), 25);
    let first = roses_core::duel_end::message::RANK_NAME_FIRST;
    let ranks = roses_net::deck::message_texts(&data.messages, &(first..=first + 12).collect::<Vec<_>>()).unwrap();
    let maps = roses_assets::parse_pal_rules(&data.executable).unwrap().maps.into_iter().map(|m| m.cells).collect();
    let library = DeckLibrary::new(None, None, opponents, &root.join("custom-decks.json"), data.names.clone(), ranks)
        .with_cards(screen.cards().clone())
        .with_maps(maps);
    let deck = DeckChoice { deck: synthetic::submissions()[0].clone(), source: "test".into() };
    let mut m = OnlineMenu::new(&path, Ok(deck), compat()).with_library(library, &root.join("online-deck.json"));
    let mut views = Vec::new();
    m.choose_deck_then(Pending::Copy);
    select(&mut m, |r| matches!(r, Row::Deck { name, .. } if name == "Pegasus Crawford"));
    views.push(("copy-list", m.view()));
    m.press(Key::Triangle);
    views.push(("view-pegasus", m.view()));
    for _ in 0..3 {
        m.press(Key::Down);
    }
    views.push(("view-pegasus-row4", m.view()));
    m.press(Key::Triangle);
    views.push(("view-pegasus-detail", m.view()));
    m.press(Key::Right);
    views.push(("view-pegasus-detail-text", m.view()));
    m.press(Key::Circle);
    m.press(Key::R2);
    m.press(Key::R2);
    views.push(("view-pegasus-atk-sort", m.view()));
    m.press(Key::Square);
    assert_eq!(m.request.take(), Some(MenuRequest::EditDeck(0)));
    views.push(("copied-deck-menu", m.view()));
    // The map choice (as after the deck, with a server that takes it).
    m.go(Screen::MapSelect { then: Pending::Computer });
    m.cursor = 2;
    views.push(("map-choice-vs02", m.view()));
    m.cursor = 0;
    views.push(("map-choice-random", m.view()));
    m.go(Screen::ComputerSelect);
    m.cursor = 1;
    views.push(("computer-deck", m.view()));
    m.go(Screen::Lobby);
    m.follow(LobbyEvent::Challenged { id: 1, from: "Kaiba".into(), map: Some(MapChoice::Game { index: 0x18 }) });
    views.push(("challenged-with-map", m.view()));
    let out = std::env::var_os("ROSES_ONLINE_SCREEN_OUT").map(std::path::PathBuf::from);
    for (name, view) in views {
        let notes = screen.draw(&view, true);
        assert!(notes.is_empty(), "{name}: {notes:?}");
        if let Some(out) = &out {
            std::fs::create_dir_all(out).unwrap();
            let image = bevy::image::Image::new(
                bevy::render::render_resource::Extent3d { width: 512, height: 512, depth_or_array_layers: 1 },
                bevy::render::render_resource::TextureDimension::D2,
                screen.rgba(),
                bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                bevy::asset::RenderAssetUsages::MAIN_WORLD,
            );
            image.try_into_dynamic().unwrap().save(out.join(format!("{name}.png"))).unwrap();
        }
    }
    let _ = std::fs::remove_dir_all(&root);
}

/// What one headless seat or spectator saw of a watched duel.
struct Seen {
    outcome: Option<roses_core::duel::Outcome>,
    /// The HUD's online notes shown ("1 WATCHING", "SPECTATING").
    notes: std::collections::BTreeSet<String>,
    /// The copy at the end and how it stopped.
    mirror: roses_core::duel::Duel,
    halted: Option<String>,
    remote_presses: u64,
    hand_rows: u64,
    flights: u64,
}

/// A seat as `play_seat`, keeping the notes it showed; slower than the
/// spectators' updates, so they join while the duel is young.
fn play_seat_seen(config: OnlineConfig, ready: MatchReady, deck: DeckSubmission, gate: Arc<AtomicBool>) -> Seen {
    let connector: Connector =
        Box::new(move || friends::connect::<ClientMessage, ServerMessage>(&config).map(|c| Box::new(c) as ClientConn));
    let cards = synthetic::catalogue();
    let link = OnlineLink::connect(connector, &cards, &RulesTables::empty(), Join::Ticket { ticket: ready.ticket }, Some(deck), Duration::from_secs(60), |_| {})
        .expect("joined with the ticket");
    let data = crate::duel_online::tests::bare_data(synthetic::catalogue(), synthetic::board());
    let mut play = DuelPlay::online(data, link).expect("online play");
    let mut bot = PadBot::new();
    let mut notes = std::collections::BTreeSet::new();
    for _ in 0..400_000u32 {
        let link = play.online.as_ref().expect("online");
        if link.ended() && link.state.revision == link.shown_revision && !play.animating() {
            break;
        }
        // The pad waits until the spectators are in.
        let can = !play.animating() && link.may_act() && gate.load(Ordering::Relaxed);
        let view = link.state.view.clone().expect("a view");
        let key = bot.key(&play.session.duel, &play.session.input, &view, link.state.revision, can, &cards);
        play.press(key);
        assert_eq!(play.halted, None);
        notes.extend(play.online_note());
        std::thread::sleep(Duration::from_micros(if can { 1500 } else { 200 }));
    }
    Seen { outcome: play.session.duel.outcome, notes, mirror: play.session.duel.clone(), halted: None, remote_presses: 0, hand_rows: 0, flights: 0 }
}

/// A spectator as the game runs one: `OnlineLink::watch` with the lobby's
/// watch ticket, then `DuelPlay` updated with no key (or Start after
/// `leave_after` updates) to the duel's end.
fn watch_duel(config: OnlineConfig, ticket: String, leave_after: Option<u32>, joined: Arc<AtomicBool>) -> Seen {
    use roses_core::duel::ai::pad;
    let connector: Connector =
        Box::new(move || friends::connect::<ClientMessage, ServerMessage>(&config).map(|c| Box::new(c) as ClientConn));
    let cards = synthetic::catalogue();
    let link = OnlineLink::watch(connector, &cards, &RulesTables::empty(), ticket, Duration::from_secs(60)).expect("watching");
    assert!(link.spectator && !link.may_act());
    joined.store(true, Ordering::Relaxed);
    let names = link.names();
    let data = crate::duel_online::tests::bare_data(synthetic::catalogue(), synthetic::board());
    let mut play = DuelPlay::online(data, link).expect("online play");
    play.seat_names = names;
    assert!(play.spectating() && play.online_seat().is_none());
    let mut seen = Seen { outcome: None, notes: Default::default(), mirror: play.session.duel.clone(), halted: None, remote_presses: 0, hand_rows: 0, flights: 0 };
    let mut camera = crate::duel_board::BoardCamera::snapped((3, 3));
    for update in 0..600_000u32 {
        let link = play.online.as_ref().expect("online");
        if link.ended() && link.state.revision == link.shown_revision && !play.animating() {
            break;
        }
        // The copy never holds a hidden card: both hands are placeholders;
        // the HUD's context shows no hand (HIDE, no R2) though the match is
        // OPEN.
        let hidden = link.hidden;
        for side in &play.session.duel.sides {
            assert!(side.hand.iter().all(|&c| c == hidden), "a hand card reached the spectator");
        }
        let ctx = play.hud_context();
        assert_eq!((ctx.option_29, ctx.r2_held), (0, [false; 2]));
        assert!(play.unknown_hand());
        let start = leave_after.is_some_and(|n| update >= n);
        seen.hand_rows += u64::from(play.hand_anim.is_some());
        seen.flights += u64::from(play.summon_flight.is_some());
        play.press(if start { pad::START } else { 0 });
        let cursor = play.cursor();
        crate::duel_play::drive_camera(&mut camera, &mut play, cursor, &|_| None);
        camera.tick();
        seen.notes.extend(play.online_note());
        if play.halted.is_some() {
            break;
        }
        if !play.animating() {
            std::thread::sleep(Duration::from_micros(200));
        }
    }
    seen.outcome = play.session.duel.outcome;
    seen.mirror = play.session.duel.clone();
    seen.halted = play.halted.clone();
    seen.remote_presses = play.online.as_ref().map_or(0, |l| l.remote_presses);
    seen
}

/// WATCH from the Online Play lobby: two players duel through their pads
/// (streaming their presses; both sides OPEN); a third player opens WATCH
/// A DUEL, sees the duel listed by both names, watches it to the end as
/// the game does (both seats' turns presented from their presses, no hand
/// and no face-down card, SPECTATING on the HUD) and comes back to the
/// WATCH list; a fourth watches a moment and leaves with Start. The
/// players see "N WATCHING". ALLOW WATCHING is on by default and kept
/// beside the session.
#[test]
fn a_third_player_watches_a_duel_from_the_lobby_and_a_fourth_leaves_with_start() {
    let root = std::env::temp_dir().join(format!("roses-online-watch-{:016x}", roses_server::random_u64()));
    let certs = root.join("server");
    let pin = roses_server::tls::self_signed(&certs, &["127.0.0.1".into()]).unwrap();
    let tls = roses_server::tls::TlsListener::bind("127.0.0.1:0".parse().unwrap(), &certs.join("cert.pem"), &certs.join("key.pem"))
        .unwrap();
    let addr = tls.local_addr().unwrap();
    let accounts = Accounts::in_memory().unwrap();
    let invite = accounts.create_invite(4, None, None).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let server = std::thread::spawn(move || {
        let settings = HubSettings {
            password_cost: PasswordCost::TEST,
            limits: roses_server::serve::Limits::UNLIMITED,
            policy: roses_net::OnlinePolicy { turn_limit: None, ..Default::default() },
            options: roses_net::MatchOptions { leader_ability: [true; 2], card_display_open: [true; 2] },
            ..HubSettings::default()
        };
        let mut hub = Hub::new(settings, HubData::synthetic(false), accounts, |l| eprintln!("hub: {l}"));
        let mut acceptors: Vec<Box<dyn HubAcceptor + Send>> = vec![Box::new(tls)];
        hub.run(&mut acceptors, &flag).unwrap();
    });
    let config = OnlineConfig { server: addr.to_string(), name: Some("Watch test".into()), tls: true, pin: Some(pin) };
    let decks = synthetic::submissions();
    let folder = |who: &str| {
        let dir = root.join(who);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("online.json"), serde_json::to_vec(&config).unwrap()).unwrap();
        dir.join("online.json")
    };
    let deck = |i: usize| Ok(DeckChoice { deck: decks[i].clone(), source: "test deck".into() });
    let mut menus: Vec<OnlineMenu> =
        ["yugi", "kaiba", "mai", "tea"].iter().enumerate().map(|(i, w)| OnlineMenu::new(&folder(w), deck(i % 2), compat())).collect();
    for m in menus.iter_mut() {
        m.press(Key::Cross); // CONNECT
    }
    fn all(menus: &mut [OnlineMenu], what: &str, done: &dyn Fn(&[&mut OnlineMenu]) -> bool) {
        let mut refs: Vec<&mut OnlineMenu> = menus.iter_mut().collect();
        until(&mut refs, what, |ms| done(ms));
    }
    all(&mut menus, "the account screens", &|ms| ms.iter().all(|m| m.screen == Screen::Account));
    for (m, name) in menus.iter_mut().zip(["Yugi", "Kaiba", "Mai", "Tea"]) {
        sign_up(m, name, &format!("{name} secret pass"), &invite);
    }
    all(&mut menus, "the lobby", &|ms| ms.iter().all(|m| m.screen == Screen::Lobby && m.spectate()));
    // The lobby offers WATCH A DUEL and ALLOW WATCHING (on, kept on disk).
    let mai_rows = menus[2].view().rows;
    assert!(mai_rows.contains(&Row::Button("WATCH A DUEL".into())));
    assert!(mai_rows.contains(&Row::Choice("ALLOW WATCHING".into(), "< YES >".into())));
    select(&mut menus[2], |r| matches!(r, Row::Choice(l, _) if l == "ALLOW WATCHING"));
    menus[2].press(Key::Right);
    assert!(!menus[2].allow_watching);
    assert!(!crate::online_menu::load_allow_watching(&root.join("mai").join(crate::online_menu::WATCHING_FILE)));
    menus[2].press(Key::Left);
    assert!(menus[2].allow_watching);
    // Nothing to watch yet.
    select(&mut menus[2], button("WATCH A DUEL"));
    menus[2].press(Key::Cross);
    assert_eq!(menus[2].screen, Screen::Watch);
    all(&mut menus, "an empty list", &|ms| ms[2].view().info.iter().any(|l| l.starts_with("Nobody is dueling")));
    // Yugi challenges Kaiba.
    all(&mut menus, "both listed", &|ms| ms[0].view().rows.contains(&Row::Player("Kaiba".into(), PlayerState::Idle)));
    select(&mut menus[0], |r| matches!(r, Row::Player(n, _) if n == "Kaiba"));
    menus[0].press(Key::Cross);
    all(&mut menus, "the challenge", &|ms| matches!(ms[1].screen, Screen::Challenged { .. }));
    menus[1].press(Key::Cross);
    all(&mut menus, "the match", &|ms| ms[..2].iter().all(|m| matches!(m.request, Some(MenuRequest::Play(_)))));
    let ready = |m: &mut OnlineMenu| match m.request.take() {
        Some(MenuRequest::Play(r)) => r,
        other => panic!("{other:?}"),
    };
    let (ry, rk) = (ready(&mut menus[0]), ready(&mut menus[1]));
    let gate = Arc::new(AtomicBool::new(false));
    let seats: Vec<_> = [ry, rk]
        .into_iter()
        .map(|r| {
            let (c, d, g) = (config.clone(), decks[usize::from(r.seat)].clone(), gate.clone());
            std::thread::spawn(move || play_seat_seen(c, r, d, g))
        })
        .collect();
    let joined = [Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false))];
    // Mai's list refreshes by itself and shows the duel by both names.
    all(&mut menus, "the duel listed", &|ms| ms[2].view().rows.iter().any(|r| matches!(r, Row::Live { .. })));
    let title = match &menus[2].view().rows[0] {
        Row::Live { title, round, watching } => {
            assert_eq!(*watching, 0);
            assert!(*round >= 1);
            title.clone()
        }
        other => panic!("{other:?}"),
    };
    assert!(title.contains("Yugi") && title.contains("Kaiba"), "{title}");
    assert_eq!(menus[2].view().heading, "WATCH");
    menus[2].press(Key::Cross);
    all(&mut menus, "Mai may watch", &|ms| matches!(ms[2].request, Some(MenuRequest::Watch(_))));
    assert_eq!(menus[2].screen, Screen::WatchStarting);
    let Some(MenuRequest::Watch(w)) = menus[2].request.take() else { unreachable!() };
    assert_eq!(menus[2].seat_names(), Some(w.players.clone()));
    let mai = {
        let c = config.clone();
        let j = joined[0].clone();
        std::thread::spawn(move || watch_duel(c, w.ticket, None, j))
    };
    // Tea watches too, then leaves with Start.
    select(&mut menus[3], button("WATCH A DUEL"));
    menus[3].press(Key::Cross);
    all(&mut menus, "Tea's list", &|ms| ms[3].view().rows.iter().any(|r| matches!(r, Row::Live { .. })));
    select(&mut menus[3], |r| matches!(r, Row::Live { .. }));
    menus[3].press(Key::Cross);
    all(&mut menus, "Tea may watch", &|ms| matches!(ms[3].request, Some(MenuRequest::Watch(_))));
    let Some(MenuRequest::Watch(tw)) = menus[3].request.take() else { unreachable!() };
    let tea = {
        let c = config.clone();
        let j = joined[1].clone();
        std::thread::spawn(move || watch_duel(c, tw.ticket, Some(200), j))
    };
    // Both watch from the duel's start; then the players' pads go.
    let start = Instant::now();
    while !joined.iter().all(|j| j.load(Ordering::Relaxed)) {
        assert!(start.elapsed() < Duration::from_secs(30), "the spectators did not get in");
        for m in menus.iter_mut() {
            m.update();
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    gate.store(true, Ordering::Relaxed);
    while seats.iter().any(|s| !s.is_finished()) || !mai.is_finished() || !tea.is_finished() {
        for m in menus.iter_mut() {
            m.update();
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let seats: Vec<Seen> = seats.into_iter().map(|s| s.join().unwrap()).collect();
    let (mai, tea) = (mai.join().unwrap(), tea.join().unwrap());
    // Tea left with Start: back on the WATCH list.
    assert_eq!(tea.halted.as_deref(), Some(crate::duel_online::STOPPED_WATCHING));
    menus[3].returned();
    assert_eq!(menus[3].screen, Screen::Watch);
    assert_eq!(menus[3].watching, None);
    // Mai watched to the end: the same outcome, both seats' turns from
    // their presses (hand rows, flights), and what Mai's copy shows of the
    // board is what the players' copies show.
    assert_eq!(mai.halted, None);
    assert!(mai.outcome.is_some());
    assert_eq!(mai.outcome, seats[0].outcome);
    assert_eq!(seats[0].outcome, seats[1].outcome);
    eprintln!(
        "Mai: {} presses presented, {} hand-row and {} flight updates, notes {:?}",
        mai.remote_presses, mai.hand_rows, mai.flights, mai.notes
    );
    assert!(mai.remote_presses > 10 && mai.hand_rows > 0 && mai.flights > 0);
    assert!(mai.notes.contains("SPECTATING"));
    for s in &seats {
        assert_eq!(roses_net::spectator_view(&mai.mirror), roses_net::spectator_view(&s.mirror));
        eprintln!("a seat's notes: {:?}", s.notes);
        assert!(s.notes.iter().any(|n| n.ends_with("WATCHING")), "{:?}", s.notes);
    }
    menus[2].returned();
    assert_eq!(menus[2].screen, Screen::Watch);
    all(&mut menus, "the ended duel unlisted", &|ms| !ms[2].view().rows.iter().any(|r| matches!(r, Row::Live { .. })));
    menus[2].press(Key::Circle);
    assert_eq!(menus[2].screen, Screen::Lobby);
    stop.store(true, Ordering::Relaxed);
    server.join().unwrap();
    let _ = std::fs::remove_dir_all(&root);
}

// ---- messages, the saved session, the lobby's cursor, names (5 Nov review) ----

/// A menu on a lobby the test plays the server of.
fn menu_on_fake_lobby(tag: &str) -> (OnlineMenu, roses_net::transport::Memory<LobbyEvent, LobbyRequest>) {
    let root = std::env::temp_dir().join(format!("roses-menu-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let config = OnlineConfig { server: "127.0.0.1:1".into(), name: None, tls: false, pin: None };
    let path = root.join("online.json");
    std::fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
    let mut m = OnlineMenu::new(&path, Err("no save".into()), compat());
    let (client, server) = roses_net::transport::memory_pair::<LobbyRequest, LobbyEvent>();
    m.lobby = Some(LobbyClient::start(Box::new(client), compat(), "test").unwrap());
    (m, server)
}

#[test]
fn a_version_refusal_says_so_when_the_server_closes_right_after_it() {
    use roses_net::lobby::LobbyRefusal;
    use roses_net::transport::Connection;
    let (mut m, mut server) = menu_on_fake_lobby("refused");
    m.screen = Screen::Account;
    server.send(&LobbyEvent::LobbyRefused { reason: LobbyRefusal::LobbyVersion { client: 1, server: 2 } }).unwrap();
    server.close();
    m.update();
    assert_eq!(m.screen, Screen::Server);
    assert!(m.note.as_deref().is_some_and(|n| n.contains("update the game")), "{:?}", m.note);
}

#[test]
fn only_a_session_the_server_refuses_is_forgotten() {
    use roses_net::transport::Connection;
    let (mut m, mut server) = menu_on_fake_lobby("session");
    for problem in [AuthProblem::Busy, AuthProblem::TooManyAttempts, AuthProblem::EmailNotVerified] {
        m.save_session("Yugi", "token");
        m.resuming = true;
        m.screen = Screen::Signing;
        server.send(&LobbyEvent::AuthFailed { problem }).unwrap();
        m.update();
        assert!(m.saved_session().is_some(), "{problem:?} says nothing about the session");
        assert_eq!(m.screen, Screen::Account);
        assert_eq!(m.note, Some(auth_note(problem)));
    }
    m.resuming = true;
    server.send(&LobbyEvent::AuthFailed { problem: AuthProblem::SessionExpired }).unwrap();
    m.update();
    assert!(m.saved_session().is_none());
    assert_eq!(m.note.as_deref(), Some("please log in"));
    // A resumed sign-in is over once signed in: a later refusal is shown as it is.
    m.save_session("Yugi", "token");
    m.resuming = true;
    server.send(&LobbyEvent::SignedIn { username: "Yugi".into(), session: "token".into() }).unwrap();
    m.update();
    assert!(!m.resuming);
}

#[test]
fn every_sign_in_problem_fits_the_note_line_whole() {
    for p in [
        AuthProblem::BadUsername,
        AuthProblem::WeakPassword,
        AuthProblem::UsernameTaken,
        AuthProblem::BadInvite,
        AuthProblem::BadCredentials,
        AuthProblem::SessionExpired,
        AuthProblem::Disabled,
        AuthProblem::TooManyAttempts,
        AuthProblem::Busy,
        AuthProblem::NotSignedIn,
        AuthProblem::ReservedName,
        AuthProblem::SignUpLimit,
        AuthProblem::EmailNotVerified,
    ] {
        let note = auth_note(p);
        assert!(note.chars().count() < LINE_MAX || note == p.to_string(), "{p:?}: {note}");
        assert!(p.to_string().chars().count() <= LINE_MAX || note != cut(&p.to_string()), "{p:?} would be cut: {note}");
    }
}

#[test]
fn the_lobby_cursor_stays_on_its_button_as_players_come_and_go() {
    use roses_net::lobby::PlayerEntry;
    use roses_net::transport::Connection;
    let (mut m, mut server) = menu_on_fake_lobby("cursor");
    m.lobby.as_mut().unwrap().user = Some("Yugi".into());
    m.screen = Screen::Lobby;
    let list = |names: &[&str]| LobbyEvent::Players {
        players: names.iter().map(|n| PlayerEntry { name: (*n).into(), state: PlayerState::Idle }).collect(),
    };
    server.send(&list(&["Yugi", "Kaiba", "Joey"])).unwrap();
    m.update();
    select(&mut m, button("LOG OUT"));
    server.send(&list(&["Yugi", "Joey"])).unwrap();
    m.update();
    assert_eq!(m.view().rows[m.cursor], Row::Button("LOG OUT".into()), "one left");
    server.send(&list(&["Yugi", "Joey", "Mai", "Tea"])).unwrap();
    m.update();
    assert_eq!(m.view().rows[m.cursor], Row::Button("LOG OUT".into()), "two came");
    // The last row stays a row.
    select(&mut m, button("BACK TO TITLE"));
    server.send(&list(&["Yugi"])).unwrap();
    m.update();
    assert_eq!(m.view().rows[m.cursor], Row::Button("BACK TO TITLE".into()));
    // On a player's row the cursor stays on the rows there are.
    server.send(&list(&["Yugi", "Joey", "Mai"])).unwrap();
    m.update();
    select(&mut m, |r| matches!(r, Row::Player(n, _) if n == "Mai"));
    server.send(&list(&["Yugi", "Joey"])).unwrap();
    m.update();
    assert!(m.cursor < m.view().rows.len());
}

#[test]
fn four_long_names_are_readable_on_the_n_player_screens() {
    let seats: Vec<String> = ["Christopher1", "Alexandra123", "Maximilian99", "Pegasus Crawford"].map(String::from).to_vec();
    let lines = name_lines(&seats);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines.iter().all(|l| l.chars().count() <= LINE_MAX), "{lines:?}");
    assert!(seats.iter().all(|s| lines.iter().any(|l| l.contains(s.as_str()))), "no name is cut: {lines:?}");
    let live = LiveMatch {
        match_id: "m".into(),
        players: [seats[0].clone(), seats[1].clone()],
        map: None,
        round: 3,
        watching: 0,
        seats: seats.clone(),
        mode: None,
    };
    // A WATCH row holds about 14 letters: the four names were squeezed in.
    assert_eq!(OnlineMenu::live_row_title(&live), "Christopher1 &3");
    // Every letter of it is in the font: an unknown one reads as '?'.
    assert!(OnlineMenu::live_row_title(&live).chars().all(|c| c == ' ' || crate::online_title::font_cell(c).is_some()));
    let two = LiveMatch { seats: Vec::new(), ..live.clone() };
    assert_eq!(OnlineMenu::live_row_title(&two), "Christopher1 vs Alexandra123");
    // Joining as a spectator names every seat, not the first two.
    let (mut m, _server) = menu_on_fake_lobby("names");
    m.watching = Some(WatchReady { match_id: "m".into(), ticket: "t".into(), players: live.players.clone(), map: None, options: Default::default() });
    m.watch_seats = Some(seats.clone());
    m.screen = Screen::WatchStarting;
    assert!(m.view().info.iter().any(|l| l.contains("Pegasus Crawford")), "{:?}", m.view().info);
}

#[test]
fn an_unreachable_server_reads_as_a_whole_line() {
    let note = unreachable_note("No connection could be made because the target machine actively refused it. (os error 10061)");
    assert!(note.chars().count() <= LINE_MAX, "{note}");
    assert_eq!(cut(&note), note);
}

#[test]
fn the_lobby_says_so_while_the_server_still_holds_a_match_that_failed_to_join() {
    use roses_net::lobby::{PlayerEntry, PlayerState};
    use roses_net::transport::Connection;
    let (mut m, mut server) = menu_on_fake_lobby("held");
    let list = |state| LobbyEvent::Players {
        players: vec![
            PlayerEntry { name: "Yugi".into(), state },
            PlayerEntry { name: "Kaiba".into(), state: PlayerState::Idle },
        ],
    };
    m.lobby.as_mut().unwrap().user = Some("Yugi".into());
    server.send(&list(PlayerState::InMatch)).unwrap();
    m.update();
    m.match_failed("could not join");
    assert_eq!(m.screen, Screen::Lobby);
    assert!(m.view().info.iter().any(|l| l.contains("still closing")), "{:?}", m.view().info);
    // Nothing new starts, not even the deck choice before a refusal.
    m.act(Action::CreateRoom);
    assert_eq!(m.screen, Screen::Lobby);
    assert!(m.note.as_deref().is_some_and(|n| n.contains("last match")), "{:?}", m.note);
    // The server let go: the lobby is the lobby again.
    server.send(&list(PlayerState::Idle)).unwrap();
    m.update();
    assert!(!m.view().info.iter().any(|l| l.contains("still closing")));
    m.note = None;
    m.act(Action::CreateRoom);
    assert!(!m.note.as_deref().is_some_and(|n| n.contains("last match")), "{:?}", m.note);
}
