# Port additions and faithful mode

The game is a recreation of the 2003 PAL release. A few things were added
that the original does not have. They can be switched off, so the original
can be played unchanged:

- **At run time:** start the game with `--faithful`, or set
  `ROSES_FAITHFUL=1`. The launcher's **Online Play and extras** checkbox
  (next to Play) does this when it is unticked. It is ticked by default and
  remembered per game in `launcher-state.json` (`toggles.extras`). The game
  declares the switch in its manifest
  (`launch_toggle` in the launcher's built-in `crates/roses-launcher/games/roses.json`,
  in the oldrusty repository).
- **At build time:** build without the `online` cargo feature, which is on
  by default:

  ```
  cd crates/roses-game
  cargo build --no-default-features --features renderer --bin roses-bevy-scene
  cargo test -p roses-core --no-default-features      # from the workspace
  cargo test -p roses-client --no-default-features
  ```

  A build like this has no network code and none of the additions. It
  always runs in faithful mode. `roses_game::faithful()`
  returns true, and the game logs "faithful mode (a build without the
  online feature)".

The feature is named `online` in `roses-game`,
`roses-core` and `roses-client`. In the game crate, the feature also
enables `roses-core/online`, `roses-client/online` and the `roses-net`
dependency. `roses-net` and `roses-server` always ask for
`roses-core/online`.

## What counts as an addition (hidden in faithful mode)

