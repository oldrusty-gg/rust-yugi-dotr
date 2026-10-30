use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

fn fixture() -> Manifest {
    Manifest {
        schema_version: MANIFEST_SCHEMA.into(),
        source: Source {
            profile: "dotr-europe-sles-51579".into(),
            disc_sha256: "0".repeat(64),
            executable_sha256: "1".repeat(64),
        },
        canvas: Canvas {
            width: 16,
            height: 16,
            aspect_width: 4,
            aspect_height: 3,
        },
        assets: vec![
            Asset {
                id: "pixel".into(),
                kind: AssetKind::Png,
                path: "pixel.png".into(),
                sha256: "2".repeat(64),
                bytes: 1,
            },
            Asset {
                id: "movie".into(),
                kind: AssetKind::Video,
                path: "intro.mp4".into(),
                sha256: "3".repeat(64),
                bytes: 1,
            },
        ],
        initial_stage: "movie".into(),
        stages: vec![
            Stage::Movie {
                id: "movie".into(),
                asset: "movie".into(),
                on_end: "title".into(),
                skip_buttons: vec![Button::Start],
                on_skip: Some("menu".into()),
            },
            Stage::Screen {
                id: "title".into(),
                layers: vec![],
                music: None,
                advance_buttons: vec![Button::Start],
                duration_ms: None,
                next: "menu".into(),
            },
            Stage::Menu {
                id: "menu".into(),
                layers: vec![],
                music: None,
                initial_selection: 0,
                confirm_buttons: vec![Button::Cross],
                cancel_target: None,
                wrap_navigation: false,
                entries: vec![
                    MenuEntry {
                        id: "first".into(),
                        label: "Synthetic unavailable branch".into(),
                        normal_layers: vec![],
                        selected_layers: vec![],
                        action: MenuAction::Unavailable,
                        campaign_side: None,
                    },
                    MenuEntry {
                        id: "second".into(),
                        label: "Synthetic return".into(),
                        normal_layers: vec![],
                        selected_layers: vec![],
                        action: MenuAction::Stage {
                            target: "title".into(),
                        },
                        campaign_side: None,
                    },
                ],
            },
        ],
        layer_sets: Default::default(),
    }
}
fn add_name_entry(manifest: &mut Manifest) {
    let glyph = |display_code, stored_code| NameEntryGlyph {
        display_code,
        stored_code,
        asset: "pixel".into(),
        source: [0, 0, 1, 1],
    };
    manifest.stages.push(Stage::NameEntry {
        id: "name".into(),
        layers: vec![],
        music: None,
        config: NameEntryConfig {
            rows: 2,
            columns: 2,
            max_length: 2,
            pages: vec![
                NameEntryPage {
                    layers: vec![],
                    cells: vec![
                        glyph(10, 100),
                        glyph(11, 101),
                        glyph(12, 102),
                        glyph(13, 103),
                    ],
                },
                NameEntryPage {
                    layers: vec![],
                    cells: vec![
                        glyph(20, 200),
                        glyph(21, 201),
                        glyph(22, 202),
                        glyph(23, 203),
                    ],
                },
            ],
            cursor: Layer {
                asset: "pixel".into(),
                source: Some([0, 0, 1, 1]),
                destination: [0, 0, 1, 1],
                tint: [255; 3],
                opacity: 255,
                flip_x: false,
                filter: Sampling::Nearest,
            },
            cursor_rects: vec![[0, 0, 1, 1], [2, 0, 1, 1], [0, 2, 1, 1], [2, 2, 1, 1]],
            name_positions: vec![[8, 8, 1, 1], [10, 8, 1, 1]],
            name_tint: [255; 3],
            name_shadow: None,
        },
        on_complete: StoryEnd::Unsupported {
            message: "Synthetic next branch is pending.".into(),
        },
    });
    manifest.initial_stage = "name".into();
}

#[test]
fn name_entry_wraps_and_commits_original_cell_codes_only() {
    let mut manifest = fixture();
    add_name_entry(&mut manifest);
    let mut model = Model::new(manifest).unwrap();
    assert_eq!(model.input(Button::Start), Outcome::None);
    assert_eq!(model.input(Button::Circle), Outcome::None);
    assert!(model.player_name().is_none());
    model.input(Button::Up);
    model.input(Button::Left);
    model.input(Button::L1);
    assert_eq!(
        (
            model.name_entry().unwrap().page(),
            model.name_entry().unwrap().row(),
            model.name_entry().unwrap().column()
        ),
        (1, 1, 1)
    );
    model.input(Button::Square);
    assert_eq!(model.name_entry().unwrap().glyphs()[0].stored_code, 203);
    model.input(Button::Down);
    model.input(Button::Right);
    model.input(Button::R1);
    assert_eq!(
        (
            model.name_entry().unwrap().page(),
            model.name_entry().unwrap().row(),
            model.name_entry().unwrap().column()
        ),
        (0, 0, 0)
    );
    model.input(Button::Cross);
    assert_eq!(model.input(Button::Cross), Outcome::None);
    assert!(model.player_name().is_none());
    model.input(Button::Circle);
    assert_eq!(model.name_entry().unwrap().glyphs().len(), 1);
    assert!(matches!(
        model.input(Button::Start),
        Outcome::Unsupported(_)
    ));
    let name = model.player_name().unwrap().clone();
    assert_eq!(
        name.glyphs,
        vec![NameCharacter {
            page: 1,
            row: 1,
            column: 1,
            display_code: 23,
            stored_code: 203
        }]
    );
    assert_eq!(model.input(Button::Circle), Outcome::None);
    assert_eq!(model.input(Button::Cross), Outcome::None);
    assert_eq!(model.input(Button::Start), Outcome::None);
    assert_eq!(model.player_name(), Some(&name));
}

