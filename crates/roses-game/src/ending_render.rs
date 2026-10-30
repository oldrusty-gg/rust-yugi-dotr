//! The red route's ending in the connected game (Claude, 4 Oct 2026;
//! research `work/research/story/claude-ending/NOTES.md`): after the
//! Cross on caption 3028 the story scene stays on map event 11184 while
//! `roses_client::ending::RouteEnding` closes the letterbox, the memory-card
//! op 2 dialog (`CardDialog::ending_save`) runs over the black screen, then
//! the movie MOVIE/EDMV_A6.PSS and the title (mode 10 with param[10] = 1:
//! no opening movie). The white route's 12156 runs the same way
//! (`RouteEnding::white`: SAVE[2] = 1, movie MOVIE/EDMV_C6.PSS, C2 = 0).
//!
//! The movie: `roses_client::ending::MovieRun` (Bevy-independent) plays the
//! story import's PSS (asset `ending-movie-red`) through the built-in
//! decoder, the same path as the intro (`movie_native`): frames go into the
//! opening overlay, the sound track into the game's one mixer (ROSES_MUTE=1:
//! no output, wall clock). No press ends it (0x242050's a2 = 0); its end
//! calls `RouteEnding::movie_finished` and the title follows. Imports made
//! before the ending movies were imported keep a black stand-in (logged;
//! X/Enter continues).
use super::*;
use bevy::ecs::world::World as EcsWorld;
use roses_game::memory_card_store as store;
use roses_client::ending::{EndingMovie, EndingPhase, EndingStep, MovieRun, MovieTick, RouteEnding};
use roses_client::memory_card::{CardDialog, CardOutcome, CardPad};

#[derive(Resource)]
pub(super) struct EndingRun {
    pub ending: RouteEnding,
    dialog: Option<CardDialog>,
    card: store::VirtualCard,
    boxes: Option<store::BoxRenderer>,
    /// Presses gathered by `update_opening` since the last update.
    pub pad: CardPad,
    /// The movie requested by 0x1ABB20's −1 (EDMV_A6 for the red route).
    pub movie: Option<EndingMovie>,
    /// Its playback (a Mutex only to make the decoder's channel `Sync`).
    player: Option<std::sync::Mutex<MovieRun>>,
    /// The stand-in's reason was reported (window title + stderr).
    stand_in_reported: bool,
    drawn: Option<(u32, Option<(roses_client::memory_card::CardBox, u32)>)>,
}

impl EndingRun {
    /// The Cross on the route's last caption (3028 red, 3508 white) handed
    /// map event 11184 / 12156 back: SAVE as the map left it at the story
    /// request, its event the route's ending event.
    pub fn start(flow: &GameFlow, route: roses_client::ending::Route) -> Result<Self, String> {
        let mut save = flow
            .map_save
            .clone()
            .ok_or("the ending needs the campaign map's state (no map save)")?;
        save.event = route.event() as i16;
        let card = store::VirtualCard::slot1(&flow.import.root);
        // SAVE[2] as the card's last file has it (the original's RAM copy).
        let white = card.current_save().is_some_and(|s| s[2] != 0);
        let boxes = store::BoxRenderer::from_duel_import(&flow.import.duel)
            .map_err(|e| eprintln!("Game: the memory-card boxes cannot be drawn ({e})"))
            .ok();
        Ok(Self {
            ending: match route {
                roses_client::ending::Route::Red => RouteEnding::red(save, white),
                roses_client::ending::Route::White => RouteEnding::white(save, white),
            },
            dialog: None,
            card,
            boxes,
            pad: CardPad::default(),
            movie: None,
            player: None,
            stand_in_reported: false,
            drawn: None,
        })
    }
}

/// The data file op 2 writes: the player's block (bit 5 of +0xFF9C when
/// both routes are cleared) and SAVE as the ending leaves it.
fn ending_file(world: &EcsWorld, run: &mut EndingRun) -> Result<Vec<u8>, String> {
    let flow = world.resource::<GameFlow>();
    let mut block = flow.player_block.clone().ok_or("no card block yet")?;
    if run.ending.marks_block() {
        roses_client::ending::mark_both_routes(&mut block);
    }
    let codes = flow
        .campaign
        .starter_deck
        .as_ref()
        .map(|c| c.name_codes.clone())
        .unwrap_or_default();
    let name = world
        .get_resource::<OpeningState>()
        .and_then(|o| o.preview.model.player_name().cloned());
    // SAVE[2] (set by the white ending) is not part of `CampaignSave`.
    let base = run.ending.record_base(run.card.current_save().as_ref());
    let file = store::build_file(&block, &run.ending.save, &codes, Some(&base))?;
    run.card.sidecar = Some(store::PortSidecar::new(flow.campaign.clone(), name));
    Ok(file)
}

