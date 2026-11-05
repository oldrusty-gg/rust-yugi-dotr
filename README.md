# Roses

**An unofficial, free, non-commercial recreation of a 2003 PlayStation 2
card-strategy game, written in Rust and running natively on Windows. You play
it from your own disc.**

Roses works with your own copy of *Yu-Gi-Oh! The Duelists of the Roses*
(PlayStation 2, **PAL / Europe, SLES-51579**). It is not an emulator: the game
logic, rules, computer opponents, menus and renderer are rewritten for PC. Everything
the game shows and plays (cards, boards, models, music, movies and text) is
read from an ISO image you made of your own disc, once, on your own PC.

> Status: **beta**. The campaign, duels, deck editing, saving and online play
> work; some screens and a few rare rule situations are still missing. See [KNOWN-ISSUES](scripts/release/KNOWN-ISSUES.txt).

## Bring your own disc

- You need an ISO image dumped from **your own PAL disc** (SLES-51579), as a
  plain 1:1 `.iso` file. US and Japanese versions and modified images are
  refused: the importer checks the image's fingerprint before reading it.
- This repository and its releases contain **no game data**: no ISO, BIOS,
  executable, card text, artwork, audio, movies or saves. No emulator or BIOS
  is needed.
- Please do not open issues or pull requests that contain game files, ISO
  images, extracted data, memory-card saves or links to them. They will be
  deleted.

## Features

- One launcher from start to finish: **Import ISO** once, then **Play**.
- The story campaign from the intro movie onwards, with the original's rules
  for summoning, fusion, terrain, leader abilities, spells, traps and the
  computer opponents' decision making.
- Deck editing, the virtual memory card (save and continue like on the PS2),
  Custom Duel, Trade, the slot machine and the map editor.
- Xbox and PlayStation controllers (XInput, DualSense, DualShock 4) and
  keyboard.
- Online play (beta): accounts, a lobby, challenges and private rooms on a
  server; anyone can run their own server (`roses-server`).

## Quick start (release build)

1. Download the latest release zip from GitHub Releases and check it against
   `SHA256SUMS` (`Get-FileHash -Algorithm SHA256 <file>.zip` in PowerShell).
2. Unzip it somewhere you can write to (Documents or the Desktop, not
   Program Files). The game keeps its data beside the launcher.
3. Run `roses-launcher.exe`. Windows SmartScreen may warn about an
   unrecognised app because the files are not signed: "More info", then
   "Run anyway".
4. **Import ISO** and choose your `.iso`. This takes a few minutes and only
   reads the image.
5. **Play**.

Requirements: Windows 10 or 11 (64-bit), a GPU with DirectX 12 or Vulkan
drivers, about 2.5 GB free for the imported data.

## Controls

Gamepad buttons follow the PS2 layout by position: A/Cross = Cross,
B/Circle = Circle, X/Square = Square, Y/Triangle = Triangle, bumpers L1/R1,
triggers L2/R2, Back/Share = Select. The game window must have focus. If you
use Steam Input or DS4Windows and presses register twice, turn those off for
this game.

| Keyboard (player 1) | PS2 button |
|---|---|
| Arrow keys | D-pad |
| X | Cross |
| Z | Circle |
| A | Square |
| D | Triangle |
| S | Start (also skips the intro movie) |
| Tab | Select |
| Q / E | L1 / R1 |
| 1 / W | L2 / R2 |

## Online play

"Online Play" on the title screen connects to an online server named in
`online.json` (a release may include one beside the launcher; your own copy
in `roses-data\online.json` takes precedence). Make an account on the
launcher's Profile page or in the game. The server stores your account name,
a password hash, your login sessions and match results; see
[docs/PRIVACY.md](docs/PRIVACY.md). Both players need the same release and the
same PAL disc.

