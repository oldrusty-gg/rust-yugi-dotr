//! Spectating (a port addition): the view of nobody never holds a hand or a
//! face-down card's identity, even when the match's Card Display is OPEN,
//! and the new messages are additive (older clients and servers read what
//! they always read).

use crate::client::{Bot, BotAction};
use crate::filter::{player_view_with, scramble_hidden_from_all, spectator_events, spectator_view, view_for, SeenEvent, Viewer};
use crate::lobby::{LiveMatch, LobbyEvent, LobbyRequest, MapChoice};
use crate::mirror::mirror_duel;
use crate::protocol::*;
use crate::synthetic;
use roses_core::duel::{build_campaign_decks, Duel, DuelCommand, DuelEvent, DuelSetup, PAD_CONTROLLER};
use roses_core::GridPos;
use serde::Deserialize;
use serde_json::{json, to_value};

/// A card id no synthetic card has: stands for every hidden card.
const SENTINEL: u16 = 0xBEEF;

fn duel(seed: u64) -> Duel {
    let cards = synthetic::catalogue();
    let decks = synthetic::decks();
    let mut rng = seed;
    let decks = build_campaign_decks(&mut rng, [&decks[0], &decks[1]]);
    Duel::new(
        DuelSetup {
            terrain: synthetic::board(),
            leaders: synthetic::leaders(),
            decks,
            first: 0,
            leader_abilities: [vec![], vec![]],
            controllers: [PAD_CONTROLLER; 2],
        },
        &cards,
    )
    .unwrap()
    .0
}

fn bytes_have(value: &impl serde::Serialize, number: u16) -> bool {
    let text = serde_json::to_string(value).unwrap();
    let n = number.to_string();
    // As a whole JSON number (not a digit run inside a longer one).
    text.match_indices(&n).any(|(i, _)| {
        let before = text[..i].chars().last();
        let after = text[i + n.len()..].chars().next();
        !before.is_some_and(|c| c.is_ascii_digit()) && !after.is_some_and(|c| c.is_ascii_digit())
    })
}

