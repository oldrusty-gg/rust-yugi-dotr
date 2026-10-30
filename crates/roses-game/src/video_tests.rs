//! Video aid (ignored tests; they need the user's own duel import): data
//! for choosing the video's decks and map, and the seed search. Both run
//! the real server match (`roses_server::OnlineMatch`, the one the hub
//! hosts) with two headless Bevy clients connected in memory, each seat
//! played through its pad by `video_bot::VideoBot` at a human pace (the
//! waits are update counts; nothing sleeps, so a 20-minute duel takes
//! seconds). A seed is the match seed (the server's deck shuffle and LCG)
//! and the bots' seeds (seat 0: the seed, seat 1: the seed ^ 0xFFFF).
//!
//! `ROSES_VIDEO_DUEL=<duel import dir> ROSES_VIDEO_OUT=<dir>`
//! - `video_dump`: `data.json` (the computer opponents' decks with card
//!   names, the game's maps, the fusions of chosen cards).
//! - `video_seed_search`: `ROSES_VIDEO_PLAN=<plan.json>` (decks, maps,
//!   seeds, modules), writes `runs.jsonl` (one line per duel) and
//!   `top.json`.
//! - `video_multi_seed_search` (MODES, 2v2 and FFA): the same with the
//!   hub's N-player match (`roses_server::multi::MultiMatch`) and four
//!   clients (`MultiPlan`); it detects Exodia itself (`exodia_ready`, the
//!   engine's `exodia_complete` condition) and scores late Exodia wins.

use crate::duel_online::{ClientConn, Connector, OnlineLink};
use crate::duel_play::DuelPlay;
use crate::video_bot::{BotConfig, Pace, VideoBot};
use roses_core::duel::{CardData, DuelEvent};
use roses_net::transport::memory_pair;
use roses_net::{ClientMessage, DeckSubmission, Join, MatchOptions, RulesTables, ServerMessage};
use roses_server::serve::ServerConn;
use roses_server::{MatchSetup, OnlineMatch};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::time::Duration;

fn dirs() -> Option<(PathBuf, PathBuf)> {
    let duel = std::env::var_os("ROSES_VIDEO_DUEL")?;
    let out = std::env::var_os("ROSES_VIDEO_OUT")?;
    let out = PathBuf::from(out);
    std::fs::create_dir_all(&out).ok()?;
    Some((PathBuf::from(duel), out))
}

/// The map's label as Online Play's CHOOSE MAP shows it.
fn map_label(index: usize) -> String {
    crate::online_decks::map_label(&roses_net::lobby::MapChoice::Game { index: index as u8 })
}

#[test]
#[ignore = "video aid: needs ROSES_VIDEO_DUEL and ROSES_VIDEO_OUT"]
fn video_dump() {
    let Some((dir, out)) = dirs() else { return };
    let import = roses_net::data::load_duel_import(&dir).unwrap();
    let data = crate::duel_play::load(&dir).unwrap();
    let name = |c: u16| data.names.get(usize::from(c)).cloned().unwrap_or_default();
    let card = |c: u16| {
        let i = data.catalogue.card(c);
        serde_json::json!({"id": c, "name": name(c), "attack": i.map(|i| i.attack), "level": i.map(|i| i.level), "monster": i.map(|i| i.is_monster())})
    };
    let decks: Vec<_> = import
        .opponents
        .iter()
        .map(|o| {
            serde_json::json!({
                "opponent": o.opponent, "name": o.name, "leader": card(o.deck.leader), "rank": o.deck.leader_rank,
                "cards": o.deck.cards.iter().map(|&c| format!("{c} {}", name(c))).collect::<Vec<_>>(),
            })
        })
        .collect();
    let maps: Vec<_> = import
        .maps
        .iter()
        .enumerate()
        .map(|(i, t)| serde_json::json!({"index": i, "label": map_label(i), "terrain": t}))
        .collect();
    let keys: Vec<u16> = std::env::var("ROSES_VIDEO_CARDS")
        .ok()
        .map(|v| v.split(',').filter_map(|s| s.trim().parse().ok()).collect())
        .unwrap_or_default();
    let fusions: Vec<_> = data
        .catalogue
        .fusions
        .iter()
        .filter(|f| keys.contains(&f[0]) || keys.contains(&f[1]) || keys.contains(&f[2]))
        .map(|f| format!("{} + {} = {}", name(f[0]), name(f[1]), name(f[2])))
        .collect();
    let rules = roses_assets::parse_pal_rules(&data.executable).unwrap();
    let json = serde_json::json!({"opponents": decks, "maps": maps, "fusions": fusions, "map_count": rules.maps.len()});
    std::fs::write(out.join("data.json"), serde_json::to_string_pretty(&json).unwrap()).unwrap();
    eprintln!("wrote {}", out.join("data.json").display());
}

