use super::*;

fn put32(b: &mut [u8], at: usize, value: u32) {
    b[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn put64(b: &mut [u8], at: usize, value: u64) {
    b[at..at + 8].copy_from_slice(&value.to_le_bytes());
}
fn packet(b: &mut [u8], at: usize, payload: usize, w: u64, h: u64) {
    let n = payload as u64 / 16;
    put64(b, at, 0x6000_0000 | (n + 5));
    put64(b, at + 8, (0x5100_0000 | (n + 5)) << 32);
    put64(b, at + 16, 0x1000_0000_0000_0003);
    put64(b, at + 24, 0xe);
    put64(b, at + 40, 0x51);
    put64(b, at + 48, w | h << 32);
    put64(b, at + 56, 0x52);
    put64(b, at + 72, 0x53);
    put64(b, at + 80, 0x0800_0000_0000_8000 | n);
}
/// Generated packet structure and invented palette/index data; no retail fixture.
fn synthetic(width: usize, height: usize, rgba32: bool) -> Vec<u8> {
    let pages = width * height / PAGE_BYTES;
    let entry_bytes = if rgba32 { 4 } else { 2 };
    let palette = 0x60 + (pages + 1) * 48;
    let palette_size = 0x60 + 256 * entry_bytes;
    let images = palette + palette_size;
    let size = images + pages * PACKET_BYTES;
    let mut b = vec![0u8; size];
    b[..4].copy_from_slice(b"RH2\0");
    for (offset, value) in [
        (8, size),
        (0x10, 0x60),
        (0x14, (pages + 1) * 48),
        (0x18, palette),
        (0x1c, palette_size),
        (0x20, images),
        (0x24, pages * PACKET_BYTES),
        (0x28, size),
        (0x50, if rgba32 { 0x13 } else { 0x20013 }),
        (0x54, width | height << 16),
        (0x58, pages + 1),
    ] {
        put32(&mut b, offset, value as u32);
    }
    for index in 0..=pages {
        let at = 0x60 + index * 48;
        let pointer = if index == 0 {
            palette
        } else {
            images + (index - 1) * PACKET_BYTES
        };
        let format = if index == 0 && !rgba32 {
            0x0201_0000
        } else {
            0x0001_0000
        };
        for (offset, value) in [
            (0, 0x5000_0002),
            (4, pointer as u32),
            (8, 0x1100_0000),
            (12, 0x5100_0002),
            (16, 0x8001),
            (20, 0x1000_0000),
            (24, 0xe),
            (36, format),
            (40, 0x50),
        ] {
            put32(&mut b, at + offset, value);
        }
    }
    packet(&mut b, palette, 256 * entry_bytes, 16, 16);
    for page in 0..pages {
        let start = images + page * PACKET_BYTES;
        packet(&mut b, start, PAGE_BYTES, 64, 32);
        b[start + 0x60..start + PACKET_BYTES].fill(page as u8 + 1);
    }
    if rgba32 {
        for (index, value) in [
            (1, 0x8000_00ff),
            (2, 0x4000_ff00),
            (3, 0xffff_0000),
            (4, 0x0011_2233),
        ] {
            put32(&mut b, palette + 0x60 + index * 4, value);
        }
    } else {
        for (index, value) in [(1, 0x801fu16), (2, 0x83e0), (3, 0xfc00), (4, 0)] {
            b[palette + 0x60 + index * 2..palette + 0x62 + index * 2]
                .copy_from_slice(&value.to_le_bytes());
        }
    }
    b
}

#[test]
fn pages_are_tiled_horizontally_and_vertically_without_interleaving() {
    let mut b = synthetic(256, 128, false);
    put32(&mut b, 0x2c, 0x1234_5678);
    let (records, padding) = inspect_records(&b).unwrap();
    assert!(padding.is_empty());
    assert_eq!(records[0].raw_header_words[11], 0x1234_5678);
    assert_eq!(records[0].palette_words_raw[1], 0x801f);
    let image = decode_record(&b, &records[0]).unwrap();
    for (x, y, expected) in [
        (0, 0, [255, 0, 0, 255]),
        (127, 63, [255, 0, 0, 255]),
        (128, 0, [0, 255, 0, 255]),
        (255, 63, [0, 255, 0, 255]),
        (0, 64, [0, 0, 255, 255]),
        (127, 127, [0, 0, 255, 255]),
        (128, 64, [0, 0, 0, 0]),
        (255, 127, [0, 0, 0, 0]),
    ] {
        let offset = (y * 256 + x) * 4;
        assert_eq!(&image.pixels[offset..offset + 4], &expected);
    }
}

#[test]
fn byte_lanes_palette_permutation_and_alpha_conversion_are_explicit() {
    for (x, y, expected) in [
        (0, 0, 0),
        (1, 0, 4),
        (4, 0, 16),
        (8, 0, 2),
        (16, 0, 32),
        (0, 1, 256),
        (0, 2, 17),
    ] {
        assert_eq!(page_index(x, y), expected);
    }
    let mut seen = std::collections::BTreeSet::new();
    for y in 0..64 {
        for x in 0..128 {
            assert!(seen.insert(page_index(x, y)));
        }
    }
    assert_eq!(seen.into_iter().max(), Some(PAGE_BYTES - 1));
    let mut b = synthetic(128, 64, true);
    let palette = u32_at(&b, 0x18).unwrap() as usize + 0x60;
    put32(&mut b, palette + 8 * 4, 0x4000_ff00);
    put32(&mut b, palette + 16 * 4, 0x8000_00ff);
    let pixels = u32_at(&b, 0x20).unwrap() as usize + 0x60;
    b[pixels] = 8;
    b[pixels + 4] = 16;
    let (records, _) = inspect_records(&b).unwrap();
    let image = decode_record(&b, &records[0]).unwrap();
    assert_eq!(&image.pixels[..8], &[255, 0, 0, 255, 0, 255, 0, 127]);
    assert_eq!(
        color(0xff11_2233, OpeningPaletteFormat::Rgba8888Alpha128),
        [0x33, 0x22, 0x11, 255]
    );
    assert_eq!(
        color(0x0011_2233, OpeningPaletteFormat::Rgba8888Alpha128),
        [0x33, 0x22, 0x11, 0]
    );
}

#[test]
fn indexed_decode_preserves_gs_alpha_before_display_conversion() {
    let mut bytes = synthetic(128, 64, true);
    let palette = u32_at(&bytes, 0x18).unwrap() as usize + 0x60;
    let pixels = u32_at(&bytes, 0x20).unwrap() as usize + 0x60;
    // First two pixels independently exercise both directions of the CLUT
    // permutation. The remaining alpha values straddle the GS unity value128.
    for (x, stored_index, palette_index, alpha) in [
        (0, 8, 16, 129u32),
        (1, 16, 8, 255),
        (2, 1, 1, 0),
        (3, 2, 2, 1),
        (4, 3, 3, 127),
        (5, 4, 4, 128),
    ] {
        put32(
            &mut bytes,
            palette + palette_index * 4,
            alpha << 24 | 0x123456,
        );
        bytes[pixels + page_index(x, 0)] = stored_index;
    }
    let (records, _) = inspect_records(&bytes).unwrap();
    let indexed = decode_indexed_record(&bytes, &records[0]).unwrap();
    assert_eq!(&indexed.palette_indices[..6], &[16, 8, 1, 2, 3, 4]);
    let raw_colors: Vec<u32> = indexed.palette_indices[..6]
        .iter()
        .map(|index| indexed.palette_words_raw[usize::from(*index)])
        .collect();
    assert_eq!(
        raw_colors,
        [0x81123456, 0xff123456, 0x00123456, 0x01123456, 0x7f123456, 0x80123456]
    );
    let display = decode_record(&bytes, &records[0]).unwrap();
    let alpha: Vec<u8> = display.pixels[..24].chunks_exact(4).map(|p| p[3]).collect();
    assert_eq!(alpha, [255, 255, 0, 1, 253, 255]);
}

#[test]
fn indexed_decode_retains_unexpanded_16_bit_words_across_pages() {
    let bytes = synthetic(256, 128, false);
    let (records, _) = inspect_records(&bytes).unwrap();
    let image = decode_indexed_record(&bytes, &records[0]).unwrap();
    assert_eq!((image.width, image.height), (256, 128));
    assert_eq!(image.palette_words_raw[1..5], [0x801f, 0x83e0, 0xfc00, 0]);
    assert_eq!(image.palette_indices.len(), 256 * 128);
    for (x, y, index) in [(127, 63, 1), (128, 63, 2), (127, 64, 3), (128, 64, 4)] {
        assert_eq!(image.palette_indices[y * 256 + x], index);
    }
}

#[test]
fn malformed_lengths_regions_descriptors_and_formats_are_rejected() {
    let original = synthetic(128, 64, false);
    for length in [0, 3, 95, original.len() - 1] {
        assert!(inspect_records(&original[..length]).is_err());
    }
    for (offset, value) in [
        (8, u32::MAX),
        (8, 16),
        (0x18, u32::MAX),
        (0x1c, 0x460),
        (0x20, 0x60),
        (0x24, 0),
        (0x50, 0x14),
        (0x54, 0),
        (0x54, 129 | 64 << 16),
        (0x54, 2048 | 64 << 16),
        (0x58, 9),
        (0x64, 0),
    ] {
        let mut changed = original.clone();
        put32(&mut changed, offset, value);
        assert!(
            inspect_records(&changed).is_err(),
            "accepted changed header at {offset:x}"
        );
    }
    let mut changed = original;
    let packet = u32_at(&changed, 0x20).unwrap() as usize;
    put32(&mut changed, packet + 0x30, 128);
    assert!(inspect_records(&changed).is_err());
}

#[test]
fn archive_padding_is_bounded_zero_and_aligned_and_records_have_offsets() {
    let record = synthetic(128, 64, false);
    let mut archive = record.clone();
    archive.resize(archive.len().next_multiple_of(2048), 0);
    let second = archive.len();
    archive.extend_from_slice(&record);
    let (records, padding) = inspect_records(&archive).unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[1].file_offset, second as u64);
    assert_eq!(padding.len(), 1);
    let mut changed = archive.clone();
    changed[record.len()] = 1;
    assert!(inspect_records(&changed).is_err());
    let mut extra = record.clone();
    extra.extend_from_slice(&[0; 2048]);
    assert!(inspect_records(&extra).is_err());
    let mut unaligned = record;
    unaligned.extend_from_slice(&[0; 16]);
    assert!(inspect_records(&unaligned).is_err());
}

#[test]
fn public_pal_entry_points_reject_unknown_or_mixed_archive_bytes() {
    assert!(inspect_pal_opening_archive(&[0; 16], OpeningArchiveKind::TexEtc).is_err());
    let unknown = vec![0; OpeningArchiveKind::TexEtc.expected_size()];
    assert!(matches!(
        inspect_pal_opening_archive(&unknown, OpeningArchiveKind::TexEtc),
        Err(Error::Unsupported(_))
    ));
    assert!(decode_pal_opening_texture(&unknown, OpeningArchiveKind::TexEtc, 0).is_err());
    let unknown = vec![0; OpeningArchiveKind::TexAnm.expected_size()];
    assert!(matches!(
        inspect_pal_opening_archive(&unknown, OpeningArchiveKind::TexAnm),
        Err(Error::Unsupported(_))
    ));
}

#[test]
#[ignore = "requires private opening archives and independent Python RGBA oracle"]
fn private_all_opening_records_match_independent_decode_oracle() {
    let root = std::path::PathBuf::from(
        std::env::var_os("ROSES_OPENING_TEXTURE_ROOT").expect("set ROSES_OPENING_TEXTURE_ROOT"),
    );
    let oracle: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("textures-provisional.json")).unwrap())
            .unwrap();
    for (kind, name) in [
        (OpeningArchiveKind::TexSys, "TEXSYS"),
        (OpeningArchiveKind::TexEve, "TEXEVE"),
        (OpeningArchiveKind::TexEtc, "TEXETC"),
    ] {
        let bytes = std::fs::read(root.join(format!("{name}.MRG"))).unwrap();
        let report = inspect_pal_opening_archive(&bytes, kind).unwrap();
        let expected = oracle
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["kind"] == name)
            .unwrap();
        assert_eq!(expected["sha256"], report.archive_sha256);
        for record in &report.records {
            let entry = &expected["records"][usize::from(record.id)];
            assert_eq!(entry["offset"], record.file_offset);
            assert_eq!(entry["size"], record.byte_len);
            let image = decode_pal_opening_texture(&bytes, kind, usize::from(record.id)).unwrap();
            assert_eq!(entry["width"], image.width);
            assert_eq!(entry["height"], image.height);
            assert_eq!(
                entry["rgba_sha256"],
                format!("{:x}", Sha256::digest(image.pixels))
            );
        }
        assert!(decode_pal_opening_texture(&bytes, kind, report.records.len()).is_err());
    }
}

