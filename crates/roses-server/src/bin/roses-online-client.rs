//! Test/CLI client for the online duel server.
//!
//!   roses-online-client --duel-import <dir> [--addr 127.0.0.1:7979]
//!                       [--seat 0|1] [--resume <token>] [--text]
//!                       [--deck-save <memory-card file or folder> [--deck 0|1|2]
//!                        | --deck-file <deck.json> | --import-side 0|1]
//!
//! Default: a computer-driven seat (intentions chosen from its own view).
//! `--text` plays from the terminal; type `help` for the commands. The
//! client reads its own import only to check compatibility (fingerprint)
//! and to know its cards; it never receives the opponent's hidden cards.
//! Its deck: a deck record of the user's own save (`--deck-save`, the card
//! block's current deck unless `--deck` names one), a JSON
//! `{leader, leader_rank, cards}` (`--deck-file`), or the import's first
//! duel side (`--import-side`, default 0: the starter deck). It is checked
//! locally with the server's rules before connecting.

use roses_core::duel::{CardData, Catalogue, DuelCommand, Pick};
use roses_core::{GridPos, UnitId};
use roses_net::client::ClientState;
use roses_net::transport::{Connection, FramedTcp, Received};
use roses_net::{
    check_deck, data_fingerprint, synthetic, ClientMessage, Compat, DeckSubmission, Join, PlayerView, RulesTables,
    SeenEvent, ServerMessage,
};
use roses_server::play::{play_bot, PlayOptions};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

