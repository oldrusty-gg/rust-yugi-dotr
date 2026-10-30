//! Friends-online accounts in one SQLite file (rusqlite, bundled SQLite):
//! users with Argon2id password hashes (PHC strings), invite codes made by
//! the server's owner, session tokens (only their SHA-256 is stored) and a
//! small match log. SQLite rather than a hand-written file store: atomic
//! transactions (an invite is consumed and its account created together),
//! crash safety (WAL journal), and a consistent copy of a live database
//! with `VACUUM INTO` ([`Accounts::backup`]), all in one file that is easy
//! to copy off the server.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use rusqlite::{params, Connection, OptionalExtension};
use roses_net::lobby::{normalize_code, username_key, AuthProblem, CODE_ALPHABET};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Sessions last this long after their last use (seconds).
pub const SESSION_SECONDS: i64 = 30 * 24 * 3600;
const SCHEMA_VERSION: i64 = 1;

/// Argon2id cost: OWASP's minimum (19 MiB, 2 passes, 1 lane) for the
/// server; tests use a tiny cost. The PHC string records the parameters,
/// so changing them later still verifies old hashes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PasswordCost {
    pub memory_kib: u32,
    pub passes: u32,
}

impl PasswordCost {
    pub const SERVER: Self = Self { memory_kib: 19 * 1024, passes: 2 };
    pub const TEST: Self = Self { memory_kib: 64, passes: 1 };
}

fn argon2(cost: PasswordCost) -> Argon2<'static> {
    let params = Params::new(cost.memory_kib, cost.passes, 1, None).expect("valid Argon2 parameters");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

/// An Argon2id PHC string of `password` with a fresh random salt.
pub fn hash_password(password: &str, cost: PasswordCost) -> String {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).expect("the operating system's random source failed");
    let salt = SaltString::encode_b64(&bytes).expect("16 salt bytes encode");
    argon2(cost)
        .hash_password(password.as_bytes(), &salt)
        .expect("Argon2 hashing with valid parameters")
        .to_string()
}

/// Whether `password` matches the PHC string (its own parameters).
pub fn verify_password(password: &str, phc: &str) -> bool {
    match PasswordHash::new(phc) {
        Ok(hash) => Argon2::default().verify_password(password.as_bytes(), &hash).is_ok(),
        Err(_) => false,
    }
}

/// A hash to verify against when the user does not exist, so a missing
/// name costs as much time as a wrong password.
pub fn dummy_hash(cost: PasswordCost) -> String {
    hash_password("not-a-real-password", cost)
}

pub fn now_seconds() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

