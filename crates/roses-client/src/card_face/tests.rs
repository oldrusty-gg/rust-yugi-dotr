use super::*;

fn mode(width: u16, height: u16, star: [u16; 4], layout: [[i32; 2]; 8]) -> ModeTables {
    let d = |slot: u8, uv: [u16; 4]| Descriptor { slot, uv };
    ModeTables {
        frames: (0..10)
            .map(|k| {
                let u = (k % 2) as u16 * width;
                d(k as u8 / 3, [u, 0, u + width, height])
            })
            .collect(),
        star: d(6, star),
        layout,
    }
}

fn tables() -> FaceTables {
    FaceTables {
        large: mode(
            256,
            368,
            [72, 24, 88, 40],
            [
                [18, 24],
                [212, 24],
                [26, 60],
                [32, 86],
                [110, 300],
                [117, 300],
                [110, 324],
                [117, 324],
            ],
        ),
        small: mode(
            120,
            180,
            [72, 40, 80, 48],
            [
                [0, 0],
                [0, 0],
                [13, 6],
                [12, 19],
                [8, 123],
                [12, 123],
                [8, 143],
                [12, 143],
            ],
        ),
        icons: (0..9)
            .map(|k| Descriptor {
                slot: 6,
                uv: [k as u16 * 24, 0, k as u16 * 24 + 24, 24],
            })
            .collect(),
    }
}

fn monster(level: u8, attack: i32, defense: i32, effect: bool) -> FaceCard {
    FaceCard {
        card: 1,
        record: CardRecordBytes {
            category: 0,
            attribute: 2,
            effect_index: if effect { 3 } else { 0xFFFF },
        },
        level,
        attack,
        defense,
        effect,
        name_width: 216,
    }
}

#[test]
fn a_large_monster_face_has_frame_stars_picture_icon_name_and_values() {
    let s = face_sprites(&monster(5, 1900, 0, false), &tables(), FaceSize::Large).unwrap();
    // frame, 5 stars, picture, icon, name, ATK label + 4 digits, DEF label + 1 digit.
    assert_eq!(s.len(), 1 + 5 + 1 + 1 + 1 + 5 + 2);
    assert_eq!(s[1].rect, [213, 60, 229, 76]);
    assert_eq!(s[5].rect, [145, 60, 161, 76]);
    assert_eq!(s[6].texture, FaceTexture::Picture);
    assert_eq!(s[6].rect, [32, 86, 224, 278]);
    assert_eq!(s[7].uv, [48, 0, 72, 24]);
    assert_eq!(s[8].rect, [18, 24, 210, 46]);
    assert_eq!(s[8].uv, [0, 198, 216, 220]);
    let digits: Vec<_> = s[10..14]
        .iter()
        .map(|d| (d.rect[0], d.uv[0] / DIGIT_CELL))
        .collect();
    assert_eq!(digits, [(205, 0), (183, 0), (161, 9), (139, 1)]);
    assert_eq!(s[15].rect[0], 205);
}

#[test]
fn a_small_face_has_no_name_or_icon_and_packs_digits_twenty_apart() {
    let s = face_sprites(&monster(5, 1900, 1400, false), &tables(), FaceSize::Small).unwrap();
    assert_eq!(s[0].rect, [0, 0, 120, 180]);
    assert_eq!(s[1].rect, [101, 6, 109, 14]);
    assert_eq!(s[5].rect, [69, 6, 77, 14]);
    assert_eq!(s[6].rect, [12, 19, 108, 115]);
    assert_eq!(s[7].rect, [8, 123, 30, 145]);
    let atk: Vec<_> = s[8..12].iter().map(|d| d.rect[0]).collect();
    assert_eq!(atk, [92, 72, 52, 32]);
    assert!(s
        .iter()
        .all(|d| !(d.texture == FaceTexture::TextPage && d.uv[1] == NAME_ROW * ROW_HEIGHT)));
}

