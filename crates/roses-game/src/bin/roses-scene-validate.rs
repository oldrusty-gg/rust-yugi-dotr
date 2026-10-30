//! Input validation without compiling or opening the optional Bevy renderer.
use roses_game::load_scene;
use std::path::PathBuf;
fn run() -> Result<(), String> {
    let mut scene = None;
    let mut root = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--scene") => {
                scene = Some(PathBuf::from(args.next().ok_or("--scene requires a path")?))
            }
            Some("--data-root") => {
                root = Some(PathBuf::from(
                    args.next().ok_or("--data-root requires a path")?,
                ))
            }
            Some("--validate-only") => {}
            Some("--help") => {
                println!(
                    "roses-scene-validate --scene <captured-scene.json> --data-root <workspace>\nRead-only validation; no window, rendering, or GPU required."
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {}", arg.to_string_lossy())),
        }
    }
    let loaded = load_scene(
        &scene.ok_or("--scene is required")?,
        &root.ok_or("--data-root is required")?,
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&loaded.report).map_err(|e| e.to_string())?
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
