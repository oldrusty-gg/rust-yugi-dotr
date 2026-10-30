//! `roses-duel-end-check`: a scripted, GPU-free run through the end of a
//! won first duel — the result banner, the rank list, the reward slot
//! machine with its prizes granted into the player's card block, and the
//! campaign map's return — rendering the screens with the CPU GS model and
//! comparing them with the private evidence when given.
//!
//! Usage:
//!   roses-duel-end-check --game <game.json> --out <new dir> [--seed N]
//!     [--golden-dir <dir with the interpreter primitive lists>]
//!     [--truth-dir <dir with truth-f00445/541/589.png> --truth-state <json>]
//!
//! `--truth-state` names the reel state of the truth frames (strips, stopped
//! positions, popup card), read from the private savestate by the research
//! scripts; nothing of it is embedded here. Frame counts printed for the
//! result states use the model's untraced waits (sound 0, HUD slide 16,
//! camera 0) and the original's frame-exact RNG cadence is not reproduced:
//! the prizes of this run are the model's, not a prediction of the game's.
use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, Image, ImageFormat, ImageSampler, ImageType};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use roses_game::campaign_map::load_map;
use roses_game::duel_end_screens::{
    BannerKind, EndAssets, FRAME, PrizePopup, SlotTextures, banner_primitives, computer_owner_key,
    current_popup, darkening_primitive, new_game_block, render_banner_layer, render_slot_screen,
};
use roses_game::duel_play;
use roses_game::game::GameImport;
use roses_client::campaign_map::{MapModel, Pad};
use roses_client::duel_hud::{PrimKind, Primitive, TextureSource};
use roses_client::gs::Target16;
use roses_core::card_block::CardBlock;
use roses_core::duel::ai::pad;
use roses_core::duel::next_rand31;
use roses_core::duel_end::{DuelEnd, DuelEndSetup, EndEvent, EndOutcome, EndTimings};
use roses_core::slot_machine::{
    Prize, PrizeGrant, PrizeKind, PrizePhase, ReelData, ReelState, RewardCardTable, Screen,
    SlotEvent, SlotMachine, buttons, special_pool,
};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Cursor;
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, String>;

const MAX_DUEL_FRAMES: u64 = 60_000;
const MAX_END_FRAMES: u64 = 2_000;
const MAX_SLOT_FRAMES: u64 = 3_000;
/// The result-state waits the model does not trace (DUEL-END §6): the sound
/// idle wait and the camera move are 0 as in the interpreter runs, the HUD
/// slides out by 8 per update from 0 to 0x80 (16 updates).
const TIMINGS: EndTimings = EndTimings {
    sound_idle_frames: 0,
    hud_slide_frames: 16,
    camera_wait_frames: 0,
};

struct Options {
    game: PathBuf,
    out: PathBuf,
    seed: u64,
    golden_dir: Option<PathBuf>,
    truth_dir: Option<PathBuf>,
    truth_state: Option<PathBuf>,
}

fn parse(args: impl Iterator<Item = OsString>) -> Result<Options> {
    let mut game = None;
    let mut out = None;
    let mut seed = 9u64;
    let mut golden_dir = None;
    let mut truth_dir = None;
    let mut truth_state = None;
    let mut args = args;
    while let Some(arg) = args.next() {
        let mut path = |what: &str| -> Result<PathBuf> {
            args.next()
                .map(PathBuf::from)
                .ok_or_else(|| format!("{what} requires a path"))
        };
        match arg.to_str() {
            Some("--game") => game = Some(path("--game")?),
            Some("--out") => out = Some(path("--out")?),
            Some("--golden-dir") => golden_dir = Some(path("--golden-dir")?),
            Some("--truth-dir") => truth_dir = Some(path("--truth-dir")?),
            Some("--truth-state") => truth_state = Some(path("--truth-state")?),
            Some("--seed") => {
                seed = args
                    .next()
                    .and_then(|v| v.to_str()?.parse().ok())
                    .ok_or("--seed requires a number")?
            }
            _ => return Err(format!("Unknown argument {}", arg.to_string_lossy())),
        }
    }
    Ok(Options {
        game: game.ok_or("--game <game.json> is required")?,
        out: out.ok_or("--out <new dir> is required")?,
        seed,
        golden_dir,
        truth_dir,
        truth_state,
    })
}