/// Every state of whole computer matches: the spectator's view has no hand,
/// no face-down card and no Destiny Draw byte; it does not change when
/// everything hidden from nobody changes, and its serialised bytes never
/// hold the hidden cards (all made the sentinel); OPEN changes nothing; it
/// knows no card that either seat (HIDE) does not; the mirror rebuilds it.
#[test]
fn a_spectator_never_sees_a_hand_or_a_face_down_card() {
    let cards = synthetic::catalogue();
    let mut states = 0;
    let mut face_down_seen = 0;
    for seed in [7u64, 11, 23] {
        let mut d = duel(seed);
        let mut bots = [Bot::new(), Bot::new()];
        for step in 0..3000u64 {
            let sv = spectator_view(&d);
            assert!(sv.spectator && sv.board.hand.is_empty() && sv.open_hand.is_none(), "step {step}");
            assert_eq!(sv.public.own_destiny, 0);
            for (u, p) in sv.board.units.iter().zip(&sv.public.units) {
                assert_eq!(u.card.is_some(), p.committed_face_up, "unit {:?} at step {step}", u.id);
                if u.card.is_none() {
                    face_down_seen += 1;
                    assert_eq!((p.attack_bonus, p.defense_bonus, p.evo_counter, p.discount), (0, 0, 0, 0));
                    assert!(p.record.is_none());
                }
            }
            // Card Display OPEN is ignored for spectators.
            assert_eq!(view_for(&d, Viewer::Nobody, [true, true]), sv);
            // Independent of every hidden card, and never in the bytes.
            let scrambled = scramble_hidden_from_all(&d, SENTINEL);
            let scrambled_view = spectator_view(&scrambled);
            assert_eq!(scrambled_view, sv, "step {step}");
            assert!(!bytes_have(&scrambled_view, SENTINEL), "step {step}");
            let snapshot = ServerMessage::Snapshot {
                snapshot: Snapshot { revision: step, last_seq: 0, status: MatchStatus::Running, view: scrambled_view, presence: vec![true; 2] },
            };
            assert!(!bytes_have(&snapshot, SENTINEL));
            // A subset of each seat's own view under HIDE.
            for seat in 0..2u8 {
                let pv = player_view_with(&d, seat, [false; 2]);
                for (s, p) in sv.board.units.iter().zip(&pv.board.units) {
                    assert!(s.card.is_none() || s.card == p.card);
                }
                assert_eq!((&sv.board.hand_counts, &sv.graveyards), (&pv.board.hand_counts, &pv.graveyards));
            }
            // The client's copy is exactly what the view allows.
            assert_eq!(spectator_view(&mirror_duel(&sv, &cards).unwrap()), sv);
            states += 1;
            if d.outcome.is_some() {
                break;
            }
            let seat = d.active;
            let view = player_view_with(&d, seat, [false; 2]);
            let next = match bots[usize::from(seat)].next(&view, step, &cards).unwrap() {
                BotAction::Command(c) => d.apply(seat, &c, &cards),
                BotAction::Forfeit => d.forfeit(seat),
            };
            let Ok((next, events)) = next else { continue };
            let seen = spectator_events(&next, Some(seat), &events);
            assert_eq!(seen.len(), events.len());
            for (s, e) in seen.iter().zip(&events) {
                match (s, e) {
                    (SeenEvent::Public { event: DuelEvent::Drew { .. } | DuelEvent::DestinyDraw { .. } }, _) => {
                        panic!("a draw's cards went to a spectator: {e:?}")
                    }
                    (SeenEvent::Public { event: DuelEvent::Fused { .. } | DuelEvent::Equipped { .. } }, _) => {
                        panic!("a hand chain went to a spectator: {e:?}")
                    }
                    (SeenEvent::Public { event: DuelEvent::Summoned { unit, .. } | DuelEvent::Created { unit, .. } }, _) => {
                        let shown = next.view_as(Viewer::Nobody).units.iter().any(|u| u.id == *unit && u.card.is_some());
                        let revealed = events.iter().any(|x| matches!(x, DuelEvent::Revealed { unit: r, .. } if r == unit));
                        assert!(shown || revealed, "a face-down summon's card went to a spectator: {e:?}");
                    }
                    _ => {}
                }
            }
            d = next;
        }
        assert!(d.outcome.is_some(), "seed {seed}: the match ended");
    }
    assert!(states > 100 && face_down_seen > 50, "{states} states, {face_down_seen} face-down units");
}

