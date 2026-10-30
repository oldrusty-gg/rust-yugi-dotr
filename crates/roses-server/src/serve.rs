//! Attaches transport connections to one [`OnlineMatch`]: handshakes, seat
//! (re)attachment, delivery of each seat's messages, disconnect detection.
//! Single-threaded polling; wall-clock milliseconds since start become the
//! match's ticks.
//!
//! Limits (before any non-loopback use): frames from clients are at most
//! `roses_net::transport::MAX_CLIENT_FRAME` bytes (the TCP listener's
//! default); each connection has a message budget ([`RATE_PER_SECOND`],
//! bursts up to [`RATE_BURST`]) and is closed when it overspends (a seated
//! one is treated as a disconnect: the match pauses, the seat may resume);
//! at most [`MAX_PENDING`] connections may be in their handshake, each for
//! at most [`HANDSHAKE_MS`].

use crate::online::{OnlineMatch, Outbox};
use roses_net::transport::{Connection, LoopbackListener, Received};
use roses_net::{ClientMessage, Refusal, ServerMessage};
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub type ServerConn = Box<dyn Connection<ServerMessage, ClientMessage> + Send>;

/// A source of new connections.
pub trait Acceptor {
    fn accept(&mut self) -> io::Result<Option<ServerConn>>;
}

impl Acceptor for LoopbackListener {
    fn accept(&mut self) -> io::Result<Option<ServerConn>> {
        Ok(LoopbackListener::accept::<ServerMessage, ClientMessage>(self)?
            .map(|c| Box::new(c) as ServerConn))
    }
}

/// Connections handed over through a channel (in-memory tests).
impl Acceptor for std::sync::mpsc::Receiver<ServerConn> {
    fn accept(&mut self) -> io::Result<Option<ServerConn>> {
        Ok(self.try_recv().ok())
    }
}

/// How long the server keeps running after the match ends so final
/// messages are read and late reconnects get the result.
pub const LINGER: Duration = Duration::from_millis(500);
/// Messages per second a connection may send on average: a client reads
/// its pad once per 25 Hz update, so a person's presses (streamed) plus
/// their intentions stay well below this.
pub const RATE_PER_SECOND: u64 = 100;
/// Messages a connection may send at once.
pub const RATE_BURST: u64 = 200;
/// Connections allowed in their handshake at the same time.
pub const MAX_PENDING: usize = 64;
/// Time a connection has to send its `Hello`.
pub const HANDSHAKE_MS: u64 = 5_000;

/// A connection's message budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub per_second: u64,
    pub burst: u64,
}

impl Limits {
    /// [`RATE_PER_SECOND`] with bursts of [`RATE_BURST`].
    pub const DEFAULT: Self = Self { per_second: RATE_PER_SECOND, burst: RATE_BURST };
    /// No budget: for in-process tests whose clients run their 25 Hz
    /// updates as fast as the machine allows.
    pub const UNLIMITED: Self = Self { per_second: u64::MAX / 4, burst: u64::MAX / 4000 };
}

/// A token bucket of messages (in thousandths of a message).
#[derive(Clone, Copy, Debug)]
pub struct Budget {
    milli: u64,
    at: u64,
    limits: Limits,
}

impl Budget {
    pub fn new(now: u64) -> Self {
        Self::with_limits(now, Limits::DEFAULT)
    }

    pub fn with_limits(now: u64, limits: Limits) -> Self {
        Self { milli: limits.burst.saturating_mul(1000), at: now, limits }
    }

    /// Spend one message at `now` (ms); false when the budget is empty.
    pub fn take(&mut self, now: u64) -> bool {
        let refill = now.saturating_sub(self.at).saturating_mul(self.limits.per_second);
        self.milli = self.milli.saturating_add(refill).min(self.limits.burst.saturating_mul(1000));
        self.at = now.max(self.at);
        if self.milli < 1000 {
            return false;
        }
        self.milli -= 1000;
        true
    }
}

/// Run the match until it ends (plus [`LINGER`]) or `stop` is set.
/// `log` receives one line per notable event.
pub fn serve(
    acceptor: &mut impl Acceptor,
    game: &mut OnlineMatch,
    stop: &AtomicBool,
    log: impl FnMut(&str),
) -> io::Result<()> {
    serve_with_limits(acceptor, game, stop, Limits::DEFAULT, log)
}

