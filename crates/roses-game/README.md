# roses-game

The game itself: a Bevy 0.19 application (`roses-bevy-scene.exe`; the name
is kept because the launcher starts the game by it) with the screens,
rendering, input, audio and the online client. It is its own Cargo
workspace so that `cargo test --workspace` at the top stays quick; it uses
the rules and models of `roses-core`, `roses-client`, `roses-assets` and
`roses-net`.

```powershell
cargo build --release --features renderer --bin roses-bevy-scene
$env:ROSES_MUTE = '1'; cargo test --features renderer
```

Start it the way the launcher does: `roses-bevy-scene.exe --game
<roses-data\game-...\game.json>`, where `game.json` is the import the
launcher (or `roses import-game --iso <your.iso> --out <new folder>`) made
from your own disc.
`--help` lists the developer options (scene checks, replays, captures).

`assets/` holds the game's own WGSL shaders (no game data). The tests in
`tests/` validate them with naga and need no GPU. Tests that need an import
are `#[ignore]`d and name the environment variable to set
(`ROSES_GAME_IMPORT`, `ROSES_DUEL_IMPORT`, ...).