/// One 25 Hz update of the ending (FixedUpdate while `EndingRun` exists).
pub(super) fn ending_tick(world: &mut EcsWorld) {
    let Some(mut run) = world.remove_resource::<EndingRun>() else {
        return;
    };
    let mut pad = std::mem::take(&mut run.pad);
    {
        let mut flow = world.resource_mut::<GameFlow>();
        // `--story-auto` (test aid): Cross every eighth update (YES on
        // "Save?", the acknowledge, the movie stand-in).
        if flow.story_auto && run.ending.updates % 8 == 7 {
            pad.cross = true;
        }
        // `--capture-map-event 11184 --capture-map-updates N` counts here.
        if flow.capture_map_gate.is_some() {
            flow.capture_map_updates += 1;
        }
    }
    let mut sfx = Vec::new();
    // 11184 sub-step 3 calls 0x1ABB20 before it tests the return.
    let mut closed = false;
    if run.ending.phase == EndingPhase::Save {
        if let Some(dialog) = run.dialog.as_mut() {
            let outcome = dialog.update(pad, &mut run.card);
            sfx.extend(dialog.sfx.iter().copied());
            match &outcome {
                CardOutcome::Saved => eprintln!(
                    "Game: the ending's save was written to {} ({}, SAVE+0x3C = 0)",
                    run.card.data_path().display(),
                    match run.ending.route {
                        roses_client::ending::Route::Red => "SAVE[1] = 1",
                        roses_client::ending::Route::White => "SAVE[2] = 1",
                    }
                ),
                CardOutcome::NotSaved => eprintln!("Game: the ending's save was declined (nothing written)"),
                _ => {}
            }
            if let Some(e) = dialog.error.take() {
                eprintln!("Game: ending save: {e}");
            }
            closed = outcome == CardOutcome::Closed;
        }
    }
    let mut back_to_title = false;
    match run.ending.update(closed) {
        EndingStep::Running => {}
        EndingStep::StartSave => match ending_file(world, &mut run) {
            Ok(file) => {
                let mut dialog = CardDialog::ending_save(file);
                // The first update runs in the same frame (box "Save?").
                dialog.update(CardPad::default(), &mut run.card);
                sfx.extend(dialog.sfx.iter().copied());
                run.dialog = Some(dialog);
            }
            Err(e) => {
                // Nothing to save: the original would still save its RAM;
                // here the dialog is skipped and the movie follows.
                eprintln!("Game: the ending's save cannot be built ({e}); continuing without it");
                if let EndingStep::PlayMovie(movie) = run.ending.update(true) {
                    start_movie(world, &mut run, movie);
                }
            }
        },
        EndingStep::PlayMovie(movie) => start_movie(world, &mut run, movie),
    }
    let pressed = pad.cross || pad.circle || pad.square || pad.triangle;
    let mut movie_frame = None;
    if let Some(player) = run.player.as_mut() {
        let player = player.get_mut().unwrap_or_else(|e| e.into_inner());
        match player.update(pressed) {
            MovieTick::Ended => {
                if player.is_native() || player.stand_in.is_none() {
                    eprintln!("Game: ending movie {} ended", player.movie.disc_path);
                }
                run.ending.movie_finished();
                back_to_title = true;
            }
            MovieTick::Playing { changed } => {
                if changed {
                    movie_frame = player.frame().map(|f| roses_client::Image {
                        width: f.width,
                        height: f.height,
                        rgba: f.rgba.clone(),
                    });
                }
                if let (Some(reason), false) = (player.stand_in.clone(), run.stand_in_reported) {
                    run.stand_in_reported = true;
                    run.drawn = None; // black again (a decode failure mid-movie)
                    stand_in(world, player.movie, &reason);
                }
            }
        }
    }
    if let Some(mut sound) = world.get_resource_mut::<duel_board_render::DuelSound>() {
        for id in sfx {
            sound.cue(roses_game::duel_audio::AudioCue::Se { slot: 0, id: id as u8, skip: false });
        }
    }
    let native = run
        .player
        .as_mut()
        .is_some_and(|p| p.get_mut().map_or(false, |p| p.is_native()));
    if let Some(frame) = movie_frame {
        // The movie's frame replaces the overlay (640x448, opaque).
        let handle = world.get_resource::<OpeningState>().and_then(|o| o.overlay.clone());
        if let Some(handle) = handle {
            if let Some(mut target) = world.resource_mut::<Assets<Image>>().get_mut(&handle) {
                *target = ui_image(frame);
            }
        }
    } else if !native {
        draw(world, &mut run);
    }
    if back_to_title {
        // 0x312B90 = 1, param[10] = 1, pop to the title (mode 10 init skips
        // the opening movie when param[10] != 0).
        eprintln!("Game: the ending is over; back to the title (no opening movie)");
        let mut flow = world.resource_mut::<GameFlow>();
        flow.capture_map_reached = flow.capture_map_gate.is_some();
        flow.map_save = None;
        flow.won = None;
        flow.request = None;
        flow.story_stage = Some("title".into());
        flow.next = Some(GameScene::Opening);
        return;
    }
    world.insert_resource(run);
}

