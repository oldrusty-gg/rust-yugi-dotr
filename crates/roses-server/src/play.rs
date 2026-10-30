//! A client runner: handshake, computer-driven intentions from the seat's
//! own view ([`roses_net::client::Bot`]), reconnect with the resume token
//! (resending undecided intentions), until the match ends.

use roses_core::duel::Catalogue;
use roses_net::client::{Bot, BotAction, ClientState};
use roses_net::transport::{Connection, Received};
use roses_net::{ClientMessage, Compat, Decision, DeckSubmission, Join, ServerMessage};
use std::io;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct PlayOptions {
    pub join: Join,
    /// The seat's own deck (for a server that takes submitted decks).
    pub deck: Option<DeckSubmission>,
    /// Drop the connection once this revision is reached, stay away for
    /// `away`, then resume (reconnect testing).
    pub drop_at_revision: Option<u64>,
    pub away: Duration,
    /// Give up after this long.
    pub timeout: Duration,
    /// How long to sleep when nothing happened (the hub's computer
    /// opponent waits longer: one thread per match).
    pub idle: Duration,
    /// Wait this long before each intention (a slower player; tests).
    pub think: Duration,
    /// Send no intention until this is set (tests: a spectator joins first).
    pub gate: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    /// Say in the hello that this client reads the spectating messages (it
    /// is then sent the "N watching" count); false: as older clients.
    pub spectate: bool,
}

impl Default for PlayOptions {
    fn default() -> Self {
        Self {
            join: Join::Seat { seat: None },
            deck: None,
            drop_at_revision: None,
            away: Duration::from_millis(50),
            timeout: Duration::from_secs(120),
            idle: Duration::from_millis(1),
            think: Duration::ZERO,
            gate: None,
            spectate: false,
        }
    }
}

#[derive(Debug, Default)]
pub struct PlayReport {
    pub state: ClientState,
    pub received: Vec<ServerMessage>,
    pub sent: Vec<ClientMessage>,
    pub reconnects: u32,
    pub rejected: u32,
}

/// Play one seat until the match ends.
pub fn play_bot<C, F>(
    mut connect: F,
    compat: &Compat,
    cards: &Catalogue,
    options: PlayOptions,
    mut on_message: impl FnMut(&ServerMessage),
) -> Result<PlayReport, String>
where
    C: Connection<ClientMessage, ServerMessage>,
    F: FnMut() -> io::Result<C>,
{
    let start = Instant::now();
    let mut report = PlayReport {
        state: ClientState::new(),
        ..PlayReport::default()
    };
    let mut bot = Bot::new();
    let mut dropped = false;
    // The revision an accepted intention produced, until its Update arrives.
    let mut awaiting: u64 = 0;
    let mut conn = connect().map_err(|e| e.to_string())?;
    let send = |conn: &mut C, report: &mut PlayReport, m: ClientMessage| {
        report.sent.push(m.clone());
        conn.send(&m).map_err(|e| e.to_string())
    };
    send(
        &mut conn,
        &mut report,
        ClientMessage::Hello {
            compat: compat.clone(),
            join: options.join.clone(),
            deck: options.deck.clone(),
            presence: false,
            spectate: options.spectate,
        },
    )?;
    loop {
        if start.elapsed() > options.timeout {
            return Err(format!(
                "timed out at revision {} (status {:?})",
                report.state.revision, report.state.status
            ));
        }
        let mut busy = false;
        let mut closed = false;
        loop {
            match conn.poll() {
                Received::Message(m) => {
                    busy = true;
                    on_message(&m);
                    report.state.apply(&m);
                    match &m {
                        ServerMessage::Refused { reason } => {
                            return Err(format!("refused: {reason:?}"))
                        }
                        ServerMessage::LobbyRefused { .. } => {
                            let why = report.state.refused.as_ref().map(|r| r.plain()).unwrap_or_default();
                            return Err(format!("refused: {why}"));
                        }
                        ServerMessage::Accepted { revision, .. }
                        | ServerMessage::Duplicate {
                            decision: Decision::Accepted { revision },
                            ..
                        } => awaiting = awaiting.max(*revision),
                        ServerMessage::Rejected { .. } => report.rejected += 1,
                        _ => {}
                    }
                    report.received.push(m);
                }
                Received::Empty => break,
                Received::Closed => {
                    closed = true;
                    break;
                }
                Received::Malformed(e) => return Err(format!("malformed server message: {e}")),
            }
        }
        if report.state.ended.is_some() {
            conn.close();
            return Ok(report);
        }
        let drop_now = !dropped
            && options
                .drop_at_revision
                .is_some_and(|r| report.state.revision >= r && report.state.my_turn());
        if closed || drop_now {
            if drop_now {
                dropped = true;
                conn.close();
                std::thread::sleep(options.away);
            }
            let token = report
                .state
                .token
                .clone()
                .ok_or("the connection closed before a seat was given")?;
            conn = connect().map_err(|e| e.to_string())?;
            report.reconnects += 1;
            send(
                &mut conn,
                &mut report,
                ClientMessage::Hello {
                    compat: compat.clone(),
                    join: Join::Resume { token },
                    deck: None,
                    presence: false,
                    spectate: options.spectate,
                },
            )?;
            for m in report.state.resend() {
                send(&mut conn, &mut report, m)?;
            }
            continue;
        }
        let gated = options.gate.as_ref().is_some_and(|g| !g.load(std::sync::atomic::Ordering::Relaxed));
        if !gated && report.state.my_turn() && report.state.unacked.is_empty() && report.state.revision >= awaiting {
            if !options.think.is_zero() {
                std::thread::sleep(options.think);
            }
            let view = report.state.view.clone().expect("my_turn has a view");
            match bot.next(&view, report.state.revision, cards) {
                Some(BotAction::Command(c)) => {
                    let m = report.state.command(c);
                    send(&mut conn, &mut report, m)?;
                    busy = true;
                }
                Some(BotAction::Forfeit) => {
                    let m = report.state.forfeit();
                    send(&mut conn, &mut report, m)?;
                    busy = true;
                }
                None => {}
            }
        }
        if !busy {
            std::thread::sleep(options.idle);
        }
    }
}