/// The deck builder's pad for the video's deck edit, found on the game's
/// own deck editor model: COPY A DECK of the opponent named
/// `ROSES_VIDEO_COPY` (e.g. Yugi), then in the trunk list Square on each
/// card of `ROSES_VIDEO_REMOVE` (out of the deck) and Cross on each of
/// `ROSES_VIDEO_ADD` (into it), walking there with Down/Up and L1/R1 (six
/// rows), then Circle. Writes `builder-words.txt` (the words, one per
/// update, for `ROSES_BUILDER_REPLAY`) and `p1-deck.json` (the deck the
/// edit leaves, in the order the server receives it).
#[test]
#[ignore = "video aid: needs ROSES_VIDEO_DUEL, ROSES_VIDEO_OUT, ROSES_VIDEO_COPY, ROSES_VIDEO_REMOVE, ROSES_VIDEO_ADD"]
fn video_builder_plan() {
    use roses_core::deck_editor::{DeckEditor, EditorEnv, PadInput, StaticCards};
    let Some((dir, out)) = dirs() else { return };
    let ids = |k: &str| -> Vec<u16> {
        std::env::var(k)
            .ok()
            .map(|v| v.split(',').filter_map(|s| s.trim().parse().ok()).collect())
            .unwrap_or_default()
    };
    let import = roses_net::data::load_duel_import(&dir).unwrap();
    let data = crate::duel_play::load(&dir).unwrap();
    let copy = std::env::var("ROSES_VIDEO_COPY").unwrap_or_else(|_| "Yugi".into());
    let source = import.opponents.iter().find(|o| o.name == copy).expect("the opponent to copy");
    let base = roses_net::custom_decks::CustomDeck::from_submission(&format!("{copy} (copy)"), &source.deck);
    let cards = StaticCards::from_executable(&data.executable, 0).unwrap();
    let mut block = roses_net::custom_decks::editor_block(&base, &cards);
    let (mut counter, mut rng) = (0u8, 1u64);
    let mut words: Vec<u16> = Vec::new();
    let mut env = EditorEnv {
        block: &mut block,
        duel_counter: &mut counter,
        cards: &cards,
        rng: &mut rng,
        graphics_busy: false,
    };
    let mut ed = DeckEditor::new(0, &mut env);
    let mut press = |ed: &mut DeckEditor, env: &mut EditorEnv<'_>, w: u16| {
        let ev = ed.step(PadInput { word: w, sticks: None }, env);
        ed.step(PadInput { word: 0, sticks: None }, env);
        words.push(w);
        words.push(0);
        ev
    };
    use roses_core::duel::ai::pad;
    let steps: Vec<(u16, u16)> = ids("ROSES_VIDEO_REMOVE")
        .into_iter()
        .map(|c| (c, pad::SQUARE))
        .chain(ids("ROSES_VIDEO_ADD").into_iter().map(|c| (c, pad::CROSS)))
        .collect();
    for (card, action) in steps {
        let mut guard = 0;
        loop {
            let snap = ed.snapshot();
            let target = snap.trunk.iter().position(|&c| c == card).expect("the card is in the trunk");
            let pos = usize::from(snap.trunk_window) + usize::from(snap.trunk_row);
            if pos == target {
                break;
            }
            let key = if target >= pos + 6 && usize::from(snap.trunk_window) + 6 + 6 <= snap.trunk.len() {
                pad::R1
            } else if target > pos {
                pad::DOWN
            } else if target + 6 <= pos && snap.trunk_window >= 6 {
                pad::L1
            } else {
                pad::UP
            };
            press(&mut ed, &mut env, key);
            guard += 1;
            assert!(guard < 400, "walking to {card}");
        }
        let ev = press(&mut ed, &mut env, action);
        eprintln!("card {card} {}: {ev:?}", data.names[usize::from(card)]);
    }
    let ev = press(&mut ed, &mut env, pad::CIRCLE);
    eprintln!("exit: {ev:?} finished {}", ed.finished());
    drop(env);
    let edited = roses_net::custom_decks::from_editor_block(&block, &base);
    eprintln!(
        "deck: {} cards, leader {} ({:?})",
        edited.cards.len(),
        data.names[usize::from(edited.leader)],
        edited.problem()
    );
    let deck = edited.submission().expect("a complete deck");
    roses_net::deck::check_deck(&deck, &import.catalogue).unwrap();
    let text: Vec<String> = words.iter().map(|w| format!("{w:x}")).collect();
    std::fs::write(out.join("builder-words.txt"), text.join(",")).unwrap();
    std::fs::write(out.join("p1-deck.json"), serde_json::to_string_pretty(&deck).unwrap()).unwrap();
    std::fs::write(
        out.join("p2-deck.json"),
        serde_json::to_string_pretty(&import.opponents.iter().find(|o| o.name == "Seto").unwrap().deck).unwrap(),
    )
    .unwrap();
    let names: Vec<String> = deck.cards.iter().map(|&c| data.names[usize::from(c)].clone()).collect();
    std::fs::write(
        out.join("p1-deck.txt"),
        format!(
            "leader {} rank {}\n{}\n",
            data.names[usize::from(deck.leader)],
            deck.leader_rank,
            names.join("\n")
        ),
    )
    .unwrap();
    eprintln!("{} builder words", words.len());
}

/// An RGBA image as a PNG (stored deflate blocks; no compression crate).
fn png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    fn crc(data: &[u8]) -> u32 {
        let mut c = 0xFFFF_FFFFu32;
        for &b in data {
            c ^= u32::from(b);
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
        }
        !c
    }
    let mut raw = Vec::new();
    for row in rgba.chunks(width as usize * 4).take(height as usize) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut z = vec![0x78, 0x01];
    for (i, block) in raw.chunks(65535).enumerate() {
        let last = (i + 1) * 65535 >= raw.len();
        z.push(u8::from(last));
        let n = block.len() as u16;
        z.extend_from_slice(&n.to_le_bytes());
        z.extend_from_slice(&(!n).to_le_bytes());
        z.extend_from_slice(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut chunk = |kind: &[u8], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut c = kind.to_vec();
        c.extend_from_slice(data);
        out.extend_from_slice(&c);
        out.extend_from_slice(&crc(&c).to_be_bytes());
    };
    let mut ihdr = width.to_be_bytes().to_vec();
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(b"IHDR", &ihdr);
    chunk(b"IDAT", &z);
    chunk(b"IEND", &[]);
    out
}

/// The game's CHOOSE MAP screen (Online Play, the room's map choice) on
/// each VS map, as the player sees it: `ROSES_VIDEO_OUT/choose-map-NN.png`.
#[test]
#[ignore = "video aid: needs ROSES_VIDEO_DUEL and ROSES_VIDEO_OUT"]
fn video_map_screens() {
    use crate::online_menu::{MapPreview, Row, View};
    let Some((dir, out)) = dirs() else { return };
    let import = roses_net::data::load_duel_import(&dir).unwrap();
    let mut screen = crate::online_screen::OnlineScreen::from_duel_import(&dir).unwrap();
    let first = crate::online_decks::VS_FIRST;
    let mut rows = vec![Row::Button("RANDOM".into())];
    rows.extend((first..import.maps.len()).map(|i| Row::Button(map_label(i))));
    for i in first..import.maps.len() {
        let view = View {
            heading: "CHOOSE MAP".into(),
            info: vec![format!("Map: {}", map_label(i)), "Whoever joins plays on it.".into()],
            rows: rows.clone(),
            cursor: i - first + 1,
            note: None,
            typing: false,
            deck: None,
            map: Some(MapPreview {
                cells: Some(import.maps[i].clone()),
            }),
        };
        let missing = screen.draw(&view, false);
        let path = out.join(format!("choose-map-{:02}.png", i - first + 1));
        std::fs::write(
            &path,
            png(
                crate::online_screen::FRAME as u32,
                crate::online_screen::FRAME as u32,
                &screen.rgba(),
            ),
        )
        .unwrap();
        eprintln!("{} ({missing:?})", path.display());
    }
}