/// 0x1A8CD0(2, 1, 0): the story import's PSS through the built-in decoder
/// (sound into the mixer; none when muted), or the stand-in when this
/// import has no ending movie.
fn start_movie(world: &mut EcsWorld, run: &mut EndingRun, movie: EndingMovie) {
    run.dialog = None;
    run.movie = Some(movie);
    let path = match world.get_resource::<OpeningState>() {
        Some(opening) => roses_client::ending::movie_path(&opening.preview.loaded, movie),
        None => Err("no story manifest is loaded".into()),
    };
    let mixer = world
        .get_resource::<duel_board_render::DuelSound>()
        .and_then(|s| s.mixer());
    if path.is_ok() {
        // The story's music stops for the movie's own sound track [H: the
        // stop/clear in 0x1A8D50 is not traced to the sound driver].
        if let Some(mut sound) = world.get_resource_mut::<duel_board_render::DuelSound>() {
            sound.cue(roses_game::duel_audio::AudioCue::FadeOut { speed: 0x100 });
        }
    }
    let player = MovieRun::open(movie, path, mixer);
    if player.is_native() {
        eprintln!(
            "Game: ending movie {} (index {}) through the built-in decoder; not skippable (a2 = 0)",
            movie.disc_path, movie.index
        );
    }
    run.player = Some(std::sync::Mutex::new(player));
}

/// No movie to play: black, X/Enter continues (the original has no such
/// screen; it returns when the movie ends).
fn stand_in(world: &mut EcsWorld, movie: EndingMovie, reason: &str) {
    let message = format!(
        "movie {} (index {}) cannot play: {reason} - press X/Enter for the title",
        movie.disc_path, movie.index
    );
    eprintln!("Game: stand-in: {message}");
    set_title(world, &format!("Roses - {message}"));
}

/// The overlay: the letterbox bands (black screen once closed) and the
/// dialog's box, replacing the caption page.
fn draw(world: &mut EcsWorld, run: &mut EndingRun) {
    let rows = run.ending.letterbox_rows();
    let shown = run
        .dialog
        .as_ref()
        .and_then(|d| d.overlay().map(|b| (b, d.counter / 5)));
    let key = (rows, shown);
    if run.drawn == Some(key) {
        return;
    }
    run.drawn = Some(key);
    let layer = match (shown, run.boxes.as_mut(), run.dialog.as_ref()) {
        (Some((b, _)), Some(r), Some(d)) => Some(r.layer(b, d.counter)),
        _ => None,
    };
    world.resource_scope(|world, opening: Mut<OpeningState>| {
        let Ok(base) = opening.preview.loaded.render_overlay(&opening.preview.model) else {
            return;
        };
        let (w, h) = (base.width as usize, base.height as usize);
        let mut rgba = vec![0u8; w * h * 4];
        for (y0, bh) in roses_client::ending::letterbox_bands(rows) {
            // Bands are on the 640×448 logical canvas.
            let (y0, y1) = (y0 as usize * h / 448, ((y0 + bh) as usize * h / 448).min(h));
            for px in rgba[y0 * w * 4..y1 * w * 4].chunks_exact_mut(4) {
                px.copy_from_slice(&[0, 0, 0, 255]);
            }
        }
        if let Some(layer) = layer {
            store::blend_layer(&mut rgba, w, h, &layer);
        }
        let image = roses_client::Image {
            width: base.width,
            height: base.height,
            rgba,
        };
        if let Some(handle) = opening.overlay.clone() {
            if let Some(mut target) = world.resource_mut::<Assets<Image>>().get_mut(&handle) {
                *target = ui_image(image);
            }
        }
    });
}