#[test]
#[ignore = "requires private story effect archive and independent RGBA oracle"]
fn private_story_effect_records_match_independent_decode_oracle() {
    verify_private_story_archive(OpeningArchiveKind::TexEff, "TEXEFF");
}

#[test]
#[ignore = "requires private animated portrait archive and independent RGBA oracle"]
fn private_story_portrait_records_match_independent_decode_oracle() {
    verify_private_story_archive(OpeningArchiveKind::TexAnm, "TEXANM");
}

fn verify_private_story_archive(kind: OpeningArchiveKind, name: &str) {
    let root = std::path::PathBuf::from(
        std::env::var_os("ROSES_STORY_TEXTURE_ROOT").expect("set ROSES_STORY_TEXTURE_ROOT"),
    );
    let oracle: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("asset-probe.json")).unwrap()).unwrap();
    let expected = oracle
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["kind"] == name)
        .unwrap();
    let bytes = std::fs::read(root.join(format!("{name}.MRG"))).unwrap();
    let report = inspect_pal_opening_archive(&bytes, kind).unwrap();
    assert_eq!(
        report.records.len(),
        expected["records"].as_array().unwrap().len()
    );
    for record in &report.records {
        let image = decode_pal_opening_texture(&bytes, kind, usize::from(record.id)).unwrap();
        let expected = &expected["records"][usize::from(record.id)];
        assert_eq!(
            expected["rgba_sha256"],
            format!("{:x}", Sha256::digest(image.pixels))
        );
    }
}

