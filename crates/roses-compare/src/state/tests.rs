use super::*;
use crate::pine::ReadWidth;
use std::io::Write;
use std::sync::OnceLock;
use zip::write::SimpleFileOptions;

fn watch() -> MemoryWatch {
    MemoryWatch {
        name: "synthetic_word".into(),
        address: 16,
        width: ReadWidth::Bits64,
    }
}

fn archive(name: &str, data: &[u8], method: zip::CompressionMethod) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file(
            name,
            SimpleFileOptions::default().compression_method(method),
        )
        .unwrap();
    writer.write_all(data).unwrap();
    writer.finish().unwrap().into_inner()
}

fn valid_archive() -> &'static Vec<u8> {
    static ZIP: OnceLock<Vec<u8>> = OnceLock::new();
    ZIP.get_or_init(|| {
        let mut memory = vec![0; EE_MEMORY_BYTES];
        memory[16..24].copy_from_slice(&[8, 7, 6, 5, 4, 3, 2, 1]);
        archive("eeMemory.bin", &memory, zip::CompressionMethod::Zstd)
    })
}

#[test]
fn reads_actual_sized_synthetic_zstd_memory_with_exact_hashes_and_scope() {
    let bytes = valid_archive();
    let capture = capture_bytes(bytes, &[watch()]).unwrap();
    assert_eq!(capture.observations[0].value, 0x0102030405060708);
    assert_eq!(capture.ee_memory_bytes, 33_554_432);
    assert_eq!(capture.compression_method, "zstandard");
    assert_eq!(capture.state_sha256, format!("{:x}", Sha256::digest(bytes)));
    let mut expected = vec![0; EE_MEMORY_BYTES];
    expected[16..24].copy_from_slice(&[8, 7, 6, 5, 4, 3, 2, 1]);
    assert_eq!(
        capture.ee_memory_sha256,
        format!("{:x}", Sha256::digest(expected))
    );
    assert!(capture.atomic_saved_memory);
    assert!(!capture.normalized_game_state);
    assert!(capture.scope.contains("not whole-machine consistency"));
}

#[test]
fn supports_deflate_and_all_little_endian_widths_at_ram_boundary() {
    let mut memory = vec![0; EE_MEMORY_BYTES];
    memory[EE_MEMORY_BYTES - 8..]
        .copy_from_slice(&[0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01]);
    let bytes = archive("eeMemory.bin", &memory, zip::CompressionMethod::Deflated);
    let watches: Vec<_> = [
        ReadWidth::Bits8,
        ReadWidth::Bits16,
        ReadWidth::Bits32,
        ReadWidth::Bits64,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, width)| MemoryWatch {
        name: format!("width_{index}"),
        address: (EE_MEMORY_BYTES - 8) as u32,
        width,
    })
    .collect();
    let capture = capture_bytes(&bytes, &watches).unwrap();
    assert_eq!(capture.compression_method, "deflate");
    assert_eq!(
        capture
            .observations
            .iter()
            .map(|v| v.value)
            .collect::<Vec<_>>(),
        [0xef, 0xcdef, 0x89ab_cdef, 0x0123_4567_89ab_cdef]
    );
}

#[test]
fn rejects_missing_short_and_duplicate_ee_entries() {
    let missing = archive("other.bin", &[0; 8], zip::CompressionMethod::Stored);
    assert!(capture_bytes(&missing, &[watch()]).is_err());
    let short = archive("eeMemory.bin", &[0; 8], zip::CompressionMethod::Stored);
    assert!(capture_bytes(&short, &[watch()]).is_err());
    // Duplicate the complete central entry, pointing to the same local data.
    // zip 2.4.2 itself silently deduplicates this; our preflight must reject it.
    let mut duplicate = valid_archive().clone();
    let footer_offset = duplicate.len() - 22;
    let directory_start = u32_at(&duplicate, footer_offset + 16) as usize;
    let central = duplicate[directory_start..footer_offset].to_vec();
    let mut footer = duplicate[footer_offset..].to_vec();
    footer[8..10].copy_from_slice(&2u16.to_le_bytes());
    footer[10..12].copy_from_slice(&2u16.to_le_bytes());
    footer[12..16].copy_from_slice(&((central.len() * 2) as u32).to_le_bytes());
    duplicate.truncate(footer_offset);
    duplicate.extend_from_slice(&central);
    duplicate.extend_from_slice(&footer);
    assert!(capture_bytes(&duplicate, &[watch()])
        .unwrap_err()
        .to_string()
        .contains("duplicate"));
}