#[test]
fn name_entry_preserves_committed_name_across_explicit_stage_transition() {
    let mut manifest = fixture();
    add_name_entry(&mut manifest);
    if let Stage::NameEntry { on_complete, .. } = manifest.stages.last_mut().unwrap() {
        *on_complete = StoryEnd::Stage {
            target: "title".into(),
        };
    }
    let mut model = Model::new(manifest).unwrap();
    model.input(Button::Cross);
    assert_eq!(model.input(Button::Start), Outcome::StageChanged);
    assert_eq!(model.stage().id(), "title");
    assert!(model.name_entry().is_none());
    assert_eq!(model.player_name().unwrap().glyphs[0].stored_code, 100);
}

#[test]
fn name_entry_rejects_incomplete_grids_flags_and_out_of_bounds_rectangles() {
    let mut original = fixture();
    add_name_entry(&mut original);
    let edits: Vec<fn(&mut NameEntryConfig)> = vec![
        |c| c.rows = 0,
        |c| c.columns = 17,
        |c| c.max_length = 13,
        |c| c.pages.clear(),
        |c| {
            c.pages[0].cells.pop();
        },
        |c| c.pages[0].cells[0].stored_code = 0x8001,
        |c| c.pages[0].cells[0].display_code = 0x2000,
        |c| c.pages[0].cells[0].source[2] = 0,
        |c| c.pages[0].cells[0].asset = "movie".into(),
        |c| {
            c.cursor_rects.pop();
        },
        |c| c.cursor_rects[0][0] = u32::MAX,
        |c| {
            c.name_positions.pop();
        },
        |c| c.name_positions[0][2] = 17,
        |c| {
            c.name_shadow = Some(NameShadow {
                offset: [i32::MIN, 1],
                tint: [0; 3],
                opacity: 255,
            })
        },
    ];
    for edit in edits {
        let mut manifest = original.clone();
        if let Stage::NameEntry { config, .. } = manifest.stages.last_mut().unwrap() {
            edit(config);
        }
        assert!(validate_manifest(&manifest).is_err());
    }
    let roundtrip: Manifest =
        serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
    assert!(validate_manifest(&roundtrip).is_ok());
}

#[test]
fn music_continues_only_for_identical_nonempty_playback_configuration() {
    let music = Music {
        asset: "synthetic".into(),
        looped: true,
        loop_start_frame: Some(4),
    };
    assert!(music_continues(Some(&music), Some(&music.clone())));
    assert!(!music_continues(None, None));
    assert!(!music_continues(Some(&music), None));
    assert!(!music_continues(None, Some(&music)));
    for changed in [
        Music {
            asset: "other".into(),
            ..music.clone()
        },
        Music {
            loop_start_frame: Some(5),
            ..music.clone()
        },
        Music {
            looped: false,
            loop_start_frame: None,
            ..music.clone()
        },
    ] {
        assert!(!music_continues(Some(&music), Some(&changed)));
    }
}

