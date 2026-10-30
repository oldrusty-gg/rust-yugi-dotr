//! Headless authoritative duel server for one two-seat match on loopback.
//! (The friends-online server is `roses-server hub ...`, with `accounts`
//! and `tls` for its invites and certificates: see roses_server::friends_cli.)
//!
//!   roses-server --duel-import <dir> [--addr 127.0.0.1:7979] [--seed N]
//!                [--grace-ms 60000] [--turn-limit-ms 120000|0] [--timeouts 3]
//!                [--replay-out <file.json>] [--import-decks] [--local-match]
//!                [--leader-ability use|not-use|<seat0>,<seat1>]
//!                [--card-display hide|open|<seat0>,<seat1>]
//!   roses-server --duel-import <dir> --verify-replay <file.json>
//!
//! `<dir>` is the user's own duel import (`roses import-duel`) or a game
//! import containing `duel/`. Each seat submits its own deck when it joins
//! (checked against the original's deck rules; illegal decks are refused
//! before the match); the import's first duel gives the board and who moves
//! first. `--import-decks` plays the first duel's two decks instead.
//! `--local-match` also runs two computer-driven clients in this process
//! over TCP (seat N submits the first duel's side-N deck) and reports the
//! result. `--synthetic` replaces the import with invented test cards
//! (smoke tests only). The 2-player Custom Duel switches are the server's
//! to decide and are told to each seat when it joins: Leader Ability
//! (default USE: each leader's abilities at its rank, from the import's
//! ability table) and Card Display (default HIDE: nothing hidden is sent;
//! OPEN shows that seat's hand and face-down cards to the other, as the
//! original's shared screen does).

