//! The friends-online subcommands of `roses-server`:
//!
//!   roses-server hub --duel-import <dir> --db <accounts.db>
//!       [--listen <addr:port> --tls-cert <fullchain.pem> --tls-key <privkey.pem>]
//!       [--local <127.0.0.1:port>] [--name <text>] [--computer] [--open-signup | --invite-only]
//!       [--replay-dir <dir> [--replay-days N]] [--max-matches N] [--max-connections N]
//!       [--max-connections-per-address N] [--name-filter <file>]
//!       [--max-spectators N] [--max-spectators-per-match N] [--spectate-delay-turns N]
//!       [--multi-computer-delay-ms N]
//!       [--leader-ability ..] [--card-display ..] [--grace-ms ..] [--turn-limit-ms ..] [--timeouts N]
//!       [--email-log <mail.jsonl> | --email-provider postmark|resend --email-from <"Name <addr>">]
//!       [--brand <name>] [--require-email] [--email-web <url>]
//!       (email addresses, verification and password reset: crate::hub::email_hub;
//!       the provider's API key is read from PLATFORM_EMAIL_API_KEY)
//!   roses-server accounts --db <accounts.db> invite [--uses N] [--days N] [--note <text>]
//!   roses-server accounts --db <accounts.db> invites | users | matches
//!   roses-server accounts --db <accounts.db> revoke <code> | disable <name> | enable <name>
//!   roses-server accounts --db <accounts.db> reset <name>     (a one-use password-reset code)
//!   roses-server accounts --db <accounts.db> password <name>  (a temporary password; the player
//!       logs in with it on the launcher's Profile page and changes it there)
//!   roses-server accounts --db <accounts.db> backup <new-file.db>
//!   roses-server accounts --db <accounts.db> delete <name> [--replay-dir <dir>]
//!       (erase an account: user, sessions, reset codes; its name in the match
//!       log becomes "(deleted)"; with --replay-dir also its matches' replays)
//!   roses-server accounts --db <accounts.db> prune-matches <days>
//!       (delete match-log rows older than <days> days)
//!   roses-server tls self-signed --out <dir> --name <host or IP> [--name ..]
//!   roses-server tls fingerprint <cert.pem>
//!   roses-server loadtest --addr <host:port> ... (see roses_server::loadtest)
//!
//! `--open-signup`: anyone with the game may make an account without an
//! invite code (at most 5 new accounts an hour per address, 60 an hour in
//! all, reserved names refused); the friends beta runs with it. Without it
//! (or with `--invite-only`) sign-up needs an invite code. Invite and
//! password-reset codes work in both modes. `--max-matches` defaults to 100 and
//! `--max-connections` to 512 (a match between two players uses four:
//! two lobby and two match connections).
//! `--max-connections-per-address` defaults to 16 (0: no limit; loopback
//! addresses are never limited). `--name-filter <file>` adds names to the
//! built-in filter for new accounts (format in roses_server::name_filter).
//! `--replay-days N` deletes replays in `--replay-dir` older than N days
//! (checked hourly).
//! Spectating (a port addition): `--max-spectators` (default 256 on the
//! server) and `--max-spectators-per-match` (default 8) cap spectators (0
//! turns spectating off: the lobby welcome then does not offer it);
//! `--spectate-delay-turns N` (default 0) shows spectators a turn's moves
//! only once N later turns have begun. Spectators get the view of nobody
//! (never a hand or a face-down card); the replays are never served.
//! MODES (a port addition): N-player rooms (free-for-all, 2v2) are always
//! offered to clients that ask for them; `--multi-computer-delay-ms N`
//! (default 350) spaces out the computer players' commands in those
//! matches so the others can follow them.
//!
//! Personal data and retention: the account database keeps each account's
//! name, password hash (Argon2id), creation time, the invite it used,
//! session-token hashes (30 days after last use, pruned hourly) and the
//! match log (both names, times, outcome) until `prune-matches`. Addresses
//! are held only in memory for the rate limits and are never written to
//! the log or the database. The log (stdout; systemd's journal on a
//! server) has times, account names and match ids: limit the journal's
//! retention on the server (`journald.conf`: `MaxRetentionSec=14day`,
//! `SystemMaxUse=200M`), and remember that sshd, the firewall and any
//! reverse proxy keep their own logs with addresses. Run `prune-matches`
//! and `--replay-days` (e.g. 30 days) to match the published privacy
//! notice, and `delete` for erasure requests (backups made with `backup`
//! keep the old data until they are rotated).
//!
//! `--listen` takes any address and requires TLS; `--local` is unencrypted
//! and loopback-only (playing on the server's own PC, tests). Times in the
//! log are UTC.

