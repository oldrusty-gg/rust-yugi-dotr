//! Email addresses, their verification and password reset by email for the
//! friends hub, on top of `platform_accounts` (a child module of
//! `crate::hub`, so it can use the hub's connection state).
//!
//! Off unless the server is started with a mail sender (`--email-log
//! <file>` today; a provider later): then the welcome says `email`, and the
//! launcher's Profile page offers an address field. With
//! `--require-email` a game connection is let into the lobby only once the
//! account has a verified address (the launcher's account-only connection
//! always signs in, so the player can add one). Sign-up without an address
//! keeps working either way.
//!
//! Privacy: the server log names the account, never the address; codes are
//! stored only as HMACs keyed by `platform-secret.key` beside the database.

use super::{ConnId, Hub, LobbyEvent, LobbyRequest, SignedIn};
use crate::accounts::{self, User};
use platform_accounts::email::{message_for, EmailSender, HttpApiSender, LogSender, Mailer, Provider};
use platform_accounts::store::{AccountState, PlatformStore, Problem};
use roses_net::lobby::{check_password, AccountStatus, EmailProblem, EmailState};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

/// Password-reset hashes (Argon2) running at once; more are told "busy".
pub const MAX_RESET_JOBS: usize = 4;

/// What the operator chose.
#[derive(Clone, Debug)]
pub struct EmailSettings {
    /// The platform's name in messages (placeholder until the brand is set).
    pub brand: String,
    /// Online play needs a verified address.
    pub require_verified: bool,
    /// The base URL of a web page that takes signed links (none: codes
    /// only, typed into the launcher).
    pub web: Option<String>,
}

impl Default for EmailSettings {
    fn default() -> Self {
        Self { brand: "Game Launcher".into(), require_verified: false, web: None }
    }
}

pub struct EmailHub {
    store: PlatformStore,
    mailer: Mailer,
    settings: EmailSettings,
    reset_tx: mpsc::Sender<ResetHashed>,
    reset_rx: mpsc::Receiver<ResetHashed>,
    resets_in_flight: usize,
    last_prune: Option<u64>,
}

struct ResetHashed {
    conn: ConnId,
    user_id: i64,
    name: String,
    phc: String,
}

impl EmailHub {
    pub fn new(store: PlatformStore, sender: Box<dyn EmailSender>, settings: EmailSettings) -> Self {
        let (reset_tx, reset_rx) = mpsc::channel();
        Self { store, mailer: Mailer::start(sender), settings, reset_tx, reset_rx, resets_in_flight: 0, last_prune: None }
    }

    pub fn describe(&self) -> String {
        format!(
            "email by {}; verified address {}",
            self.mailer.describe(),
            if self.settings.require_verified { "required for online play" } else { "optional" }
        )
    }

    pub fn store(&self) -> &PlatformStore {
        &self.store
    }
}

/// The `hub` arguments for email, collected while the other arguments are
/// read; [`EmailArgs::build`] opens everything once the database is known.
#[derive(Default)]
pub struct EmailArgs {
    log: Option<PathBuf>,
    provider: Option<Provider>,
    from: Option<String>,
    settings: EmailSettings,
    any: bool,
}

/// The API key is read from this variable, never from the command line
/// (process lists and shell history keep arguments).
pub const API_KEY_VAR: &str = "PLATFORM_EMAIL_API_KEY";

impl EmailArgs {
    /// Take `flag` if it is an email argument (`value` reads its value).
    pub fn take(&mut self, flag: &str, mut value: impl FnMut() -> Result<String, String>) -> Result<bool, String> {
        match flag {
            "--email-log" => self.log = Some(value()?.into()),
            "--email-provider" => {
                let v = value()?;
                self.provider = Some(Provider::parse(&v).ok_or(format!("--email-provider: {v}: expected postmark or resend"))?);
            }
            "--email-from" => self.from = Some(value()?),
            "--email-web" => self.settings.web = Some(value()?),
            "--brand" => self.settings.brand = value()?,
            "--require-email" => self.settings.require_verified = true,
            _ => return Ok(false),
        }
        self.any = true;
        Ok(true)
    }