/// The search's plan (`ROSES_VIDEO_PLAN`).
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Plan {
    decks: [DeckSubmission; 2],
    /// Game map indices (`MapChoice::Game`).
    maps: Vec<usize>,
    seeds: [u64; 2],
    /// Control bytes of the two seats' modules (none: the pad bot).
    modules: [Option<u8>; 2],
    #[serde(default = "default_tier")]
    tier: i16,
    #[serde(default)]
    names: Option<[String; 2]>,
}

fn default_tier() -> i16 {
    8
}

/// What one client saw.
struct ClientRun {
    seat: u8,
    #[allow(dead_code)]
    updates: u64,
    /// Updates of this seat's own turns that a window would show (an
    /// animation, a wait or a press; not the in-memory server round trips).
    own_turn: u64,
    halted: Option<String>,
    stats: crate::video_bot::BotStats,
    waited: u64,
}

fn client(
    dir: PathBuf,
    to_server: mpsc::Sender<ServerConn>,
    seat: u8,
    deck: DeckSubmission,
    tables: RulesTables,
    config: BotConfig,
    max: u64,
) -> ClientRun {
    let data = crate::duel_play::load(&dir).unwrap();
    let connector: Connector = Box::new(move || {
        let (client, server) = memory_pair::<ClientMessage, ServerMessage>();
        to_server.send(Box::new(server)).map_err(|_| std::io::Error::other("server gone"))?;
        Ok(Box::new(client) as ClientConn)
    });
    let cards = data.catalogue.clone();
    let link = OnlineLink::connect(
        connector,
        &cards,
        &tables,
        Join::Seat { seat: Some(seat) },
        Some(deck),
        Duration::from_secs(60),
        |_| {},
    )
    .expect("joined");
    let mut play = DuelPlay::online(data, link).expect("online play");
    let mut bot = VideoBot::new(config);
    let mut camera = crate::duel_board::BoardCamera::snapped((3, 3));
    let mut own_turn = 0;
    for _ in 0..max {
        let link = play.online.as_ref().expect("online");
        if link.ended() && link.state.revision == link.shown_revision && !play.animating() {
            break;
        }
        let can = !play.animating() && link.may_act();
        let view = link.state.view.clone().expect("a view");
        let revision = link.state.revision;
        let waiting = !play.animating() && !link.may_act();
        if play.session.duel.active == seat && !waiting {
            own_turn += 1;
        }
        let key = bot.key(
            &play.session.duel,
            &play.session.input,
            &view,
            revision,
            can,
            &play.data.catalogue,
            &play.data.tables,
        );
        play.press(key);
        let cursor = play.cursor();
        crate::duel_play::drive_camera(&mut camera, &mut play, cursor, &|_| None);
        camera.tick();
        if play.halted.is_some() {
            break;
        }
        if waiting {
            std::thread::sleep(Duration::from_micros(200));
        }
    }
    ClientRun {
        seat,
        updates: play.frames,
        own_turn,
        halted: play.halted.clone(),
        stats: bot.stats.clone(),
        waited: bot.pacer.waited,
    }
}

#[derive(Clone, Debug, Default, Serialize)]
struct Score {
    seed: u64,
    map: usize,
    map_label: String,
    winner: Option<u8>,
    outcome: String,
    rounds: u8,
    revisions: u64,
    minutes: f64,
    fusions: [u32; 2],
    big: [u16; 2],
    blue_eyes: [bool; 2],
    activations: [u32; 2],
    lead_changes: u32,
    final_lp: [u16; 2],
    plan_errors: [u64; 2],
    fallback_presses: [u64; 2],
    halted: Vec<String>,
    moments: Vec<String>,
    score: f64,
}

fn score_match(
    game: &OnlineMatch,
    runs: &[ClientRun; 2],
    cards: &roses_core::duel::Catalogue,
    names: &[String],
    seed: u64,
    map: usize,
) -> Score {
    let name = |c: u16| names.get(usize::from(c)).cloned().unwrap_or_else(|| format!("card {c}"));
    let record = game.replay();
    let mut duel = record.setup.build(cards).unwrap();
    let mut s = Score {
        seed,
        map,
        map_label: map_label(map),
        ..Score::default()
    };
    let mut leader: Option<bool> = None;
    for e in &record.entries {
        let (side, result) = match &e.action {
            roses_server::replay::ReplayAction::Command { side, command } => (*side, duel.apply(*side, command, cards)),
            roses_server::replay::ReplayAction::Forfeit { side } => (*side, duel.forfeit(*side)),
        };
        let Ok((next, events)) = result else { break };
        duel = next;
        let si = usize::from(side & 1);
        for ev in &events {
            match ev {
                DuelEvent::Fused { cards: pair, result } => {
                    s.fusions[si] += 1;
                    s.moments.push(format!(
                        "r{} side {side} fused {} + {} = {}",
                        duel.round,
                        name(pair[0]),
                        name(pair[1]),
                        name(*result)
                    ));
                }
                DuelEvent::Summoned { card, unit, .. } | DuelEvent::Revealed { card, unit } => {
                    if let Some(c) = cards.card(*card).filter(|c| c.is_monster()) {
                        let owner = duel.unit(*unit).map_or(side, |u| u.side);
                        let oi = usize::from(owner & 1);
                        s.big[oi] = s.big[oi].max(c.attack);
                        if *card == 0 {
                            s.blue_eyes[oi] = true;
                        }
                    }
                }
                DuelEvent::Activated { card, .. } => {
                    s.activations[si] += 1;
                    s.moments.push(format!("r{} side {side} activated {}", duel.round, name(*card)));
                }
                _ => {}
            }
        }
        let lp = [duel.sides[0].lp, duel.sides[1].lp];
        if lp[0] != lp[1] {
            let ahead = lp[0] > lp[1];
            if leader.is_some_and(|l| l != ahead) {
                s.lead_changes += 1;
            }
            leader = Some(ahead);
        }
    }
    s.final_lp = [duel.sides[0].lp, duel.sides[1].lp];
    s.rounds = duel.round;
    s.revisions = game.revision();
    if let Some((o, cause)) = game.ended() {
        s.outcome = format!("{o:?} ({cause:?})");
        s.winner = o.winner;
    }
    // At a person's pace the window shows both seats' own turns in turn
    // (the other seat's presses are replayed with their gaps), plus the
    // result banner.
    s.minutes = (runs[0].own_turn + runs[1].own_turn + 250) as f64 / 25.0 / 60.0;
    s.plan_errors = [runs[0].stats.plan_errors, runs[1].stats.plan_errors];
    s.fallback_presses = [runs[0].stats.fallback_presses, runs[1].stats.fallback_presses];
    s.halted = runs
        .iter()
        .filter_map(|r| r.halted.clone().map(|h| format!("seat {}: {h}", r.seat)))
        .collect();
    // The score (module docs of the plan): length, fusions, big monsters,
    // spell/trap moments, lead changes, a close finish won by seat 0.
    let mut score = 0.0;
    score += if (12.0..=25.0).contains(&s.minutes) {
        4.0
    } else {
        -((s.minutes - 18.0).abs() - 7.0).max(0.0) * 0.6
    };
    score += f64::from(s.fusions[0].min(3)) * 1.2
        + f64::from(s.fusions[1].min(2)) * 0.8
        + if s.fusions[0] + s.fusions[1] > 0 { 2.0 } else { 0.0 };
    score += s
        .big
        .iter()
        .map(|&b| {
            if b >= 2500 {
                1.5
            } else if b >= 2000 {
                0.7
            } else {
                0.0
            }
        })
        .sum::<f64>();
    score += if s.blue_eyes[0] { 1.0 } else { 0.0 };
    let acts = s.activations[0] + s.activations[1];
    score += if acts > 0 { 2.0 + f64::from(acts.min(4)) * 0.3 } else { 0.0 };
    score += f64::from(s.lead_changes.min(5)) * 0.8;
    if let Some(w) = s.winner {
        let lp = s.final_lp[usize::from(w & 1)];
        score += if lp <= 1000 {
            3.0
        } else if lp <= 2000 {
            2.0
        } else if lp <= 4000 {
            1.0
        } else {
            0.0
        };
        score += if w == 0 { 3.0 } else { 0.0 };
    } else {
        score -= 5.0;
    }
    if s.outcome.contains("Forfeit") || s.outcome.contains("Timeout") || !s.halted.is_empty() {
        score -= 10.0;
    }
    score -= (s.plan_errors[0] + s.plan_errors[1]) as f64 * 0.5;
    s.score = (score * 100.0).round() / 100.0;
    s
}