use crate::accounts::Accounts;
use crate::hub::{Hub, HubAcceptor, HubData, HubSettings};
use crate::name_filter::NameFilter;
use crate::replay::MatchSetup;
use crate::tls::{file_fingerprint, self_signed, TlsListener};
use roses_net::transport::LoopbackListener;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// Run a friends-online subcommand; `None` when `first` is not one.
pub fn main(first: &str, rest: Vec<String>) -> Option<Result<(), String>> {
    match first {
        "hub" => Some(hub(rest)),
        "accounts" => Some(accounts(rest)),
        "tls" => Some(tls(rest)),
        "loadtest" => Some(crate::loadtest::main(rest)),
        _ => None,
    }
}

/// `YYYY-MM-DD hh:mm:ss` UTC (civil-from-days, no calendar crate).
pub fn utc_stamp(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}", rem / 3600, rem % 3600 / 60, rem % 60)
}

fn switch(v: &str, on: &str, off: &str) -> Result<[bool; 2], String> {
    let one = |x: &str| match x {
        x if x == on => Ok(true),
        x if x == off => Ok(false),
        x => Err(format!("{x}: expected {on} or {off}")),
    };
    match v.split_once(',') {
        Some((a, b)) => Ok([one(a)?, one(b)?]),
        None => Ok([one(v)?; 2]),
    }
}