    /// The email part of the hub for the account database `db` (none when
    /// no email argument was given).
    pub fn build(self, db: &Path) -> Result<Option<EmailHub>, String> {
        if !self.any {
            return Ok(None);
        }
        let sender: Box<dyn EmailSender> = match (self.log, self.provider) {
            (Some(_), Some(_)) => return Err("use one of --email-log and --email-provider".into()),
            (Some(path), None) => Box::new(LogSender::new(path)),
            (None, Some(provider)) => {
                let key = std::env::var(API_KEY_VAR).map_err(|_| format!("--email-provider needs the API key in {API_KEY_VAR}"))?;
                let from = self.from.ok_or("--email-provider needs --email-from \"Name <no-reply@your.domain>\"")?;
                Box::new(HttpApiSender::new(provider, key, from))
            }
            (None, None) => return Err("--brand, --require-email and --email-web need --email-log <file> or --email-provider".into()),
        };
        let secret_path = db.with_file_name("platform-secret.key");
        let secret = platform_accounts::load_or_create_secret(&secret_path)?;
        let store = PlatformStore::open(db, &secret)?;
        Ok(Some(EmailHub::new(store, sender, self.settings)))
    }
}

fn problem(p: Problem) -> EmailProblem {
    match p {
        Problem::BadEmail => EmailProblem::BadEmail,
        Problem::EmailTaken => EmailProblem::EmailTaken,
        Problem::AlreadyVerified => EmailProblem::AlreadyVerified,
        Problem::TooSoon => EmailProblem::TooSoon,
        Problem::TooManyEmails => EmailProblem::TooManyEmails,
        Problem::BadCode => EmailProblem::BadCode,
        Problem::NoCode => EmailProblem::NoCode,
        Problem::TooManyTries => EmailProblem::TooManyTries,
        Problem::NoVerifiedEmail => EmailProblem::NoVerifiedEmail,
        Problem::Disabled => EmailProblem::Disabled,
    }
}

impl Hub {
    /// Switch email on (see the module notes).
    pub fn with_email(mut self, email: EmailHub) -> Self {
        self.email = Some(email);
        if let Err(e) = self.email_housekeeping() {
            self.log(&format!("platform account tables: {e}"));
        }
        self
    }

    pub fn email(&self) -> Option<&EmailHub> {
        self.email.as_ref()
    }

    fn email_housekeeping(&mut self) -> Result<(), String> {
        let Some(e) = self.email.as_mut() else { return Ok(()) };
        let added = e.store.migrate_legacy_users()?;
        let erased = e.store.prune_legacy_orphans()?;
        let pruned = e.store.prune(platform_accounts::now_seconds())?;
        if added + erased + pruned > 0 {
            self.log(&format!("platform accounts: {added} added (no email yet), {erased} deleted accounts erased, {pruned} old codes dropped"));
        }
        Ok(())
    }

    fn email_failed(&mut self, id: ConnId, p: EmailProblem, now: u64) {
        self.lobby_send(id, LobbyEvent::EmailFailed { problem: p }, now);
    }

    /// The account row as the game server has it, synced into the
    /// platform tables.
    fn platform_user(&mut self, user_id: i64) -> Option<User> {
        let user = match self.accounts.user_by_id(user_id) {
            Ok(u) => u,
            Err(e) => {
                self.log(&format!("account database error: {e}"));
                None
            }
        }?;
        let e = self.email.as_mut()?;
        if let Err(err) = e.store.sync_user(user.id, user.created, &user.name, user.disabled, platform_accounts::now_seconds()) {
            self.log(&format!("platform account tables: {err}"));
            return None;
        }
        Some(user)
    }

    fn account_status(&mut self, user_id: i64) -> Option<AccountStatus> {
        let user = self.platform_user(user_id)?;
        let e = self.email.as_ref()?;
        let p = match e.store.profile(user.id, user.disabled, platform_accounts::now_seconds()) {
            Ok(p) => p,
            Err(err) => {
                self.log(&format!("platform account tables: {err}"));
                return None;
            }
        };
        Some(AccountStatus {
            state: match p.state {
                AccountState::NoEmail => EmailState::NoEmail,
                AccountState::Unverified => EmailState::Unverified,
                AccountState::Verified => EmailState::Verified,
                AccountState::Disabled => EmailState::Disabled,
            },
            email: p.email,
            pending_email: p.pending_email,
            prompt_email: p.prompt_email,
            verification_required: e.settings.require_verified,
            supporter: p.supporter_flags,
        })
    }

