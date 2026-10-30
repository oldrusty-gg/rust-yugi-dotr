//! Best-effort bounded diagnostics beside this executable, never in game assets.
use std::io::Write;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

static PATH: OnceLock<std::path::PathBuf> = OnceLock::new();
pub fn initialize() {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let _ = PATH.set(parent.join("roses-client.log"));
        }
    }
    std::panic::set_hook(Box::new(|info| {
        record(&format!(
            "PANIC: {info}\n{}",
            std::backtrace::Backtrace::force_capture()
        ));
    }));
    record(&format!(
        "START version={} pid={}",
        env!("CARGO_PKG_VERSION"),
        std::process::id()
    ));
}
pub fn record(message: &str) {
    let Some(path) = PATH.get() else {
        return;
    };
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return;
    };
    if file
        .metadata()
        .map_or(true, |metadata| metadata.len() > 4 * 1024 * 1024)
    {
        return;
    }
    let milliseconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let bounded: String = message.chars().take(48 * 1024).collect();
    let _ = writeln!(file, "{milliseconds} pid={} {bounded}", std::process::id());
    let _ = file.flush();
}
