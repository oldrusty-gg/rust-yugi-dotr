use super::*;

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn synthetic_picture() -> Vec<u8> {
    let mut bytes = vec![0u8; PICTURE_RECORD_STRIDE];
    bytes[..4].copy_from_slice(b"RH2\0");
    bytes[8..12].copy_from_slice(&(PICTURE_CONTENT_BYTES as u32).to_le_bytes());
    put_u16(&mut bytes, 0x54, 128);
    put_u16(&mut bytes, 0x56, 128);
    for (index, word) in [
        (0, 0x8000),
        (1, 0xFFFF),
        (2, 0x001F),
        (3, 0xFC00),
        (8, 0x83E0),
        (16, 0x801F),
    ] {
        put_u16(&mut bytes, PALETTE_OFFSET + index * 2, word);
    }
    // Explicit storage positions exercise separate PS2 rows and palette lanes.
    bytes[FIRST_PIXELS] = 8;
    bytes[FIRST_PIXELS + 4] = 16;
    bytes[FIRST_PIXELS + 17] = 3;
    bytes[SECOND_PIXELS] = 1;
    bytes[SECOND_PIXELS + PIXEL_HALF_BYTES - 1] = 2;
    bytes
}

fn pixel(image: &RgbaImage, x: usize, y: usize) -> &[u8] {
    let offset = (y * image.width as usize + x) * 4;
    &image.pixels[offset..offset + 4]
}

#[test]
fn packed_card_fields_preserve_raw_words_and_unknown_bytes() {
    let mut bytes = [0; CARD_RECORD_STRIDE];
    bytes[..4].copy_from_slice(&[14, 11, 0x65, 42]);
    put_u16(&mut bytes, 4, 0x1234);
    put_u16(&mut bytes, 6, 0xBEEF);
    put_u16(&mut bytes, 8, 0xA345);
    put_u16(&mut bytes, 10, 0x5ABC);
    bytes[12..20].copy_from_slice(&[1, 3, 5, 7, 9, 11, 13, 15]);
    let card = parse_card_record(27, &bytes).unwrap();
    assert_eq!((card.id, card.kind, card.kind_offset_raw), (27, 14, 11));
    assert_eq!((card.level, card.attribute, card.deck_cost), (6, 5, 42));
    assert_eq!((card.effect_id_raw, card.xax_id_raw), (0x1234, 0xBEEF));
    assert_eq!((card.attack, card.defense), (0x0345, 0x1ABC));
    assert_eq!(
        (card.attack_word_raw, card.defense_word_raw),
        (0xA345, 0x5ABC)
    );
    assert_eq!(card.opaque_tail, [1, 3, 5, 7, 9, 11, 13, 15]);
}

#[test]
fn rejects_wrong_table_lengths_and_implausible_records() {
    let table = vec![0; CARD_COUNT * CARD_RECORD_STRIDE];
    assert_eq!(parse_card_table(&table).unwrap().len(), CARD_COUNT);
    assert!(parse_card_table(&table[..table.len() - 1]).is_err());
    assert!(parse_card_record(0, &[0; 19]).is_err());
    for (offset, bad_value) in [(0, 31), (2, 0xD0), (2, 0x06)] {
        let mut record = [0; 20];
        record[offset] = bad_value;
        assert!(parse_card_record(0, &record).is_err());
    }
}

#[test]
fn executable_fingerprint_gate_rejects_unknown_and_truncated_inputs() {
    for bytes in [
        b"".as_slice(),
        b"not a supported executable".as_slice(),
        b"\x7fELF".as_slice(),
    ] {
        assert!(matches!(
            parse_pal_catalog(bytes),
            Err(Error::UnsupportedExecutable { .. })
        ));
    }
}

#[test]
fn color_channels_and_alpha_use_the_correct_bits() {
    assert_eq!(rgba5551(0), [0, 0, 0, 0]);
    assert_eq!(rgba5551(0x801F), [255, 0, 0, 255]);
    assert_eq!(rgba5551(0x83E0), [0, 255, 0, 255]);
    assert_eq!(rgba5551(0xFC00), [0, 0, 255, 255]);
    assert_eq!(rgba5551(0x7FFF), [255, 255, 255, 0]);
    assert_eq!(rgba5551(0xFFFF), [255, 255, 255, 255]);
    assert_eq!(rgba5551(16), [131, 0, 0, 0]);
}

#[test]
fn picture_decoding_resolves_storage_rows_palette_lanes_and_second_block() {
    let image = decode_picture(&synthetic_picture(), 0).unwrap();
    assert_eq!(
        (image.width, image.height, image.pixels.len()),
        (128, 128, 128 * 128 * 4)
    );
    assert_eq!(pixel(&image, 0, 0), [255, 0, 0, 255]);
    assert_eq!(pixel(&image, 1, 0), [0, 255, 0, 255]);
    assert_eq!(pixel(&image, 0, 2), [0, 0, 255, 255]);
    assert_eq!(pixel(&image, 0, 64), [255, 255, 255, 255]);
    assert_eq!(pixel(&image, 127, 127), [255, 0, 0, 0]);
    assert_eq!(pixel(&image, 10, 10), [0, 0, 0, 255]);
}

