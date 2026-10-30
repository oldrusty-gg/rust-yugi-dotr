//! Two clients and the headless server over loopback TCP: a full match with
//! one seat dropping its connection and resuming, an incompatible client
//! refused at the handshake, a malformed frame, and the server's replay.

use roses_net::transport::{recv_timeout, Connection, FramedTcp, LoopbackListener, Received};
use roses_net::{RulesTables, data_fingerprint, synthetic, ClientMessage, Compat, Join, OnlinePolicy, Refusal, ServerMessage};
use roses_server::play::{play_bot, PlayOptions};
use roses_server::{replay, MatchSetup, OnlineMatch};
use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

type Client = FramedTcp<ClientMessage, ServerMessage>;

fn refused(addr: SocketAddr, hello: ClientMessage) -> ServerMessage {
    let mut c = Client::connect(addr).unwrap();
    c.send(&hello).unwrap();
    match recv_timeout(&mut c, Duration::from_secs(10)) {
        Received::Message(m) => m,
        other => panic!("{other:?}"),
    }
}

#[test]
fn two_tcp_clients_finish_a_match_with_a_reconnect() {
    let cards = synthetic::catalogue();
    let compat = Compat::current(data_fingerprint(&cards, &RulesTables::empty()));
    let setup = MatchSetup {
        seed: 4242,
        terrain: synthetic::board(),
        leaders: synthetic::leaders(),
        deck_sources: synthetic::decks(),
        first: 0,
        leader_abilities: [vec![], vec![]],
        custom_rules: None,
        leader_ranks: [0; 2],
        destiny_rows: None,
    };
    let mut game = OnlineMatch::new(
        "tcp-test",
        setup,
        cards.clone(),
        OnlinePolicy {
            reconnect_grace: 10_000,
            turn_limit: Some(30_000),
            timeouts_before_forfeit: 3,
        },
        [roses_server::secret_token(), roses_server::secret_token()],
    )
    .unwrap();
    let mut listener = LoopbackListener::bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr().unwrap();
    let stop = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let server = scope.spawn(|| {
            roses_server::serve::serve(&mut listener, &mut game, &stop, |line| eprintln!("server: {line}"))
                .unwrap();
            game
        });
        // Incompatible data and a non-Hello first message are refused.
        let wrong = Compat::current("sha256:not-the-same-import");
        assert!(matches!(
            refused(addr, ClientMessage::Hello { compat: wrong, join: Join::Seat { seat: None }, deck: None, presence: false, spectate: false }),
            ServerMessage::Refused { reason: Refusal::Incompatible { .. } }
        ));
        assert!(matches!(
            refused(addr, ClientMessage::Sync),
            ServerMessage::Refused { reason: Refusal::ExpectedHello }
        ));
        // A malformed first frame gets a protocol error.
        let mut junk = Client::connect(addr).unwrap();
        junk.send_raw(b"\x00\xffnot json").unwrap();
        assert!(matches!(
            recv_timeout(&mut junk, Duration::from_secs(10)),
            Received::Message(ServerMessage::ProtocolError { .. })
        ));
        let seat = |n: u8, drop: Option<u64>| {
            let (compat, cards) = (compat.clone(), cards.clone());
            scope.spawn(move || {
                play_bot(
                    || Client::connect(addr),
                    &compat,
                    &cards,
                    PlayOptions {
                        join: Join::Seat { seat: Some(n) },
                        deck: None,
                        drop_at_revision: drop,
                        away: Duration::from_millis(100),
                        timeout: Duration::from_secs(120),
                        ..PlayOptions::default()
                    },
                    |_| {},
                )
            })
        };
        let a = seat(0, None);
        let b = seat(1, Some(8));
        let a = a.join().unwrap().unwrap();
        let b = b.join().unwrap().unwrap();
        let game = server.join().unwrap();
        let (outcome, cause) = game.ended().expect("ended");
        assert_eq!(a.state.ended, Some((outcome, cause)));
        assert_eq!(b.state.ended, Some((outcome, cause)));
        assert_eq!(b.reconnects, 1);
        // Seat 0 saw the pause and the resume.
        assert!(a.received.iter().any(|m| matches!(m, ServerMessage::Paused { .. })));
        assert!(a.received.iter().any(|m| matches!(m, ServerMessage::Resumed)));
        // Every revision reached each connected seat once, in order.
        for report in [&a, &b] {
            let revs: Vec<u64> = report
                .received
                .iter()
                .filter_map(|m| match m {
                    ServerMessage::Update { revision, .. } => Some(*revision),
                    _ => None,
                })
                .collect();
            assert!(revs.windows(2).all(|w| w[0] < w[1]), "{revs:?}");
            assert_eq!(*revs.last().unwrap(), game.revision());
        }
        // Nothing in either stream carries hidden state.
        for report in [&a, &b] {
            let json = serde_json::to_string(&report.received).unwrap();
            assert!(!json.contains("\"rng\"") && !json.contains("\"deck\"") && !json.contains("\"seed\""));
        }
        assert_eq!(replay(game.replay(), &cards, &RulesTables::empty()).unwrap(), *game.duel());
        eprintln!("tcp match: {outcome:?} ({cause:?}) at revision {}", game.revision());
    });
}
