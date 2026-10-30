//! MODES (a port addition): the Online Play screens of the N-player rooms,
//! against a hub on loopback: the room setup's settings, creating a room,
//! joining it by its code, changing seats (2v2: teams), the host's
//! computer seats and settings, START, and both players sent to the match.

use super::*;
use roses_net::{synthetic, RulesTables};
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

fn setting(t: &str) -> impl Fn(&Row) -> bool + '_ {
    move |r| matches!(r, Row::Choice(l, _) if l == t)
}

fn value(m: &OnlineMenu, label: &str) -> String {
    m.view()
        .rows
        .iter()
        .find_map(|r| match r {
            Row::Choice(l, v) if l == label => Some(v.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no {label} row"))
}

fn seat_row(m: &OnlineMenu, letter: &str) -> Row {
    m.view()
        .rows
        .into_iter()
        .find(|r| matches!(r, Row::Seat { seat, .. } if seat == letter))
        .unwrap_or_else(|| panic!("no seat {letter}"))
}

#[test]
fn room_settings_step_through_the_choices_and_wrap() {
    let mut s = RoomSettings::default();
    assert_eq!((s.mode, s.cards_leave, s.start_lp, s.shared_lp, s.turn_seconds, s.spectators), (RoomMode::FreeForAll, true, 4000, false, 90, true));
    step_setting(&mut s, ModeRow::StartLp, 1);
    assert_eq!(s.start_lp, 6000);
    step_setting(&mut s, ModeRow::StartLp, 1);
    step_setting(&mut s, ModeRow::StartLp, 1);
    assert_eq!(s.start_lp, 9999);
    step_setting(&mut s, ModeRow::StartLp, 1);
    assert_eq!(s.start_lp, 1000, "wraps");
    step_setting(&mut s, ModeRow::TurnTimer, -1);
    assert_eq!(s.turn_seconds, 60);
    step_setting(&mut s, ModeRow::TurnTimer, -1);
    assert_eq!(s.turn_seconds, 0, "no clock");
    // Shared LP is a 2v2 setting only.
    step_setting(&mut s, ModeRow::SharedLp, 1);
    assert!(!s.shared_lp);
    step_setting(&mut s, ModeRow::Mode, 1);
    step_setting(&mut s, ModeRow::SharedLp, 1);
    assert_eq!((s.mode, s.shared_lp), (RoomMode::Teams, true));
    step_setting(&mut s, ModeRow::Mode, 1);
    assert_eq!((s.mode, s.shared_lp), (RoomMode::FreeForAll, false));
    step_setting(&mut s, ModeRow::CardsLeave, 1);
    step_setting(&mut s, ModeRow::Spectators, -1);
    assert!(!s.cards_leave && !s.spectators);
    assert!(settings_line(&s).starts_with("FREE-FOR-ALL, 1000 LP, no clock, cards stay"));
}

#[test]
fn modes_is_offered_only_by_a_server_that_says_modes_and_set_up_in_its_screen() {
    let dir = std::env::temp_dir().join(format!("roses-modes-menu-{:016x}", roses_server::random_u64()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut m = OnlineMenu::new(&dir.join("online.json"), Err("no deck".into()), compat());
    // No lobby: no MODES row anywhere, and the setup screen's rows.
    assert!(!m.modes());
    m.go(Screen::ModeSetup { editing: false });
    let rows = m.view().rows;
    let labels: Vec<String> = rows
        .iter()
        .map(|r| match r {
            Row::Choice(l, _) | Row::Button(l) => l.clone(),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(labels, ["MODE", "KNOCKED-OUT CARDS", "START LP", "TURN TIMER", "SPECTATORS", "CREATE ROOM", "JOIN ROOM", "BACK"]);
    assert_eq!(m.view().heading, "MODES");
    assert!(m.view().info[0].starts_with("INVENTED MODE"));
    // Left / Right and Cross change a setting; 2v2 adds TEAM LP.
    select(&mut m, setting("MODE"));
    m.press(Key::Right);
    assert_eq!(value(&m, "MODE"), "< 2 VS 2 >");
    select(&mut m, setting("TEAM LP"));
    m.press(Key::Cross);
    assert_eq!(value(&m, "TEAM LP"), "< SHARED >");
    select(&mut m, setting("START LP"));
    m.press(Key::Left);
    assert_eq!(value(&m, "START LP"), "< 2000 >");
    select(&mut m, setting("KNOCKED-OUT CARDS"));
    m.press(Key::Right);
    assert_eq!(value(&m, "KNOCKED-OUT CARDS"), "< STAY >");
    assert_eq!(
        m.mode_settings,
        RoomSettings { mode: RoomMode::Teams, cards_leave: false, start_lp: 2000, shared_lp: true, ..RoomSettings::default() }
    );
    // Circle: back to the lobby.
    m.press(Key::Circle);
    assert_eq!(m.screen, Screen::Lobby);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn two_players_set_up_a_2v2_room_change_seats_and_start_it() {
    let root = std::env::temp_dir().join(format!("roses-modes-{:016x}", roses_server::random_u64()));
    let listener = roses_net::transport::LoopbackListener::bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();
    let accounts = Accounts::in_memory().unwrap();
    let invite = accounts.create_invite(2, None, None).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let server = std::thread::spawn(move || {
        let settings = HubSettings {
            name: "Modes".into(),
            password_cost: PasswordCost::TEST,
            limits: roses_server::serve::Limits::UNLIMITED,
            ..HubSettings::default()
        };
        let mut hub = Hub::new(settings, HubData::synthetic(true), accounts, |l| eprintln!("hub: {l}"));
        let mut acceptors: Vec<Box<dyn HubAcceptor + Send>> = vec![Box::new(listener)];
        hub.run(&mut acceptors, &flag).unwrap();
    });
    let config = OnlineConfig { server: addr.to_string(), name: Some("Modes".into()), tls: false, pin: None };
    let decks = synthetic::submissions();
    let folder = |who: &str| {
        let dir = root.join(who);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("online.json"), serde_json::to_vec(&config).unwrap()).unwrap();
        dir.join("online.json")
    };
    let deck = |i: usize| Ok(DeckChoice { deck: decks[i].clone(), source: "test deck".into() });
    let mut host = OnlineMenu::new(&folder("host"), deck(0), compat());
    let mut guest = OnlineMenu::new(&folder("guest"), deck(1), compat());
    for m in [&mut host, &mut guest] {
        m.press(Key::Cross); // CONNECT
    }
    until(&mut [&mut host, &mut guest], "the account screens", |ms| ms.iter().all(|m| m.screen == Screen::Account));
    for (m, name) in [(&mut host, "Hosta"), (&mut guest, "Guestb")] {
        select(m, button("SIGN UP"));
        m.press(Key::Cross);
        for c in name.chars() {
            m.press(Key::Char(c));
        }
        m.press(Key::Cross);
        for c in "a long password".chars() {
            m.press(Key::Char(c));
        }
        m.press(Key::Cross);
        if m.wants_text() {
            for c in invite.to_lowercase().chars() {
                m.press(Key::Char(c));
            }
            m.press(Key::Cross);
        }
        m.press(Key::Cross);
    }
    until(&mut [&mut host, &mut guest], "the lobby", |ms| ms.iter().all(|m| m.screen == Screen::Lobby));
    assert!(host.modes(), "the hub offers MODES");
    // The host sets up a 2v2 room with shared LP and opens it.
    select(&mut host, button("MODES"));
    host.press(Key::Cross);
    assert_eq!(host.screen, Screen::ModeSetup { editing: false });
    select(&mut host, setting("MODE"));
    host.press(Key::Right);
    select(&mut host, setting("TEAM LP"));
    host.press(Key::Right);
    select(&mut host, button("CREATE ROOM"));
    host.press(Key::Cross);
    until(&mut [&mut host], "the room", |ms| ms[0].mode_room().is_some());
    assert_eq!(host.screen, Screen::ModeRoom);
    assert!(host.is_host());
    let room = host.mode_room().unwrap().clone();
    assert_eq!((room.settings.mode, room.settings.shared_lp), (RoomMode::Teams, true));
    assert_eq!(seat_row(&host, "S"), Row::Seat { seat: "S".into(), name: "Hosta".into(), team: Some("TEAM A".into()), you: true });
    assert!(host.view().info.iter().any(|l| l.contains(&room.code)));
    // The guest joins by the code (the first open seat, N: team A) ...
    select(&mut guest, button("MODES"));
    guest.press(Key::Cross);
    select(&mut guest, button("JOIN ROOM"));
    guest.press(Key::Cross);
    assert_eq!(guest.screen, Screen::ModeRoomCode);
    for c in room.code.chars() {
        guest.press(Key::Char(c));
    }
    select(&mut guest, button("JOIN"));
    guest.press(Key::Cross);
    until(&mut [&mut host, &mut guest], "the guest seated", |ms| {
        ms.iter().all(|m| m.mode_room().is_some_and(|r| r.seats.iter().filter(|s| s.player.is_some()).count() == 2))
    });
    assert!(!guest.is_host());
    // ... and moves to E: the other team.
    select(&mut guest, |r| matches!(r, Row::Seat { seat, .. } if seat == "E"));
    guest.press(Key::Cross);
    until(&mut [&mut host, &mut guest], "the guest at E", |ms| {
        ms[0].mode_room().is_some_and(|r| r.seats[2].player.as_deref() == Some("Guestb"))
    });
    assert_eq!(seat_row(&guest, "E"), Row::Seat { seat: "E".into(), name: "Guestb".into(), team: Some("TEAM B".into()), you: true });
    // Only the host gives a seat to the computer.
    select(&mut guest, |r| matches!(r, Row::Seat { seat, .. } if seat == "W"));
    guest.press(Key::Square);
    assert_eq!(guest.note.as_deref(), Some("only the room's host can do that"));
    select(&mut host, |r| matches!(r, Row::Seat { seat, .. } if seat == "W"));
    host.press(Key::Square);
    until(&mut [&mut host, &mut guest], "W to the computer", |ms| ms.iter().all(|m| m.mode_room().is_some_and(|r| r.seats[3].computer)));
    assert!(matches!(seat_row(&guest, "W"), Row::Seat { name, .. } if name == "COMPUTER"));
    // The host's settings: START LP up.
    select(&mut host, button("SETTINGS"));
    host.press(Key::Cross);
    assert_eq!(host.screen, Screen::ModeSetup { editing: true });
    select(&mut host, setting("START LP"));
    host.press(Key::Right);
    select(&mut host, button("APPLY"));
    host.press(Key::Cross);
    until(&mut [&mut host, &mut guest], "the new settings", |ms| ms.iter().all(|m| m.mode_room().is_some_and(|r| r.settings.start_lp == 6000)));
    assert_eq!(host.screen, Screen::ModeRoom);
    // START: the open seat gets a computer; both are sent to the match.
    assert!(guest.view().rows.iter().all(|r| r != &Row::Button("START".into())), "only the host starts");
    select(&mut host, button("START"));
    host.press(Key::Cross);
    until(&mut [&mut host, &mut guest], "the match", |ms| ms.iter().all(|m| matches!(m.request, Some(MenuRequest::PlayMulti(_)))));
    let ready = |m: &mut OnlineMenu| match m.request.take() {
        Some(MenuRequest::PlayMulti(r)) => r,
        other => panic!("{other:?}"),
    };
    let (rh, rg) = (ready(&mut host), ready(&mut guest));
    assert_eq!((rh.seat, rg.seat), (0, 2));
    assert_eq!(rh.seats.len(), 4);
    assert_eq!((rh.seats[0].name.as_str(), rh.seats[2].name.as_str()), ("Hosta", "Guestb"));
    assert!(rh.seats[1].computer && rh.seats[3].computer);
    assert_eq!(rh.settings.start_lp, 6000);
    assert_eq!(host.screen, Screen::Starting);
    assert_eq!(host.view().heading, "MODES");
    assert_eq!(host.multi_names().unwrap()[2], "Guestb");
    stop.store(true, Ordering::Relaxed);
    drop((host, guest));
    let _ = server.join();
    let _ = std::fs::remove_dir_all(&root);
}