/// Every hidden-card event, with the sentinel as the hidden card: the
/// spectator's events never carry it (both sides, OPEN or not).
#[test]
fn spectator_events_carry_no_hidden_card() {
    let cards = synthetic::catalogue();
    let d = duel(5);
    let (d, _) = d.apply(0, &DuelCommand::OpenSummon { cell: GridPos { column: 3, row: 5 } }, &cards).unwrap();
    let (d, events) = d.apply(0, &DuelCommand::ChooseSummon { hand: vec![0] }, &cards).unwrap();
    let unit = events
        .iter()
        .find_map(|e| match e {
            DuelEvent::Summoned { unit, .. } => Some(*unit),
            _ => None,
        })
        .unwrap();
    assert!(!d.unit(unit).unwrap().committed_face_up, "a face-down summon");
    let at = GridPos { column: 3, row: 5 };
    let hidden_events = vec![
        DuelEvent::Drew { side: 0, cards: vec![SENTINEL; 3] },
        DuelEvent::Drew { side: 1, cards: vec![SENTINEL] },
        DuelEvent::DestinyDraw { side: 1, card: SENTINEL },
        DuelEvent::Summoned { unit, card: SENTINEL, at, cost: 2 },
        DuelEvent::Created { unit, card: SENTINEL, at },
        DuelEvent::Fused { cards: [SENTINEL, SENTINEL], result: SENTINEL },
        DuelEvent::Equipped { side: 0, monster: SENTINEL, power_up: SENTINEL, value: 500 },
    ];
    for actor in [Some(0u8), Some(1), None] {
        let seen = spectator_events(&d, actor, &hidden_events);
        assert!(!bytes_have(&seen, SENTINEL), "{seen:?}");
        let update = ServerMessage::Update {
            revision: 3,
            actor: Actor::Server,
            cause: Cause::Command,
            events: seen.clone(),
            view: spectator_view(&scramble_hidden_from_all(&d, SENTINEL)),
        };
        assert!(!bytes_have(&update, SENTINEL));
        assert_eq!(seen[0], SeenEvent::Drew { side: 0, count: 3 });
        assert_eq!(seen[2], SeenEvent::DestinyDraw { side: 1 });
    }
    // The seat that owns them still sees them (the filter is unchanged).
    let own = crate::seen_events_with(&d, Some(0), 0, &hidden_events, [false; 2]);
    assert!(matches!(own[0], SeenEvent::Public { .. }));
    // A press's details card is removed against the spectator's view.
    let mut input = roses_core::duel::ui::DuelUi::new(&d);
    input.details = Some(roses_core::duel::ui::DetailsView { unit: None, card: d.sides[0].hand[0], page: 0 });
    let mut press = Press { base_revision: 1, through_revision: 1, key: 0x10, response: PressResponse::Moved, input, gap: 0 };
    crate::presence::sanitize(&mut press, &spectator_view(&d));
    assert_eq!(press.input.details.unwrap().card, 0);
    let mut input = roses_core::duel::ui::DuelUi::new(&d);
    input.details = Some(roses_core::duel::ui::DetailsView { unit: Some(unit), card: SENTINEL, page: 0 });
    let mut press = Press { input, ..press };
    crate::presence::sanitize(&mut press, &spectator_view(&d));
    assert_eq!(press.input.details.unwrap().card, 0, "a face-down unit's details");
}