#[test]
fn rejects_truncation_local_central_disagreement_and_wrong_actual_decode_size() {
    let original = valid_archive();
    assert!(capture_bytes(&original[..original.len() - 1], &[watch()]).is_err());
    let mut mismatch = original.clone();
    mismatch[30] = b'X'; // Change only the local entry filename.
    assert!(capture_bytes(&mismatch, &[watch()]).is_err());
    // A tiny stream falsely advertises the required size in both headers.
    let mut wrong_size = archive("eeMemory.bin", b"tiny", zip::CompressionMethod::Stored);
    let footer = wrong_size.len() - 22;
    let central = u32_at(&wrong_size, footer + 16) as usize;
    wrong_size[22..26].copy_from_slice(&(EE_MEMORY_BYTES as u32).to_le_bytes());
    wrong_size[central + 24..central + 28].copy_from_slice(&(EE_MEMORY_BYTES as u32).to_le_bytes());
    assert!(capture_bytes(&wrong_size, &[watch()]).is_err());
}

#[test]
fn rejects_bad_crc_and_compressed_stream_corruption() {
    let mut bytes = valid_archive().clone();
    let footer = bytes.len() - 22;
    let central = u32_at(&bytes, footer + 16) as usize;
    bytes[14..18].copy_from_slice(&1u32.to_le_bytes());
    bytes[central + 16..central + 20].copy_from_slice(&1u32.to_le_bytes());
    assert!(capture_bytes(&bytes, &[watch()]).is_err());
    let mut corrupted = valid_archive().clone();
    let payload = 30 + usize::from(u16_at(&corrupted, 26)) + usize::from(u16_at(&corrupted, 28));
    corrupted[payload] ^= 0xff;
    assert!(capture_bytes(&corrupted, &[watch()]).is_err());
}

#[test]
fn rejects_zip64_multidisk_and_excessive_entry_count() {
    for (offset, value) in [(4, 1u16), (10, u16::MAX), (10, 257)] {
        let mut bytes = valid_archive().clone();
        let footer = bytes.len() - 22;
        bytes[footer + offset..footer + offset + 2].copy_from_slice(&value.to_le_bytes());
        if offset == 10 {
            bytes[footer + 8..footer + 10].copy_from_slice(&value.to_le_bytes());
        }
        assert!(capture_bytes(&bytes, &[watch()]).is_err());
    }
}

#[test]
fn rejects_parser_overriding_central_extras_and_malformed_extra_lengths() {
    for kind in [0x0001u16, 0x7075, 0x9901] {
        let mut bytes = valid_archive().clone();
        let old_footer = bytes.len() - 22;
        let central = u32_at(&bytes, old_footer + 16) as usize;
        let extra_position = central + 46 + usize::from(u16_at(&bytes, central + 28));
        let old_extra_length = u16_at(&bytes, central + 30);
        let old_directory_size = u32_at(&bytes, old_footer + 12);
        let mut extra = Vec::from(kind.to_le_bytes());
        extra.extend_from_slice(&24u16.to_le_bytes());
        extra.extend_from_slice(&[0; 24]);
        bytes.splice(extra_position..extra_position, extra.iter().copied());
        bytes[central + 30..central + 32]
            .copy_from_slice(&(old_extra_length + extra.len() as u16).to_le_bytes());
        let footer = old_footer + extra.len();
        bytes[footer + 12..footer + 16]
            .copy_from_slice(&(old_directory_size + extra.len() as u32).to_le_bytes());
        assert!(validate_directory(&bytes)
            .unwrap_err()
            .to_string()
            .contains("metadata overrides"));
    }
    assert!(validate_extra_fields(&[1]).is_err());
    assert!(validate_extra_fields(&[0x55, 0x54, 2, 0, 0]).is_err());
    assert!(validate_extra_fields(&[0x55, 0x54, 1, 0, 0]).is_ok());
}

#[test]
fn rejects_invalid_watches_before_file_access() {
    let missing = Path::new("intentionally-missing-offline-state.p2s");
    assert!(capture_state(missing, &[])
        .unwrap_err()
        .to_string()
        .contains("memory watches"));
    let mut invalid_watch = watch();
    invalid_watch.address = EE_MEMORY_BYTES as u32 - 1;
    assert!(capture_state(missing, &[invalid_watch])
        .unwrap_err()
        .to_string()
        .contains("outside physical EE RAM"));
    assert!(capture_state(missing, &[watch(), watch()])
        .unwrap_err()
        .to_string()
        .contains("unique"));
}

#[test]
fn public_reader_preserves_source_and_rejects_oversized_archive_before_reading() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "roses-state-test-{}-{stamp}.p2s",
        std::process::id()
    ));
    std::fs::write(&path, valid_archive()).unwrap();
    let capture = capture_state(&path, &[watch()]).unwrap();
    assert_eq!(
        capture.state_sha256,
        format!("{:x}", Sha256::digest(std::fs::read(&path).unwrap()))
    );
    assert_eq!(std::fs::read(&path).unwrap(), *valid_archive());
    let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.set_len(MAX_STATE_BYTES + 1).unwrap();
    drop(file);
    assert!(capture_state(&path, &[watch()])
        .unwrap_err()
        .to_string()
        .contains("128 MiB"));
    std::fs::remove_file(path).unwrap();
}