| Addition | Where | Faithful mode |
|---|---|---|
| The title menu's fifth entry, **Online Play**, below Trade | `online_title.rs`, `opening.rs` (`OpeningPreview::open_with`) | Not added. The title menu has the original four entries. |
| The Online Play screens: server, log in, sign up, lobby, challenge and accept, room codes, the server's computer opponent, results | `online_menu.rs`, `online_screen.rs`, `online_render.rs` | Unreachable. Without `online` they are compiled out (`online_render_off.rs`). |
| Online decks: CHOOSE DECK, custom decks (`custom-decks.json`, `ROSES_CUSTOM_DECKS`), the remembered choice (`online-deck.json`), the DECKS row (new, copy, edit, name, rank, duplicate, delete) | `online_decks.rs`, `online_menu.rs` | Unreachable or compiled out. |
| The deck view (Triangle, with sort and card detail), COPY TO MY DECKS, and the full-pool **deck builder** that reuses the deck editor | `deck_editor_render.rs` (`builder`), `deck_editor_screen.rs` (`copies`), `online_render.rs` | Only Online Play opens them. Without `online`, `online_render::building` is always false. |
| CHOOSE MAP (random, VS maps, campaign boards, own maps) and COMPUTER DECK | `online_menu.rs`, `custom_duel_screen.rs` (map preview reused) | Unreachable or compiled out. |
| The online duel: a server-authoritative duel, its link, the other seat's streamed presses, the pad bot | `duel_online.rs`, `duel_play.rs` (`online`, `online_press`), `duel_board_render.rs` (`--online`, `online_play`, `prepare_online_scene`, `online_end_exit`) | `--online` is refused. Without `online` there is a stub with no constructor (`duel_online_off.rs`). |
| **Display names**: account names on the HUD's PLAYER / ENEMY plates and graveyard tabs | `roses-client` `duel_hud.rs` (`HudState::names`, `plate_name`), `duel_play.rs` (`seat_names`) | Never set. Without `online` the field is compiled out. |
| **Spectating** (watching online duels): WATCH A DUEL in the lobby (the live duels by both players' names, the board, the round and how many watch), joining mid-duel from a snapshot, the shared-screen camera on the active side, both LP plates and names, Start to stop watching, a small SPECTATING / "N WATCHING" note under the round counter, and the ALLOW WATCHING setting (`online-watching.json`, on by default). Spectators get the view of nobody: never a hand or a face-down card, even when Card Display is OPEN | `online_menu.rs` (`Screen::Watch`), `online_screen.rs` (`Row::Live`), `online_render.rs` (`start_watching`), `duel_online.rs` (`OnlineLink::watch`, `STOPPED_WATCHING`), `duel_play.rs` (`spectating`, `online_note`), `roses-client` `duel_hud.rs` (`HudState::notes`); server and protocol in `roses-net` (`spectator_view`, `Join::Watch`, `ListMatches`, `Watch`, `AllowSpectators`) and `roses-server` (the spectator log, `--max-spectators`, `--max-spectators-per-match`, `--spectate-delay-turns`) | Unreachable: it is part of Online Play. Without `online` it is compiled out; `HudState::notes` is never set. |
| **MODES** (invented 4-player modes, MULTIPLAYER-DESIGN.md): a 4-player free-for-all and 2v2 on the original board (leaders at the middle of each edge, turns round the table, knockouts, a chosen target enemy for single-target effects, the 40-round cap), online rooms by code with the room settings (knocked-out cards LEAVE / STAY, starting LP, separate / shared team LP, turn clock, spectators allowed), computer players on 2-side projections of the board, N-player spectating | engine `roses-core` `duel/multi.rs`, `duel/multi/computer.rs` (`Duel::multi`, `new_multi`, `forfeit_with`, `DuelCommand::Target`, `DuelEvent::KnockedOut` / `TargetChosen`, `build_decks`); `roses-net` (`RoomSettings`, `RoomInfo`, the `CreateModeRoom` .. `StartModeRoom` requests, `ModeRoom` / `MultiMatchReady` events, `modes` in the lobby hello and welcome, `Welcome::multi`, `PlayerView::multi` / `team_hands`); `roses-server` `multi.rs` (`MultiMatch`, `MultiReplay`), the hub's mode rooms (`lobby.rs`, `hub.rs`), `--multi-computer-delay-ms`; the game's screens: `crates/roses-game/src/duel_multi.rs` (stub `duel_multi_off.rs`), the MODES rows and screens in `online_menu.rs` / `online_screen.rs` / `online_render.rs`, side 2/3 branches in `duel_play.rs`, `duel_overlay.rs`, `duel_models.rs`, `duel_online.rs`, test hooks `--multi-preview`, `--multi-until` (`over`, `ko`, `ko-show`, `round:N`), `ROSES_MULTI_CAPTURE_ROUND`, `ROSES_MULTI_PREVIEW_STAY`, keyboard T (target); the N-player HUD (14 Oct, second pass): a player always sees its own and its 2v2 team-mate's cards (Card Display OPEN for those sides on the client, `DuelPlay::multi_own_sides`; the left LP plate never shows ??) while enemies stay HIDE and R2 reveals nothing — the server's Welcome still sends the default switches, so the wire and 1v1 are unchanged; the compact plates carry the seats' leader portraits (the same PICMINI pictures as the LP plates, 32 x 32, `roses-client` `HudTextures::extra_portraits` / portrait slots 2 + side); a knockout with LEAVE plays the original trap-victim destruction (c603 pieces and c545 puffs with their sounds) on each of the player's units at the overview camera (`duel_multi::KnockoutShow`), with a "<NAME> OUT" note; with STAY the player's units stay as obstacles drawn dim (the original's acted-unit brightness 64); E and W cards, models and versus cards take their facing from the seat yaw as sides 0 and 1 do (0xC000 − camera yaw: `duel_multi::model_yaw`, `duel_play::side_model_base` in `board_card_pose`, `landing_pose` and the victims' c603) | Without `roses-core/online` the N-player state and constructor are compiled out (`Duel::multi()` is always `None`; every helper then answers the original's `side ^ 1`); without the game's `online` feature the screens are compiled out; with `--faithful` they are unreachable (part of Online Play). |
| **N-player Exodia** (29 Oct; chosen with Jim): in a 4-player free-for-all and in 2v2, a player whose Exodia is complete at their own turn start (the original's 0x269D80 test: Exodia the Forbidden One as the leader, the four limbs face up, ready and unbound in its eight neighbours) wins the whole match at once (`EndReason::Exodia`, nobody is knocked out first); in 2v2 that is their team's win (the team-mate places 1st with them, `duel_multi::placings`). It is the 2-player rule unchanged, reached through the same turn start, so 1v1 is identical. The original's Exodia event 0x6D (`duel_exodia.rs`) plays before the placings as in 1v1; Summoned Lord Exodia stands behind the winner's own edge (seats 2 / 3: x = +350 / -350, an invented place), and the cards the event reveals are every other player's (the original reveals the one opponent's group). | `roses-core` `duel.rs` `start_turn` / `exodia_complete` (shared), tests `duel/multi/tests.rs` (`exodia_*`); `crates/roses-game/src/duel_exodia.rs` (`lord_pose` sides 2 / 3, `ExodiaRun::start`) | Part of MODES (unreachable). |
| The black cut used instead of a fade into and out of Online Play | `mode_fade.rs` | Unreachable. |
| Rules commands that are not the original's: the server's **forfeit**, and the per-player filtered **view** (and the spectators' view of nobody) | `roses-core` `duel.rs` (`Duel::forfeit`, `Duel::view`, `Duel::view_as`, `Viewer`, `DuelView`, `UnitView`) | Compiled out without `roses-core/online`. Offline duels never use them. |
| Online files read when Online Play opens: `online.json`, `online-session.json` (the launcher's sign-in handoff), `online-watching.json` (ALLOW WATCHING), and the server address and pin | `online_render.rs` | Never read. |
| Online test hooks: `ROSES_ONLINE_KEYS`, `_SHOTS`, `_ROOM_FILE`, `_CAPTURE*`, `ROSES_ONLINE_PADBOT`, `ROSES_ONLINE_DEBUG`, `ROSES_BUILDER_REPLAY`, `--capture-game-scene online`, `--online-bot`, `--online-until` | `online_render.rs`, `duel_board_render.rs`, `main.rs` | Refused or compiled out. |

No other player-facing additions were found: no extra entries in the
campaign SYSTEM or OPTION menus, Custom Duel, Trade or the deck editor; no
cheat, unlock or speed keys; no mouse input (audit of 10 Oct 2026).

## Kept in faithful mode (PC presentation, stand-ins for PS2 hardware)

These are not gameplay additions:

- The window (960 x 720, the view fitted at 4:3) and window titles.
- Gamepads (XInput, DualSense, DualShock 4) mapped to the PS2 pad by
  position.
- The keyboard mapped to the pad: arrows = D-pad, X Cross, Z Circle,
  A Square, D Triangle, S Start, Tab Select, Q/E L1/R1, 1/W L2/R2, C/V L3/R3
  in the deck editor, and Enter as Cross on menus. Escape closes the window.
- The virtual memory card (slot 1 always present, slot 2 a folder), with
  the port's `port.json` sidecar beside the byte-exact card file. The
  sidecar keeps the campaign state the port needs to continue. The one-time
  migration of the old interim save is also kept.
- The black "no movie" screen, shown when an old import has no ending movie.
- `ROSES_MUTE` and the development, capture and research flags (`--capture*`,
  `--*-replay`, `--duel-board`, `ROSES_TRACE_*` and the like). Players do not
  reach these.

## Tools (not gameplay)

These make videos or checks. A player never reaches them, they change
nothing in the game, and they behave the same in faithful mode.

- `ROSES_EXODIA_TEST=<side>` (29 Oct, a private test for the Exodia event): on the first turn of another side in a local duel, that side's leader becomes Exodia the Forbidden One with the four limbs face up around it, so its next turn start wins by Exodia (`duel_exodia::rig_for_test`, hook in `DuelPlay::press`). Not set, it does nothing.

- **MODES video driver** (30 Oct 2026, video aid): `video_bot.rs` plays an N-player seat through its pad with the ported computer modules on the server computers' 2-side projection (seat-turned d-pad, the Target key); `ROSES_BOT_EXODIA=1` adds an Exodia aid for an Exodia deck (the leader steps off the edge once, limbs are summoned round it and revealed when all four stand there, a turn the module ends without a summon plays one hand card). `scripts/video/run-modes-video.ps1` (four clients, one MODES room), `rehearse-local.ps1 -Mode 2v2|ffa`, the menu script's `ok:LABEL`, `roses-server hub --match-seed` now also seeds MODES rooms, and the ignored search `video_multi_seed_search`. Never used by a player.

- **Video recorder** (29 Oct 2026). Writes rendered frames and the game's
  own mixed sound straight into an H.264/AAC MP4 through Windows Media
  Foundation's Sink Writer, which is part of Windows (no ffmpeg, nothing
  installed). Code: `crates/platform-video` (the encoder, the 4:3 nearest
  scaling and NV12 conversion, the segment format, an MP4 box reader, the
  `video-check` example); the game's `crates/roses-game/src/record.rs`;
  the launcher's `crates/roses-launcher/src/record.rs` and `record_clip` in
  `windows.rs`. Small hooks elsewhere: `duel_board_render::NO_SOUND_OUTPUT`
  (the mixer without an output device), `roses-client`
  `movie_native::set_lockstep` and `sound_mix::Mixer::se_started`.
  - Game: `--record <new.mp4> [--record-size 1920x1080] [--record-fps 60]
    [--record-seconds S] [--record-keys 120:down,2.5s:x*10]
    [--record-realtime]` with `--game` (or `--scene`). Headless: no window,
    no gamepads, no sound device (speakers silent with or without
    `ROSES_MUTE=1`; the file still has the sound). Game time advances
    exactly 1/fps per frame, and the clock is held until every pipeline is
    ready, so the clip starts on a fully drawn frame. `ROSES_RECORD_WAV=<file>`
    also writes the exact mixed sound as a WAV. Without `--record-seconds`
    it records until the game closes or `ROSES_RECORD_STOP_FILE=<path>`
    exists (killing the process leaves an unplayable MP4). `ROSES_RECORD_ENCODER=software`
    skips a hardware H.264 encoder.
  - Launcher: `ROSES_LAUNCHER_RECORD=<new.mp4>` with an optional
    `ROSES_LAUNCHER_RECORD_SCRIPT` (`wait:S`, `move:game:<id>|play|import|toggle|X;Y:S`,
    `click`, `pad:<dir>`). The window is never shown. The page is drawn
    off-screen at 1800x1080 with a drawn mouse cursor. A click on Play is
    drawn pressed, but nothing is started.
  - Every clip uses one segment format: 1920x1080, 60 fps, H.264 High,
    AAC-LC 192 kbit/s 48 kHz stereo (the launcher's sound is silence). A
    `<clip>.mp4.segment.json` sidecar records the format and the exact
    frame and sound counts.

## Proof that offline play is unchanged

- The N-player modes (MODES) changed the engine's per-side arrays to
  `Vec`s and routed every "other side" through helpers that return
  `side ^ 1` in a 2-player duel. Checked to leave 1v1 bit-identical
  (14 Oct 2026): the refusal sweep's 2,944 seeded sessions (64 seeds per
  control and side) give the same trajectory hash (events and final state,
  `ROSES_SWEEP_HASHES`) before and after; 10 stored server replays verify
  (`roses-server --verify-replay`, final-state hash); the seeded tests below
  pass unchanged.
- `roses-core`: `offline_computer_duels_are_the_same_with_and_without_the_online_feature`
  (`crates/roses-core/src/duel/tests.rs`) plays six seeded
  computer-vs-computer duels to their end. It checks each final state's hash
  against recorded values. The test passes in both builds
  (`cargo test -p roses-core`, and again with `--no-default-features`).
- The game: `offline_duels_are_the_same_with_and_without_the_online_feature`
  (`crates/roses-game/src/duel_play.rs`) plays four seeded 1v1 duels
  through the game's duel presentation (`DuelPlay::update_pads`, 30,000
  updates each). It checks the SHA-256 of each final duel against recorded
  values in both builds (`--features renderer`, and again with
  `--no-default-features --features renderer`).
- The title menu: `private_faithful_title_menu_has_only_the_original_entries`
  (`opening.rs`, run with `ROSES_OPENING_MANIFEST=<import>/story/manifest.json`
  and `--ignored`). Faithful mode has the original four entries. The extras
  add only `online-play`, after them.
