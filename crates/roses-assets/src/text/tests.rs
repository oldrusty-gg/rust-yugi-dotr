use super::*;

fn pool(messages: &[Vec<u16>]) -> (Vec<u8>, Vec<u32>) {
    let mut bytes = Vec::new();
    let mut offsets = Vec::new();
    for message in messages {
        assert!(!message.is_empty());
        offsets.push((bytes.len() / 2) as u32);
        for (index, &token) in message.iter().enumerate() {
            let end = if index + 1 == message.len() {
                0x8000
            } else {
                0
            };
            bytes.extend_from_slice(&(token | end).to_le_bytes());
        }
    }
    (bytes, offsets)
}
fn reference(raw_offset: u16, count: u16) -> u16 {
    assert!(raw_offset < 128 && count < 64);
    0x4000 | (raw_offset << 6) | count
}

#[test]
fn template_references_count_raw_words_and_preserve_nested_leaf_identity() {
    let (bytes, offsets) = pool(&[
        vec![0x2005, 0x2006, 0x2007],
        vec![reference(0, 3), 0x4000],
        vec![reference(0, 2), 0x4001, 0x2008],
    ]);
    let decoded = decode_template_messages(&bytes, &offsets).unwrap();
    assert_eq!(
        decoded[2]
            .tokens
            .iter()
            .map(|atom| atom.code)
            .collect::<Vec<_>>(),
        [0x2005, 0x2006, 0x2007, 0x2008]
    );
    assert_eq!(
        decoded[2].tokens[2].source,
        MessageSource {
            message_id: 0,
            raw_token_offset: 2
        }
    );
    assert_eq!(
        decoded[2].tokens[3].source,
        MessageSource {
            message_id: 2,
            raw_token_offset: 2
        }
    );
    // Oversized source span ends at the source end flag, not subsequent text.
    let (bytes, offsets) = pool(&[vec![0x2005], vec![reference(0, 12), 0x4000, 0x2006]]);
    let decoded = decode_template_messages(&bytes, &offsets).unwrap();
    assert_eq!(
        decoded[1]
            .tokens
            .iter()
            .map(|atom| atom.code)
            .collect::<Vec<_>>(),
        [0x2005, 0x2006]
    );
}

fn synthetic_name_template() -> PalMessageTemplate {
    let mut variable = vec![0x2002, 0x2003];
    variable.extend([0x2002; 12]);
    let (bytes, offsets) = pool(&[
        vec![0x2005],
        vec![0x2006],
        vec![0x2007],
        variable,
        vec![0x2002, reference(2, 12), 0x4003, 0x2008],
        vec![reference(0, 4), 0x4004, 0x2000],
    ]);
    let decoded = decode_template_messages(&bytes, &offsets).unwrap();
    structure_template(5, &decoded[5]).unwrap()
}

#[test]
fn name_templates_omit_unused_slots_but_keep_explicit_spaces_and_literal_placeholder() {
    let template = synthetic_name_template();
    assert!(matches!(
        template.tokens[0],
        MessageTemplateToken::Direct {
            code: 0x2002,
            source: MessageSource {
                message_id: 4,
                raw_token_offset: 0
            }
        }
    ));
    assert!(matches!(
        template.tokens[1],
        MessageTemplateToken::PlayerNameSlot {
            slot: 0,
            source: MessageSource {
                message_id: 3,
                raw_token_offset: 2
            }
        }
    ));
    let resolved = resolve_pal_message_template(&template, &[0x2005, 0x2001, 0x2006]).unwrap();
    assert_eq!(
        resolved.expanded_codes,
        [0x2002, 0x2005, 0x2001, 0x2006, 0x2008, 0x2000]
    );
    assert_eq!(resolved.lines.len(), 2);
    let blank = resolve_pal_message_template(&template, &[0x2001]).unwrap();
    assert_eq!(blank.expanded_codes, [0x2002, 0x2001, 0x2008, 0x2000]);
    assert!(resolve_pal_message_template(&template, &[]).is_err());
    assert!(resolve_pal_message_template(&template, &[0x2005; 13]).is_err());
    assert!(resolve_pal_message_template(&template, &[0x2000]).is_err());
    assert!(resolve_pal_message_template(&template, &[0xA005]).is_err());
    assert!(resolve_pal_message_template(&template, &[0x00A4]).is_err());
    let initial_and_name = PalMessageTemplate {
        message_id: 3,
        tokens: [
            MessageTemplateToken::PlayerNameSlot {
                slot: 0,
                source: MessageSource {
                    message_id: 3,
                    raw_token_offset: 0,
                },
            },
            MessageTemplateToken::Direct {
                code: 0x2003,
                source: MessageSource {
                    message_id: 3,
                    raw_token_offset: 1,
                },
            },
            MessageTemplateToken::PlayerNameSlot {
                slot: 0,
                source: MessageSource {
                    message_id: 3,
                    raw_token_offset: 2,
                },
            },
            MessageTemplateToken::PlayerNameSlot {
                slot: 1,
                source: MessageSource {
                    message_id: 3,
                    raw_token_offset: 3,
                },
            },
        ]
        .to_vec(),
    };
    assert_eq!(
        resolve_pal_message_template(&initial_and_name, &[0x2007])
            .unwrap()
            .expanded_codes,
        [0x2007, 0x2003, 0x2007]
    );
}

