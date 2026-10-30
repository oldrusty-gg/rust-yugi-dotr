//! The intro movie through the built-in PSS decoder (crates/roses-movie via
//! `roses_client::movie_native`): frames are drawn into the opening's
//! overlay image (640x448 canvas, shown 4:3 like every other screen), the
//! sound track goes to the game's one mixer, so ROSES_MUTE=1 keeps it
//! silent. No system codec or external tool is involved. Imports that still
//! hold a converted MP4 keep using the Media Foundation player.
use bevy::prelude::*;
use roses_client::movie_native::{MovieState, NativeMovie};

/// True when the movie asset is the disc's own PSS stream.
pub fn is_native(path: &std::path::Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pss"))
}

/// One update. `Ok(true)` once the movie has played to its end.
pub fn update(
    player: &mut Option<NativeMovie>,
    path: &std::path::Path,
    mixer: Option<std::sync::Arc<std::sync::Mutex<roses_client::sound_mix::Mixer>>>,
    overlay: Option<&mut Image>,
) -> Result<bool, String> {
    if player.is_none() {
        *player = Some(NativeMovie::open(path, mixer)?);
        eprintln!("Movie: built-in decoder started ({})", path.display());
    }
    let movie = player.as_mut().expect("opened above");
    let (state, changed) = movie.update()?;
    if changed {
        if let (Some(image), Some(frame)) = (overlay, movie.frame()) {
            *image = super::ui_image(roses_client::Image {
                width: frame.width,
                height: frame.height,
                rgba: frame.rgba.clone(),
            });
        }
    }
    if state == MovieState::Ended {
        eprintln!(
            "Movie: ended after frame {}",
            movie.frame().map_or(-1, |f| f.index as i64)
        );
        *player = None;
        return Ok(true);
    }
    Ok(false)
}