#[test]
fn effect_monsters_use_the_sixth_frame_after_their_category_and_spells_have_no_values() {
    let t = tables();
    assert_eq!(
        face_sprites(&monster(1, 0, 0, true), &t, FaceSize::Large).unwrap()[0].uv,
        t.large.frames[6].uv.map(u32::from)
    );
    let mut spell = monster(0, 0, 0, false);
    spell.record = CardRecordBytes {
        category: 1,
        attribute: 0,
        effect_index: 0,
    };
    let s = face_sprites(&spell, &t, FaceSize::Large).unwrap();
    assert_eq!(s.len(), 4);
    assert_eq!(s[2].uv, t.icons[6].uv.map(u32::from));
}

#[test]
fn composing_blends_and_marks_drawn_pixels_opaque() {
    let mut frame = RgbaImage {
        width: 512,
        height: 368,
        pixels: vec![0; 512 * 368 * 4],
    };
    for p in frame.pixels.chunks_exact_mut(4) {
        p.copy_from_slice(&[200, 100, 50, 128]);
    }
    let page = blank_text_page();
    let lookup = |slot: u8| (slot == 0).then_some(&frame);
    let textures = FaceTextures {
        texsys: &lookup,
        picture: None,
        text_page: &page,
    };
    let sprites = vec![sprite(
        FaceTexture::Texsys(0),
        [0, 0, 256, 368],
        [0, 0, 256, 368],
    )];
    let face = compose(&sprites, &textures, FaceSize::Large).unwrap();
    assert_eq!(&face.pixels[..4], &[200, 100, 50, 255]);
}

mod private_tests {
    use super::*;
    use serde_json::Value;
    use std::path::PathBuf;

    fn import() -> PathBuf {
        PathBuf::from(std::env::var_os("ROSES_DUEL_IMPORT").expect("ROSES_DUEL_IMPORT"))
    }

    fn hex(v: &Value) -> i64 {
        i64::from_str_radix(v.as_str().unwrap().trim_start_matches("0x"), 16).unwrap()
    }