#[test]
fn templates_reject_mid_pair_unbounded_cycles_and_unproven_name_slices() {
    for messages in [
        vec![vec![reference(0, 1), 0x4000]],
        vec![vec![0x2005], vec![reference(0, 1), 0]],
        vec![vec![0x2005], vec![reference(1, 1), 0x4000]],
        vec![
            vec![0x2005],
            vec![reference(0, 1), 0x4000],
            vec![reference(0, 1), 0x4001],
        ],
        vec![
            vec![0x2005],
            vec![reference(0, 1), 0x4000],
            vec![reference(1, 1), 0x4001],
        ],
    ] {
        let (bytes, offsets) = pool(&messages);
        assert!(decode_template_messages(&bytes, &offsets).is_err());
    }
    let (bytes, offsets) = pool(&[
        vec![0x2005; 63],
        [reference(0, 63), 0x4000].repeat(MAX_EXPANDED_TOKENS / 63 + 1),
    ]);
    assert!(decode_template_messages(&bytes, &offsets).is_err());
    assert!(decode_template_messages(&bytes, &[0, u32::MAX]).is_err());
    let invalid = PalMessageTemplate {
        message_id: 7,
        tokens: vec![MessageTemplateToken::PlayerNameSlot {
            slot: 4,
            source: MessageSource {
                message_id: 3,
                raw_token_offset: 6,
            },
        }],
    };
    assert!(resolve_pal_message_template(&invalid, &[0x2005]).is_err());
    let invalid = PalMessageTemplate {
        message_id: 7,
        tokens: vec![MessageTemplateToken::PlayerNameSlot {
            slot: 1,
            source: MessageSource {
                message_id: 4,
                raw_token_offset: 3,
            },
        }],
    };
    assert!(resolve_pal_message_template(&invalid, &[0x2005]).is_err());
    assert!(parse_pal_message_templates(b"unknown", &[2112]).is_err());
}

#[test]
fn references_use_compressed_token_boundaries_through_nested_expansion() {
    let (bytes, offsets) = pool(&[
        vec![0x2005, 0x2006, 0x2007, 0x2008],  // ABCD
        vec![reference(0, 3), 0x4000, 0x201C], // ABCX (raw X position is 2)
        vec![reference(2, 1), 0x4001, 0x201D], // XY, not CY
        vec![reference(0, 2), 0x4002],         // XY, with end bit on marked operand
    ]);
    let decoded = decode_messages(&bytes, &offsets).unwrap();
    let names: Vec<_> = decoded
        .iter()
        .map(|m| decode_name(&m.glyphs).unwrap())
        .collect();
    assert_eq!(names, ["ABCD", "ABCX", "XY", "XY"]);
    assert_eq!(decoded[1].raw_boundaries, [Some(0), None, Some(3)]);
}