fn run_one(dir: &std::path::Path, plan: &Plan, import: &roses_net::data::DuelImport, seed: u64, map: usize) -> Score {
    let mut setup = MatchSetup::from_import(&import.first_duel, seed);
    setup.terrain = import.maps[map].clone();
    let cards = import.catalogue.clone();
    let tables = import.tables.clone();
    let mut game = OnlineMatch::with_submitted_decks(
        format!("video-{seed}"),
        setup,
        cards.clone(),
        roses_net::OnlinePolicy {
            turn_limit: None,
            ..roses_net::OnlinePolicy::default()
        },
        [roses_server::secret_token(), roses_server::secret_token()],
    )
    .unwrap()
    .with_rules(tables.clone(), MatchOptions::default())
    .unwrap();
    let (tx, mut rx) = mpsc::channel::<ServerConn>();
    let stop = AtomicBool::new(false);
    let runs = std::thread::scope(|scope| {
        let server = scope.spawn(|| {
            roses_server::serve::serve_with_limits(&mut rx, &mut game, &stop, roses_server::serve::Limits::UNLIMITED, |_| {}).unwrap();
        });
        let clients = [0u8, 1].map(|seat| {
            let tx = tx.clone();
            let deck = plan.decks[usize::from(seat)].clone();
            let tables = tables.clone();
            let dir = dir.to_path_buf();
            let config = BotConfig {
                pace: Pace::Human,
                seed: if seat == 0 { seed } else { seed ^ 0xFFFF },
                controller: plan.modules[usize::from(seat)],
                tier: plan.tier,
                log: None,
                exodia: false,
            };
            scope.spawn(move || client(dir, tx, seat, deck, tables, config, 2_000_000))
        });
        let runs = clients.map(|c| c.join().unwrap());
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        server.join().unwrap();
        runs
    });
    // `ROSES_VIDEO_REPLAYS=<dir>`: the server's replay of each duel (to
    // compare with a rehearsal's hub replay).
    if let Some(d) = std::env::var_os("ROSES_VIDEO_REPLAYS") {
        let d = PathBuf::from(d);
        let _ = std::fs::create_dir_all(&d);
        let _ = std::fs::write(
            d.join(format!("seed-{seed}-map-{map}.json")),
            serde_json::to_vec_pretty(game.replay()).unwrap(),
        );
    }
    let names = crate::duel_play::load(dir).unwrap().names;
    let s = score_match(&game, &runs, &cards, &names, seed, map);
    eprintln!(
        "seed {seed} {}: {} r{} {:.1} min fusions {:?} big {:?} acts {:?} leads {} lp {:?} waited {:?} score {}",
        s.map_label,
        s.outcome,
        s.rounds,
        s.minutes,
        s.fusions,
        s.big,
        s.activations,
        s.lead_changes,
        s.final_lp,
        [runs[0].waited, runs[1].waited],
        s.score
    );
    s
}

#[test]
#[ignore = "video aid: needs ROSES_VIDEO_DUEL, ROSES_VIDEO_OUT and ROSES_VIDEO_PLAN"]
fn video_seed_search() {
    let Some((dir, out)) = dirs() else { return };
    let plan: Plan =
        serde_json::from_slice(&std::fs::read(std::env::var_os("ROSES_VIDEO_PLAN").expect("ROSES_VIDEO_PLAN")).unwrap()).unwrap();
    let import = roses_net::data::load_duel_import(&dir).unwrap();
    for (i, d) in plan.decks.iter().enumerate() {
        roses_net::deck::check_deck(d, &import.catalogue).unwrap_or_else(|e| panic!("deck {i}: {e}"));
    }
    let mut all: Vec<Score> = Vec::new();
    let mut lines = String::new();
    let threads: usize = std::env::var("ROSES_VIDEO_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or(3);
    let jobs: Vec<(u64, usize)> = (plan.seeds[0]..plan.seeds[1])
        .flat_map(|s| plan.maps.iter().map(move |&m| (s, m)))
        .collect();
    let results = std::sync::Mutex::new(Vec::new());
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(&(seed, map)) = jobs.get(i) else { break };
                    let s = run_one(&dir, &plan, &import, seed, map);
                    results.lock().unwrap().push(s);
                }
            });
        }
    });
    for s in results.into_inner().unwrap() {
        lines.push_str(&serde_json::to_string(&s).unwrap());
        lines.push('\n');
        all.push(s);
    }
    std::fs::write(out.join("runs.jsonl"), lines).unwrap();
    all.sort_by(|a, b| b.score.total_cmp(&a.score));
    std::fs::write(
        out.join("top.json"),
        serde_json::to_string_pretty(&all.iter().take(15).collect::<Vec<_>>()).unwrap(),
    )
    .unwrap();
    for s in all.iter().take(5) {
        eprintln!(
            "TOP seed {} {} score {} {:.1} min winner {:?} lp {:?}",
            s.seed, s.map_label, s.score, s.minutes, s.winner, s.final_lp
        );
    }
}

