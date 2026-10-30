#![cfg_attr(windows, windows_subsystem = "windows")]
use roses_client::Loaded;
use std::path::PathBuf;
mod diagnostics;
#[cfg(windows)]
mod windows;

fn execute() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let mut manifest = None;
    let mut check = false;
    let mut fullscreen = false;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--manifest") if manifest.is_none() => {
                manifest = Some(PathBuf::from(
                    args.next().ok_or("--manifest requires a path")?,
                ))
            }
            Some("--check") => check = true,
            Some("--fullscreen") => fullscreen = true,
            Some("--help") | Some("-h") => {
                println!("roses-client --manifest <opening.json> [--check] [--fullscreen]\nImported opening media only; no native duel or story yet.\nArrows/D-pad: navigate. X: Cross. Z/Escape: Circle. S: Start. Enter: current stage's primary action. F11: fullscreen. Alt+F4: close.");
                return Ok(());
            }
            _ => {
                return Err(format!(
                    "Unknown or repeated argument: {}",
                    arg.to_string_lossy()
                ))
            }
        }
    }
    let manifest = match manifest {
        Some(path) => path,
        None => std::env::current_exe()
            .map_err(|e| e.to_string())?
            .parent()
            .ok_or("Executable has no parent directory")?
            .join("roses-data/opening/manifest.json"),
    };
    diagnostics::record(&format!("Loading manifest {}", manifest.display()));
    let loaded = Loaded::open(&manifest)?;
    diagnostics::record(&format!(
        "Assets validated; manifest SHA256 {}",
        loaded.manifest_sha256
    ));
    if check {
        println!("Validated opening manifest {} ({} assets, {} stages). Source identities are caller-provided metadata.", loaded.manifest_sha256, loaded.assets.len(), loaded.manifest.stages.len());
        return Ok(());
    }
    #[cfg(windows)]
    {
        windows::run(loaded, fullscreen)
    }
    #[cfg(not(windows))]
    {
        let _ = (loaded, fullscreen);
        Err("Native playback currently requires Windows; --check works without it.".into())
    }
}
fn main() {
    diagnostics::initialize();
    if let Err(error) = execute() {
        diagnostics::record(&format!("ERROR: {error}"));
        eprintln!("{error}");
        #[cfg(windows)]
        if !std::env::args_os().any(|arg| arg == "--check") {
            windows::show_error(&error);
        }
        std::process::exit(1);
    }
    diagnostics::record("EXIT success");
}