#[test]
fn rejects_cyclic_forward_mid_pair_and_out_of_range_references() {
    let scenarios = [
        vec![vec![reference(0, 1), 0x4000]],
        vec![vec![reference(0, 1), 0x4001], vec![0x2005]],
        vec![vec![0x2005], vec![reference(1, 1), 0x4000]],
        vec![vec![0x2005], vec![reference(0, 2), 0x4000]],
        vec![vec![0x2005], vec![reference(0, 0), 0x4000]],
        vec![vec![0x2005], vec![reference(0, 1)]],
        vec![vec![0x2005], vec![reference(0, 1), 0]], // Missing operand marker.
        vec![
            vec![0x2005],
            vec![reference(0, 1), 0x4000],
            vec![reference(1, 1), 0x4001],
        ],
    ];
    for messages in scenarios {
        let (bytes, offsets) = pool(&messages);
        assert!(decode_messages(&bytes, &offsets).is_err());
    }
}

#[test]
fn rejects_offset_and_termination_errors_without_scanning_other_sections() {
    let (bytes, offsets) = pool(&[vec![0x2005], vec![0x2006]]);
    assert!(decode_messages(&bytes, &[]).is_err());
    assert!(decode_messages(&bytes, &[1]).is_err());
    assert!(decode_messages(&bytes, &[0, 0]).is_err());
    assert!(decode_messages(&bytes, &[0, u32::MAX]).is_err());
    let mut missing_end = bytes.clone();
    missing_end[1] &= 0x7F;
    assert!(decode_messages(&missing_end, &offsets).is_err());
    let early_end = [0x05, 0xA0, 0, 0, 0x06, 0xA0];
    assert!(decode_messages(&early_end, &[0, 2]).is_err());
    assert!(decode_messages(&[0x05], &[0]).is_err());
    let mut trailer = bytes;
    trailer.extend_from_slice(b"uninterpreted trailer");
    assert_eq!(decode_messages(&trailer, &offsets).unwrap().len(), 2);
}

#[test]
fn raw_scan_and_expansion_limits_stop_amplification() {
    let (bytes, offsets) = pool(&[vec![0x2005; MAX_RAW_TOKENS + 1]]);
    assert!(decode_messages(&bytes, &offsets).is_err());
    let (bytes, offsets) = pool(&[
        vec![0x2005; 63],
        [reference(0, 63), 0x4000].repeat(MAX_EXPANDED_TOKENS / 63 + 1),
    ]);
    assert!(decode_messages(&bytes, &offsets).is_err());
    let (bytes, offsets) = pool(&vec![vec![0x2005; 256]; MESSAGE_COUNT]);
    assert!(decode_messages(&bytes, &offsets).is_err());
}

#[test]
fn supported_name_glyphs_keep_case_digits_and_punctuation() {
    let glyphs = [
        0x2016, 0x2033, 0x202C, 0x2023, 0x2003, 0x2001, 0x2005, 0x2041, 0x2043, 0x2042, 0x2001,
        0x2047, 0x2001, 0x204A, 0x2001, 0x204B, 0x2038, 0x2072,
    ];
    assert_eq!(decode_name(&glyphs).unwrap(), "Rune, A9-0 # & 'z.");
    for unsupported in [0x2000, 0x204C, 0x204D, 0x0005, 0x6005] {
        assert!(decode_name(&[unsupported]).is_err());
    }
    assert!(decode_name(&[]).is_err());
    assert!(decode_name(&[0x2005; MAX_NAME_CHARACTERS + 1]).is_err());
}

#[test]
fn rejects_unknown_message_files_and_truncated_container_headers() {
    assert!(matches!(
        parse_pal_card_names(b"MWo3"),
        Err(Error::UnsupportedMessageFile { .. })
    ));
    assert!(parse_container(b"MWo3").is_err());
    let mut invalid = vec![0; MESSAGE_BYTES];
    invalid[..4].copy_from_slice(b"MWo3");
    assert!(parse_container(&invalid).is_err());
}