#[test]
fn name_entry_renders_cursor_and_entered_glyphs_and_checks_loaded_crop_bounds() {
    let temp = Temp::new();
    let mut manifest = write_fixture(&temp.0);
    add_name_entry(&mut manifest);
    if let Stage::NameEntry { config, .. } = manifest.stages.last_mut().unwrap() {
        config.name_shadow = Some(NameShadow {
            offset: [1, 1],
            tint: [0, 255, 0],
            opacity: 255,
        });
    }
    let path = temp.0.join("manifest.json");
    std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    {
        let loaded = Loaded::open(&path).unwrap();
        let mut model = Model::new(manifest.clone()).unwrap();
        let frame = loaded.render(&model).unwrap();
        assert_eq!(&frame[..4], &[50, 100, 200, 255]);
        assert_eq!(
            &frame[(8 * 16 + 8) * 4..(8 * 16 + 8) * 4 + 4],
            &[0, 0, 0, 255]
        );
        model.input(Button::Cross);
        model.input(Button::Right);
        let frame = loaded.render(&model).unwrap();
        assert_eq!(&frame[..4], &[0, 0, 0, 255]);
        assert_eq!(&frame[8..12], &[50, 100, 200, 255]);
        assert_eq!(
            &frame[(8 * 16 + 8) * 4..(8 * 16 + 8) * 4 + 4],
            &[50, 100, 200, 255]
        );
        assert_eq!(
            &frame[(9 * 16 + 9) * 4..(9 * 16 + 9) * 4 + 4],
            &[0, 100, 0, 255]
        );
        model.name_entry.as_mut().unwrap().row = 2;
        assert!(loaded.render(&model).is_err());
    }
    if let Stage::NameEntry { config, .. } = manifest.stages.last_mut().unwrap() {
        config.pages[1].cells[3].source[2] = 2;
    }
    std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert!(Loaded::open(&path)
        .err()
        .unwrap()
        .contains("Name glyph crop exceeds"));
}
#[test]
fn flow_follows_explicit_targets_and_buttons() {
    let mut model = Model::new(fixture()).unwrap();
    assert_eq!(model.input(Button::Cross), Outcome::None);
    assert_eq!(model.video_ended(), Outcome::StageChanged);
    assert_eq!(model.stage().id(), "title");
    assert_eq!(model.elapsed(1_000_000), Outcome::None);
    assert_eq!(model.input(Button::Cross), Outcome::None);
    assert_eq!(model.input(Button::Start), Outcome::StageChanged);
    assert_eq!(model.stage().id(), "menu");
    assert!(matches!(
        model.input(Button::Cross),
        Outcome::Unavailable(_)
    ));
    assert_eq!(model.stage().id(), "menu");
    assert_eq!(model.input(Button::Up), Outcome::None);
    assert_eq!(model.input(Button::Down), Outcome::Repaint);
    assert_eq!(model.input(Button::Down), Outcome::None);
    assert_eq!(model.input(Button::Circle), Outcome::None);
    assert_eq!(model.input(Button::Cross), Outcome::StageChanged);
    assert_eq!(model.stage().id(), "title");
}
#[test]
fn a_preset_menu_cursor_applies_once_on_the_next_entry() {
    let mut model = Model::new(fixture()).unwrap();
    model.preset_selection("menu", 1);
    model.jump_to("menu").unwrap();
    assert_eq!(model.selection, 1);
    model.jump_to("title").unwrap();
    model.jump_to("menu").unwrap();
    assert_eq!(model.selection, 0);
    // An entry beyond the menu is ignored.
    model.preset_selection("menu", 7);
    model.jump_to("menu").unwrap();
    assert_eq!(model.selection, 0);
}
#[test]
fn a_locked_menu_cursor_ignores_up_and_down() {
    // The side choice after an ending (SAVE[1] or SAVE[2] set, FLOW C5).
    let mut model = Model::new(fixture()).unwrap();
    model.preset_selection("menu", 1);
    model.lock_selection("menu");
    model.jump_to("menu").unwrap();
    assert!(model.selection_locked());
    assert_eq!(model.input(Button::Up), Outcome::None);
    assert_eq!(model.selection, 1);
    model.jump_to("title").unwrap();
    assert!(!model.selection_locked());
}
#[test]
fn movie_skip_is_independent_from_natural_end() {
    let mut model = Model::new(fixture()).unwrap();
    model.input(Button::Start);
    assert_eq!(model.stage().id(), "menu");
    assert_eq!(model.video_ended(), Outcome::None);
}
#[test]
fn timer_has_exact_explicit_boundary() {
    let mut manifest = fixture();
    manifest.initial_stage = "title".into();
    if let Stage::Screen {
        duration_ms,
        advance_buttons,
        ..
    } = &mut manifest.stages[1]
    {
        *duration_ms = Some(500);
        advance_buttons.clear();
    }
    let mut model = Model::new(manifest).unwrap();
    assert_eq!(model.elapsed(499), Outcome::None);
    assert_eq!(model.elapsed(500), Outcome::StageChanged);
    assert_eq!(model.stage().id(), "menu");
}
#[test]
fn validation_rejects_ambiguous_or_unbounded_data() {
    let mut m = fixture();
    m.assets[0].path = "../pixel.png".into();
    assert!(validate_manifest(&m).is_err());
    m = fixture();
    m.assets[0].path = "pixel.png:stream".into();
    assert!(validate_manifest(&m).is_err());
    m = fixture();
    m.assets[0].sha256 = "0".repeat(63);
    assert!(validate_manifest(&m).is_err());
    m = fixture();
    m.stages.push(m.stages[0].clone());
    assert!(validate_manifest(&m).is_err());
    m = fixture();
    if let Stage::Movie { on_end, .. } = &mut m.stages[0] {
        *on_end = "missing".into();
    }
    assert!(validate_manifest(&m).is_err());
    m = fixture();
    if let Stage::Movie { on_skip, .. } = &mut m.stages[0] {
        *on_skip = None;
    }
    assert!(validate_manifest(&m).is_err());
    m = fixture();
    if let Stage::Screen { layers, .. } = &mut m.stages[1] {
        layers.push(Layer {
            asset: "pixel".into(),
            source: None,
            destination: [u32::MAX, 0, 16, 16],
            tint: [255; 3],
            opacity: 255,
            flip_x: false,
            filter: Sampling::Linear,
        });
    }
    assert!(validate_manifest(&m).is_err());
    let mut json = serde_json::to_value(fixture()).unwrap();
    json["unrecognized"] = true.into();
    assert!(serde_json::from_value::<Manifest>(json).is_err());
}
#[test]
fn aspect_is_explicit_and_centered() {
    assert_eq!(fit_rect(1920, 1080, 4, 3), [240, 0, 1440, 1080]);
    assert_eq!(fit_rect(640, 600, 4, 3), [0, 60, 640, 480]);
    assert_eq!(fit_rect(0, 0, 4, 3), [0, 0, 0, 0]);
}
#[test]
fn interpolation_uses_premultiplied_alpha_and_respects_crop() {
    let image = Image {
        width: 2,
        height: 1,
        rgba: vec![255, 0, 0, 255, 0, 0, 255, 0],
    };
    let mut frame = vec![0; 3 * 4];
    let layer = Layer {
        asset: "synthetic".into(),
        source: None,
        destination: [0, 0, 3, 1],
        tint: [255; 3],
        opacity: 255,
        flip_x: false,
        filter: Sampling::Linear,
    };
    composite(&mut frame, 3, &image, &layer);
    assert_eq!(&frame[4..7], &[128, 0, 0]);
    let cropped = Layer {
        source: Some([0, 0, 1, 1]),
        ..layer
    };
    composite(&mut frame, 3, &image, &cropped);
    assert!(frame.chunks_exact(4).all(|p| p[..3] == [255, 0, 0]));
}
#[test]
fn mirrored_sampling_swaps_opaque_pixels() {
    let image = Image {
        width: 2,
        height: 1,
        rgba: vec![255, 0, 0, 255, 0, 0, 255, 255],
    };
    let mut frame = vec![0; 8];
    let layer = Layer {
        asset: "synthetic".into(),
        source: None,
        destination: [0, 0, 2, 1],
        tint: [255; 3],
        opacity: 255,
        flip_x: true,
        filter: Sampling::Nearest,
    };
    composite(&mut frame, 2, &image, &layer);
    assert_eq!(&frame[..3], &[0, 0, 255]);
    assert_eq!(&frame[4..7], &[255, 0, 0]);
}
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "roses-client-synthetic-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn write_fixture(dir: &Path) -> Manifest {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[200, 100, 50, 255])
            .unwrap();
    }
    std::fs::write(dir.join("pixel.png"), &bytes).unwrap();
    std::fs::write(dir.join("intro.mp4"), b"synthetic not playable").unwrap();
    let mut m = fixture();
    m.assets[0].bytes = bytes.len() as u64;
    m.assets[0].sha256 = format!("{:x}", Sha256::digest(&bytes));
    m.assets[1].bytes = 22;
    m.assets[1].sha256 = format!("{:x}", Sha256::digest(b"synthetic not playable"));
    m.assets[1].bytes = b"synthetic not playable".len() as u64;
    if let Stage::Screen { layers, .. } = &mut m.stages[1] {
        layers.push(Layer {
            asset: "pixel".into(),
            source: None,
            destination: [0, 0, 16, 16],
            tint: [255, 128, 255],
            opacity: 255,
            flip_x: false,
            filter: Sampling::Linear,
        });
    }
    m.initial_stage = "title".into();
    std::fs::write(dir.join("manifest.json"), serde_json::to_vec(&m).unwrap()).unwrap();
    m
}
#[test]
fn loader_checks_hashes_and_composes_imported_pixels() {
    let temp = Temp::new();
    let mut m = write_fixture(&temp.0);
    {
        let loaded = Loaded::open(&temp.0.join("manifest.json")).unwrap();
        let model = Model::new(m.clone()).unwrap();
        let frame = loaded.render(&model).unwrap();
        assert_eq!(&frame[..4], &[50, 50, 200, 255]);
        assert_eq!(frame.len(), 16 * 16 * 4);
    }
    m.assets[0].sha256 = "a".repeat(64);
    std::fs::write(
        temp.0.join("manifest.json"),
        serde_json::to_vec(&m).unwrap(),
    )
    .unwrap();
    assert!(Loaded::open(&temp.0.join("manifest.json"))
        .err()
        .unwrap()
        .contains("SHA256 mismatch"));
}
#[test]
fn transparent_overlay_retains_coverage_and_the_existing_byte_color_output() {
    let temp = Temp::new();
    let mut manifest = write_fixture(&temp.0);
    if let Stage::Screen { layers, .. } = &mut manifest.stages[1] {
        layers[0].destination = [0, 0, 8, 8];
        layers[0].opacity = 128;
        layers.push(Layer {
            destination: [8, 0, 8, 8],
            tint: [0; 3],
            opacity: 255,
            ..layers[0].clone()
        });
    }
    std::fs::write(
        temp.0.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let loaded = Loaded::open(&temp.0.join("manifest.json")).unwrap();
    let model = Model::new(manifest).unwrap();
    let overlay = loaded.render_overlay(&model).unwrap();
    assert_eq!([overlay.width, overlay.height], [16, 16]);
    assert_eq!(&overlay.rgba[..4], &[100, 25, 25, 128]);
    // An intentional black panel is opaque; uncovered background is transparent.
    assert_eq!(&overlay.rgba[8 * 4..9 * 4], &[0, 0, 0, 255]);
    assert_eq!(&overlay.rgba[8 * 16 * 4..8 * 16 * 4 + 4], &[0; 4]);
    let native = loaded.render(&model).unwrap();
    for (rgba, bgrx) in overlay.rgba.chunks_exact(4).zip(native.chunks_exact(4)) {
        assert_eq!([rgba[2], rgba[1], rgba[0], 255], bgrx);
    }
}

#[test]
fn overlay_coverage_accumulates_across_translucent_layers() {
    let mut frame = [0; 4];
    let layer = Layer {
        asset: "synthetic".into(),
        source: None,
        destination: [0, 0, 1, 1],
        tint: [255; 3],
        opacity: 255,
        flip_x: false,
        filter: Sampling::Nearest,
    };
    for rgba in [[0, 0, 255, 128], [255, 0, 0, 128]] {
        composite(
            &mut frame,
            1,
            &Image {
                width: 1,
                height: 1,
                rgba: rgba.to_vec(),
            },
            &layer,
        );
    }
    assert_eq!(frame, [128, 0, 64, 192]);
}
#[test]
fn loader_rejects_crop_outside_decoded_asset() {
    let temp = Temp::new();
    let mut m = write_fixture(&temp.0);
    if let Stage::Screen { layers, .. } = &mut m.stages[1] {
        layers[0].source = Some([0, 0, 2, 1]);
    }
    std::fs::write(
        temp.0.join("manifest.json"),
        serde_json::to_vec(&m).unwrap(),
    )
    .unwrap();
    assert!(Loaded::open(&temp.0.join("manifest.json"))
        .err()
        .unwrap()
        .contains("crop exceeds"));
}

fn synthetic_loop_music(dir: &Path, manifest: &mut Manifest) {
    let wav = [
        b"RIFF".as_slice(),
        &52u32.to_le_bytes(),
        b"WAVEfmt ",
        &16u32.to_le_bytes(),
        &1u16.to_le_bytes(),
        &2u16.to_le_bytes(),
        &48000u32.to_le_bytes(),
        &192000u32.to_le_bytes(),
        &4u16.to_le_bytes(),
        &16u16.to_le_bytes(),
        b"data",
        &16u32.to_le_bytes(),
        &[0, 0, 1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0],
    ]
    .concat();
    std::fs::write(dir.join("music.wav"), &wav).unwrap();
    manifest.assets.push(Asset {
        id: "music".into(),
        kind: AssetKind::Wav,
        path: "music.wav".into(),
        sha256: format!("{:x}", Sha256::digest(&wav)),
        bytes: wav.len() as u64,
    });
    if let Stage::Screen { music, .. } = &mut manifest.stages[1] {
        *music = Some(Music {
            asset: "music".into(),
            looped: true,
            loop_start_frame: Some(2),
        });
    }
}

#[test]
fn music_loop_schema_preserves_old_manifests_and_rejects_nonlooping_region() {
    let music: Music = serde_json::from_str(r#"{"asset":"music","looped":true}"#).unwrap();
    assert!(music.loop_start_frame.is_none());
    let temp = Temp::new();
    let mut manifest = write_fixture(&temp.0);
    synthetic_loop_music(&temp.0, &mut manifest);
    assert!(validate_manifest(&manifest).is_ok());
    if let Stage::Screen {
        music: Some(music), ..
    } = &mut manifest.stages[1]
    {
        music.looped = false;
    }
    assert!(validate_manifest(&manifest)
        .unwrap_err()
        .contains("requires looped"));
}

#[test]
fn loader_retains_exact_pcm_and_validates_loop_bounds_before_playback() {
    let temp = Temp::new();
    let mut manifest = write_fixture(&temp.0);
    synthetic_loop_music(&temp.0, &mut manifest);
    let path = temp.0.join("manifest.json");
    std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    {
        let loaded = Loaded::open(&path).unwrap();
        let pcm = loaded.assets["music"].pcm16.as_ref().unwrap();
        assert_eq!(
            (pcm.channels(), pcm.sample_rate(), pcm.frame_count()),
            (2, 48000, 4)
        );
        assert_eq!(pcm.data(), [0, 0, 1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0]);
        assert!(loaded.assets["movie"].pcm16.is_none());
    }
    if let Stage::Screen {
        music: Some(music), ..
    } = &mut manifest.stages[1]
    {
        music.loop_start_frame = Some(4);
    }
    std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert!(Loaded::open(&path).err().unwrap().contains("Loop start"));
}

#[test]
fn loader_rejects_hash_valid_malformed_loop_audio_and_bounds_audio_memory() {
    let temp = Temp::new();
    let mut manifest = write_fixture(&temp.0);
    synthetic_loop_music(&temp.0, &mut manifest);
    let path = temp.0.join("manifest.json");
    let mut wav = std::fs::read(temp.0.join("music.wav")).unwrap();
    wav[32..34].copy_from_slice(&2u16.to_le_bytes()); // wrong stereo block alignment
    std::fs::write(temp.0.join("music.wav"), &wav).unwrap();
    manifest.assets.last_mut().unwrap().sha256 = format!("{:x}", Sha256::digest(&wav));
    std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert!(Loaded::open(&path)
        .err()
        .unwrap()
        .contains("consistent byte/block rates"));
    manifest.assets.last_mut().unwrap().bytes = audio::MAX_PCM_WAV_BYTES as u64 + 1;
    std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert!(Loaded::open(&path)
        .err()
        .unwrap()
        .contains("128 MiB PCM budget"));
}

fn story_manifest(on_end: StoryEnd) -> Manifest {
    let mut manifest = fixture();
    manifest.initial_stage = "story".into();
    manifest.stages.push(Stage::Story {
        id: "story".into(),
        music: None,
        advance_buttons: vec![Button::Cross],
        on_end,
        pages: (0..3)
            .map(|index| StoryPage {
                id: format!("page-{index}"),
                layer_set: None,
                pan: None,
                name_text: None,
                layers: vec![Layer {
                    asset: "pixel".into(),
                    source: None,
                    destination: [0, 0, 16, 16],
                    tint: [255; 3],
                    opacity: 255,
                    flip_x: false,
                    filter: Sampling::Linear,
                }],
            })
            .collect(),
    });
    manifest
}
#[test]
fn story_stops_at_explicit_unsupported_boundary() {
    let mut model = Model::new(story_manifest(StoryEnd::Unsupported {
        message: "Synthetic next scene is unavailable.".into(),
    }))
    .unwrap();
    assert_eq!(model.page_index(), 0);
    assert_eq!(model.input(Button::Circle), Outcome::None);
    assert_eq!(model.elapsed(100_000), Outcome::None);
    assert_eq!(model.input(Button::Cross), Outcome::Repaint);
    assert_eq!(model.page_index(), 1);
    assert_eq!(model.input(Button::Cross), Outcome::Repaint);
    for _ in 0..2 {
        assert_eq!(
            model.input(Button::Cross),
            Outcome::Unsupported("Synthetic next scene is unavailable.".into())
        );
        assert_eq!(model.page_index(), 2);
        assert_eq!(model.stage().id(), "story");
    }
}
#[test]
fn story_explicit_transition_and_reentry_reset_page() {
    let mut manifest = story_manifest(StoryEnd::Stage {
        target: "menu".into(),
    });
    if let Stage::Menu { entries, .. } = &mut manifest.stages[2] {
        entries[0].action = MenuAction::Stage {
            target: "story".into(),
        };
    }
    let mut model = Model::new(manifest).unwrap();
    assert_eq!(model.input(Button::Cross), Outcome::Repaint);
    assert_eq!(model.input(Button::Cross), Outcome::Repaint);
    assert_eq!(model.input(Button::Cross), Outcome::StageChanged);
    assert_eq!(model.stage().id(), "menu");
    assert_eq!(model.input(Button::Cross), Outcome::StageChanged);
    assert_eq!(model.stage().id(), "story");
    assert_eq!(model.page_index(), 0);
}
#[test]
fn story_validation_rejects_missing_duplicate_and_unbounded_pages() {
    let base = story_manifest(StoryEnd::Unsupported {
        message: "Synthetic boundary".into(),
    });
    for mutation in 0..6 {
        let mut manifest = base.clone();
        if let Stage::Story {
            pages,
            on_end,
            advance_buttons,
            ..
        } = manifest.stages.last_mut().unwrap()
        {
            match mutation {
                0 => pages.clear(),
                1 => pages[1].id = pages[0].id.clone(),
                2 => {
                    *pages = (0..129)
                        .map(|index| StoryPage {
                            id: format!("page-{index}"),
                            layer_set: None,
                            layers: pages[0].layers.clone(),
                            name_text: None,
                            pan: None,
                        })
                        .collect()
                }
                3 => pages[0].layers.clear(),
                4 => {
                    *on_end = StoryEnd::Stage {
                        target: "absent".into(),
                    }
                }
                5 => advance_buttons.clear(),
                _ => unreachable!(),
            }
        }
        assert!(validate_manifest(&manifest).is_err(), "mutation {mutation}");
    }
}
#[test]
fn story_renders_current_page_and_validates_all_page_crops() {
    let temp = Temp::new();
    let files = write_fixture(&temp.0);
    let mut manifest = story_manifest(StoryEnd::Unsupported {
        message: "Synthetic boundary".into(),
    });
    manifest.assets = files.assets;
    if let Stage::Story { pages, .. } = manifest.stages.last_mut().unwrap() {
        pages[1].layers[0].tint = [0, 255, 255];
    }
    std::fs::write(
        temp.0.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    {
        let loaded = Loaded::open(&temp.0.join("manifest.json")).unwrap();
        let mut model = Model::new(manifest.clone()).unwrap();
        assert_eq!(&loaded.render(&model).unwrap()[..4], &[50, 100, 200, 255]);
        model.input(Button::Cross);
        assert_eq!(&loaded.render(&model).unwrap()[..4], &[50, 100, 0, 255]);
    }
    if let Stage::Story { pages, .. } = manifest.stages.last_mut().unwrap() {
        pages[2].layers[0].source = Some([0, 0, 2, 1]);
    }
    std::fs::write(
        temp.0.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    assert!(Loaded::open(&temp.0.join("manifest.json"))
        .err()
        .unwrap()
        .contains("crop exceeds"));
}

#[test]
#[ignore = "requires a private imported story via ROSES_STORY_MANIFEST"]
fn private_import_connects_movie_menu_and_all_historical_pages() {
    let path =
        PathBuf::from(std::env::var_os("ROSES_STORY_MANIFEST").expect("set ROSES_STORY_MANIFEST"));
    let loaded = Loaded::open(&path).unwrap();
    let mut model = Model::new(loaded.manifest.clone()).unwrap();
    assert_eq!(model.stage().id(), "intro");
    assert!(matches!(model.input(Button::Start), Outcome::StageChanged));
    assert_eq!(model.stage().id(), "title");
    assert!(matches!(model.input(Button::Start), Outcome::StageChanged));
    assert_eq!(model.stage().id(), "main-menu");
    assert!(matches!(model.input(Button::Cross), Outcome::StageChanged));
    assert_eq!(model.stage().id(), "historical-story");
    let expected = [
        2032, 2036, 2037, 2040, 2044, 2045, 2048, 2052, 2053, 2060, 2064, 2068, 2069, 2072, 2073,
    ];
    for (index, message_id) in expected.iter().enumerate() {
        let Stage::Story { pages, .. } = model.stage() else {
            panic!("lost story stage")
        };
        assert_eq!(pages.len(), expected.len());
        assert_eq!(
            pages[model.page_index()].id,
            format!("message-{message_id}")
        );
        assert_eq!(loaded.render(&model).unwrap().len(), 640 * 448 * 4);
        assert!(matches!(model.input(Button::Circle), Outcome::None));
        assert_eq!(model.page_index(), index);
        let result = model.input(Button::Cross);
        if index + 1 == expected.len() {
            assert!(matches!(result, Outcome::StageChanged));
            assert_eq!(model.stage().id(), "simon-arrival");
        } else {
            assert!(matches!(result, Outcome::Repaint));
        }
    }
    for index in 0..12 {
        let Stage::Story { pages, .. } = model.stage() else {
            panic!("lost Simon stage")
        };
        assert_eq!(pages.len(), 12);
        assert_eq!(model.page_index(), index);
        assert_eq!(loaded.render(&model).unwrap().len(), 640 * 448 * 4);
        assert_eq!(model.input(Button::Circle), Outcome::None);
        assert_eq!(
            model.input(Button::Cross),
            if index == 11 {
                Outcome::StageChanged
            } else {
                Outcome::Repaint
            }
        );
    }
    assert_eq!(model.stage().id(), "name-entry");
    let Stage::NameEntry { config, .. } = model.stage() else {
        panic!("missing name entry")
    };
    assert_eq!(
        (
            config.rows,
            config.columns,
            config.pages.len(),
            config.max_length
        ),
        (6, 13, 2, 12)
    );
    let first_code = config.pages[0].cells[0].stored_code;
    assert_eq!(model.input(Button::Start), Outcome::None);
    model.input(Button::Up);
    model.input(Button::Left);
    model.input(Button::L1);
    let state = model.name_entry().unwrap();
    assert_eq!((state.row(), state.column(), state.page()), (5, 12, 1));
    model.input(Button::Down);
    model.input(Button::Right);
    model.input(Button::R1);
    model.input(Button::Square);
    assert_eq!(
        model.name_entry().unwrap().glyphs()[0].stored_code,
        first_code
    );
    model.input(Button::Circle);
    assert!(model.name_entry().unwrap().glyphs().is_empty());
    for _ in 0..12 {
        assert_eq!(model.input(Button::Cross), Outcome::Repaint);
    }
    assert_eq!(model.input(Button::Cross), Outcome::None);
    assert_eq!(loaded.render(&model).unwrap().len(), 640 * 448 * 4);
    assert!(matches!(
        model.input(Button::Start),
        Outcome::Unsupported(_)
    ));
    let committed = model.player_name().unwrap();
    assert_eq!(committed.glyphs.len(), 12);
    assert!(committed
        .glyphs
        .iter()
        .all(|glyph| glyph.stored_code == first_code));
}

#[test]
fn name_text_inserts_committed_glyphs_and_skips_unused_slots() {
    // 44x22 synthetic atlas: cell A (x0) is red, cell B (x22) is green.
    let mut rgba = vec![0u8; 44 * 22 * 4];
    for y in 0..22 {
        for x in 0..44 {
            let i = (y * 44 + x) * 4;
            rgba[i..i + 4].copy_from_slice(if x < 22 {
                &[255, 0, 0, 255]
            } else {
                &[0, 255, 0, 255]
            });
        }
    }
    let font = Image {
        width: 44,
        height: 22,
        rgba,
    };
    let text = NameText {
        font: "font".into(),
        origin: [10, 20],
        lines: vec![vec![
            TextCell::Glyph { cell: [0, 0] },
            TextCell::PlayerName { slot: 0 },
            TextCell::PlayerName { slot: 1 },
            TextCell::Glyph { cell: [0, 0] },
        ]],
        name_cells: vec![NameCell {
            stored_code: 7,
            cell: [22, 0],
        }],
    };
    let name = PlayerName {
        glyphs: vec![NameCharacter {
            page: 0,
            row: 0,
            column: 0,
            display_code: 9,
            stored_code: 7,
        }],
    };
    let (image, layer) = text.compose(&font, Some(&name)).unwrap();
    // Slot 1 is unused: three glyphs of 12 columns remain.
    assert_eq!((image.width, image.height), (36, 22));
    assert_eq!(&image.rgba[12 * 4..12 * 4 + 4], &[0, 255, 0, 255]);
    assert_eq!(&image.rgba[24 * 4..24 * 4 + 4], &[255, 0, 0, 255]);
    assert_eq!(layer.destination, [10, 20, 45, 19]);
    assert!(text.compose(&font, None).is_err());
    let unknown = PlayerName {
        glyphs: vec![NameCharacter {
            page: 0,
            row: 0,
            column: 0,
            display_code: 9,
            stored_code: 8,
        }],
    };
    assert!(text.compose(&font, Some(&unknown)).is_err());
}

fn pixel_layer(tint: [u8; 3]) -> Layer {
    Layer {
        asset: "pixel".into(),
        source: None,
        destination: [0, 0, 16, 16],
        tint,
        opacity: 255,
        flip_x: false,
        filter: Sampling::Linear,
    }
}

/// Name entry → three-page story → deck choice → side menu → stand-in.
fn campaign_manifest() -> Manifest {
    let mut manifest = story_manifest(StoryEnd::Stage {
        target: "decks".into(),
    });
    add_name_entry(&mut manifest);
    if let Stage::NameEntry { on_complete, .. } = manifest.stages.last_mut().unwrap() {
        *on_complete = StoryEnd::Stage {
            target: "story".into(),
        };
    }
    let config = DeckChoiceConfig {
        candidate_table: (0..16u16)
            .map(|i| (i % 3) | ((i + 1) % 3) << 5 | ((i + 2) % 3) << 10)
            .collect(),
        blank_name_code: 0x2001,
        decks: (0..3u8)
            .map(|record| StarterDeck {
                record,
                leader: 10 + u16::from(record),
                average_level_x100: 300,
                leader_name: format!("synthetic {record}"),
                cards: vec![u16::from(record) + 1; 40],
                presentation_layers: vec![],
                card_face: None,
            })
            .collect(),
    };
    manifest.stages.push(Stage::DeckChoice {
        id: "decks".into(),
        layers: vec![pixel_layer([255; 3])],
        music: None,
        config,
        confirm_buttons: vec![Button::Start],
        on_confirm: StoryEnd::Stage {
            target: "sides".into(),
        },
    });
    let entry = |id: &str, action, side| MenuEntry {
        id: id.into(),
        label: id.into(),
        normal_layers: vec![],
        selected_layers: vec![],
        action,
        campaign_side: Some(side),
    };
    manifest.stages.push(Stage::Menu {
        id: "sides".into(),
        layers: vec![],
        music: None,
        entries: vec![
            entry(
                "york",
                MenuAction::Unsupported {
                    message: "Synthetic white route is not implemented.".into(),
                },
                CampaignSide::Yorkists,
            ),
            entry(
                "lancaster",
                MenuAction::Stage {
                    target: "map".into(),
                },
                CampaignSide::Lancastrians,
            ),
        ],
        initial_selection: 1,
        confirm_buttons: vec![Button::Start],
        cancel_target: None,
        wrap_navigation: false,
    });
    manifest.stages.push(Stage::Placeholder {
        id: "map".into(),
        message: "Synthetic map stand-in.".into(),
        advance_buttons: vec![Button::Cross],
        next: StoryEnd::Unsupported {
            message: "Synthetic end.".into(),
        },
    });
    manifest.initial_stage = "name".into();
    manifest
}

#[test]
fn deck_choice_uses_the_committed_name_and_records_campaign_state() {
    let mut model = Model::new(campaign_manifest()).unwrap();
    model.input(Button::Cross); // page 0, row 0, column 0: stored code 100
    assert_eq!(model.input(Button::Start), Outcome::StageChanged);
    for _ in 0..3 {
        model.input(Button::Cross);
    }
    assert_eq!(model.stage().id(), "decks");
    assert_eq!(model.selection, 0);
    // 100 & 0xF = 4 plus eleven blanks (0x2001 & 0xF = 1): 15 → [0, 1, 2].
    let decks = model.deck_candidates().unwrap().unwrap();
    assert_eq!(decks.map(|d| d.record), [0, 1, 2]);
    assert_eq!(model.input(Button::Left), Outcome::Repaint);
    assert_eq!(model.selection, 2);
    model.input(Button::Right);
    model.input(Button::Right);
    assert_eq!(model.selection, 1);
    // Every frontend shows the untraced deck details and the selection as text.
    let status = model.explicit_status().unwrap();
    assert!(status.starts_with("[placeholder") && status.contains(">2 deck 1"));
    assert_eq!(model.input(Button::Cross), Outcome::None);
    assert!(model.campaign().starter_deck.is_none());
    assert_eq!(model.input(Button::Start), Outcome::StageChanged);
    let chosen = model.campaign().starter_deck.clone().unwrap();
    assert_eq!((chosen.checksum, chosen.record, chosen.leader), (15, 1, 11));
    assert_eq!(chosen.candidates, [0, 1, 2]);
    assert_eq!(chosen.name_codes[..2], [100 | 0x8000, 0x2001]);
    assert_eq!(chosen.cards, vec![2; 40]);
    assert_eq!(model.stage().id(), "sides");
    assert_eq!(model.selection, 1);
    let mut york = model.clone();
    york.input(Button::Up);
    assert_eq!(
        york.input(Button::Start),
        Outcome::Unsupported("Synthetic white route is not implemented.".into())
    );
    assert_eq!(york.campaign().side, Some(CampaignSide::Yorkists));
    assert_eq!(model.input(Button::Start), Outcome::StageChanged);
    assert_eq!(model.campaign().side, Some(CampaignSide::Lancastrians));
    assert_eq!(model.stage().id(), "map");
    assert!(model.explicit_status().unwrap().starts_with("[stand-in] "));
    assert_eq!(model.input(Button::Start), Outcome::None);
    assert_eq!(
        model.input(Button::Cross),
        Outcome::Unsupported("Synthetic end.".into())
    );
    let json = model.campaign().to_json();
    assert_eq!(
        serde_json::from_str::<CampaignState>(&json).unwrap(),
        *model.campaign()
    );
}

#[test]
fn deck_choice_without_a_committed_name_is_an_explicit_boundary() {
    let mut manifest = campaign_manifest();
    manifest.initial_stage = "decks".into();
    let mut model = Model::new(manifest.clone()).unwrap();
    assert!(model.deck_candidates().unwrap().is_err());
    assert!(matches!(
        model.input(Button::Start),
        Outcome::Unsupported(_)
    ));
    assert_eq!(model.stage().id(), "decks");
    for stage in &mut manifest.stages {
        if let Stage::DeckChoice { config, .. } = stage {
            // Record 3 does not exist among the three synthetic decks.
            config.candidate_table[3] = 3;
        }
    }
    assert!(validate_manifest(&manifest).is_err());
}

#[test]
fn deck_choice_renders_the_selected_records_imported_panel() {
    let temp = Temp::new();
    let files = write_fixture(&temp.0);
    let mut manifest = campaign_manifest();
    manifest.assets = files.assets;
    let colors = [[255, 0, 0], [0, 255, 0], [0, 0, 255]];
    for stage in &mut manifest.stages {
        if let Stage::DeckChoice { config, .. } = stage {
            for (deck, color) in config.decks.iter_mut().zip(colors) {
                deck.presentation_layers = vec![pixel_layer(color)];
            }
        }
    }
    let path = temp.0.join("manifest.json");
    std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let loaded = Loaded::open(&path).unwrap();
    let mut model = Model::new(manifest.clone()).unwrap();
    model.input(Button::Cross);
    model.input(Button::Start);
    for _ in 0..3 {
        model.input(Button::Cross);
    }
    assert_eq!(model.stage().id(), "decks");
    // Fixture PNG is RGB(200,100,50). Changing candidate changes the rendered panel,
    // while confirmation still stores the matching original deck record.
    assert_eq!(
        &loaded.render_overlay(&model).unwrap().rgba[..4],
        &[200, 0, 0, 255]
    );
    model.input(Button::Left);
    assert_eq!(
        &loaded.render_overlay(&model).unwrap().rgba[..4],
        &[0, 0, 50, 255]
    );
    model.input(Button::Right);
    model.input(Button::Right);
    assert_eq!(
        &loaded.render_overlay(&model).unwrap().rgba[..4],
        &[0, 100, 0, 255]
    );
    model.input(Button::Start);
    assert_eq!(model.campaign().starter_deck.as_ref().unwrap().record, 1);

    // All records are checked when loading, including unselected candidates.
    for stage in &mut manifest.stages {
        if let Stage::DeckChoice { config, .. } = stage {
            config.decks[2].presentation_layers[0].source = Some([0, 0, 2, 1]);
        }
    }
    std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert!(Loaded::open(&path).is_err());
}

#[test]
fn story_pages_draw_their_shared_layer_set_first() {
    let temp = Temp::new();
    let files = write_fixture(&temp.0);
    let mut manifest = story_manifest(StoryEnd::Unsupported {
        message: "Synthetic boundary".into(),
    });
    manifest.assets = files.assets;
    manifest
        .layer_sets
        .insert("shared".into(), vec![pixel_layer([0, 255, 0])]);
    if let Stage::Story { pages, .. } = manifest.stages.last_mut().unwrap() {
        pages[0].layer_set = Some("shared".into());
        pages[0].layers.clear();
        pages[1].layer_set = Some("shared".into());
        pages[1].layers[0].opacity = 0;
        pages[2].pan = Some(-100);
    }
    std::fs::write(
        temp.0.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let loaded = Loaded::open(&temp.0.join("manifest.json")).unwrap();
    let mut model = Model::new(manifest.clone()).unwrap();
    assert_eq!(loaded.active_layers(&model).unwrap().len(), 1);
    assert_eq!(&loaded.render(&model).unwrap()[..4], &[0, 100, 0, 255]);
    model.input(Button::Cross);
    assert_eq!(loaded.active_layers(&model).unwrap().len(), 2);
    assert_eq!(&loaded.render(&model).unwrap()[..4], &[0, 100, 0, 255]);
    model.input(Button::Cross);
    assert_eq!(model.story_page().unwrap().pan, Some(-100));
    let mut missing = manifest.clone();
    missing.layer_sets.clear();
    assert!(validate_manifest(&missing)
        .unwrap_err()
        .contains("layer set"));
    let mut far = manifest;
    if let Stage::Story { pages, .. } = far.stages.last_mut().unwrap() {
        pages[2].pan = Some(-101);
    }
    assert!(validate_manifest(&far).is_err());
}