    fn send_status(&mut self, id: ConnId, user_id: i64, now: u64) {
        match self.account_status(user_id) {
            Some(status) => self.lobby_send(id, LobbyEvent::AccountStatus { status }, now),
            None => self.email_failed(id, EmailProblem::Busy, now),
        }
    }

    /// Whether `user` may enter the lobby (a verified address when the
    /// server requires one).
    pub(super) fn email_allows_lobby(&mut self, user: &User, _now: u64) -> bool {
        if !self.email.as_ref().is_some_and(|e| e.settings.require_verified) {
            return true;
        }
        self.account_status(user.id).is_some_and(|s| s.state == EmailState::Verified)
    }

    /// The account for a reset request: a name, or a verified address.
    fn reset_account(&mut self, account: &str) -> Option<User> {
        let account = account.trim();
        let by_name = if account.contains('@') {
            let id = self.email.as_ref()?.store.user_for_email(account).ok().flatten()?;
            self.accounts.user_by_id(id).ok().flatten()
        } else {
            self.accounts.user(account).ok().flatten()
        }?;
        self.platform_user(by_name.id)
    }

    pub(super) fn email_request(&mut self, id: ConnId, user: Option<SignedIn>, request: LobbyRequest, now: u64) {
        if self.email.is_none() {
            return self.email_failed(id, EmailProblem::NotOffered, now);
        }
        let peer = self.conns.get(&id).and_then(|c| c.peer);
        let wall = platform_accounts::now_seconds();
        match request {
            LobbyRequest::AccountStatus => match user {
                Some(u) => self.send_status(id, u.id, now),
                None => self.email_failed(id, EmailProblem::NotSignedIn, now),
            },
            LobbyRequest::DismissEmailPrompt => {
                let Some(u) = user else { return self.email_failed(id, EmailProblem::NotSignedIn, now) };
                if let Some(e) = self.email.as_ref() {
                    let _ = e.store.dismiss_email_prompt(u.id, wall);
                }
                self.send_status(id, u.id, now);
            }
            LobbyRequest::SetEmail { email } => {
                let Some(u) = user else { return self.email_failed(id, EmailProblem::NotSignedIn, now) };
                if self.platform_user(u.id).is_none() {
                    return self.email_failed(id, EmailProblem::Busy, now);
                }
                let e = self.email.as_ref().expect("checked");
                let issued = match e.store.start_email_verification(u.id, &email, wall) {
                    Ok(Ok(issued)) => issued,
                    Ok(Err(p)) => return self.email_failed(id, problem(p), now),
                    Err(err) => {
                        self.log(&format!("platform account tables: {err}"));
                        return self.email_failed(id, EmailProblem::Busy, now);
                    }
                };
                let message = message_for(&issued, &e.settings.brand, &u.name, e.settings.web.as_deref());
                if !e.mailer.queue(message) {
                    return self.email_failed(id, EmailProblem::Busy, now);
                }
                self.log(&format!("{} asked to verify an email address", u.name));
                self.lobby_send(id, LobbyEvent::EmailCodeSent, now);
                self.send_status(id, u.id, now);
            }
            LobbyRequest::VerifyEmail { code } => {
                let Some(u) = user else { return self.email_failed(id, EmailProblem::NotSignedIn, now) };
                if !self.attempts.allowed(peer, Some(&u.name), now) {
                    return self.email_failed(id, EmailProblem::TooManyTries, now);
                }
                let result = self.email.as_mut().expect("checked").store.verify_email(u.id, &code, wall);
                match result {
                    Ok(Ok(_)) => {
                        self.log(&format!("{} verified an email address", u.name));
                        self.send_status(id, u.id, now);
                    }
                    Ok(Err(p)) => {
                        if p == Problem::BadCode {
                            self.attempts.failed(peer, Some(&u.name), now);
                        }
                        self.email_failed(id, problem(p), now);
                    }
                    Err(err) => {
                        self.log(&format!("platform account tables: {err}"));
                        self.email_failed(id, EmailProblem::Busy, now);
                    }
                }
            }
            LobbyRequest::RequestPasswordReset { account } => {
                if !self.attempts.allowed(peer, None, now) {
                    return self.email_failed(id, EmailProblem::TooSoon, now);
                }
                // Counted like a failed sign-in: requests are limited per
                // address whether or not the account exists.
                self.attempts.failed(peer, None, now);
                if let Some(target) = self.reset_account(&account).filter(|u| !u.disabled) {
                    let e = self.email.as_ref().expect("checked");
                    match e.store.start_password_reset(target.id, wall) {
                        Ok(Ok(issued)) => {
                            let message = message_for(&issued, &e.settings.brand, &target.name, e.settings.web.as_deref());
                            let queued = e.mailer.queue(message);
                            self.log(&format!("password reset asked for {}{}", target.name, if queued { "" } else { " (mail queue full)" }));
                        }
                        Ok(Err(p)) => self.log(&format!("password reset for {} not sent: {p}", target.name)),
                        Err(err) => self.log(&format!("platform account tables: {err}")),
                    }
                }
                // The same answer either way: nobody learns who has an address.
                self.lobby_send(id, LobbyEvent::ResetCodeSent, now);
            }
            LobbyRequest::ResetPassword { account, code, new_password } => {
                if self.authing(id) {
                    return self.email_failed(id, EmailProblem::Busy, now);
                }
                if check_password(&new_password).is_err() {
                    return self.email_failed(id, EmailProblem::WeakPassword, now);
                }
                let name_key = account.trim().to_ascii_lowercase();
                if !self.attempts.allowed(peer, Some(&name_key), now) {
                    return self.email_failed(id, EmailProblem::TooManyTries, now);
                }
                let Some(target) = self.reset_account(&account) else {
                    self.attempts.failed(peer, Some(&name_key), now);
                    return self.email_failed(id, EmailProblem::BadCode, now);
                };
                let checked = self.email.as_ref().expect("checked").store.check_password_reset(target.id, &code, wall);
                match checked {
                    Ok(Ok(_)) => {}
                    Ok(Err(p)) => {
                        self.attempts.failed(peer, Some(&name_key), now);
                        return self.email_failed(id, problem(p), now);
                    }
                    Err(err) => {
                        self.log(&format!("platform account tables: {err}"));
                        return self.email_failed(id, EmailProblem::Busy, now);
                    }
                }
                let e = self.email.as_mut().expect("checked");
                if e.resets_in_flight >= MAX_RESET_JOBS {
                    return self.email_failed(id, EmailProblem::Busy, now);
                }
                e.resets_in_flight += 1;
                let (tx, cost) = (e.reset_tx.clone(), self.settings.password_cost);
                let (user_id, name) = (target.id, target.name.clone());
                std::thread::spawn(move || {
                    let phc = accounts::hash_password(&new_password, cost);
                    let _ = tx.send(ResetHashed { conn: id, user_id, name, phc });
                });
                self.set_authing(id, true);
            }
            _ => {}
        }
    }

