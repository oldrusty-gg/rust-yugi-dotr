use super::*;
use std::collections::BTreeSet;

fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn packet() -> Vec<u8> {
    let mut b = vec![0; 80];
    for (i, v) in [0x6c01_8000, 3, 0x301e_4000, 0x412, 4, 0x6909_8001]
        .into_iter()
        .enumerate()
    {
        put(&mut b, i * 4, v);
    }
    for (i, v) in [
        -4096i16, 2048, 1, 0, 32767, 0, -20, 0, -20, 0, 0, 1, 0, 32767, 0, 20, 0, -20, 4096, 4096,
        1, 0, 32767, 0, 0, 0, 20,
    ]
    .into_iter()
    .enumerate()
    {
        b[24 + i * 2..26 + i * 2].copy_from_slice(&v.to_le_bytes());
    }
    b
}
fn draw(end: u32) -> StoryDraw {
    StoryDraw {
        pass: 0,
        dma_tag_offset: 0,
        stream_start: 0,
        stream_end_exclusive: end,
        texture_index: 0,
        material_words: [0; 12],
        gs_registers: Vec::new(),
    }
}
#[test]
fn strips_preserve_signed_vertices_gif_end_bit_and_draw_identity() {
    let mut b = packet();
    put(&mut b, 4, 0x8003);
    let strips = parse_strips(
        &b,
        Section {
            start: 0,
            end: b.len(),
        },
        &[draw(80)],
    )
    .unwrap();
    assert_eq!(strips.len(), 1);
    assert_eq!(strips[0].gif_tag_words[0], 0x8003);
    assert_eq!(
        strips[0].vertices[0],
        StoryVertex {
            texture_raw: [-4096, 2048, 1],
            normal_raw: [0, 32767, 0],
            position_raw: [-20, 0, -20]
        }
    );
    assert_eq!(strips[0].vertices[2].position_raw, [0, 0, 20]);
    assert_eq!(strips[0].draw_index, 0);
}
#[test]
fn rejects_truncation_wrong_counts_unknown_commands_and_split_draws() {
    let b = packet();
    for end in 0..b.len() {
        assert!(parse_strips(&b[..end], Section { start: 0, end }, &[draw(end as u32)]).is_err());
    }
    for (at, value) in [
        (0, 0x6d01_8000),
        (4, 86),
        (8, 0x301e_c000),
        (20, 0x690c_8001),
        (20, 0x6909_8002),
    ] {
        let mut bad = b.clone();
        put(&mut bad, at, value);
        assert!(parse_strips(&bad, Section { start: 0, end: 80 }, &[draw(80)]).is_err());
    }
    assert!(parse_strips(
        &b,
        Section { start: 0, end: 80 },
        &[
            draw(40),
            StoryDraw {
                pass: 0,
                stream_start: 40,
                ..draw(80)
            }
        ]
    )
    .is_err());
    let mut bad = b;
    bad[79] = 9;
    assert!(parse_strips(&bad, Section { start: 0, end: 80 }, &[draw(80)]).is_err());
}
fn dma_fixture() -> (Vec<u8>, Section, Section, BTreeMap<usize, u16>) {
    let mut b = vec![0; 0x350];
    let dma = Section {
        start: 0x100,
        end: 0x270,
    };
    let geo = Section {
        start: 0x300,
        end: 0x350,
    };
    for (at, ws) in [
        (0x100, [0x2000_0000, 0x2000_0080, 0x0300_0080, 0x0200_01c0]),
        (0x180, [0x1000_0008, 0, 0x1100_0000, 0x5100_0008]),
        (0x210, [0x1000_0003, 0, 0x1000_0000, 0x6c03_003c]),
        (0x250, [0x3000_0007, 0x2000_0010, 0x1000_0000, 0x6c07_0004]),
        (0x260, [0x3000_0005, 0x3000_0000, 0, 0]),
    ] {
        for (i, w) in ws.into_iter().enumerate() {
            put(&mut b, at + i * 4, w);
        }
    }
    put(&mut b, 0x190, 0x8007);
    put(&mut b, 0x194, 0x1000_0000);
    put(&mut b, 0x198, 14);
    for (i, reg) in [8, 0x34, 0x36, 0x47, 0x42, 6, 0x14].into_iter().enumerate() {
        put(&mut b, 0x1a8 + i * 16, reg);
    }
    for i in 0..12 {
        put(&mut b, 0x220 + i * 4, (i as f32).to_bits());
    }
    (b, dma, geo, BTreeMap::from([(0x1f0, 7)]))
}
#[test]
fn material_follows_tex0_patch_mapping_not_texture_order() {
    let (b, dma, geo, patches) = dma_fixture();
    let draws = parse_draws(&b, dma, geo, &patches, 0, &mut BTreeSet::new()).unwrap();
    assert_eq!(draws.len(), 1);
    assert_eq!(draws[0].texture_index, 7);
    assert_eq!(
        (draws[0].stream_start, draws[0].stream_end_exclusive),
        (0x300, 0x350)
    );
    assert_eq!(f32::from_bits(draws[0].material_words[11]), 11.0);
}
#[test]
fn rejects_dma_cycles_unmapped_textures_and_escaping_draws() {
    let (b, dma, geo, patches) = dma_fixture();
    assert!(parse_draws(&b, dma, geo, &BTreeMap::new(), 0, &mut BTreeSet::new()).is_err());
    for (at, w) in [
        (0x104, 0x2000_0000),
        (0x260, 0x3000_ffff),
        (0x264, 0x3000_0040),
        (0x180, 0x1000_ffff),
    ] {
        let mut bad = b.clone();
        put(&mut bad, at, w);
        assert!(parse_draws(&bad, dma, geo, &patches, 0, &mut BTreeSet::new()).is_err());
    }
}
#[test]
fn public_entry_rejects_incorrect_source_before_decoding() {
    assert!(decode_pal_story_geometry(&[]).is_err());
    assert!(decode_pal_story_geometry(&vec![0; ARCHIVE_BYTES]).is_err());
    assert!(Section::read(&[255; 32], 16).is_err());
}