/// [`serve`] with a given per-connection message budget.
pub fn serve_with_limits(
    acceptor: &mut impl Acceptor,
    game: &mut OnlineMatch,
    stop: &AtomicBool,
    limits: Limits,
    mut log: impl FnMut(&str),
) -> io::Result<()> {
    let start = Instant::now();
    let now = || start.elapsed().as_millis() as u64;
    let mut pending: Vec<(ServerConn, u64)> = Vec::new();
    let mut seats: [Option<ServerConn>; 2] = [None, None];
    let mut budgets = [Budget::with_limits(0, limits); 2];
    let mut ended_at: Option<Instant> = None;
    while !stop.load(Ordering::Relaxed) {
        let mut busy = false;
        while let Some(mut conn) = acceptor.accept()? {
            busy = true;
            if pending.len() >= MAX_PENDING {
                log("refused a connection: too many handshakes in progress");
                conn.close();
                continue;
            }
            pending.push((conn, now()));
        }
        // Handshakes (one message each; a silent one times out).
        let mut i = 0;
        while i < pending.len() {
            if now().saturating_sub(pending[i].1) > HANDSHAKE_MS {
                let (mut conn, _) = pending.swap_remove(i);
                log("closed a connection that sent no hello in time");
                conn.close();
                continue;
            }
            match pending[i].0.poll() {
                Received::Empty => i += 1,
                Received::Message(ClientMessage::Hello { compat, join, deck, presence, .. }) => {
                    busy = true;
                    let (mut conn, _) = pending.swap_remove(i);
                    match game.hello_streaming(&compat, &join, deck.as_ref(), presence, now()) {
                        Ok((seat, out)) => {
                            let how = match join {
                                roses_net::Join::Seat { .. } => "joined",
                                roses_net::Join::Resume { .. } | roses_net::Join::Ticket { .. } | roses_net::Join::Watch { .. } => "resumed",
                            };
                            log(&format!("seat {seat} {how}"));
                            if let Some(mut old) = seats[usize::from(seat)].replace(conn) {
                                old.close();
                            }
                            budgets[usize::from(seat)] = Budget::with_limits(now(), limits);
                            deliver(&mut seats, game, out, now(), &mut log);
                        }
                        Err(reason) => {
                            log(&format!("refused a connection: {reason:?}"));
                            let _ = conn.send(&ServerMessage::Refused { reason });
                            conn.close();
                        }
                    }
                }
                Received::Message(_) => {
                    let (mut conn, _) = pending.swap_remove(i);
                    let _ = conn.send(&ServerMessage::Refused {
                        reason: Refusal::ExpectedHello,
                    });
                    conn.close();
                }
                Received::Malformed(message) => {
                    let (mut conn, _) = pending.swap_remove(i);
                    let _ = conn.send(&ServerMessage::ProtocolError { message });
                    conn.close();
                }
                Received::Closed => {
                    pending.swap_remove(i);
                }
            }
        }
        // Seated connections.
        for seat in 0u8..2 {
            loop {
                let Some(conn) = seats[usize::from(seat)].as_mut() else {
                    break;
                };
                match conn.poll() {
                    Received::Empty => break,
                    Received::Message(m) => {
                        busy = true;
                        if !budgets[usize::from(seat)].take(now()) {
                            log(&format!("seat {seat} exceeded the message rate; closing its connection"));
                            let _ = conn.send(&ServerMessage::ProtocolError {
                                message: "message rate limit exceeded".into(),
                            });
                            conn.close();
                            seats[usize::from(seat)] = None;
                            let out = game.disconnect(seat, now());
                            deliver(&mut seats, game, out, now(), &mut log);
                            break;
                        }
                        let out = game.handle(seat, m, now());
                        deliver(&mut seats, game, out, now(), &mut log);
                    }
                    Received::Malformed(message) => {
                        log(&format!("seat {seat} sent a malformed message: {message}"));
                        let _ = conn.send(&ServerMessage::ProtocolError { message });
                        conn.close();
                        seats[usize::from(seat)] = None;
                        let out = game.disconnect(seat, now());
                        deliver(&mut seats, game, out, now(), &mut log);
                    }
                    Received::Closed => {
                        log(&format!("seat {seat} disconnected"));
                        seats[usize::from(seat)] = None;
                        let out = game.disconnect(seat, now());
                        deliver(&mut seats, game, out, now(), &mut log);
                    }
                }
            }
        }
        let out = game.tick(now());
        deliver(&mut seats, game, out, now(), &mut log);
        if let Some((outcome, cause)) = game.ended() {
            let at = *ended_at.get_or_insert_with(|| {
                log(&format!("match ended: {outcome:?} ({cause:?}) at revision {}", game.revision()));
                Instant::now()
            });
            if at.elapsed() >= LINGER || seats.iter().all(Option::is_none) {
                break;
            }
        }
        if !busy {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    for conn in seats.iter_mut().flatten() {
        conn.close();
    }
    Ok(())
}

fn deliver(
    seats: &mut [Option<ServerConn>; 2],
    game: &mut OnlineMatch,
    out: Outbox,
    now: u64,
    log: &mut impl FnMut(&str),
) {
    let mut queue: std::collections::VecDeque<_> = out.into();
    while let Some((seat, msg)) = queue.pop_front() {
        let Some(conn) = seats[usize::from(seat)].as_mut() else {
            continue;
        };
        if conn.send(&msg).is_err() {
            log(&format!("seat {seat} unreachable"));
            seats[usize::from(seat)] = None;
            queue.extend(game.disconnect(seat, now));
        }
    }
}