/// The match messages as a client and a server from before spectating knew
/// them (only the parts that changed).
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
enum OldClientMessage {
    Hello { compat: Compat, join: OldJoin, #[serde(default)] presence: bool },
    Sync,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[allow(dead_code)]
enum OldJoin {
    Seat { seat: Option<u8> },
    Resume { token: String },
    Ticket { ticket: String },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
enum OldLobbyEvent {
    LobbyWelcome { lobby_version: u32, server: String, computer: bool, invite_required: bool, #[serde(default)] choices: bool },
}

#[test]
fn spectating_is_additive_on_the_wire_both_ways() {
    let compat = Compat::current("sha256:aa");
    // An older client's hello (no `spectate`) reads as before: it is never
    // sent the spectating messages.
    let old = json!({"type": "hello", "compat": compat, "join": {"kind": "ticket", "ticket": "t"}, "presence": true});
    let read: ClientMessage = serde_json::from_value(old.clone()).unwrap();
    assert!(matches!(read, ClientMessage::Hello { spectate: false, presence: true, .. }));
    // A new client that does not read them sends exactly the old hello.
    let quiet = ClientMessage::Hello { compat: compat.clone(), join: Join::Ticket { ticket: "t".into() }, deck: None, presence: true, spectate: false };
    assert_eq!(to_value(&quiet).unwrap(), old);
    // A new client's hello (spectate: true) is read by an older server
    // (the field is ignored), and so is the same join as before.
    let new = ClientMessage::hello(compat.clone(), Join::Ticket { ticket: "t".into() }, None, true);
    assert!(matches!(serde_json::from_value::<OldClientMessage>(to_value(&new).unwrap()).unwrap(), OldClientMessage::Hello { .. }));
    // `Join::Watch` is new: an older server cannot read it (a client sends
    // it only when the lobby welcome said `spectate`).
    let watch = ClientMessage::hello(compat, Join::Watch { ticket: "w".into() }, None, false);
    assert!(serde_json::from_value::<OldClientMessage>(to_value(&watch).unwrap()).is_err());
    // A seat's view is unchanged on the wire (no `spectator` key); a
    // spectator's says so.
    let d = duel(3);
    assert!(!serde_json::to_string(&player_view_with(&d, 0, [false; 2])).unwrap().contains("spectator"));
    assert!(serde_json::to_string(&spectator_view(&d)).unwrap().contains("\"spectator\":true"));
    let back: crate::PlayerView = serde_json::from_str(&serde_json::to_string(&player_view_with(&d, 1, [false; 2])).unwrap()).unwrap();
    assert!(!back.spectator);
    // The lobby: an older server's welcome offers no spectating; a new
    // server's welcome is read by an older client.
    let welcome: LobbyEvent =
        serde_json::from_str(r#"{"type":"lobby_welcome","lobby_version":1,"server":"S","computer":true,"invite_required":false,"choices":true}"#).unwrap();
    assert!(matches!(welcome, LobbyEvent::LobbyWelcome { spectate: false, .. }));
    let new_welcome = LobbyEvent::LobbyWelcome { lobby_version: 1, server: "S".into(), computer: true, invite_required: false, choices: true, email: false, spectate: true, modes: false };
    assert!(serde_json::from_value::<OldLobbyEvent>(to_value(&new_welcome).unwrap()).is_ok());
    let quiet_welcome = LobbyEvent::LobbyWelcome { lobby_version: 1, server: "S".into(), computer: true, invite_required: false, choices: true, email: false, spectate: false, modes: false };
    assert!(!serde_json::to_string(&quiet_welcome).unwrap().contains("spectate"));
    // The new requests and events round-trip.
    for r in [LobbyRequest::ListMatches, LobbyRequest::Watch { match_id: "m".into() }, LobbyRequest::AllowSpectators { allow: false }] {
        assert_eq!(serde_json::from_value::<LobbyRequest>(to_value(&r).unwrap()).unwrap(), r);
    }
    let live = LobbyEvent::LiveMatches {
        matches: vec![LiveMatch { match_id: "m".into(), players: ["Yugi".into(), "Pegasus Crawford".into()], map: Some(MapChoice::Game { index: 0x17 }), round: 3, watching: 2 , seats: Vec::new(), mode: None }],
    };
    assert_eq!(serde_json::from_value::<LobbyEvent>(to_value(&live).unwrap()).unwrap(), live);
    for m in [
        ServerMessage::Watching { match_id: "m".into(), names: ["A".into(), "B".into()], options: MatchOptions::default(), delay_turns: 1, multi: None },
        ServerMessage::Watchers { count: 4 },
        ServerMessage::Refused { reason: Refusal::WatchersFull },
    ] {
        assert_eq!(serde_json::from_value::<ServerMessage>(to_value(&m).unwrap()).unwrap(), m);
    }
    // The protocol version did not change: both directions still match.
    assert_eq!(PROTOCOL_VERSION, 4);
}

/// A spectator's client state: no seat, never its turn, the count kept.
#[test]
fn a_spectators_client_state_never_acts() {
    let d = duel(9);
    let mut st = crate::client::ClientState::new();
    st.apply(&ServerMessage::Watching { match_id: "m".into(), names: ["A".into(), "B".into()], options: MatchOptions::default(), delay_turns: 0, multi: None });
    st.apply(&ServerMessage::Snapshot {
        snapshot: Snapshot { revision: 4, last_seq: 0, status: MatchStatus::Running, view: spectator_view(&d), presence: vec![true; 2] },
    });
    st.apply(&ServerMessage::Watchers { count: 3 });
    assert!(st.spectator && st.seat.is_none() && !st.my_turn());
    assert_eq!((st.watchers, st.revision, st.names.clone()), (3, 4, Some(["A".to_string(), "B".to_string()])));
}