fn token_hash(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

/// A random code of `len` characters from [`CODE_ALPHABET`] (OS CSPRNG,
/// rejection sampling so every character is equally likely).
pub fn random_code(len: usize) -> String {
    let n = CODE_ALPHABET.len() as u8;
    let limit = 256 - (256 % n as u16) as u16;
    let mut out = String::with_capacity(len);
    while out.len() < len {
        let mut bytes = [0u8; 16];
        getrandom::getrandom(&mut bytes).expect("the operating system's random source failed");
        for b in bytes {
            if (b as u16) < limit && out.len() < len {
                out.push(char::from(CODE_ALPHABET[usize::from(b % n)]));
            }
        }
    }
    out
}

/// An invite code as shown: `XXXX-XXXX`.
pub fn new_invite_code() -> String {
    let c = random_code(8);
    format!("{}-{}", &c[..4], &c[4..])
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct User {
    pub id: i64,
    pub name: String,
    pub phc: String,
    pub disabled: bool,
    pub created: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invite {
    /// Normalised (no dash).
    pub code: String,
    pub uses_left: i64,
    /// A password-reset invite for this account.
    pub for_user: Option<String>,
    pub note: Option<String>,
    pub expires: Option<i64>,
}

/// One match in the log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchRow {
    pub id: String,
    pub players: [String; 2],
    pub started: i64,
    pub ended: i64,
    pub outcome: String,
    pub cause: String,
    pub revisions: i64,
}

/// The name a deleted account has in the match log (not a valid username,
/// so it can never belong to a new account).
pub const DELETED_NAME: &str = "(deleted)";

/// What [`Accounts::delete_user`] removed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deleted {
    /// The account's stored name.
    pub name: String,
    pub sessions: usize,
    /// Unused password-reset codes for the account.
    pub resets: usize,
    /// Matches the account played (now logged with [`DELETED_NAME`]);
    /// their replay files are `<id>.json` in the hub's `--replay-dir`.
    pub match_ids: Vec<String>,
}

pub struct Accounts {
    db: Connection,
}

impl Accounts {
    /// Open (creating when missing) the account database.
    pub fn open(path: &Path) -> Result<Self, String> {
        let db = Connection::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::init(db)
    }

    /// A database in memory (tests).
    pub fn in_memory() -> Result<Self, String> {
        Self::init(Connection::open_in_memory().map_err(|e| e.to_string())?)
    }

    fn init(db: Connection) -> Result<Self, String> {
        let e = |e: rusqlite::Error| e.to_string();
        db.pragma_update(None, "foreign_keys", "ON").map_err(e)?;
        // WAL: readers do not block the server, a crash loses nothing
        // committed (memory databases ignore it).
        let _ = db.pragma_update(None, "journal_mode", "WAL");
        db.pragma_update(None, "synchronous", "NORMAL").map_err(e)?;
        let version: i64 = db.pragma_query_value(None, "user_version", |r| r.get(0)).map_err(e)?;
        if version > SCHEMA_VERSION {
            return Err(format!("the account database is from a newer server (schema {version})"));
        }
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS users(
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                name_key TEXT NOT NULL UNIQUE,
                phc TEXT NOT NULL,
                created INTEGER NOT NULL,
                disabled INTEGER NOT NULL DEFAULT 0,
                invite TEXT);
             CREATE TABLE IF NOT EXISTS invites(
                code TEXT PRIMARY KEY,
                uses_left INTEGER NOT NULL,
                for_user TEXT,
                note TEXT,
                created INTEGER NOT NULL,
                expires INTEGER);
             CREATE TABLE IF NOT EXISTS sessions(
                token_hash TEXT PRIMARY KEY,
                user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                created INTEGER NOT NULL,
                expires INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS matches(
                id TEXT PRIMARY KEY,
                player0 TEXT NOT NULL,
                player1 TEXT NOT NULL,
                started INTEGER NOT NULL,
                ended INTEGER NOT NULL,
                outcome TEXT NOT NULL,
                cause TEXT NOT NULL,
                revisions INTEGER NOT NULL);",
        )
        .map_err(e)?;
        db.pragma_update(None, "user_version", SCHEMA_VERSION).map_err(e)?;
        Ok(Self { db })
    }

    // ---- invites ------------------------------------------------------

    /// Make an invite usable `uses` times (expiring after `days`, if given).
    pub fn create_invite(&self, uses: i64, note: Option<&str>, days: Option<i64>) -> Result<String, String> {
        let shown = new_invite_code();
        let expires = days.map(|d| now_seconds() + d * 24 * 3600);
        self.db
            .execute(
                "INSERT INTO invites(code, uses_left, for_user, note, created, expires) VALUES (?1, ?2, NULL, ?3, ?4, ?5)",
                params![normalize_code(&shown), uses.max(1), note, now_seconds(), expires],
            )
            .map_err(|e| e.to_string())?;
        Ok(shown)
    }

    /// A one-use code that sets a new password for `name` at sign-up
    /// (valid 7 days).
    pub fn create_reset(&self, name: &str) -> Result<String, String> {
        if self.user(name)?.is_none() {
            return Err(format!("no account named {name}"));
        }
        let shown = new_invite_code();
        self.db
            .execute(
                "INSERT INTO invites(code, uses_left, for_user, note, created, expires) VALUES (?1, 1, ?2, 'password reset', ?3, ?4)",
                params![normalize_code(&shown), username_key(name), now_seconds(), now_seconds() + 7 * 24 * 3600],
            )
            .map_err(|e| e.to_string())?;
        Ok(shown)
    }

    pub fn invites(&self) -> Result<Vec<Invite>, String> {
        let mut st = self
            .db
            .prepare("SELECT code, uses_left, for_user, note, expires FROM invites ORDER BY created")
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map([], |r| {
                Ok(Invite { code: r.get(0)?, uses_left: r.get(1)?, for_user: r.get(2)?, note: r.get(3)?, expires: r.get(4)? })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
    }

    pub fn revoke_invite(&self, code: &str) -> Result<bool, String> {
        self.db
            .execute("DELETE FROM invites WHERE code = ?1", params![normalize_code(code)])
            .map(|n| n > 0)
            .map_err(|e| e.to_string())
    }

    /// The invite `code` if it can be used for `username` now.
    pub fn usable_invite(&self, code: &str, username: &str) -> Result<Option<Invite>, String> {
        let invite = self
            .db
            .query_row(
                "SELECT code, uses_left, for_user, note, expires FROM invites WHERE code = ?1",
                params![normalize_code(code)],
                |r| Ok(Invite { code: r.get(0)?, uses_left: r.get(1)?, for_user: r.get(2)?, note: r.get(3)?, expires: r.get(4)? }),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        Ok(invite.filter(|i| {
            i.uses_left > 0
                && i.expires.is_none_or(|t| t > now_seconds())
                && i.for_user.as_ref().is_none_or(|u| *u == username_key(username))
        }))
    }

    // ---- users --------------------------------------------------------

    pub fn user(&self, name: &str) -> Result<Option<User>, String> {
        self.db
            .query_row(
                "SELECT id, name, phc, disabled, created FROM users WHERE name_key = ?1",
                params![username_key(name)],
                |r| Ok(User { id: r.get(0)?, name: r.get(1)?, phc: r.get(2)?, disabled: r.get::<_, i64>(3)? != 0, created: r.get(4)? }),
            )
            .optional()
            .map_err(|e| e.to_string())
    }

    pub fn users(&self) -> Result<Vec<User>, String> {
        let mut st = self
            .db
            .prepare("SELECT id, name, phc, disabled, created FROM users ORDER BY name_key")
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map([], |r| {
                Ok(User { id: r.get(0)?, name: r.get(1)?, phc: r.get(2)?, disabled: r.get::<_, i64>(3)? != 0, created: r.get(4)? })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
    }

    /// Create `username` with the hash `phc`, consuming one use of the
    /// invite, in one transaction. A reset invite for an existing account
    /// replaces its password and ends its sessions instead. Returns the
    /// account's stored name.
    pub fn sign_up(&mut self, username: &str, phc: &str, invite: &str) -> Result<Result<String, AuthProblem>, String> {
        let Some(inv) = self.usable_invite(invite, username)? else {
            return Ok(Err(AuthProblem::BadInvite));
        };
        let existing = self.user(username)?;
        let tx = self.db.transaction().map_err(|e| e.to_string())?;
        let name = match (&inv.for_user, existing) {
            (Some(_), Some(user)) => {
                tx.execute("UPDATE users SET phc = ?1 WHERE id = ?2", params![phc, user.id]).map_err(|e| e.to_string())?;
                tx.execute("DELETE FROM sessions WHERE user_id = ?1", params![user.id]).map_err(|e| e.to_string())?;
                user.name
            }
            (Some(_), None) => return Ok(Err(AuthProblem::BadInvite)),
            (None, Some(_)) => return Ok(Err(AuthProblem::UsernameTaken)),
            (None, None) => {
                tx.execute(
                    "INSERT INTO users(name, name_key, phc, created, invite) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![username, username_key(username), phc, now_seconds(), inv.code],
                )
                .map_err(|e| e.to_string())?;
                username.to_string()
            }
        };
        tx.execute("UPDATE invites SET uses_left = uses_left - 1 WHERE code = ?1", params![inv.code])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM invites WHERE uses_left <= 0", []).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(Ok(name))
    }

    /// Create `username` with the hash `phc` without an invite (open
    /// sign-up). Returns the account's stored name.
    pub fn sign_up_open(&mut self, username: &str, phc: &str) -> Result<Result<String, AuthProblem>, String> {
        let inserted = self.db.execute(
            "INSERT INTO users(name, name_key, phc, created, invite) VALUES (?1, ?2, ?3, ?4, NULL)",
            params![username, username_key(username), phc, now_seconds()],
        );
        match inserted {
            Ok(_) => Ok(Ok(username.to_string())),
            Err(rusqlite::Error::SqliteFailure(e, _)) if e.code == rusqlite::ErrorCode::ConstraintViolation => {
                Ok(Err(AuthProblem::UsernameTaken))
            }
            Err(e) => Err(e.to_string()),
        }
    }

    /// Replace the password hash of `user_id` and end its other sessions
    /// (`keep`: the session that asked stays signed in).
    pub fn set_password(&mut self, user_id: i64, phc: &str, keep: &str) -> Result<(), String> {
        let tx = self.db.transaction().map_err(|e| e.to_string())?;
        tx.execute("UPDATE users SET phc = ?1 WHERE id = ?2", params![phc, user_id]).map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM sessions WHERE user_id = ?1 AND token_hash <> ?2", params![user_id, token_hash(keep)])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }

    /// The account with this id.
    pub fn user_by_id(&self, id: i64) -> Result<Option<User>, String> {
        self.db
            .query_row("SELECT id, name, phc, disabled, created FROM users WHERE id = ?1", params![id], |r| {
                Ok(User { id: r.get(0)?, name: r.get(1)?, phc: r.get(2)?, disabled: r.get::<_, i64>(3)? != 0, created: r.get(4)? })
            })
            .optional()
            .map_err(|e| e.to_string())
    }

    pub fn set_disabled(&self, name: &str, disabled: bool) -> Result<bool, String> {
        let n = self
            .db
            .execute("UPDATE users SET disabled = ?1 WHERE name_key = ?2", params![i64::from(disabled), username_key(name)])
            .map_err(|e| e.to_string())?;
        if disabled {
            self.db
                .execute("DELETE FROM sessions WHERE user_id IN (SELECT id FROM users WHERE name_key = ?1)", params![username_key(name)])
                .map_err(|e| e.to_string())?;
        }
        Ok(n > 0)
    }

    /// Erase the account `name` (a deletion request): the user row, all its
    /// sessions and password-reset codes, in one transaction. Its name in
    /// the match log is replaced by [`DELETED_NAME`] (the other player's
    /// history stays). Returns what was removed, or `None` when there is no
    /// such account. The name can be signed up again afterwards.
    pub fn delete_user(&mut self, name: &str) -> Result<Option<Deleted>, String> {
        let Some(user) = self.user(name)? else { return Ok(None) };
        let key = username_key(name);
        let e = |e: rusqlite::Error| e.to_string();
        let tx = self.db.transaction().map_err(e)?;
        let match_ids: Vec<String> = {
            let mut st = tx
                .prepare("SELECT id FROM matches WHERE lower(player0) = ?1 OR lower(player1) = ?1 ORDER BY ended")
                .map_err(e)?;
            let rows = st.query_map(params![key], |r| r.get(0)).map_err(e)?;
            rows.collect::<Result<_, _>>().map_err(e)?
        };
        let sessions = tx.execute("DELETE FROM sessions WHERE user_id = ?1", params![user.id]).map_err(e)?;
        let resets = tx.execute("DELETE FROM invites WHERE for_user = ?1", params![key]).map_err(e)?;
        tx.execute("UPDATE matches SET player0 = ?1 WHERE lower(player0) = ?2", params![DELETED_NAME, key]).map_err(e)?;
        tx.execute("UPDATE matches SET player1 = ?1 WHERE lower(player1) = ?2", params![DELETED_NAME, key]).map_err(e)?;
        tx.execute("DELETE FROM users WHERE id = ?1", params![user.id]).map_err(e)?;
        tx.commit().map_err(e)?;
        Ok(Some(Deleted { name: user.name, sessions, resets, match_ids }))
    }

    /// Delete match-log rows that ended more than `days` days ago (match
    /// log retention). Returns how many were deleted.
    pub fn prune_matches(&self, days: i64) -> Result<usize, String> {
        self.db
            .execute("DELETE FROM matches WHERE ended < ?1", params![now_seconds() - days.max(0) * 24 * 3600])
            .map_err(|e| e.to_string())
    }

    // ---- sessions -----------------------------------------------------

    /// A new session for `user_id`; the returned token is secret (only its
    /// hash is stored).
    pub fn create_session(&self, user_id: i64) -> Result<String, String> {
        let token = crate::secret_token();
        let now = now_seconds();
        self.db
            .execute(
                "INSERT INTO sessions(token_hash, user_id, created, expires) VALUES (?1, ?2, ?3, ?4)",
                params![token_hash(&token), user_id, now, now + SESSION_SECONDS],
            )
            .map_err(|e| e.to_string())?;
        Ok(token)
    }

    /// The account a live session belongs to (its expiry slides forward).
    pub fn session_user(&self, token: &str) -> Result<Result<User, AuthProblem>, String> {
        let now = now_seconds();
        let row: Option<(i64, i64)> = self
            .db
            .query_row(
                "SELECT user_id, expires FROM sessions WHERE token_hash = ?1",
                params![token_hash(token)],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        let Some((user_id, expires)) = row.filter(|(_, exp)| *exp > now) else {
            if row.is_some() {
                self.end_session(token)?;
            }
            return Ok(Err(AuthProblem::SessionExpired));
        };
        let _ = expires;
        let user = self
            .db
            .query_row("SELECT id, name, phc, disabled, created FROM users WHERE id = ?1", params![user_id], |r| {
                Ok(User { id: r.get(0)?, name: r.get(1)?, phc: r.get(2)?, disabled: r.get::<_, i64>(3)? != 0, created: r.get(4)? })
            })
            .map_err(|e| e.to_string())?;
        if user.disabled {
            return Ok(Err(AuthProblem::Disabled));
        }
        self.db
            .execute("UPDATE sessions SET expires = ?1 WHERE token_hash = ?2", params![now + SESSION_SECONDS, token_hash(token)])
            .map_err(|e| e.to_string())?;
        Ok(Ok(user))
    }

    pub fn end_session(&self, token: &str) -> Result<(), String> {
        self.db.execute("DELETE FROM sessions WHERE token_hash = ?1", params![token_hash(token)]).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Drop expired sessions (the server calls this now and then).
    pub fn prune_sessions(&self) -> Result<usize, String> {
        self.db.execute("DELETE FROM sessions WHERE expires <= ?1", params![now_seconds()]).map_err(|e| e.to_string())
    }

    // ---- matches ------------------------------------------------------

    pub fn record_match(&self, m: &MatchRow) -> Result<(), String> {
        self.db
            .execute(
                "INSERT OR REPLACE INTO matches(id, player0, player1, started, ended, outcome, cause, revisions) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![m.id, m.players[0], m.players[1], m.started, m.ended, m.outcome, m.cause, m.revisions],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn matches(&self, limit: i64) -> Result<Vec<MatchRow>, String> {
        let mut st = self
            .db
            .prepare("SELECT id, player0, player1, started, ended, outcome, cause, revisions FROM matches ORDER BY ended DESC LIMIT ?1")
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map(params![limit], |r| {
                Ok(MatchRow {
                    id: r.get(0)?,
                    players: [r.get(1)?, r.get(2)?],
                    started: r.get(3)?,
                    ended: r.get(4)?,
                    outcome: r.get(5)?,
                    cause: r.get(6)?,
                    revisions: r.get(7)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
    }

    /// A consistent copy of the live database at `to` (which must not
    /// exist): `VACUUM INTO`, safe while the server runs.
    pub fn backup(&self, to: &Path) -> Result<(), String> {
        if to.exists() {
            return Err(format!("{} exists already", to.display()));
        }
        let target = to.to_str().ok_or("the backup path must be UTF-8")?;
        self.db.execute("VACUUM INTO ?1", params![target]).map_err(|e| e.to_string())?;
        Ok(())
    }
}
