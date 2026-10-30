//! Research tool: CPU raster of resumed c400 cloud frames as raw band RGB.
//! Writes `step-NNN-counter-CCC.rgb` (512x224, rows 144..368) for comparison
//! against independently captured original frames. Private inputs stay local.
use roses_game::{
    cloud_raster::{BAND_BOTTOM, BAND_TOP, CLOUD_DIMX, WIDTH, fade_band, raster_band_over},
    cloud_source::load_cloud_source,
    clouds::TickInput,
};
use std::path::PathBuf;

fn run() -> Result<(), String> {
    let (mut source, mut root, mut out, mut background) = (None, None, None, None);
    let mut steps = 0_u32;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .ok_or(format!("{} requires a value", arg.to_string_lossy()))
        };
        match arg.to_str() {
            Some("--cloud-source") => source = Some(PathBuf::from(value()?)),
            Some("--data-root") => root = Some(PathBuf::from(value()?)),
            Some("--out-dir") => out = Some(PathBuf::from(value()?)),
            // Optional raw band RGB used as the pre-cloud framebuffer.
            Some("--background-rgb") => background = Some(PathBuf::from(value()?)),
            Some("--steps") => {
                steps = value()?
                    .to_str()
                    .and_then(|v| v.parse().ok())
                    .filter(|v| *v <= 1000)
                    .ok_or("--steps must be 0..=1000")?
            }
            _ => return Err(format!("Unknown argument: {}", arg.to_string_lossy())),
        }
    }
    let root = root.ok_or("--data-root is required")?;
    let out = out.ok_or("--out-dir is required")?;
    if !out.is_dir() {
        return Err(format!(
            "Output directory does not exist: {}",
            out.display()
        ));
    }
    let mut loaded = load_cloud_source(&source.ok_or("--cloud-source is required")?, &root)?;
    let band = WIDTH * (BAND_BOTTOM - BAND_TOP) * 3;
    let background = match background {
        Some(path) => {
            let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
            if bytes.len() != band {
                return Err(format!("Background must be exactly {band} bytes"));
            }
            bytes
        }
        None => fade_band(),
    };
    let atlas = loaded.rgba_gs.clone();
    let write = |step: u32, counter: u32, quads: &[_]| -> Result<(), String> {
        let rgb = raster_band_over(quads, &atlas, CLOUD_DIMX, &background);
        let path = out.join(format!("step-{step:03}-counter-{counter}.rgb"));
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, &rgb))
            .map_err(|e| format!("{}: {e}", path.display()))
    };
    if loaded.emission_checkpoint {
        let quads = loaded.state.emit(&loaded.sine);
        return write(0, loaded.state.counter(), &quads);
    }
    for step in 0..=steps {
        for _ in 0..loaded.external_rng_calls_per_tick {
            loaded.rng.next_u31();
        }
        let frame = loaded
            .state
            .step(&mut loaded.rng, &loaded.sine, TickInput::default())?;
        write(step, frame.counter_used, &frame.quads)?;
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