#[test]
#[ignore = "requires ROSES_TEST_MODEL_PATH and ROSES_TEST_MESH_ORACLE from private independent extraction"]
fn private_scene_matches_independent_packet_and_material_oracle() {
    let path = std::env::var_os("ROSES_TEST_MODEL_PATH").expect("set ROSES_TEST_MODEL_PATH");
    let oracle_path =
        std::env::var_os("ROSES_TEST_MESH_ORACLE").expect("set ROSES_TEST_MESH_ORACLE");
    let bytes = std::fs::read(path).unwrap();
    let oracle: serde_json::Value =
        serde_json::from_slice(&std::fs::read(oracle_path).unwrap()).unwrap();
    let actual = decode_pal_story_geometry(&bytes).unwrap();
    assert_eq!(actual.models.len(), 8);
    for (m, o) in actual.models.iter().zip(oracle.as_array().unwrap()) {
        assert_eq!(m.observed_object_slot, o["slot"].as_u64().unwrap() as u16);
        assert_eq!(m.strips.len(), o["strips"].as_array().unwrap().len());
        assert_eq!(m.source_sha256, o["source_sha256"].as_str().unwrap());
        let mut hash = Sha256::new();
        for (s, expected) in m.strips.iter().zip(o["strips"].as_array().unwrap()) {
            assert_eq!(
                serde_json::json!([s.record_offset, s.vertices.len(), s.draw_index]),
                *expected
            );
            for v in &s.vertices {
                for x in v
                    .texture_raw
                    .into_iter()
                    .chain(v.normal_raw)
                    .chain(v.position_raw)
                {
                    hash.update(x.to_le_bytes());
                }
            }
        }
        assert_eq!(
            format!("{:x}", hash.finalize()),
            o["vertex_bytes_sha256"].as_str().unwrap()
        );
        let draws: Vec<_> = m
            .draws
            .iter()
            .map(|d| {
                serde_json::json!([
                    d.stream_start,
                    d.stream_end_exclusive,
                    d.texture_index,
                    d.material_words
                ])
            })
            .collect();
        assert_eq!(serde_json::json!(draws), o["draws"]);
        assert_eq!(
            m.textures.len(),
            o["texture_offsets"].as_array().unwrap().len()
        );
        for (index, (t, expected)) in m
            .textures
            .iter()
            .zip(o["texture_offsets"].as_array().unwrap())
            .enumerate()
        {
            assert_eq!(t.file_offset - m.file_offset, expected.as_u64().unwrap());
            let image = opening_textures::decode_record(&bytes, t).unwrap();
            assert_eq!(
                image.pixels.len(),
                image.width as usize * image.height as usize * 4
            );
            assert_eq!(
                format!("{:x}", Sha256::digest(&image.pixels)),
                o["texture_rgba_sha256"][index].as_str().unwrap()
            );
        }
    }
    assert!(!actual.rendering_validated && !actual.gameplay_validated);
}