// ---------------------------------------------------------------- PNG I/O

fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    if path.exists() {
        return Err(format!("{} already exists", path.display()));
    }
    let image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba.to_vec(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD,
    );
    let dynamic = image.try_into_dynamic().map_err(|e| e.to_string())?;
    let mut encoded = Cursor::new(Vec::new());
    dynamic
        .write_to(
            &mut encoded,
            ImageFormat::Png
                .as_image_crate_format()
                .ok_or("PNG encoder unavailable")?,
        )
        .map_err(|e| format!("PNG encode: {e}"))?;
    std::fs::write(path, encoded.get_ref()).map_err(|e| format!("{}: {e}", path.display()))
}

/// RGB8 rows of a PNG (the software-GS dumps: plain 5-bit values shifted
/// left by 3, which is also this model's output expansion).
fn read_png_rgb(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let image = Image::from_buffer(
        &bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        false,
        ImageSampler::Default,
        RenderAssetUsages::MAIN_WORLD,
    )
    .map_err(|e| format!("{}: {e}", path.display()))?;
    let format = image.texture_descriptor.format;
    if !matches!(
        format,
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb
    ) {
        return Err(format!(
            "{}: unsupported PNG layout {format:?}",
            path.display()
        ));
    }
    let (width, height) = (image.width(), image.height());
    let data = image.data.ok_or("PNG without pixel data")?;
    let rgb = data
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    Ok((width, height, rgb))
}

fn target_rgb(target: &Target16) -> Vec<u8> {
    target
        .to_rgba8(0, 0, target.width, target.height)
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect()
}

fn target_rgba(target: &Target16) -> Vec<u8> {
    target.to_rgba8(0, 0, target.width, target.height)
}

/// Mean absolute error per channel (8-bit) and the share of exact pixels
/// over an optional rectangle (x0, y0, x1, y1).
fn compare_rgb(
    mine: &[u8],
    truth: &[u8],
    rect: Option<(usize, usize, usize, usize)>,
) -> (f64, f64) {
    let (x0, y0, x1, y1) = rect.unwrap_or((0, 0, FRAME, FRAME));
    let (mut sum, mut exact, mut n) = (0u64, 0u64, 0u64);
    for y in y0..y1 {
        for x in x0..x1 {
            let at = (y * FRAME + x) * 3;
            let d: u64 = (0..3)
                .map(|c| (i32::from(mine[at + c]) - i32::from(truth[at + c])).unsigned_abs() as u64)
                .sum();
            sum += d;
            exact += u64::from(d == 0);
            n += 1;
        }
    }
    (
        sum as f64 / (3 * n).max(1) as f64,
        exact as f64 / n.max(1) as f64,
    )
}

// ---------------------------------------------------------------- golden lists

#[derive(Deserialize)]
struct GoldenEntry {
    kind: String,
    #[serde(default)]
    prim: String,
    #[serde(default)]
    verts: Vec<Vec<serde_json::Value>>,
}