To run your own server: build `roses-server`, import the duel data from your
own ISO (`roses import-duel`), create a TLS certificate
(`roses-server tls self-signed ...`, or use a Let's Encrypt certificate) and
start `roses-server hub`. The documentation at the top of
`crates/roses-server/src/friends_cli.rs` lists every option, including
connection limits, the name filter, account deletion and log retention.

## Build from source

Windows 10/11 x64. The launcher is its own repository (`launcher`), built
side by side with this one (see [docs/REPOSITORIES.md](docs/REPOSITORIES.md)).

1. Install Rust with [rustup](https://rustup.rs) (stable 1.95 or newer; CI
   uses 1.96.1) and Visual Studio Build Tools 2022 with "Desktop development
   with C++".
2. Clone the repositories into one folder and build:

```powershell
git clone https://github.com/oldrusty-gg/platform-sdk
git clone https://github.com/oldrusty-gg/platform-accounts
git clone https://github.com/oldrusty-gg/launcher
git clone https://github.com/oldrusty-gg/rust-yugi-dotr
cd rust-yugi-dotr
cargo build --release -p roses-cli
cd crates/roses-game
cargo build --release --features renderer --bin roses-bevy-scene
cd ../../../launcher
cargo build --release -p roses-launcher
```

3. Put `roses-launcher.exe`, `roses.exe` and `roses-bevy-scene.exe` in one
   folder, with the game's `assets\*.wgsl` shaders in an `assets` folder
   beside them, or run `scripts\build-release.ps1` in this repository, which
   builds and packages all of it.

Tests need no game data (tests that do are `#[ignore]`d and say which input
they need):

```powershell
$env:ROSES_MUTE = '1'
cargo test --workspace
cd crates/roses-game; cargo test --features renderer
```

The first game build compiles about 430 crates (Bevy) and takes several
minutes.

## Project layout

| Crate | What it is |
|---|---|
| `crates/roses-game` | The game (Bevy renderer, screens, input, audio); its own workspace |
| `crates/roses-cli` | `roses.exe`: the importer and developer check commands |
| `crates/roses-client` | Game-side models of the screens (campaign map, HUD, menus) |
| `crates/roses-ee` | A developer test tool: compares the rewritten effects with the user's own executable. No shipped program contains it, and the game runs no original code |
| `crates/roses-movie` | MPEG-2 / PSS movie decoding |
| `crates/roses-server` | The authoritative duel server, lobby and accounts |
| `crates/roses-compare`, `crates/roses-viewer` | Developer tools (comparison against reference recordings, a legacy board viewer) |
| `crates/roses-core` | Rules engine: duels, fusions, computer opponents, deck editor, saves |
| `crates/roses-net` | Online duel and lobby protocol and the lobby client (accounts, TLS and connections come from `platform-net`) |
| `crates/roses-assets`, `crates/roses-disc` | ISO reading, fingerprint checks and parsers for the user's game files |
| `launcher` repository | The Windows launcher (crate `roses-launcher`) (reads `games/roses.json`, runs `roses.exe import-game` / `verify-import` and the game) |
| `platform-sdk` repository | The launcher-game contract and the shared network and account protocol |

## Legal

Roses is an independent, free, non-commercial fan project. It is not
affiliated with, endorsed, sponsored or approved by Konami, Konami Digital
Entertainment, Studio Dice or Kazuki Takahashi. Yu-Gi-Oh! and Yu-Gi-Oh! The
Duelists of the Roses are trademarks of their respective owners and are
named here only to describe compatibility. Roses contains no game files,
artwork, audio, movies, card descriptions or story text: the game runs no
original code and reads its data from your own disc. The source does mention
some card names and short phrases, mostly in comments, and keeps a few small
number tables the rewrite needs. You need your own copy of the PAL disc to
use it.

The software is provided as is, without warranty of any kind.

**Rights holders:** if you believe something in this repository or its
releases infringes your rights, open an issue in this repository, or send a
notice through [GitHub's DMCA process](https://docs.github.com/en/site-policy/content-removal-policies/dmca-takedown-policy).
We will respond promptly and remove material on request. The project has no
e-mail contact; security problems go through [SECURITY.md](SECURITY.md).

## Licence

Dual-licensed under either of

- the MIT licence ([LICENSE-MIT](LICENSE-MIT)), or
- the Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE)),

at your option. Unless you explicitly state otherwise, any contribution
intentionally submitted for inclusion in the work by you, as defined in the
Apache-2.0 licence, shall be dual-licensed as above, without any additional
terms or conditions.
