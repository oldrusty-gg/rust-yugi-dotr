use super::*;
use std::io::Cursor;
use std::sync::atomic::{AtomicU64, Ordering};

const BLOCK: usize = SECTOR as usize;

fn number16(target: &mut [u8], value: u16) {
    target[..2].copy_from_slice(&value.to_le_bytes());
    target[2..4].copy_from_slice(&value.to_be_bytes());
}
fn number32(target: &mut [u8], value: u32) {
    target[..4].copy_from_slice(&value.to_le_bytes());
    target[4..8].copy_from_slice(&value.to_be_bytes());
}
fn record(name: &[u8], lba: u32, size: u32, directory: bool) -> Vec<u8> {
    let length = 33 + name.len() + usize::from(name.len().is_multiple_of(2));
    let mut bytes = vec![0; length];
    bytes[0] = length as u8;
    number32(&mut bytes[2..10], lba);
    number32(&mut bytes[10..18], size);
    bytes[25] = if directory { 2 } else { 0 };
    number16(&mut bytes[28..32], 1);
    bytes[32] = name.len() as u8;
    bytes[33..33 + name.len()].copy_from_slice(name);
    bytes
}
fn directory(image: &mut [u8], sector: usize, records: &[Vec<u8>]) {
    let mut offset = sector * BLOCK;
    for record in records {
        image[offset..offset + record.len()].copy_from_slice(record);
        offset += record.len();
    }
}
fn fixture() -> Vec<u8> {
    let mut image = vec![0; 40 * BLOCK];
    let pvd = &mut image[16 * BLOCK..17 * BLOCK];
    pvd[0] = 1;
    pvd[1..6].copy_from_slice(b"CD001");
    pvd[6] = 1;
    pvd[40..72].fill(b' ');
    pvd[40..49].copy_from_slice(b"SYNTHETIC");
    number32(&mut pvd[80..88], 40);
    number16(&mut pvd[120..124], 1);
    number16(&mut pvd[124..128], 1);
    number16(&mut pvd[128..132], BLOCK as u16);
    let root = record(&[0], 20, BLOCK as u32, true);
    pvd[156..156 + root.len()].copy_from_slice(&root);
    let terminator = &mut image[17 * BLOCK..18 * BLOCK];
    terminator[0] = 255;
    terminator[1..6].copy_from_slice(b"CD001");
    terminator[6] = 1;
    let cnf = b"BOOT2 = cdrom0:\\SLES_515.79;1\r\nVMODE = PAL\r\n";
    directory(
        &mut image,
        20,
        &[
            record(&[0], 20, BLOCK as u32, true),
            record(&[1], 20, BLOCK as u32, true),
            record(b"DATA", 21, BLOCK as u32, true),
            record(b"README.TXT;1", 22, 5, false),
            record(b"SYSTEM.CNF;1", 24, cnf.len() as u32, false),
            record(b"SLES_515.79;1", 25, 4, false),
        ],
    );
    directory(
        &mut image,
        21,
        &[
            record(&[0], 21, BLOCK as u32, true),
            record(&[1], 20, BLOCK as u32, true),
            record(b"ITEM.BIN;1", 23, 3, false),
        ],
    );
    image[22 * BLOCK..22 * BLOCK + 5].copy_from_slice(b"hello");
    image[23 * BLOCK..23 * BLOCK + 3].copy_from_slice(&[7, 8, 9]);
    image[24 * BLOCK..24 * BLOCK + cnf.len()].copy_from_slice(cnf);
    image[25 * BLOCK..25 * BLOCK + 4].copy_from_slice(b"FAKE");
    image
}
fn inventory(image: Vec<u8>) -> Result<Inventory> {
    let len = image.len() as u64;
    parse_image(&mut Cursor::new(image), len)
}
struct TempIso(std::path::PathBuf);
impl TempIso {
    fn new(bytes: &[u8]) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "roses-disc-test-{}-{}.iso",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, bytes).unwrap();
        Self(path)
    }
}
impl Drop for TempIso {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn traverses_nested_directories_and_reads_only_requested_file() {
    let source = TempIso::new(&fixture());
    let mut disc = Disc::open(&source.0).unwrap();
    let paths: Vec<_> = disc.entries().iter().map(|e| e.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "DATA",
            "DATA/ITEM.BIN",
            "README.TXT",
            "SLES_515.79",
            "SYSTEM.CNF"
        ]
    );
    assert_eq!(disc.read_file("/data\\item.bin;1", 3).unwrap(), [7, 8, 9]);
    assert_eq!(disc.read_file("readme.txt", 5).unwrap(), b"hello");
    assert!(matches!(
        disc.read_file("readme.txt", 4),
        Err(Error::Limit(_))
    ));
    assert!(matches!(
        disc.read_file("DATA", 10000),
        Err(Error::NotFound(_))
    ));
    assert!(matches!(
        disc.read_file("missing", 100),
        Err(Error::NotFound(_))
    ));
    assert!(matches!(
        disc.read_file("DATA/../README.TXT", 100),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn inspection_does_not_trust_boot_name_without_matching_fingerprints() {
    let image = fixture();
    let expected_hash = format!("{:x}", Sha256::digest(&image));
    let source = TempIso::new(&image);
    let report = Disc::open(&source.0).unwrap().inspect().unwrap();
    assert_eq!(report.volume_id, "SYNTHETIC");
    assert_eq!(report.iso_sha256, expected_hash);
    assert_eq!(
        report.executable_sha256,
        Some(format!("{:x}", Sha256::digest(b"FAKE")))
    );
    assert_eq!(
        report.boot.unwrap().boot_executable.as_deref(),
        Some("SLES_515.79")
    );
    assert_eq!(
        report.identification.status,
        IdentificationStatus::Unrecognized
    );
    assert!(report.identification.candidate_profile.is_none());
    assert!(!report.identification.gameplay_validated);
}

#[test]
fn rejects_declared_volume_or_extent_outside_image() {
    let mut image = fixture();
    number32(&mut image[16 * BLOCK + 80..16 * BLOCK + 88], 41);
    assert!(matches!(inventory(image), Err(Error::Invalid(_))));
    let mut image = fixture();
    directory(&mut image, 21, &[record(b"BAD.BIN;1", 40, 1, false)]);
    assert!(matches!(inventory(image), Err(Error::Invalid(_))));
}

#[test]
fn rejects_malformed_directory_record_and_identifier() {
    let mut image = fixture();
    image[20 * BLOCK] = 12;
    assert!(matches!(inventory(image), Err(Error::Invalid(_))));
    let mut image = fixture();
    image[20 * BLOCK + 32] = 200;
    assert!(matches!(inventory(image), Err(Error::Invalid(_))));
}

#[test]
fn rejects_records_crossing_sector_boundary() {
    let mut image = fixture();
    image[20 * BLOCK..21 * BLOCK].fill(0);
    // A sequence of valid records ending near the sector boundary.
    let mut offset = 20 * BLOCK;
    for i in 0..51 {
        let name = format!("A{i:04}");
        let r = record(name.as_bytes(), 22, 1, false); // 38 bytes
        image[offset..offset + r.len()].copy_from_slice(&r);
        offset += r.len();
    }
    let r = record(
        b"LONGFILENAME1234567890123456789012345678901234567890123456789012345678901234567890",
        22,
        1,
        false,
    );
    assert!(offset + r.len() > 21 * BLOCK);
    image[offset..offset + r.len()].copy_from_slice(&r);
    assert!(matches!(inventory(image), Err(Error::Invalid(_))));
}

#[test]
fn rejects_mismatched_endian_values() {
    let mut image = fixture();
    image[16 * BLOCK + 84] ^= 1;
    assert!(matches!(inventory(image), Err(Error::Invalid(_))));
    let mut image = fixture();
    image[20 * BLOCK + 6] ^= 1;
    assert!(matches!(inventory(image), Err(Error::Invalid(_))));
}

#[test]
fn explicitly_rejects_multi_extent_interleaving_and_extended_attributes() {
    for (field, value) in [(25, 0x82), (26, 1), (27, 1), (1, 1)] {
        let mut image = fixture();
        image[20 * BLOCK + field] = value;
        assert!(matches!(inventory(image), Err(Error::Unsupported(_))));
    }
}

#[test]
fn rejects_directory_cycles_and_path_traversal() {
    let mut image = fixture();
    image[20 * BLOCK..21 * BLOCK].fill(0);
    directory(&mut image, 20, &[record(b"LOOP", 20, BLOCK as u32, true)]);
    assert!(matches!(inventory(image), Err(Error::Invalid(_))));
    let mut image = fixture();
    image[20 * BLOCK..21 * BLOCK].fill(0);
    directory(&mut image, 20, &[record(b"../ESCAPE;1", 22, 5, false)]);
    assert!(matches!(inventory(image), Err(Error::Invalid(_))));
}

#[test]
fn rejects_ambiguous_case_folded_paths() {
    let mut image = fixture();
    image[20 * BLOCK..21 * BLOCK].fill(0);
    directory(
        &mut image,
        20,
        &[record(b"A;1", 22, 1, false), record(b"a;1", 22, 1, false)],
    );
    assert!(matches!(inventory(image), Err(Error::Invalid(_))));
}

#[test]
fn rejects_truncated_images_descriptors_and_padding_garbage() {
    let mut image = fixture();
    image.pop();
    assert!(matches!(inventory(image), Err(Error::Invalid(_))));
    let mut image = fixture();
    image[17 * BLOCK + 1] = b'X';
    assert!(matches!(inventory(image), Err(Error::Invalid(_))));
    let mut image = fixture();
    image[21 * BLOCK - 1] = 1;
    assert!(matches!(inventory(image), Err(Error::Invalid(_))));
}

#[test]
fn parses_boot_configuration_and_rejects_unsafe_paths() {
    let config = parse_system_cnf(b"BOOT2=cdrom0:\\DIR\\GAME.ELF;1\nVMODE=pal\n").unwrap();
    assert_eq!(config.boot_executable.as_deref(), Some("DIR/GAME.ELF"));
    assert_eq!(config.video_mode.as_deref(), Some("PAL"));
    assert!(parse_system_cnf(b"BOOT2=cdrom0:\\..\\OUT;1").is_err());
    assert!(parse_system_cnf(b"BOOT2=host:GAME.ELF").is_err());
    assert!(parse_system_cnf(b"BOOT2=cdrom0:A;1\nBOOT2=cdrom0:B;1").is_err());
}

#[test]
#[ignore = "requires the user's private ISO; set ROSES_TEST_ISO_PATH"]
fn local_pal_iso_matches_known_candidate() {
    let path = std::env::var_os("ROSES_TEST_ISO_PATH")
        .expect("set ROSES_TEST_ISO_PATH to your private ISO");
    let mut disc = Disc::open(path).unwrap();
    assert!(disc.entries().iter().any(|e| e.path == "SLES_515.79"));
    let report = disc.inspect().unwrap();
    assert_eq!(
        report.identification.status,
        IdentificationStatus::KnownRetailCandidate
    );
    assert_eq!(
        report.identification.candidate_profile.as_deref(),
        Some(PAL_PROFILE_ID)
    );
    assert!(!report.identification.gameplay_validated);
}