    /// Compare our sprite lists with an oracle file (`oracle_faces.py` or
    /// `oracle_small.py`). Values (level, ATK, DEF) are the ones the
    /// original read (0x1E1BD0, 0x1E1570, 0x1E1650 returns), which include
    /// record bonuses outside this module.
    fn compare_oracle(oracle_env: &str, size: FaceSize, frame_tbp: u64) {
        let exe = std::fs::read(import().join("SLES_515.79")).unwrap();
        let msg = std::fs::read(import().join("MSG_E.DAT")).unwrap();
        let oracle_path = std::env::var_os(oracle_env).expect(oracle_env);
        let oracle: Value = serde_json::from_slice(&std::fs::read(oracle_path).unwrap()).unwrap();
        let tables = FaceTables::from_executable(&exe).unwrap();
        let ramp = text_alpha_ramp(&exe).unwrap();
        let font = roses_assets::decode_pal_name_font(&msg).unwrap();
        let ids: Vec<u16> = (0..CARDS).map(|c| NAME_MESSAGE_BASE + c).collect();
        let mut messages = Vec::new();
        for chunk in ids.chunks(200) {
            messages.extend(
                roses_assets::parse_pal_messages(&msg, chunk)
                    .unwrap()
                    .messages,
            );
        }
        let mut differences = Vec::new();
        for entry in oracle.as_array().unwrap() {
            let card = entry["card"].as_u64().unwrap() as u16;
            let calls = entry["calls"].as_array().unwrap();
            let ret = |addr: &str| {
                calls
                    .iter()
                    .find(|call| call["addr"] == addr)
                    .map_or(0, |call| hex(&call["ret"]))
            };
            let mut page = blank_text_page();
            let width = match render_text_row(
                &mut page,
                &font,
                &messages[usize::from(card)].expanded_codes,
                NAME_ROW,
                &ramp,
            ) {
                Ok(w) => w,
                Err(e) => {
                    differences.push(format!("card {card}: name {e}"));
                    continue;
                }
            };
            let record = card_record(&exe, card).unwrap();
            // 0x267A00 terrain bonuses: ATK first (a3 = 1), then DEF.
            let bonus: Vec<i64> = calls
                .iter()
                .filter(|call| call["addr"] == "0x267a00")
                .map(|call| hex(&call["ret"]) as i32 as i64)
                .collect();
            let face = FaceCard {
                card,
                record,
                level: ret("0x1e1bd0").max(0) as u8,
                attack: (ret("0x1e1570") + bonus.first().copied().unwrap_or(0)) as i32,
                defense: (ret("0x1e1650") + bonus.get(1).copied().unwrap_or(0)) as i32,
                effect: record.has_effect(),
                name_width: width,
            };
            let ours = match face_sprites(&face, &tables, size) {
                Ok(s) => s,
                Err(e) => {
                    differences.push(format!("card {card}: {e}"));
                    continue;
                }
            };
            let slot = calls
                .iter()
                .find(|call| call["addr"] == "0x1daec0")
                .map_or(0, |call| hex(&call["args"][1]) as u8);
            let mut theirs = Vec::new();
            let mut name_width = None;
            for s in entry["sprites"].as_array().unwrap() {
                if let Some(reg) = s.get("trxreg") {
                    if name_width.is_none() {
                        name_width = reg[0].as_u64();
                    }
                    continue;
                }
                let Some(tbp) = s.get("tbp").and_then(Value::as_u64) else {
                    continue;
                };
                let texture = match tbp {
                    t if t == frame_tbp => FaceTexture::Texsys(slot),
                    12032 => FaceTexture::Texsys(6),
                    13120 => FaceTexture::Picture,
                    10496 => FaceTexture::TextPage,
                    other => panic!("card {card}: unknown texture {other}"),
                };
                let f = |v: &Value| v.as_f64().unwrap();
                let uv = [
                    f(&s["uv"][0][0]),
                    f(&s["uv"][0][1]),
                    f(&s["uv"][1][0]),
                    f(&s["uv"][1][1]),
                ]
                .map(|v| (v - 0.5) as u32);
                let rect = [
                    f(&s["xy"][0][0]),
                    f(&s["xy"][0][1]),
                    f(&s["xy"][1][0]),
                    f(&s["xy"][1][1]),
                ]
                .map(|v| v as i32);
                let rgba: Vec<u8> = s["rgba"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as u8)
                    .collect();
                theirs.push((texture, uv, rect, rgba));
            }
            let has_picture = theirs.iter().any(|t| t.0 == FaceTexture::Picture);
            let mine: Vec<_> = ours
                .iter()
                .filter(|s| has_picture || s.texture != FaceTexture::Picture)
                .map(|s| (s.texture, s.uv, s.rect, s.modulate.to_vec()))
                .collect();
            // The small oracle patches a hand record instead of rebuilding it
            // with 0x1DDFE0, so Shapesnatch (675) keeps another card's
            // record bonus there; its large entry (rebuilt) matches.
            if size == FaceSize::Small && card == 675 {
                continue;
            }
            if mine != theirs {
                differences.push(format!(
                    "card {card}: ours {mine:?}\n   original {theirs:?}"
                ));
            }
            if size == FaceSize::Large && card != BACK_CARD && name_width != Some(u64::from(width))
            {
                differences.push(format!("card {card}: name width {width} vs {name_width:?}"));
            }
        }
        assert!(
            differences.is_empty(),
            "{} differences:\n{}",
            differences.len(),
            differences[..differences.len().min(8)].join("\n")
        );
    }

    #[test]
    #[ignore = "Requires the private duel import and card-face oracle (ROSES_DUEL_IMPORT, ROSES_CARD_FACE_ORACLE)"]
    fn private_large_faces_match_the_composer_for_every_card() {
        compare_oracle("ROSES_CARD_FACE_ORACLE", FaceSize::Large, 12320);
    }

    #[test]
    #[ignore = "Requires the private duel import and small card-face oracle (ROSES_DUEL_IMPORT, ROSES_CARD_FACE_SMALL_ORACLE)"]
    fn private_small_faces_match_the_composer_for_every_card() {
        compare_oracle("ROSES_CARD_FACE_SMALL_ORACLE", FaceSize::Small, 11136);
    }