type Client = FramedTcp<ClientMessage, ServerMessage>;

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut import: Option<PathBuf> = None;
    let mut synthetic_data = false;
    let mut addr = "127.0.0.1:7979".to_string();
    let mut join = Join::Seat { seat: None };
    let mut text = false;
    let mut deck_save: Option<PathBuf> = None;
    let mut deck_number: Option<u8> = None;
    let mut deck_file: Option<PathBuf> = None;
    let mut import_side: Option<usize> = None;
    while let Some(a) = args.next() {
        let mut value = || args.next().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--duel-import" => import = Some(value()?.into()),
            "--synthetic" => synthetic_data = true,
            "--addr" => addr = value()?,
            "--seat" => join = Join::Seat { seat: Some(value()?.parse().map_err(|_| "bad --seat")?) },
            "--resume" => join = Join::Resume { token: value()? },
            "--text" => text = true,
            "--deck-save" => deck_save = Some(value()?.into()),
            "--deck" => deck_number = Some(value()?.parse().map_err(|_| "--deck is 0, 1 or 2")?),
            "--deck-file" => deck_file = Some(value()?.into()),
            "--import-side" => {
                import_side = Some(value()?.parse().ok().filter(|s: &usize| *s < 2).ok_or("--import-side is 0 or 1")?)
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let seat_hint = match &join {
        Join::Seat { seat: Some(s) } => usize::from(*s & 1),
        _ => 0,
    };
    let (cards, tables, deck): (Catalogue, RulesTables, DeckSubmission) = if synthetic_data {
        let deck = match (&deck_file, &deck_save) {
            (Some(f), _) => roses_net::deck::from_json_file(f)?,
            _ => synthetic::submissions()[import_side.unwrap_or(seat_hint)].clone(),
        };
        (synthetic::catalogue(), RulesTables::empty(), deck)
    } else {
        let data = roses_net::data::load_duel_import(&import.ok_or("--duel-import <dir> is required")?)?;
        let deck = match (&deck_file, &deck_save) {
            (Some(f), _) => roses_net::deck::from_json_file(f)?,
            (None, Some(save)) => roses_net::deck::from_save_file(save, deck_number)?,
            (None, None) => roses_net::deck::from_import_side(&data.first_duel.sides[import_side.unwrap_or(0)]),
        };
        (data.catalogue, data.tables, deck)
    };
    check_deck(&deck, &cards).map_err(|p| format!("this deck would be refused: {p}"))?;
    println!("deck: leader {} (rank {}), {} cards", deck.leader, deck.leader_rank, deck.cards.len());
    let compat = Compat::current(data_fingerprint(&cards, &tables));
    let addr: SocketAddr = addr.parse().map_err(|_| format!("bad --addr {addr}"))?;
    if !text {
        let report = play_bot(
            || Client::connect(addr),
            &compat,
            &cards,
            PlayOptions { join, deck: Some(deck), timeout: Duration::from_secs(3600), ..PlayOptions::default() },
            |m| print_message(m),
        )?;
        println!(
            "ended {:?}: {} intentions sent, {} rejected",
            report.state.ended,
            report.sent.len(),
            report.rejected
        );
        return Ok(());
    }
    text_client(addr, compat, join, deck, &cards)
}

fn print_message(m: &ServerMessage) {
    match m {
        ServerMessage::Welcome { seat, match_id, resume_token, .. } => {
            println!("joined {match_id} as seat {seat}; resume token {resume_token}")
        }
        ServerMessage::Update { revision, actor, events, .. } => {
            let events: Vec<String> = events.iter().map(describe).collect();
            println!("r{revision} {actor:?}: {}", events.join("; "));
        }
        ServerMessage::Accepted { .. } | ServerMessage::Snapshot { .. } | ServerMessage::Presence { .. } => {}
        other => println!("{other:?}"),
    }
}

fn describe(e: &SeenEvent) -> String {
    match e {
        SeenEvent::Public { event } => format!("{event:?}"),
        other => format!("{other:?}"),
    }
}

fn show(view: &PlayerView, cards: &Catalogue) {
    let b = &view.board;
    println!(
        "round {} | turn: seat {} | LP {:?} | stars {:?} | hands {:?} | decks {:?}",
        b.round, b.active, b.lp, b.stars, b.hand_counts, b.deck_counts
    );
    for row in 0..7u8 {
        let line: Vec<String> = (0..7u8)
            .map(|column| {
                let p = GridPos { column, row };
                match b.units.iter().find(|u| u.position == p) {
                    Some(u) => format!("{}{:>3}", if u.side == view.seat { '+' } else { '-' }, u.id.0),
                    None => "   .".into(),
                }
            })
            .collect();
        println!("  row {row}: {}", line.join(" "));
    }
    for u in b.units.iter().filter(|u| u.side == view.seat) {
        let c = u.card.and_then(|c| cards.card(c));
        println!(
            "  unit {} at ({},{}) card {:?} atk/def {:?} {}{}",
            u.id.0,
            u.position.column,
            u.position.row,
            u.card,
            c.map(|c| (c.attack, c.defense)),
            if u.acted { "acted " } else { "" },
            if u.face_up { "face-up" } else { "face-down" }
        );
    }
    let hand: Vec<String> = b
        .hand
        .iter()
        .enumerate()
        .map(|(i, &id)| match cards.card(id) {
            Some(c) => format!("[{i}] #{id} L{} {}/{}", c.level, c.attack, c.defense),
            None => format!("[{i}] #{id}"),
        })
        .collect();
    println!("  hand: {}", hand.join("  "));
    if let Some(p) = &view.pending {
        println!("  open selection: {p:?}");
    }
}

const HELP: &str = "commands: open C R | choose I [I ...] | cancel | move UNIT C,R [C,R ...] [up] [def|atk] | \
pick cancel | pick unit N | pick grave SIDE INDEX | end | surrender | forfeit | view | sync | quit";

fn parse(line: &str) -> Result<Option<DuelCommand>, String> {
    let w: Vec<&str> = line.split_whitespace().collect();
    let num = |s: &str| s.parse::<u16>().map_err(|_| format!("not a number: {s}"));
    let cell = |s: &str| -> Result<GridPos, String> {
        let (c, r) = s.split_once(',').ok_or("cells are C,R")?;
        Ok(GridPos { column: num(c)? as u8, row: num(r)? as u8 })
    };
    Ok(Some(match w.as_slice() {
        ["open", c, r] => DuelCommand::OpenSummon { cell: GridPos { column: num(c)? as u8, row: num(r)? as u8 } },
        ["choose", rest @ ..] if !rest.is_empty() => DuelCommand::ChooseSummon {
            hand: rest.iter().map(|s| num(s).map(usize::from)).collect::<Result<_, _>>()?,
        },
        ["cancel"] => DuelCommand::CancelSummon,
        ["move", unit, rest @ ..] => {
            let mut path = Vec::new();
            let (mut face_up, mut defence) = (false, None);
            for s in rest {
                match *s {
                    "up" => face_up = true,
                    "def" => defence = Some(true),
                    "atk" => defence = Some(false),
                    s => path.push(cell(s)?),
                }
            }
            DuelCommand::Move { unit: UnitId(num(unit)?), path, face_up, defence }
        }
        ["pick", "cancel"] => DuelCommand::Pick { pick: Pick::Cancel },
        ["pick", "unit", n] => DuelCommand::Pick { pick: Pick::Unit { unit: UnitId(num(n)?) } },
        ["pick", "grave", s, i] => DuelCommand::Pick {
            pick: Pick::Graveyard { side: num(s)? as u8, index: usize::from(num(i)?) },
        },
        ["end"] => DuelCommand::EndTurn,
        ["surrender"] => DuelCommand::Surrender,
        _ => return Ok(None),
    }))
}

fn text_client(addr: SocketAddr, compat: Compat, join: Join, deck: DeckSubmission, cards: &Catalogue) -> Result<(), String> {
    let mut conn = Client::connect(addr).map_err(|e| e.to_string())?;
    conn.send(&ClientMessage::Hello { compat, join, deck: Some(deck), presence: false, spectate: false }).map_err(|e| e.to_string())?;
    let mut state = ClientState::new();
    let (tx, lines) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        let mut line = String::new();
        while stdin.read_line(&mut line).map(|n| n > 0).unwrap_or(false) {
            if tx.send(std::mem::take(&mut line)).is_err() {
                break;
            }
        }
    });
    println!("{HELP}");
    loop {
        match conn.poll() {
            Received::Message(m) => {
                print_message(&m);
                state.apply(&m);
                if matches!(m, ServerMessage::Update { .. } | ServerMessage::Snapshot { .. }) {
                    if let Some(v) = &state.view {
                        show(v, cards);
                    }
                }
                if state.ended.is_some() {
                    println!("match over: {:?}", state.ended);
                    return Ok(());
                }
                continue;
            }
            Received::Closed => return Err("the server closed the connection".into()),
            Received::Malformed(e) => return Err(e),
            Received::Empty => {}
        }
        match lines.try_recv() {
            Ok(line) => {
                let line = line.trim();
                let msg = match line {
                    "quit" => return Ok(()),
                    "help" => {
                        println!("{HELP}");
                        continue;
                    }
                    "view" => {
                        if let Some(v) = &state.view {
                            show(v, cards);
                        }
                        continue;
                    }
                    "sync" => ClientMessage::Sync,
                    "forfeit" => state.forfeit(),
                    _ => match parse(line) {
                        Ok(Some(c)) => state.command(c),
                        Ok(None) => {
                            println!("? {HELP}");
                            continue;
                        }
                        Err(e) => {
                            println!("{e}");
                            continue;
                        }
                    },
                };
                conn.send(&msg).map_err(|e| e.to_string())?;
            }
            Err(mpsc::TryRecvError::Empty) => std::thread::sleep(Duration::from_millis(5)),
            Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
        }
    }
}