// ---------------------------------------------------------------- MODES

/// The N-player search's plan (`ROSES_VIDEO_PLAN` of
/// `video_multi_seed_search`): a MODES room's settings (mode, LP, cards
/// leave; the turn clock is off, as the rehearsal's room sets it), each
/// seat's deck (a computer opponent's, by name as CHOOSE DECK lists it),
/// name and module, the maps and seeds. A seed is the hub's
/// `--match-seed`; the bots' seeds are the seed ^ (seat * 0x1111) (seat 0:
/// the seed itself).
#[derive(Clone, Debug, Serialize, Deserialize)]
struct MultiPlan {
    settings: roses_net::lobby::RoomSettings,
    /// Opponent names of the duel import (`video_dump`'s `opponents`).
    decks: Vec<String>,
    names: Vec<String>,
    modules: Vec<Option<u8>>,
    maps: Vec<usize>,
    seeds: [u64; 2],
    #[serde(default = "default_tier")]
    tier: i16,
    /// The seat whose Exodia the search looks for (FFA).
    #[serde(default)]
    exodia_seat: Option<u8>,
    /// The Exodia seat's bot uses the Exodia aid (`video_bot::exodia_assist`,
    /// the rehearsal's `ROSES_BOT_EXODIA=1`).
    #[serde(default)]
    exodia_assist: bool,
}

/// The bot seed of `seat` for match seed `seed` (the rehearsal script
/// uses the same).
fn multi_bot_seed(seed: u64, seat: u8) -> u64 {
    seed ^ (u64::from(seat) * 0x1111)
}

/// duel.rs `exodia_complete` (0x269D80), from outside the engine: the
/// side's leader is Exodia the Forbidden One and its four limbs are in the
/// eight cells round it, face-up, ready, unbound. Checked when the side's
/// turn starts (the engine checks there, after the ready flags are reset).
fn exodia_ready(duel: &roses_core::duel::Duel, s: u8) -> bool {
    use roses_core::duel::{EXODIA_HEAD, EXODIA_LIMBS};
    let Some(leader) = duel.units.iter().find(|u| u.side == s && u.slot == 0) else { return false };
    if leader.card != EXODIA_HEAD {
        return false;
    }
    EXODIA_LIMBS.iter().all(|&limb| {
        duel.units.iter().any(|u| {
            u.card == limb
                && u.side == s
                && u.committed_face_up
                && u.spellbind == 0
                && duel.chain_bound.get(&u.id.0).is_none_or(|&b| b == 0)
                && u.position.column.abs_diff(leader.position.column) <= 1
                && u.position.row.abs_diff(leader.position.row) <= 1
                && u.position != leader.position
        })
    })
}

/// The hub's N-player match of a room (hub.rs `create_multi`): every seat
/// a player with a ticket, teams by seat (2v2: S + N against E + W).
fn multi_match(plan: &MultiPlan, import: &roses_net::data::DuelImport, seed: u64, map: usize) -> roses_server::multi::MultiMatch {
    use roses_net::lobby::RoomMode;
    let mut settings = plan.settings.clone();
    settings.map = Some(roses_net::lobby::MapChoice::Game { index: map as u8 });
    let seats = (0..plan.decks.len())
        .map(|s| roses_server::multi::MultiSeatPlan {
            info: roses_net::SeatInfo {
                name: plan.names[s].clone(),
                computer: false,
                team: if settings.mode == RoomMode::Teams { (s / 2) as u8 } else { s as u8 },
            },
            deck: None,
            token: roses_server::secret_token(),
        })
        .collect();
    roses_server::multi::MultiMatch::new(
        format!("video-multi-{seed}"),
        seats,
        settings,
        import.maps[map].clone(),
        seed,
        import.catalogue.clone(),
        import.tables.clone(),
        None,
        roses_net::OnlinePolicy::default(),
    )
    .unwrap()
}

/// What the server loop saw: the Exodia moment (round, revision) when
/// the Exodia seat's turn began with the pieces assembled.
#[derive(Clone, Copy, Debug, Default)]
struct ServedMulti {
    exodia: Option<(u8, u64)>,
}