    /// Finish password resets, log the mailer's outcomes, prune hourly.
    pub(super) fn email_step(&mut self, now: u64) -> bool {
        let Some(e) = self.email.as_mut() else { return false };
        let done: Vec<ResetHashed> = e.reset_rx.try_iter().collect();
        let notes = e.mailer.notes();
        let prune = e.last_prune.is_none_or(|t| now.saturating_sub(t) > 3_600_000);
        if prune {
            e.last_prune = Some(now);
        }
        e.resets_in_flight = e.resets_in_flight.saturating_sub(done.len());
        let busy = !done.is_empty() || !notes.is_empty();
        for note in notes {
            self.log(&note);
        }
        for d in done {
            self.set_authing(d.conn, false);
            // "": no session is kept, every sign-in of the account ends.
            let stored = self.accounts.set_password(d.user_id, &d.phc, "").and_then(|()| {
                self.email.as_ref().expect("on").store.finish_password_reset(d.user_id, platform_accounts::now_seconds())
            });
            match stored {
                Ok(()) => {
                    self.log(&format!("{} reset their password by email", d.name));
                    self.lobby_send(d.conn, LobbyEvent::PasswordReset, now);
                }
                Err(err) => {
                    self.log(&format!("account database error: {err}"));
                    self.email_failed(d.conn, EmailProblem::Busy, now);
                }
            }
        }
        if prune {
            if let Err(err) = self.email_housekeeping() {
                self.log(&format!("platform account tables: {err}"));
            }
        }
        busy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::{Accounts, PasswordCost};
    use crate::hub::{HubAcceptor, HubConn, HubData, HubSettings};
    use crate::serve::Limits;
    use platform_accounts::email::LogSender;
    use roses_net::friends::{LobbyClient, LobbyConn};
    use roses_net::lobby::{AuthProblem, ClientFrame, ServerFrame};
    use roses_net::transport::memory_pair;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    struct Running {
        tx: mpsc::Sender<HubConn>,
        stop: Arc<AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
        mail: PathBuf,
        dir: PathBuf,
    }

    impl Running {
        /// A hub with a file database and a mail log; `email`: switched on.
        fn start(email: bool, require: bool) -> Self {
            let dir = std::env::temp_dir().join(format!("roses-email-hub-{:016x}", crate::random_u64()));
            std::fs::create_dir_all(&dir).unwrap();
            let db = dir.join("accounts.db");
            let mail = dir.join("mail.jsonl");
            let accounts = Accounts::open(&db).unwrap();
            let (tx, rx) = mpsc::channel::<HubConn>();
            let stop = Arc::new(AtomicBool::new(false));
            let settings = HubSettings {
                name: "Email test".into(),
                limits: Limits::UNLIMITED,
                password_cost: PasswordCost::TEST,
                open_signup: true,
                ..HubSettings::default()
            };
            let mut args = EmailArgs::default();
            if email {
                let log = mail.to_string_lossy().into_owned();
                assert!(args.take("--email-log", || Ok(log.clone())).unwrap());
                assert!(args.take("--brand", || Ok("TestBrand".into())).unwrap());
                if require {
                    assert!(args.take("--require-email", || unreachable!()).unwrap());
                }
            }
            assert!(!args.take("--unrelated", || unreachable!()).unwrap());
            let email = args.build(&db).unwrap();
            assert_eq!(email.is_some(), email_on(email.as_ref()));
            let flag = stop.clone();
            let thread = std::thread::spawn(move || {
                let mut hub = Hub::new(settings, HubData::synthetic(false), accounts, |line| eprintln!("hub: {line}"));
                if let Some(e) = email {
                    hub = hub.with_email(e);
                }
                let mut acceptors: Vec<Box<dyn HubAcceptor + Send>> = vec![Box::new(rx)];
                hub.run(&mut acceptors, &flag).unwrap();
            });
            Self { tx, stop, thread: Some(thread), mail, dir }
        }

        fn account(&self) -> Client {
            let (client, server) = memory_pair::<LobbyRequest, LobbyEvent>();
            self.tx.send(Box::new(server.retype::<ServerFrame, ClientFrame>())).unwrap();
            let c = LobbyClient::start_account(Box::new(client) as LobbyConn, "launcher test").unwrap();
            let mut c = Client { c, q: Default::default() };
            wait(&mut c, |e| matches!(e, LobbyEvent::LobbyWelcome { .. }));
            c
        }

        fn game(&self) -> Client {
            let (client, server) = memory_pair::<LobbyRequest, LobbyEvent>();
            self.tx.send(Box::new(server.retype::<ServerFrame, ClientFrame>())).unwrap();
            let compat = roses_net::Compat::current(roses_net::data_fingerprint(
                &roses_net::synthetic::catalogue(),
                &roses_net::RulesTables::empty(),
            ));
            let c = LobbyClient::start(Box::new(client) as LobbyConn, compat, "game test").unwrap();
            let mut c = Client { c, q: Default::default() };
            wait(&mut c, |e| matches!(e, LobbyEvent::LobbyWelcome { .. }));
            c
        }

        /// The newest mail's code (`XXXX-XXXX`), waiting for it to arrive.
        fn code(&self, count: usize) -> (String, String) {
            let start = Instant::now();
            loop {
                let mails = LogSender::read(&self.mail);
                if mails.len() >= count {
                    let m = mails.last().unwrap();
                    let code = m.text.lines().map(str::trim).find(|l| l.len() == 9 && l.as_bytes()[4] == b'-').unwrap().to_string();
                    return (m.to.clone(), code);
                }
                assert!(start.elapsed() < Duration::from_secs(10), "no mail {count}");
                std::thread::sleep(Duration::from_millis(5));
            }
        }

        fn stop(mut self) {
            self.stop.store(true, Ordering::Relaxed);
            self.thread.take().unwrap().join().unwrap();
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn email_on(e: Option<&EmailHub>) -> bool {
        e.is_some_and(|e| e.describe().contains("mail log file"))
    }

    /// A client and the events it received that no test step took yet.
    struct Client {
        c: LobbyClient,
        q: std::collections::VecDeque<LobbyEvent>,
    }

    impl std::ops::Deref for Client {
        type Target = LobbyClient;
        fn deref(&self) -> &LobbyClient {
            &self.c
        }
    }

    /// Pump until an event matches (oldest first); returns it.
    fn wait(c: &mut Client, mut matching: impl FnMut(&LobbyEvent) -> bool) -> LobbyEvent {
        let start = Instant::now();
        loop {
            if let Some(i) = c.q.iter().position(&mut matching) {
                return c.q.remove(i).expect("present");
            }
            let events = c.c.pump();
            c.q.extend(events);
            assert!(start.elapsed() < Duration::from_secs(10), "timed out (note {:?})", c.c.note);
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn reply(c: &mut Client, request: LobbyRequest) -> LobbyEvent {
        c.c.send(request);
        wait(c, |e| {
            matches!(
                e,
                LobbyEvent::AccountStatus { .. }
                    | LobbyEvent::EmailFailed { .. }
                    | LobbyEvent::EmailCodeSent
                    | LobbyEvent::ResetCodeSent
                    | LobbyEvent::PasswordReset
                    | LobbyEvent::SignedIn { .. }
                    | LobbyEvent::AuthFailed { .. }
            )
        })
    }

    fn status(e: LobbyEvent) -> AccountStatus {
        match e {
            LobbyEvent::AccountStatus { status } => status,
            other => panic!("expected a status, got {other:?}"),
        }
    }

    fn sign_up(c: &mut Client, name: &str, password: &str) {
        let r = reply(c, LobbyRequest::SignUp { username: name.into(), password: password.into(), invite: String::new() });
        assert!(matches!(r, LobbyEvent::SignedIn { .. }), "{r:?}");
    }

    #[test]
    fn an_address_is_verified_and_resets_the_password() {
        let hub = Running::start(true, false);
        let mut yugi = hub.account();
        assert!(yugi.email, "the welcome offers email");
        sign_up(&mut yugi, "Yugi", "password1");
        let s = status(reply(&mut yugi, LobbyRequest::AccountStatus));
        assert_eq!((s.state, s.prompt_email, s.verification_required), (EmailState::NoEmail, true, false));
        assert_eq!(reply(&mut yugi, LobbyRequest::SetEmail { email: "not an address".into() }), LobbyEvent::EmailFailed { problem: EmailProblem::BadEmail });
        assert_eq!(reply(&mut yugi, LobbyRequest::SetEmail { email: "yugi@example.com".into() }), LobbyEvent::EmailCodeSent);
        let s = status(wait(&mut yugi, |e| matches!(e, LobbyEvent::AccountStatus { .. })));
        assert_eq!((s.state, s.pending_email.as_deref()), (EmailState::Unverified, Some("yugi@example.com")));
        let (to, code) = hub.code(1);
        assert_eq!(to, "yugi@example.com");
        assert_eq!(reply(&mut yugi, LobbyRequest::VerifyEmail { code: "ZZZZ-ZZZZ".into() }), LobbyEvent::EmailFailed { problem: EmailProblem::BadCode });
        let s = status(reply(&mut yugi, LobbyRequest::VerifyEmail { code: code.to_lowercase() }));
        assert_eq!((s.state, s.email.as_deref(), s.prompt_email), (EmailState::Verified, Some("yugi@example.com"), false));
        // Another account cannot take the address.
        let mut joey = hub.account();
        sign_up(&mut joey, "Joey", "password2");
        assert_eq!(reply(&mut joey, LobbyRequest::SetEmail { email: "YUGI@example.com".into() }), LobbyEvent::EmailFailed { problem: EmailProblem::EmailTaken });

        // Signed out: reset by name; the answer is the same for nobody.
        let mut out = hub.account();
        assert_eq!(reply(&mut out, LobbyRequest::RequestPasswordReset { account: "nobody".into() }), LobbyEvent::ResetCodeSent);
        assert_eq!(reply(&mut out, LobbyRequest::RequestPasswordReset { account: "Joey".into() }), LobbyEvent::ResetCodeSent, "no address: nothing sent, same answer");
        assert_eq!(reply(&mut out, LobbyRequest::RequestPasswordReset { account: "yugi@example.com".into() }), LobbyEvent::ResetCodeSent);
        let (to, reset) = hub.code(2);
        assert_eq!(to, "yugi@example.com");
        assert_eq!(LogSender::read(&hub.mail).len(), 2, "Joey and nobody got no mail");
        let bad = LobbyRequest::ResetPassword { account: "yugi".into(), code: "WRONGCODE".into(), new_password: "newpassword".into() };
        assert_eq!(reply(&mut out, bad), LobbyEvent::EmailFailed { problem: EmailProblem::BadCode });
        let weak = LobbyRequest::ResetPassword { account: "yugi".into(), code: reset.clone(), new_password: "short".into() };
        assert_eq!(reply(&mut out, weak), LobbyEvent::EmailFailed { problem: EmailProblem::WeakPassword });
        let good = LobbyRequest::ResetPassword { account: "yugi".into(), code: reset.clone(), new_password: "newpassword".into() };
        assert_eq!(reply(&mut out, good.clone()), LobbyEvent::PasswordReset);
        assert_eq!(reply(&mut out, good), LobbyEvent::EmailFailed { problem: EmailProblem::NoCode }, "a code works once");
        // The old sign-in ended; the new password works, the old does not.
        let mut again = hub.account();
        assert_eq!(reply(&mut again, LobbyRequest::LogIn { username: "Yugi".into(), password: "password1".into() }), LobbyEvent::AuthFailed { problem: AuthProblem::BadCredentials });
        assert!(matches!(reply(&mut again, LobbyRequest::LogIn { username: "Yugi".into(), password: "newpassword".into() }), LobbyEvent::SignedIn { .. }));
        let session = yugi.session.clone().unwrap();
        let mut old = hub.account();
        assert_eq!(reply(&mut old, LobbyRequest::ResumeSession { session }), LobbyEvent::AuthFailed { problem: AuthProblem::SessionExpired });
        hub.stop();
    }

    #[test]
    fn a_required_address_keeps_the_game_out_of_the_lobby_until_verified() {
        let hub = Running::start(true, true);
        let mut launcher = hub.account();
        sign_up(&mut launcher, "Mai", "password1");
        let s = status(reply(&mut launcher, LobbyRequest::AccountStatus));
        assert!(s.verification_required);
        let mut game = hub.game();
        let login = LobbyRequest::LogIn { username: "Mai".into(), password: "password1".into() };
        assert_eq!(reply(&mut game, login.clone()), LobbyEvent::AuthFailed { problem: AuthProblem::EmailNotVerified });
        assert_eq!(reply(&mut launcher, LobbyRequest::SetEmail { email: "mai@example.com".into() }), LobbyEvent::EmailCodeSent);
        assert_eq!(status(wait(&mut launcher, |e| matches!(e, LobbyEvent::AccountStatus { .. }))).state, EmailState::Unverified);
        let (_, code) = hub.code(1);
        assert_eq!(status(reply(&mut launcher, LobbyRequest::VerifyEmail { code })).state, EmailState::Verified);
        let mut game = hub.game();
        assert!(matches!(reply(&mut game, login), LobbyEvent::SignedIn { .. }));
        hub.stop();
    }

    #[test]
    fn without_email_nothing_changes() {
        let hub = Running::start(false, false);
        let mut c = hub.account();
        assert!(!c.email);
        sign_up(&mut c, "Tea", "password1");
        assert_eq!(reply(&mut c, LobbyRequest::AccountStatus), LobbyEvent::EmailFailed { problem: EmailProblem::NotOffered });
        // The game still plays without an address.
        let mut game = hub.game();
        assert!(matches!(reply(&mut game, LobbyRequest::LogIn { username: "Tea".into(), password: "password1".into() }), LobbyEvent::SignedIn { .. }));
        // An older server's welcome (no `email`) reads as "no email".
        let old: LobbyEvent = serde_json::from_str(r#"{"type":"lobby_welcome","lobby_version":1,"server":"s","computer":false}"#).unwrap();
        assert!(matches!(old, LobbyEvent::LobbyWelcome { email: false, .. }));
        hub.stop();
    }

    #[test]
    fn email_arguments_are_checked() {
        let mut args = EmailArgs::default();
        assert!(args.take("--require-email", || unreachable!()).unwrap());
        assert!(args.build(Path::new("x.db")).err().unwrap().contains("--email-log"));
        let mut args = EmailArgs::default();
        assert!(args.take("--email-provider", || Ok("carrier-pigeon".into())).is_err());
        assert!(EmailArgs::default().build(Path::new("x.db")).unwrap().is_none());
    }
}