#[test]
fn selected_messages_preserve_order_and_reject_ambiguous_requests() {
    assert!(validate_message_selection(&[17, 3, 0]).is_ok());
    assert!(validate_message_selection(&[]).is_err());
    assert!(validate_message_selection(&[17, 3, 17]).is_err());
    assert!(validate_message_selection(&[MESSAGE_COUNT as u16]).is_err());
    assert!(validate_message_selection(&(0..257).collect::<Vec<_>>()).is_err());
    assert!(parse_pal_messages(b"unknown", &[0]).is_err());
    assert!(decode_pal_message_font(b"unknown").is_err());
    assert!(verified_message_hash(&vec![0; MESSAGE_BYTES + 1]).is_err());
}

#[test]
fn message_lines_retain_blank_lines_unknowns_and_original_codes() {
    let codes = [
        0x2000, 0x2005, 0x209C, 0x2000, 0x2000, 0x00A4, 0x6005, 0x2000,
    ];
    let message = structure_message(7, &codes);
    assert_eq!(message.message_id, 7);
    assert_eq!(message.expanded_codes, codes);
    assert_eq!(message.lines.len(), 5);
    assert!(message.lines[0].tokens.is_empty());
    assert!(message.lines[2].tokens.is_empty());
    assert!(message.lines[4].tokens.is_empty());
    assert_eq!(
        message.lines[1].tokens,
        [
            MessageToken::Glyph {
                code: 0x2005,
                atlas_index: 5,
                character: Some('A')
            },
            MessageToken::Glyph {
                code: 0x209C,
                atlas_index: 0x9C,
                character: Some(':')
            },
        ]
    );
    assert_eq!(
        message.lines[3].tokens,
        [
            MessageToken::Unknown { code: 0x00A4 },
            MessageToken::Unknown { code: 0x6005 },
        ]
    );
    // A recognized glyph need not have a guessed Unicode transcription.
    assert_eq!(
        structure_message(8, &[0x2096]).lines[0].tokens,
        [MessageToken::Glyph {
            code: 0x2096,
            atlas_index: 0x96,
            character: None
        }]
    );
}

#[test]
fn font_nibbles_and_atlas_row_boundaries_are_exact() {
    let mut source = vec![0; 17 * FONT_GLYPH_BYTES];
    source[0] = 0xF1;
    source[FONT_GLYPH_BYTES - 1] = 0x72;
    source[16 * FONT_GLYPH_BYTES] = 0x83;
    let image = decode_font_bank(&source, 17).unwrap();
    assert_eq!((image.width, image.height), (352, 44));
    let pixel = |x: usize, y: usize| {
        let start = (y * image.width as usize + x) * 4;
        &image.pixels[start..start + 4]
    };
    assert_eq!(pixel(0, 0), [255, 255, 255, 17]);
    assert_eq!(pixel(1, 0), [255, 255, 255, 255]);
    assert_eq!(pixel(20, 21), [255, 255, 255, 34]);
    assert_eq!(pixel(21, 21), [255, 255, 255, 119]);
    assert_eq!(pixel(0, 22), [255, 255, 255, 51]);
    assert_eq!(pixel(1, 22), [255, 255, 255, 136]);
    assert_eq!(pixel(22, 22), [0, 0, 0, 0]); // Unused atlas cell.
    assert!(decode_font_bank(&source[..source.len() - 1], 17).is_err());
    assert!(decode_font_bank(&source, 16).is_err());
    assert!(decode_font_bank(&[], 0).is_err());
    assert!(decode_font_bank(&[], FONT_GLYPH_COUNT + 1).is_err());
}

#[test]
fn font_mapping_never_aliases_controls_or_alternate_banks() {
    assert_eq!(glyph_index(0x2001), Some(1));
    assert_eq!(glyph_index(0x20A3), Some(163));
    for code in [0, 1, 0x00A4, 0x2000, 0x20A4, 0x6005, 0xFFFF] {
        assert_eq!(glyph_index(code), None);
    }
}