/// Serve `game` until `stop` (duel_multi_online_tests' loop), real
/// milliseconds; with `exodia_seat`, also stops everything once that
/// seat's turn begins with Exodia assembled (the N-player Exodia rule
/// ends the match there).
fn serve_multi(
    game: &mut roses_server::multi::MultiMatch,
    rx: mpsc::Receiver<ServerConn>,
    stop: &AtomicBool,
    exodia_seat: Option<u8>,
) -> ServedMulti {
    use roses_net::transport::Received;
    use std::sync::atomic::Ordering;
    let start = std::time::Instant::now();
    let mut waiting: Vec<ServerConn> = Vec::new();
    let mut seats: Vec<Option<ServerConn>> = (0..game.seat_count()).map(|_| None).collect();
    let mut served = ServedMulti::default();
    let mut turn = None;
    let deliver = |seats: &mut Vec<Option<ServerConn>>, out: roses_server::online::Outbox| {
        for (seat, m) in out {
            if let Some(Some(c)) = seats.get_mut(usize::from(seat)) {
                let _ = c.send(&m);
            }
        }
    };
    while !stop.load(Ordering::Relaxed) {
        let now = start.elapsed().as_millis() as u64;
        while let Ok(c) = rx.try_recv() {
            waiting.push(c);
        }
        let mut still = Vec::new();
        for mut c in waiting.drain(..) {
            match c.poll() {
                Received::Message(ClientMessage::Hello { compat, join, deck, presence, spectate }) => {
                    match game.hello(&compat, &join, deck.as_ref(), presence, spectate, now) {
                        Ok((seat, out)) => {
                            seats[usize::from(seat)] = Some(c);
                            deliver(&mut seats, out);
                        }
                        Err(reason) => {
                            let _ = c.send(&ServerMessage::Refused { reason });
                        }
                    }
                }
                Received::Empty => still.push(c),
                _ => {}
            }
        }
        waiting = still;
        for s in 0..seats.len() {
            loop {
                let Some(c) = seats[s].as_mut() else { break };
                match c.poll() {
                    Received::Message(m) => {
                        let out = game.handle(s as u8, m, now);
                        deliver(&mut seats, out);
                    }
                    _ => break,
                }
            }
        }
        let out = game.tick(now);
        deliver(&mut seats, out);
        if let (Some(e), Some(d)) = (exodia_seat, game.duel()) {
            let key = (d.round, d.active);
            if turn != Some(key) {
                turn = Some(key);
                if d.active == e && served.exodia.is_none() && exodia_ready(d, e) {
                    served.exodia = Some((d.round, game.revision()));
                    if game.ended().is_none() {
                        // This build has no N-player Exodia rule yet: the
                        // match would end here; the search stops.
                        stop.store(true, Ordering::Relaxed);
                    }
                }
            }
        }
        // The Exodia seat knocked out: no Exodia in this match (the search
        // stops it; `ROSES_VIDEO_FULL=1` plays such matches to their end).
        if let (Some(e), Some(d)) = (exodia_seat, game.duel()) {
            if served.exodia.is_none() && d.is_out(e) && std::env::var_os("ROSES_VIDEO_FULL").is_none() {
                stop.store(true, Ordering::Relaxed);
            }
        }
        if game.ended().is_some() && served.exodia.is_none() {
            if let (Some(e), Some(d)) = (exodia_seat, game.duel()) {
                if d.outcome.is_some_and(|o| o.reason == roses_core::duel::EndReason::Exodia && o.winner == Some(e)) {
                    served.exodia = Some((d.round, game.revision()));
                }
            }
        }
        std::thread::sleep(Duration::from_micros(300));
    }
    served
}

/// One headless client of an N-player match (`video_tests::client` with
/// `DuelPlay::online_multi` and the Target key).
#[allow(clippy::too_many_arguments)]
fn multi_client(
    dir: PathBuf,
    to_server: mpsc::Sender<ServerConn>,
    seat: u8,
    deck: DeckSubmission,
    tables: RulesTables,
    config: BotConfig,
    names: Vec<String>,
    stop: &AtomicBool,
) -> ClientRun {
    use std::sync::atomic::Ordering;
    let data = crate::duel_play::load(&dir).unwrap();
    let connector: Connector = Box::new(move || {
        let (client, server) = memory_pair::<ClientMessage, ServerMessage>();
        to_server.send(Box::new(server)).map_err(|_| std::io::Error::other("server gone"))?;
        Ok(Box::new(client) as ClientConn)
    });
    let cards = data.catalogue.clone();
    let link = OnlineLink::connect(connector, &cards, &tables, Join::Seat { seat: Some(seat) }, Some(deck), Duration::from_secs(120), |_| {})
        .expect("joined");
    let mut play = DuelPlay::online_multi(data, link, names).expect("online play");
    assert!(play.multi.is_some(), "an N-player match");
    let mut bot = VideoBot::new(config);
    let mut camera = crate::duel_board::BoardCamera::snapped((3, 3));
    let mut own_turn = 0;
    for _ in 0..8_000_000u64 {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let link = play.online.as_ref().expect("online");
        if link.ended() && link.state.revision == link.shown_revision && !play.animating() {
            break;
        }
        let can = !play.animating() && link.may_act();
        let view = link.state.view.clone().expect("a view");
        let revision = link.state.revision;
        let waiting = !play.animating() && !link.may_act();
        if play.session.duel.active == seat && !waiting {
            own_turn += 1;
        }
        let key = bot.key(&play.session.duel, &play.session.input, &view, revision, can, &play.data.catalogue, &play.data.tables);
        if bot.take_target() {
            if let Some(m) = play.multi.as_mut() {
                m.target_key = true;
            }
        }
        play.press(key);
        let cursor = play.cursor();
        crate::duel_play::drive_camera(&mut camera, &mut play, cursor, &|_| None);
        camera.tick();
        if play.halted.is_some() {
            break;
        }
        if waiting {
            std::thread::sleep(Duration::from_micros(200));
        }
    }
    ClientRun {
        seat,
        updates: play.frames,
        own_turn,
        halted: play.halted.clone().filter(|h| h != crate::duel_multi::MULTI_DONE),
        stats: bot.stats.clone(),
        waited: bot.pacer.waited,
    }
}

#[derive(Clone, Debug, Default, Serialize)]
struct MultiScore {
    seed: u64,
    map: usize,
    map_label: String,
    winner: Option<u8>,
    outcome: String,
    rounds: u8,
    revisions: u64,
    minutes: f64,
    /// The Exodia seat's turn began assembled: (round, revision).
    exodia: Option<(u8, u64)>,
    /// Knockouts in order: (round, side).
    knockouts: Vec<(u8, u8)>,
    knockouts_before_exodia: usize,
    lp_at_exodia: Vec<u16>,
    exodia_pieces_max: usize,
    /// The limbs' places as they change: (round, "hand UP+near grave -").
    exodia_places: Vec<(u8, String)>,
    fusions: Vec<u32>,
    big: Vec<u16>,
    activations: Vec<u32>,
    lead_changes: u32,
    final_lp: Vec<u16>,
    plan_errors: Vec<u64>,
    fallback_presses: Vec<u64>,
    halted: Vec<String>,
    moments: Vec<String>,
    score: f64,
}