fn hub(args: Vec<String>) -> Result<(), String> {
    let mut args = args.into_iter();
    let mut import: Option<PathBuf> = None;
    let mut synthetic_data = false;
    let mut db: Option<PathBuf> = None;
    let mut listen: Option<String> = None;
    let mut local: Option<String> = None;
    let (mut cert, mut key): (Option<PathBuf>, Option<PathBuf>) = (None, None);
    let mut settings = HubSettings::default();
    let mut computer = false;
    let mut email = crate::hub::email_hub::EmailArgs::default();
    while let Some(a) = args.next() {
        let mut value = || args.next().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--duel-import" => import = Some(value()?.into()),
            "--synthetic" => synthetic_data = true,
            "--db" => db = Some(value()?.into()),
            "--listen" => listen = Some(value()?),
            "--local" => local = Some(value()?),
            "--tls-cert" => cert = Some(value()?.into()),
            "--tls-key" => key = Some(value()?.into()),
            "--name" => settings.name = value()?,
            "--computer" => computer = true,
            "--open-signup" => settings.open_signup = true,
            "--invite-only" => settings.open_signup = false,
            "--replay-dir" => settings.replay_dir = Some(value()?.into()),
            "--max-matches" => settings.max_matches = value()?.parse().map_err(|_| "bad --max-matches")?,
            "--max-connections" => settings.max_connections = value()?.parse().map_err(|_| "bad --max-connections")?,
            "--max-connections-per-address" => {
                settings.max_connections_per_address = value()?.parse().map_err(|_| "bad --max-connections-per-address")?
            }
            "--max-spectators" => settings.max_spectators = value()?.parse().map_err(|_| "bad --max-spectators")?,
            "--max-spectators-per-match" => {
                settings.max_spectators_per_match = value()?.parse().map_err(|_| "bad --max-spectators-per-match")?
            }
            "--multi-computer-delay-ms" => {
                settings.multi_computer_delay_ms = value()?.parse().map_err(|_| "bad --multi-computer-delay-ms")?
            }
            "--spectate-delay-turns" => {
                settings.spectate_delay_turns = value()?.parse().map_err(|_| "bad --spectate-delay-turns")?
            }
            "--name-filter" => {
                settings.name_filter = Arc::new(NameFilter::builtin_and_file(&PathBuf::from(value()?))?);
            }
            "--replay-days" => settings.replay_keep_days = Some(value()?.parse().map_err(|_| "bad --replay-days")?),
            "--grace-ms" => settings.policy.reconnect_grace = value()?.parse().map_err(|_| "bad --grace-ms")?,
            "--turn-limit-ms" => {
                let ms: u64 = value()?.parse().map_err(|_| "bad --turn-limit-ms")?;
                settings.policy.turn_limit = (ms > 0).then_some(ms);
            }
            "--timeouts" => settings.policy.timeouts_before_forfeit = value()?.parse().map_err(|_| "bad --timeouts")?,
            "--leader-ability" => settings.options.leader_ability = switch(&value()?, "use", "not-use")?,
            "--card-display" => settings.options.card_display_open = switch(&value()?, "open", "hide")?,
            // Rehearsals only: every 2-player match uses this seed and the
            // host / challenger sits at seat 0; MODES rooms' N-player
            // matches use it too (HubSettings::fixed_seed).
            "--match-seed" => settings.fixed_seed = Some(value()?.parse().map_err(|_| "bad --match-seed")?),
            other if email.take(other, &mut value)? => {}
            other => return Err(format!("unknown hub argument {other}")),
        }
    }
    let db = db.ok_or("--db <accounts.db> is required (created when missing)")?;
    let data = if synthetic_data {
        HubData::synthetic(computer)
    } else {
        let dir = import.ok_or("--duel-import <dir> is required (your own `roses import-duel` output)")?;
        let d = roses_net::data::load_duel_import(&dir)?;
        let first = d.first_duel;
        let deck = roses_net::deck::from_import_side(&first.sides[1]);
        HubData {
            cards: d.catalogue,
            tables: d.tables,
            setup: Arc::new(move |seed| MatchSetup::from_import(&first, seed)),
            computer_deck: computer.then_some(deck),
            maps: d.maps,
            computer_decks: d.opponents.into_iter().map(|o| (o.name, o.deck)).collect(),
            computer_tables: d.computer_tables.map(Arc::new),
        }
    };
    let accounts = Accounts::open(&db)?;
    let mut acceptors: Vec<Box<dyn HubAcceptor + Send>> = Vec::new();
    let mut said = Vec::new();
    if let Some(addr) = &listen {
        let (cert, key) = match (&cert, &key) {
            (Some(c), Some(k)) => (c, k),
            _ => return Err("--listen needs --tls-cert and --tls-key (unencrypted play is loopback-only: --local)".into()),
        };
        let l = TlsListener::bind(addr.parse().map_err(|_| format!("bad --listen {addr}"))?, cert, key).map_err(|e| e.to_string())?;
        said.push(format!(
            "listening on {} with TLS (certificate {}, fingerprint {})",
            l.local_addr().map_err(|e| e.to_string())?,
            cert.display(),
            file_fingerprint(cert)?
        ));
        acceptors.push(Box::new(l));
    }
    if let Some(addr) = &local {
        let l = LoopbackListener::bind(addr.parse().map_err(|_| format!("bad --local {addr}"))?).map_err(|e| e.to_string())?;
        said.push(format!("listening on {} (this computer only, no TLS)", l.local_addr().map_err(|e| e.to_string())?));
        acceptors.push(Box::new(l));
    }
    if acceptors.is_empty() {
        return Err("give --listen <addr:port> with TLS, and/or --local 127.0.0.1:<port>".into());
    }
    let log = |line: &str| println!("{} {line}", utc_stamp(crate::accounts::now_seconds()));
    let email = email.build(&db)?;
    let mut hub = Hub::new(settings.clone(), data, accounts, log);
    if let Some(email) = email {
        log(&email.describe());
        hub = hub.with_email(email);
    }
    log(&format!("roses-server hub \"{}\": rules data {}", settings.name, hub.compat().data_fingerprint));
    for line in said {
        log(&line);
    }
    log(&format!(
        "accounts in {}; {} matches at most; {} connections per address at most; {} names filtered; computer opponent {}; sign-up {}; spectators {} per match, {} in all, delay {} turns",
        db.display(),
        settings.max_matches,
        settings.max_connections_per_address,
        settings.name_filter.len(),
        if computer { "on" } else { "off" },
        if settings.open_signup { "open (no invite needed)" } else { "by invite code" },
        settings.max_spectators_per_match,
        settings.max_spectators,
        settings.spectate_delay_turns
    ));
    let stop = AtomicBool::new(false);
    hub.run(&mut acceptors, &stop).map_err(|e| e.to_string())
}