#[test]
fn unswizzle_covers_each_input_pixel_exactly_once() {
    let mut visits = vec![false; PICTURE_PIXELS];
    for y in 0..128 {
        for x in 0..128 {
            let offset = ps2_indexed_offset(x, y);
            assert!(offset < PICTURE_PIXELS);
            assert!(!visits[offset], "duplicate source pixel {offset}");
            visits[offset] = true;
        }
    }
    assert!(visits.into_iter().all(|visited| visited));
    for (x, y, expected) in [
        (0, 0, 0),
        (8, 0, 2),
        (16, 0, 32),
        (0, 1, 256),
        (0, 2, 17),
        (0, 4, 528),
        (0, 16, 2048),
        (127, 127, 16383),
    ] {
        assert_eq!(ps2_indexed_offset(x, y), expected);
    }
}

#[test]
fn palette_permutation_preserves_other_bits_and_is_reversible() {
    assert_eq!(swap_palette_bits(8), 16);
    assert_eq!(swap_palette_bits(16), 8);
    assert_eq!(swap_palette_bits(24), 24);
    for input in 0..=255 {
        assert_eq!(swap_palette_bits(swap_palette_bits(input)), input);
        assert_eq!(swap_palette_bits(input) & 0xE7, input & 0xE7);
    }
}

#[test]
fn rejects_incomplete_archives_extreme_indices_and_wrong_headers() {
    let valid = synthetic_picture();
    assert!(decode_picture(&[], 0).is_err());
    assert!(decode_picture(&valid[..valid.len() - 1], 0).is_err());
    assert!(matches!(
        decode_picture(&valid, 1),
        Err(Error::PictureIndexOutOfRange { .. })
    ));
    assert!(matches!(
        decode_picture(&valid, usize::MAX),
        Err(Error::PictureIndexOutOfRange { .. })
    ));
    for offset in [0, 8, 0x54, 0x56] {
        let mut invalid = valid.clone();
        invalid[offset] ^= 1;
        assert!(matches!(
            decode_picture(&invalid, 0),
            Err(Error::Unsupported(_))
        ));
    }
}

#[test]
fn requested_picture_uses_its_own_record() {
    let mut archive = synthetic_picture();
    let mut second = synthetic_picture();
    put_u16(&mut second, PALETTE_OFFSET + 16 * 2, 0xFFFF);
    archive.extend_from_slice(&second);
    assert_eq!(
        pixel(&decode_picture(&archive, 0).unwrap(), 0, 0),
        [255, 0, 0, 255]
    );
    assert_eq!(
        pixel(&decode_picture(&archive, 1).unwrap(), 0, 0),
        [255, 255, 255, 255]
    );
}

#[test]
#[ignore = "requires private extracted PAL assets; set ROSES_TEST_ELF_PATH and ROSES_TEST_PICTURE_PATH"]
fn private_pal_catalog_and_all_pictures_decode() {
    let elf =
        std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").expect("set ROSES_TEST_ELF_PATH"))
            .unwrap();
    let archive = std::fs::read(
        std::env::var_os("ROSES_TEST_PICTURE_PATH").expect("set ROSES_TEST_PICTURE_PATH"),
    )
    .unwrap();
    let catalog = parse_pal_catalog(&elf).unwrap();
    assert_eq!(catalog.cards.len(), 854);
    assert_eq!(
        (
            catalog.cards[0].attack,
            catalog.cards[0].defense,
            catalog.cards[0].level
        ),
        (3000, 2500, 8)
    );
    assert_eq!(
        (
            catalog.cards[2].attack,
            catalog.cards[2].defense,
            catalog.cards[2].level
        ),
        (4500, 3800, 12)
    );
    assert_eq!(
        catalog.cards.iter().filter(|card| card.kind <= 20).count(),
        683
    );
    assert_eq!(catalog.cards[853].kind, 160);
    assert!(!catalog.gameplay_validated);
    assert_eq!(archive.len(), PICTURE_RECORD_COUNT * PICTURE_RECORD_STRIDE);
    for index in 0..PICTURE_RECORD_COUNT {
        let image = decode_picture(&archive, index).unwrap();
        assert_eq!(image.pixels.len(), 128 * 128 * 4);
        if index == 0 {
            // Digest of a separately generated local Python reference image.
            // The original picture bytes are deliberately absent from this tree.
            assert_eq!(
                format!("{:x}", Sha256::digest(&image.pixels)),
                "d6775927cb0335aec1d7d4faff4464e89b047fe11d6a5e0f4aa1c2856e068180"
            );
        }
    }
}
