//! A lost graphics device (issue #21): the driver reset, the laptop slept or
//! switched GPU, and wgpu reports the device lost. Nothing the renderer
//! holds is valid after that, and the render systems that touch it panic
//! one after another (wgpu's `Buffer::get_mapped_range`, then a poisoned
//! lock in bevy_pbr's clustering), which used to end the game with a panic
//! and a backtrace in game.log.
//!
//! Now the game ends with one plain line in game.log (its stderr) and exit
//! code [`EXIT_CODE`] instead:
//!
//! - [`plugin`] puts its own device-lost callback on the render device each
//!   time one is made (it replaces Bevy's, which only logs the loss and asks
//!   the app to exit); the callback notes the loss and logs it, and the next
//!   update of the game quits plainly.
//! - [`install_panic_hook`] (before the app is built): a panic after the
//!   loss, or one whose message says the device is lost, quits plainly at
//!   once instead of unwinding into the next panic. Every other panic goes
//!   to the previous hook as before.
//!
//! The save is not at risk: the memory card and the interim save are only
//! written at save points, each by writing a new file beside the old one and
//! renaming it over it (`memory_card_store::replace`, `interim_save::write`),
//! so quitting at any moment leaves the last complete save. Recovering the
//! device (Bevy's `RenderErrorPolicy::Recover`) is not tried: the game's own
//! GPU resources would all need making again. It changes nothing the game
//! does or draws while the device works.

use bevy::{
    app::{App, Update},
    render::{RenderApp, RenderStartup, renderer::RenderDevice},
    ecs::system::Res,
};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

/// The exit code after a lost device (1 is any other error).
pub const EXIT_CODE: i32 = 3;

/// The line game.log gets.
pub const MESSAGE: &str = "Game: the graphics driver stopped responding (the GPU device was lost, for example after sleep, a GPU switch or a driver reset), so the game closed. Your save is safe: it is only written at save points, each one complete. Start the game again to go on.";

/// The device was lost.
static LOST: AtomicBool = AtomicBool::new(false);
/// The plain line was written (once, whichever thread gets there first).
static REPORTED: AtomicBool = AtomicBool::new(false);

/// Whether a panic or driver message says the device is lost.
pub fn says_device_lost(text: &str) -> bool {
    let t = text.to_ascii_lowercase();
    [
        "device is lost",
        "device lost",
        "devicelost",
        "device_lost",
        "device removed",
        "device_removed",
        "device hung",
        "device_hung",
    ]
    .iter()
    .any(|k| t.contains(k))
}

/// Whether a panic with this message ends the game plainly: any panic once
/// the device is lost (they follow from it), or one that says so itself.
pub fn ends_plainly(lost: bool, panic_text: &str) -> bool {
    lost || says_device_lost(panic_text)
}

/// The line written: [`MESSAGE`] and the first line of what was seen.
pub fn report_line(detail: &str) -> String {
    let first = detail.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let first: String = first.chars().take(300).collect();
    if first.is_empty() { MESSAGE.to_string() } else { format!("{MESSAGE} (details: {first})") }
}

fn quit_plainly(detail: &str) -> ! {
    if !REPORTED.swap(true, Ordering::SeqCst) {
        let mut err = std::io::stderr().lock();
        let _ = writeln!(err, "{}", report_line(detail));
        let _ = err.flush();
    }
    std::process::exit(EXIT_CODE)
}

fn panic_text(info: &std::panic::PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    let message = payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_default();
    match info.location() {
        Some(at) => format!("{message} at {}:{}", at.file(), at.line()),
        None => message,
    }
}

/// Chain a hook in front of the current one (call once, before the app).
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let text = panic_text(info);
        if ends_plainly(LOST.load(Ordering::SeqCst), &text) {
            LOST.store(true, Ordering::SeqCst);
            quit_plainly(&text);
        }
        previous(info);
    }));
}

/// The render device's lost callback (replaces Bevy's).
fn watch_device(device: Res<RenderDevice>) {
    device.wgpu_device().set_device_lost_callback(|reason, text| {
        // Destroyed: the device was dropped on purpose (closing).
        if format!("{reason:?}") == "Destroyed" {
            return;
        }
        bevy::log::error!("GPU device lost: {reason:?} {text}");
        LOST.store(true, Ordering::SeqCst);
    });
}

fn quit_when_lost() {
    if LOST.load(Ordering::SeqCst) {
        quit_plainly("the driver reported the device lost");
    }
}

/// Add after the render plugin (DefaultPlugins).
pub fn plugin(app: &mut App) {
    app.add_systems(Update, quit_when_lost);
    if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
        render_app.add_systems(RenderStartup, watch_device);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lost_device_is_told_from_other_panics() {
        // wgpu-core's DeviceError::Lost, a Vulkan / DX12 loss, Bevy's log.
        for lost in [
            "Error in Queue::submit: Validation Error\n\nCaused by:\n  Parent device is lost",
            "ERROR_DEVICE_LOST",
            "DXGI_ERROR_DEVICE_REMOVED",
            "Caught DeviceLost error: Unknown driver implementation is at fault",
            "The GPU device hung",
        ] {
            assert!(says_device_lost(lost), "{lost}");
            assert!(ends_plainly(false, lost), "{lost}");
        }
        for other in [
            "index out of bounds: the len is 3 but the index is 7",
            "called `Option::unwrap()` on a `None` value",
            "memory card: access denied",
            "Error in Buffer::get_mapped_range: Buffer is not mapped",
        ] {
            assert!(!says_device_lost(other), "{other}");
            assert!(!ends_plainly(false, other), "{other}");
        }
        // After the loss, the panics it causes end plainly too (wgpu's
        // mapped range, then bevy_pbr's poisoned lock).
        assert!(ends_plainly(true, "Error in Buffer::get_mapped_range: Buffer is not mapped"));
        assert!(ends_plainly(true, "called `Result::unwrap()` on an `Err` value: PoisonError { .. }"));
    }

    #[test]
    fn the_line_is_plain_and_one_line() {
        let line = report_line("\n  Error in Queue::submit: Validation Error\n\nCaused by:\n  Parent device is lost\n");
        assert!(line.starts_with(MESSAGE));
        assert!(line.ends_with("(details: Error in Queue::submit: Validation Error)"));
        assert!(!line.contains('\n'));
        assert!(line.contains("Your save is safe"));
        assert_eq!(report_line(""), MESSAGE);
        assert!(report_line(&"x".repeat(5000)).len() < MESSAGE.len() + 330);
    }
}