/// Compare my primitive list with the interpreter's (positions, texel
/// coordinates and colours of every vertex; the JSON's image uploads are
/// skipped). Returns (matched, mismatch descriptions).
fn compare_golden(prims: &[Primitive], golden: &[GoldenEntry]) -> (usize, Vec<String>) {
    let draws: Vec<&GoldenEntry> = golden.iter().filter(|g| g.kind == "draw").collect();
    let mut mismatches = Vec::new();
    let mut matched = 0;
    if draws.len() != prims.len() {
        mismatches.push(format!(
            "{} primitives here, {} in the list",
            prims.len(),
            draws.len()
        ));
    }
    for (i, (p, g)) in prims.iter().zip(&draws).enumerate() {
        let kind = match p.kind {
            PrimKind::Sprite => "SPRITE",
            PrimKind::TriStrip => "TRISTRIP",
        };
        if g.prim != kind {
            mismatches.push(format!("{i}: {} here, {} in the list", kind, g.prim));
            continue;
        }
        if g.verts.len() != p.verts.len() {
            mismatches.push(format!(
                "{i}: {} vertices here, {} in the list",
                p.verts.len(),
                g.verts.len()
            ));
            continue;
        }
        let textured = p.texture != TextureSource::None;
        let mut bad = Vec::new();
        for (k, (v, gv)) in p.verts.iter().zip(&g.verts).enumerate() {
            let f = |j: usize| {
                gv.get(j)
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(f64::NAN)
            };
            let c = |j: usize| gv.get(j).and_then(serde_json::Value::as_u64).unwrap_or(999) as u32;
            let near = |a: f32, b: f64| (f64::from(a) - b).abs() < 0.01;
            if !near(v.x, f(0)) || !near(v.y, f(1)) {
                bad.push(format!(
                    "v{k} xy ({}, {}) vs ({}, {})",
                    v.x,
                    v.y,
                    f(0),
                    f(1)
                ));
            }
            if textured && (!near(v.u, f(3)) || !near(v.v, f(4))) {
                bad.push(format!(
                    "v{k} uv ({}, {}) vs ({}, {})",
                    v.u,
                    v.v,
                    f(3),
                    f(4)
                ));
            }
            let rgba = [c(5), c(6), c(7), c(8)];
            if v.rgba.map(u32::from) != rgba {
                bad.push(format!("v{k} rgba {:?} vs {:?}", v.rgba, rgba));
            }
        }
        if bad.is_empty() {
            matched += 1;
        } else {
            mismatches.push(format!("{i} ({kind}): {}", bad.join("; ")));
        }
    }
    (matched, mismatches)
}

// ---------------------------------------------------------------- the run

struct Granter<'a> {
    block: &'a mut CardBlock,
    art: &'a [roses_core::card_block::AlternateArt],
    log: Vec<String>,
}

impl PrizeGrant for Granter<'_> {
    fn grant(&mut self, prize: Prize, rng: &mut u64) {
        let ok = self.block.grant(prize.card, None, rng);
        if prize.kind == PrizeKind::GetSpecialCard {
            self.block.cycle_art(prize.card, self.art);
        }
        self.log.push(format!(
            "{:?} card {} (order {}) granted: {ok}",
            prize.kind, prize.card, prize.order
        ));
    }
}

#[derive(Deserialize)]
struct TruthState {
    strips: [Vec<u16>; 3],
    popup_pos: [i16; 3],
    popup_scroll: [i16; 3],
    popup_card: u16,
}

fn main() {
    if let Err(error) = run(std::env::args_os().skip(1)) {
        eprintln!("roses-duel-end-check: {error}");
        std::process::exit(2);
    }
}

