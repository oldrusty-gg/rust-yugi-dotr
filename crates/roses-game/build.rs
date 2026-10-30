// Belt and braces for the main thread's stack (Windows gives the main
// thread 1 MiB). The game's scene changes run on the main thread (Bevy's
// exclusive systems) and move whole scenes by value; in the dev profile
// (opt-level 1) every move is a stack copy. On 9 Oct the online duel's setup
// (Online -> Duel) overflowed it: `DuelPlay` was 49 KB inline and the
// frames of `online_render::duel_scene` (399 KB) and `game_switch` (340 KB)
// held several copies. The real fix keeps `DuelPlay`'s large presentation
// fields on the heap (a test holds it under 8 KB); this only gives the main
// thread 8 MiB of headroom, as Linux and macOS give by default.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if os == "windows" {
        if env == "msvc" {
            println!("cargo:rustc-link-arg-bins=/STACK:8388608");
        } else if env == "gnu" {
            println!("cargo:rustc-link-arg-bins=-Wl,--stack,8388608");
        }
    }
}