/// One small linear P_8 rectangle (observed for 64x64 and 64x32 model
/// textures); generated structure and invented indices only.
fn synthetic_small(width: usize, height: usize) -> Vec<u8> {
    let palette = 0x60 + 2 * 48;
    let palette_size = 0x60 + 256 * 2;
    let images = palette + palette_size;
    let size = images + 0x60 + width * height;
    let mut b = vec![0u8; size];
    b[..4].copy_from_slice(b"RH2\0");
    for (offset, value) in [
        (8, size),
        (0x10, 0x60),
        (0x14, 2 * 48),
        (0x18, palette),
        (0x1c, palette_size),
        (0x20, images),
        (0x24, 0x60 + width * height),
        (0x28, size),
        (0x50, 0x20013),
        (0x54, width | height << 16),
        (0x58, 2),
    ] {
        put32(&mut b, offset, value as u32);
    }
    // The P_8 rectangle's BITBLTBUF DBW is its width in 64-pixel units.
    let rect_format = 0x1300_0000 | (width.div_ceil(64) as u32) << 16;
    for (index, pointer, format) in [(0, palette, 0x0201_0000), (1, images, rect_format)] {
        let at = 0x60 + index * 48;
        for (offset, value) in [
            (0, 0x5000_0002),
            (4, pointer as u32),
            (8, 0x1100_0000),
            (12, 0x5100_0002),
            (16, 0x8001),
            (20, 0x1000_0000),
            (24, 0xe),
            (36, format),
            (40, 0x50),
        ] {
            put32(&mut b, at + offset, value);
        }
    }
    packet(&mut b, palette, 512, 16, 16);
    packet(&mut b, images, width * height, width as u64, height as u64);
    for (i, pixel) in b[images + 0x60..].iter_mut().enumerate() {
        *pixel = (i % 251) as u8;
    }
    b
}

