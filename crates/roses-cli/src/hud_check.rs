//! Private checks: the GS sprite model in `roses_client::gs` against a
//! software-GS draw dump of an original duel frame
//! (the project notes), using textures decoded from
//! the user's disc; and the duel HUD port `roses_client::duel_hud` against
//! the primitive lists the game's own HUD routine produced for savestates.
#[cfg(test)]
mod tests {
    use roses_client::gs::{draw, Blend, Sprite, Target16, Texture};
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    fn env(name: &str) -> PathBuf {
        PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("{name}")))
    }

    /// `key: value` fields of each register block in a dump context file.
    fn context(path: &Path) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        let mut block = String::new();
        for line in std::fs::read_to_string(path).unwrap().lines() {
            if !line.starts_with(' ') && line.ends_with(':') {
                block = line.trim_end_matches(':').to_string();
            } else if let Some((k, v)) = line.trim().split_once(':') {
                let v = v.split('#').next().unwrap().trim().to_string();
                out.insert(format!("{block}.{}", k.trim()), v);
            }
        }
        out
    }

    fn num(map: &BTreeMap<String, String>, key: &str) -> i64 {
        let v = &map[key];
        v.strip_prefix("0x").map_or_else(
            || v.parse::<f64>().unwrap() as i64,
            |h| i64::from_str_radix(h, 16).unwrap(),
        )
    }

    /// (x, y, u, v, r, g, b, a) per vertex.
    fn vertices(path: &Path) -> Vec<[f32; 8]> {
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .take_while(|l| !l.starts_with("vertex_stq") && !l.starts_with("vertex_trace"))
            .filter(|l| l.trim_start().starts_with("- {X:"))
            .map(|l| {
                let body = &l[l.find('{').unwrap() + 1..l.find('}').unwrap()];
                let mut v = [0f32; 8];
                for (i, key) in ["X", "Y", "U", "V", "R", "G", "B", "A"].iter().enumerate() {
                    if let Some(field) = body
                        .split(',')
                        .find(|f| f.trim().starts_with(&format!("{key}:")))
                    {
                        v[i] = field.split(':').nth(1).unwrap().trim().parse().unwrap();
                    }
                }
                v
            })
            .collect()
    }

    /// A C_16S render-target dump: the PNG's R and G hold the raw 16 bits.
    fn target(path: &Path) -> Target16 {
        let decoder =
            png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
        let mut reader = decoder.read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buf).unwrap();
        let channels = info.color_type.samples();
        let (w, h) = (info.width as usize, info.height as usize);
        Target16 {
            width: w,
            height: h,
            pixels: (0..w * h)
                .map(|i| u16::from(buf[i * channels]) | u16::from(buf[i * channels + 1]) << 8)
                .collect(),
        }
    }

    #[test]
    #[ignore = "Requires the user's ISO and the private GS dump"]
    fn private_hud_draws_match_the_dump() {
        let iso = env("ROSES_TEST_ISO_PATH");
        let dump = env("ROSES_TEST_GS_DUMP");
        let (mut disc, _) = crate::open_pal(&iso).unwrap();
        let texsys = disc.read_file("DATA/TEXSYS.MRG", 2 * 1024 * 1024).unwrap();
        let picmini = disc.read_file("DATA/PICMINI.MRG", 4 * 1024 * 1024).unwrap();
        let atlas = |index: usize| {
            let t = roses_assets::decode_pal_opening_indexed_texture(
                &texsys,
                roses_assets::OpeningArchiveKind::TexSys,
                index,
            )
            .unwrap();
            Texture::from_c16_clut(
                t.width as usize,
                t.height as usize,
                &t.palette_indices,
                &t.palette_words_raw,
                0,
                128,
            )
        };
        let mini = |card: u16| {
            let t = roses_assets::decode_pal_mini_picture(&picmini, card).unwrap();
            Texture::from_c16_clut(
                t.width as usize,
                t.height as usize,
                &t.palette_indices,
                &t.palette_words_raw,
                0,
                128,
            )
        };
        let (panels, icons) = (atlas(7), atlas(6));
        let portraits = [(6936, mini(392)), (6950, mini(397))];
        let (mut checked, mut pixels, mut bad) = (0, 0, Vec::new());
        for id in 6933..=6989 {
            let ctx_path = dump.join(format!("{id:05}_context.txt"));
            if !ctx_path.exists() {
                continue;
            }
            let ctx = context(&ctx_path);
            let verts = vertices(&dump.join(format!("{id:05}_vertex.txt")));
            if ctx["PRIM.PRIM"] != "6"
                || num(&ctx, "PRIM.TME") != 1
                || num(&ctx, "PRIM.FST") != 1
                || verts.is_empty()
            {
                continue;
            }
            let texture = match (num(&ctx, "TEX0.TBP0"), num(&ctx, "TEX0.CBP")) {
                (0x23C0, 0x2005) => &panels,
                (0x22C0, 0x2003) => &icons,
                (0x24E0, 0x24C0) => {
                    &portraits
                        .iter()
                        .find(|(d, _)| *d == id)
                        .expect("portrait draw")
                        .1
                }
                _ => continue,
            };
            let name = |kind: &str| {
                std::fs::read_dir(&dump)
                    .unwrap()
                    .map(|e| e.unwrap().path())
                    .find(|p| {
                        let n = p.file_name().unwrap().to_string_lossy().to_string();
                        n.starts_with(&format!("{id:05}_f")) && n.contains(kind)
                    })
                    .unwrap()
            };
            let mut t = target(&name("_rt0_"));
            let want = target(&name("_rt1_"));
            let dimx =
                (0..4)
                    .flat_map(|y| (0..4).map(move |x| (x, y)))
                    .fold(0u64, |acc, (x, y)| {
                        let v = num(&ctx, &format!("DIMX.DM{y}{x}")) as u64 & 7;
                        acc | v << ((y * 4 + x) * 4)
                    });
            for pair in verts.chunks(2) {
                let [a, b] = [pair[0], pair[1]];
                draw(
                    &mut t,
                    &Sprite {
                        xy: [a[0], a[1], b[0], b[1]],
                        uv: [a[2], a[3], b[2], b[3]],
                        rgba: [b[4] as u8, b[5] as u8, b[6] as u8, b[7] as u8],
                        texture: Some(texture),
                        modulate: num(&ctx, "TEX0.TFX") == 0,
                        tcc: num(&ctx, "TEX0.TCC") == 1,
                        blend: (num(&ctx, "PRIM.ABE") == 1).then(|| Blend {
                            a: num(&ctx, "ALPHA.A") as u8,
                            b: num(&ctx, "ALPHA.B") as u8,
                            c: num(&ctx, "ALPHA.C") as u8,
                            d: num(&ctx, "ALPHA.D") as u8,
                            fix: num(&ctx, "ALPHA.FIX") as u8,
                        }),
                        alpha_test_greater: (num(&ctx, "TEST.ATE") == 1
                            && num(&ctx, "TEST.ATST") == 6)
                            .then(|| num(&ctx, "TEST.AREF") as u8),
                        dither: (num(&ctx, "DTHE.DTHE") == 1).then_some(dimx),
                        bilinear: false,
                    },
                );
            }
            checked += 1;
            pixels += want.pixels.len();
            let diff = t
                .pixels
                .iter()
                .zip(&want.pixels)
                .filter(|(a, b)| a != b)
                .count();
            if diff != 0 {
                bad.push((id, diff));
            }
        }
        assert!(checked >= 25, "{checked} draws checked");
        assert!(
            bad.is_empty(),
            "{checked} draws, {pixels} pixels; differing: {bad:?}"
        );
    }

    /// The texture of a dumped draw, by TBP0/CBP.
    enum Tex<'a> {
        Fixed(&'a Texture),
        /// The text canvas as the dump holds it at this draw.
        Canvas,
        /// RT 0x2000 as the dump holds it at this draw.
        RenderTarget,
    }

    /// Indices of a dumped P_4 texture image (R channel = index).
    fn indices(path: &Path) -> (usize, usize, Vec<u8>) {
        let decoder =
            png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
        let mut reader = decoder.read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buf).unwrap();
        let channels = info.color_type.samples();
        let (w, h) = (info.width as usize, info.height as usize);
        (w, h, (0..w * h).map(|i| buf[i * channels]).collect())
    }

    /// Every HUD draw of the dumped frame into frame 0x0 (6929, 6933–6989)
    /// rasterised by `roses_client::gs` from the dump's own vertices and
    /// state, textures from the disc, the dumped text canvas / render target,
    /// and a second time with the text canvas composed by `duel_hud` from the
    /// message font. Reports the paths checked.
    #[test]
    #[ignore = "Requires the user's executable and ISO and the private GS dump"]
    fn private_hud_paths_match_the_dump() {
        use roses_client::duel_hud::{HudTables, TextCanvas, TextFont};
        use roses_client::gs::{draw_strip, Triangle};
        let elf = std::fs::read(env("ROSES_TEST_ELF_PATH")).unwrap();
        let iso = env("ROSES_TEST_ISO_PATH");
        let dump = env("ROSES_TEST_GS_DUMP");
        let (mut disc, _) = crate::open_pal(&iso).unwrap();
        let texsys = disc.read_file("DATA/TEXSYS.MRG", 2 * 1024 * 1024).unwrap();
        let picmini = disc.read_file("DATA/PICMINI.MRG", 4 * 1024 * 1024).unwrap();
        let message_file = disc.read_file("MSG/MSG_E.DAT", 300_000).unwrap();
        let tables =
            HudTables::from_executable(&elf, super::primitives::hud_messages(&message_file))
                .unwrap();
        let font_offset = roses_assets::decode_pal_name_font(&message_file)
            .unwrap()
            .metadata
            .file_offset;
        let font = TextFont {
            cells: &message_file[font_offset..],
        };
        let atlas = |index: usize| {
            let t = roses_assets::decode_pal_opening_indexed_texture(
                &texsys,
                roses_assets::OpeningArchiveKind::TexSys,
                index,
            )
            .unwrap();
            Texture::from_c16_clut(
                t.width as usize,
                t.height as usize,
                &t.palette_indices,
                &t.palette_words_raw,
                0,
                128,
            )
        };
        let mini = |card: u16| {
            let t = roses_assets::decode_pal_mini_picture(&picmini, card).unwrap();
            Texture::from_c16_clut(
                t.width as usize,
                t.height as usize,
                &t.palette_indices,
                &t.palette_words_raw,
                0,
                128,
            )
        };
        let (panels, icons) = (atlas(7), atlas(6));
        let portraits = [(6936, mini(392)), (6950, mini(397))];
        // The composed canvas: digit strip, then each text draw's message.
        // Messages per text draw of the round-2 frame (card box lines 1-3,
        // legend labels and the terrain name recomposed into line 1).
        let mut canvas = TextCanvas::new();
        canvas.upload_digit_strip(font).unwrap();
        let text_draws: BTreeMap<u32, Option<(u8, u16)>> = [
            (6938, None),
            (6952, None),
            (6964, Some((1, 715))),
            (6967, Some((2, 46))),
            (6971, Some((3, 84))),
            (6975, Some((1, 179))),
            (6977, Some((1, 180))),
            (6979, Some((1, 181))),
            (6981, Some((1, 177))),
            (6985, Some((1, 72))),
        ]
        .into_iter()
        .collect();
        // Card-box lines are composed together before the first of them.
        for (line, id) in [(1u8, 715u16), (2, 46), (3, 84)] {
            canvas
                .compose(line, tables.message(id).unwrap(), font)
                .unwrap();
        }
        let mut paths: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
        let mut bad = Vec::new();
        let mut canvas_regions = 0;
        for id in (6929..=6989).filter(|&id| !(6930..=6932).contains(&id)) {
            let ctx_path = dump.join(format!("{id:05}_context.txt"));
            if !ctx_path.exists() {
                continue;
            }
            let ctx = context(&ctx_path);
            let verts = vertices(&dump.join(format!("{id:05}_vertex.txt")));
            if verts.is_empty() {
                continue;
            }
            let prim = num(&ctx, "PRIM.PRIM");
            let tme = num(&ctx, "PRIM.TME") == 1;
            let tex = if !tme {
                None
            } else {
                Some(match (num(&ctx, "TEX0.TBP0"), num(&ctx, "TEX0.CBP")) {
                    (0x23C0, 0x2005) => Tex::Fixed(&panels),
                    (0x22C0, 0x2003) => Tex::Fixed(&icons),
                    (0x24E0, 0x24C0) => Tex::Fixed(
                        &portraits
                            .iter()
                            .find(|(d, _)| *d == id)
                            .expect("portrait draw")
                            .1,
                    ),
                    (0x2040, 0x2000) => Tex::Canvas,
                    (0x2000, _) => Tex::RenderTarget,
                    other => panic!("draw {id}: texture {other:?}"),
                })
            };
            let file = |kind: &str| {
                std::fs::read_dir(&dump)
                    .unwrap()
                    .map(|e| e.unwrap().path())
                    .find(|p| {
                        let n = p.file_name().unwrap().to_string_lossy().to_string();
                        // Render targets and textures as raw C_16S or P_4 images.
                        n.starts_with(&format!("{id:05}_f"))
                            && n.contains(kind)
                            && (n.contains("C_16S") || n.contains("P_4"))
                    })
                    .unwrap_or_else(|| panic!("draw {id}: {kind}"))
            };
            let dumped_canvas;
            let composed;
            let dumped_rt;
            let mut textures: Vec<(&str, Option<&Texture>)> = Vec::new();
            match &tex {
                None => textures.push(("", None)),
                Some(Tex::Fixed(t)) => textures.push(("", Some(t))),
                Some(Tex::Canvas) => {
                    let (w, h, idx) = indices(&file("_itexraw_02040_P_4"));
                    // The composed lines must equal the dump where sampled.
                    if let Some(Some((line, message))) = text_draws.get(&id) {
                        let codes = tables.message(*message).unwrap();
                        canvas.compose(*line, codes, font).unwrap();
                        let width = 12 * codes.len();
                        for y in 22 * usize::from(*line)..22 * usize::from(*line) + 22 {
                            let row = y * w;
                            assert_eq!(
                                canvas.indices[row..row + width],
                                idx[row..row + width],
                                "draw {id}: canvas line {line} (message {message})"
                            );
                        }
                    } else {
                        for y in 0..22 {
                            assert_eq!(
                                canvas.indices[y * w..y * w + 13 * 22],
                                idx[y * w..y * w + 13 * 22],
                                "draw {id}: digit strip row {y}"
                            );
                        }
                    }
                    canvas_regions += 1;
                    dumped_canvas = Texture::from_c32_clut(w, h, &idx, &tables.text_clut);
                    composed = Texture::from_c32_clut(
                        canvas.width,
                        canvas.height,
                        &canvas.indices,
                        &tables.text_clut,
                    );
                    textures.push(("dumped canvas", Some(&dumped_canvas)));
                    textures.push(("composed canvas", Some(&composed)));
                }
                Some(Tex::RenderTarget) => {
                    let rt = target(&file("_itexraw_02000_C_16S"));
                    dumped_rt = Texture::from_c16(
                        rt.width,
                        rt.height,
                        &rt.pixels,
                        num(&ctx, "TEXA.TA0") as u8,
                        num(&ctx, "TEXA.TA1") as u8,
                    );
                    textures.push(("", Some(&dumped_rt)));
                }
            }
            let want = target(&file("_rt1_"));
            let dimx =
                (0..4)
                    .flat_map(|y| (0..4).map(move |x| (x, y)))
                    .fold(0u64, |acc, (x, y)| {
                        let v = num(&ctx, &format!("DIMX.DM{y}{x}")) as u64 & 7;
                        acc | v << ((y * 4 + x) * 4)
                    });
            let bilinear = num(&ctx, "TEX1.MMAG") == 1;
            let blend = (num(&ctx, "PRIM.ABE") == 1).then(|| Blend {
                a: num(&ctx, "ALPHA.A") as u8,
                b: num(&ctx, "ALPHA.B") as u8,
                c: num(&ctx, "ALPHA.C") as u8,
                d: num(&ctx, "ALPHA.D") as u8,
                fix: num(&ctx, "ALPHA.FIX") as u8,
            });
            let alpha_test = (num(&ctx, "TEST.ATE") == 1 && num(&ctx, "TEST.ATST") == 6)
                .then(|| num(&ctx, "TEST.AREF") as u8);
            let dither = (num(&ctx, "DTHE.DTHE") == 1).then_some(dimx);
            let (modulate, tcc) = (num(&ctx, "TEX0.TFX") == 0, num(&ctx, "TEX0.TCC") == 1);
            for (label, texture) in textures {
                let mut t = target(&file("_rt0_"));
                let path = match (prim, tme, bilinear) {
                    (6, false, _) => "sprite, untextured",
                    (6, true, false) => "sprite, 8-bit CLUT, nearest",
                    (6, true, true) if matches!(tex, Some(Tex::Canvas)) => {
                        if label == "composed canvas" {
                            "sprite, 4-bit CLUT, bilinear (composed canvas)"
                        } else {
                            "sprite, 4-bit CLUT, bilinear (dumped canvas)"
                        }
                    }
                    (6, true, true) => "sprite, 16-bit render target, bilinear",
                    (_, false, _) => "triangles, Gouraud, untextured",
                    (_, true, _) => "triangles, 8-bit CLUT, nearest",
                };
                if prim == 6 {
                    for pair in verts.chunks(2) {
                        let [a, b] = [pair[0], pair[1]];
                        draw(
                            &mut t,
                            &Sprite {
                                xy: [a[0], a[1], b[0], b[1]],
                                uv: [a[2], a[3], b[2], b[3]],
                                rgba: [b[4] as u8, b[5] as u8, b[6] as u8, b[7] as u8],
                                texture,
                                modulate,
                                tcc,
                                bilinear,
                                blend,
                                alpha_test_greater: alpha_test,
                                dither,
                            },
                        );
                    }
                } else {
                    let state = Triangle {
                        xy: [[0.0; 2]; 3],
                        uv: [[0.0; 2]; 3],
                        rgba: [[0; 4]; 3],
                        gouraud: num(&ctx, "PRIM.IIP") == 1,
                        texture,
                        modulate,
                        tcc,
                        blend,
                        alpha_test_greater: alpha_test,
                        dither,
                    };
                    // The dump lists the strips as independent triangles.
                    for tri in verts.chunks(3) {
                        let v: Vec<_> = tri
                            .iter()
                            .map(|v| {
                                (
                                    [v[0], v[1]],
                                    [v[2], v[3]],
                                    [v[4] as u8, v[5] as u8, v[6] as u8, v[7] as u8],
                                )
                            })
                            .collect();
                        draw_strip(&mut t, &v, &state);
                    }
                }
                let diff = t
                    .pixels
                    .iter()
                    .zip(&want.pixels)
                    .filter(|(a, b)| a != b)
                    .count();
                let entry = paths.entry(path).or_default();
                entry.0 += 1;
                entry.1 += want.pixels.len();
                if diff != 0 {
                    let first: Vec<_> = (0..t.pixels.len())
                        .filter(|&i| t.pixels[i] != want.pixels[i])
                        .take(4)
                        .map(|i| (i % t.width, i / t.width, t.pixels[i], want.pixels[i]))
                        .collect();
                    eprintln!("draw {id} {path}: {diff} pixels differ, e.g. {first:x?}");
                    bad.push((id, path, diff));
                }
            }
        }
        for (path, (draws, pixels)) in &paths {
            eprintln!("{path}: {draws} draws, {pixels} target pixels");
        }
        eprintln!("canvas regions equal to the dump: {canvas_regions}");
        assert!(bad.is_empty(), "differing draws: {bad:?}");
        assert_eq!(canvas_regions, 10);
    }

    /// End to end: the primitives `duel_hud::build` makes for the round-2
    /// savestate, rasterised by `roses_client::gs` with the render state the
    /// primitives carry (text composed into the canvas just before each text
    /// sprite), reproduce every HUD draw of the dumped frame into frame 0x0.
    /// Only the VU1 terrain-tile picture is taken from the dump.
    #[test]
    #[ignore = "Requires the user's executable and ISO, the private savestates, getters and GS dump"]
    fn private_hud_build_renders_like_the_dump() {
        use roses_client::duel_hud::{
            build, HudTables, PrimKind, Primitive, Target as HudTarget, TextCanvas, TextFont,
            TextureSource,
        };
        use roses_client::gs::{draw_strip, Triangle};
        let elf = std::fs::read(env("ROSES_TEST_ELF_PATH")).unwrap();
        let dump = env("ROSES_TEST_GS_DUMP");
        let (mut disc, _) = crate::open_pal(&env("ROSES_TEST_ISO_PATH")).unwrap();
        let texsys = disc.read_file("DATA/TEXSYS.MRG", 2 * 1024 * 1024).unwrap();
        let picmini = disc.read_file("DATA/PICMINI.MRG", 4 * 1024 * 1024).unwrap();
        let message_file = disc.read_file("MSG/MSG_E.DAT", 300_000).unwrap();
        let tables =
            HudTables::from_executable(&elf, super::primitives::hud_messages(&message_file))
                .unwrap();
        let font_offset = roses_assets::decode_pal_name_font(&message_file)
            .unwrap()
            .metadata
            .file_offset;
        let font = TextFont {
            cells: &message_file[font_offset..],
        };
        let name = "weevil-round2-start";
        let session = env("ROSES_TEST_SESSION");
        let state_path = [
            session.join(format!("{name}.p2s")),
            session
                .parent()
                .unwrap()
                .join("checkpoints")
                .join(format!("{name}.p2s")),
        ]
        .into_iter()
        .find(|p| p.exists())
        .unwrap();
        let getters: serde_json::Value = serde_json::from_slice(
            &std::fs::read(
                env("ROSES_TEST_HUD_GOLDEN")
                    .parent()
                    .unwrap()
                    .join("golden-getters")
                    .join(format!("{name}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        let prims: Vec<Primitive> = build(
            &super::primitives::hud_state(&state_path, &getters),
            &tables,
        )
        .into_iter()
        .filter(|p| p.target == HudTarget::Frame)
        .collect();
        let atlas = |index: usize| {
            let t = roses_assets::decode_pal_opening_indexed_texture(
                &texsys,
                roses_assets::OpeningArchiveKind::TexSys,
                index,
            )
            .unwrap();
            Texture::from_c16_clut(
                t.width as usize,
                t.height as usize,
                &t.palette_indices,
                &t.palette_words_raw,
                0,
                128,
            )
        };
        let mini = |card: u16| {
            let t = roses_assets::decode_pal_mini_picture(&picmini, card).unwrap();
            Texture::from_c16_clut(
                t.width as usize,
                t.height as usize,
                &t.palette_indices,
                &t.palette_words_raw,
                0,
                128,
            )
        };
        let (icons, panels) = (atlas(6), atlas(7));
        // [H] the portraits are the sides' deck-leader cards (392 and 397 here).
        let portraits = [mini(392), mini(397)];
        let file = |id: u32, kind: &str| {
            std::fs::read_dir(&dump)
                .unwrap()
                .map(|e| e.unwrap().path())
                .find(|p| {
                    let n = p.file_name().unwrap().to_string_lossy().to_string();
                    n.starts_with(&format!("{id:05}_f")) && n.contains(kind) && n.contains("C_16S")
                })
                .unwrap_or_else(|| panic!("draw {id}: {kind}"))
        };
        let rt_texture = {
            let rt = target(&file(6933, "_itexraw_02000"));
            Texture::from_c16(rt.width, rt.height, &rt.pixels, 0, 128)
        };
        let mut canvas = TextCanvas::new();
        canvas.upload_digit_strip(font).unwrap();
        let mut next = 0;
        let (mut draws, mut prim_count, mut bad) = (0, 0, Vec::new());
        for id in (6929..=6989).filter(|&id| !(6930..=6932).contains(&id)) {
            let ctx = context(&dump.join(format!("{id:05}_context.txt")));
            let verts = vertices(&dump.join(format!("{id:05}_vertex.txt")));
            let dimx =
                (0..4)
                    .flat_map(|y| (0..4).map(move |x| (x, y)))
                    .fold(0u64, |acc, (x, y)| {
                        let v = num(&ctx, &format!("DIMX.DM{y}{x}")) as u64 & 7;
                        acc | v << ((y * 4 + x) * 4)
                    });
            // The dump lists sprites as vertex pairs and strips as triangles.
            let sprite = num(&ctx, "PRIM.PRIM") == 6;
            let count = if sprite {
                verts.len() / 2
            } else {
                verts.len() / 6
            };
            let mut t = target(&file(id, "_rt0_"));
            let want = target(&file(id, "_rt1_"));
            for p in &prims[next..next + count] {
                assert_eq!(
                    p.kind == PrimKind::Sprite,
                    sprite,
                    "draw {id}: primitive kind"
                );
                let canvas_texture;
                let texture = match p.texture {
                    TextureSource::None => None,
                    TextureSource::Atlas(6) => Some(&icons),
                    TextureSource::Atlas(_) => Some(&panels),
                    TextureSource::Portrait(side) => Some(&portraits[usize::from(side)]),
                    TextureSource::RenderTarget => Some(&rt_texture),
                    TextureSource::Picture => None,
                    TextureSource::TextCanvas { line, message } => {
                        if let Some(m) = message.filter(|_| line > 0) {
                            canvas
                                .compose(line, tables.message(m).unwrap(), font)
                                .unwrap();
                        }
                        canvas_texture = Texture::from_c32_clut(
                            canvas.width,
                            canvas.height,
                            &canvas.indices,
                            &tables.text_clut,
                        );
                        Some(&canvas_texture)
                    }
                };
                let blend = p
                    .blend
                    .map(|m| Blend::from_register(tables.alpha_modes[usize::from(m)]));
                let alpha_test_greater = p.alpha_test.then_some(0);
                let v = &p.verts;
                if sprite {
                    draw(
                        &mut t,
                        &Sprite {
                            xy: [v[0].x, v[0].y, v[1].x, v[1].y],
                            uv: [v[0].u, v[0].v, v[1].u, v[1].v],
                            rgba: v[1].rgba,
                            texture,
                            modulate: true,
                            tcc: true,
                            bilinear: p.bilinear,
                            blend,
                            alpha_test_greater,
                            dither: Some(dimx),
                        },
                    );
                } else {
                    let strip: Vec<_> =
                        v.iter().map(|v| ([v.x, v.y], [v.u, v.v], v.rgba)).collect();
                    draw_strip(
                        &mut t,
                        &strip,
                        &Triangle {
                            xy: [[0.0; 2]; 3],
                            uv: [[0.0; 2]; 3],
                            rgba: [[0; 4]; 3],
                            gouraud: p.gouraud,
                            texture,
                            modulate: true,
                            tcc: true,
                            blend,
                            alpha_test_greater,
                            dither: Some(dimx),
                        },
                    );
                }
            }
            next += count;
            prim_count += count;
            draws += 1;
            let diff = t
                .pixels
                .iter()
                .zip(&want.pixels)
                .filter(|(a, b)| a != b)
                .count();
            if diff != 0 {
                bad.push((id, diff));
            }
        }
        eprintln!("built HUD: {prim_count} primitives in {draws} dumped draws rendered");
        assert_eq!(
            next,
            prims.len(),
            "every frame primitive belongs to a dumped draw"
        );
        assert!(bad.is_empty(), "differing draws: {bad:?}");
    }

    /// The public rendering API: `HudTables::from_files`, `HudTextures::new`
    /// and `render_primitives` draw the round-2 `frame()` output into each
    /// dumped draw's frame exactly as the game did (only the VU1 tile picture
    /// is supplied from the dump); `render` and `render_layer` handle the
    /// whole frame.
    #[test]
    #[ignore = "Requires the user's executable and ISO, the private savestates, getters and GS dump"]
    fn private_hud_render_api_matches_the_dump() {
        use roses_client::duel_hud::{
            frame, render, render_layer, render_primitives, HudTables, HudTextures,
            Target as HudTarget,
        };
        let elf = std::fs::read(env("ROSES_TEST_ELF_PATH")).unwrap();
        let dump = env("ROSES_TEST_GS_DUMP");
        let (mut disc, _) = crate::open_pal(&env("ROSES_TEST_ISO_PATH")).unwrap();
        let texsys = disc.read_file("DATA/TEXSYS.MRG", 2 * 1024 * 1024).unwrap();
        let picmini = disc.read_file("DATA/PICMINI.MRG", 4 * 1024 * 1024).unwrap();
        let message_file = disc.read_file("MSG/MSG_E.DAT", 300_000).unwrap();
        let tables = HudTables::from_files(&elf, &message_file).unwrap();
        let mut textures = HudTextures::new(&texsys, &picmini, [392, 397], &message_file).unwrap();
        let name = "weevil-round2-start";
        let session = env("ROSES_TEST_SESSION");
        let state_path = [
            session.join(format!("{name}.p2s")),
            session
                .parent()
                .unwrap()
                .join("checkpoints")
                .join(format!("{name}.p2s")),
        ]
        .into_iter()
        .find(|p| p.exists())
        .unwrap();
        let getters: serde_json::Value = serde_json::from_slice(
            &std::fs::read(
                env("ROSES_TEST_HUD_GOLDEN")
                    .parent()
                    .unwrap()
                    .join("golden-getters")
                    .join(format!("{name}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        let hud = frame(
            &super::primitives::hud_state(&state_path, &getters),
            &tables,
        );
        let file = |id: u32, kind: &str| {
            std::fs::read_dir(&dump)
                .unwrap()
                .map(|e| e.unwrap().path())
                .find(|p| {
                    let n = p.file_name().unwrap().to_string_lossy().to_string();
                    n.starts_with(&format!("{id:05}_f")) && n.contains(kind) && n.contains("C_16S")
                })
                .unwrap_or_else(|| panic!("draw {id}: {kind}"))
        };
        let rt = target(&file(6933, "_itexraw_02000"));
        textures.render_target = Some(Texture::from_c16(rt.width, rt.height, &rt.pixels, 0, 128));
        // The dump's DIMX is the one the tables carry.
        let ctx = context(&dump.join("06934_context.txt"));
        let dimx = (0..4)
            .flat_map(|y| (0..4).map(move |x| (x, y)))
            .fold(0u64, |acc, (x, y)| {
                acc | (num(&ctx, &format!("DIMX.DM{y}{x}")) as u64 & 7) << ((y * 4 + x) * 4)
            });
        assert_eq!(tables.dimx, dimx);
        let prims: Vec<_> = hud
            .prims
            .iter()
            .filter(|p| p.target == HudTarget::Frame)
            .cloned()
            .collect();
        let (mut next, mut bad) = (0, Vec::new());
        for id in (6929..=6989).filter(|&id| !(6930..=6932).contains(&id)) {
            let ctx = context(&dump.join(format!("{id:05}_context.txt")));
            let verts = vertices(&dump.join(format!("{id:05}_vertex.txt")));
            let count = if num(&ctx, "PRIM.PRIM") == 6 {
                verts.len() / 2
            } else {
                verts.len() / 6
            };
            let mut t = target(&file(id, "_rt0_"));
            let want = target(&file(id, "_rt1_"));
            let skipped =
                render_primitives(&prims[next..next + count], &tables, &mut textures, &mut t);
            assert!(skipped.is_empty(), "draw {id}: {skipped:?}");
            next += count;
            let diff = t
                .pixels
                .iter()
                .zip(&want.pixels)
                .filter(|(a, b)| a != b)
                .count();
            if diff != 0 {
                bad.push((id, diff));
            }
        }
        assert_eq!(next, prims.len());
        assert!(bad.is_empty(), "differing draws: {bad:?}");
        // Whole frame: nothing skipped; the layer is opaque on the LP panel
        // gradient and clear on the board.
        let mut full = Target16 {
            width: 512,
            height: 512,
            pixels: vec![0; 512 * 512],
        };
        assert!(render(&hud, &tables, &mut textures, &mut full).is_empty());
        let (layer, skipped) = render_layer(&hud, &tables, &mut textures, 512, 512);
        assert!(skipped.is_empty());
        assert_eq!(layer[(50 * 512 + 100) * 4 + 3], 255);
        assert_eq!(layer[(300 * 512 + 256) * 4 + 3], 0);
        let partial = layer.chunks(4).filter(|p| p[3] > 0 && p[3] < 255).count();
        assert_eq!(full.to_rgba8(0, 0, 512, 512).len(), 512 * 512 * 4);
        eprintln!(
            "render API: {} primitives in 58 dumped draws identical; layer: {partial} translucent pixels",
            prims.len()
        );
    }
}

#[cfg(test)]
mod primitives {
    use roses_client::duel_hud::{
        build, frame, Element, HandOverlayCard, HudSide, HudState, HudTables, HudUnit, PrimKind,
        TextureSource, UnitStats, UnsupportedKind,
    };
    use roses_compare::pine::{MemoryWatch, ReadWidth};
    use roses_compare::state::capture_state;
    use serde_json::Value;
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::{Path, PathBuf};

    const SIDE: u32 = 0x2D_ABA0;
    const UNIT: u32 = 0x2D_B990;
    const CELLS: u32 = 0x2D_C570;

    fn env(name: &str) -> PathBuf {
        PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("{name}")))
    }

    /// Byte-wise reads of one savestate's EE RAM through `capture_state`.
    struct Ram {
        path: PathBuf,
        bytes: BTreeMap<u32, u8>,
    }

    impl Ram {
        fn load(&mut self, ranges: &[(u32, u32)]) {
            let want: BTreeSet<u32> = ranges
                .iter()
                .flat_map(|&(a, n)| a..a + n)
                .filter(|a| !self.bytes.contains_key(a))
                .collect();
            let want: Vec<u32> = want.into_iter().collect();
            for chunk in want.chunks(4096) {
                let watches: Vec<MemoryWatch> = chunk
                    .iter()
                    .map(|&address| MemoryWatch {
                        name: format!("{address:x}"),
                        address,
                        width: ReadWidth::Bits8,
                    })
                    .collect();
                for o in capture_state(&self.path, &watches).unwrap().observations {
                    self.bytes.insert(o.address, o.value as u8);
                }
            }
        }
        fn b(&self, a: u32) -> u8 {
            self.bytes[&a]
        }
        fn h(&self, a: u32) -> u16 {
            u16::from_le_bytes([self.b(a), self.b(a + 1)])
        }
        fn w(&self, a: u32) -> u32 {
            u32::from_le_bytes([self.b(a), self.b(a + 1), self.b(a + 2), self.b(a + 3)])
        }
    }

    fn record(group: u32, slot: u32) -> u32 {
        UNIT + group * 0x3F4 + slot * 0x5C
    }

    fn stats(v: &Value) -> UnitStats {
        let n = |i: usize| v[i].as_i64().unwrap() as i32;
        UnitStats {
            attack: n(0),
            defence: n(1),
            terrain_effect: n(2),
            allowance: n(3),
        }
    }

    /// 0x2154C0(side, card): u16 & 0x3FFF of the side's saved card table
    /// (block 0 when side == [0x312B50]); card 671 counts as 999, ids ≥ 683
    /// and sides outside 0..2 have none (−1).
    fn rank_address(ram: &Ram, side: u8, card: u16) -> Option<u32> {
        let card = if card == 671 { 999 } else { card };
        (side < 2 && card < 683).then(|| {
            let block = u32::from(side != ram.b(0x31_2B50));
            0x2F_2C10 + block * 0xFFA0 + u32::from(card) * 0x4C + 0x48
        })
    }

    /// The HUD state of a savestate: plain RAM fields read from the .p2s;
    /// engine-rule results (`getters`) from the game's own getters.
    pub(super) fn hud_state(path: &Path, getters: &Value) -> HudState {
        let mut ram = Ram {
            path: path.to_path_buf(),
            bytes: BTreeMap::new(),
        };
        let side_base = |s: u32| SIDE + s * 0x350;
        ram.load(&[
            (0x2D_B8D0, 0x40),
            (0x2D_B268, 8),
            (0x2D_B828, 2),
            // 0x1A8C50(30): word 30 of the table at 0x2DA950 + 0x140.
            (0x2D_AB08, 4),
            // 0x1AB8B0(1): byte 0x2DA950 + 510 + [0x2DA950 + 558].
            (0x2D_AB4E, 0x34),
            (0x31_2B50, 1),
            (0x2D_E2C0, 84 * 4),
            (side_base(0) + 0x200, 0x150),
            (side_base(1) + 0x200, 0x150),
            (UNIT, 3 * 0x3F4),
            (CELLS, 49 * 28),
        ]);
        let g = |o: u32| ram.b(SIDE + o);
        let mut st = HudState {
            d35: g(0xD35),
            d3c: g(0xD3C),
            d3d: g(0xD3D),
            d3e: g(0xD3E),
            d3f: g(0xD3F),
            d40: g(0xD40),
            d41: g(0xD41),
            d42: g(0xD42),
            d43: g(0xD43),
            d44: g(0xD44),
            d45: g(0xD45),
            d47: g(0xD47),
            d48: g(0xD48),
            d4f: g(0xD4F),
            d53: g(0xD53),
            d54: g(0xD54),
            d57: g(0xD57),
            d65: g(0xD65),
            d66: g(0xD66),
            d67: g(0xD67),
            flash: ram.h(0x2D_B828),
            option_30: ram.w(0x2D_AB08) != 0,
            overlay_mode: ram.b(0x2D_AB4E + u32::from(ram.b(0x2D_AB7E))) as i8,
            ..HudState::default()
        };
        let d4e = g(0xD4E);
        // Unit records of groups 0..2 (group 2 holds the hand cards).
        let mut index = BTreeMap::new();
        let mut rank_reads = Vec::new();
        for group in 0..3 {
            for slot in 0..11 {
                let p = record(group, slot);
                index.insert(p, st.units.len());
                let (side, card) = (ram.b(p + 0x44), ram.h(p + 0x3C));
                rank_reads.extend(rank_address(&ram, side, card).map(|a| (a, 2)));
                st.units.push(HudUnit {
                    has_card: ram.w(p) != 0,
                    card,
                    kind: ram.b(p + 0x3E),
                    attribute: ram.b(p + 0x3F),
                    cell: [ram.b(p + 0x40), ram.b(p + 0x41)],
                    side,
                    slot: ram.b(p + 0x45),
                    turns: [ram.b(p + 0x49), ram.b(p + 0x4A)],
                    flags: ram.b(p + 0x57),
                    flags2: ram.b(p + 0x58),
                    ..HudUnit::default()
                });
            }
        }
        ram.load(&rank_reads);
        for (&p, &i) in &index {
            let rank = rank_address(&ram, st.units[i].side, st.units[i].card)
                .map_or(-1, |a| i32::from(ram.h(a) & 0x3FFF));
            let u = &mut st.units[i];
            u.rank = rank;
            // 0x24F790: the group, or the other side when +0x57 bit 1 is set.
            u.controller = u.side;
            if u.side < 3 && u.slot < 11 {
                let r = record(u32::from(u.side), u32::from(u.slot));
                if ram.b(r + 0x45) < 11 && ram.b(r + 0x57) & 2 != 0 {
                    u.controller = u8::from(u.side == 0);
                }
            }
            if let Some(v) = getters["units"].get(format!("0x{p:X}")) {
                u.face_down_hidden = v["hidden"].as_i64().unwrap() != 0;
                u.effect = v["effect"].as_i64().unwrap() != 0;
                u.abilities = v["abilities"].as_u64().unwrap() as u32;
                u.legend_query = v["legend"].as_i64().unwrap() != 0;
                u.at_cursor = stats(&v["at_cursor"]);
                if !v["at_own_cell"].is_null() {
                    u.at_own_cell = stats(&v["at_own_cell"]);
                }
            }
        }
        let ptr = |p: u32| (p != 0).then(|| index.get(&p).copied()).flatten();
        for cell in 0..49u32 {
            let c = CELLS + cell * 28;
            st.board[cell as usize] = ptr(ram.w(c));
            st.terrain[cell as usize] = ram.b(c + 0x16);
        }
        st.unit_a = ptr(ram.w(0x2D_B268));
        st.unit_b = ptr(ram.w(0x2D_B26C));
        for s in 0..2u32 {
            let b = side_base(s);
            let count = |lo: u32, hi: u32| {
                (lo..=hi)
                    .filter(|&slot| ram.b(record(s, slot) + 0x45) < 11)
                    .count() as u8
            };
            st.sides[s as usize] = HudSide {
                lp: ram.h(b + 0x33A),
                controller: ram.b(b + 0x33C),
                cursor: [ram.b(b + 0x33D), ram.b(b + 0x33E)],
                stars: ram.b(b + 0x341),
                deck: ram.b(b + 0x230),
                hand: ram.b(b + 0x32E),
                summon_flags: ram.b(b + 0x346),
                summon_counter: ram.b(b + 0x344),
                monsters: count(1, 5),
                magic_traps: count(6, 10),
            };
        }
        // 0x1E07C0(2, d4f + 1): the selected hand card's record.
        let hand_slot = u32::from(st.d4f) + 1;
        let hand = (hand_slot < 11)
            .then(|| record(2, hand_slot))
            .filter(|&r| ram.b(r + 0x45) < 11);
        st.hand_card = hand.and_then(ptr);
        let card_record = hand.map_or(0, |r| ram.w(r));
        // 0x1F2580(T + 74): the terrain tile model under the cursor.
        let cursor = st.sides[usize::from(st.d3c & 1)].cursor;
        let (col, row) = (i32::from(cursor[0]), i32::from(cursor[1]));
        let terrain = if (0..7).contains(&col) && (0..7).contains(&row) {
            i32::from(st.terrain[(row * 7 + col) as usize])
        } else {
            -1
        };
        let model = match u32::try_from(terrain + 74) {
            Ok(i) if i < 84 => ram.w(0x2D_E2C0 + 4 * i),
            _ => 0,
        };
        // 0x1AFF00: hand records (2, i + 1) of side d54 → card object floats.
        let d54 = u32::from(st.d54 & 1);
        let hand_count = u32::from(ram.b(side_base(d54) + 0x32E));
        let objects: Vec<u32> = (0..hand_count)
            .map(|i| {
                let r = record(2, i + 1);
                if i + 1 < 11 && ram.b(r + 0x45) < 11 {
                    ram.w(r + 4)
                } else {
                    0
                }
            })
            .collect();
        let mut more = Vec::new();
        if card_record != 0 {
            more.push((card_record + 2, 1));
        }
        if model != 0 {
            more.extend([(model + 498, 1), (model + 524, 1)]);
        }
        more.extend(objects.iter().filter(|&&o| o != 0).map(|&o| (o, 8)));
        ram.load(&more);
        st.terrain_model_ready =
            model != 0 && (ram.b(model + 498) as i8) >= 0 && ram.b(model + 524) & 2 != 0;
        // 0x1E1B70(2, d4f + 1, 0): card level (record byte 2, high nibble)
        // minus the record's +0x48, clamped to 0..=12; −1 without a card.
        let left = match hand {
            Some(r) if card_record != 0 => {
                (i32::from(ram.b(card_record + 2) >> 4) - i32::from(ram.b(r + 0x48))).clamp(0, 12)
            }
            _ => -1,
        };
        let right = if d4e != 0 {
            getters["hand_box_right"].as_i64().unwrap() as i32
        } else {
            left
        };
        st.hand_box_values = [left, right];
        st.hand_overlay = objects
            .iter()
            .enumerate()
            .map(|(i, &o)| HandOverlayCard {
                position: (o != 0).then(|| {
                    let f = |a: u32| f32::from_bits(ram.w(a));
                    [f(o), f(o + 4)]
                }),
                // 0x1E0340(side, i): hand word (id | fusion order << 12).
                word: if i < 5 {
                    i32::from(ram.h(side_base(d54) + 0x324 + 2 * i as u32))
                } else {
                    -1
                },
                // The HUD layer does not draw the card faces (0x1AFAC0).
                face: None,
            })
            .collect();
        st.advantage = getters["advantage"].as_i64().map(|v| v as i32);
        st.legend_1fb900 = getters["legend_1fb900"].as_i64().unwrap() != 0;
        st.legend_1e3760 = getters["legend_1e3760"].as_i64().unwrap() != 0;
        st
    }

    fn element_name(e: Element) -> String {
        match e {
            Element::TerrainPicture => "terrain_picture".into(),
            Element::LpPanel(s) => format!("lp_panel/side{s}"),
            Element::CardBox => "card_box".into(),
            Element::LegendBox => "legend_box".into(),
            Element::TerrainBox => "terrain_box".into(),
            Element::RoundBox => "round_box".into(),
            Element::RoundNumber => "round_number".into(),
            Element::Banner => "banner".into(),
            Element::HandBoxLeft => "hand_box_left".into(),
            Element::HandBoxRight => "hand_box_right".into(),
            Element::HandOverlay => "hand_overlay".into(),
            Element::MessageBox => "message_box".into(),
            Element::GraveyardPanel => "graveyard_panel".into(),
        }
    }

    fn texture_name(t: TextureSource) -> Value {
        match t {
            TextureSource::None => Value::Null,
            TextureSource::Atlas(s) => format!("TEXSYS#{s}").into(),
            TextureSource::TextCanvas { .. } => "text_canvas".into(),
            TextureSource::Portrait(_) => "portrait".into(),
            TextureSource::RenderTarget => "render_target".into(),
            TextureSource::Picture => "picture".into(),
        }
    }

    pub(super) fn hud_messages(message_file: &[u8]) -> BTreeMap<u16, Vec<u16>> {
        let ids = HudTables::required_messages();
        let mut out = BTreeMap::new();
        for chunk in ids.chunks(256) {
            for m in roses_assets::parse_pal_messages(message_file, chunk)
                .unwrap()
                .messages
            {
                out.insert(m.message_id, m.expanded_codes);
            }
        }
        out
    }

    #[test]
    #[ignore = "Requires the user's executable and ISO, private savestates and HUD golden lists"]
    fn private_hud_primitives_match_the_game() {
        let elf = std::fs::read(env("ROSES_TEST_ELF_PATH")).unwrap();
        let (mut disc, _) = crate::open_pal(&env("ROSES_TEST_ISO_PATH")).unwrap();
        let message_file = disc.read_file("MSG/MSG_E.DAT", 300_000).unwrap();
        let tables = HudTables::from_executable(&elf, hud_messages(&message_file)).unwrap();
        let golden_dir = env("ROSES_TEST_HUD_GOLDEN");
        let getters_dir = golden_dir.parent().unwrap().join("golden-getters");
        let session = env("ROSES_TEST_SESSION");
        let mut goldens: Vec<PathBuf> = std::fs::read_dir(&golden_dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect();
        goldens.sort();
        let (mut passed, mut prims_checked, mut failures) = (0, 0, Vec::new());
        for path in &goldens {
            let golden: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            let name = golden["savestate"].as_str().unwrap().to_string();
            let state_path = [
                session.join(format!("{name}.p2s")),
                session
                    .parent()
                    .unwrap()
                    .join("checkpoints")
                    .join(format!("{name}.p2s")),
            ]
            .into_iter()
            .find(|p| p.exists())
            .unwrap_or_else(|| panic!("savestate {name}"));
            let getters: Value = serde_json::from_slice(
                &std::fs::read(getters_dir.join(format!("{name}.json"))).unwrap(),
            )
            .unwrap();
            let state = hud_state(&state_path, &getters);
            let hud = frame(&state, &tables);
            assert_eq!(hud.prims, build(&state, &tables));
            let want = golden["prims"].as_array().unwrap();
            let mut first_bad = None;
            for (i, w) in want.iter().enumerate() {
                let Some(p) = hud.prims.get(i) else {
                    first_bad = Some(format!("#{i}: missing, the game drew {w}"));
                    break;
                };
                let verts: Vec<Value> = p
                    .verts
                    .iter()
                    .map(|v| {
                        serde_json::json!([
                            f64::from(v.x),
                            f64::from(v.y),
                            f64::from(v.u),
                            f64::from(v.v),
                            v.rgba[0],
                            v.rgba[1],
                            v.rgba[2],
                            v.rgba[3]
                        ])
                    })
                    .collect();
                let kind = match p.kind {
                    PrimKind::Sprite => "SPRITE",
                    PrimKind::TriStrip => "TRISTRIP",
                };
                let got = serde_json::json!({
                    "element": element_name(p.element),
                    "prim": kind,
                    "tex": texture_name(p.texture),
                    "abe": u8::from(p.blend.is_some()),
                    "alpha_test": p.alpha_test,
                    "verts": verts,
                });
                let numbers = |v: &Value| -> Vec<Vec<f64>> {
                    v.as_array()
                        .unwrap()
                        .iter()
                        .map(|x| {
                            x.as_array()
                                .unwrap()
                                .iter()
                                .map(|n| n.as_f64().unwrap())
                                .collect()
                        })
                        .collect()
                };
                let same = ["element", "prim", "tex", "abe", "alpha_test"]
                    .iter()
                    .all(|k| got[k] == w[k])
                    && numbers(&got["verts"]) == numbers(&w["verts"]);
                if !same {
                    first_bad = Some(format!("#{i}: game {w}\n      port {got}"));
                    break;
                }
            }
            if first_bad.is_none() && hud.prims.len() != want.len() {
                first_bad = Some(format!(
                    "{} primitives, the game drew {}",
                    hud.prims.len(),
                    want.len()
                ));
            }
            // Only the VU1 tile render may be left out in these states.
            let unsupported: Vec<_> = hud
                .unsupported
                .iter()
                .filter(|u| !matches!(u.kind, UnsupportedKind::TerrainTileRender { .. }))
                .collect();
            match first_bad {
                None if unsupported.is_empty() => {
                    passed += 1;
                    prims_checked += want.len();
                }
                None => failures.push(format!("{name}: unsupported {unsupported:?}")),
                Some(e) => failures.push(format!("{name}: {e}")),
            }
        }
        eprintln!(
            "HUD golden: {passed}/{} savestates, {prims_checked} primitives identical",
            goldens.len()
        );
        assert!(
            failures.is_empty(),
            "{passed}/{} match\n{}",
            goldens.len(),
            failures.join("\n")
        );
        assert!(passed >= 8, "{passed} savestates checked");
    }

    /// The GRAVEYARD panel (overlay slot 0 code 0x0E) against the game's
    /// own drawer 0x1B0120 run on the RAM of an emulated Mimicat pick
    /// (`golden-graveyard/`, `tools/graveyard_golden.py`: opening slide,
    /// cursor moves, list switch, page, closing slide, and RAM pokes for the
    /// locked / scrolled / empty / CPU-tab / half-open cases). The panel's
    /// inputs are the golden's `inputs` (slot-0 slide, d36, lists, offsets,
    /// rows, controllers, flash) and card byte 0 per card.
    #[test]
    #[ignore = "Requires the user's executable and ISO and the private graveyard golden lists"]
    fn private_graveyard_panel_matches_the_game() {
        use roses_client::duel_hud::{GraveyardEntry, GraveyardPanel};
        let elf = std::fs::read(env("ROSES_TEST_ELF_PATH")).unwrap();
        let (mut disc, _) = crate::open_pal(&env("ROSES_TEST_ISO_PATH")).unwrap();
        let message_file = disc.read_file("MSG/MSG_E.DAT", 300_000).unwrap();
        let tables = HudTables::from_executable(&elf, hud_messages(&message_file)).unwrap();
        let dir = env("ROSES_TEST_HUD_GOLDEN")
            .parent()
            .unwrap()
            .join("golden-graveyard");
        let mut goldens: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect();
        goldens.sort();
        let (mut passed, mut prims_checked, mut failures) = (0, 0, Vec::new());
        for path in &goldens {
            let golden: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            let name = golden["case"].as_str().unwrap().to_string();
            let inp = &golden["inputs"];
            let n = |v: &Value| v.as_i64().unwrap();
            let d36 = n(&inp["d36"]) as u8;
            let kind = |card: u16| {
                golden["card_byte0"][card.to_string()]
                    .as_i64()
                    .map(|k| k as u8)
            };
            let list = |l: usize| -> Vec<GraveyardEntry> {
                inp["lists"][l]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|c| {
                        let card = n(c) as u16;
                        GraveyardEntry {
                            card,
                            kind: kind(card),
                        }
                    })
                    .collect()
            };
            let lists = [list(0), list(1)];
            let mut st = HudState {
                d67: 2,
                flash: n(&inp["flash"]) as u16,
                ..HudState::default()
            };
            for s in 0..2 {
                st.sides[s].controller = n(&inp["controllers"][s]) as u8;
            }
            st.graveyard = Some(GraveyardPanel {
                lists,
                shown: d36 & 0x7F,
                locked: d36 & 0x80 != 0,
                offsets: [0, 1].map(|l| n(&inp["offsets"][l]) as u16),
                rows: [0, 1].map(|l| n(&inp["rows"][l]) as u8),
                slide: n(&inp["c9c"]) as i16,
            });
            let hud = frame(&st, &tables);
            let want = golden["prims"].as_array().unwrap();
            let mut first_bad = None;
            for (i, w) in want.iter().enumerate() {
                let Some(p) = hud.prims.get(i) else {
                    first_bad = Some(format!("#{i}: missing, the game drew {w}"));
                    break;
                };
                let verts: Vec<Vec<f64>> = p
                    .verts
                    .iter()
                    .map(|v| {
                        vec![
                            f64::from(v.x),
                            f64::from(v.y),
                            f64::from(v.u),
                            f64::from(v.v),
                            f64::from(v.rgba[0]),
                            f64::from(v.rgba[1]),
                            f64::from(v.rgba[2]),
                            f64::from(v.rgba[3]),
                        ]
                    })
                    .collect();
                let want_verts: Vec<Vec<f64>> = w["verts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| {
                        x.as_array()
                            .unwrap()
                            .iter()
                            .map(|n| n.as_f64().unwrap())
                            .collect()
                    })
                    .collect();
                let kind = match p.kind {
                    PrimKind::Sprite => "SPRITE",
                    PrimKind::TriStrip => "TRISTRIP",
                };
                let got = serde_json::json!({
                    "element": element_name(p.element),
                    "prim": kind,
                    "tex": texture_name(p.texture),
                    "abe": u8::from(p.blend.is_some()),
                    "alpha_test": p.alpha_test,
                });
                let same = ["element", "prim", "tex", "abe", "alpha_test"]
                    .iter()
                    .all(|k| got[k] == w[k])
                    && verts == want_verts
                    && p.z == roses_client::duel_hud::WINDOW_Z;
                if !same {
                    first_bad = Some(format!("#{i}: game {w}\n      port {got} {verts:?}"));
                    break;
                }
            }
            if first_bad.is_none() && hud.prims.len() != want.len() {
                first_bad = Some(format!(
                    "{} primitives, the game drew {}",
                    hud.prims.len(),
                    want.len()
                ));
            }
            match first_bad {
                None if hud.unsupported.is_empty() => {
                    passed += 1;
                    prims_checked += want.len();
                }
                None => failures.push(format!("{name}: unsupported {:?}", hud.unsupported)),
                Some(e) => failures.push(format!("{name}: {e}")),
            }
        }
        eprintln!(
            "GRAVEYARD golden: {passed}/{} cases, {prims_checked} primitives identical",
            goldens.len()
        );
        assert!(
            failures.is_empty(),
            "{passed}/{} match\n{}",
            goldens.len(),
            failures.join("\n")
        );
        assert!(passed >= 40, "{passed} cases checked");
    }
}