use roses_core::duel::Catalogue;
use roses_net::transport::{FramedTcp, LoopbackListener};
use roses_net::{
    data_fingerprint, synthetic, ClientMessage, Compat, DeckSubmission, Join, MatchOptions, OnlinePolicy, RulesTables,
    ServerMessage,
};
use roses_server::play::{play_bot, PlayOptions};
use roses_server::{replay, MatchSetup, OnlineMatch, Replay};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let all: Vec<String> = std::env::args().skip(1).collect();
    if let Some(result) = all.first().and_then(|first| roses_server::friends_cli::main(first, all[1..].to_vec())) {
        return result;
    }
    let mut args = std::env::args().skip(1);
    let mut import: Option<PathBuf> = None;
    let mut synthetic_data = false;
    let mut addr = "127.0.0.1:7979".to_string();
    let mut seed: Option<u64> = None;
    let mut policy = OnlinePolicy::default();
    let mut replay_out: Option<PathBuf> = None;
    let mut verify: Option<PathBuf> = None;
    let mut local_match = false;
    let mut import_decks = false;
    let mut options = MatchOptions::default();
    // "use" / "not-use", "open" / "hide", or one value per seat "a,b".
    let switch = |v: &str, on: &str, off: &str| -> Result<[bool; 2], String> {
        let one = |x: &str| match x {
            x if x == on => Ok(true),
            x if x == off => Ok(false),
            x => Err(format!("{x}: expected {on} or {off}")),
        };
        match v.split_once(',') {
            Some((a, b)) => Ok([one(a)?, one(b)?]),
            None => Ok([one(v)?; 2]),
        }
    };
    while let Some(a) = args.next() {
        let mut value = || args.next().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--duel-import" => import = Some(value()?.into()),
            "--synthetic" => synthetic_data = true,
            "--addr" => addr = value()?,
            "--seed" => seed = Some(value()?.parse().map_err(|_| "--seed must be a number")?),
            "--grace-ms" => policy.reconnect_grace = value()?.parse().map_err(|_| "bad --grace-ms")?,
            "--turn-limit-ms" => {
                let ms: u64 = value()?.parse().map_err(|_| "bad --turn-limit-ms")?;
                policy.turn_limit = (ms > 0).then_some(ms);
            }
            "--timeouts" => policy.timeouts_before_forfeit = value()?.parse().map_err(|_| "bad --timeouts")?,
            "--replay-out" => replay_out = Some(value()?.into()),
            "--verify-replay" => verify = Some(value()?.into()),
            "--local-match" => local_match = true,
            "--import-decks" => import_decks = true,
            "--leader-ability" => options.leader_ability = switch(&value()?, "use", "not-use")?,
            "--card-display" => options.card_display_open = switch(&value()?, "open", "hide")?,
            "--help" | "-h" => {
                println!("{}", include_str!("roses-server.rs").lines().take(27).collect::<Vec<_>>().join("\n"));
                return Ok(());
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    // The local match's bots submit these decks (seat N: side N).
    let bot_decks: [DeckSubmission; 2];
    let (cards, tables, setup_for): (Catalogue, RulesTables, Box<dyn Fn(u64) -> MatchSetup>) = if synthetic_data {
        bot_decks = synthetic::submissions();
        (
            synthetic::catalogue(),
            RulesTables::empty(),
            Box::new(|seed| MatchSetup {
                seed,
                terrain: synthetic::board(),
                leaders: synthetic::leaders(),
                deck_sources: synthetic::decks(),
                first: 0,
                leader_abilities: [vec![], vec![]],
                custom_rules: None,
                leader_ranks: [0; 2],
                destiny_rows: None,
            }),
        )
    } else {
        let dir = import.ok_or("--duel-import <dir> is required (your own `roses import-duel` output)")?;
        let data = roses_net::data::load_duel_import(&dir)?;
        let first = data.first_duel;
        bot_decks = [0, 1].map(|s| roses_net::deck::from_import_side(&first.sides[s]));
        (data.catalogue, data.tables, Box::new(move |seed| MatchSetup::from_import(&first, seed)))
    };
    let fingerprint = data_fingerprint(&cards, &tables);
    if let Some(path) = verify {
        let record: Replay = serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let duel = replay(&record, &cards, &tables)?;
        println!(
            "replay {} verified: {} revisions, outcome {:?}",
            record.match_id,
            record.entries.len(),
            duel.outcome
        );
        return Ok(());
    }
    let seed = seed.unwrap_or_else(roses_server::random_u64);
    let match_id = format!("match-{:016x}", roses_server::random_u64());
    let tokens = [roses_server::secret_token(), roses_server::secret_token()];
    let mut game = if import_decks {
        OnlineMatch::new(match_id.clone(), setup_for(seed), cards.clone(), policy, tokens)?
    } else {
        OnlineMatch::with_submitted_decks(match_id.clone(), setup_for(seed), cards.clone(), policy, tokens)?
    }
    .with_rules(tables.clone(), options)?;
    let mut listener = LoopbackListener::bind(addr.parse().map_err(|_| format!("bad --addr {addr}"))?)
        .map_err(|e| e.to_string())?;
    let bound = listener.local_addr().map_err(|e| e.to_string())?;
    println!("roses-server {match_id} listening on {bound}");
    println!("rules data {fingerprint}");
    let say = |b: [bool; 2], on: &str, off: &str| format!("{} / {}", if b[0] { on } else { off }, if b[1] { on } else { off });
    println!(
        "Custom Duel switches (seat 0 / seat 1): Leader Ability {}, Card Display {}",
        say(options.leader_ability, "USE", "NOT USE"),
        say(options.card_display_open, "OPEN", "HIDE")
    );
    println!(
        "decks: {}",
        if import_decks { "the import's first duel (both seats)" } else { "each seat submits its own (checked when it joins)" }
    );
    let stop = AtomicBool::new(false);
    std::thread::scope(|scope| -> Result<(), String> {
        let clients = local_match.then(|| {
            [0u8, 1].map(|seat| {
                let cards = cards.clone();
                let deck = (!import_decks).then(|| bot_decks[usize::from(seat)].clone());
                let compat = Compat::current(data_fingerprint(&cards, &tables));
                scope.spawn(move || {
                    play_bot(
                        || FramedTcp::<ClientMessage, ServerMessage>::connect(bound),
                        &compat,
                        &cards,
                        PlayOptions {
                            join: Join::Seat { seat: Some(seat) },
                            deck,
                            // Seat 1 drops once and resumes, to show reconnects.
                            drop_at_revision: (seat == 1).then_some(6),
                            away: Duration::from_millis(200),
                            ..PlayOptions::default()
                        },
                        |_| {},
                    )
                })
            })
        });
        roses_server::serve::serve(&mut listener, &mut game, &stop, |line| println!("{line}"))
            .map_err(|e| e.to_string())?;
        if let Some(clients) = clients {
            for (seat, c) in clients.into_iter().enumerate() {
                let r = c.join().map_err(|_| "client thread panicked")??;
                println!(
                    "client seat {seat}: ended {:?}, {} intentions sent, {} rejected, {} reconnects",
                    r.state.ended,
                    r.sent.len(),
                    r.rejected,
                    r.reconnects
                );
            }
        }
        Ok(())
    })?;
    match game.ended() {
        Some((outcome, cause)) => println!("result: {outcome:?} ({cause:?}) after {} revisions", game.revision()),
        None => println!("stopped before the match ended (revision {})", game.revision()),
    }
    let replayed = replay(game.replay(), &cards, &tables)?;
    println!("server replay reproduces the final state: {}", replayed == *game.duel());
    if let Some(path) = replay_out {
        std::fs::write(&path, serde_json::to_vec_pretty(game.replay()).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        println!("replay written to {} (server-only: it contains the seed and decks)", path.display());
    }
    Ok(())
}