fn score_multi(
    game: &roses_server::multi::MultiMatch,
    served: ServedMulti,
    runs: &[ClientRun],
    plan: &MultiPlan,
    cards: &roses_core::duel::Catalogue,
    names: &[String],
    seed: u64,
    map: usize,
) -> MultiScore {
    use roses_core::duel::{MultiMode, EXODIA_LIMBS};
    let name = |c: u16| names.get(usize::from(c)).cloned().unwrap_or_else(|| format!("card {c}"));
    let record = game.replay();
    let n = plan.decks.len();
    let mut s = MultiScore {
        seed,
        map,
        map_label: map_label(map),
        exodia: served.exodia,
        fusions: vec![0; n],
        big: vec![0; n],
        activations: vec![0; n],
        ..MultiScore::default()
    };
    let Some(setup) = record.setup.as_ref() else {
        s.outcome = "never started".into();
        s.score = -100.0;
        return s;
    };
    let mut duel = setup.build(cards).unwrap();
    let teams = duel.multi().map(|m| m.rules.mode) == Some(MultiMode::Teams);
    let group = |side: u8| if teams { side / 2 } else { side };
    let mut leader: Option<u8> = None;
    for e in &record.entries {
        let (side, result) = match &e.action {
            roses_server::replay::ReplayAction::Command { side, command } => (*side, duel.apply(*side, command, cards)),
            roses_server::replay::ReplayAction::Forfeit { side } => (*side, duel.forfeit_with(*side, cards)),
        };
        let Ok((next, events)) = result else { break };
        duel = next;
        let si = usize::from(side);
        for ev in &events {
            match ev {
                DuelEvent::Fused { cards: pair, result } => {
                    s.fusions[si] += 1;
                    s.moments.push(format!("r{} side {side} fused {} + {} = {}", duel.round, name(pair[0]), name(pair[1]), name(*result)));
                }
                DuelEvent::Summoned { card, unit, .. } | DuelEvent::Revealed { card, unit } => {
                    if let Some(c) = cards.card(*card).filter(|c| c.is_monster()) {
                        let owner = usize::from(duel.unit(*unit).map_or(side, |u| u.side));
                        if owner < n {
                            s.big[owner] = s.big[owner].max(c.attack);
                        }
                    }
                }
                DuelEvent::Activated { card, .. } => {
                    s.activations[si] += 1;
                    s.moments.push(format!("r{} side {side} activated {}", duel.round, name(*card)));
                }
                DuelEvent::KnockedOut { side: out, reason } => {
                    s.knockouts.push((duel.round, *out));
                    s.moments.push(format!("r{} side {out} knocked out ({reason:?})", duel.round));
                    if s.exodia.is_none_or(|(_, rev)| e.revision <= rev) {
                        s.knockouts_before_exodia += 1;
                    }
                }
                _ => {}
            }
        }
        if let Some(x) = plan.exodia_seat {
            let pieces = EXODIA_LIMBS
                .iter()
                .filter(|&&l| duel.units.iter().any(|u| u.side == x && u.card == l && u.committed_face_up))
                .count();
            s.exodia_pieces_max = s.exodia_pieces_max.max(pieces);
            // Where each limb is (for the search's notes): hand, board
            // (face up / down, next to the leader), graveyard, deck.
            let lead = duel.units.iter().find(|u| u.side == x && u.slot == 0).map(|u| u.position);
            let place: Vec<String> = EXODIA_LIMBS
                .iter()
                .map(|&l| {
                    let side = &duel.sides[usize::from(x)];
                    if let Some(u) = duel.units.iter().find(|u| u.side == x && u.card == l) {
                        let near = lead.is_some_and(|p| p.column.abs_diff(u.position.column) <= 1 && p.row.abs_diff(u.position.row) <= 1);
                        format!("{}{}", if u.committed_face_up { "UP" } else { "down" }, if near { "+near" } else { "" })
                    } else if side.hand.contains(&l) {
                        "hand".into()
                    } else if side.graveyard.contains(&l) {
                        "grave".into()
                    } else {
                        "-".into()
                    }
                })
                .collect();
            let line = place.join(" ");
            if s.exodia_places.last().is_none_or(|(_, l)| *l != line) {
                s.exodia_places.push((duel.round, line));
            }
            if s.exodia.is_some_and(|(_, rev)| rev == e.revision) {
                s.lp_at_exodia = duel.sides.iter().map(|d| d.lp).collect();
                s.moments.push(format!("r{} side {x} EXODIA assembled at revision {}", duel.round, e.revision));
            }
        }
        // The leader: the group with the most LP (ties keep the last).
        let mut by_group: Vec<(u32, u8)> = Vec::new();
        for g in 0..n as u8 {
            if (0..n as u8).any(|x| group(x) == g) {
                let lp: u32 = (0..n as u8).filter(|&x| group(x) == g && !duel.is_out(x)).map(|x| u32::from(duel.sides[usize::from(x)].lp)).sum();
                by_group.push((lp, g));
            }
        }
        by_group.sort();
        if let [.., (b, _), (a, g)] = by_group.as_slice() {
            if a > b {
                if leader.is_some_and(|l| l != *g) {
                    s.lead_changes += 1;
                }
                leader = Some(*g);
            }
        }
    }
    s.final_lp = duel.sides.iter().map(|d| d.lp).collect();
    s.rounds = duel.round;
    s.revisions = game.revision();
    if let Some((o, cause)) = game.ended() {
        s.outcome = format!("{o:?} ({cause:?})");
        s.winner = o.winner;
    } else if let (Some(x), Some(_)) = (plan.exodia_seat, s.exodia) {
        s.outcome = "Exodia (search stopped: this build has no N-player Exodia rule)".into();
        s.winner = Some(x);
    } else {
        s.outcome = "not ended".into();
    }
    s.minutes = (runs.iter().map(|r| r.own_turn).sum::<u64>() + 300) as f64 / 25.0 / 60.0;
    s.plan_errors = runs.iter().map(|r| r.stats.plan_errors).collect();
    s.fallback_presses = runs.iter().map(|r| r.stats.fallback_presses).collect();
    s.halted = runs.iter().filter_map(|r| r.halted.clone().map(|h| format!("seat {}: {h}", r.seat))).collect();
    let mut score = 0.0;
    score += s.fusions.iter().map(|&f| f64::from(f.min(2)) * 0.6).sum::<f64>();
    score += s.big.iter().map(|&b| if b >= 2500 { 0.8 } else if b >= 2000 { 0.4 } else { 0.0 }).sum::<f64>();
    let acts: u32 = s.activations.iter().sum();
    score += f64::from(acts.min(8)) * 0.3;
    score += f64::from(s.lead_changes.min(6)) * 0.6;
    match plan.exodia_seat {
        Some(x) => {
            // FFA won by Exodia, late: after knockouts, a long fight.
            if s.exodia.is_some() {
                score += 12.0;
                score += s.knockouts_before_exodia.min(2) as f64 * 3.0;
                score += if (18.0..=35.0).contains(&s.minutes) { 4.0 } else { -((s.minutes - 26.0).abs() - 9.0).max(0.0) * 0.5 };
                let own = s.lp_at_exodia.get(usize::from(x)).copied().unwrap_or(0);
                score += if own <= 1000 { 3.0 } else if own <= 2500 { 2.0 } else { 0.0 };
                score += if s.exodia.is_some_and(|(r, _)| r >= 8) { 2.0 } else { 0.0 };
            } else {
                score -= 10.0 - s.exodia_pieces_max as f64;
            }
        }
        None => {
            // 2v2: close and eventful, won by player 1's team.
            score += if (15.0..=30.0).contains(&s.minutes) { 4.0 } else { -((s.minutes - 22.0).abs() - 8.0).max(0.0) * 0.6 };
            score += s.knockouts.len().min(3) as f64 * 1.5;
            if let Some(w) = s.winner {
                score += if group(w) == 0 { 3.0 } else { 0.0 };
                let lp: u32 = (0..n as u8).filter(|&x| group(x) == group(w)).map(|x| u32::from(s.final_lp[usize::from(x)])).sum();
                score += if lp <= 1500 { 3.0 } else if lp <= 3000 { 2.0 } else if lp <= 5000 { 1.0 } else { 0.0 };
            } else {
                score -= 5.0;
            }
        }
    }
    if s.outcome.contains("Forfeit") || s.outcome.contains("Timeout") || s.outcome.contains("Abandon") || !s.halted.is_empty() {
        score -= 10.0;
    }
    score -= s.plan_errors.iter().sum::<u64>() as f64 * 0.3;
    s.score = (score * 100.0).round() / 100.0;
    s
}