    #[test]
    #[ignore = "Requires the private duel import and the VRAM text page (ROSES_DUEL_IMPORT, ROSES_CARD_FACE_TEXT_PAGE)"]
    fn private_text_page_rows_match_the_original_vram() {
        // `duel/the project notes: 640×220 GS alpha of the
        // text page (TBP 10496) in savestate 437-deck1, whose last name is
        // card 612.
        let exe = std::fs::read(import().join("SLES_515.79")).unwrap();
        let msg = std::fs::read(import().join("MSG_E.DAT")).unwrap();
        let original = std::fs::read(
            std::env::var_os("ROSES_CARD_FACE_TEXT_PAGE").expect("ROSES_CARD_FACE_TEXT_PAGE"),
        )
        .unwrap();
        let ramp = text_alpha_ramp(&exe).unwrap();
        let font = roses_assets::decode_pal_name_font(&msg).unwrap();
        let ids = [
            DIGITS_MESSAGE,
            ATK_MESSAGE,
            DEF_MESSAGE,
            NAME_MESSAGE_BASE + 612,
        ];
        let messages = roses_assets::parse_pal_messages(&msg, &ids)
            .unwrap()
            .messages;
        let mut page = blank_text_page();
        let mut widths = Vec::new();
        for (row, m) in [0, 1, 2, NAME_ROW].iter().zip(&messages) {
            // The face reads only the ten digit cells of row 0.
            let codes = if *row == 0 {
                &m.expanded_codes[..DIGIT_GLYPHS]
            } else {
                &m.expanded_codes[..]
            };
            widths.push(render_text_row(&mut page, &font, codes, *row, &ramp).unwrap());
        }
        assert_eq!(widths, [220, 36, 36, 216]);
        // Beyond each row's width the page keeps older text (not compared).
        let mut differences = 0;
        for (row, width) in [0u32, 1, 2, NAME_ROW].into_iter().zip(widths) {
            for y in row * ROW_HEIGHT..(row + 1) * ROW_HEIGHT {
                for x in 0..width {
                    let ours = page.pixels[((y * page.width + x) * 4 + 3) as usize];
                    if ours != original[(y * 640 + x) as usize] {
                        differences += 1;
                    }
                }
            }
        }
        assert_eq!(differences, 0);
    }

    #[test]
    #[ignore = "Requires the private duel and duel-end imports (ROSES_DUEL_IMPORT, ROSES_DUEL_END_IMPORT, ROSES_CARD_FACE_DUMP)"]
    fn private_faces_compose_in_both_sizes() {
        let end = PathBuf::from(
            std::env::var_os("ROSES_DUEL_END_IMPORT").expect("ROSES_DUEL_END_IMPORT"),
        );
        let assets = FaceAssets::new(
            std::fs::read(import().join("SLES_515.79")).unwrap(),
            std::fs::read(import().join("MSG_E.DAT")).unwrap(),
            &std::fs::read(import().join("TEXSYS.MRG")).unwrap(),
            std::fs::read(end.join("PICTURE.MRG")).unwrap(),
        )
        .unwrap();
        let out =
            PathBuf::from(std::env::var_os("ROSES_CARD_FACE_DUMP").expect("ROSES_CARD_FACE_DUMP"));
        // Maiden of The Aqua (the 437-deck1 stack front card): level 4, 700/2000.
        for (card, level, attack, defense) in [
            (612u16, 4u8, 700, 2000),
            (392, 5, 1900, 1400),
            (700, 0, 0, 0),
        ] {
            for (size, tag) in [(FaceSize::Large, "large"), (FaceSize::Small, "small")] {
                let face = assets.face(card, level, attack, defense, size).unwrap();
                assert_eq!((face.width, face.height), size.dimensions());
                let mut raw = face.width.to_le_bytes().to_vec();
                raw.extend(face.height.to_le_bytes());
                raw.extend(&face.pixels);
                std::fs::write(out.join(format!("face-{tag}-{card}.rgba")), raw).unwrap();
            }
        }
    }
}
