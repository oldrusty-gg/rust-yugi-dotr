use super::*;

// Generated packet structure with invented values; no retail fixture.
fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn strip(gif1: u32) -> Vec<u8> {
    let mut b = vec![0; 80];
    for (i, v) in [0x6c01_8000, 3, gif1, 0x412, 4, 0x6909_8001]
        .into_iter()
        .enumerate()
    {
        put(&mut b, i * 4, v);
    }
    for (i, v) in [
        0i16, 0, 1, 0, 32767, 0, -20, 0, -20, 4096, 0, 1, 0, 32767, 0, 20, 0, -20, 0, 4096, 1, 0,
        32767, 0, 0, 0, 20,
    ]
    .into_iter()
    .enumerate()
    {
        b[24 + i * 2..26 + i * 2].copy_from_slice(&v.to_le_bytes());
    }
    b
}
fn draw(start: u32, end: u32) -> BoardDraw {
    BoardDraw {
        dma_tag_offset: 0,
        stream_start: start,
        stream_end_exclusive: end,
        texture_index: 0,
        material_words: [0; 12],
        gs_registers: Vec::new(),
        matrix_slot: 0,
    }
}

#[test]
fn strips_record_texture_mapping_flag_and_raw_vertices() {
    let mut b = strip(GIF_TEXTURED);
    b.extend(strip(GIF_UNTEXTURED));
    let strips = parse_board_strips(
        &b,
        Section { start: 0, end: 160 },
        &[draw(0, 80), draw(80, 160)],
    )
    .unwrap();
    assert_eq!(strips.len(), 2);
    assert!(strips[0].textured && !strips[1].textured);
    assert_eq!((strips[0].draw_index, strips[1].draw_index), (0, 1));
    assert_eq!(strips[1].vertices[1].texture_raw, [4096, 0, 1]);
    assert_eq!(strips[1].vertices[2].position_raw, [0, 0, 20]);
}

#[test]
fn strips_reject_other_primitives_counts_and_padding() {
    for (at, value) in [
        (8, 0x301e_c000),
        (8, 0x3016_0000),
        (4, 2),
        (20, 0x6909_8002),
    ] {
        let mut bad = strip(GIF_TEXTURED);
        put(&mut bad, at, value);
        assert!(parse_board_strips(&bad, Section { start: 0, end: 80 }, &[draw(0, 80)]).is_err());
    }
    let mut bad = strip(GIF_UNTEXTURED);
    bad[79] = 1;
    assert!(parse_board_strips(&bad, Section { start: 0, end: 80 }, &[draw(0, 80)]).is_err());
    let good = strip(GIF_UNTEXTURED);
    assert!(parse_board_strips(&good, Section { start: 0, end: 80 }, &[draw(0, 40)]).is_err());
}

/// Two matrix blocks (initial jump 0xF0), one GS/material setup, two draws
/// that each select a different block.
fn dma_fixture() -> (Vec<u8>, Section, Section, BTreeMap<usize, u16>) {
    let mut b = vec![0; 0x450];
    for (at, ws) in [
        (0x100, [0x2000_0000, 0x2000_00f0, 0x0300_0080, 0x0200_01c0]),
        (0x1f0, [0x1000_0008, 0, 0x1100_0000, 0x5100_0008]),
        (0x280, [0x1000_0003, 0, 0x1000_0000, 0x6c03_003c]),
        (0x2c0, [0x3000_0007, 0x2000_0010, 0x1000_0000, 0x6c07_0004]),
        (0x2d0, [0x3000_0002, 0x3000_0000, 0, 0]),
        (0x2e0, [0x3000_0007, 0x2000_0080, 0x1000_0000, 0x6c07_0004]),
        (0x2f0, [0x3000_0003, 0x3000_0020, 0, 0]),
    ] {
        for (i, w) in ws.into_iter().enumerate() {
            put(&mut b, at + i * 4, w);
        }
    }
    put(&mut b, 0x200, 0x8007);
    put(&mut b, 0x204, 0x1000_0000);
    put(&mut b, 0x208, 14);
    for (i, reg) in [8, 0x34, 0x36, 0x47, 0x42, 6, 0x14].into_iter().enumerate() {
        put(&mut b, 0x218 + i * 16, reg);
    }
    for i in 0..12 {
        put(&mut b, 0x290 + i * 4, (i as f32).to_bits());
    }
    (
        b,
        Section {
            start: 0x100,
            end: 0x300,
        },
        Section {
            start: 0x400,
            end: 0x450,
        },
        BTreeMap::from([(0x260, 3)]),
    )
}

#[test]
fn draws_follow_matrix_block_references() {
    let (b, dma, geo, patches) = dma_fixture();
    let (draws, slots) = parse_board_draws(&b, dma, geo, &patches).unwrap();
    assert_eq!(slots, 2);
    assert_eq!(draws.len(), 2);
    assert_eq!((draws[0].matrix_slot, draws[1].matrix_slot), (0, 1));
    assert_eq!(
        (draws[1].stream_start, draws[1].stream_end_exclusive),
        (0x420, 0x450)
    );
    assert_eq!(draws[0].texture_index, 3);
    assert_eq!(f32::from_bits(draws[1].material_words[11]), 11.0);
    assert_eq!(draws[0].gs_registers[3][1], 0x47);
}

#[test]
fn draws_reject_missing_or_out_of_range_matrix_blocks() {
    let (b, dma, geo, patches) = dma_fixture();
    for (at, w) in [
        // Matrix block 2 is not reserved by the 0xF0 jump.
        (0x2e4, 0x2000_00f0),
        // Unaligned matrix block address.
        (0x2c4, 0x2000_0020),
        // Jump that is not a whole number of blocks.
        (0x104, 0x2000_00f8),
        // Initial tag with a payload.
        (0x100, 0x2000_0001),
    ] {
        let mut bad = b.clone();
        put(&mut bad, at, w);
        assert!(
            parse_board_draws(&bad, dma, geo, &patches).is_err(),
            "{at:x}"
        );
    }
    // A draw before any matrix reference (swap the first reference and draw).
    let mut bad = b.clone();
    let (first, second) = (b[0x2c0..0x2d0].to_vec(), b[0x2d0..0x2e0].to_vec());
    bad[0x2c0..0x2d0].copy_from_slice(&second);
    bad[0x2d0..0x2e0].copy_from_slice(&first);
    assert!(parse_board_draws(&bad, dma, geo, &patches).is_err());
    // Runtime matrix blocks must be empty on disc.
    let mut bad = b;
    bad[0x150] = 1;
    assert!(parse_board_draws(&bad, dma, geo, &patches).is_err());
}

#[test]
fn public_entry_rejects_incorrect_source_before_decoding() {
    assert!(decode_pal_duel_board(&[]).is_err());
    assert!(decode_pal_duel_board(&vec![0; ARCHIVE_BYTES]).is_err());
    assert!(record(&[0; 64], 0).is_err());
    let mut small = vec![0u8; 0x200];
    small[..4].copy_from_slice(b"RH0\0");
    put(&mut small, 12, 0x400);
    assert!(record(&small, 0).is_err());
}