#[test]
#[ignore = "requires private messages and independently decoded full-width font; set ROSES_TEST_MESSAGE_PATH and ROSES_TEST_NAME_FONT_RGBA_PATH"]
fn private_name_font_matches_runtime_bank_and_grid_bounds() {
    let bytes = std::fs::read(std::env::var_os("ROSES_TEST_MESSAGE_PATH").unwrap()).unwrap();
    let font = decode_pal_name_font(&bytes).unwrap();
    assert_eq!((font.image.width, font.image.height), (352, 440));
    assert_eq!(font.name_glyph_rect(0x00A4), Some([88, 220, 22, 22]));
    assert_eq!(font.name_glyph_rect(0x013C), Some([264, 418, 22, 22]));
    for code in [0, 0xA3, 0x13D, 0x2000, 0x20A4, 0xFFFF] {
        assert_eq!(font.name_glyph_rect(code), None);
    }
    assert_eq!(
        decode_pal_message_font(&bytes)
            .unwrap()
            .name_glyph_rect(0xA4),
        None
    );
    let oracle =
        std::fs::read(std::env::var_os("ROSES_TEST_NAME_FONT_RGBA_PATH").unwrap()).unwrap();
    assert_eq!(font.image.pixels, oracle);
    let pages = parse_pal_messages(&bytes, &[6, 7, 9, 10]).unwrap();
    for page in &pages.messages[..2] {
        assert_eq!(page.lines.len(), 6);
        for line in &page.lines {
            assert_eq!(line.tokens.len(), 13);
        }
        for &code in page.expanded_codes.iter().filter(|&&code| code != 0x2000) {
            assert!(font.name_glyph_rect(code).is_some());
        }
    }
}

#[test]
#[ignore = "requires private English file and independent full-message JSON; set ROSES_TEST_MESSAGE_PATH and ROSES_TEST_MESSAGES_JSON_PATH"]
fn private_general_messages_match_all_independent_codes_and_font_pixels() {
    let bytes = std::fs::read(
        std::env::var_os("ROSES_TEST_MESSAGE_PATH").expect("set ROSES_TEST_MESSAGE_PATH"),
    )
    .unwrap();
    let expected_bytes = std::fs::read(
        std::env::var_os("ROSES_TEST_MESSAGES_JSON_PATH")
            .expect("set ROSES_TEST_MESSAGES_JSON_PATH"),
    )
    .unwrap();
    let expected: Vec<serde_json::Value> = serde_json::from_slice(&expected_bytes).unwrap();
    assert_eq!(expected.len(), MESSAGE_COUNT);
    for chunk in (0..MESSAGE_COUNT as u16)
        .collect::<Vec<_>>()
        .chunks(MAX_SELECTED_MESSAGES)
    {
        let selected = parse_pal_messages(&bytes, chunk).unwrap();
        assert!(!selected.gameplay_validated);
        assert_eq!(selected.messages.len(), chunk.len());
        for (actual, &id) in selected.messages.iter().zip(chunk) {
            assert_eq!(actual.message_id, id);
            assert_eq!(
                serde_json::to_value(&actual.expanded_codes).unwrap(),
                expected[usize::from(id)]["glyphs"]
            );
        }
    }
    let font = decode_pal_message_font(&bytes).unwrap();
    assert_eq!((font.image.width, font.image.height), (352, 242));
    assert_eq!(font.glyph_rect(0x209C), Some([264, 198, 22, 22]));
    assert_eq!(font.glyph_rect(0x2000), None);
    let oracle = std::fs::read(
        std::env::var_os("ROSES_TEST_FONT_RGBA_PATH").expect("set ROSES_TEST_FONT_RGBA_PATH"),
    )
    .unwrap();
    assert_eq!(font.image.pixels, oracle);
}

