//! The hub's TLS listener and certificates. A server on a real host name
//! uses Let's Encrypt's `fullchain.pem` / `privkey.pem` (re-read when the
//! files change, so a renewal needs no restart); a small private server
//! can make its own certificate ([`self_signed`]) and give friends its
//! fingerprint to pin.

use crate::hub::{HubAcceptor, HubConn};
use roses_net::tls::{accept_stream, fingerprint, server_config};
use roses_net::transport::{FramedTcp, MAX_CLIENT_FRAME};
use roses_net::lobby::{ClientFrame, ServerFrame};
use std::io;
use std::net::{IpAddr, SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

/// Write a new self-signed certificate (`cert.pem`) and key (`key.pem`)
/// for `names` (host names or IP addresses) into `dir`; returns the
/// certificate's fingerprint for clients to pin. Existing files are kept.
pub fn self_signed(dir: &Path, names: &[String]) -> Result<String, String> {
    let (cert_path, key_path) = (dir.join("cert.pem"), dir.join("key.pem"));
    if cert_path.exists() || key_path.exists() {
        return Err(format!("{} or {} exists already; remove them first", cert_path.display(), key_path.display()));
    }
    if names.is_empty() {
        return Err("give at least one --name (the address friends connect to)".into());
    }
    let certified = rcgen::generate_simple_self_signed(names.to_vec()).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    std::fs::write(&cert_path, certified.cert.pem()).map_err(|e| e.to_string())?;
    write_private(&key_path, certified.signing_key.serialize_pem().as_bytes())?;
    Ok(fingerprint(certified.cert.der()))
}

/// Write a file only its owner can read where the platform allows it.
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .map_err(|e| e.to_string())?;
        f.write_all(bytes).map_err(|e| e.to_string())
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, bytes).map_err(|e| e.to_string())
    }
}

/// The SHA-256 fingerprint of the first certificate in a PEM file.
pub fn file_fingerprint(cert_pem: &Path) -> Result<String, String> {
    roses_net::tls::first_certificate(cert_pem).map(|der| fingerprint(&der)).map_err(|e| e.to_string())
}

/// A TLS listener on any address (TLS makes leaving the machine safe).
pub struct TlsListener {
    listener: TcpListener,
    config: Arc<rustls::ServerConfig>,
    cert: PathBuf,
    key: PathBuf,
    stamp: (Option<SystemTime>, Option<SystemTime>),
    checked: Instant,
    /// Lines for the server log (certificate reloads).
    pub notes: Vec<String>,
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

impl TlsListener {
    pub fn bind(addr: SocketAddr, cert: &Path, key: &Path) -> io::Result<Self> {
        let config = server_config(cert, key)?;
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            listener,
            config,
            cert: cert.to_path_buf(),
            key: key.to_path_buf(),
            stamp: (modified(cert), modified(key)),
            checked: Instant::now(),
            notes: Vec::new(),
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Re-read the certificate when its files changed (checked once a minute).
    fn maybe_reload(&mut self) {
        if self.checked.elapsed() < Duration::from_secs(60) {
            return;
        }
        self.checked = Instant::now();
        let stamp = (modified(&self.cert), modified(&self.key));
        if stamp == self.stamp {
            return;
        }
        match server_config(&self.cert, &self.key) {
            Ok(config) => {
                self.config = config;
                self.stamp = stamp;
                self.notes.push(format!("TLS certificate reloaded from {}", self.cert.display()));
            }
            // A renewal half written: keep the old certificate, try again.
            Err(e) => self.notes.push(format!("TLS certificate not reloaded yet: {e}")),
        }
    }
}

impl HubAcceptor for TlsListener {
    fn accept(&mut self) -> io::Result<Option<(HubConn, Option<IpAddr>)>> {
        self.maybe_reload();
        match self.listener.accept() {
            Ok((sock, peer)) => {
                let stream = accept_stream(self.config.clone(), sock)?;
                let conn = FramedTcp::<ServerFrame, ClientFrame, _>::over(stream)
                    .with_max_frame(MAX_CLIENT_FRAME)
                    .with_send_queue(crate::hub::SEND_QUEUE_BYTES);
                Ok(Some((Box::new(conn), Some(peer.ip()))))
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn take_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notes)
    }
}