#[test]
fn small_rectangles_are_linear_and_keep_the_palette_permutation() {
    for (width, height) in [(64, 64), (64, 32), (32, 16), (128, 32), (128, 16), (96, 64)] {
        let bytes = synthetic_small(width, height);
        let (records, _) = inspect_records(&bytes).unwrap();
        assert_eq!(records[0].pixel_packet_count, 1);
        let image = decode_indexed_record(&bytes, &records[0]).unwrap();
        for (x, y) in [
            (0, 0),
            (width - 1, 0),
            (5, height - 1),
            (width - 1, height - 1),
        ] {
            let raw = ((y * width + x) % 251) as u8;
            let permuted = (raw & !24) | ((raw & 8) << 1) | ((raw & 16) >> 1);
            assert_eq!(image.palette_indices[y * width + x], permuted);
        }
    }
    let good = synthetic_small(64, 32);
    // Destination format must be P_8 for a rectangle, and its GIF size exact.
    let mut changed = good.clone();
    put32(&mut changed, 0x60 + 48 + 36, 0x0001_0000);
    assert!(inspect_records(&changed).is_err());
    // ... with the buffer width of its own width (1 for 64 pixels).
    let mut changed = good.clone();
    put32(&mut changed, 0x60 + 48 + 36, 0x1302_0000);
    assert!(inspect_records(&changed).is_err());
    let mut changed = good.clone();
    let images = u32_at(&changed, 0x20).unwrap() as usize;
    put32(&mut changed, images + 0x30, 32);
    assert!(inspect_records(&changed).is_err());
    // Other dimensions than the record was built for disagree with its
    // packets (a one-packet 64x128 is two bands in the data).
    for dims in [48 | 32 << 16, 64 | 128 << 16, 8 | 8 << 16] {
        let mut changed = good.clone();
        put32(&mut changed, 0x54, dims);
        assert!(inspect_records(&changed).is_err());
    }
}

