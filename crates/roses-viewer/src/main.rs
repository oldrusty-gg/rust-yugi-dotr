#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
#[cfg(windows)]
mod windows;

fn main() {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let headless = arguments
        .iter()
        .any(|argument| argument == "--check" || argument == "--help");
    if let Err(error) = run(arguments) {
        eprintln!("roses-viewer: {error}");
        #[cfg(windows)]
        if !headless {
            windows::show_error(&error);
        }
        #[cfg(not(windows))]
        let _ = headless;
        std::process::exit(2);
    }
}

fn run(arguments: Vec<std::ffi::OsString>) -> Result<(), String> {
    let options = app::Options::parse(arguments)?;
    if options.help {
        println!("roses-viewer [--demo | --capture saved.json | --snapshot view.json] [--library import-folder] [--check]\nRead-only 7x7 board viewer. No gameplay or emulator interaction.\nNo arguments opens the synthetic demo. --check validates without opening a window.");
        return Ok(());
    }
    let loaded = app::Loaded::open(options.input, options.library.as_deref())?;
    if options.check {
        let document = loaded.snapshot.document();
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "schema_version": "roses-viewer-check-1", "source": loaded.source_label,
            "synthetic": loaded.snapshot.is_synthetic(), "cells": document.cells.len(),
            "origin": &document.origin,
            "coherent_board_occupants": document.units.len(), "unlinked_candidates": document.unlinked_candidates.len(),
            "lp_candidates_by_raw_group": document.lp_candidates_by_raw_group,
            "card_metadata_loaded": loaded.library.is_some(), "warnings": loaded.warnings,
            "playable": false, "gameplay_parity_validated": false
        })).map_err(|error| error.to_string())?);
        return Ok(());
    }
    #[cfg(windows)]
    return windows::run(loaded);
    #[cfg(not(windows))]
    Err("The native window currently supports Windows; use --check for platform-neutral validation.".into())
}