#[test]
#[ignore = "requires private English messages and independent name JSON; set ROSES_TEST_MESSAGE_PATH and ROSES_TEST_NAMES_JSON_PATH"]
fn private_pal_names_match_all_independent_expected_records() {
    let bytes = std::fs::read(
        std::env::var_os("ROSES_TEST_MESSAGE_PATH").expect("set ROSES_TEST_MESSAGE_PATH"),
    )
    .unwrap();
    let expected_bytes = std::fs::read(
        std::env::var_os("ROSES_TEST_NAMES_JSON_PATH").expect("set ROSES_TEST_NAMES_JSON_PATH"),
    )
    .unwrap();
    let expected: Vec<serde_json::Value> = serde_json::from_slice(&expected_bytes).unwrap();
    let names = parse_pal_card_names(&bytes).unwrap();
    assert_eq!(names.names.len(), CARD_COUNT);
    assert_eq!(expected.len(), CARD_COUNT);
    assert_eq!(names.language, "en");
    assert!(!names.gameplay_validated);
    let mut joined = String::new();
    for (actual, expected) in names.names.iter().zip(&expected) {
        assert_eq!(Some(u64::from(actual.id)), expected["card_id"].as_u64());
        assert_eq!(
            Some(u64::from(actual.message_id)),
            expected["string_index"].as_u64()
        );
        assert_eq!(Some(actual.name.as_str()), expected["name"].as_str());
        joined.push_str(&actual.name);
        joined.push('\n');
    }
    assert_eq!(
        format!("{:x}", Sha256::digest(joined.as_bytes())),
        "35a4bf360b28de98231437094949e336e24be6c3ecbd1f1b667330e00f0366a6"
    );
}

#[test]
#[ignore = "requires private PAL file and independent original-decoder oracle; set ROSES_TEST_MESSAGE_PATH and ROSES_TEST_NAME_TEMPLATE_ORACLE_PATH"]
fn private_player_name_templates_match_original_saved_and_short_name_oracle() {
    let bytes = std::fs::read(
        std::env::var_os("ROSES_TEST_MESSAGE_PATH").expect("set ROSES_TEST_MESSAGE_PATH"),
    )
    .unwrap();
    let oracle_bytes = std::fs::read(
        std::env::var_os("ROSES_TEST_NAME_TEMPLATE_ORACLE_PATH")
            .expect("set ROSES_TEST_NAME_TEMPLATE_ORACLE_PATH"),
    )
    .unwrap();
    let oracle: serde_json::Value = serde_json::from_slice(&oracle_bytes).unwrap();
    assert_eq!(
        oracle["message_sha256"].as_str(),
        Some(PAL_ENGLISH_MESSAGE_SHA256)
    );
    let cases = oracle["cases"].as_array().expect("oracle cases");
    assert!(cases.len() >= 4);
    let mut saw_saved = false;
    let mut saw_short = false;
    let mut saw_explicit_space = false;
    for case in cases {
        let stored_raw: Vec<u16> = serde_json::from_value(case["stored_codes"].clone()).unwrap();
        assert_eq!(stored_raw.len(), 12);
        let last = stored_raw
            .iter()
            .position(|word| word & 0x8000 != 0)
            .expect("original name terminator");
        assert!(stored_raw[last + 1..].iter().all(|word| *word == 0x2001));
        let codes: Vec<u16> = stored_raw[..=last]
            .iter()
            .map(|word| word & 0x3FFF)
            .collect();
        saw_saved |= case["source_kind"].as_str() == Some("saved_state");
        saw_short |= codes.len() == 1;
        saw_explicit_space |= codes.contains(&0x2001);
        let outputs = case["messages"].as_array().expect("message outputs");
        let ids: Vec<u16> = outputs
            .iter()
            .map(|message| message["message_id"].as_u64().unwrap() as u16)
            .collect();
        let templates = parse_pal_message_templates(&bytes, &ids).unwrap();
        assert_eq!(templates.messages.len(), outputs.len());
        for (template, expected) in templates.messages.iter().zip(outputs) {
            let actual = resolve_pal_message_template(template, &codes).unwrap();
            assert_eq!(
                serde_json::to_value(actual.expanded_codes).unwrap(),
                expected["glyphs"],
                "case {} message {}",
                case["id"],
                template.message_id
            );
        }
    }
    assert!(saw_saved && saw_short && saw_explicit_space);
    // Independently verify all static leaf outputs still agree with the mature
    // card/general importer; template token provenance is additional information.
    let offsets = container_offsets(&bytes).unwrap();
    let expanded =
        decode_template_messages(&bytes[TOKEN_POOL_START..TOKEN_POOL_END], &offsets).unwrap();
    let old = parse_container(&bytes).unwrap();
    for (new, old) in expanded.iter().zip(old) {
        assert_eq!(
            new.tokens.iter().map(|atom| atom.code).collect::<Vec<_>>(),
            old.glyphs
        );
    }
}