/// The block/column model behind the P_4 page table reproduces the P_8
/// page swizzle the decoder has always used, pixel for pixel.
#[test]
fn indexed_block_model_matches_the_p8_page_swizzle_and_p4_is_a_permutation() {
    let mut host_word = vec![0usize; 2048];
    for i in 0..2048 {
        host_word[c32_page_word(i % 64, i / 64)] = i;
    }
    const BLOCKS8: [[usize; 8]; 4] = [
        [0, 1, 4, 5, 16, 17, 20, 21],
        [2, 3, 6, 7, 18, 19, 22, 23],
        [8, 9, 12, 13, 24, 25, 28, 29],
        [10, 11, 14, 15, 26, 27, 30, 31],
    ];
    for y in 0..64 {
        for x in 0..128 {
            let (word, byte) = indexed_block_position(x % 16, y % 16);
            let gs_word = BLOCKS8[y / 16][x / 16] * 64 + word;
            assert_eq!(host_word[gs_word] * 4 + byte, page_index(x, y), "({x},{y})");
        }
    }
    let mut seen = vec![false; 128 * 128];
    for y in 0..128 {
        for x in 0..128 {
            let nibble = p4_page_nibble(x, y);
            assert!(!seen[nibble]);
            seen[nibble] = true;
        }
    }
}