fn accounts(args: Vec<String>) -> Result<(), String> {
    let mut args = args.into_iter().peekable();
    let mut db: Option<PathBuf> = None;
    if args.peek().map(String::as_str) == Some("--db") {
        args.next();
        db = Some(args.next().ok_or("--db needs a file")?.into());
    }
    let db = db.ok_or("accounts needs --db <accounts.db> first")?;
    let mut a = Accounts::open(&db)?;
    let command = args.next().ok_or("accounts: invite, invites, revoke, users, disable, enable, reset, password, matches, backup, delete or prune-matches")?;
    let rest: Vec<String> = args.collect();
    let arg = |i: usize, what: &str| rest.get(i).cloned().ok_or(format!("{command} needs {what}"));
    match command.as_str() {
        "invite" => {
            let (mut uses, mut days, mut note) = (1i64, None, None);
            let mut i = 0;
            while i < rest.len() {
                let v = rest.get(i + 1).ok_or(format!("{} needs a value", rest[i]))?;
                match rest[i].as_str() {
                    "--uses" => uses = v.parse().map_err(|_| "bad --uses")?,
                    "--days" => days = Some(v.parse().map_err(|_| "bad --days")?),
                    "--note" => note = Some(v.clone()),
                    o => return Err(format!("unknown invite argument {o}")),
                }
                i += 2;
            }
            let code = a.create_invite(uses, note.as_deref(), days)?;
            println!("{code}");
            eprintln!("invite for {uses} sign-up(s){}", days.map_or(String::new(), |d| format!(", valid {d} days")));
        }
        "invites" => {
            for i in a.invites()? {
                println!(
                    "{}-{}  uses left {}  {}{}{}",
                    &i.code[..4.min(i.code.len())],
                    &i.code[4.min(i.code.len())..],
                    i.uses_left,
                    i.for_user.map_or(String::new(), |u| format!("password reset for {u}  ")),
                    i.note.unwrap_or_default(),
                    i.expires.map_or(String::new(), |t| format!("  expires {}", utc_stamp(t)))
                );
            }
        }
        "revoke" => println!("{}", if a.revoke_invite(&arg(0, "a code")?)? { "revoked" } else { "no such invite" }),
        "users" => {
            for u in a.users()? {
                println!("{}  since {}{}", u.name, utc_stamp(u.created), if u.disabled { "  DISABLED" } else { "" });
            }
        }
        "disable" | "enable" => {
            let name = arg(0, "a name")?;
            let found = a.set_disabled(&name, command == "disable")?;
            println!("{}", if found { format!("{name} {command}d") } else { format!("no account named {name}") });
        }
        "reset" => {
            let code = a.create_reset(&arg(0, "a name")?)?;
            println!("{code}");
            eprintln!("give this one-use code to the player: Sign Up with their name, a new password and this code (valid 7 days)");
        }
        "password" => {
            let name = arg(0, "a name")?;
            let user = a.user(&name)?.ok_or(format!("no account named {name}"))?;
            // 12 characters of the code alphabet (about 59 bits).
            let password = crate::accounts::random_code(12).to_lowercase();
            let phc = crate::accounts::hash_password(&password, crate::accounts::PasswordCost::SERVER);
            a.set_password(user.id, &phc, "")?;
            println!("{password}");
            eprintln!(
                "temporary password for {}: they log in with it on the launcher's Profile page, then Change password (all their sessions ended)",
                user.name
            );
        }
        "matches" => {
            for m in a.matches(50)? {
                println!("{}  {}  {} vs {}: {} [{}], {} revisions", utc_stamp(m.ended), m.id, m.players[0], m.players[1], m.outcome, m.cause, m.revisions);
            }
        }
        "delete" => {
            let name = arg(0, "a name")?;
            let replay_dir = match rest.get(1).map(String::as_str) {
                None => None,
                Some("--replay-dir") => Some(PathBuf::from(arg(2, "--replay-dir <dir>")?)),
                Some(o) => return Err(format!("unknown delete argument {o}")),
            };
            let Some(d) = a.delete_user(&name)? else {
                return Err(format!("no account named {name}"));
            };
            let mut replays = 0;
            if let Some(dir) = &replay_dir {
                for id in &d.match_ids {
                    match std::fs::remove_file(dir.join(format!("{id}.json"))) {
                        Ok(()) => replays += 1,
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                        Err(e) => return Err(format!("{}: {e}", dir.join(format!("{id}.json")).display())),
                    }
                }
            }
            println!(
                "deleted {}: {} sessions, {} reset codes; {} matches now logged as {}{}",
                d.name,
                d.sessions,
                d.resets,
                d.match_ids.len(),
                crate::accounts::DELETED_NAME,
                if replay_dir.is_some() { format!("; {replays} replays deleted") } else { String::new() }
            );
            eprintln!("a hub that has them signed in right now keeps that connection until it closes; older backups still hold the account");
        }
        "prune-matches" => {
            let days: i64 = arg(0, "a number of days")?.parse().map_err(|_| "prune-matches needs a number of days")?;
            println!("deleted {} match-log rows older than {days} days", a.prune_matches(days)?);
        }
        "backup" => {
            let to = PathBuf::from(arg(0, "a new file")?);
            a.backup(&to)?;
            println!("backed up to {}", to.display());
        }
        other => return Err(format!("unknown accounts command {other}")),
    }
    Ok(())
}

fn tls(args: Vec<String>) -> Result<(), String> {
    let mut args = args.into_iter();
    match args.next().as_deref() {
        Some("self-signed") => {
            let (mut out, mut names) = (None, Vec::new());
            while let Some(a) = args.next() {
                let mut value = || args.next().ok_or(format!("{a} needs a value"));
                match a.as_str() {
                    "--out" => out = Some(PathBuf::from(value()?)),
                    "--name" => names.push(value()?),
                    o => return Err(format!("unknown argument {o}")),
                }
            }
            let out = out.ok_or("--out <dir> is required")?;
            let fp = self_signed(&out, &names)?;
            println!("{fp}");
            eprintln!(
                "wrote {0}/cert.pem and {0}/key.pem (keep key.pem private); friends put \"pin\": \"{fp}\" in their online.json",
                out.display()
            );
            Ok(())
        }
        Some("fingerprint") => {
            let path = PathBuf::from(args.next().ok_or("fingerprint needs <cert.pem>")?);
            println!("{}", file_fingerprint(&path)?);
            Ok(())
        }
        _ => Err("tls: self-signed --out <dir> --name <host> | fingerprint <cert.pem>".into()),
    }
}
