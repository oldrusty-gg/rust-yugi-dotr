//! TOURNAMENTS (a port addition) on the Online Play screens, against a
//! lobby connection the test plays the server's side of.

use super::*;
use roses_net::lobby::{BracketGame, LobbyRequest, TournamentPrompt};
use roses_net::transport::{Connection, Memory, Received};
use roses_net::{synthetic, RulesTables};

fn compat() -> Compat {
    Compat::current(roses_net::data_fingerprint(&synthetic::catalogue(), &RulesTables::empty()))
}

fn menu(tag: &str) -> (OnlineMenu, Memory<LobbyEvent, LobbyRequest>) {
    let root = std::env::temp_dir().join(format!("roses-tmenu-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let config = OnlineConfig { server: "127.0.0.1:1".into(), name: None, tls: false, pin: None };
    let path = root.join("online.json");
    std::fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
    let mut m = OnlineMenu::new(&path, Ok(DeckChoice { deck: synthetic::submissions()[0].clone(), source: "test".into() }), compat());
    let (client, mut server) = roses_net::transport::memory_pair::<LobbyRequest, LobbyEvent>();
    m.lobby = Some(LobbyClient::start(Box::new(client), compat(), "test").unwrap());
    server
        .send(&LobbyEvent::LobbyWelcome {
            lobby_version: 1,
            server: "S".into(),
            computer: false,
            invite_required: false,
            choices: false,
            email: false,
            spectate: false,
            modes: false,
            tournaments: true,
        })
        .unwrap();
    server.send(&LobbyEvent::SignedIn { username: "Yugi".into(), session: "s".into() }).unwrap();
    m.update();
    assert_eq!(m.screen, Screen::Lobby);
    (m, server)
}

/// The requests the menu sent since the last call.
fn sent(server: &mut Memory<LobbyEvent, LobbyRequest>) -> Vec<LobbyRequest> {
    let mut out = Vec::new();
    while let Received::Message(r) = server.poll() {
        out.push(r);
    }
    out
}

fn tournament(id: u64, quick: bool, starts: i64, entrants: u16, you: Option<EntryStatus>) -> TournamentInfo {
    TournamentInfo {
        id,
        name: if quick { "Quick Tournament".into() } else { "Weekly Tournament".into() },
        creator: None,
        weekly: !quick,
        quick,
        starts,
        cap: if quick { 4 } else { 128 },
        state: TournamentState::Open,
        entrants,
        checked_in: 0,
        you,
        champion: None,
        note: None,
    }
}

fn select(m: &mut OnlineMenu, pick: impl Fn(&Row) -> bool) {
    let rows = m.view().rows;
    m.cursor = rows.iter().position(pick).unwrap_or_else(|| panic!("no such row in {rows:?}"));
}

fn button(t: &str) -> impl Fn(&Row) -> bool + '_ {
    move |r| matches!(r, Row::Button(b) if b == t)
}

/// Every character the screen draws is in the message font.
fn all_in_font(v: &View) {
    let rows = v.rows.iter().flat_map(|r| match r {
        Row::Button(t) => vec![t.clone()],
        Row::Choice(a, b) | Row::Field(a, b) => vec![a.clone(), b.clone()],
        Row::Player(n, _) => vec![n.clone()],
        _ => vec![],
    });
    for line in v.info.iter().cloned().chain(rows) {
        assert!(line.chars().all(|c| c == ' ' || crate::online_title::font_cell(c).is_some()), "not in the font: {line:?}");
        assert!(line.chars().count() <= LINE_MAX || v.info.iter().all(|l| *l != line), "too long: {line}");
    }
}

#[test]
fn the_list_shows_the_quick_tournament_first_and_start_times_in_local_time() {
    let (mut m, mut server) = menu("list");
    select(&mut m, button("TOURNAMENTS"));
    m.press(Key::Cross);
    assert_eq!(m.screen, Screen::Tournaments);
    assert!(sent(&mut server).contains(&LobbyRequest::ListTournaments));
    let now = friends::unix_now();
    let starts = now + 3 * 86_400;
    server
        .send(&LobbyEvent::Tournaments { now, tournaments: vec![tournament(7, true, 0, 2, None), tournament(5, false, starts, 9, None)] })
        .unwrap();
    m.update();
    let v = m.view();
    assert_eq!(v.info[0], "Quick tournament: 2/4 joined.");
    assert_eq!(v.info[1], "Starts when 4 join.");
    assert_eq!(v.rows[0], Row::Choice("QUICK TOURNAMENT".into(), "2/4 JOINED".into()));
    assert_eq!(v.rows[1], Row::Choice("Weekly Tournament".into(), local_time(starts)));
    all_in_font(&v);
    // The scheduled one's lines: its start in the player's time.
    m.cursor = 1;
    let v = m.view();
    assert!(v.info.iter().any(|l| l == &cut(&format!("Starts {} (your time)", local_time(starts)))), "{:?}", v.info);
    all_in_font(&v);
    // Into it: JOIN (sign-up is open from creation), no CHECK IN yet.
    m.press(Key::Cross);
    assert_eq!(m.screen, Screen::Tournament { id: 5 });
    let rows = m.view().rows;
    assert!(rows.contains(&Row::Button("JOIN".into())) && !rows.contains(&Row::Button("CHECK IN".into())), "{rows:?}");
    select(&mut m, button("JOIN"));
    m.press(Key::Cross);
    assert!(sent(&mut server).contains(&LobbyRequest::JoinTournament { id: 5 }));
    // Signed up, inside the check-in hour: CHECK IN and LEAVE.
    let soon = now + 45 * 60;
    server.send(&LobbyEvent::Tournaments { now, tournaments: vec![tournament(5, false, soon, 10, Some(EntryStatus::SignedUp))] }).unwrap();
    m.update();
    let rows = m.view().rows;
    assert!(rows.contains(&Row::Button("CHECK IN".into())) && rows.contains(&Row::Button("LEAVE".into())), "{rows:?}");
    all_in_font(&m.view());
}

#[test]
fn the_preview_bracket_carries_its_banner() {
    let (mut m, mut server) = menu("bracket");
    let now = friends::unix_now();
    let t = tournament(5, false, now + 86_400, 3, Some(EntryStatus::SignedUp));
    server.send(&LobbyEvent::Tournaments { now, tournaments: vec![t.clone()] }).unwrap();
    m.update();
    m.go(Screen::Tournament { id: 5 });
    select(&mut m, button("BRACKET"));
    m.press(Key::Cross);
    assert!(sent(&mut server).contains(&LobbyRequest::TournamentBracket { id: 5 }));
    let p = |name: &str, seed| BracketSide::Player { name: name.into(), seed };
    let game = |a, b| BracketGame { sides: [a, b], winner: None, how: None, state: GameState::Waiting };
    let bracket = BracketView {
        tournament: t,
        preview: true,
        size: 4,
        rounds: vec![vec![game(p("Yugi", 1), BracketSide::Bye), game(p("Joey", 2), p("Mai", 3))], vec![game(BracketSide::Tbd, BracketSide::Tbd)]],
    };
    server.send(&LobbyEvent::TournamentBracket { now, bracket }).unwrap();
    m.update();
    let v = m.view();
    assert_eq!(v.heading, "BRACKET");
    let text = v.info[1..].join(" ");
    assert_eq!(text, PREVIEW_BANNER, "{:?}", v.info);
    assert!(v.rows.contains(&Row::Choice("R1 Yugi v Bye".into(), String::new())));
    assert!(v.rows.contains(&Row::Choice("R1 Joey v Mai".into(), String::new())));
    all_in_font(&v);
    m.press(Key::Circle);
    assert_eq!(m.screen, Screen::Tournament { id: 5 });
}

#[test]
fn a_ready_prompt_leads_to_ready_and_the_ordinary_duel() {
    let (mut m, mut server) = menu("ready");
    let now = friends::unix_now();
    let prompt = TournamentPrompt {
        tournament: 5,
        name: "Weekly Tournament".into(),
        round: 1,
        rounds: 4,
        opponent: "Kaiba".into(),
        deadline: now + 600,
        you_ready: false,
        opponent_ready: true,
    };
    server.send(&LobbyEvent::TournamentMatch { now, prompt: prompt.clone() }).unwrap();
    m.update();
    assert_eq!(m.screen, Screen::TournamentReady { id: 5 });
    let v = m.view();
    assert!(v.info.iter().any(|l| l.contains("Opponent: Kaiba")));
    assert!(v.info.iter().any(|l| l.contains("Your opponent is ready.")));
    all_in_font(&v);
    select(&mut m, button("READY"));
    m.press(Key::Cross);
    assert!(sent(&mut server).contains(&LobbyRequest::TournamentReady { id: 5 }));
    assert!(m.view().info.iter().any(|l| l.contains("Waiting for Kaiba")));
    // The match: an ordinary MatchReady, into the existing duel.
    server
        .send(&LobbyEvent::MatchReady {
            match_id: "m1".into(),
            seat: 1,
            ticket: "t".into(),
            opponent: "Kaiba".into(),
            options: Default::default(),
            map: None,
            opponent_title: None,
        })
        .unwrap();
    m.update();
    assert_eq!(m.screen, Screen::Starting);
    assert!(matches!(m.request, Some(MenuRequest::Play(_))));
}

#[test]
fn a_filled_quick_tournament_asks_to_check_in_and_badges_show_in_the_lobby() {
    let (mut m, mut server) = menu("quick");
    let now = friends::unix_now();
    let t = tournament(7, true, now + 300, 4, Some(EntryStatus::SignedUp));
    server.send(&LobbyEvent::TournamentCheckIn { now, tournament: t.clone(), closes: now + 300 }).unwrap();
    m.update();
    assert_eq!(m.screen, Screen::QuickCheckIn { id: 7 });
    all_in_font(&m.view());
    select(&mut m, button("CHECK IN"));
    m.press(Key::Cross);
    assert!(sent(&mut server).contains(&LobbyRequest::CheckIn { id: 7 }));
    m.go(Screen::Lobby);
    let mut kaiba = PlayerEntry::new("Kaiba", PlayerState::Idle);
    kaiba.titles = 1;
    kaiba.quick_titles = 2;
    let mut me = PlayerEntry::new("Yugi", PlayerState::Idle);
    me.quick_titles = 1;
    server.send(&LobbyEvent::Players { players: vec![kaiba, me] }).unwrap();
    m.update();
    let v = m.view();
    assert!(v.rows.contains(&Row::Player("Kaiba [1] (q2)".into(), PlayerState::Idle)), "{:?}", v.rows);
    assert!(v.info[0].starts_with("Yugi (q1) on"), "{:?}", v.info);
    all_in_font(&v);
}

#[test]
fn the_create_form_steps_its_start_and_cap_and_checks_the_name() {
    let (mut m, mut server) = menu("create");
    m.go(Screen::Tournaments);
    m.act(Action::NewTournament);
    assert_eq!(m.screen, Screen::TournamentCreate);
    let start = m.create_start;
    assert!(start >= friends::unix_now() + 2 * 3600 && start % 3600 == 0);
    select(&mut m, |r| matches!(r, Row::Choice(l, _) if l == "START"));
    m.press(Key::Right);
    assert_eq!(m.create_start, start + 3600);
    m.press(Key::Left);
    m.press(Key::Left);
    m.press(Key::Left);
    assert_eq!(m.create_start, start - 3600, "never sooner than two hours on (the server wants one)");
    select(&mut m, |r| matches!(r, Row::Choice(l, _) if l == "PLAYERS"));
    m.press(Key::Right);
    assert_eq!(m.create_cap, 32);
    select(&mut m, button("CREATE"));
    m.press(Key::Cross);
    assert_eq!(m.sfx.last(), Some(&SFX_BUZZER), "no name yet");
    m.cursor = 0;
    for c in "Duel Night".chars() {
        m.press(Key::Char(c));
    }
    select(&mut m, button("CREATE"));
    m.press(Key::Cross);
    let req = sent(&mut server);
    assert!(req.iter().any(|r| matches!(r, LobbyRequest::CreateTournament { name, cap: 32, .. } if name == "Duel Night")), "{req:?}");
    all_in_font(&m.view());
}