fn run_multi(dir: &std::path::Path, plan: &MultiPlan, import: &roses_net::data::DuelImport, seed: u64, map: usize) -> MultiScore {
    let mut game = multi_match(plan, import, seed, map);
    let tables = import.tables.clone();
    let (tx, rx) = mpsc::channel::<ServerConn>();
    let stop = AtomicBool::new(false);
    let (served, runs) = std::thread::scope(|scope| {
        let stop = &stop;
        let game = &mut game;
        let server = scope.spawn(move || serve_multi(game, rx, stop, plan.exodia_seat));
        let clients: Vec<_> = (0..plan.decks.len() as u8)
            .map(|seat| {
                let tx = tx.clone();
                let deck = import.opponents.iter().find(|o| o.name == plan.decks[usize::from(seat)]).expect("the deck's opponent").deck.clone();
                let tables = tables.clone();
                let dir = dir.to_path_buf();
                let config = BotConfig {
                    pace: Pace::Human,
                    seed: multi_bot_seed(seed, seat),
                    controller: plan.modules[usize::from(seat)],
                    tier: plan.tier,
                    log: None,
                    exodia: plan.exodia_assist && plan.exodia_seat == Some(seat),
                };
                // `ROSES_VIDEO_BOT_LOGS=<dir>`: each seat's bot log.
                let config = BotConfig {
                    log: std::env::var_os("ROSES_VIDEO_BOT_LOGS").map(|d| PathBuf::from(d).join(format!("seed-{seed}-map-{map}-seat-{seat}.jsonl"))),
                    ..config
                };
                let names = plan.names.clone();
                scope.spawn(move || multi_client(dir, tx, seat, deck, tables, config, names, stop))
            })
            .collect();
        let runs: Vec<ClientRun> = clients.into_iter().map(|c| c.join().unwrap()).collect();
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        (server.join().unwrap(), runs)
    });
    if let Some(d) = std::env::var_os("ROSES_VIDEO_REPLAYS") {
        let d = PathBuf::from(d);
        let _ = std::fs::create_dir_all(&d);
        let _ = std::fs::write(d.join(format!("seed-{seed}-map-{map}.json")), serde_json::to_vec_pretty(game.replay()).unwrap());
    }
    let names = crate::duel_play::load(dir).unwrap().names;
    let s = score_multi(&game, served, &runs, plan, &import.catalogue, &names, seed, map);
    eprintln!(
        "seed {seed} {}: {} r{} {:.1} min exodia {:?} knockouts {:?} lp {:?} fusions {:?} acts {:?} leads {} errors {:?} score {}",
        s.map_label, s.outcome, s.rounds, s.minutes, s.exodia, s.knockouts, s.final_lp, s.fusions, s.activations, s.lead_changes, s.plan_errors, s.score
    );
    s
}

#[test]
#[ignore = "video aid: needs ROSES_VIDEO_DUEL, ROSES_VIDEO_OUT and ROSES_VIDEO_PLAN (an N-player plan)"]
fn video_multi_seed_search() {
    let Some((dir, out)) = dirs() else { return };
    let plan: MultiPlan =
        serde_json::from_slice(&std::fs::read(std::env::var_os("ROSES_VIDEO_PLAN").expect("ROSES_VIDEO_PLAN")).unwrap()).unwrap();
    let import = roses_net::data::load_duel_import(&dir).unwrap();
    assert!(plan.decks.len() == plan.names.len() && plan.decks.len() == plan.modules.len());
    for (i, name) in plan.decks.iter().enumerate() {
        let o = import.opponents.iter().find(|o| &o.name == name).unwrap_or_else(|| panic!("seat {i}: no opponent {name:?}"));
        roses_net::deck::check_deck(&o.deck, &import.catalogue).unwrap_or_else(|e| panic!("seat {i}: {e}"));
    }
    let threads: usize = std::env::var("ROSES_VIDEO_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or(2);
    let jobs: Vec<(u64, usize)> = (plan.seeds[0]..plan.seeds[1]).flat_map(|s| plan.maps.iter().map(move |&m| (s, m))).collect();
    let results = std::sync::Mutex::new(Vec::new());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let lines = std::sync::Mutex::new(std::fs::File::create(out.join("runs.jsonl")).unwrap());
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(&(seed, map)) = jobs.get(i) else { break };
                let s = run_multi(&dir, &plan, &import, seed, map);
                {
                    use std::io::Write;
                    let mut f = lines.lock().unwrap();
                    let _ = writeln!(f, "{}", serde_json::to_string(&s).unwrap());
                }
                results.lock().unwrap().push(s);
            });
        }
    });
    let mut all = results.into_inner().unwrap();
    all.sort_by(|a, b| b.score.total_cmp(&a.score));
    std::fs::write(out.join("top.json"), serde_json::to_string_pretty(&all.iter().take(15).collect::<Vec<_>>()).unwrap()).unwrap();
    for s in all.iter().take(5) {
        eprintln!("TOP seed {} {} score {} {:.1} min winner {:?} exodia {:?} lp {:?}", s.seed, s.map_label, s.score, s.minutes, s.winner, s.exodia, s.final_lp);
    }
}