fn run(args: impl Iterator<Item = OsString>) -> Result<()> {
    let options = parse(args)?;
    if options.out.exists() {
        return Err(format!("{} already exists", options.out.display()));
    }
    std::fs::create_dir_all(&options.out).map_err(|e| e.to_string())?;
    let out = |name: &str| options.out.join(name);
    let mut report = serde_json::Map::new();

    let import = GameImport::open(&options.game)?;
    let duel_end_dir = import
        .duel_end
        .clone()
        .ok_or("the game import has no duel-end part (re-run roses import-game)")?;
    let files = roses_game::duel_end_screens::load_duel_end_import(&duel_end_dir)?;
    let data = duel_play::load(&import.duel)?;
    let leaders = [
        data.first_duel.sides[0].leader,
        data.first_duel.sides[1].leader,
    ];
    let mut assets = EndAssets::new(
        &data.executable,
        &data.messages,
        &data.texsys,
        &data.picmini,
        leaders,
        files.pictures.clone(),
    )?;
    let slot_textures = SlotTextures::new(&files.texetc)?;
    let thresholds =
        roses_assets::parse_pal_rank_thresholds(&data.executable).map_err(|e| e.to_string())?;
    let flags =
        roses_assets::parse_pal_reward_flags(&data.executable).map_err(|e| e.to_string())?;
    let reward = RewardCardTable::from_record_flag_bytes(flags.flag_bytes.iter().copied());
    let pool = special_pool(&reward);

    // ---- the golden primitive lists of the banner drawer
    if let Some(dir) = &options.golden_dir {
        let block = CardBlock::empty(1);
        // (list, banner, c9c, press-icon cel: the interpreter runs left the
        // global counter at cel 1, the popup savestate at cel 2)
        let cases: [(&str, BannerKind, i16, u8); 6] = [
            (
                "banner-f66.json",
                BannerKind::Result { code: 2, side: 0 },
                64,
                1,
            ),
            (
                "loss-f66.json",
                BannerKind::Result { code: 3, side: 0 },
                64,
                1,
            ),
            (
                "promo-f140.json",
                BannerKind::Rank {
                    demotion: false,
                    side: 0,
                    card: 221,
                },
                64,
                1,
            ),
            (
                "demo-f140.json",
                BannerKind::Rank {
                    demotion: true,
                    side: 0,
                    card: 392,
                },
                64,
                1,
            ),
            (
                "end-popup1-popup-prims.json",
                BannerKind::Prize {
                    special: false,
                    card: 820,
                },
                0,
                2,
            ),
            (
                "popup-special-popup-prims.json",
                BannerKind::Prize {
                    special: true,
                    card: 820,
                },
                0,
                2,
            ),
        ];
        let mut golden_report = serde_json::Map::new();
        for (name, kind, slide, cel) in cases {
            let path = dir.join(name);
            if !path.is_file() {
                golden_report.insert(name.into(), serde_json::json!({"missing": true}));
                continue;
            }
            let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
            let golden: Vec<GoldenEntry> =
                serde_json::from_slice(&bytes).map_err(|e| format!("{name}: {e}"))?;
            let (prims, _) = banner_primitives(&mut assets, kind, slide, cel, &block)?;
            let (matched, mismatches) = compare_golden(&prims, &golden);
            println!(
                "golden {name}: {matched} of {} primitives match{}",
                prims.len(),
                if mismatches.is_empty() {
                    String::new()
                } else {
                    format!("; {} mismatches", mismatches.len())
                }
            );
            for m in mismatches.iter().take(8) {
                println!("  {m}");
            }
            golden_report.insert(
                name.into(),
                serde_json::json!({"primitives": prims.len(), "matched": matched, "mismatches": mismatches}),
            );
        }
        report.insert("golden".into(), serde_json::Value::Object(golden_report));
    }

    // ---- the truth frames of the slot machine (a known reel state)
    if let (Some(dir), Some(state_path)) = (&options.truth_dir, &options.truth_state) {
        let state: TruthState =
            serde_json::from_slice(&std::fs::read(state_path).map_err(|e| e.to_string())?)
                .map_err(|e| format!("{}: {e}", state_path.display()))?;
        let block = CardBlock::empty(1);
        let mut rng = 1u64;
        let data = ReelData {
            strip: state.strips[0].clone(),
            random_slip: true,
            enabled_lines: [true, false, false, false, false],
        };
        let (mut machine, _) =
            SlotMachine::new(&data, pool.clone(), &mut rng).map_err(|e| e.to_string())?;
        for r in 0..3 {
            machine.reels[r].strip = state.strips[r].clone();
        }
        let mut truth_report = serde_json::Map::new();
        let mut target = Target16 {
            width: FRAME,
            height: FRAME,
            pixels: vec![0; FRAME * FRAME],
        };
        // Stopped reels with the GET CARD popup (screen 3, phase 2).
        let mut stopped = machine.clone();
        for r in 0..3 {
            let reel = &mut stopped.reels[r];
            reel.state = ReelState::Stopped;
            reel.pos = state.popup_pos[r];
            reel.scroll = state.popup_scroll[r];
            reel.speed = 0;
            reel.card = reel.rows()[1];
        }
        stopped.screen = Screen::Prizes;
        stopped.prizes.phase = PrizePhase::Popup;
        stopped.prizes.card = state.popup_card;
        stopped.live_lines = 0;
        for (name, popup, dither_name, dither) in [
            (
                "truth-f00541.png",
                Some(PrizePopup {
                    kind: PrizeKind::GetCard,
                    card: state.popup_card,
                }),
                "none",
                None,
            ),
            (
                "truth-f00541.png",
                Some(PrizePopup {
                    kind: PrizeKind::GetCard,
                    card: state.popup_card,
                }),
                "hud",
                Some(assets.tables.dimx),
            ),
            // f00589 is darker overall (the mode fade back to the map): a
            // reference for the layout only.
            (
                "truth-f00589.png",
                None,
                "hud-fading",
                Some(assets.tables.dimx),
            ),
        ] {
            let truth = dir.join(name);
            if !truth.is_file() {
                continue;
            }
            let mut shown = stopped.clone();
            if popup.is_none() {
                shown.screen = Screen::Done;
            }
            // The popup frame's icon cel is the savestate's (cel 2).
            let notes = render_slot_screen(
                &shown,
                popup,
                &mut assets,
                &slot_textures,
                &block,
                2,
                dither,
                &mut target,
            )?;
            let mine = target_rgb(&target);
            let (_, _, t) = read_png_rgb(&truth)?;
            let (mae, exact) = compare_rgb(&mine, &t, None);
            let (mae_reels, _) = compare_rgb(&mine, &t, Some((115, 55, 397, 457)));
            println!(
                "truth {name} (dither {dither_name}): MAE {mae:.3}/255, exact {:.1}%, reel area MAE {mae_reels:.3}",
                exact * 100.0
            );
            let key = format!("{}-{dither_name}", name.trim_end_matches(".png"));
            write_png(
                &out(&format!("mine-{key}.png")),
                FRAME as u32,
                FRAME as u32,
                &target_rgba(&target),
            )?;
            truth_report.insert(key, serde_json::json!({"mae": mae, "exact": exact, "reel_area_mae": mae_reels, "notes": notes}));
        }
        // Spinning: the frame shows reel 0 already stopped (its final rows)
        // while reels 1 and 2 still spin in step; the frame since the spin
        // started is not recorded, so search the model's own spin (no
        // presses, reel 0 forced to its stopped state) for the closest frame.
        let truth = dir.join("truth-f00445.png");
        if truth.is_file() {
            let (_, _, t) = read_png_rgb(&truth)?;
            let mut spin = machine.clone();
            let mut collector: Vec<Prize> = Vec::new();
            let mut best: Option<(f64, u64, f64, i16, i16)> = None;
            let dimx = assets.tables.dimx;
            for frame in 0..400u64 {
                if frame > 0 {
                    spin.step(0, &mut rng, &mut collector);
                }
                let mut shown = spin.clone();
                shown.reels[0] = stopped.reels[0].clone();
                let icon = ((frame / 5) % 3) as u8;
                render_slot_screen(
                    &shown,
                    None,
                    &mut assets,
                    &slot_textures,
                    &block,
                    icon,
                    Some(dimx),
                    &mut target,
                )?;
                let mine = target_rgb(&target);
                let (mae, exact) = compare_rgb(&mine, &t, None);
                if best.is_none_or(|b| mae < b.0) {
                    best = Some((mae, frame, exact, shown.reels[1].pos, shown.reels[1].scroll));
                    write_png(
                        &out(&format!("mine-truth-f00445-f{frame}.png")),
                        FRAME as u32,
                        FRAME as u32,
                        &target_rgba(&target),
                    )?;
                }
            }
            if let Some((mae, frame, exact, pos, scroll)) = best {
                println!(
                    "truth truth-f00445.png: best model spin frame {frame} (reels 1-2 pos {pos} scroll {scroll}, reel 0 stopped): MAE {mae:.3}/255, exact {:.1}%",
                    exact * 100.0
                );
                truth_report.insert("truth-f00445".into(), serde_json::json!({"mae": mae, "exact": exact, "model_frame": frame, "pos": pos, "scroll": scroll}));
            }
        }
        report.insert("truth".into(), serde_json::Value::Object(truth_report));
    }

    // ---- the scripted run: duel (computer vs computer) → result → reels → map
    let mut session = duel_play::start(&data, options.seed, true)?;
    let mut frames = 0u64;
    while session.duel.outcome.is_none() {
        duel_play::tick(&mut session, &data, 0)?;
        frames += 1;
        if frames > MAX_DUEL_FRAMES {
            return Err("the duel did not end".into());
        }
    }
    let outcome = session.duel.outcome.unwrap();
    println!(
        "duel: seed {}, {frames} updates, winner {:?} ({:?})",
        options.seed, outcome.winner, outcome.reason
    );
    report.insert("duel".into(), serde_json::json!({"seed": options.seed, "updates": frames, "winner": outcome.winner, "reason": format!("{:?}", outcome.reason)}));

    let mut rng = options.seed;
    let decks = &data.first_duel.sides;
    let ranks = roses_assets::parse_pal_decks(&data.executable).map_err(|e| e.to_string())?;
    let rank_of = |record: usize| {
        ranks
            .decks
            .get(record)
            .map_or(0, |d| u16::from(d.leader_rank_raw))
    };
    let player_key = next_rand31(&mut rng) | 1;
    let mut blocks = [
        new_game_block(
            player_key,
            &decks[0].cards,
            decks[0].leader,
            rank_of(decks[0].deck_record),
            &thresholds,
            &mut rng,
        ),
        new_game_block(
            computer_owner_key(decks[1].deck_record),
            &decks[1].cards,
            decks[1].leader,
            rank_of(decks[1].deck_record),
            &thresholds,
            &mut rng,
        ),
    ];
    let before = blocks.clone();
    // S+0x344 bit 7 (Destiny Draw announced) of each side, from the duel.
    let special_draw_announced = [
        session.duel.sides[0].destiny & 0x80 != 0,
        session.duel.sides[1].destiny & 0x80 != 0,
    ];
    let setup = DuelEndSetup::from_duel(
        &session.duel,
        &data.catalogue,
        data.first_duel.board_map as u8,
        true,
        special_draw_announced,
        reward,
        TIMINGS,
    )
    .map_err(|e| e.to_string())?;
    let mut end = DuelEnd::new(setup, thresholds);
    let mut state_log: Vec<(u64, u8)> = Vec::new();
    let mut last_state = 0u8;
    let mut in_state_since = 0u64;
    let mut end_events = Vec::new();
    let mut finished = None;
    let mut captured: BTreeMap<String, bool> = BTreeMap::new();
    let render_banner = |assets: &mut EndAssets,
                         view: &roses_core::duel_end::EndView,
                         blocks: &[CardBlock; 2],
                         name: &str|
     -> Result<()> {
        let kind = match view.banner {
            2..=6 => BannerKind::Result {
                code: view.banner,
                side: 0,
            },
            9 | 0xA => match view.rank_entry {
                Some(e) => BannerKind::Rank {
                    demotion: e.demotion,
                    side: e.side,
                    card: e.card,
                },
                None => return Ok(()),
            },
            _ => return Ok(()),
        };
        let (mut prims, picture) = banner_primitives(assets, kind, view.slide, 1, &blocks[0])?;
        if let Some(d) = darkening_primitive(view.darken) {
            prims.insert(0, d);
        }
        let (layer, notes) = render_banner_layer(assets, prims, picture);
        for n in notes {
            println!("  note: {n}");
        }
        // Over black: the premultiplied colour is the frame.
        let rgba: Vec<u8> = layer
            .chunks_exact(4)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect();
        write_png(&out(name), FRAME as u32, FRAME as u32, &rgba)
    };
    for frame in 0..MAX_END_FRAMES {
        let view = end.view();
        if view.state != last_state {
            state_log.push((frame, view.state));
            last_state = view.state;
            in_state_since = frame;
        }
        let press = matches!(view.state, 0x5C | 0x61) && frame >= in_state_since + 2;
        if view.state == 0x5C && !captured.contains_key("banner-rest") {
            captured.insert("banner-rest".into(), true);
            render_banner(&mut assets, &view, &blocks, "banner-rest.png")?;
        }
        if view.slide == 32 && view.state == 0x59 && !captured.contains_key("banner-slide") {
            captured.insert("banner-slide".into(), true);
            render_banner(&mut assets, &view, &blocks, "banner-slide.png")?;
        }
        if view.state == 0x5D && view.darken == 58 && !captured.contains_key("banner-out") {
            captured.insert("banner-out".into(), true);
            render_banner(&mut assets, &view, &blocks, "banner-out-c9c93.png")?;
        }
        if view.state == 0x61
            && !captured.contains_key(&format!(
                "rank-{}",
                end.view().rank_entry.map_or(0, |e| e.card)
            ))
        {
            captured.insert(
                format!("rank-{}", view.rank_entry.map_or(0, |e| e.card)),
                true,
            );
            render_banner(
                &mut assets,
                &view,
                &blocks,
                &format!("rank-{}.png", view.rank_entry.map_or(0, |e| e.card)),
            )?;
        }
        let events = end.step(if press { pad::CROSS } else { 0 }, &mut blocks);
        // The main loop's two LCG calls per frame (DUEL-END §5.5).
        next_rand31(&mut rng);
        next_rand31(&mut rng);
        for e in events {
            if let EndEvent::Finished(o) = &e {
                finished = Some(o.clone());
            }
            end_events.push(format!("f{frame}: {e:?}"));
        }
        if finished.is_some() {
            break;
        }
    }
    println!("result states: {:?}", state_log);
    for e in &end_events {
        println!("  {e}");
    }
    let finished = finished.ok_or("the result sequence did not finish")?;
    report.insert(
        "result".into(),
        serde_json::json!({"states": state_log, "events": end_events, "frames": end.frame()}),
    );

    let mut won = false;
    let prizes_log: Vec<String>;
    match finished {
        EndOutcome::SlotMachine(reels) => {
            won = true;
            println!(
                "slot machine: strip {:?}, lines {:?}, random slip {}",
                reels.strip, reels.enabled_lines, reels.random_slip
            );
            let winner = usize::from(outcome.winner.unwrap_or(0));
            let (mut machine, events) =
                SlotMachine::new(&reels, pool.clone(), &mut rng).map_err(|e| e.to_string())?;
            let mut slot_events: Vec<String> =
                events.iter().map(|e| format!("f0: {e:?}")).collect();
            let art = assets.art.clone();
            let mut granter = Granter {
                block: &mut blocks[winner],
                art: &art,
                log: Vec::new(),
            };
            let mut last_prize = None;
            let mut popup_since = None;
            let mut done = false;
            let mut target = Target16 {
                width: FRAME,
                height: FRAME,
                pixels: vec![0; FRAME * FRAME],
            };
            let mut shots = 0;
            for frame in 1..MAX_SLOT_FRAMES {
                let mut pressed = 0u8;
                match frame {
                    60 => pressed = buttons::SQUARE,
                    80 => pressed = buttons::TRIANGLE,
                    100 => pressed = buttons::CIRCLE,
                    _ => {}
                }
                if let Some(since) = popup_since {
                    if frame >= since + 2 {
                        pressed = buttons::CROSS;
                    }
                }
                let events = machine.step(pressed, &mut rng, &mut granter);
                next_rand31(&mut rng);
                next_rand31(&mut rng);
                for e in &events {
                    if let SlotEvent::Prize(p) = e {
                        last_prize = Some((p.kind, p.card));
                    }
                    slot_events.push(format!("f{frame}: {e:?}"));
                }
                let popup = current_popup(&machine, last_prize);
                match (popup, popup_since) {
                    (Some(_), None) => popup_since = Some(frame),
                    (None, Some(_)) => popup_since = None,
                    _ => {}
                }
                let shot = match (frame, popup, popup_since) {
                    (30, _, _) => Some("slot-spinning-f30.png".to_string()),
                    (_, Some(p), Some(since)) if since == frame => {
                        Some(format!("slot-popup-{}-{:?}.png", p.card, p.kind))
                    }
                    _ if machine.screen == Screen::Prizes
                        && machine.prizes.phase == PrizePhase::Delay
                        && shots < 8
                        && !captured.contains_key("slot-stopped") =>
                    {
                        captured.insert("slot-stopped".into(), true);
                        Some("slot-stopped.png".to_string())
                    }
                    _ => None,
                };
                if let Some(name) = shot {
                    shots += 1;
                    let dimx = assets.tables.dimx;
                    let notes = render_slot_screen(
                        &machine,
                        popup,
                        &mut assets,
                        &slot_textures,
                        &blocks_view(&granter),
                        ((frame / 5) % 3) as u8,
                        Some(dimx),
                        &mut target,
                    )?;
                    for n in notes {
                        println!("  note: {n}");
                    }
                    write_png(
                        &out(&name),
                        FRAME as u32,
                        FRAME as u32,
                        &target_rgba(&target),
                    )?;
                }
                if machine.finished {
                    done = true;
                    println!("slot machine finished at frame {frame}");
                    break;
                }
            }
            if !done {
                return Err("the slot machine did not finish".into());
            }
            prizes_log = granter.log.clone();
            for e in &slot_events {
                println!("  {e}");
            }
            for g in &prizes_log {
                println!("  {g}");
            }
            report.insert("slot".into(), serde_json::json!({"strip": reels.strip, "events": slot_events, "grants": prizes_log}));
        }
        EndOutcome::ReturnToMap { human_won } => {
            won = human_won;
            prizes_log = Vec::new();
            println!("no slot machine: back to the map (won {human_won})");
        }
        EndOutcome::Return => {
            prizes_log = Vec::new();
            println!("no human side: plain return");
        }
    }
    let _ = &prizes_log;
    // Block changes of the player's side.
    let mut changes = Vec::new();
    for card in 0..roses_core::card_block::BLOCK_CARDS as u16 {
        let (a, b) = (before[0].record(card), blocks[0].record(card));
        if let (Some(a), Some(b)) = (a, b) {
            if a != b {
                changes.push(format!(
                    "card {card}: exp {} -> {}, rank {} -> {}, copies {} -> {}",
                    a.experience,
                    b.experience,
                    a.stored_rank(),
                    b.stored_rank(),
                    a.copies(),
                    b.copies()
                ));
            }
        }
    }
    println!("player block changes: {}", changes.len());
    for c in &changes {
        println!("  {c}");
    }
    println!("recent list: {:?}", blocks[0].recent);
    report.insert(
        "player_block".into(),
        serde_json::json!({"changes": changes, "recent": blocks[0].recent}),
    );

    // ---- the campaign map's return
    let map = load_map(&import.campaign_map.join("manifest.json"))?;
    let mut sim = MapModel::after_duel(map.data.clone(), won)?;
    println!("map: {} (event {})", sim.caption(), sim.save.event);
    let mut map_events = Vec::new();
    for _ in 0..200 {
        for e in sim.step(Pad::NONE) {
            map_events.push(format!("{e:?}"));
        }
    }
    println!(
        "map after 200 updates: {} ({} events, first {:?})",
        sim.caption(),
        map_events.len(),
        map_events.first()
    );
    report.insert("map".into(), serde_json::json!({"won": won, "event": sim.save.event, "caption": sim.caption(), "events": map_events.len()}));

    std::fs::write(
        out("report.json"),
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!("report: {}", out("report.json").display());
    Ok(())
}

fn blocks_view<'a>(g: &'a Granter<'_>) -> &'a CardBlock {
    g.block
}
